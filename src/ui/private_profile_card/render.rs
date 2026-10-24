use super::components::{
    cancel_invite_block_html, change_constitution_block_html, community_constitution_block_html,
    community_invite_members_block_html, community_membership_invite_response_html,
    community_profile_amendment_block_html, community_reparent_block_html,
    connect_as_component_html, disconnect_block_html, disconnect_only_html, block_only_html,
    follow_cta_block_html,
    format_joined_date_localized, gathering_rsvp_select_html,
    incoming_invite_response_component_html, leave_community_block_html,
    linked_handles_inline, profile_about_summary_html, profile_entity_edit_block_html,
    profile_recruitment_invite_block_html,
    profile_human_affiliations_block_html, profile_human_contact_block_html, profile_human_edit_block_html, profile_identity_head_html,
    profile_civic_standing_block_html, profile_secondary_actions_html, profile_view_back_cta_html,
    profile_view_full_profile_cta_html, profile_view_profile_cta_html,
    unfollow_cta_block_html,
};
use crate::html_escape::escape_html;
use crate::i18n::Catalog;
use crate::ref_keys;
use crate::social::private_profile::PrivateEntityProfile;
use crate::ui::entity_type_accent;

/// Connect / disconnect / follow / RSVP controls — always shown under the avatar bar.
fn profile_connection_rail_html(p: &PrivateEntityProfile, loc: &str, c: &Catalog) -> String {
    let mut inner = String::new();

    if p.type_label_key == ref_keys::KEY_ENTITY_TYPE_HUMAN {
        append_human_connection_rail(&mut inner, p, loc, c);
    } else if matches!(
        p.type_label_key.as_str(),
        ref_keys::KEY_ENTITY_TYPE_NON_PROFIT
            | ref_keys::KEY_ENTITY_TYPE_PROFIT
            | ref_keys::KEY_ENTITY_TYPE_GATHERING
            | ref_keys::KEY_ENTITY_TYPE_COMMUNITY,
    ) {
        append_brand_style_connection_rail(&mut inner, p, loc, c);
    } else {
        append_generic_follow_rail(&mut inner, p, loc, c);
    }

    if inner.is_empty() {
        return String::new();
    }

    format!(
        r#"<div class="profile-connection-rail" id="profile-connection-rail">{inner}</div>"#,
        inner = inner
    )
}

fn append_human_connection_rail(
    inner: &mut String,
    p: &PrivateEntityProfile,
    loc: &str,
    c: &Catalog,
) {
    if p.can_send_connection_invite {
        let mut options = vec![
            (
                "friend",
                c.t(loc, "ref_values.social_entity_relationship_type.friend"),
            ),
            (
                "family",
                c.t(loc, "ref_values.social_entity_relationship_type.family"),
            ),
            (
                "colleague",
                c.t(loc, "ref_values.social_entity_relationship_type.colleague"),
            ),
        ];
        if p.can_block_human {
            options.push(("block", c.t(loc, "ui.entity_private.connect_block")));
        }
        inner.push_str(&connect_as_component_html(
            &format!("/home/private-profile-connect/{}", p.handle),
            &c.t(loc, "ui.entity_private.connect_as_default"),
            &options,
            false,
            "",
            "",
            "",
        ));
    } else if p.can_respond_to_connection_invite {
        inner.push_str(&incoming_invite_response_component_html(
            &format!("/home/private-profile-connect-action/{}", p.handle),
            &c.t(loc, "ui.entity_private.respond_to_invitation_default"),
            p.can_respond_to_connection_invite && p.viewer_has_connection_capacity,
            &c.t(loc, "ui.entity_private.connect_accept"),
            &c.t(loc, "ui.entity_private.connect_decline"),
            &c.t(loc, "ui.entity_private.connect_block"),
        ));
    } else if p.invite_already_sent {
        let block_action = p
            .can_block_human
            .then(|| format!("/home/private-profile-block/{}", p.handle));
        inner.push_str(&cancel_invite_block_html(
            &format!("/home/private-profile-cancel-invite/{}", p.handle),
            &c.t(loc, "ui.entity_private.connection_invitation_sent"),
            &c.t(loc, "ui.entity_private.cancel_invitation_cta"),
            block_action.as_deref(),
            &c.t(loc, "ui.entity_private.connect_block"),
        ));
    } else if p.can_disconnect {
        let block_action = p
            .can_block_human
            .then(|| format!("/home/private-profile-block/{}", p.handle));
        inner.push_str(&disconnect_block_html(
            &format!("/home/private-profile-disconnect/{}", p.handle),
            &c.t(loc, "ui.entity_private.disconnect_cta"),
            block_action.as_deref(),
            &c.t(loc, "ui.entity_private.connect_block"),
        ));
    }
}

