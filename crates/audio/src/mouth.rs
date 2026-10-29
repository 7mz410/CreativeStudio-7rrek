//! Guesses a lip-sync mouth shape for each animation frame of a sound.
//!
//! Each frame is judged from its loudness, zero-crossing rate and brightness, then a lone frame
//! between two equal neighbours takes their shape.

use crate::{Pcm, RATE};

/// Preston Blair mouth shapes, one char each:
///
/// - `A`: closed, for M, B and P.
/// - `B`: slightly open, for most consonants.
/// - `C`: open, for E and EH.
/// - `D`: wide open, for A and AA.
/// - `E`: rounded, for O.
/// - `F`: puckered, for U, OO and W.
/// - `G`: teeth on the lower lip, for F and V.
/// - `H`: tongue up, for L.
/// - `X`: rest, silence.
pub const MOUTHS: [char; 9] = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'X'];

// Heuristic thresholds. `rel` is loudness against the clip's loudest frame, 0 to 1.
// shortcut: a heuristic guess from loudness, zero crossings and brightness; a phoneme recogniser would replace `mouths`.
const SILENT_REL: f32 = 0.08;
const NOISY_ZCR: f32 = 0.25;
const FRICATIVE_MAX_REL: f32 = 0.3;
const QUIET_REL: f32 = 0.2;
const DARK_BRIGHTNESS: f32 = 0.05;
const DARK_LOUD_REL: f32 = 0.5;
const LOUD_REL: f32 = 0.7;
const BRIGHT_BRIGHTNESS: f32 = 0.3;

/// Per-frame measurements over the mono mix of one window.
#[derive(Clone, Copy, Default)]
struct Frame {
    rms: f32,
    /// Sign changes per sample.
    zcr: f32,
    /// Energy of the first difference over the energy of the signal.
    brightness: f32,
}

/// Returns one mouth shape per frame at `fps`, for the length of `pcm`. Frames = ceil(seconds *
/// fps). `fps` 0 gives an empty vec.
pub fn mouths(pcm: &Pcm, fps: u32) -> Vec<char> {
    if fps == 0 {
        return Vec::new();
    }
    let total = (pcm.samples.len() / 2) as u64;
    let fps = fps as u64;
    let rate = RATE as u64;
    let count = total.saturating_mul(fps).div_ceil(rate) as usize;
    // First sample of frame `i`, clamped to the clip.
    let bound = |i: usize| -> usize { ((i as u64).saturating_mul(rate) / fps).min(total) as usize };

    let frames: Vec<Frame> = (0..count).map(|i| frame_stats(&pcm.samples, bound(i), bound(i + 1))).collect();
    let loudest = frames.iter().map(|f| f.rms).fold(0.0f32, f32::max);
    let raw: Vec<char> = frames
        .iter()
        .map(|f| {
            let rel = if loudest > 0.0 { f.rms / loudest } else { 0.0 };
            classify(rel, *f)
        })
        .collect();
    smooth(raw)
}

/// Measures the mono mix of frames `from..to` of interleaved stereo `pcm`.
fn frame_stats(pcm: &[f32], from: usize, to: usize) -> Frame {
    let n = to.saturating_sub(from);
    if n == 0 {
        return Frame::default();
    }
    let mut energy = 0.0f32;
    let mut diff_energy = 0.0f32;
    let mut crossings = 0usize;
    let mut prev: Option<f32> = None;
    for f in from..to {
        let l = pcm.get(2 * f).copied().unwrap_or(0.0);
        let r = pcm.get(2 * f + 1).copied().unwrap_or(0.0);
        let m = (l + r) * 0.5;
        energy += m * m;
        if let Some(p) = prev {
            if (p >= 0.0) != (m >= 0.0) {
                crossings += 1;
            }
            let d = m - p;
            diff_energy += d * d;
        }
        prev = Some(m);
    }
    let nf = n as f32;
    Frame { rms: (energy / nf).sqrt(), zcr: crossings as f32 / nf, brightness: if energy > 0.0 { diff_energy / energy } else { 0.0 } }
}

