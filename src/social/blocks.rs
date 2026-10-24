//! Human block list — bi-directional mute via `status.blocked` on a human connection edge.
//!
//! Block edges always store **blockee → blocker**: `from_social_entity_id` is the person
//! blocked; `to_social_entity_id` is the person who blocked (including invite and profile flows).

use std::collections::HashSet;

use sqlx::{MySql, MySqlPool, Transaction};

use crate::ref_key_values;
use crate::ref_keys;
use crate::social::scope;

const HUMAN_CONNECTION_TYPES: &[&str] = &[
    ref_keys::KEY_RELATIONSHIP_FRIEND,
    ref_keys::KEY_RELATIONSHIP_FAMILY,
    ref_keys::KEY_RELATIONSHIP_COLLEAGUE,
];

#[derive(Debug, Clone)]
pub struct BlockedPeer {
    pub entity_id: u64,
    pub handle: String,
    pub display_name: Option<String>,
}

/// SQL fragment excluding root posts whose author is blocked relative to the viewer human entity.
/// Binds (after ref-key groups): `viewer_human_entity_id` twice.
pub const SQL_FEED_EXCLUDE_BLOCKED_AUTHORS: &str = r#"
          AND NOT EXISTS (
              SELECT 1
              FROM social_entity_relationship ser_blk
              INNER JOIN ref_key_values rk_blk_type
                  ON rk_blk_type.id = ser_blk.type_id
                 AND rk_blk_type.`group` = ?
                 AND rk_blk_type.`key` IN (?, ?, ?)
              INNER JOIN ref_key_values rk_blk_st
                  ON rk_blk_st.id = ser_blk.status_id
                 AND rk_blk_st.`group` = ?
                 AND rk_blk_st.is_active = 1
                 AND rk_blk_st.`key` = ?
              WHERE ser_blk.ended_at IS NULL
                AND (
                    (ser_blk.from_social_entity_id = ? AND ser_blk.to_social_entity_id = p.entity_id)
                    OR (ser_blk.to_social_entity_id = ? AND ser_blk.from_social_entity_id = p.entity_id)
                )
          )
"#;

/// SQL fragment excluding entity rows blocked relative to the viewer human entity (`se.id` peer).
/// Binds (after ref-key groups): `viewer_human_entity_id` twice.
pub const SQL_EXCLUDE_BLOCKED_PEERS: &str = r#"
          AND NOT EXISTS (
              SELECT 1
              FROM social_entity_relationship ser_blk
              INNER JOIN ref_key_values rk_blk_type
                  ON rk_blk_type.id = ser_blk.type_id
                 AND rk_blk_type.`group` = ?
                 AND rk_blk_type.`key` IN (?, ?, ?)
              INNER JOIN ref_key_values rk_blk_st
                  ON rk_blk_st.id = ser_blk.status_id
                 AND rk_blk_st.`group` = ?
                 AND rk_blk_st.is_active = 1
                 AND rk_blk_st.`key` = ?
              WHERE ser_blk.ended_at IS NULL
                AND (
                    (ser_blk.from_social_entity_id = ? AND ser_blk.to_social_entity_id = se.id)
                    OR (ser_blk.to_social_entity_id = ? AND ser_blk.from_social_entity_id = se.id)
                )
          )
"#;

/// Standard bind order for [`SQL_EXCLUDE_BLOCKED_PEERS`] / [`SQL_FEED_EXCLUDE_BLOCKED_AUTHORS`].
pub fn bind_block_exclusion<'q>(
    q: sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>,
    viewer_human_entity_id: u64,
) -> sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments> {
    q.bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
        .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
        .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
        .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_BLOCKED)
        .bind(viewer_human_entity_id)
        .bind(viewer_human_entity_id)
}

/// Drop blocked humans from an allow-list (`entity_id` values).
pub fn retain_unblocked_peers(allowed: &mut Vec<u64>, blocked: &HashSet<u64>) {
    allowed.retain(|id| !blocked.contains(id));
}

