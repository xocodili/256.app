//! [`SocialEntityPublic`]: base on-site presence (people, orgs, events extend from here).

use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::MySql;
use sqlx::MySqlPool;
use sqlx::Transaction;

use super::connections;
use super::db;
use super::ids;
use super::scope;
use crate::content::sanitize as content_sanitize;
use crate::db::TimestampUtc;
use crate::error::{AppError, AppResult};
use crate::i18n;
use crate::locations::{self, CreateLocationRequest, ResponseCoordinatePrecision};
use crate::pricing::BillableService;
use crate::ref_key_values;
use crate::ref_keys;
use crate::social::human_onboarding;
use crate::social::noms::{self, SpendOutcome};
use crate::ui::nom_icon;
use crate::ui::countries;

const I18N_INVALID_ID: &str = "social_entity.invalid_id";
const MAX_PROFIT_TEXT_FIELD: usize = 4000;
/// Tagline / intention line on profit entities — keep short (aligned with legal-name style inputs).
const MAX_PROFIT_TAGLINE: usize = 512;
const MAX_COMMUNITY_SEED_CLAUSE: usize = 2000;
const DIGITAL_CREATE_SEED_ONLY_MSG: &str =
    "Digital entities are seed-managed and cannot be created from this endpoint.";

/// Handles reserved so they cannot be claimed as `social_entity.handle` (routing and UX).
const RESERVED_HANDLES: &[&str] = &[
    "about",
    "account",
    "admin",
    "api",
    "auth",
    "health",
    "help",
    "home",
    "friends",
    "family",
    "colleague",
    "legal",
    "login",
    "logout",
    "me",
    "moderator",
    "mod",
    "null",
    "p",
    "privacy",
    "public",
    "ref-key-values",
    "register",
    "root",
    "private",
    "social-entities",
    "social-entity-relationships",
    "staff",
    "static",
    "support",
    "system",
    "terms",
    "undefined",
    "verify-email",
    "www",
    "you",
    "locations",
];

/// Optional payload when `type_id` is **non_profit** (`social_entity_type.non_profit`).
#[derive(Debug, Deserialize)]
pub struct CreateNonProfitExtension {
    /// Public-facing name (`social_non_profit.label`).
    pub label: String,
    /// Registered legal name where applicable (`social_non_profit.legal_name`).
    pub legal_name: String,
    /// Required `YYYY-MM-DD` for `social_non_profit.founded_at`.
    pub founded_at: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub intention: Option<String>,
    #[serde(default)]
    pub website_url: Option<String>,
    /// Logo image URL (canonicalized via `content_link`); optional.
    #[serde(default)]
    pub logo_url: Option<String>,
    #[serde(default)]
    pub owner_handles: Option<String>,
}

/// For-profit extension: required when `type_id` is **profit** (`social_entity_type.profit`).
#[derive(Debug, Deserialize)]
pub struct CreateProfitExtension {
    /// Trading / public-facing name (`social_profit.label`).
    pub label: String,
    /// Registered legal name (`social_profit.legal_name`).
    pub legal_name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub intention: Option<String>,
    /// Required `YYYY-MM-DD` for `social_profit.founded_at`.
    pub founded_at: String,
    #[serde(default)]
    pub website_url: Option<String>,
    /// Direct `https://` URL to a self-hosted logo image (optional).
    #[serde(default)]
    pub logo_url: Option<String>,
    #[serde(default)]
    pub owner_handles: Option<String>,
}

/// Gathering/event extension: used when `type_id` is **gathering**.
#[derive(Debug, Deserialize)]
pub struct CreateGatheringExtension {
    /// Event/gathering display name.
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub intention: Option<String>,
    /// External event management / tickets portal URL.
    #[serde(default)]
    pub event_url: Option<String>,
    #[serde(default)]
    pub owner_handles: Option<String>,
}

/// Geographic anchor for a locale-scoped community (`social_entity.location_id`).
#[derive(Debug, Deserialize)]
pub struct CreateCommunityLocation {
    pub country_code: String,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub locality: Option<String>,
}

/// Community extension: used when `type_id` is **community**.
#[derive(Debug, Deserialize)]
pub struct CreateCommunityExtension {
    /// Community display name.
    pub label: String,
    /// Parent community handle for localization hierarchy.
    pub parent_community_handle: String,
    /// Locale for this community; required unless top-level `location_id` is supplied.
    #[serde(default)]
    pub location: Option<CreateCommunityLocation>,
    /// `social_join_policy` ref key: `open` \| `reviewed` \| `invite_only` \| `closed`.
    #[serde(default)]
    pub join_policy_key: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub intention: Option<String>,
    #[serde(default)]
    pub owner_handles: Option<String>,
    /// Optional owned clause seeded at create time (not inherited from parent).
    #[serde(default)]
    pub seed_constitution_clause: Option<String>,
}

fn normalized_community_join_policy_key(raw: &str) -> Option<&'static str> {
    let s = raw.trim();
    if s.is_empty() {
        return Some(ref_keys::KEY_JOIN_POLICY_OPEN);
    }
    if s.eq_ignore_ascii_case(ref_keys::KEY_JOIN_POLICY_OPEN) {
        Some(ref_keys::KEY_JOIN_POLICY_OPEN)
    } else if s.eq_ignore_ascii_case(ref_keys::KEY_JOIN_POLICY_REVIEWED)
        || s.eq_ignore_ascii_case("request_to_join")
    {
        Some(ref_keys::KEY_JOIN_POLICY_REVIEWED)
    } else if s.eq_ignore_ascii_case(ref_keys::KEY_JOIN_POLICY_INVITE_ONLY) {
        Some(ref_keys::KEY_JOIN_POLICY_INVITE_ONLY)
    } else if s.eq_ignore_ascii_case(ref_keys::KEY_JOIN_POLICY_CLOSED) {
        Some(ref_keys::KEY_JOIN_POLICY_CLOSED)
    } else {
        None
    }
}

fn entity_create_type_label(c: &i18n::Catalog, loc: &str, type_key: &str) -> String {
    let key = match type_key {
        ref_keys::KEY_ENTITY_TYPE_COMMUNITY => "ui.home.create_entity_card_title_community",
        ref_keys::KEY_ENTITY_TYPE_GATHERING => "ui.home.create_entity_card_title_event",
        ref_keys::KEY_ENTITY_TYPE_PROFIT => "ui.home.create_entity_card_title_business",
        ref_keys::KEY_ENTITY_TYPE_NON_PROFIT => "ui.home.create_entity_card_title_nonprofit",
        _ => "ui.home.create_entity_card_title_community",
    };
    c.t(loc, key)
}

