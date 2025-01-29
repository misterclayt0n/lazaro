use anyhow::{anyhow, Context, Result};
use chrono::Local;
use serde_json;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::{models, utils};

const PROGRAMS_DIR: &str = "programs";
const SESSIONS_DIR: &str = "sessions";

pub fn ensure_dirs() -> Result<()> {
    for dir in [PROGRAMS_DIR, SESSIONS_DIR] {
        if !Path::new(dir).exists() {
            fs::create_dir(dir).with_context(|| format!("Failed to create directory: {}", dir))?;
        }
    }
    Ok(())
}

fn programs_dir() -> PathBuf {
    Path::new(PROGRAMS_DIR).to_path_buf()
}

fn sessions_dir() -> PathBuf {
    Path::new(SESSIONS_DIR).to_path_buf()
}

pub fn start_session(program_name: &str) -> Result<()> {
    let program = load_program(program_name)?;

    let session = models::TrainingSession {
        id: Uuid::new_v4().to_string(),
        program: program.name,
        start_time: Local::now(),
        end_time: None,
        exercises: program
            .exercises
            .iter()
            .map(|e| models::SessionExercise {
                name: e.name.clone(),
                sets: Vec::new(),
                last_session_sets: Vec::new(),
                pr: None,
            })
            .collect(),
    };

    save_session(&session)?;
    println!("✅ Started session {} ({})", session.id, session.program);
    Ok(())
}

fn load_program(program_name: &str) -> Result<models::Program> {
    let path = programs_dir().join(format!("{}.toml", program_name));
    let content = fs::read_to_string(&path)
        .with_context(|| format!("Program '{}' not found", program_name))?;

    toml::from_str(&content).with_context(|| format!("Invalid program file: {}", path.display()))
}

fn save_session(session: &models::TrainingSession) -> Result<()> {
    let path = sessions_dir().join(format!("{}.json", session.id));
    let content = serde_json::to_string_pretty(session)?;
    fs::write(&path, content)
        .with_context(|| format!("Failed to save session to {}", path.display()))
}

pub fn show_session(session_id: Option<&str>) -> Result<()> {
    let session = load_session(session_id)?;

    println!("🏋️ Training Session: {}", session.program);
    println!("⏱ Started: {}", session.start_time.format("%Y-%m-%d %H:%M"));

    if let Some(end_time) = session.end_time {
        println!("🏁 Finished: {}", end_time.format("%Y-%m-%d %H:%M"));
    }

    for (ex_idx, exercise) in session.exercises.iter().enumerate() {
        println!("\n{}. {}", ex_idx + 1, exercise.name);

        for (set_idx, set) in exercise.sets.iter().enumerate() {
            print!("  Set {}: ", set_idx + 1);

            if let Some(prev_set) = exercise.last_session_sets.get(set_idx) {
                print!(
                    "(Previous: {}kgx{} → {:.1}RM) ",
                    prev_set.weight, prev_set.reps, prev_set.estimated_1rm
                );
            }

            match set.weight {
                Some(w) => print!("{}kg x {}", w, set.reps),
                None => print!("Bodyweight x {}", set.reps),
            }

            if let Some(r) = set.rpe {
                print!(" @ RPE {}", r);
            }

            if let Some(n) = &set.notes {
                print!(" - {}", n);
            }

            println!();
        }

        if let Some(pr) = &exercise.pr {
            println!(
                "  PR: {}kg x {} ({:.1}RM) on {}",
                pr.weight, pr.reps, pr.estimated_1rm, pr.date
            );
        }
    }
    Ok(())
}

pub fn edit_set(
    exercise_idx: usize,
    set_idx: usize,
    weight: Option<f32>,
    reps: Option<u32>,
    rpe: Option<f32>,
    notes: Option<String>,
) -> Result<()> {
    let mut session = load_current_session()?;

    let ex_index = exercise_idx
        .checked_sub(1)
        .ok_or_else(|| anyhow!("Exercise index must be ≥ 1"))?;

    let set_index = set_idx
        .checked_sub(1)
        .ok_or_else(|| anyhow!("Set index must be ≥ 1"))?;

    let exercise = session.exercises.get_mut(ex_index).unwrap();

    // Ensure enough sets exist
    while exercise.sets.len() <= set_index {
        exercise.sets.push(models::ExerciseSet {
            timestamp: Local::now(),
            weight: None,
            reps: 0,
            rpe: None,
            notes: None,
        });
    }

    let set = &mut exercise.sets[set_index];

    if let Some(w) = weight {
        set.weight = Some(w);
    }
    if let Some(r) = reps {
        set.reps = r;
    }
    if let Some(r) = rpe {
        set.rpe = Some(r);
    }
    if let Some(n) = notes {
        set.notes = Some(n);
    }

    save_session(&session)?;
    println!("✅ Updated set {}-{}", exercise_idx, set_idx);
    Ok(())
}

pub fn finish_session() -> Result<()> {
    let mut session = load_current_session()?;
    session.end_time = Some(Local::now());
    save_session(&session)?;

    let duration = session.end_time.unwrap() - session.start_time;
    println!("🏁 Finished session in {} minutes", duration.num_minutes());
    Ok(())
}

pub fn show_timer() -> Result<()> {
    let session = load_current_session()?;
    let duration = Local::now() - session.start_time;
    println!("⏱️  Elapsed time: {}", utils::format_duration(duration));
    Ok(())
}

fn load_session(session_id: Option<&str>) -> Result<models::TrainingSession> {
    let path = match session_id {
        Some(id) => sessions_dir().join(format!("{}.json", id)),
        None => {
            let mut sessions = get_sessions()?;
            sessions.pop().ok_or(anyhow!("No active sessions found"))?
        }
    };

    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read session file: {}", path.display()))?;

    serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse session file: {}", path.display()))
}

fn get_sessions() -> Result<Vec<PathBuf>> {
    let dir = sessions_dir();
    let mut entries = fs::read_dir(&dir)
        .with_context(|| format!("Failed to read session directory: {}", dir.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().map_or(false, |ext| ext == "json"))
        .collect::<Vec<_>>();

    // Sort by modified time (newest first)
    entries.sort_by(|a, b| {
        b.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            .cmp(
                &a.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            )
    });

    Ok(entries)
}

fn load_current_session() -> Result<models::TrainingSession> {
    load_session(None)
}

// Implement other storage functions (load_session, edit_set, etc.) similarly...
