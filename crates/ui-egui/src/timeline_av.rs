//! 7rrek (Creative Studio): the timeline's camera and sound rows, drawn under the layer rows.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use egui::{Align2, FontId, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};
use vectorcraft_engine::cmd::anim::Anim;

use crate::VectorcraftApp;
use crate::theme::Tokens;

/// Where the rows go: the grid's area, the y of the first extra row, row height, the name column's
/// width, the x of frame 1 and a frame's width (both before mirroring), the frame count, and
/// whether the grid is mirrored (right to left: x ↦ area.min.x + area.max.x − x).
#[derive(Clone, Copy, Debug)]
pub struct Geom {
    pub area: Rect,
    pub top: f32,
    pub row_h: f32,
    pub name_w: f32,
    pub x0: f32,
    pub cell_w: f32,
    pub frames: u32,
    pub rtl: bool,
}

/// Waveform buckets kept per sound file; a bar draws at most this many lines.
const PEAKS: usize = 512;
/// Half the width and height of a camera keyframe's diamond.
const DIAMOND: f32 = 4.0;

/// A sound file's length and waveform, read once.
struct Clip {
    seconds: f64,
    peaks: Vec<f32>,
}

thread_local! {
    /// Each sound file's decoded clip, by path; `None` when it can't be read or decoded.
    static CLIPS: RefCell<HashMap<String, Option<Rc<Clip>>>> = RefCell::new(HashMap::new());
}

