use crate::{
    common::{
        Offset,
        Version,
    },
    graph::{
        Graph,
        Ix,
    },
};
use anyhow::Context as _;
use petgraph::graph::NodeIndex;
use regex_lite::Regex;
use std::{
    collections::BTreeMap,
    fs::File,
    io::{
        BufRead,
        BufReader,
    },
    path::Path,
};
use walkdir::WalkDir;

pub struct Mapping {
    pub ix: NodeIndex<Ix>,
}

pub struct OffsetList {
    offsets: BTreeMap<Offset, Mapping>,
}

impl OffsetList {
    const FUNCTION_PATTERN: &'static str = r"func\t([\dA-Fa-f]+)\t[\dA-Fa-f]+";
    const GLOBAL_PATTERN: &'static str = r"global\t([\dA-Fa-f]+)";
    const NAME_PATTERN: &'static str = r"name\t([\dA-Fa-f]+)";

    fn parse(idaexport: &Path, graph: &mut Graph) -> anyhow::Result<Self> {
        let buffer_reader = |file_name| -> anyhow::Result<_> {
            let path = idaexport.join(file_name);
            let file =
                File::open(&path).with_context(|| format!("failed to open file: {path:?}"))?;
            Ok(BufReader::new(file))
        };

        let offsets = {
            let base_address = {
                let mut file = buffer_reader("idaexport_base.txt")?;
                Self::parse_base_address(&mut file).context("failed to parse idaexport_base.txt")
            }?;
            let do_parse = |file_name, pattern| {
                let mut file = buffer_reader(file_name)?;
                Self::parse_generic_offsets(&mut file, base_address, pattern)
                    .with_context(|| format!("failed to parse {file_name}"))
            };
            let function_offsets = do_parse("idaexport_func.txt", Self::FUNCTION_PATTERN)?;
            let global_offsets = do_parse("idaexport_global.txt", Self::GLOBAL_PATTERN)?;
            let name_offsets = do_parse("idaexport_name.txt", Self::NAME_PATTERN)?;

            function_offsets
                .into_iter()
                .chain(global_offsets)
                .chain(name_offsets)
                .map(|x| {
                    (
                        x,
                        Mapping {
                            ix: graph.add_node(),
                        },
                    )
                })
                .collect()
        };

        Ok(Self { offsets })
    }

    fn parse_base_address<R: BufRead>(idaexport_base: &mut R) -> anyhow::Result<u64> {
        let mut buffer = String::new();
        macro_rules! read_line {
            () => {{
                buffer.clear();
                idaexport_base.read_line(&mut buffer)
            }};
        }
        let version_pattern =
            Regex::new(r"version\t(\d+)").context("failed to build version pattern")?;
        let address_pattern = Regex::new(r"baseaddress\t([\dA-Fa-f]+)")
            .context("failed to build base address pattern")?;

        read_line!().context("failed to read version")?;
        let captures = version_pattern
            .captures(&buffer)
            .context("failed to match version pattern")?;
        if &captures[1] != "1" {
            anyhow::bail!("unsupported version: {}", &captures[1]);
        }

        read_line!().context("failed to read base address")?;
        let captures = address_pattern
            .captures(&buffer)
            .context("failed to match base address pattern")?;
        u64::from_str_radix(&captures[1], 16)
            .with_context(|| format!("failed to parse base address: {}", &captures[1]))
    }

    fn parse_generic_offsets<R: BufRead>(
        idaexport: &mut R,
        base_address: u64,
        pattern: &str,
    ) -> anyhow::Result<Vec<Offset>> {
        let mut buffer = String::new();
        macro_rules! read_line {
            () => {{
                buffer.clear();
                idaexport.read_line(&mut buffer)
            }};
        }
        let version_pattern =
            Regex::new(r"version\t(\d+)").context("failed to build version pattern")?;
        let name_pattern = Regex::new(pattern).context("failed to build offset pattern")?;

        read_line!().context("failed to read version")?;
        let captures = version_pattern
            .captures(&buffer)
            .context("failed to match version pattern")?;
        if &captures[1] != "1" {
            anyhow::bail!("unsupported version: {}", &captures[1]);
        };

        let mut offsets = Vec::new();
        loop {
            break match read_line!() {
                Ok(0) => Ok(offsets),
                Ok(_) if buffer.trim().is_empty() => Ok(offsets),
                Ok(_) => {
                    let captures = name_pattern
                        .captures(&buffer)
                        .context("failed to match offset pattern")?;
                    let offset = Self::parse_offset(base_address, &captures[1])?;
                    offsets.push(Offset(offset));
                    continue;
                }
                Err(err) => Err(err).context("failed to read offset"),
            };
        }
    }

    fn parse_offset(base_address: u64, string: &str) -> anyhow::Result<u32> {
        let address = u64::from_str_radix(string, 16)
            .with_context(|| format!("failed to parse address: {string}"))?;
        let offset: u32 = address
			.checked_sub(base_address)
			.with_context(|| format!("base address ({base_address}) is larger than given address ({address})"))?
			.try_into()
			.with_context(|| format!("given address ({address}) is too large to convert into an offset from the base address ({base_address})"))?;
        Ok(offset)
    }

