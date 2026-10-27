//! 7rrek (Creative Studio): commands for timeline row effects (`fx.add`, `fx.key`, `fx.removeKey`,
//! `fx.remove`, `fx.list`).

use serde_json::{Value, json};

use super::anim::{Anim, frame_param, row_param, show};
use super::row_fx::{FxKind, MAX_ROW_EFFECTS, RowEffect, RowFx, RowFxList};
use super::selector::Part;
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "fx.add",
            "Add Row Effect",
            [],
            None,
            "{row?: layer id (default: the current layer's row), kind: \"blur\"|\"glow\"|\"dropShadow\"|\"colorAdjust\"|\"tint\"|\"transparency\"|\"greyscale\"|\"levels\"|\"contrast\"|\"gamma\"|\"hueSaturation\"|\"colourScale\"|\"curves\"|\"colourFade\"|\"box\"|\"directional\"|\"radial\"|\"radialZoom\", part?: \"all\"|\"lineArt\"|\"colourArt\" (drawing selector: only the lines, or only the Colour Art fills, of each drawing; default all)} adds an effect to the row → {row, index, kind}",
            has_doc,
            add
        ),
        cmd!(
            "fx.key",
            "Key Row Effect",
            [],
            None,
            "{row?: layer id (default: the current layer's row), index?: effect index (default 0), frame?: frame number (default: current frame), params: object} keys the effect parameters at that frame → {row, index, frame, params}",
            has_doc,
            key
        ),
        cmd!(
            "fx.removeKey",
            "Remove Row Effect Key",
            [],
            None,
            "{row?: layer id (default: the current layer's row), index?: effect index (default 0), frame?: frame number (default: current frame)} removes the keyframe → {row, index, frame}",
            has_doc,
            remove_key
        ),
        cmd!(
            "fx.remove",
            "Remove Row Effect",
            [],
            None,
            "{row?: layer id (default: the current layer's row), index?: effect index (default 0)} removes the effect from the row → {row, index}",
            has_doc,
            remove
        ),
        cmd!(
            query "fx.list",
            "Row Effects",
            [],
            None,
            "{row?: layer id (default: the current layer's row)} lists the row's effects and keyframes → {row, effects: [{index, kind, keys}]}",
            has_doc,
            list
        ),
    ]
}

fn commit(s: &mut Session, label: &str, all: RowFxList) -> Result<()> {
    s.edit(label, |d, _| {
        all.store(d);
        Ok(())
    })?;
    let current = Anim::of(&s.doc()?.doc).current;
    show(s, current)?;
    Ok(())
}

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "fx.add";
    let row = row_param(s, p, C)?;
    let kind_str = str_param(p, "kind").ok_or_else(|| bad(C, "`kind` is required"))?;
    let kind = FxKind::parse(kind_str).ok_or_else(|| bad(C, format!("unknown kind {kind_str:?}")))?;

    let part = match str_param(p, "part") {
        None => Part::All,
        Some(v) => Part::parse(v).ok_or_else(|| bad(C, format!("unknown part {v:?} (expected all, lineArt, colourArt)")))?,
    };

    let mut all = RowFxList::of(&s.doc()?.doc);
    let idx = match all.rows.iter().position(|r| r.row == row.0) {
        Some(i) => i,
        None => {
            all.rows.push(RowFx { row: row.0, effects: Vec::new() });
            all.rows.len() - 1
        }
    };
    let row_fx = &mut all.rows[idx];

    if row_fx.effects.len() >= MAX_ROW_EFFECTS {
        return Err(bad(C, format!("a row takes at most {MAX_ROW_EFFECTS} effects")));
    }

    let index = row_fx.effects.len();
    row_fx.effects.push(RowEffect { part, ..RowEffect::new(kind) });

    commit(s, "Add Row Effect", all)?;
    Ok(json!({"row": row.0, "index": index, "kind": kind.as_str()}))
}

