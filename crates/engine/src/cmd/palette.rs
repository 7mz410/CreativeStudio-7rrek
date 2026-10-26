//! 7rrek (Creative Studio): palettes, as in Harmony. Art links its colours to the document's global
//! swatches by name (VectorCraft's model, which already recolours every linked paint when a swatch
//! changes). A palette here is a clone of those swatches: the same names with colours of its own
//! (a night version, a character's alternate look). The scene can show one palette, and a timeline
//! row its own, which wins over the scene's; the frame shown takes each linked colour from the
//! active palette, else from the swatch itself. Palettes live in the document's preserved extra
//! data (`"palettes"`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::json;
use vectorcraft_color::Color;
use vectorcraft_doc::Document;

use super::anim::{Anim, row_param, show};
use super::*;

const KEY: &str = "palettes";
/// Most palettes a document takes.
pub const MAX_PALETTES: usize = 256;

/// A palette: swatch name → its colour here. A swatch it leaves out shows its own colour.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Palette {
    pub name: String,
    #[serde(default)]
    pub colors: BTreeMap<String, Color>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Palettes {
    pub palettes: Vec<Palette>,
    /// The palette the whole scene shows, by index.
    pub scene: Option<usize>,
    /// Row id → the palette that row shows, over the scene's.
    pub rows: BTreeMap<u64, usize>,
}

impl Palettes {
    /// The document's palettes; none if they are broken (an index out of range).
    pub fn of(doc: &Document) -> Self {
        let p: Palettes = doc.unknown.get(KEY).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
        let n = p.palettes.len();
        let ok = n <= MAX_PALETTES && p.scene.is_none_or(|i| i < n) && p.rows.values().all(|i| *i < n);
        if ok { p } else { Palettes::default() }
    }

    pub fn store(&self, doc: &mut Document) {
        if self.palettes.is_empty() {
            doc.unknown.remove(KEY);
        } else if let Ok(v) = serde_json::to_value(self) {
            doc.unknown.insert(KEY.into(), v);
        }
    }
}

/// Colours every top-level row's linked paints from the palette it shows (its own, else the
/// scene's), and from the swatches themselves where none applies, so turning a palette off brings
/// the swatch colours back. Nothing happens in a document without palettes.
pub(crate) fn apply(d: &mut Document) {
    let p = Palettes::of(d);
    if p.palettes.is_empty() {
        return;
    }
    let own: BTreeMap<String, Color> = d.swatches_iter().filter_map(|w| Some((w.name.clone(), d.global_color(&w.name)?))).collect();
    let rows: Vec<NodeId> = d.layers.iter().map(|l| l.id).collect();
    for row in rows {
        let palette = p.rows.get(&row.0).copied().or(p.scene).and_then(|i| p.palettes.get(i));
        d.map_solid_paints_in(&[row], &mut |color, swatch, tint| {
            let Some(name) = swatch.as_deref() else { return false };
            let Some(base) = palette.and_then(|pl| pl.colors.get(name)).or_else(|| own.get(name)) else { return false };
            let new = base.tinted(*tint);
            let changed = new != *color;
            *color = new;
            changed
        });
    }
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "palette.list",
            "Palettes",
            [],
            None,
            "{} the palettes → {palettes: [{index, name, colors: {swatch: colour}}], scene: index or null, rows: {layer id: index}, swatches: [the global swatch names a palette can recolour]}",
            has_doc,
            list
        ),
        cmd!(
            "palette.clone",
            "Clone Palette",
            [],
            None,
            "{name?: text (default: \"Palette N\"), from?: palette index (default: the swatches themselves)} a new palette with the same swatches and colours → {index}",
            has_doc,
            clone
        ),
        cmd!(
            "palette.setColor",
            "Palette Colour",
            [],
            None,
            "{palette: index, swatch: a global swatch's name, color: as paint.setFill's color (null shows the swatch's own)} → {palette, swatch}",
            has_doc,
            set_color
        ),
        cmd!(
            "palette.use",
            "Use Palette",
            [],
            None,
            "{palette: index (null for none), scene?: true (the whole scene), row?: layer id when not the scene (default: the current layer's row)} what the scene or a row shows; a row's palette wins over the scene's → {palette}",
            has_doc,
            use_palette
        ),
        cmd!(
            "palette.remove",
            "Remove Palette",
            [],
            None,
            "{palette: index} removes it; the scene and rows that showed it go back to the swatches → {removed}",
            has_doc,
            remove
        ),
    ]
}

fn index_param(p: &Value, n: usize, cmd: &str) -> Result<usize> {
    p.get("palette")
        .and_then(Value::as_u64)
        .map(|i| i as usize)
        .filter(|i| *i < n)
        .ok_or_else(|| bad(cmd, format!("`palette` must be a palette index from 0 to {}", n.saturating_sub(1))))
}

