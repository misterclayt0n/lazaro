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

        SessionCmd::Show => {
            // Get current session info
            let session: Option<(String, String, String, String)> = sqlx::query_as(
                r#"
                SELECT ts.id, ts.start_time, pb.name, COALESCE(pb.description, '')
                FROM training_sessions ts
                JOIN program_blocks pb ON pb.id = ts.program_block_id
                WHERE ts.end_time IS NULL
                LIMIT 1
                "#,
            )
            .fetch_optional(pool)
            .await?;

            if let Some((session_id, start_time, block_name, block_desc)) = session {
                // Calculate session duration
                let duration = sqlx::query_scalar::<_, String>(
                    r#"
                    SELECT strftime('%H:%M:%S', 
                        strftime('%s', 'now') - strftime('%s', ?) || ' seconds', 
                        'unixepoch'
                    )
                    "#,
                )
                .bind(&start_time)
                .fetch_one(pool)
                .await?;

                // Print session header
                println!(
                    "{} {} — {} (started {}, duration: {})",
                    "Session:".cyan().bold(),
                    block_name.bold(),
                    block_desc.dimmed(),
                    &start_time[..16],
                    duration
                );

                // Get exercises with their PRs and variants
                let exercises = sqlx::query_as::<_, (String, String, i32, Option<String>, Option<String>, Option<f32>, Option<i32>, Option<f32>, Option<f32>, Option<i32>)>(
                    r#"
                    WITH last_prs AS (
                        SELECT 
                            exercise_id,
                            MAX(date) as last_date,
                            weight,
                            reps,
                            estimated_1rm
                        FROM personal_records
                        GROUP BY exercise_id
                    )
                    SELECT 
                        e.id,
                        e.name,
                        pe.sets,
                        pe.reps,
                        e.current_pr_date,
                        e.estimated_one_rm,
                        (SELECT COUNT(*) FROM exercise_variants WHERE exercise_id = e.idx) as variant_count,
                        (SELECT estimated_1rm FROM last_prs WHERE exercise_id = e.id),
                        (SELECT weight FROM last_prs WHERE exercise_id = e.id),
                        (SELECT reps FROM last_prs WHERE exercise_id = e.id)
                    FROM training_session_exercises tse
                    JOIN exercises e ON e.id = tse.exercise_id
                    JOIN program_exercises pe ON pe.exercise_id = e.id
                    WHERE tse.training_session_id = ?
                    ORDER BY pe.order_index
                    "#,
                )
                .bind(&session_id)
                .fetch_all(pool)
                .await?;

                println!("\n{}", "Exercises:".cyan().bold());
                for (i, (ex_id, ex_name, sets, reps, last_pr_date, _est_1rm, variant_count, last_pr_1rm, pr_weight, pr_reps)) in exercises.iter().enumerate() {
                    let idx = format!("{}", i + 1).yellow();
                    
                    // Print exercise header with PR info
                    let pr_info = if let (Some(w), Some(r), Some(rm)) = (pr_weight, pr_reps, last_pr_1rm) {
                        format!(" (PR: {}kgx{} - 1RM: {}kg)", w, r, rm)
                    } else {
                        String::new()
                    };
                    
                    let last_date = last_pr_date.as_deref()
                        .map(|d| format!(" — last performed: {}", &d[..10]))
                        .unwrap_or_default();

                    println!(
                        "{} • {}{}{}",
                        idx,
                        ex_name.bold(),
                        pr_info.dimmed(),
                        last_date.dimmed()
                    );

                    // Print variants if any
                    if let Some(count) = variant_count {
                        if *count > 0 {
                            let variants = sqlx::query_as::<_, (String,)>(
                                "SELECT name FROM exercise_variants WHERE exercise_id = (SELECT idx FROM exercises WHERE id = ?) ORDER BY name"
                            )
                            .bind(ex_id)
                            .fetch_all(pool)
                            .await?;

                            for (j, (var_name,)) in variants.iter().enumerate() {
                                let connector = if j + 1 == variants.len() { "└─" } else { "├─" };
                                println!(" {} {} {}", " ".repeat(2), connector, var_name.dimmed());
                            }
                        }
                    }

                    // Print sets
                    let reps_display = reps.as_deref()
                        .map(|r| r.split(',').collect::<Vec<_>>())
                        .unwrap_or_default();

                    for set_num in 0..*sets {
                        let target_reps = if set_num < reps_display.len().try_into().unwrap() {
                            format!("{} reps", reps_display[set_num as usize])
                        } else {
                            "reps".to_string()
                        };

                        // Get previous set info if available
                        let prev_set: Option<(f32, i32)> = sqlx::query_as(
                            r#"
                            SELECT weight, reps
                            FROM exercise_sets es
                            JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
                            WHERE tse.exercise_id = ?
                            ORDER BY es.timestamp DESC
                            LIMIT 1 OFFSET ?
                            "#,
                        )
                        .bind(ex_id)
                        .bind(set_num)
                        .fetch_optional(pool)
                        .await?;

                        let prev_info = prev_set
                            .map(|(w, r)| format!(" | prev: {}kg × {}", w, r))
                            .unwrap_or_default();

                        println!(
                            " {} {} • {}| {} |{}",
                            " ".repeat(2),
                            format!("{}", set_num + 1).yellow(),
                            format!("{:<10}", target_reps),
                            format!("{:>8}", " "),
                            prev_info.dimmed()
                        );
                    }
                    println!();
                }
            } else {
                println!("{} no active session", "error:".red().bold());
            }

            Ok(())
        }
    }
}
