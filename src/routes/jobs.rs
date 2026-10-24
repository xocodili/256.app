//! User job API: enqueue (dedupe by canonical payload hash), cancel pending.

use axum::extract::{Extension, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::i18n;
use crate::jobs;
use crate::middleware::VerifiedSession;
use crate::rate_limit;
use crate::social::ids;

#[derive(Debug, Deserialize)]
pub struct EnqueueBody {
    /// `ref_key_values.key` under group `ops_job_type`.
    pub job_type_key: String,
    pub payload: serde_json::Value,
    #[serde(default)]
    pub priority: Option<i16>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/jobs/enqueue", post(enqueue))
        .route("/jobs/{id}/cancel", post(cancel))
        .route("/jobs/run-now", post(run_now))
        .layer(rate_limit::jobs_enqueue_layer())
}

fn correlation_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-correlation-id")
        .or_else(|| headers.get("X-Correlation-Id"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

async fn enqueue(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    headers: HeaderMap,
    Json(body): Json<EnqueueBody>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();

    let key = body.job_type_key.trim();
    if key.is_empty() {
        return Err(AppError::BadRequest(
            c.t(&loc, "jobs.job_type_key_required"),
        ));
    }

    if !jobs::is_user_http_enqueue_allowed(key) {
        return Err(AppError::BadRequest(
            c.t(&loc, "jobs.job_type_not_allowed_for_user"),
        ));
    }

    let tid = jobs::resolve_job_type_id(&state.db, key).await?;
    if tid == 0 {
        return Err(AppError::BadRequest(
            c.t(&loc, "jobs.job_type_unknown"),
        ));
    }

    let priority = body.priority.unwrap_or(100).clamp(0, 32_767);
    let corr = correlation_from_headers(&headers);

    let res = jobs::enqueue_job(
        &state.db,
        Some(user_id),
        key,
        &body.payload,
        corr.as_deref(),
        priority,
    )
    .await?;

    if res.job_id == 0 {
        return Err(AppError::BadRequest(
            c.t(&loc, "jobs.job_type_unknown"),
        ));
    }

    let status = if res.deduplicated {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };

    Ok((
        status,
        Json(serde_json::json!({
            "job_id": res.job_id.to_string(),
            "deduplicated": res.deduplicated,
        })),
    ))
}

async fn cancel(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(id_str): Path<String>,
) -> AppResult<StatusCode> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let id = ids::parse_u64(&id_str, "id", "social_entity.invalid_id")?;

    let ok = jobs::cancel_job(&state.db, id, user_id).await?;
    if ok {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::BadRequest(c.t(&loc, "jobs.cancel_not_allowed")))
    }
}

async fn run_now(
    State(state): State<AppState>,
    Extension(_): Extension<VerifiedSession>,
) -> AppResult<Json<serde_json::Value>> {
    // Dev utility endpoint: available with explicit bypass flag OR debug builds.
    if !(state.config.dev_auth_bypass || cfg!(debug_assertions)) {
        return Err(AppError::NotFound);
    }
    const CORR: &str = "dev-run-now";

    // Mirror scheduler enqueue (council ticks, judiciary, graph, scope) plus dev scope rebuild.
    jobs::enqueue_recurring_system_jobs(&state.db, CORR).await;

    let rematerialize = jobs::enqueue_job(
        &state.db,
        None,
        "scope_rematerialize_all",
        &serde_json::json!({}),
        Some(CORR),
        10,
    )
    .await?;

    // Drain up to three conductor batches so a single click clears pending work.
    let mut summary = crate::jobs::queue::ConductorSummary::default();
    for _ in 0..3 {
        let tick = jobs::run_conductor_tick(&state.db, &state.config, 64).await?;
        summary.archive_rows_purged += tick.archive_rows_purged;
        summary.leases_reaped += tick.leases_reaped;
        summary.terminal_purged += tick.terminal_purged;
        summary.claimed += tick.claimed;
        summary.completed += tick.completed;
        summary.failed_retry += tick.failed_retry;
        summary.judicial_verdict_retry_candidates += tick.judicial_verdict_retry_candidates;
        summary.judicial_verdicts_resolved += tick.judicial_verdicts_resolved;
        if tick.claimed == 0 {
            break;
        }
    }

    Ok(Json(serde_json::json!({
        "ok": true,
        "enqueued": {
            "scope_rematerialize_all": {
                "job_id": rematerialize.job_id.to_string(),
                "deduplicated": rematerialize.deduplicated
            }
        },
        "summary": summary
    })))
}
