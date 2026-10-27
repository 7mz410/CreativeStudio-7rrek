//! Action Templates panel: save a span of the current row's motion and paste it onto another row
//! (`action.*`).

use egui::{DragValue, ScrollArea, TextEdit, Ui};
use serde_json::{Value, json};

use crate::VectorcraftApp;
use crate::widgets;

pub const ID: &str = "actionTemplates";

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    if app.session.active().is_none() {
        widgets::dim_label(ui, tl!("No document"));
        return;
    }

    let sel_id = ui.id().with("selected_action");
    let name_id = ui.id().with("save_name");
    let from_id = ui.id().with("save_from");
    let to_id = ui.id().with("save_to");

    let mut selected: Option<String> = ui.data(|d| d.get_temp(sel_id));
    let mut save_name: String = ui.data(|d| d.get_temp(name_id).unwrap_or_default());
    let mut save_from: u32 = ui.data(|d| d.get_temp(from_id).unwrap_or(1));
    let mut save_to: u32 = ui.data(|d| d.get_temp(to_id).unwrap_or(24));

    let actions_val = app.session.execute("action.list", &json!({})).unwrap_or(Value::Null);
    let actions = actions_val.get("actions").and_then(Value::as_array);

    if actions.as_ref().is_none_or(|a| a.is_empty()) {
        widgets::dim_label(ui, tl!("No action templates"));
    } else {
        ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
            for a in actions.into_iter().flatten() {
                let name = a.get("name").and_then(Value::as_str).unwrap_or("");
                let len = a.get("length").and_then(Value::as_u64).unwrap_or(0);
                let label = format!("{name} ({len}f)");
                let is_selected = selected.as_deref() == Some(name);
                if ui.selectable_label(is_selected, label).clicked() {
                    selected = Some(name.to_string());
                    ui.data_mut(|d| d.insert_temp(sel_id, selected.clone()));
                }
            }
        });
    }

    ui.add_space(4.0);

    ui.horizontal(|ui| {
        let has_sel = selected.is_some();
        let clicked_paste = ui.add_enabled(has_sel, egui::Button::new(tl!("Paste"))).clicked();
        if clicked_paste {
            let res = selected.as_ref().map(|sel| app.run("action.paste", json!({"name": sel})));
            if let Some(Err(e)) = res {
                app.status(e);
            }
        }
        let clicked_remove = ui.add_enabled(has_sel, egui::Button::new(tl!("Remove"))).clicked();
        if clicked_remove {
            let res = selected.as_ref().map(|sel| app.run("action.remove", json!({"name": sel})));
            match res {
                Some(Ok(_)) => {
                    selected = None;
                    ui.data_mut(|d| d.remove_temp::<String>(sel_id));
                }
                Some(Err(e)) => app.status(e),
                None => {}
            }
        }
        let to_export = if ui.add_enabled(has_sel, egui::Button::new(tl!("Export…"))).clicked() { selected.as_deref() } else { None };
        if let Some(sel) = to_export {
            match crate::io::target_path(app, None, vectorcraft_engine::cmd::action_tpl::EXTENSION) {
                Ok(path) => {
                    let res = app.run("action.export", json!({"name": sel, "path": path}));
                    if let Err(e) = res {
                        app.status(e);
                    }
                }
                Err(e) if e != "cancelled" => app.status(e),
                _ => {}
            }
        }
        if ui.button(tl!("Import…")).clicked() {
            let pick = crate::FilePick {
                filters: vec![crate::FilePick::filter("Action Templates", &[vectorcraft_engine::cmd::action_tpl::EXTENSION])],
                ..Default::default()
            };
            let path = crate::picks::open(app, &pick);
            if let Some(path) = path {
                let res = app.run("action.import", json!({"path": path}));
                if let Err(e) = res {
                    app.status(e);
                }
            }
        }
    });

    ui.separator();
    ui.label(tl!("Save Action"));

    ui.horizontal(|ui| {
        ui.label(tl!("Name:"));
        if ui.add(TextEdit::singleline(&mut save_name).desired_width(120.0)).changed() {
            ui.data_mut(|d| d.insert_temp(name_id, save_name.clone()));
        }
    });

    ui.horizontal(|ui| {
        ui.label(tl!("From:"));
        if ui.add(DragValue::new(&mut save_from).range(1..=10000)).changed() {
            ui.data_mut(|d| d.insert_temp(from_id, save_from));
        }
        ui.label(tl!("To:"));
        let to_range = save_from..=10000;
        if ui.add(DragValue::new(&mut save_to).range(to_range)).changed() {
            ui.data_mut(|d| d.insert_temp(to_id, save_to));
        }
    });

    let can_save = !save_name.trim().is_empty() && save_to >= save_from;
    if ui.add_enabled(can_save, egui::Button::new(tl!("Save Action"))).clicked() {
        match app.run("action.save", json!({"name": save_name.trim(), "from": save_from, "to": save_to})) {
            Ok(_) => {
                save_name.clear();
                ui.data_mut(|d| d.remove_temp::<String>(name_id));
            }
            Err(e) => app.status(e),
        }
    }
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
    fn headless_renders_document_with_action_without_panic() {
        let mut app = app();
        app.session.execute("file.new", &json!({})).unwrap();
        app.session.execute("action.save", &json!({"name": "blink", "from": 1, "to": 12})).unwrap();
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx, Default::default());
        frame(&mut app, &ctx);
    }
}
