//! @mention parsing, audience-scoped typeahead, and unseen mention counts.

use sqlx::{MySql, MySqlPool, Transaction};

use crate::social::audience;
use crate::social::blocks;

#[derive(Debug, serde::Serialize)]
pub struct MentionCandidate {
    pub handle: String,
    pub label: String,
    pub entity_id: u64,
}

/// Extract distinct @handles from plain post text (lowercase, no @ prefix).
#[must_use]
pub fn parse_at_handles(plain: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = plain.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'@' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() {
                let c = bytes[j];
                if c.is_ascii_alphanumeric() || c == b'_' {
                    j += 1;
                } else {
                    break;
                }
            }
            if j > start {
                let handle = plain[start..j].to_ascii_lowercase();
                if !handle.is_empty() && !out.iter().any(|x| x == &handle) {
                    out.push(handle);
                }
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

pub async fn audience_mention_candidates(
    pool: &MySqlPool,
    poster_entity_id: u64,
    user_id: u64,
    audience_handles: &[String],
    prefix: &str,
    limit: usize,
    acting_entity_id: u64,
) -> Result<Vec<MentionCandidate>, sqlx::Error> {
    let mut allowed =
        audience::expand_audience_entity_ids(pool, poster_entity_id, user_id, audience_handles)
            .await?;
    let blocked = blocks::blocked_peer_entity_ids_for_user(pool, user_id).await?;
    blocks::retain_unblocked_peers(&mut allowed, &blocked);
    if allowed.is_empty() {
        return Ok(Vec::new());
    }
    let prefix = prefix.trim().trim_start_matches('@').to_ascii_lowercase();
    let Some(prefix) = audience::handle_prefix_searchable(prefix.as_str()) else {
        return Ok(Vec::new());
    };
    let like = format!("{prefix}%");
    let lim = limit.clamp(1, 32) as i64;
    let placeholders = allowed
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        r#"
        SELECT se.id, se.handle,
               COALESCE(
                   NULLIF(TRIM(sc.label), ''),
                   NULLIF(TRIM(sp.label), ''),
                   NULLIF(TRIM(snp.label), ''),
                   NULLIF(TRIM(sg.label), ''),
                   NULLIF(TRIM(CONCAT_WS(' ', sh.given_name, sh.family_name)), ''),
                   se.handle
               ) AS label
        FROM social_entity se
        LEFT JOIN social_human sh ON sh.social_entity_id = se.id
        LEFT JOIN social_community sc ON sc.social_entity_id = se.id
        LEFT JOIN social_profit sp ON sp.social_entity_id = se.id
        LEFT JOIN social_non_profit snp ON snp.social_entity_id = se.id
        LEFT JOIN social_gathering sg ON sg.social_entity_id = se.id
        WHERE se.id IN ({placeholders})
          AND se.handle LIKE ?
          AND se.id <> ?
        ORDER BY se.handle ASC
        LIMIT ?
        "#
    );
    let mut q = sqlx::query_as::<_, (u64, String, Option<String>)>(&sql);
    for id in &allowed {
        q = q.bind(id);
    }
    q = q.bind(like);
    q = q.bind(acting_entity_id);
    q = q.bind(lim);
    let rows = q.fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(|(entity_id, handle, label)| MentionCandidate {
            handle,
            label: label.unwrap_or_default(),
            entity_id,
        })
        .collect())
}

pub async fn persist_mentions_tx(
    tx: &mut Transaction<'_, MySql>,
    post_id: u64,
    poster_entity_id: u64,
    user_id: u64,
    audience_handles: &[String],
    plain_body: &str,
) -> Result<(), sqlx::Error> {
    let handles = parse_at_handles(plain_body);
    if handles.is_empty() {
        return Ok(());
    }
    let mut allowed =
        audience::expand_audience_entity_ids_conn(&mut **tx, poster_entity_id, user_id, audience_handles)
            .await?;
    let blocked = blocks::blocked_peer_entity_ids_for_user_tx(tx, user_id).await?;
    blocks::retain_unblocked_peers(&mut allowed, &blocked);
    for handle in handles {
        let Some(entity_id): Option<u64> = sqlx::query_scalar(
            "SELECT id FROM social_entity WHERE handle = ? LIMIT 1",
        )
        .bind(&handle)
        .fetch_optional(&mut **tx)
        .await?
        else {
            continue;
        };
        if !allowed.contains(&entity_id) || entity_id == poster_entity_id {
            continue;
        }
        sqlx::query(
            r#"
            INSERT INTO post_mention (post_id, mentioned_entity_id)
            VALUES (?, ?)
            ON DUPLICATE KEY UPDATE post_id = post_id
            "#,
        )
        .bind(post_id)
        .bind(entity_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub async fn unseen_mention_count(pool: &MySqlPool, user_id: u64) -> Result<u32, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM post_mention pm
        INNER JOIN social_human sh ON sh.social_entity_id = pm.mentioned_entity_id
        INNER JOIN social_entity se ON se.id = sh.social_entity_id
        WHERE se.created_by = ?
          AND pm.seen_at IS NULL
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(count.max(0) as u32)
}

pub async fn acknowledge_mentions(pool: &MySqlPool, user_id: u64) -> Result<u32, sqlx::Error> {
    let res = sqlx::query(
        r#"
        UPDATE post_mention pm
        INNER JOIN social_human sh ON sh.social_entity_id = pm.mentioned_entity_id
        INNER JOIN social_entity se ON se.id = sh.social_entity_id
        SET pm.seen_at = UTC_TIMESTAMP(6)
        WHERE se.created_by = ?
          AND pm.seen_at IS NULL
        "#,
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected().min(u32::MAX as u64) as u32)
}

#[cfg(test)]
mod tests {
    use super::parse_at_handles;

    #[test]
    fn parse_at_handles_extracts_distinct() {
        let h = parse_at_handles("Hi @Alice and @bob — @ALICE again");
        assert_eq!(h, vec!["alice".to_string(), "bob".to_string()]);
    }
}
