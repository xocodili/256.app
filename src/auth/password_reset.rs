use sqlx::MySqlPool;

use crate::db::TimestampUtc;
use crate::ref_keys;

use super::account_tokens::{self, AccountTokenKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetConsumeOutcome {
    Valid(u64),
    Invalid,
}

/// Create a new reset token, replacing any previous tokens for this user. Returns hex secret for the URL.
pub async fn issue_password_reset_token(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<String, sqlx::Error> {
    account_tokens::issue_account_token(pool, user_id, AccountTokenKind::PasswordReset).await
}

/// Validates and consumes a one-time reset token for an eligible account.
pub async fn consume_password_reset_token(
    pool: &MySqlPool,
    token_hex: &str,
) -> Result<ResetConsumeOutcome, sqlx::Error> {
    let row: Option<(u64, Option<Vec<u8>>, Option<TimestampUtc>, Option<String>)> =
        match account_tokens::lookup_account_token(pool, token_hex, AccountTokenKind::PasswordReset)
            .await?
        {
            Some((user_id, _)) => {
                sqlx::query_as(
                    r#"
                    SELECT u.id, u.password_hash, u.email_verified_at, rk.`key`
                    FROM users u
                    LEFT JOIN ref_key_values rk ON rk.id = COALESCE(u.user_status_id, u.status_id)
                    WHERE u.id = ?
                    "#,
                )
                .bind(user_id)
                .fetch_optional(pool)
                .await?
            }
            None => None,
        };

    let Some((user_id, hash_bytes, verified_at, user_status_key)) = row else {
        return Ok(ResetConsumeOutcome::Invalid);
    };

    if !user_is_eligible_for_password_reset(
        hash_bytes.as_deref(),
        verified_at,
        user_status_key.as_deref(),
    ) {
        account_tokens::delete_account_tokens_for_user(
            pool,
            user_id,
            AccountTokenKind::PasswordReset,
        )
        .await?;
        return Ok(ResetConsumeOutcome::Invalid);
    }

    account_tokens::delete_account_tokens_for_user(pool, user_id, AccountTokenKind::PasswordReset)
        .await?;

    Ok(ResetConsumeOutcome::Valid(user_id))
}

#[must_use]
pub fn user_is_eligible_for_password_reset(
    password_hash: Option<&[u8]>,
    email_verified_at: Option<TimestampUtc>,
    user_status_key: Option<&str>,
) -> bool {
    let Some(_) = password_hash.filter(|h| !h.is_empty()) else {
        return false;
    };
    if email_verified_at.is_none() {
        return false;
    }
    !matches!(
        user_status_key,
        Some(ref_keys::KEY_TO_BE_DELETED) | Some(ref_keys::KEY_BLOCKED)
    )
}

/// Lookup for forgot-password email dispatch (anti-enumeration: caller always returns 202).
///
/// `email_canonical` is the canonical uniqueness key (see [`crate::auth::email_address::canonical`]);
/// the returned address is the account's deliverable `users.email`.
pub async fn lookup_eligible_password_reset_user(
    pool: &MySqlPool,
    email_canonical: &str,
) -> Result<Option<(u64, String)>, sqlx::Error> {
    let row: Option<(u64, Option<Vec<u8>>, Option<TimestampUtc>, Option<String>, String)> =
        sqlx::query_as(
            r#"
            SELECT u.id, u.password_hash, u.email_verified_at, rk.`key`, u.email
            FROM users u
            LEFT JOIN ref_key_values rk ON rk.id = COALESCE(u.user_status_id, u.status_id)
            WHERE u.email_canonical = ?
            "#,
        )
        .bind(email_canonical)
        .fetch_optional(pool)
        .await?;

    let Some((user_id, hash_bytes, verified_at, user_status_key, email)) = row else {
        return Ok(None);
    };

    if user_is_eligible_for_password_reset(
        hash_bytes.as_deref(),
        verified_at,
        user_status_key.as_deref(),
    ) {
        Ok(Some((user_id, email)))
    } else {
        Ok(None)
    }
}
