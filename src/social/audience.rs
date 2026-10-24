//! Audience handle parsing and entity expansion for mentions and compose.

use std::collections::HashSet;

use sqlx::{MySqlConnection, MySqlPool};

use crate::ref_keys;
use crate::social::blocks;

pub const SYSTEM_AUDIENCE_HANDLES: &[&str] =
    &["friends", "family", "colleague", "private", "public", "followers"];

/// Max entities expanded for @mention autocomplete (avoids huge `IN` lists).
pub const MENTION_EXPANSION_CAP: usize = 512;

/// Root post author + audience tokens for comment/reply @mention scoping.
#[derive(Debug, Clone)]
pub struct RootPostMentionContext {
    pub poster_entity_id: u64,
    pub audience_handles: Vec<String>,
}

/// Load root post author entity and audience tokens from persisted post scope.
pub async fn root_post_mention_context_conn(
    conn: &mut MySqlConnection,
    root_post_id: u64,
) -> Result<Option<RootPostMentionContext>, sqlx::Error> {
    let Some(poster_entity_id): Option<u64> = sqlx::query_scalar(
        "SELECT entity_id FROM post WHERE id = ? LIMIT 1",
    )
    .bind(root_post_id)
    .fetch_optional(&mut *conn)
    .await?
    else {
        return Ok(None);
    };
    let audience_handles: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT scope_key FROM post_audience_scope WHERE post_id = ?
        UNION ALL
        SELECT se.handle FROM post_audience_entity pae
        INNER JOIN social_entity se ON se.id = pae.audience_entity_id
        WHERE pae.post_id = ?
        "#,
    )
    .bind(root_post_id)
    .bind(root_post_id)
    .fetch_all(&mut *conn)
    .await?;
    Ok(Some(RootPostMentionContext {
        poster_entity_id,
        audience_handles,
    }))
}

pub async fn root_post_mention_context(
    pool: &MySqlPool,
    root_post_id: u64,
) -> Result<Option<RootPostMentionContext>, sqlx::Error> {
    let mut conn = pool.acquire().await?;
    root_post_mention_context_conn(&mut *conn, root_post_id).await
}

/// Parse `For: @friends, @foo` into normalized scope/entity handle tokens.
#[must_use]
pub fn parse_audience_handles(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut s = raw.trim().to_ascii_lowercase();
    if let Some(rest) = s.strip_prefix("for:") {
        s = rest.trim().to_string();
    }
    s = s.replace(',', " ");
    for token in s.split_whitespace() {
        if !token.starts_with('@') || token.len() < 2 {
            continue;
        }
        let handle = token.trim_start_matches('@');
        if handle.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
            let normalized = if handle == "base" {
                "followers"
            } else if handle == "colleagues" {
                "colleague"
            } else {
                handle
            };
            let h = normalized.to_string();
            if seen.insert(h.clone()) {
                out.push(h);
            }
        }
    }
    out
}

pub async fn expand_audience_entity_ids(
    pool: &MySqlPool,
    poster_entity_id: u64,
    user_id: u64,
    audience_handles: &[String],
) -> Result<Vec<u64>, sqlx::Error> {
    let mut conn = pool.acquire().await?;
    expand_audience_entity_ids_conn(&mut conn, poster_entity_id, user_id, audience_handles).await
}

