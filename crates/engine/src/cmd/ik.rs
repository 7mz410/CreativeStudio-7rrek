//! 7rrek: inverse kinematics for bone chains (two-bone analytic, FABRIK beyond).
//!
//! Pure functions over points: no document state. Angles are degrees, measured with `atan2` on
//! document coordinates (y down), so a positive turn is clockwise on screen.

use std::f64::consts::PI;

use vectorcraft_geom::{Point, Vec2};

/// Below this length a vector has no usable direction.
const EPS: f64 = 1e-12;

fn finite(p: Point) -> bool {
    p.x.is_finite() && p.y.is_finite()
}

/// Wraps an angle in radians to [-PI, PI).
fn wrap_rad(a: f64) -> f64 {
    (a + PI).rem_euclid(2.0 * PI) - PI
}

/// Wraps an angle in degrees to [-180, 180).
fn wrap_deg(a: f64) -> f64 {
    (a + 180.0).rem_euclid(360.0) - 180.0
}

fn angle(v: Vec2) -> f64 {
    v.y.atan2(v.x)
}

/// Unit vector along `v`, or +x when `v` is (near) zero.
fn unit(v: Vec2) -> Vec2 {
    let len = v.hypot();
    if len > EPS { v / len } else { Vec2::new(1.0, 0.0) }
}

/// The point `len` away from `from`, on the side facing `toward`.
fn place(from: Point, toward: Point, len: f64) -> Point {
    from + unit(toward - from) * len
}

/// Turns (degrees) for the root and mid joints of a two-bone arm so that `end` reaches `target`.
///
/// Returns `(root_turn, mid_turn)`: the root turn is the absolute change of the first bone, the
/// mid turn is local to the root bone (the change in the elbow angle). `bend_sign` is +1 or -1
/// and keeps the elbow on the same side: the sign of the cross product of the first bone and the
/// second bone in the current pose. Unreachable targets give a straight arm toward the target;
/// targets closer than `|l1 - l2|` fold the arm fully toward them. `None` for zero-length bones
/// or non-finite input.
pub fn two_bone(root: Point, mid: Point, end: Point, target: Point, bend_sign: f64) -> Option<(f64, f64)> {
    if !(finite(root) && finite(mid) && finite(end) && finite(target) && bend_sign.is_finite()) {
        return None;
    }
    let b1 = mid - root;
    let b2 = end - mid;
    let l1 = b1.hypot();
    let l2 = b2.hypot();
    if !(l1 > EPS && l2 > EPS) {
        return None;
    }
    let s = if bend_sign < 0.0 { -1.0 } else { 1.0 };

    let to_target = target - root;
    let d = to_target.hypot();
    let reachable_dir = d > EPS;
    // Direction to the target. A target exactly on the root keeps the current first bone.
    let theta_t = if reachable_dir { angle(to_target) } else { angle(b1) };
    // Angle at the root between the target direction and the first bone.
    let alpha = if reachable_dir {
        ((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d)).clamp(-1.0, 1.0).acos()
    } else if l2 > l1 {
        PI
    } else {
        0.0
    };
    let theta1 = theta_t - s * alpha;
    // Interior angle at the elbow; the bones turn by its supplement, on the chosen side.
    let interior = ((l1 * l1 + l2 * l2 - d * d) / (2.0 * l1 * l2)).clamp(-1.0, 1.0).acos();
    let theta2 = theta1 + s * (PI - interior);

    let d0 = wrap_rad(theta1 - angle(b1));
    let d1 = wrap_rad(theta2 - angle(b2));
    let root_turn = wrap_deg(d0.to_degrees());
    let mid_turn = wrap_deg(wrap_rad(d1 - d0).to_degrees());
    if root_turn.is_finite() && mid_turn.is_finite() { Some((root_turn, mid_turn)) } else { None }
}

