//! The character: Gears 3 player movement rebuilt from the rules in docs/RE_NOTES.md.
//!
//! Structure mirrors Gears: a base locomotion (walking, or sitting in cover) plus at
//! most one active *special move* (`GearPawn.SpecialMove`) and one queued move
//! (`PendingSpecialMoveStruct`). Requests follow `GearPawn.DoSpecialMove`:
//! override now → else chain (queue until the current move ends) → else drop.

use crate::config::Config;
use crate::math::{angle_diff, urot_to_rad, Vec3};
use crate::world::{CoverSpot, World};

/// One frame of player input, Xbox 360 default layout.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    /// Left stick right, -1..1.
    pub stick_x: f32,
    /// Left stick up (forward), -1..1.
    pub stick_y: f32,
    /// Camera yaw in radians (stick directions are camera-relative).
    pub cam_yaw: f32,
    /// The A button (roadie run / cover / evade).
    pub a: bool,
    /// Left trigger: aiming (`GearPC.bIsTargeting`).
    pub aim: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveKind {
    RoadieRun,
    Run2Cover,
    CoverRun,
    CoverSlip,
    EvadeFwd,
    EvadeBwd,
    EvadeLt,
    EvadeRt,
    /// `SM_PushOutOfCover`: stick held away from the wall → step back off it.
    PushOut,
    /// `SM_StdLvlSwatTurn`: dash across a gap to the next cover.
    SwatTurn,
    /// `SM_MidLvlJumpOver` (`GSM_MantleOverCoverBase`): vault over waist-high cover —
    /// jump (root motion), fall (physics), land.
    Vault,
}

impl MoveKind {
    fn is_evade(self) -> bool {
        matches!(
            self,
            Self::EvadeFwd | Self::EvadeBwd | Self::EvadeLt | Self::EvadeRt
        )
    }

    /// `<Current>.CanChainMove(next)`: queue `next` until this move ends.
    fn can_chain(self, next: MoveKind) -> bool {
        use MoveKind::*;
        match self {
            // GSM_RoadieRun: Run2MidCov, Run2StdCov (+ MantleDown, ladders: not modelled)
            RoadieRun => next == Run2Cover,
            // GSM_Run2Cover: CoverSlip, CoverRun, MidLvlJumpOver, SwatTurn (+ MantleKick,
            // MantleUp: not modelled)
            Run2Cover => matches!(next, CoverSlip | CoverRun | SwatTurn | Vault),
            // GSM_CoverSlip: RoadieRun, Run2MidCov, Run2StdCov
            CoverSlip => matches!(next, RoadieRun | Run2Cover),
            // GSM_CoverRun: StdLvlSwatTurn
            CoverRun => next == SwatTurn,
            // GSM_StdLvlSwatTurn: CoverSlip, MidLvlJumpOver (+ MantleKick: not modelled)
            SwatTurn => matches!(next, CoverSlip | Vault),
            // GSM_MantleOverCoverBase: RoadieRun, Run2MidCov, Run2StdCov
            Vault => matches!(next, RoadieRun | Run2Cover),
            // GSM_Evade: RoadieRun, Run2MidCov, Run2StdCov
            _ if self.is_evade() => matches!(next, RoadieRun | Run2Cover),
            // GSM_PushOutOfCover: anything but MidLvlJumpOver / MantleKick
            PushOut => next != Vault,
            _ => false,
        }
    }

    /// `<Current>.CanOverrideMoveWith(next)`: let `next` interrupt right now.
    fn can_override_with(self, next: MoveKind) -> bool {
        // GSM_Evade: RoadieRun, Run2MidCov, Run2StdCov
        (self.is_evade() && matches!(next, MoveKind::RoadieRun | MoveKind::Run2Cover))
            // SM_PushOutOfCover is treated like a Move2Idle transition: GearPC
            // (CanTryToRunToCover, CanEvade, UpdatePlayerPosture) and GSM_Evade exempt it
            // from every "busy" check; owner confirms full control off the wall.
            || self == MoveKind::PushOut
    }

    /// `<Next>.CanOverrideSpecialMove(current)`.
    fn overrides(next: MoveKind, current: MoveKind) -> bool {
        // GSM_CoverSlip overrides CoverRun
        next == MoveKind::CoverSlip && current == MoveKind::CoverRun
    }
}

/// Read-only view of the running special move, for animation and camera code.
#[derive(Clone, Copy, Debug)]
pub struct MoveInfo {
    pub kind: MoveKind,
    pub elapsed: f32,
    pub duration: f32,
    /// two-part moves: when the second part starts; vault: end of the jump
    pub split: f32,
    /// vault: when the fall lands
    pub land: f32,
    pub start: Vec3,
    pub mid: Vec3,
    pub target: Vec3,
    pub dir: Vec3,
}

/// Something that happened, for logs and tests: `[GEARS] t=… from -> to (reason)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub time: f32,
    pub from: String,
    pub to: String,
    pub reason: &'static str,
}

#[derive(Clone, Copy, Debug)]
struct Active {
    kind: MoveKind,
    /// `time` when the move started; it first advances on the next tick.
    started: f32,
    elapsed: f32,
    /// Total length for timed moves (slide, slip, evade).
    duration: f32,
    start: Vec3,
    target: Vec3,
    dir: Vec3,
    /// Evade: already ran the "0.2 s before the end" chain check.
    chain_checked: bool,
    /// Run2Cover: how long the stick has been held back / left / right.
    hold_back: f32,
    hold_left: f32,
    hold_right: f32,
    /// Run2Cover: `bEnableJoystickMonitoring` (one decision per slide).
    monitoring: bool,
    /// Two-part moves (evade start/end anim, swat dash/end anim): `mid` is reached at
    /// `split` seconds, then on to `target` by `duration`.
    split: f32,
    mid: Vec3,
    /// Vault: the jump rises until `up`, moves forward from `up` to `over`, ends at
    /// `split` (at `mid`); the fall lands at `land` (at `target`).
    up: f32,
    over: f32,
    land: f32,
}

#[derive(Clone, Copy, Debug)]
struct InCover {
    spot: CoverSpot,
    /// `GearPawn.LastCoverTime`
    since: f32,
}

