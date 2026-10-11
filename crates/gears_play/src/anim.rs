//! Animation driver: which of Marcus's clips play, at what time and weight, from the
//! movement state — then sampled, blended and layered into a skeleton pose.
//!
//! Same rules as the test range (web/range3d.html), which was checked against Gears 3's
//! own anim data: core clips blend like three.js's mixer (weighted slerp, the bind pose
//! fills the rest), `ADD_*` weapon layers apply as delta × base (pre-multiplied), aim
//! offsets (GearAnim_AimOffset profiles) pre-multiply in model space, the cover anims are
//! mirrored when facing left, cycles share one step clock per group.

use crate::glb::{Model, Path};
use crate::json::Json;
use crate::qmath::{Quat, Trs, V3};
use gears_core::{Character, MoveKind};
use std::collections::HashMap;

/// What the weapon side tells the animator this frame.
#[derive(Clone, Debug, Default)]
pub struct WeaponAnim {
    pub gnasher: bool,
    /// seconds since the last shot, and the pump (cocking) window after it
    pub since_shot: f32,
    pub cock_at: f32,
    pub cock_len: f32,
    pub reloading: bool,
    pub reload_elapsed: f32,
    pub reload_total: f32,
    /// 0 none, 1 good, 2 perfect, 3 failed
    pub ar_result: i32,
    pub ar_at: f32,
    pub reload_anim: &'static str,
    pub fail_anim: &'static str,
    pub success_anim: Option<&'static str>,
}

impl WeaponAnim {
    /// The weapon's reload clips (everything else default).
    pub fn names(gnasher: bool) -> WeaponAnim {
        WeaponAnim {
            gnasher,
            reload_anim: if gnasher {
                "ADD_SG_Reload"
            } else {
                "ADD_AR_Reload"
            },
            fail_anim: if gnasher {
                "ADD_SG_Reload_Fail"
            } else {
                "ADD_AR_Reload_Fail"
            },
            success_anim: gnasher.then_some("ADD_SG_Reload_Success"),
            ..Default::default()
        }
    }
}

/// Inputs for one animation frame.
pub struct AnimCtx<'a> {
    pub ch: &'a Character,
    pub aiming: bool,
    pub cam_pitch: f32,
    pub cam_yaw: f32,
    pub stick_x: f32,
    pub weapon: WeaponAnim,
}

/// The pose to draw.
#[derive(Clone, Debug, Default)]
pub struct PoseOut {
    /// local transform per node of the Marcus model (glTF space)
    pub local: Vec<Trs>,
    /// mirror the model (scale X by −1): cover anims when facing left
    pub mirror: bool,
    /// draw the character facing this yaw instead of the pawn's (slip, cover aim)
    pub yaw_override: Option<f32>,
}

#[derive(Clone, Debug)]
struct AimProfile {
    v_range: (f32, f32),
    /// bone → 9 quats (engine space): LU LC LD CU CC CD RU RC RD
    bones: HashMap<String, [Quat; 9]>,
}

pub struct Animator {
    pub model: Model,
    additive: Vec<bool>,
    w: Vec<f32>,
    t: Vec<f32>,
    phase_loco: f32,
    phase_roadie: f32,
    phase_cov: f32,
    last: String,
    land_at: f32,
    slip_yaw: f32,
    prev_yaw: f32,
    vault_mirror: bool,
    cover_h: &'static str,
    pop_t: f32,
    aim: HashMap<String, AimProfile>,
    aim_w: f32,
}

const COVER_STATES: [&str; 6] = [
    "InCover",
    "CoverRun",
    "CoverSlip",
    "SwatTurn",
    "PushOut",
    "Run2Cover",
];

impl Animator {
    pub fn new(model: Model) -> Self {
        let n = model.clips.len();
        let additive = model
            .clips
            .iter()
            .map(|c| c.name.starts_with("ADD_"))
            .collect();
        Self {
            model,
            additive,
            w: vec![0.0; n],
            t: vec![0.0; n],
            phase_loco: 0.0,
            phase_roadie: 0.0,
            phase_cov: 0.0,
            last: String::new(),
            land_at: -999.0,
            slip_yaw: 0.0,
            prev_yaw: 0.0,
            vault_mirror: false,
            cover_h: "Mid",
            pop_t: 0.0,
            aim: HashMap::new(),
            aim_w: 0.0,
        }
    }

