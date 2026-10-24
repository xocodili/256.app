//! Shared HTML vs JSON responses for the verified-session gate.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::i18n;

use super::browser::{self, LOGIN_HTML_PATH, RESEND_HTML_PATH};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthGateOutcome {
    MissingSession,
    InvalidSession,
    AccountRemoved,
    EmailNotVerified,
}

#[must_use]
pub fn wants_html(request: &Request<Body>) -> bool {
    request.method() == Method::GET && browser::accepts_html(request.headers())
}

#[must_use]
pub fn html_response_for(outcome: AuthGateOutcome) -> Response {
    let path = match outcome {
        AuthGateOutcome::EmailNotVerified => RESEND_HTML_PATH,
        AuthGateOutcome::MissingSession
        | AuthGateOutcome::InvalidSession
        | AuthGateOutcome::AccountRemoved => LOGIN_HTML_PATH,
    };
    browser::html_redirect(path)
}

pub fn json_response_for(outcome: AuthGateOutcome) -> Response {
    let c = i18n::catalog();
    let loc = i18n::request_locale();
    match outcome {
        AuthGateOutcome::MissingSession | AuthGateOutcome::InvalidSession => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": c.t(&loc, "middleware.sign_in_required"),
                "code": "auth_required"
            })),
        )
            .into_response(),
        AuthGateOutcome::AccountRemoved => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": c.t(&loc, "middleware.account_removed"),
                "code": "account_removed"
            })),
        )
            .into_response(),
        AuthGateOutcome::EmailNotVerified => (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": c.t(&loc, "middleware.verify_email_for_resource"),
                "code": "email_not_verified"
            })),
        )
            .into_response(),
    }
}

#[must_use]
pub fn respond(outcome: AuthGateOutcome, request: &Request<Body>) -> Response {
    if wants_html(request) {
        html_response_for(outcome)
    } else {
        json_response_for(outcome)
    }
}
