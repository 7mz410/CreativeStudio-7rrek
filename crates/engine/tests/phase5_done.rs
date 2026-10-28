#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Phase 5 end-to-end Done-when criteria tests (C1 through C7).
//! Tests every requirement via engine commands and real PNG export (reading pixels from exported PNGs).

use std::collections::{BTreeSet, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};

use serde_json::{Value, json};
use vectorcraft_engine::Session;
use vectorcraft_testkit::fixtures::Fixture;

/// Helper to navigate to a frame, export the document to PNG via `document.export`, and decode to `RgbaImage`.
fn export_frame_png(s: &mut Session, frame: u32, background: &str) -> image::RgbaImage {
    s.execute("anim.goto", &json!({"frame": frame})).unwrap();
    let res = s.execute("document.export", &json!({"format": "png", "background": background})).unwrap();
    let b64_str = res["dataBase64"].as_str().expect("dataBase64 in export result");
    let bytes = vectorcraft_format::base64_decode(b64_str).expect("valid base64 bytes");
    image::load_from_memory(&bytes).expect("decode valid png").to_rgba8()
}

// ---------------------------------------------------------------------------
// C1: Render graph
// ---------------------------------------------------------------------------

/// C1: rewiring two rows' order in the node view swaps them in the export.
#[test]
fn c1_rewire_swaps_export_order() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 100.0, "height": 100.0})).unwrap();
    s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 0.0, "y": 0.0, "width": 100.0, "height": 100.0})).unwrap();
    s.execute("paint.setFill", &json!({"color": "#ff0000"})).unwrap();
    let red = s.doc().unwrap().doc.layers[0].id.0;

    let blue = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
    s.execute("layer.setCurrent", &json!({"id": blue})).unwrap();
    s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 25.0, "y": 25.0, "width": 50.0, "height": 50.0})).unwrap();
    s.execute("paint.setFill", &json!({"color": "#0000ff"})).unwrap();

    // Blue was created after red, so blue is above red by default
    let img_before = export_frame_png(&mut s, 1, "transparent");
    let p_before = img_before.get_pixel(50, 50);
    assert_eq!([p_before[0], p_before[1], p_before[2]], [0, 0, 255], "blue should be on top before rewire");

    // Rewire red above blue in the composite
    s.execute("nodes.connect", &json!({"from": format!("row:{red}"), "to": "comp", "port": "image", "above": format!("row:{blue}")})).unwrap();

    let img_after = export_frame_png(&mut s, 1, "transparent");
    let p_after = img_after.get_pixel(50, 50);
    assert_eq!([p_after[0], p_after[1], p_after[2]], [255, 0, 0], "red should be on top after rewire");
}

/// C1: a scene with no edits renders as before (test over 3 scenes: empty, single, and multi-layer).
#[test]
fn c1_untouched_scene_renders_as_before() {
    // Scene 1: Fixture::Single
    {
        let mut s = Fixture::Single.session();
        let before = export_frame_png(&mut s, 1, "white");
        let _ = s.execute("nodes.graph", &json!({})).unwrap();
        let after = export_frame_png(&mut s, 1, "white");
        assert_eq!(before.as_raw(), after.as_raw(), "Fixture::Single renders identically without node edits");
    }

    // Scene 2: Fixture::Multi
    {
        let mut s = Fixture::Multi.session();
        let before = export_frame_png(&mut s, 1, "white");
        let _ = s.execute("nodes.graph", &json!({})).unwrap();
        let after = export_frame_png(&mut s, 1, "white");
        assert_eq!(before.as_raw(), after.as_raw(), "Fixture::Multi renders identically without node edits");
    }

    // Scene 3: Multi-layer animation scene with distinct colors
    {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100.0, "height": 100.0})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 0.0, "y": 0.0, "width": 60.0, "height": 60.0})).unwrap();
        s.execute("paint.setFill", &json!({"color": "#ff0000"})).unwrap();

        let l2 = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
        s.execute("layer.setCurrent", &json!({"id": l2})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 20.0, "y": 20.0, "width": 60.0, "height": 60.0})).unwrap();
        s.execute("paint.setFill", &json!({"color": "#00ff00"})).unwrap();

        let before = export_frame_png(&mut s, 1, "transparent");
        // Overlap (30, 30): green (layer 2) is over red (layer 1)
        assert_eq!([before.get_pixel(30, 30)[0], before.get_pixel(30, 30)[1], before.get_pixel(30, 30)[2]], [0, 255, 0]);

        let _ = s.execute("nodes.graph", &json!({})).unwrap();
        let after = export_frame_png(&mut s, 1, "transparent");
        assert_eq!(before.as_raw(), after.as_raw(), "multi-layer scene renders identically without node edits");
    }
}

