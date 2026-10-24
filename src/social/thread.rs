//! Thread read model for post conversations (vote-gated, three-level tree).

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::MySqlPool;

use crate::social::blocks;
use crate::social::community_graph;
use crate::social::engagement::{self, PostFeel};
use crate::social::human_onboarding;
use crate::social::public_profile;
use crate::ui::avatar_url;
use crate::ui::entity_handle;

pub const THREAD_L1_PAGE_SIZE: usize = 3;

#[derive(Debug, sqlx::FromRow)]
struct ThreadPostRow {
    id: u64,
    parent_post_id: Option<u64>,
    entity_id: u64,
    handle: String,
    display_name: Option<String>,
    avatar_path: Option<String>,
    body_html_sanitized: String,
    body_markup: String,
    body_plain: String,
    is_moderation_mirror: i8,
    source_judicial_case_comment_id: Option<u64>,
    created_at: DateTime<Utc>,
    immediate_parent_id: Option<u64>,
    immediate_parent_handle: Option<String>,
    immediate_parent_entity_id: Option<u64>,
    entity_rating: Decimal,
    entity_status_key: Option<String>,
    post_rating: Decimal,
    created_by: Option<u64>,
    country_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ThreadReplyRef {
    pub id: u64,
    pub handle: String,
}

#[derive(Debug, Serialize)]
pub struct ThreadComment {
    pub id: u64,
    pub parent_id: u64,
    pub depth: u8,
    pub handle: String,
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    pub body_html: String,
    pub is_moderation_mirror: bool,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<ThreadReplyRef>,
    pub child_count: u64,
    pub can_reply: bool,
    pub profile_linkable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_href: Option<String>,
    pub stars: f64,
    pub feel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewer_vote: Option<String>,
    #[serde(default)]
    pub can_vote: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub children: Vec<ThreadComment>,
    #[serde(default)]
    pub is_hidden: bool,
}

#[derive(Debug, Serialize)]
pub struct PostThreadResponse {
    pub root_post_id: u64,
    pub stars: f64,
    pub feel: String,
    pub comment_count: u64,
    pub viewer_vote: Option<String>,
    pub can_view_discussion: bool,
    #[serde(default)]
    pub preview_only: bool,
    pub mirrors: Vec<ThreadComment>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<ThreadComment>,
    pub has_more_l1: bool,
    pub l1_offset: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub l1_next_offset: Option<u32>,
}

fn row_body_html(r: &ThreadPostRow) -> String {
    if !r.body_html_sanitized.trim().is_empty() {
        return r.body_html_sanitized.clone();
    }
    if !r.body_markup.trim().is_empty() {
        return r.body_markup.clone();
    }
    r.body_plain.clone()
}

fn thread_profile_link(
    handle: &str,
    rating: Decimal,
    entity_status_key: Option<&str>,
) -> (Option<String>, bool) {
    let death_immune = community_graph::is_death_immune_handle(handle);
    let tone = entity_handle::entity_handle_tone(rating, entity_status_key, death_immune);
    let linkable = tone
        .map(entity_handle::entity_handle_is_linkable)
        .unwrap_or(true);
    let href = linkable.then(|| format!("/home/profile/{}", handle.trim()));
    (href, linkable)
}

fn row_to_comment(
    r: &ThreadPostRow,
    depth: u8,
    root_id: u64,
    viewer_user_id: u64,
    blocked: &HashSet<u64>,
) -> ThreadComment {
    if blocked.contains(&r.entity_id) {
        return hidden_comment_from_row(r, depth);
    }
    let reply_to = match (
        r.immediate_parent_id,
        r.immediate_parent_handle.as_deref(),
        r.immediate_parent_entity_id,
    ) {
        (Some(id), Some(handle), Some(parent_entity_id))
            if depth >= 2
                && id != root_id
                && !blocked.contains(&parent_entity_id) =>
        {
            Some(ThreadReplyRef {
                id,
                handle: handle.to_string(),
            })
        }
        _ => None,
    };
    let (profile_href, profile_linkable) =
        thread_profile_link(&r.handle, r.entity_rating, r.entity_status_key.as_deref());
    let can_vote = r.created_by.is_some_and(|author| author != viewer_user_id);
    ThreadComment {
        id: r.id,
        parent_id: r.parent_post_id.unwrap_or(0),
        depth,
        handle: r.handle.clone(),
        display_name: r.display_name.clone(),
        avatar_url: r
            .avatar_path
            .as_deref()
            .map(avatar_url::img_src_from_stored_avatar),
        body_html: row_body_html(r),
        is_moderation_mirror: r.is_moderation_mirror != 0,
        created_at: r.created_at,
        reply_to,
        child_count: 0,
        can_reply: false,
        profile_linkable,
        profile_href,
        stars: 2.5,
        feel: PostFeel::Controversial.key().to_string(),
        viewer_vote: None,
        can_vote,
        country_code: r
            .country_code
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_ascii_uppercase()),
        children: Vec::new(),
        is_hidden: false,
    }
}

