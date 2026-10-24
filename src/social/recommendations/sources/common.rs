//! Shared SQL for graph-walk recommendation sources.

use std::collections::HashSet;

use rust_decimal::Decimal;
use sqlx::MySqlPool;

use crate::ref_keys;
use crate::social::blocks;
use crate::social::public_profile;

use super::super::types::{EntityRecommendation, PER_TYPE_LIMIT};

pub const MAX_ANCHOR_COMMUNITIES: i64 = 32;
pub const MAX_MEMBERS_PER_COMMUNITY: i64 = 128;
pub const MAX_EXPANSION_COHORT: i64 = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgePool {
    DirectPeers,
    ExpansionCohort,
}

#[derive(Debug, sqlx::FromRow)]
struct CandidateRow {
    entity_id: u64,
    handle: String,
    label: String,
    type_key: String,
    avatar_path: Option<String>,
    bridge_count: i64,
    entity_rating: Decimal,
}

struct TypeQueryParts {
    entity_type_key: &'static str,
    bridge_type_keys: &'static [&'static str],
    label_expr: &'static str,
    join_extension: &'static str,
}

fn type_parts(type_key: &str) -> Option<TypeQueryParts> {
    Some(match type_key {
        ref_keys::KEY_ENTITY_TYPE_COMMUNITY => TypeQueryParts {
            entity_type_key: ref_keys::KEY_ENTITY_TYPE_COMMUNITY,
            bridge_type_keys: &[ref_keys::KEY_RELATIONSHIP_MEMBER_OF],
            label_expr: "COALESCE(sc.label, se.handle)",
            join_extension: r#"
        INNER JOIN social_community sc ON sc.social_entity_id = se.id
        INNER JOIN ref_key_values rk_jp
            ON rk_jp.id = sc.join_policy_id
           AND rk_jp.`group` = ?
           AND rk_jp.is_active = 1
           AND rk_jp.`key` <> ?
            "#,
        },
        ref_keys::KEY_ENTITY_TYPE_GATHERING => TypeQueryParts {
            entity_type_key: ref_keys::KEY_ENTITY_TYPE_GATHERING,
            bridge_type_keys: &[
                ref_keys::KEY_RELATIONSHIP_EVENT_GOING,
                ref_keys::KEY_RELATIONSHIP_EVENT_INTERESTED,
            ],
            label_expr: "COALESCE(sg.label, se.handle)",
            join_extension: r#"
        INNER JOIN social_gathering sg ON sg.social_entity_id = se.id
        INNER JOIN ref_key_values rk_jp
            ON rk_jp.id = sg.join_policy_id
           AND rk_jp.`group` = ?
           AND rk_jp.is_active = 1
           AND rk_jp.`key` <> ?
            "#,
        },
        ref_keys::KEY_ENTITY_TYPE_PROFIT => TypeQueryParts {
            entity_type_key: ref_keys::KEY_ENTITY_TYPE_PROFIT,
            bridge_type_keys: &[ref_keys::KEY_RELATIONSHIP_FOLLOW],
            label_expr: "COALESCE(sp.label, se.handle)",
            join_extension: r#"
        INNER JOIN social_profit sp ON sp.social_entity_id = se.id
        INNER JOIN ref_key_values rk_jp
            ON rk_jp.id = sp.join_policy_id
           AND rk_jp.`group` = ?
           AND rk_jp.is_active = 1
           AND rk_jp.`key` <> ?
            "#,
        },
        ref_keys::KEY_ENTITY_TYPE_NON_PROFIT => TypeQueryParts {
            entity_type_key: ref_keys::KEY_ENTITY_TYPE_NON_PROFIT,
            bridge_type_keys: &[ref_keys::KEY_RELATIONSHIP_FOLLOW],
            label_expr: "COALESCE(snp.label, se.handle)",
            join_extension: r#"
        INNER JOIN social_non_profit snp ON snp.social_entity_id = se.id
        INNER JOIN ref_key_values rk_jp
            ON rk_jp.id = snp.join_policy_id
           AND rk_jp.`group` = ?
           AND rk_jp.is_active = 1
           AND rk_jp.`key` <> ?
            "#,
        },
        _ => return None,
    })
}

