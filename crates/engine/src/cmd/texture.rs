//! 7rrek (Creative Studio): textured lines, as in Harmony's pencil and brush textures. A line keeps
//! its vector shape; its grain is a generated, tiled texture, so it looks the same at any zoom.
//! Textures live in the document's preserved extra data (`"textures"`), and their repeating vector
//! tiles live in `doc.patterns` under `"texture:<name>"`. An applied texture attaches an
//! `OpacityMask` to the node: a rectangle covering the path's bounds filled with the texture pattern.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::pattern::{PatternDef, TileType};
use vectorcraft_doc::{Appearance, AppearanceItem, Document, FillLayer, Node, OpacityMask};
use vectorcraft_geom::{Point, Rect};

use super::edit::{roots_of, selected_roots};
use super::*;

const KEY: &str = "textures";
pub const MAX_TEXTURES: usize = 64;

fn default_scale() -> u32 {
    100
}

fn default_density() -> u32 {
    50
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Texture {
    pub name: String,
    pub kind: String,
    #[serde(default = "default_scale")]
    pub scale: u32,
    #[serde(default = "default_density")]
    pub density: u32,
    #[serde(default)]
    pub seed: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Textures {
    pub textures: Vec<Texture>,
}

impl Textures {
    pub fn of(doc: &Document) -> Self {
        let t: Textures = doc.unknown.get(KEY).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
        if t.textures.len() <= MAX_TEXTURES { t } else { Textures::default() }
    }

    pub fn store(&self, doc: &mut Document) {
        if self.textures.is_empty() {
            doc.unknown.remove(KEY);
        } else if let Ok(v) = serde_json::to_value(self) {
            doc.unknown.insert(KEY.into(), v);
        }
    }
}

pub fn pattern_name_for(texture_name: &str) -> String {
    format!("texture:{texture_name}")
}

/// Checks if an opacity mask belongs to a texture applied by us, and returns the texture name.
pub fn texture_name_of_mask(mask: &OpacityMask) -> Option<String> {
    let mut found = None;
    mask.art.walk(&mut |n| {
        if found.is_some() {
            return;
        }
        for item in &n.appearance.items {
            let prefix = match item {
                AppearanceItem::Fill(f) => match &f.paint {
                    Paint::Pattern { pattern, .. } => pattern.strip_prefix("texture:"),
                    _ => None,
                },
                _ => None,
            };
            if let Some(rest) = prefix {
                found = Some(rest.to_string());
                return;
            }
        }
    });
    found
}

/// A lightweight, deterministic linear congruential generator for texture generation without
/// external dependencies.
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { 0x517c_c1b7_2722_0a95 } else { seed })
    }

    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u32() as f64) / 4294967296.0
    }

    fn range_f64(&mut self, min: f64, max: f64) -> f64 {
        min + self.next_f64() * (max - min)
    }
}

fn validate_kind(kind: &str) -> bool {
    matches!(kind, "pencil" | "chalk" | "charcoal" | "grain")
}

