//! Weighted verdict evaluation shared by human juries and council appeals.

use rust_decimal::Decimal;

use crate::ref_keys;

use super::{normalize_vote_weight, VerdictOutcome};

/// Outcome of weighted jury tally, including whether the hung band applied a safety default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeightedVerdictResult {
    pub outcome: VerdictOutcome,
    /// True when neither side reached ⅔ — a trigger-specific safety default was applied.
    pub hung_band: bool,
}

/// Sum normalized vote weights (`rating / 2.5`) for accepted jurors by direction.
#[must_use]
pub fn normalized_vote_totals(votes: &[(String, Decimal)]) -> (f64, f64) {
    let mut normalized_for = 0.0;
    let mut normalized_against = 0.0;
    for (direction, rating) in votes {
        let weight = normalize_vote_weight(rating);
        if direction == ref_keys::KEY_JCJI_VOTE_FOR {
            normalized_for += weight;
        } else {
            normalized_against += weight;
        }
    }
    (normalized_for, normalized_against)
}

/// When neither weighted side reaches two-thirds, resolve to the **safety default** for
/// the trigger (original filing assertion where that means “do not relax standards”).
#[must_use]
pub fn safety_verdict_on_hung(trigger_key: &str) -> VerdictOutcome {
    match trigger_key {
        ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT
        | ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_PROFILE_AMENDMENT
        | ref_keys::KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW
        | ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_REPARENT => {
            // Do not admit or amend without a clear supermajority.
            VerdictOutcome::Overturned
        }
        ref_keys::KEY_JUDICIAL_TRIGGER_EMERGENCY_SAFETY => {
            // Safety default: emergency filing stands when the jury is hung.
            VerdictOutcome::Upheld
        }
        _ => {
            // Civic dispute / appeal: original assertion is that the post violates standards.
            VerdictOutcome::Upheld
        }
    }
}

/// Weighted 2/3 verdict from normalized vote weights (`rating / 2.5` per juror).
#[must_use]
pub fn evaluate_weighted_verdict(
    trigger_key: &str,
    normalized_for: f64,
    normalized_against: f64,
) -> WeightedVerdictResult {
    let normalized_total = normalized_for + normalized_against;
    if normalized_total <= f64::EPSILON {
        return WeightedVerdictResult {
            outcome: safety_verdict_on_hung(trigger_key),
            hung_band: true,
        };
    }
    let threshold = normalized_total * 2.0 / 3.0;
    if normalized_for >= threshold {
        WeightedVerdictResult {
            outcome: VerdictOutcome::Upheld,
            hung_band: false,
        }
    } else if normalized_against >= threshold {
        WeightedVerdictResult {
            outcome: VerdictOutcome::Overturned,
            hung_band: false,
        }
    } else {
        WeightedVerdictResult {
            outcome: safety_verdict_on_hung(trigger_key),
            hung_band: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ref_keys;

    #[test]
    fn post_dispute_hung_defaults_to_upheld_safety() {
        assert_eq!(
            safety_verdict_on_hung(ref_keys::KEY_JUDICIAL_TRIGGER_POST_DISPUTE),
            VerdictOutcome::Upheld
        );
        let r = evaluate_weighted_verdict(ref_keys::KEY_JUDICIAL_TRIGGER_POST_DISPUTE, 1.0, 1.0);
        assert_eq!(r.outcome, VerdictOutcome::Upheld);
        assert!(r.hung_band);
    }

    #[test]
    fn constitutional_hung_defaults_to_overturned_safety() {
        assert_eq!(
            safety_verdict_on_hung(ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT),
            VerdictOutcome::Overturned
        );
        let r = evaluate_weighted_verdict(
            ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT,
            1.0,
            1.0,
        );
        assert_eq!(r.outcome, VerdictOutcome::Overturned);
        assert!(r.hung_band);
    }

    #[test]
    fn membership_review_hung_defaults_to_overturned_safety() {
        assert_eq!(
            safety_verdict_on_hung(ref_keys::KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW),
            VerdictOutcome::Overturned
        );
        let r =
            evaluate_weighted_verdict(ref_keys::KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW, 1.0, 1.0);
        assert_eq!(r.outcome, VerdictOutcome::Overturned);
        assert!(r.hung_band);
    }

    #[test]
    fn normalized_vote_totals_sums_directions() {
        use rust_decimal::Decimal;

        let votes = vec![
            (ref_keys::KEY_JCJI_VOTE_FOR.to_string(), Decimal::new(250, 2)),
            (ref_keys::KEY_JCJI_VOTE_AGAINST.to_string(), Decimal::new(500, 2)),
        ];
        let (f, a) = super::normalized_vote_totals(&votes);
        assert!((f - 1.0).abs() < f64::EPSILON);
        assert!((a - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn clear_supermajority_for_wins() {
        let upheld =
            evaluate_weighted_verdict(ref_keys::KEY_JUDICIAL_TRIGGER_POST_DISPUTE, 2.0, 0.5);
        assert_eq!(upheld.outcome, VerdictOutcome::Upheld);
        assert!(!upheld.hung_band);
        let overturned =
            evaluate_weighted_verdict(ref_keys::KEY_JUDICIAL_TRIGGER_POST_DISPUTE, 0.5, 2.0);
        assert_eq!(overturned.outcome, VerdictOutcome::Overturned);
        assert!(!overturned.hung_band);
    }
}
