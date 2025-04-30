use once_cell::sync::Lazy;
use std::{collections::HashSet, fmt::Display};
use strsim::jaro_winkler;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use sqlx::prelude::Type;

#[derive(Clone, Debug, ValueEnum, Serialize, Deserialize, Type)]
#[sqlx(type_name = "TEXT")]
#[serde(rename_all = "kebab-case")]
pub enum Muscle {
    Biceps,
    Triceps,
    Forearms,
    Chest,
    Shoulders,
    Back,
    Quads,
    Hamstrings,
    Glutes,
    Calves,
    Abs,
}

impl Display for Muscle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Biceps => "biceps",
            Self::Triceps => "triceps",
            Self::Forearms => "forearms",
            Self::Chest => "chest",
            Self::Shoulders => "shoulders",
            Self::Back => "back",
            Self::Quads => "quads",
            Self::Hamstrings => "hamstrings",
            Self::Glutes => "glutes",
            Self::Calves => "calves",
            Self::Abs => "abs",
        };

        write!(f, "{}", s)
    }
}

pub static ALLOWED_MUSCLES: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    HashSet::from([
        "biceps",
        "triceps",
        "forearms",
        "chest",
        "shoulders",
        "back",
        "quads",
        "hamstrings",
        "glutes",
        "calves",
        "abs",
    ])
});

/// Returns the canonical lowercase muscle name or `None` if not allowed.
pub fn cannonical_muscle<S: AsRef<str>>(m: S) -> Option<String> {
    let raw = m.as_ref();
    assert!(raw.chars().all(|c| !c.is_control()), "received control chars in muscle name: {raw:?}");
    
    let m = raw.to_ascii_lowercase();
    if ALLOWED_MUSCLES.contains(m.as_str()) {
        Some(m)
    } else {
        None
    }
}

/// Return the closest allowed muscle for `input`
/// if similarity ≥ 0.85 *and* clearly better than the runner-up.
/// Otherwise return `None` (no suggestion shown).
pub fn best_muscle_suggestions(input: &str) -> Option<&'static str> {
    assert!(!ALLOWED_MUSCLES.is_empty(), "ALLOWED_MUSCLES must contain at least one entry");

    let inp = input.to_ascii_lowercase();
    assert!(!inp.trim().is_empty(), "best_muscle_suggestions called with empty input"); // Sanity check.
    
    // Collect (muscle, score) pairs.
    let mut scores: Vec<(&'static str, f64)> = ALLOWED_MUSCLES
        .iter()
        .copied()
        .map(|m| (m, jaro_winkler(input, m)))
        .collect();

    // Highest score first.
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let (best_muscle, best_score) = scores[0];
    let second_score = scores.get(1).map(|(_, s)| *s).unwrap_or(0.0);

    // Tune these two constants to taste.
    const MIN_SCORE: f64 = 0.80;
    const GAP: f64 = 0.02;

    if best_score >= MIN_SCORE && best_score - second_score >= GAP {
        Some(best_muscle)
    } else {
        None
    }
}

#[derive(Deserialize)]
pub struct ExerciseDef {
    pub name: String,
    pub description: Option<String>,
    pub primary_muscle: String,
}

#[derive(Deserialize)]
pub struct ExerciseImport {
    pub exercise: Vec<ExerciseDef>,
}
