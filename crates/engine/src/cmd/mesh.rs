//! 7rrek (Creative Studio): the mesh deformer. A triangle mesh over a piece of art, with pins
//! (handles) keyed per frame; moving pins bends the mesh as rigidly as it can, after
//! "As-Rigid-As-Possible Shape Manipulation" (Igarashi, Moscovich, Hughes, SIGGRAPH 2005),
//! written fresh from the paper. Meshes live in the document's preserved extra data (`"meshes"`).
//!
//! The solve has the paper's two steps:
//! 1. Similarity: each triangle keeps each vertex where the opposite edge's local frame
//!    (x along the edge, y across it) puts it at rest. Rotation and uniform scale are free, so this
//!    is linear in the vertices: a quadratic error minimised with the pins held.
//! 2. Scale adjustment: each step-1 triangle is replaced by its rest shape, turned to fit it best
//!    (rigid 2D Procrustes: no scale), and the vertices are solved so every edge matches its
//!    fitted edge, pins held.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use vectorcraft_doc::{Document, ImageBlob, ImageObject, Node, NodeKind};
use vectorcraft_geom::{Affine, Point, Rect, Vec2};

const KEY: &str = "meshes";
/// Most meshes a document takes.
pub const MAX_MESHES: usize = 256;
/// Most vertices one mesh takes (the solve is sparse, so this bounds its time, not a dense matrix).
pub const MAX_VERTICES: usize = 4096;
/// Most triangles one mesh takes.
pub const MAX_TRIANGLES: usize = 2 * MAX_VERTICES;
/// Below this a triangle edge or area counts as collapsed and the triangle is left out.
const EPS: f64 = 1e-9;

/// A mesh over one node's art: rest vertices (document points), triangles over them, the pinned
/// vertices, and per-frame keys of where the pins are (one point per pin, in `pins` order).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Mesh {
    pub node: u64,
    pub rest: Vec<[f64; 2]>,
    pub tris: Vec<[u32; 3]>,
    #[serde(default)]
    pub pins: Vec<u32>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub keys: BTreeMap<u32, Vec<[f64; 2]>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Meshes {
    pub meshes: Vec<Mesh>,
}

impl Meshes {
    /// The document's meshes; none if any is broken (an index out of range, a bad number).
    pub fn of(doc: &Document) -> Self {
        let m: Meshes = doc.unknown.get(KEY).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
        if m.valid() { m } else { Meshes::default() }
    }

    pub fn store(&self, doc: &mut Document) {
        if self.meshes.is_empty() {
            doc.unknown.remove(KEY);
        } else if let Ok(v) = serde_json::to_value(self) {
            doc.unknown.insert(KEY.into(), v);
        }
    }

    pub fn valid(&self) -> bool {
        self.meshes.len() <= MAX_MESHES && self.meshes.iter().all(Mesh::valid)
    }

    /// The mesh over `node`, if any.
    pub fn of_node(&self, node: u64) -> Option<&Mesh> {
        self.meshes.iter().find(|m| m.node == node)
    }
}

fn finite(p: &[f64; 2]) -> bool {
    p.iter().all(|v| v.is_finite())
}

fn pt(p: &[f64; 2]) -> Point {
    Point::new(p[0], p[1])
}

impl Mesh {
    /// A `cols` × `rows` grid of cells over `r`, two triangles per cell. None when the counts are
    /// zero, the rect is empty or the grid would pass [`MAX_VERTICES`].
    pub fn grid(node: u64, r: Rect, cols: u32, rows: u32) -> Option<Self> {
        let (c, w) = (cols as usize, rows as usize);
        let ok = c > 0 && w > 0 && (c + 1).saturating_mul(w + 1) <= MAX_VERTICES;
        if !ok || !(r.width() > EPS && r.height() > EPS) || ![r.x0, r.y0, r.x1, r.y1].iter().all(|v| v.is_finite()) {
            return None;
        }
        let rest = (0..=rows)
            .flat_map(|j| (0..=cols).map(move |i| (i, j)))
            .map(|(i, j)| [r.x0 + r.width() * i as f64 / cols as f64, r.y0 + r.height() * j as f64 / rows as f64])
            .collect();
        let at = |i: u32, j: u32| j * (cols + 1) + i;
        let tris = (0..rows)
            .flat_map(|j| (0..cols).map(move |i| (i, j)))
            .flat_map(|(i, j)| [[at(i, j), at(i + 1, j), at(i + 1, j + 1)], [at(i, j), at(i + 1, j + 1), at(i, j + 1)]])
            .collect();
        Some(Mesh { node, rest, tris, pins: Vec::new(), keys: BTreeMap::new() })
    }

