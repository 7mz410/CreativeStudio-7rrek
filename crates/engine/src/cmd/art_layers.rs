//! 7rrek (Creative Studio): line art and colour art, as in Harmony. A drawing (a keyframe
//! sublayer) keeps its lines as they are; paint fills go on its "Colour Art" sublayer, made on
//! first use at the bottom of the drawing, so every fill sits under every line. A drawing is the
//! same node wherever it is exposed, so a fill shows in every exposure.

use serde_json::json;
use vectorcraft_color::Paint;
use vectorcraft_doc::{Appearance, AppearanceItem, Document, FillLayer, LayerColor, Node, NodeKind};
use vectorcraft_geom::{FillRule, PathData, Point};
use vectorcraft_pathops as po;

use super::anim::Anim;
use super::edit::selected_roots;
use super::*;

/// The name of a drawing's colour art sublayer.
pub const COLOUR_ART: &str = "Colour Art";

/// Harmony's Close Gap presets (none, small, medium, large), in points.
pub const GAPS: [f64; 4] = [0.0, 2.0, 4.0, 8.0];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "art.paintAt",
            "Paint",
            [],
            None,
            "{x, y: document point, gap?: pt 0..20 (default 4; Harmony's presets 0/2/4/8), color?, row?: layer id (default: the current layer's row)} fills the area round the point that the drawing shown at the current frame closes with its lines, closing gaps up to `gap` invisibly; the fill goes on the drawing's Colour Art, under its lines → {id}",
            has_doc,
            paint_at
        ),
        cmd!(
            "art.fill",
            "Paint Colour Art",
            [],
            None,
            "{ids?: closed paths inside a drawing (default: the selection), color?: \"#rrggbb\" (default: the current fill)} copies each path's shape onto its drawing's Colour Art sublayer (made at the bottom of the drawing on first use) filled with the colour, under every line; the line itself is left as it is → {ids: the fills made}",
            has_doc,
            fill
        ),
    ]
}

/// The drawing (keyframe sublayer) `id` sits in, itself included.
fn drawing_of(d: &Document, keys: &Anim, mut id: NodeId) -> Option<NodeId> {
    loop {
        if keys.keys.contains_key(&id.0) {
            return Some(id);
        }
        id = d.parent_of(id)?;
    }
}

/// The drawing's Colour Art sublayer, if it has one.
fn colour_art(d: &Document, drawing: NodeId) -> Option<NodeId> {
    d.node(drawing)?.children()?.iter().find(|c| matches!(c.kind, NodeKind::Layer { .. }) && c.name.as_deref() == Some(COLOUR_ART)).map(|c| c.id)
}

/// The paint a command asks for: `color`, else the current fill (never None).
fn paint_param(s: &Session, p: &Value, cmd: &str) -> Result<Paint> {
    let paint = match p.get("color").and_then(Value::as_str) {
        Some(c) => Paint::solid(vectorcraft_color::Color::from_hex(c).ok_or_else(|| bad(cmd, format!("`color`: {c} isn't a colour")))?),
        None => s.paint.fill.clone(),
    };
    if paint.is_none() {
        return Err(bad(cmd, "the fill colour is None: pick a colour to paint with"));
    }
    Ok(paint)
}

/// Puts `path` filled with `paint` on top of `drawing`'s Colour Art (made on first use).
fn add_fill(d: &mut Document, drawing: NodeId, path: PathData, paint: &Paint) -> std::result::Result<NodeId, vectorcraft_doc::DocError> {
    let layer = match colour_art(d, drawing) {
        Some(l) => l,
        None => {
            let id = d.alloc_id();
            d.insert(Some(drawing), 0, Node::layer(id, COLOUR_ART, LayerColor::Preset(0)))?
        }
    };
    let look = Appearance { items: vec![AppearanceItem::Fill(FillLayer::new(paint.clone()))], ..Appearance::default() };
    let id = d.alloc_id();
    d.insert(Some(layer), usize::MAX, Node::path(id, path, look))
}

