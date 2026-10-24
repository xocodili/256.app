use chrono::Datelike;
use crate::auth::password::MIN_PASSWORD_LEN;
use crate::html_escape::escape_html;
use crate::i18n::Catalog;
use crate::ref_keys;
use crate::social::community_constitution::ConstitutionSection;
use crate::social::human_onboarding::HumanProfileEditSnapshot;
use crate::social::private_profile::PrivateEntityProfile;

pub(super) use super::entity_edit::profile_entity_edit_block_html;
use crate::ui::avatar_url;
use crate::ui::change_password_panel::{self, ChangePasswordPanelOpts};
use crate::ui::countries;
use crate::ui::entity_handle;
use crate::ui::join_policy;
use crate::ui::nom_icon;
use crate::ui::post_icons;
use crate::ui::snippets;
use rust_decimal::Decimal;

pub(super) fn ordinal_day(day: u32) -> String {
    let suffix = match day % 100 {
        11..=13 => "th",
        _ => match day % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{day}{suffix}")
}

pub(super) fn format_joined_date_localized(loc: &str, d: chrono::NaiveDate) -> String {
    crate::ui::datetime_format::format_calendar_date_localized(loc, d)
}

pub(super) fn valid_country_code(raw: &str) -> Option<String> {
    let code = raw.trim().to_ascii_lowercase();
    if code.len() == 2 && code.chars().all(|c| c.is_ascii_alphabetic()) {
        Some(code)
    } else {
        None
    }
}

pub(super) fn linked_handles_inline(handles: &[String]) -> String {
    linked_handles_inline_with_class(handles, "profile-handle-link")
}

fn linked_handles_inline_with_class(handles: &[String], link_class: &str) -> String {
    if handles.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for handle in handles.iter() {
        let h = handle.trim();
        if h.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push_str(", ");
        }
        out.push_str(&format!(
            r#"<a href="/home/profile/{h}" class="{link_class}">@{h}</a>"#,
            h = escape_html(h),
            link_class = escape_html(link_class),
        ));
    }
    out
}

pub(super) fn rich_text_toolbar_html() -> &'static str {
    r#"<div class="create-post-editor-toolbar"><button type="button" class="create-post-editor-btn" data-cmd="bold" aria-label="Bold"><strong>B</strong></button><button type="button" class="create-post-editor-btn" data-cmd="italic" aria-label="Italic"><em>I</em></button><button type="button" class="create-post-editor-btn" data-cmd="underline" aria-label="Underline"><u>U</u></button></div>"#
}

pub(super) fn connect_as_component_html(
    action: &str,
    default_prompt: &str,
    options: &[(&str, String)],
    membership_review: bool,
    membership_review_cost_hint: &str,
    statement_label: &str,
    statement_placeholder: &str,
) -> String {
    let mut opts = format!(
        r#"<option value="">{prompt}</option>"#,
        prompt = escape_html(default_prompt),
    );
    for (value, label) in options {
        opts.push_str(&format!(
            r#"<option value="{v}">{label}</option>"#,
            v = escape_html(value),
            label = escape_html(label)
        ));
    }
    let review_block = if membership_review {
        format!(
            r#"<p class="profile-value profile-membership-review-cost">{cost_hint}</p><label class="field-label" for="profile-membership-review-editor">{statement_label}</label>{toolbar}<motion id="profile-membership-review-editor" class="create-post-editor profile-membership-review-editor" contenteditable="true" role="textbox" aria-multiline="true" data-placeholder="{placeholder}"></motion><input type="hidden" name="application_statement" id="profile-membership-review-statement-input" value="">"#,
            cost_hint = escape_html(membership_review_cost_hint),
            toolbar = rich_text_toolbar_html(),
            statement_label = escape_html(statement_label),
            placeholder = escape_html(statement_placeholder),
        )
        .replace("<motion ", "<div ")
        .replace("</motion>", "</div>")
    } else {
        String::new()
    };
    let review_attr = if membership_review {
        r#" data-membership-review="1""#
    } else {
        ""
    };
    format!(
        r#"<section class="profile-block profile-connect-block" id="profile-connect-block"><form id="profile-connect-form" action="{action}" method="post"{review_attr}><div class="profile-connect-inline profile-connect-inline--solo"><select id="profile-connect-as" name="connect_as" class="field-input profile-connect-select profile-connect-select--solo" aria-label="{aria}" autocomplete="off">{options}</select></div>{review_block}</form></section>"#,
        action = escape_html(action),
        review_attr = review_attr,
        aria = escape_html(default_prompt),
        options = opts,
        review_block = review_block,
    )
}

pub(super) fn incoming_invite_response_component_html(
    action: &str,
    default_prompt: &str,
    include_accept: bool,
    accept: &str,
    decline: &str,
    block: &str,
) -> String {
    let mut opts = format!(
        r#"<option value="">{prompt}</option>"#,
        prompt = escape_html(default_prompt),
    );
    if include_accept {
        opts.push_str(&format!(
            r#"<option value="accept">{accept}</option>"#,
            accept = escape_html(accept),
        ));
    }
    opts.push_str(&format!(
        r#"<option value="decline">{decline}</option><option value="block">{block}</option>"#,
        decline = escape_html(decline),
        block = escape_html(block),
    ));
    format!(
        r#"<section class="profile-block profile-connect-block" id="profile-connect-block"><form id="profile-invite-response-form" action="{action}" method="post"><div class="profile-connect-inline profile-connect-inline--solo"><select id="profile-invite-response" name="response_action" class="field-input profile-connect-select profile-connect-select--solo" aria-label="{aria}" autocomplete="off">{options}</select></div></form></section>"#,
        action = escape_html(action),
        aria = escape_html(default_prompt),
        options = opts,
    )
}

pub(super) fn disconnect_block_html(action: &str, cta: &str, block_action: Option<&str>, block_cta: &str) -> String {
    format!(
        "{}{}",
        disconnect_only_html(action, cta),
        block_action
            .map(|block_action| block_only_html(block_action, block_cta))
            .unwrap_or_default(),
    )
}

