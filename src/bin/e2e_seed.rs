//! Seed deterministic E2E personas for Playwright front-end tests.
//!
//! ```text
//! E2E_SEED_ALLOW=1 cargo run --bin e2e_seed -- --manifest e2e/fixtures/manifest.json
//! E2E_SEED_ALLOW=1 cargo run --bin e2e_seed -- --reset --manifest e2e/fixtures/manifest.json
//! ```

use app::e2e_seed::{self, SeedOptions};

fn usage() -> &'static str {
    "Usage: e2e_seed [--manifest PATH] [--reset]\n\
     Env: DATABASE_URL (required), E2E_SEED_ALLOW=1 for non-local databases"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();

    let mut manifest_path: Option<String> = None;
    let mut reset = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--manifest" => {
                manifest_path = Some(
                    args.next()
                        .ok_or("--manifest requires a path")?,
                );
            }
            "--reset" => reset = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(());
            }
            other => return Err(format!("unknown argument: {other}\n{}", usage()).into()),
        }
    }

    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL is not set (check your .env)")?;
    let allow_unsafe = std::env::var("E2E_SEED_ALLOW")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    e2e_seed::run(SeedOptions {
        database_url,
        manifest_path,
        reset,
        allow_unsafe,
    })
    .await?;
    Ok(())
}
