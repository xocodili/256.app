//! Raster portrait generation via `ai_gateway` (Imagen 3 subject customization).
//! Prompt assembly and council styling live in [`crate::agent_specs`].

use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::{imageops, Rgb, RgbImage};
use serde_json::Value;

use crate::ai::grpc;
use crate::ai::portrait_colour_card::PortraitColourCard;
use crate::ai::portraiture::PortraitureFacePhotos;

/// Imagen customization model that accepts reference face photos.
pub const DEFAULT_CAPABILITY_MODEL: &str = "imagen-3.0-capability-001";

/// Imagen reference IDs: `[1]` subject, `[3]` style plate.
const REF_ID_SUBJECT: i64 = 1;
const REF_ID_STYLE: i64 = 3;
pub const PORTRAIT_SAMPLE_COUNT: u32 = 3;

/// Full predict retries when every sample in a batch fails mouth-void QA.
pub const PORTRAIT_SAMPLE_BATCHES: u32 = 2;

const PORTRAIT_SUBJECT_CROP_RATIO: f32 = 0.58;
const PORTRAIT_SUBJECT_CANVAS_SIDE: u32 = 512;
const PORTRAIT_SUBJECT_FILL_RATIO: f32 = 0.90;

#[derive(Clone, Debug)]
pub struct PortraitRasterConfig {
    pub grpc_endpoint: String,
    pub model_id: String,
    /// Total gRPC client timeout for `RunPortraitAgent` (may run two Imagen sample batches).
    pub grpc_timeout_ms: u64,
    /// Per-Imagen-predict HTTP budget forwarded to ai_gateway (`requested_timeout_ms`).
    pub predict_timeout_ms: u64,
    pub output_dir: String,
}

fn ensure_capability_model(model_id: &str) -> Result<(), String> {
    if model_id.contains("capability") {
        return Ok(());
    }
    Err(format!(
        "PORTRAIT_RASTER_MODEL must be an Imagen customization model (e.g. {DEFAULT_CAPABILITY_MODEL}); \
         {model_id} does not accept reference face photos"
    ))
}

/// Tight crop, then composite the face onto a pure-black square so Imagen never sees the room.
pub fn prepare_portrait_subject_jpeg(jpeg: &[u8]) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(jpeg).map_err(|e| format!("decode subject jpeg: {e}"))?;
    let (w, h) = (img.width(), img.height());
    let max_side = w.min(h);
    let side = ((max_side as f32) * PORTRAIT_SUBJECT_CROP_RATIO)
        .round()
        .clamp(64.0, max_side as f32) as u32;
    let x = (w - side) / 2;
    let center_y = (h as f32 * 0.40).round() as u32;
    let mut y = center_y.saturating_sub(side / 2);
    if y + side > h {
        y = h.saturating_sub(side);
    }
    let cropped = img.crop_imm(x, y, side, side).to_rgb8();

    let canvas_side = PORTRAIT_SUBJECT_CANVAS_SIDE;
    let target = ((canvas_side as f32) * PORTRAIT_SUBJECT_FILL_RATIO).round() as u32;
    let scaled = imageops::resize(&cropped, target, target, imageops::FilterType::Lanczos3);
    let mut canvas = RgbImage::from_pixel(canvas_side, canvas_side, Rgb([0, 0, 0]));
    let ox = (canvas_side - target) / 2;
    let oy = (canvas_side - target) / 2;
    imageops::overlay(&mut canvas, &scaled, i64::from(ox), i64::from(oy));

    let mut out = Vec::new();
    let mut enc = JpegEncoder::new_with_quality(Cursor::new(&mut out), 92);
    enc.encode(
        canvas.as_raw(),
        canvas_side,
        canvas_side,
        image::ExtendedColorType::Rgb8,
    )
    .map_err(|e| format!("encode subject jpeg: {e}"))?;
    Ok(out)
}

