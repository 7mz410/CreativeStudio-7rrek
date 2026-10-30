//! 7rrek (Creative Studio): the mesh deformer's mesh drawn over the canvas while the Mesh Deform
//! tool is active.

use std::collections::BTreeSet;

use egui::{Pos2, Stroke};
use vectorcraft_geom::Point;
use vectorcraft_tools::MeshPoints;

use crate::VectorcraftApp;
use crate::canvas::Xf;
use crate::theme::Tokens;

/// The mesh the tool shows: the selected object's, or the only mesh. None off the Mesh Deform tool.
fn shown(app: &VectorcraftApp) -> Option<MeshPoints> {
    if app.session.tool_id() != "meshDeform" {
        return None;
    }
    let st = app.session.active()?;
    vectorcraft_engine::cmd::mesh::points(&st.doc, &st.selection.objects)
}

/// Each triangle edge once, as a (low, high) vertex index pair. Degenerate edges are dropped.
fn edges(tris: &[[u32; 3]]) -> BTreeSet<(usize, usize)> {
    tris.iter()
        .flat_map(|t| [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])])
        .map(|(a, b)| (a.min(b) as usize, a.max(b) as usize))
        .filter(|(a, b)| a != b)
        .collect()
}

/// Each vertex's screen spot, None where it is not finite.
fn screen_vertices(points: &[Point], xf: &Xf) -> Vec<Option<Pos2>> {
    points.iter().map(|p| Some(xf.to_screen(*p)).filter(|s| s.is_finite())).collect()
}

/// Draw the mesh: edges as thin lines, unpinned vertices as dots, pins as larger filled circles.
/// Nothing is drawn off the Mesh Deform tool or without a mesh.
// shortcut: draws every edge of every triangle, no culling; fine for meshes up to the engine's caps
pub fn paint(app: &VectorcraftApp, p: &egui::Painter, xf: &Xf) {
    let Some(m) = shown(app) else { return };
    let t = Tokens::get(p.ctx());
    let at = screen_vertices(&m.points, xf);
    for (a, b) in edges(&m.tris) {
        if let (Some(Some(a)), Some(Some(b))) = (at.get(a), at.get(b)) {
            p.line_segment([*a, *b], Stroke::new(1.0, t.text_dim));
        }
    }
    let pinned: BTreeSet<usize> = m.pins.iter().copied().collect();
    for spot in at.iter().enumerate().filter(|(i, _)| !pinned.contains(i)).filter_map(|(_, s)| *s) {
        p.circle_filled(spot, 2.5, t.selection);
    }
    for spot in pinned.iter().filter_map(|&i| at.get(i).copied().flatten()) {
        p.circle_filled(spot, 5.0, t.selection);
        p.circle_stroke(spot, 5.0, Stroke::new(1.5, t.measure_bg));
    }
}

#[cfg(test)]
mod tests {
    use egui::{Rect, vec2};
    use serde_json::json;
    use vectorcraft_engine::Session;

    use super::*;

    fn xf() -> Xf {
        Xf { rect: Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)), zoom: 1.0, center: Point::new(0.0, 0.0), rot: 0.0 }
    }

    /// A 200 × 20 rectangle with a 2 × 1 mesh (six vertices, four triangles) and vertex 0 pinned.
    fn app_with_mesh() -> VectorcraftApp {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 400, "height": 300})).unwrap();
        app.session.execute("shape.rectangle", &json!({"x": 0, "y": -10, "width": 200, "height": 20})).unwrap();
        app.session.execute("mesh.add", &json!({"cols": 2, "rows": 1})).unwrap();
        app.session.execute("mesh.pin", &json!({"vertex": 0})).unwrap();
        app
    }

    /// One headless frame of the painter, with the overlay drawn into it.
    fn paint_once(app: &VectorcraftApp) {
        let ctx = egui::Context::default();
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))), ..Default::default() };
        let x = xf();
        ctx.run_ui(raw, |ui| paint(app, ui.painter(), &x)).textures_delta.clear();
    }

    #[test]
    fn shared_edges_are_listed_once() {
        // Two triangles sharing the edge 1–2: five distinct edges, the shared one once.
        let got = edges(&[[0, 1, 2], [1, 3, 2]]);
        assert_eq!(got.len(), 5, "{got:?}");
        assert!(got.contains(&(1, 2)));
        assert!(edges(&[[4, 4, 4]]).is_empty(), "degenerate triangle has no edges");
    }

    #[test]
    fn a_non_finite_vertex_has_no_screen_spot() {
        let spots = screen_vertices(&[Point::new(f64::NAN, 0.0), Point::new(10.0, 10.0)], &xf());
        assert_eq!(spots[0], None);
        assert!(spots[1].is_some());
    }

    #[test]
    fn paint_draws_the_mesh_only_with_the_mesh_deform_tool() {
        let mut app = app_with_mesh();
        paint_once(&app);
        app.run("tool.select", json!({"tool": "meshDeform"})).unwrap();
        assert!(shown(&app).is_some());
        paint_once(&app);
        app.run("tool.select", json!({"tool": "rectangle"})).unwrap();
        assert!(shown(&app).is_none());
        paint_once(&app);
    }
}
