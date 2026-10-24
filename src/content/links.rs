//! Canonical URL ingestion foundation (domain extraction, trust defaults, OG cache lifecycle).
//!
//! [`registrable_domain_is_blocked`] reflects **current** rows in [`content_domain`]. The `post` table
//! also stores an optional denormalized `domain_id` (see migrations) so feed and validity checks can
//! join or filter without always traversing `content_link`.

use chrono::{DateTime, Utc};
use reqwest::Url;
use sqlx::MySqlPool;

use crate::error::{AppError, AppResult};
use crate::i18n;
use crate::ref_key_values;
use crate::ref_keys;

const MAX_CANONICAL_URL_LEN: usize = 2048;
const TRACKING_KEYS: &[&str] = &[
    "fbclid", "gclid", "dclid", "mc_eid", "mc_cid", "igshid", "mkt_tok", "si",
];

#[derive(Debug, Clone)]
pub struct CanonicalizedUrl {
    pub url_original: String,
    pub url_canonical: String,
    /// Registrable (eTLD+1) domain via public suffix list (e.g. `youtube.com`).
    pub registrable_domain: String,
}

#[derive(Debug, Clone)]
pub struct UpsertedLink {
    pub domain_id: u64,
    pub link_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchStatus {
    Pending,
    Ok,
    NotFound,
    Gone,
    Error,
}

impl FetchStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ok => "ok",
            Self::NotFound => "not_found",
            Self::Gone => "gone",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct OpenGraphSnapshot {
    pub title: Option<String>,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub site_name: Option<String>,
    pub og_type: Option<String>,
    pub og_url: Option<String>,
    pub og_locale: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FetchOutcome {
    pub status: FetchStatus,
    pub http_status: Option<u16>,
    pub etag: Option<String>,
    pub last_modified_header: Option<String>,
    pub fetch_error: Option<String>,
    pub fetched_at: DateTime<Utc>,
    pub next_recrawl_at: DateTime<Utc>,
    pub og: OpenGraphSnapshot,
}

#[must_use]
pub fn canonicalize_url(raw: &str) -> AppResult<CanonicalizedUrl> {
    let original = raw.trim();
    if original.is_empty() {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&i18n::request_locale(), "social.url_required"),
        ));
    }

    let mut url = Url::parse(original).map_err(|_| {
        AppError::BadRequest(i18n::catalog().t(&i18n::request_locale(), "social.url_invalid"))
    })?;

    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&i18n::request_locale(), "social.url_scheme_invalid"),
        ));
    }

    let host = url.host_str().map(str::to_string).ok_or_else(|| {
        AppError::BadRequest(i18n::catalog().t(&i18n::request_locale(), "social.url_invalid"))
    })?;
    if host.parse::<std::net::IpAddr>().is_ok() {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&i18n::request_locale(), "social.url_host_invalid"),
        ));
    }

    // Attribute URL immutably, but canonicalize for dedupe/query.
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_fragment(None);

    // Remove default ports.
    match (url.scheme(), url.port()) {
        ("http", Some(80)) | ("https", Some(443)) => {
            let _ = url.set_port(None);
        }
        _ => {}
    }

    if url.path().is_empty() {
        url.set_path("/");
    }

    let mut kept: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| {
            let key = k.to_ascii_lowercase();
            !key.starts_with("utm_") && !TRACKING_KEYS.contains(&key.as_str())
        })
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    kept.sort_unstable_by(|a, b| a.cmp(b));
    if kept.is_empty() {
        url.set_query(None);
    } else {
        let mut q = url.query_pairs_mut();
        q.clear();
        for (k, v) in kept {
            q.append_pair(&k, &v);
        }
    }

    let canonical = url.to_string();
    if canonical.len() > MAX_CANONICAL_URL_LEN {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&i18n::request_locale(), "social.url_too_long"),
        ));
    }

    let registrable_domain = psl::domain_str(&host)
        .ok_or_else(|| {
            AppError::BadRequest(
                i18n::catalog().t(&i18n::request_locale(), "social.url_host_invalid"),
            )
        })?
        .to_ascii_lowercase();

    Ok(CanonicalizedUrl {
        url_original: original.to_string(),
        url_canonical: canonical,
        registrable_domain,
    })
}

