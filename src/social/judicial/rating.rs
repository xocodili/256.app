use chrono::{DateTime, NaiveDateTime, Utc};
use rust_decimal::Decimal;
use sqlx::MySql;

use crate::ref_keys;

use crate::social::noms;

use super::case_access::{community_context_for_case_tx, is_community_social_entity_tx};
use super::invites::count_judicial_abstentions_in_window_tx;
use super::types::{
    civic_rating_delta_times, jury_civic_rating_delta, OpenCaseRow, VerdictOutcome,
    RATING_MULT_JUROR_PARTICIPATION, RATING_MULT_ORIGINATOR_CLEAR, MAX_FREE_ABSTENTIONS_PER_YEAR,
};

/// Wall-clock seconds from case `opened_at` to resolution used for feed-community rating tiers.
pub(crate) const COMMUNITY_FEEDBACK_THREE_DAYS_SECS: i64 = 3 * 86_400;
pub(crate) const COMMUNITY_FEEDBACK_SEVEN_DAYS_SECS: i64 = 7 * 86_400;
pub(crate) const COMMUNITY_FEEDBACK_FOURTEEN_DAYS_SECS: i64 = 14 * 86_400;
/// Divisor to normalize the 0.00-5.00 rating scale to a 02 vote weight.
/// A rating of 2.50 yields weight 1.0 (one full vote).
pub const RATING_NORMALIZATION_DIVISOR: f64 = 2.5;

/// Community edge rating changes spill to the human's global rating at 1/6.
pub const COMMUNITY_MEMBER_TO_GLOBAL_RATING_SPILLOVER_DIVISOR: i64 = 6;

/// Normalize a juror rating (0.00-5.00) to a vote weight (0.0-2.0).
pub fn normalize_vote_weight(rating: &Decimal) -> f64 {
    use rust_decimal::prelude::ToPrimitive;
    rating.to_f64().unwrap_or(0.0) / RATING_NORMALIZATION_DIVISOR
}
#[derive(Debug, sqlx::FromRow)]
struct InviteSettlementRow {
    invite_id: u64,
    juror_social_entity_id: u64,
    juror_user_id: Option<u64>,
    status_key: String,
    vote_direction_key: Option<String>,
    rating_basis_key: String,
    rating_community_entity_id: Option<u64>,
}

#[must_use]
pub fn community_member_delta_to_global_spillover(delta: Decimal) -> Decimal {
    if delta.is_zero() {
        return Decimal::ZERO;
    }
    delta / Decimal::from(COMMUNITY_MEMBER_TO_GLOBAL_RATING_SPILLOVER_DIVISOR)
}

pub(crate) async fn bump_community_member_rating_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    juror_entity_id: u64,
    community_entity_id: u64,
    delta: Decimal,
    system_actor: Option<u64>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE social_entity_relationship ser
        INNER JOIN ref_key_values rk
            ON rk.id = ser.type_id
           AND rk.`group` = ?
           AND rk.`key` = ?
           AND rk.is_active = 1
        SET ser.rating = LEAST(
                CAST(5.00 AS DECIMAL(3, 2)),
                GREATEST(CAST(0.00 AS DECIMAL(3, 2)), ser.rating + ?)
            ),
            ser.modified_by = ?
        WHERE ser.from_social_entity_id = ?
          AND ser.to_social_entity_id = ?
          AND ser.ended_at IS NULL
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .bind(delta)
    .bind(system_actor)
    .bind(juror_entity_id)
    .bind(community_entity_id)
    .execute(&mut **tx)
    .await?;

    let global_delta = community_member_delta_to_global_spillover(delta);
    if !global_delta.is_zero() {
        bump_global_entity_rating_tx(tx, juror_entity_id, global_delta, system_actor).await?;
    }
    Ok(())
}

