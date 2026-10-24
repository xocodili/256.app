use axum::extract::{Path, State};
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use axum_extra::extract::CookieJar;

use crate::app_state::AppState;
use crate::auth::session;
use crate::error::{AppError, AppResult};
use crate::html_escape::{escape_html, escape_json_for_html_script};
use crate::i18n;
use crate::ref_keys;
use crate::social::entity;
use crate::social::public_profile;
use crate::ui::entity_type_accent;
use crate::ui::post_card::{render_public_guest_post_card, PublicGuestCardOptions, PublicGuestPostRow};
use crate::ui::snippets;

/// Logged-out public HTML/API surfaces (`/public/...`).
///
/// `post_audience_scope.private` means **members-only** (no anonymous / logged-out visibility).
/// Those posts are excluded from these queries. The signed-in home feed (`social::feed`) does **not**
/// treat `private` as author-only; matched audience members still see the post there.
pub fn shell_router() -> Router<AppState> {
    Router::new()
        .route("/public/posts", get(public_posts_shell))
        .route("/public/posts/{post_id}", get(public_post_shell))
        .route("/public/{handle}", get(public_entity_shell))
}

#[derive(sqlx::FromRow)]
struct PublicPostRow {
    id: u64,
    handle: String,
    display_name: Option<String>,
    avatar_path: Option<String>,
    entity_country_code: Option<String>,
    post_country_code: Option<String>,
    body_plain: String,
    body_html_sanitized: String,
    body_markup: String,
    og_title: Option<String>,
    og_description: Option<String>,
    og_image_url: Option<String>,
    og_site_name: Option<String>,
    /// Canonical URL for the shared link row (opens OG preview).
    link_href: Option<String>,
    domain_id: Option<u64>,
    /// `ref_key_values.key` for `social_entity_type` (`human`, `digital`, ...).
    entity_type_key: Option<String>,
    entity_rating: rust_decimal::Decimal,
    engagement_comment_count: i64,
    published_at: chrono::DateTime<chrono::Utc>,
}

fn row_to_guest(r: PublicPostRow) -> PublicGuestPostRow {
    PublicGuestPostRow {
        id: r.id,
        handle: r.handle,
        display_name: r.display_name,
        avatar_path: r.avatar_path,
        entity_country_code: r.entity_country_code,
        post_country_code: r.post_country_code,
        body_plain: r.body_plain,
        body_html_sanitized: r.body_html_sanitized,
        body_markup: r.body_markup,
        og_title: r.og_title,
        og_image_url: r.og_image_url,
        og_site_name: r.og_site_name,
        link_href: r.link_href,
        domain_id: r.domain_id,
        entity_type_key: r.entity_type_key,
        entity_rating: r.entity_rating,
        engagement_comment_count: r.engagement_comment_count,
        published_at: r.published_at,
    }
}

fn trim_for_meta(input: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if out.chars().count() >= max_chars {
            break;
        }
        out.push(ch);
    }
    out.trim().to_string()
}

const PUBLIC_POST_SELECT_TAIL: &str = r#"
            rk_entity_type.`key` AS entity_type_key,
            COALESCE(se.rating, 0.00) AS entity_rating,
            __ENGAGEMENT_COMMENT_COUNT__ AS engagement_comment_count
        FROM post p
"#;

