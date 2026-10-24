//! **Initiators** for recruitment mail — permission checks + entity resolution, then `email::categories::recruitment`.
//!
//! Any caller (Search/Add UI, profit bulk upload, AI worker, system job) should use these entry points
//! rather than opening SMTP or locale keys directly.

use std::collections::HashSet;

use sqlx::MySqlPool;

use crate::auth::email_address;
use crate::config::Config;
use crate::email::categories::recruitment::{BulkInviteSummary, send_platform_invite, send_platform_invites_bulk};
use crate::email::initiator::{resolve_social_entity, ResolvedInitiator};
use crate::error::{AppError, AppResult};
use crate::i18n;
use crate::ref_keys;
use crate::social::blocks;
use crate::social::entity_profile_edit;
use crate::social::human_onboarding;
use crate::social::recruitment_outreach::{self, PlatformRecruitmentPrecheck};
use crate::status;

/// Max addresses per bulk request (client list upload).
pub const MAX_BULK_RECRUITMENT_INVITES_PER_REQUEST: usize = 64;

#[derive(Debug, Clone)]
pub enum FriendInvitePrecheck {
    /// Email is not registered — safe to send a platform invite.
    MaySend,
    /// Verified member visible to the inviter (not blocked).
    AlreadyOnPlatform { handle: String },
    /// Address is registered but we do not reveal account details (blocked, inactive, etc.).
    ExistingAccountUnavailable,
}

/// Loose address check for beta — tighten before production.
#[must_use]
pub fn parse_recipient_email(raw: &str) -> Option<String> {
    let email = raw.trim();
    if email.len() < 3 || email.len() > 254 || !email.contains('@') {
        return None;
    }
    Some(email.to_string())
}

/// Parse a recipient list from explicit `emails` plus free-text (comma / newline / semicolon separated).
#[must_use]
pub fn parse_recipient_list(emails: &[String], emails_text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut push_token = |raw: &str| {
        if let Some(email) = parse_recipient_email(raw) {
            let key = email.to_ascii_lowercase();
            if seen.insert(key) {
                out.push(email);
            }
        }
    };
    for e in emails {
        push_token(e);
    }
    for token in emails_text.split(|c: char| c == ',' || c == ';' || c == '\n' || c == '\r') {
        push_token(token);
    }
    if out.len() > MAX_BULK_RECRUITMENT_INVITES_PER_REQUEST {
        out.truncate(MAX_BULK_RECRUITMENT_INVITES_PER_REQUEST);
    }
    out
}

async fn require_acting_user_may_send_as(
    pool: &MySqlPool,
    acting_user_id: u64,
    initiator_entity_id: u64,
) -> AppResult<ResolvedInitiator> {
    let initiator = resolve_social_entity(pool, initiator_entity_id).await?;
    if entity_profile_edit::user_can_manage_entity(pool, acting_user_id, initiator_entity_id)
        .await
        .map_err(AppError::Db)?
    {
        return Ok(initiator);
    }
    if let Some(primary) = human_onboarding::primary_human_entity_id(pool, acting_user_id).await? {
        if primary == initiator_entity_id {
            return Ok(initiator);
        }
    }
    Err(AppError::NotFound)
}

