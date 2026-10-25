//! 7rrek: mesh deformer commands (`mesh.*`) over the model in [`super::mesh`].

use serde_json::{Value, json};
use vectorcraft_doc::NodeId;

use super::anim::{Anim, frame_param, show};
use super::mesh::{MAX_MESHES, Mesh, Meshes};
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "mesh.add",
            "Add Mesh",
            [],
            None,
            "{node?: id (default: the one selected object; error if none or several), cols?: 1–60 (default 4), rows?: 1–60 (default 4)} adds a grid mesh over the object's bounds, no pins → {node, vertices, triangles}",
            has_doc,
            add
        ),
        cmd!(
            "mesh.pin",
            "Pin Mesh Vertex",
            [],
            None,
            "{node?: id (default: the selected object), vertex?: vertex index, x?, y?: the rest vertex nearest this point (used when `vertex` is not given)} adds a pin on that vertex; every key keeps it at rest → {pin: index into pins, vertex}",
            has_doc,
            pin
        ),
        cmd!(
            "mesh.unpin",
            "Unpin Mesh Vertex",
            [],
            None,
            "{node?: id (default: the selected object), pin: index into pins} removes the pin and its column from every key → {pin, vertex}",
            has_doc,
            unpin
        ),
        cmd!(
            "mesh.key",
            "Mesh Key",
            [],
            None,
            "{node?: id (default: the selected object), frame?: (default: the current frame), pin: index into pins, x?, y?: move (document points), dx?, dy?: added to the move} sets the pin's position at the frame; a frame without a key starts from where the pins are then → {frame, x, y}",
            has_doc,
            key
        ),
        cmd!(
            "mesh.removeKey",
            "Remove Mesh Key",
            [],
            None,
            "{node?: id (default: the selected object), frame?: (default: the current frame)} removes the key at the frame; error if there is none → {frame}",
            has_doc,
            remove_key
        ),
        cmd!(
            "mesh.remove",
            "Remove Mesh",
            [],
            None,
            "{node?: id (default: the selected object)} removes the object's mesh → {node}",
            has_doc,
            remove
        ),
        cmd!(
            query "mesh.info",
            "Mesh",
            [],
            None,
            "{node?: id (default: the selected object)} → {node, vertices, triangles, pins: [vertex indices], keys: [frames], points: [[x, y]…] at the current frame (rest where no pin has moved)}",
            has_doc,
            info
        ),
    ]
}

/// Store `meshes` as one undo step, then show the current frame. A set that fails `valid` is refused.
fn commit(s: &mut Session, label: &str, meshes: Meshes, cmd: &str) -> Result<()> {
    if !meshes.valid() {
        return Err(bad(cmd, "the edit would leave an invalid mesh"));
    }
    s.edit(label, |d, _| {
        meshes.store(d);
        Ok(())
    })?;
    let current = Anim::of(&s.doc()?.doc).current;
    show(s, current)?;
    Ok(())
}

/// The meshes of the open document.
fn meshes_of(s: &Session) -> Result<Meshes> {
    Ok(Meshes::of(&s.doc()?.doc))
}

/// The object `p` names under `node`, or else the one selected object, or else (for commands on
/// an existing mesh) the document's only meshed object: showing a bent frame takes the original
/// art out of the selection, as its posed copy stands in for it.
fn node_of(s: &Session, p: &Value, cmd: &str) -> Result<NodeId> {
    if let Some(v) = p.get("node").filter(|v| !v.is_null()) {
        return checked_id(s, v, cmd);
    }
    let only = || match meshes_of(s).ok()?.meshes.as_slice() {
        [m] if cmd != "mesh.add" => Some(NodeId(m.node)),
        _ => None,
    };
    match s.doc()?.selection.objects.as_slice() {
        [one] => Ok(*one),
        [] => only().ok_or_else(|| bad(cmd, "nothing selected; select one object or give `node`")),
        _ => Err(bad(cmd, "several objects selected; give `node`")),
    }
}

/// The position in `meshes` of the mesh over `node`.
fn mesh_at(meshes: &Meshes, node: NodeId, cmd: &str) -> Result<usize> {
    meshes.meshes.iter().position(|m| m.node == node.0).ok_or_else(|| bad(cmd, "no mesh on this object; use mesh.add"))
}

