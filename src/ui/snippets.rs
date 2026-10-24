//! Static visual markup. Keep each symbol in one place; wire into shells with `replace(...)`.
//! Brand smiley geometry: `static/icons/brand-*-markup.svg` — see `docs/ICONOGRAPHY.md`.

use crate::html_escape::escape_html;
use crate::i18n;
use crate::ui::nom_icon;

/// Placeholder in full-page HTML templates; replaced with [`SITE_FAVICON_LINK_HTML`].
pub const SITE_FAVICON_PLACEHOLDER: &str = "__SITE_FAVICON__";

/// Site favicon link tags (256 smiley mark).
pub const SITE_FAVICON_LINK_HTML: &str =
    r#"<link rel="icon" href="/static/branding/favicon.svg" type="image/svg+xml"><link rel="shortcut icon" href="/static/branding/favicon.svg">"#;

/// Inject favicon `<link>` tags into a page shell that contains [`SITE_FAVICON_PLACEHOLDER`].
#[must_use]
pub fn inject_site_favicon(html: &str) -> String {
    html.replace(SITE_FAVICON_PLACEHOLDER, SITE_FAVICON_LINK_HTML)
}

/// Placeholder in full-page HTML templates; replaced with [`site_seo_meta_html`].
pub const SITE_SEO_PLACEHOLDER: &str = "__SITE_SEO__";

/// Static path for the default Open Graph image (AI Council 4×3 avatar grid).
pub const SITE_OG_IMAGE_PATH: &str = "/static/branding/council-grid-og.png";

/// Absolute URL for [`SITE_OG_IMAGE_PATH`] on the public site origin.
#[must_use]
pub fn site_og_image_url(public_base_url: &str) -> String {
    format!(
        "{}{}",
        public_base_url.trim_end_matches('/'),
        SITE_OG_IMAGE_PATH
    )
}

/// Open Graph / Twitter image tags for the council grid asset.
#[must_use]
pub fn site_og_image_meta_tags(public_base_url: &str) -> String {
    let og_image = site_og_image_url(public_base_url);
    format!(
        r#"<meta property="og:image" content="{img}"><meta property="og:image:alt" content="The AI Council — twelve members in a four-by-three portrait grid"><meta name="twitter:image" content="{img}">"#,
        img = escape_html(&og_image),
    )
}

/// Standard SEO + Open Graph + Twitter meta block for guest marketing shells.
#[must_use]
pub fn site_seo_meta_html(
    public_base_url: &str,
    canonical_url: &str,
    title: &str,
    description: &str,
    og_type: &str,
) -> String {
    let og_image = site_og_image_url(public_base_url);
    format!(
        concat!(
            r#"<meta name="description" content="{desc}">"#,
            r#"<link rel="canonical" href="{canonical}">"#,
            r#"<meta property="og:type" content="{og_type}">"#,
            r#"<meta property="og:site_name" content="256.app">"#,
            r#"<meta property="og:title" content="{title}">"#,
            r#"<meta property="og:description" content="{desc}">"#,
            r#"<meta property="og:url" content="{canonical}">"#,
            r#"<meta property="og:image" content="{og_image}">"#,
            r#"<meta property="og:image:alt" content="The AI Council — twelve members in a four-by-three portrait grid">"#,
            r#"<meta name="twitter:card" content="summary_large_image">"#,
            r#"<meta name="twitter:title" content="{title}">"#,
            r#"<meta name="twitter:description" content="{desc}">"#,
            r#"<meta name="twitter:image" content="{og_image}">"#,
        ),
        desc = escape_html(description),
        canonical = escape_html(canonical_url),
        og_type = escape_html(og_type),
        title = escape_html(title),
        og_image = escape_html(&og_image),
    )
}

/// Inject [`site_seo_meta_html`] output into templates that declare [`SITE_SEO_PLACEHOLDER`].
#[must_use]
pub fn inject_site_seo(html: &str, seo_html: &str) -> String {
    html.replace(SITE_SEO_PLACEHOLDER, seo_html)
}
const BRAND_SMILEY_MARKUP: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/static/icons/brand-smiley-markup.svg"));
const BRAND_ROBOT_MARKUP: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/static/icons/brand-robot-markup.svg"));
const BRAND_LOADER_MARKUP: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/static/icons/brand-loader-markup.svg"));

const BRAND_SPECTRUM_STOPS: &str = r#"<stop offset="0%" style="stop-color:var(--c-red)" /><stop offset="16%" style="stop-color:var(--c-yellow)" /><stop offset="32%" style="stop-color:var(--c-green)" /><stop offset="48%" style="stop-color:var(--c-cyan)" /><stop offset="64%" style="stop-color:var(--c-blue)" /><stop offset="80%" style="stop-color:var(--c-magenta)" /><stop offset="100%" style="stop-color:var(--c-red)" />"#;

