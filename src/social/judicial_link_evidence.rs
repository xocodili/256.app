//! Capture external URLs cited in post or case markup into `judicial_case_evidence` at filing time.

use std::collections::HashSet;

use reqwest::Url;
use sha2::{Digest, Sha256};
use sqlx::MySql;

const MAX_URLS_PER_CASE: usize = 12;
const MAX_FETCH_BYTES: usize = 1_048_576;
const FETCH_TIMEOUT_SECS: u64 = 12;

#[derive(Debug, Clone)]
pub struct CapturedUrlEvidence {
    pub source_url: String,
    pub captured_html: Option<String>,
    pub captured_text: String,
    pub content_sha256: [u8; 32],
}

/// Collect unique http(s) URLs from markup fragments and optional structured link fields.
#[must_use]
pub fn collect_external_urls(markup_parts: &[&str], extra_hrefs: &[Option<&str>]) -> Vec<String> {
    let mut seen = HashSet::<String>::new();
    let mut out = Vec::new();
    for part in markup_parts {
        for u in crate::content::url_extract::extract_urls_from_text(part) {
            if seen.insert(u.clone()) {
                out.push(u);
                if out.len() >= MAX_URLS_PER_CASE {
                    return out;
                }
            }
        }
    }
    for h in extra_hrefs.iter().flatten().copied() {
        let t = h.trim();
        if t.is_empty() {
            continue;
        }
        if let Some(norm) = crate::content::url_extract::normalize_public_http_url(t) {
            if seen.insert(norm.clone()) {
                out.push(norm);
                if out.len() >= MAX_URLS_PER_CASE {
                    break;
                }
            }
        }
    }
    out
}

fn normalize_public_http_url(raw: &str) -> Option<String> {
    crate::content::url_extract::normalize_public_http_url(raw)
}

fn html_to_plain_text(html: &str) -> String {
    strip_tags_like(html)
}

fn strip_tags_like(html: &str) -> String {
    let mut out = String::with_capacity(html.len().min(65536));
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    let t = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.len() > 500_000 {
        t.chars().take(500_000).collect()
    } else {
        t
    }
}

/// Fetch each URL (best-effort; failures still insert a row with error text for audit).
///
/// Runs fetches **concurrently** so filing is not blocked for `timeout × N` wall time when a post
/// cites many external links (each fetch is still capped by the client timeout).
pub async fn capture_external_urls(
    client: &reqwest::Client,
    urls: &[String],
) -> Vec<CapturedUrlEvidence> {
    if urls.is_empty() {
        return Vec::new();
    }
    let mut set = tokio::task::JoinSet::new();
    for url in urls {
        let url = url.clone();
        let client = client.clone();
        set.spawn(async move { fetch_one(&client, &url).await });
    }
    let mut out = Vec::with_capacity(urls.len());
    while let Some(res) = set.join_next().await {
        match res {
            Ok(row) => out.push(row),
            Err(e) => {
                tracing::warn!(error = %e, "judicial evidence fetch task join failed");
            }
        }
    }
    out
}

async fn fetch_one(client: &reqwest::Client, url: &str) -> CapturedUrlEvidence {
    let res = match client.get(url).send().await {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("[fetch error] {e}");
            let digest = sha256_bytes(msg.as_bytes());
            return CapturedUrlEvidence {
                source_url: url.to_string(),
                captured_html: None,
                captured_text: msg,
                content_sha256: digest,
            };
        }
    };
    let status = res.status();
    let ctype_header = res
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_ascii_lowercase());
    let bytes = match res.bytes().await {
        Ok(b) => b,
        Err(e) => {
            let msg = format!("[body read error: {status}] {e}");
            let digest = sha256_bytes(msg.as_bytes());
            return CapturedUrlEvidence {
                source_url: url.to_string(),
                captured_html: None,
                captured_text: msg,
                content_sha256: digest,
            };
        }
    };
    let mut body = bytes.as_ref();
    if body.len() > MAX_FETCH_BYTES {
        body = &body[..MAX_FETCH_BYTES];
    }
    let ctype = ctype_header.unwrap_or_default();
    let text_lossy = String::from_utf8_lossy(body).into_owned();
    let (captured_html, captured_text) = if ctype.contains("html") || text_lossy.trim_start().starts_with('<') {
        let plain = html_to_plain_text(&text_lossy);
        (Some(text_lossy.clone()), plain)
    } else {
        (None, text_lossy.clone())
    };
    let digest = sha256_bytes(captured_text.as_bytes());
    CapturedUrlEvidence {
        source_url: url.to_string(),
        captured_html,
        captured_text,
        content_sha256: digest,
    }
}

fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    let d = Sha256::digest(data);
    let mut out = [0u8; 32];
    out.copy_from_slice(&d);
    out
}

pub fn evidence_http_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(FETCH_TIMEOUT_SECS))
        .user_agent("256.app/1.0 judicial-link-evidence (+https://256.app)")
        .build()
}

/// Insert captured rows inside an open transaction (same unit of work as the case insert).
pub async fn insert_captured_rows_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    judicial_case_id: u64,
    rows: &[CapturedUrlEvidence],
    user_id: u64,
) -> Result<(), sqlx::Error> {
    for row in rows {
        sqlx::query(
            r#"
            INSERT INTO judicial_case_evidence (
                judicial_case_id,
                source_url,
                captured_html,
                captured_text,
                screenshot_object_key,
                content_sha256,
                created_by,
                modified_by
            )
            VALUES (?, ?, ?, ?, NULL, ?, ?, ?)
            "#,
        )
        .bind(judicial_case_id)
        .bind(&row.source_url)
        .bind(row.captured_html.as_deref())
        .bind(&row.captured_text)
        .bind(Vec::from(row.content_sha256))
        .bind(user_id)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}
