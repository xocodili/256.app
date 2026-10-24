//! Active council membership rows (AI actor → social entity → stewarded community).

use sqlx::MySqlPool;

use crate::ref_keys;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CouncilMember {
    pub ai_actor_id: u64,
    pub slug: String,
    pub entity_id: u64,
    pub display_name: String,
    pub community_entity_id: u64,
    pub community_handle: String,
}

pub async fn load_active_council_members(pool: &MySqlPool) -> Result<Vec<CouncilMember>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT
            aa.id AS ai_actor_id,
            aa.slug,
            ai_se.id AS entity_id,
            COALESCE(sd.label, ai_se.handle) AS display_name,
            cse.id AS community_entity_id,
            cse.handle AS community_handle
        FROM ai_council_membership acm
        INNER JOIN ai_actor aa ON aa.id = acm.ai_actor_id AND aa.is_active = 1
        INNER JOIN social_entity ai_se ON ai_se.id = aa.social_entity_id
        LEFT JOIN social_digital sd ON sd.social_entity_id = ai_se.id
        INNER JOIN social_entity_relationship modr
            ON modr.from_social_entity_id = ai_se.id
           AND modr.ended_at IS NULL
        INNER JOIN ref_key_values rt
            ON rt.id = modr.type_id
           AND rt.`group` = ?
           AND rt.`key` = ?
           AND rt.is_active = 1
        INNER JOIN social_entity cse ON cse.id = modr.to_social_entity_id
        WHERE acm.is_active = 1
          AND (acm.ends_at IS NULL OR acm.ends_at >= UTC_DATE())
        ORDER BY aa.slug ASC
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MODERATOR)
    .fetch_all(pool)
    .await
}

pub async fn load_council_member_by_entity_id(
    pool: &MySqlPool,
    entity_id: u64,
) -> Result<Option<CouncilMember>, sqlx::Error> {
    let slug: Option<String> = sqlx::query_scalar(
        r#"
        SELECT aa.slug
        FROM ai_actor aa
        WHERE aa.social_entity_id = ?
          AND aa.is_active = 1
        LIMIT 1
        "#,
    )
    .bind(entity_id)
    .fetch_optional(pool)
    .await?;
    match slug {
        Some(s) => load_council_member_by_slug(pool, &s).await,
        None => Ok(None),
    }
}

pub async fn load_council_member_by_slug(
    pool: &MySqlPool,
    slug: &str,
) -> Result<Option<CouncilMember>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT
            aa.id AS ai_actor_id,
            aa.slug,
            ai_se.id AS entity_id,
            COALESCE(sd.label, ai_se.handle) AS display_name,
            cse.id AS community_entity_id,
            cse.handle AS community_handle
        FROM ai_council_membership acm
        INNER JOIN ai_actor aa ON aa.id = acm.ai_actor_id AND aa.is_active = 1
        INNER JOIN social_entity ai_se ON ai_se.id = aa.social_entity_id
        LEFT JOIN social_digital sd ON sd.social_entity_id = ai_se.id
        INNER JOIN social_entity_relationship modr
            ON modr.from_social_entity_id = ai_se.id
           AND modr.ended_at IS NULL
        INNER JOIN ref_key_values rt
            ON rt.id = modr.type_id
           AND rt.`group` = ?
           AND rt.`key` = ?
           AND rt.is_active = 1
        INNER JOIN social_entity cse ON cse.id = modr.to_social_entity_id
        WHERE acm.is_active = 1
          AND (acm.ends_at IS NULL OR acm.ends_at >= UTC_DATE())
          AND aa.slug = ?
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MODERATOR)
    .bind(slug.trim())
    .fetch_optional(pool)
    .await
}
