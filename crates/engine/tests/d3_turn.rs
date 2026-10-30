#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! 7rrek: D3 tests for character turnaround views (front, 3q, side).

use std::path::Path;

use serde_json::json;
use vectorcraft_doc::NodeId;
use vectorcraft_engine::Session;
use vectorcraft_engine::cmd::anim::Anim;

fn contract_file(path: &Path) {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800.0, "height": 800.0})).unwrap();
    for v in ["front", "3q", "side"] {
        let l = s.execute("layer.new", &json!({"name": v})).unwrap()["id"].as_u64().unwrap();
        s.execute("layer.setCurrent", &json!({"id": l})).unwrap();
        for name in [
            "hips",
            "torso",
            "neck",
            "head",
            "eye_L",
            "eye_R",
            "brow_L",
            "brow_R",
            "ear_L",
            "ear_R",
            "nose",
            "mouth",
            "hair_front",
            "hair_back",
            "arm_upper_L",
            "arm_lower_L",
            "hand_L",
            "arm_upper_R",
            "arm_lower_R",
            "hand_R",
            "leg_upper_L",
            "leg_lower_L",
            "foot_L",
            "leg_upper_R",
            "leg_lower_R",
            "foot_R",
        ] {
            let mut ids =
                vec![s.execute("shape.rectangle", &json!({"x": 100.0, "y": 100.0, "width": 20.0, "height": 40.0})).unwrap()["id"].as_u64().unwrap()];
            if name == "mouth" && v == "front" {
                for m in ["A", "B", "C", "D", "E", "F", "G", "H", "X"] {
                    let a =
                        s.execute("shape.ellipse", &json!({"x": 100.0, "y": 100.0, "width": 10.0, "height": 5.0})).unwrap()["id"].as_u64().unwrap();
                    s.execute("object.setProps", &json!({"id": a, "name": format!("mouth.{m}")})).unwrap();
                    ids.push(a);
                }
            }
            if name == "eye_L" && v == "front" {
                for state in ["open", "closed", "half"] {
                    let a =
                        s.execute("shape.ellipse", &json!({"x": 100.0, "y": 100.0, "width": 5.0, "height": 5.0})).unwrap()["id"].as_u64().unwrap();
                    s.execute("object.setProps", &json!({"id": a, "name": format!("eye_L.{state}")})).unwrap();
                    ids.push(a);
                }
            }
            s.execute("select.set", &json!({"ids": ids})).unwrap();
            let g = s.execute("object.group", &json!({})).unwrap()["id"].as_u64().unwrap();
            s.execute("object.setProps", &json!({"id": g, "name": name})).unwrap();
        }
    }
    s.execute("document.save", &json!({"path": path})).unwrap();
}

fn tone(path: &Path) {
    let n = 48_000u32;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + n * 2).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&48_000u32.to_le_bytes());
    b.extend_from_slice(&(48_000u32 * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&(n * 2).to_le_bytes());
    for i in 0..n {
        let t = i as f32 / 48_000.0;
        let s = (t * 220.0 * 2.0 * std::f32::consts::PI).sin();
        b.extend_from_slice(&((s * 16384.0) as i16).to_le_bytes());
    }
    std::fs::write(path, b).unwrap();
}

fn test_dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "7rrek-d3-{}-{}-{}",
        std::process::id(),
        name,
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn character_from_rsm_with_views_loads_all_views_and_exposes_side_at_frame_1() {
    let dir = test_dir("t1");
    let rsm = dir.join("char.rsm");
    contract_file(&rsm);

    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800.0, "height": 800.0})).unwrap();
    let r = s
        .execute(
            "character.fromRsm",
            &json!({
                "path": rsm.to_str().unwrap(),
                "name": "hero",
                "views": ["side", "3q", "front"],
            }),
        )
        .unwrap();

    let head_row = NodeId(r["rows"]["head"].as_u64().unwrap());
    let mouth_row = NodeId(r["rows"]["mouth"].as_u64().unwrap());

    let d = &s.doc().unwrap().doc;
    let anim = Anim::of(d);

    // Row head has drawings: side, 3q, front
    let head_node = d.node(head_row).unwrap();
    let head_drawings: Vec<String> =
        anim.row_keys(head_node).into_iter().map(|(_, id)| d.node(id).unwrap().name.clone().unwrap_or_default()).collect();
    assert!(head_drawings.contains(&"side".to_string()));
    assert!(head_drawings.contains(&"3q".to_string()));
    assert!(head_drawings.contains(&"front".to_string()));

    // Shown at frame 1 on head is side
    let head_exp_1 = anim.exposures.get(&head_row.0).and_then(|e| e.get(&1)).copied().unwrap();
    let head_shown_name = d.node(NodeId(head_exp_1)).unwrap().name.as_deref().unwrap();
    assert_eq!(head_shown_name, "side");

    // Shown at frame 1 on mouth is side.X (borrowed default)
    let mouth_exp_1 = anim.exposures.get(&mouth_row.0).and_then(|e| e.get(&1)).copied().unwrap();
    let mouth_shown_name = d.node(NodeId(mouth_exp_1)).unwrap().name.as_deref().unwrap();
    assert_eq!(mouth_shown_name, "side.X");
}

