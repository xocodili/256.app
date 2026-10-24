//! Async face-capture pipeline: stage JPEGs on disk, enqueue vision + portrait work via job queue.

use std::path::{Path, PathBuf};

use chrono::{Duration, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::MySqlPool;
use uuid::Uuid;

use crate::ai::grpc::MOCK_AI_MODEL_ID;
use crate::ai::portraiture::{
    self, PortraitureBilling, PortraitureFacePhotos, PortraitureRequest,
};
use crate::config::Config;
use crate::error::AppError;
use crate::i18n;
use crate::pricing::BillableService;
use crate::rate_limit;
use crate::social::face_capture_validate;
use crate::social::noms;
use crate::social::user_faces;
use crate::status;

pub const JOB_FACE_PORTRAIT_PIPELINE: &str = "face_portrait_pipeline";
pub const JOB_FACE_CAPTURE_PENDING_CLEANUP: &str = "face_capture_pending_cleanup";

const PENDING_TTL_MINUTES: i64 = 15;
const DEFAULT_PENDING_DIR: &str = "tmp/face-capture-pending";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingBillingMode {
    Onboarding,
    AvatarRedraw,
}

impl PendingBillingMode {
    fn as_db_str(self) -> &'static str {
        match self {
            PendingBillingMode::Onboarding => "onboarding",
            PendingBillingMode::AvatarRedraw => "avatar_redraw",
        }
    }

    fn portraiture_billing(self) -> PortraitureBilling {
        match self {
            PendingBillingMode::Onboarding => PortraitureBilling::OnboardingWaived,
            PendingBillingMode::AvatarRedraw => PortraitureBilling::AvatarRedraw,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingStatus {
    Queued,
    Processing,
    Complete,
    Failed,
}

impl PendingStatus {
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "queued" => Some(PendingStatus::Queued),
            "processing" => Some(PendingStatus::Processing),
            "complete" => Some(PendingStatus::Complete),
            "failed" => Some(PendingStatus::Failed),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            PendingStatus::Queued => "queued",
            PendingStatus::Processing => "processing",
            PendingStatus::Complete => "complete",
            PendingStatus::Failed => "failed",
        }
    }
}

/// Client-visible pipeline step (`GET /me/face-capture/status/{pending_id}` → `stage`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStage {
    Queued,
    Validating,
    Painting,
}

