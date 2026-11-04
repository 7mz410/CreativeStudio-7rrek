//! Lip sync through Rhubarb Lip Sync (MIT, github.com/DanielSWolf/rhubarb-lip-sync), run as a
//! separate program. Its phonetic recognizer works on any language, Arabic included; its
//! PocketSphinx recognizer reads English words. Rhubarb's mouth shapes are the same Preston Blair
//! set as [`crate::MOUTHS`].

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{MOUTHS, Pcm, RATE};

/// How Rhubarb recognizes speech.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recognizer {
    /// Sounds and syllables, independent of language.
    Phonetic,
    /// English words (PocketSphinx), more precise for English.
    English,
}

impl Recognizer {
    fn arg(self) -> &'static str {
        match self {
            Self::Phonetic => "phonetic",
            Self::English => "pocketSphinx",
        }
    }
}

/// The environment variable that points at a Rhubarb executable.
pub const RHUBARB_ENV: &str = "RHUBARB_PATH";

const EXE: &str = if cfg!(windows) { "rhubarb.exe" } else { "rhubarb" };

/// Finds the Rhubarb executable: [`RHUBARB_ENV`], then next to this program (also `rhubarb/` and,
/// in a macOS bundle, `../Resources/`), then `PATH`.
pub fn rhubarb_path() -> Option<PathBuf> {
    if cfg!(target_arch = "wasm32") {
        return None;
    }
    if let Some(p) = std::env::var_os(RHUBARB_ENV).map(PathBuf::from) {
        return p.is_file().then_some(p);
    }
    let beside = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf));
    let mut dirs: Vec<PathBuf> = beside.iter().flat_map(|d| [d.clone(), d.join("rhubarb"), d.join("../Resources/rhubarb")]).collect();
    dirs.extend(std::env::var_os("PATH").iter().flat_map(std::env::split_paths));
    dirs.into_iter().map(|d| d.join(EXE)).find(|p| p.is_file())
}

/// Runs Rhubarb at `exe` on `pcm` and returns one mouth shape per frame at `fps`, as many frames
/// as [`crate::mouths`] gives for the same sound.
pub fn rhubarb_mouths(exe: &Path, pcm: &Pcm, fps: u32, recognizer: Recognizer) -> Result<Vec<char>, String> {
    // wasm has no processes, and its std::env::temp_dir panics.
    if cfg!(target_arch = "wasm32") {
        return Err("Rhubarb Lip Sync needs the desktop app".into());
    }
    if fps == 0 {
        return Ok(Vec::new());
    }
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default();
    let wav = std::env::temp_dir().join(format!("7rrek-lipsync-{}-{stamp}.wav", std::process::id()));
    std::fs::write(&wav, wav_bytes(pcm)).map_err(|e| format!("could not write the sound for Rhubarb: {e}"))?;
    let out = Command::new(exe).args(["--quiet", "--exportFormat", "json", "--recognizer", recognizer.arg()]).arg(&wav).output();
    let _ = std::fs::remove_file(&wav);
    let out = out.map_err(|e| format!("could not run Rhubarb at {}: {e}", exe.display()))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("Rhubarb failed: {}", err.trim()));
    }
    let frames = frame_count(pcm, fps);
    cues_to_frames(&String::from_utf8_lossy(&out.stdout), frames, fps)
}

fn frame_count(pcm: &Pcm, fps: u32) -> usize {
    ((pcm.samples.len() / 2) as u64).saturating_mul(fps as u64).div_ceil(RATE as u64) as usize
}

/// The mono mix of `pcm` as a 16-bit PCM WAV file at [`RATE`].
fn wav_bytes(pcm: &Pcm) -> Vec<u8> {
    let samples: Vec<i16> =
        pcm.samples.chunks(2).map(|lr| (lr.iter().sum::<f32>() / lr.len() as f32 * 32767.0).clamp(-32768.0, 32767.0) as i16).collect();
    let data = (samples.len() * 2) as u32;
    let mut w = Vec::with_capacity(44 + data as usize);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&RATE.to_le_bytes());
    w.extend_from_slice(&(RATE * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        w.extend_from_slice(&s.to_le_bytes());
    }
    w
}

