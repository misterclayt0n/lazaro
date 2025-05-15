use anyhow::Result;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use sqlx::{query, Executor, Row, SqlitePool};
use std::fs;

use crate::cli::DbCmd;

#[derive(Serialize, Deserialize)]
struct DatabaseDump {
    exercises: Vec<Exercise>,
    programs: Vec<Program>,
    sessions: Vec<Session>,
    #[serde(default)]
    personal_records: Vec<PersonalRecord>,
}

#[derive(Serialize, Deserialize)]
struct Exercise {
    id: String,
    name: String,
    primary_muscle: String,
    description: Option<String>,
    created_at: String,
    estimated_one_rm: Option<f64>,
    current_pr_date: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Program {
    id: String,
    name: String,
    description: Option<String>,
    created_at: String,
    blocks: Vec<ProgramBlock>,
}

#[derive(Serialize, Deserialize)]
struct ProgramBlock {
    id: String,
    name: String,
    description: Option<String>,
    exercises: Vec<ProgramExercise>,
}

#[derive(Serialize, Deserialize)]
struct ProgramExercise {
    id: String,
    exercise_id: String,
    sets: i32,
    reps: Option<String>,
    target_rpe: Option<String>,
    target_rm_percent: Option<String>,
    notes: Option<String>,
    program_1rm: Option<f64>,
    technique: Option<String>,
    technique_group: Option<i32>,
    order_index: i32,
}

#[derive(Serialize, Deserialize)]
struct Session {
    id: String,
    program_block_id: String,
    start_time: String,
    end_time: Option<String>,
    notes: Option<String>,
    exercises: Vec<SessionExercise>,
}

#[derive(Serialize, Deserialize)]
struct SessionExercise {
    id: String,
    exercise_id: String,
    notes: Option<String>,
    sets: Vec<ExerciseSet>,
}

#[derive(Serialize, Deserialize)]
struct ExerciseSet {
    id: String,
    weight: f64,
    reps: i32,
    rpe: Option<f64>,
    rm_percent: Option<f64>,
    notes: Option<String>,
    timestamp: String,
    ignore_for_one_rm: bool,
    bodyweight: bool,
}

#[derive(Serialize, Deserialize)]
struct PersonalRecord {
    exercise_id: String,
    date: String,
    weight: f64,
    reps: i32,
    estimated_1rm: f64,
}

/* ────────────────────────── public entry point ───────────────────────── */

pub async fn handle(cmd: DbCmd, pool: &SqlitePool) -> Result<()> {
    match cmd {
        DbCmd::Export { file } => {
            let file_path = file.unwrap_or_else(|| "dump.toml".to_string());
            export_db(pool, &file_path).await?;
            println!("{} database exported to {}", "ok:".green().bold(), file_path);
        }
        DbCmd::Import { file } => {
            import_db(pool, &file).await?;
            println!("{} database imported from {}", "ok:".green().bold(), file);
        }
        DbCmd::Migrate { old_db } => migrate(pool, &old_db).await?,
    }
    Ok(())
}

/* ───────────────────────────── migrate old ──────────────────────────── */

pub async fn migrate(pool: &SqlitePool, old_path: &str) -> Result<()> {
    /* 1. always work on one physical connection */
    let mut conn = pool.acquire().await?;

    /* 2. attach the legacy file */
    let attach = format!("ATTACH DATABASE '{}' AS old;", old_path.replace('\'', "''"));
    conn.execute(&*attach).await?;

    /* sanity-check */
    let has_exercises: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM old.sqlite_master
         WHERE type='table' AND name='exercises';",
    )
    .fetch_one(&mut *conn)
    .await?;
    if has_exercises == 0 {
        anyhow::bail!("`{old_path}` is not a Lazarus-v0 DB (missing `exercises` table)");
    }

    /* 3. make sure a placeholder program / block exists */
    const LEGACY_PROG: &str  = "legacy-prog";
    const LEGACY_BLOCK: &str = "legacy-block";

    query(
        "INSERT OR IGNORE INTO programs(id,name,description,created_at)
         VALUES(?,'Legacy import','auto-generated',datetime('now'));",
    )
    .bind(LEGACY_PROG)
    .execute(&mut *conn)
    .await?;

