use std::str::FromStr;

use anyhow::Result;
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

pub type DB = SqlitePool;

pub async fn open(path: &str) -> Result<DB> {
    let opts = SqliteConnectOptions::from_str(path)?.create_if_missing(true);

    Ok(SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?)
}
