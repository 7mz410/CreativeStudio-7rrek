//! 7rrek: `anim.arcs` follows named parts across frames, the way an animator checks arcs,
//! spacing and feet on the ground. Each part's bounds centre is sampled on every frame it shows.

use serde_json::json;
use vectorcraft_geom::{Point, Rect, union_opt};

use super::*;
use crate::cmd::anim::{Anim, at_frame};

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        query "anim.arcs",
        "Arcs",
        [],
        None,
        "{tracks: [name or node id] (at least one), from?: frame, to?: frame (1-based; default whole timeline), ground?: y of the ground line (y grows downward), travel?: px per frame a planted foot should move (default 0), check?: bool} tracks each part's bounds centre per frame: frames: [{frame, x, y, moved}] (moved: distance from the previous listed frame, 0 on the first; absent frames are skipped), lowest: {frame, y} (largest bottom edge), under: [frames whose bottom is below ground + 0.5], slides: [frames where a planted foot (bottom within 1 of ground on both frames) moves by more than 0.5 off travel] (under and slides need ground); crossings: [frames where the first two tracks swap their x order]; problems: [English strings for each under, slide and crossing]. check: an error listing the problems when there are any → {tracks, crossings, problems}",
        has_doc,
        arcs
    )]
}

/// A frame bound (`from`, `to`) from params, or `default`; must be a frame of the timeline.
fn bound(p: &Value, key: &str, default: u32, frames: u32) -> Result<u32> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_u64()
            .filter(|f| (1..=frames as u64).contains(f))
            .map(|f| f as u32)
            .ok_or_else(|| bad("anim.arcs", format!("{key} must be a whole number from 1 to {frames}"))),
    }
}

/// Bounds of the shown nodes that `track` (a name or a numeric id) matches on frame `doc`.
fn part_bounds(doc: &vectorcraft_doc::Document, track: &Value) -> Option<Rect> {
    let mut out = None;
    doc.walk(|n| {
        let hit = match track {
            Value::String(name) => n.name.as_deref() == Some(name.as_str()),
            v => v.as_u64().is_some_and(|id| id == n.id.0),
        };
        if hit && doc.is_visible(n.id) {
            out = union_opt(out, n.geometric_bounds());
        }
    });
    out
}

/// One listed frame of a track: centre and bottom edge.
struct Sample {
    frame: u32,
    x: f64,
    y: f64,
    bottom: f64,
}

