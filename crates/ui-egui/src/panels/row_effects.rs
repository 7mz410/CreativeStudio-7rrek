//! Row Effects panel: stack and animate blur, glow, drop shadow, and colour adjust on the
//! timeline row of the active layer.

use std::collections::BTreeMap;

use egui::{DragValue, RichText, TextEdit, Ui};
use serde_json::{Value, json};
use vectorcraft_engine::cmd::anim::Anim;
use vectorcraft_engine::cmd::row_fx::{FxKind, RowEffect};

use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

pub const ID: &str = "rowEffects";

/// Actions triggered by the UI, run once the panel is drawn.
enum Act {
    Add(&'static str),
    Key { index: usize, params: Value },
    RemoveKey { index: usize },
    Remove { index: usize },
}

/// Runs a command; failures are routed to the status bar.
fn run(app: &mut VectorcraftApp, id: &str, params: Value) {
    if let Err(e) = app.run(id, params) {
        app.status(e);
    }
}

/// Parses a 6-digit hex string `#rrggbb` or `rrggbb` into `[r, g, b]`.
fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let s = s.trim().strip_prefix('#').unwrap_or(s.trim());
    if s.len() == 6 {
        let r = u8::from_str_radix(s.get(0..2)?, 16).ok()?;
        let g = u8::from_str_radix(s.get(2..4)?, 16).ok()?;
        let b = u8::from_str_radix(s.get(4..6)?, 16).ok()?;
        Some([r, g, b])
    } else {
        None
    }
}

/// Formats `[r, g, b]` into `#rrggbb`.
fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

