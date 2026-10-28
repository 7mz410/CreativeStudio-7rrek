//! Node view panel: the scene's wiring (rows, pegs, cutters, row effects into the composite) as
//! a graph, read from `nodes.graph`; dragging a cable runs `nodes.connect` / `nodes.disconnect`.
//! Groups are simple visual groups: a collapsed group renders as one node with all incoming/outgoing
//! cables attached to it; double-clicking toggles collapse. Multi-selection right-click offers Group,
//! and right-clicking a group offers Ungroup and Rename.

use std::collections::{BTreeSet, HashMap};

use egui::{Align2, Color32, CornerRadius, FontId, Key, PointerButton, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::{Value, json};

use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

pub const ID: &str = "nodeView";

const NODE_W: f32 = 124.0;
const NODE_H: f32 = 36.0;
const GAP_X: f32 = 28.0;
const GAP_Y: f32 = 48.0;
const MARGIN_X: f32 = 32.0;
const MARGIN_Y: f32 = 32.0;
const PORT_R: f32 = 4.0;
/// Egui memory key (per panel) for the Fit toggle.
const FIT_KEY: &str = "node_view_fit";
/// Egui temp key (per panel) for this frame's layout: diagram size, Write rect, scale.
const LAYOUT_KEY: &str = "node_view_layout";

/// Scale that fits a graph `diagram_w` wide into `avail_w`, kept within 0.5..=1.0.
fn fit_scale(avail_w: f32, diagram_w: f32) -> f32 {
    if !(avail_w.is_finite() && diagram_w.is_finite() && diagram_w > 0.0) {
        return 1.0;
    }
    (avail_w / diagram_w).clamp(0.5, 1.0)
}

/// Parsed node representation from `nodes.graph`.
#[derive(Clone, Debug)]
struct GraphNode {
    id: String,
    kind: String,
    name: String,
    inverted: bool,
    fx: String,
    ports_in: Vec<String>,
    ports_out: Vec<String>,
}

impl GraphNode {
    fn title(&self) -> String {
        match self.kind.as_str() {
            "peg" => {
                if !self.name.is_empty() {
                    self.name.clone()
                } else {
                    self.id.clone()
                }
            }
            "row" => {
                if !self.name.is_empty() {
                    self.name.clone()
                } else {
                    self.id.clone()
                }
            }
            "cutter" => {
                if self.inverted {
                    tl!("Cutter (Inv)").to_string()
                } else {
                    tl!("Cutter").to_string()
                }
            }
            "effect" => match self.fx.as_str() {
                "blur" => tl!("Blur").to_string(),
                "glow" => tl!("Glow").to_string(),
                "dropShadow" => tl!("Drop Shadow").to_string(),
                "colorAdjust" => tl!("Color Adjust").to_string(),
                _ => self.fx.clone(),
            },
            "composite" => tl!("Composite").to_string(),
            "display" => tl!("Display").to_string(),
            "write" => tl!("Write").to_string(),
            "group" => {
                if !self.name.is_empty() {
                    self.name.clone()
                } else {
                    tl!("Group").to_string()
                }
            }
            _ => self.id.clone(),
        }
    }

    fn kind_badge(&self) -> &'static str {
        match self.kind.as_str() {
            "peg" => "PEG",
            "row" => "ROW",
            "cutter" => "CUT",
            "effect" => "FX",
            "composite" => "COMP",
            "group" => "GRP",
            _ => "",
        }
    }
}

/// Parsed group representation from `nodes.graph`.
#[derive(Clone, Debug)]
struct GraphGroup {
    id: u64,
    name: String,
    members: Vec<String>,
    collapsed: bool,
}

/// Parsed edge from `nodes.graph`.
#[derive(Clone, Debug)]
struct GraphEdge {
    from: String,
    to: String,
    port: String,
}

enum Act {
    Connect {
        from: String,
        to: String,
        port: String,
    },
    Disconnect {
        from: String,
        to: String,
        port: String,
    },
    /// Moves row `from` above or below row `target` in the composite (`place` is "above" or "below").
    Reorder {
        from: String,
        target: String,
        place: &'static str,
    },
    Group {
        members: Vec<String>,
    },
    /// Toggles the cache lock of the node's row.
    Lock {
        node: String,
    },
    Ungroup {
        id: u64,
    },
    Collapse {
        id: u64,
        collapsed: bool,
    },
    RenameGroup {
        id: u64,
        name: String,
    },
    /// Sets the Display or Write node (`cmd`) to `node`; None clears it (Reset).
    Branch {
        cmd: &'static str,
        node: Option<String>,
    },
}

/// Where a row dropped on `target` lands: above it in the upper half, below it otherwise.
fn place_for_drop(pointer_y: f32, target: Rect) -> &'static str {
    if pointer_y < target.center().y { "above" } else { "below" }
}

/// The command that sets a fixed node (Display or Write), None for any other node.
fn branch_cmd(id: &str) -> Option<&'static str> {
    match id {
        "display" => Some("nodes.display"),
        "write" => Some("nodes.write"),
        _ => None,
    }
}

/// A fixed node (Display or Write): one input, no output, placed beside the composite.
fn fixed_node(id: &str) -> GraphNode {
    GraphNode {
        id: id.into(),
        kind: id.into(),
        name: String::new(),
        inverted: false,
        fx: String::new(),
        ports_in: vec!["image".into()],
        ports_out: Vec::new(),
    }
}

