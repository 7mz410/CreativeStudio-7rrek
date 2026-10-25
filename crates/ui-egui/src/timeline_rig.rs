//! The timeline's Bones row: the current layer's pose keys.
use egui::{Align2, FontId, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};
use vectorcraft_doc::{Document, NodeId};
use vectorcraft_engine::cmd::rig::{Rig, Skeleton};

use crate::{VectorcraftApp, theme::Tokens, timeline_av::Geom};

/// Half the width and height of a pose key's diamond.
const DIAMOND: f32 = 4.0;

/// The top-level layer (timeline row) holding `id`, the way the animation commands find it.
fn row_of(doc: &Document, mut id: NodeId) -> Option<NodeId> {
    while let Some(p) = doc.parent_of(id) {
        id = p;
    }
    doc.layers.iter().any(|l| l.id == id).then_some(id)
}

/// The current layer's skeleton, when it has at least one joint.
fn skeleton(app: &VectorcraftApp) -> Option<Skeleton> {
    let st = app.session.active()?;
    let row = row_of(&st.doc, st.current_layer()?)?;
    let sk = Rig::of(&st.doc).skeletons.remove(&row.0)?;
    (!sk.joints.is_empty()).then_some(sk)
}

/// How many rows [`paint`] draws: the bones row, when the current layer has a skeleton.
pub fn rows(app: &VectorcraftApp) -> usize {
    usize::from(skeleton(app).is_some())
}

pub fn paint(app: &VectorcraftApp, p: &Painter, t: &Tokens, g: Geom) {
    let (Some(sk), Some(st)) = (skeleton(app), app.session.active()) else { return };
    let mx = |x: f32| if g.rtl { g.area.min.x + g.area.max.x - x } else { x };
    let align = if g.rtl { Align2::RIGHT_CENTER } else { Align2::LEFT_CENTER };
    let col = |f: u32| g.x0 + f.saturating_sub(1) as f32 * g.cell_w;
    // Rows keep the layers' banding: this row comes after the layer rows and the av rows.
    let base = st.doc.layers.len().max(1) + crate::timeline_av::rows(app);
    let band = if base.is_multiple_of(2) { t.panel } else { t.panel_darker };

    let y = g.top;
    let strip = Rect::from_min_size(pos2(g.area.min.x, y), vec2(g.area.width(), g.row_h));
    p.rect_filled(strip, 0.0, band);
    p.text(pos2(mx(g.area.min.x + 8.0), y + g.row_h / 2.0), align, tl!("Bones"), FontId::proportional(11.5), t.text);

    let cy = y + g.row_h / 2.0;
    let centres: Vec<Pos2> = sk.keys.keys().map(|f| pos2(mx(col(*f) + g.cell_w / 2.0), cy)).collect();
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;
    use vectorcraft_engine::Session;
    use vectorcraft_engine::cmd::rig::{Joint, Pose, Rig, Skeleton};

    use crate::VectorcraftApp;

    fn arm() -> Skeleton {
        let j = |name: &str, parent, x| Joint { name: name.into(), parent, x, y: 0.0 };
        let joints = vec![j("shoulder", None, 0.0), j("elbow", Some(0), 100.0)];
        let keys = BTreeMap::from([(1, Pose { angles: vec![0.0, 0.0], ease: None }), (9, Pose { angles: vec![0.0, 90.0], ease: None })]);
        Skeleton { joints, keys }
    }

    /// An app with a 400 × 300 file and one rectangle on the first row, with `sk` stored on it.
    fn app_with_rig(sk: Option<Skeleton>) -> VectorcraftApp {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 400, "height": 300})).unwrap();
        app.session.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        app.session.execute("shape.rectangle", &json!({"x": 0, "y": -10, "width": 200, "height": 20})).unwrap();
        if let Some(sk) = sk {
            let row = app.session.active().unwrap().doc.layers[0].id.0;
            let rig = Rig { skeletons: BTreeMap::from([(row, sk)]) };
            app.session
                .edit("Rig", |d, _| {
                    rig.store(d);
                    Ok(())
                })
                .unwrap();
        }
        app
    }

    #[test]
    fn rows_is_one_only_with_a_rig() {
        assert_eq!(super::rows(&VectorcraftApp::new(Session::new(), Default::default())), 0);
        assert_eq!(super::rows(&app_with_rig(None)), 0);
        assert_eq!(super::rows(&app_with_rig(Some(arm()))), 1);
    }
}
