//! Central rules for moving rows to a new [`ref_keys::GROUP_STATUS`] target.
//!
//! - **Users**: `pending_verification` → `verified` only via [`try_promote_user_to_verified_if_composite`]
//!   (not the generic [`transition_user`]), so email alone does not imply composite verified until
//!   [`user_meets_composite_verification`] is extended.
//! - **Entities / relationships**: `pending_verification` → `verified` is allowed through [`transition_social_entity`]
//!   / [`transition_relationship`] for judicial / policy-style flows.

use sqlx::MySqlPool;

use crate::db::TimestampUtc;
use crate::error::{AppError, AppResult};
use crate::i18n;
use crate::ref_key_values;
use crate::ref_keys;
use crate::social::scope;

fn norm(s: &str) -> &str {
    s.trim()
}

async fn resolve_status_id_in_group(pool: &MySqlPool, group: &str, to_key: &str) -> AppResult<u64> {
    let loc = i18n::request_locale();
    let k = norm(to_key);
    ref_key_values::lookup_active_id(pool, group, k)
        .await
        .map_err(AppError::Db)?
        .ok_or_else(|| {
            AppError::BadRequest(i18n::catalog().t(&loc, "status.unknown_status_target"))
        })
}

async fn user_status_key(pool: &MySqlPool, user_id: u64) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT r.`key`
        FROM users u
        INNER JOIN ref_key_values r ON r.id = COALESCE(u.user_status_id, u.status_id)
        WHERE u.id = ? AND r.`group` IN (?, ?)
        ORDER BY (r.`group` = ?) DESC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(ref_keys::GROUP_USER_STATUS)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::GROUP_USER_STATUS)
    .fetch_optional(pool)
    .await
}

async fn entity_status_key(
    pool: &MySqlPool,
    entity_id: u64,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT r.`key`
        FROM social_entity e
        INNER JOIN ref_key_values r ON r.id = COALESCE(e.entity_status_id, e.status_id)
        WHERE e.id = ? AND r.`group` IN (?, ?)
        ORDER BY (r.`group` = ?) DESC
        LIMIT 1
        "#,
    )
    .bind(entity_id)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
    .fetch_optional(pool)
    .await
}

async fn relationship_status_key(
    pool: &MySqlPool,
    relationship_id: u64,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT r.`key`
        FROM social_entity_relationship x
        INNER JOIN ref_key_values r ON r.id = COALESCE(x.relationship_status_id, x.status_id)
        WHERE x.id = ? AND r.`group` IN (?, ?)
        ORDER BY (r.`group` = ?) DESC
        LIMIT 1
        "#,
    )
    .bind(relationship_id)
    .bind(ref_keys::GROUP_SOCIAL_RELATIONSHIP_STATUS)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::GROUP_SOCIAL_RELATIONSHIP_STATUS)
    .fetch_optional(pool)
    .await
}

/// `pending_verification` → `verified` is **not** listed here; use [`try_promote_user_to_verified_if_composite`].
fn user_transition_allowed(from: &str, to: &str) -> bool {
    if from == to {
        return true;
    }
    matches!(
        (from, to),
        (
            ref_keys::KEY_PENDING_VERIFICATION,
            ref_keys::KEY_BLOCKED | ref_keys::KEY_RETIRED | ref_keys::KEY_TO_BE_DELETED
        ) | (
            ref_keys::KEY_VERIFIED,
            ref_keys::KEY_BLOCKED | ref_keys::KEY_RETIRED | ref_keys::KEY_TO_BE_DELETED
        ) | (
            ref_keys::KEY_BLOCKED,
            ref_keys::KEY_PENDING_VERIFICATION | ref_keys::KEY_TO_BE_DELETED
        ) | (ref_keys::KEY_RETIRED, ref_keys::KEY_TO_BE_DELETED)
            | (
                ref_keys::KEY_TO_BE_DELETED,
                ref_keys::KEY_PENDING_VERIFICATION
            )
    )
}

