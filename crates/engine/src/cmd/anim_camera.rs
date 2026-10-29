//! 7rrek (Creative Studio): camera commands. The camera's keys live in the animation's timing
//! (`Anim.camera`, see anim.rs); these commands set, remove and read them.

use serde_json::json;

use super::anim::{Anim, Cam, ease_param, frame_param, show};
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "anim.cameraKey",
            "Camera Keyframe",
            [],
            None,
            "{frame?: (default: the current frame), x?, y?: the view's centre (document points), zoom?: 0.01–100 (1: the artboard's size), rotation?: degrees the camera turns counter-clockwise (so the picture turns clockwise, as in Harmony), z?: how far the camera moves in toward the stage (document points; multiplane), ease?: \"linear\" | \"easeIn\" | \"easeOut\" | \"easeInOut\" | [x1, y1, x2, y2] (eases toward the next key)} sets the camera key at the frame. Missing fields keep the camera there, or with no camera yet, the first artboard's centre at zoom 1 → {frame, x, y, zoom, rotation, ease}",
            has_doc,
            camera_key
        ),
        cmd!(
            "anim.cameraRemoveKey",
            "Remove Camera Keyframe",
            [],
            None,
            "{frame?: (default: the current frame)} delete the camera key at the frame; error if there is none → {frame}",
            has_doc,
            camera_remove_key
        ),
        cmd!(
            query "anim.cameraInfo",
            "Camera",
            [],
            None,
            "{frame?: (default: the current frame)} → {keys: [{frame, x, y, zoom, rotation, ease}], at: the camera at the frame as {x, y, zoom, rotation, ease}, or null with no camera}",
            has_doc,
            camera_info
        ),
    ]
}

/// A camera as its command fields (`ease` null when it eases linearly).
fn cam_value(c: &Cam) -> Value {
    json!({"x": c.x, "y": c.y, "zoom": c.zoom, "rotation": c.rotation, "ease": c.ease, "z": c.z})
}

fn camera_key(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "anim.cameraKey";
    let st = s.doc()?;
    let anim = Anim::of(&st.doc);
    let frame = frame_param(p, &anim, C)?;
    let base = match anim.camera_at(frame) {
        Some(c) => c,
        None => {
            let board = st.doc.artboards.first().ok_or_else(|| bad(C, "no artboard to centre the camera on"))?.rect;
            let c = board.center();
            Cam { x: c.x, y: c.y, zoom: 1.0, rotation: 0.0, ease: None, z: 0.0 }
        }
    };
    // camera_at only eases between keys, so the ease comes from the key before the frame.
    let kept_ease = anim.camera.range(..=frame).next_back().or_else(|| anim.camera.iter().next()).and_then(|(_, c)| c.ease);
    let cam = Cam {
        x: number_or(p, "x", base.x, C)?,
        y: number_or(p, "y", base.y, C)?,
        zoom: number_or(p, "zoom", base.zoom, C)?,
        rotation: number_or(p, "rotation", base.rotation, C)?,
        ease: ease_param(p, C)?.unwrap_or(kept_ease),
        z: number_or(p, "z", base.z, C)?,
    };
    if !cam.valid() {
        return Err(bad(C, "zoom must be from 0.01 to 100, and x, y, z and rotation must be finite"));
    }
    s.edit("Camera Keyframe", |d, _| {
        let mut a = Anim::of(d);
        a.camera.insert(frame, cam);
        a.store(d);
        Ok(())
    })?;
    let current = Anim::of(&s.doc()?.doc).current;
    show(s, current)?;
    let mut out = cam_value(&cam);
    out["frame"] = json!(frame);
    Ok(out)
}

fn camera_remove_key(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "anim.cameraRemoveKey";
    let anim = Anim::of(&s.doc()?.doc);
    let frame = frame_param(p, &anim, C)?;
    if !anim.camera.contains_key(&frame) {
        return Err(bad(C, format!("no camera keyframe at frame {frame}")));
    }
    s.edit("Remove Camera Keyframe", |d, _| {
        let mut a = Anim::of(d);
        a.camera.remove(&frame);
        a.store(d);
        Ok(())
    })?;
    show(s, anim.current)?;
    Ok(json!({"frame": frame}))
}

