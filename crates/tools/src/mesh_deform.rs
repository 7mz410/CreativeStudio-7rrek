//! The Mesh Deform tool (stub, filled in by its task).

use vectorcraft_geom::Point;

use crate::{Action, Cursor, Mods, PointerEvent, Tool, ToolContext};

#[derive(Default)]
pub struct MeshDeformTool;

impl Tool for MeshDeformTool {
    fn id(&self) -> &'static str {
        "meshDeform"
    }

    fn pointer(&mut self, _cx: &ToolContext, _ev: &PointerEvent) -> Vec<Action> {
        vec![]
    }

    fn cursor(&self, _cx: &ToolContext, _p: Point, _m: Mods) -> Cursor {
        Cursor::Crosshair
    }
}
