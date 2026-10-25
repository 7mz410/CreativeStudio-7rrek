//! Encodes RGBA frames to a video file held in memory, with no audio and no alpha:
//! WebM (VP9) or MP4 (H.264).
//!
//! [`WebmWriter`] streams frames one at a time; [`encode_webm`] is the one-shot form. The VP9
//! encoder is the zero-dependency one copied from EffectCraft (`vp9` module), lossy at a fixed
//! quality. Odd frame sizes are accepted: chroma planes are `ceil(w/2) × ceil(h/2)`, as VP9
//! 4:2:0 allows.
//!
//! [`Mp4Writer`] streams frames into an ISO BMFF file with one H.264 (High profile) track, from
//! the `filmcraft-h264enc` and `filmcraft-isobmff` crates. H.264 needs even sides, so an odd
//! frame is padded to even by repeating its last column and row; the file stores the padded size
//! and nothing is cropped.

// The copied encoder keeps its whole API (dead_code), and its index loops as written upstream.
mod mp4;
#[allow(dead_code, clippy::needless_range_loop)]
mod vp9;
mod webm;

pub use mp4::Mp4Writer;
pub use webm::{WebmWriter, encode_webm};