fn key(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "fx.key";
    let row = row_param(s, p, C)?;
    let anim = Anim::of(&s.doc()?.doc);
    let frame = frame_param(p, &anim, C)?;
    let index = p.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;

    let params_val = p.get("params").ok_or_else(|| bad(C, "`params` is required"))?;
    if !params_val.is_object() {
        return Err(bad(C, "`params` must be an object"));
    }

    // Parameter limits and validations
    if params_val.get("radius").and_then(Value::as_f64).is_some_and(|r| !(0.0..=250.0).contains(&r)) {
        return Err(bad(C, "radius must be in 0..250"));
    }
    if params_val.get("blur").and_then(Value::as_f64).is_some_and(|b| !(0.0..=250.0).contains(&b)) {
        return Err(bad(C, "blur must be in 0..250"));
    }
    if params_val.get("gamma").and_then(Value::as_f64).is_some_and(|g| g <= 0.0) {
        return Err(bad(C, "gamma must be positive (0.1..10)"));
    }

    let mut all = RowFxList::of(&s.doc()?.doc);
    let row_fx = all.rows.iter_mut().find(|r| r.row == row.0).ok_or_else(|| bad(C, "this row has no effects"))?;
    let fx = row_fx.effects.get_mut(index).ok_or_else(|| bad(C, format!("no effect at index {index}")))?;

    fx.keys.insert(frame, params_val.clone());

    commit(s, "Key Row Effect", all)?;
    Ok(json!({"row": row.0, "index": index, "frame": frame, "params": params_val}))
}

fn remove_key(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "fx.removeKey";
    let row = row_param(s, p, C)?;
    let anim = Anim::of(&s.doc()?.doc);
    let frame = frame_param(p, &anim, C)?;
    let index = p.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;

    let mut all = RowFxList::of(&s.doc()?.doc);
    let row_fx = all.rows.iter_mut().find(|r| r.row == row.0).ok_or_else(|| bad(C, "this row has no effects"))?;
    let fx = row_fx.effects.get_mut(index).ok_or_else(|| bad(C, format!("no effect at index {index}")))?;

    if fx.keys.remove(&frame).is_none() {
        return Err(bad(C, format!("no key at frame {frame} for effect {index}")));
    }

    commit(s, "Remove Row Effect Key", all)?;
    Ok(json!({"row": row.0, "index": index, "frame": frame}))
}

fn remove(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "fx.remove";
    let row = row_param(s, p, C)?;
    let index = p.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;

    let mut all = RowFxList::of(&s.doc()?.doc);
    let row_fx = all.rows.iter_mut().find(|r| r.row == row.0).ok_or_else(|| bad(C, "this row has no effects"))?;

    if index >= row_fx.effects.len() {
        return Err(bad(C, format!("no effect at index {index}")));
    }

    row_fx.effects.remove(index);
    if row_fx.effects.is_empty() {
        all.rows.retain(|r| r.row != row.0);
    }

    commit(s, "Remove Row Effect", all)?;
    Ok(json!({"row": row.0, "index": index}))
}

