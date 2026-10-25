//! 7rrek: peg commands (`peg.*`) over the model in [`super::peg`].

use serde_json::{Value, json};
use vectorcraft_doc::{Document, NodeId};

use super::anim::{Anim, ease_param, frame_param, row_of, row_param, show};
use super::peg::{MAX_PEGS, Peg, PegKey, Pegs};
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "peg.add",
            "Add Peg",
            [],
            None,
            "{name?: text (default: \"Peg N\"), parent?: peg index (the peg it hangs from; default: none), x?, y?: pivot (default: centre of the current row's bounds, else 0), rows?: [layer ids] (default: the current row; a row moves off any other peg)} adds a peg with no keys, so it stays put → {index}",
            has_doc,
            add
        ),
        cmd!(
            "peg.attach",
            "Attach to Peg",
            [],
            None,
            "{peg: peg index, row?: layer id (default: the current layer's row)} moves the row onto that peg, off any other peg → {peg, rows: the peg's layer ids}",
            has_doc,
            attach
        ),
        cmd!(
            "peg.detach",
            "Detach from Peg",
            [],
            None,
            "{row?: layer id (default: the current layer's row)} takes the row off its peg → {detached: bool}",
            has_doc,
            detach
        ),
        cmd!(
            "peg.key",
            "Peg Key",
            [],
            None,
            "{peg: peg index, frame?: (default: the current frame), x?, y?: move (document points), angle?: turn (degrees, about the pivot), sx?, sy?: scale (factors, 1 is unchanged), ease?: \"linear\" | \"easeIn\" | \"easeOut\" | \"easeInOut\" | [x1, y1, x2, y2] (eases toward the next key)} sets the key at the frame; fields not given keep the value the peg has there → the key",
            has_doc,
            key
        ),
        cmd!(
            "peg.removeKey",
            "Remove Peg Key",
            [],
            None,
            "{peg: peg index, frame?: (default: the current frame)} removes the key at the frame; error if there is none → {frame}",
            has_doc,
            remove_key
        ),
        cmd!(
            "peg.remove",
            "Remove Peg",
            [],
            None,
            "{peg: peg index} removes the peg; its children hang from its parent, later indices shift down, and its rows are left free → {removed: the index it had}",
            has_doc,
            remove
        ),
        cmd!(
            query "peg.info",
            "Pegs",
            [],
            None,
            "{frame?: (default: the current frame)} → {pegs: [{index, name, parent, pivot, rows, keys: [{frame, x, y, angle, sx, sy, ease}]}], at: each peg's key at the frame}",
            has_doc,
            info
        ),
    ]
}

/// Store `pegs` as one undo step, then show the current frame. A set that fails `valid` is refused.
fn commit(s: &mut Session, label: &str, pegs: Pegs, cmd: &str) -> Result<()> {
    if !pegs.valid() {
        return Err(bad(cmd, "the edit would leave invalid pegs"));
    }
    s.edit(label, |d, _| {
        pegs.store(d);
        Ok(())
    })?;
    let current = Anim::of(&s.doc()?.doc).current;
    show(s, current)?;
    Ok(())
}

/// The pegs of the open document.
fn pegs_of(s: &Session) -> Result<Pegs> {
    Ok(Pegs::of(&s.doc()?.doc))
}

/// The current layer's row, if there is one.
fn current_row(s: &Session) -> Result<Option<NodeId>> {
    let st = s.doc()?;
    Ok(st.current_layer().and_then(|l| row_of(&st.doc, l)))
}

/// The centre of `row`'s bounds, or the origin when there is no row or nothing to measure.
fn centre(s: &Session, row: Option<NodeId>) -> Result<[f64; 2]> {
    let Some(row) = row else { return Ok([0.0, 0.0]) };
    let d: &Document = &s.doc()?.doc;
    Ok(match d.bounds_of(&[row], false) {
        Some(b) => [b.center().x, b.center().y],
        None => [0.0, 0.0],
    })
}

/// A finite number `p` gives for `key`.
fn num_req(p: &Value, key: &str, cmd: &str) -> Result<f64> {
    match p.get(key).and_then(Value::as_f64) {
        Some(v) if v.is_finite() => Ok(v),
        _ => Err(bad(cmd, format!("`{key}` must be a finite number"))),
    }
}

/// The finite number `p` gives for `key`, or `default` when it gives none.
fn num_or(p: &Value, key: &str, default: f64, cmd: &str) -> Result<f64> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(_) => num_req(p, key, cmd),
    }
}

/// A peg index `p` gives for `key`, below `n` pegs.
fn peg_param(p: &Value, key: &str, n: usize, cmd: &str) -> Result<usize> {
    p.get(key)
        .and_then(Value::as_u64)
        .map(index)
        .filter(|i| *i < n)
        .ok_or_else(|| bad(cmd, format!("`{key}` must be a peg index from 0 to {}", n.saturating_sub(1))))
}

