//! Google OIDC authorization code + PKCE flow.

use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use base64::Engine;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::MySqlPool;
use time::OffsetDateTime;

use crate::auth::email_address;
use crate::config::OidcGoogleConfig;
use crate::ref_key_values;
use crate::ref_keys;

use super::pending::OidcLinkPendingPayload;
use super::pending::OidcStatePayload;
use super::store::{self, GOOGLE_ISSUER, PROVIDER_GOOGLE};

pub const RECAPTCHA_ACTION_OIDC_START: &str = "oidc_google_start";
pub const RECAPTCHA_ACTION_OIDC_LINK: &str = "oidc_link_password";

#[derive(Debug, Clone)]
pub struct GoogleAuthRequest {
    pub authorize_url: String,
    pub state: OidcStatePayload,
}

#[derive(Debug, Clone)]
pub struct GoogleIdClaims {
    pub sub: String,
    pub email: String,
    pub email_verified: bool,
}

#[derive(Debug, Clone)]
pub enum GoogleCallbackOutcome {
    Login { user_id: u64 },
    Created { user_id: u64 },
    NeedsPasswordLink(OidcLinkPendingPayload),
}

#[derive(Debug, Deserialize)]
struct OpenIdConfiguration {
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
    issuer: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    id_token: String,
}

#[derive(Debug, Deserialize)]
struct JwksResponse {
    keys: Vec<JwkKey>,
}

#[derive(Debug, Deserialize)]
struct JwkKey {
    kid: String,
    kty: String,
    #[serde(default)]
    n: Option<String>,
    #[serde(default)]
    e: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleJwtClaims {
    sub: String,
    email: Option<String>,
    email_verified: Option<bool>,
    #[serde(default)]
    nonce: Option<String>,
}

pub fn generate_pkce_pair() -> (String, String) {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let verifier: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(
        verifier.as_bytes(),
    ));
    (verifier, challenge)
}

pub fn generate_state_token() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn generate_nonce() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub async fn build_google_authorize_request(
    config: &OidcGoogleConfig,
) -> Result<GoogleAuthRequest, String> {
    let discovery = fetch_openid_configuration(&config.issuer_url).await?;
    let (pkce_verifier, pkce_challenge) = generate_pkce_pair();
    let state = generate_state_token();
    let nonce = generate_nonce();
    let payload = OidcStatePayload {
        state: state.clone(),
        pkce_verifier,
        nonce: nonce.clone(),
        exp: (OffsetDateTime::now_utc() + time::Duration::minutes(10)).unix_timestamp(),
    };

    let mut url = url::Url::parse(&discovery.authorization_endpoint).map_err(|e| e.to_string())?;
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("client_id", &config.client_id);
        q.append_pair("redirect_uri", &config.redirect_uri);
        q.append_pair("response_type", "code");
        q.append_pair("scope", "openid email profile");
        q.append_pair("state", &state);
        q.append_pair("nonce", &nonce);
        q.append_pair("code_challenge", &pkce_challenge);
        q.append_pair("code_challenge_method", "S256");
        q.append_pair("prompt", "select_account");
    }

    Ok(GoogleAuthRequest {
        authorize_url: url.to_string(),
        state: payload,
    })
}

