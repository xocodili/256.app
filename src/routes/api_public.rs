//! Public JSON APIs under **`/api/public/...`** — no session, no verified-user gate.
//!
//! These are intentionally separate from authenticated routes merged under
//! [`super::require_verified`] in [`crate::routes::router`] (e.g. [`super::social`], [`super::locations`]).

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde::Serialize;
use std::time::Duration;

use crate::app_state::AppState;
use crate::content::links as content_links;
use crate::error::{AppError, AppResult};
use crate::i18n;
use crate::ref_keys;
use crate::social::community_constitution;
use crate::social::entity;
use crate::social::public_profile;
use crate::ui::join_policy;

#[derive(Serialize)]
struct PublicConstitutionClauseJson {
    point: u16,
    item: String,
}

#[derive(Serialize)]
struct PublicConstitutionSectionJson {
    community_handle: String,
    clauses: Vec<PublicConstitutionClauseJson>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/public/entity-profile/{handle}",
            get(entity_profile_card_json),
        )
        .route(
            "/api/public/content-links/preview-og",
            get(public_preview_content_link_og),
        )
}

#[derive(Serialize)]
struct PublicEntityProfileCardJson {
    /// `ref_key_values.key` for `social_entity.type_id` (`social_entity_type` group).
    type_key: String,
    handle: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    country_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entity_rating: Option<f32>,
    show_entity_rating: bool,
    avatar_path: Option<String>,
    html_title: String,
    page_heading: String,
    subtitle: String,
    type_heading: String,
    type_label: String,
    location_heading: String,
    location_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trading_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tagline: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    about: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    legal_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    founded_display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    website_url: Option<String>,
    about_heading: String,
    tagline_heading: String,
    legal_heading: String,
    founded_heading: String,
    website_heading: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    gathering_rsvp_summary_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    community_members_summary_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    human_activity_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    constitution_heading: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    constitution_tier_from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    constitution_empty: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    constitution_sections: Option<Vec<PublicConstitutionSectionJson>>,
}

#[derive(Deserialize)]
struct PublicPreviewOgQuery {
    url: String,
}

#[derive(Serialize)]
struct PublicPreviewOgResponse {
    url_canonical: String,
    og_title: Option<String>,
    og_description: Option<String>,
    og_image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    og_image_reveal_url: Option<String>,
    og_site_name: Option<String>,
    domain_blocked: bool,
    preview_image_allowed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    og_preview_reveal_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    og_preview_withheld_hint: Option<String>,
}

