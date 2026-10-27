//! The Curve Deform tool: drag an anchor or a handle of a curve deformer to key it at the current
//! frame. Anchors win over handles when both are within reach; a press on nothing does nothing.

use serde_json::json;
use vectorcraft_geom::Point;

use crate::{Action, Cursor, CurvePoints, Mods, PointerEvent, PointerKind, Tool, ToolContext};

#[derive(Default)]
pub struct CurveDeformTool {
    /// The gesture since the last press.
    press: Option<Press>,
    /// The point drag in progress, begun on the first real movement.
    drag: Option<Drag>,
}

#[derive(Clone, Copy)]
struct Press {
    at: Point,
    target: Target,
    moved: bool,
}

#[derive(Clone, Copy)]
enum Target {
    /// An anchor (by index), or one of its handles ("in" or "out").
    Point { anchor: usize, handle: Option<&'static str> },
    /// Nothing of the curve is within reach.
    Nothing,
}

/// A point drag: the curve and frame are fixed when the drag begins.
#[derive(Clone, Copy)]
struct Drag {
    node: u64,
    frame: u32,
    anchor: usize,
    handle: Option<&'static str>,
}

/// The anchor within `tol` of `p` if any (anchors win over handles), else the nearest handle.
fn target_at(c: &CurvePoints, p: Point, tol: f64) -> Target {
    let len = c.points.len();
    let anchors = if c.closed { len / 3 } else { len.saturating_sub(1) / 3 + 1 };
    let dist = |i: usize| c.points.get(i).map(|&q| (q - p).hypot()).filter(|d| d.is_finite() && *d <= tol);
    let nearest = |items: Vec<(Target, usize)>| {
        items.into_iter().filter_map(|(t, i)| dist(i).map(|d| (t, d))).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(t, _)| t)
    };
    let anchor = (0..anchors).map(|a| (Target::Point { anchor: a, handle: None }, 3 * a));
    if let Some(t) = nearest(anchor.collect()) {
        return t;
    }
    let handles = (0..anchors).flat_map(|a| {
        let at = 3 * a;
        // An open chain's first anchor has no in handle, its last no out handle.
        let (ins, outs) = if c.closed {
            ((at + len.saturating_sub(1)).checked_rem(len), (at + 1).checked_rem(len))
        } else {
            (at.checked_sub(1), Some(at + 1).filter(|k| *k < len))
        };
        [(ins, "in"), (outs, "out")].into_iter().filter_map(move |(i, h)| i.map(|i| (Target::Point { anchor: a, handle: Some(h) }, i)))
    });
    nearest(handles.collect()).unwrap_or(Target::Nothing)
}

impl CurveDeformTool {
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
            let (Target::Point { anchor, handle }, Some(c)) = (press.target, cx.curve) else { return vec![] };
            self.drag = Some(Drag { node: c.node, frame: c.frame, anchor, handle });
            out.push(Action::Begin("Move Curve Point".into()));
        }
        if let Some(d) = self.drag {
            let mut args = json!({ "node": d.node, "frame": d.frame, "anchor": d.anchor, "x": ev.pos.x, "y": ev.pos.y });
            if let Some(h) = d.handle {
                args["handle"] = json!(h);
            }
            out.push(Action::Preview("curve.key".into(), args));
        }
        out
    }
}

