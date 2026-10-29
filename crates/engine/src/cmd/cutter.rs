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
use vectorcraft_doc::{Appearance, AppearanceItem, Document, FillLayer, ImageBlob, Node, OpacityMask};

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
    /// What of the matte counts: its shape (default), or its brightness (white cuts fully, black
    /// not at all; a gradient cuts part way along it).
    #[serde(default, skip_serializing_if = "is_default")]
    pub by: MatteBy,
    /// More matte rows, combined with the first by `op` (a matte composite).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also: Vec<u64>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub op: MatteOp,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MatteBy {
    #[default]
    Shape,
    Brightness,
}

/// How a cutter's mattes combine: all of them, the first less the others, or only where all overlap.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MatteOp {
    #[default]
    Add,
    Subtract,
    Intersect,
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

/// Turns every colour one colour: a matte cuts by its shape, not its colours. An image keeps its
/// alpha and takes the colour on every pixel; the copies it makes are kept in `new`.
struct Flat<'a> {
    color: Color,
    doc: &'a Document,
    new: Vec<(String, ImageBlob)>,
}

impl ColorVisitor for Flat<'_> {
    fn paint(&mut self, p: &mut Paint) -> bool {
        let c = self.color;
        vectorcraft_doc::recolor::map_paint(p, &|_| c)
    }
    fn color(&mut self, _: Color) -> Color {
        self.color
    }
    fn image(&mut self, key: &str) -> Option<String> {
        let [r, g, b, _] = self.color.to_rgba8(1.0);
        let blob = self.doc.images.get(key)?.map_rgb(|_| [r, g, b])?;
        let k = blob.content_key();
        self.new.push((k.clone(), blob));
        Some(k)
    }
}

/// The visible art of matte row `id` as one group, every colour `paint` if given (else its own);
/// image copies it makes go in `new`.
fn matte_art(d: &Document, id: NodeId, paint: Option<Color>, new: &mut Vec<(String, ImageBlob)>) -> Option<Node> {
    let m = d.layers.iter().find(|l| l.id == id)?;
    let mut art = Node::group(NodeId(0), m.children().into_iter().flatten().filter(|c| c.visible).cloned().collect());
    if let Some(color) = paint {
        let mut flat = Flat { color, doc: d, new: Vec::new() };
        recolor_node(&mut art, Reach::ALL, &mut flat);
        new.extend(flat.new);
    }
    Some(art)
}

