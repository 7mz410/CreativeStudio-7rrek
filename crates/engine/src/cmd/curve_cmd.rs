//! 7rrek: curve and envelope deformer commands (`curve.*`) over the model in [`super::curve`].

use serde_json::{Value, json};
use vectorcraft_doc::{Document, NodeId};

use super::anim::{Anim, frame_param, show};
use super::curve::{Curve, Curves, MAX_CURVES, MAX_SEGMENTS};
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "curve.add",
            "Add Curve Deformer",
            [],
            None,
            "{node?: id (default: the one selected object; error if none or several), closed?: true or false (default false), points?: [[x, y]…] the anchors (default: an open curve of 3 anchors along the object's middle line, left to right; a closed one of 4 at its corners, grown 4 pt), radius?: number or null (art farther than this stays at rest; default none), fade?: number (default 0)} adds a curve deformer with straight handles and selects the object → {node, anchors, segments}",
            has_doc,
            add
        ),
        cmd!(
            "curve.key",
            "Curve Key",
            [],
            None,
            "{node?: id (default: the selected object, else the only curve deformer), frame?: (default: the current frame), anchor: anchor index, handle?: \"in\" or \"out\" (moves that handle instead of the anchor), x?, y?: move (document points), dx?, dy?: added to the move} sets the anchor's position at the frame (its handles move with it), or the handle's → {frame, x, y}",
            has_doc,
            key
        ),
        cmd!(
            "curve.removeKey",
            "Remove Curve Key",
            [],
            None,
            "{node?: id (default: the selected object), frame?: (default: the current frame)} removes the key at the frame; error if there is none → {frame}",
            has_doc,
            remove_key
        ),
        cmd!(
            "curve.remove",
            "Remove Curve Deformer",
            [],
            None,
            "{node?: id (default: the selected object)} removes the object's curve deformer → {node}",
            has_doc,
            remove
        ),
        cmd!(
            query "curve.info",
            "Curve Deformer",
            [],
            None,
            "{node?: id (default: the selected object)} → {node, closed, anchors, rest: [[x, y]…] at rest, now: [[x, y]…] at the current frame, keys: [frames], radius, fade}",
            has_doc,
            info
        ),
        cmd!(
            "curve.set",
            "Set Curve Deformer",
            [],
            None,
            "{node?: id (default: the selected object), radius?: number or null (null: no limit), fade?: number ≥ 0} sets the influence radius and fade width; keys not given stay as they are → {node, radius, fade}",
            has_doc,
            set
        ),
    ]
}

/// Store `curves` as one undo step, then show the current frame. A set that fails `valid` is refused.
fn commit(s: &mut Session, label: &str, curves: Curves, cmd: &str) -> Result<()> {
    if !curves.valid() {
        return Err(bad(cmd, "the edit would leave an invalid curve"));
    }
    s.edit(label, |d, _| {
        curves.store(d);
        Ok(())
    })?;
    let current = Anim::of(&s.doc()?.doc).current;
    show(s, current)?;
    Ok(())
}

/// The curve deformers of the open document.
fn curves_of(s: &Session) -> Result<Curves> {
    Ok(Curves::of(&s.doc()?.doc))
}

/// The object `p` names under `node`, or else the one selected object, or else (for commands on
/// an existing curve) the document's only curve deformer: showing a bent frame takes the original
/// art out of the selection, as its posed copy stands in for it.
fn node_of(s: &Session, p: &Value, cmd: &str) -> Result<NodeId> {
    if let Some(v) = p.get("node").filter(|v| !v.is_null()) {
        return checked_id(s, v, cmd);
    }
    let only = || match Curves::of(&s.doc().ok()?.doc).curves.as_slice() {
        [c] if cmd != "curve.add" => Some(NodeId(c.node)),
        _ => None,
    };
    match s.doc()?.selection.objects.as_slice() {
        [one] => Ok(*one),
        [] => only().ok_or_else(|| bad(cmd, "nothing selected; select one object or give `node`")),
        _ => Err(bad(cmd, "several objects selected; give `node`")),
    }
}

