//! 7rrek (Creative Studio): scene setup in one step. `scene.setup` sets an artboard's size, the
//! timeline's frame rate and length, and its video safe areas as one undo step. It is a thin
//! wrapper: the same rules as `artboard.setProps` (size, locked artboards), `anim.setFps` and
//! `anim.setFrames`, all checked before the document changes.

use serde_json::{Value, json};
use vectorcraft_doc::Document;
use vectorcraft_geom::Rect;

use super::anim::{Anim, MAX_FPS, MAX_FRAMES};
use super::*;
use crate::EngineError;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "scene.setup",
            "Scene Setup",
            [],
            None,
            "{width?, height?: artboard size in points (at least 1; the art stays), fps?: 1–60, frames?: 1–10000 (the timeline's length), safeAreas?: bool (Show Video Safe Areas), artboard?: index (default 0)} the fields left out keep their value; all of them change in one undo step, and a bad field changes nothing → {width, height, fps, frames, safeAreas, artboard}",
            has_doc,
            setup
        ),
        cmd!(
            query "scene.info",
            "Scene Info",
            [],
            None,
            "{artboard?: index (default 0)} → {width, height, fps, frames, safeAreas, artboard}",
            has_doc,
            info
        ),
    ]
}

/// The artboard `p` names (`artboard`, default 0).
fn artboard_param(p: &Value) -> Result<usize> {
    match p.get("artboard") {
        None | Some(Value::Null) => Ok(0),
        Some(v) => {
            v.as_u64().and_then(|i| usize::try_from(i).ok()).ok_or_else(|| bad("scene", format!("`artboard` must be an artboard index, not {v}")))
        }
    }
}

/// The size `key` gives, or `now` when `p` leaves it out. At least 1 point, as artboards are.
fn size_param(p: &Value, key: &str, now: f64) -> Result<f64> {
    let v = number_or(p, key, now, "scene.setup")?;
    if v.is_finite() && v >= 1.0 { Ok(v) } else { Err(bad("scene.setup", format!("{key} must be a number of at least 1"))) }
}

/// The whole number `key` gives within `lo..=hi`, or `now` when `p` leaves it out.
fn whole_param(p: &Value, key: &str, now: u32, lo: u64, hi: u64) -> Result<u32> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(now),
        Some(v) => v
            .as_u64()
            .filter(|n| (lo..=hi).contains(n))
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| bad("scene.setup", format!("{key} must be a whole number from {lo} to {hi}"))),
    }
}

fn scene_json(d: &Document, index: usize) -> Result<Value> {
    let a = d.artboards.get(index).ok_or_else(|| EngineError::Other("no such artboard".into()))?;
    let anim = Anim::of(d);
    Ok(json!({
        "width": a.rect.width(),
        "height": a.rect.height(),
        "fps": anim.fps,
        "frames": anim.frames,
        "safeAreas": a.show_video_safe_areas,
        "artboard": index,
    }))
}

fn info(s: &mut Session, p: &Value) -> Result<Value> {
    let index = artboard_param(p)?;
    scene_json(&s.doc()?.doc, index)
}

