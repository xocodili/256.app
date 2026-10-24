//! Owner edit and voluntary delete for profit, non-profit, and gathering entities.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;

use super::brand_logo;
use super::community_graph;
use super::entity;
use super::ownership;
use crate::content::sanitize as content_sanitize;
use crate::error::{AppError, AppResult};
use crate::i18n;
use crate::ref_keys;

const MAX_LABEL_CHARS: usize = 512;
const MAX_TEXT_FIELD: usize = 4000;
const MAX_TAGLINE: usize = 512;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type_key", rename_all = "snake_case")]
pub enum EntityProfileEditSnapshot {
    Profit(BrandEditFields),
    NonProfit(BrandEditFields),
    Gathering(GatheringEditFields),
}

#[derive(Debug, Clone, Serialize)]
pub struct BrandEditFields {
    pub label: String,
    pub legal_name: String,
    pub founded_at: NaiveDate,
    pub description: Option<String>,
    pub intention: Option<String>,
    pub website_url: Option<String>,
    pub logo_url: Option<String>,
    pub location_id: u64,
    pub location_place_label: String,
    pub owner_handles: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GatheringEditFields {
    pub label: String,
    pub description: Option<String>,
    pub intention: Option<String>,
    pub event_url: Option<String>,
    pub location_id: u64,
    pub location_place_label: String,
    pub owner_handles: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateEntityProfileRequest {
    pub label: Option<String>,
    pub legal_name: Option<String>,
    #[serde(default)]
    pub founded_at: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub intention: Option<String>,
    #[serde(default)]
    pub website_url: Option<String>,
    #[serde(default)]
    pub logo_url: Option<String>,
    #[serde(default)]
    pub event_url: Option<String>,
    #[serde(default)]
    pub location_id: Option<String>,
    #[serde(default)]
    pub owner_handles: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteEntityBody {
    pub password: String,
    pub confirm: bool,
}

struct ManagedEntity {
    id: u64,
    handle: String,
    type_key: String,
    entity_status_key: Option<String>,
}

pub async fn user_can_manage_entity(
    pool: &MySqlPool,
    user_id: u64,
    entity_id: u64,
) -> Result<bool, sqlx::Error> {
    let created_by: Option<u64> = sqlx::query_scalar(
        r#"SELECT created_by FROM social_entity WHERE id = ? LIMIT 1"#,
    )
    .bind(entity_id)
    .fetch_optional(pool)
    .await?;
    if created_by == Some(user_id) {
        return Ok(true);
    }
    ownership::user_has_active_role_for_entity(pool, user_id, entity_id).await
}

pub async fn fetch_entity_profile_edit_snapshot(
    pool: &MySqlPool,
    user_id: u64,
    entity_id: u64,
) -> AppResult<EntityProfileEditSnapshot> {
    if !user_can_manage_entity(pool, user_id, entity_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::NotFound);
    }

    let managed = load_managed_entity(pool, entity_id).await?;
    if managed.entity_status_key.as_deref() == Some(ref_keys::KEY_ENTITY_STATUS_DEAD) {
        return Err(AppError::BadRequest(
            i18n::catalog().t(
                &i18n::request_locale(),
                "ui.entity_private.entity_delete_already_dead",
            ),
        ));
    }

    match managed.type_key.as_str() {
        ref_keys::KEY_ENTITY_TYPE_PROFIT => {
            let row = sqlx::query_as::<
                _,
                (
                    String,
                    String,
                    NaiveDate,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    u64,
                    String,
                    u64,
                ),
            >(
                r#"
                SELECT
                    COALESCE(sp.label, ''),
                    COALESCE(sp.legal_name, ''),
                    sp.founded_at,
                    sp.description,
                    sp.intention,
                    cl_web.url_canonical,
                    cl_logo.url_canonical,
                    se.location_id,
                    COALESCE(loc.display_label, ''),
                    se.created_by
                FROM social_profit sp
                INNER JOIN social_entity se ON se.id = sp.social_entity_id
                INNER JOIN locations loc ON loc.id = se.location_id
                LEFT JOIN content_link cl_web ON cl_web.id = sp.content_link_id
                LEFT JOIN content_link cl_logo ON cl_logo.id = sp.logo_content_link_id
                WHERE sp.social_entity_id = ?
                LIMIT 1
                "#,
            )
            .bind(entity_id)
            .fetch_optional(pool)
            .await
            .map_err(AppError::Db)?
            .ok_or(AppError::NotFound)?;

            let (
                label,
                legal_name,
                founded_at,
                description,
                intention,
                website_url,
                logo_url,
                location_id,
                location_place_label,
                created_by,
            ) = row;

            let owner_handles =
                fetch_additional_owner_handles_display(pool, entity_id, created_by).await?;

            Ok(EntityProfileEditSnapshot::Profit(BrandEditFields {
                label,
                legal_name,
                founded_at,
                description: trim_opt(description),
                intention: trim_opt(intention),
                website_url: trim_opt(website_url),
                logo_url: trim_opt(logo_url),
                location_id,
                location_place_label,
                owner_handles,
            }))
        }
        ref_keys::KEY_ENTITY_TYPE_NON_PROFIT => {
            let row = sqlx::query_as::<
                _,
                (
                    String,
                    String,
                    NaiveDate,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    u64,
                    String,
                    u64,
                ),
            >(
                r#"
                SELECT
                    COALESCE(snp.label, ''),
                    COALESCE(snp.legal_name, ''),
                    snp.founded_at,
                    snp.description,
                    snp.intention,
                    cl_web.url_canonical,
                    cl_logo.url_canonical,
                    se.location_id,
                    COALESCE(loc.display_label, ''),
                    se.created_by
                FROM social_non_profit snp
                INNER JOIN social_entity se ON se.id = snp.social_entity_id
                INNER JOIN locations loc ON loc.id = se.location_id
                LEFT JOIN content_link cl_web ON cl_web.id = snp.content_link_id
                LEFT JOIN content_link cl_logo ON cl_logo.id = snp.logo_content_link_id
                WHERE snp.social_entity_id = ?
                LIMIT 1
                "#,
            )
            .bind(entity_id)
            .fetch_optional(pool)
            .await
            .map_err(AppError::Db)?
            .ok_or(AppError::NotFound)?;

            let (
                label,
                legal_name,
                founded_at,
                description,
                intention,
                website_url,
                logo_url,
                location_id,
                location_place_label,
                created_by,
            ) = row;

            let owner_handles =
                fetch_additional_owner_handles_display(pool, entity_id, created_by).await?;

            Ok(EntityProfileEditSnapshot::NonProfit(BrandEditFields {
                label,
                legal_name,
                founded_at,
                description: trim_opt(description),
                intention: trim_opt(intention),
                website_url: trim_opt(website_url),
                logo_url: trim_opt(logo_url),
                location_id,
                location_place_label,
                owner_handles,
            }))
        }
        ref_keys::KEY_ENTITY_TYPE_GATHERING => {
            let row = sqlx::query_as::<
                _,
                (
                    String,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    u64,
                    String,
                    u64,
                ),
            >(
                r#"
                SELECT
                    sg.label,
                    sg.description,
                    sg.intention,
                    cl_event.url_canonical,
                    se.location_id,
                    COALESCE(loc.display_label, ''),
                    se.created_by
                FROM social_gathering sg
                INNER JOIN social_entity se ON se.id = sg.social_entity_id
                INNER JOIN locations loc ON loc.id = se.location_id
                LEFT JOIN content_link cl_event ON cl_event.id = sg.event_portal_content_link_id
                WHERE sg.social_entity_id = ?
                LIMIT 1
                "#,
            )
            .bind(entity_id)
            .fetch_optional(pool)
            .await
            .map_err(AppError::Db)?
            .ok_or(AppError::NotFound)?;

            let (
                label,
                description,
                intention,
                event_url,
                location_id,
                location_place_label,
                created_by,
            ) = row;

            let owner_handles =
                fetch_additional_owner_handles_display(pool, entity_id, created_by).await?;

            Ok(EntityProfileEditSnapshot::Gathering(GatheringEditFields {
                label,
                description: trim_opt(description),
                intention: trim_opt(intention),
                event_url: trim_opt(event_url),
                location_id,
                location_place_label,
                owner_handles,
            }))
        }
        _ => Err(AppError::BadRequest(
            i18n::catalog().t(
                &i18n::request_locale(),
                "ui.entity_private.entity_edit_not_supported",
            ),
        )),
    }
}

pub async fn update_entity_profile_by_handle(
    pool: &MySqlPool,
    config: &crate::config::Config,
    user_id: u64,
    handle: &str,
    body: UpdateEntityProfileRequest,
) -> AppResult<entity::Public> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let managed = load_managed_entity_by_handle(pool, handle).await?;
    if !user_can_manage_entity(pool, user_id, managed.id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::NotFound);
    }
    if managed.entity_status_key.as_deref() == Some(ref_keys::KEY_ENTITY_STATUS_DEAD) {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.entity_delete_already_dead"),
        ));
    }

    let entity_location_id = if matches!(
        managed.type_key.as_str(),
        ref_keys::KEY_ENTITY_TYPE_PROFIT
            | ref_keys::KEY_ENTITY_TYPE_NON_PROFIT
            | ref_keys::KEY_ENTITY_TYPE_GATHERING
    ) {
        Some(resolve_brand_location_id(pool, managed.id, &body, &loc, &c).await?)
    } else {
        None
    };

    match managed.type_key.as_str() {
        ref_keys::KEY_ENTITY_TYPE_PROFIT => {
            let label = required_field(body.label.as_deref(), MAX_LABEL_CHARS, "social_entity.profit_label_required", &loc)?;
            let legal_name = required_field(
                body.legal_name.as_deref(),
                MAX_LABEL_CHARS,
                "social_entity.profit_legal_name_required",
                &loc,
            )?;
            let founded_at = parse_founded_at(body.founded_at.as_deref(), &loc)?;
            let description = optional_rich_text(body.description.as_deref(), &loc)?;
            let intention = optional_tagline(body.intention.as_deref(), &loc)?;
            let (existing_website, existing_logo_link_id) =
                fetch_brand_website_and_logo_link_id(pool, managed.id, &managed.type_key).await?;
            let website_for_logo = body
                .website_url
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .or(existing_website);
            let website_link_id = entity::optional_brand_url_to_content_link_id(
                pool,
                body.website_url.as_deref(),
                user_id,
                &loc,
                false,
            )
            .await?;
            let logo_link_id = resolve_logo_link_id_for_update(
                pool,
                config,
                body.logo_url.as_deref(),
                website_for_logo.as_deref(),
                existing_logo_link_id,
                user_id,
                &loc,
            )
            .await?;
            let location_id = entity_location_id.expect("entity location resolved");

            let mut tx = pool.begin().await.map_err(AppError::Db)?;
            sqlx::query(
                r#"
                UPDATE social_entity
                SET location_id = ?, modified_by = ?
                WHERE id = ?
                "#,
            )
            .bind(location_id)
            .bind(user_id)
            .bind(managed.id)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Db)?;

            sqlx::query(
                r#"
                UPDATE social_profit
                SET label = ?,
                    legal_name = ?,
                    founded_at = ?,
                    description = ?,
                    intention = ?,
                    content_link_id = ?,
                    logo_content_link_id = ?
                WHERE social_entity_id = ?
                "#,
            )
            .bind(&label)
            .bind(&legal_name)
            .bind(founded_at)
            .bind(description.as_deref())
            .bind(intention.as_deref())
            .bind(website_link_id)
            .bind(logo_link_id)
            .bind(managed.id)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Db)?;

            apply_additional_owners_in_tx(&mut tx, managed.id, body.owner_handles.as_deref(), user_id)
                .await?;

            tx.commit().await.map_err(AppError::Db)?;
        }
        ref_keys::KEY_ENTITY_TYPE_NON_PROFIT => {
            let label = required_field(
                body.label.as_deref(),
                MAX_LABEL_CHARS,
                "social_entity.non_profit_label_required",
                &loc,
            )?;
            let legal_name = required_field(
                body.legal_name.as_deref(),
                MAX_LABEL_CHARS,
                "social_entity.non_profit_legal_name_required",
                &loc,
            )?;
            let founded_at = parse_founded_at(body.founded_at.as_deref(), &loc)?;
            let description = optional_rich_text(body.description.as_deref(), &loc)?;
            let intention = optional_tagline(body.intention.as_deref(), &loc)?;
            let (existing_website, existing_logo_link_id) =
                fetch_brand_website_and_logo_link_id(pool, managed.id, &managed.type_key).await?;
            let website_for_logo = body
                .website_url
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .or(existing_website);
            let website_link_id = entity::optional_brand_url_to_content_link_id(
                pool,
                body.website_url.as_deref(),
                user_id,
                &loc,
                false,
            )
            .await?;
            let logo_link_id = resolve_logo_link_id_for_update(
                pool,
                config,
                body.logo_url.as_deref(),
                website_for_logo.as_deref(),
                existing_logo_link_id,
                user_id,
                &loc,
            )
            .await?;
            let location_id = entity_location_id.expect("entity location resolved");

            let mut tx = pool.begin().await.map_err(AppError::Db)?;
            sqlx::query(
                r#"
                UPDATE social_entity
                SET location_id = ?, modified_by = ?
                WHERE id = ?
                "#,
            )
            .bind(location_id)
            .bind(user_id)
            .bind(managed.id)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Db)?;

            sqlx::query(
                r#"
                UPDATE social_non_profit
                SET label = ?,
                    legal_name = ?,
                    founded_at = ?,
                    description = ?,
                    intention = ?,
                    content_link_id = ?,
                    logo_content_link_id = ?
                WHERE social_entity_id = ?
                "#,
            )
            .bind(&label)
            .bind(&legal_name)
            .bind(founded_at)
            .bind(description.as_deref())
            .bind(intention.as_deref())
            .bind(website_link_id)
            .bind(logo_link_id)
            .bind(managed.id)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Db)?;

            apply_additional_owners_in_tx(&mut tx, managed.id, body.owner_handles.as_deref(), user_id)
                .await?;

            tx.commit().await.map_err(AppError::Db)?;
        }
        ref_keys::KEY_ENTITY_TYPE_GATHERING => {
            let label = required_field(
                body.label.as_deref(),
                MAX_LABEL_CHARS,
                "social_entity.gathering_label_required",
                &loc,
            )?;
            let description = optional_rich_text(body.description.as_deref(), &loc)?;
            let intention = optional_tagline(body.intention.as_deref(), &loc)?;
            let event_link_id = entity::optional_brand_url_to_content_link_id(
                pool,
                body.event_url.as_deref(),
                user_id,
                &loc,
                false,
            )
            .await?;
            let location_id = entity_location_id.expect("entity location resolved");

            let mut tx = pool.begin().await.map_err(AppError::Db)?;
            sqlx::query(
                r#"
                UPDATE social_entity
                SET location_id = ?, modified_by = ?
                WHERE id = ?
                "#,
            )
            .bind(location_id)
            .bind(user_id)
            .bind(managed.id)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Db)?;

            sqlx::query(
                r#"
                UPDATE social_gathering
                SET label = ?,
                    description = ?,
                    intention = ?,
                    event_portal_content_link_id = ?
                WHERE social_entity_id = ?
                "#,
            )
            .bind(&label)
            .bind(description.as_deref())
            .bind(intention.as_deref())
            .bind(event_link_id)
            .bind(managed.id)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Db)?;

            apply_additional_owners_in_tx(&mut tx, managed.id, body.owner_handles.as_deref(), user_id)
                .await?;

            tx.commit().await.map_err(AppError::Db)?;
        }
        _ => {
            return Err(AppError::BadRequest(
                c.t(&loc, "ui.entity_private.entity_edit_not_supported"),
            ));
        }
    }

    entity::get_by_id(pool, managed.id)
        .await?
        .ok_or(AppError::NotFound)
}

