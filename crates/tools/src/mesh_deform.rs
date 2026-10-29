//! The Mesh Deform tool: drag a pin to key it at the current frame, click an unpinned vertex to
//! pin it, click art beside the mesh to mesh that object instead.

use serde_json::json;
use vectorcraft_geom::Point;

use crate::{Action, Cursor, MeshPoints, Mods, PointerEvent, PointerKind, Tool, ToolContext};

#[derive(Default)]
pub struct MeshDeformTool {
    /// The gesture since the last press.
    press: Option<Press>,
    /// The pin drag in progress, begun on the first real movement.
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
    /// A pin, by its index in `pins`.
    Pin(usize),
    /// An unpinned vertex, by its vertex index.
    Vertex(usize),
    /// Nothing of the mesh is within reach: the art under the press decides.
    Nothing,
}

/// A pin drag: the mesh and frame are fixed when the drag begins.
#[derive(Clone, Copy)]
struct Drag {
    node: u64,
    frame: u32,
    pin: usize,
}

/// The pin within `tol` of `p` if any (pins win over vertices), else the nearest unpinned vertex.
fn target_at(m: &MeshPoints, p: Point, tol: f64) -> Target {
    let dist = |vi: usize| m.points.get(vi).map(|&a| (a - p).hypot()).filter(|d| d.is_finite() && *d <= tol);
    let pin = m.pins.iter().enumerate().filter_map(|(k, &vi)| dist(vi).map(|d| (k, d))).min_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((k, _)) = pin {
        return Target::Pin(k);
    }
    let vertex = (0..m.points.len()).filter(|vi| !m.pins.contains(vi)).filter_map(|vi| dist(vi).map(|d| (vi, d))).min_by(|a, b| a.1.total_cmp(&b.1));
    match vertex {
        Some((vi, _)) => Target::Vertex(vi),
        None => Target::Nothing,
    }
}

impl MeshDeformTool {
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
            let (Target::Pin(pin), Some(m)) = (press.target, cx.mesh) else { return vec![] };
            self.drag = Some(Drag { node: m.node, frame: m.frame, pin });
            out.push(Action::Begin("Move Pin".into()));
        }
        if let Some(d) = self.drag {
            out.push(Action::Preview("mesh.key".into(), json!({ "node": d.node, "frame": d.frame, "pin": d.pin, "x": ev.pos.x, "y": ev.pos.y })));
        }
        out
    }

    fn up(&mut self, cx: &ToolContext, _ev: &PointerEvent) -> Vec<Action> {
        let press = self.press.take();
        if self.drag.take().is_some() {
            return vec![Action::Commit];
        }
        let Some(press) = press else { return vec![] };
        if press.moved {
            return vec![];
        }
        match press.target {
            Target::Pin(_) => vec![],
            Target::Vertex(vertex) => match cx.mesh {
                Some(m) => vec![Action::Exec("mesh.pin".into(), json!({ "node": m.node, "vertex": vertex }))],
                None => vec![],
            },
            Target::Nothing => {
                // The object the selection tool would pick under the click, unless it is the mesh.
                let Some(h) = vectorcraft_doc::hit::hit_test(cx.doc, press.at, cx.selection_hit_options()) else { return vec![] };
                let top = h.top_object(cx.isolation).0;
                if cx.mesh.map(|m| m.node) == Some(top) {
                    return vec![];
                }
                vec![Action::Exec("mesh.add".into(), json!({ "node": top }))]
            }
        }
    }
}

impl Tool for MeshDeformTool {
    fn id(&self) -> &'static str {
        "meshDeform"
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
                let target = cx.mesh.map_or(Target::Nothing, |m| target_at(m, ev.pos, cx.tol(6.0)));
                self.press = Some(Press { at: ev.pos, target, moved: false });
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

