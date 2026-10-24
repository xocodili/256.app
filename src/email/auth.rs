//! **Auth** transactional mail (verification, password reset) — system initiator, not social entity.

use crate::config::SmtpConfig;
use crate::email::dispatch;
use crate::email::render::{render, MailTemplate};
use crate::i18n;

/// Send a verification link (system initiator).
pub async fn send_verification_email(
    smtp: &SmtpConfig,
    public_base_url: &str,
    to_email: &str,
    verify_url: &str,
) -> Result<(), String> {
    let c = i18n::catalog();
    let loc = i18n::request_locale();
    let mail = render(
        &MailTemplate::VerifyEmail {
            verify_url: verify_url.to_string(),
        },
        &loc,
        &c,
        public_base_url,
    );
    dispatch::send_with_smtp(smtp, to_email, &mail).await
}

/// Send a password reset link (one-time sign-in to change password).
pub async fn send_password_reset_email(
    smtp: &SmtpConfig,
    public_base_url: &str,
    to_email: &str,
    reset_url: &str,
) -> Result<(), String> {
    let c = i18n::catalog();
    let loc = i18n::request_locale();
    let mail = render(
        &MailTemplate::PasswordReset {
            reset_url: reset_url.to_string(),
        },
        &loc,
        &c,
        public_base_url,
    );
    dispatch::send_with_smtp(smtp, to_email, &mail).await
}
