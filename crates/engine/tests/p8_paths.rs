//! Phase 8 (G2): motion paths and timing charts.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;
use vectorcraft_doc::NodeId;
use vectorcraft_engine::Session;
use vectorcraft_engine::cmd::anim::Anim;
use vectorcraft_geom::arc::ArcPath;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({})).unwrap();
    s
}

/// The art bounds of the row at the current frame shown.
fn row_center(s: &Session, row: NodeId) -> vectorcraft_geom::Point {
    shown_rect(s, row).geometric_bounds().unwrap().center()
}

/// The first object's path inside the row at the frame shown (e.g. inside tween ghost).
fn shown_rect(s: &Session, row: NodeId) -> vectorcraft_doc::Node {
    let d = &s.doc().unwrap().doc;
    let anim = Anim::of(d);
    if let Some(&ghost_id) = anim.ghosts.first()
        && let Some(g) = d.node(NodeId(ghost_id))
    {
        if let Some(first) = g.children().and_then(|c| c.first()) {
            return (**first).clone();
        }
        return g.clone();
    }
    let row_node = d.node(row).unwrap();
    let vis = row_node.children().unwrap().iter().find(|c| c.visible).unwrap();
    if let Some(first) = vis.children().and_then(|c| c.first()) {
        return (**first).clone();
    }
    (**vis).clone()
}

#[test]
fn rectangle_on_arc_at_mid_frame_is_at_half_length_linear_and_not_ease_in() {
    let mut s = session();

    // Row 1: animate a rectangle from frame 1 to frame 9
    let row1 = s.doc().unwrap().active_layer.unwrap();
    s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 0.0, "y": 0.0, "width": 20.0, "height": 20.0})).unwrap();
    s.execute("anim.keyframe", &json!({"frame": 9})).unwrap();
    s.execute("anim.tween", &json!({"frame": 1, "on": true})).unwrap();

    // Add another layer for the motion path
    s.execute("layer.new", &json!({"name": "PathLayer"})).unwrap();
    let arc_res = s.execute("shape.arc", &json!({"x1": 0.0, "y1": 0.0, "x2": 200.0, "y2": 150.0, "slope": 50.0})).unwrap();
    let arc_id = arc_res["id"].as_u64().unwrap();

    // Assign motion path to row 1
    s.execute("motionPath.set", &json!({"row": row1.0, "path": arc_id, "orient": false})).unwrap();

    let arc_path = {
        let d = &s.doc().unwrap().doc;
        let arc_node = d.node(NodeId(arc_id)).unwrap();
        ArcPath::new(arc_node.path_data().unwrap())
    };
    let half_pt = arc_path.at(0.5 * arc_path.len()).0;

    // Linear ease: frame 5 (halfway between 1 and 9) should land within 1 pt of halfway along the path
    s.execute("anim.goto", &json!({"frame": 5})).unwrap();
    let center_linear = row_center(&s, row1);
    assert!(
        (center_linear.x - half_pt.x).abs() <= 1.0 && (center_linear.y - half_pt.y).abs() <= 1.0,
        "linear mid frame should be within 1 pt of path midpoint: got {:?}, want {:?}",
        center_linear,
        half_pt
    );

    // easeIn: frame 5 should NOT be at the midpoint (eased progression lags behind)
    s.execute("anim.tween", &json!({"row": row1.0, "frame": 1, "ease": "easeIn"})).unwrap();
    s.execute("anim.goto", &json!({"frame": 5})).unwrap();
    let center_ease_in = row_center(&s, row1);
    let dist_from_mid = (center_ease_in - half_pt).hypot();
    assert!(dist_from_mid > 2.0, "easeIn mid frame should not be at midpoint: distance from mid is {dist_from_mid}");
}

