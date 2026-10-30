//! Shape tweens (A4): when two keyframes hold unlike shapes, the motion tween morphs one into the
//! other. Driven through `Session::execute` like the commands a user runs.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{Value, json};
use vectorcraft_engine::Session;
use vectorcraft_engine::cmd::anim::Anim;
use vectorcraft_engine::doc::{Node, NodeId};
use vectorcraft_geom::Rect;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({})).unwrap();
    s
}

fn key_id(v: &Value) -> u64 {
    v["id"].as_u64().unwrap()
}

/// The shape of the first art object of keyframe `key`, as the document holds it.
fn art(s: &Session, key: u64) -> Node {
    let d = &s.doc().unwrap().doc;
    (*d.node(NodeId(key)).unwrap().children().unwrap()[0]).clone()
}

/// The tween layer's first object at the frame shown.
fn tweened(s: &Session) -> Node {
    let d = &s.doc().unwrap().doc;
    let ghosts = Anim::of(d).ghosts;
    assert_eq!(ghosts.len(), 1, "one tween layer at this frame");
    let g = d.node(NodeId(ghosts[0])).unwrap();
    (*g.children().unwrap()[0]).clone()
}

/// Every anchor, in-handle and out-handle of a path, as plain coordinates.
fn points(n: &Node) -> Vec<(f64, f64)> {
    n.path_data().unwrap().subpaths.iter().flat_map(|s| s.anchors.iter().flat_map(|k| [k.p, k.h_in, k.h_out])).map(|p| (p.x, p.y)).collect()
}

fn bounds(n: &Node) -> Rect {
    n.geometric_bounds().unwrap()
}

fn assert_finite_path(n: &Node) {
    let pts = points(n);
    assert!(!pts.is_empty(), "non-empty path");
    assert!(pts.iter().all(|(x, y)| x.is_finite() && y.is_finite()), "finite coordinates: {pts:?}");
    let b = bounds(n);
    assert!([b.x0, b.y0, b.x1, b.y1].iter().all(|v| v.is_finite()), "finite bounds: {b:?}");
}

/// `mid` lies between `a` and `b` on each edge (1 pt of slack for curve extents).
fn assert_between(mid: Rect, a: Rect, b: Rect) {
    let lo = |p: f64, q: f64| p.min(q) - 1.0;
    let hi = |p: f64, q: f64| p.max(q) + 1.0;
    assert!(mid.x0 >= lo(a.x0, b.x0) && mid.x0 <= hi(a.x0, b.x0), "x0 {mid:?} vs {a:?} {b:?}");
    assert!(mid.x1 >= lo(a.x1, b.x1) && mid.x1 <= hi(a.x1, b.x1), "x1 {mid:?} vs {a:?} {b:?}");
    assert!(mid.y0 >= lo(a.y0, b.y0) && mid.y0 <= hi(a.y0, b.y0), "y0 {mid:?} vs {a:?} {b:?}");
    assert!(mid.y1 >= lo(a.y1, b.y1) && mid.y1 <= hi(a.y1, b.y1), "y1 {mid:?} vs {a:?} {b:?}");
}

#[test]
fn a_square_morphs_into_a_circle_halfway_between_their_bounds() {
    let mut s = session();
    let k1 = key_id(&s.execute("anim.keyframe", &json!({"frame": 1})).unwrap());
    s.execute("shape.rectangle", &json!({"x": 0, "y": 10, "width": 20, "height": 20})).unwrap();
    let k9 = key_id(&s.execute("anim.blankKeyframe", &json!({"frame": 9})).unwrap());
    s.execute("shape.ellipse", &json!({"x": 100, "y": 60, "width": 40, "height": 40})).unwrap();
    s.execute("anim.tween", &json!({"frame": 1, "on": true})).unwrap();

    let square = art(&s, k1);
    let circle = art(&s, k9);
    let (sq_b, ci_b) = (bounds(&square), bounds(&circle));
    assert_eq!((sq_b.width(), ci_b.width()), (20.0, 40.0), "different sizes");

    s.execute("anim.goto", &json!({"frame": 5})).unwrap();
    let mid = tweened(&s);
    assert_finite_path(&mid);
    let mb = bounds(&mid);
    let half = Rect::new((sq_b.x0 + ci_b.x0) / 2.0, (sq_b.y0 + ci_b.y0) / 2.0, (sq_b.x1 + ci_b.x1) / 2.0, (sq_b.y1 + ci_b.y1) / 2.0);
    for (got, want, edge) in [(mb.x0, half.x0, "x0"), (mb.y0, half.y0, "y0"), (mb.x1, half.x1, "x1"), (mb.y1, half.y1, "y1")] {
        assert!((got - want).abs() <= 1.0, "{edge}: halfway {want}, got {got} (bounds {mb:?})");
    }

    // Neither endpoint's path: the morph's anchors and handles differ from both.
    let (mp, sp, cp) = (points(&mid), points(&square), points(&circle));
    assert_ne!(mp, sp, "not the square's path");
    assert_ne!(mp, cp, "not the circle's path");
    // The tween's art is the in-between, and the keyframes keep their own.
    assert_eq!(points(&art(&s, k1)), sp, "square keyframe unchanged");
    assert_eq!(points(&art(&s, k9)), cp, "circle keyframe unchanged");
}

