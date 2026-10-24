//! RSS/Atom feed polling for council portfolio article pools.

use std::time::Instant;

use chrono::Utc;
use sqlx::MySqlPool;

use crate::content::domain_discover;
use crate::content::links::{self, canonicalize_url};
use crate::content::og_fetch::{self, is_homepage_url};
use crate::error::AppError;
use crate::social::audit;

const FEED_ENTRY_LIMIT: usize = 48;
const MIN_ARTICLE_LINKS_BEFORE_CRAWL: i64 = 4;
const CRAWL_DISCOVER_LIMIT: u32 = 32;

#[derive(Debug, Default)]
pub struct FeedPollSummary {
    pub feeds_polled: u32,
    pub links_upserted: u32,
    pub domains_crawled: u32,
    pub errors: u32,
    pub feeds_skipped: u32,
    pub crawls_skipped: u32,
    pub budget_exhausted: bool,
}

/// Six-hour dedupe bucket for recurring `council_portfolio_feed_poll`.
#[must_use]
pub fn feed_poll_period_key() -> String {
    let now = Utc::now();
    let bucket = now.format("%Y%m%d").to_string();
    let hour_bucket = now.format("%H").to_string().parse::<u32>().unwrap_or(0) / 6;
    format!("{bucket}-h{hour_bucket}")
}

