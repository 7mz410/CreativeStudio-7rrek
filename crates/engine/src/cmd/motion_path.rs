//! 7rrek: motion paths (`motionPath.*`). An animated layer (row) can follow a path drawn
//! in the document. The row moves between its first and last keyframe according to the tween's
//! easing. If `orient` is set, the row turns to align with the path's tangent.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vectorcraft_doc::{Document, NodeId};
use vectorcraft_geom::arc::ArcPath;
use vectorcraft_geom::{Affine, Point};

use super::anim::{Anim, eased, row_param, show};
use super::*;

pub const KEY: &str = "motionPaths";

/// A motion path assignment for a layer (timeline row).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct MotionPath {
    /// The document node id of the path to follow.
    pub path: u64,
    /// When true, turns the row to align with the tangent of the path.
    #[serde(default)]
    pub orient: bool,
}

/// All motion paths in the document: row id → motion path.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct MotionPaths {
    #[serde(default)]
    pub paths: BTreeMap<u64, MotionPath>,
}

impl MotionPaths {
    pub fn of(doc: &Document) -> Self {
        doc.unknown.get(KEY).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default()
    }

    pub fn store(&self, doc: &mut Document) {
        if self.paths.is_empty() {
            doc.unknown.remove(KEY);
        } else if let Ok(v) = serde_json::to_value(self) {
            doc.unknown.insert(KEY.into(), v);
        }
    }
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "motionPath.set",
            "Set Motion Path",
            [],
            None,
            "{path: id of a path in the document, row?: layer id (default: the current row), orient?: bool (turns along the tangent; default: false)} assigns a motion path to a row → {row, path, orient}",
            has_doc,
            set
        ),
        cmd!(
            "motionPath.remove",
            "Remove Motion Path",
            [],
            None,
            "{row?: layer id (default: the current row)} removes the motion path from the row → {removed: row}",
            has_doc,
            remove
        ),
        cmd!(
            query "motionPath.info",
            "Motion Path Info",
            [],
            None,
            "{row?: layer id (default: the current row)} → {row, path, orient, length, pos: [x, y], tangent: [tx, ty], angle}",
            has_doc,
            info
        ),
    ]
}

fn set(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "motionPath.set";
    let row = row_param(s, p, C)?;
    let path_id = id_param(p, "path").ok_or_else(|| bad(C, "path id is required"))?;

    let d = &s.doc()?.doc;
    let path_node = d.node(path_id).ok_or_else(|| bad(C, format!("path {} not found in document", path_id.0)))?;
    let path_data = path_node.path_data().ok_or_else(|| bad(C, format!("node {} is not a path", path_id.0)))?;
    let arc = ArcPath::new(path_data);
    if arc.is_empty() {
        return Err(bad(C, "path has zero length"));
    }

    let orient = p.get("orient").and_then(Value::as_bool).unwrap_or(false);

    let mut paths = MotionPaths::of(d);
    paths.paths.insert(row.0, MotionPath { path: path_id.0, orient });

    s.edit("Set Motion Path", |d, _| {
        paths.store(d);
        Ok(())
    })?;

    let current = Anim::of(&s.doc()?.doc).current;
    show(s, current)?;

    Ok(json!({"row": row.0, "path": path_id.0, "orient": orient}))
}

fn remove(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "motionPath.remove";
    let row = row_param(s, p, C)?;
    let mut paths = MotionPaths::of(&s.doc()?.doc);
    if paths.paths.remove(&row.0).is_none() {
        return Err(bad(C, "no motion path on this row"));
    }

    s.edit("Remove Motion Path", |d, _| {
        paths.store(d);
        Ok(())
    })?;

    let current = Anim::of(&s.doc()?.doc).current;
    show(s, current)?;

    Ok(json!({"removed": row.0}))
}