    pub fn valid(&self) -> bool {
        let n = self.rest.len();
        n <= MAX_VERTICES
            && self.tris.len() <= MAX_TRIANGLES
            && self.rest.iter().all(finite)
            && self.tris.iter().all(|t| t.iter().all(|&v| (v as usize) < n))
            && self.pins.iter().enumerate().all(|(i, &v)| (v as usize) < n && !self.pins[..i].contains(&v))
            && self.keys.values().all(|k| k.len() == self.pins.len() && k.iter().all(finite))
    }

    /// Where each pin is at `frame`: held before the first key and after the last, straight in
    /// between. No keys: the pins rest.
    pub fn pins_at(&self, frame: u32) -> Vec<Point> {
        let rest = || self.pins.iter().filter_map(|&v| self.rest.get(v as usize)).map(pt).collect();
        let before = self.keys.range(..=frame).next_back();
        let after = self.keys.range(frame..).next();
        match (before, after) {
            (Some((fa, a)), Some((fb, b))) if fb > fa => {
                let t = (frame - fa) as f64 / (fb - fa) as f64;
                a.iter().zip(b).map(|(p, q)| pt(p).lerp(pt(q), t)).collect()
            }
            (Some((_, k)), _) | (None, Some((_, k))) => k.iter().map(pt).collect(),
            (None, None) => rest(),
        }
    }

    /// The vertices at `frame`, or None when the mesh rests there (no pin moved).
    pub fn at(&self, frame: u32) -> Option<Vec<Point>> {
        let rest: Vec<Point> = self.rest.iter().map(pt).collect();
        let targets = self.pins_at(frame);
        let moved = self.pins.iter().zip(&targets).any(|(&v, t)| rest.get(v as usize).is_some_and(|r| (*r - *t).hypot() > EPS));
        if !moved {
            return None;
        }
        let pins: Vec<(usize, Point)> = self.pins.iter().map(|&v| v as usize).zip(targets).collect();
        arap(&rest, &self.tris, &pins)
    }
}

/// A mesh's map from rest to deformed at one frame: each point moves with the rest triangle it is
/// in (the deformation is affine inside each triangle), and a point outside the mesh with the
/// triangle it is closest to being inside.
pub struct Warp {
    rest: Vec<Point>,
    now: Vec<Point>,
    tris: Vec<[u32; 3]>,
}

impl Warp {
    /// The mesh's map at `frame`, None when it rests there.
    pub fn of(m: &Mesh, frame: u32) -> Option<Self> {
        let now = m.at(frame)?;
        let rest: Vec<Point> = m.rest.iter().map(pt).collect();
        let tris = live(&rest, &m.tris).into_iter().filter_map(|i| m.tris.get(i).copied()).collect();
        Some(Warp { rest, now, tris })
    }

    pub fn map(&self, p: Point) -> Point {
        // Barycentric coordinates in each rest triangle; the one whose smallest is largest holds p
        // (or, outside the mesh, is nearest to holding it).
        let mut best: Option<(f64, [f64; 3], [Point; 3])> = None;
        for t in &self.tris {
            let (Some([a, b, c]), Some(now)) = (corners(&self.rest, t), corners(&self.now, t)) else { continue };
            let det = (b - a).cross(c - a);
            if det.abs() <= EPS {
                continue;
            }
            let l1 = (p - a).cross(c - a) / det;
            let l2 = (b - a).cross(p - a) / det;
            let l = [1.0 - l1 - l2, l1, l2];
            let worst = l.iter().copied().fold(f64::INFINITY, f64::min);
            if best.as_ref().is_none_or(|(w, _, _)| worst > *w) {
                best = Some((worst, l, now));
            }
            if worst >= 0.0 {
                break;
            }
        }
        match best {
            Some((_, [l0, l1, l2], [a, b, c])) => Point::new(l0 * a.x + l1 * b.x + l2 * c.x, l0 * a.y + l1 * b.y + l2 * c.y),
            None => p,
        }
    }
}