#[derive(Debug, Deserialize)]
pub struct CreateRequest {
    /// Public handle; optional leading `@` is stripped. Stored lowercase `[a-z0-9_]`, 3–32 chars.
    pub handle: String,
    pub type_id: String,
    /// Omit or empty → default `pending_verification` in [`ref_keys::GROUP_STATUS`].
    #[serde(default)]
    pub status_id: Option<String>,
    pub location_id: String,
    /// Required when creating a **profit** entity; must be absent for other types.
    #[serde(default)]
    pub profit: Option<CreateProfitExtension>,
    /// Optional when creating a **non_profit** entity (e.g. external logo URL).
    #[serde(default)]
    pub non_profit: Option<CreateNonProfitExtension>,
    /// Optional payload when creating a **gathering** entity.
    #[serde(default)]
    pub gathering: Option<CreateGatheringExtension>,
    /// Optional payload when creating a **community** entity.
    #[serde(default)]
    pub community: Option<CreateCommunityExtension>,
}

#[derive(Debug, Serialize)]
pub struct Public {
    pub id: String,
    pub handle: String,
    pub type_id: String,
    pub status_id: String,
    pub location_id: String,
    pub created_at: TimestampUtc,
    pub updated_at: TimestampUtc,
    pub created_by: Option<String>,
    pub modified_by: Option<String>,
}

#[derive(Debug, FromRow)]
struct Row {
    id: u64,
    handle: String,
    type_id: u64,
    status_id: u64,
    location_id: u64,
    created_at: TimestampUtc,
    updated_at: TimestampUtc,
    created_by: Option<u64>,
    modified_by: Option<u64>,
}

fn row_to_public(r: Row) -> Public {
    Public {
        id: r.id.to_string(),
        handle: r.handle,
        type_id: r.type_id.to_string(),
        status_id: r.status_id.to_string(),
        location_id: r.location_id.to_string(),
        created_at: r.created_at,
        updated_at: r.updated_at,
        created_by: r.created_by.map(|x| x.to_string()),
        modified_by: r.modified_by.map(|x| x.to_string()),
    }
}

fn handle_body_valid(body: &str) -> bool {
    let b = body.as_bytes();
    (3..=32).contains(&b.len())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_')
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandleCheck {
    TooShort,
    Invalid,
    Reserved,
    Valid(String),
}

/// Normalize and classify a candidate handle for live availability checks.
#[must_use]
pub fn classify_handle_candidate(raw: &str) -> HandleCheck {
    let t = raw.trim();
    let t = t.strip_prefix('@').unwrap_or(t).trim();
    let lower = t.to_ascii_lowercase();
    if lower.len() < 3 {
        return HandleCheck::TooShort;
    }
    if !handle_body_valid(&lower) {
        return HandleCheck::Invalid;
    }
    if RESERVED_HANDLES.iter().any(|&r| r == lower.as_str()) {
        return HandleCheck::Reserved;
    }
    HandleCheck::Valid(lower)
}

/// Normalize and validate a handle for **create** (reserved names rejected).
pub fn normalize_handle_for_create(raw: &str) -> AppResult<String> {
    let loc = i18n::request_locale();
    let t = raw.trim();
    let t = t.strip_prefix('@').unwrap_or(t).trim();
    let lower = t.to_ascii_lowercase();
    if !handle_body_valid(&lower) {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "social_entity.handle_invalid"),
        ));
    }
    if RESERVED_HANDLES.iter().any(|&r| r == lower.as_str()) {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "social_entity.handle_reserved"),
        ));
    }
    Ok(lower)
}

/// Normalize handle from a public URL segment. Invalid shapes return `None` (caller should 404).
pub fn normalize_handle_from_path_segment(raw: &str) -> Option<String> {
    let t = raw.trim();
    let t = t.strip_prefix('@').unwrap_or(t).trim();
    let lower = t.to_ascii_lowercase();
    if handle_body_valid(&lower) {
        Some(lower)
    } else {
        None
    }
}