    /// Load `marcus_aim.json` (tools/gears_aimoffset.py).
    pub fn load_aim(&mut self, json: &str) -> Result<usize, String> {
        let j = Json::parse(json)?;
        let profs = j.get("profiles").and_then(Json::obj).ok_or("no profiles")?;
        for (k, p) in profs {
            let vr = p
                .get("v_range")
                .map(|v| {
                    (
                        v.idx(0).map(Json::f32).unwrap_or(-1.0),
                        v.idx(1).map(Json::f32).unwrap_or(1.0),
                    )
                })
                .unwrap_or((-1.0, 1.0));
            let mut bones = HashMap::new();
            for (b, t) in p.get("bones").and_then(Json::obj).into_iter().flatten() {
                let mut qs = [Quat::IDENTITY; 9];
                for (i, d) in ["LU", "LC", "LD", "CU", "CC", "CD", "RU", "RC", "RD"]
                    .iter()
                    .enumerate()
                {
                    if let Some(v) = t.get(d) {
                        qs[i] = Quat::new(
                            v.idx(0).map(Json::f32).unwrap_or(0.0),
                            v.idx(1).map(Json::f32).unwrap_or(0.0),
                            v.idx(2).map(Json::f32).unwrap_or(0.0),
                            v.idx(3).map(Json::f32).unwrap_or(1.0),
                        );
                    }
                }
                bones.insert(b.clone(), qs);
            }
            self.aim
                .insert(k.clone(), AimProfile { v_range: vr, bones });
        }
        Ok(self.aim.len())
    }

    /// Step clocks (walk/run, roadie run), 0..1: two footsteps per cycle.
    pub fn phases(&self) -> (f32, f32) {
        (self.phase_loco, self.phase_roadie)
    }

    pub fn has(&self, n: &str) -> bool {
        self.model.clip(n).is_some()
    }
    fn len(&self, n: &str) -> f32 {
        self.model.clip(n).map(|c| c.duration).unwrap_or(0.0)
    }

    /// The vault's height and extra forward offset for drawing (the jump follows the
    /// clip's own root curve: straight up, then over).
    pub fn vault_offset(&self, ch: &Character) -> Option<(f32, f32)> {
        let a = ch.move_info().filter(|a| a.kind == MoveKind::Vault)?;
        let mut up = ch.height();
        let mut fwd = 0.0;
        let clip = if self.vault_mirror {
            "AR_Mantle_Mirrored_Start"
        } else {
            "AR_Mantle_Start"
        };
        if let Some(cv) = self.model.clip(clip).and_then(|c| c.root_curve.as_ref()) {
            if a.elapsed < a.split && !cv.is_empty() {
                let fr = (a.elapsed * ch.cfg.vault_speed_mod * 30.0).min((cv.len() - 1) as f32);
                let i = fr.floor() as usize;
                let j = (i + 1).min(cv.len() - 1);
                let k = fr - i as f32;
                let cf = cv[i][0] + (cv[j][0] - cv[i][0]) * k;
                let cu = cv[i][1] + (cv[j][1] - cv[i][1]) * k;
                let scale = (a.mid - a.start).flat().length() / ch.cfg.vault_jump_travel;
                let done = (ch.pos - a.start).dot(a.dir);
                fwd = cf * scale - done;
                up = cu;
            }
        }
        Some((up, fwd))
    }

