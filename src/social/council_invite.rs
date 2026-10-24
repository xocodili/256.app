//! Council AI platform invitations (review-routed during rollout).
//!
//! All outbound recruitment mail is delivered to [`Config::council_invite_review_email`]
//! with the intended recipient named in the body until live delivery is product-enabled.

use sqlx::MySqlPool;

use crate::config::Config;
use crate::email::categories::council_invite::send_council_invite_to_review_inbox;
use crate::email::initiator;
use crate::social::recruitment::parse_recipient_email;

/// Deliver a council platform invite to the operator review inbox.
pub async fn deliver_platform_invite_for_review(
    pool: &MySqlPool,
    config: &Config,
    council_entity_id: u64,
    intended_to_email: &str,
    intended_recipient_label: Option<&str>,
    personal_note: Option<&str>,
) -> Result<(), String> {
    let intended_email = parse_recipient_email(intended_to_email).ok_or_else(|| {
        "invalid intended recipient email for council invite".to_string()
    })?;
    let initiator = initiator::resolve_social_entity(pool, council_entity_id)
        .await
        .map_err(|e| format!("council initiator not found: {e}"))?;
    send_council_invite_to_review_inbox(
        config,
        &initiator,
        &intended_email,
        intended_recipient_label,
        personal_note,
    )
    .await
}
