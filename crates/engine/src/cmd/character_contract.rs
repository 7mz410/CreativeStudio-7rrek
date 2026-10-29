//! 7rrek (Creative Studio): character contract checker and specification (`character.check`, `character.contract`).
//! Validates rsm character files against plans/character-contract.md.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use vectorcraft_doc::Document;

use super::*;

/// Maximum nodes walked to prevent cycles or pathological inputs.
const MAX_NODES: usize = 10_000;

#[derive(Clone, Copy, Debug)]
pub struct PartSpec {
    pub name: &'static str,
    pub parent: Option<&'static str>,
    pub required_full: bool,
    pub required_simple: bool,
    pub arabic: &'static str,
    pub default_pivot: &'static str,
    pub default_drawings: Option<&'static [&'static str]>,
    pub default_drawing: Option<&'static str>,
}

pub const REQUIRED_VIEWS: &[&str] = &["front", "3q", "side"];

pub const PARTS: &[PartSpec] = &[
    PartSpec {
        name: "hips",
        parent: None,
        required_full: true,
        required_simple: false,
        arabic: "الورك",
        default_pivot: "centre of the part",
        default_drawings: None,
        default_drawing: None,
    },
    PartSpec {
        name: "torso",
        parent: Some("hips"),
        required_full: true,
        required_simple: true,
        arabic: "الجذع",
        default_pivot: "bottom centre (the waist)",
        default_drawings: None,
        default_drawing: None,
    },
    PartSpec {
        name: "neck",
        parent: Some("torso"),
        required_full: false,
        required_simple: false,
        arabic: "الرقبة",
        default_pivot: "bottom centre",
        default_drawings: None,
        default_drawing: None,
    },
    PartSpec {
        name: "head",
        parent: Some("neck"),
        required_full: true,
        required_simple: true,
        arabic: "الرأس",
        default_pivot: "bottom centre (top of the neck)",
        default_drawings: None,
        default_drawing: None,
    },
    PartSpec {
        name: "eye_L",
        parent: Some("head"),
        required_full: false,
        required_simple: false,
        arabic: "العين اليسرى",
        default_pivot: "centre",
        default_drawings: Some(&["open", "half", "closed"]),
        default_drawing: Some("open"),
    },
    PartSpec {
        name: "eye_R",
        parent: Some("head"),
        required_full: false,
        required_simple: false,
        arabic: "العين اليمنى",
        default_pivot: "centre",
        default_drawings: Some(&["open", "half", "closed"]),
        default_drawing: Some("open"),
    },
    PartSpec {
        name: "brow_L",
        parent: Some("head"),
        required_full: false,
        required_simple: false,
        arabic: "الحاجب الأيسر",
        default_pivot: "centre",
        default_drawings: None,
        default_drawing: None,
    },
    PartSpec {
        name: "brow_R",
        parent: Some("head"),
        required_full: false,
        required_simple: false,
        arabic: "الحاجب الأيمن",
        default_pivot: "centre",
        default_drawings: None,
        default_drawing: None,
    },
    PartSpec {
        name: "ear_L",
        parent: Some("head"),
        required_full: false,
        required_simple: false,
        arabic: "الأذن اليسرى",
        default_pivot: "edge nearest the head's centre",
        default_drawings: None,
        default_drawing: None,
    },
    PartSpec {
        name: "ear_R",
        parent: Some("head"),
        required_full: false,
        required_simple: false,
        arabic: "الأذن اليمنى",
        default_pivot: "edge nearest the head's centre",
        default_drawings: None,
        default_drawing: None,
    },
    PartSpec {
        name: "nose",
        parent: Some("head"),
        required_full: false,
        required_simple: false,
        arabic: "الأنف",
        default_pivot: "top centre",
        default_drawings: None,
        default_drawing: None,
    },
    PartSpec {
        name: "mouth",
        parent: Some("head"),
        required_full: false,
        required_simple: false,
        arabic: "الفم",
        default_pivot: "centre",
        default_drawings: Some(&["A", "B", "C", "D", "E", "F", "G", "H", "X"]),
        default_drawing: Some("X"),
    },
    PartSpec {
        name: "hair_front",
        parent: Some("head"),
        required_full: false,
        required_simple: false,
        arabic: "الشعر الأمامي",
        default_pivot: "top centre",
        default_drawings: None,
        default_drawing: None,
    },
    PartSpec {
        name: "hair_back",
        parent: Some("head"),
        required_full: false,
        required_simple: false,
        arabic: "الشعر الخلفي",
        default_pivot: "top centre",
        default_drawings: None,
        default_drawing: None,
    },
    PartSpec {
        name: "arm_upper_L",
        parent: Some("torso"),
        required_full: true,
        required_simple: false,
        arabic: "الذراع العليا اليسرى",
        default_pivot: "top end (shoulder)",
        default_drawings: None,
        default_drawing: None,
    },
    PartSpec {
        name: "arm_lower_L",
        parent: Some("arm_upper_L"),
        required_full: true,
        required_simple: false,
        arabic: "الساعد الأيسر",
        default_pivot: "top end (elbow)",
        default_drawings: None,
        default_drawing: None,
    },
    PartSpec {
        name: "hand_L",
        parent: Some("arm_lower_L"),
        required_full: true,
        required_simple: true,
        arabic: "اليد اليسرى",
        default_pivot: "top end (wrist)",
        default_drawings: Some(&["open", "fist", "point"]),
        default_drawing: Some("open"),
    },
    PartSpec {
        name: "arm_upper_R",
        parent: Some("torso"),
        required_full: true,
        required_simple: false,
        arabic: "الذراع العليا اليمنى",
        default_pivot: "top end (shoulder)",
        default_drawings: None,
        default_drawing: None,
    },
    PartSpec {
        name: "arm_lower_R",
        parent: Some("arm_upper_R"),
        required_full: true,
        required_simple: false,
        arabic: "الساعد الأيمن",
        default_pivot: "top end (elbow)",
        default_drawings: None,
        default_drawing: None,
    },
    PartSpec {
        name: "hand_R",
        parent: Some("arm_lower_R"),
        required_full: true,
        required_simple: true,
        arabic: "اليد اليمنى",
        default_pivot: "top end (wrist)",
        default_drawings: Some(&["open", "fist", "point"]),
        default_drawing: Some("open"),
    },
    PartSpec {
        name: "leg_upper_L",
        parent: Some("hips"),
        required_full: true,
        required_simple: false,
        arabic: "الفخذ الأيسر",
        default_pivot: "top end (hip joint)",
        default_drawings: None,
        default_drawing: None,
    },
    PartSpec {
        name: "leg_lower_L",
        parent: Some("leg_upper_L"),
        required_full: true,
        required_simple: false,
        arabic: "الساق اليسرى",
        default_pivot: "top end (knee)",
        default_drawings: None,
        default_drawing: None,
    },
    PartSpec {
        name: "foot_L",
        parent: Some("leg_lower_L"),
        required_full: true,
        required_simple: true,
        arabic: "القدم اليسرى",
        default_pivot: "top end (ankle)",
        default_drawings: None,
        default_drawing: None,
    },
    PartSpec {
        name: "leg_upper_R",
        parent: Some("hips"),
        required_full: true,
        required_simple: false,
        arabic: "الفخذ الأيمن",
        default_pivot: "top end (hip joint)",
        default_drawings: None,
        default_drawing: None,
    },
    PartSpec {
        name: "leg_lower_R",
        parent: Some("leg_upper_R"),
        required_full: true,
        required_simple: false,
        arabic: "الساق اليمنى",
        default_pivot: "top end (knee)",
        default_drawings: None,
        default_drawing: None,
    },
    PartSpec {
        name: "foot_R",
        parent: Some("leg_lower_R"),
        required_full: true,
        required_simple: true,
        arabic: "القدم اليمنى",
        default_pivot: "top end (ankle)",
        default_drawings: None,
        default_drawing: None,
    },
];

