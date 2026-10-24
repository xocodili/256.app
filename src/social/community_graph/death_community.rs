//! Community death at rating zero.

use rust_decimal::Decimal;
use sqlx::{MySql, MySqlPool, Transaction};

use crate::ref_key_values;
use crate::ref_keys;
use crate::social::scope;

use super::death_shared::void_open_judicial_cases_for_entity_tx;
use super::immune::{is_death_immune, is_death_immune_conn};
use super::reparent::reparent_children_of_dead_community_tx;
use super::tree::list_immediate_children_conn;
use super::types::DeathSource;

/// Communities at rating 0, verified, not immune — candidates for the death job.
pub async fn list_death_candidates(
    pool: &MySqlPool,
    limit: u64,
) -> Result<Vec<u64>, sqlx::Error> {
    let lim = limit.clamp(1, 64) as i64;
    let rows: Vec<u64> = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN social_community sc ON sc.social_entity_id = se.id
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = se.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` = ?
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = se.entity_status_id
           AND rk_st.`group` = ?
           AND rk_st.`key` = ?
        WHERE se.rating = 0.00
        ORDER BY se.id ASC
        LIMIT ?
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_COMMUNITY)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(lim)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::new();
    for id in rows {
        if !is_death_immune(pool, id).await? {
            out.push(id);
        }
    }
    Ok(out)
}

/// Full death ritual. Returns `Ok(true)` when the community was transitioned; `Ok(false)` if skipped.
pub async fn community_death_tx(
    tx: &mut Transaction<'_, MySql>,
    community_id: u64,
    source: DeathSource,
    actor: Option<u64>,
) -> Result<bool, sqlx::Error> {
    if is_death_immune_conn(&mut **tx, community_id).await? {
        return Ok(false);
    }

    let dead_status_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_SOCIAL_ENTITY_STATUS,
        ref_keys::KEY_ENTITY_STATUS_DEAD,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("ref_key_values missing social_entity_status.dead".into())
    })?;

    let already_dead: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM social_entity se
            WHERE se.id = ?
              AND se.entity_status_id = ?
        )
        "#,
    )
    .bind(community_id)
    .bind(dead_status_id)
    .fetch_one(&mut **tx)
    .await?;
    if already_dead {
        return Ok(false);
    }

    let rating: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(se.rating, 0) FROM social_entity se WHERE se.id = ? LIMIT 1"#,
    )
    .bind(community_id)
    .fetch_one(&mut **tx)
    .await?;
    if rating != Decimal::ZERO {
        return Ok(false);
    }

    let death_source_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_GRAPH_DEATH_SOURCE,
        source.ref_key(),
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration(
            format!(
                "ref_key_values missing {}.{}",
                ref_keys::GROUP_GRAPH_DEATH_SOURCE,
                source.ref_key()
            )
            .into(),
        )
    })?;

    let old_parent_id: Option<u64> = sqlx::query_scalar(
        r#"SELECT parent_community_id FROM social_community WHERE social_entity_id = ? LIMIT 1"#,
    )
    .bind(community_id)
    .fetch_optional(&mut **tx)
    .await?;

    let former_members = end_memberships_for_community_tx(&mut **tx, community_id, actor).await?;
    void_open_judicial_cases_for_entity_tx(&mut **tx, community_id, actor).await?;

    sqlx::query(
        r#"
        UPDATE social_entity se
        SET se.entity_status_id = ?,
            se.status_id = ?,
            se.modified_by = ?
        WHERE se.id = ?
        "#,
    )
    .bind(dead_status_id)
    .bind(dead_status_id)
    .bind(actor)
    .bind(community_id)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE social_community
        SET died_at = UTC_TIMESTAMP(6),
            death_source_id = ?
        WHERE social_entity_id = ?
        "#,
    )
    .bind(death_source_id)
    .bind(community_id)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE community_constitution_item
        SET status = 'retired',
            modified_by = ?
        WHERE community_id = ?
          AND status = 'active'
        "#,
    )
    .bind(actor)
    .bind(community_id)
    .execute(&mut **tx)
    .await?;

    // Snapshot children before reparent clears `parent_community_id` on the dead node.
    let children = list_immediate_children_conn(&mut **tx, community_id).await?;
    reparent_children_of_dead_community_tx(&mut **tx, community_id, actor).await?;

    scope::mark_entity_dirty_in_tx(tx, community_id, "community_death").await?;
    if let Some(pid) = old_parent_id {
        scope::mark_entity_dirty_in_tx(tx, pid, "community_death").await?;
    }
    for child_id in &children {
        scope::mark_entity_dirty_in_tx(tx, *child_id, "community_death").await?;
    }
    for human_id in former_members {
        scope::mark_relationship_endpoints_dirty_in_tx(tx, human_id, community_id).await?;
    }

    sqlx::query(
        r#"
        INSERT INTO community_graph_event (
            event_type,
            community_social_entity_id,
            old_parent_id,
            new_parent_id,
            detail_json,
            created_by
        )
        VALUES ('death', ?, ?, ?, JSON_OBJECT('death_source', ?), ?)
        "#,
    )
    .bind(community_id)
    .bind(old_parent_id)
    .bind(old_parent_id)
    .bind(source.ref_key())
    .bind(actor)
    .execute(&mut **tx)
    .await?;

    Ok(true)
}

async fn end_memberships_for_community_tx(
    conn: &mut sqlx::MySqlConnection,
    community_id: u64,
    actor: Option<u64>,
) -> Result<Vec<u64>, sqlx::Error> {
    let member_edges: Vec<(u64, u64)> = sqlx::query_as(
        r#"
        SELECT ser.id, ser.from_social_entity_id
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = ser.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` = ?
        WHERE ser.to_social_entity_id = ?
          AND ser.ended_at IS NULL
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .bind(community_id)
    .fetch_all(&mut *conn)
    .await?;

    let human_ids: Vec<u64> = member_edges.iter().map(|(_, h)| *h).collect();
    for (edge_id, _human_id) in &member_edges {
        sqlx::query(
            r#"
            UPDATE social_entity_relationship
            SET ended_at = UTC_TIMESTAMP(6), modified_by = ?
            WHERE id = ?
            "#,
        )
        .bind(actor)
        .bind(edge_id)
        .execute(&mut *conn)
        .await?;
    }
    Ok(human_ids)
}
