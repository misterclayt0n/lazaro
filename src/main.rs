mod models;
mod cli;
mod storage;
mod utils;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    storage::ensure_dirs()?;
    cli::Cli::parse().execute()
}