/// Whether any active blocked human-connection edge exists between the two entities (either direction).
pub async fn is_pair_blocked(
    pool: &MySqlPool,
    entity_a: u64,
    entity_b: u64,
) -> Result<bool, sqlx::Error> {
    let blocked: i64 = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM social_entity_relationship ser
            INNER JOIN ref_key_values rk_type
                ON rk_type.id = ser.type_id
               AND rk_type.`group` = ?
               AND rk_type.`key` IN (?, ?, ?)
            INNER JOIN ref_key_values rk_st
                ON rk_st.id = ser.status_id
               AND rk_st.`group` = ?
               AND rk_st.is_active = 1
               AND rk_st.`key` = ?
            WHERE ser.ended_at IS NULL
              AND (
                  (ser.from_social_entity_id = ? AND ser.to_social_entity_id = ?)
                  OR (ser.from_social_entity_id = ? AND ser.to_social_entity_id = ?)
              )
        )
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
    .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
    .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_BLOCKED)
    .bind(entity_a)
    .bind(entity_b)
    .bind(entity_b)
    .bind(entity_a)
    .fetch_one(pool)
    .await?;
    Ok(blocked != 0)
}

/// Humans the viewer entity has blocked (`to_social_entity_id` = viewer, peer = `from`).
pub async fn list_blocked_by(
    pool: &MySqlPool,
    blocker_entity_id: u64,
) -> Result<Vec<BlockedPeer>, sqlx::Error> {
    sqlx::query_as::<_, (u64, String, Option<String>)>(
        r#"
        SELECT
            se.id,
            se.handle,
            NULLIF(TRIM(CONCAT_WS(' ', sh.given_name, sh.additional_name, sh.family_name)), '') AS display_name
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = ser.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` IN (?, ?, ?)
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = ser.status_id
           AND rk_st.`group` = ?
           AND rk_st.is_active = 1
           AND rk_st.`key` = ?
        INNER JOIN social_entity se ON se.id = ser.from_social_entity_id
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        WHERE ser.ended_at IS NULL
          AND ser.to_social_entity_id = ?
        ORDER BY se.handle ASC
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
    .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
    .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_BLOCKED)
    .bind(blocker_entity_id)
    .fetch_all(pool)
    .await
    .map(|rows| {
        rows.into_iter()
            .map(|(entity_id, handle, display_name)| BlockedPeer {
                entity_id,
                handle,
                display_name,
            })
            .collect()
    })
}

/// Human entity ids blocked relative to `user_id`'s primary human profile (either direction).
pub async fn blocked_peer_entity_ids_for_user(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<HashSet<u64>, sqlx::Error> {
    let Some(viewer_human_id) =
        crate::social::human_onboarding::primary_human_entity_id(pool, user_id).await?
    else {
        return Ok(HashSet::new());
    };
    blocked_peer_entity_ids(pool, viewer_human_id).await
}

pub async fn blocked_peer_entity_ids_for_user_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> Result<HashSet<u64>, sqlx::Error> {
    let viewer_human_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        LEFT JOIN user_social_entity_role ur
            ON ur.social_entity_id = se.id
           AND ur.user_id = ?
           AND ur.ends_at IS NULL
        WHERE ur.id IS NOT NULL OR se.created_by = ?
        ORDER BY
            CASE WHEN ur.is_primary = 1 THEN 0 ELSE 1 END,
            se.id ASC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(viewer_human_id) = viewer_human_id else {
        return Ok(HashSet::new());
    };
    let rows: Vec<u64> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT CASE
            WHEN ser.to_social_entity_id = ? THEN ser.from_social_entity_id
            ELSE ser.to_social_entity_id
        END AS peer_id
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = ser.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` IN (?, ?, ?)
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = ser.status_id
           AND rk_st.`group` = ?
           AND rk_st.is_active = 1
           AND rk_st.`key` = ?
        WHERE ser.ended_at IS NULL
          AND (
              ser.from_social_entity_id = ?
              OR ser.to_social_entity_id = ?
          )
        "#,
    )
    .bind(viewer_human_id)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
    .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
    .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_BLOCKED)
    .bind(viewer_human_id)
    .bind(viewer_human_id)
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows.into_iter().collect())
}