/// A pin index `p` gives for `pin`, below `n` pins.
fn pin_param(p: &Value, n: usize, cmd: &str) -> Result<usize> {
    p.get("pin")
        .and_then(Value::as_u64)
        .map(index)
        .filter(|i| *i < n)
        .ok_or_else(|| bad(cmd, format!("`pin` must be a pin index from 0 to {}", n.saturating_sub(1))))
}

/// A finite number `p` gives for `key`.
fn num_req(p: &Value, key: &str, cmd: &str) -> Result<f64> {
    match p.get(key).and_then(Value::as_f64) {
        Some(v) if v.is_finite() => Ok(v),
        _ => Err(bad(cmd, format!("`{key}` must be a finite number"))),
    }
}

/// The grid count `p` gives for `key`, 1 to 60; 4 when it gives none.
fn count_param(p: &Value, key: &str, cmd: &str) -> Result<u32> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(4),
        Some(v) => v
            .as_u64()
            .filter(|n| (1..=60).contains(n))
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| bad(cmd, format!("`{key}` must be a whole number from 1 to 60"))),
    }
}

/// Squared distance from `p` to `q`.
fn dist2(p: &[f64; 2], q: [f64; 2]) -> f64 {
    (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)
}

/// The index of the unpinned rest vertex nearest `at`.
fn nearest(rest: &[[f64; 2]], pins: &[u32], at: [f64; 2]) -> Option<usize> {
    rest.iter()
        .enumerate()
        .filter(|(i, _)| !pins.iter().any(|&p| p as usize == *i))
        .min_by(|a, b| dist2(a.1, at).total_cmp(&dist2(b.1, at)))
        .map(|(i, _)| i)
}

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "mesh.add";
    let node = node_of(s, p, C)?;
    let cols = count_param(p, "cols", C)?;
    let rows = count_param(p, "rows", C)?;
    let mut meshes = meshes_of(s)?;
    if meshes.meshes.len() >= MAX_MESHES {
        return Err(bad(C, format!("a document takes at most {MAX_MESHES} meshes")));
    }
    if meshes.of_node(node.0).is_some() {
        return Err(bad(C, "this object already has a mesh"));
    }
    let bounds = s.doc()?.doc.bounds_of(&[node], false).ok_or_else(|| bad(C, "nothing to measure"))?;
    let m = Mesh::grid(node.0, bounds, cols, rows).ok_or_else(|| bad(C, "the object has no area to mesh"))?;
    let out = json!({"node": node.0, "vertices": m.rest.len(), "triangles": m.tris.len()});
    meshes.meshes.push(m);
    commit(s, "Add Mesh", meshes, C)?;
    Ok(out)
}

fn pin(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "mesh.pin";
    let node = node_of(s, p, C)?;
    let mut meshes = meshes_of(s)?;
    let i = mesh_at(&meshes, node, C)?;
    let Some(m) = meshes.meshes.get_mut(i) else { return Err(bad(C, "no such mesh")) };
    let vertex = match p.get("vertex").filter(|v| !v.is_null()) {
        Some(v) => v
            .as_u64()
            .map(index)
            .filter(|v| *v < m.rest.len())
            .ok_or_else(|| bad(C, format!("`vertex` must be a vertex index from 0 to {}", m.rest.len().saturating_sub(1))))?,
        None => {
            let at = [num_req(p, "x", C)?, num_req(p, "y", C)?];
            nearest(&m.rest, &m.pins, at).ok_or_else(|| bad(C, "every vertex is pinned"))?
        }
    };
    let v = u32::try_from(vertex).map_err(|_| bad(C, "the mesh has too many vertices"))?;
    if m.pins.contains(&v) {
        return Err(bad(C, format!("vertex {v} is already pinned")));
    }
    let [rx, ry] = *m.rest.get(vertex).ok_or_else(|| bad(C, "no such vertex"))?;
    m.pins.push(v);
    for k in m.keys.values_mut() {
        k.push([rx, ry]);
    }
    let pin = m.pins.len().saturating_sub(1);
    commit(s, "Pin Mesh", meshes, C)?;
    Ok(json!({"pin": pin, "vertex": v}))
}