#[test]
fn orient_turns_rectangle_to_match_tangent_angle_at_mid_arc() {
    let mut s = session();

    let row1 = s.doc().unwrap().active_layer.unwrap();
    s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 0.0, "y": 0.0, "width": 40.0, "height": 20.0})).unwrap();
    s.execute("anim.keyframe", &json!({"frame": 9})).unwrap();
    s.execute("anim.tween", &json!({"frame": 1, "on": true})).unwrap();

    s.execute("layer.new", &json!({"name": "PathLayer"})).unwrap();
    let arc_res = s.execute("shape.arc", &json!({"x1": 0.0, "y1": 0.0, "x2": 200.0, "y2": 150.0, "slope": 50.0})).unwrap();
    let arc_id = arc_res["id"].as_u64().unwrap();

    // Set motion path with orient = true
    s.execute("motionPath.set", &json!({"row": row1.0, "path": arc_id, "orient": true})).unwrap();

    let arc_path = {
        let d = &s.doc().unwrap().doc;
        let arc_node = d.node(NodeId(arc_id)).unwrap();
        ArcPath::new(arc_node.path_data().unwrap())
    };
    let (_, tangent) = arc_path.at(0.5 * arc_path.len());
    let want_angle = tangent.y.atan2(tangent.x).to_degrees();

    s.execute("anim.goto", &json!({"frame": 5})).unwrap();
    let rect_node = shown_rect(&s, row1);
    let sp = &rect_node.path_data().unwrap().subpaths[0].anchors;
    let v = sp[1].p - sp[0].p;
    let got_angle = v.y.atan2(v.x).to_degrees();

    let diff = (got_angle - want_angle).abs();
    assert!(diff < 1.0, "orient angle at mid arc should match tangent angle: got {got_angle}°, want {want_angle}° (diff {diff}°)");
}

#[test]
fn undo_restores_everything_and_deleted_path_returns_clear_err_not_panic() {
    let mut s = session();

    let row1 = s.doc().unwrap().active_layer.unwrap();
    s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 0.0, "y": 0.0, "width": 20.0, "height": 20.0})).unwrap();

    s.execute("layer.new", &json!({"name": "PathLayer"})).unwrap();
    let arc_res = s.execute("shape.arc", &json!({"x1": 0.0, "y1": 0.0, "x2": 100.0, "y2": 100.0})).unwrap();
    let arc_id = arc_res["id"].as_u64().unwrap();

    // Set motion path
    s.execute("motionPath.set", &json!({"row": row1.0, "path": arc_id, "orient": true})).unwrap();
    let info = s.execute("motionPath.info", &json!({"row": row1.0})).unwrap();
    assert_eq!(info["path"], arc_id);
    assert_eq!(info["orient"], true);

    // Undo restores document without motion path
    s.execute("edit.undo", &json!({})).unwrap();
    let info_after_undo = s.execute("motionPath.info", &json!({"row": row1.0}));
    assert!(info_after_undo.is_err(), "after undo, motion path should be gone");

    // Redo restores motion path
    s.execute("edit.redo", &json!({})).unwrap();
    assert!(s.execute("motionPath.info", &json!({"row": row1.0})).is_ok());

    // Delete the path node from the document
    s.execute("edit.clear", &json!({"ids": [arc_id]})).unwrap();

    // Calling motionPath.info on deleted path returns a clear Err, not a panic
    let err = s.execute("motionPath.info", &json!({"row": row1.0})).unwrap_err();
    let err_msg = err.to_string();
    assert!(err_msg.contains("deleted") || err_msg.contains("not found"), "expected clear error message, got: {err_msg}");

    // Playback / goto frame does not panic when path is missing
    s.execute("anim.goto", &json!({"frame": 3})).unwrap();

    // Setting a non-existent path id returns a clear Err
    let set_bad = s.execute("motionPath.set", &json!({"row": row1.0, "path": 999999}));
    assert!(set_bad.is_err());
}

#[test]
fn a_row_of_several_shapes_moves_as_one_piece() {
    let mut s = session();
    let row = s.doc().unwrap().active_layer.unwrap();
    s.execute("anim.setFrames", &json!({"frames": 9})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 0.0, "y": 0.0, "width": 20.0, "height": 20.0})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 100.0, "y": 0.0, "width": 20.0, "height": 20.0})).unwrap();
    s.execute("layer.new", &json!({"name": "PathLayer"})).unwrap();
    let arc = s.execute("shape.arc", &json!({"x1": 0.0, "y1": 300.0, "x2": 400.0, "y2": 300.0, "slope": 50.0})).unwrap()["id"].as_u64().unwrap();
    s.execute("motionPath.set", &json!({"row": row.0, "path": arc})).unwrap();
    s.execute("anim.goto", &json!({"frame": 5})).unwrap();
    let d = &s.doc().unwrap().doc;
    let shown: Vec<_> =
        d.node(row).unwrap().children().unwrap().iter().filter(|c| c.visible).map(|c| c.geometric_bounds().unwrap().center()).collect();
    assert_eq!(shown.len(), 2);
    // Moved, and still 100 pt apart.
    assert!(shown[0].y > 100.0, "{shown:?}");
    assert!(((shown[1] - shown[0]).hypot() - 100.0).abs() < 1e-6, "{shown:?}");
}