    query(
        "INSERT OR IGNORE INTO program_blocks(id,program_id,name)
         VALUES(?,?, 'Imported sessions');",
    )
    .bind(LEGACY_BLOCK)
    .bind(LEGACY_PROG)
    .execute(&mut *conn)
    .await?;

    /* 4. exercises ---------------------------------------------------- */
    conn.execute(
        "INSERT OR IGNORE INTO exercises(id,name,description,primary_muscle,created_at)
         SELECT id,
                name,
                description,
                CASE
                    WHEN lower(primary_muscle) IN ('quadriceps','quad','quads')
                    THEN 'quads'
                    ELSE lower(primary_muscle)
                END,
                created_at
         FROM old.exercises;",
    )
    .await?;

    /* 5. sessions ----------------------------------------------------- */
    query(
        "INSERT OR IGNORE INTO training_sessions
                (id, program_block_id, start_time, end_time, notes)
         SELECT id, ?, start_time, end_time, notes
         FROM   old.training_sessions;",
    )
    .bind(LEGACY_BLOCK)
    .execute(&mut *conn)
    .await?;

    /* close any legacy sessions that never got an end_time */
    query(
        "UPDATE training_sessions
         SET   end_time = start_time
         WHERE program_block_id = ?
           AND end_time IS NULL;",
    )
    .bind(LEGACY_BLOCK)
    .execute(&mut *conn)
    .await?;

    /* 6. session-exercises + sets ------------------------------------- */
    conn.execute(
        "INSERT OR IGNORE INTO training_session_exercises
             (id, training_session_id, exercise_id, notes)
         SELECT * FROM old.training_session_exercises;",
    )
    .await?;

    conn.execute(
        "INSERT OR IGNORE INTO exercise_sets
             (id, session_exercise_id, weight, reps, rpe, rm_percent, notes,
              timestamp, ignore_for_one_rm, bodyweight)
         SELECT * FROM old.exercise_sets;",
    )
    .await?;

    /* 7. PERSONAL RECORDS (one best-set per day) ---------------------- */
    conn.execute(
        r#"
INSERT OR REPLACE INTO personal_records
      (exercise_id, date, weight, reps, estimated_1rm)
WITH ranked AS (
    SELECT
        e.id                             AS exercise_id,
        date(ts.start_time)              AS day,
        es.weight                        AS weight,
        es.reps                          AS reps,
        es.weight * (1.0 + es.reps/30.0) AS estimated_1rm,
        ROW_NUMBER() OVER (
            PARTITION BY e.id, date(ts.start_time)
            ORDER BY es.weight * (1.0 + es.reps/30.0) DESC
        ) AS rn
    FROM   exercise_sets es
    JOIN   training_session_exercises tse ON tse.id = es.session_exercise_id
    JOIN   training_sessions         ts  ON ts.id  = tse.training_session_id
    JOIN   exercises                 e   ON e.id   = tse.exercise_id
    WHERE  es.weight > 0
)
SELECT exercise_id, day, weight, reps, estimated_1rm
FROM   ranked
WHERE  rn = 1;
"#,
    )
    .await?;

    /* 8. update exercises with BEST ever 1-RM ------------------------- */
    conn.execute(
        r#"
UPDATE exercises
SET   current_pr_date  = pr.date,
      estimated_one_rm = pr.estimated_1rm
FROM (
    SELECT exercise_id,
           date,
           estimated_1rm,
           ROW_NUMBER() OVER (
               PARTITION BY exercise_id
               ORDER BY estimated_1rm DESC
           ) AS rn
    FROM   personal_records
) AS pr
WHERE pr.exercise_id = exercises.id
  AND pr.rn = 1;
"#,
    )
    .await?;

    /* 9. detach & done ------------------------------------------------ */
    conn.execute("DETACH DATABASE old;").await?;
    println!(
        "{} migration complete – legacy exercises, sessions & PRs imported",
        "ok:".green().bold()
    );

    Ok(())
}

