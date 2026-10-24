//! Domain preview reputation bands — when link-card OG images may auto-display.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::MySqlPool;

use crate::ref_keys;

pub const KNOWN_MIN_DISTINCT_HUMANS: u32 = 3;
pub const KNOWN_MIN_AGE_DAYS: i64 = 14;
pub const TRUSTED_MIN_RATING: &str = "3.50";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewBand {
    Blocked,
    Trusted,
    Known,
    Unknown,
}

impl PreviewBand {
    #[must_use]
    pub const fn preview_image_allowed(self) -> bool {
        matches!(self, Self::Trusted | Self::Known)
    }
}

#[derive(Debug, Clone)]
struct DomainRow {
    status_key: String,
    safety_state: String,
    rating: Decimal,
    first_seen_at: DateTime<Utc>,
}

#[must_use]
pub fn resolve_band(
    row: &DomainRow,
    distinct_human_attachers: u32,
    has_unreversed_acute_harm_apply: bool,
    now: DateTime<Utc>,
) -> PreviewBand {
    if row.status_key == ref_keys::KEY_BLOCKED || row.safety_state == "unsafe" {
        return PreviewBand::Blocked;
    }
    if has_unreversed_acute_harm_apply {
        return PreviewBand::Unknown;
    }
    let trusted_rating = Decimal::from_str_exact(TRUSTED_MIN_RATING).unwrap_or(Decimal::new(350, 2));
    if (row.status_key == ref_keys::KEY_VERIFIED && row.safety_state == "safe")
        || row.rating >= trusted_rating
    {
        return PreviewBand::Trusted;
    }
    if distinct_human_attachers >= KNOWN_MIN_DISTINCT_HUMANS {
        return PreviewBand::Known;
    }
    let age_cutoff = now - Duration::days(KNOWN_MIN_AGE_DAYS);
    if row.first_seen_at <= age_cutoff {
        return PreviewBand::Known;
    }
    PreviewBand::Unknown
}

/// Batch-resolve preview bands for feed/API pages (one round-trip per auxiliary query).
pub async fn resolve_preview_bands_batch(
    pool: &MySqlPool,
    domain_ids: &[u64],
) -> Result<HashMap<u64, PreviewBand>, sqlx::Error> {
    let mut out = HashMap::new();
    if domain_ids.is_empty() {
        return Ok(out);
    }
    let unique: Vec<u64> = domain_ids
        .iter()
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    let placeholders = unique.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        r#"
        SELECT d.id, rk.`key`, d.safety_state, d.rating, d.first_seen_at
        FROM content_domain d
        INNER JOIN ref_key_values rk ON rk.id = d.status_id
        WHERE d.id IN ({placeholders})
        "#
    );
    let mut q = sqlx::query_as::<_, (u64, String, String, Decimal, DateTime<Utc>)>(&sql);
    for id in &unique {
        q = q.bind(id);
    }
    let rows = q.fetch_all(pool).await?;

    let attacher_counts = count_distinct_human_attachers_batch(pool, &unique).await?;
    let acute_harm = domain_ids_with_unreversed_acute_harm_apply(pool, &unique).await?;
    let now = Utc::now();

    for (id, status_key, safety_state, rating, first_seen_at) in rows {
        let row = DomainRow {
            status_key,
            safety_state,
            rating,
            first_seen_at,
        };
        let band = resolve_band(
            &row,
            attacher_counts.get(&id).copied().unwrap_or(0),
            acute_harm.contains(&id),
            now,
        );
        out.insert(id, band);
    }
    Ok(out)
}

pub async fn resolve_preview_band_for_domain(
    pool: &MySqlPool,
    domain_id: u64,
) -> Result<PreviewBand, sqlx::Error> {
    let map = resolve_preview_bands_batch(pool, &[domain_id]).await?;
    Ok(map.get(&domain_id).copied().unwrap_or(PreviewBand::Unknown))
}

