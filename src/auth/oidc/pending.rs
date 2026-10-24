//! Signed short-lived cookies for OAuth CSRF/PKCE and pending account links.

use axum_extra::extract::cookie::{Cookie, SameSite};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use time::OffsetDateTime;

type HmacSha256 = Hmac<Sha256>;

pub const OIDC_STATE_COOKIE: &str = "oidc_state";
pub const OIDC_LINK_COOKIE: &str = "oidc_link_pending";

const STATE_TTL_SECS: i64 = 600;
const LINK_TTL_SECS: i64 = 900;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OidcStatePayload {
    pub state: String,
    pub pkce_verifier: String,
    pub nonce: String,
    /// Unix timestamp (UTC seconds).
    pub exp: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OidcLinkPendingPayload {
    pub provider: String,
    pub issuer: String,
    pub subject: String,
    pub email: String,
    pub email_canonical: String,
    /// Unix timestamp (UTC seconds).
    pub exp: i64,
}

fn sign_payload(pepper: &str, name: &str, json: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(format!("{pepper}:{name}").as_bytes()).expect("HMAC key");
    mac.update(json.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn verify_payload(pepper: &str, name: &str, json: &str, sig: &str) -> bool {
    let expected = sign_payload(pepper, name, json);
    constant_time_eq(sig.as_bytes(), expected.as_bytes())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

fn encode_signed(pepper: &str, name: &str, payload: &impl serde::Serialize) -> Option<String> {
    let json = serde_json::to_string(payload).ok()?;
    let sig = sign_payload(pepper, name, &json);
    Some(format!(
        "{}.{}",
        base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            json.as_bytes()
        ),
        sig
    ))
}

fn decode_signed<T: serde::de::DeserializeOwned>(
    pepper: &str,
    name: &str,
    raw: &str,
) -> Option<T> {
    let (body_b64, sig) = raw.split_once('.')?;
    let json = base64::Engine::decode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        body_b64,
    )
    .ok()?;
    let json = String::from_utf8(json).ok()?;
    if !verify_payload(pepper, name, &json, sig) {
        return None;
    }
    let payload: T = serde_json::from_str(&json).ok()?;
    Some(payload)
}

#[must_use]
pub fn build_oidc_state_cookie(
    pepper: &str,
    secure: bool,
    payload: OidcStatePayload,
) -> Option<Cookie<'static>> {
    let value = encode_signed(pepper, OIDC_STATE_COOKIE, &payload)?;
    Some(build_http_only_cookie(OIDC_STATE_COOKIE, value, secure, STATE_TTL_SECS))
}

#[must_use]
pub fn parse_oidc_state_cookie(pepper: &str, value: &str) -> Option<OidcStatePayload> {
    let payload: OidcStatePayload = decode_signed(pepper, OIDC_STATE_COOKIE, value)?;
    if payload.exp < OffsetDateTime::now_utc().unix_timestamp() {
        return None;
    }
    Some(payload)
}

#[must_use]
pub fn build_oidc_link_cookie(
    pepper: &str,
    secure: bool,
    payload: OidcLinkPendingPayload,
) -> Option<Cookie<'static>> {
    let value = encode_signed(pepper, OIDC_LINK_COOKIE, &payload)?;
    Some(build_http_only_cookie(OIDC_LINK_COOKIE, value, secure, LINK_TTL_SECS))
}

#[must_use]
pub fn parse_oidc_link_cookie(pepper: &str, value: &str) -> Option<OidcLinkPendingPayload> {
    let payload: OidcLinkPendingPayload = decode_signed(pepper, OIDC_LINK_COOKIE, value)?;
    if payload.exp < OffsetDateTime::now_utc().unix_timestamp() {
        return None;
    }
    Some(payload)
}

#[must_use]
pub fn clear_cookie(name: &'static str, secure: bool) -> Cookie<'static> {
    build_http_only_cookie(name, String::new(), secure, 0)
}

fn build_http_only_cookie(name: &'static str, value: String, secure: bool, max_age_secs: i64) -> Cookie<'static> {
    let mut c = Cookie::build((name, value))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::seconds(max_age_secs));
    if secure {
        c = c.secure(true);
    }
    c.build()
}
