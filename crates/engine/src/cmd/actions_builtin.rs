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

    pub const fn rot_dy(frame: u32, rot: f64, dy: f64) -> Self {
        Self { frame, rot: Some(rot), dx: None, dy: Some(dy), drawing: None }
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

// 1. walk: 24 frames cycle (right and left step)
const WALK_PARTS: &[RawPart] = &[
    RawPart {
        part: "arm_upper_L",
        keys: &[
            RawKey::rot(0, 0.0),
            RawKey::rot(5, 25.0),
            RawKey::rot(6, 25.0),
            RawKey::rot(11, 0.0),
            RawKey::rot(12, 0.0),
            RawKey::rot(17, -25.0),
            RawKey::rot(18, -25.0),
            RawKey::rot(23, 0.0),
        ],
    },
    RawPart {
        part: "arm_lower_L",
        keys: &[
            RawKey::rot(0, 0.0),
            RawKey::rot(5, 15.0),
            RawKey::rot(6, 15.0),
            RawKey::rot(11, 0.0),
            RawKey::rot(12, 0.0),
            RawKey::rot(17, 5.0),
            RawKey::rot(18, 5.0),
            RawKey::rot(23, 0.0),
        ],
    },
    RawPart {
        part: "arm_upper_R",
        keys: &[
            RawKey::rot(0, 0.0),
            RawKey::rot(5, -25.0),
            RawKey::rot(6, -25.0),
            RawKey::rot(11, 0.0),
            RawKey::rot(12, 0.0),
            RawKey::rot(17, 25.0),
            RawKey::rot(18, 25.0),
            RawKey::rot(23, 0.0),
        ],
    },
    RawPart {
        part: "arm_lower_R",
        keys: &[
            RawKey::rot(0, 0.0),
            RawKey::rot(5, 5.0),
            RawKey::rot(6, 5.0),
            RawKey::rot(11, 0.0),
            RawKey::rot(12, 0.0),
            RawKey::rot(17, 15.0),
            RawKey::rot(18, 15.0),
            RawKey::rot(23, 0.0),
        ],
    },
    RawPart {
        part: "leg_upper_L",
        keys: &[
            RawKey::rot(0, 0.0),
            RawKey::rot(5, -25.0),
            RawKey::rot(6, -25.0),
            RawKey::rot(11, 0.0),
            RawKey::rot(12, 0.0),
            RawKey::rot(17, 25.0),
            RawKey::rot(18, 25.0),
            RawKey::rot(23, 0.0),
        ],
    },
    RawPart {
        part: "leg_lower_L",
        keys: &[
            RawKey::rot(0, 0.0),
            RawKey::rot(5, 15.0),
            RawKey::rot(6, 15.0),
            RawKey::rot(11, 0.0),
            RawKey::rot(12, 0.0),
            RawKey::rot(17, 0.0),
            RawKey::rot(18, 0.0),
            RawKey::rot(23, 0.0),
        ],
    },
    RawPart {
        part: "foot_L",
        keys: &[
            RawKey::rot(0, 0.0),
            RawKey::rot(5, -10.0),
            RawKey::rot(6, -10.0),
            RawKey::rot(11, 0.0),
            RawKey::rot(12, 0.0),
            RawKey::rot(17, 10.0),
            RawKey::rot(18, 10.0),
            RawKey::rot(23, 0.0),
        ],
    },
    RawPart {
        part: "leg_upper_R",
        keys: &[
            RawKey::rot(0, 0.0),
            RawKey::rot(5, 25.0),
            RawKey::rot(6, 25.0),
            RawKey::rot(11, 0.0),
            RawKey::rot(12, 0.0),
            RawKey::rot(17, -25.0),
            RawKey::rot(18, -25.0),
            RawKey::rot(23, 0.0),
        ],
    },
    RawPart {
        part: "leg_lower_R",
        keys: &[
            RawKey::rot(0, 0.0),
            RawKey::rot(5, 0.0),
            RawKey::rot(6, 0.0),
            RawKey::rot(11, 0.0),
            RawKey::rot(12, 0.0),
            RawKey::rot(17, 15.0),
            RawKey::rot(18, 15.0),
            RawKey::rot(23, 0.0),
        ],
    },
    RawPart {
        part: "foot_R",
        keys: &[
            RawKey::rot(0, 0.0),
            RawKey::rot(5, 10.0),
            RawKey::rot(6, 10.0),
            RawKey::rot(11, 0.0),
            RawKey::rot(12, 0.0),
            RawKey::rot(17, -10.0),
            RawKey::rot(18, -10.0),
            RawKey::rot(23, 0.0),
        ],
    },
    RawPart {
        part: "hips",
        keys: &[
            RawKey::dy(0, 0.0),
            RawKey::dy(5, -2.0),
            RawKey::dy(6, -2.0),
            RawKey::dy(11, 0.0),
            RawKey::dy(12, 0.0),
            RawKey::dy(17, -2.0),
            RawKey::dy(18, -2.0),
            RawKey::dy(23, 0.0),
        ],
    },
    RawPart {
        part: "torso",
        keys: &[
            RawKey::rot_dy(0, 0.0, 0.0),
            RawKey::rot_dy(5, -1.0, -2.0),
            RawKey::rot_dy(6, -1.0, -2.0),
            RawKey::rot_dy(11, 0.0, 0.0),
            RawKey::rot_dy(12, 0.0, 0.0),
            RawKey::rot_dy(17, 1.0, -2.0),
            RawKey::rot_dy(18, 1.0, -2.0),
            RawKey::rot_dy(23, 0.0, 0.0),
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

// 5. idle: subtle breath on torso
const IDLE_PARTS: &[RawPart] = &[RawPart {
    part: "torso",
    keys: &[
        RawKey::rot_dy(0, 0.0, 0.0),
        RawKey::rot_dy(6, 1.5, -1.5),
        RawKey::rot_dy(12, 0.0, 0.0),
        RawKey::rot_dy(18, -1.5, 1.5),
        RawKey::rot_dy(23, 0.0, 0.0),
    ],
}];

pub const BUILTIN_ACTIONS: &[RawAction] = &[
    RawAction { name: "walk", length: 24, parts: WALK_PARTS },
    RawAction { name: "blink", length: 6, parts: BLINK_PARTS },
    RawAction { name: "turn_head", length: 12, parts: TURN_HEAD_PARTS },
    RawAction { name: "point", length: 12, parts: POINT_PARTS },
    RawAction { name: "idle", length: 24, parts: IDLE_PARTS },
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
