//! 7rrek (Creative Studio): P2 auto-rig. `character.fromRsm` turns a file that follows the character
//! contract (plans/character-contract.md, checked by `character.check`) into timeline rows: one row
//! per part, one peg per part hanging from its contract parent's peg and turning about the part's
//! pivot, and the part's alternative drawings (`hand_L.fist`, `mouth.A`…) as the row's drawings.
//! shortcut: pegs only, no bones or IK chains (contract section 5) and one view per call; add them
//! when P3/P5 need `rig.reach` or turnarounds.

use std::collections::{BTreeMap, HashMap};

use serde_json::json;
use vectorcraft_doc::{Document, LayerColor, Node, NodeId};
use vectorcraft_geom::Rect;

use super::anim::Anim;
use super::character_contract::{PARTS, REQUIRED_VIEWS, check_doc, part_spec};
use super::peg::{MAX_PEGS, Peg, Pegs};
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "character.fromRsm",
        "Rig Character from File",
        [],
        None,
        "{path: a file that follows the character contract (.rsm), name?: the character's name (default: the file's name), view?: \"front\" | \"3q\" | \"side\" (default front)} adds one row and one peg per part, both named `<name>/<part>` so action.apply {character: name} finds them (pegs hang as in the contract's tree, pivots from each part's `pivot` object or the contract's default), and each part's `<part>.<drawing>` alternatives as its drawings, the default one exposed → {character, peg: the whole character's peg (named after it, every part hangs from it), rows: {part: layer id}, pegs: {part: peg index}, warnings}",
        has_doc,
        from_rsm
    )]
}

/// A part read off the view layer: its group and where it turns.
struct Part<'a> {
    name: &'a str,
    node: &'a Node,
    pivot: [f64; 2],
}

fn from_rsm(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "character.fromRsm";
    let path = str_param(p, "path").filter(|x| !x.trim().is_empty()).ok_or_else(|| bad(C, "path is required"))?.to_string();
    let view = str_param(p, "view").unwrap_or("front");
    if !REQUIRED_VIEWS.contains(&view) {
        return Err(bad(C, format!("view {view:?} isn't one of front, 3q, side")));
    }
    let name = match str_param(p, "name").map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => n.to_string(),
        None => std::path::Path::new(&path).file_stem().and_then(|n| n.to_str()).unwrap_or("character").to_string(),
    };
    if name.contains('/') {
        return Err(bad(C, "the character's name can't contain /"));
    }
    let bytes = super::fileio::read_file(&path)?;
    let t = vectorcraft_format::load(&bytes).map_err(|e| bad(C, format!("cannot parse file '{path}': {e}")))?;
    let report = check_doc(&t)?;
    let missing = report["missing"][view].as_array().map(Vec::len).unwrap_or(0);
    if missing > 0 {
        return Err(bad(C, format!("view {view} lacks required parts: {}", report["missing"][view])));
    }
    let layer = t.layers.iter().find(|l| l.name.as_deref() == Some(view)).ok_or_else(|| bad(C, format!("the file has no {view} layer")))?;
    let parts = read_parts(&t, layer);
    let warnings = report["warnings"].clone();

    let (rows, pegs, peg) = s.edit("Rig Character", |d, _| {
        let mut pegs = Pegs::of(d);
        if pegs.pegs.len() + parts.len() + 1 > MAX_PEGS {
            return Err(bad(C, "the document has too many pegs for this character"));
        }
        let mut anim = Anim::of(d);
        let mut rows: BTreeMap<String, u64> = BTreeMap::new();
        let mut peg_of: HashMap<&str, usize> = HashMap::new();
        // The whole character's peg, above every part: it places and moves the character, so the
        // parts' own keys (a walk's bounce on the hips) never fight with where it stands.
        let whole = pegs.pegs.len();
        pegs.pegs.push(Peg { name: name.clone(), ..Peg::default() });
        // File order is stacking order: the first group is the bottom row.
        for part in &parts {
            let row = add_row(d, &t, part, &name, &mut anim)?;
            rows.insert(part.name.to_string(), row.0);
            peg_of.insert(part.name, pegs.pegs.len());
            pegs.pegs.push(Peg { name: part.name.to_string(), parent: None, pivot: part.pivot, rows: vec![row.0], keys: BTreeMap::new() });
        }
        // Parents must come before children, so pegs are ordered by the contract tree, not by
        // stacking order, before the parent links are set.
        let base = whole + 1;
        order_pegs(&mut pegs, &mut peg_of, base);
        let root_pivot = pegs.pegs.get(base).map(|p| p.pivot).unwrap_or_default();
        for peg in pegs.pegs.iter_mut().skip(base) {
            peg.name = format!("{name}/{}", peg.name);
            peg.parent = peg.parent.or(Some(whole));
        }
        if let Some(w) = pegs.pegs.get_mut(whole) {
            w.pivot = root_pivot;
        }
        anim.store(d);
        pegs.store(d);
        let pegs_json: BTreeMap<String, usize> = peg_of.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        Ok((rows, pegs_json, whole))
    })?;
    Ok(json!({"character": name, "peg": peg, "rows": rows, "pegs": pegs, "warnings": warnings}))
}

