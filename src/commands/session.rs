use anyhow::Result;
use colored::Colorize;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::cli::SessionCmd;

pub async fn handle(cmd: SessionCmd, pool: &SqlitePool) -> Result<()> {
    match cmd {
        SessionCmd::Start(args) => {
            // First, resolve the program name/index to its ID
            let prog_id: String = if let Ok(idx) = args.program.parse::<i64>() {
                // User passed a number - look up by row number.
                match sqlx::query_scalar(
                    r#"
                    SELECT id 
                    FROM (
                      SELECT id, ROW_NUMBER() OVER (ORDER BY name) AS rn
                      FROM programs
                    ) t
                    WHERE t.rn = ?
                    "#,
                )
                .bind(idx)
                .fetch_one(pool)
                .await
                {
                    Ok(id) => id,
                    Err(_) => {
                        println!("{} no program at index {}", "error:".red().bold(), idx);
                        return Ok(());
                    }
                }
            } else {
                // User passed a name - look up by exact name.
                match sqlx::query_scalar("SELECT id FROM programs WHERE name = ?")
                    .bind(&args.program)
                    .fetch_one(pool)
                    .await
                {
                    Ok(id) => id,
                    Err(_) => {
                        println!(
                            "{} no program named `{}`",
                            "error:".red().bold(),
                            args.program
                        );
                        return Ok(());
                    }
                }
            };

            // Then, resolve the block name to its ID.
            let block_id: String = if let Ok(idx) = args.block.parse::<i64>() {
                // User passed a number - look up by row number within this program.
                match sqlx::query_scalar(
                    r#"
                    SELECT id 
                    FROM (
                      SELECT id, ROW_NUMBER() OVER (ORDER BY name) AS rn
                      FROM program_blocks
                      WHERE program_id = ?
                    ) t
                    WHERE t.rn = ?
                    "#,
                )
                .bind(&prog_id)
                .bind(idx)
                .fetch_one(pool)
                .await
                {
                    Ok(id) => id,
                    Err(_) => {
                        println!(
                            "{} no block at index {} in program `{}`",
                            "error:".red().bold(),
                            idx,
                            args.program
                        );
                        return Ok(());
                    }
                }
            } else {
                // User passed a name - look up by exact name.
                match sqlx::query_scalar(
                    "SELECT id FROM program_blocks WHERE program_id = ? AND name = ?",
                )
                .bind(&prog_id)
                .bind(&args.block)
                .fetch_one(pool)
                .await
                {
                    Ok(id) => id,
                    Err(_) => {
                        println!(
                            "{} no block named `{}` in program `{}`",
                            "error:".red().bold(),
                            args.block,
                            args.program
                        );
                        return Ok(());
                    }
                }
            };

            // Check if there's already an active session.
            let active: Option<String> = sqlx::query_scalar("SELECT id FROM current_session")
                .fetch_optional(pool)
                .await?;

            if let Some(id) = active {
                println!(
                    "{} there is already an active session (id: {})",
                    "error:".red().bold(),
                    id
                );
                return Ok(());
            }

            // Start a transaction.
            let mut tx = pool.begin().await?;

            // Create the session
            let session_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO training_sessions (id, program_block_id, start_time) VALUES (?, ?, datetime('now'))",
            )
            .bind(&session_id)
            .bind(&block_id)
            .execute(&mut *tx)
            .await?;

            // Get all exercises for this block.
            let exercises = sqlx::query_as::<_, (String, String, i32, Option<String>)>(
                r#"
                SELECT e.id, e.name, pe.sets, pe.reps
                FROM program_exercises pe
                JOIN exercises e ON e.id = pe.exercise_id
                WHERE pe.program_block_id = ?
                ORDER BY pe.order_index
                "#,
            )
            .bind(&block_id)
            .fetch_all(&mut *tx)
            .await?;

            // Create session exercise records.
            println!("{}", "Exercises:".cyan().bold());
            for (i, (ex_id, ex_name, sets, reps)) in exercises.iter().enumerate() {
                let session_ex_id = Uuid::new_v4().to_string();
                sqlx::query(
                    "INSERT INTO training_session_exercises (id, training_session_id, exercise_id) VALUES (?, ?, ?)",
                )
                .bind(&session_ex_id)
                .bind(&session_id)
                .bind(ex_id)
                .execute(&mut *tx)
                .await?;

                // Print exercise info.
                let idx = format!("{}", i + 1).yellow();
                let reps_display = reps.as_deref().map(|r| format!(" ({})", r)).unwrap_or_default();
                println!(
                    "{} • {} — {} sets{}",
                    idx,
                    ex_name.bold(),
                    sets,
                    reps_display
                );
            }

            // Commit the transaction.
            tx.commit().await?;

            println!(
                "\n{} session started (id: {})",
                "ok:".green().bold(),
                session_id
            );

            Ok(())
        }

        SessionCmd::Cancel => {
            // Check if there's an active session.
            let active: Option<String> = sqlx::query_scalar("SELECT id FROM current_session")
                .fetch_optional(pool)
                .await?;

            if let Some(id) = active {
                // Start a transaction.
                let mut tx = pool.begin().await?;

                // Delete the session (cascade will handle exercises and sets).
                sqlx::query("DELETE FROM training_sessions WHERE id = ?")
                    .bind(&id)
                    .execute(&mut *tx)
                    .await?;

                // Commit the transaction.
                tx.commit().await?;

                println!("{} session cancelled (id: {})", "ok:".green().bold(), id);
            } else {
                println!("{} no active session to cancel", "error:".red().bold());
            }

            Ok(())
        }
    }
}
