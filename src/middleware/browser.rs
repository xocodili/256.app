//! Browser-oriented request detection and auth redirects for HTML navigations.

use axum::http::{header, HeaderMap};
use axum::response::{IntoResponse, Redirect, Response};

/// Guest sign-in panel (protected routes without a session).
pub const LOGIN_HTML_PATH: &str = "/login?login=1";

/// Resend verification panel (session present but email not verified).
pub const RESEND_HTML_PATH: &str = "/login?resend=1";

#[must_use]
pub fn accepts_html(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("text/html"))
        .unwrap_or(false)
}

#[must_use]
pub fn html_redirect(path: &'static str) -> Response {
    Redirect::temporary(path).into_response()
}
