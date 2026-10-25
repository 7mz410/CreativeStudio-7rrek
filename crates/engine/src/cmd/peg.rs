//! 7rrek (Creative Studio): pegs. A peg is a keyed transform over several layers (timeline rows),
//! like Toon Boom Harmony's pegs: moving it carries every row it holds. Pegs nest (a peg's parent
//! moves it too), and their keys ease like the camera's. The frame shown moves each held row by
//! its peg's transform, after the row's bones bent it. Pegs live in the document's preserved extra
//! data (`"pegs"`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use vectorcraft_doc::{Document, NodeId};
use vectorcraft_geom::{Affine, Vec2};

use super::anim::{eased, valid_ease};

const KEY: &str = "pegs";
/// Most pegs a document takes.
pub const MAX_PEGS: usize = 256;

/// A peg key: how far the peg moved (document points), turned about its pivot (degrees, the
/// canvas's positive sense) and scaled about it, easing toward the next key.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct PegKey {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub angle: f64,
    #[serde(default = "one")]
    pub sx: f64,
    #[serde(default = "one")]
    pub sy: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ease: Option<[f64; 4]>,
}

fn one() -> f64 {
    1.0
}

impl Default for PegKey {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0, angle: 0.0, sx: 1.0, sy: 1.0, ease: None }
    }
}

impl PegKey {
    fn valid(&self) -> bool {
        [self.x, self.y, self.angle, self.sx, self.sy].iter().all(|v| v.is_finite())
            && [self.sx, self.sy].iter().all(|s| (0.001..=1000.0).contains(&s.abs()))
            && self.ease.as_ref().is_none_or(valid_ease)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Peg {
    pub name: String,
    /// The peg this one hangs from (an earlier index), whose motion it adds to its own.
    #[serde(default)]
    pub parent: Option<usize>,
    /// The point it turns and scales about (document points).
    #[serde(default)]
    pub pivot: [f64; 2],
    /// The layers (timeline rows) it carries, by id.
    #[serde(default)]
    pub rows: Vec<u64>,
    /// Frame → key.
    #[serde(default)]
    pub keys: BTreeMap<u32, PegKey>,
}

/// Every peg in a document.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Pegs {
    pub pegs: Vec<Peg>,
}

impl Pegs {
    /// The document's pegs; none if any is broken (a parent after its child, a bad number).
    pub fn of(doc: &Document) -> Self {
        let p: Pegs = doc.unknown.get(KEY).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
        if p.valid() { p } else { Pegs::default() }
    }

    pub fn store(&self, doc: &mut Document) {
        if self.pegs.is_empty() {
            doc.unknown.remove(KEY);
        } else if let Ok(v) = serde_json::to_value(self) {
            doc.unknown.insert(KEY.into(), v);
        }
    }

    pub fn valid(&self) -> bool {
        self.pegs.len() <= MAX_PEGS
            && self
                .pegs
                .iter()
                .enumerate()
                .all(|(i, p)| p.parent.is_none_or(|q| q < i) && p.pivot.iter().all(|v| v.is_finite()) && p.keys.values().all(PegKey::valid))
    }

    /// Each peg's transform at `frame`, its parents' included.
    pub fn matrices(&self, frame: u32) -> Vec<Affine> {
        let mut m: Vec<Affine> = Vec::with_capacity(self.pegs.len());
        for p in &self.pegs {
            let parent = p.parent.and_then(|q| m.get(q).copied()).unwrap_or(Affine::IDENTITY);
            m.push(parent * p.local(frame));
        }
        m
    }

