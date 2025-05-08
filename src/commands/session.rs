use anyhow::Result;
use colored::Colorize;
use sqlx::SqlitePool;
use std::collections::HashMap;
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
                let reps_display = reps
                    .as_deref()
                    .map(|r| format!(" ({})", r))
                    .unwrap_or_default();
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

                // Get exercises with their PRs
                let exercises = sqlx::query_as::<
                    _,
                    (
                        String,
                        String,
                        i32,
                        Option<String>,
                        Option<String>,
                        Option<f32>,
                        Option<f32>,
                        Option<f32>,
                        Option<i32>,
                        Option<String>,
                        Option<String>,
                        Option<f32>,
                    ),
                >(
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
                        (SELECT estimated_1rm FROM last_prs WHERE exercise_id = e.id),
                        (SELECT weight FROM last_prs WHERE exercise_id = e.id),
                        (SELECT reps FROM last_prs WHERE exercise_id = e.id),
                        pe.target_rpe,
                        pe.target_rm_percent,
                        pe.program_1rm
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
                for (
                    i,
                    (
                        ex_id,
                        ex_name,
                        sets,
                        reps,
                        last_pr_date,
                        _est_1rm,
                        last_pr_1rm,
                        pr_weight,
                        pr_reps,
                        target_rpe,
                        target_rm_percent,
                        program_1rm,
                    ),
                ) in exercises.iter().enumerate()
                {
                    let idx = format!("{}", i + 1).yellow();

                    // Print exercise header with PR info
                    let pr_info =
                        if let (Some(w), Some(r), Some(rm)) = (pr_weight, pr_reps, last_pr_1rm) {
                            format!(" (PR: {}kgx{} - 1RM: {}kg)", w, r, rm)
                        } else {
                            String::new()
                        };

                    let last_date = last_pr_date
                        .as_deref()
                        .map(|d| format!(" — last performed: {}", &d[..10]))
                        .unwrap_or_default();

                    println!(
                        "{} • {}{}{}",
                        idx,
                        ex_name.bold(),
                        pr_info.dimmed(),
                        last_date.dimmed()
                    );

                    // Parse target values
                    let target_rpes: Vec<f32> = target_rpe
                        .as_deref()
                        .map(|s| s.split(',').filter_map(|v| v.trim().parse().ok()).collect())
                        .unwrap_or_default();

                    let target_rms: Vec<f32> = target_rm_percent
                        .as_deref()
                        .map(|s| s.split(',').filter_map(|v| v.trim().parse().ok()).collect())
                        .unwrap_or_default();

                    // Print sets
                    let reps_display = reps
                        .as_deref()
                        .map(|r| r.split(',').collect::<Vec<_>>())
                        .unwrap_or_default();

                    for set_num in 0..*sets {
                        let target_reps = if set_num < reps_display.len().try_into().unwrap() {
                            format!("{} reps", reps_display[set_num as usize])
                        } else {
                            "reps".to_string()
                        };

                        // Get current set info
                        let current_set: Option<(f32, i32, bool)> = sqlx::query_as(
                            r#"
                            WITH set_numbers AS (
                                SELECT 
                                    es.*,
                                    ROW_NUMBER() OVER (ORDER BY es.timestamp) - 1 as set_num
                                FROM exercise_sets es
                                JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
                                WHERE tse.exercise_id = ?
                            )
                            SELECT weight, reps, bodyweight
                            FROM set_numbers
                            WHERE set_num = ?
                            "#,
                        )
                        .bind(ex_id)
                        .bind(set_num)
                        .fetch_optional(pool)
                        .await?;

                        // Get previous set info
                        let prev_set: Option<(f32, i32)> = sqlx::query_as(
                            r#"
                            WITH set_numbers AS (
                                SELECT 
                                    es.weight,
                                    es.reps,
                                    es.timestamp,
                                    tse.exercise_id,
                                    ROW_NUMBER() OVER (
                                        PARTITION BY tse.exercise_id, tse.id
                                        ORDER BY es.timestamp
                                    ) - 1 as set_num
                                FROM exercise_sets es
                                JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
                                JOIN training_sessions ts ON ts.id = tse.training_session_id
                                WHERE tse.exercise_id = ?
                                AND ts.end_time IS NOT NULL  -- Only completed sessions
                                AND es.weight > 0  -- Skip empty sets
                            ),
                            last_sets AS (
                                SELECT 
                                    weight,
                                    reps,
                                    ROW_NUMBER() OVER (
                                        PARTITION BY exercise_id, set_num
                                        ORDER BY timestamp DESC
                                    ) as rn
                                FROM set_numbers
                                WHERE set_num = ?
                            )
                            SELECT weight, reps
                            FROM last_sets
                            WHERE rn = 1
                            "#,
                        )
                        .bind(ex_id)
                        .bind(set_num)
                        .fetch_optional(pool)
                        .await?;

                        let current_info = current_set
                            .map(|(w, r, bw)| {
                                if bw {
                                    format!("bw × {}", r)
                                } else {
                                    format!("{}kg × {}", w, r)
                                }
                            })
                            .unwrap_or_default();

                        let prev_info = prev_set
                            .map(|(w, r)| format!(" - {}kg × {}", w, r))
                            .unwrap_or_default();

                        // Calculate and display target info
                        let set_num_usize = set_num as usize;
                        let target_info = if let Some(program_1rm) = program_1rm {
                            if set_num_usize < target_rpes.len() {
                                format!(" @RPE {}", target_rpes[set_num_usize])
                            } else if set_num_usize < target_rms.len() {
                                let target_weight =
                                    program_1rm * (target_rms[set_num_usize] / 100.0);
                                format!(
                                    " @{}% ({}kg)",
                                    target_rms[set_num_usize],
                                    target_weight.round()
                                )
                            } else {
                                String::new()
                            }
                        } else {
                            if set_num_usize < target_rpes.len() {
                                format!(" @RPE {}", target_rpes[set_num_usize])
                            } else {
                                String::new()
                            }
                        };

                        let target_padding = if (target_reps.len() + target_info.len()) < 25 {
                            25 - (target_reps.len() + target_info.len())
                        } else {
                            0
                        };
                        let dont_know: usize = 14;
                        let prev_visible = prev_info;
                        let prev_column =
                            format!("{:<width$}", prev_visible, width = dont_know).dimmed();

                        println!(
                            " {} {} • {}{}{}{}| {}",
                            " ".repeat(2),
                            format!("{}", set_num + 1).yellow(),
                            target_reps,
                            target_info.dimmed(),
                            " ".repeat(target_padding),
                            prev_column,
                            current_info
                        );
                    }
                    println!();
                }
            } else {
                println!("{} no active session", "error:".red().bold());
            }

            Ok(())
        }

        SessionCmd::Edit {
            exercise,
            reps,
            weight,
            bw,
            set,
        } => {
            // Check if there's an active session
            let session: Option<(String,)> =
                sqlx::query_as("SELECT id FROM current_session LIMIT 1")
                    .fetch_optional(pool)
                    .await?;

            let session_id = match session {
                Some((id,)) => id,
                None => {
                    println!("{} no active session", "error:".red().bold());
                    return Ok(());
                }
            };

            // Get the exercise ID for the given index
            let exercise_id: Option<String> = sqlx::query_scalar(
                r#"
                SELECT e.id
                FROM training_session_exercises tse
                JOIN exercises e ON e.id = tse.exercise_id
                WHERE tse.training_session_id = ?
                ORDER BY (
                    SELECT order_index 
                    FROM program_exercises 
                    WHERE exercise_id = e.id 
                    AND program_block_id = (
                        SELECT program_block_id 
                        FROM training_sessions 
                        WHERE id = ?
                    )
                )
                LIMIT 1 OFFSET ?
                "#,
            )
            .bind(&session_id)
            .bind(&session_id)
            .bind((exercise - 1) as i64)
            .fetch_optional(pool)
            .await?;

            let exercise_id = match exercise_id {
                Some(id) => id,
                None => {
                    println!(
                        "{} no exercise at index {}",
                        "error:".red().bold(),
                        exercise
                    );
                    return Ok(());
                }
            };

            // Get the session exercise ID
            let session_exercise_id: String = sqlx::query_scalar(
                "SELECT id FROM training_session_exercises WHERE training_session_id = ? AND exercise_id = ?"
            )
            .bind(&session_id)
            .bind(&exercise_id)
            .fetch_one(pool)
            .await?;

            // Determine which set to edit
            let set_index = if let Some(s) = set {
                s - 1 // Convert to 0-based index
            } else {
                // Get the next unlogged set
                sqlx::query_scalar::<_, i64>(
                    r#"
                    SELECT COUNT(*)
                    FROM exercise_sets
                    WHERE session_exercise_id = ?
                    "#,
                )
                .bind(&session_exercise_id)
                .fetch_one(pool)
                .await? as usize
            };

            // Get total number of sets for this exercise
            let total_sets: i64 = sqlx::query_scalar(
                r#"
                SELECT sets
                FROM program_exercises
                WHERE exercise_id = ? AND program_block_id = (
                    SELECT program_block_id
                    FROM training_sessions
                    WHERE id = ?
                )
                "#,
            )
            .bind(&exercise_id)
            .bind(&session_id)
            .fetch_one(pool)
            .await?;

            if set_index >= total_sets as usize {
                println!(
                    "{} no set at index {}",
                    "error:".red().bold(),
                    set_index + 1
                );
                return Ok(());
            }

            // Start a transaction
            let mut tx = pool.begin().await?;

            // Check if this set already exists
            let existing_set: Option<String> = sqlx::query_scalar(
                r#"
                WITH set_numbers AS (
                    SELECT 
                        es.id,
                        ROW_NUMBER() OVER (ORDER BY es.timestamp) - 1 as set_num
                    FROM exercise_sets es
                    WHERE es.session_exercise_id = ?
                )
                SELECT id
                FROM set_numbers
                WHERE set_num = ?
                "#,
            )
            .bind(&session_exercise_id)
            .bind(set_index as i64)
            .fetch_optional(&mut *tx)
            .await?;

            if let Some(set_id) = existing_set {
                // Update existing set
                sqlx::query(
                    r#"
                    UPDATE exercise_sets
                    SET weight = ?, reps = ?, bodyweight = ?, timestamp = datetime('now')
                    WHERE id = ?
                    "#,
                )
                .bind(if bw { 0.0 } else { weight.unwrap_or(0.0) })
                .bind(reps)
                .bind(bw as i32)
                .bind(&set_id)
                .execute(&mut *tx)
                .await?;
            } else {
                // Insert new set
                sqlx::query(
                    r#"
                    INSERT INTO exercise_sets (
                        id,
                        session_exercise_id,
                        weight,
                        reps,
                        bodyweight,
                        timestamp
                    ) VALUES (?, ?, ?, ?, ?, datetime('now'))
                    "#,
                )
                .bind(Uuid::new_v4().to_string())
                .bind(&session_exercise_id)
                .bind(if bw { 0.0 } else { weight.unwrap_or(0.0) })
                .bind(reps)
                .bind(bw as i32)
                .execute(&mut *tx)
                .await?;
            }

            // Check if this is a new PR
            let is_pr = sqlx::query_scalar::<_, bool>(
                r#"
                WITH current_pr AS (
                    SELECT weight, reps, estimated_1rm
                    FROM personal_records
                    WHERE exercise_id = ?
                    ORDER BY date DESC
                    LIMIT 1
                )
                SELECT 
                    CASE 
                        WHEN ? = 0 THEN -- Bodyweight
                            ? > (SELECT reps FROM current_pr WHERE weight = 0)
                        ELSE -- Weighted
                            ? > (SELECT estimated_1rm FROM current_pr)
                    END
                "#,
            )
            .bind(&exercise_id)
            .bind(if bw { 0.0 } else { weight.unwrap_or(0.0) })
            .bind(reps)
            .bind(if bw {
                0.0
            } else {
                weight.unwrap_or(0.0) * (1.0 + (reps as f32 / 30.0))
            })
            .fetch_one(&mut *tx)
            .await?;

            if is_pr {
                // Insert new PR
                sqlx::query(
                    r#"
                    INSERT INTO personal_records (
                        id,
                        exercise_id,
                        weight,
                        reps,
                        estimated_1rm,
                        date
                    ) VALUES (?, ?, ?, ?, ?, datetime('now'))
                    "#,
                )
                .bind(Uuid::new_v4().to_string())
                .bind(&exercise_id)
                .bind(if bw { 0.0 } else { weight.unwrap_or(0.0) })
                .bind(reps)
                .bind(if bw {
                    0.0
                } else {
                    weight.unwrap_or(0.0) * (1.0 + (reps as f32 / 30.0))
                })
                .execute(&mut *tx)
                .await?;

                // Update exercise's current PR date
                sqlx::query("UPDATE exercises SET current_pr_date = datetime('now') WHERE id = ?")
                    .bind(&exercise_id)
                    .execute(&mut *tx)
                    .await?;
            }

            // Commit the transaction
            tx.commit().await?;

            // Print success message
            let set_type = if bw { "bodyweight" } else { "weighted" };
            println!(
                "{} logged {} set {} for exercise {} ({} reps)",
                "ok:".green().bold(),
                set_type,
                set_index + 1,
                exercise,
                reps
            );

            if is_pr {
                println!("{} new personal record!", "note:".yellow().bold());
            }

            Ok(())
        }

        SessionCmd::End => {
            // Check if there's an active session
            let session: Option<(String, String, String)> = sqlx::query_as(
                r#"
                SELECT ts.id, ts.start_time, pb.name
                FROM training_sessions ts
                JOIN program_blocks pb ON pb.id = ts.program_block_id
                WHERE ts.end_time IS NULL
                LIMIT 1
                "#,
            )
            .fetch_optional(pool)
            .await?;

            let (session_id, start_time, block_name) = match session {
                Some(s) => s,
                None => {
                    println!("{} no active session", "error:".red().bold());
                    return Ok(());
                }
            };

            // Start a transaction
            let mut tx = pool.begin().await?;

            // Get all exercises and their sets for this session
            let exercises = sqlx::query_as::<_, (String, String, i32, Option<f32>, bool)>(
                r#"
                SELECT 
                    e.id,
                    e.name,
                    es.reps,
                    es.weight,
                    es.bodyweight
                FROM training_session_exercises tse
                JOIN exercises e ON e.id = tse.exercise_id
                JOIN exercise_sets es ON es.session_exercise_id = tse.id
                WHERE tse.training_session_id = ?
                ORDER BY es.timestamp
                "#,
            )
            .bind(&session_id)
            .fetch_all(&mut *tx)
            .await?;

            // Group sets by exercise
            let mut exercise_sets: HashMap<String, Vec<(i32, Option<f32>, bool)>> = HashMap::new();
            for (ex_id, _ex_name, reps, weight, bw) in exercises {
                exercise_sets
                    .entry(ex_id)
                    .or_default()
                    .push((reps, weight, bw));
            }

            // Process PRs and exercise stats
            let mut pr_updates = Vec::new();
            for (ex_id, sets) in &exercise_sets {
                // Calculate estimated 1RM for each set
                let mut max_1rm = 0.0;
                let mut pr_weight = 0.0;
                let mut pr_reps = 0;

                for (reps, weight, bw) in sets {
                    if *bw {
                        // For bodyweight exercises, we only track reps
                        if *reps > pr_reps {
                            pr_reps = *reps;
                            pr_weight = 0.0;
                        }
                    } else if let Some(w) = weight {
                        // For weighted exercises, calculate estimated 1RM
                        let est_1rm = w * (1.0 + (*reps as f32 / 30.0));
                        if est_1rm > max_1rm {
                            max_1rm = est_1rm;
                            pr_weight = *w;
                            pr_reps = *reps;
                        }
                    }
                }

                // Check if this is a new PR
                let is_pr = sqlx::query_scalar::<_, bool>(
                    r#"
                    WITH current_pr AS (
                        SELECT weight, reps, estimated_1rm
                        FROM personal_records
                        WHERE exercise_id = ?
                        ORDER BY date DESC
                        LIMIT 1
                    )
                    SELECT 
                        CASE 
                            WHEN ? = 0 THEN -- Bodyweight
                                ? > (SELECT reps FROM current_pr WHERE weight = 0)
                            ELSE -- Weighted
                                ? > (SELECT estimated_1rm FROM current_pr)
                        END
                    "#,
                )
                .bind(ex_id)
                .bind(pr_weight)
                .bind(pr_reps)
                .bind(max_1rm)
                .fetch_one(&mut *tx)
                .await?;

                if is_pr {
                    pr_updates.push((ex_id.clone(), pr_weight, pr_reps, max_1rm));
                }
            }

            // Apply PR updates
            for (ex_id, pr_weight, pr_reps, max_1rm) in pr_updates {
                // Insert new PR
                sqlx::query(
                    r#"
                    INSERT INTO personal_records (
                        id,
                        exercise_id,
                        weight,
                        reps,
                        estimated_1rm,
                        date
                    ) VALUES (?, ?, ?, ?, ?, datetime('now'))
                    "#,
                )
                .bind(Uuid::new_v4().to_string())
                .bind(&ex_id)
                .bind(pr_weight)
                .bind(pr_reps)
                .bind(max_1rm)
                .execute(&mut *tx)
                .await?;

                // Update exercise's current PR date and estimated 1RM
                sqlx::query(
                    r#"
                    UPDATE exercises 
                    SET current_pr_date = datetime('now'),
                        estimated_one_rm = ?
                    WHERE id = ?
                    "#,
                )
                .bind(max_1rm)
                .bind(&ex_id)
                .execute(&mut *tx)
                .await?;
            }

            // Mark session as ended
            sqlx::query("UPDATE training_sessions SET end_time = datetime('now') WHERE id = ?")
                .bind(&session_id)
                .execute(&mut *tx)
                .await?;

            // Commit the transaction
            tx.commit().await?;

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

            // Print summary
            println!(
                "{} session ended (id: {})",
                "ok:".green().bold(),
                session_id
            );
            println!(
                "{} {} — {} (duration: {})",
                "Session:".cyan().bold(),
                block_name.bold(),
                start_time[..16].to_string(),
                duration
            );

            // Print exercise summary
            println!("\n{}", "Exercises:".cyan().bold());
            for (ex_id, sets) in &exercise_sets {
                let exercise_name: String =
                    sqlx::query_scalar("SELECT name FROM exercises WHERE id = ?")
                        .bind(ex_id)
                        .fetch_one(pool)
                        .await?;

                println!("• {}", exercise_name.bold());
                for (reps, weight, bw) in sets {
                    if *bw {
                        println!("  - {} reps (bodyweight)", reps);
                    } else if let Some(w) = weight {
                        println!("  - {}kg × {}", w, reps);
                    }
                }
            }

            Ok(())
        }
    }
}
