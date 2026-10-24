//! Authenticated home-feed post cards and shared OG/body helpers.

use crate::content::sanitize as content_sanitize;
use crate::html_escape::escape_html;
use crate::i18n;
use crate::ref_keys;
use crate::social::community_graph;
use crate::social::engagement::PostFeel;
use crate::social::feed::HomeFeedRow;
use crate::ui::entity_handle;
use crate::ui::datetime_format;
use crate::ui::entity_type_accent;
use crate::ui::nom_icon;
use crate::ui::post_icons;
use crate::ui::snippets;

/// `post_audience_scope` keys that are not navigable entity handles.
const POST_AUDIENCE_NON_PROFILE_KEYS: &[&str] =
    &["public", "private", "friends", "family", "colleague", "colleagues", "followers", "base"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PostVerdictFeedMode {
    Normal,
    UnderReview,
    RemovedHuman,
    RemovedCouncil,
    RemovedNoJury,
    ExoneratedHuman,
    ExoneratedCouncil,
    EmergencyStandingApplied,
    EmergencyContestWon,
    EmergencyContestLost,
}

fn post_verdict_feed_mode(r: &HomeFeedRow) -> PostVerdictFeedMode {
    if r.post_status_key.as_deref() == Some(ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW) {
        return PostVerdictFeedMode::UnderReview;
    }
    if r.post_status_key.as_deref() == Some(ref_keys::KEY_FLAGGED) {
        if r.judicial_appeal_case_status_key.as_deref() == Some(ref_keys::KEY_JUDICIAL_CASE_RESOLVED) {
            return PostVerdictFeedMode::RemovedCouncil;
        }
        if r.judicial_case_status_key.as_deref() == Some(ref_keys::KEY_JUDICIAL_CASE_NO_JURY) {
            return PostVerdictFeedMode::RemovedNoJury;
        }
        if r.judicial_case_status_key.as_deref() == Some(ref_keys::KEY_JUDICIAL_CASE_RESOLVED) {
            return PostVerdictFeedMode::RemovedHuman;
        }
    }
    if r.post_status_key.as_deref() == Some(ref_keys::KEY_VERIFIED) {
        if r.judicial_appeal_case_status_key.as_deref() == Some(ref_keys::KEY_JUDICIAL_CASE_RESOLVED) {
            return PostVerdictFeedMode::ExoneratedCouncil;
        }
        if r.judicial_post_moderation_case_id.is_some()
            && r.judicial_case_status_key.as_deref() == Some(ref_keys::KEY_JUDICIAL_CASE_RESOLVED)
        {
            return PostVerdictFeedMode::ExoneratedHuman;
        }
    }
    PostVerdictFeedMode::Normal
}

fn is_emergency_moderation_case(r: &HomeFeedRow) -> bool {
    r.judicial_trigger_key.as_deref() == Some(ref_keys::KEY_JUDICIAL_TRIGGER_EMERGENCY_SAFETY)
}

fn emergency_author_verdict_mode(r: &HomeFeedRow, viewer_user_id: u64) -> Option<PostVerdictFeedMode> {
    if r.post_created_by != Some(viewer_user_id) || !is_emergency_moderation_case(r) {
        return None;
    }
    if r.judicial_standing_applied != 0
        && r.post_status_key.as_deref() == Some(ref_keys::KEY_POST_SUPPRESSED)
        && r.judicial_case_status_key.as_deref() == Some(ref_keys::KEY_JUDICIAL_CASE_CONTAINED)
    {
        return Some(PostVerdictFeedMode::EmergencyStandingApplied);
    }
    if r.judicial_case_status_key.as_deref() == Some(ref_keys::KEY_JUDICIAL_CASE_RESOLVED) {
        if r.post_status_key.as_deref() == Some(ref_keys::KEY_VERIFIED) {
            return Some(PostVerdictFeedMode::EmergencyContestWon);
        }
        if r.post_status_key.as_deref() == Some(ref_keys::KEY_POST_SUPPRESSED) {
            return Some(PostVerdictFeedMode::EmergencyContestLost);
        }
    }
    None
}

fn council_appeal_in_progress(r: &HomeFeedRow) -> bool {
    matches!(
        r.judicial_appeal_case_status_key.as_deref(),
        Some("escalated_to_ai") | Some("open") | Some("jury_active")
    )
}

fn human_post_moderation_case_active(r: &HomeFeedRow) -> bool {
    if r.post_status_key.as_deref() != Some(ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW) {
        return false;
    }
    if council_appeal_in_progress(r) {
        return false;
    }
    matches!(
        r.judicial_case_status_key.as_deref(),
        Some("open") | Some("jury_active") | Some("escalated_to_ai")
    )
}

fn active_judicial_workflow(r: &HomeFeedRow) -> bool {
    human_post_moderation_case_active(r) || council_appeal_in_progress(r)
}

fn is_emergency_suppressed_hold(r: &HomeFeedRow, viewer_user_id: u64) -> bool {
    r.post_created_by == Some(viewer_user_id)
        && r.post_status_key.as_deref() == Some(ref_keys::KEY_POST_SUPPRESSED)
        && r.judicial_case_status_key.as_deref() == Some(ref_keys::KEY_JUDICIAL_CASE_CONTAINED)
}

/// Author-only: emergency-safety thread still `suppressed` (hold, standing applied, or contest lost).
fn is_emergency_author_suppressed(r: &HomeFeedRow, viewer_user_id: u64) -> bool {
    r.post_created_by == Some(viewer_user_id)
        && r.post_status_key.as_deref() == Some(ref_keys::KEY_POST_SUPPRESSED)
        && is_emergency_moderation_case(r)
}

fn can_contest_emergency(r: &HomeFeedRow, opts: &FeedCardOptions) -> bool {
    opts.show_judicial_ctas && is_emergency_suppressed_hold(r, opts.viewer_user_id)
}

fn emergency_hold_notice_html(loc: &str, c: &i18n::Catalog, opts: &FeedCardOptions) -> String {
    let body = nom_icon::locale_with_nom_price_paren(
        c,
        loc,
        "ui.post_emergency.hold_notice",
        crate::pricing::BillableService::EmergencyContest.nom_cost(),
    );
    let crisis_panel = crate::crisis_resources::panel_for_viewer(
        loc,
        opts.viewer_country_code.as_deref(),
        &[],
        true,
    );
    format!(
        r#"<div class="post-emergency-hold-notice" role="status"><p class="post-emergency-hold-notice-body">{body}</p></div>{crisis_panel}"#,
        body = body,
        crisis_panel = crisis_panel,
    )
}

fn post_verdict_notice_i18n_key(mode: PostVerdictFeedMode) -> Option<&'static str> {
    match mode {
        PostVerdictFeedMode::ExoneratedHuman => Some("ui.post_verdict.exonerated_human"),
        PostVerdictFeedMode::ExoneratedCouncil => Some("ui.post_verdict.exonerated_council"),
        PostVerdictFeedMode::RemovedHuman => Some("ui.post_verdict.removed_human"),
        PostVerdictFeedMode::RemovedCouncil => Some("ui.post_verdict.removed_council"),
        PostVerdictFeedMode::RemovedNoJury => Some("ui.post_verdict.removed_no_jury"),
        PostVerdictFeedMode::EmergencyStandingApplied => {
            Some("ui.post_verdict.emergency_standing_applied")
        }
        PostVerdictFeedMode::EmergencyContestWon => Some("ui.post_verdict.emergency_contest_won"),
        PostVerdictFeedMode::EmergencyContestLost => Some("ui.post_verdict.emergency_contest_lost"),
        _ => None,
    }
}

