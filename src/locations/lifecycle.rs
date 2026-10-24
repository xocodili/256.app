//! Administrative invalidation of `locations` rows (cadastral change, provider retraction, etc.).
//!
//! Non-active rows **rotate** `canonical_fingerprint` so a new validated row can be inserted for the
//! same real-world coordinates without colliding on the unique index.

use rand::RngCore;
use sqlx::MySqlPool;

use crate::error::{AppError, AppResult};
use crate::i18n;
use crate::ref_key_values;
use crate::ref_keys;

/// Why a row was moved out of [`ref_keys::KEY_LOCATION_LIFECYCLE_ACTIVE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationInvalidationKind {
    /// Geography policy drift; remediate before Enhanced / marketplace flows.
    Deprecated,
    /// No longer a valid reference; entities must pick a new `locations` row.
    Obsolete,
}

fn target_key(kind: LocationInvalidationKind) -> &'static str {
    match kind {
        LocationInvalidationKind::Deprecated => ref_keys::KEY_LOCATION_LIFECYCLE_DEPRECATED,
        LocationInvalidationKind::Obsolete => ref_keys::KEY_LOCATION_LIFECYCLE_OBSOLETE,
    }
}

/// Marks a location non-active, frees its dedupe fingerprint, and optionally records a replacement id.
pub async fn invalidate_location(
    pool: &MySqlPool,
    location_id: u64,
    kind: LocationInvalidationKind,
    lifecycle_note: Option<&str>,
    superseded_by_location_id: Option<u64>,
    // `None` = automated / ops-token calls (stores NULL in `modified_by`).
    acting_user_id: Option<u64>,
) -> AppResult<()> {
    let active_id = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_LOCATION_LIFECYCLE,
        ref_keys::KEY_LOCATION_LIFECYCLE_ACTIVE,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or(AppError::NotFound)?;

    let target_id = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_LOCATION_LIFECYCLE,
        target_key(kind),
    )
    .await
    .map_err(AppError::Db)?
    .ok_or(AppError::NotFound)?;

    let current: Option<u64> = sqlx::query_scalar(
        "SELECT location_lifecycle_id FROM locations WHERE id = ? LIMIT 1",
    )
    .bind(location_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Db)?;

    let Some(current_lifecycle) = current else {
        return Err(AppError::NotFound);
    };

    if current_lifecycle != active_id {
        return Ok(());
    }

    let note = lifecycle_note.map(str::trim).filter(|s| !s.is_empty());
    if note.is_some_and(|s| s.len() > 256) {
        let loc = i18n::request_locale();
        return Err(AppError::BadRequest(i18n::catalog().tf(
            &loc,
            "location.field_too_long",
            &[("field", "lifecycle_note")],
        )));
    }

    let mut new_fp = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut new_fp);

    sqlx::query(
        r#"
        UPDATE locations
        SET
            location_lifecycle_id = ?,
            lifecycle_changed_at = UTC_TIMESTAMP(),
            lifecycle_note = ?,
            superseded_by_location_id = ?,
            canonical_fingerprint = ?,
            modified_by = ?
        WHERE id = ?
          AND location_lifecycle_id = ?
        "#,
    )
    .bind(target_id)
    .bind(note)
    .bind(superseded_by_location_id)
    .bind(new_fp.as_slice())
    .bind(acting_user_id)
    .bind(location_id)
    .bind(active_id)
    .execute(pool)
    .await
    .map_err(AppError::Db)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::LocationInvalidationKind;

    #[test]
    fn invalidation_kind_variants_distinct() {
        assert_ne!(
            std::mem::discriminant(&LocationInvalidationKind::Deprecated),
            std::mem::discriminant(&LocationInvalidationKind::Obsolete)
        );
    }
}
