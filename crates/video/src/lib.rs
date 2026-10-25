//! Encodes RGBA frames to a WebM file (VP9 video, no audio, no alpha) held in memory.
//!
//! [`WebmWriter`] streams frames one at a time; [`encode_webm`] is the one-shot form. The VP9
//! encoder is the zero-dependency one copied from EffectCraft (`vp9` module), lossy at a fixed
//! quality. Odd frame sizes are accepted: chroma planes are `ceil(w/2) × ceil(h/2)`, as VP9
//! 4:2:0 allows.

// The copied encoder keeps its whole API (dead_code), and its index loops as written upstream.
#[allow(dead_code, clippy::needless_range_loop)]
mod vp9;
mod webm;

pub use webm::{WebmWriter, encode_webm};
