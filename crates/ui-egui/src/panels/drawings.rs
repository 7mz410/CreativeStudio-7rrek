//! Drawings panel: the current layer row's drawings (its keyframes) and the frames each one
//! shows from. A click exposes the drawing at the current frame on (`anim.expose`), as the
//! exposure sheet does; the drawing shown at the current frame is selected.

use egui::Ui;
use serde_json::{Value, json};

use crate::VectorcraftApp;
use crate::widgets;

/// The panel id, as the dock, the Window menu and `window.panel` name it.
pub const ID: &str = "drawings";

/// Thumbnail side in points.
const THUMB: f32 = 32.0;

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    if app.session.active().is_none() {
        widgets::dim_label(ui, tl!("No document"));
        return;
    }
    // A layer with no row (or no document yet) lists nothing: the empty state covers both.
    let list = app.session.execute("anim.drawings", &json!({})).unwrap_or(Value::Null);
    let drawings = list["drawings"].as_array().cloned().unwrap_or_default();
    if drawings.is_empty() {
        widgets::dim_label(ui, tl!("No drawings: add keyframes to this layer"));
        return;
    }
    let current = list["current"].as_u64();
    let chart = app.session.execute("anim.mouths", &json!({})).map(|c| c["chart"].clone()).unwrap_or(Value::Null);
    let mut clicked = None;
    let mut mouth = None;
    for d in &drawings {
        let id = d["id"].as_u64().unwrap_or_default();
        let frames: Vec<String> = d["frames"].as_array().into_iter().flatten().filter_map(Value::as_u64).map(|f| f.to_string()).collect();
        let shown = !frames.is_empty();
        let frames = crate::i18n::fmt(tl!("frames: {frames}"), &[("frames", &frames.join(", "))]);
        let name = d["name"].as_str().unwrap_or_default().to_string();
        // The mouth shapes this drawing shows in lip sync, if any.
        let shapes: String = chart.as_object().into_iter().flatten().filter(|(_, v)| v.as_u64() == Some(id)).map(|(k, _)| k.as_str()).collect();
        let label = if shapes.is_empty() || shapes == name { name.clone() } else { format!("{name} ({shapes})") };
        // A drawing no frame shows lists no frames.
        let text = if shown { format!("{label} · {frames}") } else { label };
        let resp = ui
            .horizontal(|ui| {
                let (th, _) = ui.allocate_exact_size(egui::vec2(THUMB, THUMB), egui::Sense::hover());
                if let Some(st) = app.session.active() {
                    let a = vectorcraft_doc::ExportAsset { id, name, nodes: vec![vectorcraft_doc::NodeId(id)] };
                    super::asset_export::paint_thumb(ui, st, &a, th);
                }
                ui.selectable_label(current == Some(id), text)
            })
            .inner;
        if resp.clicked() && current != Some(id) {
            clicked = Some(id);
        }
        resp.context_menu(|ui| {
            ui.label(tl!("Mouth Shape"));
            for m in vectorcraft_audio::MOUTHS {
                if ui.button(m.to_string()).clicked() {
                    mouth = Some((m, id));
                    ui.close();
                }
            }
        });
    }
    if let Some(Err(e)) = mouth.map(|(m, id)| app.run("anim.mouthChart", json!({"shape": m.to_string(), "drawing": id}))) {
        app.status(e);
    }
    if let Some(Err(e)) = clicked.map(|id| app.run("anim.expose", json!({"drawing": id}))) {
        app.status(e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_the_layers_two_drawings() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        app.session.execute("anim.blankKeyframe", &json!({"frame": 1})).unwrap();
        app.session.execute("anim.blankKeyframe", &json!({"frame": 2})).unwrap();
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            show(&mut app, ui);
        });
        out.textures_delta.clear();
        let list = app.session.execute("anim.drawings", &json!({})).unwrap();
        assert_eq!(list["drawings"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn a_click_exposes_that_drawing_at_the_current_frame() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        app.session.execute("anim.blankKeyframe", &json!({"frame": 1})).unwrap();
        app.session.execute("anim.blankKeyframe", &json!({"frame": 2})).unwrap();
        let list = app.session.execute("anim.drawings", &json!({})).unwrap();
        let first = list["drawings"][0]["id"].as_u64().unwrap();
        assert_ne!(list["current"].as_u64(), Some(first));
        let ctx = egui::Context::default();
        let screen = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 300.0)));
        // The first row's label sits right of its thumbnail, on the first row.
        let at = egui::pos2(THUMB + 30.0, THUMB / 2.0);
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        let inputs = [vec![], vec![egui::Event::PointerMoved(at), press(true)], vec![press(false)], vec![]];
        for events in inputs {
            let input = egui::RawInput { screen_rect: screen, events, ..Default::default() };
            ctx.run_ui(input, |ui| show(&mut app, ui)).textures_delta.clear();
        }
        let list = app.session.execute("anim.drawings", &json!({})).unwrap();
        assert_eq!(list["current"].as_u64(), Some(first));
        assert_eq!(list["drawings"][0]["frames"], json!([1, 2]));
    }
}
