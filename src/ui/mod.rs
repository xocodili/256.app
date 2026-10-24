//! Reusable HTML fragments: icons, inline SVG, and other visual building blocks.
//!
//! Add new snippets under [`snippets`] and inject them into templates via placeholders
//! (e.g. `__UI_TOOLBAR_SMILEY__`) replaced at render time.

pub mod datetime_format;
pub mod fragments;
pub mod avatar_url;
pub mod change_password_panel;
pub mod community_handle;
pub mod entity_handle;
pub mod countries;
pub mod entity_type_accent;
pub mod join_policy;
pub mod legal_card;
pub mod nom_icon;
pub mod outstanding_invitations;
pub mod post_card;
pub mod post_icons;
pub mod private_profile_card;
pub mod rail;
pub mod panoply;
pub mod snippets;
