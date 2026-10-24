use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::Deserialize;

use crate::app_state::AppState;
use crate::auth::account_reactivation;
use crate::auth::captcha_gate::{self, ACTION_LOGIN, ACTION_REGISTER};
use crate::auth::oidc::store::{self, PROVIDER_GOOGLE};
use crate::auth::password::{self, PasswordError, MIN_PASSWORD_LEN};
use crate::auth::password_reset;
use crate::auth::post_auth;
use crate::auth::session::{self, SESSION_COOKIE_NAME};
use crate::auth::verification;
use crate::db::TimestampUtc;
use crate::error::{AppError, AppResult};
use crate::i18n;
use crate::rate_limit;
use crate::ref_key_values;
use crate::ref_keys;

pub fn router() -> Router<AppState> {
    let login_resend = rate_limit::login_resend_config();
    Router::new()
        .route(
            "/auth/register",
            post(register).layer(rate_limit::register_layer()),
        )
        .route(
            "/auth/login",
            post(login).layer(rate_limit::limit_layer(login_resend.clone())),
        )
        .route("/auth/logout", post(logout))
        .route(
            "/auth/resend-verification",
            post(resend_verification).layer(rate_limit::limit_layer(login_resend.clone())),
        )
        .route(
            "/auth/forgot-password",
            post(forgot_password).layer(rate_limit::limit_layer(login_resend.clone())),
        )
        .route(
            "/auth/reactivate-account",
            post(reactivate_account).layer(rate_limit::limit_layer(login_resend)),
        )
        .route("/dev/auth/login-as", get(dev_login_as_redirect))
        .route("/dev/auth/login-as", post(dev_login_as))
}

#[derive(Debug, Deserialize)]
struct RegisterBody {
    email: String,
    password: String,
    captcha_token: String,
    /// Honeypot: must be absent or empty; bots often fill hidden fields.
    #[serde(default)]
    website: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LoginBody {
    email: String,
    password: String,
    captcha_token: String,
}

#[derive(Debug, Deserialize)]
struct ResendBody {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct ForgotPasswordBody {
    email: String,
}

#[derive(Debug, Deserialize)]
struct DevLoginAsBody {
    email: String,
    /// Optional internal path to redirect after setting session (e.g. `/home`).
    #[serde(default)]
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DevLoginAsQuery {
    email: String,
    #[serde(default)]
    next: Option<String>,
}

fn normalize_email(s: &str) -> String {
    crate::auth::email_address::normalize(s)
}

/// JSON string so clients (e.g. JavaScript) do not lose precision above 2^53−1.
fn json_id(id: u64) -> serde_json::Value {
    serde_json::Value::String(id.to_string())
}

#[derive(Clone, Copy, Debug)]
enum AccountLinkKind {
    VerifyEmail,
    PasswordReset,
}

impl AccountLinkKind {
    fn path(self) -> &'static str {
        match self {
            Self::VerifyEmail => "/verify-email?token=",
            Self::PasswordReset => "/reset-password?token=",
        }
    }

    fn log_label(self) -> &'static str {
        match self {
            Self::VerifyEmail => "verification email",
            Self::PasswordReset => "password reset email",
        }
    }
}

async fn send_account_link_email(
    state: &AppState,
    email: &str,
    user_id: u64,
    kind: AccountLinkKind,
) {
    let token = match kind {
        AccountLinkKind::VerifyEmail => {
            verification::issue_verification_token(&state.db, user_id).await
        }
        AccountLinkKind::PasswordReset => {
            password_reset::issue_password_reset_token(&state.db, user_id).await
        }
    };
    let token = match token {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, ?kind, "issue account link token failed");
            return;
        }
    };
    let url = format!("{}{}{}", state.config.public_base_url, kind.path(), token);
    match &state.config.smtp {
        Some(smtp) => {
            let send_result = match kind {
                AccountLinkKind::VerifyEmail => {
                    crate::email::send_verification_email(
                        smtp,
                        &state.config.public_base_url,
                        email,
                        &url,
                    )
                    .await
                }
                AccountLinkKind::PasswordReset => {
                    crate::email::send_password_reset_email(
                        smtp,
                        &state.config.public_base_url,
                        email,
                        &url,
                    )
                    .await
                }
            };
            if let Err(e) = send_result {
                tracing::error!(error = %e, ?kind, "account link email SMTP send failed");
            }
        }
        None => {
            tracing::info!(%url, to = %email, label = kind.log_label(), "account link email (SMTP_HOST unset; link logged only)");
        }
    }
}

