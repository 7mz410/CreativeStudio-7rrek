//! 7rrek (Creative Studio): timing charts and motion path canvas overlay.
//!
//! A timing chart is drawn under the keyframes on the timeline, marking the position of
//! each in-between frame according to the tween's easing curve (like an animator's timing chart).
//! On the canvas, when an animated layer (row) with a motion path is selected, the path is drawn
//! as a faint overlay.

use egui::{Align2, FontId, Painter, Pos2, Rect, Stroke, pos2, vec2};
use vectorcraft_doc::{Document, NodeId};
use vectorcraft_engine::cmd::anim::Anim;

use crate::VectorcraftApp;
use crate::canvas::Xf;
use crate::theme::Tokens;
use crate::timeline_av::Geom;

/// The top-level layer (timeline row) holding `id`.
fn row_of(doc: &Document, mut id: NodeId) -> Option<NodeId> {
    while let Some(p) = doc.parent_of(id) {
        id = p;
    }
    doc.layers.iter().any(|l| l.id == id).then_some(id)
}

/// A mark on the timing chart representing an in-between frame's relative position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimingMark {
    pub frame: u32,
    /// Distance or coordinate along the chart (0.0 to width).
    pub pos: f32,
    /// Normalized progress ratio (0.0 to 1.0).
    pub t: f64,
}

/// Calculate the mark positions on a timing chart between keyframes.
///
/// Pure function:
/// - `frames`: total frames in the span (inclusive, e.g. frame 1 to 5 has 5 frames).
/// - `ease`: optional CSS cubic-bezier easing control points [x1, y1, x2, y2]. `None` is linear.
/// - `width`: visual width of the span in points.
/// - `rtl`: whether display is mirrored right-to-left.
pub fn timing_marks(frames: u32, ease: Option<&[f64; 4]>, width: f32, rtl: bool) -> Vec<TimingMark> {
    if frames <= 1 {
        let pos = if rtl { width } else { 0.0 };
        return vec![TimingMark { frame: 1, pos, t: 0.0 }];
    }
    let n = (frames - 1) as f64;
    (0..frames)
        .map(|i| {
            let u = i as f64 / n;
            let t = vectorcraft_engine::cmd::motion_path::ease_value(ease, u);
            let raw_x = (t as f32) * width;
            let pos = if rtl { width - raw_x } else { raw_x };
            TimingMark { frame: i + 1, pos, t }
        })
        .collect()
}

/// How many rows [`paint`] draws: 1 if active layer has animated keys, else 0.
pub fn rows(app: &VectorcraftApp) -> usize {
    let Some(st) = app.session.active() else { return 0 };
    let Some(layer) = st.current_layer().and_then(|l| row_of(&st.doc, l)) else { return 0 };
    let anim = Anim::of(&st.doc);
    let keys = st.doc.node(layer).map(|r| anim.row_keys(r)).unwrap_or_default();
    if keys.len() >= 2 { 1 } else { 0 }
}

/// Paint the timing chart under the keyframes on the timeline.
pub fn paint(app: &VectorcraftApp, p: &Painter, t: &Tokens, g: Geom) {
    let Some(st) = app.session.active() else { return };
    let Some(layer) = st.current_layer().and_then(|l| row_of(&st.doc, l)) else { return };
    let anim = Anim::of(&st.doc);
    let keys = st.doc.node(layer).map(|r| anim.row_keys(r)).unwrap_or_default();
    if keys.len() < 2 {
        return;
    }

    let mx = |x: f32| if g.rtl { g.area.min.x + g.area.max.x - x } else { x };
    let align = if g.rtl { Align2::RIGHT_CENTER } else { Align2::LEFT_CENTER };
    let col = |f: u32| g.x0 + f.saturating_sub(1) as f32 * g.cell_w;

    let y = g.top;
    let strip = Rect::from_min_size(pos2(g.area.min.x, y), vec2(g.area.width(), g.row_h));
    p.rect_filled(strip, 0.0, t.panel_darker);
    p.text(pos2(mx(g.area.min.x + 8.0), y + g.row_h / 2.0), align, tl!("Timing Chart"), FontId::proportional(11.5), t.text_dim);

    let cy = y + g.row_h / 2.0;
    for pair in keys.windows(2) {
        let (Some(&(f_a, id_a)), Some(&(f_b, _))) = (pair.first(), pair.get(1)) else { continue };
        if f_b <= f_a {
            continue;
        }
        let span_frames = f_b - f_a + 1;
        let x_start = col(f_a) + g.cell_w / 2.0;
        let x_end = col(f_b) + g.cell_w / 2.0;
        let width = (x_end - x_start).abs();

        let ease = anim.eases.get(&id_a.0);
        let marks = timing_marks(span_frames, ease, width, false);

        // Baseline across the span
        p.line_segment([pos2(mx(x_start), cy), pos2(mx(x_end), cy)], Stroke::new(1.0, t.accent.gamma_multiply(0.4)));

        // Tick mark for each frame
        for m in marks {
            let x = mx(x_start + m.pos);
            let h = if m.frame == 1 || m.frame == span_frames { 6.0 } else { 3.5 };
            p.line_segment([pos2(x, cy - h), pos2(x, cy + h)], Stroke::new(1.0, t.accent));
        }
    }
}