fn removed_feed_card_modifier(mode: PostVerdictFeedMode) -> &'static str {
    match mode {
        PostVerdictFeedMode::RemovedHuman => " public-post-card--removed-human",
        PostVerdictFeedMode::RemovedCouncil => " public-post-card--removed-council",
        PostVerdictFeedMode::RemovedNoJury => " public-post-card--removed-no-jury",
        _ => "",
    }
}

fn post_verdict_notice_text(mode: PostVerdictFeedMode, loc: &str, c: &i18n::Catalog) -> Option<String> {
    post_verdict_notice_i18n_key(mode).map(|key| c.t(loc, key))
}

fn post_verdict_notice_html(mode: PostVerdictFeedMode, loc: &str, c: &i18n::Catalog) -> String {
    let Some(body) = post_verdict_notice_text(mode, loc, c) else {
        return String::new();
    };
    let body = escape_html(&body);
    format!(
        r#"<div class="post-verdict-notice" role="status"><span class="post-verdict-notice-author">@256</span><p class="post-verdict-notice-body">{body}</p></div>"#,
        body = body,
    )
}

fn judicial_workflow_status_badge(row: &HomeFeedRow, loc: &str, c: &i18n::Catalog) -> Option<String> {
    if council_appeal_in_progress(row) {
        return Some(c.t(loc, "ui.judicial_post_case.status_escalated"));
    }
    if human_post_moderation_case_active(row) {
        return judicial_case_status_badge(row, loc, c);
    }
    if row.post_status_key.as_deref() == Some(ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW) {
        return Some(c.t(loc, "ui.judicial_post_case.status_escalated"));
    }
    None
}

fn render_removed_feed_card(
    r: &HomeFeedRow,
    mode: PostVerdictFeedMode,
    opts: &FeedCardOptions,
    loc: &str,
    c: &i18n::Catalog,
) -> String {
    let notice_html = post_verdict_notice_html(mode, loc, c);
    let workflow_active = active_judicial_workflow(r);
    let actions_html = secondary_actions_html(r, opts, workflow_active, loc, c);
    let modifier = removed_feed_card_modifier(mode);
    format!(
        r#"<section class="card app-card public-post-card public-post-card--removed public-post-card--text{modifier}" data-post-id="{post_id}" data-post-entity-id="{post_entity_id}" data-judicial-feed-suppress="1"><div class="card-accent" role="presentation"></div>{actions_row}{notice_html}</section>"#,
        modifier = modifier,
        post_id = r.id,
        post_entity_id = r.post_entity_id,
        actions_row = if actions_html.is_empty() {
            String::new()
        } else {
            format!(
                r#"<div class="public-post-head"><div class="public-post-head-identity"></div>{actions}</div>"#,
                actions = actions_html
            )
        },
        notice_html = notice_html,
    )
}

pub struct FeedCardOptions {
    pub viewer_user_id: u64,
    /// Viewer's primary human entity — used as `entity_id` when commenting on posts.
    pub viewer_human_entity_id: Option<u64>,
    pub show_judicial_ctas: bool,
    /// Vote/comment/feel row — off on judicial filing pages (read-only post preview).
    pub show_engagement_bar: bool,
    /// Viewer profile ISO country (e.g. `US`) for crisis resource filtering.
    pub viewer_country_code: Option<String>,
    /// When false, OG preview images from unknown domains are click-to-reveal.
    pub preview_image_allowed: bool,
    pub og_preview_reveal_label: String,
    pub og_preview_withheld_hint: Option<String>,
    /// When set, body-embedded links on this host skip OG preview (platform help/legal URLs).
    pub public_base_url: Option<String>,
}

/// Hostname label for catalogue links ingested without OG crawl metadata.
#[must_use]
pub fn host_label_from_href(href: &str) -> String {
    href.trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(href)
        .trim()
        .to_string()
}

/// Rich link preview: 50/50 image | title + source.
#[must_use]
pub fn og_html(
    og_title: Option<&str>,
    og_image_url: Option<&str>,
    og_site_name: Option<&str>,
    href: Option<&str>,
) -> String {
    og_html_with_preview(
        og_title,
        og_image_url,
        og_site_name,
        href,
        true,
        "Show preview",
        None,
    )
}