pub struct Character {
    pub cfg: Config,
    pub pos: Vec3,
    pub vel: Vec3,
    /// Facing yaw, radians.
    pub yaw: f32,
    pub time: f32,
    /// Facing left along the cover (`GearPawn.bIsMirrored`).
    pub mirrored: bool,
    cover: Option<InCover>,
    active: Option<Active>,
    pending: Option<MoveKind>,
    pending_cover: Option<CoverSpot>,
    /// Swat turn destination found when the swat was asked for.
    pending_swat: Option<CoverSpot>,
    /// Vault distance (`MantleDistance`) found when the vault was asked for.
    pending_vault: Option<f32>,
    /// `TransitionFromSwatToRoadieRun` timer deadline.
    swat_roadie_timer: Option<f32>,
    /// `TryToRoadieRun` timer deadline (`GearPC.RoadieRunTimer`).
    roadie_timer: Option<f32>,
    prev_a: bool,
    /// `GearPC.BreakFromCoverHoldTimer`
    break_hold: f32,
    last_move_end: f32,
    last_evade_time: f32,
    pub events: Vec<Event>,
}

impl Character {
    pub fn new(cfg: Config, pos: Vec3, yaw: f32) -> Self {
        Self {
            cfg,
            pos,
            vel: Vec3::ZERO,
            yaw,
            time: 0.0,
            mirrored: false,
            cover: None,
            active: None,
            pending: None,
            pending_cover: None,
            pending_swat: None,
            pending_vault: None,
            swat_roadie_timer: None,
            roadie_timer: None,
            prev_a: false,
            break_hold: 0.0,
            last_move_end: -999.0,
            last_evade_time: -999.0,
            events: Vec::new(),
        }
    }

    /// Short name of the current state, e.g. "Walk", "InCover", "RoadieRun".
    pub fn state_name(&self) -> String {
        match (&self.active, &self.cover) {
            (Some(a), _) => format!("{:?}", a.kind),
            (None, Some(_)) => "InCover".into(),
            (None, None) => "Walk".into(),
        }
    }

    pub fn active_move(&self) -> Option<MoveKind> {
        self.active.map(|a| a.kind)
    }

    pub fn move_info(&self) -> Option<MoveInfo> {
        self.active.map(|a| MoveInfo {
            kind: a.kind,
            elapsed: a.elapsed,
            duration: a.duration,
            split: a.split,
            land: a.land,
            start: a.start,
            mid: a.mid,
            target: a.target,
            dir: a.dir,
        })
    }

    /// At the end of the cover you're facing (within `tolerance` uu)?
    pub fn is_at_cover_edge(&self, tolerance: f32) -> bool {
        self.at_cover_edge(tolerance)
    }

    pub fn cover_spot(&self) -> Option<CoverSpot> {
        self.cover.map(|c| c.spot)
    }

    /// Seconds since the last evade ended (the Gnasher's spread depends on it).
    pub fn time_since_evade(&self) -> f32 {
        self.time - self.last_evade_time
    }

    /// Height of the pawn above the ground it started on (only a vault leaves it):
    /// rises through the jump's up part, then falls under gravity.
    pub fn height(&self) -> f32 {
        let Some(a) = self.active.filter(|a| a.kind == MoveKind::Vault) else {
            return 0.0;
        };
        let rise = self.cfg.vault_rise;
        if a.elapsed < a.up {
            rise * a.elapsed / a.up
        } else if a.elapsed < a.split {
            rise
        } else {
            let t = (a.elapsed - a.split).min(a.land - a.split);
            (rise - 0.5 * self.cfg.gravity * t * t).max(0.0)
        }
    }

    fn log(&mut self, from: String, reason: &'static str) {
        let to = self.state_name();
        self.events.push(Event {
            time: self.time,
            from,
            to,
            reason,
        });
    }

    /// Stick in world space (camera-relative), and its raw magnitude.
    fn stick_world(&self, i: &Input) -> Vec3 {
        let fwd = Vec3::from_yaw(i.cam_yaw);
        // Right = forward rotated +90° exactly (no trig rounding at the 0.5 thresholds).
        let right = Vec3::new(-fwd.y, fwd.x, 0.0);
        fwd * i.stick_y + right * i.stick_x
    }

    /// `GearPlayerInput.RemapControlsByRotation` in cover: the stick relative to the
    /// cover (`up` = toward the wall, `right` = along it). Camera-relative elsewhere.
    fn remapped_stick(&self, i: &Input) -> (f32, f32) {
        match self.cover {
            Some(c) => {
                let w = self.stick_world(i);
                (w.dot(c.spot.right()), w.dot(-c.spot.normal))
            }
            None => (i.stick_x, i.stick_y),
        }
    }

    fn stick_len(i: &Input) -> f32 {
        (i.stick_x * i.stick_x + i.stick_y * i.stick_y).sqrt()
    }

    // ------------------------------------------------------------------ tick

    pub fn tick(&mut self, input: Input, dt: f32, world: &dyn World) {
        self.time += dt;
        let pressed = input.a && !self.prev_a;
        let released = !input.a && self.prev_a;
        self.prev_a = input.a;

        if pressed {
            self.on_a_pressed(&input, world);
        }
        if released {
            self.on_a_released(&input, world);
        }
        if let Some(deadline) = self.roadie_timer {
            if self.time >= deadline {
                self.roadie_timer = None;
                self.try_to_roadie_run(&input);
            }
        }
        if let Some(deadline) = self.swat_roadie_timer {
            if self.time >= deadline {
                self.swat_roadie_timer = None;
                self.transition_from_swat_to_roadie(&input);
            }
        }

        self.tick_active(&input, dt, world);
        self.roadie_auto_cover(&input, world);
        self.check_break_from_cover(&input, dt);

        let pushing_out = self.active.map(|a| a.kind == MoveKind::PushOut);
        if pushing_out != Some(false) {
            self.tick_locomotion(&input, dt);
        }
    }

    // --------------------------------------------------------- button logic