/// The nearest ancestor in the contract tree that this character has a peg for.
fn contract_parent(name: &str, peg_of: &HashMap<&str, usize>) -> Option<usize> {
    let mut at = part_spec(name).and_then(|s| s.parent);
    for _ in 0..PARTS.len() {
        let p = at?;
        if let Some(i) = peg_of.get(p) {
            return Some(*i);
        }
        at = part_spec(p).and_then(|s| s.parent);
    }
    None
}

/// Re-sorts the pegs added from `base` on so every contract parent comes first, then links them.
/// `x_` parts hang from the root part's peg.
fn order_pegs(pegs: &mut Pegs, peg_of: &mut HashMap<&str, usize>, base: usize) {
    let added: Vec<Peg> = pegs.pegs.drain(base..).collect();
    let rank = |n: &str| PARTS.iter().position(|p| p.name == n).unwrap_or(PARTS.len());
    let mut sorted = added;
    sorted.sort_by_key(|p| rank(&p.name));
    let names: Vec<String> = sorted.iter().map(|p| p.name.clone()).collect();
    let mut placed: HashMap<&str, usize> = HashMap::new();
    let root = names.first().cloned();
    for (k, mut peg) in sorted.into_iter().enumerate() {
        let idx = base + k;
        peg.parent =
            if part_spec(&peg.name).is_some() { contract_parent(&peg.name, &placed) } else { root.as_deref().and_then(|r| placed.get(r).copied()) };
        if let Some(n) = names.get(k) {
            placed.insert(n.as_str(), idx);
        }
        pegs.pegs.push(peg);
    }
    for (name, idx) in peg_of.iter_mut() {
        if let Some(i) = placed.get(name) {
            *idx = *i;
        }
    }
}

