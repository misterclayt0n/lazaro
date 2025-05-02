use clap::{Args, Parser, Subcommand};

use crate::types::Muscle;

#[derive(Parser)]
#[command(name = "lazarus", version, about = "CLI training app")]
#[command(arg_required_else_help = true)]
pub struct Cli {
    /// Emit machine-readable JSON instead of colorful text.
    #[arg(global = true, long)]
    pub json: bool,
    
    #[command(subcommand)]
    pub cmd: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Session-scoped commands
    #[command(subcommand, alias = "s")]
    Session(SessionCmd),

    /// Exercise management
    #[command(subcommand, alias = "ex")]
    Exercise(ExerciseCmd),

    /// View or edit lazarus config
    #[command(subcommand)]
    Config(ConfigCmd),
}

//
// Commands
// 

#[derive(Subcommand)]
pub enum SessionCmd {
    /// Start a session
    #[command(alias = "s")]
    Start(StartArgs),
}

#[derive(Subcommand)]
pub enum ExerciseCmd {
    /// Add a new exercise
    Add {
        name: String,
        #[arg(value_enum)]
        muscle: Muscle, // Clap enforces catalogue.
        #[arg(long)]
        desc: Option<String>,
    },

    /// Bulk import from a TOML file
    #[command(alias = "i")]
    Import { file: String },

    /// List existing exercises, optionally filetering by muscle group
    #[command(alias = "l")]
    List {
        #[arg(long)]
        muscle: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Show all config keys
    List,

    /// Get the value of a key
    Get { key: String },

    /// Set or override a key
    Set { key: String, val: String },
    
    /// Remove a key
    Unset { key: String }
}

#[derive(Args)]
pub struct StartArgs {
    pub program: String,
    pub block: String,
    pub week: Option<i32>,
}