#[test]
fn a_five_point_star_morphs_into_a_rectangle() {
    let mut s = session();
    let k1 = key_id(&s.execute("anim.keyframe", &json!({"frame": 1})).unwrap());
    s.execute("shape.star", &json!({"cx": 50, "cy": 50, "radius1": 30, "radius2": 12, "points": 5})).unwrap();
    let k9 = key_id(&s.execute("anim.blankKeyframe", &json!({"frame": 9})).unwrap());
    s.execute("shape.rectangle", &json!({"x": 200, "y": 100, "width": 40, "height": 20})).unwrap();
    s.execute("anim.tween", &json!({"frame": 1, "on": true})).unwrap();

    let star = art(&s, k1);
    let rect = art(&s, k9);
    assert_ne!(points(&star).len(), points(&rect).len(), "unlike anchor counts");

    s.execute("anim.goto", &json!({"frame": 5})).unwrap();
    let mid = tweened(&s);
    assert_finite_path(&mid);
    assert_between(bounds(&mid), bounds(&star), bounds(&rect));
    assert_ne!(points(&mid), points(&star));
    assert_ne!(points(&mid), points(&rect));
}

#[test]
fn the_tween_layer_is_only_there_between_keyframes() {
    let mut s = session();
    let k1 = key_id(&s.execute("anim.keyframe", &json!({"frame": 1})).unwrap());
    s.execute("shape.rectangle", &json!({"x": 0, "y": 10, "width": 20, "height": 20})).unwrap();
    let k9 = key_id(&s.execute("anim.blankKeyframe", &json!({"frame": 9})).unwrap());
    s.execute("shape.ellipse", &json!({"x": 100, "y": 60, "width": 40, "height": 40})).unwrap();
    s.execute("anim.tween", &json!({"frame": 1, "on": true})).unwrap();

    // Frame 1: the tween is at t = 0, so no in-between layer; the square itself shows.
    s.execute("anim.goto", &json!({"frame": 1})).unwrap();
    let d = &s.doc().unwrap().doc;
    assert!(Anim::of(d).ghosts.is_empty(), "t = 0 adds no in-between layer");
    assert!(d.node(NodeId(k1)).unwrap().visible, "square keyframe shows at frame 1");
    assert!(!d.node(NodeId(k9)).unwrap().visible, "circle keyframe hidden at frame 1");
    assert_eq!(bounds(&art(&s, k1)).width(), 20.0, "original square art at frame 1");

    // Frame 9: the circle keyframe holds, nothing tweened.
    s.execute("anim.goto", &json!({"frame": 9})).unwrap();
    let d = &s.doc().unwrap().doc;
    assert!(Anim::of(d).ghosts.is_empty(), "no tween past the last keyframe");
    assert!(d.node(NodeId(k9)).unwrap().visible, "circle keyframe shows at frame 9");
    assert!(!d.node(NodeId(k1)).unwrap().visible, "square keyframe hidden at frame 9");
    assert_eq!(bounds(&art(&s, k9)).width(), 40.0, "original circle art at frame 9");
}

#[test]
fn a_morph_exports_as_a_video_and_grabs_a_middle_frame() {
    let mut s = session();
    s.execute("anim.setFrames", &json!({"frames": 9})).unwrap();
    s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 0, "y": 10, "width": 20, "height": 20})).unwrap();
    s.execute("anim.blankKeyframe", &json!({"frame": 9})).unwrap();
    s.execute("shape.ellipse", &json!({"x": 100, "y": 60, "width": 40, "height": 40})).unwrap();
    s.execute("anim.tween", &json!({"frame": 1, "on": true})).unwrap();

    let dir = std::env::temp_dir().join(format!("7rrek-morph-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let video = dir.join("morph.webm");
    let r = s.execute("anim.exportVideo", &json!({"path": video.to_string_lossy(), "scale": 0.5})).unwrap();
    assert_eq!(r["frames"], 9);
    let bytes = std::fs::read(&video).unwrap();
    assert_eq!(&bytes[..4], &[0x1A, 0x45, 0xDF, 0xA3], "WebM EBML header");

    let ffmpeg_ok = std::process::Command::new("ffmpeg").arg("-version").output().map(|o| o.status.success()).unwrap_or(false);
    if !ffmpeg_ok {
        eprintln!("ffmpeg not on PATH: skipping the middle-frame PNG");
        std::fs::remove_dir_all(&dir).ok();
        return;
    }
    // Frame 5 of 9 is index 4 (0-based).
    let png = dir.join("morph-frame5.png");
    let status = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-i"])
        .arg(&video)
        .args(["-vf", "select=eq(n\\,4)", "-vframes", "1"])
        .arg(&png)
        .status()
        .unwrap();
    assert!(status.success(), "ffmpeg could not grab the middle frame");
    assert!(std::fs::metadata(&png).map(|m| m.len() > 0).unwrap_or(false), "PNG written");
    eprintln!("middle-frame PNG: {}", png.display());
}