pub async fn create(
    pool: &MySqlPool,
    config: &crate::config::Config,
    body: CreateRequest,
    acting_user_id: u64,
) -> AppResult<Public> {
    let base_handle = normalize_handle_for_create(&body.handle)?;
    let type_id = ids::parse_u64(&body.type_id, "type_id", I18N_INVALID_ID)?;
    let status_id = match body
        .status_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(s) => ids::parse_u64(s, "status_id", I18N_INVALID_ID)?,
        None => ref_key_values::default_pending_status_id(pool)
            .await
            .map_err(AppError::Db)?
            .ok_or_else(|| {
                AppError::BadRequest(i18n::catalog().t(
                    &i18n::request_locale(),
                    "ref_keys.default_status_unconfigured",
                ))
            })?,
    };
    let loc = i18n::request_locale();
    if !db::ref_row_active(pool, type_id, ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "social_entity.type_ref_invalid"),
        ));
    }
    if !db::ref_row_active(pool, status_id, ref_keys::GROUP_STATUS)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "social_entity.status_ref_invalid"),
        ));
    }
    let human_type_id = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_SOCIAL_ENTITY_TYPE,
        ref_keys::KEY_ENTITY_TYPE_HUMAN,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| {
        AppError::BadRequest(i18n::catalog().t(&loc, "social_entity.type_ref_invalid"))
    })?;
    let profit_type_id = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_SOCIAL_ENTITY_TYPE,
        ref_keys::KEY_ENTITY_TYPE_PROFIT,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| {
        AppError::BadRequest(i18n::catalog().t(&loc, "social_entity.type_ref_invalid"))
    })?;
    let non_profit_type_id = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_SOCIAL_ENTITY_TYPE,
        ref_keys::KEY_ENTITY_TYPE_NON_PROFIT,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| {
        AppError::BadRequest(i18n::catalog().t(&loc, "social_entity.type_ref_invalid"))
    })?;
    let gathering_type_id = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_SOCIAL_ENTITY_TYPE,
        ref_keys::KEY_ENTITY_TYPE_GATHERING,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| {
        AppError::BadRequest(i18n::catalog().t(&loc, "social_entity.type_ref_invalid"))
    })?;
    let community_type_id = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_SOCIAL_ENTITY_TYPE,
        ref_keys::KEY_ENTITY_TYPE_COMMUNITY,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| {
        AppError::BadRequest(i18n::catalog().t(&loc, "social_entity.type_ref_invalid"))
    })?;
    let digital_type_id = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_SOCIAL_ENTITY_TYPE,
        ref_keys::KEY_ENTITY_TYPE_DIGITAL,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| {
        AppError::BadRequest(i18n::catalog().t(&loc, "social_entity.type_ref_invalid"))
    })?;
    let is_profit = type_id == profit_type_id;
    let is_non_profit = type_id == non_profit_type_id;
    let is_gathering = type_id == gathering_type_id;
    let is_community = type_id == community_type_id;
    let is_digital = type_id == digital_type_id;
    if is_digital {
        return Err(AppError::BadRequest(DIGITAL_CREATE_SEED_ONLY_MSG.to_string()));
    }
    let handle = if is_gathering {
        gathering_handle_for_save(&base_handle)
    } else {
        base_handle
    };
    let location_id = if is_community {
        let c = body.community.as_ref().ok_or_else(|| {
            AppError::BadRequest(
                i18n::catalog().t(&loc, "social_entity.community_payload_required"),
            )
        })?;
        resolve_community_create_location_id(pool, c, body.location_id.trim(), acting_user_id)
            .await?
    } else {
        let raw_location_id = body.location_id.trim();
        if raw_location_id.is_empty() {
            if is_profit || is_non_profit || is_gathering {
                posting_anchor_location_id(pool, acting_user_id)
                    .await?
                    .ok_or_else(|| {
                        AppError::BadRequest(
                            i18n::catalog().t(&loc, "social_entity.location_invalid"),
                        )
                    })?
            } else {
                return Err(AppError::BadRequest(
                    i18n::catalog().t(&loc, "social_entity.location_invalid"),
                ));
            }
        } else {
            ids::parse_u64(raw_location_id, "location_id", I18N_INVALID_ID)?
        }
    };
    if !db::location_exists(pool, location_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "social_entity.location_invalid"),
        ));
    }

    if body.non_profit.is_some() && !is_non_profit {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "social_entity.non_profit_payload_forbidden"),
        ));
    }
    if body.gathering.is_some() && !is_gathering {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "social_entity.gathering_payload_forbidden"),
        ));
    }
    if body.community.is_some() && !is_community {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "social_entity.community_payload_forbidden"),
        ));
    }
    if is_non_profit && body.profit.is_some() {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "social_entity.profit_payload_forbidden"),
        ));
    }
    if is_gathering && (body.profit.is_some() || body.non_profit.is_some()) {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "social_entity.gathering_payload_forbidden"),
        ));
    }
    if is_community && (body.profit.is_some() || body.non_profit.is_some() || body.gathering.is_some()) {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "social_entity.community_payload_forbidden"),
        ));
    }
    let profit_row: Option<(
        String,
        String,
        Option<String>,
        Option<String>,
        NaiveDate,
        Option<u64>,
        Option<u64>,
        Vec<String>,
    )> =
        if is_profit {
            if body.non_profit.is_some() {
                return Err(AppError::BadRequest(
                    i18n::catalog().t(&loc, "social_entity.non_profit_payload_forbidden"),
                ));
            }
            let p = body.profit.as_ref().ok_or_else(|| {
                AppError::BadRequest(
                    i18n::catalog().t(&loc, "social_entity.profit_payload_required"),
                )
            })?;
            let label = trim_required_field(&p.label, 512, "social_entity.profit_label_required", &loc)?;
            let legal_name =
                trim_required_field(&p.legal_name, 512, "social_entity.profit_legal_name_required", &loc)?;
            let description = trim_optional_rich_text(
                p.description.as_deref(),
                MAX_PROFIT_TEXT_FIELD,
                "social_entity.profit_text_too_long",
                &loc,
            )?;
            let intention = trim_optional_long_text(
                p.intention.as_deref(),
                MAX_PROFIT_TAGLINE,
                "social_entity.profit_text_too_long",
                &loc,
            )?;
            let founded_at = parse_required_founded_at(p.founded_at.as_str(), &loc)?;
            let website_link_id = optional_brand_url_to_content_link_id(
                pool,
                p.website_url.as_deref(),
                acting_user_id,
                &loc,
                false,
            )
            .await?;
            let logo_link_id = super::brand_logo::optional_brand_logo_url_to_content_link_id(
                pool,
                config,
                p.logo_url.as_deref(),
                p.website_url.as_deref(),
                acting_user_id,
                &loc,
            )
            .await?;
            let owner_handles = parse_owner_handles(p.owner_handles.as_deref());
            Some((
                label,
                legal_name,
                description,
                intention,
                founded_at,
                website_link_id,
                logo_link_id,
                owner_handles,
            ))
        } else {
            if body.profit.is_some() {
                return Err(AppError::BadRequest(
                    i18n::catalog().t(&loc, "social_entity.profit_payload_forbidden"),
                ));
            }
            None
        };
    let non_profit_row: Option<(
        String,
        String,
        Option<String>,
        Option<String>,
        NaiveDate,
        Option<u64>,
        Option<u64>,
        Vec<String>,
    )> = if is_non_profit {
        let np = body.non_profit.as_ref().ok_or_else(|| {
            AppError::BadRequest(
                i18n::catalog().t(&loc, "social_entity.non_profit_payload_required"),
            )
        })?;
        let label = trim_required_field(
            &np.label,
            512,
            "social_entity.non_profit_label_required",
            &loc,
        )?;
        let legal_name = trim_required_field(
            &np.legal_name,
            512,
            "social_entity.non_profit_legal_name_required",
            &loc,
        )?;
        let description = trim_optional_rich_text(
            np.description.as_deref(),
            MAX_PROFIT_TEXT_FIELD,
            "social_entity.profit_text_too_long",
            &loc,
        )?;
        let intention = trim_optional_long_text(
            np.intention.as_deref(),
            MAX_PROFIT_TAGLINE,
            "social_entity.profit_text_too_long",
            &loc,
        )?;
        let founded_at = parse_required_founded_at(np.founded_at.as_str(), &loc)?;
        let website_link_id = optional_brand_url_to_content_link_id(
            pool,
            np.website_url.as_deref(),
            acting_user_id,
            &loc,
            false,
        )
        .await?;
        let logo_link_id = super::brand_logo::optional_brand_logo_url_to_content_link_id(
            pool,
            config,
            np.logo_url.as_deref(),
            np.website_url.as_deref(),
            acting_user_id,
            &loc,
        )
        .await?;
        let owner_handles = parse_owner_handles(np.owner_handles.as_deref());
        Some((
            label,
            legal_name,
            description,
            intention,
            founded_at,
            website_link_id,
            logo_link_id,
            owner_handles,
        ))
    } else {
        None
    };
    let gathering_row: Option<(String, Option<String>, Option<String>, Option<u64>, Vec<String>)> =
        if is_gathering {
            let g = body.gathering.as_ref().ok_or_else(|| {
                AppError::BadRequest(
                    i18n::catalog().t(&loc, "social_entity.gathering_payload_required"),
                )
            })?;
            let label = trim_required_field(
                &g.label,
                512,
                "social_entity.gathering_label_required",
                &loc,
            )?;
            let description = trim_optional_rich_text(
                g.description.as_deref(),
                MAX_PROFIT_TEXT_FIELD,
                "social_entity.profit_text_too_long",
                &loc,
            )?;
            let intention = trim_optional_long_text(
                g.intention.as_deref(),
                MAX_PROFIT_TAGLINE,
                "social_entity.profit_text_too_long",
                &loc,
            )?;
            let event_link_id = optional_brand_url_to_content_link_id(
                pool,
                g.event_url.as_deref(),
                acting_user_id,
                &loc,
                false,
            )
            .await?;
            let owner_handles = parse_owner_handles(g.owner_handles.as_deref());
            Some((label, description, intention, event_link_id, owner_handles))
        } else {
            None
        };
    let community_row: Option<(
        String,
        u64,
        Option<String>,
        Option<String>,
        Vec<String>,
        u64,
        Option<String>,
        bool,
    )> = if is_community {
            let c = body.community.as_ref().ok_or_else(|| {
                AppError::BadRequest(
                    i18n::catalog().t(&loc, "social_entity.community_payload_required"),
                )
            })?;
            let label = trim_required_field(
                &c.label,
                512,
                "social_entity.community_label_required",
                &loc,
            )?;
            let parent_handle = normalize_handle_for_create(&c.parent_community_handle)?;
            if crate::social::panoply::is_invalid_community_parent_handle(&parent_handle) {
                return Err(AppError::BadRequest(
                    i18n::catalog().t(&loc, "social_entity.community_parent_panoply_forbidden"),
                ));
            }
            let parent_row: Option<(u64,)> = sqlx::query_as(
                r#"
                SELECT sc.social_entity_id
                FROM social_entity se
                INNER JOIN social_community sc ON sc.social_entity_id = se.id
                WHERE se.handle = ?
                LIMIT 1
                "#,
            )
            .bind(parent_handle.as_str())
            .fetch_optional(pool)
            .await
            .map_err(AppError::Db)?;
            let parent_community_id = parent_row
                .map(|(id,)| id)
                .ok_or_else(|| {
                    AppError::BadRequest(
                        i18n::catalog().t(&loc, "social_entity.community_parent_invalid"),
                    )
                })?;
            let policy_raw = c
                .join_policy_key
                .as_deref()
                .unwrap_or(ref_keys::KEY_JOIN_POLICY_OPEN);
            let policy_key = normalized_community_join_policy_key(policy_raw).ok_or_else(|| {
                AppError::BadRequest(
                    i18n::catalog().t(&loc, "social_entity.community_join_policy_invalid"),
                )
            })?;
            let community_join_policy_id =
                ref_key_values::lookup_active_id(pool, ref_keys::GROUP_SOCIAL_JOIN_POLICY, policy_key)
                    .await
                    .map_err(AppError::Db)?
                    .ok_or_else(|| {
                        AppError::BadRequest(
                            i18n::catalog().t(&loc, "social_entity.community_join_policy_invalid"),
                        )
                    })?;
            let description = trim_optional_rich_text(
                c.description.as_deref(),
                MAX_PROFIT_TEXT_FIELD,
                "social_entity.profit_text_too_long",
                &loc,
            )?;
            let intention = trim_optional_long_text(
                c.intention.as_deref(),
                MAX_PROFIT_TAGLINE,
                "social_entity.profit_text_too_long",
                &loc,
            )?;
            let owner_handles = parse_owner_handles(c.owner_handles.as_deref());
            let seed_clause = trim_optional_long_text(
                c.seed_constitution_clause.as_deref(),
                MAX_COMMUNITY_SEED_CLAUSE,
                "social_entity.community_seed_clause_too_long",
                &loc,
            )?;
            let seed_dispute = seed_clause.is_some();
            Some((
                label,
                parent_community_id,
                description,
                intention,
                owner_handles,
                community_join_policy_id,
                seed_clause,
                seed_dispute,
            ))
        } else {
            None
        };

    let rating = if type_id == human_type_id
        || type_id == community_type_id
        || type_id == profit_type_id
        || type_id == non_profit_type_id
        || type_id == gathering_type_id
    {
        Decimal::new(250, 2)
    } else {
        Decimal::new(0, 2)
    };

    let invite_only_join_policy_id = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_SOCIAL_JOIN_POLICY,
        ref_keys::KEY_JOIN_POLICY_INVITE_ONLY,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| {
        AppError::BadRequest(
            i18n::catalog().t(&loc, "social_entity.join_policy_invite_only_missing"),
        )
    })?;

    let entity_type_key_for_noms = if is_community {
        Some(ref_keys::KEY_ENTITY_TYPE_COMMUNITY)
    } else if is_profit {
        Some(ref_keys::KEY_ENTITY_TYPE_PROFIT)
    } else if is_non_profit {
        Some(ref_keys::KEY_ENTITY_TYPE_NON_PROFIT)
    } else if is_gathering {
        Some(ref_keys::KEY_ENTITY_TYPE_GATHERING)
    } else {
        None
    };

    let mut tx = pool.begin().await.map_err(AppError::Db)?;

    if let Some(type_key) = entity_type_key_for_noms {
        if let Some(service) = BillableService::for_entity_type_key(type_key) {
            let human_entity_id = human_onboarding::primary_human_entity_id_tx(&mut tx, acting_user_id)
                .await
                .map_err(AppError::Db)?
                .ok_or_else(|| {
                    AppError::BadRequest(
                        i18n::catalog().t(&loc, "social_entity.human_wallet_required"),
                    )
                })?;
            let spend_outcome = noms::spend_for_service_tx(
                &mut tx,
                human_entity_id,
                service,
                acting_user_id,
                Some(&handle),
            )
            .await
            .map_err(AppError::Db)?;
            if let SpendOutcome::InsufficientBalance { needed, balance } = spend_outcome {
                let c = i18n::catalog();
                let entity_type_label = entity_create_type_label(&c, &loc, type_key);
                return Err(AppError::BadRequest(c.tf(
                    &loc,
                    "noms.insufficient_for_entity_create",
                    &[
                        ("nom_price", &nom_icon::nom_amount_plain(needed)),
                        ("balance", &balance.to_string()),
                        ("entity_type", &entity_type_label),
                    ],
                )));
            }
        }
    }

    let res = sqlx::query(
        r#"
        INSERT INTO social_entity (
            handle, type_id, status_id, entity_status_id, location_id,
            rating, created_by, modified_by
        )
        VALUES (
            ?, ?, ?,
            (
                SELECT scoped.id
                FROM ref_key_values old_status
                INNER JOIN ref_key_values scoped
                    ON scoped.`group` = 'social_entity_status'
                   AND scoped.`key` = old_status.`key`
                WHERE old_status.id = ?
                LIMIT 1
            ),
            ?, ?, ?, ?
        )
        "#,
    )
    .bind(&handle)
    .bind(type_id)
    .bind(status_id)
    .bind(status_id)
    .bind(location_id)
    .bind(rating)
    .bind(acting_user_id)
    .bind(acting_user_id)
    .execute(&mut *tx)
    .await;

    let res = match res {
        Ok(r) => r,
        Err(e) => {
            if db::is_unique_violation(&e) {
                return Err(AppError::Conflict(
                    i18n::catalog().t(&loc, "social_entity.handle_taken"),
                ));
            }
            if db::is_foreign_key_violation(&e) {
                return Err(AppError::BadRequest(
                    i18n::catalog().t(&loc, "social_entity.fk_violation"),
                ));
            }
            return Err(AppError::Db(e));
        }
    };

    let id = res.last_insert_id();
    ensure_owner_role_tx(&mut tx, acting_user_id, id).await?;
    let creator_member_verified =
        is_profit || is_non_profit || is_gathering || is_community;
    let creator_member_rating = if is_community {
        Some(rust_decimal::Decimal::new(300, 2))
    } else {
        None
    };
    ensure_creator_member_connection_tx(
        &mut tx,
        acting_user_id,
        id,
        creator_member_verified,
        creator_member_rating,
    )
    .await?;

    if let Some((
        label,
        legal_name,
        description,
        intention,
        founded_at,
        website_link_id,
        logo_link_id,
        owner_handles,
    )) = profit_row
    {
        sqlx::query(
            r#"
            INSERT INTO social_profit (
                social_entity_id, join_policy_id, label, description, intention, legal_name,
                incorporation_country, registration_subdivision, registration_number, founded_at, content_link_id,
                logo_content_link_id
            )
            VALUES (?, ?, ?, ?, ?, ?, NULL, NULL, NULL, ?, ?, ?)
            "#,
        )
        .bind(id)
        .bind(invite_only_join_policy_id)
        .bind(&label)
        .bind(description.as_deref())
        .bind(intention.as_deref())
        .bind(&legal_name)
        .bind(founded_at)
        .bind(website_link_id)
        .bind(logo_link_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            if db::is_foreign_key_violation(&e) {
                AppError::BadRequest(i18n::catalog().t(&loc, "social_entity.fk_violation"))
            } else {
                AppError::Db(e)
            }
        })?;
        assign_additional_owner_roles_tx(&mut tx, id, owner_handles, acting_user_id).await?;
    }

    if let Some((
        label,
        legal_name,
        description,
        intention,
        founded_at,
        website_link_id,
        logo_link_id,
        owner_handles,
    )) = non_profit_row
    {
        sqlx::query(
            r#"
            INSERT INTO social_non_profit (
                social_entity_id, join_policy_id, label, description, intention, legal_name,
                incorporation_country, registration_subdivision, registration_number, founded_at,
                content_link_id, logo_content_link_id
            )
            VALUES (?, ?, ?, ?, ?, ?, NULL, NULL, NULL, ?, ?, ?)
            "#,
        )
        .bind(id)
        .bind(invite_only_join_policy_id)
        .bind(&label)
        .bind(description.as_deref())
        .bind(intention.as_deref())
        .bind(&legal_name)
        .bind(founded_at)
        .bind(website_link_id)
        .bind(logo_link_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            if db::is_foreign_key_violation(&e) {
                AppError::BadRequest(i18n::catalog().t(&loc, "social_entity.fk_violation"))
            } else {
                AppError::Db(e)
            }
        })?;
        assign_additional_owner_roles_tx(&mut tx, id, owner_handles, acting_user_id).await?;
    }

    if let Some((label, description, intention, event_link_id, owner_handles)) = gathering_row {
        sqlx::query(
            r#"
            INSERT INTO social_gathering (
                social_entity_id, label, description, intention, starts_at, ends_at, timezone,
                recurrence_frequency_id, recurrence_interval, recurrence_until, join_policy_id,
                event_portal_content_link_id
            )
            VALUES (?, ?, ?, ?, UTC_TIMESTAMP(6), NULL, 'UTC', NULL, 1, NULL, ?, ?)
            "#,
        )
        .bind(id)
        .bind(&label)
        .bind(description.as_deref())
        .bind(intention.as_deref())
        .bind(invite_only_join_policy_id)
        .bind(event_link_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            if db::is_foreign_key_violation(&e) {
                AppError::BadRequest(i18n::catalog().t(&loc, "social_entity.fk_violation"))
            } else {
                AppError::Db(e)
            }
        })?;
        assign_additional_owner_roles_tx(&mut tx, id, owner_handles, acting_user_id).await?;
    }

    if let Some((
        label,
        parent_community_id,
        description,
        intention,
        owner_handles,
        community_join_policy_id,
        seed_clause,
        seed_dispute,
    )) = community_row
    {
        if parent_community_id == id {
            return Err(AppError::BadRequest(
                i18n::catalog().t(&loc, "social_entity.community_parent_self"),
            ));
        }
        sqlx::query(
            r#"
            INSERT INTO social_community (
                social_entity_id, parent_community_id, label, description, intention, rules, founded_at, join_policy_id
            )
            VALUES (?, ?, ?, ?, ?, NULL, NULL, ?)
            "#,
        )
        .bind(id)
        .bind(parent_community_id)
        .bind(&label)
        .bind(description.as_deref())
        .bind(intention.as_deref())
        .bind(community_join_policy_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            if db::is_foreign_key_violation(&e) {
                AppError::BadRequest(i18n::catalog().t(&loc, "social_entity.fk_violation"))
            } else {
                AppError::Db(e)
            }
        })?;
        if let Some(clause) = seed_clause.as_deref() {
            crate::social::community_constitution::seed_initial_constitution_clause_tx(
                &mut tx,
                id,
                clause,
                seed_dispute,
                acting_user_id,
            )
            .await
            .map_err(AppError::Db)?;
        }
        assign_additional_owner_roles_tx(&mut tx, id, owner_handles, acting_user_id).await?;
    }

    tx.commit().await.map_err(AppError::Db)?;
    // Brand entities (community, profit, non-profit, gathering) are creator-operated and
    // should be discoverable at `/public/{handle}` immediately after create.
    if creator_member_verified {
        crate::status::transition_social_entity(
            pool,
            id,
            ref_keys::KEY_VERIFIED,
            Some(acting_user_id),
        )
        .await?;
    }
    get_by_id(pool, id).await?.ok_or(AppError::NotFound)
}

