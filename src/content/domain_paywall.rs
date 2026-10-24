//! Domain paywall profile for council domain selection (never touches rating).
//!
//! New domains default to `free`. Learning only moves toward stricter tiers.

use sqlx::MySqlPool;

pub const PAYWALL_FREE: &str = "free";
pub const PAYWALL_PARTIAL: &str = "partial";
pub const PAYWALL_WHOLE: &str = "whole";

/// Paywall on this link's host (`free` → `partial`; `partial`/`whole` unchanged).
pub async fn record_paywall_hit(
    pool: &MySqlPool,
    link_id: u64,
) -> Result<Option<String>, sqlx::Error> {
    let domain: Option<String> = sqlx::query_scalar(
        r#"
        SELECT cd.domain
        FROM content_link cl
        INNER JOIN content_domain cd ON cd.id = cl.domain_id
        WHERE cl.id = ?
          AND cd.paywall = ?
        LIMIT 1
        "#,
    )
    .bind(link_id)
    .bind(PAYWALL_FREE)
    .fetch_optional(pool)
    .await?;

    let Some(domain) = domain else {
        return Ok(None);
    };

    sqlx::query(
        r#"
        UPDATE content_domain cd
        INNER JOIN content_link cl ON cl.domain_id = cd.id
        SET cd.paywall = ?,
            cd.updated_at = CURRENT_TIMESTAMP
        WHERE cl.id = ?
          AND cd.paywall = ?
        "#,
    )
    .bind(PAYWALL_PARTIAL)
    .bind(link_id)
    .bind(PAYWALL_FREE)
    .execute(pool)
    .await?;

    Ok(Some(domain))
}

#[must_use]
pub fn paywall_sort_key(tier: &str) -> u8 {
    match tier {
        PAYWALL_FREE => 0,
        PAYWALL_PARTIAL => 1,
        PAYWALL_WHOLE => 2,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_prefers_free_then_partial() {
        assert!(paywall_sort_key(PAYWALL_FREE) < paywall_sort_key(PAYWALL_PARTIAL));
        assert!(paywall_sort_key(PAYWALL_PARTIAL) < paywall_sort_key(PAYWALL_WHOLE));
    }
}
