//! Reverse geocoding via OSM Nominatim (country code only). See usage policy:
//! <https://operations.osmfoundation.org/policies/nominatim/>

use std::sync::OnceLock;
use std::time::Duration;

use serde::Deserialize;

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn http_client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(12))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("reqwest client")
    })
}

#[derive(Debug, Deserialize)]
struct NominatimAddress {
    country_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NominatimReverse {
    address: Option<NominatimAddress>,
}

#[derive(Debug)]
pub enum GeocodeError {
    InvalidCoordinates,
    Upstream,
}

/// Returns ISO 3166-1 alpha-2 when Nominatim provides `address.country_code`.
pub async fn reverse_country_code(
    latitude: f64,
    longitude: f64,
    user_agent: &str,
) -> Result<Option<String>, GeocodeError> {
    if !latitude.is_finite() || !longitude.is_finite() {
        return Err(GeocodeError::InvalidCoordinates);
    }
    if latitude < -90.0 || latitude > 90.0 || longitude < -180.0 || longitude > 180.0 {
        return Err(GeocodeError::InvalidCoordinates);
    }

    let url = format!(
        "https://nominatim.openstreetmap.org/reverse?lat={latitude}&lon={longitude}&format=json&zoom=3"
    );

    let res = http_client()
        .get(url)
        .header(reqwest::header::USER_AGENT, user_agent)
        .send()
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "nominatim reverse request failed");
            GeocodeError::Upstream
        })?;

    if !res.status().is_success() {
        tracing::warn!(status = %res.status(), "nominatim reverse non-success");
        return Err(GeocodeError::Upstream);
    }

    let body: NominatimReverse = res.json().await.map_err(|e| {
        tracing::warn!(error = %e, "nominatim reverse json decode failed");
        GeocodeError::Upstream
    })?;

    Ok(body
        .address
        .and_then(|a| a.country_code)
        .map(|c| c.to_ascii_uppercase())
        .filter(|c| c.len() == 2 && c.chars().all(|ch| ch.is_ascii_alphabetic())))
}
