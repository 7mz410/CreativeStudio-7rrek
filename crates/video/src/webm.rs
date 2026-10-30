//! WebM muxer for one VP9 video track and an optional Opus audio track, written from the
//! Matroska and WebM specifications (RFC 8794 EBML, the Matroska element registry, the WebM
//! container guidelines).
//!
//! Layout: EBML header, Segment (size patched by [`WebmWriter::finish`]), Info, Tracks, then one
//! Cluster per frame. A cluster holds the frame's SimpleBlock (track 1) and the Opus packets
//! (track 2) that start within the frame's time span. Timestamps are milliseconds (TimecodeScale
//! 1 000 000 ns). Opus packets are 20 ms; audio stops at the last packet that starts before the
//! last frame ends, so it can run up to one packet past the video.

use crate::audio::{CHANNELS, SAMPLE_RATE, frame_ms, prepare};
use crate::opus::OpusEncoder;
use crate::vp9::{EncoderConfig, Vp9Encoder, rgba_to_yuv420};

/// VP9 quantiser quality (1..=100; 100 is lossless and far larger).
const QUALITY: u8 = 80;
const MAX_DIM: u32 = 65_536;
const MAX_FPS: u32 = 1_000;
/// Opus bitrate of the audio track, bits per second.
const OPUS_BITRATE: u32 = 128_000;
/// Length of one Opus packet, milliseconds.
const PACKET_MS: u64 = 20;
/// Opus decoders need this much pre-roll after a seek (RFC 7845 section 4.2).
const SEEK_PRE_ROLL_NS: u64 = 80_000_000;

const EBML: u32 = 0x1A45_DFA3;
const SEGMENT: u32 = 0x1853_8067;
const INFO: u32 = 0x1549_A966;
const TRACKS: u32 = 0x1654_AE6B;
const TRACK_ENTRY: u32 = 0xAE;
const CLUSTER: u32 = 0x1F43_B675;

/// The audio of a WebM file: interleaved stereo PCM and the packets cut from it so far.
struct Audio {
    pcm: Vec<f32>,
    encoder: OpusEncoder,
    /// Packets emitted so far. Packet `n` holds sample frames `n * FRAME_SIZE ..` and starts at
    /// `n * PACKET_MS` milliseconds.
    packets: u64,
}

/// Streams RGBA frames into a WebM byte buffer. Call [`push`](Self::push) per frame, then
/// [`finish`](Self::finish).
pub struct WebmWriter {
    width: u32,
    height: u32,
    fps: u32,
    encoder: Vp9Encoder,
    out: Vec<u8>,
    /// Byte offset of the 8-byte Segment size field.
    size_pos: usize,
    /// Byte offset just past the Segment size field (the Segment body starts here).
    body_start: usize,
    frames: u64,
    audio: Option<Audio>,
}

impl WebmWriter {
    /// A writer for `width` × `height` frames at `fps` frames per second, with no audio.
    pub fn new(width: u32, height: u32, fps: u32) -> Result<WebmWriter, String> {
        Self::open(width, height, fps, None)
    }

    /// Like [`new`](Self::new), with an Opus audio track. `audio` is interleaved stereo at 48 kHz
    /// (`L, R, L, R, ...`), each sample in -1..=1 (values outside are clamped). Audio longer than
    /// the video is cut at its end; shorter audio just ends early.
    pub fn with_audio(width: u32, height: u32, fps: u32, audio: &[f32]) -> Result<WebmWriter, String> {
        let pcm = prepare(audio)?;
        Self::open(width, height, fps, Some(pcm))
    }