fn list(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "fx.list";
    let row = row_param(s, p, C)?;
    let all = RowFxList::of(&s.doc()?.doc);
    let effects = match all.rows.iter().find(|r| r.row == row.0) {
        Some(r) => r
            .effects
            .iter()
            .enumerate()
            .map(|(i, fx)| {
                json!({
                    "index": i,
                    "kind": fx.kind.as_str(),
                    "keys": fx.keys,
                })
            })
            .collect(),
        None => Vec::new(),
    };
    Ok(json!({"row": row.0, "effects": effects}))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::hash::{DefaultHasher, Hash, Hasher};

    use serde_json::json;

    use super::*;
    use crate::cmd::anim::{at_frame, unposed};

    fn session_with_rect() -> (Session, u64) {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();
        s.execute("anim.setFrames", &json!({"frames": 12})).unwrap();
        let _ = s.execute("shape.rectangle", &json!({"x": 50, "y": 50, "width": 100, "height": 100})).unwrap();
        let layer_id = s.doc().unwrap().doc.layers[0].id.0;
        (s, layer_id)
    }

    #[test]
    fn fx_add_and_list_and_remove() {
        let (mut s, layer) = session_with_rect();
        let add_res = s.execute("fx.add", &json!({"row": layer, "kind": "glow"})).unwrap();
        assert_eq!(add_res["index"], json!(0));
        assert_eq!(add_res["kind"], json!("glow"));

        let list_res = s.execute("fx.list", &json!({"row": layer})).unwrap();
        assert_eq!(list_res["effects"].as_array().unwrap().len(), 1);

        let rem_res = s.execute("fx.remove", &json!({"row": layer, "index": 0})).unwrap();
        assert_eq!(rem_res["index"], json!(0));

        let list_res2 = s.execute("fx.list", &json!({"row": layer})).unwrap();
        assert_eq!(list_res2["effects"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn bad_params_rejected_without_panic() {
        let (mut s, layer) = session_with_rect();
        assert!(s.execute("fx.add", &json!({"row": layer, "kind": "magic"})).is_err());
        assert!(s.execute("fx.add", &json!({"row": 999_999, "kind": "blur"})).is_err());
        assert!(s.execute("fx.key", &json!({"row": layer, "index": 0, "params": {}})).is_err(), "no effects yet");

        s.execute("fx.add", &json!({"row": layer, "kind": "blur"})).unwrap();
        assert!(s.execute("fx.key", &json!({"row": layer, "index": 5, "params": {}})).is_err());
        assert!(s.execute("fx.key", &json!({"row": layer, "index": 0, "params": "not_an_object"})).is_err());
        assert!(s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 50, "params": {}})).is_err(), "frame > frames");
        assert!(s.execute("fx.removeKey", &json!({"row": layer, "index": 0, "frame": 1})).is_err(), "no key yet");
        assert!(s.execute("fx.remove", &json!({"row": layer, "index": 5})).is_err());

        // Negative radius or > 250
        assert!(s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 1, "params": {"radius": -5.0}})).is_err());
        assert!(s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 1, "params": {"radius": 300.0}})).is_err());

        // Gamma 0
        s.execute("fx.add", &json!({"row": layer, "kind": "gamma"})).unwrap();
        assert!(s.execute("fx.key", &json!({"row": layer, "index": 1, "frame": 1, "params": {"gamma": 0.0}})).is_err());
    }

    #[test]
    fn undo_in_unposed_leaves_art_at_rest_without_effect() {
        let (mut s, layer) = session_with_rect();
        s.execute("fx.add", &json!({"row": layer, "kind": "blur"})).unwrap();
        s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 1, "params": {"radius": 10.0}})).unwrap();

        let doc = &s.doc().unwrap().doc;
        let frame_doc = at_frame(doc, 1);
        let layer_node = frame_doc.layers.iter().find(|l| l.id.0 == layer).unwrap();
        assert!(!layer_node.appearance.effects.is_empty(), "frame doc should have applied effect");

        let rest_doc = unposed(&frame_doc);
        let rest_layer = rest_doc.layers.iter().find(|l| l.id.0 == layer).unwrap();
        assert!(rest_layer.appearance.effects.is_empty(), "unposed doc should have effects removed");
    }

    #[test]
    fn glow_keyed_0_to_10_over_12_frames_exports_12_different_frames() {
        let (mut s, layer) = session_with_rect();
        s.execute("fx.add", &json!({"row": layer, "kind": "glow"})).unwrap();
        s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 1, "params": {"radius": 0.0}})).unwrap();
        s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 12, "params": {"radius": 10.0}})).unwrap();

        let doc = &s.doc().unwrap().doc;
        let rect = doc.artboards[0].rect;

        let mut hashes = BTreeSet::new();
        for f in 1..=12 {
            let fd = at_frame(doc, f);
            let mut r = vectorcraft_render::Renderer::new();
            let img = r.render_region(&fd, rect, 1.0, false);
            let mut hasher = DefaultHasher::new();
            img.pixels.hash(&mut hasher);
            let hash = hasher.finish();
            hashes.insert(hash);
        }
        assert_eq!(hashes.len(), 12, "all 12 frames must produce different pixel outputs");
    }

    #[test]
    fn tint_keyed_0_to_50_over_12_frames_exports_12_different_frames() {
        let (mut s, layer) = session_with_rect();
        s.execute("paint.setFill", &json!({"color": "#888888"})).unwrap();
        s.execute("fx.add", &json!({"row": layer, "kind": "tint"})).unwrap();
        s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 1, "params": {"amount": 0.0, "color": "#ff0000"}})).unwrap();
        s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 12, "params": {"amount": 50.0, "color": "#ff0000"}})).unwrap();

        let doc = &s.doc().unwrap().doc;
        let rect = doc.artboards[0].rect;

        let mut hashes = std::collections::BTreeSet::new();
        for f in 1..=12 {
            let fd = at_frame(doc, f);
            let mut r = vectorcraft_render::Renderer::new();
            let img = r.render_region(&fd, rect, 1.0, false);
            let mut hasher = DefaultHasher::new();
            img.pixels.hash(&mut hasher);
            let hash = hasher.finish();
            hashes.insert(hash);
        }
        assert_eq!(hashes.len(), 12, "all 12 frames must produce different pixel outputs");
    }

    fn check_12_distinct_frames(kind: &str, k1: Value, k12: Value, initial_color: Option<&str>) {
        let (mut s, layer) = session_with_rect();
        if let Some(c) = initial_color {
            s.execute("paint.setFill", &json!({"color": c})).unwrap();
        }
        s.execute("fx.add", &json!({"row": layer, "kind": kind})).unwrap();
        s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 1, "params": k1})).unwrap();
        s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 12, "params": k12})).unwrap();

        let doc = &s.doc().unwrap().doc;
        let rect = doc.artboards[0].rect;

        let mut hashes = BTreeSet::new();
        for f in 1..=12 {
            let fd = at_frame(doc, f);
            let mut r = vectorcraft_render::Renderer::new();
            let img = r.render_region(&fd, rect, 1.0, false);
            let mut hasher = DefaultHasher::new();
            img.pixels.hash(&mut hasher);
            hashes.insert(hasher.finish());
        }
        assert_eq!(hashes.len(), 12, "kind {kind} must produce 12 distinct frames across keyframes");
    }

    #[test]
    fn each_new_kind_keyed_0_to_full_over_12_frames_exports_12_different_frames() {
        let kinds: &[(&str, Value, Value, Option<&str>)] = &[
            ("transparency", json!({"amount": 0.0}), json!({"amount": 100.0}), None),
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
            ("colourFade", json!({"amount": 0.0, "color": "#00ff00"}), json!({"amount": 100.0, "color": "#00ff00"}), Some("#ff0000")),
            ("box", json!({"radius": 0.0}), json!({"radius": 25.0}), None),
            ("directional", json!({"radius": 0.0, "angle": 0.0}), json!({"radius": 25.0, "angle": 45.0}), None),
            ("radial", json!({"radius": 1.0}), json!({"radius": 15.0}), None),
            ("radialZoom", json!({"amount": 1.0}), json!({"amount": 50.0}), None),
        ];

        for &(kind, ref k1, ref k12, init_col) in kinds {
            check_12_distinct_frames(kind, k1.clone(), k12.clone(), init_col);
        }
    }

    #[test]
    fn unposed_keeps_an_effect_the_user_added_at_a_frame() {
        let (mut s, layer) = session_with_rect();
        s.execute("fx.add", &json!({"row": layer, "kind": "blur"})).unwrap();
        let mut frame_doc = at_frame(&s.doc().unwrap().doc, 1);
        let own = vectorcraft_doc::Effect { id: "stylize.dropShadow".into(), params: json!({}), visible: true };
        frame_doc.node_mut(vectorcraft_doc::NodeId(layer)).unwrap().appearance.effects.push(own);
        let rest = unposed(&frame_doc);
        let ids: Vec<_> = rest.layers.iter().find(|l| l.id.0 == layer).unwrap().appearance.effects.iter().map(|e| e.id.clone()).collect();
        assert_eq!(ids, ["stylize.dropShadow"]);
    }

    #[test]
    fn greyscale_100_percent_on_pure_red_row_gives_luma_54_plus_minus_one() {
        let (mut s, layer) = session_with_rect();
        s.execute("paint.setFill", &json!({"color": "#ff0000"})).unwrap();
        s.execute("fx.add", &json!({"row": layer, "kind": "greyscale"})).unwrap();
        s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 1, "params": {"amount": 100.0}})).unwrap();

        let doc = &s.doc().unwrap().doc;
        let rect = doc.artboards[0].rect;
        let fd = at_frame(doc, 1);
        let mut r = vectorcraft_render::Renderer::new();
        let img = r.render_region(&fd, rect, 1.0, false);

        assert!(img.width > 0 && img.height > 0);
        let p = img.pixel(img.width / 2, img.height / 2);
        let luma: f64 = 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64;
        assert!((luma - 54.0).abs() <= 1.0, "pure red under 100% greyscale must have luma 54±1, got {luma} (pixel: {p:?})");
    }

    #[test]
    fn box_and_gaussian_for_1pt_white_dot_at_radius_10_energy_symmetric_within_1_percent() {
        for kind in ["box", "blur"] {
            let mut s = Session::new();
            s.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();
            s.execute("anim.setFrames", &json!({"frames": 12})).unwrap();
            s.execute("shape.rectangle", &json!({"x": 99.5, "y": 99.5, "width": 1.0, "height": 1.0})).unwrap();
            s.execute("paint.setFill", &json!({"color": "#ffffff"})).unwrap();
            s.execute("paint.setStroke", &json!({"none": true})).unwrap();
            let layer = s.doc().unwrap().doc.layers[0].id.0;

            s.execute("fx.add", &json!({"row": layer, "kind": kind})).unwrap();
            s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 1, "params": {"radius": 10.0}})).unwrap();

            let doc = &s.doc().unwrap().doc;
            let rect = doc.artboards[0].rect;
            let fd = at_frame(doc, 1);
            let mut r = vectorcraft_render::Renderer::new();
            let img = r.render_region(&fd, rect, 1.0, false);

            let w = img.width;
            let h = img.height;
            let cx = w / 2;
            let cy = h / 2;

            let mut left = 0.0f64;
            let mut right = 0.0f64;
            let mut top = 0.0f64;
            let mut bottom = 0.0f64;
            let mut total = 0.0f64;

            for y in 0..h {
                for x in 0..w {
                    let val = img.pixel(x, y)[0] as f64;
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

            assert!(total > 0.0, "blur of kind {kind} must produce energy");
            let lr_diff = (left - right).abs() / total;
            let tb_diff = (top - bottom).abs() / total;
            assert!(lr_diff < 0.01, "{kind} left/right asymmetry {lr_diff:.4} >= 1%");
            assert!(tb_diff < 0.01, "{kind} top/bottom asymmetry {tb_diff:.4} >= 1%");
        }
    }

    #[test]
    fn directional_blur_at_angle_zero_has_no_vertical_spread() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();
        s.execute("anim.setFrames", &json!({"frames": 12})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 99.5, "y": 99.5, "width": 1.0, "height": 1.0})).unwrap();
        s.execute("paint.setFill", &json!({"color": "#ffffff"})).unwrap();
        s.execute("paint.setStroke", &json!({"none": true})).unwrap();
        let layer = s.doc().unwrap().doc.layers[0].id.0;

        let rect = s.doc().unwrap().doc.artboards[0].rect;
        let fd_orig = at_frame(&s.doc().unwrap().doc, 1);
        let img_orig = vectorcraft_render::Renderer::new().render_region(&fd_orig, rect, 1.0, false);
        let w = img_orig.width;
        let h = img_orig.height;
        let y_extent_orig: Vec<u32> = (0..h).filter(|&y| (0..w).any(|x| img_orig.pixel(x, y)[3] > 0)).collect();

        s.execute("fx.add", &json!({"row": layer, "kind": "directional"})).unwrap();
        s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 1, "params": {"radius": 15.0, "angle": 0.0}})).unwrap();

        let fd_blur = at_frame(&s.doc().unwrap().doc, 1);
        let img_blur = vectorcraft_render::Renderer::new().render_region(&fd_blur, rect, 1.0, false);
        let y_extent_blur: Vec<u32> = (0..h).filter(|&y| (0..w).any(|x| img_blur.pixel(x, y)[3] > 0)).collect();

        assert_eq!(y_extent_orig, y_extent_blur, "directional blur at angle 0 must not spread vertically");

        let x_extent_orig = (0..w).filter(|&x| (0..h).any(|y| img_orig.pixel(x, y)[3] > 0)).count();
        let x_extent_blur = (0..w).filter(|&x| (0..h).any(|y| img_blur.pixel(x, y)[3] > 0)).count();
        assert!(x_extent_blur > x_extent_orig + 10, "directional blur must spread horizontally");
    }

    #[test]
    fn rendering_same_frame_5_times_gives_same_pixels_and_row_opacity_unchanged() {
        let (mut s, layer) = session_with_rect();
        s.execute("fx.add", &json!({"row": layer, "kind": "transparency"})).unwrap();
        s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 1, "params": {"amount": 50.0}})).unwrap();
        s.execute("fx.add", &json!({"row": layer, "kind": "box"})).unwrap();
        s.execute("fx.key", &json!({"row": layer, "index": 1, "frame": 1, "params": {"radius": 5.0}})).unwrap();

        let initial_opacity = s.doc().unwrap().doc.node(vectorcraft_doc::NodeId(layer)).unwrap().opacity;
        let rect = s.doc().unwrap().doc.artboards[0].rect;

        let mut first_pixels = None;
        for _ in 0..5 {
            let fd = at_frame(&s.doc().unwrap().doc, 1);
            let mut r = vectorcraft_render::Renderer::new();
            let img = r.render_region(&fd, rect, 1.0, false);
            if let Some(ref prev) = first_pixels {
                assert_eq!(&img.pixels, prev, "repeated render must produce identical pixels");
            } else {
                first_pixels = Some(img.pixels);
            }
        }

        let final_opacity = s.doc().unwrap().doc.node(vectorcraft_doc::NodeId(layer)).unwrap().opacity;
        assert_eq!(initial_opacity, final_opacity, "row opacity in document must not change");
    }
}
