//! Piecewise-affine warp: paints a source RGBA image over a deformed triangle mesh, one triangle at
//! a time. The mesh deformer's pixel path.

use vectorcraft_geom::Point;

/// An RGBA8 image (straight alpha), row-major, `width * height * 4` bytes.
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// Largest width or height accepted for an input or an output image.
const MAX_SIDE: u32 = 16384;
/// Barycentric slack, so pixels on a shared edge are painted by one of the two triangles.
const EPS: f64 = 1e-9;

/// Each triangle `tris[i]` maps the source triangle at `src[..]` (source pixel coordinates,
/// 0..width, 0..height) onto the destination triangle at `dst[..]` (output pixel coordinates).
/// Returns an image of `out_w` × `out_h`, transparent where no triangle covers a pixel.
/// Pixel centres are at (x + 0.5, y + 0.5). Sampling is bilinear, clamped to the source edge.
/// Where triangles overlap, the later one wins. None when sizes don't match, an index is out of
/// range, a size is 0 or past 16384, or a point is not finite.
pub fn warp_rgba(img: &Rgba, src: &[Point], dst: &[Point], tris: &[[u32; 3]], out_w: u32, out_h: u32) -> Option<Rgba> {
    let sized = |w: u32, h: u32| (1..=MAX_SIDE).contains(&w) && (1..=MAX_SIDE).contains(&h);
    if !sized(img.width, img.height) || !sized(out_w, out_h) || src.len() != dst.len() {
        return None;
    }
    let img_len = usize::try_from(img.width).ok()?.checked_mul(usize::try_from(img.height).ok()?)?.checked_mul(4)?;
    if img.pixels.len() != img_len {
        return None;
    }
    if src.iter().chain(dst).any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return None;
    }
    let n = src.len();
    if tris.iter().flatten().any(|&i| i as usize >= n) {
        return None;
    }
    let out_len = usize::try_from(out_w).ok()?.checked_mul(usize::try_from(out_h).ok()?)?.checked_mul(4)?;
    let mut out = vec![0u8; out_len];

    for &[i, j, k] in tris {
        let (Some(&s0), Some(&s1), Some(&s2)) = (src.get(i as usize), src.get(j as usize), src.get(k as usize)) else {
            return None;
        };
        let (Some(&d0), Some(&d1), Some(&d2)) = (dst.get(i as usize), dst.get(j as usize), dst.get(k as usize)) else {
            return None;
        };
        let det = (d1.x - d0.x) * (d2.y - d0.y) - (d2.x - d0.x) * (d1.y - d0.y);
        if (det * 0.5).abs() < 1e-9 {
            continue;
        }
        let x_lo = d0.x.min(d1.x).min(d2.x).floor().max(0.0).min(out_w as f64) as u32;
        let x_hi = d0.x.max(d1.x).max(d2.x).ceil().max(0.0).min(out_w as f64) as u32;
        let y_lo = d0.y.min(d1.y).min(d2.y).floor().max(0.0).min(out_h as f64) as u32;
        let y_hi = d0.y.max(d1.y).max(d2.y).ceil().max(0.0).min(out_h as f64) as u32;
        for y in y_lo..y_hi {
            for x in x_lo..x_hi {
                let p = Point::new(x as f64 + 0.5, y as f64 + 0.5);
                let l1 = ((p.x - d0.x) * (d2.y - d0.y) - (p.y - d0.y) * (d2.x - d0.x)) / det;
                let l2 = ((d1.x - d0.x) * (p.y - d0.y) - (d1.y - d0.y) * (p.x - d0.x)) / det;
                let l0 = 1.0 - l1 - l2;
                if l0 < -EPS || l1 < -EPS || l2 < -EPS {
                    continue;
                }
                let qx = l0 * s0.x + l1 * s1.x + l2 * s2.x;
                let qy = l0 * s0.y + l1 * s1.y + l2 * s2.y;
                let c = sample(img, qx, qy);
                let idx = ((y as usize) * (out_w as usize) + x as usize) * 4;
                if let Some(px) = out.get_mut(idx..idx + 4) {
                    px.copy_from_slice(&c);
                }
            }
        }
    }
    Some(Rgba { width: out_w, height: out_h, pixels: out })
}