/// Humans directly connected to the viewer (connection-rail predicate).
fn sql_direct_peer_humans() -> &'static str {
    r#"
        SELECT DISTINCT peer_h.id AS bridge_human_id
        FROM social_entity_relationship ser_peer
        INNER JOIN ref_key_values rk_peer_st
            ON rk_peer_st.id = ser_peer.status_id
           AND rk_peer_st.`group` = ?
           AND rk_peer_st.is_active = 1
           AND rk_peer_st.`key` IN (?, ?, ?)
        INNER JOIN social_entity peer_h
            ON peer_h.id = CASE
                WHEN ser_peer.from_social_entity_id = ? THEN ser_peer.to_social_entity_id
                ELSE ser_peer.from_social_entity_id
            END
        INNER JOIN ref_key_values rk_human_type
            ON rk_human_type.id = peer_h.type_id
           AND rk_human_type.`group` = ?
           AND rk_human_type.`key` = ?
           AND rk_human_type.is_active = 1
        WHERE ser_peer.ended_at IS NULL
          AND (ser_peer.from_social_entity_id = ? OR ser_peer.to_social_entity_id = ?)
          AND peer_h.id <> ?
    "#
}

/// Expansion cohort: members of communities the viewer or direct peers belong to.
fn sql_expansion_cohort_humans() -> String {
    format!(
        r#"
        SELECT ranked_cohort.cohort_human_id AS bridge_human_id
        FROM (
            SELECT
                mh.id AS cohort_human_id,
                ROW_NUMBER() OVER (
                    PARTITION BY anchor_communities.community_id
                    ORDER BY mh.id ASC
                ) AS rn
            FROM (
                SELECT ser_anchor.to_social_entity_id AS community_id
                FROM social_entity_relationship ser_anchor
                INNER JOIN ref_key_values rk_anchor_type
                    ON rk_anchor_type.id = ser_anchor.type_id
                   AND rk_anchor_type.`group` = ?
                   AND rk_anchor_type.`key` = ?
                   AND rk_anchor_type.is_active = 1
                INNER JOIN ref_key_values rk_anchor_st
                    ON rk_anchor_st.id = ser_anchor.status_id
                   AND rk_anchor_st.`group` = ?
                   AND rk_anchor_st.is_active = 1
                   AND rk_anchor_st.`key` = ?
                WHERE ser_anchor.ended_at IS NULL
                  AND (
                      ser_anchor.from_social_entity_id = ?
                      OR ser_anchor.from_social_entity_id IN ({direct_peers})
                  )
                ORDER BY ser_anchor.to_social_entity_id ASC
                LIMIT ?
            ) AS anchor_communities
            INNER JOIN social_entity_relationship ser_member
                ON ser_member.to_social_entity_id = anchor_communities.community_id
            INNER JOIN ref_key_values rk_member_type
                ON rk_member_type.id = ser_member.type_id
               AND rk_member_type.`group` = ?
               AND rk_member_type.`key` = ?
               AND rk_member_type.is_active = 1
            INNER JOIN ref_key_values rk_member_st
                ON rk_member_st.id = ser_member.status_id
               AND rk_member_st.`group` = ?
               AND rk_member_st.is_active = 1
               AND rk_member_st.`key` = ?
            INNER JOIN social_entity mh
                ON mh.id = ser_member.from_social_entity_id
            INNER JOIN ref_key_values rk_mh_type
                ON rk_mh_type.id = mh.type_id
               AND rk_mh_type.`group` = ?
               AND rk_mh_type.`key` = ?
               AND rk_mh_type.is_active = 1
            WHERE ser_member.ended_at IS NULL
        ) AS ranked_cohort
        WHERE ranked_cohort.rn <= ?
        LIMIT ?
        "#,
        direct_peers = sql_direct_peer_humans(),
    )
}

fn bridge_pool_sql(pool: BridgePool) -> String {
    match pool {
        BridgePool::DirectPeers => sql_direct_peer_humans().to_string(),
        BridgePool::ExpansionCohort => sql_expansion_cohort_humans(),
    }
}

fn bind_direct_peer_subquery<'q>(
    q: sqlx::query::QueryAs<'q, sqlx::MySql, CandidateRow, sqlx::mysql::MySqlArguments>,
    viewer_human_entity_id: u64,
) -> sqlx::query::QueryAs<'q, sqlx::MySql, CandidateRow, sqlx::mysql::MySqlArguments> {
    q.bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_PENDING_VERIFICATION)
        .bind(ref_keys::KEY_VERIFIED)
        .bind(ref_keys::KEY_FLAGGED)
        .bind(viewer_human_entity_id)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
        .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
        .bind(viewer_human_entity_id)
        .bind(viewer_human_entity_id)
        .bind(viewer_human_entity_id)
}

