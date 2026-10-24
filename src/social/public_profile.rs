//! Policy-limited fields for the public profile card (`GET /api/public/entity-profile/{handle}`; HTML shell at `GET /public/{handle}`).

use chrono::NaiveDate;
use sqlx::FromRow;
use sqlx::MySqlPool;

use crate::ref_keys;

/// Only these status ref `key` values may show a public page (extend deliberately).
pub const PUBLIC_PROFILE_STATUS_KEY: &str = ref_keys::KEY_VERIFIED;

/// When `true`, include `locations.display_label` on the public page when status allows.
pub const PUBLIC_PROFILE_INCLUDE_LOCATION_LABEL: bool = true;

/// SQL predicate: entity alias is not lifecycle-dead.
///
/// Required on every logged-out / anonymous discovery path: public profiles, public post
/// listings, entity search, and invite typeahead (`/public/*`, `/api/public/*`, `/social-entities/search`).
#[must_use]
pub fn sql_alive_entity_predicate(se_alias: &str) -> String {
    format!(
        r#"NOT EXISTS (
            SELECT 1
            FROM ref_key_values rk_dead
            WHERE rk_dead.id = COALESCE({alias}.entity_status_id, {alias}.status_id)
              AND rk_dead.`group` IN ('{g_entity}', '{g_legacy}')
              AND rk_dead.`key` = '{dead}'
              AND rk_dead.is_active = 1
        )"#,
        alias = se_alias,
        g_entity = ref_keys::GROUP_SOCIAL_ENTITY_STATUS,
        g_legacy = ref_keys::GROUP_STATUS,
        dead = ref_keys::KEY_ENTITY_STATUS_DEAD,
    )
}

/// SQL expression for `generated_artwork_path` resolution (AI actor, explicit role, or entity owner).
/// `alias` must be the `social_entity` table alias in the outer query (e.g. `se` or `peer`).
#[must_use]
pub fn sql_avatar_path_expr(alias: &str) -> String {
    // Only safe when `alias` is a known identifier from our queries (not user input).
    let complete = crate::social::user_faces::COMPLETE_ARTWORK_ROW;
    format!(
        r#"COALESCE(
                (
                    SELECT uf.generated_artwork_path
                    FROM ai_actor aa
                    INNER JOIN user_faces uf ON uf.user_id = aa.user_id
                    WHERE aa.social_entity_id = {alias}.id
                      AND aa.is_active = 1
                      AND {complete}
                    LIMIT 1
                ),
                (
                    SELECT uf.generated_artwork_path
                    FROM user_social_entity_role ur
                    INNER JOIN user_faces uf ON uf.user_id = ur.user_id
                    WHERE ur.social_entity_id = {alias}.id
                      AND (ur.ends_at IS NULL OR ur.ends_at >= UTC_DATE())
                      AND {complete}
                    ORDER BY ur.is_primary DESC, ur.id ASC
                    LIMIT 1
                ),
                (
                    SELECT uf.generated_artwork_path
                    FROM user_faces uf
                    WHERE uf.user_id = {alias}.created_by
                      AND {complete}
                    LIMIT 1
                )
            )"#,
        alias = alias,
        complete = complete
    )
}

#[derive(Debug, Clone)]
pub struct PublicProfile {
    pub entity_id: u64,
    /// Normalized handle without `@` (for display prefix in the template).
    pub handle: String,
    pub type_label_key: String,
    pub entity_rating: f32,
    pub show_entity_rating: bool,
    pub display_name: Option<String>,
    pub country_code: Option<String>,
    pub location_display_label: Option<String>,
    pub avatar_path: Option<String>,
    /// `social_profit.label` / `social_community.label` / etc.
    pub trading_label: Option<String>,
    pub description: Option<String>,
    pub intention: Option<String>,
    pub legal_name: Option<String>,
    pub founded_at: Option<NaiveDate>,
    pub website_url: Option<String>,
    pub gathering_rsvp_accepted: u64,
    pub gathering_rsvp_not_going: u64,
    pub gathering_rsvp_interested: u64,
    /// Verified human `member_of` edges to this entity (communities); zero when not applicable.
    pub verified_member_count: u64,
    /// Human only: active `member_of` edges to communities (public-safe count; no handles).
    pub human_community_count: u64,
    /// Human only: public root posts authored by this entity (same visibility rules as public post cards).
    pub human_public_post_count: u64,
    /// Human only: public reply posts authored by this entity.
    pub human_public_comment_count: u64,
    /// Community only: `social_join_policy` ref key.
    pub community_join_policy_key: Option<String>,
}