fn public_post_select_core(with_content_link: bool) -> String {
    let og_cols = if with_content_link {
        r#"cl.og_title,
            cl.og_description,
            cl.og_image_url,
            cl.og_site_name,
            cl.url_canonical AS link_href,
            COALESCE(p.domain_id, cl.domain_id) AS domain_id,"#
    } else {
        r#"NULL AS og_title,
            NULL AS og_description,
            NULL AS og_image_url,
            NULL AS og_site_name,
            NULL AS link_href,
            p.domain_id AS domain_id,"#
    };
    let link_join = if with_content_link {
        "LEFT JOIN content_link cl ON cl.id = p.content_link_id"
    } else {
        ""
    };
    format!(
        r#"
        SELECT
            p.id,
            se.handle,
            COALESCE(
                NULLIF(TRIM(sp.label), ''),
                NULLIF(TRIM(snp.label), ''),
                NULLIF(TRIM(sc.label), ''),
                NULLIF(TRIM(sg.label), ''),
                NULLIF(TRIM(sd.label), ''),
                NULLIF(TRIM(CONCAT_WS(' ', sh.given_name, sh.additional_name, sh.family_name)), '')
            ) AS display_name,
            uf.generated_artwork_path AS avatar_path,
            loc.country_code AS entity_country_code,
            p.country_code AS post_country_code,
            COALESCE(p.published_at, p.created_at) AS published_at,
            p.body_plain,
            p.body_html_sanitized,
            p.body_markup,
            {og_cols}
            {PUBLIC_POST_SELECT_TAIL}
        INNER JOIN social_entity se ON se.id = p.entity_id
        INNER JOIN post_audience_scope pas ON pas.post_id = p.id
        LEFT JOIN ref_key_values rk_post_status
            ON rk_post_status.id = p.status_id
           AND rk_post_status.`group` = ?
        LEFT JOIN ref_key_values rk_entity_type
            ON rk_entity_type.id = se.type_id
           AND rk_entity_type.`group` = ?
        LEFT JOIN user_faces uf ON uf.user_id = se.created_by
           AND uf.face_captured_at IS NOT NULL
           AND uf.generated_artwork_path IS NOT NULL
           AND uf.generated_artwork_path <> ''
        LEFT JOIN social_profit sp ON sp.social_entity_id = se.id
        LEFT JOIN social_non_profit snp ON snp.social_entity_id = se.id
        LEFT JOIN social_community sc ON sc.social_entity_id = se.id
        LEFT JOIN social_gathering sg ON sg.social_entity_id = se.id
        LEFT JOIN social_digital sd ON sd.social_entity_id = se.id
        LEFT JOIN social_human sh ON sh.social_entity_id = se.id
        LEFT JOIN locations loc ON loc.id = se.location_id
        {link_join}
        WHERE
            pas.scope_key = 'public'
            AND COALESCE(rk_post_status.`key`, '') <> ?
            AND {{alive_predicate}}
            AND NOT EXISTS (
                SELECT 1
                FROM post_audience_scope pas2
                WHERE pas2.post_id = p.id
                  AND pas2.scope_key = 'private'
            )
        "#,
        og_cols = og_cols,
        PUBLIC_POST_SELECT_TAIL = PUBLIC_POST_SELECT_TAIL.replace(
            "__ENGAGEMENT_COMMENT_COUNT__",
            crate::social::engagement::SQL_ENGAGEMENT_COMMENT_COUNT,
        ),
        link_join = link_join,
    )
}

async fn fetch_public_posts(
    pool: &sqlx::MySqlPool,
    alive_predicate: &str,
    with_content_link: bool,
    post_id: Option<u64>,
    limit_clause: &str,
) -> Result<Vec<PublicPostRow>, sqlx::Error> {
    let id_filter = if post_id.is_some() {
        " AND p.id = ?"
    } else {
        ""
    };
    let sql = format!(
        "{core}{id_filter}{limit}",
        core = public_post_select_core(with_content_link).replace("{alive_predicate}", alive_predicate),
        id_filter = id_filter,
        limit = limit_clause,
    );
    let mut q = sqlx::query_as(&sql)
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
        .bind(ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW);
    if let Some(id) = post_id {
        q = q.bind(id);
    }
    q.fetch_all(pool).await
}

async fn has_content_link_table(state: &AppState) -> AppResult<bool> {
    let exists: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT 1
        FROM information_schema.tables
        WHERE table_schema = DATABASE()
          AND table_name = 'content_link'
        LIMIT 1
        "#,
    )
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    Ok(exists.is_some())
}

async fn guest_topbar_chrome(state: &AppState, jar: &CookieJar) -> snippets::GuestTopbarChrome {
    let loc = i18n::request_locale();
    let signed_in = session::optional_user_id_from_jar(&state.db, jar)
        .await
        .is_some();
    snippets::guest_topbar_chrome(&loc, signed_in)
}