/// Human entity ids with any active blocked edge involving the viewer (either direction).
pub async fn blocked_peer_entity_ids(
    pool: &MySqlPool,
    viewer_human_entity_id: u64,
) -> Result<HashSet<u64>, sqlx::Error> {
    let rows: Vec<u64> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT CASE
            WHEN ser.to_social_entity_id = ? THEN ser.from_social_entity_id
            ELSE ser.to_social_entity_id
        END AS peer_id
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = ser.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` IN (?, ?, ?)
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = ser.status_id
           AND rk_st.`group` = ?
           AND rk_st.is_active = 1
           AND rk_st.`key` = ?
        WHERE ser.ended_at IS NULL
          AND (
              ser.from_social_entity_id = ?
              OR ser.to_social_entity_id = ?
          )
        "#,
    )
    .bind(viewer_human_entity_id)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
    .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
    .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_BLOCKED)
    .bind(viewer_human_entity_id)
    .bind(viewer_human_entity_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
}

pub async fn block_human(
    pool: &MySqlPool,
    blocker_entity_id: u64,
    target_entity_id: u64,
    actor_user_id: u64,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    block_human_tx(&mut tx, blocker_entity_id, target_entity_id, actor_user_id).await?;
    tx.commit().await
}

pub async fn unblock_human(
    pool: &MySqlPool,
    blocker_entity_id: u64,
    target_entity_id: u64,
    actor_user_id: u64,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    unblock_human_tx(&mut tx, blocker_entity_id, target_entity_id, actor_user_id).await?;
    tx.commit().await
}

pub async fn block_human_tx(
    tx: &mut Transaction<'_, MySql>,
    blocker_entity_id: u64,
    target_entity_id: u64,
    actor_user_id: u64,
) -> Result<(), sqlx::Error> {
    if has_active_canonical_blocked_edge_tx(tx, target_entity_id, blocker_entity_id).await? {
        return Ok(());
    }
    end_human_connection_edges_between_tx(tx, blocker_entity_id, target_entity_id, actor_user_id)
        .await?;
    end_blocked_edges_between_tx(tx, blocker_entity_id, target_entity_id, actor_user_id).await?;
    insert_blocked_edge_tx(tx, blocker_entity_id, target_entity_id, actor_user_id).await?;
    scope::mark_relationship_endpoints_dirty_in_tx(tx, blocker_entity_id, target_entity_id).await?;
    Ok(())
}

pub async fn unblock_human_tx(
    tx: &mut Transaction<'_, MySql>,
    blocker_entity_id: u64,
    target_entity_id: u64,
    actor_user_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE social_entity_relationship ser
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = ser.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` IN (?, ?, ?)
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = ser.status_id
           AND rk_st.`group` = ?
           AND rk_st.`key` = ?
        SET ser.ended_at = UTC_DATE(),
            ser.modified_by = ?
        WHERE ser.ended_at IS NULL
          AND (
              (ser.from_social_entity_id = ? AND ser.to_social_entity_id = ?)
              OR (ser.from_social_entity_id = ? AND ser.to_social_entity_id = ?)
          )
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
    .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
    .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_BLOCKED)
    .bind(actor_user_id)
    .bind(blocker_entity_id)
    .bind(target_entity_id)
    .bind(target_entity_id)
    .bind(blocker_entity_id)
    .execute(&mut **tx)
    .await?;
    scope::mark_relationship_endpoints_dirty_in_tx(tx, blocker_entity_id, target_entity_id).await?;
    Ok(())
}