/// Builds every cutter for the frame shown in `d`: the input row gets an opacity mask of its
/// matte's visible art in white, and the matte row is hidden. A row that already has an opacity
/// mask of its own keeps it (and isn't cut). Returns (rows masked, matte rows hidden), for the next
/// frame shown to take back.
pub(crate) fn apply(d: &mut Document) -> (Vec<u64>, Vec<u64>) {
    let (mut cut, mut mattes) = (Vec::new(), Vec::new());
    for c in Cutters::of(d).cutters {
        let row = NodeId(c.row);
        let ids: Vec<NodeId> = std::iter::once(c.matte).chain(c.also.iter().copied()).map(NodeId).collect();
        if ids.contains(&row) || !d.layers.iter().any(|l| l.id == row) || d.node(row).is_some_and(|n| n.extras().mask.is_some()) {
            continue;
        }
        let Some(r) = d.bounds_of(&[row], true) else { continue };
        // The mask shows where it is white. By shape: inverted, the mattes in white (all else
        // hidden); by default, white over the whole row with the mattes in black on it. A
        // see-through matte comes out grey and cuts part way. By brightness the mattes keep their
        // colours over black, and the mask is inverted unless the cutter is.
        let (on, off, back) = match (c.by, c.inverted) {
            (MatteBy::Shape, true) => (Some(Color::WHITE), Color::BLACK, None),
            (MatteBy::Shape, false) => (Some(Color::BLACK), Color::WHITE, Some(Color::WHITE)),
            (MatteBy::Brightness, _) => (None, Color::BLACK, Some(Color::BLACK)),
        };
        let mut new = Vec::new();
        let mut parts = Vec::new();
        for (i, id) in ids.iter().enumerate() {
            let paint = if i == 0 || c.op == MatteOp::Add {
                on
            } else if c.op == MatteOp::Subtract {
                Some(off)
            } else {
                Some(Color::WHITE)
            };
            let Some(art) = matte_art(d, *id, paint, &mut new) else { continue };
            match (c.op, parts.pop()) {
                (MatteOp::Intersect, Some(first)) if i > 0 => {
                    let mut g = Node::group(NodeId(0), vec![std::sync::Arc::new(first)]);
                    g.extras_mut().mask = Some(Box::new(OpacityMask::new(art, true)));
                    parts.push(g);
                }
                (_, prev) => parts.extend(prev.into_iter().chain([art])),
            }
        }
        if parts.is_empty() {
            continue;
        }
        if let Some(back) = back {
            let margin = 0.1 * r.width().max(r.height()) + 10.0;
            let fill = Appearance { items: vec![AppearanceItem::Fill(FillLayer::new(Paint::solid(back)))], ..Appearance::default() };
            parts.insert(0, Node::path(NodeId(0), vectorcraft_geom::shapes::rectangle(r.inflate(margin, margin)), fill));
        }
        d.images.extend(new);
        let mut mask = OpacityMask::new(Node::group(NodeId(0), parts.into_iter().map(std::sync::Arc::new).collect()), true);
        mask.invert = c.by == MatteBy::Brightness && !c.inverted;
        if let Some(n) = d.node_mut(row) {
            n.extras_mut().mask = Some(Box::new(mask));
            cut.push(row.0);
        }
        for id in ids {
            if let Some(n) = d.node_mut(id)
                && n.visible
            {
                n.visible = false;
                mattes.push(id.0);
            }
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
            "{row?: layer id to cut (default: the current layer's row), matte: layer id whose shape cuts it, inverted?: bool (keep only the overlap; default false: hide the overlap), by?: \"shape\"|\"brightness\" (what of the matte counts: its shape, or its brightness, white cutting fully and black not at all; default shape), also?: [layer ids] (up to 8 more matte rows; no row twice), op?: \"add\"|\"subtract\"|\"intersect\" (how the mattes combine: all cut; the first less the others; only where all overlap; default add)} a row has one cutter, so this replaces any it had; the matte row's art stops showing → {row, matte, inverted, by, also, op}",
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

/// The option `key` names (one of `options`), or None when it is absent or null.
fn choice<T: Copy>(p: &Value, key: &str, cmd: &str, options: &[(&str, T)]) -> Result<Option<T>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_str().and_then(|s| options.iter().find(|(name, _)| *name == s)).map(|(_, t)| Some(*t)).ok_or_else(|| {
            let names: Vec<&str> = options.iter().map(|(name, _)| *name).collect();
            bad(cmd, format!("`{key}` must be one of {}", names.join(", ")))
        }),
    }
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
    const MAX_ALSO: usize = 8;
    let also = match p.get("also") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(v)) if v.len() <= MAX_ALSO => v.iter().map(|x| top_row(d, x, C, "also")).collect::<Result<Vec<NodeId>>>()?,
        Some(Value::Array(_)) => return Err(bad(C, format!("`also` takes at most {MAX_ALSO} matte rows"))),
        Some(_) => return Err(bad(C, "`also` must be a list of layer ids")),
    };
    let mut seen = vec![matte];
    for id in &also {
        if *id == row {
            return Err(bad(C, "a row can't cut itself"));
        }
        if seen.contains(id) {
            return Err(bad(C, "`also` repeats a matte row"));
        }
        seen.push(*id);
    }
    let inverted = p.get("inverted").and_then(Value::as_bool).unwrap_or(false);
    let by = choice(p, "by", C, &[("shape", MatteBy::Shape), ("brightness", MatteBy::Brightness)])?.unwrap_or_default();
    let op = choice(p, "op", C, &[("add", MatteOp::Add), ("subtract", MatteOp::Subtract), ("intersect", MatteOp::Intersect)])?.unwrap_or_default();
    let mut all = Cutters::of(d);
    all.cutters.retain(|c| c.row != row.0);
    if all.cutters.len() >= MAX_CUTTERS {
        return Err(bad(C, format!("a document takes at most {MAX_CUTTERS} cutters")));
    }
    let also: Vec<u64> = also.iter().map(|id| id.0).collect();
    all.cutters.push(Cutter { row: row.0, matte: matte.0, inverted, by, also: also.clone(), op });
    commit(s, "Add Cutter", all)?;
    Ok(json!({"row": row.0, "matte": matte.0, "inverted": inverted, "by": by, "also": also, "op": op}))
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
    use vectorcraft_doc::{ImageObject, NodeKind};
    use vectorcraft_geom::{Affine, Rect};

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

    /// A matte row holding an opaque white 4×4 image at (25, 25), 50 pt square, and the red row
    /// below it. White, so the cut comes from its outline: its own brightness would show the red.
    #[test]
    fn a_matte_image_cuts_by_its_outline_not_its_brightness() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 100, "height": 100})).unwrap();
        s.execute("paint.setFill", &json!({"color": "#ff0000"})).unwrap();
        let red = s.doc().unwrap().doc.layers[0].id.0;
        let matte = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
        s.execute("layer.setCurrent", &json!({"id": matte})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        let mut png = vec![];
        image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 255, 255, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        s.edit("Place", |d, _| {
            d.images.insert("white".into(), ImageBlob::png(png));
            let id = d.alloc_id();
            let im = ImageObject {
                key: "white".into(),
                width: 4,
                height: 4,
                xf: Affine::new([12.5, 0.0, 0.0, 12.5, 25.0, 25.0]),
                link: None,
                placement: Default::default(),
                pixels: None,
            };
            d.insert(Some(NodeId(matte)), 0, Node::new(id, NodeKind::Image(Box::new(im))))?;
            Ok(())
        })
        .unwrap();
        assert_eq!(at(&s, 50, 50), [255, 255, 255], "the white image on top before cutting");
        s.execute("cutter.add", &json!({"row": red, "matte": matte})).unwrap();
        assert_eq!(at(&s, 50, 50), [255, 255, 255], "the row is hidden under the image's outline");
        assert_eq!(at(&s, 10, 10), [255, 0, 0], "the rest stays");
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

    /// A 100 pt red page at frame 1 on the bottom row. Its id.
    fn red_page() -> (Session, u64) {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 100, "height": 100})).unwrap();
        s.execute("paint.setFill", &json!({"color": "#ff0000"})).unwrap();
        let red = s.doc().unwrap().doc.layers[0].id.0;
        (s, red)
    }

    /// A new row on top with one solid rectangle at frame 1. Its id.
    fn rect_row(s: &mut Session, x: f64, y: f64, w: f64, h: f64, hex: &str) -> u64 {
        let row = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
        s.execute("layer.setCurrent", &json!({"id": row})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("shape.rectangle", &json!({"x": x, "y": y, "width": w, "height": h})).unwrap();
        s.execute("paint.setFill", &json!({"color": hex})).unwrap();
        row
    }

    /// The red page with two full-height strips over it: x 0..60 (blue) and x 40..90 (green).
    /// Points: x 10 is in the first only, 50 in both, 80 in the second only, 95 in neither.
    fn two_strips() -> (Session, u64, u64, u64) {
        let (mut s, red) = red_page();
        let a = rect_row(&mut s, 0.0, 0.0, 60.0, 100.0, "#0000ff");
        let b = rect_row(&mut s, 40.0, 0.0, 50.0, 100.0, "#00ff00");
        (s, red, a, b)
    }

    #[test]
    fn a_brightness_matte_cuts_part_way_by_its_grey_and_smoothly() {
        let (mut s, red) = red_page();
        let m = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
        s.execute("layer.setCurrent", &json!({"id": m})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 100, "height": 100})).unwrap();
        s.execute("paint.editGradient", &json!({"kind": "linear", "stops": [{"offset": 0, "color": "#000000"}, {"offset": 1, "color": "#ffffff"}]}))
            .unwrap();
        s.execute("cutter.add", &json!({"row": red, "matte": m, "by": "brightness"})).unwrap();
        // Red over white: the green channel is 255 minus the red's alpha, so it rises left to right.
        let g: Vec<i32> = [10, 30, 50, 70, 90].iter().map(|&x| i32::from(at(&s, x, 50)[1])).collect();
        assert!(g.windows(2).all(|w| w[0] <= w[1]), "monotonic left to right: {g:?}");
        assert!(g.windows(2).all(|w| w[1] - w[0] <= 127), "no jump over half the range: {g:?}");
        assert!(g[0] < 128 && g[4] > 128, "red at the black end, cut at the white end: {g:?}");
    }

    #[test]
    fn subtract_shows_the_row_again_where_the_second_matte_overlaps_the_first() {
        let (mut s, red, a, b) = two_strips();
        s.execute("cutter.add", &json!({"row": red, "matte": a, "also": [b], "op": "subtract"})).unwrap();
        assert_eq!(at(&s, 10, 50), [255, 255, 255], "the first matte alone is cut");
        assert_eq!(at(&s, 50, 50), [255, 0, 0], "the overlap shows the row again");
        assert_eq!(at(&s, 80, 50), [255, 0, 0], "the second matte alone doesn't cut");
    }

    #[test]
    fn intersect_cuts_only_where_both_mattes_cover() {
        let (mut s, red, a, b) = two_strips();
        s.execute("cutter.add", &json!({"row": red, "matte": a, "also": [b], "op": "intersect"})).unwrap();
        assert_eq!(at(&s, 50, 50), [255, 255, 255], "both cover: cut");
        assert_eq!(at(&s, 10, 50), [255, 0, 0], "the first alone: not cut");
        assert_eq!(at(&s, 80, 50), [255, 0, 0], "the second alone: not cut");
    }

    #[test]
    fn add_cuts_where_either_matte_covers() {
        let (mut s, red, a, b) = two_strips();
        s.execute("cutter.add", &json!({"row": red, "matte": a, "also": [b]})).unwrap();
        assert_eq!(at(&s, 10, 50), [255, 255, 255]);
        assert_eq!(at(&s, 50, 50), [255, 255, 255]);
        assert_eq!(at(&s, 80, 50), [255, 255, 255]);
        assert_eq!(at(&s, 95, 50), [255, 0, 0], "in neither: kept");
    }

    #[test]
    fn cutter_add_rejects_unknown_by_and_op_and_bad_also() {
        let (mut s, red, a, b) = two_strips();
        assert!(s.execute("cutter.add", &json!({"row": red, "matte": a, "by": "colour"})).is_err());
        assert!(s.execute("cutter.add", &json!({"row": red, "matte": a, "op": "xor"})).is_err());
        assert!(s.execute("cutter.add", &json!({"row": red, "matte": a, "also": [red]})).is_err(), "a row can't cut itself");
        assert!(s.execute("cutter.add", &json!({"row": red, "matte": a, "also": [b, b]})).is_err(), "no duplicates");
        assert!(s.execute("cutter.add", &json!({"row": red, "matte": a, "also": [a]})).is_err(), "the matte again");
        assert!(s.execute("cutter.add", &json!({"row": red, "matte": a, "also": "x"})).is_err());
        assert!(Cutters::of(&s.doc().unwrap().doc).cutters.is_empty(), "nothing stored by the rejected calls");
        let r = s.execute("cutter.add", &json!({"row": red, "matte": a, "also": [b], "op": "intersect", "by": "brightness"})).unwrap();
        assert_eq!((r["by"].as_str(), r["op"].as_str()), (Some("brightness"), Some("intersect")));
        assert_eq!(r["also"], json!([b]));
    }
}
