//! MP4 writer for one H.264 video track, built on `filmcraft-h264enc` (the encoder) and
//! `filmcraft-isobmff` (the ISO BMFF muxer).
//!
//! The encoder runs with no B-frames, so presentation and decode order match and the file needs
//! no edit list. The track timescale is the frame rate, and every sample lasts one tick.

use std::io::Cursor;

use filmcraft_h264enc::{Encoder, EncoderConfig, Packet, PacketFormat, RateControl, YuvFrame};
use filmcraft_isobmff::{AvcConfig, Brand, Mp4Writer as Muxer, SampleEntry, TrackConfig, WriteSample, WriterOptions};

use crate::vp9::rgba_to_yuv420;

/// H.264 level limit per side.
const MAX_DIM: u32 = 16_384;
const MAX_FPS: u32 = 1_000;
/// Bitrate in kbit/s per pixel-second, as a divisor: 0.1 bit per pixel per frame.
const PIXELS_PER_KBPS_DIVISOR: u64 = 10_000;
const MIN_KBPS: u64 = 200;
const MAX_KBPS: u64 = 1_000_000;

/// Streams RGBA frames into an MP4 byte buffer. Call [`push`](Self::push) per frame, then
/// [`finish`](Self::finish).
pub struct Mp4Writer {
    width: u32,
    height: u32,
    /// `width` and `height` rounded up to even, the size the encoder and the file use.
    coded_width: u32,
    coded_height: u32,
    encoder: Encoder,
    mux: Muxer<Cursor<Vec<u8>>>,
    track: usize,
    frames: u64,
}

impl Mp4Writer {
    /// A writer for `width` × `height` frames at `fps` frames per second.
    pub fn new(width: u32, height: u32, fps: u32) -> Result<Mp4Writer, String> {
        if width == 0 || height == 0 || width > MAX_DIM || height > MAX_DIM {
            return Err(format!("frame size {width}x{height} is out of range (1..={MAX_DIM} per side)"));
        }
        if fps == 0 || fps > MAX_FPS {
            return Err(format!("fps {fps} is out of range (1..={MAX_FPS})"));
        }
        // Odd sides are padded to even (see `pad_to_even`); the file declares the padded size.
        let coded_width = width + (width & 1);
        let coded_height = height + (height & 1);

        let kbps = (u64::from(coded_width) * u64::from(coded_height) * u64::from(fps) / PIXELS_PER_KBPS_DIVISOR).clamp(MIN_KBPS, MAX_KBPS);
        let kbps = kbps as u32;
        let mut cfg = EncoderConfig::new(coded_width, coded_height, fps, 1);
        cfg.format = PacketFormat::LengthPrefixed;
        cfg.aud = false;
        cfg.bframes = 0;
        cfg.keyint = fps.saturating_mul(2);
        cfg.rate = RateControl::Vbr { target_kbps: kbps, max_kbps: kbps.saturating_mul(3) / 2 };
        let encoder = Encoder::new(cfg).map_err(|e| format!("H.264 encoder: {e}"))?;

        let avc = AvcConfig::parse(&encoder.avcc()).map_err(|e| format!("H.264 parameter sets: {e}"))?;
        let (cw, ch) =
            (u16::try_from(coded_width).map_err(|_| "frame width too large")?, u16::try_from(coded_height).map_err(|_| "frame height too large")?);
        let mut mux = Muxer::new(Cursor::new(Vec::new()), WriterOptions::new(Brand::Mp4)).map_err(|e| format!("MP4 muxer: {e}"))?;
        let track = mux.add_track(TrackConfig::new(SampleEntry::avc(avc, cw, ch), fps)).map_err(|e| format!("MP4 track: {e}"))?;

        Ok(Mp4Writer { width, height, coded_width, coded_height, encoder, mux, track, frames: 0 })
    }

    /// Encodes one frame: `rgba` is `width × height × 4` bytes, straight (not premultiplied)
    /// RGBA, rows top to bottom. Alpha is ignored.
    pub fn push(&mut self, rgba: &[u8]) -> Result<(), String> {
        let expected = (self.width as usize).checked_mul(self.height as usize).and_then(|n| n.checked_mul(4)).ok_or("frame size overflows usize")?;
        if rgba.len() != expected {
            return Err(format!("frame {} is {} bytes, expected {expected} for {}x{} RGBA", self.frames, rgba.len(), self.width, self.height));
        }
        let (y, u, v) = if (self.coded_width, self.coded_height) == (self.width, self.height) {
            rgba_to_yuv420(rgba, self.width, self.height, false)
        } else {
            let padded = pad_to_even(rgba, self.width, self.height, self.coded_width, self.coded_height)?;
            rgba_to_yuv420(&padded, self.coded_width, self.coded_height, false)
        };
        let frame = YuvFrame { y: &y, u: &u, v: &v, y_stride: self.coded_width as usize, uv_stride: (self.coded_width / 2) as usize };
        let packets = self.encoder.try_encode(&frame, self.frames as i64).map_err(|e| format!("H.264 encode: {e}"))?;
        self.write(packets)?;
        self.frames += 1;
        Ok(())
    }

    /// Flushes the encoder and returns the finished MP4 file. Fails if no frame was pushed.
    pub fn finish(mut self) -> Result<Vec<u8>, String> {
        if self.frames == 0 {
            return Err("no frames pushed".into());
        }
        let packets = self.encoder.flush();
        self.write(packets)?;
        let cursor = self.mux.finish().map_err(|e| format!("MP4 finish: {e}"))?;
        Ok(cursor.into_inner())
    }

    fn write(&mut self, packets: Vec<Packet>) -> Result<(), String> {
        let track = self.track;
        for p in packets {
            let composition_offset = i32::try_from(p.pts - p.dts).map_err(|_| "composition offset out of range")?;
            let sample = WriteSample { data: &p.data, duration: 1, composition_offset, is_sync: p.keyframe };
            self.mux.write_sample(track, sample).map_err(|e| format!("MP4 sample: {e}"))?;
        }
        Ok(())
    }
}

/// Pads an RGBA frame to even sides by repeating its last column and row. Chroma then samples
/// the padding like any other pixel, so the edge stays as the source had it.
fn pad_to_even(rgba: &[u8], width: u32, height: u32, coded_width: u32, coded_height: u32) -> Result<Vec<u8>, String> {
    let (w, h, cw, ch) = (width as usize, height as usize, coded_width as usize, coded_height as usize);
    let row_bytes = w.checked_mul(4).ok_or("frame row overflows usize")?;
    let mut out = Vec::with_capacity(cw.checked_mul(ch).and_then(|n| n.checked_mul(4)).ok_or("padded frame overflows usize")?);
    for y in 0..ch {
        let sy = y.min(h.saturating_sub(1));
        let row = sy.checked_mul(row_bytes).and_then(|start| rgba.get(start..start + row_bytes)).ok_or("frame is shorter than its size")?;
        for x in 0..cw {
            let sx = x.min(w.saturating_sub(1)) * 4;
            let px = row.get(sx..sx + 4).ok_or("frame is shorter than its size")?;
            out.extend_from_slice(px);
        }
    }
    Ok(out)
}
