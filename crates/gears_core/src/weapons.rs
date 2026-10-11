//! Gears of War 3 weapon rules for the Lancer and the Gnasher: fire rate, magazines and
//! reloads, range falloff, aim error and the Gnasher's pellet pattern and damage split.
//! Numbers and sources: docs/GEARS_VALUES.md (GearWeapon.ini, Default__GearWeap_Shotgun,
//! GearWeapon.ModifyDamage, GearWeap_Shotgun.FireAllPellets / SetTransientShotgunDamage).
//!
//! Deterministic like the rest of the crate: anything random (aim error) takes a
//! caller-supplied value in 0..1. The host traces the shots and applies the damage.

use crate::math::urot_to_rad;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub enum WeaponKind {
    Lancer = 0,
    Gnasher = 1,
}

/// One weapon's tunables.
#[derive(Clone, Debug)]
pub struct WeaponSpec {
    /// `WeaponDamage` (player). Per bullet for the Lancer, per shot for the Gnasher.
    pub damage: f32,
    /// `WeaponRateOfFire` (rounds per minute); interval = 60 / rate.
    pub rate_of_fire: f32,
    pub auto_fire: bool,
    pub mag_size: u32,
    pub max_spare: u32,
    /// `ReloadDuration` (s)
    pub reload_duration: f32,
    /// `WeaponAimError` (X aiming steady, Y hip / moving). ASSUMPTION: degrees.
    pub aim_error: (f32, f32),
    /// `Range_Melee/Short/Medium/Long` (uu) and `DamageMod_*` for each.
    pub ranges: [f32; 4],
    pub range_mods: [f32; 4],
    /// `WeaponRecoil` (ASSUMPTION: upward camera kick in Unreal rotation units).
    pub recoil: f32,
    /// `Default__GearWeap_Shotgun.PelletSpread` (yaw, pitch) in Unreal rotation units.
    pub pellets: &'static [(f32, f32)],
    /// Active reload (`GearWeapon.ini`): the success zone starts this far into the
    /// reload (`AR_PossibleSuccessStartPoint`), the perfect window comes first
    /// (`ActiveReload_SuperSweetSpotDurations`, one per tier), then the good window
    /// (`AR_SweetSpotDuration`). ASSUMPTION: the perfect window starts with the zone
    /// (its offset comes from native `GetActiveReloadValues`).
    pub ar_start: f32,
    pub ar_super: [f32; 3],
    pub ar_sweet: f32,
    /// Perfect reload bonus: damage × (`ActiveReload_DamageMultiplier`) for
    /// `AR_MagicBulletsTimeoutDuration` seconds.
    pub ar_damage_mult: f32,
    pub ar_bonus_time: f32,
    /// A good / perfect reload plays the rest of the reload this much faster
    /// (`ActiveReloadAnimationSpeedUp` / `SuperActiveReloadAnimationSpeedUp`) —
    /// unless the weapon has its own success anim (`BS_PawnWeaponReloadSuccess`:
    /// the Gnasher's `ADD_SG_Reload_Success`, 1.833 s), which then plays out instead.
    pub ar_speedup: f32,
    pub ar_super_speedup: f32,
    pub ar_success_anim: Option<f32>,
    /// A failed active reload switches to the jam anim (`BS_PawnWeaponReloadFail`,
    /// `ADD_*_Reload_Fail`, 4.0 s) at the same point: the reload then ends with it.
    pub ar_fail_anim: f32,
}

const GNASHER_PELLETS: [(f32, f32); 10] = [
    (0.0, 0.0),
    (0.0, 284.0),
    (-350.0, -256.0),
    (350.0, -256.0),
    (0.0, 900.0),
    (875.0, 425.0),
    (-875.0, 425.0),
    (-875.0, -425.0),
    (875.0, -425.0),
    (0.0, -900.0),
];