async fn export_db(pool: &SqlitePool, file_path: &str) -> Result<()> {
    // Fetch exercises
    let exercises = query(
        r#"
        SELECT id, name, primary_muscle, description, created_at, 
               estimated_one_rm, current_pr_date
        FROM exercises
        "#
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| Exercise {
        id: row.get("id"),
        name: row.get("name"),
        primary_muscle: row.get("primary_muscle"),
        description: row.get("description"),
        created_at: row.get("created_at"),
        estimated_one_rm: row.get("estimated_one_rm"),
        current_pr_date: row.get("current_pr_date"),
    })
    .collect::<Vec<_>>();

    // Fetch programs with their blocks and exercises
    let mut programs = Vec::new();
    let program_rows = query(
        r#"
        SELECT id, name, description, created_at
        FROM programs
        "#
    )
    .fetch_all(pool)
    .await?;

    for prog in program_rows {
        let mut blocks = Vec::new();
        let block_rows = query(
            r#"
            SELECT id, name, description
            FROM program_blocks
            WHERE program_id = ?
            "#
        )
        .bind(prog.get::<String, _>("id"))
        .fetch_all(pool)
        .await?;

        for block in block_rows {
            let exercises = query(
                r#"
                SELECT id, exercise_id, sets, reps, target_rpe, target_rm_percent,
                       notes, program_1rm, technique, technique_group, order_index
                FROM program_exercises
                WHERE program_block_id = ?
                "#
            )
            .bind(block.get::<String, _>("id"))
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|ex| ProgramExercise {
                id: ex.get("id"),
                exercise_id: ex.get("exercise_id"),
                sets: ex.get("sets"),
                reps: ex.get("reps"),
                target_rpe: ex.get("target_rpe"),
                target_rm_percent: ex.get("target_rm_percent"),
                notes: ex.get("notes"),
                program_1rm: ex.get("program_1rm"),
                technique: ex.get("technique"),
                technique_group: ex.get("technique_group"),
                order_index: ex.get("order_index"),
            })
            .collect();

            blocks.push(ProgramBlock {
                id: block.get("id"),
                name: block.get("name"),
                description: block.get("description"),
                exercises,
            });
        }

        programs.push(Program {
            id: prog.get("id"),
            name: prog.get("name"),
            description: prog.get("description"),
            created_at: prog.get("created_at"),
            blocks,
        });
    }

    // Fetch sessions with their exercises and sets
    let mut sessions = Vec::new();
    let session_rows = query(
        r#"
        SELECT id, program_block_id, start_time, end_time, notes
        FROM training_sessions
        "#
    )
    .fetch_all(pool)
    .await?;

    for sess in session_rows {
        let mut exercises = Vec::new();
        let exercise_rows = query(
            r#"
            SELECT id, exercise_id, notes
            FROM training_session_exercises
            WHERE training_session_id = ?
            "#
        )
        .bind(sess.get::<String, _>("id"))
        .fetch_all(pool)
        .await?;

        for ex in exercise_rows {
            let sets = query(
                r#"
                SELECT id, weight, reps, rpe, rm_percent, notes,
                       timestamp, ignore_for_one_rm, bodyweight
                FROM exercise_sets
                WHERE session_exercise_id = ?
                "#
            )
            .bind(ex.get::<String, _>("id"))
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|set| ExerciseSet {
                id: set.get("id"),
                weight: set.get("weight"),
                reps: set.get("reps"),
                rpe: set.get("rpe"),
                rm_percent: set.get("rm_percent"),
                notes: set.get("notes"),
                timestamp: set.get("timestamp"),
                ignore_for_one_rm: set.get::<i32, _>("ignore_for_one_rm") != 0,
                bodyweight: set.get::<i32, _>("bodyweight") != 0,
            })
            .collect();

            exercises.push(SessionExercise {
                id: ex.get("id"),
                exercise_id: ex.get("exercise_id"),
                notes: ex.get("notes"),
                sets,
            });
        }

        sessions.push(Session {
            id: sess.get("id"),
            program_block_id: sess.get("program_block_id"),
            start_time: sess.get("start_time"),
            end_time: sess.get("end_time"),
            notes: sess.get("notes"),
            exercises,
        });
    }

    // Fetch personal records
    let personal_records = query(
        r#"
        SELECT exercise_id, date, weight, reps, estimated_1rm
        FROM personal_records
        "#
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| PersonalRecord {
        exercise_id: row.get("exercise_id"),
        date: row.get("date"),
        weight: row.get("weight"),
        reps: row.get("reps"),
        estimated_1rm: row.get("estimated_1rm"),
    })
    .collect::<Vec<_>>();

    // Create the final dump structure
    let dump = DatabaseDump {
        exercises,
        programs,
        sessions,
        personal_records,
    };

    // Write to file
    let toml_string = toml::to_string_pretty(&dump)?;
    fs::write(file_path, toml_string)?;

    Ok(())
}

