//! Product pricing schedule for **Nom**-debiting services.
//!
//! Single source of truth for what each user-facing AI service or governance
//! action costs in Noms (`social_human.ai_tokens`). The schedule is held in
//! code on purpose — these are product decisions, not data, and they should
//! travel through code review the same way feature behaviour does. The DB
//! has no parallel `service_price` table; do not add one without bringing this
//! file along with it.
//!
//! When wiring a new Nom-debiting flow:
//! 1. Add a variant to [`BillableService`].
//! 2. Set its cost in [`BillableService::nom_cost`].
//! 3. Have the handler call [`crate::social::noms::spend_for_service_tx`]
//!    with the acting `users.id` (or the non-transactional
//!    [`crate::social::noms::spend_for_service`]).
//! 4. Mirror the new line in `docs/PRODUCT_GUIDE.md` (Nom price table) so
//!    product copy and code stay in lock-step.

/// Judicial case filing triggers. Mirrors
/// `ref_keys::GROUP_JUDICIAL_CASE_TRIGGER` but kept in code because the
/// **price** attached to each trigger is a product policy, not a DB-driven
/// classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JudicialTrigger {
    /// Civic charter dispute on a post (non-emergency clauses).
    CivicDispute,
    /// Emergency safety filing citing Root §4.1–§4.7.
    EmergencySafety,
    /// Legacy alias kept for ledger keys on pre-greenfield rows.
    PostDispute,
    /// A community is reviewing an applicant or sitting member for admission.
    MembershipReview,
    /// A proposed change to a community's constitution.
    ConstitutionalAmendment,
    /// A proposed change to a community's About, intention, or join policy.
    CommunityProfileAmendment,
    /// Judicial petition to move a community under a new parent in the graph.
    CommunityReparent,
}

/// Every product action that may debit Noms from a human's wallet.
///
/// All variants live here even if the originating UI is not yet wired up; this
/// gives us a complete, reviewable price list at a glance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillableService {
    /// Opening a new judicial case of the given trigger type.
    JudicialFiling(JudicialTrigger),
    /// Escalating a **resolved post dispute** to AI council review (Council of 12).
    /// Constitutional amendments and membership reviews are human-jury-only — no council appeal.
    PostDisputeCouncilAppeal,
    /// Author contests an uncontested emergency safety case (opens jury).
    EmergencyContest,
    /// Council-assisted portraiture (full image generation for the user's
    /// profile / public surfaces). Heavier compute than an avatar refresh.
    CouncilPortraiture,
    /// Council-assisted regeneration of an existing avatar in-place.
    CouncilAvatarRefresh,
    /// Council-assisted prompt rewrite / artwork brief — text-only helper
    /// that prepares a generation request without generating an image itself.
    CouncilArtworkBrief,
    /// Founding a new **community** (`social_entity_type.community`).
    EntityCreateCommunity,
    /// Founding a new **profit** org (`social_entity_type.profit`).
    EntityCreateProfit,
    /// Founding a new **non-profit** org (`social_entity_type.non_profit`).
    EntityCreateNonProfit,
    /// Founding a new **gathering** / event entity (`social_entity_type.gathering`).
    EntityCreateGathering,
    /// Publishing a root post authored as a **profit** org.
    BrandedPostProfit,
    /// Publishing a root post authored as a **non-profit** org.
    BrandedPostNonProfit,
    /// Publishing a root post authored as a **gathering** / event entity.
    BrandedPostGathering,
}