// ---------------------------------------------------------------------------
// C2: Colour ops per row, keyed
// ---------------------------------------------------------------------------

/// C2: each op keyed 0→full over 12 frames exports 12 different frames.
#[test]
fn c2_each_op_keyed_gives_12_frames() {
    let ops: &[(&str, Value, Value, Option<&str>)] = &[
        ("transparency", json!({"amount": 0.0}), json!({"amount": 100.0}), None),
        ("tint", json!({"amount": 0.0, "color": "#00ff00"}), json!({"amount": 100.0, "color": "#00ff00"}), Some("#ff0000")),
        ("colourFade", json!({"amount": 0.0, "color": "#00ff00"}), json!({"amount": 100.0, "color": "#00ff00"}), Some("#ff0000")),
        ("greyscale", json!({"amount": 0.0}), json!({"amount": 100.0}), Some("#ff2200")),
        (
            "levels",
            json!({"inBlack": 0.0, "inWhite": 255.0, "outBlack": 0.0}),
            json!({"inBlack": 80.0, "inWhite": 255.0, "outBlack": 100.0}),
            Some("#888888"),
        ),
        ("contrast", json!({"amount": 0.0}), json!({"amount": 90.0}), Some("#888888")),
        ("gamma", json!({"gamma": 1.0}), json!({"gamma": 4.0}), Some("#888888")),
        ("hueSaturation", json!({"hue": 0.0}), json!({"hue": 180.0}), Some("#ff0000")),
        ("colourScale", json!({"r": 1.0, "g": 1.0, "b": 1.0}), json!({"r": 0.0, "g": 0.0, "b": 0.0}), Some("#ffffff")),
        ("curves", json!({"points": "0,0 128,128 255,255"}), json!({"points": "0,0 128,255 255,255"}), Some("#888888")),
    ];

    for &(kind, ref k1, ref k12, initial_color) in ops {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100.0, "height": 100.0})).unwrap();
        s.execute("anim.setFrames", &json!({"frames": 12})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 10.0, "y": 10.0, "width": 80.0, "height": 80.0})).unwrap();
        s.execute("paint.setFill", &json!({"color": initial_color.unwrap_or("#ff0000")})).unwrap();
        s.execute("paint.setStroke", &json!({"none": true})).unwrap();
        let layer = s.doc().unwrap().doc.layers[0].id.0;

        s.execute("fx.add", &json!({"row": layer, "kind": kind})).unwrap();
        s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 1, "params": k1})).unwrap();
        s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 12, "params": k12})).unwrap();

        let mut hashes = BTreeSet::new();
        for f in 1..=12 {
            let img = export_frame_png(&mut s, f, "white");
            let mut hasher = DefaultHasher::new();
            img.as_raw().hash(&mut hasher);
            hashes.insert(hasher.finish());
        }
        assert_eq!(hashes.len(), 12, "op {kind} must produce 12 distinct exported frames");
    }
}

/// C2: greyscale of a pure red gives luma 54±1.
#[test]
fn c2_greyscale_red_luma_54() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 100.0, "height": 100.0})).unwrap();
    s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 0.0, "y": 0.0, "width": 100.0, "height": 100.0})).unwrap();
    s.execute("paint.setFill", &json!({"color": "#ff0000"})).unwrap();
    s.execute("paint.setStroke", &json!({"none": true})).unwrap();
    let layer = s.doc().unwrap().doc.layers[0].id.0;

    s.execute("fx.add", &json!({"row": layer, "kind": "greyscale"})).unwrap();
    s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 1, "params": {"amount": 100.0}})).unwrap();

    let img = export_frame_png(&mut s, 1, "transparent");
    let p = img.get_pixel(50, 50);
    let luma: f64 = 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64;
    assert!((luma - 54.0).abs() <= 1.0, "pure red under 100% greyscale must have luma 54±1, got {luma} (pixel: {p:?})");
}

// ---------------------------------------------------------------------------
// C3: Blurs
// ---------------------------------------------------------------------------

