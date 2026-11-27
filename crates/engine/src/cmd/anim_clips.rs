//! 7rrek: named clips on the timeline (`anim.clip`), listed by `anim.clips`, and exported per clip
//! as a sprite sheet with an optional Godot 4 SpriteFrames resource (`anim.exportSheet`).

use serde_json::json;

use super::*;
use crate::EngineError;
use crate::cmd::anim::{Anim, Clip, MAX_FPS, encode_frames};

/// Most pixels one sheet may hold (a sheet is built in memory).
const MAX_SHEET_PIXELS: u64 = 50_000_000;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "anim.clip",
            "Clip",
            [],
            None,
            "{name, from: frame, to: frame (1-based, within the timeline, from ≤ to), loop?: bool (default true), fps?: 1–60 (default: the timeline's), remove?: bool} adds or replaces the clip called name; remove: true deletes it → {clips: [{name, from, to, loop, fps}]}",
            has_doc,
            clip
        ),
        cmd!(
            query "anim.clips",
            "Clips",
            [],
            None,
            "{} → {clips: [{name, from, to, loop, fps}]}",
            has_doc,
            clips
        ),
        cmd!(
            query "anim.exportSheet",
            "Export Sprite Sheets…",
            [],
            None,
            "{dir, clip?: name (default: every clip; with no clips, the whole timeline as one clip named \"default\"), columns?: n (default: all frames in one row), scale?: (default 1), alpha?: bool (default true), godot?: bool (also writes <document title or sprite>.tres, a Godot 4 SpriteFrames resource with one animation per clip)} writes <clip>.png (frames packed left to right, top to bottom, equal cells) and <clip>.json ({frame_width, frame_height, columns, frames, fps, loop}) → {dir, files}",
            has_doc,
            export_sheet
        ),
    ]
}

/// A clip as the commands report it: its frame rate resolved against the timeline's.
fn clip_json(c: &Clip, anim: &Anim) -> Value {
    json!({"name": c.name, "from": c.from, "to": c.to, "loop": c.looped, "fps": c.fps.unwrap_or(anim.fps)})
}

fn clips_json(anim: &Anim) -> Value {
    Value::Array(anim.clips.iter().map(|c| clip_json(c, anim)).collect())
}

/// A clip name that is safe as a file name and a Godot string: not empty, no separators or quotes.
fn valid_name(n: &str) -> bool {
    !n.is_empty() && !n.chars().any(|c| matches!(c, '/' | '\\' | '"') || c.is_control())
}

/// A frame number from `p[key]`: a whole number from 1 to `frames`.
fn frame_param(p: &Value, key: &str, frames: u32, c: &str) -> Result<u32> {
    p.get(key)
        .and_then(Value::as_u64)
        .filter(|f| (1..=frames as u64).contains(f))
        .map(|f| f as u32)
        .ok_or_else(|| bad(c, format!("{key} must be a whole number from 1 to {frames}")))
}

fn clip(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "anim.clip";
    let name = str_param(p, "name")
        .filter(|n| valid_name(n))
        .ok_or_else(|| bad(C, "name is required (no slashes, quotes or control characters)"))?
        .to_string();
    if p.get("remove").and_then(Value::as_bool).unwrap_or(false) {
        let clips = s.edit("Remove Clip", |d, _| {
            let mut a = Anim::of(d);
            let before = a.clips.len();
            a.clips.retain(|c| c.name != name);
            if a.clips.len() == before {
                return Err(bad(C, format!("no clip named {name}")));
            }
            a.store(d);
            Ok(clips_json(&a))
        })?;
        return Ok(json!({"clips": clips}));
    }
    let frames = Anim::of(&s.doc()?.doc).frames;
    let from = frame_param(p, "from", frames, C)?;
    let to = frame_param(p, "to", frames, C)?;
    if from > to {
        return Err(bad(C, "from must not be after to"));
    }
    let looped = match p.get("loop") {
        None | Some(Value::Null) => true,
        Some(v) => v.as_bool().ok_or_else(|| bad(C, "loop must be true or false"))?,
    };
    let fps = match p.get("fps") {
        None | Some(Value::Null) => None,
        Some(v) => Some(
            v.as_u64()
                .filter(|f| (1..=MAX_FPS as u64).contains(f))
                .ok_or_else(|| bad(C, format!("fps must be a whole number from 1 to {MAX_FPS}")))? as u32,
        ),
    };
    let new = Clip { name, from, to, looped, fps };
    let clips = s.edit("Clip", |d, _| {
        let mut a = Anim::of(d);
        match a.clips.iter_mut().find(|c| c.name == new.name) {
            Some(c) => *c = new.clone(),
            None => a.clips.push(new.clone()),
        }
        a.store(d);
        Ok(clips_json(&a))
    })?;
    Ok(json!({"clips": clips}))
}

fn clips(s: &mut Session, _: &Value) -> Result<Value> {
    let anim = Anim::of(&s.doc()?.doc);
    Ok(json!({"clips": clips_json(&anim)}))
}

