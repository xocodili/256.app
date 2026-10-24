//! JPEG quality gates for face-capture ingress (512×512 standard).

use crate::error::AppError;
use crate::i18n::Catalog;

use super::face_capture_errors::face_capture_coded;

/// Minimum bytes for a 512×512 face JPEG at acceptable detail.
pub const MIN_FACE_JPEG_BYTES: usize = 28 * 1024;
pub const FACE_CAPTURE_DIMENSION: u32 = 512;
/// Client-reported crop side should cover enough source pixels before upscale.
pub const MIN_CROP_SOURCE_SIDE_PX: f32 = 200.0;
/// Minimum face height as a fraction of the camera frame (server-side meta gate).
pub const MIN_FACE_HEIGHT_RATIO: f32 = 0.22;

#[derive(Debug, Clone, Copy)]
pub struct FaceCaptureQualityMeta {
    pub crop_source_side_px: Option<f32>,
    pub face_height_ratio: Option<f32>,
}

/// Decode and validate a single face-capture JPEG before vision.
pub fn validate_face_capture_jpeg(
    data: &[u8],
    meta: FaceCaptureQualityMeta,
    loc: &str,
    c: &Catalog,
) -> Result<(), AppError> {
    if data.len() < MIN_FACE_JPEG_BYTES {
        tracing::warn!(
            bytes = data.len(),
            min = MIN_FACE_JPEG_BYTES,
            "face capture jpeg below minimum size"
        );
        return Err(face_capture_coded(
            loc,
            c,
            "face_quality_too_small",
            "ui.face_capture.quality_too_small",
        ));
    }

    let img = image::load_from_memory(data).map_err(|e| {
        tracing::warn!(error = %e, "face capture jpeg decode failed");
        face_capture_coded(
            loc,
            c,
            "face_quality_low_resolution",
            "ui.face_capture.quality_low_resolution",
        )
    })?;

    if img.width() != FACE_CAPTURE_DIMENSION || img.height() != FACE_CAPTURE_DIMENSION {
        tracing::warn!(
            width = img.width(),
            height = img.height(),
            expected = FACE_CAPTURE_DIMENSION,
            "face capture jpeg dimensions invalid"
        );
        return Err(face_capture_coded(
            loc,
            c,
            "face_quality_low_resolution",
            "ui.face_capture.quality_low_resolution",
        ));
    }

    if let Some(side) = meta.crop_source_side_px {
        if side < MIN_CROP_SOURCE_SIDE_PX {
            tracing::warn!(
                crop_source_side_px = side,
                min = MIN_CROP_SOURCE_SIDE_PX,
                "face capture crop source too small"
            );
            return Err(face_capture_coded(
                loc,
                c,
                "face_quality_move_closer",
                "ui.face_capture.quality_move_closer",
            ));
        }
    }

    if let Some(ratio) = meta.face_height_ratio {
        if ratio < MIN_FACE_HEIGHT_RATIO {
            tracing::warn!(
                face_height_ratio = ratio,
                "face capture subject too far from camera"
            );
            return Err(face_capture_coded(
                loc,
                c,
                "face_quality_move_closer",
                "ui.face_capture.quality_move_closer",
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::codecs::jpeg::JpegEncoder;
    use image::{ImageBuffer, Rgb};
    use std::io::Cursor;

    fn jpeg_512(fill: u8, quality: u8) -> Vec<u8> {
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_fn(512, 512, |x, y| {
                Rgb([
                    fill.wrapping_add((x % 17) as u8),
                    fill.wrapping_add((y % 13) as u8),
                    fill,
                ])
            });
        let mut buf = Cursor::new(Vec::new());
        let mut enc = JpegEncoder::new_with_quality(&mut buf, quality);
        enc.encode(img.as_raw(), 512, 512, image::ExtendedColorType::Rgb8)
            .expect("encode test jpeg");
        buf.into_inner()
    }

    #[test]
    fn rejects_non_512_dimensions() {
        let c = crate::i18n::catalog();
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_pixel(256, 256, Rgb([128, 128, 128]));
        let mut buf = Cursor::new(Vec::new());
        let mut enc = JpegEncoder::new_with_quality(&mut buf, 90);
        enc.encode(img.as_raw(), 256, 256, image::ExtendedColorType::Rgb8)
            .unwrap();
        let data = buf.into_inner();
        assert!(validate_face_capture_jpeg(
            &data,
            FaceCaptureQualityMeta {
                crop_source_side_px: None,
                face_height_ratio: None,
            },
            "en",
            &c
        )
        .is_err());
    }

    #[test]
    fn accepts_detailed_512_jpeg() {
        let c = crate::i18n::catalog();
        let data = jpeg_512(90, 92);
        assert!(data.len() >= MIN_FACE_JPEG_BYTES);
        validate_face_capture_jpeg(
            &data,
            FaceCaptureQualityMeta {
                crop_source_side_px: Some(320.0),
                face_height_ratio: Some(0.35),
            },
            "en",
            &c,
        )
        .expect("valid capture");
    }
}
