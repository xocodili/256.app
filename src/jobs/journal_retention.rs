//! systemd journal retention — daily vacuum via `journalctl --vacuum-time`.
//!
//! Belt-and-braces on top of the journald drop-in installed by
//! `scripts/ops/install_journal_retention_sudoers.sh` (`MaxRetentionSec`),
//! which is the primary retention mechanism and needs no sudo at runtime.

use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};

static LAST_ATTEMPT_UNIX: AtomicI64 = AtomicI64::new(0);
const MIN_ATTEMPT_INTERVAL_SECS: i64 = 6 * 60 * 60;

/// Process-local throttle. Queue dedup only spans live `job_queue` rows, so the
/// scheduler re-enqueues this job every tick once the previous run archives —
/// without this gate the vacuum subprocess would spawn roughly once per second.
pub fn should_attempt_now() -> bool {
    let now = chrono::Utc::now().timestamp();
    let last = LAST_ATTEMPT_UNIX.load(Ordering::Relaxed);
    if now.saturating_sub(last) < MIN_ATTEMPT_INTERVAL_SECS {
        return false;
    }
    LAST_ATTEMPT_UNIX
        .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JournalRetentionSummary {
    pub retention_days: u32,
    pub vacuum_attempted: bool,
    pub vacuum_succeeded: bool,
    pub freed_bytes: Option<u64>,
    pub skipped_reason: Option<String>,
}

/// Trim systemd journal entries older than `retention_days` (minimum 1).
#[must_use]
pub fn run_system_journal_retention(retention_days: u32) -> JournalRetentionSummary {
    let retention_days = retention_days.max(1);
    let mut summary = JournalRetentionSummary {
        retention_days,
        ..Default::default()
    };

    if !command_exists("journalctl") {
        summary.skipped_reason = Some("journalctl not available".into());
        return summary;
    }

    summary.vacuum_attempted = true;
    let vacuum_time = format!("{retention_days}d");

    let attempts: &[&[&str]] = &[
        &["sudo", "-n", "journalctl", "--vacuum-time", &vacuum_time],
        &["journalctl", "--vacuum-time", &vacuum_time],
    ];

    for argv in attempts {
        match run_journalctl_vacuum(argv) {
            Ok(output) if output.status.success() => {
                summary.vacuum_succeeded = true;
                summary.freed_bytes = parse_freed_bytes(&output);
                return summary;
            }
            Ok(output) => {
                tracing::debug!(
                    argv = ?argv,
                    code = ?output.status.code(),
                    stderr = %String::from_utf8_lossy(&output.stderr),
                    "journalctl vacuum attempt failed"
                );
            }
            Err(e) => {
                tracing::debug!(argv = ?argv, error = %e, "journalctl vacuum spawn failed");
            }
        }
    }

    summary.skipped_reason = Some(
        "journalctl vacuum failed (install passwordless sudo for journalctl on beta)".into(),
    );
    summary
}

fn command_exists(name: &str) -> bool {
    if cfg!(target_os = "windows") {
        return false;
    }
    Command::new(name)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_journalctl_vacuum(argv: &[&str]) -> std::io::Result<Output> {
    let mut cmd = Command::new(argv[0]);
    for arg in &argv[1..] {
        cmd.arg(arg);
    }
    cmd.output()
}

/// Parse `Vacuuming done, freed 1234567890 bytes` from journalctl stdout/stderr.
#[must_use]
pub fn parse_freed_bytes(output: &Output) -> Option<u64> {
    parse_freed_bytes_text(&format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

fn parse_freed_bytes_text(combined: &str) -> Option<u64> {
    for line in combined.lines() {
        let lower = line.to_ascii_lowercase();
        if !lower.contains("freed") || !lower.contains("byte") {
            continue;
        }
        for token in line.split_whitespace().rev() {
            if let Ok(n) = token.trim_matches(|c: char| !c.is_ascii_digit()).parse::<u64>() {
                if n > 0 {
                    return Some(n);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_freed_bytes_from_journalctl_output() {
        let text = "Vacuuming done, freed 1048576 bytes of archived journals from /var/log/journal.\n";
        assert_eq!(parse_freed_bytes_text(text), Some(1_048_576));
    }

    #[test]
    fn retention_days_clamps_to_at_least_one() {
        let summary = run_system_journal_retention(0);
        assert_eq!(summary.retention_days, 1);
        // vacuum_succeeded may be true on Linux CI where journalctl is available — only
        // assert the clamp, not subprocess outcome.
    }
}