pub async fn count_distinct_human_attachers(
    pool: &MySqlPool,
    domain_id: u64,
) -> Result<u32, sqlx::Error> {
    let map = count_distinct_human_attachers_batch(pool, &[domain_id]).await?;
    Ok(map.get(&domain_id).copied().unwrap_or(0))
}

pub async fn count_distinct_human_attachers_batch(
    pool: &MySqlPool,
    domain_ids: &[u64],
) -> Result<HashMap<u64, u32>, sqlx::Error> {
    let mut out = HashMap::new();
    if domain_ids.is_empty() {
        return Ok(out);
    }
    let unique: Vec<u64> = domain_ids
        .iter()
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let placeholders = unique.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        r#"
        SELECT p.domain_id, COUNT(DISTINCT p.entity_id) AS human_count
        FROM post p
        INNER JOIN social_entity se ON se.id = p.entity_id
        INNER JOIN ref_key_values rk
            ON rk.id = se.type_id
           AND rk.`group` = ?
           AND rk.`key` = ?
        WHERE p.domain_id IN ({placeholders})
          AND p.content_link_id IS NOT NULL
        GROUP BY p.domain_id
        "#
    );
    let mut q = sqlx::query_as::<_, (u64, i64)>(&sql);
    q = q
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
        .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN);
    for id in &unique {
        q = q.bind(id);
    }
    let rows = q.fetch_all(pool).await?;
    for (domain_id, count) in rows {
        out.insert(domain_id, count.max(0) as u32);
    }
    Ok(out)
}

async fn domain_ids_with_unreversed_acute_harm_apply(
    pool: &MySqlPool,
    domain_ids: &[u64],
) -> Result<HashSet<u64>, sqlx::Error> {
    let mut out = HashSet::new();
    if domain_ids.is_empty() {
        return Ok(out);
    }
    let unique: Vec<u64> = domain_ids
        .iter()
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let placeholders = unique.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        r#"
        SELECT DISTINCT e.content_domain_id
        FROM acute_harm_content_rating_event e
        WHERE e.content_domain_id IN ({placeholders})
          AND e.direction = 'apply'
          AND NOT EXISTS (
              SELECT 1
              FROM acute_harm_content_rating_event r
              WHERE r.judicial_case_id = e.judicial_case_id
                AND r.content_domain_id = e.content_domain_id
                AND r.direction = 'refund'
          )
        "#
    );
    let mut q = sqlx::query_scalar::<_, u64>(&sql);
    for id in &unique {
        q = q.bind(id);
    }
    let rows = q.fetch_all(pool).await?;
    out.extend(rows);
    Ok(out)
}

/// Fail-closed lookup: missing domain id or unknown map entry → withhold auto-display.
#[must_use]
pub fn preview_allowed_for_domain_id(
    domain_id: Option<u64>,
    map: &HashMap<u64, bool>,
) -> bool {
    domain_id
        .and_then(|id| map.get(&id).copied())
        .unwrap_or(false)
}

/// Split sanitized OG image URL for composer vs feed/card consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OgImagePreviewFields {
    pub og_image_url: Option<String>,
    pub og_image_reveal_url: Option<String>,
}

#[must_use]
pub fn split_og_image_for_preview(
    raw_image: Option<&str>,
    base_url: &str,
    preview_image_allowed: bool,
    for_composer: bool,
) -> OgImagePreviewFields {
    let sanitized = raw_image
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|u| crate::content::url_extract::sanitize_http_image_url(u, base_url));
    match sanitized {
        None => OgImagePreviewFields {
            og_image_url: None,
            og_image_reveal_url: None,
        },
        Some(u) if preview_image_allowed || for_composer => OgImagePreviewFields {
            og_image_url: Some(u),
            og_image_reveal_url: None,
        },
        Some(u) => OgImagePreviewFields {
            og_image_url: None,
            og_image_reveal_url: Some(u),
        },
    }
}

/// Map each domain id to whether OG preview images may auto-display on feed cards.
pub async fn preview_image_allowed_map(
    pool: &MySqlPool,
    domain_ids: &[u64],
) -> Result<HashMap<u64, bool>, sqlx::Error> {
    let bands = resolve_preview_bands_batch(pool, domain_ids).await?;
    Ok(bands
        .into_iter()
        .map(|(id, band)| (id, band.preview_image_allowed()))
        .collect())
}

