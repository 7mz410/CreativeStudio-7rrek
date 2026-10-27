//! 7rrek (Creative Studio): line art and colour art, as in Harmony. A drawing (a keyframe
//! sublayer) keeps its lines as they are; paint fills go on its "Colour Art" sublayer, made on
//! first use at the bottom of the drawing, so every fill sits under every line. A drawing is the
//! same node wherever it is exposed, so a fill shows in every exposure.

use serde_json::json;
use vectorcraft_color::Paint;
use vectorcraft_doc::{Appearance, AppearanceItem, Document, FillLayer, LayerColor, Node, NodeKind};

use super::anim::Anim;
use super::edit::selected_roots;
use super::*;

/// The name of a drawing's colour art sublayer.
pub const COLOUR_ART: &str = "Colour Art";

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "art.fill",
        "Paint Colour Art",
        [],
        None,
        "{ids?: closed paths inside a drawing (default: the selection), color?: \"#rrggbb\" (default: the current fill)} copies each path's shape onto its drawing's Colour Art sublayer (made at the bottom of the drawing on first use) filled with the colour, under every line; the line itself is left as it is → {ids: the fills made}",
        has_doc,
        fill
    )]
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

fn fill(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "art.fill";
    let ids = match ids_param(p, "ids") {
        Some(v) => v,
        None => selected_roots(s)?,
    };
    let paint = match p.get("color").and_then(Value::as_str) {
        Some(c) => Paint::solid(vectorcraft_color::Color::from_hex(c).ok_or_else(|| bad(C, format!("`color`: {c} isn't a colour")))?),
        None => s.paint.fill.clone(),
    };
    if paint.is_none() {
        return Err(bad(C, "the fill colour is None: pick a colour to paint with"));
    }
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
            let layer = match colour_art(d, drawing) {
                Some(l) => l,
                None => {
                    let id = d.alloc_id();
                    d.insert(Some(drawing), 0, Node::layer(id, COLOUR_ART, LayerColor::Preset(0)))?
                }
            };
            let look = Appearance { items: vec![AppearanceItem::Fill(FillLayer::new(paint.clone()))], ..Appearance::default() };
            let id = d.alloc_id();
            made.push(d.insert(Some(layer), usize::MAX, Node::path(id, path, look))?.0);
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