async fn public_posts_shell(
    State(state): State<AppState>,
    jar: CookieJar,
) -> AppResult<Html<String>> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let chrome = guest_topbar_chrome(&state, &jar).await;
    let alive_predicate = public_profile::sql_alive_entity_predicate("se");
    let with_cl = has_content_link_table(&state).await?;
    let rows = fetch_public_posts(
        &state.db,
        &alive_predicate,
        with_cl,
        None,
        " ORDER BY p.published_at DESC, p.id DESC LIMIT 50",
    )
    .await
    .map_err(AppError::Db)?;

    let domain_ids: Vec<u64> = rows.iter().filter_map(|r| r.domain_id).collect();
    let preview_map = crate::content::domain_reputation::preview_image_allowed_map(
        &state.db,
        &domain_ids,
    )
    .await
    .map_err(AppError::Db)?;
    let reveal_label = c.t(&loc, "ui.post.og_preview_reveal");
    let withheld_hint = c.t(&loc, "ui.post.og_preview_withheld_hint");
    let mut cards = String::new();
    for r in rows {
        let guest = row_to_guest(r);
        let preview_allowed =
            crate::content::domain_reputation::preview_allowed_for_domain_id(
                guest.domain_id,
                &preview_map,
            );
        let card_opts = PublicGuestCardOptions {
            show_open_post_link: true,
            preview_image_allowed: preview_allowed,
            og_preview_reveal_label: reveal_label.clone(),
            og_preview_withheld_hint: Some(withheld_hint.clone()),
        };
        cards.push_str(&render_public_guest_post_card(
            &guest,
            &card_opts,
            &loc,
            &c,
        ));
    }
    if cards.is_empty() {
        cards = format!(
            r#"<section class="card app-card public-post-card"><div class="card-accent" role="presentation"></div><p class="sub">{}</p></section>"#,
            escape_html(&c.t(&loc, "ui.entity_public.posts_empty"))
        );
    }

    let document_title = format!("{} - Public posts", c.t(&loc, "ui.shared.product_name"));
    Ok(Html(render_guest_public_document(
        &loc,
        &chrome,
        &document_title,
        "256 - Public posts",
        &format!(r#"<div class="app-cards public-post-cards">{cards}</div>"#),
        "",
    )))
}

async fn public_post_shell(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(post_id): Path<u64>,
) -> AppResult<Html<String>> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let chrome = guest_topbar_chrome(&state, &jar).await;
    let alive_predicate = public_profile::sql_alive_entity_predicate("se");
    let with_cl = has_content_link_table(&state).await?;
    let rows = fetch_public_posts(
        &state.db,
        &alive_predicate,
        with_cl,
        Some(post_id),
        " LIMIT 1",
    )
    .await
    .map_err(AppError::Db)?;
    let Some(r) = rows.into_iter().next() else {
        return Err(AppError::NotFound);
    };

    let handle = r.handle.clone();
    let post_id_val = r.id;
    let og_title_raw = r.og_title.clone();
    let og_description = r.og_description.clone();
    let og_image_url = r.og_image_url.clone();
    let body_plain = r.body_plain.clone();
    let link_href = r.link_href.clone();
    let domain_id = r.domain_id;
    let preview_allowed = match domain_id {
        Some(id) => crate::content::domain_reputation::resolve_preview_band_for_domain(
            &state.db, id,
        )
        .await
        .map(|b| b.preview_image_allowed())
        .map_err(AppError::Db)?,
        None => false,
    };
    let card = render_public_guest_post_card(
        &row_to_guest(r),
        &PublicGuestCardOptions {
            show_open_post_link: false,
            preview_image_allowed: preview_allowed,
            og_preview_reveal_label: c.t(&loc, "ui.post.og_preview_reveal"),
            og_preview_withheld_hint: Some(c.t(&loc, "ui.post.og_preview_withheld_hint")),
        },
        &loc,
        &c,
    );

    let at_handle = format!("@{handle}");
    let document_title = format!(
        "{} - {} - Public post #{}",
        c.t(&loc, "ui.shared.product_name"),
        at_handle,
        post_id_val
    );
    let topbar_title = format!("256 - {at_handle}");
    let base = state.config.public_base_url.trim_end_matches('/');
    let canonical_url = format!("{base}/public/posts/{post_id_val}");
    let site_name = c.t(&loc, "ui.shared.product_name");
    let og_title = format!(
        "{} - {}",
        at_handle,
        og_title_raw.as_deref().unwrap_or("Public post")
    );
    let meta_desc = og_description
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| trim_for_meta(s, 180))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| trim_for_meta(&body_plain, 180));
    let og_href = link_href
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| crate::ui::post_card::first_http_url_from_text(&body_plain));
    let og_url = og_href.as_deref().unwrap_or(&canonical_url);
    let head_meta_html = {
        let mut tags = String::new();
        tags.push_str(&format!(
            r#"<meta name="description" content="{desc}"><link rel="canonical" href="{canonical}"><meta property="og:type" content="article"><meta property="og:title" content="{og_title}"><meta property="og:description" content="{desc}"><meta property="og:url" content="{og_url}"><meta property="og:site_name" content="{site_name}"><meta name="twitter:card" content="summary_large_image"><meta name="twitter:title" content="{og_title}"><meta name="twitter:description" content="{desc}">"#,
            desc = escape_html(&meta_desc),
            canonical = escape_html(&canonical_url),
            og_title = escape_html(&og_title),
            og_url = escape_html(og_url),
            site_name = escape_html(&site_name),
        ));
        if let Some(img) = og_image_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            tags.push_str(&format!(
                r#"<meta property="og:image" content="{img}"><meta name="twitter:image" content="{img}">"#,
                img = escape_html(img)
            ));
        } else {
            tags.push_str(&snippets::site_og_image_meta_tags(base));
        }
        tags
    };
    Ok(Html(render_guest_public_document(
        &loc,
        &chrome,
        &document_title,
        &topbar_title,
        &format!(r#"<div class="app-cards public-post-cards">{card}</div>"#),
        &head_meta_html,
    )))
}


