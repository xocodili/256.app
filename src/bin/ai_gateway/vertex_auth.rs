//! Vertex AI / OAuth2 service-account helpers for `ai_gateway` binary.

use std::time::{Duration, Instant};

use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct ServiceAccountJson {
    pub project_id: String,
    pub private_key: String,
    pub client_email: String,
    pub token_uri: String,
}

#[derive(Serialize)]
struct JwtClaims {
    iss: String,
    sub: String,
    scope: String,
    aud: String,
    iat: i64,
    exp: i64,
}

pub fn load_service_account(path: &str) -> Result<ServiceAccountJson, String> {
    let data = std::fs::read_to_string(path)
        .map_err(|e| format!("read GOOGLE_APPLICATION_CREDENTIALS: {e}"))?;
    serde_json::from_str(&data).map_err(|e| format!("parse service account JSON: {e}"))
}

/// Build JWT and exchange for OAuth access token (cloud-platform scope).
pub async fn fetch_oauth_access_token(sa: &ServiceAccountJson) -> Result<(String, i64), String> {
    let now = chrono::Utc::now().timestamp();
    let exp = now + 3500;
    let claims = JwtClaims {
        iss: sa.client_email.clone(),
        sub: sa.client_email.clone(),
        scope: "https://www.googleapis.com/auth/cloud-platform".to_string(),
        aud: sa.token_uri.clone(),
        iat: now,
        exp,
    };
    let key = EncodingKey::from_rsa_pem(sa.private_key.as_bytes())
        .map_err(|e| format!("private key PEM: {e}"))?;
    let header = Header::new(Algorithm::RS256);
    let assertion = encode(&header, &claims, &key).map_err(|e| format!("jwt encode: {e}"))?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    let resp = client
        .post(&sa.token_uri)
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", assertion.as_str()),
        ])
        .send()
        .await
        .map_err(|e| format!("oauth token request: {e}"))?;

    let status = resp.status();
    let bytes = resp.bytes().await.map_err(|e| format!("oauth body: {e}"))?;
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);

    if !status.is_success() {
        let detail = json
            .get("error_description")
            .or_else(|| json.get("error"))
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| String::from_utf8_lossy(&bytes).chars().take(400).collect());
        return Err(format!("oauth token {status}: {detail}"));
    }

    let token = json
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "oauth response missing access_token".to_string())?
        .to_string();

    let expires_in = json
        .get("expires_in")
        .and_then(|v| v.as_i64())
        .unwrap_or(3600);

    Ok((token, expires_in))
}

pub fn vertex_generate_content_url(project: &str, location: &str, model: &str) -> String {
    format!(
        "https://{location}-aiplatform.googleapis.com/v1/projects/{project}/locations/{location}/publishers/google/models/{model}:generateContent"
    )
}

pub struct CachedAccessToken {
    pub token: String,
    /// Use cached token while `Instant::now() < valid_until`.
    pub valid_until: Instant,
}

pub fn cache_from_oauth(token: String, expires_in: i64) -> CachedAccessToken {
    let secs = expires_in.max(120) as u64;
    CachedAccessToken {
        token,
        valid_until: Instant::now() + Duration::from_secs(secs.saturating_sub(300)),
    }
}
