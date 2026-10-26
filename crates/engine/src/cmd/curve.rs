//! 7rrek (Creative Studio): curve and envelope deformers. A chain of cubic Bezier segments (open:
//! a curve, for hair and tails; closed: an envelope around the art) keyed per frame; art near the
//! chain rides it. Each art point is placed by every segment's nearest point at rest (how far along,
//! how far across) and carried to the same place on the segment now; segments weigh in by
//! closeness, so the art bends smoothly where segments meet. Past the influence radius the art
//! fades back to rest. Deformers live in the document's preserved extra data (`"curves"`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use vectorcraft_doc::{Document, Node};
use vectorcraft_geom::{Point, Rect, Vec2};

const KEY: &str = "curves";
/// Most deformers a document takes.
pub const MAX_CURVES: usize = 256;
/// Most segments one deformer takes.
pub const MAX_SEGMENTS: usize = 256;
const EPS: f64 = 1e-9;
/// Grid cells per side when a deformer is turned into a mesh warp (for images).
const GRID: u32 = 24;

/// A deformer over one node's art. `rest` is the chain at rest as Bezier points: anchor, out
/// handle, in handle, anchor, … (3n + 1 for n segments open; a closed chain's last segment runs
/// back to the first anchor, so 3n). `keys` hold the same layout per frame.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Curve {
    pub node: u64,
    pub rest: Vec<[f64; 2]>,
    #[serde(default)]
    pub closed: bool,
    /// Art farther than this from the chain stays at rest; None: no limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
    /// Width of the fade past `radius`.
    #[serde(default)]
    pub fade: f64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub keys: BTreeMap<u32, Vec<[f64; 2]>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Curves {
    pub curves: Vec<Curve>,
}

impl Curves {
    /// The document's deformers; none if any is broken.
    pub fn of(doc: &Document) -> Self {
        let c: Curves = doc.unknown.get(KEY).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
        if c.valid() { c } else { Curves::default() }
    }

    pub fn store(&self, doc: &mut Document) {
        if self.curves.is_empty() {
            doc.unknown.remove(KEY);
        } else if let Ok(v) = serde_json::to_value(self) {
            doc.unknown.insert(KEY.into(), v);
        }
    }

    pub fn valid(&self) -> bool {
        self.curves.len() <= MAX_CURVES && self.curves.iter().all(Curve::valid)
    }
}

fn pt(p: &[f64; 2]) -> Point {
    Point::new(p[0], p[1])
}

/// Segment count for `len` chain points, None when the layout is wrong.
fn segments(len: usize, closed: bool) -> Option<usize> {
    let n = if closed { (len % 3 == 0).then_some(len / 3)? } else { (len % 3 == 1).then_some(len / 3)? };
    (n >= 1 && n <= MAX_SEGMENTS && (!closed || n >= 2)).then_some(n)
}

impl Curve {
    pub fn valid(&self) -> bool {
        let ok = |k: &Vec<[f64; 2]>| k.len() == self.rest.len() && k.iter().all(|p| p.iter().all(|v| v.is_finite()));
        segments(self.rest.len(), self.closed).is_some()
            && ok(&self.rest)
            && self.keys.values().all(ok)
            && self.radius.is_none_or(|r| r.is_finite() && r >= 0.0)
            && self.fade.is_finite()
            && self.fade >= 0.0
    }

    /// The chain at `frame`: held before the first key and after the last, straight in between.
    pub fn at(&self, frame: u32) -> Vec<Point> {
        let before = self.keys.range(..=frame).next_back();
        let after = self.keys.range(frame..).next();
        match (before, after) {
            (Some((fa, a)), Some((fb, b))) if fb > fa => {
                let t = (frame - fa) as f64 / (fb - fa) as f64;
                a.iter().zip(b).map(|(p, q)| pt(p).lerp(pt(q), t)).collect()
            }
            (Some((_, k)), _) | (None, Some((_, k))) => k.iter().map(pt).collect(),
            (None, None) => self.rest.iter().map(pt).collect(),
        }
    }
}

type Seg = [Point; 4];

fn segs(p: &[Point], closed: bool) -> Vec<Seg> {
    let Some(n) = segments(p.len(), closed) else { return Vec::new() };
    (0..n).map(|i| [p[3 * i], p[3 * i + 1], p[3 * i + 2], p[(3 * i + 3) % p.len()]]).collect()
}

