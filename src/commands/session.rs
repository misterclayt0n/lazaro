use anyhow::Result;
use colored::Colorize;
use sqlx::SqlitePool;
use std::collections::HashMap;
use uuid::Uuid;
use chrono::NaiveDate;

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
                        String,
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
                        COALESCE(pe.sets, 2) as sets,
                        pe.reps,
                        e.current_pr_date,
                        e.estimated_one_rm,
                        (SELECT estimated_1rm FROM last_prs WHERE exercise_id = e.id),
                        (SELECT weight FROM last_prs WHERE exercise_id = e.id),
                        (SELECT reps FROM last_prs WHERE exercise_id = e.id),
                        pe.target_rpe,
                        pe.target_rm_percent,
                        pe.program_1rm,
                        seo.tse_id
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
                        _tse_id,
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
                        tse_id,
                    ),
                ) in exercises.iter().enumerate()
                {
                    let idx = format!("{}", i + 1).yellow();

                    // Get the best (highest 1RM) PR for this exercise
                    let (pr_weight, pr_reps, pr_1rm): (Option<f32>, Option<i32>, Option<f32>) =
                        sqlx::query_as(
                            r#"
                        SELECT weight, reps, estimated_1rm
                        FROM personal_records
                        WHERE exercise_id = ?
                        ORDER BY estimated_1rm DESC
                        LIMIT 1
                        "#,
                        )
                        .bind(ex_id)
                        .fetch_optional(pool)
                        .await?
                        .unwrap_or((None, None, None));

                    // Print exercise header with PR info
                    let pr_info = if let (Some(w), Some(r)) = (pr_weight, pr_reps) {
                        let one_rm = pr_1rm.unwrap_or_else(|| epley_1rm(w, r).round());
                        let actual_pr = format!("{}kg × {}", w, r).red().bold().to_string();
                        format!(" - PR: {} (1RM: {:.1}kg)", actual_pr, one_rm)
                    } else {
                        String::new()
                    };

                    println!("{} • {}{}", idx, ex_name.bold(), pr_info.dimmed());

                    // Print exercise note if it exists
                    let note: Option<String> = sqlx::query_scalar(
                        "SELECT notes FROM training_session_exercises WHERE id = ?",
                    )
                    .bind(&tse_id)
                    .fetch_optional(pool)
                    .await?;

                    if let Some(note) = note {
                        if note != "" {
                            println!("    {} {}", "NOTE:".blue().bold(), note);
                        }
                    }

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

                    // Get all logged sets for this exercise
                    let logged_sets_1_based_num = sqlx::query_as::<_, (i64, f32, i32, bool)>(
                        r#"
                        WITH set_numbers AS (
                            SELECT 
                                es.*,
                                ROW_NUMBER() OVER (PARTITION BY tse.id ORDER BY es.timestamp) as set_num -- 1-based
                            FROM exercise_sets es
                            JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
                            WHERE tse.exercise_id = ?
                            AND tse.training_session_id = ?
                        )
                        SELECT set_num, weight, reps, bodyweight
                        FROM set_numbers
                        ORDER BY set_num
                        "#,
                    )
                    .bind(ex_id)
                    .bind(&session_id)
                    .fetch_all(pool)
                    .await?;

                    // Convert to 0-based set numbers for internal processing
                    let logged_sets_0_based_num: Vec<(i64, f32, i32, bool)> = logged_sets_1_based_num
                        .into_iter()
                        .map(|(snum_1_based, w, r, b)| (snum_1_based - 1, w, r, b)) // Convert to 0-based set_num
                        .collect();

                    // If no sets are logged yet, show the program's sets (using 0-based set_num)
                    let sets_to_show = if logged_sets_0_based_num.is_empty() {
                        (0..*sets) // Iterate 0-based
                            .map(|i_0_based| (i_0_based as i64, 0.0, 0, false))
                            .collect::<Vec<_>>()
                    } else {
                        let mut all_sets = Vec::new();
                        // First add all sets up to the program's set count (using 0-based set_num)
                        for i_0_based in 0..*sets { // Iterate 0-based program sets
                            if let Some(set) = logged_sets_0_based_num
                                .iter()
                                .find(|(s_0_based, _, _, _)| *s_0_based == i_0_based as i64)
                            {
                                all_sets.push(*set); // s_0_based is already 0-based
                            } else {
                                all_sets.push((i_0_based as i64, 0.0, 0, false)); // Placeholder with 0-based set_num
                            }
                        }
                        // Then add any additional sets beyond the program's set count (using 0-based set_num)
                        // Additional sets are those with 0-based index >= program's set count
                        for set_to_add in logged_sets_0_based_num.iter().filter(|(s_0_based, _, _, _)| *s_0_based >= *sets as i64) {
                            // Avoid duplicating sets
                            let s_0_based_to_add = set_to_add.0;
                            if !all_sets.iter().any(|(added_s_0,_,_,_)| *added_s_0 == s_0_based_to_add) {
                                all_sets.push(*set_to_add);
                            }
                        }
                        all_sets.sort_by_key(|(s_0_based, _, _, _)| *s_0_based);
                        all_sets
                    };

                    // Display all sets
                    for (set_num_0_based_in_loop, weight, reps, bw) in sets_to_show {
                        let set_num_usize = set_num_0_based_in_loop as usize; // 0-based for array indexing
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

                        // Get previous set info from our pre-calculated list
                        let prev_info = if set_num_usize < prev_sets_info[i].len() {
                            &prev_sets_info[i][set_num_usize]
                        } else {
                            "" // Empty string for additional sets beyond program's set count
                        };
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
                        let set_num_str = format!("{}", set_num_0_based_in_loop + 1).yellow(); // Display as 1-based
                        let indent = " ".repeat(2);
                        let target_part = if target_reps.is_empty() {
                            String::new()
                        } else {
                            format!("{}{}", target_reps, target_info.dimmed())
                        };
                        let padding = " ".repeat(target_padding);

                        let current_info = if bw {
                            format!("bw × {}", reps)
                        } else if weight > 0.0 {
                            format!("{}kg × {}", weight, reps)
                        } else {
                            String::new()
                        };

                        // Print with explicit parts
                        println!(
                            " {} {} • {} {}{} | {}",
                            indent,
                            set_num_str,
                            target_part,
                            padding,
                            prev_column,
                            current_info
                        );
                    }
                    println!();
                }
            } else {
                println!("{} no active session", "error:".red().bold());
            }
        }

        SessionCmd::Edit {
            exercise,
            weight,
            reps,
            set,
            new,
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
            } else if new {
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

            // Get total number of sets for this exercise, including any additional sets
            let total_sets: i64 = sqlx::query_scalar::<_, i64>(
                r#"
                WITH program_sets AS (
                    SELECT sets
                    FROM program_exercises
                    WHERE exercise_id = ? AND program_block_id = (
                        SELECT program_block_id
                        FROM training_sessions
                        WHERE id = ?
                    )
                ),
                set_numbers AS (
                    SELECT 
                        ROW_NUMBER() OVER (ORDER BY timestamp) - 1 as set_num
                    FROM exercise_sets
                    WHERE session_exercise_id = ?
                ),
                additional_sets AS (
                    SELECT COUNT(*) as extra_sets
                    FROM set_numbers
                    WHERE set_num >= (SELECT sets FROM program_sets)
                )
                SELECT COALESCE((SELECT sets FROM program_sets), 2) + 
                       COALESCE((SELECT extra_sets FROM additional_sets), 0)
                "#,
            )
            .bind(&exercise_id)
            .bind(&session_id)
            .bind(&session_exercise_id)
            .fetch_one(pool)
            .await?;

            // Only check set limit if --new flag is not used
            if !new && set_index >= total_sets as usize {
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
                        ROW_NUMBER() OVER (PARTITION BY es.session_exercise_id ORDER BY es.timestamp) as set_num
                    FROM exercise_sets es
                    WHERE es.session_exercise_id = ?
                )
                SELECT id, timestamp
                FROM set_numbers
                WHERE set_num = ?
                "#,
            )
            .bind(&session_exercise_id)
            .bind((set_index + 1) as i64) // Query with 1-based set number
            .fetch_optional(&mut *tx)
            .await?;

            // If set exists, update it; otherwise create new
            if let Some((set_id, _)) = existing_set {
                // Update existing set
                sqlx::query(
                    r#"
                    UPDATE exercise_sets
                    SET weight = ?, reps = ?, bodyweight = ?
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
            let is_pr = if !is_bodyweight {
                let (pr_weight, pr_reps): (Option<f32>, Option<i32>) = sqlx::query_as(
                    r#"
                    SELECT weight, reps
                    FROM personal_records
                    WHERE exercise_id = ?
                    ORDER BY estimated_1rm DESC
                    LIMIT 1
                    "#,
                )
                .bind(&exercise_id)
                .fetch_optional(&mut *tx)
                .await?
                .unwrap_or((None, None));

                if let (Some(pr_weight), Some(pr_reps)) = (pr_weight, pr_reps) {
                    // For non-bodyweight exercises, compare weight × reps
                    let current_total = parsed_weight.unwrap_or(0.0) * reps as f32;
                    let pr_total = pr_weight * pr_reps as f32;
                    current_total > pr_total
                } else {
                    // No previous PR, so this is a PR
                    true
                }
            } else {
                // For bodyweight exercises, just compare reps
                let max_reps: Option<i32> = sqlx::query_scalar(
                    r#"
                    SELECT reps
                    FROM personal_records
                    WHERE exercise_id = ? AND bodyweight = 1
                    ORDER BY reps DESC
                    LIMIT 1
                    "#,
                )
                .bind(&exercise_id)
                .fetch_optional(&mut *tx)
                .await?;

                match max_reps {
                    Some(max_reps) => reps >= max_reps,
                    None => true, // If no previous PR, this is a PR
                }
            };

            if is_pr {
                // Calculate estimated 1RM
                let estimated_1rm = if is_bodyweight {
                    0.0 // For bodyweight exercises, we don't calculate 1RM
                } else {
                    epley_1rm(parsed_weight.unwrap_or(0.0), reps)
                };

                // Insert new PR
                sqlx::query(
                    r#"
                    INSERT INTO personal_records (
                        exercise_id,
                        weight,
                        reps,
                        estimated_1rm,
                        date
                    ) VALUES (?, ?, ?, ?, datetime('now'))
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

                // Update exercise's current PR date and estimated 1RM
                sqlx::query(
                    r#"
                    UPDATE exercises 
                    SET current_pr_date = datetime('now'),
                        estimated_one_rm = ?
                    WHERE id = ?
                    "#,
                )
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
                        let est_1rm = epley_1rm(*w, *reps);
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
                        ORDER BY estimated_1rm DESC
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
                        exercise_id,
                        weight,
                        reps,
                        estimated_1rm,
                        date
                    ) VALUES (?, ?, ?, ?, datetime('now'))
                    "#,
                )
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

            let (old_session_exercise_id, old_exercise_id, old_exercise_name) =
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

            // Get the original exercise's set count from the program
            let original_sets: i32 = sqlx::query_scalar(
                "SELECT COALESCE(pe.sets, 2) FROM program_exercises pe 
                 WHERE pe.program_block_id = ? AND pe.exercise_id = ?"
            )
            .bind(&program_block_id)
            .bind(&old_exercise_id)
            .fetch_optional(pool)
            .await?
            .unwrap_or(2); // Default to 2 sets if not found

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

            // Get the reps info from the program_exercises for display only
            let reps: Option<String> = sqlx::query_scalar(
                "SELECT reps FROM program_exercises WHERE program_block_id = ? AND exercise_id = ?"
            )
            .bind(&program_block_id)
            .bind(&new_exercise_id)  // Display info for the new exercise being swapped in
            .fetch_optional(&mut *tx)
            .await?;

            // Check if the new exercise already exists in program_exercises
            let existing_program_exercise: Option<String> = sqlx::query_scalar(
                "SELECT id FROM program_exercises WHERE program_block_id = ? AND exercise_id = ?"
            )
            .bind(&program_block_id)
            .bind(&new_exercise_id)
            .fetch_optional(&mut *tx)
            .await?;

            if let Some(pe_id) = existing_program_exercise {
                // Update existing program exercise to use the original exercise's set count
                sqlx::query(
                    "UPDATE program_exercises SET sets = ? WHERE id = ?"
                )
                .bind(original_sets)
                .bind(pe_id)
                .execute(&mut *tx)
                .await?;
            } else {
                // Create a new program exercise with the original exercise's set count
                sqlx::query(
                    "INSERT INTO program_exercises (id, program_block_id, exercise_id, sets, order_index) 
                     VALUES (?, ?, ?, ?, 999)"
                )
                .bind(Uuid::new_v4().to_string())
                .bind(&program_block_id)
                .bind(&new_exercise_id)
                .bind(original_sets)
                .execute(&mut *tx)
                .await?;
            }

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
                original_sets,
                reps.as_deref()
                    .map(|r| format!(" of {}", r))
                    .unwrap_or_default()
            );
        }

        SessionCmd::AddEx { exercise, sets } => {
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

            // Resolve the exercise (by index or name)
            let exercise_id: String = if let Ok(idx) = exercise.parse::<i64>() {
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

            // Get exercise name for display
            let exercise_name: String =
                sqlx::query_scalar("SELECT name FROM exercises WHERE id = ?")
                    .bind(&exercise_id)
                    .fetch_one(pool)
                    .await?;

            // Start a transaction
            let mut tx = pool.begin().await?;

            // Create a new session exercise record
            let session_exercise_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO training_session_exercises (id, training_session_id, exercise_id) VALUES (?, ?, ?)",
            )
            .bind(&session_exercise_id)
            .bind(&session_id)
            .bind(&exercise_id)
            .execute(&mut *tx)
            .await?;

            // Commit the transaction
            tx.commit().await?;

            // Show success message
            println!(
                "{} added {} ({} sets)",
                "ok:".green().bold(),
                exercise_name.bold(),
                sets
            );
        }

        SessionCmd::Note { exercise, note } => {
            let session_id: String = sqlx::query_scalar("SELECT id FROM current_session")
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| anyhow::anyhow!("no active session"))?;

            let tse_id: String = sqlx::query_scalar(
                r#"
                WITH ordered AS (
                    SELECT tse.id,
                           ROW_NUMBER() OVER (ORDER BY tse.rowid) AS rn
                    FROM training_session_exercises tse
                    WHERE tse.training_session_id = ?
                )
                SELECT id FROM ordered WHERE rn = ?
                "#,
            )
            .bind(&session_id)
            .bind(exercise as i64)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| anyhow::anyhow!(format!("no exercise at index {}", exercise)))?;

            sqlx::query("UPDATE training_session_exercises SET notes = ? WHERE id = ?")
                .bind(note.trim()) // trim is optional but tidy
                .bind(&tse_id)
                .execute(pool)
                .await?;

            println!(
                "{} note saved for exercise {}",
                "ok:".green().bold(),
                exercise
            );
        }

        SessionCmd::Log { date } => {
            // Parse the date string (format: DD-MM-YYYY)
            let date = NaiveDate::parse_from_str(&date, "%d-%m-%Y")?;
            
            // Get session info for the given date
            let session: Option<(String, String, String, String)> = sqlx::query_as(
                r#"
                SELECT ts.id, ts.start_time, pb.name, COALESCE(pb.description, '')
                FROM training_sessions ts
                JOIN program_blocks pb ON pb.id = ts.program_block_id
                WHERE date(ts.start_time) = date(?)
                AND ts.end_time IS NOT NULL
                LIMIT 1
                "#,
            )
            .bind(date.format("%Y-%m-%d").to_string())
            .fetch_optional(pool)
            .await?;

            let (session_id, start_time, block_name, block_desc) = match session {
                Some(s) => s,
                None => {
                    println!("{} no completed session found for {}", "error:".red().bold(), date.format("%d-%m-%Y"));
                    return Ok(());
                }
            };

            // Calculate session duration
            let duration = sqlx::query_scalar::<_, String>(
                r#"
                SELECT strftime('%H:%M:%S', 
                    strftime('%s', end_time) - strftime('%s', start_time) || ' seconds', 
                    'unixepoch'
                )
                FROM training_sessions
                WHERE id = ?
                "#,
            )
            .bind(&session_id)
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
                    String,
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
                    COALESCE(pe.sets, 2) as sets,
                    pe.reps,
                    e.current_pr_date,
                    e.estimated_one_rm,
                    (SELECT estimated_1rm FROM last_prs WHERE exercise_id = e.id),
                    (SELECT weight FROM last_prs WHERE exercise_id = e.id),
                    (SELECT reps FROM last_prs WHERE exercise_id = e.id),
                    pe.target_rpe,
                    pe.target_rm_percent,
                    pe.program_1rm,
                    seo.tse_id
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
                    _tse_id,
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
                    tse_id,
                ),
            ) in exercises.iter().enumerate()
            {
                let idx = format!("{}", i + 1).yellow();

                // Get the best (highest 1RM) PR for this exercise
                let (pr_weight, pr_reps, pr_1rm): (Option<f32>, Option<i32>, Option<f32>) =
                    sqlx::query_as(
                        r#"
                        SELECT weight, reps, estimated_1rm
                        FROM personal_records
                        WHERE exercise_id = ?
                        ORDER BY estimated_1rm DESC
                        LIMIT 1
                        "#,
                    )
                    .bind(ex_id)
                    .fetch_optional(pool)
                    .await?
                    .unwrap_or((None, None, None));

                // Print exercise header with PR info
                let pr_info = if let (Some(w), Some(r)) = (pr_weight, pr_reps) {
                    let one_rm = pr_1rm.unwrap_or_else(|| epley_1rm(w, r).round());
                    let actual_pr = format!("{}kg × {}", w, r).red().bold().to_string();
                    format!(" - PR: {} (1RM: {:.1}kg)", actual_pr, one_rm)
                } else {
                    String::new()
                };

                println!("{} • {}{}", idx, ex_name.bold(), pr_info.dimmed());

                // Print exercise note if it exists
                let note: Option<String> = sqlx::query_scalar(
                    "SELECT notes FROM training_session_exercises WHERE id = ?",
                )
                .bind(&tse_id)
                .fetch_optional(pool)
                .await?;

                if let Some(note) = note {
                    if note != "" {
                        println!("    {} {}", "NOTE:".blue().bold(), note);
                    }
                }

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

                // Get all logged sets for this exercise
                let logged_sets_1_based_num = sqlx::query_as::<_, (i64, f32, i32, bool)>(
                    r#"
                    WITH set_numbers AS (
                        SELECT 
                            es.*,
                            ROW_NUMBER() OVER (PARTITION BY tse.id ORDER BY es.timestamp) as set_num -- 1-based
                        FROM exercise_sets es
                        JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
                        WHERE tse.exercise_id = ?
                        AND tse.training_session_id = ?
                    )
                    SELECT set_num, weight, reps, bodyweight
                    FROM set_numbers
                    ORDER BY set_num
                    "#,
                )
                .bind(ex_id)
                .bind(&session_id)
                .fetch_all(pool)
                .await?;

                // Convert to 0-based set numbers for internal processing
                let logged_sets_0_based_num: Vec<(i64, f32, i32, bool)> = logged_sets_1_based_num
                    .into_iter()
                    .map(|(snum_1_based, w, r, b)| (snum_1_based - 1, w, r, b)) // Convert to 0-based set_num
                    .collect();

                // If no sets are logged yet, show the program's sets (using 0-based set_num)
                let sets_to_show = if logged_sets_0_based_num.is_empty() {
                    (0..*sets) // Iterate 0-based
                        .map(|i_0_based| (i_0_based as i64, 0.0, 0, false))
                        .collect::<Vec<_>>()
                } else {
                    let mut all_sets = Vec::new();
                    // First add all sets up to the program's set count (using 0-based set_num)
                    for i_0_based in 0..*sets { // Iterate 0-based program sets
                        if let Some(set) = logged_sets_0_based_num
                            .iter()
                            .find(|(s_0_based, _, _, _)| *s_0_based == i_0_based as i64)
                        {
                            all_sets.push(*set); // s_0_based is already 0-based
                        } else {
                            all_sets.push((i_0_based as i64, 0.0, 0, false)); // Placeholder with 0-based set_num
                        }
                    }
                    // Then add any additional sets beyond the program's set count (using 0-based set_num)
                    // Additional sets are those with 0-based index >= program's set count
                    for set_to_add in logged_sets_0_based_num.iter().filter(|(s_0_based, _, _, _)| *s_0_based >= *sets as i64) {
                        // Avoid duplicating sets
                        let s_0_based_to_add = set_to_add.0;
                        if !all_sets.iter().any(|(added_s_0,_,_,_)| *added_s_0 == s_0_based_to_add) {
                            all_sets.push(*set_to_add);
                        }
                    }
                    all_sets.sort_by_key(|(s_0_based, _, _, _)| *s_0_based);
                    all_sets
                };

                // Display all sets
                for (set_num_0_based_in_loop, weight, reps, bw) in sets_to_show {
                    let set_num_usize = set_num_0_based_in_loop as usize; // 0-based for array indexing
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

                    // Get previous set info from our pre-calculated list
                    let prev_info = if set_num_usize < prev_sets_info[i].len() {
                        &prev_sets_info[i][set_num_usize]
                    } else {
                        "" // Empty string for additional sets beyond program's set count
                    };
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
                    let set_num_str = format!("{}", set_num_0_based_in_loop + 1).yellow(); // Display as 1-based
                    let indent = " ".repeat(2);
                    let target_part = if target_reps.is_empty() {
                        String::new()
                    } else {
                        format!("{}{}", target_reps, target_info.dimmed())
                    };
                    let padding = " ".repeat(target_padding);

                    let current_info = if bw {
                        format!("bw × {}", reps)
                    } else if weight > 0.0 {
                        format!("{}kg × {}", weight, reps)
                    } else {
                        String::new()
                    };

                    // Print with explicit parts
                    println!(
                        " {} {} • {} {}{} | {}",
                        indent,
                        set_num_str,
                        target_part,
                        padding,
                        prev_column,
                        current_info
                    );
                }
                println!();
            }
        }
    }

    Ok(())
}

fn epley_1rm(weight: f32, reps: i32) -> f32 {
    if reps == 0 {
        0.0
    } else {
        weight * (1.0 + reps as f32 / 30.0)
    }
}

