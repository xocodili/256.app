//! SMTP transport — multipart plain + HTML alternative.

use std::time::Duration;

use lettre::message::header::ContentType;
use lettre::message::Mailbox;
use lettre::message::{MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::Tls;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::config::SmtpConfig;

/// Wall-clock cap on SMTP round-trips so job handlers cannot block the conductor indefinitely.
const SMTP_SEND_TIMEOUT_SECS: u64 = 90;

pub async fn send_plain_email(
    smtp: &SmtpConfig,
    to_email: &str,
    subject: &str,
    body_plain: &str,
) -> Result<(), String> {
    send_email(smtp, to_email, subject, body_plain, None).await
}

pub async fn send_email(
    smtp: &SmtpConfig,
    to_email: &str,
    subject: &str,
    body_plain: &str,
    body_html: Option<&str>,
) -> Result<(), String> {
    let send = send_email_inner(smtp, to_email, subject, body_plain, body_html);
    match tokio::time::timeout(Duration::from_secs(SMTP_SEND_TIMEOUT_SECS), send).await {
        Ok(result) => result,
        Err(_) => Err(format!(
            "smtp send timed out after {SMTP_SEND_TIMEOUT_SECS}s"
        )),
    }
}

async fn send_email_inner(
    smtp: &SmtpConfig,
    to_email: &str,
    subject: &str,
    body_plain: &str,
    body_html: Option<&str>,
) -> Result<(), String> {
    let from: Mailbox = smtp.from.parse::<Mailbox>().map_err(|e| e.to_string())?;
    let to: Mailbox = to_email.parse::<Mailbox>().map_err(|e| e.to_string())?;

    let message = match body_html.filter(|h| !h.trim().is_empty()) {
        Some(html) => Message::builder()
            .from(from)
            .to(to)
            .subject(subject)
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(body_plain.to_string()),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html.to_string()),
                    ),
            )
            .map_err(|e: lettre::error::Error| e.to_string())?,
        None => Message::builder()
            .from(from)
            .to(to)
            .subject(subject)
            .header(ContentType::TEXT_PLAIN)
            .body(body_plain.to_string())
            .map_err(|e: lettre::error::Error| e.to_string())?,
    };

    build_transport(smtp)?
        .send(message)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn build_transport(
    smtp: &SmtpConfig,
) -> Result<AsyncSmtpTransport<Tokio1Executor>, String> {
    let mut builder = match smtp.tls_mode.as_str() {
        "tls" => {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp.host).map_err(|e| e.to_string())?
        }
        "none" => {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&smtp.host).tls(Tls::None)
        }
        _ => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp.host)
            .map_err(|e| e.to_string())?,
    };

    builder = builder.port(smtp.port);

    if let Some(user) = &smtp.username {
        builder = builder.credentials(Credentials::new(
            user.clone(),
            smtp.password.clone().unwrap_or_default(),
        ));
    }

    Ok(builder.build())
}
