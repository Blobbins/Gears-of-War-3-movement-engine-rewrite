//! One play session: the character, level, camera, animation, guns, targets and sound
//! cues, stepped at a fixed 60 Hz. A host feeds `PadInput` and draws `Frame`.

use crate::anim::{AnimCtx, Animator, GunAnimator, PoseOut, WeaponAnim};
use crate::camera::{Camera, PAWN_HALF};
use crate::glb::Model;
use crate::level::{Level, LevelId, TARGET_H, TARGET_HEALTH};
use crate::qmath::Trs;
use crate::rig::{Retargeter, RigPose};
use gears_core::weapons::{ActiveReload, Weapon, WeaponKind};
use gears_core::{Character, Config, CoverHeight, Input, MoveKind, Vec3};

pub const DT: f32 = 1.0 / 60.0;
const TRACE_RANGE: f32 = 16384.0;

/// Controller-style input for one step (keyboard / mouse mapped by the host).
#[derive(Clone, Copy, Debug, Default)]
pub struct PadInput {
    pub stick_x: f32,
    pub stick_y: f32,
    pub look_x: f32,
    pub look_y: f32,
    /// mouse pixels since the last step
    pub mouse_dx: f32,
    pub mouse_dy: f32,
    pub a: bool,
    pub aim: bool,
    pub fire: bool,
    pub reload: bool,
    pub swap: bool,
}

/// A sound to start (`loop_id` set: keep looping until `StopLoop`).
#[derive(Clone, Debug, PartialEq)]
pub enum Cue {
    Play { name: String, volume: f32 },
    Loop { id: u32, name: String, volume: f32 },
    StopLoop { id: u32 },
}

#[derive(Clone, Debug)]
pub struct Tracer {
    pub from: Vec3,
    pub to: Vec3,
    pub age: f32,
}

#[derive(Clone, Debug)]
pub struct Target {
    pub x: f32,
    pub y: f32,
    pub hp: f32,
    /// > 0: knocked down, seconds until it comes back
    pub dead: f32,
}

#[derive(Clone, Debug)]
pub struct Popup {
    pub text: String,
    pub at: Vec3,
    pub age: f32,
}

/// Everything a host needs to draw this frame.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub pawn_pos: Vec3,
    pub draw_yaw: f32,
    pub height: f32,
    pub forward_offset: f32,
    pub pose: Option<PoseOut>,
    /// the pose on the libgears standard rig (crate::rig), already mirrored when `pose` is;
    /// model frame (Y up, +Z forward, +X left), uu, relative to the pawn's feet
    pub rig: Option<RigPose>,
    pub gun: WeaponKind2,
    pub gun_pose: Option<Vec<Trs>>,
    pub cam_pos: Vec3,
    pub cam_forward: Vec3,
    /// horizontal FOV, degrees
    pub cam_fov: f32,
    pub state: String,
    pub cam_mode: &'static str,
    pub aiming: bool,
    pub muzzle_flash: bool,
    pub hud_ammo: String,
    pub hud_msg: String,
    /// reload bar: (elapsed, base length, perfect start, perfect end, good end, result)
    pub reload_bar: Option<(f32, f32, f32, f32, f32, i32)>,
    pub bonus: bool,
    pub hit_marker: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WeaponKind2 {
    #[default]
    Lancer,
    Gnasher,
}

struct Gun {
    w: Weapon,
    anim: Option<GunAnimator>,
}

/// Small deterministic RNG (xorshift) for aim error and chatter.
struct Rng(u64);
impl Rng {
    fn f(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[((self.f() * xs.len() as f32) as usize).min(xs.len() - 1)]
    }
}

pub struct Game {
    pub ch: Character,
    pub level: Level,
    pub cam: Camera,
    pub anim: Option<Animator>,
    guns: [Gun; 2],
    cur: usize,
    pub targets: Vec<Target>,
    pub tracers: Vec<Tracer>,
    pub popups: Vec<Popup>,
    pub cues: Vec<Cue>,
    delayed: Vec<(f32, String, f32)>,
    time: f32,
    aiming: bool,
    prev: PadInput,
    since_shot: f32,
    cock: (f32, f32),
    lancer_loop: Option<u32>,
    next_loop: u32,
    flash: f32,
    hit: f32,
    ar_at: f32,
    rng: Rng,
    voice_last: f32,
    kills: Vec<f32>,
    event_i: usize,
    foot: (i32, i32),
    landed: bool,
    last_stick_x: f32,
    pose: Option<PoseOut>,
    rig: Option<Retargeter>,
}