/// Bilinear sample at source pixel coordinates `(qx, qy)` (pixel centres at +0.5), clamped to the
/// edge. Colour is interpolated premultiplied by alpha, so transparent pixels do not bleed colour.
fn sample(img: &Rgba, qx: f64, qy: f64) -> [u8; 4] {
    let max_x = (img.width - 1) as usize;
    let max_y = (img.height - 1) as usize;
    let ux = (qx - 0.5).clamp(0.0, max_x as f64);
    let uy = (qy - 0.5).clamp(0.0, max_y as f64);
    let x0 = ux.floor() as usize;
    let y0 = uy.floor() as usize;
    let fx = ux - x0 as f64;
    let fy = uy - y0 as f64;
    let x1 = (x0 + 1).min(max_x);
    let y1 = (y0 + 1).min(max_y);

    // Premultiplied r, g, b and alpha, each in 0..=1.
    let mut acc = [0.0f64; 4];
    for (x, y, w) in [(x0, y0, (1.0 - fx) * (1.0 - fy)), (x1, y0, fx * (1.0 - fy)), (x0, y1, (1.0 - fx) * fy), (x1, y1, fx * fy)] {
        let i = (y * (img.width as usize) + x) * 4;
        let Some(Ok(px)) = img.pixels.get(i..i + 4).map(<[u8; 4]>::try_from) else {
            continue;
        };
        let a = px[3] as f64 / 255.0;
        acc[0] += w * a * (px[0] as f64 / 255.0);
        acc[1] += w * a * (px[1] as f64 / 255.0);
        acc[2] += w * a * (px[2] as f64 / 255.0);
        acc[3] += w * a;
    }
    let alpha = acc[3];
    if alpha <= 0.0 {
        return [0, 0, 0, 0];
    }
    [to_u8(acc[0] / alpha), to_u8(acc[1] / alpha), to_u8(acc[2] / alpha), to_u8(alpha)]
}

fn to_u8(unit: f64) -> u8 {
    (unit * 255.0).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    /// Opaque image with a distinct colour per pixel.
    fn grid(w: u32, h: u32) -> Rgba {
        let mut pixels = Vec::new();
        for y in 0..h {
            for x in 0..w {
                pixels.extend_from_slice(&[(x * 60 + 10) as u8, (y * 60 + 20) as u8, ((x + y) * 30 + 5) as u8, 255]);
            }
        }
        Rgba { width: w, height: h, pixels }
    }

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> [Point; 4] {
        [p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)]
    }

    const QUAD: [[u32; 3]; 2] = [[0, 1, 2], [0, 2, 3]];

    fn at(img: &Rgba, x: usize, y: usize) -> [u8; 4] {
        let i = (y * img.width as usize + x) * 4;
        <[u8; 4]>::try_from(&img.pixels[i..i + 4]).unwrap()
    }

    #[test]
    fn identity_keeps_pixels_in_either_winding() {
        let img = grid(4, 4);
        let s = square(0.0, 0.0, 4.0, 4.0);
        let out = warp_rgba(&img, &s, &s, &QUAD, 4, 4).unwrap();
        assert_eq!(out.pixels, img.pixels);
        let flipped = [[0, 2, 1], [0, 3, 2]];
        let out = warp_rgba(&img, &s, &s, &flipped, 4, 4).unwrap();
        assert_eq!(out.pixels, img.pixels);
    }

    #[test]
    fn whole_pixel_translation_moves_pixels() {
        let img = grid(4, 4);
        let src = square(0.0, 0.0, 4.0, 4.0);
        let dst = square(2.0, 1.0, 6.0, 5.0);
        let out = warp_rgba(&img, &src, &dst, &QUAD, 6, 5).unwrap();
        assert_eq!((out.width, out.height), (6, 5));
        for y in 0..5 {
            for x in 0..6 {
                let got = at(&out, x, y);
                if x >= 2 && y >= 1 {
                    assert_eq!(got, at(&img, x - 2, y - 1), "pixel {x},{y}");
                } else {
                    assert_eq!(got, [0, 0, 0, 0], "pixel {x},{y}");
                }
            }
        }
    }

    #[test]
    fn double_scale_covers_output_and_keeps_corners() {
        let img = grid(2, 2);
        let out = warp_rgba(&img, &square(0.0, 0.0, 2.0, 2.0), &square(0.0, 0.0, 4.0, 4.0), &QUAD, 4, 4).unwrap();
        assert!(out.pixels.iter().skip(3).step_by(4).all(|&a| a == 255));
        assert_eq!(at(&out, 0, 0), at(&img, 0, 0));
        assert_eq!(at(&out, 3, 0), at(&img, 1, 0));
        assert_eq!(at(&out, 3, 3), at(&img, 1, 1));
        assert_eq!(at(&out, 0, 3), at(&img, 0, 1));
    }

    #[test]
    fn bad_input_returns_none() {
        let img = grid(4, 4);
        let s = square(0.0, 0.0, 4.0, 4.0);
        assert!(warp_rgba(&img, &s, &s[..3], &QUAD, 4, 4).is_none());
        assert!(warp_rgba(&img, &s, &s, &[[0, 1, 9]], 4, 4).is_none());
        assert!(warp_rgba(&img, &s, &s, &QUAD, 0, 4).is_none());
        let mut nan = s;
        nan[2] = p(f64::NAN, 4.0);
        assert!(warp_rgba(&img, &nan, &s, &QUAD, 4, 4).is_none());
        assert!(warp_rgba(&img, &s, &s, &QUAD, MAX_SIDE + 1, 4).is_none());
    }

    #[test]
    fn collapsed_triangle_draws_nothing() {
        let img = grid(4, 4);
        let src = [p(0.0, 0.0), p(4.0, 0.0), p(0.0, 4.0)];
        let dst = [p(0.0, 0.0), p(2.0, 2.0), p(4.0, 4.0)];
        let out = warp_rgba(&img, &src, &dst, &[[0, 1, 2]], 4, 4).unwrap();
        assert!(out.pixels.iter().all(|&v| v == 0));
    }
}
