//! Shared SQL checks for the social domain.

use sqlx::MySqlPool;

use crate::ref_keys;

/// Active row in `ref_key_values` for the given `group`.
pub async fn ref_row_active(pool: &MySqlPool, id: u64, group: &str) -> Result<bool, sqlx::Error> {
    let ok = sqlx::query_scalar::<_, Option<u64>>(
        r#"
        SELECT id FROM ref_key_values
        WHERE id = ? AND `group` = ? AND is_active = 1
        LIMIT 1
        "#,
    )
    .bind(id)
    .bind(group)
    .fetch_optional(pool)
    .await?;
    Ok(ok.is_some())
}

pub async fn location_exists(pool: &MySqlPool, id: u64) -> Result<bool, sqlx::Error> {
    let ok = sqlx::query_scalar::<_, Option<u64>>(
        r#"
        SELECT l.id
        FROM locations l
        INNER JOIN ref_key_values rk ON rk.id = l.location_lifecycle_id
        WHERE l.id = ?
          AND rk.`group` = ?
          AND rk.`key` = ?
          AND rk.is_active = 1
        LIMIT 1
        "#,
    )
    .bind(id)
    .bind(ref_keys::GROUP_LOCATION_LIFECYCLE)
    .bind(ref_keys::KEY_LOCATION_LIFECYCLE_ACTIVE)
    .fetch_optional(pool)
    .await?;
    Ok(ok.is_some())
}

pub async fn social_entity_exists(pool: &MySqlPool, id: u64) -> Result<bool, sqlx::Error> {
    let ok =
        sqlx::query_scalar::<_, Option<u64>>("SELECT id FROM social_entity WHERE id = ? LIMIT 1")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(ok.is_some())
}

/// MySQL server error number (e.g. 1062 duplicate, 1452 FK), if this is a MySQL
/// database error. NOTE: `DatabaseError::code()` returns the SQLSTATE (`"23000"` for
/// all integrity-constraint violations), which cannot distinguish a duplicate key
/// from a foreign-key failure — so we downcast to read the MySQL-specific number.
#[must_use]
pub fn mysql_error_number(e: &sqlx::Error) -> Option<u16> {
    e.as_database_error()
        .and_then(|db| db.try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>())
        .map(|db| db.number())
}

#[must_use]
pub fn is_foreign_key_violation(e: &sqlx::Error) -> bool {
    // 1451: cannot delete/update parent row; 1452: cannot add/update child row.
    matches!(mysql_error_number(e), Some(1451) | Some(1452))
}

#[must_use]
pub fn is_unique_violation(e: &sqlx::Error) -> bool {
    mysql_error_number(e) == Some(1062)
}
