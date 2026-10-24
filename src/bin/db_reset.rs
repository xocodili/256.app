use sqlx::mysql::MySqlConnection;
use sqlx::Connection;
use sqlx::Executor;

fn database_url_allowed(raw: &str, allow_flag: bool) -> bool {
    if allow_flag {
        return true;
    }
    let lower = raw.to_ascii_lowercase();
    lower.contains("localhost")
        || lower.contains("127.0.0.1")
        || lower.contains("/test")
        || lower.contains("_test")
        || lower.contains("_e2e")
}

fn url_with_database(raw: &str, db: &str) -> String {
    // Replace the URL path segment (database) while preserving query params.
    // Expected forms:
    // - mysql://user:pass@host:3306/dbname
    // - mysql://user:pass@host:3306/dbname?charset=utf8mb4
    let (base, query) = match raw.split_once('?') {
        Some((b, q)) => (b, Some(q)),
        None => (raw, None),
    };
    let slash = base.rfind('/').unwrap_or(base.len());
    let mut out = String::with_capacity(raw.len() + db.len());
    out.push_str(&base[..=slash]); // include trailing '/'
    out.push_str(db);
    if let Some(q) = query {
        out.push('?');
        out.push_str(q);
    }
    out
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env if present (matches app behavior).
    let _ = dotenvy::dotenv();

    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL is not set (check your .env)")?;

    let db_name = database_url
        .split_once('?')
        .map(|(b, _)| b)
        .unwrap_or(database_url.as_str())
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim()
        .to_string();

    if db_name.is_empty() || db_name.contains('@') {
        return Err(format!("Could not parse database name from DATABASE_URL").into());
    }

    let allow_unsafe = std::env::var("DB_RESET_ALLOW")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if !database_url_allowed(&database_url, allow_unsafe) {
        return Err(
            "refusing to reset: set DB_RESET_ALLOW=1 or use a local/test DATABASE_URL".into(),
        );
    }

    eprintln!(
        "Resetting database `{db_name}` (DROP + CREATE; migrations re-apply on next app start)..."
    );

    // Connect to a system database to drop/create the app database.
    // Some MySQL installs restrict access to `mysql`, but `information_schema` is typically readable.
    let admin_url = url_with_database(&database_url, "information_schema");
    let mut conn = MySqlConnection::connect(&admin_url).await?;

    let drop_sql = format!("DROP DATABASE IF EXISTS `{}`", db_name.replace('`', "``"));
    let create_sql = format!(
        "CREATE DATABASE `{}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",
        db_name.replace('`', "``")
    );

    conn.execute(drop_sql.as_str()).await?;
    conn.execute(create_sql.as_str()).await?;

    println!("Reset database `{}` OK.", db_name);
    Ok(())
}

