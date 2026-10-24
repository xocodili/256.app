//! Entity recommendations for Search/Add and future discovery surfaces.
//!
//! Two tiers: direct peer graph (`direct_connections`) and community-expanded cohort
//! (`users_like_you`). See `docs/PRODUCT_GUIDE.md` § Search / Add — entity recommendations.

mod engine;
mod sources;
mod types;

pub use engine::recommend_for_user;
pub use types::{
    EntityRecommendation, EntityRecommendationsResponse, RecommendationSection,
    RecommendationTypeBucket, SECTION_DIRECT, SECTION_INFERRED,
};
