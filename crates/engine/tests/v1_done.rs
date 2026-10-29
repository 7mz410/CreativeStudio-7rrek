#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! 7rrek v1 done-when checks. One test per finished item, each driving the commands end to end:
//! p1 character.check, p2 character.fromRsm + peg.key, p3 action.apply on two characters + video,
//! s6a anim.exportFrames (transparent PNGs), s6b scene.setup + frame export at 1280×720 / 30 fps.

use serde_json::json;
use vectorcraft_engine::Session;
use vectorcraft_engine::cmd::peg::Pegs;
use vectorcraft_geom::Point;

/// The parts a full character needs (character contract, required in full profile).
const FULL_PARTS: &[&str] = &[
    "hips",
    "torso",
    "head",
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
];

const VIEWS: &[&str] = &["front", "3q", "side"];

/// A contract file (.rsm) with every full part in each view, minus `skip`.
fn contract_file(path: &std::path::Path, skip: Option<&str>) {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800.0, "height": 800.0})).unwrap();
    for v in VIEWS {
        let l = s.execute("layer.new", &json!({"name": v})).unwrap()["id"].as_u64().unwrap();
        s.execute("layer.setCurrent", &json!({"id": l})).unwrap();
        for &part in FULL_PARTS.iter().filter(|p| Some(**p) != skip) {
            let (x, y, w, h) = match part {
                "arm_upper_L" => (100.0, 100.0, 20.0, 100.0),
                "arm_lower_L" => (100.0, 200.0, 20.0, 100.0),
                "hand_L" => (100.0, 300.0, 20.0, 30.0),
                "torso" => (130.0, 90.0, 100.0, 150.0),
                _ => (300.0, 300.0, 20.0, 20.0),
            };
            let id = s.execute("shape.rectangle", &json!({"x": x, "y": y, "width": w, "height": h})).unwrap()["id"].as_u64().unwrap();
            s.execute("select.set", &json!({"ids": [id]})).unwrap();
            let g = s.execute("object.group", &json!({})).unwrap()["id"].as_u64().unwrap();
            s.execute("object.setProps", &json!({"id": g, "name": part})).unwrap();
        }
    }
    s.execute("document.save", &json!({"path": path})).unwrap();
}

#[test]
fn p1_check() {
    let pid = std::process::id();
    let full = std::env::temp_dir().join(format!("v1-done-{pid}-p1-full.rsm"));
    let no_hand = std::env::temp_dir().join(format!("v1-done-{pid}-p1-nohand.rsm"));
    contract_file(&full, None);
    contract_file(&no_hand, Some("hand_L"));

    let mut s = Session::new();
    s.execute("file.new", &json!({})).unwrap();
    let ok = s.execute("character.check", &json!({"path": full})).unwrap();
    let bad = s.execute("character.check", &json!({"path": no_hand})).unwrap();
    let _ = std::fs::remove_file(&full);
    let _ = std::fs::remove_file(&no_hand);

    assert_eq!(ok["ok"], true, "{ok}");
    assert_eq!(bad["ok"], false, "{bad}");
    for v in VIEWS {
        assert!(bad["missing"][v].as_array().unwrap().contains(&json!("hand_L")), "{bad}");
    }
}

#[test]
fn p2_rig() {
    let path = std::env::temp_dir().join(format!("v1-done-{}-p2.rsm", std::process::id()));
    contract_file(&path, None);
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800.0, "height": 800.0})).unwrap();
    let r = s.execute("character.fromRsm", &json!({"path": path, "name": "ana"})).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(r["character"], "ana", "{r}");

    let arm = r["pegs"]["arm_upper_L"].as_u64().unwrap();
    let hand = r["pegs"]["hand_L"].as_u64().unwrap() as usize;
    s.execute("peg.key", &json!({"peg": arm, "frame": 1, "angle": 0.0})).unwrap();
    s.execute("peg.key", &json!({"peg": arm, "frame": 12, "angle": 30.0})).unwrap();

    let pegs = Pegs::of(&s.doc().unwrap().doc);
    let pivot = pegs.pegs[hand].pivot;
    let p = Point::new(pivot[0], pivot[1]);
    let moved_at_1 = pegs.matrices(1)[hand] * p;
    let moved_at_12 = pegs.matrices(12)[hand] * p;
    let shift = (moved_at_12.x - moved_at_1.x).hypot(moved_at_12.y - moved_at_1.y);
    assert!(shift > 1.0, "hand pivot moved only {shift} pt at frame 12");
}

#[test]
fn p3_retarget() {
    let pid = std::process::id();
    let path = std::env::temp_dir().join(format!("v1-done-{pid}-p3.rsm"));
    contract_file(&path, None);
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800.0, "height": 800.0})).unwrap();
    for name in ["ana", "bob"] {
        let r = s.execute("character.fromRsm", &json!({"path": path, "name": name})).unwrap();
        assert_eq!(r["character"], name, "{r}");
    }
    let _ = std::fs::remove_file(&path);

    for name in ["ana", "bob"] {
        let r = s.execute("action.apply", &json!({"character": name, "action": "walk", "at": 1})).unwrap();
        assert_eq!(r["skipped"], json!([]), "{r}");
    }

    let video = std::env::temp_dir().join(format!("v1-done-{pid}-p3.webm"));
    let r = s.execute("anim.exportVideo", &json!({"path": video, "scale": 0.25})).unwrap();
    let len = std::fs::metadata(&video).map(|m| m.len()).unwrap_or(0);
    let _ = std::fs::remove_file(&video);
    assert!(len > 0, "{r}");
}

#[test]
fn s6a_frames() {
    let dir = std::env::temp_dir().join(format!("v1-done-{}-s6a", std::process::id()));
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 100.0, "height": 100.0})).unwrap();
    s.execute("scene.setup", &json!({"frames": 3})).unwrap();
    s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 20.0, "y": 20.0, "width": 50.0, "height": 50.0})).unwrap();
    let r = s.execute("anim.exportFrames", &json!({"dir": dir})).unwrap();

    let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    let corner = image::open(dir.join("frame_0001.png")).map(|i| i.to_rgba8().get_pixel(0, 0).0[3]);
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(r["frames"], 3, "{r}");
    assert_eq!(names.len(), 3, "{names:?}");
    assert_eq!(corner.unwrap(), 0, "corner pixel should be transparent");
}

#[test]
fn s6b_setup() {
    let dir = std::env::temp_dir().join(format!("v1-done-{}-s6b", std::process::id()));
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 100.0, "height": 100.0})).unwrap();
    let info = s.execute("scene.setup", &json!({"width": 1280.0, "height": 720.0, "fps": 30})).unwrap();
    assert_eq!((info["width"].as_f64(), info["height"].as_f64(), info["fps"].as_u64()), (Some(1280.0), Some(720.0), Some(30)), "{info}");

    let r = s.execute("anim.exportFrames", &json!({"dir": dir, "from": 1, "to": 1})).unwrap();
    let img = image::open(dir.join("frame_0001.png"));
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!((r["width"].as_u64(), r["height"].as_u64()), (Some(1280), Some(720)), "{r}");
    let img = img.unwrap();
    assert_eq!((img.width(), img.height()), (1280, 720));
}
