//! Outbound email for 256.app.
//!
//! ## Three layers (initiator → function → template → dispatch)
//!
//! 1. **Initiator** — who the mail is from ([`initiator::MailInitiator`], usually `social_entity.id`).
//!    Resolved in `social::recruitment` (permissions) or job context (system / judiciary).
//! 2. **Category function** — product intent in [`categories`] (`recruitment`, `engagement`, `judiciary`).
//! 3. **Template** — copy in [`render::MailTemplate`] + branded HTML in [`layout`].
//! 4. **Dispatch** — SMTP in [`dispatch`] (never called from route handlers directly).
//!
//! ## Categories
//!
//! | Category | Examples | Initiator |
//! |----------|----------|-----------|
//! | **Recruitment** | Platform invite, profit client list, AI friend outreach | `social_entity_id` |
//! | **Engagement** | Standing failing, inactivity, future @mentions / connection notices | entity or user context |
//! | **Judiciary** | Jury summons, emergency safety, council appeal outcomes | case / community context |
//! | **Auth** (system) | Verify email, password reset | [`initiator::MailInitiator::System`] |
//!
//! Legacy [`notify`] helpers remain during migration; new mail should use [`categories`] + [`render`].

mod auth;
pub mod categories;
pub mod dispatch;
pub mod initiator;
pub mod layout;
pub mod notify;
pub mod render;
mod transport;

pub use auth::{send_password_reset_email, send_verification_email};
pub use categories::recruitment::{send_platform_invite, send_platform_invites_bulk, BulkInviteItemResult, BulkInviteSummary};
pub use notify::{
    notify_ai_council_decision_available, notify_community_failing_admins,
    notify_emergency_contest_opened, notify_emergency_contest_verdict_outcomes,
    notify_emergency_filed_to_author, notify_emergency_filed_to_filer,
    notify_emergency_standing_applied, notify_human_community_edge_failing,
    notify_human_global_failing, notify_human_global_recovery, notify_human_inactivity_acceleration_3x,
    notify_human_inactivity_acceleration_6x, notify_human_inactivity_decrement_started,
    notify_human_inactivity_pruning, notify_human_inactivity_warning,
    notify_juror_invites_created,
};
