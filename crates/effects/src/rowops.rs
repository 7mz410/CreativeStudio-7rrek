//! Raster operations for animation row effects: greyscale, box blur, directional blur,
//! radial zoom blur, colour scale, and transparency.
//!
//! Evaluates pixel buffers safely without unwraps, bounds-checked, and capped (radius ≤ 250).

/// Maximum allowed radius to prevent hangs or excessive allocation.
pub const MAX_RADIUS: f64 = 250.0;

/// Converts an RGBA pixel buffer to greyscale by `amount` in [0, 100] percent.
/// Uses Rec.709 luma coefficients: Y = 0.2126 * R + 0.7152 * G + 0.0722 * B.
/// For pure red (#ff0000) at 100%, luma = 0.2126 * 255 ≈ 54.
pub fn greyscale(px: &mut [[u8; 4]], amount: f64) {
    let t = (amount / 100.0).clamp(0.0, 1.0) as f32;
    if t <= 0.0 {
        return;
    }
    for p in px.iter_mut() {
        let r = p[0] as f32;
        let g = p[1] as f32;
        let b = p[2] as f32;
        let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        p[0] = ((1.0 - t) * r + t * y).round().clamp(0.0, 255.0) as u8;
        p[1] = ((1.0 - t) * g + t * y).round().clamp(0.0, 255.0) as u8;
        p[2] = ((1.0 - t) * b + t * y).round().clamp(0.0, 255.0) as u8;
    }
}

