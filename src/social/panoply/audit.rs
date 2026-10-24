//! Panoply operator audit log (`safety_operator_action`).

use chrono::{DateTime, Utc};
use sqlx::{MySql, MySqlPool, Transaction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionType {
    EmergencyFile,
    CivicFile,
    VacateEmergency,
    Eject,
    SessionKill,
    DomainBlock,
}

impl ActionType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EmergencyFile => "emergency_file",
            Self::CivicFile => "civic_file",
            Self::VacateEmergency => "vacate_emergency",
            Self::Eject => "eject",
            Self::SessionKill => "session_kill",
            Self::DomainBlock => "domain_block",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuditRow {
    pub id: u64,
    pub action_type: String,
    pub note: String,
    pub target_post_id: Option<u64>,
    pub target_user_id: Option<u64>,
    pub target_domain: Option<String>,
    pub judicial_case_id: Option<u64>,
    pub created_at: DateTime<Utc>,
}

pub async fn insert_action(
    pool: &MySqlPool,
    actor_user_id: u64,
    action_type: ActionType,
    target_post_id: Option<u64>,
    target_user_id: Option<u64>,
    target_domain: Option<&str>,
    judicial_case_id: Option<u64>,
    reason_code: Option<&str>,
    note: &str,
) -> Result<u64, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let id = insert_action_tx(
        &mut tx,
        actor_user_id,
        action_type,
        target_post_id,
        target_user_id,
        target_domain,
        judicial_case_id,
        reason_code,
        note,
    )
    .await?;
    tx.commit().await?;
    Ok(id)
}

pub async fn insert_action_tx(
    tx: &mut Transaction<'_, MySql>,
    actor_user_id: u64,
    action_type: ActionType,
    target_post_id: Option<u64>,
    target_user_id: Option<u64>,
    target_domain: Option<&str>,
    judicial_case_id: Option<u64>,
    reason_code: Option<&str>,
    note: &str,
) -> Result<u64, sqlx::Error> {
    let res = sqlx::query(
        r#"
        INSERT INTO safety_operator_action (
            actor_user_id, action_type, target_post_id, target_user_id,
            target_domain, judicial_case_id, reason_code, note
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(actor_user_id)
    .bind(action_type.as_str())
    .bind(target_post_id)
    .bind(target_user_id)
    .bind(target_domain)
    .bind(judicial_case_id)
    .bind(reason_code)
    .bind(note)
    .execute(&mut **tx)
    .await?;
    Ok(res.last_insert_id())
}

pub async fn recent_actions(pool: &MySqlPool, limit: u32) -> Result<Vec<AuditRow>, sqlx::Error> {
    let cap = limit.clamp(1, 100);
    let rows = sqlx::query_as::<_, (u64, String, String, Option<u64>, Option<u64>, Option<String>, Option<u64>, DateTime<Utc>)>(
        r#"
        SELECT id, action_type, note, target_post_id, target_user_id, target_domain,
               judicial_case_id, created_at
        FROM safety_operator_action
        ORDER BY created_at DESC
        LIMIT ?
        "#,
    )
    .bind(cap)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, action_type, note, target_post_id, target_user_id, target_domain, judicial_case_id, created_at)| {
                AuditRow {
                    id,
                    action_type,
                    note,
                    target_post_id,
                    target_user_id,
                    target_domain,
                    judicial_case_id,
                    created_at,
                }
            },
        )
        .collect())
}
