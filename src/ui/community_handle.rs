//! Community `@handle` presentation — thin aliases over [`crate::ui::entity_handle`].

pub use crate::ui::entity_handle::{
    entity_handle_css_classes, entity_handle_line_html, entity_handle_tone, EntityHandleKind,
    EntityHandleTone,
};

pub type CommunityHandleTone = EntityHandleTone;

#[must_use]
pub fn community_handle_tone(
    rating: rust_decimal::Decimal,
    entity_status_key: Option<&str>,
    death_immune: bool,
) -> Option<CommunityHandleTone> {
    entity_handle_tone(rating, entity_status_key, death_immune)
}

#[must_use]
pub fn community_handle_css_classes(tone: CommunityHandleTone) -> &'static str {
    entity_handle_css_classes(tone, EntityHandleKind::Community)
}

#[must_use]
pub fn community_handle_line_html(handle_with_at: &str, tone: CommunityHandleTone) -> String {
    entity_handle_line_html(handle_with_at, tone, EntityHandleKind::Community)
}