pub fn build_pattern_def(tex: &Texture, d: &mut Document) -> PatternDef {
    let base_size = 32.0;
    let scale_factor = (tex.scale as f64 / 100.0).clamp(0.1, 4.0);
    let tile_size = base_size * scale_factor;
    let tile_rect = Rect::new(0.0, 0.0, tile_size, tile_size);

    let mut art = Vec::new();
    let bg =
        Node::path(d.alloc_id(), vectorcraft_geom::shapes::rectangle(tile_rect), Appearance::basic(Paint::solid(Color::WHITE), Paint::None, 0.0));
    art.push(Arc::new(bg));

    let mut rng = Lcg::new(tex.seed);
    let density_factor = (tex.density as f64 / 50.0).clamp(0.0, 2.0);
    let count = (70.0 * density_factor) as usize;

    for _ in 0..count {
        match tex.kind.as_str() {
            "grain" => {
                let x = rng.range_f64(0.0, tile_size);
                let y = rng.range_f64(0.0, tile_size);
                let r = rng.range_f64(0.4, 1.2) * scale_factor;
                let g = rng.range_f64(0.1, 0.7) as f32;
                let dot = Node::path(
                    d.alloc_id(),
                    vectorcraft_geom::shapes::ellipse(Rect::from_center_size(Point::new(x, y), (r * 2.0, r * 2.0))),
                    Appearance::basic(Paint::solid(Color::rgb(g, g, g)), Paint::None, 0.0),
                );
                art.push(Arc::new(dot));
            }
            "pencil" => {
                let x = rng.range_f64(0.0, tile_size);
                let y = rng.range_f64(0.0, tile_size);
                let len = rng.range_f64(1.5, 4.0) * scale_factor;
                let w = rng.range_f64(0.6, 1.4) * scale_factor;
                let angle = rng.range_f64(0.4, 0.9);
                let dx = angle.cos() * len * 0.5;
                let dy = angle.sin() * len * 0.5;
                let g = rng.range_f64(0.2, 0.75) as f32;
                let p1 = Point::new(x - dx, y - dy);
                let p2 = Point::new(x + dx, y + dy);
                let dash = Node::path(
                    d.alloc_id(),
                    vectorcraft_geom::shapes::line(p1, p2),
                    Appearance::basic(Paint::None, Paint::solid(Color::rgb(g, g, g)), w),
                );
                art.push(Arc::new(dash));
            }
            "chalk" => {
                let x = rng.range_f64(0.0, tile_size);
                let y = rng.range_f64(0.0, tile_size);
                let r = rng.range_f64(1.0, 2.5) * scale_factor;
                let g = rng.range_f64(0.35, 0.85) as f32;
                let dab = Node::path(
                    d.alloc_id(),
                    vectorcraft_geom::shapes::ellipse(Rect::from_center_size(Point::new(x, y), (r * 2.0, r * 2.0))),
                    Appearance::basic(Paint::solid(Color::rgb(g, g, g)), Paint::None, 0.0),
                );
                art.push(Arc::new(dab));
            }
            "charcoal" => {
                let x = rng.range_f64(0.0, tile_size);
                let y = rng.range_f64(0.0, tile_size);
                let rw = rng.range_f64(1.0, 3.0) * scale_factor;
                let rh = rng.range_f64(0.8, 2.2) * scale_factor;
                let g = rng.range_f64(0.05, 0.55) as f32;
                let rect = Rect::new(x, y, x + rw, y + rh);
                let clump = Node::path(
                    d.alloc_id(),
                    vectorcraft_geom::shapes::rectangle(rect),
                    Appearance::basic(Paint::solid(Color::rgb(g, g, g)), Paint::None, 0.0),
                );
                art.push(Arc::new(clump));
            }
            _ => {}
        }
    }

    let mut def = PatternDef::new(&pattern_name_for(&tex.name), art);
    def.tile = tile_rect;
    def.tile_type = TileType::Grid;
    def
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "texture.add",
            "Add Texture",
            [],
            None,
            "{name: str, kind: 'pencil'|'chalk'|'charcoal'|'grain', scale?: 10..400, density?: 0..100, seed?: u64} → {name, kind, scale, density, seed}",
            has_doc,
            add
        ),
        cmd!(query "texture.list", "List Textures", [], None, "{} → {textures: [{name, kind, scale, density, seed}]}", has_doc, list),
        cmd!(
            "texture.set",
            "Set Texture",
            [],
            None,
            "{name: str, kind?: str, scale?: 10..400, density?: 0..100, seed?: u64} updates texture and its pattern tile → {name, kind, scale, density, seed}",
            has_doc,
            set
        ),
        cmd!(
            "texture.apply",
            "Apply Texture",
            [],
            None,
            "{name: str, ids?: [u64]} applies texture opacity mask to nodes (default selection) → {name, ids}",
            has_doc,
            apply
        ),
        cmd!(
            "texture.remove",
            "Remove Texture",
            [],
            None,
            "{ids?: [u64]} removes texture opacity mask from nodes (default selection) → {ids}",
            has_doc,
            remove
        ),
        cmd!(
            "texture.delete",
            "Delete Texture",
            [],
            None,
            "{name: str} deletes texture and removes its mask from all nodes → {deleted}",
            has_doc,
            delete
        ),
    ]
}

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "texture.add";
    let name = str_param(p, "name").map(str::trim).filter(|n| !n.is_empty()).ok_or_else(|| bad(C, "`name` is required"))?;
    let kind = str_param(p, "kind").ok_or_else(|| bad(C, "`kind` is required (pencil, chalk, charcoal, grain)"))?;
    if !validate_kind(kind) {
        return Err(bad(C, "`kind` must be pencil, chalk, charcoal, or grain"));
    }

    let scale = match p.get("scale").and_then(Value::as_u64) {
        Some(v) if (10..=400).contains(&v) => v as u32,
        Some(_) => return Err(bad(C, "scale must be between 10 and 400")),
        None => 100,
    };

    let density = match p.get("density").and_then(Value::as_u64) {
        Some(v) if v <= 100 => v as u32,
        Some(_) => return Err(bad(C, "density must be between 0 and 100")),
        None => 50,
    };

    let seed = p.get("seed").and_then(Value::as_u64).unwrap_or(1);

    let mut textures = Textures::of(&s.doc()?.doc);
    if textures.textures.iter().any(|t| t.name == name) {
        return Err(bad(C, format!("texture `{name}` already exists")));
    }
    if textures.textures.len() >= MAX_TEXTURES {
        return Err(bad(C, format!("maximum {MAX_TEXTURES} textures per document")));
    }

    let tex = Texture { name: name.to_string(), kind: kind.to_string(), scale, density, seed };
    textures.textures.push(tex.clone());

    let pat_name = pattern_name_for(name);
    s.edit("Add Texture", |d, _| {
        textures.store(d);
        let pat_def = build_pattern_def(&tex, d);
        d.patterns.retain(|pat| pat.name != pat_name);
        d.patterns.push(pat_def);
        Ok(())
    })?;

    Ok(json!({"name": name, "kind": kind, "scale": scale, "density": density, "seed": seed}))
}

