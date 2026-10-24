//! Home and profile post feeds.
//!
//! **Aggregate home feed** (no profile handle): visibility matches
//! [`viewer_feed_audience_entity_ids`] — the viewer’s entities plus peers reachable by **verified**
//! `social_entity_relationship` edges (pending / flagged do not expand the audience). Authors
//! always see their own posts (`p.created_by`), including while a post is under judicial review.
//! Jurors reach disputed corpus only through the juror workspace (de-personalized); those roots stay
//! out of the aggregate feed. Thread roots
//! appear if any day in the thread is addressed to that audience set or authored by the viewer.
//!
//! **`post_audience_scope` `public` vs `private`**: `public` means the post may appear on **logged-out**
//! surfaces (e.g. marketing / public post lists). **`@private` alone** (no `@friends` / `@family` /
//! `@colleagues` / `@base`) means a direct-audience post: aggregate feed entity matching uses the
//! viewer’s **owned** entities only, not graph peers (godmother welcome). **`@private` plus relationship
//! scopes** still reach verified edges via entity audience and/or live relationship-scope checks.
//!
//! **Profile feed** (handle in query): [`resolve_profile_feed_filter`] — own entities see the full
//! thread; another user’s **human** profile is public-only until a **verified** human↔human edge
//! exists; non-human targets use read-scope (`entity_read_scope` / `vw_entity_read_scope_live`).
//!
//! **Own human + `mentions_only`**: when the viewer opens their own human profile feed, treat the
//! request as the aggregate “all about me” filter ([`SQL_FEED_ABOUT_VIEWER_ON_ROOT`]) — do not AND
//! profile entity scope with mention participation (that hid comments on others’ threads).

use chrono::{DateTime, Utc};
use sqlx::MySqlPool;

use crate::ref_keys;
use crate::social::blocks;
use crate::social::entity;
use crate::social::human_onboarding;
use crate::social::public_profile;

#[derive(Debug, sqlx::FromRow)]
pub struct HomeFeedRow {
    pub id: u64,
    /// `post.entity_id` (authoring social entity for this post).
    pub post_entity_id: u64,
    pub handle: String,
    pub display_name: Option<String>,
    pub avatar_path: Option<String>,
    pub entity_country_code: Option<String>,
    pub post_country_code: Option<String>,
    pub body_plain: String,
    pub body_markup: String,
    pub body_html_sanitized: String,
    pub og_title: Option<String>,
    pub og_description: Option<String>,
    pub og_image_url: Option<String>,
    pub og_site_name: Option<String>,
    pub link_href: Option<String>,
    pub is_public: i64,
    pub post_status_key: Option<String>,
    /// Latest `judicial_case_status` ref key for the latest post-scoped moderation case on this thread root, if any.
    pub judicial_case_status_key: Option<String>,
    /// `judicial_case_trigger` ref key for [`judicial_post_moderation_case_id`] (`civic_dispute`, `emergency_safety`, …).
    pub judicial_trigger_key: Option<String>,
    /// 1 when the linked moderation case has `standing_applied_at` set (uncontested emergency standing).
    pub judicial_standing_applied: i64,
    /// `judicial_case.id` for that latest post-moderation row (see `jc_mod` join), when present.
    pub judicial_post_moderation_case_id: Option<u64>,
    /// Child cases with `appealed_from_judicial_case_id` pointing at the latest post-moderation case.
    pub judicial_moderation_appeal_count: i64,
    /// Latest council appeal case (`moderation_appeal`) filed against [`judicial_post_moderation_case_id`], if any.
    pub judicial_appeal_case_id: Option<u64>,
    /// `judicial_case_status` ref key for [`judicial_appeal_case_id`].
    pub judicial_appeal_case_status_key: Option<String>,
    /// Whether the feed viewer has an active `user_social_entity_role` on the case originator or target entity.
    pub judicial_viewer_is_moderation_party: i64,
    /// `ref_key_values.key` for `social_entity_type` (`human`, `digital`, …).
    pub entity_type_key: Option<String>,
    pub entity_status_key: Option<String>,
    pub entity_created_by: u64,
    pub entity_rating: rust_decimal::Decimal,
    pub sort_modified_at: DateTime<Utc>,
    /// When the post was published (`COALESCE(published_at, created_at)`).
    pub published_at: DateTime<Utc>,
    /// `users.id` of the account that created the post row (`post.created_by`).
    pub post_created_by: Option<u64>,
    /// Comma-separated audience tokens (e.g. `@you, @public`); `post_audience_entity` + `post_audience_scope`.
    pub audience_for_label: Option<String>,
    /// Weighted engagement stars (0..5) on the root post.
    pub post_engagement_rating: rust_decimal::Decimal,
    /// Non-mirror replies under this root.
    pub engagement_comment_count: i64,
    /// `yes` / `no` when the feed viewer has voted on this root.
    pub viewer_engagement_vote: Option<String>,
    /// Votes on this root from users other than the post author.
    pub non_author_root_vote_count: i64,
    /// 1 when the root row has a `content_link_id`.
    pub has_content_link: i64,
    pub content_link_id: Option<u64>,
    pub domain_id: Option<u64>,
}

/// Human jury post disputes may not be opened by the post author (council appeal after verdict is separate).
pub fn viewer_may_file_human_post_dispute(viewer_user_id: u64, row: &HomeFeedRow) -> bool {
    if row.post_created_by == Some(viewer_user_id) {
        return false;
    }
    if row.entity_created_by == viewer_user_id {
        return false;
    }
    true
}