fn unpin(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "mesh.unpin";
    let node = node_of(s, p, C)?;
    let mut meshes = meshes_of(s)?;
    let i = mesh_at(&meshes, node, C)?;
    let Some(m) = meshes.meshes.get_mut(i) else { return Err(bad(C, "no such mesh")) };
    let pin = pin_param(p, m.pins.len(), C)?;
    let vertex = m.pins.remove(pin);
    for k in m.keys.values_mut() {
        if pin < k.len() {
            k.remove(pin);
        }
    }
    if m.pins.is_empty() {
        m.keys.clear();
    }
    commit(s, "Unpin Mesh", meshes, C)?;
    Ok(json!({"pin": pin, "vertex": vertex}))
}

fn key(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "mesh.key";
    let node = node_of(s, p, C)?;
    let mut meshes = meshes_of(s)?;
    let i = mesh_at(&meshes, node, C)?;
    let frame = frame_param(p, &Anim::of(&s.doc()?.doc), C)?;
    let Some(m) = meshes.meshes.get_mut(i) else { return Err(bad(C, "no such mesh")) };
    let pin = pin_param(p, m.pins.len(), C)?;
    let mut cur: Vec<[f64; 2]> = match m.keys.get(&frame) {
        Some(k) => k.clone(),
        None => m.pins_at(frame).iter().map(|q| [q.x, q.y]).collect(),
    };
    let Some(slot) = cur.get_mut(pin) else { return Err(bad(C, "no such pin")) };
    let [now_x, now_y] = *slot;
    let x = number_or(p, "x", now_x, C)? + number_or(p, "dx", 0.0, C)?;
    let y = number_or(p, "y", now_y, C)? + number_or(p, "dy", 0.0, C)?;
    if !(x.is_finite() && y.is_finite()) {
        return Err(bad(C, "the pin's position must be finite"));
    }
    *slot = [x, y];
    m.keys.insert(frame, cur);
    commit(s, "Mesh Key", meshes, C)?;
    Ok(json!({"frame": frame, "x": x, "y": y}))
}

fn remove_key(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "mesh.removeKey";
    let node = node_of(s, p, C)?;
    let mut meshes = meshes_of(s)?;
    let i = mesh_at(&meshes, node, C)?;
    let frame = frame_param(p, &Anim::of(&s.doc()?.doc), C)?;
    let Some(m) = meshes.meshes.get_mut(i) else { return Err(bad(C, "no such mesh")) };
    if m.keys.remove(&frame).is_none() {
        return Err(bad(C, format!("no mesh key at frame {frame}")));
    }
    commit(s, "Remove Mesh Key", meshes, C)?;
    Ok(json!({"frame": frame}))
}

fn remove(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "mesh.remove";
    let node = node_of(s, p, C)?;
    let mut meshes = meshes_of(s)?;
    let i = mesh_at(&meshes, node, C)?;
    meshes.meshes.remove(i);
    commit(s, "Remove Mesh", meshes, C)?;
    Ok(json!({"node": node.0}))
}

