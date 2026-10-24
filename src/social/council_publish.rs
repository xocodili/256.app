//! Internal publish path for council digital entities (bypasses HTTP session gates).

use sqlx::{MySql, Transaction};

use crate::content::sanitize::sanitize_post_markup;
use crate::ref_keys;
use crate::social::audience;
use crate::social::conversation_rating;
use crate::social::engagement;
use crate::social::mentions;

const ERR_EMPTY: &str = "post body is empty";
const ERR_MODERATED_THREAD: &str = "thread is under moderation";
const ERR_LINK_NOT_SHAREABLE: &str = "content link is not shareable";

fn validated_body(markup: &str) -> Result<(String, String, String), String> {
    let raw = markup.trim();
    if raw.is_empty() {
        return Err(ERR_EMPTY.to_string());
    }
    let sanitized = sanitize_post_markup(raw).trim().to_string();
    if sanitized.is_empty() {
        return Err(ERR_EMPTY.to_string());
    }
    let plain = strip_tags_plain(&sanitized);
    if plain.trim().is_empty() {
        return Err(ERR_EMPTY.to_string());
    }
    Ok((raw.to_string(), sanitized, plain))
}

async fn assert_thread_open_for_council_reply_tx(
    tx: &mut Transaction<'_, MySql>,
    root_post_id: u64,
) -> Result<(), String> {
    let blocked: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM post root
            INNER JOIN ref_key_values rk
                ON rk.id = root.status_id
               AND rk.`group` = ?
               AND rk.`key` IN (?, ?, ?)
            WHERE root.id = ?
        )
        "#,
    )
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_POST_SUPPRESSED)
    .bind(ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW)
    .bind(ref_keys::KEY_FLAGGED)
    .bind(root_post_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    if blocked {
        Err(ERR_MODERATED_THREAD.to_string())
    } else {
        Ok(())
    }
}

async fn assert_content_link_shareable_tx(
    tx: &mut Transaction<'_, MySql>,
    ai_actor_id: u64,
    content_link_id: u64,
) -> Result<u64, String> {
    let domain_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT cd.id
        FROM content_link cl
        INNER JOIN content_domain cd ON cd.id = cl.domain_id
        INNER JOIN ai_actor_content_domain pac
            ON pac.content_domain_id = cd.id
           AND pac.ai_actor_id = ?
           AND pac.is_active = 1
           AND pac.removed_at IS NULL
        INNER JOIN ref_key_values rk
            ON rk.id = cd.status_id
           AND rk.`group` = ?
           AND rk.`key` <> ?
           AND rk.is_active = 1
        WHERE cl.id = ?
          AND cl.rating >= 2.50
          AND cd.rating >= 2.50
          AND cd.safety_state = 'safe'
          AND cl.fetch_status = 'ok'
        LIMIT 1
        "#,
    )
    .bind(ai_actor_id)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_BLOCKED)
    .bind(content_link_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    domain_id.ok_or_else(|| ERR_LINK_NOT_SHAREABLE.to_string())
}

