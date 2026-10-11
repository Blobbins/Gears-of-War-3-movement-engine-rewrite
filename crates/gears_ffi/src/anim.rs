//! Animation + standard rig C API: drive the libgears standard rig (gears_play::rig) from
//! the player's own Gears export (marcus.glb, loaded at runtime), so a host game can pose
//! its own character. The rig contains nothing from Gears.

use crate::{GearsCharacter, GearsVec3, GearsWeapon};
use gears_core::weapons::{ActiveReload, WeaponKind, WeaponSpec};
use gears_play::anim::{AnimCtx, Animator, WeaponAnim};
use gears_play::glb::Model;
use gears_play::qmath::{Quat, V3};
use gears_play::rig::{standard_rig, Retargeter};
use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::OnceLock;

/// Bones in the standard rig.
pub const GEARS_RIG_BONES: usize = 24;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct GearsQuat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

/// What the host tells the animator each frame.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GearsAnimInput {
    /// 1 while aiming (left trigger / right mouse)
    pub aiming: i32,
    /// camera angles, radians (yaw: Gears convention, X forward, Y right)
    pub cam_pitch: f32,
    pub cam_yaw: f32,
    /// movement stick X, −1..1 (cover lean direction)
    pub stick_x: f32,
    /// 0 Lancer, 1 Gnasher
    pub weapon_kind: i32,
    /// shots fired this frame (GearsWeaponTick.shots)
    pub shots: u32,
    /// the weapon, for reload animation state; may be NULL
    pub weapon: *const GearsWeapon,
}

/// The rig's pose. Rig frame: Y up, +Z forward, +X the character's left, Unreal units,
/// relative to the pawn's feet, facing the pawn's yaw (or `yaw_override`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GearsRigPose {
    pub bone_count: u32,
    /// joint positions
    pub pos: [GearsVec3; GEARS_RIG_BONES],
    /// rotation relative to the parent bone, from the T-pose (what you retarget from)
    pub local: [GearsQuat; GEARS_RIG_BONES],
    /// rotation in the rig frame, from the T-pose
    pub model: [GearsQuat; GEARS_RIG_BONES],
    /// gun grip point and barrel direction
    pub gun_at: GearsVec3,
    pub gun_dir: GearsVec3,
    /// 1: the pose is mirrored (cover facing left) — already applied to the values above
    pub mirrored: i32,
    /// 1: face `yaw_override` (radians, Gears convention) instead of the pawn's yaw
    pub has_yaw_override: i32,
    pub yaw_override: f32,
    /// vault: lift the root by `height` and move it `forward_offset` along the pawn's yaw
    pub height: f32,
    pub forward_offset: f32,
}

impl Default for GearsRigPose {
    fn default() -> Self {
        GearsRigPose {
            bone_count: GEARS_RIG_BONES as u32,
            pos: [GearsVec3::default(); GEARS_RIG_BONES],
            local: [GearsQuat::default(); GEARS_RIG_BONES],
            model: [GearsQuat::default(); GEARS_RIG_BONES],
            gun_at: GearsVec3::default(),
            gun_dir: GearsVec3::default(),
            mirrored: 0,
            has_yaw_override: 0,
            yaw_override: 0.0,
            height: 0.0,
            forward_offset: 0.0,
        }
    }
}

pub struct GearsAnim {
    anim: Animator,
    rig: Retargeter,
    since_shot: f32,
    cock: (f32, f32),
    ar_at: f32,
    last_ar: i32,
    pose: GearsRigPose,
}

fn v(p: V3) -> GearsVec3 {
    GearsVec3 {
        x: p.x,
        y: p.y,
        z: p.z,
    }
}
fn q(r: Quat) -> GearsQuat {
    GearsQuat {
        x: r.x,
        y: r.y,
        z: r.z,
        w: r.w,
    }
}

/// Load the player's exported marcus.glb (bytes) and optional marcus_aim.json (NUL-
/// terminated, may be NULL). Returns NULL if the file isn't a usable export.
///
/// # Safety
/// `glb` must point to `len` readable bytes; `aim_json` NULL or a C string.
#[no_mangle]
pub unsafe extern "C" fn gears_anim_new(
    glb: *const u8,
    len: usize,
    aim_json: *const c_char,
) -> *mut GearsAnim {
    if glb.is_null() {
        return std::ptr::null_mut();
    }
    let Ok(model) = Model::parse(std::slice::from_raw_parts(glb, len)) else {
        return std::ptr::null_mut();
    };
    let Some(rig) = Retargeter::for_marcus(&model) else {
        return std::ptr::null_mut();
    };
    let mut anim = Animator::new(model);
    if !aim_json.is_null() {
        if let Ok(s) = std::ffi::CStr::from_ptr(aim_json).to_str() {
            let _ = anim.load_aim(s);
        }
    }
    Box::into_raw(Box::new(GearsAnim {
        anim,
        rig,
        since_shot: 99.0,
        cock: (0.0, 0.0),
        ar_at: 0.0,
        last_ar: 0,
        pose: GearsRigPose::default(),
    }))
}