/// Same as [`viewer_may_file_human_post_dispute`] plus primary-human entity ownership of the post row.
pub async fn viewer_may_file_human_post_dispute_tx(
    pool: &MySqlPool,
    viewer_user_id: u64,
    row: &HomeFeedRow,
) -> Result<bool, sqlx::Error> {
    if !viewer_may_file_human_post_dispute(viewer_user_id, row) {
        return Ok(false);
    }
    if row.entity_type_key.as_deref() == Some(ref_keys::KEY_ENTITY_TYPE_HUMAN) {
        if let Some(primary_human_entity_id) =
            human_onboarding::primary_human_entity_id(pool, viewer_user_id).await?
        {
            if primary_human_entity_id == row.post_entity_id {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

#[derive(Debug)]
pub struct FeedPageRows {
    pub rows: Vec<HomeFeedRow>,
    pub has_newer: bool,
    pub has_older: bool,
}

#[derive(Debug, Clone, Copy)]
struct ProfileFeedFilter {
    target_entity_id: u64,
    viewer_can_see_private: bool,
}

/// Correlated on outer `p.id`; append after `sort_modified_at` in SELECT lists.
pub const SQL_POST_AUDIENCE_FOR_LABEL: &str = r#"(
    SELECT GROUP_CONCAT(aud_part ORDER BY aud_part SEPARATOR ', ')
    FROM (
        SELECT CONCAT('@', se_aud.handle) AS aud_part
        FROM post_audience_entity pae0
        INNER JOIN social_entity se_aud ON se_aud.id = pae0.audience_entity_id
        WHERE pae0.post_id = p.id
          AND pae0.audience_entity_id <> p.entity_id
        UNION
        SELECT CASE pas0.scope_key
            WHEN 'public' THEN '@public'
            WHEN 'private' THEN '@private'
            WHEN 'followers' THEN '@base'
            WHEN 'colleague' THEN '@colleagues'
            ELSE CONCAT('@', pas0.scope_key)
        END AS aud_part
        FROM post_audience_scope pas0
        WHERE pas0.post_id = p.id
    ) AS aud_union
) AS audience_for_label"#;

/// Audience label subquery for an arbitrary root post id / poster entity SQL expression.
#[must_use]
pub fn sql_audience_label_subquery(post_id_sql: &str, poster_entity_id_sql: &str) -> String {
    format!(
        r#"(SELECT GROUP_CONCAT(aud_part ORDER BY aud_part SEPARATOR ', ')
    FROM (
        SELECT CONCAT('@', se_aud.handle) AS aud_part
        FROM post_audience_entity pae0
        INNER JOIN social_entity se_aud ON se_aud.id = pae0.audience_entity_id
        WHERE pae0.post_id = {post_id_sql}
          AND pae0.audience_entity_id <> {poster_entity_id_sql}
        UNION
        SELECT CASE pas0.scope_key
            WHEN 'public' THEN '@public'
            WHEN 'private' THEN '@private'
            WHEN 'followers' THEN '@base'
            WHEN 'colleague' THEN '@colleagues'
            ELSE CONCAT('@', pas0.scope_key)
        END AS aud_part
        FROM post_audience_scope pas0
        WHERE pas0.post_id = {post_id_sql}
    ) AS aud_union)"#,
        post_id_sql = post_id_sql,
        poster_entity_id_sql = poster_entity_id_sql,
    )
}

const FEED_SELECT_COLUMNS: &str = r#"
    p.id,
    p.entity_id AS post_entity_id,
    se.handle,
    COALESCE(
        NULLIF(TRIM(sp_ent.label), ''),
        NULLIF(TRIM(snp_ent.label), ''),
        NULLIF(TRIM(sc_ent.label), ''),
        NULLIF(TRIM(sg_ent.label), ''),
        NULLIF(TRIM(sd_ent.label), ''),
        NULLIF(TRIM(CONCAT_WS(' ', sh_ent.given_name, sh_ent.additional_name, sh_ent.family_name)), '')
    ) AS display_name,
    COALESCE(NULLIF(TRIM(cl_sp_logo.url_canonical), ''), NULLIF(TRIM(cl_snp_logo.url_canonical), ''), __AVATAR_PATH_SQL__) AS avatar_path,
    loc.country_code AS entity_country_code,
    p.country_code AS post_country_code,
    p.body_plain,
    p.body_markup,
    p.body_html_sanitized,
    cl.og_title,
    cl.og_description,
    cl.og_image_url,
    cl.og_site_name,
    cl.url_canonical AS link_href,
    EXISTS (SELECT 1 FROM post_audience_scope pas_pub_row WHERE pas_pub_row.post_id = p.id AND pas_pub_row.scope_key = 'public') AS is_public,
    rk_post_status.`key` AS post_status_key,
    rk_jc_status.`key` AS judicial_case_status_key,
    rt_jc_trig.`key` AS judicial_trigger_key,
    IF(jc_mod.standing_applied_at IS NOT NULL, 1, 0) AS judicial_standing_applied,
    jc_mod.id AS judicial_post_moderation_case_id,
    IF(
        jc_mod.id IS NULL,
        0,
        COALESCE(
            (
                SELECT COUNT(*)
                FROM judicial_case j_aff
                WHERE j_aff.appealed_from_judicial_case_id = jc_mod.id
            ),
            0
        )
    ) AS judicial_moderation_appeal_count,
    jc_appeal.id AS judicial_appeal_case_id,
    rk_jc_appeal_status.`key` AS judicial_appeal_case_status_key,
    IF(
        jc_mod.id IS NULL,
        0,
        (
            EXISTS (
                SELECT 1
                FROM user_social_entity_role ur_o
                WHERE ur_o.user_id = feed_viewer_ctx.feed_viewer_user_id
                  AND ur_o.ends_at IS NULL
                  AND ur_o.social_entity_id = jc_mod.originator_social_entity_id
            )
            OR EXISTS (
                SELECT 1
                FROM social_entity se_o
                WHERE se_o.id = jc_mod.originator_social_entity_id
                  AND se_o.created_by = feed_viewer_ctx.feed_viewer_user_id
            )
            OR EXISTS (
                SELECT 1
                FROM user_social_entity_role ur_t
                WHERE ur_t.user_id = feed_viewer_ctx.feed_viewer_user_id
                  AND ur_t.ends_at IS NULL
                  AND ur_t.social_entity_id = jc_mod.target_social_entity_id
            )
            OR EXISTS (
                SELECT 1
                FROM social_entity se_t
                WHERE se_t.id = jc_mod.target_social_entity_id
                  AND se_t.created_by = feed_viewer_ctx.feed_viewer_user_id
            )
        )
    ) AS judicial_viewer_is_moderation_party,
    rk_entity_type.`key` AS entity_type_key,
    rk_entity_status.`key` AS entity_status_key,
    se.created_by AS entity_created_by,
    COALESCE(se.rating, 0.00) AS entity_rating,
    p.updated_at AS sort_modified_at,
    COALESCE(p.published_at, p.created_at) AS published_at,
    p.created_by AS post_created_by,
    COALESCE(p.rating, 0.00) AS post_engagement_rating,
    __ENGAGEMENT_COMMENT_COUNT__ AS engagement_comment_count,
    (
        SELECT pev.direction
        FROM post_engagement_vote pev
        WHERE pev.target_post_id = p.id
          AND pev.voter_user_id = feed_viewer_ctx.feed_viewer_user_id
        LIMIT 1
    ) AS viewer_engagement_vote,
    (
        SELECT COUNT(*)
        FROM post_engagement_vote pev
        INNER JOIN post rp ON rp.id = pev.target_post_id
        WHERE pev.target_post_id = p.id
          AND pev.voter_user_id <> rp.created_by
    ) AS non_author_root_vote_count,
    IF(p.content_link_id IS NOT NULL, 1, 0) AS has_content_link,
    p.content_link_id AS content_link_id,
    COALESCE(p.domain_id, cl.domain_id) AS domain_id
"#;

#[must_use]
pub fn feed_select_columns_sql() -> String {
    format!(
        "{}, {}",
        FEED_SELECT_COLUMNS
            .trim()
            .replace(
                "__ENGAGEMENT_COMMENT_COUNT__",
                crate::social::engagement::SQL_ENGAGEMENT_COMMENT_COUNT,
            )
            .replace(
                "__AVATAR_PATH_SQL__",
                &public_profile::sql_avatar_path_expr("se"),
            ),
        SQL_POST_AUDIENCE_FOR_LABEL
    )
}
const FEED_FROM_JOINS: &str = r#"
    FROM post p
    INNER JOIN social_entity se ON se.id = p.entity_id
    LEFT JOIN ref_key_values rk_post_status ON rk_post_status.id = p.status_id AND rk_post_status.`group` = ?
    LEFT JOIN ref_key_values rk_entity_type ON rk_entity_type.id = se.type_id AND rk_entity_type.`group` = ?
    LEFT JOIN ref_key_values rk_entity_status ON rk_entity_status.id = se.entity_status_id AND rk_entity_status.`group` = ?
    LEFT JOIN social_profit sp_ent ON sp_ent.social_entity_id = se.id
    LEFT JOIN social_non_profit snp_ent ON snp_ent.social_entity_id = se.id
    LEFT JOIN social_community sc_ent ON sc_ent.social_entity_id = se.id
    LEFT JOIN social_gathering sg_ent ON sg_ent.social_entity_id = se.id
    LEFT JOIN social_digital sd_ent ON sd_ent.social_entity_id = se.id
    LEFT JOIN social_human sh_ent ON sh_ent.social_entity_id = se.id
    LEFT JOIN content_link cl_sp_logo ON cl_sp_logo.id = sp_ent.logo_content_link_id
    LEFT JOIN content_link cl_snp_logo ON cl_snp_logo.id = snp_ent.logo_content_link_id
    LEFT JOIN locations loc ON loc.id = se.location_id
    LEFT JOIN content_link cl ON cl.id = p.content_link_id
    LEFT JOIN judicial_case jc_mod ON jc_mod.id = (
        SELECT j.id
        FROM judicial_case j
        INNER JOIN ref_key_values rt_trig
            ON rt_trig.id = j.trigger_type_id
           AND rt_trig.`group` = 'judicial_case_trigger'
           AND rt_trig.`key` IN ('civic_dispute', 'post_moderation', 'emergency_safety')
        WHERE j.subject_post_id = p.id
        ORDER BY j.id DESC
        LIMIT 1
    )
    LEFT JOIN ref_key_values rk_jc_status
        ON rk_jc_status.id = jc_mod.status_id
       AND rk_jc_status.`group` = 'judicial_case_status'
    LEFT JOIN ref_key_values rt_jc_trig
        ON rt_jc_trig.id = jc_mod.trigger_type_id
       AND rt_jc_trig.`group` = 'judicial_case_trigger'
    LEFT JOIN judicial_case jc_appeal ON jc_appeal.id = (
        SELECT j.id
        FROM judicial_case j
        INNER JOIN ref_key_values rt_app
            ON rt_app.id = j.trigger_type_id
           AND rt_app.`group` = 'judicial_case_trigger'
           AND rt_app.`key` = 'moderation_appeal'
        WHERE j.appealed_from_judicial_case_id = jc_mod.id
        ORDER BY j.id DESC
        LIMIT 1
    )
    LEFT JOIN ref_key_values rk_jc_appeal_status
        ON rk_jc_appeal_status.id = jc_appeal.status_id
       AND rk_jc_appeal_status.`group` = 'judicial_case_status'
    CROSS JOIN (SELECT ? AS feed_viewer_user_id) AS feed_viewer_ctx
"#;

/// `IN` list or `NULL` when the viewer has no audience entities (matches aggregate feed SQL).
fn audience_entity_placeholders(audience_ids: &[u64]) -> String {
    if audience_ids.is_empty() {
        "NULL".to_string()
    } else {
        std::iter::repeat_n("?", audience_ids.len())
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// WHERE fragment for the signed-in aggregate feed: judicial review posts, thread-level
/// audience gate, and root `post_audience_entity` gate. `ph` is [`audience_entity_placeholders`].
/// Viewer-authored thread roots under active post-dispute review sort first (aggregate + profile feeds).
const FEED_ORDER_BY_PIN_JUDICIAL_SQL: &str = r#"
ORDER BY
    (CASE
        WHEN p.created_by = ?
          AND (
              (rk_post_status.`key` = ? AND COALESCE(rk_jc_status.`key`, '') IN (?, ?, ?))
              OR (rk_post_status.`key` = ? AND COALESCE(rk_jc_status.`key`, '') = ?)
          )
        THEN 0
        ELSE 1
    END) ASC,
    p.updated_at DESC,
    p.id DESC"#;

/// `@private` without relationship scopes — godmother-style direct audience (not friends-only).
const SQL_POST_AUDIENCE_PRIVATE_ONLY: &str = r#"
EXISTS (
    SELECT 1
    FROM post_audience_scope pas_po
    WHERE pas_po.post_id = {post_id}
      AND pas_po.scope_key = 'private'
)
AND NOT EXISTS (
    SELECT 1
    FROM post_audience_scope pas_rel
    WHERE pas_rel.post_id = {post_id}
      AND pas_rel.scope_key IN ('friends', 'family', 'colleague', 'followers')
)"#;

/// Verified human↔author relationship matching `@friends` / `@family` / `@colleague` on `post_id`.
const SQL_POST_RELATIONSHIP_SCOPE_VISIBLE: &str = r#"
EXISTS (
    SELECT 1
    FROM post_audience_scope pas_rs
    WHERE pas_rs.post_id = {post_id}
      AND pas_rs.scope_key IN ('friends', 'family', 'colleague')
      AND EXISTS (
          SELECT 1
          FROM social_entity_relationship ser
          INNER JOIN ref_key_values rk_st
              ON rk_st.id = ser.status_id
             AND rk_st.`group` = ?
             AND rk_st.is_active = 1
             AND rk_st.`key` = ?
          INNER JOIN ref_key_values rk_rel
              ON rk_rel.id = ser.type_id
             AND rk_rel.`group` = ?
             AND (
                  (pas_rs.scope_key = 'friends' AND rk_rel.`key` = ?)
                  OR (pas_rs.scope_key = 'family' AND rk_rel.`key` = ?)
                  OR (pas_rs.scope_key = 'colleague' AND rk_rel.`key` = ?)
             )
          WHERE ser.ended_at IS NULL
            AND (
                  (ser.from_social_entity_id = {author_entity_id} AND ser.to_social_entity_id = ?)
                  OR (ser.to_social_entity_id = {author_entity_id} AND ser.from_social_entity_id = ?)
            )
      )
)"#;

/// Post carries `@friends`, `@family`, `@colleagues`, or `@base` audience scopes.
const SQL_POST_HAS_RELATIONSHIP_SCOPE: &str = r#"
EXISTS (
    SELECT 1
    FROM post_audience_scope pas_rel
    WHERE pas_rel.post_id = {post_id}
      AND pas_rel.scope_key IN ('friends', 'family', 'colleague', 'followers')
)"#;

fn home_feed_audience_visibility_sql(ph: &str, ph_owned: &str) -> String {
    let private_only_day = SQL_POST_AUDIENCE_PRIVATE_ONLY.replace("{post_id}", "p_day.id");
    let has_rel_day = SQL_POST_HAS_RELATIONSHIP_SCOPE.replace("{post_id}", "p_day.id");
    let rel_scope_day = SQL_POST_RELATIONSHIP_SCOPE_VISIBLE
        .replace("{post_id}", "p_day.id")
        .replace("{author_entity_id}", "p_day.entity_id");
    let private_only_root = SQL_POST_AUDIENCE_PRIVATE_ONLY.replace("{post_id}", "p.id");
    let has_rel_root = SQL_POST_HAS_RELATIONSHIP_SCOPE.replace("{post_id}", "p.id");
    let rel_scope_root = SQL_POST_RELATIONSHIP_SCOPE_VISIBLE
        .replace("{post_id}", "p.id")
        .replace("{author_entity_id}", "p.entity_id");
    format!(
        r#"
              AND (COALESCE(rk_post_status.`key`, '') <> ? OR p.created_by = ?)
              AND (COALESCE(rk_post_status.`key`, '') <> ? OR p.created_by = ?)
              {blocked}
              AND NOT EXISTS (
                  SELECT 1
                  FROM users u_plat
                  INNER JOIN ref_key_values rk_plat
                      ON rk_plat.id = COALESCE(u_plat.user_status_id, u_plat.status_id)
                     AND rk_plat.`group` IN (?, ?)
                     AND rk_plat.is_active = 1
                     AND rk_plat.`key` = ?
                  WHERE u_plat.id = p.created_by
              )
              AND EXISTS (
                    SELECT 1
                    FROM post p_day
                    WHERE COALESCE(p_day.root_post_id, p_day.id) = p.id
                      AND (
                          p_day.created_by = ?
                          OR (
                              NOT ({has_rel_day})
                              AND NOT ({private_only_day})
                              AND EXISTS (
                                  SELECT 1
                                  FROM post_audience_entity pae_day
                                  WHERE pae_day.post_id = p_day.id
                                    AND pae_day.audience_entity_id IN ({ph})
                              )
                          )
                          OR (
                              ({private_only_day})
                              AND EXISTS (
                                  SELECT 1
                                  FROM post_audience_entity pae_day
                                  WHERE pae_day.post_id = p_day.id
                                    AND pae_day.audience_entity_id IN ({ph_owned})
                              )
                          )
                          OR ({rel_scope_day})
                      )
              )
              AND (
                  p.created_by = ?
                  OR (
                      NOT ({has_rel_root})
                      AND NOT ({private_only_root})
                      AND EXISTS (
                          SELECT 1
                          FROM post_audience_entity pae
                          WHERE pae.post_id = p.id
                            AND pae.audience_entity_id IN ({ph})
                      )
                  )
                  OR (
                      ({private_only_root})
                      AND EXISTS (
                          SELECT 1
                          FROM post_audience_entity pae
                          WHERE pae.post_id = p.id
                            AND pae.audience_entity_id IN ({ph_owned})
                      )
                  )
                  OR ({rel_scope_root})
              )"#,
        ph = ph,
        ph_owned = ph_owned,
        blocked = super::blocks::SQL_FEED_EXCLUDE_BLOCKED_AUTHORS,
        private_only_day = private_only_day,
        has_rel_day = has_rel_day,
        rel_scope_day = rel_scope_day,
        private_only_root = private_only_root,
        has_rel_root = has_rel_root,
        rel_scope_root = rel_scope_root,
    )
}

fn bind_home_feed_visibility<'q>(
    mut q: sqlx::query::QueryAs<'q, sqlx::MySql, HomeFeedRow, sqlx::mysql::MySqlArguments>,
    viewer_user_id: u64,
    viewer_human_entity_id: u64,
    audience_ids: &[u64],
    owned_entity_ids: &[u64],
) -> sqlx::query::QueryAs<'q, sqlx::MySql, HomeFeedRow, sqlx::mysql::MySqlArguments> {
    q = q
        .bind(ref_keys::KEY_POST_SUPPRESSED)
        .bind(viewer_user_id)
        .bind(ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW)
        .bind(viewer_user_id)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
        .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
        .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
        .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_BLOCKED)
        .bind(viewer_human_entity_id)
        .bind(viewer_human_entity_id)
        .bind(ref_keys::GROUP_USER_STATUS)
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_BLOCKED)
        .bind(viewer_user_id);
    for id in audience_ids {
        q = q.bind(*id);
    }
    for id in owned_entity_ids {
        q = q.bind(*id);
    }
    q = q
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_VERIFIED)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
        .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
        .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
        .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
        .bind(viewer_human_entity_id)
        .bind(viewer_human_entity_id);
    q = q.bind(viewer_user_id);
    for id in audience_ids {
        q = q.bind(*id);
    }
    for id in owned_entity_ids {
        q = q.bind(*id);
    }
    q = q
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_VERIFIED)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
        .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
        .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
        .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
        .bind(viewer_human_entity_id)
        .bind(viewer_human_entity_id);
    q
}