fn hidden_comment_from_row(r: &ThreadPostRow, depth: u8) -> ThreadComment {
    ThreadComment {
        id: r.id,
        parent_id: r.parent_post_id.unwrap_or(0),
        depth,
        handle: String::new(),
        display_name: None,
        avatar_url: None,
        body_html: String::new(),
        is_moderation_mirror: false,
        created_at: r.created_at,
        reply_to: None,
        child_count: 0,
        can_reply: false,
        profile_linkable: false,
        profile_href: None,
        stars: 0.0,
        feel: String::new(),
        viewer_vote: None,
        can_vote: false,
        country_code: None,
        children: Vec::new(),
        is_hidden: true,
    }
}

fn thread_avatar_path_sql() -> String {
    format!(
        r#"COALESCE(
                NULLIF(TRIM(cl_sp_logo.url_canonical), ''),
                NULLIF(TRIM(cl_snp_logo.url_canonical), ''),
                {}
            )"#,
        public_profile::sql_avatar_path_expr("se")
    )
}

fn thread_child_select_sql() -> String {
    format!(
        r#"
        SELECT
            p.id,
            p.parent_post_id,
            p.entity_id,
            se.handle,
            COALESCE(
                NULLIF(TRIM(sc.label), ''),
                NULLIF(TRIM(CONCAT_WS(' ', sh.given_name, sh.family_name)), ''),
                se.handle
            ) AS display_name,
            {avatar_path} AS avatar_path,
            p.body_html_sanitized,
            p.body_markup,
            p.body_plain,
            p.is_moderation_mirror,
            p.source_judicial_case_comment_id,
            p.created_at,
            pref.id AS immediate_parent_id,
            se_pref.handle AS immediate_parent_handle,
            pref.entity_id AS immediate_parent_entity_id,
            COALESCE(se.rating, 0.00) AS entity_rating,
            (
                SELECT rk.`key`
                FROM ref_key_values rk
                WHERE rk.id = COALESCE(se.entity_status_id, se.status_id)
                LIMIT 1
            ) AS entity_status_key,
            COALESCE(p.rating, 0.00) AS post_rating,
            p.created_by,
            loc.country_code AS country_code
        FROM post p
        INNER JOIN social_entity se ON se.id = p.entity_id
        LEFT JOIN social_community sc ON sc.social_entity_id = se.id
        LEFT JOIN social_human sh ON sh.social_entity_id = se.id
        LEFT JOIN locations loc ON loc.id = se.location_id
        LEFT JOIN social_profit sp_ent ON sp_ent.social_entity_id = se.id
        LEFT JOIN content_link cl_sp_logo ON cl_sp_logo.id = sp_ent.logo_content_link_id
        LEFT JOIN social_non_profit snp_ent ON snp_ent.social_entity_id = se.id
        LEFT JOIN content_link cl_snp_logo ON cl_snp_logo.id = snp_ent.logo_content_link_id
        LEFT JOIN post pref ON pref.id = p.parent_post_id
        LEFT JOIN social_entity se_pref ON se_pref.id = pref.entity_id
        WHERE p.parent_post_id = ?
          AND COALESCE(p.root_post_id, p.id) = ?
          AND p.is_moderation_mirror = 0
"#,
        avatar_path = thread_avatar_path_sql()
    )
}

