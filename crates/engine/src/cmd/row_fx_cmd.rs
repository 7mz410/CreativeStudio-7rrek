//! 7rrek (Creative Studio): commands for timeline row effects (`fx.add`, `fx.key`, `fx.removeKey`,
//! `fx.remove`, `fx.list`).

use serde_json::{Value, json};

use super::anim::{Anim, frame_param, row_param, show};
use super::row_fx::{FxKind, MAX_ROW_EFFECTS, RowEffect, RowFx, RowFxList};
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "fx.add",
            "Add Row Effect",
            [],
            None,
            "{row?: layer id (default: the current layer's row), kind: \"blur\"|\"glow\"|\"dropShadow\"|\"colorAdjust\"|\"tint\"} adds an effect to the row → {row, index, kind}",
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
    let kind_str = str_param(p, "kind").ok_or_else(|| bad(C, "`kind` is required (blur, glow, dropShadow, colorAdjust, tint)"))?;
    let kind =
        FxKind::parse(kind_str).ok_or_else(|| bad(C, format!("unknown kind {kind_str:?} (expected blur, glow, dropShadow, colorAdjust, tint)")))?;

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
    row_fx.effects.push(RowEffect::new(kind));

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
}
