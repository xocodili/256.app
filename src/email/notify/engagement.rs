//! Away-user engagement digest mail.

use crate::config::Config;
use crate::email::dispatch;
use crate::email::render::{render, MailTemplate};
use crate::i18n;
use crate::social::engagement_digest::EngagementDigestCounts;

/// Send the away-user engagement digest. No-op when SMTP is unset.
pub async fn send_user_engagement_digest(
    config: &Config,
    email: &str,
    display_label: &str,
    counts: &EngagementDigestCounts,
) {
    let Some(smtp) = config.smtp.as_ref() else {
        return;
    };
    if email.trim().is_empty() || !counts.has_sendable_content() {
        return;
    }

    let c = i18n::catalog();
    let loc = i18n::request_locale();
    let home_url = format!("{}/home", config.public_base_url.trim_end_matches('/'));

    let mail = render(
        &MailTemplate::UserEngagementDigest {
            label: display_label.to_string(),
            home_url: home_url.clone(),
            pending_connections: nonzero(counts.pending_connections),
            audience_mentions: nonzero(counts.audience_mentions),
            replies_to_author: nonzero(counts.replies_to_author),
            missed_feed_posts: nonzero(counts.missed_feed_posts),
            community_posts: if counts.is_community_only() {
                nonzero(counts.community_posts)
            } else {
                None
            },
        },
        &loc,
        &c,
        &config.public_base_url,
    );

    if let Err(err) = dispatch::send_with_smtp(smtp, email, &mail).await {
        tracing::warn!(
            email = %email,
            error = %err,
            "user engagement digest email failed"
        );
    }
}

fn nonzero(n: u32) -> Option<u32> {
    (n > 0).then_some(n)
}
