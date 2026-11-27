//! Ink Outlines: `shape.ink` turns the outlines of shapes into open ink strokes whose width follows
//! inking rules: heavier on the side away from the light, heavier on the outer silhouette, thinner
//! on tight curves, broken at sharp corners, tapered at the ends and wobbling slightly (seeded).

use std::sync::Arc;

use kurbo::{BezPath, ParamCurve, Point, Vec2};
use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::appearance::{AppearanceItem, LineCap, LineJoin, StrokeLayer, WidthProfile};
use vectorcraft_doc::{Appearance, Node, NodeId, NodeKind};
use vectorcraft_geom::hit::fill_contains;
use vectorcraft_geom::{FillRule, PathData, SubPath};

use super::{CommandSpec, Result, Session, bad, cmd, has_doc};
use crate::EngineError;

/// Most profile points one ink stroke keeps (its samples are thinned to this).
const MAX_PROFILE_POINTS: usize = 48;
/// Share of a piece's length over which its ends taper.
const TAPER: f64 = 0.15;
/// Width at a taper's very end, as a share of the full width.
const TAPER_MIN: f64 = 0.15;

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "shape.ink",
        "Ink Outlines",
        ["Object", "Path"],
        None,
        "{ids?|id? (default: the selection), weight?: base width pt (default 4, 0 < w ≤ 1000), light?: [x, y] direction the light comes FROM (default [-1, -1], up-left), break?: degrees (default 60: split at corners that turn more than this), wobble?: pt (default 0.5, ≥ 0), seed?: int (default 1), color?: \"#rrggbb\" (default black), keep?: bool (default true: keep the source shapes; false hides them)} draws the outlines of the shapes as open strokes with a width profile, in a new group named \"ink\" → {id: the group, paths: how many strokes}",
        has_doc,
        ink
    )]
}

/// One sampled point of an outline with its outward unit normal (after orientation).
struct Piece {
    points: Vec<Point>,
    normals: Vec<Vec2>,
}

/// A shape to ink: its path and fill rule.
struct Shape {
    path: BezPath,
    rule: FillRule,
}