/// Resolve whether a friend-invite email may receive outbound recruitment mail.
pub async fn precheck_friend_invite_email(
    pool: &MySqlPool,
    acting_user_id: u64,
    to_email: &str,
) -> AppResult<FriendInvitePrecheck> {
    let email = parse_recipient_email(to_email).ok_or_else(|| {
        AppError::BadRequest(i18n::catalog().t(&i18n::request_locale(), "ui.home.friend_invite_invalid_email"))
    })?;
    let canonical = email_address::canonical(&email);
    if canonical.is_empty() {
        return Ok(FriendInvitePrecheck::MaySend);
    }

    let row: Option<(u64, String)> = sqlx::query_as(
        r#"
        SELECT u.id, rk.`key`
        FROM users u
        INNER JOIN ref_key_values rk
            ON rk.id = u.status_id
           AND rk.`group` = ?
           AND rk.is_active = 1
        WHERE u.email_canonical = ?
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_STATUS)
    .bind(&canonical)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Db)?;

    let Some((existing_user_id, status_key)) = row else {
        if recruitment_outreach::outreach_exists(pool, &canonical).await? {
            return Ok(FriendInvitePrecheck::ExistingAccountUnavailable);
        }
        return Ok(FriendInvitePrecheck::MaySend);
    };

    if status_key == ref_keys::KEY_TO_BE_DELETED || status_key == ref_keys::KEY_BLOCKED {
        return Ok(FriendInvitePrecheck::ExistingAccountUnavailable);
    }

    if !status::user_has_verified_status(pool, existing_user_id)
        .await
        .map_err(AppError::Db)?
    {
        return Ok(FriendInvitePrecheck::ExistingAccountUnavailable);
    }

    let Some(handle) = human_onboarding::human_entity_handle_for_user(pool, existing_user_id)
        .await
        .map_err(AppError::Db)?
    else {
        return Ok(FriendInvitePrecheck::ExistingAccountUnavailable);
    };

    let Some(viewer_human_id) = human_onboarding::primary_human_entity_id(pool, acting_user_id)
        .await
        .map_err(AppError::Db)?
    else {
        return Ok(FriendInvitePrecheck::ExistingAccountUnavailable);
    };

    let Some(target_human_id) = human_onboarding::primary_human_entity_id(pool, existing_user_id)
        .await
        .map_err(AppError::Db)?
    else {
        return Ok(FriendInvitePrecheck::ExistingAccountUnavailable);
    };

    if blocks::is_pair_blocked(pool, viewer_human_id, target_human_id)
        .await
        .map_err(AppError::Db)?
    {
        return Ok(FriendInvitePrecheck::ExistingAccountUnavailable);
    }

    Ok(FriendInvitePrecheck::AlreadyOnPlatform { handle })
}

/// Send one platform invite as `initiator_entity_id` (human, profit, AI digital entity, etc.).
pub async fn invite_to_platform_as_entity(
    pool: &MySqlPool,
    config: &Config,
    acting_user_id: u64,
    initiator_entity_id: u64,
    to_email: &str,
) -> AppResult<()> {
    invite_to_platform_as_entity_with_note(pool, config, acting_user_id, initiator_entity_id, to_email, None).await
}

pub async fn invite_to_platform_as_entity_with_note(
    pool: &MySqlPool,
    config: &Config,
    acting_user_id: u64,
    initiator_entity_id: u64,
    to_email: &str,
    personal_note: Option<&str>,
) -> AppResult<()> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let email = parse_recipient_email(to_email).ok_or_else(|| {
        AppError::BadRequest(c.t(&loc, "ui.home.friend_invite_invalid_email"))
    })?;
    match precheck_friend_invite_email(pool, acting_user_id, &email).await? {
        FriendInvitePrecheck::MaySend => {}
        FriendInvitePrecheck::AlreadyOnPlatform { .. } | FriendInvitePrecheck::ExistingAccountUnavailable => {
            return Err(AppError::BadRequest(
                c.t(&loc, "ui.home.friend_invite_email_registered"),
            ));
        }
    }
    let initiator =
        require_acting_user_may_send_as(pool, acting_user_id, initiator_entity_id).await?;

    let mut tx = pool.begin().await.map_err(AppError::Db)?;
    if !recruitment_outreach::claim_outreach_tx(
        &mut tx,
        &email,
        initiator_entity_id,
        None,
        None,
        None,
        None,
        "direct",
    )
    .await
    .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.home.friend_invite_email_registered"),
        ));
    }
    tx.commit().await.map_err(AppError::Db)?;

    send_platform_invite(config, &initiator, &email, personal_note)
        .await
        .map_err(|e| {
            if e == "smtp not configured" {
                AppError::BadRequest(c.t(&loc, "ui.home.friend_invite_smtp_unconfigured"))
            } else {
                AppError::BadRequest(e)
            }
        })
}

/// Convenience: acting human invites using their primary human entity as initiator.
pub async fn invite_to_platform_as_viewer_human(
    pool: &MySqlPool,
    config: &Config,
    acting_user_id: u64,
    to_email: &str,
    personal_note: Option<&str>,
) -> AppResult<()> {
    let entity_id = human_onboarding::primary_human_entity_id(pool, acting_user_id)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::BadRequest(
            "Complete your human profile before sending invitations.".to_string(),
        ))?;
    invite_to_platform_as_entity_with_note(pool, config, acting_user_id, entity_id, to_email, personal_note).await
}