fn list(s: &mut Session, _: &Value) -> Result<Value> {
    let textures = Textures::of(&s.doc()?.doc);
    Ok(json!({"textures": textures.textures}))
}

fn set(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "texture.set";
    let name = str_param(p, "name").ok_or_else(|| bad(C, "`name` is required"))?;
    let mut textures = Textures::of(&s.doc()?.doc);
    let tex = textures.textures.iter_mut().find(|t| t.name == name).ok_or_else(|| bad(C, format!("texture `{name}` not found")))?;

    if let Some(kind) = str_param(p, "kind") {
        if !validate_kind(kind) {
            return Err(bad(C, "`kind` must be pencil, chalk, charcoal, or grain"));
        }
        tex.kind = kind.to_string();
    }

    if let Some(scale_val) = p.get("scale").and_then(Value::as_u64) {
        if !(10..=400).contains(&scale_val) {
            return Err(bad(C, "scale must be between 10 and 400"));
        }
        tex.scale = scale_val as u32;
    }

    if let Some(density_val) = p.get("density").and_then(Value::as_u64) {
        if density_val > 100 {
            return Err(bad(C, "density must be between 0 and 100"));
        }
        tex.density = density_val as u32;
    }

    if let Some(seed_val) = p.get("seed").and_then(Value::as_u64) {
        tex.seed = seed_val;
    }

    let updated = tex.clone();
    let pat_name = pattern_name_for(name);

    s.edit("Set Texture", |d, _| {
        textures.store(d);
        let pat_def = build_pattern_def(&updated, d);
        d.patterns.retain(|pat| pat.name != pat_name);
        d.patterns.push(pat_def);
        Ok(())
    })?;

    Ok(json!({
        "name": updated.name,
        "kind": updated.kind,
        "scale": updated.scale,
        "density": updated.density,
        "seed": updated.seed,
    }))
}

fn apply(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "texture.apply";
    let name = str_param(p, "name").ok_or_else(|| bad(C, "`name` is required"))?;
    let textures = Textures::of(&s.doc()?.doc);
    if !textures.textures.iter().any(|t| t.name == name) {
        return Err(bad(C, format!("texture `{name}` not found")));
    }

    let ids = match ids_param(p, "ids") {
        Some(v) => v,
        None => selected_roots(s)?,
    };
    if ids.is_empty() {
        return Err(bad(C, "no nodes selected to apply texture to"));
    }

    let d = &s.doc()?.doc;
    for id in &ids {
        let node = d.node(*id).ok_or(EngineError::NoNode(*id))?;
        if node.extras().mask.as_deref().is_some_and(|m| texture_name_of_mask(m).is_none()) {
            return Err(bad(C, format!("node {} already has a non-texture opacity mask", id.0)));
        }
    }

    let pat_name = pattern_name_for(name);
    s.edit("Apply Texture", |d, _| {
        for id in &ids {
            let node = d.node(*id).ok_or(EngineError::NoNode(*id))?;
            let bounds =
                node.visual_bounds().or_else(|| node.path_data().and_then(|path| path.bounds())).unwrap_or(Rect::new(0.0, 0.0, 100.0, 100.0));
            let stroke_w = node.appearance.stroke().map(|st| st.width).unwrap_or(1.0);
            let margin = (stroke_w * 2.0).max(20.0);
            let rect = bounds.inflate(margin, margin);

            let fill = Appearance {
                items: vec![AppearanceItem::Fill(FillLayer::new(vectorcraft_doc::pattern::pattern_paint(&pat_name)))],
                ..Appearance::default()
            };
            let mask_art = Node::path(d.alloc_id(), vectorcraft_geom::shapes::rectangle(rect), fill);
            let mask = OpacityMask::new(mask_art, false);
            let node_mut = d.node_mut(*id).ok_or(EngineError::NoNode(*id))?;
            node_mut.extras_mut().mask = Some(Box::new(mask));
        }
        Ok(())
    })?;

    Ok(json!({"name": name, "ids": ids.iter().map(|i| i.0).collect::<Vec<_>>()}))
}

