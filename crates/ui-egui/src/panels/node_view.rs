//! Node view panel: the scene's wiring (rows, pegs, cutters, row effects into the composite) as
//! a graph, read from `nodes.graph`; dragging a cable runs `nodes.connect` / `nodes.disconnect`.

use std::collections::HashMap;

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

/// Parsed node representation from `nodes.graph`.
#[derive(Clone, Debug)]
struct GraphNode {
    id: String,
    kind: String,
    name: String,
    inverted: bool,
    fx: String,
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
            _ => "",
        }
    }
}

/// Parsed edge from `nodes.graph`.
#[derive(Clone, Debug)]
struct GraphEdge {
    from: String,
    to: String,
    port: String,
}

enum Act {
    Connect { from: String, to: String, port: String },
    Disconnect { to: String, port: String },
}

fn parse_graph(val: &Value) -> (Vec<GraphNode>, Vec<GraphEdge>) {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

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

            nodes.push(GraphNode { id, kind, name, inverted, fx });
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

    (nodes, edges)
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

    let (nodes, edges) = parse_graph(&graph_val);
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

    let max_y = rects.values().map(|r| r.bottom()).fold(row_y + NODE_H, f32::max);
    let comp_y = max_y + GAP_Y;
    let actual_total_cols = row_nodes.len().max(peg_nodes.len()).max(next_col).max(1);
    let total_width = col_x(actual_total_cols.saturating_sub(1)) + NODE_W - MARGIN_X;
    let comp_x = MARGIN_X + (total_width - NODE_W).max(0.0) * 0.5;
    rects.insert("comp".to_string(), Rect::from_min_size(pos2(comp_x, comp_y), vec2(NODE_W, NODE_H)));

    let mut input_ports: Vec<InputPort> = Vec::new();
    let mut output_ports: HashMap<String, Pos2> = HashMap::new();

    for n in &nodes {
        let Some(rect) = rects.get(&n.id) else { continue };

        if n.kind != "composite" {
            output_ports.insert(n.id.clone(), pos2(rect.center().x, rect.bottom()));
        }

        match n.kind.as_str() {
            "peg" => {
                input_ports.push(InputPort { node_id: n.id.clone(), port: "transform".into(), pos: pos2(rect.center().x, rect.top()) });
            }
            "row" => {
                input_ports.push(InputPort {
                    node_id: n.id.clone(),
                    port: "transform".into(),
                    pos: pos2(rect.left() + rect.width() * 0.3, rect.top()),
                });
                input_ports.push(InputPort { node_id: n.id.clone(), port: "matte".into(), pos: pos2(rect.left() + rect.width() * 0.7, rect.top()) });
            }
            "cutter" => {
                input_ports.push(InputPort { node_id: n.id.clone(), port: "image".into(), pos: pos2(rect.left() + rect.width() * 0.3, rect.top()) });
                input_ports.push(InputPort { node_id: n.id.clone(), port: "matte".into(), pos: pos2(rect.left() + rect.width() * 0.7, rect.top()) });
            }
            _ => {
                input_ports.push(InputPort { node_id: n.id.clone(), port: "image".into(), pos: pos2(rect.center().x, rect.top()) });
            }
        }
    }

    let drag_id = ui.id().with("node_view_drag");
    let mut drag_from = ui.data_mut(|d| d.get_temp::<String>(drag_id));
    let sel_id = ui.id().with("node_view_selected_edge");
    let mut selected_edge = ui.data_mut(|d| d.get_temp::<(String, String)>(sel_id));

    let diagram_size = vec2(total_width + MARGIN_X * 2.0, comp_y + NODE_H + MARGIN_Y);

    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        let (canvas_rect, _resp) = ui.allocate_exact_size(diagram_size, Sense::click_and_drag());
        let painter = ui.painter_at(canvas_rect);

        let ptr_pos = ui.input(|i| i.pointer.hover_pos());
        let primary_pressed = ui.input(|i| i.pointer.primary_pressed());
        let primary_down = ui.input(|i| i.pointer.primary_down());
        let primary_released = ui.input(|i| i.pointer.primary_released());
        let secondary_clicked = ui.input(|i| i.pointer.button_clicked(PointerButton::Secondary));
        // Only over the graph, so Delete elsewhere never cuts a selected cable.
        let delete_pressed = ui.rect_contains_pointer(canvas_rect) && ui.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace));

        // Background subtle dot grid
        let step = 28.0;
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

        // Cables computation and hover testing
        let mut hovered_edge_idx: Option<usize> = None;
        let mut edge_curves: Vec<Vec<Pos2>> = Vec::with_capacity(edges.len());

        for (i, edge) in edges.iter().enumerate() {
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
                if let Some(edge) = edges.get(h_idx) {
                    if secondary_clicked {
                        acts.push(Act::Disconnect { to: edge.to.clone(), port: edge.port.clone() });
                        selected_edge = None;
                        ui.data_mut(|d| d.remove_temp::<(String, String)>(sel_id));
                    } else if primary_pressed {
                        selected_edge = Some((edge.to.clone(), edge.port.clone()));
                        ui.data_mut(|d| d.insert_temp(sel_id, selected_edge.clone()));
                    }
                }
            } else if primary_pressed && !output_ports.values().any(|&pos| ptr_pos.is_some_and(|p| p.distance(pos) <= PORT_R + 5.0)) {
                selected_edge = None;
                ui.data_mut(|d| d.remove_temp::<(String, String)>(sel_id));
            }

            let to_disconnect = if delete_pressed { selected_edge.take() } else { None };
            if let Some((to, port)) = to_disconnect {
                acts.push(Act::Disconnect { to, port });
                ui.data_mut(|d| d.remove_temp::<(String, String)>(sel_id));
            }
        }

        // Draw cables
        for (i, edge) in edges.iter().enumerate() {
            let Some(pts) = edge_curves.get(i) else { continue };
            let is_hovered = hovered_edge_idx == Some(i);
            let is_selected = selected_edge.as_ref().is_some_and(|(to, port)| to == &edge.to && port == &edge.port);

            let (base_color, base_width, is_dashed) = match edge.port.as_str() {
                "transform" => (t.accent, 2.0, false),
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

        // Port dragging interaction
        if let Some(p) = ptr_pos.filter(|_| primary_pressed) {
            for (from_id, &out_pos) in &output_ports {
                if p.distance(out_pos) <= PORT_R + 5.0 {
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
                painter.add(Shape::line(pts, Stroke::new(2.5, t.accent_strong)));
            }

            for inp in &input_ports {
                if p.distance(inp.pos) <= PORT_R + 6.0 && &inp.node_id != from_id {
                    hovered_input_port = Some(inp);
                    break;
                }
            }
        }

        let released_from = if primary_released { drag_from.take() } else { None };
        if let Some(from_id) = released_from {
            ui.data_mut(|d| d.remove_temp::<String>(drag_id));
            if let Some(p) = ptr_pos {
                for inp in &input_ports {
                    if p.distance(inp.pos) <= PORT_R + 8.0 && inp.node_id != from_id {
                        acts.push(Act::Connect { from: from_id.clone(), to: inp.node_id.clone(), port: inp.port.clone() });
                        break;
                    }
                }
            }
        }

        // Draw Nodes
        for n in &nodes {
            let Some(&rect) = rects.get(&n.id) else { continue };

            let bg_color = t.button;
            let border_color = t.divider;
            painter.rect_filled(rect, CornerRadius::same(5), bg_color);
            painter.rect_stroke(rect, CornerRadius::same(5), Stroke::new(1.0, border_color), StrokeKind::Inside);

            let stripe_color = match n.kind.as_str() {
                "peg" => t.accent,
                "row" => t.selection,
                "cutter" => t.warning,
                "effect" => t.axis_x,
                "composite" => t.axis_z,
                _ => t.text_dim,
            };
            let stripe_rect = Rect::from_min_max(pos2(rect.left(), rect.top()), pos2(rect.left() + 4.0, rect.bottom()));
            painter.rect_filled(stripe_rect, CornerRadius { nw: 5, sw: 5, ne: 0, se: 0 }, stripe_color);

            let title = n.title();
            painter.text(pos2(rect.left() + 10.0, rect.top() + 12.0), Align2::LEFT_CENTER, title, FontId::proportional(12.0), t.text);

            let badge = n.kind_badge();
            if !badge.is_empty() {
                painter.text(pos2(rect.left() + 10.0, rect.top() + 25.0), Align2::LEFT_CENTER, badge, FontId::proportional(9.0), t.text_dim);
            }
        }

        // Draw Ports
        for inp in &input_ports {
            let is_hot = hovered_input_port.is_some_and(|h| h.node_id == inp.node_id && h.port == inp.port)
                || (drag_from.is_none() && ptr_pos.is_some_and(|p| p.distance(inp.pos) <= PORT_R + 3.0));

            let port_color = match inp.port.as_str() {
                "transform" => t.accent,
                "matte" => t.warning,
                _ => t.selection,
            };

            let radius = if is_hot { PORT_R + 1.5 } else { PORT_R };
            let fill = if is_hot { port_color } else { t.panel_darker };
            painter.circle_filled(inp.pos, radius, fill);
            painter.circle_stroke(inp.pos, radius, Stroke::new(1.5, port_color));

            if is_hot {
                painter.text(pos2(inp.pos.x, inp.pos.y - 8.0), Align2::CENTER_BOTTOM, &inp.port, FontId::proportional(10.0), t.text_strong);
            }
        }

        for (from_id, &out_pos) in &output_ports {
            let is_hot = drag_from.as_ref() == Some(from_id) || (drag_from.is_none() && ptr_pos.is_some_and(|p| p.distance(out_pos) <= PORT_R + 3.0));

            let node_kind = nodes.iter().find(|n| &n.id == from_id).map(|n| n.kind.as_str()).unwrap_or("");
            let out_color = if node_kind == "peg" { t.accent } else { t.selection };

            let radius = if is_hot { PORT_R + 1.5 } else { PORT_R };
            let fill = if is_hot { out_color } else { t.panel_darker };
            painter.circle_filled(out_pos, radius, fill);
            painter.circle_stroke(out_pos, radius, Stroke::new(1.5, out_color));
        }
    });

    for act in acts {
        match act {
            Act::Connect { from, to, port } => {
                if let Err(e) = app.run("nodes.connect", json!({"from": from, "to": to, "port": port})) {
                    app.status(e);
                }
            }
            Act::Disconnect { to, port } => {
                if let Err(e) = app.run("nodes.disconnect", json!({"to": to, "port": port})) {
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
    fn no_document_renders_the_empty_state() {
        let mut app = app();
        let ctx = egui::Context::default();
        frame(&mut app, &ctx);
    }
}