fn brand_spectrum_gradient_def(grad_id: &str) -> String {
    format!(
        r#"<defs><linearGradient id="{grad_id}" x1="0%" y1="0%" x2="100%" y2="100%">{stops}</linearGradient></defs>"#,
        grad_id = escape_html(grad_id),
        stops = BRAND_SPECTRUM_STOPS,
    )
}

fn brand_smiley_svg(extra_class: &str) -> String {
    format!(
        r#"<svg class="brand-hero-svg {extra_class}" viewBox="0 0 256 256" role="img" aria-hidden="true" xmlns="http://www.w3.org/2000/svg">{markup}</svg>"#,
        extra_class = extra_class,
        markup = BRAND_SMILEY_MARKUP,
    )
}

fn brand_robot_svg(grad_id: &str, extra_class: &str) -> String {
    format!(
        r#"<svg class="brand-hero-svg brand-hero-svg--robot {extra_class}" viewBox="0 0 256 256" role="img" aria-hidden="true" xmlns="http://www.w3.org/2000/svg">{defs}{markup}</svg>"#,
        extra_class = extra_class,
        defs = brand_spectrum_gradient_def(grad_id),
        markup = BRAND_ROBOT_MARKUP,
    )
}

/// Smiley for the app top bar (`--icon-size-lg` via CSS).
#[must_use]
pub fn toolbar_smiley() -> String {
    format!(
        r#"<span class="brand-frame brand-frame--toolbar">{}</span>"#,
        brand_smiley_svg("brand-hero-svg--toolbar"),
    )
}

/// Robot smiley for AI-assisted surfaces — toolbar size.
#[must_use]
pub fn toolbar_robot_smiley() -> String {
    format!(
        r#"<span class="brand-frame brand-frame--toolbar">{}</span>"#,
        brand_robot_svg("brand-robot-spectrum-toolbar", "brand-hero-svg--toolbar"),
    )
}

/// Large smiley for marketing / brand columns.
#[must_use]
pub fn brand_panel_smiley() -> String {
    format!(
        r#"<div class="brand-frame">{}</div>"#,
        brand_smiley_svg("brand-hero"),
    )
}

/// Large robot smiley for AI-marked brand columns.
#[must_use]
pub fn brand_panel_robot_smiley() -> String {
    format!(
        r#"<div class="brand-frame">{}</div>"#,
        brand_robot_svg("brand-robot-spectrum", "brand-hero"),
    )
}

/// Centered async loader: thin spectrum frame (`.brand-frame--loader`); whole smiley spins.
#[must_use]
pub fn card_loader(_gradient_id_suffix: &str) -> String {
    format!(
        r#"<div class="card-loader"><span class="brand-frame brand-frame--loader"><svg class="brand-loader-svg" viewBox="0 0 256 256" role="img" aria-hidden="true" xmlns="http://www.w3.org/2000/svg"><g class="brand-loader-spin">{markup}</g></svg></span></div>"#,
        markup = BRAND_LOADER_MARKUP,
    )
}

#[must_use]
pub fn avatar_not_found_html() -> &'static str {
    r#"<span class="app-avatar-not-found" aria-hidden="true"><svg viewBox="0 0 40 40" width="20" height="20" xmlns="http://www.w3.org/2000/svg"><circle cx="20" cy="20" r="19" fill="rgba(3,4,8,0.94)"/><path d="M20 28.8a1.35 1.35 0 1 1 0 2.7 1.35 1.35 0 0 1 0-2.7zm0-20.4c-3.55 0-6.1 2.05-6.1 5.25a1.25 1.25 0 0 0 2.5 0c0-1.68 1.36-2.75 3.6-2.75 2.31 0 3.72 1.02 3.72 2.62 0 1.11-.7 1.93-2.31 3.11-2.19 1.61-3.16 2.95-3.16 5.26v1.13a1.25 1.25 0 1 0 2.5 0v-1.13c0-1.36.48-2.07 2.14-3.29 2.05-1.5 3.33-2.82 3.33-5.08 0-3.15-2.66-5.12-6.22-5.12z" fill="var(--text)"/></svg></span>"#
}

