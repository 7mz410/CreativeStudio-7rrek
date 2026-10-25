//! 7rrek (Creative Studio): the Timeline panel under the canvas.
//!
//! A thin view of `anim.info`: one row per top-level layer, a column per frame, a dot where a
//! keyframe starts and a band while it holds, and the playhead. Everything it changes goes through
//! the `anim.*` commands. Playback is view state kept in egui's memory: while it plays, the panel
//! steps `anim.next` at the document's frame rate.

use egui::{Align2, Color32, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::{Value, json};

use crate::VectorcraftApp;

use crate::theme::{self, Tokens};

const NAME_W: f32 = 140.0;
const CELL_W: f32 = 14.0;
const ROW_H: f32 = 20.0;
const BAR_H: f32 = 30.0;
const RULER_H: f32 = 16.0;
const MAX_ROWS_SHOWN: usize = 6;

#[derive(Clone, Copy, Default)]
struct Playback {
    playing: bool,
    /// Seconds since the last step.
    acc: f32,
}

fn playback_id() -> egui::Id {
    egui::Id::new("7rrek-playback")
}

/// The Timeline's height for the active document (its rows, up to [`MAX_ROWS_SHOWN`]).
pub fn height(app: &VectorcraftApp) -> f32 {
    let rows = app.session.active().map_or(1, |st| st.doc.layers.len());
    BAR_H + RULER_H + rows.clamp(1, MAX_ROWS_SHOWN) as f32 * ROW_H + 6.0
}

/// Draw the Timeline filling `ui` (the caller gives it its strip of the window).
pub fn timeline(app: &mut VectorcraftApp, ui: &mut Ui) {
    // Read the timeline straight from the document: a command call every frame would count as
    // activity (and, on macOS, rebuild the menus).
    let Some(info) = app.session.active().map(|st| info_of(&st.doc)) else { return };
    let t = Tokens::get(ui.ctx());
    let rows = info["rows"].as_array().cloned().unwrap_or_default();
    let frames = info["frames"].as_u64().unwrap_or(1).max(1) as u32;
    let current = info["current"].as_u64().unwrap_or(1) as u32;
    let fps = info["fps"].as_u64().unwrap_or(12).max(1) as u32;
    let onion = info["onion"].as_bool().unwrap_or(false);

    tick(app, ui, fps);

    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, t.panel);
    ui.painter().line_segment([full.left_top(), full.right_top()], Stroke::new(1.0, t.border));
    let bar = Rect::from_min_size(full.min, vec2(full.width(), BAR_H));
    // Right to left in Arabic; scrolls sideways when the window is too narrow for every button.
    let flow = if egui::is_rtl() { egui::Layout::right_to_left(egui::Align::Center) } else { egui::Layout::left_to_right(egui::Align::Center) };
    ui.scope_builder(egui::UiBuilder::new().max_rect(bar), |ui| {
        egui::ScrollArea::horizontal().id_salt("timeline-bar").show(ui, |ui| {
            ui.set_min_height(BAR_H);
            ui.with_layout(flow, |ui| controls(app, ui, &t, current, frames, fps, onion));
        });
    });
    let below = Rect::from_min_max(pos2(full.min.x, bar.max.y), full.max);
    ui.scope_builder(egui::UiBuilder::new().max_rect(below), |ui| {
        egui::ScrollArea::both().id_salt("timeline-grid").auto_shrink([false, false]).show(ui, |ui| {
            grid(app, ui, &t, &rows, current, frames);
        });
    });
}

fn info_of(doc: &vectorcraft_doc::Document) -> Value {
    use vectorcraft_engine::cmd::anim::Anim;
    let a = Anim::of(doc);
    let rows: Vec<Value> = doc
        .layers
        .iter()
        .rev()
        .map(|r| {
            let keys: Vec<Value> = a
                .row_keys(r)
                .into_iter()
                .map(|(f, id)| json!({"frame": f, "id": id.0, "tween": a.tweens.contains(&id.0), "ease": a.eases.get(&id.0)}))
                .collect();
            json!({"id": r.id.0, "name": r.name.clone().unwrap_or_default(), "keys": keys})
        })
        .collect();
    json!({"fps": a.fps, "frames": a.frames, "current": a.current, "rows": rows, "onion": a.onion_before + a.onion_after > 0})
}

