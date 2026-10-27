//! Row Effects panel: stack and animate blur, glow, drop shadow, and colour adjust on the
//! timeline row of the active layer.

use std::collections::BTreeMap;

use egui::{DragValue, RichText, TextEdit, Ui};
use serde_json::{Value, json};
use vectorcraft_engine::cmd::anim::Anim;
use vectorcraft_engine::cmd::row_fx::{FxKind, RowEffect};

use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets::{self, Live};

pub const ID: &str = "rowEffects";
/// A value drag is open (its undo step not yet committed).
const LIVE: &str = "rowEffects.live";

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
        // Blurs
        if ui.button(tl!("Blur")).clicked() {
            acts.push(Act::Add("blur"));
            ui.close();
        }
        if ui.button(tl!("Box Blur")).clicked() {
            acts.push(Act::Add("box"));
            ui.close();
        }
        if ui.button(tl!("Directional Blur")).clicked() {
            acts.push(Act::Add("directional"));
            ui.close();
        }
        if ui.button(tl!("Radial Blur")).clicked() {
            acts.push(Act::Add("radial"));
            ui.close();
        }
        if ui.button(tl!("Radial Zoom Blur")).clicked() {
            acts.push(Act::Add("radialZoom"));
            ui.close();
        }
        ui.separator();
        // Light & Shadow
        if ui.button(tl!("Glow")).clicked() {
            acts.push(Act::Add("glow"));
            ui.close();
        }
        if ui.button(tl!("Drop Shadow")).clicked() {
            acts.push(Act::Add("dropShadow"));
            ui.close();
        }
        ui.separator();
        // Color & Tone
        if ui.button(tl!("Color Adjust")).clicked() {
            acts.push(Act::Add("colorAdjust"));
            ui.close();
        }
        if ui.button(tl!("Transparency")).clicked() {
            acts.push(Act::Add("transparency"));
            ui.close();
        }
        if ui.button(tl!("Greyscale")).clicked() {
            acts.push(Act::Add("greyscale"));
            ui.close();
        }
        if ui.button(tl!("Levels")).clicked() {
            acts.push(Act::Add("levels"));
            ui.close();
        }
        if ui.button(tl!("Contrast")).clicked() {
            acts.push(Act::Add("contrast"));
            ui.close();
        }
        if ui.button(tl!("Gamma")).clicked() {
            acts.push(Act::Add("gamma"));
            ui.close();
        }
        if ui.button(tl!("Hue / Saturation")).clicked() {
            acts.push(Act::Add("hueSaturation"));
            ui.close();
        }
        if ui.button(tl!("Colour Scale")).clicked() {
            acts.push(Act::Add("colourScale"));
            ui.close();
        }
        if ui.button(tl!("Curves")).clicked() {
            acts.push(Act::Add("curves"));
            ui.close();
        }
        if ui.button(tl!("Colour Fade")).clicked() {
            acts.push(Act::Add("colourFade"));
            ui.close();
        }
        if ui.button(tl!("Tint")).clicked() {
            acts.push(Act::Add("tint"));
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
            let row_fx = RowEffect { keys, ..RowEffect::new(kind) };
            let mut params = row_fx.params_at(current_frame);

            let kind_label = match kind {
                FxKind::Blur => tl!("Blur"),
                FxKind::Glow => tl!("Glow"),
                FxKind::DropShadow => tl!("Drop Shadow"),
                FxKind::ColorAdjust => tl!("Color Adjust"),
                FxKind::Tint => tl!("Tint"),
                FxKind::Transparency => tl!("Transparency"),
                FxKind::Greyscale => tl!("Greyscale"),
                FxKind::Levels => tl!("Levels"),
                FxKind::Contrast => tl!("Contrast"),
                FxKind::Gamma => tl!("Gamma"),
                FxKind::HueSaturation => tl!("Hue / Saturation"),
                FxKind::ColourScale => tl!("Colour Scale"),
                FxKind::Curves => tl!("Curves"),
                FxKind::ColourFade => tl!("Colour Fade"),
                FxKind::BoxBlur => tl!("Box Blur"),
                FxKind::DirectionalBlur => tl!("Directional Blur"),
                FxKind::RadialBlur => tl!("Radial Blur"),
                FxKind::RadialZoomBlur => tl!("Radial Zoom Blur"),
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
                    FxKind::Tint => {
                        let mut amount = params.get("amount").and_then(Value::as_f64).unwrap_or(50.0);
                        let color_str = params.get("color").and_then(Value::as_str).unwrap_or("#ff8800").to_string();

                        ui.horizontal(|ui| {
                            ui.label(tl!("Amount:"));
                            if ui.add(DragValue::new(&mut amount).speed(1.0).range(0.0..=100.0)).changed() {
                                params["amount"] = json!(amount);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label(tl!("Color:"));
                            let mut rgb = parse_hex(&color_str).unwrap_or([255, 136, 0]);
                            if ui.color_edit_button_srgb(&mut rgb).changed() {
                                let new_hex = hex(rgb);
                                params["color"] = json!(new_hex);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }

                            let text_id = ui.id().with(("tint_hex", i));
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
                    FxKind::Transparency => {
                        let mut amount = params.get("amount").and_then(Value::as_f64).unwrap_or(100.0);
                        ui.horizontal(|ui| {
                            ui.label(tl!("Amount:"));
                            if ui.add(DragValue::new(&mut amount).speed(1.0).range(0.0..=100.0)).changed() {
                                params["amount"] = json!(amount);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });
                    }
                    FxKind::Greyscale => {
                        let mut amount = params.get("amount").and_then(Value::as_f64).unwrap_or(100.0);
                        ui.horizontal(|ui| {
                            ui.label(tl!("Amount:"));
                            if ui.add(DragValue::new(&mut amount).speed(1.0).range(0.0..=100.0)).changed() {
                                params["amount"] = json!(amount);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });
                    }
                    FxKind::Levels => {
                        let mut in_black = params.get("inBlack").and_then(Value::as_f64).unwrap_or(0.0);
                        let mut in_white = params.get("inWhite").and_then(Value::as_f64).unwrap_or(255.0);
                        let mut gamma = params.get("gamma").and_then(Value::as_f64).unwrap_or(1.0);
                        let mut out_black = params.get("outBlack").and_then(Value::as_f64).unwrap_or(0.0);
                        let mut out_white = params.get("outWhite").and_then(Value::as_f64).unwrap_or(255.0);

                        ui.horizontal(|ui| {
                            ui.label(tl!("In Black:"));
                            if ui.add(DragValue::new(&mut in_black).speed(1.0).range(0.0..=255.0)).changed() {
                                params["inBlack"] = json!(in_black);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                            ui.label(tl!("In White:"));
                            if ui.add(DragValue::new(&mut in_white).speed(1.0).range(0.0..=255.0)).changed() {
                                params["inWhite"] = json!(in_white);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(tl!("Gamma:"));
                            if ui.add(DragValue::new(&mut gamma).speed(0.05).range(0.1..=10.0)).changed() {
                                params["gamma"] = json!(gamma);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(tl!("Out Black:"));
                            if ui.add(DragValue::new(&mut out_black).speed(1.0).range(0.0..=255.0)).changed() {
                                params["outBlack"] = json!(out_black);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                            ui.label(tl!("Out White:"));
                            if ui.add(DragValue::new(&mut out_white).speed(1.0).range(0.0..=255.0)).changed() {
                                params["outWhite"] = json!(out_white);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });
                    }
                    FxKind::Contrast => {
                        let mut amount = params.get("amount").and_then(Value::as_f64).unwrap_or(0.0);
                        ui.horizontal(|ui| {
                            ui.label(tl!("Amount:"));
                            if ui.add(DragValue::new(&mut amount).speed(1.0).range(-100.0..=100.0)).changed() {
                                params["amount"] = json!(amount);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });
                    }
                    FxKind::Gamma => {
                        let mut gamma = params.get("gamma").and_then(Value::as_f64).unwrap_or(1.0);
                        ui.horizontal(|ui| {
                            ui.label(tl!("Gamma:"));
                            if ui.add(DragValue::new(&mut gamma).speed(0.05).range(0.1..=10.0)).changed() {
                                params["gamma"] = json!(gamma);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });
                    }
                    FxKind::HueSaturation => {
                        let mut hue = params.get("hue").and_then(Value::as_f64).unwrap_or(0.0);
                        let mut saturation = params.get("saturation").and_then(Value::as_f64).unwrap_or(0.0);
                        let mut lightness = params.get("lightness").and_then(Value::as_f64).unwrap_or(0.0);
                        ui.horizontal(|ui| {
                            ui.label(tl!("Hue:"));
                            if ui.add(DragValue::new(&mut hue).speed(1.0).range(-180.0..=180.0)).changed() {
                                params["hue"] = json!(hue);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(tl!("Saturation:"));
                            if ui.add(DragValue::new(&mut saturation).speed(1.0).range(-100.0..=100.0)).changed() {
                                params["saturation"] = json!(saturation);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(tl!("Lightness:"));
                            if ui.add(DragValue::new(&mut lightness).speed(1.0).range(-100.0..=100.0)).changed() {
                                params["lightness"] = json!(lightness);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });
                    }
                    FxKind::ColourScale => {
                        let mut r = params.get("r").and_then(Value::as_f64).unwrap_or(1.0);
                        let mut g = params.get("g").and_then(Value::as_f64).unwrap_or(1.0);
                        let mut b = params.get("b").and_then(Value::as_f64).unwrap_or(1.0);
                        let mut a = params.get("a").and_then(Value::as_f64).unwrap_or(1.0);
                        ui.horizontal(|ui| {
                            ui.label(tl!("R:"));
                            if ui.add(DragValue::new(&mut r).speed(0.05).range(0.0..=5.0)).changed() {
                                params["r"] = json!(r);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                            ui.label(tl!("G:"));
                            if ui.add(DragValue::new(&mut g).speed(0.05).range(0.0..=5.0)).changed() {
                                params["g"] = json!(g);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(tl!("B:"));
                            if ui.add(DragValue::new(&mut b).speed(0.05).range(0.0..=5.0)).changed() {
                                params["b"] = json!(b);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                            ui.label(tl!("A:"));
                            if ui.add(DragValue::new(&mut a).speed(0.05).range(0.0..=5.0)).changed() {
                                params["a"] = json!(a);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });
                    }
                    FxKind::Curves => {
                        let cur_points_str = params.get("points").map(|v| v.to_string()).unwrap_or_else(|| "[[0,0],[255,255]]".to_string());
                        let text_id = ui.id().with(("curves_pts", i));
                        let mut text_pts = ui.data_mut(|d| d.get_temp::<String>(text_id)).unwrap_or_else(|| cur_points_str.clone());
                        ui.horizontal(|ui| {
                            ui.label(tl!("Points:"));
                            if ui.add(TextEdit::singleline(&mut text_pts).desired_width(140.0)).changed() {
                                ui.data_mut(|d| d.insert_temp(text_id, text_pts.clone()));
                                if let Ok(parsed) = serde_json::from_str::<Value>(&text_pts)
                                    && parsed.is_array()
                                {
                                    params["points"] = parsed;
                                    acts.push(Act::Key { index: i, params: params.clone() });
                                }
                            }
                        });
                    }
                    FxKind::ColourFade => {
                        let mut amount = params.get("amount").and_then(Value::as_f64).unwrap_or(0.0);
                        let color_str = params.get("color").and_then(Value::as_str).unwrap_or("#000000").to_string();

                        ui.horizontal(|ui| {
                            ui.label(tl!("Amount:"));
                            if ui.add(DragValue::new(&mut amount).speed(1.0).range(0.0..=100.0)).changed() {
                                params["amount"] = json!(amount);
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

                            let text_id = ui.id().with(("fade_hex", i));
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
                    FxKind::BoxBlur => {
                        let mut radius = params.get("radius").and_then(Value::as_f64).unwrap_or(5.0);
                        ui.horizontal(|ui| {
                            ui.label(tl!("Radius:"));
                            if ui.add(DragValue::new(&mut radius).speed(0.1).range(0.0..=250.0)).changed() {
                                params["radius"] = json!(radius);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });
                    }
                    FxKind::DirectionalBlur => {
                        let mut radius = params.get("radius").and_then(Value::as_f64).unwrap_or(10.0);
                        let mut angle = params.get("angle").and_then(Value::as_f64).unwrap_or(0.0);
                        ui.horizontal(|ui| {
                            ui.label(tl!("Radius:"));
                            if ui.add(DragValue::new(&mut radius).speed(0.1).range(0.0..=250.0)).changed() {
                                params["radius"] = json!(radius);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(tl!("Angle:"));
                            if ui.add(DragValue::new(&mut angle).speed(1.0).range(0.0..=360.0)).changed() {
                                params["angle"] = json!(angle);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });
                    }
                    FxKind::RadialBlur => {
                        let mut radius = params.get("radius").and_then(Value::as_f64).unwrap_or(5.0);
                        ui.horizontal(|ui| {
                            ui.label(tl!("Radius:"));
                            if ui.add(DragValue::new(&mut radius).speed(0.1).range(0.0..=250.0)).changed() {
                                params["radius"] = json!(radius);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });
                    }
                    FxKind::RadialZoomBlur => {
                        let mut amount = params.get("amount").and_then(Value::as_f64).unwrap_or(10.0);
                        let mut cx = params.get("cx").and_then(Value::as_f64).unwrap_or(0.5);
                        let mut cy = params.get("cy").and_then(Value::as_f64).unwrap_or(0.5);
                        ui.horizontal(|ui| {
                            ui.label(tl!("Amount:"));
                            if ui.add(DragValue::new(&mut amount).speed(0.5).range(0.0..=100.0)).changed() {
                                params["amount"] = json!(amount);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(tl!("Center X:"));
                            if ui.add(DragValue::new(&mut cx).speed(0.01).range(0.0..=1.0)).changed() {
                                params["cx"] = json!(cx);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                            ui.label(tl!("Center Y:"));
                            if ui.add(DragValue::new(&mut cy).speed(0.01).range(0.0..=1.0)).changed() {
                                params["cy"] = json!(cy);
                                acts.push(Act::Key { index: i, params: params.clone() });
                            }
                        });
                    }
                });
            }
        }
    });

    // A drag previews each step and lands as one undo step on release (see `live_run`).
    let down = ui.input(|i| i.pointer.any_down());
    let mut keyed = false;
    for act in acts {
        match act {
            Act::Add(kind) => {
                run(app, "fx.add", json!({"kind": kind}));
            }
            Act::Key { index, params } => {
                keyed = true;
                let phase = if down { Live::Dragging } else { Live::Released };
                super::live_run(app, "Key Row Effect", "fx.key", json!({"index": index, "frame": current_frame, "params": params}), phase);
                super::set_pstate(ui.ctx(), LIVE, down);
            }
            Act::RemoveKey { index } => {
                run(app, "fx.removeKey", json!({"index": index, "frame": current_frame}));
            }
            Act::Remove { index } => {
                run(app, "fx.remove", json!({"index": index}));
            }
        }
    }
    // Released without a last change: close the drag's undo step.
    if !keyed && !down && super::pstate::<bool>(ui.ctx(), LIVE) {
        super::set_pstate(ui.ctx(), LIVE, false);
        app.session.commit_interaction().ok();
        app.sync_views();
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

    #[test]
    fn all_effect_kinds_show_in_ui_without_panic() {
        let all_kinds = [
            "blur",
            "glow",
            "dropShadow",
            "colorAdjust",
            "tint",
            "transparency",
            "greyscale",
            "levels",
            "contrast",
            "gamma",
            "hueSaturation",
            "colourScale",
            "curves",
            "colourFade",
            "box",
            "directional",
            "radial",
            "radialZoom",
        ];
        let mut app = anim_doc();
        for kind in all_kinds {
            app.run("fx.add", json!({"kind": kind})).unwrap();
        }
        let ctx = ctx();
        frame(&mut app, &ctx);
        let list = app.session.execute("fx.list", &json!({})).unwrap();
        assert_eq!(list["effects"].as_array().unwrap().len(), 18);
    }
}
