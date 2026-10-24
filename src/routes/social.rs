//! HTTP surface for [`crate::social`].

use axum::extract::{Extension, Multipart, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, patch, post};
use axum_extra::extract::cookie::CookieJar;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::QueryBuilder;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use crate::ai::portraiture::PortraitureFacePhotos;
use crate::app_state::AppState;
use crate::auth::account as auth_account;
use crate::auth::post_auth;
use crate::auth::password::MIN_PASSWORD_LEN;
use crate::auth::session;
use crate::content::links as content_links;
use crate::content::og_fetch as content_og_fetch;
use crate::content::sanitize as content_sanitize;
use crate::error::{AppError, AppResult};
use crate::geocode::{self, GeocodeError};
use crate::i18n;
use crate::middleware::VerifiedSession;
use crate::pricing::BillableService;
use crate::rate_limit;
use super::profile_gate;
use crate::ref_keys;
use crate::ref_key_values;
use crate::social::{
    audience, blocks, community_constitution, community_graph, community_invite, engagement, entity,
    entity_profile_edit, face_capture_pending, feed, human_onboarding, ids, noms, public_profile,
    recruitment, relationship, safety, scope as social_scope, thread, user_faces,
};
use crate::social::noms::SpendOutcome;
use crate::ui::nom_icon;
use crate::ui::private_profile_card;
use crate::google_maps::places::{self, PlacesError};
use crate::locations;
use crate::status;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/me/reverse-geocode",
            post(reverse_geocode).layer(rate_limit::reverse_geocode_layer()),
        )
        .route(
            "/me/google-place-location",
            post(confirm_google_place_location).layer(rate_limit::google_places_confirm_layer()),
        )
        .route(
            "/me/google-place-search",
            get(search_google_places).layer(rate_limit::google_places_confirm_layer()),
        )
        .route(
            "/me/human-profile",
            post(create_human_profile)
                .patch(update_human_profile)
                .layer(rate_limit::social_entity_create_layer()),
        )
        .route(
            "/me/terms-consent",
            post(accept_terms_consent).layer(rate_limit::social_entity_create_layer()),
        )
        .route("/me/face-capture", post(face_capture))
        .route("/me/face-capture/status/{pending_id}", get(face_capture_status))
        .route("/me/avatar-redraw", post(avatar_redraw))
        .route(
            "/me/change-password",
            post(change_password).layer(rate_limit::limit_layer(rate_limit::login_resend_config())),
        )
        .route(
            "/me/account/delete",
            post(delete_my_account).layer(rate_limit::limit_layer(rate_limit::login_resend_config())),
        )
        .route(
            "/me/entities/{handle}",
            patch(update_managed_entity_profile)
                .layer(rate_limit::social_entity_create_layer()),
        )
        .route(
            "/me/entities/{handle}/delete",
            post(delete_managed_entity)
                .layer(rate_limit::limit_layer(rate_limit::login_resend_config())),
        )
        .route(
            "/me/entities/{handle}/recruitment-invites",
            post(send_entity_recruitment_invites)
                .layer(rate_limit::limit_layer(rate_limit::login_resend_config())),
        )
        .route("/me/block-list", get(list_my_blocks))
        .route(
            "/me/blocks/unblock",
            post(unblock_user).layer(rate_limit::social_entity_relationship_create_layer()),
        )
        .route("/me/godmother-options", get(godmother_options))
        .route(
            "/me/godmother",
            post(select_godmother).layer(rate_limit::social_entity_relationship_create_layer()),
        )
        .route(
            "/social-entities",
            post(create_entity).layer(rate_limit::social_entity_create_layer()),
        )
        .route(
            "/social-entities/handle-availability",
            get(check_entity_handle_availability)
                .layer(rate_limit::social_entity_create_layer()),
        )
        .route("/social-entities/search", get(search_entities))
        .route(
            "/social-entities/effective-constitution",
            get(effective_constitution_preview),
        )
        .route("/social-entities/{id}", get(get_entity))
        .route(
            "/social-entity-relationships",
            post(create_relationship).layer(rate_limit::social_entity_relationship_create_layer()),
        )
        .route("/social-entity-relationships/{id}", get(get_relationship))
        .route("/content-links/ingest", post(ingest_content_link))
        .route("/content-links/preview-og", post(preview_content_link_og))
        .route("/me/posting-entities", get(posting_entities))
        .route("/me/entity-recommendations", get(entity_recommendations))
        .route(
            "/me/friend-invite",
            post(send_friend_invite).layer(rate_limit::limit_layer(rate_limit::login_resend_config())),
        )
        .route("/posts", post(create_post))
        .route("/posts/share-link", post(share_link_post))
        .route("/posts/{root_id}/engagement-vote", post(record_engagement_vote))
        .route(
            "/posts/{root_id}/comments/{comment_id}/engagement-vote",
            post(record_comment_engagement_vote),
        )
        .route("/posts/audience-mention-candidates", post(audience_mention_candidates))
        .route("/posts/audience-handle-candidates", post(audience_handle_candidates))
        .route("/posts/{root_id}/comments", post(create_post_comment))
        .route("/posts/{root_id}/thread", get(get_post_thread))
        .layer(axum::extract::DefaultBodyLimit::max(12 * 1024 * 1024))
}

fn engagement_error_message(err: engagement::EngagementError, loc: &str) -> String {
    let c = i18n::catalog();
    match err {
        engagement::EngagementError::DailyPostCap => c.t(loc, "social.engagement.daily_post_cap"),
        engagement::EngagementError::ThreadBranchFull => c.t(loc, "social.engagement.thread_branch_full"),
        engagement::EngagementError::ThreadDepthLimit => c.t(loc, "social.engagement.thread_depth_limit"),
        engagement::EngagementError::CommunityAudienceGate => {
            c.t(loc, "social.engagement.community_audience_gate")
        }
        engagement::EngagementError::SelfVote => c.t(loc, "social.engagement.self_vote"),
    }
}

fn map_engagement_vote_err(err: sqlx::Error, loc: &str) -> AppError {
    if engagement::is_self_vote_error(&err) {
        AppError::BadRequest(i18n::catalog().t(loc, "social.engagement.self_vote"))
    } else {
        AppError::Db(err)
    }
}

#[derive(Debug, Deserialize)]
struct ReverseGeocodeBody {
    latitude: f64,
    longitude: f64,
}

