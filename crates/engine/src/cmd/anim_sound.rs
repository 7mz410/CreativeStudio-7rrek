//! 7rrek (Creative Studio): sound commands.

use std::path::Path;

use serde_json::{Value, json};
use vectorcraft_audio::{Clip, Pcm, RATE, decode, mix};
use vectorcraft_doc::Document;

use super::anim::{Anim, Sound, frame_param};
use super::*;
use crate::EngineError;

/// Loudest gain a sound clip takes (its file's level times this).
const MAX_GAIN: f32 = 4.0;

fn start_param(p: &Value, anim: &Anim, cmd: &str) -> Result<u32> {
    match p.get("start") {
        None | Some(Value::Null) => Ok(anim.current),
        Some(_) => frame_param(&json!({ "frame": p["start"] }), anim, cmd),
    }
}

fn gain_param(p: &Value, cmd: &str) -> Result<Option<f32>> {
    match p.get("gain") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_f64()
            .map(|g| g as f32)
            .filter(|g| g.is_finite() && (0.0..=MAX_GAIN).contains(g))
            .map(Some)
            .ok_or_else(|| bad(cmd, "gain must be a number from 0 to 4")),
    }
}

fn name_param(p: &Value, cmd: &str) -> Result<Option<String>> {
    match p.get("name") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            v.as_str().filter(|n| !n.trim().is_empty()).map(|n| Some(n.to_string())).ok_or_else(|| bad(cmd, "name must be a non-empty string"))
        }
    }
}

fn index_param(p: &Value, len: usize, cmd: &str) -> Result<usize> {
    p.get("index")
        .and_then(Value::as_u64)
        .map(|i| i as usize)
        .filter(|i| *i < len)
        .ok_or_else(|| bad(cmd, format!("index must be a sound from 0 to {}", len.saturating_sub(1))))
}

/// `path`'s file, decoded. A missing or unreadable file, or one that isn't a sound, is an error.
fn decode_file(path: &str) -> Result<Pcm> {
    let bytes = super::fileio::read_file(path)?;
    let ext = Path::new(path).extension().and_then(|e| e.to_str());
    decode(&bytes, ext).map_err(|e| EngineError::Other(format!("{path}: not a sound file ({e})")))
}

/// A decoded sound's length in seconds.
fn secs(pcm: &Pcm) -> f64 {
    (pcm.samples.len() / 2) as f64 / RATE as f64
}

fn file_name(path: &str) -> String {
    Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or("sound").to_string()
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "anim.soundImport",
            "Import Sound…",
            [],
            None,
            "{path, start?: frame (default: the current frame), gain?: 0–4 (default 1), name?: (default: the file name)} adds a sound clip to the timeline; the file must be a WAV, MP3 or Ogg → {index, name, start, gain, seconds}",
            has_doc,
            sound_import
        ),
        cmd!(
            "anim.soundSet",
            "Sound Options",
            [],
            None,
            "{index, start?: frame, gain?: 0–4, name?} changes a sound clip → {index, name, start, gain, seconds: number, or null when its file can't be read now}",
            has_doc,
            sound_set
        ),
        cmd!(
            "anim.soundRemove",
            "Remove Sound",
            [],
            None,
            "{index} removes a sound clip from the timeline; its file is untouched → {index}",
            has_doc,
            sound_remove
        ),
        cmd!(query "anim.soundInfo", "Sounds", [], None, "{} → {sounds: [{index, name, path, start, gain}]}", has_doc, sound_info),
    ]
}

