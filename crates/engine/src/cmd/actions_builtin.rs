//! 7rrek (Creative Studio): starter action templates as pure data (`walk`, `blink`, `turn_head`, `point`, `idle`).
//! Keys are keyed by contract part names relative to rest pose.

use std::collections::BTreeMap;

use super::action_tpl::{Action, PartKey, PartTrack, Tracks};

#[derive(Clone, Copy)]
pub struct RawKey {
    pub frame: u32,
    pub rot: Option<f64>,
    pub dx: Option<f64>,
    pub dy: Option<f64>,
    pub drawing: Option<&'static str>,
}

impl RawKey {
    pub const fn rot(frame: u32, rot: f64) -> Self {
        Self { frame, rot: Some(rot), dx: None, dy: None, drawing: None }
    }

    pub const fn dy(frame: u32, dy: f64) -> Self {
        Self { frame, rot: None, dx: None, dy: Some(dy), drawing: None }
    }

    pub const fn drawing(frame: u32, drawing: &'static str) -> Self {
        Self { frame, rot: None, dx: None, dy: None, drawing: Some(drawing) }
    }
}

pub struct RawPart {
    pub part: &'static str,
    pub keys: &'static [RawKey],
}

pub struct RawAction {
    pub name: &'static str,
    pub length: u32,
    pub parts: &'static [RawPart],
}

