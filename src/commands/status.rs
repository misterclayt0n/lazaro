use anyhow::Result;
use chrono::{DateTime, Utc};
use colored::Colorize;
use sqlx::SqlitePool;

fn create_ascii_graph(data: &[(DateTime<Utc>, f32)], width: usize, height: usize, title: &str) -> Vec<String> {
    if data.is_empty() {
        return vec!["No data available".to_string()];
    }

    let min_value = data.iter().map(|(_, v)| *v).fold(f32::INFINITY, f32::min);
    let max_value = data.iter().map(|(_, v)| *v).fold(f32::NEG_INFINITY, f32::max);
    let range = max_value - min_value;
    
    if range == 0.0 {
        return vec!["No variation in data".to_string()];
    }
    
    // Create the graph grid
    let mut grid = vec![vec![' '; width]; height];
    
    // Draw the data points and lines
    for i in 0..data.len() {
        let (_, value) = data[i];
        let x = (i as f32 / (data.len() - 1) as f32 * (width - 1) as f32) as usize;
        let y = ((value - min_value) / range * (height - 1) as f32) as usize;
        let y = height - 1 - y; // Flip the y-axis
        
        if y < height && x < width {
            grid[y][x] = '●';
        }

        // Draw connecting lines
        if i > 0 {
            let prev_x = ((i - 1) as f32 / (data.len() - 1) as f32 * (width - 1) as f32) as usize;
            let prev_y = ((data[i-1].1 - min_value) / range * (height - 1) as f32) as usize;
            let prev_y = height - 1 - prev_y;
            
            // Draw line between points
            let dx = x as isize - prev_x as isize;
            let dy = y as isize - prev_y as isize;
            let steps = dx.abs().max(dy.abs());
            
            for step in 1..steps {
                let px = prev_x as isize + (dx * step / steps);
                let py = prev_y as isize + (dy * step / steps);
                
                if px >= 0 && px < width as isize && py >= 0 && py < height as isize {
                    let px = px as usize;
                    let py = py as usize;
                    if grid[py][px] == ' ' {
                        grid[py][px] = '·';
                    }
                }
            }
        }
    }
    
    // Convert grid to strings with y-axis labels
    let mut result = Vec::new();
    let step = range / (height - 1) as f32;
    
    // Add title
    result.push(format!("\n{} {}", title.bold(), "Progression"));
    result.push("─".repeat(width + 7));
    
    // Add the graph with y-axis labels
    for (i, row) in grid.iter().enumerate() {
        let value = min_value + step * (height - 1 - i) as f32;
        let label = format!("{:4.0} │{}", value, row.iter().collect::<String>());
        result.push(label);
    }
    
    // Add x-axis
    result.push(format!("     └{}", "─".repeat(width)));
    
    // Add date labels
    if !data.is_empty() {
        let first_date = data.first().unwrap().0.format("%Y-%m-%d").to_string();
        let last_date = data.last().unwrap().0.format("%Y-%m-%d").to_string();
        result.push(format!("     {}  {}", first_date, last_date));
    }
    
    result
}