pub async fn delete_entity_by_handle(
    pool: &MySqlPool,
    user_id: u64,
    handle: &str,
    password: &str,
) -> AppResult<()> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let managed = load_managed_entity_by_handle(pool, handle).await?;
    if !user_can_manage_entity(pool, user_id, managed.id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::NotFound);
    }
    if managed.entity_status_key.as_deref() == Some(ref_keys::KEY_ENTITY_STATUS_DEAD) {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.entity_delete_already_dead"),
        ));
    }
    if !matches!(
        managed.type_key.as_str(),
        ref_keys::KEY_ENTITY_TYPE_PROFIT
            | ref_keys::KEY_ENTITY_TYPE_NON_PROFIT
            | ref_keys::KEY_ENTITY_TYPE_GATHERING
    ) {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.entity_delete_not_allowed"),
        ));
    }

    let ok = crate::auth::account::verify_user_password(pool, user_id, password).await?;
    if !ok {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.account_password_incorrect"),
        ));
    }

    let mut tx = pool.begin().await.map_err(AppError::Db)?;
    let deleted =
        community_graph::extension_entity_death_voluntary_tx(&mut tx, managed.id, user_id)
            .await
            .map_err(AppError::Db)?;
    if !deleted {
        tx.rollback().await.map_err(AppError::Db)?;
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.entity_delete_failed"),
        ));
    }
    tx.commit().await.map_err(AppError::Db)?;
    Ok(())
}

