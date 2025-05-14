use std::collections::HashMap;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Commands};
use db::open;
use types::{Config, OutputFmt};

mod cli;
mod db;
mod commands;
mod types;

#[tokio::main]
async fn main() -> Result<()> {
    let config_path = dirs::config_dir().context("no config dir")?.join("lazarus").join("config");
    let cfg = Config::load(&config_path)?;
    let json_default = cfg.json_default();
    let alias_map = cfg.aliases();

    let new_args = rewrite_args(&alias_map);
    
    let cli = Cli::parse_from(new_args);

    let fmt = OutputFmt {
        json: cli.json || json_default,
    };
    
    let db_path = "./lazarus.db";
    assert!(!db_path.is_empty(), "database path must not be empty");
    
    let pool = open(&db_path).await?;

    match cli.cmd {
        Commands::Session(cmd) => commands::session::handle(cmd, &pool).await?,
        Commands::Exercise(cmd) => commands::exercise::handle(cmd, &pool, fmt).await?,
        Commands::Config(cmd) => commands::config::handle(cmd, cfg, config_path).await?,
        Commands::Program(cmd) => commands::program::handle(cmd, &pool, fmt).await?,
        Commands::Calendar { year, month } => commands::calendar::handle(&pool, year, month).await?,
        Commands::Db(cmd) => commands::db::handle(cmd, &pool).await?
    }

    Ok(())
}

fn rewrite_args(alias_map: &HashMap<String, Vec<String>>) -> Vec<String> {
    let original: Vec<String> = std::env::args().collect();
    let mut rewritten = Vec::with_capacity(original.len() * 2);

    rewritten.push(original[0].clone());

    let mut cur_path: Vec<String> = Vec::new();

    for arg in original.into_iter().skip(1) {
        if let Some(canon) = alias_map.get(&arg) {
            let missing = if canon.starts_with(&cur_path) {
                &canon[cur_path.len()..]
            } else {
                &canon[..]
            };
            rewritten.extend(missing.iter().cloned());
            cur_path = canon.clone();
        } else {
            rewritten.push(arg.clone());
            cur_path.push(arg);
        }
    }

    rewritten
}