/// Stores `p` as one undo step and shows the current frame again in its colours.
fn commit(s: &mut Session, label: &str, p: Palettes) -> Result<()> {
    s.edit(label, |d, _| {
        p.store(d);
        Ok(())
    })?;
    let current = Anim::of(&s.doc()?.doc).current;
    show(s, current)?;
    Ok(())
}

fn list(s: &mut Session, _: &Value) -> Result<Value> {
    let d = &s.doc()?.doc;
    let p = Palettes::of(d);
    let palettes: Vec<Value> = p.palettes.iter().enumerate().map(|(i, pl)| json!({"index": i, "name": pl.name, "colors": pl.colors})).collect();
    let swatches: Vec<&str> = d.swatches_iter().filter(|w| d.global_color(&w.name).is_some()).map(|w| w.name.as_str()).collect();
    Ok(json!({"palettes": palettes, "scene": p.scene, "rows": p.rows, "swatches": swatches}))
}

fn clone(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "palette.clone";
    let d = &s.doc()?.doc;
    let mut all = Palettes::of(d);
    if all.palettes.len() >= MAX_PALETTES {
        return Err(bad(C, format!("a document takes at most {MAX_PALETTES} palettes")));
    }
    let colors = match p.get("from").filter(|v| !v.is_null()) {
        Some(_) => {
            let i = index_param(&json!({"palette": p["from"]}), all.palettes.len(), C)?;
            all.palettes.get(i).map(|pl| pl.colors.clone()).unwrap_or_default()
        }
        None => d.swatches_iter().filter_map(|w| Some((w.name.clone(), d.global_color(&w.name)?))).collect(),
    };
    let name = p
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("Palette {}", all.palettes.len() + 1));
    all.palettes.push(Palette { name, colors });
    let index = all.palettes.len() - 1;
    commit(s, "Clone Palette", all)?;
    Ok(json!({"index": index}))
}

fn set_color(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "palette.setColor";
    let d = &s.doc()?.doc;
    let mut all = Palettes::of(d);
    let i = index_param(p, all.palettes.len(), C)?;
    let swatch = p
        .get("swatch")
        .and_then(Value::as_str)
        .filter(|n| d.global_color(n).is_some())
        .ok_or_else(|| bad(C, "`swatch` must name a global swatch"))?
        .to_string();
    let color = match p.get("color") {
        None => return Err(bad(C, "`color` is required (null shows the swatch's own)")),
        Some(Value::Null) => None,
        Some(v) => Some(super::paint::new_color(s, p, color_value(v).ok_or_else(|| bad(C, "`color` is not a colour"))?)),
    };
    let Some(pl) = all.palettes.get_mut(i) else { return Err(bad(C, "no such palette")) };
    match color {
        Some(c) => pl.colors.insert(swatch.clone(), c),
        None => pl.colors.remove(&swatch),
    };
    commit(s, "Palette Colour", all)?;
    Ok(json!({"palette": i, "swatch": swatch}))
}

fn use_palette(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "palette.use";
    let mut all = Palettes::of(&s.doc()?.doc);
    let palette = match p.get("palette") {
        Some(Value::Null) => None,
        Some(_) => Some(index_param(p, all.palettes.len(), C)?),
        None => return Err(bad(C, "`palette` is required (null for none)")),
    };
    if p.get("scene").and_then(Value::as_bool) == Some(true) {
        all.scene = palette;
    } else {
        let row = row_param(s, p, C)?;
        match palette {
            Some(i) => all.rows.insert(row.0, i),
            None => all.rows.remove(&row.0),
        };
    }
    commit(s, "Use Palette", all)?;
    Ok(json!({"palette": palette}))
}

fn remove(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "palette.remove";
    let mut all = Palettes::of(&s.doc()?.doc);
    let i = index_param(p, all.palettes.len(), C)?;
    all.palettes.remove(i);
    // Later palettes move down one; what showed the removed one shows the swatches.
    let shift = |j: usize| (j != i).then(|| if j > i { j - 1 } else { j });
    all.scene = all.scene.and_then(shift);
    all.rows = all.rows.iter().filter_map(|(r, j)| Some((*r, shift(*j)?))).collect();
    s.edit("Remove Palette", |d, _| {
        all.store(d);
        // With none left, frames stop recolouring: the art takes its swatches' colours back here.
        if all.palettes.is_empty() {
            restore(d);
        }
        Ok(())
    })?;
    let current = Anim::of(&s.doc()?.doc).current;
    show(s, current)?;
    Ok(json!({"removed": i}))
}

