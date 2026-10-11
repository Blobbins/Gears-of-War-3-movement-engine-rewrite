//! The Gears 3 third-person camera (numbers: docs/GEARS_VALUES.md → Camera), same as the
//! browser range: per-mode FOV and shoulder offsets blended by pitch, per-axis follow lag,
//! look rate with yaw acceleration, roadie swing-in, cover yaw limit, aim and pop-up.

use crate::level::{Level, FULL_H, MID_H, WALL_T};
use gears_core::{Character, CoverHeight, MoveKind, Vec3};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

const URAD: f32 = TAU / 65536.0;
pub const PAWN_HALF: f32 = 78.0;
pub const EYE: f32 = 60.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mode {
    pub name: &'static str,
    pub fov: f32,
    pub high: [f32; 3],
    pub mid: [f32; 3],
    pub low: [f32; 3],
    pub follow: [f32; 3],
    pub blend: f32,
}

pub const TARGET: Mode = Mode {
    name: "GameplayCam_Targeting",
    fov: 50.0,
    high: [-40.0, 40.0, 24.0],
    mid: [-70.0, 40.0, 6.0],
    low: [-125.0, 45.0, 20.0],
    follow: [1e3, 1e3, 8.0],
    blend: 0.067,
};
pub const DEFAULT: Mode = Mode {
    name: "GameplayCam_Default",
    fov: 80.0,
    high: [-128.0, 56.0, 40.0],
    mid: [-160.0, 48.0, 16.0],
    low: [-160.0, 48.0, 56.0],
    follow: [5.0, 20.0, 20.0],
    blend: 0.15,
};
pub const ROADIE: Mode = Mode {
    name: "GameplayCam_RoadieRun",
    fov: 105.0,
    high: [0.0, 56.0, 50.0],
    mid: [-88.0, 48.0, -40.0],
    low: [-88.0, 56.0, -40.0],
    follow: [8.0, 20.0, 20.0],
    blend: 0.15,
};
pub const COVER: Mode = Mode {
    name: "GameplayCam_Cover",
    fov: 95.0,
    high: [-128.0, 0.0, 24.0],
    mid: [-160.0, 0.0, 0.0],
    low: [-160.0, 0.0, 32.0],
    follow: [12.0, 12.0, 12.0],
    blend: 0.15,
};
const EVADE_ADJ: [f32; 3] = [0.0, 0.0, -28.0];
const COVER_POSTURE_MID: [f32; 3] = [-32.0, 40.0, -12.0];
const COVER_POSTURE_STD: [f32; 3] = [0.0, 40.0, 0.0];
const COVER_EDGE_Y: f32 = 84.0;
const LOOK_YAW: f32 = 100.0 * 300.0;
const LOOK_PITCH: f32 = 100.0 * 250.0;
const SENS: f32 = 0.8;
const PITCH_RATIO: f32 = 0.75;
const YAW_ACCEL_THRESHOLD: f32 = 0.5;
const YAW_ACCEL_RAMP: f32 = 1.3;
const YAW_ACCEL_MAX: f32 = 0.6;
const YAW_ACCEL_MULT: f32 = 2.5;
const ROADIE_CAM_TURN: f32 = 0.15;
const PITCH_LIMIT: f32 = 60.0 * PI / 180.0;
/// mouse: radians per pixel at FOV 90 (the browser range's feel)
pub const MOUSE_RAD_PER_PX: f32 = 0.0024;

#[derive(Clone, Debug)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub origin: Vec3,
    pub offset: [f32; 3],
    /// horizontal FOV, degrees (UE3 FOVAngle)
    pub fov: f32,
    accel: f32,
    last_look: f32,
    pub mode: Mode,
    /// final camera position (engine space)
    pub pos: Vec3,
}

fn angle_diff(a: f32, b: f32) -> f32 {
    let mut d = (b - a) % TAU;
    if d > PI {
        d -= TAU
    } else if d < -PI {
        d += TAU
    }
    d
}
fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

impl Camera {
    pub fn new(ch: &Character, yaw: f32) -> Self {
        Camera {
            yaw,
            pitch: -0.12,
            origin: Vec3::new(ch.pos.x, ch.pos.y, PAWN_HALF + EYE),
            offset: DEFAULT.mid,
            fov: 80.0,
            accel: 0.0,
            last_look: 0.0,
            mode: DEFAULT,
            pos: ch.pos,
        }
    }

