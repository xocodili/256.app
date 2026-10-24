//! Reference rows from `ref_key_values` (enumerations, taxonomy, etc.).
//!
//! Display strings are **not** stored here. Add nested keys under `ref_values` in each
//! `locales/<locale>.json` so the flattened catalog key matches [`locale_key_for`] (e.g.
//! `ref_values.status.pending_verification` ↔ nested JSON under `ref_values.status`.

use serde::Serialize;
use sqlx::FromRow;
use sqlx::{MySqlConnection, MySqlPool};

use crate::error::{AppError, AppResult};
use crate::i18n;
use crate::ref_keys;

/// Active row id for `group` + `key`, if any.
pub async fn lookup_active_id_conn(
    conn: &mut MySqlConnection,
    group: &str,
    key: &str,
) -> Result<Option<u64>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT id FROM ref_key_values
        WHERE `group` = ? AND `key` = ? AND is_active = 1
        LIMIT 1
        "#,
    )
    .bind(group)
    .bind(key)
    .fetch_optional(conn)
    .await
}

/// Active row id for `group` + `key`, if any.
pub async fn lookup_active_id(
    pool: &MySqlPool,
    group: &str,
    key: &str,
) -> Result<Option<u64>, sqlx::Error> {
    let mut conn = pool.acquire().await?;
    lookup_active_id_conn(&mut *conn, group, key).await
}

/// Default [`ref_keys::KEY_PENDING_VERIFICATION`] in [`ref_keys::GROUP_STATUS`] (for new users, entities, edges).
pub async fn default_pending_status_id(pool: &MySqlPool) -> Result<Option<u64>, sqlx::Error> {
    lookup_active_id(
        pool,
        ref_keys::GROUP_STATUS,
        ref_keys::KEY_PENDING_VERIFICATION,
    )
    .await
}

const MAX_GROUP_LEN: usize = 128;

/// Flat i18n catalog key for `group` + `key` (dots in DB segments become `_` to avoid ambiguous paths).
#[must_use]
pub fn locale_key_for(group: &str, key: &str) -> String {
    let g = group.replace('.', "_");
    let k = key.replace('.', "_");
    format!("ref_values.{g}.{k}")
}

#[derive(Debug, Serialize)]
pub struct RefKeyValuePublic {
    /// String so JSON clients do not lose precision above 2^53−1.
    pub id: String,
    pub group: String,
    pub key: String,
    /// Flat i18n key: use with your locale bundle (same key the server passes to [`crate::i18n::Catalog::t`]).
    pub locale_key: String,
    pub sort_order: i32,
}

#[derive(Debug, FromRow)]
struct RefKeyValueRow {
    id: u64,
    ref_group: String,
    ref_key: String,
    sort_order: i32,
}

fn validate_group(raw: &str) -> AppResult<String> {
    let g = raw.trim();
    if g.is_empty() {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&i18n::request_locale(), "ref_key_values.group_required"),
        ));
    }
    if g.len() > MAX_GROUP_LEN {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&i18n::request_locale(), "ref_key_values.group_invalid"),
        ));
    }
    if !g
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
    {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&i18n::request_locale(), "ref_key_values.group_invalid"),
        ));
    }
    Ok(g.to_string())
}

/// Active rows for a validated `group`, ordered by `sort_order` then `key`.
pub async fn list_active_by_group(
    pool: &MySqlPool,
    group: &str,
) -> AppResult<Vec<RefKeyValuePublic>> {
    let group = validate_group(group)?;
    if group == ref_keys::GROUP_VERIFICATION_TIER {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&i18n::request_locale(), "ref_key_values.group_forbidden"),
        ));
    }
    let rows = sqlx::query_as::<_, RefKeyValueRow>(
        r#"
        SELECT
            id,
            `group` AS ref_group,
            `key` AS ref_key,
            sort_order
        FROM ref_key_values
        WHERE `group` = ? AND is_active = 1
        ORDER BY sort_order ASC, `key` ASC
        "#,
    )
    .bind(&group)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let locale_key = locale_key_for(&r.ref_group, &r.ref_key);
            RefKeyValuePublic {
                id: r.id.to_string(),
                group: r.ref_group,
                key: r.ref_key,
                locale_key,
                sort_order: r.sort_order,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_group_accepts_safe_tokens() {
        let _ = i18n::catalog();
        assert!(validate_group("status").is_ok());
        assert!(validate_group("  foo_bar-1  ").unwrap() == "foo_bar-1");
    }

    #[test]
    fn validate_group_rejects_empty_and_bad_chars() {
        let _ = i18n::catalog();
        assert!(validate_group("").is_err());
        assert!(validate_group("   ").is_err());
        assert!(validate_group("a;b").is_err());
    }

    #[test]
    fn locale_key_flattens_dots() {
        assert_eq!(
            locale_key_for("status", "pending_verification"),
            "ref_values.status.pending_verification"
        );
        assert_eq!(locale_key_for("a.b", "c"), "ref_values.a_b.c");
    }
}
