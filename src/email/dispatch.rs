//! **Transport** — rendered mail → SMTP. Category functions never open SMTP directly.

use crate::config::{Config, SmtpConfig};
use crate::email::render::RenderedMail;
use crate::email::transport::send_email;

/// Send using an explicit SMTP profile (auth flows that already hold `&SmtpConfig`).
pub async fn send_with_smtp(
    smtp: &SmtpConfig,
    to_email: &str,
    mail: &RenderedMail,
) -> Result<(), String> {
    if to_email.trim().is_empty() {
        return Err("recipient email empty".to_string());
    }
    send_email(
        smtp,
        to_email,
        &mail.subject,
        &mail.body_plain,
        Some(&mail.body_html),
    )
    .await
}

/// Send when SMTP is configured on `Config`; returns `Ok(())` or a transport error string.
pub async fn send(config: &Config, to_email: &str, mail: &RenderedMail) -> Result<(), String> {
    let smtp = config
        .smtp
        .as_ref()
        .ok_or_else(|| "smtp not configured".to_string())?;
    send_with_smtp(smtp, to_email, mail).await
}

/// Fire-and-forget product notification (logs warning on failure).
pub async fn send_or_log(config: &Config, to_email: &str, mail: &RenderedMail) {
    if let Err(err) = send(config, to_email, mail).await {
        tracing::warn!(error = %err, to = %to_email, "outbound email failed");
    }
}
