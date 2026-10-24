//! Connection nav strip: pinned search/create + horizontally scrollable connections.

use crate::html_escape::escape_html;
use crate::i18n::Catalog;
use crate::ref_keys;
use crate::social::connections::ConnectionItem;
use crate::ui::avatar_url;
use crate::ui::entity_type_accent;
use crate::ui::snippets;

/// Shared system-icon sprite (`docs/ICONOGRAPHY.md` Phase 2).
const SYSTEM_ICONS_SPRITE: &str = "/static/icons/post-actions.svg?v=20260812-chrome-nav";

fn search_create_slot_html(loc: &str, c: &Catalog) -> String {
    let title = c.t(loc, "ui.home.rail_search_create");
    let title_esc = escape_html(&title);
    let placeholder = c.t(loc, "ui.home.search_add_placeholder");
    let placeholder_esc = escape_html(&placeholder);
    let input_label = c.t(loc, "ui.home.rail_search_input_label");
    let input_label_esc = escape_html(&input_label);
    let close_label = c.t(loc, "ui.home.rail_search_close");
    let close_label_esc = escape_html(&close_label);
    format!(
        r#"<li class="app-rail-slot-item app-rail-slot-item--search" role="listitem"><div class="app-rail-search-control" id="app-rail-search-control"><button type="button" class="app-rail-slot app-rail-slot--search-create system-icon-btn" id="app-rail-search-create-btn" title="{title}" aria-label="{title}" aria-expanded="false" aria-controls="app-rail-search-input-wrap"><span class="app-rail-slot-frame app-rail-slot-frame--action app-rail-slot-frame--search-action" aria-hidden="true"><span class="app-rail-action-disc"><svg class="app-rail-search-create-icon" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" aria-hidden="true"><use href="{sprite}#icon-rail-search"/></svg></span></span></button><div class="app-rail-search-input-wrap" id="app-rail-search-input-wrap" hidden><div class="app-rail-search-field"><input type="search" id="app-rail-search-input" class="app-rail-search-input search-add-input" autocomplete="off" spellcheck="false" maxlength="32" placeholder="{placeholder}" aria-label="{input_label}" /><button type="button" class="app-rail-search-close-btn system-icon-btn" id="app-rail-search-close-btn" title="{close_label}" aria-label="{close_label}"><svg class="app-rail-search-close-icon" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" aria-hidden="true"><path fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" d="M7.5 7.5 16.5 16.5M16.5 7.5 7.5 16.5"/></svg></button></div></div></div></li>"#,
        title = title_esc,
        placeholder = placeholder_esc,
        input_label = input_label_esc,
        close_label = close_label_esc,
        sprite = SYSTEM_ICONS_SPRITE,
    )
}

