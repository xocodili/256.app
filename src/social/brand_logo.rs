//! Brand logo ingress: domain match with website, fetch, hash gate, council vision review.

use std::collections::HashMap;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use serde::Deserialize;
use serde_json::Value;
use sqlx::MySqlPool;

use crate::ai::grpc::{self, AiClientError, BRAND_LOGO_VALIDATE_AGENT};
use crate::config::Config;
use crate::content::links::{self, CanonicalizedUrl};
use crate::error::{AppError, AppResult};
use crate::i18n;
use crate::i18n::Catalog;
use crate::social::safety::{self, sanitation};

const LOGO_FETCH_MAX_BYTES: usize = 2 * 1024 * 1024;
const LOGO_FETCH_TIMEOUT_SECS: u64 = 10;

#[derive(Debug, Deserialize)]
struct ValidateResponse {
    valid: Option<bool>,
}

/// Logo URL must share the website's registrable (eTLD+1) domain — carpets match curtains.
#[must_use]
pub fn logo_shares_website_registrable_domain(
    logo: &CanonicalizedUrl,
    website: &CanonicalizedUrl,
) -> bool {
    logo.registrable_domain == website.registrable_domain
}

/// Canonicalize optional logo URL, enforce website domain match, fetch + review, upsert `content_link`.
pub async fn optional_brand_logo_url_to_content_link_id(
    pool: &MySqlPool,
    config: &Config,
    logo_url: Option<&str>,
    website_url: Option<&str>,
    acting_user_id: u64,
    loc: &str,
) -> AppResult<Option<u64>> {
    let Some(logo_raw) = logo_url.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let Some(website_raw) = website_url.map(str::trim).filter(|s| !s.is_empty()) else {
        return Err(AppError::BadRequest(
            i18n::catalog().t(loc, "social_entity.logo_requires_website"),
        ));
    };

    let logo_canonical = links::canonicalize_url(logo_raw)?;
    let website_canonical = links::canonicalize_url(website_raw)?;

    if !logo_canonical
        .url_canonical
        .to_ascii_lowercase()
        .starts_with("https://")
    {
        return Err(AppError::BadRequest(
            i18n::catalog().t(loc, "social_entity.profit_logo_url_invalid"),
        ));
    }

    if !logo_shares_website_registrable_domain(&logo_canonical, &website_canonical) {
        return Err(AppError::BadRequest(
            i18n::catalog().t(loc, "social_entity.logo_domain_mismatch"),
        ));
    }

    links::ensure_domain_allowed(
        pool,
        &logo_canonical.registrable_domain,
        "entity_brand_logo",
        Some(acting_user_id),
        None,
    )
    .await?;

    let bytes = fetch_logo_bytes(&logo_canonical.url_canonical).await?;
    scan_logo_bytes(&bytes, config, acting_user_id, pool).await?;

    let c = i18n::catalog();
    validate_logo_with_council(config, acting_user_id, &bytes, loc, &c).await?;

    let up = links::upsert_canonical_link(pool, &logo_canonical, Some(acting_user_id)).await?;
    Ok(Some(up.link_id))
}

async fn fetch_logo_bytes(url: &str) -> AppResult<Vec<u8>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(LOGO_FETCH_TIMEOUT_SECS))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .map_err(|e| AppError::BadRequest(format!("logo fetch client: {e}")))?;

    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("256.app/brand-logo-ingress (+https://256.app)"),
    );
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("image/jpeg, image/png, image/webp, */*;q=0.1"),
    );

    let resp = client
        .get(url)
        .headers(headers)
        .send()
        .await
        .map_err(|_| {
            AppError::BadRequest(i18n::catalog().t(
                &i18n::request_locale(),
                "social_entity.logo_fetch_failed",
            ))
        })?;

    if !resp.status().is_success() {
        return Err(AppError::BadRequest(
            i18n::catalog().t(
                &i18n::request_locale(),
                "social_entity.logo_fetch_failed",
            ),
        ));
    }

    let ctype = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if !content_type_is_allowed_logo(&ctype, url) {
        return Err(AppError::BadRequest(
            i18n::catalog().t(
                &i18n::request_locale(),
                "social_entity.logo_not_image",
            ),
        ));
    }

    let bytes = resp.bytes().await.map_err(|_| {
        AppError::BadRequest(i18n::catalog().t(
            &i18n::request_locale(),
            "social_entity.logo_fetch_failed",
        ))
    })?;

    if bytes.len() > LOGO_FETCH_MAX_BYTES {
        return Err(AppError::BadRequest(
            i18n::catalog().t(
                &i18n::request_locale(),
                "social_entity.logo_fetch_failed",
            ),
        ));
    }

    if !magic_bytes_look_like_image(&bytes) {
        return Err(AppError::BadRequest(
            i18n::catalog().t(
                &i18n::request_locale(),
                "social_entity.logo_not_image",
            ),
        ));
    }

    Ok(bytes.to_vec())
}