pub fn part_spec(name: &str) -> Option<&'static PartSpec> {
    PARTS.iter().find(|p| p.name == name)
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "character.check",
            "Check Character",
            [],
            None,
            "{path?: file (.rsm)} check an rsm file (or active document) against the character contract → {ok, profile, views, missing, optional, defaultPivots, drawings, warnings}",
            always,
            check
        ),
        cmd!(
            query "character.contract",
            "Character Contract",
            [],
            None,
            "{} return the character contract specification as JSON → {views, profiles, parts}",
            always,
            contract
        ),
    ]
}

fn detect_profile(doc: &Document) -> &'static str {
    for l in &doc.layers {
        if l.name.as_deref() == Some("profile.simple") {
            return "simple";
        }
        if let Some(children) = l.children() {
            for c in children {
                if c.name.as_deref() == Some("profile.simple") {
                    return "simple";
                }
            }
        }
    }
    "full"
}

pub fn check_doc(doc: &Document) -> Result<Value> {
    let profile = detect_profile(doc);
    let is_simple = profile == "simple";

    let required_parts: Vec<&'static str> =
        PARTS.iter().filter(|p| if is_simple { p.required_simple } else { p.required_full }).map(|p| p.name).collect();

    let optional_parts: Vec<&'static str> =
        PARTS.iter().filter(|p| if is_simple { !p.required_simple } else { !p.required_full }).map(|p| p.name).collect();

    let mut views_found = Vec::new();
    let mut warnings = Vec::new();
    let mut parts_with_pivot = BTreeSet::new();
    let mut parts_present_any_view = BTreeSet::new();
    let mut drawings_map: BTreeMap<String, Value> = BTreeMap::new();
    let mut missing_map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut optional_map: BTreeMap<String, Vec<String>> = BTreeMap::new();

    let mut duplicate_found = false;
    let mut malformed_drawing_found = false;
    let mut total_nodes = 0;

    for &req_view in REQUIRED_VIEWS {
        let view_layer = doc.layers.iter().find(|l| l.name.as_deref() == Some(req_view));
        if let Some(layer) = view_layer {
            views_found.push(req_view.to_string());
            let mut parts_in_view = Vec::new();
            let mut seen_in_view = BTreeSet::new();

            if let Some(kids) = layer.children() {
                for part_node in kids {
                    total_nodes += 1;
                    if total_nodes > MAX_NODES {
                        return Err(bad("character.check", "document has too many nodes"));
                    }

                    let raw_name = match part_node.name.as_deref() {
                        Some(n) => n.trim(),
                        None => continue,
                    };
                    if raw_name.is_empty() || raw_name == "profile.simple" {
                        continue;
                    }

                    let is_known = part_spec(raw_name).is_some();
                    let is_extension = raw_name.starts_with("x_");
                    if !is_known && !is_extension {
                        warnings.push(format!("unknown part name: {raw_name}"));
                    }

                    if is_known && !seen_in_view.insert(raw_name.to_string()) {
                        duplicate_found = true;
                        warnings.push(format!("duplicate part in view '{req_view}': {raw_name}"));
                    } else if is_known {
                        parts_in_view.push(raw_name);
                        parts_present_any_view.insert(raw_name.to_string());
                    }

                    let children = part_node.children();
                    let is_empty = children.is_none_or(|c| c.is_empty());
                    if is_empty {
                        warnings.push(format!("empty group: {raw_name} in view '{req_view}'"));
                    }

                    let has_pivot = children.is_some_and(|c| c.iter().any(|kid| kid.name.as_deref() == Some("pivot")));
                    if is_known && has_pivot {
                        parts_with_pivot.insert(raw_name.to_string());
                    }

                    let mut part_drawings = Vec::new();
                    let prefix = format!("{raw_name}.");
                    if let Some(sub_nodes) = children {
                        for sub in sub_nodes {
                            total_nodes += 1;
                            if total_nodes > MAX_NODES {
                                return Err(bad("character.check", "document has too many nodes"));
                            }
                            if let Some(sub_name) = sub.name.as_deref() {
                                if let Some(drw) = sub_name.strip_prefix(&prefix) {
                                    if !drw.is_empty() {
                                        part_drawings.push(drw.to_string());
                                    } else {
                                        malformed_drawing_found = true;
                                        warnings.push(format!("malformed drawing name in {raw_name}: {sub_name}"));
                                    }
                                } else if sub_name != "pivot" && sub_name.contains('.') {
                                    malformed_drawing_found = true;
                                    warnings.push(format!("malformed drawing name in {raw_name}: {sub_name}"));
                                }
                            }
                        }
                    }

                    if is_known && !part_drawings.is_empty() {
                        let def = part_spec(raw_name).and_then(|spec| spec.default_drawing).filter(|def| part_drawings.iter().any(|d| d == def));
                        let default_drw = def.map(str::to_string).or_else(|| part_drawings.first().cloned()).unwrap_or_default();

                        if raw_name == "mouth" {
                            const MOUTH_CHART: &[&str] = &["A", "B", "C", "D", "E", "F", "G", "H", "X"];
                            let missing_mouth: Vec<&str> = MOUTH_CHART.iter().copied().filter(|m| !part_drawings.iter().any(|d| d == m)).collect();
                            if !missing_mouth.is_empty() {
                                warnings.push(format!("incomplete mouth chart: missing {}", missing_mouth.join(", ")));
                            }
                        }

                        drawings_map.insert(
                            raw_name.to_string(),
                            json!({
                                "drawings": part_drawings,
                                "default": default_drw,
                            }),
                        );
                    } else if is_known && !drawings_map.contains_key(raw_name) {
                        drawings_map.insert(
                            raw_name.to_string(),
                            json!({
                                "drawings": [raw_name],
                                "default": raw_name,
                            }),
                        );
                    }
                }
            }

            let missing_req: Vec<String> = required_parts.iter().filter(|req| !parts_in_view.contains(req)).map(|s| s.to_string()).collect();
            missing_map.insert(req_view.to_string(), missing_req);

            let missing_opt: Vec<String> = optional_parts.iter().filter(|opt| !parts_in_view.contains(opt)).map(|s| s.to_string()).collect();
            optional_map.insert(req_view.to_string(), missing_opt);
        } else {
            warnings.push(format!("missing view: {req_view}"));
            missing_map.insert(req_view.to_string(), required_parts.iter().map(|s| s.to_string()).collect());
            optional_map.insert(req_view.to_string(), optional_parts.iter().map(|s| s.to_string()).collect());
        }
    }

    let all_views_present = views_found.len() == REQUIRED_VIEWS.len();
    let no_missing_required = missing_map.values().all(|v| v.is_empty());
    let ok = all_views_present && no_missing_required && !duplicate_found && !malformed_drawing_found;

    let default_pivots: Vec<String> = parts_present_any_view.into_iter().filter(|p| !parts_with_pivot.contains(p)).collect();

    Ok(json!({
        "ok": ok,
        "profile": profile,
        "views": views_found,
        "missing": missing_map,
        "optional": optional_map,
        "defaultPivots": default_pivots,
        "drawings": drawings_map,
        "warnings": warnings,
    }))
}

