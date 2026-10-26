//! 7rrek (Creative Studio): frame-by-frame animation on top of layers.
//!
//! The timeline's rows are the top-level layers. A keyframe is a sublayer of its row: it holds
//! the art from its frame until the row's next keyframe. Going to a frame shows, in every row, the
//! keyframe in effect there and hides the row's other keyframes; art in a row outside its
//! keyframes shows on every frame. Nothing new is added to the file format: the timing lives in
//! the document's preserved extra data under `"animation"` (`fps`, `frames`, `current` and `keys`,
//! a map from keyframe sublayer id to its frame), so rsm opens a 7rrek file as plain layers.
//!
//! Frames count from 1. Changing frames is a view change, not an edit: it isn't undoable and
//! doesn't add an undo step; the timeline's edits (keyframes, frame rate, length) are.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vectorcraft_doc::blend;
use vectorcraft_doc::{Document, Node, NodeId};
use vectorcraft_geom::{Affine, Point, Rect};

use super::*;
use crate::EngineError;

const KEY: &str = "animation";
/// Longest timeline (frames) and fastest frame rate a command accepts.
const MAX_FRAMES: u32 = 10_000;
const MAX_FPS: u32 = 60;
/// Most keyframes the onion skin shows on each side.
const MAX_ONION: u32 = 5;
/// Largest export, in pixels per frame (width × height) and frames.
const MAX_EXPORT_PIXELS: u64 = 4096 * 4096;

/// What the camera sees at a keyframe: the view's centre (document points), zoom (1: the artboard's
/// size, 2: twice as close) and rotation (degrees, counter-clockwise), easing toward the next key.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Cam {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
    pub rotation: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ease: Option<[f64; 4]>,
}

impl Cam {
    pub fn valid(&self) -> bool {
        [self.x, self.y, self.zoom, self.rotation].iter().all(|v| v.is_finite())
            && (0.01..=100.0).contains(&self.zoom)
            && self.ease.as_ref().is_none_or(valid_ease)
    }
}

/// A sound clip: the file it came from, the frame it starts on and its gain (1: as recorded).
/// `data` names the file's bytes when they were embedded on import: the id of the base64 copy in
/// the document's `animationSounds` store, so the sound travels with the document. `path` is the
/// fallback when it is missing.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Sound {
    pub name: String,
    pub path: String,
    pub start: u32,
    #[serde(default = "unity")]
    pub gain: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

