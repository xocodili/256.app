//! Gate `/static/.../user-faces/*` so orphan PNGs on disk are not served without a `user_faces` row.

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::app_state::AppState;
use crate::error::AppResult;
use crate::social::user_faces;

/// Web path prefix for generated user avatars (e.g. `/static/generated/user-faces`).
#[must_use]
pub fn web_mount_prefix(generated_faces_dir: &str) -> String {
    let trimmed = generated_faces_dir.trim().replace('\\', "/");
    let trimmed = trimmed.trim_matches('/');
    if trimmed.starts_with("static/") {
        format!("/{trimmed}")
    } else {
        format!("/static/{trimmed}")
    }
}

/// `GET /static/generated/user-faces/{user_id}/portrait.png` — only when `user_faces` authorizes it.
pub async fn serve_generated_artwork(
    State(state): State<AppState>,
    Path(asset_path): Path<String>,
) -> AppResult<Response> {
    let asset_path = asset_path.trim().trim_start_matches('/');
    let Some((user_id, filename)) = parse_asset_path(asset_path) else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    if filename != "portrait.png" {
        return Ok(StatusCode::NOT_FOUND.into_response());
    }

    let Some(stored_rel) = user_faces::artwork_path_for_user(&state.db, user_id).await? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let abs = authorized_abs_path(&stored_rel, &state.config.generated_faces_dir)?;
    let bytes = tokio::fs::read(&abs).await.map_err(|e| {
        tracing::warn!(
            user_id,
            path = %abs.display(),
            error = %e,
            "user face artwork missing on disk"
        );
        crate::error::AppError::NotFound
    })?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .header(header::CACHE_CONTROL, "private, max-age=300")
        .body(Body::from(bytes))
        .expect("response build"))
}

fn parse_asset_path(raw: &str) -> Option<(u64, &str)> {
    let mut parts = raw.split('/');
    let uid_str = parts.next()?;
    let file = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let uid = uid_str.parse().ok()?;
    Some((uid, file))
}

fn authorized_abs_path(
    stored_rel: &str,
    generated_faces_dir: &str,
) -> Result<std::path::PathBuf, crate::error::AppError> {
    let rel = stored_rel
        .trim()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string();
    let expected_prefix = generated_faces_dir
        .trim()
        .replace('\\', "/")
        .trim_matches('/')
        .to_string();
    if rel.is_empty()
        || rel.contains("..")
        || !rel.starts_with(&format!("{expected_prefix}/"))
        || !rel.ends_with("/portrait.png")
    {
        return Err(crate::error::AppError::NotFound);
    }
    Ok(crate::app_root::runtime_app_root().join(rel))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_prefix_from_default_dir() {
        assert_eq!(
            web_mount_prefix("static/generated/user-faces"),
            "/static/generated/user-faces"
        );
    }

    #[test]
    fn parse_portrait_asset_path() {
        assert_eq!(parse_asset_path("42/portrait.png"), Some((42, "portrait.png")));
        assert_eq!(parse_asset_path("42/other.png"), Some((42, "other.png")));
        assert_eq!(parse_asset_path("42/a/b"), None);
    }
}