impl Game {
    pub fn new(level: LevelId) -> Self {
        let level = Level::new(level);
        let ch = Character::new(Config::default(), level.start, level.yaw);
        let cam = Camera::new(&ch, level.yaw);
        let targets = level
            .targets
            .iter()
            .map(|&(x, y)| Target {
                x,
                y,
                hp: TARGET_HEALTH,
                dead: 0.0,
            })
            .collect();
        Game {
            ch,
            cam,
            targets,
            level,
            anim: None,
            guns: [
                Gun {
                    w: Weapon::new(WeaponKind::Lancer),
                    anim: None,
                },
                Gun {
                    w: Weapon::new(WeaponKind::Gnasher),
                    anim: None,
                },
            ],
            cur: 0,
            tracers: Vec::new(),
            popups: Vec::new(),
            cues: Vec::new(),
            delayed: Vec::new(),
            time: 0.0,
            aiming: false,
            prev: PadInput::default(),
            since_shot: 99.0,
            cock: (0.0, 0.0),
            lancer_loop: None,
            next_loop: 1,
            flash: 0.0,
            hit: 0.0,
            ar_at: 0.0,
            rng: Rng(0x9e3779b97f4a7c15),
            voice_last: -99.0,
            kills: Vec::new(),
            event_i: 0,
            foot: (0, 0),
            landed: false,
            last_stick_x: 0.0,
            pose: None,
            rig: None,
        }
    }

    /// Restart on another range, keeping the loaded assets.
    pub fn load_level(&mut self, id: LevelId) {
        let anim = self.anim.take();
        let rig = self.rig.take();
        let guns = [self.guns[0].anim.take(), self.guns[1].anim.take()];
        *self = Game::new(id);
        self.anim = anim;
        self.rig = rig;
        let [g0, g1] = guns;
        self.guns[0].anim = g0;
        self.guns[1].anim = g1;
    }

    /// Your exported files (marcus.glb, lancer.glb, gnasher.glb, marcus_aim.json).
    pub fn load_assets(
        &mut self,
        marcus: &[u8],
        aim_json: Option<&str>,
        lancer: Option<&[u8]>,
        gnasher: Option<&[u8]>,
    ) -> Result<(), String> {
        let mut a = Animator::new(Model::parse(marcus)?);
        self.rig = Retargeter::for_marcus(&a.model);
        if let Some(j) = aim_json {
            a.load_aim(j)?;
        }
        self.anim = Some(a);
        if let Some(b) = lancer {
            self.guns[0].anim = Some(GunAnimator::new(Model::parse(b)?));
        }
        if let Some(b) = gnasher {
            self.guns[1].anim = Some(GunAnimator::new(Model::parse(b)?));
        }
        Ok(())
    }

    /// The gun's own skeleton (lancer.glb / gnasher.glb), 0 = Lancer, 1 = Gnasher.
    pub fn gun_model(&self, i: usize) -> Option<&Model> {
        self.guns
            .get(i)
            .and_then(|g| g.anim.as_ref())
            .map(|a| &a.model)
    }

    pub fn weapon(&self) -> &Weapon {
        &self.guns[self.cur].w
    }

    fn can_aim(&self) -> bool {
        self.ch
            .active_move()
            .map(|k| k == MoveKind::PushOut)
            .unwrap_or(true)
    }

    fn play(&mut self, name: &str, volume: f32) {
        self.cues.push(Cue::Play {
            name: name.to_string(),
            volume,
        });
    }
    fn later(&mut self, name: &str, delay: f32, volume: f32) {
        self.delayed
            .push((self.time + delay, name.to_string(), volume));
    }
    fn stop_loop(&mut self, tail: bool) {
        if let Some(id) = self.lancer_loop.take() {
            self.cues.push(Cue::StopLoop { id });
            if tail {
                self.play("CogARifleFireStopPlayer01", 0.9);
            }
        }
    }