    /// Advance by `dt` and build this frame's pose.
    pub fn update(&mut self, cx: &AnimCtx, dt: f32) -> PoseOut {
        let targets = self.targets(cx, dt);
        let name = cx.ch.state_name();
        // weights ease toward their targets (0.06 s), times are driven
        let blend = 1.0 - (-dt / 0.06).exp();
        for (i, c) in self.model.clips.iter().enumerate() {
            let want = targets.get(&c.name);
            self.w[i] += (want.map(|w| w.0).unwrap_or(0.0) - self.w[i]) * blend;
            if let Some(&(_, t)) = want {
                self.t[i] = t.clamp(0.0, c.duration);
            }
        }
        let mut local = self.blend_core();
        self.apply_additives(&mut local);
        self.apply_aim(&mut local, cx, &name, dt);

        let cover_aim = name == "InCover" && cx.aiming && self.cover_h != "Mid";
        let mirror = if cover_aim {
            false
        } else if name == "Vault" {
            self.vault_mirror
                && cx
                    .ch
                    .move_info()
                    .map(|a| a.elapsed < a.split)
                    .unwrap_or(false)
        } else {
            cx.ch.mirrored && COVER_STATES.contains(&name.as_str())
        };
        let yaw_override = if name == "CoverSlip" {
            Some(self.slip_yaw)
        } else if cover_aim {
            Some(cx.cam_yaw)
        } else {
            None
        };
        self.prev_yaw = cx.ch.yaw;
        PoseOut {
            local,
            mirror,
            yaw_override,
        }
    }

