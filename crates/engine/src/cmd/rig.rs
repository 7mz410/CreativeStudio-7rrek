//! 7rrek (Creative Studio): bones. A layer (a timeline row) can hold a skeleton: joints with
//! parents in a rest pose, and pose keys (an angle per joint) on frames, eased like tweens. The
//! frame shown bends the row's paths along the bones by linear blend skinning, with weights from
//! each point's distance to each bone. The rig lives in the document's preserved extra data
//! (`"rig"`), apart from the animation's timing, so the `.7rrek` format is unchanged.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use vectorcraft_doc::{Document, NodeId, NodeKind};
use vectorcraft_geom::{Affine, Anchor, Point, SubPath, Vec2};

use super::anim::{eased, valid_ease};

const KEY: &str = "rig";
/// Most joints a skeleton takes.
pub const MAX_JOINTS: usize = 256;

/// A joint at its rest position (document points); the root has no parent.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Joint {
    pub name: String,
    pub parent: Option<usize>,
    pub x: f64,
    pub y: f64,
}

/// A pose key: each joint's turn from rest (degrees, the canvas's positive sense), easing toward
/// the next key.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Pose {
    pub angles: Vec<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ease: Option<[f64; 4]>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Skeleton {
    pub joints: Vec<Joint>,
    /// Frame → pose.
    pub keys: BTreeMap<u32, Pose>,
}

/// Every skeleton in a document, by the id of the layer (timeline row) it bends.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Rig {
    pub skeletons: BTreeMap<u64, Skeleton>,
}

impl Rig {
    pub fn of(doc: &Document) -> Self {
        let mut r: Rig = doc.unknown.get(KEY).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
        r.skeletons.retain(|_, s| s.valid());
        r
    }

    pub fn store(&self, doc: &mut Document) {
        if self.skeletons.is_empty() {
            doc.unknown.remove(KEY);
        } else if let Ok(v) = serde_json::to_value(self) {
            doc.unknown.insert(KEY.into(), v);
        }
    }
}

/// The skeleton of layer `row` posed at the current frame, for the Bone tool.
pub fn bones(doc: &Document, row: Option<NodeId>) -> Option<vectorcraft_tools::Bones> {
    let row = super::anim::row_of(doc, row?)?.0;
    let sk = Rig::of(doc).skeletons.remove(&row)?;
    let frame = super::anim::Anim::of(doc).current;
    let angles = sk.pose_at(frame).unwrap_or_else(|| vec![0.0; sk.joints.len()]);
    let at = sk.matrices(&angles).iter().zip(&sk.joints).map(|(m, j)| *m * Point::new(j.x, j.y)).collect();
    Some(vectorcraft_tools::Bones { row, frame, at, parents: sk.joints.iter().map(|j| j.parent).collect(), angles })
}

impl Skeleton {
    /// Parents come before their children, every coordinate and angle is finite, and every pose
    /// has an angle per joint.
    pub fn valid(&self) -> bool {
        self.joints.len() <= MAX_JOINTS
            && self.joints.iter().enumerate().all(|(i, j)| j.parent.is_none_or(|p| p < i) && j.x.is_finite() && j.y.is_finite())
            && self
                .keys
                .values()
                .all(|p| p.angles.len() == self.joints.len() && p.angles.iter().all(|a| a.is_finite()) && p.ease.as_ref().is_none_or(valid_ease))
    }

    /// The joints' angles at `frame`: held before the first key and after the last, eased in
    /// between, each turning the short way round. None without keys.
    pub fn pose_at(&self, frame: u32) -> Option<Vec<f64>> {
        let before = self.keys.range(..=frame).next_back();
        let after = self.keys.range(frame..).next();
        match (before, after) {
            (Some((fa, a)), Some((fb, b))) if fb > fa => {
                let t = eased(a.ease.as_ref(), (frame - fa) as f64 / (fb - fa) as f64);
                Some(a.angles.iter().zip(&b.angles).map(|(x, y)| x + ((y - x + 180.0).rem_euclid(360.0) - 180.0) * t).collect())
            }
            (Some((_, p)), _) | (None, Some((_, p))) => Some(p.angles.clone()),
            (None, None) => None,
        }
    }

    /// Each joint's transform for `angles`: its parent's, then a turn about the joint's rest spot.
    pub fn matrices(&self, angles: &[f64]) -> Vec<Affine> {
        let mut m: Vec<Affine> = Vec::with_capacity(self.joints.len());
        for (i, j) in self.joints.iter().enumerate() {
            let parent = j.parent.and_then(|p| m.get(p).copied()).unwrap_or(Affine::IDENTITY);
            let a = angles.get(i).copied().unwrap_or(0.0).to_radians();
            let at = Vec2::new(j.x, j.y);
            m.push(parent * Affine::translate(at) * Affine::rotate(a) * Affine::translate(-at));
        }
        m
    }