/// The rows `p` names under `key`: layer ids, or any object inside a row (which names its row).
fn rows_param(s: &Session, p: &Value, key: &str, cmd: &str) -> Result<Vec<NodeId>> {
    let d: &Document = &s.doc()?.doc;
    let list = p.get(key).and_then(Value::as_array).ok_or_else(|| bad(cmd, format!("{key} must be an array of layer ids")))?;
    let mut rows: Vec<NodeId> = Vec::with_capacity(list.len());
    for v in list {
        let id = v.as_u64().map(NodeId).ok_or_else(|| bad(cmd, format!("{key} must be an array of layer ids")))?;
        let row = row_of(d, id).ok_or_else(|| bad(cmd, "not a layer in this document"))?;
        if !rows.contains(&row) {
            rows.push(row);
        }
    }
    Ok(rows)
}

/// Take `row` off every peg: a row is held by one peg at most.
fn release(pegs: &mut Pegs, row: NodeId) {
    for q in &mut pegs.pegs {
        q.rows.retain(|r| *r != row.0);
    }
}

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "peg.add";
    let mut pegs = pegs_of(s)?;
    let n = pegs.pegs.len();
    if n >= MAX_PEGS {
        return Err(bad(C, format!("a document takes at most {MAX_PEGS} pegs")));
    }
    let parent = match p.get("parent") {
        None | Some(Value::Null) => None,
        Some(_) => Some(peg_param(p, "parent", n, C)?),
    };
    let name = match p.get("name") {
        None | Some(Value::Null) => format!("Peg {}", n + 1),
        Some(Value::String(t)) if !t.trim().is_empty() => t.clone(),
        Some(_) => return Err(bad(C, "name must be non-empty text")),
    };
    let rows: Vec<NodeId> = match p.get("rows") {
        None | Some(Value::Null) => current_row(s)?.into_iter().collect(),
        Some(_) => rows_param(s, p, "rows", C)?,
    };
    let [here_x, here_y] = centre(s, current_row(s)?)?;
    let x = num_or(p, "x", here_x, C)?;
    let y = num_or(p, "y", here_y, C)?;
    for r in &rows {
        release(&mut pegs, *r);
    }
    pegs.pegs.push(Peg { name, parent, pivot: [x, y], rows: rows.iter().map(|r| r.0).collect(), ..Peg::default() });
    commit(s, "Add Peg", pegs, C)?;
    Ok(json!({"index": n}))
}

fn attach(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "peg.attach";
    let row = row_param(s, p, C)?;
    let mut pegs = pegs_of(s)?;
    let i = peg_param(p, "peg", pegs.pegs.len(), C)?;
    release(&mut pegs, row);
    let Some(peg) = pegs.pegs.get_mut(i) else { return Err(bad(C, "no such peg")) };
    peg.rows.push(row.0);
    let rows = peg.rows.clone();
    commit(s, "Attach to Peg", pegs, C)?;
    Ok(json!({"peg": i, "rows": rows}))
}

fn detach(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "peg.detach";
    let row = row_param(s, p, C)?;
    let mut pegs = pegs_of(s)?;
    let held = pegs.pegs.iter().any(|q| q.rows.contains(&row.0));
    if held {
        release(&mut pegs, row);
        commit(s, "Detach from Peg", pegs, C)?;
    }
    Ok(json!({"detached": held}))
}

fn key(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "peg.key";
    let mut pegs = pegs_of(s)?;
    let i = peg_param(p, "peg", pegs.pegs.len(), C)?;
    let frame = frame_param(p, &Anim::of(&s.doc()?.doc), C)?;
    let Some(peg) = pegs.pegs.get(i) else { return Err(bad(C, "no such peg")) };
    let now = peg.key_at(frame);
    let own_ease = peg.keys.get(&frame).and_then(|k| k.ease);
    let k = PegKey {
        x: num_or(p, "x", now.x, C)?,
        y: num_or(p, "y", now.y, C)?,
        angle: num_or(p, "angle", now.angle, C)?,
        sx: num_or(p, "sx", now.sx, C)?,
        sy: num_or(p, "sy", now.sy, C)?,
        ease: ease_param(p, C)?.unwrap_or(own_ease),
    };
    if let Some(peg) = pegs.pegs.get_mut(i) {
        peg.keys.insert(frame, k);
    }
    commit(s, "Peg Key", pegs, C)?;
    Ok(json!(k))
}

