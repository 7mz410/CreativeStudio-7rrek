//! 7rrek (Creative Studio): P5 scene spec. `scene.build {spec}` turns one JSON description (stage,
//! characters, actions, moves, dialogue, camera) into a scene by running the commands an animator
//! would: scene.setup, character.fromRsm, action.apply, peg.key, anim.soundImport + anim.lipSync,
//! anim.cameraKey. Everything is checked first and runs as one undo step; a failure undoes it all.
//! shortcut: one shot per spec; add `shots` when a film needs cuts.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};

use super::*;

/// Most items of one kind a spec may hold.
const MAX_ITEMS: usize = 1_000;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "scene.build",
            "Build Scene",
            [],
            None,
            "{spec: a scene spec object or the path of a .json file holding one (see scene.specSchema; the files it names are relative to that file)} builds the scene in the open document as one undo step; any error undoes it all and names the field (`actions[2].at`) → {characters: {name: peg}, frames, lines: [{character, at, text}] for captions, skipped: [what was left out and why]}",
            has_doc,
            build
        ),
        cmd!(query "scene.specSchema", "Scene Spec Schema", [], None, "{} the shape of a scene spec, for scripts and AI → {schema}", always, schema),
    ]
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    width: Option<f64>,
    height: Option<f64>,
    fps: Option<u32>,
    frames: Option<u32>,
    #[serde(default)]
    characters: Vec<Character>,
    #[serde(default)]
    actions: Vec<ActionUse>,
    #[serde(default)]
    turns: Vec<Turn>,
    #[serde(default)]
    moves: Vec<Move>,
    stage: Option<Stage>,
    #[serde(default)]
    dialogue: Vec<Line>,
    #[serde(default)]
    camera: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Stage {
    /// Top and bottom colours of a linear gradient filling the artboard.
    background: Option<[String; 2]>,
    #[serde(default)]
    shadows: bool,
    #[serde(default)]
    safe_areas: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Character {
    name: String,
    file: String,
    view: Option<String>,
    #[serde(default)]
    views: Vec<String>,
    #[serde(default)]
    x: f64,
    #[serde(default)]
    y: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Turn {
    character: String,
    frame: u32,
    view: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionUse {
    character: String,
    action: String,
    at: u32,
    #[serde(default = "one")]
    times: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Move {
    character: String,
    from: u32,
    to: u32,
    #[serde(default)]
    dx: f64,
    #[serde(default)]
    dy: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Line {
    character: String,
    sound: String,
    at: u32,
    #[serde(default)]
    text: String,
}

fn one() -> u32 {
    1
}

fn is_hex_colour(c: &str) -> bool {
    c.strip_prefix('#').is_some_and(|h| h.len() == 6 && h.chars().all(|ch| ch.is_ascii_hexdigit()))
}

/// The stage: safe areas, each character's contact shadow, then the background at the very bottom.
/// Returns what was left out.
fn build_stage(
    s: &mut Session,
    spec: &Spec,
    stage: &Stage,
    frames: u32,
    pegs: &HashMap<String, u64>,
    feet: &HashMap<String, Vec<u64>>,
) -> Result<Vec<String>> {
    let mut skipped = Vec::new();
    if stage.safe_areas {
        s.execute("scene.setup", &json!({"safeAreas": true}))?;
    }
    if stage.shadows {
        for c in &spec.characters {
            let Some(&peg) = pegs.get(&c.name) else { continue };
            let doc = super::peg::Pegs::of(&s.doc()?.doc);
            let Some(cp) = doc.pegs.get(peg as usize) else { continue };
            // Measure the feet at rest: the drawing each foot row shows at frame 1, in the art
            // itself (not the posed copy on screen, which the character's peg already moved).
            let rows: Vec<u64> = match feet.get(&c.name) {
                Some(f) if !f.is_empty() => f.clone(),
                _ => doc.pegs.iter().filter(|p| p.name.starts_with(&format!("{}/", c.name))).flat_map(|p| p.rows.clone()).collect(),
            };
            let art = super::anim::unposed(&s.doc()?.doc);
            let anim = super::anim::Anim::of(&art);
            let ids: Vec<NodeId> = rows
                .iter()
                .filter_map(|r| art.node(NodeId(*r)))
                .map(|row| anim.row_frames(row).into_iter().rfind(|(f, _)| *f <= 1).map_or(row.id, |(_, id)| id))
                .collect();
            let Some(b) = art.bounds_of(&ids, false) else {
                skipped.push(format!("{}: nothing to measure, no shadow", c.name));
                continue;
            };
            let w = b.width() * 1.3;
            let h = w * 0.18;
            if !(w.is_finite() && w > 0.0) {
                skipped.push(format!("{}: feet have no width, no shadow", c.name));
                continue;
            }
            let (cx, feet_y) = ((b.x0 + b.x1) / 2.0, b.y1);
            let layer = s.execute("layer.new", &json!({"name": format!("{}/shadow", c.name)}))?["id"]
                .as_u64()
                .ok_or_else(|| bad("scene.build", "no layer id"))?;
            s.execute("layer.setCurrent", &json!({"id": layer}))?;
            let shape = s.execute("shape.ellipse", &json!({"x": cx - w / 2.0, "y": feet_y - h / 2.0, "width": w, "height": h}))?["id"]
                .as_u64()
                .ok_or_else(|| bad("scene.build", "no shape id"))?;
            s.execute("paint.setFill", &json!({"ids": [shape], "color": "#000000"}))?;
            s.execute("paint.setStroke", &json!({"ids": [shape], "none": true}))?;
            s.execute("transparency.set", &json!({"ids": [shape], "opacity": 22}))?;
            to_bottom(s, layer)?;
            // Own peg: follows the character's x every frame, keeps its y from frame 1.
            let shadow = s.execute("peg.add", &json!({"rows": [layer]}))?["index"].as_u64().ok_or_else(|| bad("scene.build", "no peg"))?;
            let rest_y = cp.key_at(1).y;
            for f in 1..=frames {
                s.execute("peg.key", &json!({"peg": shadow, "frame": f, "x": cp.key_at(f).x, "y": rest_y}))?;
            }
        }
    }
    if let Some([top, bottom]) = &stage.background {
        let r = s.doc()?.doc.artboards.first().ok_or_else(|| bad("scene.build", "no artboard"))?.rect;
        let (x0, y0, w, h) = (r.x0, r.y0, r.width(), r.height());
        let layer = s.execute("layer.new", &json!({"name": "background"}))?["id"].as_u64().ok_or_else(|| bad("scene.build", "no layer id"))?;
        s.execute("layer.setCurrent", &json!({"id": layer}))?;
        let rect = s.execute("shape.rectangle", &json!({"x": x0, "y": y0, "width": w, "height": h}))?["id"]
            .as_u64()
            .ok_or_else(|| bad("scene.build", "no shape id"))?;
        s.execute("paint.setStroke", &json!({"ids": [rect], "none": true}))?;
        s.execute(
            "paint.editGradient",
            &json!({"ids": [rect], "stroke": false, "kind": "linear", "stops": [{"offset": 0, "color": top}, {"offset": 1, "color": bottom}]}),
        )?;
        s.execute("paint.setGradientGeom", &json!({"ids": [rect], "stroke": false, "start": [x0 + w / 2.0, y0], "end": [x0 + w / 2.0, y0 + h]}))?;
        to_bottom(s, layer)?;
    }
    Ok(skipped)
}

/// Moves a layer below the bottom one.
fn to_bottom(s: &mut Session, id: u64) -> Result<()> {
    let Some(bottom) = s.doc()?.doc.layers.first().map(|l| l.id.0) else { return Ok(()) };
    if bottom != id {
        s.execute("layer.move", &json!({"id": id, "target": bottom, "place": "below"}))?;
    }
    Ok(())
}

fn schema(_: &mut Session, _: &Value) -> Result<Value> {
    Ok(json!({"schema": {
        "width": "number, stage width in points (optional)",
        "height": "number (optional)",
        "fps": "1–60 (optional)",
        "frames": "the scene's length in frames (optional; at least what the actions need)",
        "characters": [{"name": "text, no /", "file": "a .rsm that passes character.check", "view": "front | 3q | side (default front; walking needs side)", "views": "[\"front\", \"3q\", \"side\"] (optional)", "x": "where it stands, points from where it was drawn", "y": "number"}],
        "actions": [{"character": "a name above", "action": "walk | blink | turn_head | point | idle | a saved action", "at": "first frame", "times": "repeats back to back (default 1)"}],
        "turns": [{"character": "a name above", "frame": "frame from 1", "view": "front | 3q | side"}],
        "moves": [{"character": "name", "from": "frame", "to": "frame", "dx": "points travelled", "dy": "number"}],
        "stage": {"background": "[\"#top\", \"#bottom\"] (optional; a linear gradient, top to bottom, behind everything)", "shadows": "bool (default false): a soft contact shadow under each character's feet, following its x", "safeAreas": "bool (default false): show the safe areas"},
        "dialogue": [{"character": "name", "sound": "WAV, MP3 or Ogg file", "at": "frame it starts", "text": "what is said (kept for captions)"}],
        "camera": [{"frame": "frame", "x": "centre", "y": "centre", "zoom": "1 = the stage", "rotation": "degrees", "ease": "linear | easeIn | easeOut | easeInOut"}]
    }}))
}

fn build(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "scene.build";
    let (raw, base) = match p.get("spec") {
        Some(Value::String(path)) => {
            let bytes = super::fileio::read_file(path)?;
            let v: Value = serde_json::from_slice(&bytes).map_err(|e| bad(C, format!("{path}: {e}")))?;
            (v, Path::new(path).parent().map(Path::to_path_buf))
        }
        Some(v @ Value::Object(_)) => (v.clone(), None),
        _ => return Err(bad(C, "`spec` is required: a scene spec object or the path of a .json file")),
    };
    let spec: Spec = serde_json::from_value(raw).map_err(|e| bad(C, format!("spec: {e}")))?;
    let plan = check(&spec, base.as_deref())?;

    s.begin_named_undo_group(Some("Build Scene".into()));
    let r = run(s, &spec, &plan);
    s.end_undo_group(r.is_err());
    r
}

/// What `check` worked out for `run`: resolved file paths and each action's length.
struct Plan {
    files: Vec<PathBuf>,
    sounds: Vec<PathBuf>,
    lengths: Vec<u32>,
    frames: u32,
}

fn check(spec: &Spec, base: Option<&Path>) -> Result<Plan> {
    const C: &str = "scene.build";
    let err = |at: String, what: &str| bad(C, format!("{at}: {what}"));
    let path = |f: &str| match base {
        Some(b) if Path::new(f).is_relative() => b.join(f),
        _ => PathBuf::from(f),
    };
    let lens = [spec.characters.len(), spec.actions.len(), spec.turns.len(), spec.moves.len(), spec.dialogue.len(), spec.camera.len()];
    if lens.iter().any(|n| *n > MAX_ITEMS) {
        return Err(bad(C, format!("a spec holds at most {MAX_ITEMS} items of each kind")));
    }
    let mut names: HashMap<&str, usize> = HashMap::new();
    let mut files = Vec::new();
    for (i, c) in spec.characters.iter().enumerate() {
        let at = format!("characters[{i}]");
        if c.name.trim().is_empty() || c.name.contains('/') {
            return Err(err(format!("{at}.name"), "a name, without /"));
        }
        if names.insert(c.name.as_str(), i).is_some() {
            return Err(err(format!("{at}.name"), "used twice"));
        }
        if c.view.as_deref().is_some_and(|v| !["front", "3q", "side"].contains(&v)) {
            return Err(err(format!("{at}.view"), "front, 3q or side"));
        }
        for v in &c.views {
            if !["front", "3q", "side"].contains(&v.as_str()) {
                return Err(err(format!("{at}.views"), "front, 3q or side"));
            }
        }
        if !c.x.is_finite() || !c.y.is_finite() {
            return Err(err(at, "x and y must be numbers"));
        }
        let f = path(&c.file);
        if !f.is_file() {
            return Err(err(format!("{at}.file"), &format!("no file at {}", f.display())));
        }
        files.push(f);
    }
    let known = |who: &str, at: String| if names.contains_key(who) { Ok(()) } else { Err(err(at, "no character by that name")) };
    let mut end = 1u32;
    let mut lengths = Vec::new();
    for (i, a) in spec.actions.iter().enumerate() {
        let at = format!("actions[{i}]");
        known(&a.character, format!("{at}.character"))?;
        let len = super::actions_builtin::builtin_action(&a.action).map(|x| x.length.max(1)).unwrap_or(1);
        if a.at < 1 || a.times < 1 || a.times > 100 {
            return Err(err(format!("{at}.at"), "at from 1, times from 1 to 100"));
        }
        end = end.max(a.at.saturating_add(len.saturating_mul(a.times)));
        lengths.push(len);
    }
    for (i, t) in spec.turns.iter().enumerate() {
        let at = format!("turns[{i}]");
        known(&t.character, format!("{at}.character"))?;
        if t.frame < 1 {
            return Err(err(format!("{at}.frame"), "a frame from 1"));
        }
        if !["front", "3q", "side"].contains(&t.view.as_str()) {
            return Err(err(format!("{at}.view"), "front, 3q or side"));
        }
        end = end.max(t.frame);
    }
    for (i, m) in spec.moves.iter().enumerate() {
        let at = format!("moves[{i}]");
        known(&m.character, format!("{at}.character"))?;
        if m.from < 1 || m.to <= m.from || !m.dx.is_finite() || !m.dy.is_finite() {
            return Err(err(at, "from ≥ 1, to after from, dx and dy numbers"));
        }
        end = end.max(m.to);
    }
    if let Some([top, bottom]) = spec.stage.as_ref().and_then(|st| st.background.as_ref()) {
        for c in [top, bottom] {
            if !is_hex_colour(c) {
                return Err(err("stage.background".into(), "two colours, each #rrggbb"));
            }
        }
    }
    let mut sounds = Vec::new();
    for (i, l) in spec.dialogue.iter().enumerate() {
        let at = format!("dialogue[{i}]");
        known(&l.character, format!("{at}.character"))?;
        if l.at < 1 {
            return Err(err(format!("{at}.at"), "from 1"));
        }
        let f = path(&l.sound);
        if !f.is_file() {
            return Err(err(format!("{at}.sound"), &format!("no file at {}", f.display())));
        }
        sounds.push(f);
    }
    for (i, k) in spec.camera.iter().enumerate() {
        if k.get("frame").and_then(Value::as_u64).is_none_or(|f| f < 1) {
            return Err(err(format!("camera[{i}].frame"), "a frame from 1"));
        }
    }
    let frames = spec.frames.unwrap_or(end).max(end.saturating_sub(1)).max(1);
    Ok(Plan { files, sounds, lengths, frames })
}

fn run(s: &mut Session, spec: &Spec, plan: &Plan) -> Result<Value> {
    let at = |field: String| move |e: EngineError| EngineError::Other(format!("scene.build: {field}: {e}"));
    let mut setup = json!({"frames": plan.frames});
    for (k, v) in [("width", spec.width.map(Value::from)), ("height", spec.height.map(Value::from)), ("fps", spec.fps.map(Value::from))] {
        if let Some(v) = v {
            setup[k] = v;
        }
    }
    s.execute("scene.setup", &setup).map_err(at("scene".into()))?;

    let mut pegs: HashMap<String, u64> = HashMap::new();
    let mut mouths: HashMap<String, u64> = HashMap::new();
    // Where each character's whole peg stands, as moves add up.
    let mut place: HashMap<String, (f64, f64)> = HashMap::new();
    // Each character's foot rows, for the shadow's width.
    let mut feet: HashMap<String, Vec<u64>> = HashMap::new();
    for (i, (c, file)) in spec.characters.iter().zip(&plan.files).enumerate() {
        let mut rsm_params = json!({"path": file, "name": c.name});
        if let Some(v) = &c.view {
            rsm_params["view"] = json!(v);
        }
        if !c.views.is_empty() {
            rsm_params["views"] = json!(c.views);
        }
        let r = s.execute("character.fromRsm", &rsm_params).map_err(at(format!("characters[{i}]")))?;
        let peg = r["peg"].as_u64().unwrap_or(0);
        if let Some(m) = r["rows"]["mouth"].as_u64() {
            mouths.insert(c.name.clone(), m);
        }
        feet.insert(c.name.clone(), ["foot_L", "foot_R"].iter().filter_map(|p| r["rows"][*p].as_u64()).collect());
        s.execute("peg.key", &json!({"peg": peg, "frame": 1, "x": c.x, "y": c.y})).map_err(at(format!("characters[{i}]")))?;
        pegs.insert(c.name.clone(), peg);
        place.insert(c.name.clone(), (c.x, c.y));
    }

    // Turns go before actions so a blink after a turn picks that view's drawing.
    for (i, t) in spec.turns.iter().enumerate() {
        s.execute("character.view", &json!({"character": t.character, "frame": t.frame, "view": t.view})).map_err(at(format!("turns[{i}]")))?;
    }

    let mut skipped: Vec<String> = Vec::new();
    // Saved actions are only known now that the document is at hand; built-ins were sized in check.
    let listed = s.execute("action.list", &json!({}))?;
    let saved: HashMap<&str, u32> = listed["actions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| Some((a["name"].as_str()?, a["length"].as_u64().and_then(|n| u32::try_from(n).ok())?.max(1))))
        .collect();
    for (i, (a, len)) in spec.actions.iter().zip(&plan.lengths).enumerate() {
        let len = saved.get(a.action.as_str()).copied().unwrap_or(*len);
        for n in 0..a.times {
            let r = s
                .execute("action.apply", &json!({"character": a.character, "action": a.action, "at": a.at.saturating_add(n.saturating_mul(len))}))
                .map_err(at(format!("actions[{i}]")))?;
            for item in r["skipped"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                let note = format!("actions[{i}]: {item}");
                if !skipped.contains(&note) {
                    skipped.push(note);
                }
            }
        }
    }

    let mut moves: Vec<(usize, &Move)> = spec.moves.iter().enumerate().collect();
    moves.sort_by_key(|(_, m)| m.from);
    for (i, m) in moves {
        let (Some(&peg), Some(&(x, y))) = (pegs.get(&m.character), place.get(&m.character)) else { continue };
        s.execute("peg.key", &json!({"peg": peg, "frame": m.from, "x": x, "y": y})).map_err(at(format!("moves[{i}]")))?;
        let to = (x + m.dx, y + m.dy);
        s.execute("peg.key", &json!({"peg": peg, "frame": m.to, "x": to.0, "y": to.1})).map_err(at(format!("moves[{i}]")))?;
        place.insert(m.character.clone(), to);
    }

    if let Some(stage) = &spec.stage {
        let more = build_stage(s, spec, stage, plan.frames, &pegs, &feet).map_err(at("stage".into()))?;
        skipped.extend(more);
    }

    for (i, (l, file)) in spec.dialogue.iter().zip(&plan.sounds).enumerate() {
        let r = s.execute("anim.soundImport", &json!({"path": file, "start": l.at})).map_err(at(format!("dialogue[{i}]")))?;
        match mouths.get(&l.character) {
            Some(&row) => {
                let ls = s.execute("anim.lipSync", &json!({"row": row, "sound": r["index"]})).map_err(at(format!("dialogue[{i}]")))?;
                if ls["missing"].as_array().is_some_and(|m| !m.is_empty()) {
                    skipped.push(format!("dialogue[{i}]: mouth shapes without a drawing {}", ls["missing"]));
                }
            }
            None => skipped.push(format!("dialogue[{i}]: {} has no mouth, no lip sync", l.character)),
        }
    }

    for (i, k) in spec.camera.iter().enumerate() {
        s.execute("anim.cameraKey", k).map_err(at(format!("camera[{i}]")))?;
    }

    let frames = super::anim::Anim::of(&s.doc()?.doc).frames;
    let characters: HashMap<&String, &u64> = pegs.iter().collect();
    let lines: Vec<Value> = spec.dialogue.iter().map(|l| json!({"character": l.character, "at": l.at, "text": l.text})).collect();
    Ok(json!({"characters": characters, "frames": frames, "lines": lines, "skipped": skipped}))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A full contract file with a mouth chart in front, written by commands.
    fn contract_file(path: &Path) {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 800.0, "height": 800.0})).unwrap();
        for v in super::super::character_contract::REQUIRED_VIEWS {
            let l = s.execute("layer.new", &json!({"name": v})).unwrap()["id"].as_u64().unwrap();
            s.execute("layer.setCurrent", &json!({"id": l})).unwrap();
            for spec in super::super::character_contract::PARTS.iter().filter(|p| p.required_full || p.name == "mouth") {
                let mut ids = vec![
                    s.execute("shape.rectangle", &json!({"x": 300.0, "y": 300.0, "width": 20.0, "height": 40.0})).unwrap()["id"].as_u64().unwrap(),
                ];
                if spec.name == "mouth" && *v == "front" {
                    for m in ["A", "B", "C", "D", "E", "F", "G", "H", "X"] {
                        let a = s.execute("shape.ellipse", &json!({"x": 300.0, "y": 300.0, "width": 10.0, "height": 5.0})).unwrap()["id"]
                            .as_u64()
                            .unwrap();
                        s.execute("object.setProps", &json!({"id": a, "name": format!("mouth.{m}")})).unwrap();
                        ids.push(a);
                    }
                }
                s.execute("select.set", &json!({"ids": ids})).unwrap();
                let g = s.execute("object.group", &json!({})).unwrap()["id"].as_u64().unwrap();
                s.execute("object.setProps", &json!({"id": g, "name": spec.name})).unwrap();
            }
        }
        s.execute("document.save", &json!({"path": path})).unwrap();
    }

    /// 1 s of a 220 Hz tone, 48 kHz 16-bit mono WAV.
    fn tone(path: &Path) {
        let n = 48_000u32;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + n * 2).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&48_000u32.to_le_bytes());
        b.extend_from_slice(&96_000u32.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(n * 2).to_le_bytes());
        for i in 0..n {
            let v = ((i as f64 * 220.0 * std::f64::consts::TAU / 48_000.0).sin() * 12_000.0) as i16;
            b.extend_from_slice(&v.to_le_bytes());
        }
        std::fs::write(path, b).unwrap();
    }

    fn spec(dir: &Path) -> Value {
        json!({
            "width": 640, "height": 360, "fps": 24, "frames": 120,
            "characters": [{"name": "ana", "file": dir.join("c.rsm"), "view": "side", "x": -200}],
            "actions": [{"character": "ana", "action": "walk", "at": 1, "times": 2}, {"character": "ana", "action": "blink", "at": 60}],
            "moves": [{"character": "ana", "from": 1, "to": 48, "dx": 200}],
            "dialogue": [{"character": "ana", "sound": dir.join("line.wav"), "at": 72, "text": "marhaba"}],
            "camera": [{"frame": 1, "zoom": 1}, {"frame": 120, "zoom": 1.4, "ease": "easeInOut"}]
        })
    }

    #[test]
    fn a_spec_builds_a_five_second_shot_with_sound() {
        let dir = std::env::temp_dir().join(format!("scene-p5-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        contract_file(&dir.join("c.rsm"));
        tone(&dir.join("line.wav"));
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let r = s.execute("scene.build", &json!({"spec": spec(&dir)})).unwrap();
        assert_eq!(r["frames"], 120, "{r}");
        let video = dir.join("shot.mp4");
        let v = s.execute("anim.exportVideo", &json!({"path": video, "format": "mp4", "scale": 0.25})).unwrap();
        assert_eq!(v["frames"], 120);
        assert_eq!(v["sound"], true, "{v}");
        // One undo takes the whole scene back.
        s.execute("edit.undo", &json!({})).unwrap();
        assert!(s.doc().unwrap().doc.layers.iter().all(|l| !l.name.as_deref().unwrap_or("").starts_with("ana/")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stage_puts_the_background_at_the_bottom_and_shadows_follow_the_peg() {
        let dir = std::env::temp_dir().join(format!("scene-p5-stage-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        contract_file(&dir.join("c.rsm"));
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let mut sp = spec(&dir);
        sp["actions"] = json!([{"character": "ana", "action": "walk", "at": 1, "times": 1}]);
        sp["dialogue"] = json!([]);
        sp["camera"] = json!([]);
        sp["moves"] = json!([{"character": "ana", "from": 1, "to": 12, "dx": 100}]);
        sp["stage"] = json!({"background": ["#ffffff", "#336699"], "shadows": true, "safeAreas": true});
        let r = s.execute("scene.build", &json!({"spec": sp})).unwrap();
        let d = &s.doc().unwrap().doc;
        assert_eq!(d.layers[0].name.as_deref(), Some("background"), "{r}");
        let bg = s.execute("document.node", &json!({"id": d.layers[0].children().unwrap()[0].id.0, "summary": true})).unwrap();
        assert_eq!(bg["fill"], "Linear gradient", "the gradient fills the background: {bg}");
        let d = &s.doc().unwrap().doc;
        let shadow = d.layers.iter().find(|l| l.name.as_deref() == Some("ana/shadow")).expect("shadow row").id.0;
        let pegs = super::super::peg::Pegs::of(d);
        let ch = &pegs.pegs[r["characters"]["ana"].as_u64().unwrap() as usize];
        let sh = pegs.pegs.iter().find(|p| p.rows.contains(&shadow)).expect("shadow peg");
        for f in [1, 12] {
            assert!((sh.key_at(f).x - ch.key_at(f).x).abs() < 0.01, "frame {f}: {} vs {}", sh.key_at(f).x, ch.key_at(f).x);
        }
        assert!((sh.key_at(1).y - sh.key_at(12).y).abs() < 0.01, "shadow y stays constant");
        assert!((sh.key_at(1).y - ch.key_at(1).y).abs() < 0.01, "shadow y is the peg's frame-1 y");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bad_field_names_itself_and_changes_nothing() {
        let dir = std::env::temp_dir().join(format!("scene-p5-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        contract_file(&dir.join("c.rsm"));
        tone(&dir.join("line.wav"));
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let layers = s.doc().unwrap().doc.layers.len();
        let mut bad = spec(&dir);
        bad["actions"][1]["character"] = json!("bob");
        let e = s.execute("scene.build", &json!({"spec": bad})).unwrap_err().to_string();
        assert!(e.contains("actions[1].character"), "{e}");
        bad["actions"][1]["character"] = json!("ana");
        bad["dialogue"][0]["sound"] = json!(dir.join("missing.wav"));
        let e = s.execute("scene.build", &json!({"spec": bad})).unwrap_err().to_string();
        assert!(e.contains("dialogue[0].sound"), "{e}");
        assert_eq!(s.doc().unwrap().doc.layers.len(), layers);
        assert!(s.execute("scene.build", &json!({})).is_err());
        assert!(s.execute("scene.build", &json!({"spec": {"nonsense": 1}})).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
