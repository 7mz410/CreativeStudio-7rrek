//! 7rrek (Creative Studio): bone commands. The skeletons live in `rig.rs`.

use serde_json::{Value, json};
use vectorcraft_doc::{Document, NodeId};

use super::anim::{Anim, ease_param, frame_param, row_param, show};
use super::rig::{Joint, MAX_JOINTS, Pose, Rig, Skeleton};
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "rig.addJoint",
            "Add Joint",
            [],
            None,
            "{row?: layer id (default: the current layer's row), x, y: the joint's rest spot (document points), parent?: index of the joint it hangs from (default: a root), name?: text (default: \"Joint N\")} adds a joint; pose keys get an angle of 0 for it → {index}",
            has_doc,
            add_joint
        ),
        cmd!(
            "rig.moveJoint",
            "Move Joint",
            [],
            None,
            "{row?: layer id (default: the current layer's row), index: joint index, x, y: the joint's rest spot (document points)} moves a joint at rest → {index, x, y}",
            has_doc,
            move_joint
        ),
        cmd!(
            "rig.removeJoint",
            "Remove Joint",
            [],
            None,
            "{row?: layer id (default: the current layer's row), index: joint index} removes the joint and every joint below it, renumbering the rest; removing the last joint removes the skeleton → {removed}",
            has_doc,
            remove_joint
        ),
        cmd!(
            "rig.pose",
            "Pose Joints",
            [],
            None,
            "{row?: layer id (default: the current layer's row), frame?: (default: the current frame), angles?: [degrees, one per joint], joint?: joint index, angle?: degrees (sets one joint; give both or neither), ease?: \"linear\" | \"easeIn\" | \"easeOut\" | \"easeInOut\" | [x1, y1, x2, y2] (eases toward the next key)} sets the pose key at the frame; joints not given keep the pose there, else 0 → {frame, angles}",
            has_doc,
            pose
        ),
        cmd!(
            "rig.removePose",
            "Remove Pose Key",
            [],
            None,
            "{row?: layer id (default: the current layer's row), frame?: (default: the current frame)} removes the pose key at the frame; error if there is none → {frame}",
            has_doc,
            remove_pose
        ),
        cmd!(
            query "rig.info",
            "Skeleton",
            [],
            None,
            "{row?: layer id (default: the current layer's row), frame?: (default: the current frame)} → {joints: [{index, name, parent, x, y}], keys: [{frame, angles, ease}], at: the angles at the frame (held or eased), or null}",
            has_doc,
            info
        ),
    ]
}

/// The skeleton on `row`, or an empty one.
fn skeleton(s: &Session, row: NodeId) -> Result<Skeleton> {
    Ok(Rig::of(&s.doc()?.doc).skeletons.remove(&row.0).unwrap_or_default())
}

/// Store `sk` on `row`; a skeleton without joints is removed.
fn save(d: &mut Document, row: NodeId, sk: Skeleton) {
    let mut r = Rig::of(d);
    if sk.joints.is_empty() {
        r.skeletons.remove(&row.0);
    } else {
        r.skeletons.insert(row.0, sk);
    }
    r.store(d);
}

/// Store `sk` on `row` as one undo step, then show the current frame.
fn commit(s: &mut Session, label: &str, row: NodeId, sk: Skeleton, cmd: &str) -> Result<()> {
    if !sk.valid() {
        return Err(bad(cmd, "the edit would leave an invalid skeleton"));
    }
    s.edit(label, |d, _| {
        save(d, row, sk);
        Ok(())
    })?;
    let current = Anim::of(&s.doc()?.doc).current;
    show(s, current)?;
    Ok(())
}

/// A finite number `p` gives for `key`.
fn num_req(p: &Value, key: &str, cmd: &str) -> Result<f64> {
    match p.get(key).and_then(Value::as_f64) {
        Some(v) if v.is_finite() => Ok(v),
        _ => Err(bad(cmd, format!("`{key}` must be a finite number"))),
    }
}

/// A joint index `p` gives for `key`, below `n` joints.
fn joint_param(p: &Value, key: &str, n: usize, cmd: &str) -> Result<usize> {
    p.get(key)
        .and_then(Value::as_u64)
        .map(index)
        .filter(|i| *i < n)
        .ok_or_else(|| bad(cmd, format!("`{key}` must be a joint index from 0 to {}", n.saturating_sub(1))))
}