fn remove_key(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "peg.removeKey";
    let mut pegs = pegs_of(s)?;
    let i = peg_param(p, "peg", pegs.pegs.len(), C)?;
    let frame = frame_param(p, &Anim::of(&s.doc()?.doc), C)?;
    let Some(peg) = pegs.pegs.get_mut(i) else { return Err(bad(C, "no such peg")) };
    if peg.keys.remove(&frame).is_none() {
        return Err(bad(C, format!("no peg key at frame {frame}")));
    }
    commit(s, "Remove Peg Key", pegs, C)?;
    Ok(json!({"frame": frame}))
}

fn remove(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "peg.remove";
    let mut pegs = pegs_of(s)?;
    let i = peg_param(p, "peg", pegs.pegs.len(), C)?;
    // Parents come before children, so the removed peg's parent is below `i` and keeps its index.
    let Some(gone) = (i < pegs.pegs.len()).then(|| pegs.pegs.remove(i)) else { return Err(bad(C, "no such peg")) };
    for q in &mut pegs.pegs {
        q.parent = match q.parent {
            Some(j) if j == i => gone.parent,
            Some(j) if j > i => Some(j - 1),
            other => other,
        };
    }
    commit(s, "Remove Peg", pegs, C)?;
    Ok(json!({"removed": i}))
}

