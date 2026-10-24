//! Shared Open Graph fetch + HTML meta extraction (human preview + council shares).

use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use reqwest::Url;
use sqlx::MySqlPool;

use crate::error::{AppError, AppResult};

use super::links::{self, FetchOutcome, FetchStatus, OpenGraphSnapshot};
use super::og_text::normalize_og_field_optional;

pub const OG_FETCH_USER_AGENT: &str = "256.app/og-preview (+https://256.app)";
const OG_FETCH_TIMEOUT_SECS: u64 = 6;

#[must_use]
pub fn extract_attr_value(tag: &str, attr: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let needle = format!("{attr}=");
    let idx = lower.find(&needle)?;
    let raw = &tag[idx + needle.len()..];
    let mut chars = raw.chars();
    let quote = chars.next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &raw[quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(rest[..end].trim().to_string())
}

#[must_use]
pub fn extract_meta_content(html: &str, key_attr: &str, key_val: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let mut from = 0usize;
    let key_pair = format!(r#"{key_attr}="{key_val}""#);
    let key_pair_sq = format!(r#"{key_attr}='{key_val}'"#);
    while let Some(rel) = lower[from..].find("<meta") {
        let start = from + rel;
        let end_rel = lower[start..].find('>')?;
        let end = start + end_rel + 1;
        let tag_l = &lower[start..end];
        if tag_l.contains(&key_pair) || tag_l.contains(&key_pair_sq) {
            let tag = &html[start..end];
            if let Some(v) = extract_attr_value(tag, "content") {
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
        from = end;
    }
    None
}

#[must_use]
pub fn youtube_watch_id_from_url(url: &str) -> Option<String> {
    let u = Url::parse(url).ok()?;
    let host = u.host_str()?.to_ascii_lowercase();
    if host != "youtube.com" && host != "www.youtube.com" && host != "m.youtube.com" {
        return None;
    }
    if u.path() != "/watch" {
        return None;
    }
    for (k, v) in u.query_pairs() {
        if k == "v" {
            let vv = v.trim();
            if !vv.is_empty() {
                return Some(vv.to_string());
            }
            break;
        }
    }
    None
}

#[must_use]
pub fn parse_og_from_html(html: &str, url_canonical: &str) -> OpenGraphSnapshot {
    let mut og_image_url = extract_meta_content(html, "property", "og:image");
    if og_image_url.is_none() {
        if let Some(video_id) = youtube_watch_id_from_url(url_canonical) {
            og_image_url = Some(format!("https://i.ytimg.com/vi/{video_id}/hqdefault.jpg"));
        }
    }
    let image_url = og_image_url
        .as_deref()
        .and_then(|u| crate::content::url_extract::sanitize_http_image_url(u, url_canonical));
    OpenGraphSnapshot {
        title: extract_meta_content(html, "property", "og:title"),
        description: extract_meta_content(html, "property", "og:description")
            .or_else(|| extract_meta_content(html, "name", "description")),
        image_url,
        site_name: extract_meta_content(html, "property", "og:site_name"),
        og_type: extract_meta_content(html, "property", "og:type"),
        og_url: extract_meta_content(html, "property", "og:url"),
        og_locale: extract_meta_content(html, "property", "og:locale"),
    }
}

#[derive(Debug, Clone)]
pub struct UrlFetchResult {
    pub html: Option<String>,
    pub http_status: Option<u16>,
    pub content_type: Option<String>,
    pub fetch_error: Option<String>,
}

pub async fn fetch_url_html(url: &str) -> UrlFetchResult {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(OG_FETCH_TIMEOUT_SECS))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return UrlFetchResult {
                html: None,
                http_status: None,
                content_type: None,
                fetch_error: Some(format!("client_init:{e}")),
            };
        }
    };
    match client
        .get(url)
        .header(reqwest::header::USER_AGENT, OG_FETCH_USER_AGENT)
        .header(reqwest::header::ACCEPT_ENCODING, "identity")
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let content_type = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            let text = resp.text().await.ok();
            UrlFetchResult {
                html: text,
                http_status: Some(status),
                content_type,
                fetch_error: None,
            }
        }
        Err(e) => UrlFetchResult {
            html: None,
            http_status: None,
            content_type: None,
            fetch_error: Some(e.to_string()),
        },
    }
}

#[must_use]
pub fn http_status_to_fetch_status(status: u16) -> FetchStatus {
    match status {
        200..=299 => FetchStatus::Ok,
        404 => FetchStatus::NotFound,
        410 => FetchStatus::Gone,
        _ => FetchStatus::Error,
    }
}

