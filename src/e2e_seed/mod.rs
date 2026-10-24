mod inserts;
pub mod manifest;
pub mod personas;

use std::fs;
use std::path::Path;

pub use manifest::E2eManifest;

use crate::db;
use inserts::{database_url_allowed, seed_all};

pub struct SeedOptions {
    pub database_url: String,
    pub manifest_path: Option<String>,
    pub reset: bool,
    pub allow_unsafe: bool,
}

pub async fn run(opts: SeedOptions) -> Result<E2eManifest, String> {
    if !database_url_allowed(&opts.database_url, opts.allow_unsafe) {
        return Err(
            "refusing to seed: set E2E_SEED_ALLOW=1 or use a local/test DATABASE_URL".to_string(),
        );
    }

    let pool = db::connect_and_migrate(&opts.database_url)
        .await
        .map_err(|e| format!("connect/migrate DATABASE_URL: {e}"))?;

    let manifest = seed_all(&pool, opts.reset).await?;

    if let Some(path) = opts.manifest_path {
        write_manifest(&path, &manifest)?;
        eprintln!("wrote manifest → {path}");
    }

    eprintln!(
        "e2e seed ok: full={} consent={} partial={}",
        manifest.personas.full.email, manifest.personas.consent.email, manifest.personas.partial.email
    );

    pool.close().await;
    Ok(manifest)
}

pub fn write_manifest(path: &str, manifest: &E2eManifest) -> Result<(), String> {
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| format!("create manifest dir: {e}"))?;
        }
    }
    let json = serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| format!("write manifest: {e}"))?;
    Ok(())
}