/// New joint positions for a chain so that its last joint reaches `target`, root fixed and every
/// bone length kept (FABRIK, Aristidou & Lasenby 2011).
///
/// Iterates until the end is within `tol` of the target or `max_iter` passes. An unreachable
/// target gives the chain laid straight toward it. `None` for fewer than two joints, zero-length
/// bones, or non-finite input.
pub fn fabrik(chain: &[Point], target: Point, tol: f64, max_iter: usize) -> Option<Vec<Point>> {
    if chain.len() < 2 || !finite(target) || !tol.is_finite() || tol < 0.0 {
        return None;
    }
    if chain.iter().any(|p| !finite(*p)) {
        return None;
    }
    let lengths: Vec<f64> = chain
        .windows(2)
        .filter_map(|w| match w {
            [a, b] => Some((*b - *a).hypot()),
            _ => None,
        })
        .collect();
    if lengths.iter().any(|&l| !(l > EPS && l.is_finite())) {
        return None;
    }
    let total: f64 = lengths.iter().sum();
    if !total.is_finite() {
        return None;
    }
    let root = *chain.first()?;
    let to_target = target - root;

    if to_target.hypot() >= total {
        // Unreachable: lay the chain straight toward the target.
        let u = unit(to_target);
        let mut p = root;
        let mut out = vec![root];
        for l in &lengths {
            p += u * *l;
            out.push(p);
        }
        return Some(out);
    }

    let mut pts = chain.to_vec();
    for _ in 0..max_iter {
        let Some(end) = pts.last().copied() else { break };
        if (end - target).hypot() <= tol {
            break;
        }
        // Forward pass: pin the end on the target and pull each joint along its bone.
        if let Some(last) = pts.last_mut() {
            *last = target;
        }
        let mut next = target;
        for (i, l) in lengths.iter().enumerate().rev() {
            if let Some(p) = pts.get_mut(i) {
                *p = place(next, *p, *l);
                next = *p;
            }
        }
        // Backward pass: pin the root back and push each joint out along its bone.
        if let Some(first) = pts.first_mut() {
            *first = root;
        }
        let mut prev = root;
        for (i, l) in lengths.iter().enumerate() {
            if let Some(p) = pts.get_mut(i + 1) {
                *p = place(prev, *p, *l);
                prev = *p;
            }
        }
    }
    Some(pts)
}

