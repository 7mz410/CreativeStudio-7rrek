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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::Session;

    /// A drawing on frame 1: a black 10 pt square outline, and a red fill of it on Colour Art.
    /// Returns (session, row, line id, colour art id).
    fn drawing() -> (Session, u64, u64, u64) {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        let row = s.doc().unwrap().doc.layers[0].id.0;
        let line = s.execute("shape.rectangle", &json!({"x": 20, "y": 20, "width": 60, "height": 60})).unwrap()["id"].as_u64().unwrap();
        s.execute("paint.setFill", &json!({"none": true})).unwrap();
        s.execute("stroke.set", &json!({"weight": 10})).unwrap();
        s.execute("art.fill", &json!({"ids": [line], "color": "#ff0000"})).unwrap();
        let d = &s.doc().unwrap().doc;
        let drawing = d.layers[0].children().unwrap()[0].id;
        let ca = d.node(drawing).unwrap().children().unwrap().iter().find(|c| c.name.as_deref() == Some(COLOUR_ART)).unwrap().id.0;
        (s, row, line, ca)
    }

    /// How many effects the node carries on the frame shown.
    fn effects(s: &Session, id: u64) -> usize {
        s.doc().unwrap().doc.node(NodeId(id)).unwrap().appearance.effects.len()
    }

    #[test]
    fn line_art_takes_the_effect_and_colour_art_does_not() {
        let (mut s, row, line, ca) = drawing();
        s.execute("fx.add", &json!({"row": row, "kind": "glow", "part": "lineArt"})).unwrap();
        s.execute("anim.goto", &json!({"frame": 1})).unwrap();
        assert_eq!(effects(&s, ca), 0, "colour art has no effect");
        assert_eq!(effects(&s, line), 1, "the line has the glow");
    }

    #[test]
    fn colour_art_takes_the_effect_and_line_art_does_not() {
        let (mut s, row, line, ca) = drawing();
        s.execute("fx.add", &json!({"row": row, "kind": "glow", "part": "colourArt"})).unwrap();
        s.execute("anim.goto", &json!({"frame": 1})).unwrap();
        assert_eq!(effects(&s, line), 0, "the line has no effect");
        assert_eq!(effects(&s, ca), 1, "colour art has the glow");
    }

    #[test]
    fn showing_frames_again_never_stacks_a_selector_effect() {
        let (mut s, row, line, ca) = drawing();
        s.execute("fx.add", &json!({"row": row, "kind": "glow", "part": "lineArt"})).unwrap();
        for f in [2, 1, 1, 2, 1] {
            s.execute("anim.goto", &json!({"frame": f})).unwrap();
            assert_eq!(effects(&s, line), 1, "after showing frame {f}");
            assert_eq!(effects(&s, ca), 0, "colour art after showing frame {f}");
        }
    }
}