pub async fn expand_audience_entity_ids_conn(
    conn: &mut MySqlConnection,
    poster_entity_id: u64,
    user_id: u64,
    audience_handles: &[String],
) -> Result<Vec<u64>, sqlx::Error> {
    let mut ids: Vec<u64> = Vec::new();
    let mut scopes: Vec<String> = Vec::new();
    for raw in audience_handles {
        let h = raw.trim().trim_start_matches('@').to_ascii_lowercase();
        if h.is_empty() {
            continue;
        }
        if SYSTEM_AUDIENCE_HANDLES.contains(&h.as_str())
            || h == "colleagues"
            || h == "base"
        {
            scopes.push(match h.as_str() {
                "colleagues" => "colleague".to_string(),
                "base" => "followers".to_string(),
                other => other.to_string(),
            });
        } else if let Some(id) = sqlx::query_scalar::<_, u64>(
            "SELECT id FROM social_entity WHERE handle = ? LIMIT 1",
        )
        .bind(&h)
        .fetch_optional(&mut *conn)
        .await?
        {
            ids.push(id);
        }
    }
    if !ids.contains(&poster_entity_id) {
        ids.push(poster_entity_id);
    }

    if scopes.iter().any(|s| s == "private") {
        let own: Vec<u64> = sqlx::query_scalar(
            r#"
            SELECT se.id
            FROM social_entity se
            LEFT JOIN user_social_entity_role ur
              ON ur.social_entity_id = se.id AND ur.user_id = ? AND ur.ends_at IS NULL
            WHERE se.created_by = ? OR ur.id IS NOT NULL
            "#,
        )
        .bind(user_id)
        .bind(user_id)
        .fetch_all(&mut *conn)
        .await?;
        ids.extend(own);
    }

    for scope in &scopes {
        match scope.as_str() {
            "friends" => {
                ids.extend(
                    relation_peers_conn(conn, poster_entity_id, ref_keys::KEY_RELATIONSHIP_FRIEND)
                        .await?,
                );
            }
            "family" => {
                ids.extend(
                    relation_peers_conn(conn, poster_entity_id, ref_keys::KEY_RELATIONSHIP_FAMILY)
                        .await?,
                );
            }
            "colleague" => {
                ids.extend(
                    relation_peers_conn(
                        conn,
                        poster_entity_id,
                        ref_keys::KEY_RELATIONSHIP_COLLEAGUE,
                    )
                    .await?,
                );
            }
            "followers" => {
                ids.extend(follower_peers_conn(conn, poster_entity_id).await?);
            }
            "public" => {}
            _ => {}
        }
    }

    let community_ids: Vec<u64> = ids.clone();
    for community_id in community_ids {
        let is_community: Option<u64> = sqlx::query_scalar(
            "SELECT social_entity_id FROM social_community WHERE social_entity_id = ? LIMIT 1",
        )
        .bind(community_id)
        .fetch_optional(&mut *conn)
        .await?;
        if is_community.is_some() {
            let members: Vec<u64> = sqlx::query_scalar(
                r#"
                SELECT ser.from_social_entity_id
                FROM social_entity_relationship ser
                INNER JOIN ref_key_values rk_type ON rk_type.id = ser.type_id
                   AND rk_type.`group` = ? AND rk_type.`key` = ? AND rk_type.is_active = 1
                INNER JOIN ref_key_values rk_st ON rk_st.id = ser.status_id
                   AND rk_st.`group` = ? AND rk_st.`key` = ? AND rk_st.is_active = 1
                WHERE ser.ended_at IS NULL
                  AND ser.to_social_entity_id = ?
                LIMIT ?
                "#,
            )
            .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
            .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
            .bind(ref_keys::GROUP_STATUS)
            .bind(ref_keys::KEY_VERIFIED)
            .bind(community_id)
            .bind(MENTION_EXPANSION_CAP as i64)
            .fetch_all(&mut *conn)
            .await?;
            ids.extend(members);
        }
    }

    ids.sort_unstable();
    ids.dedup();
    if ids.len() > MENTION_EXPANSION_CAP {
        ids.truncate(MENTION_EXPANSION_CAP);
    }
    Ok(ids)
}

