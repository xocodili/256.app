//! `@handle` presentation by `social_entity.rating` and lifecycle status (humans and communities).

use rust_decimal::Decimal;

use crate::ref_keys;
use crate::social::judicial::failing_rating_threshold;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityHandleTone {
    /// `rating >= failing threshold` and alive (communities: sun-burst yellow; humans: default site yellow).
    Healthy,
    /// `rating < failing threshold` and not dead.
    Failing,
    /// `social_entity_status.dead`.
    Dead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityHandleKind {
    Community,
    Human,
    Brand,
    Gathering,
}

#[must_use]
pub fn entity_handle_tone(
    rating: Decimal,
    entity_status_key: Option<&str>,
    death_immune: bool,
) -> Option<EntityHandleTone> {
    if death_immune {
        return Some(EntityHandleTone::Healthy);
    }
    if entity_status_key == Some(ref_keys::KEY_ENTITY_STATUS_DEAD) {
        return Some(EntityHandleTone::Dead);
    }
    if rating < failing_rating_threshold() {
        return Some(EntityHandleTone::Failing);
    }
    Some(EntityHandleTone::Healthy)
}

/// Human failing-band styling is private to the entity owner (like rating stars).
#[must_use]
pub fn effective_handle_tone(
    tone: EntityHandleTone,
    kind: EntityHandleKind,
    reveal_failing_standing: bool,
) -> EntityHandleTone {
    if tone == EntityHandleTone::Failing
        && kind == EntityHandleKind::Human
        && !reveal_failing_standing
    {
        EntityHandleTone::Healthy
    } else {
        tone
    }
}

#[must_use]
pub fn entity_handle_css_classes(tone: EntityHandleTone, kind: EntityHandleKind) -> &'static str {
    match (tone, kind) {
        (EntityHandleTone::Healthy, EntityHandleKind::Community) => {
            "public-post-handle-line--community-healthy"
        }
        (EntityHandleTone::Healthy, EntityHandleKind::Human) => "public-post-handle-line",
        (EntityHandleTone::Healthy, EntityHandleKind::Brand | EntityHandleKind::Gathering) => {
            "public-post-handle-line"
        }
        (EntityHandleTone::Failing, EntityHandleKind::Community) => {
            "public-post-handle-line--community-failing"
        }
        (EntityHandleTone::Failing, EntityHandleKind::Human) => {
            "public-post-handle-line--human-failing"
        }
        (EntityHandleTone::Failing, EntityHandleKind::Brand | EntityHandleKind::Gathering) => {
            "public-post-handle-line--human-failing"
        }
        (EntityHandleTone::Dead, EntityHandleKind::Community) => {
            "public-post-handle-line--community-dead"
        }
        (EntityHandleTone::Dead, EntityHandleKind::Human) => {
            "public-post-handle-line--human-dead"
        }
        (EntityHandleTone::Dead, EntityHandleKind::Brand | EntityHandleKind::Gathering) => {
            "public-post-handle-line--human-dead"
        }
    }
}

#[must_use]
pub fn entity_handle_line_html(
    handle_with_at: &str,
    tone: EntityHandleTone,
    kind: EntityHandleKind,
) -> String {
    let class = entity_handle_css_classes(tone, kind);
    format!(
        r#"<span class="public-post-handle-line {class}">{handle}</span>"#,
        class = class,
        handle = handle_with_at,
    )
}

#[must_use]
pub fn entity_handle_is_linkable(tone: EntityHandleTone) -> bool {
    tone != EntityHandleTone::Dead
}

#[must_use]
pub fn entity_handle_kind(type_key: Option<&str>) -> Option<EntityHandleKind> {
    match type_key {
        Some(ref_keys::KEY_ENTITY_TYPE_COMMUNITY) => Some(EntityHandleKind::Community),
        Some(ref_keys::KEY_ENTITY_TYPE_HUMAN) => Some(EntityHandleKind::Human),
        Some(ref_keys::KEY_ENTITY_TYPE_PROFIT) | Some(ref_keys::KEY_ENTITY_TYPE_NON_PROFIT) => {
            Some(EntityHandleKind::Brand)
        }
        Some(ref_keys::KEY_ENTITY_TYPE_GATHERING) => Some(EntityHandleKind::Gathering),
        _ => None,
    }
}

/// Feed card: handle block (linked unless dead).
#[must_use]
pub fn feed_entity_handle_block_html(
    handle: &str,
    handle_line: &str,
    linkable: bool,
) -> String {
    if linkable {
        format!(
            r#"<a class="public-post-handle feed-profile-link" href="/home/profile/{handle}" data-profile-handle="{handle}">{handle_line}</a>"#,
            handle = handle,
            handle_line = handle_line,
        )
    } else {
        format!(
            r#"<span class="public-post-handle public-post-handle--removed" aria-disabled="true">{handle_line}</span>"#,
            handle_line = handle_line,
        )
    }
}

/// Feed / profile: styled handle line for humans and communities when rating rules apply.
#[must_use]
pub fn entity_handle_line_for_type(
    handle_with_at: &str,
    rating: Decimal,
    entity_status_key: Option<&str>,
    type_key: Option<&str>,
    death_immune: bool,
    reveal_failing_standing: bool,
) -> String {
    let Some(kind) = entity_handle_kind(type_key) else {
        return format!(
            r#"<span class="public-post-handle-line">{}</span>"#,
            handle_with_at
        );
    };
    if let Some(tone) = entity_handle_tone(rating, entity_status_key, death_immune) {
        let tone = effective_handle_tone(tone, kind, reveal_failing_standing);
        entity_handle_line_html(handle_with_at, tone, kind)
    } else {
        format!(
            r#"<span class="public-post-handle-line">{}</span>"#,
            handle_with_at
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn human_failing_hidden_when_not_owner() {
        assert_eq!(
            effective_handle_tone(EntityHandleTone::Failing, EntityHandleKind::Human, false),
            EntityHandleTone::Healthy
        );
    }

    #[test]
    fn human_failing_shown_to_owner() {
        assert_eq!(
            effective_handle_tone(EntityHandleTone::Failing, EntityHandleKind::Human, true),
            EntityHandleTone::Failing
        );
    }

    #[test]
    fn community_failing_stays_public() {
        assert_eq!(
            effective_handle_tone(EntityHandleTone::Failing, EntityHandleKind::Community, false),
            EntityHandleTone::Failing
        );
    }

    #[test]
    fn failing_human_handle_line_masks_css_for_non_owner() {
        let html = entity_handle_line_for_type(
            "@alice",
            Decimal::new(50, 2),
            None,
            Some(ref_keys::KEY_ENTITY_TYPE_HUMAN),
            false,
            false,
        );
        assert!(
            !html.contains("human-failing"),
            "non-owner must not see failing human styling: {html}"
        );
    }

    #[test]
    fn failing_human_handle_line_shows_css_for_owner() {
        let html = entity_handle_line_for_type(
            "@alice",
            Decimal::new(50, 2),
            None,
            Some(ref_keys::KEY_ENTITY_TYPE_HUMAN),
            false,
            true,
        );
        assert!(
            html.contains("human-failing"),
            "owner must see failing human styling: {html}"
        );
    }
}
