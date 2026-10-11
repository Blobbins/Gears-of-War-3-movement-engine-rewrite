//! The libgears standard rig: an original, shareable humanoid skeleton (our own bone
//! names, hierarchy and proportions, in a T-pose), plus a retargeter that drives it from
//! whatever source skeleton a player has loaded locally (Marcus, exported from their own
//! copy of Gears 3). Nothing here comes from Gears: the rig is defined below, the motion
//! arrives at runtime.
//!
//! Space: the source model's frame — Y up, +Z forward, +X the character's left — in
//! Unreal units (uu). Every rig bone's rest frame is the model frame itself (identity),
//! so a bone's model rotation is simply "how far it has turned from the T-pose".
//!
//! Retarget: core bones (pelvis, spine, neck, head, shoulders, hands, feet) copy how far
//! the matching source bone has turned from its own rest pose; arms and legs are solved
//! with two-bone IK so hands and feet land where the source's do (scaled to our limb
//! lengths) and elbows / knees bend the same way. Bone lengths never stretch.

use crate::glb::Model;
use crate::qmath::{Quat, Trs, V3};

/// One bone of the standard rig.
#[derive(Clone, Debug)]
pub struct RigBone {
    pub name: &'static str,
    pub parent: Option<usize>,
    /// joint position in the T-pose, model frame, uu
    pub rest: V3,
    /// drawing radius (uu)
    pub radius: f32,
}

macro_rules! b {
    ($n:expr, $p:expr, $x:expr, $y:expr, $z:expr, $r:expr) => {
        RigBone {
            name: $n,
            parent: $p,
            rest: V3 {
                x: $x,
                y: $y,
                z: $z,
            },
            radius: $r,
        }
    };
}

/// The rig, parents before children. 156 uu tall (the Gears pawn's collision height,
/// Default__Pawn 2 × 78); proportions are generic human ones of our own.
pub fn standard_rig() -> Vec<RigBone> {
    vec![
        b!("pelvis", None, 0.0, 90.0, 0.0, 13.0),            // 0
        b!("spine", Some(0), 0.0, 102.0, 0.0, 13.0),         // 1
        b!("chest", Some(1), 0.0, 120.0, 0.0, 15.0),         // 2
        b!("neck", Some(2), 0.0, 138.0, 0.0, 5.0),           // 3
        b!("head", Some(3), 0.0, 147.0, 2.0, 10.0),          // 4
        b!("head_end", Some(4), 0.0, 165.0, 2.0, 0.0),       // 5
        b!("shoulder_l", Some(2), 5.0, 135.0, -1.0, 6.0),    // 6
        b!("upperarm_l", Some(6), 18.0, 135.0, -1.0, 6.0),   // 7
        b!("forearm_l", Some(7), 44.0, 135.0, -1.0, 5.0),    // 8
        b!("hand_l", Some(8), 67.0, 135.0, -1.0, 4.0),       // 9
        b!("hand_l_end", Some(9), 77.0, 135.0, -1.0, 0.0),   // 10
        b!("shoulder_r", Some(2), -5.0, 135.0, -1.0, 6.0),   // 11
        b!("upperarm_r", Some(11), -18.0, 135.0, -1.0, 6.0), // 12
        b!("forearm_r", Some(12), -44.0, 135.0, -1.0, 5.0),  // 13
        b!("hand_r", Some(13), -67.0, 135.0, -1.0, 4.0),     // 14
        b!("hand_r_end", Some(14), -77.0, 135.0, -1.0, 0.0), // 15
        b!("thigh_l", Some(0), 9.0, 86.0, 0.0, 7.0),         // 16
        b!("shin_l", Some(16), 10.0, 47.0, 0.0, 6.0),        // 17
        b!("foot_l", Some(17), 10.0, 9.0, 0.0, 5.0),         // 18
        b!("toe_l", Some(18), 10.0, 2.0, 16.0, 0.0),         // 19
        b!("thigh_r", Some(0), -9.0, 86.0, 0.0, 7.0),        // 20
        b!("shin_r", Some(20), -10.0, 47.0, 0.0, 6.0),       // 21
        b!("foot_r", Some(21), -10.0, 9.0, 0.0, 5.0),        // 22
        b!("toe_r", Some(22), -10.0, 2.0, 16.0, 0.0),        // 23
    ]
}

