#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! P3 Retargeting and Starter Actions tests (`action.apply`, `action.list`, etc.).
//! Verifies:
//! 1. Full and Simple profile characters built with commands.
//! 2. Walk retargeting on both characters: frame 6 has arm_upper_L rotated on both, simple returns skipped for missing parts.
//! 3. Export 24-frame video of both succeeds and has size > 0.
//! 4. Old row-based action file still applies.
//! 5. Unknown character returns Err.
//! 6. All 5 starter actions (idle, blink, walk, turn_head, point) work properly.

use serde_json::{Value, json};
use vectorcraft_engine::Session;
use vectorcraft_engine::cmd::rig::Rig;

/// Helper to build a full-profile character conforming to plans/character-contract.md
fn build_full_character(s: &mut Session, name: &str, base_x: f64, base_y: f64) -> u64 {
    let row = s.execute("layer.new", &json!({"name": name})).unwrap()["id"].as_u64().unwrap();
    s.execute("layer.setCurrent", &json!({"id": row})).unwrap();

    // Keyframe with visible art
    s.execute("anim.keyframe", &json!({"frame": 1, "row": row})).unwrap();
    s.execute(
        "shape.rectangle",
        &json!({
            "x": base_x,
            "y": base_y,
            "width": 120.0,
            "height": 280.0
        }),
    )
    .unwrap();
    s.execute("paint.setFill", &json!({"color": "#ff3333"})).unwrap();

    // 15 required parts for full profile
    // 0: hips (root)
    s.execute("rig.addJoint", &json!({"row": row, "name": "hips", "x": base_x + 60.0, "y": base_y + 150.0})).unwrap();
    // 1: torso (parent: hips)
    s.execute("rig.addJoint", &json!({"row": row, "name": "torso", "parent": 0, "x": base_x + 60.0, "y": base_y + 100.0})).unwrap();
    // 2: neck (parent: torso)
    s.execute("rig.addJoint", &json!({"row": row, "name": "neck", "parent": 1, "x": base_x + 60.0, "y": base_y + 60.0})).unwrap();
    // 3: head (parent: neck)
    s.execute("rig.addJoint", &json!({"row": row, "name": "head", "parent": 2, "x": base_x + 60.0, "y": base_y + 30.0})).unwrap();
    // 4: arm_upper_L (parent: torso)
    s.execute("rig.addJoint", &json!({"row": row, "name": "arm_upper_L", "parent": 1, "x": base_x + 90.0, "y": base_y + 80.0})).unwrap();
    // 5: arm_lower_L (parent: arm_upper_L)
    s.execute("rig.addJoint", &json!({"row": row, "name": "arm_lower_L", "parent": 4, "x": base_x + 110.0, "y": base_y + 120.0})).unwrap();
    // 6: hand_L (parent: arm_lower_L)
    s.execute("rig.addJoint", &json!({"row": row, "name": "hand_L", "parent": 5, "x": base_x + 120.0, "y": base_y + 140.0})).unwrap();
    // 7: arm_upper_R (parent: torso)
    s.execute("rig.addJoint", &json!({"row": row, "name": "arm_upper_R", "parent": 1, "x": base_x + 30.0, "y": base_y + 80.0})).unwrap();
    // 8: arm_lower_R (parent: arm_upper_R)
    s.execute("rig.addJoint", &json!({"row": row, "name": "arm_lower_R", "parent": 7, "x": base_x + 10.0, "y": base_y + 120.0})).unwrap();
    // 9: hand_R (parent: arm_lower_R)
    s.execute("rig.addJoint", &json!({"row": row, "name": "hand_R", "parent": 8, "x": base_x, "y": base_y + 140.0})).unwrap();
    // 10: leg_upper_L (parent: hips)
    s.execute("rig.addJoint", &json!({"row": row, "name": "leg_upper_L", "parent": 0, "x": base_x + 80.0, "y": base_y + 180.0})).unwrap();
    // 11: leg_lower_L (parent: leg_upper_L)
    s.execute("rig.addJoint", &json!({"row": row, "name": "leg_lower_L", "parent": 10, "x": base_x + 80.0, "y": base_y + 220.0})).unwrap();
    // 12: foot_L (parent: leg_lower_L)
    s.execute("rig.addJoint", &json!({"row": row, "name": "foot_L", "parent": 11, "x": base_x + 90.0, "y": base_y + 260.0})).unwrap();
    // 13: leg_upper_R (parent: hips)
    s.execute("rig.addJoint", &json!({"row": row, "name": "leg_upper_R", "parent": 0, "x": base_x + 40.0, "y": base_y + 180.0})).unwrap();
    // 14: leg_lower_R (parent: leg_upper_R)
    s.execute("rig.addJoint", &json!({"row": row, "name": "leg_lower_R", "parent": 13, "x": base_x + 40.0, "y": base_y + 220.0})).unwrap();
    // 15: foot_R (parent: leg_lower_R)
    s.execute("rig.addJoint", &json!({"row": row, "name": "foot_R", "parent": 14, "x": base_x + 30.0, "y": base_y + 260.0})).unwrap();

    row
}