/// Picks the shape for one frame by the rules in order.
fn classify(rel: f32, f: Frame) -> char {
    if rel < SILENT_REL {
        return 'X';
    }
    if f.zcr > NOISY_ZCR {
        return if rel < FRICATIVE_MAX_REL { 'G' } else { 'B' };
    }
    if rel < QUIET_REL {
        return 'A';
    }
    if f.brightness < DARK_BRIGHTNESS {
        return if rel < DARK_LOUD_REL { 'F' } else { 'E' };
    }
    if rel > LOUD_REL {
        return 'D';
    }
    if f.brightness > BRIGHT_BRIGHTNESS {
        return 'C';
    }
    'B'
}

/// A single frame between two equal neighbours takes their shape. Reads from `raw`, so one
/// change never cascades into the next frame's decision.
fn smooth(raw: Vec<char>) -> Vec<char> {
    let mut out = raw.clone();
    for (i, w) in raw.windows(3).enumerate() {
        let [a, b, c] = w else { continue };
        if let Some(slot) = out.get_mut(i + 1).filter(|_| a == c && b != a) {
            *slot = *a;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stereo sine at `hz` and `amp`, lasting `secs`.
    fn sine(secs: f64, hz: f64, amp: f32) -> Vec<f32> {
        let n = (RATE as f64 * secs) as usize;
        let mut samples = Vec::with_capacity(n * 2);
        for i in 0..n {
            let v = amp * (2.0 * std::f64::consts::PI * hz * i as f64 / RATE as f64).sin() as f32;
            samples.push(v);
            samples.push(v);
        }
        samples
    }

    /// White noise from a linear congruential generator, stereo, lasting `secs`.
    fn noise(secs: f64) -> Vec<f32> {
        let n = (RATE as f64 * secs) as usize;
        let mut state: u32 = 12345;
        let mut samples = Vec::with_capacity(n * 2);
        for _ in 0..n * 2 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            samples.push((state >> 8) as f32 / 8_388_608.0 - 1.0);
        }
        samples
    }

    #[test]
    fn silence_is_all_rest() {
        // Two seconds of stereo silence: RATE frames per second, two samples per frame.
        let pcm = Pcm { samples: vec![0.0; RATE as usize * 4] };
        let out = mouths(&pcm, 24);
        assert_eq!(out.len(), 48);
        assert!(out.iter().all(|c| *c == 'X'));
    }

    #[test]
    fn loud_tone_opens_wide() {
        let pcm = Pcm { samples: sine(2.0, 200.0, 0.8) };
        let out = mouths(&pcm, 24);
        assert_eq!(out.len(), 48);
        let open = out.iter().filter(|c| **c == 'D' || **c == 'E').count();
        assert!(open > 40, "expected mostly D or E, got {out:?}");
        assert!(out.iter().all(|c| *c != 'X'));
    }

    #[test]
    fn white_noise_is_consonant() {
        let pcm = Pcm { samples: noise(1.0) };
        let out = mouths(&pcm, 24);
        assert!(out.iter().all(|c| *c == 'B' || *c == 'G'), "got {out:?}");
    }

    #[test]
    fn silence_then_tone_starts_at_rest_and_ends_open() {
        let mut samples = vec![0.0; RATE as usize];
        samples.extend(sine(0.5, 200.0, 0.8));
        let out = mouths(&Pcm { samples }, 24);
        assert_eq!(out.first(), Some(&'X'));
        assert!(out.last().is_some_and(|c| *c != 'X'), "got {out:?}");
    }

    #[test]
    fn zero_fps_gives_nothing() {
        let pcm = Pcm { samples: sine(1.0, 200.0, 0.8) };
        assert!(mouths(&pcm, 0).is_empty());
    }
}