/// Menu button allowing the user to append an effect to the row.
fn add_effect_menu(ui: &mut Ui, acts: &mut Vec<Act>) {
    ui.menu_button(tl!("Add Effect"), |ui| {
        if ui.button(tl!("Blur")).clicked() {
            acts.push(Act::Add("blur"));
            ui.close();
        }
        if ui.button(tl!("Glow")).clicked() {
            acts.push(Act::Add("glow"));
            ui.close();
        }
        if ui.button(tl!("Drop Shadow")).clicked() {
            acts.push(Act::Add("dropShadow"));
            ui.close();
        }
        if ui.button(tl!("Color Adjust")).clicked() {
            acts.push(Act::Add("colorAdjust"));
            ui.close();
        }
    });
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    if app.session.active().is_none() {
        widgets::dim_label(ui, tl!("No document"));
        return;
    }

    let current_frame = match app.session.active() {
        Some(st) => Anim::of(&st.doc).current,
        None => 0,
    };

    let list_res = app.session.execute("fx.list", &json!({}));
    let effects_val = list_res.ok().unwrap_or(Value::Null);
    let effects = effects_val.get("effects").and_then(Value::as_array).cloned().unwrap_or_default();

    let mut acts: Vec<Act> = Vec::new();
    let t = Tokens::get(ui.ctx());

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal(|ui| {
            add_effect_menu(ui, &mut acts);
        });

        ui.add_space(4.0);

        if effects.is_empty() {
            widgets::dim_label(ui, tl!("No effects on this row"));
            return;
        }

        for (i, fx_val) in effects.iter().enumerate() {
            let kind_str = fx_val.get("kind").and_then(Value::as_str).unwrap_or("blur");
            let kind = FxKind::parse(kind_str).unwrap_or(FxKind::Blur);

            let mut keys = BTreeMap::new();
            if let Some(obj) = fx_val.get("keys").and_then(Value::as_object) {
                for (k, v) in obj {
                    if let Ok(frame_num) = k.parse::<u32>() {
                        keys.insert(frame_num, v.clone());
                    }
                }
            }

            let has_key = keys.contains_key(&current_frame);
            let row_fx = RowEffect { kind, keys };
            let mut params = row_fx.params_at(current_frame);

            let kind_label = match kind {
                FxKind::Blur => tl!("Blur"),
                FxKind::Glow => tl!("Glow"),
                FxKind::DropShadow => tl!("Drop Shadow"),
                FxKind::ColorAdjust => tl!("Color Adjust"),
            };

            let open_id = ui.id().with(("fx_open", i));
            let mut open = ui.data_mut(|d| *d.get_temp_mut_or(open_id, true));

            widgets::divider(ui);

            ui.horizontal(|ui| {
                let icon = if open { "chevron-down" } else { "chevron-right" };
                if widgets::icon_button(ui, icon, if open { "Collapse" } else { "Expand" }, false, 18.0).clicked() {
                    open = !open;
                    ui.data_mut(|d| d.insert_temp(open_id, open));
                }

                ui.label(RichText::new(kind_label).strong());

                let marker_tip = tl!("Key at this frame");
                let marker_text = if has_key { "◆" } else { "◇" };
                let marker_color = if has_key { t.accent } else { t.text_dim };
                if ui.small_button(RichText::new(marker_text).color(marker_color)).on_hover_text(marker_tip).clicked() && !has_key {
                    acts.push(Act::Key { index: i, params: params.clone() });
                }

                if ui.add_enabled(has_key, egui::Button::new(tl!("Remove Key")).small()).clicked() {
                    acts.push(Act::RemoveKey { index: i });
                }

                if widgets::secondary_button(ui, tl!("Remove")).clicked() {
                    acts.push(Act::Remove { index: i });
                }
            });

            if open {
                ui.indent(open_id, |ui| match kind {
                    FxKind::Blur => {
                        let mut radius =
                            params.get("radius").and_then(Value::as_f64).or_else(|| params.get("blur").and_then(Value::as_f64)).unwrap_or(5.0);
                        ui.horizontal(|ui| {
                            ui.label(tl!("Radius:"));
                            if ui.add(DragValue::new(&mut radius).speed(0.1).range(0.0..=100.0)).changed() {
                                params["radius"] = json!(radius);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });
                    }
                    FxKind::Glow => {
                        let mut blur =
                            params.get("blur").and_then(Value::as_f64).or_else(|| params.get("radius").and_then(Value::as_f64)).unwrap_or(5.0);
                        let mut opacity = params.get("opacity").and_then(Value::as_f64).unwrap_or(75.0);
                        let color_str = params.get("color").and_then(Value::as_str).unwrap_or("#ffff00").to_string();

                        ui.horizontal(|ui| {
                            ui.label(tl!("Radius:"));
                            if ui.add(DragValue::new(&mut blur).speed(0.1).range(0.0..=100.0)).changed() {
                                params["blur"] = json!(blur);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label(tl!("Opacity:"));
                            if ui.add(DragValue::new(&mut opacity).speed(1.0).range(0.0..=100.0)).changed() {
                                params["opacity"] = json!(opacity);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label(tl!("Color:"));
                            let mut rgb = parse_hex(&color_str).unwrap_or([255, 255, 0]);
                            if ui.color_edit_button_srgb(&mut rgb).changed() {
                                let new_hex = hex(rgb);
                                params["color"] = json!(new_hex);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }

                            let text_id = ui.id().with(("glow_hex", i));
                            let mut text_hex = ui.data_mut(|d| d.get_temp::<String>(text_id)).unwrap_or_else(|| color_str.clone());
                            if ui.add(TextEdit::singleline(&mut text_hex).desired_width(70.0)).changed() {
                                ui.data_mut(|d| d.insert_temp(text_id, text_hex.clone()));
                                if parse_hex(&text_hex).is_some() {
                                    let normalized = if text_hex.starts_with('#') { text_hex.clone() } else { format!("#{text_hex}") };
                                    params["color"] = json!(normalized);
                                    acts.push(Act::Key { index: i, params: params.clone() });
                                }
                            }
                        });
                    }
                    FxKind::DropShadow => {
                        let mut x = params.get("x").and_then(Value::as_f64).unwrap_or(7.0);
                        let mut y = params.get("y").and_then(Value::as_f64).unwrap_or(7.0);
                        let mut blur =
                            params.get("blur").and_then(Value::as_f64).or_else(|| params.get("radius").and_then(Value::as_f64)).unwrap_or(5.0);
                        let mut opacity = params.get("opacity").and_then(Value::as_f64).unwrap_or(75.0);
                        let color_str = params.get("color").and_then(Value::as_str).unwrap_or("#000000").to_string();

                        ui.horizontal(|ui| {
                            ui.label(tl!("X Offset:"));
                            if ui.add(DragValue::new(&mut x).speed(0.5)).changed() {
                                params["x"] = json!(x);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                            ui.label(tl!("Y Offset:"));
                            if ui.add(DragValue::new(&mut y).speed(0.5)).changed() {
                                params["y"] = json!(y);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label(tl!("Blur:"));
                            if ui.add(DragValue::new(&mut blur).speed(0.1).range(0.0..=100.0)).changed() {
                                params["blur"] = json!(blur);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label(tl!("Opacity:"));
                            if ui.add(DragValue::new(&mut opacity).speed(1.0).range(0.0..=100.0)).changed() {
                                params["opacity"] = json!(opacity);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label(tl!("Color:"));
                            let mut rgb = parse_hex(&color_str).unwrap_or([0, 0, 0]);
                            if ui.color_edit_button_srgb(&mut rgb).changed() {
                                let new_hex = hex(rgb);
                                params["color"] = json!(new_hex);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }

                            let text_id = ui.id().with(("shadow_hex", i));
                            let mut text_hex = ui.data_mut(|d| d.get_temp::<String>(text_id)).unwrap_or_else(|| color_str.clone());
                            if ui.add(TextEdit::singleline(&mut text_hex).desired_width(70.0)).changed() {
                                ui.data_mut(|d| d.insert_temp(text_id, text_hex.clone()));
                                if parse_hex(&text_hex).is_some() {
                                    let normalized = if text_hex.starts_with('#') { text_hex.clone() } else { format!("#{text_hex}") };
                                    params["color"] = json!(normalized);
                                    acts.push(Act::Key { index: i, params: params.clone() });
                                }
                            }
                        });
                    }
                    FxKind::ColorAdjust => {
                        let mut brightness = params.get("brightness").and_then(Value::as_f64).unwrap_or(0.0);
                        let mut contrast = params.get("contrast").and_then(Value::as_f64).unwrap_or(0.0);

                        ui.horizontal(|ui| {
                            ui.label(tl!("Brightness:"));
                            if ui.add(DragValue::new(&mut brightness).speed(1.0).range(-100.0..=100.0)).changed() {
                                params["brightness"] = json!(brightness);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label(tl!("Contrast:"));
                            if ui.add(DragValue::new(&mut contrast).speed(1.0).range(-100.0..=100.0)).changed() {
                                params["contrast"] = json!(contrast);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });
                    }
                });
            }
        }
    });

    for act in acts {
        match act {
            Act::Add(kind) => {
                run(app, "fx.add", json!({"kind": kind}));
            }
            Act::Key { index, params } => {
                run(app, "fx.key", json!({"index": index, "frame": current_frame, "params": params}));
            }
            Act::RemoveKey { index } => {
                run(app, "fx.removeKey", json!({"index": index, "frame": current_frame}));
            }
            Act::Remove { index } => {
                run(app, "fx.remove", json!({"index": index}));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> VectorcraftApp {
        VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default())
    }

    /// One frame of the panel in a 400×300 window.
    fn frame(app: &mut VectorcraftApp, ctx: &egui::Context) {
        let screen = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 300.0)));
        let input = egui::RawInput { screen_rect: screen, ..Default::default() };
        ctx.run_ui(input, |ui| show(app, ui)).textures_delta.clear();
    }

    fn anim_doc() -> VectorcraftApp {
        let mut app = app();
        app.session.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        app.session.execute("anim.setFrames", &json!({"frames": 12})).unwrap();
        app
    }

    fn ctx() -> egui::Context {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx, Default::default());
        ctx
    }

    #[test]
    fn a_hex_field_with_arabic_letters_is_not_a_colour() {
        assert_eq!(parse_hex("#ابج"), None);
        assert_eq!(parse_hex("ff8000"), Some([255, 128, 0]));
    }

    #[test]
    fn no_document_shows_without_panic() {
        let mut app = app();
        let ctx = ctx();
        frame(&mut app, &ctx);
    }

    #[test]
    fn no_effects_shows_without_panic() {
        let mut app = anim_doc();
        let ctx = ctx();
        frame(&mut app, &ctx);
        let list = app.session.execute("fx.list", &json!({})).unwrap();
        assert!(list["effects"].as_array().unwrap().is_empty());
    }

    #[test]
    fn glow_keyed_at_frame_1_shows_without_panic() {
        let mut app = anim_doc();
        app.session.execute("fx.add", &json!({"kind": "glow"})).unwrap();
        app.session
            .execute(
                "fx.key",
                &json!({
                    "index": 0,
                    "frame": 1,
                    "params": {"blur": 8.0, "color": "#ff00ff", "opacity": 80.0, "mode": "screen"}
                }),
            )
            .unwrap();
        let ctx = ctx();
        frame(&mut app, &ctx);
        let list = app.session.execute("fx.list", &json!({})).unwrap();
        assert_eq!(list["effects"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn add_path_runs_and_shows_in_list() {
        let mut app = anim_doc();
        app.run("fx.add", json!({"kind": "blur"})).unwrap();
        let list = app.session.execute("fx.list", &json!({})).unwrap();
        let effects = list["effects"].as_array().unwrap();
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0]["kind"], "blur");
    }
}
