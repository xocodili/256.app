//! [Places API (New)](https://developers.google.com/maps/documentation/places/web-service/place-details) — Place Details by id.

use std::str::FromStr;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::locations::{CreateLocationRequest, ResponseCoordinatePrecision};

const PLACES_BASE: &str = "https://places.googleapis.com/v1/places/";
const PLACES_AUTOCOMPLETE: &str = "https://places.googleapis.com/v1/places:autocomplete";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LatLng {
    latitude: Option<f64>,
    longitude: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddressComponent {
    long_text: Option<String>,
    short_text: Option<String>,
    types: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaceDetailsBody {
    id: Option<String>,
    formatted_address: Option<String>,
    location: Option<LatLng>,
    address_components: Option<Vec<AddressComponent>>,
}

/// Google returns `LocalizedText` as `{ "text": "...", "matches": [...] }`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AutocompleteLocalizedText {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AutocompletePlacePrediction {
    place: Option<String>,
    place_id: Option<String>,
    text: Option<AutocompleteLocalizedText>,
    structured_format: Option<AutocompleteStructuredFormat>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AutocompleteStructuredFormat {
    main_text: Option<AutocompleteLocalizedText>,
    secondary_text: Option<AutocompleteLocalizedText>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AutocompleteSuggestion {
    place_prediction: Option<AutocompletePlacePrediction>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AutocompleteBody {
    suggestions: Option<Vec<AutocompleteSuggestion>>,
}

#[derive(Debug, Clone)]
pub struct PlaceSuggestion {
    pub place_id: String,
    pub main_text: String,
    pub secondary_text: Option<String>,
}

/// Normalize client input: strip optional `places/` prefix and trim.
pub fn normalize_place_id(raw: &str) -> String {
    let t = raw.trim();
    t.strip_prefix("places/")
        .map(str::trim)
        .unwrap_or(t)
        .to_string()
}

fn place_id_syntax_ok(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 256
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn decimal_from_f64(v: f64) -> Option<Decimal> {
    if !v.is_finite() {
        return None;
    }
    Decimal::try_from(v).ok().or_else(|| Decimal::from_str(&format!("{v:.10}")).ok())
}

/// Fetches Place Details and maps into a [`CreateLocationRequest`] suitable for [`crate::locations::create_or_get`].
///
/// Caller supplies `acting_user_id` only for logging context; it is not sent to Google.
pub async fn build_create_location_from_place_id(
    api_key: &str,
    place_id: &str,
    timeout: Duration,
) -> Result<CreateLocationRequest, PlacesError> {
    if api_key.trim().is_empty() {
        return Err(PlacesError::NotConfigured);
    }
    let pid = normalize_place_id(place_id);
    if !place_id_syntax_ok(&pid) {
        return Err(PlacesError::InvalidPlaceId);
    }

    let url = format!("{PLACES_BASE}{pid}");
    let mut headers = HeaderMap::new();
    headers.insert(
        "X-Goog-FieldMask",
        HeaderValue::from_static("id,formattedAddress,addressComponents,location"),
    );
    headers.insert(
        reqwest::header::ACCEPT,
        HeaderValue::from_static("application/json"),
    );

    let client = crate::google_maps::http_client();
    let res = client
        .get(url)
        .header("X-Goog-Api-Key", api_key.trim())
        .headers(headers)
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "google places request failed");
            PlacesError::Upstream
        })?;

    let status = res.status();
    let body_bytes = res.bytes().await.map_err(|e| {
        tracing::warn!(error = %e, "google places body read failed");
        PlacesError::Upstream
    })?;

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(PlacesError::NotFound);
    }
    if !status.is_success() {
        tracing::warn!(status = %status, len = body_bytes.len(), "google places non-success");
        return Err(PlacesError::UpstreamStatus(status.as_u16()));
    }

    let parsed: PlaceDetailsBody = serde_json::from_slice(&body_bytes).map_err(|e| {
        tracing::warn!(error = %e, "google places json decode failed");
        PlacesError::BadPayload
    })?;

    let country_code = extract_country_iso(&parsed).ok_or(PlacesError::MissingCountry)?;
    let region = extract_first_type(&parsed, "administrative_area_level_1");
    let locality = extract_locality(&parsed);
    let postal_code = extract_first_type(&parsed, "postal_code");

    let (lat_dec, lon_dec) = match &parsed.location {
        Some(loc) => {
            let la = loc.latitude;
            let lo = loc.longitude;
            match (la, lo) {
                (Some(lat), Some(lon)) => {
                    let la_d = decimal_from_f64(lat).ok_or(PlacesError::BadCoordinates)?;
                    let lo_d = decimal_from_f64(lon).ok_or(PlacesError::BadCoordinates)?;
                    (Some(la_d), Some(lo_d))
                }
                _ => (None, None),
            }
        }
        None => (None, None),
    };

    let validation_ref = parsed
        .id
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| pid.clone());

    let display_label = parsed
        .formatted_address
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("{country_code} (Google Place)"))
        .chars()
        .take(512)
        .collect::<String>();

    Ok(CreateLocationRequest {
        country_code,
        region,
        locality,
        postal_code,
        latitude: lat_dec,
        longitude: lon_dec,
        display_label,
        validation_source: "google.places".to_string(),
        validation_ref: Some(validation_ref),
        store_coordinates: lat_dec.is_some() && lon_dec.is_some(),
        response_coordinate_precision: ResponseCoordinatePrecision::Omit,
    })
}

