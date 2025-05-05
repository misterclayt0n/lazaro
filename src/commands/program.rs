use std::{
    collections::{HashMap, HashSet},
    fs::read_to_string,
};

use anyhow::{Context, Result};
use colored::Colorize;
use serde::Deserialize;
use sqlx::{Row, SqlitePool};

use crate::{
    cli::ProgramCmd,
    types::{OutputFmt, emit},
};

#[derive(Debug, Deserialize)]
struct ProgramToml {
    name: String,
    description: Option<String>,
    weeks: Option<Vec<WeekToml>>,
    blocks: Option<Vec<BlockToml>>,
}

#[derive(Debug, Deserialize)]
struct WeekToml {
    week: u32,
    blocks: Vec<BlockToml>,
}

#[derive(Debug, Deserialize)]
struct BlockToml {
    name: String,
    description: Option<String>,
    exercises: Vec<BlockExerciseToml>,
}

#[derive(Debug, Deserialize)]
struct BlockExerciseToml {
    name: String,
    sets: u32,
    reps: Option<Vec<String>>,
    target_rpe: Option<Vec<f32>>,
    target_rm_percent: Option<Vec<f32>>,
    notes: Option<String>,
    program_1rm: Option<f32>,
    technique: Option<String>,
    group: Option<u32>,
}

#[derive(Debug)]
struct BlockRow {
    name: String,
    week: Option<i64>,
}

#[derive(serde::Serialize)]
struct ProgJson {
    idx: i64,
    name: String,
    description: String,
    created_at: String,
    blocks: i64,
}

fn plain_len(s: &str) -> usize {
    let mut n = 0;
    let mut esc = false;
    for b in s.bytes() {
        match (esc, b) {
            (true, b'm') => esc = false,
            (true, _) => {}
            (false, 0x1B) => esc = true,
            (false, _) => n += 1,
        }
    }
    n
}