/// Step the playhead while playing.
fn tick(app: &mut VectorcraftApp, ui: &Ui, fps: u32) {
    let ctx = ui.ctx().clone();
    let mut pb: Playback = ctx.data(|d| d.get_temp(playback_id())).unwrap_or_default();
    if !pb.playing {
        return;
    }
    pb.acc += ctx.input(|i| i.stable_dt).min(0.5);
    let step = 1.0 / fps as f32;
    if pb.acc >= step {
        pb.acc %= step;
        let _ = app.run("anim.next", json!({}));
    }
    ctx.data_mut(|d| d.insert_temp(playback_id(), pb));
    ctx.request_repaint();
}

fn set_playing(ui: &Ui, on: bool) {
    ui.ctx().data_mut(|d| d.insert_temp(playback_id(), Playback { playing: on, acc: 0.0 }));
}

fn controls(app: &mut VectorcraftApp, ui: &mut Ui, t: &Tokens, current: u32, frames: u32, fps: u32, onion: bool) {
    let playing = ui.ctx().data(|d| d.get_temp::<Playback>(playback_id())).is_some_and(|p| p.playing);
    {
        ui.add_space(8.0);
        ui.label(egui::RichText::new(tl!("Timeline")).font(theme::semibold(12.0)).color(t.text_strong));
        ui.add_space(10.0);
        if ui.small_button("⏮").on_hover_text(tl!("First Frame")).clicked() {
            let _ = app.run("anim.goto", json!({"frame": 1}));
        }
        if ui.small_button("◀").on_hover_text(tl!("Previous Frame")).clicked() {
            let _ = app.run("anim.prev", json!({}));
        }
        let play = if playing { "⏸" } else { "▶" };
        if ui.small_button(play).on_hover_text(if playing { tl!("Pause") } else { tl!("Play") }).clicked() {
            set_playing(ui, !playing);
        }
        if ui.small_button("▶|").on_hover_text(tl!("Next Frame")).clicked() {
            let _ = app.run("anim.next", json!({}));
        }
        if ui.small_button("⏭").on_hover_text(tl!("Last Frame")).clicked() {
            let _ = app.run("anim.goto", json!({"frame": frames}));
        }
        ui.add_space(8.0);
        ui.label(egui::RichText::new(format!("{current} / {frames}")).size(12.0).color(t.text));
        ui.add_space(12.0);
        ui.label(egui::RichText::new(tl!("FPS")).size(11.5).color(t.text_dim));
        let mut f = fps;
        if ui.add(egui::DragValue::new(&mut f).range(1..=60)).changed() {
            let _ = app.run("anim.setFps", json!({"fps": f}));
        }
        ui.label(egui::RichText::new(tl!("Frames")).size(11.5).color(t.text_dim));
        let mut n = frames;
        if ui.add(egui::DragValue::new(&mut n).range(1..=10_000)).changed() {
            let _ = app.run("anim.setFrames", json!({"frames": n}));
        }
        ui.add_space(12.0);
        if ui.button(tl!("Keyframe")).on_hover_text(tl!("Insert Keyframe")).clicked() {
            run_reporting(app, "anim.keyframe");
        }
        if ui.button(tl!("Blank Keyframe")).on_hover_text(tl!("Insert Blank Keyframe")).clicked() {
            run_reporting(app, "anim.blankKeyframe");
        }
        if ui.button(tl!("Clear Keyframe")).clicked() {
            run_reporting(app, "anim.removeKeyframe");
        }
        if ui.button(tl!("Motion Tween")).on_hover_text(tl!("Move the keyframe's art toward the next keyframe")).clicked() {
            run_reporting(app, "anim.tween");
        }
        ui.menu_button(tl!("Easing"), |ui| {
            for (id, label) in
                [("linear", tl!("Linear")), ("easeIn", tl!("Ease In")), ("easeOut", tl!("Ease Out")), ("easeInOut", tl!("Ease In Out"))]
            {
                if ui.button(label).clicked() {
                    if let Err(e) = app.run("anim.tween", json!({"ease": id})) {
                        app.status(e);
                    }
                    ui.close();
                }
            }
        })
        .response
        .on_hover_text(tl!("How the tween speeds up and slows down"));
        ui.add_space(8.0);
        // Onion skin: one keyframe on each side, faint.
        if ui.selectable_label(onion, tl!("Onion Skin")).on_hover_text(tl!("Show the keyframes before and after faintly")).clicked() {
            let n = if onion { 0 } else { 1 };
            let _ = app.run("anim.onion", json!({"before": n, "after": n}));
        }
        ui.add_space(8.0);
        if ui.button(tl!("Export GIF…")).on_hover_text(tl!("Export Animated GIF")).clicked() {
            run_reporting(app, "anim.exportGifAs");
        }
        if ui.button(tl!("Export Video…")).on_hover_text(tl!("Export the frames as a WebM video")).clicked() {
            run_reporting(app, "anim.exportVideoAs");
        }
    }
}

