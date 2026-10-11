//! The three test ranges (same as the browser range), wall collision push-out and shot
//! traces. Engine space: Unreal units, X forward, Y right, Z up.

use gears_core::testworld::{Wall, WallWorld};
use gears_core::{CoverHeight, Vec3};

/// Visual heights (uu) of the walls; slabs are WALL_T thick with cover on both faces.
pub const MID_H: f32 = 100.0;
pub const FULL_H: f32 = 230.0;
pub const WALL_T: f32 = 24.0;
/// Target cylinder (a pawn: Default__Pawn radius 34, height 156) and Locust Drone health.
pub const TARGET_R: f32 = 34.0;
pub const TARGET_H: f32 = 156.0;
pub const TARGET_HEALTH: f32 = 1000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelId {
    Lane,
    Edges,
    Yard,
}

pub struct Level {
    pub id: LevelId,
    pub start: Vec3,
    pub yaw: f32,
    /// the walls as authored (one face each), for drawing
    pub walls: Vec<Wall>,
    /// the collision / cover world (both faces of every slab)
    pub world: WallWorld,
    pub targets: Vec<(f32, f32)>,
}

fn w(ax: f32, ay: f32, bx: f32, by: f32, nx: f32, ny: f32, standing: bool) -> Wall {
    Wall {
        a: Vec3::new(ax, ay, 0.0),
        b: Vec3::new(bx, by, 0.0),
        normal: Vec3::new(nx, ny, 0.0).normalized(),
        height: if standing {
            CoverHeight::Standing
        } else {
            CoverHeight::Mid
        },
    }
}

impl Level {
    pub fn new(id: LevelId) -> Self {
        let (start, walls, targets) = match id {
            LevelId::Lane => (
                Vec3::new(-420.0, 0.0, 0.0),
                vec![
                    w(-100.0, -250.0, -100.0, 250.0, -1.0, 0.0, false),
                    w(250.0, -250.0, 250.0, 250.0, -1.0, 0.0, false),
                    w(600.0, -250.0, 600.0, 250.0, -1.0, 0.0, false),
                    w(250.0, 450.0, 600.0, 450.0, 0.0, -1.0, true),
                    w(-100.0, -450.0, 250.0, -450.0, 0.0, 1.0, false),
                ],
                vec![
                    (900.0, -150.0),
                    (900.0, 150.0),
                    (1150.0, 0.0),
                    (420.0, -650.0),
                ],
            ),
            LevelId::Edges => (
                Vec3::new(-400.0, -120.0, 0.0),
                vec![
                    w(-150.0, -200.0, -150.0, -40.0, -1.0, 0.0, false),
                    w(-150.0, 60.0, -150.0, 220.0, -1.0, 0.0, false),
                    w(150.0, -320.0, 150.0, -160.0, -1.0, 0.0, false),
                    w(150.0, -20.0, 150.0, 140.0, -1.0, 0.0, false),
                    w(450.0, -200.0, 450.0, -40.0, -1.0, 0.0, false),
                    w(450.0, 60.0, 450.0, 220.0, -1.0, 0.0, false),
                    w(750.0, -120.0, 750.0, 40.0, -1.0, 0.0, true),
                ],
                vec![(1000.0, -250.0), (1000.0, 180.0), (600.0, -480.0)],
            ),
            LevelId::Yard => (
                Vec3::new(-500.0, -300.0, 0.0),
                vec![
                    w(-200.0, -400.0, -200.0, -150.0, -1.0, 0.0, false),
                    w(0.0, 100.0, 260.0, 260.0, -0.524, 0.852, false),
                    w(250.0, -350.0, 450.0, -150.0, -0.707, 0.707, false),
                    w(500.0, 200.0, 500.0, 450.0, -1.0, 0.0, true),
                    w(-300.0, 350.0, -50.0, 350.0, 0.0, -1.0, false),
                    w(700.0, -250.0, 700.0, 0.0, -1.0, 0.0, false),
                ],
                vec![(950.0, -120.0), (850.0, 320.0), (300.0, -700.0)],
            ),
        };
        let world = WallWorld::two_sided(walls.clone(), WALL_T);
        Level {
            id,
            start,
            yaw: 0.0,
            walls,
            world,
            targets,
        }
    }

    pub fn wall_height(w: &Wall) -> f32 {
        if w.height == CoverHeight::Standing {
            FULL_H
        } else {
            MID_H
        }
    }

    /// Host-side push-out (the core doesn't sweep collision): keep the pawn 32 uu off
    /// every wall face.
    pub fn push_out(&self, pos: Vec3) -> Vec3 {
        let r = 32.0;
        let mut p = pos;
        for w in &self.world.walls {
            let s = w.b - w.a;
            let l2 = s.x * s.x + s.y * s.y;
            let t = (((p.x - w.a.x) * s.x + (p.y - w.a.y) * s.y) / l2).clamp(0.0, 1.0);
            let (px, py) = (w.a.x + s.x * t, w.a.y + s.y * t);
            let (dx, dy) = (p.x - px, p.y - py);
            let d = (dx * dx + dy * dy).sqrt();
            if d < r && d > 1e-6 {
                p = Vec3::new(px + dx / d * r, py + dy / d * r, p.z);
            }
        }
        p
    }

    /// Nearest hit along a ray: target cylinders (`alive`), wall faces up to their
    /// visual height, the floor. Returns (distance, target index).
    pub fn trace(
        &self,
        o: Vec3,
        d: Vec3,
        range: f32,
        targets: &[(f32, f32, bool)],
    ) -> (f32, Option<usize>) {
        let mut best = (range, None);
        for (i, &(tx, ty, alive)) in targets.iter().enumerate() {
            if !alive {
                continue;
            }
            let (fx, fy) = (o.x - tx, o.y - ty);
            let a = d.x * d.x + d.y * d.y;
            if a < 1e-9 {
                continue;
            }
            let b = fx * d.x + fy * d.y;
            let c = fx * fx + fy * fy - TARGET_R * TARGET_R;
            let disc = b * b - a * c;
            if disc < 0.0 {
                continue;
            }
            let t = (-b - disc.sqrt()) / a;
            let z = o.z + d.z * t;
            if t > 0.0 && t < best.0 && (0.0..=TARGET_H).contains(&z) {
                best = (t, Some(i));
            }
        }
        for w in &self.world.walls {
            let n = w.normal.normalized();
            let den = d.x * n.x + d.y * n.y;
            if den.abs() < 1e-9 {
                continue;
            }
            let t = ((w.a.x - o.x) * n.x + (w.a.y - o.y) * n.y) / den;
            if t <= 0.0 || t >= best.0 {
                continue;
            }
            let (px, py, z) = (o.x + d.x * t, o.y + d.y * t, o.z + d.z * t);
            let s = w.b - w.a;
            let l = (s.x * s.x + s.y * s.y).sqrt();
            let u = ((px - w.a.x) * s.x + (py - w.a.y) * s.y) / l;
            if (0.0..=l).contains(&u) && z >= 0.0 && z <= Self::wall_height(w) {
                best = (t, None);
            }
        }
        if d.z < 0.0 {
            let t = -o.z / d.z;
            if t > 0.0 && t < best.0 {
                best = (t, None);
            }
        }
        best
    }
}