async fn end_human_connection_edges_between_tx(
    tx: &mut Transaction<'_, MySql>,
    entity_a: u64,
    entity_b: u64,
    actor_user_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE social_entity_relationship ser
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = ser.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` IN (?, ?, ?)
        SET ser.ended_at = COALESCE(ser.ended_at, UTC_DATE()),
            ser.modified_by = ?
        WHERE ser.ended_at IS NULL
          AND (
              (ser.from_social_entity_id = ? AND ser.to_social_entity_id = ?)
              OR (ser.from_social_entity_id = ? AND ser.to_social_entity_id = ?)
          )
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
    .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
    .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
    .bind(actor_user_id)
    .bind(entity_a)
    .bind(entity_b)
    .bind(entity_b)
    .bind(entity_a)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn end_blocked_edges_between_tx(
    tx: &mut Transaction<'_, MySql>,
    entity_a: u64,
    entity_b: u64,
    actor_user_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE social_entity_relationship ser
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = ser.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` IN (?, ?, ?)
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = ser.status_id
           AND rk_st.`group` = ?
           AND rk_st.`key` = ?
        SET ser.ended_at = COALESCE(ser.ended_at, UTC_DATE()),
            ser.modified_by = ?
        WHERE ser.ended_at IS NULL
          AND (
              (ser.from_social_entity_id = ? AND ser.to_social_entity_id = ?)
              OR (ser.from_social_entity_id = ? AND ser.to_social_entity_id = ?)
          )
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
    .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
    .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_BLOCKED)
    .bind(actor_user_id)
    .bind(entity_a)
    .bind(entity_b)
    .bind(entity_b)
    .bind(entity_a)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn has_active_canonical_blocked_edge_tx(
    tx: &mut Transaction<'_, MySql>,
    blockee_entity_id: u64,
    blocker_entity_id: u64,
) -> Result<bool, sqlx::Error> {
    let found: i64 = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM social_entity_relationship ser
            INNER JOIN ref_key_values rk_type
                ON rk_type.id = ser.type_id
               AND rk_type.`group` = ?
               AND rk_type.`key` IN (?, ?, ?)
            INNER JOIN ref_key_values rk_st
                ON rk_st.id = ser.status_id
               AND rk_st.`group` = ?
               AND rk_st.`key` = ?
            WHERE ser.ended_at IS NULL
              AND ser.from_social_entity_id = ?
              AND ser.to_social_entity_id = ?
        )
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
    .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
    .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_BLOCKED)
    .bind(blockee_entity_id)
    .bind(blocker_entity_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(found != 0)
}

async fn insert_blocked_edge_tx(
    tx: &mut Transaction<'_, MySql>,
    blocker_entity_id: u64,
    target_entity_id: u64,
    actor_user_id: u64,
) -> Result<(), sqlx::Error> {
    let type_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE,
        ref_keys::KEY_RELATIONSHIP_FRIEND,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("ref_key_values missing friend relationship type".into())
    })?;
    let status_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_STATUS,
        ref_keys::KEY_BLOCKED,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("ref_key_values missing status.blocked".into())
    })?;

    sqlx::query(
        r#"
        INSERT INTO social_entity_relationship (
            from_social_entity_id, to_social_entity_id,
            type_id, status_id, relationship_status_id,
            created_by, modified_by
        )
        VALUES (
            ?, ?, ?, ?,
            (
                SELECT scoped.id
                FROM ref_key_values old_status
                INNER JOIN ref_key_values scoped
                    ON scoped.`group` = 'social_relationship_status'
                   AND scoped.`key` = old_status.`key`
                WHERE old_status.id = ?
                LIMIT 1
            ),
            ?, ?
        )
        "#,
    )
    .bind(target_entity_id)
    .bind(blocker_entity_id)
    .bind(type_id)
    .bind(status_id)
    .bind(status_id)
    .bind(actor_user_id)
    .bind(actor_user_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
