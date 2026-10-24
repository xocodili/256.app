//! Civic engagement on feed posts: weighted yes/no votes, feel labels, thread caps.

use rust_decimal::Decimal;
use sqlx::{MySql, MySqlPool, Transaction};

use crate::ref_keys;
use crate::social::judicial::RATING_NORMALIZATION_DIVISOR;

pub const MAX_POSTS_PER_USER_PER_DAY: i64 = 256;
pub const CAP_ROOT_CHILDREN: i64 = 256;
pub const CAP_LAYER1_REPLIES: i64 = 16;
pub const CAP_LAYER2_REPLIES: i64 = 4;

/// Correlated subquery fragment: non-mirror reply count for root post alias `p`.
pub const SQL_ENGAGEMENT_COMMENT_COUNT: &str = r#"(
    SELECT COUNT(*)
    FROM post c_eng
    WHERE c_eng.root_post_id = p.id
      AND c_eng.id <> p.id
      AND c_eng.is_moderation_mirror = 0
)"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngagementError {
    DailyPostCap,
    ThreadBranchFull,
    ThreadDepthLimit,
    CommunityAudienceGate,
    SelfVote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngagementDirection {
    Yes,
    No,
}

impl EngagementDirection {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Yes => "yes",
            Self::No => "no",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "yes" | "up" | "for" => Some(Self::Yes),
            "no" | "down" | "against" => Some(Self::No),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostFeel {
    Loved,
    Controversial,
    Loathed,
}

impl PostFeel {
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Loved => "loved",
            Self::Controversial => "controversial",
            Self::Loathed => "loathed",
        }
    }

    #[must_use]
    pub fn from_stars(stars: f64) -> Self {
        if stars < 2.0 {
            Self::Loathed
        } else if stars <= 3.0 {
            Self::Controversial
        } else {
            Self::Loved
        }
    }
}

/// Stars on 0..5 from frozen vote weights.
#[must_use]
pub fn stars_from_weights(w_yes: f64, w_no: f64) -> f64 {
    let total = w_yes + w_no;
    if total <= f64::EPSILON {
        return 2.5;
    }
    let r = w_yes / total;
    (r * 5.0).clamp(0.0, 5.0)
}

pub async fn human_entity_id_for_user(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<Option<u64>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT sh.social_entity_id
        FROM social_human sh
        INNER JOIN social_entity se ON se.id = sh.social_entity_id
        WHERE se.created_by = ?
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

pub async fn voter_weight_for_user_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> Result<Decimal, sqlx::Error> {
    let rating: Option<Decimal> = sqlx::query_scalar(
        r#"
        SELECT COALESCE(se.rating, 0.00)
        FROM social_human sh
        INNER JOIN social_entity se ON se.id = sh.social_entity_id
        WHERE se.created_by = ?
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?;
    let rating = rating.unwrap_or(Decimal::ZERO);
    let r = rating.to_string().parse::<f64>().unwrap_or(0.0);
    let w = r / RATING_NORMALIZATION_DIVISOR;
    Ok(Decimal::from_f64_retain(w.max(0.0)).unwrap_or(Decimal::ZERO))
}

pub async fn check_daily_post_cap_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> Result<(), EngagementError> {
    let posts_today: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM post
        WHERE created_by = ?
          AND created_at >= UTC_DATE()
          AND created_at < (UTC_DATE() + INTERVAL 1 DAY)
        "#,
    )
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|_| EngagementError::DailyPostCap)?;
    if posts_today >= MAX_POSTS_PER_USER_PER_DAY {
        return Err(EngagementError::DailyPostCap);
    }
    Ok(())
}

