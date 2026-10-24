//! Harm-class routing for post-scoped filing (Root points 4 / 13 / 15).

use crate::social::judicial::is_emergency_safety_point;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyHarmClass {
    /// CSAM / exploitation imagery (§4.1 — point 401).
    A,
    /// Non-consensual intimate imagery (§4.3 — point 403).
    B,
    /// Imminent violence or active doxxing (§4.2 / §4.4 — points 402, 404).
    C,
    /// Non-acute Safety Floor disputes (§4.5 / §4.6 — points 405, 406) → civic jury.
    NonAcute,
    /// Malware / phishing URLs (§4.7 — point 407) → link sanitation.
    LinkPolicy,
}

impl SafetyHarmClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::NonAcute => "D",
            Self::LinkPolicy => "link",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilingRoute {
    /// Class A/B/link — no `judicial_case`; audit via `safety_sanitation_event`.
    SanitationOnly(SafetyHarmClass),
    /// Class C — suppress-first emergency path (contest opens jury).
    EmergencySafety,
    /// Civic moderation (point 13), including non-acute §4.5–§4.6.
    CivicDispute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteError {
    Empty,
    MixedHarmClasses,
}

#[must_use]
pub fn harm_class_for_safety_point(point: u16) -> Option<SafetyHarmClass> {
    match point {
        401 => Some(SafetyHarmClass::A),
        403 => Some(SafetyHarmClass::B),
        402 | 404 => Some(SafetyHarmClass::C),
        405 | 406 => Some(SafetyHarmClass::NonAcute),
        407 => Some(SafetyHarmClass::LinkPolicy),
        _ => None,
    }
}

/// Points handled under Root point 15 — not selectable on the post filing form.
#[must_use]
pub fn is_point_excluded_from_filing_ui(point: u16) -> bool {
    matches!(
        harm_class_for_safety_point(point),
        Some(SafetyHarmClass::A | SafetyHarmClass::B | SafetyHarmClass::LinkPolicy)
    )
}

/// Resolve filing path from cited constitution **points** (after clause-id → point lookup).
#[must_use]
pub fn route_filing_for_clause_points(points: &[u16]) -> Result<FilingRoute, RouteError> {
    if points.is_empty() {
        return Err(RouteError::Empty);
    }

    let safety_classes: Vec<SafetyHarmClass> = points
        .iter()
        .filter_map(|&p| harm_class_for_safety_point(p))
        .collect();
    let has_non_safety_civic = points
        .iter()
        .any(|&p| !is_emergency_safety_point(p) && p != 4);

    if !safety_classes.is_empty() && has_non_safety_civic {
        return Err(RouteError::MixedHarmClasses);
    }

    if safety_classes.is_empty() {
        return Ok(FilingRoute::CivicDispute);
    }

    if safety_classes
        .iter()
        .any(|c| matches!(c, SafetyHarmClass::A | SafetyHarmClass::B | SafetyHarmClass::LinkPolicy))
    {
        let hc = if safety_classes.iter().any(|c| *c == SafetyHarmClass::A) {
            SafetyHarmClass::A
        } else if safety_classes.iter().any(|c| *c == SafetyHarmClass::B) {
            SafetyHarmClass::B
        } else {
            SafetyHarmClass::LinkPolicy
        };
        return Ok(FilingRoute::SanitationOnly(hc));
    }

    let has_c = safety_classes.iter().any(|c| *c == SafetyHarmClass::C);
    let has_non_acute = safety_classes
        .iter()
        .any(|c| *c == SafetyHarmClass::NonAcute);

    if has_c && has_non_acute {
        return Err(RouteError::MixedHarmClasses);
    }
    if has_c {
        return Ok(FilingRoute::EmergencySafety);
    }
    if has_non_acute {
        return Ok(FilingRoute::CivicDispute);
    }

    Err(RouteError::Empty)
}

/// Class C safety sub-clauses imply the serious civic `no_jury` clock when routed through civic or emergency paths.
#[must_use]
pub fn points_imply_serious_tier(points: &[u16]) -> bool {
    points
        .iter()
        .any(|&p| matches!(harm_class_for_safety_point(p), Some(SafetyHarmClass::C)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filing_ui_excludes_sanitation_points() {
        assert!(is_point_excluded_from_filing_ui(401));
        assert!(is_point_excluded_from_filing_ui(403));
        assert!(is_point_excluded_from_filing_ui(407));
        assert!(!is_point_excluded_from_filing_ui(402));
        assert!(!is_point_excluded_from_filing_ui(405));
        assert!(!is_point_excluded_from_filing_ui(100));
    }

    #[test]
    fn class_a_b_link_sanitation_only() {
        assert!(matches!(
            route_filing_for_clause_points(&[401]),
            Ok(FilingRoute::SanitationOnly(SafetyHarmClass::A))
        ));
        assert!(matches!(
            route_filing_for_clause_points(&[403]),
            Ok(FilingRoute::SanitationOnly(SafetyHarmClass::B))
        ));
        assert!(matches!(
            route_filing_for_clause_points(&[407]),
            Ok(FilingRoute::SanitationOnly(SafetyHarmClass::LinkPolicy))
        ));
    }

    #[test]
    fn class_c_emergency_safety() {
        assert!(matches!(
            route_filing_for_clause_points(&[402]),
            Ok(FilingRoute::EmergencySafety)
        ));
        assert!(matches!(
            route_filing_for_clause_points(&[404]),
            Ok(FilingRoute::EmergencySafety)
        ));
    }

    #[test]
    fn non_acute_civic() {
        assert!(matches!(
            route_filing_for_clause_points(&[405]),
            Ok(FilingRoute::CivicDispute)
        ));
        assert!(matches!(
            route_filing_for_clause_points(&[405, 406]),
            Ok(FilingRoute::CivicDispute)
        ));
    }

    #[test]
    fn mixed_c_and_non_acute_rejected() {
        assert!(matches!(
            route_filing_for_clause_points(&[402, 405]),
            Err(RouteError::MixedHarmClasses)
        ));
    }

    #[test]
    fn community_clause_civic() {
        assert!(matches!(
            route_filing_for_clause_points(&[100]),
            Ok(FilingRoute::CivicDispute)
        ));
    }

    #[test]
    fn serious_tier_from_class_c_points() {
        assert!(points_imply_serious_tier(&[402]));
        assert!(!points_imply_serious_tier(&[405]));
    }
}