/// Bends the paths and images in `n` (and under it) that sit under a node with a warp in `warps`,
/// keyed by node id. Paths are split first so long segments curve with the mesh, and leave their
/// live shape; images are redrawn over the bent mesh (see [`warp_image`]) into `images`.
pub(crate) fn deform(n: &mut Node, warps: &BTreeMap<u64, Warp>, active: Option<&Warp>, images: &mut BTreeMap<String, ImageBlob>) {
    let active = warps.get(&n.id.0).or(active);
    let id = n.id.0;
    if let (Some(w), NodeKind::Image(im)) = (active, &mut n.kind) {
        warp_image(im, w, id, images);
    }
    if let Some(w) = active {
        if let NodeKind::Path { live, .. } = &mut n.kind {
            *live = None;
        }
        if let Some(path) = n.path_data_mut() {
            for sp in &mut path.subpaths {
                super::rig::densify(sp);
                for k in &mut sp.anchors {
                    (k.p, k.h_in, k.h_out) = (w.map(k.p), w.map(k.h_in), w.map(k.h_out));
                }
            }
        }
    }
    if let Some(children) = n.children_mut() {
        for c in children {
            deform(std::sync::Arc::make_mut(c), warps, active, images);
        }
    }
}

/// Most pixels a side of a bent image gets.
const MAX_WARP_SIDE: f64 = 4096.0;

/// Redraws image `im` (node `id`) over the bent mesh at about its own resolution, as a PNG kept in
/// `images` under one key per node (the next frame shown replaces it; saving drops it with the
/// other unused images). Left as it is when its pixels don't decode or its placement is degenerate.
fn warp_image(im: &mut ImageObject, w: &Warp, id: u64, images: &mut BTreeMap<String, ImageBlob>) -> Option<()> {
    let det = im.xf.determinant();
    if !det.is_finite() || det.abs() <= EPS {
        return None;
    }
    let src_img = image::load_from_memory(&images.get(&im.key)?.bytes).ok()?.to_rgba8();
    let src_img = vectorcraft_render::mesh_warp::Rgba { width: src_img.width(), height: src_img.height(), pixels: src_img.into_raw() };
    let inv = im.xf.inverse();
    let src: Vec<Point> = w.rest.iter().map(|p| inv * *p).collect();
    let (min, max) =
        w.now.iter().fold((Point::new(f64::INFINITY, f64::INFINITY), Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY)), |(lo, hi), p| {
            (Point::new(lo.x.min(p.x), lo.y.min(p.y)), Point::new(hi.x.max(p.x), hi.y.max(p.y)))
        });
    let (bw, bh) = (max.x - min.x, max.y - min.y);
    // Document points per pixel: the image's own, coarser if that would pass MAX_WARP_SIDE.
    let s = det.abs().sqrt().max(bw.max(bh) / MAX_WARP_SIDE);
    if !(bw.is_finite() && bh.is_finite() && s > EPS) {
        return None;
    }
    let (out_w, out_h) = ((bw / s).ceil().max(1.0) as u32, (bh / s).ceil().max(1.0) as u32);
    let dst: Vec<Point> = w.now.iter().map(|p| Point::new((p.x - min.x) / s, (p.y - min.y) / s)).collect();
    let out = vectorcraft_render::mesh_warp::warp_rgba(&src_img, &src, &dst, &w.tris, out_w, out_h)?;
    let mut png = Vec::new();
    image::RgbaImage::from_raw(out.width, out.height, out.pixels)?.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).ok()?;
    let key = format!("mesh-warp-{id}");
    images.insert(key.clone(), ImageBlob::png(png));
    (im.key, im.width, im.height) = (key, out.width, out.height);
    im.xf = Affine::translate(min.to_vec2()) * Affine::scale(s);
    (im.link, im.pixels) = (None, None);
    Some(())
}

/// Whether `n` or anything under it has a warp in `warps`.
pub(crate) fn warped(n: &Node, warps: &BTreeMap<u64, Warp>) -> bool {
    let mut hit = false;
    n.walk(&mut |c| hit |= warps.contains_key(&c.id.0));
    hit
}

/// A symmetric sparse matrix as rows of (column, value), built by adding entries.
struct Sparse(Vec<BTreeMap<usize, f64>>);

impl Sparse {
    fn new(n: usize) -> Self {
        Sparse((0..n).map(|_| BTreeMap::new()).collect())
    }

    fn add(&mut self, i: usize, j: usize, v: f64) {
        if let Some(row) = self.0.get_mut(i) {
            *row.entry(j).or_insert(0.0) += v;
        }
    }
}