    /// One fixed 1/60 s step.
    pub fn step(&mut self, pad: PadInput) {
        let dt = DT;
        self.time += dt;
        let aim = pad.aim && self.can_aim();
        self.aiming = aim;
        let input = Input {
            stick_x: pad.stick_x,
            stick_y: pad.stick_y,
            cam_yaw: self.cam.yaw,
            a: pad.a,
            aim: aim && self.ch.cover_spot().is_none(),
        };
        self.ch.tick(input, dt, &self.level.world);
        let vaulting = self.ch.active_move() == Some(MoveKind::Vault);
        if !matches!(
            self.ch.active_move(),
            Some(MoveKind::Run2Cover) | Some(MoveKind::Vault)
        ) {
            self.ch.pos = self.level.push_out(self.ch.pos);
        }
        let vault_up = if vaulting {
            self.anim
                .as_ref()
                .and_then(|a| a.vault_offset(&self.ch))
                .map(|v| v.0)
                .unwrap_or(self.ch.height())
        } else {
            0.0
        };
        self.cam.step(
            &self.ch,
            aim,
            (pad.look_x, pad.look_y),
            (pad.mouse_dx, pad.mouse_dy),
            vault_up,
            dt,
        );
        self.tick_weapons(&pad, dt);
        self.movement_cues();
        // delayed sounds
        let now = self.time;
        let due: Vec<_> = self
            .delayed
            .iter()
            .filter(|d| d.0 <= now)
            .cloned()
            .collect();
        self.delayed.retain(|d| d.0 > now);
        for (_, n, v) in due {
            self.play(&n, v);
        }
        for t in &mut self.tracers {
            t.age += dt;
        }
        self.tracers.retain(|t| t.age < 0.07);
        for p in &mut self.popups {
            p.age += dt;
            p.at.z += 60.0 * dt;
        }
        self.popups.retain(|p| p.age < 0.8);
        for t in &mut self.targets {
            if t.dead > 0.0 {
                t.dead -= dt;
                if t.dead <= 0.0 {
                    t.dead = 0.0;
                    t.hp = TARGET_HEALTH;
                }
            }
        }
        self.last_stick_x = pad.stick_x;
        // animation
        let wa = self.weapon_anim();
        if let Some(anim) = self.anim.as_mut() {
            let cx = AnimCtx {
                ch: &self.ch,
                aiming: aim,
                cam_pitch: self.cam.pitch,
                cam_yaw: self.cam.yaw,
                stick_x: pad.stick_x,
                weapon: wa,
            };
            self.pose = Some(anim.update(&cx, dt));
        }
        if let Some(g) = self.guns[self.cur].anim.as_mut() {
            g.update(dt);
        }
        self.cam.place(&self.ch, &self.level);
        self.prev = pad;
    }

    fn weapon_anim(&self) -> WeaponAnim {
        let w = &self.guns[self.cur].w;
        let gnasher = self.cur == 1;
        WeaponAnim {
            gnasher,
            since_shot: self.since_shot,
            cock_at: self.cock.0,
            cock_len: self.cock.1,
            reloading: w.reloading > 0.0,
            reload_elapsed: w.reload_elapsed,
            reload_total: w.reload_total,
            ar_result: w.active_result as i32,
            ar_at: self.ar_at,
            ..WeaponAnim::names(gnasher)
        }
    }