pub async fn run_portfolio_feed_poll(
    pool: &MySqlPool,
    wall_budget_secs: u64,
) -> Result<FeedPollSummary, String> {
    let system_user = audit::system_user_id(pool)
        .await
        .map_err(|e| e.to_string())?;

    let mut summary = FeedPollSummary::default();
    let budget = wall_budget_secs.max(1);
    let started = Instant::now();

    let feeds = sqlx::query_as::<_, FeedRow>(
        r#"
        SELECT cdf.id, cdf.feed_url, cdf.feed_type, cd.id AS domain_id, cd.domain
        FROM content_domain_feed cdf
        INNER JOIN content_domain cd ON cd.id = cdf.content_domain_id
        INNER JOIN ai_actor_content_domain pac ON pac.content_domain_id = cd.id
        WHERE cdf.is_active = 1
        GROUP BY cdf.id, cdf.feed_url, cdf.feed_type, cd.id, cd.domain
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    for feed in feeds {
        if poll_budget_exhausted(started, budget) {
            summary.budget_exhausted = true;
            summary.feeds_skipped += 1;
            continue;
        }
        summary.feeds_polled += 1;
        match poll_one_feed(pool, &feed, system_user).await {
            Ok(n) => summary.links_upserted += n,
            Err(err) => {
                summary.errors += 1;
                let _ = sqlx::query(
                    r#"
                    UPDATE content_domain_feed
                    SET last_polled_at = UTC_TIMESTAMP(6), last_error = ?
                    WHERE id = ?
                    "#,
                )
                .bind(err.as_str())
                .bind(feed.id)
                .execute(pool)
                .await;
                tracing::warn!(
                    feed_id = feed.id,
                    domain = %feed.domain,
                    error = %err,
                    "feed poll failed"
                );
            }
        }
    }

    let crawl_targets = sqlx::query_as::<_, DomainRow>(
        r#"
        SELECT cd.id, cd.domain
        FROM content_domain cd
        INNER JOIN ai_actor_content_domain pac ON pac.content_domain_id = cd.id
        LEFT JOIN content_domain_feed cdf
            ON cdf.content_domain_id = cd.id AND cdf.is_active = 1
        WHERE cdf.id IS NULL
        GROUP BY cd.id, cd.domain
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    for domain in crawl_targets {
        if poll_budget_exhausted(started, budget) {
            summary.budget_exhausted = true;
            summary.crawls_skipped += 1;
            continue;
        }

        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM content_link cl
            WHERE cl.domain_id = ?
              AND cl.fetch_status = 'ok'
              AND cl.url_canonical NOT LIKE '%://%/'
              AND cl.url_canonical NOT LIKE '%://%/#'
            "#,
        )
        .bind(domain.id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;

        if count >= MIN_ARTICLE_LINKS_BEFORE_CRAWL {
            continue;
        }

        summary.domains_crawled += 1;
        match domain_discover::discover_article_urls_for_domain(
            pool,
            domain.id,
            CRAWL_DISCOVER_LIMIT,
            system_user,
        )
        .await
        {
            Ok(n) => summary.links_upserted += n,
            Err(err) => {
                summary.errors += 1;
                tracing::warn!(
                    domain_id = domain.id,
                    domain = %domain.domain,
                    error = %err,
                    "domain discover failed"
                );
            }
        }
    }

    if summary.budget_exhausted {
        tracing::info!(
            budget_secs = budget,
            feeds_polled = summary.feeds_polled,
            feeds_skipped = summary.feeds_skipped,
            domains_crawled = summary.domains_crawled,
            crawls_skipped = summary.crawls_skipped,
            "feed poll stopped early — wall budget exhausted"
        );
    }

    Ok(summary)
}

fn poll_budget_exhausted(started: Instant, budget_secs: u64) -> bool {
    started.elapsed().as_secs() >= budget_secs
}

#[derive(Debug, sqlx::FromRow)]
struct FeedRow {
    id: u64,
    feed_url: String,
    feed_type: String,
    domain_id: u64,
    domain: String,
}

#[derive(Debug, sqlx::FromRow)]
struct DomainRow {
    id: u64,
    domain: String,
}

async fn poll_one_feed(
    pool: &MySqlPool,
    feed: &FeedRow,
    acting_user_id: Option<u64>,
) -> Result<u32, String> {
    let fetched = og_fetch::fetch_url_html(&feed.feed_url).await;
    let body = fetched
        .html
        .as_deref()
        .filter(|_| fetched.http_status.unwrap_or(0) == 200)
        .ok_or_else(|| {
            fetched
                .fetch_error
                .unwrap_or_else(|| format!("feed_http_{}", fetched.http_status.unwrap_or(0)))
        })?;

    let urls = parse_feed_entry_urls(body, &feed.feed_type);
    let mut upserted = 0u32;
    for url in urls.into_iter().take(FEED_ENTRY_LIMIT) {
        if is_homepage_url(&url) {
            continue;
        }
        let canonical = match canonicalize_url(&url) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if canonical.registrable_domain != feed.domain {
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

    sqlx::query(
        r#"
        UPDATE content_domain_feed
        SET last_polled_at = UTC_TIMESTAMP(6), last_error = NULL
        WHERE id = ?
        "#,
    )
    .bind(feed.id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(upserted)
}

/// Extract article URLs from RSS or Atom XML (lightweight tag scan, no XML crate).
#[must_use]
pub fn parse_feed_entry_urls(xml: &str, feed_type: &str) -> Vec<String> {
    let lower_type = feed_type.to_ascii_lowercase();
    if lower_type == "atom" {
        parse_atom_entry_urls(xml)
    } else {
        parse_rss_item_urls(xml)
    }
}

fn parse_rss_item_urls(xml: &str) -> Vec<String> {
    let lower = xml.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut search = 0usize;
    while let Some(rel) = lower[search..].find("<item") {
        let start = search + rel;
        let end = lower[start..]
            .find("</item>")
            .map(|i| start + i + "</item>".len())
            .unwrap_or_else(|| xml.len().min(start + 8000));
        let block = &xml[start..end];
        if let Some(url) = first_link_in_feed_block(block) {
            push_unique_url(&mut out, url);
        }
        search = end;
    }
    out
}

fn parse_atom_entry_urls(xml: &str) -> Vec<String> {
    let lower = xml.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut search = 0usize;
    while let Some(rel) = lower[search..].find("<entry") {
        let start = search + rel;
        let end = lower[start..]
            .find("</entry>")
            .map(|i| start + i + "</entry>".len())
            .unwrap_or_else(|| xml.len().min(start + 8000));
        let block = &xml[start..end];
        if let Some(url) = first_atom_link_in_block(block) {
            push_unique_url(&mut out, url);
        }
        search = end;
    }
    out
}

fn first_link_in_feed_block(block: &str) -> Option<String> {
    if let Some(url) = tag_text_content(block, "link") {
        if url.starts_with("http://") || url.starts_with("https://") {
            return Some(url);
        }
    }
    if let Some(url) = tag_text_content(block, "guid") {
        if url.starts_with("http://") || url.starts_with("https://") {
            return Some(url);
        }
    }
    None
}

fn first_atom_link_in_block(block: &str) -> Option<String> {
    let lower = block.to_ascii_lowercase();
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find("<link") {
        let start = from + rel;
        let end = lower[start..].find('>').map(|i| start + i + 1)?;
        let tag = &block[start..end];
        if let Some(href) = extract_href(tag) {
            if href.starts_with("http://") || href.starts_with("https://") {
                return Some(href);
            }
        }
        from = end;
    }
    None
}

fn tag_text_content(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let lower = block.to_ascii_lowercase();
    let start = lower.find(&open)?;
    let after_open = lower[start..].find('>')? + start + 1;
    let close_rel = lower[after_open..].find(&close)?;
    let inner = block[after_open..after_open + close_rel].trim();
    if inner.is_empty() {
        None
    } else {
        Some(inner.to_string())
    }
}

fn extract_href(tag: &str) -> Option<String> {
    og_fetch::extract_attr_value(tag, "href")
}

fn push_unique_url(out: &mut Vec<String>, url: String) {
    if !out.iter().any(|u| u == &url) {
        out.push(url);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rss_item_links_parsed() {
        let xml = r#"
        <rss><channel>
          <item><link>https://example.com/a</link></item>
          <item><guid>https://example.com/b</guid></item>
        </channel></rss>"#;
        let urls = parse_feed_entry_urls(xml, "rss");
        assert_eq!(urls.len(), 2);
        assert!(urls.contains(&"https://example.com/a".to_string()));
    }

    #[test]
    fn atom_entry_href_parsed() {
        let xml = r#"
        <feed xmlns="http://www.w3.org/2005/Atom">
          <entry><link rel="alternate" href="https://example.com/x"/></entry>
        </feed>"#;
        let urls = parse_feed_entry_urls(xml, "atom");
        assert_eq!(urls, vec!["https://example.com/x".to_string()]);
    }
}
