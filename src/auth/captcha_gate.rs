//! reCAPTCHA v3 enforcement for auth endpoints.

use axum::http::HeaderMap;

use crate::app_state::AppState;
use crate::auth::recaptcha::{self, RecaptchaError};
use crate::error::{AppError, AppResult};
use crate::i18n;

pub const ACTION_REGISTER: &str = "register";
pub const ACTION_LOGIN: &str = "login";

pub fn client_ip_from_headers(headers: &HeaderMap, trusted_proxy: bool) -> Option<String> {
    if !trusted_proxy {
        return None;
    }
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
}

pub async fn require_recaptcha(
    state: &AppState,
    headers: &HeaderMap,
    token: Option<&str>,
    expected_action: &str,
) -> AppResult<()> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let generic = c.t(&loc, "auth.sign_in_failed_generic");

    let Some(config) = state.config.recaptcha.as_ref() else {
        if state.config.dev_auth_bypass {
            return Ok(());
        }
        tracing::error!("recaptcha required but RECAPTCHA_SECRET_KEY unset");
        return Err(AppError::BadRequest(generic));
    };

    if config.bypass {
        return Ok(());
    }

    let ip_owned = client_ip_from_headers(headers, state.config.trusted_proxy);
    let ip = ip_owned.as_deref();
    match recaptcha::verify_recaptcha_v3(config, token.unwrap_or(""), ip, expected_action).await {
        Ok(_) => Ok(()),
        Err(RecaptchaError::NotConfigured) => Err(AppError::BadRequest(generic)),
        Err(e) => {
            tracing::warn!(?e, action = expected_action, "recaptcha gate rejected request");
            Err(AppError::Unauthorized)
        }
    }
}
