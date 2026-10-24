//! Contact resolution for council invite pipeline — fetch profile pages, extract verifiable emails.

use std::time::Duration;

use reqwest::Client;
use reqwest::Url;

use crate::content::links::{canonicalize_url, ensure_domain_allowed};
use crate::content::og_fetch::is_homepage_url;
use crate::content::web_search::SearchHit;
use crate::social::recruitment::parse_recipient_email;
use crate::social::recruitment_outreach::{self, PlatformRecruitmentPrecheck};

use super::council_invite_pipeline::StewardInviteArticle;

const FETCH_TIMEOUT_SECS: u64 = 8;
const MAX_PAGE_BYTES: usize = 512_000;

#[derive(Debug, Clone)]
pub struct ResolvedContact {
    pub email: String,
    pub verification_blob: String,
}

/// Case-insensitive substring match — email must appear verbatim in auditable fetched text.
#[must_use]
pub fn email_verifiable_in_blob(email: &str, blob: &str) -> bool {
    let Some(parsed) = parse_recipient_email(email) else {
        return false;
    };
    let needle = parsed.to_ascii_lowercase();
    if needle.is_empty() || !needle.contains('@') {
        return false;
    }
    blob.to_ascii_lowercase().contains(&needle)
}

#[must_use]
pub fn verification_blob_for_article(article: &StewardInviteArticle) -> String {
    let mut blob = String::new();
    blob.push_str(&article.link_url);
    blob.push(' ');
    blob.push_str(&article.body_plain);
    if let Some(t) = &article.og_title {
        blob.push(' ');
        blob.push_str(t);
    }
    if let Some(d) = &article.og_description {
        blob.push(' ');
        blob.push_str(d);
    }
    if let Some(s) = &article.og_site_name {
        blob.push(' ');
        blob.push_str(s);
    }
    blob
}

#[must_use]
pub fn extract_emails_from_html(html: &str) -> Vec<String> {
    let mut found = Vec::new();
    for token in split_email_tokens(html) {
        if is_blocked_local_part(&token) {
            continue;
        }
        if let Some(email) = parse_recipient_email(&token) {
            if !found.iter().any(|e: &String| e.eq_ignore_ascii_case(&email)) {
                found.push(email);
            }
        }
    }
    found
}

fn split_email_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for word in text.split(|c: char| {
        c.is_whitespace() || c == '"' || c == '\'' || c == '>' || c == '<' || c == '(' || c == ')'
    }) {
        let w = word.trim().trim_matches(',').trim_end_matches('.');
        if w.contains('@') {
            tokens.push(w.to_string());
        }
    }
    let lower = text.to_ascii_lowercase();
    for prefix in ["mailto:", "mailto%3a"] {
        let mut from = 0usize;
        while let Some(rel) = lower[from..].find(prefix) {
            let start = from + rel + prefix.len();
            let rest = &text[start..];
            let end = rest
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '>' || c == '?')
                .unwrap_or(rest.len().min(120));
            tokens.push(rest[..end].trim().trim_end_matches('.').to_string());
            from = start + 1;
        }
    }
    tokens
}

fn is_blocked_local_part(email: &str) -> bool {
    let lower = email.to_ascii_lowercase();
    let local = lower.split('@').next().unwrap_or("");
    local.contains("noreply")
        || local.contains("no-reply")
        || local.contains("donotreply")
        || local.contains("newsletter")
        || local.contains("mailer-daemon")
        || local.starts_with("postmaster@")
        || local.starts_with("abuse@")
}

pub async fn fetch_page_text(url: &str) -> Option<String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(FETCH_TIMEOUT_SECS))
        .user_agent(crate::content::og_fetch::OG_FETCH_USER_AGENT)
        .build()
        .ok()?;
    let resp = client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let bytes = resp.bytes().await.ok()?;
    let slice = if bytes.len() > MAX_PAGE_BYTES {
        &bytes[..MAX_PAGE_BYTES]
    } else {
        &bytes
    };
    Some(String::from_utf8_lossy(slice).into_owned())
}