    pub fn get(&self, key: Offset) -> Option<&Mapping> {
        self.offsets.get(&key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Offset, &Mapping)> {
        self.offsets.iter()
    }
}

pub struct OffsetLists {
    db: BTreeMap<Version, OffsetList>,
}

impl OffsetLists {
    pub fn parse_all(root_dir: &Path) -> anyhow::Result<(Self, Graph)> {
        println!("parsing offsets...");

        let mut db = BTreeMap::default();
        let mut graph = Graph::default();
        let dir_pattern =
            Regex::new(r"(\d+)\.(\d+)\.(\d+)").context("failed to build directory pattern")?;

        for dir_entry in WalkDir::new(root_dir) {
            let dir_entry = dir_entry.with_context(|| {
                format!("error while locating idaexport directories in directory: {root_dir:?}")
            })?;
            let path = dir_entry.path();
            let metadata = dir_entry
                .metadata()
                .with_context(|| format!("failed to get metadata for directory entry: {path:?}"))?;
            if metadata.is_dir() {
                if let Some(file_name) = path.file_name().and_then(|x| x.to_str()) {
                    if let Some(captures) = dir_pattern.captures(file_name) {
                        let version: Version = (&captures[1], &captures[2], &captures[3])
                            .try_into()
                            .with_context(|| {
                                format!("failed to construct version from directory name: {path:?}")
                            })?;
                        let offsets = OffsetList::parse(path, &mut graph).with_context(|| {
                            format!("failed to parse offset list from directory: {path:?}")
                        })?;
                        db.insert(version, offsets);
                    }
                }
            }
        }

        Ok((Self { db }, graph))
    }

    pub fn get(&self, key: Version) -> Option<&OffsetList> {
        self.db.get(&key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Version, &OffsetList)> {
        self.db.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::OffsetList;
    use std::io::BufReader;

    #[test]
    fn test_base_address() -> anyhow::Result<()> {
        let mut buffer = BufReader::new(
            &br"version	1
baseaddress	140000000
"[..],
        );
        let result = OffsetList::parse_base_address(&mut buffer)?;
        assert_eq!(result, 0x140000000);
        Ok(())
    }

    #[test]
    fn test_function_offsets() -> anyhow::Result<()> {
        let mut buffer = BufReader::new(
            &br"version	1
func	140001000	14000100B
func	140001060	14000106B
func	140001080	140001083
func	140001090	140001105
func	140001110	140001113
func	140001120	14000112C
func	140001140	140001170
func	140001180	140001187
"[..],
        );
        let result = OffsetList::parse_generic_offsets(
            &mut buffer,
            0x140000000,
            OffsetList::FUNCTION_PATTERN,
        )?
        .iter()
        .map(|x| x.0)
        .collect::<Vec<_>>();
        assert_eq!(
            result,
            [0x1000, 0x1060, 0x1080, 0x1090, 0x1110, 0x1120, 0x1140, 0x1180]
        );
        Ok(())
    }

    #[test]
    fn test_global_offsets() -> anyhow::Result<()> {
        let mut buffer = BufReader::new(
            &br"version	1
global	142C0F30C	char[4]
global	142C166DC	char[292]
global	142C17000	BOOL __stdcall(LPSTR lpBuffer, LPDWORD pcbBuffer)
global	146736290	PVOID
global	14674C73B
global	146A8C000
global	146A8F570
"[..],
        );
        let result = OffsetList::parse_generic_offsets(
            &mut buffer,
            0x140000000,
            OffsetList::GLOBAL_PATTERN,
        )?
        .iter()
        .map(|x| x.0)
        .collect::<Vec<_>>();
        assert_eq!(
            result,
            [0x2C0F30C, 0x2C166DC, 0x2C17000, 0x6736290, 0x674C73B, 0x6A8C000, 0x6A8F570]
        );
        Ok(())
    }

    #[test]
    fn test_name_offsets() -> anyhow::Result<()> {
        let mut buffer = BufReader::new(
			&br"version	1
name	140001000	??0_Fac_node@std@@QEAA@PEAU01@PEAV_Facet_base@1@@Z	std::_Fac_node::_Fac_node(std::_Fac_node *,std::_Facet_base *)
name	140001080	nullsub_4382
name	1400015C0	?Swap@?$List@UListEntry@details@Concurrency@@VNoCount@CollectionTypes@23@@details@Concurrency@@QEAAXPEAV123@@Z	Concurrency::details::List<Concurrency::details::ListEntry,Concurrency::details::CollectionTypes::NoCount>::Swap(Concurrency::details::List<Concurrency::details::ListEntry,Concurrency::details::CollectionTypes::NoCount> *)
name	1400015D0	?Swap@?$List@UListEntry@details@Concurrency@@VNoCount@CollectionTypes@23@@details@Concurrency@@QEAAXPEAV123@@Z_0	Concurrency::details::List<Concurrency::details::ListEntry,Concurrency::details::CollectionTypes::NoCount>::Swap(Concurrency::details::List<Concurrency::details::ListEntry,Concurrency::details::CollectionTypes::NoCount> *)
name	140002A70	unknown_libname_1
name	146737000	ExceptionDir
name	146A8C000	TlsStart
name	146A8F570	TlsEnd
"[..],
        );
        let result =
            OffsetList::parse_generic_offsets(&mut buffer, 0x140000000, OffsetList::NAME_PATTERN)?
                .iter()
                .map(|x| x.0)
                .collect::<Vec<_>>();
        assert_eq!(
            result,
            [
                0x0001000, 0x0001080, 0x00015C0, 0x00015D0, 0x0002A70, 0x6737000, 0x6A8C000,
                0x6A8F570,
            ]
        );
        Ok(())
    }
}