fn camera_info(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "anim.cameraInfo";
    let anim = Anim::of(&s.doc()?.doc);
    let frame = frame_param(p, &anim, C)?;
    let keys: Vec<Value> = anim
        .camera
        .iter()
        .map(|(f, c)| {
            let mut v = cam_value(c);
            v["frame"] = json!(f);
            v
        })
        .collect();
    let at = anim.camera_at(frame).map(|c| cam_value(&c)).unwrap_or(Value::Null);
    Ok(json!({"keys": keys, "at": at}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s
    }

    fn key(s: &mut Session, frame: u64, x: f64, zoom: f64) {
        s.execute("anim.cameraKey", &json!({"frame": frame, "x": x, "y": 0.0, "zoom": zoom, "rotation": 0.0})).unwrap();
    }

    fn key_count(s: &mut Session) -> usize {
        s.execute("anim.cameraInfo", &json!({})).unwrap()["keys"].as_array().unwrap().len()
    }

    #[test]
    fn a_new_camera_key_starts_on_the_artboard() {
        let mut s = session();
        let c = s.doc().unwrap().doc.artboards[0].rect.center();
        let r = s.execute("anim.cameraKey", &json!({})).unwrap();
        assert!((r["x"].as_f64().unwrap() - c.x).abs() < 1e-9 && (r["y"].as_f64().unwrap() - c.y).abs() < 1e-9);
        assert_eq!((r["zoom"].as_f64(), r["rotation"].as_f64()), (Some(1.0), Some(0.0)));
    }

    #[test]
    fn camera_keys_ease_between_frames() {
        let mut s = session();
        key(&mut s, 1, 0.0, 1.0);
        key(&mut s, 11, 100.0, 4.0);
        let info = s.execute("anim.cameraInfo", &json!({"frame": 6})).unwrap();
        assert_eq!(info["keys"].as_array().unwrap().len(), 2);
        let at = &info["at"];
        assert!((at["x"].as_f64().unwrap() - 50.0).abs() < 1e-9, "x halfway: {at}");
        assert!((at["zoom"].as_f64().unwrap() - 2.0).abs() < 1e-9, "zoom geometric halfway: {at}");
        assert_eq!(s.execute("anim.cameraInfo", &json!({"frame": 1})).unwrap()["at"]["x"], 0.0);
    }

    #[test]
    fn bad_camera_values_are_errors() {
        let mut s = session();
        assert!(s.execute("anim.cameraKey", &json!({"zoom": 0})).is_err());
        assert!(s.execute("anim.cameraKey", &json!({"ease": "wobble"})).is_err());
        assert!(s.execute("anim.cameraRemoveKey", &json!({"frame": 3})).is_err());
    }

    #[test]
    fn removing_a_camera_key_is_undoable() {
        let mut s = session();
        key(&mut s, 1, 0.0, 1.0);
        key(&mut s, 11, 100.0, 4.0);
        s.execute("anim.cameraRemoveKey", &json!({"frame": 11})).unwrap();
        assert_eq!(key_count(&mut s), 1);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(key_count(&mut s), 2);
    }

    #[test]
    fn camera_keys_survive_save_and_open() {
        let mut s = session();
        key(&mut s, 1, 0.0, 1.0);
        s.execute("anim.cameraKey", &json!({"frame": 5, "x": 7.0, "zoom": 2.0, "ease": "easeInOut"})).unwrap();
        let before = Anim::of(&s.doc().unwrap().doc).camera;
        let bytes = vectorcraft_format::save(&s.doc().unwrap().doc, false);
        let back = vectorcraft_format::load(&bytes).unwrap();
        assert_eq!(Anim::of(&back).camera, before);
    }
}
