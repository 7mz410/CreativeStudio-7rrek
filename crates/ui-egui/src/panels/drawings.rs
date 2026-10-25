//! Drawings panel: the current layer row's drawings (its keyframes) and the frames each one
//! shows from. A click exposes the drawing at the current frame on (`anim.expose`), as the
//! exposure sheet does; the drawing shown at the current frame is selected.

use egui::Ui;
use serde_json::{Value, json};

use crate::VectorcraftApp;
use crate::widgets;

/// The panel id, as the dock, the Window menu and `window.panel` name it.
pub const ID: &str = "drawings";

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
    let mut clicked = None;
    for d in &drawings {
        let id = d["id"].as_u64().unwrap_or_default();
        let frames: Vec<String> = d["frames"].as_array().into_iter().flatten().filter_map(Value::as_u64).map(|f| f.to_string()).collect();
        let frames = crate::i18n::fmt(tl!("frames: {frames}"), &[("frames", &frames.join(", "))]);
        let text = format!("{} · {frames}", d["name"].as_str().unwrap_or_default());
        if ui.selectable_label(current == Some(id), text).clicked() && current != Some(id) {
            clicked = Some(id);
        }
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
}