fn info(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "mesh.info";
    let node = node_of(s, p, C)?;
    let meshes = meshes_of(s)?;
    let i = mesh_at(&meshes, node, C)?;
    let Some(m) = meshes.meshes.get(i) else { return Err(bad(C, "no such mesh")) };
    let frame = Anim::of(&s.doc()?.doc).current;
    let points: Vec<[f64; 2]> = match m.at(frame) {
        Some(v) => v.iter().map(|q| [q.x, q.y]).collect(),
        None => m.rest.clone(),
    };
    Ok(json!({
        "node": m.node,
        "vertices": m.rest.len(),
        "triangles": m.tris.len(),
        "pins": m.pins,
        "keys": m.keys.keys().collect::<Vec<_>>(),
        "points": points,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s
    }

    /// A 100 × 50 rectangle at the origin; its id (it becomes the selection).
    fn rect(s: &mut Session) -> u64 {
        s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 100, "height": 50})).unwrap()["id"].as_u64().unwrap()
    }

    fn info(s: &mut Session, node: u64) -> Value {
        s.execute("mesh.info", &json!({"node": node})).unwrap()
    }

    fn points(v: &Value) -> Vec<[f64; 2]> {
        v["points"].as_array().unwrap().iter().map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()]).collect()
    }

    #[test]
    fn add_meshes_the_selected_object_over_its_bounds() {
        let mut s = session();
        assert!(s.execute("mesh.add", &json!({})).is_err(), "nothing selected");
        let r = rect(&mut s);
        let m = s.execute("mesh.add", &json!({"cols": 3, "rows": 2})).unwrap();
        assert_eq!((m["node"].as_u64(), m["vertices"].as_u64(), m["triangles"].as_u64()), (Some(r), Some(12), Some(12)));
        let rest = points(&info(&mut s, r));
        assert_eq!((rest[0], rest[11]), ([0.0, 0.0], [100.0, 50.0]));
        assert!(s.execute("mesh.add", &json!({})).is_err(), "already has a mesh");
        s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 10, "height": 10})).unwrap();
        let d = s.execute("mesh.add", &json!({})).unwrap();
        assert_eq!(d["vertices"].as_u64(), Some(25), "defaults to a 4 × 4 grid");
    }

    #[test]
    fn a_pinned_corner_moves_its_key_and_the_far_side_follows() {
        let mut s = session();
        s.execute("anim.setFrames", &json!({"frames": 5})).unwrap();
        // Keys move the art, which drops it from the selection, so every call names the object.
        let n = rect(&mut s);
        s.execute("mesh.add", &json!({"node": n, "cols": 2, "rows": 1})).unwrap();
        // Vertices: 0 (0, 0), 1 (50, 0), 2 (100, 0), 3 (0, 50), 4 (50, 50), 5 (100, 50).
        assert_eq!(s.execute("mesh.pin", &json!({"node": n, "vertex": 0})).unwrap()["pin"], 0);
        s.execute("mesh.pin", &json!({"node": n, "x": 1, "y": 49})).unwrap();
        s.execute("mesh.key", &json!({"node": n, "pin": 0, "frame": 1})).unwrap();
        s.execute("mesh.key", &json!({"node": n, "pin": 0, "frame": 5, "dx": 20})).unwrap();
        s.execute("anim.goto", &json!({"frame": 5})).unwrap();
        let at5 = points(&info(&mut s, n));
        s.execute("anim.goto", &json!({"frame": 1})).unwrap();
        let info1 = info(&mut s, n);
        let at1 = points(&info1);
        assert_eq!(at1, vec![[0.0, 0.0], [50.0, 0.0], [100.0, 0.0], [0.0, 50.0], [50.0, 50.0], [100.0, 50.0]], "rest at frame 1");
        assert_eq!(info1["pins"], json!([0, 3]));
        assert_eq!(info1["keys"], json!([1, 5]));
        assert!((at5[0][0] - 20.0).abs() < 1e-6 && at5[0][1].abs() < 1e-6, "the pin: {:?}", at5[0]);
        assert!((at5[2][0] - at1[2][0]).hypot(at5[2][1] - at1[2][1]) > 1.0, "the far side moves: {:?}", at5[2]);
    }

    #[test]
    fn unpin_drops_the_pin_and_its_key_column() {
        let mut s = session();
        let n = rect(&mut s);
        s.execute("mesh.add", &json!({"node": n, "cols": 2, "rows": 1})).unwrap();
        s.execute("mesh.pin", &json!({"node": n, "vertex": 0})).unwrap();
        s.execute("mesh.pin", &json!({"node": n, "vertex": 3})).unwrap();
        s.execute("mesh.key", &json!({"node": n, "pin": 0, "frame": 1})).unwrap();
        assert_eq!(s.execute("mesh.unpin", &json!({"node": n, "pin": 0})).unwrap(), json!({"pin": 0, "vertex": 0}));
        let i = info(&mut s, n);
        assert_eq!(i["pins"], json!([3]));
        assert_eq!(Meshes::of(&s.doc().unwrap().doc).meshes[0].keys.get(&1).map(Vec::len), Some(1));
        s.execute("mesh.unpin", &json!({"node": n, "pin": 0})).unwrap();
        assert!(info(&mut s, n)["keys"].as_array().unwrap().is_empty(), "no pins, no keys");
    }

    #[test]
    fn pin_picks_the_nearest_rest_vertex_and_keys_can_be_removed() {
        let mut s = session();
        let n = rect(&mut s);
        s.execute("mesh.add", &json!({"node": n, "cols": 2, "rows": 1})).unwrap();
        assert_eq!(s.execute("mesh.pin", &json!({"node": n, "x": 98, "y": 52})).unwrap()["vertex"], 5);
        assert_ne!(s.execute("mesh.pin", &json!({"node": n, "x": 99, "y": 49})).unwrap()["vertex"], 5, "the nearest free vertex");
        s.execute("mesh.key", &json!({"node": n, "pin": 0, "frame": 1, "dx": 3})).unwrap();
        assert_eq!(s.execute("mesh.removeKey", &json!({"node": n, "frame": 1})).unwrap(), json!({"frame": 1}));
        assert!(s.execute("mesh.removeKey", &json!({"node": n, "frame": 1})).is_err());
        assert!(info(&mut s, n)["keys"].as_array().unwrap().is_empty());
        assert!(s.execute("mesh.remove", &json!({"node": n})).unwrap()["node"].as_u64().is_some());
        assert!(s.execute("mesh.info", &json!({"node": n})).is_err(), "no mesh left");
    }

    #[test]
    fn bad_params_are_errors_and_change_nothing() {
        let mut s = session();
        let r = rect(&mut s);
        assert!(s.execute("mesh.add", &json!({"cols": 0})).is_err());
        assert!(s.execute("mesh.add", &json!({"rows": 61})).is_err());
        assert!(s.execute("mesh.add", &json!({"node": 999_999})).is_err(), "no such object");
        s.execute("mesh.add", &json!({"node": r, "cols": 2, "rows": 2})).unwrap();
        assert!(s.execute("mesh.add", &json!({"node": r})).is_err(), "already meshed");
        assert!(s.execute("mesh.pin", &json!({"vertex": 99})).is_err());
        assert!(s.execute("mesh.pin", &json!({"x": 1.0})).is_err(), "y missing");
        s.execute("mesh.pin", &json!({"vertex": 0})).unwrap();
        assert!(s.execute("mesh.pin", &json!({"vertex": 0})).is_err(), "already pinned");
        assert!(s.execute("mesh.key", &json!({"pin": 9, "dx": 1})).is_err());
        assert!(s.execute("mesh.key", &json!({"pin": 0, "x": "nan"})).is_err());
        assert!(s.execute("mesh.key", &json!({"pin": 0, "x": 1e308, "dx": 1e308})).is_err(), "not finite");
        assert!(s.execute("mesh.key", &json!({"pin": 0, "frame": 0})).is_err());
        assert!(s.execute("mesh.unpin", &json!({"pin": 1})).is_err());
        assert!(s.execute("mesh.removeKey", &json!({"frame": 3})).is_err());
        let i = info(&mut s, r);
        assert_eq!((i["pins"].clone(), i["keys"].clone()), (json!([0]), json!([])));
    }

    #[test]
    fn meshes_survive_save_and_load() {
        let mut s = session();
        let r = rect(&mut s);
        s.execute("mesh.add", &json!({"cols": 2, "rows": 2})).unwrap();
        s.execute("mesh.pin", &json!({"vertex": 0})).unwrap();
        s.execute("mesh.key", &json!({"pin": 0, "frame": 1, "dx": 5})).unwrap();
        let before = Meshes::of(&s.doc().unwrap().doc);
        let bytes = vectorcraft_format::save(&s.doc().unwrap().doc, false);
        let back = vectorcraft_format::load(&bytes).unwrap();
        assert_eq!(Meshes::of(&back), before);
        assert_eq!(before.of_node(r).map(|m| m.pins.clone()), Some(vec![0]));
    }

    #[test]
    fn with_nothing_selected_the_only_mesh_is_used_and_pins_pick_free_vertices() {
        let mut s = session();
        let id = rect(&mut s);
        s.execute("mesh.add", &json!({"cols": 2, "rows": 1})).unwrap();
        let a = s.execute("mesh.pin", &json!({"x": 0, "y": 0})).unwrap()["vertex"].as_u64().unwrap();
        let b = s.execute("mesh.pin", &json!({"x": 0, "y": 0})).unwrap()["vertex"].as_u64().unwrap();
        assert_ne!(a, b, "the second pin takes the nearest free vertex");
        s.execute("select.none", &json!({})).unwrap();
        s.execute("mesh.key", &json!({"pin": 0, "dx": 10})).unwrap();
        assert_eq!(s.execute("mesh.info", &json!({})).unwrap()["node"], json!(id));
        assert!(s.execute("mesh.add", &json!({})).is_err(), "adding still needs a selected object");
    }
}
