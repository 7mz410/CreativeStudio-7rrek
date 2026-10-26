//! 7rrek (Creative Studio): the curve deformer drawn over the canvas while the Curve Deform
//! tool is active.

use egui::{Pos2, Rect, Stroke, StrokeKind, vec2};
use vectorcraft_geom::Point;
use vectorcraft_tools::CurvePoints;

use crate::VectorcraftApp;
use crate::canvas::Xf;
use crate::theme::Tokens;

/// The curve the tool shows: the selected object's, or the only curve deformer. None off the Curve Deform tool.
fn shown(app: &VectorcraftApp) -> Option<CurvePoints> {
    if app.session.tool_id() != "curveDeform" {
        return None;
    }
    let st = app.session.active()?;
    vectorcraft_engine::cmd::curve::points(&st.doc, &st.selection.objects)
}

/// Each point's screen spot, None where it is not finite.
fn screen_points(points: &[Point], xf: &Xf) -> Vec<Option<Pos2>> {
    points.iter().map(|p| Some(xf.to_screen(*p)).filter(|s| s.is_finite())).collect()
}

/// Discretize a cubic Bezier segment between 4 points into polyline screen positions.
fn bezier_segment_points(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2) -> Vec<Pos2> {
    const STEPS: usize = 20;
    (0..=STEPS)
        .map(|i| {
            let t = i as f32 / STEPS as f32;
            let u = 1.0 - t;
            let w0 = u * u * u;
            let w1 = 3.0 * u * u * t;
            let w2 = 3.0 * u * t * t;
            let w3 = t * t * t;
            Pos2::new(w0 * p0.x + w1 * p1.x + w2 * p2.x + w3 * p3.x, w0 * p0.y + w1 * p1.y + w2 * p2.y + w3 * p3.y)
        })
        .collect()
}

/// Draw the curve deformer: Bezier chain as lines, anchors as squares, handles as circles joined
/// to their anchor by lines.
pub fn paint(app: &VectorcraftApp, p: &egui::Painter, xf: &Xf) {
    let Some(c) = shown(app) else { return };
    let t = Tokens::get(p.ctx());
    let at = screen_points(&c.points, xf);
    let len = c.points.len();
    let seg_count = if c.closed {
        if len.is_multiple_of(3) { len / 3 } else { 0 }
    } else if len % 3 == 1 {
        len / 3
    } else {
        0
    };
    if seg_count == 0 {
        return;
    }

    // Draw Bezier chain through anchors
    for seg in 0..seg_count {
        let i0 = 3 * seg;
        let i1 = 3 * seg + 1;
        let i2 = 3 * seg + 2;
        let i3 = (3 * seg + 3) % len;
        if let (Some(Some(p0)), Some(Some(p1)), Some(Some(p2)), Some(Some(p3))) = (at.get(i0), at.get(i1), at.get(i2), at.get(i3)) {
            let poly = bezier_segment_points(*p0, *p1, *p2, *p3);
            for w in poly.windows(2) {
                p.line_segment([w[0], w[1]], Stroke::new(1.0, t.text_dim));
            }
        }
    }

    let anchors = if c.closed { len / 3 } else { len.saturating_sub(1) / 3 + 1 };

    // Draw handle connector lines, handle circles, and anchor squares
    for a in 0..anchors {
        let at_idx = 3 * a;
        // An open chain's first anchor has no in handle, its last no out handle.
        let (ins, outs) =
            if c.closed { (Some((at_idx + len - 1) % len), Some((at_idx + 1) % len)) } else { (at_idx.checked_sub(1), Some(at_idx + 1)) };

        if let Some(Some(anc_pos)) = at.get(at_idx) {
            for h in [outs, ins].into_iter().flatten() {
                if let Some(Some(h_pos)) = at.get(h) {
                    p.line_segment([*anc_pos, *h_pos], Stroke::new(1.0, t.text_dim));
                    p.circle_filled(*h_pos, 3.0, t.selection);
                    p.circle_stroke(*h_pos, 3.0, Stroke::new(1.0, t.measure_bg));
                }
            }

            // Draw anchor as square
            let r = 4.0;
            let square = Rect::from_center_size(*anc_pos, vec2(2.0 * r, 2.0 * r));
            p.rect_filled(square, 0.0, t.selection);
            p.rect_stroke(square, 0.0, Stroke::new(1.0, t.measure_bg), StrokeKind::Outside);
        }
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

    /// A 200 × 20 rectangle with a curve deformer over it.
    fn app_with_curve() -> VectorcraftApp {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 400, "height": 300})).unwrap();
        let r = app.session.execute("shape.rectangle", &json!({"x": 0, "y": -10, "width": 200, "height": 20})).unwrap();
        let node = r.get("id").or_else(|| r.get("node")).and_then(|v| v.as_u64()).unwrap();
        app.session.execute("curve.add", &json!({"node": node})).unwrap();
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
    fn a_non_finite_point_has_no_screen_spot() {
        let spots = screen_points(&[Point::new(f64::NAN, 0.0), Point::new(10.0, 10.0)], &xf());
        assert_eq!(spots[0], None);
        assert!(spots[1].is_some());
    }

    #[test]
    fn paint_draws_the_curve_only_with_the_curve_deform_tool() {
        let mut app = app_with_curve();
        paint_once(&app);
        app.run("tool.select", json!({"tool": "curveDeform"})).unwrap();
        assert!(shown(&app).is_some());
        paint_once(&app);
        app.run("tool.select", json!({"tool": "rectangle"})).unwrap();
        assert!(shown(&app).is_none());
        paint_once(&app);
    }
}
