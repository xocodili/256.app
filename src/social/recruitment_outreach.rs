//! Global once-only gate for platform recruitment emails (council AI + human initiators).

use sqlx::{MySql, MySqlPool, Transaction};

use crate::auth::email_address;
use crate::social::recruitment::parse_recipient_email;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformRecruitmentPrecheck {
    MaySend,
    AlreadyOnPlatform,
    AlreadyInvited,
    InvalidEmail,
}

/// Whether an address may receive a first-time platform recruitment email.
pub async fn precheck_platform_recruitment_email(
    pool: &MySqlPool,
    raw_email: &str,
) -> Result<PlatformRecruitmentPrecheck, sqlx::Error> {
    let Some(email) = parse_recipient_email(raw_email) else {
        return Ok(PlatformRecruitmentPrecheck::InvalidEmail);
    };
    let canonical = email_address::canonical(&email);
    if canonical.is_empty() {
        return Ok(PlatformRecruitmentPrecheck::InvalidEmail);
    }

    let on_platform: Option<u64> = sqlx::query_scalar(
        r#"SELECT id FROM users WHERE email_canonical = ? LIMIT 1"#,
    )
    .bind(&canonical)
    .fetch_optional(pool)
    .await?;
    if on_platform.is_some() {
        return Ok(PlatformRecruitmentPrecheck::AlreadyOnPlatform);
    }

    if outreach_exists(pool, &canonical).await? {
        return Ok(PlatformRecruitmentPrecheck::AlreadyInvited);
    }

    Ok(PlatformRecruitmentPrecheck::MaySend)
}

pub async fn outreach_exists(pool: &MySqlPool, email_canonical: &str) -> Result<bool, sqlx::Error> {
    let n: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM platform_recruitment_outreach WHERE email_canonical = ?"#,
    )
    .bind(email_canonical)
    .fetch_one(pool)
    .await?;
    Ok(n > 0)
}

/// Claim a global outreach slot. Returns false when the email was already invited.
pub async fn claim_outreach_tx(
    tx: &mut Transaction<'_, MySql>,
    email: &str,
    invited_by_social_entity_id: u64,
    invited_by_ai_actor_id: Option<u64>,
    source_post_id: Option<u64>,
    source_content_link_id: Option<u64>,
    intended_label: Option<&str>,
    delivery_route: &str,
) -> Result<bool, sqlx::Error> {
    let canonical = email_address::canonical(email);
    if canonical.is_empty() {
        return Ok(false);
    }
    let res = sqlx::query(
        r#"
        INSERT INTO platform_recruitment_outreach (
            email_canonical,
            invited_by_social_entity_id,
            invited_by_ai_actor_id,
            source_post_id,
            source_content_link_id,
            intended_label,
            delivery_route
        )
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&canonical)
    .bind(invited_by_social_entity_id)
    .bind(invited_by_ai_actor_id)
    .bind(source_post_id)
    .bind(source_content_link_id)
    .bind(intended_label.map(str::trim).filter(|s| !s.is_empty()))
    .bind(delivery_route)
    .execute(&mut **tx)
    .await?;
    Ok(res.rows_affected() > 0)
}
