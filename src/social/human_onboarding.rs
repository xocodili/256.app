//! First-time human profile for a verified user: one `social_entity` (type human) + `social_human` row.

use std::time::Duration;

use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;

use super::db;
use super::entity;
use crate::error::{AppError, AppResult};
use crate::google_maps::places::{self, PlacesError};
use crate::i18n;
use crate::locations::{self, CreateLocationRequest};
use crate::ref_key_values;
use crate::ref_keys;
use crate::social::noms::{self, ledger::LedgerReason};
use crate::status;
use crate::ui::countries;

const MAX_NAME_CHARS: usize = 128;
const MAX_ADDITIONAL_NAME_CHARS: usize = 256;
const MAX_PUBLIC_LOCATION_LABEL_CHARS: usize = 512;
pub const MIN_AGE_YEARS: i32 = 16;

/// Google Places resolution on human profile edit save (`PATCH /me/human-profile`).
pub struct HumanProfileGoogleLocationConfig<'a> {
    pub api_key: &'a str,
    pub timeout: Duration,
}

/// Bump when Root Charter / onboarding terms copy changes; stored on `users.terms_consent_version`.
pub const CURRENT_TERMS_CONSENT_VERSION: &str = "v2-2026-05-29";

#[derive(Debug, Clone, Serialize)]
pub struct HumanProfileEditSnapshot {
    pub given_name: String,
    pub family_name: String,
    pub additional_name: Option<String>,
    pub birth_date: NaiveDate,
    pub country_code: String,
    pub region: Option<String>,
    pub locality: Option<String>,
    pub public_location_label: String,
    pub website_url: Option<String>,
    pub user_email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateHumanProfileRequest {
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    #[serde(default)]
    pub additional_name: Option<String>,
    #[serde(default)]
    pub country_code: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub locality: Option<String>,
    #[serde(default)]
    pub public_location_label: Option<String>,
    #[serde(default)]
    pub website_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateHumanProfileRequest {
    pub handle: String,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    #[serde(default)]
    pub additional_name: Option<String>,
    pub birth_date: String,
    /// ISO 3166-1 alpha-2 from the profile form (required).
    #[serde(default)]
    pub country_code: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub locality: Option<String>,
    #[serde(default)]
    pub public_location_label: Option<String>,
}

/// `true` when this user accepted the current onboarding terms / Root Charter version.
pub async fn user_has_terms_consent(pool: &MySqlPool, user_id: u64) -> Result<bool, sqlx::Error> {
    let version: Option<String> = sqlx::query_scalar(
        "SELECT terms_consent_version FROM users WHERE id = ? LIMIT 1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(version.as_deref() == Some(CURRENT_TERMS_CONSENT_VERSION))
}

/// Record acceptance of [`CURRENT_TERMS_CONSENT_VERSION`] (idempotent).
pub async fn record_terms_consent(pool: &MySqlPool, user_id: u64) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE users
        SET terms_consent_version = ?, modified_by = ?
        WHERE id = ?
          AND (terms_consent_version IS NULL OR terms_consent_version <> ?)
        "#,
    )
    .bind(CURRENT_TERMS_CONSENT_VERSION)
    .bind(user_id)
    .bind(user_id)
    .bind(CURRENT_TERMS_CONSENT_VERSION)
    .execute(pool)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// `true` when this user has a human `social_entity` row (owner role or creator).
pub async fn user_has_human_profile(pool: &MySqlPool, user_id: u64) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM social_entity se
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        INNER JOIN ref_key_values t
            ON t.id = se.type_id
            AND t.`group` = ?
            AND t.`key` = ?
            AND t.is_active = 1
        LEFT JOIN user_social_entity_role ur
            ON ur.social_entity_id = se.id
           AND ur.user_id = ?
           AND ur.ends_at IS NULL
        WHERE ur.id IS NOT NULL OR se.created_by = ?
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .bind(user_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

/// Primary human `social_entity.id` for this user (owner role or `created_by`), if any.
pub async fn primary_human_entity_id(pool: &MySqlPool, user_id: u64) -> Result<Option<u64>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        LEFT JOIN user_social_entity_role ur
            ON ur.social_entity_id = se.id
           AND ur.user_id = ?
           AND ur.ends_at IS NULL
        WHERE ur.id IS NOT NULL OR se.created_by = ?
        ORDER BY
            CASE WHEN ur.is_primary = 1 THEN 0 ELSE 1 END,
            se.id ASC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Primary human `social_entity.id` inside an existing transaction.
pub async fn primary_human_entity_id_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    user_id: u64,
) -> Result<Option<u64>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        LEFT JOIN user_social_entity_role ur
            ON ur.social_entity_id = se.id
           AND ur.user_id = ?
           AND ur.ends_at IS NULL
        WHERE ur.id IS NOT NULL OR se.created_by = ?
        ORDER BY
            CASE WHEN ur.is_primary = 1 THEN 0 ELSE 1 END,
            se.id ASC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
}

/// Public `handle` of the viewer's human `social_entity`, when they have created one.
pub async fn human_entity_handle_for_user(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT se.handle
        FROM social_entity se
        INNER JOIN ref_key_values t
            ON t.id = se.type_id
            AND t.`group` = ?
            AND t.`key` = ?
            AND t.is_active = 1
        WHERE se.created_by = ?
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// AI token balance for the viewer's human profile (`social_human.ai_tokens`), defaulting to `0`.
pub async fn ai_tokens_for_user(pool: &MySqlPool, user_id: u64) -> Result<u16, sqlx::Error> {
    let Some(entity_id) = primary_human_entity_id(pool, user_id).await? else {
        return Ok(0);
    };
    let v: Option<u16> = sqlx::query_scalar("SELECT ai_tokens FROM social_human WHERE social_entity_id = ? LIMIT 1")
        .bind(entity_id)
        .fetch_optional(pool)
        .await?;
    Ok(v.unwrap_or(0))
}

/// SSR rating for the viewer's primary human `social_entity` (0.0 when absent/unset).
pub async fn ssr_rating_for_user(pool: &MySqlPool, user_id: u64) -> Result<Option<f32>, sqlx::Error> {
    let Some(entity_id) = primary_human_entity_id(pool, user_id).await? else {
        return Ok(None);
    };
    let v: Option<Decimal> =
        sqlx::query_scalar("SELECT rating FROM social_entity WHERE id = ? LIMIT 1")
            .bind(entity_id)
            .fetch_optional(pool)
            .await?;
    Ok(v.and_then(|d| d.to_string().parse::<f32>().ok()))
}

fn trim_name_field(raw: Option<String>, max_chars: usize) -> Option<String> {
    let raw = raw?;
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    Some(s.chars().take(max_chars).collect())
}

fn trim_country_code(raw: Option<String>) -> Option<String> {
    let raw = raw?;
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    Some(s.to_ascii_uppercase())
}

fn trim_public_location_label(raw: Option<String>) -> Option<String> {
    let s = trim_name_field(raw, MAX_PUBLIC_LOCATION_LABEL_CHARS)?;
    Some(s)
}

/// Human profile card: prefer `Country, Region, Locality` when structured fields exist.
#[must_use]
pub fn human_profile_location_label(
    country_code: Option<&str>,
    region: Option<&str>,
    locality: Option<&str>,
    stored_display_label: Option<&str>,
) -> Option<String> {
    let region = region.map(str::trim).filter(|s| !s.is_empty());
    let locality = locality.map(str::trim).filter(|s| !s.is_empty());

    if region.is_some() || locality.is_some() {
        let cc = country_code.map(str::trim).filter(|s| !s.is_empty())?;
        let country_name = crate::ui::countries::display_name_for_code(cc)
            .unwrap_or_else(|| cc.to_string());
        let mut parts = vec![country_name];
        if let Some(r) = region {
            parts.push(r.to_string());
        }
        if let Some(l) = locality {
            parts.push(l.to_string());
        }
        return Some(parts.join(", "));
    }

    stored_display_label
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn manual_human_location_request(
    cc: String,
    region: Option<String>,
    locality: Option<String>,
    public_location_label: Option<String>,
) -> CreateLocationRequest {
    let display_label = match trim_public_location_label(public_location_label) {
        Some(l) => l,
        None => countries::display_name_for_code(&cc).unwrap_or_else(|| cc.clone()),
    };
    CreateLocationRequest {
        country_code: cc,
        region,
        locality,
        postal_code: None,
        latitude: None,
        longitude: None,
        display_label,
        validation_source: "user_profile".to_string(),
        validation_ref: None,
        store_coordinates: false,
        response_coordinate_precision: locations::ResponseCoordinatePrecision::default(),
    }
}

async fn resolve_human_profile_location_request(
    cc: String,
    region: Option<String>,
    locality: Option<String>,
    public_location_label: Option<String>,
    google: Option<HumanProfileGoogleLocationConfig<'_>>,
) -> CreateLocationRequest {
    let manual = || {
        manual_human_location_request(
            cc.clone(),
            region.clone(),
            locality.clone(),
            public_location_label.clone(),
        )
    };

    let Some(g) = google else {
        return manual();
    };

    let country_name = countries::display_name_for_code(&cc).unwrap_or_else(|| cc.clone());
    match places::resolve_place_from_profile_fields(
        g.api_key,
        &cc,
        &country_name,
        region.as_deref(),
        locality.as_deref(),
        public_location_label.as_deref(),
        g.timeout,
    )
    .await
    {
        Ok(req) => req,
        Err(PlacesError::NotConfigured) => manual(),
        Err(PlacesError::UpstreamStatus(403)) => {
            tracing::warn!(
                country_code = %cc,
                "google places API returned 403 on human profile save — enable Places API (New) for this key in GCP; using manual location"
            );
            manual()
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                country_code = %cc,
                "google place resolve failed on human profile save; using manual location"
            );
            manual()
        }
    }
}

/// Whole calendar years completed (birthday not yet reached this year reduces the count).
fn completed_age_years(birth: NaiveDate, today: NaiveDate) -> i32 {
    let mut years = today.year() - birth.year();
    let today_md = (today.month(), today.day());
    let birth_md = (birth.month(), birth.day());
    if today_md < birth_md {
        years -= 1;
    }
    years
}

/// Latest calendar birth date (YYYY-MM-DD) so `completed_age_years(birth, today) >= min_years`.
#[must_use]
pub fn latest_birth_date_for_min_age(min_years: i32, today: NaiveDate) -> NaiveDate {
    let target_year = today.year() - min_years;
    NaiveDate::from_ymd_opt(target_year, today.month(), today.day())
        .or_else(|| NaiveDate::from_ymd_opt(target_year, today.month(), 28))
        .unwrap_or(today)
}

/// Creates location (deduped), `social_entity` (human, verified), and `social_human` in one transaction.
pub async fn create_human_profile(
    pool: &MySqlPool,
    user_id: u64,
    body: CreateHumanProfileRequest,
) -> AppResult<entity::Public> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();

    if user_has_human_profile(pool, user_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::Conflict(
            c.t(&loc, "human_profile.already_completed"),
        ));
    }