/// Normalize URL for search-hit / profile grounding comparison (lowercase host, trim trailing slash).
#[must_use]
pub fn normalize_url_for_match(url: &str) -> Option<String> {
    let canonical = canonicalize_url(url.trim()).ok()?;
    let parsed = Url::parse(&canonical.url_canonical).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    let mut path = parsed.path().trim_end_matches('/').to_string();
    if path.is_empty() {
        path = "/".to_string();
    }
    Some(format!("https://{host}{path}"))
}

#[must_use]
pub fn url_matches_search_hits(profile_url: &str, hits: &[SearchHit]) -> bool {
    let Some(norm) = normalize_url_for_match(profile_url) else {
        return false;
    };
    hits.iter().any(|h| {
        normalize_url_for_match(&h.url)
            .is_some_and(|hit_norm| hit_norm == norm)
    })
}

#[must_use]
pub fn profile_url_on_article_host(profile_url: &str, article: &StewardInviteArticle) -> bool {
    let Ok(profile) = canonicalize_url(profile_url.trim()) else {
        return false;
    };
    let Ok(article_url) = canonicalize_url(&article.link_url) else {
        return false;
    };
    if profile.registrable_domain != article_url.registrable_domain {
        return false;
    }
    if is_homepage_url(&profile.url_canonical) {
        return false;
    }
    crate::content::domain_discover::article_like_url(&profile.url_canonical, &profile.registrable_domain)
}

#[must_use]
pub fn profile_url_in_article_blob(profile_url: &str, article: &StewardInviteArticle) -> bool {
    let blob = verification_blob_for_article(article).to_ascii_lowercase();
    let needle = profile_url.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return false;
    }
    blob.contains(&needle)
}

/// Whether a person-discover `profile_url` is grounded in search hits or article host context.
pub async fn profile_url_is_allowed(
    pool: &sqlx::MySqlPool,
    profile_url: &str,
    article: &StewardInviteArticle,
    search_hits: &[SearchHit],
    search_configured: bool,
) -> Result<bool, sqlx::Error> {
    if profile_url.trim().is_empty() || is_homepage_url(profile_url) {
        return Ok(false);
    }
    let canonical = match canonicalize_url(profile_url.trim()) {
        Ok(c) => c,
        Err(_) => return Ok(false),
    };
    if ensure_domain_allowed(
        pool,
        canonical.registrable_domain.as_str(),
        "council_invite_profile",
        None,
        Some(article.post_id),
    )
    .await
    .is_err()
    {
        return Ok(false);
    }

    let has_hits = search_configured && !search_hits.is_empty();
    if has_hits {
        return Ok(url_matches_search_hits(profile_url, search_hits));
    }

    Ok(profile_url_on_article_host(profile_url, article)
        || profile_url_in_article_blob(profile_url, article))
}

/// Resolve a verifiable email for an invite candidate.
///
/// Attempt order:
/// 1. Search snippets + article blob (no HTTP fetch)
/// 2. Profile page fetch + HTML extract (merged with snippets/article blob)
/// 3. Article OG/body blob alone
pub async fn resolve_email_for_candidate(
    pool: &sqlx::MySqlPool,
    profile_url: &str,
    article: &StewardInviteArticle,
    search_snippets: &[String],
) -> Result<Option<ResolvedContact>, sqlx::Error> {
    let article_blob = verification_blob_for_article(article);
    let snippet_blob = search_snippets.join(" ");
    let mut combined_blob = article_blob.clone();
    if !snippet_blob.is_empty() {
        combined_blob.push(' ');
        combined_blob.push_str(&snippet_blob);
    }

    for email in extract_emails_from_html(&combined_blob) {
        if !email_verifiable_in_blob(&email, &combined_blob) {
            continue;
        }
        if precheck_ok(pool, &email).await? {
            return Ok(Some(ResolvedContact {
                email,
                verification_blob: combined_blob.clone(),
            }));
        }
    }

    if let Some(html) = fetch_page_text(profile_url).await {
        let mut blob = combined_blob.clone();
        blob.push(' ');
        blob.push_str(&html);
        for email in extract_emails_from_html(&html) {
            if !email_verifiable_in_blob(&email, &blob) {
                continue;
            }
            if precheck_ok(pool, &email).await? {
                return Ok(Some(ResolvedContact {
                    email,
                    verification_blob: blob,
                }));
            }
        }
    }

    for email in extract_emails_from_html(&article_blob) {
        if !email_verifiable_in_blob(&email, &article_blob) {
            continue;
        }
        if precheck_ok(pool, &email).await? {
            return Ok(Some(ResolvedContact {
                email,
                verification_blob: article_blob,
            }));
        }
    }

    Ok(None)
}

