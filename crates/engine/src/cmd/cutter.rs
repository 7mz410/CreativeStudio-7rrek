//! 7rrek (Creative Studio): cutters, as in Harmony. A cutter cuts one timeline row (its input) by
//! the shape of another (its matte): by default the input is hidden where the matte covers it, and
//! inverted only that overlap shows. The matte's art is not drawn; only its shape counts (its
//! colours don't, its transparency cuts part way). The frame shown builds each cut as an opacity
//! mask on the input row (the matte's art at that frame turned white), and hides the matte row;
//! the next frame shown takes both back (`Anim.cut`, `Anim.mattes`). Cutters live in the
//! document's preserved extra data (`"cutters"`).

use serde::{Deserialize, Serialize};
use serde_json::json;
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::recolor::{ColorVisitor, Reach, recolor_node};
use vectorcraft_doc::{Appearance, AppearanceItem, Document, FillLayer, Node, OpacityMask};

use super::anim::{Anim, row_param, show};
use super::*;

const KEY: &str = "cutters";
/// Most cutters a document takes.
pub const MAX_CUTTERS: usize = 256;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Cutter {
    /// The row that is cut.
    pub row: u64,
    /// The row whose shape cuts it.
    pub matte: u64,
    /// Keep only where the matte covers the row (instead of hiding it there).
    #[serde(default)]
    pub inverted: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Cutters {
    pub cutters: Vec<Cutter>,
}

impl Cutters {
    pub fn of(doc: &Document) -> Self {
        let c: Cutters = doc.unknown.get(KEY).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
        if c.cutters.len() <= MAX_CUTTERS { c } else { Cutters::default() }
    }

    pub fn store(&self, doc: &mut Document) {
        if self.cutters.is_empty() {
            doc.unknown.remove(KEY);
        } else if let Ok(v) = serde_json::to_value(self) {
            doc.unknown.insert(KEY.into(), v);
        }
    }
}

/// Turns every colour one colour: a matte cuts by its shape, not its colours.
struct Flat(Color);

impl ColorVisitor for Flat {
    fn paint(&mut self, p: &mut Paint) -> bool {
        let c = self.0;
        vectorcraft_doc::recolor::map_paint(p, &|_| c)
    }
    fn color(&mut self, _: Color) -> Color {
        self.0
    }
    // shortcut: an image in a matte cuts by its own brightness, not its outline; turn its pixels
    // white (keeping alpha) when image mattes matter.
    fn image(&mut self, _: &str) -> Option<String> {
        None
    }
}

/// Builds every cutter for the frame shown in `d`: the input row gets an opacity mask of its
/// matte's visible art in white, and the matte row is hidden. A row that already has an opacity
/// mask of its own keeps it (and isn't cut). Returns (rows masked, matte rows hidden), for the next
/// frame shown to take back.
pub(crate) fn apply(d: &mut Document) -> (Vec<u64>, Vec<u64>) {
    let (mut cut, mut mattes) = (Vec::new(), Vec::new());
    for c in Cutters::of(d).cutters {
        let (row, matte) = (NodeId(c.row), NodeId(c.matte));
        if row == matte || !d.layers.iter().any(|l| l.id == row) || d.node(row).is_some_and(|n| n.extras().mask.is_some()) {
            continue;
        }
        let Some(m) = d.layers.iter().find(|l| l.id == matte) else { continue };
        let shown = m.children().into_iter().flatten().filter(|c| c.visible).cloned().collect();
        let mut matte_art = Node::group(NodeId(0), shown);
        // The mask shows where it is white: inverted, the matte itself in white (all else hidden);
        // by default, white over the whole row with the matte in black on it. A see-through matte
        // comes out grey and cuts part way.
        let art = if c.inverted {
            recolor_node(&mut matte_art, Reach::ALL, &mut Flat(Color::WHITE));
            matte_art
        } else {
            recolor_node(&mut matte_art, Reach::ALL, &mut Flat(Color::BLACK));
            let Some(r) = d.bounds_of(&[row], true) else { continue };
            let margin = 0.1 * r.width().max(r.height()) + 10.0;
            let fill = Appearance { items: vec![AppearanceItem::Fill(FillLayer::new(Paint::solid(Color::WHITE)))], ..Appearance::default() };
            let backdrop = Node::path(NodeId(0), vectorcraft_geom::shapes::rectangle(r.inflate(margin, margin)), fill);
            Node::group(NodeId(0), vec![std::sync::Arc::new(backdrop), std::sync::Arc::new(matte_art)])
        };
        let mask = OpacityMask::new(art, true);
        if let Some(n) = d.node_mut(row) {
            n.extras_mut().mask = Some(Box::new(mask));
            cut.push(row.0);
        }
        if let Some(n) = d.node_mut(matte)
            && n.visible
        {
            n.visible = false;
            mattes.push(matte.0);
        }
    }
    (cut, mattes)
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "cutter.add",
            "Add Cutter",
            [],
            None,
            "{row?: layer id to cut (default: the current layer's row), matte: layer id whose shape cuts it, inverted?: bool (keep only the overlap; default false: hide the overlap)} a row has one cutter, so this replaces any it had; the matte row's art stops showing → {row, matte, inverted}",
            has_doc,
            add
        ),
        cmd!(
            "cutter.remove",
            "Remove Cutter",
            [],
            None,
            "{row?: layer id (default: the current layer's row)} the row shows whole again, and its matte's art shows again → {row}",
            has_doc,
            remove
        ),
        cmd!(query "cutter.list", "Cutters", [], None, "{} → {cutters: [{row, matte, inverted}]}", has_doc, list),
    ]
}