fn paint_at(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "art.paintAt";
    let num = |k: &str| p.get(k).and_then(Value::as_f64).filter(|v| v.is_finite());
    let (Some(x), Some(y)) = (num("x"), num("y")) else { return Err(bad(C, "`x` and `y` must be numbers")) };
    let gap = num("gap").unwrap_or(4.0);
    if !(0.0..=20.0).contains(&gap) {
        return Err(bad(C, "`gap` must be 0..20 pt"));
    }
    let paint = paint_param(s, p, C)?;
    let row = super::anim::row_param(s, p, C)?;
    let d = &s.doc()?.doc;
    let anim = Anim::of(d);
    let row_node = d.node(row).ok_or(EngineError::NoNode(row))?;
    let drawing = anim
        .row_frames(row_node)
        .into_iter()
        .rfind(|(f, _)| *f <= anim.current)
        .map(|(_, id)| id)
        .ok_or_else(|| bad(C, "no drawing at this frame"))?;
    // The lines: every stroked path of the drawing outside its Colour Art, outlined with the gap
    // added, so a gap up to `gap` wide closes.
    let ca = colour_art(d, drawing);
    let mut lines = Vec::new();
    let mut widest: f64 = 0.0;
    let mut walk = vec![d.node(drawing).ok_or(EngineError::NoNode(drawing))?];
    while let Some(n) = walk.pop() {
        if Some(n.id) == ca {
            continue;
        }
        walk.extend(n.children().into_iter().flatten().map(|c| c.as_ref()));
        if let (Some(path), Some(st)) = (n.path_data(), n.appearance.stroke()) {
            widest = widest.max(st.width);
            lines.push(po::outline_stroke(path, st.width + gap, po::Cap::Round, po::Join::Round, 4.0));
        }
    }
    let refs: Vec<(&PathData, FillRule)> = lines.iter().map(|l| (l, FillRule::NonZero)).collect();
    let walls = po::boolean_n(&refs, |v| v.iter().any(|b| *b));
    let Some(r) = walls.bounds() else { return Err(bad(C, "the drawing has no lines to paint inside")) };
    let outside = r.inflate(10.0, 10.0);
    let open = po::boolean(&vectorcraft_geom::shapes::rectangle(outside), FillRule::NonZero, &walls, FillRule::NonZero, po::BoolOp::Difference);
    let point = Point::new(x, y);
    let zone = po::region_at(&[po::Shape::new(open, FillRule::NonZero, 0)], point).ok_or_else(|| bad(C, "that point is on a line"))?;
    // The face that reaches the frame round the lines is outside them: the area leaks.
    if zone.path.bounds().is_none_or(|b| b.width() >= outside.width() - 1.0 || b.height() >= outside.height() - 1.0) {
        return Err(bad(C, "the area isn't closed: raise `gap` or close the lines"));
    }
    // Grown back under the lines, so no seam shows between fill and line.
    let path = po::offset_path(&zone.path, (widest + gap) / 2.0, po::Join::Round, 4.0);
    let mut made = None;
    s.edit("Paint", |d, _| {
        made = Some(add_fill(d, drawing, path, &paint)?);
        Ok(())
    })?;
    Ok(json!({"id": made.map(|i| i.0)}))
}

fn fill(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "art.fill";
    let ids = match ids_param(p, "ids") {
        Some(v) => v,
        None => selected_roots(s)?,
    };
    let paint = paint_param(s, p, C)?;
    let d = &s.doc()?.doc;
    let anim = Anim::of(d);
    let mut jobs = Vec::new();
    for id in ids {
        let n = d.node(id).ok_or(EngineError::NoNode(id))?;
        let path = n.path_data().filter(|p| p.is_closed()).ok_or_else(|| bad(C, format!("{} isn't a closed path", id.0)))?;
        let drawing = drawing_of(d, &anim, id).ok_or_else(|| bad(C, format!("{} isn't in a drawing (a keyframe)", id.0)))?;
        jobs.push((drawing, path.clone()));
    }
    if jobs.is_empty() {
        return Err(bad(C, "nothing to paint: select closed paths in a drawing"));
    }
    let mut made = Vec::new();
    s.edit("Paint Colour Art", |d, _| {
        for (drawing, path) in jobs {
            made.push(add_fill(d, drawing, path, &paint)?.0);
        }
        Ok(())
    })?;
    Ok(json!({"ids": made}))
}

#[cfg(test)]
mod tests {
    use vectorcraft_geom::Rect;

    use super::*;