pub const PELVIS: usize = 0;
pub const HAND_R: usize = 14;

/// The rig's pose this frame (model frame, uu).
#[derive(Clone, Debug, Default)]
pub struct RigPose {
    /// joint positions
    pub pos: Vec<V3>,
    /// model-space rotation of each bone, relative to the T-pose
    pub rot: Vec<Quat>,
    /// local rotation (relative to the parent) — what a host engine retargets from
    pub local: Vec<Quat>,
    /// the gun: grip point and barrel direction
    pub gun_at: V3,
    pub gun_dir: V3,
}

impl RigPose {
    /// Reflect across the model's X axis (the source drew mirrored: cover facing left).
    pub fn mirrored(mut self) -> RigPose {
        let fx = |v: V3| V3::new(-v.x, v.y, v.z);
        let fq = |q: Quat| Quat::new(q.x, -q.y, -q.z, q.w);
        self.pos.iter_mut().for_each(|v| *v = fx(*v));
        self.rot.iter_mut().for_each(|q| *q = fq(*q));
        self.local.iter_mut().for_each(|q| *q = fq(*q));
        self.gun_at = fx(self.gun_at);
        self.gun_dir = fx(self.gun_dir);
        self
    }
}

/// Which source bone drives each rig bone, and how.
#[derive(Clone, Copy, Debug)]
enum Drive {
    /// copy the source bone's turn from its rest pose (source bone, alignment from our
    /// T-pose rest to the source's bind direction)
    Copy(usize, Quat),
    /// part of a two-bone IK limb (see `limbs`)
    Limb,
    /// follows its parent (end markers)
    None,
}

pub struct Retargeter {
    pub bones: Vec<RigBone>,
    drives: Vec<Drive>,
    limbs: Vec<([usize; 3], [usize; 3])>,
    src_bind: Vec<Trs>,
    src_pelvis: usize,
    src_socket: Option<usize>,
    src_hand_r: usize,
}

/// Rotation turning unit vector `a` onto unit vector `b` (shortest arc).
fn arc(a: V3, b: V3) -> Quat {
    let d = a.dot(b);
    if d < -0.9999 {
        let axis = if a.x.abs() < 0.9 {
            V3::new(1.0, 0.0, 0.0).cross(a)
        } else {
            V3::new(0.0, 1.0, 0.0).cross(a)
        };
        let l = axis.len();
        return Quat::from_axis_angle(axis.mul(1.0 / l), std::f32::consts::PI);
    }
    let c = a.cross(b);
    Quat::new(c.x, c.y, c.z, 1.0 + d).normalize()
}

fn norm(v: V3) -> V3 {
    let l = v.len();
    if l < 1e-6 {
        V3::ZERO
    } else {
        v.mul(1.0 / l)
    }
}

/// Rotation that takes rest direction `d0` to `d1`, then twists about `d1` so the rest
/// secondary axis `s0` (after the swing) lines up with `s1` as well as it can.
fn aim(d0: V3, s0: V3, d1: V3, s1: V3) -> Quat {
    let swing = arc(d0, d1);
    let s_sw = swing.rotate(s0);
    // project both onto the plane normal to d1
    let a = norm(s_sw.sub(d1.mul(s_sw.dot(d1))));
    let b = norm(s1.sub(d1.mul(s1.dot(d1))));
    if a.len() < 0.5 || b.len() < 0.5 {
        return swing;
    }
    let ang = a.cross(b).dot(d1).atan2(a.dot(b));
    Quat::from_axis_angle(d1, ang).mul(swing)
}