fn top_row(d: &Document, v: &Value, cmd: &str, key: &str) -> Result<NodeId> {
    let id = v.as_u64().ok_or_else(|| bad(cmd, format!("`{key}` must be a layer id")))?;
    d.layers.iter().any(|l| l.id.0 == id).then_some(NodeId(id)).ok_or_else(|| bad(cmd, format!("`{key}` must be a top-level layer (a timeline row)")))
}

fn commit(s: &mut Session, label: &str, c: Cutters) -> Result<()> {
    s.edit(label, |d, _| {
        c.store(d);
        Ok(())
    })?;
    let current = Anim::of(&s.doc()?.doc).current;
    show(s, current)?;
    Ok(())
}

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "cutter.add";
    let row = row_param(s, p, C)?;
    let d = &s.doc()?.doc;
    let matte = top_row(d, p.get("matte").unwrap_or(&Value::Null), C, "matte")?;
    if matte == row {
        return Err(bad(C, "a row can't cut itself"));
    }
    let inverted = p.get("inverted").and_then(Value::as_bool).unwrap_or(false);
    let mut all = Cutters::of(d);
    all.cutters.retain(|c| c.row != row.0);
    if all.cutters.len() >= MAX_CUTTERS {
        return Err(bad(C, format!("a document takes at most {MAX_CUTTERS} cutters")));
    }
    all.cutters.push(Cutter { row: row.0, matte: matte.0, inverted });
    commit(s, "Add Cutter", all)?;
    Ok(json!({"row": row.0, "matte": matte.0, "inverted": inverted}))
}

fn remove(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "cutter.remove";
    let row = row_param(s, p, C)?;
    let mut all = Cutters::of(&s.doc()?.doc);
    let before = all.cutters.len();
    all.cutters.retain(|c| c.row != row.0);
    if all.cutters.len() == before {
        return Err(bad(C, "this layer has no cutter"));
    }
    commit(s, "Remove Cutter", all)?;
    Ok(json!({"row": row.0}))
}

fn list(s: &mut Session, _: &Value) -> Result<Value> {
    Ok(json!({"cutters": Cutters::of(&s.doc()?.doc).cutters}))
}

#[cfg(test)]
mod tests {
    use vectorcraft_geom::Rect;

    use super::*;

    /// Row 1: a red 100 pt square at the origin. Row 2 (the matte, drawn above): a blue 50 pt
    /// square in its middle. Both keyed at frame 1. Their ids.
    fn scene() -> (Session, u64, u64) {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 100, "height": 100})).unwrap();
        s.execute("paint.setFill", &json!({"color": "#ff0000"})).unwrap();
        let red = s.doc().unwrap().doc.layers[0].id.0;
        let blue = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
        s.execute("layer.setCurrent", &json!({"id": blue})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 25, "y": 25, "width": 50, "height": 50})).unwrap();
        s.execute("paint.setFill", &json!({"color": "#0000ff"})).unwrap();
        (s, red, blue)
    }

    /// The colour shown at (x, y) on frame 1, as RGB.
    fn at(s: &Session, x: u32, y: u32) -> [u8; 3] {
        let d = crate::cmd::anim::at_frame(&s.doc().unwrap().doc, 1);
        let img = vectorcraft_render::Renderer::new().render_region(&d, Rect::new(0.0, 0.0, 100.0, 100.0), 1.0, true);
        let px = img.to_straight();
        let i = ((y * img.width + x) * 4) as usize;
        [px[i], px[i + 1], px[i + 2]]
    }

    #[test]
    fn a_cutter_hides_the_row_under_its_matte_and_the_matte_itself() {
        let (mut s, red, blue) = scene();
        assert_eq!(at(&s, 50, 50), [0, 0, 255], "the blue square on top before cutting");
        s.execute("cutter.add", &json!({"row": red, "matte": blue})).unwrap();
        assert_eq!(at(&s, 50, 50), [255, 255, 255], "cut out, and the matte isn't drawn");
        assert_eq!(at(&s, 10, 10), [255, 0, 0], "the rest stays");
    }

    #[test]
    fn an_inverted_cutter_keeps_only_the_overlap() {
        let (mut s, red, blue) = scene();
        s.execute("cutter.add", &json!({"row": red, "matte": blue, "inverted": true})).unwrap();
        assert_eq!(at(&s, 50, 50), [255, 0, 0]);
        assert_eq!(at(&s, 10, 10), [255, 255, 255]);
    }

    #[test]
    fn removing_a_cutter_shows_both_rows_whole_and_frames_never_stack_masks() {
        let (mut s, red, blue) = scene();
        s.execute("cutter.add", &json!({"row": red, "matte": blue})).unwrap();
        // Showing frames over and over keeps one mask, and the art keeps no trace of it.
        s.execute("anim.goto", &json!({"frame": 2})).unwrap();
        s.execute("anim.goto", &json!({"frame": 1})).unwrap();
        assert_eq!(at(&s, 50, 50), [255, 255, 255]);
        s.execute("cutter.remove", &json!({"row": red})).unwrap();
        let d = &s.doc().unwrap().doc;
        assert!(d.node(NodeId(red)).unwrap().extras().mask.is_none());
        assert!(d.node(NodeId(blue)).unwrap().visible);
        assert_eq!(at(&s, 50, 50), [0, 0, 255]);
        assert!(s.execute("cutter.add", &json!({"row": red, "matte": red})).is_err());
        assert!(s.execute("cutter.remove", &json!({"row": red})).is_err());
    }
}
