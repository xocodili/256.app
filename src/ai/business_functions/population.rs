//! Live population counts for council presence budgets (√√N rule).

use sqlx::MySqlPool;

use crate::ref_keys;

/// Active verified humans with a recent sign-in (28-day window aligned with inactivity phases).
pub async fn count_active_verified_humans(pool: &MySqlPool) -> Result<u64, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT se.id)
        FROM social_entity se
        INNER JOIN ref_key_values t
            ON t.id = se.type_id
           AND t.`group` = ?
           AND t.`key` = ?
           AND t.is_active = 1
        INNER JOIN ref_key_values es
            ON es.id = se.entity_status_id
           AND es.`group` = ?
           AND es.`key` = ?
           AND es.is_active = 1
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        INNER JOIN user_social_entity_role ur
            ON ur.social_entity_id = se.id
           AND ur.is_primary = 1
           AND (ur.ends_at IS NULL OR ur.ends_at >= UTC_DATE())
        INNER JOIN users u ON u.id = ur.user_id
        WHERE se.rating > 0
          AND sh.died_at IS NULL
          AND u.email_verified_at IS NOT NULL
          AND COALESCE(u.last_login_at, u.created_at) >= UTC_TIMESTAMP(6) - INTERVAL 28 DAY
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .fetch_one(pool)
    .await?;
    Ok(count.max(0) as u64)
}
