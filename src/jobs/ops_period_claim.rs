//! Idempotent once-per-period claims for recurring ops jobs.
//!
//! Queue dedup is `(job_type_id, payload_hash)` on **live** rows only. Daily coordinators
//! (`council_post_tick`, `council_invite_tick`, `panoply_daily_status`) enqueue every tick;
//! handlers claim their period here before fan-out / send.

use sqlx::MySqlPool;

/// Returns `true` when this `(digest_key, period_key)` pair was already claimed.
pub async fn ops_period_already_claimed(
    pool: &MySqlPool,
    digest_key: &str,
    period_key: &str,
) -> Result<bool, sqlx::Error> {
    let claimed: Option<i32> = sqlx::query_scalar(
        r#"
        SELECT 1
        FROM ops_daily_digest_sent
        WHERE digest_key = ? AND period_key = ?
        LIMIT 1
        "#,
    )
    .bind(digest_key.trim())
    .bind(period_key.trim())
    .fetch_optional(pool)
    .await?;
    Ok(claimed.is_some())
}

/// Returns `true` when this `(digest_key, period_key)` pair is newly claimed.
pub async fn claim_ops_period_once(
    pool: &MySqlPool,
    digest_key: &str,
    period_key: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        INSERT INTO ops_daily_digest_sent (digest_key, period_key)
        VALUES (?, ?)
        "#,
    )
    .bind(digest_key.trim())
    .bind(period_key.trim())
    .execute(pool)
    .await;
    match result {
        Ok(_) => Ok(true),
        Err(e) if crate::social::db::is_unique_violation(&e) => Ok(false),
        Err(e) => Err(e),
    }
}
