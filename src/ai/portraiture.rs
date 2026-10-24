//! Council-assisted avatar portraiture — face photos → stylized raster portrait.
//!
//! Onboarding uses Imagen subject customization (reference photos + Epic style/RAG prompt)
//! with billing waived. Paid redraw re-captures photos and debits Noms after successful raster.

use sqlx::MySqlPool;

use crate::ai::portrait_colour_card;
use crate::ai::portrait_raster::{self, PortraitRasterConfig, PORTRAIT_SAMPLE_BATCHES};
use crate::ai::registry as ai_registry;
use crate::config::Config;
use crate::error::AppError;
use crate::i18n;

use crate::social::face_capture_errors::{artist_at_handle, face_capture_coded_detail, face_capture_coded_detail_tf, is_portrait_infrastructure_error, truncate_client_detail};
use crate::pricing::BillableService;
use crate::social::noms::{self, SpendOutcome};
use crate::social::user_faces;
use crate::ui::nom_icon;

/// Per-Imagen-predict HTTP budget (3 samples per call).
const MIN_PORTRAIT_PREDICT_TIMEOUT_MS: u64 = 90_000;
const MAX_PORTRAIT_PREDICT_TIMEOUT_MS: u64 = 120_000;
/// Total gRPC wait for up to two sample batches plus postprocess.
const MAX_PORTRAIT_GRPC_TIMEOUT_MS: u64 = 300_000;
const PORTRAIT_GRPC_OVERHEAD_MS: u64 = 15_000;

/// Per-predict and total gRPC timeouts for multi-sample Imagen portrait generation.
#[must_use]
pub fn portrait_timeout_budget(configured_ms: u64) -> (u64, u64) {
    let predict_timeout_ms = configured_ms.clamp(
        MIN_PORTRAIT_PREDICT_TIMEOUT_MS,
        MAX_PORTRAIT_PREDICT_TIMEOUT_MS,
    );
    let grpc_timeout_ms = predict_timeout_ms
        .saturating_mul(u64::from(PORTRAIT_SAMPLE_BATCHES))
        .saturating_add(PORTRAIT_GRPC_OVERHEAD_MS)
        .min(MAX_PORTRAIT_GRPC_TIMEOUT_MS);
    (predict_timeout_ms, grpc_timeout_ms)
}

#[derive(Debug, Clone)]
pub struct PortraitureFacePhotos {
    pub neutral: Vec<u8>,
    pub smile: Vec<u8>,
    pub frown: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortraitureBilling {
    /// First onboarding capture — no Nom debit.
    OnboardingWaived,
    /// Paid redraw from fresh capture photos (`BillableService::CouncilAvatarRefresh`).
    AvatarRedraw,
}

#[derive(Debug)]
pub struct PortraitureOutput {
    /// Relative path to the stored generated avatar (the only durable artifact).
    pub generated_artwork_path: Option<String>,
    pub portrait_model_id: String,
    pub noms_spent: u16,
    pub noms_balance_after: u16,
}

#[derive(Debug)]
pub enum PortraitureError {
    PortraitRaster(String),
    InsufficientNoms { needed: u16, balance: u16 },
    MissingFacePhotos,
    MissingPortraitConfig(String),
    Db(sqlx::Error),
}

impl std::fmt::Display for PortraitureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PortraitureError::PortraitRaster(s) => write!(f, "{s}"),
            PortraitureError::InsufficientNoms { needed, balance } => {
                write!(f, "insufficient noms: need {needed}, have {balance}")
            }
            PortraitureError::MissingFacePhotos => {
                write!(f, "face photos required but not supplied")
            }
            PortraitureError::MissingPortraitConfig(s) => write!(f, "{s}"),
            PortraitureError::Db(e) => write!(f, "{e}"),
        }
    }
}

impl From<sqlx::Error> for PortraitureError {
    fn from(e: sqlx::Error) -> Self {
        PortraitureError::Db(e)
    }
}

pub struct PortraitureRequest<'a> {
    pub pool: &'a MySqlPool,
    pub config: &'a Config,
    pub user_id: u64,
    pub human_entity_id: u64,
    pub council_agent_handle: &'a str,
    pub portrait_cfg: Option<PortraitRasterConfig>,
    pub billing: PortraitureBilling,
    pub images: Option<PortraitureFacePhotos>,
}