fn parse_graph(val: &Value) -> (Vec<GraphNode>, Vec<GraphEdge>, Vec<GraphGroup>) {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut groups = Vec::new();

    if let Some(nodes_arr) = val.get("nodes").and_then(Value::as_array) {
        for n in nodes_arr {
            let id = n.get("id").and_then(Value::as_str).unwrap_or("").to_string();
            if id.is_empty() {
                continue;
            }
            let kind = n.get("kind").and_then(Value::as_str).unwrap_or("").to_string();
            let name = n.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            let inverted = n.get("inverted").and_then(Value::as_bool).unwrap_or(false);
            let fx = n.get("fx").and_then(Value::as_str).unwrap_or("").to_string();

            let (ports_in, ports_out) = match n.get("ports") {
                Some(p) => {
                    let ins = p
                        .get("in")
                        .and_then(Value::as_array)
                        .map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect())
                        .unwrap_or_default();
                    let outs = p
                        .get("out")
                        .and_then(Value::as_array)
                        .map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect())
                        .unwrap_or_default();
                    (ins, outs)
                }
                None => match kind.as_str() {
                    "row" => (vec!["transform".into(), "matte".into()], vec!["image".into()]),
                    "peg" => (vec!["transform".into()], vec!["transform".into()]),
                    "cutter" => (vec!["image".into(), "matte".into()], vec!["image".into()]),
                    "effect" => (vec!["image".into()], vec!["image".into()]),
                    "composite" => (vec!["image".into()], vec!["image".into()]),
                    _ => (vec!["image".into()], vec!["image".into()]),
                },
            };

            nodes.push(GraphNode { id, kind, name, inverted, fx, ports_in, ports_out });
        }
    }

    if let Some(edges_arr) = val.get("edges").and_then(Value::as_array) {
        for e in edges_arr {
            let from = e.get("from").and_then(Value::as_str).unwrap_or("").to_string();
            let to = e.get("to").and_then(Value::as_str).unwrap_or("").to_string();
            let port = e.get("port").and_then(Value::as_str).unwrap_or("").to_string();
            if !from.is_empty() && !to.is_empty() {
                edges.push(GraphEdge { from, to, port });
            }
        }
    }

    if let Some(groups_arr) = val.get("groups").and_then(Value::as_array) {
        for g in groups_arr {
            let id = g.get("id").and_then(Value::as_u64).unwrap_or(0);
            let name = g.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            let collapsed = g.get("collapsed").and_then(Value::as_bool).unwrap_or(false);
            let members = g
                .get("members")
                .and_then(Value::as_array)
                .map(|arr| arr.iter().filter_map(Value::as_str).map(|s| s.to_string()).collect())
                .unwrap_or_default();
            groups.push(GraphGroup { id, name, members, collapsed });
        }
    }

    (nodes, edges, groups)
}

fn bezier_points(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2) -> Vec<Pos2> {
    const STEPS: usize = 24;
    (0..=STEPS)
        .map(|i| {
            let t = i as f32 / STEPS as f32;
            let u = 1.0 - t;
            let w0 = u * u * u;
            let w1 = 3.0 * u * u * t;
            let w2 = 3.0 * u * t * t;
            let w3 = t * t * t;
            Pos2::new(w0 * p0.x + w1 * p1.x + w2 * p2.x + w3 * p3.x, w0 * p0.y + w1 * p1.y + w2 * p2.y + w3 * p3.y)
        })
        .collect()
}

fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_sq();
    if len_sq < 1e-6 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

fn dist_to_curve(p: Pos2, pts: &[Pos2]) -> f32 {
    let mut min_d = f32::INFINITY;
    for w in pts.windows(2) {
        let d = dist_to_segment(p, w[0], w[1]);
        if d < min_d {
            min_d = d;
        }
    }
    min_d
}

fn draw_dashed_polyline(painter: &egui::Painter, pts: &[Pos2], color: Color32, width: f32, dash_len: f32, gap_len: f32) {
    let mut draw = true;
    let mut remain = dash_len;
    for w in pts.windows(2) {
        let mut a = w[0];
        let b = w[1];
        let mut dist = a.distance(b);
        if dist < 1e-4 {
            continue;
        }
        let dir = (b - a) / dist;
        while dist > 0.0 {
            if dist <= remain {
                if draw {
                    painter.line_segment([a, b], Stroke::new(width, color));
                }
                remain -= dist;
                dist = 0.0;
            } else {
                let mid = a + dir * remain;
                if draw {
                    painter.line_segment([a, mid], Stroke::new(width, color));
                }
                a = mid;
                dist -= remain;
                draw = !draw;
                remain = if draw { dash_len } else { gap_len };
            }
        }
    }
}

#[derive(Clone, Debug)]
struct InputPort {
    node_id: String,
    port: String,
    pos: Pos2,
}

