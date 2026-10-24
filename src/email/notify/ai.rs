//! AI-triggered outbound mail.
//!
//! Council workers, godmother flows, and scheduled AI jobs should call functions in this
//! module (or add new ones here) rather than opening SMTP inline under `src/ai/`.
//!
//! Pattern: resolve recipient from `users` / case context → build i18n body →
//! [`super::send_localized_plain`].

use crate::config::Config;
use sqlx::MySqlPool;

use super::{send_localized_plain, user_email};

/// Placeholder hook for future council decision digests pushed to case parties.
pub async fn notify_ai_council_decision_available(
    pool: &MySqlPool,
    config: &Config,
    recipient_user_id: u64,
    case_id: u64,
) {
    let Some(email) = user_email(pool, recipient_user_id).await else {
        return;
    };
    let home_url = format!("{}/home", config.public_base_url.trim_end_matches('/'));
    send_localized_plain(
        config,
        &email,
        "email.ai_council_decision_subject",
        "email.ai_council_decision_body",
        &[
            ("case_id", &case_id.to_string()),
            ("home_url", home_url.as_str()),
        ],
    )
    .await;
}
