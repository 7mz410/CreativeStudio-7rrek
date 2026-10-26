//! 7rrek (Creative Studio): multiplane, as in Harmony. Each timeline row sits at a depth: its own
//! (`Anim.depth`) plus what the pegs carrying it add (`PegKey.z`). The camera looks at the stage
//! from `LENS` in front of it and can move in (`Cam.z`); a row `d` from the lens looks `LENS / d`
//! its size and moves that much of a camera pan (parallax). A row set to keep its size
//! (`Anim.maintain`) is drawn bigger by as much as its depth shrinks it, so only its motion
//! changes. Rows nearer than `NEAR` or past `FAR` are not seen, and farther rows are drawn first.
//! Depth only changes what the camera sees (exports); the canvas stays flat for drawing.
//!
//! Units: document points, 36 to Harmony's field (half an inch); positive depth is away from the
//! camera.

use std::sync::Arc;

use vectorcraft_doc::Document;
use vectorcraft_geom::{Affine, Point, Rect};

use super::anim::{Anim, Cam};
use super::peg::Pegs;

/// Points in one field.
pub const FIELD: f64 = 36.0;
/// How far the lens sits in front of the stage (depth 0): 12 fields.
pub const LENS: f64 = 12.0 * FIELD;
/// Nearer than this to the lens, a row is not seen (1 field).
pub const NEAR: f64 = FIELD;
/// Past this from the lens, a row is not seen (1000 fields).
pub const FAR: f64 = 1000.0 * FIELD;

/// Each row's depth in `doc` at its current frame: its own plus its pegs'.
pub fn row_depths(doc: &Document) -> Vec<(u64, f64)> {
    let anim = Anim::of(doc);
    let pegs = Pegs::of(doc);
    doc.layers.iter().map(|l| (l.id.0, anim.depth.get(&l.id.0).copied().unwrap_or(0.0) + pegs.row_z(l.id.0, anim.current))).collect()
}

/// How big a row at `depth` looks through camera `c` (1 at the stage with the camera at rest),
/// None when the row is out of view (too near or too far).
pub fn apparent_scale(depth: f64, c: &Cam) -> Option<f64> {
    let d = LENS + depth - c.z;
    (d.is_finite() && (NEAR..=FAR).contains(&d)).then(|| LENS / d)
}

/// `doc` seen through camera `c`, framed to `rect`: each row placed by its depth (see the module
/// doc), farther rows first, rows out of view hidden.
pub(crate) fn project(doc: &mut Document, c: &Cam, rect: Rect) {
    let anim = Anim::of(doc);
    let depths = row_depths(doc);
    let flat = depths.iter().all(|(_, z)| *z == 0.0) && c.z == 0.0;
    let view = |s: f64| Affine::translate(rect.center().to_vec2()) * Affine::rotate(-c.rotation.to_radians()) * Affine::scale(c.zoom * s);
    let pan = Affine::translate((-c.x, -c.y));
    for l in &mut doc.layers {
        let depth = depths.iter().find(|(id, _)| *id == l.id.0).map_or(0.0, |(_, z)| *z);
        let Some(s) = apparent_scale(depth, c) else {
            Arc::make_mut(l).visible = false;
            continue;
        };
        // A row that keeps its size is seen at its own size about the stage's centre (the default
        // pivot), which still moves `s` of the pan: screen = centre + zoom·R·(p − a + s·(a − cam)).
        let m = if anim.maintain.contains(&l.id.0) {
            let a = rect.center();
            let shift = (a - Point::new(c.x, c.y)) * s - a.to_vec2();
            view(1.0) * Affine::translate(shift)
        } else {
            view(s) * pan
        };
        Arc::make_mut(l).transform(m, true);
    }
    if !flat {
        // Farther first; equal depths keep their layer order (the sort is stable).
        let at = |id: u64| depths.iter().find(|(i, _)| *i == id).map_or(0.0, |(_, z)| *z);
        doc.layers.sort_by(|a, b| at(b.id.0).total_cmp(&at(a.id.0)));
    }
}

use serde_json::json;

use super::anim::{row_param, show};
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "anim.depth",
            "Layer Depth",
            [],
            None,
            "{row?: layer id (default: the current layer's row), z?: depth behind the stage (document points; 36 is one field; negative is in front), maintain?: bool (keep its size as it moves back: only its motion shows the depth)} sets a row's multiplane depth; fields left out keep their value → {row, z, maintain, scale: how big it looks with the camera at rest}",
            has_doc,
            set_depth
        ),
        cmd!(
            query "anim.depths",
            "Layer Depths",
            [],
            None,
            "{} each row's multiplane depth at the current frame → {lens: the camera's distance to the stage, rows: [{row, own: its own depth, z: with its pegs', maintain}]}",
            has_doc,
            depths
        ),
    ]
}

fn set_depth(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "anim.depth";
    let row = row_param(s, p, C)?;
    let a = Anim::of(&s.doc()?.doc);
    let z = number_or(p, "z", a.depth.get(&row.0).copied().unwrap_or(0.0), C)?;
    if !z.is_finite() || LENS + z < NEAR || LENS + z > FAR {
        return Err(bad(C, format!("z must keep the row in the camera's view: from {} to {}", NEAR - LENS, FAR - LENS)));
    }
    let maintain = p.get("maintain").and_then(Value::as_bool).unwrap_or(a.maintain.contains(&row.0));
    s.edit("Layer Depth", |d, _| {
        let mut a = Anim::of(d);
        if z == 0.0 {
            a.depth.remove(&row.0);
        } else {
            a.depth.insert(row.0, z);
        }
        if maintain {
            a.maintain.insert(row.0);
        } else {
            a.maintain.remove(&row.0);
        }
        a.store(d);
        Ok(())
    })?;
    let current = Anim::of(&s.doc()?.doc).current;
    show(s, current)?;
    let rest = Cam { x: 0.0, y: 0.0, zoom: 1.0, rotation: 0.0, ease: None, z: 0.0 };
    Ok(json!({"row": row.0, "z": z, "maintain": maintain, "scale": apparent_scale(z, &rest)}))
}

