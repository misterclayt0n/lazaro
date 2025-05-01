use crate::{cli::ConfigCmd, types::Config};
use anyhow::{Context, Result};
use colored::Colorize;

pub async fn handle(cmd: ConfigCmd) -> Result<()> {
    let config_path = dirs::config_dir().map(|d| d.join("lazarus").join("config")).context("Could not determine config directory")?;
    let mut cfg = Config::load(&config_path)?;

    match cmd {
        ConfigCmd::List => {
            if cfg.map.is_empty() {
                println!("{}", "(no config set)".dimmed());
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
