//! Shared gates for private profile mutations (full verified status + CSRF).

use axum::http::HeaderMap;

use sqlx::MySqlPool;

use crate::auth::csrf;
use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::status;

pub fn csrf_pepper(config: &Config) -> &str {
    config.csrf_pepper.as_str()
}

pub fn csrf_token_for_request(headers: &HeaderMap, config: &Config) -> Option<String> {
    let session = csrf::session_token_from_headers(headers)?;
    Some(csrf::token_from_session(&session, csrf_pepper(config)))
}

pub async fn require_full_verified_user(pool: &MySqlPool, user_id: u64) -> AppResult<()> {
    let ok = status::user_has_verified_status(pool, user_id)
        .await
        .map_err(AppError::Db)?;
    if ok {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}

pub fn require_csrf(headers: &HeaderMap, config: &Config) -> AppResult<()> {
    if csrf::validate_session(headers, csrf_pepper(config)) {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}

pub async fn require_password_change_mutation(
    pool: &MySqlPool,
    config: &Config,
    user_id: u64,
    headers: &HeaderMap,
    password_reset_only: bool,
) -> AppResult<()> {
    if !password_reset_only {
        require_full_verified_user(pool, user_id).await?;
    }
    require_csrf(headers, config)?;
    Ok(())
}

pub async fn require_profile_mutation(
    pool: &MySqlPool,
    config: &Config,
    user_id: u64,
    headers: &HeaderMap,
) -> AppResult<()> {
    require_password_change_mutation(pool, config, user_id, headers, false).await
}
