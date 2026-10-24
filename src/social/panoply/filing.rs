//! Operator-initiated post moderation filings (reuse judicial tables; waive Noms / daily cap).

use rust_decimal::Decimal;
use sqlx::{MySql, MySqlPool, Transaction};

use crate::ref_key_values;
use crate::ref_keys;
use crate::social::judicial::{self, EmergencyFilingError};

use super::audit::{insert_action_tx, ActionType};

#[derive(Debug, Clone)]
pub struct OpsPostTarget {
    pub root_post_id: u64,
    pub post_entity_id: u64,
    pub author_user_id: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilingError {
    PostNotFound,
    MixedClauseKinds,
    DuplicateEmergency,
    InvalidClause,
    SanitationClause,
    Db,
}

impl From<sqlx::Error> for FilingError {
    fn from(_: sqlx::Error) -> Self {
        Self::Db
    }
}

pub async fn load_post_target(pool: &MySqlPool, post_id: u64) -> Result<Option<OpsPostTarget>, sqlx::Error> {
    sqlx::query_as::<_, (u64, u64, Option<u64>)>(
        r#"
        SELECT COALESCE(p.root_post_id, p.id), p.entity_id, p.created_by
        FROM post p
        WHERE p.id = ?
        LIMIT 1
        "#,
    )
    .bind(post_id)
    .fetch_optional(pool)
    .await
    .map(|row| {
        row.map(|(root_post_id, post_entity_id, author_user_id)| OpsPostTarget {
            root_post_id,
            post_entity_id,
            author_user_id,
        })
    })
}

pub async fn file_emergency_safety(
    pool: &MySqlPool,
    operator_user_id: u64,
    operator_entity_id: u64,
    post_id: u64,
    clause_id: u64,
    ops_note: &str,
) -> Result<u64, FilingError> {
    let Some(target) = load_post_target(pool, post_id).await? else {
        return Err(FilingError::PostNotFound);
    };
    let mut tx = pool.begin().await?;
    let case_id = file_emergency_safety_tx(
        &mut tx,
        operator_user_id,
        operator_entity_id,
        &target,
        clause_id,
        ops_note,
    )
    .await?;
    tx.commit().await?;
    Ok(case_id)
}

pub async fn file_civic_dispute(
    pool: &MySqlPool,
    operator_user_id: u64,
    operator_entity_id: u64,
    post_id: u64,
    clause_ids: &[u64],
    ops_note: &str,
) -> Result<u64, FilingError> {
    if clause_ids.is_empty() {
        return Err(FilingError::InvalidClause);
    }
    let Some(target) = load_post_target(pool, post_id).await? else {
        return Err(FilingError::PostNotFound);
    };
    let mut tx = pool.begin().await?;
    let case_id = file_civic_dispute_tx(
        &mut tx,
        operator_user_id,
        operator_entity_id,
        &target,
        clause_ids,
        ops_note,
    )
    .await?;
    tx.commit().await?;
    Ok(case_id)
}

async fn file_emergency_safety_tx(
    tx: &mut Transaction<'_, MySql>,
    operator_user_id: u64,
    operator_entity_id: u64,
    target: &OpsPostTarget,
    clause_id: u64,
    ops_note: &str,
) -> Result<u64, FilingError> {
    let points = match judicial::classify_clause_ids_tx(tx, std::slice::from_ref(&clause_id)).await {
        Ok(p) => p,
        Err(EmergencyFilingError::MixedClauseKinds) => return Err(FilingError::MixedClauseKinds),
        Err(_) => return Err(FilingError::InvalidClause),
    };
    if !points.iter().all(|p| judicial::is_emergency_safety_point(*p)) {
        return Err(FilingError::InvalidClause);
    }
    match crate::social::safety::route_filing_for_clause_points(&points) {
        Ok(crate::social::safety::FilingRoute::EmergencySafety) => {}
        Ok(crate::social::safety::FilingRoute::SanitationOnly(_)) => {
            return Err(FilingError::SanitationClause);
        }
        Ok(crate::social::safety::FilingRoute::CivicDispute) => {
            return Err(FilingError::InvalidClause);
        }
        Err(_) => return Err(FilingError::InvalidClause),
    }
    if judicial::active_emergency_case_on_post_tx(tx, target.root_post_id)
        .await
        .map_err(|_| FilingError::Db)?
    {
        return Err(FilingError::DuplicateEmergency);
    }

    let trigger_type_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_JUDICIAL_CASE_TRIGGER,
        ref_keys::KEY_JUDICIAL_TRIGGER_EMERGENCY_SAFETY,
    )
    .await?
    .ok_or(FilingError::Db)?;
    let case_status_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_CONTAINED,
    )
    .await?
    .ok_or(FilingError::Db)?;

    let originator_rating: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(se.rating, 0) FROM social_entity se WHERE se.id = ? LIMIT 1"#,
    )
    .bind(operator_entity_id)
    .fetch_one(&mut **tx)
    .await?;

    let jc_res = sqlx::query(
        r#"
        INSERT INTO judicial_case (
            originator_social_entity_id, target_social_entity_id,
            trigger_type_id, status_id, subject_post_id, jury_size,
            jury_votes_for, jury_votes_against, ai_votes_for, ai_votes_against,
            created_by, modified_by
        )
        VALUES (?, ?, ?, ?, ?, 0, 0, 0, 0, 0, ?, ?)
        "#,
    )
    .bind(operator_entity_id)
    .bind(target.post_entity_id)
    .bind(trigger_type_id)
    .bind(case_status_id)
    .bind(target.root_post_id)
    .bind(operator_user_id)
    .bind(operator_user_id)
    .execute(&mut **tx)
    .await?;
    let case_id = jc_res.last_insert_id();

    sqlx::query(
        r#"
        INSERT INTO judicial_case_constitution_item (judicial_case_id, constitution_item_id, created_by)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(case_id)
    .bind(clause_id)
    .bind(operator_user_id)
    .execute(&mut **tx)
    .await?;

    insert_originator_comment_tx(
        tx,
        case_id,
        operator_user_id,
        operator_entity_id,
        originator_rating,
        ops_note,
    )
    .await?;

    judicial::suppress_post_thread_tx(tx, target.root_post_id, operator_user_id).await?;

    insert_action_tx(
        tx,
        operator_user_id,
        ActionType::EmergencyFile,
        Some(target.root_post_id),
        target.author_user_id,
        None,
        Some(case_id),
        Some("panoply"),
        ops_note,
    )
    .await?;

    Ok(case_id)
}

