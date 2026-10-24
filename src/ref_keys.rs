//! Canonical `ref_key_values`.`group` and `key` slugs shared across users, social entities, and edges.
//!
//! # Status group (`GROUP_STATUS`)
//! One row set applies everywhere `status_id` is used. Semantics:
//! - **`pending_verification`**: default for new rows; not yet through the full verification pipeline.
//! - **`verified`**: composite verification satisfied (email verification alone is only one future input).
//! - **`blocked`**: active enforcement — relationships or access you must honor.
//! - **`retired`**: e.g. deceased user or sunset entity; content may remain, distinct from blocks.
//! - **`to_be_deleted`**: scheduled removal.

/// `ref_key_values`.`group` for lifecycle status (users, `social_entity`, `social_entity_relationship`, …).
pub const GROUP_STATUS: &str = "status";
pub const GROUP_USER_STATUS: &str = "user_status";
pub const GROUP_SOCIAL_ENTITY_STATUS: &str = "social_entity_status";
pub const GROUP_SOCIAL_RELATIONSHIP_STATUS: &str = "social_relationship_status";

/// `ref_key_values`.`group` for `social_entity.type_id`.
pub const GROUP_SOCIAL_ENTITY_TYPE: &str = "social_entity_type";

/// `social_entity_type` key for event-style entities; details in `social_gathering`.
pub const KEY_ENTITY_TYPE_GATHERING: &str = "gathering";

/// `social_entity_type` key for people; structured name + DOB in `social_human`.
pub const KEY_ENTITY_TYPE_HUMAN: &str = "human";

/// `social_entity_type` key for communities; see `social_community`.
pub const KEY_ENTITY_TYPE_COMMUNITY: &str = "community";

/// `social_entity_type` key for machine accounts; see `social_digital`.
pub const KEY_ENTITY_TYPE_DIGITAL: &str = "digital";

/// `social_entity_type` key for profit-seeking organizations; see `social_profit`.
pub const KEY_ENTITY_TYPE_PROFIT: &str = "profit";

/// `social_entity_type` key for non-profit organizations; see `social_non_profit`.
pub const KEY_ENTITY_TYPE_NON_PROFIT: &str = "non_profit";

/// `ref_key_values`.`group` for `social_digital.digital_type_id`.
pub const GROUP_SOCIAL_DIGITAL_TYPE: &str = "social_digital_type";

pub const KEY_DIGITAL_TYPE_AI_AGENT: &str = "ai_agent";
pub const KEY_DIGITAL_TYPE_BOT: &str = "bot";
pub const KEY_DIGITAL_TYPE_SERVICE_ACCOUNT: &str = "service_account";
pub const KEY_DIGITAL_TYPE_AUTOMATION: &str = "automation";
pub const KEY_DIGITAL_TYPE_INTEGRATION: &str = "integration";

/// `ref_key_values`.`group` for `join_policy_id` on `social_gathering`, `social_human`, `social_community`, `social_digital`, `social_profit`, `social_non_profit`.
pub const GROUP_SOCIAL_JOIN_POLICY: &str = "social_join_policy";

pub const KEY_JOIN_POLICY_OPEN: &str = "open";
pub const KEY_JOIN_POLICY_REVIEWED: &str = "reviewed";
/// Default join policy for new **business** (`social_profit`) entities: admin roles require invitation.
pub const KEY_JOIN_POLICY_INVITE_ONLY: &str = "invite_only";
pub const KEY_JOIN_POLICY_CLOSED: &str = "closed";

/// `ref_key_values`.`group` for `social_gathering.recurrence_frequency_id` (NULL = one-off).
pub const GROUP_SOCIAL_GATHERING_RECURRENCE: &str = "social_gathering_recurrence";

/// Recurrence frequency keys; combine with `social_gathering.recurrence_interval` (e.g. weekly + 2 ≡ `fortnightly`).
pub const KEY_RECURRENCE_DAILY: &str = "daily";
pub const KEY_RECURRENCE_WEEKLY: &str = "weekly";
pub const KEY_RECURRENCE_FORTNIGHTLY: &str = "fortnightly";
pub const KEY_RECURRENCE_MONTHLY: &str = "monthly";
pub const KEY_RECURRENCE_YEARLY: &str = "yearly";