async fn send_verification_email_for_user(state: &AppState, email: &str, user_id: u64) {
    send_account_link_email(state, email, user_id, AccountLinkKind::VerifyEmail).await;
}

async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<RegisterBody>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let loc = i18n::request_locale();
    captcha_gate::require_recaptcha(
        &state,
        &headers,
        Some(body.captcha_token.as_str()),
        ACTION_REGISTER,
    )
    .await?;

    let email = normalize_email(&body.email);
    if email.is_empty() || !email.contains('@') {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "auth.invalid_email"),
        ));
    }
    let email_canonical = crate::auth::email_address::canonical(&body.email);

    // Reject disposable / temp-mail domains at registration (DB-driven denylist).
    if let Some(domain) = crate::auth::email_address::domain_of(&email) {
        if crate::auth::email_address::domain_is_disposable(&state.db, domain).await? {
            return Err(AppError::BadRequest(
                i18n::catalog().t(&loc, "auth.email_domain_disposable"),
            ));
        }
    }

    if body
        .website
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .is_some()
    {
        tracing::warn!("registration rejected: honeypot field populated");
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "ui.register.create_account_failed"),
        ));
    }

    let pending_status = ref_key_values::default_pending_status_id(&state.db)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(i18n::catalog().t(&loc, "ref_keys.default_status_unconfigured"))
        })?;
    let human_user_type = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_USER_TYPE,
        ref_keys::KEY_USER_TYPE_HUMAN,
    )
    .await?
    .ok_or_else(|| {
        AppError::BadRequest(i18n::catalog().t(&loc, "ref_keys.default_status_unconfigured"))
    })?;
    let basic_tier = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_VERIFICATION_TIER,
        ref_keys::KEY_VERIFICATION_TIER_BASIC,
    )
    .await?
    .ok_or_else(|| {
        AppError::BadRequest(i18n::catalog().t(&loc, "ref_keys.default_status_unconfigured"))
    })?;

    let hash = password::hash_password(&body.password).map_err(|e| match e {
        PasswordError::TooShort => {
            let min = MIN_PASSWORD_LEN.to_string();
            AppError::BadRequest(i18n::catalog().tf(
                &loc,
                "auth.password_too_short",
                &[("min_len", &min)],
            ))
        }
        _ => AppError::BadRequest(i18n::catalog().t(&loc, "auth.invalid_password")),
    })?;

    let res = sqlx::query(
        r#"
        INSERT INTO users (email, email_canonical, password_hash, status_id, user_type_id, verification_tier_id, terms_consent_version)
        VALUES (?, ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(&email)
    .bind(&email_canonical)
    .bind(hash.as_bytes())
    .bind(pending_status)
    .bind(human_user_type)
    .bind(basic_tier)
    .execute(&state.db)
    .await;

    let res = match res {
        Ok(r) => r,
        Err(e) => {
            if crate::social::db::is_unique_violation(&e) {
                return Err(AppError::Conflict(
                    i18n::catalog().t(&loc, "auth.email_registered"),
                ));
            }
            return Err(AppError::Db(e));
        }
    };

    let id = res.last_insert_id();
    sqlx::query(
        r#"
        UPDATE users SET created_by = ?, modified_by = ? WHERE id = ?
        "#,
    )
    .bind(id)
    .bind(id)
    .bind(id)
    .execute(&state.db)
    .await?;

    send_verification_email_for_user(&state, &email, id).await;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "user_id": json_id(id) })),
    ))
}

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(body): Json<LoginBody>,
) -> AppResult<Response> {
    captcha_gate::require_recaptcha(
        &state,
        &headers,
        Some(body.captcha_token.as_str()),
        ACTION_LOGIN,
    )
    .await?;

    let email_canonical = crate::auth::email_address::canonical(&body.email);

    let row: Option<(u64, Option<Vec<u8>>, Option<TimestampUtc>)> = sqlx::query_as(
        r#"SELECT id, password_hash, email_verified_at FROM users WHERE email_canonical = ?"#,
    )
    .bind(&email_canonical)
    .fetch_optional(&state.db)
    .await?;

    let loc = i18n::request_locale();
    let c = i18n::catalog();

    match row {
        Some((user_id, hash_opt, verified_at)) => {
            let has_password = hash_opt.as_ref().is_some_and(|h| !h.is_empty());
            if !has_password {
                if store::user_has_provider(&state.db, user_id, PROVIDER_GOOGLE)
                    .await
                    .map_err(AppError::Db)?
                {
                    return Ok((
                        StatusCode::UNAUTHORIZED,
                        Json(serde_json::json!({
                            "error": c.t(&loc, "auth.use_google_sign_in"),
                            "code": "use_oidc",
                            "provider": PROVIDER_GOOGLE,
                        })),
                    )
                        .into_response());
                }
                return Err(AppError::Unauthorized);
            }
            let hash_bytes = hash_opt.unwrap();
            let phc = String::from_utf8_lossy(&hash_bytes);
            let ok = password::verify_password(&body.password, phc.as_ref())
                .map_err(|_| AppError::Unauthorized)?;
            if !ok {
                return Err(AppError::Unauthorized);
            }
            if verified_at.is_none() {
                return Err(AppError::EmailNotVerified);
            }
            let (cookie, login_outcome) =
                session::start_user_session(&state.db, user_id, state.config.secure_cookies)
                .await
                .map_err(AppError::Db)?;
            let jar = jar.add(cookie);
            let next = post_auth::next_path_after_login(
                &state.db,
                user_id,
                state.config.face_vision_mock,
                &login_outcome,
            )
                .await
                .map_err(AppError::Db)?;
            Ok((
                jar,
                Json(serde_json::json!({ "user_id": json_id(user_id), "next": next })),
            )
                .into_response())
        }
        None => Err(AppError::Unauthorized),
    }
}