async fn resolve_profile_feed_filter(
    pool: &sqlx::MySqlPool,
    viewer_user_id: u64,
    raw_profile_handle: Option<&str>,
) -> Result<Option<ProfileFeedFilter>, sqlx::Error> {
    let Some(raw_handle) = raw_profile_handle.map(str::trim).filter(|h| !h.is_empty()) else {
        return Ok(None);
    };
    let Some(normalized) = entity::normalize_handle_from_path_segment(raw_handle) else {
        return Ok(None);
    };
    let target_entity_id = sqlx::query_scalar::<_, u64>(
        r#"
        SELECT se.id
        FROM social_entity se
        WHERE se.handle = ?
        LIMIT 1
        "#,
    )
    .bind(normalized)
    .fetch_optional(pool)
    .await?;
    let Some(target_entity_id) = target_entity_id else {
        return Ok(None);
    };
    if let Some(viewer_human_id) =
        human_onboarding::primary_human_entity_id(pool, viewer_user_id).await?
    {
        let target_is_human: i64 = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM social_human sh WHERE sh.social_entity_id = ?)"#,
        )
        .bind(target_entity_id)
        .fetch_one(pool)
        .await?;
        if target_is_human != 0
            && blocks::is_pair_blocked(pool, viewer_human_id, target_entity_id).await?
        {
            return Ok(None);
        }
    }
    // Own profile/org: full feed. Another user's **human** profile: private rows only if there is a
    // **verified** human↔human relationship (pending invitations → public-only). Non-human targets
    // keep read-scope / materialized graph rules.
    // Use `IF(…,1,0)` so MySQL returns a single numeric column (avoids driver quirks with `SELECT (a OR b)`).
    let viewer_can_see_private: i64 = sqlx::query_scalar(
        r#"
        SELECT IF(
            EXISTS (
                SELECT 1
                FROM social_entity se
                LEFT JOIN user_social_entity_role ur
                    ON ur.social_entity_id = se.id
                   AND ur.user_id = ?
                   AND ur.ends_at IS NULL
                WHERE se.id = ?
                  AND (se.created_by = ? OR ur.id IS NOT NULL)
            )
            OR (
                EXISTS (
                    SELECT 1 FROM social_human sh_t WHERE sh_t.social_entity_id = ?
                )
                AND EXISTS (
                    SELECT 1
                    FROM social_human sh_v
                    INNER JOIN social_entity se_v
                        ON se_v.id = sh_v.social_entity_id
                       AND se_v.created_by = ?
                    INNER JOIN social_entity_relationship ser
                        ON ser.ended_at IS NULL
                       AND (
                            (ser.from_social_entity_id = sh_v.social_entity_id AND ser.to_social_entity_id = ?)
                         OR (ser.to_social_entity_id = sh_v.social_entity_id AND ser.from_social_entity_id = ?)
                        )
                    INNER JOIN ref_key_values rk
                        ON rk.id = ser.status_id
                       AND rk.`group` = ?
                       AND rk.is_active = 1
                       AND rk.`key` IN (?, ?)
                )
            )
            OR (
                NOT EXISTS (
                    SELECT 1 FROM social_human sh_nt WHERE sh_nt.social_entity_id = ?
                )
                AND EXISTS (
                    SELECT 1
                    FROM social_entity se_subject
                    LEFT JOIN user_social_entity_role ur_subject
                        ON ur_subject.social_entity_id = se_subject.id
                       AND ur_subject.user_id = ?
                       AND ur_subject.ends_at IS NULL
                    WHERE (se_subject.created_by = ? OR ur_subject.id IS NOT NULL)
                      AND (
                          EXISTS (
                              SELECT 1
                              FROM entity_read_scope ers
                              WHERE ers.subject_entity_id = se_subject.id
                                AND ers.visible_entity_id = ?
                          )
                          OR EXISTS (
                              SELECT 1
                              FROM vw_entity_read_scope_live v
                              WHERE v.subject_entity_id = se_subject.id
                                AND v.visible_entity_id = ?
                          )
                      )
                )
            ),
            1,
            0
        )
        "#,
    )
    .bind(viewer_user_id)
    .bind(target_entity_id)
    .bind(viewer_user_id)
    .bind(target_entity_id)
    .bind(viewer_user_id)
    .bind(target_entity_id)
    .bind(target_entity_id)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(ref_keys::KEY_FLAGGED)
    .bind(target_entity_id)
    .bind(viewer_user_id)
    .bind(viewer_user_id)
    .bind(target_entity_id)
    .bind(target_entity_id)
    .fetch_one(pool)
    .await?;
    let viewer_can_see_private = viewer_can_see_private != 0;
    Ok(Some(ProfileFeedFilter {
        target_entity_id,
        viewer_can_see_private,
    }))
}