#[test]
fn character_view_swaps_all_rows_at_frame_10_and_keeps_previous_view_at_frame_9() {
    let dir = test_dir("t2");
    let rsm = dir.join("char.rsm");
    contract_file(&rsm);

    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800.0, "height": 800.0})).unwrap();
    let r = s
        .execute(
            "character.fromRsm",
            &json!({
                "path": rsm.to_str().unwrap(),
                "name": "hero",
                "views": ["side", "3q", "front"],
            }),
        )
        .unwrap();

    let res = s
        .execute(
            "character.view",
            &json!({
                "character": "hero",
                "frame": 10,
                "view": "front",
            }),
        )
        .unwrap();
    assert_eq!(res["view"], "front");
    assert_eq!(res["frame"], 10);

    let d = &s.doc().unwrap().doc;
    let anim = Anim::of(d);

    let head_row = NodeId(r["rows"]["head"].as_u64().unwrap());
    let mouth_row = NodeId(r["rows"]["mouth"].as_u64().unwrap());

    // Active at frame 9 on head is side
    let head_node = d.node(head_row).unwrap();
    let head_at_9 = anim.row_frames(head_node).into_iter().rfind(|(f, _)| *f <= 9).map(|(_, id)| id).unwrap();
    assert_eq!(d.node(head_at_9).unwrap().name.as_deref().unwrap(), "side");

    // Active at frame 10 on head is front
    let head_at_10 = anim.row_frames(head_node).into_iter().rfind(|(f, _)| *f <= 10).map(|(_, id)| id).unwrap();
    assert_eq!(d.node(head_at_10).unwrap().name.as_deref().unwrap(), "front");

    // Active at frame 9 on mouth is side.X
    let mouth_node = d.node(mouth_row).unwrap();
    let mouth_at_9 = anim.row_frames(mouth_node).into_iter().rfind(|(f, _)| *f <= 9).map(|(_, id)| id).unwrap();
    assert_eq!(d.node(mouth_at_9).unwrap().name.as_deref().unwrap(), "side.X");

    // Active at frame 10 on mouth is front.X
    let mouth_at_10 = anim.row_frames(mouth_node).into_iter().rfind(|(f, _)| *f <= 10).map(|(_, id)| id).unwrap();
    assert_eq!(d.node(mouth_at_10).unwrap().name.as_deref().unwrap(), "front.X");

    // Check all character rows at frame 10 show front or front.<default>
    for layer in &d.layers {
        if layer.name.as_deref().is_some_and(|n| n.starts_with("hero/")) {
            let row_node = d.node(layer.id).unwrap();
            let at_10 = anim.row_frames(row_node).into_iter().rfind(|(f, _)| *f <= 10).map(|(_, id)| id).unwrap();
            let name_at_10 = d.node(at_10).unwrap().name.as_deref().unwrap();
            assert!(
                name_at_10 == "front" || name_at_10.starts_with("front."),
                "layer {} at frame 10 should show front, got {}",
                layer.name.as_deref().unwrap(),
                name_at_10
            );

            let at_9 = anim.row_frames(row_node).into_iter().rfind(|(f, _)| *f <= 9).map(|(_, id)| id).unwrap();
            let name_at_9 = d.node(at_9).unwrap().name.as_deref().unwrap();
            assert!(
                name_at_9 == "side" || name_at_9.starts_with("side."),
                "layer {} at frame 9 should show side, got {}",
                layer.name.as_deref().unwrap(),
                name_at_9
            );
        }
    }
}

