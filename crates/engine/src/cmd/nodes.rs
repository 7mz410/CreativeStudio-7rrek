//! 7rrek (Creative Studio): the node view, as in Harmony. The graph is not stored: it is read off
//! the scene (rows, pegs, cutters, row effects) every time, and rewiring a cable edits those, so
//! the graph and the render can never disagree. Node ids: `row:<layer id>`, `peg:<index>`,
//! `cut:<cut row id>`, `fx:<row id>:<index>`, `comp` (the one composite). Cables run from an
//! output to a node's `image`, `matte` or `transform` port. The composite takes rows in timeline
//! order (its cables carry `order`, 0 at the bottom): plugging a row in above or below another
//! moves the row, and unplugging one stops it being drawn (`Anim.unplugged`).

use serde_json::json;
use vectorcraft_doc::Document;

use super::anim::{Anim, show};
use super::cutter::{Cutter, Cutters, MAX_CUTTERS};
use super::peg::Pegs;
use super::row_fx::RowFxList;
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "nodes.graph",
            "Node Graph",
            [],
            None,
            "{} → {nodes: [{id, kind: \"row\"|\"peg\"|\"cutter\"|\"effect\"|\"composite\", name?, inverted?, fx?, ports: {in, out}}], edges: [{from, to, port: \"image\"|\"matte\"|\"transform\"}], groups: [{id, name, members, collapsed}], display: id|null, write: id|null}",
            has_doc,
            graph
        ),
        cmd!(
            "nodes.connect",
            "Connect Nodes",
            [],
            None,
            "{from, to, port, above?|below?: row node} peg → row or peg (transform: carries it / hangs it from an earlier peg); row → cut (image: that row is cut instead; matte: cuts by it); row → row (matte: adds a cutter); row (or the last node of its chain) → comp (image: the row is drawn again; with above/below it moves above or below that row, which is the composite order) → {from, to, port}",
            has_doc,
            connect
        ),
        cmd!(
            "nodes.disconnect",
            "Disconnect Nodes",
            [],
            None,
            "{to, port, from?} the row leaves its peg, the peg its parent, or the cutter goes (image or matte); to comp (with from: the row, or the last node of its chain) the row is no longer drawn → {to, port}",
            has_doc,
            disconnect
        ),
        cmd!(
            "nodes.lock",
            "Lock Node Cache",
            [],
            None,
            "{node: row, effect or cutter id, on?: bool (default true)} cache-locks that node's row: while the row comes out of a frame unchanged the renderer reuses its effects → {row, on}",
            has_doc,
            lock_cmd
        ),
        cmd!(
            "nodes.display",
            "Display Node",
            [],
            None,
            "{node?: id|null} the canvas shows only the rows feeding that node; null (or none) shows every row → {node}",
            has_doc,
            display_cmd
        ),
        cmd!(
            "nodes.write",
            "Write Node",
            [],
            None,
            "{node?: id|null} exports only the rows feeding that node; null (or none) exports every row → {node}",
            has_doc,
            write_cmd
        ),
    ]
}

/// Cache-locks the row a node belongs to (`on: false` unlocks it). One undo step.
fn lock_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "nodes.lock";
    let id = p.get("node").and_then(Value::as_str).ok_or_else(|| bad(C, "`node` must be a node id"))?;
    let on = p.get("on").and_then(Value::as_bool).unwrap_or(true);
    let row = id
        .strip_prefix("row:")
        .or_else(|| id.strip_prefix("fx:"))
        .or_else(|| id.strip_prefix("cut:"))
        .and_then(|r| r.split(':').next()?.parse::<u64>().ok())
        .filter(|r| s.doc().is_ok_and(|d| d.doc.layers.iter().any(|l| l.id.0 == *r)))
        .ok_or_else(|| bad(C, format!("{id} isn't a row, effect or cutter node")))?;
    s.edit(if on { "Lock Node Cache" } else { "Unlock Node Cache" }, |d, _| {
        let mut a = Anim::of(d);
        if on {
            a.locked.insert(row);
        } else {
            a.locked.remove(&row);
        }
        a.store(d);
        Ok(())
    })?;
    Ok(json!({"row": row, "on": on}))
}