/// `ref_key_values`.`group` for `users.user_type_id` (human vs platform AI accounts).
pub const GROUP_USER_TYPE: &str = "user_type";

pub const KEY_USER_TYPE_HUMAN: &str = "human";

pub const KEY_USER_TYPE_AI: &str = "ai";

pub const GROUP_USER_SOCIAL_ENTITY_ROLE: &str = "user_social_entity_role";
pub const KEY_USER_SOCIAL_ENTITY_ROLE_OWNER: &str = "owner";

pub const GROUP_AI_COUNCIL_ROLE: &str = "ai_council_role";
pub const KEY_AI_COUNCIL_ROLE_MEMBER: &str = "member";
pub const KEY_AI_COUNCIL_ROLE_LEAD: &str = "lead";

pub const GROUP_AI_DOMAIN_OWNERSHIP_ROLE: &str = "ai_domain_ownership_role";
pub const KEY_AI_DOMAIN_OWNERSHIP_PRIMARY: &str = "primary";
pub const KEY_AI_DOMAIN_OWNERSHIP_SECONDARY: &str = "secondary";

pub const KEY_PENDING_VERIFICATION: &str = "pending_verification";
pub const KEY_VERIFIED: &str = "verified";
pub const KEY_RETIRED: &str = "retired";
pub const KEY_TO_BE_DELETED: &str = "to_be_deleted";
pub const KEY_BLOCKED: &str = "blocked";
/// Community terminated by graph lifecycle (rating zero death job).
pub const KEY_ENTITY_STATUS_DEAD: &str = "dead";
pub const KEY_FLAGGED: &str = "flagged";
/// Emergency safety hold — post hidden from all viewers including author.
pub const KEY_POST_SUPPRESSED: &str = "suppressed";
/// Post lifecycle `status` ref key while a civic dispute case is active on the post.
pub const KEY_POST_UNDER_JUDICIAL_REVIEW: &str = "under_judicial_review";

pub const GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE: &str = "social_entity_relationship_type";
pub const KEY_RELATIONSHIP_FRIEND: &str = "friend";
pub const KEY_RELATIONSHIP_FAMILY: &str = "family";
pub const KEY_RELATIONSHIP_COLLEAGUE: &str = "colleague";
pub const KEY_RELATIONSHIP_FOLLOW: &str = "follow";
pub const KEY_RELATIONSHIP_EVENT_GOING: &str = "event_going";
pub const KEY_RELATIONSHIP_EVENT_NOT_GOING: &str = "event_not_going";
pub const KEY_RELATIONSHIP_EVENT_INTERESTED: &str = "event_interested";
pub const KEY_RELATIONSHIP_MEMBER_OF: &str = "member_of";
pub const KEY_RELATIONSHIP_MODERATOR: &str = "moderator";
pub const KEY_RELATIONSHIP_GODMOTHER: &str = "godmother";

// ---------------------------------------------------------------------------
// Seeded platform identity (`migrations/20260411000002_seed_system_presence.sql`).
// ---------------------------------------------------------------------------

/// Email of the deploy-time platform user; `email_verified_at` is set at seed; `password_hash` is NULL (not for normal login).
pub const SYSTEM_USER_EMAIL: &str = "system@256.app";

/// Public handle (without `@`) for the platform `social_entity` row.
pub const SYSTEM_ENTITY_HANDLE: &str = "256";

/// Seeded sample for-profit entity.
pub const SEED_PROFIT_ENTITY_HANDLE: &str = "examplestudio";

/// Invite-only platform safety operator community (`member_of` → ops dashboard + death immunity).
pub const PANOPLY_COMMUNITY_HANDLE: &str = "panoply";

/// Seeded Panoply operator human (`operator@example.com`).
pub const SEED_PANOPLY_OPERATOR_HANDLE: &str = "sphinx";

