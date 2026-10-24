//! Maps `social_entity_type` ref `key` values to CSS class suffixes for palette accents (rail rings, profile card top border).
//!
//! **Post cards** must use the type of the **posting** `social_entity` (`post.entity_id` — who the post is *as*), not the
//! human operator. Example: a human posting as a non-profit org yields `non_profit` → burgundy (`--c-magenta`) top accent.

use crate::ref_keys;

/// Classes on `.app-rail-slot-frame` for the avatar ring (`brand.css` `app-rail-slot-frame--*`).
#[must_use]
pub fn rail_slot_frame_class(type_key: &str) -> &'static str {
    if type_key == ref_keys::KEY_ENTITY_TYPE_HUMAN {
        " app-rail-slot-frame--human"
    } else if type_key == ref_keys::KEY_ENTITY_TYPE_DIGITAL {
        " app-rail-slot-frame--digital"
    } else if type_key == ref_keys::KEY_ENTITY_TYPE_COMMUNITY {
        " app-rail-slot-frame--community"
    } else if type_key == ref_keys::KEY_ENTITY_TYPE_NON_PROFIT {
        " app-rail-slot-frame--non_profit"
    } else if type_key == ref_keys::KEY_ENTITY_TYPE_PROFIT {
        " app-rail-slot-frame--profit"
    } else if type_key == ref_keys::KEY_ENTITY_TYPE_GATHERING {
        " app-rail-slot-frame--gathering"
    } else {
        ""
    }
}

/// Classes on public/private profile cards and feed/public post cards for `--card-accent-bg` (`brand.css` `entity-profile-accent--*`).
/// `type_key` is the `ref_key_values.key` for [`crate::ref_keys::GROUP_SOCIAL_ENTITY_TYPE`] (e.g. [`crate::ref_keys::KEY_ENTITY_TYPE_NON_PROFIT`] = `"non_profit"` → burgundy ring / top border).
#[must_use]
pub fn profile_card_accent_class(type_key: &str) -> &'static str {
    if type_key == ref_keys::KEY_ENTITY_TYPE_HUMAN {
        " entity-profile-accent--human"
    } else if type_key == ref_keys::KEY_ENTITY_TYPE_DIGITAL {
        " entity-profile-accent--digital"
    } else if type_key == ref_keys::KEY_ENTITY_TYPE_COMMUNITY {
        " entity-profile-accent--community"
    } else if type_key == ref_keys::KEY_ENTITY_TYPE_NON_PROFIT {
        " entity-profile-accent--non_profit"
    } else if type_key == ref_keys::KEY_ENTITY_TYPE_PROFIT {
        " entity-profile-accent--profit"
    } else if type_key == ref_keys::KEY_ENTITY_TYPE_GATHERING {
        " entity-profile-accent--gathering"
    } else {
        ""
    }
}
