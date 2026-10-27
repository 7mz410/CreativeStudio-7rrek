//! 7rrek (Creative Studio): the node view, as in Harmony. The graph is not stored: it is read off
//! the scene (rows, pegs, cutters, row effects) every time, and rewiring a cable edits those, so
//! the graph and the render can never disagree. Node ids: `row:<layer id>`, `peg:<index>`,
//! `cut:<cut row id>`, `fx:<row id>:<index>`, `comp` (the one composite). Cables run from an
//! output to a node's `image`, `matte` or `transform` port; the composite takes rows in timeline
//! order.

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
            "{} → {nodes: [{id, kind: \"row\"|\"peg\"|\"cutter\"|\"effect\"|\"composite\", name?, inverted?, fx?}], edges: [{from, to, port: \"image\"|\"matte\"|\"transform\"}]}",
            has_doc,
            graph
        ),
        cmd!(
            "nodes.connect",
            "Connect Nodes",
            [],
            None,
            "{from, to, port} peg → row or peg (transform: carries it / hangs it from an earlier peg); row → cut (image: that row is cut instead; matte: cuts by it); row → row (matte: adds a cutter) → {from, to, port}",
            has_doc,
            connect
        ),
        cmd!(
            "nodes.disconnect",
            "Disconnect Nodes",
            [],
            None,
            "{to, port} the row leaves its peg, the peg its parent, or the cutter goes (image or matte) → {to, port}",
            has_doc,
            disconnect
        ),
    ]
}

fn graph(s: &mut Session, _: &Value) -> Result<Value> {
    let d = &s.doc()?.doc;
    let (pegs, cuts, fx) = (Pegs::of(d), Cutters::of(d), RowFxList::of(d));
    let (mut nodes, mut edges) = (Vec::new(), Vec::new());
    let mut edge = |from: String, to: String, port: &str| edges.push(json!({"from": from, "to": to, "port": port}));
    for (i, p) in pegs.pegs.iter().enumerate() {
        nodes.push(json!({"id": format!("peg:{i}"), "kind": "peg", "name": p.name}));
        if let Some(q) = p.parent {
            edge(format!("peg:{q}"), format!("peg:{i}"), "transform");
        }
        for r in &p.rows {
            edge(format!("peg:{i}"), format!("row:{r}"), "transform");
        }
    }
    for l in &d.layers {
        nodes.push(json!({"id": format!("row:{}", l.id.0), "kind": "row", "name": l.name}));
    }
    for c in &cuts.cutters {
        nodes.push(json!({"id": format!("cut:{}", c.row), "kind": "cutter", "inverted": c.inverted}));
        edge(format!("row:{}", c.row), format!("cut:{}", c.row), "image");
        edge(format!("row:{}", c.matte), format!("cut:{}", c.row), "matte");
    }
    nodes.push(json!({"id": "comp", "kind": "composite"}));
    for l in &d.layers {
        let r = l.id.0;
        // A matte row's art isn't drawn, so it doesn't reach the composite.
        if cuts.cutters.iter().any(|c| c.matte == r && c.row != r) {
            continue;
        }
        let mut last = if cuts.cutters.iter().any(|c| c.row == r) { format!("cut:{r}") } else { format!("row:{r}") };
        for (i, e) in fx.rows.iter().filter(|f| f.row == r).flat_map(|f| &f.effects).enumerate() {
            let id = format!("fx:{r}:{i}");
            nodes.push(json!({"id": id, "kind": "effect", "fx": e.kind.as_str()}));
            edge(last, id.clone(), "image");
            last = id;
        }
        edge(last, "comp".into(), "image");
    }
    Ok(json!({"nodes": nodes, "edges": edges}))
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

fn connect(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "nodes.connect";
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
            let mut c = old.clone().unwrap_or(Cutter { row: r, matte: m, inverted: false });
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

    fn has(g: &Value, from: &str, to: &str, port: &str) -> bool {
        g["edges"].as_array().unwrap().contains(&json!({"from": from, "to": to, "port": port}))
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
}