fn display_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    set_branch(s, p, "nodes.display", "Display Node", false)
}

fn write_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    set_branch(s, p, "nodes.write", "Write Node", true)
}

/// Sets `Anim.write` (exports) or `Anim.display` (canvas) to a node, or clears it with null. One undo step.
fn set_branch(s: &mut Session, p: &Value, cmd: &str, label: &str, export: bool) -> Result<Value> {
    let node = match p.get("node") {
        None | Some(Value::Null) => None,
        Some(v) => Some(v.as_str().ok_or_else(|| bad(cmd, "`node` must be a node id or null"))?.to_string()),
    };
    if let Some(id) = &node
        && branch_rows(&s.doc()?.doc, id).is_none()
    {
        return Err(bad(cmd, format!("{id} isn't a node in the graph")));
    }
    s.edit(label, |d, _| {
        let mut a = Anim::of(d);
        if export {
            a.write = node.clone();
        } else {
            a.display = node.clone();
        }
        a.store(d);
        Ok(())
    })?;
    let current = Anim::of(&s.doc()?.doc).current;
    show(s, current)?;
    Ok(json!({"node": node}))
}

fn graph(s: &mut Session, _: &Value) -> Result<Value> {
    let d = &s.doc()?.doc;
    let (pegs, cuts, fx) = (Pegs::of(d), Cutters::of(d), RowFxList::of(d));
    let groups = super::node_groups::NodeGroups::of(d);
    let (mut nodes, mut edges) = (Vec::new(), Vec::new());
    let edge = |from: String, to: String, port: &str| json!({"from": from, "to": to, "port": port});
    for (i, p) in pegs.pegs.iter().enumerate() {
        nodes.push(json!({
            "id": format!("peg:{i}"),
            "kind": "peg",
            "name": p.name,
            "ports": {
                "in": ["transform"],
                "out": ["transform"]
            }
        }));
        if let Some(q) = p.parent {
            edges.push(edge(format!("peg:{q}"), format!("peg:{i}"), "transform"));
        }
        for r in &p.rows {
            edges.push(edge(format!("peg:{i}"), format!("row:{r}"), "transform"));
        }
    }
    for l in &d.layers {
        nodes.push(json!({
            "id": format!("row:{}", l.id.0),
            "kind": "row",
            "name": l.name,
            "ports": {
                "in": ["transform", "matte"],
                "out": ["image"]
            }
        }));
    }
    for c in &cuts.cutters {
        nodes.push(json!({
            "id": format!("cut:{}", c.row),
            "kind": "cutter",
            "inverted": c.inverted,
            "ports": {
                "in": ["image", "matte"],
                "out": ["image"]
            }
        }));
        edges.push(edge(format!("row:{}", c.row), format!("cut:{}", c.row), "image"));
        edges.push(edge(format!("row:{}", c.matte), format!("cut:{}", c.row), "matte"));
    }
    nodes.push(json!({
        "id": "comp",
        "kind": "composite",
        "ports": {
            "in": ["image"],
            "out": ["image"]
        }
    }));
    let Anim { unplugged, display, write, locked, .. } = Anim::of(d);
    let mut order = 0;
    for l in &d.layers {
        let r = l.id.0;
        // A matte row's art isn't drawn, so it doesn't reach the composite.
        if cuts.cutters.iter().any(|c| c.matte == r && c.row != r) {
            continue;
        }
        let mut last = if cuts.cutters.iter().any(|c| c.row == r) { format!("cut:{r}") } else { format!("row:{r}") };
        for (i, e) in fx.rows.iter().filter(|f| f.row == r).flat_map(|f| &f.effects).enumerate() {
            let id = format!("fx:{r}:{i}");
            nodes.push(json!({
                "id": id,
                "kind": "effect",
                "fx": e.kind.as_str(),
                "ports": {
                    "in": ["image"],
                    "out": ["image"]
                }
            }));
            edges.push(edge(last, id.clone(), "image"));
            last = id;
        }
        if !unplugged.contains(&r) {
            edges.push(json!({"from": last, "to": "comp", "port": "image", "order": order}));
            order += 1;
        }
    }
    let groups_json: Vec<Value> = groups
        .groups
        .iter()
        .map(|g| {
            json!({
                "id": g.id,
                "name": g.name,
                "members": g.members,
                "collapsed": g.collapsed,
            })
        })
        .collect();
    Ok(json!({"nodes": nodes, "edges": edges, "groups": groups_json, "display": display, "write": write, "locked": locked}))
}

