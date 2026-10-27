//! Node view panel: the scene's wiring (rows, pegs, cutters, row effects into the composite) as
//! a graph, read from `nodes.graph`; dragging a cable runs `nodes.connect` / `nodes.disconnect`.

use egui::Ui;

use crate::VectorcraftApp;

pub const ID: &str = "nodeView";

pub fn show(_app: &mut VectorcraftApp, _ui: &mut Ui) {}
