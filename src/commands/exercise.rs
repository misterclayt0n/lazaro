use sqlx::SqlitePool;

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
                Ok(info) if info.rows_affected() == 1 =>  println!("Exercise \"{}\" added", &name),
                Ok(_) => println!("Exercise \"{}\" was not inserted", &name),
                Err(sqlx::Error::Database(db_err)) if db_err.code() == Some("2067".into()) => {
                    // 2067 = SQLITE_CONSTRAINT_UNIQUE
                    println!(
                        "Exercise \"{}\" already exists — use `ex list` to view all exercises",
                        name
                    );
                }
                Err(e) => return Err(e.into())
            }
        }
        
        ExerciseCmd::Import { file: _ } => {
            // read TOML, deserialize into a Vec<struct { name, description, primary_muscle }>
            // then loop and `INSERT OR IGNORE INTO exercises ...`
            todo!("impl exercise import");
        }
        _ => todo!("not gucci")
    }

    Ok(())
}