fn entity_like_transition_allowed(from: &str, to: &str) -> bool {
    if from == to {
        return true;
    }
    matches!(
        (from, to),
        (
            ref_keys::KEY_PENDING_VERIFICATION,
            ref_keys::KEY_VERIFIED
                | ref_keys::KEY_BLOCKED
                | ref_keys::KEY_RETIRED
                | ref_keys::KEY_TO_BE_DELETED
        ) | (
            ref_keys::KEY_VERIFIED,
            ref_keys::KEY_BLOCKED | ref_keys::KEY_RETIRED | ref_keys::KEY_TO_BE_DELETED
        ) | (
            ref_keys::KEY_BLOCKED,
            ref_keys::KEY_PENDING_VERIFICATION
                | ref_keys::KEY_VERIFIED
                | ref_keys::KEY_TO_BE_DELETED
        ) | (ref_keys::KEY_RETIRED, ref_keys::KEY_TO_BE_DELETED)
            | (
                ref_keys::KEY_TO_BE_DELETED,
                ref_keys::KEY_PENDING_VERIFICATION
            )
    )
}

fn deny_bad_transition(loc: &str, from: &str, to: &str) -> AppResult<()> {
    Err(AppError::BadRequest(i18n::catalog().tf(
        loc,
        "status.transition_not_allowed",
        &[("from", from), ("to", to)],
    )))
}

/// Move a user to `to_key` if the transition is allowed from their current status.
pub async fn transition_user(
    pool: &MySqlPool,
    user_id: u64,
    to_key: &str,
    modified_by: Option<u64>,
) -> AppResult<()> {
    let loc = i18n::request_locale();
    let to_k = norm(to_key);
    let from_key = user_status_key(pool, user_id)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::NotFound)?;
    if from_key == to_k {
        return Ok(());
    }
    if from_key == ref_keys::KEY_PENDING_VERIFICATION && to_k == ref_keys::KEY_VERIFIED {
        return deny_bad_transition(&loc, &from_key, to_k);
    }
    if !user_transition_allowed(&from_key, to_k) {
        return deny_bad_transition(&loc, &from_key, to_k);
    }
    let sid_legacy = resolve_status_id_in_group(pool, ref_keys::GROUP_STATUS, to_k).await?;
    let sid_scoped = resolve_status_id_in_group(pool, ref_keys::GROUP_USER_STATUS, to_k).await?;
    sqlx::query(
        r#"
        UPDATE users
        SET status_id = ?, user_status_id = ?, modified_by = ?
        WHERE id = ?
        "#,
    )
    .bind(sid_legacy)
    .bind(sid_scoped)
    .bind(modified_by)
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Move a social entity to `to_key` if allowed.
pub async fn transition_social_entity(
    pool: &MySqlPool,
    entity_id: u64,
    to_key: &str,
    modified_by: Option<u64>,
) -> AppResult<()> {
    let loc = i18n::request_locale();
    let to_k = norm(to_key);
    let from_key = entity_status_key(pool, entity_id)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::NotFound)?;
    if from_key == to_k {
        return Ok(());
    }
    if !entity_like_transition_allowed(&from_key, to_k) {
        return deny_bad_transition(&loc, &from_key, to_k);
    }
    let sid_legacy = resolve_status_id_in_group(pool, ref_keys::GROUP_STATUS, to_k).await?;
    let sid_scoped =
        resolve_status_id_in_group(pool, ref_keys::GROUP_SOCIAL_ENTITY_STATUS, to_k).await?;
    sqlx::query(
        r#"
        UPDATE social_entity
        SET status_id = ?, entity_status_id = ?, modified_by = ?
        WHERE id = ?
        "#,
    )
    .bind(sid_legacy)
    .bind(sid_scoped)
    .bind(modified_by)
    .bind(entity_id)
    .execute(pool)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Move a relationship to `to_key` if allowed.
pub async fn transition_relationship(
    pool: &MySqlPool,
    relationship_id: u64,
    to_key: &str,
    modified_by: Option<u64>,
) -> AppResult<()> {
    let loc = i18n::request_locale();
    let to_k = norm(to_key);
    let from_key = relationship_status_key(pool, relationship_id)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::NotFound)?;
    if from_key == to_k {
        return Ok(());
    }
    if !entity_like_transition_allowed(&from_key, to_k) {
        return deny_bad_transition(&loc, &from_key, to_k);
    }
    let sid_legacy = resolve_status_id_in_group(pool, ref_keys::GROUP_STATUS, to_k).await?;
    let sid_scoped =
        resolve_status_id_in_group(pool, ref_keys::GROUP_SOCIAL_RELATIONSHIP_STATUS, to_k).await?;
    sqlx::query(
        r#"
        UPDATE social_entity_relationship
        SET status_id = ?, relationship_status_id = ?, modified_by = ?
        WHERE id = ?
        "#,
    )
    .bind(sid_legacy)
    .bind(sid_scoped)
    .bind(modified_by)
    .bind(relationship_id)
    .execute(pool)
    .await
    .map_err(AppError::Db)?;
    mark_relationship_scope_dirty(pool, relationship_id).await;
    Ok(())
}