impl Tool for CurveDeformTool {
    fn id(&self) -> &'static str {
        "curveDeform"
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
                let target = cx.curve.map_or(Target::Nothing, |c| target_at(c, ev.pos, cx.tol(6.0)));
                self.press = Some(Press { at: ev.pos, target, moved: false });
                vec![]
            }
            PointerKind::Drag => self.drag(cx, ev),
            PointerKind::Up => {
                self.press = None;
                if self.drag.take().is_some() { vec![Action::Commit] } else { vec![] }
            }
            _ => vec![],
        }
    }

    fn cursor(&self, _cx: &ToolContext, _p: Point, _m: Mods) -> Cursor {
        Cursor::CurveDeform
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use vectorcraft_doc::Selection;

    /// An open curve of three anchors at x = 0, 50, 100 (layout: anchor, out, in, anchor, …): the
    /// out handle of the middle anchor sits 10 pt away at (60, 0), clear of every anchor.
    fn curve(node: u64, frame: u32) -> CurvePoints {
        let xs = [0.0, 10.0, 40.0, 50.0, 60.0, 90.0, 100.0];
        CurvePoints { node, frame, points: xs.iter().map(|&x| Point::new(x, 0.0)).collect(), closed: false }
    }

    #[test]
    fn an_anchor_beats_a_handle_that_is_just_as_near() {
        let (d, id) = doc_with_rect();
        let (s, p) = (Selection::default(), paint());
        // The out handle of anchor 0 sits 3 pt away; the press is 2 pt from the anchor, 1 pt from the handle.
        let c = CurvePoints {
            node: id.0,
            frame: 0,
            points: vec![Point::new(0.0, 0.0), Point::new(3.0, 0.0), Point::new(40.0, 0.0), Point::new(50.0, 0.0)],
            closed: false,
        };
        let cx = ToolContext { curve: Some(&c), ..cx(&d, &s, &p) };
        let mut t = CurveDeformTool::default();
        t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 2.0, 0.0));
        let acts = t.pointer(&cx, &PointerEvent::new(PointerKind::Drag, 20.0, 0.0));
        assert!(matches!(&acts[1], Action::Preview(cmd, args) if cmd == "curve.key" && args["anchor"] == json!(0) && args.get("handle").is_none()));
    }

    #[test]
    fn a_press_on_a_handle_picks_that_handle_and_nothing_else() {
        let (d, id) = doc_with_rect();
        let (s, p) = (Selection::default(), paint());
        let c = curve(id.0, 0);
        let cx = ToolContext { curve: Some(&c), ..cx(&d, &s, &p) };
        let mut t = CurveDeformTool::default();
        // (60, 0) is the middle anchor's out handle; the anchor itself is 10 pt away.
        t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 60.0, 0.0));
        let acts = t.pointer(&cx, &PointerEvent::new(PointerKind::Drag, 70.0, 5.0));
        assert_eq!(
            acts,
            vec![
                Action::Begin("Move Curve Point".into()),
                Action::Preview("curve.key".into(), json!({ "node": id.0, "frame": 0, "anchor": 1, "handle": "out", "x": 70.0, "y": 5.0 })),
            ]
        );
        assert!(t.busy());
        assert_eq!(t.pointer(&cx, &PointerEvent::new(PointerKind::Up, 70.0, 5.0)), vec![Action::Commit]);
        assert!(!t.busy());
    }

    #[test]
    fn dragging_an_anchor_keys_it_without_a_handle() {
        let (d, id) = doc_with_rect();
        let (s, p) = (Selection::default(), paint());
        let c = curve(id.0, 0);
        let cx = ToolContext { curve: Some(&c), ..cx(&d, &s, &p) };
        let mut t = CurveDeformTool::default();
        t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 49.0, 1.0));
        let acts = t.pointer(&cx, &PointerEvent::new(PointerKind::Drag, 55.0, 8.0));
        assert_eq!(acts[1], Action::Preview("curve.key".into(), json!({ "node": id.0, "frame": 0, "anchor": 1, "x": 55.0, "y": 8.0 })));
    }

    #[test]
    fn a_drag_keys_the_frame_it_began_on() {
        let (d, id) = doc_with_rect();
        let (s, p) = (Selection::default(), paint());
        let c = curve(id.0, 12);
        let cx = ToolContext { curve: Some(&c), ..cx(&d, &s, &p) };
        let mut t = CurveDeformTool::default();
        t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 60.0, 0.0));
        let acts = t.pointer(&cx, &PointerEvent::new(PointerKind::Drag, 65.0, 0.0));
        assert!(matches!(&acts[1], Action::Preview(cmd, args) if cmd == "curve.key" && args["frame"] == json!(12)));
    }

    #[test]
    fn a_press_on_nothing_does_nothing() {
        let (d, id) = doc_with_rect();
        let (s, p) = (Selection::default(), paint());
        let c = curve(id.0, 0);
        let cx = ToolContext { curve: Some(&c), ..cx(&d, &s, &p) };
        let mut t = CurveDeformTool::default();
        t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 30.0, 30.0));
        assert!(t.pointer(&cx, &PointerEvent::new(PointerKind::Drag, 50.0, 50.0)).is_empty());
        assert!(t.pointer(&cx, &PointerEvent::new(PointerKind::Up, 50.0, 50.0)).is_empty());
        assert!(!t.busy());
    }

    #[test]
    fn without_a_curve_nothing_is_hit() {
        let (d, _) = doc_with_rect();
        let (s, p) = (Selection::default(), paint());
        let cx = cx(&d, &s, &p);
        let mut t = CurveDeformTool::default();
        t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 0.0, 0.0));
        assert!(t.pointer(&cx, &PointerEvent::new(PointerKind::Drag, 9.0, 9.0)).is_empty());
    }

    #[test]
    fn the_cursor_is_curve_deform() {
        let (d, id) = doc_with_rect();
        let (s, p) = (Selection::default(), paint());
        let c = curve(id.0, 0);
        let cx = ToolContext { curve: Some(&c), ..cx(&d, &s, &p) };
        assert_eq!(CurveDeformTool::default().cursor(&cx, Point::new(60.0, 0.0), Mods::default()), Cursor::CurveDeform);
    }

    #[test]
    fn non_finite_pointer_yields_no_actions() {
        let (d, id) = doc_with_rect();
        let (s, p) = (Selection::default(), paint());
        let c = curve(id.0, 0);
        let cx = ToolContext { curve: Some(&c), ..cx(&d, &s, &p) };
        let mut t = CurveDeformTool::default();
        for kind in [PointerKind::Down, PointerKind::Drag, PointerKind::Up] {
            assert!(t.pointer(&cx, &PointerEvent::new(kind, f64::NAN, 0.0)).is_empty());
            assert!(t.pointer(&cx, &PointerEvent::new(kind, 0.0, f64::INFINITY)).is_empty());
        }
    }
}