#[derive(Debug, Serialize)]
struct ReverseGeocodeResponse {
    country_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IngestContentLinkBody {
    url: String,
}

#[derive(Debug, Serialize)]
struct IngestContentLinkResponse {
    domain_id: String,
    link_id: String,
    domain: String,
    url_original: String,
    url_canonical: String,
}

#[derive(Debug, Deserialize)]
struct PreviewOgBody {
    url: String,
    /// When true (create-post composer), return withheld images in `og_image_url` for author preview.
    #[serde(default)]
    for_composer: bool,
}

#[derive(Debug, Serialize)]
struct PreviewOgResponse {
    domain_id: String,
    link_id: String,
    domain: String,
    url_original: String,
    url_canonical: String,
    og_title: Option<String>,
    og_description: Option<String>,
    og_image_url: Option<String>,
    /// Populated when an image exists but feed auto-display is withheld (click-to-reveal only).
    #[serde(skip_serializing_if = "Option::is_none")]
    og_image_reveal_url: Option<String>,
    og_site_name: Option<String>,
    /// True when `content_domain` is blocked or marked `unsafe` — no outbound OG fetch is performed.
    domain_blocked: bool,
    /// Whether feed cards may auto-display the OG image for this domain band.
    preview_image_allowed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    og_preview_reveal_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    og_preview_withheld_hint: Option<String>,
}

#[derive(Debug, Serialize)]
struct PostingEntitiesResponse {
    wallet_balance: u16,
    entities: Vec<PostingEntityOption>,
}

#[derive(Debug, Serialize)]
struct PostingEntityOption {
    id: String,
    handle: String,
    label: String,
    type_key: String,
    /// Nom cost to publish a root post as this entity (`0` for human).
    post_nom_cost: u16,
    /// `false` when branded authorship costs more Noms than the viewer's wallet holds.
    can_afford_post: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_path: Option<String>,
}

fn posting_entity_option(
    id: u64,
    handle: String,
    type_key: String,
    label: Option<String>,
    avatar_path: Option<String>,
    wallet_balance: u16,
) -> PostingEntityOption {
    let post_nom_cost = BillableService::for_branded_post_type_key(&type_key)
        .map(BillableService::nom_cost)
        .unwrap_or(0);
    PostingEntityOption {
        id: id.to_string(),
        handle: format!("@{handle}"),
        label: label.unwrap_or_else(|| format!("@{handle}")),
        type_key,
        post_nom_cost,
        can_afford_post: wallet_balance >= post_nom_cost,
        avatar_path: avatar_path.and_then(|p| {
            let t = p.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        }),
    }
}

#[derive(Debug, Deserialize)]
struct EntityRecommendationsQuery {
    /// Max handles per entity-type cell (default 6, max 6).
    per_type: Option<u32>,
}

async fn entity_recommendations(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Query(q): Query<EntityRecommendationsQuery>,
) -> AppResult<Json<crate::social::recommendations::EntityRecommendationsResponse>> {
    let per_type = q.per_type.unwrap_or(6) as usize;
    let payload = crate::social::recommendations::recommend_for_user(&state.db, user_id, per_type)
        .await
        .map_err(AppError::Db)?;
    Ok(Json(payload))
}

#[derive(Debug, Deserialize)]
struct SearchEntitiesQuery {
    q: Option<String>,
    /// When set to `community`, only verified communities are returned.
    type_key: Option<String>,
    /// When set, exclude humans that already have any active edge to this community handle.
    exclude_community_member: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EffectiveConstitutionQuery {
    handle: Option<String>,
}

#[derive(Debug, Serialize)]
struct EffectiveConstitutionResponse {
    html: String,
    empty: bool,
}

#[derive(Debug, Deserialize)]
struct HandleAvailabilityQuery {
    handle: Option<String>,
}

#[derive(Debug, Serialize)]
struct HandleAvailabilityResponse {
    normalized_handle: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct CreatePostBody {
    entity_id: String,
    audience_handles: String,
    body_markup: String,
    domain_id: Option<String>,
    content_link_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreatePostResponse {
    id: String,
}

const SYSTEM_AUDIENCE_HANDLES: &[&str] =
    &["friends", "family", "colleague", "private", "public", "followers"];
const POST_BODY_MAX_CHARS: usize = 4000;
const ERR_POST_TEXT_REQUIRED: &str = "Post text is required.";
const ERR_POST_BODY_TOO_LONG_MARKUP: &str = "Post body exceeds 4000 characters (markup).";
const ERR_POST_BODY_TOO_LONG_SANITIZED: &str =
    "Post body exceeds 4000 characters (sanitized HTML).";
const ERR_POST_BODY_TOO_LONG_PLAIN: &str = "Post body exceeds 4000 characters (plain text).";

fn normalize_country_code(raw: &str) -> Option<String> {
    let code = raw.trim().to_ascii_uppercase();
    if code.len() == 2 && code.chars().all(|c| c.is_ascii_alphabetic()) {
        Some(code)
    } else {
        None
    }
}

fn country_code_from_headers(headers: &HeaderMap) -> Option<String> {
    const KEYS: &[&str] = &[
        "cf-ipcountry",
        "cloudfront-viewer-country",
        "x-country-code",
        "x-geo-country",
        "x-appengine-country",
    ];
    for key in KEYS {
        let Some(v) = headers.get(*key) else {
            continue;
        };
        let Ok(s) = v.to_str() else {
            continue;
        };
        if let Some(cc) = normalize_country_code(s) {
            return Some(cc);
        }
    }
    None
}

async fn fallback_country_for_user_or_entity(
    pool: &sqlx::MySqlPool,
    user_id: u64,
    entity_id: u64,
) -> Result<Option<String>, sqlx::Error> {
    let posting_entity_cc: Option<String> = sqlx::query_scalar(
        r#"
        SELECT loc.country_code
        FROM social_entity se
        INNER JOIN locations loc ON loc.id = se.location_id
        WHERE se.id = ?
        LIMIT 1
        "#,
    )
    .bind(entity_id)
    .fetch_optional(pool)
    .await?;
    if let Some(cc) = posting_entity_cc.and_then(|s| normalize_country_code(&s)) {
        return Ok(Some(cc));
    }

    let user_default_cc: Option<String> = sqlx::query_scalar(
        r#"
        SELECT loc.country_code
        FROM social_entity se
        INNER JOIN locations loc ON loc.id = se.location_id
        LEFT JOIN user_social_entity_role ur
          ON ur.social_entity_id = se.id
         AND ur.user_id = ?
         AND ur.ends_at IS NULL
        WHERE se.created_by = ? OR ur.id IS NOT NULL
        ORDER BY CASE WHEN se.created_by = ? THEN 0 ELSE 1 END, se.id ASC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(user_default_cc.and_then(|s| normalize_country_code(&s)))
}

fn strip_html_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

struct ValidatedPostBody {
    raw_markup: String,
    sanitized_markup: String,
    plain_text: String,
}

fn validate_post_body(markup: &str) -> AppResult<ValidatedPostBody> {
    let markup_raw = markup.trim().to_string();
    if markup_raw.is_empty() {
        return Err(AppError::BadRequest(ERR_POST_TEXT_REQUIRED.to_string()));
    }
    if markup_raw.chars().count() > POST_BODY_MAX_CHARS {
        return Err(AppError::BadRequest(
            ERR_POST_BODY_TOO_LONG_MARKUP.to_string(),
        ));
    }

    let markup_sanitized = content_sanitize::sanitize_post_markup(&markup_raw)
        .trim()
        .to_string();
    if markup_sanitized.is_empty() {
        return Err(AppError::BadRequest(ERR_POST_TEXT_REQUIRED.to_string()));
    }
    if markup_sanitized.chars().count() > POST_BODY_MAX_CHARS {
        return Err(AppError::BadRequest(
            ERR_POST_BODY_TOO_LONG_SANITIZED.to_string(),
        ));
    }

    let plain = strip_html_tags(&markup_sanitized).trim().to_string();
    if plain.is_empty() {
        return Err(AppError::BadRequest(ERR_POST_TEXT_REQUIRED.to_string()));
    }
    if plain.chars().count() > POST_BODY_MAX_CHARS {
        return Err(AppError::BadRequest(
            ERR_POST_BODY_TOO_LONG_PLAIN.to_string(),
        ));
    }

    Ok(ValidatedPostBody {
        raw_markup: markup_raw,
        sanitized_markup: markup_sanitized,
        plain_text: plain,
    })
}

async fn user_can_post_as_entity(
    pool: &sqlx::MySqlPool,
    user_id: u64,
    entity_id: u64,
) -> Result<bool, sqlx::Error> {
    let ok: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = se.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` IN (?, ?, ?, ?)
        LEFT JOIN user_social_entity_role ur
            ON ur.social_entity_id = se.id
           AND ur.user_id = ?
           AND ur.ends_at IS NULL
        LEFT JOIN ref_key_values rk_role
            ON rk_role.id = ur.role_id
           AND rk_role.`group` = 'user_social_entity_role'
        LEFT JOIN social_gathering sg ON sg.social_entity_id = se.id
        WHERE se.id = ?
          AND (
            se.created_by = ?
            OR (
                rk_type.`key` IN (?, ?, ?)
                AND ur.id IS NOT NULL
            )
            OR (
                rk_type.`key` = ?
                AND rk_role.`key` IN ('owner', 'admin')
                AND (sg.ends_at IS NULL OR sg.ends_at >= UTC_TIMESTAMP(6))
            )
          )
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .bind(ref_keys::KEY_ENTITY_TYPE_NON_PROFIT)
    .bind(ref_keys::KEY_ENTITY_TYPE_PROFIT)
    .bind(ref_keys::KEY_ENTITY_TYPE_GATHERING)
    .bind(user_id)
    .bind(entity_id)
    .bind(user_id)
    .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .bind(ref_keys::KEY_ENTITY_TYPE_NON_PROFIT)
    .bind(ref_keys::KEY_ENTITY_TYPE_PROFIT)
    .bind(ref_keys::KEY_ENTITY_TYPE_GATHERING)
    .fetch_optional(pool)
    .await?;
    Ok(ok.is_some())
}

async fn resolve_audience_entity_ids(
    pool: &sqlx::MySqlPool,
    subject_entity_id: u64,
    handles: &[String],
) -> AppResult<Vec<u64>> {
    if handles.is_empty() {
        return Ok(Vec::new());
    }

    let mut handle_to_id: HashMap<String, u64> = HashMap::new();
    let mut lookup = QueryBuilder::<sqlx::MySql>::new("SELECT handle, id FROM social_entity WHERE handle IN (");
    {
        let mut separated = lookup.separated(", ");
        for handle in handles {
            separated.push_bind(handle);
        }
    }
    lookup.push(")");
    let rows = lookup
        .build_query_as::<(String, u64)>()
        .fetch_all(pool)
        .await
        .map_err(AppError::Db)?;
    for (handle, id) in rows {
        handle_to_id.insert(handle, id);
    }

    for handle in handles {
        if !handle_to_id.contains_key(handle) {
            return Err(AppError::BadRequest(format!(
                "Unknown audience handle: @{handle}"
            )));
        }
    }

    let ordered_ids: Vec<u64> = handles
        .iter()
        .filter_map(|h| handle_to_id.get(h).copied())
        .collect();
    if ordered_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut scope = QueryBuilder::<sqlx::MySql>::new(
        r#"
        SELECT visible_entity_id
        FROM (
            SELECT ers.visible_entity_id
            FROM entity_read_scope ers
            WHERE ers.subject_entity_id = 
        "#,
    );
    scope.push_bind(subject_entity_id);
    scope.push(" AND ers.visible_entity_id IN (");
    {
        let mut separated = scope.separated(", ");
        for id in &ordered_ids {
            separated.push_bind(*id);
        }
    }
    scope.push(
        r#")
            UNION
            SELECT v.visible_entity_id
            FROM vw_entity_read_scope_live v
            WHERE v.subject_entity_id = "#,
    );
    scope.push_bind(subject_entity_id);
    scope.push(" AND v.visible_entity_id IN (");
    {
        let mut separated = scope.separated(", ");
        for id in &ordered_ids {
            separated.push_bind(*id);
        }
    }
    scope.push(
        r#")
        ) scoped
        "#,
    );
    let visible_ids: HashSet<u64> = scope
        .build_query_scalar::<u64>()
        .fetch_all(pool)
        .await
        .map_err(AppError::Db)?
        .into_iter()
        .collect();

    for handle in handles {
        let Some(target_id) = handle_to_id.get(handle).copied() else {
            continue;
        };
        if !visible_ids.contains(&target_id) {
            return Err(AppError::BadRequest(format!(
                "Audience handle is outside read scope: @{handle}"
            )));
        }
    }

    Ok(ordered_ids)
}

async fn create_post(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    headers: HeaderMap,
    Json(body): Json<CreatePostBody>,
) -> AppResult<(StatusCode, Json<CreatePostResponse>)> {
    require_posting_rights(&state.db, user_id).await?;

    let entity_id = ids::parse_u64(body.entity_id.trim(), "entity_id", "social_entity.invalid_id")?;
    if !user_can_post_as_entity(&state.db, user_id, entity_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::Unauthorized);
    }
    let posting_type_key: Option<String> = sqlx::query_scalar(
        r#"
        SELECT rk.`key`
        FROM social_entity se
        INNER JOIN ref_key_values rk
            ON rk.id = se.type_id
           AND rk.`group` = ?
        WHERE se.id = ?
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(entity_id)
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    let posting_type_key = posting_type_key.unwrap_or_default();
    let is_community_post = posting_type_key == ref_keys::KEY_ENTITY_TYPE_COMMUNITY;
    let is_gathering_post = posting_type_key == ref_keys::KEY_ENTITY_TYPE_GATHERING;
    let is_human_post = posting_type_key == ref_keys::KEY_ENTITY_TYPE_HUMAN;

    let validated_post_body = validate_post_body(&body.body_markup)?;

    crate::content::domain_reputation::ensure_inline_body_urls_allowed(
        &state.db,
        &[
            validated_post_body.raw_markup.as_str(),
            validated_post_body.sanitized_markup.as_str(),
            validated_post_body.plain_text.as_str(),
        ],
        "create_post",
        user_id,
        None,
    )
    .await?;

    let mut content_link_id = body
        .content_link_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| ids::parse_u64(s, "content_link_id", "social_entity.invalid_id"))
        .transpose()?;
    let mut domain_id = body
        .domain_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| ids::parse_u64(s, "domain_id", "social_entity.invalid_id"))
        .transpose()?;

    if let Some(lid) = content_link_id {
        let link_domain_id: Option<u64> = sqlx::query_scalar("SELECT domain_id FROM content_link WHERE id = ?")
            .bind(lid)
            .fetch_optional(&state.db)
            .await
            .map_err(AppError::Db)?;
        let Some(ldid) = link_domain_id else {
            return Err(AppError::BadRequest("Invalid content_link_id.".to_string()));
        };
        if let Some(did) = domain_id {
            if did != ldid {
                return Err(AppError::BadRequest(
                    "domain_id does not match content_link_id.".to_string(),
                ));
            }
        } else {
            domain_id = Some(ldid);
        }
    }

    if let Some(did) = domain_id {
        let row: Option<(String, String)> = sqlx::query_as(
            r#"
            SELECT rk.`key`, d.safety_state
            FROM content_domain d
            INNER JOIN ref_key_values rk ON rk.id = d.status_id
            WHERE d.id = ?
            LIMIT 1
            "#,
        )
        .bind(did)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Db)?;
        let Some((status_key, safety_state)) = row else {
            return Err(AppError::BadRequest("Invalid domain_id.".to_string()));
        };
        if status_key == ref_keys::KEY_BLOCKED || safety_state == "unsafe" {
            let domain_name: Option<String> = sqlx::query_scalar(
                "SELECT domain FROM content_domain WHERE id = ? LIMIT 1",
            )
            .bind(did)
            .fetch_optional(&state.db)
            .await
            .map_err(AppError::Db)?;
            if let Some(ref domain) = domain_name {
                crate::social::safety::sanitation::log_domain_deny(
                    &state.db,
                    "create_post",
                    domain,
                    Some(user_id),
                    None,
                )
                .await;
            }
            return Err(AppError::BadRequest(
                i18n::catalog()
                    .t(&i18n::request_locale(), "social.domain_blocked_for_post"),
            ));
        }
    }

    let mut audience_handles = audience::parse_audience_handles(body.audience_handles.as_str());
    if is_community_post || is_gathering_post {
        audience_handles.clear();
    }
    // @public and @private are mutually exclusive; later token in reading order wins.
    if let (Some(pub_pos), Some(priv_pos)) = (
        audience_handles.iter().position(|s| s == "public"),
        audience_handles.iter().position(|s| s == "private"),
    ) {
        if pub_pos > priv_pos {
            audience_handles.retain(|s| s != "private");
        } else {
            audience_handles.retain(|s| s != "public");
        }
    }
    // Privacy by default: if the author omits both @public and @private, treat the post as @private.
    if is_human_post
        && !audience_handles.iter().any(|s| s == "public")
        && !audience_handles.iter().any(|s| s == "private")
    {
        audience_handles.push("private".to_string());
    }
    let mut system_scopes: Vec<String> = Vec::new();
    let mut audience_entity_handles: Vec<String> = Vec::new();
    for handle in audience_handles {
        if SYSTEM_AUDIENCE_HANDLES.iter().any(|k| *k == handle.as_str()) {
            system_scopes.push(handle);
            continue;
        }
        audience_entity_handles.push(handle);
    }
    let brand_style_public_default = posting_type_key == ref_keys::KEY_ENTITY_TYPE_PROFIT
        || posting_type_key == ref_keys::KEY_ENTITY_TYPE_NON_PROFIT
        || posting_type_key == ref_keys::KEY_ENTITY_TYPE_GATHERING;
    if brand_style_public_default && !system_scopes.iter().any(|s| s == "private") {
        if !system_scopes.iter().any(|s| s == "public") {
            system_scopes.push("public".to_string());
        }
    }
    let mut audience_entity_ids =
        resolve_audience_entity_ids(&state.db, entity_id, &audience_entity_handles).await?;
    if !audience_entity_ids.contains(&entity_id) {
        audience_entity_ids.push(entity_id);
    }
    let post_country_code = match country_code_from_headers(&headers) {
        Some(cc) => Some(cc),
        None => fallback_country_for_user_or_entity(&state.db, user_id, entity_id)
            .await
            .map_err(AppError::Db)?,
    };

    let mut tx = state.db.begin().await.map_err(AppError::Db)?;
    let user_row_exists: Option<u64> = sqlx::query_scalar("SELECT id FROM users WHERE id = ? FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(AppError::Db)?;
    if user_row_exists.is_none() {
        return Err(AppError::Unauthorized);
    }
    engagement::check_daily_post_cap_tx(&mut tx, user_id)
        .await
        .map_err(|e| AppError::BadRequest(engagement_error_message(e, &i18n::request_locale())))?;
    engagement::check_community_audience_gate_tx(&mut tx, user_id, &audience_entity_ids)
        .await
        .map_err(|e| AppError::BadRequest(engagement_error_message(e, &i18n::request_locale())))?;

    if let Some(service) = BillableService::for_branded_post_type_key(&posting_type_key) {
        let loc = i18n::request_locale();
        let c = i18n::catalog();
        let human_entity_id = human_onboarding::primary_human_entity_id_tx(&mut tx, user_id)
            .await
            .map_err(AppError::Db)?
            .ok_or_else(|| {
                AppError::BadRequest(c.t(&loc, "social_entity.human_wallet_required"))
            })?;
        let author_handle: String = sqlx::query_scalar(
            "SELECT handle FROM social_entity WHERE id = ? LIMIT 1",
        )
        .bind(entity_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(AppError::Db)?
        .unwrap_or_default();
        let spend_outcome = noms::spend_for_service_tx(
            &mut tx,
            human_entity_id,
            service,
            user_id,
            Some(author_handle.as_str()),
        )
        .await
        .map_err(AppError::Db)?;
        if let SpendOutcome::InsufficientBalance { needed, balance } = spend_outcome {
            return Err(AppError::BadRequest(nom_icon::insufficient_noms_plain(
                &c,
                &loc,
                "noms.insufficient_for_branded_post",
                needed,
                Some(balance),
            )));
        }
    }

    let post_res = sqlx::query(
        r#"
        INSERT INTO post (
            entity_id, content_link_id, parent_post_id, root_post_id, status_id, rating,
            body_plain, body_markup, body_html_sanitized, published_at,
            created_by, modified_by, domain_id, country_code
        )
        VALUES (?, ?, NULL, NULL, NULL, 0.00, ?, ?, ?, UTC_TIMESTAMP(6), ?, ?, ?, ?)
        "#,
    )
    .bind(entity_id)
    .bind(content_link_id.take())
    .bind(validated_post_body.plain_text.as_str())
    .bind(validated_post_body.raw_markup.as_str())
    .bind(validated_post_body.sanitized_markup.as_str())
    .bind(user_id)
    .bind(user_id)
    .bind(domain_id)
    .bind(post_country_code)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;
    let post_id = post_res.last_insert_id();

    crate::social::conversation_rating::init_conversation_tx(&mut tx, post_id)
        .await
        .map_err(AppError::Db)?;

    let audience_for_mentions: Vec<String> = system_scopes
        .iter()
        .chain(audience_entity_handles.iter())
        .cloned()
        .collect();
    crate::social::mentions::persist_mentions_tx(
        &mut tx,
        post_id,
        entity_id,
        user_id,
        &audience_for_mentions,
        validated_post_body.plain_text.as_str(),
    )
    .await
    .map_err(AppError::Db)?;

    let mut post_activity_entities = audience_entity_ids.clone();
    if is_community_post && !post_activity_entities.contains(&entity_id) {
        post_activity_entities.push(entity_id);
    }
    crate::social::community_graph::touch_post_activity_for_entities_tx(
        &mut tx,
        &post_activity_entities,
    )
    .await
    .map_err(AppError::Db)?;

    for audience_entity_id in audience_entity_ids {
        sqlx::query(
            r#"
            INSERT INTO post_audience_entity (post_id, audience_entity_id, created_by)
            VALUES (?, ?, ?)
            ON DUPLICATE KEY UPDATE created_by = COALESCE(VALUES(created_by), created_by)
            "#,
        )
        .bind(post_id)
        .bind(audience_entity_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Db)?;
    }
    for scope_key in system_scopes {
        sqlx::query(
            r#"
            INSERT INTO post_audience_scope (post_id, scope_key, created_by)
            VALUES (?, ?, ?)
            ON DUPLICATE KEY UPDATE created_by = COALESCE(VALUES(created_by), created_by)
            "#,
        )
        .bind(post_id)
        .bind(scope_key)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Db)?;
    }
    tx.commit().await.map_err(AppError::Db)?;

    Ok((
        StatusCode::CREATED,
        Json(CreatePostResponse {
            id: post_id.to_string(),
        }),
    ))
}

#[derive(Debug, Deserialize)]
struct EngagementVoteBody {
    direction: String,
}

#[derive(Debug, Serialize)]
struct EngagementVoteResponse {
    stars: f64,
    feel: String,
    direction: String,
}

async fn record_engagement_vote(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(root_id): Path<u64>,
    Json(body): Json<EngagementVoteBody>,
) -> AppResult<Json<EngagementVoteResponse>> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let direction = engagement::EngagementDirection::parse(&body.direction).ok_or_else(|| {
        AppError::BadRequest(c.t(&loc, "social.engagement.direction_invalid"))
    })?;
    if !feed::viewer_can_engage_on_root_post(&state.db, user_id, root_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::NotFound);
    }
    let mut tx = state.db.begin().await.map_err(AppError::Db)?;
    let stars = engagement::record_root_engagement_vote_tx(&mut tx, root_id, user_id, direction)
        .await
        .map_err(|e| map_engagement_vote_err(e, &loc))?;
    crate::social::conversation_rating::touch_conversation_activity_tx(&mut tx, root_id)
        .await
        .map_err(AppError::Db)?;
    tx.commit().await.map_err(AppError::Db)?;
    let feel = engagement::PostFeel::from_stars(stars).key().to_string();
    Ok(Json(EngagementVoteResponse {
        stars,
        feel,
        direction: direction.as_str().to_string(),
    }))
}

async fn record_comment_engagement_vote(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path((root_id, comment_id)): Path<(u64, u64)>,
    Json(body): Json<EngagementVoteBody>,
) -> AppResult<Json<EngagementVoteResponse>> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let direction = engagement::EngagementDirection::parse(&body.direction).ok_or_else(|| {
        AppError::BadRequest(c.t(&loc, "social.engagement.direction_invalid"))
    })?;
    if !feed::viewer_can_engage_on_root_post(&state.db, user_id, root_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::NotFound);
    }
    if !engagement::can_participate_in_root_discussion(&state.db, root_id, user_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            c.t(&loc, "social.engagement.vote_root_before_reply"),
        ));
    }
    let comment_row: Option<(u64,)> = sqlx::query_as(
        r#"
        SELECT id FROM post
        WHERE id = ? AND COALESCE(root_post_id, id) = ? AND is_moderation_mirror = 0
        LIMIT 1
        "#,
    )
    .bind(comment_id)
    .bind(root_id)
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    if comment_row.is_none() {
        return Err(AppError::NotFound);
    }
    let mut tx = state.db.begin().await.map_err(AppError::Db)?;
    let stars =
        engagement::record_engagement_vote_tx(&mut tx, root_id, comment_id, user_id, direction)
            .await
            .map_err(|e| map_engagement_vote_err(e, &loc))?;
    crate::social::conversation_rating::touch_conversation_activity_tx(&mut tx, comment_id)
        .await
        .map_err(AppError::Db)?;
    tx.commit().await.map_err(AppError::Db)?;
    let feel = engagement::PostFeel::from_stars(stars).key().to_string();
    Ok(Json(EngagementVoteResponse {
        stars,
        feel,
        direction: direction.as_str().to_string(),
    }))
}

#[derive(Debug, Deserialize)]
struct AudienceMentionCandidatesBody {
    entity_id: String,
    audience_handles: String,
    prefix: String,
    #[serde(default)]
    root_post_id: Option<String>,
}

async fn audience_mention_candidates(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Json(body): Json<AudienceMentionCandidatesBody>,
) -> AppResult<Json<Vec<crate::social::mentions::MentionCandidate>>> {
    let entity_id = ids::parse_u64(body.entity_id.trim(), "entity_id", "social_entity.invalid_id")?;
    if !user_can_post_as_entity(&state.db, user_id, entity_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::Unauthorized);
    }
    let (poster_entity_id, handles) =
        if let Some(root_post_id_str) = body
            .root_post_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let root_post_id =
                ids::parse_u64(root_post_id_str, "root_post_id", "social_entity.invalid_id")?;
            if !feed::viewer_can_engage_on_root_post(&state.db, user_id, root_post_id)
                .await
                .map_err(AppError::Db)?
            {
                return Err(AppError::NotFound);
            }
            let Some(ctx) = audience::root_post_mention_context(&state.db, root_post_id)
                .await
                .map_err(AppError::Db)?
            else {
                return Err(AppError::NotFound);
            };
            (ctx.poster_entity_id, ctx.audience_handles)
        } else {
            (
                entity_id,
                audience::parse_audience_handles(body.audience_handles.as_str()),
            )
        };
    let items = crate::social::mentions::audience_mention_candidates(
        &state.db,
        poster_entity_id,
        user_id,
        &handles,
        body.prefix.as_str(),
        12,
        entity_id,
    )
    .await
    .map_err(AppError::Db)?;
    Ok(Json(items))
}

#[derive(Debug, Deserialize)]
struct AudienceHandleCandidatesBody {
    prefix: String,
}

async fn audience_handle_candidates(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Json(body): Json<AudienceHandleCandidatesBody>,
) -> AppResult<Json<Vec<audience::AudienceHandleCandidate>>> {
    let items = audience::audience_handle_candidates(&state.db, user_id, body.prefix.as_str(), 16)
        .await
        .map_err(AppError::Db)?;
    Ok(Json(items))
}

#[derive(Debug, Deserialize)]
struct CreatePostCommentBody {
    entity_id: String,
    parent_post_id: String,
    body_markup: String,
}

async fn create_post_comment(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(root_id): Path<u64>,
    headers: HeaderMap,
    Json(body): Json<CreatePostCommentBody>,
) -> AppResult<(StatusCode, Json<CreatePostResponse>)> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    require_posting_rights(&state.db, user_id).await?;
    if !feed::viewer_can_engage_on_root_post(&state.db, user_id, root_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::NotFound);
    }
    let entity_id = ids::parse_u64(body.entity_id.trim(), "entity_id", "social_entity.invalid_id")?;
    if !user_can_post_as_entity(&state.db, user_id, entity_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::Unauthorized);
    }
    let parent_id = ids::parse_u64(
        body.parent_post_id.trim(),
        "parent_post_id",
        "social_entity.invalid_id",
    )?;
    let validated = validate_post_body(&body.body_markup)?;

    crate::content::domain_reputation::ensure_inline_body_urls_allowed(
        &state.db,
        &[
            validated.raw_markup.as_str(),
            validated.sanitized_markup.as_str(),
            validated.plain_text.as_str(),
        ],
        "create_post_comment",
        user_id,
        Some(root_id),
    )
    .await?;

    if !engagement::can_participate_in_root_discussion(&state.db, root_id, user_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            c.t(&loc, "social.engagement.vote_before_comment"),
        ));
    }

    let mut tx = state.db.begin().await.map_err(AppError::Db)?;
    engagement::check_daily_post_cap_tx(&mut tx, user_id)
        .await
        .map_err(|e| AppError::BadRequest(engagement_error_message(e, &i18n::request_locale())))?;

    let parent_row: Option<(u64, u64)> = sqlx::query_as(
        r#"SELECT id, COALESCE(root_post_id, id) FROM post WHERE id = ? LIMIT 1"#,
    )
    .bind(parent_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::Db)?;
    let Some((_, thread_root)) = parent_row else {
        return Err(AppError::BadRequest("Invalid parent post.".to_string()));
    };
    if thread_root != root_id {
        return Err(AppError::BadRequest("Parent is not in this thread.".to_string()));
    }

    engagement::assert_can_add_child_tx(&mut tx, parent_id, root_id, false)
        .await
        .map_err(|e| AppError::BadRequest(engagement_error_message(e, &i18n::request_locale())))?;

  // Comments inherit root audience visibility — no new audience on reply for now.
    let post_country_code = match country_code_from_headers(&headers) {
        Some(cc) => Some(cc),
        None => fallback_country_for_user_or_entity(&state.db, user_id, entity_id)
            .await
            .map_err(AppError::Db)?,
    };

    let post_res = sqlx::query(
        r#"
        INSERT INTO post (
            entity_id, content_link_id, parent_post_id, root_post_id, status_id, rating,
            body_plain, body_markup, body_html_sanitized, published_at,
            created_by, modified_by, domain_id, country_code,
            is_moderation_mirror
        )
        VALUES (?, NULL, ?, ?, NULL, 0.00, ?, ?, ?, UTC_TIMESTAMP(6), ?, ?, NULL, ?, 0)
        "#,
    )
    .bind(entity_id)
    .bind(parent_id)
    .bind(root_id)
    .bind(validated.plain_text.as_str())
    .bind(validated.raw_markup.as_str())
    .bind(validated.sanitized_markup.as_str())
    .bind(user_id)
    .bind(user_id)
    .bind(post_country_code)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;
    let post_id = post_res.last_insert_id();

    crate::social::conversation_rating::init_conversation_tx(&mut tx, post_id)
        .await
        .map_err(AppError::Db)?;
    crate::social::conversation_rating::touch_conversation_activity_tx(&mut tx, post_id)
        .await
        .map_err(AppError::Db)?;

    let mention_ctx = audience::root_post_mention_context_conn(&mut *tx, root_id)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::NotFound)?;
    crate::social::mentions::persist_mentions_tx(
        &mut tx,
        post_id,
        mention_ctx.poster_entity_id,
        user_id,
        &mention_ctx.audience_handles,
        validated.plain_text.as_str(),
    )
    .await
    .map_err(AppError::Db)?;

    tx.commit().await.map_err(AppError::Db)?;

    Ok((
        StatusCode::CREATED,
        Json(CreatePostResponse {
            id: post_id.to_string(),
        }),
    ))
}

async fn get_post_thread(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(root_id): Path<u64>,
    axum::extract::Query(query): axum::extract::Query<ThreadQuery>,
) -> AppResult<Json<thread::PostThreadResponse>> {
    if !feed::viewer_can_engage_on_root_post(&state.db, user_id, root_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::NotFound);
    }
    let page = thread::load_thread(&state.db, root_id, user_id, query.l1_offset)
        .await
        .map_err(AppError::Db)?;
    let Some(page) = page else {
        return Err(AppError::NotFound);
    };
    Ok(Json(page))
}

#[derive(Debug, Deserialize, Default)]
struct ThreadQuery {
    #[serde(default)]
    l1_offset: u32,
}

#[derive(Debug, Deserialize)]
struct ShareLinkPostBody {
    entity_id: String,
    content_link_id: String,
    audience_handles: String,
}

async fn share_link_post(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    headers: HeaderMap,
    Json(body): Json<ShareLinkPostBody>,
) -> AppResult<(StatusCode, Json<CreatePostResponse>)> {
    require_posting_rights(&state.db, user_id).await?;
    let entity_id = ids::parse_u64(body.entity_id.trim(), "entity_id", "social_entity.invalid_id")?;
    if !user_can_post_as_entity(&state.db, user_id, entity_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::Unauthorized);
    }
    let content_link_id = ids::parse_u64(
        body.content_link_id.trim(),
        "content_link_id",
        "social_entity.invalid_id",
    )?;

    let link_row: Option<(u64, String)> = sqlx::query_as(
        r#"
        SELECT cl.domain_id, cd.domain
        FROM content_link cl
        INNER JOIN content_domain cd ON cd.id = cl.domain_id
        WHERE cl.id = ?
        "#,
    )
    .bind(content_link_id)
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    let Some((link_domain_id, registrable_domain)) = link_row else {
        return Err(AppError::BadRequest("Invalid content_link_id.".to_string()));
    };
    content_links::ensure_domain_allowed(
        &state.db,
        &registrable_domain,
        "share_link_post",
        Some(user_id),
        None,
    )
    .await?;

    let mut audience_handles = audience::parse_audience_handles(body.audience_handles.as_str());
    let mut system_scopes: Vec<String> = Vec::new();
    let mut audience_entity_handles: Vec<String> = Vec::new();
    for handle in audience_handles.drain(..) {
        if SYSTEM_AUDIENCE_HANDLES.iter().any(|k| *k == handle.as_str()) {
            system_scopes.push(handle);
            continue;
        }
        audience_entity_handles.push(handle);
    }
    if system_scopes.iter().all(|s| s != "private") && !system_scopes.iter().any(|s| s == "public") {
        system_scopes.push("private".to_string());
    }
    let audience_entity_ids =
        resolve_audience_entity_ids(&state.db, entity_id, &audience_entity_handles).await?;
    if !audience_entity_ids.contains(&entity_id) {
        return Err(AppError::BadRequest(
            "Share target audience is invalid.".to_string(),
        ));
    }

    let placeholder_plain = "Shared link";
    let placeholder_markup = "<p>Shared link</p>";
    let placeholder_html = "<p>Shared link</p>";

    let mut tx = state.db.begin().await.map_err(AppError::Db)?;
    engagement::check_daily_post_cap_tx(&mut tx, user_id)
        .await
        .map_err(|e| AppError::BadRequest(engagement_error_message(e, &i18n::request_locale())))?;
    engagement::check_community_audience_gate_tx(&mut tx, user_id, &audience_entity_ids)
        .await
        .map_err(|e| AppError::BadRequest(engagement_error_message(e, &i18n::request_locale())))?;

    let post_country_code = match country_code_from_headers(&headers) {
        Some(cc) => Some(cc),
        None => fallback_country_for_user_or_entity(&state.db, user_id, entity_id)
            .await
            .map_err(AppError::Db)?,
    };

    let post_res = sqlx::query(
        r#"
        INSERT INTO post (
            entity_id, content_link_id, parent_post_id, root_post_id, status_id, rating,
            body_plain, body_markup, body_html_sanitized, published_at,
            created_by, modified_by, domain_id, country_code,
            is_moderation_mirror
        )
        VALUES (?, ?, NULL, NULL, NULL, 0.00, ?, ?, ?, UTC_TIMESTAMP(6), ?, ?, ?, ?, 0)
        "#,
    )
    .bind(entity_id)
    .bind(content_link_id)
    .bind(placeholder_plain)
    .bind(placeholder_markup)
    .bind(placeholder_html)
    .bind(user_id)
    .bind(user_id)
    .bind(link_domain_id)
    .bind(post_country_code)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;
    let post_id = post_res.last_insert_id();

    sqlx::query(
        r#"UPDATE content_link SET share_count = share_count + 1 WHERE id = ?"#,
    )
    .bind(content_link_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;

    for audience_entity_id in &audience_entity_ids {
        sqlx::query(
            r#"
            INSERT INTO post_audience_entity (post_id, audience_entity_id, created_by)
            VALUES (?, ?, ?)
            ON DUPLICATE KEY UPDATE created_by = COALESCE(VALUES(created_by), created_by)
            "#,
        )
        .bind(post_id)
        .bind(audience_entity_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Db)?;
    }
    for scope_key in system_scopes {
        sqlx::query(
            r#"
            INSERT INTO post_audience_scope (post_id, scope_key, created_by)
            VALUES (?, ?, ?)
            ON DUPLICATE KEY UPDATE created_by = COALESCE(VALUES(created_by), created_by)
            "#,
        )
        .bind(post_id)
        .bind(scope_key)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Db)?;
    }

    tx.commit().await.map_err(AppError::Db)?;

    Ok((
        StatusCode::CREATED,
        Json(CreatePostResponse {
            id: post_id.to_string(),
        }),
    ))
}

async fn reverse_geocode(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: _user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Json(body): Json<ReverseGeocodeBody>,
) -> AppResult<Json<ReverseGeocodeResponse>> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let cc = match geocode::reverse_country_code(
        body.latitude,
        body.longitude,
        &state.config.nominatim_user_agent,
    )
    .await
    {
        Ok(o) => o,
        Err(GeocodeError::InvalidCoordinates) => {
            return Err(AppError::BadRequest(
                c.t(&loc, "geocode.invalid_coordinates"),
            ));
        }
        Err(GeocodeError::Upstream) => {
            return Err(AppError::BadRequest(c.t(&loc, "geocode.unavailable")));
        }
    };
    Ok(Json(ReverseGeocodeResponse { country_code: cc }))
}

#[derive(Debug, Deserialize)]
struct GooglePlaceLocationBody {
    /// Google Place ID (optionally prefixed with `places/`).
    place_id: String,
    /// When `true` (default), updates the viewer's primary human `social_entity.location_id`.
    /// Set to `false` when recording a place for another flow (e.g. new business) without moving the human pin.
    #[serde(default = "google_place_update_human_default")]
    update_human_entity: bool,
}

#[derive(Debug, Deserialize)]
struct GooglePlaceSearchQuery {
    q: Option<String>,
}

#[derive(Debug, Serialize)]
struct GooglePlaceSearchResult {
    place_id: String,
    main_text: String,
    secondary_text: Option<String>,
}

const fn google_place_update_human_default() -> bool {
    true
}

#[derive(Debug, Serialize)]
struct GooglePlaceLocationResponse {
    location_id: String,
    display_label: String,
    country_code: String,
}

fn map_places_error(e: PlacesError) -> AppError {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    match e {
        PlacesError::NotConfigured => AppError::BadRequest(c.t(&loc, "google_place.not_configured")),
        PlacesError::InvalidPlaceId => AppError::BadRequest(c.t(&loc, "google_place.invalid_place_id")),
        PlacesError::NotFound => AppError::BadRequest(c.t(&loc, "google_place.place_not_found")),
        PlacesError::Upstream | PlacesError::UpstreamStatus(_) | PlacesError::BadPayload => {
            AppError::BadRequest(c.t(&loc, "google_place.unavailable"))
        }
        PlacesError::MissingCountry | PlacesError::BadCoordinates => {
            AppError::BadRequest(c.t(&loc, "google_place.incomplete_place"))
        }
        PlacesError::QueryTooShort => AppError::BadRequest(c.t(&loc, "google_place.query_too_short")),
    }
}

/// Google Place Details → `locations` row.
/// By default also updates the viewer's primary human `social_entity.location_id`.
/// Set `update_human_entity: false` to only create/resolve `locations` (e.g. new business address) without moving the human pin.
/// Response omits coordinates and Google-only ids; internal verification tier is unchanged.
async fn confirm_google_place_location(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Json(body): Json<GooglePlaceLocationBody>,
) -> AppResult<(StatusCode, Json<GooglePlaceLocationResponse>)> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();

    let key = state
        .config
        .google_maps_api_key
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| AppError::BadRequest(c.t(&loc, "google_place.not_configured")))?;

    let ms = state.config.google_places_timeout_ms.clamp(1000, 30_000);
    let timeout = Duration::from_millis(ms);

    let human_entity_id = human_onboarding::primary_human_entity_id(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
        .ok_or_else(|| AppError::BadRequest(c.t(&loc, "google_place.need_human_profile")))?;

    let req = places::build_create_location_from_place_id(key, &body.place_id, timeout)
        .await
        .map_err(map_places_error)?;

    let loc_pub = locations::create_or_get(&state.db, req, user_id).await?;

    let new_location_id: u64 = loc_pub.id.parse().map_err(|_| {
        AppError::BadRequest(c.t(&loc, "google_place.location_id_parse"))
    })?;

    if body.update_human_entity {
        let res = sqlx::query(
            "UPDATE social_entity SET location_id = ?, modified_by = ? WHERE id = ?",
        )
        .bind(new_location_id)
        .bind(user_id)
        .bind(human_entity_id)
        .execute(&state.db)
        .await
        .map_err(AppError::Db)?;

        if res.rows_affected() == 0 {
            return Err(AppError::NotFound);
        }
    }

    Ok((
        StatusCode::OK,
        Json(GooglePlaceLocationResponse {
            location_id: loc_pub.id,
            display_label: loc_pub.display_label,
            country_code: loc_pub.country_code,
        }),
    ))
}

async fn preferred_country_bias_for_user(
    pool: &sqlx::MySqlPool,
    user_id: u64,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT l.country_code
        FROM social_entity se
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        INNER JOIN locations l ON l.id = se.location_id
        LEFT JOIN user_social_entity_role ur
            ON ur.social_entity_id = se.id
           AND ur.user_id = ?
           AND ur.ends_at IS NULL
        WHERE ur.id IS NOT NULL OR se.created_by = ?
        ORDER BY
            CASE WHEN ur.is_primary = 1 THEN 0 ELSE 1 END,
            se.id ASC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

async fn search_google_places(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Query(q): Query<GooglePlaceSearchQuery>,
) -> AppResult<Json<Vec<GooglePlaceSearchResult>>> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let raw = q.q.unwrap_or_default();
    if raw.trim().chars().count() < 5 {
        return Ok(Json(Vec::new()));
    }
    let key = state
        .config
        .google_maps_api_key
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| AppError::BadRequest(c.t(&loc, "google_place.not_configured")))?;
    let ms = state.config.google_places_timeout_ms.clamp(1000, 30_000);
    let timeout = Duration::from_millis(ms);
    let country_bias = preferred_country_bias_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let suggestions = places::autocomplete_places(key, &raw, country_bias.as_deref(), timeout)
        .await
        .map_err(map_places_error)?;
    let rows = suggestions
        .into_iter()
        .map(|s| GooglePlaceSearchResult {
            place_id: s.place_id,
            main_text: s.main_text,
            secondary_text: s.secondary_text,
        })
        .collect();
    Ok(Json(rows))
}

/// Thin ingestion endpoint: canonicalize URL and upsert `content_domain`/`content_link`.
async fn ingest_content_link(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Json(body): Json<IngestContentLinkBody>,
) -> AppResult<(StatusCode, Json<IngestContentLinkResponse>)> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let canonical = content_links::canonicalize_url(&body.url)?;
    content_links::ensure_domain_allowed(
        &state.db,
        &canonical.registrable_domain,
        "content_link_ingest",
        Some(user_id),
        None,
    )
    .await?;
    let up = content_links::upsert_canonical_link(&state.db, &canonical, Some(user_id)).await?;
    Ok((
        StatusCode::CREATED,
        Json(IngestContentLinkResponse {
            domain_id: up.domain_id.to_string(),
            link_id: up.link_id.to_string(),
            domain: canonical.registrable_domain,
            url_original: canonical.url_original,
            url_canonical: canonical.url_canonical,
        }),
    ))
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
    let mut video_id = None;
    for (k, v) in u.query_pairs() {
        if k == "v" {
            let vv = v.trim();
            if !vv.is_empty() {
                video_id = Some(vv.to_string());
            }
            break;
        }
    }
    video_id
}

async fn preview_content_link_og(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Json(body): Json<PreviewOgBody>,
) -> AppResult<(StatusCode, Json<PreviewOgResponse>)> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let canonical = content_links::canonicalize_url(&body.url)?;
    let blocked = content_links::registrable_domain_is_blocked(&state.db, &canonical.registrable_domain)
        .await
        .map_err(AppError::Db)?;
    if blocked {
        return Ok((
            StatusCode::OK,
            Json(PreviewOgResponse {
                domain_id: String::new(),
                link_id: String::new(),
                domain: canonical.registrable_domain.clone(),
                url_original: canonical.url_original.clone(),
                url_canonical: canonical.url_canonical.clone(),
                og_title: None,
                og_description: None,
                og_image_url: None,
                og_image_reveal_url: None,
                og_site_name: None,
                domain_blocked: true,
                preview_image_allowed: false,
                og_preview_reveal_label: None,
                og_preview_withheld_hint: None,
            }),
        ));
    }

    let up = content_links::upsert_canonical_link(&state.db, &canonical, Some(user_id))
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

    let og_row_before = if let Some(ref up) = up {
        content_og_fetch::load_link_og_row(&state.db, up.link_id)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    if let Some(ref up) = up {
        if og_row_before
            .as_ref()
            .is_none_or(|r| r.og_image_url.as_deref().is_none_or(|u| u.trim().is_empty()))
        {
            let outcome = content_og_fetch::fetch_and_record_og_for_link(
                &state.db,
                up.link_id,
                &canonical.url_canonical,
                Some(user_id),
            )
            .await;
            if outcome.is_err() {
                tracing::debug!(link_id = up.link_id, "human preview OG fetch failed");
            }
        }
    }

    let og_row = if let Some(ref up) = up {
        content_og_fetch::load_link_og_row(&state.db, up.link_id)
            .await
            .ok()
            .flatten()
    } else {
        None
    };
    let prior_image = og_row_before.and_then(|b| b.og_image_url);
    let (og_title, og_description, og_image_raw, og_site_name) = if let Some(row) = og_row {
        let image = row.og_image_url.or(prior_image);
        (
            row.og_title,
            row.og_description,
            image,
            row.og_site_name,
        )
    } else {
        (None, None, None, None)
    };

    let image_fields = crate::content::domain_reputation::split_og_image_for_preview(
        og_image_raw.as_deref(),
        &canonical.url_canonical,
        preview_image_allowed,
        body.for_composer,
    );
    let (og_preview_reveal_label, og_preview_withheld_hint) = if image_fields.og_image_reveal_url.is_some() {
        (
            Some(c.t(&loc, "ui.post.og_preview_reveal")),
            Some(c.t(&loc, "ui.post.og_preview_withheld_hint")),
        )
    } else {
        (None, None)
    };

    Ok((
        StatusCode::OK,
        Json(PreviewOgResponse {
            domain_id: up
                .as_ref()
                .map(|x| x.domain_id.to_string())
                .unwrap_or_default(),
            link_id: up
                .as_ref()
                .map(|x| x.link_id.to_string())
                .unwrap_or_default(),
            domain: canonical.registrable_domain,
            url_original: canonical.url_original,
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
        }),
    ))
}

async fn posting_entities(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
) -> AppResult<Json<PostingEntitiesResponse>> {
    let wallet_balance = human_onboarding::ai_tokens_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let rows = sqlx::query_as::<_, (u64, String, String, Option<String>)>(
        r#"
        SELECT DISTINCT
            se.id,
            se.handle,
            rk_type.`key` AS type_key,
            COALESCE(sp.label, snp.label, sg.label) AS label
        FROM social_entity se
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = se.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` IN (?, ?, ?, ?)
        LEFT JOIN user_social_entity_role ur
            ON ur.social_entity_id = se.id
           AND ur.user_id = ?
           AND ur.ends_at IS NULL
        LEFT JOIN ref_key_values rk_role
            ON rk_role.id = ur.role_id
           AND rk_role.`group` = 'user_social_entity_role'
        LEFT JOIN social_profit sp ON sp.social_entity_id = se.id
        LEFT JOIN social_non_profit snp ON snp.social_entity_id = se.id
        LEFT JOIN social_gathering sg ON sg.social_entity_id = se.id
        WHERE
            se.created_by = ?
            OR (
                rk_type.`key` IN (?, ?, ?)
                AND ur.id IS NOT NULL
            )
            OR (
                rk_type.`key` = ?
                AND rk_role.`key` IN ('owner', 'admin')
                AND (sg.ends_at IS NULL OR sg.ends_at >= UTC_TIMESTAMP(6))
            )
        ORDER BY
            CASE rk_type.`key`
                WHEN 'human' THEN 0
                WHEN 'non_profit' THEN 1
                WHEN 'profit' THEN 2
                WHEN 'gathering' THEN 3
                ELSE 9
            END,
            se.handle ASC
        "#,
    )
    .bind(crate::ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(crate::ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .bind(crate::ref_keys::KEY_ENTITY_TYPE_NON_PROFIT)
    .bind(crate::ref_keys::KEY_ENTITY_TYPE_PROFIT)
    .bind(crate::ref_keys::KEY_ENTITY_TYPE_GATHERING)
    .bind(user_id)
    .bind(user_id)
    .bind(crate::ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .bind(crate::ref_keys::KEY_ENTITY_TYPE_NON_PROFIT)
    .bind(crate::ref_keys::KEY_ENTITY_TYPE_PROFIT)
    .bind(crate::ref_keys::KEY_ENTITY_TYPE_GATHERING)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Db)?;

    let entities = rows
        .into_iter()
        .map(|(id, handle, type_key, label)| {
            posting_entity_option(id, handle, type_key, label, None, wallet_balance)
        })
        .collect();

    Ok(Json(PostingEntitiesResponse {
        wallet_balance,
        entities,
    }))
}

async fn search_entities(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Query(q): Query<SearchEntitiesQuery>,
) -> AppResult<Json<Vec<PostingEntityOption>>> {
    let raw = q.q.unwrap_or_default();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Json(Vec::new()));
    }

    let is_handle_mode = trimmed.starts_with('@');
    let needle_raw = if is_handle_mode { &trimmed[1..] } else { trimmed };
    let needle = needle_raw
        .trim()
        .chars()
        .take(32)
        .collect::<String>()
        .to_ascii_lowercase();
    if needle.is_empty() {
        return Ok(Json(Vec::new()));
    }
    let min_ok = if is_handle_mode {
        needle.chars().filter(|c| c.is_ascii_alphanumeric()).count() >= 3
    } else {
        needle.chars().count() >= 3
    };
    if !min_ok {
        return Ok(Json(Vec::new()));
    }

    let community_only = q
        .type_key
        .as_deref()
        .map(str::trim)
        .is_some_and(|k| k.eq_ignore_ascii_case(ref_keys::KEY_ENTITY_TYPE_COMMUNITY));
    let exclude_community_handle = q
        .exclude_community_member
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_start_matches('@').to_ascii_lowercase())
        .filter(|s| !s.is_empty());

    let prefix_like = format!("{needle}%");
    let broad_like = format!("%{needle}%");
    let alive_predicate = public_profile::sql_alive_entity_predicate("se");
    let viewer_human_entity_id =
        human_onboarding::primary_human_entity_id(&state.db, user_id)
            .await
            .map_err(AppError::Db)?;
    let conceal_panoply =
        crate::social::panoply::conceal_panoply_from_viewer(&state.db, user_id)
            .await
            .map_err(AppError::Db)?;
    let panoply_exclude_sql = if conceal_panoply {
        crate::social::panoply::sql_exclude_panoply_entity("se")
    } else {
        String::new()
    };
    let block_sql = viewer_human_entity_id
        .map(|_| blocks::SQL_EXCLUDE_BLOCKED_PEERS)
        .unwrap_or("");
    let avatar_sql = public_profile::sql_avatar_path_expr("se");
    let rows = if let Some(community_handle) = exclude_community_handle.as_deref() {
        let Some(community_entity_id) =
            community_invite::community_entity_id_for_handle(&state.db, community_handle)
                .await
                .map_err(AppError::Db)?
        else {
            return Ok(Json(Vec::new()));
        };
        community_invite::search_invitable_humans(
            &state.db,
            community_entity_id,
            needle.as_str(),
            prefix_like.as_str(),
            broad_like.as_str(),
            viewer_human_entity_id,
        )
        .await
        .map_err(AppError::Db)?
    } else if community_only {
        let parent_search_panoply_exclude =
            crate::social::panoply::sql_exclude_panoply_entity("se");
        sqlx::query_as::<_, (u64, String, String, Option<String>, Option<String>, i64)>(&format!(
            r#"
            SELECT
                se.id,
                se.handle,
                rk_type.`key` AS type_key,
                COALESCE(sc.label, se.handle) AS label,
                {avatar_sql} AS avatar_path,
                (
                    CASE WHEN se.handle = ? THEN 100 ELSE 0 END +
                    CASE WHEN se.handle LIKE ? THEN 60 ELSE 0 END +
                    CASE WHEN COALESCE(sc.label, se.handle) LIKE ? THEN 30 ELSE 0 END +
                    CASE WHEN COALESCE(sc.description, '') LIKE ? THEN 15 ELSE 0 END
                ) AS rank_score
            FROM social_entity se
            INNER JOIN ref_key_values rk_type
                ON rk_type.id = se.type_id
               AND rk_type.`group` = ?
               AND rk_type.`key` = ?
            INNER JOIN social_community sc ON sc.social_entity_id = se.id
            WHERE
                (
                    se.handle LIKE ?
                    OR COALESCE(sc.label, se.handle) LIKE ?
                    OR COALESCE(sc.description, '') LIKE ?
                )
                AND {alive_predicate}
                {parent_search_panoply_exclude}
            ORDER BY
                rank_score DESC,
                CASE WHEN se.handle = ? THEN 0 ELSE 1 END,
                se.handle ASC
            LIMIT 16
            "#,
            alive_predicate = alive_predicate,
            parent_search_panoply_exclude = parent_search_panoply_exclude,
            avatar_sql = avatar_sql,
        ))
        .bind(needle.as_str())
        .bind(prefix_like.as_str())
        .bind(prefix_like.as_str())
        .bind(broad_like.as_str())
        .bind(crate::ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
        .bind(crate::ref_keys::KEY_ENTITY_TYPE_COMMUNITY)
        .bind(prefix_like.as_str())
        .bind(broad_like.as_str())
        .bind(broad_like.as_str())
        .bind(needle.as_str())
        .fetch_all(&state.db)
        .await
        .map_err(AppError::Db)?
    } else {
        let sql = format!(
            r#"
            SELECT
                se.id,
                se.handle,
                rk_type.`key` AS type_key,
                COALESCE(sc.label, sp.label, snp.label, sd.label, sg.label) AS label,
                {avatar_sql} AS avatar_path,
                (
                    CASE WHEN se.handle = ? THEN 100 ELSE 0 END +
                    CASE WHEN se.handle LIKE ? THEN 60 ELSE 0 END +
                    CASE WHEN COALESCE(sc.label, sp.label, snp.label, sd.label, sg.label, se.handle) LIKE ? THEN 30 ELSE 0 END +
                    CASE WHEN COALESCE(sc.description, sp.description, snp.description, sd.description, sg.description, '') LIKE ? THEN 15 ELSE 0 END
                ) AS rank_score
            FROM social_entity se
            INNER JOIN ref_key_values rk_type
                ON rk_type.id = se.type_id
               AND rk_type.`group` = ?
               AND rk_type.`key` IN (?, ?, ?, ?, ?, ?)
            LEFT JOIN social_community sc ON sc.social_entity_id = se.id
            LEFT JOIN social_profit sp ON sp.social_entity_id = se.id
            LEFT JOIN social_non_profit snp ON snp.social_entity_id = se.id
            LEFT JOIN social_digital sd ON sd.social_entity_id = se.id
            LEFT JOIN social_gathering sg ON sg.social_entity_id = se.id
            WHERE
                (
                    se.handle LIKE ?
                    OR COALESCE(sc.label, sp.label, snp.label, sd.label, sg.label, se.handle) LIKE ?
                    OR COALESCE(sc.description, sp.description, snp.description, sd.description, sg.description, '') LIKE ?
                )
                AND {alive_predicate}
                {panoply_exclude_sql}
                {block_sql}
            ORDER BY
                rank_score DESC,
                CASE WHEN se.handle = ? THEN 0 ELSE 1 END,
                CASE
                    WHEN rk_type.`key` = 'human' THEN 0
                    WHEN rk_type.`key` = 'community' THEN 1
                    WHEN rk_type.`key` = 'non_profit' THEN 2
                    WHEN rk_type.`key` = 'profit' THEN 3
                    WHEN rk_type.`key` = 'gathering' THEN 4
                    ELSE 9
                END,
                se.handle ASC
            LIMIT 16
            "#,
            alive_predicate = alive_predicate,
            panoply_exclude_sql = panoply_exclude_sql,
            block_sql = block_sql,
            avatar_sql = avatar_sql,
        );
        let mut q = sqlx::query_as::<_, (u64, String, String, Option<String>, Option<String>, i64)>(&sql)
        .bind(needle.as_str())
        .bind(prefix_like.as_str())
        .bind(prefix_like.as_str())
        .bind(broad_like.as_str())
        .bind(crate::ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
        .bind(crate::ref_keys::KEY_ENTITY_TYPE_HUMAN)
        .bind(crate::ref_keys::KEY_ENTITY_TYPE_DIGITAL)
        .bind(crate::ref_keys::KEY_ENTITY_TYPE_COMMUNITY)
        .bind(crate::ref_keys::KEY_ENTITY_TYPE_NON_PROFIT)
        .bind(crate::ref_keys::KEY_ENTITY_TYPE_PROFIT)
        .bind(crate::ref_keys::KEY_ENTITY_TYPE_GATHERING)
        .bind(prefix_like.as_str())
        .bind(broad_like.as_str())
        .bind(broad_like.as_str())
        .bind(needle.as_str());
        let mut q = q;
        if let Some(viewer_human_id) = viewer_human_entity_id {
            q = q
                .bind(crate::ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
                .bind(crate::ref_keys::KEY_RELATIONSHIP_FRIEND)
                .bind(crate::ref_keys::KEY_RELATIONSHIP_FAMILY)
                .bind(crate::ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
                .bind(crate::ref_keys::GROUP_STATUS)
                .bind(crate::ref_keys::KEY_BLOCKED)
                .bind(viewer_human_id)
                .bind(viewer_human_id);
        }
        q.fetch_all(&state.db)
        .await
        .map_err(AppError::Db)?
    };

    let options = rows
        .into_iter()
        .filter(|(_, handle, _, _, _, _)| {
            !conceal_panoply || !crate::social::panoply::is_panoply_community_handle(handle)
        })
        .map(|(id, handle, type_key, label, avatar_path, _rank_score)| {
            search_entity_option(id, handle, type_key, label, avatar_path)
        })
        .collect();
    Ok(Json(options))
}

fn search_entity_option(
    id: u64,
    handle: String,
    type_key: String,
    label: Option<String>,
    avatar_path: Option<String>,
) -> PostingEntityOption {
    posting_entity_option(id, handle, type_key, label, avatar_path, u16::MAX)
}

async fn effective_constitution_preview(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: _user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Query(q): Query<EffectiveConstitutionQuery>,
) -> AppResult<Json<EffectiveConstitutionResponse>> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let raw = q.handle.unwrap_or_default();
    let normalized = match entity::classify_handle_candidate(&raw) {
        entity::HandleCheck::Valid(h) => h,
        _ => {
            return Ok(Json(EffectiveConstitutionResponse {
                html: String::new(),
                empty: true,
            }));
        }
    };

    let row: Option<(u64,)> = sqlx::query_as(
        r#"
        SELECT sc.social_entity_id
        FROM social_entity se
        INNER JOIN social_community sc ON sc.social_entity_id = se.id
        WHERE se.handle = ?
        LIMIT 1
        "#,
    )
    .bind(normalized.as_str())
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;

    let Some((community_entity_id,)) = row else {
        return Ok(Json(EffectiveConstitutionResponse {
            html: String::new(),
            empty: true,
        }));
    };

    let sections =
        community_constitution::fetch_effective_constitution(&state.db, community_entity_id)
            .await
            .map_err(AppError::Db)?;
    let html = private_profile_card::community_constitution_block_html(&sections, &loc, &c);
    Ok(Json(EffectiveConstitutionResponse { html, empty: false }))
}

async fn check_entity_handle_availability(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: _user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Query(q): Query<HandleAvailabilityQuery>,
) -> AppResult<Json<HandleAvailabilityResponse>> {
    let raw = q.handle.unwrap_or_default();
    let check = entity::classify_handle_candidate(&raw);
    let (normalized_handle, status) = match check {
        entity::HandleCheck::TooShort => (String::new(), "too_short".to_string()),
        entity::HandleCheck::Invalid => (String::new(), "invalid".to_string()),
        entity::HandleCheck::Reserved => (String::new(), "reserved".to_string()),
        entity::HandleCheck::Valid(handle) => {
            let exists: Option<u64> =
                sqlx::query_scalar("SELECT id FROM social_entity WHERE handle = ? LIMIT 1")
                    .bind(handle.as_str())
                    .fetch_optional(&state.db)
                    .await
                    .map_err(AppError::Db)?;
            let status = if exists.is_some() { "taken" } else { "available" };
            (handle, status.to_string())
        }
    };
    Ok(Json(HandleAvailabilityResponse {
        normalized_handle,
        status,
    }))
}

async fn create_human_profile(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Json(body): Json<human_onboarding::CreateHumanProfileRequest>,
) -> AppResult<(StatusCode, Json<entity::Public>)> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    if !human_onboarding::user_has_terms_consent(&state.db, user_id).await? {
        return Err(AppError::BadRequest(
            c.t(&loc, "auth.must_confirm_constitution"),
        ));
    }
    let row = human_onboarding::create_human_profile(&state.db, user_id, body).await?;
    Ok((StatusCode::CREATED, Json(row)))
}

#[derive(Debug, Deserialize)]
struct TermsConsentBody {
    adult_ack: bool,
    constitution_ack: bool,
}

async fn accept_terms_consent(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Json(body): Json<TermsConsentBody>,
) -> AppResult<Json<serde_json::Value>> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    if !body.adult_ack {
        return Err(AppError::BadRequest(c.t(&loc, "auth.must_confirm_adult")));
    }
    if !body.constitution_ack {
        return Err(AppError::BadRequest(
            c.t(&loc, "auth.must_confirm_constitution"),
        ));
    }
    human_onboarding::record_terms_consent(&state.db, user_id).await?;
    let next = post_auth::next_path_after_auth(&state.db, user_id, state.config.face_vision_mock)
        .await
        .map_err(AppError::Db)?;
    Ok(Json(serde_json::json!({ "ok": true, "next": next })))
}

async fn update_human_profile(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    headers: HeaderMap,
    Json(body): Json<human_onboarding::UpdateHumanProfileRequest>,
) -> AppResult<Json<entity::Public>> {
    profile_gate::require_profile_mutation(&state.db, &state.config, user_id, &headers).await?;
    let google = state
        .config
        .google_maps_api_key
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|key| human_onboarding::HumanProfileGoogleLocationConfig {
            api_key: key,
            timeout: Duration::from_millis(state.config.google_places_timeout_ms.clamp(1_000, 30_000)),
        });
    let row = human_onboarding::update_human_profile(&state.db, user_id, body, google).await?;
    Ok(Json(row))
}

async fn update_managed_entity_profile(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    headers: HeaderMap,
    Path(handle): Path<String>,
    Json(body): Json<entity_profile_edit::UpdateEntityProfileRequest>,
) -> AppResult<Json<entity::Public>> {
    profile_gate::require_profile_mutation(&state.db, &state.config, user_id, &headers).await?;
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(normalized) = entity::normalize_handle_from_path_segment(handle.trim()) else {
        return Err(AppError::BadRequest(
            c.t(&loc, "social_entity.handle_invalid"),
        ));
    };
    let row =
        entity_profile_edit::update_entity_profile_by_handle(
            &state.db,
            &state.config,
            user_id,
            &normalized,
            body,
        )
            .await?;
    Ok(Json(row))
}

async fn delete_managed_entity(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    headers: HeaderMap,
    Path(handle): Path<String>,
    Json(body): Json<entity_profile_edit::DeleteEntityBody>,
) -> AppResult<Json<serde_json::Value>> {
    profile_gate::require_profile_mutation(&state.db, &state.config, user_id, &headers).await?;
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    if !body.confirm {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.entity_delete_confirm_required"),
        ));
    }
    let password = body.password.trim();
    if password.is_empty() {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.account_password_required"),
        ));
    }
    let Some(normalized) = entity::normalize_handle_from_path_segment(handle.trim()) else {
        return Err(AppError::BadRequest(
            c.t(&loc, "social_entity.handle_invalid"),
        ));
    };
    entity_profile_edit::delete_entity_by_handle(&state.db, user_id, &normalized, password).await?;
    Ok(Json(serde_json::json!({ "ok": true, "redirect": "/home" })))
}