/// Maps `social_entity.rating` (0–5) to full/half star glyphs. At **0**, the wrapper is still emitted
/// but contains no stars, so the avatar strip often looks empty — that is intentional unless callers add outlines.
#[must_use]
pub fn ssr_stars_html(rating: f32, class_name: &str) -> String {
    let clamped = rating.clamp(0.0, 5.0);
    let rounded_half_steps = (clamped * 2.0).round() as u8;
    let full = (rounded_half_steps / 2) as usize;
    let has_half = rounded_half_steps % 2 == 1;
    let mut out = String::new();
    out.push_str(&format!(
        r#"<span class="{}" aria-label="SSR {:.1} out of 5">"#,
        escape_html(class_name),
        clamped
    ));
    for _ in 0..full {
        out.push_str(r#"<span class="ssr-star ssr-star--full" aria-hidden="true">★</span>"#);
    }
    if has_half {
        out.push_str(r#"<span class="ssr-star ssr-star--half" aria-hidden="true">☆</span>"#);
    }
    out.push_str("</span>");
    out
}

#[must_use]
pub fn topbar_avatar_html(
    face_path: Option<&str>,
    profile_trigger: bool,
    aria_label: &str,
    ssr_rating: Option<f32>,
    // When Some (full verification), avatar links to the private profile shell instead of toggling onboarding.
    private_profile_href: Option<&str>,
) -> String {
    let aria_esc = escape_html(aria_label);
    let avatar_inner = if let Some(path) = face_path {
        let src = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        };
        let src_esc = escape_html(&src);
        format!(
            r#"<img class="guest-council-avatar-img" src="{src_esc}" alt="" width="64" height="64" loading="lazy" decoding="async" onerror="window.__avatarImgFallback&&window.__avatarImgFallback(this)">"#
        )
    } else {
        avatar_not_found_html().to_string()
    };
    let stars_html = ssr_rating
        .map(|r| ssr_stars_html(r, "ssr-stars ssr-stars--avatar"))
        .unwrap_or_default();
    let avatar_stack_html = format!(
        r#"<span class="avatar-stack">{avatar}{stars}</span>"#,
        avatar = avatar_inner,
        stars = stars_html
    );
    if let Some(href) = private_profile_href {
        let href_esc = escape_html(href);
        return format!(
            r#"<a class="app-topbar-avatar-btn" href="{href_esc}" aria-label="{aria_esc}"><span class="guest-council-avatar-frame brand-frame brand-frame--toolbar">{avatar_inner}</span></a>"#,
            avatar_inner = avatar_stack_html,
        );
    }
    if profile_trigger {
        format!(
            r#"<button type="button" class="app-topbar-avatar-btn" id="app-topbar-avatar-btn" aria-label="{aria_esc}"><span class="guest-council-avatar-frame brand-frame brand-frame--toolbar">{avatar_inner}</span></button>"#,
            avatar_inner = avatar_stack_html,
        )
    } else {
        format!(
            r#"<span class="app-topbar-avatar-static" aria-hidden="true"><span class="guest-council-avatar-frame brand-frame brand-frame--toolbar">{avatar_inner}</span></span>"#,
            avatar_inner = avatar_stack_html,
        )
    }
}

/// Visible when the viewer has a pending jury summons; links to home feed.
#[must_use]
pub fn topbar_juror_summons_badge_html(label: &str) -> String {
    format!(
        r#"<a class="app-topbar-juror-badge" href="/home" data-juror-summons-badge="1" aria-label="{label}"><span class="app-topbar-juror-badge-dot" aria-hidden="true"></span><span class="app-topbar-juror-badge-text">{label}</span></a>"#,
        label = escape_html(label),
    )
}

#[must_use]
pub fn topbar_ai_tokens_html(ai_tokens: u16, aria_label: &str, currency_tooltip: &str) -> String {
    nom_icon::topbar_wallet_html(ai_tokens, currency_tooltip, aria_label)
}

#[must_use]
pub fn topbar_create_post_html(aria_label: &str) -> String {
    let aria_esc = escape_html(aria_label);
    format!(
        r#"<button type="button" class="app-topbar-create-btn" id="app-topbar-create-btn" aria-label="{aria}"><span class="brand-frame brand-frame--toolbar app-topbar-create-frame" aria-hidden="true"><svg class="app-topbar-create-icon" viewBox="0 0 40 40" width="40" height="40" xmlns="http://www.w3.org/2000/svg"><defs><linearGradient id="app-topbar-create-plus-spectrum" x1="0%" y1="0%" x2="100%" y2="100%"><stop offset="0%" style="stop-color:var(--c-red)" /><stop offset="20%" style="stop-color:var(--c-yellow)" /><stop offset="40%" style="stop-color:var(--c-green)" /><stop offset="60%" style="stop-color:var(--c-cyan)" /><stop offset="80%" style="stop-color:var(--c-blue)" /><stop offset="100%" style="stop-color:var(--c-magenta)" /></linearGradient></defs><circle class="app-topbar-create-ring" cx="20" cy="20" r="18"/><rect class="app-topbar-create-plus-v" x="17.2" y="10.8" width="5.6" height="18.4" rx="2.8" fill="url(#app-topbar-create-plus-spectrum)"/><rect class="app-topbar-create-plus-h" x="10.8" y="17.2" width="18.4" height="5.6" rx="2.8" fill="url(#app-topbar-create-plus-spectrum)"/></svg></span></button>"#,
        aria = aria_esc
    )
}

#[must_use]
pub fn topbar_run_jobs_html(enabled: bool, label: &str) -> String {
    if !enabled {
        return String::new();
    }
    format!(
        r#"<button type="button" class="app-topbar-run-jobs-btn" id="app-topbar-run-jobs-btn">{}</button>"#,
        escape_html(label)
    )
}

/// Connection slot meter in the primary top bar (`12/256` or `256` at capacity).
#[must_use]
pub fn topbar_connection_meter_html(meter: &str, aria_label: &str) -> String {
    format!(
        r#"<span class="app-topbar-connection-meter mono" aria-label="{aria}">{meter}</span>"#,
        aria = escape_html(aria_label),
        meter = escape_html(meter),
    )
}

#[must_use]
pub fn secured_topbar_html(
    leading_html: &str,
    run_jobs_html: &str,
    ai_tokens_html: &str,
    create_html: &str,
    avatar_html: &str,
    logo_home_aria: &str,
) -> String {
    format!(
        r#"<header class="app-topbar" role="banner"><div class="app-topbar-logo-cell"><a class="app-logo-link" href="/home" aria-label="{logo_aria}">{logo}</a></div><div class="app-topbar-rest">{leading}{run_jobs}{tokens}{avatar}</div><div class="app-topbar-action-cell">{create}</div></header>"#,
        logo_aria = escape_html(logo_home_aria),
        logo = toolbar_smiley(),
        leading = leading_html,
        run_jobs = run_jobs_html,
        tokens = ai_tokens_html,
        create = create_html,
        avatar = avatar_html,
    )
}

/// Same as [`secured_topbar_html`], but uses the robot smiley for AI-assisted flows (e.g. face capture / vision).
#[must_use]
pub fn secured_topbar_html_ai(
    leading_html: &str,
    run_jobs_html: &str,
    ai_tokens_html: &str,
    create_html: &str,
    avatar_html: &str,
    logo_home_aria: &str,
) -> String {
    format!(
        r#"<header class="app-topbar" role="banner"><div class="app-topbar-logo-cell"><a class="app-logo-link" href="/home" aria-label="{logo_aria}">{logo}</a></div><div class="app-topbar-rest">{leading}{run_jobs}{tokens}{avatar}</div><div class="app-topbar-action-cell">{create}</div></header>"#,
        logo_aria = escape_html(logo_home_aria),
        logo = toolbar_robot_smiley(),
        leading = leading_html,
        run_jobs = run_jobs_html,
        tokens = ai_tokens_html,
        create = create_html,
        avatar = avatar_html,
    )
}

/// Logo and nav targets for guest/public topbars: feed home when signed in, marketing login when not.
#[must_use]
pub struct GuestTopbarChrome {
    pub logo_href: String,
    pub logo_aria: String,
    pub nav_label: String,
    pub nav_href: String,
}

#[must_use]
pub fn guest_topbar_chrome(loc: &str, signed_in: bool) -> GuestTopbarChrome {
    let c = i18n::catalog();
    if signed_in {
        GuestTopbarChrome {
            logo_href: "/home".to_string(),
            logo_aria: c.t(loc, "ui.home.logo_aria_home"),
            nav_label: c.t(loc, "ui.shared.home"),
            nav_href: "/home".to_string(),
        }
    } else {
        GuestTopbarChrome {
            logo_href: "/login".to_string(),
            logo_aria: c.t(loc, "ui.login.logo_aria"),
            nav_label: c.t(loc, "ui.login.choice_log_in"),
            nav_href: "/login?login=1".to_string(),
        }
    }
}

impl GuestTopbarChrome {
    #[must_use]
    pub fn public_topbar(&self, center_html: &str) -> String {
        public_topbar_html(
            center_html,
            &self.logo_href,
            &self.logo_aria,
            &self.nav_label,
            &self.nav_href,
        )
    }
}

const GUEST_PANTHEON_CARD_FRAGMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/guest_pantheon_card_fragment.html"
));
const GUEST_MARKETING_FEATURES_SPRITE_FRAGMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/guest_marketing_features_sprite_fragment.html"
));
const GUEST_WELCOME_BENEFITS_GRID_FRAGMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/guest_welcome_benefits_grid_fragment.html"
));
const GUEST_PUBLIC_JOIN_CARD_FRAGMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/guest_public_join_card_fragment.html"
));