    /// `GearPC.PlayerWalking.TryASpecialMove` / `PlayerTakingCover.TryASpecialMove`.
    fn on_a_pressed(&mut self, input: &Input, world: &dyn World) {
        // Mid swat turn: A starts the 0.2 s swat → roadie check.
        if self
            .active
            .map(|a| a.kind == MoveKind::SwatTurn)
            .unwrap_or(false)
        {
            if self.swat_roadie_timer.is_none() {
                self.swat_roadie_timer = Some(self.time + self.cfg.swat_to_roadie_delay);
            }
            return;
        }
        if self.cover.is_some() {
            // Monkey moves first, in Gears' order: (MantleUp) → CoverSlip →
            // (MantleKick) → MidLvlJumpOver → SwatTurn. Bracketed ones: not modelled.
            if self.can_cover_slip(input) {
                self.request(MoveKind::CoverSlip, "A in cover: cover slip");
                return;
            }
            if let Some(dist) = self.can_vault(input, world) {
                self.pending_vault = Some(dist);
                if self.request(MoveKind::Vault, "A in cover: vault") {
                    return;
                }
                self.pending_vault = None;
            }
            if let Some(target) = self.can_swat_turn(input, world) {
                self.pending_swat = Some(target);
                if self.request(MoveKind::SwatTurn, "A in cover: swat turn") {
                    return;
                }
                self.pending_swat = None;
            }
            // At the cover edge: try an evade out.
            if self.at_cover_edge(0.5) && self.try_to_evade(input, world) {
                return;
            }
        }
        // Otherwise: start the roadie timer (A held this long → roadie run).
        // PlayerWalking.TryASpecialMove: never while targeting.
        if self.cover.is_none() && input.aim {
            return;
        }
        self.roadie_timer = Some(self.time + self.cfg.roadie_hold_time);
    }

    /// `GearPlayerInput.HandleActionButton(bPressed=false)`.
    fn on_a_released(&mut self, input: &Input, world: &dyn World) {
        if let Some(a) = self.active {
            if matches!(a.kind, MoveKind::RoadieRun | MoveKind::CoverRun) {
                self.end_move("A released");
                self.roadie_timer = None;
                return;
            }
        }
        let tapped = self.roadie_timer.is_some();
        if self.cover.is_some() {
            if tapped {
                self.try_to_evade(input, world);
            }
            return;
        }
        if tapped {
            self.roadie_timer = None;
            if !(self.can_try_to_run_to_cover()
                && self.try_to_run_to_cover(input, world, 1.0, false))
            {
                self.try_to_evade(input, world);
            }
        }
    }

    /// `GearPC.TryToRoadieRun`.
    fn try_to_roadie_run(&mut self, input: &Input) {
        if !input.a || self.active.map(|a| a.kind.is_evade()).unwrap_or(false) {
            return;
        }
        if self.cover.is_none() {
            self.request(MoveKind::RoadieRun, "A held");
        } else {
            // RoadieRun can't start from cover (InternalCanDoSpecialMove) → CoverRun.
            self.request(MoveKind::CoverRun, "A held in cover");
        }
    }

    // ------------------------------------------------------- move requests

    fn can_do(&self, kind: MoveKind, input: Option<&Input>) -> bool {
        match kind {
            MoveKind::RoadieRun => self.cover.is_none(),
            MoveKind::Run2Cover => self.pending_cover.is_some(),
            MoveKind::CoverRun | MoveKind::PushOut => self.cover.is_some(),
            MoveKind::SwatTurn => self.cover.is_some() && self.pending_swat.is_some(),
            MoveKind::Vault => self.cover.is_some() && self.pending_vault.is_some(),
            MoveKind::CoverSlip => {
                self.cover.is_some() && input.map(|i| self.can_cover_slip(i)).unwrap_or(true)
            }
            _ => true,
        }
    }

    /// `GearPawn.DoSpecialMove` request logic.
    fn request(&mut self, kind: MoveKind, reason: &'static str) -> bool {
        if let Some(cur) = self.active {
            if cur.kind.can_override_with(kind) || MoveKind::overrides(kind, cur.kind) {
                self.start_move(kind, reason);
                return true;
            }
            if cur.kind.can_chain(kind) {
                self.pending = Some(kind);
                return true;
            }
            return false;
        }
        if !self.can_do(kind, None) {
            return false;
        }
        self.start_move(kind, reason);
        true
    }