pub fn portrait_raster_config(config: &Config) -> Result<PortraitRasterConfig, PortraitureError> {
    if config.ai_grpc_endpoint.trim().is_empty() {
        return Err(PortraitureError::MissingPortraitConfig(
            "AI_GRPC_ENDPOINT is required for generated face artwork (portrait raster via ai_gateway)"
                .to_string(),
        ));
    }
    let (predict_timeout_ms, grpc_timeout_ms) = portrait_timeout_budget(config.portrait_raster_timeout_ms);
    Ok(PortraitRasterConfig {
        grpc_endpoint: config.ai_grpc_endpoint.clone(),
        model_id: config.portrait_raster_model.clone(),
        grpc_timeout_ms,
        predict_timeout_ms,
        output_dir: config.generated_faces_dir.clone(),
    })
}

pub fn map_to_app_error(
    e: PortraitureError,
    loc: &str,
    billing: PortraitureBilling,
    artist_handle: &str,
) -> AppError {
    let c = i18n::catalog();
    match e {
        PortraitureError::InsufficientNoms { needed, balance } => AppError::BadRequest(
            nom_icon::insufficient_noms_plain(
                &c,
                loc,
                "noms.insufficient_for_avatar_redraw",
                needed,
                Some(balance),
            ),
        ),
        PortraitureError::MissingFacePhotos => {
            AppError::BadRequest(c.t(loc, "ui.face_capture.err_incomplete"))
        }
        PortraitureError::MissingPortraitConfig(msg) => AppError::BadRequest(msg),
        PortraitureError::PortraitRaster(msg) => {
            tracing::warn!(error = %msg, "portrait raster generation failed");
            if is_portrait_infrastructure_error(&msg) {
                face_capture_coded_detail(
                    loc,
                    &c,
                    "face_portrait_unavailable",
                    "ui.face_capture.pipeline_unavailable",
                    None,
                )
            } else if billing == PortraitureBilling::OnboardingWaived {
                let at = artist_at_handle(artist_handle);
                face_capture_coded_detail_tf(
                    loc,
                    &c,
                    "face_portrait_failed",
                    "ui.face_capture.portrait_failed",
                    &[("artist_handle", &at)],
                    Some(truncate_client_detail(msg)),
                )
            } else {
                AppError::BadRequest(c.tf(
                    loc,
                    "ui.entity_private.avatar_redraw_failed",
                    &[("detail", &msg)],
                ))
            }
        }
        PortraitureError::Db(err) => AppError::Db(err),
    }
}

async fn resolve_council_slug(
    pool: &MySqlPool,
    council_agent_handle: &str,
) -> Result<String, sqlx::Error> {
    let resolved = ai_registry::resolve_active_actor(pool, council_agent_handle).await?;
    Ok(resolved
        .map(|a| a.slug)
        .unwrap_or_else(|| council_agent_handle.to_string()))
}

