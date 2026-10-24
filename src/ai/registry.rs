//! Runtime AI actor/profile resolution for app flows.

use sqlx::FromRow;
use sqlx::MySqlPool;

#[derive(Debug, Clone)]
pub struct ResolvedAiActor {
    pub slug: String,
    pub ai_actor_id: u64,
    pub policy_bundle_key: Option<String>,
}

#[derive(Debug, FromRow)]
struct ActorRow {
    ai_actor_id: u64,
    slug: String,
    policy_key: Option<String>,
}

/// Resolve active AI actor from DB assignments.
/// Prefers `preferred_slug`, then falls back to any active assignment.
pub async fn resolve_active_actor(
    pool: &MySqlPool,
    preferred_slug: &str,
) -> Result<Option<ResolvedAiActor>, sqlx::Error> {
    let preferred = preferred_slug.trim().to_ascii_lowercase();

    let preferred_row = sqlx::query_as::<_, ActorRow>(
        r#"
        SELECT
            aa.id AS ai_actor_id,
            aa.slug AS slug,
            pb.`key` AS policy_key
        FROM ai_actor aa
        INNER JOIN ai_profile_assignment apa
            ON apa.ai_actor_id = aa.id
           AND apa.is_active = 1
           AND (apa.ends_at IS NULL OR apa.ends_at >= UTC_DATE())
        LEFT JOIN ai_policy_bundle pb ON pb.id = apa.policy_bundle_id
        WHERE aa.slug = ?
          AND aa.is_active = 1
        ORDER BY apa.id DESC
        LIMIT 1
        "#,
    )
    .bind(&preferred)
    .fetch_optional(pool)
    .await?;

    let row = match preferred_row {
        Some(r) => Some(r),
        None => {
            sqlx::query_as::<_, ActorRow>(
                r#"
                SELECT
                    aa.id AS ai_actor_id,
                    aa.slug AS slug,
                    pb.`key` AS policy_key
                FROM ai_actor aa
                INNER JOIN ai_profile_assignment apa
                    ON apa.ai_actor_id = aa.id
                   AND apa.is_active = 1
                   AND (apa.ends_at IS NULL OR apa.ends_at >= UTC_DATE())
                LEFT JOIN ai_policy_bundle pb ON pb.id = apa.policy_bundle_id
                WHERE aa.is_active = 1
                ORDER BY aa.slug ASC, apa.id DESC
                LIMIT 1
                "#,
            )
            .fetch_optional(pool)
            .await?
        }
    };

    Ok(row.map(|r| ResolvedAiActor {
        slug: r.slug,
        ai_actor_id: r.ai_actor_id,
        policy_bundle_key: r.policy_key,
    }))
}