/// Per-joint local turns (degrees) that turn the rig from `before` to `after`.
///
/// Joint i's turn is the change of bone i to i+1 minus the change of its parent bone, so a
/// parent-relative rig reproduces `after`. Joint 0's turn is its bone's absolute change. Each
/// turn is wrapped to [-180, 180). Returns one value per bone (the joints except the last); the
/// two slices are compared pairwise up to the shorter length.
pub fn turns(before: &[Point], after: &[Point]) -> Vec<f64> {
    let deltas: Vec<f64> = before
        .windows(2)
        .zip(after.windows(2))
        .map(|pair| match pair {
            ([b0, b1], [a0, a1]) => wrap_rad(angle(*a1 - *a0) - angle(*b1 - *b0)),
            _ => 0.0,
        })
        .collect();
    let mut parent: Option<f64> = None;
    deltas
        .into_iter()
        .map(|d| {
            let local = match parent {
                None => d,
                Some(p) => wrap_rad(d - p),
            };
            parent = Some(d);
            let deg = wrap_deg(local.to_degrees());
            if deg.is_finite() { deg } else { 0.0 }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn rot(pt: Point, about: Point, deg: f64) -> Point {
        let (sin, cos) = deg.to_radians().sin_cos();
        let v = pt - about;
        about + Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
    }

    /// Rig with parent-relative turns: root turn rotates mid and end about the root, mid turn
    /// rotates the end about the (already turned) mid.
    fn apply(root: Point, mid: Point, end: Point, turn: (f64, f64)) -> (Point, Point) {
        let m = rot(mid, root, turn.0);
        let e = rot(end, root, turn.0);
        (m, rot(e, m, turn.1))
    }

    fn cross(a: Vec2, b: Vec2) -> f64 {
        a.x * b.y - a.y * b.x
    }

    #[test]
    fn two_bone_reaches_reachable_targets() {
        let cases = [
            (p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0), p(1.0, 1.0)),
            (p(0.0, 0.0), p(2.0, 0.0), p(3.0, 0.0), p(-1.0, 1.0)),
            (p(0.0, 0.0), p(0.0, -2.0), p(1.0, -3.0), p(2.0, -1.5)),
            (p(5.0, 5.0), p(5.0, 4.0), p(4.0, 3.0), p(3.0, 4.5)),
        ];
        for (root, mid, end, target) in cases {
            let sign = if cross(mid - root, end - mid) < 0.0 { -1.0 } else { 1.0 };
            let turn = two_bone(root, mid, end, target, sign).expect("reachable");
            let (m, e) = apply(root, mid, end, turn);
            assert!((e - target).hypot() < 0.5, "end {e:?} misses {target:?}");
            assert!(((m - root).hypot() - (mid - root).hypot()).abs() < 1e-9);
            assert!(((e - m).hypot() - (end - mid).hypot()).abs() < 1e-9);
        }
    }

    #[test]
    fn two_bone_keeps_bend_sign() {
        // Same target, both elbow sides: each keeps the side it was asked for.
        let (root, mid, end, target) = (p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0), p(1.0, 1.0));
        for sign in [1.0, -1.0] {
            let turn = two_bone(root, mid, end, target, sign).expect("reachable");
            let (m, e) = apply(root, mid, end, turn);
            assert!((e - target).hypot() < 0.5);
            assert!(cross(m - root, e - m).signum() == sign, "bend flipped for {sign}");
        }
    }

    #[test]
    fn two_bone_unreachable_points_straight_at_target() {
        let (root, mid, end) = (p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0));
        for target in [p(0.0, 10.0), p(-3.0, 4.0)] {
            let turn = two_bone(root, mid, end, target, 1.0).expect("unreachable still solves");
            assert!(turn.0.is_finite() && turn.1.is_finite());
            let (m, e) = apply(root, mid, end, turn);
            let dir = unit(target - root);
            assert!(((m - root).hypot() - 1.0).abs() < 1e-9);
            assert!(((e - root).hypot() - 2.0).abs() < 1e-9);
            assert!(cross(e - root, dir).abs() < 1e-9, "arm not aimed at {target:?}");
            assert!((e - root).dot(dir) > 0.0);
        }
    }

    #[test]
    fn two_bone_folds_when_target_is_too_close() {
        // l1 = 2, l2 = 1: closest reachable end is 1 from the root, so 0.5 from the target.
        let (root, mid, end) = (p(0.0, 0.0), p(2.0, 0.0), p(3.0, 0.0));
        let target = p(0.5, 0.0);
        let turn = two_bone(root, mid, end, target, 1.0).expect("solves");
        let (_, e) = apply(root, mid, end, turn);
        assert!(((e - target).hypot() - 0.5).abs() < 1e-6, "end {e:?}");
    }

    #[test]
    fn two_bone_rejects_degenerate_input() {
        let (root, mid, end) = (p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0));
        assert!(two_bone(root, root, end, p(1.0, 1.0), 1.0).is_none());
        assert!(two_bone(root, mid, mid, p(1.0, 1.0), 1.0).is_none());
        assert!(two_bone(root, mid, end, p(f64::NAN, 0.0), 1.0).is_none());
        assert!(two_bone(root, mid, end, p(1.0, 1.0), f64::INFINITY).is_none());
    }

    #[test]
    fn fabrik_reaches_target_and_keeps_lengths() {
        let chain = [p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0), p(3.0, 0.0)];
        let lengths: Vec<f64> = chain.windows(2).map(|w| (w[1] - w[0]).hypot()).collect();
        for target in [p(1.5, 1.5), p(-1.0, 1.5)] {
            let out = fabrik(&chain, target, 0.01, 200).expect("solves");
            assert_eq!(out.len(), chain.len());
            assert_eq!(out[0], chain[0], "root moved");
            let end = *out.last().expect("non-empty");
            assert!((end - target).hypot() < 0.5, "end {end:?} misses {target:?}");
            for (i, l) in lengths.iter().enumerate() {
                let got = (out[i + 1] - out[i]).hypot();
                assert!((got - l).abs() < 1e-6, "bone {i} length {got} != {l}");
            }
        }
    }

    #[test]
    fn fabrik_unreachable_is_straight() {
        let chain = [p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0)];
        let target = p(0.0, 10.0);
        let out = fabrik(&chain, target, 0.01, 50).expect("solves");
        let dir = unit(target - chain[0]);
        for (i, q) in out.iter().enumerate() {
            assert!(q.x.is_finite() && q.y.is_finite());
            assert!(((*q - chain[0]).hypot() - i as f64).abs() < 1e-9);
            assert!(cross(*q - chain[0], dir).abs() < 1e-9);
        }
    }

    #[test]
    fn fabrik_rejects_bad_input() {
        let chain = [p(0.0, 0.0), p(1.0, 0.0)];
        assert!(fabrik(&chain[..1], p(1.0, 1.0), 0.01, 10).is_none());
        assert!(fabrik(&[], p(1.0, 1.0), 0.01, 10).is_none());
        assert!(fabrik(&chain, p(f64::NAN, 1.0), 0.01, 10).is_none());
        assert!(fabrik(&[p(0.0, 0.0), p(0.0, 0.0)], p(1.0, 1.0), 0.01, 10).is_none());
    }

    #[test]
    fn turns_round_trips_two_bone() {
        let (root, mid, end, target) = (p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0), p(1.0, 1.0));
        let want = two_bone(root, mid, end, target, 1.0).expect("solves");
        let (m, e) = apply(root, mid, end, want);
        let got = turns(&[root, mid, end], &[root, m, e]);
        assert_eq!(got.len(), 2);
        assert!((got[0] - want.0).abs() < 1e-6, "root {} vs {}", got[0], want.0);
        assert!((got[1] - want.1).abs() < 1e-6, "mid {} vs {}", got[1], want.1);
    }

    #[test]
    fn turns_wraps_and_handles_short_input() {
        // 170 degrees to -170 is a 20 degree turn through 180, not -340.
        let at = |deg: f64| {
            let (s, c) = deg.to_radians().sin_cos();
            p(c, s)
        };
        let before = [p(0.0, 0.0), at(170.0)];
        let after = [p(0.0, 0.0), at(-170.0)];
        let t = turns(&before, &after);
        assert_eq!(t.len(), 1);
        assert!((t[0] - 20.0).abs() < 1e-9, "turn {}", t[0]);
        assert!(turns(&[], &[]).is_empty());
        assert!(turns(&[p(0.0, 0.0)], &[p(1.0, 1.0)]).is_empty());
    }
}
