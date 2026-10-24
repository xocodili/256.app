use sqlx::MySqlPool;

use crate::db::TimestampUtc;
use crate::status;

use super::account_tokens::{self, AccountTokenKind};

/// Create a new token, replacing any previous tokens for this user. Returns hex-encoded secret for the URL.
pub async fn issue_verification_token(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<String, sqlx::Error> {
    account_tokens::issue_account_token(pool, user_id, AccountTokenKind::EmailVerification).await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyOutcome {
    /// Email was just verified for the first time.
    Verified(u64),
    /// Email was already verified; token consumed when present.
    AlreadyVerified(u64),
    Invalid,
}

pub async fn consume_verification_token(
    pool: &MySqlPool,
    token_hex: &str,
) -> Result<VerifyOutcome, sqlx::Error> {
    let Some((user_id, _)) =
        account_tokens::lookup_account_token(pool, token_hex, AccountTokenKind::EmailVerification)
            .await?
    else {
        return Ok(VerifyOutcome::Invalid);
    };

    let email_verified_at: Option<TimestampUtc> =
        sqlx::query_scalar::<_, Option<TimestampUtc>>(
            r#"SELECT email_verified_at FROM users WHERE id = ?"#,
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .flatten();

    if email_verified_at.is_some() {
        account_tokens::delete_account_tokens_for_user(
            pool,
            user_id,
            AccountTokenKind::EmailVerification,
        )
        .await?;
        return Ok(VerifyOutcome::AlreadyVerified(user_id));
    }

    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"
        UPDATE users
        SET email_verified_at = UTC_TIMESTAMP()
        WHERE id = ? AND email_verified_at IS NULL
        "#,
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM email_verification_tokens WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    status::after_user_email_verified(pool, user_id).await;

    Ok(VerifyOutcome::Verified(user_id))
}