pub(super) fn disconnect_only_html(action: &str, cta: &str) -> String {
    format!(
        r#"<section class="profile-block profile-connect-block" id="profile-connect-block"><form id="profile-disconnect-form" action="{action}" method="post" class="profile-connect-cta-row"><button type="submit" class="auth-text-btn btn-cta profile-connect-cta-btn">{cta}</button></form></section>"#,
        action = escape_html(action),
        cta = escape_html(cta),
    )
}

pub(super) fn block_only_html(action: &str, block_cta: &str) -> String {
    format!(
        r#"<section class="profile-block profile-connect-block profile-connect-block--block" id="profile-block-connect-block"><form id="profile-block-form" action="{action}" method="post" class="profile-connect-cta-row"><button type="submit" class="auth-text-btn profile-account-delete-btn profile-connect-cta-btn">{block_cta}</button></form></section>"#,
        action = escape_html(action),
        block_cta = escape_html(block_cta),
    )
}

pub(super) fn cancel_invite_block_html(
    action: &str,
    notice: &str,
    cta: &str,
    block_action: Option<&str>,
    block_cta: &str,
) -> String {
    let block_row = block_action
        .map(|block_action| {
            format!(
                r#"<form id="profile-block-form" action="{block_action}" method="post" class="profile-connect-cta-row"><button type="submit" class="auth-text-btn profile-account-delete-btn profile-connect-cta-btn">{block_cta}</button></form>"#,
                block_action = escape_html(block_action),
                block_cta = escape_html(block_cta),
            )
        })
        .unwrap_or_default();
    format!(
        r#"<section class="profile-block profile-connect-block" id="profile-connect-block"><p class="profile-value" id="profile-connect-sent-notice">{notice}</p><form id="profile-cancel-invite-form" action="{action}" method="post" class="profile-connect-cta-row"><button type="submit" class="auth-text-btn btn-cta profile-connect-cta-btn">{cta}</button></form>{block_row}</section>"#,
        notice = escape_html(notice),
        action = escape_html(action),
        cta = escape_html(cta),
        block_row = block_row,
    )
}

pub(super) fn profile_secondary_actions_html(p: &PrivateEntityProfile, loc: &str, c: &Catalog) -> String {
    let public_url = format!("/public/{}", p.handle);
    let mut parts: Vec<String> = Vec::new();
    let suffix = format!("profile-{}", p.handle);

    if !p.is_self_profile
        && p.type_label_key == ref_keys::KEY_ENTITY_TYPE_HUMAN
        && p.can_disconnect
    {
        let msg_label = escape_html(&c.t(loc, "ui.entity_private.private_message_aria"));
        parts.push(format!(
            r#"<button type="button" class="post-action-btn" data-profile-private-message="1" data-profile-handle="{handle}" title="{msg_label}" aria-label="{msg_label}">{icon}</button>"#,
            handle = escape_html(&p.handle),
            msg_label = msg_label,
            icon = post_icons::post_icon_private_message(&suffix),
        ));
    }

    let copy_label = escape_html(&c.t(loc, "ui.post_engagement.copy_public_link_aria"));
    parts.push(format!(
        r#"<button type="button" class="post-action-btn" data-public-link="{public_url}" title="{copy_label}" aria-label="{copy_label}">{icon}</button>"#,
        public_url = escape_html(&public_url),
        copy_label = copy_label,
        icon = post_icons::post_icon_copy_public_link(&suffix),
    ));

    if p.is_self_profile && p.type_label_key == ref_keys::KEY_ENTITY_TYPE_HUMAN {
        let redraw_label = escape_html(&c.tf(
            loc,
            "ui.entity_private.avatar_redraw_aria",
            &[(
                "nom_price",
                &nom_icon::nom_amount_plain(p.avatar_redraw_nom_cost),
            )],
        ));
        parts.push(format!(
            r#"<button type="button" class="post-action-btn" id="profile-avatar-redraw-btn" data-profile-avatar-redraw="1" data-avatar-redraw-cost="{cost}" data-avatar-redraw-balance="{balance}" data-avatar-redraw-can-afford="{can_afford}" title="{label}" aria-label="{label}">{icon}</button>"#,
            cost = p.avatar_redraw_nom_cost,
            balance = p.avatar_redraw_noms_balance,
            can_afford = if p.avatar_redraw_can_afford { "1" } else { "0" },
            label = redraw_label,
            icon = post_icons::post_icon_avatar_redraw(&suffix),
        ));
        if p.human_profile_edit.is_some() {
            let edit_label = escape_html(&c.t(loc, "ui.entity_private.profile_edit_aria"));
            parts.push(format!(
                r#"<button type="button" class="post-action-btn" id="profile-human-edit-toggle" data-profile-edit-toggle="1" title="{label}" aria-label="{label}" aria-controls="profile-human-edit-block" aria-expanded="false">{icon}</button>"#,
                label = edit_label,
                icon = post_icons::post_icon_edit(&suffix),
            ));
        }
        let logout_label = escape_html(&c.t(loc, "ui.entity_private.logout_aria"));
        parts.push(format!(
            r#"<form method="post" action="/auth/logout" class="profile-logout-form" id="profile-logout-action"><button type="submit" class="post-action-btn post-action-btn--logout" title="{label}" aria-label="{label}">{icon}</button></form>"#,
            label = logout_label,
            icon = post_icons::post_icon_logout(&suffix),
        ));
    }
    if p.entity_profile_edit.is_some() {
        let edit_label = escape_html(&c.t(loc, "ui.entity_private.profile_edit_aria"));
        parts.push(format!(
            r#"<button type="button" class="post-action-btn" id="profile-entity-edit-toggle" data-profile-entity-edit-toggle="1" title="{label}" aria-label="{label}" aria-controls="profile-entity-edit-block" aria-expanded="false">{icon}</button>"#,
            label = edit_label,
            icon = post_icons::post_icon_edit(&suffix),
        ));
    }

    format!(
        r#"<div class="public-post-head-actions" role="group" aria-label="{}">{inner}</div>"#,
        escape_html(&c.t(loc, "ui.post_card.secondary_actions_aria")),
        inner = parts.join(""),
    )
}

