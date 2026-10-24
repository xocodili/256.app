//! Pick the best Imagen candidate by comparing a face-region likeness grid to the subject reference.
//!
//! Uses the prepared neutral JPEG (same black-ground crop as Imagen ref `[1]`) — transient,
//! in-request only, no biometric storage.

use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, GrayImage};

use crate::ai::portrait_postprocess;

/// Downscale side for likeness grid (matches diversity gate scale).
pub const LIKENESS_GRID_SIDE: u32 = 64;

/// Grid cells sampled inside the face ellipse (likeness validation samples).
const GRID_CELLS: u32 = 8;

/// Center crop before downscale — head-centred like capture diversity.
const LIKENESS_CENTER_CROP_RATIO: f32 = 0.72;

/// Weight for luminance grid vs gradient-structure grid.
const LUMA_GRID_WEIGHT: f32 = 0.65;
const GRAD_GRID_WEIGHT: f32 = 0.35;

/// Lower score = closer match to the reference subject patch.
#[must_use]
pub fn likeness_distance(reference_jpeg: &[u8], candidate_png: &[u8]) -> Result<f32, String> {
    let reference = face_patch_gray(reference_jpeg)?;
    let candidate = face_patch_gray(candidate_png)?;
    if reference.dimensions() != candidate.dimensions() {
        return Err("likeness patch dimension mismatch".into());
    }

    let luma = grid_signature_distance(&reference, &candidate, GridKind::Luma);
    let grad = grid_signature_distance(&reference, &candidate, GridKind::Gradient);
    Ok(LUMA_GRID_WEIGHT * luma + GRAD_GRID_WEIGHT * grad)
}

/// Pick the candidate index with lowest likeness distance. Ties favour lower index.
pub fn pick_best_likeness_index(
    reference_jpeg: &[u8],
    candidates: &[Vec<u8>],
) -> Result<usize, String> {
    if candidates.is_empty() {
        return Err("no portrait candidates to score".into());
    }
    let mut best_idx = 0usize;
    let mut best_dist = f32::MAX;
    for (idx, png) in candidates.iter().enumerate() {
        let dist = likeness_distance(reference_jpeg, png)?;
        tracing::debug!(candidate = idx, likeness_distance = dist, "portrait likeness scored");
        if dist < best_dist {
            best_dist = dist;
            best_idx = idx;
        }
    }
    tracing::info!(
        picked = best_idx,
        candidate_count = candidates.len(),
        likeness_distance = best_dist,
        "portrait likeness pick"
    );
    Ok(best_idx)
}

fn face_patch_gray(bytes: &[u8]) -> Result<GrayImage, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("decode likeness image: {e}"))?;
    Ok(to_compare_gray(&img))
}

fn to_compare_gray(img: &DynamicImage) -> GrayImage {
    let (w, h) = img.dimensions();
    let side = ((w.min(h) as f32) * LIKENESS_CENTER_CROP_RATIO) as u32;
    let side = side.max(1).min(w.min(h));
    let x = (w - side) / 2;
    let y = (h - side) / 2;
    img.crop_imm(x, y, side, side)
        .resize_exact(
            LIKENESS_GRID_SIDE,
            LIKENESS_GRID_SIDE,
            FilterType::Triangle,
        )
        .to_luma8()
}

#[derive(Clone, Copy)]
enum GridKind {
    Luma,
    Gradient,
}

fn grid_signature_distance(a: &GrayImage, b: &GrayImage, kind: GridKind) -> f32 {
    let sig_a = likeness_grid_signature(a, kind);
    let sig_b = likeness_grid_signature(b, kind);
    l2_normalized(&sig_a, &sig_b)
}

