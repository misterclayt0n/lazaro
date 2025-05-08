use std::{collections::BTreeSet, path::Path};

use serde::Serialize;
use sqlx::{Row, SqlitePool};
use colored::Colorize;
use crate::{cli::ExerciseCmd, types::{best_muscle_suggestions, cannonical_muscle, emit, ExerciseImport, ALLOWED_MUSCLES}, OutputFmt};
use anyhow::{Context, Result};

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
                "#
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(&name)
            .bind(muscle.to_string())
            .bind(desc.unwrap_or_default())
            .execute(pool)
            .await;

            match res {
                Ok(info) if info.rows_affected() == 1 => println!("{} Exercise \"{}\" added", "info:".blue().bold(), &name),
                Ok(_) => println!("{} Exercise \"{}\" was not inserted", "info:".blue().bold(), &name),
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
            let toml_str = tokio::fs::read_to_string(path).await.with_context(|| format!("Could not read file: `{}`", file))?;
            assert!(toml_str.is_char_boundary(toml_str.len()), "read_to_string returned invalid UTF-8");

            // Parse into Vec<ExerciseDef>.
            let import: ExerciseImport = toml::from_str(&toml_str).context("Failed to parse TOML: Expected `[[exercise]] entries`")?;

            if import.exercise.is_empty() {
                println!("{}", "warning: no [[exercise]] entries found".yellow().bold());
                return Ok(());
            }

            // Loop and insert/ignore.
            let mut inserted = 0;
            let mut skipped = 0;
            let mut unknowns: BTreeSet<String> = BTreeSet::new();
            
            for ex in import.exercise {
                assert!(!ex.name.trim().is_empty(), "exercise.name must not be empty");
                
                // Validate the `primary_muscle` field.
                let musc = match cannonical_muscle(&ex.primary_muscle) {
                    Some(m) => m,
                    None => {
                        // Did you mean?
                        if let Some(sug) = best_muscle_suggestions(&ex.primary_muscle.to_ascii_lowercase()) {
                            println!("{} `{}` skipped – unknown muscle `{}` -- did you mean: `{}`?", "warning:".yellow().bold(), ex.name, ex.primary_muscle, sug.green());
                        } else {
                            println!("{} `{}` skipped – unknown muscle `{}`", "warning:".yellow().bold(), ex.name, ex.primary_muscle);
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
                    "#
                )
                .bind(uuid::Uuid::new_v4().to_string())
                .bind(&ex.name)
                .bind(&musc)
                .bind(desc)
                .execute(pool)
                .await
                .with_context(|| format!("DB error inserting `{}`", ex.name))?;

                assert!(res.rows_affected() <= 1, "unexpected rows_affected {} for insert {}", res.rows_affected(), &ex.name);

                if res.rows_affected() == 1 {
                    inserted += 1;
                    println!("{} `{}`", "ok:".green().bold(), ex.name);
                } else {
                    skipped += 1;
                    println!("{} `{}` (already exists)", "info:".blue().bold(), ex.name);
                }
            }
            
            // Summary.
            println!("\n{} {} inserted, {} skipped", "Summary:".cyan().bold(), inserted, skipped);

            // Print allowed list if at least one exercise is unknown.
            if !unknowns.is_empty() {
                let allowed = ALLOWED_MUSCLES.iter()
                             .cloned()
                             .collect::<Vec<_>>()
                             .join(", ");

                let bad = unknowns.into_iter().collect::<Vec<_>>().join(", ");

                println!();
                println!("{} {}", "Unknown muscles:".yellow().bold(), bad);
                println!("{} {}", "Allowed muscles:".cyan().bold(), allowed);
                println!("{} You can write in any case sensitive manner (e.g. `chest` == `CHEST` == `Chest`)", "Note:".blue().bold())
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
                .map(|r| {
                    ExJson {
                        idx: r.get("idx"),
                        name: r.get("name"),
                        primary_muscle: r.get("primary_muscle"),
                        description: r.get("description"),
                        created_at: r.get("created_at"),
                    }
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
                let mut left  = Vec::<String>::new();
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
                    right.push(format!("added {}", &ex.created_at[..10]).dimmed().to_string());
                }

                // ---------- compute printable pad
                let printable_pad = left
                    .iter()
                    .map(|s| plain_len(s))
                    .max()
                    .unwrap_or(0);

                // print
                for (l, r) in left.into_iter().zip(right) {
                    let extra_hidden = l.len() - plain_len(&l);
                    let total_pad = printable_pad + extra_hidden;
                    println!("{:<total_pad$} {} {}", l, "|".blue(), r, total_pad = total_pad);
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
    }

    Ok(())
}