/// Seeded Panoply operator login email.
pub const SEED_PANOPLY_OPERATOR_EMAIL: &str = "operator@example.com";

/// Seeded `256.app` community entity (global location).
pub const SEED_COMMUNITY_256_HANDLE: &str = "256app";

/// Root Operating Charter community (`operating_charter_256`).
pub const ROOT_OPERATING_CHARTER_HANDLE: &str = "operating_charter_256";

/// `ref_key_values`.`group` for `locations.location_lifecycle_id`.
pub const GROUP_LOCATION_LIFECYCLE: &str = "location_lifecycle";

pub const KEY_LOCATION_LIFECYCLE_ACTIVE: &str = "active";
pub const KEY_LOCATION_LIFECYCLE_DEPRECATED: &str = "deprecated";
pub const KEY_LOCATION_LIFECYCLE_OBSOLETE: &str = "obsolete";

/// `ref_key_values`.`group` for `users.verification_tier_id` (product entitlements).
pub const GROUP_VERIFICATION_TIER: &str = "verification_tier";

/// Onboarding-complete baseline (country, face sighting, avatar, AI godmother — enforced elsewhere).
pub const KEY_VERIFICATION_TIER_BASIC: &str = "basic";
/// Dating / marketplace-style flows: precise, Google-validated place (see `crate::verification`).
pub const KEY_VERIFICATION_TIER_ENHANCED: &str = "enhanced";
/// Registered profit / non-profit presence: location plus future KYB (see `crate::verification`).
pub const KEY_VERIFICATION_TIER_PROFESSIONAL: &str = "professional";

/// `ref_key_values`.`group` for `judicial_case.trigger_type_id`.
pub const GROUP_JUDICIAL_CASE_TRIGGER: &str = "judicial_case_trigger";

pub const KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW: &str = "membership_review";
/// Stored `judicial_case_trigger` ref key (legacy substring in DB enum value).
pub const KEY_JUDICIAL_TRIGGER_CASE_APPEAL: &str = "moderation_appeal";
/// Legacy slug; deactivated in favour of [`KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE`].
pub const KEY_JUDICIAL_TRIGGER_POST_DISPUTE: &str = "post_moderation";
/// Emergency safety filing citing Root §4.1–§4.7 — suppress post, no jury until author contests.
pub const KEY_JUDICIAL_TRIGGER_EMERGENCY_SAFETY: &str = "emergency_safety";
/// Civic charter dispute on a post — jury from filing.
pub const KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE: &str = "civic_dispute";
pub const KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT: &str = "constitutional_amendment";
pub const KEY_JUDICIAL_TRIGGER_COMMUNITY_PROFILE_AMENDMENT: &str = "community_profile_amendment";
pub const KEY_JUDICIAL_TRIGGER_COMMUNITY_REPARENT: &str = "community_reparent";

/// `ref_key_values`.`group` for `judicial_case.status_id`.
pub const GROUP_JUDICIAL_CASE_STATUS: &str = "judicial_case_status";

pub const KEY_JUDICIAL_CASE_OPEN: &str = "open";
pub const KEY_JUDICIAL_CASE_JURY_ACTIVE: &str = "jury_active";
pub const KEY_JUDICIAL_CASE_ESCALATED_TO_AI: &str = "escalated_to_ai";
pub const KEY_JUDICIAL_CASE_RESOLVED: &str = "resolved";
/// Post-moderation case closed after 21 days without a seated jury; post stays moderated.
pub const KEY_JUDICIAL_CASE_NO_JURY: &str = "no_jury";
pub const KEY_JUDICIAL_CASE_VOIDED: &str = "voided";
/// Emergency safety case closed uncontested or awaiting contest window.
pub const KEY_JUDICIAL_CASE_CONTAINED: &str = "contained";

/// `ref_key_values`.`group` for `judicial_case_comment.type_id`.
pub const GROUP_JUDICIAL_CASE_COMMENT_TYPE: &str = "judicial_case_comment_type";