/// The position in `curves` of the curve deformer over `node`.
fn curve_at(curves: &Curves, node: NodeId, cmd: &str) -> Result<usize> {
    curves.curves.iter().position(|c| c.node == node.0).ok_or_else(|| bad(cmd, "no curve deformer on this object; use curve.add"))
}

/// Anchors in a chain: every third point, plus the last one for an open chain.
fn anchors(c: &Curve) -> usize {
    if c.closed { c.rest.len() / 3 } else { c.rest.len().saturating_sub(1) / 3 + 1 }
}

/// An anchor index `p` gives for `anchor`, below `n` anchors.
fn anchor_param(p: &Value, n: usize, cmd: &str) -> Result<usize> {
    p.get("anchor")
        .and_then(Value::as_u64)
        .map(index)
        .filter(|i| *i < n)
        .ok_or_else(|| bad(cmd, format!("`anchor` must be an anchor index from 0 to {}", n.saturating_sub(1))))
}

/// The radius `p` gives: a number of 0 or more, or null for no limit; None when it gives none.
fn radius_param(p: &Value, cmd: &str) -> Result<Option<f64>> {
    match p.get("radius") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_f64().filter(|r| r.is_finite() && *r >= 0.0).map(Some).ok_or_else(|| bad(cmd, "`radius` must be a number of 0 or more, or null")),
    }
}

/// The fade width `p` gives: a number of 0 or more; 0 when it gives none.
fn fade_param(p: &Value, cmd: &str) -> Result<f64> {
    match p.get("fade") {
        None | Some(Value::Null) => Ok(0.0),
        Some(v) => v.as_f64().filter(|f| f.is_finite() && *f >= 0.0).ok_or_else(|| bad(cmd, "`fade` must be a number of 0 or more")),
    }
}

/// The anchors `p` gives as `points`: 2 to the most a chain takes; None when it gives none.
fn points_param(p: &Value, cmd: &str) -> Result<Option<Vec<[f64; 2]>>> {
    let Some(v) = p.get("points").filter(|v| !v.is_null()) else { return Ok(None) };
    let list = v.as_array().ok_or_else(|| bad(cmd, "`points` must be a list of [x, y] anchors"))?;
    if !(2..=MAX_SEGMENTS + 1).contains(&list.len()) {
        return Err(bad(cmd, format!("`points` takes 2 to {} anchors", MAX_SEGMENTS + 1)));
    }
    let anchor = |q: &Value| match q.as_array().map(Vec::as_slice) {
        Some([x, y]) => match (x.as_f64(), y.as_f64()) {
            (Some(x), Some(y)) if x.is_finite() && y.is_finite() => Ok([x, y]),
            _ => Err(bad(cmd, "each anchor is [x, y] with finite numbers")),
        },
        _ => Err(bad(cmd, "each anchor is [x, y] with finite numbers")),
    };
    list.iter().map(anchor).collect::<Result<Vec<_>>>().map(Some)
}

/// The anchors of a default curve over `node`: 3 along its middle line (open), or 4 at its corners
/// grown 4 pt (closed).
fn default_anchors(doc: &Document, node: NodeId, closed: bool, cmd: &str) -> Result<Vec<[f64; 2]>> {
    let b = doc.bounds_of(&[node], false).ok_or_else(|| bad(cmd, "nothing to measure"))?;
    let (x0, y0, x1, y1) = (b.x0, b.y0, b.x1, b.y1);
    Ok(if closed {
        vec![[x0 - 4.0, y0 - 4.0], [x1 + 4.0, y0 - 4.0], [x1 + 4.0, y1 + 4.0], [x0 - 4.0, y1 + 4.0]]
    } else {
        let mid = (y0 + y1) / 2.0;
        vec![[x0, mid], [(x0 + x1) / 2.0, mid], [x1, mid]]
    })
}