/// The rows that feed node `id` (the composite: every row); None for an id that isn't in the graph.
pub fn branch_rows(d: &Document, id: &str) -> Option<std::collections::BTreeSet<u64>> {
    if id == "comp" {
        return Some(d.layers.iter().map(|l| l.id.0).collect());
    }
    let (kind, rest) = id.split_once(':')?;
    let row: u64 = rest.split(':').next()?.parse().ok()?;
    d.layers.iter().any(|l| l.id.0 == row).then_some(())?;
    let mut rows = std::collections::BTreeSet::from([row]);
    match kind {
        "row" | "fx" => {}
        // A cutter's output also takes its matte row.
        "cut" => rows.extend(Cutters::of(d).cutters.iter().filter(|c| c.row == row).map(|c| c.matte)),
        _ => return None,
    }
    Some(rows)
}

/// A node id's kind and number (`fx` ids aren't wired by hand).
fn parse<'a>(v: &'a Value, key: &str, cmd: &str) -> Result<(&'a str, u64)> {
    let s = v.get(key).and_then(Value::as_str).ok_or_else(|| bad(cmd, format!("`{key}` must be a node id")))?;
    s.split_once(':')
        .and_then(|(k, n)| Some((k, n.parse().ok()?)))
        .filter(|(k, _)| ["row", "peg", "cut"].contains(k))
        .ok_or_else(|| bad(cmd, format!("`{key}`: {s} isn't a row, peg or cut node")))
}

fn port<'a>(p: &'a Value, cmd: &str) -> Result<&'a str> {
    p.get("port")
        .and_then(Value::as_str)
        .filter(|q| ["image", "matte", "transform"].contains(q))
        .ok_or_else(|| bad(cmd, "`port` must be image, matte or transform"))
}

fn row(d: &Document, id: u64, cmd: &str) -> Result<u64> {
    d.layers.iter().any(|l| l.id.0 == id).then_some(id).ok_or_else(|| bad(cmd, format!("row:{id} isn't a timeline row")))
}

fn commit(s: &mut Session, label: &str, pegs: Option<Pegs>, cuts: Option<Cutters>, cmd: &str) -> Result<()> {
    if pegs.as_ref().is_some_and(|p| !p.valid()) {
        return Err(bad(cmd, "the edit would leave invalid pegs"));
    }
    s.edit(label, |d, _| {
        if let Some(p) = &pegs {
            p.store(d);
        }
        if let Some(c) = &cuts {
            c.store(d);
        }
        Ok(())
    })?;
    let current = Anim::of(&s.doc()?.doc).current;
    show(s, current)?;
    Ok(())
}

/// The row a cable into the composite carries: `row:<id>`, `cut:<id>` or `fx:<id>:<i>` all
/// stand for row `id`.
fn comp_row(d: &Document, v: &Value, cmd: &str) -> Result<u64> {
    let s = v.as_str().ok_or_else(|| bad(cmd, "`from` must be a node id"))?;
    let id = s.split(':').nth(1).and_then(|n| n.parse().ok()).filter(|_| ["row:", "cut:", "fx:"].iter().any(|k| s.starts_with(k)));
    row(d, id.ok_or_else(|| bad(cmd, format!("{s} doesn't carry a row into the composite")))?, cmd)
}

