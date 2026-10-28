//! 7rrek (Creative Studio): node groups, as in Harmony. Simple groups only tidy the view and have
//! no effect on rendering. Stored in document extra data (`"nodeGroups"`).
//! A node id belongs to at most one group. When a row, peg, or other member is removed from the
//! scene, its member entry disappears on read.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vectorcraft_doc::Document;

use super::cutter::Cutters;
use super::peg::Pegs;
use super::row_fx::RowFxList;
use super::*;

pub const KEY: &str = "nodeGroups";
pub const MAX_GROUPS: usize = 256;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct NodeGroup {
    pub id: u64,
    pub name: String,
    pub members: Vec<String>,
    #[serde(default)]
    pub collapsed: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeGroups {
    pub groups: Vec<NodeGroup>,
}

pub fn scene_nodes(d: &Document) -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    let pegs = Pegs::of(d);
    for i in 0..pegs.pegs.len() {
        set.insert(format!("peg:{i}"));
    }
    let cuts = Cutters::of(d);
    let fx = RowFxList::of(d);
    for l in &d.layers {
        let r = l.id.0;
        set.insert(format!("row:{r}"));
        if cuts.cutters.iter().any(|c| c.row == r) {
            set.insert(format!("cut:{r}"));
        }
        if let Some(rf) = fx.rows.iter().find(|f| f.row == r) {
            for i in 0..rf.effects.len() {
                set.insert(format!("fx:{r}:{i}"));
            }
        }
    }
    set
}

impl NodeGroups {
    pub fn of(doc: &Document) -> Self {
        let raw: Vec<NodeGroup> = doc
            .unknown
            .get(KEY)
            .and_then(|v| {
                if v.is_array() {
                    serde_json::from_value(v.clone()).ok()
                } else if let Some(arr) = v.get("groups") {
                    serde_json::from_value(arr.clone()).ok()
                } else {
                    None
                }
            })
            .unwrap_or_default();
        if raw.len() > MAX_GROUPS {
            return NodeGroups::default();
        }
        let valid = scene_nodes(doc);
        let mut groups = Vec::with_capacity(raw.len());
        for mut g in raw {
            g.members.retain(|m| valid.contains(m));
            groups.push(g);
        }
        NodeGroups { groups }
    }

    pub fn store(&self, doc: &mut Document) {
        let valid = scene_nodes(doc);
        let mut clean = Vec::with_capacity(self.groups.len());
        for g in &self.groups {
            let mut g = g.clone();
            g.members.retain(|m| valid.contains(m));
            clean.push(g);
        }
        if clean.is_empty() {
            doc.unknown.remove(KEY);
        } else if let Ok(v) = serde_json::to_value(&clean) {
            doc.unknown.insert(KEY.into(), v);
        }
    }
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "nodes.group",
            "Group Nodes",
            [],
            None,
            "{members: [node ids], name?: string} creates a visual group containing the members → {id}",
            has_doc,
            group
        ),
        cmd!("nodes.ungroup", "Ungroup Nodes", [], None, "{id: group id} dissolves the group → {id}", has_doc, ungroup),
        cmd!(
            "nodes.collapse",
            "Collapse Group",
            [],
            None,
            "{id: group id, collapsed: bool} collapses or expands the group → {id, collapsed}",
            has_doc,
            collapse
        ),
        cmd!("nodes.renameGroup", "Rename Group", [], None, "{id: group id, name: string} renames the group → {id, name}", has_doc, rename_group),
    ]
}

fn group_id_param(p: &Value, cmd: &str) -> Result<u64> {
    if let Some(n) = p.get("id").and_then(Value::as_u64) {
        return Ok(n);
    }
    if let Some(s) = p.get("id").and_then(Value::as_str) {
        let s = s.strip_prefix("grp:").unwrap_or(s);
        if let Ok(n) = s.parse::<u64>() {
            return Ok(n);
        }
    }
    Err(bad(cmd, "`id` must be a group id"))
}