async fn file_civic_dispute_tx(
    tx: &mut Transaction<'_, MySql>,
    operator_user_id: u64,
    operator_entity_id: u64,
    target: &OpsPostTarget,
    clause_ids: &[u64],
    ops_note: &str,
) -> Result<u64, FilingError> {
    let points = match judicial::classify_clause_ids_tx(tx, clause_ids).await {
        Ok(p) => p,
        Err(EmergencyFilingError::MixedClauseKinds) => return Err(FilingError::MixedClauseKinds),
        Err(_) => return Err(FilingError::InvalidClause),
    };
    match crate::social::safety::route_filing_for_clause_points(&points) {
        Ok(crate::social::safety::FilingRoute::CivicDispute) => {}
        Ok(crate::social::safety::FilingRoute::EmergencySafety) => {
            return Err(FilingError::InvalidClause);
        }
        Ok(crate::social::safety::FilingRoute::SanitationOnly(_)) => {
            return Err(FilingError::SanitationClause);
        }
        Err(_) => return Err(FilingError::MixedClauseKinds),
    }

    let trigger_type_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_JUDICIAL_CASE_TRIGGER,
        ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE,
    )
    .await?
    .ok_or(FilingError::Db)?;
    let case_status_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_OPEN,
    )
    .await?
    .ok_or(FilingError::Db)?;

    let jury_size_frozen = judicial::compute_jury_size_to_freeze_at_filing_tx(
        tx,
        operator_entity_id,
        target.post_entity_id,
        Some(operator_user_id),
        Some(target.root_post_id),
        ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE,
    )
    .await
    .map_err(|_| FilingError::Db)?;

    let originator_rating: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(se.rating, 0) FROM social_entity se WHERE se.id = ? LIMIT 1"#,
    )
    .bind(operator_entity_id)
    .fetch_one(&mut **tx)
    .await?;

    let jc_res = sqlx::query(
        r#"
        INSERT INTO judicial_case (
            originator_social_entity_id, target_social_entity_id,
            trigger_type_id, status_id, subject_post_id, jury_size,
            jury_votes_for, jury_votes_against, ai_votes_for, ai_votes_against,
            created_by, modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, 0, 0, 0, 0, ?, ?)
        "#,
    )
    .bind(operator_entity_id)
    .bind(target.post_entity_id)
    .bind(trigger_type_id)
    .bind(case_status_id)
    .bind(target.root_post_id)
    .bind(jury_size_frozen)
    .bind(operator_user_id)
    .bind(operator_user_id)
    .execute(&mut **tx)
    .await?;
    let case_id = jc_res.last_insert_id();

    for clause_id in clause_ids {
        sqlx::query(
            r#"
            INSERT INTO judicial_case_constitution_item (judicial_case_id, constitution_item_id, created_by)
            VALUES (?, ?, ?)
            "#,
        )
        .bind(case_id)
        .bind(clause_id)
        .bind(operator_user_id)
        .execute(&mut **tx)
        .await?;
    }

    insert_originator_comment_tx(
        tx,
        case_id,
        operator_user_id,
        operator_entity_id,
        originator_rating,
        ops_note,
    )
    .await?;

    judicial::set_subject_post_under_judicial_review_tx(tx, target.root_post_id, operator_user_id)
        .await
        .map_err(|_| FilingError::Db)?;

    insert_action_tx(
        tx,
        operator_user_id,
        ActionType::CivicFile,
        Some(target.root_post_id),
        target.author_user_id,
        None,
        Some(case_id),
        Some("panoply"),
        ops_note,
    )
    .await?;

    Ok(case_id)
}

async fn insert_originator_comment_tx(
    tx: &mut Transaction<'_, MySql>,
    case_id: u64,
    operator_user_id: u64,
    operator_entity_id: u64,
    originator_rating: Decimal,
    ops_note: &str,
) -> Result<(), sqlx::Error> {
    let type_originator_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_JUDICIAL_CASE_COMMENT_TYPE,
        ref_keys::KEY_JCC_ORIGINATOR,
    )
    .await?
    .expect("originator comment type");
    let plain = ops_note.trim();
    let markup = format!("<p>[Panoply ops] {plain}</p>");
    let sanitized = crate::content::sanitize::sanitize_post_markup(&markup);
    sqlx::query(
        r#"
        INSERT INTO judicial_case_comment (
            judicial_case_id, type_id, body_plain, body_markup, body_html_sanitized,
            author_user_id, author_social_entity_id, author_entity_rating_at_create,
            in_reply_to_comment_id, created_by, modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
        "#,
    )
    .bind(case_id)
    .bind(type_originator_id)
    .bind(plain)
    .bind(&markup)
    .bind(sanitized.as_str())
    .bind(operator_user_id)
    .bind(operator_entity_id)
    .bind(originator_rating)
    .bind(operator_user_id)
    .bind(operator_user_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
