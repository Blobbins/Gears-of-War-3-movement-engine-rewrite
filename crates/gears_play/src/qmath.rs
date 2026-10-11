//! Small vector / quaternion maths in glTF space (Y up, right-handed), f32.
#![allow(clippy::should_implement_trait)]

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl V3 {
    pub const ZERO: V3 = V3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub const ONE: V3 = V3 {
        x: 1.0,
        y: 1.0,
        z: 1.0,
    };
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    pub fn add(self, o: V3) -> V3 {
        V3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
    pub fn sub(self, o: V3) -> V3 {
        V3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
    pub fn mul(self, s: f32) -> V3 {
        V3::new(self.x * s, self.y * s, self.z * s)
    }
    pub fn cmul(self, o: V3) -> V3 {
        V3::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }
    pub fn lerp(self, o: V3, t: f32) -> V3 {
        self.add(o.sub(self).mul(t))
    }
    pub fn dot(self, o: V3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: V3) -> V3 {
        V3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn len(self) -> f32 {
        self.dot(self).sqrt()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Default for Quat {
    fn default() -> Self {
        Quat::IDENTITY
    }
}

impl Quat {
    pub const IDENTITY: Quat = Quat {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }
    pub fn from_axis_angle(a: V3, ang: f32) -> Quat {
        let s = (ang * 0.5).sin();
        Quat::new(a.x * s, a.y * s, a.z * s, (ang * 0.5).cos())
    }
    /// self * o (apply o first, then self)
    pub fn mul(self, o: Quat) -> Quat {
        Quat::new(
            self.w * o.x + self.x * o.w + self.y * o.z - self.z * o.y,
            self.w * o.y - self.x * o.z + self.y * o.w + self.z * o.x,
            self.w * o.z + self.x * o.y - self.y * o.x + self.z * o.w,
            self.w * o.w - self.x * o.x - self.y * o.y - self.z * o.z,
        )
    }
    pub fn conj(self) -> Quat {
        Quat::new(-self.x, -self.y, -self.z, self.w)
    }
    pub fn dot(self, o: Quat) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z + self.w * o.w
    }
    pub fn normalize(self) -> Quat {
        let l = self.dot(self).sqrt();
        if l < 1e-9 {
            Quat::IDENTITY
        } else {
            Quat::new(self.x / l, self.y / l, self.z / l, self.w / l)
        }
    }
    pub fn rotate(self, v: V3) -> V3 {
        let q = V3::new(self.x, self.y, self.z);
        let t = q.cross(v).mul(2.0);
        v.add(t.mul(self.w)).add(q.cross(t))
    }
    /// Spherical interpolation (same as three.js Quaternion.slerp).
    pub fn slerp(self, b: Quat, t: f32) -> Quat {
        if t <= 0.0 {
            return self;
        }
        if t >= 1.0 {
            return b;
        }
        let mut cos = self.dot(b);
        let mut b = b;
        if cos < 0.0 {
            cos = -cos;
            b = Quat::new(-b.x, -b.y, -b.z, -b.w);
        }
        if cos >= 1.0 {
            return self;
        }
        let sq = 1.0 - cos * cos;
        if sq <= f32::EPSILON {
            let s = 1.0 - t;
            return Quat::new(
                s * self.x + t * b.x,
                s * self.y + t * b.y,
                s * self.z + t * b.z,
                s * self.w + t * b.w,
            )
            .normalize();
        }
        let sin = sq.sqrt();
        let half = sin.atan2(cos);
        let ra = ((1.0 - t) * half).sin() / sin;
        let rb = (t * half).sin() / sin;
        Quat::new(
            self.x * ra + b.x * rb,
            self.y * ra + b.y * rb,
            self.z * ra + b.z * rb,
            self.w * ra + b.w * rb,
        )
    }
}

/// A bone transform: translation, rotation, scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trs {
    pub t: V3,
    pub r: Quat,
    pub s: V3,
}

impl Default for Trs {
    fn default() -> Self {
        Trs {
            t: V3::ZERO,
            r: Quat::IDENTITY,
            s: V3::ONE,
        }
    }
}

impl Trs {
    /// parent * child
    pub fn compose(self, c: Trs) -> Trs {
        Trs {
            t: self.t.add(self.r.rotate(c.t.cmul(self.s))),
            r: self.r.mul(c.r),
            s: self.s.cmul(c.s),
        }
    }
}
