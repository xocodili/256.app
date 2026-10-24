//! Emergency safety filings (Root §4.1–§4.7): suppress post, optional contest → jury.

use sqlx::{MySql, MySqlPool, Transaction};

use crate::ref_key_values;
use crate::ref_keys;
use crate::social::community_graph::{self, DeathSource};
use crate::social::judicial::{
    bump_community_member_rating_tx, bump_global_entity_rating_tx, civic_rating_delta_times,
};
use crate::social::post_community;

pub const MAX_EMERGENCY_FILINGS_PER_USER_PER_DAY: i64 = 3;
pub const EMERGENCY_STANDING_SETTLEMENT_DAYS: i64 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmergencyFilingError {
    DailyCap,
    DuplicatePost,
    MixedClauseKinds,
    NotEmergencyClause,
    NotCivicClause,
    CaseNotFound,
    NotAuthor,
    AlreadyContested,
    NotContained,
}

#[must_use]
pub fn is_emergency_safety_point(point: u16) -> bool {
    (401..=407).contains(&point)
}

/// Suppress entire thread (root + replies).
pub async fn suppress_post_thread_tx(
    tx: &mut Transaction<'_, MySql>,
    root_post_id: u64,
    actor: u64,
) -> Result<(), sqlx::Error> {
    let Some(suppressed_id) = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_STATUS,
        ref_keys::KEY_POST_SUPPRESSED,
    )
    .await?
    else {
        return Ok(());
    };
    sqlx::query(
        r#"
        UPDATE post
        SET status_id = ?, modified_by = ?
        WHERE id = ? OR root_post_id = ?
        "#,
    )
    .bind(suppressed_id)
    .bind(actor)
    .bind(root_post_id)
    .bind(root_post_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Restore thread to verified after successful contest.
pub async fn restore_post_thread_tx(
    tx: &mut Transaction<'_, MySql>,
    root_post_id: u64,
    actor: u64,
) -> Result<(), sqlx::Error> {
    let Some(verified_id) = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_STATUS,
        ref_keys::KEY_VERIFIED,
    )
    .await?
    else {
        return Ok(());
    };
    sqlx::query(
        r#"
        UPDATE post
        SET status_id = ?, modified_by = ?
        WHERE id = ? OR root_post_id = ?
        "#,
    )
    .bind(verified_id)
    .bind(actor)
    .bind(root_post_id)
    .bind(root_post_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn check_daily_emergency_filing_cap_tx(
    tx: &mut Transaction<'_, MySql>,
    filer_user_id: u64,
) -> Result<(), EmergencyFilingError> {
    let trigger_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_JUDICIAL_CASE_TRIGGER,
        ref_keys::KEY_JUDICIAL_TRIGGER_EMERGENCY_SAFETY,
    )
    .await
    .map_err(|_| EmergencyFilingError::DailyCap)?
    .ok_or(EmergencyFilingError::DailyCap)?;

    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM judicial_case jc
        WHERE jc.trigger_type_id = ?
          AND jc.created_by = ?
          AND jc.opened_at >= UTC_DATE()
          AND jc.opened_at < (UTC_DATE() + INTERVAL 1 DAY)
        "#,
    )
    .bind(trigger_id)
    .bind(filer_user_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|_| EmergencyFilingError::DailyCap)?;

    if count >= MAX_EMERGENCY_FILINGS_PER_USER_PER_DAY {
        return Err(EmergencyFilingError::DailyCap);
    }
    Ok(())
}

pub async fn active_emergency_case_on_post_tx(
    tx: &mut Transaction<'_, MySql>,
    root_post_id: u64,
) -> Result<bool, sqlx::Error> {
    let trigger_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_JUDICIAL_CASE_TRIGGER,
        ref_keys::KEY_JUDICIAL_TRIGGER_EMERGENCY_SAFETY,
    )
    .await?;
    let Some(trigger_id) = trigger_id else {
        return Ok(false);
    };
    let contained_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_CONTAINED,
    )
    .await?;
    let open_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_OPEN,
    )
    .await?;
    let jury_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_JURY_ACTIVE,
    )
    .await?;

    let mut status_ids = Vec::new();
    if let Some(id) = contained_id {
        status_ids.push(id);
    }
    if let Some(id) = open_id {
        status_ids.push(id);
    }
    if let Some(id) = jury_id {
        status_ids.push(id);
    }
    if status_ids.is_empty() {
        return Ok(false);
    }

    let ph = std::iter::repeat_n("?", status_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        r#"
        SELECT COUNT(*)
        FROM judicial_case jc
        WHERE jc.subject_post_id = ?
          AND jc.trigger_type_id = ?
          AND jc.status_id IN ({ph})
        "#,
        ph = ph
    );
    let mut q = sqlx::query_scalar::<_, i64>(&sql)
        .bind(root_post_id)
        .bind(trigger_id);
    for id in status_ids {
        q = q.bind(id);
    }
    let count = q.fetch_one(&mut **tx).await?;
    Ok(count > 0)
}