#[must_use]
pub(super) fn entity_social_metric_line_html(
    p: &PrivateEntityProfile,
    loc: &str,
    c: &Catalog,
) -> String {
    let line = match p.type_label_key.as_str() {
        ref_keys::KEY_ENTITY_TYPE_COMMUNITY => {
            let n = p.verified_member_count.unwrap_or(0).max(0) as u64;
            join_policy::community_members_heading(
                c,
                loc,
                n,
                p.community_join_policy_key.as_deref(),
            )
        }
        ref_keys::KEY_ENTITY_TYPE_GATHERING => c.tf(
            loc,
            "ui.entity_private.gathering_rsvp_summary_line",
            &[
                ("accepted", &p.gathering_rsvp_accepted.to_string()),
                ("not_going", &p.gathering_rsvp_not_going.to_string()),
                ("interested", &p.gathering_rsvp_interested.to_string()),
            ],
        ),
        ref_keys::KEY_ENTITY_TYPE_PROFIT | ref_keys::KEY_ENTITY_TYPE_NON_PROFIT => c.tf(
            loc,
            "ui.entity_private.brand_base_heading",
            &[("count", &p.brand_base_follower_count.to_string())],
        ),
        _ => return String::new(),
    };
    format!(
        r#"<span class="public-post-entity-metric">{}</span>"#,
        escape_html(&line)
    )
}

pub(super) fn profile_about_summary_html(p: &PrivateEntityProfile, loc: &str, c: &Catalog) -> String {
    let empty_label = c.t(loc, "ui.entity_private.names_empty");
    let desc_body = p
        .description
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(empty_label.as_str());
    let expand_label = escape_html(&c.t(loc, "ui.entity_private.constitution_show_more"));
    format!(
        r#"<section class="profile-block profile-about-block"><div class="profile-about-body profile-brand-text" id="profile-about-body">{}</div><button type="button" class="profile-about-expand-btn" data-profile-about-expand="1" aria-controls="profile-about-body" aria-expanded="false" hidden>{expand_label}</button></section>"#,
        escape_html(desc_body),
        expand_label = expand_label,
    )
}

/// Human private profile: optional website link and public location (omitted when unset).
pub(super) fn profile_human_contact_block_html(
    p: &PrivateEntityProfile,
    loc: &str,
    c: &Catalog,
) -> String {
    if p.type_label_key != ref_keys::KEY_ENTITY_TYPE_HUMAN {
        return String::new();
    }
    let website = p
        .website_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let location = p
        .location_display_label
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if website.is_none() && location.is_none() {
        return String::new();
    }

    let mut body = String::new();
    if let Some(url) = website {
        body.push_str(&format!(
            r#"<p class="profile-value"><a href="{href}" class="profile-human-contact-link" target="_blank" rel="noopener noreferrer nofollow">{label}</a></p>"#,
            href = escape_html(url),
            label = escape_html(url),
        ));
    }
    if let Some(loc_label) = location {
        body.push_str(&format!(
            r#"<p class="profile-value profile-human-contact-location">{}</p>"#,
            escape_html(loc_label),
        ));
        if p.is_self_profile {
            let pid_text = p
                .location_google_place_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .unwrap_or_else(|| c.t(loc, "ui.entity_private.google_place_id_not_found"));
            body.push_str(&format!(
                r#"<p class="public-post-published-row profile-human-contact-place-id-row"><span class="public-post-published-at">{label}: {pid}</span></p>"#,
                label = escape_html(&c.t(loc, "ui.entity_private.google_place_id_label")),
                pid = escape_html(&pid_text),
            ));
        }
    }

    format!(
        r#"<section class="profile-block profile-human-contact-block">{body}</section>"#,
        body = body,
    )
}

pub(super) fn profile_view_full_profile_cta_html(loc: &str, c: &Catalog) -> String {
    profile_view_toggle_cta_html(loc, c, ProfileViewToggleCta::ViewProfileTop)
}

pub(super) fn profile_view_profile_cta_html(loc: &str, c: &Catalog) -> String {
    profile_view_toggle_cta_html(loc, c, ProfileViewToggleCta::ViewProfileTop)
}

pub(super) fn profile_view_back_cta_html(loc: &str, c: &Catalog) -> String {
    profile_view_toggle_cta_html(loc, c, ProfileViewToggleCta::BackBottom)
}

pub(super) fn profile_view_summary_profile_cta_html(loc: &str, c: &Catalog) -> String {
    profile_view_back_cta_html(loc, c)
}

enum ProfileViewToggleCta {
    ViewProfileTop,
    BackBottom,
}

fn profile_view_toggle_cta_html(loc: &str, c: &Catalog, cta: ProfileViewToggleCta) -> String {
    let (label_key, row_class, row_hidden, row_modifier) = match cta {
        ProfileViewToggleCta::ViewProfileTop => (
            "ui.entity_private.view_profile_cta",
            "profile-view-full-row--top",
            "",
            " profile-view-toggle-view",
        ),
        ProfileViewToggleCta::BackBottom => (
            "ui.entity_private.back_profile_cta",
            "profile-view-full-row--bottom",
            r#" hidden"#,
            " profile-view-toggle-back",
        ),
    };
    format!(
        r#"<div class="profile-view-full-row {row_class}{row_modifier}"{row_hidden}><button type="button" class="auth-text-btn btn-action profile-view-full-btn" data-profile-view-toggle="1" aria-expanded="false">{label}</button></div>"#,
        row_class = row_class,
        row_modifier = row_modifier,
        row_hidden = row_hidden,
        label = escape_html(&c.t(loc, label_key)),
    )
}