pub async fn bump_global_entity_rating_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    entity_id: u64,
    delta: Decimal,
    system_actor: Option<u64>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE social_entity
        SET rating = LEAST(
                CAST(5.00 AS DECIMAL(3, 2)),
                GREATEST(CAST(0.00 AS DECIMAL(3, 2)), rating + ?)
            ),
            modified_by = ?
        WHERE id = ?
        "#,
    )
    .bind(delta)
    .bind(system_actor)
    .bind(entity_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Apply a juror invite delta on community edge (when present) or global rating otherwise.
async fn apply_invite_rating_delta_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    juror_entity_id: u64,
    delta: Decimal,
    rating_basis_key: &str,
    rating_community_entity_id: Option<u64>,
    system_actor: Option<u64>,
) -> Result<(), sqlx::Error> {
    if rating_basis_key == ref_keys::KEY_JCJI_RB_COMMUNITY {
        if let Some(comm) = rating_community_entity_id {
            bump_community_member_rating_tx(tx, juror_entity_id, comm, delta, system_actor).await
        } else {
            bump_global_entity_rating_tx(tx, juror_entity_id, delta, system_actor).await
        }
    } else {
        bump_global_entity_rating_tx(tx, juror_entity_id, delta, system_actor).await
    }
}

async fn bump_moderation_originator_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    case_row: &OpenCaseRow,
    verdict: VerdictOutcome,
    system_actor: Option<u64>,
) -> Result<(), sqlx::Error> {
    let originator_delta = match verdict {
        VerdictOutcome::Upheld => civic_rating_delta_times(RATING_MULT_ORIGINATOR_CLEAR),
        VerdictOutcome::Overturned => civic_rating_delta_times(-RATING_MULT_ORIGINATOR_CLEAR),
        VerdictOutcome::Hung => return Ok(()),
    };
    if let Some(community_id) = community_context_for_case_tx(tx, case_row).await? {
        bump_community_member_rating_tx(
            tx,
            case_row.originator_social_entity_id,
            community_id,
            originator_delta,
            system_actor,
        )
        .await?;
    } else {
        bump_global_entity_rating_tx(
            tx,
            case_row.originator_social_entity_id,
            originator_delta,
            system_actor,
        )
        .await?;
    }
    Ok(())
}

