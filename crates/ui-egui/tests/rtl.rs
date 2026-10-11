//! Creative Studio: the right-to-left switch in the vendored egui (`vendor/egui`, `egui::set_rtl`). It is
//! process-wide, so it is tested here, in its own test binary, rather than among the unit tests.
#![allow(clippy::unwrap_used)]

use egui::{Rect, pos2, vec2};

/// The rects of the widgets a frame of `add` places, in order.
fn frame(add: impl Fn(&mut egui::Ui, &mut Vec<Rect>)) -> Vec<Rect> {
    let ctx = egui::Context::default();
    let mut rects = vec![];
    // Two passes: grids size their columns from the previous frame.
    for _ in 0..2 {
        rects.clear();
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))), ..Default::default() };
        let mut out = ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| add(ui, &mut rects));
        });
        out.textures_delta.clear();
    }
    rects
}

fn label(ui: &mut egui::Ui, rects: &mut Vec<Rect>, text: &str) {
    rects.push(ui.label(text).rect);
}

#[test]
fn rtl_mirrors_rows_grids_columns_and_alignment() {
    for rtl in [true, false] {
        egui::set_rtl(rtl);
        assert_eq!(egui::is_rtl(), rtl);
        // First before second: right of it when right to left.
        let first_is_right = |r: &[Rect]| r[0].center().x > r[1].center().x;

        let row = frame(|ui, r| {
            ui.horizontal(|ui| {
                label(ui, r, "one");
                label(ui, r, "two");
            });
        });
        assert_eq!(first_is_right(&row), rtl, "horizontal, rtl {rtl}");

        let grid = frame(|ui, r| {
            egui::Grid::new("g").show(ui, |ui| {
                label(ui, r, "name");
                label(ui, r, "value");
                ui.end_row();
            });
        });
        assert_eq!(first_is_right(&grid), rtl, "grid, rtl {rtl}");

        let cols = frame(|ui, r| {
            ui.columns(2, |c| {
                label(&mut c[0], r, "a");
                label(&mut c[1], r, "b");
            });
        });
        assert_eq!(first_is_right(&cols), rtl, "columns, rtl {rtl}");

        // A short label in a vertical layout hugs the start edge.
        let v = frame(|ui, r| {
            ui.vertical(|ui| label(ui, r, "x"));
        });
        assert_eq!(v[0].center().x > 400.0, rtl, "vertical alignment, rtl {rtl}");
    }
    egui::set_rtl(false);
}