pub async fn handle_google_callback(
    pool: &MySqlPool,
    config: &OidcGoogleConfig,
    code: &str,
    state_payload: &OidcStatePayload,
) -> Result<GoogleCallbackOutcome, String> {
    let discovery = fetch_openid_configuration(&config.issuer_url).await?;
    let id_token =
        exchange_code_for_id_token(config, &discovery, code, &state_payload.pkce_verifier).await?;
    let claims = validate_google_id_token(config, &discovery, &id_token, state_payload).await?;

    if let Some(user_id) = store::find_user_by_provider_subject(
        pool,
        PROVIDER_GOOGLE,
        GOOGLE_ISSUER,
        &claims.sub,
    )
    .await
    .map_err(|e| e.to_string())?
    {
        return Ok(GoogleCallbackOutcome::Login { user_id });
    }

    let email = claims.email.trim().to_string();
    if email.is_empty() || !claims.email_verified {
        return Err("google_email_unverified".to_string());
    }
    let email_canonical = email_address::canonical(&email);
    if email_canonical.is_empty() {
        return Err("google_email_invalid".to_string());
    }

    if let Some(domain) = email_address::domain_of(&email) {
        if email_address::domain_is_disposable(pool, domain)
            .await
            .map_err(|e| e.to_string())?
        {
            return Err("email_domain_disposable".to_string());
        }
    }

    let existing: Option<(u64, Option<Vec<u8>>)> = sqlx::query_as(
        r#"SELECT id, password_hash FROM users WHERE email_canonical = ? LIMIT 1"#,
    )
    .bind(&email_canonical)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    if let Some((user_id, hash_bytes)) = existing {
        let has_password = hash_bytes.as_ref().is_some_and(|h| !h.is_empty());
        if has_password {
            return Ok(GoogleCallbackOutcome::NeedsPasswordLink(
                OidcLinkPendingPayload {
                    provider: PROVIDER_GOOGLE.to_string(),
                    issuer: GOOGLE_ISSUER.to_string(),
                    subject: claims.sub,
                    email,
                    email_canonical,
                    exp: (OffsetDateTime::now_utc() + time::Duration::minutes(15)).unix_timestamp(),
                },
            ));
        }
        store::insert_identity(
            pool,
            user_id,
            PROVIDER_GOOGLE,
            GOOGLE_ISSUER,
            &claims.sub,
            &email,
        )
        .await
        .map_err(|e| e.to_string())?;
        return Ok(GoogleCallbackOutcome::Login { user_id });
    }

    let user_id = create_oidc_user(pool, &email, &email_canonical).await?;
    store::insert_identity(
        pool,
        user_id,
        PROVIDER_GOOGLE,
        GOOGLE_ISSUER,
        &claims.sub,
        &email,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(GoogleCallbackOutcome::Created { user_id })
}

pub async fn complete_password_link(
    pool: &MySqlPool,
    pending: &OidcLinkPendingPayload,
    password: &str,
) -> Result<u64, String> {
    let row: Option<(u64, Option<Vec<u8>>)> = sqlx::query_as(
        r#"SELECT id, password_hash FROM users WHERE email_canonical = ? LIMIT 1"#,
    )
    .bind(&pending.email_canonical)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let (user_id, hash_bytes) = row.ok_or_else(|| "link_user_missing".to_string())?;
    let hash_bytes = hash_bytes
        .filter(|h| !h.is_empty())
        .ok_or_else(|| "link_no_password".to_string())?;
    let phc = String::from_utf8_lossy(&hash_bytes);
    let ok = crate::auth::password::verify_password(password, phc.as_ref())
        .map_err(|_| "link_password_invalid".to_string())?;
    if !ok {
        return Err("link_password_invalid".to_string());
    }

    if let Some(linked_user) = store::find_user_by_provider_subject(
        pool,
        &pending.provider,
        &pending.issuer,
        &pending.subject,
    )
    .await
    .map_err(|e| e.to_string())?
    {
        if linked_user != user_id {
            return Err("oidc_identity_conflict".to_string());
        }
        return Ok(user_id);
    }

    store::insert_identity(
        pool,
        user_id,
        &pending.provider,
        &pending.issuer,
        &pending.subject,
        &pending.email,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(user_id)
}

async fn create_oidc_user(
    pool: &MySqlPool,
    email: &str,
    email_canonical: &str,
) -> Result<u64, String> {
    let pending_status = ref_key_values::default_pending_status_id(pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "status_unconfigured".to_string())?;
    let human_user_type = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_USER_TYPE,
        ref_keys::KEY_USER_TYPE_HUMAN,
    )
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "user_type_unconfigured".to_string())?;
    let basic_tier = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_VERIFICATION_TIER,
        ref_keys::KEY_VERIFICATION_TIER_BASIC,
    )
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "tier_unconfigured".to_string())?;

    let res = sqlx::query(
        r#"
        INSERT INTO users (
            email, email_canonical, password_hash, email_verified_at,
            status_id, user_type_id, verification_tier_id, terms_consent_version
        )
        VALUES (?, ?, NULL, UTC_TIMESTAMP(6), ?, ?, ?, NULL)
        "#,
    )
    .bind(email)
    .bind(email_canonical)
    .bind(pending_status)
    .bind(human_user_type)
    .bind(basic_tier)
    .execute(pool)
    .await
    .map_err(|e| {
        if crate::social::db::is_unique_violation(&e) {
            "email_registered".to_string()
        } else {
            e.to_string()
        }
    })?;

    let id = res.last_insert_id();
    sqlx::query("UPDATE users SET created_by = ?, modified_by = ? WHERE id = ?")
        .bind(id)
        .bind(id)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(id)
}

async fn fetch_openid_configuration(issuer: &str) -> Result<OpenIdConfiguration, String> {
    let url = format!(
        "{}/.well-known/openid-configuration",
        issuer.trim_end_matches('/')
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

async fn exchange_code_for_id_token(
    config: &OidcGoogleConfig,
    discovery: &OpenIdConfiguration,
    code: &str,
    pkce_verifier: &str,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let resp: TokenResponse = client
        .post(&discovery.token_endpoint)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", config.redirect_uri.as_str()),
            ("client_id", config.client_id.as_str()),
            ("client_secret", config.client_secret.as_str()),
            ("code_verifier", pkce_verifier),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.id_token)
}

async fn validate_google_id_token(
    config: &OidcGoogleConfig,
    discovery: &OpenIdConfiguration,
    id_token: &str,
    state_payload: &OidcStatePayload,
) -> Result<GoogleIdClaims, String> {
    let header = decode_header(id_token).map_err(|e| e.to_string())?;
    let kid = header.kid.ok_or_else(|| "jwt_missing_kid".to_string())?;
    let decoding_key = fetch_decoding_key(&discovery.jwks_uri, &kid).await?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[config.client_id.as_str()]);
    validation.set_issuer(&[discovery.issuer.as_str(), GOOGLE_ISSUER]);

    let token_data = decode::<GoogleJwtClaims>(id_token, &decoding_key, &validation)
        .map_err(|e| e.to_string())?;
    let claims = token_data.claims;

    if claims.nonce.as_deref() != Some(state_payload.nonce.as_str()) {
        return Err("nonce_mismatch".to_string());
    }

    let email = claims.email.unwrap_or_default();
    Ok(GoogleIdClaims {
        sub: claims.sub,
        email,
        email_verified: claims.email_verified.unwrap_or(false),
    })
}

async fn fetch_decoding_key(jwks_uri: &str, kid: &str) -> Result<DecodingKey, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let jwks: JwksResponse = client
        .get(jwks_uri)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let key = jwks
        .keys
        .into_iter()
        .find(|k| k.kid == kid && k.kty == "RSA")
        .ok_or_else(|| "jwks_key_not_found".to_string())?;
    let n = key.n.ok_or_else(|| "jwks_missing_n".to_string())?;
    let e = key.e.ok_or_else(|| "jwks_missing_e".to_string())?;
    DecodingKey::from_rsa_components(&n, &e).map_err(|e| e.to_string())
}