#[must_use]
fn response_body_is_html(content_type: Option<&str>) -> bool {
    content_type
        .map(|ct| {
            let ct = ct.to_ascii_lowercase();
            ct.contains("text/html") || ct.contains("application/xhtml")
        })
        .unwrap_or(true)
}

#[must_use]
pub fn extract_html_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let tag_end = lower[start..].find('>')? + start + 1;
    let end = lower[tag_end..].find("</title")? + tag_end;
    let title = html[tag_end..end].trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

#[must_use]
pub fn text_looks_like_error_page(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return false;
    }
    let lower = t.to_ascii_lowercase();
    const NEEDLES: &[&str] = &[
        "404",
        "not found",
        "page not found",
        "page unavailable",
        "page doesn't exist",
        "page does not exist",
        "couldn't find",
        "could not find",
        "could not be found",
        "couldn't be found",
        "no longer available",
        "access denied",
        "forbidden",
        "unauthorized",
        "bad gateway",
        "service unavailable",
        "internal server error",
        "error occurred",
        "something went wrong",
    ];
    NEEDLES.iter().any(|needle| lower.contains(needle))
}

#[must_use]
pub fn page_looks_like_error_page(html: &str, og: &OpenGraphSnapshot) -> bool {
    if og
        .title
        .as_deref()
        .filter(|t| !t.trim().is_empty())
        .is_some_and(text_looks_like_error_page)
    {
        return true;
    }
    if og
        .description
        .as_deref()
        .filter(|d| !d.trim().is_empty())
        .is_some_and(text_looks_like_error_page)
    {
        return true;
    }
    extract_html_title(html)
        .as_deref()
        .is_some_and(text_looks_like_error_page)
}

#[must_use]
pub fn build_fetch_outcome(
    url_canonical: &str,
    http_status: Option<u16>,
    html: Option<&str>,
    content_type: Option<&str>,
    fetch_error: Option<String>,
    fetched_at: chrono::DateTime<Utc>,
) -> FetchOutcome {
    let retry_hours = |h: i64| fetched_at + ChronoDuration::hours(h);
    let retry_days = |d: i64| fetched_at + ChronoDuration::days(d);

    let Some(status_code) = http_status else {
        return FetchOutcome {
            status: FetchStatus::Error,
            http_status: None,
            etag: None,
            last_modified_header: None,
            fetch_error: fetch_error.or_else(|| Some("og_fetch_failed".to_string())),
            fetched_at,
            next_recrawl_at: retry_hours(6),
            og: OpenGraphSnapshot::default(),
        };
    };

    let mut fetch_status = http_status_to_fetch_status(status_code);
    let Some(doc) = html.filter(|_| fetch_status == FetchStatus::Ok) else {
        let status = if fetch_status == FetchStatus::Ok {
            FetchStatus::Error
        } else {
            fetch_status
        };
        return FetchOutcome {
            status,
            http_status: Some(status_code),
            etag: None,
            last_modified_header: None,
            fetch_error: if fetch_status == FetchStatus::Ok {
                Some("empty_body".to_string())
            } else {
                fetch_error.or_else(|| Some(format!("http_{status_code}")))
            },
            fetched_at,
            next_recrawl_at: if status == FetchStatus::Ok {
                retry_days(7)
            } else {
                retry_hours(6)
            },
            og: OpenGraphSnapshot::default(),
        };
    };

    if !response_body_is_html(content_type) {
        return FetchOutcome {
            status: FetchStatus::Error,
            http_status: Some(status_code),
            etag: None,
            last_modified_header: None,
            fetch_error: Some("non_html_content".to_string()),
            fetched_at,
            next_recrawl_at: retry_hours(6),
            og: OpenGraphSnapshot::default(),
        };
    }

    let mut og = parse_og_from_html(doc, url_canonical);
    normalize_og_snapshot(&mut og);
    if page_looks_like_error_page(doc, &og) {
        fetch_status = FetchStatus::NotFound;
        return FetchOutcome {
            status: fetch_status,
            http_status: Some(status_code),
            etag: None,
            last_modified_header: None,
            fetch_error: Some("error_page_heuristic".to_string()),
            fetched_at,
            next_recrawl_at: retry_days(7),
            og: OpenGraphSnapshot::default(),
        };
    }

    FetchOutcome {
        status: fetch_status,
        http_status: Some(status_code),
        etag: None,
        last_modified_header: None,
        fetch_error: None,
        fetched_at,
        next_recrawl_at: retry_days(7),
        og,
    }
}

/// Apply normalization to all OG text fields in place.
pub fn normalize_og_snapshot(og: &mut OpenGraphSnapshot) {
    og.title = normalize_og_field_optional(og.title.as_deref());
    og.description = normalize_og_field_optional(og.description.as_deref());
    og.site_name = normalize_og_field_optional(og.site_name.as_deref());
}