fn ink(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "shape.ink";
    let num = |k: &str| p.get(k).and_then(Value::as_f64);
    let weight = num("weight").unwrap_or(4.0);
    if !(weight.is_finite() && weight > 0.0 && weight <= 1000.0) {
        return Err(bad(C, "weight must be a number above 0 and up to 1000 pt"));
    }
    let wobble = num("wobble").unwrap_or(0.5);
    if !(wobble.is_finite() && wobble >= 0.0) {
        return Err(bad(C, "wobble must be a number of 0 or more"));
    }
    let brk = num("break").unwrap_or(60.0);
    if !brk.is_finite() {
        return Err(bad(C, "break must be a number of degrees"));
    }
    let light = match p.get("light") {
        None => Vec2::new(-1.0, -1.0),
        Some(v) => {
            let a = v.as_array().filter(|a| a.len() == 2).ok_or_else(|| bad(C, "light must be [x, y]"))?;
            let (x, y) = (a[0].as_f64(), a[1].as_f64());
            match (x, y) {
                (Some(x), Some(y)) if x.is_finite() && y.is_finite() && x.hypot(y) > 1e-9 => Vec2::new(x, y),
                _ => return Err(bad(C, "light must be a non-zero [x, y]")),
            }
        }
    };
    let light = light / light.hypot();
    let seed = p.get("seed").and_then(Value::as_u64).unwrap_or(1);
    let color = match p.get("color") {
        None => Color::BLACK,
        Some(v) => v.as_str().and_then(Color::from_hex).ok_or_else(|| bad(C, "color must be \"#rrggbb\""))?,
    };
    let keep = p.get("keep").and_then(Value::as_bool).unwrap_or(true);
    let ids = super::targets(s, p)?;

    s.edit("Ink Outlines", |d, sel| {
        // Every filled or open path under the targets, groups expanded; guides and clipping paths are not inked.
        let mut shapes: Vec<Shape> = vec![];
        let mut sources: Vec<NodeId> = vec![];
        for id in &ids {
            let Some(root) = d.node(*id) else { continue };
            root.walk(&mut |c: &Node| {
                if let NodeKind::Path { path, rule, guide: false, clipping: false, .. } = &c.kind {
                    let mut bp = BezPath::new();
                    for sp in &path.subpaths {
                        sp.to_bezpath_into(&mut bp);
                    }
                    shapes.push(Shape { path: bp, rule: *rule });
                    sources.push(c.id);
                }
            });
        }
        if shapes.is_empty() {
            return Err(bad(C, "select paths to ink"));
        }

        // The subpaths to ink: (node index, subpath, closed).
        let mut inked: Vec<(usize, SubPath, bool)> = vec![];
        for (k, id) in sources.iter().enumerate() {
            if let Some(NodeKind::Path { path, .. }) = d.node(*id).map(|n| &n.kind) {
                for sp in &path.subpaths {
                    if sp.segment_count() > 0 {
                        inked.push((k, sp.clone(), sp.closed));
                    }
                }
            }
        }

        let mut strokes: Vec<Arc<Node>> = vec![];
        let mut counter: u64 = 0;
        for (k, sp, closed) in &inked {
            let own = &shapes[*k].path;
            let own_rule = shapes[*k].rule;
            let Some(sampled) = sample(sp, *closed, brk) else { continue };
            let (pts, breaks) = sampled;
            // Outward side: the normal side that lies outside the shape it belongs to.
            let orient = if *closed { outward_sign(&pts, own, own_rule) } else { 1.0 };
            let pieces = split(&pts, &breaks, *closed);
            for (edges, taper) in pieces {
                let piece = build_piece(&pts, &edges, orient);
                // Silhouette test: just outside the piece, against every other inked shape.
                let mut factors = Vec::with_capacity(piece.points.len());
                let m = piece.points.len();
                for i in 0..m {
                    let (pt, n) = (piece.points[i], piece.normals[i]);
                    let shade = 0.6 + 0.6 * (-n.dot(light)).max(0.0);
                    let probe = pt + n * (0.5 * weight.max(1.0));
                    let outer = !shapes.iter().enumerate().any(|(j, o)| j != *k && fill_contains(&o.path, o.rule, probe));
                    // shortcut: a point just outside is tested against each other shape alone, not against the exact union outline.
                    let sil = if outer { 1.25 } else { 0.8 };
                    let thin = (curve_radius(&piece.points, i) / (8.0 * weight)).clamp(0.5, 1.0);
                    let t = if m > 1 { i as f64 / (m - 1) as f64 } else { 0.0 };
                    let tap = if taper {
                        let edge = t.min(1.0 - t);
                        TAPER_MIN + (1.0 - TAPER_MIN) * (edge / TAPER).min(1.0)
                    } else {
                        1.0
                    };
                    factors.push(shade * sil * thin * tap);
                }
                // Wobble: a seeded normal offset per point (splitmix64, no rand dependency).
                let mut out: Vec<Point> = Vec::with_capacity(m);
                for i in 0..m {
                    counter = counter.wrapping_add(1);
                    let r = unit(seed, counter);
                    out.push(piece.points[i] + piece.normals[i] * (wobble * r));
                }
                let profile = profile_points(&factors);
                let sub = SubPath::polyline(&out, false);
                let mut layer = StrokeLayer::new(Paint::solid(color), weight);
                layer.cap = LineCap::Round;
                layer.join = LineJoin::Round;
                layer.profile = Some(WidthProfile { points: profile });
                let look = Appearance { items: vec![AppearanceItem::Stroke(layer)], ..Appearance::default() };
                let id = d.alloc_id();
                strokes.push(Arc::new(Node::path(id, PathData::single(sub), look)));
            }
        }
        if strokes.is_empty() {
            return Err(bad(C, "nothing to ink: the shapes are too small"));
        }
        let count = strokes.len();
        let parent = sources.first().and_then(|id| d.parent_of(*id));
        let gid = d.alloc_id();
        let mut group = Node::group(gid, strokes);
        group.name = Some("ink".into());
        d.insert(parent, usize::MAX, group).map_err(|e| EngineError::Other(format!("{C}: {e:?}")))?;
        if !keep {
            for id in &sources {
                if let Some(n) = d.node_mut(*id) {
                    n.visible = false;
                }
            }
        }
        sel.set([gid]);
        Ok(json!({"id": gid.0, "paths": count}))
    })
}