    let handle = entity::normalize_handle_for_create(&body.handle)?;

    let birth = NaiveDate::parse_from_str(body.birth_date.trim(), "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest(c.t(&loc, "human_profile.birth_date_invalid")))?;
    let today = Utc::now().date_naive();
    if birth > today {
        return Err(AppError::BadRequest(
            c.t(&loc, "human_profile.birth_date_future"),
        ));
    }

    if completed_age_years(birth, today) < MIN_AGE_YEARS {
        return Err(AppError::BadRequest(
            c.t(&loc, "human_profile.birth_date_too_young"),
        ));
    }

    let (given_name, family_name) = match (
        trim_name_field(body.given_name, MAX_NAME_CHARS),
        trim_name_field(body.family_name, MAX_NAME_CHARS),
    ) {
        (Some(g), Some(f)) => (g, f),
        _ => {
            return Err(AppError::BadRequest(
                c.t(&loc, "human_profile.names_required"),
            ));
        }
    };
    let additional_name = trim_name_field(body.additional_name, MAX_ADDITIONAL_NAME_CHARS);

    let cc = trim_country_code(body.country_code)
        .ok_or_else(|| AppError::BadRequest(c.t(&loc, "human_profile.country_required")))?;

    let display_label = match trim_public_location_label(body.public_location_label) {
        Some(l) => l,
        None => countries::display_name_for_code(&cc).unwrap_or_else(|| cc.clone()),
    };
    let loc_req = CreateLocationRequest {
        country_code: cc,
        region: body.region,
        locality: body.locality,
        postal_code: None,
        latitude: None,
        longitude: None,
        display_label,
        validation_source: "user_profile".to_string(),
        validation_ref: None,
        store_coordinates: false,
        response_coordinate_precision: locations::ResponseCoordinatePrecision::default(),
    };
    let loc_row = locations::create_or_get(pool, loc_req, user_id).await?;
    let location_id: u64 = loc_row
        .id
        .parse()
        .map_err(|_| AppError::BadRequest(c.t(&loc, "human_profile.location_id_invalid")))?;

    let type_id = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_SOCIAL_ENTITY_TYPE,
        ref_keys::KEY_ENTITY_TYPE_HUMAN,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "social_entity.type_ref_invalid")))?;

    let status_id =
        ref_key_values::lookup_active_id(pool, ref_keys::GROUP_STATUS, ref_keys::KEY_VERIFIED)
            .await
            .map_err(AppError::Db)?
            .ok_or_else(|| AppError::BadRequest(c.t(&loc, "social_entity.status_ref_invalid")))?;

    let join_policy_id = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_SOCIAL_JOIN_POLICY,
        ref_keys::KEY_JOIN_POLICY_OPEN,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "human_profile.join_policy_missing")))?;

    if !db::ref_row_active(pool, type_id, ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            c.t(&loc, "social_entity.type_ref_invalid"),
        ));
    }
    if !db::ref_row_active(pool, status_id, ref_keys::GROUP_STATUS)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            c.t(&loc, "social_entity.status_ref_invalid"),
        ));
    }
    if !db::ref_row_active(pool, join_policy_id, ref_keys::GROUP_SOCIAL_JOIN_POLICY)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            c.t(&loc, "human_profile.join_policy_missing"),
        ));
    }

    let mut tx = pool.begin().await.map_err(AppError::Db)?;

    let res = sqlx::query(
        r#"
        INSERT INTO social_entity (
            handle, type_id, status_id, location_id,
            rating, created_by, modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&handle)
    .bind(type_id)
    .bind(status_id)
    .bind(location_id)
    .bind(Decimal::new(250, 2))
    .bind(user_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await;

    let res = match res {
        Ok(r) => r,
        Err(e) => {
            if db::is_unique_violation(&e) {
                return Err(AppError::Conflict(c.t(&loc, "social_entity.handle_taken")));
            }
            if db::is_foreign_key_violation(&e) {
                return Err(AppError::BadRequest(
                    c.t(&loc, "social_entity.fk_violation"),
                ));
            }
            return Err(AppError::Db(e));
        }
    };

    let entity_id = res.last_insert_id();

    sqlx::query(
        r#"
        INSERT INTO social_human (
            social_entity_id, family_name, given_name, additional_name,
            birth_date, birth_location_id, join_policy_id, ai_tokens
        )
        VALUES (?, ?, ?, ?, ?, NULL, ?, ?)
        "#,
    )
    .bind(entity_id)
    .bind(&family_name)
    .bind(&given_name)
    .bind(additional_name.as_deref())
    .bind(birth)
    .bind(join_policy_id)
    .bind(noms::SIGNUP_NOM_GRANT)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if db::is_foreign_key_violation(&e) {
            AppError::BadRequest(c.t(&loc, "social_entity.fk_violation"))
        } else {
            AppError::Db(e)
        }
    })?;

    noms::ledger::append_entry_tx(
        &mut tx,
        entity_id,
        noms::SIGNUP_NOM_GRANT as i16,
        noms::SIGNUP_NOM_GRANT,
        LedgerReason::CreditSignup,
        None,
        Some("signup"),
        Some(user_id),
    )
    .await
    .map_err(AppError::Db)?;

    tx.commit().await.map_err(AppError::Db)?;
    super::ownership::ensure_owner_role(pool, user_id, entity_id)
        .await
        .map_err(AppError::Db)?;
    status::try_promote_user_to_verified_if_composite(pool, user_id, Some(user_id)).await?;

    entity::get_by_id(pool, entity_id)
        .await?
        .ok_or(AppError::NotFound)
}