    fn start_move(&mut self, kind: MoveKind, reason: &'static str) {
        let from = self.state_name();
        let ground = self.cfg.ground_speed;
        let mut a = Active {
            kind,
            started: self.time,
            elapsed: 0.0,
            duration: 0.0,
            start: self.pos,
            target: self.pos,
            dir: Vec3::from_yaw(self.yaw),
            chain_checked: false,
            hold_back: 0.0,
            hold_left: 0.0,
            hold_right: 0.0,
            monitoring: kind == MoveKind::Run2Cover,
            split: 0.0,
            mid: self.pos,
            up: 0.0,
            over: 0.0,
            land: 0.0,
        };
        match kind {
            MoveKind::Run2Cover => {
                let spot = self.pending_cover.take().expect("Run2Cover needs a cover");
                // GSM_Run2Cover.PlayTransitionAnimation:
                // duration = dist / (GroundSpeed * rate), at least MinTravelAnimDuration
                let dist = (spot.location - self.pos).flat().length();
                let rate = self.cfg.run2cover_speed_mod;
                a.duration = (dist / (ground * rate)).max(self.cfg.run2cover_min_duration);
                a.target = spot.location;
                a.dir = (spot.location - self.pos).flat().normalized();
                // GearPC.AcquireCover happens at the start of the slide.
                self.cover = Some(InCover {
                    spot,
                    since: self.time,
                });
            }
            MoveKind::CoverSlip => {
                let spot = self.cover.expect("slip needs cover").spot;
                let side = if self.mirrored {
                    -spot.right()
                } else {
                    spot.right()
                };
                // AR_Cov_Mid_Slip root motion: sideways round the edge + forward past it.
                let travel =
                    side * self.cfg.slip_travel_side + (-spot.normal) * self.cfg.slip_travel_fwd;
                a.dir = travel.normalized();
                a.duration = self.cfg.slip_anim_length / self.cfg.slip_speed_mod;
                a.target = self.pos + travel;
                self.cover = None; // bBreakFromCover
            }
            MoveKind::RoadieRun => {
                self.cover = None;
            }
            MoveKind::SwatTurn => {
                // GSM_StdLvlSwatTurn: dash (precise destination) at GroundSpeed × 1.7
                // until EndTurnAnimDist (256 uu) out, then PlayEndAnimation: the end
                // anim at rate 1.3, started at (256 - dist) / 256 of the way in, carries
                // the pawn the rest (its root motion is 254 uu, so ≈ linear here).
                let target = self.pending_swat.take().expect("swat needs a target");
                let dist = (target.location - self.pos).flat().length();
                a.target = target.location;
                a.dir = (target.location - self.pos).flat().normalized();
                let end_dist = self.cfg.swat_end_dist;
                let end_time = self.cfg.swat_end_anim_length / self.cfg.swat_anim_rate;
                let dash = (dist - end_dist).max(0.0);
                a.split = dash / (ground * self.cfg.swat_speed_mod);
                a.mid = self.pos + a.dir * dash;
                let left = dist.min(end_dist) / end_dist;
                a.duration = (a.split + end_time * left).max(1.0 / 60.0);
                self.cover = Some(InCover {
                    spot: target,
                    since: self.time,
                });
                self.swat_roadie_timer = None;
            }
            MoveKind::PushOut => {
                // GSM_PushOutOfCover.DoPushOutSpecifics: Velocity = -GroundSpeed *
                // facing * SpeedModifier; the move ends with the cover-break anim.
                let rate = self.cfg.push_out_speed_mod;
                a.dir = -Vec3::from_yaw(self.yaw);
                a.duration = self.cfg.push_out_anim_length / rate;
                self.vel = a.dir * (self.cfg.ground_speed * rate);
                self.cover = None; // PCOwner.LeaveCover() as the move starts
                self.break_hold = 0.0;
            }
            MoveKind::Vault => {
                let dist = self.pending_vault.take().expect("vault needs a distance");
                let c = self.cover.expect("vault needs cover").spot;
                let cfg = &self.cfg;
                let rate = cfg.vault_speed_mod;
                // SpecialMoveStarted / PlayJump: face straight over the cover.
                a.dir = -c.normal;
                self.yaw = a.dir.yaw();
                // AdaptJumpToPathing: the jump's forward root motion is scaled to fit.
                let scale = (dist / cfg.vault_default_dist).max(cfg.vault_min_root_scale);
                let frame = 1.0 / (30.0 * rate);
                a.up = cfg.vault_up_frames * frame;
                a.over = cfg.vault_over_frames * frame;
                a.split = cfg.vault_jump_frames * frame;
                let travel = cfg.vault_jump_travel * scale;
                a.mid = self.pos + a.dir * travel;
                // PlayFall: Velocity = facing × (2D distance to EstimatedLandingLoc) /
                // sqrt(drop / |g|). VerifyLandingClear puts that spot GetEstimatedHorizFallDist
                // = FallForwardVelocity × sqrt(VerticalFallStartDist / |g|) past the fall
                // start; on flat ground its height is the standing centre + 2 × Extent.Z
                // (Extent.Z = CollisionHeight / 4), so the drop is rise − height / 2.
                let g = cfg.gravity;
                let est_horiz = cfg.vault_fall_fwd_vel * (cfg.vault_fall_start_dist / g).sqrt();
                let ahead = (est_horiz - (travel - dist)).max(0.0);
                let drop = cfg.vault_rise - 0.5 * cfg.collision_height;
                let fwd_vel = if drop > 1.0 {
                    ahead / (drop / g).sqrt()
                } else {
                    cfg.vault_fall_fwd_vel
                };
                // Falling physics from the top of the jump (no vertical speed) to the floor.
                let fall = (2.0 * cfg.vault_rise / g).sqrt();
                a.land = a.split + fall;
                a.target = a.mid + a.dir * (fwd_vel * fall);
                // PlayLand: AR_Mantle_End at SpeedModifier, no root motion.
                a.duration = a.land + cfg.vault_land_frames * frame;
                self.vel = Vec3::ZERO;
            }
            k if k.is_evade() => {
                // GSM_Evade: start anim then end anim, both at SpeedModifier, root motion
                // (RootMotionMode 3, all axes) carries the pawn.
                let e = self.evade_anim(k);
                let rate = self.cfg.evade_speed_mod;
                a.dir = self.evade_move_dir(k);
                a.split = e.start_length / rate;
                a.duration = (e.start_length + e.end_length) / rate;
                a.mid = self.pos + a.dir * e.start_dist;
                a.target = a.mid + a.dir * e.end_dist;
                self.vel = Vec3::ZERO; // root motion drives the evade
                self.cover = None; // bBreakFromCover
            }
            _ => {}
        }
        self.active = Some(a);
        self.log(from, reason);
    }

    /// `DoSpecialMove(SM_None)`: end the active move; start the queued one if allowed.
    fn end_move(&mut self, reason: &'static str) {
        let Some(a) = self.active.take() else { return };
        let from = format!("{:?}", a.kind);
        self.last_move_end = self.time;
        match a.kind {
            MoveKind::CoverSlip => {
                // GSM_CoverSlip.SpecialMoveEnded: keep moving at GroundSpeed.
                self.vel = a.dir * self.cfg.ground_speed;
                if self.prev_a && self.pending.is_none() {
                    self.pending = Some(MoveKind::RoadieRun); // QueueRoadieRunMove
                }
            }
            k if k.is_evade() => self.last_evade_time = self.time,
            MoveKind::Vault => {
                // bBreakFromCoverOnEnd; SpecialMoveEnded: A held → QueueRoadieRunMove.
                self.cover = None;
                self.vel = Vec3::ZERO;
                if self.prev_a && self.pending.is_none() {
                    self.pending = Some(MoveKind::RoadieRun);
                }
            }
            MoveKind::SwatTurn => {
                self.swat_roadie_timer = None;
                self.vel = Vec3::ZERO; // ZeroMovementVariables
                if self.cover.is_some() {
                    // INFERRED: the end anim plays mirrored (`!bIsMirrored`), so you land
                    // facing back toward the gap you crossed.
                    self.mirrored = !self.mirrored;
                }
            }
            _ => {}
        }
        self.log(from, reason);
        if let Some(next) = self.pending.take() {
            if self.can_do(next, None) {
                self.start_move(next, "queued move");
            }
        }
    }

