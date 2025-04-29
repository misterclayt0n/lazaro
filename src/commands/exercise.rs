use sqlx::{Row, SqlitePool};
use colored::Colorize;
use crate::cli::ExerciseCmd;
use anyhow::Result;

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
        
        ExerciseCmd::Import { file: _ } => {
            // read TOML, deserialize into a Vec<struct { name, description, primary_muscle }>
            // then loop and `INSERT OR IGNORE INTO exercises ...`
            todo!("impl exercise import");
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
