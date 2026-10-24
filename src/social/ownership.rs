//! Explicit user<->social_entity ownership and role links.

use sqlx::MySqlPool;

/// Ensure the actor has an owner role for this social entity.
pub async fn ensure_owner_role(
    pool: &MySqlPool,
    user_id: u64,
    social_entity_id: u64,
) -> Result<(), sqlx::Error> {
    let role_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT id
        FROM ref_key_values
        WHERE `group` = 'user_social_entity_role'
          AND `key` = 'owner'
          AND is_active = 1
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?;
    let Some(role_id) = role_id else {
        return Ok(());
    };

    sqlx::query(
        r#"
        INSERT INTO user_social_entity_role (
            user_id, social_entity_id, role_id, is_primary, starts_at, created_by, modified_by
        )
        VALUES (?, ?, ?, 1, UTC_DATE(), ?, ?)
        ON DUPLICATE KEY UPDATE
            modified_by = VALUES(modified_by),
            is_primary = GREATEST(is_primary, VALUES(is_primary))
        "#,
    )
    .bind(user_id)
    .bind(social_entity_id)
    .bind(role_id)
    .bind(user_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// True when the user has any non-expired role row for the social entity (owner, operator, etc.).
pub async fn user_has_active_role_for_entity(
    pool: &MySqlPool,
    user_id: u64,
    social_entity_id: u64,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM user_social_entity_role
            WHERE user_id = ?
              AND social_entity_id = ?
              AND (ends_at IS NULL OR ends_at >= UTC_DATE())
        )
        "#,
    )
    .bind(user_id)
    .bind(social_entity_id)
    .fetch_one(pool)
    .await
}

/// Active primary social entity id for a user, when available.
pub async fn primary_entity_id_for_user(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<Option<u64>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT social_entity_id
        FROM user_social_entity_role
        WHERE user_id = ? AND is_primary = 1 AND (ends_at IS NULL OR ends_at >= UTC_DATE())
        ORDER BY id ASC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}
