//! Connection rail: active [`social_entity_relationship`] edges from the viewer's human profile to peers.

use sqlx::MySqlPool;
use rust_decimal::Decimal;

use crate::ref_keys;
use crate::social::public_profile;

/// Visual slots in the left rail **and** the service-layer Dunbar-scale cap on a human
/// `social_entity`'s **distinct connections**. Enforced for inserts in
/// [`crate::social::relationship::create`] and the creator-member edge in
/// [`crate::social::entity::create`]; only applies when the affected end is `social_entity_type.human`
/// (digital actors, communities, orgs, gatherings are not capped).
pub const MAX_CONNECTION_SLOTS: usize = 256;

/// True when `entity_id` is an active `social_entity` of type **human**. Cap enforcement skips
/// non-human ends.
pub async fn entity_is_human(
    pool: &MySqlPool,
    entity_id: u64,
) -> Result<bool, sqlx::Error> {
    let found: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN ref_key_values rk
            ON rk.id = se.type_id
           AND rk.`group` = ?
           AND rk.`key` = ?
           AND rk.is_active = 1
        WHERE se.id = ?
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .bind(entity_id)
    .fetch_optional(pool)
    .await?;
    Ok(found.is_some())
}

/// True when `peer_entity_id` is already counted as one of `human_entity_id`'s distinct
/// connections (same predicate as [`distinct_peer_count_for_human_entity`]: any relationship
/// type, status in pending/verified/flagged, `ended_at IS NULL`, peer entity active type).
///
/// Used by [`human_can_accept_new_peer`] so adding a *second* edge to an already-counted peer
/// (e.g. friend + colleague to the same person) does not consume a new slot.
pub async fn peer_already_in_connection_set(
    pool: &MySqlPool,
    human_entity_id: u64,
    peer_entity_id: u64,
) -> Result<bool, sqlx::Error> {
    let found: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT ser.id
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = ser.status_id
            AND rk_st.`group` = ?
            AND rk_st.is_active = 1
            AND rk_st.`key` IN (?, ?, ?)
        INNER JOIN social_entity peer
            ON peer.id = CASE
                WHEN ser.from_social_entity_id = ? THEN ser.to_social_entity_id
                ELSE ser.from_social_entity_id
            END
        INNER JOIN ref_key_values rk_et
            ON rk_et.id = peer.type_id
            AND rk_et.`group` = ?
            AND rk_et.is_active = 1
        WHERE ser.ended_at IS NULL
          AND (ser.from_social_entity_id = ? OR ser.to_social_entity_id = ?)
          AND peer.id = ?
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_PENDING_VERIFICATION)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(ref_keys::KEY_FLAGGED)
    .bind(human_entity_id)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(human_entity_id)
    .bind(human_entity_id)
    .bind(peer_entity_id)
    .fetch_optional(pool)
    .await?;
    Ok(found.is_some())
}

/// Returns `Ok(true)` when `human_entity_id` has capacity for a new edge to `peer_entity_id`:
/// either the peer is already in the connection set (no slot growth) or distinct peers are
/// strictly below [`MAX_CONNECTION_SLOTS`].
///
/// Used by [`crate::social::relationship::create`] and the creator-member auto-edge in
/// [`crate::social::entity::create`]. Returns `Ok(true)` immediately when the supplied entity
/// is not a human (cap only applies to meat-brain accounts per `AGENTS.md`).
pub async fn human_can_accept_new_peer(
    pool: &MySqlPool,
    human_entity_id: u64,
    peer_entity_id: u64,
) -> Result<bool, sqlx::Error> {
    if !entity_is_human(pool, human_entity_id).await? {
        return Ok(true);
    }
    if peer_already_in_connection_set(pool, human_entity_id, peer_entity_id).await? {
        return Ok(true);
    }
    let count = distinct_peer_count_for_human_entity(pool, human_entity_id).await?;
    Ok((count as usize) < MAX_CONNECTION_SLOTS)
}

/// Distinct peers connected to this **human** `social_entity.id` (same rules as [`list_for_user`]:
/// any relationship `type_id`, status `pending_verification` \| `verified` \| `flagged`, `ended_at` IS NULL;
/// peer must have an active `social_entity_type` ref row).
pub async fn distinct_peer_count_for_human_entity(
    pool: &MySqlPool,
    human_entity_id: u64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT peer.id)
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = ser.status_id
            AND rk_st.`group` = ?
            AND rk_st.is_active = 1
            AND rk_st.`key` IN (?, ?, ?)
        INNER JOIN social_entity my
            ON my.id = ?
            AND (my.id = ser.from_social_entity_id OR my.id = ser.to_social_entity_id)
        INNER JOIN social_entity peer ON peer.id = CASE
            WHEN ser.from_social_entity_id = my.id THEN ser.to_social_entity_id
            ELSE ser.from_social_entity_id
        END
        INNER JOIN ref_key_values rk_et
            ON rk_et.id = peer.type_id
            AND rk_et.`group` = ?
            AND rk_et.is_active = 1
        WHERE ser.ended_at IS NULL
        "#,
    )
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_PENDING_VERIFICATION)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(ref_keys::KEY_FLAGGED)
    .bind(human_entity_id)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .fetch_one(pool)
    .await
}

