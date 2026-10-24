//! Portrait post-processing — enforce flat `#000000` ground after Imagen returns.

use std::collections::VecDeque;
use std::io::Cursor;

use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder, Rgb, RgbImage};

const FLATTEN_CORNER_SAMPLE: u32 = 12;
const FLATTEN_COLOR_THRESHOLD: f32 = 42.0;
const FLATTEN_BORDER_STRIP_RATIO: f32 = 0.10;
const FLATTEN_BRIGHT_LUMA: f32 = 175.0;
const FLATTEN_WHITE_BACKDROP_LUMA: f32 = 235.0;
const FLATTEN_DARK_CORNER_LUMA: f32 = 36.0;
const FLATTEN_MAX_BG_FRACTION: f32 = 0.72;

fn luma(r: u8, g: u8, b: u8) -> f32 {
    0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b)
}

fn color_dist(a: (u8, u8, u8), b: (u8, u8, u8)) -> f32 {
    let dr = f32::from(a.0) - f32::from(b.0);
    let dg = f32::from(a.1) - f32::from(b.1);
    let db = f32::from(a.2) - f32::from(b.2);
    (dr * dr + dg * dg + db * db).sqrt()
}

fn corner_means(img: &RgbImage, sample: u32) -> Vec<(u8, u8, u8)> {
    let w = img.width();
    let h = img.height();
    let sample = sample.min(w).min(h).max(1);
    let patches = [
        (0, 0),
        (w.saturating_sub(sample), 0),
        (0, h.saturating_sub(sample)),
        (w.saturating_sub(sample), h.saturating_sub(sample)),
    ];
    patches
        .into_iter()
        .map(|(x0, y0)| {
            let mut r_sum = 0u64;
            let mut g_sum = 0u64;
            let mut b_sum = 0u64;
            let mut count = 0u64;
            for y in y0..y0.saturating_add(sample).min(h) {
                for x in x0..x0.saturating_add(sample).min(w) {
                    let p = img.get_pixel(x, y).0;
                    r_sum += u64::from(p[0]);
                    g_sum += u64::from(p[1]);
                    b_sum += u64::from(p[2]);
                    count += 1;
                }
            }
            (
                (r_sum / count) as u8,
                (g_sum / count) as u8,
                (b_sum / count) as u8,
            )
        })
        .collect()
}

fn corners_are_dark(corners: &[(u8, u8, u8)]) -> bool {
    corners
        .iter()
        .all(|c| luma(c.0, c.1, c.2) <= FLATTEN_DARK_CORNER_LUMA)
}

fn similar_to_corners(pixel: (u8, u8, u8), corners: &[(u8, u8, u8)]) -> bool {
    corners
        .iter()
        .any(|c| color_dist(pixel, *c) < FLATTEN_COLOR_THRESHOLD)
}

fn bright_background(pixel: (u8, u8, u8)) -> bool {
    luma(pixel.0, pixel.1, pixel.2) >= FLATTEN_BRIGHT_LUMA
}

fn in_forced_border(x: u32, y: u32, w: u32, h: u32) -> bool {
    let strip_x = ((w as f32) * FLATTEN_BORDER_STRIP_RATIO).round() as u32;
    let strip_y = ((h as f32) * FLATTEN_BORDER_STRIP_RATIO).round() as u32;
    x < strip_x || y < strip_y || x >= w.saturating_sub(strip_x) || y >= h.saturating_sub(strip_y)
}

/// Central bust region — never flatten subject pixels here unless extremely bright (window blow-out).
/// Face guard ellipse used by likeness-grid sampling and backdrop cleanup.
pub fn in_face_guard_ellipse(x: u32, y: u32, w: u32, h: u32) -> bool {
    in_subject_guard_ellipse(x, y, w, h)
}