/// `anim.exportGifAs`: the frames as a GIF at `path`, else a picked file (the web downloads it).
pub fn export_gif(app: &mut VectorcraftApp, path: Option<String>, p: &Value) -> Result<Value, String> {
    let doc = app.session.active().ok_or("no document")?.doc.clone();
    let path = crate::io::target_path(app, path, "gif")?;
    let (bytes, mut info) = vectorcraft_engine::cmd::anim::encode_gif(&doc, p).map_err(|e| e.to_string())?;
    crate::io::write_to(&mut app.services, &path, &bytes)?;
    info["path"] = json!(path);
    Ok(info)
}

/// `anim.exportVideoAs`: the frames as a WebM at `path`, else a picked file (the web downloads it).
pub fn export_video(app: &mut VectorcraftApp, path: Option<String>, p: &Value) -> Result<Value, String> {
    let doc = app.session.active().ok_or("no document")?.doc.clone();
    let ext = if p.get("format").and_then(Value::as_str) == Some("mp4") { "mp4" } else { "webm" };
    let path = crate::io::target_path(app, path, ext)?;
    // A name typed with .mp4 picks MP4.
    let mut p = p.clone();
    if p.get("format").is_none() && path.to_ascii_lowercase().ends_with(".mp4") {
        p["format"] = json!("mp4");
    }
    let (bytes, mut info) = vectorcraft_engine::cmd::anim::encode_video(&doc, &p).map_err(|e| e.to_string())?;
    crate::io::write_to(&mut app.services, &path, &bytes)?;
    info["path"] = json!(path);
    Ok(info)
}

fn run_reporting(app: &mut VectorcraftApp, id: &str) {
    if let Err(e) = app.run(id, json!({})) {
        app.status(e);
    }
}