fn add_joint(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "rig.addJoint";
    let row = row_param(s, p, C)?;
    let mut sk = skeleton(s, row)?;
    let n = sk.joints.len();
    if n >= MAX_JOINTS {
        return Err(bad(C, format!("a skeleton takes at most {MAX_JOINTS} joints")));
    }
    let (x, y) = (num_req(p, "x", C)?, num_req(p, "y", C)?);
    let parent = match p.get("parent") {
        None | Some(Value::Null) => None,
        Some(_) => Some(joint_param(p, "parent", n, C)?),
    };
    let name = match p.get("name") {
        None | Some(Value::Null) => format!("Joint {}", n + 1),
        Some(Value::String(t)) if !t.trim().is_empty() => t.clone(),
        Some(_) => return Err(bad(C, "name must be non-empty text")),
    };
    sk.joints.push(Joint { name, parent, x, y });
    for pose in sk.keys.values_mut() {
        pose.angles.push(0.0);
    }
    commit(s, "Add Joint", row, sk, C)?;
    Ok(json!({"index": n}))
}

fn move_joint(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "rig.moveJoint";
    let row = row_param(s, p, C)?;
    let mut sk = skeleton(s, row)?;
    let i = joint_param(p, "index", sk.joints.len(), C)?;
    let (x, y) = (num_req(p, "x", C)?, num_req(p, "y", C)?);
    let Some(j) = sk.joints.get_mut(i) else { return Err(bad(C, "no such joint")) };
    (j.x, j.y) = (x, y);
    commit(s, "Move Joint", row, sk, C)?;
    Ok(json!({"index": i, "x": x, "y": y}))
}

fn remove_joint(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "rig.removeJoint";
    let row = row_param(s, p, C)?;
    let sk = skeleton(s, row)?;
    let n = sk.joints.len();
    let i = joint_param(p, "index", n, C)?;
    // Parents come before their children, so one pass finds the whole branch.
    let mut gone: Vec<bool> = Vec::with_capacity(n);
    for (k, j) in sk.joints.iter().enumerate() {
        let below = k == i || j.parent.is_some_and(|q| gone.get(q) == Some(&true));
        gone.push(below);
    }
    // The new index of each kept joint, by its old one.
    let mut at = vec![0; n];
    let mut kept = 0;
    for (old, g) in gone.iter().enumerate() {
        if !g {
            if let Some(slot) = at.get_mut(old) {
                *slot = kept;
            }
            kept += 1;
        }
    }
    let joints = sk
        .joints
        .iter()
        .zip(&gone)
        .filter(|(_, g)| !**g)
        .map(|(j, _)| {
            let parent = match j.parent {
                Some(q) => Some(*at.get(q).ok_or_else(|| bad(C, "a joint's parent is missing"))?),
                None => None,
            };
            Ok(Joint { name: j.name.clone(), parent, x: j.x, y: j.y })
        })
        .collect::<Result<Vec<Joint>>>()?;
    let keys = sk
        .keys
        .iter()
        .map(|(f, pose)| (*f, Pose { angles: pose.angles.iter().zip(&gone).filter(|(_, g)| !**g).map(|(a, _)| *a).collect(), ease: pose.ease }))
        .collect();
    let removed = n - kept;
    commit(s, "Remove Joint", row, Skeleton { joints, keys }, C)?;
    Ok(json!({"removed": removed}))
}

fn pose(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "rig.pose";
    let row = row_param(s, p, C)?;
    let mut sk = skeleton(s, row)?;
    let n = sk.joints.len();
    if n == 0 {
        return Err(bad(C, "the layer has no skeleton; add a joint first"));
    }
    let frame = frame_param(p, &Anim::of(&s.doc()?.doc), C)?;
    let mut angles = sk.pose_at(frame).unwrap_or_else(|| vec![0.0; n]);
    let given = |k: &str| p.get(k).filter(|v| !v.is_null());
    match (given("angles"), given("joint"), given("angle")) {
        (Some(list), None, None) => {
            let list = list.as_array().filter(|a| a.len() == n).ok_or_else(|| bad(C, format!("angles must list {n} numbers, one per joint")))?;
            angles = list
                .iter()
                .map(|v| v.as_f64().filter(|a| a.is_finite()).ok_or_else(|| bad(C, "angles must be finite numbers")))
                .collect::<Result<Vec<f64>>>()?;
        }
        (None, Some(_), Some(_)) => {
            let j = joint_param(p, "joint", n, C)?;
            let angle = num_req(p, "angle", C)?;
            let Some(slot) = angles.get_mut(j) else { return Err(bad(C, "no such joint")) };
            *slot = angle;
        }
        (None, None, None) => {}
        _ => return Err(bad(C, "give angles, or joint with angle")),
    }
    let existing = sk.keys.get(&frame).and_then(|k| k.ease);
    let ease = ease_param(p, C)?.unwrap_or(existing);
    let shown = angles.clone();
    sk.keys.insert(frame, Pose { angles, ease });
    commit(s, "Pose Joints", row, sk, C)?;
    Ok(json!({"frame": frame, "angles": shown}))
}

