use std::{collections::HashSet, fs::read_to_string};

use anyhow::{Context, Result};
use colored::Colorize;
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::{cli::ProgramCmd, types::OutputFmt};

#[derive(Debug, Deserialize)]
struct ProgramToml {
    name: String,
    description: Option<String>,

    weeks: Option<Vec<WeekToml>>,
    blocks: Option<Vec<BlockToml>>,
}

#[derive(Debug, Deserialize)]
struct WeekToml {
    week: u32,
    blocks: Vec<BlockToml>,
}

#[derive(Debug, Deserialize)]
struct BlockToml {
    name: String,
    description: Option<String>,
    exercises: Vec<BlockExerciseToml>,
}

#[derive(Debug, Deserialize)]
struct BlockExerciseToml {
    name: String,
    sets: u32,
    reps: Option<Vec<String>>,
    target_rpe: Option<Vec<f32>>,
    target_rm_percent: Option<Vec<f32>>,
    notes: Option<String>,
    program_1rm: Option<f32>,
    options: Option<Vec<String>>,
    technique: Option<String>,
    group: Option<u32>,
}

pub async fn handle(cmd: ProgramCmd, pool: &SqlitePool, _fmt: OutputFmt) -> Result<()> {
    match cmd {
        ProgramCmd::Import { files } => {
            if files.is_empty() {
                println!("{} No program file provided", "warning".yellow().bold())
            }

            for file in files {
                import_single_program(pool, &file).await?;
            }
        }

        ProgramCmd::List => {}
    }

    Ok(())
}

async fn import_single_program(pool: &SqlitePool, file: &str) -> Result<()> {
    let toml_str = read_to_string(file).with_context(|| format!("reading `{file}`"))?;
    let prog: ProgramToml =
        toml::from_str(&toml_str).with_context(|| format!("parsing `{file}`"))?;

    // Make sure every exercise mentioned exists in DB first.
    let mut all_ex_names = std::collections::HashSet::<&str>::new();
    if let Some(weeks) = &prog.weeks {
        for w in weeks {
            for b in &w.blocks {
                for ex in &b.exercises {
                    all_ex_names.insert(ex.name.as_str());
                }
            }
        }
    }
    if let Some(blocks) = &prog.blocks {
        for b in blocks {
            for ex in &b.exercises {
                all_ex_names.insert(ex.name.as_str());
            }
        }
    }

    if !all_ex_names.is_empty() {
        // Build the IN (?,?,?,...) clause first.
        let placeholders = std::iter::repeat("?")
            .take(all_ex_names.len())
            .collect::<Vec<_>>()
            .join(",");

        // Start the query.
        let query = &format!(
            "SELECT name FROM exercises WHERE name IN ({})",
            placeholders,
        );
        let mut q = sqlx::query_as::<_, (String,)>(query);

        // And bind every parameter one‑by‑one.
        for name in &all_ex_names {
            q = q.bind(name);
        }

        let rows: Vec<(String,)> = q.fetch_all(pool).await?;

        let present: std::collections::HashSet<_> = rows.into_iter().map(|(n,)| n).collect();

        let missing: Vec<_> = all_ex_names
            .into_iter()
            .filter(|n| !present.contains(*n))
            .collect();

        if !missing.is_empty() {
            println!(
                "{} cannot import program `{}` – these exercises are missing: {}",
                "warning:".yellow().bold(),
                prog.name,
                missing.join(", ")
            );
            return Ok(());
        }
    }

    // Transactional import.
    let mut tx = pool.begin().await?;

    // Try to insert the program row.
    let prog_id = uuid::Uuid::new_v4().to_string();
    let res = sqlx::query(
        r#"INSERT INTO programs (id,name,description,created_at)
           VALUES (?1,?2,?3,datetime('now'))"#,
    )
    .bind(&prog_id)
    .bind(&prog.name)
    .bind(prog.description.as_deref())
    .execute(&mut *tx)
    .await;

    if let Err(sqlx::Error::Database(db_err)) = &res {
        if db_err.code() == Some("2067".into()) {
            println!(
                "{} program `{}` already exists – skipping import",
                "warning:".yellow().bold(),
                prog.name
            );
            tx.rollback().await?;
            return Ok(());
        }
    }
    res?; // Propagate any other error.

    // Flatten weeks->blocks.
    let mut blocks: Vec<(Option<u32>, BlockToml)> = Vec::new();
    if let Some(weeks) = prog.weeks {
        for w in weeks {
            for b in w.blocks {
                blocks.push((Some(w.week), b));
            }
        }
    }
    if let Some(bs) = prog.blocks {
        for b in bs {
            blocks.push((None, b));
        }
    }

    // Insert blocks & exercises.
    for (week_opt, block) in blocks {
        // Duplicate exercise in the same block detection
        // NOTE: We do not allow in the same block something like this:
        // "day 1"
        // - Bench press
        // - Incline press
        // - Bench press
        let mut seen: HashSet<&str> = HashSet::new();
        let mut dupes: Vec<&str> = Vec::new();

        for ex in &block.exercises {
            if !seen.insert(ex.name.as_str()) {
                dupes.push(ex.name.as_str());
            }
        }

        if !dupes.is_empty() {
            println!(
                "{} block `{}` in program `{}` contains duplicate exercise names: {} – skipping this block",
                "warning:".yellow().bold(),
                block.name,
                prog.name,
                dupes.join(", ")
            );
            continue; // Don’t try to insert this block
        }

        let block_id = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            r#"INSERT INTO program_blocks
                 (id, program_id, name, description, week)
               VALUES (?1,?2,?3,?4,?5)"#,
        )
        .bind(&block_id)
        .bind(&prog_id)
        .bind(&block.name)
        .bind(block.description.as_deref())
        .bind(week_opt)
        .execute(&mut *tx)
        .await?;

        for (order_idx, ex) in block.exercises.iter().enumerate() {
            let ex_id: String = sqlx::query_scalar("SELECT id FROM exercises WHERE name = ?")
                .bind(&ex.name)
                .fetch_one(&mut *tx)
                .await?; // Safe – we validated earlier.

            let reps_csv = ex.reps.as_ref().map(|v| v.join(","));
            let rpe_csv = ex.target_rpe.as_ref().map(|v| {
                v.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            });
            let rm_csv = ex.target_rm_percent.as_ref().map(|v| {
                v.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            });
            let opt_csv = ex.options.as_ref().map(|v| v.join(","));

            sqlx::query(
                r#"INSERT INTO program_exercises
                     (id, program_block_id, exercise_id, sets, reps,
                      target_rpe, target_rm_percent, notes, program_1rm,
                      options, technique, technique_group, order_index)
                   VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)"#,
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(&block_id)
            .bind(&ex_id)
            .bind(ex.sets as i32)
            .bind(reps_csv)
            .bind(rpe_csv)
            .bind(rm_csv)
            .bind(ex.notes.as_deref())
            .bind(ex.program_1rm)
            .bind(opt_csv)
            .bind(ex.technique.as_deref())
            .bind(ex.group.map(|g| g as i32))
            .bind(order_idx as i32)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
    println!("{} `{}`", "ok:".green().bold(), prog.name);
    Ok(())
}