    /// The bones: (parent joint, child joint) pairs. A bone moves with its parent joint's turn.
    fn bones(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.joints.iter().enumerate().filter_map(|(i, j)| j.parent.map(|p| (p, i)))
    }

    /// `q` bent by the pose `m` (from [`matrices`](Self::matrices)): the bones' transforms mixed by
    /// inverse squared distance from `q` to each bone at rest.
    pub fn skin(&self, m: &[Affine], q: Point) -> Point {
        let mut sum = Vec2::ZERO;
        let mut total = 0.0;
        for (p, c) in self.bones() {
            let (Some(a), Some(b), Some(xf)) = (self.joints.get(p), self.joints.get(c), m.get(p)) else { continue };
            let d = dist_to_segment(q, Point::new(a.x, a.y), Point::new(b.x, b.y));
            let w = 1.0 / (d * d + 1.0);
            sum += (*xf * q).to_vec2() * w;
            total += w;
        }
        if total > 0.0 { (sum / total).to_point() } else { q }
    }
}

/// Longest piece (points) a segment keeps before it is split, so a straight edge can bend.
const PIECE: f64 = 12.0;

/// `sp` with its long segments split into pieces of at most about [`PIECE`] (same shape).
fn densify(sp: &mut SubPath) {
    let n = sp.anchors.len();
    if n < 2 {
        return;
    }
    let segs = if sp.closed { n } else { n - 1 };
    let mut out: Vec<Anchor> = Vec::with_capacity(n * 2);
    for i in 0..segs {
        let (Some(a), Some(b)) = (sp.anchors.get(i), sp.anchors.get((i + 1) % n)) else { return };
        let mut a = if i == 0 { *a } else { out.pop().unwrap_or(*a) };
        let mut cur = [a.p, a.h_out, b.h_in, b.p];
        let len = cur[0].distance(cur[1]) + cur[1].distance(cur[2]) + cur[2].distance(cur[3]);
        let k = ((len / PIECE).ceil() as usize).clamp(1, 64);
        for j in 0..k - 1 {
            // Split off 1/(remaining pieces) of what is left (de Casteljau).
            let t = 1.0 / (k - j) as f64;
            let [p0, p1, p2, p3] = cur;
            let (q0, q1, q2) = (p0.lerp(p1, t), p1.lerp(p2, t), p2.lerp(p3, t));
            let (r0, r1) = (q0.lerp(q1, t), q1.lerp(q2, t));
            let m = r0.lerp(r1, t);
            a.h_out = q0;
            out.push(a);
            a = Anchor::with_handles(m, r0, r1);
            cur = [m, r1, q2, p3];
        }
        a.h_out = cur[1];
        out.push(a);
        let mut end = *b;
        end.h_in = cur[2];
        out.push(end);
    }
    if sp.closed {
        // The last piece ends on the first anchor: keep its in handle there.
        if let (Some(last), Some(first)) = (out.pop(), out.first_mut()) {
            first.h_in = last.h_in;
        }
    }
    sp.anchors = out;
}

