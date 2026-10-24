use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};

use crate::app_state::AppState;
use crate::error::AppResult;
use crate::rate_limit;
use crate::ref_key_values;

#[derive(Debug, serde::Deserialize)]
pub struct ListQuery {
    /// Logical group namespace (see `ref_key_values` table column `` `group` ``).
    #[serde(default)]
    group: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/ref-key-values",
        get(list).layer(rate_limit::ref_key_values_list_layer()),
    )
}

async fn list(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Vec<ref_key_values::RefKeyValuePublic>>> {
    let items =
        ref_key_values::list_active_by_group(&state.db, q.group.as_deref().unwrap_or("")).await?;
    Ok(Json(items))
}
