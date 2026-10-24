//! Human removal at rating zero (or voluntary account deletion).

use rust_decimal::Decimal;
use sqlx::{MySql, MySqlPool, Transaction};

use crate::ref_key_values;
use crate::ref_keys;
use crate::social::scope;

use super::death_shared::{
    end_all_relationships_for_entity_tx, void_open_judicial_cases_for_entity_tx,
};
use super::immune::{is_human_death_immune, is_human_death_immune_conn};
use super::types::DeathSource;
use crate::social::judicial::failing_rating_threshold;

pub async fn is_human_failing(pool: &MySqlPool, human_id: u64) -> Result<bool, sqlx::Error> {
    if is_human_death_immune(pool, human_id).await? {
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
    .bind(human_id)
    .fetch_optional(pool)
    .await?;
    if status_key.as_deref() == Some(ref_keys::KEY_ENTITY_STATUS_DEAD) {
        return Ok(false);
    }
    let rating: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(se.rating, 0) FROM social_entity se WHERE se.id = ? LIMIT 1"#,
    )
    .bind(human_id)
    .fetch_one(pool)
    .await?;
    Ok(rating < failing_rating_threshold())
}

/// Humans at rating 0, verified, not immune — candidates for the graph lifecycle job.
pub async fn list_human_death_candidates(
    pool: &MySqlPool,
    limit: u64,
) -> Result<Vec<u64>, sqlx::Error> {
    let lim = limit.clamp(1, 64) as i64;
    let rows: Vec<u64> = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
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
    .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(lim)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::new();
    for id in rows {
        if !is_human_death_immune(pool, id).await? {
            out.push(id);
        }
    }
    Ok(out)
}

async fn human_already_dead(
    conn: &mut sqlx::MySqlConnection,
    human_id: u64,
    dead_status_id: u64,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM social_entity se
            WHERE se.id = ?
              AND se.entity_status_id = ?
        )
        "#,
    )
    .bind(human_id)
    .bind(dead_status_id)
    .fetch_one(&mut *conn)
    .await
}

/// Shared human removal ritual: end relationships, void cases, retire users, mark entity dead.
async fn execute_human_death_ritual_tx(
    tx: &mut Transaction<'_, MySql>,
    human_id: u64,
    source: DeathSource,
    actor: Option<u64>,
) -> Result<bool, sqlx::Error> {
    let dead_status_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_SOCIAL_ENTITY_STATUS,
        ref_keys::KEY_ENTITY_STATUS_DEAD,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("ref_key_values missing social_entity_status.dead".into())
    })?;

    if human_already_dead(&mut **tx, human_id, dead_status_id).await? {
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

    let peer_ids = end_all_relationships_for_entity_tx(&mut **tx, human_id, actor).await?;
    void_open_judicial_cases_for_entity_tx(&mut **tx, human_id, actor).await?;
    retire_user_accounts_for_human_tx(&mut **tx, human_id, actor).await?;

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
    .bind(human_id)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE social_human
        SET died_at = UTC_TIMESTAMP(6),
            death_source_id = ?
        WHERE social_entity_id = ?
        "#,
    )
    .bind(death_source_id)
    .bind(human_id)
    .execute(&mut **tx)
    .await?;

    scope::mark_entity_dirty_in_tx(tx, human_id, "human_death").await?;
    for peer_id in peer_ids {
        scope::mark_relationship_endpoints_dirty_in_tx(tx, human_id, peer_id).await?;
    }

    sqlx::query(
        r#"
        INSERT INTO human_graph_event (
            event_type,
            human_social_entity_id,
            detail_json,
            created_by
        )
        VALUES ('death', ?, JSON_OBJECT('death_source', ?), ?)
        "#,
    )
    .bind(human_id)
    .bind(source.ref_key())
    .bind(actor)
    .execute(&mut **tx)
    .await?;

    Ok(true)
}

