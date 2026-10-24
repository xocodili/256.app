//! Resolve the deployed application tree (repo root on the server).
//!
//! Release binaries built on CI embed `env!("CARGO_MANIFEST_DIR")` from the runner workspace,
//! which does not exist on the VM. Prefer `APP_ROOT`, the process working directory
//! (systemd `WorkingDirectory`), or walking up from `current_exe`.

use std::path::{Path, PathBuf};

fn has_locale_catalog(root: &Path) -> bool {
    root.join("locales").join("en.json").is_file()
}

/// Directory containing `locales/`, `static/`, `agents/`, etc.
#[must_use]
pub fn runtime_app_root() -> PathBuf {
    if let Ok(raw) = std::env::var("APP_ROOT") {
        let root = PathBuf::from(raw.trim());
        if has_locale_catalog(&root) {
            return root;
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        if has_locale_catalog(&cwd) {
            return cwd;
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(PathBuf::from);
        for _ in 0..8 {
            if let Some(ref d) = dir {
                if has_locale_catalog(d) {
                    return d.clone();
                }
                dir = d.parent().map(PathBuf::from);
            } else {
                break;
            }
        }
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
