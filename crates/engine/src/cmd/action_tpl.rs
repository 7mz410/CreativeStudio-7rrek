//! 7rrek (Creative Studio): action templates, as in Harmony. A span of a row's motion (exposures,
//! bone, peg, mesh, curve and row effect keys, tweens) saved by name and pasted onto another
//! character, matched by names and order, never ids.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vectorcraft_doc::{Document, Node, NodeId};

use super::anim::{Anim, row_param, show};
use super::curve::Curves;
use super::mesh::Meshes;
use super::peg::{PegKey, Pegs};
use super::rig::{Pose, Rig};
use super::row_fx::RowFxList;
use super::*;

pub const EXTENSION: &str = "7rrekaction";
const KEY: &str = "actions";
/// Most action templates a document takes.
pub const MAX_ACTIONS: usize = 256;
/// Longest action template (frames).
pub const MAX_LENGTH: u32 = 10_000;
/// Largest import file allowed (16 MB).
const MAX_IMPORT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct MeshTrack {
    pub index: usize,
    pub keys: BTreeMap<u32, Vec<[f64; 2]>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CurveTrack {
    pub index: usize,
    pub keys: BTreeMap<u32, Vec<[f64; 2]>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct RowFxTrack {
    pub index: usize,
    pub kind: String,
    pub keys: BTreeMap<u32, Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Tracks {
    /// Drawing name by relative frame (0..length-1).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub exposures: BTreeMap<u32, String>,

    /// Joint name -> (relative frame -> angle).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub bones: BTreeMap<String, BTreeMap<u32, f64>>,

    /// Skeleton pose eases by relative frame.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub bone_eases: BTreeMap<u32, [f64; 4]>,

    /// Peg keys by relative frame.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub peg: BTreeMap<u32, PegKey>,

    /// Mesh keys by node index under row.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub meshes: Vec<MeshTrack>,

    /// Curve keys by node index under row.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub curves: Vec<CurveTrack>,

    /// Row effect keys by effect index.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub row_fx: Vec<RowFxTrack>,

    /// Relative frames that have tweens.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub tweens: BTreeSet<u32>,

    /// Relative frame -> tween cubic-bezier ease.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub eases: BTreeMap<u32, [f64; 4]>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Action {
    pub name: String,
    pub length: u32,
    pub tracks: Tracks,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Actions {
    pub actions: Vec<Action>,
}

impl Action {
    /// Every frame inside the action and the action within [`MAX_LENGTH`], so pasting at a frame up
    /// to [`MAX_LENGTH`] never overflows (files and documents are untrusted).
    pub fn valid(&self) -> bool {
        let t = &self.tracks;
        let ok = |f: &u32| *f < self.length;
        self.length <= MAX_LENGTH
            && t.exposures.keys().all(ok)
            && t.bones.values().all(|k| k.keys().all(ok))
            && t.bone_eases.keys().all(ok)
            && t.peg.keys().all(ok)
            && t.meshes.iter().all(|m| m.keys.keys().all(ok))
            && t.curves.iter().all(|c| c.keys.keys().all(ok))
            && t.row_fx.iter().all(|r| r.keys.keys().all(ok))
            && t.tweens.iter().all(ok)
            && t.eases.keys().all(ok)
    }
}

impl Actions {
    pub fn of(doc: &Document) -> Self {
        let a: Actions = doc.unknown.get(KEY).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
        if a.actions.len() <= MAX_ACTIONS && a.actions.iter().all(Action::valid) { a } else { Actions::default() }
    }

    pub fn store(&self, doc: &mut Document) {
        if self.actions.is_empty() {
            doc.unknown.remove(KEY);
        } else if let Ok(v) = serde_json::to_value(self) {
            doc.unknown.insert(KEY.into(), v);
        }
    }
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "action.save",
            "Save Action",
            [],
            None,
            "{row?: layer id (default: current layer), from: whole number >= 1, to: whole number >= from, name: text} saves a span of a row's motion as an action template → {name, length, tracks: {exposures, bones, peg, meshes, curves, rowFx, tweens, eases}}",
            has_doc,
            save
        ),
        cmd!(
            query "action.list",
            "List Actions",
            [],
            None,
            "{} lists all action templates in the document → {actions: [{name, length}]}",
            has_doc,
            list
        ),
        cmd!(
            "action.paste",
            "Paste Action",
            [],
            None,
            "{row?: layer id (default: current layer), name: text, at?: whole number >= 1 (default: current frame)} pastes an action template onto a row, matching by names and order → {row, at, skipped: [text]}",
            has_doc,
            paste
        ),
        cmd!("action.remove", "Remove Action", [], None, "{name: text} removes an action template by name → {name}", has_doc, remove),
        cmd!(
            "action.export",
            "Export Action",
            [],
            None,
            "{name: text, path: file (.7rrekaction)} exports an action template to a file → {name, path, bytes}",
            has_doc,
            export
        ),
        cmd!(
            "action.import",
            "Import Action",
            [],
            None,
            "{path: file (.7rrekaction)} imports an action template from a file → {name, length, path}",
            has_doc,
            import
        ),
    ]
}

/// Pre-order node IDs in `n`'s tree, `n` included.
fn ids_in(n: &Node) -> Vec<u64> {
    let mut v = Vec::new();
    n.walk(&mut |c| v.push(c.id.0));
    v
}

fn save(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "action.save";
    let row = row_param(s, p, C)?;
    let from =
        p.get("from").and_then(Value::as_u64).and_then(|v| u32::try_from(v).ok()).ok_or_else(|| bad(C, "`from` must be a whole number >= 1"))?;
    if from < 1 {
        return Err(bad(C, "`from` must be >= 1"));
    }
    let to =
        p.get("to").and_then(Value::as_u64).and_then(|v| u32::try_from(v).ok()).ok_or_else(|| bad(C, "`to` must be a whole number >= `from`"))?;
    if to < from {
        return Err(bad(C, "`to` must be >= `from`"));
    }
    let length = to - from + 1;
    if length > MAX_LENGTH {
        return Err(bad(C, format!("length {length} exceeds maximum of {MAX_LENGTH}")));
    }
    let name = str_param(p, "name").map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| bad(C, "`name` is required"))?.to_string();

    let doc = &s.doc()?.doc;
    let anim = Anim::of(doc);

    // 1. Exposures & drawings
    let mut exposures = BTreeMap::new();
    if let Some(row_exp) = anim.exposures.get(&row.0) {
        for (&f, &drawing_id) in row_exp {
            if f < from || f > to {
                continue;
            }
            if let Some(drawing_name) = doc.node(NodeId(drawing_id)).and_then(|n| n.name.as_deref()).map(str::trim).filter(|s| !s.is_empty()) {
                exposures.insert(f - from, drawing_name.to_string());
            }
        }
    }
    if let Some(row_node) = doc.node(row) {
        for (f, k_id) in anim.row_keys(row_node) {
            if f < from || f > to || exposures.contains_key(&(f - from)) {
                continue;
            }
            if let Some(drawing_name) = doc.node(k_id).and_then(|n| n.name.as_deref()).map(str::trim).filter(|s| !s.is_empty()) {
                exposures.insert(f - from, drawing_name.to_string());
            }
        }
    }

    // 2. Bones
    let rig = Rig::of(doc);
    let mut bones = BTreeMap::new();
    let mut bone_eases = BTreeMap::new();
    if let Some(sk) = rig.skeletons.get(&row.0) {
        for (&f, pose) in &sk.keys {
            if f >= from && f <= to {
                let rel_f = f - from;
                for (j_idx, joint) in sk.joints.iter().enumerate() {
                    if let Some(&angle) = pose.angles.get(j_idx) {
                        bones.entry(joint.name.clone()).or_insert_with(BTreeMap::new).insert(rel_f, angle);
                    }
                }
                if let Some(ease) = pose.ease {
                    bone_eases.insert(rel_f, ease);
                }
            }
        }
    }

    // 3. Peg
    let pegs = Pegs::of(doc);
    let mut peg_keys = BTreeMap::new();
    if let Some(peg) = pegs.pegs.iter().find(|p| p.rows.contains(&row.0)) {
        for (&f, key) in &peg.keys {
            if f >= from && f <= to {
                peg_keys.insert(f - from, *key);
            }
        }
    }

    // 4. Meshes and Curves
    let mut mesh_tracks = Vec::new();
    let mut curve_tracks = Vec::new();
    let meshes = Meshes::of(doc);
    let curves = Curves::of(doc);
    if let Some(row_node) = doc.node(row) {
        let ids = ids_in(row_node);
        for (idx, &id) in ids.iter().enumerate() {
            if let Some(m) = meshes.meshes.iter().find(|m| m.node == id) {
                let mut keys = BTreeMap::new();
                for (&f, pts) in &m.keys {
                    if f >= from && f <= to {
                        keys.insert(f - from, pts.clone());
                    }
                }
                if !keys.is_empty() {
                    mesh_tracks.push(MeshTrack { index: idx, keys });
                }
            }
            if let Some(c) = curves.curves.iter().find(|c| c.node == id) {
                let mut keys = BTreeMap::new();
                for (&f, pts) in &c.keys {
                    if f >= from && f <= to {
                        keys.insert(f - from, pts.clone());
                    }
                }
                if !keys.is_empty() {
                    curve_tracks.push(CurveTrack { index: idx, keys });
                }
            }
        }
    }

    // 5. Row Effects
    let fx_list = RowFxList::of(doc);
    let mut fx_tracks = Vec::new();
    if let Some(r_fx) = fx_list.rows.iter().find(|r| r.row == row.0) {
        for (idx, effect) in r_fx.effects.iter().enumerate() {
            let mut keys = BTreeMap::new();
            for (&f, val) in &effect.keys {
                if f >= from && f <= to {
                    keys.insert(f - from, val.clone());
                }
            }
            if !keys.is_empty() {
                fx_tracks.push(RowFxTrack { index: idx, kind: effect.kind.as_str().to_string(), keys });
            }
        }
    }

    // 6. Tweens and Eases
    let mut tweens = BTreeSet::new();
    let mut eases = BTreeMap::new();
    if let Some(row_node) = doc.node(row) {
        for (f, k_id) in anim.row_keys(row_node) {
            if f >= from && f <= to {
                let rel_f = f - from;
                if anim.tweens.contains(&k_id.0) {
                    tweens.insert(rel_f);
                }
                if let Some(ease) = anim.eases.get(&k_id.0) {
                    eases.insert(rel_f, *ease);
                }
            }
        }
    }

    let action = Action {
        name: name.clone(),
        length,
        tracks: Tracks { exposures, bones, bone_eases, peg: peg_keys, meshes: mesh_tracks, curves: curve_tracks, row_fx: fx_tracks, tweens, eases },
    };

    let track_counts = json!({
        "exposures": action.tracks.exposures.len(),
        "bones": action.tracks.bones.len(),
        "peg": action.tracks.peg.len(),
        "meshes": action.tracks.meshes.len(),
        "curves": action.tracks.curves.len(),
        "rowFx": action.tracks.row_fx.len(),
        "tweens": action.tracks.tweens.len(),
        "eases": action.tracks.eases.len(),
    });

    s.edit("Save Action", |d, _| {
        let mut actions = Actions::of(d);
        if actions.actions.len() >= MAX_ACTIONS && !actions.actions.iter().any(|a| a.name == action.name) {
            return Err(bad(C, format!("maximum of {MAX_ACTIONS} actions reached")));
        }
        if let Some(pos) = actions.actions.iter().position(|a| a.name == action.name) {
            if let Some(slot) = actions.actions.get_mut(pos) {
                *slot = action.clone();
            }
        } else {
            actions.actions.push(action.clone());
        }
        actions.store(d);
        Ok(())
    })?;

    Ok(json!({
        "name": name,
        "length": length,
        "tracks": track_counts,
    }))
}

fn list(s: &mut Session, _p: &Value) -> Result<Value> {
    let actions: Vec<Value> = Actions::of(&s.doc()?.doc).actions.iter().map(|a| json!({ "name": a.name, "length": a.length })).collect();
    Ok(json!({ "actions": actions }))
}

fn paste(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "action.paste";
    let row = row_param(s, p, C)?;
    let name = str_param(p, "name").map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| bad(C, "`name` is required"))?;
    let doc = &s.doc()?.doc;
    let anim = Anim::of(doc);
    let at = match p.get("at") {
        None | Some(Value::Null) => anim.current.clamp(1, MAX_LENGTH),
        Some(v) => v
            .as_u64()
            .filter(|f| (1..=MAX_LENGTH as u64).contains(f))
            .map(|f| f as u32)
            .ok_or_else(|| bad(C, format!("`at` must be a whole number 1..{MAX_LENGTH}")))?,
    };
    let actions = Actions::of(doc);
    let action = actions.actions.iter().find(|a| a.name == name).cloned().ok_or_else(|| bad(C, format!("action `{name}` not found")))?;

    let mut skipped = Vec::new();
    s.edit("Paste Action", |d, _| {
        let mut anim = Anim::of(d);
        let (row_sublayers, row_keys, ids) = {
            let row_node = d.node(row).ok_or_else(|| bad(C, "no such layer"))?;
            let sublayers: Vec<(u64, String)> =
                row_node.children().into_iter().flatten().filter(|c| c.is_layer()).map(|c| (c.id.0, c.name.clone().unwrap_or_default())).collect();
            let keys = anim.row_keys(row_node);
            let ids = ids_in(row_node);
            (sublayers, keys, ids)
        };

        // 1. Exposures
        for (rel_f, draw_name) in &action.tracks.exposures {
            let target_frame = at + rel_f;
            if let Some((sub_id, _)) = row_sublayers.iter().find(|(_, name)| name == draw_name) {
                anim.exposures.entry(row.0).or_default().insert(target_frame, *sub_id);
            } else {
                let s_item = format!("drawing:{draw_name}");
                if !skipped.contains(&s_item) {
                    skipped.push(s_item);
                }
            }
        }

        // 2. Tweens and eases
        for rel_f in &action.tracks.tweens {
            let target_frame = at + rel_f;
            if let Some((_, sub_id)) = row_keys.iter().find(|(f, _)| *f == target_frame) {
                anim.tweens.insert(sub_id.0);
            }
        }
        for (rel_f, ease) in &action.tracks.eases {
            let target_frame = at + rel_f;
            if let Some((_, sub_id)) = row_keys.iter().find(|(f, _)| *f == target_frame) {
                anim.eases.insert(sub_id.0, *ease);
            }
        }

        if at + action.length > anim.frames {
            anim.frames = (at + action.length).clamp(1, MAX_LENGTH);
        }
        anim.store(d);

        // 3. Bones
        if !action.tracks.bones.is_empty() {
            let mut rig = Rig::of(d);
            if let Some(sk) = rig.skeletons.get_mut(&row.0) {
                let mut joint_indices = HashMap::new();
                for (j_idx, j) in sk.joints.iter().enumerate() {
                    joint_indices.insert(j.name.clone(), j_idx);
                }

                let mut frames_set = BTreeSet::new();
                for (b_name, b_keys) in &action.tracks.bones {
                    if joint_indices.contains_key(b_name) {
                        for rel_f in b_keys.keys() {
                            frames_set.insert(*rel_f);
                        }
                    } else {
                        let s_item = format!("bone:{b_name}");
                        if !skipped.contains(&s_item) {
                            skipped.push(s_item);
                        }
                    }
                }

                for rel_f in frames_set {
                    let target_frame = at + rel_f;
                    let mut angles = sk.pose_at(target_frame).unwrap_or_else(|| vec![0.0; sk.joints.len()]);
                    if angles.len() < sk.joints.len() {
                        angles.resize(sk.joints.len(), 0.0);
                    }
                    for (b_name, b_keys) in &action.tracks.bones {
                        let slot = joint_indices.get(b_name).and_then(|&idx| angles.get_mut(idx));
                        if let (Some(slot), Some(&ang)) = (slot, b_keys.get(&rel_f)) {
                            *slot = ang;
                        }
                    }
                    let ease = action.tracks.bone_eases.get(&rel_f).copied();
                    sk.keys.insert(target_frame, Pose { angles, ease });
                }
                rig.store(d);
            } else {
                for b_name in action.tracks.bones.keys() {
                    let s_item = format!("bone:{b_name}");
                    if !skipped.contains(&s_item) {
                        skipped.push(s_item);
                    }
                }
            }
        }

        // 4. Peg
        if !action.tracks.peg.is_empty() {
            let mut pegs = Pegs::of(d);
            if let Some(peg) = pegs.pegs.iter_mut().find(|p| p.rows.contains(&row.0)) {
                for (rel_f, key) in &action.tracks.peg {
                    peg.keys.insert(at + rel_f, *key);
                }
                pegs.store(d);
            } else {
                skipped.push("peg".to_string());
            }
        }

        // 5. Meshes
        if !action.tracks.meshes.is_empty() {
            let mut meshes = Meshes::of(d);
            for m_track in &action.tracks.meshes {
                let target_node_id = ids.get(m_track.index).copied();
                if let Some(target_id) = target_node_id.and_then(|id| meshes.meshes.iter_mut().find(|m| m.node == id)) {
                    for (rel_f, pts) in &m_track.keys {
                        target_id.keys.insert(at + rel_f, pts.clone());
                    }
                } else {
                    skipped.push(format!("mesh:{}", m_track.index));
                }
            }
            meshes.store(d);
        }

        // 6. Curves
        if !action.tracks.curves.is_empty() {
            let mut curves = Curves::of(d);
            for c_track in &action.tracks.curves {
                let target_node_id = ids.get(c_track.index).copied();
                if let Some(target_curve) = target_node_id.and_then(|id| curves.curves.iter_mut().find(|c| c.node == id)) {
                    for (rel_f, pts) in &c_track.keys {
                        target_curve.keys.insert(at + rel_f, pts.clone());
                    }
                } else {
                    skipped.push(format!("curve:{}", c_track.index));
                }
            }
            curves.store(d);
        }

        // 7. Row effects
        if !action.tracks.row_fx.is_empty() {
            let mut fx_list = RowFxList::of(d);
            if let Some(r_fx) = fx_list.rows.iter_mut().find(|r| r.row == row.0) {
                for fx_track in &action.tracks.row_fx {
                    if let Some(target_eff) = r_fx.effects.get_mut(fx_track.index) {
                        if target_eff.kind.as_str() == fx_track.kind {
                            for (rel_f, val) in &fx_track.keys {
                                target_eff.keys.insert(at + rel_f, val.clone());
                            }
                        } else {
                            skipped.push(format!("rowFx:{}", fx_track.index));
                        }
                    } else {
                        skipped.push(format!("rowFx:{}", fx_track.index));
                    }
                }
                fx_list.store(d);
            } else {
                for fx_track in &action.tracks.row_fx {
                    skipped.push(format!("rowFx:{}", fx_track.index));
                }
            }
        }

        Ok(())
    })?;

    let current = Anim::of(&s.doc()?.doc).current;
    show(s, current)?;
    Ok(json!({
        "row": row.0,
        "at": at,
        "skipped": skipped,
    }))
}