async fn blocks_by_program(pool: &SqlitePool) -> Result<HashMap<String, Vec<BlockRow>>> {
    let rows = sqlx::query(
        r#"
        SELECT program_id, name, week
        FROM   program_blocks
        ORDER  BY program_id, COALESCE(week,1), name
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut map: HashMap<String, Vec<BlockRow>> = HashMap::new();
    for r in rows {
        map.entry(r.get::<String, _>("program_id"))
            .or_default()
            .push(BlockRow {
                name: r.get("name"),
                week: r.get("week"),
            });
    }
    Ok(map)
}

fn pretty_print(
    progs: &[ProgJson],
    blk_map: &HashMap<String, Vec<BlockRow>>,
    idx2id: &HashMap<i64, String>,
) {
    if progs.is_empty() {
        println!("{}", "  (no programs found)".dimmed());
        return;
    }

    println!("{}", "Programs:".cyan().bold());

    let idx_w = progs
        .iter()
        .map(|p| p.idx.to_string().len())
        .max()
        .unwrap_or(1);
    let mut left = Vec::<String>::new();
    let mut right = Vec::<String>::new();

    for p in progs {
        //
        // Program row.
        //
        let idx = format!("{:>width$}", p.idx, width = idx_w).yellow();
        let desc = if p.description.is_empty() {
            String::new()
        } else {
            format!("– {}", p.description).dimmed().to_string()
        };
        left.push(format!(" {} • {} {}", idx, p.name.bold(), desc));
        right.push(
            format!("added {}", &p.created_at[..10])
                .dimmed()
                .to_string(),
        );

        //
        // Block rows
        //
        if let Some(id) = idx2id.get(&p.idx) {
            if let Some(blocks) = blk_map.get(id) {
                for (i, b) in blocks.iter().enumerate() {
                    let connector = if i + 1 == blocks.len() {
                        "└─"
                    } else {
                        "├─"
                    };
                    let b_idx_col = format!("{:>width$}", i + 1, width = idx_w).yellow();
                    let label = match b.week {
                        Some(w) => format!("{} (week {})", b.name, w),
                        None => b.name.clone(),
                    };
                    left.push(format!(
                        " {}   {} {} • {}",
                        " ".repeat(idx_w).yellow(),
                        connector,
                        b_idx_col,
                        label.bold()
                    ));
                    right.push(String::new());
                }
            }
        }
    }

    let pad_plain = left.iter().map(|s| plain_len(s)).max().unwrap_or(0);
    for (l, r) in left.into_iter().zip(right) {
        let pad = pad_plain + (l.len() - plain_len(&l));
        if r.is_empty() {
            println!("{}", l);
        } else {
            println!("{:<pad$} {} {}", l, "|".blue(), r, pad = pad);
        }
    }
}

pub async fn handle(cmd: ProgramCmd, pool: &SqlitePool, fmt: OutputFmt) -> Result<()> {
    match cmd {
        ProgramCmd::Import { files } => {
            if files.is_empty() {
                println!("{} no program file provided", "warning:".yellow().bold());
            }
            for f in files {
                match import_single_program(pool, &f).await {
                    Ok(()) => {}
                    Err(e) => {
                        if let Some(io_err) = e.downcast_ref::<std::io::Error>() {
                            if io_err.kind() == std::io::ErrorKind::NotFound {
                                println!(
                                    "{} cannot open file `{}` – file not found",
                                    "error:".red().bold(),
                                    f
                                );
                                continue;
                            }

                            return Err(e);
                        }
                    }
                }
            }
        }

        ProgramCmd::List => {
            let rows = sqlx::query(
                r#"
                SELECT ROW_NUMBER() OVER (ORDER BY name) AS idx,
                       id, name,
                       COALESCE(description,'') AS description,
                       created_at
                FROM   programs
                ORDER  BY idx
                "#,
            )
            .fetch_all(pool)
            .await?;

            let mut progs = Vec::<ProgJson>::new();
            let mut idx2id = HashMap::<i64, String>::new();
            for r in &rows {
                let idx: i64 = r.get("idx");
                progs.push(ProgJson {
                    idx,
                    name: r.get("name"),
                    description: r.get("description"),
                    created_at: r.get("created_at"),
                    blocks: 0,
                });
                idx2id.insert(idx, r.get("id"));
            }

            let blk_map = blocks_by_program(pool).await?;
            for p in &mut progs {
                if let Some(id) = idx2id.get(&p.idx) {
                    p.blocks = blk_map.get(id).map(|v| v.len() as i64).unwrap_or(0);
                }
            }

            emit(fmt, &progs, || pretty_print(&progs, &blk_map, &idx2id));
        }
    }
    Ok(())
}

async fn import_single_program(pool: &SqlitePool, file: &str) -> Result<()> {
    let toml_str = read_to_string(file).with_context(|| format!("reading `{file}`"))?;
    let prog: ProgramToml =
        toml::from_str(&toml_str).with_context(|| format!("parsing `{file}`"))?;

    // Check all exercises exist.
    let mut all_ex = HashSet::<&str>::new();
    if let Some(weeks) = &prog.weeks {
        for w in weeks {
            for b in &w.blocks {
                for e in &b.exercises {
                    all_ex.insert(&e.name);
                }
            }
        }
    }
    if let Some(blocks) = &prog.blocks {
        for b in blocks {
            for e in &b.exercises {
                all_ex.insert(&e.name);
            }
        }
    }

    if !all_ex.is_empty() {
        let q_marks = std::iter::repeat("?")
            .take(all_ex.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!("SELECT name FROM exercises WHERE name IN ({})", q_marks);
        let mut q = sqlx::query_as::<_, (String,)>(&sql);
        for n in &all_ex {
            q = q.bind(n);
        }
        let present: HashSet<_> = q.fetch_all(pool).await?.into_iter().map(|(n,)| n).collect();
        let missing: Vec<_> = all_ex
            .into_iter()
            .filter(|n| !present.contains(*n))
            .collect();
        if !missing.is_empty() {
            println!(
                "{} cannot import program `{}` – missing exercises: {}",
                "warning:".yellow().bold(),
                prog.name,
                missing.join(", ")
            );
            return Ok(());
        }
    }

    // Transactional import.
    let mut tx = pool.begin().await?;

    // Program row.
    let prog_id = uuid::Uuid::new_v4().to_string();
    let res = sqlx::query(
        r#"INSERT INTO programs (id,name,description,created_at)
               VALUES (?1,?2,?3,datetime('now'))"#,
    )
    .bind(&prog_id)
    .bind(&prog.name)
    .bind(prog.description.as_deref())
    .execute(&mut *tx)
    .await;

    if let Err(sqlx::Error::Database(db_err)) = &res {
        if db_err.code() == Some("2067".into()) {
            println!(
                "{} program `{}` already exists – skipping",
                "warning:".yellow().bold(),
                prog.name
            );
            tx.rollback().await?;
            return Ok(());
        }
    }
    res?;

    // Flatten blocks.
    let mut blocks: Vec<(Option<u32>, BlockToml)> = Vec::new();
    if let Some(weeks) = prog.weeks {
        for w in weeks {
            for b in w.blocks {
                blocks.push((Some(w.week), b));
            }
        }
    }
    if let Some(bs) = prog.blocks {
        for b in bs {
            blocks.push((None, b));
        }
    }

    // Insert blocks + exercises.
    // NOTE: Removed the "options" column.
    for (week_opt, block) in blocks {
        let mut seen = HashSet::new();
        let mut dup = Vec::<&str>::new();
        for e in &block.exercises {
            if !seen.insert(e.name.as_str()) {
                dup.push(&e.name);
            }
        }
        if !dup.is_empty() {
            println!(
                "{} block `{}` in program `{}` has duplicates: {} – skipped",
                "warning:".yellow().bold(),
                block.name,
                prog.name,
                dup.join(", ")
            );
            continue;
        }

        let block_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            r#"INSERT INTO program_blocks
                 (id,program_id,name,description,week)
               VALUES (?1,?2,?3,?4,?5)"#,
        )
        .bind(&block_id)
        .bind(&prog_id)
        .bind(&block.name)
        .bind(block.description.as_deref())
        .bind(week_opt)
        .execute(&mut *tx)
        .await?;

        for (order_idx, ex) in block.exercises.iter().enumerate() {
            let ex_id: String = sqlx::query_scalar("SELECT id FROM exercises WHERE name = ?")
                .bind(&ex.name)
                .fetch_one(&mut *tx)
                .await?;

            let reps_csv = ex.reps.as_ref().map(|v| v.join(","));
            let rpe_csv = ex.target_rpe.as_ref().map(|v| {
                v.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            });
            let rm_csv = ex.target_rm_percent.as_ref().map(|v| {
                v.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            });

            sqlx::query(
                r#"INSERT INTO program_exercises
                     (id,program_block_id,exercise_id,sets,reps,
                      target_rpe,target_rm_percent,notes,program_1rm,
                      technique,technique_group,order_index)
                   VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)"#,
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(&block_id)
            .bind(&ex_id)
            .bind(ex.sets as i32)
            .bind(reps_csv)
            .bind(rpe_csv)
            .bind(rm_csv)
            .bind(ex.notes.as_deref())
            .bind(ex.program_1rm)
            .bind(ex.technique.as_deref())
            .bind(ex.group.map(|g| g as i32))
            .bind(order_idx as i32)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
    println!("{} `{}`", "ok:".green().bold(), prog.name);
    Ok(())
}
