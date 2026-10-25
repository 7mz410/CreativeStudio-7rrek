//! The Bone tool: click to add a joint, drag a joint to pose the skeleton at the current frame.

use serde_json::json;
use vectorcraft_geom::Point;

use crate::{Action, Bones, Cursor, Mods, PointerEvent, PointerKind, Tool, ToolContext};

#[derive(Default)]
pub struct BoneTool {
    /// The gesture since the last press.
    press: Option<Press>,
    /// The drag in progress (a pose or a root move), begun on the first real movement.
    drag: Option<Drag>,
    /// The joint the tool last added or pressed; a click on empty canvas hangs the new one from it.
    last: Option<usize>,
}

#[derive(Clone, Copy)]
struct Press {
    at: Point,
    /// The joint under the press, if any.
    joint: Option<usize>,
    moved: bool,
}

#[derive(Clone, Copy)]
enum Drag {
    /// Turns joint `joint`'s children (the dragged joint's bone) about `pivot` = joint `joint`.
    Pose { joint: usize, base: f64, from: f64, pivot: Point, frame: u32 },
    /// Moves a root joint at rest.
    Move { index: usize },
    /// Cmd-drag: the joint reaches for the pointer and the chain above it follows (IK).
    Reach { joint: usize, frame: u32 },
}

impl Drag {
    fn preview(self, pos: Point) -> Option<Action> {
        match self {
            Drag::Pose { joint, base, from, pivot, frame } => {
                let angle = base + wrap(angle_about(pivot, pos) - from);
                angle.is_finite().then(|| Action::Preview("rig.pose".into(), json!({ "frame": frame, "joint": joint, "angle": angle })))
            }
            Drag::Move { index } => Some(Action::Preview("rig.moveJoint".into(), json!({ "index": index, "x": pos.x, "y": pos.y }))),
            Drag::Reach { joint, frame } => {
                Some(Action::Preview("rig.reach".into(), json!({ "frame": frame, "joint": joint, "x": pos.x, "y": pos.y })))
            }
        }
    }
}

/// Degrees of the direction from `c` to `p`, in the same sense as `atan2` on document coordinates.
fn angle_about(c: Point, p: Point) -> f64 {
    let d = p - c;
    d.y.atan2(d.x).to_degrees()
}

/// Folds a turn into -180..180 so a drag across the 180° line does not jump.
fn wrap(deg: f64) -> f64 {
    (deg + 180.0).rem_euclid(360.0) - 180.0
}