fn arcs(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "anim.arcs";
    let st = s.doc()?;
    let anim = Anim::of(&st.doc);
    let tracks = p
        .get("tracks")
        .and_then(Value::as_array)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| bad(C, "give at least one track (a part name or node id)"))?;
    let from = bound(p, "from", 1, anim.frames)?;
    let to = bound(p, "to", anim.frames, anim.frames)?;
    if from > to {
        return Err(bad(C, "from must not be after to"));
    }
    let ground = p.get("ground").and_then(Value::as_f64).filter(|g| g.is_finite());
    let travel = p.get("travel").and_then(Value::as_f64).filter(|t| t.is_finite()).unwrap_or(0.0);
    let check = p.get("check").and_then(Value::as_bool).unwrap_or(false);

    // Sample every track on every frame of the range.
    let mut samples: Vec<Vec<Sample>> = tracks.iter().map(|_| Vec::new()).collect();
    for f in from..=to {
        let shown = at_frame(&st.doc, f);
        for (t, track) in tracks.iter().enumerate() {
            if let Some(r) = part_bounds(&shown, track) {
                let c: Point = r.center();
                samples[t].push(Sample { frame: f, x: c.x, y: c.y, bottom: r.y1 });
            }
        }
    }
    if let Some(t) = samples.iter().position(Vec::is_empty) {
        let name = tracks.get(t).map(|v| v.to_string()).unwrap_or_default();
        return Err(bad(C, format!("no part named or numbered {name} in frames {from} to {to}")));
    }

    let mut problems: Vec<String> = Vec::new();
    let mut out_tracks = Vec::new();
    for (t, track) in tracks.iter().enumerate() {
        let name = track.as_str().map(str::to_owned).unwrap_or_else(|| track.to_string());
        let list = &samples[t];
        let mut frames = Vec::new();
        let mut prev: Option<&Sample> = None;
        for sm in list {
            let moved = prev.map_or(0.0, |p| (sm.x - p.x).hypot(sm.y - p.y));
            frames.push(json!({"frame": sm.frame, "x": sm.x, "y": sm.y, "moved": moved}));
            prev = Some(sm);
        }
        let lowest = list.iter().fold(None::<&Sample>, |best, sm| match best {
            Some(b) if b.bottom >= sm.bottom => Some(b),
            _ => Some(sm),
        });
        let mut under = Vec::new();
        let mut slides = Vec::new();
        if let Some(g) = ground {
            for sm in list.iter().filter(|sm| sm.bottom > g + 0.5) {
                under.push(sm.frame);
                problems.push(format!("{name} under the ground at frame {}", sm.frame));
            }
            for pair in list.windows(2) {
                let (a, b) = (&pair[0], &pair[1]);
                let planted = |sm: &Sample| (sm.bottom - g).abs() <= 1.0;
                if b.frame == a.frame + 1 && planted(a) && planted(b) && (b.x - a.x + travel).abs() > 0.5 {
                    slides.push(b.frame);
                    problems.push(format!("{name} slides at frame {}", b.frame));
                }
            }
        }
        out_tracks.push(json!({
            "track": track,
            "frames": frames,
            "lowest": lowest.map(|l| json!({"frame": l.frame, "y": l.bottom})),
            "under": under,
            "slides": slides,
        }));
    }

    // Crossings: the first two tracks swap their x order between consecutive frames.
    let mut crossings = Vec::new();
    if tracks.len() >= 2 {
        let (a, b) = (&samples[0], &samples[1]);
        let at = |list: &[Sample], f: u32| list.iter().find(|sm| sm.frame == f).map(|sm| sm.x);
        let names = (tracks[0].to_string(), tracks[1].to_string());
        for f in from + 1..=to {
            if let (Some(ax), Some(bx), Some(ap), Some(bp)) = (at(a, f), at(b, f), at(a, f - 1), at(b, f - 1)) {
                let (now, before) = (ax - bx, ap - bp);
                if now * before < 0.0 {
                    crossings.push(f);
                    problems.push(format!("{} and {} cross at frame {f}", names.0, names.1));
                }
            }
        }
    }

    if check && !problems.is_empty() {
        return Err(bad(C, problems.join("; ")));
    }
    Ok(json!({"tracks": out_tracks, "crossings": crossings, "problems": problems}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s
    }

    /// A 20×20 square named `name` at (x, y) in the current keyframe.
    fn square(s: &mut Session, name: &str, x: f64, y: f64) -> u64 {
        let id = s.execute("shape.rectangle", &json!({"x": x, "y": y, "width": 20, "height": 20})).unwrap()["id"].as_u64().unwrap();
        s.execute("layer.setProps", &json!({"id": id, "name": name})).unwrap();
        id
    }

    /// Frames 1 to 4 of a square named `ball`. Ground at y = 100 (bottom 100 is planted).
    /// Frame 1 and 2 are planted and 30 px apart (a slide with travel 0); frame 3 sinks to bottom 110.
    fn scene() -> Session {
        let mut s = session();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        square(&mut s, "ball", 10.0, 80.0);
        s.execute("anim.blankKeyframe", &json!({"frame": 2})).unwrap();
        square(&mut s, "ball", 40.0, 80.0);
        s.execute("anim.blankKeyframe", &json!({"frame": 3})).unwrap();
        square(&mut s, "ball", 40.0, 90.0);
        s.execute("anim.blankKeyframe", &json!({"frame": 4})).unwrap();
        square(&mut s, "ball", 40.0, 80.0);
        s
    }

    #[test]
    fn positions_and_moved_per_frame() {
        let mut s = scene();
        let r = s.execute("anim.arcs", &json!({"tracks": ["ball"], "to": 4})).unwrap();
        let frames = r["tracks"][0]["frames"].as_array().unwrap();
        assert_eq!(frames.len(), 4);
        assert_eq!(frames[0]["x"], 20.0);
        assert_eq!(frames[0]["y"], 90.0);
        assert_eq!(frames[0]["moved"], 0.0);
        assert_eq!(frames[1]["x"], 50.0);
        assert_eq!(frames[1]["moved"], 30.0);
        assert_eq!(frames[2]["moved"], 10.0);
        assert_eq!(r["tracks"][0]["lowest"]["frame"], 3);
        assert_eq!(r["tracks"][0]["lowest"]["y"], 110.0);
    }

    #[test]
    fn a_frame_below_ground_is_under_and_a_moving_planted_foot_slides() {
        let mut s = scene();
        let r = s.execute("anim.arcs", &json!({"tracks": ["ball"], "ground": 100.0})).unwrap();
        assert_eq!(r["tracks"][0]["under"], json!([3]));
        assert_eq!(r["tracks"][0]["slides"], json!([2]));
        assert!(r["problems"].as_array().unwrap().iter().any(|p| p == "ball under the ground at frame 3"));
    }

    #[test]
    fn check_errors_with_the_problems() {
        let mut s = scene();
        let err = s.execute("anim.arcs", &json!({"tracks": ["ball"], "ground": 100.0, "check": true})).unwrap_err().to_string();
        assert!(err.contains("under the ground at frame 3"), "{err}");
        assert!(err.contains("slides at frame 2"), "{err}");
    }

    #[test]
    fn unknown_track_is_an_error_naming_it() {
        let mut s = scene();
        let err = s.execute("anim.arcs", &json!({"tracks": ["nobody"]})).unwrap_err().to_string();
        assert!(err.contains("nobody"), "{err}");
    }
}