async fn ensure_owner_role_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    social_entity_id: u64,
) -> AppResult<()> {
    let role_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT id
        FROM ref_key_values
        WHERE `group` = 'user_social_entity_role'
          AND `key` = 'owner'
          AND is_active = 1
        LIMIT 1
        "#,
    )
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::Db)?;
    let Some(role_id) = role_id else {
        return Ok(());
    };
    sqlx::query(
        r#"
        INSERT INTO user_social_entity_role (
            user_id, social_entity_id, role_id, is_primary, starts_at, created_by, modified_by
        )
        VALUES (?, ?, ?, 1, UTC_DATE(), ?, ?)
        ON DUPLICATE KEY UPDATE
            modified_by = VALUES(modified_by),
            is_primary = GREATEST(is_primary, VALUES(is_primary))
        "#,
    )
    .bind(user_id)
    .bind(social_entity_id)
    .bind(role_id)
    .bind(user_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await
    .map_err(AppError::Db)?;
    Ok(())
}

/// Ensures the acting user’s human profile has a `member_of` edge to the new entity.
/// When `creator_relationship_verified`, that edge uses status `verified` (creator is immediately
/// a verified connection for communities, profits, non-profits, and gatherings).
async fn ensure_creator_member_connection_tx(
    tx: &mut Transaction<'_, MySql>,
    acting_user_id: u64,
    created_entity_id: u64,
    creator_relationship_verified: bool,
    creator_edge_rating: Option<rust_decimal::Decimal>,
) -> AppResult<()> {
    let human_entity_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
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
    .bind(acting_user_id)
    .bind(acting_user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::Db)?;
    let Some(human_entity_id) = human_entity_id else {
        return Ok(());
    };
    // Dunbar-scale cap on the creator's human entity. The pool-flavored helper in
    // `connections` runs the same SQL; we inline it here so the check stays inside the open tx
    // and a cap violation aborts the entity insert atomically.
    let peer_already_connected: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT ser.id
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = ser.status_id
           AND rk_st.`group` = ?
           AND rk_st.is_active = 1
           AND rk_st.`key` IN (?, ?, ?)
        WHERE ser.ended_at IS NULL
          AND (
              (ser.from_social_entity_id = ? AND ser.to_social_entity_id = ?)
              OR (ser.from_social_entity_id = ? AND ser.to_social_entity_id = ?)
          )
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_PENDING_VERIFICATION)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(ref_keys::KEY_FLAGGED)
    .bind(human_entity_id)
    .bind(created_entity_id)
    .bind(created_entity_id)
    .bind(human_entity_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::Db)?;
    if peer_already_connected.is_none() {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(DISTINCT peer.id)
            FROM social_entity_relationship ser
            INNER JOIN ref_key_values rk_st
                ON rk_st.id = ser.status_id
                AND rk_st.`group` = ?
                AND rk_st.is_active = 1
                AND rk_st.`key` IN (?, ?, ?)
            INNER JOIN social_entity peer ON peer.id = CASE
                WHEN ser.from_social_entity_id = ? THEN ser.to_social_entity_id
                ELSE ser.from_social_entity_id
            END
            INNER JOIN ref_key_values rk_et
                ON rk_et.id = peer.type_id
                AND rk_et.`group` = ?
                AND rk_et.is_active = 1
            WHERE ser.ended_at IS NULL
              AND (ser.from_social_entity_id = ? OR ser.to_social_entity_id = ?)
            "#,
        )
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_PENDING_VERIFICATION)
        .bind(ref_keys::KEY_VERIFIED)
        .bind(ref_keys::KEY_FLAGGED)
        .bind(human_entity_id)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
        .bind(human_entity_id)
        .bind(human_entity_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(AppError::Db)?;
        if (count as usize) >= connections::MAX_CONNECTION_SLOTS {
            let loc = i18n::request_locale();
            return Err(AppError::BadRequest(i18n::catalog().t(
                &loc,
                "social_entity_relationship.human_connection_cap_reached_self",
            )));
        }
    }
    let member_of_type_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT id
        FROM ref_key_values
        WHERE `group` = ?
          AND `key` = ?
          AND is_active = 1
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::Db)?;
    let Some(member_of_type_id) = member_of_type_id else {
        return Ok(());
    };
    let status_key = if creator_relationship_verified {
        ref_keys::KEY_VERIFIED
    } else {
        ref_keys::KEY_PENDING_VERIFICATION
    };
    let status_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT id
        FROM ref_key_values
        WHERE `group` = ?
          AND `key` = ?
          AND is_active = 1
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_STATUS)
    .bind(status_key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::Db)?
        .ok_or_else(|| {
            AppError::BadRequest(i18n::catalog().t(
                &i18n::request_locale(),
                "ref_keys.default_status_unconfigured",
            ))
        })?;
    let edge_rating =
        creator_edge_rating.unwrap_or_else(|| rust_decimal::Decimal::new(250, 2));
    sqlx::query(
        r#"
        INSERT INTO social_entity_relationship (
            from_social_entity_id, to_social_entity_id, type_id, status_id, relationship_status_id,
            rating, started_at, ended_at, created_by, modified_by
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
            ?, UTC_DATE(), NULL, ?, ?
        )
        "#,
    )
    .bind(human_entity_id)
    .bind(created_entity_id)
    .bind(member_of_type_id)
    .bind(status_id)
    .bind(status_id)
    .bind(edge_rating)
    .bind(acting_user_id)
    .bind(acting_user_id)
    .execute(&mut **tx)
    .await
    .map_err(AppError::Db)?;
    scope::mark_relationship_endpoints_dirty_in_tx(tx, human_entity_id, created_entity_id)
        .await
        .map_err(AppError::Db)?;
    Ok(())
}

