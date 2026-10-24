//! Tier 2 — org entities reachable through community-expanded peer cohorts.

use std::collections::HashSet;

use sqlx::MySqlPool;

use super::common::{recommend_for_bridge_pool, BridgePool};
use super::super::types::{EntityRecommendation, SECTION_INFERRED};

pub async fn recommend(
    pool: &MySqlPool,
    viewer_human_entity_id: u64,
    type_key: &str,
    conceal_panoply: bool,
    exclude_entity_ids: &HashSet<u64>,
    per_type: usize,
) -> Result<Vec<EntityRecommendation>, sqlx::Error> {
    recommend_for_bridge_pool(
        pool,
        viewer_human_entity_id,
        BridgePool::ExpansionCohort,
        type_key,
        SECTION_INFERRED,
        conceal_panoply,
        exclude_entity_ids,
        per_type,
    )
    .await
}
