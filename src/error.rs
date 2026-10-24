use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::i18n;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error")]
    Db(#[from] sqlx::Error),
    #[error("not found")]
    NotFound,
    #[error("unauthorized")]
    Unauthorized,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("bad request ({code}): {message}")]
    BadRequestCoded {
        message: String,
        code: String,
        detail: Option<String>,
    },
    #[error("email not verified")]
    EmailNotVerified,
    #[error("too many requests")]
    TooManyRequests {
        message: String,
        retry_after_secs: u64,
    },
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let c = i18n::catalog();
        let loc = i18n::request_locale();
        let verbose_server_errors = std::env::var("VERBOSE_SERVER_ERRORS")
            .map(|v| {
                let s = v.trim().to_ascii_lowercase();
                !(s.is_empty() || s == "0" || s == "false" || s == "off" || s == "no")
            })
            .unwrap_or(true);

        if matches!(self, AppError::EmailNotVerified) {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": c.t(&loc, "error.email_not_verified"),
                    "code": "email_not_verified"
                })),
            )
                .into_response();
        }

        if let AppError::TooManyRequests {
            message,
            retry_after_secs,
        } = &self
        {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({
                    "error": message,
                    "code": "rate_limited",
                    "retry_after_secs": retry_after_secs,
                })),
            )
                .into_response();
        }

        if matches!(self, AppError::Unauthorized) {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": c.t(&loc, "error.unauthorized"),
                    "code": "unauthorized"
                })),
            )
                .into_response();
        }

        let (status, msg, code, detail) = match &self {
            AppError::Db(e) => {
                let base = c.t(&loc, "error.internal");
                let msg = if verbose_server_errors {
                    format!("{base} [debug: {e}]")
                } else {
                    base
                };
                (StatusCode::INTERNAL_SERVER_ERROR, msg, None, None)
            }
            AppError::NotFound => (StatusCode::NOT_FOUND, c.t(&loc, "error.not_found"), None, None),
            AppError::Unauthorized => unreachable!(),
            AppError::Conflict(m) => (StatusCode::CONFLICT, m.clone(), None, None),
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone(), None, None),
            AppError::BadRequestCoded {
                message,
                code,
                detail,
            } => (
                StatusCode::BAD_REQUEST,
                message.clone(),
                Some(code.clone()),
                detail.clone(),
            ),
            AppError::EmailNotVerified => unreachable!(),
            AppError::TooManyRequests { .. } => unreachable!(),
        };
        if let AppError::Db(ref e) = self {
            tracing::error!(error = %e, "request failed");
        }
        let mut body = serde_json::json!({ "error": msg });
        if let Some(code) = code {
            body["code"] = serde_json::Value::String(code);
        }
        if let Some(detail) = detail {
            body["detail"] = serde_json::Value::String(detail);
        }
        (status, Json(body)).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
