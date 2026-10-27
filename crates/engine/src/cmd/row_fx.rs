//! 7rrek (Creative Studio): animation row effects. Blur, glow, drop shadow, and colour adjust
//! applied per timeline row and keyed across frames. Parameters interpolate linearly between
//! keyframes and stay held past the last keyframe.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vectorcraft_doc::{Document, Effect, NodeId};

use super::selector::Part;

pub const KEY: &str = "rowFx";
pub const MAX_ROW_EFFECTS: usize = 32;
/// Marks a live effect added for a frame, so `unposed` removes it and never the user's own.
pub const MARK: &str = "rowFx";

/// The supported effect kinds on an animation row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FxKind {
    #[default]
    Blur,
    Glow,
    DropShadow,
    ColorAdjust,
    Tint,
}

impl FxKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "blur" => Some(Self::Blur),
            "glow" => Some(Self::Glow),
            "dropShadow" => Some(Self::DropShadow),
            "colorAdjust" => Some(Self::ColorAdjust),
            "tint" => Some(Self::Tint),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Blur => "blur",
            Self::Glow => "glow",
            Self::DropShadow => "dropShadow",
            Self::ColorAdjust => "colorAdjust",
            Self::Tint => "tint",
        }
    }

    pub fn default_params(&self) -> Value {
        match self {
            Self::Blur => json!({"radius": 5.0}),
            Self::Glow => json!({"blur": 5.0, "color": "#ffff00", "opacity": 75.0, "mode": "screen"}),
            Self::DropShadow => json!({"x": 7.0, "y": 7.0, "blur": 5.0, "opacity": 75.0, "color": "#000000", "mode": "multiply"}),
            Self::ColorAdjust => json!({"brightness": 0.0, "contrast": 0.0}),
            Self::Tint => json!({"color": "#ff8800", "amount": 50.0}),
        }
    }

    pub fn live_effect_id(&self) -> &'static str {
        match self {
            Self::Blur => "blur.gaussian",
            Self::Glow => "stylize.outerGlow",
            Self::DropShadow => "stylize.dropShadow",
            Self::ColorAdjust => "adjust.brightnessContrast",
            Self::Tint => "adjust.shiftToColor",
        }
    }
}

/// Recursively interpolates two JSON values with parameter `t` in [0, 1].
pub fn interpolate_value(a: &Value, b: &Value, t: f64) -> Value {
    match (a, b) {
        (Value::Number(na), Value::Number(nb)) => {
            if let (Some(va), Some(vb)) = (na.as_f64(), nb.as_f64()) {
                json!(va + (vb - va) * t)
            } else {
                b.clone()
            }
        }
        (Value::String(sa), Value::String(sb)) => {
            if let (Some(ca), Some(cb)) = (vectorcraft_color::Color::from_hex(sa), vectorcraft_color::Color::from_hex(sb)) {
                let [ra, ga, ba] = ca.to_rgb();
                let [rb, gb, bb] = cb.to_rgb();
                let mix = |va: f32, vb: f32| va + (vb - va) * t as f32;
                let r = mix(ra, rb);
                let g = mix(ga, gb);
                let b = mix(ba, bb);
                json!(vectorcraft_color::Color::rgb(r, g, b).to_hex())
            } else if t >= 1.0 {
                b.clone()
            } else {
                a.clone()
            }
        }
        (Value::Object(oa), Value::Object(ob)) => {
            let mut out = serde_json::Map::new();
            for (k, va) in oa {
                if let Some(vb) = ob.get(k) {
                    out.insert(k.clone(), interpolate_value(va, vb, t));
                } else {
                    out.insert(k.clone(), va.clone());
                }
            }
            for (k, vb) in ob {
                if !oa.contains_key(k) {
                    out.insert(k.clone(), vb.clone());
                }
            }
            Value::Object(out)
        }
        _ => {
            if t >= 1.0 {
                b.clone()
            } else {
                a.clone()
            }
        }
    }
}

/// An effect instance on a row, with parameter keyframes.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct RowEffect {
    pub kind: FxKind,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub keys: BTreeMap<u32, Value>,
    /// Which part of each drawing the effect takes (drawing selector; default all of the row).
    #[serde(default, skip_serializing_if = "is_all")]
    pub part: Part,
}

fn is_all(p: &Part) -> bool {
    *p == Part::All
}

impl RowEffect {
    pub fn new(kind: FxKind) -> Self {
        Self { kind, keys: BTreeMap::new(), part: Part::All }
    }

    /// Evaluates parameters at `frame`: straight interpolation between keys, held before and after.
    pub fn params_at(&self, frame: u32) -> Value {
        if self.keys.is_empty() {
            return self.kind.default_params();
        }
        let before = self.keys.range(..=frame).next_back();
        let after = self.keys.range(frame..).next();
        match (before, after) {
            (Some((fa, a)), Some((fb, b))) if fb > fa => {
                let t = (frame - fa) as f64 / (fb - fa) as f64;
                interpolate_value(a, b, t)
            }
            (Some((_, k)), _) | (None, Some((_, k))) => k.clone(),
            (None, None) => self.kind.default_params(),
        }
    }