fn subject_reference_image(jpeg: &[u8], subject_label: &str) -> Value {
    use base64::Engine as _;
    let label = subject_label.trim();
    let subject_description = if label.is_empty() {
        "this person from the reference photo".to_string()
    } else {
        label.to_string()
    };
    serde_json::json!({
        "referenceType": "REFERENCE_TYPE_SUBJECT",
        "referenceId": REF_ID_SUBJECT,
        "referenceImage": {
            "bytesBase64Encoded": base64::engine::general_purpose::STANDARD.encode(jpeg)
        },
        "subjectImageConfig": {
            "subjectDescription": subject_description,
            "subjectType": "SUBJECT_TYPE_PERSON"
        }
    })
}

fn style_reference_image(png: &[u8], style_description: &str) -> Value {
    use base64::Engine as _;
    serde_json::json!({
        "referenceType": "REFERENCE_TYPE_STYLE",
        "referenceId": REF_ID_STYLE,
        "referenceImage": {
            "bytesBase64Encoded": base64::engine::general_purpose::STANDARD.encode(png)
        },
        "styleImageConfig": {
            "styleDescription": style_description
        }
    })
}

/// Build Imagen refs: subject `[1]`, optional style plate `[3]`.
pub fn build_reference_images(
    photos: &PortraitureFacePhotos,
    style_reference_png: Option<&[u8]>,
    style_description: &str,
    subject_label: &str,
) -> Vec<Value> {
    let neutral_jpeg = prepare_portrait_subject_jpeg(&photos.neutral).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "portrait subject isolation failed; using full neutral frame");
        photos.neutral.clone()
    });

    let mut refs: Vec<Value> = Vec::with_capacity(2);
    refs.push(subject_reference_image(&neutral_jpeg, subject_label));

    if let Some(png) = style_reference_png {
        refs.push(style_reference_image(png, style_description));
    }

    refs
}

/// Medium label for Imagen style ref `[3]` — must not describe a person.
pub fn portrait_style_medium_description(_council_handle: &str) -> String {
    "filled humanist editorial illustration on pure black ground — grey medium demonstration only, never copy grey as skin or hair"
        .to_string()
}

