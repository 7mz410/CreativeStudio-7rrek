//! 7rrek (Creative Studio): character templates, like Harmony's master templates. A character is
//! one timeline row (a top-level layer) with everything that animates it: its drawings and keys,
//! tweens, exposures and mouth chart, its skeleton, the pegs that carry it, and the meshes on its
//! art. A template is a native document holding just that row and that data; dropping it into a
//! document gives every node new ids and renames them wherever the data names them.

use std::collections::{BTreeMap, HashMap};

use serde_json::json;
use vectorcraft_doc::{Document, Node, NodeId, NodeKind};

use super::anim::{Anim, row_param, show, unposed};
use super::mesh::Meshes;
use super::palette::Palettes;
use super::peg::{MAX_PEGS, Pegs};
use super::rig::Rig;
use super::*;

/// The extension character templates are saved with (a 7rrek document inside).
pub const EXTENSION: &str = "7rrekchar";

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "character.save",
            "Save Character",
            [],
            None,
            "{row?: layer id (default: the current layer's row), path: file (.7rrekchar)} saves the row with its drawings, keys, skeleton, pegs and meshes as a character template → {path, bytes}",
            has_doc,
            save
        ),
        cmd!(
            "character.load",
            "Load Character",
            [],
            None,
            "{path: a .7rrekchar template} adds its character as a new layer on top named after the file, with new ids, its animation merged in → {layer}",
            has_doc,
            load
        ),
    ]
}

/// Row `row`'s template as file bytes, None if it is not a top-level layer.
pub fn template_bytes(doc: &Document, row: NodeId) -> Option<Vec<u8>> {
    extract(doc, row).map(|t| vectorcraft_format::save(&t, false))
}

fn save(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "character.save";
    let path = str_param(p, "path").filter(|x| !x.is_empty()).ok_or_else(|| bad(C, "path is required"))?.to_string();
    let row = row_param(s, p, C)?;
    let bytes = template_bytes(&s.doc()?.doc, row).ok_or_else(|| bad(C, "no such layer"))?;
    super::fileio::write_file(&path, &bytes)?;
    Ok(json!({"path": path, "bytes": bytes.len()}))
}

fn load(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "character.load";
    let path = str_param(p, "path").filter(|x| !x.is_empty()).ok_or_else(|| bad(C, "path is required"))?.to_string();
    let bytes = super::fileio::read_file(&path)?;
    let t = vectorcraft_format::load(&bytes).map_err(|e| EngineError::Other(format!("{path}: not a character template ({e})")))?;
    // The layer takes the template's file name ("hero.7rrekchar" → "hero"), told apart from the
    // document's own layers.
    let name = std::path::Path::new(&path).file_stem().and_then(|n| n.to_str()).filter(|n| !n.trim().is_empty()).map(str::to_string);
    let layer = s.edit("Load Character", |d, _| {
        let layer = insert(d, &t).ok_or_else(|| bad(C, "the template holds no layer, or the document has too many pegs for it"))?;
        if let (Some(name), Some(n)) = (name, d.node_mut(layer)) {
            n.name = Some(name);
        }
        Ok(layer)
    })?;
    let current = Anim::of(&s.doc()?.doc).current;
    show(s, current)?;
    Ok(json!({"layer": layer.0}))
}

/// Every node id in `n`'s tree, `n` included.
fn ids_in(n: &Node) -> Vec<u64> {
    let mut v = Vec::new();
    n.walk(&mut |c| v.push(c.id.0));
    v
}