fn unity() -> f32 {
    1.0
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Anim {
    pub fps: u32,
    pub frames: u32,
    pub current: u32,
    /// Keyframe sublayer id → its frame.
    pub keys: BTreeMap<u64, u32>,
    /// Onion skin: how many keyframes before and after the current one show faintly (0: off).
    pub onion_before: u32,
    pub onion_after: u32,
    /// Keyframes with a motion tween: their art moves and scales toward the row's next keyframe.
    pub tweens: BTreeSet<u64>,
    /// A tween's easing as CSS `cubic-bezier(x1, y1, x2, y2)` control points (none: linear).
    pub eases: BTreeMap<u64, [f64; 4]>,
    /// Camera keyframes, frame → view. None: exports show the artboard as it is.
    pub camera: BTreeMap<u32, Cam>,
    /// Sound clips on the timeline.
    pub sounds: Vec<Sound>,
    /// The locked in-between layers the frame shown added (removed on the next frame change).
    pub ghosts: Vec<u64>,
    /// The art the frame shown hid behind its bent or pegged copies (shown again on the next frame
    /// change).
    #[serde(default)]
    pub posed: Vec<u64>,
    /// Drawing substitution: row id → frame → the row's drawing (one of its keyframe sublayers)
    /// shown from that frame on, as a keyframe would be.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub exposures: BTreeMap<u64, BTreeMap<u32, u64>>,
    /// Lip sync mouth chart: row id → mouth shape (`A`–`H`, `X`) → the row's drawing for it. A
    /// shape left out falls back to the row's drawing named after it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub mouths: BTreeMap<u64, BTreeMap<char, u64>>,
    /// The rows the frame shown cut with an opacity mask, and the matte rows it hid (both taken
    /// back on the next frame change; see cutter.rs).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cut: Vec<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mattes: Vec<u64>,
}

impl Default for Anim {
    fn default() -> Self {
        Self {
            fps: 12,
            frames: 24,
            current: 1,
            keys: BTreeMap::new(),
            onion_before: 0,
            onion_after: 0,
            tweens: BTreeSet::new(),
            eases: BTreeMap::new(),
            camera: BTreeMap::new(),
            sounds: Vec::new(),
            ghosts: Vec::new(),
            posed: Vec::new(),
            exposures: BTreeMap::new(),
            mouths: BTreeMap::new(),
            cut: Vec::new(),
            mattes: Vec::new(),
        }
    }
}

impl Anim {
    pub fn of(doc: &Document) -> Self {
        doc.unknown.get(KEY).and_then(|v| serde_json::from_value(v.clone()).ok()).map(Self::sane).unwrap_or_default()
    }

    fn sane(mut self) -> Self {
        self.fps = self.fps.clamp(1, MAX_FPS);
        self.frames = self.frames.clamp(1, MAX_FRAMES);
        self.current = self.current.clamp(1, self.frames);
        self.onion_before = self.onion_before.min(MAX_ONION);
        self.onion_after = self.onion_after.min(MAX_ONION);
        self.eases.retain(|_, e| valid_ease(e));
        self.camera.retain(|f, c| (1..=MAX_FRAMES).contains(f) && c.valid());
        for e in self.exposures.values_mut() {
            e.retain(|f, _| (1..=MAX_FRAMES).contains(f));
        }
        self.sounds.retain(|c| c.gain.is_finite() && (0.0..=4.0).contains(&c.gain) && (1..=MAX_FRAMES).contains(&c.start));
        self
    }

    pub(crate) fn store(&self, doc: &mut Document) {
        if let Ok(v) = serde_json::to_value(self) {
            doc.unknown.insert(KEY.into(), v);
        }
    }

    /// A row's keyframes, by frame: `(frame, sublayer)`, earliest first.
    pub fn row_keys(&self, row: &Node) -> Vec<(u32, NodeId)> {
        let mut out: Vec<(u32, NodeId)> =
            row.children().into_iter().flatten().filter(|c| c.is_layer()).filter_map(|c| self.keys.get(&c.id.0).map(|f| (*f, c.id))).collect();
        out.sort();
        out
    }

    /// What `row` shows from each frame on: its keyframes and its exposures of drawings it still
    /// holds, earliest first (an exposure wins over a keyframe at the same frame).
    pub fn row_frames(&self, row: &Node) -> Vec<(u32, NodeId)> {
        let mut out: BTreeMap<u32, NodeId> = self.row_keys(row).into_iter().collect();
        let held = |id: u64| row.children().into_iter().flatten().any(|c| c.is_layer() && c.id.0 == id);
        for (f, id) in self.exposures.get(&row.id.0).into_iter().flatten() {
            if held(*id) {
                out.insert(*f, NodeId(*id));
            }
        }
        out.into_iter().collect()
    }

    /// The keyframe of `row` in effect at `frame`, if the row has one there.
    pub fn key_at(&self, row: &Node, frame: u32) -> Option<NodeId> {
        self.row_keys(row).into_iter().rfind(|(f, _)| *f <= frame).map(|(_, id)| id)
    }
}

/// `doc` without what the frame shown added: its in-between and posed copies gone, the art they
/// stood in for shown again (the ghost and posed lists are left for the caller).
pub(crate) fn unposed(doc: &Document) -> Document {
    let anim = Anim::of(doc);
    let mut d = doc.clone();
    for g in &anim.ghosts {
        // Already gone (deleted by hand) is fine.
        let _ = d.remove(NodeId(*g));
    }
    for id in anim.posed.iter().chain(&anim.mattes) {
        if let Some(n) = d.node_mut(NodeId(*id)) {
            n.visible = true;
        }
    }
    for id in &anim.cut {
        if let Some(n) = d.node_mut(NodeId(*id)) {
            n.extras_mut().mask = None;
        }
    }
    d
}

/// `doc` showing `frame`: each animated row shows its keyframe in effect, the others hidden.
/// With `onion`, the document's onion-skin keyframes around it show faintly and locked.
pub fn at_frame_with(doc: &Document, frame: u32, onion: bool) -> Document {
    let anim = Anim::of(doc);
    let mut d = unposed(doc);
    super::palette::apply(&mut d);
    let mut ghosts = Vec::new();
    for row in &doc.layers {
        let keys = anim.row_frames(row);
        let Some(shown) = keys.iter().rposition(|(f, _)| *f <= frame).or((!keys.is_empty()).then_some(usize::MAX)) else { continue };
        for (_, id) in &keys {
            if let Some(n) = d.node_mut(*id) {
                (n.visible, n.locked, n.opacity) = (false, false, 1.0);
            }
        }
        // A drawing exposed on several frames appears more than once: the one shown wins, then the
        // nearest faint one.
        let mut order: Vec<(usize, i64)> =
            (0..keys.len()).map(|i| (i, if shown == usize::MAX { i64::MAX } else { i as i64 - shown as i64 })).collect();
        order.sort_by_key(|(_, k)| std::cmp::Reverse(k.unsigned_abs()));
        for (i, k) in order {
            let Some(n) = keys.get(i).and_then(|(_, id)| d.node_mut(*id)) else { continue };
            // Distance in keyframes from the one shown (before the first keyframe, nothing is).
            let faint = onion && k != i64::MAX && ((k < 0 && -k <= anim.onion_before as i64) || (k > 0 && k <= anim.onion_after as i64));
            if k == 0 {
                (n.visible, n.locked, n.opacity) = (true, false, 1.0);
            } else if faint {
                (n.visible, n.locked, n.opacity) = (true, true, 0.35 / k.unsigned_abs() as f32);
            }
        }
        if let Some(&(fa, a)) = keys.get(shown)
            && anim.keys.get(&a.0) == Some(&fa)
            && anim.tweens.contains(&a.0)
            && let Some(&(fb, b)) = keys.get(shown + 1)
            && frame > fa
            && let Some(g) = tween(&mut d, a, b, eased(anim.eases.get(&a.0), (frame - fa) as f64 / (fb - fa) as f64))
        {
            ghosts.push(g.0);
        }
    }
    let posed = pose(&mut d, frame, &mut ghosts);
    let (cut, mattes) = super::cutter::apply(&mut d);
    let mut a = anim;
    a.ghosts = ghosts;
    a.posed = posed;
    (a.cut, a.mattes) = (cut, mattes);
    a.current = frame.clamp(1, a.frames);
    a.store(&mut d);
    d
}

/// Named easings → CSS cubic-bezier control points (the CSS Easing Functions spec's values).
const EASES: [(&str, [f64; 4]); 3] = [("easeIn", [0.42, 0.0, 1.0, 1.0]), ("easeOut", [0.0, 0.0, 0.58, 1.0]), ("easeInOut", [0.42, 0.0, 0.58, 1.0])];

pub(crate) fn valid_ease(e: &[f64; 4]) -> bool {
    e.iter().all(|v| v.is_finite()) && (0.0..=1.0).contains(&e[0]) && (0.0..=1.0).contains(&e[2]) && e[1].abs() <= 10.0 && e[3].abs() <= 10.0
}

/// Progress `x` (0–1) through a tween with easing `e` → how far the art has gone (may overshoot).
pub(crate) fn eased(e: Option<&[f64; 4]>, x: f64) -> f64 {
    let Some(&[x1, y1, x2, y2]) = e else { return x };
    let bez = |a: f64, b: f64, u: f64| 3.0 * a * u * (1.0 - u) * (1.0 - u) + 3.0 * b * u * u * (1.0 - u) + u * u * u;
    // x(u) rises monotonically for x1, x2 in 0–1: bisect for the u where it reaches x.
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..50 {
        let mid = (lo + hi) / 2.0;
        if bez(x1, x2, mid) < x { lo = mid } else { hi = mid }
    }
    bez(y1, y2, (lo + hi) / 2.0)
}

impl Anim {
    /// The camera at `frame`: held before the first key and after the last, eased in between.
    pub fn camera_at(&self, frame: u32) -> Option<Cam> {
        let before = self.camera.range(..=frame).next_back();
        let after = self.camera.range(frame..).next();
        match (before, after) {
            (Some((fa, a)), Some((fb, b))) if fb > fa => {
                let t = eased(a.ease.as_ref(), (frame - fa) as f64 / (fb - fa) as f64);
                let mix = |x: f64, y: f64| x + (y - x) * t;
                let turn = (b.rotation - a.rotation + 180.0).rem_euclid(360.0) - 180.0;
                Some(Cam {
                    x: mix(a.x, b.x),
                    y: mix(a.y, b.y),
                    zoom: a.zoom * (b.zoom / a.zoom).powf(t),
                    rotation: a.rotation + turn * t,
                    ease: None,
                })
            }
            (Some((_, c)), _) | (None, Some((_, c))) => Some(*c),
            (None, None) => None,
        }
    }
}

/// `doc` as the camera sees it through the artboard `rect`: the camera's centre lands on the
/// artboard's, zoomed and turned.
pub fn through_camera(mut doc: Document, cam: Option<Cam>, rect: Rect) -> Document {
    let Some(c) = cam else { return doc };
    let m = Affine::translate(rect.center().to_vec2())
        * Affine::rotate(-c.rotation.to_radians())
        * Affine::scale(c.zoom)
        * Affine::translate((-c.x, -c.y));
    for l in &mut doc.layers {
        Arc::make_mut(l).transform(m, true);
    }
    doc
}

/// Keyframe `a`'s art `t` (0–1) of the way to keyframe `b`'s, as a locked layer shown in `a`'s
/// place → its id. Objects pair up by name, else by order. A pair whose paths have the same
/// anchors up to a transform turns, scales and moves along that transform; any other pair morphs
/// (the Blend tool's interpolation). Fill, stroke and opacity mix either way.
fn tween(d: &mut Document, a: NodeId, b: NodeId, t: f64) -> Option<NodeId> {
    let from = d.node(a)?.clone();
    let to: Vec<Arc<Node>> = d.node(b)?.children().cloned().unwrap_or_default();
    let mut g = d.reid(&from);
    g.name = Some("Tween".into());
    (g.visible, g.locked, g.opacity) = (true, true, 1.0);
    for (i, c) in g.children_mut().into_iter().flatten().enumerate() {
        let named = c.name.as_ref().and_then(|n| to.iter().find(|e| e.name.as_ref() == Some(n)));
        let Some(end) = named.or(to.get(i)) else { continue };
        let c = Arc::make_mut(c);
        if let Some(m) = fit(c, end)
            && let Some(r) = c.geometric_bounds()
        {
            // Turn and scale about the centre, which moves in a straight line.
            let (p, [a, b, cc, dd, _, _]) = (r.center(), m.as_coeffs());
            let q = p + (m * p - p) * t;
            let turn = blend::lerp_affine(Affine::IDENTITY, Affine::new([a, b, cc, dd, 0.0, 0.0]), t);
            c.transform(Affine::translate(q.to_vec2()) * turn * Affine::translate(-p.to_vec2()), true);
            c.appearance = blend::lerp_appearance(&c.appearance, &end.appearance, t).into();
            c.opacity = c.opacity + (end.opacity - c.opacity) * t as f32;
        } else {
            let id = c.id;
            *c = blend::lerp_node(c, end, t);
            c.id = id;
        }
    }
    let n = d.node_mut(a)?;
    (n.visible, n.locked, n.opacity) = (false, false, 1.0);
    let parent = d.parent_of(a);
    let at = d.index_path(a)?.last()? + 1;
    d.insert(parent, at, g).ok()
}

/// Rigged rows bend their art to the pose at `frame`, and pegs then carry their rows. The art
/// shown is never changed: each visible child of such a row hides behind a locked copy that is
/// bent and moved (a tween's in-between is bent in place, being a copy already) → the hidden ids.
fn pose(d: &mut Document, frame: u32, ghosts: &mut Vec<u64>) -> Vec<u64> {
    let (rig, pegs) = (super::rig::Rig::of(d), super::peg::Pegs::of(d));
    let warps: BTreeMap<u64, super::mesh::Warp> =
        super::mesh::Meshes::of(d).meshes.iter().filter_map(|m| Some((m.node, super::mesh::Warp::of(m, frame)?))).collect();
    let mut hidden = Vec::new();
    let rows: Vec<NodeId> = d.layers.iter().map(|l| l.id).collect();
    for row in rows {
        let bones = super::rig::pose_matrices(&rig, row.0, frame);
        let peg = pegs.row_matrix(row.0, frame);
        let meshed = !warps.is_empty() && d.node(row).is_some_and(|r| super::mesh::warped(r, &warps));
        if bones.is_none() && peg.is_none() && !meshed {
            continue;
        }
        let shown: Vec<NodeId> = d.node(row).and_then(|r| r.children()).into_iter().flatten().filter(|c| c.visible).map(|c| c.id).collect();
        for id in shown {
            let target = if ghosts.contains(&id.0) {
                id
            } else {
                let Some(mut n) = d.node(id).cloned() else { continue };
                // The mesh bends the art first (while it still has the ids its mesh names), then
                // bones and pegs move it.
                super::mesh::deform(&mut n, &warps, None, &mut d.images);
                let mut copy = d.reid(&n);
                copy.locked = true;
                let Some(at) = d.index_path(id).and_then(|p| p.last().copied()) else { continue };
                let Ok(copy) = d.insert(Some(row), at + 1, copy) else { continue };
                if let Some(n) = d.node_mut(id) {
                    n.visible = false;
                }
                hidden.push(id.0);
                ghosts.push(copy.0);
                copy
            };
            if let Some((sk, m)) = &bones {
                super::rig::bend(d, target, sk, m);
            }
            if let (Some(m), Some(n)) = (peg, d.node_mut(target)) {
                n.transform(m, true);
            }
        }
    }
    hidden
}

/// The affine taking path `a`'s points (anchors and handles) onto path `b`'s, when both have the
/// same anchor counts and the fit is close (least squares, within half a point per point).
fn fit(a: &Node, b: &Node) -> Option<Affine> {
    let pts = |n: &Node| -> Option<(Vec<usize>, Vec<Point>)> {
        let p = n.path_data()?;
        let counts = p.subpaths.iter().map(|s| s.anchors.len()).collect();
        Some((counts, p.subpaths.iter().flat_map(|s| s.anchors.iter().flat_map(|k| [k.p, k.h_in, k.h_out])).collect()))
    };
    let ((ca, pa), (cb, pb)) = (pts(a)?, pts(b)?);
    if ca != cb || pa.is_empty() {
        return None;
    }
    // Normal equations: [Σxx Σxy Σx; Σxy Σyy Σy; Σx Σy n] · [m; k; e] = [Σxu; Σyu; Σu], once for u = x' and once for u = y'.
    let mut s = [[0.0f64; 3]; 3];
    let (mut rx, mut ry) = ([0.0f64; 3], [0.0f64; 3]);
    for (p, q) in pa.iter().zip(&pb) {
        let v = [p.x, p.y, 1.0];
        for (i, vi) in v.iter().enumerate() {
            for (j, vj) in v.iter().enumerate() {
                s[i][j] += vi * vj;
            }
            rx[i] += vi * q.x;
            ry[i] += vi * q.y;
        }
    }
    let det3 = |m: &[[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let det = det3(&s);
    // Too few distinct points (a line or a dot) to pin a transform down.
    if det.abs() < 1e-9 {
        return None;
    }
    let solve = |r: [f64; 3]| -> [f64; 3] {
        let col = |k: usize| {
            let mut m = s;
            for (row, v) in m.iter_mut().zip(r) {
                row[k] = v;
            }
            det3(&m) / det
        };
        [col(0), col(1), col(2)]
    };
    let ([xa, xc, xe], [yb, yd, yf]) = (solve(rx), solve(ry));
    let m = Affine::new([xa, yb, xc, yd, xe, yf]);
    pa.iter().zip(&pb).all(|(p, q)| (m * *p - *q).hypot() < 0.5).then_some(m)
}

/// `doc` showing `frame` as it renders: no onion skin (exports, frames of a movie).
pub fn at_frame(doc: &Document, frame: u32) -> Document {
    at_frame_with(doc, frame, false)
}

/// The top-level layer (timeline row) that holds `id`.
pub fn row_of(doc: &Document, id: NodeId) -> Option<NodeId> {
    let mut cur = id;
    while let Some(p) = doc.parent_of(cur) {
        cur = p;
    }
    doc.layers.iter().any(|l| l.id == cur).then_some(cur)
}

pub(crate) fn row_param(s: &Session, p: &Value, cmd: &str) -> Result<NodeId> {
    let st = s.doc()?;
    let start = id_param(p, "row").or(st.current_layer()).ok_or_else(|| bad(cmd, "no layer"))?;
    row_of(&st.doc, start).ok_or_else(|| bad(cmd, "not a layer in this document"))
}

pub(crate) fn frame_param(p: &Value, anim: &Anim, cmd: &str) -> Result<u32> {
    match p.get("frame") {
        None | Some(Value::Null) => Ok(anim.current),
        Some(v) => v
            .as_u64()
            .filter(|f| (1..=anim.frames as u64).contains(f))
            .map(|f| f as u32)
            .ok_or_else(|| bad(cmd, format!("frame must be a whole number from 1 to {}", anim.frames))),
    }
}

/// Show `frame` without an undo step, and draw into the current row's keyframe there.
pub(crate) fn show(s: &mut Session, frame: u32) -> Result<Value> {
    let st = s.doc_mut()?;
    let row = st.current_layer().and_then(|l| row_of(&st.doc, l));
    let d = at_frame_with(&st.doc, frame, true);
    let anim = Anim::of(&d);
    if let Some(key) = row.and_then(|r| d.node(r)).and_then(|r| anim.key_at(r, frame)) {
        st.active_layer = Some(key);
    }
    // Art now hidden behind its posed copy can't stay selected.
    st.selection.objects.retain(|id| !anim.posed.contains(&id.0));
    st.doc = Arc::new(d);
    st.revision += 1;
    Ok(json!({ "frame": anim.current }))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "anim.info", "Timeline", [], None, "{} → {fps, frames, current, rows: [{id, name, keys: [{frame, id}]}]}", has_doc, info),
        cmd!(query "anim.goto", "Go to Frame", [], None, "{frame} show a frame (1-based). View state: not an undo step → {frame}", has_doc, goto),
        cmd!(query "anim.next", "Next Frame", [], None, "{} the next frame, wrapping to the first. View state: not an undo step → {frame}", has_doc, next),
        cmd!(query "anim.prev", "Previous Frame", [], None, "{} the previous frame, wrapping to the last. View state: not an undo step → {frame}", has_doc, prev),
        cmd!(
            "anim.keyframe",
            "Insert Keyframe",
            [],
            None,
            "{row?: layer id (default: the current layer's row), frame?: (default: the current frame), blank?: bool} a keyframe in the row at the frame: a copy of the art in effect there, or empty with `blank` → {id, frame}",
            has_doc,
            keyframe
        ),
        cmd!(
            "anim.blankKeyframe",
            "Insert Blank Keyframe",
            [],
            None,
            "{row?, frame?} as anim.keyframe with blank: true → {id, frame}",
            has_doc,
            blank_keyframe
        ),
        cmd!(
            "anim.removeKeyframe",
            "Clear Keyframe",
            [],
            None,
            "{row?, frame?} delete the row's keyframe at the frame and its art → {deleted}",
            has_doc,
            remove_keyframe
        ),
        cmd!(
            query "anim.onion",
            "Onion Skin",
            [],
            None,
            "{before?: 0–5, after?: 0–5} how many keyframes before and after the current one show faintly (0 turns a side off). View state: not an undo step → {before, after}",
            has_doc,
            onion
        ),
        cmd!(
            "anim.tween",
            "Motion Tween",
            [],
            None,
            "{row?, frame?, on?: bool (default: toggle; on when ease is given), ease?: \"linear\" | \"easeIn\" | \"easeOut\" | \"easeInOut\" | [x1, y1, x2, y2] (CSS cubic-bezier)} the keyframe in effect at the frame tweens its art toward the row's next keyframe (objects pair by name, else order; same-shape pairs move, turn and scale, others morph; fill, stroke and opacity mix) → {id, tween, ease: [x1, y1, x2, y2] | null}",
            has_doc,
            set_tween
        ),
        cmd!("anim.setFps", "Frame Rate", [], None, "{fps: 1–60} frames per second → {fps}", has_doc, set_fps),
        cmd!("anim.setFrames", "Timeline Length", [], None, "{frames: 1–10000} the timeline's length → {frames}", has_doc, set_frames),
        cmd!(
            query "anim.exportGif",
            "Export Animated GIF…",
            [],
            None,
            "{path, scale?: (default 1), artboard?: index (default 0), from?, to?} every frame of the artboard as an animated GIF at the frame rate → {path, frames, width, height}",
            has_doc,
            export_gif
        ),
        cmd!(
            query "anim.exportVideo",
            "Export Video…",
            [],
            None,
            "{path, format?: \"webm\" (default) | \"mp4\", fps?: 1–60 (default: the timeline's), scale?: (default 1), artboard?: index (default 0), from?, to?} the artboard's frames as a VP9 WebM or H.264 MP4 movie, transparency on white, with the timeline's sounds (Opus in WebM, AAC in MP4) → {path, frames, width, height, fps, sound}",
            has_doc,
            export_video
        ),
    ]
}

fn info(s: &mut Session, _: &Value) -> Result<Value> {
    let d = &s.doc()?.doc;
    let anim = Anim::of(d);
    let rows: Vec<Value> = d
        .layers
        .iter()
        .rev()
        .map(|r| {
            let keys: Vec<Value> = anim
                .row_keys(r)
                .into_iter()
                .map(|(f, id)| json!({"frame": f, "id": id.0, "tween": anim.tweens.contains(&id.0), "ease": anim.eases.get(&id.0)}))
                .collect();
            json!({"id": r.id.0, "name": r.name.clone().unwrap_or_default(), "keys": keys})
        })
        .collect();
    Ok(json!({"fps": anim.fps, "frames": anim.frames, "current": anim.current, "rows": rows}))
}

fn goto(s: &mut Session, p: &Value) -> Result<Value> {
    let anim = Anim::of(&s.doc()?.doc);
    let f = frame_param(p, &anim, "anim.goto")?;
    show(s, f)
}

fn next(s: &mut Session, _: &Value) -> Result<Value> {
    let a = Anim::of(&s.doc()?.doc);
    show(s, if a.current >= a.frames { 1 } else { a.current + 1 })
}

fn prev(s: &mut Session, _: &Value) -> Result<Value> {
    let a = Anim::of(&s.doc()?.doc);
    show(s, if a.current <= 1 { a.frames } else { a.current - 1 })
}

fn blank_keyframe(s: &mut Session, p: &Value) -> Result<Value> {
    let mut p = p.clone();
    if let Some(o) = p.as_object_mut() {
        o.insert("blank".into(), json!(true));
    } else {
        p = json!({"blank": true});
    }
    keyframe(s, &p)
}

fn keyframe(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "anim.keyframe";
    let row = row_param(s, p, C)?;
    let (anim, row_node) = {
        let d = &s.doc()?.doc;
        (Anim::of(d), d.node(row).cloned().ok_or_else(|| bad(C, "no such layer"))?)
    };
    let frame = frame_param(p, &anim, C)?;
    if anim.row_keys(&row_node).iter().any(|(f, _)| *f == frame) {
        return Err(bad(C, format!("this layer already has a keyframe at frame {frame}")));
    }
    let from = anim.key_at(&row_node, frame).filter(|_| !bool_or(p, "blank", false));
    let name = format!("Frame {frame}");
    s.begin_named_undo_group(Some("Insert Keyframe".into()));
    let made = (|| -> Result<NodeId> {
        let id = match from {
            Some(src) => {
                let r = s.execute("layer.duplicate", &json!({"ids": [src.0]}))?;
                let id = r["ids"].get(0).and_then(Value::as_u64).map(NodeId).ok_or_else(|| bad(C, "the copy failed"))?;
                s.execute("layer.setProps", &json!({"ids": [id.0], "name": name}))?;
                id
            }
            None => {
                let r = s.execute("layer.newSublayer", &json!({"parent": row.0, "name": name}))?;
                r["id"].as_u64().map(NodeId).ok_or_else(|| bad(C, "the new layer failed"))?
            }
        };
        s.edit("Insert Keyframe", |d, _| {
            let mut a = Anim::of(d);
            a.keys.insert(id.0, frame);
            a.store(d);
            Ok(())
        })?;
        Ok(id)
    })();
    s.end_undo_group(made.is_err());
    let id = made?;
    show(s, frame)?;
    s.doc_mut()?.active_layer = Some(id);
    Ok(json!({"id": id.0, "frame": frame}))
}

fn remove_keyframe(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "anim.removeKeyframe";
    let row = row_param(s, p, C)?;
    let d = &s.doc()?.doc;
    let anim = Anim::of(d);
    let frame = frame_param(p, &anim, C)?;
    let row_node = d.node(row).ok_or_else(|| bad(C, "no such layer"))?;
    let Some((_, id)) = anim.row_keys(row_node).into_iter().find(|(f, _)| *f == frame) else {
        return Err(bad(C, format!("no keyframe at frame {frame} on this layer")));
    };
    s.begin_named_undo_group(Some("Clear Keyframe".into()));
    let done = (|| -> Result<()> {
        s.execute("layer.delete", &json!({"ids": [id.0]}))?;
        s.edit("Clear Keyframe", |d, _| {
            let mut a = Anim::of(d);
            a.keys.remove(&id.0);
            a.store(d);
            Ok(())
        })
    })();
    s.end_undo_group(done.is_err());
    done?;
    show(s, frame)?;
    Ok(json!({"deleted": 1}))
}

fn onion(s: &mut Session, p: &Value) -> Result<Value> {
    let side = |k: &str, now: u32| -> Result<u32> {
        match p.get(k) {
            None | Some(Value::Null) => Ok(now),
            Some(v) => {
                v.as_u64().filter(|n| *n <= MAX_ONION as u64).map(|n| n as u32).ok_or_else(|| bad("anim.onion", format!("{k} must be from 0 to 5")))
            }
        }
    };
    let st = s.doc_mut()?;
    let mut d = (*st.doc).clone();
    let mut a = Anim::of(&d);
    a.onion_before = side("before", a.onion_before)?;
    a.onion_after = side("after", a.onion_after)?;
    a.store(&mut d);
    let current = a.current;
    st.doc = Arc::new(d);
    show(s, current)?;
    Ok(json!({"before": a.onion_before, "after": a.onion_after}))
}

/// `p.ease`: absent → None, "linear" → Some(None), a name or [x1, y1, x2, y2] → Some(Some(curve)).
pub(crate) fn ease_param(p: &Value, c: &str) -> Result<Option<Option<[f64; 4]>>> {
    Ok(match p.get("ease") {
        None | Some(Value::Null) => None,
        Some(Value::String(n)) if n == "linear" => Some(None),
        Some(Value::String(n)) => Some(Some(
            EASES
                .iter()
                .find(|(k, _)| k == n)
                .map(|(_, e)| *e)
                .ok_or_else(|| bad(c, format!("ease {n:?} isn't one of linear, easeIn, easeOut, easeInOut")))?,
        )),
        Some(v) => {
            let e: [f64; 4] = serde_json::from_value(v.clone())
                .ok()
                .filter(valid_ease)
                .ok_or_else(|| bad(c, "ease must be a name or [x1, y1, x2, y2] with x1 and x2 from 0 to 1"))?;
            Some(Some(e))
        }
    })
}

fn set_tween(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "anim.tween";
    let row = row_param(s, p, C)?;
    let d = &s.doc()?.doc;
    let anim = Anim::of(d);
    let frame = frame_param(p, &anim, C)?;
    let key = d.node(row).and_then(|r| anim.key_at(r, frame)).ok_or_else(|| bad(C, format!("no keyframe at frame {frame} on this layer")))?;
    let ease = ease_param(p, C)?;
    // Setting an ease turns the tween on; otherwise `on` toggles it.
    let on = p.get("on").and_then(Value::as_bool).unwrap_or(ease.is_some() || !anim.tweens.contains(&key.0));
    s.edit("Motion Tween", |d, _| {
        let mut a = Anim::of(d);
        if on {
            a.tweens.insert(key.0);
        } else {
            a.tweens.remove(&key.0);
        }
        match ease {
            Some(Some(e)) => _ = a.eases.insert(key.0, e),
            Some(None) => _ = a.eases.remove(&key.0),
            None => {}
        }
        a.store(d);
        Ok(())
    })?;
    show(s, anim.current)?;
    Ok(json!({"id": key.0, "tween": on, "ease": Anim::of(&s.doc()?.doc).eases.get(&key.0)}))
}

fn set_fps(s: &mut Session, p: &Value) -> Result<Value> {
    let fps = p
        .get("fps")
        .and_then(Value::as_u64)
        .filter(|f| (1..=MAX_FPS as u64).contains(f))
        .ok_or_else(|| bad("anim.setFps", "fps must be a whole number from 1 to 60"))? as u32;
    s.edit("Frame Rate", |d, _| {
        let mut a = Anim::of(d);
        a.fps = fps;
        a.store(d);
        Ok(())
    })?;
    Ok(json!({"fps": fps}))
}

fn set_frames(s: &mut Session, p: &Value) -> Result<Value> {
    let frames = p
        .get("frames")
        .and_then(Value::as_u64)
        .filter(|f| (1..=MAX_FRAMES as u64).contains(f))
        .ok_or_else(|| bad("anim.setFrames", "frames must be a whole number from 1 to 10000"))? as u32;
    s.edit("Timeline Length", |d, _| {
        let mut a = Anim::of(d);
        a.frames = frames;
        a.current = a.current.min(frames);
        a.store(d);
        Ok(())
    })?;
    Ok(json!({"frames": frames}))
}

fn export_gif(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_param(p, "path").ok_or_else(|| bad("anim.exportGif", "path is required"))?.to_string();
    let doc = s.doc()?.doc.clone();
    let (out, mut info) = encode_gif(&doc, p)?;
    super::fileio::write_file(&path, &out)?;
    info["path"] = json!(path);
    Ok(info)
}

/// What a movie export renders: the artboard, scale, pixel size and frame range from `p`.
struct Range {
    rect: Rect,
    scale: f64,
    w: u32,
    h: u32,
    from: u32,
    to: u32,
}

fn range(doc: &Document, anim: &Anim, p: &Value, c: &str) -> Result<Range> {
    let scale = f64_or(p, "scale", 1.0);
    if !scale.is_finite() || !(0.05..=8.0).contains(&scale) {
        return Err(bad(c, "scale must be from 0.05 to 8"));
    }
    let board = p.get("artboard").and_then(Value::as_u64).unwrap_or(0) as usize;
    let rect = doc.artboards.get(board).map(|a| a.rect).ok_or_else(|| bad(c, "no such artboard"))?;
    let (w, h) = vectorcraft_render::region_pixels(rect, scale);
    if w == 0 || h == 0 || (w as u64) * (h as u64) > MAX_EXPORT_PIXELS {
        return Err(bad(c, format!("{w} × {h} pixels is outside what an animation export takes")));
    }
    let from = p.get("from").and_then(Value::as_u64).map(|f| f as u32).unwrap_or(1).clamp(1, anim.frames);
    let to = p.get("to").and_then(Value::as_u64).map(|f| f as u32).unwrap_or(anim.frames).clamp(from, anim.frames);
    Ok(Range { rect, scale, w, h, from, to })
}

fn export_video(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_param(p, "path").ok_or_else(|| bad("anim.exportVideo", "path is required"))?.to_string();
    let doc = s.doc()?.doc.clone();
    let (out, mut info) = encode_video(&doc, p)?;
    super::fileio::write_file(&path, &out)?;
    info["path"] = json!(path);
    Ok(info)
}

/// `doc`'s frames as a WebM (VP9) or MP4 (H.264) movie (`anim.exportVideo`'s options in `p`, `path` aside) → the
/// bytes and `{frames, width, height, fps}`. Transparent areas come out white.
pub fn encode_video(doc: &Document, p: &Value) -> Result<(Vec<u8>, Value)> {
    const C: &str = "anim.exportVideo";
    let anim = Anim::of(doc);
    let fps = match p.get("fps") {
        None | Some(Value::Null) => anim.fps,
        Some(v) => v.as_u64().filter(|f| (1..=60).contains(f)).ok_or_else(|| bad(C, "fps must be a whole number from 1 to 60"))? as u32,
    };
    let mp4 = match str_param(p, "format").unwrap_or("webm") {
        "webm" => false,
        "mp4" => true,
        f => return Err(bad(C, format!("format {f:?} isn't supported (webm, mp4)"))),
    };
    let Range { rect, scale, w, h, from, to } = range(doc, &anim, p, C)?;
    enum Enc {
        Webm(Box<vectorcraft_video::WebmWriter>),
        Mp4(Box<vectorcraft_video::Mp4Writer>),
    }
    // The timeline's sounds, timed at the export's frame rate so they stay in step with the frames.
    let sound = super::anim_sound::mix_for_export(doc, from, to, fps)?;
    let mut enc = match (mp4, &sound) {
        (true, Some(a)) => Enc::Mp4(Box::new(vectorcraft_video::Mp4Writer::with_audio(w, h, fps, a).map_err(|e| bad(C, e))?)),
        (true, None) => Enc::Mp4(Box::new(vectorcraft_video::Mp4Writer::new(w, h, fps).map_err(|e| bad(C, e))?)),
        (false, Some(a)) => Enc::Webm(Box::new(vectorcraft_video::WebmWriter::with_audio(w, h, fps, a).map_err(|e| bad(C, e))?)),
        (false, None) => Enc::Webm(Box::new(vectorcraft_video::WebmWriter::new(w, h, fps).map_err(|e| bad(C, e))?)),
    };
    let mut r = vectorcraft_render::Renderer::new();
    for f in from..=to {
        let img = r.render_region(&through_camera(at_frame(doc, f), anim.camera_at(f), rect), rect, scale, true);
        let mut rgba = img.to_straight();
        // Video has no alpha: put the frame on white.
        for [r, g, b, a] in rgba.as_chunks_mut::<4>().0 {
            let k = *a as u32;
            for c in [r, g, b] {
                *c = ((*c as u32 * k + 255 * (255 - k)) / 255) as u8;
            }
            *a = 255;
        }
        match &mut enc {
            Enc::Webm(e) => e.push(&rgba),
            Enc::Mp4(e) => e.push(&rgba),
        }
        .map_err(|e| bad(C, e))?;
    }
    let out = match enc {
        Enc::Webm(e) => e.finish(),
        Enc::Mp4(e) => e.finish().map_err(|e| bad(C, e))?,
    };
    Ok((out, json!({"frames": to - from + 1, "width": w, "height": h, "fps": fps, "sound": sound.is_some()})))
}

/// `doc`'s frames as an animated GIF (`anim.exportGif`'s options in `p`, `path` aside) → the bytes
/// and `{frames, width, height}`. The UI writes them itself so the web can download them.
pub fn encode_gif(doc: &Document, p: &Value) -> Result<(Vec<u8>, Value)> {
    use image::codecs::gif::{GifEncoder, Repeat};
    use image::{Delay, Frame, RgbaImage};
    const C: &str = "anim.exportGif";
    let anim = Anim::of(doc);
    let Range { rect, scale, w, h, from, to } = range(doc, &anim, p, C)?;
    let delay = Delay::from_numer_denom_ms(1000, anim.fps.max(1));
    let mut out = Vec::new();
    {
        let mut enc = GifEncoder::new_with_speed(&mut out, 10);
        enc.set_repeat(Repeat::Infinite).map_err(|e| EngineError::Other(format!("GIF: {e}")))?;
        let mut r = vectorcraft_render::Renderer::new();
        for f in from..=to {
            let frame_doc = through_camera(at_frame(doc, f), anim.camera_at(f), rect);
            let img = r.render_region(&frame_doc, rect, scale, true);
            let rgba = RgbaImage::from_raw(img.width, img.height, img.to_straight()).ok_or_else(|| bad(C, "a frame didn't render"))?;
            enc.encode_frame(Frame::from_parts(rgba, 0, 0, delay)).map_err(|e| EngineError::Other(format!("GIF: {e}")))?;
        }
    }
    Ok((out, json!({"frames": to - from + 1, "width": w, "height": h})))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s
    }

    fn rect(s: &mut Session, x: f64) {
        s.execute("shape.rectangle", &json!({"x": x, "y": 10, "width": 20, "height": 20})).unwrap();
    }

    fn visible(s: &Session, id: u64) -> bool {
        s.doc().unwrap().doc.node(NodeId(id)).unwrap().visible
    }

    #[test]
    fn keyframes_hold_until_the_next_one_and_frames_switch_them() {
        let mut s = session();
        let k1 = s.execute("anim.keyframe", &json!({"frame": 1})).unwrap()["id"].as_u64().unwrap();
        rect(&mut s, 10.0);
        let k5 = s.execute("anim.keyframe", &json!({"frame": 5})).unwrap()["id"].as_u64().unwrap();
        // Frame 5's key is a copy of frame 1's art, and new art goes into it.
        assert_eq!(s.doc().unwrap().active_layer, Some(NodeId(k5)));
        assert_eq!(s.doc().unwrap().doc.node(NodeId(k5)).unwrap().children().unwrap().len(), 1);
        for (f, one, five) in [(1, true, false), (4, true, false), (5, false, true), (24, false, true)] {
            s.execute("anim.goto", &json!({"frame": f})).unwrap();
            assert_eq!((visible(&s, k1), visible(&s, k5)), (one, five), "frame {f}");
        }
        let info = s.execute("anim.info", &json!({})).unwrap();
        assert_eq!(info["current"], 24);
        assert_eq!(info["rows"][0]["keys"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn changing_frames_is_not_an_undo_step_but_keyframes_are() {
        let mut s = session();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("anim.blankKeyframe", &json!({"frame": 3})).unwrap();
        let before = s.doc().unwrap().history.undo.len();
        s.execute("anim.next", &json!({})).unwrap();
        s.execute("anim.prev", &json!({})).unwrap();
        assert_eq!(s.doc().unwrap().history.undo.len(), before);
        s.execute("edit.undo", &json!({})).unwrap();
        let info = s.execute("anim.info", &json!({})).unwrap();
        assert_eq!(info["rows"][0]["keys"].as_array().unwrap().len(), 1, "undo takes the blank keyframe back");
    }

    #[test]
    fn the_onion_skin_shows_neighbours_faintly_but_never_in_exports() {
        let mut s = session();
        let k1 = s.execute("anim.keyframe", &json!({"frame": 1})).unwrap()["id"].as_u64().unwrap();
        let k3 = s.execute("anim.blankKeyframe", &json!({"frame": 3})).unwrap()["id"].as_u64().unwrap();
        let k5 = s.execute("anim.blankKeyframe", &json!({"frame": 5})).unwrap()["id"].as_u64().unwrap();
        s.execute("anim.onion", &json!({"before": 1, "after": 1})).unwrap();
        s.execute("anim.goto", &json!({"frame": 3})).unwrap();
        let d = s.doc().unwrap().doc.clone();
        let n = |id: u64| d.node(NodeId(id)).unwrap().clone();
        assert!(n(k1).visible && n(k1).locked && n(k1).opacity < 0.5, "frame 1 faint");
        assert!(n(k3).visible && !n(k3).locked && n(k3).opacity == 1.0, "frame 3 itself");
        assert!(n(k5).visible && n(k5).locked, "frame 5 faint");
        let clean = at_frame(&d, 3);
        assert!(!clean.node(NodeId(k1)).unwrap().visible && clean.node(NodeId(k3)).unwrap().opacity == 1.0, "exports see only frame 3");
        s.execute("anim.onion", &json!({"before": 0, "after": 0})).unwrap();
        assert!(!s.doc().unwrap().doc.node(NodeId(k1)).unwrap().visible);
    }

    #[test]
    fn bad_params_are_errors() {
        let mut s = session();
        assert!(s.execute("anim.goto", &json!({"frame": 0})).is_err());
        assert!(s.execute("anim.goto", &json!({"frame": 99})).is_err());
        assert!(s.execute("anim.setFps", &json!({"fps": 0})).is_err());
        assert!(s.execute("anim.onion", &json!({"before": 9})).is_err());
        assert!(s.execute("anim.setFrames", &json!({"frames": 20000})).is_err());
        s.execute("anim.keyframe", &json!({"frame": 2})).unwrap();
        assert!(s.execute("anim.keyframe", &json!({"frame": 2})).is_err(), "one keyframe per frame");
        assert!(s.execute("anim.removeKeyframe", &json!({"frame": 7})).is_err());
        assert!(s.execute("anim.exportVideo", &json!({"path": "x.webm", "fps": 0})).is_err());
        assert!(s.execute("anim.exportVideo", &json!({"path": "x.webm", "format": "avi"})).is_err());
        assert!(s.execute("anim.exportVideo", &json!({"path": "x.webm", "scale": 99})).is_err());
    }

    #[test]
    fn the_timing_survives_save_and_open() {
        let mut s = session();
        s.execute("anim.setFps", &json!({"fps": 24})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 3})).unwrap();
        let doc = s.doc().unwrap().doc.clone();
        let bytes = vectorcraft_format::save(&doc, false);
        let back = vectorcraft_format::load(&bytes).unwrap();
        let a = Anim::of(&back);
        assert_eq!((a.fps, a.keys.len()), (24, 1));
    }

    #[test]
    fn a_motion_tween_moves_the_art_between_keyframes_without_changing_them() {
        let mut s = session();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        rect(&mut s, 0.0);
        s.execute("anim.blankKeyframe", &json!({"frame": 11})).unwrap();
        rect(&mut s, 100.0);
        s.execute("anim.goto", &json!({"frame": 1})).unwrap();
        assert_eq!(s.execute("anim.tween", &json!({})).unwrap()["tween"], true);
        s.execute("anim.goto", &json!({"frame": 6})).unwrap();
        let d = &s.doc().unwrap().doc;
        let ghosts = Anim::of(d).ghosts;
        assert_eq!(ghosts.len(), 1);
        let g = d.node(NodeId(ghosts[0])).unwrap();
        assert!(g.visible && g.locked);
        let x = g.children().unwrap()[0].geometric_bounds().unwrap().x0;
        assert!((x - 50.0).abs() < 1e-6, "halfway: {x}");
        // The keyframes keep their art, and leaving the frame takes the in-between away.
        s.execute("anim.goto", &json!({"frame": 11})).unwrap();
        let d = &s.doc().unwrap().doc;
        assert!(Anim::of(d).ghosts.is_empty());
        let a = d.layers[0].children().unwrap().iter().find(|c| c.is_layer()).unwrap();
        assert_eq!(a.children().unwrap()[0].geometric_bounds().unwrap().x0, 0.0);
        assert_eq!(d.layers[0].children().unwrap().len(), 2, "two keyframes, no leftover tween");
    }

    /// The tween layer's first object at the frame shown.
    fn tweened(s: &Session) -> Node {
        let d = &s.doc().unwrap().doc;
        let g = d.node(NodeId(Anim::of(d).ghosts[0])).unwrap();
        (*g.children().unwrap()[0]).clone()
    }

    fn first_child(s: &Session, key: u64) -> u64 {
        s.doc().unwrap().doc.node(NodeId(key)).unwrap().children().unwrap()[0].id.0
    }

    #[test]
    fn a_motion_tween_turns_and_recolours_along_the_way() {
        let mut s = session();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        rect(&mut s, 0.0);
        s.execute("paint.setFill", &json!({"color": "#ff0000"})).unwrap();
        let k5 = s.execute("anim.keyframe", &json!({"frame": 5})).unwrap()["id"].as_u64().unwrap();
        let c = first_child(&s, k5);
        s.execute("object.rotate", &json!({"ids": [c], "angle": 90})).unwrap();
        s.execute("object.move", &json!({"ids": [c], "dx": 100, "dy": 0})).unwrap();
        s.execute("paint.setFill", &json!({"ids": [c], "color": "#0000ff"})).unwrap();
        s.execute("anim.tween", &json!({"frame": 1, "on": true})).unwrap();
        s.execute("anim.goto", &json!({"frame": 3})).unwrap();
        let n = tweened(&s);
        let sp = &n.path_data().unwrap().subpaths[0].anchors;
        let v = sp[1].p - sp[0].p;
        let turn = v.y.atan2(v.x).to_degrees().rem_euclid(90.0);
        assert!((turn - 45.0).abs() < 0.5, "halfway turned 45°, got {turn}");
        let b = n.geometric_bounds().unwrap();
        assert!((b.center().x - 60.0).abs() < 0.5, "halfway moved: {}", b.center().x);
        let vectorcraft_color::Paint::Solid { color, .. } = n.appearance.fill_paint() else { panic!("solid fill") };
        let [r, _, bl] = color.to_rgb();
        assert!((r - 0.5).abs() < 0.05 && (bl - 0.5).abs() < 0.05, "colour halfway: {r} {bl}");
    }

    #[test]
    fn easing_curves_bend_the_timing() {
        assert_eq!(eased(None, 0.5), 0.5);
        let at = |n: &str, x: f64| eased(EASES.iter().find(|(k, _)| *k == n).map(|(_, e)| e), x);
        assert!(at("easeIn", 0.5) < 0.4 && at("easeOut", 0.5) > 0.6);
        assert!((at("easeInOut", 0.5) - 0.5).abs() < 1e-6);
        // CSS "ease" (0.25, 0.1, 0.25, 1) at 0.5 is about 0.8024.
        assert!((eased(Some(&[0.25, 0.1, 0.25, 1.0]), 0.5) - 0.8024).abs() < 1e-3);
        for x in [0.0, 1.0] {
            assert!((at("easeIn", x) - x).abs() < 1e-9);
        }
    }

    #[test]
    fn a_tween_keeps_its_ease_through_save_and_moves_by_it() {
        let mut s = session();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        rect(&mut s, 0.0);
        s.execute("anim.blankKeyframe", &json!({"frame": 11})).unwrap();
        rect(&mut s, 100.0);
        let r = s.execute("anim.tween", &json!({"frame": 1, "ease": "easeIn"})).unwrap();
        assert_eq!(r["tween"], true);
        assert!(s.execute("anim.tween", &json!({"frame": 1, "ease": "bouncy"})).is_err());
        assert!(s.execute("anim.tween", &json!({"frame": 1, "ease": [2, 0, 1, 1]})).is_err());
        s.execute("anim.goto", &json!({"frame": 6})).unwrap();
        let x = tweened(&s).geometric_bounds().unwrap().x0;
        assert!(x < 40.0, "eased in, behind halfway: {x}");
        let back = vectorcraft_format::load(&vectorcraft_format::save(&s.doc().unwrap().doc, false)).unwrap();
        assert_eq!(Anim::of(&back).eases.values().next(), Some(&[0.42, 0.0, 1.0, 1.0]));
        s.execute("anim.tween", &json!({"frame": 1, "ease": "linear"})).unwrap();
        assert!(Anim::of(&s.doc().unwrap().doc).eases.is_empty());
    }

    #[test]
    fn the_camera_holds_outside_its_keys_and_eases_between() {
        let mut a = Anim::default();
        assert_eq!(a.camera_at(5), None);
        let k = |x: f64, zoom: f64, rotation: f64| Cam { x, y: 0.0, zoom, rotation, ease: None };
        a.camera.insert(3, k(0.0, 1.0, 350.0));
        a.camera.insert(7, k(100.0, 4.0, 10.0));
        assert_eq!(a.camera_at(1).unwrap().x, 0.0);
        assert_eq!(a.camera_at(9).unwrap().x, 100.0);
        let mid = a.camera_at(5).unwrap();
        assert!((mid.x - 50.0).abs() < 1e-9 && (mid.zoom - 2.0).abs() < 1e-9, "zoom halfway is geometric: {mid:?}");
        assert!((mid.rotation - 360.0).abs() < 1e-9, "the short way round: {}", mid.rotation);
        // A view centred on (110, 20), twice as close: that point lands on the artboard's centre.
        let mut s = session();
        rect(&mut s, 100.0);
        let doc = s.doc().unwrap().doc.clone();
        let board = doc.artboards[0].rect;
        let seen = through_camera((*doc).clone(), Some(Cam { x: 110.0, y: 20.0, zoom: 2.0, rotation: 0.0, ease: None }), board);
        let b = seen.layers[0].children().unwrap()[0].geometric_bounds().unwrap();
        assert!((b.center() - board.center()).hypot() < 1e-6 && (b.width() - 40.0).abs() < 1e-6);
    }

    #[test]
    fn objects_pair_by_name_before_order_and_odd_pairs_morph() {
        let mut s = session();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        rect(&mut s, 0.0);
        s.execute("shape.ellipse", &json!({"x": 200, "y": 10, "width": 20, "height": 20})).unwrap();
        let k1 = Anim::of(&s.doc().unwrap().doc).keys.keys().next().copied().unwrap();
        let ids: Vec<u64> = s.doc().unwrap().doc.node(NodeId(k1)).unwrap().children().unwrap().iter().map(|c| c.id.0).collect();
        s.execute("layer.setProps", &json!({"ids": [ids[0]], "name": "box"})).unwrap();
        let k5 = s.execute("anim.keyframe", &json!({"frame": 5})).unwrap()["id"].as_u64().unwrap();
        // Swap the order in the second keyframe: the name still pairs the box with the box.
        let later: Vec<u64> = s.doc().unwrap().doc.node(NodeId(k5)).unwrap().children().unwrap().iter().map(|c| c.id.0).collect();
        s.execute("object.arrange", &json!({"ids": [later[0]], "to": "front"})).ok();
        s.execute("object.move", &json!({"ids": [later[0]], "dx": 40, "dy": 0})).unwrap();
        s.execute("anim.tween", &json!({"frame": 1, "on": true})).unwrap();
        s.execute("anim.goto", &json!({"frame": 3})).unwrap();
        let n = tweened(&s);
        assert_eq!(n.name.as_deref(), Some("box"));
        assert!((n.geometric_bounds().unwrap().x0 - 20.0).abs() < 0.5);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn exports_an_animated_gif() {
        let mut s = session();
        s.execute("anim.setFrames", &json!({"frames": 3})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        rect(&mut s, 10.0);
        s.execute("anim.blankKeyframe", &json!({"frame": 2})).unwrap();
        rect(&mut s, 60.0);
        let dir = std::env::temp_dir().join(format!("7rrek-gif-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("walk.gif");
        let r = s.execute("anim.exportGif", &json!({"path": path.to_string_lossy(), "scale": 0.25})).unwrap();
        assert_eq!(r["frames"], 3);
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..6], b"GIF89a");
        let path = dir.join("walk.webm");
        let r = s.execute("anim.exportVideo", &json!({"path": path.to_string_lossy(), "scale": 0.25, "fps": 6})).unwrap();
        assert_eq!((r["frames"].as_u64(), r["fps"].as_u64()), (Some(3), Some(6)));
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..4], &[0x1A, 0x45, 0xDF, 0xA3], "EBML header");
        if let Ok(o) = std::process::Command::new("ffprobe")
            .args(["-v", "error", "-count_frames", "-select_streams", "v:0", "-show_entries", "stream=nb_read_frames,codec_name", "-of", "csv=p=0"])
            .arg(&path)
            .output()
        {
            assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "vp9,3");
        }
        let path = dir.join("walk.mp4");
        s.execute("anim.exportVideo", &json!({"path": path.to_string_lossy(), "scale": 0.25, "format": "mp4"})).unwrap();
        assert_eq!(std::fs::read(&path).unwrap().get(4..8), Some(&b"ftyp"[..]));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_new_layer_made_from_a_keyframe_is_a_new_row() {
        let mut s = session();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        let id = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
        let d = &s.doc().unwrap().doc;
        assert_eq!(d.layers.len(), 2, "a top-level row, not a sublayer beside the keyframe");
        assert!(d.layers.iter().any(|l| l.id.0 == id));
    }
}
