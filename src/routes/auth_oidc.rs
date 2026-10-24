//! Google OIDC HTTP routes (`/auth/oidc/*`).

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::cookie::CookieJar;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::auth::captcha_gate;
use crate::auth::oidc::{
    build_google_authorize_request, build_oidc_link_cookie, build_oidc_state_cookie,
    clear_cookie, complete_password_link, handle_google_callback, parse_oidc_link_cookie,
    parse_oidc_state_cookie, GoogleCallbackOutcome, OIDC_LINK_COOKIE, OIDC_STATE_COOKIE,
    RECAPTCHA_ACTION_OIDC_LINK, RECAPTCHA_ACTION_OIDC_START,
};
use crate::auth::post_auth;
use crate::auth::session;
use crate::error::{AppError, AppResult};
use crate::i18n;
use crate::rate_limit;
use crate::html_escape::escape_html;
use crate::ui::snippets;

const OIDC_LINK_TEMPLATE: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/oidc_link.html"));

pub fn router() -> Router<AppState> {
    let login_resend = rate_limit::login_resend_config();
    Router::new()
        .route(
            "/auth/oidc/google/start",
            post(oidc_google_start).layer(rate_limit::limit_layer(login_resend.clone())),
        )
        .route(
            "/auth/oidc/callback",
            get(oidc_google_callback).layer(rate_limit::limit_layer(login_resend.clone())),
        )
        .route("/auth/oidc/link", get(oidc_link_page))
        .route(
            "/auth/oidc/link",
            post(oidc_link_submit).layer(rate_limit::limit_layer(login_resend)),
        )
}

#[derive(Debug, Deserialize)]
struct OidcStartBody {
    captcha_token: String,
}

#[derive(Debug, Deserialize)]
struct OidcCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OidcLinkBody {
    password: String,
    captcha_token: String,
}

fn oidc_config(state: &AppState) -> AppResult<&crate::config::OidcGoogleConfig> {
    state
        .config
        .oidc_google
        .as_ref()
        .ok_or(AppError::NotFound)
}

#[derive(Debug, Serialize)]
struct OidcStartResponse {
    authorize_url: String,
}

async fn oidc_google_start(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(body): Json<OidcStartBody>,
) -> AppResult<(CookieJar, Json<OidcStartResponse>)> {
    let config = oidc_config(&state)?;
    captcha_gate::require_recaptcha(
        &state,
        &headers,
        Some(body.captcha_token.as_str()),
        RECAPTCHA_ACTION_OIDC_START,
    )
    .await?;

    let auth = build_google_authorize_request(config)
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "oidc authorize build failed");
            AppError::BadRequest(i18n::catalog().t(&i18n::request_locale(), "auth.sign_in_failed_generic"))
        })?;

    let cookie = build_oidc_state_cookie(
        &state.config.csrf_pepper,
        state.config.secure_cookies,
        auth.state,
    )
    .ok_or_else(|| AppError::BadRequest("oidc state cookie failed".to_string()))?;

    let jar = jar.add(cookie);
    Ok((
        jar,
        Json(OidcStartResponse {
            authorize_url: auth.authorize_url,
        }),
    ))
}

async fn oidc_google_callback(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<OidcCallbackQuery>,
) -> AppResult<(CookieJar, Redirect)> {
    if q.error.is_some() {
        return Ok((
            jar,
            Redirect::temporary("/login?login=1&oidc_error=1"),
        ));
    }

    let code = q.code.unwrap_or_default();
    let state_param = q.state.unwrap_or_default();
    if code.trim().is_empty() || state_param.trim().is_empty() {
        return Ok((jar, Redirect::temporary("/login?login=1&oidc_error=1")));
    }

    let state_cookie = match jar
        .get(OIDC_STATE_COOKIE)
        .and_then(|c| parse_oidc_state_cookie(&state.config.csrf_pepper, c.value()))
    {
        Some(s) if s.state == state_param.trim() => s,
        _ => {
            return Ok((
                jar,
                Redirect::temporary("/login?login=1&oidc_error=1"),
            ));
        }
    };

    let config = oidc_config(&state)?;
    let outcome = match handle_google_callback(&state.db, config, code.trim(), &state_cookie).await
    {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!(error = %e, "oidc callback failed");
            return Ok((
                jar.remove(clear_cookie(
                    OIDC_STATE_COOKIE,
                    state.config.secure_cookies,
                )),
                Redirect::temporary("/login?login=1&oidc_error=1"),
            ));
        }
    };

    let mut jar = jar.remove(clear_cookie(OIDC_STATE_COOKIE, state.config.secure_cookies));

    match outcome {
        GoogleCallbackOutcome::Login { user_id } | GoogleCallbackOutcome::Created { user_id } => {
            let (session_cookie, login_outcome) =
                session::start_user_session(&state.db, user_id, state.config.secure_cookies)
                    .await
                    .map_err(AppError::Db)?;
            jar = jar.add(session_cookie);
            let next = post_auth::next_path_after_login(
                &state.db,
                user_id,
                state.config.face_vision_mock,
                &login_outcome,
            )
                .await
                .map_err(AppError::Db)?;
            Ok((jar, Redirect::temporary(&next)))
        }
        GoogleCallbackOutcome::NeedsPasswordLink(pending) => {
            let link_cookie = build_oidc_link_cookie(
                &state.config.csrf_pepper,
                state.config.secure_cookies,
                pending,
            )
            .ok_or_else(|| AppError::BadRequest("oidc link cookie failed".to_string()))?;
            jar = jar.add(link_cookie);
            Ok((jar, Redirect::temporary("/auth/oidc/link")))
        }
    }
}