/// The template of top-level layer `row`: the document cut down to that layer, with the frame
/// shown undone, and only the animation data of the row and its nodes. None if `row` is not a
/// top-level layer.
pub fn extract(doc: &Document, row: NodeId) -> Option<Document> {
    let mut t = unposed(doc);
    t.layers.retain(|l| l.id == row);
    let layer = t.layers.first()?;
    let ids = ids_in(layer);
    let has = |id: &u64| ids.contains(id);

    let a = Anim::of(doc);
    let mut keep = Anim { fps: a.fps, frames: a.frames, current: a.current, ..Anim::default() };
    keep.keys = a.keys.into_iter().filter(|(id, _)| has(id)).collect();
    keep.tweens = a.tweens.into_iter().filter(|id| has(id)).collect();
    keep.eases = a.eases.into_iter().filter(|(id, _)| has(id)).collect();
    keep.exposures = a.exposures.into_iter().filter(|(r, _)| *r == row.0).collect();
    keep.mouths = a.mouths.into_iter().filter(|(r, _)| *r == row.0).collect();

    let mut rig = Rig::of(doc);
    rig.skeletons.retain(|r, _| *r == row.0);

    // The pegs carrying the row and the pegs they hang from, in order, renumbered.
    let all = Pegs::of(doc);
    let mut wanted = vec![false; all.pegs.len()];
    let mut at = all.pegs.iter().position(|p| p.rows.contains(&row.0));
    while let Some(i) = at {
        let Some(w) = wanted.get_mut(i) else { break };
        *w = true;
        at = all.pegs.get(i).and_then(|p| p.parent);
    }
    let renumber: HashMap<usize, usize> =
        (0..all.pegs.len()).filter(|i| wanted.get(*i) == Some(&true)).enumerate().map(|(new, old)| (old, new)).collect();
    let pegs = Pegs {
        pegs: all
            .pegs
            .iter()
            .enumerate()
            .filter(|(i, _)| renumber.contains_key(i))
            .map(|(_, p)| {
                let mut p = p.clone();
                p.parent = p.parent.and_then(|q| renumber.get(&q).copied());
                p.rows.retain(|r| *r == row.0);
                p
            })
            .collect(),
    };

    let mut meshes = Meshes::of(doc);
    meshes.meshes.retain(|m| has(&m.node));

    // The palette the row shows (its own, else the scene's), as the template's only one, held by
    // the row. Its global swatches stay in the template (the whole swatch list comes along).
    let all = Palettes::of(doc);
    let shown = all.rows.get(&row.0).copied().or(all.scene).and_then(|i| all.palettes.get(i)).cloned();
    let palettes = Palettes { rows: shown.iter().map(|_| (row.0, 0)).collect(), palettes: shown.into_iter().collect(), scene: None };

    t.unknown.clear();
    keep.store(&mut t);
    palettes.store(&mut t);
    rig.store(&mut t);
    pegs.store(&mut t);
    meshes.store(&mut t);
    Some(t)
}

/// Pairs each id in `old`'s tree with the id at the same place in `new`'s (a [`Document::reid`]
/// copy has the same shape).
pub(crate) fn pair_ids(old: &Node, new: &Node, map: &mut HashMap<u64, u64>) {
    map.insert(old.id.0, new.id.0);
    if let (Some(a), Some(b)) = (old.children(), new.children()) {
        for (o, n) in a.iter().zip(b) {
            pair_ids(o, n, map);
        }
    }
}

/// Points each image under `n` at pixels in `doc`: the template's blob under its key when `doc`
/// has no such key or holds the same bytes there, else under the blob's content key.
fn bring_images(n: &mut Node, t: &Document, doc: &mut Document) {
    if let NodeKind::Image(im) = &mut n.kind
        && let Some(blob) = t.images.get(&im.key)
    {
        match doc.images.get(&im.key) {
            Some(b) if b.bytes != blob.bytes => {
                let key = blob.content_key();
                doc.images.entry(key.clone()).or_insert_with(|| blob.clone());
                im.key = key;
            }
            Some(_) => {}
            None => {
                doc.images.insert(im.key.clone(), blob.clone());
            }
        }
    }
    if let Some(children) = n.children_mut() {
        for c in children {
            bring_images(std::sync::Arc::make_mut(c), t, doc);
        }
    }
}

