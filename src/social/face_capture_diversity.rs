//! Expression-agnostic diversity gate for the three face-capture JPEGs.
//!
//! Compares downscaled grayscale pixels in-request only — no embeddings or durable storage.
//! Rejects static-photo replay and near-identical frames; does not score smile vs neutral.

use image::GenericImageView;
use image::imageops::FilterType;
use image::{DynamicImage, GrayImage};

use crate::error::AppError;
use crate::i18n::Catalog;

use super::face_capture_errors::face_capture_coded;

/// Downscale dimension for pairwise comparison (64×64 grayscale MAD).
pub const DIVERSITY_COMPARE_SIDE: u32 = 64;

/// Minimum mean absolute pixel difference between any pair. Identical or JPEG-recompressed
/// duplicates sit near zero; live captures (even subtle expression shifts) exceed this floor.
pub const MIN_PAIR_MAD: f32 = 2.5;

/// Any pair below this is treated as a byte-identical / static replay set.
pub const EXACT_DUP_MAD: f32 = 0.8;

/// Center crop ratio before downscale — head crops are face-centred; this amplifies mouth/expression drift.
const DIVERSITY_CENTER_CROP_RATIO: f32 = 0.72;

pub fn assert_minimum_capture_diversity(
    neutral: &[u8],
    smile: &[u8],
    frown: &[u8],
    loc: &str,
    c: &Catalog,
) -> Result<(), AppError> {
    let n = decode_gray_downscale(neutral).map_err(|e| decode_err(&e, loc, c))?;
    let s = decode_gray_downscale(smile).map_err(|e| decode_err(&e, loc, c))?;
    let f = decode_gray_downscale(frown).map_err(|e| decode_err(&e, loc, c))?;

    let pairs = [
        ("neutral", "smile", mean_abs_diff(&n, &s)),
        ("neutral", "frown", mean_abs_diff(&n, &f)),
        ("smile", "frown", mean_abs_diff(&s, &f)),
    ];

    let mut below_floor = 0u8;
    let mut min_mad = f32::MAX;
    for (a, b, mad) in pairs {
        min_mad = min_mad.min(mad);
        if mad < MIN_PAIR_MAD {
            below_floor += 1;
            tracing::info!(
                pair = %format!("{a}/{b}"),
                mad,
                min = MIN_PAIR_MAD,
                "face capture diversity pair below floor"
            );
        }
    }

    // Exact replay: all three pairs nearly identical. Live capture with drift: at most one weak pair.
    if min_mad < EXACT_DUP_MAD || below_floor >= 2 {
        tracing::info!(
            min_mad,
            below_floor,
            "face capture diversity gate rejected near-duplicate set"
        );
        return Err(face_capture_coded(
            loc,
            c,
            "face_insufficient_diversity",
            "ui.face_capture.insufficient_diversity",
        ));
    }

    Ok(())
}

fn decode_err(err: &str, loc: &str, c: &Catalog) -> AppError {
    tracing::warn!(error = %err, "face capture diversity decode failed");
    face_capture_coded(
        loc,
        c,
        "face_quality_low_resolution",
        "ui.face_capture.quality_low_resolution",
    )
}

fn decode_gray_downscale(jpeg: &[u8]) -> Result<GrayImage, String> {
    let img = image::load_from_memory(jpeg).map_err(|e| e.to_string())?;
    Ok(to_compare_gray(&img))
}

fn to_compare_gray(img: &DynamicImage) -> GrayImage {
    let (w, h) = img.dimensions();
    let side = ((w.min(h) as f32) * DIVERSITY_CENTER_CROP_RATIO) as u32;
    let side = side.max(1).min(w.min(h));
    let x = (w - side) / 2;
    let y = (h - side) / 2;
    img.crop_imm(x, y, side, side)
        .resize_exact(
            DIVERSITY_COMPARE_SIDE,
            DIVERSITY_COMPARE_SIDE,
            FilterType::Triangle,
        )
        .to_luma8()
}