fn eval(s: &Seg, t: f64) -> Point {
    let u = 1.0 - t;
    let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
    Point::new(s.iter().zip(w).map(|(p, w)| p.x * w).sum(), s.iter().zip(w).map(|(p, w)| p.y * w).sum())
}

/// Unit tangent at `t`; a zero-length derivative (a handle on its anchor) falls back to the chord.
fn tangent(s: &Seg, t: f64) -> Vec2 {
    let u = 1.0 - t;
    let d = (s[1] - s[0]) * (3.0 * u * u) + (s[2] - s[1]) * (6.0 * u * t) + (s[3] - s[2]) * (3.0 * t * t);
    let d = if d.hypot() > EPS { d } else { s[3] - s[0] };
    if d.hypot() > EPS { d / d.hypot() } else { Vec2::new(1.0, 0.0) }
}

/// The parameter of `s`'s point nearest `p`: a coarse scan, then a few Newton steps.
fn nearest(s: &Seg, p: Point) -> f64 {
    const SCAN: usize = 32;
    let mut t = (0..=SCAN).map(|i| i as f64 / SCAN as f64).min_by(|a, b| (eval(s, *a) - p).hypot2().total_cmp(&(eval(s, *b) - p).hypot2())).unwrap_or(0.0);
    for _ in 0..4 {
        let h = 1e-4;
        let f = |t: f64| (eval(s, t) - p).hypot2();
        let (d1, d2) = ((f(t + h) - f(t - h)) / (2.0 * h), (f(t + h) - 2.0 * f(t) + f(t - h)) / (h * h));
        if d2 <= EPS {
            break;
        }
        t = (t - d1 / d2).clamp(0.0, 1.0);
    }
    t
}

/// A deformer's map from rest to now at one frame.
pub struct CurveMap {
    rest: Vec<Seg>,
    now: Vec<Seg>,
    closed: bool,
    radius: Option<f64>,
    fade: f64,
}

impl CurveMap {
    /// The map at `frame`, None when the chain rests there.
    pub fn of(c: &Curve, frame: u32) -> Option<Self> {
        let now = c.at(frame);
        if now.iter().zip(&c.rest).all(|(p, r)| (*p - pt(r)).hypot() <= EPS) {
            return None;
        }
        let rest: Vec<Point> = c.rest.iter().map(pt).collect();
        Some(CurveMap { rest: segs(&rest, c.closed), now: segs(&now, c.closed), closed: c.closed, radius: c.radius, fade: c.fade })
    }

    pub fn map(&self, p: Point) -> Point {
        let last = self.rest.len().saturating_sub(1);
        let (mut sum, mut wsum, mut dmin) = (Vec2::ZERO, 0.0, f64::INFINITY);
        for (i, (r, n)) in self.rest.iter().zip(&self.now).enumerate() {
            let t = nearest(r, p);
            let (o, tr) = (eval(r, t), tangent(r, t));
            let v = p - o;
            let (along, across) = (v.dot(tr), tr.cross(v));
            // An open chain's ends reach on past themselves along the tangent; elsewhere only the
            // across distance counts, so a point is carried by where it sits beside the segment.
            let end = !self.closed && ((i == 0 && t <= EPS) || (i == last && t >= 1.0 - EPS));
            let along = if end { along } else { 0.0 };
            let (o2, t2) = (eval(n, t), tangent(n, t));
            let q = o2 + t2 * along + Vec2::new(-t2.y, t2.x) * across;
            let d = v.hypot();
            dmin = dmin.min(d);
            let w = 1.0 / (d * d + 1e-6).powi(2);
            sum += q.to_vec2() * w;
            wsum += w;
        }
        if wsum <= 0.0 {
            return p;
        }
        let q = (sum / wsum).to_point();
        let k = match self.radius {
            Some(r) if dmin > r + self.fade => 0.0,
            Some(r) if dmin > r => 1.0 - (dmin - r) / self.fade.max(EPS),
            _ => 1.0,
        };
        p.lerp(q, k)
    }

