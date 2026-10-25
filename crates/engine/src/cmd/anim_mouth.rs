//! 7rrek (Creative Studio): lip sync, as in Harmony's mouth chart. A row's mouth chart names
//! which of its drawings shows each Preston Blair mouth shape (`A`–`H`, `X`); mapping a shape to a
//! frame exposes that drawing there (`Anim.exposures`, see anim_draw.rs), by hand (`anim.mouth`)
//! or for every frame of a sound clip (`anim.lipSync`).

use std::collections::BTreeMap;

use serde_json::json;
use vectorcraft_audio::{MOUTHS, mouths};
use vectorcraft_doc::Document;

use super::anim::{Anim, frame_param, row_param, show};
use super::anim_sound::sound_pcm;
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "anim.mouthChart",
            "Set Mouth Shape Drawing",
            [],
            None,
            "{row?: layer id (default: the current layer's row), shape: A–H or X, drawing?: id of one of the row's drawings; none clears it (the drawing named after the shape is used)} → {chart}",
            has_doc,
            mouth_chart
        ),
        cmd!(
            query "anim.mouths",
            "Mouth Chart",
            [],
            None,
            "{row?: layer id (default: the current layer's row)} the drawing each mouth shape shows → {row, chart: {shape: drawing id}}",
            has_doc,
            chart_query
        ),
        cmd!(
            "anim.mouth",
            "Set Mouth",
            [],
            None,
            "{row?: layer id (default: the current layer's row), frame?: (default: the current frame), shape: A–H or X} exposes the shape's drawing from the frame on → {frame, shape, drawing}",
            has_doc,
            mouth
        ),
        cmd!(
            "anim.lipSync",
            "Lip Sync from Sound",
            [],
            None,
            "{row?: layer id (default: the current layer's row), sound?: index of the sound clip (default 0)} guesses a mouth shape per frame of the clip and exposes the chart's drawings over the frames it plays (replacing the exposures there) → {keys: [{frame, shape}], missing: [shapes with no drawing]}",
            has_doc,
            lip_sync
        ),
    ]
}

fn shape_param(p: &Value, cmd: &str) -> Result<char> {
    let shape = p.get("shape").and_then(Value::as_str).unwrap_or_default().trim().to_ascii_uppercase();
    match shape.chars().collect::<Vec<_>>()[..] {
        [c] if MOUTHS.contains(&c) => Ok(c),
        _ => Err(bad(cmd, "`shape` must be one of A–H or X")),
    }
}

/// Each shape's drawing on `row`: the chart's own entry, else the row's drawing named after it.
fn chart(d: &Document, anim: &Anim, row: NodeId) -> BTreeMap<char, u64> {
    let keys = d.node(row).map(|n| anim.row_keys(n)).unwrap_or_default();
    let own = anim.mouths.get(&row.0);
    MOUTHS
        .iter()
        .filter_map(|&m| {
            let set = own.and_then(|c| c.get(&m)).copied().filter(|id| keys.iter().any(|(_, k)| k.0 == *id));
            let named = || {
                keys.iter()
                    .find(|(_, k)| d.node(*k).is_some_and(|n| n.name.as_deref().is_some_and(|x| x.trim().eq_ignore_ascii_case(&m.to_string()))))
                    .map(|(_, k)| k.0)
            };
            set.or_else(named).map(|id| (m, id))
        })
        .collect()
}

fn chart_json(c: &BTreeMap<char, u64>) -> Value {
    json!(c.iter().map(|(m, id)| (m.to_string(), json!(id))).collect::<serde_json::Map<_, _>>())
}