    // --------------------------------------------------------- run to cover

    /// `GearPC.CanTryToRunToCover`: no special move, except during a push out
    /// (`IsDoingSpecialMove(SM_PushOutOfCover)`), which the slide cuts short.
    fn can_try_to_run_to_cover(&self) -> bool {
        self.active
            .map(|a| a.kind == MoveKind::PushOut)
            .unwrap_or(true)
    }

    /// `GearPC.CanRunToCover` + `AcquireCover`.
    fn try_to_run_to_cover(
        &mut self,
        input: &Input,
        world: &dyn World,
        check_scale: f32,
        skip_move_check: bool,
    ) -> bool {
        if self.cover.is_some() || (!skip_move_check && !self.can_try_to_run_to_cover()) {
            return false;
        }
        let dz = self.cfg.dead_zone;
        let mut sideways = false;
        let dir = if Self::stick_len(input) > dz {
            if input.stick_x.abs() > input.stick_y.abs() {
                sideways = true;
            } else if input.stick_y < -dz {
                return false; // pulling back never runs to cover
            }
            self.stick_world(input).normalized()
        } else {
            Vec3::from_yaw(self.yaw)
        };
        let max = if sideways {
            self.cfg.run2cover_side_max_dist
        } else {
            self.cfg.run2cover_max_dist * check_scale
        };
        match world.find_cover(self.pos, dir, max) {
            Some(spot) => {
                self.pending_cover = Some(spot);
                if self.request(MoveKind::Run2Cover, "run to cover") {
                    true
                } else {
                    self.pending_cover = None;
                    false
                }
            }
            None => false,
        }
    }

    /// `GearPC.PlayerTick` roadie → cover check + `TransitionFromRoadieRunToCover`.
    fn roadie_auto_cover(&mut self, input: &Input, world: &dyn World) {
        let Some(a) = self.active else { return };
        if a.kind != MoveKind::RoadieRun
            || self.time - self.last_move_end <= self.cfg.roadie_cover_after_move
        {
            return;
        }
        let facing = Vec3::from_yaw(self.yaw);
        let dz = self.cfg.dead_zone;
        let check = if input.stick_y > dz || input.stick_x.abs() > dz {
            self.stick_world(input).normalized()
        } else {
            facing
        };
        if check.dot(facing) <= self.cfg.roadie_cover_dir_dot {
            return;
        }
        let reach = self.cfg.roadie_to_cover_dist + self.cfg.collision_radius;
        if let Some(spot) = world.find_cover(self.pos, check, reach) {
            let seamless = spot.normal.dot(facing) > self.cfg.seamless_cover_dot;
            let from = self.state_name();
            self.active = None;
            self.last_move_end = self.time;
            self.pos = spot.location;
            self.vel = Vec3::ZERO;
            self.yaw = (-spot.normal).yaw();
            self.cover = Some(InCover {
                spot,
                since: self.time,
            });
            self.log(from, "roadie run into cover");
            if seamless && spot.dist_to_left_edge > 1.0 && spot.dist_to_right_edge > 1.0 && input.a
            {
                self.request(MoveKind::CoverRun, "seamless roadie → cover run");
            }
        }
    }

    // ------------------------------------------------------------ cover slip

    fn edge_dist(&self) -> Option<f32> {
        let c = self.cover?;
        Some(if self.mirrored {
            c.spot.dist_to_left_edge
        } else {
            c.spot.dist_to_right_edge
        })
    }

    fn at_cover_edge(&self, tolerance: f32) -> bool {
        self.edge_dist()
            .map(|d| d <= tolerance + 1.0)
            .unwrap_or(false)
    }

    /// `GSM_CoverSlip.InternalCanDoSpecialMove`.
    fn can_cover_slip(&self, input: &Input) -> bool {
        let Some(c) = self.cover else { return false };
        let allowed = if self.mirrored {
            c.spot.can_slip_left
        } else {
            c.spot.can_slip_right
        };
        let Some(edge) = self.edge_dist() else {
            return false;
        };
        if !allowed || edge > self.cfg.slip_max_edge_dist {
            return false;
        }
        // Stick "up": raw if cover was just taken, else cover-remapped (toward the wall).
        let up = if self.time - c.since < self.cfg.slip_raw_stick_window {
            input.stick_y
        } else {
            self.stick_world(input).dot(-c.spot.normal)
        };
        up >= self.cfg.slip_stick_up
    }

    // ------------------------------------------------------------- swat turn

    /// `GSM_StdLvlSwatTurn.InternalCanDoSpecialMove`: at the edge you're facing, stick
    /// pushed that way (sideways-dominant), camera looking over the cover, and a swat
    /// target across the gap.
    fn can_swat_turn(&self, input: &Input, world: &dyn World) -> Option<CoverSpot> {
        let c = self.cover?;
        let dz = self.cfg.dead_zone;
        let (right, up) = self.remapped_stick(input);
        if right.abs() < dz
            || (self.mirrored && right > dz)
            || (!self.mirrored && right < -dz)
            || up.abs() > right.abs()
        {
            return None;
        }
        let facing = -c.spot.normal;
        if Vec3::from_yaw(input.cam_yaw).dot(facing) < self.cfg.swat_camera_dot {
            return None;
        }
        let edge = self.edge_dist()?;
        if edge > self.cfg.swat_edge_dist {
            return None;
        }
        let side = if self.mirrored {
            -c.spot.right()
        } else {
            c.spot.right()
        };
        let edge_point = c.spot.location + side * edge;
        world.find_swat_target(&c.spot, edge_point, side, self.cfg.swat_max_dist)
    }

    // ----------------------------------------------------------------- vault