    /// Move every held row of `doc` by its peg at `frame`. A row held by two pegs goes with the first.
    pub fn apply(&self, doc: &mut Document, frame: u32) {
        let m = self.matrices(frame);
        let mut done = Vec::new();
        for (p, a) in self.pegs.iter().zip(&m) {
            for row in &p.rows {
                if done.contains(row) || *a == Affine::IDENTITY {
                    continue;
                }
                done.push(*row);
                if let Some(n) = doc.node_mut(NodeId(*row)) {
                    n.transform(*a, true);
                }
            }
        }
    }
}

impl Peg {
    /// The key at `frame`: held before the first and after the last, eased in between, turning the
    /// short way round and scaling geometrically. No keys: still.
    pub fn key_at(&self, frame: u32) -> PegKey {
        let before = self.keys.range(..=frame).next_back();
        let after = self.keys.range(frame..).next();
        match (before, after) {
            (Some((fa, a)), Some((fb, b))) if fb > fa => {
                let t = eased(a.ease.as_ref(), (frame - fa) as f64 / (fb - fa) as f64);
                let mix = |x: f64, y: f64| x + (y - x) * t;
                let geo = |x: f64, y: f64| if x * y > 0.0 { x * (y / x).powf(t) } else { mix(x, y) };
                let turn = (b.angle - a.angle + 180.0).rem_euclid(360.0) - 180.0;
                PegKey { x: mix(a.x, b.x), y: mix(a.y, b.y), angle: a.angle + turn * t, sx: geo(a.sx, b.sx), sy: geo(a.sy, b.sy), ease: None }
            }
            (Some((_, k)), _) | (None, Some((_, k))) => *k,
            (None, None) => PegKey::default(),
        }
    }

    fn local(&self, frame: u32) -> Affine {
        let k = self.key_at(frame);
        let at = Vec2::new(self.pivot[0], self.pivot[1]);
        Affine::translate(at + Vec2::new(k.x, k.y))
            * Affine::rotate(k.angle.to_radians())
            * Affine::scale_non_uniform(k.sx, k.sy)
            * Affine::translate(-at)
    }
}

/// Move the held rows of `doc` by their pegs at `frame`.
pub fn apply(doc: &mut Document, frame: u32) {
    Pegs::of(doc).apply(doc, frame);
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_geom::Point;

    fn peg(parent: Option<usize>, keys: &[(u32, PegKey)]) -> Peg {
        Peg { name: "p".into(), parent, pivot: [0.0, 0.0], rows: vec![], keys: keys.iter().copied().collect() }
    }

    #[test]
    fn a_peg_moves_and_eases_between_keys() {
        let k = |x| PegKey { x, ..PegKey::default() };
        let p = peg(None, &[(1, k(0.0)), (11, k(100.0))]);
        assert_eq!(p.key_at(6).x, 50.0);
        assert_eq!(p.key_at(30).x, 100.0);
        let pegs = Pegs { pegs: vec![p] };
        assert_eq!(pegs.matrices(11)[0] * Point::new(5.0, 5.0), Point::new(105.0, 5.0));
    }

    #[test]
    fn nested_pegs_compose() {
        let mv = PegKey { x: 100.0, ..PegKey::default() };
        let turn = PegKey { angle: 90.0, ..PegKey::default() };
        let mut child = peg(Some(0), &[(1, turn)]);
        child.pivot = [10.0, 0.0];
        let pegs = Pegs { pegs: vec![peg(None, &[(1, mv)]), child] };
        let q = pegs.matrices(1)[1] * Point::new(20.0, 0.0);
        assert!((q - Point::new(110.0, 10.0)).hypot() < 1e-9, "{q:?}");
    }

    #[test]
    fn broken_pegs_are_dropped() {
        let bad = Pegs { pegs: vec![peg(Some(0), &[])] };
        assert!(!bad.valid());
        let nan = Pegs { pegs: vec![peg(None, &[(1, PegKey { sx: f64::NAN, ..PegKey::default() })])] };
        assert!(!nan.valid());
        let zero = Pegs { pegs: vec![peg(None, &[(1, PegKey { sy: 0.0, ..PegKey::default() })])] };
        assert!(!zero.valid());
    }
}