    /// Converts this row effect at `frame` into a VectorCraft live effect.
    pub fn to_live_effect(&self, frame: u32) -> Effect {
        let mut params = self.params_at(frame);
        // Normalize radius / blur parameter for the underlying effect
        match self.kind {
            FxKind::Blur => {
                if let Some(b) = params.get("blur").filter(|_| params.get("radius").is_none()) {
                    params["radius"] = b.clone();
                }
            }
            FxKind::Glow => {
                if let Some(r) = params.get("radius").filter(|_| params.get("blur").is_none()) {
                    params["blur"] = r.clone();
                }
            }
            FxKind::DropShadow => {
                if let Some(r) = params.get("radius").filter(|_| params.get("blur").is_none()) {
                    params["blur"] = r.clone();
                }
            }
            FxKind::ColorAdjust => {}
            FxKind::Tint => {}
        }
        let is_zero = match self.kind {
            FxKind::Blur => params.get("radius").and_then(Value::as_f64).is_some_and(|r| r <= 0.0),
            FxKind::Glow => {
                params.get("blur").and_then(Value::as_f64).is_some_and(|b| b <= 0.0)
                    || params.get("opacity").and_then(Value::as_f64).is_some_and(|o| o <= 0.0)
            }
            FxKind::DropShadow => params.get("opacity").and_then(Value::as_f64).is_some_and(|o| o <= 0.0),
            FxKind::ColorAdjust => false,
            FxKind::Tint => params.get("amount").and_then(Value::as_f64).is_some_and(|a| a <= 0.0),
        };
        if let Some(o) = params.as_object_mut() {
            o.insert(MARK.into(), json!(true));
        }
        Effect { id: self.kind.live_effect_id().to_string(), params, visible: !is_zero }
    }
}

/// The effects stacked on one timeline row.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct RowFx {
    pub row: u64,
    #[serde(default)]
    pub effects: Vec<RowEffect>,
}

/// The document's row effect configuration.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RowFxList {
    pub rows: Vec<RowFx>,
}

impl RowFxList {
    pub fn of(doc: &Document) -> Self {
        doc.unknown.get(KEY).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default()
    }

    pub fn store(&self, doc: &mut Document) {
        if self.rows.is_empty() {
            doc.unknown.remove(KEY);
        } else if let Ok(v) = serde_json::to_value(self) {
            doc.unknown.insert(KEY.into(), v);
        }
    }
}

/// Applies every row's effects for `frame` onto the document layers.
/// Returns a map of `node id -> count of effects added` (the row, or the parts a selector picks) so `unposed` can revert them.
pub(crate) fn apply(d: &mut Document, frame: u32) -> BTreeMap<u64, usize> {
    let all = RowFxList::of(d);
    let mut applied = BTreeMap::new();
    for r in all.rows {
        let row_id = NodeId(r.row);
        for fx in &r.effects {
            let live = fx.to_live_effect(frame);
            for id in super::selector::targets(d, row_id, fx.part) {
                if let Some(n) = d.node_mut(id) {
                    n.appearance.effects.push(live.clone());
                    *applied.entry(id.0).or_insert(0) += 1;
                }
            }
        }
    }
    applied
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolation_holds_before_and_after_and_interpolates_midway() {
        let mut fx = RowEffect::new(FxKind::Glow);
        fx.keys.insert(1, json!({"radius": 0.0}));
        fx.keys.insert(12, json!({"radius": 10.0}));

        let p1 = fx.params_at(1);
        assert_eq!(p1["radius"], json!(0.0));

        let p0 = fx.params_at(0);
        assert_eq!(p0["radius"], json!(0.0), "held before first key");

        let p6 = fx.params_at(6);
        let r6 = p6["radius"].as_f64().unwrap();
        // At frame 6: (6 - 1) / (12 - 1) * 10 = 50 / 11 ≈ 4.545
        assert!((r6 - 4.545).abs() < 0.01, "expected ~4.55, got {r6}");

        let p12 = fx.params_at(12);
        assert_eq!(p12["radius"], json!(10.0));

        let p20 = fx.params_at(20);
        assert_eq!(p20["radius"], json!(10.0), "held after last key");
    }

    #[test]
    fn to_live_effect_maps_radius_to_blur_for_glow() {
        let mut fx = RowEffect::new(FxKind::Glow);
        fx.keys.insert(1, json!({"radius": 8.0, "opacity": 80.0}));
        let live = fx.to_live_effect(1);
        assert_eq!(live.id, "stylize.outerGlow");
        assert_eq!(live.params["blur"], json!(8.0));
        assert_eq!(live.params["opacity"], json!(80.0));
    }

    #[test]
    fn tint_to_live_effect_and_zero_visibility() {
        let mut fx = RowEffect::new(FxKind::Tint);
        fx.keys.insert(1, json!({"amount": 0.0, "color": "#00ff00"}));
        fx.keys.insert(10, json!({"amount": 50.0, "color": "#ff0000"}));

        let live1 = fx.to_live_effect(1);
        assert_eq!(live1.id, "adjust.shiftToColor");
        assert!(!live1.visible, "amount 0 should have visible: false");

        let live10 = fx.to_live_effect(10);
        assert_eq!(live10.id, "adjust.shiftToColor");
        assert!(live10.visible, "amount 50 should have visible: true");
        assert_eq!(live10.params["amount"], json!(50.0));
    }
}