/// Entity ids the viewer owns or holds an active role on (no graph peer expansion).
pub async fn viewer_owned_entity_ids(
    pool: &sqlx::MySqlPool,
    viewer_user_id: u64,
) -> Result<Vec<u64>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        LEFT JOIN user_social_entity_role ur
          ON ur.social_entity_id = se.id
         AND ur.user_id = ?
         AND ur.ends_at IS NULL
        WHERE se.created_by = ? OR ur.id IS NOT NULL
        "#,
    )
    .bind(viewer_user_id)
    .bind(viewer_user_id)
    .fetch_all(pool)
    .await
}

pub async fn viewer_feed_audience_entity_ids(
    pool: &sqlx::MySqlPool,
    viewer_user_id: u64,
) -> Result<Vec<u64>, sqlx::Error> {
    let mut ids = viewer_owned_entity_ids(pool, viewer_user_id).await?;

    // Peers reached only through **verified** graph edges. Pending/flagged connections do not
    // expand the feed audience (own entities and roles are unchanged; authors still see their
    // posts via `p.created_by` in the feed query).
    let connected: Vec<u64> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT peer_id
        FROM (
            SELECT ser.to_social_entity_id AS peer_id
            FROM social_entity_relationship ser
            INNER JOIN ref_key_values rk_st
                ON rk_st.id = ser.status_id
               AND rk_st.`group` = ?
               AND rk_st.is_active = 1
               AND rk_st.`key` = ?
            WHERE ser.ended_at IS NULL
              AND ser.from_social_entity_id IN (
                  SELECT se.id
                  FROM social_entity se
                  LEFT JOIN user_social_entity_role ur
                    ON ur.social_entity_id = se.id
                   AND ur.user_id = ?
                   AND ur.ends_at IS NULL
                  WHERE se.created_by = ? OR ur.id IS NOT NULL
              )
            UNION
            SELECT ser.from_social_entity_id AS peer_id
            FROM social_entity_relationship ser
            INNER JOIN ref_key_values rk_st
                ON rk_st.id = ser.status_id
               AND rk_st.`group` = ?
               AND rk_st.is_active = 1
               AND rk_st.`key` = ?
            WHERE ser.ended_at IS NULL
              AND ser.to_social_entity_id IN (
                  SELECT se.id
                  FROM social_entity se
                  LEFT JOIN user_social_entity_role ur
                    ON ur.social_entity_id = se.id
                   AND ur.user_id = ?
                   AND ur.ends_at IS NULL
                  WHERE se.created_by = ? OR ur.id IS NOT NULL
              )
        ) peers
        "#,
    )
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(viewer_user_id)
    .bind(viewer_user_id)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(viewer_user_id)
    .bind(viewer_user_id)
    .fetch_all(pool)
    .await?;
    ids.extend(connected);
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

