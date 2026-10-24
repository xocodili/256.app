use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};

use crate::app_state::AppState;
use crate::error::AppResult;
use crate::locations::{self, CreateLocationRequest, LocationPublic};
use crate::middleware::VerifiedSession;
use crate::rate_limit;

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/locations",
        post(create_location).layer(rate_limit::locations_create_layer()),
    )
}

async fn create_location(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Json(body): Json<CreateLocationRequest>,
) -> AppResult<(StatusCode, Json<LocationPublic>)> {
    let loc = locations::create_or_get(&state.db, body, user_id).await?;
    Ok((StatusCode::OK, Json(loc)))
}