impl Retargeter {
    /// Map the standard rig onto a loaded source skeleton (Marcus's bone names).
    pub fn for_marcus(src: &Model) -> Option<Retargeter> {
        let n = |s: &str| src.node(s);
        let bones = standard_rig();
        let mut drives = vec![Drive::None; bones.len()];
        let src_bind = src.model_space(&src.bind_pose());
        // (our bone, our child, source bone, source child): the alignment turns our rest
        // direction onto the source's bind direction, so an A-pose source and our T-pose
        // agree (no child = no alignment)
        let copy: [(usize, usize, &str, &str); 11] = [
            (0, 1, "b_MF_Pelvis", ""),
            (1, 2, "b_MF_Spine_01", ""),
            (2, 3, "b_MF_Spine_03", ""),
            (3, 4, "b_MF_Neck", ""),
            (4, 5, "b_MF_Head", ""),
            (6, 7, "b_MF_Clavicle_L", "b_MF_UpperArm_L"),
            (9, 10, "b_MF_Hand_L", "b_MF_Finger_04_L"),
            (11, 12, "b_MF_Clavicle_R", "b_MF_UpperArm_R"),
            (14, 15, "b_MF_Hand_R", "b_MF_Finger_04_R"),
            (18, 19, "b_MF_Foot_L", "b_MF_Toe_L"),
            (22, 23, "b_MF_Foot_R", "b_MF_Toe_R"),
        ];
        for (i, c, s, sc) in copy {
            let si = n(s)?;
            let align = match n(sc) {
                Some(ci) if !sc.is_empty() => arc(
                    norm(bones[c].rest.sub(bones[i].rest)),
                    norm(src_bind[ci].t.sub(src_bind[si].t)),
                ),
                _ => Quat::IDENTITY,
            };
            drives[i] = Drive::Copy(si, align);
        }
        let limbs = vec![
            (
                [
                    n("b_MF_UpperArm_L")?,
                    n("b_MF_Forearm_L")?,
                    n("b_MF_Hand_L")?,
                ],
                [7, 8, 9],
            ),
            (
                [
                    n("b_MF_UpperArm_R")?,
                    n("b_MF_Forearm_R")?,
                    n("b_MF_Hand_R")?,
                ],
                [12, 13, 14],
            ),
            (
                [n("b_MF_Thigh_L")?, n("b_MF_Calf_L")?, n("b_MF_Foot_L")?],
                [16, 17, 18],
            ),
            (
                [n("b_MF_Thigh_R")?, n("b_MF_Calf_R")?, n("b_MF_Foot_R")?],
                [20, 21, 22],
            ),
        ];
        for (_, r) in &limbs {
            drives[r[0]] = Drive::Limb;
            drives[r[1]] = Drive::Limb;
        }
        Some(Retargeter {
            bones,
            drives,
            limbs,
            src_bind,
            src_pelvis: n("b_MF_Pelvis")?,
            src_socket: n("b_MF_Weapon_R_End"),
            src_hand_r: n("b_MF_Hand_R")?,
        })
    }

