//! Align `_sqlx_migrations.checksum` with on-disk migration files, then run pending migrations.
//!
//! Use when `cargo run` / `sqlx migrate run` fails with VersionMismatch after a migration
//! file was edited post-apply (dev or beta catch-up). Requires `DATABASE_URL`.

use sha2::{Digest, Sha384};
use sqlx::Connection;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL")?;
    let mut conn = sqlx::mysql::MySqlConnection::connect(&url).await?;

    let mut repaired = 0u64;
    for entry in std::fs::read_dir("migrations")? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".sql") {
            continue;
        }
        let version: i64 = name
            .split('_')
            .next()
            .ok_or("migration filename missing version prefix")?
            .parse()?;
        let bytes = std::fs::read(entry.path())?;
        let digest = Sha384::digest(bytes);

        let updated = sqlx::query(
            "UPDATE _sqlx_migrations SET checksum = ? WHERE version = ?",
        )
        .bind(digest.as_slice())
        .bind(version)
        .execute(&mut conn)
        .await?
        .rows_affected();

        if updated > 0 {
            repaired += updated;
            eprintln!("repaired checksum: {name}");
        }
    }

    eprintln!("checksum rows updated: {repaired}");
    sqlx::migrate!("./migrations").run(&mut conn).await?;
    eprintln!("migrations up to date");
    Ok(())
}