#[test]
fn lip_sync_with_sound_after_frame_10_exposes_front_mouth_shapes() {
    let dir = test_dir("t3");
    let rsm = dir.join("char.rsm");
    let wav = dir.join("tone.wav");
    contract_file(&rsm);
    tone(&wav);

    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800.0, "height": 800.0})).unwrap();
    s.execute("scene.setup", &json!({"frames": 50})).unwrap();
    let r = s
        .execute(
            "character.fromRsm",
            &json!({
                "path": rsm.to_str().unwrap(),
                "name": "hero",
                "views": ["side", "3q", "front"],
            }),
        )
        .unwrap();

    let mouth_row = r["rows"]["mouth"].as_u64().unwrap();

    s.execute(
        "character.view",
        &json!({
            "character": "hero",
            "frame": 10,
            "view": "front",
        }),
    )
    .unwrap();

    let snd = s.execute("anim.soundImport", &json!({"path": wav.to_str().unwrap(), "start": 12})).unwrap();
    let ls = s.execute("anim.lipSync", &json!({"row": mouth_row, "sound": snd["index"]})).unwrap();
    assert!(!ls["keys"].as_array().unwrap().is_empty());

    let d = &s.doc().unwrap().doc;
    let anim = Anim::of(d);
    let mouth_node = d.node(NodeId(mouth_row)).unwrap();
    let frames = anim.row_frames(mouth_node);

    // Exposures for frames >= 12 must be front.<shape>, not side.<shape>
    let mut checked_any = false;
    for (f, drawing_id) in frames {
        if f >= 12 {
            let drawing_name = d.node(drawing_id).unwrap().name.as_deref().unwrap();
            assert!(drawing_name.starts_with("front."), "at frame {f}, mouth should show front.<shape>, got {drawing_name}");
            checked_any = true;
        }
    }
    assert!(checked_any);
}

#[test]
fn action_apply_blink_after_frame_10_exposes_front_closed() {
    let dir = test_dir("t4");
    let rsm = dir.join("char.rsm");
    contract_file(&rsm);

    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800.0, "height": 800.0})).unwrap();
    s.execute("scene.setup", &json!({"frames": 50})).unwrap();
    let r = s
        .execute(
            "character.fromRsm",
            &json!({
                "path": rsm.to_str().unwrap(),
                "name": "hero",
                "views": ["side", "3q", "front"],
            }),
        )
        .unwrap();

    let eye_row = NodeId(r["rows"]["eye_L"].as_u64().unwrap());

    s.execute(
        "character.view",
        &json!({
            "character": "hero",
            "frame": 10,
            "view": "front",
        }),
    )
    .unwrap();

    let app = s.execute("action.apply", &json!({"character": "hero", "action": "blink", "at": 15})).unwrap();
    assert!(!app["skipped"].as_array().unwrap().contains(&json!("drawing:eye_L.closed")));

    let d = &s.doc().unwrap().doc;
    let anim = Anim::of(d);
    let eye_node = d.node(eye_row).unwrap();
    let frames = anim.row_frames(eye_node);

    let closed_drawing = frames.iter().find(|(f, id)| *f >= 15 && d.node(*id).unwrap().name.as_deref() == Some("front.closed"));
    assert!(closed_drawing.is_some(), "eye_L should take front.closed after frame 10");
}

