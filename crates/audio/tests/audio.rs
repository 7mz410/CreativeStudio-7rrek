use std::process::Command;

use vectorcraft_audio::{Clip, Pcm, RATE, decode, mix, peaks};

/// 16-bit PCM WAV, mono, holding a sine of `secs` seconds at `rate` Hz.
fn sine_wav(rate: u32, secs: f64) -> Vec<u8> {
    let n = (rate as f64 * secs) as usize;
    let mut data = Vec::with_capacity(n * 2);
    for i in 0..n {
        let v = (2.0 * std::f64::consts::PI * 440.0 * i as f64 / rate as f64).sin();
        data.extend_from_slice(&((v * 16000.0) as i16).to_le_bytes());
    }
    let mut w = Vec::new();
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    w.extend_from_slice(b"WAVE");
    w.extend_from_slice(b"fmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&1u16.to_le_bytes()); // mono
    w.extend_from_slice(&rate.to_le_bytes());
    w.extend_from_slice(&(rate * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(data.len() as u32).to_le_bytes());
    w.extend_from_slice(&data);
    w
}

fn constant(value: f32, secs: f64) -> Pcm {
    Pcm { samples: vec![value; (secs * RATE as f64) as usize * 2] }
}

fn assert_half_second(pcm: &Pcm, tolerance: f64) {
    let want = 0.5 * RATE as f64 * 2.0;
    let got = pcm.samples.len() as f64;
    assert!((got - want).abs() <= want * tolerance, "length {got} not within {tolerance} of {want}");
}

#[test]
fn wav_decodes_to_48k_stereo() {
    let pcm = decode(&sine_wav(22_050, 0.5), Some("wav")).unwrap();
    assert_half_second(&pcm, 0.01);
    assert!(pcm.samples.iter().any(|s| s.abs() > 0.1), "decoded audio is silent");
    // Mono is duplicated to both channels.
    assert_eq!(pcm.samples[100], pcm.samples[101]);
}

#[test]
fn wav_decodes_without_extension_hint() {
    let pcm = decode(&sine_wav(48_000, 0.5), None).unwrap();
    assert_half_second(&pcm, 0.01);
}

#[test]
fn mix_places_clips_at_their_start() {
    let a = constant(0.5, 1.0);
    let b = constant(0.25, 0.25);
    let clips = [Clip { pcm: &a, start: 0.0, gain: 1.0 }, Clip { pcm: &b, start: 0.5, gain: 1.0 }];
    let out = mix(&clips, 0.0, 1.0);
    assert_eq!(out.len(), RATE as usize * 2);
    let frame = |t: f64| (t * RATE as f64) as usize * 2;
    assert_eq!(out[frame(0.25)], 0.5);
    assert_eq!(out[frame(0.6)], 0.75);
    assert_eq!(out[frame(0.9)], 0.5);
}

#[test]
fn mix_window_offsets_and_gain() {
    let a = constant(0.5, 1.0);
    let clips = [Clip { pcm: &a, start: 0.0, gain: 0.5 }];
    // Window starts after the clip ends: silence.
    assert!(mix(&clips, 2.0, 3.0).iter().all(|s| *s == 0.0));
    // Window starts inside the clip: value is the clip at gain.
    let out = mix(&clips, 0.5, 0.75);
    assert_eq!(out.len(), RATE as usize / 4 * 2);
    assert_eq!(out[0], 0.25);
    assert_eq!(out[out.len() - 1], 0.25);
}

#[test]
fn mix_clamps_and_survives_bad_input() {
    let a = constant(0.9, 0.1);
    let clips = [
        Clip { pcm: &a, start: 0.0, gain: 1.0 },
        Clip { pcm: &a, start: 0.0, gain: 1.0 },
        Clip { pcm: &a, start: f64::NAN, gain: 1.0 },
        Clip { pcm: &a, start: -1.0, gain: f32::NAN },
    ];
    let out = mix(&clips, 0.0, 0.1);
    assert_eq!(out[0], 1.0);
    // Clip starting before the window: its tail is still heard.
    let early = [Clip { pcm: &a, start: -0.05, gain: 1.0 }];
    assert_eq!(mix(&early, 0.0, 0.01)[0], 0.9);
    assert!(mix(&clips, f64::NAN, 1.0).is_empty());
    assert!(mix(&clips, 1.0, 0.0).is_empty());
    assert!(mix(&clips, 0.0, f64::INFINITY).is_empty());
    // A negative window is valid timeline time: silence, no panic.
    assert!(mix(&[], -5.0, -1.0).iter().all(|s| *s == 0.0));
}

#[test]
fn peaks_has_requested_length() {
    let pcm = decode(&sine_wav(22_050, 0.5), Some("wav")).unwrap();
    assert_eq!(peaks(&pcm, 64).len(), 64);
    assert_eq!(peaks(&pcm, 1).len(), 1);
    assert!(peaks(&pcm, 64).iter().all(|p| (0.0..=1.0).contains(p)));
    assert!(peaks(&Pcm::default(), 8).iter().all(|p| *p == 0.0));
}

#[test]
fn garbage_is_an_error() {
    assert!(decode(b"this is not audio at all", None).is_err());
    assert!(decode(&[], Some("mp3")).is_err());
}

fn ffmpeg_sine(ext: &str, codec: &[&str]) -> Option<Vec<u8>> {
    let dir = std::env::temp_dir().join(format!("vc-audio-{}-{}", std::process::id(), ext));
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("tone.{ext}"));
    let status = Command::new("ffmpeg").args(["-v", "error", "-y", "-f", "lavfi", "-i", "sine=d=0.5"]).args(codec).arg(&path).status().ok()?;
    let bytes = if status.success() { std::fs::read(&path).ok() } else { None };
    let _ = std::fs::remove_dir_all(&dir);
    bytes
}

#[test]
fn ffmpeg_mp3_and_ogg_decode() {
    for (ext, codec) in [("mp3", &["-c:a", "libmp3lame"][..]), ("ogg", &["-c:a", "vorbis", "-strict", "-2", "-ac", "2"][..])] {
        let Some(bytes) = ffmpeg_sine(ext, codec) else {
            eprintln!("skipping {ext}: ffmpeg not available");
            continue;
        };
        let pcm = decode(&bytes, Some(ext)).unwrap();
        // Encoder delay and padding add a few hundredths of a second; allow 0.1 s.
        let got = pcm.samples.len() as f64 / (RATE as f64 * 2.0);
        assert!((got - 0.5).abs() <= 0.1, "{ext} decoded to {got} s");
    }
}