fn mouth_chart(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "anim.mouthChart";
    let row = row_param(s, p, C)?;
    let shape = shape_param(p, C)?;
    let d = &s.doc()?.doc;
    let anim = Anim::of(d);
    let drawing = match p.get("drawing") {
        None | Some(Value::Null) => None,
        Some(v) => {
            let id = v.as_u64().ok_or_else(|| bad(C, "`drawing` must be a layer id"))?;
            let row_node = d.node(row).ok_or_else(|| bad(C, "no such layer"))?;
            if !anim.row_keys(row_node).iter().any(|(_, k)| k.0 == id) {
                return Err(bad(C, "`drawing` must be one of the row's drawings (its keyframe sublayers)"));
            }
            Some(id)
        }
    };
    s.edit("Set Mouth Shape Drawing", |d, _| {
        let mut a = Anim::of(d);
        let c = a.mouths.entry(row.0).or_default();
        match drawing {
            Some(id) => c.insert(shape, id),
            None => c.remove(&shape),
        };
        a.mouths.retain(|_, c| !c.is_empty());
        a.store(d);
        Ok(())
    })?;
    let d = &s.doc()?.doc;
    Ok(json!({"chart": chart_json(&chart(d, &Anim::of(d), row))}))
}

fn chart_query(s: &mut Session, p: &Value) -> Result<Value> {
    let row = row_param(s, p, "anim.mouths")?;
    let d = &s.doc()?.doc;
    Ok(json!({"row": row.0, "chart": chart_json(&chart(d, &Anim::of(d), row))}))
}

fn mouth(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "anim.mouth";
    let row = row_param(s, p, C)?;
    let shape = shape_param(p, C)?;
    let d = &s.doc()?.doc;
    let anim = Anim::of(d);
    let frame = frame_param(p, &anim, C)?;
    let drawing = *chart(d, &anim, row)
        .get(&shape)
        .ok_or_else(|| bad(C, format!("no drawing for mouth {shape}: name one {shape} or set it in the mouth chart")))?;
    s.edit("Set Mouth", |d, _| {
        let mut a = Anim::of(d);
        a.exposures.entry(row.0).or_default().insert(frame, drawing);
        a.store(d);
        Ok(())
    })?;
    let current = Anim::of(&s.doc()?.doc).current;
    show(s, current)?;
    Ok(json!({"frame": frame, "shape": shape.to_string(), "drawing": drawing}))
}