/// Plugs a row into the composite (drawn again), above or below another row if asked.
fn plug(s: &mut Session, p: &Value, on: bool) -> Result<Value> {
    let cmd = if on { "nodes.connect" } else { "nodes.disconnect" };
    let d = &s.doc()?.doc;
    let r = comp_row(d, p.get("from").unwrap_or(&Value::Null), cmd)?;
    let place = ["above", "below"].into_iter().find_map(|k| p.get(k).map(|v| (k, v)));
    let target = match place {
        Some((k, v)) => Some((k, comp_row(d, v, cmd)?)),
        None => None,
    };
    let mut a = Anim::of(d);
    let changed = if on { a.unplugged.remove(&r) } else { a.unplugged.insert(r) };
    if !changed && target.is_none() {
        return Err(bad(cmd, if on { "that row is already drawn" } else { "no cable there" }));
    }
    s.begin_named_undo_group(Some(if on { "Connect Nodes".into() } else { "Disconnect Nodes".into() }));
    let done = (|| -> Result<()> {
        if changed {
            s.edit(if on { "Plug Row" } else { "Unplug Row" }, |d, _| {
                let mut b = Anim::of(d);
                b.unplugged = a.unplugged.clone();
                b.store(d);
                Ok(())
            })?;
        }
        if let Some((place, t)) = target.filter(|(_, t)| *t != r) {
            s.execute("layer.move", &json!({"id": r, "target": t, "place": place}))?;
        }
        let current = Anim::of(&s.doc()?.doc).current;
        show(s, current)?;
        Ok(())
    })();
    s.end_undo_group(done.is_err());
    done?;
    Ok(json!({"from": p["from"], "to": "comp", "port": "image"}))
}

fn connect(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "nodes.connect";
    if p.get("to").and_then(Value::as_str) == Some("comp") {
        return plug(s, p, true);
    }
    let (from, to, port) = (parse(p, "from", C)?, parse(p, "to", C)?, port(p, C)?);
    let d = &s.doc()?.doc;
    let (mut pegs, mut cuts) = (Pegs::of(d), Cutters::of(d));
    let peg = |i: u64, pegs: &Pegs| (i as usize) < pegs.pegs.len();
    match (from, to, port) {
        (("peg", i), ("row", r), "transform") if peg(i, &pegs) => {
            let r = row(d, r, C)?;
            pegs.pegs.iter_mut().for_each(|q| q.rows.retain(|x| *x != r));
            pegs.pegs[i as usize].rows.push(r);
            commit(s, "Attach to Peg", Some(pegs), None, C)?;
        }
        (("peg", i), ("peg", c), "transform") if peg(i, &pegs) && peg(c, &pegs) => {
            // shortcut: a peg hangs only from an earlier one (Pegs::valid); reorder pegs when that bites.
            if i >= c {
                return Err(bad(C, "a peg can only hang from an earlier peg"));
            }
            pegs.pegs[c as usize].parent = Some(i as usize);
            commit(s, "Parent Peg", Some(pegs), None, C)?;
        }
        (("row", m), ("cut", r), "image" | "matte") | (("row", m), ("row", r), "matte") => {
            let (m, r) = (row(d, m, C)?, row(d, r, C)?);
            let old = cuts.cutters.iter().find(|c| c.row == r).cloned();
            let mut c = old.clone().unwrap_or(Cutter { row: r, matte: m, ..Cutter::default() });
            if to.0 == "cut" && old.is_none() {
                return Err(bad(C, format!("cut:{r} doesn't exist")));
            }
            if port == "image" {
                c.row = m
            } else {
                c.matte = m
            }
            if c.row == c.matte {
                return Err(bad(C, "a row can't cut itself"));
            }
            // A row has one cutter: the one moved onto it replaces any it had.
            cuts.cutters.retain(|x| x.row != r && x.row != c.row);
            if cuts.cutters.len() >= MAX_CUTTERS {
                return Err(bad(C, format!("a document takes at most {MAX_CUTTERS} cutters")));
            }
            cuts.cutters.push(c);
            commit(s, "Rewire Cutter", None, Some(cuts), C)?;
        }
        _ => return Err(bad(C, "those ports can't be wired together")),
    }
    Ok(json!({"from": p["from"], "to": p["to"], "port": port}))
}

