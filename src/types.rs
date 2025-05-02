use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    fs::{create_dir_all, read_to_string},
    path::Path,
};
use strsim::jaro_winkler;

use clap::{Command, CommandFactory, ValueEnum};
use serde::{Deserialize, Serialize};
use sqlx::prelude::Type;

use crate::cli::Cli;

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
    assert!(
        raw.chars().all(|c| !c.is_control()),
        "received control chars in muscle name: {raw:?}"
    );

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
    assert!(
        !ALLOWED_MUSCLES.is_empty(),
        "ALLOWED_MUSCLES must contain at least one entry"
    );

    let inp = input.to_ascii_lowercase();
    assert!(
        !inp.trim().is_empty(),
        "best_muscle_suggestions called with empty input"
    ); // Sanity check.

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

#[derive(Debug, Default)]
pub struct Config {
    pub map: HashMap<String, String>,
}

impl Config {
    /// Load from disk (returns empty if file not found).
    pub fn load(path: &Path) -> Result<Self> {
        let mut cfg = Config::default();
        if let Ok(s) = read_to_string(path) {
            for line in s.lines() {
                let line = line.trim();

                // Comments.
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }

                if let Some(eq) = line.find('=') {
                    let key = line[..eq].trim().to_string();
                    let val = line[eq + 1..].trim().to_string();
                    cfg.map.insert(key, val);
                }
            }
        }

        Ok(cfg)
    }

    /// Persists back to disk.
    pub fn save(&self, path: &Path) -> Result<()> {
        let mut out = String::new();
        for (k, v) in &self.map {
            out.push_str(&format!("{} = {}\n", k, v));
        }
        create_dir_all(path.parent().unwrap())?;
        std::fs::write(path, out).context("Failed to write config file")
    }

    /// Returns a map from alias - cannonical path segments.
    /// e.g. "st" -> ["session", "start"].
    pub fn aliases(&self) -> HashMap<String, Vec<String>> {
        let mut m = HashMap::new();
        for (k, v) in &self.map {
            if let Some(rest) = k.strip_prefix("aliases.") {
                // 'rest' is like "session" or "session.start".
                let canon: Vec<String> = rest.split('.').map(String::from).collect();
                m.insert(v.clone(), canon);
            }
        }

        return m;
    }

    /// Validate a key is of the form "aliases.<cmd>[.<subcmd>]" and exists in CLI.
    pub fn validate_key(&self, key: &str) -> bool {
        match key {
            "json" => true,
            _ if key.starts_with("aliases.") => {
                let rest = match key.strip_prefix("aliases.") {
                    Some(r) => r,
                    None => return false,
                };

                let parts: Vec<&str> = rest.split('.').collect();
                if parts.is_empty() {
                    return false;
                }

                let mut cmd: Command = Cli::command();

                for &seg in &parts {
                    if let Some(subcmd) = cmd.clone().get_subcommands().find(|sc| {
                        sc.get_name() == seg || sc.get_all_aliases().any(|alias| alias == seg)
                    }) {
                        cmd = subcmd.clone();
                    } else {
                        return false;
                    }
                }

                return true;
            }
            _ => false,
        }
    }

    pub fn json_default(&self) -> bool {
        matches!(self.map.get("json").map(|v| v.as_str()), Some("true" | "1"))
    }
}

/// How the user wants to see stuff.
#[derive(Clone, Copy)]
pub struct OutputFmt {
    pub json: bool,
}

/// Generic one-liner: if JSON is requested -> dump, else, run closure.
pub fn emit<T, F>(fmt: OutputFmt, value: &T, pretty: F)
where
    T: Serialize,
    F: FnOnce(),
{
    if fmt.json {
        println!(
            "{}",
            serde_json::to_string_pretty(value).expect("json serialize")
        );
    } else {
        pretty();
    }
}