fn feed_nav_slot_html(nav: &str, label: &str, icon_id: &str, hidden: bool) -> String {
    let label_esc = escape_html(label);
    let hidden_attr = if hidden { r#" hidden"# } else { "" };
    let disabled_attr = if hidden {
        r#" disabled aria-disabled="true""#
    } else {
        ""
    };
    let item_class = if nav == "first" {
        "app-rail-slot-item--feed-nav-first"
    } else {
        "app-rail-slot-item--feed-nav-other"
    };
    format!(
        r#"<li class="app-rail-slot-item app-rail-slot-item--feed-nav {item_class}" role="listitem"><button type="button" class="app-rail-slot app-rail-feed-nav-btn system-icon-btn" data-feed-nav="{nav}" aria-label="{label}"{hidden_attr}{disabled_attr}><span class="app-rail-slot-frame app-rail-slot-frame--action app-rail-slot-frame--feed-nav-action" aria-hidden="true"><span class="app-rail-action-disc"><svg class="app-rail-feed-nav-icon" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" aria-hidden="true"><use href="{sprite}#{icon_id}"/></svg></span></span></button></li>"#,
        nav = nav,
        label = label_esc,
        hidden_attr = hidden_attr,
        disabled_attr = disabled_attr,
        item_class = item_class,
        icon_id = icon_id,
        sprite = SYSTEM_ICONS_SPRITE,
    )
}

fn feed_nav_pinned_html(loc: &str, c: &Catalog) -> String {
    let first = c.t(loc, "ui.home.feed_nav_first_page");
    let newer = c.t(loc, "ui.home.feed_nav_newer");
    format!(
        "{}{}",
        feed_nav_slot_html("first", &first, "icon-feed-first", true),
        feed_nav_slot_html("prev", &newer, "icon-feed-prev", true),
    )
}

fn feed_nav_next_button_html(loc: &str, c: &Catalog) -> String {
    let older = c.t(loc, "ui.home.feed_nav_older");
    let label_esc = escape_html(&older);
    format!(
        r#"<button type="button" class="app-rail-slot app-rail-feed-nav-btn system-icon-btn" data-feed-nav="next" aria-label="{label}"><span class="app-rail-slot-frame app-rail-slot-frame--action app-rail-slot-frame--feed-nav-action" aria-hidden="true"><span class="app-rail-action-disc"><svg class="app-rail-feed-nav-icon" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" aria-hidden="true"><use href="{sprite}#icon-feed-next"/></svg></span></span></button>"#,
        label = label_esc,
        sprite = SYSTEM_ICONS_SPRITE,
    )
}

fn pinned_slots_html(loc: &str, c: &Catalog) -> String {
    format!("{}{}", feed_nav_pinned_html(loc, c), search_create_slot_html(loc, c))
}

fn connection_slot_html(conn: &ConnectionItem, loc: &str, c: &Catalog) -> String {
    let mut buf = String::with_capacity(380);
    buf.push_str(r#"<li class="app-rail-slot-item" role="listitem">"#);
    let h = conn.peer_handle.as_str();
    let at = format!("@{h}");
    let mut title = at.clone();
    if conn.is_pending {
        title = format!("{at} — {}", c.t(loc, "ui.home.pending_connection"));
    }
    let title_esc = escape_html(&title);
    let aria = c.tf(loc, "ui.home.rail_slot_profile", &[("handle", at.as_str())]);
    buf.push_str(r#"<a class="app-rail-slot app-rail-slot--filled" href="/home/profile/"#);
    buf.push_str(h);
    buf.push_str(r#"" title=""#);
    buf.push_str(&title_esc);
    buf.push_str(r#"" aria-label=""#);
    buf.push_str(&escape_html(&aria));
    buf.push_str(r#"">"#);
    let frame_entity = entity_type_accent::rail_slot_frame_class(conn.peer_type_key.as_str());
    buf.push_str(r#"<span class="brand-frame brand-frame--toolbar app-rail-slot-frame"#);
    buf.push_str(frame_entity);
    buf.push_str(r#"" aria-hidden="true">"#);
    buf.push_str(r#"<span class="avatar-stack">"#);
    let show_img = conn
        .avatar_path
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .is_some();
    if show_img {
        let p = conn.avatar_path.as_deref().unwrap_or_default();
        let src_esc = escape_html(&avatar_url::img_src_from_stored_avatar(p));
        buf.push_str(r#"<img class="app-rail-slot-img" src=""#);
        buf.push_str(&src_esc);
        buf.push_str(r#"" title=""#);
        buf.push_str(&title_esc);
        buf.push_str(r#"" alt="" width="64" height="64" loading="lazy" decoding="async" referrerpolicy="no-referrer" />"#);
    } else {
        buf.push_str(snippets::avatar_not_found_html());
    }
    let show_ssr = matches!(
        conn.peer_type_key.as_str(),
        ref_keys::KEY_ENTITY_TYPE_COMMUNITY
            | ref_keys::KEY_ENTITY_TYPE_NON_PROFIT
            | ref_keys::KEY_ENTITY_TYPE_PROFIT
            | ref_keys::KEY_ENTITY_TYPE_GATHERING
    );
    if show_ssr || conn.is_pending {
        buf.push_str(r#"<span class="avatar-stack-under">"#);
        if show_ssr {
            buf.push_str(&snippets::ssr_stars_html(
                conn.peer_rating,
                "ssr-stars ssr-stars--rail ssr-stars--in-underlay",
            ));
        }
        if conn.is_pending {
            buf.push_str(&format!(
                r#"<span class="app-avatar-pending-badge" title="{}" aria-hidden="true">?</span>"#,
                escape_html(&c.t(loc, "ui.home.pending_connection"))
            ));
        }
        buf.push_str(r#"</span>"#);
    }
    buf.push_str(r#"</span>"#);
    buf.push_str(r#"</span></a>"#);
    buf.push_str("</li>");
    buf
}

fn connection_slots_html(items: &[ConnectionItem], loc: &str, c: &Catalog) -> String {
    let mut buf = String::with_capacity(items.len() * 380);
    for conn in items {
        buf.push_str(&connection_slot_html(conn, loc, c));
    }
    buf
}

/// Secondary nav: feed paging, search/add, scrollable connections, forward page.
#[must_use]
pub fn connections_nav_html(items: &[ConnectionItem], loc: &str, c: &Catalog) -> String {
    let load_aria = escape_html(&c.t(loc, "ui.home.feed_loading_aria"));
    format!(
        r#"<nav class="app-rail" id="app-subnav" aria-label="{nav_label}"><div class="app-rail-pinned"><ul class="app-rail-pinned-slots" role="list">{pinned}</ul></div><div class="app-rail-scroll"><ul class="app-rail-slots" role="list">{connections}</ul></div><div class="app-rail-pinned-end"><div class="app-rail-end-anchor">{next_nav}</div><div class="app-rail-feed-loading" id="app-feed-controls-loading" hidden aria-live="polite" aria-label="{load_aria}"></div></div></nav>"#,
        nav_label = escape_html(&c.t(loc, "ui.home.subnav_label")),
        pinned = pinned_slots_html(loc, c),
        connections = connection_slots_html(items, loc, c),
        next_nav = feed_nav_next_button_html(loc, c),
        load_aria = load_aria,
    )
}