impl WeaponSpec {
    pub fn of(kind: WeaponKind) -> Self {
        match kind {
            // [GearWeap_AssaultRifle]
            WeaponKind::Lancer => Self {
                damage: 51.0,
                rate_of_fire: 550.0,
                auto_fire: true,
                mag_size: 60,
                max_spare: 360,
                reload_duration: 2.5,
                aim_error: (1.5, 5.0),
                ranges: [200.0, 750.0, 2048.0, 4096.0],
                range_mods: [1.3, 1.0, 1.0, 0.8],
                recoil: 35.0,
                pellets: &[],
                ar_start: 0.75,
                ar_super: [0.100, 0.075, 0.050],
                ar_sweet: 0.30,
                ar_damage_mult: 1.3,
                ar_bonus_time: 8.0,
                ar_speedup: 1.66,
                ar_super_speedup: 2.5,
                ar_success_anim: None,
                ar_fail_anim: 4.0,
            },
            // [GearWeap_Shotgun]
            WeaponKind::Gnasher => Self {
                damage: 500.0,
                rate_of_fire: 60.0,
                auto_fire: false,
                mag_size: 8,
                max_spare: 31,
                reload_duration: 3.0,
                aim_error: (0.0, 8.0),
                ranges: [300.0, 1000.0, 1300.0, 1650.0],
                range_mods: [1.0, 0.75, 0.45, 0.35],
                recoil: 300.0,
                pellets: &GNASHER_PELLETS,
                ar_start: 0.85,
                ar_super: [0.100, 0.075, 0.050],
                ar_sweet: 0.25,
                ar_damage_mult: 1.2,
                ar_bonus_time: 9.75,
                ar_speedup: 1.66,
                ar_super_speedup: 2.5,
                ar_success_anim: Some(55.0 / 30.0),
                ar_fail_anim: 4.0,
            },
        }
    }

    /// `GearWeapon.GetFireInterval` = 60 / rate of fire.
    pub fn fire_interval(&self) -> f32 {
        60.0 / self.rate_of_fire
    }

    /// `GearWeapon.ModifyDamage` range scale: the melee mod inside Range_Melee, then
    /// linear from band to band, the long-range mod beyond Range_Long.
    pub fn range_scale(&self, dist: f32) -> f32 {
        let (r, m) = (&self.ranges, &self.range_mods);
        if dist <= r[0] {
            return m[0];
        }
        for i in 1..4 {
            if dist <= r[i] {
                return m[i - 1] + (m[i] - m[i - 1]) * (dist - r[i - 1]) / (r[i] - r[i - 1]);
            }
        }
        m[3]
    }

    /// `GetPlayerAimError` = Lerp(X, Y, accuracy), in radians (degrees assumed).
    pub fn aim_error_rad(&self, accuracy: f32) -> f32 {
        let (x, y) = self.aim_error;
        (x + (y - x) * accuracy.clamp(0.0, 1.0)).to_radians()
    }

    /// Damage for one shot on one target. Lancer: per bullet. Gnasher: `hits` pellets
    /// of 10 hit this target (`SetTransientShotgunDamage`: full at ≥ 75 %, else by
    /// share; × 1.2 when the target is under 256 uu away). `dist` = shooter → hit.
    pub fn damage(&self, hits: u32, dist: f32) -> f32 {
        let mut d = self.damage;
        if !self.pellets.is_empty() {
            let n = self.pellets.len() as f32;
            if (hits as f32) < n * 0.75 {
                d *= hits as f32 / n;
            }
            if dist < 256.0 {
                d *= 1.2;
            }
        }
        d * self.range_scale(dist)
    }

    /// `GearWeap_Shotgun.FireAllPellets` spread scale.
    pub fn spread_scale(aiming: bool, since_evade: f32, speed: f32, ground_speed: f32) -> f32 {
        if aiming && since_evade > 1.5 {
            0.5
        } else if since_evade < 1.5 {
            1.1
        } else {
            0.9 + 0.2 * speed / ground_speed
        }
    }

