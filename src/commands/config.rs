use std::path::PathBuf;

use crate::{cli::ConfigCmd, types::Config};
use anyhow::Result;
use colored::Colorize;

pub async fn handle(cmd: ConfigCmd, mut cfg: Config, config_path: PathBuf) -> Result<()> {
    match cmd {
        ConfigCmd::List => {
            if cfg.map.is_empty() {
                println!("{} {}", "warning:".yellow().bold(), "no config set".dimmed());
            } else {
                println!("{}", "Config:".cyan().bold());
                for (k, v) in &cfg.map {
                    println!("  {} = {}", k.green(), v);
                }
            }
        } 

        ConfigCmd::Get { key } => {
            match cfg.map.get(&key) {
                Some(val) => println!("{}", val),
                None      => println!("{} key `{}` not found", "warning:".yellow().bold(), key),
            }
        }

        ConfigCmd::Set { key, val } => {
            if !cfg.validate_key(&key) {
                println!("{} Invalid config key `{}`", "error:".red().bold(), key);
                return Ok(());
            }
            
            cfg.map.insert(key.clone(), val.clone());
            cfg.save(&config_path)?;
            println!("{} set `{}` = `{}`", "info:".blue().bold(), key.green(), val);
        }

        ConfigCmd::Unset { key } => {
            if cfg.map.remove(&key).is_some() {
                cfg.save(&config_path)?;
                println!("{} removed `{}`", "info:".blue().bold(), key.green());
            } else {
                println!("{} key `{}` not found", "warning:".yellow().bold(), key);
            }
        }
    }
    
    Ok(())
}