    fn tick_weapons(&mut self, pad: &PadInput, dt: f32) {
        self.flash = (self.flash - dt).max(0.0);
        self.hit = (self.hit - dt).max(0.0);
        self.since_shot += dt;
        // swap (Y / Q)
        if pad.swap && !self.prev.swap && self.can_aim() {
            self.stop_loop(false);
            self.delayed.clear();
            self.cur = 1 - self.cur;
            self.guns[self.cur].w.reloading = 0.0;
            self.guns[self.cur].w.cooldown = 0.3;
            let raise = if self.cur == 0 {
                "CogRifleRaise"
            } else {
                "ShotgunRaise01"
            };
            self.play(raise, 0.8);
            self.say(
                &[
                    "ThisWillDo_Medium01",
                    "ArmedAndReady_Medium03",
                    "ReadyAndWaiting_Medium01",
                ],
                0.3,
            );
        }
        let speed = self.ch.vel.length_2d();
        let steady = self.aiming && speed < 20.0;
        let fire = pad.fire && self.can_aim();
        let reload_pressed = pad.reload && !self.prev.reload;
        let was_reloading = self.guns[self.cur].w.reloading > 0.0;
        let mag_before = self.guns[self.cur].w.mag;
        let t = self.guns[self.cur].w.tick(fire, reload_pressed, steady, dt);
        let gnasher = self.cur == 1;
        if t.reload_started {
            self.stop_loop(true);
            let dur = self.guns[self.cur].w.spec.reload_duration;
            let clip = if gnasher { "SG_Reload" } else { "AR_Reload" };
            if let Some(g) = self.guns[self.cur].anim.as_mut() {
                g.play(clip, Some(dur), 0.0);
            }
            if gnasher {
                self.play("ShotgunReloadEject01", 0.8);
                self.later("ShotgunReloadInsert01", 1.2, 0.8);
                self.later("ShotgunReloadArm01", 2.4, 0.8);
            } else {
                self.play("CogRifleReloadArm01", 0.8);
                self.later("CogRifleReloadInsert01", 1.45, 0.8);
            }
        }
        match t.active_reload {
            ActiveReload::None => {}
            r => {
                let w = &self.guns[self.cur].w;
                self.ar_at = w.reload_elapsed;
                let left = w.reload_total - w.reload_elapsed;
                self.delayed.clear();
                if r == ActiveReload::Failed {
                    let at = w.reload_elapsed;
                    if let Some(g) = self.guns[self.cur].anim.as_mut() {
                        g.play(
                            if gnasher {
                                "SG_Reload_Fail"
                            } else {
                                "AR_Reload_Fumble"
                            },
                            None,
                            at,
                        );
                    }
                    // ASSUMPTION: the fail cue's wave (ActiveReloadFailCue isn't decoded)
                    self.play("ActiveReloadChunk", 1.0);
                } else {
                    let perfect = r == ActiveReload::Perfect;
                    if let Some(g) = self.guns[self.cur].anim.as_mut() {
                        if gnasher {
                            g.play("SG_Reload_Success", Some(left), 0.0);
                        } else {
                            g.speed_up(if perfect { 2.5 } else { 1.66 });
                        }
                    }
                    self.play(
                        if perfect {
                            "ActiveReloadSuperSuccess02"
                        } else {
                            "ActiveReloadSuccess03"
                        },
                        1.0,
                    );
                    self.later(
                        if gnasher {
                            "ShotgunReloadArm01"
                        } else {
                            "CogRifleReloadInsert01"
                        },
                        left * 0.6,
                        0.8,
                    );
                }
            }
        }
        let _ = was_reloading;
        if pad.fire && !self.prev.fire && mag_before == 0 && t.shots == 0 {
            self.play("CogRifleFireEmpty04", 0.7);
        }
        for _ in 0..t.shots {
            self.fire_shot();
        }
        let reloading = self.guns[self.cur].w.reloading > 0.0;
        if self.lancer_loop.is_some()
            && (!pad.fire || self.guns[self.cur].w.mag == 0 || reloading || !self.can_aim())
        {
            self.stop_loop(true);
        }
    }

    fn muzzle(&self) -> Vec3 {
        let f = self.cam.forward();
        let p = self.ch.pos;
        Vec3::new(
            p.x + self.ch.yaw.cos() * 50.0 + f.x * 5.0,
            p.y + self.ch.yaw.sin() * 50.0 + f.y * 5.0,
            120.0 + self.ch.height(),
        )
    }