/// Applies a 2D separable box blur to a `w` × `h` RGBA pixel buffer.
/// `radius` is clamped to [0, 250].
pub fn box_blur(px: &mut [[u8; 4]], w: usize, h: usize, radius: f64) {
    if w == 0 || h == 0 || px.len() < w * h {
        return;
    }
    let r = radius.clamp(0.0, MAX_RADIUS).round() as usize;
    if r == 0 {
        return;
    }

    let mut temp = vec![[0u8; 4]; w * h];
    let win = (2 * r + 1) as f32;

    // Horizontal pass: px -> temp
    for y in 0..h {
        let row = y * w;
        for x in 0..w {
            let mut sum = [0.0f32; 4];
            let start = x.saturating_sub(r);
            let end = (x + r).min(w.saturating_sub(1));
            for k in start..=end {
                if let Some(p) = px.get(row + k) {
                    sum[0] += p[0] as f32;
                    sum[1] += p[1] as f32;
                    sum[2] += p[2] as f32;
                    sum[3] += p[3] as f32;
                }
            }
            if let Some(out) = temp.get_mut(row + x) {
                out[0] = (sum[0] / win).round().clamp(0.0, 255.0) as u8;
                out[1] = (sum[1] / win).round().clamp(0.0, 255.0) as u8;
                out[2] = (sum[2] / win).round().clamp(0.0, 255.0) as u8;
                out[3] = (sum[3] / win).round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    // Vertical pass: temp -> px
    for x in 0..w {
        for y in 0..h {
            let mut sum = [0.0f32; 4];
            let start = y.saturating_sub(r);
            let end = (y + r).min(h.saturating_sub(1));
            for k in start..=end {
                if let Some(p) = temp.get(k * w + x) {
                    sum[0] += p[0] as f32;
                    sum[1] += p[1] as f32;
                    sum[2] += p[2] as f32;
                    sum[3] += p[3] as f32;
                }
            }
            if let Some(out) = px.get_mut(y * w + x) {
                out[0] = (sum[0] / win).round().clamp(0.0, 255.0) as u8;
                out[1] = (sum[1] / win).round().clamp(0.0, 255.0) as u8;
                out[2] = (sum[2] / win).round().clamp(0.0, 255.0) as u8;
                out[3] = (sum[3] / win).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

/// Directional blur: smears pixels along `angle` (in degrees) by `radius`.
/// Radius clamped to [0, 250].
/// When angle = 0, smears purely horizontally, leaving vertical extent unchanged.
pub fn directional_blur(px: &mut [[u8; 4]], w: usize, h: usize, radius: f64, angle: f64) {
    if w == 0 || h == 0 || px.len() < w * h {
        return;
    }
    let r = radius.clamp(0.0, MAX_RADIUS);
    if r <= 0.0 {
        return;
    }

    let rad = angle.to_radians();
    let dx = rad.cos();
    let dy = rad.sin();

    let steps = (r.round() as usize).max(1);
    let count = (2 * steps + 1) as f32;
    let original = px.to_vec();

    for y in 0..h {
        for x in 0..w {
            let mut sum_r = 0.0f32;
            let mut sum_g = 0.0f32;
            let mut sum_b = 0.0f32;
            let mut sum_a = 0.0f32;

            for step in -(steps as isize)..=(steps as isize) {
                let frac = step as f64 / steps as f64;
                let sx = (x as f64 + frac * r * dx).round() as isize;
                let sy = (y as f64 + frac * r * dy).round() as isize;

                let cx = sx.clamp(0, w.saturating_sub(1) as isize) as usize;
                let cy = sy.clamp(0, h.saturating_sub(1) as isize) as usize;
                let p = original.get(cy * w + cx).copied().unwrap_or([0, 0, 0, 0]);

                sum_r += p[0] as f32;
                sum_g += p[1] as f32;
                sum_b += p[2] as f32;
                sum_a += p[3] as f32;
            }

            if let Some(out) = px.get_mut(y * w + x) {
                out[0] = (sum_r / count).round().clamp(0.0, 255.0) as u8;
                out[1] = (sum_g / count).round().clamp(0.0, 255.0) as u8;
                out[2] = (sum_b / count).round().clamp(0.0, 255.0) as u8;
                out[3] = (sum_a / count).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

/// Radial zoom blur: blurs radiating outward/inward from center `(cx, cy)` by `amount` (0..100).
pub fn radial_zoom_blur(px: &mut [[u8; 4]], w: usize, h: usize, cx: f64, cy: f64, amount: f64) {
    if w == 0 || h == 0 || px.len() < w * h {
        return;
    }
    let amt = (amount / 100.0).clamp(0.0, 1.0);
    if amt <= 0.0 {
        return;
    }

    let passes = 16;
    let original = px.to_vec();

    for y in 0..h {
        for x in 0..w {
            let vx = x as f64 - cx;
            let vy = y as f64 - cy;

            let mut sum_r = 0.0f32;
            let mut sum_g = 0.0f32;
            let mut sum_b = 0.0f32;
            let mut sum_a = 0.0f32;

            for step in 0..passes {
                let scale = 1.0 - amt * (step as f64 / passes as f64) * 0.5;
                let sx = (cx + vx * scale).round() as isize;
                let sy = (cy + vy * scale).round() as isize;

                let px_val = if sx >= 0 && sx < w as isize && sy >= 0 && sy < h as isize {
                    original.get((sy as usize) * w + (sx as usize)).copied().unwrap_or([0, 0, 0, 0])
                } else {
                    [0, 0, 0, 0]
                };

                sum_r += px_val[0] as f32;
                sum_g += px_val[1] as f32;
                sum_b += px_val[2] as f32;
                sum_a += px_val[3] as f32;
            }

            if let Some(out) = px.get_mut(y * w + x) {
                out[0] = (sum_r / passes as f32).round().clamp(0.0, 255.0) as u8;
                out[1] = (sum_g / passes as f32).round().clamp(0.0, 255.0) as u8;
                out[2] = (sum_b / passes as f32).round().clamp(0.0, 255.0) as u8;
                out[3] = (sum_a / passes as f32).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

/// Multiplies each channel by (r, g, b, a) multipliers while preserving premultiplied RGBA bounds.
pub fn colour_scale(px: &mut [[u8; 4]], r: f64, g: f64, b: f64, a: f64) {
    let rf = r.max(0.0) as f32;
    let gf = g.max(0.0) as f32;
    let bf = b.max(0.0) as f32;
    let af = a.clamp(0.0, 100.0) as f32;

    for p in px.iter_mut() {
        let new_a = (p[3] as f32 * af).round().clamp(0.0, 255.0) as u8;
        p[0] = (p[0] as f32 * rf * af).round().clamp(0.0, new_a as f32) as u8;
        p[1] = (p[1] as f32 * gf * af).round().clamp(0.0, new_a as f32) as u8;
        p[2] = (p[2] as f32 * bf * af).round().clamp(0.0, new_a as f32) as u8;
        p[3] = new_a;
    }
}

/// Multiplies pixel alpha by `(1.0 - amount / 100)` and scales RGB to preserve premultiplication.
pub fn transparency(px: &mut [[u8; 4]], amount: f64) {
    let t = (amount / 100.0).clamp(0.0, 1.0) as f32;
    let factor = 1.0 - t;
    for p in px.iter_mut() {
        p[0] = (p[0] as f32 * factor).round() as u8;
        p[1] = (p[1] as f32 * factor).round() as u8;
        p[2] = (p[2] as f32 * factor).round() as u8;
        p[3] = (p[3] as f32 * factor).round() as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greyscale_100_percent_pure_red_gives_luma_54_plus_minus_1() {
        let mut px = vec![[255, 0, 0, 255]];
        greyscale(&mut px, 100.0);
        let luma = px[0][0];
        assert_eq!(px[0][0], px[0][1]);
        assert_eq!(px[0][1], px[0][2]);
        assert!((luma as i32 - 54).abs() <= 1, "expected 54 ± 1, got {luma}");
    }

    #[test]
    fn box_blur_symmetric_energy_within_1_percent() {
        let w = 51;
        let h = 51;
        let cx = 25;
        let cy = 25;

        let mut px_box = vec![[0u8, 0u8, 0u8, 0u8]; w * h];
        px_box[cy * w + cx] = [255, 255, 255, 255];
        box_blur(&mut px_box, w, h, 10.0);

        let mut left_energy = 0.0f64;
        let mut right_energy = 0.0f64;
        let mut top_energy = 0.0f64;
        let mut bottom_energy = 0.0f64;
        let mut total_energy = 0.0f64;

        for y in 0..h {
            for x in 0..w {
                let val = px_box[y * w + x][0] as f64;
                total_energy += val;
                if x < cx {
                    left_energy += val;
                } else if x > cx {
                    right_energy += val;
                }
                if y < cy {
                    top_energy += val;
                } else if y > cy {
                    bottom_energy += val;
                }
            }
        }

        assert!(total_energy > 0.0, "box blur should produce energy");
        let lr_diff = (left_energy - right_energy).abs() / total_energy;
        let tb_diff = (top_energy - bottom_energy).abs() / total_energy;

        assert!(lr_diff < 0.01, "box blur left/right asymmetry {lr_diff:.4} > 1%");
        assert!(tb_diff < 0.01, "box blur top/bottom asymmetry {tb_diff:.4} > 1%");
    }

    #[test]
    fn directional_blur_angle_0_spreads_horizontally_only() {
        let w = 31;
        let h = 31;
        let cx = 15;
        let cy = 15;
        let mut px = vec![[0u8, 0u8, 0u8, 0u8]; w * h];
        px[cy * w + cx] = [255, 255, 255, 255];

        directional_blur(&mut px, w, h, 8.0, 0.0);

        for y in 0..h {
            for x in 0..w {
                let val = px[y * w + x][0];
                if y != cy {
                    assert_eq!(val, 0, "pixel at ({x}, {y}) should be 0 for angle 0");
                }
            }
        }

        let horizontal_nonzeros = (0..w).filter(|&x| px[cy * w + x][0] > 0).count();
        assert!(horizontal_nonzeros > 1, "horizontal spread should occur");
    }

    #[test]
    fn colour_scale_multiplies_channels() {
        let mut px = vec![[100, 100, 100, 200]];
        colour_scale(&mut px, 1.0, 0.5, 0.0, 0.5);
        assert_eq!(px[0], [50, 25, 0, 100]);
    }

    #[test]
    fn transparency_reduces_alpha_and_rgb() {
        let mut px = vec![[200, 100, 50, 200]];
        transparency(&mut px, 50.0);
        assert_eq!(px[0], [100, 50, 25, 100]);
    }
}
