//! Hard funnel enforcement: consent → face capture → profile → godmother → home.
//!
//! Handlers can drift; this middleware re-resolves the required step on every GET to
//! `/home` and `/onboarding` and redirects when the browser is on the wrong surface.

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{Method, Uri};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Extension;

use crate::app_state::AppState;
use crate::middleware::VerifiedSession;

use super::post_auth;

fn normalize_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_string()
    }
}

fn onboarding_query_value(query: Option<&str>) -> Option<&str> {
    query.and_then(|q| {
        for pair in q.split('&') {
            if pair.is_empty() {
                continue;
            }
            let mut kv = pair.splitn(2, '=');
            if kv.next()? == "onboarding" {
                return kv.next();
            }
        }
        None
    })
}

/// True when the browser is already on the onboarding surface `required` points at.
fn onboarding_surfaces_match(required: &str, current_path: &str, current_query: Option<&str>) -> bool {
    let Ok(required_uri) = required.parse::<Uri>() else {
        return false;
    };

    let required_path = normalize_path(required_uri.path());
    let current_path = normalize_path(current_path);

    if required == "/onboarding" && current_path == "/onboarding" {
        return true;
    }

    if required_path != current_path {
        return false;
    }

    let required_onboarding = onboarding_query_value(required_uri.query());
    let current_onboarding = onboarding_query_value(current_query);
    required_onboarding == current_onboarding
}

/// Redirect GET `/home` / `/onboarding` when the session belongs on a different onboarding step.
pub async fn redirect_wrong_onboarding_step(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if request.method() != Method::GET {
        return next.run(request).await;
    }

    let path = request.uri().path();
    if path != "/home" && path != "/onboarding" {
        return next.run(request).await;
    }

    let required = match post_auth::next_path_after_auth(
        &state.db,
        user_id,
        state.config.face_vision_mock,
    )
    .await
    {
        Ok(p) => p,
        Err(err) => {
            tracing::warn!(error = %err, user_id, "onboarding gate lookup failed");
            return next.run(request).await;
        }
    };

    let current_query = request.uri().query();
    if onboarding_surfaces_match(&required, path, current_query) {
        return next.run(request).await;
    }

    // Plain `/home` is fine when the feed is the target (no onboarding query needed).
    if required == "/home"
        && normalize_path(path) == "/home"
        && onboarding_query_value(current_query).is_none()
    {
        return next.run(request).await;
    }

    tracing::debug!(
        user_id,
        current = %request.uri(),
        required = %required,
        "onboarding gate redirect"
    );
    Redirect::temporary(&required).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consent_surfaces_match_with_onboarding_query() {
        assert!(onboarding_surfaces_match(
            "/home?onboarding=consent",
            "/home",
            Some("onboarding=consent"),
        ));
    }

    #[test]
    fn consent_surfaces_match_with_extra_query_params() {
        assert!(onboarding_surfaces_match(
            "/home?onboarding=consent",
            "/home",
            Some("foo=1&onboarding=consent"),
        ));
    }

    #[test]
    fn consent_required_rejects_plain_home() {
        assert!(!onboarding_surfaces_match(
            "/home?onboarding=consent",
            "/home",
            None,
        ));
    }

    #[test]
    fn onboarding_path_matches_onboarding_required() {
        assert!(onboarding_surfaces_match("/onboarding", "/onboarding", None));
    }

    #[test]
    fn profile_step_matches() {
        assert!(onboarding_surfaces_match(
            "/home?onboarding=profile",
            "/home",
            Some("onboarding=profile"),
        ));
    }

    #[test]
    fn home_feed_matches_without_onboarding_param() {
        assert!(onboarding_surfaces_match("/home", "/home", None));
    }
}
