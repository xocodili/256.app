//! Away-user engagement digest — every 3 idle days (3, 6, 9 …) until inactivity mail at day 21.

use chrono::{DateTime, Utc};
use sqlx::MySqlPool;

use crate::ref_keys;
use crate::social::community_graph::INACTIVITY_WARNING_IDLE_DAYS;
use crate::social::feed;

/// Minimum idle days before the first digest.
pub const ENGAGEMENT_DIGEST_IDLE_DAYS: i64 = 3;
/// Resend cadence while away (day 3, 6, 9, 12, 15, 18).
pub const ENGAGEMENT_DIGEST_INTERVAL_DAYS: i64 = 3;

#[derive(Debug, Clone, Default)]
pub struct EngagementDigestCounts {
    pub pending_connections: u32,
    pub audience_mentions: u32,
    pub replies_to_author: u32,
    pub missed_feed_posts: u32,
    pub community_posts: u32,
}

impl EngagementDigestCounts {
    #[must_use]
    pub fn has_sendable_content(&self) -> bool {
        self.pending_connections > 0
            || self.audience_mentions > 0
            || self.replies_to_author > 0
            || self.missed_feed_posts > 0
            || self.community_posts > 0
    }

    /// True when only the community fallback line would appear (no specific activity).
    #[must_use]
    pub fn is_community_only(&self) -> bool {
        self.community_posts > 0
            && self.pending_connections == 0
            && self.audience_mentions == 0
            && self.replies_to_author == 0
            && self.missed_feed_posts == 0
    }
}

#[derive(Debug, sqlx::FromRow)]
struct DigestCandidate {
    user_id: u64,
    email: String,
    human_entity_id: u64,
    display_label: Option<String>,
    handle: String,
    last_login_at: DateTime<Utc>,
}

