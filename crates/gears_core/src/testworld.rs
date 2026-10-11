//! A tiny world made of straight cover walls, for tests, the CLI and the sandbox.

use crate::math::Vec3;
use crate::world::{CoverHeight, CoverSpot, World};

/// A straight cover wall segment from `a` to `b` (on the ground plane). The cover
/// side is the side `normal` points to.
#[derive(Clone, Copy, Debug)]
pub struct Wall {
    pub a: Vec3,
    pub b: Vec3,
    pub normal: Vec3,
    pub height: CoverHeight,
}

pub struct WallWorld {
    pub walls: Vec<Wall>,
    /// Distance the character stands off the wall when in cover.
    pub standoff: f32,
}

impl WallWorld {
    pub fn new(walls: Vec<Wall>) -> Self {
        Self {
            walls,
            standoff: 34.0,
        }
    }

    /// Every wall is a slab `thickness` thick with cover on both faces: each wall
    /// gets a back face `thickness` behind it, facing the other way.
    pub fn two_sided(walls: Vec<Wall>, thickness: f32) -> Self {
        let mut all = walls.clone();
        for w in &walls {
            let n = w.normal.normalized();
            all.push(Wall {
                a: w.b - n * thickness,
                b: w.a - n * thickness,
                normal: -n,
                height: w.height,
            });
        }
        Self::new(all)
    }

    /// Two parallel walls facing each other across `gap` uu, each `length` long,
    /// centred on the X axis — the classic wallbounce lane.
    pub fn lane(gap: f32, length: f32) -> Self {
        let h = length / 2.0;
        Self::new(vec![
            Wall {
                a: Vec3::new(gap / 2.0, -h, 0.0),
                b: Vec3::new(gap / 2.0, h, 0.0),
                normal: Vec3::new(-1.0, 0.0, 0.0),
                height: CoverHeight::Mid,
            },
            Wall {
                a: Vec3::new(-gap / 2.0, -h, 0.0),
                b: Vec3::new(-gap / 2.0, h, 0.0),
                normal: Vec3::new(1.0, 0.0, 0.0),
                height: CoverHeight::Mid,
            },
        ])
    }
}

impl WallWorld {
    /// A cover spot at `loc` (already off the wall by the standoff) on wall `w`.
    fn spot_on(&self, w: &Wall, loc: Vec3) -> CoverSpot {
        let n = w.normal.normalized();
        let tmp = CoverSpot {
            location: loc,
            normal: n,
            height: w.height,
            dist_to_left_edge: 0.0,
            dist_to_right_edge: 0.0,
            can_slip_left: true,
            can_slip_right: true,
        };
        // Edge distances along the character's right when facing the wall.
        let r = tmp.right();
        let to_a = (w.a - loc).dot(r);
        let to_b = (w.b - loc).dot(r);
        let (right_edge, left_edge) = if to_a > to_b {
            (to_a, -to_b)
        } else {
            (to_b, -to_a)
        };
        CoverSpot {
            dist_to_left_edge: left_edge.max(0.0),
            dist_to_right_edge: right_edge.max(0.0),
            ..tmp
        }
    }
}

impl World for WallWorld {
    fn find_cover(&self, from: Vec3, dir: Vec3, max_dist: f32) -> Option<CoverSpot> {
        let mut best: Option<(f32, CoverSpot)> = None;
        for w in &self.walls {
            let n = w.normal.normalized();
            // Only cover we're on the right side of and moving toward.
            let side = (from - w.a).dot(n);
            if side <= 0.0 || dir.dot(-n) <= 0.1 {
                continue;
            }
            // Ray from `from` along `dir` to the plane offset by standoff.
            let dist_to_plane = side - self.standoff;
            let along = dir.dot(-n);
            // Already closer than the standoff (pressed against the wall): t = 0.
            let t = (dist_to_plane / along).max(0.0);
            if t > max_dist {
                continue;
            }
            let hit = from + dir * t;
            let seg = w.b - w.a;
            let len = seg.length();
            let u = (hit - w.a).dot(seg.normalized());
            if u < 0.0 || u > len {
                continue;
            }
            let loc = w.a + seg.normalized() * u + n * self.standoff;
            // Gears finds cover with traces: never through another wall.
            if !self.is_clear(from, loc, 0.0) {
                continue;
            }
            let spot = self.spot_on(w, Vec3::new(loc.x, loc.y, from.z));
            if best.map(|(bt, _)| t < bt).unwrap_or(true) {
                best = Some((t, spot));
            }
        }
        best.map(|(_, s)| s)
    }