    /// Retarget one source pose (`src_local`: the source model's local transforms).
    pub fn retarget(&self, src: &Model, src_local: &[Trs]) -> RigPose {
        let ms = src.model_space(src_local);
        let p = |i: usize| ms[i].t.mul(100.0); // source is in metres
        let delta = |i: usize| ms[i].r.mul(self.src_bind[i].r.conj()).normalize();
        let n = self.bones.len();
        let mut rot = vec![Quat::IDENTITY; n];
        let mut pos = vec![V3::ZERO; n];
        let rest_off = |i: usize| {
            self.bones[i]
                .rest
                .sub(self.bones[self.bones[i].parent.unwrap()].rest)
        };

        // pelvis: the source's, scaled to our pelvis height
        let sp = p(self.src_pelvis);
        let sb = self.src_bind[self.src_pelvis].t.mul(100.0);
        let k = self.bones[PELVIS].rest.y / sb.y.max(1.0);
        pos[PELVIS] = V3::new(sp.x, sp.y * k, sp.z);

        for i in 0..n {
            if let Some(par) = self.bones[i].parent {
                pos[i] = pos[par].add(rot[par].rotate(rest_off(i)));
            }
            rot[i] = match self.drives[i] {
                Drive::Copy(s, align) => delta(s).mul(align).normalize(),
                Drive::None => self.bones[i]
                    .parent
                    .map(|q| rot[q])
                    .unwrap_or(Quat::IDENTITY),
                Drive::Limb => rot[i], // solved below, once the root of the limb is placed
            };
            // solve a limb when we reach its upper bone
            if let Some((s, r)) = self.limbs.iter().find(|(_, r)| r[0] == i) {
                let (s, r) = (*s, *r);
                let (lu, ll) = (
                    self.bones[r[1]].rest.sub(self.bones[r[0]].rest).len(),
                    self.bones[r[2]].rest.sub(self.bones[r[1]].rest).len(),
                );
                let su = p(s[1]).sub(p(s[0])).len() + p(s[2]).sub(p(s[1])).len();
                // end target: the source end relative to the source root, scaled to our limb
                let reach = p(s[2]).sub(p(s[0])).mul((lu + ll) / su.max(1e-3));
                let target = pos[i].add(reach);
                // bend toward the source's elbow / knee
                let pole = p(s[1]).sub(p(s[0]).lerp(p(s[2]), 0.5));
                let (elbow, end) = two_bone(pos[i], target, lu, ll, pole);
                let d0 = norm(self.bones[r[1]].rest.sub(self.bones[r[0]].rest));
                let d1 = norm(self.bones[r[2]].rest.sub(self.bones[r[1]].rest));
                let fwd = V3::new(0.0, 0.0, 1.0);
                // keep each limb segment's twist with the source's (its forward axis)
                let s_up = delta(s[0]).rotate(fwd);
                let s_lo = delta(s[1]).rotate(fwd);
                rot[r[0]] = aim(d0, fwd, norm(elbow.sub(pos[i])), s_up);
                rot[r[1]] = aim(d1, fwd, norm(end.sub(elbow)), s_lo);
            }
        }
        // second pass: positions from the final rotations (limbs solved above)
        for i in 0..n {
            if let Some(par) = self.bones[i].parent {
                pos[i] = pos[par].add(rot[par].rotate(rest_off(i)));
            }
        }
        let local = (0..n)
            .map(|i| match self.bones[i].parent {
                Some(par) => rot[par].conj().mul(rot[i]).normalize(),
                None => rot[i],
            })
            .collect();
        // the gun: the source socket's offset from its hand, from our hand; its barrel
        let (gun_at, gun_dir) = match self.src_socket {
            Some(sk) => (
                pos[HAND_R].add(p(sk).sub(p(self.src_hand_r))),
                ms[sk].r.rotate(V3::new(1.0, 0.0, 0.0)),
            ),
            None => (pos[HAND_R], rot[HAND_R].rotate(V3::new(-1.0, 0.0, 0.0))),
        };
        RigPose {
            pos,
            rot,
            local,
            gun_at,
            gun_dir: norm(gun_dir),
        }
    }
}

/// Two-bone IK: from `root`, reach `target` with segments `a`, `b`, bending toward
/// `pole`. Returns (middle joint, end joint).
pub fn two_bone(root: V3, target: V3, a: f32, b: f32, pole: V3) -> (V3, V3) {
    let to = target.sub(root);
    let d = to.len().clamp(1e-3, (a + b) * 0.9999);
    let dir = norm(to);
    let end = root.add(dir.mul(d));
    // elbow distance along dir and out to the side (law of cosines)
    let x = (a * a - b * b + d * d) / (2.0 * d);
    let h = (a * a - x * x).max(0.0).sqrt();
    let mut side = pole.sub(dir.mul(pole.dot(dir)));
    if side.len() < 1e-4 {
        side = if dir.y.abs() < 0.9 {
            V3::new(0.0, 1.0, 0.0).cross(dir)
        } else {
            V3::new(1.0, 0.0, 0.0).cross(dir)
        };
    }
    let side = norm(side);
    (root.add(dir.mul(x)).add(side.mul(h)), end)
}