async fn fetch_children(
    pool: &MySqlPool,
    parent_id: u64,
    root_id: u64,
) -> Result<Vec<ThreadPostRow>, sqlx::Error> {
    let sql = format!(
        "{} ORDER BY p.created_at ASC, p.id ASC",
        thread_child_select_sql()
    );
    sqlx::query_as(&sql)
        .bind(parent_id)
        .bind(root_id)
        .fetch_all(pool)
        .await
}

async fn enrich_comment_engagement(
    pool: &MySqlPool,
    comment: &mut ThreadComment,
    post_rating: Decimal,
    viewer_user_id: u64,
) -> Result<(), sqlx::Error> {
    let stars = post_rating.to_string().parse::<f64>().unwrap_or(2.5);
    comment.stars = stars;
    comment.feel = PostFeel::from_stars(stars).key().to_string();
    comment.viewer_vote = engagement::viewer_vote_direction(pool, comment.id, viewer_user_id)
        .await?
        .map(|d| d.as_str().to_string());
    Ok(())
}

async fn build_comment_tree(
    pool: &MySqlPool,
    row: ThreadPostRow,
    depth: u8,
    root_id: u64,
    viewer_user_id: u64,
    blocked: &HashSet<u64>,
) -> Result<ThreadComment, sqlx::Error> {
    let hidden = blocked.contains(&row.entity_id);
    let mut comment = row_to_comment(&row, depth, root_id, viewer_user_id, blocked);
    if !hidden {
        enrich_comment_engagement(pool, &mut comment, row.post_rating, viewer_user_id).await?;
    }
    if depth >= 3 {
        return Ok(comment);
    }

    let child_rows = fetch_children(pool, row.id, root_id).await?;
    comment.child_count = child_rows.len() as u64;
    comment.can_reply = !hidden && engagement::can_add_reply_at_depth(depth, comment.child_count);

    let mut children = Vec::with_capacity(child_rows.len());
    for child_row in child_rows {
        if depth >= 2 {
            let child_hidden = blocked.contains(&child_row.entity_id);
            let mut child = row_to_comment(&child_row, depth + 1, root_id, viewer_user_id, blocked);
            if !child_hidden {
                enrich_comment_engagement(pool, &mut child, child_row.post_rating, viewer_user_id)
                    .await?;
            }
            children.push(child);
        } else {
            children.push(
                Box::pin(build_comment_tree(
                    pool,
                    child_row,
                    depth + 1,
                    root_id,
                    viewer_user_id,
                    blocked,
                ))
                .await?,
            );
        }
    }
    comment.children = children;

    Ok(comment)
}