#[derive(Debug, Deserialize)]
struct FriendInviteBody {
    email: String,
    #[serde(default)]
    personal_message: String,
}

async fn send_friend_invite(
    State(state): State<AppState>,
    Extension(VerifiedSession {
        user_id: user_id,
        password_reset_only: _,
    }): Extension<VerifiedSession>,
    headers: HeaderMap,
    Json(body): Json<FriendInviteBody>,
) -> AppResult<Json<serde_json::Value>> {
    profile_gate::require_profile_mutation(&state.db, &state.config, user_id, &headers).await?;
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    match recruitment::precheck_friend_invite_email(&state.db, user_id, &body.email).await? {
        recruitment::FriendInvitePrecheck::MaySend => {
            recruitment::invite_to_platform_as_viewer_human(
                &state.db,
                &state.config,
                user_id,
                &body.email,
                Some(body.personal_message.trim()).filter(|s| !s.is_empty()),
            )
            .await?;
            Ok(Json(serde_json::json!({
                "ok": true,
                "message": c.t(&loc, "ui.home.friend_invite_sent"),
            })))
        }
        recruitment::FriendInvitePrecheck::AlreadyOnPlatform { handle } => {
            let show_handle = format!("@{handle}");
            Ok(Json(serde_json::json!({
                "ok": true,
                "already_member": true,
                "handle": show_handle,
                "message": c.tf(
                    &loc,
                    "ui.home.friend_invite_already_on_platform",
                    &[("handle", show_handle.as_str())],
                ),
            })))
        }
        recruitment::FriendInvitePrecheck::ExistingAccountUnavailable => Err(AppError::BadRequest(
            c.t(&loc, "ui.home.friend_invite_email_registered"),
        )),
    }
}