/// Whether the registrable domain is currently marked unusable for links/embeds in [`content_domain`]:
/// trust-tier `status` = `blocked`, or `safety_state` = `unsafe`.
pub async fn registrable_domain_is_blocked(
    pool: &MySqlPool,
    registrable_domain: &str,
) -> Result<bool, sqlx::Error> {
    let row: Option<(String, String)> = sqlx::query_as(
        r#"
        SELECT rk.`key`, d.safety_state
        FROM content_domain d
        INNER JOIN ref_key_values rk
            ON rk.id = d.status_id
           AND rk.`group` = ?
           AND rk.is_active = 1
        WHERE d.domain = ?
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_STATUS)
    .bind(registrable_domain)
    .fetch_optional(pool)
    .await?;

    Ok(row.is_some_and(|(key, safety)| {
        key == ref_keys::KEY_BLOCKED || safety.as_str() == "unsafe"
    }))
}

/// Reject blocked domains and append a `safety_sanitation_event` audit row (point 15).
pub async fn ensure_domain_allowed(
    pool: &MySqlPool,
    registrable_domain: &str,
    source: &str,
    actor_user_id: Option<u64>,
    target_post_id: Option<u64>,
) -> AppResult<()> {
    if registrable_domain_is_blocked(pool, registrable_domain)
        .await
        .map_err(AppError::Db)?
    {
        crate::social::safety::sanitation::log_domain_deny(
            pool,
            source,
            registrable_domain,
            actor_user_id,
            target_post_id,
        )
        .await;
        return Err(AppError::BadRequest(
            i18n::catalog().t(&i18n::request_locale(), "social.domain_blocked_for_post"),
        ));
    }
    Ok(())
}

/// Insert or touch domain+link rows.
/// New rows default to `status.pending_verification` and neutral rating `2.50`.
pub async fn upsert_canonical_link(
    pool: &MySqlPool,
    canonical: &CanonicalizedUrl,
    acting_user_id: Option<u64>,
) -> AppResult<UpsertedLink> {
    let pending_status_id = ref_key_values::default_pending_status_id(pool)
        .await
        .map_err(AppError::Db)?
        .ok_or_else(|| {
            AppError::BadRequest(i18n::catalog().t(
                &i18n::request_locale(),
                "ref_keys.default_status_unconfigured",
            ))
        })?;

    let domain_res = sqlx::query(
        r#"
        INSERT INTO content_domain (
            domain, status_id, rating, first_seen_at, last_seen_at, created_by, modified_by
        )
        VALUES (?, ?, 2.50, UTC_TIMESTAMP(6), UTC_TIMESTAMP(6), ?, ?)
        ON DUPLICATE KEY UPDATE
            id = LAST_INSERT_ID(id),
            last_seen_at = UTC_TIMESTAMP(6),
            modified_by = COALESCE(VALUES(modified_by), modified_by)
        "#,
    )
    .bind(canonical.registrable_domain.as_str())
    .bind(pending_status_id)
    .bind(acting_user_id)
    .bind(acting_user_id)
    .execute(pool)
    .await
    .map_err(AppError::Db)?;
    let domain_id = domain_res.last_insert_id();

    let link_res = sqlx::query(
        r#"
        INSERT INTO content_link (
            domain_id, url_original, url_canonical, url_canonical_sha,
            status_id, rating, fetch_status, fetch_attempts, next_recrawl_at,
            created_by, modified_by
        )
        VALUES (
            ?, ?, ?, UNHEX(SHA2(?, 256)),
            ?, 2.50, 'pending', 0, UTC_TIMESTAMP(6) + INTERVAL 6 HOUR,
            ?, ?
        )
        ON DUPLICATE KEY UPDATE
            id = LAST_INSERT_ID(id),
            domain_id = VALUES(domain_id),
            modified_by = COALESCE(VALUES(modified_by), modified_by)
        "#,
    )
    .bind(domain_id)
    .bind(canonical.url_original.as_str())
    .bind(canonical.url_canonical.as_str())
    .bind(canonical.url_canonical.as_str())
    .bind(pending_status_id)
    .bind(acting_user_id)
    .bind(acting_user_id)
    .execute(pool)
    .await
    .map_err(AppError::Db)?;
    let link_id = link_res.last_insert_id();

    Ok(UpsertedLink { domain_id, link_id })
}

