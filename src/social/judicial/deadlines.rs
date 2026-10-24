use sqlx::{MySql, MySqlPool};

use crate::ref_key_values;
use crate::ref_keys;
use crate::social::audit;

use super::case_access::fetch_open_case_row_tx;
use super::community_reparent;
use super::membership_review;
use super::moderation_tier::{REGULAR_NO_JURY_DEADLINE_DAYS, SERIOUS_NO_JURY_DEADLINE_DAYS};
use super::rating::apply_community_no_jury_penalty_tx;

/// When a post-moderation case is still `open` or `jury_active` after this many days, it is closed
/// as [`ref_keys::KEY_JUDICIAL_CASE_NO_JURY`] and the feed-scoped community (if any) is penalized.
pub(crate) const COMMUNITY_NO_JURY_DEADLINE_SECS: i64 = REGULAR_NO_JURY_DEADLINE_DAYS * 86_400;

fn post_dispute_no_jury_deadline_predicate_sql() -> String {
    format!(
        r#"
        (
            rk_tr.`key` NOT IN ('civic_dispute', 'post_moderation', 'emergency_safety')
            AND jc.opened_at <= UTC_TIMESTAMP(6) - INTERVAL {regular} DAY
        )
        OR (
            rk_tr.`key` IN ('civic_dispute', 'post_moderation', 'emergency_safety')
            AND EXISTS (
                SELECT 1
                FROM judicial_case_constitution_item jcci
                INNER JOIN community_constitution_item cci ON cci.id = jcci.constitution_item_id
                INNER JOIN ref_key_values rk_tier ON rk_tier.id = cci.moderation_tier_id
                WHERE jcci.judicial_case_id = jc.id
                  AND rk_tier.`group` = '{tier_group}'
                  AND rk_tier.`key` = '{serious}'
                  AND rk_tier.is_active = 1
            )
            AND jc.opened_at <= UTC_TIMESTAMP(6) - INTERVAL {serious_days} DAY
        )
        OR (
            rk_tr.`key` IN ('civic_dispute', 'post_moderation', 'emergency_safety')
            AND EXISTS (
                SELECT 1
                FROM judicial_case_constitution_item jcci
                INNER JOIN community_constitution_item cci ON cci.id = jcci.constitution_item_id
                WHERE jcci.judicial_case_id = jc.id
                  AND cci.point IN (402, 404)
            )
            AND jc.opened_at <= UTC_TIMESTAMP(6) - INTERVAL {serious_days} DAY
        )
        OR (
            rk_tr.`key` IN ('civic_dispute', 'post_moderation')
            AND NOT EXISTS (
                SELECT 1
                FROM judicial_case_constitution_item jcci
                INNER JOIN community_constitution_item cci ON cci.id = jcci.constitution_item_id
                INNER JOIN ref_key_values rk_tier ON rk_tier.id = cci.moderation_tier_id
                WHERE jcci.judicial_case_id = jc.id
                  AND rk_tier.`group` = '{tier_group}'
                  AND rk_tier.`key` = '{serious}'
                  AND rk_tier.is_active = 1
            )
            AND jc.opened_at <= UTC_TIMESTAMP(6) - INTERVAL {regular} DAY
        )
        "#,
        regular = REGULAR_NO_JURY_DEADLINE_DAYS,
        serious_days = SERIOUS_NO_JURY_DEADLINE_DAYS,
        tier_group = ref_keys::GROUP_JUDICIAL_MODERATION_TIER,
        serious = ref_keys::KEY_JUDICIAL_MODERATION_TIER_SERIOUS,
    )
}