    /// A mesh warp over `r` that follows this map, for art a point map can't bend (images).
    pub fn grid(&self, r: Rect) -> Option<super::mesh::Warp> {
        let m = super::mesh::Mesh::grid(0, r, GRID, GRID)?;
        let now = m.rest.iter().map(|p| self.map(pt(p))).collect();
        Some(super::mesh::Warp::from_points(&m, now))
    }
}

/// The deformers in `doc` at `frame` as warps, keyed by node; each knows its art's box for images.
pub(crate) fn warps(doc: &Document, frame: u32) -> BTreeMap<u64, super::mesh::Warp> {
    Curves::of(doc)
        .curves
        .iter()
        .filter_map(|c| {
            let map = CurveMap::of(c, frame)?;
            let n: &Node = doc.node(vectorcraft_doc::NodeId(c.node))?;
            let grid = n.visual_bounds().and_then(|r| map.grid(r.inflate(1.0, 1.0)));
            Some((c.node, super::mesh::Warp::curve(map, grid)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(x0: f64, x1: f64, n: usize) -> Vec<[f64; 2]> {
        let k = 3 * n;
        (0..=k).map(|i| [x0 + (x1 - x0) * i as f64 / k as f64, 0.0]).collect()
    }

    #[test]
    fn a_three_handle_curve_lands_the_strip_end_on_its_handle() {
        // Three anchors (two segments) along a 200 pt strip; the end anchor is keyed up and round.
        let rest = line(0.0, 200.0, 2);
        let mut now: Vec<[f64; 2]> = rest.clone();
        let bend = [[0.0, 0.0], [33.0, 0.0], [66.0, 0.0], [100.0, 0.0], [133.0, 0.0], [160.0, -30.0], [170.0, -80.0]];
        now.copy_from_slice(&bend);
        let c = Curve { node: 1, rest, keys: [(10, now)].into(), ..Default::default() };
        assert!(c.valid());
        let m = CurveMap::of(&c, 10).unwrap();
        let end = m.map(Point::new(200.0, 0.0));
        assert!((end - Point::new(170.0, -80.0)).hypot() < 0.5, "{end:?}");
        // The root stays put, and a point beside the strip keeps its distance from the curve.
        assert!((m.map(Point::new(0.0, 0.0)) - Point::new(0.0, 0.0)).hypot() < 0.5);
        let side = m.map(Point::new(50.0, 10.0));
        assert!((side - Point::new(50.0, 10.0)).hypot() < 1.0, "{side:?}");
    }

    #[test]
    fn an_envelope_carries_what_it_closes_around_and_fades_past_its_radius() {
        // A square envelope (4 straight segments) moved 50 pt right.
        let sq = [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)];
        let mut rest = Vec::new();
        for i in 0..4 {
            let (a, b) = (sq[i], sq[(i + 1) % 4]);
            let at = |t: f64| [a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t];
            rest.extend([at(0.0), at(1.0 / 3.0), at(2.0 / 3.0)]);
        }
        let now: Vec<[f64; 2]> = rest.iter().map(|p| [p[0] + 50.0, p[1]]).collect();
        let mut c = Curve { node: 1, rest, closed: true, keys: [(0, now)].into(), ..Default::default() };
        assert!(c.valid());
        let m = CurveMap::of(&c, 0).unwrap();
        for p in [Point::new(50.0, 50.0), Point::new(10.0, 90.0), Point::new(100.0, 0.0)] {
            assert!((m.map(p) - (p + Vec2::new(50.0, 0.0))).hypot() < 0.5, "{p:?}");
        }
        c.radius = Some(10.0);
        c.fade = 10.0;
        let m = CurveMap::of(&c, 0).unwrap();
        assert!((m.map(Point::new(50.0, 50.0)) - Point::new(50.0, 50.0)).hypot() < EPS, "past radius + fade: rest");
        assert!((m.map(Point::new(50.0, 5.0)) - Point::new(100.0, 5.0)).hypot() < 0.5);
    }

    #[test]
    fn broken_layouts_are_refused() {
        assert!(!Curve { rest: line(0.0, 1.0, 1)[..3].to_vec(), ..Default::default() }.valid());
        assert!(!Curve { rest: vec![[0.0, 0.0]; 3], closed: true, ..Default::default() }.valid());
        assert!(!Curve { rest: line(0.0, 1.0, 1), radius: Some(f64::NAN), ..Default::default() }.valid());
    }
}
