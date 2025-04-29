use anyhow::Result;

use crate::cli::SessionCmd;

pub async fn handle(cmd: SessionCmd, _pool: &sqlx::SqlitePool) -> Result<()> {
    match cmd {
        SessionCmd::Start(_start_args) => println!("we gucci")
    }
    
    Ok(())
}