pub const KEY_JCC_ORIGINATOR: &str = "originator";
pub const KEY_JCC_EVIDENCE: &str = "evidence";
pub const KEY_JCC_REBUTTAL: &str = "rebuttal";
pub const KEY_JCC_AI_BRIEF: &str = "ai_brief";
pub const KEY_JCC_VERDICT_JUSTIFICATION: &str = "verdict_justification";
pub const KEY_JCC_CLOSING_STATEMENT: &str = "closing_statement";

/// `ref_key_values`.`group` for `judicial_case_juror_invite.invite_status_id`.
pub const GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS: &str = "judicial_case_juror_invite_status";

pub const KEY_JCJI_PENDING: &str = "pending";
pub const KEY_JCJI_ACCEPTED: &str = "accepted";
pub const KEY_JCJI_DECLINED: &str = "declined";
pub const KEY_JCJI_ABSTAINED: &str = "abstained";
pub const KEY_JCJI_EXPIRED: &str = "expired";
pub const KEY_JCJI_VOIDED: &str = "voided";

/// `ref_key_values`.`group` for `judicial_case_juror_invite.vote_direction_id`.
pub const GROUP_JUDICIAL_CASE_JUROR_INVITE_VOTE_DIRECTION: &str =
    "judicial_case_juror_invite_vote_direction";

pub const KEY_JCJI_VOTE_FOR: &str = "for";
pub const KEY_JCJI_VOTE_AGAINST: &str = "against";

/// `ref_key_values`.`group` for `judicial_case_juror_invite.rating_basis_id`.
pub const GROUP_JUDICIAL_CASE_JUROR_INVITE_RATING_BASIS: &str =
    "judicial_case_juror_invite_rating_basis";

pub const KEY_JCJI_RB_GLOBAL: &str = "global";

/// `ref_key_values`.`group` for `judicial_case.resolution_tier_id`.
pub const GROUP_JUDICIAL_RESOLUTION_TIER: &str = "judicial_resolution_tier";
/// Post-dispute moderation severity (`regular` | `serious`) on constitution items.
pub const GROUP_JUDICIAL_MODERATION_TIER: &str = "judicial_moderation_tier";

pub const KEY_JUDICIAL_RESOLUTION_TIER_JURY: &str = "jury";
pub const KEY_JUDICIAL_RESOLUTION_TIER_AI_COUNCIL: &str = "ai_council";
pub const KEY_JUDICIAL_MODERATION_TIER_REGULAR: &str = "regular";
pub const KEY_JUDICIAL_MODERATION_TIER_SERIOUS: &str = "serious";

/// `ref_key_values`.`group` for `social_human_ai_token_ledger.reason_id`.
pub const GROUP_NOM_LEDGER_REASON: &str = "nom_ledger_reason";

pub const KEY_NOM_LEDGER_CREDIT_SIGNUP: &str = "credit_signup";
pub const KEY_NOM_LEDGER_CREDIT_DAILY_LOGIN: &str = "credit_daily_login";
pub const KEY_NOM_LEDGER_CREDIT_JURY_DUTY: &str = "credit_jury_duty";
pub const KEY_NOM_LEDGER_CREDIT_PURCHASE: &str = "credit_purchase";
pub const KEY_NOM_LEDGER_SPEND_BILLABLE_SERVICE: &str = "spend_billable_service";
pub const KEY_NOM_LEDGER_ADMIN_ADJUSTMENT: &str = "admin_adjustment";
pub const KEY_JCJI_RB_COMMUNITY: &str = "community";

/// `ref_key_values`.`group` for `social_human.death_source_id` / `social_community.death_source_id`.
pub const GROUP_GRAPH_DEATH_SOURCE: &str = "graph_death_source";

pub const KEY_GRAPH_DEATH_SOURCE_RATING_ZERO: &str = "rating_zero";
pub const KEY_GRAPH_DEATH_SOURCE_MANUAL_OPS: &str = "manual_ops";
pub const KEY_GRAPH_DEATH_SOURCE_USER_REQUESTED: &str = "user_requested";
