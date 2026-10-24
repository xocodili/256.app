//! Per-IP rate limits (via [`tower_governor`]).
//!
//! Uses [`SmartIpKeyExtractor`] so `X-Forwarded-For` / `Forwarded` work behind a **trusted** proxy.
//! Requires [`axum::serve`] with [`IntoMakeServiceWithConnectInfo`] so direct clients still expose peer IP.
//!
//! UI card surfaces use [`SessionOrIpKeyExtractor`] so multiple signed-in browsers on one machine
//! (same IP, different session cookies) do not share one quota.

use std::num::NonZeroU32;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use axum::body::Body;
use axum::http::Request;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use governor::clock::{Clock, DefaultClock};
use governor::middleware::NoOpMiddleware;
use governor::{Quota, RateLimiter};
use tower_governor::governor::{GovernorConfig, GovernorConfigBuilder};
use tower_governor::key_extractor::{KeyExtractor, SmartIpKeyExtractor};
use tower_governor::{GovernorError, GovernorLayer};

use crate::auth::csrf;
use crate::error::AppError;
use crate::i18n;

/// Rate-limit signed-in card traffic per session; fall back to client IP for guests.
#[derive(Clone, Copy, Debug)]
pub struct SessionOrIpKeyExtractor;

impl KeyExtractor for SessionOrIpKeyExtractor {
    type Key = String;

    fn name(&self) -> &'static str {
        "session or IP"
    }

    fn extract<B>(&self, req: &Request<B>) -> Result<Self::Key, GovernorError> {
        if let Some(token) = csrf::session_token_from_headers(req.headers()) {
            return Ok(format!("sid:{token}"));
        }
        SmartIpKeyExtractor
            .extract(req)
            .map(|ip| format!("ip:{ip}"))
    }
}

fn smart_ip_config(
    period: Duration,
    burst: u32,
) -> Arc<GovernorConfig<SmartIpKeyExtractor, NoOpMiddleware>> {
    let mut b = GovernorConfigBuilder::default()
        .const_period(period)
        .const_burst_size(burst);
    let mut b2 = b.key_extractor(SmartIpKeyExtractor);
    Arc::new(b2.finish().expect("non-zero period and burst"))
}

fn session_or_ip_config(
    period: Duration,
    burst: u32,
) -> Arc<GovernorConfig<SessionOrIpKeyExtractor, NoOpMiddleware>> {
    let mut b = GovernorConfigBuilder::default()
        .const_period(period)
        .const_burst_size(burst);
    let mut b2 = b.key_extractor(SessionOrIpKeyExtractor);
    Arc::new(b2.finish().expect("non-zero period and burst"))
}

fn json_rate_limit_error(err: GovernorError) -> axum::response::Response {
    let c = i18n::catalog();
    let loc = i18n::request_locale();
    match err {
        GovernorError::TooManyRequests { wait_time, headers } => {
            let mut res = (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({
                    "error": c.t(&loc, "rate_limit.too_many_requests"),
                    "code": "rate_limited",
                    "retry_after_secs": wait_time,
                })),
            )
                .into_response();
            if let Some(h) = headers {
                res.headers_mut().extend(h);
            }
            res
        }
        GovernorError::UnableToExtractKey => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": c.t(&loc, "rate_limit.key_error"),
                "code": "rate_limit_key_error",
            })),
        )
            .into_response(),
        GovernorError::Other { code, msg, headers } => {
            let mut res = (
                code,
                Json(serde_json::json!({
                    "error": msg.unwrap_or_else(|| c.t(&loc, "rate_limit.generic")),
                    "code": "rate_limited",
                })),
            )
                .into_response();
            if let Some(h) = headers {
                res.headers_mut().extend(h);
            }
            res
        }
    }
}

pub fn limit_layer(
    cfg: Arc<GovernorConfig<SmartIpKeyExtractor, NoOpMiddleware>>,
) -> GovernorLayer<SmartIpKeyExtractor, NoOpMiddleware, Body> {
    GovernorLayer::new(cfg).error_handler(json_rate_limit_error)
}

pub fn ui_card_limit_layer(
    cfg: Arc<GovernorConfig<SessionOrIpKeyExtractor, NoOpMiddleware>>,
) -> GovernorLayer<SessionOrIpKeyExtractor, NoOpMiddleware, Body> {
    GovernorLayer::new(cfg).error_handler(json_rate_limit_error)
}

/// Create account: tight cap to slow throwaway signups.
pub fn register_layer() -> GovernorLayer<SmartIpKeyExtractor, NoOpMiddleware, Body> {
    limit_layer(smart_ip_config(Duration::from_secs(600), 5))
}

/// Shared limiter config for login + resend (same bucket when wrapped with [`limit_layer`] clones).
pub fn login_resend_config() -> Arc<GovernorConfig<SmartIpKeyExtractor, NoOpMiddleware>> {
    smart_ip_config(Duration::from_secs(2), 25)
}

/// Email verification link hits (GET, often prefetched).
pub fn verify_email_layer() -> GovernorLayer<SmartIpKeyExtractor, NoOpMiddleware, Body> {
    limit_layer(smart_ip_config(Duration::from_secs(1), 40))
}

/// Creating validated locations (POST) — moderate cost, privacy-sensitive payload.
pub fn locations_create_layer() -> GovernorLayer<SmartIpKeyExtractor, NoOpMiddleware, Body> {
    limit_layer(smart_ip_config(Duration::from_secs(4), 12))
}

