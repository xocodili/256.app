//! Authorization for `POST /jobs/enqueue` (verified-user HTTP API).
//!
//! System jobs are enqueued only from server handlers (`scheduler`, judicial filing,
//! `select_godmother`, ops conductor). The generic HTTP enqueue route must not expose them.

/// Job types that may be submitted through `POST /jobs/enqueue`.
const USER_HTTP_ENQUEUE_ALLOWLIST: &[&str] = &[];

/// Returns whether a verified user may enqueue this job type via the HTTP API.
#[must_use]
pub fn is_user_http_enqueue_allowed(job_type_key: &str) -> bool {
    let key = job_type_key.trim();
    USER_HTTP_ENQUEUE_ALLOWLIST.iter().any(|allowed| *allowed == key)
}
