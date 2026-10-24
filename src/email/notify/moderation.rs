//! Emergency safety and civic moderation notification mail.

use crate::config::Config;
use crate::social::judicial::VerdictOutcome;
use sqlx::MySqlPool;

use super::{send_localized_plain, user_email};

/// Author notice: post suppressed by emergency safety filing; contest path included.
pub async fn notify_emergency_filed_to_author(
    pool: &MySqlPool,
    config: &Config,
    author_user_id: u64,
    root_post_id: u64,
) {
    let Some(email) = user_email(pool, author_user_id).await else {
        return;
    };
    let contest_url = format!(
        "{}/home/judicial/post/{}/contest",
        config.public_base_url.trim_end_matches('/'),
        root_post_id
    );
    send_localized_plain(
        config,
        &email,
        "email.emergency_filed_author_subject",
        "email.emergency_filed_author_body",
        &[
            ("post_id", &root_post_id.to_string()),
            ("contest_url", contest_url.as_str()),
        ],
    )
    .await;
}

/// Filer notice: emergency safety case recorded; post suppressed pending contest window.
pub async fn notify_emergency_filed_to_filer(
    pool: &MySqlPool,
    config: &Config,
    filer_user_id: u64,
    root_post_id: u64,
    case_id: u64,
) {
    let Some(email) = user_email(pool, filer_user_id).await else {
        return;
    };
    send_localized_plain(
        config,
        &email,
        "email.emergency_filed_filer_subject",
        "email.emergency_filed_filer_body",
        &[
            ("post_id", &root_post_id.to_string()),
            ("case_id", &case_id.to_string()),
        ],
    )
    .await;
}

/// Author notice: contest accepted; jury will convene.
pub async fn notify_emergency_contest_opened(
    pool: &MySqlPool,
    config: &Config,
    author_user_id: u64,
    case_id: u64,
) {
    let Some(email) = user_email(pool, author_user_id).await else {
        return;
    };
    let home_url = format!("{}/home", config.public_base_url.trim_end_matches('/'));
    send_localized_plain(
        config,
        &email,
        "email.emergency_contest_opened_subject",
        "email.emergency_contest_opened_body",
        &[
            ("case_id", &case_id.to_string()),
            ("home_url", home_url.as_str()),
        ],
    )
    .await;
}

/// Outcome mail after a contested emergency jury verdict closes.
pub async fn notify_emergency_contest_verdict_outcomes(
    pool: &MySqlPool,
    config: &Config,
    case_id: u64,
    verdict: VerdictOutcome,
) {
    let row: Option<(Option<u64>, Option<u64>, u64)> = sqlx::query_as(
        r#"
        SELECT jc.created_by, se.created_by, COALESCE(p.root_post_id, p.id) AS root_post_id
        FROM judicial_case jc
        INNER JOIN post p ON p.id = jc.subject_post_id
        INNER JOIN social_entity se ON se.id = jc.target_social_entity_id
        WHERE jc.id = ?
          AND jc.contested_at IS NOT NULL
        LIMIT 1
        "#,
    )
    .bind(case_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let Some((filer_user_id, author_user_id, root_post_id)) = row else {
        return;
    };

    let home_url = format!("{}/home", config.public_base_url.trim_end_matches('/'));
    let pairs = [
        ("case_id", case_id.to_string()),
        ("post_id", root_post_id.to_string()),
        ("home_url", home_url.clone()),
    ];
    let pair_refs: Vec<(&str, &str)> = pairs
        .iter()
        .map(|(k, v)| (*k, v.as_str()))
        .collect();

    match verdict {
        VerdictOutcome::Overturned => {
            if let Some(user_id) = author_user_id {
                if let Some(email) = user_email(pool, user_id).await {
                    send_localized_plain(
                        config,
                        &email,
                        "email.emergency_contest_author_won_subject",
                        "email.emergency_contest_author_won_body",
                        &pair_refs,
                    )
                    .await;
                }
            }
            if let Some(user_id) = filer_user_id {
                if let Some(email) = user_email(pool, user_id).await {
                    send_localized_plain(
                        config,
                        &email,
                        "email.emergency_contest_filer_lost_subject",
                        "email.emergency_contest_filer_lost_body",
                        &pair_refs,
                    )
                    .await;
                }
            }
        }
        VerdictOutcome::Upheld | VerdictOutcome::Hung => {
            if let Some(user_id) = author_user_id {
                if let Some(email) = user_email(pool, user_id).await {
                    send_localized_plain(
                        config,
                        &email,
                        "email.emergency_contest_author_lost_subject",
                        "email.emergency_contest_author_lost_body",
                        &pair_refs,
                    )
                    .await;
                }
            }
            if let Some(user_id) = filer_user_id {
                if let Some(email) = user_email(pool, user_id).await {
                    send_localized_plain(
                        config,
                        &email,
                        "email.emergency_contest_filer_won_subject",
                        "email.emergency_contest_filer_won_body",
                        &pair_refs,
                    )
                    .await;
                }
            }
        }
    }
}

/// Author notice: uncontested emergency passed T+3; standing penalties applied.
pub async fn notify_emergency_standing_applied(
    pool: &MySqlPool,
    config: &Config,
    author_user_id: u64,
    root_post_id: u64,
) {
    let Some(email) = user_email(pool, author_user_id).await else {
        return;
    };
    send_localized_plain(
        config,
        &email,
        "email.emergency_standing_applied_subject",
        "email.emergency_standing_applied_body",
        &[("post_id", &root_post_id.to_string())],
    )
    .await;
}