#[test]
fn scene_build_with_views_and_turns_succeeds_and_undo_reverts_everything() {
    let dir = test_dir("t5");
    let rsm = dir.join("char.rsm");
    let wav = dir.join("tone.wav");
    contract_file(&rsm);
    tone(&wav);

    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800.0, "height": 800.0})).unwrap();
    let layers_before = s.doc().unwrap().doc.layers.len();

    let spec = json!({
        "frames": 24,
        "characters": [
            {
                "name": "hero",
                "file": rsm.to_str().unwrap(),
                "views": ["side", "3q", "front"],
                "x": 10.0,
                "y": 20.0,
            }
        ],
        "actions": [
            {
                "character": "hero",
                "action": "idle",
                "at": 1,
            }
        ],
        "turns": [
            {
                "character": "hero",
                "frame": 10,
                "view": "front",
            }
        ],
        "moves": [
            {
                "character": "hero",
                "from": 1,
                "to": 5,
                "dx": 5.0,
                "dy": 0.0,
            }
        ],
        "dialogue": [
            {
                "character": "hero",
                "sound": wav.to_str().unwrap(),
                "at": 12,
                "text": "Hello world",
            }
        ],
    });

    let res = s.execute("scene.build", &json!({"spec": spec})).unwrap();
    assert!(res["characters"]["hero"].as_u64().is_some());

    let d = &s.doc().unwrap().doc;
    let cv = vectorcraft_engine::cmd::character_rig::CharacterViews::of(d);
    assert_eq!(cv.view_at("hero", 1), Some("side"));
    assert_eq!(cv.view_at("hero", 9), Some("side"));
    assert_eq!(cv.view_at("hero", 10), Some("front"));

    // Single undo restores everything
    s.execute("edit.undo", &json!({})).unwrap();
    let d_after = &s.doc().unwrap().doc;
    assert_eq!(d_after.layers.len(), layers_before);
    let cv_after = vectorcraft_engine::cmd::character_rig::CharacterViews::of(d_after);
    assert!(cv_after.views.is_empty());
}

#[test]
fn without_views_character_rig_keeps_existing_behavior() {
    let dir = test_dir("t6");
    let rsm = dir.join("char.rsm");
    contract_file(&rsm);

    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800.0, "height": 800.0})).unwrap();
    let r = s
        .execute(
            "character.fromRsm",
            &json!({
                "path": rsm.to_str().unwrap(),
                "name": "hero",
                "view": "front",
            }),
        )
        .unwrap();

    let mouth_row = NodeId(r["rows"]["mouth"].as_u64().unwrap());
    let d = &s.doc().unwrap().doc;
    let anim = Anim::of(d);
    let mouth_node = d.node(mouth_row).unwrap();
    let drawings: Vec<String> = anim.row_keys(mouth_node).into_iter().map(|(_, id)| d.node(id).unwrap().name.clone().unwrap_or_default()).collect();

    // In single-view mode, drawings are named A..H and X without view prefix
    assert!(drawings.contains(&"X".to_string()));
    assert!(drawings.contains(&"A".to_string()));
    assert!(!drawings.contains(&"front.X".to_string()));
}

#[test]
fn scene_build_blink_after_turn_takes_the_new_view() {
    let dir = test_dir("t7");
    let rsm = dir.join("char.rsm");
    contract_file(&rsm);

    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800.0, "height": 800.0})).unwrap();
    let spec = json!({
        "frames": 40,
        "characters": [{"name": "hero", "file": rsm.to_str().unwrap(), "views": ["side", "3q", "front"]}],
        "actions": [{"character": "hero", "action": "blink", "at": 20}],
        "turns": [{"character": "hero", "frame": 10, "view": "front"}],
    });
    s.execute("scene.build", &json!({"spec": spec})).unwrap();

    let d = &s.doc().unwrap().doc;
    let anim = Anim::of(d);
    let eye = d.layers.iter().find(|l| l.name.as_deref() == Some("hero/eye_L")).unwrap();
    let names: Vec<String> = anim.row_frames(eye).iter().filter(|(f, _)| *f >= 20).filter_map(|(_, id)| d.node(*id)?.name.clone()).collect();
    assert!(names.iter().any(|n| n == "front.closed"), "blink after the turn should use front drawings: {names:?}");
    assert!(!names.iter().any(|n| n.starts_with("side.")), "no side drawings after the turn: {names:?}");
}

#[test]
fn only_the_exposed_drawing_of_each_row_renders_before_any_turn() {
    let dir = test_dir("t-render");
    let rsm = dir.join("char.rsm");
    contract_file(&rsm);
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800.0, "height": 800.0})).unwrap();
    let r = s.execute("character.fromRsm", &json!({"path": rsm.to_str().unwrap(), "name": "hero", "views": ["side", "3q", "front"]})).unwrap();
    let d = vectorcraft_engine::cmd::anim::at_frame(&s.doc().unwrap().doc, 1);
    for (part, row) in r["rows"].as_object().unwrap() {
        let row = d.node(NodeId(row.as_u64().unwrap())).unwrap();
        let shown: Vec<String> =
            row.children().into_iter().flatten().filter(|c| c.is_layer() && c.visible).map(|c| c.name.clone().unwrap_or_default()).collect();
        assert!(shown.len() <= 1, "{part}: {shown:?} all show at frame 1");
    }
}
