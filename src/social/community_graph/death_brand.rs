//! Voluntary removal for profit, non-profit, and gathering social entities.

use sqlx::{MySql, Transaction};

use super::death_shared::{
    end_all_relationships_for_entity_tx, void_open_judicial_cases_for_entity_tx,
};
use super::types::DeathSource;
use crate::ref_key_values;
use crate::ref_keys;
use crate::social::scope;

/// Voluntary owner-initiated removal for brand/gathering extension entities.
pub async fn extension_entity_death_voluntary_tx(
    tx: &mut Transaction<'_, MySql>,
    entity_id: u64,
    actor: u64,
) -> Result<bool, sqlx::Error> {
    execute_extension_entity_death_tx(tx, entity_id, DeathSource::Voluntary, Some(actor)).await
}

async fn execute_extension_entity_death_tx(
    tx: &mut Transaction<'_, MySql>,
    entity_id: u64,
    source: DeathSource,
    actor: Option<u64>,
) -> Result<bool, sqlx::Error> {
    let Some((type_key,)) = sqlx::query_as::<_, (String,)>(
        r#"
        SELECT rk_type.`key`
        FROM social_entity se
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = se.type_id
           AND rk_type.`group` = ?
           AND rk_type.is_active = 1
        WHERE se.id = ?
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(entity_id)
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Ok(false);
    };

    if !matches!(
        type_key.as_str(),
        ref_keys::KEY_ENTITY_TYPE_PROFIT
            | ref_keys::KEY_ENTITY_TYPE_NON_PROFIT
            | ref_keys::KEY_ENTITY_TYPE_GATHERING
    ) {
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
            SELECT 1
            FROM social_entity se
            WHERE se.id = ?
              AND COALESCE(se.entity_status_id, se.status_id) = ?
        )
        "#,
    )
    .bind(entity_id)
    .bind(dead_status_id)
    .fetch_one(&mut **tx)
    .await?;
    if already_dead {
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

    let peer_ids = end_all_relationships_for_entity_tx(&mut **tx, entity_id, actor).await?;
    void_open_judicial_cases_for_entity_tx(&mut **tx, entity_id, actor).await?;

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
    .bind(entity_id)
    .execute(&mut **tx)
    .await?;

    match type_key.as_str() {
        ref_keys::KEY_ENTITY_TYPE_PROFIT => {
            sqlx::query(
                r#"
                UPDATE social_profit
                SET died_at = UTC_TIMESTAMP(6),
                    death_source_id = ?
                WHERE social_entity_id = ?
                "#,
            )
            .bind(death_source_id)
            .bind(entity_id)
            .execute(&mut **tx)
            .await?;
        }
        ref_keys::KEY_ENTITY_TYPE_NON_PROFIT => {
            sqlx::query(
                r#"
                UPDATE social_non_profit
                SET died_at = UTC_TIMESTAMP(6),
                    death_source_id = ?
                WHERE social_entity_id = ?
                "#,
            )
            .bind(death_source_id)
            .bind(entity_id)
            .execute(&mut **tx)
            .await?;
        }
        ref_keys::KEY_ENTITY_TYPE_GATHERING => {
            sqlx::query(
                r#"
                UPDATE social_gathering
                SET died_at = UTC_TIMESTAMP(6),
                    death_source_id = ?
                WHERE social_entity_id = ?
                "#,
            )
            .bind(death_source_id)
            .bind(entity_id)
            .execute(&mut **tx)
            .await?;
        }
        _ => {}
    }

    scope::mark_entity_dirty_in_tx(tx, entity_id, "extension_entity_death").await?;
    for peer_id in peer_ids {
        scope::mark_relationship_endpoints_dirty_in_tx(tx, entity_id, peer_id).await?;
    }

    Ok(true)
}