async fn oidc_link_page(State(state): State<AppState>, jar: CookieJar) -> AppResult<Response> {
    if state.config.oidc_google.is_none() {
        return Err(AppError::NotFound);
    }
    let pending = jar
        .get(OIDC_LINK_COOKIE)
        .and_then(|c| parse_oidc_link_cookie(&state.config.csrf_pepper, c.value()))
        .ok_or(AppError::NotFound)?;

    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let recaptcha_site_key = state
        .config
        .recaptcha
        .as_ref()
        .map(|r| r.site_key.as_str())
        .unwrap_or("");
    let recaptcha_script = if recaptcha_site_key.is_empty() {
        String::new()
    } else {
        format!(
            r#"<script src="https://www.google.com/recaptcha/api.js?render={}"></script>"#,
            escape_html(recaptcha_site_key)
        )
    };
    let subtitle = c.tf(
        &loc,
        "auth.oidc_link_subtitle",
        &[("email", &pending.email)],
    );
    let html = OIDC_LINK_TEMPLATE
        .replace("__HTML_LANG__", &loc)
        .replace("__OIDC_LINK_EMAIL__", &escape_html(&pending.email))
        .replace("__RECAPTCHA_SITE_KEY__", recaptcha_site_key)
        .replace("__RECAPTCHA_SCRIPT__", &recaptcha_script)
        .replace(
            "__OIDC_LINK_I18N_JSON__",
            &serde_json::json!({
                "heading": c.t(&loc, "auth.oidc_link_heading"),
                "subtitle": subtitle,
                "password_label": c.t(&loc, "ui.shared.password_label"),
                "submit": c.t(&loc, "auth.oidc_link_submit"),
                "sign_in_failed_generic": c.t(&loc, "auth.sign_in_failed_generic"),
                "network_error_detail": c.t(&loc, "ui.shared.network_error_detail"),
            })
            .to_string(),
        );
    Ok(Html(i18n::subst_markers(
        &snippets::inject_site_favicon(&html),
        &loc,
    ))
    .into_response())
}

async fn oidc_link_submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(body): Json<OidcLinkBody>,
) -> AppResult<(CookieJar, Json<serde_json::Value>)> {
    oidc_config(&state)?;
    captcha_gate::require_recaptcha(
        &state,
        &headers,
        Some(body.captcha_token.as_str()),
        RECAPTCHA_ACTION_OIDC_LINK,
    )
    .await?;

    let pending = jar
        .get(OIDC_LINK_COOKIE)
        .and_then(|c| parse_oidc_link_cookie(&state.config.csrf_pepper, c.value()))
        .ok_or(AppError::Unauthorized)?;

    let user_id = complete_password_link(&state.db, &pending, body.password.trim())
        .await
        .map_err(|_| AppError::Unauthorized)?;

    let (session_cookie, login_outcome) =
        session::start_user_session(&state.db, user_id, state.config.secure_cookies)
            .await
            .map_err(AppError::Db)?;
    let mut jar = jar
        .remove(clear_cookie(OIDC_LINK_COOKIE, state.config.secure_cookies))
        .add(session_cookie);

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
        Json(serde_json::json!({ "ok": true, "next": next })),
    ))
}
