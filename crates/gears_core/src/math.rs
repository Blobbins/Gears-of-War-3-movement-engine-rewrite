//! Minimal vector math. Unreal conventions: X forward, Y right, Z up, units in uu.

use core::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3::new(0.0, 0.0, 0.0);

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn length_2d(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn flat(self) -> Vec3 {
        Vec3::new(self.x, self.y, 0.0)
    }

    pub fn normalized(self) -> Vec3 {
        let l = self.length();
        if l > 1e-6 {
            self * (1.0 / l)
        } else {
            Vec3::ZERO
        }
    }

    /// Unit vector on the ground plane for a yaw in radians (0 = +X, +π/2 = +Y).
    pub fn from_yaw(yaw: f32) -> Vec3 {
        Vec3::new(yaw.cos(), yaw.sin(), 0.0)
    }

    pub fn yaw(self) -> f32 {
        self.y.atan2(self.x)
    }

    /// Move `self` toward `target` by at most `max_delta`.
    pub fn move_toward(self, target: Vec3, max_delta: f32) -> Vec3 {
        let d = target - self;
        let l = d.length();
        if l <= max_delta || l < 1e-6 {
            target
        } else {
            self + d * (max_delta / l)
        }
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl AddAssign for Vec3 {
    fn add_assign(&mut self, o: Vec3) {
        *self = *self + o;
    }
}
impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl SubAssign for Vec3 {
    fn sub_assign(&mut self, o: Vec3) {
        *self = *self - o;
    }
}
impl Mul<f32> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f32) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}
impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}

/// Unreal rotation units (65536 per turn) to radians. Used for values copied from
/// Gears script, e.g. the shipped 16834 evade-left yaw.
pub fn urot_to_rad(u: i32) -> f32 {
    u as f32 * core::f32::consts::TAU / 65536.0
}

/// Shortest signed angle from `a` to `b`, radians.
pub fn angle_diff(a: f32, b: f32) -> f32 {
    let mut d = (b - a) % core::f32::consts::TAU;
    if d > core::f32::consts::PI {
        d -= core::f32::consts::TAU;
    } else if d < -core::f32::consts::PI {
        d += core::f32::consts::TAU;
    }
    d
}
