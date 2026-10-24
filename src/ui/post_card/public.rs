//! Logged-out public post cards (comment count only — no vote/compose).

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::html_escape::escape_html;
use crate::i18n;
use crate::ref_keys;
use crate::ui::datetime_format;
use crate::ui::entity_type_accent;
use crate::ui::snippets;

use super::feed::{first_http_url_from_text, og_html_with_preview, public_guest_engagement_bar_html};

/// Row shape for anonymous `/public/posts` surfaces.
#[derive(Debug, Clone)]
pub struct PublicGuestPostRow {
    pub id: u64,
    pub handle: String,
    pub display_name: Option<String>,
    pub avatar_path: Option<String>,
    pub entity_country_code: Option<String>,
    pub post_country_code: Option<String>,
    pub body_plain: String,
    pub body_html_sanitized: String,
    pub body_markup: String,
    pub og_title: Option<String>,
    pub og_image_url: Option<String>,
    pub og_site_name: Option<String>,
    pub link_href: Option<String>,
    pub domain_id: Option<u64>,
    pub entity_type_key: Option<String>,
    pub entity_rating: Decimal,
    pub engagement_comment_count: i64,
    pub published_at: DateTime<Utc>,
}

pub struct PublicGuestCardOptions {
    /// When true, append an “Open post” link below the card (list view).
    pub show_open_post_link: bool,
    pub preview_image_allowed: bool,
    pub og_preview_reveal_label: String,
    pub og_preview_withheld_hint: Option<String>,
}

impl Default for PublicGuestCardOptions {
    fn default() -> Self {
        Self {
            show_open_post_link: false,
            preview_image_allowed: true,
            og_preview_reveal_label: "Show preview".to_string(),
            og_preview_withheld_hint: None,
        }
    }
}

fn body_inner_html(r: &PublicGuestPostRow) -> String {
    if !r.body_html_sanitized.trim().is_empty() {
        return r.body_html_sanitized.clone();
    }
    if !r.body_markup.trim().is_empty() {
        return r.body_markup.clone();
    }
    escape_html(&r.body_plain)
}

fn body_class_suffix(r: &PublicGuestPostRow) -> &'static str {
    if !r.body_html_sanitized.trim().is_empty() || !r.body_markup.trim().is_empty() {
        ""
    } else {
        " public-post-body--plain-fallback"
    }
}

fn accent_classes(is_linked: bool, entity_type_key: Option<&str>) -> String {
    let mut out = if is_linked {
        String::from("public-post-card--linked")
    } else {
        String::from("public-post-card--text")
    };
    out.push_str(entity_type_accent::profile_card_accent_class(
        entity_type_key.unwrap_or("").trim(),
    ));
    out
}

