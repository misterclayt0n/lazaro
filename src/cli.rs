use anyhow::Result;
use clap::{Parser, Subcommand};
use crate::storage;

#[derive(Parser)]
#[command(name = "lazaro")]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Display current training session
    ShowSession {
        #[arg(short, long)]
        session_id: Option<String>,
    },

    /// Edit a specific set in an exercise
    EditSet {
        #[arg(help = "Exercise index (starting from 1)")]
        exercise_idx: usize,

        #[arg(help = "Set index (starting from 1)")]
        set_idx: usize,

        #[arg(short, long)]
        weight: Option<f32>,

        #[arg(short = 'n', long)]
        reps: Option<u32>,

        #[arg(short, long)]
        rpe: Option<f32>,

        #[arg(short = 't', long)]
        notes: Option<String>,
    },

    /// Start a new training session
    StartSession {
        program_name: String,
    },

    /// Finish current session
    FinishSession,

    /// Show elapsed time
    Timer,
}

impl Cli {
    pub fn execute(self) -> Result<()> {
        match self.command {
            Commands::ShowSession { session_id } => storage::show_session(session_id.as_deref()),
            Commands::EditSet { exercise_idx, set_idx, weight, reps, rpe, notes } => {
                storage::edit_set(exercise_idx, set_idx, weight, reps, rpe, notes)
            }
            Commands::StartSession { program_name } => storage::start_session(&program_name),
            Commands::FinishSession => storage::finish_session(),
            Commands::Timer => storage::show_timer(),
        }
    }
}