/// Same response in all cases to avoid account enumeration.
async fn resend_verification(
    State(state): State<AppState>,
    Json(body): Json<ResendBody>,
) -> AppResult<StatusCode> {
    let email_canonical = crate::auth::email_address::canonical(&body.email);

    let row: Option<(u64, String, Option<Vec<u8>>, Option<TimestampUtc>)> = sqlx::query_as(
        r#"SELECT id, email, password_hash, email_verified_at FROM users WHERE email_canonical = ?"#,
    )
    .bind(&email_canonical)
    .fetch_optional(&state.db)
    .await?;

    if let Some((user_id, account_email, Some(hash_bytes), verified_at)) = row {
        if verified_at.is_none() {
            let phc = String::from_utf8_lossy(&hash_bytes);
            if password::verify_password(&body.password, phc.as_ref()).unwrap_or(false) {
                send_verification_email_for_user(&state, &account_email, user_id).await;
            }
        }
    }

    Ok(StatusCode::ACCEPTED)
}

async fn send_password_reset_email_for_user(state: &AppState, email: &str, user_id: u64) {
    send_account_link_email(state, email, user_id, AccountLinkKind::PasswordReset).await;
}

/// Same response in all cases to avoid account enumeration.
async fn forgot_password(
    State(state): State<AppState>,
    Json(body): Json<ForgotPasswordBody>,
) -> AppResult<StatusCode> {
    let email = normalize_email(&body.email);
    if email.is_empty() || !email.contains('@') {
        return Ok(StatusCode::ACCEPTED);
    }
    let email_canonical = crate::auth::email_address::canonical(&body.email);

    if let Some((user_id, account_email)) =
        password_reset::lookup_eligible_password_reset_user(&state.db, &email_canonical).await?
    {
        send_password_reset_email_for_user(&state, &account_email, user_id).await;
    }

    Ok(StatusCode::ACCEPTED)
}