/// Council portfolio ingest fallback only — do not use for share picks (use [`crate::content::og_fetch`]).
pub async fn mark_link_fetch_ok_for_council(
    pool: &MySqlPool,
    link_id: u64,
    acting_user_id: Option<u64>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE content_link cl
        INNER JOIN content_domain cd ON cd.id = cl.domain_id
        SET cl.fetch_status = 'ok',
            cl.og_site_name = COALESCE(NULLIF(TRIM(cl.og_site_name), ''), cd.domain),
            cl.og_title = COALESCE(NULLIF(TRIM(cl.og_title), ''), cd.domain),
            cl.modified_by = COALESCE(?, cl.modified_by)
        WHERE cl.id = ?
        "#,
    )
    .bind(acting_user_id)
    .bind(link_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Apply fetch/OG refresh result (supports changed/deleted detection + recrawl schedule).
pub async fn record_fetch_outcome(
    pool: &MySqlPool,
    link_id: u64,
    outcome: &FetchOutcome,
    acting_user_id: Option<u64>,
) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE content_link
        SET
            fetch_status = ?,
            fetch_attempts = fetch_attempts + 1,
            last_http_status = ?,
            fetch_error = ?,
            fetched_at = ?,
            etag = ?,
            last_modified_header = ?,
            next_recrawl_at = ?,
            og_title = ?,
            og_description = ?,
            og_image_url = ?,
            og_site_name = ?,
            og_type = ?,
            og_url = ?,
            og_locale = ?,
            last_success_at = CASE
                WHEN ? = 'ok' THEN ?
                ELSE last_success_at
            END,
            content_deleted_at = CASE
                WHEN ? IN ('not_found', 'gone') AND content_deleted_at IS NULL THEN ?
                WHEN ? = 'ok' THEN NULL
                ELSE content_deleted_at
            END,
            content_changed_at = CASE
                WHEN ? = 'ok'
                     AND (
                        IFNULL(og_title, '') <> IFNULL(?, '') OR
                        IFNULL(og_description, '') <> IFNULL(?, '') OR
                        IFNULL(og_image_url, '') <> IFNULL(?, '') OR
                        IFNULL(og_site_name, '') <> IFNULL(?, '')
                     )
                THEN ?
                ELSE content_changed_at
            END,
            modified_by = COALESCE(?, modified_by)
        WHERE id = ?
        "#,
    )
    .bind(outcome.status.as_str())
    .bind(outcome.http_status.map(u32::from))
    .bind(outcome.fetch_error.as_deref())
    .bind(outcome.fetched_at)
    .bind(outcome.etag.as_deref())
    .bind(outcome.last_modified_header.as_deref())
    .bind(outcome.next_recrawl_at)
    .bind(outcome.og.title.as_deref())
    .bind(outcome.og.description.as_deref())
    .bind(outcome.og.image_url.as_deref())
    .bind(outcome.og.site_name.as_deref())
    .bind(outcome.og.og_type.as_deref())
    .bind(outcome.og.og_url.as_deref())
    .bind(outcome.og.og_locale.as_deref())
    .bind(outcome.status.as_str())
    .bind(outcome.fetched_at)
    .bind(outcome.status.as_str())
    .bind(outcome.fetched_at)
    .bind(outcome.status.as_str())
    .bind(outcome.status.as_str())
    .bind(outcome.og.title.as_deref())
    .bind(outcome.og.description.as_deref())
    .bind(outcome.og.image_url.as_deref())
    .bind(outcome.og.site_name.as_deref())
    .bind(outcome.fetched_at)
    .bind(acting_user_id)
    .bind(link_id)
    .execute(pool)
    .await
    .map_err(AppError::Db)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalize_removes_tracking_and_fragment() {
        let _ = crate::i18n::catalog();
        let c = canonicalize_url(
            "https://www.youtube.com/watch?v=abc123&utm_source=x&fbclid=z#section",
        )
        .expect("canonicalize");
        assert_eq!(c.registrable_domain, "youtube.com");
        assert_eq!(c.url_canonical, "https://www.youtube.com/watch?v=abc123");
    }
}
