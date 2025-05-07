use std::{
    collections::{HashMap, HashSet},
    fs::read_to_string,
};

use anyhow::Result;
use colored::Colorize;
use serde::Deserialize;
use sqlx::{Row, SqlitePool};

use crate::{
    cli::ProgramCmd,
    types::{OutputFmt, emit},
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProgramToml {
    name: String,
    description: Option<String>,
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
    let rows = sqlx::query("SELECT program_id, name FROM program_blocks ORDER BY program_id, name")
        .fetch_all(pool)
        .await?;

    let mut map: HashMap<String, Vec<BlockRow>> = HashMap::new();
    for r in rows {
        let pid: String = r.get("program_id");
        let name: String = r.get("name");
        map.entry(pid).or_default().push(BlockRow { name });
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
                    left.push(format!(
                        " {}   {} {} • {}",
                        " ".repeat(idx_w).yellow(),
                        connector,
                        b_idx_col,
                        b.name.bold()
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
                // Read TOML.
                let toml = match read_to_string(&f) {
                    Ok(s) => s,
                    Err(_) => {
                        println!("{} cannot open `{}`", "error:".red().bold(), f);
                        continue;
                    }
                };
                let prog: ProgramToml = match toml::from_str(&toml) {
                    Ok(p) => p,
                    Err(e) => {
                        println!("{} parsing `{}`: {}", "error:".red().bold(), f, e);
                        continue;
                    }
                };

                // Validate exercises exist.
                let mut all_ex = HashSet::new();
                for b in &prog.blocks {
                    for e in &b.exercises {
                        all_ex.insert(e.name.as_str());
                    }
                }
                if !all_ex.is_empty() {
                    let marks = std::iter::repeat("?")
                        .take(all_ex.len())
                        .collect::<Vec<_>>()
                        .join(",");
                    let query_str =
                        &format!("SELECT name FROM exercises WHERE name IN ({})", marks);

                    let mut q = sqlx::query_as::<_, (String,)>(query_str);
                    for &n in &all_ex {
                        q = q.bind(n);
                    }
                    let present: HashSet<_> =
                        q.fetch_all(pool).await?.into_iter().map(|(n,)| n).collect();
                    let missing: Vec<_> = all_ex
                        .into_iter()
                        .filter(|n| !present.contains(*n))
                        .collect();
                    if !missing.is_empty() {
                        println!(
                            "{} missing exercises: {}",
                            "warning:".yellow().bold(),
                            missing.join(", ")
                        );
                        continue;
                    }
                }

                // Insert program.
                let mut tx = pool.begin().await?;
                let pid = uuid::Uuid::new_v4().to_string();
                
                // Check if program exists.
                let existing_id: Option<String> = sqlx::query_scalar("SELECT id FROM programs WHERE name = ?")
                    .bind(&prog.name)
                    .fetch_optional(&mut *tx)
                    .await?;

                let pid = if let Some(ref existing_id) = existing_id {
                    // Update existing program.
                    sqlx::query("UPDATE programs SET description = ? WHERE id = ?")
                        .bind(prog.description.as_deref())
                        .bind(&existing_id)
                        .execute(&mut *tx)
                        .await?;

                    // Delete existing blocks and exercises.
                    sqlx::query("DELETE FROM program_blocks WHERE program_id = ?")
                        .bind(&existing_id)
                        .execute(&mut *tx)
                        .await?;

                    existing_id
                } else {
                    // Insert new program.
                    sqlx::query("INSERT INTO programs (id,name,description,created_at) VALUES (?1,?2,?3,datetime('now'))")
                        .bind(&pid)
                        .bind(&prog.name)
                        .bind(prog.description.as_deref())
                        .execute(&mut *tx)
                        .await?;
                    &pid
                };

                // Insert blocks & exercises.
                for b in prog.blocks {
                    let bid = uuid::Uuid::new_v4().to_string();
                    sqlx::query("INSERT INTO program_blocks (id,program_id,name,description) VALUES (?1,?2,?3,?4)")
                        .bind(&bid).bind(&pid).bind(&b.name).bind(b.description.as_deref())
                        .execute(&mut *tx).await?;
                    let mut seen = HashSet::new();
                    for (idx, ex) in b.exercises.into_iter().enumerate() {
                        if !seen.insert(ex.name.clone()) {
                            println!(
                                "{} duplicate `{}` in block `{}`—skipped",
                                "warning:".yellow().bold(),
                                ex.name,
                                b.name
                            );
                            continue;
                        }
                        let ex_id: String =
                            sqlx::query_scalar("SELECT id FROM exercises WHERE name=?")
                                .bind(&ex.name)
                                .fetch_one(&mut *tx)
                                .await?;
                        sqlx::query("INSERT INTO program_exercises (id,program_block_id,exercise_id,sets,reps,target_rpe,target_rm_percent,notes,program_1rm,technique,technique_group,order_index) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)")
                            .bind(uuid::Uuid::new_v4().to_string())
                            .bind(&bid)
                            .bind(&ex_id)
                            .bind(ex.sets as i32)
                            .bind(ex.reps.map(|v|v.join(",")))
                            .bind(ex.target_rpe.map(|v| v.into_iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")))
                            .bind(ex.target_rm_percent.map(|v| v.into_iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")))
                            .bind(ex.notes.as_deref())
                            .bind(ex.program_1rm)
                            .bind(ex.technique.as_deref())
                            .bind(ex.group.map(|g|g as i32))
                            .bind(idx as i32)
                            .execute(&mut *tx).await?;
                    }
                }
                tx.commit().await?;
                if existing_id.is_some() {
                    println!("{} `{}` updated", "ok:".green().bold(), prog.name);
                } else {
                    println!("{} `{}`", "ok:".green().bold(), prog.name);
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

        ProgramCmd::Show { program } => {
            // Figure out the real UUID for this program.
            let prog_id: String = if let Ok(idx) = program.parse::<i64>() {
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
                .await {
                    Ok(id) => id,
                    Err(_) => {
                        println!("{} no program at index {}", "error:".red().bold(), idx);
                        return Ok(());
                    }
                }
            } else {
                // User passed a name - look up by exact name.
                match sqlx::query_scalar("SELECT id FROM programs WHERE name = ?")
                    .bind(&program)
                    .fetch_one(pool)
                    .await {
                    Ok(id) => id,
                    Err(_) => {
                        println!("{} no program named `{}`", "error:".red().bold(), program);
                        return Ok(());
                    }
                }
            };

            // Fetch the program's metadata.
            let (name, desc, created) = sqlx::query_as::<_, (String, String, String)>(
                r#"
                SELECT name, COALESCE(description,''), created_at
                FROM programs
                WHERE id = ?
                "#,
            )
            .bind(&prog_id)
            .fetch_one(pool)
            .await?;

            if !desc.is_empty() {
                println!(
                    "{} {} — {} (added {})",
                    "Program:".cyan().bold(),
                    name.bold(),
                    desc.dimmed(),
                    &created[..10]
                );
            } else {
                println!(
                    "{} {} (added {})",
                    "Program:".cyan().bold(),
                    name.bold(),
                    &created[..10]
                );
            }

            // Fetch its blocks in order.
            let blocks = sqlx::query_as::<_, (String,String)>(
                "SELECT name, COALESCE(description,'') FROM program_blocks WHERE program_id = ? ORDER BY name",
            )
            .bind(&prog_id)
            .fetch_all(pool)
            .await?;

            if blocks.is_empty() {
                println!("{} no blocks defined)", "warning".yellow().bold());
            } else {
                println!("{}", "Blocks:".cyan().bold());
                
                for (i, (block_name, block_desc)) in blocks.into_iter().enumerate() {
                    let idx = format!("{}", i + 1).yellow();
                    let desc = if !block_desc.is_empty() {
                        format!(" — {}", block_desc).dimmed().to_string()
                    } else {
                        String::new()
                    };
                    println!("{} • {}{}", idx, block_name.bold(), desc);
                    
                    // Fetch the exercises in that block.
                    let exs = sqlx::query_as::<_, (i32, String, i32)>(
                        r#"
                        SELECT pe.order_index,
                               e.name,
                               pe.sets
                      FROM program_exercises pe
                      JOIN exercises e
                        ON e.id = pe.exercise_id
                     WHERE pe.program_block_id = (
                       SELECT id
                         FROM program_blocks
                        WHERE program_id = ? AND name = ?
                            LIMIT 1
                         )
                      ORDER BY pe.order_index
                        "#,
                    )
                    .bind(&prog_id)
                    .bind(&block_name)
                    .fetch_all(pool)
                    .await?;

                    for (order, ex_name, sets) in exs.clone() {
                        let reps_csv: Option<String> = sqlx::query_scalar(
                            r#"
                            SELECT reps
                              FROM program_exercises pe
                             WHERE pe.program_block_id = (
                               SELECT id
                                 FROM program_blocks
                                WHERE program_id = ? AND name = ?
                                LIMIT 1
                              )
                           AND pe.exercise_id = (
                               SELECT e.id FROM exercises e WHERE e.name = ?
                             )
                            "#,
                        )
                        .bind(&prog_id)
                        .bind(&block_name)
                        .bind(&ex_name)
                        .fetch_one(pool)
                        .await?;

                        // format the reps into a nicer "(5, 6–10, 15 reps)" if present
                        let reps_display = reps_csv
                            .map(|csv| {
                                let pretty = csv
                                    .split(',')
                                    .map(|s| s.trim())
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                format!(" ({pretty} reps)")
                            })
                            .unwrap_or_default();

                        let connector = if order + 1 == exs.len() as i32 {
                            "└─"
                        } else {
                            "├─"
                        };
                        let idx = format!("{}", order + 1).yellow();

                        println!(
                            " {} {} {} • {} -> {} sets{}",
                            " ".repeat(2),
                            connector,
                            idx,
                            ex_name.bold(),
                            sets,
                            reps_display
                        );
                    }
                }
            }
        }

        ProgramCmd::Delete { program } => {
            // Figure out the real UUID for this program.
            let prog_id: String = if let Ok(idx) = program.parse::<i64>() {
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
                .await {
                    Ok(id) => id,
                    Err(_) => {
                        println!("{} no program at index {}", "error:".red().bold(), idx);
                        return Ok(());
                    }
                }
            } else {
                // User passed a name - look up by exact name.
                match sqlx::query_scalar("SELECT id FROM programs WHERE name = ?")
                    .bind(&program)
                    .fetch_one(pool)
                    .await {
                    Ok(id) => id,
                    Err(_) => {
                        println!("{} no program named `{}`", "error:".red().bold(), program);
                        return Ok(());
                    }
                }
            };

            // Get program name for confirmation message.
            let name: String = sqlx::query_scalar("SELECT name FROM programs WHERE id = ?")
                .bind(&prog_id)
                .fetch_one(pool)
                .await?;

            // Delete the program (cascade will handle blocks and exercises as well).
            sqlx::query("DELETE FROM programs WHERE id = ?")
                .bind(&prog_id)
                .execute(pool)
                .await?;

            println!("{} deleted program `{}`", "ok:".green().bold(), name);
        }
    }
    Ok(())
}
