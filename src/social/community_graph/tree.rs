//! Community subtree navigation and failing-state helpers.

use rust_decimal::Decimal;
use sqlx::MySqlPool;

use crate::social::judicial::failing_rating_threshold;

use crate::ref_keys;

use super::immune::is_death_immune;

pub async fn list_immediate_children(
    pool: &MySqlPool,
    community_id: u64,
) -> Result<Vec<u64>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT social_entity_id
        FROM social_community
        WHERE parent_community_id = ?
        "#,
    )
    .bind(community_id)
    .fetch_all(pool)
    .await
}

pub(crate) async fn list_immediate_children_conn(
    conn: &mut sqlx::MySqlConnection,
    community_id: u64,
) -> Result<Vec<u64>, sqlx::Error> {
    sqlx::query_scalar(
        r#"SELECT social_entity_id FROM social_community WHERE parent_community_id = ?"#,
    )
    .bind(community_id)
    .fetch_all(conn)
    .await
}

/// `root_id` plus all descendants via `parent_community_id`.
pub async fn collect_subtree_community_ids(
    pool: &MySqlPool,
    root_id: u64,
) -> Result<Vec<u64>, sqlx::Error> {
    collect_subtree_community_ids_conn(&mut *pool.acquire().await?, root_id).await
}

pub async fn collect_subtree_community_ids_conn(
    conn: &mut sqlx::MySqlConnection,
    root_id: u64,
) -> Result<Vec<u64>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        WITH RECURSIVE subtree AS (
            SELECT social_entity_id AS id
            FROM social_community
            WHERE social_entity_id = ?
            UNION ALL
            SELECT sc.social_entity_id
            FROM social_community sc
            INNER JOIN subtree s ON sc.parent_community_id = s.id
        )
        SELECT id FROM subtree
        "#,
    )
    .bind(root_id)
    .fetch_all(&mut *conn)
    .await
}

/// Reject if `new_parent_id` is in the descendant chain of `community_id` (would cycle).
pub async fn would_create_cycle(
    pool: &MySqlPool,
    community_id: u64,
    new_parent_id: u64,
) -> Result<bool, sqlx::Error> {
    if community_id == new_parent_id {
        return Ok(true);
    }
    let subtree = collect_subtree_community_ids(pool, community_id).await?;
    Ok(subtree.contains(&new_parent_id))
}

pub async fn community_rating(pool: &MySqlPool, community_id: u64) -> Result<Decimal, sqlx::Error> {
    let rating: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(se.rating, 0) FROM social_entity se WHERE se.id = ? LIMIT 1"#,
    )
    .bind(community_id)
    .fetch_one(pool)
    .await?;
    Ok(rating)
}

pub async fn is_community_failing(pool: &MySqlPool, community_id: u64) -> Result<bool, sqlx::Error> {
    if is_death_immune(pool, community_id).await? {
        return Ok(false);
    }
    let status_key: Option<String> = sqlx::query_scalar(
        r#"
        SELECT rk.`key`
        FROM social_entity se
        LEFT JOIN ref_key_values rk
            ON rk.id = se.entity_status_id
           AND rk.`group` = ?
        WHERE se.id = ?
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
    .bind(community_id)
    .fetch_optional(pool)
    .await?;
    if status_key.as_deref() == Some(ref_keys::KEY_ENTITY_STATUS_DEAD) {
        return Ok(false);
    }
    let rating = community_rating(pool, community_id).await?;
    Ok(rating < failing_rating_threshold())
}