#[derive(Debug, Deserialize, Default)]
struct RecruitmentInvitesBody {
    #[serde(default)]
    emails: Vec<String>,
    #[serde(default)]
    emails_text: String,
    #[serde(default)]
    personal_message: String,
}

async fn send_entity_recruitment_invites(
    State(state): State<AppState>,
    Extension(VerifiedSession {
        user_id: user_id,
        password_reset_only: _,
    }): Extension<VerifiedSession>,
    headers: HeaderMap,
    Path(handle): Path<String>,
    Json(body): Json<RecruitmentInvitesBody>,
) -> AppResult<Json<serde_json::Value>> {
    profile_gate::require_profile_mutation(&state.db, &state.config, user_id, &headers).await?;
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let summary = recruitment::invite_to_platform_bulk_by_handle(
        &state.db,
        &state.config,
        user_id,
        &handle,
        &body.emails,
        &body.emails_text,
        Some(body.personal_message.trim()).filter(|s| !s.is_empty()),
    )
    .await?;
    let message = c.tf(
        &loc,
        "ui.entity_private.recruitment_invite_bulk_result",
        &[
            ("sent", &summary.sent.to_string()),
            ("failed", &summary.failed.to_string()),
            ("skipped", &summary.skipped_invalid.to_string()),
        ],
    );
    Ok(Json(serde_json::json!({
        "ok": true,
        "message": message,
        "sent": summary.sent,
        "failed": summary.failed,
        "skipped_invalid": summary.skipped_invalid,
    })))
}