/// Depth of `post_id` in its thread: 0 = root, 1 = L1, 2 = L2, 3 = L3.
pub async fn thread_depth_tx(
    tx: &mut Transaction<'_, MySql>,
    post_id: u64,
    root_id: u64,
) -> Result<u8, sqlx::Error> {
    if post_id == root_id {
        return Ok(0);
    }
    let parent: Option<u64> = sqlx::query_scalar(
        r#"SELECT parent_post_id FROM post WHERE id = ? LIMIT 1"#,
    )
    .bind(post_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(parent_id) = parent else {
        return Ok(0);
    };
    if parent_id == root_id {
        return Ok(1);
    }
    let grand: Option<u64> = sqlx::query_scalar(
        r#"SELECT parent_post_id FROM post WHERE id = ? LIMIT 1"#,
    )
    .bind(parent_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(grand_id) = grand else {
        return Ok(2);
    };
    if grand_id == root_id {
        return Ok(2);
    }
    Ok(3)
}

#[must_use]
pub fn max_children_for_parent_depth(parent_depth: u8) -> Option<i64> {
    match parent_depth {
        0 => Some(CAP_ROOT_CHILDREN),
        1 => Some(CAP_LAYER1_REPLIES),
        2 => Some(CAP_LAYER2_REPLIES),
        _ => None,
    }
}

/// Whether another direct reply may be added under a comment at `depth` (1 = L1, 2 = L2).
#[must_use]
pub fn can_add_reply_at_depth(depth: u8, child_count: u64) -> bool {
    if depth >= 3 {
        return false;
    }
    max_children_for_parent_depth(depth).is_some_and(|cap| (child_count as i64) < cap)
}

pub async fn assert_can_add_child_tx(
    tx: &mut Transaction<'_, MySql>,
    parent_id: u64,
    root_id: u64,
    is_moderation_mirror: bool,
) -> Result<(), EngagementError> {
    if is_moderation_mirror {
        return Ok(());
    }
    let parent_depth = thread_depth_tx(tx, parent_id, root_id)
        .await
        .map_err(|_| EngagementError::ThreadDepthLimit)?;
    let Some(cap) = max_children_for_parent_depth(parent_depth) else {
        return Err(EngagementError::ThreadDepthLimit);
    };
    if parent_depth >= 3 {
        return Err(EngagementError::ThreadDepthLimit);
    }
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM post
        WHERE parent_post_id = ?
          AND is_moderation_mirror = 0
        "#,
    )
    .bind(parent_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|_| EngagementError::ThreadBranchFull)?;
    if count >= cap {
        return Err(EngagementError::ThreadBranchFull);
    }
    Ok(())
}

pub async fn recompute_post_engagement_rating_tx(
    tx: &mut Transaction<'_, MySql>,
    target_post_id: u64,
) -> Result<f64, sqlx::Error> {
    let rows: Vec<(String, Decimal)> = sqlx::query_as(
        r#"
        SELECT pev.direction, pev.voter_weight
        FROM post_engagement_vote pev
        INNER JOIN post p ON p.id = pev.target_post_id
        WHERE pev.target_post_id = ?
          AND pev.voter_user_id <> p.created_by
        "#,
    )
    .bind(target_post_id)
    .fetch_all(&mut **tx)
    .await?;

    let mut w_yes = 0.0_f64;
    let mut w_no = 0.0_f64;
    for (dir, weight) in rows {
        let w = weight.to_string().parse::<f64>().unwrap_or(0.0);
        if dir == "yes" {
            w_yes += w;
        } else {
            w_no += w;
        }
    }
    let stars = stars_from_weights(w_yes, w_no);
    let rating_dec =
        Decimal::from_f64_retain(stars).unwrap_or_else(|| Decimal::from(0));
    sqlx::query(
        r#"
        UPDATE post
        SET rating = ?
        WHERE id = ?
        "#,
    )
    .bind(rating_dec)
    .bind(target_post_id)
    .execute(&mut **tx)
    .await?;
    Ok(stars)
}

const SELF_VOTE_PROTOCOL: &str = "engagement:self_vote";

pub async fn assert_not_self_vote_tx(
    tx: &mut Transaction<'_, MySql>,
    target_post_id: u64,
    voter_user_id: u64,
) -> Result<(), sqlx::Error> {
    let row: Option<(Option<u64>,)> = sqlx::query_as(
        r#"SELECT created_by FROM post WHERE id = ? LIMIT 1"#,
    )
    .bind(target_post_id)
    .fetch_optional(&mut **tx)
    .await?;
    if row.and_then(|(author,)| author) == Some(voter_user_id) {
        return Err(sqlx::Error::Protocol(SELF_VOTE_PROTOCOL.into()));
    }
    Ok(())
}

pub async fn record_engagement_vote_tx(
    tx: &mut Transaction<'_, MySql>,
    root_post_id: u64,
    target_post_id: u64,
    voter_user_id: u64,
    direction: EngagementDirection,
) -> Result<f64, sqlx::Error> {
    assert_not_self_vote_tx(tx, target_post_id, voter_user_id).await?;
    let weight = voter_weight_for_user_tx(tx, voter_user_id).await?;
    sqlx::query(
        r#"
        INSERT INTO post_engagement_vote (root_post_id, target_post_id, voter_user_id, direction, voter_weight)
        VALUES (?, ?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE
            direction = VALUES(direction),
            voter_weight = VALUES(voter_weight),
            updated_at = UTC_TIMESTAMP(6)
        "#,
    )
    .bind(root_post_id)
    .bind(target_post_id)
    .bind(voter_user_id)
    .bind(direction.as_str())
    .bind(weight)
    .execute(&mut **tx)
    .await?;
    recompute_post_engagement_rating_tx(tx, target_post_id).await
}

pub async fn record_root_engagement_vote_tx(
    tx: &mut Transaction<'_, MySql>,
    root_post_id: u64,
    voter_user_id: u64,
    direction: EngagementDirection,
) -> Result<f64, sqlx::Error> {
    record_engagement_vote_tx(tx, root_post_id, root_post_id, voter_user_id, direction).await
}

#[must_use]
pub fn is_self_vote_error(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Protocol(msg) if msg == SELF_VOTE_PROTOCOL)
}

pub async fn viewer_vote_direction(
    pool: &MySqlPool,
    target_post_id: u64,
    viewer_user_id: u64,
) -> Result<Option<EngagementDirection>, sqlx::Error> {
    let dir: Option<String> = sqlx::query_scalar(
        r#"
        SELECT direction
        FROM post_engagement_vote
        WHERE target_post_id = ? AND voter_user_id = ?
        LIMIT 1
        "#,
    )
    .bind(target_post_id)
    .bind(viewer_user_id)
    .fetch_optional(pool)
    .await?;
    Ok(dir.and_then(|d| EngagementDirection::parse(&d)))
}

pub async fn non_author_root_vote_count(
    pool: &MySqlPool,
    root_post_id: u64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM post_engagement_vote pev
        INNER JOIN post p ON p.id = pev.target_post_id
        WHERE pev.target_post_id = ?
          AND pev.voter_user_id <> p.created_by
        "#,
    )
    .bind(root_post_id)
    .fetch_one(pool)
    .await
}