/// # Safety
/// `a` from gears_anim_new (or NULL), freed once.
#[no_mangle]
pub unsafe extern "C" fn gears_anim_free(a: *mut GearsAnim) {
    if !a.is_null() {
        drop(Box::from_raw(a));
    }
}

/// Advance the animation by `dt` seconds (after gears_character_tick) and return the rig
/// pose, valid until the next call or gears_anim_free.
///
/// # Safety
/// Valid `a`, `c`, `input`; `input.weapon` NULL or a live GearsWeapon.
#[no_mangle]
pub unsafe extern "C" fn gears_anim_update(
    a: *mut GearsAnim,
    c: *const GearsCharacter,
    input: *const GearsAnimInput,
    dt: f32,
) -> *const GearsRigPose {
    let (a, c, i) = (&mut *a, &*c, &*input);
    let gnasher = i.weapon_kind == 1;
    a.since_shot += dt;
    if i.shots > 0 {
        a.since_shot = 0.0;
        if gnasher {
            // GearWeap_Shotgun: firing 0.25 × interval, then cocking 0.75 × interval
            let iv = WeaponSpec::of(WeaponKind::Gnasher).fire_interval();
            a.cock = (iv * 0.25, iv * 0.75);
        }
    }
    let mut wa = WeaponAnim {
        since_shot: a.since_shot,
        cock_at: a.cock.0,
        cock_len: a.cock.1,
        ..WeaponAnim::names(gnasher)
    };
    if let Some(w) = i.weapon.as_ref().map(|w| &w.w) {
        let ar = w.active_result as i32;
        if ar != a.last_ar && w.active_result != ActiveReload::None {
            a.ar_at = w.reload_elapsed;
        }
        a.last_ar = ar;
        wa.reloading = w.reloading > 0.0;
        wa.reload_elapsed = w.reload_elapsed;
        wa.reload_total = w.reload_total;
        wa.ar_result = ar;
        wa.ar_at = a.ar_at;
    }
    let cx = AnimCtx {
        ch: &c.ch,
        aiming: i.aiming != 0,
        cam_pitch: i.cam_pitch,
        cam_yaw: i.cam_yaw,
        stick_x: i.stick_x,
        weapon: wa,
    };
    let po = a.anim.update(&cx, dt);
    let mut r = a.rig.retarget(&a.anim.model, &po.local);
    if po.mirror {
        r = r.mirrored();
    }
    let (height, fwd) = a.anim.vault_offset(&c.ch).unwrap_or((0.0, 0.0));
    let p = &mut a.pose;
    for k in 0..GEARS_RIG_BONES.min(r.pos.len()) {
        p.pos[k] = v(r.pos[k]);
        p.local[k] = q(r.local[k]);
        p.model[k] = q(r.rot[k]);
    }
    p.gun_at = v(r.gun_at);
    p.gun_dir = v(r.gun_dir);
    p.mirrored = po.mirror as i32;
    p.has_yaw_override = po.yaw_override.is_some() as i32;
    p.yaw_override = po.yaw_override.unwrap_or(0.0);
    p.height = height;
    p.forward_offset = fwd;
    p
}

fn names() -> &'static [CString] {
    static N: OnceLock<Vec<CString>> = OnceLock::new();
    N.get_or_init(|| {
        standard_rig()
            .iter()
            .map(|b| CString::new(b.name).unwrap())
            .collect()
    })
}

/// Standard rig bone count (GEARS_RIG_BONES).
#[no_mangle]
pub extern "C" fn gears_rig_bone_count() -> u32 {
    GEARS_RIG_BONES as u32
}

/// Bone name ("pelvis", "upperarm_l", ...), or NULL past the end. Static storage.
#[no_mangle]
pub extern "C" fn gears_rig_bone_name(i: u32) -> *const c_char {
    names()
        .get(i as usize)
        .map(|c| c.as_ptr())
        .unwrap_or(std::ptr::null())
}

/// Parent bone index, −1 for the root (or past the end).
#[no_mangle]
pub extern "C" fn gears_rig_bone_parent(i: u32) -> i32 {
    standard_rig()
        .get(i as usize)
        .and_then(|b| b.parent)
        .map(|p| p as i32)
        .unwrap_or(-1)
}

/// T-pose joint position (rig frame, uu).
#[no_mangle]
pub extern "C" fn gears_rig_bone_rest(i: u32) -> GearsVec3 {
    standard_rig()
        .get(i as usize)
        .map(|b| v(b.rest))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rig_table_matches_the_rig() {
        assert_eq!(standard_rig().len(), GEARS_RIG_BONES);
        let n = unsafe { std::ffi::CStr::from_ptr(gears_rig_bone_name(7)) };
        assert_eq!(n.to_str().unwrap(), "upperarm_l");
        assert_eq!(gears_rig_bone_parent(0), -1);
        assert!(gears_rig_bone_name(GEARS_RIG_BONES as u32).is_null());
    }
}