async fn precheck_ok(pool: &sqlx::MySqlPool, email: &str) -> Result<bool, sqlx::Error> {
    Ok(matches!(
        recruitment_outreach::precheck_platform_recruitment_email(pool, email).await?,
        PlatformRecruitmentPrecheck::MaySend
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::web_search::SearchHit;

    fn sample_article(body: &str, title: Option<&str>) -> StewardInviteArticle {
        StewardInviteArticle {
            post_id: 1,
            content_link_id: 10,
            body_plain: body.to_string(),
            link_url: "https://example.com/story".to_string(),
            og_title: title.map(str::to_string),
            og_description: Some("A feature interview".to_string()),
            og_site_name: Some("Example News".to_string()),
        }
    }

    #[test]
    fn email_verifiable_in_og_body() {
        let article = sample_article(
            "Contact jane.doe@example.org for comments.",
            Some("Jane Doe on love"),
        );
        let blob = verification_blob_for_article(&article);
        assert!(email_verifiable_in_blob("jane.doe@example.org", &blob));
        assert!(!email_verifiable_in_blob("hallucinated@fake.test", &blob));
    }

    #[test]
    fn extract_skips_noreply() {
        let html = r#"<a href="mailto:noreply@example.com">x</a> hello@author.org"#;
        let emails = extract_emails_from_html(html);
        assert!(emails.iter().any(|e| e.contains("hello@author.org")));
        assert!(!emails.iter().any(|e| e.contains("noreply")));
    }

    #[test]
    fn negative_hallucination_not_in_blob() {
        let blob = "Story about Jane Doe with no address";
        assert!(!email_verifiable_in_blob("jane@doe.com", blob));
    }

    #[test]
    fn snippet_email_resolves_without_article_body() {
        let article = sample_article("No addresses in the article body.", None);
        let blob = verification_blob_for_article(&article);
        let snippet_blob = "Contact writer@press.org for interviews.";
        let combined = format!("{blob} {snippet_blob}");
        let emails = extract_emails_from_html(&combined);
        assert!(emails.iter().any(|e| e.contains("writer@press.org")));
        assert!(email_verifiable_in_blob("writer@press.org", &combined));
        assert!(!email_verifiable_in_blob("hallucinated@fake.test", &combined));
    }

    #[test]
    fn url_matches_search_hit_normalizes_trailing_slash() {
        let hits = vec![SearchHit {
            title: "Jane".into(),
            url: "https://Example.com/author/jane/".into(),
            snippet: String::new(),
        }];
        assert!(url_matches_search_hits(
            "https://example.com/author/jane",
            &hits
        ));
    }

    #[test]
    fn profile_url_on_article_host_rejects_homepage() {
        let article = sample_article("body", Some("t"));
        assert!(!profile_url_on_article_host("https://example.com/", &article));
        assert!(profile_url_on_article_host(
            "https://example.com/author/jane",
            &article
        ));
    }

    #[test]
    fn profile_url_rejected_when_not_in_hits() {
        let article = sample_article("body", Some("t"));
        let hits = vec![SearchHit {
            title: "Other".into(),
            url: "https://other.test/person".into(),
            snippet: String::new(),
        }];
        assert!(!url_matches_search_hits(
            "https://random.test/not-in-hits",
            &hits
        ));
        assert!(!profile_url_on_article_host(
            "https://other.test/person",
            &article
        ));
    }

    #[test]
    fn profile_url_fallback_same_host_when_no_search() {
        let article = sample_article("body", Some("t"));
        assert!(profile_url_on_article_host(
            "https://example.com/writers/jane-doe",
            &article
        ));
    }
}