fn check(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "character.check";
    let doc_storage: Document;
    let doc: &Document = if let Some(path) = p.get("path").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
        let bytes = super::fileio::read_file(path).map_err(|e| bad(C, format!("cannot read file '{path}': {e}")))?;
        doc_storage = vectorcraft_format::load(&bytes).map_err(|e| bad(C, format!("cannot parse file '{path}': {e}")))?;
        &doc_storage
    } else {
        match s.doc() {
            Ok(st) => &st.doc,
            Err(_) => return Err(bad(C, "no document open and no path given")),
        }
    };
    check_doc(doc)
}

fn contract(_: &mut Session, _: &Value) -> Result<Value> {
    let full_required: Vec<&'static str> = PARTS.iter().filter(|p| p.required_full).map(|p| p.name).collect();
    let simple_required: Vec<&'static str> = PARTS.iter().filter(|p| p.required_simple).map(|p| p.name).collect();

    let parts_json: Vec<Value> = PARTS
        .iter()
        .map(|p| {
            json!({
                "name": p.name,
                "parent": p.parent,
                "required": p.required_full,
                "requiredFull": p.required_full,
                "requiredSimple": p.required_simple,
                "arabic": p.arabic,
                "defaultPivot": p.default_pivot,
                "defaultDrawings": p.default_drawings,
                "defaultDrawing": p.default_drawing,
            })
        })
        .collect();

    Ok(json!({
        "views": REQUIRED_VIEWS,
        "profiles": {
            "full": full_required,
            "simple": simple_required,
        },
        "parts": parts_json,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_part_group(s: &mut Session, name: &str, with_pivot: bool) {
        let shape = s.execute("shape.rectangle", &json!({"x": 10.0, "y": 10.0, "width": 20.0, "height": 30.0})).unwrap()["id"].as_u64().unwrap();
        if with_pivot {
            let pivot = s.execute("shape.ellipse", &json!({"x": 20.0, "y": 10.0, "width": 2.0, "height": 2.0})).unwrap()["id"].as_u64().unwrap();
            s.execute("object.setProps", &json!({"id": pivot, "name": "pivot"})).unwrap();
            s.execute("select.set", &json!({"ids": [shape, pivot]})).unwrap();
        } else {
            s.execute("select.set", &json!({"ids": [shape]})).unwrap();
        }
        let grp = s.execute("object.group", &json!({})).unwrap()["id"].as_u64().unwrap();
        s.execute("object.setProps", &json!({"id": grp, "name": name})).unwrap();
    }

    fn make_view_with_parts(s: &mut Session, view_name: &str, parts: &[&str], pivot_part: Option<&str>) {
        let layer_id = s.execute("layer.new", &json!({"name": view_name})).unwrap()["id"].as_u64().unwrap();
        s.execute("layer.setCurrent", &json!({"id": layer_id})).unwrap();
        for &p in parts {
            make_part_group(s, p, pivot_part == Some(p));
        }
    }

    fn full_required_parts() -> Vec<&'static str> {
        PARTS.iter().filter(|p| p.required_full).map(|p| p.name).collect()
    }

    fn simple_required_parts() -> Vec<&'static str> {
        PARTS.iter().filter(|p| p.required_simple).map(|p| p.name).collect()
    }

    /// 1. شخصية كاملة (15 جزء مطلوب بـ 3 views): ok true، missing فاضية
    #[test]
    fn complete_character_three_views_is_ok() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 500.0, "height": 500.0})).unwrap();
        let parts = full_required_parts();
        for &v in REQUIRED_VIEWS {
            make_view_with_parts(&mut s, v, &parts, None);
        }

        let res = s.execute("character.check", &json!({})).unwrap();
        assert_eq!(res["ok"], true);
        assert_eq!(res["profile"], "full");
        assert_eq!(res["views"], json!(["front", "3q", "side"]));
        for &v in REQUIRED_VIEWS {
            assert!(res["missing"][v].as_array().unwrap().is_empty(), "missing in {v} should be empty");
        }
    }

    /// 2. نفسها بدون hand_L: ok false، وmissing فيها hand_L
    #[test]
    fn missing_hand_l_fails_check() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 500.0, "height": 500.0})).unwrap();
        let mut parts = full_required_parts();
        parts.retain(|p| *p != "hand_L");
        for &v in REQUIRED_VIEWS {
            make_view_with_parts(&mut s, v, &parts, None);
        }

        let res = s.execute("character.check", &json!({})).unwrap();
        assert_eq!(res["ok"], false);
        for &v in REQUIRED_VIEWS {
            assert!(res["missing"][v].as_array().unwrap().contains(&json!("hand_L")));
        }
    }

    /// 3. 3 views (front، 3q، side) وside ناقصها foot_R: الخطأ بيطلع على side بس
    #[test]
    fn only_side_missing_foot_r_reports_error_on_side_only() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 500.0, "height": 500.0})).unwrap();
        let full_parts = full_required_parts();
        let mut side_parts = full_parts.clone();
        side_parts.retain(|p| *p != "foot_R");

        make_view_with_parts(&mut s, "front", &full_parts, None);
        make_view_with_parts(&mut s, "3q", &full_parts, None);
        make_view_with_parts(&mut s, "side", &side_parts, None);

        let res = s.execute("character.check", &json!({})).unwrap();
        assert_eq!(res["ok"], false);
        assert!(res["missing"]["front"].as_array().unwrap().is_empty(), "front should have no missing parts");
        assert!(res["missing"]["3q"].as_array().unwrap().is_empty(), "3q should have no missing parts");
        assert_eq!(res["missing"]["side"], json!(["foot_R"]), "side should report foot_R missing");
    }

    /// 4. pivot: جزء فيه object اسمه `pivot` ما بيطلع بـ defaultPivots، والباقي بيطلعوا
    #[test]
    fn custom_pivot_excluded_from_default_pivots() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 500.0, "height": 500.0})).unwrap();
        let parts = full_required_parts();
        // Give head a custom pivot
        for &v in REQUIRED_VIEWS {
            make_view_with_parts(&mut s, v, &parts, Some("head"));
        }

        let res = s.execute("character.check", &json!({})).unwrap();
        let defaults = res["defaultPivots"].as_array().unwrap();
        assert!(!defaults.contains(&json!("head")), "head has custom pivot, should not be in defaultPivots");
        assert!(defaults.contains(&json!("torso")), "torso has no custom pivot, must be in defaultPivots");
        assert!(defaults.contains(&json!("arm_upper_L")), "arm_upper_L must be in defaultPivots");
    }

    /// 5. mouth فيه A–H وX: ما في تحذير. mouth فيه A وX بس: تحذير جدول ناقص
    #[test]
    fn mouth_chart_completeness_warning() {
        // Complete mouth chart
        {
            let mut s = Session::new();
            s.execute("file.new", &json!({"width": 500.0, "height": 500.0})).unwrap();
            let layer_id = s.execute("layer.new", &json!({"name": "front"})).unwrap()["id"].as_u64().unwrap();
            s.execute("layer.setCurrent", &json!({"id": layer_id})).unwrap();

            let mut drw_ids = Vec::new();
            for shape_name in ["A", "B", "C", "D", "E", "F", "G", "H", "X"] {
                let r = s.execute("shape.rectangle", &json!({"x": 0.0, "y": 0.0, "width": 5.0, "height": 5.0})).unwrap()["id"].as_u64().unwrap();
                s.execute("select.set", &json!({"ids": [r]})).unwrap();
                let grp = s.execute("object.group", &json!({})).unwrap()["id"].as_u64().unwrap();
                s.execute("object.setProps", &json!({"id": grp, "name": format!("mouth.{shape_name}")})).unwrap();
                drw_ids.push(grp);
            }
            s.execute("select.set", &json!({"ids": drw_ids})).unwrap();
            let mouth_grp = s.execute("object.group", &json!({})).unwrap()["id"].as_u64().unwrap();
            s.execute("object.setProps", &json!({"id": mouth_grp, "name": "mouth"})).unwrap();

            let res = s.execute("character.check", &json!({})).unwrap();
            let warnings = res["warnings"].as_array().unwrap();
            assert!(!warnings.iter().any(|w| w.as_str().unwrap().contains("mouth")), "complete mouth chart must not warn");
            assert_eq!(res["drawings"]["mouth"]["default"], "X");
        }

        // Incomplete mouth chart: only A and X
        {
            let mut s = Session::new();
            s.execute("file.new", &json!({"width": 500.0, "height": 500.0})).unwrap();
            let layer_id = s.execute("layer.new", &json!({"name": "front"})).unwrap()["id"].as_u64().unwrap();
            s.execute("layer.setCurrent", &json!({"id": layer_id})).unwrap();

            let mut drw_ids = Vec::new();
            for shape_name in ["A", "X"] {
                let r = s.execute("shape.rectangle", &json!({"x": 0.0, "y": 0.0, "width": 5.0, "height": 5.0})).unwrap()["id"].as_u64().unwrap();
                s.execute("select.set", &json!({"ids": [r]})).unwrap();
                let grp = s.execute("object.group", &json!({})).unwrap()["id"].as_u64().unwrap();
                s.execute("object.setProps", &json!({"id": grp, "name": format!("mouth.{shape_name}")})).unwrap();
                drw_ids.push(grp);
            }
            s.execute("select.set", &json!({"ids": drw_ids})).unwrap();
            let mouth_grp = s.execute("object.group", &json!({})).unwrap()["id"].as_u64().unwrap();
            s.execute("object.setProps", &json!({"id": mouth_grp, "name": "mouth"})).unwrap();

            let res = s.execute("character.check", &json!({})).unwrap();
            let warnings = res["warnings"].as_array().unwrap();
            assert!(warnings.iter().any(|w| w.as_str().unwrap().contains("mouth")), "incomplete mouth chart must warn");
        }
    }

    /// 6. اسم مش معروف `tail`: تحذير، و`x_tail`: ولا إشي. جزء مكرر بنفس الـ view: تحذير
    #[test]
    fn unknown_names_and_duplicates_warnings() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 500.0, "height": 500.0})).unwrap();
        let layer_id = s.execute("layer.new", &json!({"name": "front"})).unwrap()["id"].as_u64().unwrap();
        s.execute("layer.setCurrent", &json!({"id": layer_id})).unwrap();

        make_part_group(&mut s, "tail", false);
        make_part_group(&mut s, "x_tail", false);
        make_part_group(&mut s, "torso", false);
        make_part_group(&mut s, "torso", false); // duplicate in front view

        let res = s.execute("character.check", &json!({})).unwrap();
        let warnings = res["warnings"].as_array().unwrap();

        assert!(warnings.iter().any(|w| w.as_str().unwrap().contains("tail") && !w.as_str().unwrap().contains("x_tail")), "unknown 'tail' must warn");
        assert!(!warnings.iter().any(|w| w.as_str().unwrap().contains("x_tail")), "'x_tail' must not generate a warning");
        assert!(
            warnings.iter().any(|w| w.as_str().unwrap().contains("duplicate") && w.as_str().unwrap().contains("torso")),
            "duplicate torso must warn"
        );
        assert_eq!(res["ok"], false, "duplicate part makes ok false");
    }

    /// 7. character.contract بيرجّع 15 جزء مطلوب، وأب كل جزء متل الشجرة بقسم 2
    #[test]
    fn contract_spec_returns_15_required_parts_and_parent_tree() {
        let mut s = Session::new();
        let res = s.execute("character.contract", &json!({})).unwrap();

        let parts = res["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 26);

        let full_required = res["profiles"]["full"].as_array().unwrap();
        assert_eq!(full_required.len(), 15, "full profile requires exactly 15 parts");

        let simple_required = res["profiles"]["simple"].as_array().unwrap();
        assert_eq!(simple_required.len(), 6, "simple profile requires exactly 6 parts");

        let find_part = |name: &str| parts.iter().find(|p| p["name"] == name).unwrap();

        assert!(find_part("hips")["parent"].is_null(), "hips is root");
        assert_eq!(find_part("torso")["parent"], "hips");
        assert_eq!(find_part("neck")["parent"], "torso");
        assert_eq!(find_part("head")["parent"], "neck");
        assert_eq!(find_part("arm_upper_L")["parent"], "torso");
        assert_eq!(find_part("arm_lower_L")["parent"], "arm_upper_L");
        assert_eq!(find_part("hand_L")["parent"], "arm_lower_L");
        assert_eq!(find_part("arm_upper_R")["parent"], "torso");
        assert_eq!(find_part("arm_lower_R")["parent"], "arm_upper_R");
        assert_eq!(find_part("hand_R")["parent"], "arm_lower_R");
        assert_eq!(find_part("leg_upper_L")["parent"], "hips");
        assert_eq!(find_part("leg_lower_L")["parent"], "leg_upper_L");
        assert_eq!(find_part("foot_L")["parent"], "leg_lower_L");
        assert_eq!(find_part("leg_upper_R")["parent"], "hips");
        assert_eq!(find_part("leg_lower_R")["parent"], "leg_upper_R");
        assert_eq!(find_part("foot_R")["parent"], "leg_lower_R");
        assert_eq!(find_part("eye_L")["parent"], "head");
        assert_eq!(find_part("mouth")["parent"], "head");
    }

    /// 8. path غلط = Err بدون panic
    #[test]
    fn bad_path_returns_err_without_panic() {
        let mut s = Session::new();
        let res = s.execute("character.check", &json!({"path": "/path/to/nonexistent_file.rsm"}));
        assert!(res.is_err(), "nonexistent file path must return Err");
    }

    #[test]
    fn saved_file_is_checked_by_path() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 500.0, "height": 500.0})).unwrap();
        for v in REQUIRED_VIEWS {
            make_view_with_parts(&mut s, v, &full_required_parts(), None);
        }
        let path = std::env::temp_dir().join(format!("contract-{}.rsm", std::process::id()));
        s.execute("document.save", &json!({"path": path})).unwrap();
        let mut other = Session::new();
        let r = other.execute("character.check", &json!({"path": path})).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(r["ok"], true, "{r}");
    }

    /// 9. شخصية simple كاملة = ok
    #[test]
    fn complete_simple_character_is_ok() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 500.0, "height": 500.0})).unwrap();
        // Add profile.simple layer
        s.execute("layer.new", &json!({"name": "profile.simple"})).unwrap();

        let parts = simple_required_parts();
        for &v in REQUIRED_VIEWS {
            make_view_with_parts(&mut s, v, &parts, None);
        }

        let res = s.execute("character.check", &json!({})).unwrap();
        assert_eq!(res["profile"], "simple");
        assert_eq!(res["ok"], true);
        for &v in REQUIRED_VIEWS {
            assert!(res["missing"][v].as_array().unwrap().is_empty(), "simple profile should have no missing parts");
        }
    }

    /// 10. simple بدون hand_R = missing hand_R
    #[test]
    fn simple_character_without_hand_r_reports_missing() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 500.0, "height": 500.0})).unwrap();
        s.execute("layer.new", &json!({"name": "profile.simple"})).unwrap();

        let mut parts = simple_required_parts();
        parts.retain(|p| *p != "hand_R");
        for &v in REQUIRED_VIEWS {
            make_view_with_parts(&mut s, v, &parts, None);
        }

        let res = s.execute("character.check", &json!({})).unwrap();
        assert_eq!(res["ok"], false);
        for &v in REQUIRED_VIEWS {
            assert!(res["missing"][v].as_array().unwrap().contains(&json!("hand_R")));
        }
    }

    /// 11. ملف فيه front وside بس = ok false وmissing view 3q
    #[test]
    fn missing_3q_view_fails_check_and_reports_missing_view() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 500.0, "height": 500.0})).unwrap();
        let parts = full_required_parts();
        make_view_with_parts(&mut s, "front", &parts, None);
        make_view_with_parts(&mut s, "side", &parts, None);

        let res = s.execute("character.check", &json!({})).unwrap();
        assert_eq!(res["ok"], false);
        assert_eq!(res["views"], json!(["front", "side"]));
        assert!(!res["views"].as_array().unwrap().contains(&json!("3q")));
        assert!(res["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("3q")));
    }
}