async fn require_posting_rights(pool: &sqlx::MySqlPool, user_id: u64) -> AppResult<()> {
    if status::user_has_verified_status(pool, user_id)
        .await
        .map_err(AppError::Db)?
    {
        return Ok(());
    }
    Err(AppError::BadRequest(
        "Complete profile setup before posting: email verification, human profile, and face capture."
            .to_string(),
    ))
}

/// Max JPEG per pose; 512×512 photos should stay well under this.
const MAX_FACE_JPEG_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Deserialize, Default)]
struct FaceCaptureMeta {
    #[serde(default)]
    quality_score: Option<f32>,
    #[serde(default)]
    camera_width: Option<u32>,
    #[serde(default)]
    camera_height: Option<u32>,
    #[serde(default)]
    crop_source_side_px: Option<f32>,
    #[serde(default)]
    face_height_ratio: Option<f32>,
}

#[derive(Debug, Serialize)]
struct FaceCaptureQueuedResponse {
    ok: bool,
    pending_id: String,
    status: String,
}

#[derive(Debug, Serialize)]
struct FaceCaptureStatusResponse {
    ok: bool,
    pending_id: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    stage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    noms_spent: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    noms_balance: Option<u16>,
}

#[derive(Debug, Serialize)]
struct AvatarRedrawQueuedResponse {
    ok: bool,
    pending_id: String,
    status: String,
}