/// End a relationship (set `ended_at = COALESCE(ended_at, UTC_DATE())`) and enqueue both endpoints
/// for scope recalc. Used by the home-profile decline / disconnect / leave / unfollow / cancel
/// flows where the row stays around but stops contributing to `vw_entity_read_scope_live`.
///
/// Returns `Ok(true)` when the row existed and was updated (or already ended). Callers that
/// need 404 semantics should look up the relationship first.
pub async fn end_relationship(
    pool: &MySqlPool,
    relationship_id: u64,
    modified_by: u64,
) -> AppResult<bool> {
    let r = sqlx::query(
        r#"
        UPDATE social_entity_relationship
        SET ended_at = COALESCE(ended_at, UTC_DATE()), modified_by = ?
        WHERE id = ?
        "#,
    )
    .bind(modified_by)
    .bind(relationship_id)
    .execute(pool)
    .await
    .map_err(AppError::Db)?;
    let affected = r.rows_affected() > 0;
    if affected {
        mark_relationship_scope_dirty(pool, relationship_id).await;
    }
    Ok(affected)
}

/// Best-effort scope enqueue keyed on a relationship id. Looks up both endpoints and bumps their
/// `scope_version` + queues recalc; failures are logged but never escalate to the caller, on the
/// principle that the periodic `scope_rematerialize_all` job will reconcile any drift.
async fn mark_relationship_scope_dirty(pool: &MySqlPool, relationship_id: u64) {
    let endpoints: Result<Option<(u64, u64)>, sqlx::Error> = sqlx::query_as(
        r#"
        SELECT from_social_entity_id, to_social_entity_id
        FROM social_entity_relationship
        WHERE id = ?
        "#,
    )
    .bind(relationship_id)
    .fetch_optional(pool)
    .await;
    match endpoints {
        Ok(Some((from_id, to_id))) => {
            if let Err(e) =
                scope::mark_relationship_endpoints_dirty(pool, from_id, to_id).await
            {
                tracing::warn!(
                    error = %e,
                    relationship_id,
                    "scope_recalc enqueue failed after relationship transition; periodic rematerialize will reconcile"
                );
            }
        }
        Ok(None) => {}
        Err(e) => {
            tracing::warn!(
                error = %e,
                relationship_id,
                "lookup of relationship endpoints failed when marking scope dirty"
            );
        }
    }
}

/// Composite verification gate for posting rights.
/// Requires:
/// - email verified
/// - a human social profile
/// - a captured face profile (`user_faces` with a `face_captured_at` completion timestamp)
pub async fn user_meets_composite_verification(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<bool, sqlx::Error> {
    let row: Option<(Option<TimestampUtc>, i64, i64, i64)> = sqlx::query_as(
        r#"
        SELECT
            u.email_verified_at,
            (
                SELECT COUNT(*)
                FROM social_human sh
                INNER JOIN social_entity se ON se.id = sh.social_entity_id
                LEFT JOIN user_social_entity_role ur
                    ON ur.social_entity_id = se.id
                   AND ur.user_id = u.id
                   AND ur.ends_at IS NULL
                WHERE (ur.id IS NOT NULL OR se.created_by = u.id)
            ) AS human_profiles,
            (
                SELECT COUNT(*)
                FROM user_faces uf
                WHERE uf.user_id = u.id
                  AND uf.face_captured_at IS NOT NULL
            ) AS face_profiles,
            (
                SELECT COUNT(*)
                FROM social_entity se_h
                INNER JOIN social_human sh ON sh.social_entity_id = se_h.id
                LEFT JOIN user_social_entity_role ur
                    ON ur.social_entity_id = se_h.id
                   AND ur.user_id = u.id
                   AND ur.ends_at IS NULL
                INNER JOIN social_entity_relationship godm ON godm.from_social_entity_id = se_h.id
                INNER JOIN ref_key_values godm_type ON godm_type.id = godm.type_id
                INNER JOIN social_entity ai_se ON ai_se.id = godm.to_social_entity_id
                INNER JOIN social_entity_relationship modr
                    ON modr.from_social_entity_id = ai_se.id
                   AND modr.ended_at IS NULL
                INNER JOIN ref_key_values modr_type ON modr_type.id = modr.type_id
                INNER JOIN social_entity_relationship memb
                    ON memb.from_social_entity_id = se_h.id
                   AND memb.to_social_entity_id = modr.to_social_entity_id
                   AND memb.ended_at IS NULL
                INNER JOIN ref_key_values memb_type ON memb_type.id = memb.type_id
                WHERE (ur.id IS NOT NULL OR se_h.created_by = u.id)
                  AND godm.ended_at IS NULL
                  AND godm_type.`group` = ?
                  AND godm_type.`key` = ?
                  AND modr_type.`group` = ?
                  AND modr_type.`key` = ?
                  AND memb_type.`group` = ?
                  AND memb_type.`key` = ?
            ) AS onboarding_links
        FROM users u
        WHERE u.id = ?
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_GODMOTHER)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MODERATOR)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    let Some((email_at, human_profiles, face_profiles, onboarding_links)) = row else {
        return Ok(false);
    };
    Ok(email_at.is_some() && human_profiles > 0 && face_profiles > 0 && onboarding_links > 0)
}