/// Turns Rhubarb's JSON (`{"mouthCues": [{"start", "end", "value"}]}`, seconds) into one shape per
/// frame, taken at each frame's middle. Frames no cue covers rest (`X`).
fn cues_to_frames(json: &str, frames: usize, fps: u32) -> Result<Vec<char>, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("Rhubarb gave unreadable output: {e}"))?;
    let cues = v.get("mouthCues").and_then(|c| c.as_array()).ok_or("Rhubarb's output has no mouthCues")?;
    let mut parsed = Vec::with_capacity(cues.len());
    for c in cues {
        let start = c.get("start").and_then(serde_json::Value::as_f64);
        let end = c.get("end").and_then(serde_json::Value::as_f64);
        let shape = c.get("value").and_then(|s| s.as_str()).and_then(|s| s.chars().next()).filter(|m| MOUTHS.contains(m));
        match (start, end, shape) {
            (Some(s), Some(e), Some(m)) => parsed.push((s, e, m)),
            _ => return Err(format!("Rhubarb gave a mouth cue 7rrek cannot read: {c}")),
        }
    }
    if parsed.iter().any(|(s, e, _)| !s.is_finite() || !e.is_finite()) {
        return Err("Rhubarb gave a mouth cue with no real time".into());
    }
    parsed.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut next = 0;
    Ok((0..frames)
        .map(|i| {
            let t = (i as f64 + 0.5) / fps as f64;
            while parsed.get(next).is_some_and(|c| c.1 <= t) {
                next += 1;
            }
            parsed.get(next).filter(|c| c.0 <= t).map_or('X', |c| c.2)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cues_land_on_frame_middles() {
        let json = r#"{"metadata":{"duration":0.5},"mouthCues":[
            {"start":0.00,"end":0.10,"value":"X"},{"start":0.10,"end":0.25,"value":"D"},
            {"start":0.25,"end":0.40,"value":"A"},{"start":0.40,"end":0.50,"value":"X"}]}"#;
        // 10 fps: middles at 0.05, 0.15, …, 0.45; the last frame has no cue past 0.5.
        let got: String = cues_to_frames(json, 6, 10).unwrap().into_iter().collect();
        assert_eq!(got, "XDAAXX");
    }

    #[test]
    fn extended_shapes_pass_and_unknown_ones_are_refused() {
        let ok = r#"{"mouthCues":[{"start":0,"end":1,"value":"G"}]}"#;
        assert_eq!(cues_to_frames(ok, 2, 2).unwrap(), vec!['G', 'G']);
        let bad = r#"{"mouthCues":[{"start":0,"end":1,"value":"Q"}]}"#;
        assert!(cues_to_frames(bad, 2, 2).is_err());
        assert!(cues_to_frames("not json", 2, 2).is_err());
    }

    #[test]
    fn wav_is_mono_16_bit_at_rate() {
        let pcm = Pcm { samples: vec![1.0, 1.0, -1.0, 0.0, 0.0, 0.0] };
        let w = wav_bytes(&pcm);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(u16::from_le_bytes([w[22], w[23]]), 1);
        assert_eq!(u32::from_le_bytes([w[24], w[25], w[26], w[27]]), RATE);
        assert_eq!(u32::from_le_bytes([w[40], w[41], w[42], w[43]]), 6);
        assert_eq!(i16::from_le_bytes([w[44], w[45]]), 32767);
        assert_eq!(i16::from_le_bytes([w[46], w[47]]), -16383);
        // Round-trips through 7rrek's own decoder.
        assert_eq!(crate::decode(&w, Some("wav")).unwrap().samples.len(), 6);
    }

    /// A stand-in Rhubarb (a shell script) checks the arguments 7rrek passes and answers with cues.
    #[cfg(unix)]
    #[test]
    fn runs_the_program_and_reads_its_cues() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("7rrek-fake-rhubarb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("rhubarb");
        std::fs::write(
            &exe,
            "#!/bin/sh\n[ \"$1 $2 $3 $4 $5\" = '--quiet --exportFormat json --recognizer phonetic' ] || { echo \"bad args: $*\" >&2; exit 2; }\n\
             [ -s \"$6\" ] || { echo 'no wav' >&2; exit 3; }\n\
             echo '{\"mouthCues\":[{\"start\":0,\"end\":0.5,\"value\":\"X\"},{\"start\":0.5,\"end\":1,\"value\":\"D\"}]}'\n",
        )
        .unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let pcm = Pcm { samples: vec![0.0; (RATE as usize) * 2] };
        let got: String = rhubarb_mouths(&exe, &pcm, 4, Recognizer::Phonetic).unwrap().into_iter().collect();
        assert_eq!(got, "XXDD");
        let err = rhubarb_mouths(&exe, &pcm, 4, Recognizer::English).unwrap_err();
        assert!(err.contains("bad args"), "{err}");
        assert!(rhubarb_mouths(&dir.join("missing"), &pcm, 4, Recognizer::Phonetic).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn frame_count_matches_the_guess() {
        let pcm = Pcm { samples: vec![0.0; (RATE as usize) * 2] };
        assert_eq!(frame_count(&pcm, 24), crate::mouths(&pcm, 24).len());
    }
}