/// Copies one part into a new top-level row. Alternatives become keyframe sublayers named after
/// the drawing, all keyed at frame 1 with the default exposed there, so the others wait to be
/// exposed; any other art shows on every frame. The `pivot` marker is left out.
fn add_row(d: &mut Document, t: &Document, part: &Part, character: &str, anim: &mut Anim) -> Result<NodeId> {
    let id = d.alloc_id();
    let row = d
        .insert(None, d.layers.len(), Node::layer(id, &format!("{character}/{}", part.name), LayerColor::Preset((d.layers.len() % 8) as u8)))
        .map_err(|e| EngineError::Other(e.to_string()))?;
    let prefix = format!("{}.", part.name);
    let default = part_spec(part.name).and_then(|s| s.default_drawing);
    let mut first = None;
    let mut chosen = None;
    for kid in part.node.children().into_iter().flatten() {
        let name = kid.name.as_deref().unwrap_or("");
        if name == "pivot" {
            continue;
        }
        let mut copy = d.reid(kid);
        super::character::bring_images(&mut copy, t, d);
        match name.strip_prefix(&prefix).filter(|n| !n.is_empty()) {
            Some(drawing) => {
                let sub = d.alloc_id();
                let sub = d
                    .insert(Some(row), usize::MAX, Node::layer(sub, drawing, LayerColor::Preset(0)))
                    .map_err(|e| EngineError::Other(e.to_string()))?;
                d.insert(Some(sub), 0, copy).map_err(|e| EngineError::Other(e.to_string()))?;
                anim.keys.insert(sub.0, 1);
                first.get_or_insert(sub);
                if Some(drawing) == default {
                    chosen = Some(sub);
                }
            }
            None => {
                d.insert(Some(row), usize::MAX, copy).map_err(|e| EngineError::Other(e.to_string()))?;
            }
        }
    }
    if let Some(shown) = chosen.or(first) {
        anim.exposures.entry(row.0).or_default().insert(1, shown.0);
    }
    Ok(row)
}

/// The contract parts on the view layer, in file order, each with its pivot: the centre of its
/// `pivot` object, else the contract's default worked out from the part's bounds.
fn read_parts<'a>(t: &Document, layer: &'a Node) -> Vec<Part<'a>> {
    let mut out: Vec<Part<'a>> = Vec::new();
    let mut pivots: HashMap<&str, [f64; 2]> = HashMap::new();
    let groups: Vec<&Node> = layer.children().into_iter().flatten().map(|n| n.as_ref()).collect();
    fn named(n: &Node) -> Option<&str> {
        n.name.as_deref().map(str::trim).filter(|n| part_spec(n).is_some() || n.starts_with("x_"))
    }
    // Pivots are worked out in contract order so "nearest the parent's pivot" has a parent to use.
    let mut by_rank: Vec<&Node> = groups.iter().copied().filter(|n| named(n).is_some()).collect();
    by_rank.sort_by_key(|n| n.name.as_deref().and_then(|x| PARTS.iter().position(|p| p.name == x.trim())).unwrap_or(PARTS.len()));
    for g in by_rank {
        let Some(name) = named(g) else { continue };
        let marker = g.children().into_iter().flatten().find(|k| k.name.as_deref() == Some("pivot"));
        let parent_pivot = part_spec(name).and_then(|s| s.parent).and_then(|p| pivots.get(p).copied());
        let pivot = match marker.and_then(|m| t.bounds_of(&[m.id], false)) {
            Some(r) => [r.center().x, r.center().y],
            None => default_pivot(name, t.bounds_of(&[g.id], false), parent_pivot),
        };
        pivots.entry(name).or_insert(pivot);
    }
    for g in groups {
        let Some(name) = named(g) else { continue };
        // A part found twice keeps its first group (character.check warns about the rest).
        if out.iter().any(|p| p.name == name) {
            continue;
        }
        let pivot = pivots.get(name).copied().unwrap_or([0.0, 0.0]);
        out.push(Part { name, node: g, pivot });
    }
    out
}

