//! Batch detection for global recovery godmother outreach.

use sqlx::MySqlPool;

use crate::config::Config;
use crate::ref_keys;
use crate::social::judicial::failing_rating_threshold;

use super::{enqueue_moment, GodmotherMoment};

#[derive(Debug, Clone, sqlx::FromRow)]
struct RecoveryCandidateRow {
    entity_id: u64,
}

/// Detect humans eligible for global recovery godmother outreach (email sent after post).
pub async fn flush_recovery_candidates(pool: &MySqlPool, config: &Config, limit: u64) {
    let lim = limit.clamp(1, 64) as i64;
    if let Err(err) = flush_recovery_candidates_inner(pool, config, lim).await {
        tracing::warn!(error = %err, "flush_recovery_candidates failed");
    }
}

async fn flush_recovery_candidates_inner(
    pool: &MySqlPool,
    config: &Config,
    limit: i64,
) -> Result<(), sqlx::Error> {
    let healthy = failing_rating_threshold();
    let rows: Vec<RecoveryCandidateRow> = sqlx::query_as(
        r#"
        SELECT se.id AS entity_id
        FROM social_entity se
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = se.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` = ?
        WHERE se.rating >= ?
          AND se.rating_failing_notified_at IS NOT NULL
          AND se.global_recovery_notified_at IS NULL
        ORDER BY se.id ASC
        LIMIT ?
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .bind(healthy)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    for row in rows {
        if super::resolve::should_skip_human(pool, row.entity_id).await? {
            continue;
        }

        let user_id = super::notify::lookup_human_owner_user_id(pool, row.entity_id).await?;

        if let Err(err) = enqueue_moment(
            pool,
            user_id,
            row.entity_id,
            GodmotherMoment::GlobalRecovery,
            Some("global_recovery_flush"),
        )
        .await
        {
            tracing::warn!(
                human_entity_id = row.entity_id,
                error = %err,
                "godmother global recovery job enqueue failed"
            );
        }
    }

    Ok(())
}