async fn viewer_is_post_author(
    pool: &MySqlPool,
    post_id: u64,
    viewer_user_id: u64,
) -> Result<bool, sqlx::Error> {
    let row: Option<(Option<u64>,)> = sqlx::query_as(
        r#"SELECT created_by FROM post WHERE id = ? LIMIT 1"#,
    )
    .bind(post_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|(author,)| author) == Some(viewer_user_id))
}

/// True when the post author may view/participate because someone else voted on the root.
pub async fn author_discussion_unlocked(
    pool: &MySqlPool,
    root_post_id: u64,
    viewer_user_id: u64,
) -> Result<bool, sqlx::Error> {
    if !viewer_is_post_author(pool, root_post_id, viewer_user_id).await? {
        return Ok(false);
    }
    Ok(non_author_root_vote_count(pool, root_post_id).await? > 0)
}

/// Viewer may see the thread and comment or vote on replies (not their own content).
pub async fn can_participate_in_root_discussion(
    pool: &MySqlPool,
    root_post_id: u64,
    viewer_user_id: u64,
) -> Result<bool, sqlx::Error> {
    if viewer_vote_direction(pool, root_post_id, viewer_user_id)
        .await?
        .is_some()
    {
        return Ok(true);
    }
    author_discussion_unlocked(pool, root_post_id, viewer_user_id).await
}

pub async fn assert_community_audience_gate_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    audience_entity_ids: &[u64],
) -> Result<(), sqlx::Error> {
    if audience_entity_ids.is_empty() {
        return Ok(());
    }
    let Some(human_entity_id) = sqlx::query_scalar::<_, u64>(
        r#"
        SELECT sh.social_entity_id
        FROM social_human sh
        INNER JOIN social_entity se ON se.id = sh.social_entity_id
        WHERE se.created_by = ?
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Err(sqlx::Error::Protocol("community gate: no human".into()));
    };

    for community_id in audience_entity_ids {
        let is_community: Option<u64> = sqlx::query_scalar(
            r#"
            SELECT sc.social_entity_id
            FROM social_community sc
            WHERE sc.social_entity_id = ?
            LIMIT 1
            "#,
        )
        .bind(community_id)
        .fetch_optional(&mut **tx)
        .await?;
        if is_community.is_none() {
            continue;
        }
        let ok: Option<u64> = sqlx::query_scalar(
            r#"
            SELECT ser.id
            FROM social_entity_relationship ser
            INNER JOIN ref_key_values rk_type
                ON rk_type.id = ser.type_id
               AND rk_type.`group` = ?
               AND rk_type.`key` = ?
               AND rk_type.is_active = 1
            INNER JOIN ref_key_values rk_st
                ON rk_st.id = ser.status_id
               AND rk_st.`group` = ?
               AND rk_st.`key` = ?
               AND rk_st.is_active = 1
            WHERE ser.ended_at IS NULL
              AND ser.rating >= 1.00
              AND (
                    (ser.from_social_entity_id = ? AND ser.to_social_entity_id = ?)
                 OR (ser.from_social_entity_id = ? AND ser.to_social_entity_id = ?)
              )
            LIMIT 1
            "#,
        )
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
        .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_VERIFIED)
        .bind(human_entity_id)
        .bind(community_id)
        .bind(community_id)
        .bind(human_entity_id)
        .fetch_optional(&mut **tx)
        .await?;
        if ok.is_none() {
            return Err(sqlx::Error::RowNotFound);
        }
    }
    Ok(())
}

pub async fn check_community_audience_gate_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    audience_entity_ids: &[u64],
) -> Result<(), EngagementError> {
    assert_community_audience_gate_tx(tx, user_id, audience_entity_ids)
        .await
        .map_err(|_| EngagementError::CommunityAudienceGate)
}
