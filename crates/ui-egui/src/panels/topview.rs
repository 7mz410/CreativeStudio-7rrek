//! Top View panel: the scene's rows seen from above, as in Harmony's multiplane. The camera sits at
//! the bottom, the stage (depth 0) a lens above it, and each row is a segment at its depth, so a
//! row further back sits higher and looks narrower. Below the diagram each row's own depth is
//! a field value to drag, and its Maintain Size keeps it its size as it moves back (`anim.depth`).

use egui::{Align2, DragValue, FontId, Sense, Shape, Stroke, Ui, pos2, vec2};
use serde_json::{Value, json};
use vectorcraft_engine::cmd::depth::{FAR, FIELD, LENS, NEAR};

use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

/// The panel id, as the dock, the Window menu and `window.panel` name it.
pub const ID: &str = "topview";

/// The diagram's height in points.
const DIAGRAM_H: f32 = 180.0;

/// A top-level layer and its row's depth, as `anim.depths` reports it.
struct Row {
    row: u64,
    name: String,
    own: f64,
    z: f64,
    maintain: bool,
}

/// Runs a command; a failure goes to the status bar.
fn run(app: &mut VectorcraftApp, id: &str, params: Value) {
    if let Err(e) = app.run(id, params) {
        app.status(e);
    }
}

/// Sets row `row`'s own depth to `fields` fields (`anim.depth`); the depth its pegs add stays.
pub(crate) fn set_fields(app: &mut VectorcraftApp, row: u64, fields: f64) -> Result<Value, String> {
    app.run("anim.depth", json!({"row": row, "z": fields * FIELD}))
}

/// The rows in layer order, with the current layer's row.
fn rows_of(app: &mut VectorcraftApp) -> (Vec<Row>, Option<u64>) {
    let names: Vec<(u64, String)> = match app.session.active() {
        Some(st) => st.doc.layers.iter().map(|l| (l.id.0, l.display_name())).collect(),
        None => return (Vec::new(), None),
    };
    let depths = app.session.execute("anim.depths", &json!({})).unwrap_or(Value::Null);
    let entries = depths["rows"].as_array().cloned().unwrap_or_default();
    let rows = names
        .into_iter()
        .filter_map(|(row, name)| {
            let e = entries.iter().find(|e| e["row"].as_u64() == Some(row))?;
            Some(Row {
                row,
                name,
                own: e["own"].as_f64().unwrap_or(0.0),
                z: e["z"].as_f64().unwrap_or(0.0),
                maintain: e["maintain"].as_bool().unwrap_or(false),
            })
        })
        .collect();
    let cur = app.session.execute("anim.drawings", &json!({})).ok().and_then(|v| v["row"].as_u64());
    (rows, cur)
}

/// The diagram: the camera at the bottom, the stage a lens above it, each row at its depth.
fn diagram(ui: &mut Ui, rows: &[Row], cur: Option<u64>) {
    let t = Tokens::get(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), DIAGRAM_H), Sense::hover());
    let painter = ui.painter_at(rect);
    let cx = rect.center().x;
    let cam_y = rect.bottom() - 14.0;
    let top = rect.top() + 14.0;
    // Distance from the camera: the stage is LENS away and each row LENS + its depth. The range
    // runs from the camera to the farthest row, with margins.
    let lens = LENS as f32;
    let far = rows.iter().map(|r| (LENS + r.z) as f32).fold(lens, f32::max) * 1.1;
    let k = (cam_y - top) / far;
    let y_of = |d: f32| cam_y - d * k;
    let stage_y = y_of(lens);
    let half = rect.width() * 0.25;
    // Seen from above a row keeps its real width (the view cone widening behind it is what makes
    // it look smaller to the camera); a row that keeps its size is drawn bigger by its depth.
    let width_at = |d: f32, maintain: bool| (half * 2.0 * if maintain { d / lens } else { 1.0 }).min(rect.width() * 0.9);

    // The view cone: lines from the camera through the stage's ends.
    let spread = half * (cam_y - top) / (cam_y - stage_y);
    for s in [-1.0_f32, 1.0] {
        painter.line_segment([pos2(cx, cam_y), pos2(cx + s * spread, top)], Stroke::new(1.0, t.divider));
    }
    painter.line_segment([pos2(cx - half, stage_y), pos2(cx + half, stage_y)], Stroke::new(1.5, t.text));
    // On the left, so a row at the stage can name itself on the right.
    painter.text(pos2(cx - half - 6.0, stage_y), Align2::RIGHT_CENTER, tl!("Stage"), FontId::proportional(11.0), t.text_dim);

    for r in rows {
        let d = (LENS + r.z) as f32;
        let y = y_of(d);
        let w = width_at(d, r.maintain);
        let current = Some(r.row) == cur;
        let colour = if current { t.accent } else { t.text_dim };
        let width = if current { 2.5 } else { 1.5 };
        painter.line_segment([pos2(cx - w / 2.0, y), pos2(cx + w / 2.0, y)], Stroke::new(width, colour));
        // Past the segment's end, else (a segment as wide as the panel) over its right end.
        let (x, align) =
            if cx + w / 2.0 + 60.0 < rect.right() { (cx + w / 2.0 + 6.0, Align2::LEFT_CENTER) } else { (rect.right() - 4.0, Align2::RIGHT_BOTTOM) };
        painter.text(pos2(x, y), align, &r.name, FontId::proportional(11.0), colour);
    }

    let camera = vec![pos2(cx - 7.0, cam_y + 4.0), pos2(cx + 7.0, cam_y + 4.0), pos2(cx, cam_y - 8.0)];
    painter.add(Shape::convex_polygon(camera, t.icon, Stroke::NONE));
    painter.text(pos2(cx + 12.0, cam_y), Align2::LEFT_CENTER, tl!("Camera"), FontId::proportional(11.0), t.text_dim);
}