fn export_sheet(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "anim.exportSheet";
    let dir = str_param(p, "dir").ok_or_else(|| bad(C, "dir is required"))?.to_string();
    let doc = s.doc()?.doc.clone();
    let anim = Anim::of(&doc);
    let all = if anim.clips.is_empty() {
        vec![Clip { name: "default".into(), from: 1, to: anim.frames, looped: true, fps: None }]
    } else {
        anim.clips.clone()
    };
    let chosen: Vec<Clip> = match str_param(p, "clip") {
        Some(name) => all.into_iter().filter(|c| c.name == name).collect(),
        None => all,
    };
    if chosen.is_empty() {
        return Err(bad(C, format!("no clip named {}", str_param(p, "clip").unwrap_or_default())));
    }
    for c in &chosen {
        if c.to > anim.frames || c.from > c.to {
            return Err(bad(C, format!("clip {} is outside the timeline's {} frames", c.name, anim.frames)));
        }
    }
    let columns = match p.get("columns") {
        None | Some(Value::Null) => None,
        Some(v) => Some(v.as_u64().filter(|n| *n >= 1).ok_or_else(|| bad(C, "columns must be a whole number from 1 up"))? as usize),
    };
    let godot = p.get("godot").and_then(Value::as_bool).unwrap_or(false);
    std::fs::create_dir_all(&dir).map_err(|e| EngineError::Other(format!("{dir}: {e}")))?;

    let mut files = Vec::new();
    // Per clip: (clip, frame cell width, height, columns) for the Godot resource.
    let mut cells: Vec<(Clip, u32, u32, usize)> = Vec::new();
    for c in &chosen {
        let mut ep = json!({"from": c.from, "to": c.to});
        for key in ["scale", "alpha"] {
            if let Some(v) = p.get(key) {
                ep[key] = v.clone();
            }
        }
        let frames = encode_frames(&doc, &ep)?;
        let mut images = Vec::with_capacity(frames.len());
        for (_, bytes) in &frames {
            let img = image::load_from_memory(bytes).map_err(|e| bad(C, format!("frame: {e}")))?;
            images.push(img.to_rgba8());
        }
        let n = images.len();
        let (fw, fh) = images.first().map(|i| (i.width(), i.height())).ok_or_else(|| bad(C, "a clip has no frames"))?;
        let cols = columns.unwrap_or(n).min(n).max(1);
        let rows = n.div_ceil(cols);
        let (sw, sh) = (cols as u64 * fw as u64, rows as u64 * fh as u64);
        if sw * sh > MAX_SHEET_PIXELS {
            return Err(bad(C, format!("{} sheet is {sw} × {sh} pixels, more than a sheet takes", c.name)));
        }
        let mut sheet = image::RgbaImage::new(sw as u32, sh as u32);
        for (i, img) in images.iter().enumerate() {
            let (x, y) = ((i % cols) as u32 * fw, (i / cols) as u32 * fh);
            image::imageops::replace(&mut sheet, img, x as i64, y as i64);
        }
        let mut png = Vec::new();
        sheet.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).map_err(|e| bad(C, format!("PNG: {e}")))?;
        let fps = c.fps.unwrap_or(anim.fps);
        let meta = json!({"frame_width": fw, "frame_height": fh, "columns": cols, "frames": n, "fps": fps, "loop": c.looped});
        let meta_bytes = serde_json::to_vec_pretty(&meta).map_err(|e| bad(C, format!("json: {e}")))?;
        let png_name = format!("{}.png", c.name);
        let json_name = format!("{}.json", c.name);
        super::fileio::write_file(&join(&dir, &png_name), &png)?;
        super::fileio::write_file(&join(&dir, &json_name), &meta_bytes)?;
        files.push(png_name);
        files.push(json_name);
        cells.push((c.clone(), fw, fh, cols));
    }

    if godot {
        let title = doc.title.trim();
        let base: String = if title.is_empty() {
            "sprite".into()
        } else {
            title.chars().map(|ch| if matches!(ch, '/' | '\\' | '"') || ch.is_control() { '_' } else { ch }).collect()
        };
        let tres_name = format!("{base}.tres");
        super::fileio::write_file(&join(&dir, &tres_name), godot_tres(&cells, &anim).as_bytes())?;
        files.push(tres_name);
    }
    Ok(json!({"dir": dir, "files": files}))
}

fn join(dir: &str, name: &str) -> String {
    std::path::Path::new(dir).join(name).to_string_lossy().into_owned()
}

