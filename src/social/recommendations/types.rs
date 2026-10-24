//! Serializable recommendation rows for Search/Add and future discovery surfaces.

use rust_decimal::Decimal;
use serde::Serialize;

/// Ordered entity types shown in each recommendation section (2×2 grid).
pub const RECOMMENDATION_TYPE_KEYS: &[&str] = &["community", "gathering", "profit", "non_profit"];

pub const PER_TYPE_LIMIT: usize = 6;

pub const SECTION_DIRECT: &str = "direct_connections";
pub const SECTION_INFERRED: &str = "users_like_you";

#[derive(Debug, Clone, Serialize)]
pub struct EntityRecommendation {
    pub entity_id: u64,
    pub handle: String,
    pub label: String,
    pub type_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_path: Option<String>,
    #[serde(serialize_with = "serialize_rating")]
    pub entity_rating: Decimal,
    /// Stable source id (`direct_connections`, `users_like_you`) for analytics.
    pub source: String,
}

fn serialize_rating<S>(value: &Decimal, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use rust_decimal::prelude::ToPrimitive;
    let f: f64 = value.to_f64().unwrap_or(0.0);
    serializer.serialize_f64(f)
}

#[derive(Debug, Clone, Serialize)]
pub struct RecommendationTypeBucket {
    pub type_key: String,
    pub items: Vec<EntityRecommendation>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecommendationSection {
    pub key: String,
    pub types: Vec<RecommendationTypeBucket>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EntityRecommendationsResponse {
    pub sections: Vec<RecommendationSection>,
}

impl EntityRecommendationsResponse {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            sections: Vec::new(),
        }
    }
}
