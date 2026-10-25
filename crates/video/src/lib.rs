//! Encodes RGBA frames to a video file held in memory, with no alpha: WebM (VP9, optional Opus
//! audio) or MP4 (H.264, optional AAC-LC audio).
//!
//! [`WebmWriter`] streams frames one at a time; [`encode_webm`] is the one-shot form. The VP9
//! encoder is the zero-dependency one copied from EffectCraft (`vp9` module), lossy at a fixed
//! quality. The Opus encoder is likewise copied from EffectCraft opusenc (`opus` module).
//! Audio is given up front to [`WebmWriter::with_audio`] / [`Mp4Writer::with_audio`] as
//! interleaved stereo 48 kHz samples and is interleaved with the video; [`WebmWriter::new`] and
//! [`Mp4Writer::new`] write video only. Odd frame sizes are accepted: chroma planes are `ceil(w/2) × ceil(h/2)`, as VP9
//! 4:2:0 allows.
//!
//! [`Mp4Writer`] streams frames into an ISO BMFF file with one H.264 (High profile) track, from
//! the `filmcraft-h264enc` and `filmcraft-isobmff` crates. H.264 needs even sides, so an odd
//! frame is padded to even by repeating its last column and row; the file stores the padded size
//! and nothing is cropped.

mod audio;
// The copied encoders keep their whole API (dead_code), and their index loops as written upstream.
mod mp4;
#[allow(dead_code, clippy::needless_range_loop)]
mod opus;
#[allow(dead_code, clippy::needless_range_loop)]
mod vp9;
mod webm;

pub use mp4::Mp4Writer;
pub use webm::{WebmWriter, encode_webm};