pub(crate) async fn assign_additional_owner_roles_tx(
    tx: &mut Transaction<'_, MySql>,
    social_entity_id: u64,
    owner_handles: Vec<String>,
    acting_user_id: u64,
) -> AppResult<()> {
    if owner_handles.is_empty() {
        return Ok(());
    }
    let owner_role_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT id
        FROM ref_key_values
        WHERE `group` = 'user_social_entity_role'
          AND `key` = 'owner'
          AND is_active = 1
        LIMIT 1
        "#,
    )
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::Db)?;
    let Some(role_id) = owner_role_id else {
        return Ok(());
    };
    for h in owner_handles {
        let user_row: Option<u64> = sqlx::query_scalar(
            r#"
            SELECT created_by
            FROM social_entity
            WHERE handle = ?
              AND created_by IS NOT NULL
            LIMIT 1
            "#,
        )
        .bind(h.as_str())
        .fetch_optional(&mut **tx)
        .await
        .map_err(AppError::Db)?;
        let Some(owner_user_id) = user_row else {
            continue;
        };
        if owner_user_id == acting_user_id {
            continue;
        }
        sqlx::query(
            r#"
            INSERT INTO user_social_entity_role (
                user_id, social_entity_id, role_id, is_primary, starts_at, created_by, modified_by
            )
            VALUES (?, ?, ?, 0, UTC_DATE(), ?, ?)
            ON DUPLICATE KEY UPDATE
                modified_by = VALUES(modified_by)
            "#,
        )
        .bind(owner_user_id)
        .bind(social_entity_id)
        .bind(role_id)
        .bind(acting_user_id)
        .bind(acting_user_id)
        .execute(&mut **tx)
        .await
        .map_err(AppError::Db)?;
    }
    Ok(())
}

