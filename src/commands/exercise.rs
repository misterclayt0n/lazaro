use std::{collections::BTreeSet, path::Path};

use sqlx::{Row, SqlitePool};
use colored::Colorize;
use crate::{cli::ExerciseCmd, types::{best_muscle_suggestions, cannonical_muscle, ExerciseImport, ALLOWED_MUSCLES}};
use anyhow::{Context, Result};

pub async fn handle(cmd: ExerciseCmd, pool: &SqlitePool) -> Result<()> {
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
                SELECT name, primary_muscle, 
                COALESCE(description, '') AS description, 
                created_at
                FROM exercises
            ";

            // Add a filter if requested.
            let rows = if let Some(musc) = muscle {
                let q = format!("{base} WHERE primary_muscle = ? ORDER BY name");
                sqlx::query(&q).bind(musc).fetch_all(pool).await? // Probably not a problem using ? here.
            } else {
                let q = format!("{base} ORDER BY name");
                sqlx::query(&q).fetch_all(pool).await?
            };

            println!("{}", "Exercises:".cyan().bold());

            for row in &rows {
                let name: String        = row.get("name");
                let muscle: String      = row.get("primary_muscle");
                let desc: String        = row.get("description");
                let created_at: String  = row.get("created_at");

                // e.g.  • Preacher Curl (biceps) – EZ-bar variation • added 2025-04-29
                println!(
                    "  • {} ({}) {} {}",
                    name.bold(),
                    muscle.yellow(),
                    if desc.is_empty() {
                        "".dimmed().to_string()
                    } else {
                        format!("– {}", desc).dimmed().to_string()
                    },
                    format!("• added {}", &created_at[..10]).dimmed(),
                );
            }
            
            if rows.is_empty() {
                println!("{}", "  (no exercises found)".dimmed());
            }
        }
    }

    Ok(())
}