/// A Godot 4 SpriteFrames resource: one animation per clip, each frame an AtlasTexture region
/// of the clip's sheet (`cells`: clip, frame width, frame height, columns). Frames play at 1 s
/// duration each, so a clip's speed is its fps.
fn godot_tres(cells: &[(Clip, u32, u32, usize)], anim: &Anim) -> String {
    let sub_total: usize = cells.iter().map(|(c, ..)| (c.to - c.from + 1) as usize).sum();
    let mut head = String::new();
    let mut subs = String::new();
    let mut anims = Vec::new();
    let mut k = 0usize;
    for (i, (c, fw, fh, cols)) in cells.iter().enumerate() {
        let ext = format!("{}_png", i + 1);
        head.push_str(&format!("[ext_resource type=\"Texture2D\" path=\"res://{}.png\" id=\"{ext}\"]\n", c.name));
        let n = (c.to - c.from + 1) as usize;
        let mut frames = Vec::with_capacity(n);
        for j in 0..n {
            k += 1;
            let (x, y) = ((j % cols) as u32 * fw, (j / cols) as u32 * fh);
            subs.push_str(&format!(
                "\n[sub_resource type=\"AtlasTexture\" id=\"AtlasTexture_{k}\"]\natlas = ExtResource(\"{ext}\")\nregion = Rect2({x}, {y}, {fw}, {fh})\n"
            ));
            frames.push(format!("{{\n\"duration\": 1.0,\n\"texture\": SubResource(\"AtlasTexture_{k}\")\n}}"));
        }
        let fps = c.fps.unwrap_or(anim.fps);
        anims.push(format!("{{\n\"frames\": [{}],\n\"loop\": {},\n\"name\": &\"{}\",\n\"speed\": {fps}.0\n}}", frames.join(", "), c.looped, c.name));
    }
    let steps = cells.len() + sub_total + 1;
    format!("[gd_resource type=\"SpriteFrames\" load_steps={steps} format=3]\n\n{head}{subs}\n[resource]\nanimations = [{}]\n", anims.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        s
    }

    #[test]
    fn clip_adds_replaces_and_removes_and_persists() {
        let mut s = session();
        s.execute("anim.setFrames", &json!({"frames": 6})).unwrap();
        let r = s.execute("anim.clip", &json!({"name": "walk", "from": 1, "to": 4})).unwrap();
        assert_eq!(r["clips"], json!([{"name": "walk", "from": 1, "to": 4, "loop": true, "fps": 12}]));
        let r = s.execute("anim.clip", &json!({"name": "walk", "from": 2, "to": 5, "loop": false, "fps": 8})).unwrap();
        assert_eq!(r["clips"], json!([{"name": "walk", "from": 2, "to": 5, "loop": false, "fps": 8}]));
        let anim = Anim::of(&s.doc().unwrap().doc);
        assert_eq!(anim.clips.len(), 1, "same name replaces");
        assert!(!anim.clips[0].looped);
        assert_eq!(s.execute("anim.clips", &json!({})).unwrap()["clips"][0]["fps"], 8);
        let r = s.execute("anim.clip", &json!({"name": "walk", "remove": true})).unwrap();
        assert_eq!(r["clips"], json!([]));
        assert!(Anim::of(&s.doc().unwrap().doc).clips.is_empty());
        assert!(s.execute("anim.clip", &json!({"name": "walk", "remove": true})).is_err());
    }

    #[test]
    fn bad_range_errors() {
        let mut s = session();
        s.execute("anim.setFrames", &json!({"frames": 6})).unwrap();
        assert!(s.execute("anim.clip", &json!({"name": "x", "from": 0, "to": 3})).is_err());
        assert!(s.execute("anim.clip", &json!({"name": "x", "from": 1, "to": 7})).is_err());
        assert!(s.execute("anim.clip", &json!({"name": "x", "from": 4, "to": 2})).is_err());
        assert!(s.execute("anim.clip", &json!({"name": "a/b", "from": 1, "to": 2})).is_err());
        assert!(Anim::of(&s.doc().unwrap().doc).clips.is_empty());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn export_sheet_packs_columns_and_writes_godot_resource() {
        let mut s = session();
        s.execute("anim.setFrames", &json!({"frames": 3})).unwrap();
        let dir = std::env::temp_dir().join(format!("7rrek-sheet-{}", std::process::id()));
        let dir_str = dir.to_string_lossy().into_owned();
        let r = s.execute("anim.exportSheet", &json!({"dir": dir_str, "columns": 2, "scale": 0.5, "godot": true})).unwrap();
        assert!(r["files"].as_array().unwrap().iter().any(|f| f == "default.png"));
        let sheet = image::open(dir.join("default.png")).unwrap().to_rgba8();
        let meta: Value = serde_json::from_slice(&std::fs::read(dir.join("default.json")).unwrap()).unwrap();
        assert_eq!(meta["columns"], 2);
        assert_eq!(meta["frames"], 3);
        assert_eq!(sheet.width(), 2 * meta["frame_width"].as_u64().unwrap() as u32);
        assert_eq!(sheet.height(), 2 * meta["frame_height"].as_u64().unwrap() as u32);
        let tres_name = r["files"].as_array().unwrap().iter().filter_map(Value::as_str).find(|f| f.ends_with(".tres")).unwrap().to_string();
        let tres = std::fs::read_to_string(dir.join(tres_name)).unwrap();
        assert!(tres.contains("[gd_resource type=\"SpriteFrames\""), "{tres}");
        assert!(tres.contains("&\"default\""), "{tres}");
        assert!(tres.contains("res://default.png"), "{tres}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn export_sheet_unknown_clip_errors() {
        let mut s = session();
        assert!(s.execute("anim.exportSheet", &json!({"dir": "/nonexistent-7rrek", "clip": "nope"})).is_err());
    }
}