    pub fn forward(&self) -> Vec3 {
        let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
        Vec3::new(cp * self.yaw.cos(), cp * self.yaw.sin(), sp)
    }

    /// One fixed step. `look` = right stick (x, y), `mouse` = pixels moved since last step.
    pub fn step(
        &mut self,
        ch: &Character,
        aiming: bool,
        look: (f32, f32),
        mouse: (f32, f32),
        vault_up: f32,
        dt: f32,
    ) {
        let roadie = ch.active_move() == Some(MoveKind::RoadieRun);
        let cover = ch.cover_spot();
        let mode = if aiming {
            TARGET
        } else if roadie {
            ROADIE
        } else if cover.is_some() {
            COVER
        } else {
            DEFAULT
        };
        self.mode = mode;
        let fov_scale = self.fov / 90.0;
        if look.0.abs() > YAW_ACCEL_THRESHOLD {
            self.accel = (self.accel + dt / YAW_ACCEL_RAMP).min(YAW_ACCEL_MAX);
        } else {
            self.accel = (self.accel - 4.0 * dt / YAW_ACCEL_RAMP).max(0.0);
        }
        let yaw_rate = LOOK_YAW * SENS * fov_scale * (1.0 + YAW_ACCEL_MULT * self.accel) * URAD;
        let pitch_rate = LOOK_PITCH * SENS * PITCH_RATIO * fov_scale * URAD;
        let looked = look.0 != 0.0 || look.1 != 0.0 || mouse.0 != 0.0 || mouse.1 != 0.0;
        self.yaw += look.0 * yaw_rate * dt + mouse.0 * MOUSE_RAD_PER_PX * fov_scale;
        self.pitch += look.1 * pitch_rate * dt - mouse.1 * MOUSE_RAD_PER_PX * fov_scale;
        if looked {
            self.last_look = ch.time;
        }
        if roadie && !aiming {
            // GSM_RoadieRun.RRCameraTurnTime: swing in behind the run; PitchAutoCenterSpeedRoadieRun
            self.yaw += angle_diff(self.yaw, ch.yaw) * (1.0 - (-dt / ROADIE_CAM_TURN).exp());
            self.pitch += (-3000.0 * URAD - self.pitch) * (1.75 * dt).min(1.0);
        } else if ch.time - self.last_look > 2.0 {
            self.pitch += (-0.12 - self.pitch) * (0.5 * dt).min(1.0);
        }
        if let Some(c) = cover {
            // GameplayCam_Cover.ViewMaxLimit: ±75° around the direction over the cover
            let base = (-c.normal.y).atan2(-c.normal.x);
            let d = angle_diff(base, self.yaw);
            let lim = 13650.0 * URAD;
            if d.abs() > lim {
                let over = d - d.signum() * lim;
                self.yaw -= over.signum() * over.abs().min(32768.0 * URAD * dt);
            }
        }
        self.pitch = self.pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT);

        let mut target = if self.pitch >= 0.0 {
            lerp3(mode.mid, mode.low, self.pitch / FRAC_PI_2)
        } else {
            lerp3(mode.mid, mode.high, -self.pitch / FRAC_PI_2)
        };
        if ch
            .active_move()
            .map(|k| {
                matches!(
                    k,
                    MoveKind::EvadeFwd | MoveKind::EvadeBwd | MoveKind::EvadeLt | MoveKind::EvadeRt
                )
            })
            .unwrap_or(false)
        {
            target = [
                target[0] + EVADE_ADJ[0],
                target[1] + EVADE_ADJ[1],
                target[2] + EVADE_ADJ[2],
            ];
        }
        if cover.is_some() && ch.mirrored {
            target[1] = -target[1];
        }
        let kb = 1.0 - (-dt / (mode.blend / 3.0)).exp();
        self.offset = lerp3(self.offset, target, kb);
        self.fov += (mode.fov - self.fov) * kb;

