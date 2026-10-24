//! One-time email when global or community-edge ratings first drop below the failing threshold.

use sqlx::MySqlPool;

use crate::config::Config;
use crate::ref_keys;
use crate::social::community_graph::{is_death_immune_handle, is_human_death_immune};
use crate::social::judicial::failing_rating_threshold;

#[derive(Debug, Clone, sqlx::FromRow)]
struct GlobalFailingRow {
    entity_id: u64,
    type_key: String,
    handle: String,
    display_label: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct EdgeFailingRow {
    relationship_id: u64,
    human_entity_id: u64,
    user_id: u64,
    email: String,
    community_handle: String,
    community_label: Option<String>,
}

/// Drain pending failing-rating notifications (at most `limit` per category per call).
pub async fn flush_rating_failing_notifications(pool: &MySqlPool, config: &Config, limit: u64) {
    let lim = limit.clamp(1, 64) as i64;
    if let Err(err) = flush_global_entity_failing(pool, config, lim).await {
        tracing::warn!(error = %err, "flush_global_entity_failing failed");
    }
    if let Err(err) = flush_community_edge_failing(pool, config, lim).await {
        tracing::warn!(error = %err, "flush_community_edge_failing failed");
    }
}

async fn flush_global_entity_failing(
    pool: &MySqlPool,
    config: &Config,
    limit: i64,
) -> Result<(), sqlx::Error> {
    let failing = failing_rating_threshold();
    let rows: Vec<GlobalFailingRow> = sqlx::query_as(
        r#"
        SELECT
            se.id AS entity_id,
            rk_type.`key` AS type_key,
            se.handle,
            COALESCE(
                NULLIF(TRIM(sc.label), ''),
                NULLIF(TRIM(CONCAT_WS(' ', sh.given_name, sh.additional_name, sh.family_name)), ''),
                se.handle
            ) AS display_label
        FROM social_entity se
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = se.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` IN (?, ?)
        LEFT JOIN social_community sc ON sc.social_entity_id = se.id
        LEFT JOIN social_human sh ON sh.social_entity_id = se.id
        WHERE se.rating < ?
          AND se.rating_failing_notified_at IS NULL
        ORDER BY se.id ASC
        LIMIT ?
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_COMMUNITY)
    .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .bind(failing)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    for row in rows {
        if row.type_key == ref_keys::KEY_ENTITY_TYPE_COMMUNITY && is_death_immune_handle(&row.handle) {
            continue;
        }
        if row.type_key == ref_keys::KEY_ENTITY_TYPE_HUMAN
            && is_human_death_immune(pool, row.entity_id)
                .await
                .unwrap_or(true)
        {
            continue;
        }

        if row.type_key == ref_keys::KEY_ENTITY_TYPE_COMMUNITY {
            let claimed = sqlx::query(
                r#"
                UPDATE social_entity
                SET rating_failing_notified_at = UTC_TIMESTAMP(6)
                WHERE id = ?
                  AND rating < ?
                  AND rating_failing_notified_at IS NULL
                "#,
            )
            .bind(row.entity_id)
            .bind(failing)
            .execute(pool)
            .await?
            .rows_affected();

            if claimed == 0 {
                continue;
            }

            crate::email::notify_community_failing_admins(
                pool,
                config,
                row.entity_id,
                &row.handle,
                row.display_label.as_deref(),
            )
            .await;
        } else {
            let user_id: Option<u64> = sqlx::query_scalar(
                r#"
                SELECT u.id
                FROM users u
                INNER JOIN user_social_entity_role ur ON ur.user_id = u.id
                INNER JOIN ref_key_values rk
                    ON rk.id = ur.role_id
                   AND rk.`group` = 'user_social_entity_role'
                   AND rk.`key` = 'owner'
                WHERE ur.social_entity_id = ?
                  AND (ur.ends_at IS NULL OR ur.ends_at >= UTC_DATE())
                ORDER BY ur.is_primary DESC, ur.id ASC
                LIMIT 1
                "#,
            )
            .bind(row.entity_id)
            .fetch_optional(pool)
            .await?;

            let Some(user_id) = user_id else {
                tracing::warn!(
                    human_entity_id = row.entity_id,
                    "global failing notify skipped (no owner role)"
                );
                continue;
            };

            crate::email::notify_human_global_failing(
                pool,
                config,
                user_id,
                &row.handle,
                row.display_label.as_deref(),
            )
            .await;

            if let Err(err) = crate::social::godmother::enqueue_moment(
                pool,
                Some(user_id),
                row.entity_id,
                crate::social::godmother::GodmotherMoment::GlobalFailing,
                Some("global_failing_notify"),
            )
            .await
            {
                tracing::warn!(
                    human_entity_id = row.entity_id,
                    error = %err,
                    "godmother global failing outreach job enqueue failed"
                );
            }

            let claimed = sqlx::query(
                r#"
                UPDATE social_entity
                SET rating_failing_notified_at = UTC_TIMESTAMP(6)
                WHERE id = ?
                  AND rating < ?
                  AND rating_failing_notified_at IS NULL
                "#,
            )
            .bind(row.entity_id)
            .bind(failing)
            .execute(pool)
            .await?
            .rows_affected();

            if claimed == 0 {
                tracing::warn!(
                    human_entity_id = row.entity_id,
                    "global failing notify claim lost after outreach enqueue"
                );
            }
        }
    }
    Ok(())
}

async fn flush_community_edge_failing(
    pool: &MySqlPool,
    config: &Config,
    limit: i64,
) -> Result<(), sqlx::Error> {
    let failing = failing_rating_threshold();
    let rows: Vec<EdgeFailingRow> = sqlx::query_as(
        r#"
        SELECT
            ser.id AS relationship_id,
            human_se.id AS human_entity_id,
            u.id AS user_id,
            u.email,
            se.handle AS community_handle,
            COALESCE(NULLIF(TRIM(sc.label), ''), se.handle) AS community_label
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = ser.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` = ?
        INNER JOIN social_entity se ON se.id = ser.to_social_entity_id
        INNER JOIN social_community sc ON sc.social_entity_id = se.id
        INNER JOIN social_entity human_se ON human_se.id = ser.from_social_entity_id
        INNER JOIN user_social_entity_role ur ON ur.social_entity_id = human_se.id
        INNER JOIN ref_key_values rk_role
            ON rk_role.id = ur.role_id
           AND rk_role.`group` = 'user_social_entity_role'
           AND rk_role.`key` = 'owner'
        INNER JOIN users u ON u.id = ur.user_id
        WHERE ser.rating < ?
          AND ser.ended_at IS NULL
          AND ser.rating_failing_notified_at IS NULL
          AND u.email_verified_at IS NOT NULL
          AND TRIM(u.email) <> ''
        ORDER BY ser.id ASC
        LIMIT ?
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .bind(failing)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    for row in rows {
        if is_death_immune_handle(&row.community_handle)
            || is_human_death_immune(pool, row.human_entity_id)
                .await
                .unwrap_or(true)
        {
            continue;
        }

        let claimed = sqlx::query(
            r#"
            UPDATE social_entity_relationship
            SET rating_failing_notified_at = UTC_TIMESTAMP(6)
            WHERE id = ?
              AND rating < ?
              AND rating_failing_notified_at IS NULL
            "#,
        )
        .bind(row.relationship_id)
        .bind(failing)
        .execute(pool)
        .await?
        .rows_affected();

        if claimed == 0 {
            continue;
        }

        crate::email::notify_human_community_edge_failing(
            pool,
            config,
            row.user_id,
            &row.email,
            &row.community_handle,
            row.community_label.as_deref(),
        )
        .await;
    }
    Ok(())
}
