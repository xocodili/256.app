//! Persist successful face capture rows (`user_faces`).
//!
//! Biometric note: this table intentionally stores **no** biometric data — no embeddings,
//! no raw images, and no AI appearance description. The capture images are processed
//! transiently (vision description → generated avatar) and discarded. The only durable
//! artifacts are the generated artwork path and a `face_captured_at` completion timestamp.

use std::path::Path;

use sqlx::MySqlPool;
use tracing::warn;

/// Upsert one successful capture per user (no biometric data stored). Returns row `id`.
pub async fn insert_face_capture(
    pool: &MySqlPool,
    user_id: u64,
    generated_artwork_path: Option<&str>,
    ai_model_id: &str,
    quality_score: Option<f32>,
    actor_user_id: u64,
) -> Result<u64, sqlx::Error> {
    let r = sqlx::query(
        r#"
        INSERT INTO user_faces (
            user_id, generated_artwork_path, ai_model_id, quality_score,
            captured_at, face_captured_at, created_by, modified_by
        )
        VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, ?, ?)
        ON DUPLICATE KEY UPDATE
            generated_artwork_path = VALUES(generated_artwork_path),
            ai_model_id = VALUES(ai_model_id),
            quality_score = VALUES(quality_score),
            captured_at = CURRENT_TIMESTAMP,
            face_captured_at = CURRENT_TIMESTAMP,
            modified_by = VALUES(modified_by),
            id = LAST_INSERT_ID(id)
        "#,
    )
    .bind(user_id)
    .bind(generated_artwork_path)
    .bind(ai_model_id)
    .bind(quality_score)
    .bind(actor_user_id)
    .bind(actor_user_id)
    .execute(pool)
    .await?;

    Ok(r.last_insert_id())
}

/// SQL fragment: completed face capture with a stored artwork path (alias `uf`).
pub const COMPLETE_ARTWORK_ROW: &str =
    "uf.face_captured_at IS NOT NULL AND uf.generated_artwork_path IS NOT NULL AND uf.generated_artwork_path <> ''";

#[must_use]
pub fn artwork_file_exists(rel_path: &str) -> bool {
    let trimmed = rel_path.trim();
    !trimmed.is_empty() && Path::new(trimmed).is_file()
}

async fn stored_artwork_path(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<Option<(String, i64)>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT generated_artwork_path, UNIX_TIMESTAMP(face_captured_at)
        FROM user_faces
        WHERE user_id = ?
          AND face_captured_at IS NOT NULL
          AND generated_artwork_path IS NOT NULL
          AND generated_artwork_path <> ''
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

#[must_use]
pub fn artwork_path_with_cache_bust(path: &str, version: i64) -> String {
    let trimmed = path.trim();
    if version <= 0 {
        return trimmed.to_string();
    }
    if trimmed.contains('?') {
        format!("{trimmed}&v={version}")
    } else {
        format!("{trimmed}?v={version}")
    }
}

/// Raw stored artwork path (no cache-bust) — for file authorization and disk reads.
pub async fn artwork_path_for_user(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<Option<String>, sqlx::Error> {
    let Some((path, _version)) = stored_artwork_path(pool, user_id).await? else {
        return Ok(None);
    };
    if artwork_file_exists(&path) {
        Ok(Some(path))
    } else {
        repair_orphan_artwork_row(pool, user_id, &path).await;
        Ok(None)
    }
}

/// Browser-facing avatar src with cache-bust from `face_captured_at`.
pub async fn artwork_display_src_for_user(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<Option<String>, sqlx::Error> {
    let Some((path, version)) = stored_artwork_path(pool, user_id).await? else {
        return Ok(None);
    };
    if artwork_file_exists(&path) {
        Ok(Some(artwork_path_with_cache_bust(&path, version)))
    } else {
        repair_orphan_artwork_row(pool, user_id, &path).await;
        Ok(None)
    }
}

/// Drop a live-mode row whose DB path points at a missing file (manual file delete, etc.).
async fn repair_orphan_artwork_row(pool: &MySqlPool, user_id: u64, stale_path: &str) {
    let result = sqlx::query("DELETE FROM user_faces WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await;
    match result {
        Ok(r) if r.rows_affected() > 0 => {
            warn!(
                user_id,
                stale_path,
                "removed orphan user_faces row (artwork file missing on disk)"
            );
        }
        Ok(_) => {}
        Err(err) => {
            warn!(
                user_id,
                stale_path,
                error = %err,
                "failed to remove orphan user_faces row"
            );
        }
    }
}

/// Whether the user has a completed face capture row (completion signal present).
pub async fn user_has_face_capture(pool: &MySqlPool, user_id: u64) -> Result<bool, sqlx::Error> {
    let exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(SELECT 1 FROM user_faces WHERE user_id = ? AND face_captured_at IS NOT NULL)"#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

/// Onboarding / post-auth gate. Live mode requires artwork on disk; mock mode accepts
/// `face_captured_at` alone (placeholder capture with no raster).
pub async fn user_passes_face_capture_gate(
    pool: &MySqlPool,
    user_id: u64,
    face_vision_mock: bool,
) -> Result<bool, sqlx::Error> {
    if face_vision_mock {
        return user_has_face_capture(pool, user_id).await;
    }

    let Some((path, _version)) = stored_artwork_path(pool, user_id).await? else {
        return Ok(false);
    };
    if artwork_file_exists(&path) {
        return Ok(true);
    }

    repair_orphan_artwork_row(pool, user_id, &path).await;
    Ok(false)
}

/// Replace generated artwork after a paid redraw (inside an open transaction).
pub async fn update_generated_artwork_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    user_id: u64,
    generated_artwork_path: &str,
    ai_model_id: &str,
    actor_user_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE user_faces
        SET generated_artwork_path = ?,
            ai_model_id = ?,
            face_captured_at = CURRENT_TIMESTAMP,
            modified_by = ?
        WHERE user_id = ?
        "#,
    )
    .bind(generated_artwork_path)
    .bind(ai_model_id)
    .bind(actor_user_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
