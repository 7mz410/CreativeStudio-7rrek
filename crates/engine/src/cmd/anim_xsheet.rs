//! 7rrek (Creative Studio): the Xsheet (exposure sheet), as in Harmony: a column per timeline row,
//! a cell per frame. A cell where a drawing's exposure starts (a key exposure: a keyframe or an
//! exposure) names the drawing; the cells after it hold it. Typing into a cell is `anim.expose`
//! with a `name`; `anim.exposure` makes a hold longer or shorter, moving everything after it.

use std::collections::BTreeMap;

use serde_json::json;

use super::anim::{Anim, frame_param, row_param, show};
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "anim.xsheet",
            "Xsheet",
            [],
            None,
            "{} the exposure sheet → {frames, current, columns: [{row: layer id, name, keys: [{frame, drawing: id, name}] (where each exposure starts; the frames after hold it)}]} for the rows that have drawings, bottom layer first",
            has_doc,
            xsheet
        ),
        cmd!(
            "anim.exposure",
            "Change Exposure",
            [],
            None,
            "{row?: layer id (default: the current layer's row), frame?: (default: the current frame), by: whole number, not 0} makes the drawing held at the frame last `by` frames longer (or shorter, below 0), moving the row's later keys and exposures with it; the timeline grows to fit → {moved: how many keys moved}",
            has_doc,
            exposure
        ),
    ]
}

fn xsheet(s: &mut Session, _: &Value) -> Result<Value> {
    let d = &s.doc()?.doc;
    let anim = Anim::of(d);
    let name = |id: NodeId| d.node(id).and_then(|n| n.name.clone()).unwrap_or_default();
    let columns: Vec<Value> = d
        .layers
        .iter()
        .filter_map(|row| {
            let keys = anim.row_frames(row);
            (!keys.is_empty()).then(|| {
                let keys: Vec<Value> = keys.iter().map(|(f, id)| json!({"frame": f, "drawing": id.0, "name": name(*id)})).collect();
                json!({"row": row.id.0, "name": row.name.clone().unwrap_or_default(), "keys": keys})
            })
        })
        .collect();
    Ok(json!({"frames": anim.frames, "current": anim.current, "columns": columns}))
}

/// `frame` moved by `by`, None below frame 1 or past `u32`.
fn moved(frame: u32, by: i64) -> Option<u32> {
    u32::try_from(i64::from(frame) + by).ok().filter(|f| *f >= 1)
}

