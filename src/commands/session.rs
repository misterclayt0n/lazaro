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
                    ),
                    session_exercise_order AS (
                        -- Use SQLite rowid to maintain original insertion order
                        SELECT 
                            tse.id as tse_id,
                            tse.exercise_id,
                            ROW_NUMBER() OVER (ORDER BY tse.rowid) as display_order
                        FROM training_session_exercises tse
                        WHERE tse.training_session_id = ?
                    )
                    SELECT 
                        e.id,
                        e.name,
                        COALESCE(pe.sets, 3) as sets, -- Default to 3 sets for swapped exercises
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
                    JOIN session_exercise_order seo ON seo.tse_id = tse.id
                    JOIN exercises e ON e.id = tse.exercise_id
                    LEFT JOIN program_exercises pe ON pe.exercise_id = e.id 
                        AND pe.program_block_id = (
                            SELECT program_block_id 
                            FROM training_sessions 
                            WHERE id = ?
                        )
                    WHERE tse.training_session_id = ?
                    ORDER BY seo.display_order
                    "#,
                )
                .bind(&session_id)
                .bind(&session_id)
                .bind(&session_id)
                .fetch_all(pool)
                .await?;

                println!("\n{}", "Exercises:".cyan().bold());

                // Pre-calculate all previous set information to find the maximum width
                let mut prev_sets_info = Vec::new();
                for (
                    _i,
                    (
                        ex_id,
                        _ex_name,
                        sets,
                        _reps,
                        _last_pr_date,
                        _est_1rm,
                        _last_pr_1rm,
                        _pr_weight,
                        _pr_reps,
                        _target_rpe,
                        _target_rm_percent,
                        _program_1rm,
                    ),
                ) in exercises.iter().enumerate()
                {
                    let mut exercise_prev_sets = Vec::new();

                    // For each set
                    for set_num in 0..*sets {
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

                        let prev_info = prev_set
                            .map(|(w, r)| format!(" - {}kg × {}", w, r))
                            .unwrap_or_default();

                        exercise_prev_sets.push(prev_info);
                    }

                    prev_sets_info.push(exercise_prev_sets);
                }

                // Find maximum width of prev_info
                let max_prev_width = prev_sets_info
                    .iter()
                    .flat_map(|sets| sets.iter().map(|s| s.len()))
                    .max()
                    .unwrap_or(0);

                // Now display everything with consistent padding
                for (
                    i,
                    (
                        ex_id,
                        ex_name,
                        sets,
                        reps,
                        _last_pr_date,
                        _est_1rm,
                        _last_pr_1rm,
                        _pr_weight,
                        _pr_reps,
                        _target_rpe,
                        _target_rm_percent,
                        _program_1rm,
                    ),
                ) in exercises.iter().enumerate()
                {
                    let idx = format!("{}", i + 1).yellow();

                    // Print exercise header with PR info
                    let pr_info = if let (Some(w), Some(r)) = (_pr_weight, _pr_reps) {
                        let one_rm = epley_1rm(*w, *r).round();
                        let actual_pr = format!("{}kg × {}", w, r).red().bold().to_string();
                        format!(" - PR: {} (1RM: {}kg)", actual_pr, one_rm)
                    } else {
                        String::new()
                    };

                    println!(
                        "{} • {}{}",
                        idx,
                        ex_name.bold(),
                        pr_info.dimmed(),
                        // last_date.dimmed()
                    );

                    // Parse target values
                    let target_rpes: Vec<f32> = _target_rpe
                        .as_deref()
                        .map(|s| s.split(',').filter_map(|v| v.trim().parse().ok()).collect())
                        .unwrap_or_default();

                    let target_rms: Vec<f32> = _target_rm_percent
                        .as_deref()
                        .map(|s| s.split(',').filter_map(|v| v.trim().parse().ok()).collect())
                        .unwrap_or_default();

                    // Print sets
                    let reps_display = reps
                        .as_deref()
                        .map(|r| r.split(',').collect::<Vec<_>>())
                        .unwrap_or_default();

                    for set_num in 0..*sets {
                        let set_num_usize = set_num as usize;
                        let target_info = if let Some(program_1rm) = _program_1rm {
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
                                AND tse.training_session_id = ?
                            )
                            SELECT weight, reps, bodyweight
                            FROM set_numbers
                            WHERE set_num = ?
                            "#,
                        )
                        .bind(ex_id)
                        .bind(&session_id)
                        .bind(set_num)
                        .fetch_optional(pool)
                        .await?;

                        // Determine if this set is a PR by comparing to the PR table
                        let is_pr = if let Some((weight, reps, bw)) = current_set {
                            if bw {
                                // For bodyweight, check if reps exceed the PR reps
                                let pr_reps: Option<i32> = sqlx::query_scalar(
                                    "SELECT MAX(reps) FROM personal_records WHERE exercise_id = ? AND weight = 0"
                                )
                                .bind(ex_id)
                                .fetch_optional(pool)
                                .await?;

                                match pr_reps {
                                    Some(max_reps) => reps >= max_reps,
                                    None => false,
                                }
                            } else if weight > 0.0 {
                                // For weighted, check if this matches the PR weight/reps
                                let is_matching_pr: Option<bool> = sqlx::query_scalar(
                                    "SELECT 1 FROM personal_records WHERE exercise_id = ? AND weight = ? AND reps = ? LIMIT 1"
                                )
                                .bind(ex_id)
                                .bind(weight)
                                .bind(reps)
                                .fetch_optional(pool)
                                .await?;

                                is_matching_pr.is_some()
                            } else {
                                false
                            }
                        } else {
                            false
                        };

                        let current_info = current_set
                            .map(|(w, r, bw)| {
                                if bw {
                                    format!("bw × {}", r)
                                } else {
                                    format!("{}kg × {}", w, r)
                                }
                            })
                            .unwrap_or_default();

                        // Apply green color if it's a PR
                        let current_info_colored = if is_pr && !current_info.is_empty() {
                            current_info.green().to_string()
                        } else {
                            current_info
                        };

                        let prev_info = &prev_sets_info[i][set_num as usize];
                        let prev_column =
                            format!("{:<width$}", prev_info, width = max_prev_width).dimmed();

                        let target_reps = if set_num_usize < reps_display.len() {
                            format!("{} reps", reps_display[set_num_usize])
                        } else {
                            String::from("do your thing")
                        };

                        let target_padding = if (target_reps.len() + target_info.len()) < 25 {
                            25 - (target_reps.len() + target_info.len())
                        } else {
                            0
                        };

                        // Create all parts of the display separately
                        let set_num_str = format!("{}", set_num + 1).yellow();
                        let indent = " ".repeat(2);
                        let target_part = if target_reps.is_empty() {
                            String::new()
                        } else {
                            format!("{}{}", target_reps, target_info.dimmed())
                        };
                        let padding = " ".repeat(target_padding);

                        // Print with explicit parts
                        println!(
                            " {} {} • {} {}{} | {}",
                            indent,
                            set_num_str,
                            target_part,
                            padding,
                            prev_column,
                            current_info_colored
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
            weight,
            reps,
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

            // Parse weight - handle bodyweight exercises
            let (is_bodyweight, parsed_weight) = if weight.to_lowercase() == "bw" {
                (true, None)
            } else {
                match weight.parse::<f32>() {
                    Ok(w) => (false, Some(w)),
                    Err(_) => {
                        println!("{} invalid weight: {}", "error:".red().bold(), weight);
                        return Ok(());
                    }
                }
            };

            // Get the exercise ID for the given index
            let exercise_info: Option<(String, String)> = sqlx::query_as(
                r#"
                WITH session_exercise_order AS (
                    -- Use SQLite rowid to maintain original insertion order
                    SELECT 
                        tse.id as tse_id,
                        tse.exercise_id,
                        ROW_NUMBER() OVER (ORDER BY tse.rowid) as display_order
                    FROM training_session_exercises tse
                    WHERE tse.training_session_id = ?
                )
                SELECT tse.exercise_id, tse.id as session_exercise_id
                FROM training_session_exercises tse
                JOIN session_exercise_order seo ON seo.tse_id = tse.id
                WHERE tse.training_session_id = ?
                ORDER BY seo.display_order
                LIMIT 1 OFFSET ?
                "#,
            )
            .bind(&session_id)
            .bind(&session_id)
            .bind((exercise - 1) as i64)
            .fetch_optional(pool)
            .await?;

            let (exercise_id, session_exercise_id) = match exercise_info {
                Some(info) => info,
                None => {
                    println!(
                        "{} no exercise at index {}",
                        "error:".red().bold(),
                        exercise
                    );
                    return Ok(());
                }
            };

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

            // Get total number of sets for this exercise, default to 3 for swapped exercises
            let total_sets: i64 = match sqlx::query_scalar::<_, i64>(
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
            .fetch_optional(pool)
            .await?
            {
                Some(sets) => sets,
                None => 3, // Default to 3 sets for swapped exercises
            };

            if set_index >= total_sets as usize {
                println!(
                    "{} no set at index {} (max: {})",
                    "error:".red().bold(),
                    set_index + 1,
                    total_sets
                );
                return Ok(());
            }

            // Start a transaction
            let mut tx = pool.begin().await?;

            // Check if this set already exists and fetch its creation date
            let existing_set: Option<(String, String)> = sqlx::query_as(
                r#"
                WITH set_numbers AS (
                    SELECT 
                        es.id,
                        es.timestamp,
                        ROW_NUMBER() OVER (ORDER BY es.timestamp) - 1 as set_num
                    FROM exercise_sets es
                    WHERE es.session_exercise_id = ?
                )
                SELECT id, timestamp
                FROM set_numbers
                WHERE set_num = ?
                "#,
            )
            .bind(&session_exercise_id)
            .bind(set_index as i64)
            .fetch_optional(&mut *tx)
            .await?;

            if let Some((id, _timestamp)) = existing_set {
                // Update existing set
                sqlx::query(
                    r#"
                    UPDATE exercise_sets
                    SET weight = ?, reps = ?, bodyweight = ?, timestamp = datetime('now')
                    WHERE id = ?
                    "#,
                )
                .bind(if is_bodyweight {
                    0.0
                } else {
                    parsed_weight.unwrap_or(0.0)
                })
                .bind(reps)
                .bind(is_bodyweight as i32)
                .bind(&id)
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
                .bind(if is_bodyweight {
                    0.0
                } else {
                    parsed_weight.unwrap_or(0.0)
                })
                .bind(reps)
                .bind(is_bodyweight as i32)
                .execute(&mut *tx)
                .await?;
            }

            // Check if this is a new PR
            let estimated_1rm = if is_bodyweight {
                0.0 // For bodyweight, we don't calculate 1RM
            } else {
                epley_1rm(parsed_weight.unwrap_or(0.0), reps)
            };

            // Get the current PR for this exercise
            let current_pr: Option<(f32, i32, f32)> = sqlx::query_as(
                r#"
                SELECT weight, reps, estimated_1rm
                FROM personal_records
                WHERE exercise_id = ?
                ORDER BY estimated_1rm DESC
                LIMIT 1
                "#,
            )
            .bind(&exercise_id)
            .fetch_optional(&mut *tx)
            .await?;

            let is_pr = match current_pr {
                Some((curr_weight, curr_reps, curr_1rm)) => {
                    if is_bodyweight {
                        // For bodyweight, PR is when reps is higher
                        reps > curr_reps && curr_weight == 0.0
                    } else {
                        // For weighted, PR is when estimated 1RM is higher
                        estimated_1rm > curr_1rm
                    }
                }
                None => true, // First ever set is always a PR
            };

            if is_pr {
                // Insert new PR
                sqlx::query(
                    r#"
                    INSERT INTO personal_records (
                        exercise_id,
                        date,
                        weight,
                        reps,
                        estimated_1rm
                    ) VALUES (?, datetime('now'), ?, ?, ?)
                    "#,
                )
                .bind(&exercise_id)
                .bind(if is_bodyweight {
                    0.0
                } else {
                    parsed_weight.unwrap_or(0.0)
                })
                .bind(reps)
                .bind(estimated_1rm)
                .execute(&mut *tx)
                .await?;

                // Update exercise's current PR date
                sqlx::query("UPDATE exercises SET current_pr_date = datetime('now'), estimated_one_rm = ? WHERE id = ?")
                    .bind(estimated_1rm)
                    .bind(&exercise_id)
                    .execute(&mut *tx)
                    .await?;
            }

            // Commit the transaction
            tx.commit().await?;

            // Print success message
            let set_type = if is_bodyweight {
                "bodyweight"
            } else {
                "weighted"
            };
            let weight_display = if is_bodyweight {
                "bodyweight".to_string()
            } else {
                format!("{}kg", parsed_weight.unwrap_or(0.0))
            };

            println!(
                "{} logged {} set {} for exercise {} ({} × {})",
                "ok:".green().bold(),
                set_type,
                set_index + 1,
                exercise,
                weight_display,
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

        SessionCmd::Swap {
            exercise,
            new_exercise,
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

            // Get information about the current session's block
            let program_block_id: String =
                sqlx::query_scalar("SELECT program_block_id FROM training_sessions WHERE id = ?")
                    .bind(&session_id)
                    .fetch_one(pool)
                    .await?;

            // Get the exercise to replace info with its order_index
            let old_exercise_info: Option<(String, String, String)> = sqlx::query_as(
                r#"
                WITH session_exercise_order AS (
                    -- Use SQLite rowid to maintain original insertion order
                    SELECT 
                        tse.id as tse_id,
                        tse.exercise_id,
                        ROW_NUMBER() OVER (ORDER BY tse.rowid) as display_order
                    FROM training_session_exercises tse
                    WHERE tse.training_session_id = ?
                )
                SELECT tse.id, tse.exercise_id, e.name
                FROM training_session_exercises tse
                JOIN session_exercise_order seo ON seo.tse_id = tse.id
                JOIN exercises e ON e.id = tse.exercise_id
                WHERE tse.training_session_id = ?
                ORDER BY seo.display_order
                LIMIT 1 OFFSET ?
                "#,
            )
            .bind(&session_id)
            .bind(&session_id)
            .bind((exercise - 1) as i64)
            .fetch_optional(pool)
            .await?;

            let (old_session_exercise_id, _old_exercise_id, old_exercise_name) =
                match old_exercise_info {
                    Some(info) => info,
                    None => {
                        println!(
                            "{} no exercise at index {} in current session",
                            "error:".red().bold(),
                            exercise
                        );
                        return Ok(());
                    }
                };

            // Resolve the new exercise (by index or name)
            let new_exercise_id: String = if let Ok(idx) = new_exercise.parse::<i64>() {
                // User provided an index from exercise list
                match sqlx::query_scalar::<_, String>(
                    r#"
                    SELECT id 
                    FROM exercises
                    ORDER BY idx  -- Order by the autoincrement field, not by name
                    LIMIT 1 OFFSET ?
                    "#,
                )
                .bind(idx - 1) // Convert to 0-based for SQL
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
                // User provided an exercise name
                match sqlx::query_scalar::<_, String>("SELECT id FROM exercises WHERE name = ?")
                    .bind(&new_exercise)
                    .fetch_optional(pool)
                    .await?
                {
                    Some(id) => id,
                    None => {
                        println!(
                            "{} no exercise named `{}`",
                            "error:".red().bold(),
                            new_exercise
                        );
                        return Ok(());
                    }
                }
            };

            // Get new exercise name
            let new_exercise_name: String =
                sqlx::query_scalar("SELECT name FROM exercises WHERE id = ?")
                    .bind(&new_exercise_id)
                    .fetch_one(pool)
                    .await?;

            // Start a transaction
            let mut tx = pool.begin().await?;

            // Get the sets and reps info from the program_exercises for display only
            let (sets, reps) = sqlx::query_as::<_, (i32, Option<String>)>(
                "SELECT sets, reps FROM program_exercises WHERE program_block_id = ? AND exercise_id = ?"
            )
            .bind(&program_block_id)
            .bind(&new_exercise_id)  // Display info for the new exercise being swapped in
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or((3, None)); // Default to 3 sets with no specific reps for swapped exercises

            // Update the training_session_exercise record ONLY
            sqlx::query("UPDATE training_session_exercises SET exercise_id = ? WHERE id = ?")
                .bind(&new_exercise_id)
                .bind(&old_session_exercise_id)
                .execute(&mut *tx)
                .await?;

            // Commit the transaction
            tx.commit().await?;

            // Show success message
            println!(
                "{} swapped {} with {} ({} sets{})",
                "ok:".green().bold(),
                old_exercise_name.bold(),
                new_exercise_name.bold(),
                sets,
                reps.as_deref()
                    .map(|r| format!(" of {}", r))
                    .unwrap_or_default()
            );

            Ok(())
        }
    }
}

fn epley_1rm(weight: f32, reps: i32) -> f32 {
    if reps == 0 {
        0.0
    } else {
        weight * (1.0 + reps as f32 / 30.0)
    }
}