/// Listing reference key values (GET) — cacheable but limit scraping.
pub fn ref_key_values_list_layer() -> GovernorLayer<SmartIpKeyExtractor, NoOpMiddleware, Body> {
    limit_layer(smart_ip_config(Duration::from_secs(60), 120))
}

/// Creating a social entity (POST) — validates refs + location.
pub fn social_entity_create_layer() -> GovernorLayer<SmartIpKeyExtractor, NoOpMiddleware, Body> {
    limit_layer(smart_ip_config(Duration::from_secs(4), 15))
}

/// Nominatim-backed reverse geocode (POST) — keep well under public rate limits.
pub fn reverse_geocode_layer() -> GovernorLayer<SmartIpKeyExtractor, NoOpMiddleware, Body> {
    limit_layer(smart_ip_config(Duration::from_secs(60), 20))
}

/// Google Places Details (POST) — billable upstream; moderate per-IP cap.
pub fn google_places_confirm_layer() -> GovernorLayer<SmartIpKeyExtractor, NoOpMiddleware, Body> {
    limit_layer(smart_ip_config(Duration::from_secs(120), 24))
}

/// Ops bearer endpoints (POST) — slow brute force on shared secrets.
pub fn ops_invalidate_layer() -> GovernorLayer<SmartIpKeyExtractor, NoOpMiddleware, Body> {
    limit_layer(smart_ip_config(Duration::from_secs(3600), 40))
}

/// Job queue conductor (`POST /ops/jobs/run`) — same class as other ops bearer routes.
pub fn ops_jobs_run_layer() -> GovernorLayer<SmartIpKeyExtractor, NoOpMiddleware, Body> {
    limit_layer(smart_ip_config(Duration::from_secs(3600), 40))
}

/// User job enqueue / cancel — moderate per-IP cap.
pub fn jobs_enqueue_layer() -> GovernorLayer<SmartIpKeyExtractor, NoOpMiddleware, Body> {
    limit_layer(smart_ip_config(Duration::from_secs(60), 60))
}

/// Face capture + avatar redraw: per-user cap on **Vertex-bound attempts** (vision + portrait),
/// checked in [`try_consume_face_capture_vertex_attempt`] after cheap local gates pass.
/// Not a global Vertex quota — council AI and other gRPC paths use separate limits upstream.
pub fn try_consume_face_capture_vertex_attempt(user_id: u64) -> Result<(), AppError> {
    let limiter = face_capture_vertex_limiter();
    match limiter.check_key(&user_id) {
        Ok(()) => Ok(()),
        Err(not_until) => {
            let wait = not_until
                .wait_time_from(DefaultClock::default().now())
                .as_secs()
                .max(1);
            let c = i18n::catalog();
            let loc = i18n::request_locale();
            Err(AppError::TooManyRequests {
                message: c.tf(
                    &loc,
                    "ui.face_capture.rate_limited_retry",
                    &[("seconds", &wait.to_string())],
                ),
                retry_after_secs: wait,
            })
        }
    }
}

type FaceCaptureVertexLimiter =
    RateLimiter<u64, governor::state::keyed::DashMapStateStore<u64>, DefaultClock>;

fn face_capture_vertex_limiter() -> &'static FaceCaptureVertexLimiter {
    static LIMITER: OnceLock<FaceCaptureVertexLimiter> = OnceLock::new();
    LIMITER.get_or_init(|| {
        let dev_bypass = std::env::var("DEV_AUTH_BYPASS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let quota = if dev_bypass {
            // Local onboarding retests: generous per-user budget.
            Quota::with_period(Duration::from_secs(60))
                .expect("non-zero period")
                .allow_burst(NonZeroU32::new(30).expect("non-zero burst"))
        } else {
            // Production: three full portrait pipelines (vision + Imagen) per 10 minutes per user.
            Quota::with_period(Duration::from_secs(200))
                .expect("non-zero period")
                .allow_burst(NonZeroU32::new(3).expect("non-zero burst"))
        };
        RateLimiter::dashmap(quota)
    })
}

/// Creating a social entity relationship (POST).
pub fn social_entity_relationship_create_layer(
) -> GovernorLayer<SmartIpKeyExtractor, NoOpMiddleware, Body> {
    limit_layer(smart_ip_config(Duration::from_secs(4), 20))
}

/// Public entity profile surface (`GET /public/{handle}` shell, `GET /api/public/entity-profile/{handle}` JSON) — limit scraping.
pub fn social_entity_public_profile_layer(
) -> GovernorLayer<SmartIpKeyExtractor, NoOpMiddleware, Body> {
    limit_layer(smart_ip_config(Duration::from_secs(60), 180))
}

/// Shared quota for **HTML card surfaces** (login shell + fragments, public profile shells, signed-in home).
/// Keys by session cookie when present, else client IP — so multiple local browsers do not share one bucket.
pub fn ui_card_surface_config(
    dev_auth_bypass: bool,
) -> Arc<GovernorConfig<SessionOrIpKeyExtractor, NoOpMiddleware>> {
    static NORMAL: OnceLock<Arc<GovernorConfig<SessionOrIpKeyExtractor, NoOpMiddleware>>> =
        OnceLock::new();
    static DEV: OnceLock<Arc<GovernorConfig<SessionOrIpKeyExtractor, NoOpMiddleware>>> =
        OnceLock::new();
    let generous = dev_auth_bypass || cfg!(debug_assertions);
    if generous {
        DEV.get_or_init(|| session_or_ip_config(Duration::from_secs(60), 10_000))
            .clone()
    } else {
        NORMAL.get_or_init(|| session_or_ip_config(Duration::from_secs(60), 120))
            .clone()
    }
}
