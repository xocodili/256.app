//! Transient article text extraction for council share read stage (never persisted).

use std::time::Duration;

use sqlx::MySqlPool;

use super::og_fetch::{fetch_url_html, text_looks_like_error_page};

pub const SUMMARY_CAP_CHARS: usize = 1200;
const EXTRACT_TIMEOUT_SECS: u64 = 12;
const MIN_OPEN_BODY_CHARS: usize = 280;

#[derive(Debug, Clone)]
pub struct ArticleExtract {
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtractFailure {
    Paywalled,
    FetchFailed,
    TooShort,
    ErrorPage,
}

/// Fetch URL and return a capped plain-text summary (in memory only).
pub async fn fetch_open_article_summary(url: &str) -> Result<ArticleExtract, ExtractFailure> {
    let fetched = fetch_url_html_with_timeout(url).await;
    let status = fetched.http_status.unwrap_or(0);
    if status == 402 || status == 451 {
        return Err(ExtractFailure::Paywalled);
    }
    let html = fetched
        .html
        .as_deref()
        .filter(|_| (200..300).contains(&status))
        .ok_or(ExtractFailure::FetchFailed)?;

    if html_indicates_paywall(html) {
        return Err(ExtractFailure::Paywalled);
    }

    let text = extract_main_text(html);
    if text.len() < MIN_OPEN_BODY_CHARS {
        if html_indicates_paywall(html) || status == 403 {
            return Err(ExtractFailure::Paywalled);
        }
        return Err(ExtractFailure::TooShort);
    }
    if text_looks_like_error_page(&text) {
        return Err(ExtractFailure::ErrorPage);
    }

    let summary = truncate_summary(&text, SUMMARY_CAP_CHARS);
    if summary.trim().len() < 80 {
        return Err(ExtractFailure::TooShort);
    }
    Ok(ArticleExtract { summary })
}

async fn fetch_url_html_with_timeout(url: &str) -> super::og_fetch::UrlFetchResult {
    let url = url.to_string();
    tokio::time::timeout(
        Duration::from_secs(EXTRACT_TIMEOUT_SECS),
        super::og_fetch::fetch_url_html(&url),
    )
    .await
    .unwrap_or(super::og_fetch::UrlFetchResult {
        html: None,
        http_status: None,
        content_type: None,
        fetch_error: Some("extract_timeout".into()),
    })
}

#[must_use]
pub fn html_indicates_paywall(html: &str) -> bool {
    let lower = html.to_ascii_lowercase();
    const NEEDLES: &[&str] = &[
        "subscribe to continue",
        "subscription required",
        "subscriber-only",
        "subscriber only",
        "sign in to read",
        "sign in to continue",
        "register to continue",
        "paywall",
        "premium content",
        "members-only",
        "members only",
        "you've reached your limit",
        "enable javascript to continue",
        "data-paywall",
        "class=\"paywall",
        "id=\"paywall",
    ];
    NEEDLES.iter().any(|n| lower.contains(n))
}

#[must_use]
pub fn extract_main_text(html: &str) -> String {
    let article = extract_tag_inner(html, "article");
    let main = extract_tag_inner(html, "main");
    let candidate = article
        .as_deref()
        .filter(|s| s.len() > 200)
        .or(main.as_deref().filter(|s| s.len() > 200))
        .unwrap_or(html);
    strip_to_plain_text(candidate)
}

fn extract_tag_inner(html: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let lower = html.to_ascii_lowercase();
    let start = lower.find(&open)?;
    let content_start = lower[start..].find('>')? + start + 1;
    let end = lower[content_start..].find(&close)? + content_start;
    Some(html[content_start..end].to_string())
}

fn strip_to_plain_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut tag_buf = String::new();
    for ch in html.chars() {
        match ch {
            '<' => {
                in_tag = true;
                tag_buf.clear();
            }
            '>' => {
                in_tag = false;
                let tag_l = tag_buf.to_ascii_lowercase();
                if tag_l == "p"
                    || tag_l == "br"
                    || tag_l == "div"
                    || tag_l == "li"
                    || tag_l.starts_with("/p")
                    || tag_l.starts_with("/div")
                    || tag_l.starts_with("/li")
                {
                    out.push('\n');
                }
                tag_buf.clear();
            }
            _ if in_tag => tag_buf.push(ch),
            _ => out.push(ch),
        }
    }
    super::og_text::normalize_og_field(&out)
}

fn truncate_summary(text: &str, cap: usize) -> String {
    if text.chars().count() <= cap {
        return text.to_string();
    }
    text.chars().take(cap).collect::<String>() + "…"
}

/// Persist extract status on content_link (not the text).
pub async fn mark_link_extract_status(
    pool: &MySqlPool,
    link_id: u64,
    status: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE content_link
        SET extract_status = ?, updated_at = CURRENT_TIMESTAMP
        WHERE id = ?
        "#,
    )
    .bind(status)
    .bind(link_id)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_article_extracts_paragraphs() {
        let html = r#"<html><body><article><p>First paragraph with enough words to pass the minimum length gate for extraction testing purposes here.</p><p>Second paragraph adds more readable body text for the council share read stage summary builder.</p></article></body></html>"#;
        let text = extract_main_text(html);
        assert!(text.contains("First paragraph"));
        assert!(text.contains("Second paragraph"));
        assert!(!html_indicates_paywall(html));
    }

    #[test]
    fn paywall_shell_detected() {
        let html = r#"<html><body><div class="paywall"><p>Subscribe to continue reading this article.</p></div></body></html>"#;
        assert!(html_indicates_paywall(html));
    }

    #[test]
    fn error_page_text_rejected() {
        let html = r#"<html><body><h1>404 Not Found</h1><p>The page you requested could not be found on this server.</p></body></html>"#;
        let text = extract_main_text(html);
        assert!(text_looks_like_error_page(&text));
    }
}
