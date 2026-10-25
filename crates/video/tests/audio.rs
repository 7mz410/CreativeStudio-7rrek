//! Writes 24 frames (64×48, 12 fps) with 2 s of 440 Hz stereo sine to WebM and to MP4. When
//! ffprobe is on PATH, checks the stream list of each file; when ffmpeg is too, checks that it
//! decodes both files without errors.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use vectorcraft_video::{Mp4Writer, WebmWriter};

const W: u32 = 64;
const H: u32 = 48;
const FPS: u32 = 12;
const FRAMES: u32 = 24;
const RATE: usize = 48_000;

fn frame(k: u32) -> Vec<u8> {
    let r = (k * 10 % 256) as u8;
    let g = (255 - k * 10 % 256) as u8;
    let mut px = Vec::with_capacity((W * H * 4) as usize);
    for _ in 0..W * H {
        px.extend_from_slice(&[r, g, 128, 255]);
    }
    px
}

/// Two seconds of 440 Hz sine, interleaved stereo at 48 kHz, half scale.
fn sine() -> Vec<f32> {
    (0..RATE * 2)
        .flat_map(|i| {
            let s = (2.0 * std::f64::consts::PI * 440.0 * i as f64 / RATE as f64).sin() as f32 * 0.5;
            [s, s]
        })
        .collect()
}

fn webm_bytes(audio: &[f32]) -> Result<Vec<u8>, String> {
    let mut w = WebmWriter::with_audio(W, H, FPS, audio)?;
    for k in 0..FRAMES {
        w.push(&frame(k))?;
    }
    if w.frame_count() != u64::from(FRAMES) {
        return Err(format!("pushed {} frames, expected {FRAMES}", w.frame_count()));
    }
    Ok(w.finish())
}

fn mp4_bytes(audio: &[f32]) -> Result<Vec<u8>, String> {
    let mut m = Mp4Writer::with_audio(W, H, FPS, audio)?;
    for k in 0..FRAMES {
        m.push(&frame(k))?;
    }
    m.finish()
}

fn have(tool: &str) -> bool {
    Command::new(tool).arg("-version").output().is_ok()
}

fn write_tmp(name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    let path = std::env::temp_dir().join(format!("vectorcraft-audio-{}-{name}", std::process::id()));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

fn stream_codecs(file: &Path) -> Result<Vec<String>, String> {
    let out: Output = Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "stream=codec_name", "-of", "csv=p=0"])
        .arg(file)
        .output()
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    Ok(text.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
}

/// Decodes the whole file with ffmpeg and returns its stderr (empty when decoding is clean).
fn decode_errors(file: &Path) -> Result<String, String> {
    let out = Command::new("ffmpeg").args(["-v", "error", "-i"]).arg(file).args(["-f", "null", "-"]).output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("ffmpeg exited with {}", out.status));
    }
    Ok(String::from_utf8_lossy(&out.stderr).trim().to_string())
}

#[test]
fn webm_has_vp9_and_opus_tracks() -> Result<(), String> {
    let bytes = webm_bytes(&sine())?;
    assert_eq!(&bytes[..4], &[0x1A, 0x45, 0xDF, 0xA3], "EBML magic");
    assert!(bytes.windows(5).any(|w| w == b"V_VP9"), "VP9 track");
    assert!(bytes.windows(6).any(|w| w == b"A_OPUS"), "Opus track");
    assert!(bytes.windows(8).any(|w| w == b"OpusHead"), "OpusHead CodecPrivate");

    if !have("ffprobe") {
        eprintln!("skipped ffprobe checks: ffprobe not on PATH");
        return Ok(());
    }
    let path = write_tmp("audio.webm", &bytes)?;
    let codecs = stream_codecs(&path)?;
    assert_eq!(codecs, ["vp9", "opus"], "ffprobe stream codec names");
    if have("ffmpeg") {
        let errors = decode_errors(&path)?;
        assert!(errors.is_empty(), "ffmpeg decode errors: {errors}");
    } else {
        eprintln!("skipped ffmpeg decode check: ffmpeg not on PATH");
    }
    let _ = std::fs::remove_file(&path);
    Ok(())
}

#[test]
fn mp4_has_h264_and_aac_tracks() -> Result<(), String> {
    let bytes = mp4_bytes(&sine())?;
    assert_eq!(&bytes[4..8], b"ftyp", "ISO BMFF file type box");
    assert!(bytes.windows(4).any(|w| w == b"avc1"), "H.264 sample entry");
    assert!(bytes.windows(4).any(|w| w == b"mp4a"), "AAC sample entry");

    if !have("ffprobe") {
        eprintln!("skipped ffprobe checks: ffprobe not on PATH");
        return Ok(());
    }
    let path = write_tmp("audio.mp4", &bytes)?;
    let codecs = stream_codecs(&path)?;
    assert_eq!(codecs, ["h264", "aac"], "ffprobe stream codec names");
    if have("ffmpeg") {
        let errors = decode_errors(&path)?;
        assert!(errors.is_empty(), "ffmpeg decode errors: {errors}");
    } else {
        eprintln!("skipped ffmpeg decode check: ffmpeg not on PATH");
    }
    let _ = std::fs::remove_file(&path);
    Ok(())
}

#[test]
fn audio_past_the_video_end_is_cut() -> Result<(), String> {
    // 10 s of audio against 2 s of video: the first 2 s are the same sine, so the files must match.
    let long: Vec<f32> = sine().into_iter().cycle().take(RATE * 2 * 10).collect();
    assert_eq!(webm_bytes(&long)?, webm_bytes(&sine())?, "WebM: audio past the video end was written");
    assert_eq!(mp4_bytes(&long)?, mp4_bytes(&sine())?, "MP4: audio past the video end was written");
    Ok(())
}

#[test]
fn bad_audio_is_refused() {
    assert!(WebmWriter::with_audio(W, H, FPS, &[]).is_err(), "empty audio");
    assert!(WebmWriter::with_audio(W, H, FPS, &[0.0; 3]).is_err(), "odd sample count");
    assert!(Mp4Writer::with_audio(W, H, FPS, &[]).is_err(), "empty audio");
    assert!(Mp4Writer::with_audio(W, H, FPS, &[0.0; 3]).is_err(), "odd sample count");
}
