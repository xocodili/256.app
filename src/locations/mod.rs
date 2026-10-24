//! Validated geographic locations: deduplicated rows you own, with privacy-minded defaults.
//!
//! - **Coordinates** are optional; callers can validate with a third party but choose not to persist
//!   precise points ([`CreateLocationRequest::store_coordinates`]).
//! - **API responses** default to omitting coordinates ([`ResponseCoordinatePrecision::Omit`]); when
//!   included, values are **rounded** so responses are not more precise than needed.
//! - **Validator metadata** (`validation_ref`) is stored for audit but **never** returned in JSON.
//! - **Logging** avoids writing coordinates or full labels at `info` level.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::MySqlPool;

use crate::crypto::sha3;
use crate::db::TimestampUtc;
use crate::error::{AppError, AppResult};
use crate::i18n;
use crate::ref_key_values;
use crate::ref_keys;

pub mod lifecycle;

const MAX_REGION_LEN: usize = 128;
const MAX_LOCALITY_LEN: usize = 128;
const MAX_POSTAL_LEN: usize = 32;
const MAX_LABEL_LEN: usize = 512;
const MAX_VALIDATION_SOURCE_LEN: usize = 64;
const MAX_VALIDATION_REF_LEN: usize = 255;

/// Fingerprint format version (prefix byte) so future normalizations can coexist.
const FINGERPRINT_VERSION: u8 = 1;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseCoordinatePrecision {
    Omit,
    City,
    Neighborhood,
    Full,
}

impl Default for ResponseCoordinatePrecision {
    fn default() -> Self {
        Self::Omit
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateLocationRequest {
    pub country_code: String,
    pub region: Option<String>,
    pub locality: Option<String>,
    pub postal_code: Option<String>,
    pub latitude: Option<Decimal>,
    pub longitude: Option<Decimal>,
    pub display_label: String,
    pub validation_source: String,
    pub validation_ref: Option<String>,
    /// When false (default), lat/lon are cleared before storage even if supplied.
    #[serde(default)]
    pub store_coordinates: bool,
    #[serde(default)]
    pub response_coordinate_precision: ResponseCoordinatePrecision,
}

#[derive(Debug, Serialize)]
pub struct CoordinatesPublic {
    pub latitude: String,
    pub longitude: String,
}

#[derive(Debug, Serialize)]
pub struct LocationPublic {
    /// String so JSON clients do not lose precision above 2^53−1.
    pub id: String,
    pub country_code: String,
    pub region: Option<String>,
    pub locality: Option<String>,
    pub postal_code: Option<String>,
    pub coordinates: Option<CoordinatesPublic>,
    pub display_label: String,
    pub validated_at: TimestampUtc,
}

#[derive(Debug, Clone, FromRow)]
struct LocationRow {
    id: u64,
    country_code: String,
    region: Option<String>,
    locality: Option<String>,
    postal_code: Option<String>,
    latitude: Option<Decimal>,
    longitude: Option<Decimal>,
    display_label: String,
    validated_at: TimestampUtc,
}

fn normalize_country(code: &str) -> AppResult<String> {
    let loc = i18n::request_locale();
    let c = code.trim();
    if c.len() != 2 || !c.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "location.country_code"),
        ));
    }
    Ok(c.to_ascii_uppercase())
}

fn trim_opt_max(s: Option<String>, max: usize, field: &'static str) -> AppResult<Option<String>> {
    let Some(s) = s else {
        return Ok(None);
    };
    let t = s.trim();
    if t.is_empty() {
        return Ok(None);
    }
    if t.len() > max {
        let loc = i18n::request_locale();
        return Err(AppError::BadRequest(i18n::catalog().tf(
            &loc,
            "location.field_too_long",
            &[("field", field)],
        )));
    }
    Ok(Some(t.to_string()))
}

fn validate_validation_source(src: &str) -> AppResult<String> {
    let loc = i18n::request_locale();
    let t = src.trim();
    if t.is_empty() || t.len() > MAX_VALIDATION_SOURCE_LEN {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "location.validation_source_len"),
        ));
    }
    if !t
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
    {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "location.validation_source_chars"),
        ));
    }
    Ok(t.to_ascii_lowercase())
}

fn validate_validation_ref(r: Option<String>) -> AppResult<Option<String>> {
    let Some(s) = r else {
        return Ok(None);
    };
    let t = s.trim();
    if t.is_empty() {
        return Ok(None);
    }
    if t.len() > MAX_VALIDATION_REF_LEN {
        let loc = i18n::request_locale();
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "location.validation_ref_too_long"),
        ));
    }
    Ok(Some(t.to_string()))
}

