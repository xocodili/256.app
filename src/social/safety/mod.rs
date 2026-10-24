//! Acute harm sanitation (Root point 15) — pre-persist media gate and audit log.

pub mod harm_class;
pub mod media_hash_gate;
pub mod sanitation;

pub use harm_class::{
    harm_class_for_safety_point, is_point_excluded_from_filing_ui, points_imply_serious_tier,
    route_filing_for_clause_points, FilingRoute, RouteError, SafetyHarmClass,
};
pub use media_hash_gate::{scan_jpeg_bytes, HarmClass, MediaScanOutcome};
pub use sanitation::{log_filing_sanitation_routed, log_sanitation_event};
