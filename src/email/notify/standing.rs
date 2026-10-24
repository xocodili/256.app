//! Graph standing and login-inactivity notification mail.

use crate::config::Config;
use sqlx::MySqlPool;

use super::{send_localized_plain, user_email};

/// Email all active owners when a community's global rating drops below 1.
pub async fn notify_community_failing_admins(
    pool: &MySqlPool,
    config: &Config,
    community_entity_id: u64,
    community_handle: &str,
    community_label: Option<&str>,
) {
    let Some(_smtp) = config.smtp.as_ref() else {
        return;
    };
    let label = community_label.unwrap_or(community_handle);
    let profile_url = format!(
        "{}/home/profile/{}",
        config.public_base_url.trim_end_matches('/'),
        community_handle
    );

    let owners: Vec<(u64, String)> = match sqlx::query_as(
        r#"
        SELECT u.id, u.email
        FROM user_social_entity_role ur
        INNER JOIN users u ON u.id = ur.user_id
        INNER JOIN ref_key_values rk
            ON rk.id = ur.role_id
           AND rk.`group` = 'user_social_entity_role'
           AND rk.`key` = 'owner'
        WHERE ur.social_entity_id = ?
          AND (ur.ends_at IS NULL OR ur.ends_at >= UTC_DATE())
          AND u.email_verified_at IS NOT NULL
          AND TRIM(u.email) <> ''
        "#,
    )
    .bind(community_entity_id)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(err) => {
            tracing::warn!(error = %err, community_entity_id, "community failing admin lookup failed");
            return;
        }
    };

    for (user_id, email) in owners {
        send_localized_plain(
            config,
            &email,
            "email.community_failing_admin_subject",
            "email.community_failing_admin_body",
            &[
                ("community_label", label),
                ("community_handle", community_handle),
                ("profile_url", profile_url.as_str()),
            ],
        )
        .await;
        let _ = user_id;
    }
}

/// Email a human when their profile's global rating drops below 1.
pub async fn notify_human_global_failing(
    pool: &MySqlPool,
    config: &Config,
    user_id: u64,
    handle: &str,
    display_label: Option<&str>,
) {
    let Some(email) = user_email(pool, user_id).await else {
        return;
    };
    let label = display_label.unwrap_or(handle);
    let profile_url = format!(
        "{}/home/profile/{}",
        config.public_base_url.trim_end_matches('/'),
        handle
    );
    send_localized_plain(
        config,
        &email,
        "email.human_global_failing_subject",
        "email.human_global_failing_body",
        &[("label", label), ("profile_url", profile_url.as_str())],
    )
    .await;
}

/// Email a human when their global rating returns to the healthy band (>= 0.66).
pub async fn notify_human_global_recovery(
    pool: &MySqlPool,
    config: &Config,
    user_id: u64,
    handle: &str,
    display_label: Option<&str>,
) {
    let Some(email) = user_email(pool, user_id).await else {
        return;
    };
    let label = display_label.unwrap_or(handle);
    let profile_url = format!(
        "{}/home/profile/{}",
        config.public_base_url.trim_end_matches('/'),
        handle
    );
    let how_url = format!(
        "{}/login?legal=how#civic-standing",
        config.public_base_url.trim_end_matches('/')
    );
    send_localized_plain(
        config,
        &email,
        "email.human_global_recovery_subject",
        "email.human_global_recovery_body",
        &[
            ("label", label),
            ("how_url", how_url.as_str()),
            ("profile_url", profile_url.as_str()),
        ],
    )
    .await;
}

/// Email a member when their standing in a community drops below 1.
pub async fn notify_human_community_edge_failing(
    _pool: &MySqlPool,
    config: &Config,
    _user_id: u64,
    email: &str,
    community_handle: &str,
    community_label: Option<&str>,
) {
    let label = community_label.unwrap_or(community_handle);
    let profile_url = format!(
        "{}/home/profile/{}",
        config.public_base_url.trim_end_matches('/'),
        community_handle
    );
    send_localized_plain(
        config,
        email,
        "email.human_community_edge_failing_subject",
        "email.human_community_edge_failing_body",
        &[
            ("community_label", label),
            ("profile_url", profile_url.as_str()),
        ],
    )
    .await;
}

/// Email at 21 idle days — one week before decrement phase begins.
pub async fn notify_human_inactivity_warning(config: &Config, email: &str, display_label: &str) {
    send_inactivity_email(
        config,
        email,
        display_label,
        "email.human_inactivity_warning_subject",
        "email.human_inactivity_warning_body",
        None,
    )
    .await;
}

/// Email when daily decrements begin (28 idle days).
pub async fn notify_human_inactivity_decrement_started(
    config: &Config,
    email: &str,
    display_label: &str,
) {
    send_inactivity_email(
        config,
        email,
        display_label,
        "email.human_inactivity_decrement_started_subject",
        "email.human_inactivity_decrement_started_body",
        None,
    )
    .await;
}

/// Email when decrement phase enters 3× rate (decrement day 22).
pub async fn notify_human_inactivity_acceleration_3x(
    config: &Config,
    email: &str,
    display_label: &str,
) {
    send_inactivity_email(
        config,
        email,
        display_label,
        "email.human_inactivity_acceleration_3x_subject",
        "email.human_inactivity_acceleration_3x_body",
        None,
    )
    .await;
}

/// Email when decrement phase enters 6× rate (decrement day 43).
pub async fn notify_human_inactivity_acceleration_6x(
    config: &Config,
    email: &str,
    display_label: &str,
) {
    send_inactivity_email(
        config,
        email,
        display_label,
        "email.human_inactivity_acceleration_6x_subject",
        "email.human_inactivity_acceleration_6x_body",
        None,
    )
    .await;
}

/// Email when login inactivity triggers the first weekly rating penalty (21+ days idle).
pub async fn notify_human_inactivity_pruning(
    config: &Config,
    email: &str,
    display_label: &str,
    estimated_deletion_date: &str,
) {
    send_inactivity_email(
        config,
        email,
        display_label,
        "email.human_inactivity_pruning_subject",
        "email.human_inactivity_pruning_body",
        Some(("estimated_deletion_date", estimated_deletion_date)),
    )
    .await;
}

async fn send_inactivity_email(
    config: &Config,
    email: &str,
    display_label: &str,
    subject_key: &str,
    body_key: &str,
    extra: Option<(&str, &str)>,
) {
    let home_url = format!("{}/home", config.public_base_url.trim_end_matches('/'));
    let mut pairs = vec![
        ("label", display_label),
        ("home_url", home_url.as_str()),
    ];
    if let Some((k, v)) = extra {
        pairs.push((k, v));
    }
    send_localized_plain(config, email, subject_key, body_key, &pairs).await;
}