fn in_subject_guard_ellipse(x: u32, y: u32, w: u32, h: u32) -> bool {
    let cx = w as f32 * 0.5;
    let cy = h as f32 * 0.44;
    let rx = w as f32 * 0.36;
    let ry = h as f32 * 0.40;
    let dx = (x as f32 + 0.5 - cx) / rx;
    let dy = (y as f32 + 0.5 - cy) / ry;
    dx * dx + dy * dy <= 1.0
}

fn seed_background(
    pixel: (u8, u8, u8),
    corners: &[(u8, u8, u8)],
    dark_corners: bool,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> bool {
    if in_forced_border(x, y, w, h) {
        return true;
    }
    if bright_background(pixel) {
        return true;
    }
    !dark_corners && similar_to_corners(pixel, corners)
}

fn can_flood(
    from: (u8, u8, u8),
    to: (u8, u8, u8),
    corners: &[(u8, u8, u8)],
    dark_corners: bool,
) -> bool {
    if bright_background(to) {
        return true;
    }
    if dark_corners {
        return bright_background(from) && color_dist(from, to) < FLATTEN_COLOR_THRESHOLD;
    }
    color_dist(from, to) < FLATTEN_COLOR_THRESHOLD || similar_to_corners(to, corners)
}

fn in_lower_face_ellipse(x: u32, y: u32, w: u32, h: u32) -> bool {
    let cx = w as f32 * 0.5;
    let cy = h as f32 * 0.54;
    let rx = w as f32 * 0.22;
    let ry = h as f32 * 0.17;
    let dx = (x as f32 + 0.5 - cx) / rx;
    let dy = (y as f32 + 0.5 - cy) / ry;
    dx * dx + dy * dy <= 1.0
}

/// Fraction of lower-face ellipse pixels that are near-pure black (mouth void artifact).
pub fn lower_face_void_fraction(png: &[u8]) -> Result<f32, String> {
    let img = image::load_from_memory(png).map_err(|e| format!("decode portrait png: {e}"))?;
    let rgb = img.to_rgb8();
    let w = rgb.width();
    let h = rgb.height();
    if w == 0 || h == 0 {
        return Err("portrait png has zero dimensions".to_string());
    }

    let mut total = 0u32;
    let mut void_px = 0u32;
    for y in 0..h {
        for x in 0..w {
            if !in_lower_face_ellipse(x, y, w, h) {
                continue;
            }
            total += 1;
            let p = rgb.get_pixel(x, y).0;
            if luma(p[0], p[1], p[2]) <= 12.0 {
                void_px += 1;
            }
        }
    }
    if total == 0 {
        return Ok(0.0);
    }
    Ok(void_px as f32 / total as f32)
}

/// Imagen sometimes leaves a black hole through lips/jaw — reject before likeness pick.
pub fn has_unacceptable_mouth_void(png: &[u8]) -> Result<bool, String> {
    Ok(lower_face_void_fraction(png)? >= 0.10)
}

/// Remove coloured / off-white backdrop pixels outside the bust guard ellipse → `#000000`.
pub fn remove_colored_backdrops_outside_bust(png: &[u8]) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(png).map_err(|e| format!("decode portrait png: {e}"))?;
    let mut rgb = img.to_rgb8();
    let w = rgb.width();
    let h = rgb.height();
    if w == 0 || h == 0 {
        return Err("portrait png has zero dimensions".to_string());
    }

    let mut removed = 0usize;
    for y in 0..h {
        for x in 0..w {
            if in_subject_guard_ellipse(x, y, w, h) {
                continue;
            }
            let p = rgb.get_pixel(x, y).0;
            if is_colored_backdrop_pixel(p[0], p[1], p[2]) {
                rgb.put_pixel(x, y, Rgb([0, 0, 0]));
                removed += 1;
            }
        }
    }

    if removed == 0 {
        return Ok(png.to_vec());
    }

    tracing::debug!(removed_pixels = removed, "removed colored portrait backdrop pixels");

    let mut out = Vec::new();
    let enc = PngEncoder::new(Cursor::new(&mut out));
    enc.write_image(rgb.as_raw(), w, h, ExtendedColorType::Rgb8)
        .map_err(|e| format!("encode cleaned portrait png: {e}"))?;
    Ok(out)
}

