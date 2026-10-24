//! Display labels for `social_join_policy` ref keys (profile cards, summaries).

use crate::i18n::Catalog;
use crate::ref_keys;

/// User-facing label for a community's join policy on profile surfaces.
pub fn community_join_policy_display(c: &Catalog, loc: &str, policy_key: &str) -> String {
    match policy_key {
        ref_keys::KEY_JOIN_POLICY_REVIEWED => {
            c.t(loc, "ui.home.create_entity_community_join_reviewed_label")
        }
        ref_keys::KEY_JOIN_POLICY_OPEN => c.t(loc, "ref_values.social_join_policy.open"),
        ref_keys::KEY_JOIN_POLICY_INVITE_ONLY => c.t(loc, "ref_values.social_join_policy.invite_only"),
        ref_keys::KEY_JOIN_POLICY_CLOSED => c.t(loc, "ref_values.social_join_policy.closed"),
        other => other.replace('_', " "),
    }
}

/// Verified-member metric line for community private/public profile cards.
pub fn community_members_heading(
    c: &Catalog,
    loc: &str,
    count: u64,
    join_policy_key: Option<&str>,
) -> String {
    let policy = join_policy_key
        .map(|k| community_join_policy_display(c, loc, k))
        .unwrap_or_else(|| c.t(loc, "ref_values.social_join_policy.open"));
    c.tf(
        loc,
        "ui.entity_private.community_members_heading",
        &[("count", &count.to_string()), ("policy", &policy)],
    )
}
