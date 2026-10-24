//! Session-bound CSRF tokens for cookie-authenticated mutating requests.

use axum::http::HeaderMap;

use crate::auth::session::{self, SESSION_COOKIE_NAME};
use crate::crypto::sha3;

pub const CSRF_HEADER: &str = "X-CSRF-Token";
pub const CSRF_META_NAME: &str = "csrf-token";

/// Derive a hex CSRF token from the session id and server pepper.
#[must_use]
pub fn token_from_session(session_token: &str, pepper: &str) -> String {
    let mut material = Vec::with_capacity(pepper.len() + 1 + session_token.len());
    material.extend_from_slice(pepper.as_bytes());
    material.push(0);
    material.extend_from_slice(session_token.as_bytes());
    sha3::digest_hex(&material)
}

pub fn session_token_from_headers(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let part = part.trim();
        if let Some((name, value)) = part.split_once('=') {
            if name.trim() == SESSION_COOKIE_NAME {
                let v = value.trim();
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

pub fn supplied_token_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get(CSRF_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.as_bytes()
        .iter()
        .zip(b.as_bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

pub fn validate_session(headers: &HeaderMap, pepper: &str) -> bool {
    let Some(session_token) = session_token_from_headers(headers) else {
        return false;
    };
    let Some(supplied) = supplied_token_from_headers(headers) else {
        return false;
    };
    let expected = token_from_session(&session_token, pepper);
    constant_time_eq(&expected, &supplied)
}