/// C3: a 1 pt white dot blurred by 10 spreads its energy symmetric within 1% (gaussian, test).
#[test]
fn c3_gaussian_dot_symmetric() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 200.0, "height": 200.0})).unwrap();
    s.execute("anim.setFrames", &json!({"frames": 12})).unwrap();
    s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 99.5, "y": 99.5, "width": 1.0, "height": 1.0})).unwrap();
    s.execute("paint.setFill", &json!({"color": "#ffffff"})).unwrap();
    s.execute("paint.setStroke", &json!({"none": true})).unwrap();
    let layer = s.doc().unwrap().doc.layers[0].id.0;

    s.execute("fx.add", &json!({"row": layer, "kind": "blur"})).unwrap();
    s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 1, "params": {"radius": 10.0}})).unwrap();

    let img = export_frame_png(&mut s, 1, "transparent");
    let (w, h) = img.dimensions();
    let cx = w / 2;
    let cy = h / 2;

    let mut left = 0.0f64;
    let mut right = 0.0f64;
    let mut top = 0.0f64;
    let mut bottom = 0.0f64;
    let mut total = 0.0f64;

    for y in 0..h {
        for x in 0..w {
            let val = img.get_pixel(x, y)[0] as f64;
            total += val;
            if x < cx {
                left += val;
            } else {
                right += val;
            }
            if y < cy {
                top += val;
            } else {
                bottom += val;
            }
        }
    }

    assert!(total > 0.0, "gaussian blur must produce energy");
    let lr_diff = (left - right).abs() / total;
    let tb_diff = (top - bottom).abs() / total;
    assert!(lr_diff < 0.01, "gaussian left/right asymmetry {lr_diff:.4} >= 1%");
    assert!(tb_diff < 0.01, "gaussian top/bottom asymmetry {tb_diff:.4} >= 1%");
}

/// C3: motion blur of a row moving 50 pt/frame streaks along the move (export check).
#[test]
fn c3_motion_blur_streaks_along_move() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 200.0, "height": 200.0})).unwrap();
    s.execute("anim.setFrames", &json!({"frames": 12})).unwrap();
    s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 95.0, "y": 95.0, "width": 10.0, "height": 10.0})).unwrap();
    s.execute("paint.setFill", &json!({"color": "#ffffff"})).unwrap();
    s.execute("paint.setStroke", &json!({"none": true})).unwrap();
    let moving_row = s.doc().unwrap().doc.layers[0].id.0;

    s.execute("peg.add", &json!({"rows": [moving_row]})).unwrap();
    s.execute("peg.key", &json!({"peg": 0, "frame": 1, "x": 0.0})).unwrap();
    s.execute("peg.key", &json!({"peg": 0, "frame": 2, "x": 50.0})).unwrap();

    s.execute("fx.add", &json!({"row": moving_row, "kind": "motionBlur"})).unwrap();
    s.execute("fx.key", &json!({"row": moving_row, "index": 0, "frame": 1, "params": {"amount": 100.0, "samples": 16}})).unwrap();

    let img = export_frame_png(&mut s, 1, "transparent");
    let (w, h) = img.dimensions();
    let x_extent = (0..w).filter(|&x| (0..h).any(|y| img.get_pixel(x, y)[3] > 10)).count();
    let y_extent = (0..h).filter(|&y| (0..w).any(|x| img.get_pixel(x, y)[3] > 10)).count();

    assert!(x_extent >= y_extent * 2, "horizontal streak expected: x_extent {x_extent} vs y_extent {y_extent}");
}

// ---------------------------------------------------------------------------
// C4: Mattes
// ---------------------------------------------------------------------------

/// C4: a gradient matte fades a row smoothly left to right (export check).
#[test]
fn c4_gradient_matte_fades_left_to_right() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 100.0, "height": 100.0})).unwrap();
    s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 0.0, "y": 0.0, "width": 100.0, "height": 100.0})).unwrap();
    s.execute("paint.setFill", &json!({"color": "#ff0000"})).unwrap();
    s.execute("paint.setStroke", &json!({"none": true})).unwrap();
    let red = s.doc().unwrap().doc.layers[0].id.0;

    let m = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
    s.execute("layer.setCurrent", &json!({"id": m})).unwrap();
    s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 0.0, "y": 0.0, "width": 100.0, "height": 100.0})).unwrap();
    s.execute(
        "paint.editGradient",
        &json!({"stroke": false, "kind": "linear", "stops": [{"offset": 0, "color": "#000000"}, {"offset": 1, "color": "#ffffff"}]}),
    )
    .unwrap();

    s.execute("cutter.add", &json!({"row": red, "matte": m, "by": "brightness"})).unwrap();

    // Export over white: green channel is 255 minus red alpha, so it rises smoothly from left to right
    let img = export_frame_png(&mut s, 1, "white");
    let g: Vec<i32> = [10, 30, 50, 70, 90].iter().map(|&x| i32::from(img.get_pixel(x, 50)[1])).collect();
    assert!(g.windows(2).all(|w| w[0] <= w[1]), "monotonic left to right: {g:?}");
    assert!(g.windows(2).all(|w| w[1] - w[0] <= 127), "no jump over half the range: {g:?}");
    assert!(g[0] < 128 && g[4] > 128, "red at the black end, cut at the white end: {g:?}");
}