impl BillableService {
    /// Cost in Noms. `0` means the action is intentionally free and the
    /// caller should skip the wallet round-trip; see [`Self::requires_payment`].
    #[must_use]
    pub const fn nom_cost(self) -> u16 {
        match self {
            // Judicial pricing is policy-fixed and documented in
            // docs/PRODUCT_GUIDE.md. Membership review is free under all
            // circumstances — joining a community must never be gated by
            // token balance.
            BillableService::JudicialFiling(trigger) => match trigger {
                JudicialTrigger::CivicDispute | JudicialTrigger::PostDispute => 1,
                JudicialTrigger::EmergencySafety => 1,
                JudicialTrigger::MembershipReview => 0,
                JudicialTrigger::ConstitutionalAmendment => 32,
                JudicialTrigger::CommunityProfileAmendment => 32,
                JudicialTrigger::CommunityReparent => 64,
            },
            BillableService::PostDisputeCouncilAppeal => 32,
            BillableService::EmergencyContest => 1,

            // AI council service pricing — calibrated against signup grant + pack ladder.
            BillableService::CouncilPortraiture => 4,
            BillableService::CouncilAvatarRefresh => 16,
            BillableService::CouncilArtworkBrief => 1,

            BillableService::EntityCreateCommunity => 32,
            BillableService::EntityCreateProfit => 32,
            BillableService::EntityCreateNonProfit => 16,
            BillableService::EntityCreateGathering => 16,

            // Branded post authorship — human posts remain free.
            BillableService::BrandedPostProfit => 4,
            BillableService::BrandedPostNonProfit => 2,
            BillableService::BrandedPostGathering => 2,
        }
    }

    /// Map a `social_entity_type` ref key to the founding cost, if any.
    /// Human and digital types are intentionally excluded (no charge).
    #[must_use]
    pub fn for_entity_type_key(type_key: &str) -> Option<Self> {
        match type_key {
            crate::ref_keys::KEY_ENTITY_TYPE_COMMUNITY => Some(Self::EntityCreateCommunity),
            crate::ref_keys::KEY_ENTITY_TYPE_PROFIT => Some(Self::EntityCreateProfit),
            crate::ref_keys::KEY_ENTITY_TYPE_NON_PROFIT => Some(Self::EntityCreateNonProfit),
            crate::ref_keys::KEY_ENTITY_TYPE_GATHERING => Some(Self::EntityCreateGathering),
            _ => None,
        }
    }

    /// Map a `social_entity_type` ref key to the branded-post publish cost, if any.
    /// Human, community, and digital types are intentionally excluded (no charge).
    #[must_use]
    pub fn for_branded_post_type_key(type_key: &str) -> Option<Self> {
        match type_key {
            crate::ref_keys::KEY_ENTITY_TYPE_PROFIT => Some(Self::BrandedPostProfit),
            crate::ref_keys::KEY_ENTITY_TYPE_NON_PROFIT => Some(Self::BrandedPostNonProfit),
            crate::ref_keys::KEY_ENTITY_TYPE_GATHERING => Some(Self::BrandedPostGathering),
            _ => None,
        }
    }

    /// `true` when this service has a non-zero Nom cost. Use this to gate UI
    /// affordances (e.g. dimming a button when the user can't afford it) and
    /// to short-circuit the wallet update path on free services.
    #[must_use]
    pub const fn requires_payment(self) -> bool {
        self.nom_cost() > 0
    }