impl PipelineStage {
    pub fn as_str(self) -> &'static str {
        match self {
            PipelineStage::Queued => "queued",
            PipelineStage::Validating => "validating",
            PipelineStage::Painting => "painting",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct QueueFaceCaptureResult {
    pub pending_id: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct FaceCaptureStatusSnapshot {
    pub pending_id: String,
    pub status: String,
    pub stage: Option<String>,
    pub error: Option<String>,
    pub code: Option<String>,
    pub id: Option<u64>,
    pub avatar_path: Option<String>,
    pub noms_spent: Option<u16>,
    pub noms_balance: Option<u16>,
}

#[derive(Debug)]
struct PendingRow {
    id: String,
    user_id: u64,
    human_entity_id: Option<u64>,
    billing_mode: PendingBillingMode,
    status: PendingStatus,
    quality_score: Option<f32>,
    user_face_id: Option<u64>,
    generated_artwork_path: Option<String>,
    portrait_model_id: Option<String>,
    noms_spent: Option<u16>,
    noms_balance_after: Option<u16>,
    error_code: Option<String>,
    error_message: Option<String>,
    pipeline_stage: Option<String>,
}

pub fn pending_storage_root(_config: &Config) -> PathBuf {
    PathBuf::from(
        std::env::var("FACE_CAPTURE_PENDING_DIR")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_PENDING_DIR.to_string()),
    )
}

fn pending_dir(root: &Path, pending_id: &str) -> PathBuf {
    root.join(pending_id)
}

fn is_valid_pending_id(pending_id: &str) -> bool {
    Uuid::parse_str(pending_id).is_ok()
}

pub async fn user_has_in_flight_pending(pool: &MySqlPool, user_id: u64) -> Result<bool, sqlx::Error> {
    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM face_capture_pending
            WHERE user_id = ?
              AND status IN ('queued', 'processing')
              AND expires_at > UTC_TIMESTAMP(6)
        )
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

pub async fn queue_face_capture(
    pool: &MySqlPool,
    config: &Config,
    user_id: u64,
    human_entity_id: Option<u64>,
    billing: PendingBillingMode,
    photos: &PortraitureFacePhotos,
    quality_score: Option<f32>,
) -> Result<QueueFaceCaptureResult, AppError> {
    if user_has_in_flight_pending(pool, user_id).await? {
        let loc = i18n::request_locale();
        let c = i18n::catalog();
        return Err(AppError::Conflict(c.t(&loc, "ui.face_capture.pipeline_in_flight")));
    }

    if billing == PendingBillingMode::AvatarRedraw {
        let human_entity_id = human_entity_id.ok_or_else(|| {
            let loc = i18n::request_locale();
            let c = i18n::catalog();
            AppError::BadRequest(c.t(&loc, "ui.entity_private.human_profile_required"))
        })?;
        let cost = BillableService::CouncilAvatarRefresh.nom_cost();
        let balance = noms::balance_for_human_entity(pool, human_entity_id)
            .await
            .map_err(AppError::Db)?;
        if cost > 0 && balance < cost {
            let loc = i18n::request_locale();
            let c = i18n::catalog();
            return Err(AppError::BadRequest(crate::ui::nom_icon::insufficient_noms_plain(
                &c,
                &loc,
                "noms.insufficient_for_avatar_redraw",
                cost,
                Some(balance),
            )));
        }
    }

    let pending_id = Uuid::new_v4().to_string();
    let expires_at = (Utc::now() + Duration::minutes(PENDING_TTL_MINUTES)).naive_utc();
    let root = pending_storage_root(config);
    let dir = pending_dir(&root, &pending_id);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| AppError::BadRequest(format!("face capture staging dir: {e}")))?;

    write_jpeg(&dir.join("neutral.jpg"), &photos.neutral).await?;
    write_jpeg(&dir.join("smile.jpg"), &photos.smile).await?;
    write_jpeg(&dir.join("frown.jpg"), &photos.frown).await?;

    sqlx::query(
        r#"
        INSERT INTO face_capture_pending (
            id, user_id, human_entity_id, billing_mode, status, pipeline_stage, quality_score, expires_at
        ) VALUES (?, ?, ?, ?, 'queued', 'queued', ?, ?)
        "#,
    )
    .bind(&pending_id)
    .bind(user_id)
    .bind(human_entity_id)
    .bind(billing.as_db_str())
    .bind(quality_score)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(AppError::Db)?;

    let payload = serde_json::json!({ "pending_id": pending_id });
    let enqueue = crate::jobs::queue::enqueue_job(
        pool,
        Some(user_id),
        JOB_FACE_PORTRAIT_PIPELINE,
        &payload,
        Some(&pending_id),
        5,
    )
    .await
    .map_err(AppError::Db)?;

    if enqueue.job_id == 0 {
        let _ = remove_pending_dir(&dir).await;
        let _ = mark_failed(pool, &pending_id, "pipeline_unavailable", "job type not registered")
            .await;
        let loc = i18n::request_locale();
        let c = i18n::catalog();
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.face_capture.pipeline_unavailable"),
        ));
    }

    // Kick the queue without blocking the 202 — the handler can run for minutes (vision + Imagen).
    let kick_pool = pool.clone();
    let kick_config = config.clone();
    let kick_pending_id = pending_id.clone();
    tokio::spawn(async move {
        if let Err(err) =
            crate::jobs::queue::run_conductor_tick(&kick_pool, &kick_config, 2).await
        {
            tracing::warn!(
                error = %err,
                pending_id = %kick_pending_id,
                "face capture post-enqueue conductor tick failed"
            );
        }
    });

    Ok(QueueFaceCaptureResult {
        pending_id,
        status: PendingStatus::Queued.as_str().to_string(),
    })
}

pub async fn status_for_user(
    pool: &MySqlPool,
    user_id: u64,
    pending_id: &str,
) -> Result<Option<FaceCaptureStatusSnapshot>, sqlx::Error> {
    if !is_valid_pending_id(pending_id) {
        return Ok(None);
    }
    let row = load_pending_row(pool, pending_id).await?;
    let Some(row) = row else {
        return Ok(None);
    };
    if row.user_id != user_id {
        return Ok(None);
    }
    Ok(Some(row_to_snapshot(row)))
}

