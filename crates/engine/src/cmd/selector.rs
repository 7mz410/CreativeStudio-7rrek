//! 7rrek (Creative Studio): drawing selectors, as in Harmony's Line Art and Colour Art nodes. A row
//! effect can take only part of each drawing: its line art (everything outside the drawing's
//! Colour Art sublayer) or its colour art (that sublayer). The effect then goes on those nodes
//! instead of on the row.

use serde::{Deserialize, Serialize};
use vectorcraft_doc::{Document, Node, NodeKind};

use super::NodeId;
use super::art_layers::COLOUR_ART;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Part {
    #[default]
    All,
    LineArt,
    ColourArt,
}

impl Part {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "all" => Some(Self::All),
            "lineArt" => Some(Self::LineArt),
            "colourArt" => Some(Self::ColourArt),
            _ => None,
        }
    }
}

fn is_colour_art(n: &Node) -> bool {
    matches!(n.kind, NodeKind::Layer { .. }) && n.name.as_deref() == Some(COLOUR_ART)
}

/// The nodes an effect on `row` for `part` goes on: the row itself for all of it. Otherwise every
/// Colour Art sublayer under the row, or every node beside one (a drawing without Colour Art is all
/// lines, so its children count as line art).
pub fn targets(d: &Document, row: NodeId, part: Part) -> Vec<NodeId> {
    if part == Part::All {
        return vec![row];
    }
    let mut out = Vec::new();
    let Some(n) = d.node(row) else { return out };
    walk(n, part, &mut out);
    out
}

fn walk(n: &Node, part: Part, out: &mut Vec<NodeId>) {
    let Some(kids) = n.children() else { return };
    let has_colour = kids.iter().any(|c| is_colour_art(c));
    for c in kids {
        if is_colour_art(c) {
            if part == Part::ColourArt {
                out.push(c.id);
            }
        } else if matches!(c.kind, NodeKind::Layer { .. }) && !has_colour {
            // A sublayer (a drawing) without Colour Art of its own here: look inside it.
            walk(c, part, out);
        } else if part == Part::LineArt {
            out.push(c.id);
        }
    }
}