fn validate_coordinates(lat: Option<Decimal>, lon: Option<Decimal>) -> AppResult<()> {
    let c = i18n::catalog();
    let loc = i18n::request_locale();
    match (lat, lon) {
        (None, None) => Ok(()),
        (Some(_), None) | (None, Some(_)) => {
            Err(AppError::BadRequest(c.t(&loc, "location.lat_lon_both")))
        }
        (Some(lat), Some(lon)) => {
            if lat < dec_neg_90() || lat > dec_90() {
                return Err(AppError::BadRequest(c.t(&loc, "location.latitude_range")));
            }
            if lon < dec_neg_180() || lon > dec_180() {
                return Err(AppError::BadRequest(c.t(&loc, "location.longitude_range")));
            }
            Ok(())
        }
    }
}

fn dec_90() -> Decimal {
    Decimal::new(90, 0)
}
fn dec_neg_90() -> Decimal {
    Decimal::new(-90, 0)
}
fn dec_180() -> Decimal {
    Decimal::new(180, 0)
}
fn dec_neg_180() -> Decimal {
    Decimal::new(-180, 0)
}

/// Canonical bytes for [`sha3::digest_256`]; only reflects fields that are **persisted**.
fn fingerprint_payload(
    version: u8,
    country: &str,
    region: Option<&str>,
    locality: Option<&str>,
    postal: Option<&str>,
    lat: Option<Decimal>,
    lon: Option<Decimal>,
) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(version);
    out.extend_from_slice(country.as_bytes());
    out.push(0x1f);
    push_opt(&mut out, region);
    out.push(0x1f);
    push_opt(&mut out, locality);
    out.push(0x1f);
    push_opt(&mut out, postal);
    out.push(0x1f);
    if let (Some(la), Some(lo)) = (lat, lon) {
        let coords = format!("{:.6},{:.6}", la.normalize(), lo.normalize());
        out.extend_from_slice(coords.as_bytes());
    }
    out
}

fn push_opt(buf: &mut Vec<u8>, s: Option<&str>) {
    if let Some(t) = s {
        buf.extend_from_slice(t.as_bytes());
    }
}

fn row_to_public(row: &LocationRow, precision: &ResponseCoordinatePrecision) -> LocationPublic {
    let coordinates = match precision {
        ResponseCoordinatePrecision::Omit => None,
        ResponseCoordinatePrecision::City => {
            row.latitude
                .zip(row.longitude)
                .map(|(lat, lon)| CoordinatesPublic {
                    latitude: lat.round_dp(2).normalize().to_string(),
                    longitude: lon.round_dp(2).normalize().to_string(),
                })
        }
        ResponseCoordinatePrecision::Neighborhood => {
            row.latitude
                .zip(row.longitude)
                .map(|(lat, lon)| CoordinatesPublic {
                    latitude: lat.round_dp(3).normalize().to_string(),
                    longitude: lon.round_dp(3).normalize().to_string(),
                })
        }
        ResponseCoordinatePrecision::Full => {
            row.latitude
                .zip(row.longitude)
                .map(|(lat, lon)| CoordinatesPublic {
                    latitude: lat.normalize().to_string(),
                    longitude: lon.normalize().to_string(),
                })
        }
    };

    LocationPublic {
        id: row.id.to_string(),
        country_code: row.country_code.clone(),
        region: row.region.clone(),
        locality: row.locality.clone(),
        postal_code: row.postal_code.clone(),
        coordinates,
        display_label: row.display_label.clone(),
        validated_at: row.validated_at,
    }
}