    /// Yaw / pitch offsets (radians) of every pellet, already scaled.
    pub fn pellet_offsets(&self, scale: f32) -> Vec<(f32, f32)> {
        self.pellets
            .iter()
            .map(|&(y, p)| {
                (
                    urot_to_rad((y * scale) as i32),
                    urot_to_rad((p * scale) as i32),
                )
            })
            .collect()
    }
}

/// `Default__GearWeapon.Inaccuracy_Interp_Base_Speed`
pub const INACCURACY_INTERP: f32 = 4.5;

/// Result of pressing reload again during a reload.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub enum ActiveReload {
    #[default]
    None = 0,
    Good = 1,
    Perfect = 2,
    Failed = 3,
}

/// What happened this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeaponTick {
    /// Shots fired this tick (0 or more for the Lancer at long frame times).
    pub shots: u32,
    pub reload_started: bool,
    pub reload_finished: bool,
    pub active_reload: ActiveReload,
}

/// A weapon in the hands: ammo, fire timer, reload, accuracy.
#[derive(Clone, Debug)]
pub struct Weapon {
    pub kind: WeaponKind,
    pub spec: WeaponSpec,
    pub mag: u32,
    pub spare: u32,
    /// Seconds until the next shot may fire.
    pub cooldown: f32,
    /// Seconds of reload left (0 = not reloading).
    pub reloading: f32,
    /// Seconds since this reload started, and its current total length.
    pub reload_elapsed: f32,
    pub reload_total: f32,
    /// This reload's active-reload result (`None` until reload is pressed again).
    pub active_result: ActiveReload,
    /// `GearPawn.ActiveReload_CurrTier`: perfect reloads in a row shrink the window.
    pub ar_tier: usize,
    /// Seconds of perfect-reload damage bonus left.
    pub bonus: f32,
    /// 0 = steady aim, 1 = hip / moving (`GearInventoryManager.Accuracy`; ASSUMPTION:
    /// target 0 when aiming and nearly still, else 1).
    pub accuracy: f32,
    prev_fire: bool,
}

impl Weapon {
    pub fn new(kind: WeaponKind) -> Self {
        let spec = WeaponSpec::of(kind);
        Self {
            kind,
            mag: spec.mag_size,
            spare: spec.max_spare,
            spec,
            cooldown: 0.0,
            reloading: 0.0,
            reload_elapsed: 0.0,
            reload_total: 0.0,
            active_result: ActiveReload::None,
            ar_tier: 0,
            bonus: 0.0,
            accuracy: 1.0,
            prev_fire: false,
        }
    }

    pub fn start_reload(&mut self) -> bool {
        if self.reloading > 0.0 || self.mag >= self.spec.mag_size || self.spare == 0 {
            return false;
        }
        self.reloading = self.spec.reload_duration;
        self.reload_total = self.spec.reload_duration;
        self.reload_elapsed = 0.0;
        self.active_result = ActiveReload::None;
        true
    }

    /// The perfect and good windows of the current tier, as (start, end) seconds into
    /// the reload.
    pub fn active_windows(&self) -> ((f32, f32), (f32, f32)) {
        let s = &self.spec;
        let sup = s.ar_super[self.ar_tier.min(2)];
        (
            (s.ar_start, s.ar_start + sup),
            (s.ar_start + sup, s.ar_start + sup + s.ar_sweet),
        )
    }