async fn relation_peers_conn(
    conn: &mut MySqlConnection,
    entity_id: u64,
    rel_key: &str,
) -> Result<Vec<u64>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT DISTINCT peer_id FROM (
            SELECT ser.to_social_entity_id AS peer_id
            FROM social_entity_relationship ser
            INNER JOIN ref_key_values rk_type ON rk_type.id = ser.type_id
               AND rk_type.`group` = ? AND rk_type.`key` = ? AND rk_type.is_active = 1
            INNER JOIN ref_key_values rk_st ON rk_st.id = ser.status_id
               AND rk_st.`group` = ? AND rk_st.`key` = ? AND rk_st.is_active = 1
            WHERE ser.from_social_entity_id = ? AND ser.ended_at IS NULL
            UNION
            SELECT ser.from_social_entity_id AS peer_id
            FROM social_entity_relationship ser
            INNER JOIN ref_key_values rk_type ON rk_type.id = ser.type_id
               AND rk_type.`group` = ? AND rk_type.`key` = ? AND rk_type.is_active = 1
            INNER JOIN ref_key_values rk_st ON rk_st.id = ser.status_id
               AND rk_st.`group` = ? AND rk_st.`key` = ? AND rk_st.is_active = 1
            WHERE ser.to_social_entity_id = ? AND ser.ended_at IS NULL
        ) peers
        LIMIT ?
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(rel_key)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(entity_id)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(rel_key)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(entity_id)
    .bind(MENTION_EXPANSION_CAP as i64)
    .fetch_all(&mut *conn)
    .await
}

async fn follower_peers_conn(
    conn: &mut MySqlConnection,
    entity_id: u64,
) -> Result<Vec<u64>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT ser.from_social_entity_id
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk_type ON rk_type.id = ser.type_id
           AND rk_type.`group` = ? AND rk_type.`key` = ? AND rk_type.is_active = 1
        INNER JOIN ref_key_values rk_st ON rk_st.id = ser.status_id
           AND rk_st.`group` = ? AND rk_st.`key` = ? AND rk_st.is_active = 1
        WHERE ser.to_social_entity_id = ? AND ser.ended_at IS NULL
        LIMIT ?
        "#,
    )
    .bind(entity_id)
    .bind(MENTION_EXPANSION_CAP as i64)
    .fetch_all(&mut *conn)
    .await
}

#[derive(Debug, serde::Serialize)]
pub struct AudienceHandleCandidate {
    pub handle: String,
    pub label: String,
    /// `scope` (system audience token) or `entity` (connection).
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_key: Option<String>,
}

const AUDIENCE_SCOPE_SUGGESTIONS: &[(&str, &str, &[&str])] = &[
    ("friends", "Friends", &["friends"]),
    ("family", "Family", &["family"]),
    ("colleague", "Colleagues", &["colleague", "colleagues"]),
    ("followers", "Base audience", &["followers", "base"]),
    ("public", "Public", &["public"]),
    ("private", "Private", &["private"]),
];

/// Minimum searchable prefix length for handle typeahead (alphanumeric count).
#[must_use]
pub fn handle_prefix_searchable(prefix: &str) -> Option<String> {
    let p = prefix.trim().trim_start_matches('@').to_ascii_lowercase();
    if p.chars().filter(|c| c.is_ascii_alphanumeric()).count() < 3 {
        return None;
    }
    Some(p.chars().take(32).collect())
}