fn trim_required_field(raw: &str, max: usize, empty_i18n: &str, loc: &str) -> AppResult<String> {
    let t = raw.trim();
    if t.is_empty() {
        return Err(AppError::BadRequest(
            i18n::catalog().t(loc, empty_i18n),
        ));
    }
    if t.len() > max {
        return Err(AppError::BadRequest(
            i18n::catalog().t(loc, "social_entity.profit_field_too_long"),
        ));
    }
    Ok(t.to_string())
}

fn trim_optional_long_text(
    raw: Option<&str>,
    max: usize,
    too_long_i18n: &str,
    loc: &str,
) -> AppResult<Option<String>> {
    let Some(s) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if s.len() > max {
        return Err(AppError::BadRequest(
            i18n::catalog().t(loc, too_long_i18n),
        ));
    }
    Ok(Some(s.to_string()))
}

fn trim_optional_rich_text(
    raw: Option<&str>,
    max: usize,
    too_long_i18n: &str,
    loc: &str,
) -> AppResult<Option<String>> {
    let Some(s) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let sanitized = content_sanitize::sanitize_post_markup(s).trim().to_string();
    if sanitized.is_empty() {
        return Ok(None);
    }
    if sanitized.chars().count() > max {
        return Err(AppError::BadRequest(i18n::catalog().t(loc, too_long_i18n)));
    }
    Ok(Some(sanitized))
}

