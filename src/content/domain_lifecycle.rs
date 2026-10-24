//! Block `content_domain` rows whose rating has fallen to zero or below.

use sqlx::MySqlPool;

use crate::ref_key_values;
use crate::ref_keys;

#[derive(Debug, Clone, Default)]
pub struct DomainLifecycleSummary {
    pub domains_blocked: u64,
}

pub async fn run_domain_lifecycle_batch(
    pool: &MySqlPool,
    limit: u32,
) -> Result<DomainLifecycleSummary, sqlx::Error> {
    let blocked_id = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_STATUS,
        ref_keys::KEY_BLOCKED,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("missing status.blocked ref key".into())
    })?;

    let lim = limit.clamp(1, 256) as i64;
    let rows: Vec<u64> = sqlx::query_scalar(
        r#"
        SELECT d.id
        FROM content_domain d
        INNER JOIN ref_key_values rk ON rk.id = d.status_id AND rk.`key` <> ?
        WHERE d.rating <= 0.00
          AND d.safety_state = 'safe'
        ORDER BY d.id ASC
        LIMIT ?
        "#,
    )
    .bind(ref_keys::KEY_BLOCKED)
    .bind(lim)
    .fetch_all(pool)
    .await?;

    let mut summary = DomainLifecycleSummary::default();
    for domain_id in rows {
        let updated = sqlx::query(
            r#"
            UPDATE content_domain
            SET status_id = ?, modified_by = (
                SELECT id FROM users WHERE email = ? LIMIT 1
            )
            WHERE id = ? AND rating <= 0.00
            "#,
        )
        .bind(blocked_id)
        .bind(ref_keys::SYSTEM_USER_EMAIL)
        .bind(domain_id)
        .execute(pool)
        .await?;
        if updated.rows_affected() > 0 {
            summary.domains_blocked += 1;
        }
    }
    Ok(summary)
}