    fn fire_shot(&mut self) {
        let gnasher = self.cur == 1;
        let spec = self.guns[self.cur].w.spec.clone();
        let mult = self.guns[self.cur].w.damage_mult();
        let err = spec.aim_error_rad(self.guns[self.cur].w.accuracy);
        self.flash = 0.05;
        self.since_shot = 0.0;
        // trace from the camera (GetWeaponStartTraceLocation), started at the pawn's depth
        let f = self.cam.forward();
        let cp = self.cam.pos;
        let k = ((self.ch.pos.x - cp.x) * f.x + (self.ch.pos.y - cp.y) * f.y).max(0.0);
        let o = Vec3::new(cp.x + f.x * k, cp.y + f.y * k, cp.z + f.z * k);
        let ang = self.rng.f() * std::f32::consts::TAU;
        let rad = self.rng.f().sqrt() * err;
        let (yaw, pitch) = (
            self.cam.yaw + ang.cos() * rad,
            self.cam.pitch + ang.sin() * rad,
        );
        let dir = |y: f32, p: f32| Vec3::new(p.cos() * y.cos(), p.cos() * y.sin(), p.sin());
        let alive: Vec<(f32, f32, bool)> = self
            .targets
            .iter()
            .map(|t| (t.x, t.y, t.dead <= 0.0))
            .collect();
        let mz = self.muzzle();
        if gnasher {
            let shot = self.rng.pick(&[
                "ShotgunFirePlayer01",
                "ShotgunFirePlayer02",
                "ShotgunFirePlayer03",
            ]);
            self.play(shot, 1.0);
            // GearWeap_Shotgun: firing 0.25 × interval, then ShotgunCocking 0.75 × interval
            let iv = spec.fire_interval();
            self.cock = (iv * 0.25, iv * 0.75);
            self.later("shotguncock01", iv * 0.25 + 0.3, 0.8);
            let b = if self.rng.f() < 0.5 {
                "ShotgunAmmoBounce01"
            } else {
                "ShotgunAmmoBounce02"
            };
            self.later(b, iv * 0.25 + 0.7, 0.5);
            let tw = self.time + iv * 0.25;
            self.delayed.push((tw, "__gun_chamber".into(), iv * 0.75));
            let scale = gears_core::WeaponSpec::spread_scale(
                self.aiming,
                self.ch.time_since_evade(),
                self.ch.vel.length_2d(),
                self.ch.cfg.ground_speed,
            );
            let mut hits: Vec<(usize, u32, Vec3)> = Vec::new();
            for (i, (py, pp)) in spec.pellet_offsets(scale).into_iter().enumerate() {
                let d = dir(yaw + py, pitch + pp);
                let (t, tg) = self.level.trace(o, d, TRACE_RANGE, &alive);
                let p = o + d * t;
                if let Some(ti) = tg {
                    match hits.iter_mut().find(|h| h.0 == ti) {
                        Some(h) => h.1 += 1,
                        None => hits.push((ti, 1, p)),
                    }
                }
                if i == 0 || i % 2 == 1 {
                    self.tracers.push(Tracer {
                        from: mz,
                        to: p,
                        age: 0.0,
                    });
                }
            }
            for (ti, n, p) in hits {
                let dist = ((p.x - self.ch.pos.x).powi(2)
                    + (p.y - self.ch.pos.y).powi(2)
                    + (p.z - PAWN_HALF).powi(2))
                .sqrt();
                let tg = &self.targets[ti];
                let close =
                    ((tg.x - self.ch.pos.x).powi(2) + (tg.y - self.ch.pos.y).powi(2)).sqrt();
                // spec.damage applies the 1.2× inside 256 uu using its distance argument;
                // Gears measures that one to the target (2D) and the range bands to the hit
                let mut dmg = spec.damage(n, dist) * mult;
                if dist < 256.0 && close >= 256.0 {
                    dmg /= 1.2;
                } else if dist >= 256.0 && close < 256.0 {
                    dmg *= 1.2;
                }
                self.damage(ti, dmg.round(), p);
            }
        } else {
            if self.lancer_loop.is_none() {
                let id = self.next_loop;
                self.next_loop += 1;
                self.lancer_loop = Some(id);
                self.cues.push(Cue::Loop {
                    id,
                    name: "CogARifleFireLoopPlayer01".into(),
                    volume: 0.9,
                });
            }
            if let Some(g) = self.guns[0].anim.as_mut() {
                g.play("AR_Fire", None, 0.0);
            }
            let d = dir(yaw, pitch);
            let (t, tg) = self.level.trace(o, d, TRACE_RANGE, &alive);
            let p = o + d * t;
            self.tracers.push(Tracer {
                from: mz,
                to: p,
                age: 0.0,
            });
            if let Some(ti) = tg {
                let dist = ((p.x - self.ch.pos.x).powi(2)
                    + (p.y - self.ch.pos.y).powi(2)
                    + (p.z - PAWN_HALF).powi(2))
                .sqrt();
                self.damage(ti, (spec.damage(1, dist) * mult).round(), p);
            }
        }
        // WeaponRecoil: ASSUMPTION an upward camera kick of that many Unreal rotation units
        self.cam.pitch = (self.cam.pitch + gears_core::math::urot_to_rad(spec.recoil as i32))
            .min(60f32.to_radians());
    }