async fn show_global_progression(pool: &SqlitePool, weeks: u32, show_graph: bool) -> Result<()> {
    // Get weekly tonnage data
    let tonnage_data: Vec<(String, f64)> = sqlx::query_as(
        r#"
        WITH weekly_data AS (
            SELECT 
                date(es.timestamp, 'weekday 1', '-6 days') as week_start,
                SUM(CAST(es.weight AS REAL) * CAST(es.reps AS INTEGER)) as tonnage
            FROM exercise_sets es
            JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
            JOIN training_sessions ts ON ts.id = tse.training_session_id
            WHERE es.timestamp >= datetime('now', '-' || ? || ' days')
            AND ts.end_time IS NOT NULL
            AND es.weight > 0
            GROUP BY week_start
            ORDER BY week_start
        )
        SELECT week_start, tonnage FROM weekly_data
        "#,
    )
    .bind(weeks * 7)
    .fetch_all(pool)
    .await?;

    // Get global stats
    let (total_tonnage, total_sets, total_sessions, active_exercises): (f64, i64, i64, i64) = sqlx::query_as(
        r#"
        WITH period_data AS (
            SELECT 
                es.weight,
                es.reps,
                tse.exercise_id,
                ts.id as session_id
            FROM exercise_sets es
            JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
            JOIN training_sessions ts ON ts.id = tse.training_session_id
            WHERE es.timestamp >= datetime('now', '-' || ? || ' days')
            AND ts.end_time IS NOT NULL
        )
        SELECT 
            COALESCE(SUM(CAST(weight AS REAL) * CAST(reps AS INTEGER)), 0) as total_tonnage,
            CAST(COUNT(*) AS INTEGER) as total_sets,
            CAST(COUNT(DISTINCT session_id) AS INTEGER) as total_sessions,
            CAST(COUNT(DISTINCT exercise_id) AS INTEGER) as active_exercises
        FROM period_data
        "#,
    )
    .bind(weeks * 7)
    .fetch_one(pool)
    .await?;

    // Get PR progression data for the period
    let pr_progression_data: Vec<(String, f32)> = sqlx::query_as(
        r#"
        WITH weekly_pr_data AS (
            SELECT 
                date(es.timestamp, 'weekday 1', '-6 days') as week_start,
                tse.exercise_id,
                MAX(CAST(es.weight AS REAL) * (1 + CAST(es.reps AS REAL) / 30)) as week_best_1rm
            FROM exercise_sets es
            JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
            JOIN training_sessions ts ON ts.id = tse.training_session_id
            WHERE es.timestamp >= datetime('now', '-' || ? || ' days')
            AND ts.end_time IS NOT NULL
            AND es.weight > 0
            GROUP BY week_start, tse.exercise_id
        ),
        baseline_prs AS (
            SELECT 
                exercise_id,
                MAX(CAST(weight AS REAL) * (1 + CAST(reps AS REAL) / 30)) as baseline_1rm
            FROM exercise_sets es
            JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
            JOIN training_sessions ts ON ts.id = tse.training_session_id
            WHERE es.timestamp < datetime('now', '-' || ? || ' days')
            AND ts.end_time IS NOT NULL
            AND es.weight > 0
            GROUP BY exercise_id
        ),
        weekly_improvements AS (
            SELECT 
                wpd.week_start,
                AVG(CASE 
                    WHEN bp.baseline_1rm > 0 THEN 
                        ((wpd.week_best_1rm - bp.baseline_1rm) / bp.baseline_1rm) * 100
                    ELSE 0 
                END) as avg_improvement_percent
            FROM weekly_pr_data wpd
            JOIN baseline_prs bp ON bp.exercise_id = wpd.exercise_id
            GROUP BY wpd.week_start
            ORDER BY wpd.week_start
        )
        SELECT week_start, avg_improvement_percent FROM weekly_improvements
        "#,
    )
    .bind(weeks * 7)
    .bind(weeks * 7)
    .fetch_all(pool)
    .await?;

    // Calculate percentage improvements
    let (early_tonnage, late_tonnage, early_sets, late_sets) = if tonnage_data.len() >= 4 {
        // Compare first 25% of weeks to last 25% of weeks
        let quarter_point = tonnage_data.len() / 4;
        let early_weeks = &tonnage_data[0..quarter_point];
        let late_weeks = &tonnage_data[tonnage_data.len() - quarter_point..];
        
        let early_avg_tonnage = early_weeks.iter().map(|(_, t)| *t).sum::<f64>() / early_weeks.len() as f64;
        let late_avg_tonnage = late_weeks.iter().map(|(_, t)| *t).sum::<f64>() / late_weeks.len() as f64;
        
        // Get corresponding sets data for the same periods
        let early_sets_data: Vec<(String, i64)> = sqlx::query_as(
            r#"
            WITH weekly_data AS (
                SELECT 
                    date(es.timestamp, 'weekday 1', '-6 days') as week_start,
                    COUNT(*) as total_sets
                FROM exercise_sets es
                JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
                JOIN training_sessions ts ON ts.id = tse.training_session_id
                WHERE es.timestamp >= datetime('now', '-' || ? || ' days')
                AND es.timestamp < datetime('now', '-' || ? || ' days')
                AND ts.end_time IS NOT NULL
                GROUP BY week_start
                ORDER BY week_start
            )
            SELECT week_start, total_sets FROM weekly_data
            "#,
        )
        .bind(weeks * 7)
        .bind((weeks * 3 / 4) * 7)  // Early period: from start to 3/4 point
        .fetch_all(pool)
        .await?;

        let late_sets_data: Vec<(String, i64)> = sqlx::query_as(
            r#"
            WITH weekly_data AS (
                SELECT 
                    date(es.timestamp, 'weekday 1', '-6 days') as week_start,
                    COUNT(*) as total_sets
                FROM exercise_sets es
                JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
                JOIN training_sessions ts ON ts.id = tse.training_session_id
                WHERE es.timestamp >= datetime('now', '-' || ? || ' days')
                AND ts.end_time IS NOT NULL
                GROUP BY week_start
                ORDER BY week_start
            )
            SELECT week_start, total_sets FROM weekly_data
            "#,
        )
        .bind((weeks / 4) * 7)  // Late period: last quarter
        .fetch_all(pool)
        .await?;

        let early_avg_sets = if !early_sets_data.is_empty() {
            early_sets_data.iter().map(|(_, s)| *s as f64).sum::<f64>() / early_sets_data.len() as f64
        } else {
            0.0
        };
        
        let late_avg_sets = if !late_sets_data.is_empty() {
            late_sets_data.iter().map(|(_, s)| *s as f64).sum::<f64>() / late_sets_data.len() as f64
        } else {
            0.0
        };

        (early_avg_tonnage, late_avg_tonnage, early_avg_sets, late_avg_sets)
    } else {
        (0.0, 0.0, 0.0, 0.0)
    };

    // Calculate PR improvement statistics
    let (pr_improvement_percent, exercises_with_prs) = if !pr_progression_data.is_empty() {
        let avg_improvement = pr_progression_data.iter()
            .map(|(_, improvement)| *improvement as f64)
            .sum::<f64>() / pr_progression_data.len() as f64;
        (avg_improvement, pr_progression_data.len())
    } else {
        (0.0, 0)
    };

    println!("{} ({} weeks)", "Global Training Status".cyan().bold(), weeks);
    println!();

    // Print summary stats
    println!("{}: {:.0} kg", "Total tonnage".cyan().bold(), total_tonnage);
    println!("{}: {} sets", "Total volume".cyan().bold(), total_sets);
    println!("{}: {} sessions", "Training sessions".cyan().bold(), total_sessions);
    println!("{}: {} exercises", "Active exercises".cyan().bold(), active_exercises);
    
    if total_sessions > 0 {
        let avg_frequency = total_sessions as f64 / (weeks as f64);
        let avg_tonnage_per_session = total_tonnage / total_sessions as f64;
        println!("{}: {:.1} sessions/week", "Avg frequency".cyan().bold(), avg_frequency);
        println!("{}: {:.0} kg/session", "Avg tonnage/session".cyan().bold(), avg_tonnage_per_session);
    }

    // Print percentage improvements
    if early_tonnage > 0.0 && late_tonnage > 0.0 {
        let tonnage_improvement = ((late_tonnage - early_tonnage) / early_tonnage) * 100.0;
        let sets_improvement = if early_sets > 0.0 {
            ((late_sets - early_sets) / early_sets) * 100.0
        } else {
            0.0
        };

        println!();
        println!("{}", "Volume trends over period:".cyan().bold());
        
        let tonnage_color = if tonnage_improvement > 0.0 { "▲".green() } else { "▼".red() };
        let sets_color = if sets_improvement > 0.0 { "▲".green() } else { "▼".red() };
        
        println!("  {} Weekly tonnage: {:+.1}% ({:.0} → {:.0} kg)", 
                tonnage_color, tonnage_improvement, early_tonnage, late_tonnage);
        println!("  {} Weekly volume: {:+.1}% ({:.0} → {:.0} sets)", 
                sets_color, sets_improvement, early_sets, late_sets);
    }

    // Print PR improvement statistics
    if exercises_with_prs > 0 {
        println!();
        println!("{}", "Strength progression:".cyan().bold());
        
        let pr_color = if pr_improvement_percent > 0.0 { "▲".green() } else { "▼".red() };
        println!("  {} Average PR improvement: {:+.1}% across {} exercises", 
                pr_color, pr_improvement_percent, exercises_with_prs);
    }

    if show_graph {
        if !tonnage_data.is_empty() {
            // Convert tonnage data to graph format
            let tonnage_graph_data: Vec<(DateTime<Utc>, f32)> = tonnage_data
                .into_iter()
                .filter_map(|(week_start, tonnage)| {
                    // Parse the date and convert to DateTime<Utc>
                    if let Ok(naive_date) = chrono::NaiveDate::parse_from_str(&week_start, "%Y-%m-%d") {
                        let naive_datetime = naive_date.and_hms_opt(0, 0, 0)?;
                        Some((naive_datetime.and_utc(), tonnage as f32))
                    } else {
                        None
                    }
                })
                .collect();

            if !tonnage_graph_data.is_empty() {
                // Get terminal size
                let (term_width, term_height) = term_size::dimensions().unwrap_or((80, 24));
                let width = (term_width / 2).min(60);
                let height = (term_height / 2).min(15);

                let graph = create_ascii_graph(&tonnage_graph_data, width, height, "Weekly Tonnage");
                for line in graph {
                    println!("{}", line);
                }
            }
        }

        // Add PR progression graph
        if !pr_progression_data.is_empty() {
            let pr_graph_data: Vec<(DateTime<Utc>, f32)> = pr_progression_data
                .into_iter()
                .filter_map(|(week_start, improvement)| {
                    if let Ok(naive_date) = chrono::NaiveDate::parse_from_str(&week_start, "%Y-%m-%d") {
                        let naive_datetime = naive_date.and_hms_opt(0, 0, 0)?;
                        Some((naive_datetime.and_utc(), improvement))
                    } else {
                        None
                    }
                })
                .collect();

            if !pr_graph_data.is_empty() {
                let (term_width, term_height) = term_size::dimensions().unwrap_or((80, 24));
                let width = (term_width / 2).min(60);
                let height = (term_height / 2).min(15);

                let graph = create_ascii_graph(&pr_graph_data, width, height, "PR Improvement (%)");
                for line in graph {
                    println!("{}", line);
                }
            }
        }
    }

    Ok(())
}

