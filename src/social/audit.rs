//! Shared helpers for `created_by` / `modified_by` audit columns on graph and judicial rows.

use sqlx::{MySql, MySqlPool, Transaction};

use crate::ref_keys;

/// Platform user (`system@256.app`) for scheduler-driven and settlement writes.
pub async fn system_user_id(pool: &MySqlPool) -> Result<Option<u64>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT id FROM users WHERE email = ? LIMIT 1
        "#,
    )
    .bind(ref_keys::SYSTEM_USER_EMAIL)
    .fetch_optional(pool)
    .await
}

pub async fn touch_social_entity_modified_by_tx(
    tx: &mut Transaction<'_, MySql>,
    entity_id: u64,
    user_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE social_entity
        SET modified_by = ?
        WHERE id = ?
        "#,
    )
    .bind(user_id)
    .bind(entity_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn touch_judicial_case_modified_by_tx(
    tx: &mut Transaction<'_, MySql>,
    case_id: u64,
    user_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE judicial_case
        SET modified_by = ?
        WHERE id = ?
        "#,
    )
    .bind(user_id)
    .bind(case_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