        // origin: pawn centre + eye height (+ vault height), plus the cover posture shift
        let mut o = Vec3::new(ch.pos.x, ch.pos.y, PAWN_HALF + EYE + vault_up);
        if let Some(c) = cover {
            let mut rel = if c.height == CoverHeight::Standing {
                COVER_POSTURE_STD
            } else {
                COVER_POSTURE_MID
            };
            if aiming && c.height != CoverHeight::Standing {
                // GameplayCam_CoverTargeting: PawnRel_Posture_MidLevel + PawnRel_Mod_PopUp_MidLevel
                rel = [-10.0 + 15.0, 8.0 + 34.0, -82.0 + 86.0];
            }
            if ch.is_at_cover_edge(0.5) {
                rel[1] += COVER_EDGE_Y;
            }
            if ch.mirrored {
                rel[1] = -rel[1];
            }
            let (fx, fy) = (ch.yaw.cos(), ch.yaw.sin());
            let (rx, ry) = (-fy, fx);
            o = Vec3::new(
                o.x + fx * rel[0] + rx * rel[1],
                o.y + fy * rel[0] + ry * rel[1],
                o.z + rel[2],
            );
        }
        // per-axis follow in the camera's frame (PerAxisOriginLocInterpSpeed)
        let (cfx, cfy) = (self.yaw.cos(), self.yaw.sin());
        let (crx, cry) = (-cfy, cfx);
        let (dx, dy, dz) = (
            o.x - self.origin.x,
            o.y - self.origin.y,
            o.z - self.origin.z,
        );
        let along = (dx * cfx + dy * cfy) * (mode.follow[0] * dt).min(1.0);
        let side = (dx * crx + dy * cry) * (mode.follow[1] * dt).min(1.0);
        self.origin = Vec3::new(
            self.origin.x + cfx * along + crx * side,
            self.origin.y + cfy * along + cry * side,
            self.origin.z + dz * (mode.follow[2] * dt).min(1.0),
        );
    }

    /// Place the camera: offset in the camera's frame, then pulled in front of any wall
    /// between the pawn and the camera that is taller than the camera.
    pub fn place(&mut self, ch: &Character, level: &Level) {
        let (cy, sy, cp, sp) = (
            self.yaw.cos(),
            self.yaw.sin(),
            self.pitch.cos(),
            self.pitch.sin(),
        );
        let fwd = Vec3::new(cp * cy, cp * sy, sp);
        let right = Vec3::new(-sy, cy, 0.0);
        let up = Vec3::new(-sp * cy, -sp * sy, cp);
        let [ox, oy, oz] = self.offset;
        let mut p = Vec3::new(
            self.origin.x + fwd.x * ox + right.x * oy + up.x * oz,
            self.origin.y + fwd.y * ox + right.y * oy + up.y * oz,
            self.origin.z + fwd.z * ox + right.z * oy + up.z * oz,
        );
        let pivot = (ch.pos.x, ch.pos.y);
        let mut best = 1.0f32;
        for w in &level.walls {
            let h = if w.height == CoverHeight::Standing {
                FULL_H
            } else {
                MID_H
            };
            if p.z > h + 8.0 {
                continue;
            }
            let n = w.normal.normalized();
            for off in [6.0, -WALL_T - 6.0] {
                let a = (w.a.x + n.x * off, w.a.y + n.y * off);
                let b = (w.b.x + n.x * off, w.b.y + n.y * off);
                if let Some(t) = seg_hit(pivot, (p.x, p.y), a, b) {
                    best = best.min(t);
                }
            }
        }
        if best < 1.0 {
            p = Vec3::new(
                pivot.0 + (p.x - pivot.0) * best,
                pivot.1 + (p.y - pivot.1) * best,
                p.z,
            );
        }
        self.pos = p;
    }
}

fn seg_hit(p: (f32, f32), q: (f32, f32), a: (f32, f32), b: (f32, f32)) -> Option<f32> {
    let (rx, ry, sx, sy) = (q.0 - p.0, q.1 - p.1, b.0 - a.0, b.1 - a.1);
    let den = rx * sy - ry * sx;
    if den.abs() < 1e-9 {
        return None;
    }
    let t = ((a.0 - p.0) * sy - (a.1 - p.1) * sx) / den;
    let u = ((a.0 - p.0) * ry - (a.1 - p.1) * rx) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some(t)
}
