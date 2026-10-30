//! Camera Preview panel: the current frame as the animation camera sees it, depth and multiplane
//! included, so the animator sees the depth while drawing. It is the picture the frame export
//! makes (`anim.exportVideo`), fitted to the panel's width and drawn again only when the document,
//! the frame or the panel's pixel size changes.

use std::cell::RefCell;

use egui::{Color32, Pos2, Rect as UiRect, Sense, TextureHandle, TextureOptions, Ui, vec2};
use vectorcraft_engine::cmd::anim::{Anim, at_frame, through_camera};

use crate::VectorcraftApp;
use crate::widgets;

/// The panel id, as the dock, the Window menu and `window.panel` name it.
pub const ID: &str = "cameraPreview";

/// Most pixels one preview renders; a tall artboard is scaled down to stay under it.
const MAX_PIXELS: f64 = 4.0e6;

/// The document, its revision, the frame and the render scale (bits): what the picture depends on.
type Key = (u64, u64, u32, u64);

thread_local! {
    static RENDERER: RefCell<vectorcraft_render::Renderer> = RefCell::new(vectorcraft_render::Renderer::new());
    /// The last picture drawn; one slot is enough, the panel shows one frame.
    static SHOWN: RefCell<Option<(Key, TextureHandle)>> = const { RefCell::new(None) };
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let Some(st) = app.session.active() else {
        widgets::dim_label(ui, tl!("No document"));
        return;
    };
    let Some(art) = st.doc.artboards.first().map(|a| a.rect).filter(|r| r.width() > 0.0 && r.height() > 0.0) else {
        widgets::dim_label(ui, tl!("No artboard"));
        return;
    };
    let anim = Anim::of(&st.doc);
    let frame = anim.current;
    if anim.camera.is_empty() {
        widgets::dim_label(ui, tl!("No camera keys: showing the artboard"));
    }

    // Device pixels across the panel, and the scale that gives them (capped by MAX_PIXELS).
    let ppp = ui.ctx().pixels_per_point();
    let (w, h) = (art.width(), art.height());
    let fit = f64::from(ui.available_width().max(1.0) * ppp);
    let scale = (fit / w).min((MAX_PIXELS / (w * h)).sqrt());
    if !(scale.is_finite() && scale > 0.0) {
        widgets::dim_label(ui, tl!("No artboard"));
        return;
    }
    let (area, _) = ui.allocate_exact_size(vec2((w * scale) as f32, (h * scale) as f32) / ppp, Sense::hover());

    let key: Key = (st.uid, st.revision, frame, scale.to_bits());
    let tex = SHOWN.with(|shown| {
        let mut shown = shown.borrow_mut();
        if !matches!(shown.as_ref(), Some((k, _)) if *k == key) {
            let img = RENDERER.with(|r| {
                let seen = through_camera(at_frame(&st.doc, frame), anim.camera_at(frame), art);
                r.borrow_mut().render_region(&seen, art, scale, true)
            });
            let color = crate::canvas::color_image(&img);
            let tex = ui.ctx().load_texture("camera-preview", color, TextureOptions::LINEAR);
            *shown = Some((key, tex));
        }
        shown.as_ref().map(|(_, t)| t.clone())
    });
    ui.painter().rect_filled(area, 0.0, Color32::WHITE);
    if let Some(t) = tex {
        let full = UiRect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        ui.painter().image(t.id(), area, full, Color32::WHITE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn app() -> VectorcraftApp {
        VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default())
    }

    /// One frame of the panel in a 400×300 window.
    fn frame(app: &mut VectorcraftApp, ctx: &egui::Context) {
        let screen = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(400.0, 300.0)));
        let input = egui::RawInput { screen_rect: screen, ..Default::default() };
        ctx.run_ui(input, |ui| show(app, ui)).textures_delta.clear();
    }

    #[test]
    fn camera_preview_without_a_document_shows_the_empty_state() {
        let mut app = app();
        let ctx = egui::Context::default();
        frame(&mut app, &ctx);
    }

    #[test]
    fn camera_preview_without_camera_keys_shows_the_artboard() {
        let mut app = app();
        app.session.execute("file.new", &json!({"width": 100, "height": 60})).unwrap();
        let ctx = egui::Context::default();
        frame(&mut app, &ctx);
        frame(&mut app, &ctx);
    }

    #[test]
    fn camera_preview_with_a_camera_key_renders_through_the_camera() {
        let mut app = app();
        app.session.execute("file.new", &json!({"width": 100, "height": 60})).unwrap();
        app.session.execute("anim.cameraKey", &json!({"frame": 1, "x": 10.0, "y": 0.0, "zoom": 1.5, "rotation": 0.0})).unwrap();
        let ctx = egui::Context::default();
        frame(&mut app, &ctx);
        frame(&mut app, &ctx);
    }
}
