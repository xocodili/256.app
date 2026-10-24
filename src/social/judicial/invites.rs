use sqlx::{MySql, MySqlPool};

use crate::ref_keys;
use crate::social::audit;

use super::types::{
    AbstainOutcome, CastVoteOutcome, DeclineOutcome, PendingJurorInvite, MAX_FREE_ABSTENTIONS_PER_YEAR,
    ABSTENTION_WINDOW_DAYS,
};
pub async fn count_recent_abstentions_for_user(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<u32, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM judicial_case_juror_invite jcji
        INNER JOIN ref_key_values rk
            ON rk.id = jcji.invite_status_id
           AND rk.`group` = ?
           AND rk.`key` = ?
           AND rk.is_active = 1
        WHERE jcji.juror_user_id = ?
          AND COALESCE(jcji.responded_at, jcji.selected_at)
              >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL ? DAY)
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_ABSTAINED)
    .bind(user_id)
    .bind(ABSTENTION_WINDOW_DAYS)
    .fetch_one(pool)
    .await?;
    Ok(count.max(0) as u32)
}
async fn lock_case_and_invite_row_for_update(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    invite_id: u64,
) -> Result<Option<(u64, Option<u64>, String)>, sqlx::Error> {
    let Some(case_id): Option<u64> = sqlx::query_scalar(
        r#"SELECT judicial_case_id FROM judicial_case_juror_invite WHERE id = ?"#,
    )
    .bind(invite_id)
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Ok(None);
    };

    let case_hit: Option<u64> = sqlx::query_scalar(
        r#"SELECT id FROM judicial_case WHERE id = ? FOR UPDATE"#,
    )
    .bind(case_id)
    .fetch_optional(&mut **tx)
    .await?;

    if case_hit.is_none() {
        return Ok(None);
    }

    sqlx::query_as(
        r#"
        SELECT jcji.judicial_case_id, jcji.juror_user_id, rk.`key`
        FROM judicial_case_juror_invite jcji
        INNER JOIN ref_key_values rk
            ON rk.id = jcji.invite_status_id
           AND rk.`group` = ?
           AND rk.is_active = 1
        WHERE jcji.id = ?
        FOR UPDATE
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(invite_id)
    .fetch_optional(&mut **tx)
    .await
}

/// Record a juror's abstention from a single jury invitation.
///
/// The invite is identified by id rather than `(case_id, user_id)` so the
/// caller can route directly from a notification link. The user check is
/// done inside the transaction to keep authorization atomic with the state
/// flip.
///
/// On success the seat is freed; the next `judicial_jury_nomination` tick
/// will refill it. See [`AbstainOutcome`] for the result shape.
///
/// **Transactions:** abstention update is committed in one transaction; optional verdict
/// resolution runs in a separate transaction via [`check_and_resolve_verdict`] (same split as
/// [`cast_vote`]).
pub async fn abstain_from_invite(
    pool: &MySqlPool,
    user_id: u64,
    invite_id: u64,
) -> Result<AbstainOutcome, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let row = lock_case_and_invite_row_for_update(&mut tx, invite_id).await?;

    let Some((case_id, juror_user_id, current_status_key)) = row else {
        tx.rollback().await?;
        return Ok(AbstainOutcome::NotInvited);
    };

    if juror_user_id != Some(user_id) {
        tx.rollback().await?;
        return Ok(AbstainOutcome::NotInvited);
    }

    if current_status_key != ref_keys::KEY_JCJI_PENDING {
        tx.rollback().await?;
        return Ok(AbstainOutcome::AlreadyResponded);
    }

    // Count prior abstentions in the rolling window **before** marking this
    // one so the count represents the pre-flip state.
    let prior_in_window: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM judicial_case_juror_invite jcji
        INNER JOIN ref_key_values rk
            ON rk.id = jcji.invite_status_id
           AND rk.`group` = ?
           AND rk.`key` = ?
           AND rk.is_active = 1
        WHERE jcji.juror_user_id = ?
          AND COALESCE(jcji.responded_at, jcji.selected_at)
              >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL ? DAY)
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_ABSTAINED)
    .bind(user_id)
    .bind(ABSTENTION_WINDOW_DAYS)
    .fetch_one(&mut *tx)
    .await?;
    let prior_in_window = prior_in_window.max(0) as u32;

    let trigger_key: String = sqlx::query_scalar(
        r#"
        SELECT rk.`key`
        FROM judicial_case jc
        INNER JOIN ref_key_values rk
            ON rk.id = jc.trigger_type_id
           AND rk.`group` = ?
           AND rk.is_active = 1
        WHERE jc.id = ?
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(case_id)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE judicial_case_juror_invite jcji
        INNER JOIN ref_key_values rk_pending
            ON rk_pending.`group` = ?
           AND rk_pending.`key` = ?
           AND rk_pending.is_active = 1
           AND rk_pending.id = jcji.invite_status_id
        INNER JOIN ref_key_values rk_abs
            ON rk_abs.`group` = ?
           AND rk_abs.`key` = ?
           AND rk_abs.is_active = 1
        SET jcji.invite_status_id = rk_abs.id,
            jcji.responded_at = UTC_TIMESTAMP(6),
            jcji.modified_by = ?
        WHERE jcji.id = ?
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_PENDING)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_ABSTAINED)
    .bind(user_id)
    .bind(invite_id)
    .execute(&mut *tx)
    .await?;

    audit::touch_judicial_case_modified_by_tx(&mut tx, case_id, user_id).await?;

    tx.commit().await?;

    let _ = super::try_resolve_verdict(pool, case_id).await?;

    let severe_case =
        super::moderation_tier::case_is_severe_for_juror_abstention(pool, case_id, &trigger_key)
            .await?;
    if severe_case || prior_in_window < MAX_FREE_ABSTENTIONS_PER_YEAR {
        Ok(AbstainOutcome::Free { prior_in_window })
    } else {
        Ok(AbstainOutcome::OverFreeQuota {
            prior_in_window,
            cap: MAX_FREE_ABSTENTIONS_PER_YEAR,
        })
    }
}