/// Solves `m x = rhs` for the entries not `fixed`, the fixed ones keeping their values in `x`
/// (which also holds the starting guess). Conjugate gradients: `m` restricted to the free entries
/// is symmetric positive semi-definite, and a free entry no equation reaches keeps its guess.
// shortcut: solved afresh every frame (~140 ms at 3,721 vertices); factor each pin set's matrices
// once, as the paper does, when big meshes play back.
fn solve_fixed(m: &Sparse, rhs: &[f64], x: &mut [f64], fixed: &[bool]) {
    let n = x.len();
    let free = |i: usize| !fixed.get(i).copied().unwrap_or(true);
    // The free rows once, as plain vectors over the free columns, and
    // b = rhs − M_free,fixed · x_fixed.
    let mut b = vec![0.0; n];
    let rows: Vec<Vec<(usize, f64)>> = (0..n)
        .map(|i| {
            let mut row = Vec::new();
            if !free(i) {
                return row;
            }
            let mut bi = rhs.get(i).copied().unwrap_or(0.0);
            for (&j, &v) in m.0.get(i).into_iter().flatten() {
                if free(j) {
                    row.push((j, v));
                } else {
                    bi -= v * x.get(j).copied().unwrap_or(0.0);
                }
            }
            if let Some(slot) = b.get_mut(i) {
                *slot = bi;
            }
            row
        })
        .collect();
    // y = M_free,free · v (zero on fixed rows).
    let apply = |v: &[f64]| -> Vec<f64> { rows.iter().map(|r| r.iter().map(|&(j, a)| a * v.get(j).copied().unwrap_or(0.0)).sum()).collect() };
    let dot = |a: &[f64], c: &[f64]| a.iter().zip(c).map(|(p, q)| p * q).sum::<f64>();
    let xf: Vec<f64> = (0..n).map(|i| if free(i) { x.get(i).copied().unwrap_or(0.0) } else { 0.0 }).collect();
    let ax = apply(&xf);
    let mut r: Vec<f64> = b.iter().zip(&ax).map(|(p, q)| p - q).collect();
    let mut d = r.clone();
    let mut rr = dot(&r, &r);
    let tol = 1e-24 * dot(&b, &b).max(1.0);
    for _ in 0..(4 * n + 100) {
        // Converged, or a NaN crept in: stop either way.
        if rr.is_nan() || rr <= tol {
            break;
        }
        let ad = apply(&d);
        let dad = dot(&d, &ad);
        if dad.is_nan() || dad <= 0.0 {
            break;
        }
        let a = rr / dad;
        for i in 0..n {
            if let (Some(xi), Some(di), Some(ri), Some(adi)) = (x.get_mut(i), d.get(i), r.get_mut(i), ad.get(i))
                && free(i)
            {
                *xi += a * di;
                *ri -= a * adi;
            }
        }
        let rr2 = dot(&r, &r);
        let beta = rr2 / rr;
        for (di, ri) in d.iter_mut().zip(&r) {
            *di = ri + beta * *di;
        }
        rr = rr2;
    }
}

/// A triangle's corners as points, if all three index `v`.
fn corners(v: &[Point], t: &[u32; 3]) -> Option<[Point; 3]> {
    Some([*v.get(t[0] as usize)?, *v.get(t[1] as usize)?, *v.get(t[2] as usize)?])
}

/// Turned a quarter: (x, y) → (−y, x).
fn perp(v: Vec2) -> Vec2 {
    Vec2::new(-v.y, v.x)
}

/// The rest triangles that are not collapsed, as indices into `tris`.
fn live(rest: &[Point], tris: &[[u32; 3]]) -> Vec<usize> {
    (0..tris.len())
        .filter(|&i| {
            tris.get(i).and_then(|t| corners(rest, t)).is_some_and(|[a, b, c]| {
                let area = (b - a).cross(c - a).abs() / 2.0;
                area > EPS && [(b - a), (c - b), (a - c)].iter().all(|e| e.hypot() > EPS)
            })
        })
        .collect()
}

/// The mesh's vertices with each pin `(vertex, target)` moved to its target and the rest following
/// as rigidly as they can (see the module doc). No pins: the rest pose. One pin: a translation.
/// None when an input is not finite or the solve comes out not finite.
pub fn arap(rest: &[Point], tris: &[[u32; 3]], pins: &[(usize, Point)]) -> Option<Vec<Point>> {
    solve(rest, tris, pins).map(|(_, out)| out)
}