/// Verified humans idle ≥ [`ENGAGEMENT_DIGEST_IDLE_DAYS`] and &lt; [`INACTIVITY_WARNING_IDLE_DAYS`]
/// (21d), due for a digest every [`ENGAGEMENT_DIGEST_INTERVAL_DAYS`] since the last one or sign-in.
pub async fn run_user_engagement_digest_batch(
    pool: &MySqlPool,
    config: &crate::config::Config,
    limit: u64,
) -> Result<u64, sqlx::Error> {
    let Some(_smtp) = config.smtp.as_ref() else {
        return Ok(0);
    };
    if !config.ops_digest_live {
        tracing::debug!("user engagement digest skipped (OPS_DIGEST_LIVE=0)");
        return Ok(0);
    }

    let lim = limit.clamp(1, 128) as i64;
    let candidates: Vec<DigestCandidate> = sqlx::query_as(
        r#"
        SELECT
            u.id AS user_id,
            u.email,
            se.id AS human_entity_id,
            se.handle,
            COALESCE(
                NULLIF(TRIM(CONCAT_WS(' ', sh.given_name, sh.additional_name, sh.family_name)), ''),
                se.handle
            ) AS display_label,
            COALESCE(u.last_login_at, u.created_at) AS last_login_at
        FROM users u
        INNER JOIN user_social_entity_role ur
            ON ur.user_id = u.id
           AND ur.is_primary = 1
           AND (ur.ends_at IS NULL OR ur.ends_at >= UTC_DATE())
        INNER JOIN ref_key_values rk_role
            ON rk_role.id = ur.role_id
           AND rk_role.`group` = 'user_social_entity_role'
           AND rk_role.`key` = 'owner'
        INNER JOIN social_entity se ON se.id = ur.social_entity_id
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = se.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` = ?
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = se.entity_status_id
           AND rk_st.`group` = ?
           AND rk_st.`key` = ?
        WHERE u.email_verified_at IS NOT NULL
          AND TRIM(u.email) <> ''
          AND sh.died_at IS NULL
          AND se.rating > 0.00
          AND TIMESTAMPDIFF(
                DAY,
                COALESCE(u.last_login_at, u.created_at),
                UTC_TIMESTAMP(6)
              ) >= ?
          AND TIMESTAMPDIFF(
                DAY,
                COALESCE(u.last_login_at, u.created_at),
                UTC_TIMESTAMP(6)
              ) < ?
          AND (
                u.engagement_digest_sent_at IS NULL
                OR u.engagement_digest_sent_at < COALESCE(u.last_login_at, u.created_at)
                OR TIMESTAMPDIFF(
                    DAY,
                    u.engagement_digest_sent_at,
                    UTC_TIMESTAMP(6)
                ) >= ?
              )
        ORDER BY u.id ASC
        LIMIT ?
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(ENGAGEMENT_DIGEST_IDLE_DAYS)
    .bind(INACTIVITY_WARNING_IDLE_DAYS)
    .bind(ENGAGEMENT_DIGEST_INTERVAL_DAYS)
    .bind(lim)
    .fetch_all(pool)
    .await?;

    let mut sent = 0u64;
    for row in candidates {
        let label = row
            .display_label
            .clone()
            .unwrap_or_else(|| row.handle.clone());
        let counts = collect_counts(pool, row.user_id, row.human_entity_id, row.last_login_at).await?;
        if !counts.has_sendable_content() {
            continue;
        }

        let claimed: u64 = sqlx::query(
            r#"
            UPDATE users
            SET engagement_digest_sent_at = UTC_TIMESTAMP(6)
            WHERE id = ?
              AND TIMESTAMPDIFF(
                    DAY,
                    COALESCE(last_login_at, created_at),
                    UTC_TIMESTAMP(6)
                  ) >= ?
              AND TIMESTAMPDIFF(
                    DAY,
                    COALESCE(last_login_at, created_at),
                    UTC_TIMESTAMP(6)
                  ) < ?
              AND (
                    engagement_digest_sent_at IS NULL
                    OR engagement_digest_sent_at < COALESCE(last_login_at, created_at)
                    OR TIMESTAMPDIFF(
                        DAY,
                        engagement_digest_sent_at,
                        UTC_TIMESTAMP(6)
                    ) >= ?
                  )
            "#,
        )
        .bind(row.user_id)
        .bind(ENGAGEMENT_DIGEST_IDLE_DAYS)
        .bind(INACTIVITY_WARNING_IDLE_DAYS)
        .bind(ENGAGEMENT_DIGEST_INTERVAL_DAYS)
        .execute(pool)
        .await?
        .rows_affected();
        if claimed == 0 {
            continue;
        }

        crate::email::notify::send_user_engagement_digest(
            config,
            &row.email,
            &label,
            &counts,
        )
        .await;
        sent += 1;
    }

    Ok(sent)
}

async fn collect_counts(
    pool: &MySqlPool,
    user_id: u64,
    human_entity_id: u64,
    since: DateTime<Utc>,
) -> Result<EngagementDigestCounts, sqlx::Error> {
    let since_naive = since.naive_utc();
    let pending_connections = count_pending_connections_since(pool, human_entity_id, since_naive).await?;
    let audience_mentions =
        count_audience_mentions_since(pool, user_id, human_entity_id, since_naive).await?;
    let replies_to_author =
        count_replies_to_author_since(pool, user_id, human_entity_id, since_naive).await?;
    let missed_feed_posts = feed::count_missed_feed_roots_since(pool, user_id, since).await?;
    let community_posts =
        count_community_posts_since(pool, user_id, human_entity_id, since_naive).await?;

    Ok(EngagementDigestCounts {
        pending_connections,
        audience_mentions,
        replies_to_author,
        missed_feed_posts,
        community_posts,
    })
}

async fn count_pending_connections_since(
    pool: &MySqlPool,
    human_entity_id: u64,
    since: chrono::NaiveDateTime,
) -> Result<u32, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = ser.type_id
           AND rk_type.`group` = ?
        INNER JOIN ref_key_values rk_status
            ON rk_status.id = ser.status_id
           AND rk_status.`group` = ?
           AND rk_status.`key` = ?
        WHERE ser.ended_at IS NULL
          AND ser.created_at > ?
          AND (
                (
                    rk_type.`key` IN (?, ?, ?)
                    AND ser.to_social_entity_id = ?
                )
                OR (
                    rk_type.`key` = ?
                    AND ser.from_social_entity_id = ?
                    AND EXISTS (
                        SELECT 1
                        FROM social_entity se_from
                        INNER JOIN ref_key_values rk_from_type
                            ON rk_from_type.id = se_from.type_id
                           AND rk_from_type.`group` = ?
                           AND rk_from_type.`key` = ?
                        WHERE se_from.id = ser.from_social_entity_id
                    )
                )
          )
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_PENDING_VERIFICATION)
    .bind(since)
    .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
    .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
    .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
    .bind(human_entity_id)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .bind(human_entity_id)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .fetch_one(pool)
    .await?;
    Ok(count.max(0) as u32)
}

async fn count_audience_mentions_since(
    pool: &MySqlPool,
    user_id: u64,
    human_entity_id: u64,
    since: chrono::NaiveDateTime,
) -> Result<u32, sqlx::Error> {
    let audience_ids = feed::viewer_feed_audience_entity_ids(pool, user_id).await?;
    let ph = if audience_ids.is_empty() {
        "NULL".to_string()
    } else {
        std::iter::repeat_n("?", audience_ids.len())
            .collect::<Vec<_>>()
            .join(",")
    };

    let sql = format!(
        r#"
        SELECT COUNT(DISTINCT p.id)
        FROM post p
        WHERE p.root_post_id IS NULL
          AND COALESCE(p.published_at, p.created_at) > ?
          AND p.created_by <> ?
          AND (
                EXISTS (
                    SELECT 1
                    FROM post_mention pm
                    INNER JOIN post pm_post ON pm_post.id = pm.post_id
                    WHERE pm.mentioned_entity_id = ?
                      AND COALESCE(pm_post.root_post_id, pm_post.id) = p.id
                )
                OR EXISTS (
                    SELECT 1
                    FROM post_audience_entity pae
                    INNER JOIN post pae_post ON pae_post.id = pae.post_id
                    WHERE COALESCE(pae_post.root_post_id, pae_post.id) = p.id
                      AND pae.audience_entity_id IN ({ph})
                )
                OR EXISTS (
                    SELECT 1
                    FROM post_audience_scope pas
                    INNER JOIN post pas_post ON pas_post.id = pas.post_id
                    WHERE COALESCE(pas_post.root_post_id, pas_post.id) = p.id
                      AND pas.scope_key IN ('friends', 'family', 'colleague')
                      AND EXISTS (
                          SELECT 1
                          FROM social_entity_relationship ser
                          INNER JOIN ref_key_values rk_st
                              ON rk_st.id = ser.status_id
                             AND rk_st.`group` = ?
                             AND rk_st.`key` = ?
                          INNER JOIN ref_key_values rk_rel
                              ON rk_rel.id = ser.type_id
                             AND rk_rel.`group` = ?
                             AND (
                                  (pas.scope_key = 'friends' AND rk_rel.`key` = ?)
                                  OR (pas.scope_key = 'family' AND rk_rel.`key` = ?)
                                  OR (pas.scope_key = 'colleague' AND rk_rel.`key` = ?)
                             )
                          WHERE ser.ended_at IS NULL
                            AND (
                                  (ser.from_social_entity_id = p.entity_id AND ser.to_social_entity_id = ?)
                                  OR (ser.to_social_entity_id = p.entity_id AND ser.from_social_entity_id = ?)
                            )
                      )
                )
                OR EXISTS (
                    SELECT 1
                    FROM post_audience_scope pas
                    INNER JOIN post pas_post ON pas_post.id = pas.post_id
                    WHERE COALESCE(pas_post.root_post_id, pas_post.id) = p.id
                      AND pas.scope_key = 'private'
                      AND (
                            p.created_by = ?
                            OR EXISTS (
                                SELECT 1
                                FROM post_audience_entity pae
                                WHERE pae.post_id = p.id
                                  AND pae.audience_entity_id IN ({ph})
                            )
                      )
                )
          )
        "#
    );

    let mut q = sqlx::query_scalar::<_, i64>(&sql);
    q = q.bind(since).bind(user_id).bind(human_entity_id);
    for id in &audience_ids {
        q = q.bind(id);
    }
    q = q
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_VERIFIED)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
        .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
        .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
        .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
        .bind(human_entity_id)
        .bind(human_entity_id)
        .bind(user_id);
    for id in &audience_ids {
        q = q.bind(id);
    }
    let count = q.fetch_one(pool).await?;
    Ok(count.max(0) as u32)
}

async fn count_replies_to_author_since(
    pool: &MySqlPool,
    user_id: u64,
    human_entity_id: u64,
    since: chrono::NaiveDateTime,
) -> Result<u32, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM post c
        INNER JOIN post root ON root.id = c.root_post_id
        WHERE c.root_post_id IS NOT NULL
          AND COALESCE(c.published_at, c.created_at) > ?
          AND c.created_by <> ?
          AND (root.created_by = ? OR root.entity_id = ?)
        "#,
    )
    .bind(since)
    .bind(user_id)
    .bind(user_id)
    .bind(human_entity_id)
    .fetch_one(pool)
    .await?;
    Ok(count.max(0) as u32)
}

async fn count_community_posts_since(
    pool: &MySqlPool,
    user_id: u64,
    human_entity_id: u64,
    since: chrono::NaiveDateTime,
) -> Result<u32, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM post p
        INNER JOIN social_entity se ON se.id = p.entity_id
        INNER JOIN ref_key_values rk_et
            ON rk_et.id = se.type_id
           AND rk_et.`group` = ?
           AND rk_et.`key` = ?
        WHERE p.root_post_id IS NULL
          AND COALESCE(p.published_at, p.created_at) > ?
          AND p.created_by <> ?
          AND EXISTS (
                SELECT 1
                FROM social_entity_relationship ser
                INNER JOIN ref_key_values rk_type
                    ON rk_type.id = ser.type_id
                   AND rk_type.`group` = ?
                   AND rk_type.`key` = ?
                INNER JOIN ref_key_values rk_st
                    ON rk_st.id = ser.status_id
                   AND rk_st.`group` = ?
                   AND rk_st.`key` = ?
                WHERE ser.ended_at IS NULL
                  AND ser.from_social_entity_id = ?
                  AND ser.to_social_entity_id = p.entity_id
          )
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_COMMUNITY)
    .bind(since)
    .bind(user_id)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(human_entity_id)
    .fetch_one(pool)
    .await?;
    Ok(count.max(0) as u32)
}