fn disconnect(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "nodes.disconnect";
    if p.get("to").and_then(Value::as_str) == Some("comp") {
        return plug(s, p, false);
    }
    let (to, port) = (parse(p, "to", C)?, port(p, C)?);
    let d = &s.doc()?.doc;
    let (mut pegs, mut cuts) = (Pegs::of(d), Cutters::of(d));
    match (to, port) {
        (("row", r), "transform") if pegs.pegs.iter().any(|q| q.rows.contains(&r)) => {
            pegs.pegs.iter_mut().for_each(|q| q.rows.retain(|x| *x != r));
            commit(s, "Detach from Peg", Some(pegs), None, C)?;
        }
        (("peg", i), "transform") if pegs.pegs.get(i as usize).is_some_and(|q| q.parent.is_some()) => {
            pegs.pegs[i as usize].parent = None;
            commit(s, "Unparent Peg", Some(pegs), None, C)?;
        }
        (("cut", r), "image" | "matte") if cuts.cutters.iter().any(|c| c.row == r) => {
            cuts.cutters.retain(|c| c.row != r);
            commit(s, "Remove Cutter", None, Some(cuts), C)?;
        }
        _ => return Err(bad(C, "no cable there")),
    }
    Ok(json!({"to": p["to"], "port": port}))
}

#[cfg(test)]
mod tests {
    use vectorcraft_geom::Rect;

    use super::*;

    /// Three keyed rows: red 100 pt square, blue 50 pt square in its middle, green 20 pt square
    /// in the corner. Their ids.
    fn scene() -> (Session, [u64; 3]) {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        let mut ids = [0; 3];
        for (i, (x, w, c)) in [(0, 100, "#ff0000"), (25, 50, "#0000ff"), (0, 20, "#00ff00")].into_iter().enumerate() {
            if i > 0 {
                let id = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
                s.execute("layer.setCurrent", &json!({"id": id})).unwrap();
            }
            s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
            s.execute("shape.rectangle", &json!({"x": x, "y": x, "width": w, "height": w})).unwrap();
            s.execute("paint.setFill", &json!({"color": c})).unwrap();
        }
        for (i, l) in s.doc().unwrap().doc.layers.iter().enumerate() {
            ids[i] = l.id.0;
        }
        (s, ids)
    }

    fn at(s: &Session, x: u32, y: u32) -> [u8; 3] {
        let d = crate::cmd::anim::at_frame(&s.doc().unwrap().doc, 1);
        let img = vectorcraft_render::Renderer::new().render_region(&d, Rect::new(0.0, 0.0, 100.0, 100.0), 1.0, true);
        let px = img.to_straight();
        let i = ((y * img.width + x) * 4) as usize;
        [px[i], px[i + 1], px[i + 2]]
    }