/// Drops template `t` into `doc` as a new top-level layer above the others, with new ids, its
/// animation data merged in under them. Returns the new layer. None if the template holds no
/// layer or `doc` would pass its peg limit (nothing changes then).
pub fn insert(doc: &mut Document, t: &Document) -> Option<NodeId> {
    let layer = t.layers.first()?;
    let mut pegs = Pegs::of(doc);
    let t_pegs = Pegs::of(t);
    if pegs.pegs.len() + t_pegs.pegs.len() > MAX_PEGS {
        return None;
    }
    let mut copy = doc.reid(layer);
    let mut map = HashMap::new();
    pair_ids(layer, &copy, &mut map);
    bring_images(&mut copy, t, doc);
    let row = copy.id;
    let top = doc.layers.len();
    doc.insert(None, top, copy).ok()?;
    let id = |old: &u64| map.get(old).copied();

    let ta = Anim::of(t);
    let mut a = Anim::of(doc);
    a.keys.extend(ta.keys.iter().filter_map(|(k, f)| Some((id(k)?, *f))));
    a.tweens.extend(ta.tweens.iter().filter_map(id));
    a.eases.extend(ta.eases.iter().filter_map(|(k, e)| Some((id(k)?, *e))));
    for (r, e) in &ta.exposures {
        let Some(r) = id(r) else { continue };
        a.exposures.insert(r, e.iter().filter_map(|(f, d)| Some((*f, id(d)?))).collect());
    }
    for (r, c) in &ta.mouths {
        let Some(r) = id(r) else { continue };
        a.mouths.insert(r, c.iter().filter_map(|(m, d)| Some((*m, id(d)?))).collect::<BTreeMap<_, _>>());
    }
    a.store(doc);

    let mut rig = Rig::of(doc);
    for (r, sk) in Rig::of(t).skeletons {
        if let Some(r) = id(&r) {
            rig.skeletons.insert(r, sk);
        }
    }
    rig.store(doc);

    let base = pegs.pegs.len();
    for mut p in t_pegs.pegs {
        p.parent = p.parent.map(|q| q + base);
        p.rows = p.rows.iter().filter_map(id).collect();
        pegs.pegs.push(p);
    }
    pegs.store(doc);

    // Swatches the character's art links to that the document lacks, by name; a name the document
    // has keeps the document's colour.
    let missing: Vec<_> = t.swatches_iter().filter(|w| doc.swatch(&w.name).is_none()).cloned().collect();
    doc.swatches.extend(missing);
    let mut palettes = Palettes::of(doc);
    let tp = Palettes::of(t);
    let base = palettes.palettes.len();
    if base + tp.palettes.len() <= super::palette::MAX_PALETTES {
        for (r, i) in &tp.rows {
            if let Some(r) = id(r) {
                palettes.rows.insert(r, i + base);
            }
        }
        palettes.palettes.extend(tp.palettes);
        palettes.store(doc);
    }

    let mut meshes = Meshes::of(doc);
    for mut m in Meshes::of(t).meshes {
        let Some(n) = id(&m.node) else { continue };
        // A node meshed twice keeps the mesh it had.
        if meshes.of_node(n).is_none() {
            m.node = n;
            meshes.meshes.push(m);
        }
    }
    meshes.store(doc);
    Some(row)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::Session;

    /// A row with drawings at frames 1 and 5 (a tween between), an exposure and a mouth on frame
    /// 9, a two-joint skeleton, two nested pegs (the inner carrying it) keyed, and a mesh; its id
    /// and the session.
    fn character() -> (Session, u64) {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        let rect = s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 100, "height": 50})).unwrap()["id"].as_u64().unwrap();
        s.execute("anim.tween", &json!({})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 5})).unwrap();
        let k1 = Anim::of(&s.doc().unwrap().doc).keys.iter().find(|(_, f)| **f == 1).map(|(k, _)| *k).unwrap();
        s.execute("anim.expose", &json!({"frame": 9, "drawing": k1})).unwrap();
        s.execute("anim.mouthChart", &json!({"shape": "A", "drawing": k1})).unwrap();
        s.execute("rig.addJoint", &json!({"x": 0, "y": 25})).unwrap();
        s.execute("rig.addJoint", &json!({"x": 100, "y": 25, "parent": 0})).unwrap();
        s.execute("peg.add", &json!({"rows": []})).unwrap();
        s.execute("peg.add", &json!({"parent": 0})).unwrap();
        s.execute("peg.key", &json!({"frame": 1, "x": 0})).unwrap();
        s.execute("peg.key", &json!({"frame": 5, "x": 30})).unwrap();
        s.execute("mesh.add", &json!({"node": rect})).unwrap();
        s.execute("mesh.pin", &json!({"node": rect, "vertex": 0})).unwrap();
        s.execute("swatch.new", &json!({"name": "Hero Skin", "color": "#ffcc99", "global": true})).unwrap();
        s.execute("paint.setFill", &json!({"ids": [rect], "swatch": "Hero Skin"})).unwrap();
        let night = s.execute("palette.clone", &json!({"name": "Hero Night"})).unwrap()["index"].as_u64().unwrap();
        s.execute("palette.setColor", &json!({"palette": night, "swatch": "Hero Skin", "color": "#223344"})).unwrap();
        let row = s.doc().unwrap().doc.layers[0].id.0;
        s.execute("palette.use", &json!({"palette": night, "row": row})).unwrap();
        let row = s.doc().unwrap().doc.layers[0].id.0;
        (s, row)
    }

    #[test]
    fn a_template_carries_the_rows_animation_and_nothing_else() {
        let (mut s, row) = character();
        // Showing a bent frame first: the template must not keep its copies.
        s.execute("anim.goto", &json!({"frame": 5})).unwrap();
        let doc = s.doc().unwrap().doc.clone();
        let t = extract(&doc, NodeId(row)).unwrap();
        let t = vectorcraft_format::load(&vectorcraft_format::save(&t, false)).unwrap();
        assert_eq!(t.layers.len(), 1);
        let a = Anim::of(&t);
        assert!(a.ghosts.is_empty() && a.posed.is_empty() && a.sounds.is_empty());
        assert_eq!((a.keys.len(), a.tweens.len(), a.exposures.len(), a.mouths.len()), (2, 1, 1, 1));
        let ids = ids_in(&t.layers[0]);
        assert!(a.keys.keys().all(|k| ids.contains(k)), "only the row's own nodes");
        assert_eq!(Rig::of(&t).skeletons.keys().copied().collect::<Vec<_>>(), vec![row]);
        let pegs = Pegs::of(&t).pegs;
        assert_eq!(pegs.len(), 2, "the carrying peg and its parent");
        assert_eq!((pegs[1].parent, pegs[1].rows.clone()), (Some(0), vec![row]));
        assert_eq!(Meshes::of(&t).meshes.len(), 1);
        assert!(extract(&doc, NodeId(9999)).is_none());
    }

    #[test]
    fn a_template_drops_into_another_file_with_new_ids() {
        let (s, row) = character();
        let t = extract(&s.doc().unwrap().doc, NodeId(row)).unwrap();
        // The other file already has a layer and a peg of its own.
        let (other, _) = character();
        let mut d = (*other.doc().unwrap().doc).clone();
        let before = d.layers.len();
        let theirs: Vec<u64> = d.layers.iter().flat_map(|l| ids_in(l)).collect();
        let new_row = insert(&mut d, &t).unwrap();
        assert_eq!(d.layers.len(), before + 1);
        let layer = d.node(new_row).unwrap();
        let ids = ids_in(layer);
        assert!(!ids.iter().any(|i| theirs.contains(i)), "new ids, none clashing with the file's own");
        let a = Anim::of(&d);
        assert_eq!(a.keys.keys().filter(|k| ids.contains(k)).count(), 2);
        assert_eq!(a.tweens.iter().filter(|k| ids.contains(k)).count(), 1);
        let drawing = a.exposures[&new_row.0][&9];
        assert!(ids.contains(&drawing), "the exposure names the new drawing");
        assert_eq!(a.mouths[&new_row.0][&'A'], drawing);
        assert!(Rig::of(&d).skeletons.contains_key(&new_row.0));
        let pegs = Pegs::of(&d).pegs;
        assert_eq!(pegs.len(), 4, "two pegs of its own plus two brought");
        assert_eq!((pegs[3].parent, pegs[3].rows.clone()), (Some(2), vec![new_row.0]));
        let m = Meshes::of(&d);
        assert_eq!(m.meshes.len(), 2);
        // Its palette comes along and the new row shows it.
        let pals = Palettes::of(&d);
        assert_eq!(pals.palettes.len(), 2, "the file's own palette plus the character's");
        assert_eq!(pals.rows.get(&new_row.0), Some(&1));
        assert!(ids.contains(&m.meshes[1].node));
        // The dropped character animates: its peg moves it 30 pt by frame 5.
        let x = |d: &Document| d.bounds_of(&[new_row], false).unwrap().min_x();
        let f1 = crate::cmd::anim::at_frame(&d, 1);
        let f5 = crate::cmd::anim::at_frame(&d, 5);
        assert!((x(&f5) - x(&f1) - 30.0).abs() < 1.0, "{} vs {}", x(&f1), x(&f5));
    }

    #[test]
    fn save_and_load_commands_carry_a_character_between_files() {
        let (mut s, row) = character();
        let path = std::env::temp_dir().join(format!("hero-{}.{EXTENSION}", std::process::id()));
        let path = path.to_str().unwrap();
        s.execute("character.save", &json!({"row": row, "path": path})).unwrap();
        let mut other = Session::new();
        other.execute("file.new", &json!({})).unwrap();
        let layer = other.execute("character.load", &json!({"path": path})).unwrap()["layer"].as_u64().unwrap();
        let _ = std::fs::remove_file(path);
        let d = &other.doc().unwrap().doc;
        assert!(Rig::of(d).skeletons.contains_key(&layer), "its bones");
        assert!(d.global_color("Hero Skin").is_some(), "the swatch its art links to comes along");
        assert_eq!(Palettes::of(d).rows.get(&layer), Some(&0), "and the palette it showed");
        assert_eq!(d.node(NodeId(layer)).unwrap().name, Some(format!("hero-{}", std::process::id())), "named after the file");
        assert_eq!(other.execute("anim.drawings", &json!({"row": layer})).unwrap()["drawings"].as_array().unwrap().len(), 2, "its drawings");
        other.execute("edit.undo", &json!({})).unwrap();
        assert!(other.doc().unwrap().doc.node(NodeId(layer)).is_none(), "one undo step");
        assert!(other.execute("character.load", &json!({"path": "/no/such/file.7rrekchar"})).is_err());
    }
}
