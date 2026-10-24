//! Owner lookup + recovery notification finalize (email + dedupe timestamp).

use sqlx::MySqlPool;

use crate::config::Config;
use crate::ref_keys;
use crate::social::judicial::failing_rating_threshold;

pub async fn lookup_human_owner_user_id(
    pool: &MySqlPool,
    human_entity_id: u64,
) -> Result<Option<u64>, sqlx::Error> {
    sqlx::query_scalar(
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
    .bind(human_entity_id)
    .fetch_optional(pool)
    .await
}

/// Mark global recovery notified and email owner (idempotent on timestamp).
pub async fn finalize_global_recovery_notification(
    pool: &MySqlPool,
    config: &Config,
    human_entity_id: u64,
) {
    let healthy = failing_rating_threshold();
    let claimed = match sqlx::query(
        r#"
        UPDATE social_entity
        SET global_recovery_notified_at = UTC_TIMESTAMP(6)
        WHERE id = ?
          AND rating >= ?
          AND rating_failing_notified_at IS NOT NULL
          AND global_recovery_notified_at IS NULL
        "#,
    )
    .bind(human_entity_id)
    .bind(healthy)
    .execute(pool)
    .await
    {
        Ok(res) => res.rows_affected(),
        Err(err) => {
            tracing::warn!(
                error = %err,
                human_entity_id,
                "finalize_global_recovery_notification claim failed"
            );
            return;
        }
    };

    if claimed == 0 {
        return;
    }

    let row: Option<(String, Option<String>)> = match sqlx::query_as(
        r#"
        SELECT
            se.handle,
            COALESCE(
                NULLIF(TRIM(CONCAT_WS(' ', sh.given_name, sh.additional_name, sh.family_name)), ''),
                se.handle
            ) AS display_label
        FROM social_entity se
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = se.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` = ?
        LEFT JOIN social_human sh ON sh.social_entity_id = se.id
        WHERE se.id = ?
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .bind(human_entity_id)
    .fetch_optional(pool)
    .await
    {
        Ok(row) => row,
        Err(err) => {
            tracing::warn!(
                error = %err,
                human_entity_id,
                "finalize_global_recovery_notification profile lookup failed"
            );
            return;
        }
    };

    let Some((handle, display_label)) = row else {
        return;
    };

    let user_id = match lookup_human_owner_user_id(pool, human_entity_id).await {
        Ok(uid) => uid,
        Err(err) => {
            tracing::warn!(
                error = %err,
                human_entity_id,
                "finalize_global_recovery_notification owner lookup failed"
            );
            None
        }
    };

    if let Some(user_id) = user_id {
        crate::email::notify_human_global_recovery(
            pool,
            config,
            user_id,
            &handle,
            display_label.as_deref(),
        )
        .await;
    }
}
