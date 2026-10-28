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
            "{row?: layer id (default: the current layer's row), kind: \"blur\"|\"glow\"|\"dropShadow\"|\"colorAdjust\"|\"tint\"|\"transparency\"|\"greyscale\"|\"levels\"|\"contrast\"|\"gamma\"|\"hueSaturation\"|\"colourScale\"|\"curves\"|\"colourFade\"|\"box\"|\"directional\"|\"radial\"|\"radialZoom\"|\"chromaKey\"|\"tone\"|\"highlight\"|\"gradient\"|\"bloom\"|\"motionBlur\", part?: \"all\"|\"lineArt\"|\"colourArt\" (drawing selector: only the lines, or only the Colour Art fills, of each drawing; default all), matte?: layer id} adds an effect to the row → {row, index, kind, matte?}",
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
            "fx.set",
            "Set Row Effect",
            [],
            None,
            "{row?: layer id (default: the current layer's row), index: effect index, part?: \"all\"|\"lineArt\"|\"colourArt\" (drawing selector: which part of each drawing the effect takes), matte?: layer id | null} changes the part or matte of an existing effect → {row, index, part, matte}",
            has_doc,
            set
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
            "{row?: layer id (default: the current layer's row)} lists the row's effects and keyframes → {row, effects: [{index, kind, keys, part, matte}]}",
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

    let matte = match p.get("matte") {
        Some(Value::Null) | None => None,
        Some(v) => {
            let m = v.as_u64().ok_or_else(|| bad(C, "`matte` must be a layer id"))?;
            if m == row.0 {
                return Err(bad(C, "a row cannot be its own matte"));
            }
            if !s.doc()?.doc.layers.iter().any(|l| l.id.0 == m) {
                return Err(bad(C, format!("matte layer {m} not found")));
            }
            Some(m)
        }
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
    row_fx.effects.push(RowEffect { part, matte, ..RowEffect::new(kind) });

    commit(s, "Add Row Effect", all)?;
    let mut ret = json!({"row": row.0, "index": index, "kind": kind.as_str()});
    if let Some(m) = matte {
        ret["matte"] = json!(m);
    }
    Ok(ret)
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
    if params_val.get("opacity").and_then(Value::as_f64).is_some_and(|o| !(0.0..=100.0).contains(&o)) {
        return Err(bad(C, "opacity must be in 0..100"));
    }
    if params_val.get("threshold").and_then(Value::as_f64).is_some_and(|t| !(0.0..=100.0).contains(&t)) {
        return Err(bad(C, "threshold must be in 0..100"));
    }
    if params_val.get("amount").and_then(Value::as_f64).is_some_and(|a| !(0.0..=100.0).contains(&a)) {
        return Err(bad(C, "amount must be in 0..100"));
    }
    if params_val.get("samples").and_then(Value::as_u64).is_some_and(|s| !(2..=32).contains(&s)) {
        return Err(bad(C, "samples must be in 2..32"));
    }

    let mut all = RowFxList::of(&s.doc()?.doc);
    let row_fx = all.rows.iter_mut().find(|r| r.row == row.0).ok_or_else(|| bad(C, "this row has no effects"))?;
    let fx = row_fx.effects.get_mut(index).ok_or_else(|| bad(C, format!("no effect at index {index}")))?;

    if fx.kind == FxKind::ChromaKey && params_val.get("color").is_some_and(|c| !c.as_str().is_some_and(is_hex_colour)) {
        return Err(bad(C, "chroma key colour must be #rrggbb"));
    }
    if (fx.kind == FxKind::Tone || fx.kind == FxKind::Highlight) && params_val.get("color").is_some_and(|c| !c.as_str().is_some_and(is_hex_colour)) {
        return Err(bad(C, "color must be #rrggbb"));
    }
    if fx.kind == FxKind::Gradient {
        if let Some(t) = params_val.get("type").and_then(Value::as_str)
            && t != "linear"
            && t != "radial"
        {
            return Err(bad(C, "gradient type must be linear or radial"));
        }
        if params_val.get("color0").is_some_and(|c| !c.as_str().is_some_and(is_hex_colour)) {
            return Err(bad(C, "color0 must be #rrggbb"));
        }
        if params_val.get("color1").is_some_and(|c| !c.as_str().is_some_and(is_hex_colour)) {
            return Err(bad(C, "color1 must be #rrggbb"));
        }
        for coord in ["x0", "y0", "x1", "y1"] {
            if params_val.get(coord).and_then(Value::as_f64).is_some_and(|v| !(0.0..=1.0).contains(&v)) {
                return Err(bad(C, format!("{coord} must be in 0..1")));
            }
        }
    }

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

/// `#rrggbb`, the only colour form the chroma key reads.
fn is_hex_colour(s: &str) -> bool {
    s.strip_prefix('#').is_some_and(|d| d.len() == 6 && d.bytes().all(|c| c.is_ascii_hexdigit()))
}

fn set(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "fx.set";
    let row = row_param(s, p, C)?;
    let index = p.get("index").and_then(Value::as_u64).and_then(|i| usize::try_from(i).ok()).ok_or_else(|| bad(C, "`index` is required"))?;

    let mut all = RowFxList::of(&s.doc()?.doc);
    let row_fx = all.rows.iter_mut().find(|r| r.row == row.0).ok_or_else(|| bad(C, "this row has no effects"))?;
    let fx = row_fx.effects.get_mut(index).ok_or_else(|| bad(C, format!("no effect at index {index}")))?;

    let mut changed = false;
    if let Some(part_str) = str_param(p, "part") {
        let part = Part::parse(part_str).ok_or_else(|| bad(C, format!("unknown part {part_str:?} (expected all, lineArt, colourArt)")))?;
        fx.part = part;
        changed = true;
    }

    if let Some(mv) = p.get("matte") {
        if mv.is_null() {
            fx.matte = None;
            changed = true;
        } else if let Some(m) = mv.as_u64() {
            if m == row.0 {
                return Err(bad(C, "a row cannot be its own matte"));
            }
            if !s.doc()?.doc.layers.iter().any(|l| l.id.0 == m) {
                return Err(bad(C, format!("matte layer {m} not found")));
            }
            fx.matte = Some(m);
            changed = true;
        } else {
            return Err(bad(C, "`matte` must be a layer id or null"));
        }
    }

    if !changed {
        return Err(bad(C, "`part` or `matte` is required"));
    }

    let (saved_part, saved_matte) = (fx.part, fx.matte);
    commit(s, "Set Row Effect", all)?;
    let mut ret = json!({"row": row.0, "index": index, "part": saved_part});
    if let Some(m) = saved_matte {
        ret["matte"] = json!(m);
    } else {
        ret["matte"] = Value::Null;
    }
    Ok(ret)
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
                let mut item = json!({
                    "index": i,
                    "kind": fx.kind.as_str(),
                    "keys": fx.keys,
                    "part": fx.part,
                });
                if let Some(m) = fx.matte {
                    item["matte"] = json!(m);
                } else {
                    item["matte"] = Value::Null;
                }
                item
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
    use crate::cmd::art_layers::COLOUR_ART;

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
            ("tone", json!({"opacity": 0.0, "color": "#000000"}), json!({"opacity": 100.0, "color": "#000000"}), Some("#888888")),
            ("highlight", json!({"opacity": 0.0, "color": "#ffffff"}), json!({"opacity": 100.0, "color": "#ffffff"}), Some("#888888")),
            ("gradient", json!({"color0": "#000000", "color1": "#000000"}), json!({"color0": "#000000", "color1": "#ffffff"}), Some("#888888")),
            (
                "bloom",
                json!({"amount": 0.0, "threshold": 50.0, "radius": 10.0}),
                json!({"amount": 100.0, "threshold": 50.0, "radius": 10.0}),
                Some("#ffffff"),
            ),
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

    #[test]
    fn chroma_key_keyed_spill_0_to_full_over_12_frames_exports_12_different_frames() {
        check_12_distinct_frames(
            "chromaKey",
            json!({"threshold": 20.0, "softness": 10.0, "spill": 0.0}),
            json!({"threshold": 20.0, "softness": 10.0, "spill": 100.0}),
            Some("#64c864"),
        );
    }

    /// A black outline square on frame 1 with a red Colour Art fill. Returns (session, row, line id, colour art id).
    fn drawing_with_colour_art() -> (Session, u64, u64, u64) {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        let row = s.doc().unwrap().doc.layers[0].id.0;
        let line = s.execute("shape.rectangle", &json!({"x": 20, "y": 20, "width": 60, "height": 60})).unwrap()["id"].as_u64().unwrap();
        s.execute("paint.setFill", &json!({"none": true})).unwrap();
        s.execute("stroke.set", &json!({"weight": 10})).unwrap();
        s.execute("art.fill", &json!({"ids": [line], "color": "#ff0000"})).unwrap();
        let d = &s.doc().unwrap().doc;
        let drawing = d.layers[0].children().unwrap()[0].id;
        let ca = d.node(drawing).unwrap().children().unwrap().iter().find(|c| c.name.as_deref() == Some(COLOUR_ART)).unwrap().id.0;
        (s, row, line, ca)
    }

    /// Effects on (row, line art, colour art) on frame 1.
    fn effect_counts(s: &mut Session, row: u64, line: u64, ca: u64) -> (usize, usize, usize) {
        s.execute("anim.goto", &json!({"frame": 1})).unwrap();
        let count = |id: u64| s.doc().unwrap().doc.node(vectorcraft_doc::NodeId(id)).unwrap().appearance.effects.len();
        (count(row), count(line), count(ca))
    }

    #[test]
    fn fx_set_moves_the_effect_lineart_then_colourart_then_all_on_the_right_nodes() {
        let (mut s, row, line, ca) = drawing_with_colour_art();
        s.execute("fx.add", &json!({"row": row, "kind": "glow"})).unwrap();
        assert_eq!(effect_counts(&mut s, row, line, ca), (1, 0, 0), "all: on the row");

        s.execute("fx.set", &json!({"row": row, "index": 0, "part": "lineArt"})).unwrap();
        assert_eq!(effect_counts(&mut s, row, line, ca), (0, 1, 0), "lineArt: on the line only");

        s.execute("fx.set", &json!({"row": row, "index": 0, "part": "colourArt"})).unwrap();
        assert_eq!(effect_counts(&mut s, row, line, ca), (0, 0, 1), "colourArt: on the colour art only");

        s.execute("fx.set", &json!({"row": row, "index": 0, "part": "all"})).unwrap();
        assert_eq!(effect_counts(&mut s, row, line, ca), (1, 0, 0), "all: back on the row");
        assert_eq!(RowFxList::of(&s.doc().unwrap().doc).rows[0].effects[0].part, Part::All);
    }

    #[test]
    fn fx_set_is_one_undo_step() {
        let (mut s, row, line, ca) = drawing_with_colour_art();
        s.execute("fx.add", &json!({"row": row, "kind": "glow"})).unwrap();
        s.execute("fx.set", &json!({"row": row, "index": 0, "part": "lineArt"})).unwrap();
        assert_eq!(effect_counts(&mut s, row, line, ca), (0, 1, 0));
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(RowFxList::of(&s.doc().unwrap().doc).rows[0].effects[0].part, Part::All, "undo restores the part");
    }

    #[test]
    fn fx_set_bad_part_or_index_or_row_is_an_error_and_changes_nothing() {
        let (mut s, row, _, _) = drawing_with_colour_art();
        s.execute("fx.add", &json!({"row": row, "kind": "glow"})).unwrap();
        assert!(s.execute("fx.set", &json!({"row": row, "index": 0, "part": "bogus"})).is_err());
        assert!(s.execute("fx.set", &json!({"row": row, "index": 0})).is_err(), "part is required");
        assert!(s.execute("fx.set", &json!({"row": row, "part": "lineArt"})).is_err(), "index is required");
        assert!(s.execute("fx.set", &json!({"row": row, "index": 7, "part": "lineArt"})).is_err());
        assert!(s.execute("fx.set", &json!({"row": row, "index": -1, "part": "lineArt"})).is_err());
        assert!(s.execute("fx.set", &json!({"row": 999_999, "index": 0, "part": "all"})).is_err());
        assert_eq!(RowFxList::of(&s.doc().unwrap().doc).rows[0].effects[0].part, Part::All);
    }

    #[test]
    fn chroma_key_bad_colour_is_an_error_and_a_hex_colour_is_kept() {
        let (mut s, row, _, _) = drawing_with_colour_art();
        s.execute("fx.add", &json!({"row": row, "kind": "chromaKey"})).unwrap();
        assert!(s.execute("fx.key", &json!({"row": row, "index": 0, "frame": 1, "params": {"color": "nope"}})).is_err());
        assert!(s.execute("fx.key", &json!({"row": row, "index": 0, "frame": 1, "params": {"color": "#0f0"}})).is_err());
        assert!(s.execute("fx.key", &json!({"row": row, "index": 0, "frame": 1, "params": {"color": 5}})).is_err());
        assert!(s.execute("fx.key", &json!({"row": row, "index": 0, "frame": 1, "params": {"color": "#00ff00", "threshold": 30}})).is_ok());
    }

    #[test]
    fn tone_with_matte_darkens_inside_and_leaves_outside_identical_on_frames_1_and_12() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();
        s.execute("anim.setFrames", &json!({"frames": 12})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 20, "y": 20, "width": 160, "height": 160})).unwrap();
        s.execute("paint.setFill", &json!({"color": "#888888"})).unwrap();
        s.execute("paint.setStroke", &json!({"none": true})).unwrap();
        let target_row = s.doc().unwrap().doc.layers[0].id.0;

        let matte_row = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
        s.execute("layer.setCurrent", &json!({"id": matte_row})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 20, "y": 20, "width": 80, "height": 160})).unwrap();
        s.execute("paint.setFill", &json!({"color": "#ffffff"})).unwrap();
        s.execute("paint.setStroke", &json!({"none": true})).unwrap();

        // Peg for matte row moving 20 pt right from frame 1 to frame 12
        s.execute("peg.add", &json!({"rows": [matte_row]})).unwrap();
        s.execute("peg.key", &json!({"peg": 0, "frame": 1, "x": 0})).unwrap();
        s.execute("peg.key", &json!({"peg": 0, "frame": 12, "x": 20})).unwrap();

        let rect = s.doc().unwrap().doc.artboards[0].rect;

        // Render frame 1 baseline (before tone effect)
        let fd1_base = at_frame(&s.doc().unwrap().doc, 1);
        let img1_base = vectorcraft_render::Renderer::new().render_region(&fd1_base, rect, 1.0, false);
        let fd12_base = at_frame(&s.doc().unwrap().doc, 12);
        let img12_base = vectorcraft_render::Renderer::new().render_region(&fd12_base, rect, 1.0, false);

        // Add tone effect on target row with matte = matte_row
        s.execute("fx.add", &json!({"row": target_row, "kind": "tone", "matte": matte_row})).unwrap();
        s.execute("fx.key", &json!({"row": target_row, "index": 0, "frame": 1, "params": {"opacity": 50.0, "color": "#000000", "blur": 0.0}}))
            .unwrap();

        // Render frame 1 with tone effect
        let fd1 = at_frame(&s.doc().unwrap().doc, 1);
        let img1 = vectorcraft_render::Renderer::new().render_region(&fd1, rect, 1.0, false);

        // Inside matte at frame 1: x in 20..100 (e.g. x: 50, y: 100) -> darker
        let px1_in = img1.pixel(50, 100);
        let px1_in_base = img1_base.pixel(50, 100);
        assert!(px1_in[0] < px1_in_base[0], "pixels inside matte must be darker: {px1_in:?} vs base {px1_in_base:?}");

        // Outside matte at frame 1: x in 105..180 (e.g. x: 150, y: 100) -> byte identical
        let px1_out = img1.pixel(150, 100);
        let px1_out_base = img1_base.pixel(150, 100);
        assert_eq!(px1_out, px1_out_base, "pixels outside matte must be byte-identical at frame 1");

        // Render frame 12 with tone effect
        let fd12 = at_frame(&s.doc().unwrap().doc, 12);
        let img12 = vectorcraft_render::Renderer::new().render_region(&fd12, rect, 1.0, false);

        // Inside matte at frame 12: matte moved by 20 to x in 40..120 (e.g. x: 70, y: 100) -> darker
        let px12_in = img12.pixel(70, 100);
        let px12_in_base = img12_base.pixel(70, 100);
        assert!(px12_in[0] < px12_in_base[0], "pixels inside moved matte must be darker: {px12_in:?} vs base {px12_in_base:?}");

        // Outside matte at frame 12: (e.g. x: 160, y: 100) -> byte identical
        let px12_out = img12.pixel(160, 100);
        let px12_out_base = img12_base.pixel(160, 100);
        assert_eq!(px12_out, px12_out_base, "pixels outside matte must be byte-identical at frame 12");
    }

    #[test]
    fn gradient_linear_midpoint_is_between_120_and_135() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 100, "height": 100})).unwrap();
        s.execute("paint.setFill", &json!({"color": "#888888"})).unwrap();
        s.execute("paint.setStroke", &json!({"none": true})).unwrap();
        let layer = s.doc().unwrap().doc.layers[0].id.0;

        s.execute("fx.add", &json!({"row": layer, "kind": "gradient"})).unwrap();
        s.execute(
            "fx.key",
            &json!({
                "row": layer,
                "index": 0,
                "frame": 1,
                "params": {"type": "linear", "color0": "#000000", "color1": "#ffffff", "x0": 0.0, "y0": 0.5, "x1": 1.0, "y1": 0.5}
            }),
        )
        .unwrap();

        let doc = &s.doc().unwrap().doc;
        let rect = doc.artboards[0].rect;
        let fd = at_frame(doc, 1);
        let img = vectorcraft_render::Renderer::new().render_region(&fd, rect, 1.0, false);
        let p = img.pixel(50, 50);
        assert!((120..=135).contains(&p[0]), "gradient linear midpoint must be between 120 and 135, got {:?}", p[0]);
    }

    #[test]
    fn bloom_white_dot_on_dark_grey_spreads_light_leaving_distant_grey_unchanged() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 200, "height": 200})).unwrap();
        s.execute("paint.setFill", &json!({"color": "#303030"})).unwrap();
        s.execute("paint.setStroke", &json!({"none": true})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 95, "y": 95, "width": 10, "height": 10})).unwrap();
        s.execute("paint.setFill", &json!({"color": "#ffffff"})).unwrap();
        s.execute("paint.setStroke", &json!({"none": true})).unwrap();
        let layer = s.doc().unwrap().doc.layers[0].id.0;

        let rect = s.doc().unwrap().doc.artboards[0].rect;
        let fd_orig = at_frame(&s.doc().unwrap().doc, 1);
        let img_orig = vectorcraft_render::Renderer::new().render_region(&fd_orig, rect, 1.0, false);

        s.execute("fx.add", &json!({"row": layer, "kind": "bloom"})).unwrap();
        s.execute(
            "fx.key",
            &json!({
                "row": layer,
                "index": 0,
                "frame": 1,
                "params": {"threshold": 70.0, "radius": 15.0, "amount": 80.0}
            }),
        )
        .unwrap();

        let fd_bloom = at_frame(&s.doc().unwrap().doc, 1);
        let img_bloom = vectorcraft_render::Renderer::new().render_region(&fd_bloom, rect, 1.0, false);

        // Pixel near the white dot (e.g. at x: 88, y: 100) gets brighter due to bloom
        let p_near_orig = img_orig.pixel(88, 100);
        let p_near_bloom = img_bloom.pixel(88, 100);
        assert!(p_near_bloom[0] > p_near_orig[0], "light must spread around white dot: {p_near_bloom:?} vs {p_near_orig:?}");

        // Distant pixel (e.g. at x: 10, y: 10) stays unchanged
        let p_far_orig = img_orig.pixel(10, 10);
        let p_far_bloom = img_bloom.pixel(10, 10);
        assert_eq!(p_far_bloom, p_far_orig, "distant grey pixel must stay unchanged");
    }

    #[test]
    fn motion_blur_horizontal_streak_on_moving_row_still_row_unchanged() {
        // 1. Moving row: moving 50 pt/frame to the right
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();
        s.execute("anim.setFrames", &json!({"frames": 12})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 95, "y": 95, "width": 10, "height": 10})).unwrap();
        s.execute("paint.setFill", &json!({"color": "#ffffff"})).unwrap();
        s.execute("paint.setStroke", &json!({"none": true})).unwrap();
        let moving_row = s.doc().unwrap().doc.layers[0].id.0;

        s.execute("peg.add", &json!({"rows": [moving_row]})).unwrap();
        s.execute("peg.key", &json!({"peg": 0, "frame": 1, "x": 0})).unwrap();
        s.execute("peg.key", &json!({"peg": 0, "frame": 2, "x": 50})).unwrap();

        s.execute("fx.add", &json!({"row": moving_row, "kind": "motionBlur"})).unwrap();
        s.execute(
            "fx.key",
            &json!({
                "row": moving_row,
                "index": 0,
                "frame": 1,
                "params": {"amount": 100.0, "samples": 16}
            }),
        )
        .unwrap();

        let rect = s.doc().unwrap().doc.artboards[0].rect;
        let fd = at_frame(&s.doc().unwrap().doc, 1);
        let img = vectorcraft_render::Renderer::new().render_region(&fd, rect, 1.0, false);

        let w = img.width;
        let h = img.height;
        let x_extent = (0..w).filter(|&x| (0..h).any(|y| img.pixel(x, y)[3] > 10)).count();
        let y_extent = (0..h).filter(|&y| (0..w).any(|x| img.pixel(x, y)[3] > 10)).count();
        assert!(x_extent >= y_extent * 2, "horizontal streak expected: x_extent {x_extent} vs y_extent {y_extent}");

        // 2. Still row: unchanged
        let mut s_still = Session::new();
        s_still.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        s_still.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s_still.execute("shape.rectangle", &json!({"x": 20, "y": 20, "width": 40, "height": 40})).unwrap();
        s_still.execute("paint.setFill", &json!({"color": "#ffffff"})).unwrap();
        s_still.execute("paint.setStroke", &json!({"none": true})).unwrap();
        let still_row = s_still.doc().unwrap().doc.layers[0].id.0;

        let fd_still_orig = at_frame(&s_still.doc().unwrap().doc, 1);
        let rect_still = s_still.doc().unwrap().doc.artboards[0].rect;
        let img_still_orig = vectorcraft_render::Renderer::new().render_region(&fd_still_orig, rect_still, 1.0, false);

        s_still.execute("fx.add", &json!({"row": still_row, "kind": "motionBlur"})).unwrap();
        s_still
            .execute(
                "fx.key",
                &json!({
                    "row": still_row,
                    "index": 0,
                    "frame": 1,
                    "params": {"amount": 100.0, "samples": 8}
                }),
            )
            .unwrap();

        let fd_still_blur = at_frame(&s_still.doc().unwrap().doc, 1);
        let img_still_blur = vectorcraft_render::Renderer::new().render_region(&fd_still_blur, rect_still, 1.0, false);
        assert_eq!(img_still_blur.pixels, img_still_orig.pixels, "still row must remain unchanged under motion blur");
    }

    #[test]
    fn old_file_without_matte_deserializes_and_renders_identically() {
        let json_old = json!({
            "rows": [
                {
                    "row": 1,
                    "effects": [
                        {
                            "kind": "blur",
                            "keys": {
                                "1": {"radius": 5.0}
                            }
                        }
                    ]
                }
            ]
        });
        let list: RowFxList = serde_json::from_value(json_old).unwrap();
        assert_eq!(list.rows.len(), 1);
        assert_eq!(list.rows[0].effects[0].matte, None);
        assert_eq!(list.rows[0].effects[0].kind, FxKind::Blur);
    }

    #[test]
    fn bad_inputs_matte_or_type_or_color_return_err_without_panic() {
        let (mut s, layer) = session_with_rect();
        // Matte = same row
        assert!(s.execute("fx.add", &json!({"row": layer, "kind": "tone", "matte": layer})).is_err());
        // Matte = non-existent row
        assert!(s.execute("fx.add", &json!({"row": layer, "kind": "tone", "matte": 999_999})).is_err());

        s.execute("fx.add", &json!({"row": layer, "kind": "tone"})).unwrap();
        // Set matte = same row
        assert!(s.execute("fx.set", &json!({"row": layer, "index": 0, "matte": layer})).is_err());
        // Set matte = non-existent row
        assert!(s.execute("fx.set", &json!({"row": layer, "index": 0, "matte": 999_999})).is_err());

        // Invalid colors on tone
        assert!(s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 1, "params": {"color": "red"}})).is_err());
        assert!(s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 1, "params": {"color": "#12345"}})).is_err());

        // Invalid opacity
        assert!(s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 1, "params": {"opacity": -10.0}})).is_err());
        assert!(s.execute("fx.key", &json!({"row": layer, "index": 0, "frame": 1, "params": {"opacity": 150.0}})).is_err());

        // Invalid gradient type and colors
        s.execute("fx.add", &json!({"row": layer, "kind": "gradient"})).unwrap();
        assert!(s.execute("fx.key", &json!({"row": layer, "index": 1, "frame": 1, "params": {"type": "diagonal"}})).is_err());
        assert!(s.execute("fx.key", &json!({"row": layer, "index": 1, "frame": 1, "params": {"color0": "nothex"}})).is_err());
        assert!(s.execute("fx.key", &json!({"row": layer, "index": 1, "frame": 1, "params": {"color1": "nothex"}})).is_err());

        // Invalid motion blur samples
        s.execute("fx.add", &json!({"row": layer, "kind": "motionBlur"})).unwrap();
        assert!(s.execute("fx.key", &json!({"row": layer, "index": 2, "frame": 1, "params": {"samples": 1}})).is_err());
        assert!(s.execute("fx.key", &json!({"row": layer, "index": 2, "frame": 1, "params": {"samples": 50}})).is_err());
        assert!(s.execute("fx.key", &json!({"row": layer, "index": 2, "frame": 1, "params": {"amount": -1.0}})).is_err());
    }

    #[test]
    fn matte_ellipse_leaves_pixels_inside_bbox_outside_ellipse_unchanged_and_center_changed() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();
        s.execute("anim.setFrames", &json!({"frames": 12})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 200, "height": 200})).unwrap();
        s.execute("paint.setFill", &json!({"color": "#888888"})).unwrap();
        s.execute("paint.setStroke", &json!({"none": true})).unwrap();
        let target_row = s.doc().unwrap().doc.layers[0].id.0;

        let rect = s.doc().unwrap().doc.artboards[0].rect;
        let fd_base = at_frame(&s.doc().unwrap().doc, 1);
        let img_base = vectorcraft_render::Renderer::new().render_region(&fd_base, rect, 1.0, false);

        let matte_row = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
        s.execute("layer.setCurrent", &json!({"id": matte_row})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        // Ellipse with bbox x: 50..150, y: 50..150, center at (100, 100), radius 50
        s.execute("shape.ellipse", &json!({"x": 50, "y": 50, "width": 100, "height": 100})).unwrap();
        s.execute("paint.setFill", &json!({"color": "#ffffff"})).unwrap();
        s.execute("paint.setStroke", &json!({"none": true})).unwrap();

        s.execute("fx.add", &json!({"row": target_row, "kind": "tone", "matte": matte_row})).unwrap();
        s.execute("fx.key", &json!({"row": target_row, "index": 0, "frame": 1, "params": {"opacity": 50.0, "color": "#000000", "blur": 0.0}}))
            .unwrap();

        let fd = at_frame(&s.doc().unwrap().doc, 1);
        let img = vectorcraft_render::Renderer::new().render_region(&fd, rect, 1.0, false);

        // Center pixel (100, 100) is inside ellipse: must be changed (darker)
        let px_center = img.pixel(100, 100);
        let px_center_base = img_base.pixel(100, 100);
        assert!(px_center[0] < px_center_base[0], "center pixel inside ellipse must change: {px_center:?} vs base {px_center_base:?}");

        // Corner pixel (55, 55) is inside bbox (50..150) but outside ellipse: must NOT change
        let px_corner = img.pixel(55, 55);
        let px_corner_base = img_base.pixel(55, 55);
        assert_eq!(px_corner, px_corner_base, "pixel inside bbox but outside ellipse must not change");
    }

    #[test]
    fn matte_at_50_percent_opacity_gives_half_the_center_change_within_10_percent() {
        let make_session = |matte_opacity: f64| {
            let mut s = Session::new();
            s.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();
            s.execute("anim.setFrames", &json!({"frames": 12})).unwrap();
            s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
            s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 200, "height": 200})).unwrap();
            s.execute("paint.setFill", &json!({"color": "#888888"})).unwrap();
            s.execute("paint.setStroke", &json!({"none": true})).unwrap();
            let target_row = s.doc().unwrap().doc.layers[0].id.0;

            let matte_row = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
            s.execute("layer.setCurrent", &json!({"id": matte_row})).unwrap();
            s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
            s.execute("shape.rectangle", &json!({"x": 50, "y": 50, "width": 100, "height": 100})).unwrap();
            s.execute("paint.setFill", &json!({"color": "#ffffff"})).unwrap();
            s.execute("paint.setStroke", &json!({"none": true})).unwrap();
            if (matte_opacity - 100.0).abs() > 1e-4 {
                s.execute("transparency.set", &json!({"ids": [matte_row], "opacity": matte_opacity})).unwrap();
            }

            s.execute("fx.add", &json!({"row": target_row, "kind": "tone", "matte": matte_row})).unwrap();
            s.execute("fx.key", &json!({"row": target_row, "index": 0, "frame": 1, "params": {"opacity": 50.0, "color": "#000000", "blur": 0.0}}))
                .unwrap();
            (s, target_row, matte_row)
        };

        let rect = kurbo::Rect::new(0.0, 0.0, 200.0, 200.0);

        // Baseline (no effect; matte row removed as it only serves as a matte)
        let (s_base, _, matte_row) = make_session(100.0);
        let mut d_base = (*s_base.doc().unwrap().doc).clone();
        RowFxList::default().store(&mut d_base);
        let _ = d_base.remove(vectorcraft_doc::NodeId(matte_row));
        let fd_base = at_frame(&d_base, 1);
        let img_base = vectorcraft_render::Renderer::new().render_region(&fd_base, rect, 1.0, false);
        let base_val = img_base.pixel(100, 100)[0] as f64;

        // 100% matte
        let (s_full, _, _) = make_session(100.0);
        let fd_full = at_frame(&s_full.doc().unwrap().doc, 1);
        let img_full = vectorcraft_render::Renderer::new().render_region(&fd_full, rect, 1.0, false);
        let full_val = img_full.pixel(100, 100)[0] as f64;
        let delta_full = (base_val - full_val).abs();
        assert!(delta_full > 10.0, "full matte must produce significant change: delta_full={delta_full}");

        // 50% matte
        let (s_half, _, _) = make_session(50.0);
        let fd_half = at_frame(&s_half.doc().unwrap().doc, 1);
        let img_half = vectorcraft_render::Renderer::new().render_region(&fd_half, rect, 1.0, false);
        let half_val = img_half.pixel(100, 100)[0] as f64;
        let delta_half = (base_val - half_val).abs();

        let ratio = delta_half / delta_full;
        assert!(
            (ratio - 0.5).abs() <= 0.10,
            "50% matte change ({delta_half}) should be roughly half full change ({delta_full}): ratio={ratio:.3} (expected 0.40..0.60)"
        );
    }

    #[test]
    fn at_frame_and_unposed_leaves_document_identical_node_count_visibility_and_children() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();
        s.execute("anim.setFrames", &json!({"frames": 12})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        let target_row = s.doc().unwrap().doc.layers[0].id.0;

        let matte_row = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
        s.execute("layer.setCurrent", &json!({"id": matte_row})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("shape.ellipse", &json!({"x": 20, "y": 20, "width": 40, "height": 40})).unwrap();

        s.execute("fx.add", &json!({"row": target_row, "kind": "tone", "matte": matte_row})).unwrap();
        s.execute("fx.key", &json!({"row": target_row, "index": 0, "frame": 1, "params": {"opacity": 50.0, "color": "#000000"}})).unwrap();

        fn dump_tree(nodes: &[std::sync::Arc<vectorcraft_doc::Node>]) -> Vec<(u64, bool, Vec<u64>)> {
            let mut out = Vec::new();
            for n in nodes {
                let ch_ids = n.children().map(|ch| ch.iter().map(|c| c.id.0).collect()).unwrap_or_default();
                out.push((n.id.0, n.visible, ch_ids));
                if let Some(ch) = n.children() {
                    out.extend(dump_tree(ch));
                }
            }
            out
        }

        let doc_before = unposed(&s.doc().unwrap().doc);
        let tree_before = dump_tree(&doc_before.layers);

        // Advance to frame 1
        let frame_doc = at_frame(&doc_before, 1);
        let tree_frame = dump_tree(&frame_doc.layers);
        // The frame doc has ghosts added and matte row hidden, so it differs
        assert_ne!(tree_before, tree_frame, "frame_doc must reflect posed/effects state");

        // Unpose
        let rest_doc = unposed(&frame_doc);
        let tree_rest = dump_tree(&rest_doc.layers);

        assert_eq!(tree_before, tree_rest, "document after unposed must be completely identical to before at_frame");
    }
}