/// Same response in all cases to avoid account enumeration.
async fn reactivate_account(
    State(state): State<AppState>,
    Json(body): Json<ForgotPasswordBody>,
) -> AppResult<StatusCode> {
    let email = normalize_email(&body.email);
    if email.is_empty() || !email.contains('@') {
        return Ok(StatusCode::ACCEPTED);
    }
    let email_canonical = crate::auth::email_address::canonical(&body.email);

    if let Some((user_id, account_email)) =
        account_reactivation::lookup_eligible_reactivation_user(&state.db, &email_canonical).await?
    {
        tracing::info!(
            user_id,
            to = %account_email,
            "reactivation request matched deleted account (email dispatch not yet wired)"
        );
    }

    Ok(StatusCode::ACCEPTED)
}

async fn logout(State(state): State<AppState>, jar: CookieJar) -> AppResult<(CookieJar, Redirect)> {
    if let Some(c) = jar.get(SESSION_COOKIE_NAME) {
        let v = c.value();
        if v.len() == session::SESSION_TOKEN_HEX_LEN
            && v.as_bytes().iter().all(|b| b.is_ascii_hexdigit())
        {
            let _ = session::delete_session(&state.db, v).await;
        }
    }
    let mut del = Cookie::build((SESSION_COOKIE_NAME, ""))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::seconds(0));
    if state.config.secure_cookies {
        del = del.secure(true);
    }
    let jar = jar.remove(del.build());
    Ok((jar, Redirect::to("/login")))
}

fn normalize_next_path(next: Option<&str>) -> &'static str {
    match next.map(str::trim) {
        Some("/home") => "/home",
        Some("/account") => "/account",
        _ => "/home",
    }
}

async fn issue_dev_session_for_email(
    state: &AppState,
    jar: CookieJar,
    email: &str,
) -> AppResult<(CookieJar, u64, session::LoginRecordOutcome)> {
    if !state.config.dev_auth_bypass {
        return Err(AppError::NotFound);
    }

    let email_canonical = crate::auth::email_address::canonical(email);
    if email_canonical.is_empty() {
        return Err(AppError::BadRequest("email is required".to_string()));
    }

    let row: Option<(u64, Option<TimestampUtc>)> = sqlx::query_as(
        r#"SELECT id, email_verified_at FROM users WHERE email_canonical = ? LIMIT 1"#,
    )
    .bind(&email_canonical)
    .fetch_optional(&state.db)
    .await?;

    let (user_id, verified_at) = match row {
        Some(v) => v,
        None => return Err(AppError::NotFound),
    };
    if verified_at.is_none() {
        return Err(AppError::BadRequest(
            "dev login-as requires a verified user".to_string(),
        ));
    }

    let (cookie, login_outcome) =
        session::start_user_session(&state.db, user_id, state.config.secure_cookies)
        .await
        .map_err(AppError::Db)?;
    let jar = jar.add(cookie);
    Ok((jar, user_id, login_outcome))
}

async fn dev_login_as(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<DevLoginAsBody>,
) -> AppResult<(CookieJar, Json<serde_json::Value>)> {
    let (jar, user_id, login_outcome) = issue_dev_session_for_email(&state, jar, &body.email).await?;
    let next = post_auth::append_inactivity_refund_notice(
        normalize_next_path(body.next.as_deref()),
        login_outcome.inactivity_refunded,
    );
    Ok((
        jar,
        Json(serde_json::json!({
            "ok": true,
            "user_id": json_id(user_id),
            "next": next
        })),
    ))
}

async fn dev_login_as_redirect(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<DevLoginAsQuery>,
) -> AppResult<(CookieJar, Redirect)> {
    let (jar, _, login_outcome) = issue_dev_session_for_email(&state, jar, &q.email).await?;
    let next = post_auth::append_inactivity_refund_notice(
        normalize_next_path(q.next.as_deref()),
        login_outcome.inactivity_refunded,
    );
    Ok((jar, Redirect::to(&next)))
}