pub async fn run_face_portrait_pipeline_job(
    pool: &MySqlPool,
    config: &Config,
    payload: &Value,
) -> Result<(), String> {
    let pending_id = payload
        .get("pending_id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty() && is_valid_pending_id(s))
        .ok_or_else(|| "missing pending_id".to_string())?;

    let row = load_pending_row(pool, pending_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("pending row not found: {pending_id}"))?;

    if row.status == PendingStatus::Complete {
        return Ok(());
    }
    if row.status == PendingStatus::Failed {
        return Ok(());
    }

    mark_processing(pool, pending_id)
        .await
        .map_err(|e| e.to_string())?;

    if !config.skip_face_vision_validate {
        update_pipeline_stage(pool, pending_id, PipelineStage::Validating)
            .await
            .map_err(|e| e.to_string())?;
    }

    let root = pending_storage_root(config);
    let dir = pending_dir(&root, pending_id);
    let photos = match load_photos_from_disk(&dir).await {
        Ok(p) => p,
        Err(msg) => {
            mark_failed(pool, pending_id, "staging_missing", &msg)
                .await
                .map_err(|e| e.to_string())?;
            let _ = remove_pending_dir(&dir).await;
            return Ok(());
        }
    };

    let loc = "en";
    let c = i18n::catalog();

    if config.face_vision_mock {
        return run_mock_pipeline(pool, config, pending_id, &row, photos, &dir).await;
    }

    if config.skip_face_vision_validate {
        tracing::warn!(
            user_id = row.user_id,
            pending_id,
            "SKIP_FACE_VISION_VALIDATE=1 — bypassing async face vision gate (local dev only)"
        );
    } else if let Err(e) = face_capture_validate::validate_face_capture_photos(
        config,
        row.user_id,
        &photos,
        loc,
        &c,
    )
    .await
    {
        let (code, message) = app_error_to_pipeline_failure(&e);
        mark_failed(pool, pending_id, &code, &message)
            .await
            .map_err(|e| e.to_string())?;
        let _ = remove_pending_dir(&dir).await;
        return Ok(());
    }

    if let Err(e) = rate_limit::try_consume_face_capture_vertex_attempt(row.user_id) {
        let msg = e.to_string();
        mark_failed(pool, pending_id, "face_capture_rate_limited", &msg)
            .await
            .map_err(|e| e.to_string())?;
        let _ = remove_pending_dir(&dir).await;
        return Ok(());
    }

    update_pipeline_stage(pool, pending_id, PipelineStage::Painting)
        .await
        .map_err(|e| e.to_string())?;

    let human_entity_id = row.human_entity_id.unwrap_or(0);
    let portrait_cfg = match portraiture::portrait_raster_config(config) {
        Ok(cfg) => cfg,
        Err(e) => {
            mark_failed(pool, pending_id, "face_portrait_config", &e.to_string())
                .await
                .map_err(|e| e.to_string())?;
            let _ = remove_pending_dir(&dir).await;
            return Ok(());
        }
    };

    update_pipeline_stage(pool, pending_id, PipelineStage::Painting)
        .await
        .map_err(|e| e.to_string())?;

    let output = match portraiture::run_portraiture(PortraitureRequest {
        pool,
        config,
        user_id: row.user_id,
        human_entity_id,
        council_agent_handle: config.council_agent_handle.as_str(),
        portrait_cfg: Some(portrait_cfg),
        billing: row.billing_mode.portraiture_billing(),
        images: Some(photos.clone()),
    })
    .await
    {
        Ok(o) => o,
        Err(e) => {
            let (code, message) = portraiture_error_to_pipeline_failure(
                e,
                row.billing_mode.portraiture_billing(),
            );
            mark_failed(pool, pending_id, &code, &message)
                .await
                .map_err(|e| e.to_string())?;
            let _ = remove_pending_dir(&dir).await;
            return Ok(());
        }
    };

    complete_pipeline(pool, pending_id, &row, output, &dir).await
}

async fn run_mock_pipeline(
    pool: &MySqlPool,
    config: &Config,
    pending_id: &str,
    row: &PendingRow,
    _photos: PortraitureFacePhotos,
    dir: &Path,
) -> Result<(), String> {
    if row.billing_mode == PendingBillingMode::AvatarRedraw {
        mark_failed(
            pool,
            pending_id,
            "avatar_redraw_mock",
            "avatar redraw unavailable in mock mode",
        )
        .await
        .map_err(|e| e.to_string())?;
        let _ = remove_pending_dir(dir).await;
        return Ok(());
    }

    let user_face_id = user_faces::insert_face_capture(
        pool,
        row.user_id,
        None,
        MOCK_AI_MODEL_ID,
        row.quality_score,
        row.user_id,
    )
    .await
    .map_err(|e| e.to_string())?;

    status::try_promote_user_to_verified_if_composite(pool, row.user_id, Some(row.user_id))
        .await
        .map_err(|e| e.to_string())?;

    mark_complete(
        pool,
        pending_id,
        Some(user_face_id),
        None,
        Some(MOCK_AI_MODEL_ID.to_string()),
        0,
        None,
    )
    .await
    .map_err(|e| e.to_string())?;
    let _ = remove_pending_dir(dir).await;
    let _ = config; // mock path ignores portrait config
    Ok(())
}

async fn complete_pipeline(
    pool: &MySqlPool,
    pending_id: &str,
    row: &PendingRow,
    output: portraiture::PortraitureOutput,
    dir: &Path,
) -> Result<(), String> {
    let mut user_face_id = None;
    if row.billing_mode == PendingBillingMode::Onboarding {
        let id = user_faces::insert_face_capture(
            pool,
            row.user_id,
            output.generated_artwork_path.as_deref(),
            &output.portrait_model_id,
            row.quality_score,
            row.user_id,
        )
        .await
        .map_err(|e| e.to_string())?;
        user_face_id = Some(id);
        status::try_promote_user_to_verified_if_composite(pool, row.user_id, Some(row.user_id))
            .await
            .map_err(|e| e.to_string())?;
    }

    mark_complete(
        pool,
        pending_id,
        user_face_id,
        output.generated_artwork_path.clone(),
        Some(output.portrait_model_id),
        output.noms_spent,
        Some(output.noms_balance_after),
    )
    .await
    .map_err(|e| e.to_string())?;
    let _ = remove_pending_dir(dir).await;
    Ok(())
}

pub async fn run_face_capture_pending_cleanup_job(pool: &MySqlPool, config: &Config) -> Result<(), String> {
    let root = pending_storage_root(config);
    let expired: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT id FROM face_capture_pending
        WHERE expires_at <= UTC_TIMESTAMP(6)
           OR (status IN ('complete', 'failed') AND updated_at <= UTC_TIMESTAMP(6) - INTERVAL 1 HOUR)
        LIMIT 256
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    for pending_id in expired {
        let _ = remove_pending_dir(&pending_dir(&root, &pending_id)).await;
        sqlx::query("DELETE FROM face_capture_pending WHERE id = ?")
            .bind(&pending_id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn write_jpeg(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    tokio::fs::write(path, bytes)
        .await
        .map_err(|e| AppError::BadRequest(format!("write staged jpeg {}: {e}", path.display())))
}

async fn load_photos_from_disk(dir: &Path) -> Result<PortraitureFacePhotos, String> {
    let neutral = read_jpeg(dir.join("neutral.jpg")).await?;
    let smile = read_jpeg(dir.join("smile.jpg")).await?;
    let frown = read_jpeg(dir.join("frown.jpg")).await?;
    Ok(PortraitureFacePhotos {
        neutral,
        smile,
        frown,
    })
}

async fn read_jpeg(path: PathBuf) -> Result<Vec<u8>, String> {
    tokio::fs::read(&path)
        .await
        .map_err(|e| format!("read {}: {e}", path.display()))
}

async fn remove_pending_dir(dir: &Path) -> Result<(), String> {
    if dir.exists() {
        tokio::fs::remove_dir_all(dir)
            .await
            .map_err(|e| format!("remove {}: {e}", dir.display()))?;
    }
    Ok(())
}

async fn load_pending_row(pool: &MySqlPool, pending_id: &str) -> Result<Option<PendingRow>, sqlx::Error> {
    let raw: Option<(
        String,
        u64,
        Option<u64>,
        String,
        String,
        Option<f32>,
        Option<u64>,
        Option<String>,
        Option<String>,
        Option<u16>,
        Option<u16>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        r#"
        SELECT
            id,
            user_id,
            human_entity_id,
            billing_mode,
            status,
            quality_score,
            user_face_id,
            generated_artwork_path,
            portrait_model_id,
            noms_spent,
            noms_balance_after,
            error_code,
            error_message,
            pipeline_stage
        FROM face_capture_pending
        WHERE id = ?
        "#,
    )
    .bind(pending_id)
    .fetch_optional(pool)
    .await?;

    Ok(raw.and_then(
        |(
            id,
            user_id,
            human_entity_id,
            billing_raw,
            status_raw,
            quality_score,
            user_face_id,
            generated_artwork_path,
            portrait_model_id,
            noms_spent,
            noms_balance_after,
            error_code,
            error_message,
            pipeline_stage,
        )| {
            let billing_mode = match billing_raw.as_str() {
                "onboarding" => PendingBillingMode::Onboarding,
                "avatar_redraw" => PendingBillingMode::AvatarRedraw,
                _ => return None,
            };
            let status = PendingStatus::parse(&status_raw)?;
            Some(PendingRow {
                id,
                user_id,
                human_entity_id,
                billing_mode,
                status,
                quality_score,
                user_face_id,
                generated_artwork_path,
                portrait_model_id,
                noms_spent,
                noms_balance_after,
                error_code,
                error_message,
                pipeline_stage,
            })
        },
    ))
}

fn row_to_snapshot(row: PendingRow) -> FaceCaptureStatusSnapshot {
    let avatar_path = row.generated_artwork_path.map(|path| {
        user_faces::artwork_path_with_cache_bust(&path, chrono::Utc::now().timestamp())
    });
    FaceCaptureStatusSnapshot {
        pending_id: row.id,
        status: row.status.as_str().to_string(),
        stage: row.pipeline_stage,
        error: row.error_message,
        code: row.error_code,
        id: row.user_face_id,
        avatar_path,
        noms_spent: row.noms_spent,
        noms_balance: row.noms_balance_after,
    }
}

async fn mark_processing(pool: &MySqlPool, pending_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"UPDATE face_capture_pending SET status = 'processing' WHERE id = ? AND status IN ('queued', 'processing')"#,
    )
    .bind(pending_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn update_pipeline_stage(
    pool: &MySqlPool,
    pending_id: &str,
    stage: PipelineStage,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"UPDATE face_capture_pending SET pipeline_stage = ? WHERE id = ?"#,
    )
    .bind(stage.as_str())
    .bind(pending_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn mark_complete(
    pool: &MySqlPool,
    pending_id: &str,
    user_face_id: Option<u64>,
    generated_artwork_path: Option<String>,
    portrait_model_id: Option<String>,
    noms_spent: u16,
    noms_balance_after: Option<u16>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE face_capture_pending
        SET status = 'complete',
            user_face_id = ?,
            generated_artwork_path = ?,
            portrait_model_id = ?,
            noms_spent = ?,
            noms_balance_after = ?
        WHERE id = ?
        "#,
    )
    .bind(user_face_id)
    .bind(generated_artwork_path)
    .bind(portrait_model_id)
    .bind(if noms_spent > 0 { Some(noms_spent) } else { None })
    .bind(noms_balance_after)
    .bind(pending_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn mark_failed(
    pool: &MySqlPool,
    pending_id: &str,
    error_code: &str,
    error_message: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE face_capture_pending
        SET status = 'failed', error_code = ?, error_message = ?
        WHERE id = ?
        "#,
    )
    .bind(error_code)
    .bind(error_message.chars().take(512).collect::<String>())
    .bind(pending_id)
    .execute(pool)
    .await?;
    Ok(())
}

fn app_error_to_pipeline_failure(err: &AppError) -> (String, String) {
    match err {
        AppError::BadRequestCoded {
            message,
            code,
            detail,
        } => {
            let message = match detail {
                Some(d) if !d.trim().is_empty() => format!("{message} ({d})"),
                _ => message.clone(),
            };
            (code.clone(), message)
        }
        AppError::BadRequest(msg) => ("face_capture_rejected".to_string(), msg.clone()),
        AppError::TooManyRequests { message, .. } => {
            ("face_capture_rate_limited".to_string(), message.clone())
        }
        other => ("face_capture_failed".to_string(), other.to_string()),
    }
}

fn portraiture_error_to_pipeline_failure(
    err: portraiture::PortraitureError,
    billing: PortraitureBilling,
) -> (String, String) {
    match err {
        portraiture::PortraitureError::InsufficientNoms { needed, balance } => (
            "insufficient_noms".to_string(),
            format!("need {needed}, have {balance}"),
        ),
        portraiture::PortraitureError::PortraitRaster(msg) => {
            if crate::social::face_capture_errors::is_portrait_infrastructure_error(&msg) {
                (
                    "face_portrait_unavailable".to_string(),
                    msg,
                )
            } else if billing == PortraitureBilling::OnboardingWaived {
                ("face_portrait_failed".to_string(), msg)
            } else {
                ("avatar_redraw_failed".to_string(), msg)
            }
        }
        portraiture::PortraitureError::MissingPortraitConfig(msg) => {
            ("face_portrait_config".to_string(), msg)
        }
        other => ("face_capture_failed".to_string(), other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_id_validation_accepts_uuid_v4() {
        assert!(is_valid_pending_id("550e8400-e29b-41d4-a716-446655440000"));
        assert!(!is_valid_pending_id("../etc/passwd"));
    }
}