/// Insert or return an existing row with the same canonical fingerprint.
pub async fn create_or_get(
    pool: &MySqlPool,
    req: CreateLocationRequest,
    acting_user_id: u64,
) -> AppResult<LocationPublic> {
    let country_code = normalize_country(&req.country_code)?;
    let region = trim_opt_max(req.region, MAX_REGION_LEN, "region")?;
    let locality = trim_opt_max(req.locality, MAX_LOCALITY_LEN, "locality")?;
    let postal_code = trim_opt_max(req.postal_code, MAX_POSTAL_LEN, "postal_code")?;

    let label = req.display_label.trim();
    if label.is_empty() || label.len() > MAX_LABEL_LEN {
        let loc = i18n::request_locale();
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "location.display_label_len"),
        ));
    }
    let display_label = label.to_string();

    validate_coordinates(req.latitude, req.longitude)?;

    let (lat_stored, lon_stored) = if req.store_coordinates {
        (req.latitude, req.longitude)
    } else {
        (None, None)
    };

    let validation_source = validate_validation_source(&req.validation_source)?;
    let validation_ref = validate_validation_ref(req.validation_ref)?;

    let fp = sha3::digest_256(&fingerprint_payload(
        FINGERPRINT_VERSION,
        &country_code,
        region.as_deref(),
        locality.as_deref(),
        postal_code.as_deref(),
        lat_stored,
        lon_stored,
    ));

    tracing::info!(
        country = %country_code,
        has_stored_coordinates = lat_stored.is_some(),
        validation_source = %validation_source,
        "location upsert"
    );

    let active_lifecycle_id = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_LOCATION_LIFECYCLE,
        ref_keys::KEY_LOCATION_LIFECYCLE_ACTIVE,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or(AppError::NotFound)?;

    let mut tx = pool.begin().await?;

    let existing: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT l.id
        FROM locations l
        INNER JOIN ref_key_values rk ON rk.id = l.location_lifecycle_id
        WHERE l.canonical_fingerprint = ?
          AND rk.`group` = ?
          AND rk.`key` = ?
          AND rk.is_active = 1
        LIMIT 1
        "#,
    )
    .bind(fp.as_slice())
    .bind(ref_keys::GROUP_LOCATION_LIFECYCLE)
    .bind(ref_keys::KEY_LOCATION_LIFECYCLE_ACTIVE)
    .fetch_optional(&mut *tx)
    .await?;

    let id = if let Some(id) = existing {
        if crate::verification::is_google_maps_validation_source(&validation_source)
            && validation_ref.is_some()
        {
            sqlx::query(
                r#"
                UPDATE locations
                SET validation_source = ?,
                    validation_ref = ?,
                    latitude = COALESCE(?, latitude),
                    longitude = COALESCE(?, longitude),
                    display_label = ?,
                    region = COALESCE(?, region),
                    locality = COALESCE(?, locality),
                    postal_code = COALESCE(?, postal_code),
                    validated_at = UTC_TIMESTAMP(),
                    modified_by = ?
                WHERE id = ?
                "#,
            )
            .bind(&validation_source)
            .bind(validation_ref.as_deref())
            .bind(lat_stored)
            .bind(lon_stored)
            .bind(&display_label)
            .bind(region.as_deref())
            .bind(locality.as_deref())
            .bind(postal_code.as_deref())
            .bind(acting_user_id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
        id
    } else {
        let res = sqlx::query(
            r#"
            INSERT INTO locations (
                country_code, region, locality, postal_code,
                latitude, longitude, display_label,
                validation_source, validation_ref, validated_at,
                canonical_fingerprint, location_lifecycle_id, created_by, modified_by
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, UTC_TIMESTAMP(), ?, ?, ?, ?)
            "#,
        )
        .bind(&country_code)
        .bind(&region)
        .bind(&locality)
        .bind(&postal_code)
        .bind(lat_stored)
        .bind(lon_stored)
        .bind(&display_label)
        .bind(&validation_source)
        .bind(&validation_ref)
        .bind(fp.as_slice())
        .bind(active_lifecycle_id)
        .bind(acting_user_id)
        .bind(acting_user_id)
        .execute(&mut *tx)
        .await?;
        res.last_insert_id()
    };

    tx.commit().await?;

    let row = sqlx::query_as::<_, LocationRow>(
        r#"
        SELECT
            id, country_code, region, locality, postal_code,
            latitude, longitude, display_label, validated_at
        FROM locations WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(row_to_public(&row, &req.response_coordinate_precision))
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn fingerprint_stable_for_same_place() {
        let a = sha3::digest_256(&fingerprint_payload(
            FINGERPRINT_VERSION,
            "US",
            Some("TX"),
            Some("Austin"),
            Some("78701"),
            Some(d("30.2672")),
            Some(d("-97.7431")),
        ));
        let b = sha3::digest_256(&fingerprint_payload(
            FINGERPRINT_VERSION,
            "US",
            Some("TX"),
            Some("Austin"),
            Some("78701"),
            Some(d("30.2672")),
            Some(d("-97.7431")),
        ));
        assert_eq!(a, b);
    }

    #[test]
    fn fingerprint_differs_when_coords_omitted() {
        let with_coords = sha3::digest_256(&fingerprint_payload(
            FINGERPRINT_VERSION,
            "US",
            Some("TX"),
            Some("Austin"),
            None,
            Some(d("30.0")),
            Some(d("-97.0")),
        ));
        let without = sha3::digest_256(&fingerprint_payload(
            FINGERPRINT_VERSION,
            "US",
            Some("TX"),
            Some("Austin"),
            None,
            None,
            None,
        ));
        assert_ne!(with_coords, without);
    }
}