async fn import_db(pool: &SqlitePool, file_path: &str) -> Result<()> {
    // Read and parse the TOML file
    let toml_str = fs::read_to_string(file_path)?;
    let dump: DatabaseDump = toml::from_str(&toml_str)?;

    // Start a transaction
    let mut tx = pool.begin().await?;

    // Import exercises
    for ex in dump.exercises {
        query(
            r#"
            INSERT OR REPLACE INTO exercises 
            (id, name, primary_muscle, description, created_at, estimated_one_rm, current_pr_date)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&ex.id)
        .bind(&ex.name)
        .bind(&ex.primary_muscle)
        .bind(&ex.description)
        .bind(&ex.created_at)
        .bind(ex.estimated_one_rm)
        .bind(&ex.current_pr_date)
        .execute(&mut *tx)
        .await?;
    }

    // Import programs with their blocks and exercises
    for prog in dump.programs {
        // Insert program
        query(
            r#"
            INSERT OR REPLACE INTO programs (id, name, description, created_at)
            VALUES (?, ?, ?, ?)
            "#
        )
        .bind(&prog.id)
        .bind(&prog.name)
        .bind(&prog.description)
        .bind(&prog.created_at)
        .execute(&mut *tx)
        .await?;

        // Insert blocks
        for block in prog.blocks {
            query(
                r#"
                INSERT OR REPLACE INTO program_blocks (id, program_id, name, description)
                VALUES (?, ?, ?, ?)
                "#
            )
            .bind(&block.id)
            .bind(&prog.id)
            .bind(&block.name)
            .bind(&block.description)
            .execute(&mut *tx)
            .await?;

            // Insert program exercises
            for ex in block.exercises {
                query(
                    r#"
                    INSERT OR REPLACE INTO program_exercises 
                    (id, program_block_id, exercise_id, sets, reps, target_rpe, 
                     target_rm_percent, notes, program_1rm, technique, technique_group, order_index)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#
                )
                .bind(&ex.id)
                .bind(&block.id)
                .bind(&ex.exercise_id)
                .bind(ex.sets)
                .bind(&ex.reps)
                .bind(&ex.target_rpe)
                .bind(&ex.target_rm_percent)
                .bind(&ex.notes)
                .bind(ex.program_1rm)
                .bind(&ex.technique)
                .bind(ex.technique_group)
                .bind(ex.order_index)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    // Import sessions with their exercises and sets
    for sess in dump.sessions {
        // Insert session
        query(
            r#"
            INSERT OR REPLACE INTO training_sessions 
            (id, program_block_id, start_time, end_time, notes)
            VALUES (?, ?, ?, ?, ?)
            "#
        )
        .bind(&sess.id)
        .bind(&sess.program_block_id)
        .bind(&sess.start_time)
        .bind(&sess.end_time)
        .bind(&sess.notes)
        .execute(&mut *tx)
        .await?;

        // Insert session exercises and their sets
        for ex in sess.exercises {
            query(
                r#"
                INSERT OR REPLACE INTO training_session_exercises
                (id, training_session_id, exercise_id, notes)
                VALUES (?, ?, ?, ?)
                "#
            )
            .bind(&ex.id)
            .bind(&sess.id)
            .bind(&ex.exercise_id)
            .bind(&ex.notes)
            .execute(&mut *tx)
            .await?;

            // Insert sets
            for set in ex.sets {
                query(
                    r#"
                    INSERT OR REPLACE INTO exercise_sets
                    (id, session_exercise_id, weight, reps, rpe, rm_percent, notes,
                     timestamp, ignore_for_one_rm, bodyweight)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#
                )
                .bind(&set.id)
                .bind(&ex.id)
                .bind(set.weight)
                .bind(set.reps)
                .bind(set.rpe)
                .bind(set.rm_percent)
                .bind(&set.notes)
                .bind(&set.timestamp)
                .bind(set.ignore_for_one_rm as i32)
                .bind(set.bodyweight as i32)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    // Import personal records if there are any in the dump
    if !dump.personal_records.is_empty() {
        for pr in dump.personal_records {
            query(
                r#"
                INSERT OR REPLACE INTO personal_records
                (exercise_id, date, weight, reps, estimated_1rm)
                VALUES (?, ?, ?, ?, ?)
                "#
            )
            .bind(&pr.exercise_id)
            .bind(&pr.date)
            .bind(pr.weight)
            .bind(pr.reps)
            .bind(pr.estimated_1rm)
            .execute(&mut *tx)
            .await?;
        }
    } else {
        // If no PRs in the dump, calculate them from session sets
        // First, clear any existing PRs
        query("DELETE FROM personal_records")
            .execute(&mut *tx)
            .await?;

        // Insert daily PRs - one best set per exercise per day
        query(
            r#"
            INSERT INTO personal_records
                  (exercise_id, date, weight, reps, estimated_1rm)
            WITH ranked AS (
                SELECT
                    e.id                             AS exercise_id,
                    date(ts.start_time)              AS day,
                    es.weight                        AS weight,
                    es.reps                          AS reps,
                    es.weight * (1.0 + es.reps/30.0) AS estimated_1rm,
                    ROW_NUMBER() OVER (
                        PARTITION BY e.id, date(ts.start_time)
                        ORDER BY es.weight * (1.0 + es.reps/30.0) DESC
                    ) AS rn
                FROM   exercise_sets es
                JOIN   training_session_exercises tse ON tse.id = es.session_exercise_id
                JOIN   training_sessions         ts  ON ts.id  = tse.training_session_id
                JOIN   exercises                 e   ON e.id   = tse.exercise_id
                WHERE  es.weight > 0
                  AND  es.ignore_for_one_rm = 0
            )
            SELECT exercise_id, day, weight, reps, estimated_1rm
            FROM   ranked
            WHERE  rn = 1
            "#
        )
        .execute(&mut *tx)
        .await?;

        // Find all-time PR for each exercise
        let exercise_prs = query(
            r#"
            WITH all_sets AS (
                SELECT
                    e.id AS exercise_id,
                    e.name AS exercise_name,
                    es.weight,
                    es.reps,
                    es.weight * (1.0 + es.reps/30.0) AS estimated_1rm,
                    date(ts.start_time) AS date
                FROM exercise_sets es
                JOIN training_session_exercises tse ON tse.id = es.session_exercise_id
                JOIN training_sessions ts ON ts.id = tse.training_session_id
                JOIN exercises e ON e.id = tse.exercise_id
                WHERE es.weight > 0
                  AND es.ignore_for_one_rm = 0
            ),
            ranked_by_1rm AS (
                SELECT
                    exercise_id,
                    date,
                    weight,
                    reps,
                    estimated_1rm,
                    ROW_NUMBER() OVER (PARTITION BY exercise_id ORDER BY estimated_1rm DESC) AS rn
                FROM all_sets
            )
            SELECT exercise_id, date, weight, reps, estimated_1rm
            FROM ranked_by_1rm
            WHERE rn = 1
            "#
        )
        .fetch_all(&mut *tx)
        .await?;

        // Update each exercise with its best PR
        for row in exercise_prs {
            let exercise_id: String = row.get("exercise_id");
            let date: String = row.get("date");
            let estimated_1rm: f64 = row.get("estimated_1rm");

            query(
                r#"
                UPDATE exercises
                SET current_pr_date = ?,
                    estimated_one_rm = ?
                WHERE id = ?
                "#
            )
            .bind(&date)
            .bind(estimated_1rm)
            .bind(&exercise_id)
            .execute(&mut *tx)
            .await?;
        }
    }

    // Commit all changes
    tx.commit().await?;

    Ok(())
}