/// Legacy name — delegates to [`remove_colored_backdrops_outside_bust`].
pub fn remove_near_white_backdrop(png: &[u8]) -> Result<Vec<u8>, String> {
    remove_colored_backdrops_outside_bust(png)
}

fn subject_pixel_protected(x: u32, y: u32, w: u32, h: u32, _pixel: (u8, u8, u8)) -> bool {
    // Protect the entire bust core — including dark mouth/lip shadows — from background flood-fill.
    in_subject_guard_ellipse(x, y, w, h)
}

/// Replace detected background pixels with pure black `#000000` without eating the bust subject.
pub fn flatten_portrait_background(png: &[u8]) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(png).map_err(|e| format!("decode portrait png: {e}"))?;
    let mut rgb = img.to_rgb8();
    let w = rgb.width();
    let h = rgb.height();
    if w == 0 || h == 0 {
        return Err("portrait png has zero dimensions".to_string());
    }

    let corners = corner_means(&rgb, FLATTEN_CORNER_SAMPLE);
    let dark_corners = corners_are_dark(&corners);
    let len = (w * h) as usize;
    let mut is_bg = vec![false; len];
    let mut queue = VecDeque::new();

    let mut try_seed = |x: u32, y: u32| {
        let i = (y * w + x) as usize;
        if is_bg[i] {
            return;
        }
        let p = rgb.get_pixel(x, y).0;
        let pixel = (p[0], p[1], p[2]);
        if subject_pixel_protected(x, y, w, h, pixel) {
            return;
        }
        if seed_background(pixel, &corners, dark_corners, x, y, w, h) {
            is_bg[i] = true;
            queue.push_back((x, y));
        }
    };

    for x in 0..w {
        try_seed(x, 0);
        try_seed(x, h - 1);
    }
    for y in 0..h {
        try_seed(0, y);
        try_seed(w - 1, y);
    }

    while let Some((x, y)) = queue.pop_front() {
        let from = rgb.get_pixel(x, y).0;
        let from_px = (from[0], from[1], from[2]);
        for (nx, ny) in [
            (x.saturating_sub(1), y),
            (x + 1, y),
            (x, y.saturating_sub(1)),
            (x, y + 1),
        ] {
            if nx >= w || ny >= h {
                continue;
            }
            let i = (ny * w + nx) as usize;
            if is_bg[i] {
                continue;
            }
            let to = rgb.get_pixel(nx, ny).0;
            let to_px = (to[0], to[1], to[2]);
            if subject_pixel_protected(nx, ny, w, h, to_px) && !bright_background(to_px) {
                continue;
            }
            if can_flood(from_px, to_px, &corners, dark_corners) {
                is_bg[i] = true;
                queue.push_back((nx, ny));
            }
        }
    }

    let bg_count = is_bg.iter().filter(|v| **v).count();
    if bg_count as f32 / len as f32 > FLATTEN_MAX_BG_FRACTION {
        tracing::warn!(
            bg_fraction = bg_count as f32 / len as f32,
            dark_corners,
            "portrait flatten skipped — would remove too much of the bust"
        );
        return Ok(png.to_vec());
    }

    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if is_bg[i] {
                rgb.put_pixel(x, y, Rgb([0, 0, 0]));
            }
        }
    }

    let mut out = Vec::new();
    let enc = PngEncoder::new(Cursor::new(&mut out));
    enc.write_image(rgb.as_raw(), w, h, ExtendedColorType::Rgb8)
        .map_err(|e| format!("encode flattened portrait png: {e}"))?;
    Ok(out)
}

fn is_colored_backdrop_pixel(r: u8, g: u8, b: u8) -> bool {
    if luma(r, g, b) >= FLATTEN_WHITE_BACKDROP_LUMA {
        return true;
    }
    is_saturated_backdrop_pixel(r, g, b)
}

