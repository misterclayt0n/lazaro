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
    #[command(subcommand, visible_alias = "s")]
    Session(SessionCmd),

    /// Exercise management
    #[command(subcommand, visible_alias = "ex")]
    Exercise(ExerciseCmd),

    /// View or edit lazarus config
    #[command(subcommand)]
    Config(ConfigCmd),

    /// Program management
    #[command(subcommand, visible_alias = "p")]
    Program(ProgramCmd),
}

//
// Commands
//

#[derive(Subcommand)]
pub enum SessionCmd {
    /// Start a session
    #[command(visible_alias = "s")]
    Start(StartArgs),

    /// Cancel the current session
    #[command(visible_alias = "c")]
    Cancel,

    /// Show current session details
    #[command(visible_alias = "i")]
    Show,

    /// Edit a set in the current session
    #[command(visible_alias = "e")]
    Edit {
        /// Exercise index (from session show)
        exercise: usize,

        /// Number of reps performed
        reps: i32,

        /// Weight in kg (ignored if --bw is set)
        #[arg(required_unless_present = "bw")]
        weight: Option<f32>,

        /// Mark as bodyweight exercise
        #[arg(long)]
        bw: bool,

        /// Specific set index to edit (defaults to next unlogged set)
        #[arg(long, short = 's')]
        set: Option<usize>,
    },
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
    #[command(visible_alias = "i")]
    Import { file: String },

    /// List existing exercises, optionally filetering by muscle group
    #[command(visible_alias = "l")]
    List {
        #[arg(long, short = 'm')]
        muscle: Option<String>,

        /// Hide variants (shown by default)
        #[arg(long = "no-variants", short = 'n', action = clap::ArgAction::SetFalse, default_value_t = true)]
        variants: bool,
    },

    #[command(visible_alias = "v")]
    Variant {
        /// Either the exercise index (number) or its name
        exercise: String,

        /// If provided, adds this as new variant; if ommited, lists all variants
        variant: Option<String>,
    },

    /// Delete an exercise
    #[command(visible_alias = "d")]
    Delete {
        /// Exercise index (from `ex list`) or exact name
        exercise: String
    }
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
    Unset { key: String },
}

#[derive(Subcommand)]
pub enum ProgramCmd {
    /// Import one or more programs
    #[command(visible_alias = "i")]
    Import { files: Vec<String> },

    /// List programs 
    #[command(visible_alias = "l")]
    List,

    /// Show a single program in detail
    #[command(visible_alias = "s")]
    Show {
        /// Program index (from `p list`) or exact name
        program: String
    },

    /// Delete a program
    #[command(visible_alias = "d")]
    Delete {
        /// Program index (from `p list`) or exact name
        program: String
    }
}

#[derive(Args)]
pub struct StartArgs {
    pub program: String,
    pub block: String,
    pub week: Option<i32>,
}
