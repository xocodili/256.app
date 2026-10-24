//! [`Public`]: directed link between two [`super::entity::Public`] rows.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::MySqlPool;

use super::connections;
use super::db;
use super::ids;
use super::scope;
use crate::db::TimestampUtc;
use crate::error::{AppError, AppResult};
use crate::i18n;
use crate::ref_key_values;
use crate::ref_keys;

pub const REF_GROUP_TYPE: &str = "social_entity_relationship_type";

const I18N_INVALID_ID: &str = "social_entity_relationship.invalid_id";

#[derive(Debug, Deserialize)]
pub struct CreateRequest {
    pub from_social_entity_id: String,
    pub to_social_entity_id: String,
    pub type_id: String,
    /// Omit or empty → default pending status ref row.
    #[serde(default)]
    pub status_id: Option<String>,
    /// `YYYY-MM-DD` when the relationship began (e.g. employment start); omit for unknown / not applicable.
    #[serde(default)]
    pub started_at: Option<String>,
    /// `YYYY-MM-DD` actual or expected end; omit when ongoing or unknown.
    #[serde(default)]
    pub ended_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Public {
    pub id: String,
    pub from_social_entity_id: String,
    pub to_social_entity_id: String,
    pub type_id: String,
    pub status_id: String,
    pub rating: Decimal,
    pub started_at: Option<NaiveDate>,
    pub ended_at: Option<NaiveDate>,
    pub created_at: TimestampUtc,
    pub updated_at: TimestampUtc,
    pub created_by: Option<String>,
    pub modified_by: Option<String>,
}

#[derive(Debug, FromRow)]
struct Row {
    id: u64,
    from_social_entity_id: u64,
    to_social_entity_id: u64,
    type_id: u64,
    status_id: u64,
    rating: Decimal,
    started_at: Option<NaiveDate>,
    ended_at: Option<NaiveDate>,
    created_at: TimestampUtc,
    updated_at: TimestampUtc,
    created_by: Option<u64>,
    modified_by: Option<u64>,
}

fn row_to_public(r: Row) -> Public {
    Public {
        id: r.id.to_string(),
        from_social_entity_id: r.from_social_entity_id.to_string(),
        to_social_entity_id: r.to_social_entity_id.to_string(),
        type_id: r.type_id.to_string(),
        status_id: r.status_id.to_string(),
        rating: r.rating,
        started_at: r.started_at,
        ended_at: r.ended_at,
        created_at: r.created_at,
        updated_at: r.updated_at,
        created_by: r.created_by.map(|x| x.to_string()),
        modified_by: r.modified_by.map(|x| x.to_string()),
    }
}

fn parse_optional_date(raw: &Option<String>, field: &'static str) -> AppResult<Option<NaiveDate>> {
    let loc = i18n::request_locale();
    let Some(s) = raw.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map(Some)
        .map_err(|_| {
            AppError::BadRequest(i18n::catalog().tf(
                &loc,
                "social_entity_relationship.date_invalid",
                &[("field", field)],
            ))
        })
}

pub async fn create(
    pool: &MySqlPool,
    body: CreateRequest,
    acting_user_id: u64,
) -> AppResult<Public> {
    let from_id = ids::parse_u64(
        &body.from_social_entity_id,
        "from_social_entity_id",
        I18N_INVALID_ID,
    )?;
    let to_id = ids::parse_u64(
        &body.to_social_entity_id,
        "to_social_entity_id",
        I18N_INVALID_ID,
    )?;
    let type_id = ids::parse_u64(&body.type_id, "type_id", I18N_INVALID_ID)?;
    let status_id = match body
        .status_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(s) => ids::parse_u64(s, "status_id", I18N_INVALID_ID)?,
        None => ref_key_values::default_pending_status_id(pool)
            .await
            .map_err(AppError::Db)?
            .ok_or_else(|| {
                AppError::BadRequest(i18n::catalog().t(
                    &i18n::request_locale(),
                    "ref_keys.default_status_unconfigured",
                ))
            })?,
    };

    let started_at = parse_optional_date(&body.started_at, "started_at")?;
    let ended_at = parse_optional_date(&body.ended_at, "ended_at")?;