#[derive(Debug, Serialize)]
struct GodmotherOption {
    handle: String,
    label: String,
    avatar_path: String,
    community_label: String,
}

#[derive(Debug, Deserialize)]
struct SelectGodmotherBody {
    handle: String,
}

#[derive(Debug, Serialize)]
struct SelectGodmotherResponse {
    ok: bool,
    godmother_handle: String,
    community_handle: String,
}

fn validate_jpeg_prefix(data: &[u8]) -> bool {
    data.len() >= 500 && data[0] == 0xff && data[1] == 0xd8
}

/// Three expression photos plus biometric-free metadata, read from a multipart upload.
struct FaceCaptureUpload {
    photos: PortraitureFacePhotos,
    quality_score: Option<f32>,
}

/// Read + validate the three capture JPEGs (neutral/smile/frown) and optional metadata from a
/// multipart body, running the media safety hash-gate. Shared by onboarding capture and paid redraw.
async fn read_face_capture_upload(
    state: &AppState,
    user_id: u64,
    mut multipart: Multipart,
) -> Result<FaceCaptureUpload, AppError> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();

    let mut meta_json: Option<String> = None;
    let mut neutral: Option<Vec<u8>> = None;
    let mut smile: Option<Vec<u8>> = None;
    let mut frown: Option<Vec<u8>> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("multipart: {e}")))?
    {
        let name = field.name().unwrap_or("").to_string();
        let data = field
            .bytes()
            .await
            .map_err(|e| AppError::BadRequest(format!("multipart field: {e}")))?;

        match name.as_str() {
            "meta" => {
                meta_json = Some(String::from_utf8(data.to_vec()).map_err(|_| {
                    AppError::BadRequest(c.t(&loc, "ui.face_capture.bad_meta_utf8"))
                })?);
            }
            "neutral" | "smile" | "frown" => {
                if data.len() > MAX_FACE_JPEG_BYTES {
                    return Err(AppError::BadRequest(
                        c.t(&loc, "ui.face_capture.image_too_large"),
                    ));
                }
                if !validate_jpeg_prefix(&data) {
                    return Err(AppError::BadRequest(c.t(&loc, "ui.face_capture.bad_image")));
                }
                match name.as_str() {
                    "neutral" => neutral = Some(data.to_vec()),
                    "smile" => smile = Some(data.to_vec()),
                    "frown" => frown = Some(data.to_vec()),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    // Metadata is optional and biometric-free (quality score only).
    let meta: FaceCaptureMeta = match meta_json {
        Some(s) => serde_json::from_str(&s)
            .map_err(|_| AppError::BadRequest(c.t(&loc, "ui.face_capture.bad_meta_json")))?,
        None => FaceCaptureMeta::default(),
    };

    let neutral = neutral
        .ok_or_else(|| AppError::BadRequest(c.t(&loc, "ui.face_capture.missing_neutral")))?;
    let smile =
        smile.ok_or_else(|| AppError::BadRequest(c.t(&loc, "ui.face_capture.missing_smile")))?;
    let frown =
        frown.ok_or_else(|| AppError::BadRequest(c.t(&loc, "ui.face_capture.missing_frown")))?;

    let quality_meta = crate::social::face_capture_quality::FaceCaptureQualityMeta {
        crop_source_side_px: meta.crop_source_side_px,
        face_height_ratio: meta.face_height_ratio,
    };

    for (label, jpeg) in [
        ("neutral", &neutral),
        ("smile", &smile),
        ("frown", &frown),
    ] {
        if let Err(e) = crate::social::face_capture_quality::validate_face_capture_jpeg(
            jpeg, quality_meta, &loc, &c,
        ) {
            tracing::warn!(pose = label, error = %e, "face capture quality gate rejected upload");
            return Err(e);
        }
    }

    if !state.config.face_vision_mock {
        crate::social::face_capture_diversity::assert_minimum_capture_diversity(
            &neutral,
            &smile,
            &frown,
            &loc,
            &c,
        )?;
    }

    for (label, jpeg) in [
        ("neutral", &neutral),
        ("smile", &smile),
        ("frown", &frown),
    ] {
        if let safety::MediaScanOutcome::Blocked {
            harm_class,
            sha256_hex,
        } = safety::scan_jpeg_bytes(jpeg, &state.config.media_hash_blocklist)
        {
            safety::sanitation::log_hash_gate_block(
                &state.db,
                "face_capture",
                harm_class.as_str(),
                &sha256_hex,
                Some(user_id),
            )
            .await;
            return Err(crate::social::face_capture_errors::face_capture_coded(
                &loc,
                &c,
                "face_media_blocked",
                "social.media_blocked_safety",
            ));
        }
    }

    let photos = PortraitureFacePhotos {
        neutral,
        smile,
        frown,
    };

    Ok(FaceCaptureUpload {
        photos,
        quality_score: meta.quality_score,
    })
}

async fn face_capture(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    multipart: Multipart,
) -> AppResult<(StatusCode, Json<FaceCaptureQueuedResponse>)> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    if !human_onboarding::user_has_terms_consent(&state.db, user_id).await? {
        return Err(AppError::BadRequest(
            c.t(&loc, "auth.must_confirm_constitution"),
        ));
    }

    let upload = read_face_capture_upload(&state, user_id, multipart).await?;

    let human_entity_id = human_onboarding::primary_human_entity_id(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;

    let queued = face_capture_pending::queue_face_capture(
        &state.db,
        &state.config,
        user_id,
        human_entity_id,
        face_capture_pending::PendingBillingMode::Onboarding,
        &upload.photos,
        upload.quality_score,
    )
    .await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(FaceCaptureQueuedResponse {
            ok: true,
            pending_id: queued.pending_id,
            status: queued.status,
        }),
    ))
}

async fn face_capture_status(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(pending_id): Path<String>,
) -> AppResult<Json<FaceCaptureStatusResponse>> {
    let snapshot = face_capture_pending::status_for_user(&state.db, user_id, &pending_id)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::NotFound)?;

    Ok(Json(FaceCaptureStatusResponse {
        ok: true,
        pending_id: snapshot.pending_id,
        status: snapshot.status,
        stage: snapshot.stage,
        error: snapshot.error,
        code: snapshot.code,
        id: snapshot.id,
        avatar_path: snapshot.avatar_path,
        noms_spent: snapshot.noms_spent,
        noms_balance: snapshot.noms_balance,
    }))
}