/// Hard conflict: juror declines to participate (does not consume abstention quota).
pub async fn decline_from_invite(
    pool: &MySqlPool,
    user_id: u64,
    invite_id: u64,
) -> Result<DeclineOutcome, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let row = lock_case_and_invite_row_for_update(&mut tx, invite_id).await?;

    let Some((case_id, juror_user_id, current_status_key)) = row else {
        tx.rollback().await?;
        return Ok(DeclineOutcome::NotInvited);
    };

    if juror_user_id != Some(user_id) {
        tx.rollback().await?;
        return Ok(DeclineOutcome::NotInvited);
    }

    if current_status_key != ref_keys::KEY_JCJI_PENDING {
        tx.rollback().await?;
        return Ok(DeclineOutcome::AlreadyResponded);
    }

    sqlx::query(
        r#"
        UPDATE judicial_case_juror_invite jcji
        INNER JOIN ref_key_values rk_pending
            ON rk_pending.`group` = ?
           AND rk_pending.`key` = ?
           AND rk_pending.is_active = 1
           AND rk_pending.id = jcji.invite_status_id
        INNER JOIN ref_key_values rk_decl
            ON rk_decl.`group` = ?
           AND rk_decl.`key` = ?
           AND rk_decl.is_active = 1
        SET jcji.invite_status_id = rk_decl.id,
            jcji.responded_at = UTC_TIMESTAMP(6),
            jcji.modified_by = ?
        WHERE jcji.id = ?
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_PENDING)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_DECLINED)
    .bind(user_id)
    .bind(invite_id)
    .execute(&mut *tx)
    .await?;

    audit::touch_judicial_case_modified_by_tx(&mut tx, case_id, user_id).await?;

    tx.commit().await?;

    let _ = super::try_resolve_verdict(pool, case_id).await?;

    Ok(DeclineOutcome::Recorded)
}
pub async fn cast_vote(
    pool: &MySqlPool,
    user_id: u64,
    invite_id: u64,
    direction: &str,
) -> Result<CastVoteOutcome, sqlx::Error> {
    assert!(
        direction == ref_keys::KEY_JCJI_VOTE_FOR || direction == ref_keys::KEY_JCJI_VOTE_AGAINST,
        "vote direction must be for|against ref keys"
    );

    let mut tx = pool.begin().await?;

    let row = lock_case_and_invite_row_for_update(&mut tx, invite_id).await?;

    let Some((case_id, juror_user_id, current_status_key)) = row else {
        tx.rollback().await?;
        return Ok(CastVoteOutcome::NotInvited);
    };

    if juror_user_id != Some(user_id) {
        tx.rollback().await?;
        return Ok(CastVoteOutcome::NotInvited);
    }

    if current_status_key != ref_keys::KEY_JCJI_PENDING {
        tx.rollback().await?;
        return Ok(CastVoteOutcome::AlreadyResponded);
    }

    sqlx::query(
        r#"
        UPDATE judicial_case_juror_invite jcji
        INNER JOIN ref_key_values rk_pending
            ON rk_pending.`group` = ?
           AND rk_pending.`key` = ?
           AND rk_pending.is_active = 1
           AND rk_pending.id = jcji.invite_status_id
        INNER JOIN ref_key_values rk_acc
            ON rk_acc.`group` = ?
           AND rk_acc.`key` = ?
           AND rk_acc.is_active = 1
        INNER JOIN ref_key_values rk_vote
            ON rk_vote.`group` = ?
           AND rk_vote.`key` = ?
           AND rk_vote.is_active = 1
        SET jcji.invite_status_id = rk_acc.id,
            jcji.vote_direction_id = rk_vote.id,
            jcji.responded_at = UTC_TIMESTAMP(6),
            jcji.modified_by = ?
        WHERE jcji.id = ?
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_PENDING)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_ACCEPTED)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_VOTE_DIRECTION)
    .bind(direction)
    .bind(user_id)
    .bind(invite_id)
    .execute(&mut *tx)
    .await?;

    let tally_col = if direction == ref_keys::KEY_JCJI_VOTE_FOR {
        "jury_votes_for"
    } else {
        "jury_votes_against"
    };
    let tally_sql = format!(
        "UPDATE judicial_case SET {tally_col} = {tally_col} + 1, modified_by = ? WHERE id = ?"
    );
    sqlx::query(&tally_sql)
        .bind(user_id)
        .bind(case_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    let _ = super::try_resolve_verdict(pool, case_id).await?;
    Ok(CastVoteOutcome::Recorded)
}
pub(crate) async fn count_judicial_abstentions_in_window_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    user_id: u64,
) -> Result<u32, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM judicial_case_juror_invite jcji
        INNER JOIN ref_key_values rk
            ON rk.id = jcji.invite_status_id
           AND rk.`group` = ?
           AND rk.`key` = ?
           AND rk.is_active = 1
        WHERE jcji.juror_user_id = ?
          AND COALESCE(jcji.responded_at, jcji.selected_at)
              >= DATE_SUB(UTC_TIMESTAMP(6), INTERVAL ? DAY)
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_ABSTAINED)
    .bind(user_id)
    .bind(ABSTENTION_WINDOW_DAYS)
    .fetch_one(&mut **tx)
    .await?;
    Ok(count.max(0) as u32)
}
pub async fn find_pending_human_jury_invite_for_user(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<Option<PendingJurorInvite>, sqlx::Error> {
    let row: Option<(u64, u64, Option<u64>, String, Option<String>, Option<String>)> =
        sqlx::query_as(
            r#"
        SELECT
            jcji.id,
            jc.id,
            jc.subject_post_id,
            rk_t.`key`,
            se.handle,
            COALESCE(NULLIF(TRIM(sc.label), ''), se.handle)
        FROM judicial_case_juror_invite jcji
        INNER JOIN judicial_case jc ON jc.id = jcji.judicial_case_id
        INNER JOIN ref_key_values rk_t
            ON rk_t.id = jc.trigger_type_id
           AND rk_t.`group` = ?
           AND rk_t.`key` IN (?, ?, ?, ?, ?, ?, ?)
           AND rk_t.is_active = 1
        INNER JOIN ref_key_values rk_s
            ON rk_s.id = jc.status_id
           AND rk_s.`group` = ?
           AND rk_s.`key` IN (?, ?)
           AND rk_s.is_active = 1
        INNER JOIN ref_key_values rk_inv
            ON rk_inv.id = jcji.invite_status_id
           AND rk_inv.`group` = ?
           AND rk_inv.`key` = ?
           AND rk_inv.is_active = 1
        LEFT JOIN social_entity se ON se.id = jc.target_social_entity_id
        LEFT JOIN social_community sc ON sc.social_entity_id = se.id
        WHERE jcji.juror_user_id = ?
          AND (
              jc.subject_post_id IS NOT NULL
              OR rk_t.`key` IN (?, ?, ?, ?)
          )
        ORDER BY jcji.selected_at ASC, jcji.id ASC
        LIMIT 1
        "#,
        )
        .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
        .bind(ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE)
        .bind(ref_keys::KEY_JUDICIAL_TRIGGER_POST_DISPUTE)
        .bind(ref_keys::KEY_JUDICIAL_TRIGGER_EMERGENCY_SAFETY)
        .bind(ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT)
        .bind(ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_PROFILE_AMENDMENT)
        .bind(ref_keys::KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW)
        .bind(ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_REPARENT)
        .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
        .bind(ref_keys::KEY_JUDICIAL_CASE_OPEN)
        .bind(ref_keys::KEY_JUDICIAL_CASE_JURY_ACTIVE)
        .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
        .bind(ref_keys::KEY_JCJI_PENDING)
        .bind(user_id)
        .bind(ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT)
        .bind(ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_PROFILE_AMENDMENT)
        .bind(ref_keys::KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW)
        .bind(ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_REPARENT)
        .fetch_optional(pool)
        .await?;

    let Some((
        invite_id,
        case_id,
        subject_post_id,
        trigger_key,
        community_handle,
        community_label,
    )) = row
    else {
        return Ok(None);
    };

    let is_severe_case =
        super::moderation_tier::case_is_severe_for_juror_abstention(pool, case_id, &trigger_key)
            .await?;

    Ok(Some(PendingJurorInvite {
        invite_id,
        case_id,
        subject_post_id,
        trigger_key,
        community_handle,
        community_label,
        is_severe_case,
    }))
}

/// Oldest pending post-dispute invite (legacy helper).
pub async fn find_pending_post_dispute_invite_for_user(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<Option<PendingJurorInvite>, sqlx::Error> {
    Ok(find_pending_human_jury_invite_for_user(pool, user_id)
        .await?
        .filter(|inv| {
            inv.trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE
                || inv.trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_POST_DISPUTE
        }))
}