pub async fn load_thread(
    pool: &MySqlPool,
    root_post_id: u64,
    viewer_user_id: u64,
    l1_offset: u32,
) -> Result<Option<PostThreadResponse>, sqlx::Error> {
    let root_exists: Option<u64> = sqlx::query_scalar(
        r#"SELECT id FROM post WHERE id = ? AND parent_post_id IS NULL LIMIT 1"#,
    )
    .bind(root_post_id)
    .fetch_optional(pool)
    .await?;
    if root_exists.is_none() {
        return Ok(None);
    }

    let rating: rust_decimal::Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(rating, 0.00) FROM post WHERE id = ?"#,
    )
    .bind(root_post_id)
    .fetch_one(pool)
    .await?;
    let stars = rating.to_string().parse::<f64>().unwrap_or(2.5);
    let feel = PostFeel::from_stars(stars).key().to_string();

    let comment_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM post
        WHERE root_post_id = ?
          AND id <> ?
          AND is_moderation_mirror = 0
        "#,
    )
    .bind(root_post_id)
    .bind(root_post_id)
    .fetch_one(pool)
    .await?;

    let viewer_vote = engagement::viewer_vote_direction(pool, root_post_id, viewer_user_id).await?;
    let can_view =
        engagement::can_participate_in_root_discussion(pool, root_post_id, viewer_user_id).await?;

    let blocked = match human_onboarding::primary_human_entity_id(pool, viewer_user_id).await? {
        Some(viewer_human_id) => blocks::blocked_peer_entity_ids(pool, viewer_human_id).await?,
        None => HashSet::new(),
    };

    let mirror_avatar = thread_avatar_path_sql();
    let mirror_rows: Vec<ThreadPostRow> = if can_view {
        sqlx::query_as(&format!(
            r#"
        SELECT
            p.id,
            p.parent_post_id,
            p.entity_id,
            se.handle,
            COALESCE(NULLIF(TRIM(sc.label), ''), se.handle) AS display_name,
            {mirror_avatar} AS avatar_path,
            p.body_html_sanitized,
            p.body_markup,
            p.body_plain,
            p.is_moderation_mirror,
            p.source_judicial_case_comment_id,
            p.created_at,
            NULL AS immediate_parent_id,
            NULL AS immediate_parent_handle,
            NULL AS immediate_parent_entity_id,
            COALESCE(se.rating, 0.00) AS entity_rating,
            (
                SELECT rk.`key`
                FROM ref_key_values rk
                WHERE rk.id = COALESCE(se.entity_status_id, se.status_id)
                LIMIT 1
            ) AS entity_status_key,
            COALESCE(p.rating, 0.00) AS post_rating,
            p.created_by,
            loc.country_code AS country_code
        FROM post p
        INNER JOIN social_entity se ON se.id = p.entity_id
        LEFT JOIN social_community sc ON sc.social_entity_id = se.id
        LEFT JOIN locations loc ON loc.id = se.location_id
        LEFT JOIN social_profit sp_ent ON sp_ent.social_entity_id = se.id
        LEFT JOIN content_link cl_sp_logo ON cl_sp_logo.id = sp_ent.logo_content_link_id
        LEFT JOIN social_non_profit snp_ent ON snp_ent.social_entity_id = se.id
        LEFT JOIN content_link cl_snp_logo ON cl_snp_logo.id = snp_ent.logo_content_link_id
        WHERE p.root_post_id = ?
          AND p.is_moderation_mirror = 1
        ORDER BY p.created_at DESC, p.id DESC
        "#,
        ))
        .bind(root_post_id)
        .fetch_all(pool)
        .await?
    } else {
        Vec::new()
    };

    let mirrors: Vec<ThreadComment> = mirror_rows
        .into_iter()
        .map(|r| row_to_comment(&r, 1, root_post_id, viewer_user_id, &blocked))
        .collect();

    let mut comments = Vec::new();
    let mut has_more_l1 = false;
    let mut preview_only = false;

    if can_view {
        let l1_sql = format!(
            "{} ORDER BY p.created_at DESC, p.id DESC",
            thread_child_select_sql()
        );
        let l1_all: Vec<ThreadPostRow> = sqlx::query_as(&l1_sql)
            .bind(root_post_id)
            .bind(root_post_id)
            .fetch_all(pool)
            .await?;
        let offset = l1_offset as usize;
        has_more_l1 = l1_all.len() > offset + THREAD_L1_PAGE_SIZE;
        for l1_row in l1_all
            .into_iter()
            .skip(offset)
            .take(THREAD_L1_PAGE_SIZE)
        {
            comments.push(
                build_comment_tree(
                    pool,
                    l1_row,
                    1,
                    root_post_id,
                    viewer_user_id,
                    &blocked,
                )
                .await?,
            );
        }
    } else if comment_count > 0 {
        preview_only = true;
        let preview_sql = format!(
            "{} ORDER BY COALESCE(p.rating, 0.00) DESC, p.created_at DESC, p.id DESC LIMIT {}",
            thread_child_select_sql(),
            THREAD_L1_PAGE_SIZE
        );
        let preview_rows: Vec<ThreadPostRow> = sqlx::query_as(&preview_sql)
            .bind(root_post_id)
            .bind(root_post_id)
            .fetch_all(pool)
            .await?;
        comments = preview_rows
            .into_iter()
            .map(|r| {
                let mut c = row_to_comment(&r, 1, root_post_id, viewer_user_id, &blocked);
                c.can_vote = false;
                c.can_reply = false;
                c.children.clear();
                c
            })
            .collect();
    }

    let l1_next_offset = if has_more_l1 {
        Some(l1_offset + comments.len() as u32)
    } else {
        None
    };

    Ok(Some(PostThreadResponse {
        root_post_id,
        stars,
        feel,
        comment_count: comment_count.max(0) as u64,
        viewer_vote: viewer_vote.map(|d| d.as_str().to_string()),
        can_view_discussion: can_view,
        preview_only,
        mirrors,
        comments,
        has_more_l1,
        l1_offset,
        l1_next_offset,
    }))
}

