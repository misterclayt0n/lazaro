use anyhow::Result;
use colored::Colorize;
use sqlx::{Executor, SqlitePool, query};

/* ────────────────────────── public entry point ───────────────────────── */

pub async fn handle(cmd: crate::cli::DbCmd, pool: &SqlitePool) -> Result<()> {
    match cmd {
        // crate::cli::DbCmd::Export { path }   => export (pool, path).await,
        // crate::cli::DbCmd::Import { path }   => import (pool, path).await,
        crate::cli::DbCmd::Migrate { old_db } => migrate(pool, &old_db).await,
        _ => Ok(()),
    }
}

/* ───────────────────────────── migrate old ──────────────────────────── */

pub async fn migrate(pool: &SqlitePool, old_path: &str) -> Result<()> {
    // ── 1. grab ONE physical connection from the pool ────────────────
    let mut conn = pool.acquire().await?;

    // ── 2. attach the v0 DB on that very same connection ─────────────
    let attach = format!("ATTACH DATABASE '{}' AS old;", old_path.replace('\'', "''"));
    conn.execute(&*attach).await?;

    // sanity-check: must have table `old.exercises`
    let exists: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM old.sqlite_master \
         WHERE type='table' AND name='exercises';")
        .fetch_one(&mut *conn)
        .await?;
    if exists == 0 {
        anyhow::bail!("`{old_path}` is not a Lazarus-v0 DB (table `exercises` missing)");
    }

    // ── 3. placeholder program / block ───────────────────────────────
    let legacy_prog  = "legacy-prog";
    let legacy_block = "legacy-block";

    query("INSERT OR IGNORE INTO programs(id,name,description,created_at)
           VALUES(?,'Legacy import','auto-generated',datetime('now'))")
        .bind(legacy_prog)
        .execute(&mut *conn).await?;

    query("INSERT OR IGNORE INTO program_blocks(id,program_id,name)
           VALUES(?,?, 'Imported sessions')")
        .bind(legacy_block)
        .bind(legacy_prog)
        .execute(&mut *conn).await?;

    // ── 4. exercises (quadriceps → quads) ────────────────────────────
    conn.execute(
        "INSERT OR IGNORE INTO exercises(id,name,description,primary_muscle,created_at)
         SELECT id,name,description,
                CASE
                    WHEN lower(primary_muscle) IN ('quadriceps','quad','quads')
                         THEN 'quads'
                    ELSE lower(primary_muscle)
                END,
                created_at
         FROM old.exercises;").await?;

    // ── 5. sessions, sets, PRs ───────────────────────────────────────
    query("INSERT OR IGNORE INTO training_sessions
             (id,program_block_id,start_time,end_time,notes)
           SELECT id, ?, start_time,end_time,notes
             FROM old.training_sessions;")
        .bind(legacy_block)
        .execute(&mut *conn).await?;

    conn.execute(
        "INSERT OR IGNORE INTO training_session_exercises
             (id,training_session_id,exercise_id,notes)
         SELECT *
           FROM old.training_session_exercises;").await?;

    conn.execute(
        "INSERT OR IGNORE INTO exercise_sets
             (id,session_exercise_id,weight,reps,rpe,rm_percent,notes,timestamp,
              ignore_for_one_rm,bodyweight)
         SELECT *
           FROM old.exercise_sets;").await?;

    conn.execute(
        "INSERT OR IGNORE INTO personal_records
             (exercise_id,date,weight,reps,estimated_1rm)
         SELECT *
           FROM old.personal_records;").await?;

    // ── 6. detach & finish ───────────────────────────────────────────
    conn.execute("DETACH DATABASE old;").await?;
    println!("{} migration complete – exercises & sessions imported",
             "ok:".green().bold());
    Ok(())
}
