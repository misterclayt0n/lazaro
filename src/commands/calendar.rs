use anyhow::Result;
use chrono::{Datelike, NaiveDate, DateTime, Utc};
use colored::Colorize;
use sqlx::SqlitePool;

pub async fn handle(pool: &SqlitePool, year: Option<i32>, month: Option<u32>) -> Result<()> {
    // Get current date if year/month not specified
    let now = chrono::Local::now();
    let year = year.unwrap_or(now.year());
    let month = month.unwrap_or(now.month());

    // Validate month
    if month < 1 || month > 12 {
        println!("{} month must be between 1 and 12", "error:".red().bold());
        return Ok(());
    }

    // Get first and last day of the month
    let first_day = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let last_day = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
    }.pred_opt().unwrap();

    // Get all sessions in the month
    let sessions = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, String, String)>(
        r#"
        SELECT ts.id, ts.start_time, ts.end_time, ts.notes, p.name as program_name, pb.name as block_name
        FROM training_sessions ts
        JOIN program_blocks pb ON pb.id = ts.program_block_id
        JOIN programs p ON p.id = pb.program_id
        WHERE ts.start_time >= ? AND ts.start_time < ?
        ORDER BY ts.start_time
        "#,
    )
    .bind(first_day.and_hms_opt(0, 0, 0).unwrap().format("%Y-%m-%d %H:%M:%S").to_string())
    .bind(last_day.and_hms_opt(23, 59, 59).unwrap().format("%Y-%m-%d %H:%M:%S").to_string())
    .fetch_all(pool)
    .await?;

    // Print calendar header
    let month_name = first_day.format("%B %Y").to_string();
    println!("\n{}", month_name.bold().cyan());
    println!("{}", "Su Mo Tu We Th Fr Sa".dimmed());

    // Get the day of week for the first day (0 = Sunday)
    let first_weekday = first_day.weekday().num_days_from_sunday() as usize;
    
    // Print leading spaces
    print!("{}", "   ".repeat(first_weekday));

    // Create a map of sessions by day
    let mut sessions_by_day = std::collections::HashMap::new();
    for session in &sessions {
        let start = DateTime::parse_from_rfc3339(&session.1)
            .unwrap()
            .with_timezone(&Utc)
            .naive_local();
        let day = start.day() as usize;
        sessions_by_day.entry(day).or_insert_with(Vec::new).push(session);
    }

    // Print calendar days
    for day in 1..=last_day.day() {
        let day_num = day as usize;
        
        // Print day number
        if let Some(_sessions) = sessions_by_day.get(&day_num) {
            // Day has sessions - print in green
            print!("{:2} ", day.to_string().green().bold());
        } else {
            // Regular day
            print!("{:2} ", day);
        }

        // New line at end of week
        if (first_weekday + day_num) % 7 == 0 {
            println!();
        }
    }
    println!("\n");

    // Print session details
    if !sessions.is_empty() {
        println!("{}", "Sessions:".bold().cyan());
        for session in sessions {
            let start = DateTime::parse_from_rfc3339(&session.1)
                .unwrap()
                .with_timezone(&Utc)
                .naive_local();
            let end = if let Some(end_time) = &session.2 {
                DateTime::parse_from_rfc3339(end_time)
                    .unwrap()
                    .with_timezone(&Utc)
                    .naive_local()
            } else {
                chrono::Local::now().naive_local()
            };
            let duration = end - start;
            
            println!("  {} - {} ({}) | {} - {}", 
                start.format("%a %b %d %H:%M").to_string().green(),
                end.format("%H:%M").to_string(),
                format_duration(duration),
                session.4.bold(), // program name
                session.5 // block name
            );
            
            if let Some(notes) = session.3 {
                if !notes.is_empty() {
                    println!("    {}", notes.dimmed());
                }
            }
        }
    }

    Ok(())
}

fn format_duration(duration: chrono::Duration) -> String {
    let hours = duration.num_hours();
    let minutes = duration.num_minutes() % 60;
    
    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
} 