//! Shared one-time account link tokens (email verification, password reset).

use chrono::{Duration, Utc};
use rand::RngCore;
use sqlx::MySqlPool;

use crate::crypto::sha3;

const TOKEN_BYTES: usize = 32;
const TOKEN_TTL_HOURS: i64 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountTokenKind {
    EmailVerification,
    PasswordReset,
}

impl AccountTokenKind {
    const fn table(self) -> &'static str {
        match self {
            Self::EmailVerification => "email_verification_tokens",
            Self::PasswordReset => "password_reset_tokens",
        }
    }
}

/// Create a new token, replacing any previous row for this user. Returns hex secret for the URL.
pub async fn issue_account_token(
    pool: &MySqlPool,
    user_id: u64,
    kind: AccountTokenKind,
) -> Result<String, sqlx::Error> {
    let mut raw = [0u8; TOKEN_BYTES];
    rand::thread_rng().fill_bytes(&mut raw);
    let token_hex = hex::encode(raw);
    let token_hash = sha3::digest(&raw);
    let expires_at = Utc::now() + Duration::hours(TOKEN_TTL_HOURS);

    let delete_sql = format!("DELETE FROM {} WHERE user_id = ?", kind.table());
    sqlx::query(&delete_sql)
        .bind(user_id)
        .execute(pool)
        .await?;

    let insert_sql = format!(
        "INSERT INTO {} (user_id, token_hash, expires_at) VALUES (?, ?, ?)",
        kind.table()
    );
    sqlx::query(&insert_sql)
        .bind(user_id)
        .bind(token_hash.as_slice())
        .bind(expires_at)
        .execute(pool)
        .await?;

    Ok(token_hex)
}

/// Returns `(user_id, token_hash)` when the secret matches a non-expired row.
pub async fn lookup_account_token(
    pool: &MySqlPool,
    token_hex: &str,
    kind: AccountTokenKind,
) -> Result<Option<(u64, Vec<u8>)>, sqlx::Error> {
    let raw = match hex::decode(token_hex) {
        Ok(b) if b.len() == TOKEN_BYTES => b,
        _ => return Ok(None),
    };
    let token_hash = sha3::digest(raw.as_slice());

    let select_sql = format!(
        r#"
        SELECT user_id, token_hash
        FROM {}
        WHERE token_hash = ? AND expires_at > UTC_TIMESTAMP()
        "#,
        kind.table()
    );
    sqlx::query_as(&select_sql)
        .bind(token_hash.as_slice())
        .fetch_optional(pool)
        .await
}

pub async fn delete_account_tokens_for_user(
    pool: &MySqlPool,
    user_id: u64,
    kind: AccountTokenKind,
) -> Result<(), sqlx::Error> {
    let delete_sql = format!("DELETE FROM {} WHERE user_id = ?", kind.table());
    sqlx::query(&delete_sql)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
