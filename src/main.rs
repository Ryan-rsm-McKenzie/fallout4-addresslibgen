#![warn(clippy::pedantic)]
#![allow(clippy::redundant_else)]

mod addrlib;
mod common;
mod diffs;
mod graph;
mod offsets;

use addrlib::AddressBins;
use anyhow::Context as _;
use clap::Parser;
use diffs::DiffLists;
use offsets::OffsetLists;
use std::path::PathBuf;

fn input_directory_validator(input_directory: &str) -> Result<PathBuf, &'static str> {
    let input_directory: PathBuf = input_directory.into();
    if !input_directory.exists() {
        Err("input directory does not exist")
    } else if !input_directory.is_dir() {
        Err("input directory is not a directory")
    } else {
        Ok(input_directory)
    }
}

#[derive(Parser)]
struct Cli {
    #[arg(value_parser = input_directory_validator)]
    input_directory: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let (offset_lists, mut graph) =
        OffsetLists::parse_all(&cli.input_directory).context("failed to parse all offsets")?;

    {
        let diff_lists =
            DiffLists::parse_all(&cli.input_directory).context("failed to parse all diffs")?;
        graph
            .add_edges(&offset_lists, &diff_lists)
            .context("failed to add edges from diff lists")?;
    }

    let address_bins =
        AddressBins::parse_all(&cli.input_directory).context("failed to parse all address bins")?;
    graph
        .seed_ids(&offset_lists, &address_bins)
        .context("failed to seed ids from address bins")?;
    let largest_unused_id = address_bins.largest_unused_id();

    graph
        .assign_all_ids(largest_unused_id)
        .context("failed to assign ids to all offsets")?;
    addrlib::write_bins(&cli.input_directory, &graph, &offset_lists, &address_bins)
        .context("failed to write address bins")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::offsets::OffsetLists;
    use std::path::Path;

    #[test]
    fn it_works() -> anyhow::Result<()> {
        let (offset_lists, _) = OffsetLists::parse_all(Path::new(
            r"E:\Repos\fallout4-addresslibgen\target\artifacts",
        ))?;
        for (version, _) in offset_lists.iter() {
            println!("{version}");
        }
        Ok(())
    }
}