async fn void_pending_invites_for_no_jury_timeout_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    case_id: u64,
    system_actor: Option<u64>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE judicial_case_juror_invite jcji
        INNER JOIN ref_key_values rk_void
            ON rk_void.`group` = ?
           AND rk_void.`key` = ?
           AND rk_void.is_active = 1
        INNER JOIN ref_key_values rk_cur
            ON rk_cur.id = jcji.invite_status_id
           AND rk_cur.`group` = ?
           AND rk_cur.`key` = ?
           AND rk_cur.is_active = 1
        SET jcji.invite_status_id = rk_void.id,
            jcji.modified_by = ?
        WHERE jcji.judicial_case_id = ?
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_VOIDED)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_PENDING)
    .bind(system_actor)
    .bind(case_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn close_one_jury_case_no_jury_deadline(
    pool: &MySqlPool,
    case_id: u64,
    no_jury_status_id: u64,
    trigger_key: &str,
) -> Result<(), sqlx::Error> {
    let system_actor = audit::system_user_id(pool).await?;
    let mut tx = pool.begin().await?;

    let deadline_sql = post_dispute_no_jury_deadline_predicate_sql();
    let eligible: Option<u64> = sqlx::query_scalar(&format!(
        r#"
        SELECT jc.id
        FROM judicial_case jc
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = jc.status_id
           AND rk_st.`group` = ?
           AND rk_st.`key` IN (?, ?)
           AND rk_st.is_active = 1
        INNER JOIN ref_key_values rk_tr
            ON rk_tr.id = jc.trigger_type_id
           AND rk_tr.`group` = ?
           AND rk_tr.`key` = ?
           AND rk_tr.is_active = 1
        WHERE jc.id = ?
          AND jc.community_rating_feedback_applied_at IS NULL
          AND ({deadline_sql})
        FOR UPDATE
        "#
    ))
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_OPEN)
    .bind(ref_keys::KEY_JUDICIAL_CASE_JURY_ACTIVE)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(trigger_key)
    .bind(case_id)
    .fetch_optional(&mut *tx)
    .await?;

    if eligible.is_none() {
        tx.rollback().await?;
        return Ok(());
    }

    let Some(case_row) = fetch_open_case_row_tx(&mut tx, case_id).await? else {
        tx.rollback().await?;
        return Ok(());
    };

    void_pending_invites_for_no_jury_timeout_tx(&mut tx, case_id, system_actor).await?;

    sqlx::query(
        r#"
        UPDATE judicial_case jc
        SET jc.status_id = ?,
            jc.closed_at = UTC_TIMESTAMP(6),
            jc.modified_by = ?
        WHERE jc.id = ?
        "#,
    )
    .bind(no_jury_status_id)
    .bind(system_actor)
    .bind(case_id)
    .execute(&mut *tx)
    .await?;

    if trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT {
        crate::social::judicial_constitutional::apply_constitutional_no_jury_penalties_tx(
            &mut tx, case_id,
        )
        .await?;
    } else if trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_PROFILE_AMENDMENT {
        crate::social::judicial_community_profile::apply_profile_amendment_no_jury_penalties_tx(
            &mut tx, case_id,
        )
        .await?;
    } else if trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW {
        membership_review::apply_no_jury_settlement_tx(&mut tx, case_id, &case_row).await?;
    } else if trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_REPARENT {
        community_reparent::apply_no_jury_settlement_tx(&mut tx, case_id, &case_row).await?;
        apply_community_no_jury_penalty_tx(&mut tx, case_id, &case_row).await?;
    } else if super::triggers::trigger_is_civic_dispute(&trigger_key) {
        apply_community_no_jury_penalty_tx(&mut tx, case_id, &case_row).await?;
        super::verdict::settle_post_moderation_subject_after_no_jury_tx(
            &mut tx,
            case_id,
            system_actor,
        )
        .await?;
    } else {
        apply_community_no_jury_penalty_tx(&mut tx, case_id, &case_row).await?;
    }

    sqlx::query(
        r#"UPDATE judicial_case SET community_rating_feedback_applied_at = UTC_TIMESTAMP(6), modified_by = ? WHERE id = ?"#,
    )
    .bind(system_actor)
    .bind(case_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Close a full-pool community jury case when the eligible pool cannot fill the frozen jury.
/// Returns `Ok(true)` when the case was transitioned to `no_jury`.
pub(crate) async fn close_full_community_jury_pool_insufficient_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    case_id: u64,
    trigger_key: &str,
) -> Result<bool, sqlx::Error> {
    let Some(no_jury_status_id) = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_NO_JURY,
    )
    .await?
    else {
        return Err(sqlx::Error::Configuration(
            "ref_key_values missing judicial_case_status.no_jury".into(),
        ));
    };

    let system_actor: Option<u64> = sqlx::query_scalar(
        r#"SELECT id FROM users WHERE email = ? LIMIT 1"#,
    )
    .bind(ref_keys::SYSTEM_USER_EMAIL)
    .fetch_optional(&mut **tx)
    .await?;

    let eligible: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT jc.id
        FROM judicial_case jc
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = jc.status_id
           AND rk_st.`group` = ?
           AND rk_st.`key` IN (?, ?)
           AND rk_st.is_active = 1
        INNER JOIN ref_key_values rk_tr
            ON rk_tr.id = jc.trigger_type_id
           AND rk_tr.`group` = ?
           AND rk_tr.`key` = ?
           AND rk_tr.is_active = 1
        WHERE jc.id = ?
          AND jc.community_rating_feedback_applied_at IS NULL
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_OPEN)
    .bind(ref_keys::KEY_JUDICIAL_CASE_JURY_ACTIVE)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(trigger_key)
    .bind(case_id)
    .fetch_optional(&mut **tx)
    .await?;

    if eligible.is_none() {
        return Ok(false);
    }

    void_pending_invites_for_no_jury_timeout_tx(tx, case_id, system_actor).await?;

    sqlx::query(
        r#"
        UPDATE judicial_case jc
        SET jc.status_id = ?,
            jc.closed_at = UTC_TIMESTAMP(6),
            jc.modified_by = ?
        WHERE jc.id = ?
        "#,
    )
    .bind(no_jury_status_id)
    .bind(system_actor)
    .bind(case_id)
    .execute(&mut **tx)
    .await?;

    if trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_PROFILE_AMENDMENT {
        crate::social::judicial_community_profile::apply_profile_amendment_no_jury_penalties_tx(
            tx, case_id,
        )
        .await?;
    } else {
        crate::social::judicial_constitutional::apply_constitutional_no_jury_penalties_tx(tx, case_id)
            .await?;
    }

    sqlx::query(
        r#"UPDATE judicial_case SET community_rating_feedback_applied_at = UTC_TIMESTAMP(6), modified_by = ? WHERE id = ?"#,
    )
    .bind(system_actor)
    .bind(case_id)
    .execute(&mut **tx)
    .await?;

    Ok(true)
}