fn remove(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "texture.remove";
    // Given ids take the same roots as the selection: an object inside a listed group is the group's.
    let ids = match ids_param(p, "ids") {
        Some(v) => {
            let d = &s.doc()?.doc;
            for id in &v {
                d.node(*id).ok_or(EngineError::NoNode(*id))?;
            }
            roots_of(d, v)
        }
        None => selected_roots(s)?,
    };

    let d = &s.doc()?.doc;
    for id in &ids {
        let node = d.node(*id).ok_or(EngineError::NoNode(*id))?;
        if node.extras().mask.as_deref().is_some_and(|m| texture_name_of_mask(m).is_none()) {
            return Err(bad(C, format!("node {} has a non-texture opacity mask", id.0)));
        }
    }

    s.edit("Remove Texture", |d, _| {
        for id in &ids {
            if let Some(node) = d.node_mut(*id) {
                let is_texture = node.extras().mask.as_deref().is_some_and(|m| texture_name_of_mask(m).is_some());
                if is_texture {
                    node.extras_mut().mask = None;
                }
            }
        }
        Ok(())
    })?;

    Ok(json!({"ids": ids.iter().map(|i| i.0).collect::<Vec<_>>()}))
}

fn clear_texture_mask(n: &mut Arc<Node>, tex_name: &str) {
    let node = Arc::make_mut(n);
    let matches_tex = node.extras().mask.as_deref().and_then(texture_name_of_mask).as_deref() == Some(tex_name);
    if matches_tex {
        node.extras_mut().mask = None;
    }
    if let Some(children) = node.children_mut() {
        for child in children {
            clear_texture_mask(child, tex_name);
        }
    }
}