fn port_color(port: &str, t: &Tokens) -> Color32 {
    match port {
        "transform" => t.axis_y,
        "matte" => t.warning,
        _ => t.selection,
    }
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    if app.session.active().is_none() {
        widgets::dim_label(ui, tl!("No document"));
        return;
    }

    let graph_val = match app.session.execute("nodes.graph", &json!({})) {
        Ok(v) => v,
        Err(e) => {
            app.status(e.to_string());
            return;
        }
    };

    let (nodes, edges, groups) = parse_graph(&graph_val);
    let has_rows = nodes.iter().any(|n| n.kind == "row");
    if !has_rows {
        widgets::dim_label(ui, tl!("No rows in document"));
        return;
    }

    let t = Tokens::get(ui.ctx());
    let mut acts: Vec<Act> = Vec::new();

    let row_nodes: Vec<&GraphNode> = nodes.iter().filter(|n| n.kind == "row").collect();
    let peg_nodes: Vec<&GraphNode> = nodes.iter().filter(|n| n.kind == "peg").collect();

    let mut row_col: HashMap<String, usize> = HashMap::new();
    for (i, r) in row_nodes.iter().enumerate() {
        row_col.insert(r.id.clone(), i);
    }

    let col_x = |col: usize| MARGIN_X + col as f32 * (NODE_W + GAP_X);

    let has_pegs = !peg_nodes.is_empty();
    let row_y = if has_pegs { MARGIN_Y + NODE_H + GAP_Y } else { MARGIN_Y };

    let mut rects: HashMap<String, Rect> = HashMap::new();

    for (i, p) in peg_nodes.iter().enumerate() {
        let x = col_x(i);
        rects.insert(p.id.clone(), Rect::from_min_size(pos2(x, MARGIN_Y), vec2(NODE_W, NODE_H)));
    }

    for (i, r) in row_nodes.iter().enumerate() {
        let x = col_x(i);
        rects.insert(r.id.clone(), Rect::from_min_size(pos2(x, row_y), vec2(NODE_W, NODE_H)));
    }

    let mut cutter_row: HashMap<String, String> = HashMap::new();
    for n in nodes.iter().filter(|n| n.kind == "cutter") {
        if let Some(r_str) = n.id.strip_prefix("cut:") {
            cutter_row.insert(n.id.clone(), format!("row:{r_str}"));
        }
    }

    let mut next_col = row_nodes.len();
    for n in nodes.iter().filter(|n| n.kind == "cutter") {
        let col = cutter_row.get(&n.id).and_then(|r_id| row_col.get(r_id)).copied().unwrap_or_else(|| {
            let c = next_col;
            next_col += 1;
            c
        });
        let x = col_x(col);
        let y = row_y + NODE_H + GAP_Y;
        rects.insert(n.id.clone(), Rect::from_min_size(pos2(x, y), vec2(NODE_W, NODE_H)));
    }

    for n in nodes.iter().filter(|n| n.kind == "effect") {
        let parts: Vec<&str> = n.id.split(':').collect();
        let (r_id, idx) = if parts.len() >= 3 {
            let r_str = parts.get(1).copied().unwrap_or("");
            let i = parts.get(2).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
            (format!("row:{r_str}"), i)
        } else {
            (String::new(), 0)
        };

        let col = row_col.get(&r_id).copied().unwrap_or_else(|| {
            let c = next_col;
            next_col += 1;
            c
        });

        let has_cutter = nodes.iter().any(|c| c.kind == "cutter" && cutter_row.get(&c.id).map(|s| s.as_str()) == Some(&r_id));
        let step = if has_cutter { 2 + idx } else { 1 + idx };
        let x = col_x(col);
        let y = row_y + step as f32 * (NODE_H + GAP_Y);
        rects.insert(n.id.clone(), Rect::from_min_size(pos2(x, y), vec2(NODE_W, NODE_H)));
    }

    // Mapping of node id -> collapsed group id
    let mut member_to_collapsed: HashMap<String, u64> = HashMap::new();
    let mut collapsed_groups: HashMap<u64, &GraphGroup> = HashMap::new();
    for g in &groups {
        if g.collapsed {
            collapsed_groups.insert(g.id, g);
            for m in &g.members {
                member_to_collapsed.insert(m.clone(), g.id);
            }
        }
    }

    // Replace collapsed members with one group node
    let mut group_rects: HashMap<String, Rect> = HashMap::new();
    for (&gid, g) in &collapsed_groups {
        let member_positions: Vec<Rect> = g.members.iter().filter_map(|m| rects.get(m).copied()).collect();
        let group_rect = if let Some(first) = member_positions.first() {
            let min_x = member_positions.iter().map(|r| r.min.x).fold(first.min.x, f32::min);
            let min_y = member_positions.iter().map(|r| r.min.y).fold(first.min.y, f32::min);
            Rect::from_min_size(pos2(min_x, min_y), vec2(NODE_W, NODE_H))
        } else {
            let x = col_x(next_col);
            next_col += 1;
            Rect::from_min_size(pos2(x, row_y), vec2(NODE_W, NODE_H))
        };
        group_rects.insert(format!("group:{gid}"), group_rect);
    }

    for m in member_to_collapsed.keys() {
        rects.remove(m);
    }
    for (gid_str, r) in group_rects {
        rects.insert(gid_str, r);
    }

    let mut display_nodes: Vec<GraphNode> = nodes.iter().filter(|n| !member_to_collapsed.contains_key(&n.id)).cloned().collect();
    for (&gid, g) in &collapsed_groups {
        display_nodes.push(GraphNode {
            id: format!("group:{gid}"),
            kind: "group".into(),
            name: g.name.clone(),
            inverted: false,
            fx: String::new(),
            ports_in: vec!["image".into()],
            ports_out: vec!["image".into()],
        });
    }
    display_nodes.push(fixed_node("display"));
    display_nodes.push(fixed_node("write"));

    let max_y = rects.values().map(|r| r.bottom()).fold(row_y + NODE_H, f32::max);
    let comp_y = max_y + GAP_Y;
    let actual_total_cols = row_nodes.len().max(peg_nodes.len()).max(next_col).max(1);
    let total_width = col_x(actual_total_cols.saturating_sub(1)) + NODE_W - MARGIN_X;
    let comp_x = MARGIN_X + (total_width - NODE_W).max(0.0) * 0.5;
    rects.insert("comp".to_string(), Rect::from_min_size(pos2(comp_x, comp_y), vec2(NODE_W, NODE_H)));

    // Display and Write sit beside the composite, one after the other.
    let display_x = comp_x + NODE_W + GAP_X;
    let write_x = display_x + NODE_W + GAP_X;
    rects.insert("display".to_string(), Rect::from_min_size(pos2(display_x, comp_y), vec2(NODE_W, NODE_H)));
    rects.insert("write".to_string(), Rect::from_min_size(pos2(write_x, comp_y), vec2(NODE_W, NODE_H)));
    let fixed_right = write_x + NODE_W + MARGIN_X;

    let mut input_ports: Vec<InputPort> = Vec::new();
    let mut output_ports: HashMap<String, Pos2> = HashMap::new();

    for n in &display_nodes {
        let Some(&rect) = rects.get(&n.id) else { continue };

        if !n.ports_out.is_empty() && n.kind != "composite" {
            output_ports.insert(n.id.clone(), pos2(rect.center().x, rect.bottom()));
        }

        let n_in = n.ports_in.len();
        if n_in == 1 {
            if let Some(p_name) = n.ports_in.first() {
                input_ports.push(InputPort { node_id: n.id.clone(), port: p_name.clone(), pos: pos2(rect.center().x, rect.top()) });
            }
        } else if n_in > 1 {
            for (idx, p_name) in n.ports_in.iter().enumerate() {
                let frac = (idx + 1) as f32 / (n_in + 1) as f32;
                input_ports.push(InputPort { node_id: n.id.clone(), port: p_name.clone(), pos: pos2(rect.left() + rect.width() * frac, rect.top()) });
            }
        }
    }

    let mut display_edges: Vec<GraphEdge> = Vec::new();
    for edge in edges {
        let from_v = if let Some(gid) = member_to_collapsed.get(&edge.from) { format!("group:{gid}") } else { edge.from.clone() };
        let to_v = if let Some(gid) = member_to_collapsed.get(&edge.to) { format!("group:{gid}") } else { edge.to.clone() };
        if from_v == to_v {
            continue;
        }
        display_edges.push(GraphEdge { from: from_v, to: to_v, port: edge.port });
    }

    let drag_id = ui.id().with("node_view_drag");
    let mut drag_from = ui.data_mut(|d| d.get_temp::<String>(drag_id));
    let sel_id = ui.id().with("node_view_selected_edge");
    let mut selected_edge = ui.data_mut(|d| d.get_temp::<(String, String, String)>(sel_id));
    let sel_nodes_id = ui.id().with("node_view_selected_nodes");
    let mut selected_nodes: BTreeSet<String> = ui.data_mut(|d| d.get_temp::<BTreeSet<String>>(sel_nodes_id).unwrap_or_default());
    let menu_id = ui.id().with("node_view_menu");
    let mut menu_state: Option<(Pos2, String)> = ui.data_mut(|d| d.get_temp(menu_id));
    let rename_id = ui.id().with("node_view_rename");
    let mut rename_state: Option<(u64, String)> = ui.data_mut(|d| d.get_temp(rename_id));

    let diagram_size = vec2((total_width + MARGIN_X * 2.0).max(fixed_right), comp_y + NODE_H + MARGIN_Y);
    // The sources of the Display and Write cables: a node id, or None for the composite.
    let fixed_src = |key: &str| graph_val.get(key).and_then(Value::as_str).map(String::from);
    let (display_src, write_src) = (fixed_src("display"), fixed_src("write"));

    // Top bar: Fit scales the graph to the panel width (0.5..=1.0). The choice stays in egui memory.
    let fit_id = ui.id().with(FIT_KEY);
    let mut fit = ui.data_mut(|d| d.get_persisted::<bool>(fit_id).unwrap_or(false));
    let avail_w = ui.available_width();
    ui.horizontal(|ui| {
        if ui.selectable_label(fit, tl!("Fit")).clicked() {
            fit = !fit;
            ui.data_mut(|d| d.insert_persisted(fit_id, fit));
        }
    });
    let s = if fit { fit_scale(avail_w, diagram_size.x) } else { 1.0 };
    // Read by the layout tests: the content size, the Write node's rect and the scale in use.
    let write_rect = rects.get("write").copied();
    ui.data_mut(|d| d.insert_temp(ui.id().with(LAYOUT_KEY), (diagram_size, write_rect, s)));

    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        let (canvas_rect, resp) = ui.allocate_exact_size(diagram_size * s, Sense::click_and_drag());
        resp.on_hover_text(tl!("Drag a row's output dot onto another row to move it above or below that row").to_string());
        let painter = ui.painter_at(canvas_rect);
        // The layout above is in diagram units; draw and hit-test it at scale `s` where the canvas landed.
        let off = canvas_rect.min.to_vec2();
        let at = |p: Pos2| (p.to_vec2() * s + off).to_pos2();
        let port_r = PORT_R * s;
        let font = |px: f32| FontId::proportional(px * s);
        let rects: HashMap<String, Rect> = rects.iter().map(|(k, r)| (k.clone(), Rect::from_min_max(at(r.min), at(r.max)))).collect();
        let input_ports: Vec<InputPort> = input_ports.iter().map(|p| InputPort { pos: at(p.pos), ..p.clone() }).collect();
        let output_ports: HashMap<String, Pos2> = output_ports.iter().map(|(k, p)| (k.clone(), at(*p))).collect();

        let ptr_pos = ui.input(|i| i.pointer.hover_pos());
        let primary_pressed = ui.input(|i| i.pointer.primary_pressed());
        let primary_down = ui.input(|i| i.pointer.primary_down());
        let primary_released = ui.input(|i| i.pointer.primary_released());
        let secondary_clicked = ui.input(|i| i.pointer.button_clicked(PointerButton::Secondary));
        let double_clicked = ui.input(|i| i.pointer.button_double_clicked(PointerButton::Primary));
        // Only over the graph, so Delete elsewhere never cuts a selected cable.
        let delete_pressed = ui.rect_contains_pointer(canvas_rect) && ui.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace));

        // Background subtle dot grid
        let step = 28.0 * s;
        let start_x = (canvas_rect.left() / step).floor() * step;
        let start_y = (canvas_rect.top() / step).floor() * step;
        let mut gx = start_x;
        while gx <= canvas_rect.right() {
            let mut gy = start_y;
            while gy <= canvas_rect.bottom() {
                painter.circle_filled(pos2(gx, gy), 1.0, t.divider);
                gy += step;
            }
            gx += step;
        }

        // Expanded group backdrops
        for g in &groups {
            if g.collapsed {
                continue;
            }
            let member_rects: Vec<Rect> = g.members.iter().filter_map(|m| rects.get(m).copied()).collect();
            if let Some(first) = member_rects.first() {
                let mut union_r = *first;
                for r in &member_rects {
                    union_r = union_r.union(*r);
                }
                let frame_r = union_r.expand2(vec2(10.0, 16.0) * s);
                painter.rect_stroke(frame_r, CornerRadius::same(6), Stroke::new(1.0, t.divider), StrokeKind::Inside);
                let header_r = Rect::from_min_size(frame_r.min, vec2(frame_r.width(), 18.0 * s));
                painter.rect_filled(header_r, CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, t.panel_darker);
                painter.text(pos2(header_r.left() + 6.0 * s, header_r.center().y), Align2::LEFT_CENTER, &g.name, font(11.0), t.text_dim);
                painter.text(pos2(header_r.right() - 8.0 * s, header_r.center().y), Align2::RIGHT_CENTER, "−", font(13.0), t.text_dim);

                if double_clicked && ptr_pos.is_some_and(|p| header_r.contains(p)) {
                    acts.push(Act::Collapse { id: g.id, collapsed: true });
                }
            }
        }

        // Cables computation and hover testing
        let mut hovered_edge_idx: Option<usize> = None;
        let mut edge_curves: Vec<Vec<Pos2>> = Vec::with_capacity(display_edges.len());

        for (i, edge) in display_edges.iter().enumerate() {
            let p0 = output_ports
                .get(&edge.from)
                .copied()
                .unwrap_or_else(|| rects.get(&edge.from).map(|r| pos2(r.center().x, r.bottom())).unwrap_or_default());

            let p3 = input_ports
                .iter()
                .find(|inp| inp.node_id == edge.to && inp.port == edge.port)
                .map(|inp| inp.pos)
                .unwrap_or_else(|| rects.get(&edge.to).map(|r| pos2(r.center().x, r.top())).unwrap_or_default());

            let dy = ((p3.y - p0.y).abs() * 0.5).max(25.0);
            let p1 = Pos2::new(p0.x, p0.y + dy);
            let p2 = Pos2::new(p3.x, p3.y - dy);
            let pts = bezier_points(p0, p1, p2, p3);

            if drag_from.is_none() && ptr_pos.is_some_and(|p| dist_to_curve(p, &pts) <= 7.0) {
                hovered_edge_idx = Some(i);
            }

            edge_curves.push(pts);
        }

        // Check edge selection / right-click disconnect
        if drag_from.is_none() {
            if let Some(h_idx) = hovered_edge_idx {
                if let Some(edge) = display_edges.get(h_idx) {
                    if secondary_clicked {
                        acts.push(Act::Disconnect { from: edge.from.clone(), to: edge.to.clone(), port: edge.port.clone() });
                        selected_edge = None;
                        ui.data_mut(|d| d.remove_temp::<(String, String, String)>(sel_id));
                    } else if primary_pressed {
                        selected_edge = Some((edge.from.clone(), edge.to.clone(), edge.port.clone()));
                        ui.data_mut(|d| d.insert_temp(sel_id, selected_edge.clone()));
                    }
                }
            } else if primary_pressed && !output_ports.values().any(|&pos| ptr_pos.is_some_and(|p| p.distance(pos) <= port_r + 5.0)) {
                selected_edge = None;
                ui.data_mut(|d| d.remove_temp::<(String, String, String)>(sel_id));
            }

            let to_disconnect = if delete_pressed { selected_edge.take() } else { None };
            if let Some((from, to, port)) = to_disconnect {
                acts.push(Act::Disconnect { from, to, port });
                ui.data_mut(|d| d.remove_temp::<(String, String, String)>(sel_id));
            }
        }

        // Node selection and click detection
        if let Some(p) = ptr_pos.filter(|_| drag_from.is_none()) {
            let hit_node = display_nodes.iter().find(|n| rects.get(&n.id).is_some_and(|r| r.contains(p)));
            if let Some(n) = hit_node {
                if double_clicked {
                    if n.kind == "group" {
                        if let Some(gid) = n.id.strip_prefix("group:").and_then(|s| s.parse::<u64>().ok()) {
                            acts.push(Act::Collapse { id: gid, collapsed: false });
                        }
                    } else if let Some(g) = groups.iter().find(|g| g.members.contains(&n.id)) {
                        acts.push(Act::Collapse { id: g.id, collapsed: true });
                    }
                } else if secondary_clicked {
                    if !selected_nodes.contains(&n.id) {
                        selected_nodes.clear();
                        selected_nodes.insert(n.id.clone());
                    }
                    menu_state = Some((p, n.id.clone()));
                } else if primary_pressed && !output_ports.values().any(|&out_pos| p.distance(out_pos) <= port_r + 5.0) {
                    let shift = ui.input(|i| i.modifiers.shift || i.modifiers.command || i.modifiers.ctrl);
                    if shift {
                        if selected_nodes.contains(&n.id) {
                            selected_nodes.remove(&n.id);
                        } else {
                            selected_nodes.insert(n.id.clone());
                        }
                    } else {
                        selected_nodes.clear();
                        selected_nodes.insert(n.id.clone());
                    }
                }
            } else if primary_pressed && !output_ports.values().any(|&out_pos| p.distance(out_pos) <= port_r + 5.0) {
                selected_nodes.clear();
                menu_state = None;
            }
        }

        // Draw cables
        for (i, edge) in display_edges.iter().enumerate() {
            let Some(pts) = edge_curves.get(i) else { continue };
            let is_hovered = hovered_edge_idx == Some(i);
            let is_selected = selected_edge.as_ref().is_some_and(|(from, to, port)| from == &edge.from && to == &edge.to && port == &edge.port);

            let (base_color, base_width, is_dashed) = match edge.port.as_str() {
                "transform" => (t.axis_y, 2.0, false),
                "matte" => (t.warning, 2.0, true),
                _ => (t.selection, 2.0, false),
            };

            let (color, width) = if is_selected {
                (t.highlight, base_width + 1.5)
            } else if is_hovered {
                (t.text_strong, base_width + 1.0)
            } else {
                (base_color, base_width)
            };

            if is_dashed {
                draw_dashed_polyline(&painter, pts, color, width, 6.0, 4.0);
            } else {
                painter.add(Shape::line(pts.clone(), Stroke::new(width, color)));
            }
        }

        // Display and Write cables: from the node they point at, or from the composite when none.
        for (node_id, src) in [("display", display_src.as_deref()), ("write", write_src.as_deref())] {
            let src = src.unwrap_or("comp");
            let p0 = output_ports.get(src).copied().or_else(|| rects.get(src).map(|r| pos2(r.center().x, r.bottom())));
            let p3 = rects.get(node_id).map(|r| pos2(r.center().x, r.top()));
            if let (Some(p0), Some(p3)) = (p0, p3) {
                let dy = ((p3.y - p0.y).abs() * 0.5).max(25.0);
                let pts = bezier_points(p0, Pos2::new(p0.x, p0.y + dy), Pos2::new(p3.x, p3.y - dy), p3);
                painter.add(Shape::line(pts, Stroke::new(2.0, t.accent)));
            }
        }

        // Port dragging interaction
        if let Some(p) = ptr_pos.filter(|_| primary_pressed) {
            for (from_id, &out_pos) in &output_ports {
                if p.distance(out_pos) <= port_r + 5.0 {
                    drag_from = Some(from_id.clone());
                    ui.data_mut(|d| d.insert_temp(drag_id, from_id.clone()));
                    break;
                }
            }
        }

        // Drag preview cable
        let mut hovered_input_port: Option<&InputPort> = None;
        if let (true, Some(from_id), Some(p)) = (primary_down, &drag_from, ptr_pos) {
            if let Some(&p0) = output_ports.get(from_id) {
                let dy = ((p.y - p0.y).abs() * 0.5).max(25.0);
                let p1 = Pos2::new(p0.x, p0.y + dy);
                let p2 = Pos2::new(p.x, p.y - dy);
                let pts = bezier_points(p0, p1, p2, p);
                painter.add(Shape::line(pts, Stroke::new(2.5, t.selection)));
            }

            for inp in &input_ports {
                if p.distance(inp.pos) <= port_r + 6.0 && &inp.node_id != from_id {
                    hovered_input_port = Some(inp);
                    break;
                }
            }
        }

        let released_from = if primary_released { drag_from.take() } else { None };
        if let Some(from_id) = released_from {
            ui.data_mut(|d| d.remove_temp::<String>(drag_id));
            if let Some(p) = ptr_pos {
                let from_is_row = row_nodes.iter().any(|r| r.id == from_id);
                let port_hit = input_ports.iter().find(|inp| p.distance(inp.pos) <= port_r + 8.0 && inp.node_id != from_id);
                if let Some(inp) = port_hit {
                    match branch_cmd(&inp.node_id) {
                        Some(cmd) => acts.push(Act::Branch { cmd, node: Some(from_id.clone()) }),
                        None => acts.push(Act::Connect { from: from_id.clone(), to: inp.node_id.clone(), port: inp.port.clone() }),
                    }
                } else if from_is_row {
                    let hit = row_nodes.iter().find_map(|r| {
                        let rect = rects.get(&r.id)?;
                        (r.id != from_id && rect.contains(p)).then(|| (r.id.clone(), *rect))
                    });
                    if let Some((target, rect)) = hit {
                        acts.push(Act::Reorder { from: from_id.clone(), target, place: place_for_drop(p.y, rect) });
                    }
                }
            }
        }

        // Draw Nodes
        for n in &display_nodes {
            let Some(&rect) = rects.get(&n.id) else { continue };

            let bg_color = t.button;
            let border_color = t.divider;
            painter.rect_filled(rect, CornerRadius::same(5), bg_color);
            painter.rect_stroke(rect, CornerRadius::same(5), Stroke::new(1.0, border_color), StrokeKind::Inside);

            let stripe_color = match n.kind.as_str() {
                "peg" => t.axis_y,
                "row" => t.selection,
                "cutter" => t.warning,
                "effect" => t.axis_x,
                "composite" => t.selection,
                "group" | "display" | "write" => t.accent,
                _ => t.text_dim,
            };
            let stripe_rect = Rect::from_min_max(pos2(rect.left(), rect.top()), pos2(rect.left() + 4.0, rect.bottom()));
            painter.rect_filled(stripe_rect, CornerRadius { nw: 5, sw: 5, ne: 0, se: 0 }, stripe_color);

            let title = n.title();
            painter.text(pos2(rect.left() + 10.0 * s, rect.top() + 12.0 * s), Align2::LEFT_CENTER, title, font(12.0), t.text);

            let badge = n.kind_badge();
            if !badge.is_empty() {
                painter.text(pos2(rect.left() + 10.0 * s, rect.top() + 25.0 * s), Align2::LEFT_CENTER, badge, font(9.0), t.text_dim);
            }

            if selected_nodes.contains(&n.id) {
                painter.rect_stroke(rect, CornerRadius::same(5), Stroke::new(2.0, t.highlight), StrokeKind::Inside);
            }
        }

        // Draw Ports
        for inp in &input_ports {
            let is_hot = hovered_input_port.is_some_and(|h| h.node_id == inp.node_id && h.port == inp.port)
                || (drag_from.is_none() && ptr_pos.is_some_and(|p| p.distance(inp.pos) <= port_r + 3.0));

            let p_color = port_color(&inp.port, &t);
            let radius = if is_hot { port_r + 1.5 } else { port_r };
            let fill = if is_hot { p_color } else { t.panel_darker };
            painter.circle_filled(inp.pos, radius, fill);
            painter.circle_stroke(inp.pos, radius, Stroke::new(1.5, p_color));

            if is_hot {
                painter.text(pos2(inp.pos.x, inp.pos.y - 8.0 * s), Align2::CENTER_BOTTOM, &inp.port, font(10.0), t.text_strong);
            }
        }

        for (from_id, &out_pos) in &output_ports {
            let is_hot = drag_from.as_ref() == Some(from_id) || (drag_from.is_none() && ptr_pos.is_some_and(|p| p.distance(out_pos) <= port_r + 3.0));

            let node_kind = display_nodes.iter().find(|n| &n.id == from_id).map(|n| n.kind.as_str()).unwrap_or("");
            let out_color = if node_kind == "peg" { t.axis_y } else { t.selection };

            let radius = if is_hot { port_r + 1.5 } else { port_r };
            let fill = if is_hot { out_color } else { t.panel_darker };
            painter.circle_filled(out_pos, radius, fill);
            painter.circle_stroke(out_pos, radius, Stroke::new(1.5, out_color));
        }

        // Context Menu
        let mut close_menu = false;
        if let Some((menu_pos, ref target_id)) = menu_state {
            egui::Area::new(ui.id().with("node_view_ctx_area")).fixed_pos(menu_pos).order(egui::Order::Foreground).show(ui.ctx(), |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    if let Some(cmd) = branch_cmd(target_id) {
                        if ui.button(tl!("Reset")).clicked() {
                            acts.push(Act::Branch { cmd, node: None });
                            close_menu = true;
                        }
                        return;
                    }
                    if ["row:", "fx:", "cut:"].iter().any(|p| target_id.starts_with(p)) && ui.button(tl!("Lock Cache")).clicked() {
                        acts.push(Act::Lock { node: target_id.clone() });
                        close_menu = true;
                    }
                    if selected_nodes.len() > 1 && selected_nodes.contains(target_id) && ui.button(tl!("Group")).clicked() {
                        acts.push(Act::Group { members: selected_nodes.iter().cloned().collect() });
                        close_menu = true;
                    }
                    let gid_opt = if let Some(gid_str) = target_id.strip_prefix("group:") {
                        gid_str.parse::<u64>().ok()
                    } else {
                        groups.iter().find(|g| g.members.contains(target_id)).map(|g| g.id)
                    };
                    if let Some(gid) = gid_opt {
                        if ui.button(tl!("Ungroup")).clicked() {
                            acts.push(Act::Ungroup { id: gid });
                            close_menu = true;
                        }
                        if ui.button(tl!("Rename")).clicked() {
                            let cur_name = groups.iter().find(|g| g.id == gid).map(|g| g.name.clone()).unwrap_or_default();
                            rename_state = Some((gid, cur_name));
                            close_menu = true;
                        }
                    }
                });
            });
        }
        if close_menu {
            menu_state = None;
        }
    });

    // Rename dialog
    if let Some((gid, mut name)) = rename_state {
        let mut close_rename = false;
        let mut commit_name = false;
        egui::Window::new(tl!("Rename Group")).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(
            ui.ctx(),
            |ui| {
                ui.horizontal(|ui| {
                    ui.label(tl!("Name:"));
                    let resp = ui.text_edit_singleline(&mut name);
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        commit_name = true;
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button(tl!("OK")).clicked() {
                        commit_name = true;
                    }
                    if ui.button(tl!("Cancel")).clicked() {
                        close_rename = true;
                    }
                });
            },
        );
        if commit_name {
            acts.push(Act::RenameGroup { id: gid, name: name.clone() });
            close_rename = true;
        }
        if close_rename {
            ui.data_mut(|d| d.remove_temp::<(u64, String)>(rename_id));
        } else {
            ui.data_mut(|d| d.insert_temp(rename_id, (gid, name)));
        }
    }

    ui.data_mut(|d| {
        d.insert_temp(sel_nodes_id, selected_nodes);
        d.insert_temp(menu_id, menu_state);
    });

    for act in acts {
        match act {
            Act::Connect { from, to, port } => {
                if let Err(e) = app.run("nodes.connect", json!({"from": from, "to": to, "port": port})) {
                    app.status(e);
                }
            }
            Act::Disconnect { from, to, port } => {
                if let Err(e) = app.run("nodes.disconnect", json!({"from": from, "to": to, "port": port})) {
                    app.status(e);
                }
            }
            Act::Reorder { from, target, place } => {
                if let Err(e) = app.run("nodes.connect", json!({"from": from, "to": "comp", "port": "image", (place): target})) {
                    app.status(e);
                }
            }
            Act::Lock { node } => {
                // The row is locked when `nodes.graph` lists it: then this unlocks it.
                let row = node.split(':').nth(1).and_then(|r| r.parse::<u64>().ok());
                let locked = app
                    .session
                    .execute("nodes.graph", &json!({}))
                    .ok()
                    .and_then(|g| g.get("locked").and_then(|l| l.as_array()).cloned())
                    .unwrap_or_default();
                let on = !row.is_some_and(|r| locked.iter().any(|v| v.as_u64() == Some(r)));
                if let Err(e) = app.run("nodes.lock", json!({"node": node, "on": on})) {
                    app.status(e);
                }
            }
            Act::Group { members } => {
                if let Err(e) = app.run("nodes.group", json!({"members": members})) {
                    app.status(e);
                }
            }
            Act::Ungroup { id } => {
                if let Err(e) = app.run("nodes.ungroup", json!({"id": id})) {
                    app.status(e);
                }
            }
            Act::Collapse { id, collapsed } => {
                if let Err(e) = app.run("nodes.collapse", json!({"id": id, "collapsed": collapsed})) {
                    app.status(e);
                }
            }
            Act::RenameGroup { id, name } => {
                if let Err(e) = app.run("nodes.renameGroup", json!({"id": id, "name": name})) {
                    app.status(e);
                }
            }
            Act::Branch { cmd, node } => {
                if let Err(e) = app.run(cmd, json!({"node": node})) {
                    app.status(e);
                }
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

    /// One frame of the panel in a 500x400 window.
    fn frame(app: &mut VectorcraftApp, ctx: &egui::Context) {
        let screen = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(500.0, 400.0)));
        let input = egui::RawInput { screen_rect: screen, ..Default::default() };
        ctx.run_ui(input, |ui| show(app, ui)).textures_delta.clear();
    }

    fn scene_with_two_rows_and_cutter() -> VectorcraftApp {
        let mut app = app();
        let s = &mut app.session;
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        let r0 = s.doc().unwrap().doc.layers[0].id.0;
        let r1 = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
        s.execute("nodes.connect", &json!({"from": format!("row:{r1}"), "to": format!("row:{r0}"), "port": "matte"})).unwrap();
        app
    }

    #[test]
    fn scene_with_two_rows_and_cutter_renders_without_panic() {
        let mut app = scene_with_two_rows_and_cutter();
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx, Default::default());
        frame(&mut app, &ctx);
    }

    #[test]
    fn drop_in_upper_half_goes_above_lower_half_below() {
        let target = Rect::from_min_size(pos2(0.0, 100.0), vec2(124.0, 36.0));
        assert_eq!(place_for_drop(105.0, target), "above");
        assert_eq!(place_for_drop(129.0, target), "below");
    }

    #[test]
    fn row_reorder_moves_the_row_in_the_composite() {
        let mut app = app();
        let s = &mut app.session;
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        let r0 = s.doc().unwrap().doc.layers[0].id.0;
        let r1 = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
        s.execute("nodes.connect", &json!({"from": format!("row:{r0}"), "to": "comp", "port": "image", "above": format!("row:{r1}")})).unwrap();
        let graph = s.execute("nodes.graph", &json!({})).unwrap();
        let order = |id: u64| {
            graph["edges"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["from"] == format!("row:{id}") && e["to"] == "comp")
                .and_then(|e| e["order"].as_u64())
        };
        assert!(order(r0) > order(r1), "row:{r0} drawn above row:{r1}");
    }

    #[test]
    fn no_document_renders_the_empty_state() {
        let mut app = app();
        let ctx = egui::Context::default();
        frame(&mut app, &ctx);
    }

    #[test]
    fn collapsed_group_renders_without_panic() {
        let mut app = scene_with_two_rows_and_cutter();
        let r0 = app.session.doc().unwrap().doc.layers[0].id.0;
        let r1 = app.session.doc().unwrap().doc.layers[1].id.0;
        let r = app.session.execute("nodes.group", &json!({"members": [format!("row:{r0}"), format!("row:{r1}")]})).unwrap();
        let gid = r["id"].as_u64().unwrap();
        app.session.execute("nodes.collapse", &json!({"id": gid, "collapsed": true})).unwrap();
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx, Default::default());
        frame(&mut app, &ctx);
    }

    #[test]
    fn expanded_group_renders_without_panic() {
        let mut app = scene_with_two_rows_and_cutter();
        let r0 = app.session.doc().unwrap().doc.layers[0].id.0;
        let r1 = app.session.doc().unwrap().doc.layers[1].id.0;
        app.session.execute("nodes.group", &json!({"members": [format!("row:{r0}"), format!("row:{r1}")]})).unwrap();
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx, Default::default());
        frame(&mut app, &ctx);
    }

    #[test]
    fn display_and_write_nodes_render_and_show_what_they_point_at() {
        let mut app = scene_with_two_rows_and_cutter();
        let r0 = app.session.doc().unwrap().doc.layers[0].id.0;
        app.session.execute("nodes.display", &json!({"node": format!("row:{r0}")})).unwrap();
        app.session.execute("nodes.write", &json!({"node": "comp"})).unwrap();
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx, Default::default());
        frame(&mut app, &ctx);
        let g = app.session.execute("nodes.graph", &json!({})).unwrap();
        assert_eq!(g["display"], format!("row:{r0}"));
        assert_eq!(g["write"], "comp");
    }

    #[test]
    fn fixed_nodes_map_to_their_commands() {
        assert_eq!(branch_cmd("display"), Some("nodes.display"));
        assert_eq!(branch_cmd("write"), Some("nodes.write"));
        assert_eq!(branch_cmd(&format!("row:{}", 1)), None);
    }

    /// A file with six rows (one from `file.new`, five from `layer.new`).
    fn six_row_scene() -> VectorcraftApp {
        let mut app = app();
        let s = &mut app.session;
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        for _ in 0..5 {
            s.execute("layer.new", &json!({})).unwrap();
        }
        app
    }

    /// One frame in a `width`-wide panel with Fit set as given: (diagram size, Write rect, scale).
    fn layout_in_panel(app: &mut VectorcraftApp, width: f32, fit: bool) -> (egui::Vec2, Option<Rect>, f32) {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx, Default::default());
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width, 400.0))), ..Default::default() };
        let mut layout = None;
        ctx.run_ui(input, |ui| {
            if fit {
                ui.data_mut(|d| d.insert_persisted(ui.id().with(FIT_KEY), true));
            }
            show(app, ui);
            layout = ui.data_mut(|d| d.get_temp::<(egui::Vec2, Option<Rect>, f32)>(ui.id().with(LAYOUT_KEY)));
        })
        .textures_delta
        .clear();
        layout.expect("the panel stores its layout every frame")
    }

    #[test]
    fn fit_scale_stays_between_half_and_full() {
        assert_eq!(fit_scale(4000.0, 948.0), 1.0);
        assert_eq!(fit_scale(200.0, 948.0), 0.5);
        assert_eq!(fit_scale(300.0, 0.0), 1.0);
        assert_eq!(fit_scale(f32::NAN, 948.0), 1.0);
    }

    #[test]
    fn write_node_is_inside_the_scroll_content_in_a_narrow_panel() {
        let mut app = six_row_scene();
        let (size, write, scale) = layout_in_panel(&mut app, 200.0, false);
        assert_eq!(scale, 1.0);
        assert!(Rect::from_min_size(Pos2::ZERO, size).contains_rect(write.expect("write node laid out")));
    }

    #[test]
    fn fit_scales_the_whole_graph_into_the_panel_width() {
        let mut app = six_row_scene();
        let (size, write, scale) = layout_in_panel(&mut app, 700.0, true);
        assert!(scale > 0.5 && scale < 1.0, "scale {scale}");
        let content = Rect::from_min_size(Pos2::ZERO, size * scale);
        assert!(content.width() <= 700.0 + 0.5, "content width {}", content.width());
        let w = write.expect("write node laid out");
        let w = Rect::from_min_max((w.min.to_vec2() * scale).to_pos2(), (w.max.to_vec2() * scale).to_pos2());
        assert!(content.contains_rect(w));
    }

    #[test]
    fn fit_in_a_200px_panel_stops_at_the_half_scale_floor_and_the_write_node_is_still_reachable() {
        let mut app = six_row_scene();
        let (size, write, scale) = layout_in_panel(&mut app, 200.0, true);
        assert_eq!(scale, 0.5);
        let content = Rect::from_min_size(Pos2::ZERO, size * scale);
        let w = write.expect("write node laid out");
        let w = Rect::from_min_max((w.min.to_vec2() * scale).to_pos2(), (w.max.to_vec2() * scale).to_pos2());
        assert!(content.contains_rect(w));
    }
}