fn likeness_grid_signature(gray: &GrayImage, kind: GridKind) -> Vec<f32> {
    let w = gray.width();
    let h = gray.height();
    let mut values = Vec::with_capacity((GRID_CELLS * GRID_CELLS) as usize);
    for gy in 0..GRID_CELLS {
        for gx in 0..GRID_CELLS {
            let cx = ((gx as f32 + 0.5) / GRID_CELLS as f32 * w as f32) as u32;
            let cy = ((gy as f32 + 0.5) / GRID_CELLS as f32 * h as f32) as u32;
            if !portrait_postprocess::in_face_guard_ellipse(cx, cy, w, h) {
                continue;
            }
            let v = match kind {
                GridKind::Luma => gray.get_pixel(cx.min(w - 1), cy.min(h - 1)).0[0] as f32,
                GridKind::Gradient => gradient_magnitude(gray, cx.min(w - 1), cy.min(h - 1)),
            };
            values.push(v);
        }
    }
    normalize_signature(values)
}

fn gradient_magnitude(gray: &GrayImage, x: u32, y: u32) -> f32 {
    let w = gray.width();
    let h = gray.height();
    let l = |px: u32, py: u32| gray.get_pixel(px.min(w - 1), py.min(h - 1)).0[0] as f32;
    let dx = l(x + 1, y) - l(x.saturating_sub(1), y);
    let dy = l(x, y + 1) - l(x, y.saturating_sub(1));
    (dx * dx + dy * dy).sqrt()
}

fn normalize_signature(mut values: Vec<f32>) -> Vec<f32> {
    if values.is_empty() {
        return values;
    }
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    for v in &mut values {
        *v -= mean;
    }
    let var = values.iter().map(|v| v * v).sum::<f32>() / values.len() as f32;
    let std = var.sqrt().max(1e-3);
    for v in &mut values {
        *v /= std;
    }
    values
}

fn l2_normalized(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n == 0 {
        return f32::MAX;
    }
    let mut sum = 0f32;
    for i in 0..n {
        let d = a[i] - b[i];
        sum += d * d;
    }
    (sum / n as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::codecs::jpeg::JpegEncoder;
    use image::{Rgb, RgbImage};
    use std::io::Cursor;

    fn jpeg_from_rgb(img: &RgbImage) -> Vec<u8> {
        let mut out = Vec::new();
        JpegEncoder::new_with_quality(Cursor::new(&mut out), 90)
            .encode(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgb8)
            .expect("encode");
        out
    }

    fn png_from_rgb(img: &RgbImage) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        image::write_buffer_with_format(
            &mut out,
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgb8,
            image::ImageFormat::Png,
        )
        .expect("png");
        out.into_inner()
    }

    fn face_like_image(seed: u8) -> RgbImage {
        let mut img = RgbImage::new(512, 512);
        for y in 0..512 {
            for x in 0..512 {
                let in_face = {
                    let cx = 256f32;
                    let cy = 220f32;
                    let dx = (x as f32 - cx) / 140.0;
                    let dy = (y as f32 - cy) / 160.0;
                    dx * dx + dy * dy <= 1.0
                };
                let px = if in_face {
                    Rgb([
                        seed.wrapping_add((x / 8) as u8),
                        seed.wrapping_add((y / 10) as u8),
                        seed.wrapping_add(((x + y) / 12) as u8),
                    ])
                } else {
                    Rgb([0, 0, 0])
                };
                img.put_pixel(x, y, px);
            }
        }
        img
    }

    #[test]
    fn identical_reference_and_candidate_score_zeroish() {
        let img = face_like_image(120);
        let png = png_from_rgb(&img);
        let dist = likeness_distance(&png, &png).expect("score");
        assert!(dist < 0.05, "expected near-zero distance, got {dist}");
    }

    #[test]
    fn closer_candidate_wins_pick() {
        let reference = jpeg_from_rgb(&face_like_image(100));
        let close = png_from_rgb(&face_like_image(102));
        let far = png_from_rgb(&face_like_image(200));
        let idx = pick_best_likeness_index(&reference, &[far, close]).expect("pick");
        assert_eq!(idx, 1);
    }

    #[test]
    fn pick_rejects_empty() {
        assert!(pick_best_likeness_index(&[], &[]).is_err());
    }
}