fn group(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "nodes.group";
    let members_val = p.get("members").and_then(Value::as_array).ok_or_else(|| bad(C, "`members` must be an array of node ids"))?;
    if members_val.is_empty() {
        return Err(bad(C, "members cannot be empty"));
    }
    let mut members = Vec::with_capacity(members_val.len());
    for v in members_val {
        let id_str = v.as_str().ok_or_else(|| bad(C, "`members` must be string node ids"))?;
        if id_str == "comp" {
            return Err(bad(C, "comp cannot be grouped"));
        }
        members.push(id_str.to_string());
    }
    let d = &s.doc()?.doc;
    let valid = scene_nodes(d);
    for m in &members {
        if !valid.contains(m) {
            return Err(bad(C, format!("node `{m}` not found in graph")));
        }
    }
    let mut all = NodeGroups::of(d);
    for g in &all.groups {
        for m in &members {
            if g.members.contains(m) {
                return Err(bad(C, format!("node `{m}` is already in a group")));
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    for m in &members {
        if !seen.insert(m) {
            return Err(bad(C, format!("duplicate member `{m}` in group")));
        }
    }
    if all.groups.len() >= MAX_GROUPS {
        return Err(bad(C, format!("a document takes at most {MAX_GROUPS} groups")));
    }
    let id = all.groups.iter().map(|g| g.id).max().unwrap_or(0) + 1;
    let name = p.get("name").and_then(Value::as_str).filter(|s| !s.trim().is_empty()).map(|s| s.to_string()).unwrap_or_else(|| format!("Group {id}"));
    all.groups.push(NodeGroup { id, name, members, collapsed: false });
    s.edit("Group Nodes", |d, _| {
        all.store(d);
        Ok(())
    })?;
    Ok(json!({"id": id}))
}

fn ungroup(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "nodes.ungroup";
    let id = group_id_param(p, C)?;
    let mut all = NodeGroups::of(&s.doc()?.doc);
    let before = all.groups.len();
    all.groups.retain(|g| g.id != id);
    if all.groups.len() == before {
        return Err(bad(C, format!("group `{id}` not found")));
    }
    s.edit("Ungroup Nodes", |d, _| {
        all.store(d);
        Ok(())
    })?;
    Ok(json!({"id": id}))
}

fn collapse(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "nodes.collapse";
    let id = group_id_param(p, C)?;
    let collapsed = p.get("collapsed").and_then(Value::as_bool).ok_or_else(|| bad(C, "`collapsed` must be a boolean"))?;
    let mut all = NodeGroups::of(&s.doc()?.doc);
    let group = all.groups.iter_mut().find(|g| g.id == id).ok_or_else(|| bad(C, format!("group `{id}` not found")))?;
    group.collapsed = collapsed;
    let label = if collapsed { "Collapse Group" } else { "Expand Group" };
    s.edit(label, |d, _| {
        all.store(d);
        Ok(())
    })?;
    Ok(json!({"id": id, "collapsed": collapsed}))
}

fn rename_group(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "nodes.renameGroup";
    let id = group_id_param(p, C)?;
    let name = p.get("name").and_then(Value::as_str).ok_or_else(|| bad(C, "`name` must be a string"))?;
    if name.trim().is_empty() {
        return Err(bad(C, "`name` cannot be empty"));
    }
    let mut all = NodeGroups::of(&s.doc()?.doc);
    let group = all.groups.iter_mut().find(|g| g.id == id).ok_or_else(|| bad(C, format!("group `{id}` not found")))?;
    group.name = name.to_string();
    s.edit("Rename Group", |d, _| {
        all.store(d);
        Ok(())
    })?;
    Ok(json!({"id": id, "name": name}))
}

#[cfg(test)]
mod tests {
    use vectorcraft_geom::Rect;

    use super::*;

    fn scene_3_rows() -> (Session, [u64; 3]) {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        let mut ids = [0; 3];
        for (i, (x, w, c)) in [(0, 100, "#ff0000"), (25, 50, "#0000ff"), (0, 20, "#00ff00")].into_iter().enumerate() {
            if i > 0 {
                let id = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
                s.execute("layer.setCurrent", &json!({"id": id})).unwrap();
            }
            s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
            s.execute("shape.rectangle", &json!({"x": x, "y": x, "width": w, "height": w})).unwrap();
            s.execute("paint.setFill", &json!({"color": c})).unwrap();
        }
        for (i, l) in s.doc().unwrap().doc.layers.iter().enumerate() {
            if let Some(slot) = ids.get_mut(i) {
                *slot = l.id.0;
            }
        }
        (s, ids)
    }

    fn render_bytes(s: &Session) -> Vec<u8> {
        let d = crate::cmd::anim::at_frame(&s.doc().unwrap().doc, 1);
        let img = vectorcraft_render::Renderer::new().render_region(&d, Rect::new(0.0, 0.0, 100.0, 100.0), 1.0, true);
        img.to_straight()
    }

    #[test]
    fn group_collapse_and_expand_keeps_render_identical_and_graph_json_unchanged() {
        let (mut s, [red, blue, green]) = scene_3_rows();
        let before_img = render_bytes(&s);
        let members = vec![format!("row:{red}"), format!("row:{blue}"), format!("row:{green}")];
        let r = s.execute("nodes.group", &json!({"members": members, "name": "TestGroup"})).unwrap();
        let grp_id = r["id"].as_u64().unwrap();

        // Expanded state
        let g_expanded = s.execute("nodes.graph", &json!({})).unwrap();
        assert_eq!(render_bytes(&s), before_img);
        assert_eq!(g_expanded["groups"][0]["collapsed"], false);

        // Collapsed state
        s.execute("nodes.collapse", &json!({"id": grp_id, "collapsed": true})).unwrap();
        let g_collapsed = s.execute("nodes.graph", &json!({})).unwrap();
        assert_eq!(render_bytes(&s), before_img, "export image byte-identical when collapsed");
        assert_eq!(g_collapsed["nodes"], g_expanded["nodes"], "nodes in json are identical when collapsed");
        assert_eq!(g_collapsed["edges"], g_expanded["edges"], "edges in json are identical when collapsed");
        assert_eq!(g_collapsed["groups"][0]["collapsed"], true);

        // Expand again
        s.execute("nodes.collapse", &json!({"id": grp_id, "collapsed": false})).unwrap();
        let g_expanded_again = s.execute("nodes.graph", &json!({})).unwrap();
        assert_eq!(render_bytes(&s), before_img, "export image byte-identical when expanded again");
        assert_eq!(g_expanded_again["nodes"], g_expanded["nodes"]);
        assert_eq!(g_expanded_again["edges"], g_expanded["edges"]);
        assert_eq!(g_expanded_again["groups"][0]["collapsed"], false);
    }

    #[test]
    fn deleting_member_row_removes_it_from_members_on_read() {
        let (mut s, [red, blue, _]) = scene_3_rows();
        s.execute("nodes.group", &json!({"members": [format!("row:{red}"), format!("row:{blue}")]})).unwrap();
        let g1 = s.execute("nodes.graph", &json!({})).unwrap();
        assert_eq!(g1["groups"][0]["members"].as_array().unwrap().len(), 2);

        // Delete the blue row
        s.execute("layer.delete", &json!({"id": blue})).unwrap();
        let g2 = s.execute("nodes.graph", &json!({})).unwrap();
        let members: Vec<&str> = g2["groups"][0]["members"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        assert_eq!(members, vec![format!("row:{red}").as_str()], "deleted member disappeared on read");
    }

    #[test]
    fn undo_after_nodes_group_restores_node_groups_extra_data() {
        let (mut s, [red, blue, _]) = scene_3_rows();
        assert!(s.doc().unwrap().doc.unknown.get(KEY).is_none());

        s.execute("nodes.group", &json!({"members": [format!("row:{red}"), format!("row:{blue}")]})).unwrap();
        assert!(s.doc().unwrap().doc.unknown.get(KEY).is_some());

        s.execute("edit.undo", &json!({})).unwrap();
        assert!(s.doc().unwrap().doc.unknown.get(KEY).is_none(), "undo restored nodeGroups extra data");
    }

    #[test]
    fn bad_inputs_return_err_without_panic() {
        let (mut s, [red, blue, _]) = scene_3_rows();
        // Empty members
        assert!(s.execute("nodes.group", &json!({"members": []})).is_err());
        // Non-existent id
        assert!(s.execute("nodes.group", &json!({"members": ["row:999999"]})).is_err());
        // comp node
        assert!(s.execute("nodes.group", &json!({"members": ["comp"]})).is_err());
        // Create first group
        s.execute("nodes.group", &json!({"members": [format!("row:{red}")]})).unwrap();
        // Same id in second group
        assert!(s.execute("nodes.group", &json!({"members": [format!("row:{red}")]})).is_err());
        // Duplicate id in same group request
        assert!(s.execute("nodes.group", &json!({"members": [format!("row:{blue}"), format!("row:{blue}")]})).is_err());
        // Non-existent group id on ungroup
        assert!(s.execute("nodes.ungroup", &json!({"id": 999999})).is_err());
        // Non-existent group id on collapse
        assert!(s.execute("nodes.collapse", &json!({"id": 999999, "collapsed": true})).is_err());
        // Non-existent group id on renameGroup
        assert!(s.execute("nodes.renameGroup", &json!({"id": 999999, "name": "New"})).is_err());
    }

    #[test]
    fn nodes_graph_assigns_correct_typed_ports_by_node_kind() {
        let (mut s, [red, blue, _]) = scene_3_rows();
        s.execute("peg.add", &json!({})).unwrap();
        s.execute("fx.add", &json!({"row": red, "kind": "blur"})).unwrap();
        s.execute("nodes.connect", &json!({"from": format!("row:{blue}"), "to": format!("row:{red}"), "port": "matte"})).unwrap();

        let g = s.execute("nodes.graph", &json!({})).unwrap();
        let nodes = g["nodes"].as_array().unwrap();

        let row_node = nodes.iter().find(|n| n["id"] == format!("row:{red}")).unwrap();
        assert_eq!(row_node["ports"]["in"], json!(["transform", "matte"]));
        assert_eq!(row_node["ports"]["out"], json!(["image"]));

        let peg_node = nodes.iter().find(|n| n["id"] == "peg:0").unwrap();
        assert_eq!(peg_node["ports"]["in"], json!(["transform"]));
        assert_eq!(peg_node["ports"]["out"], json!(["transform"]));

        let cut_node = nodes.iter().find(|n| n["id"] == format!("cut:{red}")).unwrap();
        assert_eq!(cut_node["ports"]["in"], json!(["image", "matte"]));
        assert_eq!(cut_node["ports"]["out"], json!(["image"]));

        let fx_node = nodes.iter().find(|n| n["id"] == format!("fx:{red}:0")).unwrap();
        assert_eq!(fx_node["ports"]["in"], json!(["image"]));
        assert_eq!(fx_node["ports"]["out"], json!(["image"]));

        let comp_node = nodes.iter().find(|n| n["id"] == "comp").unwrap();
        assert_eq!(comp_node["ports"]["in"], json!(["image"]));
        assert_eq!(comp_node["ports"]["out"], json!(["image"]));
    }
}