/// Shared SVG sprite for guest marketing benefit icons (once per shell).
#[must_use]
pub fn guest_marketing_sprite_html() -> String {
    GUEST_MARKETING_FEATURES_SPRITE_FRAGMENT.to_string()
}

fn guest_benefits_grid_html(loc: &str) -> String {
    i18n::subst_markers(GUEST_WELCOME_BENEFITS_GRID_FRAGMENT, loc)
}

/// Feature tiles below the council pantheon on guest shells.
#[must_use]
pub fn guest_welcome_benefits_html(loc: &str) -> String {
    format!(
        r#"<aside class="guest-welcome-benefits">{grid}</aside>"#,
        grid = guest_benefits_grid_html(loc),
    )
}

/// Council pantheon card markup.
#[must_use]
pub fn guest_pantheon_card_html(loc: &str) -> String {
    i18n::subst_markers(GUEST_PANTHEON_CARD_FRAGMENT, loc)
}

/// Council pantheon, then feature tiles (login / public guest marketing stack).
#[must_use]
pub fn guest_welcome_marketing_html(loc: &str) -> String {
    let pantheon = guest_pantheon_card_html(loc);
    let benefits = guest_welcome_benefits_html(loc);
    format!(
        r#"<div class="guest-welcome-marketing" id="guest-welcome-marketing">{pantheon}{benefits}</div>"#,
        pantheon = pantheon,
        benefits = benefits,
    )
}

