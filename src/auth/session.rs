use axum_extra::extract::cookie::{Cookie, SameSite};
use axum_extra::extract::CookieJar;
use rand::rngs::OsRng;
use rand::RngCore;
use rust_decimal::Decimal;
use sqlx::MySqlPool;

use crate::social::judicial::bump_global_entity_rating_tx;
use crate::social::noms;

pub const SESSION_COOKIE_NAME: &str = "sid";
pub const SESSION_TTL_DAYS: i64 = 7;

const SESSION_TOKEN_BYTES: usize = 32;
/// Hex-encoded length of [`new_session_token`] (32 bytes → 64 hex chars).
pub const SESSION_TOKEN_HEX_LEN: usize = SESSION_TOKEN_BYTES * 2;

fn new_session_token() -> String {
    let mut bytes = [0u8; SESSION_TOKEN_BYTES];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// HttpOnly session cookie; `Max-Age` matches [`SESSION_TTL_DAYS`].
pub fn build_session_cookie(token: String, secure: bool) -> Cookie<'static> {
    let mut c = Cookie::build((SESSION_COOKIE_NAME, token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::days(SESSION_TTL_DAYS));
    if secure {
        c = c.secure(true);
    }
    c.build()
}

pub async fn create_session(pool: &MySqlPool, user_id: u64) -> Result<String, sqlx::Error> {
    create_session_with_mode(pool, user_id, false).await
}

async fn create_session_with_mode(
    pool: &MySqlPool,
    user_id: u64,
    password_reset_only: bool,
) -> Result<String, sqlx::Error> {
    let token = new_session_token();
    sqlx::query(
        r#"
        INSERT INTO sessions (user_id, session_token, password_reset_only, expires_at, created_by, modified_by)
        VALUES (?, ?, ?, TIMESTAMPADD(DAY, ?, UTC_TIMESTAMP()), ?, ?)
        "#,
    )
    .bind(user_id)
    .bind(&token)
    .bind(password_reset_only)
    .bind(SESSION_TTL_DAYS)
    .bind(user_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(token)
}

/// One-time session after a reset link: full site access is blocked until password change.
pub async fn start_password_reset_session(
    pool: &MySqlPool,
    user_id: u64,
    secure_cookies: bool,
) -> Result<Cookie<'static>, sqlx::Error> {
    revoke_all_user_sessions(pool, user_id).await?;
    let token = create_session_with_mode(pool, user_id, true).await?;
    Ok(build_session_cookie(token, secure_cookies))
}

/// Side-effects recorded on sign-in (inactivity refund, daily Nom credit, counter reset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoginRecordOutcome {
    pub inactivity_refunded: Decimal,
}

/// Creates a session, records login side-effects, and returns the HttpOnly cookie to set.
pub async fn start_user_session(
    pool: &MySqlPool,
    user_id: u64,
    secure_cookies: bool,
) -> Result<(Cookie<'static>, LoginRecordOutcome), sqlx::Error> {
    let token = create_session(pool, user_id).await?;
    let outcome = record_user_login(pool, user_id).await?;
    Ok((build_session_cookie(token, secure_cookies), outcome))
}

