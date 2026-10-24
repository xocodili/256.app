//! Server-side vision gate: confirm face-capture JPEGs contain acceptable portrait subjects
//! before they are sent to Imagen subject customization.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use crate::ai::grpc::{self, AiClientError};
use crate::ai::portraiture::PortraitureFacePhotos;
use crate::config::Config;
use crate::error::AppError;
use crate::i18n::Catalog;

use super::face_capture_errors::{
    artist_at_handle, face_capture_coded, face_capture_coded_detail, face_capture_coded_detail_tf,
    face_capture_coded_tf,
};

/// Three 512×512 JPEGs need more wall-clock than a single text agent call.
const FACE_CAPTURE_VALIDATE_TIMEOUT_MS: u64 = 15_000;

pub use crate::ai::grpc::FACE_CAPTURE_VALIDATE_AGENT;

#[derive(Debug, Deserialize)]
struct ValidateResponse {
    valid: Option<bool>,
    reason_code: Option<String>,
}

/// Run vision validation on all three capture JPEGs. Skipped when `face_vision_mock` is enabled.
pub async fn validate_face_capture_photos(
    config: &Config,
    user_id: u64,
    photos: &PortraitureFacePhotos,
    loc: &str,
    c: &Catalog,
) -> Result<(), AppError> {
    if config.face_vision_mock {
        return Ok(());
    }

    let images = vec![
        ("neutral".to_string(), photos.neutral.clone()),
        ("smile".to_string(), photos.smile.clone()),
        ("frown".to_string(), photos.frown.clone()),
    ];

    let mut metadata = HashMap::new();
    metadata.insert(
        "text_prompt".to_string(),
        crate::agent_specs::compose_face_capture_validate_prompt(),
    );
    metadata.insert(
        "business_function".to_string(),
        "face_capture_validate".to_string(),
    );

    let timeout_ms = config
        .ai_grpc_timeout_ms
        .max(FACE_CAPTURE_VALIDATE_TIMEOUT_MS);

    let (response_text, model_id) = grpc::run_text_agent(
        &config.ai_grpc_endpoint,
        timeout_ms,
        FACE_CAPTURE_VALIDATE_AGENT,
        user_id,
        images,
        metadata,
    )
    .await
    .map_err(|e| map_vision_error(e, loc, c, timeout_ms))?;

    match parse_validate_response(&response_text) {
        Ok(ValidateOutcome::Accepted) => {
            tracing::debug!(model_id = %model_id, "face capture vision validation passed");
            Ok(())
        }
        Ok(ValidateOutcome::Rejected { reason_code }) => {
            tracing::info!(
                model_id = %model_id,
                reason_code = reason_code.as_deref().unwrap_or("unspecified"),
                "face capture vision validation rejected photos"
            );
            Err(vision_reject_error(config, loc, c, reason_code.as_deref()))
        }
        Err(reason) => {
            tracing::warn!(
                model_id = %model_id,
                reason = %reason,
                response_preview = %response_text.chars().take(200).collect::<String>(),
                "face capture vision validation parse failed"
            );
            Err(face_capture_coded_detail(
                loc,
                c,
                "face_vision_unavailable",
                "ui.face_capture.vision_failed",
                Some(format!("validate parse: {reason}")),
            ))
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ValidateOutcome {
    Accepted,
    Rejected { reason_code: Option<String> },
}

fn vision_reject_error(config: &Config, loc: &str, c: &Catalog, reason_code: Option<&str>) -> AppError {
    let at = artist_at_handle(&config.council_agent_handle);
    let (code, locale_key) = match reason_code {
        Some("not_portrait") | Some("no_face") | Some("obscured") => (
            "face_vision_rejected_not_live",
            "ui.face_capture.vision_rejected_not_live",
        ),
        Some("not_same_person") => (
            "face_vision_rejected_not_same_person",
            "ui.face_capture.vision_rejected_not_same_person",
        ),
        Some("multiple_faces") => (
            "face_vision_rejected_multiple_faces",
            "ui.face_capture.vision_rejected_multiple_faces",
        ),
        _ => (
            "face_vision_rejected",
            "ui.face_capture.vision_rejected",
        ),
    };
    face_capture_coded_detail_tf(
        loc,
        c,
        code,
        locale_key,
        &[("artist_handle", &at)],
        reason_code.map(str::to_string),
    )
}

fn map_vision_error(err: AiClientError, loc: &str, c: &Catalog, timeout_ms: u64) -> AppError {
    tracing::warn!(error = %err, timeout_ms, "face capture vision validation unavailable");
    match err {
        AiClientError::Timeout => face_capture_coded_tf(
            loc,
            c,
            "face_vision_timeout",
            "ui.face_capture.vision_timeout",
            &[("seconds", &format!("{}", timeout_ms / 1000))],
        ),
        AiClientError::Rpc(msg) if vertex_rate_limited(&msg) => face_capture_coded(
            loc,
            c,
            "face_vision_rate_limited",
            "ui.face_capture.vision_rate_limited",
        ),
        _ => face_capture_coded(loc, c, "face_vision_unavailable", "ui.face_capture.vision_failed"),
    }
}

fn vertex_rate_limited(message: &str) -> bool {
    let m = message.to_ascii_lowercase();
    m.contains("429")
        || m.contains("resource exhausted")
        || m.contains("too many requests")
        || m.contains("resourceexhausted")
}

fn parse_validate_response(text: &str) -> Result<ValidateOutcome, String> {
    let json = extract_json_object(text).ok_or_else(|| "no json object in response".to_string())?;
    let parsed: ValidateResponse =
        serde_json::from_value(json).map_err(|e| format!("invalid validate json: {e}"))?;
    match parsed.valid {
        Some(true) => Ok(ValidateOutcome::Accepted),
        Some(false) => Ok(ValidateOutcome::Rejected {
            reason_code: parsed.reason_code,
        }),
        None => Err("missing valid field".to_string()),
    }
}

fn extract_json_object(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
        if v.is_object() {
            return Some(v);
        }
    }
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if end > start {
                if let Ok(v) = serde_json::from_str::<Value>(&trimmed[start..=end]) {
                    if v.is_object() {
                        return Some(v);
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_true_json() {
        assert_eq!(
            parse_validate_response(r#"{"valid": true}"#).unwrap(),
            ValidateOutcome::Accepted
        );
    }

    #[test]
    fn rejects_valid_false_json() {
        assert_eq!(
            parse_validate_response(r#"{"valid": false, "reason_code": "no_face"}"#).unwrap(),
            ValidateOutcome::Rejected {
                reason_code: Some("no_face".to_string())
            }
        );
    }

    #[test]
    fn parses_json_in_fenced_prose() {
        assert_eq!(
            parse_validate_response(
                "Here is the result:\n```json\n{\"valid\": true}\n```"
            )
            .unwrap(),
            ValidateOutcome::Accepted
        );
    }

    #[test]
    fn missing_valid_field_is_error() {
        assert!(parse_validate_response(r#"{"reason_code":"no_face"}"#).is_err());
    }

    fn sample_validate_jpegs() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        use image::codecs::jpeg::JpegEncoder;
        use image::{ImageBuffer, Rgb};
        use std::io::Cursor;

        fn jpeg(fill: u8) -> Vec<u8> {
            let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
                ImageBuffer::from_fn(512, 512, |x, y| {
                    Rgb([
                        fill.wrapping_add((x % 17) as u8),
                        fill.wrapping_add((y % 13) as u8),
                        fill,
                    ])
                });
            let mut buf = Cursor::new(Vec::new());
            let mut enc = JpegEncoder::new_with_quality(&mut buf, 90);
            enc.encode(img.as_raw(), 512, 512, image::ExtendedColorType::Rgb8)
                .expect("encode test jpeg");
            buf.into_inner()
        }

        (jpeg(40), jpeg(90), jpeg(140))
    }

    #[test]
    fn detects_vertex_rate_limit_messages() {
        assert!(vertex_rate_limited(
            "status: ResourceExhausted, message: \"vertex api 429 Too Many Requests\""
        ));
        assert!(!vertex_rate_limited("status: Unavailable, message: connection refused"));
    }

    #[tokio::test]
    #[ignore = "requires local ai_gateway + Vertex credentials"]
    async fn live_gateway_face_validate_smoke() {
        let _ = dotenvy::dotenv();
        let endpoint = std::env::var("AI_GRPC_ENDPOINT")
            .unwrap_or_else(|_| "http://127.0.0.1:50051".to_string());
        let timeout_ms: u64 = std::env::var("AI_GRPC_TIMEOUT_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(30_000)
            .max(FACE_CAPTURE_VALIDATE_TIMEOUT_MS);
        let (neutral, smile, frown) = sample_validate_jpegs();
        let images = vec![
            ("neutral".to_string(), neutral),
            ("smile".to_string(), smile),
            ("frown".to_string(), frown),
        ];
        let mut metadata = HashMap::new();
        metadata.insert(
        "text_prompt".to_string(),
        crate::agent_specs::compose_face_capture_validate_prompt(),
    );
        metadata.insert(
            "business_function".to_string(),
            "face_capture_validate".to_string(),
        );

        let result = grpc::run_text_agent(
            &endpoint,
            timeout_ms,
            FACE_CAPTURE_VALIDATE_AGENT,
            1,
            images,
            metadata,
        )
        .await;

        match result {
            Ok((text, model)) => {
                eprintln!("model={model} text={text}");
                let parsed = parse_validate_response(&text);
                eprintln!("parsed={parsed:?}");
                assert!(parsed.is_ok(), "expected parseable validate json");
            }
            Err(err) => {
                panic!("live gateway validate failed: {err}");
            }
        }
    }
}