/// Helper to build a simple-profile character conforming to plans/character-contract.md
/// Simple profile: torso, head, hand_L, hand_R, foot_L, foot_R.
/// Also has arm_upper_L so walk retargeting exercises it. Missing hips, legs, etc.
fn build_simple_character(s: &mut Session, name: &str, base_x: f64, base_y: f64) -> u64 {
    let row = s.execute("layer.new", &json!({"name": name})).unwrap()["id"].as_u64().unwrap();
    s.execute("layer.setCurrent", &json!({"id": row})).unwrap();

    // Keyframe with visible art (different size: 60x100)
    s.execute("anim.keyframe", &json!({"frame": 1, "row": row})).unwrap();
    s.execute(
        "shape.rectangle",
        &json!({
            "x": base_x,
            "y": base_y,
            "width": 60.0,
            "height": 100.0
        }),
    )
    .unwrap();
    s.execute("paint.setFill", &json!({"color": "#3333ff"})).unwrap();

    // 0: torso (root for simple profile)
    s.execute("rig.addJoint", &json!({"row": row, "name": "torso", "x": base_x + 30.0, "y": base_y + 40.0})).unwrap();
    // 1: head (parent: torso)
    s.execute("rig.addJoint", &json!({"row": row, "name": "head", "parent": 0, "x": base_x + 30.0, "y": base_y + 15.0})).unwrap();
    // 2: arm_upper_L (parent: torso)
    s.execute("rig.addJoint", &json!({"row": row, "name": "arm_upper_L", "parent": 0, "x": base_x + 50.0, "y": base_y + 35.0})).unwrap();
    // 3: hand_L (parent: arm_upper_L)
    s.execute("rig.addJoint", &json!({"row": row, "name": "hand_L", "parent": 2, "x": base_x + 65.0, "y": base_y + 50.0})).unwrap();
    // 4: hand_R (parent: torso)
    s.execute("rig.addJoint", &json!({"row": row, "name": "hand_R", "parent": 0, "x": base_x + 5.0, "y": base_y + 50.0})).unwrap();
    // 5: foot_L (parent: torso)
    s.execute("rig.addJoint", &json!({"row": row, "name": "foot_L", "parent": 0, "x": base_x + 45.0, "y": base_y + 90.0})).unwrap();
    // 6: foot_R (parent: torso)
    s.execute("rig.addJoint", &json!({"row": row, "name": "foot_R", "parent": 0, "x": base_x + 15.0, "y": base_y + 90.0})).unwrap();

    row
}