fn bind_expansion_cohort_subquery<'q>(
    q: sqlx::query::QueryAs<'q, sqlx::MySql, CandidateRow, sqlx::mysql::MySqlArguments>,
    viewer_human_entity_id: u64,
) -> sqlx::query::QueryAs<'q, sqlx::MySql, CandidateRow, sqlx::mysql::MySqlArguments> {
    let q = q
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
        .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_VERIFIED)
        .bind(viewer_human_entity_id);
    let q = bind_direct_peer_subquery(q, viewer_human_entity_id);
    q.bind(MAX_ANCHOR_COMMUNITIES)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
        .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_VERIFIED)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
        .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
        .bind(MAX_MEMBERS_PER_COMMUNITY)
        .bind(MAX_EXPANSION_COHORT)
}

fn bind_bridge_pool<'q>(
    q: sqlx::query::QueryAs<'q, sqlx::MySql, CandidateRow, sqlx::mysql::MySqlArguments>,
    pool: BridgePool,
    viewer_human_entity_id: u64,
) -> sqlx::query::QueryAs<'q, sqlx::MySql, CandidateRow, sqlx::mysql::MySqlArguments> {
    match pool {
        BridgePool::DirectPeers => bind_direct_peer_subquery(q, viewer_human_entity_id),
        BridgePool::ExpansionCohort => bind_expansion_cohort_subquery(q, viewer_human_entity_id),
    }
}

/// Binds for `{block_sql}` ([`blocks::SQL_EXCLUDE_BLOCKED_PEERS`]) in the outer query.
fn bind_block_exclusion_query_as<'q>(
    q: sqlx::query::QueryAs<'q, sqlx::MySql, CandidateRow, sqlx::mysql::MySqlArguments>,
    viewer_human_entity_id: u64,
) -> sqlx::query::QueryAs<'q, sqlx::MySql, CandidateRow, sqlx::mysql::MySqlArguments> {
    q.bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
        .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
        .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
        .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_BLOCKED)
        .bind(viewer_human_entity_id)
        .bind(viewer_human_entity_id)
}

/// Binds for the `NOT EXISTS` already-connected exclusion on `se`.
fn bind_already_connected_exclusion<'q>(
    q: sqlx::query::QueryAs<'q, sqlx::MySql, CandidateRow, sqlx::mysql::MySqlArguments>,
    viewer_human_entity_id: u64,
) -> sqlx::query::QueryAs<'q, sqlx::MySql, CandidateRow, sqlx::mysql::MySqlArguments> {
    q.bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_PENDING_VERIFICATION)
        .bind(ref_keys::KEY_VERIFIED)
        .bind(ref_keys::KEY_FLAGGED)
        .bind(ref_keys::KEY_BLOCKED)
        .bind(viewer_human_entity_id)
        .bind(viewer_human_entity_id)
}

/// Post-bridge filters in **SQL clause order** (must match the formatted query):
/// 1. [`blocks::SQL_EXCLUDE_BLOCKED_PEERS`]
/// 2. already-connected `NOT EXISTS`
/// 3. optional tier-dedupe `NOT IN`
/// 4. `LIMIT`
fn bind_post_bridge_filters<'q>(
    mut q: sqlx::query::QueryAs<'q, sqlx::MySql, CandidateRow, sqlx::mysql::MySqlArguments>,
    viewer_human_entity_id: u64,
    exclude_entity_ids: &[u64],
    lim: i64,
) -> sqlx::query::QueryAs<'q, sqlx::MySql, CandidateRow, sqlx::mysql::MySqlArguments> {
    q = bind_block_exclusion_query_as(q, viewer_human_entity_id);
    q = bind_already_connected_exclusion(q, viewer_human_entity_id);
    for id in exclude_entity_ids {
        q = q.bind(*id);
    }
    q.bind(lim)
}