fn info(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "motionPath.info";
    let row = row_param(s, p, C)?;
    let d = &s.doc()?.doc;
    let paths = MotionPaths::of(d);
    let mp = paths.paths.get(&row.0).copied().ok_or_else(|| bad(C, "no motion path on this row"))?;

    let path_node = d.node(NodeId(mp.path)).ok_or_else(|| bad(C, format!("path {} was deleted from document", mp.path)))?;
    let path_data = path_node.path_data().ok_or_else(|| bad(C, format!("node {} is not a path", mp.path)))?;
    let arc = ArcPath::new(path_data);
    if arc.is_empty() {
        return Err(bad(C, "path has zero length"));
    }

    let anim = Anim::of(d);
    let t = eval_progress(d, row, anim.current);
    let s_dist = t.clamp(0.0, 1.0) * arc.len();
    let (pt, tangent) = arc.at(s_dist);
    let angle = tangent.y.atan2(tangent.x);

    Ok(json!({
        "row": row.0,
        "path": mp.path,
        "orient": mp.orient,
        "length": arc.len(),
        "pos": [pt.x, pt.y],
        "tangent": [tangent.x, tangent.y],
        "angle": angle.to_degrees(),
    }))
}

/// Progress ratio eased by `ease`.
pub fn ease_value(ease: Option<&[f64; 4]>, u: f64) -> f64 {
    eased(ease, u)
}

/// Normalized progress [0.0, 1.0] along the motion path for `row` at `frame`.
pub fn eval_progress(d: &Document, row: NodeId, frame: u32) -> f64 {
    let anim = Anim::of(d);
    let keys = d.node(row).map(|r| anim.row_keys(r)).unwrap_or_default();
    if keys.is_empty() {
        if anim.frames <= 1 {
            return 0.0;
        }
        return ((frame as f64 - 1.0) / (anim.frames as f64 - 1.0)).clamp(0.0, 1.0);
    }
    let Some(&(f_start, start_id)) = keys.first() else { return 0.0 };
    let Some(&(f_end, _)) = keys.last() else { return 0.0 };
    if f_end <= f_start {
        return if frame >= f_start { 1.0 } else { 0.0 };
    }
    let u = ((frame as f64 - f_start as f64) / (f_end as f64 - f_start as f64)).clamp(0.0, 1.0);
    let ease = anim.eases.get(&start_id.0);
    eased(ease, u)
}

/// Applies motion paths to animated rows at `frame`.
pub(crate) fn apply(d: &mut Document, frame: u32, ghosts: &mut Vec<u64>, posed: &mut Vec<u64>) {
    let all = MotionPaths::of(d);
    if all.paths.is_empty() {
        return;
    }
    let rows: Vec<NodeId> = d.layers.iter().map(|l| l.id).collect();
    for row in rows {
        let Some(mp) = all.paths.get(&row.0).copied() else { continue };
        let Some(path_node) = d.node(NodeId(mp.path)) else { continue };
        let Some(path_data) = path_node.path_data() else { continue };
        let arc = ArcPath::new(path_data);
        if arc.is_empty() {
            continue;
        }

        let t = eval_progress(d, row, frame);
        let s = t.clamp(0.0, 1.0) * arc.len();
        let (pt, tangent) = arc.at(s);
        let angle = tangent.y.atan2(tangent.x);

        let shown: Vec<NodeId> = d.node(row).and_then(|r| r.children()).into_iter().flatten().filter(|c| c.visible).map(|c| c.id).collect();

        let mut targets = Vec::with_capacity(shown.len());
        for id in shown {
            let target = if ghosts.contains(&id.0) {
                id
            } else {
                let Some(n) = d.node(id).cloned() else { continue };
                let mut copy = d.reid(&n);
                copy.locked = true;
                let Some(at) = d.index_path(id).and_then(|p| p.last().copied()) else { continue };
                let Ok(copy_id) = d.insert(Some(row), at + 1, copy) else { continue };
                if let Some(n) = d.node_mut(id) {
                    n.visible = false;
                }
                posed.push(id.0);
                ghosts.push(copy_id.0);
                copy_id
            };
            targets.push(target);
        }
        // The row moves as one piece: its art's centre goes on the path.
        let center = d.bounds_of(&targets, false).map(|b| b.center()).unwrap_or(Point::ZERO);
        let m = Affine::translate(pt.to_vec2())
            * (if mp.orient { Affine::rotate(angle) } else { Affine::IDENTITY })
            * Affine::translate(-center.to_vec2());
        for target in targets {
            if let Some(n) = d.node_mut(target) {
                n.transform(m, true);
            }
        }
    }
}