/// A number of fields as the note shows it: whole numbers bare, others to a tenth.
fn fields_text(n: f64) -> String {
    if (n - n.round()).abs() < 1e-6 { format!("{}", n.round() as i64) } else { format!("{n:.1}") }
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    if app.session.active().is_none() {
        widgets::dim_label(ui, tl!("No document"));
        return;
    }
    let (rows, cur) = rows_of(app);
    let t = Tokens::get(ui.ctx());
    // Edits found while drawing, run once the panel is drawn.
    let mut depth_sets: Vec<(u64, f64)> = Vec::new();
    let mut maintain_sets: Vec<(u64, bool)> = Vec::new();

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        diagram(ui, &rows, cur);
        ui.add_space(8.0);
        widgets::divider(ui);
        for r in &rows {
            let pegs = r.z - r.own;
            // The own depth's range keeps LENS + z inside the camera's view, whatever the pegs add.
            let lo = (NEAR - LENS - pegs) / FIELD;
            let hi = (FAR - LENS - pegs) / FIELD;
            ui.horizontal(|ui| {
                if Some(r.row) == cur {
                    ui.label(egui::RichText::new(&r.name).color(t.accent));
                } else {
                    ui.label(&r.name);
                }
                let mut fields = r.own / FIELD;
                if ui.add(DragValue::new(&mut fields).speed(0.1).range(lo..=hi)).changed() {
                    depth_sets.push((r.row, fields));
                }
                widgets::dim_label(ui, tl!("Fields"));
                if pegs.abs() > 1e-9 {
                    let n = fields_text(pegs / FIELD);
                    widgets::dim_label(ui, &crate::i18n::fmt(tl!("(+{n} fields from pegs)"), &[("n", &n)]));
                }
                if widgets::check(ui, "Maintain Size", r.maintain, true) {
                    maintain_sets.push((r.row, !r.maintain));
                }
            });
        }
    });

    for (row, fields) in depth_sets {
        if let Err(e) = set_fields(app, row, fields) {
            app.status(e);
        }
    }
    for (row, maintain) in maintain_sets {
        run(app, "anim.depth", json!({"row": row, "maintain": maintain}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> VectorcraftApp {
        VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default())
    }

    /// One frame of the panel in a 400×300 window.
    fn frame(app: &mut VectorcraftApp, ctx: &egui::Context) {
        let screen = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(400.0, 300.0)));
        let input = egui::RawInput { screen_rect: screen, ..Default::default() };
        ctx.run_ui(input, |ui| show(app, ui)).textures_delta.clear();
    }

    /// Two rows: the first at the stage, the second two fields back. Their ids.
    fn two_rows(app: &mut VectorcraftApp) -> (u64, u64) {
        let s = &mut app.session;
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        let near = s.doc().unwrap().doc.layers[0].id.0;
        let far = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
        s.execute("anim.depth", &json!({"row": far, "z": 2.0 * FIELD})).unwrap();
        (near, far)
    }

    fn z_of(app: &mut VectorcraftApp, row: u64) -> f64 {
        let rows = app.session.execute("anim.depths", &json!({})).unwrap()["rows"].clone();
        rows.as_array().unwrap().iter().find(|r| r["row"].as_u64() == Some(row)).unwrap()["z"].as_f64().unwrap()
    }

    #[test]
    fn two_rows_at_different_depths_render() {
        let mut app = app();
        let (near, far) = two_rows(&mut app);
        let ctx = egui::Context::default();
        frame(&mut app, &ctx);
        assert_eq!(z_of(&mut app, near), 0.0);
        assert_eq!(z_of(&mut app, far), 2.0 * FIELD);
    }

    #[test]
    fn no_document_renders_the_empty_state() {
        let mut app = app();
        let ctx = egui::Context::default();
        frame(&mut app, &ctx);
    }

    #[test]
    fn set_fields_sets_the_own_depth() {
        let mut app = app();
        let (_, far) = two_rows(&mut app);
        set_fields(&mut app, far, 3.0).unwrap();
        assert_eq!(z_of(&mut app, far), 3.0 * FIELD);
    }
}
