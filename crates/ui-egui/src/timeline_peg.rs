//! The timeline's peg rows: one per peg, with its keys.
use egui::{Align2, FontId, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};
use vectorcraft_engine::cmd::peg::Pegs;

use crate::{VectorcraftApp, theme::Tokens, timeline_av::Geom};

/// Half the width and height of a peg key's diamond.
const DIAMOND: f32 = 4.0;
/// Indent per level of nesting, in points.
const INDENT: f32 = 12.0;
/// Deepest nesting the indent follows; a longer chain is drawn at this depth.
const MAX_DEPTH: usize = 8;

/// The document's pegs, when a document is open.
fn pegs(app: &VectorcraftApp) -> Option<Pegs> {
    let st = app.session.active()?;
    Some(Pegs::of(&st.doc))
}

/// How deep peg `i` hangs under its parents: 0 for a top-level peg.
fn depth(pegs: &Pegs, i: usize) -> usize {
    let mut d = 0;
    let mut q = i;
    while d < MAX_DEPTH {
        match pegs.pegs.get(q).and_then(|p| p.parent) {
            Some(parent) => {
                d += 1;
                q = parent;
            }
            None => break,
        }
    }
    d
}

/// How many rows [`paint`] draws: one per peg.
pub fn rows(app: &VectorcraftApp) -> usize {
    pegs(app).map_or(0, |p| p.pegs.len())
}

pub fn paint(app: &VectorcraftApp, p: &Painter, t: &Tokens, g: Geom) {
    let (Some(pegs), Some(st)) = (pegs(app), app.session.active()) else { return };
    let mx = |x: f32| if g.rtl { g.area.min.x + g.area.max.x - x } else { x };
    let align = if g.rtl { Align2::RIGHT_CENTER } else { Align2::LEFT_CENTER };
    let col = |f: u32| g.x0 + f.saturating_sub(1) as f32 * g.cell_w;
    // Rows keep the layers' banding: these rows come after the layer, av and bones rows.
    let base = st.doc.layers.len().max(1) + crate::timeline_av::rows(app) + crate::timeline_rig::rows(app);

    for (k, peg) in pegs.pegs.iter().enumerate() {
        let y = g.top + k as f32 * g.row_h;
        let band = if (base + k).is_multiple_of(2) { t.panel } else { t.panel_darker };
        let strip = Rect::from_min_size(pos2(g.area.min.x, y), vec2(g.area.width(), g.row_h));
        p.rect_filled(strip, 0.0, band);
        let indent = depth(&pegs, k) as f32 * INDENT;
        let label = format!("{} ({})", peg.name, peg.rows.len());
        p.text(pos2(mx(g.area.min.x + 8.0 + indent), y + g.row_h / 2.0), align, label, FontId::proportional(11.5), t.text);

        let cy = y + g.row_h / 2.0;
        let centres: Vec<Pos2> = peg.keys.keys().map(|f| pos2(mx(col(*f) + g.cell_w / 2.0), cy)).collect();
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
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vectorcraft_engine::Session;

    use crate::VectorcraftApp;

    #[test]
    fn rows_is_one_per_peg() {
        assert_eq!(super::rows(&VectorcraftApp::new(Session::new(), Default::default())), 0);
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 400, "height": 300})).unwrap();
        assert_eq!(super::rows(&app), 0);
        app.session.execute("peg.add", &json!({})).unwrap();
        app.session.execute("peg.add", &json!({})).unwrap();
        assert_eq!(super::rows(&app), 2);
    }
}