fn mean_abs_diff(a: &GrayImage, b: &GrayImage) -> f32 {
    debug_assert_eq!(a.dimensions(), b.dimensions());
    let mut sum = 0u64;
    let n = a.pixels().len().max(1) as f32;
    for (pa, pb) in a.pixels().zip(b.pixels()) {
        sum += pa.0[0].abs_diff(pb.0[0]) as u64;
    }
    sum as f32 / n
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::codecs::jpeg::JpegEncoder;
    use image::{ImageBuffer, Rgb};
    use std::io::Cursor;

    fn jpeg_512_from_fn(f: impl Fn(u32, u32) -> Rgb<u8>, quality: u8) -> Vec<u8> {
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_fn(512, 512, |x, y| f(x, y));
        let mut buf = Cursor::new(Vec::new());
        let mut enc = JpegEncoder::new_with_quality(&mut buf, quality);
        enc.encode(img.as_raw(), 512, 512, image::ExtendedColorType::Rgb8)
            .expect("encode test jpeg");
        buf.into_inner()
    }

    fn jpeg_512(fill: u8, quality: u8) -> Vec<u8> {
        jpeg_512_from_fn(move |x, y| {
            Rgb([
                fill.wrapping_add((x % 17) as u8),
                fill.wrapping_add((y % 13) as u8),
                fill,
            ])
        }, quality)
    }

    fn catalog() -> &'static crate::i18n::Catalog {
        crate::i18n::catalog()
    }

    fn err_code(err: AppError) -> String {
        match err {
            AppError::BadRequestCoded { code, .. } => code,
            other => panic!("expected BadRequestCoded, got {other:?}"),
        }
    }

    #[test]
    fn rejects_identical_bytes_three_times() {
        let c = catalog();
        let blob = jpeg_512(100, 90);
        let err = assert_minimum_capture_diversity(&blob, &blob, &blob, "en", &c)
            .expect_err("identical");
        assert_eq!(err_code(err), "face_insufficient_diversity");
    }

    #[test]
    fn rejects_same_image_different_jpeg_quality() {
        let c = catalog();
        let a = jpeg_512(80, 95);
        let b = jpeg_512(80, 60);
        let err = assert_minimum_capture_diversity(&a, &b, &a, "en", &c).expect_err("recompress");
        assert_eq!(err_code(err), "face_insufficient_diversity");
    }

    #[test]
    fn passes_three_distinct_live_style_captures() {
        let c = catalog();
        let neutral = jpeg_512(40, 90);
        let smile = jpeg_512(90, 90);
        let frown = jpeg_512(140, 90);
        assert_minimum_capture_diversity(&neutral, &smile, &frown, "en", &c)
            .expect("distinct fills");
    }

    #[test]
    fn passes_small_shifts_and_noise() {
        let c = catalog();
        let drift = |shift: u32| {
            jpeg_512_from_fn(
                move |x, y| {
                    let sx = x.saturating_sub(shift);
                    let sy = y.saturating_sub(shift / 2);
                    Rgb([
                        (sx / 2) as u8,
                        (sy / 2) as u8,
                        ((sx + sy) / 3) as u8,
                    ])
                },
                88,
            )
        };
        let a = drift(0);
        let b = drift(24);
        let c_img = drift(48);
        assert_minimum_capture_diversity(&a, &b, &c_img, "en", &c).expect("shifted frames");
    }

    #[test]
    fn passes_live_session_with_slight_head_drift() {
        let c = catalog();
        let drift = |shift: u32, mouth: u8| {
            jpeg_512_from_fn(
                move |x, y| {
                    let sx = x.saturating_sub(shift);
                    let sy = y.saturating_sub(shift / 2);
                    let mouth_band = y > 280 && y < 340 && x > 180 && x < 332;
                    let lift = if mouth_band { mouth } else { 0 };
                    Rgb([
                        ((sx / 2) as u8).wrapping_add(lift / 2),
                        ((sy / 2) as u8).wrapping_add(lift),
                        (((sx + sy) / 3) as u8).wrapping_add(lift / 3),
                    ])
                },
                90,
            )
        };
        let neutral = drift(0, 0);
        let smile = drift(10, 18);
        let frown = drift(20, 8);
        assert_minimum_capture_diversity(&neutral, &smile, &frown, "en", &c)
            .expect("live session drift");
    }

    #[test]
    fn rejects_two_identical_plus_one_different() {
        let c = catalog();
        let dup = jpeg_512(70, 90);
        let other = jpeg_512(160, 90);
        let err =
            assert_minimum_capture_diversity(&dup, &dup, &other, "en", &c).expect_err("pair dup");
        assert_eq!(err_code(err), "face_insufficient_diversity");
    }
}