/// Publish a thread reply as a council digital entity.
pub async fn publish_reply_tx(
    tx: &mut Transaction<'_, MySql>,
    entity_id: u64,
    system_user_id: u64,
    parent_post_id: u64,
    body_markup: &str,
) -> Result<u64, String> {
    let (raw, sanitized, plain) = validated_body(body_markup)?;

    let (parent_id, root_id, thread_root): (u64, u64, u64) = sqlx::query_as(
        r#"
        SELECT p.id, COALESCE(p.root_post_id, p.id), COALESCE(r.root_post_id, r.id)
        FROM post p
        LEFT JOIN post r ON r.id = COALESCE(p.root_post_id, p.id)
        WHERE p.id = ?
        LIMIT 1
        "#,
    )
    .bind(parent_post_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "parent post not found".to_string())?;

    if thread_root != root_id {
        return Err("parent is not in thread".to_string());
    }

    assert_thread_open_for_council_reply_tx(tx, root_id).await?;

    engagement::assert_can_add_child_tx(tx, parent_id, root_id, false)
        .await
        .map_err(|e| format!("thread cap: {e:?}"))?;

    let post_res = sqlx::query(
        r#"
        INSERT INTO post (
            entity_id, content_link_id, parent_post_id, root_post_id, status_id, rating,
            body_plain, body_markup, body_html_sanitized, published_at,
            created_by, modified_by, domain_id, country_code, is_moderation_mirror
        )
        VALUES (?, NULL, ?, ?, NULL, 0.00, ?, ?, ?, UTC_TIMESTAMP(6), ?, ?, NULL, NULL, 0)
        "#,
    )
    .bind(entity_id)
    .bind(parent_id)
    .bind(root_id)
    .bind(plain.as_str())
    .bind(raw.as_str())
    .bind(sanitized.as_str())
    .bind(system_user_id)
    .bind(system_user_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    let post_id = post_res.last_insert_id();

    conversation_rating::init_conversation_tx(tx, post_id)
        .await
        .map_err(|e| e.to_string())?;
    conversation_rating::touch_conversation_activity_tx(tx, post_id)
        .await
        .map_err(|e| e.to_string())?;

    let mention_ctx = audience::root_post_mention_context_conn(&mut **tx, root_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "root mention context missing".to_string())?;
    mentions::persist_mentions_tx(
        tx,
        post_id,
        mention_ctx.poster_entity_id,
        system_user_id,
        &mention_ctx.audience_handles,
        plain.as_str(),
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(post_id)
}

/// Share a catalogue link as a root post to the stewarded community (@public).
pub async fn publish_community_share_tx(
    tx: &mut Transaction<'_, MySql>,
    entity_id: u64,
    community_entity_id: u64,
    ai_actor_id: u64,
    system_user_id: u64,
    content_link_id: u64,
    body_markup: &str,
) -> Result<u64, String> {
    let (raw, sanitized, plain) = validated_body(body_markup)?;
    let domain_id = assert_content_link_shareable_tx(tx, ai_actor_id, content_link_id).await?;

    let post_res = sqlx::query(
        r#"
        INSERT INTO post (
            entity_id, content_link_id, parent_post_id, root_post_id, status_id, rating,
            body_plain, body_markup, body_html_sanitized, published_at,
            created_by, modified_by, domain_id, country_code, is_moderation_mirror
        )
        VALUES (?, ?, NULL, NULL, NULL, 0.00, ?, ?, ?, UTC_TIMESTAMP(6), ?, ?, ?, NULL, 0)
        "#,
    )
    .bind(entity_id)
    .bind(content_link_id)
    .bind(plain.as_str())
    .bind(raw.as_str())
    .bind(sanitized.as_str())
    .bind(system_user_id)
    .bind(system_user_id)
    .bind(domain_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    let post_id = post_res.last_insert_id();

    conversation_rating::init_conversation_tx(tx, post_id)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query(r#"UPDATE content_link SET share_count = share_count + 1 WHERE id = ?"#)
        .bind(content_link_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query(
        r#"INSERT INTO post_audience_entity (post_id, audience_entity_id, created_by) VALUES (?, ?, ?)"#,
    )
    .bind(post_id)
    .bind(community_entity_id)
    .bind(system_user_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query(
        r#"INSERT INTO post_audience_scope (post_id, scope_key, created_by) VALUES (?, 'public', ?)"#,
    )
    .bind(post_id)
    .bind(system_user_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;

    crate::social::community_graph::touch_post_activity_for_entities_tx(
        tx,
        &[community_entity_id],
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(post_id)
}

/// Godmother welcome: `@private` root post visible to the new human only.
pub async fn publish_godmother_welcome_tx(
    tx: &mut Transaction<'_, MySql>,
    godmother_entity_id: u64,
    human_entity_id: u64,
    system_user_id: u64,
    body_markup: &str,
) -> Result<u64, String> {
    crate::social::godmother::publish_private_post_tx(
        tx,
        godmother_entity_id,
        human_entity_id,
        system_user_id,
        body_markup,
    )
    .await
}

fn strip_tags_plain(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
