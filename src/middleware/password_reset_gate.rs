use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::i18n;
use super::browser;
use super::VerifiedSession;

fn password_reset_allowed(method: &Method, path: &str) -> bool {
    matches!(
        (method, path),
        (&Method::GET, "/account/change-password")
            | (&Method::GET, "/account/change-password/")
            | (&Method::POST, "/me/change-password")
    )
}

/// Restrict password-reset-only sessions to the change-password surface until complete.
pub async fn restrict_password_reset_session(
    request: Request<Body>,
    next: Next,
) -> Response {
    if !request
        .extensions()
        .get::<VerifiedSession>()
        .is_some_and(|s| s.password_reset_only)
    {
        return next.run(request).await;
    }

    let path = request.uri().path();
    if password_reset_allowed(request.method(), path) {
        return next.run(request).await;
    }

    let c = i18n::catalog();
    let loc = i18n::request_locale();

    if request.method() == Method::GET && browser::accepts_html(request.headers()) {
        return browser::html_redirect("/account/change-password");
    }

    (
        StatusCode::FORBIDDEN,
        Json(serde_json::json!({
            "error": c.t(&loc, "middleware.password_reset_required"),
            "code": "password_reset_required"
        })),
    )
        .into_response()
}
