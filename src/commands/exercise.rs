use std::{collections::BTreeSet, path::Path};

use crate::{
    OutputFmt,
    cli::ExerciseCmd,
    types::{ALLOWED_MUSCLES, ExerciseImport, best_muscle_suggestions, cannonical_muscle, emit},
};
use anyhow::{Context, Result};
use colored::Colorize;
use serde::Serialize;
use sqlx::{Row, SqlitePool};

#[derive(Serialize)]
struct ExJson {
    idx: i64,
    name: String,
    primary_muscle: String,
    description: String,
    created_at: String,
}

fn plain_len(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut count = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1B {
            // Skip \x1b[... m
            while i < bytes.len() && bytes[i] != b'm' {
                i += 1;
            }

            i += 1; // Skip the 'm'
        } else {
            count += 1;
            i += 1;
        }
    }

    return count;
}

pub async fn handle(cmd: ExerciseCmd, pool: &SqlitePool, fmt: OutputFmt) -> Result<()> {
    match cmd {
        ExerciseCmd::Add { name, muscle, desc } => {
            let res = sqlx::query(
                r#"
                INSERT INTO exercises
                (id, name, primary_muscle, description, created_at)
                VALUES (?1, ?2, ?3, ?4, datetime('now'))
                "#,
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(&name)
            .bind(muscle.to_string())
            .bind(desc.unwrap_or_default())
            .execute(pool)
            .await;

            match res {
                Ok(info) if info.rows_affected() == 1 => {
                    println!("{} Exercise \"{}\" added", "info:".blue().bold(), &name)
                }
                Ok(_) => println!(
                    "{} Exercise \"{}\" was not inserted",
                    "info:".blue().bold(),
                    &name
                ),
                Err(sqlx::Error::Database(db_err)) if db_err.code() == Some("2067".into()) => {
                    // 2067 = SQLITE_CONSTRAINT_UNIQUE
                    println!(
                        "{} Exercise \"{}\" already exists — use `ex list` to view all exercises",
                        "warning:".yellow().bold(),
                        name
                    );
                }
                Err(e) => {
                    println!("{} {}", "error:".red().bold(), e.to_string().red());
                    return Err(e.into());
                }
            }
        }

        ExerciseCmd::Import { file } => {
            let path = Path::new(&file);
            let toml_str = tokio::fs::read_to_string(path)
                .await
                .with_context(|| format!("Could not read file: `{}`", file))?;
            assert!(
                toml_str.is_char_boundary(toml_str.len()),
                "read_to_string returned invalid UTF-8"
            );

            // Parse into Vec<ExerciseDef>.
            let import: ExerciseImport = toml::from_str(&toml_str)
                .context("Failed to parse TOML: Expected `[[exercise]] entries`")?;

            if import.exercise.is_empty() {
                println!(
                    "{}",
                    "warning: no [[exercise]] entries found".yellow().bold()
                );
                return Ok(());
            }

            // Loop and insert/ignore.
            let mut inserted = 0;
            let mut skipped = 0;
            let mut unknowns: BTreeSet<String> = BTreeSet::new();

            for ex in import.exercise {
                assert!(
                    !ex.name.trim().is_empty(),
                    "exercise.name must not be empty"
                );

                // Validate the `primary_muscle` field.
                let musc = match cannonical_muscle(&ex.primary_muscle) {
                    Some(m) => m,
                    None => {
                        // Did you mean?
                        if let Some(sug) =
                            best_muscle_suggestions(&ex.primary_muscle.to_ascii_lowercase())
                        {
                            println!(
                                "{} `{}` skipped – unknown muscle `{}` -- did you mean: `{}`?",
                                "warning:".yellow().bold(),
                                ex.name,
                                ex.primary_muscle,
                                sug.green()
                            );
                        } else {
                            println!(
                                "{} `{}` skipped – unknown muscle `{}`",
                                "warning:".yellow().bold(),
                                ex.name,
                                ex.primary_muscle
                            );
                        }

                        skipped += 1;
                        unknowns.insert(ex.primary_muscle);
                        continue;
                    }
                };

                let desc = ex.description.unwrap_or_default();

                let res = sqlx::query(
                    r#"
                    INSERT OR IGNORE INTO exercises
                      (id, name, primary_muscle, description, created_at)
                    VALUES (?1, ?2, ?3, ?4, datetime('now'))
                    "#,
                )
                .bind(uuid::Uuid::new_v4().to_string())
                .bind(&ex.name)
                .bind(&musc)
                .bind(desc)
                .execute(pool)
                .await
                .with_context(|| format!("DB error inserting `{}`", ex.name))?;

                assert!(
                    res.rows_affected() <= 1,
                    "unexpected rows_affected {} for insert {}",
                    res.rows_affected(),
                    &ex.name
                );

                if res.rows_affected() == 1 {
                    inserted += 1;
                    println!("{} `{}`", "ok:".green().bold(), ex.name);
                } else {
                    skipped += 1;
                    println!("{} `{}` (already exists)", "info:".blue().bold(), ex.name);
                }
            }

            // Summary.
            println!(
                "\n{} {} inserted, {} skipped",
                "Summary:".cyan().bold(),
                inserted,
                skipped
            );

            // Print allowed list if at least one exercise is unknown.
            if !unknowns.is_empty() {
                let allowed = ALLOWED_MUSCLES
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ");

                let bad = unknowns.into_iter().collect::<Vec<_>>().join(", ");

                println!();
                println!("{} {}", "Unknown muscles:".yellow().bold(), bad);
                println!("{} {}", "Allowed muscles:".cyan().bold(), allowed);
                println!(
                    "{} You can write in any case sensitive manner (e.g. `chest` == `CHEST` == `Chest`)",
                    "Note:".blue().bold()
                )
            }
        }

        ExerciseCmd::List { muscle } => {
            let base = "
                SELECT idx, name, primary_muscle, 
                COALESCE(description, '') AS description, 
                created_at
                FROM exercises
            ";

            // Add a filter if requested.
            let db_rows = if let Some(musc) = muscle {
                let q = format!("{base} WHERE primary_muscle = ? ORDER BY idx");
                sqlx::query(&q).bind(musc).fetch_all(pool).await? // Probably not a problem using ? here.
            } else {
                let q = format!("{base} ORDER BY idx");
                sqlx::query(&q).fetch_all(pool).await?
            };

            let json_rows: Vec<ExJson> = db_rows
                .iter()
                .map(|r| ExJson {
                    idx: r.get("idx"),
                    name: r.get("name"),
                    primary_muscle: r.get("primary_muscle"),
                    description: r.get("description"),
                    created_at: r.get("created_at"),
                })
                .collect();

            emit(fmt, &json_rows, || {
                println!("{}", "Exercises:".cyan().bold());

                // ---------- widths
                let idx_w = json_rows
                    .iter()
                    .map(|e| e.idx.to_string().len())
                    .max()
                    .unwrap_or(1);

                // build all lines first
                let mut left = Vec::<String>::new();
                let mut right = Vec::<String>::new();

                for ex in &json_rows {
                    // exercise row
                    let idx_col = format!("{:>width$}", ex.idx, width = idx_w).yellow();
                    let desc = if ex.description.is_empty() {
                        String::new()
                    } else {
                        format!("– {}", ex.description).dimmed().to_string()
                    };
                    left.push(format!(
                        " {} • {} ({}) {}",
                        idx_col,
                        ex.name.bold(),
                        ex.primary_muscle.yellow(),
                        desc
                    ));
                    right.push(
                        format!("added {}", &ex.created_at[..10])
                            .dimmed()
                            .to_string(),
                    );
                }

                // ---------- compute printable pad
                let printable_pad = left.iter().map(|s| plain_len(s)).max().unwrap_or(0);

                // print
                for (l, r) in left.into_iter().zip(right) {
                    let extra_hidden = l.len() - plain_len(&l);
                    let total_pad = printable_pad + extra_hidden;
                    println!(
                        "{:<total_pad$} {} {}",
                        l,
                        "|".blue(),
                        r,
                        total_pad = total_pad
                    );
                }

                if json_rows.is_empty() {
                    println!("{}", "  (no exercises found)".dimmed());
                }
            });
        }

        ExerciseCmd::Delete { exercise } => {
            // Resolve exercise to its idx.
            let idx: i64 = if let Ok(n) = exercise.parse::<i64>() {
                // User passed a number.
                n
            } else {
                // User passed a name: look it up.
                match sqlx::query_scalar("SELECT idx FROM exercises WHERE name = ?")
                    .bind(&exercise)
                    .fetch_one(pool)
                    .await
                {
                    Ok(n) => n,
                    Err(_) => {
                        println!("{} no such exercise `{}`", "error:".red().bold(), exercise);
                        return Ok(());
                    }
                }
            };

            // Get exercise name for confirmation message.
            let name: String = sqlx::query_scalar("SELECT name FROM exercises WHERE idx = ?")
                .bind(idx)
                .fetch_one(pool)
                .await?;

            // Delete the exercise (cascade will handle variants).
            sqlx::query("DELETE FROM exercises WHERE idx = ?")
                .bind(idx)
                .execute(pool)
                .await?;

            println!("{} deleted exercise `{}`", "ok:".green().bold(), name);
        }

        ExerciseCmd::Show { exercise } => {
            let exercise = exercise.join(" ");
            
            // Resolve exercise to its ID
            let exercise_id: String = if let Ok(idx) = exercise.parse::<i64>() {
                // User passed a number - look up by idx
                match sqlx::query_scalar("SELECT id FROM exercises WHERE idx = ?")
                    .bind(idx)
                    .fetch_optional(pool)
                    .await?
                {
                    Some(id) => id,
                    None => {
                        println!("{} no exercise at index {}", "error:".red().bold(), idx);
                        return Ok(());
                    }
                }
            } else {
                // User passed a name - look up by exact name
                match sqlx::query_scalar("SELECT id FROM exercises WHERE name = ?")
                    .bind(&exercise)
                    .fetch_optional(pool)
                    .await?
                {
                    Some(id) => id,
                    None => {
                        println!("{} no exercise named `{}`", "error:".red().bold(), exercise);
                        return Ok(());
                    }
                }
            };

            // Get basic exercise info
            let (name, muscle, created_at): (String, String, String) = sqlx::query_as(
                "SELECT name, primary_muscle, created_at FROM exercises WHERE id = ?",
            )
            .bind(&exercise_id)
            .fetch_one(pool)
            .await?;

            // Get last performed date and total sessions
            let (last_performed, total_sessions): (Option<String>, i64) = sqlx::query_as(
                r#"
                WITH exercise_sessions AS (
                    SELECT DISTINCT ts.id, ts.start_time
                    FROM training_sessions ts
                    JOIN training_session_exercises tse ON tse.training_session_id = ts.id
                    WHERE tse.exercise_id = ?
                    AND ts.end_time IS NOT NULL
                )
                SELECT 
                    MAX(start_time) as last_performed,
                    CAST(COUNT(*) AS INTEGER) as total_sessions
                FROM exercise_sessions
                "#,
            )
            .bind(&exercise_id)
            .fetch_one(pool)
            .await?;

            // Get current PR info
            let (pr_weight, pr_reps, pr_date, pr_1rm): (Option<f32>, Option<i32>, Option<String>, Option<f32>) = sqlx::query_as(
                r#"
                WITH all_sets AS (
                    SELECT 
                        es.weight,
                        es.reps,
                        es.timestamp,
                        CASE 
                            WHEN es.bodyweight = 1 THEN 0
                            ELSE CAST(es.weight AS REAL) * (1 + CAST(es.reps AS REAL) / 30)
                        END as estimated_1rm
                    FROM exercise_sets es
                    JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
                    WHERE tse.exercise_id = ?
                    AND es.weight > 0
                )
                SELECT 
                    weight,
                    reps,
                    timestamp,
                    estimated_1rm
                FROM all_sets
                ORDER BY estimated_1rm DESC, weight DESC, reps DESC
                LIMIT 1
                "#,
            )
            .bind(&exercise_id)
            .fetch_optional(pool)
            .await?
            .unwrap_or((None, None, None, None));

            // Get 30-day PR change
            let (prev_pr_1rm, _prev_pr_date): (Option<f32>, Option<String>) = sqlx::query_as(
                r#"
                WITH all_sets AS (
                    SELECT 
                        es.weight,
                        es.reps,
                        es.timestamp,
                        CASE 
                            WHEN es.bodyweight = 1 THEN 0
                            ELSE CAST(es.weight AS REAL) * (1 + CAST(es.reps AS REAL) / 30)
                        END as estimated_1rm
                    FROM exercise_sets es
                    JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
                    WHERE tse.exercise_id = ?
                    AND es.weight > 0
                    AND es.timestamp < datetime('now', '-30 days')
                )
                SELECT 
                    estimated_1rm,
                    timestamp
                FROM all_sets
                ORDER BY estimated_1rm DESC, weight DESC, reps DESC
                LIMIT 1
                "#,
            )
            .bind(&exercise_id)
            .fetch_optional(pool)
            .await?
            .unwrap_or((None, None));

            // Get 30-day tonnage
            let (current_tonnage, prev_tonnage): (Option<f64>, Option<f64>) = sqlx::query_as(
                r#"
                WITH current_tonnage AS (
                    SELECT CAST(COALESCE(SUM(CAST(weight AS REAL) * CAST(reps AS INTEGER)), 0) AS REAL) as total
                    FROM exercise_sets es
                    JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
                    WHERE tse.exercise_id = ?
                    AND es.timestamp >= datetime('now', '-30 days')
                ),
                prev_tonnage AS (
                    SELECT CAST(COALESCE(SUM(CAST(weight AS REAL) * CAST(reps AS INTEGER)), 0) AS REAL) as total
                    FROM exercise_sets es
                    JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
                    WHERE tse.exercise_id = ?
                    AND es.timestamp >= datetime('now', '-60 days')
                    AND es.timestamp < datetime('now', '-30 days')
                )
                SELECT 
                    (SELECT total FROM current_tonnage),
                    (SELECT total FROM prev_tonnage)
                "#
            )
            .bind(&exercise_id)
            .bind(&exercise_id)
            .fetch_one(pool)
            .await?;

            // Get lifetime volume stats
            let (total_sets, total_reps, total_tonnage): (i64, i64, f64) = sqlx::query_as(
                r#"
                SELECT 
                    CAST(COUNT(*) AS INTEGER) as sets,
                    CAST(COALESCE(SUM(CAST(reps AS INTEGER)), 0) AS INTEGER) as reps,
                    CAST(COALESCE(SUM(CAST(weight AS REAL) * CAST(reps AS INTEGER)), 0) AS REAL) as tonnage
                FROM exercise_sets es
                JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
                WHERE tse.exercise_id = ?
                "#
            )
            .bind(&exercise_id)
            .fetch_one(pool)
            .await?;

            // Get average frequency and longest gap
            let (avg_freq, longest_gap): (Option<f64>, Option<i64>) = sqlx::query_as(
                r#"
                WITH session_dates AS (
                    SELECT DISTINCT date(ts.start_time) as session_date
                    FROM training_sessions ts
                    JOIN training_session_exercises tse ON tse.training_session_id = ts.id
                    WHERE tse.exercise_id = ?
                    AND ts.end_time IS NOT NULL
                    ORDER BY session_date
                ),
                gaps AS (
                    SELECT 
                        CAST(julianday(session_date) - julianday(LAG(session_date) OVER (ORDER BY session_date)) AS INTEGER) as gap
                    FROM session_dates
                )
                SELECT 
                    CAST((SELECT COUNT(*) * 7.0 / 56.0 FROM session_dates WHERE session_date >= date('now', '-56 days')) AS REAL),
                    (SELECT MAX(gap) FROM gaps)
                "#
            )
            .bind(&exercise_id)
            .fetch_one(pool)
            .await?;

            // Get top 5 heaviest sets
            let top_sets: Vec<(f32, i32, String)> = sqlx::query_as(
                r#"
                WITH set_volumes AS (
                    SELECT 
                        CAST(weight AS REAL) as weight,
                        CAST(reps AS INTEGER) as reps,
                        timestamp,
                        CASE 
                            WHEN bodyweight = 1 THEN 0
                            ELSE CAST(weight AS REAL) * (1 + CAST(reps AS REAL) / 30)
                        END as estimated_1rm
                    FROM exercise_sets es
                    JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
                    WHERE tse.exercise_id = ?
                    AND weight > 0
                )
                SELECT 
                    weight,
                    reps,
                    timestamp
                FROM set_volumes
                ORDER BY estimated_1rm DESC, weight DESC, reps DESC
                LIMIT 5
                "#,
            )
            .bind(&exercise_id)
            .fetch_all(pool)
            .await?;

            // Get last 10 sets with PR information
            let last_sets: Vec<(String, f32, i32, Option<f32>, bool)> = sqlx::query_as(
                r#"
                WITH set_info AS (
                    SELECT 
                        es.timestamp,
                        CAST(es.weight AS REAL) as weight,
                        CAST(es.reps AS INTEGER) as reps,
                        CAST(es.rpe AS REAL) as rpe,
                        CASE 
                            WHEN es.bodyweight = 1 THEN 0
                            ELSE CAST(es.weight AS REAL) * (1 + CAST(es.reps AS REAL) / 30)
                        END as estimated_1rm,
                        ROW_NUMBER() OVER (
                            ORDER BY 
                                CAST(es.weight AS REAL) * (1 + CAST(es.reps AS REAL) / 30) DESC,
                                es.timestamp DESC
                        ) as set_rank
                    FROM exercise_sets es
                    JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
                    WHERE tse.exercise_id = ?
                    AND es.weight > 0
                    ORDER BY es.timestamp DESC
                    LIMIT 10
                )
                SELECT 
                    timestamp,
                    weight,
                    reps,
                    rpe,
                    set_rank = 1 as is_pr
                FROM set_info
                ORDER BY timestamp DESC
                "#,
            )
            .bind(&exercise_id)
            .fetch_all(pool)
            .await?;

            // Print exercise header
            println!(
                "{}: {} ({})",
                "Exercise".cyan().bold(),
                name.bold(),
                muscle.yellow()
            );
            println!(
                "{}: {} | {}: {} | {}: {}",
                "Added".dimmed(),
                &created_at[..10],
                "Last performed".dimmed(),
                last_performed.map_or("never".to_string(), |d| d[..10].to_string()),
                "Total sessions".dimmed(),
                total_sessions
            );
            println!();

            // Print PR info
            if let (Some(w), Some(r), Some(d), Some(rm)) = (pr_weight, pr_reps, pr_date, pr_1rm) {
                println!(
                    "{}: {}kg × {}  (1 RM est: {}kg)  on {}",
                    "Current PR".cyan().bold(),
                    w,
                    r,
                    rm.round(),
                    &d[..10]
                );
            }

            // Print 30-day changes
            if let (Some(prev_rm), _) = (prev_pr_1rm, _prev_pr_date) {
                let diff = pr_1rm.unwrap_or(0.0) - prev_rm;
                let pct = (diff / prev_rm) * 100.0;
                let arrow = if diff > 0.0 { "▲" } else { "▼" };
                println!(
                    "{} {} {:.1} kg  ({:+.1} %)",
                    "30-day 1 RM change:".cyan().bold(),
                    arrow,
                    diff.abs(),
                    pct
                );
            }

            if let (Some(curr), Some(prev)) = (current_tonnage, prev_tonnage) {
                println!(
                    "{}: {:.0} kg   (prev 30 d: {:.0} kg)",
                    "30-day tonnage".cyan().bold(),
                    curr,
                    prev
                );
            }
            println!();

            // Print lifetime stats
            println!(
                "{}: {} sets  – {} reps  – {:.0} t",
                "Lifetime volume".cyan().bold(),
                total_sets,
                total_reps,
                total_tonnage
            );

            if let (Some(freq), Some(gap)) = (avg_freq, longest_gap) {
                println!(
                    "{}: {:.1} sessions / week | {}: {} days",
                    "Avg frequency (8 w)".cyan().bold(),
                    freq,
                    "Longest gap".cyan().bold(),
                    gap
                );
            }
            println!();

            // Print top 5 heaviest sets
            println!("{}", "Top 5 heaviest sets".cyan().bold());
            for (weight, reps, timestamp) in top_sets {
                println!("  {}kg × {}   {}", weight, reps, &timestamp[..10]);
            }
            println!();

            // Print last 10 sets
            println!("{}", "Last 10 sets".cyan().bold());
            for (timestamp, weight, reps, rpe, is_pr) in last_sets {
                let set_info = if weight == 0.0 {
                    format!("bw × {}", reps)
                } else {
                    format!("{}kg × {}", weight, reps)
                };

                let rpe_info = rpe.map_or(String::new(), |r| format!("   @RPE {}", r));
                let pr_mark = if is_pr {
                    "   ← PR".green().to_string()
                } else {
                    String::new()
                };

                let set_display = if is_pr {
                    set_info.green().to_string()
                } else {
                    set_info
                };

                println!(
                    "  {}  {}{}{}",
                    &timestamp[..10],
                    set_display,
                    rpe_info.dimmed(),
                    pr_mark
                );
            }
        }
    }

    Ok(())
}