fn setup(s: &mut Session, p: &Value) -> Result<Value> {
    let index = artboard_param(p)?;
    // Read every current value and check every field before the document changes.
    let (was, width, height, fps, frames, safe, locked) = {
        let d = &s.doc()?.doc;
        let a = d.artboards.get(index).ok_or_else(|| EngineError::Other("no such artboard".into()))?;
        let anim = Anim::of(d);
        let was = a.rect;
        let width = size_param(p, "width", was.width())?;
        let height = size_param(p, "height", was.height())?;
        let fps = whole_param(p, "fps", anim.fps, 1, MAX_FPS as u64)?;
        let frames = whole_param(p, "frames", anim.frames, 1, MAX_FRAMES as u64)?;
        let safe = switch_or(p, "safeAreas", a.show_video_safe_areas, "scene.setup")?;
        (was, width, height, fps, frames, safe, a.locked)
    };
    let resized = (width - was.width()).abs() > 1e-6 || (height - was.height()).abs() > 1e-6;
    if resized && locked {
        return Err(EngineError::Other("the artboard is locked".into()));
    }
    s.edit("Scene Setup", |d, _| {
        let a = d.artboards.get_mut(index).ok_or_else(|| EngineError::Other("no such artboard".into()))?;
        a.rect = Rect::new(was.x0, was.y0, was.x0 + width, was.y0 + height);
        a.show_video_safe_areas = safe;
        let mut anim = Anim::of(d);
        anim.fps = fps;
        anim.frames = frames;
        anim.current = anim.current.min(frames);
        anim.store(d);
        Ok(())
    })?;
    scene_json(&s.doc()?.doc, index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 1920, "height": 1080})).unwrap();
        s.execute("anim.setFps", &json!({"fps": 24})).unwrap();
        s
    }

    fn sized(v: &Value) -> (f64, f64) {
        (v["width"].as_f64().unwrap(), v["height"].as_f64().unwrap())
    }

    #[test]
    fn setup_changes_everything_and_one_undo_restores_it() {
        let mut s = session();
        let before = s.execute("scene.info", &json!({})).unwrap();
        assert_eq!(sized(&before), (1920.0, 1080.0));
        assert_eq!((before["fps"].as_u64(), before["safeAreas"].as_bool()), (Some(24), Some(false)));

        let after = s.execute("scene.setup", &json!({"width": 1280, "height": 720, "fps": 30, "frames": 48, "safeAreas": true})).unwrap();
        assert_eq!(sized(&after), (1280.0, 720.0));
        let info = s.execute("scene.info", &json!({})).unwrap();
        assert_eq!(sized(&info), (1280.0, 720.0));
        assert_eq!((info["fps"].as_u64(), info["frames"].as_u64(), info["safeAreas"].as_bool()), (Some(30), Some(48), Some(true)));

        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.execute("scene.info", &json!({})).unwrap(), before, "one undo restores all four values");
    }

    #[test]
    fn omitted_fields_keep_their_value() {
        let mut s = session();
        s.execute("scene.setup", &json!({"frames": 12})).unwrap();
        let info = s.execute("scene.info", &json!({})).unwrap();
        assert_eq!((sized(&info), info["fps"].as_u64(), info["frames"].as_u64()), ((1920.0, 1080.0), Some(24), Some(12)));
    }

    #[test]
    fn bad_fields_are_errors_and_change_nothing() {
        let mut s = session();
        let before = s.execute("scene.info", &json!({})).unwrap();
        for p in [
            json!({"width": 640, "fps": 0}),
            json!({"width": 640, "height": 0}),
            json!({"width": "640"}),
            json!({"frames": 10001}),
            json!({"safeAreas": "yes"}),
            json!({"height": 480, "artboard": 9}),
        ] {
            assert!(s.execute("scene.setup", &p).is_err(), "{p} should fail");
            assert_eq!(s.execute("scene.info", &json!({})).unwrap(), before, "{p} changed the document");
        }
        assert!(s.execute("scene.info", &json!({"artboard": 9})).is_err());
    }

    #[test]
    fn limits_match_anim_setfps_and_setframes() {
        let mut s = session();
        assert!(s.execute("anim.setFps", &json!({"fps": 60})).is_ok());
        assert!(s.execute("anim.setFps", &json!({"fps": 61})).is_err());
        assert!(s.execute("scene.setup", &json!({"fps": 60})).is_ok());
        assert!(s.execute("scene.setup", &json!({"fps": 61})).is_err());
        assert!(s.execute("scene.setup", &json!({"frames": 10000})).is_ok());
        assert!(s.execute("scene.setup", &json!({"frames": 10001})).is_err());
        assert!(s.execute("scene.setup", &json!({"frames": 0})).is_err());
    }

    #[test]
    fn export_after_setup_is_the_new_size() {
        let mut s = session();
        s.execute("scene.setup", &json!({"width": 1280, "height": 720, "fps": 30, "frames": 48, "safeAreas": true})).unwrap();
        let dir = std::env::temp_dir().join(format!("scene_setup_export_{}", std::process::id()));
        let dir_str = dir.to_string_lossy().into_owned();
        s.execute("anim.exportFrames", &json!({"dir": dir_str, "scale": 1, "from": 1, "to": 1})).unwrap();
        let img = image::open(dir.join("frame_0001.png")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!((img.width(), img.height()), (1280, 720));
    }
}