/// Same as [`og_html`], with domain preview reputation gating for remote images.
#[must_use]
pub fn og_html_with_preview(
    og_title: Option<&str>,
    og_image_url: Option<&str>,
    og_site_name: Option<&str>,
    href: Option<&str>,
    preview_image_allowed: bool,
    reveal_label: &str,
    withheld_hint: Option<&str>,
) -> String {
    let href = href.map(str::trim).filter(|s| !s.is_empty());
    let img_raw = og_image_url
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|u| {
            let base = href.unwrap_or(u);
            crate::content::url_extract::sanitize_http_image_url(u, base)
        });
    let withhold_image = img_raw.is_some() && !preview_image_allowed;
    let img = if withhold_image {
        None
    } else {
        img_raw.as_deref()
    };
    let title_raw = og_title.map(str::trim).filter(|s| !s.is_empty());
    let site_raw = og_site_name.map(str::trim).filter(|s| !s.is_empty());
    let (title_s, site_s) = if title_raw.is_some() || site_raw.is_some() || img.is_some() || withhold_image {
        (
            escape_html(title_raw.unwrap_or("")),
            escape_html(site_raw.unwrap_or("")),
        )
    } else if let Some(u) = href {
        let host = host_label_from_href(u);
        if host.is_empty() {
            return String::new();
        }
        let escaped = escape_html(&host);
        (escaped.clone(), escaped)
    } else {
        return String::new();
    };

    let media_inner = if let Some(u) = img {
        format!(
            r#"<img class="public-post-og-image" src="{}" alt="" loading="lazy" decoding="async" />"#,
            escape_html(u)
        )
    } else if let Some(u) = img_raw.as_deref().filter(|_| withhold_image) {
        let hint_html = withheld_hint
            .filter(|h| !h.is_empty())
            .map(|h| {
                format!(
                    r#"<p class="public-post-og-withheld-hint">{}</p>"#,
                    escape_html(h)
                )
            })
            .unwrap_or_default();
        format!(
            r#"<button type="button" class="public-post-og-reveal-btn" data-og-image-url="{url}">{label}</button>{hint}"#,
            url = escape_html(u),
            label = escape_html(reveal_label),
            hint = hint_html,
        )
    } else {
        String::new()
    };

    let og_class = if withhold_image {
        "public-post-og public-post-og--image-withheld"
    } else if img.is_some() {
        "public-post-og"
    } else {
        "public-post-og public-post-og--no-image"
    };

    let data_img = if withhold_image {
        img_raw
            .as_deref()
            .map(|u| format!(r#" data-og-image-url="{}""#, escape_html(u)))
            .unwrap_or_default()
    } else {
        String::new()
    };

    let block = format!(
        r#"<div class="{og_class}"{data_img}><div class="public-post-og-media">{media}</div><div class="public-post-og-copy"><div class="public-post-og-title">{title}</div><div class="public-post-og-site">{site}</div></div></div>"#,
        og_class = og_class,
        data_img = data_img,
        media = media_inner,
        title = title_s,
        site = site_s
    );

    match href {
        Some(u) => format!(
            r#"<a class="public-post-og-link" href="{href}" target="_blank" rel="noopener noreferrer">{block}</a>"#,
            href = escape_html(u),
            block = block
        ),
        None => block,
    }
}

#[must_use]
pub fn first_http_url_from_text(s: &str) -> Option<String> {
    for (i, _) in s.char_indices() {
        let slice = &s[i..];
        if !(slice.starts_with("http://") || slice.starts_with("https://")) {
            continue;
        }
        let mut end = s.len();
        for (off, ch) in slice.char_indices() {
            if ch.is_whitespace() || matches!(ch, '<' | '"' | '\'' | ')' | ']') {
                end = i + off;
                break;
            }
        }
        let raw = &s[i..end];
        let trimmed = raw.trim_end_matches(|c: char| ".,;:!?)".contains(c));
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn redact_plain_http_urls_for_judicial(text: &str) -> String {
    let mut redacted = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        let pos_https = rest.find("https://");
        let pos_http = rest.find("http://");
        let url_start = match (pos_https, pos_http) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        match url_start {
            None => {
                redacted.push_str(rest);
                break;
            }
            Some(pos) => {
                redacted.push_str(&rest[..pos]);
                redacted.push_str("[link withheld during review]");
                let after = &rest[pos..];
                let skip_len = if after.starts_with("https://") {
                    "https://".len()
                } else {
                    "http://".len()
                };
                let mut tail = &after[skip_len..];
                let end = tail
                    .find(|c: char| c.is_whitespace() || matches!(c, '<' | '"' | '\'' | ')' | ']'))
                    .unwrap_or(tail.len());
                tail = &tail[end..];
                rest = tail;
            }
        }
    }
    escape_html(&redacted)
}

fn body_inner_html(r: &HomeFeedRow, body_quarantine: bool) -> String {
    if body_quarantine {
        if !r.body_html_sanitized.trim().is_empty() {
            return content_sanitize::sanitize_post_markup_judicial_quarantine(&r.body_html_sanitized);
        }
        if !r.body_markup.trim().is_empty() {
            return content_sanitize::sanitize_post_markup_judicial_quarantine(&r.body_markup);
        }
        return redact_plain_http_urls_for_judicial(&r.body_plain);
    }
    if !r.body_html_sanitized.trim().is_empty() {
        return r.body_html_sanitized.clone();
    }
    if !r.body_markup.trim().is_empty() {
        return r.body_markup.clone();
    }
    escape_html(&r.body_plain)
}

fn body_class_suffix(r: &HomeFeedRow) -> &'static str {
    if !r.body_html_sanitized.trim().is_empty() || !r.body_markup.trim().is_empty() {
        ""
    } else {
        " public-post-body--plain-fallback"
    }
}

fn has_link_preview_data(r: &HomeFeedRow) -> bool {
    (r.has_content_link != 0
        && r.link_href
            .as_deref()
            .map(str::trim)
            .is_some_and(|s| !s.is_empty()))
        || r.og_title
            .as_deref()
            .map(str::trim)
            .is_some_and(|s| !s.is_empty())
        || r.og_image_url
            .as_deref()
            .map(str::trim)
            .is_some_and(|s| !s.is_empty())
        || r.link_href
            .as_deref()
            .map(str::trim)
            .is_some_and(|s| !s.is_empty())
        || first_http_url_from_text(&r.body_plain).is_some()
        || first_http_url_from_text(&r.body_html_sanitized).is_some()
        || first_http_url_from_text(&r.body_markup).is_some()
}

fn can_share_link_to_audience(r: &HomeFeedRow, suppress_secondary_actions: bool) -> bool {
    !suppress_secondary_actions && r.has_content_link != 0 && has_link_preview_data(r)
}

fn can_copy_public_link(r: &HomeFeedRow, suppress_secondary_actions: bool) -> bool {
    !suppress_secondary_actions && r.is_public != 0
}

fn can_open_post_case(
    r: &HomeFeedRow,
    viewer_user_id: u64,
    suppress_secondary_actions: bool,
    include_ctas: bool,
) -> bool {
    if !include_ctas || suppress_secondary_actions {
        return false;
    }
    if active_judicial_workflow(r) {
        return false;
    }
    if !crate::social::feed::viewer_may_file_human_post_dispute(viewer_user_id, r) {
        return false;
    }
    r.judicial_case_status_key.as_deref() != Some("resolved")
}

fn can_escalate_to_council(
    r: &HomeFeedRow,
    _viewer_user_id: u64,
    suppress_secondary_actions: bool,
    include_ctas: bool,
) -> bool {
    if !include_ctas || suppress_secondary_actions {
        return false;
    }
    if active_judicial_workflow(r) {
        return false;
    }
    r.judicial_case_status_key.as_deref() == Some("resolved")
        && r.judicial_post_moderation_case_id.is_some()
        && r.judicial_moderation_appeal_count == 0
        && r.judicial_viewer_is_moderation_party != 0
}

fn judicial_case_status_badge(row: &HomeFeedRow, loc: &str, c: &i18n::Catalog) -> Option<String> {
    if row.post_status_key.as_deref() != Some(ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW) {
        return None;
    }
    match row.judicial_case_status_key.as_deref() {
        Some("open") => Some(c.t(loc, "ui.judicial_post_case.status_started")),
        Some("jury_active") => Some(c.t(loc, "ui.judicial_post_case.status_consideration")),
        Some("escalated_to_ai") => Some(c.t(loc, "ui.judicial_post_case.status_escalated")),
        Some("resolved") | Some("voided") => Some(c.t(loc, "ui.judicial_post_case.status_resolved")),
        Some("no_jury") => Some(c.t(loc, "ui.judicial_post_case.status_no_jury")),
        _ => Some(c.t(loc, "ui.judicial_post_case.status_started")),
    }
}

fn judicial_case_author_withdraw_select_html(
    row: &HomeFeedRow,
    status_label: &str,
    loc: &str,
    c: &i18n::Catalog,
) -> String {
    format!(
        r#"<form class="public-post-judicial-withdraw-form" method="post" action="/home/judicial/post/{post_id}/author-withdraw" data-judicial-withdraw-bound="0"><div class="public-post-judicial-withdraw-inline"><select name="withdraw_action" class="field-input profile-connect-select profile-connect-select--solo public-post-judicial-withdraw-select" aria-label="{aria}"><option value="">{status_esc}</option><option value="withdraw_post">{del_esc}</option></select></div></form>"#,
        post_id = row.id,
        aria = escape_html(status_label),
        status_esc = escape_html(status_label),
        del_esc = escape_html(&c.t(loc, "ui.judicial_post_case.withdraw_delete_post")),
    )
}

fn secondary_actions_html(
    r: &HomeFeedRow,
    opts: &FeedCardOptions,
    suppress_secondary_actions: bool,
    loc: &str,
    c: &i18n::Catalog,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    let suffix = format!("p{}", r.id);

    if can_share_link_to_audience(r, suppress_secondary_actions) {
        let link_id = r.content_link_id.unwrap_or(0);
        let label = escape_html(&c.t(loc, "ui.post_engagement.share_link_aria"));
        let icon = post_icons::post_icon_share_link(&suffix);
        parts.push(format!(
            r#"<button type="button" class="post-action-btn post-share-audience-btn" data-share-link-id="{link_id}" data-share-entity-id="{entity_id}" title="{label}" aria-label="{label}">{icon}</button>"#,
            link_id = link_id,
            entity_id = r.post_entity_id,
            label = label,
            icon = icon,
        ));
    }

    if can_copy_public_link(r, suppress_secondary_actions) {
        let label = escape_html(&c.t(loc, "ui.post_engagement.copy_public_link_aria"));
        let icon = post_icons::post_icon_copy_public_link(&suffix);
        parts.push(format!(
            r#"<button type="button" class="post-action-btn" data-public-link="/public/posts/{post_id}" title="{label}" aria-label="{label}">{icon}</button>"#,
            post_id = r.id,
            label = label,
            icon = icon,
        ));
    }

    if opts.show_judicial_ctas {
        if can_contest_emergency(r, opts) {
            let contest_cost = crate::pricing::BillableService::EmergencyContest.nom_cost();
            let label_esc = escape_html(&c.tf(
                loc,
                "ui.post_emergency.contest_cta_aria",
                &[("nom_price", &nom_icon::nom_amount_plain(contest_cost))],
            ));
            let icon = post_icons::post_icon_open_case(&suffix);
            parts.push(format!(
                r#"<a class="post-action-btn" href="/home/judicial/post/{post_id}/contest?return_to=%2Fhome" title="{label_esc}" aria-label="{label_esc}">{icon}</a>"#,
                post_id = r.id,
                label_esc = label_esc,
                icon = icon,
            ));
        } else if active_judicial_workflow(r) {
            if let Some(status_label) = judicial_workflow_status_badge(r, loc, c) {
                let author_may_withdraw = r.post_created_by == Some(opts.viewer_user_id)
                    && r.post_status_key.as_deref() == Some(ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW)
                    && !matches!(
                        r.judicial_case_status_key.as_deref(),
                        Some("resolved") | Some("voided") | Some("no_jury")
                    );
                if author_may_withdraw {
                    parts.push(judicial_case_author_withdraw_select_html(
                        r,
                        &status_label,
                        loc,
                        c,
                    ));
                } else {
                    parts.push(format!(
                        r#"<span class="public-post-judicial-status">{}</span>"#,
                        escape_html(&status_label)
                    ));
                }
            }
        } else if can_escalate_to_council(r, opts.viewer_user_id, suppress_secondary_actions, true) {
            let case_id = r.judicial_post_moderation_case_id.unwrap_or(0);
            let label = escape_html(&c.t(loc, "ui.judicial_post_case.cta_appeal_from_feed"));
            let icon = post_icons::post_icon_escalate_council(&suffix);
            parts.push(format!(
                r#"<a class="post-action-btn" href="/home/judicial/post/{post_id}/appeal?from_case={case_id}&return_to=%2Fhome" title="{label}" aria-label="{label}">{icon}</a>"#,
                post_id = r.id,
                case_id = case_id,
                label = label,
                icon = icon,
            ));
        } else if can_open_post_case(r, opts.viewer_user_id, suppress_secondary_actions, true) {
            let label = escape_html(&c.t(loc, "ui.judicial_post_case.cta_from_feed"));
            let icon = post_icons::post_icon_open_case(&suffix);
            parts.push(format!(
                r#"<a class="post-action-btn" href="/home/judicial/post/{post_id}?return_to=%2Fhome" title="{label}" aria-label="{label}">{icon}</a>"#,
                post_id = r.id,
                label = label,
                icon = icon,
            ));
        }
    }

    if parts.is_empty() {
        return String::new();
    }
    format!(
        r#"<div class="public-post-head-actions" role="group" aria-label="{}">{inner}</div>"#,
        escape_html(&c.t(loc, "ui.post_card.secondary_actions_aria")),
        inner = parts.join("")
    )
}

fn engagement_show_discussion_ui(r: &HomeFeedRow, viewer_user_id: u64) -> bool {
    r.viewer_engagement_vote.is_some()
        || (r.post_created_by == Some(viewer_user_id) && r.non_author_root_vote_count > 0)
}

fn engagement_bar_html(r: &HomeFeedRow, viewer_user_id: u64, loc: &str, c: &i18n::Catalog) -> String {
    if r.post_created_by == Some(viewer_user_id) && r.non_author_root_vote_count <= 0 {
        return String::new();
    }

    let stars = r
        .post_engagement_rating
        .to_string()
        .parse::<f64>()
        .unwrap_or(2.5);
    let feel = PostFeel::from_stars(stars);
    let feel_key = feel.key();
    let feel_label = match feel {
        PostFeel::Loved => c.t(loc, "ui.post_engagement.feel_loved"),
        PostFeel::Controversial => c.t(loc, "ui.post_engagement.feel_controversial"),
        PostFeel::Loathed => c.t(loc, "ui.post_engagement.feel_loathed"),
    };
    let feel_heart = post_icons::post_icon_feel_heart(feel_key);
    let count_icon = post_icons::post_icon_thread_count();
    let thumb_gid = format!("feed-vote-{}", r.id);
    let up = post_icons::post_icon_vote_up(&thumb_gid);
    let down = post_icons::post_icon_vote_down(&thumb_gid);
    let show_discussion = engagement_show_discussion_ui(r, viewer_user_id);
    let count = r.engagement_comment_count.max(0);
    let vote_aria = escape_html(&c.t(loc, "ui.post_engagement.vote_aria"));
    let vote_block = if r.post_created_by == Some(viewer_user_id) {
        String::new()
    } else {
        format!(
            r#"<span class="post-engagement-votes" role="group" aria-label="{vote_aria}">
    <button type="button" class="post-action-btn post-engagement-vote-btn post-engagement-vote-btn--yes" data-vote="yes" aria-pressed="false">{up}</button>
    <button type="button" class="post-action-btn post-engagement-vote-btn post-engagement-vote-btn--no" data-vote="no" aria-pressed="false">{down}</button>
  </span>"#,
            vote_aria = vote_aria,
            up = up,
            down = down,
        )
    };

    let thread_slot = format!(
        r#"<div class="post-engagement-thread-slot{preview_class}" data-thread-slot="{root_id}"></div>"#,
        preview_class = if show_discussion {
            ""
        } else {
            " post-engagement-thread-slot--preview"
        },
        root_id = r.id
    );

    if show_discussion {
        let compose = format!(
            r#"<label class="post-engagement-compose-wrap"><span class="visually-hidden">{}</span><input type="text" class="post-engagement-compose-input" data-post-comment-root="{}" placeholder="{}" autocomplete="off" /></label>"#,
            escape_html(&c.t(loc, "ui.post_engagement.comment_placeholder")),
            r.id,
            escape_html(&c.t(loc, "ui.post_engagement.comment_placeholder")),
        );
        format!(
            r#"<div class="post-engagement-bar post-engagement-bar--voted" data-post-engagement-bar data-root-id="{root_id}" data-voted="1" data-feel="{feel_key}">
  {compose}<span class="post-engagement-feel-badge post-engagement-feel-badge--mood post-engagement-feel--{feel_key}" title="{feel_esc}" aria-label="{feel_esc}">{feel_heart}</span>
</div>{thread_slot}"#,
            root_id = r.id,
            feel_key = feel_key,
            compose = compose,
            feel_esc = escape_html(&feel_label),
            feel_heart = feel_heart,
            thread_slot = thread_slot,
        )
    } else {
        let (count_label, count_aria) = if count == 0 {
            let prompt = c.t(loc, "ui.post_engagement.vote_to_start");
            (escape_html(&prompt), escape_html(&prompt))
        } else {
            let aria = c.tf(
                loc,
                "ui.post_engagement.comment_count_aria",
                &[("count", &count.to_string())],
            );
            (count.to_string(), escape_html(&aria))
        };
        let count_class = if count == 0 {
            "post-engagement-count post-engagement-count--prompt"
        } else {
            "post-engagement-count"
        };
        format!(
            r#"<div class="post-engagement-bar post-engagement-bar--pre-vote" data-post-engagement-bar data-root-id="{root_id}" data-voted="0">
  <span class="post-engagement-count-badge" aria-label="{count_aria}">{count_icon}<span class="{count_class}" data-comment-count>{count_label}</span></span>
  {vote_block}
</div>{thread_slot}"#,
            root_id = r.id,
            count_aria = count_aria,
            count_icon = count_icon,
            count_class = count_class,
            count_label = count_label,
            vote_block = vote_block,
            thread_slot = thread_slot,
        )
    }
}

/// Logged-out public cards: comment count only (no vote / compose).
#[must_use]
pub fn public_guest_engagement_bar_html(comment_count: i64, loc: &str, c: &i18n::Catalog) -> String {
    let count = comment_count.max(0);
    let count_aria = escape_html(&c.tf(
        loc,
        "ui.post_engagement.comment_count_aria",
        &[("count", &count.to_string())],
    ));
    let count_icon = post_icons::post_icon_thread_count();
    format!(
        r#"<div class="post-engagement-bar post-engagement-bar--public-guest">
  <span class="post-engagement-count-badge" aria-label="{count_aria}">{count_icon}<span class="post-engagement-count">{count}</span></span>
</div>"#,
        count_aria = count_aria,
        count_icon = count_icon,
        count = count,
    )
}

#[must_use]
pub fn feed_post_audience_line_html(
    poster_handle: &str,
    raw_label: &str,
    suppress_public_scope: bool,
) -> String {
    let poster = poster_handle.trim();
    let mut chunks: Vec<String> = Vec::new();
    for raw_tok in raw_label.split(',') {
        let tok = raw_tok.trim();
        if tok.is_empty() || !tok.starts_with('@') {
            continue;
        }
        let key = tok.trim_start_matches('@');
        if key.is_empty() {
            continue;
        }
        if key.eq_ignore_ascii_case(poster) {
            continue;
        }
        let key_lower = key.to_ascii_lowercase();
        if suppress_public_scope && key_lower == "public" {
            continue;
        }
        let is_scope = POST_AUDIENCE_NON_PROFILE_KEYS
            .iter()
            .any(|k| k.eq_ignore_ascii_case(&key_lower));
        if is_scope {
            let display = if key_lower == "followers" {
                "@base"
            } else if key_lower == "colleague" {
                "@colleagues"
            } else {
                tok
            };
            chunks.push(format!(
                r#"<span class="public-post-audience-token public-post-audience-token--scope">{}</span>"#,
                escape_html(display)
            ));
        } else {
            let h_esc = escape_html(key);
            let label_esc = escape_html(tok);
            chunks.push(format!(
                r#"<a class="public-post-audience-token feed-profile-link" href="/home/profile/{h}" data-profile-handle="{h}" aria-label="Open profile @{h}">{label}</a>"#,
                h = h_esc,
                label = label_esc
            ));
        }
    }
    if chunks.is_empty() {
        return String::new();
    }
    let mut inner = String::new();
    for (i, c) in chunks.iter().enumerate() {
        if i > 0 {
            inner.push_str(r#"<span class="public-post-audience-sep">, </span>"#);
        }
        inner.push_str(c);
    }
    format!(r#"<span class="public-post-audience-for">{}</span>"#, inner)
}

fn country_code_to_flag_emoji(raw: &str) -> Option<String> {
    let code = raw.trim().to_ascii_uppercase();
    if code.len() != 2 || !code.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let mut out = String::new();
    for ch in code.chars() {
        let base = 0x1F1E6u32;
        let offset = (ch as u32).saturating_sub('A' as u32);
        out.push(char::from_u32(base + offset)?);
    }
    Some(out)
}

/// Full authenticated feed / profile post card.
#[must_use]
pub fn render_authenticated_feed_card(
    r: &HomeFeedRow,
    opts: &FeedCardOptions,
    c: &i18n::Catalog,
    loc: &str,
) -> String {
    let verdict_mode = post_verdict_feed_mode(r);
    if matches!(
        verdict_mode,
        PostVerdictFeedMode::RemovedHuman
            | PostVerdictFeedMode::RemovedCouncil
            | PostVerdictFeedMode::RemovedNoJury
    ) {
        return render_removed_feed_card(r, verdict_mode, opts, loc, c);
    }

    let emergency_author_mode = emergency_author_verdict_mode(r, opts.viewer_user_id);
    let under_judicial_review =
        r.post_status_key.as_deref() == Some(ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW);
    let emergency_suppressed_hold =
        is_emergency_suppressed_hold(r, opts.viewer_user_id);
    let emergency_author_suppressed =
        is_emergency_author_suppressed(r, opts.viewer_user_id);
    let body_quarantine = under_judicial_review || emergency_author_suppressed;
    let workflow_active = active_judicial_workflow(r);
    let suppress_feed_interactions =
        workflow_active || emergency_suppressed_hold || emergency_author_suppressed;
    let link_url = if body_quarantine {
        None
    } else {
        first_http_url_from_text(&r.body_plain)
            .or_else(|| first_http_url_from_text(&r.body_html_sanitized))
            .or_else(|| first_http_url_from_text(&r.body_markup))
    };
    let has_attached_link = r.has_content_link != 0
        && r.link_href
            .as_deref()
            .map(str::trim)
            .is_some_and(|s| !s.is_empty());
    let is_linked = !body_quarantine
        && (link_url.is_some()
            || has_attached_link
            || r.body_html_sanitized.contains("<a ")
            || r.body_markup.contains("<a "));
    let is_public = r.is_public != 0 && !body_quarantine;

    let avatar = r
        .avatar_path
        .as_deref()
        .map(|p| {
            let src = crate::ui::avatar_url::img_src_from_stored_avatar(p);
            format!(
                r#"<img class="public-post-avatar-img" src="{}" alt="@{}" loading="lazy" decoding="async" referrerpolicy="no-referrer" />"#,
                escape_html(&src),
                escape_html(&r.handle)
            )
        })
        .unwrap_or_else(|| snippets::avatar_not_found_html().to_string());

    let show_ssr = matches!(
        r.entity_type_key.as_deref(),
        Some(ref_keys::KEY_ENTITY_TYPE_NON_PROFIT)
            | Some(ref_keys::KEY_ENTITY_TYPE_PROFIT)
            | Some(ref_keys::KEY_ENTITY_TYPE_COMMUNITY)
            | Some(ref_keys::KEY_ENTITY_TYPE_GATHERING)
    ) || (r.entity_type_key.as_deref() == Some(ref_keys::KEY_ENTITY_TYPE_HUMAN)
        && r.entity_created_by == opts.viewer_user_id);
    let avatar_stars = if show_ssr {
        snippets::ssr_stars_html(
            r.entity_rating.to_string().parse::<f32>().unwrap_or(0.0),
            "ssr-stars ssr-stars--avatar",
        )
    } else {
        String::new()
    };

    let body_class = format!(
        "public-post-body create-post-editor{}",
        body_class_suffix(r)
    );

    let cc = r
        .post_country_code
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| r.entity_country_code.as_deref())
        .unwrap_or("");
    let flag = country_code_to_flag_emoji(cc).unwrap_or_default();
    let flag_html = if flag.is_empty() {
        String::new()
    } else {
        let cc_l = cc.to_ascii_lowercase();
        format!(
            r#"<span class="public-post-flag-icon public-post-flag-inline fi fi-{cc_l}" data-country-code="{cc}" title="Posted from {cc}" aria-hidden="true"></span>"#,
            cc = escape_html(cc),
            cc_l = escape_html(&cc_l)
        )
    };

    let handle_at = format!("@{}", escape_html(&r.handle));
    let death_immune = community_graph::is_death_immune_handle(&r.handle);
    let reveal_failing_standing = r.entity_type_key.as_deref()
        == Some(ref_keys::KEY_ENTITY_TYPE_HUMAN)
        && r.entity_created_by == opts.viewer_user_id;
    let handle_kind = entity_handle::entity_handle_kind(r.entity_type_key.as_deref());
    let raw_tone = entity_handle::entity_handle_tone(
        r.entity_rating,
        r.entity_status_key.as_deref(),
        death_immune,
    );
    let handle_tone = raw_tone.map(|tone| {
        handle_kind.map_or(tone, |kind| {
            entity_handle::effective_handle_tone(tone, kind, reveal_failing_standing)
        })
    });
    let handle_line = entity_handle::entity_handle_line_for_type(
        &handle_at,
        r.entity_rating,
        r.entity_status_key.as_deref(),
        r.entity_type_key.as_deref(),
        death_immune,
        reveal_failing_standing,
    );
    let handle_linkable = handle_tone
        .map(entity_handle::entity_handle_is_linkable)
        .unwrap_or(true);
    let handle_block = entity_handle::feed_entity_handle_block_html(
        &escape_html(&r.handle),
        &handle_line,
        handle_linkable,
    );
    let display_name_line = r
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(escape_html)
        .unwrap_or_default();
    let audience_for_line = r
        .audience_for_label
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|labels| feed_post_audience_line_html(&r.handle, labels, body_quarantine))
        .unwrap_or_default();

    let body_id = format!("app-feed-post-body-{}", r.id);
    let show_more_label = c.t(loc, "ui.post_engagement.show_more");
    let show_more_btn = format!(
        r#"<button type="button" class="app-feed-post-expand-btn" data-feed-expand-target="{body_id}" hidden>{show_more_label}</button>"#,
        body_id = escape_html(&body_id),
        show_more_label = escape_html(&show_more_label),
    );

    let og_href_candidate = if body_quarantine {
        None
    } else {
        r.link_href
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .or_else(|| first_http_url_from_text(&r.body_plain))
            .or_else(|| first_http_url_from_text(&r.body_html_sanitized))
            .or_else(|| first_http_url_from_text(&r.body_markup))
    };
    let og_href = og_href_candidate.filter(|u| {
        if r.has_content_link != 0 {
            return true;
        }
        opts.public_base_url
            .as_deref()
            .is_none_or(|base| !crate::content::url_extract::is_platform_self_link(u, base))
    });
    let og = if body_quarantine {
        String::new()
    } else {
        og_html_with_preview(
            r.og_title.as_deref(),
            r.og_image_url.as_deref(),
            r.og_site_name.as_deref(),
            og_href.as_deref(),
            opts.preview_image_allowed,
            &opts.og_preview_reveal_label,
            opts.og_preview_withheld_hint.as_deref(),
        )
    };

    let mut card_classes = String::from("card app-card public-post-card ");
    card_classes.push_str(if is_linked {
        "public-post-card--linked"
    } else {
        "public-post-card--text"
    });
    if !is_public {
        card_classes.push_str(" private-post-card");
    }
    if body_quarantine {
        card_classes.push_str(" public-post-card--moderation");
    }
    if emergency_suppressed_hold {
        card_classes.push_str(" public-post-card--emergency-hold");
    }
    card_classes.push_str(entity_type_accent::profile_card_accent_class(
        r.entity_type_key.as_deref().unwrap_or("").trim(),
    ));

    let actions_html = secondary_actions_html(r, opts, suppress_feed_interactions, loc, c);
    let engagement_bar = if suppress_feed_interactions || !opts.show_engagement_bar {
        String::new()
    } else {
        engagement_bar_html(r, opts.viewer_user_id, loc, c)
    };
    let exoneration_notice = match verdict_mode {
        PostVerdictFeedMode::ExoneratedHuman | PostVerdictFeedMode::ExoneratedCouncil => {
            post_verdict_notice_html(verdict_mode, loc, c)
        }
        _ => String::new(),
    };
    let emergency_author_notice = emergency_author_mode
        .map(|mode| post_verdict_notice_html(mode, loc, c))
        .unwrap_or_default();
    let emergency_hold_notice = if emergency_suppressed_hold && emergency_author_notice.is_empty() {
        emergency_hold_notice_html(loc, c, opts)
    } else {
        String::new()
    };
    let verdict_outcome_notice = if emergency_author_notice.is_empty() {
        exoneration_notice
    } else {
        emergency_author_notice
    };
    let judicial_suppress = if suppress_feed_interactions {
        r#" data-judicial-feed-suppress="1""#
    } else {
        ""
    };

    let avatar_head = if handle_linkable {
        format!(
            r#"<a class="public-post-avatar-link feed-profile-link" href="/home/profile/{handle}" data-profile-handle="{handle}" aria-label="Open profile @{handle}"><span class="brand-frame brand-frame--toolbar public-post-avatar"><span class="avatar-stack">{avatar}{avatar_stars}</span></span></a>"#,
            handle = escape_html(&r.handle),
            avatar = avatar,
            avatar_stars = avatar_stars,
        )
    } else {
        format!(
            r#"<span class="public-post-avatar-link"><span class="brand-frame brand-frame--toolbar public-post-avatar"><span class="avatar-stack">{avatar}{avatar_stars}</span></span></span>"#,
            avatar = avatar,
            avatar_stars = avatar_stars,
        )
    };

    let identity_meta = format!(
        r#"<div class="public-post-handle-block">{handle_block}<span class="public-post-name-row"><span class="public-post-display-name">{display_name}</span>{flag_html}</span>{audience_for}</div>"#,
        handle_block = handle_block,
        display_name = display_name_line,
        flag_html = flag_html,
        audience_for = audience_for_line,
    );
    let published_at_html = datetime_format::post_published_at_html(loc, r.published_at);

    let viewer_entity_attr = opts
        .viewer_human_entity_id
        .map(|id| format!(r#" data-viewer-entity-id="{id}""#, id = id))
        .unwrap_or_default();
    let audience_attr = r
        .audience_for_label
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|label| {
            format!(
                r#" data-post-audience-handles="{}""#,
                escape_html(&format!("For: {label}"))
            )
        })
        .unwrap_or_default();
    format!(
        r#"<section class="{card_classes}" data-post-id="{post_id}" data-post-entity-id="{post_entity_id}"{viewer_entity_attr}{audience_attr}{judicial_suppress}{link_attr}><div class="card-accent" role="presentation"></div><div class="public-post-head"><div class="public-post-head-identity">{avatar_head}{identity_meta}</div>{actions_html}</div>{emergency_hold_notice}{og}<div class="public-post-published-row">{published_at_html}</div><div id="{body_id}" class="{body_class} app-feed-post-body" role="article">{body}</div>{show_more_btn}{exoneration_notice}{engagement_bar}</section>"#,
        card_classes = card_classes,
        post_id = r.id,
        post_entity_id = r.post_entity_id,
        viewer_entity_attr = viewer_entity_attr,
        engagement_bar = engagement_bar,
        judicial_suppress = judicial_suppress,
        link_attr = link_url
            .as_ref()
            .map(|u| format!(r#" data-link-url="{}""#, escape_html(u)))
            .unwrap_or_default(),
        avatar_head = avatar_head,
        identity_meta = identity_meta,
        actions_html = actions_html,
        og = og,
        published_at_html = published_at_html,
        emergency_hold_notice = emergency_hold_notice,
        body_id = escape_html(&body_id),
        body_class = body_class,
        body = body_inner_html(r, body_quarantine),
        show_more_btn = show_more_btn,
        exoneration_notice = verdict_outcome_notice,
    )
}

/// Anonymized corpus for jurors who accepted a seat (workspace only).
#[must_use]
pub fn juror_corpus_post_card_html(r: &HomeFeedRow, c: &i18n::Catalog, loc: &str) -> String {
    let party = c.t(loc, "ui.juror_workspace.party_redacted");
    let link = c.t(loc, "ui.juror_workspace.link_redacted");
    let heading = escape_html(&c.t(loc, "ui.juror_workspace.corpus_heading"));
    let body = crate::content::juror_anonymize::juror_facing_post_body(
        &r.body_html_sanitized,
        &r.body_markup,
        &r.body_plain,
        &party,
        &link,
    );
    let body_class = format!(
        "public-post-body create-post-editor juror-corpus-body{}",
        body_class_suffix(r)
    );
    format!(
        r#"<article class="card app-card judicial-post-case-card juror-corpus-card" data-juror-corpus="1">
  <div class="card-accent" role="presentation"></div>
  <h3 class="juror-workspace-h">{heading}</h3>
  <div class="juror-corpus-body-wrap {body_class}" role="article">{body}</div>
</article>"#,
        heading = heading,
        body_class = body_class,
        body = body,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_case_after_no_jury_or_voided() {
        let mut r = base_row();
        r.judicial_case_status_key = Some("no_jury".to_string());
        assert!(can_open_post_case(&r, 99, false, true));
        r.judicial_case_status_key = Some("voided".to_string());
        assert!(can_open_post_case(&r, 99, false, true));
    }

    #[test]
    fn author_cannot_open_human_jury_case() {
        let mut r = base_row();
        r.post_created_by = Some(42);
        assert!(!can_open_post_case(&r, 42, false, true));
        assert!(can_open_post_case(&r, 99, false, true));
    }

    #[test]
    fn no_open_case_when_resolved() {
        let mut r = base_row();
        r.judicial_case_status_key = Some("resolved".to_string());
        r.judicial_post_moderation_case_id = Some(1);
        assert!(!can_open_post_case(&r, 99, false, true));
    }

    #[test]
    fn escalate_moderation_party_only() {
        let mut r = base_row();
        r.judicial_case_status_key = Some("resolved".to_string());
        r.judicial_post_moderation_case_id = Some(1);
        r.post_created_by = Some(42);
        assert!(!can_escalate_to_council(&r, 42, false, true));
        r.judicial_viewer_is_moderation_party = 1;
        assert!(can_escalate_to_council(&r, 42, false, true));
    }

    #[test]
    fn open_case_on_regular_post_with_stale_open_case_status() {
        let mut r = base_row();
        r.post_status_key = Some("verified".to_string());
        r.judicial_case_status_key = Some("open".to_string());
        assert!(can_open_post_case(&r, 99, false, true));
    }

    #[test]
    fn active_workflow_blocks_open_case() {
        let mut r = base_row();
        r.post_status_key = Some("under_judicial_review".to_string());
        r.judicial_case_status_key = Some("jury_active".to_string());
        assert!(!can_open_post_case(&r, 99, false, true));
    }

    #[test]
    fn removed_human_when_flagged_and_case_resolved() {
        let mut r = base_row();
        r.post_status_key = Some("flagged".to_string());
        r.judicial_case_status_key = Some("resolved".to_string());
        assert_eq!(post_verdict_feed_mode(&r), PostVerdictFeedMode::RemovedHuman);
    }

    #[test]
    fn removed_no_jury_when_flagged_and_case_no_jury() {
        let mut r = base_row();
        r.post_status_key = Some("flagged".to_string());
        r.judicial_case_status_key = Some("no_jury".to_string());
        assert_eq!(post_verdict_feed_mode(&r), PostVerdictFeedMode::RemovedNoJury);
    }

    #[test]
    fn removed_council_takes_precedence_over_no_jury_when_appeal_resolved() {
        let mut r = base_row();
        r.post_status_key = Some("flagged".to_string());
        r.judicial_case_status_key = Some("no_jury".to_string());
        r.judicial_appeal_case_status_key = Some("resolved".to_string());
        assert_eq!(post_verdict_feed_mode(&r), PostVerdictFeedMode::RemovedCouncil);
    }

    #[test]
    fn removed_stub_uses_structured_verdict_notice() {
        let mut r = base_row();
        r.post_status_key = Some("flagged".to_string());
        r.judicial_case_status_key = Some("resolved".to_string());
        r.body_plain = "Secret quarantined body text".to_string();
        let c = i18n::catalog();
        let html = super::render_removed_feed_card(
            &r,
            PostVerdictFeedMode::RemovedHuman,
            &super::FeedCardOptions {
                viewer_user_id: 1,
                viewer_human_entity_id: Some(1),
                show_judicial_ctas: false,
                show_engagement_bar: false,
                viewer_country_code: None,
                preview_image_allowed: true,
                og_preview_reveal_label: "Show preview".to_string(),
                og_preview_withheld_hint: None,
                public_base_url: None,
            },
            "en",
            &c,
        );
        assert!(html.contains("post-verdict-notice-author"));
        assert!(html.contains("@256"));
        assert!(html.contains("post-verdict-notice-body"));
        assert!(!html.contains("public-post-removed-notice"));
        assert!(!html.contains(r.body_plain.as_str()));
    }

    #[test]
    fn emergency_contest_lost_suppresses_engagement_for_author() {
        let mut r = base_row();
        r.post_created_by = Some(42);
        r.post_status_key = Some("suppressed".to_string());
        r.judicial_trigger_key = Some("emergency_safety".to_string());
        r.judicial_case_status_key = Some("resolved".to_string());
        r.has_content_link = 1;
        r.link_href = Some("https://example.test/article".to_string());
        r.og_title = Some("Example".to_string());
        let c = i18n::catalog();
        let html = super::render_authenticated_feed_card(
            &r,
            &super::FeedCardOptions {
                viewer_user_id: 42,
                viewer_human_entity_id: Some(1),
                show_judicial_ctas: false,
                show_engagement_bar: true,
                viewer_country_code: None,
                preview_image_allowed: true,
                og_preview_reveal_label: "Show preview".to_string(),
                og_preview_withheld_hint: None,
                public_base_url: None,
            },
            &c,
            "en",
        );
        assert!(html.contains("data-judicial-feed-suppress=\"1\""));
        assert!(html.contains("post-verdict-notice-author"));
        assert!(!html.contains("public-post-og"));
        assert!(!html.contains("post-engagement-bar"));
    }

    #[test]
    fn emergency_standing_applied_for_author_after_settlement() {
        let mut r = base_row();
        r.post_created_by = Some(42);
        r.post_status_key = Some("suppressed".to_string());
        r.judicial_trigger_key = Some("emergency_safety".to_string());
        r.judicial_case_status_key = Some("contained".to_string());
        r.judicial_standing_applied = 1;
        assert_eq!(
            emergency_author_verdict_mode(&r, 42),
            Some(PostVerdictFeedMode::EmergencyStandingApplied)
        );
        assert!(emergency_author_verdict_mode(&r, 99).is_none());
    }

    #[test]
    fn removed_council_when_flagged_after_appeal_resolved() {
        let mut r = base_row();
        r.post_status_key = Some("flagged".to_string());
        r.judicial_appeal_case_status_key = Some("resolved".to_string());
        assert_eq!(post_verdict_feed_mode(&r), PostVerdictFeedMode::RemovedCouncil);
    }

    #[test]
    fn exonerated_human_after_jury_clears_post() {
        let mut r = base_row();
        r.post_status_key = Some("verified".to_string());
        r.judicial_case_status_key = Some("resolved".to_string());
        r.judicial_post_moderation_case_id = Some(1);
        assert_eq!(post_verdict_feed_mode(&r), PostVerdictFeedMode::ExoneratedHuman);
    }

    #[test]
    fn exonerated_council_after_appeal_clears_post() {
        let mut r = base_row();
        r.post_status_key = Some("verified".to_string());
        r.judicial_appeal_case_status_key = Some("resolved".to_string());
        assert_eq!(post_verdict_feed_mode(&r), PostVerdictFeedMode::ExoneratedCouncil);
    }

    #[test]
    fn author_bar_hidden_until_peer_vote() {
        let mut r = base_row();
        r.post_created_by = Some(42);
        r.non_author_root_vote_count = 0;
        assert!(engagement_bar_html(&r, 42, "en", &i18n::catalog()).is_empty());

        r.non_author_root_vote_count = 1;
        let html = engagement_bar_html(&r, 42, "en", &i18n::catalog());
        assert!(html.contains("post-engagement-bar--voted"));
        assert!(!html.contains("post-engagement-vote-btn"));
    }

    #[test]
    fn non_author_still_sees_pre_vote_bar() {
        let r = base_row();
        let html = engagement_bar_html(&r, 99, "en", &i18n::catalog());
        assert!(html.contains("post-engagement-bar--pre-vote"));
        assert!(html.contains("post-engagement-vote-btn"));
    }

    #[test]
    fn withheld_og_preview_renders_reveal_control() {
        let html = super::og_html_with_preview(
            Some("Title"),
            Some("https://cdn.example.test/x.jpg"),
            Some("unknown-preview.test"),
            Some("https://unknown-preview.test/page"),
            false,
            "Show preview",
            Some("hidden"),
        );
        assert!(html.contains("public-post-og--image-withheld"));
        assert!(html.contains("public-post-og-reveal-btn"));
        assert!(!html.contains("<img class=\"public-post-og-image\""));
    }

    #[test]
    fn platform_help_link_in_body_skips_og_preview() {
        let mut r = base_row();
        r.body_markup = r#"<p><a href="https://beta.twofivesix.app/login?legal=how">How it works</a></p>"#.to_string();
        let c = i18n::catalog();
        let html = super::render_authenticated_feed_card(
            &r,
            &super::FeedCardOptions {
                viewer_user_id: 1,
                viewer_human_entity_id: Some(1),
                show_judicial_ctas: false,
                show_engagement_bar: false,
                viewer_country_code: None,
                preview_image_allowed: true,
                og_preview_reveal_label: "Show preview".to_string(),
                og_preview_withheld_hint: None,
                public_base_url: Some("https://beta.twofivesix.app".to_string()),
            },
            &c,
            "en",
        );
        assert!(!html.contains("public-post-og"));
    }

    fn base_row() -> HomeFeedRow {
        HomeFeedRow {
            id: 1,
            post_entity_id: 1,
            handle: "test".to_string(),
            display_name: None,
            avatar_path: None,
            entity_country_code: None,
            post_country_code: None,
            body_plain: String::new(),
            body_markup: String::new(),
            body_html_sanitized: String::new(),
            og_title: None,
            og_description: None,
            og_image_url: None,
            og_site_name: None,
            link_href: None,
            is_public: 0,
            post_status_key: None,
            judicial_case_status_key: None,
            judicial_trigger_key: None,
            judicial_standing_applied: 0,
            judicial_post_moderation_case_id: None,
            judicial_moderation_appeal_count: 0,
            judicial_appeal_case_id: None,
            judicial_appeal_case_status_key: None,
            judicial_viewer_is_moderation_party: 0,
            entity_type_key: Some("human".to_string()),
            entity_status_key: None,
            entity_created_by: 1,
            entity_rating: rust_decimal::Decimal::new(250, 2),
            sort_modified_at: chrono::Utc::now(),
            published_at: chrono::Utc::now(),
            post_created_by: Some(1),
            audience_for_label: None,
            post_engagement_rating: rust_decimal::Decimal::new(250, 2),
            engagement_comment_count: 0,
            viewer_engagement_vote: None,
            non_author_root_vote_count: 0,
            has_content_link: 0,
            content_link_id: None,
            domain_id: None,
        }
    }
}