/// Loathed-equivalent standing (same multipliers as conversation settlement).
pub async fn apply_emergency_loathed_standing_tx(
    tx: &mut Transaction<'_, MySql>,
    root_post_id: u64,
    author_human_entity_id: u64,
) -> Result<(), sqlx::Error> {
    if let Some(community_id) =
        post_community::community_context_for_root_post_tx(tx, root_post_id).await?
    {
        bump_community_member_rating_tx(
            tx,
            author_human_entity_id,
            community_id,
            civic_rating_delta_times(super::types::RATING_MULT_CONVERSATION_LOATHED_EDGE),
            None,
        )
        .await?;
    }
    bump_global_entity_rating_tx(
        tx,
        author_human_entity_id,
        civic_rating_delta_times(super::types::RATING_MULT_CONVERSATION_LOATHED_GLOBAL),
        None,
    )
    .await?;
    Ok(())
}

pub async fn run_standing_settlement_batch(
    pool: &MySqlPool,
    limit: u32,
) -> Result<Vec<(u64, u64)>, sqlx::Error> {
    let lim = limit.clamp(1, 256) as i64;
    let trigger_id = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_JUDICIAL_CASE_TRIGGER,
        ref_keys::KEY_JUDICIAL_TRIGGER_EMERGENCY_SAFETY,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("missing judicial_case_trigger.emergency_safety".into())
    })?;
    let contained_id = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_CONTAINED,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("missing judicial_case_status.contained".into())
    })?;

    let rows: Vec<(u64, u64, u64)> = sqlx::query_as(
        r#"
        SELECT jc.id, jc.subject_post_id, p.entity_id
        FROM judicial_case jc
        INNER JOIN post p ON p.id = jc.subject_post_id
        WHERE jc.trigger_type_id = ?
          AND jc.status_id = ?
          AND jc.contested_at IS NULL
          AND jc.standing_applied_at IS NULL
          AND jc.opened_at <= UTC_TIMESTAMP(6) - INTERVAL ? DAY
        ORDER BY jc.opened_at ASC
        LIMIT ?
        "#,
    )
    .bind(trigger_id)
    .bind(contained_id)
    .bind(EMERGENCY_STANDING_SETTLEMENT_DAYS)
    .bind(lim)
    .fetch_all(pool)
    .await?;

    let mut notified = Vec::new();
    for (case_id, root_post_id, author_entity_id) in rows {
        let author_user_id: Option<u64> = sqlx::query_scalar(
            r#"SELECT created_by FROM post WHERE id = ? LIMIT 1"#,
        )
        .bind(root_post_id)
        .fetch_optional(pool)
        .await?;
        let mut tx = pool.begin().await?;
        let locked: Option<u64> = sqlx::query_scalar(
            r#"
            SELECT id FROM judicial_case
            WHERE id = ?
              AND standing_applied_at IS NULL
              AND contested_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(case_id)
        .fetch_optional(&mut *tx)
        .await?;
        if locked.is_none() {
            tx.rollback().await?;
            continue;
        }
        apply_emergency_loathed_standing_tx(&mut tx, root_post_id, author_entity_id).await?;
        super::acute_harm_content::apply_or_refund_acute_harm_content_rating_tx(
            &mut tx,
            case_id,
            root_post_id,
            super::acute_harm_content::AcuteHarmDirection::Apply,
        )
        .await?;
        sqlx::query(
            r#"
            UPDATE judicial_case
            SET standing_applied_at = UTC_TIMESTAMP(6)
            WHERE id = ?
            "#,
        )
        .bind(case_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        if let Some(uid) = author_user_id {
            notified.push((uid, root_post_id));
        }
        let _ = case_id;
    }
    Ok(notified)
}

/// Platform safety ban: entity dead + user `blocked` (not reactivatable).
pub async fn platform_safety_ban_human_tx(
    pool: &MySqlPool,
    human_entity_id: u64,
    actor: Option<u64>,
) -> Result<bool, sqlx::Error> {
    community_graph::human_safety_ban_tx(pool, human_entity_id, DeathSource::ManualOps, actor).await
}

/// Classify clause ids: all emergency, all civic, or mixed error.
pub async fn classify_clause_ids_tx(
    tx: &mut Transaction<'_, MySql>,
    clause_ids: &[u64],
) -> Result<Vec<u16>, EmergencyFilingError> {
    if clause_ids.is_empty() {
        return Err(EmergencyFilingError::MixedClauseKinds);
    }
    let mut placeholders = String::new();
    for (i, _) in clause_ids.iter().enumerate() {
        if i > 0 {
            placeholders.push(',');
        }
        placeholders.push('?');
    }
    let sql = format!(
        r#"
        SELECT point FROM community_constitution_item
        WHERE id IN ({placeholders}) AND status = 'active'
        "#,
        placeholders = placeholders
    );
    let mut q = sqlx::query_scalar::<_, u16>(&sql);
    for id in clause_ids {
        q = q.bind(id);
    }
    let points: Vec<u16> = q
        .fetch_all(&mut **tx)
        .await
        .map_err(|_| EmergencyFilingError::MixedClauseKinds)?;
    if points.len() != clause_ids.len() {
        return Err(EmergencyFilingError::MixedClauseKinds);
    }
    let mut any_emergency = false;
    let mut any_civic = false;
    for point in &points {
        if is_emergency_safety_point(*point) {
            any_emergency = true;
        } else if *point != 4 {
            any_civic = true;
        }
    }
    if any_emergency && any_civic {
        return Err(EmergencyFilingError::MixedClauseKinds);
    }
    Ok(points)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContestOutcome {
    Opened { case_id: u64 },
    NotFound,
    Forbidden,
    AlreadyContested,
    NotContained,
    InsufficientNoms,
}

/// Author contests an uncontested emergency case (1 Nom) — opens jury from `contained` → `open`.
pub async fn contest_emergency_case(
    pool: &MySqlPool,
    user_id: u64,
    human_entity_id: u64,
    root_post_id: u64,
) -> Result<ContestOutcome, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let author_ok: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM post p
            WHERE COALESCE(p.root_post_id, p.id) = ?
              AND (p.created_by = ? OR p.entity_id = ?)
        )
        "#,
    )
    .bind(root_post_id)
    .bind(user_id)
    .bind(human_entity_id)
    .fetch_one(&mut *tx)
    .await?;
    if !author_ok {
        tx.rollback().await?;
        return Ok(ContestOutcome::Forbidden);
    }

    let trigger_id = ref_key_values::lookup_active_id_conn(
        &mut *tx,
        ref_keys::GROUP_JUDICIAL_CASE_TRIGGER,
        ref_keys::KEY_JUDICIAL_TRIGGER_EMERGENCY_SAFETY,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("missing judicial_case_trigger.emergency_safety".into())
    })?;
    let contained_id = ref_key_values::lookup_active_id_conn(
        &mut *tx,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_CONTAINED,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("missing judicial_case_status.contained".into())
    })?;
    let open_id = ref_key_values::lookup_active_id_conn(
        &mut *tx,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_OPEN,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("missing judicial_case_status.open".into())
    })?;

    let case_row: Option<(u64, Option<u64>)> = sqlx::query_as(
        r#"
        SELECT jc.id, jc.contested_at
        FROM judicial_case jc
        WHERE jc.subject_post_id = ?
          AND jc.trigger_type_id = ?
          AND jc.status_id = ?
        ORDER BY jc.id DESC
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .bind(root_post_id)
    .bind(trigger_id)
    .bind(contained_id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((case_id, contested_at)) = case_row else {
        tx.rollback().await?;
        return Ok(ContestOutcome::NotFound);
    };
    if contested_at.is_some() {
        tx.rollback().await?;
        return Ok(ContestOutcome::AlreadyContested);
    }

    let spend_ref = format!("emergency_contest:case:{case_id}");
    let spend_outcome = crate::social::noms::spend_for_service_tx(
        &mut tx,
        human_entity_id,
        crate::pricing::BillableService::EmergencyContest,
        user_id,
        Some(&spend_ref),
    )
    .await?;
    if !spend_outcome.is_paid() {
        tx.rollback().await?;
        return Ok(ContestOutcome::InsufficientNoms);
    }

    let target_entity_id: u64 = sqlx::query_scalar(
        r#"SELECT target_social_entity_id FROM judicial_case WHERE id = ?"#,
    )
    .bind(case_id)
    .fetch_one(&mut *tx)
    .await?;

    let jury_size_frozen = crate::social::judicial::compute_jury_size_to_freeze_at_filing_tx(
        &mut tx,
        human_entity_id,
        target_entity_id,
        Some(user_id),
        Some(root_post_id),
        ref_keys::KEY_JUDICIAL_TRIGGER_EMERGENCY_SAFETY,
    )
    .await?;

    sqlx::query(
        r#"
        UPDATE judicial_case
        SET status_id = ?,
            jury_size = ?,
            contested_at = UTC_TIMESTAMP(6),
            contested_by = ?,
            modified_by = ?
        WHERE id = ?
        "#,
    )
    .bind(open_id)
    .bind(jury_size_frozen)
    .bind(user_id)
    .bind(user_id)
    .bind(case_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(ContestOutcome::Opened { case_id })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emergency_points_are_401_to_407() {
        assert!(is_emergency_safety_point(401));
        assert!(is_emergency_safety_point(407));
        assert!(!is_emergency_safety_point(4));
        assert!(!is_emergency_safety_point(13));
    }
}