fn exposure(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "anim.exposure";
    let row = row_param(s, p, C)?;
    let d = &s.doc()?.doc;
    let anim = Anim::of(d);
    let frame = frame_param(p, &anim, C)?;
    let by = p
        .get("by")
        .and_then(Value::as_i64)
        .filter(|b| *b != 0 && b.unsigned_abs() <= 10_000)
        .ok_or_else(|| bad(C, "`by` must be a whole number from -10000 to 10000, not 0"))?;
    let row_node = d.node(row).ok_or_else(|| bad(C, "no such layer"))?;
    let keys: Vec<(u32, NodeId)> = anim.row_keys(row_node).into_iter().filter(|(f, _)| *f > frame).collect();
    let exposed: Vec<u32> = anim.exposures.get(&row.0).into_iter().flatten().map(|(f, _)| *f).filter(|f| *f > frame).collect();
    let Some(first) = keys.iter().map(|(f, _)| *f).chain(exposed.iter().copied()).min() else {
        return Err(bad(C, format!("nothing after frame {frame} on this layer to move")));
    };
    // Shortening can't pass the frame itself: the next exposure would land on or before it.
    if moved(first, by).is_none_or(|f| f <= frame) {
        return Err(bad(C, format!("the exposure at frame {frame} can't be that much shorter")));
    }
    let last = keys.iter().map(|(f, _)| *f).chain(exposed.iter().copied()).max().unwrap_or(first);
    let end = moved(last, by).ok_or_else(|| bad(C, "the exposure would move past the end"))?;
    if end > 10_000 {
        return Err(bad(C, "the timeline can't grow past 10000 frames"));
    }
    let count = keys.len() + exposed.len();
    s.edit("Change Exposure", |d, _| {
        let mut a = Anim::of(d);
        for (f, id) in &keys {
            if let (Some(slot), Some(nf)) = (a.keys.get_mut(&id.0), moved(*f, by)) {
                *slot = nf;
            }
        }
        if let Some(e) = a.exposures.get_mut(&row.0) {
            let old = std::mem::take(e);
            *e = old.into_iter().map(|(f, id)| (if f > frame { moved(f, by).unwrap_or(f) } else { f }, id)).collect::<BTreeMap<_, _>>();
        }
        a.frames = a.frames.max(end);
        a.store(d);
        Ok(())
    })?;
    let current = Anim::of(&s.doc()?.doc).current;
    show(s, current)?;
    Ok(json!({"moved": count}))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drawings named 1, 2, 3 at frames 1, 3, 5 on one row.
    fn sheet() -> (Session, Vec<u64>) {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let ids = (1..=3u32)
            .map(|i| {
                let id = s.execute("anim.blankKeyframe", &json!({"frame": 2 * i - 1})).unwrap()["id"].as_u64().unwrap();
                s.execute("layer.setProps", &json!({"ids": [id], "name": i.to_string()})).unwrap();
                id
            })
            .collect();
        (s, ids)
    }

    fn keys(s: &mut Session) -> Vec<(u64, u64)> {
        let x = s.execute("anim.xsheet", &json!({})).unwrap();
        x["columns"][0]["keys"].as_array().unwrap().iter().map(|k| (k["frame"].as_u64().unwrap(), k["drawing"].as_u64().unwrap())).collect()
    }

    #[test]
    fn typing_a_name_or_number_exposes_that_drawing() {
        let (mut s, ids) = sheet();
        assert_eq!(keys(&mut s), vec![(1, ids[0]), (3, ids[1]), (5, ids[2])]);
        // Typed `2` into frame 6: drawing 2 shows from frame 6.
        s.execute("anim.expose", &json!({"frame": 6, "name": "2"})).unwrap();
        assert_eq!(keys(&mut s).last(), Some(&(6, ids[1])));
        s.execute("layer.setProps", &json!({"ids": [ids[0]], "name": "blink"})).unwrap();
        s.execute("anim.expose", &json!({"frame": 7, "name": "blink"})).unwrap();
        assert_eq!(keys(&mut s).last(), Some(&(7, ids[0])));
        assert!(s.execute("anim.expose", &json!({"frame": 8, "name": "9"})).is_err());
        assert!(s.execute("anim.expose", &json!({"frame": 8, "name": 5})).is_err());
    }

    #[test]
    fn exposures_grow_and_shrink_moving_what_follows() {
        let (mut s, ids) = sheet();
        s.execute("anim.exposure", &json!({"frame": 1, "by": 2})).unwrap();
        assert_eq!(keys(&mut s), vec![(1, ids[0]), (5, ids[1]), (7, ids[2])]);
        s.execute("anim.exposure", &json!({"frame": 5, "by": -1})).unwrap();
        assert_eq!(keys(&mut s), vec![(1, ids[0]), (5, ids[1]), (6, ids[2])]);
        assert!(s.execute("anim.exposure", &json!({"frame": 5, "by": -1})).is_err(), "frame 6 can't land on 5");
        assert!(s.execute("anim.exposure", &json!({"frame": 6, "by": 1})).is_err(), "nothing after the last");
        assert!(s.execute("anim.exposure", &json!({"frame": 1, "by": 0})).is_err());
        s.execute("anim.exposure", &json!({"frame": 1, "by": 30})).unwrap();
        let frames = s.execute("anim.xsheet", &json!({})).unwrap()["frames"].as_u64().unwrap();
        assert!(frames >= 36, "the timeline grows: {frames}");
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(keys(&mut s), vec![(1, ids[0]), (5, ids[1]), (6, ids[2])], "one undo step");
    }
}