fn grid(app: &mut VectorcraftApp, ui: &mut Ui, t: &Tokens, rows: &[Value], current: u32, frames: u32) {
    let width = NAME_W + frames as f32 * CELL_W + 8.0;
    let extra = crate::timeline_av::rows(app);
    let height = RULER_H + (rows.len().max(1) + extra) as f32 * ROW_H;
    let (area, resp) = ui.allocate_exact_size(vec2(width, height), Sense::click_and_drag());
    let p = ui.painter_at(area);
    // Laid out left to right, then mirrored in a right-to-left language: names on the right,
    // frame 1 next to them, time running leftwards.
    let rtl = egui::is_rtl();
    let mx = |x: f32| if rtl { area.min.x + area.max.x - x } else { x };
    let mrect = |r: Rect| Rect::from_two_pos(pos2(mx(r.min.x), r.min.y), pos2(mx(r.max.x), r.max.y));
    let (start, end_align) = if rtl { (Align2::RIGHT_TOP, Align2::RIGHT_CENTER) } else { (Align2::LEFT_TOP, Align2::LEFT_CENTER) };
    let x0 = area.min.x + NAME_W;
    let col = |f: u32| x0 + (f - 1) as f32 * CELL_W;

    // Frame ruler: a tick per frame, numbers every 5.
    for f in 1..=frames {
        let x = mx(col(f));
        let major = f == 1 || f % 5 == 0;
        p.line_segment([pos2(x, area.min.y + if major { 2.0 } else { 9.0 }), pos2(x, area.min.y + RULER_H)], Stroke::new(1.0, t.ruler_tick));
        if major {
            p.text(pos2(x + if rtl { -2.0 } else { 2.0 }, area.min.y + 1.0), start, f.to_string(), egui::FontId::proportional(9.5), t.text_dim);
        }
    }

    for (i, row) in rows.iter().enumerate() {
        let y = area.min.y + RULER_H + i as f32 * ROW_H;
        let band = Rect::from_min_size(pos2(area.min.x, y), vec2(width, ROW_H));
        p.rect_filled(band, 0.0, if i % 2 == 0 { t.panel } else { t.panel_darker });
        let name = row["name"].as_str().unwrap_or("");
        p.text(pos2(mx(area.min.x + 8.0), y + ROW_H / 2.0), end_align, name, egui::FontId::proportional(11.5), t.text);
        let keys: Vec<(u32, bool)> = row["keys"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|k| Some((k["frame"].as_u64()? as u32, k["tween"].as_bool().unwrap_or(false))))
            .collect();
        for (k, (f, tween)) in keys.iter().enumerate() {
            let next = keys.get(k + 1).map(|(n, _)| *n);
            let end = next.unwrap_or(frames + 1);
            let span = Rect::from_min_max(pos2(col(*f), y + 3.0), pos2(col(end), y + ROW_H - 3.0));
            p.rect_filled(mrect(span), 3.0, t.accent.gamma_multiply(if *tween && next.is_some() { 0.45 } else { 0.25 }));
            if *tween && next.is_some() {
                let len = col(end) - col(*f) - CELL_W * 1.5;
                p.arrow(pos2(mx(col(*f) + CELL_W), span.center().y), vec2(if rtl { -len } else { len }, 0.0), Stroke::new(1.0, t.accent));
            }
            p.circle_filled(pos2(mx(col(*f) + CELL_W / 2.0), y + ROW_H / 2.0), 3.5, t.accent);
        }
    }

    // The camera and sound rows, under the layers.
    let top = area.min.y + RULER_H + rows.len().max(1) as f32 * ROW_H;
    crate::timeline_av::paint(app, &p, t, crate::timeline_av::Geom { area, top, row_h: ROW_H, name_w: NAME_W, x0, cell_w: CELL_W, frames, rtl });

    // The playhead.
    let x = mx(col(current) + CELL_W / 2.0);
    p.line_segment([pos2(x, area.min.y), pos2(x, area.max.y)], Stroke::new(1.5, Color32::from_rgb(0xeb, 0x1b, 0x3c)));

    // Click or drag on the frames: go there (and to that row's layer).
    if let Some(pos) = resp.interact_pointer_pos().filter(|_| resp.clicked() || resp.dragged()).map(|q| pos2(mx(q.x), q.y))
        && pos.x >= x0
    {
        let f = (((pos.x - x0) / CELL_W).floor() as u32 + 1).clamp(1, frames);
        if f != current {
            set_playing(ui, false);
            let _ = app.run("anim.goto", json!({"frame": f}));
        }
        let i = ((pos.y - area.min.y - RULER_H) / ROW_H).floor();
        if resp.clicked()
            && i >= 0.0
            && let Some(id) = rows.get(i as usize).and_then(|r| r["id"].as_u64())
        {
            let _ = app.run("layer.setCurrent", json!({"id": id}));
            let _ = app.run("anim.goto", json!({"frame": f}));
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vectorcraft_engine::Session;

    use crate::VectorcraftApp;
    use crate::tests_labels::painted_text;

    fn app_with_keys() -> VectorcraftApp {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        app.run("anim.keyframe", json!({"frame": 1})).unwrap();
        app.run("shape.ellipse", json!({"x": 20, "y": 20, "width": 40, "height": 40})).unwrap();
        app.run("anim.blankKeyframe", json!({"frame": 6})).unwrap();
        app.run("anim.goto", json!({"frame": 8})).unwrap();
        app
    }

    #[test]
    fn the_timeline_shows_rows_frames_and_controls() {
        let mut app = app_with_keys();
        let text = painted_text(&mut app, super::timeline);
        for s in ["Timeline", "8 / 24", "Keyframe", "Blank Keyframe", "Clear Keyframe", "Motion Tween", "Export GIF…", "Layer 1", "1", "5", "10"] {
            assert!(text.lines().any(|l| l == s), "{s} in {text}");
        }
    }

    #[test]
    fn export_gif_as_writes_the_frames() {
        let mut app = app_with_keys();
        app.services.write = Some(Box::new(|p: &str, b: &[u8]| std::fs::write(p, b).map_err(|e| e.to_string())));
        let path = std::env::temp_dir().join(format!("7rrek-ui-gif-{}.gif", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let r = app.run("anim.exportGifAs", json!({"path": path.to_string_lossy(), "scale": 0.25})).unwrap();
        assert_eq!(r["frames"], 24);
        assert!(std::fs::read(&path).unwrap().starts_with(b"GIF89a"));
        let _ = std::fs::remove_file(&path);
        let path = path.with_extension("webm");
        let r = app.run("anim.exportVideoAs", json!({"path": path.to_string_lossy(), "scale": 0.25})).unwrap();
        assert_eq!(r["frames"], 24);
        assert!(std::fs::read(&path).unwrap().starts_with(&[0x1A, 0x45, 0xDF, 0xA3]));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn playing_steps_frames_at_the_frame_rate() {
        let mut app = app_with_keys();
        app.run("anim.setFps", json!({"fps": 10})).unwrap();
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        ctx.data_mut(|d| d.insert_temp(super::playback_id(), super::Playback { playing: true, acc: 0.0 }));
        for _ in 0..3 {
            let input = egui::RawInput { predicted_dt: 0.1, ..Default::default() };
            ctx.run_ui(input, |ui| super::timeline(&mut app, ui)).textures_delta.clear();
        }
        let now = app.run("anim.info", json!({})).unwrap()["current"].as_u64().unwrap();
        assert!(now > 8, "the playhead moved on from frame 8: {now}");
    }
}

#[cfg(test)]
mod snapshot_probe {
    use serde_json::json;
    use vectorcraft_engine::Session;

    use crate::VectorcraftApp;

    #[test]
    #[ignore = "writes a PNG of the whole window for a look; run by hand"]
    fn whole_window_png() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({})).unwrap();
        app.run("anim.keyframe", json!({"frame": 1})).unwrap();
        app.run("shape.ellipse", json!({"x": 100, "y": 300, "width": 80, "height": 80})).unwrap();
        app.run("anim.blankKeyframe", json!({"frame": 6})).unwrap();
        app.run("anim.tween", json!({"frame": 1, "ease": "easeOut"})).unwrap();
        // TIMELINE_LANG=ar for a look right to left.
        if let Some(l) = std::env::var("TIMELINE_LANG").ok().and_then(|c| crate::i18n::Lang::from_code(&c)) {
            crate::i18n::set_current(l);
            egui::set_rtl(crate::i18n::bidi::RTL_CODES.contains(&l.code()));
        }
        let mut ready = false;
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1000.0, 200.0)).wgpu().build_ui_state(
            move |ui, app: &mut VectorcraftApp| {
                if !ready {
                    crate::theme::install_fonts(ui.ctx());
                    crate::theme::apply(ui.ctx(), Default::default());
                    ready = true;
                    return;
                }
                egui::CentralPanel::default().show(ui, |ui| super::timeline(app, ui));
            },
            app,
        );
        h.run_steps(4);
        let path = std::env::var("TIMELINE_PNG").unwrap_or_else(|_| "/tmp/timeline.png".into());
        h.render().unwrap().save(path).unwrap();
    }
}
