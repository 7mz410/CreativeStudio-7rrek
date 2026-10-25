//! 7rrek (Creative Studio): the current layer's bones drawn over the canvas.

use egui::{Pos2, Stroke};
use vectorcraft_doc::{Document, NodeId};
use vectorcraft_engine::cmd::anim::Anim;
use vectorcraft_engine::cmd::rig::{Rig, Skeleton};
use vectorcraft_geom::Point;

use crate::VectorcraftApp;
use crate::canvas::Xf;
use crate::theme::Tokens;

/// The top-level layer (timeline row) holding `id`, the way the animation commands find it.
fn row_of(doc: &Document, mut id: NodeId) -> Option<NodeId> {
    while let Some(p) = doc.parent_of(id) {
        id = p;
    }
    doc.layers.iter().any(|l| l.id == id).then_some(id)
}

/// The skeleton of the current layer's row, and the frame shown. None without a rig.
fn shown(app: &VectorcraftApp) -> Option<(Skeleton, u32)> {
    let st = app.session.active()?;
    let row = row_of(&st.doc, st.current_layer()?)?;
    let sk = Rig::of(&st.doc).skeletons.remove(&row.0)?;
    (!sk.joints.is_empty()).then(|| (sk, Anim::of(&st.doc).current))
}

/// Each joint's screen spot at `frame`: its rest spot turned by its posed transform (rest pose
/// without keys).
fn screen_joints(sk: &Skeleton, frame: u32, xf: &Xf) -> Vec<Pos2> {
    let angles = sk.pose_at(frame).unwrap_or_else(|| vec![0.0; sk.joints.len()]);
    sk.matrices(&angles).iter().zip(&sk.joints).map(|(m, j)| xf.to_screen(*m * Point::new(j.x, j.y))).collect()
}

/// The bones as screen segments (parent joint, child joint), dropping any with a non-finite end.
fn screen_bones(sk: &Skeleton, joints: &[Pos2]) -> Vec<[Pos2; 2]> {
    sk.joints
        .iter()
        .enumerate()
        .filter_map(|(c, j)| j.parent.map(|p| (p, c)))
        .filter_map(|(p, c)| {
            let (a, b) = (*joints.get(p)?, *joints.get(c)?);
            (a.is_finite() && b.is_finite()).then_some([a, b])
        })
        .collect()
}

/// Draw the current layer's bones and joints. Nothing is drawn without a rig on the row.
// shortcut: always shown; add View › Show Bones when users ask
pub fn paint(app: &VectorcraftApp, p: &egui::Painter, xf: &Xf) {
    let Some((sk, frame)) = shown(app) else { return };
    let t = Tokens::get(p.ctx());
    let joints = screen_joints(&sk, frame, xf);
    for [a, b] in screen_bones(&sk, &joints) {
        p.line_segment([a, b], Stroke::new(6.0, t.measure_bg));
        p.line_segment([a, b], Stroke::new(3.0, t.selection));
    }
    for (i, at) in joints.iter().enumerate().filter(|(_, at)| at.is_finite()) {
        let r = if sk.joints.get(i).is_some_and(|j| j.parent.is_none()) { 5.0 } else { 3.5 };
        p.circle_filled(*at, r, t.selection);
        p.circle_stroke(*at, r, Stroke::new(1.5, t.measure_bg));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use egui::{Rect, pos2, vec2};
    use serde_json::json;
    use vectorcraft_engine::Session;
    use vectorcraft_engine::cmd::rig::{Joint, Pose, Rig, Skeleton};
    use vectorcraft_geom::Point;

    use super::*;

    /// Shoulder at 0, elbow at 100, hand at 200, along x. Frame 1 holds the rest pose, frame 9 bends
    /// the elbow a quarter turn.
    fn arm() -> Skeleton {
        let j = |name: &str, parent, x| Joint { name: name.into(), parent, x, y: 0.0 };
        let joints = vec![j("shoulder", None, 0.0), j("elbow", Some(0), 100.0), j("hand", Some(1), 200.0)];
        let keys = BTreeMap::from([(1, Pose { angles: vec![0.0, 0.0, 0.0], ease: None }), (9, Pose { angles: vec![0.0, 90.0, 0.0], ease: None })]);
        Skeleton { joints, keys }
    }

    fn xf() -> Xf {
        Xf { rect: Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)), zoom: 1.0, center: Point::new(0.0, 0.0), rot: 0.0 }
    }

    /// An app with a 400 × 300 file, one rectangle on the first row, and `sk` stored on that row.
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

    /// One headless frame of the painter, with the overlay drawn into it.
    fn paint_once(app: &VectorcraftApp) {
        let ctx = egui::Context::default();
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))), ..Default::default() };
        let x = xf();
        ctx.run_ui(raw, |ui| paint(app, ui.painter(), &x)).textures_delta.clear();
    }

    #[test]
    fn paint_is_quiet_without_a_document_or_a_rig() {
        paint_once(&VectorcraftApp::new(Session::new(), Default::default()));
        let plain = app_with_rig(None);
        assert!(shown(&plain).is_none());
        paint_once(&plain);
    }

    #[test]
    fn paint_draws_a_rigged_row_without_panicking() {
        let app = app_with_rig(Some(arm()));
        assert!(shown(&app).is_some());
        paint_once(&app);
    }

    #[test]
    fn the_bones_turn_with_the_pose_at_the_frame() {
        let sk = arm();
        let x = xf();
        let rest = screen_bones(&sk, &screen_joints(&sk, 1, &x));
        assert_eq!(rest.len(), 2, "two bones for three joints");
        assert!((rest[1][1].y - rest[1][0].y).abs() < 1e-3, "forearm straight at rest: {rest:?}");

        let bent = screen_bones(&sk, &screen_joints(&sk, 9, &x));
        assert_eq!(bent.len(), 2);
        // The upper arm stays put; the forearm turns a quarter about the elbow at (100, 0): the
        // hand moves from (200, 0) to (100, 100).
        assert_eq!(bent[0], rest[0]);
        let want = [x.to_screen(Point::new(100.0, 0.0)), x.to_screen(Point::new(100.0, 100.0))];
        for (got, want) in bent[1].iter().zip(want) {
            assert!((*got - want).length() < 0.5, "elbow→hand {bent:?}, want {want:?}");
        }
    }

    #[test]
    fn a_non_finite_joint_drops_its_bones() {
        let sk = arm();
        let joints = [pos2(0.0, 0.0), pos2(f32::NAN, 0.0), pos2(2.0, 0.0)];
        assert!(screen_bones(&sk, &joints).is_empty());
    }

    /// Writes the canvas with the rig to `$RIG_PNG` when set (a visual check, not a gate).
    #[test]
    fn the_rig_over_the_canvas_png() {
        let Some(path) = std::env::var_os("RIG_PNG") else { return };
        let mut app = app_with_rig(Some(arm()));
        app.ui.view.edges = true;
        let mut h = egui_kittest::Harness::builder()
            .with_size(vec2(800.0, 600.0))
            .wgpu()
            .build_ui_state(|ui, app: &mut VectorcraftApp| crate::canvas::show(app, ui), app);
        h.run_steps(4);
        h.render().unwrap().save(std::path::PathBuf::from(path)).unwrap();
    }
}