#[must_use]
pub fn is_homepage_url(url: &str) -> bool {
    let Ok(u) = Url::parse(url) else {
        return true;
    };
    let path = u.path();
    path.is_empty() || path == "/"
}

/// Title + description required for council share commentary (not hostname-only stubs).
#[must_use]
pub fn og_snapshot_is_shareable(og: &OpenGraphSnapshot, registrable_domain: &str) -> bool {
    let title = og.title.as_deref().unwrap_or("").trim();
    let description = og.description.as_deref().unwrap_or("").trim();
    !title.is_empty()
        && !description.is_empty()
        && !title.eq_ignore_ascii_case(registrable_domain)
        && !text_looks_like_error_page(title)
        && !text_looks_like_error_page(description)
}

/// Fetch live OG metadata and persist via [`links::record_fetch_outcome`].
pub async fn fetch_and_record_og_for_link(
    pool: &MySqlPool,
    link_id: u64,
    url_canonical: &str,
    acting_user_id: Option<u64>,
) -> AppResult<FetchOutcome> {
    let fetched = fetch_url_html(url_canonical).await;
    let now = Utc::now();
    let outcome = build_fetch_outcome(
        url_canonical,
        fetched.http_status,
        fetched.html.as_deref(),
        fetched.content_type.as_deref(),
        fetched.fetch_error,
        now,
    );
    links::record_fetch_outcome(pool, link_id, &outcome, acting_user_id).await?;
    Ok(outcome)
}

#[derive(Debug, Clone)]
pub struct LinkOgRow {
    pub link_id: u64,
    pub url_canonical: String,
    pub domain: String,
    pub og_title: Option<String>,
    pub og_description: Option<String>,
    pub og_site_name: Option<String>,
    pub og_image_url: Option<String>,
    pub fetch_status: String,
}

pub async fn load_link_og_row(pool: &MySqlPool, link_id: u64) -> AppResult<Option<LinkOgRow>> {
    let row = sqlx::query_as::<_, (u64, String, String, Option<String>, Option<String>, Option<String>, Option<String>, String)>(
        r#"
        SELECT
            cl.id,
            cl.url_canonical,
            cd.domain,
            cl.og_title,
            cl.og_description,
            cl.og_site_name,
            cl.og_image_url,
            cl.fetch_status
        FROM content_link cl
        INNER JOIN content_domain cd ON cd.id = cl.domain_id
        WHERE cl.id = ?
        LIMIT 1
        "#,
    )
    .bind(link_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Db)?;

    Ok(row.map(
        |(link_id, url_canonical, domain, og_title, og_description, og_site_name, og_image_url, fetch_status)| {
            LinkOgRow {
                link_id,
                url_canonical,
                domain,
                og_title,
                og_description,
                og_site_name,
                og_image_url,
                fetch_status,
            }
        },
    ))
}

#[must_use]
pub fn link_row_needs_og_refresh(row: &LinkOgRow) -> bool {
    if row.fetch_status != "ok" {
        return true;
    }
    let og = OpenGraphSnapshot {
        title: row.og_title.clone(),
        description: row.og_description.clone(),
        image_url: None,
        site_name: row.og_site_name.clone(),
        og_type: None,
        og_url: None,
        og_locale: None,
    };
    !og_snapshot_is_shareable(&og, &row.domain)
}