/// Flat thread row for council dossiers (no pagination; moderation mirrors excluded).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CouncilThreadNodeRow {
    pub id: u64,
    pub parent_post_id: Option<u64>,
    pub entity_id: u64,
    pub handle: String,
    pub body_plain: String,
    pub body_markup: String,
    pub post_rating: Decimal,
    pub published_at: DateTime<Utc>,
}

/// Full non-mirror thread under a root for council reply briefing.
#[derive(Debug, Clone)]
pub struct CouncilThreadFull {
    pub root_post_id: u64,
    pub nodes: Vec<CouncilThreadNodeRow>,
}

/// Load every non-mirror post in a thread (root + all replies within product depth caps).
pub async fn load_thread_full_for_council(
    pool: &MySqlPool,
    root_post_id: u64,
) -> Result<Option<CouncilThreadFull>, sqlx::Error> {
    let root_exists: Option<u64> = sqlx::query_scalar(
        r#"SELECT id FROM post WHERE id = ? AND parent_post_id IS NULL LIMIT 1"#,
    )
    .bind(root_post_id)
    .fetch_optional(pool)
    .await?;
    if root_exists.is_none() {
        return Ok(None);
    }

    let nodes: Vec<CouncilThreadNodeRow> = sqlx::query_as(
        r#"
        SELECT
            p.id,
            p.parent_post_id,
            p.entity_id,
            se.handle,
            p.body_plain,
            p.body_markup,
            COALESCE(p.rating, 0.00) AS post_rating,
            COALESCE(p.published_at, p.created_at) AS published_at
        FROM post p
        INNER JOIN social_entity se ON se.id = p.entity_id
        WHERE (p.id = ? OR p.root_post_id = ?)
          AND p.is_moderation_mirror = 0
        ORDER BY (p.id = ?) DESC, p.created_at ASC, p.id ASC
        "#,
    )
    .bind(root_post_id)
    .bind(root_post_id)
    .bind(root_post_id)
    .fetch_all(pool)
    .await?;

    Ok(Some(CouncilThreadFull {
        root_post_id,
        nodes,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_to_shown_for_depth_two_and_three() {
        let row = ThreadPostRow {
            id: 99,
            parent_post_id: Some(50),
            entity_id: 1,
            handle: "child".into(),
            display_name: None,
            avatar_path: None,
            body_html_sanitized: String::new(),
            body_markup: String::new(),
            body_plain: "hi".into(),
            is_moderation_mirror: 0,
            source_judicial_case_comment_id: None,
            created_at: Utc::now(),
            immediate_parent_id: Some(50),
            immediate_parent_handle: Some("parent".into()),
            immediate_parent_entity_id: Some(5),
            entity_rating: Decimal::new(250, 2),
            entity_status_key: None,
            post_rating: Decimal::ZERO,
            created_by: Some(1),
            country_code: Some("US".into()),
        };
        let blocked = HashSet::new();
        let l2 = row_to_comment(&row, 2, 10, 99, &blocked);
        assert!(l2.reply_to.is_some());
        let l3 = row_to_comment(&row, 3, 10, 99, &blocked);
        assert!(l3.reply_to.is_some());
    }

    #[test]
    fn reply_to_hidden_when_parent_author_blocked() {
        let row = ThreadPostRow {
            id: 99,
            parent_post_id: Some(50),
            entity_id: 1,
            handle: "child".into(),
            display_name: None,
            avatar_path: None,
            body_html_sanitized: String::new(),
            body_markup: String::new(),
            body_plain: "hi".into(),
            is_moderation_mirror: 0,
            source_judicial_case_comment_id: None,
            created_at: Utc::now(),
            immediate_parent_id: Some(50),
            immediate_parent_handle: Some("blocked_parent".into()),
            immediate_parent_entity_id: Some(99),
            entity_rating: Decimal::new(250, 2),
            entity_status_key: None,
            post_rating: Decimal::ZERO,
            created_by: Some(1),
            country_code: None,
        };
        let mut blocked = HashSet::new();
        blocked.insert(99);
        let l2 = row_to_comment(&row, 2, 10, 99, &blocked);
        assert!(l2.reply_to.is_none());
    }

    #[test]
    fn blocked_author_comment_is_hidden_stub() {
        let row = ThreadPostRow {
            id: 42,
            parent_post_id: Some(10),
            entity_id: 7,
            handle: "blocked_user".into(),
            display_name: Some("Secret Name".into()),
            avatar_path: Some("/avatars/x.jpg".into()),
            body_html_sanitized: "<p>secret</p>".into(),
            body_markup: String::new(),
            body_plain: "secret".into(),
            is_moderation_mirror: 0,
            source_judicial_case_comment_id: None,
            created_at: Utc::now(),
            immediate_parent_id: None,
            immediate_parent_handle: None,
            immediate_parent_entity_id: None,
            entity_rating: Decimal::new(250, 2),
            entity_status_key: None,
            post_rating: Decimal::ZERO,
            created_by: Some(2),
            country_code: None,
        };
        let mut blocked = HashSet::new();
        blocked.insert(7);
        let comment = row_to_comment(&row, 1, 10, 99, &blocked);
        assert!(comment.is_hidden);
        assert!(comment.handle.is_empty());
        assert!(comment.body_html.is_empty());
        assert!(comment.avatar_url.is_none());
        assert!(!comment.profile_linkable);
    }

    #[test]
    fn thread_profile_href_for_healthy_author() {
        let row = ThreadPostRow {
            id: 1,
            parent_post_id: Some(10),
            entity_id: 2,
            handle: "test_0001".into(),
            display_name: None,
            avatar_path: None,
            body_html_sanitized: String::new(),
            body_markup: String::new(),
            body_plain: String::new(),
            is_moderation_mirror: 0,
            source_judicial_case_comment_id: None,
            created_at: Utc::now(),
            immediate_parent_id: None,
            immediate_parent_handle: None,
            immediate_parent_entity_id: None,
            entity_rating: Decimal::new(250, 2),
            entity_status_key: None,
            post_rating: Decimal::ZERO,
            created_by: Some(3),
            country_code: Some("AU".into()),
        };
        let blocked = HashSet::new();
        let comment = row_to_comment(&row, 1, 10, 99, &blocked);
        assert!(comment.profile_linkable);
        assert_eq!(
            comment.profile_href.as_deref(),
            Some("/home/profile/test_0001")
        );
    }

    #[test]
    fn author_comment_cannot_be_voted_by_viewer() {
        let row = ThreadPostRow {
            id: 5,
            parent_post_id: Some(10),
            entity_id: 2,
            handle: "self".into(),
            display_name: None,
            avatar_path: None,
            body_html_sanitized: String::new(),
            body_markup: String::new(),
            body_plain: String::new(),
            is_moderation_mirror: 0,
            source_judicial_case_comment_id: None,
            created_at: Utc::now(),
            immediate_parent_id: None,
            immediate_parent_handle: None,
            immediate_parent_entity_id: None,
            entity_rating: Decimal::new(250, 2),
            entity_status_key: None,
            post_rating: Decimal::ZERO,
            created_by: Some(42),
            country_code: None,
        };
        let comment = row_to_comment(&row, 1, 10, 42, &HashSet::new());
        assert!(!comment.can_vote);
    }

    #[test]
    fn can_add_reply_respects_depth_caps() {
        assert!(engagement::can_add_reply_at_depth(1, 15));
        assert!(!engagement::can_add_reply_at_depth(1, 16));
        assert!(engagement::can_add_reply_at_depth(2, 3));
        assert!(!engagement::can_add_reply_at_depth(2, 4));
        assert!(!engagement::can_add_reply_at_depth(3, 0));
    }
}
