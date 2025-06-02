use clap::{Args, Parser, Subcommand};

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

    /// Show training sessions in a calendar view
    #[command(visible_alias = "cal")]
    Calendar {
        /// Year to show (defaults to current year)
        #[arg(short, long)]
        year: Option<i32>,

        /// Month to show (1-12, defaults to current month)
        #[arg(short, long)]
        month: Option<u32>,
    },

    /// Show global progression and training status
    Status {
        /// Show progression for a specific muscle group
        #[arg(short, long)]
        muscle: Option<String>,

        /// Time period in weeks (defaults to 12)
        #[arg(short, long, default_value = "12")]
        weeks: u32,

        /// Show graph instead of summary
        #[arg(short, long)]
        graph: bool,
    },

    /// Db operations
    #[command(subcommand)]
    Db(DbCmd),
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

    /// Add an exercise to the current session
    AddEx { exercise: String, sets: i32 },

    #[command(visible_alias = "n")]
    #[command(override_usage = "session note <EX_IDX> <NOTE_STRING>")]
    Note {
        /// 1-based index of the exercise (same order shown in `session show`)
        #[arg(value_name = "EX_IDX")]
        exercise: usize,

        /// Free-form text
        #[arg(value_name = "NOTE_STRING")]
        note: String,
    },

    /// Show details of a completed session from a specific date
    Log {
        /// Date in DD-MM-YYYY format
        #[arg(short, long)]
        date: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ExerciseCmd {
    /// Add a new exercise
    #[command(visible_alias = "a")]
    Add {
        /// Exercise name
        name: String,

        /// Primary muscle group
        #[arg(short, long)]
        muscle: String,

        /// Exercise description
        #[arg(short, long)]
        desc: Option<String>,
    },

    /// Import exercises from a TOML file
    #[command(visible_alias = "i")]
    Import {
        /// Path to TOML file
        file: String,
    },

    /// List all exercises
    #[command(visible_alias = "l")]
    List {
        /// Filter by muscle group
        #[arg(short, long)]
        muscle: Option<String>,
    },

    /// Delete an exercise
    #[command(visible_alias = "d")]
    Delete {
        /// Exercise index or name
        exercise: String,
    },

    /// Show detailed exercise information
    #[command(visible_alias = "s",     // keep the short alias
              trailing_var_arg = true, // ← take the rest of the command-line
              verbatim_doc_comment)] // keep the doc-comment exactly
    Show {
        /// Exercise index or name
        exercise: Vec<String>,

        /// Show progression graph
        #[arg(short, long)]
        graph: bool,
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
        program: String,
    },

    /// Delete a program
    #[command(visible_alias = "d")]
    Delete {
        /// Program index (from `p list`) or exact name
        program: String,
    },
}

#[derive(Args)]
pub struct StartArgs {
    pub program: String,
    pub block: String,
    pub week: Option<i32>,
}

#[derive(Subcommand)]
pub enum DbCmd {
    /// Export database to a TOML file
    Export {
        /// Output file path (defaults to dump.toml)
        #[arg(short, long)]
        file: Option<String>,
    },

    /// Import database from a TOML file
    Import {
        /// Input TOML file path
        file: String,
    },

    /// Migrate an *old* lazaro.db into the current one
    Migrate {
        /// path to the old lazaro.db (source)
        old_db: String,
    },
}