/// SQL fragment: root posts about the viewer — authored by them, @mentioned, or commented on.
pub const SQL_FEED_ABOUT_VIEWER_ON_ROOT: &str = r#"
          AND (
              p.entity_id = ?
              OR EXISTS (
                  SELECT 1
                  FROM post_mention pm
                  INNER JOIN post pm_post ON pm_post.id = pm.post_id
                  WHERE pm.mentioned_entity_id = ?
                    AND COALESCE(pm_post.root_post_id, pm_post.id) = p.id
              )
              OR EXISTS (
                  SELECT 1
                  FROM post cmt
                  WHERE cmt.root_post_id IS NOT NULL
                    AND cmt.root_post_id = p.id
                    AND cmt.created_by = ?
              )
          )
"#;

/// Back-compat alias for the personal feed filter (`mentions_only` query param).
pub const SQL_FEED_MENTIONED_ON_ROOT: &str = SQL_FEED_ABOUT_VIEWER_ON_ROOT;

fn bind_feed_about_viewer_filter<'q>(
    q: sqlx::query::QueryAs<'q, sqlx::MySql, HomeFeedRow, sqlx::mysql::MySqlArguments>,
    user_id: u64,
    viewer_human_entity_id: u64,
) -> sqlx::query::QueryAs<'q, sqlx::MySql, HomeFeedRow, sqlx::mysql::MySqlArguments> {
    q.bind(viewer_human_entity_id)
        .bind(viewer_human_entity_id)
        .bind(user_id)
}