// 1. walk: a 24-frame cycle for a character facing right in the side view, two steps of 12
// frames, each keyed at contact, down, passing and up. Angles are degrees from the rest pose; a
// positive turn is clockwise on screen, so a leg or arm swings forward with a negative one. Knees
// bend with a positive shin turn, the hips drop on the down pose and rise on the up pose.
// Checked by eye on a traced character (plans/refs/char1.rsm, side view).
const WALK_PARTS: &[RawPart] = &[
    RawPart {
        part: "leg_upper_L",
        keys: &[
            RawKey::rot(0, -22.0),
            RawKey::rot(3, -15.0),
            RawKey::rot(6, -2.0),
            RawKey::rot(9, 10.0),
            RawKey::rot(12, 18.0),
            RawKey::rot(15, 18.0),
            RawKey::rot(18, -10.0),
            RawKey::rot(21, -22.0),
        ],
    },
    RawPart {
        part: "leg_lower_L",
        keys: &[
            RawKey::rot(0, 4.0),
            RawKey::rot(3, 12.0),
            RawKey::rot(6, 4.0),
            RawKey::rot(9, 2.0),
            RawKey::rot(12, 8.0),
            RawKey::rot(15, 22.0),
            RawKey::rot(18, 40.0),
            RawKey::rot(21, 18.0),
        ],
    },
    RawPart {
        part: "foot_L",
        keys: &[
            RawKey::rot(0, -8.0),
            RawKey::rot(3, 0.0),
            RawKey::rot(6, 0.0),
            RawKey::rot(9, 6.0),
            RawKey::rot(12, 10.0),
            RawKey::rot(15, 14.0),
            RawKey::rot(18, 8.0),
            RawKey::rot(21, -4.0),
        ],
    },
    RawPart {
        part: "leg_upper_R",
        keys: &[
            RawKey::rot(0, 18.0),
            RawKey::rot(3, 18.0),
            RawKey::rot(6, -10.0),
            RawKey::rot(9, -22.0),
            RawKey::rot(12, -22.0),
            RawKey::rot(15, -15.0),
            RawKey::rot(18, -2.0),
            RawKey::rot(21, 10.0),
        ],
    },
    RawPart {
        part: "leg_lower_R",
        keys: &[
            RawKey::rot(0, 8.0),
            RawKey::rot(3, 22.0),
            RawKey::rot(6, 40.0),
            RawKey::rot(9, 18.0),
            RawKey::rot(12, 4.0),
            RawKey::rot(15, 12.0),
            RawKey::rot(18, 4.0),
            RawKey::rot(21, 2.0),
        ],
    },
    RawPart {
        part: "foot_R",
        keys: &[
            RawKey::rot(0, 10.0),
            RawKey::rot(3, 14.0),
            RawKey::rot(6, 8.0),
            RawKey::rot(9, -4.0),
            RawKey::rot(12, -8.0),
            RawKey::rot(15, 0.0),
            RawKey::rot(18, 0.0),
            RawKey::rot(21, 6.0),
        ],
    },
    RawPart {
        part: "arm_upper_L",
        keys: &[
            RawKey::rot(0, 16.0),
            RawKey::rot(3, 10.0),
            RawKey::rot(6, 0.0),
            RawKey::rot(9, -10.0),
            RawKey::rot(12, -16.0),
            RawKey::rot(15, -10.0),
            RawKey::rot(18, 0.0),
            RawKey::rot(21, 10.0),
        ],
    },
    RawPart {
        part: "arm_lower_L",
        keys: &[
            RawKey::rot(0, -5.0),
            RawKey::rot(3, -6.0),
            RawKey::rot(6, -10.0),
            RawKey::rot(9, -14.0),
            RawKey::rot(12, -18.0),
            RawKey::rot(15, -14.0),
            RawKey::rot(18, -10.0),
            RawKey::rot(21, -6.0),
        ],
    },
    RawPart {
        part: "arm_upper_R",
        keys: &[
            RawKey::rot(0, -16.0),
            RawKey::rot(3, -10.0),
            RawKey::rot(6, 0.0),
            RawKey::rot(9, 10.0),
            RawKey::rot(12, 16.0),
            RawKey::rot(15, 10.0),
            RawKey::rot(18, 0.0),
            RawKey::rot(21, -10.0),
        ],
    },
    RawPart {
        part: "arm_lower_R",
        keys: &[
            RawKey::rot(0, -18.0),
            RawKey::rot(3, -14.0),
            RawKey::rot(6, -10.0),
            RawKey::rot(9, -6.0),
            RawKey::rot(12, -5.0),
            RawKey::rot(15, -6.0),
            RawKey::rot(18, -10.0),
            RawKey::rot(21, -14.0),
        ],
    },
    RawPart {
        part: "torso",
        keys: &[
            RawKey::rot(0, 3.0),
            RawKey::rot(3, 3.0),
            RawKey::rot(6, 4.0),
            RawKey::rot(9, 3.0),
            RawKey::rot(12, 3.0),
            RawKey::rot(15, 3.0),
            RawKey::rot(18, 4.0),
            RawKey::rot(21, 3.0),
        ],
    },
    RawPart {
        part: "head",
        keys: &[
            RawKey::rot(0, -2.0),
            RawKey::rot(3, -2.0),
            RawKey::rot(6, -3.0),
            RawKey::rot(9, -2.0),
            RawKey::rot(12, -2.0),
            RawKey::rot(15, -2.0),
            RawKey::rot(18, -3.0),
            RawKey::rot(21, -2.0),
        ],
    },
    RawPart {
        part: "hips",
        keys: &[
            RawKey::dy(0, 0.0),
            RawKey::dy(3, 6.0),
            RawKey::dy(6, 0.0),
            RawKey::dy(9, -4.0),
            RawKey::dy(12, 0.0),
            RawKey::dy(15, 6.0),
            RawKey::dy(18, 0.0),
            RawKey::dy(21, -4.0),
        ],
    },
];

// 2. blink: 6 frames with eye_*.closed
const BLINK_PARTS: &[RawPart] = &[
    RawPart {
        part: "eye_L",
        keys: &[
            RawKey::drawing(0, "open"),
            RawKey::drawing(1, "half"),
            RawKey::drawing(2, "closed"),
            RawKey::drawing(3, "closed"),
            RawKey::drawing(4, "half"),
            RawKey::drawing(5, "open"),
        ],
    },
    RawPart {
        part: "eye_R",
        keys: &[
            RawKey::drawing(0, "open"),
            RawKey::drawing(1, "half"),
            RawKey::drawing(2, "closed"),
            RawKey::drawing(3, "closed"),
            RawKey::drawing(4, "half"),
            RawKey::drawing(5, "open"),
        ],
    },
];

