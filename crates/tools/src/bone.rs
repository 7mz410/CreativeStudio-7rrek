//! The Bone tool: click to add a joint, drag a joint to pose the skeleton at the current frame.
use crate::{Action, PointerEvent, Tool, ToolContext};

#[derive(Default)]
pub struct BoneTool {}

impl Tool for BoneTool {
    fn id(&self) -> &'static str {
        "bone"
    }
    fn pointer(&mut self, _cx: &ToolContext, _ev: &PointerEvent) -> Vec<Action> {
        vec![]
    }
}
