use crate::{
    common::{
        Id,
        Offset,
        Version,
    },
    graph::Graph,
    offsets::OffsetLists,
};
use anyhow::Context as _;
use byteorder::{
    LittleEndian,
    ReadBytesExt as _,
    WriteBytesExt as _,
};
use regex_lite::Regex;
use std::{
    collections::BTreeMap,
    fs::File,
    io::Read,
    path::Path,
};
use walkdir::WalkDir;

pub struct AddressBin {
    mappings: Vec<(Id, Offset)>,
}

impl AddressBin {
    fn parse<R: Read>(src: &mut R) -> anyhow::Result<Self> {
        let mut read_u64 = || {
            src.read_u64::<LittleEndian>()
                .context("error while reading address bin")
        };
        let len = read_u64().context("failed to read len")?;
        let mut mappings = Vec::new();
        for _ in 0..len {
            let id = read_u64()
                .context("failed to read id")?
                .try_into()
                .context("read an id with an invalid representation")?;
            let offset = read_u64()
                .context("failed to read offset")?
                .try_into()
                .context("read an offset too large to fit into a u32")?;
            mappings.push((id, Offset(offset)));
        }
        Ok(Self { mappings })
    }

    pub fn iter(&self) -> impl Iterator<Item = &(Id, Offset)> {
        self.mappings.iter()
    }
}

pub struct AddressBins {
    bins: BTreeMap<Version, AddressBin>,
}

impl AddressBins {
    pub fn parse_all(root_dir: &Path) -> anyhow::Result<Self> {
        println!("parsing address bins...");

        let pattern = Regex::new(r"version-(\d+)-(\d+)-(\d+)-(\d+)\.bin")
            .context("failed to build file name pattern")?;
        let mut bins = BTreeMap::new();

        for dir_entry in WalkDir::new(root_dir) {
            let dir_entry = dir_entry.with_context(|| {
                format!("error while locating address bins in directory: {root_dir:?}")
            })?;
            let path = dir_entry.path();
            let metadata = dir_entry
                .metadata()
                .with_context(|| format!("failed to get metadata for file: {path:?}"))?;
            if metadata.is_file() {
                if let Some(file_name) = path.file_name().and_then(|x| x.to_str()) {
                    if let Some(captures) = pattern.captures(file_name) {
                        let version: Version =
                            (&captures[1], &captures[2], &captures[3], &captures[4])
                                .try_into()
                                .with_context(|| {
                                    format!("failed to parse version from file name: {path:?}")
                                })?;
                        let bin = {
                            let mut file = File::open(path)
                                .with_context(|| format!("failed to open file: {path:?}"))?;
                            AddressBin::parse(&mut file)
                                .with_context(|| format!("failed to parse address bin: {path:?}"))
                        }?;
                        bins.insert(version, bin);
                    }
                }
            }
        }

        Ok(Self { bins })
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Version, &AddressBin)> {
        self.bins.iter()
    }

    pub fn largest_unused_id(&self) -> Id {
        self.bins
            .values()
            .flat_map(|x| &x.mappings)
            .map(|x| x.0)
            .max()
            .map(Id::next)
            .unwrap_or_default()
    }
}

pub fn write_bins(
    root_dir: &Path,
    graph: &Graph,
    offset_lists: &OffsetLists,
    address_bins: &AddressBins,
) -> anyhow::Result<()> {
    println!("writing bins...");

    for (version, offset_list) in offset_lists.iter() {
        if !address_bins.bins.contains_key(version) {
            let mut file = {
                let path = root_dir.join(format!(
                    "version-{}-{}-{}-{}.bin",
                    version[0], version[1], version[2], version[3]
                ));
                if path.exists() {
                    anyhow::bail!("can not write to file because it already exists: {path:?}");
                }
                File::create(&path).with_context(|| format!("failed to create file: {path:?}"))
            }?;
            let mut write_u64 = |x| {
                file.write_u64::<LittleEndian>(x)
                    .with_context(|| format!("failed write for address bin: {version}"))
            };

            let mappings = {
                let mut v = offset_list
                    .iter()
                    .map(|(offset, mapping)| {
                        let id = graph.get(mapping.ix);
                        (id.get(), u64::from(offset.0))
                    })
                    .collect::<Vec<_>>();
                v.sort_by_key(|x| x.0);
                v
            };

            write_u64(mappings.len() as u64)?;
            for (id, offset) in mappings {
                write_u64(id)?;
                write_u64(offset)?;
            }
        }
    }

    Ok(())
}