/// Jurors who voted get +3×d; excess abstainers get -3×d on the invite rating surface.
/// Post-dispute / appeal originators get ±6×d on a clear verdict (not hung band).
pub async fn apply_jury_civic_rating_settlement_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    case_id: u64,
    case_row: &OpenCaseRow,
    verdict: VerdictOutcome,
    hung_band: bool,
) -> Result<(), sqlx::Error> {
    let system_actor: Option<u64> = sqlx::query_scalar(
        r#"SELECT id FROM users WHERE email = ? LIMIT 1"#,
    )
    .bind(ref_keys::SYSTEM_USER_EMAIL)
    .fetch_optional(&mut **tx)
    .await?;

    let already: Option<NaiveDateTime> = sqlx::query_scalar(
        r#"SELECT jury_civic_rating_applied_at FROM judicial_case WHERE id = ? FOR UPDATE"#,
    )
    .bind(case_id)
    .fetch_optional(&mut **tx)
    .await?
    .flatten();
    if already.is_some() {
        return Ok(());
    }

    let rows: Vec<InviteSettlementRow> = sqlx::query_as(
        r#"
        SELECT
            jcji.id AS invite_id,
            jcji.juror_social_entity_id,
            jcji.juror_user_id,
            rk_status.`key` AS status_key,
            rk_vote.`key` AS vote_direction_key,
            rk_basis.`key` AS rating_basis_key,
            jcji.rating_community_entity_id
        FROM judicial_case_juror_invite jcji
        INNER JOIN ref_key_values rk_status
            ON rk_status.id = jcji.invite_status_id
           AND rk_status.`group` = ?
           AND rk_status.is_active = 1
        LEFT JOIN ref_key_values rk_vote
            ON rk_vote.id = jcji.vote_direction_id
           AND rk_vote.`group` = ?
           AND rk_vote.is_active = 1
        INNER JOIN ref_key_values rk_basis
            ON rk_basis.id = jcji.rating_basis_id
           AND rk_basis.`group` = ?
           AND rk_basis.is_active = 1
        WHERE jcji.judicial_case_id = ?
          AND (
              (rk_status.`key` = ? AND jcji.vote_direction_id IS NOT NULL)
           OR rk_status.`key` = ?
          )
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_VOTE_DIRECTION)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_RATING_BASIS)
    .bind(case_id)
    .bind(ref_keys::KEY_JCJI_ACCEPTED)
    .bind(ref_keys::KEY_JCJI_ABSTAINED)
    .fetch_all(&mut **tx)
    .await?;

    let voter_delta = civic_rating_delta_times(RATING_MULT_JUROR_PARTICIPATION);
    let abstainer_delta = civic_rating_delta_times(-RATING_MULT_JUROR_PARTICIPATION);

    let severe_abstention_exempt = super::triggers::trigger_is_emergency_safety(&case_row.trigger_key)
        || super::moderation_tier::is_serious_moderation_tier_key(
            super::moderation_tier::case_moderation_tier_key_tx(tx, case_id)
                .await?
                .as_deref(),
        );

    for row in rows {
        if row.status_key == ref_keys::KEY_JCJI_ACCEPTED {
            apply_invite_rating_delta_tx(
                tx,
                row.juror_social_entity_id,
                voter_delta,
                &row.rating_basis_key,
                row.rating_community_entity_id,
                system_actor,
            )
            .await?;
            let _ = noms::try_credit_jury_duty_tx(
                tx,
                row.invite_id,
                row.juror_social_entity_id,
                case_id,
                row.juror_user_id,
            )
            .await?;
        } else if row.status_key == ref_keys::KEY_JCJI_ABSTAINED {
            let Some(uid) = row.juror_user_id else {
                continue;
            };
            if severe_abstention_exempt {
                continue;
            }
            let cnt = count_judicial_abstentions_in_window_tx(tx, uid).await?;
            if cnt <= MAX_FREE_ABSTENTIONS_PER_YEAR {
                continue;
            }
            apply_invite_rating_delta_tx(
                tx,
                row.juror_social_entity_id,
                abstainer_delta,
                &row.rating_basis_key,
                row.rating_community_entity_id,
                system_actor,
            )
            .await?;
        }
    }

    let is_moderation = super::triggers::trigger_is_post_dispute_like(&case_row.trigger_key)
        || super::triggers::trigger_is_emergency_safety(&case_row.trigger_key);
    if is_moderation && !hung_band {
        bump_moderation_originator_tx(tx, case_row, verdict, system_actor).await?;
    }

    sqlx::query(
        r#"UPDATE judicial_case SET jury_civic_rating_applied_at = UTC_TIMESTAMP(6), modified_by = ? WHERE id = ?"#,
    )
    .bind(system_actor)
    .bind(case_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// +3×d on the feed community when a jury successfully resolves (any outcome).
pub(crate) async fn apply_community_jury_resolved_bonus_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    case_id: u64,
    case_row: &OpenCaseRow,
) -> Result<(), sqlx::Error> {
    let already: Option<NaiveDateTime> = sqlx::query_scalar(
        r#"SELECT community_jury_resolved_bonus_applied_at FROM judicial_case WHERE id = ? FOR UPDATE"#,
    )
    .bind(case_id)
    .fetch_optional(&mut **tx)
    .await?
    .flatten();
    if already.is_some() {
        return Ok(());
    }

    let Some(community_id) = community_context_for_case_tx(tx, case_row).await? else {
        return Ok(());
    };
    if !is_community_social_entity_tx(tx, community_id).await? {
        return Ok(());
    }

    let system_actor: Option<u64> = sqlx::query_scalar(
        r#"SELECT id FROM users WHERE email = ? LIMIT 1"#,
    )
    .bind(ref_keys::SYSTEM_USER_EMAIL)
    .fetch_optional(&mut **tx)
    .await?;

    bump_global_entity_rating_tx(
        tx,
        community_id,
        civic_rating_delta_times(RATING_MULT_JUROR_PARTICIPATION),
        system_actor,
    )
    .await?;

    sqlx::query(
        r#"UPDATE judicial_case SET community_jury_resolved_bonus_applied_at = UTC_TIMESTAMP(6), modified_by = ? WHERE id = ?"#,
    )
    .bind(system_actor)
    .bind(case_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Elapsed seconds from `opened_at` to `closed_at` for feed-community reward tiers on verdict.
#[must_use]
pub(crate) fn community_verdict_community_rating_delta(elapsed_secs: i64) -> Decimal {
    let d = jury_civic_rating_delta();
    let elapsed_secs = elapsed_secs.max(0);
    if elapsed_secs <= COMMUNITY_FEEDBACK_THREE_DAYS_SECS {
        d * Decimal::from(3i64)
    } else if elapsed_secs <= COMMUNITY_FEEDBACK_SEVEN_DAYS_SECS {
        d
    } else if elapsed_secs <= COMMUNITY_FEEDBACK_FOURTEEN_DAYS_SECS {
        Decimal::ZERO
    } else {
        Decimal::ZERO
    }
}

pub(crate) async fn apply_community_feed_moderation_rating_feedback_on_resolved_case_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    case_id: u64,
) -> Result<(), sqlx::Error> {
    let row: Option<(
        Option<NaiveDateTime>,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
        String,
        String,
        u64,
        u64,
        u64,
        Option<u64>,
        Option<u64>,
        u8,
    )> = sqlx::query_as(
        r#"
        SELECT
            jc.community_rating_feedback_applied_at,
            jc.opened_at,
            jc.closed_at,
            rk_t.`key`,
            rk_s.`key`,
            jc.id,
            jc.originator_social_entity_id,
            jc.target_social_entity_id,
            jc.created_by,
            jc.subject_post_id,
            jc.jury_size
        FROM judicial_case jc
        INNER JOIN ref_key_values rk_t
            ON rk_t.id = jc.trigger_type_id
           AND rk_t.`group` = ?
           AND rk_t.is_active = 1
        INNER JOIN ref_key_values rk_s
            ON rk_s.id = jc.status_id
           AND rk_s.`group` = ?
           AND rk_s.is_active = 1
        WHERE jc.id = ?
        FOR UPDATE
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(case_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some((
        applied_at,
        opened_at,
        closed_at,
        trigger_key,
        status_key,
        id,
        originator_social_entity_id,
        target_social_entity_id,
        created_by,
        subject_post_id,
        jury_size,
    )) = row
    else {
        return Ok(());
    };

    if applied_at.is_some() {
        return Ok(());
    }
    if status_key != ref_keys::KEY_JUDICIAL_CASE_RESOLVED {
        return Ok(());
    }
    if !super::triggers::trigger_is_civic_dispute(&trigger_key)
        && trigger_key != ref_keys::KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW
    {
        return Ok(());
    }
    let Some(closed_at) = closed_at else {
        return Ok(());
    };

    let case_row = OpenCaseRow {
        id,
        originator_social_entity_id,
        target_social_entity_id,
        created_by,
        trigger_key,
        subject_post_id,
        jury_size,
        status_key,
    };

    let Some(community_id) = community_context_for_case_tx(tx, &case_row).await? else {
        return Ok(());
    };

    if !is_community_social_entity_tx(tx, community_id).await? {
        return Ok(());
    }

    let elapsed_secs = closed_at
        .signed_duration_since(opened_at)
        .num_seconds()
        .max(0);
    let system_actor: Option<u64> = sqlx::query_scalar(
        r#"SELECT id FROM users WHERE email = ? LIMIT 1"#,
    )
    .bind(ref_keys::SYSTEM_USER_EMAIL)
    .fetch_optional(&mut **tx)
    .await?;

    let delta = community_verdict_community_rating_delta(elapsed_secs);
    if !delta.is_zero() {
        bump_global_entity_rating_tx(tx, community_id, delta, system_actor).await?;
    }

    sqlx::query(
        r#"UPDATE judicial_case SET community_rating_feedback_applied_at = UTC_TIMESTAMP(6), modified_by = ? WHERE id = ?"#,
    )
    .bind(system_actor)
    .bind(case_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Community-only -6×d when no jury could be seated (self-governance failure).
pub async fn apply_community_no_jury_penalty_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    case_id: u64,
    case_row: &OpenCaseRow,
) -> Result<(), sqlx::Error> {
    let Some(community_id) = community_context_for_case_tx(tx, case_row).await? else {
        return Ok(());
    };
    if !is_community_social_entity_tx(tx, community_id).await? {
        return Ok(());
    }
    let system_actor: Option<u64> = sqlx::query_scalar(
        r#"SELECT id FROM users WHERE email = ? LIMIT 1"#,
    )
    .bind(ref_keys::SYSTEM_USER_EMAIL)
    .fetch_optional(&mut **tx)
    .await?;
    bump_global_entity_rating_tx(
        tx,
        community_id,
        civic_rating_delta_times(-RATING_MULT_ORIGINATOR_CLEAR),
        system_actor,
    )
    .await?;
    Ok(())
}