    /// The colour shown at (x, y) on `frame`, as RGB.
    fn at(s: &Session, frame: u32, x: u32, y: u32) -> [u8; 3] {
        let d = crate::cmd::anim::at_frame(&s.doc().unwrap().doc, frame);
        let img = vectorcraft_render::Renderer::new().render_region(&d, Rect::new(0.0, 0.0, 100.0, 100.0), 1.0, true);
        let px = img.to_straight();
        let i = ((y * img.width + x) * 4) as usize;
        [px[i], px[i + 1], px[i + 2]]
    }

    /// D3: a 60 pt circle line with a 3 pt gap fills with gap closing on, and leaks without it.
    #[test]
    fn paint_closes_small_gaps_and_refuses_a_leak() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        // An open arc: the circle round (50, 50), radius 30, with a 3 pt gap at its right.
        let a = (3.0f64 / 30.0) / 2.0;
        let pts: Vec<[f64; 2]> = (0..=96)
            .map(|i| a + (std::f64::consts::TAU - 2.0 * a) * i as f64 / 96.0)
            .map(|t| [50.0 + 30.0 * t.cos(), 50.0 + 30.0 * t.sin()])
            .collect();
        s.execute("path.freehand", &json!({"points": pts, "fidelity": 0.2})).unwrap();
        s.execute("paint.setFill", &json!({"none": true})).unwrap();
        s.execute("stroke.set", &json!({"weight": 2})).unwrap();
        assert!(s.execute("art.paintAt", &json!({"x": 50, "y": 50, "gap": 0, "color": "#ff0000"})).is_err(), "leaks without gap closing");
        s.execute("art.paintAt", &json!({"x": 50, "y": 50, "gap": 4, "color": "#ff0000"})).unwrap();
        assert_eq!(at(&s, 1, 50, 50), [255, 0, 0]);
        assert_eq!(at(&s, 1, 50, 50 + 29), [0, 0, 0], "the line stays on top");
        assert_eq!(at(&s, 1, 5, 5), [255, 255, 255], "nothing outside");
        assert!(s.execute("art.paintAt", &json!({"x": 50, "y": 50, "gap": 50})).is_err());
    }

    #[test]
    fn a_fill_goes_under_the_line_on_colour_art_in_every_exposure() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        let k1 = s.execute("anim.keyframe", &json!({"frame": 1})).unwrap()["id"].as_u64().unwrap();
        // A black 10 pt line round a square, no fill.
        let sq = s.execute("shape.rectangle", &json!({"x": 20, "y": 20, "width": 60, "height": 60})).unwrap()["id"].as_u64().unwrap();
        s.execute("paint.setFill", &json!({"none": true})).unwrap();
        s.execute("stroke.set", &json!({"weight": 10})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 5, "blank": true})).unwrap();
        let r = s.execute("art.fill", &json!({"ids": [sq], "color": "#ff0000"})).unwrap();
        assert_eq!(r["ids"].as_array().unwrap().len(), 1);
        let d = &s.doc().unwrap().doc;
        let ca = colour_art(d, NodeId(k1)).expect("a colour art sublayer");
        assert_eq!(d.node(NodeId(k1)).unwrap().children().unwrap()[0].id, ca, "at the bottom of the drawing");
        assert_eq!(at(&s, 1, 50, 50), [255, 0, 0], "filled");
        assert_eq!(at(&s, 1, 20, 50), [0, 0, 0], "the line stays on top of the fill");
        // Drawing 1 exposed again at frame 8: the fill comes with it.
        s.execute("anim.expose", &json!({"frame": 8, "drawing": k1})).unwrap();
        assert_eq!(at(&s, 8, 50, 50), [255, 0, 0]);
        assert_eq!(at(&s, 5, 50, 50), [255, 255, 255], "the blank drawing has none");
        // A second fill reuses the sublayer; one undo takes it back.
        s.execute("art.fill", &json!({"ids": [sq], "color": "#00ff00"})).unwrap();
        assert_eq!(s.doc().unwrap().doc.node(ca).unwrap().children().unwrap().len(), 2);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc().unwrap().doc.node(ca).unwrap().children().unwrap().len(), 1);
        assert!(s.execute("art.fill", &json!({"ids": [999999]})).is_err());
    }
}
