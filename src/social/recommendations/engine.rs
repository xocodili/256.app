//! Orchestrates recommendation sources into sectioned Search/Add grids.

use std::collections::HashSet;

use sqlx::MySqlPool;

use crate::social::human_onboarding;
use crate::social::panoply;

use super::sources::{direct_connections, users_like_you};
use super::types::{
    EntityRecommendationsResponse, RecommendationSection,
    RecommendationTypeBucket, PER_TYPE_LIMIT, RECOMMENDATION_TYPE_KEYS, SECTION_DIRECT,
    SECTION_INFERRED,
};

pub async fn recommend_for_user(
    pool: &MySqlPool,
    user_id: u64,
    per_type: usize,
) -> Result<EntityRecommendationsResponse, sqlx::Error> {
    let per_type = per_type.clamp(1, PER_TYPE_LIMIT);
    let Some(viewer_human_entity_id) = human_onboarding::primary_human_entity_id(pool, user_id).await?
    else {
        return Ok(EntityRecommendationsResponse::empty());
    };

    let conceal_panoply = panoply::conceal_panoply_from_viewer(pool, user_id).await?;

    let direct_section =
        build_section(pool, viewer_human_entity_id, conceal_panoply, per_type, SECTION_DIRECT)
            .await?;

    let mut exclude: HashSet<u64> = HashSet::new();
    for bucket in &direct_section.types {
        for item in &bucket.items {
            exclude.insert(item.entity_id);
        }
    }

    let inferred_section = build_section_with_exclusions(
        pool,
        viewer_human_entity_id,
        conceal_panoply,
        per_type,
        SECTION_INFERRED,
        &exclude,
    )
    .await?;

    let mut sections = Vec::new();
    if section_has_items(&direct_section) {
        sections.push(direct_section);
    }
    if section_has_items(&inferred_section) {
        sections.push(inferred_section);
    }

    Ok(EntityRecommendationsResponse { sections })
}

fn section_has_items(section: &RecommendationSection) -> bool {
    section.types.iter().any(|bucket| !bucket.items.is_empty())
}

async fn build_section(
    pool: &MySqlPool,
    viewer_human_entity_id: u64,
    conceal_panoply: bool,
    per_type: usize,
    section_key: &str,
) -> Result<RecommendationSection, sqlx::Error> {
    build_section_with_exclusions(
        pool,
        viewer_human_entity_id,
        conceal_panoply,
        per_type,
        section_key,
        &HashSet::new(),
    )
    .await
}

async fn build_section_with_exclusions(
    pool: &MySqlPool,
    viewer_human_entity_id: u64,
    conceal_panoply: bool,
    per_type: usize,
    section_key: &str,
    exclude_entity_ids: &HashSet<u64>,
) -> Result<RecommendationSection, sqlx::Error> {
    let mut types = Vec::with_capacity(RECOMMENDATION_TYPE_KEYS.len());
    for type_key in RECOMMENDATION_TYPE_KEYS {
        let items = match section_key {
            SECTION_DIRECT => {
                direct_connections::recommend(
                    pool,
                    viewer_human_entity_id,
                    type_key,
                    conceal_panoply,
                    exclude_entity_ids,
                    per_type,
                )
                .await?
            }
            SECTION_INFERRED => {
                users_like_you::recommend(
                    pool,
                    viewer_human_entity_id,
                    type_key,
                    conceal_panoply,
                    exclude_entity_ids,
                    per_type,
                )
                .await?
            }
            _ => Vec::new(),
        };
        types.push(RecommendationTypeBucket {
            type_key: (*type_key).to_string(),
            items,
        });
    }

    Ok(RecommendationSection {
        key: section_key.to_string(),
        types,
    })
}