/// [`arap`]'s step-1 result and its final one.
fn solve(rest: &[Point], tris: &[[u32; 3]], pins: &[(usize, Point)]) -> Option<(Vec<Point>, Vec<Point>)> {
    let n = rest.len();
    if rest.iter().chain(pins.iter().map(|(_, p)| p)).any(|p| !(p.x.is_finite() && p.y.is_finite())) {
        return None;
    }
    let pins: Vec<(usize, Point)> = pins.iter().copied().filter(|(v, _)| *v < n).collect();
    let shift = match pins.as_slice() {
        [] => return Some((rest.to_vec(), rest.to_vec())),
        [(v, p)] => {
            let d = *p - *rest.get(*v)?;
            let moved: Vec<Point> = rest.iter().map(|r| *r + d).collect();
            return Some((moved.clone(), moved));
        }
        many => many.iter().map(|(v, p)| *p - rest.get(*v).copied().unwrap_or(*p)).fold(Vec2::ZERO, |a, d| a + d) / many.len() as f64,
    };
    let live = live(rest, tris);
    let mut fixed = vec![false; 2 * n];
    // The starting guess: everything shifted by the pins' mean move (a vertex no triangle reaches
    // stays there), the pins at their targets.
    let mut x: Vec<f64> = rest.iter().flat_map(|r| [r.x + shift.x, r.y + shift.y]).collect();
    for (v, p) in &pins {
        for (c, val) in [(0, p.x), (1, p.y)] {
            if let (Some(f), Some(xi)) = (fixed.get_mut(2 * v + c), x.get_mut(2 * v + c)) {
                (*f, *xi) = (true, val);
            }
        }
    }

    // Step 1: for each live triangle and each corner k opposite edge (i, j), the error
    // e = v_k − v_i − s (v_j − v_i) − t perp(v_j − v_i), with (s, t) the corner's rest coordinates
    // in the edge's frame. Each error component is a row of coefficients over the six coordinates;
    // G gathers Σ rowᵀ row.
    let mut g = Sparse::new(2 * n);
    for &ti in &live {
        let Some(t) = tris.get(ti) else { continue };
        for (i, j, k) in [(t[0], t[1], t[2]), (t[1], t[2], t[0]), (t[2], t[0], t[1])] {
            let (Some(pi), Some(pj), Some(pk)) = (rest.get(i as usize), rest.get(j as usize), rest.get(k as usize)) else { continue };
            let e = *pj - *pi;
            let len2 = e.hypot2();
            let r = *pk - *pi;
            let (s, tt) = (r.dot(e) / len2, r.dot(perp(e)) / len2);
            let (i, j, k) = (2 * i as usize, 2 * j as usize, 2 * k as usize);
            // v_k − (1 − s) v_i − s v_j − t perp(v_j − v_i); perp(x, y) = (−y, x).
            let rows: [[(usize, f64); 6]; 2] = [
                [(k, 1.0), (i, -(1.0 - s)), (j, -s), (j + 1, tt), (i + 1, -tt), (k + 1, 0.0)],
                [(k + 1, 1.0), (i + 1, -(1.0 - s)), (j + 1, -s), (j, -tt), (i, tt), (k, 0.0)],
            ];
            for row in &rows {
                for (a, va) in row {
                    for (b, vb) in row {
                        g.add(*a, *b, va * vb);
                    }
                }
            }
        }
    }
    let zero = vec![0.0; 2 * n];
    solve_fixed(&g, &zero, &mut x, &fixed);
    let step1: Vec<Point> = x.chunks(2).map(|c| Point::new(c.first().copied().unwrap_or(0.0), c.get(1).copied().unwrap_or(0.0))).collect();

    // Step 2: fit each rest triangle to its step-1 shape by a turn and a move, then match every
    // edge to its fitted edge (each coordinate on its own: a graph Laplacian).
    let mut h = Sparse::new(2 * n);
    let mut rhs = vec![0.0; 2 * n];
    for &ti in &live {
        let Some(t) = tris.get(ti) else { continue };
        let (Some(r), Some(p)) = (corners(rest, t), corners(&step1, t)) else { continue };
        let (cr, cp) = (r.iter().fold(Vec2::ZERO, |a, q| a + q.to_vec2()) / 3.0, p.iter().fold(Vec2::ZERO, |a, q| a + q.to_vec2()) / 3.0);
        let (mut cross, mut dot) = (0.0, 0.0);
        for (a, b) in r.iter().zip(&p) {
            let (u, w) = (a.to_vec2() - cr, b.to_vec2() - cp);
            cross += u.cross(w);
            dot += u.dot(w);
        }
        let (sin, cos) = cross.atan2(dot).sin_cos();
        let fit = |q: Point| {
            let u = q.to_vec2() - cr;
            Vec2::new(u.x * cos - u.y * sin, u.x * sin + u.y * cos)
        };
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            let (Some(&va), Some(&vb), Some(&ra), Some(&rb)) = (t.get(a), t.get(b), r.get(a), r.get(b)) else { continue };
            let edge = fit(ra) - fit(rb);
            for (c, ev) in [(0, edge.x), (1, edge.y)] {
                let (ia, ib) = (2 * va as usize + c, 2 * vb as usize + c);
                h.add(ia, ia, 1.0);
                h.add(ib, ib, 1.0);
                h.add(ia, ib, -1.0);
                h.add(ib, ia, -1.0);
                if let Some(v) = rhs.get_mut(ia) {
                    *v += ev;
                }
                if let Some(v) = rhs.get_mut(ib) {
                    *v -= ev;
                }
            }
        }
    }
    solve_fixed(&h, &rhs, &mut x, &fixed);
    let out: Vec<Point> = x.chunks(2).map(|c| Point::new(c.first().copied().unwrap_or(0.0), c.get(1).copied().unwrap_or(0.0))).collect();
    out.iter().all(|p| p.x.is_finite() && p.y.is_finite()).then_some((step1, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rot(p: Point, about: Point, deg: f64) -> Point {
        let (sin, cos) = deg.to_radians().sin_cos();
        let v = p - about;
        about + Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
    }

    fn sheet() -> Mesh {
        Mesh::grid(1, Rect::new(0.0, 0.0, 100.0, 50.0), 8, 4).unwrap()
    }

    fn rest(m: &Mesh) -> Vec<Point> {
        m.rest.iter().map(pt).collect()
    }

    /// The largest stretch or squash of any triangle edge, as |length / rest length − 1|.
    fn distortion(m: &Mesh, v: &[Point]) -> f64 {
        let r = rest(m);
        m.tris
            .iter()
            .flat_map(|t| [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])])
            .map(|(a, b)| {
                let (a, b) = (a as usize, b as usize);
                ((v[a] - v[b]).hypot() / (r[a] - r[b]).hypot() - 1.0).abs()
            })
            .fold(0.0, f64::max)
    }

    fn mean_distortion(m: &Mesh, v: &[Point]) -> f64 {
        let r = rest(m);
        let e: Vec<f64> = m
            .tris
            .iter()
            .flat_map(|t| [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])])
            .map(|(a, b)| {
                let (a, b) = (a as usize, b as usize);
                ((v[a] - v[b]).hypot() / (r[a] - r[b]).hypot() - 1.0).abs()
            })
            .collect();
        e.iter().sum::<f64>() / e.len() as f64
    }

    #[test]
    fn grid_counts_and_bad_grids() {
        let m = sheet();
        assert_eq!((m.rest.len(), m.tris.len()), (45, 64));
        assert!(m.valid());
        assert!(Mesh::grid(1, Rect::new(0.0, 0.0, 0.0, 10.0), 2, 2).is_none());
        assert!(Mesh::grid(1, Rect::new(0.0, 0.0, 10.0, 10.0), 0, 2).is_none());
        assert!(Mesh::grid(1, Rect::new(0.0, 0.0, 10.0, 10.0), 100, 100).is_none(), "past MAX_VERTICES");
    }

    #[test]
    fn pins_left_alone_keep_the_rest_pose() {
        let m = sheet();
        let r = rest(&m);
        let pins: Vec<(usize, Point)> = [0, 8, 44].iter().map(|&v| (v, r[v])).collect();
        let out = arap(&r, &m.tris, &pins).unwrap();
        assert!(out.iter().zip(&r).all(|(a, b)| (*a - *b).hypot() < 1e-6));
        assert_eq!(arap(&r, &m.tris, &[]).unwrap(), r);
    }

    #[test]
    fn one_pin_moves_the_whole_mesh() {
        let m = sheet();
        let r = rest(&m);
        let out = arap(&r, &m.tris, &[(4, r[4] + Vec2::new(10.0, -5.0))]).unwrap();
        assert!(out.iter().zip(&r).all(|(a, b)| (*a - (*b + Vec2::new(10.0, -5.0))).hypot() < 1e-9));
    }

    #[test]
    fn three_pins_moved_rigidly_move_the_mesh_rigidly() {
        // The paper's check: handles carried by a rigid motion leave every triangle undeformed.
        let m = sheet();
        let r = rest(&m);
        let about = Point::new(30.0, 20.0);
        let go = |p: Point| rot(p, about, 30.0) + Vec2::new(15.0, 40.0);
        let pins: Vec<(usize, Point)> = [0, 8, 40].iter().map(|&v| (v, go(r[v]))).collect();
        let out = arap(&r, &m.tris, &pins).unwrap();
        for (a, b) in out.iter().zip(&r) {
            assert!((*a - go(*b)).hypot() < 1e-6, "{a:?} vs {:?}", go(*b));
        }
    }

    #[test]
    fn dragging_one_of_three_pins_bends_without_stretching_much() {
        // Pin the left corners, lift the right middle 30 pt: the pins land exactly, the left end
        // stays put, edges keep close to their rest length on average (the worst ones sit next to
        // the pins), and step 2 is closer to rigid than step 1 alone (the scale step's point).
        let m = sheet();
        let r = rest(&m);
        let target = r[26] + Vec2::new(0.0, -30.0);
        let pins = [(0, r[0]), (36, r[36]), (26, target)];
        let (step1, out) = solve(&r, &m.tris, &pins).unwrap();
        for (v, p) in pins {
            assert!((out[v] - p).hypot() < 1e-6, "pin {v}");
        }
        assert!((out[18] - r[18]).hypot() < 3.0, "the left edge stays near rest: {:?}", out[18]);
        assert!(out[44].y < r[44].y - 10.0, "the right end lifts with the handle");
        let mean = mean_distortion(&m, &out);
        assert!(mean < 0.05, "edges keep their length: {mean}");
        assert!(distortion(&m, &out) < 0.3);
        assert!(mean < mean_distortion(&m, &step1), "step 2 is more rigid than step 1");
    }

    #[test]
    fn pins_pulled_far_apart_stay_finite() {
        let m = sheet();
        let r = rest(&m);
        let pins = [(0, Point::new(-1e5, 0.0)), (44, Point::new(1e5, 1e5)), (22, r[22])];
        let out = arap(&r, &m.tris, &pins).unwrap();
        assert!(out.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
        assert!(arap(&r, &m.tris, &[(0, Point::new(f64::NAN, 0.0)), (1, r[1])]).is_none());
        assert!(arap(&r, &m.tris, &[(9999, r[0]), (1, r[1])]).is_some(), "a pin off the mesh is ignored");
    }

    #[test]
    fn keys_ease_straight_and_survive_storage() {
        let mut m = sheet();
        m.pins = vec![0, 44];
        m.keys.insert(1, vec![m.rest[0], m.rest[44]]);
        m.keys.insert(11, vec![m.rest[0], [m.rest[44][0], m.rest[44][1] + 20.0]]);
        assert!(m.at(1).is_none(), "at rest on its first key");
        assert_eq!(m.pins_at(6)[1], Point::new(100.0, 60.0));
        assert!(m.at(6).is_some());
        let mut d = Document::new(100.0, 100.0);
        Meshes { meshes: vec![m.clone()] }.store(&mut d);
        assert_eq!(Meshes::of(&d).of_node(1), Some(&m));
        m.pins.push(9999);
        Meshes { meshes: vec![m] }.store(&mut d);
        assert!(Meshes::of(&d).meshes.is_empty(), "a broken mesh is dropped");
    }

    #[test]
    fn the_frame_shown_bends_meshed_art_and_leaves_the_original() {
        use serde_json::json;
        let mut s = crate::Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        let id = s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 100, "height": 50})).unwrap()["id"].as_u64().unwrap();
        let mut m = Mesh::grid(id, Rect::new(0.0, 0.0, 100.0, 50.0), 8, 4).unwrap();
        m.pins = vec![0, 36, 26];
        m.keys.insert(1, vec![m.rest[0], m.rest[36], m.rest[26]]);
        m.keys.insert(9, vec![m.rest[0], m.rest[36], [100.0, -5.0]]);
        s.edit("Mesh", |d, _| {
            Meshes { meshes: vec![m] }.store(d);
            Ok(())
        })
        .unwrap();
        let anchors = |d: &Document, visible: bool| {
            let mut v = Vec::new();
            d.layers[0].walk(&mut |n| {
                if let (Some(p), true) = (n.path_data(), n.id.0 == id || visible) {
                    v.extend(p.subpaths.iter().flat_map(|sp| sp.anchors.iter().map(|k| k.p)));
                }
            });
            v
        };
        let doc = s.doc().unwrap().doc.clone();
        let bent = crate::cmd::anim::at_frame(&doc, 9);
        let shown: Vec<Point> = {
            let mut v = Vec::new();
            for c in bent.layers[0].children().into_iter().flatten().filter(|c| c.visible) {
                c.walk(&mut |n| {
                    if let Some(p) = n.path_data() {
                        v.extend(p.subpaths.iter().flat_map(|sp| sp.anchors.iter().map(|k| k.p)));
                    }
                });
            }
            v
        };
        assert!(shown.iter().any(|p| p.y < -10.0), "the right end lifts: {shown:?}");
        assert!(shown.iter().filter(|p| p.x < 1.0).all(|p| p.y > -1.0 && p.y < 51.0), "the pinned left edge stays");
        assert_eq!(anchors(&bent, false), anchors(&doc, false), "the original keeps its points");
        let rest = crate::cmd::anim::at_frame(&bent, 1);
        assert!(anchors(&rest, true).iter().all(|p| p.y > -1.0), "frame 1 rests");
        let again = crate::cmd::anim::at_frame(&rest, 9);
        let again_shown = {
            let mut v = Vec::new();
            for c in again.layers[0].children().into_iter().flatten().filter(|c| c.visible) {
                c.walk(&mut |n| {
                    if let Some(p) = n.path_data() {
                        v.extend(p.subpaths.iter().flat_map(|sp| sp.anchors.iter().map(|k| k.p)));
                    }
                });
            }
            v
        };
        assert_eq!(again_shown, shown, "bent once only");
    }

    #[test]
    fn the_frame_shown_redraws_a_meshed_image_bent() {
        use serde_json::json;
        let mut s = crate::Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        // 20×10 pixels at 5 pt each: red on the left half, blue on the right.
        let img = image::RgbaImage::from_fn(20, 10, |x, _| if x < 10 { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([0, 0, 255, 255]) });
        let mut png = vec![];
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let id = s
            .edit("Place", |d, _| {
                d.images.insert("pic".into(), ImageBlob::png(png));
                let id = d.alloc_id();
                let im = ImageObject {
                    key: "pic".into(),
                    width: 20,
                    height: 10,
                    xf: Affine::scale(5.0),
                    link: None,
                    placement: Default::default(),
                    pixels: None,
                };
                d.insert(d.default_layer(), 0, Node::new(id, NodeKind::Image(Box::new(im))))?;
                Ok(id)
            })
            .unwrap();
        let mut m = Mesh::grid(id.0, Rect::new(0.0, 0.0, 100.0, 50.0), 8, 4).unwrap();
        m.pins = vec![0, 36, 26];
        m.keys.insert(9, vec![m.rest[0], m.rest[36], [100.0, -5.0]]);
        s.edit("Mesh", |d, _| {
            Meshes { meshes: vec![m] }.store(d);
            Ok(())
        })
        .unwrap();
        let bent = crate::cmd::anim::at_frame(&s.doc().unwrap().doc, 9);
        let mut shown = Vec::new();
        bent.layers[0].walk(&mut |n| {
            if let NodeKind::Image(im) = &n.kind {
                shown.push((n.id, (**im).clone()));
            }
        });
        let original = shown.iter().find(|(n, _)| *n == id).map(|(_, im)| im.clone()).unwrap();
        assert_eq!((original.key.as_str(), original.xf), ("pic", Affine::scale(5.0)), "the original stays");
        let (_, im) = shown.iter().find(|(n, _)| *n != id).unwrap();
        assert_eq!(im.key, format!("mesh-warp-{}", id.0));
        let r = im.xf.transform_rect_bbox(Rect::new(0.0, 0.0, im.width as f64, im.height as f64));
        assert!(r.y0 < -10.0 && (r.x0).abs() < 1.0, "the bent image reaches up: {r:?}");
        let px = image::load_from_memory(&bent.images[&im.key].bytes).unwrap().to_rgba8();
        let has = |c: [u8; 4]| px.pixels().any(|p| p.0 == c);
        assert!(has([255, 0, 0, 255]) && has([0, 0, 255, 255]), "both halves drawn");
        assert!(has([0, 0, 0, 0]), "the bent shape leaves corners of its box clear");
    }
}
#[cfg(test)]
mod timing {
    use super::*;
    #[test]
    #[ignore]
    fn big() {
        let m = Mesh::grid(1, Rect::new(0.0, 0.0, 600.0, 600.0), 60, 60).unwrap();
        let r: Vec<Point> = m.rest.iter().map(pt).collect();
        let n = r.len();
        let pins = [(0, r[0]), (60, r[60]), (n - 1, r[n - 1] + Vec2::new(-100.0, -150.0))];
        let t = std::time::Instant::now();
        let out = arap(&r, &m.tris, &pins).unwrap();
        eprintln!("{n} vertices: {:?}, {:?}", t.elapsed(), out[n - 1]);
    }
}