    fn damage(&mut self, ti: usize, dmg: f32, at: Vec3) {
        self.hit = 0.15;
        let lancer = self.cur == 0;
        if !lancer || self.rng.f() < 0.35 {
            self.play(
                if lancer {
                    "RifleAmmoImpactBodyFlesh01"
                } else {
                    "ShotgunAmmoImpactBodyFlesh01"
                },
                0.5,
            );
        }
        self.popups.push(Popup {
            text: format!("{dmg:.0}"),
            at,
            age: 0.0,
        });
        let t = &mut self.targets[ti];
        t.hp -= dmg;
        if t.hp <= 0.0 && t.dead <= 0.0 {
            t.hp = 0.0;
            t.dead = 3.0;
            let close = !lancer
                && ((t.x - self.ch.pos.x).powi(2) + (t.y - self.ch.pos.y).powi(2)).sqrt() < 300.0;
            self.on_kill(close);
        }
    }

    // GUD-style chatter: ASSUMPTION chance and cooldown (GUDS tables aren't decoded)
    fn say(&mut self, lines: &[&str], chance: f32) {
        if self.time - self.voice_last < 4.0 || self.rng.f() > chance {
            return;
        }
        self.voice_last = self.time;
        let l = self.rng.pick(lines);
        self.play(&format!("MarcusChatter_{l}"), 1.0);
    }

    fn on_kill(&mut self, close: bool) {
        let now = self.time;
        self.kills.retain(|&t| now - t < 2.5);
        self.kills.push(now);
        if self.kills.len() >= 2 {
            self.voice_last = -99.0;
            self.say(&["ThatsATwofer_Loud01"], 1.0);
        } else if close {
            self.say(
                &[
                    "NothingButBits_Medium01",
                    "RestInPieces_Loud01",
                    "LookMaNoFace_Medium02",
                    "AndStayDown_Medium02",
                ],
                0.6,
            );
        } else {
            self.say(
                &[
                    "GotOne_Medium01",
                    "Next_Loud01",
                    "WhosNext_Loud01",
                    "WhosNext_Medium02",
                    "EatIt_Medium01",
                    "SmokedHim_Medium02",
                    "TooUglyToLive_Medium01",
                    "ThatWasEasy_Medium01",
                    "Gotcha_Medium02",
                    "GoodNight_Loud01",
                ],
                0.6,
            );
        }
    }

    /// Movement sounds from the move log, footsteps from the step clock.
    fn movement_cues(&mut self) {
        if self.event_i > self.ch.events.len() {
            self.event_i = 0;
        }
        while self.event_i < self.ch.events.len() {
            let e = self.ch.events[self.event_i].clone();
            self.event_i += 1;
            let std = self
                .ch
                .cover_spot()
                .map(|c| c.height == CoverHeight::Standing)
                .unwrap_or(false);
            if e.to == "Run2Cover" {
                self.play("Footsteps_Marcus_ConcreteSlide01", 0.6);
            }
            if e.to == "InCover" && (e.from == "Run2Cover" || e.from == "RoadieRun") {
                let n = if std {
                    "CogBodyCoverHighIn01"
                } else if self.rng.f() < 0.5 {
                    "CogBodyCoverLowIn01"
                } else {
                    "CogBodyCoverLowIn03"
                };
                self.play(n, 0.8);
            }
            if e.from == "InCover"
                && (e.to == "PushOut" || e.to.starts_with("Evade") || e.to == "CoverSlip")
            {
                self.play(
                    if std {
                        "CogBodyCoverHighOut01"
                    } else {
                        "CogBodyCoverLowOut01"
                    },
                    0.6,
                );
            }
            if e.to.starts_with("Evade") {
                let n = self
                    .rng
                    .pick(&["CogBodyRoll01", "CogBodyRoll02", "CogBodyRoll03"]);
                self.play(n, 0.8);
            }
            if e.to == "SwatTurn" {
                let n = self.rng.pick(&["CoverTurn01", "CoverTurn03"]);
                self.play(n, 0.8);
            }
            if e.to == "Vault" {
                let n = self
                    .rng
                    .pick(&["CoverCombat_LeapOver01", "CoverCombat_LeapOver02"]);
                self.play(n, 0.8);
            }
        }
        let landed = self
            .ch
            .move_info()
            .map(|a| a.kind == MoveKind::Vault && a.elapsed >= a.land)
            .unwrap_or(false);
        if landed && !self.landed {
            self.play("CoverCombat_Recover01", 0.7);
            self.play("Footsteps_Marcus_ConcreteLand01", 0.7);
        }
        self.landed = landed;
        // the special "__gun_chamber" delayed entries start the Gnasher's chamber anim
        let now = self.time;
        if let Some(pos) = self
            .delayed
            .iter()
            .position(|d| d.1 == "__gun_chamber" && d.0 <= now)
        {
            let (_, _, dur) = self.delayed.remove(pos);
            if self.cur == 1 {
                if let Some(g) = self.guns[1].anim.as_mut() {
                    g.play("SG_Chamber", Some(dur), 0.0);
                }
            }
        }
        // two footsteps per cycle of the step clock
        let name = self.ch.state_name();
        let speed = self.ch.vel.length_2d();
        if let Some(a) = &self.anim {
            let (pl, pr) = a.phases();
            let (hl, hr) = ((pl * 2.0).floor() as i32, (pr * 2.0).floor() as i32);
            let steps = [
                "Footsteps_Marcus_Concrete01",
                "Footsteps_Marcus_Concrete02",
                "Footsteps_Marcus_Concrete03",
                "Footsteps_Marcus_Concrete04",
            ];
            if hl != self.foot.0 && name == "Walk" && speed > 40.0 {
                let n = self.rng.pick(&steps);
                self.play(n, 0.3);
            }
            if hr != self.foot.1 && name == "RoadieRun" {
                let n = self.rng.pick(&steps);
                self.play(n, 0.55);
            }
            self.foot = (hl, hr);
        }
    }

