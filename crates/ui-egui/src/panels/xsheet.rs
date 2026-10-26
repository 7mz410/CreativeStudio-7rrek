//! Xsheet panel: the exposure sheet, as in Harmony. One column per timeline row, one cell per
//! frame. The cell where a drawing's exposure starts shows its name; the cells after it that hold
//! it show a thin line. A click goes to the frame and makes the column's row current; a
//! double-click types a drawing's name or number into the cell (`anim.expose`).

use egui::{Align2, FontId, Id, Key, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::{Value, json};

use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

/// The panel id, as the dock, the Window menu and `window.panel` name it.
pub const ID: &str = "xsheet";

/// Row height in points: the header and every frame row are this tall.
const CELL_H: f32 = 18.0;
/// Frame-number column width in points.
const FRAME_W: f32 = 44.0;
/// One column's width in points.
const COL_W: f32 = 84.0;

/// A column (timeline row) and the exposures that start in it, by frame.
struct Column {
    row: u64,
    name: String,
    keys: Vec<(u32, String)>,
}

/// The cell being typed into.
#[derive(Clone)]
struct Edit {
    row: u64,
    frame: u32,
    text: String,
}

fn edit_id() -> Id {
    Id::new("xsheet_edit")
}

#[cfg(test)]
fn cells_id() -> Id {
    Id::new("xsheet_cells")
}

fn columns_of(sheet: &Value) -> Vec<Column> {
    sheet["columns"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| {
            let mut keys: Vec<(u32, String)> = c["keys"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|k| Some((u32::try_from(k["frame"].as_u64()?).ok()?, k["name"].as_str().unwrap_or_default().to_string())))
                .collect();
            keys.sort_by_key(|(f, _)| *f);
            Column { row: c["row"].as_u64().unwrap_or_default(), name: c["name"].as_str().unwrap_or_default().to_string(), keys }
        })
        .collect()
}

fn is_key(columns: &[Column], row: u64, frame: u32) -> bool {
    columns.iter().any(|c| c.row == row && c.keys.iter().any(|(f, _)| *f == frame))
}

/// Runs a command; a failure goes to the status bar.
fn run(app: &mut VectorcraftApp, id: &str, params: Value) {
    if let Err(e) = app.run(id, params) {
        app.status(e);
    }
}

/// Types `text` into the cell at `frame` of `row`: exposes the drawing it names or numbers. With
/// no text, clears the exposure there (a key exposure only; an empty cell has nothing to clear).
pub fn commit_text(app: &mut VectorcraftApp, row: u64, frame: u32, text: &str, is_key: bool) -> Result<(), String> {
    let text = text.trim();
    let params = if !text.is_empty() {
        json!({"row": row, "frame": frame, "name": text})
    } else if is_key {
        json!({"row": row, "frame": frame})
    } else {
        return Ok(());
    };
    app.run("anim.expose", params).map(|_| ())
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    if app.session.active().is_none() {
        widgets::dim_label(ui, tl!("No document"));
        return;
    }
    let sheet = app.session.execute("anim.xsheet", &json!({})).unwrap_or(Value::Null);
    let columns = columns_of(&sheet);
    if columns.is_empty() {
        widgets::dim_label(ui, tl!("No drawings yet: add keyframes on the timeline"));
        return;
    }
    let frames = u32::try_from(sheet["frames"].as_u64().unwrap_or(1)).unwrap_or(1);
    let current = u32::try_from(sheet["current"].as_u64().unwrap_or(1)).unwrap_or(1);
    // The current layer's row: its column gets the highlighted cell.
    let cur_row = app.session.execute("anim.drawings", &json!({})).ok().and_then(|v| v["row"].as_u64());
    let t = Tokens::get(ui.ctx());
    let mut edit = ui.ctx().data(|d| d.get_temp::<Option<Edit>>(edit_id())).flatten();
    if ui.input(|i| i.key_pressed(Key::Escape)) {
        edit = None;
    }

    let mut exposure = None;
    ui.horizontal(|ui| {
        if ui.button("+1").on_hover_text(tl!("Increase Exposure")).clicked() {
            exposure = Some(1);
        }
        if ui.button("−1").on_hover_text(tl!("Decrease Exposure")).clicked() {
            exposure = Some(-1);
        }
    });
    if let Some(by) = exposure {
        run(app, "anim.exposure", json!({"by": by}));
    }

    let mut goto = None;
    let mut select = None;
    let mut commits: Vec<(u64, u32, String)> = Vec::new();
    let mut cells: Vec<(u64, u32, Rect)> = Vec::new();
    let rows = usize::try_from(frames).unwrap_or(0) + 1;
    egui::ScrollArea::both().auto_shrink([false, false]).show_rows(ui, CELL_H, rows, |ui, range| {
        for i in range {
            // Row 0 is the header; row `f` is frame `f`.
            let f = u32::try_from(i).unwrap_or(0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                if f == 0 {
                    header_row(ui, &t, &columns);
                    return;
                }
                let (rect, resp) = ui.allocate_exact_size(vec2(FRAME_W, CELL_H), Sense::click());
                ui.painter().rect_filled(rect, 0.0, if f == current { t.row_selected } else { t.panel_darker });
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, f.to_string(), FontId::proportional(11.0), t.text_dim);
                if resp.clicked() {
                    goto = Some(f);
                }
                for col in &columns {
                    let (rect, resp) = ui.allocate_exact_size(vec2(COL_W, CELL_H), Sense::click());
                    cells.push((col.row, f, rect));
                    let on_current = Some(col.row) == cur_row && f == current;
                    let painter = ui.painter();
                    painter.rect_filled(rect, 0.0, if f == current { t.row_selected } else { t.panel });
                    let start = col.keys.iter().find(|(kf, _)| *kf == f);
                    match start {
                        Some((_, name)) => {
                            if !name.is_empty() {
                                painter.text(rect.left_center() + vec2(4.0, 0.0), Align2::LEFT_CENTER, name, FontId::proportional(12.0), t.text);
                            }
                        }
                        None => {
                            // A hold: the drawing that started before this frame, if it shows one.
                            let held = col.keys.iter().rev().find(|(kf, _)| *kf < f).is_some_and(|(_, n)| !n.is_empty());
                            if held {
                                let x = rect.center().x;
                                painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.0, t.text_dim));
                            }
                        }
                    }
                    painter.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, t.divider));
                    if on_current {
                        painter.rect_stroke(rect.shrink(1.0), 0.0, Stroke::new(2.0, t.accent), StrokeKind::Inside);
                    }

                    let editing = edit.as_ref().is_some_and(|e| e.row == col.row && e.frame == f);
                    if editing {
                        let mut finish = None;
                        if let Some(e) = edit.as_mut() {
                            let r = ui.put(rect, egui::TextEdit::singleline(&mut e.text).id_salt(("xsheet_cell", col.row, f)));
                            if r.lost_focus() {
                                finish = Some((e.text.clone(), ui.input(|i| i.key_pressed(Key::Enter))));
                            } else {
                                r.request_focus();
                            }
                        }
                        // Enter commits and moves the edit down a frame; a click elsewhere only commits.
                        if let Some((text, enter)) = finish {
                            edit = None;
                            commits.push((col.row, f, text));
                            if enter && f < frames {
                                edit = Some(Edit { row: col.row, frame: f + 1, text: String::new() });
                            }
                        }
                    } else if resp.double_clicked() {
                        if let Some(old) = edit.take() {
                            commits.push((old.row, old.frame, old.text));
                        }
                        edit = Some(Edit { row: col.row, frame: f, text: String::new() });
                    } else if resp.clicked() {
                        goto = Some(f);
                        select = Some(col.row);
                    }
                }
            });
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(edit_id(), edit));
    // The cells' places, for the tests to click; the app has no use for them.
    #[cfg(test)]
    ui.ctx().data_mut(|d| d.insert_temp(cells_id(), cells));
    #[cfg(not(test))]
    let _ = cells;

    for (row, frame, text) in commits {
        if let Err(e) = commit_text(app, row, frame, &text, is_key(&columns, row, frame)) {
            app.status(e);
        }
    }
    if let Some(f) = goto {
        run(app, "anim.goto", json!({"frame": f}));
    }
    if let Some(row) = select {
        run(app, "layer.setCurrent", json!({"id": row}));
    }
}