/// Public entity profile: join + pantheon only (profile and benefits follow in the shell).
#[must_use]
pub fn guest_public_entity_stack_top_html(loc: &str) -> String {
    let join = i18n::subst_markers(GUEST_PUBLIC_JOIN_CARD_FRAGMENT, loc);
    let pantheon = guest_pantheon_card_html(loc);
    format!(
        r#"<div class="guest-public-stack-top">{join}<div class="guest-welcome-marketing" id="guest-welcome-marketing">{pantheon}</div></div>"#,
        join = join,
        pantheon = pantheon,
    )
}

/// Public entity profile: benefit tiles below the profile card.
#[must_use]
pub fn guest_public_benefits_footer_html(loc: &str) -> String {
    format!(
        r#"<div class="guest-public-benefits-footer" id="guest-public-benefits">{benefits}</div>"#,
        benefits = guest_welcome_benefits_html(loc),
    )
}

/// Left column for public post shells: join CTA + full marketing stack.
#[must_use]
pub fn guest_public_left_column_html(loc: &str) -> String {
    let join = i18n::subst_markers(GUEST_PUBLIC_JOIN_CARD_FRAGMENT, loc);
    let marketing = guest_welcome_marketing_html(loc);
    format!(
        r#"<div class="guest-left-panel">{join}{marketing}</div>"#,
        join = join,
        marketing = marketing,
    )
}

#[must_use]
pub fn public_topbar_html(
    center_html: &str,
    logo_href: &str,
    logo_home_aria: &str,
    login_label: &str,
    login_href: &str,
) -> String {
    let login_link = if login_label.trim().is_empty() || login_href.trim().is_empty() {
        String::new()
    } else {
        format!(
            r#"<a class="app-topbar-logout-btn app-topbar-login-link" href="{login_href}">{login}</a>"#,
            login_href = escape_html(login_href),
            login = escape_html(login_label),
        )
    };
    format!(
        r#"<header class="app-topbar" role="banner"><div class="app-topbar-logo-cell"><a class="app-logo-link" href="{logo_href}" aria-label="{logo_aria}">{logo}</a></div><div class="app-topbar-rest">{center}{login_link}</div></header>"#,
        logo_href = escape_html(logo_href),
        logo_aria = escape_html(logo_home_aria),
        logo = toolbar_smiley(),
        center = center_html,
        login_link = login_link,
    )
}

