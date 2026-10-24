//! Non-filing Panoply operator actions.

use sqlx::MySqlPool;

use crate::ref_key_values;
use crate::ref_keys;
use crate::social::community_graph::is_human_death_immune;
use crate::social::judicial::{platform_safety_ban_human_tx, restore_post_thread_tx};

use super::audit::{insert_action, ActionType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionError {
    HandleNotFound,
    HumanNotFound,
    DeathImmune,
    AlreadyDead,
    CaseNotFound,
    CaseNotVacatable,
    DomainInvalid,
    Db,
}

pub async fn eject_human_by_handle(
    pool: &MySqlPool,
    operator_user_id: u64,
    handle: &str,
    note: &str,
) -> Result<(), ActionError> {
    let handle = handle.trim().trim_start_matches('@');
    if handle.is_empty() {
        return Err(ActionError::HandleNotFound);
    }
    let row: Option<(u64, Option<u64>)> = sqlx::query_as(
        r#"
        SELECT se.id, ur.user_id
        FROM social_entity se
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        LEFT JOIN user_social_entity_role ur ON ur.social_entity_id = se.id AND ur.ends_at IS NULL
        WHERE se.handle = ?
        LIMIT 1
        "#,
    )
    .bind(handle)
    .fetch_optional(pool)
    .await
    .map_err(|_| ActionError::Db)?;
    let Some((human_entity_id, target_user_id)) = row else {
        return Err(ActionError::HandleNotFound);
    };
    if is_human_death_immune(pool, human_entity_id)
        .await
        .map_err(|_| ActionError::Db)?
    {
        return Err(ActionError::DeathImmune);
    }
    let applied = platform_safety_ban_human_tx(pool, human_entity_id, Some(operator_user_id))
        .await
        .map_err(|_| ActionError::Db)?;
    if !applied {
        return Err(ActionError::AlreadyDead);
    }
    insert_action(
        pool,
        operator_user_id,
        ActionType::Eject,
        None,
        target_user_id,
        None,
        None,
        Some("panoply"),
        note,
    )
    .await
    .map_err(|_| ActionError::Db)?;
    Ok(())
}

pub async fn kill_sessions_for_handle(
    pool: &MySqlPool,
    operator_user_id: u64,
    handle: &str,
    note: &str,
) -> Result<(), ActionError> {
    let handle = handle.trim().trim_start_matches('@');
    let user_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT ur.user_id
        FROM social_entity se
        INNER JOIN user_social_entity_role ur ON ur.social_entity_id = se.id AND ur.ends_at IS NULL
        WHERE se.handle = ?
        LIMIT 1
        "#,
    )
    .bind(handle)
    .fetch_optional(pool)
    .await
    .map_err(|_| ActionError::Db)?;
    let Some(user_id) = user_id else {
        return Err(ActionError::HandleNotFound);
    };
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|_| ActionError::Db)?;
    insert_action(
        pool,
        operator_user_id,
        ActionType::SessionKill,
        None,
        Some(user_id),
        None,
        None,
        Some("panoply"),
        note,
    )
    .await
    .map_err(|_| ActionError::Db)?;
    Ok(())
}

pub async fn block_domain(
    pool: &MySqlPool,
    operator_user_id: u64,
    domain: &str,
    note: &str,
) -> Result<(), ActionError> {
    let domain = domain.trim().to_ascii_lowercase();
    if domain.is_empty() || domain.contains('/') {
        return Err(ActionError::DomainInvalid);
    }
    let blocked_status = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_STATUS,
        ref_keys::KEY_BLOCKED,
    )
    .await
    .map_err(|_| ActionError::Db)?
    .ok_or(ActionError::Db)?;

    sqlx::query(
        r#"
        INSERT INTO content_domain (
            domain, status_id, rating, first_seen_at, last_seen_at,
            safety_checked_at, safety_state, safety_note, created_by, modified_by
        )
        VALUES (?, ?, 0.00, UTC_TIMESTAMP(6), UTC_TIMESTAMP(6), UTC_TIMESTAMP(6), 'unsafe', ?, ?, ?)
        ON DUPLICATE KEY UPDATE
            status_id = VALUES(status_id),
            safety_state = 'unsafe',
            safety_checked_at = UTC_TIMESTAMP(6),
            safety_note = VALUES(safety_note),
            modified_by = VALUES(modified_by)
        "#,
    )
    .bind(&domain)
    .bind(blocked_status)
    .bind(note)
    .bind(operator_user_id)
    .bind(operator_user_id)
    .execute(pool)
    .await
    .map_err(|_| ActionError::Db)?;

    insert_action(
        pool,
        operator_user_id,
        ActionType::DomainBlock,
        None,
        None,
        Some(&domain),
        None,
        Some("panoply"),
        note,
    )
    .await
    .map_err(|_| ActionError::Db)?;
    Ok(())
}

pub async fn vacate_emergency_case(
    pool: &MySqlPool,
    operator_user_id: u64,
    case_id: u64,
    note: &str,
) -> Result<(), ActionError> {
    let row: Option<(u64, Option<u64>)> = sqlx::query_as(
        r#"
        SELECT jc.subject_post_id, jc.contested_at
        FROM judicial_case jc
        INNER JOIN ref_key_values rk_tr ON rk_tr.id = jc.trigger_type_id
        INNER JOIN ref_key_values rk_st ON rk_st.id = jc.status_id
        WHERE jc.id = ?
          AND rk_tr.`group` = ?
          AND rk_tr.`key` = ?
          AND rk_st.`group` = ?
          AND rk_st.`key` = ?
        LIMIT 1
        "#,
    )
    .bind(case_id)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(ref_keys::KEY_JUDICIAL_TRIGGER_EMERGENCY_SAFETY)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_CONTAINED)
    .fetch_optional(pool)
    .await
    .map_err(|_| ActionError::Db)?;
    let Some((root_post_id, contested_at)) = row else {
        return Err(ActionError::CaseNotFound);
    };
    if contested_at.is_some() {
        return Err(ActionError::CaseNotVacatable);
    }

    let void_status_id = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_VOIDED,
    )
    .await
    .map_err(|_| ActionError::Db)?
    .ok_or(ActionError::Db)?;

    let mut tx = pool.begin().await.map_err(|_| ActionError::Db)?;
    sqlx::query(
        r#"
        UPDATE judicial_case
        SET status_id = ?, closed_at = UTC_TIMESTAMP(6), modified_by = ?
        WHERE id = ?
        "#,
    )
    .bind(void_status_id)
    .bind(operator_user_id)
    .bind(case_id)
    .execute(&mut *tx)
    .await
    .map_err(|_| ActionError::Db)?;

    restore_post_thread_tx(&mut tx, root_post_id, operator_user_id)
        .await
        .map_err(|_| ActionError::Db)?;

    super::audit::insert_action_tx(
        &mut tx,
        operator_user_id,
        ActionType::VacateEmergency,
        Some(root_post_id),
        None,
        None,
        Some(case_id),
        Some("panoply"),
        note,
    )
    .await
    .map_err(|_| ActionError::Db)?;

    tx.commit().await.map_err(|_| ActionError::Db)?;
    Ok(())
}