fn dist_to_segment(q: Point, a: Point, b: Point) -> f64 {
    let ab = b - a;
    let len2 = ab.hypot2();
    let t = if len2 > 1e-12 { ((q - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (q - (a + ab * t)).hypot()
}

/// Bend every path under each rigged row of `doc` to its skeleton's pose at `frame`.
pub fn apply(doc: &mut Document, frame: u32) {
    let rig = Rig::of(doc);
    for (row, sk) in &rig.skeletons {
        let Some(angles) = sk.pose_at(frame) else { continue };
        if angles.iter().all(|a| *a == 0.0) {
            continue;
        }
        let m = sk.matrices(&angles);
        let Some(node) = doc.node(NodeId(*row)) else { continue };
        let mut paths = Vec::new();
        node.walk(&mut |n| {
            if matches!(n.kind, NodeKind::Path { .. }) {
                paths.push(n.id);
            }
        });
        for id in paths {
            let Some(n) = doc.node_mut(id) else { continue };
            // The path no longer matches its live shape's parameters once bent.
            if let NodeKind::Path { live, .. } = &mut n.kind {
                *live = None;
            }
            let Some(path) = n.path_data_mut() else { continue };
            for sp in &mut path.subpaths {
                densify(sp);
                for k in &mut sp.anchors {
                    (k.p, k.h_in, k.h_out) = (sk.skin(&m, k.p), sk.skin(&m, k.h_in), sk.skin(&m, k.h_out));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shoulder at 0, elbow at 100, hand at 200, along x.
    fn arm() -> Skeleton {
        let j = |name: &str, parent, x| Joint { name: name.into(), parent, x, y: 0.0 };
        Skeleton { joints: vec![j("shoulder", None, 0.0), j("elbow", Some(0), 100.0), j("hand", Some(1), 200.0)], keys: BTreeMap::new() }
    }

    #[test]
    fn an_elbow_bends_the_forearm_and_leaves_the_upper_arm() {
        let sk = arm();
        let m = sk.matrices(&[0.0, 90.0, 0.0]);
        let hand = sk.skin(&m, Point::new(200.0, 0.0));
        let expect = Affine::translate((100.0, 0.0)) * Affine::rotate(90f64.to_radians()) * Point::new(100.0, 0.0);
        assert!((hand - expect).hypot() < 0.5, "hand at {hand:?}, want {expect:?}");
        let near_shoulder = sk.skin(&m, Point::new(10.0, 0.0));
        assert!((near_shoulder - Point::new(10.0, 0.0)).hypot() < 1.0, "{near_shoulder:?}");
    }

    #[test]
    fn poses_hold_outside_their_keys_and_turn_the_short_way() {
        let mut sk = arm();
        assert_eq!(sk.pose_at(3), None);
        sk.keys.insert(1, Pose { angles: vec![0.0, 350.0, 0.0], ease: None });
        sk.keys.insert(5, Pose { angles: vec![0.0, 10.0, 0.0], ease: None });
        assert_eq!(sk.pose_at(9).unwrap()[1], 10.0);
        assert!((sk.pose_at(3).unwrap()[1] - 360.0).abs() < 1e-9);
    }

    #[test]
    fn densifying_keeps_the_shape_and_splits_long_edges() {
        let sq = |c| SubPath::new([(0.0, 0.0), (100.0, 0.0), (100.0, 20.0), (0.0, 20.0)].map(|(x, y)| Anchor::corner(Point::new(x, y))).to_vec(), c);
        for closed in [true, false] {
            let mut sp = sq(closed);
            densify(&mut sp);
            assert!(sp.anchors.len() > 10, "long edges split");
            assert!(
                sp.anchors.iter().all(|k| k.p.y.abs() < 1e-9 || (k.p.y - 20.0).abs() < 1e-9 || k.p.x.abs() < 1e-9 || (k.p.x - 100.0).abs() < 1e-9),
                "points stay on the rectangle"
            );
            assert_eq!(sp.anchors.first().map(|k| k.p), Some(Point::ZERO));
        }
    }

    #[test]
    fn a_bad_skeleton_is_dropped() {
        let mut sk = arm();
        sk.joints[0].parent = Some(2);
        assert!(!sk.valid());
        let mut sk = arm();
        sk.keys.insert(1, Pose { angles: vec![0.0], ease: None });
        assert!(!sk.valid());
    }

    #[test]
    fn the_frame_shown_bends_a_rigged_rows_art() {
        use serde_json::json;
        let mut s = crate::Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 0, "y": -10, "width": 200, "height": 20})).unwrap();
        let row = s.doc().unwrap().doc.layers[0].id.0;
        let mut sk = arm();
        sk.keys.insert(1, Pose { angles: vec![0.0, 0.0, 0.0], ease: None });
        sk.keys.insert(9, Pose { angles: vec![0.0, 90.0, 0.0], ease: None });
        s.edit("Rig", |d, _| {
            Rig { skeletons: BTreeMap::from([(row, sk)]) }.store(d);
            Ok(())
        })
        .unwrap();
        let bent = crate::cmd::anim::at_frame(&s.doc().unwrap().doc, 9);
        let back = vectorcraft_format::load(&vectorcraft_format::save(&bent, false)).unwrap();
        assert_eq!(Rig::of(&back).skeletons.len(), 1, "the rig survives save and load");
        let mut tips = Vec::new();
        bent.layers[0].walk(&mut |n| {
            if let Some(p) = n.path_data() {
                tips.extend(p.subpaths.iter().flat_map(|sp| sp.anchors.iter().map(|k| k.p)));
            }
        });
        // The far end of the bar swung down by the elbow: nothing is left past x = 120.
        assert!(!tips.is_empty() && tips.iter().all(|p| p.x < 120.0), "{tips:?}");
        assert!(tips.iter().any(|p| p.y > 80.0), "the forearm hangs down: {tips:?}");
    }
}
