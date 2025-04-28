use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct CLI {
    /// Lazaro
    name: Option<String>,

    /// Sets a custom config file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    // Turn on debugging info
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Test {
        #[arg(short, long)]
        list: bool,
    },
}

fn main() {
    let cli = CLI::parse();

    if let Some(name) = cli.name.as_deref() {
        println!("value of name: {name}")
    }

    if let Some(config_path) = cli.config.as_deref() {
        println!("Value for config: {}", config_path.display())
    }

    match cli.debug {
        0 => println!("Debug mode off"),
        1 => println!("Debug kinda on"),
        2 => println!("Debug on"),
        3 => println!("Debug very on"),
        _ => println!("Nope"),
    }

    match &cli.command {
        Some(Commands::Test { list }) => {
            if *list {
                println!("printing testing list...")
            } else {
                println!("not printing shit...")
            }
        }
        None => {}
    }

    println!("Hello, world!");
}