/// Audience-field typeahead: system scope handles + verified connections (≤256 peers).
pub async fn audience_handle_candidates(
    pool: &MySqlPool,
    user_id: u64,
    prefix: &str,
    limit: usize,
) -> Result<Vec<AudienceHandleCandidate>, sqlx::Error> {
    let Some(prefix) = handle_prefix_searchable(prefix) else {
        return Ok(Vec::new());
    };
    let lim = limit.clamp(1, 32) as i64;
    let mut out: Vec<AudienceHandleCandidate> = Vec::new();

    for (canonical, label, aliases) in AUDIENCE_SCOPE_SUGGESTIONS {
        if aliases.iter().any(|a| a.starts_with(prefix.as_str())) {
            let display_handle = match *canonical {
                "colleague" => "colleagues",
                "followers" => "base",
                other => other,
            };
            out.push(AudienceHandleCandidate {
                handle: display_handle.to_string(),
                label: (*label).to_string(),
                kind: "scope".to_string(),
                entity_id: None,
                type_key: Some("scope".to_string()),
            });
        }
    }

    let viewer_human_entity_id =
        crate::social::human_onboarding::primary_human_entity_id(pool, user_id).await?;
    let Some(viewer_human_entity_id) = viewer_human_entity_id else {
        out.truncate(lim as usize);
        return Ok(out);
    };

    let like = format!("{prefix}%");
    let block_sql = blocks::SQL_EXCLUDE_BLOCKED_PEERS;
    let rows = sqlx::query_as::<_, (u64, String, String, Option<String>)>(&format!(
        r#"
        SELECT
            se.id,
            se.handle,
            rk_et.`key` AS type_key,
            COALESCE(
                NULLIF(TRIM(sc_peer.label), ''),
                NULLIF(TRIM(sp_peer.label), ''),
                NULLIF(TRIM(snp_peer.label), ''),
                NULLIF(TRIM(sg_peer.label), ''),
                NULLIF(TRIM(CONCAT_WS(' ', sh_peer.given_name, sh_peer.family_name)), ''),
                se.handle
            ) AS label
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = ser.status_id
           AND rk_st.`group` = ?
           AND rk_st.is_active = 1
           AND rk_st.`key` IN (?, ?, ?)
        INNER JOIN social_entity my
            INNER JOIN social_human sh ON sh.social_entity_id = my.id
            ON my.id = ?
           AND (my.id = ser.from_social_entity_id OR my.id = ser.to_social_entity_id)
        INNER JOIN social_entity se ON se.id = CASE
            WHEN ser.from_social_entity_id = my.id THEN ser.to_social_entity_id
            ELSE ser.from_social_entity_id
        END
        LEFT JOIN social_human sh_peer ON sh_peer.social_entity_id = se.id
        LEFT JOIN social_community sc_peer ON sc_peer.social_entity_id = se.id
        LEFT JOIN social_profit sp_peer ON sp_peer.social_entity_id = se.id
        LEFT JOIN social_non_profit snp_peer ON snp_peer.social_entity_id = se.id
        LEFT JOIN social_gathering sg_peer ON sg_peer.social_entity_id = se.id
        INNER JOIN ref_key_values rk_et
            ON rk_et.id = se.type_id
           AND rk_et.`group` = ?
           AND rk_et.is_active = 1
        WHERE ser.ended_at IS NULL
          AND se.handle LIKE ?
          {block_sql}
        GROUP BY se.id, se.handle, rk_et.`key`, label
        ORDER BY se.handle ASC
        LIMIT ?
        "#
    ))
    .bind(crate::ref_keys::GROUP_STATUS)
    .bind(crate::ref_keys::KEY_PENDING_VERIFICATION)
    .bind(crate::ref_keys::KEY_VERIFIED)
    .bind(crate::ref_keys::KEY_FLAGGED)
    .bind(viewer_human_entity_id)
    .bind(crate::ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(like)
    .bind(lim)
    .bind(crate::ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(crate::ref_keys::KEY_RELATIONSHIP_FRIEND)
    .bind(crate::ref_keys::KEY_RELATIONSHIP_FAMILY)
    .bind(crate::ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
    .bind(crate::ref_keys::GROUP_STATUS)
    .bind(crate::ref_keys::KEY_BLOCKED)
    .bind(viewer_human_entity_id)
    .bind(viewer_human_entity_id)
    .fetch_all(pool)
    .await?;

    for (entity_id, handle, type_key, label) in rows {
        if out.iter().any(|c| c.handle == handle) {
            continue;
        }
        out.push(AudienceHandleCandidate {
            handle,
            label: label.unwrap_or_default(),
            kind: "entity".to_string(),
            entity_id: Some(entity_id),
            type_key: Some(type_key),
        });
    }

    out.truncate(lim as usize);
    Ok(out)
}
