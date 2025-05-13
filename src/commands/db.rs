use anyhow::Result;
use colored::Colorize;
use sqlx::{query, Executor, SqlitePool};

/* ────────────────────────── public entry point ───────────────────────── */

pub async fn handle(cmd: crate::cli::DbCmd, pool: &SqlitePool) -> Result<()> {
    match cmd {
        crate::cli::DbCmd::Migrate { old_db } => migrate(pool, &old_db).await,
        // Export / import can be added later
        _ => Ok(()),
    }
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

