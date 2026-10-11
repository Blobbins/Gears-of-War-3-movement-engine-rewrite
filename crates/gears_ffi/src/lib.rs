//! C API for libgears. The header is `include/libgears.h`; keep the two in sync.
//!
//! Units: Unreal units (1 uu ≈ 1 inch in Gears / about 0.75 of a WaW unit, convert in
//! your host), seconds, radians. Axes: X forward, Y right, Z up. Every pointer may be
//! NULL-checked by us; passing a freed handle is undefined behaviour.

#![allow(clippy::missing_safety_doc)]

use gears_core::testworld::{Wall, WallWorld};
use gears_core::{
    Character, Config, CoverHeight, CoverSpot, Input, MoveKind, Vec3, Weapon, WeaponKind,
    WeaponSpec, WeaponTick, World,
};
pub mod anim;
pub use anim::*;
use std::ffi::c_void;
use std::os::raw::c_char;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct GearsVec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<GearsVec3> for Vec3 {
    fn from(v: GearsVec3) -> Self {
        Vec3::new(v.x, v.y, v.z)
    }
}
impl From<Vec3> for GearsVec3 {
    fn from(v: Vec3) -> Self {
        GearsVec3 {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GearsInput {
    pub stick_x: f32,
    pub stick_y: f32,
    pub cam_yaw: f32,
    pub a: i32,
    pub aim: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GearsCoverSpot {
    pub location: GearsVec3,
    /// Points away from the wall, toward the character.
    pub normal: GearsVec3,
    /// 0 = waist-high (vaultable), 1 = standing.
    pub standing: i32,
    pub dist_to_left_edge: f32,
    pub dist_to_right_edge: f32,
    pub can_slip_left: i32,
    pub can_slip_right: i32,
}

impl From<GearsCoverSpot> for CoverSpot {
    fn from(c: GearsCoverSpot) -> Self {
        CoverSpot {
            location: c.location.into(),
            normal: Vec3::from(c.normal).normalized(),
            height: if c.standing != 0 {
                CoverHeight::Standing
            } else {
                CoverHeight::Mid
            },
            dist_to_left_edge: c.dist_to_left_edge,
            dist_to_right_edge: c.dist_to_right_edge,
            can_slip_left: c.can_slip_left != 0,
            can_slip_right: c.can_slip_right != 0,
        }
    }
}
impl From<CoverSpot> for GearsCoverSpot {
    fn from(c: CoverSpot) -> Self {
        GearsCoverSpot {
            location: c.location.into(),
            normal: c.normal.into(),
            standing: (c.height == CoverHeight::Standing) as i32,
            dist_to_left_edge: c.dist_to_left_edge,
            dist_to_right_edge: c.dist_to_right_edge,
            can_slip_left: c.can_slip_left as i32,
            can_slip_right: c.can_slip_right as i32,
        }
    }
}

/// World queries answered by the host. Any callback may be NULL (= "no").
#[repr(C)]
#[derive(Clone, Copy)]
pub struct GearsWorld {
    pub user: *mut c_void,
    pub find_cover: Option<
        unsafe extern "C" fn(*mut c_void, GearsVec3, GearsVec3, f32, *mut GearsCoverSpot) -> i32,
    >,
    pub is_clear: Option<unsafe extern "C" fn(*mut c_void, GearsVec3, GearsVec3, f32) -> i32>,
    pub find_swat_target: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const GearsCoverSpot,
            GearsVec3,
            GearsVec3,
            f32,
            *mut GearsCoverSpot,
        ) -> i32,
    >,
    pub mantle_distance:
        Option<unsafe extern "C" fn(*mut c_void, GearsVec3, GearsVec3, f32, f32, *mut f32) -> i32>,
}

struct HostWorld(GearsWorld);

impl World for HostWorld {
    fn find_cover(&self, from: Vec3, dir: Vec3, max_dist: f32) -> Option<CoverSpot> {
        let f = self.0.find_cover?;
        let mut out = GearsCoverSpot::default();
        (unsafe { f(self.0.user, from.into(), dir.into(), max_dist, &mut out) } != 0)
            .then(|| out.into())
    }
    fn is_clear(&self, from: Vec3, to: Vec3, radius: f32) -> bool {
        match self.0.is_clear {
            Some(f) => unsafe { f(self.0.user, from.into(), to.into(), radius) != 0 },
            None => true,
        }
    }
    fn find_swat_target(
        &self,
        spot: &CoverSpot,
        edge: Vec3,
        side: Vec3,
        max_dist: f32,
    ) -> Option<CoverSpot> {
        let f = self.0.find_swat_target?;
        let s: GearsCoverSpot = (*spot).into();
        let mut out = GearsCoverSpot::default();
        (unsafe {
            f(
                self.0.user,
                &s,
                edge.into(),
                side.into(),
                max_dist,
                &mut out,
            )
        } != 0)
            .then(|| out.into())
    }
    fn mantle_distance(&self, from: Vec3, dir: Vec3, max_dist: f32, radius: f32) -> Option<f32> {
        let f = self.0.mantle_distance?;
        let mut out = 0.0f32;
        (unsafe {
            f(
                self.0.user,
                from.into(),
                dir.into(),
                max_dist,
                radius,
                &mut out,
            )
        } != 0)
            .then_some(out)
    }
}

// ----------------------------------------------------------------- character

pub struct GearsCharacter {
    ch: Character,
}

/// State codes for `GearsState.state`.
fn state_code(c: &Character) -> i32 {
    match c.active_move() {
        None if c.cover_spot().is_some() => 1,
        None => 0,
        Some(k) => {
            2 + match k {
                MoveKind::RoadieRun => 0,
                MoveKind::Run2Cover => 1,
                MoveKind::CoverRun => 2,
                MoveKind::CoverSlip => 3,
                MoveKind::EvadeFwd => 4,
                MoveKind::EvadeBwd => 5,
                MoveKind::EvadeLt => 6,
                MoveKind::EvadeRt => 7,
                MoveKind::PushOut => 8,
                MoveKind::SwatTurn => 9,
                MoveKind::Vault => 10,
            }
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GearsState {
    pub pos: GearsVec3,
    pub vel: GearsVec3,
    pub yaw: f32,
    /// Height above the ground the move started on (vaults).
    pub height: f32,
    /// GEARS_STATE_* (see libgears.h).
    pub state: i32,
    pub in_cover: i32,
    /// Facing left along the cover.
    pub mirrored: i32,
    pub cover: GearsCoverSpot,
    pub time: f32,
    pub time_since_evade: f32,
}

#[no_mangle]
pub extern "C" fn gears_character_new(pos: GearsVec3, yaw: f32) -> *mut GearsCharacter {
    Box::into_raw(Box::new(GearsCharacter {
        ch: Character::new(Config::default(), pos.into(), yaw),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn gears_character_free(c: *mut GearsCharacter) {
    if !c.is_null() {
        drop(Box::from_raw(c));
    }
}

/// Teleport (respawn, host collision push-out). Velocity is kept.
#[no_mangle]
pub unsafe extern "C" fn gears_character_set_pos(c: *mut GearsCharacter, pos: GearsVec3) {
    if let Some(c) = c.as_mut() {
        c.ch.pos = pos.into();
    }
}

fn to_input(i: &GearsInput) -> Input {
    Input {
        stick_x: i.stick_x,
        stick_y: i.stick_y,
        cam_yaw: i.cam_yaw,
        a: i.a != 0,
        aim: i.aim != 0,
    }
}

#[no_mangle]
pub unsafe extern "C" fn gears_character_tick(
    c: *mut GearsCharacter,
    input: *const GearsInput,
    dt: f32,
    world: *const GearsWorld,
) {
    let (Some(c), Some(i), Some(w)) = (c.as_mut(), input.as_ref(), world.as_ref()) else {
        return;
    };
    c.ch.tick(to_input(i), dt, &HostWorld(*w));
}

#[no_mangle]
pub unsafe extern "C" fn gears_character_state(c: *const GearsCharacter, out: *mut GearsState) {
    let (Some(c), Some(out)) = (c.as_ref(), out.as_mut()) else {
        return;
    };
    let ch = &c.ch;
    *out = GearsState {
        pos: ch.pos.into(),
        vel: ch.vel.into(),
        yaw: ch.yaw,
        height: ch.height(),
        state: state_code(ch),
        in_cover: ch.cover_spot().is_some() as i32,
        mirrored: ch.mirrored as i32,
        cover: ch.cover_spot().map(Into::into).unwrap_or_default(),
        time: ch.time,
        time_since_evade: ch.time_since_evade(),
    };
}

#[no_mangle]
pub unsafe extern "C" fn gears_character_event_count(c: *const GearsCharacter) -> u32 {
    c.as_ref().map(|c| c.ch.events.len() as u32).unwrap_or(0)
}

/// Writes event `index` as "[GEARS] t=1.234 Walk -> Run2Cover (reason)" (NUL-terminated,
/// truncated to `len`). Returns the full length, or -1 if there is no such event.
#[no_mangle]
pub unsafe extern "C" fn gears_character_event(
    c: *const GearsCharacter,
    index: u32,
    buf: *mut c_char,
    len: u32,
) -> i32 {
    let Some(e) = c.as_ref().and_then(|c| c.ch.events.get(index as usize)) else {
        return -1;
    };
    let s = format!(
        "[GEARS] t={:.3} {} -> {} ({})",
        e.time, e.from, e.to, e.reason
    );
    if !buf.is_null() && len > 0 {
        let n = s.len().min(len as usize - 1);
        std::ptr::copy_nonoverlapping(s.as_ptr() as *const c_char, buf, n);
        *buf.add(n) = 0;
    }
    s.len() as i32
}

// ------------------------------------------------------- built-in wall world

/// For hosts that just want to describe cover as straight walls.
pub struct GearsWalls {
    walls: Vec<Wall>,
    thickness: f32,
    built: Option<WallWorld>,
}

/// `thickness` > 0: every wall is a slab with cover on both faces.
#[no_mangle]
pub extern "C" fn gears_walls_new(thickness: f32) -> *mut GearsWalls {
    Box::into_raw(Box::new(GearsWalls {
        walls: Vec::new(),
        thickness,
        built: None,
    }))
}

#[no_mangle]
pub unsafe extern "C" fn gears_walls_free(w: *mut GearsWalls) {
    if !w.is_null() {
        drop(Box::from_raw(w));
    }
}

/// A wall from (ax, ay) to (bx, by); (nx, ny) points to the side you take cover on.
#[no_mangle]
pub unsafe extern "C" fn gears_walls_add(
    w: *mut GearsWalls,
    ax: f32,
    ay: f32,
    bx: f32,
    by: f32,
    nx: f32,
    ny: f32,
    standing: i32,
) {
    let Some(w) = w.as_mut() else { return };
    w.walls.push(Wall {
        a: Vec3::new(ax, ay, 0.0),
        b: Vec3::new(bx, by, 0.0),
        normal: Vec3::new(nx, ny, 0.0).normalized(),
        height: if standing != 0 {
            CoverHeight::Standing
        } else {
            CoverHeight::Mid
        },
    });
    w.built = None;
}

#[no_mangle]
pub unsafe extern "C" fn gears_character_tick_walls(
    c: *mut GearsCharacter,
    input: *const GearsInput,
    dt: f32,
    walls: *mut GearsWalls,
) {
    let (Some(c), Some(i), Some(w)) = (c.as_mut(), input.as_ref(), walls.as_mut()) else {
        return;
    };
    if w.built.is_none() {
        w.built = Some(if w.thickness > 0.0 {
            WallWorld::two_sided(w.walls.clone(), w.thickness)
        } else {
            WallWorld::new(w.walls.clone())
        });
    }
    c.ch.tick(to_input(i), dt, w.built.as_ref().unwrap());
}

// ------------------------------------------------------------------ weapons

pub struct GearsWeapon {
    w: Weapon,
}

fn kind_of(k: i32) -> WeaponKind {
    if k == 1 {
        WeaponKind::Gnasher
    } else {
        WeaponKind::Lancer
    }
}

/// 0 = Lancer, 1 = Gnasher.
#[no_mangle]
pub extern "C" fn gears_weapon_new(kind: i32) -> *mut GearsWeapon {
    Box::into_raw(Box::new(GearsWeapon {
        w: Weapon::new(kind_of(kind)),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn gears_weapon_free(w: *mut GearsWeapon) {
    if !w.is_null() {
        drop(Box::from_raw(w));
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GearsWeaponTick {
    pub shots: u32,
    pub reload_started: i32,
    pub reload_finished: i32,
    /// GEARS_AR_*: 0 none, 1 good, 2 perfect, 3 failed (reload pressed again mid-reload)
    pub active_reload: i32,
}

/// `fire` = trigger held, `reload` = reload pressed this tick, `steady` = aiming and
/// nearly still. Returns the shots to trace this tick.
#[no_mangle]
pub unsafe extern "C" fn gears_weapon_tick(
    w: *mut GearsWeapon,
    fire: i32,
    reload: i32,
    steady: i32,
    dt: f32,
) -> GearsWeaponTick {
    let Some(w) = w.as_mut() else {
        return GearsWeaponTick::default();
    };
    let t: WeaponTick = w.w.tick(fire != 0, reload != 0, steady != 0, dt);
    GearsWeaponTick {
        shots: t.shots,
        reload_started: t.reload_started as i32,
        reload_finished: t.reload_finished as i32,
        active_reload: t.active_reload as i32,
    }
}

/// Damage multiplier from a perfect active reload (1.0 when no bonus is running):
/// multiply `gears_weapon_damage` by it.
#[no_mangle]
pub unsafe extern "C" fn gears_weapon_damage_mult(w: *const GearsWeapon) -> f32 {
    w.as_ref().map(|w| w.w.damage_mult()).unwrap_or(1.0)
}

/// Active-reload bar for your HUD: seconds into the reload, its total, and the
/// perfect / good windows (start, end) in seconds into the reload.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GearsReloadBar {
    pub elapsed: f32,
    pub total: f32,
    pub perfect_start: f32,
    pub perfect_end: f32,
    pub good_end: f32,
    pub result: i32,
}

#[no_mangle]
pub unsafe extern "C" fn gears_weapon_reload_bar(w: *const GearsWeapon) -> GearsReloadBar {
    w.as_ref()
        .map(|w| {
            let ((p0, p1), (_, g1)) = w.w.active_windows();
            GearsReloadBar {
                elapsed: w.w.reload_elapsed,
                total: w.w.reload_total,
                perfect_start: p0,
                perfect_end: p1,
                good_end: g1,
                result: w.w.active_result as i32,
            }
        })
        .unwrap_or_default()
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GearsAmmo {
    pub mag: u32,
    pub spare: u32,
    /// Seconds of reload left (0 = not reloading).
    pub reloading: f32,
    pub reload_duration: f32,
}

#[no_mangle]
pub unsafe extern "C" fn gears_weapon_ammo(w: *const GearsWeapon) -> GearsAmmo {
    w.as_ref()
        .map(|w| GearsAmmo {
            mag: w.w.mag,
            spare: w.w.spare,
            reloading: w.w.reloading,
            reload_duration: w.w.spec.reload_duration,
        })
        .unwrap_or_default()
}

/// Half-angle (radians) of the random cone to add to this shot's aim.
#[no_mangle]
pub unsafe extern "C" fn gears_weapon_aim_error(w: *const GearsWeapon) -> f32 {
    w.as_ref()
        .map(|w| w.w.spec.aim_error_rad(w.w.accuracy))
        .unwrap_or(0.0)
}

/// Damage to one target. Lancer: per bullet (`hits` ignored). Gnasher: `hits` of the 10
/// pellets hit this target. `dist` = shooter to hit point (uu).
#[no_mangle]
pub extern "C" fn gears_weapon_damage(kind: i32, hits: u32, dist: f32) -> f32 {
    WeaponSpec::of(kind_of(kind)).damage(hits.max(1), dist)
}

/// Gnasher pellet offsets: writes up to `cap` (yaw, pitch) radian pairs into `out`
/// (2 floats each). Returns the pellet count (10; 0 for the Lancer).
#[no_mangle]
pub unsafe extern "C" fn gears_weapon_pellets(
    kind: i32,
    aiming: i32,
    time_since_evade: f32,
    speed: f32,
    out: *mut f32,
    cap: u32,
) -> u32 {
    let spec = WeaponSpec::of(kind_of(kind));
    let scale = WeaponSpec::spread_scale(
        aiming != 0,
        time_since_evade,
        speed,
        Config::default().ground_speed,
    );
    let p = spec.pellet_offsets(scale);
    if !out.is_null() {
        for (i, (y, pt)) in p.iter().take(cap as usize).enumerate() {
            *out.add(2 * i) = *y;
            *out.add(2 * i + 1) = *pt;
        }
    }
    p.len() as u32
}

/// Upward camera kick per shot, radians (ASSUMPTION: WeaponRecoil in Unreal rotation units).
#[no_mangle]
pub extern "C" fn gears_weapon_recoil(kind: i32) -> f32 {
    gears_core::math::urot_to_rad(WeaponSpec::of(kind_of(kind)).recoil as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slide_into_cover_through_the_c_api() {
        unsafe {
            let w = gears_walls_new(24.0);
            gears_walls_add(w, 300.0, -200.0, 300.0, 200.0, -1.0, 0.0, 0);
            let c = gears_character_new(GearsVec3::default(), 0.0);
            for f in 0..90 {
                let i = GearsInput {
                    stick_y: 1.0,
                    a: (f < 3) as i32,
                    ..Default::default()
                };
                gears_character_tick_walls(c, &i, 1.0 / 60.0, w);
            }
            let mut s = GearsState::default();
            gears_character_state(c, &mut s);
            assert_eq!(s.state, 1, "in cover");
            assert!((s.pos.x - 266.0).abs() < 0.5);
            let mut buf = [0 as c_char; 128];
            assert!(gears_character_event(c, 0, buf.as_mut_ptr(), 128) > 0);
            gears_character_free(c);
            gears_walls_free(w);
        }
    }

    #[test]
    fn gnasher_pellets_and_damage() {
        let mut out = [0.0f32; 20];
        let n = unsafe { gears_weapon_pellets(1, 1, 10.0, 0.0, out.as_mut_ptr(), 10) };
        assert_eq!(n, 10);
        // pellet 1: pitch 284 URot × 0.5 aiming
        assert!((out[3] - urot(142)).abs() < 1e-6);
        assert!((gears_weapon_damage(1, 10, 200.0) - 600.0).abs() < 1e-3);
    }

    fn urot(u: i32) -> f32 {
        gears_core::math::urot_to_rad(u)
    }
}