/// Every linked paint back to its swatch's own colour (after the last palette goes).
fn restore(d: &mut Document) {
    let own: BTreeMap<String, Color> = d.swatches_iter().filter_map(|w| Some((w.name.clone(), d.global_color(&w.name)?))).collect();
    d.map_solid_paints(&mut |color, swatch, tint| {
        let Some(base) = swatch.as_deref().and_then(|n| own.get(n)) else { return false };
        let new = base.tinted(*tint);
        let changed = new != *color;
        *color = new;
        changed
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three drawings (frames 1–3), each a square filled with global swatch "Skin" (#ffcc99),
    /// and their square ids.
    fn scene() -> (Session, Vec<u64>) {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s.execute("swatch.new", &json!({"name": "Skin", "color": "#ffcc99", "global": true})).unwrap();
        let ids = (1..=3u32)
            .map(|f| {
                s.execute("anim.blankKeyframe", &json!({"frame": f})).unwrap();
                let id = s.execute("shape.rectangle", &json!({"x": 10 * f, "y": 10, "width": 20, "height": 20})).unwrap()["id"].as_u64().unwrap();
                s.execute("paint.setFill", &json!({"swatch": "Skin"})).unwrap();
                id
            })
            .collect();
        (s, ids)
    }

    /// The fill of `id` as shown at `frame`, as hex.
    fn hex(s: &Session, id: u64, frame: u32) -> String {
        let d = crate::cmd::anim::at_frame(&s.doc().unwrap().doc, frame);
        d.node(NodeId(id)).unwrap().appearance.fill_paint().color().unwrap().to_hex()
    }

    #[test]
    fn a_swatch_edit_recolours_every_drawing_on_every_frame() {
        let (mut s, ids) = scene();
        s.execute("swatch.edit", &json!({"name": "Skin", "color": "#996633"})).unwrap();
        for (f, id) in ids.iter().enumerate() {
            assert_eq!(hex(&s, *id, f as u32 + 1).to_lowercase(), "#996633", "frame {}", f + 1);
        }
    }

    #[test]
    fn a_scene_palette_recolours_and_a_row_palette_wins() {
        let (mut s, ids) = scene();
        let night = s.execute("palette.clone", &json!({"name": "Night"})).unwrap()["index"].as_u64().unwrap();
        s.execute("palette.setColor", &json!({"palette": night, "swatch": "Skin", "color": "#334466"})).unwrap();
        s.execute("palette.use", &json!({"palette": night, "scene": true})).unwrap();
        for (f, id) in ids.iter().enumerate() {
            assert_eq!(hex(&s, *id, f as u32 + 1).to_lowercase(), "#334466", "night on frame {}", f + 1);
        }
        let row = s.doc().unwrap().doc.layers[0].id.0;
        let red = s.execute("palette.clone", &json!({"name": "Red", "from": night})).unwrap()["index"].as_u64().unwrap();
        s.execute("palette.setColor", &json!({"palette": red, "swatch": "Skin", "color": "#ff0000"})).unwrap();
        s.execute("palette.use", &json!({"palette": red, "row": row})).unwrap();
        assert_eq!(hex(&s, ids[1], 2).to_lowercase(), "#ff0000", "the row's palette wins");
        // The swatch itself never changed, and turning palettes off shows it again.
        assert_eq!(s.doc().unwrap().doc.global_color("Skin").unwrap().to_hex().to_lowercase(), "#ffcc99");
        s.execute("palette.use", &json!({"palette": null, "row": row})).unwrap();
        s.execute("palette.use", &json!({"palette": null, "scene": true})).unwrap();
        assert_eq!(hex(&s, ids[2], 3).to_lowercase(), "#ffcc99");
        // Saved and loaded, the palettes stay.
        let back = vectorcraft_format::load(&vectorcraft_format::save(&s.doc().unwrap().doc, false)).unwrap();
        assert_eq!(Palettes::of(&back).palettes.len(), 2);
    }

    #[test]
    fn removing_palettes_renumbers_and_the_last_one_gives_the_colours_back() {
        let (mut s, ids) = scene();
        let a = s.execute("palette.clone", &json!({})).unwrap()["index"].as_u64().unwrap();
        let b = s.execute("palette.clone", &json!({})).unwrap()["index"].as_u64().unwrap();
        s.execute("palette.setColor", &json!({"palette": b, "swatch": "Skin", "color": "#00ff00"})).unwrap();
        s.execute("palette.use", &json!({"palette": b, "scene": true})).unwrap();
        s.execute("palette.remove", &json!({"palette": a})).unwrap();
        assert_eq!(Palettes::of(&s.doc().unwrap().doc).scene, Some(0), "b moved down to 0");
        assert_eq!(hex(&s, ids[0], 1).to_lowercase(), "#00ff00");
        s.execute("palette.remove", &json!({"palette": 0})).unwrap();
        let d = &s.doc().unwrap().doc;
        assert_eq!(d.node(NodeId(ids[0])).unwrap().appearance.fill_paint().color().unwrap().to_hex().to_lowercase(), "#ffcc99");
        assert!(s.execute("palette.setColor", &json!({"palette": 0, "swatch": "Skin", "color": "#000000"})).is_err());
        assert!(s.execute("palette.clone", &json!({"from": 7})).is_err());
    }
}
