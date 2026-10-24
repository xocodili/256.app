//! **Council AI recruitment** — platform invites routed to a review inbox during rollout.
//!
//! Intended recipients are embedded in the message so operators can audit council
//! outreach before live delivery is enabled.

use crate::config::Config;
use crate::email::dispatch;
use crate::email::initiator::ResolvedInitiator;
use crate::email::render::{render, MailTemplate, RenderedMail};
use crate::i18n;

fn council_invite_review_mail(
    initiator: &ResolvedInitiator,
    intended_recipient_email: &str,
    intended_recipient_label: Option<&str>,
    join_url: &str,
    personal_note: Option<&str>,
    loc: &str,
    public_base_url: &str,
) -> RenderedMail {
    render(
        &MailTemplate::CouncilInviteReview {
            inviter_line: initiator.display_line.clone(),
            intended_recipient_email: intended_recipient_email.to_string(),
            intended_recipient_label: intended_recipient_label
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            join_url: join_url.to_string(),
            personal_note: personal_note
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(str::to_string),
        },
        loc,
        i18n::catalog(),
        public_base_url,
    )
}

/// Send a council-attributed platform invite to the configured **review inbox**
/// (not to `intended_recipient_email`). The body names who should have received it.
pub async fn send_council_invite_to_review_inbox(
    config: &Config,
    initiator: &ResolvedInitiator,
    intended_recipient_email: &str,
    intended_recipient_label: Option<&str>,
    personal_note: Option<&str>,
) -> Result<(), String> {
    let review_inbox = config.council_invite_review_email.trim();
    if review_inbox.is_empty() {
        return Err("council invite review inbox is not configured".to_string());
    }
    let loc = i18n::request_locale();
    let join_url = format!(
        "{}/login?join=1",
        config.public_base_url.trim_end_matches('/')
    );
    let mail = council_invite_review_mail(
        initiator,
        intended_recipient_email,
        intended_recipient_label,
        &join_url,
        personal_note,
        &loc,
        &config.public_base_url,
    );
    tracing::info!(
        review_inbox = %review_inbox,
        intended_recipient = %intended_recipient_email,
        "platform invite routed to review inbox (not sent to intended recipient)"
    );
    dispatch::send(config, review_inbox, &mail).await
}