    /// realTargets: clip name → (weight, time).
    fn targets(&mut self, cx: &AnimCtx, dt: f32) -> HashMap<String, (f32, f32)> {
        let ch = cx.ch;
        let name = ch.state_name();
        let a = ch.move_info();
        if let Some(c) = ch.cover_spot() {
            self.cover_h = if c.height == gears_core::CoverHeight::Standing {
                "Std"
            } else {
                "Mid"
            };
        }
        let h = self.cover_h;
        if self.last == "Run2Cover" && name == "InCover" {
            self.land_at = ch.time;
        }
        if name == "RoadieRun" && self.last != "RoadieRun" {
            self.land_at = -999.0;
        }
        if name == "CoverSlip" && self.last != "CoverSlip" {
            self.slip_yaw = self.prev_yaw;
        }
        if name == "Vault" && self.last != "Vault" {
            self.vault_mirror = ch.mirrored;
        }
        self.last = name.clone();
        let mut out: HashMap<String, (f32, f32)> = HashMap::new();
        let mut set = |me: &Self, n: &str, w: f32, t: f32| {
            if me.has(n) {
                out.insert(n.to_string(), (w, t));
            }
        };
        let speed = ch.vel.length_2d();
        let cfg = &ch.cfg;
        match name.as_str() {
            "Walk" => {
                let s = speed;
                let idle = (1.0 - s / 60.0).max(0.0);
                let run = ((s - 120.0) / 160.0).clamp(0.0, 1.0);
                let (ww, wr) = ((1.0 - idle) * (1.0 - run), (1.0 - idle) * run);
                set(self, "Idle_Core", idle, 0.0);
                set(
                    self,
                    if cx.aiming {
                        "ADD_AR_Idle_Ready_Aim"
                    } else {
                        "ADD_AR_Idle_Ready"
                    },
                    idle,
                    0.0,
                );
                let p = self.phase_loco;
                let mut fw = 0.0;
                if cx.aiming && self.has("AR_Walk_Fwd_Aim_Core") {
                    let (fx, fy) = (ch.yaw.cos(), ch.yaw.sin());
                    let vl = s.max(1e-3);
                    let fwd = (ch.vel.x * fx + ch.vel.y * fy) / vl;
                    let rt = (-ch.vel.x * fy + ch.vel.y * fx) / vl;
                    let parts = [
                        ("Fwd", fwd.max(0.0)),
                        ("Bwd", (-fwd).max(0.0)),
                        ("Rt", rt.max(0.0)),
                        ("Lt", (-rt).max(0.0)),
                    ];
                    let sum: f32 = parts.iter().map(|p| p.1).sum::<f32>().max(1e-6);
                    for (d, w) in parts {
                        let n = format!("AR_Walk_{d}_Aim_Core");
                        set(self, &n, ww * w / sum, p * self.len(&n));
                        fw += (s / 150.0).max(0.6) / self.len(&n).max(1e-3) * w / sum;
                    }
                    set(self, "ADD_AR_Walk_Aim", ww, p * self.len("ADD_AR_Walk_Aim"));
                } else {
                    let n = "AR_Walk_Fwd_Ready_Core";
                    set(self, n, ww, p * self.len(n));
                    let add = if cx.aiming {
                        "ADD_AR_Walk_Aim"
                    } else {
                        "ADD_AR_Walk_Ready"
                    };
                    set(self, add, ww, p * self.len(add));
                    fw = (s / 160.0).max(0.6) / self.len(n).max(1e-3);
                }
                let n = "AR_Run_Fwd_Ready_Core";
                set(self, n, wr, p * self.len(n));
                set(
                    self,
                    "ADD_AR_Run_Ready",
                    wr,
                    p * self.len("ADD_AR_Run_Ready"),
                );
                let fr = (s / 300.0).max(0.7) / self.len(n).max(1e-3);
                if ww + wr > 1e-3 {
                    self.phase_loco =
                        (self.phase_loco + dt * (ww * fw + wr * fr) / (ww + wr)) % 1.0;
                }
            }
            "RoadieRun" => {
                let p = self.phase_roadie;
                set(
                    self,
                    "Roadie_Run_Core",
                    1.0,
                    p * self.len("Roadie_Run_Core"),
                );
                set(
                    self,
                    "ADD_AR_RoadieRun_Fwd",
                    1.0,
                    p * self.len("ADD_AR_RoadieRun_Fwd"),
                );
                self.phase_roadie = (p + dt / self.len("Roadie_Run_Core").max(1e-3)) % 1.0;
                // AnimSetMarcus_Overlay steering lean
                if cx.stick_x > 0.05 {
                    set(
                        self,
                        "ADD_Overlay_AR_RoadieRun_Lean_R",
                        cx.stick_x.min(1.0),
                        0.0,
                    );
                } else if cx.stick_x < -0.05 {
                    set(
                        self,
                        "ADD_Overlay_AR_RoadieRun_Lean_L",
                        (-cx.stick_x).min(1.0),
                        0.0,
                    );
                }
            }
            "Run2Cover" => {
                let a = a.unwrap();
                let n = format!("AR_Run_Fwd2Cov_{h}_Start");
                set(
                    self,
                    &n,
                    1.0,
                    (a.elapsed / a.duration).min(1.0) * self.len(&n),
                );
            }
            "InCover"
                if cx.aiming && h == "Mid" && self.has("AR_Cov_Mid_Idle_Ready2DownSights_Aim") =>
            {
                let up = "AR_Cov_Mid_Idle_Ready2DownSights_Aim";
                self.pop_t = (self.pop_t + dt).min(self.len(up));
                set(self, up, 1.0, self.pop_t);
            }
            "InCover" if cx.aiming => {
                set(self, "Idle_Core", 1.0, 0.0);
                set(self, "ADD_AR_Idle_Ready_Aim", 1.0, 0.0);
            }
            "InCover" => {
                self.pop_t = 0.0;
                let since = ch.time - self.land_at;
                let stop = format!("AR_Run_Fwd2Cov_{h}_Stop");
                let mv = (speed / 60.0).min(1.0);
                if since < self.len(&stop) && speed < 10.0 {
                    set(self, &stop, 1.0, since);
                } else {
                    set(self, &format!("Cov_{h}_Idle_Core"), 1.0 - mv, 0.0);
                    set(self, &format!("ADD_AR_Cov_{h}_Idle_Ready"), 1.0, 0.0);
                    if mv > 0.001 {
                        let n = format!("Cov_{h}_Run_Core");
                        set(self, &n, mv, self.phase_cov * self.len(&n));
                        self.phase_cov = (self.phase_cov
                            + dt * (speed / 300.0).max(0.5) / self.len(&n).max(1e-3))
                            % 1.0;
                    }
                }
            }
            "CoverRun" => {
                let n = format!("Cov_{h}_Run_Core");
                let p = self.phase_cov;
                set(self, &n, 1.0, p * self.len(&n));
                set(
                    self,
                    "ADD_AR_Cov_RoadieRun",
                    1.0,
                    p * self.len("ADD_AR_Cov_RoadieRun"),
                );
                self.phase_cov = (p + dt * cfg.cover_run_speed_mod / self.len(&n).max(1e-3)) % 1.0;
            }
            "CoverSlip" => {
                let a = a.unwrap();
                set(
                    self,
                    &format!("AR_Cov_{h}_Slip"),
                    1.0,
                    a.elapsed * cfg.slip_speed_mod,
                );
            }
            "SwatTurn" => {
                let a = a.unwrap();
                let (st, en, rate) = (
                    format!("AR_Cov_{h}_SwatTurn_Start"),
                    format!("AR_Cov_{h}_SwatTurn_End"),
                    cfg.swat_anim_rate,
                );
                if a.elapsed < a.split {
                    set(self, &st, 1.0, (a.elapsed * rate).min(self.len(&st)));
                } else {
                    let end_time = cfg.swat_end_anim_length / rate;
                    let left = ((a.duration - a.split) / end_time).min(1.0);
                    let k = if a.duration > a.split {
                        (a.elapsed - a.split) / (a.duration - a.split)
                    } else {
                        1.0
                    };
                    set(self, &en, 1.0, ((1.0 - left) + k * left) * self.len(&en));
                }
            }
            "Vault" => {
                let a = a.unwrap();
                let m = if self.vault_mirror { "Mirrored_" } else { "" };
                let rate = cfg.vault_speed_mod;
                if a.elapsed < a.split {
                    set(self, &format!("AR_Mantle_{m}Start"), 1.0, a.elapsed * rate);
                } else if a.elapsed < a.land {
                    let span = (cfg.vault_rise / cfg.gravity).sqrt() + 0.07;
                    let mid = format!("AR_Mantle_{m}Mid");
                    set(
                        self,
                        &mid,
                        1.0,
                        ((a.elapsed - a.split) / span).min(1.0) * self.len(&mid),
                    );
                } else {
                    set(
                        self,
                        &format!("AR_Mantle_{m}End"),
                        1.0,
                        (a.elapsed - a.land) * rate,
                    );
                }
            }
            "PushOut" => {
                let a = a.unwrap();
                set(
                    self,
                    &format!("AR_Cov_{h}_Idle2Idle_Ready_Aim"),
                    1.0,
                    a.elapsed * cfg.push_out_speed_mod,
                );
            }
            n if n.starts_with("Evade") => {
                let a = a.unwrap();
                let d = &n[5..];
                let rate = cfg.evade_speed_mod;
                if a.elapsed < a.split {
                    set(self, &format!("AR_Evade_{d}_Start"), 1.0, a.elapsed * rate);
                } else {
                    set(
                        self,
                        &format!("AR_Evade_{d}_End"),
                        1.0,
                        (a.elapsed - a.split) * rate,
                    );
                }
            }
            _ => {}
        }
        // Gnasher pump: the chamber layer replaces the hold layer over the cocking time
        let w = &cx.weapon;
        if w.gnasher
            && w.since_shot >= w.cock_at
            && w.since_shot < w.cock_at + w.cock_len
            && self.has("ADD_SG_Idle_Ready_Chamber")
            && (name == "Walk" || name == "InCover")
        {
            out.retain(|k, _| !k.starts_with("ADD_"));
            let n = "ADD_SG_Idle_Ready_Chamber";
            out.insert(
                n.into(),
                (1.0, (w.since_shot - w.cock_at) / w.cock_len * self.len(n)),
            );
        }
        // reloading: the weapon's reload layer replaces the hold / aim layer
        if w.reloading && self.has(w.reload_anim) {
            out.retain(|k, _| !k.starts_with("ADD_"));
            if w.ar_result == 3 && self.has(w.fail_anim) {
                out.insert(
                    w.fail_anim.into(),
                    (1.0, w.reload_elapsed.min(self.len(w.fail_anim))),
                );
            } else if (1..3).contains(&w.ar_result)
                && w.success_anim.map(|s| self.has(s)).unwrap_or(false)
            {
                let s = w.success_anim.unwrap();
                out.insert(
                    s.into(),
                    (1.0, (w.reload_elapsed - w.ar_at).min(self.len(s))),
                );
            } else {
                out.insert(
                    w.reload_anim.into(),
                    (
                        1.0,
                        (w.reload_elapsed / w.reload_total.max(1e-3)).min(1.0)
                            * self.len(w.reload_anim),
                    ),
                );
            }
        }
        out
    }