pub async fn viewer_can_engage_on_root_post(
    pool: &MySqlPool,
    viewer_user_id: u64,
    root_post_id: u64,
) -> Result<bool, sqlx::Error> {
    Ok(fetch_visible_root_post_for_viewer(pool, viewer_user_id, root_post_id)
        .await?
        .is_some())
}

pub async fn fetch_page_rows(
    pool: &sqlx::MySqlPool,
    user_id: u64,
    page: u32,
    page_size: u64,
    raw_profile_handle: Option<&str>,
    mentions_only: bool,
) -> Result<FeedPageRows, sqlx::Error> {
    let row_offset = (page as u64) * page_size;
    let profile_handle_requested = raw_profile_handle
        .map(str::trim)
        .filter(|h| !h.is_empty())
        .is_some();
    let profile_filter = resolve_profile_feed_filter(pool, user_id, raw_profile_handle).await?;
    let own_human_about_me = if mentions_only {
        if let Some(ref filter) = profile_filter {
            if filter.viewer_can_see_private {
                human_onboarding::primary_human_entity_id(pool, user_id)
                    .await?
                    .map(|hid| hid == filter.target_entity_id)
                    .unwrap_or(false)
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };
    let profile_scope = profile_filter.filter(|_| !own_human_about_me);
    let mentions_sql = if mentions_only {
        SQL_FEED_MENTIONED_ON_ROOT
    } else {
        ""
    };
    let viewer_human_entity_id = if mentions_only {
        human_onboarding::primary_human_entity_id(pool, user_id)
            .await?
            .unwrap_or(0)
    } else {
        0
    };

    let mut rows: Vec<HomeFeedRow> = if profile_handle_requested && profile_filter.is_none() {
        Vec::new()
    } else if let Some(filter) = profile_scope {
        // `viewer_can_see_private`: full thread roots for this entity (trusted viewer / owner).
        // Profile feeds include posts authored by the profile entity and posts addressed to it.
        // Else: internet-equivalent surface — humans need explicit `@public`; profit / non-profit
        // default to world-visible unless `@private` (matches signed-out `/public/posts` semantics for brands).
        let rows_trusted = if filter.viewer_can_see_private {
            let sql = format!(
                r#"
                SELECT
                    {select_cols}
                {from_joins}
                WHERE p.root_post_id IS NULL
                  AND (
                      p.entity_id = ?
                      OR EXISTS (
                          SELECT 1
                          FROM post p_day
                          INNER JOIN post_audience_entity pae_day
                              ON pae_day.post_id = p_day.id
                             AND pae_day.audience_entity_id = ?
                          WHERE COALESCE(p_day.root_post_id, p_day.id) = p.id
                      )
                  )
                {mentions_sql}
                {order_by}
                LIMIT ? OFFSET ?
                "#,
                select_cols = feed_select_columns_sql(),
                from_joins = FEED_FROM_JOINS,
                order_by = FEED_ORDER_BY_PIN_JUDICIAL_SQL,
                mentions_sql = mentions_sql,
            );
            let mut q = sqlx::query_as::<_, HomeFeedRow>(&sql)
                .bind(ref_keys::GROUP_STATUS)
                .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
                .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
                .bind(user_id)
                .bind(filter.target_entity_id)
                .bind(filter.target_entity_id);
            if mentions_only {
                q = bind_feed_about_viewer_filter(q, user_id, viewer_human_entity_id);
            }
            q = q
                .bind(user_id)
                .bind(ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW)
                .bind(ref_keys::KEY_JUDICIAL_CASE_OPEN)
                .bind(ref_keys::KEY_JUDICIAL_CASE_JURY_ACTIVE)
                .bind(ref_keys::KEY_JUDICIAL_CASE_ESCALATED_TO_AI)
                .bind(ref_keys::KEY_POST_SUPPRESSED)
                .bind(ref_keys::KEY_JUDICIAL_CASE_CONTAINED)
                .bind(page_size + 1)
                .bind(row_offset);
            q.fetch_all(pool).await?
        } else {
            let sql = format!(
                r#"
                SELECT
                    {select_cols}
                {from_joins}
                WHERE p.root_post_id IS NULL
                  AND (
                      p.entity_id = ?
                      OR EXISTS (
                          SELECT 1
                          FROM post p_day
                          INNER JOIN post_audience_entity pae_day
                              ON pae_day.post_id = p_day.id
                             AND pae_day.audience_entity_id = ?
                          WHERE COALESCE(p_day.root_post_id, p_day.id) = p.id
                      )
                  )
                  AND NOT EXISTS (
                      SELECT 1 FROM post_audience_scope pas_priv
                      WHERE pas_priv.post_id = p.id AND pas_priv.scope_key = 'private'
                  )
                  AND COALESCE(rk_post_status.`key`, '') <> ?
                  AND (
                      EXISTS (
                          SELECT 1 FROM post_audience_scope pas_pub
                          WHERE pas_pub.post_id = p.id AND pas_pub.scope_key = 'public'
                      )
                      OR rk_entity_type.`key` IN ('profit', 'non_profit', 'gathering')
                  )
                {mentions_sql}
                {order_by}
                LIMIT ? OFFSET ?
                "#,
                select_cols = feed_select_columns_sql(),
                from_joins = FEED_FROM_JOINS,
                order_by = FEED_ORDER_BY_PIN_JUDICIAL_SQL,
                mentions_sql = mentions_sql,
            );
            let mut q = sqlx::query_as::<_, HomeFeedRow>(&sql)
                .bind(ref_keys::GROUP_STATUS)
                .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
                .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
                .bind(user_id)
                .bind(filter.target_entity_id)
                .bind(filter.target_entity_id)
                .bind(ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW);
            if mentions_only {
                q = bind_feed_about_viewer_filter(q, user_id, viewer_human_entity_id);
            }
            q = q
                .bind(user_id)
                .bind(ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW)
                .bind(ref_keys::KEY_JUDICIAL_CASE_OPEN)
                .bind(ref_keys::KEY_JUDICIAL_CASE_JURY_ACTIVE)
                .bind(ref_keys::KEY_JUDICIAL_CASE_ESCALATED_TO_AI)
                .bind(ref_keys::KEY_POST_SUPPRESSED)
                .bind(ref_keys::KEY_JUDICIAL_CASE_CONTAINED)
                .bind(page_size + 1)
                .bind(row_offset);
            q.fetch_all(pool).await?
        };
        rows_trusted
    } else {
        let audience_ids = viewer_feed_audience_entity_ids(pool, user_id).await?;
        let owned_entity_ids = viewer_owned_entity_ids(pool, user_id).await?;
        let viewer_human_entity_id = human_onboarding::primary_human_entity_id(pool, user_id)
            .await?
            .unwrap_or(0);
        let ph = audience_entity_placeholders(&audience_ids);
        let ph_owned = audience_entity_placeholders(&owned_entity_ids);
        let vis = home_feed_audience_visibility_sql(&ph, &ph_owned);
        let sql = format!(
            r#"
            SELECT
                {select_cols}
            {from_joins}
            WHERE p.root_post_id IS NULL
            {vis}
            {mentions_sql}
            {order_by}
            LIMIT ? OFFSET ?
            "#,
            select_cols = feed_select_columns_sql(),
            from_joins = FEED_FROM_JOINS,
            vis = vis,
            mentions_sql = mentions_sql,
            order_by = FEED_ORDER_BY_PIN_JUDICIAL_SQL,
        );
        let mut q = sqlx::query_as::<_, HomeFeedRow>(&sql)
            .bind(ref_keys::GROUP_STATUS)
            .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
            .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
            .bind(user_id);
        q = bind_home_feed_visibility(
            q,
            user_id,
            viewer_human_entity_id,
            &audience_ids,
            &owned_entity_ids,
        );
        if mentions_only {
            q = bind_feed_about_viewer_filter(q, user_id, viewer_human_entity_id);
        }
        q = q
            .bind(user_id)
            .bind(ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW)
            .bind(ref_keys::KEY_JUDICIAL_CASE_OPEN)
            .bind(ref_keys::KEY_JUDICIAL_CASE_JURY_ACTIVE)
            .bind(ref_keys::KEY_JUDICIAL_CASE_ESCALATED_TO_AI)
            .bind(ref_keys::KEY_POST_SUPPRESSED)
            .bind(ref_keys::KEY_JUDICIAL_CASE_CONTAINED)
            .bind(page_size + 1)
            .bind(row_offset);
        q.fetch_all(pool).await?
    };

    let has_older = rows.len() > page_size as usize;
    if has_older {
        rows.truncate(page_size as usize);
    }

    Ok(FeedPageRows {
        rows,
        has_newer: page > 0,
        has_older,
    })
}

/// Root post row visible to the viewer under the same rules as the signed-in aggregate home feed
/// ([`fetch_page_rows`] without a profile handle). `post_id` may be a thread reply; the root row
/// is resolved first.
pub async fn fetch_visible_root_post_for_viewer(
    pool: &sqlx::MySqlPool,
    viewer_user_id: u64,
    post_id: u64,
) -> Result<Option<HomeFeedRow>, sqlx::Error> {
    let root_id: Option<u64> = sqlx::query_scalar(
        r#"SELECT COALESCE(p.root_post_id, p.id) FROM post p WHERE p.id = ?"#,
    )
    .bind(post_id)
    .fetch_optional(pool)
    .await?;

    let Some(root_id) = root_id else {
        return Ok(None);
    };

    let audience_ids = viewer_feed_audience_entity_ids(pool, viewer_user_id).await?;
    let owned_entity_ids = viewer_owned_entity_ids(pool, viewer_user_id).await?;
    let viewer_human_entity_id = human_onboarding::primary_human_entity_id(pool, viewer_user_id)
        .await?
        .unwrap_or(0);
    let ph = audience_entity_placeholders(&audience_ids);
    let ph_owned = audience_entity_placeholders(&owned_entity_ids);
    let vis = home_feed_audience_visibility_sql(&ph, &ph_owned);
    let sql = format!(
        r#"
            SELECT
                {select_cols}
            {from_joins}
            WHERE p.root_post_id IS NULL
            {vis}
              AND p.id = ?
            LIMIT 1
            "#,
        select_cols = feed_select_columns_sql(),
        from_joins = FEED_FROM_JOINS,
        vis = vis,
    );
    let mut q = sqlx::query_as::<_, HomeFeedRow>(&sql)
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
        .bind(viewer_user_id);
    q = bind_home_feed_visibility(
        q,
        viewer_user_id,
        viewer_human_entity_id,
        &audience_ids,
        &owned_entity_ids,
    );
    q.bind(root_id).fetch_optional(pool).await
}

/// Root posts visible in the viewer's aggregate home feed published after `since` (excludes own posts).
pub async fn count_missed_feed_roots_since(
    pool: &MySqlPool,
    viewer_user_id: u64,
    since: DateTime<Utc>,
) -> Result<u32, sqlx::Error> {
    let audience_ids = viewer_feed_audience_entity_ids(pool, viewer_user_id).await?;
    let owned_entity_ids = viewer_owned_entity_ids(pool, viewer_user_id).await?;
    let viewer_human_entity_id = human_onboarding::primary_human_entity_id(pool, viewer_user_id)
        .await?
        .unwrap_or(0);
    let ph = audience_entity_placeholders(&audience_ids);
    let ph_owned = audience_entity_placeholders(&owned_entity_ids);
    let vis = home_feed_audience_visibility_sql(&ph, &ph_owned);
    let since_naive = since.naive_utc();
    let sql = format!(
        r#"
        SELECT COUNT(*)
        {from_joins}
        WHERE p.root_post_id IS NULL
          AND COALESCE(p.published_at, p.created_at) > ?
          AND p.created_by <> ?
        {vis}
        "#,
        from_joins = FEED_FROM_JOINS,
        vis = vis,
    );
    let mut q = sqlx::query_scalar::<_, i64>(&sql)
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
        .bind(viewer_user_id)
        .bind(ref_keys::KEY_POST_SUPPRESSED)
        .bind(viewer_user_id)
        .bind(ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW)
        .bind(viewer_user_id)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
        .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
        .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
        .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_BLOCKED)
        .bind(viewer_human_entity_id)
        .bind(viewer_human_entity_id)
        .bind(ref_keys::GROUP_USER_STATUS)
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_BLOCKED)
        .bind(viewer_user_id);
    for id in &audience_ids {
        q = q.bind(*id);
    }
    for id in &owned_entity_ids {
        q = q.bind(*id);
    }
    q = q
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_VERIFIED)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
        .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
        .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
        .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
        .bind(viewer_human_entity_id)
        .bind(viewer_human_entity_id);
    q = q.bind(viewer_user_id);
    for id in &audience_ids {
        q = q.bind(*id);
    }
    for id in &owned_entity_ids {
        q = q.bind(*id);
    }
    q = q
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_VERIFIED)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
        .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
        .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
        .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
        .bind(viewer_human_entity_id)
        .bind(viewer_human_entity_id);
    let count = q.bind(since_naive).bind(viewer_user_id).fetch_one(pool).await?;
    Ok(count.max(0) as u32)
}

