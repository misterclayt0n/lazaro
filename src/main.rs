use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};
use db::open;

mod cli;
mod db;
mod commands;
mod types;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let db_path = "./lazarus.db";
    assert!(!db_path.is_empty(), "database path must not be empty");
    
    let pool = open(&db_path).await?;

    match cli.cmd {
        Commands::Session(cmd) => commands::session::handle(cmd, &pool).await?,
        Commands::Exercise(cmd) => commands::exercise::handle(cmd, &pool).await?,
        Commands::Config(cmd) => commands::config::handle(cmd).await?
    }

    Ok(())
}