    /// Take this step's sounds.
    pub fn drain_cues(&mut self) -> Vec<Cue> {
        std::mem::take(&mut self.cues)
    }

    pub fn frame(&self) -> Frame {
        let vault = self.anim.as_ref().and_then(|a| a.vault_offset(&self.ch));
        let (height, forward_offset) = vault.unwrap_or((0.0, 0.0));
        let w = &self.guns[self.cur].w;
        let reloading = w.reloading > 0.0;
        let ((p0, p1), (_, g1)) = w.active_windows();
        let ar = w.active_result as i32;
        let pose = self.pose.clone();
        let draw_yaw = pose
            .as_ref()
            .and_then(|p| p.yaw_override)
            .unwrap_or(self.ch.yaw);
        let rig = match (&self.rig, &self.anim, &pose) {
            (Some(rt), Some(a), Some(p)) => {
                let r = rt.retarget(&a.model, &p.local);
                Some(if p.mirror { r.mirrored() } else { r })
            }
            _ => None,
        };
        Frame {
            pawn_pos: self.ch.pos,
            draw_yaw,
            height,
            forward_offset,
            pose,
            rig,
            gun: if self.cur == 0 {
                WeaponKind2::Lancer
            } else {
                WeaponKind2::Gnasher
            },
            gun_pose: self.guns[self.cur].anim.as_ref().map(|g| g.current()),
            cam_pos: self.cam.pos,
            cam_forward: self.cam.forward(),
            cam_fov: self.cam.fov,
            state: self.ch.state_name(),
            cam_mode: self.cam.mode.name,
            aiming: self.aiming,
            muzzle_flash: self.flash > 0.0,
            hud_ammo: if reloading {
                "reloading".into()
            } else {
                format!("{} / {}", w.mag, w.spare)
            },
            hud_msg: if reloading {
                ["Press reload in the zone", "Good", "Perfect", "Jammed"][ar.clamp(0, 3) as usize]
                    .into()
            } else if w.bonus > 0.0 {
                format!("Damage up {:.1} s", w.bonus)
            } else {
                String::new()
            },
            #[allow(clippy::unnecessary_lazy_evaluations)]
            reload_bar: reloading.then(|| {
                (
                    if ar != 0 {
                        self.ar_at
                    } else {
                        w.reload_elapsed
                    },
                    w.spec.reload_duration,
                    p0,
                    p1,
                    g1,
                    ar,
                )
            }),
            bonus: w.bonus > 0.0,
            hit_marker: self.hit > 0.0,
        }
    }

    pub fn target_height() -> f32 {
        TARGET_H
    }
}