fn profile_affiliation_group_html(heading_key: &str, links: &str, loc: &str, c: &Catalog) -> String {
    if links.is_empty() {
        return String::new();
    }
    format!(
        r#"<div class="profile-civic-standing-row profile-civic-standing-row--communities"><span class="profile-civic-standing-label">{heading}</span></div><div class="profile-affiliation-list">{links}</div>"#,
        heading = escape_html(&c.t(loc, heading_key)),
        links = links,
    )
}

/// Other human profiles: communities, owned profits, non-profits, and events (hidden when all empty).
pub(super) fn profile_human_affiliations_block_html(
    p: &PrivateEntityProfile,
    loc: &str,
    c: &Catalog,
) -> String {
    if p.is_self_profile || p.type_label_key != ref_keys::KEY_ENTITY_TYPE_HUMAN {
        return String::new();
    }

    let link_class = "profile-civic-standing-community-link";
    let community_handles: Vec<String> = p
        .member_communities
        .iter()
        .map(|m| m.handle.clone())
        .collect();
    let community_links =
        linked_handles_inline_with_class(&community_handles, link_class);
    let page_links = linked_handles_inline_with_class(&p.owned_profit_handles, link_class);
    let nonprofit_links =
        linked_handles_inline_with_class(&p.owned_non_profit_handles, link_class);
    let event_links = linked_handles_inline_with_class(&p.owned_gathering_handles, link_class);

    if community_links.is_empty()
        && page_links.is_empty()
        && nonprofit_links.is_empty()
        && event_links.is_empty()
    {
        return String::new();
    }

    let mut groups = String::new();
    groups.push_str(&profile_affiliation_group_html(
        "ui.entity_private.communities_list_label",
        &community_links,
        loc,
        c,
    ));
    groups.push_str(&profile_affiliation_group_html(
        "ui.entity_private.profits_list_label",
        &page_links,
        loc,
        c,
    ));
    groups.push_str(&profile_affiliation_group_html(
        "ui.entity_private.non_profits_list_label",
        &nonprofit_links,
        loc,
        c,
    ));
    groups.push_str(&profile_affiliation_group_html(
        "ui.entity_private.events_list_label",
        &event_links,
        loc,
        c,
    ));

    format!(
        r#"<section class="profile-block profile-civic-standing-block profile-human-affiliations-block">{groups}</section>"#,
        groups = groups,
    )
}

/// Owner-only human profile: global + per-community edge standing.
pub(super) fn profile_civic_standing_block_html(
    p: &PrivateEntityProfile,
    loc: &str,
    c: &Catalog,
) -> String {
    if !p.is_self_profile || p.type_label_key != ref_keys::KEY_ENTITY_TYPE_HUMAN {
        return String::new();
    }

    let rating_display = format!("{:.2}", p.entity_rating);
    let global_row = format!(
        r#"<div class="profile-civic-standing-row profile-civic-standing-row--global"><span class="profile-civic-standing-label">{}</span><span class="profile-civic-standing-value"><span class="profile-civic-standing-numeric mono">{rating}</span></span></div>"#,
        escape_html(&c.t(loc, "ui.entity_private.civic_standing_global_label")),
        rating = escape_html(&rating_display),
    );

    let mut community_rows = String::new();
    for community in &p.member_communities {
        let handle = community.handle.trim();
        if handle.is_empty() {
            continue;
        }
        let edge_display = format!("{:.2}", community.edge_rating);
        community_rows.push_str(&format!(
            r#"<div class="profile-civic-standing-row"><a href="/home/profile/{handle}" class="profile-handle-link profile-civic-standing-community-link">@{handle}</a><span class="profile-civic-standing-value"><span class="profile-civic-standing-numeric mono">{edge}</span></span></div>"#,
            handle = escape_html(handle),
            edge = escape_html(&edge_display),
        ));
    }

    let communities_heading = if !community_rows.is_empty() {
        format!(
            r#"<div class="profile-civic-standing-row profile-civic-standing-row--communities"><span class="profile-civic-standing-label">{}</span></div>"#,
            escape_html(&c.t(loc, "ui.entity_private.civic_standing_communities_label")),
        )
    } else {
        String::new()
    };

    format!(
        r#"<section class="profile-block profile-civic-standing-block">{global_row}<p class="profile-value profile-civic-standing-hint">{hint}</p>{communities_heading}{community_rows}<p class="profile-civic-standing-learn"><a href="/login?legal=how#civic-standing" class="profile-civic-standing-learn-link">{learn}</a></p></section>"#,
        global_row = global_row,
        hint = escape_html(&c.t(loc, "ui.entity_private.civic_standing_global_hint")),
        communities_heading = communities_heading,
        community_rows = community_rows,
        learn = escape_html(&c.t(loc, "ui.entity_private.civic_standing_learn_more")),
    )
}

