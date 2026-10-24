//! Integrity checks for the refactored council/business object model.

use sqlx::MySqlPool;

#[derive(Debug, serde::Serialize)]
pub struct CouncilIntegrity {
    pub ai_actor_count: u64,
    pub council_membership_count: u64,
    pub domain_ownership_count: u64,
    pub profile_assignment_count: u64,
    pub judicial_scope_count: u64,
    pub ok: bool,
}

pub async fn council_integrity(pool: &MySqlPool) -> Result<CouncilIntegrity, sqlx::Error> {
    // MySQL COUNT(*) decodes as signed BIGINT; use i64 then clamp to u64.
    let ai_actor_count: u64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM ai_actor WHERE is_active = 1",
    )
    .fetch_one(pool)
    .await?
    .max(0) as u64;
    let council_membership_count: u64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM ai_council_membership WHERE is_active = 1",
    )
    .fetch_one(pool)
    .await?
    .max(0) as u64;
    let domain_ownership_count: u64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM ai_domain_ownership WHERE is_active = 1",
    )
    .fetch_one(pool)
    .await?
    .max(0) as u64;
    let profile_assignment_count: u64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM ai_profile_assignment WHERE is_active = 1",
    )
    .fetch_one(pool)
    .await?
    .max(0) as u64;
    let judicial_scope_count: u64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM ai_judicial_scope WHERE is_active = 1",
    )
    .fetch_one(pool)
    .await?
    .max(0) as u64;

    let ok = ai_actor_count >= 12
        && council_membership_count >= 12
        && domain_ownership_count >= 12
        && profile_assignment_count >= 12
        && judicial_scope_count >= 12;

    Ok(CouncilIntegrity {
        ai_actor_count,
        council_membership_count,
        domain_ownership_count,
        profile_assignment_count,
        judicial_scope_count,
        ok,
    })
}