fn parse_required_founded_at(raw: &str, loc: &str) -> AppResult<NaiveDate> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(AppError::BadRequest(
            i18n::catalog().t(loc, "social_entity.profit_founded_invalid"),
        ));
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| {
        AppError::BadRequest(i18n::catalog().t(loc, "social_entity.profit_founded_invalid"))
    })
}

fn month_abbrev(month: u32) -> &'static str {
    match month {
        1 => "jan",
        2 => "feb",
        3 => "mar",
        4 => "apr",
        5 => "may",
        6 => "jun",
        7 => "jul",
        8 => "aug",
        9 => "sep",
        10 => "oct",
        11 => "nov",
        _ => "dec",
    }
}

fn is_month_token(token: &str) -> bool {
    matches!(
        token,
        "jan"
            | "january"
            | "feb"
            | "february"
            | "mar"
            | "march"
            | "apr"
            | "april"
            | "may"
            | "jun"
            | "june"
            | "jul"
            | "july"
            | "aug"
            | "august"
            | "sep"
            | "sept"
            | "september"
            | "oct"
            | "october"
            | "nov"
            | "november"
            | "dec"
            | "december"
    )
}

fn gathering_handle_for_save_with_date(base_handle: &str, month: &str, year: i32) -> String {
    let parts: Vec<&str> = base_handle.split('_').filter(|p| !p.is_empty()).collect();
    let has_month = parts.iter().any(|p| is_month_token(p));
    let has_year = parts
        .iter()
        .any(|p| p.len() == 4 && p.chars().all(|ch| ch.is_ascii_digit()));

    let mut suffix_parts: Vec<String> = Vec::new();
    if !has_month {
        suffix_parts.push(month.to_string());
    }
    if !has_year {
        suffix_parts.push(year.to_string());
    }
    if suffix_parts.is_empty() {
        return base_handle.to_string();
    }

    let suffix = format!("_{}", suffix_parts.join("_"));
    let max_base_len = 32usize.saturating_sub(suffix.len());
    let trimmed_base = if base_handle.len() > max_base_len {
        &base_handle[..max_base_len]
    } else {
        base_handle
    };
    format!("{trimmed_base}{suffix}")
}

fn gathering_handle_for_save(base_handle: &str) -> String {
    let now = Utc::now();
    gathering_handle_for_save_with_date(base_handle, month_abbrev(now.month()), now.year())
}

/// Canonicalize optional URL through `content_link` (domain registry, dedupe, blocklist).
/// When `require_https` is `true`, only `https:` URLs are accepted (recommended for externally hosted logos).
pub(crate) async fn optional_brand_url_to_content_link_id(
    pool: &MySqlPool,
    raw_url: Option<&str>,
    acting_user_id: u64,
    loc: &str,
    require_https: bool,
) -> AppResult<Option<u64>> {
    let Some(raw) = raw_url.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let canonical = crate::content::links::canonicalize_url(raw)?;
    if require_https && !canonical.url_canonical.to_ascii_lowercase().starts_with("https://") {
        return Err(AppError::BadRequest(
            i18n::catalog().t(loc, "social_entity.profit_logo_url_invalid"),
        ));
    }
    crate::content::links::ensure_domain_allowed(
        pool,
        &canonical.registrable_domain,
        "entity_brand_url",
        Some(acting_user_id),
        None,
    )
    .await?;
    let up = crate::content::links::upsert_canonical_link(pool, &canonical, Some(acting_user_id))
        .await?;
    Ok(Some(up.link_id))
}

