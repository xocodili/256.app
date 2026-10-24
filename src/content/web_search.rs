//! Web search for council invite person discovery (Brave or Google CSE).

use std::time::Duration;

use serde::Deserialize;

use crate::config::{Config, WebSearchProvider};

const DEFAULT_TIMEOUT_MS: u64 = 8000;
const MAX_QUERY_LEN: usize = 180;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Copy)]
pub struct WebSearchLimits {
    pub max_queries: u8,
    pub hits_per_query: u8,
}

impl Default for WebSearchLimits {
    fn default() -> Self {
        Self {
            max_queries: 3,
            hits_per_query: 5,
        }
    }
}

/// Returns empty when API key unset or provider misconfigured.
pub async fn search(
    config: &Config,
    query: &str,
    limit: u8,
) -> Vec<SearchHit> {
    let q = sanitize_query(query);
    if q.is_empty() || !config.web_search_configured() {
        return Vec::new();
    }
    let timeout = Duration::from_millis(config.council_web_search_timeout_ms.max(1000));
    let client = match reqwest::Client::builder().timeout(timeout).build() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    match config.council_web_search_provider {
        WebSearchProvider::Brave => search_brave(&client, config, &q, limit).await,
        WebSearchProvider::GoogleCse => search_google_cse(&client, config, &q, limit).await,
        WebSearchProvider::None => Vec::new(),
    }
}

#[must_use]
pub fn sanitize_query(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.chars().take(MAX_QUERY_LEN) {
        if ch.is_control() {
            continue;
        }
        out.push(ch);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ").trim().to_string()
}

async fn search_brave(
    client: &reqwest::Client,
    config: &Config,
    query: &str,
    limit: u8,
) -> Vec<SearchHit> {
    let Some(key) = config.council_web_search_api_key.as_deref() else {
        return Vec::new();
    };
    let resp = match client
        .get("https://api.search.brave.com/res/v1/web/search")
        .header("X-Subscription-Token", key)
        .header("Accept", "application/json")
        .query(&[("q", query), ("count", &limit.max(1).min(10).to_string())])
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    if !resp.status().is_success() {
        return Vec::new();
    }
    let body: BraveResponse = match resp.json().await {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    body.web
        .and_then(|w| w.results)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|r| {
            let url = r.url?.trim().to_string();
            if url.is_empty() || !url.starts_with("http") {
                return None;
            }
            Some(SearchHit {
                title: r.title.unwrap_or_default(),
                url,
                snippet: r.description.unwrap_or_default(),
            })
        })
        .take(limit as usize)
        .collect()
}

async fn search_google_cse(
    client: &reqwest::Client,
    config: &Config,
    query: &str,
    limit: u8,
) -> Vec<SearchHit> {
    let Some(key) = config.council_web_search_api_key.as_deref() else {
        return Vec::new();
    };
    let Some(cx) = config.council_web_search_cx.as_deref() else {
        return Vec::new();
    };
    let resp = match client
        .get("https://www.googleapis.com/customsearch/v1")
        .query(&[
            ("key", key),
            ("cx", cx),
            ("q", query),
            ("num", &limit.max(1).min(10).to_string()),
        ])
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    if !resp.status().is_success() {
        return Vec::new();
    }
    let body: GoogleCseResponse = match resp.json().await {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    body.items
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| {
            let url = item.link?.trim().to_string();
            if url.is_empty() {
                return None;
            }
            Some(SearchHit {
                title: item.title.unwrap_or_default(),
                url,
                snippet: item.snippet.unwrap_or_default(),
            })
        })
        .take(limit as usize)
        .collect()
}

#[derive(Debug, Deserialize)]
struct BraveResponse {
    web: Option<BraveWeb>,
}

#[derive(Debug, Deserialize)]
struct BraveWeb {
    results: Option<Vec<BraveResult>>,
}

#[derive(Debug, Deserialize)]
struct BraveResult {
    title: Option<String>,
    url: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleCseResponse {
    items: Option<Vec<GoogleCseItem>>,
}

#[derive(Debug, Deserialize)]
struct GoogleCseItem {
    title: Option<String>,
    link: Option<String>,
    snippet: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_control_chars() {
        assert_eq!(sanitize_query("Jane Doe\x00 author"), "Jane Doe author");
    }
}