pub async fn recommend_for_bridge_pool(
    pool: &MySqlPool,
    viewer_human_entity_id: u64,
    bridge_pool: BridgePool,
    type_key: &str,
    source_key: &str,
    conceal_panoply: bool,
    exclude_entity_ids: &HashSet<u64>,
    per_type: usize,
) -> Result<Vec<EntityRecommendation>, sqlx::Error> {
    let Some(parts) = type_parts(type_key) else {
        return Ok(Vec::new());
    };

    let alive_predicate = public_profile::sql_alive_entity_predicate("se");
    let avatar_sql = public_profile::sql_avatar_path_expr("se");
    let panoply_exclude = if conceal_panoply {
        crate::social::panoply::sql_exclude_panoply_entity("se")
    } else {
        String::new()
    };
    let block_sql = blocks::SQL_EXCLUDE_BLOCKED_PEERS;

    let bridge_type_placeholders = parts
        .bridge_type_keys
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");

    let mut exclude_sorted: Vec<u64> = exclude_entity_ids.iter().copied().collect();
    exclude_sorted.sort_unstable();

    let exclude_sql = if exclude_sorted.is_empty() {
        String::new()
    } else {
        let placeholders = exclude_sorted
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(", ");
        format!("AND se.id NOT IN ({placeholders})")
    };

    let lim = per_type.clamp(1, PER_TYPE_LIMIT) as i64;
    let bridge_pool_sql = bridge_pool_sql(bridge_pool);

    let sql = format!(
        r#"
        SELECT
            se.id AS entity_id,
            se.handle,
            {label_expr} AS label,
            rk_type.`key` AS type_key,
            ANY_VALUE({avatar_sql}) AS avatar_path,
            COUNT(DISTINCT bridge_human.id) AS bridge_count,
            COALESCE(se.rating, 0.00) AS entity_rating
        FROM social_entity se
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = se.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` = ?
           AND rk_type.is_active = 1
        {join_extension}
        INNER JOIN social_entity_relationship ser_bridge
            ON ser_bridge.to_social_entity_id = se.id
           AND ser_bridge.ended_at IS NULL
        INNER JOIN ref_key_values rk_bridge_type
            ON rk_bridge_type.id = ser_bridge.type_id
           AND rk_bridge_type.`group` = ?
           AND rk_bridge_type.`key` IN ({bridge_type_placeholders})
           AND rk_bridge_type.is_active = 1
        INNER JOIN ref_key_values rk_bridge_st
            ON rk_bridge_st.id = ser_bridge.status_id
           AND rk_bridge_st.`group` = ?
           AND rk_bridge_st.is_active = 1
           AND rk_bridge_st.`key` = ?
        INNER JOIN social_entity bridge_human
            ON bridge_human.id = ser_bridge.from_social_entity_id
        INNER JOIN ref_key_values rk_bridge_human_type
            ON rk_bridge_human_type.id = bridge_human.type_id
           AND rk_bridge_human_type.`group` = ?
           AND rk_bridge_human_type.`key` = ?
           AND rk_bridge_human_type.is_active = 1
        INNER JOIN ({bridge_pool_sql}) bridge_pool
            ON bridge_pool.bridge_human_id = bridge_human.id
        WHERE {alive_predicate}
          {panoply_exclude}
          {block_sql}
          AND NOT EXISTS (
              SELECT 1
              FROM social_entity_relationship ser_own
              INNER JOIN ref_key_values rk_own_st
                  ON rk_own_st.id = ser_own.status_id
                 AND rk_own_st.`group` = ?
                 AND rk_own_st.is_active = 1
                 AND rk_own_st.`key` IN (?, ?, ?, ?)
              WHERE ser_own.ended_at IS NULL
                AND (
                    (ser_own.from_social_entity_id = ? AND ser_own.to_social_entity_id = se.id)
                    OR (ser_own.to_social_entity_id = ? AND ser_own.from_social_entity_id = se.id)
                )
          )
          {exclude_sql}
        GROUP BY se.id, se.handle, label, rk_type.`key`, se.rating
        ORDER BY bridge_count DESC, entity_rating DESC, se.handle ASC
        LIMIT ?
        "#,
        label_expr = parts.label_expr,
        avatar_sql = avatar_sql,
        join_extension = parts.join_extension,
        bridge_type_placeholders = bridge_type_placeholders,
        bridge_pool_sql = bridge_pool_sql,
        alive_predicate = alive_predicate,
        panoply_exclude = panoply_exclude,
        block_sql = block_sql,
        exclude_sql = exclude_sql,
    );

    let mut q = sqlx::query_as::<_, CandidateRow>(&sql)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
        .bind(parts.entity_type_key)
        .bind(ref_keys::GROUP_SOCIAL_JOIN_POLICY)
        .bind(ref_keys::KEY_JOIN_POLICY_INVITE_ONLY)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE);
    for key in parts.bridge_type_keys {
        q = q.bind(*key);
    }
    q = q
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_VERIFIED)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
        .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN);

    q = bind_bridge_pool(q, bridge_pool, viewer_human_entity_id);

    q = bind_post_bridge_filters(q, viewer_human_entity_id, &exclude_sorted, lim);

    let rows = q.fetch_all(pool).await?;

    Ok(rows
        .into_iter()
        .map(|row| EntityRecommendation {
            entity_id: row.entity_id,
            handle: row.handle,
            label: row.label,
            type_key: row.type_key,
            avatar_path: row.avatar_path,
            entity_rating: row.entity_rating,
            source: source_key.to_string(),
        })
        .collect())
}