    /// `GSM_MidLvlJumpOver` / `GSM_MantleOverCoverBase.InternalCanDoSpecialMove`:
    /// waist-high cover, stick pushed toward the wall (up-dominant, past the dead zone),
    /// close to the cover slot (96 uu during a slide, else 3 × radius), a mantle
    /// distance the world can find, and a clear landing. Returns `MantleDistance`.
    fn can_vault(&self, input: &Input, world: &dyn World) -> Option<f32> {
        let c = self.cover?;
        if c.spot.height != crate::world::CoverHeight::Mid {
            return None;
        }
        let (right, up) = self.remapped_stick(input);
        if up.abs() < right.abs() || up <= self.cfg.dead_zone {
            return None;
        }
        let r = self.cfg.collision_radius;
        let sliding = self.active.map(|a| a.kind == MoveKind::Run2Cover);
        let to_slot = (c.spot.location - self.pos).flat().length();
        if (sliding == Some(true) && to_slot > self.cfg.vault_slide_slot_dist) || to_slot > 3.0 * r
        {
            return None;
        }
        // Measured from the slot, where a slide is about to put the pawn.
        let from = c.spot.location;
        let dir = -c.spot.normal;
        let dist = world.mantle_distance(from, dir, self.cfg.vault_max_dist, r)?;
        // VerifyLandingClear (flat ground): nothing in the way from the fall start to
        // the estimated landing spot, one radius beyond.
        let g = self.cfg.gravity;
        let est_horiz = self.cfg.vault_fall_fwd_vel * (self.cfg.vault_fall_start_dist / g).sqrt();
        let fall_start = from + dir * dist;
        if !world.is_clear(fall_start, fall_start + dir * (est_horiz + r), 0.0) {
            return None;
        }
        Some(dist)
    }

    /// `GearPC.TransitionFromSwatToRoadieRun`: A still held, still swatting → far from
    /// the target (> 2 × collision radius): leave cover and force a roadie run; near
    /// it: try a cover slip.
    fn transition_from_swat_to_roadie(&mut self, input: &Input) {
        let Some(a) = self.active else { return };
        if !input.a || a.kind != MoveKind::SwatTurn {
            return;
        }
        if (a.target - self.pos).flat().length() > 2.0 * self.cfg.collision_radius {
            self.cover = None;
            self.pending = None;
            self.active = None;
            self.last_move_end = self.time;
            let from = MoveKind::SwatTurn;
            self.events.push(Event {
                time: self.time,
                from: format!("{from:?}"),
                to: "Walk".into(),
                reason: "swat cut short by roadie run",
            });
            self.start_move(MoveKind::RoadieRun, "swat → roadie (A held)");
        } else if self.can_cover_slip(input) {
            self.request(MoveKind::CoverSlip, "swat → cover slip (A held)");
        }
    }

    // ----------------------------------------------------------------- evade

    /// `GearPC.ChooseEvadeMoveBasedOnStickDir` (cover-remapped stick ≈ camera stick here).
    fn choose_evade(&self, input: &Input) -> MoveKind {
        let dz = self.cfg.dead_zone;
        let (x, y) = self.remapped_stick(input);
        if y.abs() > x.abs() {
            if y < -dz {
                return MoveKind::EvadeBwd;
            }
            MoveKind::EvadeFwd
        } else if x > dz {
            MoveKind::EvadeRt
        } else if x < -dz {
            MoveKind::EvadeLt
        } else {
            MoveKind::EvadeFwd
        }
    }

    fn evade_anim(&self, kind: MoveKind) -> crate::config::EvadeAnim {
        match kind {
            MoveKind::EvadeBwd => self.cfg.evade_bwd,
            MoveKind::EvadeLt => self.cfg.evade_lt,
            MoveKind::EvadeRt => self.cfg.evade_rt,
            _ => self.cfg.evade_fwd,
        }
    }

    /// Where the evade goes: its anim's root motion is straight along one axis, so
    /// exactly 90° / 180° from the facing.
    fn evade_move_dir(&self, kind: MoveKind) -> Vec3 {
        let off = match kind {
            MoveKind::EvadeRt => std::f32::consts::FRAC_PI_2,
            MoveKind::EvadeLt => -std::f32::consts::FRAC_PI_2,
            MoveKind::EvadeBwd => std::f32::consts::PI,
            _ => 0.0,
        };
        Vec3::from_yaw(self.yaw + off)
    }

    /// The direction `GSM_Evade.InternalCanDoSpecialMoveEvade_Worker` traces before
    /// an evade from cover, with its shipped 16834 for left (only the check uses it).
    fn evade_dir(&self, kind: MoveKind) -> Vec3 {
        // GSM_Evade direction offsets in Unreal rotation units, including the shipped
        // evade-left value 16834 (not 16384).
        let off = match kind {
            MoveKind::EvadeRt => urot_to_rad(16384),
            MoveKind::EvadeLt => -urot_to_rad(16834),
            MoveKind::EvadeBwd => -urot_to_rad(32768),
            _ => 0.0,
        };
        Vec3::from_yaw(self.yaw + off)
    }

    /// `GearPC.TryToEvade` + `GSM_Evade.InternalCanDoSpecialMoveEvade_Worker`.
    /// `CanEvade` allows it during a push out (which it then cuts short).
    fn try_to_evade(&mut self, input: &Input, world: &dyn World) -> bool {
        let pushing_out = self.active.map(|a| a.kind == MoveKind::PushOut);
        if pushing_out == Some(false)
            || self.time - self.last_evade_time < self.cfg.evade_retrigger_delay
        {
            return false;
        }
        // PlayerTakingCover.TryToEvade: from cover only with the stick along the wall.
        if self.cover.is_some() && self.remapped_stick(input).0.abs() <= self.cfg.dead_zone {
            return false;
        }
        let kind = self.choose_evade(input);
        // Gears turns the pawn to the controller yaw before evading.
        if self.cover.is_none() && pushing_out != Some(false) {
            self.yaw = input.cam_yaw;
        }
        let dir = self.evade_dir(kind);
        if self.cover.is_some() && !world.is_clear(self.pos, self.pos + dir * 128.0, 12.0) {
            return false;
        }
        self.request(kind, "evade")
    }