fn lerp2(a: &[f64; 2], b: &[f64; 2], t: f64) -> [f64; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// The rest chain for `anchors`, straight handles a third of the way to the next anchor. None
/// under 2 anchors.
fn chain(anchors: &[[f64; 2]], closed: bool) -> Option<Vec<[f64; 2]>> {
    let n = anchors.len();
    if n < 2 {
        return None;
    }
    let segs = if closed { n } else { n - 1 };
    let mut out = Vec::with_capacity(3 * segs + 1);
    for (i, a) in anchors.iter().take(segs).enumerate() {
        let b = anchors.get((i + 1) % n)?;
        out.extend([*a, lerp2(a, b, 1.0 / 3.0), lerp2(a, b, 2.0 / 3.0)]);
    }
    if !closed {
        out.push(*anchors.last()?);
    }
    Some(out)
}

/// The in and out handle points of the anchor at point `at` in a chain of `len` points; None where
/// an open chain has none.
fn handles(closed: bool, len: usize, at: usize) -> (Option<usize>, Option<usize>) {
    if closed {
        ((at + len).saturating_sub(1).checked_rem(len), (at + 1).checked_rem(len))
    } else {
        (at.checked_sub(1), Some(at + 1).filter(|k| *k < len))
    }
}

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "curve.add";
    let node = node_of(s, p, C)?;
    let closed = switch_or(p, "closed", false, C)?;
    let radius = radius_param(p, C)?;
    let fade = fade_param(p, C)?;
    let mut curves = curves_of(s)?;
    if curves.curves.len() >= MAX_CURVES {
        return Err(bad(C, format!("a document takes at most {MAX_CURVES} curve deformers")));
    }
    if curves.curves.iter().any(|c| c.node == node.0) {
        return Err(bad(C, "this object already has a curve deformer"));
    }
    let anchors = match points_param(p, C)? {
        Some(a) => a,
        None => default_anchors(&s.doc()?.doc, node, closed, C)?,
    };
    let rest = chain(&anchors, closed).ok_or_else(|| bad(C, "the anchors make no curve"))?;
    let c = Curve { node: node.0, rest, closed, radius, fade, keys: Default::default() };
    if !c.valid() {
        return Err(bad(C, "the anchors make no valid curve"));
    }
    let segments = if closed { anchors.len() } else { anchors.len().saturating_sub(1) };
    let out = json!({"node": node.0, "anchors": anchors.len(), "segments": segments});
    curves.curves.push(c);
    commit(s, "Add Curve Deformer", curves, C)?;
    // Selected, so the curve's overlay works on this deformer next.
    s.select(|_, sel| sel.set([node]))?;
    Ok(out)
}

fn key(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "curve.key";
    let node = node_of(s, p, C)?;
    let mut curves = curves_of(s)?;
    let i = curve_at(&curves, node, C)?;
    let frame = frame_param(p, &Anim::of(&s.doc()?.doc), C)?;
    let Some(c) = curves.curves.get_mut(i) else { return Err(bad(C, "no such curve")) };
    let a = anchor_param(p, anchors(c), C)?;
    let at = a * 3;
    let (ins, outs) = handles(c.closed, c.rest.len(), at);
    let target = match str_param(p, "handle") {
        None => at,
        Some("in") => ins.ok_or_else(|| bad(C, "this anchor has no in handle"))?,
        Some("out") => outs.ok_or_else(|| bad(C, "this anchor has no out handle"))?,
        Some(_) => return Err(bad(C, "`handle` must be \"in\" or \"out\"")),
    };
    let mut cur: Vec<[f64; 2]> = match c.keys.get(&frame) {
        Some(k) => k.clone(),
        None => c.at(frame).iter().map(|q| [q.x, q.y]).collect(),
    };
    let [now_x, now_y] = *cur.get(target).ok_or_else(|| bad(C, "no such point"))?;
    let x = number_or(p, "x", now_x, C)? + number_or(p, "dx", 0.0, C)?;
    let y = number_or(p, "y", now_y, C)? + number_or(p, "dy", 0.0, C)?;
    if !(x.is_finite() && y.is_finite()) {
        return Err(bad(C, "the position must be finite"));
    }
    if target == at {
        // An anchor takes its handles with it.
        let (dx, dy) = (x - now_x, y - now_y);
        for h in [ins, outs].into_iter().flatten() {
            if let Some(q) = cur.get_mut(h) {
                q[0] += dx;
                q[1] += dy;
            }
        }
    }
    if let Some(q) = cur.get_mut(target) {
        *q = [x, y];
    }
    c.keys.insert(frame, cur);
    commit(s, "Curve Key", curves, C)?;
    Ok(json!({"frame": frame, "x": x, "y": y}))
}