/// Card payload for the public HTML shell at `GET /public/{handle}` (verified entities only).
async fn entity_profile_card_json(
    State(state): State<AppState>,
    Path(handle): Path<String>,
) -> AppResult<Json<PublicEntityProfileCardJson>> {
    let Some(normalized) = entity::normalize_handle_from_path_segment(&handle) else {
        return Err(AppError::NotFound);
    };

    let loc = i18n::request_locale();
    let c = i18n::catalog();

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

    let html_title = c.tf(
        &loc,
        "ui.entity_public.html_title",
        &[("handle", &format!("@{}", profile.handle))],
    );

    let page_heading = c.tf(
        &loc,
        "ui.entity_public.page_heading",
        &[("handle", &format!("@{}", profile.handle))],
    );

    let founded_display = profile.founded_at.map(|d| d.format("%Y-%m-%d").to_string());

    let is_profit_or_np = matches!(
        profile.type_label_key.as_str(),
        ref_keys::KEY_ENTITY_TYPE_PROFIT | ref_keys::KEY_ENTITY_TYPE_NON_PROFIT
    );
    let is_gathering = profile.type_label_key.as_str() == ref_keys::KEY_ENTITY_TYPE_GATHERING;
    let is_community = profile.type_label_key.as_str() == ref_keys::KEY_ENTITY_TYPE_COMMUNITY;
    let is_human = profile.type_label_key.as_str() == ref_keys::KEY_ENTITY_TYPE_HUMAN;
    let use_brand_style_headings = is_profit_or_np || is_gathering || is_community;
    let about_heading = if use_brand_style_headings {
        c.t(&loc, "ui.entity_private.description_heading")
    } else {
        c.t(&loc, "ui.entity_public.about_heading")
    };
    let tagline_heading = if is_gathering {
        c.t(&loc, "ui.entity_private.location_heading")
    } else if use_brand_style_headings {
        c.t(&loc, "ui.entity_private.intention_heading")
    } else {
        c.t(&loc, "ui.entity_public.tagline_heading")
    };
    // Consumer-facing duty: always expose `social_entity.rating` for for-profit / non-profit avatars.
    let entity_rating = if is_profit_or_np || is_gathering || is_community || profile.show_entity_rating {
        Some(profile.entity_rating)
    } else {
        None
    };
    let show_entity_rating =
        profile.show_entity_rating || is_profit_or_np || is_gathering || is_community;

    let gathering_rsvp_summary_label = if is_gathering {
        let accepted = profile.gathering_rsvp_accepted.to_string();
        let not_going = profile.gathering_rsvp_not_going.to_string();
        let interested = profile.gathering_rsvp_interested.to_string();
        Some(c.tf(
            &loc,
            "ui.entity_private.gathering_rsvp_summary_line",
            &[
                ("accepted", accepted.as_str()),
                ("not_going", not_going.as_str()),
                ("interested", interested.as_str()),
            ],
        ))
    } else {
        None
    };

    let community_members_summary_label = if is_community {
        Some(join_policy::community_members_heading(
            &c,
            &loc,
            profile.verified_member_count,
            profile.community_join_policy_key.as_deref(),
        ))
    } else {
        None
    };

    let human_activity_summary = if is_human {
        let communities = profile.human_community_count.to_string();
        let posts = profile.human_public_post_count.to_string();
        let comments = profile.human_public_comment_count.to_string();
        Some(c.tf(
            &loc,
            "ui.entity_public.human_activity_summary",
            &[
                ("communities", communities.as_str()),
                ("posts", posts.as_str()),
                ("comments", comments.as_str()),
            ],
        ))
    } else {
        None
    };

    let (
        constitution_heading,
        constitution_tier_from,
        constitution_empty,
        constitution_sections,
    ) = if is_community {
        let sections = community_constitution::fetch_effective_constitution(
            &state.db,
            profile.entity_id,
        )
        .await
        .map_err(AppError::Db)?;
        let json_sections = sections
            .into_iter()
            .map(|s| PublicConstitutionSectionJson {
                community_handle: s.community_handle,
                clauses: s
                    .clauses
                    .into_iter()
                    .map(|c| PublicConstitutionClauseJson {
                        point: c.point,
                        item: c.item,
                    })
                    .collect(),
            })
            .collect();
        (
            Some(c.t(&loc, "ui.entity_private.constitution_heading")),
            Some(c.t(&loc, "ui.entity_private.constitution_tier_from")),
            Some(c.t(&loc, "ui.entity_private.constitution_empty")),
            Some(json_sections),
        )
    } else {
        (None, None, None, None)
    };

    let tagline = if is_gathering {
        profile
            .location_display_label
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    } else {
        profile
            .intention
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    Ok(Json(PublicEntityProfileCardJson {
        type_key: profile.type_label_key.clone(),
        handle: profile.handle.clone(),
        display_name: profile.display_name.clone(),
        country_code: profile.country_code.clone(),
        entity_rating,
        show_entity_rating,
        avatar_path: profile.avatar_path.clone(),
        html_title,
        page_heading,
        subtitle: if is_community {
            String::new()
        } else {
            c.t(&loc, "ui.entity_public.subtitle")
        },
        type_heading: c.t(&loc, "ui.entity_public.type_heading"),
        type_label,
        location_heading: c.t(&loc, "ui.entity_public.location_heading"),
        location_label: if is_human || is_community {
            None
        } else {
            profile.location_display_label
        },
        trading_name: if is_community {
            None
        } else {
            profile.trading_label.clone()
        },
        tagline,
        about: profile.description.clone(),
        legal_name: profile.legal_name.clone(),
        founded_display,
        website_url: profile.website_url.clone(),
        about_heading,
        tagline_heading,
        legal_heading: c.t(&loc, "ui.entity_public.legal_heading"),
        founded_heading: c.t(&loc, "ui.entity_public.founded_heading"),
        website_heading: c.t(&loc, "ui.entity_public.website_heading"),
        gathering_rsvp_summary_label,
        community_members_summary_label,
        human_activity_summary,
        constitution_heading,
        constitution_tier_from,
        constitution_empty,
        constitution_sections,
    }))
}

async fn public_preview_content_link_og(
    State(state): State<AppState>,
    Query(q): Query<PublicPreviewOgQuery>,
) -> AppResult<Json<PublicPreviewOgResponse>> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let canonical = content_links::canonicalize_url(&q.url)?;
    let blocked =
        content_links::registrable_domain_is_blocked(&state.db, &canonical.registrable_domain)
            .await
            .map_err(AppError::Db)?;
    if blocked {
        return Ok(Json(PublicPreviewOgResponse {
            url_canonical: canonical.url_canonical,
            og_title: None,
            og_description: None,
            og_image_url: None,
            og_image_reveal_url: None,
            og_site_name: None,
            domain_blocked: true,
            preview_image_allowed: false,
            og_preview_reveal_label: None,
            og_preview_withheld_hint: None,
        }));
    }

    let up = content_links::upsert_canonical_link(&state.db, &canonical, None)
        .await
        .ok();
    let preview_image_allowed = if let Some(ref up) = up {
        crate::content::domain_reputation::resolve_preview_band_for_domain(&state.db, up.domain_id)
            .await
            .map(|b| b.preview_image_allowed())
            .unwrap_or(false)
    } else {
        false
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(6))
        .build()
        .map_err(|e| AppError::BadRequest(format!("Could not initialize OG fetch: {e}")))?;
    let html = client
        .get(&canonical.url_canonical)
        .header(
            reqwest::header::USER_AGENT,
            "256.app/og-preview (+https://256.app)",
        )
        .header(reqwest::header::ACCEPT_ENCODING, "identity")
        .send()
        .await;
    let html = match html {
        Ok(resp) => resp.text().await.ok(),
        Err(_) => None,
    };

    let (og_title, og_description, og_image_raw, og_site_name) = if let Some(doc) = html {
        (
            extract_meta_content(&doc, "property", "og:title"),
            extract_meta_content(&doc, "property", "og:description")
                .or_else(|| extract_meta_content(&doc, "name", "description")),
            extract_meta_content(&doc, "property", "og:image"),
            extract_meta_content(&doc, "property", "og:site_name"),
        )
    } else {
        (None, None, None, None)
    };
    let mut og_image_raw = og_image_raw;
    if og_image_raw.is_none() {
        if let Some(video_id) = youtube_watch_id_from_url(&canonical.url_canonical) {
            og_image_raw = Some(format!("https://i.ytimg.com/vi/{video_id}/hqdefault.jpg"));
        }
    }

    let image_fields = crate::content::domain_reputation::split_og_image_for_preview(
        og_image_raw.as_deref(),
        &canonical.url_canonical,
        preview_image_allowed,
        false,
    );
    let (og_preview_reveal_label, og_preview_withheld_hint) = if image_fields.og_image_reveal_url.is_some() {
        (
            Some(c.t(&loc, "ui.post.og_preview_reveal")),
            Some(c.t(&loc, "ui.post.og_preview_withheld_hint")),
        )
    } else {
        (None, None)
    };

    Ok(Json(PublicPreviewOgResponse {
        url_canonical: canonical.url_canonical,
        og_title,
        og_description,
        og_image_url: image_fields.og_image_url,
        og_image_reveal_url: image_fields.og_image_reveal_url,
        og_site_name,
        domain_blocked: false,
        preview_image_allowed,
        og_preview_reveal_label,
        og_preview_withheld_hint,
    }))
}