/// ISO country code on the viewer's primary human profile, if set.
pub async fn country_code_for_user(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<Option<String>, sqlx::Error> {
    let row: Option<String> = sqlx::query_scalar(
        r#"
        SELECT loc.country_code
        FROM users u
        INNER JOIN social_entity se ON se.created_by = u.id
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        INNER JOIN locations loc ON loc.id = se.location_id
        WHERE u.id = ?
        ORDER BY se.id ASC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row
        .map(|s| s.trim().to_ascii_uppercase())
        .filter(|s| !s.is_empty()))
}

/// Editable human profile fields for the signed-in owner (private profile card).
pub async fn fetch_human_profile_edit_snapshot(
    pool: &MySqlPool,
    user_id: u64,
    entity_id: u64,
) -> AppResult<HumanProfileEditSnapshot> {
    let Some(row) = sqlx::query_as::<
        _,
        (
            String,
            String,
            Option<String>,
            NaiveDate,
            String,
            Option<String>,
            Option<String>,
            String,
            Option<String>,
        ),
    >(
        r#"
        SELECT
            sh.given_name,
            sh.family_name,
            sh.additional_name,
            sh.birth_date,
            loc.country_code,
            loc.region,
            loc.locality,
            loc.display_label,
            cl_web.url_canonical
        FROM social_entity se
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        INNER JOIN locations loc ON loc.id = se.location_id
        LEFT JOIN content_link cl_web ON cl_web.id = se.profile_content_link_id
        WHERE se.id = ?
          AND se.created_by = ?
        LIMIT 1
        "#,
    )
    .bind(entity_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Db)?
    else {
        return Err(AppError::NotFound);
    };

    let user_email: Option<String> = sqlx::query_scalar(
        r#"SELECT email FROM users WHERE id = ? LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Db)?;

    let (
        given_name,
        family_name,
        additional_name,
        birth_date,
        country_code,
        region,
        locality,
        public_location_label,
        website_url,
    ) = row;

    Ok(HumanProfileEditSnapshot {
        given_name,
        family_name,
        additional_name: additional_name.filter(|s| !s.trim().is_empty()),
        birth_date,
        country_code: country_code.trim().to_ascii_uppercase(),
        region: region.filter(|s| !s.trim().is_empty()),
        locality: locality.filter(|s| !s.trim().is_empty()),
        public_location_label,
        website_url: website_url
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        user_email,
    })
}

/// Updates the viewer's primary human profile names and location (handle and birth date stay fixed).
pub async fn update_human_profile(
    pool: &MySqlPool,
    user_id: u64,
    body: UpdateHumanProfileRequest,
    google: Option<HumanProfileGoogleLocationConfig<'_>>,
) -> AppResult<entity::Public> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();

    let Some(entity_id) = primary_human_entity_id(pool, user_id).await.map_err(AppError::Db)? else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.human_profile_required"),
        ));
    };

    let (given_name, family_name) = match (
        trim_name_field(body.given_name, MAX_NAME_CHARS),
        trim_name_field(body.family_name, MAX_NAME_CHARS),
    ) {
        (Some(g), Some(f)) => (g, f),
        _ => {
            return Err(AppError::BadRequest(
                c.t(&loc, "human_profile.names_required"),
            ));
        }
    };
    let additional_name = trim_name_field(body.additional_name, MAX_ADDITIONAL_NAME_CHARS);

    let cc = trim_country_code(body.country_code)
        .ok_or_else(|| AppError::BadRequest(c.t(&loc, "human_profile.country_required")))?;

    let loc_req = resolve_human_profile_location_request(
        cc,
        body.region,
        body.locality,
        body.public_location_label,
        google,
    )
    .await;
    let loc_row = locations::create_or_get(pool, loc_req, user_id).await?;
    let location_id: u64 = loc_row
        .id
        .parse()
        .map_err(|_| AppError::BadRequest(c.t(&loc, "human_profile.location_id_invalid")))?;

    let profile_link_id = entity::optional_brand_url_to_content_link_id(
        pool,
        body.website_url.as_deref(),
        user_id,
        &loc,
        false,
    )
    .await?;

    let mut tx = pool.begin().await.map_err(AppError::Db)?;

    sqlx::query(
        r#"
        UPDATE social_human
        SET given_name = ?, family_name = ?, additional_name = ?
        WHERE social_entity_id = ?
        "#,
    )
    .bind(&given_name)
    .bind(&family_name)
    .bind(additional_name.as_deref())
    .bind(entity_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;

    sqlx::query(
        r#"
        UPDATE social_entity
        SET location_id = ?, profile_content_link_id = ?, modified_by = ?
        WHERE id = ? AND created_by = ?
        "#,
    )
    .bind(location_id)
    .bind(profile_link_id)
    .bind(user_id)
    .bind(entity_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;

    tx.commit().await.map_err(AppError::Db)?;

    entity::get_by_id(pool, entity_id)
        .await?
        .ok_or(AppError::NotFound)
}

#[cfg(test)]
mod location_label_tests {
    use super::human_profile_location_label;

    #[test]
    fn human_profile_location_label_prefers_structured_country_region_locality() {
        assert_eq!(
            human_profile_location_label(
                Some("AU"),
                Some("NSW"),
                Some("Leura"),
                Some("Australia"),
            ),
            Some("Australia, NSW, Leura".to_string())
        );
    }

    #[test]
    fn human_profile_location_label_falls_back_to_stored_label() {
        assert_eq!(
            human_profile_location_label(
                Some("AU"),
                None,
                None,
                Some("Sydney, NSW, Australia"),
            ),
            Some("Sydney, NSW, Australia".to_string())
        );
    }
}