    #[test]
    fn write_node_exports_only_its_branch_and_display_only_shows_its_branch() {
        let (mut s, [red, blue, _]) = scene();
        let mut a = Anim::of(&s.doc().unwrap().doc);
        (a.write, a.display) = (Some(format!("row:{blue}")), Some(format!("row:{red}")));
        let st = s.doc_mut().unwrap();
        a.store(std::sync::Arc::make_mut(&mut st.doc));
        // Export: the blue square on white, no red around it, no green corner.
        assert_eq!(at(&s, 50, 50), [0, 0, 255]);
        assert_eq!(at(&s, 5, 5), [255, 255, 255]);
        // Canvas: only red rows stay visible.
        let d = crate::cmd::anim::at_frame_with(&s.doc().unwrap().doc, 1, true);
        let vis: Vec<u64> = d.layers.iter().filter(|l| l.visible).map(|l| l.id.0).collect();
        assert_eq!(vis, vec![red]);
        // Back at rest every row shows again.
        let rest = crate::cmd::anim::unposed(&d);
        assert!(rest.layers.iter().all(|l| l.visible));
        assert_eq!(branch_rows(&rest, "comp").map(|r| r.len()), Some(3));
        assert_eq!(branch_rows(&rest, "row:999999"), None);
        assert_eq!(branch_rows(&rest, "peg:0"), None);
    }

    fn has(g: &Value, from: &str, to: &str, port: &str) -> bool {
        g["edges"].as_array().unwrap().iter().any(|e| e["from"] == from && e["to"] == to && e["port"] == port)
    }

    #[test]
    fn the_graph_shows_the_wiring_and_rewiring_a_cutter_changes_the_render() {
        let (mut s, [red, blue, green]) = scene();
        s.execute("peg.add", &json!({})).unwrap();
        s.execute("fx.add", &json!({"row": red, "kind": "blur"})).unwrap();
        s.execute("nodes.connect", &json!({"from": format!("row:{blue}"), "to": format!("row:{red}"), "port": "matte"})).unwrap();
        s.execute("nodes.connect", &json!({"from": "peg:0", "to": format!("row:{green}"), "port": "transform"})).unwrap();
        let g = s.execute("nodes.graph", &json!({})).unwrap();
        assert!(has(&g, &format!("row:{red}"), &format!("cut:{red}"), "image"));
        assert!(has(&g, &format!("row:{blue}"), &format!("cut:{red}"), "matte"));
        assert!(has(&g, &format!("cut:{red}"), &format!("fx:{red}:0"), "image"));
        assert!(has(&g, &format!("fx:{red}:0"), "comp", "image"));
        assert!(has(&g, "peg:0", &format!("row:{green}"), "transform"));
        assert!(!has(&g, &format!("row:{blue}"), "comp", "image"), "a matte isn't drawn");
        s.execute("fx.remove", &json!({"row": red, "index": 0})).unwrap();
        assert_eq!(at(&s, 50, 50), [255, 255, 255], "cut by blue");
        assert_eq!(at(&s, 10, 10), [0, 255, 0]);
        // Rewire the matte port to green: the middle shows red again, the corner is cut.
        s.execute("nodes.connect", &json!({"from": format!("row:{green}"), "to": format!("cut:{red}"), "port": "matte"})).unwrap();
        assert_eq!(at(&s, 50, 50), [0, 0, 255]);
        assert_eq!(at(&s, 10, 10), [255, 255, 255]);
        s.execute("nodes.disconnect", &json!({"to": format!("cut:{red}"), "port": "matte"})).unwrap();
        assert_eq!(at(&s, 10, 10), [0, 255, 0]);
        assert!(s.execute("nodes.disconnect", &json!({"to": format!("cut:{red}"), "port": "matte"})).is_err());
        assert!(s.execute("nodes.connect", &json!({"from": format!("row:{red}"), "to": format!("row:{red}"), "port": "matte"})).is_err());
        assert!(s.execute("nodes.connect", &json!({"from": "peg:0", "to": "comp", "port": "image"})).is_err());
    }