#[derive(Debug, Clone)]
pub struct ConnectionItem {
    pub peer_handle: String,
    /// Relative or site-root path to 512×512 artwork when present (same resolution as public profile).
    pub avatar_path: Option<String>,
    /// `ref_key_values.key` for `peer.type_id` (`social_entity_type` group).
    pub peer_type_key: String,
    pub peer_rating: f32,
    pub is_pending: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct Row {
    peer_handle: String,
    avatar_path: Option<String>,
    peer_type_key: String,
    peer_rating: Decimal,
    pending_edge_count: i64,
    verified_edge_count: i64,
}

/// Active relationships where the user's human `social_entity` is on either end, relationship `status`
/// is `pending_verification`, `verified`, or `flagged`, and `ended_at` is null.
///
/// One row per peer (earliest edge wins ordering). At most [`MAX_CONNECTION_SLOTS`] rows.
///
/// Pending badge ([`ConnectionItem::is_pending`]): show only when there is at least one
/// `pending_verification` edge **and** no `verified` edge to that peer. Otherwise stale or duplicate
/// rows (e.g. creator `member_of` left pending after another edge was verified) would incorrectly
/// show “pending” for an effectively verified connection.
pub async fn list_for_user(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<Vec<ConnectionItem>, sqlx::Error> {
    let avatar = public_profile::sql_avatar_path_expr("peer");
    let sql = format!(
        r#"
        SELECT
            peer.handle AS peer_handle,
            MAX(
                COALESCE(
                    NULLIF(TRIM(cl_sp_logo.url_canonical), ''),
                    NULLIF(TRIM(cl_snp_logo.url_canonical), ''),
                    ({avatar})
                )
            ) AS avatar_path,
            MAX(rk_et.`key`) AS peer_type_key,
            MAX(COALESCE(peer.rating, 0.00)) AS peer_rating,
            CAST(SUM(CASE WHEN rk_st.`key` = ? THEN 1 ELSE 0 END) AS SIGNED) AS pending_edge_count,
            CAST(SUM(CASE WHEN rk_st.`key` = ? THEN 1 ELSE 0 END) AS SIGNED) AS verified_edge_count
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = ser.status_id
            AND rk_st.`group` = ?
            AND rk_st.is_active = 1
            AND rk_st.`key` IN (?, ?, ?)
        INNER JOIN social_entity my
            INNER JOIN social_human sh ON sh.social_entity_id = my.id
            ON my.created_by = ?
            AND (my.id = ser.from_social_entity_id OR my.id = ser.to_social_entity_id)
        INNER JOIN social_entity peer ON peer.id = CASE
            WHEN ser.from_social_entity_id = my.id THEN ser.to_social_entity_id
            ELSE ser.from_social_entity_id
        END
        LEFT JOIN social_profit sp_peer ON sp_peer.social_entity_id = peer.id
        LEFT JOIN social_non_profit snp_peer ON snp_peer.social_entity_id = peer.id
        LEFT JOIN content_link cl_sp_logo ON cl_sp_logo.id = sp_peer.logo_content_link_id
        LEFT JOIN content_link cl_snp_logo ON cl_snp_logo.id = snp_peer.logo_content_link_id
        INNER JOIN ref_key_values rk_et
            ON rk_et.id = peer.type_id
            AND rk_et.`group` = ?
            AND rk_et.is_active = 1
        WHERE ser.ended_at IS NULL
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
                    (ser_blk.from_social_entity_id = my.id AND ser_blk.to_social_entity_id = peer.id)
                    OR (ser_blk.from_social_entity_id = peer.id AND ser_blk.to_social_entity_id = my.id)
                )
          )
        GROUP BY peer.id, peer.handle
        ORDER BY MIN(ser.created_at) ASC
        LIMIT {limit}
        "#,
        avatar = avatar,
        limit = MAX_CONNECTION_SLOTS
    );

    let rows = sqlx::query_as::<_, Row>(&sql)
        .bind(ref_keys::KEY_PENDING_VERIFICATION)
        .bind(ref_keys::KEY_VERIFIED)
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_PENDING_VERIFICATION)
        .bind(ref_keys::KEY_VERIFIED)
        .bind(ref_keys::KEY_FLAGGED)
        .bind(user_id)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
        .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
        .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
        .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_BLOCKED)
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|r| ConnectionItem {
            peer_handle: r.peer_handle,
            avatar_path: r.avatar_path,
            peer_type_key: r.peer_type_key,
            peer_rating: r.peer_rating.to_string().parse::<f32>().unwrap_or(0.0),
            is_pending: r.pending_edge_count > 0 && r.verified_edge_count == 0,
        })
        .collect())
}