/// Small globe icon for private-post heading/badge usage.
/// Centered on the north pole to avoid any regional preference.
#[must_use]
pub fn private_post_globe_icon() -> &'static str {
    r#"<span class="private-post-globe-icon" aria-hidden="true"><svg viewBox="0 0 40 40" width="20" height="20" xmlns="http://www.w3.org/2000/svg"><defs><linearGradient id="private-post-globe-spectrum" x1="0%" y1="0%" x2="100%" y2="100%"><stop offset="0%" style="stop-color:var(--c-red)" /><stop offset="20%" style="stop-color:var(--c-yellow)" /><stop offset="40%" style="stop-color:var(--c-green)" /><stop offset="60%" style="stop-color:var(--c-cyan)" /><stop offset="80%" style="stop-color:var(--c-blue)" /><stop offset="100%" style="stop-color:var(--c-magenta)" /></linearGradient></defs><circle cx="20" cy="20" r="17" fill="none" stroke="url(#private-post-globe-spectrum)" stroke-width="2.4"/><circle cx="20" cy="7.2" r="1.7" fill="var(--c-yellow)"/><ellipse cx="20" cy="20" rx="9.2" ry="17" fill="none" stroke="var(--c-cyan)" stroke-width="1.4"/><ellipse cx="20" cy="20" rx="5.2" ry="17" fill="none" stroke="var(--c-blue)" stroke-width="1.2"/><ellipse cx="20" cy="20" rx="17" ry="6.2" fill="none" stroke="var(--c-green)" stroke-width="1.2"/><ellipse cx="20" cy="20" rx="17" ry="11" fill="none" stroke="var(--c-magenta)" stroke-width="1.1" opacity="0.9"/><path d="M20 6.2V33.8" stroke="var(--c-yellow)" stroke-width="1.1" stroke-linecap="round"/></svg></span>"#
}

/// Thumb **up** (gesture): unrotated hand model reads as thumb-up in the UI.
#[must_use]
pub fn brand_vote_thumb_up_svg(gradient_id_suffix: &str) -> String {
    let safe: String = gradient_id_suffix
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    let gid = format!("brand-vote-up-fill-{safe}");
    format!(
        r##"<span class="brand-frame brand-frame--toolbar brand-vote-thumb-frame" aria-hidden="true"><svg class="brand-vote-thumb-svg brand-vote-thumb-svg--up" viewBox="0 0 40 40" width="40" height="40" xmlns="http://www.w3.org/2000/svg"><defs><linearGradient id="{gid}" x1="12%" y1="8%" x2="88%" y2="92%"><stop offset="0%" stop-color="#fff6b0"/><stop offset="42%" stop-color="var(--c-yellow)"/><stop offset="100%" stop-color="#c9a026"/></linearGradient></defs><g stroke="#141822" stroke-width="1.12" stroke-linejoin="round" stroke-linecap="round"><path fill="url(#{gid})" d="M8.4 31.4V21.2c0-2.05 1.5-3.55 3.55-3.55h3.05l1.2-7.95c.26-1.72 1.88-2.88 3.62-2.65 1.48.18 2.58 1.42 2.58 2.88v2.02h7.05c2.32 0 4.2 1.88 4.2 4.2v9.05c0 2.32-1.88 4.2-4.2 4.2H12.2c-2.05 0-3.8-1.75-3.8-3.8z"/><path fill="none" stroke-width="1.02" d="M15.1 17.2c.42-3.45 2.95-6.1 5.75-7.9.9-.55 1.98.02 2.08 1.05l.52 4.2"/></g><circle cx="15.1" cy="23.5" r="1.32" fill="#141822"/><circle cx="20.05" cy="23.05" r="1.32" fill="#141822"/><path d="M15.5 27.35q3.05 2.55 5.9 1.35" fill="none" stroke="#141822" stroke-width="1.05" stroke-linecap="round"/></svg></span>"##,
        gid = escape_html(&gid)
    )
}

/// Thumb **down** (gesture): rotated hand model reads as thumb-down in the UI.
#[must_use]
pub fn brand_vote_thumb_down_svg(gradient_id_suffix: &str) -> String {
    let safe: String = gradient_id_suffix
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    let gid = format!("brand-vote-down-fill-{safe}");
    format!(
        r##"<span class="brand-frame brand-frame--toolbar brand-vote-thumb-frame" aria-hidden="true"><svg class="brand-vote-thumb-svg brand-vote-thumb-svg--down" viewBox="0 0 40 40" width="40" height="40" xmlns="http://www.w3.org/2000/svg"><defs><linearGradient id="{gid}" x1="88%" y1="8%" x2="12%" y2="92%"><stop offset="0%" stop-color="#ffe0c2"/><stop offset="50%" stop-color="#e8a040"/><stop offset="100%" stop-color="#a85a28"/></linearGradient></defs><g transform="rotate(180 20 20)"><g stroke="#141822" stroke-width="1.12" stroke-linejoin="round" stroke-linecap="round"><path fill="url(#{gid})" d="M8.4 31.4V21.2c0-2.05 1.5-3.55 3.55-3.55h3.05l1.2-7.95c.26-1.72 1.88-2.88 3.62-2.65 1.48.18 2.58 1.42 2.58 2.88v2.02h7.05c2.32 0 4.2 1.88 4.2 4.2v9.05c0 2.32-1.88 4.2-4.2 4.2H12.2c-2.05 0-3.8-1.75-3.8-3.8z"/><path fill="none" stroke-width="1.02" d="M15.1 17.2c.42-3.45 2.95-6.1 5.75-7.9.9-.55 1.98.02 2.08 1.05l.52 4.2"/></g><circle cx="15.1" cy="23.5" r="1.32" fill="#141822"/><circle cx="20.05" cy="23.05" r="1.32" fill="#141822"/><path d="M15.5 27.35q3.05 2.55 5.9 1.35" fill="none" stroke="#141822" stroke-width="1.05" stroke-linecap="round"/></g></svg></span>"##,
        gid = escape_html(&gid)
    )
}

