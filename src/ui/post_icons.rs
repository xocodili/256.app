//! Feed post action icons — shared SVG sprite at `/static/icons/post-actions.svg`.

/// Bump `v=` when adding symbols to `static/icons/post-actions.svg` (browsers cache the sprite document for `<use>`).
const SPRITE_HREF: &str = "/static/icons/post-actions.svg?v=20260814-logout";
const ICON_VIEW: &str = "0 0 24 24";

fn sprite_use(symbol_id: &str, class: &str, width: u16, height: u16) -> String {
    format!(
        r#"<svg class="{class}" viewBox="{view}" width="{width}" height="{height}" aria-hidden="true" xmlns="http://www.w3.org/2000/svg"><use href="{href}#{symbol_id}"/></svg>"#,
        class = class,
        view = ICON_VIEW,
        width = width,
        height = height,
        href = SPRITE_HREF,
        symbol_id = symbol_id,
    )
}

/// Up arrow (engagement vote yes).
#[must_use]
pub fn post_icon_vote_up(_suffix: &str) -> String {
    sprite_use("icon-vote-up", "post-action-icon post-action-icon--vote-up system-icon--md", 32, 32)
}

/// Down arrow (engagement vote no).
#[must_use]
pub fn post_icon_vote_down(_suffix: &str) -> String {
    sprite_use("icon-vote-down", "post-action-icon post-action-icon--vote-down system-icon--md", 32, 32)
}

/// Share link to a new audience (export arrow).
#[must_use]
pub fn post_icon_share_link(_suffix: &str) -> String {
    sprite_use("icon-share", "post-action-icon post-action-icon--share system-icon--md", 32, 32)
}

/// Copy public post URL.
#[must_use]
pub fn post_icon_copy_public_link(_suffix: &str) -> String {
    sprite_use("icon-copy-link", "post-action-icon post-action-icon--copy-link system-icon--md", 32, 32)
}

/// Open create-post for a private @private message to a connected human.
#[must_use]
pub fn post_icon_private_message(_suffix: &str) -> String {
    sprite_use(
        "icon-private-message",
        "post-action-icon post-action-icon--private-message system-icon--md",
        32,
        32,
    )
}

/// File a post dispute (open case).
#[must_use]
pub fn post_icon_open_case(_suffix: &str) -> String {
    sprite_use("icon-open-case", "post-action-icon post-action-icon--open-case system-icon--md", 32, 32)
}

/// Escalate resolved dispute to AI Council.
#[must_use]
pub fn post_icon_escalate_council(_suffix: &str) -> String {
    sprite_use(
        "icon-escalate",
        "post-action-icon post-action-icon--escalate system-icon--md",
        32,
        32,
    )
}

/// Compact reply arrow (thread row action).
#[must_use]
pub fn post_icon_reply() -> String {
    sprite_use("icon-reply", "post-action-icon post-action-icon--reply system-icon--sm", 16, 16)
}

/// Thread comment count (pre-vote engagement bar).
#[must_use]
pub fn post_icon_thread_count() -> String {
    sprite_use("icon-thread-count", "post-engagement-count-icon system-icon--sm", 16, 16)
}

/// Edit profile / entity details.
#[must_use]
pub fn post_icon_edit(_suffix: &str) -> String {
    sprite_use("icon-edit", "post-action-icon post-action-icon--edit system-icon--md", 32, 32)
}

/// Regenerate avatar art from stored face reference.
#[must_use]
pub fn post_icon_avatar_redraw(_suffix: &str) -> String {
    sprite_use(
        "icon-avatar-redraw",
        "post-action-icon post-action-icon--avatar-redraw system-icon--md",
        32,
        32,
    )
}

/// Sign out — fire-exit door + arrow (profile header; semantic red, not yellow hover).
#[must_use]
pub fn post_icon_logout(_suffix: &str) -> String {
    sprite_use(
        "icon-logout",
        "post-action-icon post-action-icon--logout system-icon--md",
        32,
        32,
    )
}

/// Feel heart for the post-vote compose row.
#[must_use]
pub fn post_icon_feel_heart(feel_key: &str) -> String {
    let fill = match feel_key {
        "loved" => "var(--c-yellow)",
        "controversial" => "var(--c-cyan)",
        _ => "var(--c-red)",
    };
    format!(
        r#"<svg class="post-engagement-feel-heart" viewBox="{view}" width="22" height="22" aria-hidden="true" xmlns="http://www.w3.org/2000/svg"><use href="{href}#icon-feel-heart" fill="{fill}"/></svg>"#,
        view = ICON_VIEW,
        href = SPRITE_HREF,
        fill = fill,
    )
}