/// C4: keying out green leaves no green pixel above 5% (test).
#[test]
fn c4_chroma_key_no_green_above_5pct() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 100.0, "height": 100.0})).unwrap();
    s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 10.0, "y": 10.0, "width": 80.0, "height": 80.0})).unwrap();
    s.execute("paint.setFill", &json!({"color": "#00ff00"})).unwrap();
    s.execute("paint.setStroke", &json!({"none": true})).unwrap();
    let row = s.doc().unwrap().doc.layers[0].id.0;

    s.execute("fx.add", &json!({"row": row, "kind": "chromaKey"})).unwrap();
    s.execute(
        "fx.key",
        &json!({"row": row, "index": 0, "frame": 1, "params": {"color": "#00ff00", "threshold": 20.0, "softness": 10.0, "spill": 50.0}}),
    )
    .unwrap();

    let img = export_frame_png(&mut s, 1, "transparent");
    // Any remaining green component on kept pixels (or alpha > 5% * 255 = 12 with green > 12)
    let green_pixels_above_5pct = img.pixels().filter(|p| p[3] > 12 && p[1] > 12).count();
    assert_eq!(green_pixels_above_5pct, 0, "keying out green must leave no green pixel above 5%");
}

// ---------------------------------------------------------------------------
// C5: Light and shadow from mattes
// ---------------------------------------------------------------------------

/// C5: a tone matte drawn on the face darkens only inside it, on frames 1 and 12 (export check).
#[test]
fn c5_tone_matte_darkens_only_inside_on_frames_1_and_12() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 200.0, "height": 200.0})).unwrap();
    s.execute("anim.setFrames", &json!({"frames": 12})).unwrap();
    s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 20.0, "y": 20.0, "width": 160.0, "height": 160.0})).unwrap();
    s.execute("paint.setFill", &json!({"color": "#888888"})).unwrap();
    s.execute("paint.setStroke", &json!({"none": true})).unwrap();
    let target_row = s.doc().unwrap().doc.layers[0].id.0;

    let matte_row = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
    s.execute("layer.setCurrent", &json!({"id": matte_row})).unwrap();
    s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 20.0, "y": 20.0, "width": 80.0, "height": 160.0})).unwrap();
    s.execute("paint.setFill", &json!({"color": "#ffffff"})).unwrap();
    s.execute("paint.setStroke", &json!({"none": true})).unwrap();

    // Peg for matte row moving 20 pt right from frame 1 to frame 12
    s.execute("peg.add", &json!({"rows": [matte_row]})).unwrap();
    s.execute("peg.key", &json!({"peg": 0, "frame": 1, "x": 0.0})).unwrap();
    s.execute("peg.key", &json!({"peg": 0, "frame": 12, "x": 20.0})).unwrap();

    // Baseline before tone effect
    let img1_base = export_frame_png(&mut s, 1, "transparent");
    let img12_base = export_frame_png(&mut s, 12, "transparent");

    // Add tone effect on target row driven by matte_row
    s.execute("fx.add", &json!({"row": target_row, "kind": "tone", "matte": matte_row})).unwrap();
    s.execute("fx.key", &json!({"row": target_row, "index": 0, "frame": 1, "params": {"opacity": 50.0, "color": "#000000", "blur": 0.0}})).unwrap();

    // Export frame 1
    let img1 = export_frame_png(&mut s, 1, "transparent");
    let px1_in = img1.get_pixel(50, 100);
    let px1_in_base = img1_base.get_pixel(50, 100);
    assert!(px1_in[0] < px1_in_base[0], "pixels inside matte must be darker at frame 1: {px1_in:?} vs base {px1_in_base:?}");

    let px1_out = img1.get_pixel(150, 100);
    let px1_out_base = img1_base.get_pixel(150, 100);
    assert_eq!(px1_out, px1_out_base, "pixels outside matte must be identical at frame 1");

    // Export frame 12 (matte shifted +20 pt to x: 40..120)
    let img12 = export_frame_png(&mut s, 12, "transparent");
    let px12_in = img12.get_pixel(70, 100);
    let px12_in_base = img12_base.get_pixel(70, 100);
    assert!(px12_in[0] < px12_in_base[0], "pixels inside moved matte must be darker at frame 12: {px12_in:?} vs base {px12_in_base:?}");

    let px12_out = img12.get_pixel(160, 100);
    let px12_out_base = img12_base.get_pixel(160, 100);
    assert_eq!(px12_out, px12_out_base, "pixels outside matte must be identical at frame 12");
}