    // ------------------------------------------------------------- per tick

    fn tick_active(&mut self, input: &Input, dt: f32, world: &dyn World) {
        let Some(mut a) = self.active else { return };
        let first_frame = a.started == self.time;
        if first_frame {
            // The move began this frame (Gears starts the anim; motion follows).
            if a.kind == MoveKind::RoadieRun && Self::stick_len(input) > self.cfg.dead_zone {
                // GSM_RoadieRun.SpecialMoveStarted: face the stick direction.
                self.yaw = self.stick_world(input).yaw();
            }
            return;
        }
        a.elapsed += dt;
        match a.kind {
            MoveKind::RoadieRun => {
                // GSM_RoadieRun.PreProcessInput: always forward; stick left/right
                // steers at SteeringPct (no strafing).
                let turn = input.stick_x * self.cfg.roadie_steering_pct * self.cfg.turn_rate * dt;
                self.yaw += turn;
                let speed = self.cfg.ground_speed * self.cfg.roadie_speed_mod;
                self.vel = Vec3::from_yaw(self.yaw) * speed;
                self.pos += self.vel * dt;
                self.active = Some(a);
            }
            MoveKind::CoverRun => {
                let c = self.cover.expect("cover run needs cover").spot;
                let side = if input.stick_x < -self.cfg.dead_zone {
                    -c.right()
                } else {
                    c.right()
                };
                self.mirrored = input.stick_x < -self.cfg.dead_zone;
                let speed = self.cfg.ground_speed * self.cfg.cover_run_speed_mod;
                let limit = if self.mirrored {
                    c.dist_to_left_edge
                } else {
                    c.dist_to_right_edge
                };
                let step = (speed * dt).min(limit.max(0.0));
                self.pos += side * step;
                if let Some(cv) = self.cover.as_mut() {
                    if self.mirrored {
                        cv.spot.dist_to_left_edge -= step;
                        cv.spot.dist_to_right_edge += step;
                    } else {
                        cv.spot.dist_to_right_edge -= step;
                        cv.spot.dist_to_left_edge += step;
                    }
                    cv.spot.location = self.pos;
                }
                self.active = Some(a);
                // reached the end (with a hair of slack so f32 and f64 agree)
                if limit - step <= 0.01 {
                    self.end_move("reached cover edge");
                }
            }
            MoveKind::Run2Cover => {
                // GSM_Run2Cover joystick monitoring, on the cover-remapped stick (the PC
                // is already in its cover state during the slide). JoystickHeldLongEnough
                // checks `HoldingBack > 0` but `HoldingLeft >= MinHoldTimeToSwitchMirroring`:
                // pulling away from the wall aborts at once (GearPC.AbortRun2Cover); left /
                // right only picks the side after 0.25 s. One decision per slide.
                let dz = self.cfg.dead_zone;
                let hold = self.cfg.run2cover_mirror_hold;
                let (right, up) = self.remapped_stick(input);
                let mut abort = false;
                if a.monitoring {
                    let held = |on: bool, t: f32| if on { t + dt } else { 0.0 };
                    a.hold_back = held(up < -dz && up.abs() >= right.abs(), a.hold_back);
                    a.hold_left = held(right < -dz, a.hold_left);
                    a.hold_right = held(right > dz, a.hold_right);
                    if a.hold_back > 0.0 {
                        a.monitoring = false;
                        abort = true;
                    } else if a.hold_left >= hold || a.hold_right >= hold {
                        a.monitoring = false;
                        self.mirrored = a.hold_left >= hold;
                    }
                }
                let t = (a.elapsed / a.duration).min(1.0);
                self.pos = a.start + (a.target - a.start) * t;
                if let Some(c) = self.cover {
                    let goal = (-c.spot.normal).yaw();
                    self.yaw += angle_diff(self.yaw, goal) * t.min(1.0);
                }
                self.active = Some(a);
                if abort {
                    // GearPC.AbortRun2Cover: leave cover, drop anything queued. The slide
                    // moves the pawn by velocity (RootMotionMode 2 = ignore root motion), so
                    // it keeps moving; ASSUMPTION: capped to GroundSpeed once the 2.3× is
                    // gone (UE3 walking physics is native).
                    self.cover = None;
                    self.pending = None;
                    self.pending_swat = None;
                    self.vel = a.dir * self.cfg.ground_speed;
                    self.end_move("slide aborted (stick back)");
                } else if a.elapsed >= a.duration {
                    self.vel = Vec3::ZERO;
                    if let Some(c) = self.cover {
                        self.yaw = (-c.spot.normal).yaw();
                    }
                    // GSM_Run2Cover.SpecialMoveEnded: still monitoring → any hold sets
                    // the side.
                    if a.monitoring {
                        if a.hold_left > 0.0 {
                            self.mirrored = true;
                        } else if a.hold_right > 0.0 {
                            self.mirrored = false;
                        }
                    }
                    self.end_move("reached cover");
                }
            }
            MoveKind::CoverSlip => {
                let t = (a.elapsed / a.duration).min(1.0);
                self.pos = a.start + (a.target - a.start) * t;
                self.yaw = a.dir.yaw();
                self.active = Some(a);
                if a.elapsed >= a.duration {
                    self.end_move("slip finished");
                }
            }
            MoveKind::SwatTurn => {
                self.pos = Self::two_part(&a);
                if let Some(c) = self.cover {
                    self.yaw = (-c.spot.normal).yaw(); // bLockPawnRotation
                }
                self.active = Some(a);
                if a.elapsed >= a.duration {
                    self.end_move("swat turn finished");
                }
            }
            MoveKind::Vault => {
                self.pos = if a.elapsed < a.up {
                    a.start
                } else if a.elapsed < a.over {
                    a.start + (a.mid - a.start) * ((a.elapsed - a.up) / (a.over - a.up))
                } else if a.elapsed < a.split {
                    a.mid
                } else {
                    let t = ((a.elapsed - a.split) / (a.land - a.split)).min(1.0);
                    a.mid + (a.target - a.mid) * t
                };
                self.yaw = a.dir.yaw(); // bLockPawnRotation
                self.active = Some(a);
                // Landed → AllowTransitionToAMove after 0.1 / SpeedModifier, then every
                // tick CheckTransitionToAMove: A held → roadie run cuts the landing short.
                let allow = a.land + self.cfg.vault_roadie_after_land / self.cfg.vault_speed_mod;
                if a.elapsed >= allow && input.a && self.pending.is_none() {
                    self.cover = None;
                    self.vel = Vec3::ZERO;
                    self.active = None;
                    self.last_move_end = self.time;
                    self.events.push(Event {
                        time: self.time,
                        from: "Vault".into(),
                        to: "Walk".into(),
                        reason: "landing cut short by roadie run",
                    });
                    self.start_move(MoveKind::RoadieRun, "vault → roadie (A held)");
                } else if a.elapsed >= a.duration {
                    self.end_move("vault finished");
                }
            }
            MoveKind::PushOut => {
                // Only a shove off the wall: walking runs underneath (see tick), so the
                // stick steers from the 225 uu/s start velocity; any move cuts it short.
                self.active = Some(a);
                if a.elapsed >= a.duration {
                    self.end_move("push out finished");
                }
            }
            k if k.is_evade() => {
                self.pos = Self::two_part(&a);
                // GSM_Evade.CheckTransitionToAnotherMove, 0.2 s before the end.
                if !a.chain_checked && a.duration - a.elapsed <= self.cfg.evade_chain_lead {
                    a.chain_checked = true;
                    self.active = Some(a);
                    if input.a {
                        let scale = self.cfg.evade_cover_check_scale;
                        if !self.try_to_run_to_cover(input, world, scale, true) {
                            self.request(MoveKind::RoadieRun, "evade → roadie (A held)");
                        }
                        return;
                    }
                }
                self.active = Some(a);
                if a.elapsed >= a.duration {
                    self.end_move("evade finished");
                }
            }
            _ => self.active = Some(a),
        }
    }