pub(super) fn profile_identity_head_html(
    p: &PrivateEntityProfile,
    loc: &str,
    c: &Catalog,
) -> String {
    let handle = p.handle.as_str();
    let at = format!("@{handle}");
    let avatar_inner = if let Some(path) = p.avatar_path.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let src = avatar_url::img_src_from_stored_avatar(path);
        format!(
            r#"<img class="public-post-avatar-img" src="{}" alt="{}" width="64" height="64" loading="lazy" decoding="async" referrerpolicy="no-referrer" />"#,
            escape_html(&src),
            escape_html(&at)
        )
    } else {
        snippets::avatar_not_found_html().to_string()
    };
    let avatar_stars_html = if p.type_label_key == ref_keys::KEY_ENTITY_TYPE_NON_PROFIT
        || p.type_label_key == ref_keys::KEY_ENTITY_TYPE_PROFIT
        || p.show_entity_rating
    {
        snippets::ssr_stars_html(p.entity_rating, "ssr-stars ssr-stars--avatar")
    } else {
        String::new()
    };
    let flag_html = p
        .country_code
        .as_deref()
        .and_then(valid_country_code)
        .map(|cc| {
            format!(
                r#"<span class="public-post-flag-icon public-post-flag-inline fi fi-{}" data-country-code="{}" title="Profile country {}" aria-hidden="true"></span>"#,
                escape_html(&cc),
                escape_html(&cc.to_ascii_uppercase()),
                escape_html(&cc.to_ascii_uppercase())
            )
        })
        .unwrap_or_default();
    let rating = Decimal::from_f32_retain(p.entity_rating).unwrap_or(Decimal::ZERO);
    let death_immune = crate::social::community_graph::is_death_immune_handle(handle);
    let reveal_failing_standing =
        p.is_self_profile && p.type_label_key == ref_keys::KEY_ENTITY_TYPE_HUMAN;
    let handle_line = entity_handle::entity_handle_line_for_type(
        &escape_html(&at),
        rating,
        p.entity_status_key.as_deref(),
        Some(p.type_label_key.as_str()),
        death_immune,
        reveal_failing_standing,
    );
    let handle_block = entity_handle::feed_entity_handle_block_html(
        &escape_html(handle),
        &handle_line,
        false,
    );
    let display_name = p
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(escape_html)
        .unwrap_or_default();
    let metric_line = entity_social_metric_line_html(p, loc, c);
    format!(
        r#"<div class="public-post-head-identity"><span class="public-post-avatar-link"><span class="brand-frame brand-frame--toolbar public-post-avatar"><span class="avatar-stack">{avatar_inner}{avatar_stars_html}</span></span></span><div class="public-post-handle-block">{handle_block}<span class="public-post-name-row"><span class="public-post-display-name">{display_name}</span>{flag_html}</span>{metric_line}</div></div>"#,
        avatar_inner = avatar_inner,
        avatar_stars_html = avatar_stars_html,
        handle_block = handle_block,
        display_name = display_name,
        flag_html = flag_html,
        metric_line = metric_line,
    )
}

