//! Constitutional-amendment jury: pool tiers, verdict settlement, and applying passed amendments.
//!
//! **Human jury only.** The AI council does not review or vote on constitutional changes; council
//! members receive seed constitutional clauses for their communities and arbitrate post disputes
//! against those rules via [`crate::social::judicial_council`].

use rust_decimal::Decimal;
use sqlx::{MySql, MySqlPool, Transaction};

use crate::ref_keys;
use crate::social::judicial::{
    bump_community_member_rating_tx, civic_rating_delta_times, community_context_for_case_tx,
    fetch_open_case_row_tx, is_community_social_entity_tx, VerdictOutcome,
    RATING_MULT_JUROR_PARTICIPATION, RATING_MULT_ORIGINATOR_CLEAR,
    RATING_MULT_ORIGINATOR_HUNG_PROPOSAL,
};

/// Proposer edge delta for constitutional/profile amendment verdicts.
#[must_use]
pub fn constitutional_originator_delta(verdict: VerdictOutcome, hung_band: bool) -> Decimal {
    if hung_band {
        return civic_rating_delta_times(-RATING_MULT_ORIGINATOR_HUNG_PROPOSAL);
    }
    match verdict {
        VerdictOutcome::Upheld => civic_rating_delta_times(RATING_MULT_ORIGINATOR_CLEAR),
        VerdictOutcome::Overturned | VerdictOutcome::Hung => {
            civic_rating_delta_times(-RATING_MULT_ORIGINATOR_CLEAR)
        }
    }
}

pub fn juror_min_rating_primary() -> Decimal {
    Decimal::new(250, 2)
}

pub fn juror_min_rating_fallback() -> Decimal {
    Decimal::new(250, 2)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ConstitutionalAmendmentRow {
    pub amendment_kind: String,
    pub target_constitution_item_id: Option<u64>,
    pub proposed_body_plain: String,
    pub proposed_body_html_sanitized: String,
    pub target_point: Option<u16>,
    pub target_item: Option<String>,
}

pub async fn fetch_constitutional_amendment_for_case(
    pool: &MySqlPool,
    case_id: u64,
) -> Result<Option<ConstitutionalAmendmentRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT
            jca.amendment_kind,
            jca.target_constitution_item_id,
            jca.proposed_body_plain,
            jca.proposed_body_html_sanitized,
            cci.point AS target_point,
            cci.item AS target_item
        FROM judicial_case_constitutional_amendment jca
        LEFT JOIN community_constitution_item cci
            ON cci.id = jca.target_constitution_item_id
        WHERE jca.judicial_case_id = ?
        LIMIT 1
        "#,
    )
    .bind(case_id)
    .fetch_optional(pool)
    .await
}

/// Weighted 2/3; hung band uses [`crate::social::judicial::verdict_eval::evaluate_weighted_verdict`].
#[must_use]
pub fn verdict_from_normalized_weights(normalized_for: f64, normalized_against: f64) -> VerdictOutcome {
    use crate::ref_keys;
    use crate::social::judicial::verdict_eval::evaluate_weighted_verdict;
    evaluate_weighted_verdict(
        ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT,
        normalized_for,
        normalized_against,
    )
    .outcome
}