    fn open(width: u32, height: u32, fps: u32, pcm: Option<Vec<f32>>) -> Result<WebmWriter, String> {
        if width == 0 || height == 0 || width > MAX_DIM || height > MAX_DIM {
            return Err(format!("frame size {width}x{height} is out of range (1..={MAX_DIM} per side)"));
        }
        if fps == 0 || fps > MAX_FPS {
            return Err(format!("fps {fps} is out of range (1..={MAX_FPS})"));
        }
        let encoder = Vp9Encoder::new(EncoderConfig {
            width,
            height,
            quality: QUALITY,
            full_range: false,
            // A key frame every two seconds.
            keyframe_interval: fps.saturating_mul(2),
            target_kbps: None,
            frame_rate: f64::from(fps),
            loop_filter: true,
        });
        let audio = pcm.map(|pcm| Audio { pcm, encoder: OpusEncoder::new(CHANNELS as u8, OPUS_BITRATE), packets: 0 });

        let mut out = Vec::new();
        let mut eb = Vec::new();
        el_uint(&mut eb, 0x4286, 1); // EBMLVersion
        el_uint(&mut eb, 0x42F7, 1); // EBMLReadVersion
        el_uint(&mut eb, 0x42F2, 4); // EBMLMaxIDLength
        el_uint(&mut eb, 0x42F3, 8); // EBMLMaxSizeLength
        el_str(&mut eb, 0x4282, "webm"); // DocType
        el_uint(&mut eb, 0x4287, 4); // DocTypeVersion
        el_uint(&mut eb, 0x4285, 2); // DocTypeReadVersion
        el(&mut out, EBML, &eb);

        // Segment: 8-byte size (marker 0x01), written as unknown and patched in finish().
        out.extend(id_bytes(SEGMENT));
        let size_pos = out.len();
        out.extend_from_slice(&[0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
        let body_start = out.len();

        let mut info = Vec::new();
        el_uint(&mut info, 0x2AD7B1, 1_000_000); // TimecodeScale
        el_str(&mut info, 0x4D80, "VectorCraft"); // MuxingApp
        el_str(&mut info, 0x5741, "VectorCraft"); // WritingApp
        el(&mut out, INFO, &info);

        let mut video = Vec::new();
        el_uint(&mut video, 0xB0, u64::from(width)); // PixelWidth
        el_uint(&mut video, 0xBA, u64::from(height)); // PixelHeight
        let mut track = Vec::new();
        el_uint(&mut track, 0xD7, 1); // TrackNumber
        el_uint(&mut track, 0x73C5, 1); // TrackUID
        el_uint(&mut track, 0x83, 1); // TrackType: video
        el_uint(&mut track, 0x9C, 0); // FlagLacing
        el_str(&mut track, 0x86, "V_VP9"); // CodecID
        el_uint(&mut track, 0x23E383, 1_000_000_000 / u64::from(fps)); // DefaultDuration, ns
        el(&mut track, 0xE0, &video);
        let mut tracks = Vec::new();
        el(&mut tracks, TRACK_ENTRY, &track);

        if let Some(a) = &audio {
            let mut track = Vec::new();
            el_uint(&mut track, 0xD7, 2); // TrackNumber
            el_uint(&mut track, 0x73C5, 2); // TrackUID
            el_uint(&mut track, 0x83, 2); // TrackType: audio
            el_uint(&mut track, 0x9C, 0); // FlagLacing
            el_str(&mut track, 0x86, "A_OPUS"); // CodecID
            el(&mut track, 0x63A2, &a.encoder.opus_head()); // CodecPrivate: OpusHead
            let pre_skip = u64::from(a.encoder.pre_skip());
            el_uint(&mut track, 0x56AA, pre_skip * 1_000_000_000 / u64::from(SAMPLE_RATE)); // CodecDelay, ns
            el_uint(&mut track, 0x56BB, SEEK_PRE_ROLL_NS); // SeekPreRoll
            let mut audio_el = Vec::new();
            el_float(&mut audio_el, 0xB5, f64::from(SAMPLE_RATE)); // SamplingFrequency
            el_uint(&mut audio_el, 0x9F, CHANNELS as u64); // Channels
            el(&mut track, 0xE1, &audio_el);
            el(&mut tracks, TRACK_ENTRY, &track);
        }
        el(&mut out, TRACKS, &tracks);

        Ok(WebmWriter { width, height, fps, encoder, out, size_pos, body_start, frames: 0, audio })
    }

    /// Frames pushed so far.
    pub fn frame_count(&self) -> u64 {
        self.frames
    }

    /// Encodes one frame: `rgba` is `width × height × 4` bytes, straight (not premultiplied)
    /// RGBA, rows top to bottom. Alpha is ignored.
    pub fn push(&mut self, rgba: &[u8]) -> Result<(), String> {
        let expected = (self.width as usize).checked_mul(self.height as usize).and_then(|n| n.checked_mul(4)).ok_or("frame size overflows usize")?;
        if rgba.len() != expected {
            return Err(format!("frame {} is {} bytes, expected {expected} for {}x{} RGBA", self.frames, rgba.len(), self.width, self.height));
        }
        let (y, u, v) = rgba_to_yuv420(rgba, self.width, self.height, false);
        let frame = self.encoder.encode_frame(&y, &u, &v, false);
        let start = frame_ms(self.frames, self.fps);
        let end = frame_ms(self.frames + 1, self.fps);
        let mut blocks = vec![block(1, 0, if frame.key { 0x80 } else { 0x00 }, &frame.data)];
        blocks.extend(self.audio_blocks(start, end));
        self.out.extend(cluster(start, &blocks));
        self.frames += 1;
        Ok(())
    }

    /// The Opus packets that start in `[start, end)` ms and have not been written yet, as
    /// SimpleBlock payloads on track 2 (timecodes relative to `start`).
    fn audio_blocks(&mut self, start: u64, end: u64) -> Vec<Vec<u8>> {
        let Some(a) = self.audio.as_mut() else { return Vec::new() };
        let frame_size = OpusEncoder::FRAME_SIZE;
        let total_frames = (a.pcm.len() / CHANNELS) as u64;
        let mut blocks = Vec::new();
        loop {
            let packet_start_ms = a.packets * PACKET_MS;
            let first_frame = a.packets * frame_size as u64;
            if packet_start_ms >= end || first_frame >= total_frames {
                break;
            }
            let mut chunk = vec![0.0f32; frame_size * CHANNELS];
            if let Some(src) = a.pcm.get(first_frame as usize * CHANNELS..) {
                for (dst, s) in chunk.iter_mut().zip(src) {
                    *dst = *s;
                }
            }
            let packet = a.encoder.encode_float(&chunk);
            let rel = i16::try_from(packet_start_ms.saturating_sub(start)).unwrap_or(i16::MAX);
            blocks.push(block(2, rel, 0x80, &packet));
            a.packets += 1;
        }
        blocks
    }

    /// Patches the Segment size and returns the WebM bytes. Zero frames gives a valid header-only
    /// file, which players reject; [`encode_webm`] refuses that case instead.
    pub fn finish(mut self) -> Vec<u8> {
        let seg_len = (self.out.len() - self.body_start) as u64;
        let bytes = seg_len.to_be_bytes();
        let mut size = [0x01u8; 8];
        for (dst, src) in size.iter_mut().skip(1).zip(bytes.iter().skip(1)) {
            *dst = *src;
        }
        if let Some(field) = self.out.get_mut(self.size_pos..self.body_start) {
            field.copy_from_slice(&size);
        }
        self.out
    }
}

/// Encodes `frames` (each `width × height × 4` straight RGBA) at `fps` into a WebM byte buffer.
/// Errors on a zero or oversized dimension, a zero fps, a frame of the wrong length, or no
/// frames.
pub fn encode_webm(frames: impl IntoIterator<Item = Vec<u8>>, width: u32, height: u32, fps: u32) -> Result<Vec<u8>, String> {
    let mut writer = WebmWriter::new(width, height, fps)?;
    for frame in frames {
        writer.push(&frame)?;
    }
    if writer.frame_count() == 0 {
        return Err("no frames to encode".to_string());
    }
    Ok(writer.finish())
}

/// One Cluster at `ms`, holding the given SimpleBlock payloads.
fn cluster(ms: u64, blocks: &[Vec<u8>]) -> Vec<u8> {
    let mut body = Vec::new();
    el_uint(&mut body, 0xE7, ms); // Timestamp
    for b in blocks {
        el(&mut body, 0xA3, b); // SimpleBlock
    }
    let mut out = Vec::new();
    el(&mut out, CLUSTER, &body);
    out
}

/// A SimpleBlock payload: track number, timecode relative to the cluster, flags, frame.
fn block(track: u64, rel: i16, flags: u8, data: &[u8]) -> Vec<u8> {
    let mut b = vint(track);
    b.extend_from_slice(&rel.to_be_bytes());
    b.push(flags);
    b.extend_from_slice(data);
    b
}

/// Element id, without leading zero bytes.
fn id_bytes(id: u32) -> Vec<u8> {
    id.to_be_bytes().iter().skip_while(|b| **b == 0).copied().collect()
}

/// EBML variable-size integer for an element data size (shortest form).
fn vint(n: u64) -> Vec<u8> {
    for len in 1..=8u32 {
        let max = (1u64 << (7 * len)) - 2;
        if n <= max {
            let marked = n | (1u64 << (7 * len));
            return marked.to_be_bytes().iter().skip(8 - len as usize).copied().collect();
        }
    }
    let mut v = vec![0x01];
    v.extend(n.to_be_bytes().iter().skip(1));
    v
}

fn el(out: &mut Vec<u8>, id: u32, data: &[u8]) {
    out.extend(id_bytes(id));
    out.extend(vint(data.len() as u64));
    out.extend_from_slice(data);
}

fn el_uint(out: &mut Vec<u8>, id: u32, v: u64) {
    let bytes: Vec<u8> = v.to_be_bytes().iter().skip_while(|b| **b == 0).copied().collect();
    el(out, id, &if bytes.is_empty() { vec![0] } else { bytes });
}

fn el_float(out: &mut Vec<u8>, id: u32, v: f64) {
    el(out, id, &v.to_be_bytes());
}

fn el_str(out: &mut Vec<u8>, id: u32, s: &str) {
    el(out, id, s.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vints_and_ids() {
        assert_eq!(vint(0), vec![0x80]);
        assert_eq!(vint(126), vec![0xFE]);
        assert_eq!(vint(127), vec![0x40, 0x7F]);
        assert_eq!(id_bytes(0xA3), vec![0xA3]);
        assert_eq!(id_bytes(EBML), vec![0x1A, 0x45, 0xDF, 0xA3]);
    }

    #[test]
    fn rejects_bad_input() -> Result<(), String> {
        assert!(WebmWriter::new(0, 4, 30).is_err());
        assert!(WebmWriter::new(4, 0, 30).is_err());
        assert!(WebmWriter::new(4, 4, 0).is_err());
        let mut w = WebmWriter::new(4, 4, 30)?;
        assert!(w.push(&[0u8; 63]).is_err());
        assert!(w.push(&[0u8; 64]).is_ok());
        Ok(())
    }

    #[test]
    fn audio_input_is_checked() {
        assert!(WebmWriter::with_audio(4, 4, 30, &[]).is_err());
        assert!(WebmWriter::with_audio(4, 4, 30, &[0.0; 3]).is_err());
        assert!(WebmWriter::with_audio(4, 4, 30, &[f32::NAN, 2.0]).is_ok());
    }
}