#[derive(Debug, FromRow)]
struct Row {
    entity_id: u64,
    handle: String,
    type_key: String,
    entity_rating: rust_decimal::Decimal,
    display_name: Option<String>,
    country_code: Option<String>,
    display_label: Option<String>,
    avatar_path: Option<String>,
    extension_label: Option<String>,
    extension_description: Option<String>,
    extension_intention: Option<String>,
    extension_legal_name: Option<String>,
    extension_founded: Option<NaiveDate>,
    website_url: Option<String>,
    gathering_rsvp_accepted: i64,
    gathering_rsvp_not_going: i64,
    gathering_rsvp_interested: i64,
    verified_member_count: i64,
    human_community_count: i64,
    human_public_post_count: i64,
    human_public_comment_count: i64,
    community_join_policy_key: Option<String>,
}

/// SQL fragment: public-visible posts by `se.id` (optionally root-only or reply-only).
fn sql_human_public_post_count(alias: &str, root_only: bool) -> String {
    let parent_filter = if root_only {
        "AND p_cnt.parent_post_id IS NULL"
    } else {
        "AND p_cnt.parent_post_id IS NOT NULL"
    };
    format!(
        r#"(
            SELECT COUNT(*)
            FROM post p_cnt
            INNER JOIN post_audience_scope pas_cnt
                ON pas_cnt.post_id = p_cnt.id
               AND pas_cnt.scope_key = 'public'
            LEFT JOIN ref_key_values rk_cnt_post
                ON rk_cnt_post.id = p_cnt.status_id
               AND rk_cnt_post.`group` = '{g_status}'
            WHERE p_cnt.entity_id = se.id
              AND p_cnt.is_moderation_mirror = 0
              {parent_filter}
              AND COALESCE(rk_cnt_post.`key`, '') <> '{k_review}'
              AND NOT EXISTS (
                  SELECT 1
                  FROM post_audience_scope pas_priv
                  WHERE pas_priv.post_id = p_cnt.id
                    AND pas_priv.scope_key = 'private'
              )
        ) AS {alias}"#,
        g_status = ref_keys::GROUP_STATUS,
        k_review = ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW,
        parent_filter = parent_filter,
        alias = alias,
    )
}