/// Full [`HomeFeedRow`] for the disputed root post when `invite_id` is an active juror seat
/// (`pending` or `accepted` on an open/jury_active case) for `viewer_user_id`. Used by the juror
/// workspace only — not the home feed — so callers can render a de-personalized corpus.
#[must_use]
pub async fn fetch_root_post_row_for_pending_juror_invite(
    pool: &sqlx::MySqlPool,
    viewer_user_id: u64,
    invite_id: u64,
) -> Result<Option<HomeFeedRow>, sqlx::Error> {
    let root_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT COALESCE(pr.root_post_id, pr.id)
        FROM judicial_case_juror_invite jcji
        INNER JOIN judicial_case jc ON jc.id = jcji.judicial_case_id
        INNER JOIN post pr ON pr.id = jc.subject_post_id
        INNER JOIN ref_key_values rk_inv
            ON rk_inv.id = jcji.invite_status_id
           AND rk_inv.`group` = ?
           AND rk_inv.`key` IN (?, ?)
           AND rk_inv.is_active = 1
        INNER JOIN ref_key_values rk_jc_st
            ON rk_jc_st.id = jc.status_id
           AND rk_jc_st.`group` = ?
           AND rk_jc_st.`key` IN (?, ?)
        WHERE jcji.id = ?
          AND jcji.juror_user_id = ?
          AND jc.closed_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_PENDING)
    .bind(ref_keys::KEY_JCJI_ACCEPTED)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_OPEN)
    .bind(ref_keys::KEY_JUDICIAL_CASE_JURY_ACTIVE)
    .bind(invite_id)
    .bind(viewer_user_id)
    .fetch_optional(pool)
    .await?;

    let Some(root_id) = root_id else {
        return Ok(None);
    };

    let sql = format!(
        r#"
            SELECT
                {select_cols}
            {from_joins}
            WHERE p.root_post_id IS NULL
              AND p.id = ?
            LIMIT 1
            "#,
        select_cols = feed_select_columns_sql(),
        from_joins = FEED_FROM_JOINS,
    );
    sqlx::query_as::<_, HomeFeedRow>(&sql)
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
        .bind(viewer_user_id)
        .bind(root_id)
        .fetch_optional(pool)
        .await
}
