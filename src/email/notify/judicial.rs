//! Judicial workflow notification mail (human jury summons).

use crate::config::Config;
use crate::i18n;
use crate::social::judicial::CreatedJurorInvite;
use sqlx::MySqlPool;

use super::user_email;
use crate::email::dispatch;
use crate::email::render::{render, MailTemplate};

/// Send jury-summons emails for invites created in a nomination pass. No-op when SMTP is unset.
pub async fn notify_juror_invites_created(
    pool: &MySqlPool,
    config: &Config,
    invites: &[CreatedJurorInvite],
) {
    let Some(smtp) = config.smtp.as_ref() else {
        return;
    };
    if invites.is_empty() {
        return;
    }

    let c = i18n::catalog();
    let loc = i18n::request_locale();
    let home_url = format!("{}/home", config.public_base_url.trim_end_matches('/'));

    for inv in invites {
        let Some(email) = user_email(pool, inv.juror_user_id).await else {
            continue;
        };

        let mail = render(
            &MailTemplate::JurorSummons {
                home_url: home_url.clone(),
                invite_id: inv.invite_id.to_string(),
            },
            &loc,
            &c,
            &config.public_base_url,
        );
        if let Err(err) = dispatch::send_with_smtp(smtp, &email, &mail).await {
            tracing::warn!(
                user_id = inv.juror_user_id,
                invite_id = inv.invite_id,
                error = %err,
                "juror summons email failed"
            );
        }
    }
}
