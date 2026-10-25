//! Audio input shared by the WebM and MP4 writers, and the frame timing both use.

/// Sample rate of the audio the writers take: Opus runs at 48 kHz and AAC is encoded at the same rate.
pub(crate) const SAMPLE_RATE: u32 = 48_000;
/// Channels of the audio the writers take: interleaved stereo.
pub(crate) const CHANNELS: usize = 2;

/// Checks the caller's audio and returns a copy with NaN and infinities set to silence and every
/// sample clamped to the full-scale range.
pub(crate) fn prepare(audio: &[f32]) -> Result<Vec<f32>, String> {
    if audio.is_empty() {
        return Err("audio is empty; use new() for video only".to_string());
    }
    if !audio.len().is_multiple_of(CHANNELS) {
        return Err(format!("audio has {} samples, not a whole number of stereo frames", audio.len()));
    }
    Ok(audio.iter().map(|s| if s.is_finite() { s.clamp(-1.0, 1.0) } else { 0.0 }).collect())
}

/// Start of video frame `k` in milliseconds, rounded to the nearest millisecond.
pub(crate) fn frame_ms(k: u64, fps: u32) -> u64 {
    let fps = u64::from(fps.max(1));
    (k.saturating_mul(1000) + fps / 2) / fps
}
