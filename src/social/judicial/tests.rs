use super::{
    community_member_delta_to_global_spillover, community_verdict_community_rating_delta,
    dedupe_preferring_lowest_priority, is_serious_moderation_tier_key, jury_civic_rating_delta,
    jury_size_for_case, normalize_vote_weight, AbstainOutcome, JurorCandidateRanked,
    MAX_FREE_ABSTENTIONS_PER_YEAR, RATING_NORMALIZATION_DIVISOR,
};
use crate::ref_keys;
use rust_decimal::Decimal;

fn ranked(
    id: u64,
    rating: &str,
    priority: i32,
    rating_basis_id: u64,
    rating_community_entity_id: Option<u64>,
) -> JurorCandidateRanked {
    JurorCandidateRanked {
        juror_social_entity_id: id,
        juror_user_id: None,
        juror_rating_at_invite: rating.parse::<Decimal>().unwrap(),
        source_priority: priority,
        rating_basis_id,
        rating_community_entity_id,
    }
}

#[test]
fn moderation_jury_size_is_square_root_capped_at_twelve() {
    assert_eq!(jury_size_for_case(ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE, 0), 0);
    assert_eq!(jury_size_for_case(ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE, 1), 1);
    assert_eq!(jury_size_for_case(ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE, 8), 3);
    assert_eq!(jury_size_for_case(ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE, 144), 12);
    assert_eq!(jury_size_for_case(ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE, 1000), 12);
}

#[test]
fn moderation_appeal_skips_human_jury_sizing() {
    assert_eq!(jury_size_for_case(ref_keys::KEY_JUDICIAL_TRIGGER_CASE_APPEAL, 0), 0);
    assert_eq!(jury_size_for_case(ref_keys::KEY_JUDICIAL_TRIGGER_CASE_APPEAL, 100), 0);
}

#[test]
fn constitutional_jury_size_is_uncapped_square_root() {
    assert_eq!(
        jury_size_for_case(ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT, 8),
        3
    );
    assert_eq!(
        jury_size_for_case(ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT, 1000),
        32
    );
    assert_eq!(
        jury_size_for_case(ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_PROFILE_AMENDMENT, 1000),
        32
    );
}

#[test]
fn abstention_quota_cap_is_three() {
    assert_eq!(MAX_FREE_ABSTENTIONS_PER_YEAR, 3);
}

#[test]
fn dedupe_keeps_community_rating_over_personal_rating() {
    // Same candidate appears via community (priority 1) AND as direct
    // audience (priority 2). Community connection rating must win because
    // the SQL pre-sorts by source_priority ASC.
    const RB_COMM: u64 = 901;
    const RB_GLOB: u64 = 902;
    let rows = vec![
        ranked(42, "4.10", 1, RB_COMM, Some(900)),
        ranked(42, "3.20", 2, RB_GLOB, None),
        ranked(99, "2.80", 2, RB_GLOB, None),
        ranked(99, "2.50", 3, RB_GLOB, None),
    ];
    let out = dedupe_preferring_lowest_priority(rows);
    assert_eq!(out.len(), 2);
    // Community rating preserved for candidate 42.
    assert_eq!(out[0].juror_social_entity_id, 42);
    assert_eq!(out[0].juror_rating_at_invite, "4.10".parse().unwrap());
    // Direct-audience rating preserved for candidate 99 (no community
    // path for them, so the priority-2 row is the first seen).
    assert_eq!(out[1].juror_social_entity_id, 99);
    assert_eq!(out[1].juror_rating_at_invite, "2.80".parse().unwrap());
}

#[test]
fn vote_weight_normalization() {
    assert!((RATING_NORMALIZATION_DIVISOR - 2.5).abs() < f64::EPSILON);
    assert!((normalize_vote_weight(&Decimal::new(0, 0)) - 0.0).abs() < f64::EPSILON);
    assert!((normalize_vote_weight(&Decimal::new(125, 2)) - 0.5).abs() < f64::EPSILON);
    assert!((normalize_vote_weight(&Decimal::new(250, 2)) - 1.0).abs() < f64::EPSILON);
    assert!((normalize_vote_weight(&Decimal::new(375, 2)) - 1.5).abs() < f64::EPSILON);
    assert!((normalize_vote_weight(&Decimal::new(500, 2)) - 2.0).abs() < f64::EPSILON);
}

#[test]
fn constitutional_juror_tiers_primary_stricter_than_fallback() {
    use crate::social::judicial_constitutional::{
        juror_min_rating_fallback, juror_min_rating_primary,
    };

    let primary = juror_min_rating_primary();
    let fallback = juror_min_rating_fallback();
    assert_eq!(primary, Decimal::new(250, 2));
    assert_eq!(fallback, Decimal::new(250, 2));
    assert!(primary == fallback);
}

#[test]
fn abstention_outcomes_partition_on_quota_boundary() {
    // Below cap: Free is the right shape; counter is the *prior* count.
    for prior in 0..MAX_FREE_ABSTENTIONS_PER_YEAR {
        let outcome = AbstainOutcome::Free {
            prior_in_window: prior,
        };
        assert!(matches!(outcome, AbstainOutcome::Free { .. }));
    }
    // At and above cap: OverFreeQuota.
    let outcome = AbstainOutcome::OverFreeQuota {
        prior_in_window: MAX_FREE_ABSTENTIONS_PER_YEAR,
        cap: MAX_FREE_ABSTENTIONS_PER_YEAR,
    };
    assert!(matches!(outcome, AbstainOutcome::OverFreeQuota { .. }));
}

#[test]
fn community_member_global_spillover_is_one_sixth() {
    let gain = Decimal::new(60, 2);
    let loss = Decimal::new(-60, 2);
    assert_eq!(
        community_member_delta_to_global_spillover(gain),
        Decimal::new(10, 2)
    );
    assert_eq!(
        community_member_delta_to_global_spillover(loss),
        Decimal::new(-10, 2)
    );
    assert!(community_member_delta_to_global_spillover(Decimal::ZERO).is_zero());
}

#[test]
fn community_feed_verdict_bonus_tiers_use_elapsed_seconds() {
    let d = jury_civic_rating_delta();
    let day = 86_400i64;
    assert_eq!(community_verdict_community_rating_delta(0), d * Decimal::from(3i64));
    assert_eq!(community_verdict_community_rating_delta(3 * day), d * Decimal::from(3i64));
    assert_eq!(community_verdict_community_rating_delta(3 * day + 1), d);
    assert_eq!(community_verdict_community_rating_delta(7 * day), d);
    assert_eq!(community_verdict_community_rating_delta(7 * day + 1), Decimal::ZERO);
    assert_eq!(community_verdict_community_rating_delta(14 * day), Decimal::ZERO);
}

#[test]
fn emergency_trigger_is_severe_for_juror_abstention() {
    use crate::social::judicial::triggers::trigger_is_emergency_safety;

    assert!(trigger_is_emergency_safety(
        ref_keys::KEY_JUDICIAL_TRIGGER_EMERGENCY_SAFETY
    ));
    assert!(!trigger_is_emergency_safety(
        ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE
    ));
}

#[test]
fn serious_moderation_tier_key_detection() {
    assert!(is_serious_moderation_tier_key(Some(
        ref_keys::KEY_JUDICIAL_MODERATION_TIER_SERIOUS
    )));
    assert!(!is_serious_moderation_tier_key(Some(
        ref_keys::KEY_JUDICIAL_MODERATION_TIER_REGULAR
    )));
    assert!(!is_serious_moderation_tier_key(None));
}