fn remove_key(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "curve.removeKey";
    let node = node_of(s, p, C)?;
    let mut curves = curves_of(s)?;
    let i = curve_at(&curves, node, C)?;
    let frame = frame_param(p, &Anim::of(&s.doc()?.doc), C)?;
    let Some(c) = curves.curves.get_mut(i) else { return Err(bad(C, "no such curve")) };
    if c.keys.remove(&frame).is_none() {
        return Err(bad(C, format!("no curve key at frame {frame}")));
    }
    commit(s, "Remove Curve Key", curves, C)?;
    Ok(json!({"frame": frame}))
}

fn remove(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "curve.remove";
    let node = node_of(s, p, C)?;
    let mut curves = curves_of(s)?;
    let i = curve_at(&curves, node, C)?;
    curves.curves.remove(i);
    commit(s, "Remove Curve Deformer", curves, C)?;
    Ok(json!({"node": node.0}))
}

fn info(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "curve.info";
    let node = node_of(s, p, C)?;
    let curves = curves_of(s)?;
    let i = curve_at(&curves, node, C)?;
    let Some(c) = curves.curves.get(i) else { return Err(bad(C, "no such curve")) };
    let frame = Anim::of(&s.doc()?.doc).current;
    let now: Vec<[f64; 2]> = c.at(frame).iter().map(|q| [q.x, q.y]).collect();
    Ok(json!({
        "node": c.node,
        "closed": c.closed,
        "anchors": anchors(c),
        "rest": c.rest.clone(),
        "now": now,
        "keys": c.keys.keys().collect::<Vec<_>>(),
        "radius": c.radius,
        "fade": c.fade,
    }))
}