    /// Core clips: weighted slerp / lerp in clip order, the bind pose fills weight < 1.
    fn blend_core(&self) -> Vec<Trs> {
        let bind = self.model.bind_pose();
        let n = bind.len();
        let mut rq = vec![Quat::IDENTITY; n];
        let mut rw = vec![0.0f32; n];
        let mut tp = vec![V3::ZERO; n];
        let mut tw = vec![0.0f32; n];
        for (ci, c) in self.model.clips.iter().enumerate() {
            let w = if self.w[ci] < 0.002 { 0.0 } else { self.w[ci] };
            if self.additive[ci] || w <= 0.0 {
                continue;
            }
            let t = self.t[ci];
            for tr in &c.tracks {
                match tr.path {
                    Path::Rotation => {
                        let q = tr.rot(t);
                        let acc = rw[tr.node];
                        rq[tr.node] = if acc == 0.0 {
                            q
                        } else {
                            rq[tr.node].slerp(q, w / (acc + w))
                        };
                        rw[tr.node] = acc + w;
                    }
                    Path::Translation => {
                        let v = tr.pos(t);
                        let acc = tw[tr.node];
                        tp[tr.node] = if acc == 0.0 {
                            v
                        } else {
                            tp[tr.node].lerp(v, w / (acc + w))
                        };
                        tw[tr.node] = acc + w;
                    }
                }
            }
        }
        let mut out = bind.clone();
        for i in 0..n {
            if rw[i] > 0.0 {
                out[i].r = if rw[i] < 1.0 {
                    rq[i].slerp(bind[i].r, 1.0 - rw[i])
                } else {
                    rq[i]
                };
            }
            if tw[i] > 0.0 {
                out[i].t = if tw[i] < 1.0 {
                    tp[i].lerp(bind[i].t, 1.0 - tw[i])
                } else {
                    tp[i]
                };
            }
        }
        out
    }