pub(super) fn profile_human_edit_block_html(
    handle: &str,
    edit: &HumanProfileEditSnapshot,
    loc: &str,
    c: &Catalog,
) -> String {
    let country_rows = countries::option_rows_html_with_selected(Some(&edit.country_code));
    let birth_display = edit.birth_date.format("%Y-%m-%d").to_string();
    let required_abbr = escape_html(&c.t(loc, "ui.home_profile.required_abbr"));
    let email_row = edit
        .user_email
        .as_deref()
        .map(|email| {
            format!(
                r#"<label class="profile-field-label profile-field-label--readonly" for="profile-edit-email"><span class="profile-field-label__text">{}</span></label><input id="profile-edit-email" type="email" value="{}" readonly autocomplete="email">"#,
                escape_html(&c.t(loc, "ui.entity_private.profile_edit_email_label")),
                escape_html(email),
            )
        })
        .unwrap_or_default();
    format!(
        r#"<section class="profile-block profile-human-edit-block" id="profile-human-edit-block" data-password-min-len="{password_min}" hidden>
  <div id="profile-human-edit-panel">
    <h2 class="profile-block-heading">{heading}</h2>
    <form id="profile-human-edit-form" class="profile-completion-form profile-human-edit-form" autocomplete="on" novalidate>
      <label class="profile-field-label profile-field-label--readonly" for="profile-edit-handle"><span class="profile-field-label__text">{handle_label}</span></label>
      <input id="profile-edit-handle" type="text" value="{handle_at}" readonly autocomplete="username">
      <label class="profile-field-label profile-field-label--required" for="profile-edit-given"><span class="profile-field-label__text">{given_label}</span><abbr class="profile-req-mark" title="{required_abbr}">*</abbr></label>
      <input id="profile-edit-given" name="given_name" type="text" required autocomplete="given-name" maxlength="128" value="{given}">
      <label class="profile-field-label profile-field-label--required" for="profile-edit-family"><span class="profile-field-label__text">{family_label}</span><abbr class="profile-req-mark" title="{required_abbr}">*</abbr></label>
      <input id="profile-edit-family" name="family_name" type="text" required autocomplete="family-name" maxlength="128" value="{family}">
      <label class="profile-field-label profile-field-label--optional" for="profile-edit-additional"><span class="profile-field-label__text">{additional_label}</span></label>
      <input id="profile-edit-additional" name="additional_name" type="text" autocomplete="additional-name" maxlength="256" value="{additional}">
      <label class="profile-field-label profile-field-label--readonly" for="profile-edit-birth"><span class="profile-field-label__text">{birth_label}</span></label>
      <input id="profile-edit-birth" type="date" value="{birth}" readonly autocomplete="bday">
      {email_row}
      <p class="profile-completion-location-heading">{location_heading}</p>
      <label class="profile-field-label profile-field-label--required" for="profile-edit-country"><span class="profile-field-label__text">{country_label}</span><abbr class="profile-req-mark" title="{required_abbr}">*</abbr></label>
      <select id="profile-edit-country" name="country_code" required autocomplete="country">
        <option value="" disabled>{country_prompt}</option>
        {country_rows}
      </select>
      <label class="profile-field-label profile-field-label--optional" for="profile-edit-region"><span class="profile-field-label__text">{region_label}</span></label>
      <input id="profile-edit-region" name="region" type="text" autocomplete="address-level1" maxlength="128" value="{region}">
      <label class="profile-field-label profile-field-label--optional" for="profile-edit-locality"><span class="profile-field-label__text">{locality_label}</span></label>
      <input id="profile-edit-locality" name="locality" type="text" autocomplete="address-level2" maxlength="128" value="{locality}">
      <label class="profile-field-label profile-field-label--optional" for="profile-edit-public-location"><span class="profile-field-label__text">{public_location_label}</span></label>
      <input id="profile-edit-public-location" name="public_location_label" type="text" maxlength="512" value="{public_location}">
      <p class="hint">{public_location_hint}</p>
      <label class="create-entity-handle-label" for="profile-edit-website">{website_label}</label>
      <input id="profile-edit-website" name="website_url" class="search-add-input" type="url" inputmode="url" autocomplete="url" spellcheck="false" placeholder="{website_ph}" value="{website}">
    </form>
    <div class="profile-human-edit-actions">
      <button type="submit" form="profile-human-edit-form" class="auth-text-btn btn-cta profile-connect-cta-btn" id="profile-human-edit-save">{save_label}</button>
      <button type="button" class="auth-text-btn btn-action profile-connect-cta-btn" id="profile-change-password-open">{change_password_label}</button>
      <button type="button" class="auth-text-btn btn-action profile-connect-cta-btn" id="profile-manage-blocks-open">{manage_blocks_label}</button>
      <button type="button" class="auth-text-btn profile-account-delete-btn profile-connect-cta-btn" id="profile-delete-account-open">{delete_account_label}</button>
      <button type="button" class="auth-text-btn profile-connect-cta-btn" id="profile-human-edit-cancel">{back_label}</button>
    </div>
  </div>
  {change_password_panel}
  <div class="profile-account-panel" id="profile-manage-blocks-block" hidden>
    <h2 class="profile-block-heading">{manage_blocks_heading}</h2>
    <p class="sub">{manage_blocks_lead}</p>
    <ul class="profile-block-list" id="profile-block-list" hidden></ul>
    <p class="profile-value" id="profile-block-list-empty" hidden>{manage_blocks_empty}</p>
    <p class="err" id="profile-block-list-msg" role="alert" hidden></p>
    <div class="profile-human-edit-actions">
      <button type="button" class="auth-text-btn profile-connect-cta-btn" id="profile-manage-blocks-cancel">{back_label}</button>
    </div>
  </div>
  <div class="profile-account-panel profile-delete-account-card" id="profile-delete-account-block" hidden>
    <h2 class="profile-block-heading">{delete_account_heading}</h2>
    <p class="sub">{delete_account_lead}</p>
    <ul class="profile-delete-account-list">
      <li>{delete_account_bullet_connections}</li>
      <li>{delete_account_bullet_cases}</li>
      <li>{delete_account_bullet_posts}</li>
      <li>{delete_account_bullet_login}</li>
    </ul>
    <label class="profile-field-label profile-field-label--required" for="profile-delete-password"><span class="profile-field-label__text">{current_password_label}</span><abbr class="profile-req-mark" title="{required_abbr}">*</abbr></label>
    <input id="profile-delete-password" type="password" autocomplete="current-password" minlength="{password_min}">
    <label class="profile-field-label profile-field-label--required" for="profile-delete-password-confirm"><span class="profile-field-label__text">{confirm_current_password_label}</span><abbr class="profile-req-mark" title="{required_abbr}">*</abbr></label>
    <input id="profile-delete-password-confirm" type="password" autocomplete="current-password" minlength="{password_min}">
    <label class="profile-delete-account-ack" for="profile-delete-account-ack">
      <input id="profile-delete-account-ack" type="checkbox" name="delete_ack">
      <span class="profile-delete-account-ack-text">{delete_account_ack_label}</span>
    </label>
    <div class="profile-human-edit-actions">
      <button type="button" class="auth-text-btn btn-cta profile-account-delete-btn profile-connect-cta-btn" id="profile-delete-account-submit">{delete_account_confirm_label}</button>
      <button type="button" class="auth-text-btn profile-connect-cta-btn" id="profile-delete-account-cancel">{back_label}</button>
    </div>
    <p class="err" id="profile-delete-account-msg" role="alert" hidden></p>
  </div>
  <p class="err" id="profile-human-edit-msg" role="alert" hidden></p>
</section>"#,
        password_min = MIN_PASSWORD_LEN,
        heading = escape_html(&c.t(loc, "ui.entity_private.profile_edit_heading")),
        handle_label = escape_html(&c.t(loc, "ui.home_profile.handle_label")),
        handle_at = escape_html(&format!("@{handle}")),
        given_label = escape_html(&c.t(loc, "ui.home_profile.given_label")),
        given = escape_html(&edit.given_name),
        family_label = escape_html(&c.t(loc, "ui.home_profile.family_label")),
        family = escape_html(&edit.family_name),
        additional_label = escape_html(&c.t(loc, "ui.home_profile.additional_name_label")),
        additional = escape_html(edit.additional_name.as_deref().unwrap_or("")),
        birth_label = escape_html(&c.t(loc, "ui.home_profile.birth_date_label")),
        birth = escape_html(&birth_display),
        email_row = email_row,
        location_heading = escape_html(&c.t(loc, "ui.home_profile.location_section")),
        country_label = escape_html(&c.t(loc, "ui.home_profile.country_label")),
        required_abbr = required_abbr,
        country_prompt = escape_html(&c.t(loc, "ui.home_profile.country_select_prompt")),
        country_rows = country_rows,
        region_label = escape_html(&c.t(loc, "ui.home_profile.region_label")),
        region = escape_html(edit.region.as_deref().unwrap_or("")),
        locality_label = escape_html(&c.t(loc, "ui.home_profile.locality_label")),
        locality = escape_html(edit.locality.as_deref().unwrap_or("")),
        public_location_label = escape_html(&c.t(loc, "ui.home_profile.public_location_label")),
        public_location = escape_html(&edit.public_location_label),
        public_location_hint = escape_html(&c.t(loc, "ui.home_profile.public_location_hint")),
        website_label = escape_html(&c.t(loc, "ui.home.create_entity_website_label")),
        website_ph = escape_html(&c.t(loc, "ui.home.create_entity_website_placeholder")),
        website = escape_html(edit.website_url.as_deref().unwrap_or("")),
        save_label = escape_html(&c.t(loc, "ui.entity_private.profile_edit_save")),
        change_password_label = escape_html(&c.t(loc, "ui.entity_private.change_password_open")),
        manage_blocks_label = escape_html(&c.t(loc, "ui.entity_private.manage_blocks_open")),
        delete_account_label = escape_html(&c.t(loc, "ui.entity_private.delete_account_open")),
        back_label = escape_html(&c.t(loc, "ui.entity_private.back_profile_cta")),
        change_password_panel = change_password_panel::change_password_panel_html(
            loc,
            c,
            ChangePasswordPanelOpts::default(),
        ),
        manage_blocks_heading = escape_html(&c.t(loc, "ui.entity_private.manage_blocks_heading")),
        manage_blocks_lead = escape_html(&c.t(loc, "ui.entity_private.manage_blocks_lead")),
        manage_blocks_empty = escape_html(&c.t(loc, "ui.entity_private.manage_blocks_empty")),
        delete_account_heading = escape_html(&c.t(loc, "ui.entity_private.delete_account_heading")),
        delete_account_lead = escape_html(&c.t(loc, "ui.entity_private.delete_account_lead")),
        current_password_label = escape_html(&c.t(loc, "ui.entity_private.account_current_password_label")),
        confirm_current_password_label = escape_html(&c.t(loc, "ui.entity_private.account_confirm_password_label")),
        delete_account_bullet_connections = escape_html(&c.t(loc, "ui.entity_private.delete_account_bullet_connections")),
        delete_account_bullet_cases = escape_html(&c.t(loc, "ui.entity_private.delete_account_bullet_cases")),
        delete_account_bullet_posts = escape_html(&c.t(loc, "ui.entity_private.delete_account_bullet_posts")),
        delete_account_bullet_login = escape_html(&c.t(loc, "ui.entity_private.delete_account_bullet_login")),
        delete_account_ack_label = escape_html(&c.t(loc, "ui.entity_private.delete_account_ack")),
        delete_account_confirm_label = escape_html(&c.t(loc, "ui.entity_private.delete_account_confirm")),
    )
}