fn depths(s: &mut Session, _: &Value) -> Result<Value> {
    let d = &s.doc()?.doc;
    let a = Anim::of(d);
    let rows: Vec<Value> = row_depths(d)
        .into_iter()
        .map(|(row, z)| json!({"row": row, "own": a.depth.get(&row).copied().unwrap_or(0.0), "z": z, "maintain": a.maintain.contains(&row)}))
        .collect();
    Ok(json!({"lens": LENS, "rows": rows}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam(x: f64, z: f64) -> Cam {
        Cam { x, y: 50.0, zoom: 1.0, rotation: 0.0, ease: None, z }
    }

    /// Two rows with a 20 pt square each at x 40–60: the first at the stage, the second `depth`
    /// behind it. Their ids.
    fn scene(depth: f64, maintain: bool) -> (Session, u64, u64) {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 40, "y": 40, "width": 20, "height": 20})).unwrap();
        let near = s.doc().unwrap().doc.layers[0].id.0;
        let far = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
        s.execute("layer.setCurrent", &json!({"id": far})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 40, "y": 40, "width": 20, "height": 20})).unwrap();
        s.execute("anim.depth", &json!({"row": far, "z": depth, "maintain": maintain})).unwrap();
        (s, near, far)
    }

    /// Row `row`'s bounds as camera `c` sees it (framed to the 100 pt artboard).
    fn seen(s: &Session, row: u64, c: Cam) -> Rect {
        let d =
            super::super::anim::through_camera(super::super::anim::at_frame(&s.doc().unwrap().doc, 1), Some(c), Rect::new(0.0, 0.0, 100.0, 100.0));
        d.bounds_of(&[NodeId(row)], false).unwrap()
    }

    #[test]
    fn depth_shrinks_and_out_of_view_is_none() {
        let rest = cam(0.0, 0.0);
        assert_eq!(apparent_scale(0.0, &rest), Some(1.0));
        assert_eq!(apparent_scale(LENS, &rest), Some(0.5), "12 fields back: half size");
        assert!(apparent_scale(-LENS + 1.0, &rest).is_none(), "nearer than the near plane");
        assert!(apparent_scale(FAR, &rest).is_none(), "past the far plane");
        // The camera 6 fields in: the stage looks twice its size, 12 fields back looks 12/18.
        let truck = cam(0.0, 6.0 * FIELD);
        assert_eq!(apparent_scale(0.0, &truck), Some(2.0));
        assert!((apparent_scale(LENS, &truck).unwrap() - 12.0 / 18.0).abs() < 1e-12);
    }

    #[test]
    fn a_pan_moves_a_far_row_less() {
        let (s, near, far) = scene(LENS, false);
        let (a, b) = (seen(&s, near, cam(50.0, 0.0)), seen(&s, near, cam(150.0, 0.0)));
        assert!((a.min_x() - b.min_x() - 100.0).abs() < 1e-6, "the stage moves the whole pan");
        let (a, b) = (seen(&s, far, cam(50.0, 0.0)), seen(&s, far, cam(150.0, 0.0)));
        assert!((a.min_x() - b.min_x() - 50.0).abs() < 1e-6, "12 fields back moves half: {} {}", a.min_x(), b.min_x());
        assert!((a.width() - 10.0).abs() < 1e-6, "and looks half its size");
    }

    #[test]
    fn a_row_that_keeps_its_size_still_moves_less() {
        let (s, _, far) = scene(LENS, true);
        let (a, b) = (seen(&s, far, cam(50.0, 0.0)), seen(&s, far, cam(150.0, 0.0)));
        assert!((a.width() - 20.0).abs() < 1e-6, "its own size: {}", a.width());
        assert!((a.min_x() - b.min_x() - 50.0).abs() < 1e-6, "half the pan");
    }

    #[test]
    fn pegs_add_depth_and_farther_rows_draw_first() {
        let (mut s, near, far) = scene(0.0, false);
        let peg = s.execute("peg.add", &json!({"rows": [near]})).unwrap()["index"].clone();
        s.execute("peg.key", &json!({"peg": peg, "frame": 1, "z": 2.0 * FIELD})).unwrap();
        let rows = s.execute("anim.depths", &json!({})).unwrap()["rows"].clone();
        assert_eq!(rows[0]["z"], json!(2.0 * FIELD), "the peg's depth counts");
        let d = super::super::anim::through_camera(
            super::super::anim::at_frame(&s.doc().unwrap().doc, 1),
            Some(cam(50.0, 0.0)),
            Rect::new(0.0, 0.0, 100.0, 100.0),
        );
        assert_eq!(d.layers.first().map(|l| l.id.0), Some(near), "the farther row is drawn first");
        assert_eq!(d.layers.last().map(|l| l.id.0), Some(far));
        assert!(s.execute("anim.depth", &json!({"z": -LENS})).is_err(), "at the lens");
        let back = vectorcraft_format::load(&vectorcraft_format::save(&s.doc().unwrap().doc, false)).unwrap();
        assert_eq!(row_depths(&back), row_depths(&s.doc().unwrap().doc));
    }
}