/// Close post-moderation, membership-review, and constitutional cases still `open` /
/// `jury_active` after their tier deadline.
pub async fn close_jury_cases_past_no_jury_deadline(
    pool: &MySqlPool,
    limit: u64,
) -> Result<u64, sqlx::Error> {
    let Some(no_jury_status_id) = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_NO_JURY,
    )
    .await?
    else {
        tracing::warn!(
            "ref_key_values missing judicial_case_status.no_jury; skipping jury deadline closes"
        );
        return Ok(0);
    };

    let lim = limit.max(1).min(256) as i64;
    let deadline_sql = post_dispute_no_jury_deadline_predicate_sql();
    let candidates: Vec<(u64, String)> = sqlx::query_as(&format!(
        r#"
        SELECT jc.id, rk_tr.`key`
        FROM judicial_case jc
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = jc.status_id
           AND rk_st.`group` = ?
           AND rk_st.`key` IN (?, ?)
           AND rk_st.is_active = 1
        INNER JOIN ref_key_values rk_tr
            ON rk_tr.id = jc.trigger_type_id
           AND rk_tr.`group` = ?
           AND rk_tr.`key` IN (?, ?, ?, ?, ?, ?, ?)
           AND rk_tr.is_active = 1
        WHERE jc.community_rating_feedback_applied_at IS NULL
          AND ({deadline_sql})
        ORDER BY jc.opened_at ASC, jc.id ASC
        LIMIT ?
        "#
    ))
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_OPEN)
    .bind(ref_keys::KEY_JUDICIAL_CASE_JURY_ACTIVE)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE)
    .bind(ref_keys::KEY_JUDICIAL_TRIGGER_POST_DISPUTE)
    .bind(ref_keys::KEY_JUDICIAL_TRIGGER_EMERGENCY_SAFETY)
    .bind(ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT)
    .bind(ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_PROFILE_AMENDMENT)
    .bind(ref_keys::KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW)
    .bind(ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_REPARENT)
    .bind(lim)
    .fetch_all(pool)
    .await?;

    let mut closed = 0u64;
    for (case_id, trigger_key) in candidates {
        match close_one_jury_case_no_jury_deadline(pool, case_id, no_jury_status_id, &trigger_key)
            .await
        {
            Ok(()) => closed += 1,
            Err(err) => {
                tracing::warn!(error = %err, case_id, "close_one_jury_case_no_jury_deadline failed");
            }
        }
    }
    Ok(closed)
}

/// Back-compat alias for scheduler call sites.
pub async fn close_post_moderation_cases_past_no_jury_deadline(
    pool: &MySqlPool,
    limit: u64,
) -> Result<u64, sqlx::Error> {
    close_jury_cases_past_no_jury_deadline(pool, limit).await
}
