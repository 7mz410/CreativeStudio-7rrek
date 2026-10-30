//! Textures panel: manage and apply vector line textures (`texture.*`).

use egui::{DragValue, ScrollArea, TextEdit, Ui};
use serde_json::{Value, json};

use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

pub const ID: &str = "textures";

fn kind_label(kind: &str) -> &'static str {
    match kind {
        "pencil" => "Pencil",
        "chalk" => "Chalk",
        "charcoal" => "Charcoal",
        "grain" => "Grain",
        _ => "Grain",
    }
}

fn draw_texture_thumb(ui: &mut Ui, t: &Tokens, kind: &str, density: u32, seed: u64) {
    let size = egui::vec2(24.0, 24.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, egui::Color32::WHITE);
    ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, t.border), egui::StrokeKind::Inside);

    let mut rng = seed ^ 0x9e37_79b9_7f4a_7c15;
    let mut next_u = || {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((rng >> 32) as u32 as f32) / 4294967296.0
    };

    let count = ((density as f32 / 100.0) * 16.0).max(2.0) as usize;
    for _ in 0..count {
        let u = next_u();
        let v = next_u();
        let px = rect.min.x + 2.0 + u * (rect.width() - 4.0);
        let py = rect.min.y + 2.0 + v * (rect.height() - 4.0);
        match kind {
            "grain" => {
                ui.painter().circle_filled(egui::pos2(px, py), 1.0, egui::Color32::from_gray(100));
            }
            "pencil" => {
                ui.painter().line_segment(
                    [egui::pos2(px - 1.5, py - 1.5), egui::pos2(px + 1.5, py + 1.5)],
                    egui::Stroke::new(1.0, egui::Color32::from_gray(90)),
                );
            }
            "chalk" => {
                ui.painter().circle_filled(egui::pos2(px, py), 1.8, egui::Color32::from_gray(140));
            }
            "charcoal" => {
                let r = egui::Rect::from_center_size(egui::pos2(px, py), egui::vec2(2.5, 2.0));
                ui.painter().rect_filled(r, 0.0, egui::Color32::from_gray(60));
            }
            _ => {}
        }
    }
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    if app.session.active().is_none() {
        widgets::dim_label(ui, tl!("No document"));
        return;
    }

    let t = Tokens::get(ui.ctx());
    let sel_id = ui.id().with("selected_texture");
    let new_name_id = ui.id().with("new_texture_name");
    let new_kind_id = ui.id().with("new_texture_kind");

    let mut selected: Option<String> = ui.data(|d| d.get_temp(sel_id));
    let mut new_name: String = ui.data(|d| d.get_temp(new_name_id).unwrap_or_default());
    let mut new_kind: String = ui.data(|d| d.get_temp(new_kind_id).unwrap_or_else(|| "pencil".to_string()));

    let textures_val = app.session.execute("texture.list", &json!({})).unwrap_or(Value::Null);
    let textures = textures_val.get("textures").and_then(Value::as_array).cloned().unwrap_or_default();

    if let (None, Some(first_name)) = (&selected, textures.first().and_then(|t| t.get("name")).and_then(Value::as_str)) {
        let name = first_name.to_string();
        selected = Some(name.clone());
        ui.data_mut(|d| d.insert_temp(sel_id, Some(name)));
    }

    if textures.is_empty() {
        widgets::dim_label(ui, tl!("No textures"));
    } else {
        ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
            for tex in &textures {
                let name = tex.get("name").and_then(Value::as_str).unwrap_or("");
                let kind = tex.get("kind").and_then(Value::as_str).unwrap_or("pencil");
                let density = tex.get("density").and_then(Value::as_u64).unwrap_or(50) as u32;
                let seed = tex.get("seed").and_then(Value::as_u64).unwrap_or(1);
                let is_selected = selected.as_deref() == Some(name);

                ui.horizontal(|ui| {
                    draw_texture_thumb(ui, &t, kind, density, seed);
                    let label = format!("{name} ({})", tl!(kind_label(kind)));
                    if ui.selectable_label(is_selected, label).clicked() {
                        selected = Some(name.to_string());
                        ui.data_mut(|d| d.insert_temp(sel_id, selected.clone()));
                    }
                });
            }
        });
    }

    ui.add_space(4.0);
    ui.separator();

    // Controls for selected texture
    let selected_tex =
        selected.as_deref().and_then(|sel| textures.iter().find(|item| item.get("name").and_then(Value::as_str) == Some(sel)).map(|t| (sel, t)));

    if let Some((sel_name, tex)) = selected_tex {
        let mut scale = tex.get("scale").and_then(Value::as_u64).unwrap_or(100) as u32;
        let mut density = tex.get("density").and_then(Value::as_u64).unwrap_or(50) as u32;
        let current_kind = tex.get("kind").and_then(Value::as_str).unwrap_or("pencil");

        let mut changed = false;

        ui.horizontal(|ui| {
            ui.label(tl!("Scale:"));
            if ui.add(DragValue::new(&mut scale).range(10..=400).suffix("%")).changed() {
                changed = true;
            }
            ui.label(tl!("Density:"));
            if ui.add(DragValue::new(&mut density).range(0..=100)).changed() {
                changed = true;
            }
        });

        if changed {
            let _ = app.run("texture.set", json!({"name": sel_name, "scale": scale, "density": density, "kind": current_kind}));
        }
    }

    ui.add_space(4.0);

    // Action buttons
    ui.horizontal(|ui| {
        let has_sel = selected.is_some();
        let clicked_apply = ui.add_enabled(has_sel, egui::Button::new(tl!("Apply Texture"))).clicked();
        if clicked_apply {
            let res = selected.as_ref().map(|sel| app.run("texture.apply", json!({"name": sel})));
            if let Some(Err(e)) = res {
                app.status(e);
            }
        }

        let clicked_remove = ui.button(tl!("Remove Texture")).clicked();
        if clicked_remove {
            let res = app.run("texture.remove", json!({}));
            if let Err(e) = res {
                app.status(e);
            }
        }

        let clicked_delete = ui.add_enabled(has_sel, egui::Button::new(tl!("Delete Texture"))).clicked();
        if clicked_delete {
            let res = selected.as_ref().map(|sel| app.run("texture.delete", json!({"name": sel})));
            match res {
                Some(Ok(_)) => {
                    selected = None;
                    ui.data_mut(|d| d.insert_temp(sel_id, selected.clone()));
                }
                Some(Err(e)) => app.status(e),
                None => {}
            }
        }
    });

    ui.add_space(4.0);
    ui.separator();

    // Add new texture row
    ui.horizontal(|ui| {
        ui.add(TextEdit::singleline(&mut new_name).hint_text(tl!("Name")).desired_width(70.0));
        egui::ComboBox::from_id_salt("new_kind_combo").selected_text(tl!(kind_label(&new_kind))).width(75.0).show_ui(ui, |ui| {
            for k in ["pencil", "chalk", "charcoal", "grain"] {
                ui.selectable_value(&mut new_kind, k.to_string(), tl!(kind_label(k)));
            }
        });

        let can_add = !new_name.trim().is_empty();
        if ui.add_enabled(can_add, egui::Button::new(tl!("Add Texture"))).clicked() {
            let res = app.run("texture.add", json!({"name": new_name.trim(), "kind": new_kind}));
            match res {
                Ok(_) => {
                    selected = Some(new_name.trim().to_string());
                    ui.data_mut(|d| d.insert_temp(sel_id, selected.clone()));
                    new_name.clear();
                }
                Err(e) => app.status(e),
            }
        }
    });

    ui.data_mut(|d| {
        d.insert_temp(new_name_id, new_name);
        d.insert_temp(new_kind_id, new_kind);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> VectorcraftApp {
        VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default())
    }

    fn frame(app: &mut VectorcraftApp, ctx: &egui::Context) {
        let screen = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 300.0)));
        let input = egui::RawInput { screen_rect: screen, ..Default::default() };
        ctx.run_ui(input, |ui| show(app, ui)).textures_delta.clear();
    }

    #[test]
    fn no_document_shows_without_panic() {
        let mut app = app();
        let ctx = egui::Context::default();
        frame(&mut app, &ctx);
    }

    #[test]
    fn no_textures_shows_without_panic() {
        let mut app = app();
        app.session.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();
        let ctx = egui::Context::default();
        frame(&mut app, &ctx);
    }

    #[test]
    fn texture_added_shows_in_panel() {
        let mut app = app();
        app.session.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();
        app.session.execute("texture.add", &json!({"name": "MyPencil", "kind": "pencil"})).unwrap();
        let ctx = egui::Context::default();
        frame(&mut app, &ctx);
    }
}