/// Canonical public footer: product info and legal links (Community rules in footer, not on landing hero).
#[must_use]
pub fn public_footer_html() -> &'static str {
    r#"<footer class="public-footer" role="contentinfo" aria-label="Legal links">
    <a href="/login?legal=how" data-auth-legal="how">How it works</a>
    <a href="/login?legal=safety" data-auth-legal="safety">Safety &amp; Crisis</a>
    <a href="/login?legal=constitution" data-auth-legal="constitution">Community rules</a>
    <a href="/login?legal=privacy" data-auth-legal="privacy">Privacy</a>
    <a href="/login?legal=terms" data-auth-legal="terms">Terms</a>
  </footer>"#
}

/// Secured app shells use the same footer as guest/public pages.
#[must_use]
pub fn secured_public_footer_html() -> &'static str {
    public_footer_html()
}

/// Injects [`public_footer_html`] into templates that declare `__PUBLIC_FOOTER__`.
#[must_use]
pub fn inject_public_footer(html: &str) -> String {
    inject_site_favicon(&html.replace("__PUBLIC_FOOTER__", public_footer_html()))
}

/// Inline i18n + deferred script for Nom purchase (secured shells).
#[must_use]
pub fn nom_purchase_extras_html(c: &crate::i18n::Catalog, loc: &str) -> String {
    let i18n = serde_json::json!({
        "noms_purchase_title": c.t(loc, "noms.purchase_title"),
        "noms_purchase_pack_line": c.t(loc, "noms.purchase_pack_line"),
        "noms_purchase_select_pack": c.t(loc, "noms.purchase_select_pack"),
        "noms_purchase_balance_line": c.t(loc, "noms.purchase_balance_line"),
        "noms_purchase_pack_requires_below": c.t(loc, "noms.purchase_pack_requires_below"),
        "noms_purchase_hint": c.t(loc, "noms.purchase_hint"),
        "noms_purchase_choose_pack_hint": c.t(loc, "noms.purchase_choose_pack_hint"),
        "noms_purchase_stripe_powered": c.t(loc, "noms.purchase_stripe_powered"),
        "noms_purchase_wallet_full": c.t(loc, "noms.purchase_wallet_full"),
        "noms_purchase_wallet_cap": c.t(loc, "noms.purchase_wallet_cap"),
        "noms_purchase_checkout_pending": c.t(loc, "noms.purchase_checkout_pending"),
        "noms_purchase_unavailable": c.t(loc, "noms.purchase_unavailable"),
        "noms_purchase_cancel": c.t(loc, "ui.shared.cancel"),
        "noms_purchase_pay": c.t(loc, "noms.purchase_pay"),
        "noms_purchase_processing": c.t(loc, "noms.purchase_processing"),
        "noms_purchase_stage_contacting_stripe": c.t(loc, "noms.purchase_stage_contacting_stripe"),
        "noms_purchase_stage_processing_payment": c.t(loc, "noms.purchase_stage_processing_payment"),
        "noms_purchase_stage_crediting_noms": c.t(loc, "noms.purchase_stage_crediting_noms"),
        "noms_purchase_success": c.t(loc, "noms.purchase_success"),
        "noms_purchase_failed": c.t(loc, "noms.purchase_failed"),
        "noms_purchase_pending": c.t(loc, "noms.purchase_pending"),
        "noms_wallet_aria": c.t(loc, "noms.wallet_aria"),
    });
    let json_esc = crate::html_escape::escape_html(&i18n.to_string());
    format!(
        r#"<script type="application/json" id="nom-purchase-i18n">{json_esc}</script><script src="/static/js/nom-purchase.js?v=20260813-stripe-appearance" defer></script>"#
    )
}