fn sound_import(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "anim.soundImport";
    let path = str_param(p, "path").filter(|x| !x.is_empty()).ok_or_else(|| bad(C, "path is required"))?.to_string();
    let anim = Anim::of(&s.doc()?.doc);
    let start = start_param(p, &anim, C)?;
    let gain = gain_param(p, C)?.unwrap_or(1.0);
    let name = name_param(p, C)?.unwrap_or_else(|| file_name(&path));
    let pcm = decode_file(&path)?;
    let sound = Sound { name, path, start, gain };
    let index = s.edit("Import Sound", |d, _| {
        let mut a = Anim::of(d);
        a.sounds.push(sound.clone());
        a.store(d);
        Ok(a.sounds.len() - 1)
    })?;
    Ok(json!({"index": index, "name": sound.name, "start": start, "gain": gain, "seconds": secs(&pcm)}))
}

fn sound_set(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "anim.soundSet";
    let anim = Anim::of(&s.doc()?.doc);
    let i = index_param(p, anim.sounds.len(), C)?;
    let start = p.get("start").filter(|v| !v.is_null()).map(|_| start_param(p, &anim, C)).transpose()?;
    let gain = gain_param(p, C)?;
    let name = name_param(p, C)?;
    let sound = s.edit("Sound Options", |d, _| {
        let mut a = Anim::of(d);
        let sd = a.sounds.get_mut(i).ok_or_else(|| bad(C, "no such sound"))?;
        if let Some(v) = start {
            sd.start = v;
        }
        if let Some(v) = gain {
            sd.gain = v;
        }
        if let Some(v) = &name {
            sd.name = v.clone();
        }
        let out = sd.clone();
        a.store(d);
        Ok(out)
    })?;
    // The length is for the user to see; a file that has moved since import shows as null.
    let seconds = decode_file(&sound.path).ok().map(|pcm| secs(&pcm));
    Ok(json!({"index": i, "name": sound.name, "start": sound.start, "gain": sound.gain, "seconds": seconds}))
}

fn sound_remove(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "anim.soundRemove";
    let anim = Anim::of(&s.doc()?.doc);
    let i = index_param(p, anim.sounds.len(), C)?;
    s.edit("Remove Sound", |d, _| {
        let mut a = Anim::of(d);
        a.sounds.remove(i);
        a.store(d);
        Ok(())
    })?;
    Ok(json!({"index": i}))
}

fn sound_info(s: &mut Session, _: &Value) -> Result<Value> {
    let anim = Anim::of(&s.doc()?.doc);
    let sounds: Vec<Value> =
        anim.sounds.iter().enumerate().map(|(i, c)| json!({"index": i, "name": c.name, "path": c.path, "start": c.start, "gain": c.gain})).collect();
    Ok(json!({"sounds": sounds}))
}

