use serde::{Deserialize, Serialize};
use chrono::{DateTime, Local};

/// Represents a single training session with timing and exercise data.
/// Includes both active sessions (end_time = None) and completed sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingSession {
    pub id: String,
    pub program: String,
    pub start_time: DateTime<Local>,
    pub end_time: Option<DateTime<Local>>,
    pub exercises: Vec<SessionExercise>,
}

/// Exercise-specific data within a session.
/// Contains both current performance and historical context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionExercise {
    pub name: String,
    pub sets: Vec<ExerciseSet>,
    pub last_session_sets: Vec<SetReference>,
    pub pr: Option<PersonalRecord>,
}

/// Individual set record with optional performance metrics.
/// Timestamped for detailed activity tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExerciseSet {
    pub timestamp: DateTime<Local>,
    pub weight: Option<f32>,
    pub reps: u32,
    pub rpe: Option<f32>,
    pub notes: Option<String>,
}

/// Reference to previous performance for comparison.
/// Used to show "last session's performance" next to current sets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetReference {
    pub weight: f32,
    pub reps: u32,
    pub estimated_1rm: f32,
}

/// All-time personal record for an exercise.
/// Tracked using calculated 1RM from different formulas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalRecord {
    pub weight: f32,
    pub reps: u32,
    pub estimated_1rm: f32,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Program {
    pub name: String,
    pub exercises: Vec<ProgramExercise>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramExercise {
    pub name: String,
    pub sets: u32,
    pub reps: String,
}

#[derive(Debug, Clone, Copy)]
pub enum OneRMFormula {
    Epley,
    Brzycki,
    Lombardi,
    OConner,
}
