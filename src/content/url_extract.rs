//! Extract http(s) URLs from plain text and markup (posts, judicial evidence).

use reqwest::Url;

/// Scan text for `http://` / `https://` URLs (whitespace- and delimiter-bounded).
#[must_use]
pub fn extract_urls_from_text(s: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut search_from = 0usize;
    while search_from < s.len() {
        if search_from + 7 > s.len() {
            break;
        }
        let rest = &s[search_from..];
        let rel = match (rest.find("http://"), rest.find("https://")) {
            (Some(h), Some(hs)) => Some(h.min(hs)),
            (Some(h), None) => Some(h),
            (None, Some(hs)) => Some(hs),
            (None, None) => break,
        };
        let rel = rel.unwrap_or(0);
        let start = search_from + rel;
        let tail = &s[start..];
        let mut end = start;
        for (idx, c) in tail.char_indices() {
            if idx == 0 {
                end = start + c.len_utf8();
                continue;
            }
            if c.is_whitespace()
                || matches!(
                    c,
                    '"' | '\'' | '<' | '>' | ')' | ']' | '`' | '{' | '}' | ',' | ';'
                )
            {
                break;
            }
            end = start + idx + c.len_utf8();
        }
        let slice = &s[start..end];
        if let Some(n) = normalize_public_http_url(slice) {
            found.push(n);
        }
        search_from = end.max(start + 1);
    }
    found
}

#[must_use]
pub fn normalize_public_http_url(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.len() > 2048 {
        return None;
    }
    let url = Url::parse(t).ok()?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return None;
    }
    Some(url.as_str().trim_end_matches('/').to_string())
}

/// True when `href` is on this deployment's public host (help/login/legal links), not an off-site share.
#[must_use]
pub fn is_platform_self_link(href: &str, public_base_url: &str) -> bool {
    let href_url = Url::parse(href.trim()).ok();
    let base_url = Url::parse(public_base_url.trim_end_matches('/')).ok();
    match (href_url, base_url) {
        (Some(h), Some(b)) => h.scheme() == b.scheme() && h.host_str() == b.host_str(),
        _ => false,
    }
}

/// Resolve `raw` against `base_url` when relative; keep only absolute http(s) URLs for OG images.
#[must_use]
pub fn sanitize_http_image_url(raw: &str, base_url: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 2048 {
        return None;
    }
    let resolved = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        Url::parse(base_url)
            .ok()?
            .join(trimmed)
            .ok()?
            .to_string()
    };
    let url = Url::parse(&resolved).ok()?;
    match url.scheme() {
        "http" | "https" => Some(url.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_urls_from_mixed_text() {
        let urls = extract_urls_from_text("See https://example.com/path and http://foo.bar");
        assert_eq!(urls.len(), 2);
        assert!(urls[0].contains("example.com"));
    }

    #[test]
    fn stops_at_whitespace_and_punctuation() {
        let urls = extract_urls_from_text("Link: https://example.com/page, next.");
        assert_eq!(urls.len(), 1);
        assert!(urls[0].contains("example.com/page"));
    }

    #[test]
    fn extracts_url_before_unicode_punctuation() {
        let urls =
            extract_urls_from_text("BBC story https://www.bbc.com/news — discuss");
        assert_eq!(urls.len(), 1);
        assert!(urls[0].contains("bbc.com/news"));
    }

    #[test]
    fn sanitize_rejects_non_http_schemes() {
        assert!(sanitize_http_image_url("javascript:alert(1)", "https://example.com").is_none());
        assert!(sanitize_http_image_url("data:image/png;base64,abc", "https://example.com").is_none());
    }

    #[test]
    fn sanitize_resolves_relative_against_base() {
        let u = sanitize_http_image_url("/img/preview.jpg", "https://example.com/story")
            .expect("relative");
        assert_eq!(u, "https://example.com/img/preview.jpg");
    }

    #[test]
    fn platform_self_link_matches_public_host() {
        assert!(is_platform_self_link(
            "https://beta.twofivesix.app/login?legal=how",
            "https://beta.twofivesix.app"
        ));
        assert!(!is_platform_self_link(
            "https://example.com/story",
            "https://beta.twofivesix.app"
        ));
    }
}