fn render_guest_public_document(
    lang: &str,
    chrome: &snippets::GuestTopbarChrome,
    document_title: &str,
    topbar_title: &str,
    right_panel_inner_html: &str,
    head_meta_html: &str,
) -> String {
    let topbar_html = chrome.public_topbar(&format!(
        r#"<span class="app-topbar-title mono">{}</span>"#,
        escape_html(topbar_title)
    ));
    // Placeholder for right column: user post HTML must NOT pass through `subst_markers` (can contain `@@`
    // or other sequences that confuse marker replacement).
    const RIGHT_PUBLIC_HTML_PLACEHOLDER: &str = "__256_PUBLIC_RIGHT_PANEL_PLACEHOLDER__";
    const LEFT_PUBLIC_HTML_PLACEHOLDER: &str = "__256_PUBLIC_LEFT_COLUMN_PLACEHOLDER__";
    const MARKETING_SPRITE_PLACEHOLDER: &str = "__256_PUBLIC_MARKETING_SPRITE_PLACEHOLDER__";
    let left_column = snippets::guest_public_left_column_html(lang);
    let marketing_sprite = snippets::guest_marketing_sprite_html();
    let html = format!(
        r#"<!DOCTYPE html><html lang="{lang}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">__SITE_FAVICON__<title>{title}</title><link rel="stylesheet" href="/static/css/brand.css?v=20260613-no-backdrop">{head_meta}</head><body class="app-shell-body app-shell-body--viewport-lock"><div class="app-shell app-shell--guest">{topbar}{sprite_ph}<div class="guest-split" id="guest-split-public">{left_ph}<section class="guest-right-panel">{right_ph}</section></div></div>__PUBLIC_FOOTER__<script src="/static/js/guest-shell-scroll.js" defer></script><script src="/static/js/smiley-blink.js" defer></script><script src="/static/js/brand-card-loader.js" defer></script><script src="/static/js/og-preview-client.js" defer></script><script src="/static/js/public-post-cards.js" defer></script></body></html>"#,
        lang = escape_html(lang),
        title = escape_html(document_title),
        head_meta = head_meta_html,
        topbar = topbar_html,
        sprite_ph = MARKETING_SPRITE_PLACEHOLDER,
        left_ph = LEFT_PUBLIC_HTML_PLACEHOLDER,
        right_ph = RIGHT_PUBLIC_HTML_PLACEHOLDER,
    );
    let html = i18n::subst_markers(&html, lang);
    let html = html.replace(MARKETING_SPRITE_PLACEHOLDER, &marketing_sprite);
    let html = html.replace(LEFT_PUBLIC_HTML_PLACEHOLDER, &left_column);
    let html = html.replace(RIGHT_PUBLIC_HTML_PLACEHOLDER, right_panel_inner_html);
    snippets::inject_public_footer(&html)
}