fn extract_country_iso(p: &PlaceDetailsBody) -> Option<String> {
    let comps = p.address_components.as_ref()?;
    for c in comps {
        let types = c.types.as_ref()?;
        if types.iter().any(|t| t == "country") {
            let short = c.short_text.as_deref()?.trim();
            if short.len() == 2 && short.chars().all(|ch| ch.is_ascii_alphabetic()) {
                return Some(short.to_ascii_uppercase());
            }
        }
    }
    None
}

fn extract_first_type(p: &PlaceDetailsBody, want: &str) -> Option<String> {
    let comps = p.address_components.as_ref()?;
    for c in comps {
        let types = c.types.as_ref()?;
        if types.iter().any(|t| t == want) {
            let s = c
                .long_text
                .as_deref()
                .or(c.short_text.as_deref())?
                .trim();
            if !s.is_empty() {
                return Some(s.chars().take(128).collect());
            }
        }
    }
    None
}

fn extract_locality(p: &PlaceDetailsBody) -> Option<String> {
    for want in ["locality", "postal_town", "sublocality", "sublocality_level_1"] {
        if let Some(s) = extract_first_type(p, want) {
            return Some(s);
        }
    }
    None
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum PlacesError {
    #[error("maps api key not configured")]
    NotConfigured,
    #[error("invalid place id")]
    InvalidPlaceId,
    #[error("place not found")]
    NotFound,
    #[error("upstream maps error")]
    Upstream,
    #[error("upstream maps error: HTTP {0}")]
    UpstreamStatus(u16),
    #[error("unexpected response payload")]
    BadPayload,
    #[error("country not returned for place")]
    MissingCountry,
    #[error("coordinates invalid or missing")]
    BadCoordinates,
    #[error("query too short")]
    QueryTooShort,
}

/// Places Autocomplete suggestions with optional country bias (`ISO-3166 alpha-2`).
pub async fn autocomplete_places(
    api_key: &str,
    input: &str,
    country_code: Option<&str>,
    timeout: Duration,
) -> Result<Vec<PlaceSuggestion>, PlacesError> {
    if api_key.trim().is_empty() {
        return Err(PlacesError::NotConfigured);
    }
    let needle = input.trim();
    if needle.chars().count() < 5 {
        return Err(PlacesError::QueryTooShort);
    }

    let mut body = serde_json::json!({
        "input": needle,
        "languageCode": "en",
        "includeQueryPredictions": false,
    });
    if let Some(cc) = country_code {
        // Places API expects ISO-3166 alpha-2; bias is best-effort (skip invalid values).
        let c = cc.trim().to_ascii_uppercase();
        if c.len() == 2 && c.chars().all(|ch| ch.is_ascii_alphabetic()) {
            body["includedRegionCodes"] = serde_json::json!([c]);
        }
    }

    let mut headers = HeaderMap::new();
    // Field paths must match Places API (New) leaf fields; wrong masks yield HTTP 400.
    headers.insert(
        "X-Goog-FieldMask",
        HeaderValue::from_static(
            "suggestions.placePrediction.place,suggestions.placePrediction.placeId,suggestions.placePrediction.text,suggestions.placePrediction.structuredFormat.mainText,suggestions.placePrediction.structuredFormat.secondaryText",
        ),
    );
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );

    let client = crate::google_maps::http_client();
    let res = client
        .post(PLACES_AUTOCOMPLETE)
        .header("X-Goog-Api-Key", api_key.trim())
        .headers(headers)
        .timeout(timeout)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "google places autocomplete request failed");
            PlacesError::Upstream
        })?;
    let status = res.status();
    let body_bytes = res.bytes().await.map_err(|e| {
        tracing::warn!(error = %e, "google places autocomplete body read failed");
        PlacesError::Upstream
    })?;
    if !status.is_success() {
        let snippet = String::from_utf8_lossy(&body_bytes[..body_bytes.len().min(512)]);
        tracing::warn!(
            status = %status,
            len = body_bytes.len(),
            body_snippet = %snippet,
            "google places autocomplete non-success"
        );
        return Err(PlacesError::UpstreamStatus(status.as_u16()));
    }
    let parsed: AutocompleteBody = serde_json::from_slice(&body_bytes).map_err(|e| {
        tracing::warn!(error = %e, "google places autocomplete json decode failed");
        PlacesError::BadPayload
    })?;

    let mut out = Vec::new();
    for s in parsed.suggestions.unwrap_or_default() {
        let Some(pred) = s.place_prediction else { continue };
        let place_id = pred
            .place_id
            .or_else(|| pred.place.as_deref().map(normalize_place_id))
            .unwrap_or_default();
        if !place_id_syntax_ok(place_id.as_str()) {
            continue;
        }
        let main_text = pred
            .structured_format
            .as_ref()
            .and_then(|f| f.main_text.as_ref())
            .and_then(|t| t.text.as_ref())
            .or_else(|| pred.text.as_ref().and_then(|t| t.text.as_ref()))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Address".to_string());
        let secondary_text = pred
            .structured_format
            .as_ref()
            .and_then(|f| f.secondary_text.as_ref())
            .and_then(|t| t.text.as_ref())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        out.push(PlaceSuggestion {
            place_id,
            main_text,
            secondary_text,
        });
        if out.len() >= 8 {
            break;
        }
    }
    Ok(out)
}