/// The timeline's sounds mixed over frames `from` to `to` (both inclusive, 1-based) at `fps` →
/// interleaved stereo at 48 kHz, or None when the timeline has no sounds. A sound whose file has
/// gone or no longer decodes is left out with a warning, so the export still renders.
pub fn mix_for_export(doc: &Document, from: u32, to: u32, fps: u32) -> Result<Option<Vec<f32>>> {
    if fps == 0 {
        return Err(EngineError::Other("frame rate must be at least 1".into()));
    }
    let anim = Anim::of(doc);
    if anim.sounds.is_empty() {
        return Ok(None);
    }
    let loaded: Vec<(Pcm, f64, f32)> = anim
        .sounds
        .iter()
        .filter_map(|c| match decode_file(&c.path) {
            Ok(pcm) => Some((pcm, ((c.start.max(1) - 1) as f64) / fps as f64, c.gain)),
            Err(e) => {
                log::warn!("sound `{}` left out of the export: {e}", c.name);
                None
            }
        })
        .collect();
    let clips: Vec<Clip> = loaded.iter().map(|(pcm, start, gain)| Clip { pcm, start: *start, gain: *gain }).collect();
    let from = from.max(1);
    Ok(Some(mix(&clips, (from - 1) as f64 / fps as f64, to as f64 / fps as f64)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s
    }

    /// A 0.5 s stereo 16-bit 48 kHz WAV of a steady level.
    fn wav() -> Vec<u8> {
        let frames = 24_000u32;
        let data_len = frames * 4;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + data_len).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&48_000u32.to_le_bytes());
        b.extend_from_slice(&(48_000u32 * 4).to_le_bytes());
        b.extend_from_slice(&4u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&data_len.to_le_bytes());
        for _ in 0..frames * 2 {
            b.extend_from_slice(&8_000i16.to_le_bytes());
        }
        b
    }

    fn anim(s: &Session) -> Anim {
        Anim::of(&s.doc().unwrap().doc)
    }

    #[test]
    fn sounds_import_edit_undo_remove_and_mix() {
        let d = std::env::temp_dir().join(format!("vc-anim-sound-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let file = d.join("tone.wav");
        std::fs::write(&file, wav()).unwrap();
        let path = file.to_str().unwrap();
        let mut s = session();

        // Import at frame 3 of 12 fps: the clip starts at 2/12 s.
        s.execute("anim.setFps", &json!({"fps": 12})).unwrap();
        let got = s.execute("anim.soundImport", &json!({"path": path, "start": 3})).unwrap();
        assert_eq!(got["index"], 0);
        assert_eq!(got["name"], "tone.wav");
        assert_eq!(got["start"], 3);
        assert_eq!(got["gain"].as_f64(), Some(1.0));
        assert_eq!(got["seconds"].as_f64(), Some(0.5));

        let info = s.execute("anim.soundInfo", &json!({})).unwrap();
        assert_eq!(info["sounds"].as_array().unwrap().len(), 1);
        assert_eq!(info["sounds"][0]["path"], path);
        assert_eq!(info["sounds"][0]["start"], 3);

        // Gain change, then undo brings the old gain back.
        let set = s.execute("anim.soundSet", &json!({"index": 0, "gain": 2})).unwrap();
        assert_eq!(set["gain"].as_f64(), Some(2.0));
        assert_eq!(anim(&s).sounds[0].gain, 2.0);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(anim(&s).sounds[0].gain, 1.0);

        // Bad input changes nothing.
        assert!(s.execute("anim.soundImport", &json!({"path": d.join("missing.wav").to_str().unwrap()})).is_err());
        let junk = d.join("junk.wav");
        std::fs::write(&junk, b"not audio at all").unwrap();
        assert!(s.execute("anim.soundImport", &json!({"path": junk.to_str().unwrap()})).is_err());
        assert!(s.execute("anim.soundImport", &json!({"path": path, "start": 99})).is_err());
        assert!(s.execute("anim.soundSet", &json!({"index": 5, "gain": 1})).is_err());
        assert!(s.execute("anim.soundSet", &json!({"index": 0, "gain": 5})).is_err());
        assert_eq!(anim(&s).sounds.len(), 1);

        // Mix over frames 1..=12: silence before 2/12 s, the clip after.
        let mixed = mix_for_export(&s.doc().unwrap().doc, 1, 12, 12).unwrap().unwrap();
        assert_eq!(mixed.len(), 48_000 * 2);
        assert!(mixed[..16_000].iter().all(|v| *v == 0.0));
        assert!(mixed[16_000..].iter().any(|v| *v != 0.0));

        // A sound whose file has gone is left out of the export as silence.
        std::fs::rename(&file, d.join("moved.wav")).unwrap();
        let gone = mix_for_export(&s.doc().unwrap().doc, 1, 12, 12).unwrap().unwrap();
        assert_eq!(gone.len(), 48_000 * 2);
        assert!(gone.iter().all(|v| *v == 0.0));
        let set = s.execute("anim.soundSet", &json!({"index": 0, "name": "gone"})).unwrap();
        assert!(set["seconds"].is_null());

        s.execute("anim.soundRemove", &json!({"index": 0})).unwrap();
        assert!(anim(&s).sounds.is_empty());
        assert!(mix_for_export(&s.doc().unwrap().doc, 1, 12, 12).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&d);
    }
}