async fn block_user_accounts_for_human_tx(
    conn: &mut sqlx::MySqlConnection,
    human_id: u64,
    actor: Option<u64>,
) -> Result<(), sqlx::Error> {
    let blocked_user_status = ref_key_values::lookup_active_id_conn(
        conn,
        ref_keys::GROUP_USER_STATUS,
        ref_keys::KEY_BLOCKED,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("ref_key_values missing user_status.blocked".into())
    })?;
    let blocked_legacy = ref_key_values::lookup_active_id_conn(
        conn,
        ref_keys::GROUP_STATUS,
        ref_keys::KEY_BLOCKED,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("ref_key_values missing status.blocked".into())
    })?;

    let user_ids: Vec<u64> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT ur.user_id
        FROM user_social_entity_role ur
        WHERE ur.social_entity_id = ?
          AND ur.ends_at IS NULL
        "#,
    )
    .bind(human_id)
    .fetch_all(&mut *conn)
    .await?;

    for user_id in user_ids {
        sqlx::query(
            r#"
            UPDATE users
            SET status_id = ?, user_status_id = ?, modified_by = ?
            WHERE id = ?
            "#,
        )
        .bind(blocked_legacy)
        .bind(blocked_user_status)
        .bind(actor)
        .bind(user_id)
        .execute(&mut *conn)
        .await?;

        sqlx::query("DELETE FROM sessions WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

async fn execute_human_safety_ban_ritual_tx(
    tx: &mut Transaction<'_, MySql>,
    human_id: u64,
    source: DeathSource,
    actor: Option<u64>,
) -> Result<bool, sqlx::Error> {
    let dead_status_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_SOCIAL_ENTITY_STATUS,
        ref_keys::KEY_ENTITY_STATUS_DEAD,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("ref_key_values missing social_entity_status.dead".into())
    })?;

    if human_already_dead(&mut **tx, human_id, dead_status_id).await? {
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

    let peer_ids = end_all_relationships_for_entity_tx(&mut **tx, human_id, actor).await?;
    void_open_judicial_cases_for_entity_tx(&mut **tx, human_id, actor).await?;
    block_user_accounts_for_human_tx(&mut **tx, human_id, actor).await?;

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
    .bind(human_id)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE social_human
        SET died_at = UTC_TIMESTAMP(6),
            death_source_id = ?
        WHERE social_entity_id = ?
        "#,
    )
    .bind(death_source_id)
    .bind(human_id)
    .execute(&mut **tx)
    .await?;

    scope::mark_entity_dirty_in_tx(tx, human_id, "human_safety_ban").await?;
    for peer_id in peer_ids {
        scope::mark_relationship_endpoints_dirty_in_tx(tx, human_id, peer_id).await?;
    }

    sqlx::query(
        r#"
        INSERT INTO human_graph_event (
            event_type,
            human_social_entity_id,
            detail_json,
            created_by
        )
        VALUES ('death', ?, JSON_OBJECT('death_source', ?), ?)
        "#,
    )
    .bind(human_id)
    .bind(source.ref_key())
    .bind(actor)
    .execute(&mut **tx)
    .await?;

    Ok(true)
}

/// Platform safety ban after lost emergency contest — `blocked` users, not reactivatable.
pub async fn human_safety_ban_tx(
    pool: &MySqlPool,
    human_id: u64,
    source: DeathSource,
    actor: Option<u64>,
) -> Result<bool, sqlx::Error> {
    if is_human_death_immune(pool, human_id).await? {
        return Ok(false);
    }
    let mut tx = pool.begin().await?;
    let applied = execute_human_safety_ban_ritual_tx(&mut tx, human_id, source, actor).await?;
    tx.commit().await?;
    Ok(applied)
}

/// Full human removal ritual at rating zero. Returns `Ok(true)` when transitioned.
pub async fn human_death_tx(
    tx: &mut Transaction<'_, MySql>,
    human_id: u64,
    source: DeathSource,
    actor: Option<u64>,
) -> Result<bool, sqlx::Error> {
    if is_human_death_immune_conn(&mut **tx, human_id).await? {
        return Ok(false);
    }

    let rating: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(se.rating, 0) FROM social_entity se WHERE se.id = ? LIMIT 1"#,
    )
    .bind(human_id)
    .fetch_one(&mut **tx)
    .await?;
    if rating != Decimal::ZERO {
        return Ok(false);
    }

    execute_human_death_ritual_tx(tx, human_id, source, actor).await
}

/// Voluntary account deletion: zero rating, then the same removal ritual as rating-zero death.
pub async fn human_death_voluntary_tx(
    tx: &mut Transaction<'_, MySql>,
    human_id: u64,
    actor_user_id: u64,
) -> Result<bool, sqlx::Error> {
    if is_human_death_immune_conn(&mut **tx, human_id).await? {
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

    if human_already_dead(&mut **tx, human_id, dead_status_id).await? {
        return Ok(false);
    }

    sqlx::query(
        r#"
        UPDATE social_entity
        SET rating = 0.00,
            modified_by = ?
        WHERE id = ?
        "#,
    )
    .bind(actor_user_id)
    .bind(human_id)
    .execute(&mut **tx)
    .await?;

    execute_human_death_ritual_tx(
        tx,
        human_id,
        DeathSource::Voluntary,
        Some(actor_user_id),
    )
    .await
}

async fn retire_user_accounts_for_human_tx(
    conn: &mut sqlx::MySqlConnection,
    human_id: u64,
    actor: Option<u64>,
) -> Result<(), sqlx::Error> {
    let to_be_deleted_id = ref_key_values::lookup_active_id_conn(
        conn,
        ref_keys::GROUP_USER_STATUS,
        ref_keys::KEY_TO_BE_DELETED,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("ref_key_values missing user_status.to_be_deleted".into())
    })?;
    let legacy_deleted_id = ref_key_values::lookup_active_id_conn(
        conn,
        ref_keys::GROUP_STATUS,
        ref_keys::KEY_TO_BE_DELETED,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("ref_key_values missing status.to_be_deleted".into())
    })?;

    let user_ids: Vec<u64> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT ur.user_id
        FROM user_social_entity_role ur
        WHERE ur.social_entity_id = ?
          AND ur.ends_at IS NULL
        "#,
    )
    .bind(human_id)
    .fetch_all(&mut *conn)
    .await?;

    for user_id in user_ids {
        sqlx::query(
            r#"
            UPDATE users
            SET status_id = ?, user_status_id = ?, modified_by = ?
            WHERE id = ?
            "#,
        )
        .bind(legacy_deleted_id)
        .bind(to_be_deleted_id)
        .bind(actor)
        .bind(user_id)
        .execute(&mut *conn)
        .await?;

        sqlx::query("DELETE FROM sessions WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}