    /// Stable analytics key stored on `social_human_ai_token_ledger.service_key`.
    #[must_use]
    pub const fn ledger_key(self) -> &'static str {
        match self {
            BillableService::JudicialFiling(JudicialTrigger::CivicDispute) => {
                "judicial_filing.civic_dispute"
            }
            BillableService::JudicialFiling(JudicialTrigger::EmergencySafety) => {
                "judicial_filing.emergency_safety"
            }
            BillableService::JudicialFiling(JudicialTrigger::PostDispute) => {
                "judicial_filing.post_dispute"
            }
            BillableService::JudicialFiling(JudicialTrigger::MembershipReview) => {
                "judicial_filing.membership_review"
            }
            BillableService::JudicialFiling(JudicialTrigger::ConstitutionalAmendment) => {
                "judicial_filing.constitutional_amendment"
            }
            BillableService::JudicialFiling(JudicialTrigger::CommunityProfileAmendment) => {
                "judicial_filing.community_profile_amendment"
            }
            BillableService::JudicialFiling(JudicialTrigger::CommunityReparent) => {
                "judicial_filing.community_reparent"
            }
            BillableService::PostDisputeCouncilAppeal => "judicial_appeal.post_dispute",
            BillableService::EmergencyContest => "judicial_contest.emergency_safety",
            BillableService::CouncilPortraiture => "council_portraiture",
            BillableService::CouncilAvatarRefresh => "council_avatar_refresh",
            BillableService::CouncilArtworkBrief => "council_artwork_brief",
            BillableService::EntityCreateCommunity => "entity_create.community",
            BillableService::EntityCreateProfit => "entity_create.profit",
            BillableService::EntityCreateNonProfit => "entity_create.non_profit",
            BillableService::EntityCreateGathering => "entity_create.gathering",
            BillableService::BrandedPostProfit => "branded_post.profit",
            BillableService::BrandedPostNonProfit => "branded_post.non_profit",
            BillableService::BrandedPostGathering => "branded_post.gathering",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn judicial_filing_prices_match_marketing_canon() {
        assert_eq!(
            BillableService::JudicialFiling(JudicialTrigger::PostDispute).nom_cost(),
            1
        );
        assert_eq!(
            BillableService::JudicialFiling(JudicialTrigger::MembershipReview).nom_cost(),
            0
        );
        assert_eq!(
            BillableService::JudicialFiling(JudicialTrigger::ConstitutionalAmendment).nom_cost(),
            32
        );
    }

    #[test]
    fn judicial_appeal_post_dispute_costs_more_than_filing() {
        assert_eq!(
            BillableService::JudicialFiling(JudicialTrigger::PostDispute).nom_cost(),
            1
        );
        assert_eq!(BillableService::PostDisputeCouncilAppeal.nom_cost(), 32);
    }

    #[test]
    fn membership_review_is_free_and_requires_no_payment() {
        let item = BillableService::JudicialFiling(JudicialTrigger::MembershipReview);
        assert_eq!(item.nom_cost(), 0);
        assert!(!item.requires_payment());
    }

    #[test]
    fn constitutional_amendment_fits_inside_the_nom_cap() {
        // The wallet cap is 256 Noms (see migrations/20260416093000). A single
        // priced action must not exceed that cap, or it would be unreachable.
        assert!(
            BillableService::JudicialFiling(JudicialTrigger::ConstitutionalAmendment).nom_cost()
                <= 256
        );
    }

    #[test]
    fn ai_services_have_positive_starting_prices() {
        for item in [
            BillableService::CouncilPortraiture,
            BillableService::CouncilAvatarRefresh,
            BillableService::CouncilArtworkBrief,
        ] {
            assert!(item.requires_payment(), "{item:?} must charge something");
            assert!(item.nom_cost() <= 256, "{item:?} cost exceeds wallet cap");
        }
    }

    #[test]
    fn entity_create_prices_match_product_schedule() {
        assert_eq!(BillableService::EntityCreateCommunity.nom_cost(), 32);
        assert_eq!(BillableService::EntityCreateProfit.nom_cost(), 32);
        assert_eq!(BillableService::EntityCreateNonProfit.nom_cost(), 16);
        assert_eq!(BillableService::EntityCreateGathering.nom_cost(), 16);
    }

    #[test]
    fn branded_post_prices_match_product_schedule() {
        assert_eq!(BillableService::BrandedPostProfit.nom_cost(), 4);
        assert_eq!(BillableService::BrandedPostNonProfit.nom_cost(), 2);
        assert_eq!(BillableService::BrandedPostGathering.nom_cost(), 2);
    }

    #[test]
    fn for_branded_post_type_key_maps_brands_only() {
        use crate::ref_keys;
        assert_eq!(
            BillableService::for_branded_post_type_key(ref_keys::KEY_ENTITY_TYPE_PROFIT),
            Some(BillableService::BrandedPostProfit)
        );
        assert_eq!(
            BillableService::for_branded_post_type_key(ref_keys::KEY_ENTITY_TYPE_HUMAN),
            None
        );
    }

    #[test]
    fn for_entity_type_key_maps_ref_keys_and_skips_human() {
        use crate::ref_keys;
        assert_eq!(
            BillableService::for_entity_type_key(ref_keys::KEY_ENTITY_TYPE_COMMUNITY),
            Some(BillableService::EntityCreateCommunity)
        );
        assert_eq!(
            BillableService::for_entity_type_key(ref_keys::KEY_ENTITY_TYPE_HUMAN),
            None
        );
    }

    #[test]
    fn entity_create_costs_fit_inside_nom_cap() {
        for item in [
            BillableService::EntityCreateCommunity,
            BillableService::EntityCreateProfit,
            BillableService::EntityCreateNonProfit,
            BillableService::EntityCreateGathering,
        ] {
            assert!(item.nom_cost() <= 256, "{item:?} cost exceeds wallet cap");
        }
    }
}
