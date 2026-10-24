//! Database pool and migrations.
//!
//! # Timestamps
//! MySQL **`TIMESTAMP`** columns: use the [`TimestampUtc`] alias (`chrono::DateTime<Utc>`) with sqlx for both
//! reads and binds. Do not use `NaiveDateTime` for those columns — sqlx treats it as **DATETIME**, which
//! mismatches MySQL `TIMESTAMP` and fails at decode time.
//!
//! # Unicode / emoji
//! Connections must use **`utf8mb4`** (full UTF-8). Legacy MySQL `utf8` is utf8mb3 and cannot store emoji;
//! without an explicit charset, some servers default to utf8mb3 and string binds fail with error 3988.

use std::str::FromStr;

use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};
use sqlx::MySqlPool;

/// UTC instant for MySQL `TIMESTAMP` (sqlx-compatible).
pub type TimestampUtc = chrono::DateTime<chrono::Utc>;

pub async fn connect_and_migrate(database_url: &str) -> Result<MySqlPool, sqlx::Error> {
    let opts = MySqlConnectOptions::from_str(database_url)?
        .charset("utf8mb4")
        .collation("utf8mb4_unicode_ci");
    let pool = MySqlPoolOptions::new()
        .max_connections(10)
        .test_before_acquire(true)
        .connect_with(opts)
        .await?;
    // Embedded migrations at compile time (`migrations/`).
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