    /// ADD_* layers: rotation pre-multiplied (delta × base), translation added.
    fn apply_additives(&self, local: &mut [Trs]) {
        for (ci, c) in self.model.clips.iter().enumerate() {
            let w = self.w[ci];
            if !self.additive[ci] || w < 0.002 {
                continue;
            }
            let t = self.t[ci].min(c.duration);
            for tr in &c.tracks {
                match tr.path {
                    Path::Rotation => {
                        let mut q = tr.rot(t);
                        if w < 0.999 {
                            q = Quat::IDENTITY.slerp(q, w);
                        }
                        local[tr.node].r = q.mul(local[tr.node].r);
                    }
                    Path::Translation => {
                        local[tr.node].t = local[tr.node].t.add(tr.pos(t).mul(w));
                    }
                }
            }
        }
    }

    /// Aim offsets: bilinear over the 3×3 grid, pre-multiplied in model space.
    fn apply_aim(&mut self, local: &mut [Trs], cx: &AnimCtx, name: &str, dt: f32) {
        let cover_mid = name == "InCover" && cx.aiming && self.cover_h == "Mid";
        let want = if cx.aiming && (name == "Walk" || cover_mid) {
            1.0
        } else {
            0.0
        };
        self.aim_w += (want - self.aim_w) * (1.0 - (-dt / 0.1).exp());
        let Some(prof) = self.aim.get(if cover_mid { "cover_mid" } else { "stand" }) else {
            return;
        };
        if self.aim_w < 0.01 {
            return;
        }
        // ASSUMPTION: Aim.Y = view pitch / 90°, Aim.X = 0 (the pawn faces the aim)
        let y = (cx.cam_pitch / std::f32::consts::FRAC_PI_2).clamp(prof.v_range.0, prof.v_range.1);
        let orig = self.model.model_space(local);
        let mut next: Vec<Option<Quat>> = vec![None; local.len()];
        for &i in &self.model.order {
            let nd = &self.model.nodes[i];
            let parent_new = nd.parent.and_then(|p| next[p]);
            let key = if prof.bones.contains_key(&nd.name) {
                Some(nd.name.clone())
            } else {
                nd.name.strip_prefix("b_").map(|r| format!("B_{r}"))
            };
            if let Some(qs) = key.and_then(|k| prof.bones.get(&k)) {
                let target = aim_quat(qs, 0.0, y)
                    .slerp(Quat::IDENTITY, 1.0 - self.aim_w)
                    .mul(orig[i].r);
                let pq = parent_new
                    .unwrap_or_else(|| nd.parent.map(|p| orig[p].r).unwrap_or(Quat::IDENTITY));
                local[i].r = pq.conj().mul(target);
                next[i] = Some(target);
            } else if let Some(pn) = parent_new {
                next[i] = Some(pn.mul(local[i].r));
            }
        }
    }
}

