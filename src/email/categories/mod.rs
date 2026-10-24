//! Product mail grouped by intent:
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`recruitment`] | Invitations to join the platform |
//! | **engagement** | Standing, inactivity — `crate::email::notify::standing` (migrate here) |
//! | **judiciary** | Jury, emergency safety, council — `crate::email::notify::{judicial,moderation,ai}` |
//!
//! **Auth** (verify / reset) stays at `crate::email::auth` — system initiator, not social entity.

pub mod council_invite;
pub mod recruitment;