async fn load_managed_entity_by_handle(pool: &MySqlPool, handle: &str) -> AppResult<ManagedEntity> {
    let Some(id) = sqlx::query_scalar::<_, u64>(
        r#"SELECT id FROM social_entity WHERE handle = ? LIMIT 1"#,
    )
    .bind(handle)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Db)?
    else {
        return Err(AppError::NotFound);
    };
    load_managed_entity(pool, id).await
}

async fn load_managed_entity(pool: &MySqlPool, entity_id: u64) -> AppResult<ManagedEntity> {
    let Some((handle, type_key, entity_status_key)) = sqlx::query_as::<_, (String, String, Option<String>)>(
        r#"
        SELECT
            se.handle,
            rk_type.`key`,
            (
                SELECT r.`key`
                FROM ref_key_values r
                WHERE r.id = COALESCE(se.entity_status_id, se.status_id)
                LIMIT 1
            )
        FROM social_entity se
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = se.type_id
           AND rk_type.`group` = ?
           AND rk_type.is_active = 1
        WHERE se.id = ?
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(entity_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Db)?
    else {
        return Err(AppError::NotFound);
    };

    Ok(ManagedEntity {
        id: entity_id,
        handle,
        type_key,
        entity_status_key,
    })
}

async fn fetch_additional_owner_handles_display(
    pool: &MySqlPool,
    entity_id: u64,
    creator_user_id: u64,
) -> AppResult<String> {
    let handles: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT se_h.handle
        FROM user_social_entity_role ur
        INNER JOIN ref_key_values rk_role
            ON rk_role.id = ur.role_id
           AND rk_role.`group` = 'user_social_entity_role'
           AND rk_role.`key` = 'owner'
           AND rk_role.is_active = 1
        INNER JOIN social_entity se_h
            ON se_h.created_by = ur.user_id
        INNER JOIN social_human sh
            ON sh.social_entity_id = se_h.id
        WHERE ur.social_entity_id = ?
          AND ur.user_id != ?
          AND (ur.ends_at IS NULL OR ur.ends_at >= UTC_DATE())
        ORDER BY se_h.handle ASC
        "#,
    )
    .bind(entity_id)
    .bind(creator_user_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::Db)?;

    Ok(handles
        .into_iter()
        .map(|h| format!("@{h}"))
        .collect::<Vec<_>>()
        .join(" "))
}