fn info(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "peg.info";
    let frame = frame_param(p, &Anim::of(&s.doc()?.doc), C)?;
    let pegs = pegs_of(s)?;
    let list: Vec<Value> = pegs
        .pegs
        .iter()
        .enumerate()
        .map(|(i, q)| {
            let keys: Vec<Value> = q
                .keys
                .iter()
                .map(|(f, k)| json!({"frame": f, "x": k.x, "y": k.y, "angle": k.angle, "sx": k.sx, "sy": k.sy, "ease": k.ease}))
                .collect();
            json!({"index": i, "name": q.name, "parent": q.parent, "pivot": q.pivot, "rows": q.rows, "keys": keys})
        })
        .collect();
    let at: Vec<Value> = pegs.pegs.iter().map(|q| json!(q.key_at(frame))).collect();
    Ok(json!({"pegs": list, "at": at}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;
    use crate::cmd::anim::at_frame;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s
    }

    /// A new row holding one 20-pt square whose top-left is (`x`, 0); the row is made current.
    fn row(s: &mut Session, x: f64) -> u64 {
        let id = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
        s.execute("shape.rectangle", &json!({"x": x, "y": 0, "width": 20, "height": 20})).unwrap();
        id
    }

    /// The bounds of `row` as drawn at `frame` of the open document.
    fn drawn(s: &Session, row: u64, frame: u32) -> vectorcraft_geom::Rect {
        at_frame(&s.doc().unwrap().doc, frame).bounds_of(&[NodeId(row)], false).unwrap()
    }

    fn info(s: &mut Session) -> Value {
        s.execute("peg.info", &json!({"frame": 1})).unwrap()
    }

    #[test]
    fn a_peg_moved_on_a_frame_carries_its_layers() {
        let mut s = session();
        s.execute("anim.setFrames", &json!({"frames": 2})).unwrap();
        let held: Vec<u64> = (0..3).map(|k| row(&mut s, 10.0 * k as f64)).collect();
        let free = row(&mut s, 500.0);
        let r = s.execute("peg.add", &json!({"rows": held})).unwrap();
        assert_eq!(r["index"], 0);
        s.execute("peg.key", &json!({"peg": 0, "frame": 1, "x": 0, "y": 0})).unwrap();
        let k = s.execute("peg.key", &json!({"peg": 0, "frame": 2, "x": 100})).unwrap();
        assert_eq!((k["x"].as_f64(), k["y"].as_f64(), k["sx"].as_f64()), (Some(100.0), Some(0.0), Some(1.0)));
        for r in &held {
            let (a, b) = (drawn(&s, *r, 1), drawn(&s, *r, 2));
            assert!((b.x0 - a.x0 - 100.0).abs() < 1e-9 && (b.y0 - a.y0).abs() < 1e-9, "row {r}: {a:?} -> {b:?}");
        }
        let (a, b) = (drawn(&s, free, 1), drawn(&s, free, 2));
        assert_eq!(a.x0, b.x0, "a row off the peg stays put");
    }

    #[test]
    fn nested_pegs_compose() {
        let mut s = session();
        let r = row(&mut s, -10.0); // centre (0, 10)
        s.execute("peg.add", &json!({"rows": []})).unwrap();
        s.execute("peg.key", &json!({"peg": 0, "frame": 1, "x": 100})).unwrap();
        s.execute("peg.add", &json!({"parent": 0, "rows": [], "x": 0, "y": 0})).unwrap();
        s.execute("peg.key", &json!({"peg": 1, "frame": 1, "angle": 90})).unwrap();
        // The child turns the row a quarter turn about the origin, under a parent that moves it 100 pt.
        // Attaching shows frame 1 once with both pegs in place, so the open document holds that view.
        s.execute("peg.attach", &json!({"peg": 1, "row": r})).unwrap();
        let c = s.doc().unwrap().doc.bounds_of(&[NodeId(r)], false).unwrap().center();
        assert!(((c.x - 100.0).abs() - 10.0).abs() < 1e-6 && c.y.abs() < 1e-6, "{c:?}");
        let at = info(&mut s)["at"].clone();
        assert_eq!((at[0]["x"].as_f64(), at[1]["angle"].as_f64()), (Some(100.0), Some(90.0)));
    }

    #[test]
    fn removing_a_peg_rehangs_its_children() {
        let mut s = session();
        s.execute("peg.add", &json!({})).unwrap();
        s.execute("peg.add", &json!({"parent": 0})).unwrap();
        s.execute("peg.add", &json!({"parent": 1})).unwrap();
        let r = s.execute("peg.remove", &json!({"peg": 1})).unwrap();
        assert_eq!(r["removed"], 1);
        let pegs = info(&mut s)["pegs"].clone();
        assert_eq!(pegs.as_array().unwrap().len(), 2);
        assert_eq!((pegs[1]["name"].as_str(), pegs[1]["parent"].as_u64()), (Some("Peg 3"), Some(0)));
    }

    #[test]
    fn edits_undo() {
        let mut s = session();
        let r = row(&mut s, 0.0);
        s.execute("peg.add", &json!({"rows": [r]})).unwrap();
        s.execute("peg.key", &json!({"peg": 0, "frame": 1, "x": 50})).unwrap();
        assert_eq!(info(&mut s)["pegs"][0]["keys"].as_array().unwrap().len(), 1);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(drawn(&s, r, 1).x0, 0.0);
        s.execute("edit.undo", &json!({})).unwrap();
        assert!(info(&mut s)["pegs"].as_array().unwrap().is_empty());
    }

    #[test]
    fn attach_detach_and_remove_key() {
        let mut s = session();
        let r = row(&mut s, 0.0);
        s.execute("peg.add", &json!({})).unwrap();
        s.execute("peg.add", &json!({})).unwrap();
        assert_eq!(s.execute("peg.attach", &json!({"peg": 1, "row": r})).unwrap()["rows"], json!([r]));
        assert_eq!(s.execute("peg.attach", &json!({"peg": 0, "row": r})).unwrap()["rows"], json!([r]));
        assert_eq!(info(&mut s)["pegs"][1]["rows"], json!([]), "moved off the other peg");
        assert_eq!(s.execute("peg.detach", &json!({"row": r})).unwrap()["detached"], true);
        assert_eq!(s.execute("peg.detach", &json!({"row": r})).unwrap()["detached"], false);
        s.execute("peg.key", &json!({"peg": 0, "frame": 1, "x": 5})).unwrap();
        s.execute("peg.removeKey", &json!({"peg": 0, "frame": 1})).unwrap();
        assert!(s.execute("peg.removeKey", &json!({"peg": 0, "frame": 1})).is_err());
        assert_eq!(info(&mut s)["pegs"][0]["keys"], json!([]));
    }

    #[test]
    fn bad_peg_inputs_are_errors() {
        let mut s = session();
        assert!(s.execute("peg.key", &json!({"peg": 0, "x": 1})).is_err(), "no peg yet");
        assert!(s.execute("peg.add", &json!({"x": "40"})).is_err());
        assert!(s.execute("peg.add", &json!({"name": ""})).is_err());
        assert!(s.execute("peg.add", &json!({"parent": 0})).is_err(), "no peg 0 yet");
        assert!(s.execute("peg.add", &json!({"rows": [9999]})).is_err(), "not a layer");
        let r = row(&mut s, 0.0);
        s.execute("peg.add", &json!({"rows": [r]})).unwrap();
        for (label, p) in [
            ("peg out of range", json!({"peg": 1, "x": 1})),
            ("frame 0", json!({"peg": 0, "frame": 0, "x": 1})),
            ("text move", json!({"peg": 0, "x": "1"})),
            ("zero scale", json!({"peg": 0, "sx": 0})),
            ("huge scale", json!({"peg": 0, "sy": 1e6})),
            ("unknown ease", json!({"peg": 0, "ease": "wobble"})),
        ] {
            assert!(s.execute("peg.key", &p).is_err(), "{label}");
        }
        assert!(s.execute("peg.attach", &json!({"peg": 5, "row": r})).is_err());
        assert!(s.execute("peg.remove", &json!({"peg": 3})).is_err());
        assert!(s.execute("peg.add", &json!({"parent": 0, "name": "x", "bogus": 1})).is_err(), "unknown key");
        assert_eq!(info(&mut s)["pegs"].as_array().unwrap().len(), 1);
    }
}
