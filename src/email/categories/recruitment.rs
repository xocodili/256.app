//! **Recruitment** — platform invitations (human friend invite, AI outreach, profit client list).
//!
//! Initiator is always a [`ResolvedInitiator`] (any `social_entity` type). Callers in `social::recruitment`
//! enforce permissions before invoking these functions.

use crate::config::Config;
use crate::email::categories::council_invite::send_council_invite_to_review_inbox;
use crate::email::dispatch;
use crate::email::initiator::ResolvedInitiator;
use crate::email::render::{render, MailTemplate, RenderedMail};
use crate::i18n;

#[derive(Debug, Clone)]
pub struct BulkInviteItemResult {
    pub email: String,
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct BulkInviteSummary {
    pub sent: u32,
    pub failed: u32,
    pub skipped_invalid: u32,
    pub results: Vec<BulkInviteItemResult>,
}

fn platform_invite_mail(
    initiator: &ResolvedInitiator,
    join_url: &str,
    personal_note: Option<&str>,
    loc: &str,
    public_base_url: &str,
) -> RenderedMail {
    render(
        &MailTemplate::PlatformInvite {
            inviter_line: initiator.display_line.clone(),
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

/// Single recruitment invite attributed to `initiator`.
pub async fn send_platform_invite(
    config: &Config,
    initiator: &ResolvedInitiator,
    to_email: &str,
    personal_note: Option<&str>,
) -> Result<(), String> {
    if config.platform_recruitment_review_only {
        return send_council_invite_to_review_inbox(
            config,
            initiator,
            to_email,
            None,
            personal_note,
        )
        .await;
    }
    let loc = i18n::request_locale();
    let join_url = format!(
        "{}/login?join=1",
        config.public_base_url.trim_end_matches('/')
    );
    let mail = platform_invite_mail(initiator, &join_url, personal_note, &loc, &config.public_base_url);
    dispatch::send(config, to_email, &mail).await
}

/// Bulk recruitment — dedupes addresses, skips invalid rows, continues on per-recipient failure.
pub async fn send_platform_invites_bulk(
    config: &Config,
    initiator: &ResolvedInitiator,
    raw_emails: &[String],
    personal_note: Option<&str>,
) -> BulkInviteSummary {
    let loc = i18n::request_locale();
    let join_url = format!(
        "{}/login?join=1",
        config.public_base_url.trim_end_matches('/')
    );
    let mail = platform_invite_mail(initiator, &join_url, personal_note, &loc, &config.public_base_url);

    let mut seen = std::collections::HashSet::new();
    let mut summary = BulkInviteSummary::default();

    for raw in raw_emails {
        let email = raw.trim();
        if email.len() < 3 || email.len() > 254 || !email.contains('@') {
            summary.skipped_invalid += 1;
            continue;
        }
        let key = email.to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }
        let send_result = if config.platform_recruitment_review_only {
            send_council_invite_to_review_inbox(config, initiator, email, None, personal_note).await
        } else {
            dispatch::send(config, email, &mail).await
        };
        match send_result {
            Ok(()) => {
                summary.sent += 1;
                summary.results.push(BulkInviteItemResult {
                    email: email.to_string(),
                    ok: true,
                    error: None,
                });
            }
            Err(err) => {
                summary.failed += 1;
                summary.results.push(BulkInviteItemResult {
                    email: email.to_string(),
                    ok: false,
                    error: Some(err),
                });
            }
        }
    }

    summary
}
