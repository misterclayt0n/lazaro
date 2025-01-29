use std::path::PathBuf;

use chrono::{DateTime, Local};
use clap::{Parser, Subcommand};

/// Represents a single training session with timing and exercise data.
/// Includes both active sessions (end_time = None) and completed sessions.
struct TrainingSession {
    id: String,
    program: String,
    start_time: Option<DateTime<Local>>,
    end_time: Option<DateTime<Local>>,
    exercises: Vec<SessionExercise>,
}

/// Reference to previous performance for comparison.
/// Used to show "last session's performance" next to current sets.
struct SetReference {
    weight: f32,
    reps: u32,
    estimated_1rm: f32,
}

/// All-time personal record for an exercise.
/// Tracked using calculated 1RM from different formulas.
struct PersonalRecord {
    weight: f32,
    reps: u32,
    estimated_1rm: f32,
    date: String,
}

/// Individual set record with optional performance metrics.
/// Timestamped for detailed activity tracking.
struct ExerciseSet {
    timestamp: DateTime<Local>,
    weight: Option<f32>, // None for bodyweight if wanted.
    reps: u32,
    rpe: Option<f32>,
    notes: Option<String>, // Free-form user commends.
}

/// Exercise-specific data within a session.
/// Contains both current performance and historical context.
struct SessionExercise {
    name: String,
    sets: Vec<ExerciseSet>,
    last_session_sets: Vec<SetReference>, // Prev workout's sets
    pr: Option<PersonalRecord>,           // All time best
}

fn session_dir() -> PathBuf {
    let mut path = dirs::home_dir().unwrap();
    path.push("./lazaro/sessions");
    return path;
}

/// Supported 1RM calculation formulas.
/// Each has different accuracy characteristics.
enum OneRMFormula {
    Epley,    // Good for moderate rep ranges (3-10)
    Brzycki,  // Popular for powerlifting
    Lombardi, // Better for high-rep sets
    OConner,  // Simple linear approximation
}

fn calculate_1rm(weight: f32, reps: u32, formula: &OneRMFormula) -> f32 {
    match formula {
        OneRMFormula::Epley => weight * (1.0 + reps as f32 / 30.0),
        OneRMFormula::Brzycki => weight / (1.0278 - 0.0278 * reps as f32),
        OneRMFormula::Lombardi => weight * (reps as f32).powf(0.10),
        OneRMFormula::OConner => weight * (1.0 + 0.025 * reps as f32),
    }
}

#[derive(Parser)]
#[command(name = "lazaro")]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Display current training session.
    ShowSession {
        #[arg(short, long)]
        session_id: Option<String>,
    },

    /// Edit a specific set in an exercise.
    EditSet {
        #[arg(help = "Exercise index (starting from 1)")]
        exercise_idx: usize,

        #[arg(help = "Set index (starting from 1)")]
        set_idx: usize,

        #[arg(short, long)]
        weight: Option<f32>,

        #[arg(short, long)]
        reps: Option<u32>,

        #[arg(short, long)]
        rpe: Option<f32>,

        #[arg(short, long)]
        notes: Option<String>,
    },

    /// Start a new training session.
    StartSession {
        program_name: String,
    },

    /// Finish current session.
    FinishSession,

    /// Show elapsed time.
    Timer,
}

fn main() {
    println!("hello world");
}
