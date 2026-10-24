//! Judicial cases: jury nomination, weighted verdicts, settlement, and deadlines.
//!
//! Submodules mirror the object model:
//! - [`verdict_eval`] — weighted ⅔ and safety defaults on hung juries
//! - [`types`] — shared enums and row shapes

mod acute_harm_content;
mod case_access;
mod deadlines;
mod emergency;
mod invites;
mod moderation_tier;
mod nomination;
mod pool;
pub mod post_verdict_copy;
mod rating;
mod sizing;
mod verdict;

#[cfg(test)]
mod tests;

pub mod community_reparent;
pub mod membership_review;
pub mod triggers;
pub mod types;
pub mod verdict_eval;

pub use acute_harm_content::{apply_or_refund_acute_harm_content_rating_tx, AcuteHarmDirection};

pub use triggers::{
    trigger_is_civic_dispute, trigger_is_community_profile_amendment,
    trigger_is_constitutional_amendment, trigger_is_emergency_safety,
    trigger_is_post_dispute_like, trigger_requires_full_community_jury_pool,
    trigger_uses_ai_council_review, trigger_uses_human_jury,
};

pub use types::{
    civic_rating_delta_times, failing_rating_threshold, jury_civic_rating_delta, AbstainOutcome,
    AuthorWithdrawPostUnderReviewOutcome, CastVoteOutcome,
    CreatedJurorInvite, DeclineOutcome, JuryNominationSummary, MembershipReviewApplicantOutcome,
    PendingJurorInvite, VerdictOutcome, VerdictResolutionRetrySummary,
    RATING_MULT_CONVERSATION_LOATHED_EDGE, RATING_MULT_CONVERSATION_LOATHED_GLOBAL,
    RATING_MULT_CONVERSATION_LOVED_EDGE, RATING_MULT_CONVERSATION_LOVED_GLOBAL,
    RATING_MULT_JUROR_PARTICIPATION, RATING_MULT_ORIGINATOR_CLEAR, RATING_MULT_ORIGINATOR_HUNG_PROPOSAL,
    MAX_FREE_ABSTENTIONS_PER_YEAR, ABSTENTION_WINDOW_DAYS,
};
pub use verdict_eval::{evaluate_weighted_verdict, safety_verdict_on_hung, WeightedVerdictResult};

pub use deadlines::{
    close_jury_cases_past_no_jury_deadline, close_post_moderation_cases_past_no_jury_deadline,
};
pub(crate) use case_access::{
    community_context_for_case_tx, fetch_open_case_row_tx, is_community_social_entity_tx,
};
pub(crate) use rating::bump_community_member_rating_tx;
pub use invites::{
    abstain_from_invite, cast_vote, count_recent_abstentions_for_user, decline_from_invite,
    find_pending_human_jury_invite_for_user, find_pending_post_dispute_invite_for_user,
};
pub use nomination::{nominate_jurors_for_open_cases, retry_ready_verdict_resolutions};
pub use rating::{
    apply_community_no_jury_penalty_tx, apply_jury_civic_rating_settlement_tx,
    bump_global_entity_rating_tx, community_member_delta_to_global_spillover,
    normalize_vote_weight, COMMUNITY_MEMBER_TO_GLOBAL_RATING_SPILLOVER_DIVISOR,
    RATING_NORMALIZATION_DIVISOR,
};
pub use moderation_tier::{
    case_is_severe_for_juror_abstention, is_serious_moderation_tier_key,
    REGULAR_NO_JURY_DEADLINE_DAYS, SERIOUS_NO_JURY_DEADLINE_DAYS,
};
pub use sizing::{
    compute_jury_size_to_freeze_at_filing, compute_jury_size_to_freeze_at_filing_tx,
    jury_size_for_case,
};
pub use emergency::{
    active_emergency_case_on_post_tx, check_daily_emergency_filing_cap_tx,
    classify_clause_ids_tx, contest_emergency_case, is_emergency_safety_point,
    run_standing_settlement_batch, suppress_post_thread_tx, ContestOutcome,
    EmergencyFilingError, EMERGENCY_STANDING_SETTLEMENT_DAYS,
    MAX_EMERGENCY_FILINGS_PER_USER_PER_DAY,
};
pub use verdict::{
    author_withdraw_post_under_review, check_and_resolve_verdict, resolve_council_appeal_verdict,
    set_subject_post_under_judicial_review_tx,
};

pub(crate) use emergency::{platform_safety_ban_human_tx, restore_post_thread_tx};

pub(crate) use pool::dedupe_preferring_lowest_priority;
pub(crate) use rating::community_verdict_community_rating_delta;
pub(crate) use types::JurorCandidateRanked;

use sqlx::MySqlPool;

/// After an invite response commits, attempt verdict resolution on a fresh transaction.
pub(crate) async fn try_resolve_verdict(
    pool: &MySqlPool,
    case_id: u64,
) -> Result<(), sqlx::Error> {
    let _ = verdict::check_and_resolve_verdict(pool, case_id).await?;
    Ok(())
}