    /// `GearPC.PlayerTakingCover.UpdatePlayerPosture`: hold the (cover-remapped) stick
    /// away from the wall for BreakFromCoverHoldTime → `DoSpecialMove(SM_PushOutOfCover)`.
    /// Only counts while no special move is running.
    fn check_break_from_cover(&mut self, input: &Input, dt: f32) {
        if self.cover.is_none() || self.active.is_some() {
            self.break_hold = 0.0;
            return;
        }
        let dz = self.cfg.dead_zone;
        let (right, up) = self.remapped_stick(input);
        let up_dominant = up.abs() > right.abs();
        if !up_dominant || up.abs() < dz {
            self.break_hold = 0.0;
        }
        if up_dominant && up < -dz {
            self.break_hold += dt;
            if self.break_hold >= self.cfg.break_from_cover_hold {
                self.request(MoveKind::PushOut, "stick away from cover: push out");
            }
        }
    }

    /// Position along a two-part move: start → mid by `split`, mid → target by the end.
    fn two_part(a: &Active) -> Vec3 {
        if a.elapsed < a.split {
            a.start + (a.mid - a.start) * (a.elapsed / a.split)
        } else {
            let rest = a.duration - a.split;
            let t = if rest > 0.0 {
                ((a.elapsed - a.split) / rest).min(1.0)
            } else {
                1.0
            };
            a.mid + (a.target - a.mid) * t
        }
    }

    /// Moving along cover (`PlayerTakingCover.UpdatePlayerPosture` + native cover
    /// physics): stick sideways-dominant → face that way (`SetMirroredSide`) and move
    /// along the wall at GroundSpeed × `CoverMovementPct[type]`, stopping at the ends.
    fn tick_cover_move(&mut self, input: &Input, dt: f32) {
        let Some(c) = self.cover else { return };
        let dz = self.cfg.dead_zone;
        let (right, up) = self.remapped_stick(input);
        let along = if right.abs() > dz && right.abs() > up.abs() {
            self.mirrored = right < 0.0;
            right.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        let pct = match c.spot.height {
            crate::world::CoverHeight::Mid => self.cfg.cover_move_pct_mid,
            crate::world::CoverHeight::Standing => self.cfg.cover_move_pct_std,
        };
        let side = c.spot.right();
        let cur = self.vel.dot(side);
        let goal = along * self.cfg.ground_speed * pct;
        let max_dv = self.cfg.accel_rate * dt;
        let mut v = cur + (goal - cur).clamp(-max_dv, max_dv);
        // the ends of the cover stop you
        let room = if v >= 0.0 {
            c.spot.dist_to_right_edge
        } else {
            c.spot.dist_to_left_edge
        };
        let mut step = v * dt;
        if step.abs() > room {
            step = room.max(0.0) * step.signum();
            v = 0.0;
        }
        self.vel = side * v;
        if step != 0.0 {
            self.pos += side * step;
            if let Some(cv) = self.cover.as_mut() {
                cv.spot.dist_to_right_edge -= step;
                cv.spot.dist_to_left_edge += step;
                cv.spot.location = self.pos;
            }
        }
    }

    /// Plain walking: accelerate toward the stick at AccelRate, cap at GroundSpeed.
    fn tick_locomotion(&mut self, input: &Input, dt: f32) {
        if self.cover.is_some() {
            self.tick_cover_move(input, dt);
            return;
        }
        let mut wish = self.stick_world(input);
        if wish.length() > 1.0 {
            wish = wish.normalized();
        }
        if Self::stick_len(input) <= self.cfg.dead_zone {
            wish = Vec3::ZERO;
        }
        // Aiming: GearWeapon.bForceWalkWhenTargeting → GearPC.HandleWalking → walking at
        // GroundSpeed × WalkingPct, facing where the camera aims (strafing).
        let speed = if input.aim {
            self.cfg.ground_speed * self.cfg.walking_pct
        } else {
            self.cfg.ground_speed
        };
        let target = wish * speed;
        self.vel = self.vel.move_toward(target, self.cfg.accel_rate * dt);
        self.pos += self.vel * dt;
        if input.aim {
            self.yaw = input.cam_yaw;
        } else if self.vel.length_2d() > 1.0 {
            self.yaw = self.vel.yaw();
        }
    }
}