/// The contract's default pivot for `name` on `bounds` (section 2 of the contract).
fn default_pivot(name: &str, bounds: Option<Rect>, parent: Option<[f64; 2]>) -> [f64; 2] {
    let Some(r) = bounds else { return parent.unwrap_or([0.0, 0.0]) };
    let c = r.center();
    let rule = part_spec(name).map(|s| s.default_pivot).unwrap_or("centre");
    if rule.starts_with("bottom centre") {
        return [c.x, r.y1];
    }
    if rule.starts_with("top centre") {
        return [c.x, r.y0];
    }
    if rule.starts_with("top end") || rule.starts_with("edge") {
        // A part standing up pivots at its top end; one lying down (an arm held out, an ear) at the
        // end nearest its parent's pivot. Parents' pivots alone mislead: the torso's sits at the
        // waist, nearer the elbow than the shoulder.
        if r.height() > r.width() {
            return [c.x, r.y0];
        }
        let (a, b) = ([r.x0, c.y], [r.x1, c.y]);
        let Some(q) = parent else { return a };
        let d = |p: [f64; 2]| (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2);
        return if d(a) <= d(b) { a } else { b };
    }
    [c.x, c.y]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A contract file: every full part as a stacked box in each view, the arm down the left.
    fn contract_file(path: &std::path::Path) {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 800.0, "height": 800.0})).unwrap();
        for v in REQUIRED_VIEWS {
            let l = s.execute("layer.new", &json!({"name": v})).unwrap()["id"].as_u64().unwrap();
            s.execute("layer.setCurrent", &json!({"id": l})).unwrap();
            for spec in PARTS.iter().filter(|p| p.required_full) {
                let (x, y, w, h) = match spec.name {
                    "arm_upper_L" => (100.0, 100.0, 20.0, 100.0),
                    "arm_lower_L" => (100.0, 200.0, 20.0, 100.0),
                    "hand_L" => (100.0, 300.0, 20.0, 30.0),
                    "torso" => (130.0, 90.0, 100.0, 150.0),
                    _ => (300.0, 300.0, 20.0, 20.0),
                };
                let mut ids = vec![s.execute("shape.rectangle", &json!({"x": x, "y": y, "width": w, "height": h})).unwrap()["id"].as_u64().unwrap()];
                if spec.name == "torso" {
                    // A second shape drawn on top, to check stacking order survives.
                    ids.push(
                        s.execute("shape.ellipse", &json!({"x": 150.0, "y": 100.0, "width": 10.0, "height": 10.0})).unwrap()["id"].as_u64().unwrap(),
                    );
                }
                if spec.name == "hand_L" {
                    for alt in ["hand_L.open", "hand_L.fist"] {
                        let a = s.execute("shape.ellipse", &json!({"x": x, "y": y, "width": w, "height": h})).unwrap()["id"].as_u64().unwrap();
                        s.execute("object.setProps", &json!({"id": a, "name": alt})).unwrap();
                        ids.push(a);
                    }
                }
                s.execute("select.set", &json!({"ids": ids})).unwrap();
                let g = s.execute("object.group", &json!({})).unwrap()["id"].as_u64().unwrap();
                s.execute("object.setProps", &json!({"id": g, "name": spec.name})).unwrap();
            }
        }
        assert_eq!(s.execute("character.check", &json!({})).unwrap()["ok"], true);
        s.execute("document.save", &json!({"path": path})).unwrap();
    }

    #[test]
    fn upper_arm_turn_carries_forearm_and_hand() {
        let path = std::env::temp_dir().join(format!("rig-p2-{}.rsm", std::process::id()));
        contract_file(&path);
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 800.0, "height": 800.0})).unwrap();
        let r = s.execute("character.fromRsm", &json!({"path": path})).unwrap();
        let _ = std::fs::remove_file(&path);
        let peg = r["pegs"]["arm_upper_L"].as_u64().unwrap();
        s.execute("peg.key", &json!({"peg": peg, "frame": 1, "angle": 0.0})).unwrap();
        s.execute("peg.key", &json!({"peg": peg, "frame": 12, "angle": 30.0})).unwrap();
        let pegs = Pegs::of(&s.doc().unwrap().doc);
        let m = pegs.matrices(12);
        // The shoulder stays put; the elbow and the hand swing with the upper arm.
        let shoulder = pegs.pegs[peg as usize].pivot;
        assert!((shoulder[0] - 110.0).abs() < 1e-6 && (shoulder[1] - 100.0).abs() < 1e-6, "{shoulder:?}");
        for part in ["arm_lower_L", "hand_L"] {
            let i = r["pegs"][part].as_u64().unwrap() as usize;
            let pivot = pegs.pegs[i].pivot;
            let moved = m[i] * vectorcraft_geom::Point::new(pivot[0], pivot[1]);
            let (dx, dy) = (pivot[0] - shoulder[0], pivot[1] - shoulder[1]);
            let turned = moved - vectorcraft_geom::Point::new(shoulder[0], shoulder[1]);
            let angle = turned.y.atan2(turned.x) - dy.atan2(dx);
            assert!((angle.abs().to_degrees() - 30.0).abs() < 0.5, "{part} turned {}°", angle.to_degrees());
        }
        // The torso doesn't move.
        let torso = r["pegs"]["torso"].as_u64().unwrap() as usize;
        assert_eq!(m[torso], vectorcraft_geom::Affine::IDENTITY);
    }

    #[test]
    fn alternatives_become_drawings_with_the_default_exposed() {
        let path = std::env::temp_dir().join(format!("rig-p2-alt-{}.rsm", std::process::id()));
        contract_file(&path);
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let r = s.execute("character.fromRsm", &json!({"path": path, "view": "side"})).unwrap();
        let _ = std::fs::remove_file(&path);
        let torso = r["rows"]["torso"].as_u64().unwrap();
        let doc = &s.doc().unwrap().doc;
        let kids = doc.node(NodeId(torso)).unwrap().children().unwrap();
        let widths: Vec<f64> = kids.iter().map(|k| k.geometric_bounds().unwrap().width().round()).collect();
        assert_eq!(widths, vec![100.0, 10.0], "the shape drawn last stays on top");
        let row = r["rows"]["hand_L"].as_u64().unwrap();
        let d = s.execute("anim.drawings", &json!({"row": row})).unwrap();
        let names: Vec<&str> = d["drawings"].as_array().unwrap().iter().map(|x| x["name"].as_str().unwrap()).collect();
        assert_eq!(names.len(), 2, "{d}");
        assert!(names.contains(&"open") && names.contains(&"fist"));
        let open = d["drawings"].as_array().unwrap().iter().find(|x| x["name"] == "open").unwrap()["id"].clone();
        assert_eq!(d["current"], open);
    }

    #[test]
    fn one_walk_runs_on_two_rigged_characters() {
        let path = std::env::temp_dir().join(format!("rig-p3-{}.rsm", std::process::id()));
        contract_file(&path);
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 800.0, "height": 800.0})).unwrap();
        for name in ["ana", "bob"] {
            let r = s.execute("character.fromRsm", &json!({"path": path, "name": name})).unwrap();
            assert_eq!(r["character"], name);
        }
        let _ = std::fs::remove_file(&path);
        for name in ["ana", "bob"] {
            let r = s.execute("action.apply", &json!({"character": name, "action": "walk", "at": 1})).unwrap();
            assert_eq!(r["skipped"], json!([]), "{r}");
        }
        let pegs = Pegs::of(&s.doc().unwrap().doc);
        for name in ["ana", "bob"] {
            let arm = pegs.pegs.iter().find(|p| p.name == format!("{name}/arm_upper_L")).unwrap();
            assert_eq!(arm.keys.get(&6).map(|k| k.angle), Some(25.0), "{name}");
        }
        let video = std::env::temp_dir().join(format!("rig-p3-{}.webm", std::process::id()));
        let r = s.execute("anim.exportVideo", &json!({"path": video, "scale": 0.25})).unwrap();
        let _ = std::fs::remove_file(&video);
        assert!(r["frames"].as_u64().unwrap() >= 24, "{r}");
    }

    #[test]
    fn bad_input_is_an_error() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        assert!(s.execute("character.fromRsm", &json!({})).is_err());
        assert!(s.execute("character.fromRsm", &json!({"path": "/nonexistent/x.rsm"})).is_err());
        assert!(s.execute("character.fromRsm", &json!({"path": "/nonexistent/x.rsm", "view": "back"})).is_err());
        assert!(s.execute("character.fromRsm", &json!({"path": "/nonexistent/x.rsm", "name": "a/b"})).is_err());
    }
}