// 3. turn_head: head rotates 20°
const TURN_HEAD_PARTS: &[RawPart] = &[RawPart { part: "head", keys: &[RawKey::rot(0, 0.0), RawKey::rot(6, 20.0), RawKey::rot(11, 20.0)] }];

// 4. point: arm_upper_R + arm_lower_R
const POINT_PARTS: &[RawPart] = &[
    RawPart { part: "arm_upper_R", keys: &[RawKey::rot(0, 0.0), RawKey::rot(6, -45.0), RawKey::rot(11, -45.0)] },
    RawPart { part: "arm_lower_R", keys: &[RawKey::rot(0, 0.0), RawKey::rot(6, 30.0), RawKey::rot(11, 30.0)] },
    RawPart { part: "hand_R", keys: &[RawKey::drawing(0, "open"), RawKey::drawing(6, "point"), RawKey::drawing(11, "point")] },
];

// 5. idle: a slow breath over 48 frames: the hips settle, the torso and head sway a little, the
// arms hang and drift.
const IDLE_PARTS: &[RawPart] = &[
    RawPart { part: "hips", keys: &[RawKey::dy(0, 0.0), RawKey::dy(24, 2.0), RawKey::dy(47, 0.0)] },
    RawPart { part: "torso", keys: &[RawKey::rot(0, 0.0), RawKey::rot(24, 1.5), RawKey::rot(47, 0.0)] },
    RawPart { part: "head", keys: &[RawKey::rot(0, 0.0), RawKey::rot(18, -2.0), RawKey::rot(36, 1.0), RawKey::rot(47, 0.0)] },
    RawPart { part: "arm_upper_L", keys: &[RawKey::rot(0, 0.0), RawKey::rot(24, 2.0), RawKey::rot(47, 0.0)] },
    RawPart { part: "arm_upper_R", keys: &[RawKey::rot(0, 0.0), RawKey::rot(24, -2.0), RawKey::rot(47, 0.0)] },
    RawPart { part: "arm_lower_L", keys: &[RawKey::rot(0, 0.0), RawKey::rot(24, -3.0), RawKey::rot(47, 0.0)] },
    RawPart { part: "arm_lower_R", keys: &[RawKey::rot(0, 0.0), RawKey::rot(24, -3.0), RawKey::rot(47, 0.0)] },
];

pub const BUILTIN_ACTIONS: &[RawAction] = &[
    RawAction { name: "walk", length: 24, parts: WALK_PARTS },
    RawAction { name: "blink", length: 6, parts: BLINK_PARTS },
    RawAction { name: "turn_head", length: 12, parts: TURN_HEAD_PARTS },
    RawAction { name: "point", length: 12, parts: POINT_PARTS },
    RawAction { name: "idle", length: 48, parts: IDLE_PARTS },
];

pub fn build_action(raw: &RawAction) -> Action {
    let mut parts = BTreeMap::new();
    for p in raw.parts {
        let mut keys = BTreeMap::new();
        for k in p.keys {
            keys.insert(k.frame, PartKey { rot: k.rot, dx: k.dx, dy: k.dy, drawing: k.drawing.map(str::to_string), ease: None });
        }
        parts.insert(p.part.to_string(), PartTrack::new(keys));
    }
    Action { name: raw.name.to_string(), length: raw.length, tracks: Tracks::default(), parts }
}

pub fn builtin_actions() -> Vec<Action> {
    BUILTIN_ACTIONS.iter().map(build_action).collect()
}

pub fn builtin_action(name: &str) -> Option<Action> {
    BUILTIN_ACTIONS.iter().find(|a| a.name == name).map(build_action)
}
