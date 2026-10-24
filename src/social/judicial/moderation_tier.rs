//! Post-dispute moderation severity derived from cited constitution items.

use sqlx::{MySql, MySqlPool, Transaction};

use crate::ref_keys;
use crate::social::safety::points_imply_serious_tier;

/// Regular post-dispute `no_jury` close window (days).
pub const REGULAR_NO_JURY_DEADLINE_DAYS: i64 = 21;
/// Accelerated window when any cited clause carries the serious tier.
pub const SERIOUS_NO_JURY_DEADLINE_DAYS: i64 = 7;

#[must_use]
pub fn is_serious_moderation_tier_key(key: Option<&str>) -> bool {
    key == Some(ref_keys::KEY_JUDICIAL_MODERATION_TIER_SERIOUS)
}

/// Highest severity among cited clause tiers for a case (`serious` wins over `regular`).
/// Harm-class routing (Class C §4.2/§4.4) also implies serious tier for `no_jury` clocks.
pub(crate) async fn case_moderation_tier_key_tx(
    tx: &mut Transaction<'_, MySql>,
    case_id: u64,
) -> Result<Option<String>, sqlx::Error> {
    let serious_hit: Option<i8> = sqlx::query_scalar(
        r#"
        SELECT 1
        FROM judicial_case_constitution_item jcci
        INNER JOIN community_constitution_item cci ON cci.id = jcci.constitution_item_id
        INNER JOIN ref_key_values rk ON rk.id = cci.moderation_tier_id
        WHERE jcci.judicial_case_id = ?
          AND rk.`group` = ?
          AND rk.`key` = ?
          AND rk.is_active = 1
        LIMIT 1
        "#,
    )
    .bind(case_id)
    .bind(ref_keys::GROUP_JUDICIAL_MODERATION_TIER)
    .bind(ref_keys::KEY_JUDICIAL_MODERATION_TIER_SERIOUS)
    .fetch_optional(&mut **tx)
    .await?;

    if serious_hit.is_some() {
        return Ok(Some(
            ref_keys::KEY_JUDICIAL_MODERATION_TIER_SERIOUS.to_string(),
        ));
    }

    let cited_points: Vec<u16> = sqlx::query_scalar(
        r#"
        SELECT cci.point
        FROM judicial_case_constitution_item jcci
        INNER JOIN community_constitution_item cci ON cci.id = jcci.constitution_item_id
        WHERE jcci.judicial_case_id = ?
        "#,
    )
    .bind(case_id)
    .fetch_all(&mut **tx)
    .await?;

    if points_imply_serious_tier(&cited_points) {
        return Ok(Some(
            ref_keys::KEY_JUDICIAL_MODERATION_TIER_SERIOUS.to_string(),
        ));
    }

    let regular_hit: Option<i8> = sqlx::query_scalar(
        r#"
        SELECT 1
        FROM judicial_case_constitution_item jcci
        INNER JOIN community_constitution_item cci ON cci.id = jcci.constitution_item_id
        INNER JOIN ref_key_values rk ON rk.id = cci.moderation_tier_id
        WHERE jcci.judicial_case_id = ?
          AND rk.`group` = ?
          AND rk.`key` = ?
          AND rk.is_active = 1
        LIMIT 1
        "#,
    )
    .bind(case_id)
    .bind(ref_keys::GROUP_JUDICIAL_MODERATION_TIER)
    .bind(ref_keys::KEY_JUDICIAL_MODERATION_TIER_REGULAR)
    .fetch_optional(&mut **tx)
    .await?;

    if regular_hit.is_some() {
        return Ok(Some(
            ref_keys::KEY_JUDICIAL_MODERATION_TIER_REGULAR.to_string(),
        ));
    }

    Ok(None)
}

/// Emergency safety or serious post-moderation tier — abstention is quota-free for jurors.
pub async fn case_is_severe_for_juror_abstention(
    pool: &MySqlPool,
    case_id: u64,
    trigger_key: &str,
) -> Result<bool, sqlx::Error> {
    if super::triggers::trigger_is_emergency_safety(trigger_key) {
        return Ok(true);
    }
    let mut tx = pool.begin().await?;
    let tier = case_moderation_tier_key_tx(&mut tx, case_id).await?;
    tx.rollback().await.ok();
    Ok(is_serious_moderation_tier_key(tier.as_deref()))
}