    #[test]
    fn a_peg_hangs_from_an_earlier_peg_and_only_that_way() {
        let (mut s, _) = scene();
        s.execute("peg.add", &json!({})).unwrap();
        s.execute("peg.add", &json!({})).unwrap();
        s.execute("nodes.connect", &json!({"from": "peg:0", "to": "peg:1", "port": "transform"})).unwrap();
        let g = s.execute("nodes.graph", &json!({})).unwrap();
        assert!(has(&g, "peg:0", "peg:1", "transform"));
        assert!(s.execute("nodes.connect", &json!({"from": "peg:1", "to": "peg:0", "port": "transform"})).is_err());
        s.execute("nodes.disconnect", &json!({"to": "peg:1", "port": "transform"})).unwrap();
        let g = s.execute("nodes.graph", &json!({})).unwrap();
        assert!(!has(&g, "peg:0", "peg:1", "transform"));
    }

    #[test]
    fn moving_the_image_port_moves_the_cutter_to_the_new_row() {
        let (mut s, [red, blue, green]) = scene();
        s.execute("nodes.connect", &json!({"from": format!("row:{blue}"), "to": format!("row:{red}"), "port": "matte"})).unwrap();
        s.execute("nodes.connect", &json!({"from": format!("row:{green}"), "to": format!("cut:{red}"), "port": "image"})).unwrap();
        let g = s.execute("nodes.graph", &json!({})).unwrap();
        let has_node = |id: String| g["nodes"].as_array().unwrap().iter().any(|n| n["id"] == id.as_str());
        assert!(has_node(format!("cut:{green}")));
        assert!(!has_node(format!("cut:{red}")));
        assert!(has(&g, &format!("row:{green}"), &format!("cut:{green}"), "image"));
        assert!(has(&g, &format!("row:{blue}"), &format!("cut:{green}"), "matte"));
    }