async fn avatar_redraw(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    multipart: Multipart,
) -> AppResult<(StatusCode, Json<AvatarRedrawQueuedResponse>)> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();

    if state.config.face_vision_mock {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.avatar_redraw_unavailable_mock"),
        ));
    }

    let human_entity_id = human_onboarding::primary_human_entity_id(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
        .ok_or_else(|| {
            AppError::BadRequest(c.t(&loc, "ui.entity_private.human_profile_required"))
        })?;

    if !user_faces::user_has_face_capture(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.avatar_redraw_face_required"),
        ));
    }

    let upload = read_face_capture_upload(&state, user_id, multipart).await?;

    let queued = face_capture_pending::queue_face_capture(
        &state.db,
        &state.config,
        user_id,
        Some(human_entity_id),
        face_capture_pending::PendingBillingMode::AvatarRedraw,
        &upload.photos,
        upload.quality_score,
    )
    .await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(AvatarRedrawQueuedResponse {
            ok: true,
            pending_id: queued.pending_id,
            status: queued.status,
        }),
    ))
}

async fn godmother_options(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: _user_id, password_reset_only: _ }): Extension<VerifiedSession>,
) -> AppResult<Json<Vec<GodmotherOption>>> {
    let rows = sqlx::query_as::<_, (String, Option<String>, Option<String>, String)>(
        r#"
        SELECT
            aa.slug,
            sd.label,
            uf.generated_artwork_path,
            sc.label
        FROM ai_actor aa
        INNER JOIN social_entity ai_se ON ai_se.id = aa.social_entity_id
        LEFT JOIN social_digital sd ON sd.social_entity_id = ai_se.id
        LEFT JOIN user_faces uf ON uf.user_id = aa.user_id
           AND uf.face_captured_at IS NOT NULL
           AND uf.generated_artwork_path IS NOT NULL
           AND uf.generated_artwork_path <> ''
        INNER JOIN social_entity_relationship modr ON modr.from_social_entity_id = ai_se.id AND modr.ended_at IS NULL
        INNER JOIN ref_key_values modr_type ON modr_type.id = modr.type_id
        INNER JOIN social_entity cse ON cse.id = modr.to_social_entity_id
        INNER JOIN social_community sc ON sc.social_entity_id = cse.id
        WHERE modr_type.`group` = ? AND modr_type.`key` = ?
        ORDER BY aa.slug ASC
        "#,
    )
    .bind(crate::ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(crate::ref_keys::KEY_RELATIONSHIP_MODERATOR)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Db)?;

    let out = rows
        .into_iter()
        .map(|(slug, label, avatar, community_label)| GodmotherOption {
            handle: slug.clone(),
            label: label.unwrap_or(slug.clone()),
            avatar_path: avatar.unwrap_or_else(|| {
                format!("static/generated/council/{slug}/avatar-default.svg")
            }),
            community_label,
        })
        .collect();
    Ok(Json(out))
}

