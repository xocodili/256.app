//! Bounded article URL discovery for portfolio hosts without RSS feeds.

use reqwest::Url;
use sqlx::MySqlPool;

use crate::content::links::{self, canonicalize_url};
use crate::content::og_fetch::{self, is_homepage_url};
use crate::error::AppError;

pub async fn discover_article_urls_for_domain(
    pool: &MySqlPool,
    domain_id: u64,
    limit: u32,
    acting_user_id: Option<u64>,
) -> Result<u32, String> {
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT domain FROM content_domain WHERE id = ? LIMIT 1",
    )
    .bind(domain_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("content_domain {domain_id} not found"))?;

    let domain = row.0;
    let base_hosts = base_host_candidates(&domain);
    let mut discovered = Vec::new();

    for host in &base_hosts {
        let robots = fetch_robots_txt(host).await;
        if robots_disallows_all(&robots) {
            continue;
        }

        for sitemap_url in sitemap_candidates(host) {
            if discovered.len() >= limit as usize {
                break;
            }
            if robots_disallows_path(&robots, &sitemap_url) {
                continue;
            }
            let fetched = og_fetch::fetch_url_html(&sitemap_url).await;
            if fetched.http_status != Some(200) {
                continue;
            }
            if let Some(body) = fetched.html.as_deref() {
                for url in parse_sitemap_locs(body) {
                    if article_like_url(&url, &domain) {
                        push_unique(&mut discovered, url);
                    }
                    if discovered.len() >= limit as usize {
                        break;
                    }
                }
            }
        }

        if discovered.len() >= limit as usize {
            break;
        }

        let homepage = format!("https://{host}/");
        if !robots_disallows_path(&robots, &homepage) {
            if let Some(html) = fetch_html_ok(&homepage).await {
                for raw in extract_hrefs_from_html(&html) {
                    if let Some(abs) = resolve_same_domain_url(&raw, &homepage, &domain) {
                        if article_like_url(&abs, &domain) {
                            push_unique(&mut discovered, abs);
                        }
                    }
                    if discovered.len() >= limit as usize {
                        break;
                    }
                }
            }
        }
    }

    let mut upserted = 0u32;
    for url in discovered.into_iter().take(limit as usize) {
        let canonical = match canonicalize_url(&url) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if canonical.registrable_domain != domain {
            continue;
        }
        let upsert = links::upsert_canonical_link(pool, &canonical, acting_user_id)
            .await
            .map_err(|e: AppError| e.to_string())?;
        let _ = og_fetch::fetch_and_record_og_for_link(
            pool,
            upsert.link_id,
            &canonical.url_canonical,
            acting_user_id,
        )
        .await;
        upserted += 1;
    }
    Ok(upserted)
}

async fn fetch_html_ok(url: &str) -> Option<String> {
    let fetched = og_fetch::fetch_url_html(url).await;
    fetched
        .html
        .filter(|_| fetched.http_status == Some(200))
}

fn base_host_candidates(domain: &str) -> Vec<String> {
    vec![
        domain.to_string(),
        format!("www.{domain}"),
    ]
}

fn sitemap_candidates(host: &str) -> Vec<String> {
    vec![
        format!("https://{host}/sitemap.xml"),
        format!("https://{host}/sitemap_index.xml"),
    ]
}

async fn fetch_robots_txt(host: &str) -> String {
    let url = format!("https://{host}/robots.txt");
    let fetched = og_fetch::fetch_url_html(&url).await;
    fetched.html.unwrap_or_default()
}

#[must_use]
pub fn parse_sitemap_locs(xml: &str) -> Vec<String> {
    let lower = xml.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find("<loc") {
        let start = from + rel;
        let end = match lower[start..].find("</loc>") {
            Some(i) => start + i,
            None => continue,
        };
        let block = &xml[start..end];
        if let Some(url) = loc_text(block) {
            push_unique(&mut out, url);
        }
        from = end + "</loc>".len();
    }
    out
}

fn loc_text(block: &str) -> Option<String> {
    let gt = block.find('>')? + 1;
    let text = block[gt..].trim();
    if text.starts_with("http://") || text.starts_with("https://") {
        Some(text.to_string())
    } else {
        None
    }
}

#[must_use]
pub fn article_like_url(url: &str, registrable_domain: &str) -> bool {
    if is_homepage_url(url) {
        return false;
    }
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    let domain = registrable_domain.to_ascii_lowercase();
    if !(host == domain || host.ends_with(&format!(".{domain}"))) {
        return false;
    }
    let path = parsed.path();
    path.len() > 1 && path.split('/').filter(|s| !s.is_empty()).count() >= 1
}

fn extract_hrefs_from_html(html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find("href=") {
        let start = from + rel + "href=".len();
        let quote = html.as_bytes().get(start).copied().unwrap_or(b' ');
        if quote != b'"' && quote != b'\'' {
            from = start;
            continue;
        }
        let rest = &html[start + 1..];
        let end = rest.find(quote as char).unwrap_or(rest.len());
        let href = rest[..end].trim();
        if !href.is_empty() && !href.starts_with('#') && !href.starts_with("mailto:") {
            out.push(href.to_string());
        }
        from = start + 1 + end;
    }
    out
}

fn resolve_same_domain_url(raw: &str, base: &str, domain: &str) -> Option<String> {
    let resolved = if raw.starts_with("http://") || raw.starts_with("https://") {
        raw.to_string()
    } else {
        Url::parse(base).ok()?.join(raw).ok()?.to_string()
    };
    let canonical = canonicalize_url(&resolved).ok()?;
    if canonical.registrable_domain == domain {
        Some(canonical.url_canonical)
    } else {
        None
    }
}

#[must_use]
pub fn robots_disallows_all(robots: &str) -> bool {
    robots_disallows_path(robots, "/")
}

#[must_use]
pub fn robots_disallows_path(robots: &str, url: &str) -> bool {
    let path = Url::parse(url)
        .ok()
        .map(|u| u.path().to_string())
        .unwrap_or_else(|| "/".to_string());
    let mut active = false;
    for line in robots.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("user-agent:") {
            let agent = lower["user-agent:".len()..].trim();
            active = agent == "*" || agent.contains("256.app");
            continue;
        }
        if active && lower.starts_with("disallow:") {
            let rule = line["disallow:".len()..].trim();
            if rule.is_empty() {
                continue;
            }
            if rule == "/" || path.starts_with(rule) {
                return true;
            }
        }
    }
    false
}

fn push_unique(out: &mut Vec<String>, url: String) {
    if !out.iter().any(|u| u == &url) {
        out.push(url);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sitemap_loc_extracted() {
        let xml = "<urlset><url><loc>https://news.com/story-1</loc></url></urlset>";
        assert_eq!(
            parse_sitemap_locs(xml),
            vec!["https://news.com/story-1".to_string()]
        );
    }

    #[test]
    fn article_like_rejects_homepage() {
        assert!(!article_like_url("https://bbc.com/", "bbc.com"));
        assert!(article_like_url(
            "https://bbc.com/news/world-123",
            "bbc.com"
        ));
    }

    #[test]
    fn robots_disallow_root() {
        let robots = "User-agent: *\nDisallow: /\n";
        assert!(robots_disallows_all(robots));
    }
}
