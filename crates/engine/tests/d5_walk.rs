#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! D5: the walk plants its feet. Two walk cycles and a stop on a side-view character: the foot on
//! the ground never slides more than 1 pt from one frame to the next, it stays on the floor, and
//! the character travels by what the stride gives.

use serde_json::json;
use vectorcraft_engine::Session;
use vectorcraft_engine::cmd::peg::Pegs;
use vectorcraft_geom::Point;

/// (part, x, y, width, height, pivot): a character facing right, legs 200 pt long.
const PARTS: &[(&str, f64, f64, f64, f64, (f64, f64))] = &[
    ("hips", 180.0, 280.0, 40.0, 40.0, (200.0, 300.0)),
    ("torso", 170.0, 150.0, 60.0, 140.0, (200.0, 290.0)),
    ("head", 165.0, 80.0, 70.0, 70.0, (200.0, 150.0)),
    ("arm_upper_L", 190.0, 160.0, 20.0, 70.0, (200.0, 165.0)),
    ("arm_lower_L", 190.0, 230.0, 20.0, 60.0, (200.0, 232.0)),
    ("hand_L", 190.0, 290.0, 20.0, 20.0, (200.0, 292.0)),
    ("arm_upper_R", 190.0, 160.0, 20.0, 70.0, (200.0, 165.0)),
    ("arm_lower_R", 190.0, 230.0, 20.0, 60.0, (200.0, 232.0)),
    ("hand_R", 190.0, 290.0, 20.0, 20.0, (200.0, 292.0)),
    ("leg_upper_L", 190.0, 300.0, 20.0, 100.0, (200.0, 305.0)),
    ("leg_lower_L", 190.0, 400.0, 20.0, 100.0, (200.0, 402.0)),
    ("foot_L", 185.0, 500.0, 45.0, 15.0, (200.0, 502.0)),
    ("leg_upper_R", 190.0, 300.0, 20.0, 100.0, (200.0, 305.0)),
    ("leg_lower_R", 190.0, 400.0, 20.0, 100.0, (200.0, 402.0)),
    ("foot_R", 185.0, 500.0, 45.0, 15.0, (200.0, 502.0)),
];

fn contract_file(path: &std::path::Path) {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800.0, "height": 800.0})).unwrap();
    for v in ["front", "3q", "side"] {
        let l = s.execute("layer.new", &json!({"name": v})).unwrap()["id"].as_u64().unwrap();
        s.execute("layer.setCurrent", &json!({"id": l})).unwrap();
        for &(part, x, y, w, h, (px, py)) in PARTS {
            let art = s.execute("shape.rectangle", &json!({"x": x, "y": y, "width": w, "height": h})).unwrap()["id"].as_u64().unwrap();
            let pivot =
                s.execute("shape.ellipse", &json!({"x": px - 1.5, "y": py - 1.5, "width": 3.0, "height": 3.0})).unwrap()["id"].as_u64().unwrap();
            s.execute("object.setProps", &json!({"id": pivot, "name": "pivot"})).unwrap();
            s.execute("select.set", &json!({"ids": [art, pivot]})).unwrap();
            let g = s.execute("object.group", &json!({})).unwrap()["id"].as_u64().unwrap();
            s.execute("object.setProps", &json!({"id": g, "name": part})).unwrap();
        }
    }
    s.execute("document.save", &json!({"path": path})).unwrap();
}

#[test]
fn walk_plants_the_foot_on_the_ground_and_travels_by_its_stride() {
    let path = std::env::temp_dir().join(format!("d5-walk-{}.rsm", std::process::id()));
    contract_file(&path);
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800.0, "height": 800.0})).unwrap();
    s.execute("character.fromRsm", &json!({"path": path, "name": "boy", "view": "side"})).unwrap();
    let _ = std::fs::remove_file(&path);
    for at in [1, 25] {
        let r = s.execute("action.apply", &json!({"character": "boy", "action": "walk", "at": at})).unwrap();
        assert_eq!(r["skipped"], json!([]), "{r}");
    }
    s.execute("action.apply", &json!({"character": "boy", "action": "stop", "at": 49})).unwrap();

    let pegs = Pegs::of(&s.doc().unwrap().doc);
    let peg = |name: &str| pegs.pegs.iter().position(|p| p.name == name).unwrap();
    let feet = [peg("boy/foot_L"), peg("boy/foot_R")];
    let corners = [Point::new(185.0, 515.0), Point::new(230.0, 515.0)];
    let points = |f: u32| -> Vec<Point> {
        let m = pegs.matrices(f);
        feet.iter().flat_map(|&i| corners.iter().map(|&c| m[i] * c).collect::<Vec<_>>()).collect()
    };
    let lowest = |pts: &[Point]| (0..pts.len()).max_by(|&a, &b| pts[a].y.total_cmp(&pts[b].y)).unwrap();

    for f in 1..57 {
        let (now, next) = (points(f), points(f + 1));
        let i = lowest(&now);
        assert!((now[i].y - 515.0).abs() < 0.5, "frame {f}: the lowest foot point is off the floor at y {}", now[i].y);
        let slide = (next[i].x - now[i].x).abs();
        assert!(slide <= 1.0, "frame {f}→{}: the planted foot slid {slide:.2} pt", f + 1);
    }
    let travel = pegs.pegs[peg("boy")].key_at(48).x;
    assert!(travel > 100.0, "two cycles should carry the character forward, got {travel:.1} pt");
}