fn set(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "curve.set";
    let node = node_of(s, p, C)?;
    let mut curves = curves_of(s)?;
    let i = curve_at(&curves, node, C)?;
    let radius_given = p.get("radius").is_some();
    let fade_given = p.get("fade").is_some();
    let new_radius = if radius_given { radius_param(p, C)? } else { None };
    let new_fade = if fade_given { Some(fade_param(p, C)?) } else { None };
    let Some(c) = curves.curves.get_mut(i) else { return Err(bad(C, "no such curve")) };
    if radius_given {
        c.radius = new_radius;
    }
    if let Some(f) = new_fade {
        c.fade = f;
    }
    let out = json!({"node": node.0, "radius": c.radius, "fade": c.fade});
    commit(s, "Set Curve Deformer", curves, C)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;
    use vectorcraft_geom::Point;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s
    }

    /// A 100 × 50 rectangle at the origin; its id (it becomes the selection).
    fn rect(s: &mut Session) -> u64 {
        s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 100, "height": 50})).unwrap()["id"].as_u64().unwrap()
    }

    fn info(s: &mut Session, node: u64) -> Value {
        s.execute("curve.info", &json!({"node": node})).unwrap()
    }

    fn points(v: &Value, key: &str) -> Vec<[f64; 2]> {
        v[key].as_array().unwrap().iter().map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()]).collect()
    }

    fn near(a: [f64; 2], b: [f64; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
    }

    #[test]
    fn add_builds_an_open_curve_along_the_middle_line() {
        let mut s = session();
        assert!(s.execute("curve.add", &json!({})).is_err(), "nothing selected");
        let r = rect(&mut s);
        let a = s.execute("curve.add", &json!({})).unwrap();
        assert_eq!((a["node"].as_u64(), a["anchors"].as_u64(), a["segments"].as_u64()), (Some(r), Some(3), Some(2)));
        let rest = points(&info(&mut s, r), "rest");
        assert_eq!(rest.len(), 7, "3 anchors and 2 handles each way");
        assert_eq!((rest[0], rest[6]), ([0.0, 25.0], [100.0, 25.0]));
        assert!(near(rest[1], [100.0 / 6.0, 25.0]), "straight handle a third along: {:?}", rest[1]);
        assert!(s.execute("curve.add", &json!({})).is_err(), "already has a curve");
    }

    #[test]
    fn a_closed_curve_has_twelve_rest_points_at_the_corners() {
        let mut s = session();
        let r = rect(&mut s);
        let a = s.execute("curve.add", &json!({"closed": true})).unwrap();
        assert_eq!((a["anchors"].as_u64(), a["segments"].as_u64()), (Some(4), Some(4)));
        let rest = points(&info(&mut s, r), "rest");
        assert_eq!(rest.len(), 12);
        assert_eq!((rest[0], rest[3], rest[6], rest[9]), ([-4.0, -4.0], [104.0, -4.0], [104.0, 54.0], [-4.0, 54.0]));
    }

    #[test]
    fn keying_the_last_anchor_moves_the_rectangles_right_middle() {
        let mut s = session();
        s.execute("anim.setFrames", &json!({"frames": 5})).unwrap();
        let n = rect(&mut s);
        s.execute("curve.add", &json!({"node": n})).unwrap();
        // The right-middle corner of the rectangle is the curve's last anchor at rest.
        let k = s.execute("curve.key", &json!({"node": n, "anchor": 2, "frame": 5, "x": 150, "y": 60})).unwrap();
        assert_eq!(k, json!({"frame": 5, "x": 150.0, "y": 60.0}));
        s.execute("anim.goto", &json!({"frame": 5})).unwrap();
        assert!(near(points(&info(&mut s, n), "now")[6], [150.0, 60.0]));
        let posed = crate::cmd::curve::warps(&s.doc().unwrap().doc, 5);
        let moved = posed[&n].map(Point::new(100.0, 25.0));
        assert!((moved - Point::new(150.0, 60.0)).hypot() < 1.0, "{moved:?}");
    }

    #[test]
    fn handles_move_alone_and_an_anchor_takes_its_handles() {
        let mut s = session();
        s.execute("anim.setFrames", &json!({"frames": 5})).unwrap();
        let n = rect(&mut s);
        s.execute("curve.add", &json!({"node": n})).unwrap();
        s.execute("curve.key", &json!({"node": n, "anchor": 0, "handle": "out", "frame": 2, "dy": -10})).unwrap();
        s.execute("curve.key", &json!({"node": n, "anchor": 1, "frame": 2, "dx": 5})).unwrap();
        s.execute("anim.goto", &json!({"frame": 2})).unwrap();
        let now = points(&info(&mut s, n), "now");
        assert!(near(now[0], [0.0, 25.0]), "the anchor stays: {:?}", now[0]);
        assert!(near(now[1], [100.0 / 6.0, 15.0]), "its out handle moved alone: {:?}", now[1]);
        assert!(near(now[3], [55.0, 25.0]), "the anchor moved: {:?}", now[3]);
        assert!(near(now[2], [100.0 / 3.0 + 5.0, 25.0]), "its in handle came along: {:?}", now[2]);
        assert!(near(now[4], [200.0 / 3.0 + 5.0, 25.0]), "its out handle came along: {:?}", now[4]);
    }

    #[test]
    fn set_remove_key_and_remove_work_and_survive_save() {
        let mut s = session();
        s.execute("anim.setFrames", &json!({"frames": 5})).unwrap();
        let n = rect(&mut s);
        s.execute("curve.add", &json!({"node": n, "radius": 20})).unwrap();
        s.execute("curve.set", &json!({"node": n, "radius": null, "fade": 4})).unwrap();
        s.execute("curve.key", &json!({"node": n, "anchor": 1, "frame": 3, "dy": 9})).unwrap();
        let i = info(&mut s, n);
        assert_eq!((i["radius"].clone(), i["fade"].clone(), i["keys"].clone()), (Value::Null, json!(4.0), json!([3])));
        let before = Curves::of(&s.doc().unwrap().doc);
        let bytes = vectorcraft_format::save(&s.doc().unwrap().doc, false);
        let back = vectorcraft_format::load(&bytes).unwrap();
        assert_eq!(Curves::of(&back), before);
        assert_eq!(s.execute("curve.removeKey", &json!({"node": n, "frame": 3})).unwrap(), json!({"frame": 3}));
        assert!(s.execute("curve.removeKey", &json!({"node": n, "frame": 3})).is_err());
        assert!(info(&mut s, n)["keys"].as_array().unwrap().is_empty());
        assert_eq!(s.execute("curve.remove", &json!({"node": n})).unwrap(), json!({"node": n}));
        assert!(s.execute("curve.info", &json!({"node": n})).is_err(), "no curve left");
    }

    #[test]
    fn bad_params_are_errors_and_change_nothing() {
        let mut s = session();
        s.execute("anim.setFrames", &json!({"frames": 5})).unwrap();
        let r = rect(&mut s);
        assert!(s.execute("curve.add", &json!({"node": 999_999})).is_err(), "no such object");
        assert!(s.execute("curve.add", &json!({"node": r, "closed": "yes"})).is_err());
        assert!(s.execute("curve.add", &json!({"node": r, "radius": -1})).is_err());
        assert!(s.execute("curve.add", &json!({"node": r, "fade": "x"})).is_err());
        s.execute("curve.add", &json!({"node": r})).unwrap();
        assert!(s.execute("curve.add", &json!({"node": r})).is_err(), "already has a curve");
        assert!(s.execute("curve.key", &json!({"node": r, "anchor": 3, "dx": 1})).is_err(), "3 anchors: 0 to 2");
        assert!(s.execute("curve.key", &json!({"node": r, "dx": 1})).is_err(), "anchor missing");
        assert!(s.execute("curve.key", &json!({"node": r, "anchor": 0, "handle": "in", "dx": 1})).is_err(), "open start has no in handle");
        assert!(s.execute("curve.key", &json!({"node": r, "anchor": 0, "handle": "mid", "dx": 1})).is_err());
        assert!(s.execute("curve.key", &json!({"node": r, "anchor": 0, "x": "nan"})).is_err());
        assert!(s.execute("curve.key", &json!({"node": r, "anchor": 0, "frame": 9})).is_err(), "frame past the end");
        assert!(s.execute("curve.set", &json!({"node": r, "radius": -2})).is_err());
        assert!(s.execute("curve.set", &json!({"node": r, "fade": -2})).is_err());
        assert!(s.execute("curve.removeKey", &json!({"node": r, "frame": 3})).is_err());
        let other = s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 10, "height": 10})).unwrap()["id"].as_u64().unwrap();
        assert!(s.execute("curve.add", &json!({"node": other, "points": [[0, 0]]})).is_err(), "one anchor is no curve");
        assert!(s.execute("curve.add", &json!({"node": other, "points": [[0, 0], [1, "x"]]})).is_err());
        let i = info(&mut s, r);
        assert_eq!((i["rest"].as_array().map(Vec::len), i["keys"].clone(), i["radius"].clone()), (Some(7), json!([]), Value::Null));
        assert!(s.execute("curve.info", &json!({"node": other})).is_err(), "the refused curve was not stored");
    }
}