/// True when the user's lifecycle status is `verified`.
pub async fn user_has_verified_status(pool: &MySqlPool, user_id: u64) -> Result<bool, sqlx::Error> {
    let key: Option<String> = user_status_key(pool, user_id).await?;
    Ok(matches!(key.as_deref(), Some(k) if k == ref_keys::KEY_VERIFIED))
}

/// If [`user_meets_composite_verification`] passes and status is `pending_verification`, set `verified`.
pub async fn try_promote_user_to_verified_if_composite(
    pool: &MySqlPool,
    user_id: u64,
    modified_by: Option<u64>,
) -> AppResult<()> {
    if !user_meets_composite_verification(pool, user_id)
        .await
        .map_err(AppError::Db)?
    {
        return Ok(());
    }
    let from_key = user_status_key(pool, user_id)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::NotFound)?;
    if from_key != ref_keys::KEY_PENDING_VERIFICATION {
        return Ok(());
    }
    let sid =
        resolve_status_id_in_group(pool, ref_keys::GROUP_STATUS, ref_keys::KEY_VERIFIED).await?;
    let sid_scoped =
        resolve_status_id_in_group(pool, ref_keys::GROUP_USER_STATUS, ref_keys::KEY_VERIFIED)
            .await?;
    let basic_tier = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_VERIFICATION_TIER,
        ref_keys::KEY_VERIFICATION_TIER_BASIC,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| {
        let loc = i18n::request_locale();
        AppError::BadRequest(
            i18n::catalog()
                .t(&loc, "ref_keys.default_status_unconfigured")
                .to_string(),
        )
    })?;
    sqlx::query(
        r#"
        UPDATE users
        SET status_id = ?, user_status_id = ?, verification_tier_id = ?, modified_by = ?
        WHERE id = ?
        "#,
    )
    .bind(sid)
    .bind(sid_scoped)
    .bind(basic_tier)
    .bind(modified_by)
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// After email link succeeds: attempt composite promotion (no-op until [`user_meets_composite_verification`] is stricter).
/// Errors are logged; verification itself has already committed.
pub async fn after_user_email_verified(pool: &MySqlPool, user_id: u64) {
    if let Err(e) = try_promote_user_to_verified_if_composite(pool, user_id, None).await {
        tracing::warn!(
            error = %e,
            user_id,
            "status promotion after email verify failed"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_matrix_denies_pending_to_verified() {
        assert!(!user_transition_allowed(
            ref_keys::KEY_PENDING_VERIFICATION,
            ref_keys::KEY_VERIFIED,
        ));
        assert!(user_transition_allowed(
            ref_keys::KEY_PENDING_VERIFICATION,
            ref_keys::KEY_BLOCKED,
        ));
    }

    #[test]
    fn entity_allows_pending_to_verified() {
        assert!(entity_like_transition_allowed(
            ref_keys::KEY_PENDING_VERIFICATION,
            ref_keys::KEY_VERIFIED,
        ));
    }
}
