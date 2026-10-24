//! Shared coded bad-request helpers for face-capture ingress gates.

use crate::error::AppError;
use crate::i18n::Catalog;

pub fn face_capture_coded(loc: &str, c: &Catalog, code: &str, locale_key: &str) -> AppError {
    face_capture_coded_detail(loc, c, code, locale_key, None)
}

pub fn face_capture_coded_tf(
    loc: &str,
    c: &Catalog,
    code: &str,
    locale_key: &str,
    vars: &[(&str, &str)],
) -> AppError {
    face_capture_coded_detail_tf(loc, c, code, locale_key, vars, None)
}

pub fn face_capture_coded_detail(
    loc: &str,
    c: &Catalog,
    code: &str,
    locale_key: &str,
    detail: Option<String>,
) -> AppError {
    AppError::BadRequestCoded {
        message: c.t(loc, locale_key),
        code: code.to_string(),
        detail: detail.map(truncate_client_detail),
    }
}

pub fn face_capture_coded_detail_tf(
    loc: &str,
    c: &Catalog,
    code: &str,
    locale_key: &str,
    vars: &[(&str, &str)],
    detail: Option<String>,
) -> AppError {
    AppError::BadRequestCoded {
        message: c.tf(loc, locale_key, vars),
        code: code.to_string(),
        detail: detail.map(truncate_client_detail),
    }
}

pub fn artist_at_handle(raw: &str) -> String {
    let handle = raw.trim().trim_start_matches('@');
    format!("@{handle}")
}

pub fn truncate_client_detail(raw: String) -> String {
    raw.trim().chars().take(360).collect()
}

/// gRPC / gateway / Vertex transport failures — not a user photo-quality rejection.
#[must_use]
pub fn is_portrait_infrastructure_error(raw: &str) -> bool {
    let s = raw.to_ascii_lowercase();
    s.contains("h2 protocol error")
        || s.contains("transport error")
        || s.contains("timeout expired")
        || s.contains("grpc timeout")
        || s.contains("connection reset")
        || s.contains("connection refused")
        || s.contains("broken pipe")
        || s.contains("status: unavailable")
        || s.contains("status: cancelled")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infrastructure_error_detects_h2_cancelled() {
        assert!(is_portrait_infrastructure_error(
            r#"status: Cancelled, message: "h2 protocol error: http2 error""#
        ));
    }

    #[test]
    fn infrastructure_error_ignores_quality_gate() {
        assert!(!is_portrait_infrastructure_error(
            "portrait generation failed quality gate: all samples had mouth void"
        ));
    }
}
