use axum::body::Body;
use axum::http::{header, Request};
use axum::middleware::Next;
use axum::response::Response;

use crate::i18n;

/// Resolves locale from `Accept-Language` and runs the rest of the stack with [`i18n::request_locale`].
pub async fn negotiate_locale(req: Request<Body>, next: Next) -> Response {
    let header_val = req
        .headers()
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|h| h.to_str().ok());
    let locale = i18n::catalog().pick_locale(header_val);
    i18n::with_request_locale(locale, next.run(req)).await
}