/// The nearest joint within `tol` of `p`.
fn joint_at(bones: &Bones, p: Point, tol: f64) -> Option<usize> {
    bones
        .at
        .iter()
        .enumerate()
        .map(|(i, &a)| (i, (a - p).hypot()))
        .filter(|&(_, d)| d.is_finite() && d <= tol)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

impl BoneTool {
    /// The drag a press on joint `j` starts, with its undo label.
    fn start(bones: &Bones, j: usize, press_at: Point, reach: bool) -> Option<(Drag, &'static str)> {
        bones.at.get(j)?;
        match bones.parents.get(j).copied().flatten() {
            Some(_) if reach => Some((Drag::Reach { joint: j, frame: bones.frame }, "Reach")),
            Some(p) => {
                let pivot = *bones.at.get(p)?;
                let base = bones.angles.get(p).copied().unwrap_or(0.0);
                let drag = Drag::Pose { joint: p, base, from: angle_about(pivot, press_at), pivot, frame: bones.frame };
                Some((drag, "Pose Joint"))
            }
            None => Some((Drag::Move { index: j }, "Move Joint")),
        }
    }

    fn drag(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        let Some(press) = self.press else { return vec![] };
        let moved = press.moved || (ev.pos - press.at).hypot() > cx.tol(2.0);
        if moved != press.moved {
            self.press = Some(Press { moved, ..press });
        }
        if !moved {
            return vec![];
        }
        let mut out = Vec::new();
        if self.drag.is_none() {
            let Some(bones) = cx.bones else { return vec![] };
            let Some(j) = press.joint else { return vec![] };
            let Some((drag, label)) = Self::start(bones, j, press.at, ev.mods.cmd) else { return vec![] };
            self.drag = Some(drag);
            out.push(Action::Begin(label.into()));
        }
        if let Some(action) = self.drag.and_then(|d| d.preview(ev.pos)) {
            out.push(action);
        }
        out
    }

    fn up(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        let press = self.press.take();
        if self.drag.take().is_some() {
            return vec![Action::Commit];
        }
        let Some(press) = press else { return vec![] };
        if press.joint.is_some() || press.moved {
            return vec![];
        }
        let mut args = json!({ "x": press.at.x, "y": press.at.y });
        // The remembered joint may belong to another layer's skeleton by now.
        let count = cx.bones.map_or(0, |b| b.at.len());
        if let Some(p) = self.last.filter(|p| !ev.mods.alt && *p < count) {
            args["parent"] = json!(p);
        }
        self.last = Some(count);
        vec![Action::Exec("rig.addJoint".into(), args)]
    }
}

impl Tool for BoneTool {
    fn id(&self) -> &'static str {
        "bone"
    }

    fn busy(&self) -> bool {
        self.drag.is_some()
    }

    fn pointer(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        if !ev.pos.x.is_finite() || !ev.pos.y.is_finite() {
            return vec![];
        }
        match ev.kind {
            PointerKind::Down => {
                let joint = cx.bones.and_then(|b| joint_at(b, ev.pos, cx.tol(6.0)));
                if let Some(j) = joint {
                    self.last = Some(j);
                }
                self.press = Some(Press { at: ev.pos, joint, moved: false });
                vec![]
            }
            PointerKind::Drag => self.drag(cx, ev),
            PointerKind::Up => self.up(cx, ev),
            _ => vec![],
        }
    }

    fn cursor(&self, _cx: &ToolContext, _p: Point, _m: Mods) -> Cursor {
        Cursor::Crosshair
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use vectorcraft_doc::Selection;

    /// Joints at (0,0) root and (100,0) hanging from it, at rest.
    fn arm() -> Bones {
        Bones { row: 1, frame: 1, at: vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0)], parents: vec![None, Some(0)], angles: vec![0.0, 0.0] }
    }

    #[test]
    fn click_adds_joints_with_parent_chain() {
        let (d, _) = doc_with_rect();
        let s = Selection::default();
        let p = paint();
        let cx = cx(&d, &s, &p);
        let mut t = BoneTool::default();
        let click = |t: &mut BoneTool, cx: &ToolContext, x: f64, y: f64, alt: bool| {
            let m = Mods { alt, ..Default::default() };
            t.pointer(cx, &PointerEvent::new(PointerKind::Down, x, y).with_mods(m));
            t.pointer(cx, &PointerEvent::new(PointerKind::Up, x, y).with_mods(m))
        };
        assert_eq!(click(&mut t, &cx, 10.0, 10.0, false), vec![Action::Exec("rig.addJoint".into(), json!({ "x": 10.0, "y": 10.0 }))]);
        let mut b = arm();
        let cx2 = ToolContext { bones: Some(&b), ..cx };
        assert_eq!(click(&mut t, &cx2, 50.0, 0.0, false), vec![Action::Exec("rig.addJoint".into(), json!({ "x": 50.0, "y": 0.0, "parent": 0 }))]);
        b.at.push(Point::new(50.0, 0.0));
        let cx = ToolContext { bones: Some(&b), ..cx };
        assert_eq!(click(&mut t, &cx, 90.0, 0.0, true), vec![Action::Exec("rig.addJoint".into(), json!({ "x": 90.0, "y": 0.0 }))], "Alt adds a root");
    }

    #[test]
    fn dragging_child_around_root_poses_ninety_degrees() {
        let (d, _) = doc_with_rect();
        let s = Selection::default();
        let p = paint();
        let b = arm();
        let cx = ToolContext { bones: Some(&b), ..cx(&d, &s, &p) };
        let mut t = BoneTool::default();
        assert!(t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 100.0, 0.0)).is_empty());
        let acts = t.pointer(&cx, &PointerEvent::new(PointerKind::Drag, 0.0, 100.0));
        assert_eq!(acts.len(), 2, "{acts:?}");
        assert_eq!(acts[0], Action::Begin("Pose Joint".into()));
        let Action::Preview(cmd, args) = &acts[1] else { panic!("expected a preview: {acts:?}") };
        assert_eq!(cmd, "rig.pose");
        assert_eq!(args["joint"], json!(0));
        assert_eq!(args["frame"], json!(1));
        let angle = args["angle"].as_f64().unwrap();
        assert!((angle - 90.0).abs() < 1e-9, "angle was {angle}");
        assert_eq!(t.pointer(&cx, &PointerEvent::new(PointerKind::Up, 0.0, 100.0)), vec![Action::Commit]);
    }

    #[test]
    fn dragging_root_previews_move() {
        let (d, _) = doc_with_rect();
        let s = Selection::default();
        let p = paint();
        let b = arm();
        let cx = ToolContext { bones: Some(&b), ..cx(&d, &s, &p) };
        let mut t = BoneTool::default();
        t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 0.0, 0.0));
        let acts = t.pointer(&cx, &PointerEvent::new(PointerKind::Drag, 50.0, 50.0));
        assert_eq!(
            acts,
            vec![Action::Begin("Move Joint".into()), Action::Preview("rig.moveJoint".into(), json!({ "index": 0, "x": 50.0, "y": 50.0 })),]
        );
        assert_eq!(t.pointer(&cx, &PointerEvent::new(PointerKind::Up, 50.0, 50.0)), vec![Action::Commit]);
    }

    #[test]
    fn non_finite_pointer_yields_no_actions() {
        let (d, _) = doc_with_rect();
        let s = Selection::default();
        let p = paint();
        let b = arm();
        let cx = ToolContext { bones: Some(&b), ..cx(&d, &s, &p) };
        let mut t = BoneTool::default();
        for kind in [PointerKind::Down, PointerKind::Drag, PointerKind::Up] {
            assert!(t.pointer(&cx, &PointerEvent::new(kind, f64::NAN, 0.0)).is_empty());
            assert!(t.pointer(&cx, &PointerEvent::new(kind, 0.0, f64::INFINITY)).is_empty());
        }
    }

    #[test]
    fn cmd_dragging_a_joint_reaches_for_the_pointer() {
        let (d, _) = doc_with_rect();
        let s = Selection::default();
        let p = paint();
        let b = arm();
        let cx = ToolContext { bones: Some(&b), ..cx(&d, &s, &p) };
        let mut t = BoneTool::default();
        let cmd = Mods { cmd: true, ..Default::default() };
        t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 100.0, 0.0).with_mods(cmd));
        let acts = t.pointer(&cx, &PointerEvent::new(PointerKind::Drag, 60.0, 60.0).with_mods(cmd));
        assert_eq!(
            acts,
            vec![Action::Begin("Reach".into()), Action::Preview("rig.reach".into(), json!({ "frame": 1, "joint": 1, "x": 60.0, "y": 60.0 }))]
        );
    }
}