pub async fn apply_constitutional_amendment_on_upheld_tx(
    tx: &mut Transaction<'_, MySql>,
    case_id: u64,
    community_id: u64,
    system_actor: Option<u64>,
) -> Result<(), sqlx::Error> {
    let Some(row) = sqlx::query_as::<_, (String, Option<u64>, String)>(
        r#"
        SELECT amendment_kind, target_constitution_item_id, proposed_body_plain
        FROM judicial_case_constitutional_amendment
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

    let (kind, target_id, body) = row;
    let body = body.trim();
    if body.is_empty() && kind != "remove_clause" {
        return Ok(());
    }

    match kind.as_str() {
        "new_clause" => {
            let next_point_raw: i64 = sqlx::query_scalar(
                r#"
                SELECT COALESCE(MAX(point), 0) + 1
                FROM community_constitution_item
                WHERE community_id = ?
                "#,
            )
            .bind(community_id)
            .fetch_one(&mut **tx)
            .await?;
            let next_point: u16 = u16::try_from(next_point_raw.max(1)).map_err(|_| {
                sqlx::Error::Configuration(
                    "community_constitution_item.point overflow for u16".into(),
                )
            })?;
            sqlx::query(
                r#"
                INSERT INTO community_constitution_item (
                    community_id, point, item, status, created_by, modified_by
                )
                VALUES (?, ?, ?, 'active', ?, ?)
                "#,
            )
            .bind(community_id)
            .bind(next_point)
            .bind(body)
            .bind(system_actor)
            .bind(system_actor)
            .execute(&mut **tx)
            .await?;
        }
        "amend_clause" => {
            let Some(item_id) = target_id else {
                return Ok(());
            };
            sqlx::query(
                r#"
                UPDATE community_constitution_item
                SET item = ?, modified_by = ?
                WHERE id = ? AND community_id = ? AND status = 'active'
                "#,
            )
            .bind(body)
            .bind(system_actor)
            .bind(item_id)
            .bind(community_id)
            .execute(&mut **tx)
            .await?;
        }
        "remove_clause" => {
            let Some(item_id) = target_id else {
                return Ok(());
            };
            sqlx::query(
                r#"
                UPDATE community_constitution_item
                SET status = 'retired', modified_by = ?
                WHERE id = ? AND community_id = ? AND status = 'active'
                "#,
            )
            .bind(system_actor)
            .bind(item_id)
            .bind(community_id)
            .execute(&mut **tx)
            .await?;
        }
        _ => {}
    }
    Ok(())
}

pub async fn apply_constitutional_verdict_rating_settlement_tx(
    tx: &mut Transaction<'_, MySql>,
    case_id: u64,
    verdict: VerdictOutcome,
    hung_band: bool,
) -> Result<(), sqlx::Error> {
    let system_actor: Option<u64> = sqlx::query_scalar(
        r#"SELECT id FROM users WHERE email = ? LIMIT 1"#,
    )
    .bind(ref_keys::SYSTEM_USER_EMAIL)
    .fetch_optional(&mut **tx)
    .await?;

    let already: Option<chrono::NaiveDateTime> = sqlx::query_scalar(
        r#"SELECT jury_civic_rating_applied_at FROM judicial_case WHERE id = ? FOR UPDATE"#,
    )
    .bind(case_id)
    .fetch_optional(&mut **tx)
    .await?
    .flatten();
    if already.is_some() {
        return Ok(());
    }

    let Some(case_row) = fetch_open_case_row_tx(tx, case_id).await? else {
        return Ok(());
    };
    let Some(community_id) = community_context_for_case_tx(tx, &case_row).await? else {
        return Ok(());
    };
    if !is_community_social_entity_tx(tx, community_id).await? {
        return Ok(());
    }

    let triple = civic_rating_delta_times(RATING_MULT_JUROR_PARTICIPATION);

    let voted_jurors: Vec<u64> = sqlx::query_scalar(
        r#"
        SELECT jcji.juror_social_entity_id
        FROM judicial_case_juror_invite jcji
        INNER JOIN ref_key_values rk
            ON rk.id = jcji.invite_status_id
           AND rk.`group` = ?
           AND rk.`key` = ?
           AND rk.is_active = 1
        WHERE jcji.judicial_case_id = ?
          AND jcji.vote_direction_id IS NOT NULL
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_ACCEPTED)
    .bind(case_id)
    .fetch_all(&mut **tx)
    .await?;

    for juror_id in voted_jurors {
        bump_community_member_rating_tx(tx, juror_id, community_id, triple, system_actor).await?;
    }

    let originator_delta = constitutional_originator_delta(verdict, hung_band);
    bump_community_member_rating_tx(
        tx,
        case_row.originator_social_entity_id,
        community_id,
        originator_delta,
        system_actor,
    )
    .await?;

    sqlx::query(
        r#"UPDATE judicial_case SET jury_civic_rating_applied_at = UTC_TIMESTAMP(6), modified_by = ? WHERE id = ?"#,
    )
    .bind(system_actor)
    .bind(case_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Insufficient jury or timeout: penalize the community global rating only (−6×δ).
pub async fn apply_constitutional_no_jury_penalties_tx(
    tx: &mut Transaction<'_, MySql>,
    case_id: u64,
) -> Result<(), sqlx::Error> {
    let already: Option<chrono::NaiveDateTime> = sqlx::query_scalar(
        r#"SELECT jury_civic_rating_applied_at FROM judicial_case WHERE id = ? FOR UPDATE"#,
    )
    .bind(case_id)
    .fetch_optional(&mut **tx)
    .await?
    .flatten();
    if already.is_some() {
        return Ok(());
    }

    let Some(case_row) = fetch_open_case_row_tx(tx, case_id).await? else {
        return Ok(());
    };

    crate::social::judicial::apply_community_no_jury_penalty_tx(tx, case_id, &case_row)
        .await?;

    let system_actor: Option<u64> = sqlx::query_scalar(
        r#"SELECT id FROM users WHERE email = ? LIMIT 1"#,
    )
    .bind(ref_keys::SYSTEM_USER_EMAIL)
    .fetch_optional(&mut **tx)
    .await?;

    sqlx::query(
        r#"UPDATE judicial_case SET jury_civic_rating_applied_at = UTC_TIMESTAMP(6), modified_by = ? WHERE id = ?"#,
    )
    .bind(system_actor)
    .bind(case_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hung_proposal_penalty_is_three_x() {
        let delta = constitutional_originator_delta(VerdictOutcome::Overturned, true);
        assert_eq!(delta, civic_rating_delta_times(-RATING_MULT_ORIGINATOR_HUNG_PROPOSAL));
    }

    #[test]
    fn clear_constitutional_verdicts_use_six_x() {
        assert_eq!(
            constitutional_originator_delta(VerdictOutcome::Upheld, false),
            civic_rating_delta_times(RATING_MULT_ORIGINATOR_CLEAR)
        );
        assert_eq!(
            constitutional_originator_delta(VerdictOutcome::Overturned, false),
            civic_rating_delta_times(-RATING_MULT_ORIGINATOR_CLEAR)
        );
    }
}

