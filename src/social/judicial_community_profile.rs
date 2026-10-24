//! Community profile amendment jury: pool tiers, verdict settlement, and applying upheld changes.
//!
//! Human jury only (same pattern as constitutional amendments). Amends `social_community`
//! `description`, `intention`, and `join_policy_id` when upheld. Location is not amendable.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{MySql, MySqlPool, Transaction};
use sqlx::types::Json;

use crate::ref_keys;
use crate::social::judicial::VerdictOutcome;

pub fn juror_min_rating_primary() -> Decimal {
    Decimal::new(250, 2)
}

pub fn juror_min_rating_fallback() -> Decimal {
    Decimal::new(250, 2)
}

/// Frozen or proposed profile fields (capped in handlers to post-body limits).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileFieldSnapshot {
    pub description_plain: String,
    pub description_html_sanitized: String,
    pub intention_plain: String,
    pub intention_html_sanitized: String,
    pub join_policy_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CommunityProfileAmendmentRow {
    pub change_description: bool,
    pub change_intention: bool,
    pub change_join_policy: bool,
    pub current: ProfileFieldSnapshot,
    pub proposed: ProfileFieldSnapshot,
}

pub async fn fetch_profile_amendment_for_case(
    pool: &MySqlPool,
    case_id: u64,
) -> Result<Option<CommunityProfileAmendmentRow>, sqlx::Error> {
    let row: Option<(
        bool,
        bool,
        bool,
        Json<ProfileFieldSnapshot>,
        Json<ProfileFieldSnapshot>,
    )> = sqlx::query_as(
        r#"
        SELECT
            change_description,
            change_intention,
            change_join_policy,
            current_snapshot,
            proposed_snapshot
        FROM judicial_case_community_profile_amendment
        WHERE judicial_case_id = ?
        LIMIT 1
        "#,
    )
    .bind(case_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(change_description, change_intention, change_join_policy, current, proposed)| {
            CommunityProfileAmendmentRow {
                change_description,
                change_intention,
                change_join_policy,
                current: current.0,
                proposed: proposed.0,
            }
        },
    ))
}

pub async fn apply_profile_amendment_on_upheld_tx(
    tx: &mut Transaction<'_, MySql>,
    case_id: u64,
    community_id: u64,
    _system_actor: Option<u64>,
) -> Result<(), sqlx::Error> {
    let Some(row) = sqlx::query_as::<_, (bool, bool, bool, Json<ProfileFieldSnapshot>)>(
        r#"
        SELECT change_description, change_intention, change_join_policy, proposed_snapshot
        FROM judicial_case_community_profile_amendment
        WHERE judicial_case_id = ?
        LIMIT 1
        "#,
    )
    .bind(case_id)
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Ok(());
    };

    let (change_description, change_intention, change_join_policy, proposed) = row;
    let proposed = proposed.0;

    if change_description {
        sqlx::query(
            r#"
            UPDATE social_community
            SET description = ?
            WHERE social_entity_id = ?
            "#,
        )
        .bind(proposed.description_plain.trim())
        .bind(community_id)
        .execute(&mut **tx)
        .await?;
    }
    if change_intention {
        sqlx::query(
            r#"
            UPDATE social_community
            SET intention = ?
            WHERE social_entity_id = ?
            "#,
        )
        .bind(proposed.intention_plain.trim())
        .bind(community_id)
        .execute(&mut **tx)
        .await?;
    }
    if change_join_policy {
        let Some(key) = proposed.join_policy_key.as_deref() else {
            return Ok(());
        };
        let Some(join_policy_id) = crate::ref_key_values::lookup_active_id_conn(
            &mut **tx,
            ref_keys::GROUP_SOCIAL_JOIN_POLICY,
            key,
        )
        .await?
        else {
            return Ok(());
        };
        sqlx::query(
            r#"
            UPDATE social_community
            SET join_policy_id = ?
            WHERE social_entity_id = ?
            "#,
        )
        .bind(join_policy_id)
        .bind(community_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub async fn apply_profile_amendment_verdict_rating_settlement_tx(
    tx: &mut Transaction<'_, MySql>,
    case_id: u64,
    verdict: VerdictOutcome,
    hung_band: bool,
) -> Result<(), sqlx::Error> {
    crate::social::judicial_constitutional::apply_constitutional_verdict_rating_settlement_tx(
        tx, case_id, verdict, hung_band,
    )
    .await
}

pub async fn apply_profile_amendment_no_jury_penalties_tx(
    tx: &mut Transaction<'_, MySql>,
    case_id: u64,
) -> Result<(), sqlx::Error> {
    crate::social::judicial_constitutional::apply_constitutional_no_jury_penalties_tx(tx, case_id)
        .await
}