#[test]
fn retarget_walk_on_full_and_simple_characters_and_export_video() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800.0, "height": 600.0})).unwrap();

    let full_row = build_full_character(&mut s, "char_full", 100.0, 100.0);
    let simple_row = build_simple_character(&mut s, "char_simple", 450.0, 150.0);

    // Apply walk to full character
    let res_full = s
        .execute(
            "action.apply",
            &json!({
                "character": "char_full",
                "action": "walk",
                "at": 1
            }),
        )
        .unwrap();
    assert_eq!(res_full["character"], "char_full");
    assert_eq!(res_full["action"], "walk");
    let full_skipped = res_full["skipped"].as_array().unwrap();
    assert!(full_skipped.is_empty(), "char_full should have no missing parts, got {full_skipped:?}");

    // Apply walk to simple character
    let res_simple = s
        .execute(
            "action.apply",
            &json!({
                "character": "char_simple",
                "action": "walk",
                "at": 1
            }),
        )
        .unwrap();
    assert_eq!(res_simple["character"], "char_simple");
    assert_eq!(res_simple["action"], "walk");
    let simple_skipped: Vec<&str> = res_simple["skipped"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    assert!(!simple_skipped.is_empty(), "char_simple must return skipped for missing parts");
    assert!(simple_skipped.contains(&"leg_upper_L"), "expected leg_upper_L in skipped: {simple_skipped:?}");
    assert!(simple_skipped.contains(&"hips"), "expected hips in skipped: {simple_skipped:?}");

    // Verify frame 6 has arm_upper_L moved in both characters
    let doc = &s.doc().unwrap().doc;
    let rig = Rig::of(doc);

    let sk_full = rig.skeletons.get(&full_row).expect("char_full skeleton");
    let arm_l_full = sk_full.joints.iter().position(|j| j.name == "arm_upper_L").unwrap();
    let pose6_full = sk_full.pose_at(6).expect("pose at frame 6 for char_full");
    assert!(pose6_full[arm_l_full].abs() > 0.01, "arm_upper_L must be moved on full character, got {}", pose6_full[arm_l_full]);

    let sk_simple = rig.skeletons.get(&simple_row).expect("char_simple skeleton");
    let arm_l_simple = sk_simple.joints.iter().position(|j| j.name == "arm_upper_L").unwrap();
    let pose6_simple = sk_simple.pose_at(6).expect("pose at frame 6 for char_simple");
    assert!(pose6_simple[arm_l_simple].abs() > 0.01, "arm_upper_L must be moved on simple character, got {}", pose6_simple[arm_l_simple]);

    // Export 24-frame video of both characters
    let dir = std::env::temp_dir().join(format!("7rrek-p3-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let video_path = dir.join("two_chars_walk.mp4");
    let path_str = video_path.to_str().unwrap();

    let exp = s
        .execute(
            "anim.exportVideo",
            &json!({
                "path": path_str,
                "format": "mp4",
                "from": 1,
                "to": 24,
                "fps": 24,
                "scale": 0.5
            }),
        )
        .unwrap();
    assert_eq!(exp["frames"], 24);

    let meta = std::fs::metadata(&video_path).expect("exported mp4 exists");
    assert!(meta.len() > 0, "video size must be > 0 bytes, got {}", meta.len());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn old_row_based_action_file_still_applies() {
    let mut s = Session::new();
    s.execute("file.new", &json!({})).unwrap();

    let row1 = s.doc().unwrap().doc.layers[0].id.0;
    s.execute("object.setProps", &json!({"id": row1, "name": "source_char"})).unwrap();
    s.execute("rig.addJoint", &json!({"row": row1, "name": "arm", "x": 10.0, "y": 10.0})).unwrap();
    s.execute("rig.addJoint", &json!({"row": row1, "name": "forearm", "parent": 0, "x": 30.0, "y": 10.0})).unwrap();
    s.execute("rig.pose", &json!({"row": row1, "joint": 0, "angle": 35.0, "frame": 1})).unwrap();
    s.execute("rig.pose", &json!({"row": row1, "joint": 1, "angle": 45.0, "frame": 5})).unwrap();

    // Save as old-style action (with row)
    s.execute("action.save", &json!({"row": row1, "from": 1, "to": 5, "name": "old_action"})).unwrap();

    // Create target character row
    let row2 = s.execute("layer.new", &json!({"name": "target_char"})).unwrap()["id"].as_u64().unwrap();
    s.execute("rig.addJoint", &json!({"row": row2, "name": "arm", "x": 100.0, "y": 100.0})).unwrap();
    s.execute("rig.addJoint", &json!({"row": row2, "name": "forearm", "parent": 0, "x": 130.0, "y": 100.0})).unwrap();

    // Apply old action using action.apply
    let res = s
        .execute(
            "action.apply",
            &json!({
                "character": "target_char",
                "action": "old_action",
                "at": 1
            }),
        )
        .unwrap();
    assert_eq!(res["character"], "target_char");
    assert!(res["skipped"].as_array().unwrap().is_empty());

    let doc = &s.doc().unwrap().doc;
    let rig = Rig::of(doc);
    let sk2 = rig.skeletons.get(&row2).unwrap();
    assert_eq!(sk2.pose_at(1).unwrap()[0], 35.0);
    assert_eq!(sk2.pose_at(5).unwrap()[1], 45.0);
}

#[test]
fn wrong_character_returns_err() {
    let mut s = Session::new();
    s.execute("file.new", &json!({})).unwrap();

    let res = s.execute(
        "action.apply",
        &json!({
            "character": "nonexistent_character_name",
            "action": "walk"
        }),
    );
    assert!(res.is_err(), "unknown character must return Err");

    let err_msg = res.unwrap_err().to_string();
    assert!(err_msg.contains("nonexistent_character_name"), "err message should state character: {err_msg}");
}

#[test]
fn action_list_returns_builtins_plus_saved() {
    let mut s = Session::new();
    s.execute("file.new", &json!({})).unwrap();

    // Before saving any, list has the 5 builtins
    let res = s.execute("action.list", &json!({})).unwrap();
    let actions: Vec<String> = res["actions"].as_array().unwrap().iter().filter_map(|a| a["name"].as_str().map(str::to_string)).collect();

    assert!(actions.contains(&"walk".to_string()));
    assert!(actions.contains(&"blink".to_string()));
    assert!(actions.contains(&"turn_head".to_string()));
    assert!(actions.contains(&"point".to_string()));
    assert!(actions.contains(&"idle".to_string()));

    // Save a custom action
    let row = s.doc().unwrap().doc.layers[0].id.0;
    s.execute("action.save", &json!({"row": row, "from": 1, "to": 4, "name": "custom_act"})).unwrap();

    let res2 = s.execute("action.list", &json!({})).unwrap();
    let actions2: Vec<String> = res2["actions"].as_array().unwrap().iter().filter_map(|a| a["name"].as_str().map(str::to_string)).collect();

    assert!(actions2.contains(&"custom_act".to_string()));
    assert!(actions2.contains(&"walk".to_string()));
}

#[test]
fn starter_actions_blink_turnhead_point_idle() {
    let mut s = Session::new();
    s.execute("file.new", &json!({})).unwrap();

    let row = build_full_character(&mut s, "hero", 100.0, 100.0);

    // 1. turn_head (head turns 20°)
    s.execute("action.apply", &json!({"character": "hero", "action": "turn_head", "at": 1})).unwrap();
    let doc = &s.doc().unwrap().doc;
    let rig = Rig::of(doc);
    let sk = rig.skeletons.get(&row).unwrap();
    let head_idx = sk.joints.iter().position(|j| j.name == "head").unwrap();
    let head_angle = sk.pose_at(7).unwrap()[head_idx];
    assert!((head_angle - 20.0).abs() < 1e-4, "head should rotate 20°, got {head_angle}");

    // 2. point (arm_upper_R + arm_lower_R)
    s.execute("action.apply", &json!({"character": "hero", "action": "point", "at": 1})).unwrap();
    let doc = &s.doc().unwrap().doc;
    let rig = Rig::of(doc);
    let sk = rig.skeletons.get(&row).unwrap();
    let r_upper = sk.joints.iter().position(|j| j.name == "arm_upper_R").unwrap();
    let r_lower = sk.joints.iter().position(|j| j.name == "arm_lower_R").unwrap();
    let pose = sk.pose_at(7).unwrap();
    assert!(pose[r_upper].abs() > 10.0, "arm_upper_R should be pointing");
    assert!(pose[r_lower].abs() > 10.0, "arm_lower_R should be pointing");

    // 3. idle (breath on torso)
    s.execute("action.apply", &json!({"character": "hero", "action": "idle", "at": 1})).unwrap();
    let doc = &s.doc().unwrap().doc;
    let rig = Rig::of(doc);
    let sk = rig.skeletons.get(&row).unwrap();
    let torso_idx = sk.joints.iter().position(|j| j.name == "torso").unwrap();
    let torso_pose = sk.pose_at(7).unwrap();
    assert!(torso_pose[torso_idx].abs() > 0.01, "torso should move during idle breath");

    // 4. blink (6 frames with eye_*.closed)
    // Add eye drawings to hero
    s.execute("layer.setCurrent", &json!({"id": row})).unwrap();
    let e_open = s.execute("shape.rectangle", &json!({"x": 160.0, "y": 70.0, "width": 10.0, "height": 10.0})).unwrap()["id"].as_u64().unwrap();
    s.execute("object.setProps", &json!({"id": e_open, "name": "eye_L.open"})).unwrap();
    let e_closed = s.execute("shape.rectangle", &json!({"x": 160.0, "y": 70.0, "width": 10.0, "height": 2.0})).unwrap()["id"].as_u64().unwrap();
    s.execute("object.setProps", &json!({"id": e_closed, "name": "eye_L.closed"})).unwrap();

    let res_blink = s.execute("action.apply", &json!({"character": "hero", "action": "blink", "at": 1})).unwrap();
    assert_eq!(res_blink["action"], "blink");
}