/// Ensure link has fresh, shareable OG metadata; returns false when fetch/metadata inadequate.
///
/// When `force_live` is true (council shares), always HTTP-fetches before deciding — stale
/// `fetch_status = ok` rows from an earlier crawl cannot skip verification.
pub async fn ensure_shareable_og_for_link(
    pool: &MySqlPool,
    link_id: u64,
    acting_user_id: Option<u64>,
    force_live: bool,
) -> AppResult<bool> {
    let Some(mut row) = load_link_og_row(pool, link_id).await? else {
        return Ok(false);
    };
    if force_live || link_row_needs_og_refresh(&row) {
        let outcome =
            fetch_and_record_og_for_link(pool, link_id, &row.url_canonical, acting_user_id).await?;
        if outcome.status != FetchStatus::Ok {
            return Ok(false);
        }
        row = load_link_og_row(pool, link_id)
            .await?
            .unwrap_or(row);
    }
    let og = OpenGraphSnapshot {
        title: row.og_title.clone(),
        description: row.og_description.clone(),
        image_url: None,
        site_name: row.og_site_name.clone(),
        og_type: None,
        og_url: None,
        og_locale: None,
    };
    Ok(row.fetch_status == "ok" && og_snapshot_is_shareable(&og, &row.domain))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fixture_og_title_and_description() {
        let html = r#"<!doctype html><html><head>
            <meta property="og:title" content="Example Headline" />
            <meta property="og:description" content="A specific article angle." />
            <meta property="og:site_name" content="Example News" />
            </head><body></body></html>"#;
        let og = parse_og_from_html(html, "https://example.com/story");
        assert_eq!(og.title.as_deref(), Some("Example Headline"));
        assert_eq!(og.description.as_deref(), Some("A specific article angle."));
        assert_eq!(og.site_name.as_deref(), Some("Example News"));
    }

    #[test]
    fn homepage_detection() {
        assert!(is_homepage_url("https://bbc.com/"));
        assert!(is_homepage_url("https://www.un.org/"));
        assert!(!is_homepage_url("https://bbc.com/news/world-123"));
    }

    #[test]
    fn shareable_og_rejects_hostname_title_only() {
        let og = OpenGraphSnapshot {
            title: Some("bbc.com".into()),
            description: Some("Real description".into()),
            ..Default::default()
        };
        assert!(!og_snapshot_is_shareable(&og, "bbc.com"));
    }

    #[test]
    fn http_status_mapping() {
        assert_eq!(http_status_to_fetch_status(200), FetchStatus::Ok);
        assert_eq!(http_status_to_fetch_status(404), FetchStatus::NotFound);
        assert_eq!(http_status_to_fetch_status(410), FetchStatus::Gone);
        assert_eq!(http_status_to_fetch_status(500), FetchStatus::Error);
        assert_eq!(http_status_to_fetch_status(403), FetchStatus::Error);
    }

    #[test]
    fn error_page_heuristics() {
        assert!(text_looks_like_error_page("404 Not Found"));
        assert!(text_looks_like_error_page("Page not found - Example"));
        assert!(text_looks_like_error_page("Sorry, we could not find that page."));
        assert!(!text_looks_like_error_page("A real headline about policy"));
    }

    #[test]
    fn shareable_og_rejects_error_description() {
        let og = OpenGraphSnapshot {
            title: Some("Example News".into()),
            description: Some("The page you requested could not be found.".into()),
            ..Default::default()
        };
        assert!(!og_snapshot_is_shareable(&og, "example.com"));
    }

    #[test]
    fn build_outcome_normalizes_entities_in_og_fields() {
        let html = r#"<!doctype html><html><head>
            <meta property="og:title" content="Tom &amp; Jerry" />
            <meta property="og:description" content="It&#8217;s a story." />
            <meta property="og:site_name" content="Example News" />
            </head><body></body></html>"#;
        let now = Utc::now();
        let outcome = build_fetch_outcome(
            "https://example.com/story",
            Some(200),
            Some(html),
            Some("text/html"),
            None,
            now,
        );
        assert_eq!(outcome.og.title.as_deref(), Some("Tom & Jerry"));
        assert_eq!(outcome.og.description.as_deref(), Some("It's a story."));
    }

    #[test]
    fn build_outcome_rejects_http_404_even_with_og_tags() {
        let html = r#"<!doctype html><html><head>
            <title>404 Not Found</title>
            <meta property="og:title" content="404 Not Found" />
            <meta property="og:description" content="We could not find that page." />
            </head><body></body></html>"#;
        let now = Utc::now();
        let outcome = build_fetch_outcome(
            "https://example.com/missing",
            Some(404),
            Some(html),
            Some("text/html"),
            None,
            now,
        );
        assert_eq!(outcome.status, FetchStatus::NotFound);
        assert!(outcome.og.title.is_none());
    }

    #[test]
    fn build_outcome_rejects_soft_404_on_200() {
        let html = r#"<!doctype html><html><head>
            <title>Page not found</title>
            <meta property="og:title" content="Page not found" />
            <meta property="og:description" content="Try the homepage." />
            </head><body></body></html>"#;
        let now = Utc::now();
        let outcome = build_fetch_outcome(
            "https://example.com/ghost",
            Some(200),
            Some(html),
            Some("text/html"),
            None,
            now,
        );
        assert_eq!(outcome.status, FetchStatus::NotFound);
        assert_eq!(outcome.fetch_error.as_deref(), Some("error_page_heuristic"));
    }

    #[test]
    fn build_outcome_accepts_real_article_on_200() {
        let html = r#"<!doctype html><html><head>
            <meta property="og:title" content="Example Headline" />
            <meta property="og:description" content="A specific article angle." />
            </head><body></body></html>"#;
        let now = Utc::now();
        let outcome = build_fetch_outcome(
            "https://example.com/story",
            Some(200),
            Some(html),
            Some("text/html"),
            None,
            now,
        );
        assert_eq!(outcome.status, FetchStatus::Ok);
        assert_eq!(outcome.og.title.as_deref(), Some("Example Headline"));
    }
}