pub async fn generate_portrait_raster(
    cfg: &PortraitRasterConfig,
    user_id: u64,
    photos: &PortraitureFacePhotos,
    council_handle: &str,
    colour_card: Option<&PortraitColourCard>,
) -> Result<(Vec<u8>, String), String> {
    ensure_capability_model(&cfg.model_id)?;

    let spec = crate::agent_specs::load_council_agent(council_handle)?;
    let style = crate::agent_specs::portrait_synthesis_spec(&spec)?;
    if !style.parameters.is_object() {
        return Err("council agent portrait_synthesis.parameters must be a JSON object".to_string());
    }

    let subject_label = colour_card
        .map(PortraitColourCard::imagen_subject_label)
        .unwrap_or_else(|| "this person from the reference photo".to_string());
    let colour_card_block = colour_card.map(PortraitColourCard::prompt_block).unwrap_or_default();
    let style_reference_png =
        crate::agent_specs::load_portrait_style_reference_bytes(style)?;
    let has_style_plate = style_reference_png.is_some();
    let prompt = crate::agent_specs::compose_imagen_portrait_prompt(
        &spec,
        &subject_label,
        &colour_card_block,
        has_style_plate,
    );
    let parameters_json = style.parameters.to_string();

    let png = grpc::run_portrait_agent(
        &cfg.grpc_endpoint,
        cfg.grpc_timeout_ms,
        cfg.predict_timeout_ms,
        user_id,
        council_handle,
        photos,
        style_reference_png.clone(),
        &prompt,
        &parameters_json,
        &cfg.model_id,
        &subject_label,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok((png, cfg.model_id.clone()))
}

pub async fn write_generated_image(
    output_dir: &str,
    user_id: u64,
    png_bytes: &[u8],
) -> Result<String, String> {
    let base_dir = crate::app_root::runtime_app_root();
    let rel_dir = Path::new(output_dir).join(user_id.to_string());
    let abs_dir: PathBuf = base_dir.join(&rel_dir);

    tokio::fs::create_dir_all(&abs_dir)
        .await
        .map_err(|e| format!("create generated image dir: {e}"))?;

    let filename = "portrait.png";
    let rel_path = rel_dir.join(&filename);
    let abs_path = abs_dir.join(&filename);

    tokio::fs::write(&abs_path, png_bytes)
        .await
        .map_err(|e| format!("write generated image file: {e}"))?;

    Ok(rel_path.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::codecs::jpeg::JpegEncoder;

    fn sample_jpeg_with_border() -> Vec<u8> {
        let mut img = RgbImage::new(512, 512);
        for y in 0..512 {
            for x in 0..512 {
                let color = if x < 40 || y < 40 || x >= 472 || y >= 472 {
                    Rgb([120, 180, 220])
                } else {
                    Rgb([210, 170, 150])
                };
                img.put_pixel(x, y, color);
            }
        }
        let mut out = Vec::new();
        JpegEncoder::new_with_quality(Cursor::new(&mut out), 90)
            .encode(img.as_raw(), 512, 512, image::ExtendedColorType::Rgb8)
            .expect("encode sample jpeg");
        out
    }

    #[test]
    fn prepare_portrait_subject_composites_onto_black() {
        let jpeg = sample_jpeg_with_border();
        let out = prepare_portrait_subject_jpeg(&jpeg).expect("prepare subject");
        let img = image::load_from_memory(&out)
            .expect("decode prepared jpeg")
            .to_rgb8();
        assert_eq!(img.width(), PORTRAIT_SUBJECT_CANVAS_SIDE);
        assert_eq!(img.height(), PORTRAIT_SUBJECT_CANVAS_SIDE);
        assert_eq!(img.get_pixel(0, 0).0, [0, 0, 0]);
        assert_eq!(img.get_pixel(511, 511).0, [0, 0, 0]);
        assert_ne!(img.get_pixel(256, 256).0, [0, 0, 0]);
    }

    #[test]
    fn reference_images_include_subject_only_without_style_plate() {
        let photos = PortraitureFacePhotos {
            neutral: sample_jpeg_with_border(),
            smile: vec![0xff, 0xd8, 0xff, 0x02],
            frown: vec![0xff, 0xd8, 0xff, 0x03],
        };
        let refs = build_reference_images(&photos, None, "", "fair-skinned person with glasses");
        assert_eq!(refs.len(), 1, "subject [1] only");
        assert_eq!(
            refs[0].get("referenceType").and_then(|v| v.as_str()),
            Some("REFERENCE_TYPE_SUBJECT")
        );
    }

    #[test]
    fn reference_images_append_style_plate_as_id_three() {
        let photos = PortraitureFacePhotos {
            neutral: sample_jpeg_with_border(),
            smile: vec![0xff, 0xd8, 0xff, 0x02],
            frown: vec![0xff, 0xd8, 0xff, 0x03],
        };
        let style_png = vec![0x89, 0x50, 0x4e, 0x47];
        let refs = build_reference_images(
            &photos,
            Some(&style_png),
            "filled editorial illustration",
            "person with beard",
        );
        assert_eq!(refs.len(), 2, "subject [1] + style [3]");
        assert_eq!(
            refs[1].get("referenceId").and_then(|v| v.as_i64()),
            Some(3)
        );
        assert_eq!(
            refs[1].get("referenceType").and_then(|v| v.as_str()),
            Some("REFERENCE_TYPE_STYLE")
        );
    }

    #[test]
    fn subject_description_uses_short_label() {
        let photos = PortraitureFacePhotos {
            neutral: sample_jpeg_with_border(),
            smile: vec![],
            frown: vec![],
        };
        let refs = build_reference_images(
            &photos,
            None,
            "",
            "person with fair skin and rectangular glasses",
        );
        let desc = refs[0]
            .get("subjectImageConfig")
            .and_then(|c| c.get("subjectDescription"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(desc.contains("rectangular glasses"));
        assert!(!desc.contains("MANDATORY IDENTITY"));
    }

    #[test]
    fn rejects_non_capability_model() {
        assert!(ensure_capability_model("imagen-3.0-fast-generate-001").is_err());
        assert!(ensure_capability_model(DEFAULT_CAPABILITY_MODEL).is_ok());
    }
}
