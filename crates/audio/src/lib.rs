//! Decodes sound files held in memory to 48 kHz stereo `f32` PCM, and mixes clips into a window
//! of the animation timeline.
//!
//! [`decode`] reads WAV, MP3 and Ogg Vorbis through symphonia. [`mix`] sums [`Clip`]s at their
//! start times, and [`peaks`] reduces a clip to per-bucket maxima for a waveform.
//! [`mouths`] guesses a lip-sync mouth shape for each animation frame.

mod mouth;

use std::io::Cursor;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub use mouth::{MOUTHS, mouths};

/// Output sample rate in Hz.
pub const RATE: u32 = 48_000;

/// Longest sound [`decode`] accepts, in seconds of source audio.
const MAX_SECONDS: usize = 30 * 60;

/// Decoded audio: interleaved stereo `f32` samples at [`RATE`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pcm {
    /// Interleaved stereo: `[L0, R0, L1, R1, ...]`.
    pub samples: Vec<f32>,
}

/// One sound placed on the timeline.
#[derive(Debug, Clone, Copy)]
pub struct Clip<'a> {
    pub pcm: &'a Pcm,
    /// Where the clip starts on the timeline, in seconds.
    pub start: f64,
    pub gain: f32,
}

/// Decodes a sound file held in memory. `ext_hint` is a file extension such as `"mp3"`, used to
/// pick the format faster; probing still works without it.
pub fn decode(bytes: &[u8], ext_hint: Option<&str>) -> Result<Pcm, String> {
    let source = MediaSourceStream::new(Box::new(Cursor::new(bytes.to_vec())), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = ext_hint {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, source, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("unrecognised audio: {e}"))?;
    let mut format = probed.format;
    let track = format.default_track().ok_or("no audio track")?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("unsupported audio codec: {e}"))?;

    let mut rate: Option<u32> = None;
    let mut stereo: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(format!("read failed: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            // A corrupt packet is skipped, as players do; the rest of the file still plays.
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(format!("decode failed: {e}")),
        };
        let spec = *decoded.spec();
        match rate {
            None => {
                if spec.rate == 0 {
                    return Err("sample rate is zero".into());
                }
                rate = Some(spec.rate);
            }
            Some(r) if r != spec.rate => return Err("sample rate changed mid-stream".into()),
            Some(_) => {}
        }
        let channels = spec.channels.count();
        if channels == 0 {
            return Err("no channels".into());
        }
        let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buf.copy_interleaved_ref(decoded);
        for frame in buf.samples().chunks_exact(channels) {
            // Mono is duplicated to both sides; more than two channels keeps the first two.
            let (l, r) = match frame {
                [m] => (*m, *m),
                [l, r, ..] => (*l, *r),
                _ => continue,
            };
            stereo.push(l);
            stereo.push(r);
        }
        let src_rate = rate.unwrap_or(RATE) as usize;
        if stereo.len() / 2 > src_rate * MAX_SECONDS {
            return Err("audio is longer than 30 minutes".into());
        }
    }

    let src_rate = rate.unwrap_or(RATE);
    Ok(Pcm { samples: resample(&stereo, src_rate) })
}

/// Linear-interpolation resampler from `from_rate` to [`RATE`] over interleaved stereo.
// shortcut: linear resampling, upgrade to windowed sinc if quality complaints
fn resample(src: &[f32], from_rate: u32) -> Vec<f32> {
    if from_rate == RATE {
        return src.to_vec();
    }
    let in_frames = src.len() / 2;
    let out_frames = (in_frames as u64 * RATE as u64 / from_rate as u64) as usize;
    let step = from_rate as f64 / RATE as f64;
    let mut out = Vec::with_capacity(out_frames * 2);
    for i in 0..out_frames {
        let pos = i as f64 * step;
        let i0 = pos.floor() as usize;
        let frac = (pos - i0 as f64) as f32;
        let i1 = (i0 + 1).min(in_frames.saturating_sub(1));
        for c in 0..2 {
            let a = src.get(i0 * 2 + c).copied().unwrap_or(0.0);
            let b = src.get(i1 * 2 + c).copied().unwrap_or(0.0);
            out.push(a + (b - a) * frac);
        }
    }
    out
}

/// Mixes `clips` over the timeline window `[from, to)` seconds into a stereo buffer at [`RATE`].
/// Clips are summed at their start times and the result is clamped to ±1. Non-finite bounds
/// give an empty buffer; windows longer than 30 minutes are cut to 30 minutes.
pub fn mix(clips: &[Clip], from: f64, to: f64) -> Vec<f32> {
    if !from.is_finite() || !to.is_finite() || to <= from {
        return Vec::new();
    }
    let rate = RATE as f64;
    let w0 = (from * rate).round() as i64;
    let max_frames = (MAX_SECONDS as f64 * rate) as usize;
    let frames = (((to - from) * rate).round() as usize).min(max_frames);
    let mut out = vec![0.0f32; frames * 2];

    for clip in clips {
        if !clip.start.is_finite() || !clip.gain.is_finite() {
            continue;
        }
        let s = (clip.start * rate).round() as i64;
        let clip_frames = (clip.pcm.samples.len() / 2) as i64;
        // Overlap of the clip with the window, as output frame indices.
        let o0 = (s.saturating_sub(w0)).max(0);
        let o1 = (s.saturating_add(clip_frames).saturating_sub(w0)).min(frames as i64);
        if o1 <= o0 {
            continue;
        }
        let src0 = (o0 + w0 - s).max(0) as usize;
        let (o0, o1) = (o0 as usize, o1 as usize);
        let src1 = src0 + (o1 - o0);
        let (Some(dst), Some(src)) = (out.get_mut(o0 * 2..o1 * 2), clip.pcm.samples.get(src0 * 2..src1 * 2)) else {
            continue;
        };
        for (d, v) in dst.iter_mut().zip(src) {
            *d += v * clip.gain;
        }
    }
    for v in &mut out {
        *v = v.clamp(-1.0, 1.0);
    }
    out
}

/// Maximum absolute sample per bucket, over both channels. Returns `buckets` values; a silent or
/// empty clip gives all zeros.
pub fn peaks(pcm: &Pcm, buckets: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; buckets];
    let total = pcm.samples.len() as u64;
    if buckets == 0 || total == 0 {
        return out;
    }
    for (i, s) in pcm.samples.iter().enumerate() {
        let b = (i as u64 * buckets as u64 / total) as usize;
        if let Some(p) = out.get_mut(b) {
            *p = p.max(s.abs());
        }
    }
    out
}