/// The header: a blank frame corner, then each column's name.
fn header_row(ui: &mut Ui, t: &Tokens, columns: &[Column]) {
    let (rect, _) = ui.allocate_exact_size(vec2(FRAME_W, CELL_H), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, t.panel_darker);
    for col in columns {
        let (rect, _) = ui.allocate_exact_size(vec2(COL_W, CELL_H), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 0.0, t.panel);
        painter.text(rect.left_center() + vec2(4.0, 0.0), Align2::LEFT_CENTER, &col.name, FontId::proportional(12.0), t.text);
        painter.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, t.divider));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> VectorcraftApp {
        VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default())
    }

    /// One frame of the panel with `events`, in a 400×300 window.
    fn frame(app: &mut VectorcraftApp, ctx: &egui::Context, events: Vec<egui::Event>) {
        let screen = Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(400.0, 300.0)));
        let input = egui::RawInput { screen_rect: screen, events, ..Default::default() };
        ctx.run_ui(input, |ui| show(app, ui)).textures_delta.clear();
    }

    /// Where a cell is on screen, as the last frame laid it out.
    fn cell_center(ctx: &egui::Context, row: u64, frame: u32) -> Option<egui::Pos2> {
        let cells = ctx.data(|d| d.get_temp::<Vec<(u64, u32, Rect)>>(cells_id())).unwrap_or_default();
        cells.iter().find(|(r, f, _)| *r == row && *f == frame).map(|(_, _, rect)| rect.center())
    }

    /// Two rows: drawings at frames 1 and 3 on the first, one at frame 2 on a new second row.
    fn two_rows(app: &mut VectorcraftApp) {
        let s = &mut app.session;
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        s.execute("anim.blankKeyframe", &json!({"frame": 1})).unwrap();
        s.execute("anim.blankKeyframe", &json!({"frame": 3})).unwrap();
        let second = s.execute("layer.new", &json!({"name": "Two", "top": true})).unwrap()["id"].as_u64().unwrap();
        s.execute("anim.blankKeyframe", &json!({"row": second, "frame": 2})).unwrap();
    }

    #[test]
    fn a_two_row_scene_renders() {
        let mut app = app();
        two_rows(&mut app);
        let ctx = egui::Context::default();
        frame(&mut app, &ctx, vec![]);
        let sheet = app.session.execute("anim.xsheet", &json!({})).unwrap();
        assert_eq!(sheet["columns"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn an_empty_sheet_renders_the_empty_state() {
        let mut app = app();
        app.session.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        let ctx = egui::Context::default();
        frame(&mut app, &ctx, vec![]);
    }

    #[test]
    fn a_click_on_a_cell_goes_to_its_frame() {
        let mut app = app();
        two_rows(&mut app);
        let ctx = egui::Context::default();
        frame(&mut app, &ctx, vec![]);
        let sheet = app.session.execute("anim.xsheet", &json!({})).unwrap();
        let row = sheet["columns"][0]["row"].as_u64().unwrap();
        app.session.execute("anim.goto", &json!({"frame": 1})).unwrap();
        assert_eq!(app.session.execute("anim.xsheet", &json!({})).unwrap()["current"].as_u64(), Some(1));
        let at = cell_center(&ctx, row, 2).expect("the cell is laid out");
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at)]);
        frame(&mut app, &ctx, vec![press(true)]);
        frame(&mut app, &ctx, vec![press(false)]);
        frame(&mut app, &ctx, vec![]);
        let sheet = app.session.execute("anim.xsheet", &json!({})).unwrap();
        assert_eq!(sheet["current"].as_u64(), Some(2));
        assert_eq!(app.session.execute("anim.drawings", &json!({})).unwrap()["row"].as_u64(), Some(row));
    }

    #[test]
    fn typing_a_number_exposes_that_drawing() {
        let mut app = app();
        let s = &mut app.session;
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        let ids: Vec<u64> =
            [1, 3, 5].iter().map(|f| s.execute("anim.blankKeyframe", &json!({"frame": f})).unwrap()["id"].as_u64().unwrap()).collect();
        let row = s.execute("anim.xsheet", &json!({})).unwrap()["columns"][0]["row"].as_u64().unwrap();
        // Typed `2` at frame 5: drawing 2 of the row shows from frame 5.
        commit_text(&mut app, row, 5, "2", true).unwrap();
        let sheet = app.session.execute("anim.xsheet", &json!({})).unwrap();
        let at5 = sheet["columns"][0]["keys"].as_array().unwrap().iter().find(|k| k["frame"] == 5).unwrap();
        assert_eq!(at5["drawing"].as_u64(), Some(ids[1]));
        // Empty text on a cell with nothing to clear is a no-op.
        assert!(commit_text(&mut app, row, 4, "", false).is_ok());
    }
}