async fn public_entity_shell(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(handle): Path<String>,
) -> AppResult<Html<String>> {
    let Some(normalized) = entity::normalize_handle_from_path_segment(&handle) else {
        return Err(AppError::NotFound);
    };

    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let chrome = guest_topbar_chrome(&state, &jar).await;

    let Some(profile) = public_profile::fetch(&state.db, &normalized)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::NotFound);
    };

    let type_label = c.t(
        &loc,
        &format!(
            "ref_values.{}.{}",
            ref_keys::GROUP_SOCIAL_ENTITY_TYPE,
            profile.type_label_key
        ),
    );

    let at_handle = format!("@{}", profile.handle);
    let document_title = c.tf(
        &loc,
        "ui.entity_public.html_title",
        &[("handle", &at_handle)],
    );
    let meta_description = c.tf(
        &loc,
        "ui.entity_public.meta_description",
        &[("handle", &at_handle), ("type_label", &type_label)],
    );

    let base = state.config.public_base_url.trim_end_matches('/');
    let canonical_url = format!("{}/public/{}", base, profile.handle);
    let site_name = c.t(&loc, "ui.shared.product_name");

    let ld = serde_json::json!({
        "@context": "https://schema.org",
        "@type": "ProfilePage",
        "name": document_title,
        "url": canonical_url,
        "description": meta_description,
        "isPartOf": {
            "@type": "WebSite",
            "name": site_name,
            "url": base
        }
    });
    let ld_str = serde_json::to_string(&ld).unwrap_or_else(|_| "{}".to_string());
    let ui_boot = serde_json::json!({
        "not_found": c.t(&loc, "ui.entity_public.card_not_found"),
        "network_error": c.t(&loc, "ui.shared.network_error_short"),
        "boot_invalid": c.t(&loc, "ui.entity_public.boot_invalid"),
        "constitution_show_more": c.t(&loc, "ui.entity_private.constitution_show_more"),
        "constitution_show_less": c.t(&loc, "ui.entity_private.constitution_show_less"),
    });
    let ui_boot_str = serde_json::to_string(&ui_boot).unwrap_or_else(|_| "{}".to_string());
    let boot = serde_json::json!({
        "card_url": format!("/api/public/entity-profile/{}", normalized),
    });
    let boot_str = serde_json::to_string(&boot).unwrap_or_else(|_| "{}".to_string());
    let topbar_html = chrome.public_topbar(&format!(
        r#"<span class="app-topbar-title mono">{}</span>"#,
        escape_html(&format!("256 - @{}", profile.handle))
    ));

    let accent = entity_type_accent::profile_card_accent_class(profile.type_label_key.as_str());
    let html = ENTITY_PUBLIC_TEMPLATE
        .replace("__HTML_LANG__", &loc)
        .replace("__ENTITY_PUBLIC_PROFILE_ACCENT_CLASSES__", accent)
        .replace("__DOCUMENT_TITLE__", &escape_html(&document_title))
        .replace("__META_DESCRIPTION__", &escape_html(&meta_description))
        .replace("__CANONICAL_URL__", &escape_html(&canonical_url))
        .replace("__OG_TITLE__", &escape_html(&document_title))
        .replace("__OG_DESCRIPTION__", &escape_html(&meta_description))
        .replace("__OG_URL__", &escape_html(&canonical_url))
        .replace("__OG_SITE_NAME__", &escape_html(&site_name))
        .replace("__JSON_LD__", &escape_json_for_html_script(&ld_str))
        .replace("__PUBLIC_TOPBAR__", &topbar_html)
        .replace(
            "__CARD_LOADER_ENTITY__",
            &snippets::card_loader("entity-public"),
        )
        .replace(
            "__ENTITY_PUBLIC_UI_JSON__",
            &escape_json_for_html_script(&ui_boot_str),
        )
        .replace(
            "__ENTITY_PUBLIC_BOOT_JSON__",
            &escape_json_for_html_script(&boot_str),
        )
        .replace(
            "__SITE_OG_IMAGE_META__",
            &snippets::site_og_image_meta_tags(&base),
        )
        .replace(
            "__GUEST_PUBLIC_STACK_TOP__",
            &snippets::guest_public_entity_stack_top_html(&loc),
        )
        .replace(
            "__GUEST_PUBLIC_BENEFITS__",
            &snippets::guest_public_benefits_footer_html(&loc),
        )
        .replace(
            "__GUEST_MARKETING_SPRITE__",
            &snippets::guest_marketing_sprite_html(),
        );

    Ok(Html(snippets::inject_public_footer(
        &i18n::subst_markers(&html, &loc),
    )))
}

const ENTITY_PUBLIC_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/entity_public.html"
));