    let loc = i18n::request_locale();
    if let (Some(s), Some(e)) = (started_at, ended_at) {
        if e < s {
            return Err(AppError::BadRequest(
                i18n::catalog().t(&loc, "social_entity_relationship.date_range_invalid"),
            ));
        }
    }

    if !db::social_entity_exists(pool, from_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "social_entity_relationship.from_entity_invalid"),
        ));
    }
    if !db::social_entity_exists(pool, to_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "social_entity_relationship.to_entity_invalid"),
        ));
    }

    if !db::ref_row_active(pool, type_id, REF_GROUP_TYPE)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "social_entity_relationship.type_ref_invalid"),
        ));
    }
    if !db::ref_row_active(pool, status_id, ref_keys::GROUP_STATUS)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "social_entity_relationship.status_ref_invalid"),
        ));
    }

    enforce_human_connection_cap(pool, from_id, to_id, &loc).await?;

    let res = sqlx::query(
        r#"
        INSERT INTO social_entity_relationship (
            from_social_entity_id, to_social_entity_id,
            type_id, status_id, relationship_status_id,
            started_at, ended_at,
            created_by, modified_by
        )
        VALUES (
            ?, ?, ?, ?,
            (
                SELECT scoped.id
                FROM ref_key_values old_status
                INNER JOIN ref_key_values scoped
                    ON scoped.`group` = 'social_relationship_status'
                   AND scoped.`key` = old_status.`key`
                WHERE old_status.id = ?
                LIMIT 1
            ),
            ?, ?, ?, ?
        )
        "#,
    )
    .bind(from_id)
    .bind(to_id)
    .bind(type_id)
    .bind(status_id)
    .bind(status_id)
    .bind(started_at)
    .bind(ended_at)
    .bind(acting_user_id)
    .bind(acting_user_id)
    .execute(pool)
    .await;

    let res = match res {
        Ok(r) => r,
        Err(e) => {
            if db::is_foreign_key_violation(&e) {
                return Err(AppError::BadRequest(
                    i18n::catalog().t(&loc, "social_entity_relationship.fk_violation"),
                ));
            }
            return Err(AppError::Db(e));
        }
    };

    let id = res.last_insert_id();
    if let Err(e) = scope::mark_relationship_endpoints_dirty(pool, from_id, to_id).await {
        tracing::warn!(
            error = %e,
            from_entity_id = from_id,
            to_entity_id = to_id,
            "scope_recalc enqueue failed after relationship insert; periodic rematerialize will reconcile"
        );
    }
    get_by_id(pool, id).await?.ok_or(AppError::NotFound)
}

/// Service-layer Dunbar-scale cap (256). Applies to **either** end that is a `social_entity_type.human`.
/// Adding a second edge to an already-connected peer is free (no slot growth); otherwise the
/// human must have strictly fewer than [`connections::MAX_CONNECTION_SLOTS`] distinct peers.
async fn enforce_human_connection_cap(
    pool: &MySqlPool,
    from_id: u64,
    to_id: u64,
    loc: &str,
) -> AppResult<()> {
    if !connections::human_can_accept_new_peer(pool, from_id, to_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            i18n::catalog().t(loc, "social_entity_relationship.human_connection_cap_reached_self"),
        ));
    }
    if !connections::human_can_accept_new_peer(pool, to_id, from_id)
        .await
        .map_err(AppError::Db)?
    {
        let handle: Option<String> = sqlx::query_scalar(
            r#"SELECT handle FROM social_entity WHERE id = ? LIMIT 1"#,
        )
        .bind(to_id)
        .fetch_optional(pool)
        .await
        .map_err(AppError::Db)?;
        return Err(AppError::BadRequest(i18n::catalog().tf(
            loc,
            "social_entity_relationship.human_connection_cap_reached_other",
            &[("handle", handle.as_deref().unwrap_or(""))],
        )));
    }
    Ok(())
}

pub async fn get_by_id(pool: &MySqlPool, id: u64) -> AppResult<Option<Public>> {
    let row = sqlx::query_as::<_, Row>(
        r#"
        SELECT
            id, from_social_entity_id, to_social_entity_id, type_id, status_id, rating,
            started_at, ended_at,
            created_at, updated_at, created_by, modified_by
        FROM social_entity_relationship
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(row_to_public))
}
