use std::str::FromStr;

use sqlx::mysql::MySqlConnectOptions;
use sqlx::Connection;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    let db_url = std::env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL is required for migration repair")?;
    let opts = MySqlConnectOptions::from_str(&db_url)?
        .charset("utf8mb4")
        .collation("utf8mb4_unicode_ci");
    let mut conn = sqlx::MySqlConnection::connect_with(&opts).await?;

    let version: i64 = std::env::var("REPAIR_MIGRATION_VERSION")
        .unwrap_or_else(|_| "20260416100000".to_string())
        .parse()
        .map_err(|_| "REPAIR_MIGRATION_VERSION must be numeric")?;
    let checksum_hex = std::env::var("REPAIR_MIGRATION_CHECKSUM").ok();

    if std::env::var("REPAIR_MIGRATION_DELETE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        sqlx::query("DELETE FROM _sqlx_migrations WHERE version = ?")
            .bind(version)
            .execute(&mut conn)
            .await?;
        println!("deleted migration row {version}");
        return Ok(());
    }

    if let Some(cs) = checksum_hex.as_deref() {
        sqlx::query(
            r#"
            UPDATE _sqlx_migrations
            SET success = 1,
                checksum = UNHEX(?)
            WHERE version = ?
            "#,
        )
        .bind(cs)
        .bind(version)
        .execute(&mut conn)
        .await?;
    } else {
        sqlx::query(
            r#"
            UPDATE _sqlx_migrations
            SET success = 1
            WHERE version = ?
            "#,
        )
        .bind(version)
        .execute(&mut conn)
        .await?;
    }

    let status: Option<i64> =
        sqlx::query_scalar("SELECT success FROM _sqlx_migrations WHERE version = ?")
            .bind(version)
            .fetch_optional(&mut conn)
            .await?;

    println!("migration {version} success={status:?}");
    Ok(())
}