// ---------------------------------------------------------------------------
// C6: Drawing selectors
// ---------------------------------------------------------------------------

/// C6: glow on line art only leaves the fills untouched (export check).
#[test]
fn c6_glow_on_line_art_leaves_fills() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 100.0, "height": 100.0})).unwrap();
    s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
    let row = s.doc().unwrap().doc.layers[0].id.0;
    let line = s.execute("shape.rectangle", &json!({"x": 20.0, "y": 20.0, "width": 60.0, "height": 60.0})).unwrap()["id"].as_u64().unwrap();
    s.execute("paint.setFill", &json!({"none": true})).unwrap();
    s.execute("stroke.set", &json!({"weight": 10.0})).unwrap();
    s.execute("art.fill", &json!({"ids": [line], "color": "#ff0000"})).unwrap();

    // Export baseline
    let img_base = export_frame_png(&mut s, 1, "transparent");
    let fill_base = *img_base.get_pixel(50, 50);
    assert_eq!([fill_base[0], fill_base[1], fill_base[2]], [255, 0, 0], "fill center is red");

    // Add glow with part: lineArt
    s.execute("fx.add", &json!({"row": row, "kind": "glow", "part": "lineArt"})).unwrap();

    let img_glow = export_frame_png(&mut s, 1, "transparent");
    let fill_glow = *img_glow.get_pixel(50, 50);

    assert_eq!(fill_glow, fill_base, "glow on line art must leave fills untouched");

    // And verify the glow is indeed applied around the line
    let outside_base = *img_base.get_pixel(12, 50);
    let outside_glow = *img_glow.get_pixel(12, 50);
    assert!(outside_glow[3] > outside_base[3], "glow from line art should spread beyond the stroke");
}

// ---------------------------------------------------------------------------
// C7: Groups and ports in the node view
// ---------------------------------------------------------------------------

/// C7: a group of 3 nodes collapses and expands without changing the render (test).
#[test]
fn c7_group_collapse_expand_same_render() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 100.0, "height": 100.0})).unwrap();
    s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 0.0, "y": 0.0, "width": 100.0, "height": 100.0})).unwrap();
    s.execute("paint.setFill", &json!({"color": "#ff0000"})).unwrap();
    let red = s.doc().unwrap().doc.layers[0].id.0;

    let blue = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
    s.execute("layer.setCurrent", &json!({"id": blue})).unwrap();
    s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 25.0, "y": 25.0, "width": 50.0, "height": 50.0})).unwrap();
    s.execute("paint.setFill", &json!({"color": "#0000ff"})).unwrap();

    let green = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
    s.execute("layer.setCurrent", &json!({"id": green})).unwrap();
    s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 10.0, "y": 10.0, "width": 30.0, "height": 30.0})).unwrap();
    s.execute("paint.setFill", &json!({"color": "#00ff00"})).unwrap();

    let img_baseline = export_frame_png(&mut s, 1, "transparent");

    // Group the 3 nodes
    let grp_res = s.execute("nodes.group", &json!({"members": [format!("row:{red}"), format!("row:{blue}"), format!("row:{green}")]})).unwrap();
    let grp_id = grp_res["id"].as_u64().unwrap();

    let img_grouped = export_frame_png(&mut s, 1, "transparent");
    assert_eq!(img_grouped.as_raw(), img_baseline.as_raw(), "grouping nodes must not change the render");

    // Collapse the group
    s.execute("nodes.collapse", &json!({"id": grp_id, "collapsed": true})).unwrap();
    let img_collapsed = export_frame_png(&mut s, 1, "transparent");
    assert_eq!(img_collapsed.as_raw(), img_baseline.as_raw(), "collapsing group must not change the render");

    // Expand the group
    s.execute("nodes.collapse", &json!({"id": grp_id, "collapsed": false})).unwrap();
    let img_expanded = export_frame_png(&mut s, 1, "transparent");
    assert_eq!(img_expanded.as_raw(), img_baseline.as_raw(), "expanding group must not change the render");
}
