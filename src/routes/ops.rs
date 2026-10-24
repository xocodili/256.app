//! Internal maintenance HTTP (bearer token). Not for end-user browsers.

use axum::extract::{Json, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::Router;
use serde::Deserialize;

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::i18n;
use crate::jobs;
use crate::locations::lifecycle::{invalidate_location, LocationInvalidationKind};
use crate::rate_limit;
use crate::social::ids;

#[derive(Debug, Deserialize)]
pub struct OpsJobsRunBody {
    /// Cap on claimed jobs this tick (default 32, max 256).
    #[serde(default)]
    pub batch_size: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct OpsInvalidateLocationBody {
    pub location_id: String,
    /// `deprecated` or `obsolete`
    pub kind: String,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub superseded_by_location_id: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/ops/locations/invalidate",
            post(ops_invalidate_location).layer(rate_limit::ops_invalidate_layer()),
        )
        .route(
            "/ops/jobs/run",
            post(ops_jobs_run).layer(rate_limit::ops_jobs_run_layer()),
        )
}

async fn ops_jobs_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<OpsJobsRunBody>,
) -> AppResult<impl IntoResponse> {
    let Some(expected) = state.config.ops_job_queue_token.as_deref() else {
        return Err(AppError::NotFound);
    };

    let auth = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let token = auth
        .strip_prefix("Bearer ")
        .unwrap_or("")
        .trim();
    if token != expected {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let batch = body.batch_size.unwrap_or(32).clamp(1, 256);
    let summary = jobs::run_conductor_tick(&state.db, &state.config, batch).await?;
    Ok(Json(summary).into_response())
}

async fn ops_invalidate_location(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<OpsInvalidateLocationBody>,
) -> AppResult<StatusCode> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();

    let Some(expected) = state.config.ops_location_invalidate_token.as_deref() else {
        return Err(AppError::NotFound);
    };

    let auth = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let token = auth
        .strip_prefix("Bearer ")
        .unwrap_or("")
        .trim();
    if token != expected {
        return Ok(StatusCode::UNAUTHORIZED);
    }

    let location_id = ids::parse_u64(
        &body.location_id,
        "location_id",
        "social_entity.invalid_id",
    )?;

    let kind = match body.kind.trim() {
        "deprecated" => LocationInvalidationKind::Deprecated,
        "obsolete" => LocationInvalidationKind::Obsolete,
        _ => {
            return Err(AppError::BadRequest(
                c.t(&loc, "ops.invalidate_location_kind_invalid"),
            ));
        }
    };

    let superseded = match body.superseded_by_location_id.as_deref() {
        None | Some("") => None,
        Some(s) => Some(ids::parse_u64(
            s,
            "superseded_by_location_id",
            "social_entity.invalid_id",
        )?),
    };

    let note = body.note.as_deref();
    invalidate_location(
        &state.db,
        location_id,
        kind,
        note,
        superseded,
        None,
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
