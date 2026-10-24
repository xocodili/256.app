//! Outbound **notifications** to humans — lifecycle, judicial, moderation, and AI-triggered mail.
//!
//! Auth flows (verify / reset) live in [`crate::email::auth`]. Everything here is product
//! or system-initiated: graph standing, jury summons, emergency safety, council digests, etc.
//!
//! Callers should prefer these entry points over opening SMTP inline in route handlers or AI workers.

mod ai;
mod engagement;
mod judicial;
mod panoply;
mod moderation;
mod standing;

pub use ai::*;
pub use engagement::*;
pub use judicial::*;
pub use panoply::*;
pub use moderation::*;
pub use standing::*;

use crate::config::Config;
use crate::i18n;

pub(crate) async fn send_localized_plain(
    config: &Config,
    to_email: &str,
    subject_key: &str,
    body_key: &str,
    pairs: &[(&str, &str)],
) {
    if config.smtp.is_none() || to_email.trim().is_empty() {
        return;
    }
    let c = i18n::catalog();
    let loc = i18n::request_locale();
    let template_pairs: Vec<(String, String)> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let mail = crate::email::render::render(
        &crate::email::render::MailTemplate::LocalizedKeys {
            subject_key: subject_key.to_string(),
            body_key: body_key.to_string(),
            pairs: template_pairs,
        },
        &loc,
        &c,
        &config.public_base_url,
    );
    crate::email::dispatch::send_or_log(config, to_email, &mail).await;
}

pub(crate) async fn user_email(pool: &sqlx::MySqlPool, user_id: u64) -> Option<String> {
    sqlx::query_scalar("SELECT email FROM users WHERE id = ? LIMIT 1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .filter(|e: &String| !e.trim().is_empty())
}