fn parse_owner_handles(raw: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let Some(s) = raw else {
        return out;
    };
    let s = s.trim().to_ascii_lowercase().replace(',', " ");
    for token in s.split_whitespace() {
        let h = token.trim_start_matches('@');
        if h.len() < 3 || h.len() > 32 {
            continue;
        }
        if h.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
            let hs = h.to_string();
            if seen.insert(hs.clone()) {
                out.push(hs);
            }
        }
    }
    out
}

pub async fn get_by_id(pool: &MySqlPool, id: u64) -> AppResult<Option<Public>> {
    let row = sqlx::query_as::<_, Row>(
        r#"
        SELECT
            id, handle, type_id, status_id, location_id,
            created_at, updated_at, created_by, modified_by
        FROM social_entity
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(row_to_public))
}

/// `location_id` on the viewer's human `social_entity` row (anchor for creating additional entities).
async fn resolve_community_create_location_id(
    pool: &MySqlPool,
    ext: &CreateCommunityExtension,
    body_location_id: &str,
    acting_user_id: u64,
) -> AppResult<u64> {
    let loc = i18n::request_locale();
    if let Some(loc_in) = &ext.location {
        let cc = loc_in.country_code.trim();
        if cc.len() != 2 || !cc.chars().all(|ch| ch.is_ascii_alphabetic()) {
            return Err(AppError::BadRequest(
                i18n::catalog().t(&loc, "social_entity.community_location_country_required"),
            ));
        }
        let country_code = cc.to_ascii_uppercase();
        let region = trim_optional_long_text(
            loc_in.region.as_deref(),
            128,
            "social_entity.community_location_field_too_long",
            &loc,
        )?;
        let locality = trim_optional_long_text(
            loc_in.locality.as_deref(),
            128,
            "social_entity.community_location_field_too_long",
            &loc,
        )?;
        let display_label = community_location_display_label(
            &country_code,
            region.as_deref(),
            locality.as_deref(),
        );
        let loc_req = CreateLocationRequest {
            country_code,
            region,
            locality,
            postal_code: None,
            latitude: None,
            longitude: None,
            display_label,
            validation_source: "community_create".to_string(),
            validation_ref: None,
            store_coordinates: false,
            response_coordinate_precision: ResponseCoordinatePrecision::default(),
        };
        let loc_row = locations::create_or_get(pool, loc_req, acting_user_id).await?;
        return loc_row
            .id
            .parse()
            .map_err(|_| AppError::BadRequest(i18n::catalog().t(&loc, "social_entity.location_invalid")));
    }
    if body_location_id.is_empty() {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "social_entity.community_location_required"),
        ));
    }
    let location_id = ids::parse_u64(body_location_id, "location_id", I18N_INVALID_ID)?;
    if !db::location_exists(pool, location_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            i18n::catalog().t(&loc, "social_entity.location_invalid"),
        ));
    }
    Ok(location_id)
}

fn community_location_display_label(
    country_code: &str,
    region: Option<&str>,
    locality: Option<&str>,
) -> String {
    if country_code.eq_ignore_ascii_case(crate::ui::countries::GLOBAL_COUNTRY_CODE) {
        return "Global".to_string();
    }
    let mut parts: Vec<String> = Vec::new();
    if let Some(l) = locality.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(l.to_string());
    }
    if let Some(r) = region.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(r.to_string());
    }
    if parts.is_empty() {
        countries::display_name_for_code(country_code)
            .unwrap_or_else(|| country_code.to_string())
    } else {
        parts.join(", ")
    }
}

/// Country / region / locality from the creator's human posting anchor, when set.
pub async fn posting_anchor_location_fields(
    pool: &MySqlPool,
    user_id: u64,
) -> AppResult<(Option<String>, String, String)> {
    let Some(lid) = posting_anchor_location_id(pool, user_id).await? else {
        return Ok((None, String::new(), String::new()));
    };
    let row: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT country_code, region, locality
        FROM locations
        WHERE id = ?
        LIMIT 1
        "#,
    )
    .bind(lid)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Db)?;
    Ok(row
        .map(|(cc, region, locality)| {
            (
                Some(cc),
                region.unwrap_or_default(),
                locality.unwrap_or_default(),
            )
        })
        .unwrap_or((None, String::new(), String::new())))
}

pub async fn posting_anchor_location_id(pool: &MySqlPool, user_id: u64) -> AppResult<Option<u64>> {
    let row: Option<(u64,)> = sqlx::query_as(
        r#"
        SELECT se.location_id
        FROM social_entity se
        INNER JOIN ref_key_values rk
            ON rk.id = se.type_id
           AND rk.`group` = ?
           AND rk.`key` = ?
        LEFT JOIN user_social_entity_role ur
            ON ur.social_entity_id = se.id
           AND ur.user_id = ?
           AND ur.ends_at IS NULL
        WHERE ur.id IS NOT NULL OR se.created_by = ?
        ORDER BY se.id ASC
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .bind(user_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Db)?;
    Ok(row.map(|(lid,)| lid).filter(|&lid| lid > 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_path_accepts_typical_handle() {
        assert_eq!(
            normalize_handle_from_path_segment("@SomeGuy111").as_deref(),
            Some("someguy111")
        );
    }

    #[test]
    fn normalize_path_rejects_too_short() {
        assert!(normalize_handle_from_path_segment("ab").is_none());
    }

    #[test]
    fn create_rejects_reserved() {
        let err = normalize_handle_for_create("admin").unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn gathering_handle_appends_only_missing_date_parts() {
        assert_eq!(
            gathering_handle_for_save_with_date("winter_solstice_2026", "apr", 2026),
            "winter_solstice_2026_apr"
        );
        assert_eq!(
            gathering_handle_for_save_with_date("winter_solstice_apr", "apr", 2026),
            "winter_solstice_apr_2026"
        );
        assert_eq!(
            gathering_handle_for_save_with_date("winter_solstice_apr_2026", "apr", 2026),
            "winter_solstice_apr_2026"
        );
        assert_eq!(
            gathering_handle_for_save_with_date("winter_solstice_april_2026", "apr", 2026),
            "winter_solstice_april_2026"
        );
    }
}