fn is_saturated_backdrop_pixel(r: u8, g: u8, b: u8) -> bool {
    let l = luma(r, g, b);
    if l < 40.0 || l > 220.0 {
        return false;
    }
    let rf = f32::from(r);
    let gf = f32::from(g);
    let bf = f32::from(b);
    // Teal / navy wash backgrounds.
    if bf > rf * 1.08 && bf > gf * 0.92 {
        return true;
    }
    // Salmon, pink, coral, orange backdrop disks Imagen sometimes inserts.
    if rf > gf * 1.12 && rf > bf * 1.15 && gf > 45.0 && (rf - bf) > 25.0 {
        return true;
    }
    // Violet / purple wash backgrounds.
    if bf > rf * 0.9 && bf > gf * 1.05 && rf > 70.0 {
        return true;
    }
    false
}

/// Legacy name — delegates to [`remove_colored_backdrops_outside_bust`].
pub fn remove_saturated_backdrop_disks(png: &[u8]) -> Result<Vec<u8>, String> {
    remove_colored_backdrops_outside_bust(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_removes_colored_border_and_bright_regions() {
        let mut img = RgbImage::new(64, 64);
        for y in 0..64 {
            for x in 0..64 {
                let pixel = if x < 8 || y < 8 || x >= 56 || y >= 56 {
                    Rgb([40, 120, 200])
                } else if x > 48 && y < 20 {
                    Rgb([240, 245, 250])
                } else {
                    Rgb([180, 130, 110])
                };
                img.put_pixel(x, y, pixel);
            }
        }
        let mut png = Cursor::new(Vec::new());
        image::write_buffer_with_format(
            &mut png,
            img.as_raw(),
            64,
            64,
            ExtendedColorType::Rgb8,
            image::ImageFormat::Png,
        )
        .expect("encode test png");
        let png = png.into_inner();

        let flat = flatten_portrait_background(&png).expect("flatten");
        let out = image::load_from_memory(&flat)
            .expect("decode flattened")
            .to_rgb8();

        assert_eq!(out.get_pixel(0, 0).0, [0, 0, 0]);
        assert_eq!(out.get_pixel(63, 63).0, [0, 0, 0]);
        assert_eq!(out.get_pixel(60, 10).0, [0, 0, 0], "bright window region");
        assert_ne!(out.get_pixel(32, 32).0, [0, 0, 0], "face center preserved");
    }

    #[test]
    fn flatten_preserves_dark_mouth_region_inside_face_ellipse() {
        let mut img = RgbImage::new(64, 64);
        for y in 0..64 {
            for x in 0..64 {
                img.put_pixel(x, y, Rgb([0, 0, 0]));
            }
        }
        // Face fill
        for y in 18..46 {
            for x in 18..46 {
                img.put_pixel(x, y, Rgb([200, 160, 140]));
            }
        }
        // Dark mouth interior (must not be flood-expanded)
        for y in 30..34 {
            for x in 24..40 {
                img.put_pixel(x, y, Rgb([0, 0, 0]));
            }
        }
        let mut png = Cursor::new(Vec::new());
        image::write_buffer_with_format(
            &mut png,
            img.as_raw(),
            64,
            64,
            ExtendedColorType::Rgb8,
            image::ImageFormat::Png,
        )
        .expect("encode test png");
        let flat = flatten_portrait_background(&png.into_inner()).expect("flatten");
        let out = image::load_from_memory(&flat)
            .expect("decode flattened")
            .to_rgb8();
        assert_eq!(out.get_pixel(28, 32).0, [0, 0, 0], "mouth shadow preserved");
        assert_ne!(out.get_pixel(32, 24).0, [0, 0, 0], "cheek preserved");
    }

    #[test]
    fn flatten_preserves_sparse_line_art_on_black_ground() {
        let mut img = RgbImage::from_pixel(64, 64, Rgb([0, 0, 0]));
        for y in 24..40 {
            for x in 20..44 {
                img.put_pixel(x, y, Rgb([180, 140, 120]));
            }
        }
        for i in 0..64 {
            img.put_pixel(i, 0, Rgb([0, 0, 0]));
            img.put_pixel(i, 63, Rgb([0, 0, 0]));
            img.put_pixel(0, i, Rgb([0, 0, 0]));
            img.put_pixel(63, i, Rgb([0, 0, 0]));
        }
        let mut png = Cursor::new(Vec::new());
        image::write_buffer_with_format(
            &mut png,
            img.as_raw(),
            64,
            64,
            ExtendedColorType::Rgb8,
            image::ImageFormat::Png,
        )
        .expect("encode test png");
        let flat = flatten_portrait_background(&png.into_inner()).expect("flatten");
        let out = image::load_from_memory(&flat)
            .expect("decode flattened")
            .to_rgb8();
        assert_ne!(
            out.get_pixel(32, 32).0,
            [0, 0, 0],
            "dark-corner flatten must not erase central subject"
        );
    }

    #[test]
    fn removes_salmon_backdrop_disk() {
        let mut img = RgbImage::from_pixel(64, 64, Rgb([0, 0, 0]));
        for y in 0..64 {
            for x in 0..64 {
                let dx = x as i32 - 32;
                let dy = y as i32 - 32;
                if dx * dx + dy * dy < 24 * 24 {
                    img.put_pixel(x, y, Rgb([220, 110, 95]));
                }
            }
        }
        for y in 26..38 {
            for x in 24..40 {
                img.put_pixel(x, y, Rgb([180, 140, 120]));
            }
        }
        let mut png = Cursor::new(Vec::new());
        image::write_buffer_with_format(
            &mut png,
            img.as_raw(),
            64,
            64,
            ExtendedColorType::Rgb8,
            image::ImageFormat::Png,
        )
        .expect("encode test png");
        let cleaned = remove_saturated_backdrop_disks(&png.into_inner()).expect("clean");
        let out = image::load_from_memory(&cleaned)
            .expect("decode cleaned")
            .to_rgb8();
        assert_eq!(out.get_pixel(4, 32).0, [0, 0, 0], "salmon disk edge removed");
        assert_ne!(out.get_pixel(32, 32).0, [0, 0, 0], "face core preserved");
    }

    #[test]
    fn detects_large_mouth_void() {
        let mut img = RgbImage::new(128, 128);
        for y in 0..128 {
            for x in 0..128 {
                img.put_pixel(x, y, Rgb([190, 150, 130]));
            }
        }
        for y in 58..88 {
            for x in 40..88 {
                img.put_pixel(x, y, Rgb([0, 0, 0]));
            }
        }
        let mut png = Cursor::new(Vec::new());
        image::write_buffer_with_format(
            &mut png,
            img.as_raw(),
            128,
            128,
            ExtendedColorType::Rgb8,
            image::ImageFormat::Png,
        )
        .expect("encode");
        let bytes = png.into_inner();
        assert!(has_unacceptable_mouth_void(&bytes).expect("check"));
    }

    #[test]
    fn remove_near_white_backdrop_outside_bust() {
        let mut img = RgbImage::new(64, 64);
        for y in 0..64 {
            for x in 0..64 {
                let px = if x < 20 || x > 44 || y < 16 || y > 48 {
                    Rgb([250, 250, 250])
                } else {
                    Rgb([180, 140, 120])
                };
                img.put_pixel(x, y, px);
            }
        }
        let mut png = Cursor::new(Vec::new());
        image::write_buffer_with_format(
            &mut png,
            img.as_raw(),
            64,
            64,
            ExtendedColorType::Rgb8,
            image::ImageFormat::Png,
        )
        .expect("encode");
        let cleaned = remove_near_white_backdrop(&png.into_inner()).expect("clean");
        let out = image::load_from_memory(&cleaned)
            .expect("decode")
            .to_rgb8();
        assert_eq!(out.get_pixel(0, 0).0, [0, 0, 0]);
        assert_ne!(out.get_pixel(32, 32).0, [0, 0, 0]);
    }
}