/// Records a successful sign-in, refunds inactivity debits, and resets the penalty cycle.
pub async fn record_user_login(pool: &MySqlPool, user_id: u64) -> Result<LoginRecordOutcome, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let debited: Decimal = sqlx::query_scalar::<_, Option<Decimal>>(
        r#"SELECT inactivity_rating_debited FROM users WHERE id = ? FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    .flatten()
    .unwrap_or(Decimal::ZERO);

    let human_entity_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        LEFT JOIN user_social_entity_role ur
            ON ur.social_entity_id = se.id
           AND ur.user_id = ?
           AND ur.ends_at IS NULL
        WHERE ur.id IS NOT NULL OR se.created_by = ?
        ORDER BY
            CASE WHEN ur.is_primary = 1 THEN 0 ELSE 1 END,
            se.id ASC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;

    if debited > Decimal::ZERO {
        if let Some(human_entity_id) = human_entity_id {
            bump_global_entity_rating_tx(&mut tx, human_entity_id, debited, Some(user_id)).await?;
        }
    }

    sqlx::query(
        r#"
        UPDATE users
        SET last_login_at = UTC_TIMESTAMP(6),
            inactivity_penalties_applied = 0,
            inactivity_pruning_notified_at = NULL,
            inactivity_warning_notified_at = NULL,
            inactivity_decrement_started_notified_at = NULL,
            inactivity_acceleration_3x_notified_at = NULL,
            inactivity_acceleration_6x_notified_at = NULL,
            inactivity_rating_debited = 0.00
        WHERE id = ?
        "#,
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    if let Some(human_entity_id) = human_entity_id {
        let utc_date = chrono::Utc::now().date_naive();
        let _ = noms::try_credit_daily_login_tx(&mut tx, user_id, human_entity_id, utc_date).await?;
    }

    tx.commit().await?;
    Ok(LoginRecordOutcome {
        inactivity_refunded: debited,
    })
}

/// If the session exists and is not expired, extend `expires_at` by [`SESSION_TTL_DAYS`] from DB `UTC_TIMESTAMP()`.
pub async fn touch_session(pool: &MySqlPool, session_token: &str) -> Result<bool, sqlx::Error> {
    let res = sqlx::query(
        r#"
        UPDATE sessions
        SET expires_at = TIMESTAMPADD(DAY, ?, UTC_TIMESTAMP()),
            modified_by = user_id
        WHERE session_token = ? AND expires_at > UTC_TIMESTAMP()
        "#,
    )
    .bind(SESSION_TTL_DAYS)
    .bind(session_token)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn delete_session(pool: &MySqlPool, session_token: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM sessions WHERE session_token = ?")
        .bind(session_token)
        .execute(pool)
        .await?;
    Ok(())
}

/// Ends every session row for the user (password change, credential rotation, etc.).
pub async fn revoke_all_user_sessions(pool: &MySqlPool, user_id: u64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Clears existing sessions and issues a fresh HttpOnly cookie (does not run login side-effects).
pub async fn rotate_session_after_credential_change(
    pool: &MySqlPool,
    user_id: u64,
    secure_cookies: bool,
) -> Result<Cookie<'static>, sqlx::Error> {
    revoke_all_user_sessions(pool, user_id).await?;
    let token = create_session(pool, user_id).await?;
    Ok(build_session_cookie(token, secure_cookies))
}

/// Ends every session, issues a normal session, and records login side-effects (password-reset completion).
pub async fn complete_password_reset_session(
    pool: &MySqlPool,
    user_id: u64,
    secure_cookies: bool,
) -> Result<(Cookie<'static>, LoginRecordOutcome), sqlx::Error> {
    revoke_all_user_sessions(pool, user_id).await?;
    let token = create_session_with_mode(pool, user_id, false).await?;
    let outcome = record_user_login(pool, user_id).await?;
    Ok((build_session_cookie(token, secure_cookies), outcome))
}

/// Returns the signed-in user id when the request carries a valid, non-expired session cookie.
pub async fn optional_user_id_from_jar(pool: &MySqlPool, jar: &CookieJar) -> Option<u64> {
    let token = jar.get(SESSION_COOKIE_NAME)?.value();
    if token.len() != SESSION_TOKEN_HEX_LEN
        || !token.as_bytes().iter().all(|b| b.is_ascii_hexdigit())
    {
        return None;
    }
    sqlx::query_scalar::<_, u64>(
        r#"
        SELECT s.user_id
        FROM sessions s
        WHERE s.session_token = ? AND s.expires_at > UTC_TIMESTAMP()
        "#,
    )
    .bind(token)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

pub fn clear_session_cookie(secure_cookies: bool) -> Cookie<'static> {
    let mut del = Cookie::build((SESSION_COOKIE_NAME, ""))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::seconds(0));
    if secure_cookies {
        del = del.secure(true);
    }
    del.build()
}
