//! Shared Imagen subject-customization HTTP helpers (used by `ai_gateway`).

use std::time::Duration;

use base64::Engine as _;
use image::imageops::FilterType;
use image::ImageFormat;
use reqwest::Client;
use serde_json::Value;

/// Imagen `:predict` endpoint for a publisher model.
#[must_use]
pub fn publisher_predict_url(project: &str, location: &str, model: &str) -> String {
    format!(
        "https://{location}-aiplatform.googleapis.com/v1/projects/{project}/locations/{location}/publishers/google/models/{model}:predict"
    )
}

pub fn extract_prediction_image_bytes(json: &Value) -> Result<Vec<u8>, String> {
    extract_all_prediction_image_bytes(json)?
        .into_iter()
        .next()
        .ok_or_else(|| "portrait raster response missing predictions".to_string())
}

pub fn extract_all_prediction_image_bytes(json: &Value) -> Result<Vec<Vec<u8>>, String> {
    let preds = json
        .get("predictions")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "portrait raster response missing predictions array".to_string())?;
    if preds.is_empty() {
        return Err("portrait raster response has empty predictions".to_string());
    }

    let mut out = Vec::with_capacity(preds.len());
    for (idx, pred) in preds.iter().enumerate() {
        let b64 = pred
            .get("bytesBase64Encoded")
            .or_else(|| pred.get("image").and_then(|v| v.get("bytesBase64Encoded")))
            .or_else(|| pred.get("imageBytes"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                format!("portrait raster response missing image bytes at predictions[{idx}]")
            })?;
        let raw = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|e| format!("decode image bytes at predictions[{idx}]: {e}"))?;
        out.push(raw);
    }
    Ok(out)
}

pub fn normalize_to_png_512(raw: &[u8]) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(raw).map_err(|e| format!("decode generated image: {e}"))?;
    let resized = img.resize_exact(512, 512, FilterType::Lanczos3);
    let mut out = Vec::new();
    resized
        .write_to(&mut std::io::Cursor::new(&mut out), ImageFormat::Png)
        .map_err(|e| format!("encode normalized PNG: {e}"))?;
    Ok(out)
}

/// Call Vertex Imagen `:predict` with a pre-built JSON body (first sample only).
pub async fn predict_imagen(
    endpoint: &str,
    access_token: &str,
    body: &Value,
    timeout_ms: u64,
) -> Result<Vec<u8>, String> {
    let json = predict_imagen_json(endpoint, access_token, body, timeout_ms).await?;
    let raw = extract_prediction_image_bytes(&json)?;
    normalize_to_png_512(&raw)
}

/// Call Vertex Imagen `:predict` and return every sample in the response (normalized 512 PNG).
pub async fn predict_imagen_samples(
    endpoint: &str,
    access_token: &str,
    body: &Value,
    timeout_ms: u64,
) -> Result<Vec<Vec<u8>>, String> {
    let json = predict_imagen_json(endpoint, access_token, body, timeout_ms).await?;
    let raws = extract_all_prediction_image_bytes(&json)?;
    raws.into_iter()
        .map(|raw| normalize_to_png_512(&raw))
        .collect()
}

async fn predict_imagen_json(
    endpoint: &str,
    access_token: &str,
    body: &Value,
    timeout_ms: u64,
) -> Result<Value, String> {
    let client = Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .map_err(|e| format!("portrait raster http client: {e}"))?;

    let resp = client
        .post(endpoint)
        .bearer_auth(access_token)
        .json(body)
        .send()
        .await
        .map_err(|e| format!("portrait raster request: {e}"))?;

    let status = resp.status();
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("portrait raster body: {e}"))?;
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    if !status.is_success() {
        let detail = json
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .map(String::from)
            .unwrap_or_else(|| String::from_utf8_lossy(&bytes).chars().take(400).collect());
        return Err(format!("portrait raster {status}: {detail}"));
    }
    Ok(json)
}