/// Load publishable data for `handle` (already normalized). Returns `None` if unknown, non-public status, or inactive refs.
pub async fn fetch(pool: &MySqlPool, handle: &str) -> Result<Option<PublicProfile>, sqlx::Error> {
    if crate::social::panoply::is_panoply_community_handle(handle) {
        return Ok(None);
    }

    let avatar = sql_avatar_path_expr("se");
    let gathering_rsvp_count_sql = |rel_key: &str, alias: &str| {
        format!(
            r#"(SELECT COUNT(*)
            FROM social_entity_relationship ser_rsvp
            INNER JOIN ref_key_values rk_rsvp_type
                ON rk_rsvp_type.id = ser_rsvp.type_id
               AND rk_rsvp_type.`group` = '{g_rel}'
               AND rk_rsvp_type.`key` = '{rel_key}'
            INNER JOIN ref_key_values rk_rsvp_status
                ON rk_rsvp_status.id = ser_rsvp.status_id
               AND rk_rsvp_status.`group` = '{g_status}'
               AND rk_rsvp_status.is_active = 1
               AND rk_rsvp_status.`key` IN ('{v}', '{f}')
            WHERE ser_rsvp.to_social_entity_id = se.id
              AND ser_rsvp.ended_at IS NULL
        ) AS {alias}"#,
            g_rel = ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE,
            rel_key = rel_key,
            g_status = ref_keys::GROUP_STATUS,
            v = ref_keys::KEY_VERIFIED,
            f = ref_keys::KEY_FLAGGED,
            alias = alias,
        )
    };
    let gathering_rsvp_accepted_sql =
        gathering_rsvp_count_sql(ref_keys::KEY_RELATIONSHIP_EVENT_GOING, "gathering_rsvp_accepted");
    let gathering_rsvp_not_going_sql =
        gathering_rsvp_count_sql(ref_keys::KEY_RELATIONSHIP_EVENT_NOT_GOING, "gathering_rsvp_not_going");
    let gathering_rsvp_interested_sql =
        gathering_rsvp_count_sql(ref_keys::KEY_RELATIONSHIP_EVENT_INTERESTED, "gathering_rsvp_interested");
    let verified_member_count_sql = format!(
        r#"(
                SELECT COUNT(DISTINCT ser_m.from_social_entity_id)
                FROM social_entity_relationship ser_m
                INNER JOIN social_entity se_m
                    ON se_m.id = ser_m.from_social_entity_id
                INNER JOIN ref_key_values rk_m_type
                    ON rk_m_type.id = se_m.type_id
                   AND rk_m_type.`group` = '{g_ent}'
                   AND rk_m_type.`key` = '{k_human}'
                INNER JOIN ref_key_values rk_m_status
                    ON rk_m_status.id = COALESCE(se_m.entity_status_id, se_m.status_id)
                INNER JOIN ref_key_values rk_rel_type
                    ON rk_rel_type.id = ser_m.type_id
                   AND rk_rel_type.`group` = '{g_rel}'
                   AND rk_rel_type.`key` = '{k_member}'
                WHERE ser_m.to_social_entity_id = se.id
                  AND ser_m.ended_at IS NULL
                  AND rk_m_status.`key` = '{k_verified}'
            ) AS verified_member_count"#,
        g_ent = ref_keys::GROUP_SOCIAL_ENTITY_TYPE,
        k_human = ref_keys::KEY_ENTITY_TYPE_HUMAN,
        g_rel = ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE,
        k_member = ref_keys::KEY_RELATIONSHIP_MEMBER_OF,
        k_verified = ref_keys::KEY_VERIFIED,
    );
    let exclude_panoply_peer = crate::social::panoply::sql_exclude_panoply_entity("se_peer");
    let alive_peer = sql_alive_entity_predicate("se_peer");
    let human_community_count_sql = format!(
        r#"(
                SELECT COUNT(DISTINCT ser_hc.to_social_entity_id)
                FROM social_entity_relationship ser_hc
                INNER JOIN ref_key_values rk_hc_rel
                    ON rk_hc_rel.id = ser_hc.type_id
                   AND rk_hc_rel.`group` = '{g_rel}'
                   AND rk_hc_rel.`key` = '{k_member}'
                INNER JOIN ref_key_values rk_hc_status
                    ON rk_hc_status.id = ser_hc.status_id
                   AND rk_hc_status.`group` = '{g_status}'
                   AND rk_hc_status.is_active = 1
                   AND rk_hc_status.`key` IN ('{k_verified}', '{k_flagged}')
                INNER JOIN social_entity se_peer ON se_peer.id = ser_hc.to_social_entity_id
                INNER JOIN ref_key_values rk_hc_peer_type
                    ON rk_hc_peer_type.id = se_peer.type_id
                   AND rk_hc_peer_type.`group` = '{g_ent}'
                   AND rk_hc_peer_type.`key` = '{k_community}'
                WHERE ser_hc.from_social_entity_id = se.id
                  AND ser_hc.ended_at IS NULL
                  AND {alive_peer}
                  {exclude_panoply_peer}
            ) AS human_community_count"#,
        g_rel = ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE,
        k_member = ref_keys::KEY_RELATIONSHIP_MEMBER_OF,
        g_status = ref_keys::GROUP_STATUS,
        k_verified = ref_keys::KEY_VERIFIED,
        k_flagged = ref_keys::KEY_FLAGGED,
        g_ent = ref_keys::GROUP_SOCIAL_ENTITY_TYPE,
        k_community = ref_keys::KEY_ENTITY_TYPE_COMMUNITY,
        alive_peer = alive_peer,
        exclude_panoply_peer = exclude_panoply_peer,
    );
    let human_public_post_count_sql = sql_human_public_post_count("human_public_post_count", true);
    let human_public_comment_count_sql = sql_human_public_post_count("human_public_comment_count", false);
    let q = format!(
        r#"
        SELECT
            se.id AS entity_id,
            se.handle AS handle,
            rk_type.`key` AS type_key,
            COALESCE(se.rating, 0.00) AS entity_rating,
            COALESCE(
                NULLIF(TRIM(sp.label), ''),
                NULLIF(TRIM(snp.label), ''),
                NULLIF(TRIM(sc.label), ''),
                NULLIF(TRIM(sg.label), ''),
                NULLIF(TRIM(sd.label), ''),
                NULLIF(TRIM(CONCAT_WS(' ', sh.given_name, sh.additional_name, sh.family_name)), '')
            ) AS display_name,
            loc.country_code AS country_code,
            loc.display_label AS display_label,
            COALESCE(
                NULLIF(TRIM(cl_plogo.url_canonical), ''),
                NULLIF(TRIM(cl_snplogo.url_canonical), ''),
                ({avatar})
            ) AS avatar_path,
            COALESCE(sp.label, snp.label, sc.label, sg.label) AS extension_label,
            COALESCE(sp.description, snp.description, sc.description, sg.description, sd.description) AS extension_description,
            COALESCE(sp.intention, snp.intention, sc.intention, sg.intention, sd.intention) AS extension_intention,
            COALESCE(sp.legal_name, snp.legal_name) AS extension_legal_name,
            COALESCE(sp.founded_at, snp.founded_at, sc.founded_at) AS extension_founded,
            COALESCE(
                NULLIF(TRIM(cl_profit.url_canonical), ''),
                NULLIF(TRIM(cl_nonprofit.url_canonical), ''),
                NULLIF(TRIM(cl_profile.url_canonical), ''),
                NULLIF(TRIM(cl_gathering.url_canonical), '')
            ) AS website_url,
            {verified_member_count_sql},
            {human_community_count_sql},
            {human_public_post_count_sql},
            {human_public_comment_count_sql},
            (
                SELECT rk_jp.`key`
                FROM social_community sc_jp
                INNER JOIN ref_key_values rk_jp
                    ON rk_jp.id = sc_jp.join_policy_id
                   AND rk_jp.`group` = 'social_join_policy'
                   AND rk_jp.is_active = 1
                WHERE sc_jp.social_entity_id = se.id
                LIMIT 1
            ) AS community_join_policy_key,
            {gathering_rsvp_accepted_sql},
            {gathering_rsvp_not_going_sql},
            {gathering_rsvp_interested_sql}
        FROM social_entity se
"#,
        avatar = avatar,
        verified_member_count_sql = verified_member_count_sql,
        human_community_count_sql = human_community_count_sql,
        human_public_post_count_sql = human_public_post_count_sql,
        human_public_comment_count_sql = human_public_comment_count_sql,
        gathering_rsvp_accepted_sql = gathering_rsvp_accepted_sql,
        gathering_rsvp_not_going_sql = gathering_rsvp_not_going_sql,
        gathering_rsvp_interested_sql = gathering_rsvp_interested_sql,
    );
    let alive_predicate = sql_alive_entity_predicate("se");
    let row = sqlx::query_as::<_, Row>(
        &(format!(
            "{q}{tail}",
            q = q,
            tail = format!(
                r#"
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = se.type_id
            AND rk_type.`group` = ?
            AND rk_type.is_active = 1
        INNER JOIN ref_key_values rk_status
            ON rk_status.id = COALESCE(se.entity_status_id, se.status_id)
            AND rk_status.is_active = 1
            AND rk_status.`key` = ?
        LEFT JOIN locations loc ON loc.id = se.location_id
        LEFT JOIN social_profit sp ON sp.social_entity_id = se.id
        LEFT JOIN social_non_profit snp ON snp.social_entity_id = se.id
        LEFT JOIN social_community sc ON sc.social_entity_id = se.id
        LEFT JOIN social_gathering sg ON sg.social_entity_id = se.id
        LEFT JOIN social_digital sd ON sd.social_entity_id = se.id
        LEFT JOIN social_human sh ON sh.social_entity_id = se.id
        LEFT JOIN content_link cl_profit ON cl_profit.id = sp.content_link_id
        LEFT JOIN content_link cl_nonprofit ON cl_nonprofit.id = snp.content_link_id
        LEFT JOIN content_link cl_profile ON cl_profile.id = se.profile_content_link_id
        LEFT JOIN content_link cl_gathering ON cl_gathering.id = sg.event_portal_content_link_id
        LEFT JOIN content_link cl_plogo ON cl_plogo.id = sp.logo_content_link_id
        LEFT JOIN content_link cl_snplogo ON cl_snplogo.id = snp.logo_content_link_id
        WHERE se.handle = ?
          AND {alive_predicate}
        LIMIT 1
        "#,
                alive_predicate = alive_predicate,
            ),
        )),
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(PUBLIC_PROFILE_STATUS_KEY)
    .bind(handle)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| {
        let trim_opt = |s: Option<String>| {
            s.map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
        };
        PublicProfile {
            entity_id: r.entity_id,
            handle: r.handle,
            type_label_key: r.type_key.clone(),
            entity_rating: r.entity_rating.to_string().parse::<f32>().unwrap_or(0.0),
            show_entity_rating: matches!(
                r.type_key.as_str(),
                ref_keys::KEY_ENTITY_TYPE_COMMUNITY
                    | ref_keys::KEY_ENTITY_TYPE_NON_PROFIT
                    | ref_keys::KEY_ENTITY_TYPE_PROFIT
                    | ref_keys::KEY_ENTITY_TYPE_GATHERING
            ),
            display_name: trim_opt(r.display_name),
            country_code: trim_opt(r.country_code).map(|s| s.to_ascii_uppercase()),
            location_display_label: if PUBLIC_PROFILE_INCLUDE_LOCATION_LABEL
                && r.type_key != ref_keys::KEY_ENTITY_TYPE_HUMAN
            {
                r.display_label
            } else {
                None
            },
            avatar_path: r.avatar_path,
            trading_label: trim_opt(r.extension_label),
            description: trim_opt(r.extension_description),
            intention: trim_opt(r.extension_intention),
            legal_name: trim_opt(r.extension_legal_name),
            founded_at: r.extension_founded,
            website_url: trim_opt(r.website_url),
            gathering_rsvp_accepted: r.gathering_rsvp_accepted.max(0) as u64,
            gathering_rsvp_not_going: r.gathering_rsvp_not_going.max(0) as u64,
            gathering_rsvp_interested: r.gathering_rsvp_interested.max(0) as u64,
            verified_member_count: r.verified_member_count.max(0) as u64,
            human_community_count: if r.type_key == ref_keys::KEY_ENTITY_TYPE_HUMAN {
                r.human_community_count.max(0) as u64
            } else {
                0
            },
            human_public_post_count: if r.type_key == ref_keys::KEY_ENTITY_TYPE_HUMAN {
                r.human_public_post_count.max(0) as u64
            } else {
                0
            },
            human_public_comment_count: if r.type_key == ref_keys::KEY_ENTITY_TYPE_HUMAN {
                r.human_public_comment_count.max(0) as u64
            } else {
                0
            },
            community_join_policy_key: r.community_join_policy_key,
        }
    }))
}