/// Reject any blocked/unsafe inline URL in post body text.
pub async fn ensure_inline_body_urls_allowed(
    pool: &MySqlPool,
    parts: &[&str],
    source: &str,
    actor_user_id: u64,
    target_post_id: Option<u64>,
) -> crate::error::AppResult<()> {
    use std::collections::HashSet;

    use crate::content::links::{self, CanonicalizedUrl};

    let mut seen_domains = HashSet::new();
    for part in parts {
        for raw in crate::content::url_extract::extract_urls_from_text(part) {
            let canonical: CanonicalizedUrl = links::canonicalize_url(&raw)?;
            if !seen_domains.insert(canonical.registrable_domain.clone()) {
                continue;
            }
            links::ensure_domain_allowed(
                pool,
                &canonical.registrable_domain,
                source,
                Some(actor_user_id),
                target_post_id,
            )
            .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn row(status: &str, safety: &str, rating: &str, age_days_ago: i64) -> DomainRow {
        DomainRow {
            status_key: status.to_string(),
            safety_state: safety.to_string(),
            rating: Decimal::from_str_exact(rating).unwrap(),
            first_seen_at: Utc::now() - Duration::days(age_days_ago),
        }
    }

    #[test]
    fn blocked_status_is_blocked_band() {
        let now = Utc.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap();
        let band = resolve_band(&row("blocked", "safe", "2.50", 0), 0, false, now);
        assert_eq!(band, PreviewBand::Blocked);
    }

    #[test]
    fn verified_safe_is_trusted_at_default_rating() {
        let now = Utc.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap();
        let band = resolve_band(&row("verified", "safe", "2.50", 0), 0, false, now);
        assert_eq!(band, PreviewBand::Trusted);
    }

    #[test]
    fn rating_alone_at_250_is_unknown_without_other_signals() {
        let now = Utc.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap();
        let band = resolve_band(&row("pending_verification", "safe", "2.50", 0), 0, false, now);
        assert_eq!(band, PreviewBand::Unknown);
    }

    #[test]
    fn preview_allowed_missing_domain_is_false() {
        let map = HashMap::from([(1_u64, true)]);
        assert!(!preview_allowed_for_domain_id(None, &map));
        assert!(!preview_allowed_for_domain_id(Some(99), &map));
        assert!(preview_allowed_for_domain_id(Some(1), &map));
    }

    #[test]
    fn split_og_image_withholds_for_feed_but_not_composer() {
        let img = "https://cdn.example.test/preview.jpg";
        let feed = split_og_image_for_preview(Some(img), "https://example.com", false, false);
        assert!(feed.og_image_url.is_none());
        assert_eq!(feed.og_image_reveal_url.as_deref(), Some(img));
        let composer = split_og_image_for_preview(Some(img), "https://example.com", false, true);
        assert_eq!(composer.og_image_url.as_deref(), Some(img));
        assert!(composer.og_image_reveal_url.is_none());
    }

    #[test]
    fn three_human_attachers_promotes_to_known() {
        let now = Utc.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap();
        let band = resolve_band(
            &row("pending_verification", "safe", "2.50", 0),
            3,
            false,
            now,
        );
        assert_eq!(band, PreviewBand::Known);
    }

    #[test]
    fn acute_harm_apply_demotes_to_unknown() {
        let now = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
        let band = resolve_band(
            &row("verified", "safe", "4.00", 1000),
            10,
            true,
            now,
        );
        assert_eq!(band, PreviewBand::Unknown);
    }

    #[test]
    fn age_alone_promotes_to_known() {
        let now = Utc::now();
        let domain = row("pending_verification", "safe", "2.50", KNOWN_MIN_AGE_DAYS + 1);
        let band = resolve_band(&domain, 0, false, now);
        assert_eq!(band, PreviewBand::Known);
    }
}