/// The motion path of the selected row, if any.
pub fn shown_motion_path(app: &VectorcraftApp) -> Option<(vectorcraft_doc::Node, bool)> {
    let st = app.session.active()?;
    let row = st.current_layer().and_then(|l| row_of(&st.doc, l))?;
    let paths = vectorcraft_engine::cmd::motion_path::MotionPaths::of(&st.doc);
    let mp = paths.paths.get(&row.0)?;
    let node = st.doc.node(NodeId(mp.path))?.clone();
    Some((node, mp.orient))
}

/// Paint the faint motion path overlay on the canvas when its row is selected.
// shortcut: drawn when the row is selected; add toggle under View menu if requested
pub fn paint_overlay(app: &VectorcraftApp, p: &Painter, xf: &Xf) {
    let Some((path_node, _orient)) = shown_motion_path(app) else { return };
    let Some(path_data) = path_node.path_data() else { return };
    let t = Tokens::get(p.ctx());

    for seg in path_data.to_bezpath().segments() {
        const STEPS: usize = 16;
        let mut prev: Option<Pos2> = None;
        for i in 0..=STEPS {
            use vectorcraft_geom::kurbo::ParamCurve;
            let u = i as f64 / STEPS as f64;
            let pt = seg.eval(u);
            let screen = xf.to_screen(pt);
            if screen.is_finite() {
                if let Some(p0) = prev {
                    // Faint overlay line (alternating segments for dashed appearance)
                    if i.is_multiple_of(2) {
                        p.line_segment([p0, screen], Stroke::new(1.5, t.accent.gamma_multiply(0.45)));
                    }
                }
                prev = Some(screen);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_chart_linear_spacing_is_equal() {
        let marks = timing_marks(5, None, 100.0, false);
        assert_eq!(marks.len(), 5);
        let diffs: Vec<f32> = marks.windows(2).map(|w| w[1].pos - w[0].pos).collect();
        assert_eq!(diffs.len(), 4);
        for d in &diffs {
            assert!((d - 25.0).abs() < 1e-4, "linear spacing should be 25.0, got {d}");
        }
    }

    #[test]
    fn timing_chart_ease_in_out_is_clustered_at_ends() {
        // easeInOut: [0.42, 0.0, 0.58, 1.0]
        let ease = [0.42, 0.0, 0.58, 1.0];
        let marks = timing_marks(9, Some(&ease), 100.0, false);
        assert_eq!(marks.len(), 9);
        let diffs: Vec<f32> = marks.windows(2).map(|w| w[1].pos - w[0].pos).collect();
        let d_start = diffs[0];
        let d_end = *diffs.last().unwrap();
        let d_mid = diffs[diffs.len() / 2];
        assert!(d_start < d_mid, "start spacing {d_start} should be less than mid {d_mid}");
        assert!(d_end < d_mid, "end spacing {d_end} should be less than mid {d_mid}");
    }

    #[test]
    fn timing_chart_rtl_mirrors_positions() {
        let ltr = timing_marks(5, None, 100.0, false);
        let rtl = timing_marks(5, None, 100.0, true);
        assert_eq!(ltr.len(), rtl.len());
        for (a, b) in ltr.iter().zip(rtl.iter()) {
            assert!((a.pos + b.pos - 100.0).abs() < 1e-4, "sum should be 100.0: {} + {}", a.pos, b.pos);
        }
    }
}