fn entity_data_attr(entity_type_key: Option<&str>) -> String {
    let k = entity_type_key.map(str::trim).unwrap_or("");
    if k.is_empty() {
        String::new()
    } else {
        format!(r#" data-entity-type="{}""#, escape_html(k))
    }
}

/// Render a single anonymous public post card.
#[must_use]
pub fn render_public_guest_post_card(
    r: &PublicGuestPostRow,
    opts: &PublicGuestCardOptions,
    loc: &str,
    c: &i18n::Catalog,
) -> String {
    let link_url = first_http_url_from_text(&r.body_plain)
        .or_else(|| first_http_url_from_text(&r.body_html_sanitized))
        .or_else(|| first_http_url_from_text(&r.body_markup));
    let is_linked = link_url.is_some()
        || r.body_html_sanitized.contains("<a ")
        || r.body_markup.contains("<a ");
    let avatar = r
        .avatar_path
        .as_deref()
        .map(|p| {
            format!(
                r#"<img class="public-post-avatar-img" src="/{}" alt="@{}" loading="lazy" decoding="async" />"#,
                escape_html(p.trim_start_matches('/')),
                escape_html(&r.handle)
            )
        })
        .unwrap_or_else(|| snippets::avatar_not_found_html().to_string());
    let avatar_stars = if matches!(
        r.entity_type_key.as_deref(),
        Some(ref_keys::KEY_ENTITY_TYPE_COMMUNITY)
            | Some(ref_keys::KEY_ENTITY_TYPE_NON_PROFIT)
            | Some(ref_keys::KEY_ENTITY_TYPE_PROFIT)
            | Some(ref_keys::KEY_ENTITY_TYPE_GATHERING)
    ) {
        snippets::ssr_stars_html(
            r.entity_rating.to_string().parse::<f32>().unwrap_or(0.0),
            "ssr-stars ssr-stars--avatar",
        )
    } else {
        String::new()
    };
    let og_href = r
        .link_href
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| first_http_url_from_text(&r.body_plain))
        .or_else(|| first_http_url_from_text(&r.body_html_sanitized))
        .or_else(|| first_http_url_from_text(&r.body_markup));
    let og = og_html_with_preview(
        r.og_title.as_deref(),
        r.og_image_url.as_deref(),
        r.og_site_name.as_deref(),
        og_href.as_deref(),
        opts.preview_image_allowed,
        &opts.og_preview_reveal_label,
        opts.og_preview_withheld_hint.as_deref(),
    );
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
    let flag_html = if cc.is_empty() {
        String::new()
    } else {
        let cc_l = cc.to_ascii_lowercase();
        format!(
            r#"<span class="public-post-flag-icon public-post-flag-inline fi fi-{cc_l}" data-country-code="{cc}" title="Posted from {cc}" aria-hidden="true"></span>"#,
            cc = escape_html(cc),
            cc_l = escape_html(&cc_l),
        )
    };
    let handle_line = format!(r#"@{}"#, escape_html(&r.handle));
    let display_name = r
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(escape_html)
        .unwrap_or_default();
    let engagement = public_guest_engagement_bar_html(r.engagement_comment_count, loc, c);
    let link_attr = link_url
        .as_ref()
        .map(|u| format!(r#" data-link-url="{}""#, escape_html(u)))
        .unwrap_or_default();
    let footer = if opts.show_open_post_link {
        format!(
            r#"<p class="sub"><a href="/public/posts/{post_id}">Open post</a></p>"#,
            post_id = r.id
        )
    } else {
        String::new()
    };

    let handle_block = format!(
        r#"<a class="public-post-handle" href="/public/{handle}"><span class="public-post-handle-line">{handle_line}</span></a>"#,
        handle = escape_html(&r.handle),
        handle_line = handle_line,
    );
    let identity_meta = format!(
        r#"<div class="public-post-handle-block">{handle_block}<span class="public-post-name-row"><span class="public-post-display-name">{display_name}</span>{flag_html}</span></div>"#,
        handle_block = handle_block,
        display_name = display_name,
        flag_html = flag_html,
    );
    let published_at_html = datetime_format::post_published_at_html(loc, r.published_at);

    format!(
        r#"<section class="card app-card public-post-card {accent_class}"{entity_attr}{link_attr}><div class="card-accent" role="presentation"></div><div class="public-post-head"><div class="public-post-head-identity"><a class="public-post-avatar-link" href="/public/{handle}" aria-label="Open public profile @{handle}"><span class="brand-frame brand-frame--toolbar public-post-avatar"><span class="avatar-stack">{avatar}{avatar_stars}</span></span></a>{identity_meta}</div></div>{og}<div class="public-post-published-row">{published_at_html}</div><div class="{body_class}" role="article">{body}</div>{engagement}{footer}</section>"#,
        accent_class = accent_classes(is_linked, r.entity_type_key.as_deref()),
        entity_attr = entity_data_attr(r.entity_type_key.as_deref()),
        link_attr = link_attr,
        handle = escape_html(&r.handle),
        identity_meta = identity_meta,
        avatar = avatar,
        avatar_stars = avatar_stars,
        og = og,
        published_at_html = published_at_html,
        body_class = body_class,
        body = body_inner_html(r),
        engagement = engagement,
        footer = footer,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guest_card_includes_comment_count_only() {
        let row = PublicGuestPostRow {
            id: 1,
            handle: "test".into(),
            display_name: None,
            avatar_path: None,
            entity_country_code: None,
            post_country_code: None,
            body_plain: "Hello".into(),
            body_html_sanitized: String::new(),
            body_markup: String::new(),
            og_title: None,
            og_image_url: None,
            og_site_name: None,
            link_href: None,
            domain_id: None,
            entity_type_key: Some("human".into()),
            entity_rating: Decimal::from(3),
            engagement_comment_count: 4,
            published_at: chrono::Utc::now(),
        };
        let c = i18n::catalog();
        let html = render_public_guest_post_card(
            &row,
            &PublicGuestCardOptions::default(),
            "en",
            &c,
        );
        assert!(html.contains("public-post-head-identity"));
        assert!(html.contains("public-post-handle-block"));
        assert!(html.contains("post-engagement-bar--public-guest"));
        assert!(html.contains(">4<"));
        assert!(!html.contains("post-engagement-vote-btn"));
    }
}
