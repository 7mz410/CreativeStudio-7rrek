//! 7rrek (Creative Studio): drawing substitution, as in Harmony's exposure sheet. A row's drawings
//! are its keyframe sublayers; exposing one at a frame shows it from that frame on, so one drawing
//! can come back on many frames (`Anim.exposures`, see anim.rs).

use serde_json::json;

use super::anim::{Anim, frame_param, row_param, show};
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "anim.expose",
            "Expose Drawing",
            [],
            None,
            "{row?: layer id (default: the current layer's row), frame?: (default: the current frame), drawing?: id of one of the row's drawings (its keyframe sublayers), name?: a drawing's name, or its number in the row (1 is the first), as typed in the Xsheet; neither clears the exposure at the frame} shows the drawing from the frame on → {frame, drawing}",
            has_doc,
            expose
        ),
        cmd!(
            query "anim.drawings",
            "Drawings",
            [],
            None,
            "{row?: layer id (default: the current layer's row)} the row's drawings → {row, drawings: [{id, name, frames: [frames it shows from]}], current: id shown at the current frame or null}",
            has_doc,
            drawings
        ),
    ]
}

fn expose(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "anim.expose";
    let row = row_param(s, p, C)?;
    let d = &s.doc()?.doc;
    let anim = Anim::of(d);
    let frame = frame_param(p, &anim, C)?;
    let drawing = match p.get("drawing") {
        None | Some(Value::Null) if p.get("name").is_some_and(|n| !n.is_null()) => {
            let typed = p.get("name").and_then(Value::as_str).map(str::trim).ok_or_else(|| bad(C, "`name` must be text"))?;
            let row_node = d.node(row).ok_or_else(|| bad(C, "no such layer"))?;
            let keys = anim.row_keys(row_node);
            let named = keys.iter().find(|(_, k)| d.node(*k).and_then(|n| n.name.as_deref()).is_some_and(|n| n.trim() == typed));
            let numbered = || typed.parse::<usize>().ok().and_then(|i| i.checked_sub(1)).and_then(|i| keys.get(i));
            let (_, k) = named.or_else(numbered).ok_or_else(|| bad(C, format!("this layer has no drawing named or numbered `{typed}`")))?;
            Some(k.0)
        }
        None | Some(Value::Null) => None,
        Some(v) => {
            let id = v.as_u64().ok_or_else(|| bad(C, "`drawing` must be a layer id"))?;
            let row_node = d.node(row).ok_or_else(|| bad(C, "no such layer"))?;
            if !anim.row_keys(row_node).iter().any(|(_, k)| k.0 == id) {
                return Err(bad(C, "`drawing` must be one of the row's drawings (its keyframe sublayers)"));
            }
            Some(id)
        }
    };
    if drawing.is_none() && !anim.exposures.get(&row.0).is_some_and(|e| e.contains_key(&frame)) {
        return Err(bad(C, format!("no exposure at frame {frame} on this layer")));
    }
    s.edit("Expose Drawing", |d, _| {
        let mut a = Anim::of(d);
        let e = a.exposures.entry(row.0).or_default();
        match drawing {
            Some(id) => e.insert(frame, id),
            None => e.remove(&frame),
        };
        a.exposures.retain(|_, e| !e.is_empty());
        a.store(d);
        Ok(())
    })?;
    let current = Anim::of(&s.doc()?.doc).current;
    show(s, current)?;
    Ok(json!({"frame": frame, "drawing": drawing}))
}

fn drawings(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "anim.drawings";
    let row = row_param(s, p, C)?;
    let d = &s.doc()?.doc;
    let anim = Anim::of(d);
    let row_node = d.node(row).ok_or_else(|| bad(C, "no such layer"))?;
    let frames = anim.row_frames(row_node);
    let list: Vec<Value> = anim
        .row_keys(row_node)
        .into_iter()
        .map(|(_, id)| {
            let name = d.node(id).map(|n| n.name.clone()).unwrap_or_default();
            let at: Vec<u32> = frames.iter().filter(|(_, k)| *k == id).map(|(f, _)| *f).collect();
            json!({"id": id.0, "name": name, "frames": at})
        })
        .collect();
    let current = frames.iter().rfind(|(f, _)| *f <= anim.current).map(|(_, id)| id.0);
    Ok(json!({"row": row.0, "drawings": list, "current": current}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::anim::at_frame;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s
    }

    /// Three blank drawings at frames 1–3, each with a square at a different spot.
    fn three(s: &mut Session) -> Vec<u64> {
        (1..=3u32)
            .map(|f| {
                let id = s.execute("anim.blankKeyframe", &json!({"frame": f})).unwrap()["id"].as_u64().unwrap();
                s.execute("shape.rectangle", &json!({"x": 50.0 * f as f64, "y": 10, "width": 20, "height": 20})).unwrap();
                id
            })
            .collect()
    }

    /// The left edge of the one drawing visible at `frame` (frames differ when this does).
    fn shown_x(s: &Session, frame: u32) -> f64 {
        let d = at_frame(&s.doc().unwrap().doc, frame);
        let row = d.layers[0].children().unwrap();
        let shown: Vec<_> = row.iter().filter(|c| c.is_layer() && c.visible).collect();
        assert_eq!(shown.len(), 1, "frame {frame}");
        d.bounds_of(&[shown[0].id], false).unwrap().min_x()
    }

    #[test]
    fn exposures_swap_drawings_per_frame() {
        let mut s = session();
        s.execute("anim.setFrames", &json!({"frames": 6})).unwrap();
        let k = three(&mut s);
        // Frames 4–6 run the drawings backwards: 3, 2, 1.
        for (f, id) in [(4, k[2]), (5, k[1]), (6, k[0])] {
            s.execute("anim.expose", &json!({"frame": f, "drawing": id})).unwrap();
        }
        let xs: Vec<f64> = (4..=6).map(|f| shown_x(&s, f)).collect();
        assert_eq!(xs, vec![150.0, 100.0, 50.0]);
        let list = s.execute("anim.drawings", &json!({})).unwrap();
        assert_eq!(list["drawings"].as_array().unwrap().len(), 3);
        assert_eq!(list["drawings"][0]["frames"], json!([1, 6]));
        // Clearing frame 6 holds frame 5's drawing again.
        s.execute("anim.expose", &json!({"frame": 6})).unwrap();
        assert_eq!(shown_x(&s, 6), 100.0);
        assert!(s.execute("anim.expose", &json!({"frame": 6})).is_err());
        assert!(s.execute("anim.expose", &json!({"frame": 2, "drawing": 999_999})).is_err());
    }

    #[test]
    fn exposures_survive_save_and_load() {
        let mut s = session();
        let k = three(&mut s);
        s.execute("anim.expose", &json!({"frame": 4, "drawing": k[0]})).unwrap();
        let doc = &s.doc().unwrap().doc;
        let bytes = vectorcraft_format::save(doc, false);
        let back = vectorcraft_format::load(&bytes).unwrap();
        assert_eq!(Anim::of(&back).exposures, Anim::of(doc).exposures);
        assert!(!Anim::of(&back).exposures.is_empty());
    }
}
