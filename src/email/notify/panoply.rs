//! Panoply operator daily status mail.

use crate::config::Config;
use crate::email::dispatch;
use crate::email::render::{render, MailTemplate};
use crate::i18n;
use crate::social::panoply::PanoplyDailyMetrics;

pub async fn send_panoply_daily_status(
    config: &Config,
    email: &str,
    metrics: &PanoplyDailyMetrics,
) {
    let Some(smtp) = config.smtp.as_ref() else {
        return;
    };
    if email.trim().is_empty() {
        return;
    }

    let c = i18n::catalog();
    let loc = i18n::request_locale();
    let ops_url = format!("{}/home/ops", config.public_base_url.trim_end_matches('/'));

    let mail = render(
        &MailTemplate::PanoplyDailyStatus {
            report_date: metrics.report_date.clone(),
            ops_url: ops_url.clone(),
            metrics: metrics.clone(),
        },
        &loc,
        &c,
        &config.public_base_url,
    );

    if let Err(err) = dispatch::send_with_smtp(smtp, email, &mail).await {
        tracing::warn!(
            email = %email,
            error = %err,
            "panoply daily status email failed"
        );
    }
}