async fn select_godmother(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Json(body): Json<SelectGodmotherBody>,
) -> AppResult<(StatusCode, Json<SelectGodmotherResponse>)> {
    let human_entity_id = human_onboarding::primary_human_entity_id(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
        .ok_or_else(|| {
            AppError::BadRequest("Create a human profile before choosing a godmother.".to_string())
        })?;

    if !user_faces::user_has_face_capture(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            "Complete face capture before choosing a godmother.".to_string(),
        ));
    }

    let mut tx = state.db.begin().await.map_err(AppError::Db)?;

    let existing: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT ser.id
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values t ON t.id = ser.type_id
        WHERE ser.from_social_entity_id = ?
          AND ser.ended_at IS NULL
          AND t.`group` = ?
          AND t.`key` = ?
        LIMIT 1
        "#,
    )
    .bind(human_entity_id)
    .bind(crate::ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(crate::ref_keys::KEY_RELATIONSHIP_GODMOTHER)
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::Db)?;
    if existing.is_some() {
        return Err(AppError::Conflict(
            "Godmother already selected for this user.".to_string(),
        ));
    }

    let chosen = body.handle.trim().to_ascii_lowercase();
    let pair: Option<(u64, u64, String)> = sqlx::query_as(
        r#"
        SELECT ai_se.id, cse.id, cse.handle
        FROM ai_actor aa
        INNER JOIN social_entity ai_se ON ai_se.id = aa.social_entity_id
        INNER JOIN social_entity_relationship modr ON modr.from_social_entity_id = ai_se.id AND modr.ended_at IS NULL
        INNER JOIN ref_key_values t ON t.id = modr.type_id
        INNER JOIN social_entity cse ON cse.id = modr.to_social_entity_id
        WHERE aa.slug = ?
          AND t.`group` = ?
          AND t.`key` = ?
        LIMIT 1
        "#,
    )
    .bind(&chosen)
    .bind(crate::ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(crate::ref_keys::KEY_RELATIONSHIP_MODERATOR)
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::Db)?;
    let Some((godmother_entity_id, community_entity_id, community_handle)) = pair else {
        return Err(AppError::BadRequest(
            "Unknown godmother handle.".to_string(),
        ));
    };

    let rel_godmother: u64 = ref_key_values::lookup_active_id(
        &state.db,
        crate::ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE,
        crate::ref_keys::KEY_RELATIONSHIP_GODMOTHER,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| {
        AppError::BadRequest("godmother relationship type is not configured".to_string())
    })?;
    let rel_member_of: u64 = ref_key_values::lookup_active_id(
        &state.db,
        crate::ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE,
        crate::ref_keys::KEY_RELATIONSHIP_MEMBER_OF,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| {
        AppError::BadRequest("member_of relationship type is not configured".to_string())
    })?;
    let status_verified: u64 = ref_key_values::lookup_active_id(
        &state.db,
        crate::ref_keys::GROUP_STATUS,
        crate::ref_keys::KEY_VERIFIED,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest("verified status is not configured".to_string()))?;

    sqlx::query(
        r#"
        INSERT INTO social_entity_relationship (
            from_social_entity_id, to_social_entity_id, type_id, status_id, relationship_status_id,
            started_at, ended_at, created_by, modified_by
        )
        VALUES (
            ?, ?, ?, ?,
            (
                SELECT scoped.id
                FROM ref_key_values old_status
                INNER JOIN ref_key_values scoped
                    ON scoped.`group` = 'social_relationship_status'
                   AND scoped.`key` = old_status.`key`
                WHERE old_status.id = ?
                LIMIT 1
            ),
            UTC_DATE(), NULL, ?, ?
        )
        "#,
    )
    .bind(human_entity_id)
    .bind(godmother_entity_id)
    .bind(rel_godmother)
    .bind(status_verified)
    .bind(status_verified)
    .bind(user_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;
    social_scope::mark_relationship_endpoints_dirty_in_tx(
        &mut tx,
        human_entity_id,
        godmother_entity_id,
    )
    .await
    .map_err(AppError::Db)?;

    sqlx::query(
        r#"
        INSERT INTO social_entity_relationship (
            from_social_entity_id, to_social_entity_id, type_id, status_id, relationship_status_id,
            started_at, ended_at, created_by, modified_by
        )
        VALUES (
            ?, ?, ?, ?,
            (
                SELECT scoped.id
                FROM ref_key_values old_status
                INNER JOIN ref_key_values scoped
                    ON scoped.`group` = 'social_relationship_status'
                   AND scoped.`key` = old_status.`key`
                WHERE old_status.id = ?
                LIMIT 1
            ),
            UTC_DATE(), NULL, ?, ?
        )
        "#,
    )
    .bind(human_entity_id)
    .bind(community_entity_id)
    .bind(rel_member_of)
    .bind(status_verified)
    .bind(status_verified)
    .bind(user_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;
    social_scope::mark_relationship_endpoints_dirty_in_tx(
        &mut tx,
        human_entity_id,
        community_entity_id,
    )
    .await
    .map_err(AppError::Db)?;

    tx.commit().await.map_err(AppError::Db)?;
    status::try_promote_user_to_verified_if_composite(&state.db, user_id, Some(user_id)).await?;

    if let Err(err) = crate::social::godmother::enqueue_moment(
        &state.db,
        Some(user_id),
        human_entity_id,
        crate::social::godmother::GodmotherMoment::Welcome,
        Some("select_godmother"),
    )
    .await
    {
        tracing::warn!(
            human_entity_id,
            error = %err,
            "godmother welcome outreach job enqueue failed"
        );
    }

    Ok((
        StatusCode::CREATED,
        Json(SelectGodmotherResponse {
            ok: true,
            godmother_handle: chosen,
            community_handle,
        }),
    ))
}

async fn create_entity(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Json(body): Json<entity::CreateRequest>,
) -> AppResult<(StatusCode, Json<entity::Public>)> {
    require_posting_rights(&state.db, user_id).await?;
    let row = entity::create(&state.db, &state.config, body, user_id).await?;
    Ok((StatusCode::CREATED, Json(row)))
}

async fn get_entity(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<entity::Public>> {
    let id = ids::parse_u64(id.trim(), "id", "social_entity.invalid_id")?;
    let row = entity::get_by_id(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

async fn create_relationship(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Json(body): Json<relationship::CreateRequest>,
) -> AppResult<(StatusCode, Json<relationship::Public>)> {
    require_posting_rights(&state.db, user_id).await?;
    let row = relationship::create(&state.db, body, user_id).await?;
    Ok((StatusCode::CREATED, Json(row)))
}

async fn get_relationship(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<relationship::Public>> {
    let id = ids::parse_u64(id.trim(), "id", "social_entity_relationship.invalid_id")?;
    let row = relationship::get_by_id(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

#[derive(Debug, Deserialize)]
struct ChangePasswordBody {
    current_password: String,
    new_password: String,
}

#[derive(Debug, Deserialize)]
struct DeleteAccountBody {
    password: String,
    confirm: bool,
}

async fn change_password(
    State(state): State<AppState>,
    Extension(VerifiedSession {
        user_id,
        password_reset_only: reset_only,
    }): Extension<VerifiedSession>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(body): Json<ChangePasswordBody>,
) -> AppResult<(CookieJar, (StatusCode, Json<serde_json::Value>))> {
    profile_gate::require_password_change_mutation(
        &state.db,
        &state.config,
        user_id,
        &headers,
        reset_only,
    )
    .await?;
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let current = body.current_password.trim();
    let new_password = body.new_password.trim();
    if new_password.is_empty() {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.account_password_required"),
        ));
    }
    if !reset_only && current.is_empty() {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.account_password_required"),
        ));
    }
    if new_password.len() < MIN_PASSWORD_LEN {
        let min = MIN_PASSWORD_LEN.to_string();
        return Err(AppError::BadRequest(
            c.tf(&loc, "auth.password_too_short", &[("min_len", &min)]),
        ));
    }
    if reset_only {
        auth_account::set_user_password(&state.db, user_id, new_password).await?;
    } else {
        auth_account::change_user_password(&state.db, user_id, current, new_password)
            .await
            .map_err(|e| match e {
                AppError::Unauthorized => {
                    AppError::BadRequest(c.t(&loc, "ui.entity_private.account_password_incorrect"))
                }
                other => other,
            })?;
    }
    let (cookie, redirect) = if reset_only {
        let (cookie, login_outcome) = session::complete_password_reset_session(
            &state.db,
            user_id,
            state.config.secure_cookies,
        )
        .await
        .map_err(AppError::Db)?;
        let redirect = post_auth::next_path_after_login(
            &state.db,
            user_id,
            state.config.face_vision_mock,
            &login_outcome,
        )
        .await
        .map_err(AppError::Db)?;
        (cookie, redirect)
    } else {
        let cookie = session::rotate_session_after_credential_change(
            &state.db,
            user_id,
            state.config.secure_cookies,
        )
        .await
        .map_err(AppError::Db)?;
        (cookie, String::new())
    };
    Ok((
        jar.add(cookie),
        (
            StatusCode::OK,
            Json(serde_json::json!({ "ok": true, "redirect": redirect })),
        ),
    ))
}

async fn delete_my_account(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(body): Json<DeleteAccountBody>,
) -> AppResult<(CookieJar, (StatusCode, Json<serde_json::Value>))> {
    profile_gate::require_profile_mutation(&state.db, &state.config, user_id, &headers).await?;
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    if !body.confirm {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.account_delete_confirm_required"),
        ));
    }
    let password = body.password.trim();
    if password.is_empty() {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.account_password_required"),
        ));
    }
    let ok = auth_account::verify_user_password(&state.db, user_id, password).await?;
    if !ok {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.account_password_incorrect"),
        ));
    }

    let human_entity_id = human_onboarding::primary_human_entity_id(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
        .ok_or_else(|| {
            AppError::BadRequest(c.t(&loc, "ui.entity_private.human_profile_required"))
        })?;

    if community_graph::is_human_death_immune(&state.db, human_entity_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.account_delete_not_allowed"),
        ));
    }

    let mut tx = state.db.begin().await.map_err(AppError::Db)?;
    let deleted = community_graph::human_death_voluntary_tx(&mut tx, human_entity_id, user_id)
        .await
        .map_err(AppError::Db)?;
    if !deleted {
        tx.rollback().await.map_err(AppError::Db)?;
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.account_delete_failed"),
        ));
    }
    tx.commit().await.map_err(AppError::Db)?;

    Ok((
        jar.remove(session::clear_session_cookie(state.config.secure_cookies)),
        (
            StatusCode::OK,
            Json(serde_json::json!({ "ok": true, "redirect": "/login" })),
        ),
    ))
}

#[derive(Serialize)]
struct BlockListEntry {
    handle: String,
    display_name: Option<String>,
}

async fn list_my_blocks(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
) -> AppResult<Json<Vec<BlockListEntry>>> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(viewer_human_id) = human_onboarding::primary_human_entity_id(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.human_profile_required"),
        ));
    };
    let rows = blocks::list_blocked_by(&state.db, viewer_human_id)
        .await
        .map_err(AppError::Db)?;
    Ok(Json(
        rows.into_iter()
            .map(|row| BlockListEntry {
                handle: row.handle,
                display_name: row.display_name,
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
struct UnblockBody {
    handle: String,
}

async fn unblock_user(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    headers: HeaderMap,
    Json(body): Json<UnblockBody>,
) -> AppResult<Json<serde_json::Value>> {
    profile_gate::require_profile_mutation(&state.db, &state.config, user_id, &headers).await?;
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(normalized) = entity::normalize_handle_from_path_segment(body.handle.trim()) else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.block_unavailable"),
        ));
    };
    let Some(viewer_human_id) = human_onboarding::primary_human_entity_id(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.human_profile_required"),
        ));
    };
    let target_entity_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        WHERE se.handle = ?
        LIMIT 1
        "#,
    )
    .bind(normalized.as_str())
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    let Some(target_entity_id) = target_entity_id else {
        return Err(AppError::NotFound);
    };
    blocks::unblock_human(&state.db, viewer_human_id, target_entity_id, user_id)
        .await
        .map_err(AppError::Db)?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "message": c.t(&loc, "ui.entity_private.unblock_success"),
    })))
}
