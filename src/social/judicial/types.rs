//! Shared judicial domain types.

use rust_decimal::Decimal;

/// Each user may abstain from up to this many jury invitations in any rolling
/// 365-day window without product-level penalty.
pub const MAX_FREE_ABSTENTIONS_PER_YEAR: u32 = 3;

/// Window used when deciding whether an abstention is "free".
pub const ABSTENTION_WINDOW_DAYS: i64 = 365;

/// Standard civic rating increment/decrement unit (0.035).
#[must_use]
pub fn jury_civic_rating_delta() -> Decimal {
    Decimal::new(35, 3)
}

/// Scale the basal civic delta by an integer multiplier (e.g. 3 → 0.105).
#[must_use]
pub fn civic_rating_delta_times(multiplier: i64) -> Decimal {
    jury_civic_rating_delta() * Decimal::from(multiplier)
}

/// Alive entities with rating strictly below this show failing UI (arterial red @handle).
#[must_use]
pub fn failing_rating_threshold() -> Decimal {
    Decimal::new(66, 2)
}

/// Community-edge multiplier when a post conversation settles Loved (+) or Loathed (−).
pub const RATING_MULT_CONVERSATION_LOVED_EDGE: i64 = 3;
pub const RATING_MULT_CONVERSATION_LOATHED_EDGE: i64 = -3;
/// Global direct multiplier on the author when a conversation settles Loved (+) or Loathed (−).
pub const RATING_MULT_CONVERSATION_LOVED_GLOBAL: i64 = 1;
pub const RATING_MULT_CONVERSATION_LOATHED_GLOBAL: i64 = -1;

/// Juror participation reward/penalty magnitude (±3×δ).
pub const RATING_MULT_JUROR_PARTICIPATION: i64 = 3;
/// Clear-verdict originator reward/penalty on moderation or adopted/rejected motions (±6×δ).
pub const RATING_MULT_ORIGINATOR_CLEAR: i64 = 6;
/// Hung constitutional/profile proposal without a clear supermajority (−3×δ on proposer).
pub const RATING_MULT_ORIGINATOR_HUNG_PROPOSAL: i64 = 3;

/// A freshly seated `pending` juror invite (for outbound notification).
#[derive(Debug, Clone, Copy)]
pub struct CreatedJurorInvite {
    pub invite_id: u64,
    pub juror_user_id: u64,
}

#[derive(Debug, Clone, Default)]
pub struct JuryNominationSummary {
    pub cases_examined: u64,
    pub cases_filled: u64,
    pub invites_created: u64,
    pub cases_closed_no_jury: u64,
    pub new_invites: Vec<CreatedJurorInvite>,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct OpenCaseRow {
    pub id: u64,
    pub originator_social_entity_id: u64,
    pub target_social_entity_id: u64,
    pub created_by: Option<u64>,
    pub trigger_key: String,
    pub subject_post_id: Option<u64>,
    pub jury_size: u8,
    pub status_key: String,
}

#[derive(Debug, Clone)]
pub(crate) struct JurorCandidate {
    pub juror_social_entity_id: u64,
    pub juror_user_id: Option<u64>,
    pub juror_rating_at_invite: Decimal,
    pub rating_basis_id: u64,
    pub rating_community_entity_id: Option<u64>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct JurorCandidateRanked {
    pub juror_social_entity_id: u64,
    pub juror_user_id: Option<u64>,
    pub juror_rating_at_invite: Decimal,
    pub source_priority: i32,
    pub rating_basis_id: u64,
    pub rating_community_entity_id: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerdictOutcome {
    /// Original filing assertion stands (e.g. post violates standards; amendment adopted).
    Upheld,
    /// Filing assertion rejected (e.g. post cleared; amendment not adopted).
    Overturned,
    /// Legacy rows only; resolution now maps hung bands to [`super::verdict_eval::safety_verdict_on_hung`].
    Hung,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastVoteOutcome {
    Recorded,
    NotInvited,
    AlreadyResponded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbstainOutcome {
    Free { prior_in_window: u32 },
    OverFreeQuota { prior_in_window: u32, cap: u32 },
    NotInvited,
    AlreadyResponded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclineOutcome {
    Recorded,
    NotInvited,
    AlreadyResponded,
}

/// Closed membership-review application from the applicant's perspective.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MembershipReviewApplicantOutcome {
    Denied,
    NoJury,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorWithdrawPostUnderReviewOutcome {
    ThreadDeleted,
    NotFound,
    Forbidden,
    NotUnderReview,
    CaseTerminal,
    WrongTrigger,
    ThreadHasOtherAuthors,
}

/// Seated juror invitation the viewer has not yet answered (`pending`).
#[derive(Debug, Clone)]
pub struct PendingJurorInvite {
    pub invite_id: u64,
    pub case_id: u64,
    pub subject_post_id: Option<u64>,
    pub trigger_key: String,
    pub community_handle: Option<String>,
    pub community_label: Option<String>,
    /// Emergency safety or serious moderation tier — abstention does not consume annual quota.
    pub is_severe_case: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct VerdictResolutionRetrySummary {
    pub candidates: u64,
    pub resolved: u64,
}