    /// `Reloading.HandleButtonPress` → `DetermineActiveReloadSuccessOrFailure`: one try
    /// per reload. Inside the perfect window: perfect (bonus, next tier); inside the
    /// good window: good; anywhere else: jam.
    pub fn try_active_reload(&mut self) -> ActiveReload {
        if self.reloading <= 0.0 || self.active_result != ActiveReload::None {
            return ActiveReload::None;
        }
        let t = self.reload_elapsed;
        let ((p0, p1), (_, g1)) = self.active_windows();
        let left = self.reload_total - t;
        let r = if t >= p0 && t <= p1 {
            ActiveReload::Perfect
        } else if t > p1 && t <= g1 {
            ActiveReload::Good
        } else {
            ActiveReload::Failed
        };
        match r {
            ActiveReload::Perfect | ActiveReload::Good => {
                let perfect = r == ActiveReload::Perfect;
                let new_left = match self.spec.ar_success_anim {
                    Some(len) => len,
                    None => {
                        left / if perfect {
                            self.spec.ar_super_speedup
                        } else {
                            self.spec.ar_speedup
                        }
                    }
                };
                self.reload_total = t + new_left;
                if perfect {
                    self.bonus = self.spec.ar_bonus_time;
                    self.ar_tier = (self.ar_tier + 1).min(2);
                }
            }
            _ => {
                // FailedActiveReload: tier back to 0, bonus off; the jam anim plays out
                self.ar_tier = 0;
                self.bonus = 0.0;
                self.reload_total = t.max(0.0) + (self.spec.ar_fail_anim - t).max(0.0);
            }
        }
        self.reloading = self.reload_total - t;
        self.active_result = r;
        r
    }

    /// Damage multiplier right now (perfect-reload bonus).
    pub fn damage_mult(&self) -> f32 {
        if self.bonus > 0.0 {
            self.spec.ar_damage_mult
        } else {
            1.0
        }
    }