#[must_use]
pub fn community_constitution_block_html(
    sections: &[ConstitutionSection],
    loc: &str,
    c: &Catalog,
) -> String {
    let empty_label = c.t(loc, "ui.entity_private.constitution_empty");
    let tier_from = c.t(loc, "ui.entity_private.constitution_tier_from");
    let show_tier_labels = sections.len() > 1;
    let mut body =
        String::from(r#"<div class="profile-constitution-body" id="profile-constitution-body">"#);

    if sections.is_empty() {
        body.push_str(&format!(
            r#"<p class="profile-value profile-brand-text profile-constitution-empty">{}</p>"#,
            escape_html(&empty_label)
        ));
    } else {
        for (idx, section) in sections.iter().enumerate() {
            if show_tier_labels && idx > 0 {
                body.push_str(&format!(
                    r#"<h3 class="profile-constitution-tier">{} <span class="profile-constitution-tier-handle">@{}</span></h3>"#,
                    escape_html(&tier_from),
                    escape_html(&section.community_handle),
                ));
            }
            body.push_str(r#"<ol class="profile-constitution-list">"#);
            for clause in &section.clauses {
                body.push_str(&format!(
                    r#"<li><span class="profile-constitution-point mono">#{}</span> <span class="profile-constitution-item">{}</span></li>"#,
                    clause.point,
                    escape_html(clause.item.trim()),
                ));
            }
            body.push_str("</ol>");
        }
    }

    body.push_str("</div>");
    let expand_label = escape_html(&c.t(loc, "ui.entity_private.constitution_show_more"));
    format!(
        r#"<section class="profile-block profile-constitution-block"><h2>{}</h2>{body}<button type="button" class="profile-constitution-expand-btn" data-profile-constitution-expand="1" aria-controls="profile-constitution-body" aria-expanded="false" hidden>{expand_label}</button></section>"#,
        escape_html(&c.t(loc, "ui.entity_private.constitution_heading")),
        body = body,
        expand_label = expand_label,
    )
}

pub(super) fn change_constitution_block_html(handle: &str, cta: &str) -> String {
    format!(
        r#"<section class="profile-block profile-connect-block" id="profile-change-constitution-block"><div class="profile-connect-cta-row"><a class="auth-text-btn btn-cta profile-connect-cta-btn" href="/home/judicial/community/{handle}">{cta}</a></div></section>"#,
        handle = escape_html(handle),
        cta = escape_html(cta),
    )
}

pub(super) fn community_reparent_block_html(handle: &str, cta: &str) -> String {
    format!(
        r#"<section class="profile-block profile-connect-block" id="profile-reparent-block"><div class="profile-connect-cta-row"><a class="auth-text-btn btn-cta profile-connect-cta-btn" href="/home/community/{handle}/reparent">{cta}</a></div></section>"#,
        handle = escape_html(handle),
        cta = escape_html(cta),
    )
}

pub(super) fn community_profile_amendment_block_html(handle: &str, cta: &str) -> String {
    format!(
        r#"<section class="profile-block profile-connect-block" id="profile-change-community-profile-block"><div class="profile-connect-cta-row"><a class="auth-text-btn btn-cta profile-connect-cta-btn" href="/home/judicial/community/{handle}/profile">{cta}</a></div></section>"#,
        handle = escape_html(handle),
        cta = escape_html(cta),
    )
}

pub(super) fn leave_community_block_html(action: &str, cta: &str) -> String {
    format!(
        r#"<section class="profile-block profile-connect-block" id="profile-leave-community-block"><form id="profile-leave-community-form" action="{action}" method="post" class="profile-connect-cta-row"><button type="submit" class="auth-text-btn btn-cta profile-connect-cta-btn">{cta}</button></form></section>"#,
        action = escape_html(action),
        cta = escape_html(cta),
    )
}

pub(super) fn community_membership_invite_response_html(
    action: &str,
    default_prompt: &str,
    accept: &str,
    decline: &str,
) -> String {
    format!(
        r#"<section class="profile-block profile-connect-block" id="profile-connect-block"><form id="profile-community-invite-response-form" action="{action}" method="post"><div class="profile-connect-inline profile-connect-inline--solo"><select id="profile-community-invite-response" name="response_action" class="field-input profile-connect-select profile-connect-select--solo" aria-label="{aria}" autocomplete="off"><option value="">{prompt}</option><option value="accept">{accept}</option><option value="decline">{decline}</option></select></div></form></section>"#,
        action = escape_html(action),
        aria = escape_html(default_prompt),
        prompt = escape_html(default_prompt),
        accept = escape_html(accept),
        decline = escape_html(decline),
    )
}

pub(super) fn profile_recruitment_invite_block_html(
    entity_handle: &str,
    heading: &str,
    hint: &str,
    placeholder: &str,
    message_placeholder: &str,
    send_cta: &str,
) -> String {
    format!(
        r#"<section class="profile-block profile-recruitment-invite-block" id="profile-recruitment-invite-block" data-entity-handle="{handle}">
  <h2 class="profile-block-heading">{heading}</h2>
  <p class="hint profile-recruitment-invite-hint">{hint}</p>
  <label for="profile-recruitment-invite-emails" class="visually-hidden">{placeholder}</label>
  <textarea id="profile-recruitment-invite-emails" class="field-input profile-recruitment-invite-textarea" rows="4" placeholder="{placeholder}" spellcheck="false" autocomplete="off"></textarea>
  <label for="profile-recruitment-invite-message" class="visually-hidden">{message_placeholder}</label>
  <textarea id="profile-recruitment-invite-message" class="field-input profile-recruitment-invite-message" rows="3" placeholder="{message_placeholder}" maxlength="500"></textarea>
  <p class="hint create-post-submit-status" id="profile-recruitment-invite-status" hidden aria-live="polite"></p>
  <button type="button" class="auth-text-btn btn-cta profile-recruitment-invite-send" id="profile-recruitment-invite-send" disabled>{send_cta}</button>
</section>"#,
        handle = escape_html(entity_handle),
        heading = escape_html(heading),
        hint = escape_html(hint),
        placeholder = escape_html(placeholder),
        message_placeholder = escape_html(message_placeholder),
        send_cta = escape_html(send_cta),
    )
}

pub(super) fn community_invite_members_block_html(
    community_handle: &str,
    search_label: &str,
    search_placeholder: &str,
    send_cta: &str,
    action: &str,
) -> String {
    format!(
        r#"<section class="profile-block profile-community-invite-block" id="profile-community-invite-block" data-community-handle="{handle}">
  <label for="profile-community-invite-search" class="create-entity-handle-label">{search_label}</label>
  <input type="hidden" id="profile-community-invite-handle" name="invitee_handle" value="" />
  <input id="profile-community-invite-search" class="search-add-input" type="text" autocomplete="off" spellcheck="false" placeholder="{search_placeholder}" aria-autocomplete="list" aria-controls="profile-community-invite-results" />
  <p id="profile-community-invite-search-status" class="hint create-post-submit-status" hidden aria-live="polite"></p>
  <ul id="profile-community-invite-results" class="search-add-results profile-community-invite-results" aria-live="polite"></ul>
  <form id="profile-community-invite-form" action="{action}" method="post" class="profile-connect-cta-row">
    <button type="submit" class="auth-text-btn btn-cta profile-connect-cta-btn" id="profile-community-invite-submit" disabled>{send_cta}</button>
  </form>
</section>"#,
        handle = escape_html(community_handle),
        search_label = escape_html(search_label),
        search_placeholder = escape_html(search_placeholder),
        send_cta = escape_html(send_cta),
        action = escape_html(action),
    )
}

pub(super) fn follow_cta_block_html(action: &str, cta: &str) -> String {
    format!(
        r#"<section class="profile-block profile-connect-block" id="profile-entity-follow-block"><form id="profile-follow-form" action="{action}" method="post" class="profile-connect-cta-row"><button type="submit" class="auth-text-btn btn-cta profile-connect-cta-btn">{cta}</button></form></section>"#,
        action = escape_html(action),
        cta = escape_html(cta),
    )
}

pub(super) fn unfollow_cta_block_html(action: &str, cta: &str) -> String {
    format!(
        r#"<section class="profile-block profile-connect-block" id="profile-entity-follow-block"><form id="profile-unfollow-form" action="{action}" method="post" class="profile-connect-cta-row"><button type="submit" class="auth-text-btn btn-cta profile-connect-cta-btn">{cta}</button></form></section>"#,
        action = escape_html(action),
        cta = escape_html(cta),
    )
}

pub(super) fn gathering_rsvp_select_html(
    action: &str,
    default_prompt: &str,
    current: Option<&str>,
    going: &str,
    not_going: &str,
    interested: &str,
) -> String {
    let cur = current.map(|s| s.trim()).filter(|s| !s.is_empty());
    let sel_empty = if cur.is_none() { " selected" } else { "" };
    let sel_going = if cur.as_deref() == Some("going") {
        " selected"
    } else {
        ""
    };
    let sel_not = if cur.as_deref() == Some("not_going") {
        " selected"
    } else {
        ""
    };
    let sel_int = if cur.as_deref() == Some("interested") {
        " selected"
    } else {
        ""
    };
    format!(
        r#"<section class="profile-block profile-connect-block" id="profile-gathering-rsvp-block"><form id="profile-gathering-rsvp-form" action="{action}" method="post"><div class="profile-connect-inline profile-connect-inline--solo"><select id="profile-gathering-rsvp" name="gathering_rsvp" class="field-input profile-connect-select profile-connect-select--solo" aria-label="{aria}" autocomplete="off"><option value=""{sel_empty}>{default_prompt}</option><option value="going"{sel_going}>{going}</option><option value="not_going"{sel_not}>{not_going}</option><option value="interested"{sel_int}>{interested}</option></select></div></form></section>"#,
        action = escape_html(action),
        aria = escape_html(default_prompt),
        default_prompt = escape_html(default_prompt),
        going = escape_html(going),
        not_going = escape_html(not_going),
        interested = escape_html(interested),
        sel_empty = sel_empty,
        sel_going = sel_going,
        sel_not = sel_not,
        sel_int = sel_int,
    )
}