fn append_community_connection_rail(
    inner: &mut String,
    p: &PrivateEntityProfile,
    loc: &str,
    c: &Catalog,
) {
    let member_label = c.t(loc, "ui.entity_private.community_connect_member_option");
    let connect_default = c.t(loc, "ui.entity_private.connect_as_default");
    if p.can_request_community_membership || p.can_apply_for_membership_review {
        let is_reviewed = p.community_join_policy_key.as_deref()
            == Some(ref_keys::KEY_JOIN_POLICY_REVIEWED);
        inner.push_str(&connect_as_component_html(
            &format!("/home/private-profile-connect/{}", p.handle),
            &connect_default,
            &[("member", member_label)],
            is_reviewed,
            &c.t(loc, "ui.entity_private.membership_review_cost_free"),
            &c.t(loc, "ui.entity_private.membership_review_statement_label"),
            &c.t(loc, "ui.entity_private.membership_review_statement_placeholder"),
        ));
    } else if p.has_open_membership_review_application {
        inner.push_str(&cancel_invite_block_html(
            &format!("/home/private-profile-cancel-invite/{}", p.handle),
            &c.t(loc, "ui.entity_private.open_membership_review_notice"),
            &c.t(loc, "ui.entity_private.cancel_membership_review_cta"),
            None,
            "",
        ));
    } else if p.invite_already_sent
        && p.community_join_policy_key.as_deref() == Some(ref_keys::KEY_JOIN_POLICY_OPEN)
    {
        inner.push_str(&cancel_invite_block_html(
            &format!("/home/private-profile-cancel-invite/{}", p.handle),
            &c.t(loc, "ui.entity_private.connection_invitation_sent"),
            &c.t(loc, "ui.entity_private.cancel_invitation_cta"),
            None,
            "",
        ));
    } else if let Some(outcome) = p.membership_review_application_outcome.as_deref() {
        let notice = match outcome {
            "denied" => c.t(loc, "ui.entity_private.membership_review_denied_notice"),
            "no_jury" => c.t(loc, "ui.entity_private.membership_review_no_jury_notice"),
            _ => String::new(),
        };
        if !notice.is_empty() {
            inner.push_str(r#"<section class="profile-block">"#);
            inner.push_str(&format!(
                r#"<p class="profile-value">{}</p>"#,
                escape_html(&notice)
            ));
            inner.push_str("</section>");
        }
    }

    if p.can_respond_to_community_invite {
        inner.push_str(&community_membership_invite_response_html(
            &format!("/home/private-profile-community-invite-action/{}", p.handle),
            &c.t(loc, "ui.entity_private.community_invite_respond_prompt"),
            &c.t(loc, "ui.entity_private.connect_accept"),
            &c.t(loc, "ui.entity_private.connect_decline"),
        ));
    }

    if p.can_leave_community {
        inner.push_str(&leave_community_block_html(
            &format!("/home/private-profile-leave-community/{}", p.handle),
            &c.t(loc, "ui.entity_private.leave_community_cta"),
        ));
    }
}

fn append_brand_style_connection_rail(
    inner: &mut String,
    p: &PrivateEntityProfile,
    loc: &str,
    c: &Catalog,
) {
    let is_gathering_card = p.type_label_key == ref_keys::KEY_ENTITY_TYPE_GATHERING;
    let is_community_card = p.type_label_key == ref_keys::KEY_ENTITY_TYPE_COMMUNITY;

    if is_community_card {
        append_community_connection_rail(inner, p, loc, c);
    }

    if is_gathering_card && p.can_use_gathering_rsvp {
        inner.push_str(&gathering_rsvp_select_html(
            &format!("/home/private-profile-gathering-rsvp/{}", p.handle),
            &c.t(loc, "ui.entity_private.respond_as_default"),
            p.gathering_rsvp.as_deref(),
            &c.t(loc, "ref_values.social_entity_relationship_type.event_going"),
            &c.t(loc, "ref_values.social_entity_relationship_type.event_not_going"),
            &c.t(loc, "ref_values.social_entity_relationship_type.event_interested"),
        ));
    }

    if !is_gathering_card && p.can_follow_brand {
        inner.push_str(&follow_cta_block_html(
            &format!("/home/private-profile-follow/{}", p.handle),
            &c.t(loc, "ui.entity_private.follow_cta"),
        ));
    } else if !is_gathering_card && p.is_following_brand {
        inner.push_str(&unfollow_cta_block_html(
            &format!("/home/private-profile-unfollow/{}", p.handle),
            &c.t(loc, "ui.entity_private.unfollow_cta"),
        ));
    }
}

fn append_generic_follow_rail(
    inner: &mut String,
    p: &PrivateEntityProfile,
    loc: &str,
    c: &Catalog,
) {
    if p.can_follow_brand {
        inner.push_str(&follow_cta_block_html(
            &format!("/home/private-profile-follow/{}", p.handle),
            &c.t(loc, "ui.entity_private.follow_cta"),
        ));
    } else if p.is_following_brand {
        inner.push_str(&unfollow_cta_block_html(
            &format!("/home/private-profile-unfollow/{}", p.handle),
            &c.t(loc, "ui.entity_private.unfollow_cta"),
        ));
    }
}

/// Collapsed other-user human card: Disconnect → View profile → Block (always visible).
fn human_summary_connection_stack_html(p: &PrivateEntityProfile, loc: &str, c: &Catalog) -> String {
    let mut inner = String::new();
    if p.can_disconnect {
        inner.push_str(&disconnect_only_html(
            &format!("/home/private-profile-disconnect/{}", p.handle),
            &c.t(loc, "ui.entity_private.disconnect_cta"),
        ));
        inner.push_str(&profile_view_profile_cta_html(loc, c));
        if p.can_block_human {
            inner.push_str(&block_only_html(
                &format!("/home/private-profile-block/{}", p.handle),
                &c.t(loc, "ui.entity_private.connect_block"),
            ));
        }
    } else {
        inner.push_str(&profile_connection_rail_html(p, loc, c));
        inner.push_str(&profile_view_profile_cta_html(loc, c));
    }
    format!(
        r#"<div class="profile-connection-rail profile-connection-rail--summary">{inner}</div>"#,
        inner = inner
    )
}

/// Expanded other-user human card: Disconnect / Block after profile details, before Back.
fn human_expanded_connection_rail_html(p: &PrivateEntityProfile, loc: &str, c: &Catalog) -> String {
    if p.can_disconnect {
        let mut inner = disconnect_only_html(
            &format!("/home/private-profile-disconnect/{}", p.handle),
            &c.t(loc, "ui.entity_private.disconnect_cta"),
        );
        if p.can_block_human {
            inner.push_str(&block_only_html(
                &format!("/home/private-profile-block/{}", p.handle),
                &c.t(loc, "ui.entity_private.connect_block"),
            ));
        }
        format!(
            r#"<div class="profile-connection-rail profile-connection-rail--expanded">{inner}</div>"#,
            inner = inner
        )
    } else {
        let rail = profile_connection_rail_html(p, loc, c);
        if rail.is_empty() {
            String::new()
        } else {
            rail.replace(
                "profile-connection-rail",
                "profile-connection-rail profile-connection-rail--expanded",
            )
        }
    }
}

#[must_use]
pub fn entity_card_html(
    p: &PrivateEntityProfile,
    loc: &str,
    c: &Catalog,
    summary_only: bool,
) -> String {
    let is_human_private = p.type_label_key == ref_keys::KEY_ENTITY_TYPE_HUMAN;
    let type_label = c.t(
        loc,
        &format!(
            "ref_values.{}.{}",
            ref_keys::GROUP_SOCIAL_ENTITY_TYPE,
            p.type_label_key
        ),
    );
    let status_label = p
        .entity_status_key
        .as_ref()
        .map(|k| c.t(loc, &format!("ref_values.status.{k}")));

    let mut body = String::new();

    body.push_str(r#"<div class="public-post-head">"#);
    body.push_str(&profile_identity_head_html(p, loc, c));
    body.push_str(&profile_secondary_actions_html(p, loc, c));
    body.push_str("</div>");

    if let Some(ref edit) = p.human_profile_edit {
        body.push_str(&profile_human_edit_block_html(&p.handle, edit, loc, c));
    }
    if let Some(ref edit) = p.entity_profile_edit {
        body.push_str(&profile_entity_edit_block_html(&p.handle, edit, loc, c));
    }

    let is_brand_style_card = matches!(
        p.type_label_key.as_str(),
        ref_keys::KEY_ENTITY_TYPE_NON_PROFIT
            | ref_keys::KEY_ENTITY_TYPE_PROFIT
            | ref_keys::KEY_ENTITY_TYPE_GATHERING
            | ref_keys::KEY_ENTITY_TYPE_COMMUNITY,
    );
    let is_gathering_card = p.type_label_key == ref_keys::KEY_ENTITY_TYPE_GATHERING;
    let is_community_card = p.type_label_key == ref_keys::KEY_ENTITY_TYPE_COMMUNITY;
    let hide_detail_sections = summary_only || p.entity_profile_edit.is_some();
    let use_detail_sections_wrapper =
        summary_only || p.entity_profile_edit.is_some() || p.is_self_profile;

    if !is_human_private {
        if is_brand_style_card {
            body.push_str(&profile_about_summary_html(p, loc, c));
            body.push_str(&profile_connection_rail_html(p, loc, c));
            if summary_only {
                body.push_str(&profile_view_full_profile_cta_html(loc, c));
            }

            body.push_str(if hide_detail_sections {
                r#"<div class="profile-entity-detail-sections" id="profile-entity-detail-sections" hidden>"#
            } else {
                r#"<div class="profile-entity-detail-sections" id="profile-entity-detail-sections">"#
            });

            let empty_label = c.t(loc, "ui.entity_private.names_empty");

            if is_gathering_card {
                let int_body = p
                    .intention
                    .as_deref()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or(empty_label.as_str());
                body.push_str(r#"<section class="profile-block">"#);
                body.push_str(&format!(
                    r#"<h2>{}</h2><p class="profile-value profile-brand-text">{}</p>"#,
                    escape_html(&c.t(loc, "ui.entity_private.intention_heading")),
                    escape_html(int_body)
                ));
                body.push_str("</section>");
                let loc_body = p
                    .location_display_label
                    .as_deref()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or(empty_label.as_str());
                body.push_str(r#"<section class="profile-block">"#);
                body.push_str(&format!(
                    r#"<h2>{}</h2><p class="profile-value profile-brand-text">{}</p>"#,
                    escape_html(&c.t(loc, "ui.entity_private.location_heading")),
                    escape_html(loc_body)
                ));
                body.push_str("</section>");
            } else {
                let int_body = p
                    .intention
                    .as_deref()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or(empty_label.as_str());
                body.push_str(r#"<section class="profile-block">"#);
                body.push_str(&format!(
                    r#"<h2>{}</h2><p class="profile-value profile-brand-text">{}</p>"#,
                    escape_html(&c.t(loc, "ui.entity_private.intention_heading")),
                    escape_html(int_body)
                ));
                body.push_str("</section>");
            }
            if is_community_card {
                body.push_str(&community_constitution_block_html(
                    &p.effective_constitution,
                    loc,
                    c,
                ));
            }
            if is_community_card && p.community_is_failing {
                body.push_str(&format!(
                    r#"<section class="profile-block profile-notice profile-notice--failing"><p class="profile-value">{}</p></section>"#,
                    escape_html(&c.t(loc, "ui.entity_private.community_failing_notice"))
                ));
            }
            if p.can_send_recruitment_invites {
                body.push_str(&profile_recruitment_invite_block_html(
                    &p.handle,
                    &c.t(loc, "ui.entity_private.recruitment_invite_heading"),
                    &c.t(loc, "ui.entity_private.recruitment_invite_hint"),
                    &c.t(loc, "ui.entity_private.recruitment_invite_placeholder"),
                    &c.t(loc, "ui.entity_private.recruitment_invite_message_placeholder"),
                    &c.t(loc, "ui.home.search_add_invite_send_cta"),
                ));
            }
            if is_community_card && p.can_invite_community_members {
                body.push_str(&community_invite_members_block_html(
                    &p.handle,
                    &c.t(loc, "ui.entity_private.community_invite_search_label"),
                    &c.t(loc, "ui.entity_private.community_invite_search_placeholder"),
                    &c.t(loc, "ui.entity_private.community_invite_send_cta"),
                    &format!("/home/private-profile-community-invite/{}", p.handle),
                ));
            }
            if is_community_card
                && p.can_propose_constitutional_amendment
                && !p.community_is_failing
            {
                body.push_str(&change_constitution_block_html(
                    &p.handle,
                    &c.t(loc, "ui.entity_private.change_constitution_cta"),
                ));
                if p.can_propose_community_reparent {
                    body.push_str(&community_reparent_block_html(
                        &p.handle,
                        &c.t(loc, "ui.entity_private.community_reparent_cta"),
                    ));
                }
                if p.can_propose_community_profile_amendment {
                    body.push_str(&community_profile_amendment_block_html(
                        &p.handle,
                        &c.t(loc, "ui.entity_private.community_profile_amendment_cta"),
                    ));
                }
            }
            body.push_str("</div>");
        } else {
            body.push_str(&profile_connection_rail_html(p, loc, c));
            if summary_only {
                body.push_str(&profile_view_full_profile_cta_html(loc, c));
            }
            body.push_str(if hide_detail_sections {
                r#"<div class="profile-entity-detail-sections" id="profile-entity-detail-sections" hidden>"#
            } else {
                r#"<div class="profile-entity-detail-sections" id="profile-entity-detail-sections">"#
            });
            body.push_str(&format!(
                r#"<p class="sub profile-private-lead">{}</p>"#,
                escape_html(&c.t(loc, "ui.entity_private.subtitle"))
            ));
            body.push_str(&format!(
                r#"<p class="sub profile-private-note">{}</p>"#,
                escape_html(&c.t(loc, "ui.entity_private.sensitive_note"))
            ));
            body.push_str(r#"<section class="profile-block">"#);
            body.push_str(&format!(
                r#"<h2>{}</h2><p class="profile-value">{}</p>"#,
                escape_html(&c.t(loc, "ui.entity_private.type_heading")),
                escape_html(&type_label)
            ));
            body.push_str("</section>");
            if let Some(ref loc_label) = p.location_display_label.as_ref() {
                body.push_str(r#"<section class="profile-block">"#);
                body.push_str(&format!(
                    r#"<h2>{}</h2><p class="profile-value">{}</p>"#,
                    escape_html(&c.t(loc, "ui.entity_private.location_heading")),
                    escape_html(loc_label)
                ));
                body.push_str("</section>");
            }
            if let Some(ref st) = status_label {
                body.push_str(r#"<section class="profile-block profile-private-sensitive">"#);
                body.push_str(&format!(
                    r#"<h2>{}</h2><p class="profile-value">{}</p>"#,
                    escape_html(&c.t(loc, "ui.entity_private.status_heading")),
                    escape_html(st)
                ));
                body.push_str("</section>");
            }
            body.push_str("</div>");
        }
    } else {
        let summary_other_human = summary_only && !p.is_self_profile;
        if summary_other_human {
            body.push_str(&human_summary_connection_stack_html(p, loc, c));
        } else if !(summary_only && p.is_self_profile) {
            body.push_str(&profile_connection_rail_html(p, loc, c));
            if summary_only {
                body.push_str(&profile_view_profile_cta_html(loc, c));
            }
        } else if summary_only {
            body.push_str(&profile_view_profile_cta_html(loc, c));
        }

        if use_detail_sections_wrapper {
            body.push_str(if hide_detail_sections {
                r#"<div class="profile-entity-detail-sections" id="profile-entity-detail-sections" hidden>"#
            } else {
                r#"<div class="profile-entity-detail-sections" id="profile-entity-detail-sections">"#
            });
        }
        if p.show_human_inactivity_notice {
            let date = p
                .human_estimated_deletion_display
                .as_deref()
                .unwrap_or("—");
            body.push_str(&format!(
                r#"<section class="profile-block profile-notice profile-notice--inactivity"><p class="profile-value">{}</p></section>"#,
                escape_html(&c.tf(
                    loc,
                    "ui.entity_private.human_inactivity_notice",
                    &[("estimated_deletion_date", date)],
                ))
            ));
        }
        if p.show_human_rating_warning {
            body.push_str(&format!(
                r#"<section class="profile-block profile-notice profile-notice--failing"><p class="profile-value">{}</p></section>"#,
                escape_html(&c.t(loc, "ui.entity_private.human_failing_notice"))
            ));
        }
        if let Some(d) = p.joined_on {
            let joined = format_joined_date_localized(loc, d);
            body.push_str(r#"<section class="profile-block">"#);
            body.push_str(&format!(
                r#"<p class="public-post-published-row"><span class="public-post-published-at">{}</span></p>"#,
                escape_html(&c.tf(loc, "ui.entity_private.member_since", &[("date", &joined)]))
            ));
            body.push_str("</section>");
        }

        body.push_str(&profile_human_contact_block_html(p, loc, c));
        body.push_str(&profile_human_affiliations_block_html(p, loc, c));
        body.push_str(&profile_civic_standing_block_html(p, loc, c));

        let mut brands = p.admired_non_profit_handles.clone();
        brands.extend(p.admired_profit_handles.iter().cloned());
        brands.sort();
        brands.dedup();
        let brand_links = linked_handles_inline(&brands);
        if !brand_links.is_empty() {
            body.push_str(r#"<section class="profile-block">"#);
            body.push_str(&format!(
                r#"<h2 class="profile-block-heading profile-block-heading--brands">{}</h2><p class="profile-value profile-handle-list">{}</p>"#,
                escape_html(&c.tf(
                    loc,
                    "ui.entity_private.brands_heading",
                    &[("count", &brands.len().to_string())]
                )),
                brand_links
            ));
            body.push_str("</section>");
        }
        if summary_only && summary_other_human {
            body.push_str(&human_expanded_connection_rail_html(p, loc, c));
            body.push_str(&profile_view_back_cta_html(loc, c));
        } else if summary_only {
            body.push_str(&profile_view_back_cta_html(loc, c));
        }
        if use_detail_sections_wrapper {
            body.push_str("</div>");
        }
    }

    let accent = entity_type_accent::profile_card_accent_class(p.type_label_key.as_str());
    let actions_json = serde_json::to_string(&p.allowed_actions()).unwrap_or_else(|_| "[]".to_string());
    let summary_attr = if summary_only {
        r#" data-profile-summary-only="1" data-profile-view="summary""#
    } else {
        ""
    };
    format!(
        r#"<div class="card app-card profile-private-card{accent}" data-card-profile="private" data-profile-handle="{handle}" data-profile-actions='{actions}'{summary_attr}><div class="card-accent" role="presentation"></div>{body}</div>"#,
        accent = accent,
        handle = escape_html(&p.handle),
        actions = escape_html(&actions_json),
        summary_attr = summary_attr,
        body = body
    )
}
