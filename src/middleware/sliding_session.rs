use axum::body::Body;
use axum::extract::State;
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderMap, HeaderValue, Method, Request};
use axum::middleware::Next;
use axum::response::Response;
use axum_extra::extract::cookie::Cookie;

use crate::app_state::AppState;
use crate::auth::session::{self, SESSION_COOKIE_NAME};

fn session_token_from_cookies(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let part = part.trim();
        if let Ok(c) = Cookie::parse(part) {
            if c.name() == SESSION_COOKIE_NAME {
                let v = c.value();
                if v.len() == session::SESSION_TOKEN_HEX_LEN
                    && v.as_bytes().iter().all(|b| b.is_ascii_hexdigit())
                {
                    return Some(v.to_string());
                }
                return None;
            }
        }
    }
    None
}

/// Avoid sliding on auth POSTs so logout/login/register are not confused with an active visit.
fn skip_sliding<B>(req: &Request<B>) -> bool {
    if req.method() != Method::POST {
        return false;
    }
    matches!(
        req.uri().path(),
        "/auth/register" | "/auth/login" | "/auth/logout" | "/auth/resend-verification"
            | "/auth/forgot-password"
    )
}

pub async fn sliding_session(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if skip_sliding(&request) {
        return next.run(request).await;
    }

    let token = session_token_from_cookies(request.headers());
    let Some(token) = token else {
        return next.run(request).await;
    };

    let refreshed = match session::touch_session(&state.db, &token).await {
        Ok(true) => true,
        Ok(false) => false,
        Err(e) => {
            tracing::warn!(error = %e, "session touch failed");
            false
        }
    };

    let mut response = next.run(request).await;

    if refreshed {
        let cookie = session::build_session_cookie(token, state.config.secure_cookies);
        if let Ok(val) = HeaderValue::from_str(&cookie.to_string()) {
            response.headers_mut().append(SET_COOKIE, val);
        }
    }

    response
}