fn remove(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "action.remove";
    let name = str_param(p, "name").map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| bad(C, "`name` is required"))?;

    s.edit("Remove Action", |d, _| {
        let mut actions = Actions::of(d);
        let pos = actions.actions.iter().position(|a| a.name == name).ok_or_else(|| bad(C, format!("action `{name}` not found")))?;
        actions.actions.remove(pos);
        actions.store(d);
        Ok(())
    })?;

    Ok(json!({ "name": name }))
}

fn export(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "action.export";
    let name = str_param(p, "name").map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| bad(C, "`name` is required"))?;
    let path = str_param(p, "path").map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| bad(C, "`path` is required"))?;

    let actions = Actions::of(&s.doc()?.doc);
    let action = actions.actions.iter().find(|a| a.name == name).ok_or_else(|| bad(C, format!("action `{name}` not found")))?;

    let bytes = serde_json::to_vec_pretty(action).map_err(|e| bad(C, format!("failed to serialize action: {e}")))?;
    super::fileio::write_file(path, &bytes)?;
    Ok(json!({ "name": name, "path": path, "bytes": bytes.len() }))
}

fn import(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "action.import";
    let path = str_param(p, "path").map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| bad(C, "`path` is required"))?;

    let metadata = std::fs::metadata(path).map_err(|e| bad(C, format!("{path}: cannot read ({e})")))?;
    if metadata.len() > MAX_IMPORT_BYTES {
        return Err(bad(C, format!("{path}: file is too large ({} bytes, max 16 MB)", metadata.len())));
    }

    let bytes = super::fileio::read_file(path)?;
    let action: Action = serde_json::from_slice(&bytes).map_err(|e| bad(C, format!("{path}: corrupt action file ({e})")))?;
    if action.name.trim().is_empty() {
        return Err(bad(C, "action has empty name"));
    }
    if !action.valid() {
        return Err(bad(C, format!("{path}: the action's frames go past its length or past {MAX_LENGTH}")));
    }

    s.edit("Import Action", |d, _| {
        let mut actions = Actions::of(d);
        if actions.actions.len() >= MAX_ACTIONS && !actions.actions.iter().any(|a| a.name == action.name) {
            return Err(bad(C, format!("maximum of {MAX_ACTIONS} actions reached")));
        }
        if let Some(pos) = actions.actions.iter().position(|a| a.name == action.name) {
            if let Some(slot) = actions.actions.get_mut(pos) {
                *slot = action.clone();
            }
        } else {
            actions.actions.push(action.clone());
        }
        actions.store(d);
        Ok(())
    })?;

    Ok(json!({ "name": action.name, "length": action.length, "path": path }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_12_frame_blink_pasted_on_a_second_character_plays_the_same() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let char1_row = s.doc().unwrap().doc.layers[0].id.0;

        // Character 1 drawings: "open", "half", "closed"
        s.execute("anim.keyframe", &json!({"frame": 1, "row": char1_row})).unwrap();
        let k1 = Anim::of(&s.doc().unwrap().doc).key_at(&s.doc().unwrap().doc.layers[0], 1).unwrap();
        s.edit("name", |d, _| {
            d.node_mut(k1).unwrap().name = Some("open".into());
            Ok(())
        })
        .unwrap();

        s.execute("anim.keyframe", &json!({"frame": 2, "row": char1_row})).unwrap();
        let k2 = Anim::of(&s.doc().unwrap().doc).key_at(&s.doc().unwrap().doc.layers[0], 2).unwrap();
        s.edit("name", |d, _| {
            d.node_mut(k2).unwrap().name = Some("half".into());
            Ok(())
        })
        .unwrap();

        s.execute("anim.keyframe", &json!({"frame": 3, "row": char1_row})).unwrap();
        let k3 = Anim::of(&s.doc().unwrap().doc).key_at(&s.doc().unwrap().doc.layers[0], 3).unwrap();
        s.edit("name", |d, _| {
            d.node_mut(k3).unwrap().name = Some("closed".into());
            Ok(())
        })
        .unwrap();

        // Exposures on Char 1 across frames 1..=12
        s.execute("anim.expose", &json!({"row": char1_row, "frame": 1, "name": "open"})).unwrap();
        s.execute("anim.expose", &json!({"row": char1_row, "frame": 4, "name": "half"})).unwrap();
        s.execute("anim.expose", &json!({"row": char1_row, "frame": 6, "name": "closed"})).unwrap();
        s.execute("anim.expose", &json!({"row": char1_row, "frame": 8, "name": "half"})).unwrap();
        s.execute("anim.expose", &json!({"row": char1_row, "frame": 10, "name": "open"})).unwrap();

        // Second character (row 2)
        s.execute("layer.new", &json!({})).unwrap();
        let char2_row = s.doc().unwrap().doc.layers.iter().find(|l| l.id.0 != char1_row).unwrap().id.0;

        s.execute("anim.keyframe", &json!({"frame": 1, "row": char2_row})).unwrap();
        let doc = &s.doc().unwrap().doc;
        let c2_node = doc.node(NodeId(char2_row)).unwrap();
        let c2_k1 = Anim::of(doc).key_at(c2_node, 1).unwrap();
        s.edit("name", |d, _| {
            d.node_mut(c2_k1).unwrap().name = Some("open".into());
            Ok(())
        })
        .unwrap();

        s.execute("anim.keyframe", &json!({"frame": 2, "row": char2_row})).unwrap();
        let doc = &s.doc().unwrap().doc;
        let c2_node = doc.node(NodeId(char2_row)).unwrap();
        let c2_k2 = Anim::of(doc).key_at(c2_node, 2).unwrap();
        s.edit("name", |d, _| {
            d.node_mut(c2_k2).unwrap().name = Some("half".into());
            Ok(())
        })
        .unwrap();

        s.execute("anim.keyframe", &json!({"frame": 3, "row": char2_row})).unwrap();
        let doc = &s.doc().unwrap().doc;
        let c2_node = doc.node(NodeId(char2_row)).unwrap();
        let c2_k3 = Anim::of(doc).key_at(c2_node, 3).unwrap();
        s.edit("name", |d, _| {
            d.node_mut(c2_k3).unwrap().name = Some("closed".into());
            Ok(())
        })
        .unwrap();

        // Save action "blink" from frame 1 to 12
        let save_res = s.execute("action.save", &json!({"row": char1_row, "from": 1, "to": 12, "name": "blink"})).unwrap();
        assert_eq!(save_res["name"], "blink");
        assert_eq!(save_res["length"], 12);

        // Paste action on character 2
        let paste_res = s.execute("action.paste", &json!({"row": char2_row, "name": "blink", "at": 1})).unwrap();
        assert_eq!(paste_res["row"], char2_row);
        assert_eq!(paste_res["at"], 1);
        assert!(paste_res["skipped"].as_array().unwrap().is_empty());

        // Check each frame 1..=12: drawing name shown on both characters is identical
        let doc = s.doc().unwrap().doc.clone();
        let anim = Anim::of(&doc);
        let c1 = doc.node(NodeId(char1_row)).unwrap();
        let c2 = doc.node(NodeId(char2_row)).unwrap();
        let c1_f = anim.row_frames(c1);
        let c2_f = anim.row_frames(c2);

        for f in 1..=12 {
            let d1_id = c1_f.iter().rfind(|(fr, _)| *fr <= f).map(|(_, id)| *id).unwrap();
            let d2_id = c2_f.iter().rfind(|(fr, _)| *fr <= f).map(|(_, id)| *id).unwrap();
            let name1 = doc.node(d1_id).unwrap().name.as_deref().unwrap();
            let name2 = doc.node(d2_id).unwrap().name.as_deref().unwrap();
            assert_eq!(name1, name2, "mismatch at frame {f}: {name1} vs {name2}");
        }
    }

    #[test]
    fn bones_two_bone_arm_rotation_pastes_matching_angles() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let row1 = s.doc().unwrap().doc.layers[0].id.0;

        s.execute("rig.addJoint", &json!({"row": row1, "x": 0, "y": 0, "name": "arm"})).unwrap();
        s.execute("rig.addJoint", &json!({"row": row1, "x": 50, "y": 0, "parent": 0, "name": "forearm"})).unwrap();

        s.execute("rig.pose", &json!({"row": row1, "joint": 0, "angle": 30.0, "frame": 1})).unwrap();
        s.execute("rig.pose", &json!({"row": row1, "joint": 1, "angle": 45.0, "frame": 1})).unwrap();
        s.execute("rig.pose", &json!({"row": row1, "joint": 0, "angle": 60.0, "frame": 5})).unwrap();
        s.execute("rig.pose", &json!({"row": row1, "joint": 1, "angle": 90.0, "frame": 5})).unwrap();

        // Row 2 with same bone names
        s.execute("layer.new", &json!({})).unwrap();
        let row2 = s.doc().unwrap().doc.layers.iter().find(|l| l.id.0 != row1).unwrap().id.0;
        s.execute("rig.addJoint", &json!({"row": row2, "x": 10, "y": 10, "name": "arm"})).unwrap();
        s.execute("rig.addJoint", &json!({"row": row2, "x": 60, "y": 10, "parent": 0, "name": "forearm"})).unwrap();

        s.execute("action.save", &json!({"row": row1, "from": 1, "to": 5, "name": "arm_motion"})).unwrap();
        let res = s.execute("action.paste", &json!({"row": row2, "name": "arm_motion", "at": 1})).unwrap();
        assert!(res["skipped"].as_array().unwrap().is_empty());

        let doc = &s.doc().unwrap().doc;
        let rig = Rig::of(doc);
        let sk1 = &rig.skeletons[&row1];
        let sk2 = &rig.skeletons[&row2];
        for f in 1..=5 {
            assert_eq!(sk1.pose_at(f), sk2.pose_at(f), "angles mismatch at frame {f}");
        }
    }

    #[test]
    fn peg_keys_transfer_to_second_row_peg_or_skipped_without_peg() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let row1 = s.doc().unwrap().doc.layers[0].id.0;

        s.execute("peg.add", &json!({"rows": [row1]})).unwrap();
        s.execute("peg.key", &json!({"peg": 0, "frame": 1, "x": 10.0, "y": 20.0})).unwrap();
        s.execute("peg.key", &json!({"peg": 0, "frame": 5, "x": 50.0, "y": 60.0})).unwrap();

        // Row 2 with peg
        s.execute("layer.new", &json!({})).unwrap();
        let row2 = s.doc().unwrap().doc.layers.iter().find(|l| l.id.0 != row1).unwrap().id.0;
        s.execute("peg.add", &json!({"rows": [row2]})).unwrap();

        // Row 3 without peg
        s.execute("layer.new", &json!({})).unwrap();
        let row3 = s.doc().unwrap().doc.layers.iter().find(|l| l.id.0 != row1 && l.id.0 != row2).unwrap().id.0;

        s.execute("action.save", &json!({"row": row1, "from": 1, "to": 5, "name": "peg_action"})).unwrap();

        // Paste on row 2
        let res2 = s.execute("action.paste", &json!({"row": row2, "name": "peg_action", "at": 1})).unwrap();
        assert!(res2["skipped"].as_array().unwrap().is_empty());
        let pegs = Pegs::of(&s.doc().unwrap().doc);
        let peg2 = pegs.pegs.iter().find(|p| p.rows.contains(&row2)).unwrap();
        assert_eq!(peg2.keys.get(&1).map(|k| (k.x, k.y)), Some((10.0, 20.0)));
        assert_eq!(peg2.keys.get(&5).map(|k| (k.x, k.y)), Some((50.0, 60.0)));

        // Paste on row 3 (has no peg)
        let res3 = s.execute("action.paste", &json!({"row": row3, "name": "peg_action", "at": 1})).unwrap();
        let skipped: Vec<&str> = res3["skipped"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        assert!(skipped.contains(&"peg"));
    }

    #[test]
    fn paste_at_frame_20_shifts_keys_correctly() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let row1 = s.doc().unwrap().doc.layers[0].id.0;

        s.execute("peg.add", &json!({"rows": [row1]})).unwrap();
        s.execute("peg.key", &json!({"peg": 0, "frame": 1, "x": 5.0})).unwrap();
        s.execute("peg.key", &json!({"peg": 0, "frame": 4, "x": 25.0})).unwrap();

        s.execute("layer.new", &json!({})).unwrap();
        let row2 = s.doc().unwrap().doc.layers.iter().find(|l| l.id.0 != row1).unwrap().id.0;
        s.execute("peg.add", &json!({"rows": [row2]})).unwrap();

        s.execute("action.save", &json!({"row": row1, "from": 1, "to": 4, "name": "shift_test"})).unwrap();
        s.execute("action.paste", &json!({"row": row2, "name": "shift_test", "at": 20})).unwrap();

        let pegs = Pegs::of(&s.doc().unwrap().doc);
        let peg2 = pegs.pegs.iter().find(|p| p.rows.contains(&row2)).unwrap();
        assert_eq!(peg2.keys.get(&20).map(|k| k.x), Some(5.0));
        assert_eq!(peg2.keys.get(&23).map(|k| k.x), Some(25.0));
    }

    #[test]
    fn one_undo_step_restores_second_row_as_it_was() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let row1 = s.doc().unwrap().doc.layers[0].id.0;

        s.execute("peg.add", &json!({"rows": [row1]})).unwrap();
        s.execute("peg.key", &json!({"peg": 0, "frame": 1, "x": 100.0})).unwrap();

        s.execute("layer.new", &json!({})).unwrap();
        let row2 = s.doc().unwrap().doc.layers.iter().find(|l| l.id.0 != row1).unwrap().id.0;
        s.execute("peg.add", &json!({"rows": [row2]})).unwrap();

        s.execute("action.save", &json!({"row": row1, "from": 1, "to": 5, "name": "undo_test"})).unwrap();

        let before_keys = Pegs::of(&s.doc().unwrap().doc).pegs.iter().find(|p| p.rows.contains(&row2)).unwrap().keys.clone();
        s.execute("action.paste", &json!({"row": row2, "name": "undo_test", "at": 1})).unwrap();
        let after_keys = Pegs::of(&s.doc().unwrap().doc).pegs.iter().find(|p| p.rows.contains(&row2)).unwrap().keys.clone();
        assert_ne!(before_keys, after_keys);

        // One undo step restores row 2 keys
        s.execute("edit.undo", &json!({})).unwrap();
        let restored_keys = Pegs::of(&s.doc().unwrap().doc).pegs.iter().find(|p| p.rows.contains(&row2)).unwrap().keys.clone();
        assert_eq!(before_keys, restored_keys);
    }

    #[test]
    fn export_and_import_roundtrip_and_error_handling() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let row1 = s.doc().unwrap().doc.layers[0].id.0;

        s.execute("peg.add", &json!({"rows": [row1]})).unwrap();
        s.execute("peg.key", &json!({"peg": 0, "frame": 1, "x": 42.0})).unwrap();

        s.execute("action.save", &json!({"row": row1, "from": 1, "to": 10, "name": "export_act"})).unwrap();

        let path = std::env::temp_dir().join(format!("test_act_{}.{EXTENSION}", std::process::id()));
        let path_str = path.to_str().unwrap();

        // Export
        let exp = s.execute("action.export", &json!({"name": "export_act", "path": path_str})).unwrap();
        assert_eq!(exp["name"], "export_act");

        // Remove from document
        s.execute("action.remove", &json!({"name": "export_act"})).unwrap();
        assert!(Actions::of(&s.doc().unwrap().doc).actions.is_empty());

        // Import back
        let imp = s.execute("action.import", &json!({"path": path_str})).unwrap();
        assert_eq!(imp["name"], "export_act");
        assert_eq!(imp["length"], 10);
        assert_eq!(Actions::of(&s.doc().unwrap().doc).actions.len(), 1);

        // Corrupt file
        let corrupt_path = std::env::temp_dir().join(format!("corrupt_{}.{EXTENSION}", std::process::id()));
        std::fs::write(&corrupt_path, b"not valid json at all").unwrap();
        let err = s.execute("action.import", &json!({"path": corrupt_path.to_str().unwrap()}));
        assert!(err.is_err(), "corrupt json must return error");

        // Cleanup
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&corrupt_path);
    }

    #[test]
    fn bad_inputs_all_return_err() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let row1 = s.doc().unwrap().doc.layers[0].id.0;

        // from > to
        assert!(s.execute("action.save", &json!({"row": row1, "from": 10, "to": 5, "name": "bad"})).is_err());

        // missing or empty name
        assert!(s.execute("action.save", &json!({"row": row1, "from": 1, "to": 5, "name": ""})).is_err());

        // nonexistent row
        assert!(s.execute("action.save", &json!({"row": 9999999, "from": 1, "to": 5, "name": "bad"})).is_err());

        // nonexistent action name in paste
        assert!(s.execute("action.paste", &json!({"row": row1, "name": "no_such_action"})).is_err());

        // nonexistent action in remove
        assert!(s.execute("action.remove", &json!({"name": "no_such_action"})).is_err());
    }

    #[test]
    fn a_file_with_frames_past_its_length_is_refused_and_far_pastes_never_overflow() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let dir = std::env::temp_dir().join(format!("7rrek-action-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("far.7rrekaction");
        let bad_action = json!({"name": "far", "length": 2, "tracks": {"peg": {"4294967295": {}}}});
        std::fs::write(&path, serde_json::to_vec(&bad_action).unwrap()).unwrap();
        assert!(s.execute("action.import", &json!({"path": path.to_str().unwrap()})).is_err());
        assert!(s.execute("action.paste", &json!({"name": "far", "at": u64::MAX})).is_err());
        assert!(s.execute("action.save", &json!({"from": 4294967297u64, "to": 4294967298u64, "name": "x"})).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