/// engine → skeleton frame: found by test, the inverse rotation (aim up raises the barrel)
fn map_q(q: Quat) -> Quat {
    Quat::new(-q.x, -q.y, -q.z, q.w)
}

fn aim_quat(qs: &[Quat; 9], x: f32, y: f32) -> Quat {
    // index: L* = 0..3, C* = 3..6, R* = 6..9; U = +0, C = +1, D = +2
    let col = |c: usize, r: usize| map_q(qs[c * 3 + r]);
    let (c0, c1) = (1usize, if x >= 0.0 { 2 } else { 0 });
    let (r0, r1) = (1usize, if y >= 0.0 { 0 } else { 2 });
    let (fx, fy) = (x.abs(), y.abs());
    let a = col(c0, r0).slerp(col(c0, r1), fy);
    let b = col(c1, r0).slerp(col(c1, r1), fy);
    a.slerp(b, fx)
}

/// Gun anims (the Lancer's / Gnasher's own skeleton): one clip at a time, by duration.
pub struct GunAnimator {
    pub model: Model,
    clip: Option<usize>,
    time: f32,
    rate: f32,
    last: Vec<Trs>,
}

impl GunAnimator {
    pub fn new(model: Model) -> Self {
        let last = model.bind_pose();
        Self {
            model,
            clip: None,
            time: 0.0,
            rate: 1.0,
            last,
        }
    }
    /// Play `name` from `start`, stretched to last `dur` seconds (PlayWeaponAnimByDuration).
    pub fn play(&mut self, name: &str, dur: Option<f32>, start: f32) {
        if let Some(&i) = self.model.clip_by_name.get(name) {
            let len = self.model.clips[i].duration;
            self.clip = Some(i);
            self.time = start;
            self.rate = dur
                .map(|d| if d > 0.0 { len / d } else { 1.0 })
                .unwrap_or(1.0);
        }
    }
    pub fn speed_up(&mut self, k: f32) {
        self.rate *= k;
    }
    pub fn current(&self) -> Vec<Trs> {
        self.last.clone()
    }
    pub fn update(&mut self, dt: f32) -> Vec<Trs> {
        let mut local = self.model.bind_pose();
        if let Some(i) = self.clip {
            let c = &self.model.clips[i];
            self.time = (self.time + dt * self.rate).min(c.duration);
            for tr in &c.tracks {
                match tr.path {
                    Path::Rotation => local[tr.node].r = tr.rot(self.time),
                    Path::Translation => local[tr.node].t = tr.pos(self.time),
                }
            }
        }
        self.last = local.clone();
        local
    }
}
