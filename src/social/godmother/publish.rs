//! Private @private godmother post publish path.

use sqlx::{MySql, Transaction};

use crate::content::sanitize::sanitize_post_markup;
use crate::social::conversation_rating;

fn validated_body(markup: &str) -> Result<(String, String, String), String> {
    let raw = markup.trim();
    if raw.is_empty() {
        return Err("post body is empty".to_string());
    }
    let sanitized = sanitize_post_markup(raw).trim().to_string();
    if sanitized.is_empty() {
        return Err("post body is empty".to_string());
    }
    let plain = strip_tags_plain(&sanitized);
    if plain.trim().is_empty() {
        return Err("post body is empty".to_string());
    }
    Ok((raw.to_string(), sanitized, plain))
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

/// Godmother outreach: `@private` root post visible to the human audience only.
pub async fn publish_private_post_tx(
    tx: &mut Transaction<'_, MySql>,
    godmother_entity_id: u64,
    human_entity_id: u64,
    system_user_id: u64,
    body_markup: &str,
) -> Result<u64, String> {
    let (raw, sanitized, plain) = validated_body(body_markup)?;

    let post_res = sqlx::query(
        r#"
        INSERT INTO post (
            entity_id, content_link_id, parent_post_id, root_post_id, status_id, rating,
            body_plain, body_markup, body_html_sanitized, published_at,
            created_by, modified_by, domain_id, country_code, is_moderation_mirror
        )
        VALUES (?, NULL, NULL, NULL, NULL, 0.00, ?, ?, ?, UTC_TIMESTAMP(6), ?, ?, NULL, NULL, 0)
        "#,
    )
    .bind(godmother_entity_id)
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

    sqlx::query(
        r#"INSERT INTO post_audience_entity (post_id, audience_entity_id, created_by) VALUES (?, ?, ?)"#,
    )
    .bind(post_id)
    .bind(human_entity_id)
    .bind(system_user_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query(
        r#"INSERT INTO post_audience_scope (post_id, scope_key, created_by) VALUES (?, 'private', ?)"#,
    )
    .bind(post_id)
    .bind(system_user_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;

    Ok(post_id)
}