    #[test]
    fn undo_takes_back_one_connect() {
        let (mut s, [red, blue, _]) = scene();
        s.execute("nodes.connect", &json!({"from": format!("row:{blue}"), "to": format!("row:{red}"), "port": "matte"})).unwrap();
        assert_eq!(s.execute("cutter.list", &json!({})).unwrap()["cutters"].as_array().unwrap().len(), 1);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.execute("cutter.list", &json!({})).unwrap()["cutters"], json!([]));
    }

    #[test]
    fn bad_node_ids_and_ports_are_errors() {
        let (mut s, [red, blue, _]) = scene();
        assert!(s.execute("nodes.connect", &json!({"to": format!("row:{red}"), "port": "matte"})).is_err());
        assert!(s.execute("nodes.connect", &json!({"from": "fx:1:0", "to": format!("row:{red}"), "port": "image"})).is_err());
        assert!(s.execute("nodes.connect", &json!({"from": format!("row:{blue}"), "to": format!("row:{red}"), "port": "foo"})).is_err());
        assert!(s.execute("nodes.connect", &json!({"from": format!("row:{blue}"), "to": "row:999999", "port": "matte"})).is_err());
        assert!(s.execute("nodes.disconnect", &json!({"to": "row:999999", "port": "transform"})).is_err());
        assert_eq!(s.execute("cutter.list", &json!({})).unwrap()["cutters"], json!([]));
    }

    /// C1: the composite's cables are the render order; unplugging a row stops it being drawn, and
    /// rewiring two rows' order swaps them in the export.
    #[test]
    fn the_composite_order_and_its_cables_are_the_render() {
        let (mut s, [red, blue, green]) = scene();
        let comp_order = |s: &mut Session| -> Vec<String> {
            let g = s.execute("nodes.graph", &json!({})).unwrap();
            let mut e: Vec<(u64, String)> = g["edges"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|e| e["to"] == "comp")
                .map(|e| (e["order"].as_u64().unwrap(), e["from"].as_str().unwrap().to_string()))
                .collect();
            e.sort();
            e.into_iter().map(|(_, f)| f).collect()
        };
        assert_eq!(comp_order(&mut s), [format!("row:{red}"), format!("row:{blue}"), format!("row:{green}")]);
        assert_eq!(at(&s, 50, 50), [0, 0, 255], "blue over red");
        // Plug red in above blue: red is drawn over it.
        s.execute("nodes.connect", &json!({"from": format!("row:{red}"), "to": "comp", "port": "image", "above": format!("row:{blue}")})).unwrap();
        assert_eq!(comp_order(&mut s)[..2], [format!("row:{blue}"), format!("row:{red}")]);
        assert_eq!(at(&s, 50, 50), [255, 0, 0], "red over blue now");
        // Unplugged, red isn't drawn, on the canvas or in exports.
        s.execute("nodes.disconnect", &json!({"from": format!("row:{red}"), "to": "comp", "port": "image"})).unwrap();
        assert_eq!(at(&s, 50, 50), [0, 0, 255]);
        assert_eq!(at(&s, 90, 90), [255, 255, 255], "red's corner is gone too");
        assert!(!comp_order(&mut s).contains(&format!("row:{red}")));
        assert!(s.execute("nodes.disconnect", &json!({"from": format!("row:{red}"), "to": "comp", "port": "image"})).is_err());
        // One undo plugs it back; the art itself never changed.
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(at(&s, 50, 50), [255, 0, 0]);
        assert!(s.execute("nodes.connect", &json!({"from": "peg:9", "to": "comp", "port": "image"})).is_err());
    }

    #[test]
    fn lock_marks_the_row_and_refuses_other_nodes() {
        let (mut s, [red, ..]) = scene();
        assert_eq!(s.execute("nodes.lock", &json!({"node": format!("row:{red}")})).unwrap()["row"], json!(red));
        assert_eq!(s.execute("nodes.graph", &json!({})).unwrap()["locked"], json!([red]));
        s.execute("nodes.lock", &json!({"node": format!("row:{red}"), "on": false})).unwrap();
        assert_eq!(s.execute("nodes.graph", &json!({})).unwrap()["locked"], json!([]));
        assert!(s.execute("nodes.lock", &json!({"node": "comp"})).is_err());
        assert!(s.execute("nodes.lock", &json!({"node": "row:999999"})).is_err());
        assert!(s.execute("nodes.lock", &json!({})).is_err());
    }

    #[test]
    fn display_and_write_nodes_are_set_read_cleared_and_undone() {
        let (mut s, [red, blue, _]) = scene();
        let fixed = |s: &mut Session| {
            let g = s.execute("nodes.graph", &json!({})).unwrap();
            (g["display"].clone(), g["write"].clone())
        };
        assert_eq!(fixed(&mut s), (Value::Null, Value::Null));
        s.execute("nodes.display", &json!({"node": format!("row:{red}")})).unwrap();
        s.execute("nodes.write", &json!({"node": format!("row:{blue}")})).unwrap();
        assert_eq!(fixed(&mut s), (json!(format!("row:{red}")), json!(format!("row:{blue}"))));
        s.execute("nodes.display", &json!({"node": null})).unwrap();
        assert_eq!(fixed(&mut s), (Value::Null, json!(format!("row:{blue}"))));
        // One undo step per command: the clear goes back to red, the next undo clears the write.
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(fixed(&mut s), (json!(format!("row:{red}")), json!(format!("row:{blue}"))));
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(fixed(&mut s), (json!(format!("row:{red}")), Value::Null));
        // Bad ids and non-id values are errors and change nothing.
        assert!(s.execute("nodes.display", &json!({"node": "row:999999"})).is_err());
        assert!(s.execute("nodes.write", &json!({"node": "peg:0"})).is_err());
        assert!(s.execute("nodes.write", &json!({"node": 5})).is_err());
        assert_eq!(fixed(&mut s), (json!(format!("row:{red}")), Value::Null));
        // The composite is every row, so it is a valid node.
        s.execute("nodes.write", &json!({"node": "comp"})).unwrap();
        assert_eq!(fixed(&mut s).1, json!("comp"));
    }
}