fn delete(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "texture.delete";
    let name = str_param(p, "name").ok_or_else(|| bad(C, "`name` is required"))?;
    let mut textures = Textures::of(&s.doc()?.doc);
    let before_len = textures.textures.len();
    textures.textures.retain(|t| t.name != name);
    if textures.textures.len() == before_len {
        return Err(bad(C, format!("texture `{name}` not found")));
    }

    let pat_name = pattern_name_for(name);
    s.edit("Delete Texture", |d, _| {
        textures.store(d);
        d.patterns.retain(|pat| pat.name != pat_name);

        for layer in &mut d.layers {
            clear_texture_mask(layer, name);
        }

        Ok(())
    })?;

    Ok(json!({"deleted": name}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_textured_stroke_exports_with_its_grain_at_2_zoom_levels_same_look() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        s.execute("texture.add", &json!({"name": "Pencil1", "kind": "pencil", "scale": 100, "density": 60, "seed": 42})).unwrap();

        let sq = s.execute("shape.rectangle", &json!({"x": 20, "y": 40, "width": 60, "height": 20})).unwrap()["id"].as_u64().unwrap();
        s.execute("paint.setFill", &json!({"color": "#000000"})).unwrap();
        s.execute("texture.apply", &json!({"name": "Pencil1", "ids": [sq]})).unwrap();

        let doc = &s.doc().unwrap().doc;
        let region = Rect::new(20.0, 40.0, 80.0, 60.0);

        let mut r1 = vectorcraft_render::Renderer::new();
        let img1 = r1.render_region(doc, region, 1.0, false);

        let mut r2 = vectorcraft_render::Renderer::new();
        let img2 = r2.render_region(doc, region, 2.0, false);

        assert_eq!(img2.width, img1.width * 2);
        assert_eq!(img2.height, img1.height * 2);

        let mut diff_sum = 0.0;
        let mut count = 0.0;
        let mut alphas = Vec::new();

        for y in 0..img1.height {
            for x in 0..img1.width {
                let a1 = img1.pixel(x, y)[3] as f64;
                let a2_00 = img2.pixel(2 * x, 2 * y)[3] as f64;
                let a2_10 = img2.pixel(2 * x + 1, 2 * y)[3] as f64;
                let a2_01 = img2.pixel(2 * x, 2 * y + 1)[3] as f64;
                let a2_11 = img2.pixel(2 * x + 1, 2 * y + 1)[3] as f64;
                let a2_avg = (a2_00 + a2_10 + a2_01 + a2_11) / 4.0;

                diff_sum += (a1 - a2_avg).abs();
                count += 1.0;

                if a1 > 20.0 {
                    alphas.push(a1);
                }
            }
        }

        let mean_diff = diff_sum / count;
        assert!(mean_diff < 6.0, "mean alpha diff between 1x and downsampled 2x is {mean_diff}, must be < 6.0");

        assert!(!alphas.is_empty());
        let mean_alpha = alphas.iter().sum::<f64>() / alphas.len() as f64;
        let variance = alphas.iter().map(|a| (a - mean_alpha).powi(2)).sum::<f64>() / alphas.len() as f64;
        let std_dev = variance.sqrt();
        assert!(std_dev > 5.0, "stroke must have grain variance, got std_dev = {std_dev}");
    }

    #[test]
    fn changing_stroke_color_after_texture_keeps_texture_and_recolors() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        s.execute("texture.add", &json!({"name": "Chalk1", "kind": "chalk", "scale": 100, "density": 50})).unwrap();

        let line = s.execute("shape.line", &json!({"x1": 10, "y1": 50, "x2": 90, "y2": 50})).unwrap()["id"].as_u64().unwrap();
        s.execute("stroke.set", &json!({"weight": 10, "ids": [line]})).unwrap();
        s.execute("paint.setStroke", &json!({"color": "#000000", "ids": [line]})).unwrap();
        s.execute("texture.apply", &json!({"name": "Chalk1", "ids": [line]})).unwrap();

        // Change stroke color to red
        s.execute("paint.setStroke", &json!({"color": "#ff0000", "ids": [line]})).unwrap();

        let doc = &s.doc().unwrap().doc;
        let node = doc.node(NodeId(line)).unwrap();
        assert!(node.extras().mask.is_some(), "texture mask stays after stroke recoloring");
        let stroke_color = node.appearance.stroke().and_then(|st| st.paint.color()).unwrap();
        assert_eq!(stroke_color.to_hex().to_lowercase(), "#ff0000");

        let mut r = vectorcraft_render::Renderer::new();
        let region = Rect::new(10.0, 40.0, 90.0, 60.0);
        let img = r.render_region(doc, region, 1.0, false);
        let mid = img.pixel(40, 10);
        assert_eq!(mid[0], 255, "red channel is 255");
        assert_eq!(mid[1], 0, "green channel is 0");
        assert_eq!(mid[2], 0, "blue channel is 0");
    }

    #[test]
    fn texture_set_updates_all_connected_lines() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        s.execute("texture.add", &json!({"name": "Charcoal1", "kind": "charcoal", "scale": 100, "density": 20, "seed": 10})).unwrap();

        let sq = s.execute("shape.rectangle", &json!({"x": 20, "y": 20, "width": 50, "height": 50})).unwrap()["id"].as_u64().unwrap();
        s.execute("paint.setFill", &json!({"color": "#000000"})).unwrap();
        s.execute("texture.apply", &json!({"name": "Charcoal1", "ids": [sq]})).unwrap();

        let mut r = vectorcraft_render::Renderer::new();
        let region = Rect::new(20.0, 20.0, 70.0, 70.0);
        let img_before = r.render_region(&s.doc().unwrap().doc, region, 1.0, false);

        // Update texture to high density and different seed
        s.execute("texture.set", &json!({"name": "Charcoal1", "density": 90, "seed": 999})).unwrap();

        let mut r2 = vectorcraft_render::Renderer::new();
        let img_after = r2.render_region(&s.doc().unwrap().doc, region, 1.0, false);

        assert_ne!(img_before.pixels, img_after.pixels, "texture.set must change appearance of connected lines");
    }

    #[test]
    fn texture_remove_reverts_to_solid_and_foreign_mask_errors() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        s.execute("texture.add", &json!({"name": "Grain1", "kind": "grain"})).unwrap();

        let sq = s.execute("shape.rectangle", &json!({"x": 20, "y": 20, "width": 50, "height": 50})).unwrap()["id"].as_u64().unwrap();
        s.execute("texture.apply", &json!({"name": "Grain1", "ids": [sq]})).unwrap();
        assert!(s.doc().unwrap().doc.node(NodeId(sq)).unwrap().extras().mask.is_some());

        // Remove texture
        s.execute("texture.remove", &json!({"ids": [sq]})).unwrap();
        assert!(s.doc().unwrap().doc.node(NodeId(sq)).unwrap().extras().mask.is_none(), "texture mask removed");

        // Foreign mask on a node
        let foreign_node = s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 10, "height": 10})).unwrap()["id"].as_u64().unwrap();
        s.edit("Add Foreign Mask", |d, _| {
            let dummy_art = Node::path(d.alloc_id(), vectorcraft_geom::shapes::rectangle(Rect::new(0.0, 0.0, 10.0, 10.0)), Appearance::default());
            let n = d.node_mut(NodeId(foreign_node)).unwrap();
            n.extras_mut().mask = Some(Box::new(OpacityMask::new(dummy_art, true)));
            Ok(())
        })
        .unwrap();

        // texture.remove on foreign mask returns Err and doesn't change it
        assert!(s.execute("texture.remove", &json!({"ids": [foreign_node]})).is_err());
        assert!(s.doc().unwrap().doc.node(NodeId(foreign_node)).unwrap().extras().mask.is_some());

        // texture.apply on foreign mask returns Err
        assert!(s.execute("texture.apply", &json!({"name": "Grain1", "ids": [foreign_node]})).is_err());
    }

    #[test]
    fn invalid_inputs_rejected_without_panic() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();

        assert!(s.execute("texture.add", &json!({"name": "", "kind": "pencil"})).is_err());
        assert!(s.execute("texture.add", &json!({"name": "P", "kind": "magic"})).is_err());
        assert!(s.execute("texture.add", &json!({"name": "P", "kind": "pencil", "scale": 0})).is_err());
        assert!(s.execute("texture.add", &json!({"name": "P", "kind": "pencil", "scale": 500})).is_err());
        assert!(s.execute("texture.add", &json!({"name": "P", "kind": "pencil", "density": 200})).is_err());

        s.execute("texture.add", &json!({"name": "Valid", "kind": "pencil"})).unwrap();
        // duplicate name
        assert!(s.execute("texture.add", &json!({"name": "Valid", "kind": "pencil"})).is_err());

        assert!(s.execute("texture.set", &json!({"name": "NoSuchTexture", "scale": 100})).is_err());
        assert!(s.execute("texture.apply", &json!({"name": "NoSuchTexture", "ids": [1]})).is_err());
        assert!(s.execute("texture.delete", &json!({"name": "NoSuchTexture"})).is_err());
    }

    #[test]
    fn save_and_open_keeps_texture_and_mask() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        s.execute("texture.add", &json!({"name": "SavedPencil", "kind": "pencil", "scale": 150, "density": 70})).unwrap();

        let line = s.execute("shape.line", &json!({"x1": 10, "y1": 10, "x2": 90, "y2": 90})).unwrap()["id"].as_u64().unwrap();
        s.execute("texture.apply", &json!({"name": "SavedPencil", "ids": [line]})).unwrap();

        let saved = vectorcraft_format::save(&s.doc().unwrap().doc, false);
        let loaded = vectorcraft_format::load(&saved).expect("loads successfully");

        let texs = Textures::of(&loaded);
        assert_eq!(texs.textures.len(), 1);
        assert_eq!(texs.textures[0].name, "SavedPencil");
        assert_eq!(texs.textures[0].scale, 150);
        assert_eq!(texs.textures[0].density, 70);

        let node = loaded.node(NodeId(line)).expect("node exists");
        let mask = node.extras().mask.as_deref().expect("mask exists");
        assert_eq!(texture_name_of_mask(mask).as_deref(), Some("SavedPencil"));
    }
}
