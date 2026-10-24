//! Community tree reparenting (death cascade and judicial verdict).

use sqlx::{MySql, Transaction};

use crate::social::scope;

use super::tree::collect_subtree_community_ids_conn;

pub async fn reparent_children_of_dead_community_tx(
    conn: &mut sqlx::MySqlConnection,
    dead_id: u64,
    _actor: Option<u64>,
) -> Result<(), sqlx::Error> {
    let new_parent: Option<u64> = sqlx::query_scalar(
        r#"SELECT parent_community_id FROM social_community WHERE social_entity_id = ? LIMIT 1"#,
    )
    .bind(dead_id)
    .fetch_optional(&mut *conn)
    .await?;

    sqlx::query(
        r#"
        UPDATE social_community
        SET parent_community_id = ?
        WHERE parent_community_id = ?
        "#,
    )
    .bind(new_parent)
    .bind(dead_id)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Apply judicial reparent: move `moving_community_id` under `proposed_parent_id`.
pub async fn apply_judicial_reparent_tx(
    tx: &mut Transaction<'_, MySql>,
    judicial_case_id: u64,
    moving_community_id: u64,
    proposed_parent_id: u64,
    actor: Option<u64>,
) -> Result<(), sqlx::Error> {
    let old_parent: Option<u64> = sqlx::query_scalar(
        r#"SELECT parent_community_id FROM social_community WHERE social_entity_id = ? LIMIT 1"#,
    )
    .bind(moving_community_id)
    .fetch_optional(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE social_community
        SET parent_community_id = ?
        WHERE social_entity_id = ?
        "#,
    )
    .bind(proposed_parent_id)
    .bind(moving_community_id)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE judicial_case_community_reparent
        SET phase = 'completed'
        WHERE judicial_case_id = ?
        "#,
    )
    .bind(judicial_case_id)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO community_graph_event (
            event_type,
            community_social_entity_id,
            old_parent_id,
            new_parent_id,
            judicial_case_id,
            created_by
        )
        VALUES ('reparent_judicial', ?, ?, ?, ?, ?)
        "#,
    )
    .bind(moving_community_id)
    .bind(old_parent)
    .bind(proposed_parent_id)
    .bind(judicial_case_id)
    .bind(actor)
    .execute(&mut **tx)
    .await?;

    scope::mark_entity_dirty_in_tx(tx, moving_community_id, "community_reparent").await?;
    scope::mark_entity_dirty_in_tx(tx, proposed_parent_id, "community_reparent").await?;
    if let Some(op) = old_parent {
        scope::mark_entity_dirty_in_tx(tx, op, "community_reparent").await?;
    }
    let subtree = collect_subtree_community_ids_conn(&mut **tx, moving_community_id).await?;
    for id in subtree {
        scope::mark_entity_dirty_in_tx(tx, id, "community_reparent").await?;
    }
    Ok(())
}