/// The clip at `path`, decoded on first use only.
fn clip(path: &str) -> Option<Rc<Clip>> {
    CLIPS.with(|c| {
        let mut c = c.borrow_mut();
        if let Some(v) = c.get(path) {
            return v.clone();
        }
        let v = load(path).map(Rc::new);
        c.insert(path.to_string(), v.clone());
        v
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn load(path: &str) -> Option<Clip> {
    let bytes = std::fs::read(path).ok()?;
    let ext = std::path::Path::new(path).extension().and_then(|e| e.to_str());
    let pcm = vectorcraft_audio::decode(&bytes, ext).ok()?;
    let seconds = (pcm.samples.len() / 2) as f64 / f64::from(vectorcraft_audio::RATE);
    Some(Clip { seconds, peaks: vectorcraft_audio::peaks(&pcm, PEAKS) })
}

/// The web build has no file access: a sound draws as an empty bar.
#[cfg(target_arch = "wasm32")]
fn load(_path: &str) -> Option<Clip> {
    None
}

/// How many rows [`paint`] draws for the active document: the camera row (when it has keys) and
/// one per sound.
pub fn rows(app: &VectorcraftApp) -> usize {
    let Some(st) = app.session.active() else { return 0 };
    let a = Anim::of(&st.doc);
    usize::from(!a.camera.is_empty()) + a.sounds.len()
}

pub fn paint(app: &VectorcraftApp, p: &Painter, t: &Tokens, g: Geom) {
    let Some(st) = app.session.active() else { return };
    let a = Anim::of(&st.doc);
    let mx = |x: f32| if g.rtl { g.area.min.x + g.area.max.x - x } else { x };
    let mrect = |r: Rect| Rect::from_two_pos(pos2(mx(r.min.x), r.min.y), pos2(mx(r.max.x), r.max.y));
    let align = if g.rtl { Align2::RIGHT_CENTER } else { Align2::LEFT_CENTER };
    let col = |f: u32| g.x0 + f.saturating_sub(1) as f32 * g.cell_w;
    // Rows keep the layers' banding: the extra rows continue after the layer rows.
    let base = st.doc.layers.len().max(1);
    let band = |i: usize| if i.is_multiple_of(2) { t.panel } else { t.panel_darker };
    let draw_row = |k: usize, name: &str| -> f32 {
        let y = g.top + k as f32 * g.row_h;
        let strip = Rect::from_min_size(pos2(g.area.min.x, y), vec2(g.area.width(), g.row_h));
        p.rect_filled(strip, 0.0, band(base + k));
        p.text(pos2(mx(g.area.min.x + 8.0), y + g.row_h / 2.0), align, name, FontId::proportional(11.5), t.text);
        y
    };

    let mut k = 0;
    if !a.camera.is_empty() {
        let y = draw_row(k, tl!("Camera"));
        let cy = y + g.row_h / 2.0;
        let centres: Vec<Pos2> = a.camera.keys().map(|f| pos2(mx(col(*f) + g.cell_w / 2.0), cy)).collect();
        for pair in centres.windows(2) {
            if let &[from, to] = pair {
                p.line_segment([from, to], Stroke::new(1.0, t.accent));
            }
        }
        for c in &centres {
            let d = DIAMOND;
            let pts = vec![pos2(c.x, c.y - d), pos2(c.x + d, c.y), pos2(c.x, c.y + d), pos2(c.x - d, c.y)];
            p.add(Shape::convex_polygon(pts, t.accent, Stroke::NONE));
        }
        k += 1;
    }

    for s in &a.sounds {
        let y = draw_row(k, &s.name);
        let clip = clip(&s.path);
        // The bar's length in frames: the file's seconds at the document's rate; one frame when the
        // file can't be read, so the bar still shows where the clip starts.
        let len = clip.as_ref().map_or(1.0, |c| (c.seconds * f64::from(a.fps)).round().max(1.0) as f32);
        let left = col(s.start);
        let logical = Rect::from_min_max(pos2(left, y + 3.0), pos2(left + len * g.cell_w, y + g.row_h - 3.0));
        p.rect_filled(mrect(logical), 3.0, t.accent.gamma_multiply(0.25));
        if let Some(c) = &clip {
            let cols = ((logical.width() / 2.0) as usize).clamp(1, PEAKS);
            let step = logical.width() / cols as f32;
            let (cy, half) = (logical.center().y, logical.height() / 2.0);
            for i in 0..cols {
                let amp = c.peaks.get(i * PEAKS / cols).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let x = mx(logical.min.x + (i as f32 + 0.5) * step);
                p.line_segment([pos2(x, cy - amp * half), pos2(x, cy + amp * half)], Stroke::new(1.0, t.text_dim));
            }
        }
        k += 1;
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;
    use vectorcraft_engine::Session;

    use crate::VectorcraftApp;

    /// A half-second mono 16-bit WAV at 22.05 kHz, written by hand into a temp folder.
    fn write_wav(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("7rrek-av-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rate: u32 = 22_050;
        let mut data = Vec::new();
        for i in 0..rate / 2 {
            let v = ((i as f32 * 0.05).sin() * 12_000.0) as i16;
            data.extend_from_slice(&v.to_le_bytes());
        }
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes()); // PCM
        b.extend_from_slice(&1u16.to_le_bytes()); // mono
        b.extend_from_slice(&rate.to_le_bytes());
        b.extend_from_slice(&(rate * 2).to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(data.len() as u32).to_le_bytes());
        b.extend_from_slice(&data);
        let path = dir.join(name);
        std::fs::write(&path, b).unwrap();
        path
    }

    fn cam(app: &mut VectorcraftApp, frame: u32, zoom: f64) {
        app.run("anim.cameraKey", json!({"frame": frame, "x": 0.0, "y": 0.0, "zoom": zoom, "rotation": 0.0})).unwrap();
    }

    #[test]
    fn rows_count_the_camera_row_and_each_sound() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        assert_eq!(super::rows(&app), 0);
        app.run("file.new", json!({})).unwrap();
        assert_eq!(super::rows(&app), 0);
        cam(&mut app, 1, 1.0);
        cam(&mut app, 10, 2.0);
        let wav = write_wav("tone.wav");
        app.run("anim.soundImport", json!({"path": wav.to_string_lossy(), "start": 3})).unwrap();
        assert_eq!(super::rows(&app), 2);
    }

    #[test]
    fn a_missing_sound_file_is_an_empty_bar_not_a_panic() {
        assert!(super::clip("/nonexistent/7rrek/missing.wav").is_none());
    }

    #[test]
    #[ignore = "writes PNGs of the camera and sound rows for a look; run by hand"]
    fn av_rows_png() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({})).unwrap();
        app.run("anim.keyframe", json!({"frame": 1})).unwrap();
        app.run("shape.ellipse", json!({"x": 100, "y": 300, "width": 80, "height": 80})).unwrap();
        app.run("anim.blankKeyframe", json!({"frame": 6})).unwrap();
        cam(&mut app, 1, 1.0);
        cam(&mut app, 12, 2.0);
        let wav = write_wav("look.wav");
        app.run("anim.soundImport", json!({"path": wav.to_string_lossy(), "start": 2})).unwrap();
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
                egui::CentralPanel::default().show(ui, |ui| crate::timeline::timeline(app, ui));
            },
            app,
        );
        h.run_steps(4);
        let path = std::env::var("TIMELINE_PNG").unwrap_or_else(|_| "/tmp/timeline_av.png".into());
        h.render().unwrap().save(path).unwrap();
    }
}
