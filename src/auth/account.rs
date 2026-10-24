//! Signed-in account security: password verification and updates.

use sqlx::MySqlPool;

use crate::error::{AppError, AppResult};
use crate::i18n;

use super::password::{self, PasswordError, MIN_PASSWORD_LEN};

pub async fn verify_user_password(
    pool: &MySqlPool,
    user_id: u64,
    plain: &str,
) -> AppResult<bool> {
    let hash_bytes: Option<Vec<u8>> =
        sqlx::query_scalar(r#"SELECT password_hash FROM users WHERE id = ?"#)
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .map_err(AppError::Db)?;
    let Some(hash_bytes) = hash_bytes.filter(|h| !h.is_empty()) else {
        return Ok(false);
    };
    let phc = String::from_utf8_lossy(&hash_bytes);
    password::verify_password(plain, phc.as_ref())
        .map_err(|_| AppError::Unauthorized)
}

pub async fn change_user_password(
    pool: &MySqlPool,
    user_id: u64,
    current_password: &str,
    new_password: &str,
) -> AppResult<()> {
    let ok = verify_user_password(pool, user_id, current_password).await?;
    if !ok {
        return Err(AppError::Unauthorized);
    }
    set_user_password(pool, user_id, new_password).await
}

/// Sets a new password hash; rejects when `new_password` matches the current hash.
pub async fn set_user_password(
    pool: &MySqlPool,
    user_id: u64,
    new_password: &str,
) -> AppResult<()> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();

    if verify_user_password(pool, user_id, new_password).await? {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.account_password_same_as_current"),
        ));
    }

    let hash = password::hash_password(new_password).map_err(|e| match e {
        PasswordError::TooShort => {
            let min = MIN_PASSWORD_LEN.to_string();
            AppError::BadRequest(c.tf(&loc, "auth.password_too_short", &[("min_len", &min)]))
        }
        _ => AppError::BadRequest(c.t(&loc, "auth.invalid_password")),
    })?;

    sqlx::query(
        r#"
        UPDATE users
        SET password_hash = ?, modified_by = ?
        WHERE id = ?
        "#,
    )
    .bind(hash.as_bytes())
    .bind(user_id)
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(AppError::Db)?;

    Ok(())
}