fn extract_attr_value(tag: &str, attr: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let needle = format!("{attr}=");
    let idx = lower.find(&needle)?;
    let raw = &tag[idx + needle.len()..];
    let mut chars = raw.chars();
    let quote = chars.next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &raw[quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(rest[..end].trim().to_string())
}

fn extract_meta_content(html: &str, key_attr: &str, key_val: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let mut from = 0usize;
    let key_pair = format!(r#"{key_attr}="{key_val}""#);
    let key_pair_sq = format!(r#"{key_attr}='{key_val}'"#);
    while let Some(rel) = lower[from..].find("<meta") {
        let start = from + rel;
        let end_rel = lower[start..].find('>')?;
        let end = start + end_rel + 1;
        let tag_l = &lower[start..end];
        if tag_l.contains(&key_pair) || tag_l.contains(&key_pair_sq) {
            let tag = &html[start..end];
            if let Some(v) = extract_attr_value(tag, "content") {
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
        from = end;
    }
    None
}

fn youtube_watch_id_from_url(url: &str) -> Option<String> {
    let u = reqwest::Url::parse(url).ok()?;
    let host = u.host_str()?.to_ascii_lowercase();
    if host != "youtube.com" && host != "www.youtube.com" && host != "m.youtube.com" {
        return None;
    }
    if u.path() != "/watch" {
        return None;
    }
    for (k, v) in u.query_pairs() {
        if k == "v" {
            let vv = v.trim();
            if !vv.is_empty() {
                return Some(vv.to_string());
            }
        }
    }
    None
}