/// Bulk client-list style recruitment (e.g. profit entity owner inviting their base).
pub async fn invite_to_platform_bulk_as_entity(
    pool: &MySqlPool,
    config: &Config,
    acting_user_id: u64,
    initiator_entity_id: u64,
    raw_emails: &[String],
) -> AppResult<BulkInviteSummary> {
    invite_to_platform_bulk_parsed(pool, config, acting_user_id, initiator_entity_id, raw_emails, "", None)
        .await
}

/// Bulk recruitment with explicit list + optional pasted text (profit / non-profit client lists).
pub async fn invite_to_platform_bulk_parsed(
    pool: &MySqlPool,
    config: &Config,
    acting_user_id: u64,
    initiator_entity_id: u64,
    emails: &[String],
    emails_text: &str,
    personal_note: Option<&str>,
) -> AppResult<BulkInviteSummary> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let parsed = parse_recipient_list(emails, emails_text);
    if parsed.is_empty() {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.home.friend_invite_invalid_email"),
        ));
    }
    let initiator =
        require_acting_user_may_send_as(pool, acting_user_id, initiator_entity_id).await?;
    if config.smtp.is_none() {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.home.friend_invite_smtp_unconfigured"),
        ));
    }
    let mut sendable = Vec::new();
    for email in parsed {
        if !matches!(
            recruitment_outreach::precheck_platform_recruitment_email(pool, &email).await?,
            PlatformRecruitmentPrecheck::MaySend
        ) {
            continue;
        }
        let mut tx = pool.begin().await.map_err(AppError::Db)?;
        if recruitment_outreach::claim_outreach_tx(
            &mut tx,
            &email,
            initiator_entity_id,
            None,
            None,
            None,
            None,
            "direct",
        )
        .await
        .map_err(AppError::Db)?
        {
            tx.commit().await.map_err(AppError::Db)?;
            sendable.push(email);
        } else {
            tx.rollback().await.ok();
        }
    }
    Ok(send_platform_invites_bulk(config, &initiator, &sendable, personal_note).await)
}

/// Resolve entity id from handle then send bulk recruitment mail.
pub async fn invite_to_platform_bulk_by_handle(
    pool: &MySqlPool,
    config: &Config,
    acting_user_id: u64,
    handle: &str,
    emails: &[String],
    emails_text: &str,
    personal_note: Option<&str>,
) -> AppResult<BulkInviteSummary> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(normalized) = crate::social::entity::normalize_handle_from_path_segment(handle.trim())
    else {
        return Err(AppError::BadRequest(
            c.t(&loc, "social_entity.handle_invalid"),
        ));
    };
    let entity_id: u64 = sqlx::query_scalar(
        "SELECT id FROM social_entity WHERE handle = ? LIMIT 1",
    )
    .bind(&normalized)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Db)?
    .ok_or(AppError::NotFound)?;
    invite_to_platform_bulk_parsed(
        pool,
        config,
        acting_user_id,
        entity_id,
        emails,
        emails_text,
        personal_note,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_recipient_list_dedupes_and_splits() {
        let list = parse_recipient_list(
            &["A@example.com".to_string()],
            "b@example.com, a@example.com\n c@example.com;",
        );
        assert_eq!(list.len(), 3);
        assert!(list.iter().any(|e| e.eq_ignore_ascii_case("a@example.com")));
        assert!(list.iter().any(|e| e.eq_ignore_ascii_case("b@example.com")));
        assert!(list.iter().any(|e| e.eq_ignore_ascii_case("c@example.com")));
    }

    #[test]
    fn parse_recipient_list_caps_at_max() {
        let many: Vec<String> = (0..80)
            .map(|i| format!("user{i}@example.com"))
            .collect();
        let list = parse_recipient_list(&many, "");
        assert_eq!(list.len(), MAX_BULK_RECRUITMENT_INVITES_PER_REQUEST);
    }
}