/// Samples of one subpath: points, and the sample indices where a corner turns more than `brk`
/// degrees (index 0 for a corner at the seam of a closed path). `None` when it has no segments.
fn sample(sp: &SubPath, closed: bool, brk: f64) -> Option<(Vec<Point>, Vec<usize>)> {
    let n = sp.segment_count();
    if n == 0 {
        return None;
    }
    let mut pts = vec![];
    let mut starts = vec![];
    let mut tangents = vec![];
    for i in 0..n {
        let seg = sp.segment(i);
        starts.push(pts.len());
        let k = if sp.segment_is_line(i) { 4 } else { 16 };
        for j in 0..k {
            pts.push(seg.eval(j as f64 / k as f64));
        }
        let a = seg.eval(1e-3) - seg.eval(0.0);
        let b = seg.eval(1.0) - seg.eval(1.0 - 1e-3);
        tangents.push((a, b));
    }
    // The closing point: a closed path's seam (equal to its first point), or an open path's end.
    pts.push(sp.segment(n - 1).eval(1.0));
    let mut breaks = vec![];
    for i in 0..n {
        // A closed path's seam is between its last and first segment.
        if i == 0 && !closed {
            continue;
        }
        let prev = if i == 0 { n - 1 } else { i - 1 };
        if turn_deg(tangents[prev].1, tangents[i].0) > brk {
            breaks.push(starts[i]);
        }
    }
    Some((pts, breaks))
}

/// Angle in degrees between two directions; 0 when either is (near) zero.
fn turn_deg(a: Vec2, b: Vec2) -> f64 {
    let (la, lb) = (a.hypot(), b.hypot());
    if la < 1e-12 || lb < 1e-12 {
        return 0.0;
    }
    (a.dot(b) / (la * lb)).clamp(-1.0, 1.0).acos().to_degrees()
}

/// The sign that makes `normal = sign * (ty, -tx)` point outside a closed shape.
fn outward_sign(pts: &[Point], own: &BezPath, rule: FillRule) -> f64 {
    let m = pts.len();
    if m < 3 {
        return 1.0;
    }
    let i = m / 2;
    let t = pts[(i + 1) % m] - pts[(i + m - 1) % m];
    let len = t.hypot();
    if len < 1e-12 {
        return 1.0;
    }
    let n = Vec2::new(t.y, -t.x) / len;
    let probe = pts[i] + n * 0.5;
    if fill_contains(own, rule, probe) { -1.0 } else { 1.0 }
}

/// Splits the samples into pieces at the breaks: (edge indices into the piece's own point list, taper).
/// A closed path without breaks is one piece that wraps, so it does not taper.
fn split(pts: &[Point], breaks: &[usize], closed: bool) -> Vec<(Vec<usize>, bool)> {
    let m = pts.len();
    let taper = !(closed && breaks.is_empty());
    let mut edges: Vec<usize> = breaks.iter().copied().filter(|b| *b < m).collect();
    edges.push(0);
    edges.push(m.saturating_sub(1));
    edges.sort_unstable();
    edges.dedup();
    edges.windows(2).filter(|w| w[1] > w[0]).map(|w| (vec![w[0], w[1]], taper)).collect()
}

/// Points and outward normals of one piece, given its edge indices `[a, b]` into `pts`.
fn build_piece(pts: &[Point], edges: &[usize], orient: f64) -> Piece {
    let (a, b) = (edges[0], edges[1]);
    let points: Vec<Point> = pts.get(a..=b).map(|s| s.to_vec()).unwrap_or_default();
    let m = points.len();
    let normals = (0..m)
        .map(|i| {
            let lo = i.saturating_sub(1);
            let hi = (i + 1).min(m.saturating_sub(1));
            let t = points[hi] - points[lo];
            let len = t.hypot();
            if len < 1e-12 { Vec2::new(0.0, 0.0) } else { Vec2::new(t.y, -t.x) / len * orient }
        })
        .collect();
    Piece { points, normals }
}

/// Curvature radius at sample `i` of a piece (infinite on straight runs and at the ends).
fn curve_radius(pts: &[Point], i: usize) -> f64 {
    if i == 0 || i + 1 >= pts.len() {
        return f64::INFINITY;
    }
    let (din, dout) = (pts[i] - pts[i - 1], pts[i + 1] - pts[i]);
    let avg = (din.hypot() + dout.hypot()) / 2.0;
    let k = turn_deg(din, dout).to_radians() / avg.max(1e-12);
    if k < 1e-9 { f64::INFINITY } else { 1.0 / k }
}

/// Profile points (t, width, width) from per-sample factors, thinned to at most [`MAX_PROFILE_POINTS`].
fn profile_points(factors: &[f64]) -> Vec<(f64, f64, f64)> {
    let n = factors.len();
    if n == 0 {
        return vec![(0.0, 1.0, 1.0), (1.0, 1.0, 1.0)];
    }
    let m = n.min(MAX_PROFILE_POINTS);
    if m < 2 {
        return vec![(0.0, factors[0], factors[0]), (1.0, factors[0], factors[0])];
    }
    (0..m)
        .map(|k| {
            let idx = k * (n - 1) / (m - 1);
            let f = factors[idx];
            (idx as f64 / (n - 1) as f64, f, f)
        })
        .collect()
}

