use crate::common::{
    Offset,
    Version,
};
use anyhow::Context as _;
use regex_lite::Regex;
use std::{
    fs::File,
    io::{
        BufRead,
        BufReader,
    },
    num::ParseIntError,
    path::Path,
};
use walkdir::WalkDir;

pub struct Diff {
    pub left: Offset,
    pub right: Offset,
}

impl TryFrom<(&str, &str)> for Diff {
    type Error = ParseIntError;

    fn try_from(value: (&str, &str)) -> Result<Self, Self::Error> {
        Ok(Self {
            left: Offset(u32::from_str_radix(value.0, 16)?),
            right: Offset(u32::from_str_radix(value.1, 16)?),
        })
    }
}

pub struct DiffList {
    diffs: Vec<Diff>,
    pub left: Version,
    pub right: Version,
}

impl DiffList {
    fn parse_diffs<R: BufRead>(file: &mut R) -> anyhow::Result<Vec<Diff>> {
        let mut buffer = String::new();
        macro_rules! read_line {
            () => {{
                buffer.clear();
                file.read_line(&mut buffer)
                    .context("error while reading from diff file")
            }};
        }

        loop {
            match read_line!() {
                Ok(0) => anyhow::bail!("reached end of file before finding end of diff report"),
                Ok(_) => {
                    if buffer.starts_with("Overall success:") {
                        read_line!()?;
                        if buffer.trim().is_empty() {
                            break;
                        } else {
                            anyhow::bail!("expected empty line to follow diff report: {buffer}");
                        }
                    }
                }
                Err(err) => return Err(err),
            };
        }

        let diff_pattern = Regex::new(r"0x14([\dA-Fa-f]+)\t0x14([\dA-Fa-f]+)")
            .context("failed to build diff pattern")?;
        let mut diffs = Vec::new();
        loop {
            break match read_line!() {
                Ok(0) => Ok(diffs),
                Ok(_) if buffer.trim().is_empty() => Ok(diffs),
                Ok(_) => {
                    let captures = diff_pattern
                        .captures(&buffer)
                        .context("failed to match diff pattern")?;
                    let diff = (&captures[1], &captures[2])
                        .try_into()
                        .with_context(|| format!("failed to construct diff from line: {buffer}"))?;
                    diffs.push(diff);
                    continue;
                }
                Err(err) => Err(err),
            };
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Diff> {
        self.diffs.iter()
    }
}

pub struct DiffLists {
    lists: Vec<DiffList>,
}

impl DiffLists {
    pub fn parse_all(root_dir: &Path) -> anyhow::Result<Self> {
        println!("parsing diffs...");

        let pattern = Regex::new(r"(\d+)\.(\d+)\.(\d+)_(\d+)\.(\d+)\.(\d+)\.txt")
            .context("failed to build file name pattern")?;
        let mut lists = Vec::new();

        for dir_entry in WalkDir::new(root_dir) {
            let dir_entry = dir_entry.with_context(|| {
                format!("error while locating diff files in directory: {root_dir:?}")
            })?;
            let path = dir_entry.path();
            let metadata = dir_entry
                .metadata()
                .with_context(|| format!("failed to parse metadata from file: {path:?}"))?;
            if metadata.is_file() {
                if let Some(file_name) = path.file_name().and_then(|x| x.to_str()) {
                    if let Some(captures) = pattern.captures(file_name) {
                        let parse_version = |i1, i2, i3| {
                            Version::try_from((&captures[i1], &captures[i2], &captures[i3]))
                                .with_context(|| {
                                    format!("failed to parse version from file name: {path:?}")
                                })
                        };
                        let left = parse_version(1, 2, 3)?;
                        let right = parse_version(4, 5, 6)?;
                        if left == right {
                            anyhow::bail!(
                                "found a diff file that maps from one version to itself: {path:?}"
                            );
                        }
                        let diffs = {
                            let file = File::open(path)
                                .with_context(|| format!("failed to open file: {path:?}"))?;
                            let mut file = BufReader::new(file);
                            DiffList::parse_diffs(&mut file)
                                .with_context(|| format!("error while parsing file: {path:?}"))
                        }?;
                        lists.push(DiffList { diffs, left, right });
                    }
                }
            }
        }

        Ok(Self { lists })
    }

    pub fn iter(&self) -> impl Iterator<Item = &DiffList> {
        self.lists.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::DiffList;
    use std::io::BufReader;

    #[test]
    fn test_diffs() -> anyhow::Result<()> {
        let mut buffer = BufReader::new(
            &br"Previous version had 811238 total offsets that needed matching from.
Next version had 891318 total offsets that needed matching to.
Matched 164485 offsets from one version to another (20.276% / 18.454%) over 187328 passes.
The amount of matches that were perfect was 157617 (95.825%).
Previous version had 646753 (79.724%) offsets that could not be matched to new version.
Next version had 726833 (81.546%) offsets that could not be matched to old version.
The amount of objects that have the exact same address in previous and next version is 0 (0%).
Matches by segment in previous version:
0x1000 .text: 6748 (2.15%)
0x2C0C000 .interpr: 3 (60%)
0x2C17000 .idata: 13 (1.879%)
0x2C18670 .rdata: 127560 (33.32%)
0x36CB000 .data: 30161 (26.5%)
Converted 12417 locations to 12240 functions in previous version.
Overall success: 18.454%

0x1436C69FE	0x142C6201E
0x1436C70A4	0x142C62630
0x1436CAE1D	0x142C6065D
0x1430C7E4C	0x14272DE5C
0x142E626D8	0x1424D0528
"[..],
        );
        let result = DiffList::parse_diffs(&mut buffer)?
            .iter()
            .map(|x| (x.left.0, x.right.0))
            .collect::<Vec<_>>();
        assert_eq!(
            result,
            [
                (0x36C69FE, 0x2C6201E),
                (0x36C70A4, 0x2C62630),
                (0x36CAE1D, 0x2C6065D),
                (0x30C7E4C, 0x272DE5C),
                (0x2E626D8, 0x24D0528),
            ]
        );
        Ok(())
    }
}