fn remove_pose(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "rig.removePose";
    let row = row_param(s, p, C)?;
    let mut sk = skeleton(s, row)?;
    let frame = frame_param(p, &Anim::of(&s.doc()?.doc), C)?;
    if sk.keys.remove(&frame).is_none() {
        return Err(bad(C, format!("no pose key at frame {frame}")));
    }
    commit(s, "Remove Pose Key", row, sk, C)?;
    Ok(json!({"frame": frame}))
}

fn info(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "rig.info";
    let row = row_param(s, p, C)?;
    let sk = skeleton(s, row)?;
    let frame = frame_param(p, &Anim::of(&s.doc()?.doc), C)?;
    let joints: Vec<Value> =
        sk.joints.iter().enumerate().map(|(i, j)| json!({"index": i, "name": j.name, "parent": j.parent, "x": j.x, "y": j.y})).collect();
    let keys: Vec<Value> = sk.keys.iter().map(|(f, k)| json!({"frame": f, "angles": k.angles, "ease": k.ease})).collect();
    let at = sk.pose_at(frame).map(|a| json!(a)).unwrap_or(Value::Null);
    Ok(json!({"joints": joints, "keys": keys, "at": at}))
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

    /// Shoulder, elbow and hand along x, 100 apart.
    fn arm(s: &mut Session) {
        s.execute("rig.addJoint", &json!({"x": 0, "y": 0, "name": "shoulder"})).unwrap();
        s.execute("rig.addJoint", &json!({"x": 100, "y": 0, "parent": 0, "name": "elbow"})).unwrap();
        s.execute("rig.addJoint", &json!({"x": 200, "y": 0, "parent": 1, "name": "hand"})).unwrap();
    }

    fn look(s: &mut Session, frame: u64) -> Value {
        s.execute("rig.info", &json!({"frame": frame})).unwrap()
    }

    fn joint_count(s: &mut Session) -> usize {
        look(s, 1)["joints"].as_array().unwrap().len()
    }

    #[test]
    fn an_elbow_pose_eases_between_frames() {
        let mut s = session();
        arm(&mut s);
        s.execute("rig.pose", &json!({"frame": 1, "angles": [0, 0, 0]})).unwrap();
        let r = s.execute("rig.pose", &json!({"frame": 9, "joint": 1, "angle": 90})).unwrap();
        assert_eq!((r["frame"].as_u64(), r["angles"][1].as_f64()), (Some(9), Some(90.0)));
        let at = look(&mut s, 5);
        assert!((at["at"][1].as_f64().unwrap() - 45.0).abs() < 1e-9, "{at}");
        assert_eq!(at["joints"][1]["name"], "elbow");
        assert_eq!(at["joints"][0]["parent"], Value::Null);
        assert_eq!(at["joints"][2]["parent"], 1);
        assert_eq!(at["keys"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn a_new_joint_gets_an_angle_in_every_pose() {
        let mut s = session();
        arm(&mut s);
        s.execute("rig.pose", &json!({"frame": 1, "angles": [5, 6, 7]})).unwrap();
        let r = s.execute("rig.addJoint", &json!({"x": 0, "y": 50, "parent": 0})).unwrap();
        assert_eq!(r["index"], 3);
        let at = look(&mut s, 1);
        assert_eq!(at["keys"][0]["angles"], json!([5.0, 6.0, 7.0, 0.0]));
        assert_eq!(at["joints"][3]["name"], "Joint 4");
    }

    #[test]
    fn bad_pose_inputs_are_errors() {
        let mut s = session();
        assert!(s.execute("rig.pose", &json!({"angles": [0]})).is_err(), "no skeleton yet");
        arm(&mut s);
        for (label, p) in [
            ("short list", json!({"angles": [0, 90]})),
            ("text angle in list", json!({"angles": ["90", 0, 0]})),
            ("joint without angle", json!({"joint": 1})),
            ("angle without joint", json!({"angle": 5})),
            ("angles and joint", json!({"angles": [0, 0, 0], "joint": 1, "angle": 5})),
            ("joint out of range", json!({"joint": 3, "angle": 5})),
            ("text angle", json!({"joint": 1, "angle": "90"})),
            ("frame 0", json!({"frame": 0, "angles": [0, 0, 0]})),
            ("unknown ease", json!({"angles": [0, 0, 0], "ease": "wobble"})),
        ] {
            assert!(s.execute("rig.pose", &p).is_err(), "{label}");
        }
    }

    #[test]
    fn removing_a_joint_takes_its_branch_and_renumbers() {
        let mut s = session();
        arm(&mut s);
        s.execute("rig.addJoint", &json!({"x": 0, "y": 50, "parent": 0, "name": "head"})).unwrap();
        s.execute("rig.pose", &json!({"frame": 1, "angles": [1, 2, 3, 4]})).unwrap();
        let r = s.execute("rig.removeJoint", &json!({"index": 1})).unwrap();
        assert_eq!(r["removed"], 2);
        let at = look(&mut s, 1);
        assert_eq!(at["joints"].as_array().unwrap().len(), 2);
        assert_eq!((at["joints"][1]["name"].as_str(), at["joints"][1]["parent"].as_u64()), (Some("head"), Some(0)));
        assert_eq!(at["keys"][0]["angles"], json!([1.0, 4.0]));
        assert_eq!(at["at"], json!([1.0, 4.0]));
        // Removing the root takes the rest, and the last joint takes the skeleton with it.
        assert_eq!(s.execute("rig.removeJoint", &json!({"index": 0})).unwrap()["removed"], 2);
        let gone = look(&mut s, 1);
        assert_eq!((gone["joints"].as_array().unwrap().len(), gone["keys"].as_array().unwrap().len(), &gone["at"]), (0, 0, &Value::Null));
    }

    #[test]
    fn edits_undo() {
        let mut s = session();
        arm(&mut s);
        s.execute("rig.pose", &json!({"frame": 1, "angles": [0, 0, 0]})).unwrap();
        s.execute("rig.removeJoint", &json!({"index": 1})).unwrap();
        assert_eq!(joint_count(&mut s), 1);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(joint_count(&mut s), 3);
        s.execute("rig.moveJoint", &json!({"index": 2, "x": 9, "y": 9})).unwrap();
        assert_eq!(look(&mut s, 1)["joints"][2]["x"], 9.0);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(look(&mut s, 1)["joints"][2]["x"], 200.0);
        s.execute("rig.removePose", &json!({"frame": 1})).unwrap();
        assert_eq!(look(&mut s, 1)["keys"].as_array().unwrap().len(), 0);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(look(&mut s, 1)["keys"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn bad_joint_inputs_are_errors() {
        let mut s = session();
        assert!(s.execute("rig.addJoint", &json!({"x": "40", "y": 0})).is_err());
        assert!(s.execute("rig.addJoint", &json!({"y": 0})).is_err());
        assert!(s.execute("rig.addJoint", &json!({"x": 0, "y": 0, "parent": 0})).is_err(), "no joint 0 yet");
        arm(&mut s);
        assert!(s.execute("rig.addJoint", &json!({"x": 0, "y": 0, "parent": 3})).is_err());
        assert!(s.execute("rig.addJoint", &json!({"x": 0, "y": 0, "name": ""})).is_err());
        assert!(s.execute("rig.moveJoint", &json!({"index": 3, "x": 0, "y": 0})).is_err());
        assert!(s.execute("rig.removeJoint", &json!({"index": 3})).is_err());
        assert!(s.execute("rig.removePose", &json!({"frame": 4})).is_err());
        assert!(s.execute("rig.info", &json!({"frame": 0})).is_err());
        assert_eq!(joint_count(&mut s), 3);
    }

    #[test]
    fn a_skeleton_takes_at_most_max_joints() {
        let mut s = session();
        for _ in 0..MAX_JOINTS {
            s.execute("rig.addJoint", &json!({"x": 0, "y": 0})).unwrap();
        }
        assert!(s.execute("rig.addJoint", &json!({"x": 0, "y": 0})).is_err());
        assert_eq!(joint_count(&mut s), MAX_JOINTS);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_posed_animation_exports() {
        let mut s = session();
        s.execute("anim.setFrames", &json!({"frames": 2})).unwrap();
        s.execute("anim.keyframe", &json!({"frame": 1})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 0, "y": -10, "width": 200, "height": 20})).unwrap();
        arm(&mut s);
        s.execute("rig.pose", &json!({"frame": 1, "angles": [0, 0, 0]})).unwrap();
        s.execute("rig.pose", &json!({"frame": 2, "joint": 1, "angle": 90})).unwrap();
        let dir = std::env::temp_dir().join(format!("7rrek-rig-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("arm.webm");
        let r = s.execute("anim.exportVideo", &json!({"path": path.to_string_lossy(), "scale": 0.25})).unwrap();
        assert_eq!(r["frames"], 2);
        assert_eq!(std::fs::read(&path).unwrap().get(..4), Some(&[0x1A, 0x45, 0xDF, 0xA3][..]), "EBML header");
        std::fs::remove_dir_all(&dir).ok();
    }
}
