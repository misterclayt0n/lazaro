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

    /// End the current session
    // #[command(visible_alias = "e")]
    End,

    /// Edit a set in the current session - Usage: session edit EXERCISE WEIGHT REPS
    #[command(visible_alias = "e")]
    #[command(override_usage = "session edit <EXERCISE> <WEIGHT> <REPS>")]
    Edit {
        /// Exercise index
        #[arg(value_name = "EXERCISE")]
        exercise: usize,

        /// Weight in kg (use "bw" for bodyweight exercises)
        #[arg(value_name = "WEIGHT")]
        weight: String,

        /// Number of reps
        #[arg(value_name = "REPS")]
        reps: i32,

        /// Specific set index to edit (defaults to next unlogged set)
        #[arg(long, short = 's')]
        set: Option<usize>,

        /// Add a new set even if all sets are already logged
        #[arg(long, short = 'n')]
        new: bool,
    },
    
    /// Swap an exercise in the current session with another - Usage: session swap EXERCISE NEW_EXERCISE
    #[command(visible_alias = "sw")]
    Swap {
        /// Exercise index in the current session to replace
        #[arg(value_name = "EXERCISE")]
        exercise: usize,
        
        /// New exercise index or name to swap in
        #[arg(value_name = "NEW_EXERCISE")]
        new_exercise: String,
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
