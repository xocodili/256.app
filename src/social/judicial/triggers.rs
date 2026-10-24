//! Judicial trigger classification (which paths use human jury vs AI council).

use crate::ref_keys;

/// Case was filed as an appeal from a resolved post dispute → AI council review only.
#[must_use]
pub fn trigger_uses_ai_council_review(trigger_key: &str) -> bool {
    trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_CASE_APPEAL
}

/// Civic charter dispute or legacy post-moderation slug (pre-greenfield rows).
#[must_use]
pub fn trigger_is_civic_dispute(trigger_key: &str) -> bool {
    trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE
        || trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_POST_DISPUTE
}

/// Emergency safety filing (Root §4.1–§4.7) — suppressed until contested.
#[must_use]
pub fn trigger_is_emergency_safety(trigger_key: &str) -> bool {
    trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_EMERGENCY_SAFETY
}

/// Post-scoped dispute triggers including council appeal.
#[must_use]
pub fn trigger_is_post_dispute_like(trigger_key: &str) -> bool {
    trigger_is_civic_dispute(trigger_key)
        || trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_CASE_APPEAL
}

/// Constitutional amendments are decided by a human community jury only.
#[must_use]
pub fn trigger_is_constitutional_amendment(trigger_key: &str) -> bool {
    trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT
}

/// Community profile amendments use the same human-jury-only supermajority pattern.
#[must_use]
pub fn trigger_is_community_profile_amendment(trigger_key: &str) -> bool {
    trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_PROFILE_AMENDMENT
}

/// Uncapped square-root jury drawn from the community member pool (≥3.00 primary tier).
#[must_use]
pub fn trigger_requires_full_community_jury_pool(trigger_key: &str) -> bool {
    matches!(
        trigger_key,
        ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT
            | ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_PROFILE_AMENDMENT
    )
}

/// Human jurors may be seated for this trigger (never the AI council as jurors).
#[must_use]
pub fn trigger_uses_human_jury(trigger_key: &str) -> bool {
    matches!(
        trigger_key,
        ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE
            | ref_keys::KEY_JUDICIAL_TRIGGER_POST_DISPUTE
            | ref_keys::KEY_JUDICIAL_TRIGGER_EMERGENCY_SAFETY
            | ref_keys::KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW
            | ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT
            | ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_PROFILE_AMENDMENT
            | ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_REPARENT
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_post_dispute_appeal_uses_council() {
        assert!(trigger_uses_ai_council_review(
            ref_keys::KEY_JUDICIAL_TRIGGER_CASE_APPEAL
        ));
        assert!(!trigger_uses_ai_council_review(
            ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT
        ));
        assert!(!trigger_uses_ai_council_review(
            ref_keys::KEY_JUDICIAL_TRIGGER_POST_DISPUTE
        ));
    }

    #[test]
    fn constitutional_amendment_is_human_jury_only() {
        assert!(trigger_is_constitutional_amendment(
            ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT
        ));
        assert!(trigger_uses_human_jury(
            ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT
        ));
        assert!(!trigger_uses_ai_council_review(
            ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT
        ));
    }
}
