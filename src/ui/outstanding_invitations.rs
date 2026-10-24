//! Home feed cards for pending social invitations (same visual language as jury summons).

use rust_decimal::Decimal;

use crate::html_escape::escape_html;
use crate::i18n::Catalog;
use crate::ref_keys;
use crate::social::community_graph;
use crate::social::pending_invitations::{PendingInvitation, PendingInvitationKind};
use crate::ui::avatar_url;
use crate::ui::entity_handle;
use crate::ui::entity_type_accent;
use crate::ui::snippets;

fn valid_country_code(raw: &str) -> Option<String> {
    let code = raw.trim().to_ascii_lowercase();
    if code.len() == 2 && code.chars().all(|c| c.is_ascii_alphabetic()) {
        Some(code)
    } else {
        None
    }
}

fn counterparty_head_html(inv: &PendingInvitation) -> String {
    let handle = inv.counterparty_handle.as_str();
    let at = format!("@{handle}");
    let avatar_inner = if let Some(path) = inv
        .counterparty_avatar_path
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        let src = avatar_url::img_src_from_stored_avatar(path);
        format!(
            r#"<img class="public-post-avatar-img" src="{}" alt="{}" width="64" height="64" loading="lazy" decoding="async" referrerpolicy="no-referrer" />"#,
            escape_html(&src),
            escape_html(&at)
        )
    } else {
        snippets::avatar_not_found_html().to_string()
    };
    let show_ssr = matches!(
        inv.counterparty_type_key.as_str(),
        ref_keys::KEY_ENTITY_TYPE_NON_PROFIT
            | ref_keys::KEY_ENTITY_TYPE_PROFIT
            | ref_keys::KEY_ENTITY_TYPE_COMMUNITY
            | ref_keys::KEY_ENTITY_TYPE_GATHERING
    );
    let avatar_stars = if show_ssr {
        snippets::ssr_stars_html(
            inv.counterparty_rating,
            "ssr-stars ssr-stars--avatar",
        )
    } else {
        String::new()
    };
    let flag_html = inv
        .counterparty_country_code
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
    let rating = Decimal::from_f32_retain(inv.counterparty_rating)
        .unwrap_or(Decimal::ZERO);
    let death_immune = community_graph::is_death_immune_handle(handle);
    let reveal_failing_standing = false;
    let handle_kind = entity_handle::entity_handle_kind(Some(inv.counterparty_type_key.as_str()));
    let raw_tone = entity_handle::entity_handle_tone(
        rating,
        inv.counterparty_status_key.as_deref(),
        death_immune,
    );
    let handle_tone = raw_tone.map(|tone| {
        handle_kind.map_or(tone, |kind| {
            entity_handle::effective_handle_tone(tone, kind, reveal_failing_standing)
        })
    });
    let handle_line = entity_handle::entity_handle_line_for_type(
        &escape_html(&at),
        rating,
        inv.counterparty_status_key.as_deref(),
        Some(inv.counterparty_type_key.as_str()),
        death_immune,
        reveal_failing_standing,
    );
    let handle_linkable = handle_tone
        .map(entity_handle::entity_handle_is_linkable)
        .unwrap_or(true);
    let handle_block = entity_handle::feed_entity_handle_block_html(
        &escape_html(handle),
        &handle_line,
        handle_linkable,
    );
    let display_name = escape_html(inv.counterparty_label.trim());
    let avatar_head = if handle_linkable {
        format!(
            r#"<a class="public-post-avatar-link feed-profile-link" href="/home/profile/{handle}" data-profile-handle="{handle}" aria-label="Open profile @{handle}"><span class="brand-frame brand-frame--toolbar public-post-avatar"><span class="avatar-stack">{avatar_inner}{avatar_stars}</span></span></a>"#,
            handle = escape_html(handle),
            avatar_inner = avatar_inner,
            avatar_stars = avatar_stars,
        )
    } else {
        format!(
            r#"<span class="public-post-avatar-link"><span class="brand-frame brand-frame--toolbar public-post-avatar"><span class="avatar-stack">{avatar_inner}{avatar_stars}</span></span></span>"#,
            avatar_inner = avatar_inner,
            avatar_stars = avatar_stars,
        )
    };
    format!(
        r#"<div class="public-post-head outstanding-invitation-head"><div class="public-post-head-identity">{avatar_head}<div class="public-post-handle-block">{handle_block}<span class="public-post-name-row"><span class="public-post-display-name">{display_name}</span>{flag_html}</span></div></div></div>"#,
        avatar_head = avatar_head,
        handle_block = handle_block,
        display_name = display_name,
        flag_html = flag_html,
    )
}

pub fn invitation_card_html(inv: &PendingInvitation, loc: &str, c: &Catalog) -> String {
    let (intro_key, rel_label) = match inv.kind {
        PendingInvitationKind::CommunityMembership => (
            "ui.home.pending_invitation_community_intro",
            None,
        ),
        PendingInvitationKind::HumanConnection => (
            "ui.home.pending_invitation_human_intro",
            inv.relationship_type_key.as_deref().map(|k| {
                c.t(
                    loc,
                    &format!(
                        "ref_values.{}.{}",
                        ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE,
                        k
                    ),
                )
            }),
        ),
    };
    let intro = if let Some(rel) = rel_label {
        c.tf(loc, intro_key, &[("relationship", rel.as_str())])
    } else {
        c.t(loc, intro_key)
    };
    let intro_html = escape_html(&intro);
    let respond_prompt = escape_html(&c.t(loc, "ui.home.pending_invitation_respond_prompt"));
    let accept_lbl = escape_html(&c.t(loc, "ui.entity_private.connect_accept"));
    let decline_lbl = escape_html(&c.t(loc, "ui.entity_private.connect_decline"));
    let block_lbl = escape_html(&c.t(loc, "ui.entity_private.connect_block"));
    let rel_id = inv.relationship_id;
    let head_html = counterparty_head_html(inv);
    let accent = entity_type_accent::profile_card_accent_class(inv.counterparty_type_key.as_str());
    let block_option = if inv.kind == PendingInvitationKind::HumanConnection {
        format!(r#"<option value="block">{block_lbl}</option>"#)
    } else {
        String::new()
    };
    format!(
        r#"<section class="card app-card juror-summons-card outstanding-invitation-card{accent}" data-pending-invitation="1" data-relationship-id="{rel_id}">
  <div class="card-accent" role="presentation"></div>
  {head_html}
  <p class="juror-summons-intro outstanding-invitation-intro">{intro_html}</p>
  <div class="juror-summons-actions">
    <section class="profile-block profile-connect-block juror-summons-connect-block">
      <div class="profile-connect-inline profile-connect-inline--solo">
        <select id="pending-invitation-response-{rel_id}" class="field-input profile-connect-select profile-connect-select--solo" data-pending-invitation-select="1" aria-label="{respond_prompt}">
          <option value="">{respond_prompt}</option>
          <option value="accept">{accept_lbl}</option>
          <option value="decline">{decline_lbl}</option>
          {block_option}
        </select>
      </div>
    </section>
  </div>
</section>"#,
    )
}