    /// A 2 × 2 grid of vertices at the corners of a 100-point square, pins on vertices 0 and 3, at
    /// frame 2, over the node `node`.
    fn mesh(node: u64) -> MeshPoints {
        MeshPoints {
            node,
            frame: 2,
            points: vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0), Point::new(0.0, 100.0), Point::new(100.0, 100.0)],
            tris: vec![[0, 1, 2], [1, 3, 2]],
            pins: vec![0, 3],
        }
    }

    #[test]
    fn dragging_a_pin_keys_it_and_commits() {
        let (d, id) = doc_with_rect();
        let (s, p) = (Selection::default(), paint());
        let m = mesh(id.0);
        let cx = ToolContext { mesh: Some(&m), ..cx(&d, &s, &p) };
        let mut t = MeshDeformTool::default();
        assert!(t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 101.0, 99.0)).is_empty());
        let acts = t.pointer(&cx, &PointerEvent::new(PointerKind::Drag, 120.0, 130.0));
        assert_eq!(
            acts,
            vec![
                Action::Begin("Move Pin".into()),
                Action::Preview("mesh.key".into(), json!({ "node": id.0, "frame": 2, "pin": 1, "x": 120.0, "y": 130.0 })),
            ]
        );
        assert!(t.busy());
        let acts = t.pointer(&cx, &PointerEvent::new(PointerKind::Drag, 130.0, 140.0));
        assert_eq!(acts, vec![Action::Preview("mesh.key".into(), json!({ "node": id.0, "frame": 2, "pin": 1, "x": 130.0, "y": 140.0 }))]);
        assert_eq!(t.pointer(&cx, &PointerEvent::new(PointerKind::Up, 130.0, 140.0)), vec![Action::Commit]);
        assert!(!t.busy());
    }

    #[test]
    fn pins_beat_vertices_when_both_are_near() {
        let (d, id) = doc_with_rect();
        let (s, p) = (Selection::default(), paint());
        let m = MeshPoints { node: id.0, frame: 1, points: vec![Point::new(0.0, 0.0), Point::new(3.0, 0.0)], tris: vec![], pins: vec![0] };
        let cx = ToolContext { mesh: Some(&m), ..cx(&d, &s, &p) };
        let mut t = MeshDeformTool::default();
        // Nearer the unpinned vertex (1 away) than the pin (2 away), yet the pin is taken.
        t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 2.0, 0.0));
        let acts = t.pointer(&cx, &PointerEvent::new(PointerKind::Drag, 20.0, 0.0));
        assert_eq!(acts[0], Action::Begin("Move Pin".into()));
        assert!(matches!(&acts[1], Action::Preview(cmd, args) if cmd == "mesh.key" && args["pin"] == json!(0)));
    }

    #[test]
    fn clicking_an_unpinned_vertex_pins_it() {
        let (d, id) = doc_with_rect();
        let (s, p) = (Selection::default(), paint());
        let m = mesh(id.0);
        let cx = ToolContext { mesh: Some(&m), ..cx(&d, &s, &p) };
        let mut t = MeshDeformTool::default();
        t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 100.0, 1.0));
        assert_eq!(
            t.pointer(&cx, &PointerEvent::new(PointerKind::Up, 100.0, 1.0)),
            vec![Action::Exec("mesh.pin".into(), json!({ "node": id.0, "vertex": 1 }))]
        );
    }

    #[test]
    fn clicking_a_pin_does_nothing() {
        let (d, id) = doc_with_rect();
        let (s, p) = (Selection::default(), paint());
        let m = mesh(id.0);
        let cx = ToolContext { mesh: Some(&m), ..cx(&d, &s, &p) };
        let mut t = MeshDeformTool::default();
        t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 0.0, 0.0));
        assert!(t.pointer(&cx, &PointerEvent::new(PointerKind::Up, 0.0, 0.0)).is_empty());
    }

    #[test]
    fn clicking_art_without_a_mesh_meshes_it() {
        let (d, id) = doc_with_rect();
        let (s, p) = (Selection::default(), paint());
        let cx = cx(&d, &s, &p);
        let mut t = MeshDeformTool::default();
        t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 150.0, 150.0));
        assert_eq!(t.pointer(&cx, &PointerEvent::new(PointerKind::Up, 150.0, 150.0)), vec![Action::Exec("mesh.add".into(), json!({ "node": id.0 }))]);
    }

    #[test]
    fn clicking_the_meshed_art_adds_nothing() {
        let (d, id) = doc_with_rect();
        let (s, p) = (Selection::default(), paint());
        let m = mesh(id.0);
        let cx = ToolContext { mesh: Some(&m), ..cx(&d, &s, &p) };
        let mut t = MeshDeformTool::default();
        t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 150.0, 150.0));
        assert!(t.pointer(&cx, &PointerEvent::new(PointerKind::Up, 150.0, 150.0)).is_empty());
    }

    #[test]
    fn non_finite_pointer_yields_no_actions() {
        let (d, id) = doc_with_rect();
        let (s, p) = (Selection::default(), paint());
        let m = mesh(id.0);
        let cx = ToolContext { mesh: Some(&m), ..cx(&d, &s, &p) };
        let mut t = MeshDeformTool::default();
        for kind in [PointerKind::Down, PointerKind::Drag, PointerKind::Up] {
            assert!(t.pointer(&cx, &PointerEvent::new(kind, f64::NAN, 0.0)).is_empty());
            assert!(t.pointer(&cx, &PointerEvent::new(kind, 0.0, f64::INFINITY)).is_empty());
        }
    }
}
