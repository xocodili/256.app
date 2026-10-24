//! Resolve active godmother edge and human context for outreach.

use sqlx::MySqlPool;

use crate::ref_keys;
use crate::social::community_graph::is_human_death_immune;

#[derive(Debug, Clone)]
pub struct ResolvedGodmother {
    pub human_entity_id: u64,
    pub godmother_entity_id: u64,
    pub human_handle: String,
}

pub async fn resolve_active_godmother(
    pool: &MySqlPool,
    human_entity_id: u64,
) -> Result<Option<ResolvedGodmother>, sqlx::Error> {
    let row: Option<(u64, String)> = sqlx::query_as(
        r#"
        SELECT ser.to_social_entity_id, se.handle
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk
            ON rk.id = ser.type_id
           AND rk.`group` = ?
           AND rk.`key` = ?
           AND rk.is_active = 1
        INNER JOIN social_entity se ON se.id = ser.from_social_entity_id
        WHERE ser.from_social_entity_id = ?
          AND ser.ended_at IS NULL
        ORDER BY ser.id ASC
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_GODMOTHER)
    .bind(human_entity_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|(godmother_entity_id, human_handle)| ResolvedGodmother {
        human_entity_id,
        godmother_entity_id,
        human_handle,
    }))
}

pub async fn should_skip_human(
    pool: &MySqlPool,
    human_entity_id: u64,
) -> Result<bool, sqlx::Error> {
    Ok(is_human_death_immune(pool, human_entity_id)
        .await
        .unwrap_or(true))
}
