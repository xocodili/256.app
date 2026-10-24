//! Imagen portrait raster via Vertex (shares outbound rate limit with text agents).

use app::ai::portrait_likeness;
use app::ai::portrait_postprocess;
use app::ai::portrait_raster::{
    build_reference_images, portrait_style_medium_description, prepare_portrait_subject_jpeg,
    PORTRAIT_SAMPLE_BATCHES, PORTRAIT_SAMPLE_COUNT, DEFAULT_CAPABILITY_MODEL,
};
use app::ai::portrait_vertex::{predict_imagen_samples, publisher_predict_url};
use app::ai::portraiture::PortraitureFacePhotos;
use serde_json::Value;
use tonic::Status;

use crate::pb::RunPortraitAgentRequest;

pub fn photos_from_request(req: &RunPortraitAgentRequest) -> Result<PortraitureFacePhotos, Status> {
    let mut neutral = None;
    let mut smile = None;
    let mut frown = None;
    for frame in &req.images {
        match frame.label.trim() {
            "neutral" => neutral = Some(frame.jpeg.clone()),
            "smile" => smile = Some(frame.jpeg.clone()),
            "frown" => frown = Some(frame.jpeg.clone()),
            "style" => {}
            _ => {}
        }
    }
    Ok(PortraitureFacePhotos {
        neutral: neutral.ok_or_else(|| Status::invalid_argument("missing neutral image"))?,
        smile: smile.ok_or_else(|| Status::invalid_argument("missing smile image"))?,
        frown: frown.ok_or_else(|| Status::invalid_argument("missing frown image"))?,
    })
}

fn style_reference_from_request(req: &RunPortraitAgentRequest) -> Option<Vec<u8>> {
    req.images
        .iter()
        .find(|frame| frame.label.trim() == "style")
        .map(|frame| frame.jpeg.clone())
        .filter(|bytes| !bytes.is_empty())
}

fn ensure_capability_model(model_id: &str) -> Result<(), Status> {
    if model_id.contains("capability") {
        return Ok(());
    }
    Err(Status::invalid_argument(format!(
        "model_id must be an Imagen customization model (e.g. {DEFAULT_CAPABILITY_MODEL})"
    )))
}

fn parameters_with_sample_count(parameters: &Value, sample_count: u32) -> Value {
    let mut merged = parameters.clone();
    if let Some(obj) = merged.as_object_mut() {
        obj.insert(
            "sampleCount".to_string(),
            Value::from(sample_count.clamp(1, 4)),
        );
    }
    merged
}

fn postprocess_candidate(png: Vec<u8>) -> Vec<u8> {
    let flattened = portrait_postprocess::flatten_portrait_background(&png).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "portrait background flatten failed; using raw Imagen output");
        png
    });
    portrait_postprocess::remove_colored_backdrops_outside_bust(&flattened).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "portrait backdrop cleanup failed; using flattened output");
        flattened
    })
}

fn pick_best_sample(
    reference_subject_jpeg: &[u8],
    samples: Vec<Vec<u8>>,
) -> Result<Vec<u8>, Status> {
    let mut acceptable = Vec::new();
    for (idx, png) in samples.into_iter().enumerate() {
        let cleaned = postprocess_candidate(png);
        match portrait_postprocess::has_unacceptable_mouth_void(&cleaned) {
            Ok(true) => {
                tracing::debug!(sample = idx, "portrait sample rejected: mouth void");
                continue;
            }
            Ok(false) => acceptable.push(cleaned),
            Err(e) => {
                tracing::warn!(sample = idx, error = %e, "mouth void check failed; keeping sample");
                acceptable.push(cleaned);
            }
        }
    }

    if acceptable.is_empty() {
        return Err(Status::failed_precondition(
            "portrait generation failed quality gate: all samples had mouth void",
        ));
    }

    let idx = portrait_likeness::pick_best_likeness_index(reference_subject_jpeg, &acceptable)
        .map_err(Status::internal)?;
    Ok(acceptable
        .into_iter()
        .nth(idx)
        .expect("likeness pick index in range"))
}

pub async fn generate_portrait(
    req: &RunPortraitAgentRequest,
    project: &str,
    location: &str,
    access_token: &str,
    timeout_ms: u64,
) -> Result<(Vec<u8>, String), Status> {
    let model_id = req.model_id.trim();
    if model_id.is_empty() {
        return Err(Status::invalid_argument("model_id is required"));
    }
    ensure_capability_model(model_id)?;

    let prompt = req.prompt.trim();
    if prompt.is_empty() {
        return Err(Status::invalid_argument("prompt is required"));
    }

    let parameters: Value = serde_json::from_str(req.parameters_json.trim()).map_err(|e| {
        Status::invalid_argument(format!("parameters_json must be a JSON object: {e}"))
    })?;
    if !parameters.is_object() {
        return Err(Status::invalid_argument(
            "parameters_json must be a JSON object",
        ));
    }

    let photos = photos_from_request(req)?;
    let reference_subject_jpeg = prepare_portrait_subject_jpeg(&photos.neutral).map_err(|e| {
        Status::invalid_argument(format!("prepare portrait subject reference: {e}"))
    })?;

    let style_reference_png = style_reference_from_request(req);
    let style_description = if style_reference_png.is_some() {
        portrait_style_medium_description(req.council_handle.trim())
    } else {
        String::new()
    };
    let subject_label = req.subject_description_suffix.trim();
    let label = if subject_label.is_empty() {
        "this person from the reference photo"
    } else {
        subject_label
    };
    let reference_images = build_reference_images(
        &photos,
        style_reference_png.as_deref(),
        style_description.trim(),
        label,
    );
    let parameters = parameters_with_sample_count(&parameters, PORTRAIT_SAMPLE_COUNT);
    let body = serde_json::json!({
        "instances": [{
            "prompt": prompt,
            "referenceImages": reference_images
        }],
        "parameters": parameters
    });

    let endpoint = publisher_predict_url(project, location, model_id);
    let mut last_err = String::from("portrait generation exhausted sample batches");

    for batch in 1..=PORTRAIT_SAMPLE_BATCHES {
        let samples = match predict_imagen_samples(&endpoint, access_token, &body, timeout_ms).await
        {
            Ok(samples) => samples,
            Err(e) => {
                last_err = e.clone();
                if batch == PORTRAIT_SAMPLE_BATCHES {
                    return Err(Status::unavailable(e));
                }
                tracing::warn!(batch, error = %e, "portrait Imagen batch failed; retrying");
                continue;
            }
        };

        match pick_best_sample(&reference_subject_jpeg, samples) {
            Ok(png) => {
                tracing::info!(
                    batch,
                    sample_count = PORTRAIT_SAMPLE_COUNT,
                    "portrait multi-sample pick complete"
                );
                return Ok((png, model_id.to_string()));
            }
            Err(status) if batch < PORTRAIT_SAMPLE_BATCHES => {
                last_err = status.message().to_string();
                tracing::warn!(batch, error = %last_err, "portrait sample batch failed QA; retrying");
            }
            Err(status) => return Err(status),
        }
    }

    Err(Status::failed_precondition(last_err))
}
