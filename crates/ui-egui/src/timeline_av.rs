//! 7rrek (Creative Studio): the timeline's camera and sound rows, drawn under the layer rows.

use egui::{Painter, Rect};

use crate::VectorcraftApp;
use crate::theme::Tokens;

/// Where the rows go: the grid's area, the y of the first extra row, row height, the name column's
/// width, the x of frame 1 and a frame's width (both before mirroring), the frame count, and
/// whether the grid is mirrored (right to left: x ↦ area.min.x + area.max.x − x).
#[derive(Clone, Copy, Debug)]
pub struct Geom {
    pub area: Rect,
    pub top: f32,
    pub row_h: f32,
    pub name_w: f32,
    pub x0: f32,
    pub cell_w: f32,
    pub frames: u32,
    pub rtl: bool,
}

/// How many rows [`paint`] draws for the active document.
pub fn rows(_app: &VectorcraftApp) -> usize {
    0
}

pub fn paint(_app: &VectorcraftApp, _p: &Painter, _t: &Tokens, _g: Geom) {}