/// Search queries for resolving a human profile location (most specific first).
#[must_use]
pub fn profile_location_search_queries(
    country_display_name: &str,
    region: Option<&str>,
    locality: Option<&str>,
    public_location_label: Option<&str>,
) -> Vec<String> {
    let country = country_display_name.trim();
    let region = region.map(str::trim).filter(|s| !s.is_empty());
    let locality = locality.map(str::trim).filter(|s| !s.is_empty());
    let public_label = public_location_label
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let mut out = Vec::new();
    let mut push = |q: String| {
        if q.chars().count() >= 5 && !out.iter().any(|existing| existing == &q) {
            out.push(q);
        }
    };

    if locality.is_some() || region.is_some() {
        let mut parts = Vec::new();
        if let Some(l) = locality {
            parts.push(l.to_string());
        }
        if let Some(r) = region {
            parts.push(r.to_string());
        }
        if !country.is_empty() {
            parts.push(country.to_string());
        }
        push(parts.join(", "));
    }

    if let Some(label) = public_label {
        if !country.is_empty() {
            push(format!("{label}, {country}"));
        }
        push(label.to_string());
    }

    if !country.is_empty() {
        push(country.to_string());
    }

    out
}

/// Autocomplete + Place Details from profile edit fields; prefers matches in `country_code`.
pub async fn resolve_place_from_profile_fields(
    api_key: &str,
    country_code: &str,
    country_display_name: &str,
    region: Option<&str>,
    locality: Option<&str>,
    public_location_label: Option<&str>,
    timeout: Duration,
) -> Result<CreateLocationRequest, PlacesError> {
    let cc = country_code.trim().to_ascii_uppercase();
    let queries = profile_location_search_queries(
        country_display_name,
        region,
        locality,
        public_location_label,
    );
    if queries.is_empty() {
        return Err(PlacesError::QueryTooShort);
    }

    for q in queries {
        let suggestions = match autocomplete_places(api_key, &q, Some(cc.as_str()), timeout).await
        {
            Ok(rows) => rows,
            Err(PlacesError::QueryTooShort) => continue,
            Err(e) => return Err(e),
        };
        let Some(first) = suggestions.first() else {
            continue;
        };
        let req = build_create_location_from_place_id(api_key, &first.place_id, timeout).await?;
        if req.country_code.eq_ignore_ascii_case(&cc) {
            return Ok(req);
        }
        tracing::warn!(
            query = %q,
            expected_country = %cc,
            got_country = %req.country_code,
            "google place country mismatch; trying next query"
        );
    }

    Err(PlacesError::NotFound)
}

#[cfg(test)]
mod profile_location_query_tests {
    use super::profile_location_search_queries;

    #[test]
    fn prefers_locality_region_country() {
        let q = profile_location_search_queries(
            "Australia",
            Some("NSW"),
            Some("Leura"),
            None,
        );
        assert_eq!(q.first().map(String::as_str), Some("Leura, NSW, Australia"));
    }

    #[test]
    fn includes_country_only_fallback() {
        let q = profile_location_search_queries("Australia", None, None, None);
        assert!(q.contains(&"Australia".to_string()));
    }

    #[test]
    fn skips_queries_shorter_than_five_chars() {
        let q = profile_location_search_queries("AU", Some("NSW"), None, None);
        assert!(!q.iter().any(|s| s.chars().count() < 5));
    }

    #[tokio::test]
    #[ignore = "requires GOOGLE_MAPS_API_KEY and network"]
    async fn resolve_coffee_camp_live() {
        dotenvy::dotenv().ok();
        let key = std::env::var("GOOGLE_MAPS_API_KEY").expect("GOOGLE_MAPS_API_KEY");
        let req = super::resolve_place_from_profile_fields(
            &key,
            "AU",
            "Australia",
            Some("NSW"),
            Some("Leura"),
            None,
            std::time::Duration::from_secs(20),
        )
        .await
        .expect("resolve");
        assert_eq!(req.validation_source, "google.places");
        assert!(req.validation_ref.as_ref().is_some_and(|s| !s.is_empty()));
        eprintln!("place_id={:?} region={:?} locality={:?}", req.validation_ref, req.region, req.locality);
    }
}