/// A deterministic value in [-1, 1] from the seed and a counter (splitmix64).
fn unit(seed: u64, counter: u64) -> f64 {
    let mut z = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(counter.wrapping_mul(0xBF58_476D_1CE4_E5B9));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();
        s
    }

    /// The children of the ink group `gid`.
    fn strokes(s: &Session, gid: u64) -> Vec<Arc<Node>> {
        match &s.doc().unwrap().doc.node(NodeId(gid)).unwrap().kind {
            NodeKind::Group { children, .. } => children.clone(),
            _ => panic!("not a group"),
        }
    }

    fn stroke_layer(n: &Node) -> &StrokeLayer {
        n.appearance.items.iter().find_map(|i| if let AppearanceItem::Stroke(l) = i { Some(l) } else { None }).unwrap()
    }

    #[test]
    fn light_from_left_makes_the_right_side_heavier() {
        let mut s = session();
        let id = s.execute("shape.ellipse", &json!({"x": 50, "y": 50, "width": 100, "height": 100})).unwrap()["id"].as_u64().unwrap();
        let gid = s.execute("shape.ink", &json!({"ids": [id], "wobble": 0, "light": [-1, 0]})).unwrap()["id"].as_u64().unwrap();
        let kids = strokes(&s, gid);
        assert_eq!(kids.len(), 1, "a smooth circle is one piece");
        let pd = kids[0].path_data().unwrap().clone();
        let sp = &pd.subpaths[0];
        let n = sp.anchors.len();
        let prof = stroke_layer(&kids[0]).profile.clone().unwrap();
        let (mut max_i, mut min_i) = (0, 0);
        for (i, a) in sp.anchors.iter().enumerate() {
            if a.p.x > sp.anchors[max_i].p.x {
                max_i = i;
            }
            if a.p.x < sp.anchors[min_i].p.x {
                min_i = i;
            }
        }
        let frac = |i: usize| i as f64 / (n - 1) as f64;
        let right = prof.at(frac(max_i)).0;
        let left = prof.at(frac(min_i)).0;
        assert!(right > left, "right {right} should be heavier than left {left}");
    }

    #[test]
    fn a_square_breaks_into_four_strokes() {
        let mut s = session();
        let id = s.execute("shape.rectangle", &json!({"x": 20, "y": 20, "width": 60, "height": 60})).unwrap()["id"].as_u64().unwrap();
        let gid = s.execute("shape.ink", &json!({"ids": [id]})).unwrap()["id"].as_u64().unwrap();
        assert_eq!(strokes(&s, gid).len(), 4);
    }

    #[test]
    fn strokes_have_a_profile_and_no_fill() {
        let mut s = session();
        let id = s.execute("shape.rectangle", &json!({"x": 20, "y": 20, "width": 60, "height": 60})).unwrap()["id"].as_u64().unwrap();
        let gid = s.execute("shape.ink", &json!({"ids": [id]})).unwrap()["id"].as_u64().unwrap();
        for k in strokes(&s, gid) {
            assert!(stroke_layer(&k).profile.is_some());
            assert!(!k.appearance.items.iter().any(|i| matches!(i, AppearanceItem::Fill(_))));
        }
    }

    #[test]
    fn keep_false_hides_the_sources_and_undo_removes_the_ink() {
        let mut s = session();
        let id = s.execute("shape.rectangle", &json!({"x": 20, "y": 20, "width": 60, "height": 60})).unwrap()["id"].as_u64().unwrap();
        let gid = s.execute("shape.ink", &json!({"ids": [id], "keep": false})).unwrap()["id"].as_u64().unwrap();
        assert!(!s.doc().unwrap().doc.node(NodeId(id)).unwrap().visible);
        s.execute("edit.undo", &json!({})).unwrap();
        assert!(s.doc().unwrap().doc.node(NodeId(gid)).is_none());
        assert!(s.doc().unwrap().doc.node(NodeId(id)).unwrap().visible);
    }

    #[test]
    fn bad_weight_is_an_error() {
        let mut s = session();
        let id = s.execute("shape.rectangle", &json!({"x": 20, "y": 20, "width": 60, "height": 60})).unwrap()["id"].as_u64().unwrap();
        assert!(s.execute("shape.ink", &json!({"ids": [id], "weight": -2})).is_err());
        assert!(s.execute("shape.ink", &json!({"ids": [id], "weight": 0})).is_err());
    }
}
