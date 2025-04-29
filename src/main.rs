use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};
use db::open;

mod cli;
mod db;
mod commands;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let db_path = "./lazarus.db";
    let pool = open(&db_path).await?;

    match cli.cmd {
        Commands::Session(cmd) => commands::session::handle(cmd, &pool).await?
    }

    Ok(())
}