async fn resolve_brand_location_id(
    pool: &MySqlPool,
    entity_id: u64,
    body: &UpdateEntityProfileRequest,
    loc: &str,
    c: &crate::i18n::Catalog,
) -> AppResult<u64> {
    if let Some(raw) = body
        .location_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let parsed: u64 = raw
            .parse()
            .map_err(|_| AppError::BadRequest(c.t(loc, "human_profile.location_id_invalid")))?;
        let exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM locations WHERE id = ? LIMIT 1)"#,
        )
        .bind(parsed)
        .fetch_one(pool)
        .await
        .map_err(AppError::Db)?;
        if exists {
            return Ok(parsed);
        }
    }

    sqlx::query_scalar(r#"SELECT location_id FROM social_entity WHERE id = ? LIMIT 1"#)
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .map_err(AppError::Db)
}

async fn apply_additional_owners_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    entity_id: u64,
    raw_handles: Option<&str>,
    acting_user_id: u64,
) -> AppResult<()> {
    let handles = parse_owner_handles_raw(raw_handles);
    if handles.is_empty() {
        return Ok(());
    }
    entity::assign_additional_owner_roles_tx(tx, entity_id, handles, acting_user_id).await
}

fn parse_owner_handles_raw(raw: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let Some(s) = raw else {
        return out;
    };
    let s = s.trim().to_ascii_lowercase().replace(',', " ");
    for token in s.split_whitespace() {
        let h = token.trim_start_matches('@');
        if h.len() < 3 || h.len() > 32 {
            continue;
        }
        if h.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
            let hs = h.to_string();
            if seen.insert(hs.clone()) {
                out.push(hs);
            }
        }
    }
    out
}

