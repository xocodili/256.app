//! Community context resolved from a root post (author community or audience).

use sqlx::{MySqlConnection, MySqlPool, Transaction};

pub async fn community_context_for_root_post(
    pool: &MySqlPool,
    root_post_id: u64,
) -> Result<Option<u64>, sqlx::Error> {
    let mut conn = pool.acquire().await?;
    community_context_for_root_post_conn(&mut conn, root_post_id).await
}

pub async fn community_context_for_root_post_tx(
    tx: &mut Transaction<'_, sqlx::MySql>,
    root_post_id: u64,
) -> Result<Option<u64>, sqlx::Error> {
    community_context_for_root_post_conn(&mut **tx, root_post_id).await
}

pub async fn community_context_for_root_post_conn(
    conn: &mut MySqlConnection,
    root_post_id: u64,
) -> Result<Option<u64>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT community_id
        FROM (
            SELECT p.entity_id AS community_id, 0 AS ord
            FROM post p
            INNER JOIN social_community sc_post ON sc_post.social_entity_id = p.entity_id
            WHERE p.id = ?
            UNION ALL
            SELECT pae.audience_entity_id AS community_id, 1 AS ord
            FROM post_audience_entity pae
            INNER JOIN social_community sc_aud ON sc_aud.social_entity_id = pae.audience_entity_id
            WHERE pae.post_id = ?
        ) communities
        ORDER BY ord ASC, community_id ASC
        LIMIT 1
        "#,
    )
    .bind(root_post_id)
    .bind(root_post_id)
    .fetch_optional(conn)
    .await
}