fn lip_sync(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "anim.lipSync";
    let row = row_param(s, p, C)?;
    let d = &s.doc()?.doc;
    let anim = Anim::of(d);
    let index = match p.get("sound") {
        None | Some(Value::Null) => 0,
        Some(v) => v.as_u64().ok_or_else(|| bad(C, "`sound` must be a sound clip index"))? as usize,
    };
    let sound = anim.sounds.get(index).ok_or_else(|| bad(C, "no such sound clip: import a sound first"))?;
    let shapes = mouths(&sound_pcm(d, sound)?, anim.fps);
    let chart = chart(d, &anim, row);
    if chart.is_empty() {
        return Err(bad(C, "this layer has no mouth drawings: name its drawings A–H and X, or set them in the mouth chart"));
    }
    // One key where the shape changes, over the frames the clip plays inside the timeline.
    let mut keys: Vec<(u32, char)> = Vec::new();
    for (i, m) in shapes.iter().enumerate() {
        let frame = sound.start.saturating_add(i as u32);
        if frame > anim.frames {
            break;
        }
        if keys.last().is_none_or(|(_, last)| last != m) {
            keys.push((frame, *m));
        }
    }
    let end = sound.start.saturating_add(shapes.len() as u32);
    let mut missing: Vec<String> = keys.iter().filter(|(_, m)| !chart.contains_key(m)).map(|(_, m)| m.to_string()).collect();
    missing.sort();
    missing.dedup();
    let start = sound.start;
    s.edit("Lip Sync from Sound", |d, _| {
        let mut a = Anim::of(d);
        let e = a.exposures.entry(row.0).or_default();
        e.retain(|f, _| !(start..end).contains(f));
        // A shape with no drawing holds the drawing before it.
        e.extend(keys.iter().filter_map(|(f, m)| chart.get(m).map(|id| (*f, *id))));
        a.exposures.retain(|_, e| !e.is_empty());
        a.store(d);
        Ok(())
    })?;
    let current = Anim::of(&s.doc()?.doc).current;
    show(s, current)?;
    let keys: Vec<Value> = keys.iter().map(|(f, m)| json!({"frame": f, "shape": m.to_string()})).collect();
    Ok(json!({"keys": keys, "missing": missing}))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A document with blank drawings named `names` at frames 1, 2, 3…; their ids.
    fn drawings(s: &mut Session, names: &[&str]) -> Vec<u64> {
        s.execute("file.new", &json!({})).unwrap();
        names
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let id = s.execute("anim.blankKeyframe", &json!({"frame": i + 1})).unwrap()["id"].as_u64().unwrap();
                s.execute("layer.setProps", &json!({"ids": [id], "name": n})).unwrap();
                id
            })
            .collect()
    }

    #[test]
    fn hand_mapped_chart_round_trips() {
        let mut s = Session::new();
        let ids = drawings(&mut s, &["A", "open", "X"]);
        s.execute("anim.mouthChart", &json!({"shape": "d", "drawing": ids[1]})).unwrap();
        let c = s.execute("anim.mouths", &json!({})).unwrap()["chart"].clone();
        assert_eq!(c, json!({"A": ids[0], "D": ids[1], "X": ids[2]}));
        s.execute("anim.mouth", &json!({"frame": 10, "shape": "D"})).unwrap();
        let doc = &s.doc().unwrap().doc;
        let back = vectorcraft_format::load(&vectorcraft_format::save(doc, false)).unwrap();
        assert_eq!(Anim::of(&back).mouths, Anim::of(doc).mouths);
        assert_eq!(chart_json(&chart(&back, &Anim::of(&back), NodeId(Anim::of(&back).mouths.keys().next().copied().unwrap()))), c);
        let a = Anim::of(&back);
        assert_eq!(a.exposures.values().next().unwrap().get(&10), Some(&ids[1]));
        assert!(s.execute("anim.mouth", &json!({"shape": "F"})).is_err(), "F has no drawing");
        assert!(s.execute("anim.mouth", &json!({"shape": "Q"})).is_err());
    }

    /// 1 s of silence then 1 s of a loud 220 Hz tone, as a 48 kHz 16-bit stereo WAV.
    fn wav() -> Vec<u8> {
        let rate = vectorcraft_audio::RATE;
        let frames: Vec<i16> = (0..2 * rate)
            .map(|i| if i < rate { 0.0 } else { (i as f32 * 220.0 * std::f32::consts::TAU / rate as f32).sin() * 0.8 })
            .map(|v| (v * i16::MAX as f32) as i16)
            .collect();
        let data: Vec<u8> = frames.iter().flat_map(|v| [v.to_le_bytes(), v.to_le_bytes()].concat()).collect();
        let mut w = Vec::new();
        w.extend(b"RIFF");
        w.extend((36 + data.len() as u32).to_le_bytes());
        w.extend(b"WAVEfmt ");
        w.extend(16u32.to_le_bytes());
        w.extend(1u16.to_le_bytes());
        w.extend(2u16.to_le_bytes());
        w.extend(rate.to_le_bytes());
        w.extend((rate * 4).to_le_bytes());
        w.extend(4u16.to_le_bytes());
        w.extend(16u16.to_le_bytes());
        w.extend(b"data");
        w.extend((data.len() as u32).to_le_bytes());
        w.extend(data);
        w
    }

    #[test]
    fn a_two_second_clip_yields_mouth_keys() {
        let mut s = Session::new();
        let names: Vec<String> = MOUTHS.iter().map(char::to_string).collect();
        let ids = drawings(&mut s, &names.iter().map(String::as_str).collect::<Vec<_>>());
        let path = std::env::temp_dir().join(format!("lipsync-{}.wav", std::process::id()));
        std::fs::write(&path, wav()).unwrap();
        s.execute("anim.soundImport", &json!({"path": path, "start": 1})).unwrap();
        let _ = std::fs::remove_file(&path);
        let out = s.execute("anim.lipSync", &json!({})).unwrap();
        let keys = out["keys"].as_array().unwrap();
        assert_eq!(keys[0], json!({"frame": 1, "shape": "X"}), "silence first");
        assert!(keys.len() >= 2 && keys.iter().any(|k| k["shape"] != "X"), "the tone opens the mouth: {keys:?}");
        assert_eq!(out["missing"], json!([]));
        let x = ids[MOUTHS.iter().position(|m| *m == 'X').unwrap()];
        let a = Anim::of(&s.doc().unwrap().doc);
        assert_eq!(a.exposures.values().next().unwrap().get(&1), Some(&x));
    }
}