async fn show_muscle_progression(pool: &SqlitePool, muscle: &str, weeks: u32, show_graph: bool) -> Result<()> {
    // Get weekly volume data for the muscle group
    let muscle_volume_data: Vec<(String, i64)> = sqlx::query_as(
        r#"
        WITH weekly_muscle_data AS (
            SELECT 
                date(es.timestamp, 'weekday 1', '-6 days') as week_start,
                COUNT(*) as weekly_sets
            FROM exercise_sets es
            JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
            JOIN training_sessions ts ON ts.id = tse.training_session_id
            JOIN exercises e ON e.id = tse.exercise_id
            WHERE es.timestamp >= datetime('now', '-' || ? || ' days')
            AND ts.end_time IS NOT NULL
            AND e.primary_muscle = ?
            GROUP BY week_start
            ORDER BY week_start
        )
        SELECT week_start, weekly_sets FROM weekly_muscle_data
        "#,
    )
    .bind(weeks * 7)
    .bind(muscle)
    .fetch_all(pool)
    .await?;

    // Get muscle-specific stats
    let (muscle_tonnage, muscle_sets, active_exercises): (f64, i64, i64) = sqlx::query_as(
        r#"
        WITH period_data AS (
            SELECT 
                es.weight,
                es.reps,
                tse.exercise_id
            FROM exercise_sets es
            JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
            JOIN training_sessions ts ON ts.id = tse.training_session_id
            JOIN exercises e ON e.id = tse.exercise_id
            WHERE es.timestamp >= datetime('now', '-' || ? || ' days')
            AND ts.end_time IS NOT NULL
            AND e.primary_muscle = ?
        )
        SELECT 
            COALESCE(SUM(CAST(weight AS REAL) * CAST(reps AS INTEGER)), 0) as tonnage,
            CAST(COUNT(*) AS INTEGER) as sets,
            CAST(COUNT(DISTINCT exercise_id) AS INTEGER) as exercises
        FROM period_data
        "#,
    )
    .bind(weeks * 7)
    .bind(muscle)
    .fetch_one(pool)
    .await?;

    // Get PR progression data for this muscle group
    let pr_progression_data: Vec<(String, f32)> = sqlx::query_as(
        r#"
        WITH weekly_pr_data AS (
            SELECT 
                date(es.timestamp, 'weekday 1', '-6 days') as week_start,
                tse.exercise_id,
                MAX(CAST(es.weight AS REAL) * (1 + CAST(es.reps AS REAL) / 30)) as week_best_1rm
            FROM exercise_sets es
            JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
            JOIN training_sessions ts ON ts.id = tse.training_session_id
            JOIN exercises e ON e.id = tse.exercise_id
            WHERE es.timestamp >= datetime('now', '-' || ? || ' days')
            AND ts.end_time IS NOT NULL
            AND e.primary_muscle = ?
            AND es.weight > 0
            GROUP BY week_start, tse.exercise_id
        ),
        baseline_prs AS (
            SELECT 
                exercise_id,
                MAX(CAST(weight AS REAL) * (1 + CAST(reps AS REAL) / 30)) as baseline_1rm
            FROM exercise_sets es
            JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
            JOIN training_sessions ts ON ts.id = tse.training_session_id
            JOIN exercises e ON e.id = tse.exercise_id
            WHERE es.timestamp < datetime('now', '-' || ? || ' days')
            AND ts.end_time IS NOT NULL
            AND e.primary_muscle = ?
            AND es.weight > 0
            GROUP BY exercise_id
        ),
        weekly_improvements AS (
            SELECT 
                wpd.week_start,
                AVG(CASE 
                    WHEN bp.baseline_1rm > 0 THEN 
                        ((wpd.week_best_1rm - bp.baseline_1rm) / bp.baseline_1rm) * 100
                    ELSE 0 
                END) as avg_improvement_percent
            FROM weekly_pr_data wpd
            JOIN baseline_prs bp ON bp.exercise_id = wpd.exercise_id
            GROUP BY wpd.week_start
            ORDER BY wpd.week_start
        )
        SELECT week_start, avg_improvement_percent FROM weekly_improvements
        "#,
    )
    .bind(weeks * 7)
    .bind(muscle)
    .bind(weeks * 7)
    .bind(muscle)
    .fetch_all(pool)
    .await?;

    // Get top exercises for this muscle
    let top_exercises: Vec<(String, f64, f32)> = sqlx::query_as(
        r#"
        SELECT 
            e.name,
            COALESCE(SUM(CAST(es.weight AS REAL) * CAST(es.reps AS INTEGER)), 0) as tonnage,
            MAX(CAST(es.weight AS REAL) * (1 + CAST(es.reps AS REAL) / 30)) as best_1rm
        FROM exercise_sets es
        JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
        JOIN training_sessions ts ON ts.id = tse.training_session_id
        JOIN exercises e ON e.id = tse.exercise_id
        WHERE es.timestamp >= datetime('now', '-' || ? || ' days')
        AND ts.end_time IS NOT NULL
        AND e.primary_muscle = ?
        AND es.weight > 0
        GROUP BY e.id, e.name
        ORDER BY tonnage DESC
        LIMIT 5
        "#,
    )
    .bind(weeks * 7)
    .bind(muscle)
    .fetch_all(pool)
    .await?;

    if muscle_tonnage == 0.0 {
        println!("{} No training data found for muscle group: {}", "warning:".yellow().bold(), muscle);
        return Ok(());
    }

    // Calculate percentage improvements for muscle volume
    let (early_volume, late_volume) = if muscle_volume_data.len() >= 4 {
        let quarter_point = muscle_volume_data.len() / 4;
        let early_weeks = &muscle_volume_data[0..quarter_point];
        let late_weeks = &muscle_volume_data[muscle_volume_data.len() - quarter_point..];
        
        let early_avg_volume = early_weeks.iter().map(|(_, s)| *s as f64).sum::<f64>() / early_weeks.len() as f64;
        let late_avg_volume = late_weeks.iter().map(|(_, s)| *s as f64).sum::<f64>() / late_weeks.len() as f64;
        
        (early_avg_volume, late_avg_volume)
    } else {
        (0.0, 0.0)
    };

    // Calculate PR improvement statistics for this muscle
    let (pr_improvement_percent, exercises_with_prs) = if !pr_progression_data.is_empty() {
        let avg_improvement = pr_progression_data.iter()
            .map(|(_, improvement)| *improvement as f64)
            .sum::<f64>() / pr_progression_data.len() as f64;
        (avg_improvement, pr_progression_data.len())
    } else {
        (0.0, 0)
    };

    println!("{} {} ({} weeks)", "Muscle Group Progress:".cyan().bold(), muscle.bold(), weeks);
    println!();

    // Print muscle-specific stats
    println!("{}: {:.0} kg", "Total tonnage".cyan().bold(), muscle_tonnage);
    println!("{}: {} sets", "Total volume".cyan().bold(), muscle_sets);
    println!("{}: {} exercises", "Active exercises".cyan().bold(), active_exercises);

    // Print percentage improvement for muscle volume
    if early_volume > 0.0 && late_volume > 0.0 {
        let volume_improvement = ((late_volume - early_volume) / early_volume) * 100.0;
        
        println!();
        println!("{}", "Volume trends over period:".cyan().bold());
        
        let volume_color = if volume_improvement > 0.0 { "▲".green() } else { "▼".red() };
        println!("  {} Weekly volume: {:+.1}% ({:.0} → {:.0} sets/week)", 
                volume_color, volume_improvement, early_volume, late_volume);
    }

    // Print PR improvement statistics
    if exercises_with_prs > 0 {
        println!();
        println!("{}", "Strength progression:".cyan().bold());
        
        let pr_color = if pr_improvement_percent > 0.0 { "▲".green() } else { "▼".red() };
        println!("  {} Average PR improvement: {:+.1}% across {} exercises", 
                pr_color, pr_improvement_percent, exercises_with_prs);
    }

    println!();
    println!("{}", "Top exercises by tonnage:".cyan().bold());
    for (name, tonnage, best_1rm) in top_exercises {
        println!("  {} — {:.0} kg tonnage, {:.0} kg best 1RM", name.bold(), tonnage, best_1rm);
    }

    if show_graph {
        if !muscle_volume_data.is_empty() {
            // Convert muscle volume data to graph format
            let muscle_graph_data: Vec<(DateTime<Utc>, f32)> = muscle_volume_data
                .into_iter()
                .filter_map(|(week_start, volume)| {
                    if let Ok(naive_date) = chrono::NaiveDate::parse_from_str(&week_start, "%Y-%m-%d") {
                        let naive_datetime = naive_date.and_hms_opt(0, 0, 0)?;
                        Some((naive_datetime.and_utc(), volume as f32))
                    } else {
                        None
                    }
                })
                .collect();

            if !muscle_graph_data.is_empty() {
                // Get terminal size
                let (term_width, term_height) = term_size::dimensions().unwrap_or((80, 24));
                let width = (term_width / 2).min(60);
                let height = (term_height / 2).min(15);

                let title = format!("{} Weekly Volume (sets)", muscle);
                let graph = create_ascii_graph(&muscle_graph_data, width, height, &title);
                for line in graph {
                    println!("{}", line);
                }
            }
        }

        // Add PR progression graph for muscle
        if !pr_progression_data.is_empty() {
            let pr_graph_data: Vec<(DateTime<Utc>, f32)> = pr_progression_data
                .into_iter()
                .filter_map(|(week_start, improvement)| {
                    if let Ok(naive_date) = chrono::NaiveDate::parse_from_str(&week_start, "%Y-%m-%d") {
                        let naive_datetime = naive_date.and_hms_opt(0, 0, 0)?;
                        Some((naive_datetime.and_utc(), improvement))
                    } else {
                        None
                    }
                })
                .collect();

            if !pr_graph_data.is_empty() {
                let (term_width, term_height) = term_size::dimensions().unwrap_or((80, 24));
                let width = (term_width / 2).min(60);
                let height = (term_height / 2).min(15);

                let title = format!("{} PR Improvement (%)", muscle);
                let graph = create_ascii_graph(&pr_graph_data, width, height, &title);
                for line in graph {
                    println!("{}", line);
                }
            }
        }
    }

    Ok(())
}

pub async fn handle_status(muscle: Option<String>, weeks: u32, graph: bool, pool: &SqlitePool) -> Result<()> {
    match muscle {
        Some(muscle_name) => show_muscle_progression(pool, &muscle_name, weeks, graph).await,
        None => show_global_progression(pool, weeks, graph).await,
    }
} 