    /// Advance by `dt`. `fire` = trigger held, `reload` = reload pressed this tick,
    /// `steady` = aiming and nearly still. Fires as many shots as the rate allows.
    pub fn tick(&mut self, fire: bool, reload: bool, steady: bool, dt: f32) -> WeaponTick {
        let mut out = WeaponTick::default();
        let want = if steady { 0.0 } else { 1.0 };
        self.accuracy += (want - self.accuracy) * (INACCURACY_INTERP * dt).min(1.0);
        self.bonus = (self.bonus - dt).max(0.0);
        if reload {
            if self.reloading > 0.0 {
                out.active_reload = self.try_active_reload();
            } else {
                out.reload_started = self.start_reload();
            }
        }
        if self.reloading > 0.0 {
            self.reload_elapsed += dt;
            self.reloading -= dt;
            if self.reloading <= 0.0 {
                self.reloading = 0.0;
                let n = (self.spec.mag_size - self.mag).min(self.spare);
                self.mag += n;
                self.spare -= n;
                out.reload_finished = true;
            }
            self.prev_fire = fire;
            return out;
        }
        self.cooldown -= dt;
        let pressed = fire && !self.prev_fire;
        self.prev_fire = fire;
        if fire && (self.spec.auto_fire || pressed) {
            while self.cooldown <= 0.0 && self.mag > 0 {
                self.mag -= 1;
                self.cooldown += self.spec.fire_interval();
                out.shots += 1;
                if !self.spec.auto_fire {
                    break;
                }
            }
            if self.mag == 0 && self.start_reload() {
                out.reload_started = true;
            }
        }
        self.cooldown = self.cooldown.max(0.0);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lancer_fires_550_a_minute() {
        let mut w = Weapon::new(WeaponKind::Lancer);
        let mut shots = 0;
        for _ in 0..180 {
            shots += w.tick(true, false, true, 1.0 / 60.0).shots;
        }
        // 3 s at 550 rpm: one shot every 0.109 s, the first at once → 28
        assert_eq!(shots, 28);
        assert_eq!(w.mag, 32);
    }

    #[test]
    fn gnasher_is_semi_auto_once_a_second() {
        let mut w = Weapon::new(WeaponKind::Gnasher);
        let mut shots = 0;
        for i in 0..240 {
            // tap the trigger every 0.5 s for 4 s
            shots += w.tick(i % 30 == 0, false, true, 1.0 / 60.0).shots;
        }
        assert_eq!(shots, 4, "one shot per second at most");
    }

    #[test]
    fn gnasher_damage_rules() {
        let g = WeaponSpec::of(WeaponKind::Gnasher);
        assert!((g.damage(10, 200.0) - 600.0).abs() < 1e-3); // full, × 1.2 close
        assert!((g.damage(8, 290.0) - 500.0).abs() < 1e-3); // ≥ 75 % = full
        assert!((g.damage(5, 290.0) - 250.0).abs() < 1e-3); // share
        assert!((g.damage(10, 1000.0) - 375.0).abs() < 1e-3); // short band 0.75
        assert!((g.damage(10, 2000.0) - 175.0).abs() < 1e-3); // beyond long 0.35
    }

    #[test]
    fn lancer_range_bands() {
        let l = WeaponSpec::of(WeaponKind::Lancer);
        assert!((l.damage(1, 100.0) - 51.0 * 1.3).abs() < 1e-3);
        assert!((l.damage(1, 1000.0) - 51.0).abs() < 1e-3);
        assert!((l.damage(1, 5000.0) - 51.0 * 0.8).abs() < 1e-3);
    }

    #[test]
    fn reload_refills_from_spare() {
        let mut w = Weapon::new(WeaponKind::Gnasher);
        w.mag = 3;
        assert!(w.tick(false, true, false, 0.1).reload_started);
        let mut done = false;
        for _ in 0..40 {
            done |= w.tick(false, false, false, 0.1).reload_finished;
        }
        assert!(done);
        assert_eq!((w.mag, w.spare), (8, 26));
    }

    fn reload_and_press_at(kind: WeaponKind, at: f32) -> (ActiveReload, f32) {
        let mut w = Weapon::new(kind);
        w.mag = 0;
        w.tick(false, true, false, 0.0);
        let dt = 1.0 / 120.0;
        let mut t = 0.0;
        let mut res = ActiveReload::None;
        while w.reloading > 0.0 {
            let press = res == ActiveReload::None && w.reload_elapsed + dt >= at;
            let o = w.tick(false, press, false, dt);
            if o.active_reload != ActiveReload::None {
                res = o.active_reload;
            }
            t += dt;
        }
        (res, t)
    }

    #[test]
    fn active_reload_windows_and_timing() {
        // Lancer: zone at 0.75 s; perfect 0.75–0.85 (tier 0), good to 1.15
        let (r, t) = reload_and_press_at(WeaponKind::Lancer, 0.80);
        assert_eq!(r, ActiveReload::Perfect);
        assert!(
            (t - (0.80 + 1.70 / 2.5)).abs() < 0.02,
            "perfect reload took {t}"
        );
        let (r, t) = reload_and_press_at(WeaponKind::Lancer, 1.0);
        assert_eq!(r, ActiveReload::Good);
        assert!(
            (t - (1.0 + 1.5 / 1.66)).abs() < 0.02,
            "good reload took {t}"
        );
        let (r, t) = reload_and_press_at(WeaponKind::Lancer, 0.4);
        assert_eq!(r, ActiveReload::Failed);
        assert!((t - 4.0).abs() < 0.02, "jam took {t}");
        // Gnasher success plays its own 1.833 s anim
        let (r, t) = reload_and_press_at(WeaponKind::Gnasher, 0.9);
        assert_eq!(r, ActiveReload::Perfect);
        assert!((t - (0.9 + 55.0 / 30.0)).abs() < 0.02, "gnasher took {t}");
    }

    #[test]
    fn perfect_reload_gives_bonus_and_shrinks_the_window() {
        let mut w = Weapon::new(WeaponKind::Lancer);
        w.mag = 0;
        w.tick(false, true, false, 0.0);
        while w.reload_elapsed < 0.78 {
            w.tick(false, false, false, 0.01);
        }
        assert_eq!(
            w.tick(false, true, false, 0.01).active_reload,
            ActiveReload::Perfect
        );
        assert_eq!(w.damage_mult(), 1.3);
        assert_eq!(w.ar_tier, 1);
        assert!((w.active_windows().0 .1 - 0.825).abs() < 1e-5);
        for _ in 0..900 {
            w.tick(false, false, false, 0.01);
        }
        assert_eq!(w.damage_mult(), 1.0, "bonus over after 8 s");
    }
}