fn content_type_is_allowed_logo(ctype: &str, url: &str) -> bool {
    if ctype.starts_with("image/jpeg")
        || ctype.starts_with("image/png")
        || ctype.starts_with("image/webp")
    {
        return true;
    }
    let path = url.to_ascii_lowercase();
    path.ends_with(".jpg")
        || path.ends_with(".jpeg")
        || path.ends_with(".png")
        || path.ends_with(".webp")
}

fn magic_bytes_look_like_image(bytes: &[u8]) -> bool {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return true;
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return true;
    }
    bytes.len() >= 12 && bytes.starts_with(b"RIFF") && bytes[8..12] == *b"WEBP"
}

async fn scan_logo_bytes(
    bytes: &[u8],
    config: &Config,
    acting_user_id: u64,
    pool: &MySqlPool,
) -> AppResult<()> {
    if let safety::MediaScanOutcome::Blocked {
        harm_class,
        sha256_hex,
    } = safety::scan_jpeg_bytes(bytes, &config.media_hash_blocklist)
    {
        sanitation::log_hash_gate_block(
            pool,
            "brand_logo",
            harm_class.as_str(),
            &sha256_hex,
            Some(acting_user_id),
        )
        .await;
        return Err(AppError::BadRequest(i18n::catalog().t(
            &i18n::request_locale(),
            "social_entity.logo_rejected_safety",
        )));
    }
    Ok(())
}

async fn validate_logo_with_council(
    config: &Config,
    user_id: u64,
    bytes: &[u8],
    loc: &str,
    c: &Catalog,
) -> AppResult<()> {
    if config.face_vision_mock {
        return Ok(());
    }

    let mut metadata = HashMap::new();
    metadata.insert(
        "text_prompt".to_string(),
        crate::agent_specs::compose_brand_logo_validate_prompt(),
    );
    metadata.insert(
        "business_function".to_string(),
        "brand_logo_validate".to_string(),
    );

    let (response_text, model_id) = grpc::run_text_agent(
        &config.ai_grpc_endpoint,
        config.ai_grpc_timeout_ms,
        BRAND_LOGO_VALIDATE_AGENT,
        user_id,
        vec![("logo".to_string(), bytes.to_vec())],
        metadata,
    )
    .await
    .map_err(|e| map_vision_error(e, loc, c))?;

    match parse_validate_response(&response_text) {
        Ok(true) => {
            tracing::debug!(model_id = %model_id, "brand logo vision validation passed");
            Ok(())
        }
        Ok(false) => {
            tracing::info!(model_id = %model_id, "brand logo vision validation rejected image");
            Err(AppError::BadRequest(
                c.t(loc, "social_entity.logo_rejected_safety"),
            ))
        }
        Err(reason) => {
            tracing::warn!(
                model_id = %model_id,
                reason = %reason,
                response_preview = %response_text.chars().take(200).collect::<String>(),
                "brand logo vision validation parse failed — rejecting"
            );
            Err(AppError::BadRequest(
                c.t(loc, "social_entity.logo_rejected_safety"),
            ))
        }
    }
}

fn map_vision_error(err: AiClientError, loc: &str, c: &Catalog) -> AppError {
    tracing::warn!(error = %err, "brand logo vision validation unavailable");
    AppError::BadRequest(c.t(loc, "social_entity.logo_validation_unavailable"))
}

fn parse_validate_response(text: &str) -> Result<bool, String> {
    let json = extract_json_object(text).ok_or_else(|| "no json object in response".to_string())?;
    let parsed: ValidateResponse =
        serde_json::from_value(json).map_err(|e| format!("invalid validate json: {e}"))?;
    match parsed.valid {
        Some(true) => Ok(true),
        Some(false) => Ok(false),
        None => Err("missing valid field".to_string()),
    }
}

fn extract_json_object(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
        if v.is_object() {
            return Some(v);
        }
    }
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if end > start {
                if let Ok(v) = serde_json::from_str::<Value>(&trimmed[start..=end]) {
                    if v.is_object() {
                        return Some(v);
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canon(url: &str, domain: &str) -> CanonicalizedUrl {
        CanonicalizedUrl {
            url_original: url.to_string(),
            url_canonical: url.to_string(),
            registrable_domain: domain.to_string(),
        }
    }

    #[test]
    fn logo_domain_must_match_website_etld1() {
        let site = canon("https://www.riverkeepers.org/about", "riverkeepers.org");
        let logo = canon("https://cdn.riverkeepers.org/logo.png", "riverkeepers.org");
        assert!(logo_shares_website_registrable_domain(&logo, &site));

        let bad = canon("https://evil.com/logo.png", "evil.com");
        assert!(!logo_shares_website_registrable_domain(&bad, &site));
    }

    #[test]
    fn magic_bytes_detect_common_formats() {
        assert!(magic_bytes_look_like_image(&[0xFF, 0xD8, 0xFF, 0x00]));
        assert!(magic_bytes_look_like_image(b"\x89PNG\r\n\x1a\n"));
        assert!(!magic_bytes_look_like_image(b"not an image"));
    }
}