    fn is_clear(&self, from: Vec3, to: Vec3, radius: f32) -> bool {
        // Blocked if the segment crosses any wall line (within its length).
        for w in &self.walls {
            let n = w.normal.normalized();
            let s0 = (from - w.a).dot(n);
            let s1 = (to - w.a).dot(n);
            if (s0 > radius) == (s1 > radius) && (s0 < -radius) == (s1 < -radius) {
                continue;
            }
            let t = s0 / (s0 - s1);
            let p = from + (to - from) * t;
            let seg = w.b - w.a;
            let u = (p - w.a).dot(seg.normalized());
            if u >= 0.0 && u <= seg.length() {
                return false;
            }
        }
        true
    }

    fn find_swat_target(
        &self,
        spot: &CoverSpot,
        edge: Vec3,
        side: Vec3,
        max_dist: f32,
    ) -> Option<CoverSpot> {
        // The next wall in line past the gap: same facing, wholly beyond the edge,
        // roughly on the same line, nearest end within reach, gap clear.
        let mut best: Option<(f32, CoverSpot)> = None;
        for w in &self.walls {
            let n = w.normal.normalized();
            if n.dot(spot.normal) < 0.7 {
                continue;
            }
            let (pa, pb) = ((w.a - edge).dot(side), (w.b - edge).dot(side));
            if pa.min(pb) <= 1.0 {
                continue;
            }
            // `edge` is the character's spot at the end of the cover (standoff off the
            // wall): measure the gap along the wall, the offset across it.
            let (near, along) = if pa < pb { (w.a, pa) } else { (w.b, pb) };
            let across = (near - edge).dot(spot.normal) + self.standoff;
            if along > max_dist || across.abs() > 64.0 {
                continue;
            }
            let loc = near + n * self.standoff;
            let loc = Vec3::new(loc.x, loc.y, spot.location.z);
            if !self.is_clear(edge, loc, 0.0) {
                continue;
            }
            let d = along;
            if best.map(|(bd, _)| d < bd).unwrap_or(true) {
                best = Some((d, self.spot_on(w, loc)));
            }
        }
        best.map(|(_, s)| s)
    }

    fn mantle_distance(&self, from: Vec3, dir: Vec3, max_dist: f32, radius: f32) -> Option<f32> {
        // Every wall face the ray crosses within reach, nearest first.
        let to = from + dir * max_dist;
        let mut hits: Vec<(f32, &Wall)> = Vec::new();
        for w in &self.walls {
            let n = w.normal.normalized();
            let s0 = (from - w.a).dot(n);
            let s1 = (to - w.a).dot(n);
            if (s0 > 0.0) == (s1 > 0.0) {
                continue;
            }
            let t = s0 / (s0 - s1);
            let p = from + (to - from) * t;
            let seg = w.b - w.a;
            let u = (p - w.a).dot(seg.normalized());
            if u >= 0.0 && u <= seg.length() {
                hits.push((t * max_dist, w));
            }
        }
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        // The first face must be waist-high cover we face; the far side is the slab's
        // back face (facing along `dir`) if it has one, else the same thin wall.
        let (first, w0) = *hits.first()?;
        if w0.height != CoverHeight::Mid || w0.normal.dot(dir) >= 0.0 {
            return None;
        }
        let far = hits
            .get(1)
            .filter(|(_, w)| w.height == CoverHeight::Mid && w.normal.dot(dir) > 0.0)
            .map(|h| h.0)
            .unwrap_or(first);
        let dist = far + radius;
        (dist <= max_dist).then_some(dist)
    }
}
