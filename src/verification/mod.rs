//! User verification **tiers** and **location** requirements for gated features (internal policy).
//! `verification_tier` is **not** exposed via `GET /ref-key-values`; keep tiers and ratings out of user-facing APIs.
//!
//! - **Basic**: coarse `locations` row from onboarding (country-first; coordinates optional).
//! - **Enhanced**: dating / marketplace-style risk — persisted coordinates + Google Places identity
//!   (`validation_source` / `validation_ref`) for server-side proximity without exposing raw points
//!   to peers (implemented in future modules).
//! - **Professional**: profit / non-profit entities will add KYB beyond location; location rules
//!   currently match Enhanced until org onboarding ships.
//!
//! Google Maps Platform (Places API, Geocoding) is the default validator; see [`crate::google_maps`].

use crate::locations::{CreateLocationRequest, ResponseCoordinatePrecision};
use crate::ref_keys;

/// Product tier carried on `users.verification_tier_id` (`ref_keys::GROUP_VERIFICATION_TIER`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationTier {
    Basic,
    Enhanced,
    Professional,
}

impl VerificationTier {
    #[must_use]
    pub const fn ref_key(self) -> &'static str {
        match self {
            Self::Basic => ref_keys::KEY_VERIFICATION_TIER_BASIC,
            Self::Enhanced => ref_keys::KEY_VERIFICATION_TIER_ENHANCED,
            Self::Professional => ref_keys::KEY_VERIFICATION_TIER_PROFESSIONAL,
        }
    }

    /// Parse from `ref_key_values.key` in group `verification_tier`.
    pub fn from_ref_key(key: &str) -> Option<Self> {
        match key {
            ref_keys::KEY_VERIFICATION_TIER_BASIC => Some(Self::Basic),
            ref_keys::KEY_VERIFICATION_TIER_ENHANCED => Some(Self::Enhanced),
            ref_keys::KEY_VERIFICATION_TIER_PROFESSIONAL => Some(Self::Professional),
            _ => None,
        }
    }
}

/// Returns `true` when `validation_source` is a Google Maps / Places pipeline (stored lowercase).
#[must_use]
pub fn is_google_maps_validation_source(source: &str) -> bool {
    matches!(
        source,
        "google.places" | "google.maps.places" | "google.maps.geocode" | "google.geocode"
    )
}

/// Whether a [`CreateLocationRequest`] satisfies the **location** bar for `tier`.
pub fn location_request_satisfies_tier(req: &CreateLocationRequest, tier: VerificationTier) -> bool {
    match tier {
        VerificationTier::Basic => basic_location_ok(req),
        VerificationTier::Enhanced => enhanced_location_ok(req),
        VerificationTier::Professional => professional_location_ok(req),
    }
}

fn basic_location_ok(req: &CreateLocationRequest) -> bool {
    let cc = req.country_code.trim();
    cc.len() == 2 && cc.chars().all(|c| c.is_ascii_alphabetic())
}

fn enhanced_location_ok(req: &CreateLocationRequest) -> bool {
    if !basic_location_ok(req) {
        return false;
    }
    if !req.store_coordinates {
        return false;
    }
    if req.latitude.is_none() || req.longitude.is_none() {
        return false;
    }
    let src = req.validation_source.trim().to_ascii_lowercase();
    if !is_google_maps_validation_source(&src) {
        return false;
    }
    let Some(ref rf) = req.validation_ref else {
        return false;
    };
    !rf.trim().is_empty()
}

fn professional_location_ok(req: &CreateLocationRequest) -> bool {
    // Until KYB ships, professional location evidence matches Enhanced.
    enhanced_location_ok(req)
}

/// Human-readable expectation for clients / logs (not end-user i18n).
#[must_use]
pub fn tier_location_requirements_summary(tier: VerificationTier) -> &'static str {
    match tier {
        VerificationTier::Basic => {
            "ISO country; optional region/locality/label; coordinates may be omitted (privacy default)."
        }
        VerificationTier::Enhanced => {
            "Google-validated place: store_coordinates, lat/lon, validation_ref (e.g. Place ID), validation_source google.places|google.maps.geocode."
        }
        VerificationTier::Professional => {
            "Same location evidence as Enhanced today; additional business verification will layer on later."
        }
    }
}

/// Default API response precision for a tier (server still omits unless policy widens).
#[must_use]
pub fn default_coordinate_precision_for_tier(tier: VerificationTier) -> ResponseCoordinatePrecision {
    match tier {
        VerificationTier::Basic => ResponseCoordinatePrecision::Omit,
        VerificationTier::Enhanced | VerificationTier::Professional => {
            ResponseCoordinatePrecision::Neighborhood
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ref_keys;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn basic_allows_no_coordinates() {
        let req = CreateLocationRequest {
            country_code: "AU".into(),
            region: None,
            locality: None,
            postal_code: None,
            latitude: None,
            longitude: None,
            display_label: "Australia".into(),
            validation_source: "user_profile".into(),
            validation_ref: None,
            store_coordinates: false,
            response_coordinate_precision: ResponseCoordinatePrecision::Omit,
        };
        assert!(location_request_satisfies_tier(&req, VerificationTier::Basic));
        assert!(!location_request_satisfies_tier(&req, VerificationTier::Enhanced));
    }

    #[test]
    fn enhanced_requires_google_and_coords() {
        let ok = CreateLocationRequest {
            country_code: "AU".into(),
            region: Some("NSW".into()),
            locality: Some("Sydney".into()),
            postal_code: None,
            latitude: Some(Decimal::from_str("-33.8688").unwrap()),
            longitude: Some(Decimal::from_str("151.2093").unwrap()),
            display_label: "Sydney".into(),
            validation_source: "google.places".into(),
            validation_ref: Some("ChIJN1t_tDeuEmsRUsoyG83frY4".into()),
            store_coordinates: true,
            response_coordinate_precision: ResponseCoordinatePrecision::Neighborhood,
        };
        assert!(location_request_satisfies_tier(&ok, VerificationTier::Enhanced));

        let mut bad = ok.clone();
        bad.store_coordinates = false;
        assert!(!location_request_satisfies_tier(&bad, VerificationTier::Enhanced));

        let mut bad = ok.clone();
        bad.validation_ref = None;
        assert!(!location_request_satisfies_tier(&bad, VerificationTier::Enhanced));

        let mut bad = ok.clone();
        bad.validation_source = "nominatim".into();
        assert!(!location_request_satisfies_tier(&bad, VerificationTier::Enhanced));

        assert!(location_request_satisfies_tier(&ok, VerificationTier::Professional));
    }

    #[test]
    fn tier_helpers_and_ref_keys_round_trip() {
        for tier in [
            VerificationTier::Basic,
            VerificationTier::Enhanced,
            VerificationTier::Professional,
        ] {
            assert!(!tier_location_requirements_summary(tier).is_empty());
            let _ = default_coordinate_precision_for_tier(tier);
        }
        assert_eq!(VerificationTier::Basic.ref_key(), ref_keys::KEY_VERIFICATION_TIER_BASIC);
        assert_eq!(
            VerificationTier::from_ref_key(ref_keys::KEY_VERIFICATION_TIER_ENHANCED),
            Some(VerificationTier::Enhanced)
        );
    }
}