/// Runs council avatar portraiture from fresh capture photos (photos are not stored).
pub async fn run_portraiture(
    req: PortraitureRequest<'_>,
) -> Result<PortraitureOutput, PortraitureError> {
    let council_slug = resolve_council_slug(req.pool, req.council_agent_handle).await?;

    let images = req
        .images
        .as_ref()
        .ok_or(PortraitureError::MissingFacePhotos)?;

    let portrait_cfg = req.portrait_cfg.as_ref().ok_or_else(|| {
        PortraitureError::MissingPortraitConfig(
            "portrait raster configuration missing".to_string(),
        )
    })?;

    if req.billing == PortraitureBilling::AvatarRedraw {
        let cost = BillableService::CouncilAvatarRefresh.nom_cost();
        let balance = noms::balance_for_human_entity(req.pool, req.human_entity_id).await?;
        if cost > 0 && balance < cost {
            return Err(PortraitureError::InsufficientNoms {
                needed: cost,
                balance,
            });
        }
    }

    let colour_card = portrait_colour_card::extract_portrait_colour_card(
        req.config,
        req.user_id,
        &images.neutral,
    )
    .await;

    let (png_bytes, portrait_model_id) = portrait_raster::generate_portrait_raster(
        portrait_cfg,
        req.user_id,
        images,
        &council_slug,
        colour_card.as_ref(),
    )
    .await
    .map_err(PortraitureError::PortraitRaster)?;

    let saved_rel_path =
        portrait_raster::write_generated_image(&portrait_cfg.output_dir, req.user_id, &png_bytes)
            .await
            .map_err(PortraitureError::PortraitRaster)?;

    let mut noms_spent = 0u16;
    let mut noms_balance_after =
        noms::balance_for_human_entity(req.pool, req.human_entity_id).await?;

    if req.billing == PortraitureBilling::AvatarRedraw {
        let mut tx = req.pool.begin().await?;
        let spend_ref = format!("avatar_redraw:user:{}", req.user_id);
        match noms::spend_for_service_tx(
            &mut tx,
            req.human_entity_id,
            BillableService::CouncilAvatarRefresh,
            req.user_id,
            Some(&spend_ref),
        )
        .await?
        {
            SpendOutcome::Spent { cost, balance } => {
                noms_spent = cost;
                noms_balance_after = balance;
            }
            SpendOutcome::InsufficientBalance { needed, balance } => {
                return Err(PortraitureError::InsufficientNoms { needed, balance });
            }
            SpendOutcome::NoCharge { balance } => {
                noms_balance_after = balance;
            }
        }

        user_faces::update_generated_artwork_tx(
            &mut tx,
            req.user_id,
            &saved_rel_path,
            &portrait_model_id,
            req.user_id,
        )
        .await?;

        tx.commit().await?;
    }

    Ok(PortraitureOutput {
        generated_artwork_path: Some(saved_rel_path),
        portrait_model_id,
        noms_spent,
        noms_balance_after,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WebSearchProvider;
    use std::collections::HashSet;
    use std::net::IpAddr;

    fn test_config(portrait_raster_timeout_ms: u64) -> Config {
        Config {
            database_url: "mysql://test/itest".to_string(),
            host: "127.0.0.1".parse::<IpAddr>().unwrap(),
            port: 8080,
            nominatim_user_agent: "256.app-tests".to_string(),
            secure_cookies: false,
            public_base_url: "http://127.0.0.1:8080".to_string(),
            dev_auth_bypass: true,
            trusted_proxy: false,
            smtp: None,
            face_vision_mock: true,
            skip_face_vision_validate: false,
            ai_grpc_endpoint: "http://127.0.0.1:50051".to_string(),
            ai_grpc_timeout_ms: 3000,
            verbose_ai_errors: true,
            portrait_raster_model: "imagen-3.0-capability-001".to_string(),
            portrait_raster_timeout_ms,
            generated_faces_dir: "static/generated/user-faces".to_string(),
            council_agent_handle: "epic".to_string(),
            council_invite_review_email: "test@example.com".to_string(),
            platform_recruitment_review_only: true,
            google_maps_api_key: None,
            google_places_timeout_ms: 12_000,
            ops_location_invalidate_token: None,
            ops_job_queue_token: None,
            request_timeout_secs: 60,
            scheduler_tick_secs: 0,
            scheduler_batch_size: 32,
            job_conductor_parallelism: 4,
            job_conductor_llm_parallelism: 2,
            feed_poll_wall_budget_secs: 120,
            ops_digest_live: true,
            journal_retention_days: 3,
            csrf_pepper: "test".to_string(),
            media_hash_blocklist: HashSet::new(),
            recaptcha: None,
            oidc_google: None,
            stripe: None,
            platform_safety_contact_email: "safety@example.com".to_string(),
            council_web_search_api_key: None,
            council_web_search_provider: WebSearchProvider::None,
            council_web_search_cx: None,
            council_web_search_timeout_ms: 8000,
        }
    }

    #[test]
    fn portrait_timeout_budget_raises_legacy_thirty_second_env() {
        let (predict, grpc) = portrait_timeout_budget(30_000);
        assert_eq!(predict, 90_000, "legacy 30s env must clamp up to 90s per predict");
        assert_eq!(
            grpc,
            90_000 * u64::from(PORTRAIT_SAMPLE_BATCHES) + PORTRAIT_GRPC_OVERHEAD_MS,
            "gRPC budget must cover two sample batches"
        );
    }

    #[test]
    fn portrait_timeout_budget_default_ninety_second_predict() {
        let (predict, grpc) = portrait_timeout_budget(90_000);
        assert_eq!(predict, 90_000);
        assert_eq!(grpc, 195_000);
    }

    #[test]
    fn portrait_timeout_budget_caps_per_predict_at_two_minutes() {
        let (predict, grpc) = portrait_timeout_budget(999_999);
        assert_eq!(predict, MAX_PORTRAIT_PREDICT_TIMEOUT_MS);
        assert_eq!(grpc, 255_000);
    }

    #[test]
    fn portrait_raster_config_requires_grpc_endpoint() {
        let mut cfg = test_config(90_000);
        cfg.ai_grpc_endpoint.clear();
        assert!(portrait_raster_config(&cfg).is_err());
    }

    #[test]
    fn portrait_raster_config_forwards_timeout_budget() {
        let cfg = portrait_raster_config(&test_config(90_000)).expect("config");
        assert_eq!(cfg.predict_timeout_ms, 90_000);
        assert_eq!(cfg.grpc_timeout_ms, 195_000);
    }
}
