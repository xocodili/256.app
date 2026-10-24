//! Scoped-read invalidation: bump `social_entity.scope_version` and enqueue affected entities
//! into `entity_scope_recalc_queue` so the [`scope_recalc_drain`] job re-materializes
//! `entity_read_scope` rows from `vw_entity_read_scope_live`.
//!
//! Any code path that inserts, ends, or transitions a `social_entity_relationship` row whose
//! visibility (`pending_verification` | `verified` | `flagged` | `ended_at IS NULL`) might
//! change must call into here. Failure to enqueue is **not** fatal because the periodic
//! `scope_rematerialize_all` job will eventually reconcile drift — but realtime callers should
//! always enqueue so feed/profile reads see fresh visibility within one conductor tick.
//!
//! [`scope_recalc_drain`]: crate::jobs::queue (handler: `run_scope_recalc_drain_job`)

use sqlx::{MySql, MySqlPool, Transaction};

const REASON_RELATIONSHIP_CHANGED: &str = "relationship_changed";

/// Pool-flavored: enqueue both endpoints of a relationship for recalc and bump their `scope_version`.
pub async fn mark_relationship_endpoints_dirty(
    pool: &MySqlPool,
    from_entity_id: u64,
    to_entity_id: u64,
) -> Result<(), sqlx::Error> {
    mark_entity_dirty(pool, from_entity_id, REASON_RELATIONSHIP_CHANGED).await?;
    if to_entity_id != from_entity_id {
        mark_entity_dirty(pool, to_entity_id, REASON_RELATIONSHIP_CHANGED).await?;
    }
    Ok(())
}

/// Tx-flavored variant of [`mark_relationship_endpoints_dirty`] — use when the caller already
/// owns a transaction (e.g. `entity::create`'s creator-member edge, godmother onboarding).
pub async fn mark_relationship_endpoints_dirty_in_tx(
    tx: &mut Transaction<'_, MySql>,
    from_entity_id: u64,
    to_entity_id: u64,
) -> Result<(), sqlx::Error> {
    mark_entity_dirty_in_tx(tx, from_entity_id, REASON_RELATIONSHIP_CHANGED).await?;
    if to_entity_id != from_entity_id {
        mark_entity_dirty_in_tx(tx, to_entity_id, REASON_RELATIONSHIP_CHANGED).await?;
    }
    Ok(())
}

/// Pool-flavored: enqueue a single subject entity for recalc and bump `scope_version`.
pub async fn mark_entity_dirty(
    pool: &MySqlPool,
    entity_id: u64,
    reason: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO entity_scope_recalc_queue (subject_entity_id, requested_at, reason)
        VALUES (?, UTC_TIMESTAMP(6), ?)
        ON DUPLICATE KEY UPDATE
            requested_at = VALUES(requested_at),
            reason = VALUES(reason)
        "#,
    )
    .bind(entity_id)
    .bind(reason)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"UPDATE social_entity SET scope_version = scope_version + 1 WHERE id = ?"#,
    )
    .bind(entity_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Tx-flavored variant of [`mark_entity_dirty`].
pub async fn mark_entity_dirty_in_tx(
    tx: &mut Transaction<'_, MySql>,
    entity_id: u64,
    reason: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO entity_scope_recalc_queue (subject_entity_id, requested_at, reason)
        VALUES (?, UTC_TIMESTAMP(6), ?)
        ON DUPLICATE KEY UPDATE
            requested_at = VALUES(requested_at),
            reason = VALUES(reason)
        "#,
    )
    .bind(entity_id)
    .bind(reason)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        r#"UPDATE social_entity SET scope_version = scope_version + 1 WHERE id = ?"#,
    )
    .bind(entity_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}