/// Injects [`secured_public_footer_html`] into templates that declare `__SECURED_PUBLIC_FOOTER__`.
#[must_use]
pub fn inject_secured_public_footer(html: &str) -> String {
    inject_site_favicon(&html.replace(
        "__SECURED_PUBLIC_FOOTER__",
        secured_public_footer_html(),
    ))
}

/// Footer + Nom purchase i18n/script (locale-aware secured shells).
#[must_use]
pub fn inject_secured_public_footer_localized(html: &str, c: &crate::i18n::Catalog, loc: &str) -> String {
    let extras = nom_purchase_extras_html(c, loc);
    let with_footer = inject_secured_public_footer(html);
    if with_footer.contains("</body>") {
        inject_site_favicon(&with_footer.replace("</body>", &format!("{extras}\n</body>")))
    } else {
        inject_site_favicon(&format!("{with_footer}{extras}"))
    }
}

/// Share link to a new audience (spectrum globe + arrow).
#[must_use]
pub fn post_share_link_icon_svg(suffix: &str) -> String {
    let safe: String = suffix
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    let gid = format!("post-share-ring-{safe}");
    format!(
        r#"<span class="brand-frame brand-frame--toolbar post-share-link-frame" aria-hidden="true"><svg viewBox="0 0 24 24" width="12" height="12" xmlns="http://www.w3.org/2000/svg"><defs><linearGradient id="{gid}" x1="0%" y1="0%" x2="100%" y2="100%"><stop offset="0%" style="stop-color:var(--c-cyan)"/><stop offset="100%" style="stop-color:var(--c-blue)"/></linearGradient></defs><circle cx="12" cy="12" r="10.5" fill="rgba(3,4,8,0.94)" stroke="url(#{gid})" stroke-width="0.9"/><path d="M8 12h6.5M13.5 9.5 16 12l-2.5 2.5" fill="none" stroke="var(--c-yellow)" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round"/></svg></span>"#,
        gid = escape_html(&gid),
    )
}

const FACE_CAPTURE_ARTIST_EMBED_FRAGMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/face_capture_artist_embed_fragment.html"
));

/// Epic (council portrait artist) public profile card under onboarding face capture.
#[must_use]
pub fn face_capture_artist_embed_html(loc: &str, artist_handle: &str, hidden_attr: &str) -> String {
    use crate::html_escape::escape_json_for_html_script;
    use crate::ref_keys;
    use crate::social::entity;
    use crate::ui::entity_type_accent;

    let c = i18n::catalog();
    let normalized = entity::normalize_handle_from_path_segment(
        artist_handle.trim().trim_start_matches('@'),
    )
    .unwrap_or_else(|| "epic".to_string());
    let at_handle = format!("@{normalized}");
    let accent = entity_type_accent::profile_card_accent_class(ref_keys::KEY_ENTITY_TYPE_DIGITAL);
    let ui_boot = serde_json::json!({
        "not_found": c.t(loc, "ui.entity_public.card_not_found"),
        "network_error": c.t(loc, "ui.shared.network_error_short"),
        "boot_invalid": c.t(loc, "ui.entity_public.boot_invalid"),
        "constitution_show_more": c.t(loc, "ui.entity_private.constitution_show_more"),
        "constitution_show_less": c.t(loc, "ui.entity_private.constitution_show_less"),
    });
    let boot = serde_json::json!({
        "card_url": format!("/api/public/entity-profile/{normalized}"),
    });
    let artist_lead = escape_html(&c.tf(
        loc,
        "ui.face_capture.artist_lead",
        &[("artist_handle", &at_handle)],
    ));
    let html = FACE_CAPTURE_ARTIST_EMBED_FRAGMENT
        .replace("__FACE_CAPTURE_ARTIST_HIDDEN_ATTR__", hidden_attr)
        .replace("__FACE_CAPTURE_ARTIST_ACCENT__", accent)
        .replace(
            "__FACE_CAPTURE_ARTIST_LOADER__",
            &card_loader("face-capture-artist"),
        )
        .replace(
            "__FACE_CAPTURE_ARTIST_UI_JSON__",
            &escape_json_for_html_script(&ui_boot.to_string()),
        )
        .replace(
            "__FACE_CAPTURE_ARTIST_BOOT_JSON__",
            &escape_json_for_html_script(&boot.to_string()),
        )
        .replace("__FACE_CAPTURE_ARTIST_LEAD__", &artist_lead);
    i18n::subst_markers(&html, loc)
}
