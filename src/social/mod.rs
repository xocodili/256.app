//! Social graph backbone: [`entity`] (presence) and [`relationship`] (edges).
//!
//! Shared building blocks: [`ids`], [`db`]. All **`status_id`** FKs use the same
//! [`crate::ref_keys::GROUP_STATUS`] ref rows (users, entities, relationships).
//! Event-style presence uses `social_entity_type.gathering` plus the `social_gathering` extension table
//! (`label`, schedule, optional recurrence via `ref_keys::GROUP_SOCIAL_GATHERING_RECURRENCE`; see migrations).
//! People use `social_entity_type.human` plus `social_human` (names, `birth_date`, optional `birth_location_id`, `join_policy_id`; display name is `social_entity.handle`).
//! Communities use `social_entity_type.community` plus `social_community` (`label`, `description`, `intention`, `rules`, `founded_at`, `join_policy_id`).
//! `social_gathering` and `social_digital` also carry `join_policy_id`. Join policy refs: `ref_keys::GROUP_SOCIAL_JOIN_POLICY`
//! (`open`, `reviewed`, `invite_only`, `closed` — semantics enforced when joining, not only at insert).
//! Digital / machine accounts use `social_entity_type.digital` plus `social_digital` (`digital_type_id`, `join_policy_id`, optional `label`, `description`, `intention`, `operator_disclosure`, `activated_at`).
//! For-profit orgs use `social_entity_type.profit` plus `social_profit` (`join_policy_id`, optional `label`, `description`, `intention`, legal/registry fields, `founded_at`).
//! New businesses default to **invite-only** join policy for administrative roles; public following as a lightweight connection is planned separately.
//! Non-profits use `social_entity_type.non_profit` plus `social_non_profit` (same column shape as `social_profit`; `registration_number` is charity / society id locally).

pub mod audience;
pub mod audit;
pub mod blocks;
pub mod community_constitution;
pub mod community_graph;
pub mod community_invite;
pub mod council_invite;
pub mod council_invite_contact;
pub mod council_invite_pipeline;
pub mod council_jobs;
pub mod council_members;
pub mod council_thread_dossier;
pub mod council_portfolio;
pub mod council_share_pipeline;
pub mod council_publish;
pub mod godmother;
pub mod brand_logo;
pub mod connections;
pub mod db;
pub mod entity;
pub mod entity_profile_edit;
pub mod conversation_rating;
pub mod engagement;
pub mod engagement_digest;
pub mod feed;
pub mod face_capture_diversity;
pub mod face_capture_errors;
pub mod face_capture_pending;
pub mod face_capture_quality;
pub mod face_capture_validate;
pub mod recruitment;
pub mod recruitment_outreach;
pub mod mentions;
pub mod post_community;
pub mod moderation_mirror;
pub mod thread;
pub mod home_context;
pub mod human_onboarding;
pub mod ids;
pub mod integrity;
pub mod judicial;
pub mod judicial_constitutional;
pub mod judicial_community_profile;
pub mod judicial_link_evidence;
pub mod noms;
pub mod panoply;
pub mod judicial_council;
pub mod ownership;
pub mod pending_invitations;
pub mod private_profile;
pub mod profile_actions;
pub mod public_profile;
pub mod recommendations;
pub mod rating_notify;
pub mod relationship;
pub mod safety;
pub mod scope;
pub mod user_faces;