fn trim_opt(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn required_field(
    raw: Option<&str>,
    max: usize,
    empty_i18n: &str,
    loc: &str,
) -> AppResult<String> {
    let t = raw.map(str::trim).unwrap_or("");
    if t.is_empty() {
        return Err(AppError::BadRequest(
            i18n::catalog().t(loc, empty_i18n),
        ));
    }
    if t.len() > max {
        return Err(AppError::BadRequest(
            i18n::catalog().t(loc, "social_entity.profit_field_too_long"),
        ));
    }
    Ok(t.to_string())
}

fn optional_tagline(raw: Option<&str>, loc: &str) -> AppResult<Option<String>> {
    let Some(s) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if s.len() > MAX_TAGLINE {
        return Err(AppError::BadRequest(
            i18n::catalog().t(loc, "social_entity.profit_text_too_long"),
        ));
    }
    Ok(Some(s.to_string()))
}

fn optional_rich_text(raw: Option<&str>, loc: &str) -> AppResult<Option<String>> {
    let Some(s) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let sanitized = content_sanitize::sanitize_post_markup(s).trim().to_string();
    if sanitized.is_empty() {
        return Ok(None);
    }
    if sanitized.chars().count() > MAX_TEXT_FIELD {
        return Err(AppError::BadRequest(
            i18n::catalog().t(loc, "social_entity.profit_text_too_long"),
        ));
    }
    Ok(Some(sanitized))
}

fn parse_founded_at(raw: Option<&str>, loc: &str) -> AppResult<NaiveDate> {
    let s = raw.map(str::trim).unwrap_or("");
    if s.is_empty() {
        return Err(AppError::BadRequest(
            i18n::catalog().t(loc, "social_entity.profit_founded_invalid"),
        ));
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| {
        AppError::BadRequest(i18n::catalog().t(loc, "social_entity.profit_founded_invalid"))
    })
}

async fn fetch_brand_website_and_logo_link_id(
    pool: &MySqlPool,
    entity_id: u64,
    type_key: &str,
) -> AppResult<(Option<String>, Option<u64>)> {
    let row: Option<(Option<String>, Option<u64>)> = match type_key {
        ref_keys::KEY_ENTITY_TYPE_PROFIT => {
            sqlx::query_as(
                r#"
                SELECT cl_web.url_canonical, sp.logo_content_link_id
                FROM social_profit sp
                LEFT JOIN content_link cl_web ON cl_web.id = sp.content_link_id
                WHERE sp.social_entity_id = ?
                LIMIT 1
                "#,
            )
            .bind(entity_id)
            .fetch_optional(pool)
            .await
            .map_err(AppError::Db)?
        }
        ref_keys::KEY_ENTITY_TYPE_NON_PROFIT => {
            sqlx::query_as(
                r#"
                SELECT cl_web.url_canonical, snp.logo_content_link_id
                FROM social_non_profit snp
                LEFT JOIN content_link cl_web ON cl_web.id = snp.content_link_id
                WHERE snp.social_entity_id = ?
                LIMIT 1
                "#,
            )
            .bind(entity_id)
            .fetch_optional(pool)
            .await
            .map_err(AppError::Db)?
        }
        _ => None,
    };
    Ok(row.unwrap_or((None, None)))
}

async fn resolve_logo_link_id_for_update(
    pool: &MySqlPool,
    config: &crate::config::Config,
    logo_url: Option<&str>,
    website_url: Option<&str>,
    existing_logo_link_id: Option<u64>,
    user_id: u64,
    loc: &str,
) -> AppResult<Option<u64>> {
    match logo_url.map(str::trim) {
        None => Ok(existing_logo_link_id),
        Some("") => Ok(None),
        Some(logo) => {
            brand_logo::optional_brand_logo_url_to_content_link_id(
                pool,
                config,
                Some(logo),
                website_url,
                user_id,
                loc,
            )
            .await
        }
    }
}
