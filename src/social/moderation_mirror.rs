//! Mirror selected `judicial_case_comment` rows onto the subject post when a case resolves.

use sqlx::{MySql, Transaction};

use crate::ref_keys;
use crate::social::judicial::triggers;

/// Human-facing case comment types mirrored onto the post thread (newest-first with normal comments).
const MIRROR_COMMENT_TYPES: &[&str] = &[
    "originator",
    "evidence",
    "rebuttal",
    "verdict_justification",
    "closing_statement",
    "ai_brief",
];

/// Insert mirror `post` rows for a resolved post-moderation / appeal case (idempotent per source comment).
pub async fn mirror_case_comments_to_post_tx(
    tx: &mut Transaction<'_, MySql>,
    case_id: u64,
    system_actor: u64,
) -> Result<(), sqlx::Error> {
    let row: Option<(Option<u64>, String)> = sqlx::query_as(
        r#"
        SELECT jc.subject_post_id, rk_tr.`key`
        FROM judicial_case jc
        INNER JOIN ref_key_values rk_tr
            ON rk_tr.id = jc.trigger_type_id
           AND rk_tr.`group` = ?
           AND rk_tr.is_active = 1
        WHERE jc.id = ?
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(case_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some((Some(subject_post_id), trigger_key)) = row else {
        return Ok(());
    };
    if !triggers::trigger_is_post_dispute_like(&trigger_key) {
        return Ok(());
    }

    let root_id: u64 = sqlx::query_scalar(
        r#"SELECT COALESCE(p.root_post_id, p.id) FROM post p WHERE p.id = ? LIMIT 1"#,
    )
    .bind(subject_post_id)
    .fetch_one(&mut **tx)
    .await?;

    let comments: Vec<(
        u64,
        String,
        String,
        String,
        String,
        Option<u64>,
    )> = sqlx::query_as(
        r#"
        SELECT
            jcc.id,
            jcc.body_plain,
            jcc.body_markup,
            jcc.body_html_sanitized,
            rk_typ.`key`,
            jcc.author_social_entity_id
        FROM judicial_case_comment jcc
        INNER JOIN ref_key_values rk_typ
            ON rk_typ.id = jcc.type_id
           AND rk_typ.`group` = 'judicial_case_comment_type'
           AND rk_typ.is_active = 1
        WHERE jcc.judicial_case_id = ?
        ORDER BY jcc.created_at ASC, jcc.id ASC
        "#,
    )
    .bind(case_id)
    .fetch_all(&mut **tx)
    .await?;

    let fallback_entity: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        WHERE se.handle = ?
        LIMIT 1
        "#,
    )
    .bind("256")
    .fetch_optional(&mut **tx)
    .await?;

    for (jcc_id, plain, markup, html, type_key, author_entity) in comments {
        if !MIRROR_COMMENT_TYPES.contains(&type_key.as_str()) {
            continue;
        }
        let exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM post p WHERE p.source_judicial_case_comment_id = ?
            )
            "#,
        )
        .bind(jcc_id)
        .fetch_one(&mut **tx)
        .await?;
        if exists {
            continue;
        }

        let entity_id = author_entity.or(fallback_entity).unwrap_or(0);
        if entity_id == 0 {
            continue;
        }

        let _ = sqlx::query(
            r#"
            INSERT INTO post (
                entity_id, content_link_id, parent_post_id, root_post_id, status_id, rating,
                body_plain, body_markup, body_html_sanitized, published_at,
                created_by, modified_by, domain_id, country_code,
                is_moderation_mirror, source_judicial_case_comment_id
            )
            VALUES (?, NULL, ?, ?, NULL, 0.00, ?, ?, ?, UTC_TIMESTAMP(6), ?, ?, NULL, NULL, 1, ?)
            "#,
        )
        .bind(entity_id)
        .bind(root_id)
        .bind(root_id)
        .bind(plain.as_str())
        .bind(markup.as_str())
        .bind(html.as_str())
        .bind(system_actor)
        .bind(system_actor)
        .bind(jcc_id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

/// Idempotent AI-voiced verdict notice mirrored onto the post thread (exoneration paths).
pub async fn mirror_ai_verdict_notice_tx(
    tx: &mut Transaction<'_, MySql>,
    case_id: u64,
    notice_plain: &str,
    notice_html: &str,
) -> Result<(), sqlx::Error> {
    let system_actor: u64 = sqlx::query_scalar(
        r#"SELECT id FROM users WHERE email = ? LIMIT 1"#,
    )
    .bind(ref_keys::SYSTEM_USER_EMAIL)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or(0);

    let Some(root_id): Option<u64> = sqlx::query_scalar(
        r#"
        SELECT COALESCE(p.root_post_id, p.id)
        FROM judicial_case jc
        INNER JOIN post p ON p.id = jc.subject_post_id
        WHERE jc.id = ?
        LIMIT 1
        "#,
    )
    .bind(case_id)
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Ok(());
    };

    let marker = format!("[verdict_notice:case_id={case_id}]");
    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM post p
            WHERE COALESCE(p.root_post_id, p.id) = ?
              AND p.is_moderation_mirror = 1
              AND p.body_plain LIKE ?
        )
        "#,
    )
    .bind(root_id)
    .bind(format!("{marker}%"))
    .fetch_one(&mut **tx)
    .await?;
    if exists {
        return Ok(());
    }

    let fallback_entity: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        WHERE se.handle = ?
        LIMIT 1
        "#,
    )
    .bind("256")
    .fetch_optional(&mut **tx)
    .await?;

    let Some(entity_id) = fallback_entity else {
        return Ok(());
    };

    let plain = format!("{marker}{notice_plain}");
    let html = format!(
        "<p>{}</p>{}",
        escape_html(notice_plain),
        notice_html.trim()
    );

    sqlx::query(
        r#"
        INSERT INTO post (
            entity_id, content_link_id, parent_post_id, root_post_id, status_id, rating,
            body_plain, body_markup, body_html_sanitized, published_at,
            created_by, modified_by, domain_id, country_code,
            is_moderation_mirror, source_judicial_case_comment_id
        )
        VALUES (?, NULL, ?, ?, NULL, 0.00, ?, ?, ?, UTC_TIMESTAMP(6), ?, ?, NULL, NULL, 1, NULL)
        "#,
    )
    .bind(entity_id)
    .bind(root_id)
    .bind(root_id)
    .bind(plain.as_str())
    .bind(html.as_str())
    .bind(html.as_str())
    .bind(system_actor)
    .bind(system_actor)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn escape_html(s: &str) -> String {
    crate::html_escape::escape_html(s)
}
