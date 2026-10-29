//! Palettes panel: the document's palettes, as in Harmony. A palette recolours the global swatches
//! it names. The list picks one to edit; each global swatch then gets a colour in it (a chip, a
//! picker, Reset back to the swatch's own), and the palette can show in the whole scene or on the
//! current layer's row.

use egui::{Id, Ui};
use serde_json::{Value, json};
use vectorcraft_color::Color;

use crate::VectorcraftApp;
use crate::widgets;

/// The panel id, as the dock, the Window menu and `window.panel` name it.
pub const ID: &str = "palettes";

/// A click, run once the panel is drawn.
enum Act {
    Pick(usize),
    New,
    Duplicate(usize),
    Delete(usize),
    Colour { palette: usize, swatch: String, color: Option<String> },
    Use { palette: Option<usize>, scene: bool },
    Protect { swatch: String, on: bool },
    Recover,
}

/// The palette picked in the list, by index (panel-local; not in the document).
fn picked_id() -> Id {
    Id::new("palettes_picked")
}

fn set_picked(ctx: &egui::Context, i: Option<usize>) {
    ctx.data_mut(|d| d.insert_temp(picked_id(), i));
}

/// Runs a command; a failure goes to the status bar and gives none.
fn run(app: &mut VectorcraftApp, id: &str, params: Value) -> Option<Value> {
    match app.run(id, params) {
        Ok(v) => Some(v),
        Err(e) => {
            app.status(e);
            None
        }
    }
}

/// The index a `palette.clone` answer names.
fn index_of(v: &Value) -> Option<usize> {
    v["index"].as_u64().and_then(|i| usize::try_from(i).ok())
}

/// Sets palette `palette`'s colour of global swatch `swatch` to `color` (a hex), or back to the
/// swatch's own with None (`palette.setColor`).
pub(crate) fn set_colour(app: &mut VectorcraftApp, palette: usize, swatch: &str, color: Option<&str>) -> Result<Value, String> {
    app.run("palette.setColor", json!({"palette": palette, "swatch": swatch, "color": color}))
}

fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    if app.session.active().is_none() {
        widgets::dim_label(ui, tl!("No document"));
        return;
    }
    let list = app.session.execute("palette.list", &json!({})).unwrap_or(Value::Null);
    let palettes = list["palettes"].as_array().cloned().unwrap_or_default();
    let swatches: Vec<String> = list["swatches"].as_array().into_iter().flatten().filter_map(|s| s.as_str().map(str::to_string)).collect();
    let protected: std::collections::BTreeSet<String> =
        list["protected"].as_array().into_iter().flatten().filter_map(|s| s.as_str().map(str::to_string)).collect();
    let scene = list["scene"].as_u64().and_then(|i| usize::try_from(i).ok());
    // The current layer's row, and the palette it shows (none without a row).
    let layer_row = app.session.execute("anim.drawings", &json!({})).ok().and_then(|v| v["row"].as_u64());
    let layer_palette = layer_row.and_then(|r| list["rows"][r.to_string()].as_u64()).and_then(|i| usize::try_from(i).ok());
    // Each global swatch's own colour, for the palettes that leave it out.
    let own: std::collections::BTreeMap<String, Color> = app
        .session
        .execute("swatch.list", &json!({}))
        .ok()
        .map(|v| {
            v["swatches"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|s| Some((s["name"].as_str()?.to_string(), Color::from_hex(s["hex"].as_str()?)?)))
                .collect()
        })
        .unwrap_or_default();
    let picked = ui.ctx().data(|d| d.get_temp::<Option<usize>>(picked_id())).flatten().filter(|i| *i < palettes.len());
    let mut acts: Vec<Act> = Vec::new();

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal(|ui| {
            if widgets::primary_button_enabled(ui, "New Palette", !swatches.is_empty()).clicked() {
                acts.push(Act::New);
            }
            if let Some(i) = picked {
                if widgets::secondary_button(ui, "Duplicate").clicked() {
                    acts.push(Act::Duplicate(i));
                }
                if widgets::secondary_button(ui, "Delete").clicked() {
                    acts.push(Act::Delete(i));
                }
            }
            if widgets::secondary_button(ui, tl!("Recover Missing Swatches")).clicked() {
                acts.push(Act::Recover);
            }
        });
        if swatches.is_empty() {
            widgets::dim_label(ui, tl!("Make a swatch Global to recolour it with palettes"));
        } else if palettes.is_empty() {
            widgets::dim_label(ui, tl!("No palettes yet: New Palette clones the global swatches"));
        }
        if !palettes.is_empty() {
            widgets::list_box(ui, |ui| {
                for (i, p) in palettes.iter().enumerate() {
                    let name = p["name"].as_str().unwrap_or_default();
                    ui.horizontal(|ui| {
                        if ui.selectable_label(picked == Some(i), name).clicked() {
                            acts.push(Act::Pick(i));
                        }
                        if scene == Some(i) {
                            widgets::dim_label(ui, tl!("Scene"));
                        }
                        if layer_palette == Some(i) {
                            widgets::dim_label(ui, tl!("Layer"));
                        }
                    });
                }
            });
        }

        if let Some(i) = picked {
            let colors = palettes[i]["colors"].as_object().cloned().unwrap_or_default();
            ui.add_space(8.0);
            widgets::divider(ui);
            for swatch in &swatches {
                let mine = colors.get(swatch).and_then(|v| serde_json::from_value::<Color>(v.clone()).ok());
                let Some(shown) = mine.or_else(|| own.get(swatch).copied()) else { continue };
                let [r, g, b, _] = shown.to_rgba8(1.0);
                let is_protected = protected.contains(swatch);
                ui.horizontal(|ui| {
                    // shortcut: every step of a picker drag is its own undo step; commit once when
                    // the picker closes if that clutters History.
                    let mut rgb = [r, g, b];
                    let picker = ui.color_edit_button_srgb(&mut rgb);
                    // A swatch this palette leaves out: its name dim, the chip its own colour.
                    if mine.is_some() {
                        ui.label(swatch.as_str());
                    } else {
                        widgets::dim_label(ui, swatch.as_str());
                    }
                    if picker.changed() {
                        acts.push(Act::Colour { palette: i, swatch: swatch.clone(), color: Some(hex(rgb)) });
                    }
                    if ui.add_enabled(mine.is_some(), egui::Button::new(tl!("Reset")).small()).clicked() {
                        acts.push(Act::Colour { palette: i, swatch: swatch.clone(), color: None });
                    }
                    let mut prot = is_protected;
                    if ui.checkbox(&mut prot, tl!("Protected")).changed() {
                        acts.push(Act::Protect { swatch: swatch.clone(), on: prot });
                    }
                });
            }
            ui.add_space(8.0);
            let on = scene == Some(i);
            if widgets::check(ui, "Show in Scene", on, true) {
                acts.push(Act::Use { palette: if on { None } else { Some(i) }, scene: true });
            }
            let on = layer_palette == Some(i);
            if widgets::check(ui, "Show on This Layer", on, layer_row.is_some()) {
                acts.push(Act::Use { palette: if on { None } else { Some(i) }, scene: false });
            }
        } else if !swatches.is_empty() {
            ui.add_space(8.0);
            widgets::divider(ui);
            for swatch in &swatches {
                let is_protected = protected.contains(swatch);
                ui.horizontal(|ui| {
                    // A chip, not an editor: swatch colours are edited in the Swatches panel.
                    if let Some(col) = own.get(swatch) {
                        let [r, g, b, _] = col.to_rgba8(1.0);
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgb(r, g, b));
                    }
                    ui.label(swatch.as_str());
                    let mut prot = is_protected;
                    if ui.checkbox(&mut prot, tl!("Protected")).changed() {
                        acts.push(Act::Protect { swatch: swatch.clone(), on: prot });
                    }
                });
            }
        }
    });

    for act in acts {
        match act {
            Act::Pick(i) => set_picked(ui.ctx(), Some(i)),
            Act::New => {
                if let Some(v) = run(app, "palette.clone", json!({})) {
                    set_picked(ui.ctx(), index_of(&v));
                }
            }
            Act::Duplicate(from) => {
                if let Some(v) = run(app, "palette.clone", json!({"from": from})) {
                    set_picked(ui.ctx(), index_of(&v));
                }
            }
            Act::Delete(i) => {
                if run(app, "palette.remove", json!({"palette": i})).is_some() {
                    set_picked(ui.ctx(), None);
                }
            }
            Act::Colour { palette, swatch, color } => {
                if let Err(e) = set_colour(app, palette, &swatch, color.as_deref()) {
                    app.status(e);
                }
            }
            Act::Use { palette, scene } => {
                let mut params = json!({"palette": palette});
                if scene {
                    params["scene"] = json!(true);
                }
                run(app, "palette.use", params);
            }
            Act::Protect { swatch, on } => {
                run(app, "palette.protect", json!({"name": swatch, "on": on}));
            }
            Act::Recover => {
                run(app, "palette.recover", json!({}));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A context with the app's fonts and theme, as a frame of the app has them.
    fn ctx() -> egui::Context {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx, Default::default());
        ctx
    }

    fn app() -> VectorcraftApp {
        VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default())
    }

    /// One frame of the panel in a 400×300 window.
    fn frame(app: &mut VectorcraftApp, ctx: &egui::Context) {
        let screen = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 300.0)));
        let input = egui::RawInput { screen_rect: screen, ..Default::default() };
        ctx.run_ui(input, |ui| show(app, ui)).textures_delta.clear();
    }

    /// A document with one global swatch, "Skin".
    fn scene() -> VectorcraftApp {
        let mut app = app();
        app.session.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        app.session.execute("swatch.new", &json!({"name": "Skin", "color": "#ffcc99", "global": true})).unwrap();
        app
    }

    /// The colour of `swatch` in palette `palette` as `palette.list` shows it, as hex.
    fn listed(app: &mut VectorcraftApp, palette: usize, swatch: &str) -> Option<String> {
        let list = app.session.execute("palette.list", &json!({})).unwrap();
        let v = list["palettes"][palette]["colors"].get(swatch)?;
        Some(serde_json::from_value::<Color>(v.clone()).unwrap().to_hex().to_lowercase())
    }

    #[test]
    fn renders_with_no_palettes() {
        let mut app = app();
        app.session.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        let ctx = ctx();
        frame(&mut app, &ctx);
        assert!(app.session.execute("palette.list", &json!({})).unwrap()["palettes"].as_array().unwrap().is_empty());
        // The same with a global swatch, which the empty-state message then names.
        let mut app = scene();
        frame(&mut app, &ctx);
        assert!(app.session.execute("palette.list", &json!({})).unwrap()["palettes"].as_array().unwrap().is_empty());
    }

    #[test]
    fn renders_one_palette_over_a_global_swatch() {
        let mut app = scene();
        app.session.execute("palette.clone", &json!({"name": "Night"})).unwrap();
        app.session.execute("palette.use", &json!({"palette": 0, "scene": true})).unwrap();
        let ctx = ctx();
        set_picked(&ctx, Some(0));
        frame(&mut app, &ctx);
        assert_eq!(listed(&mut app, 0, "Skin").as_deref(), Some("#ffcc99"), "a clone holds the swatch's colour");
    }

    #[test]
    fn set_colour_shows_in_the_list_and_reset_takes_it_back() {
        let mut app = scene();
        app.session.execute("palette.clone", &json!({"name": "Night"})).unwrap();
        set_colour(&mut app, 0, "Skin", Some("#334466")).unwrap();
        assert_eq!(listed(&mut app, 0, "Skin").as_deref(), Some("#334466"));
        set_colour(&mut app, 0, "Skin", None).unwrap();
        assert_eq!(listed(&mut app, 0, "Skin"), None, "Reset leaves the swatch's own colour");
    }
}
