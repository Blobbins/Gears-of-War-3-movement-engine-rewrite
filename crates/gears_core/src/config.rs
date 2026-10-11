//! Tunables. Every value carries its source in Gears of War 3 (see docs/GEARS_VALUES.md).
//! `PLACEHOLDER` values are not from Gears yet and must be replaced (mostly root-motion
//! distances that come from animations, milestone L4).

#[derive(Clone, Debug)]
pub struct Config {
    // --- walking (Engine.Pawn / GearPawn defaults) ---
    /// `Default__GearPawn.GroundSpeed`
    pub ground_speed: f32,
    /// `Default__Pawn.AccelRate` (Engine; not overridden by GearPawn)
    pub accel_rate: f32,
    /// `GearPC.DeadZoneThreshold`
    pub dead_zone: f32,
    /// `Engine Default__Pawn.CollisionCylinder.CollisionRadius` (GearPawn inherits it)
    pub collision_radius: f32,
    /// `Engine Default__Pawn.WalkingPct`: walking speed while aiming
    /// (`Default__GearWeapon.bForceWalkWhenTargeting`)
    pub walking_pct: f32,

    // --- roadie run ---
    /// `Default__GSM_RoadieRun.SpeedModifier`
    pub roadie_speed_mod: f32,
    /// `Default__GSM_RoadieRun.SteeringPct`
    pub roadie_steering_pct: f32,
    /// `GearPC.RoadieRunTimer` — A held this long before roadie starts
    pub roadie_hold_time: f32,
    /// PLACEHOLDER: full-steering turn rate (rad/s) that SteeringPct scales
    pub turn_rate: f32,

    // --- run to cover (slide) ---
    /// `Default__GSM_Run2Cover.SpeedModifier`
    pub run2cover_speed_mod: f32,
    /// `GSM_Run2Cover.MinTravelAnimDuration`
    pub run2cover_min_duration: f32,
    /// `GSM_Run2Cover.MinHoldTimeToSwitchMirroring`
    pub run2cover_mirror_hold: f32,
    /// `Default__GearPawn.Run2CoverMaxDist`
    pub run2cover_max_dist: f32,
    /// `Default__GearPawn.Run2CoverPerpendicularMaxDist`
    pub run2cover_side_max_dist: f32,
    /// `Default__GearPawn.RoadieRun2CoverSlideDist`
    pub roadie_to_cover_dist: f32,
    /// `GearPC.TransitionFromRoadieRunToCover`: seamless if dot(normal, facing) > this
    pub seamless_cover_dot: f32,

    /// `GearPC.PlayerTick`: roadie auto-cover needs dot(check dir, facing) > this
    pub roadie_cover_dir_dot: f32,
    /// `GearPC.PlayerTick`: ...and the previous special move ended this long ago
    pub roadie_cover_after_move: f32,

    // --- cover ---
    /// `GearPC.TimeBeforeAButtonPressLeavesCover`
    pub a_press_keeps_cover_time: f32,
    /// `Default__GSM_CoverRun.SpeedModifier` (GearPawn.ini agrees)
    pub cover_run_speed_mod: f32,
    /// `Default__GearPawn.CoverMovementPct[CT_MidLevel]` / `[CT_Standing]`
    pub cover_move_pct_mid: f32,
    pub cover_move_pct_std: f32,
    /// `GearPC.BreakFromCoverHoldTime`: stick held away from cover this long → push out
    pub break_from_cover_hold: f32,
    /// `Default__GSM_PushOutOfCover.SpeedModifier` (anim rate and speed × GroundSpeed)
    pub push_out_speed_mod: f32,
    /// `AR_Cov_Mid_Idle2Idle_Ready_Aim` length at rate 1.0 (14 frames, 0.4667 s), the
    /// cover-break anim GSM_PushOutOfCover plays and ends on (AnimSetMarcus_CamSkel)
    pub push_out_anim_length: f32,

    // --- cover slip ---
    /// `Default__GSM_CoverSlip.SpeedModifier` (anim play rate)
    pub slip_speed_mod: f32,
    /// `GSM_CoverSlip.InternalCanDoSpecialMove`: max distance to the edge slot
    pub slip_max_edge_dist: f32,
    /// same function: stick "up" needed
    pub slip_stick_up: f32,
    /// same function: raw stick used if cover was taken less than this ago
    pub slip_raw_stick_window: f32,
    /// `AR_Cov_Mid_Slip` root motion over the anim: along the slip side / past the
    /// wall (uu). Read from the owner's export, see GEARS_VALUES.md → Animations.
    pub slip_travel_side: f32,
    pub slip_travel_fwd: f32,
    /// `AR_Cov_Mid_Slip` length at rate 1.0 (21 frames, 0.7 s; AnimSetMarcus_CamSkel)
    pub slip_anim_length: f32,

    // --- swat turn ---
    /// `Default__GSM_StdLvlSwatTurn.SpeedModifier` (× GroundSpeed while dashing)
    pub swat_speed_mod: f32,
    /// `Engine Default__CoverLink.TurnDist`: farthest swat turn target
    pub swat_max_dist: f32,
    /// `GSM_StdLvlSwatTurn.InternalCanDoSpecialMove`: camera · pawn facing ≥ this
    pub swat_camera_dot: f32,
    /// `GearPC.PlayerTakingCover.TryASpecialMove`: A during a swat → roadie check after
    pub swat_to_roadie_delay: f32,
    /// `GSM_StdLvlSwatTurn.EndTurnAnimDist`: the end anim takes over this far out
    pub swat_end_dist: f32,
    /// `GSM_StdLvlSwatTurn.AnimRateScale`
    pub swat_anim_rate: f32,
    /// `AR_Cov_Mid_SwatTurn_End` length at rate 1.0 (25 frames)
    pub swat_end_anim_length: f32,
    /// ASSUMPTION: "at the edge slot" tolerance (Gears uses hand-placed slots); we
    /// reuse GSM_CoverSlip's 96 uu edge distance
    pub swat_edge_dist: f32,

    // --- evade ---
    /// `Default__GSM_Evade.SpeedModifier` (anim play rate)
    pub evade_speed_mod: f32,
    /// `GearPC.EvadeRetriggerDelay`: `NotifyEvadeFinished` blocks the next evade this long.
    /// (`GearPawn.MinTimeBetweenEvades` = 2.0 is not used by the player's script.)
    pub evade_retrigger_delay: f32,
    /// `GSM_Evade.PlayEndAnim`: check for next move this long before the end
    pub evade_chain_lead: f32,
    /// `GSM_Evade.CheckTransitionToAnotherMove`: run-to-cover CheckScale
    pub evade_cover_check_scale: f32,
    /// Evade anims (`AR_Evade_<Dir>_Start` / `_End`), each played at SpeedModifier:
    /// lengths at rate 1.0 and root-motion travel, from the owner's export.
    pub evade_fwd: EvadeAnim,
    pub evade_bwd: EvadeAnim,
    pub evade_lt: EvadeAnim,
    pub evade_rt: EvadeAnim,

    // --- vault over mid cover (SM_MidLvlJumpOver, GSM_MantleOverCoverBase) ---
    /// `Default__GSM_MidLvlJumpOver.SpeedModifier` (anim play rate)
    pub vault_speed_mod: f32,
    /// `Default__GSM_MantleOverCoverBase.DefaultMantleDistance`: the jump anim's own
    /// travel; `RootMotionScaleFactor = max(MantleDistance / this, 0.5)`
    pub vault_default_dist: f32,
    pub vault_min_root_scale: f32,
    /// `Default__GearScout.MaxMantleLateralDist`: farthest the far side can be
    pub vault_max_dist: f32,
    /// `GSM_MantleOverCoverBase.InternalCanDoSpecialMove`: during a slide the vault is
    /// only allowed this close to the cover slot (`IsDoingSpecialMove(SM_Run2MidCov)`)
    pub vault_slide_slot_dist: f32,
    /// `AR_Mantle_Start` root track (AnimSetMarcus_CamSkel, 21 frames): straight up to
    /// `vault_rise` by frame 15, then forward `vault_jump_travel` by frame 20
    pub vault_jump_frames: f32,
    pub vault_up_frames: f32,
    pub vault_over_frames: f32,
    pub vault_rise: f32,
    pub vault_jump_travel: f32,
    /// `Default__GSM_MantleOverCoverBase.FallForwardVelocity`
    pub vault_fall_fwd_vel: f32,
    /// `Default__GSM_MantleOverCoverBase.VerticalFallStartDist`
    pub vault_fall_start_dist: f32,
    /// `Engine Default__WorldInfo.DefaultGravityZ` (magnitude)
    pub gravity: f32,
    /// `Engine Default__Pawn.CollisionCylinder.CollisionHeight` (half-height)
    pub collision_height: f32,
    /// `AR_Mantle_End` length at rate 1.0 (10 frames); no root motion
    pub vault_land_frames: f32,
    /// `GSM_MantleOverCoverBase.Landed`: roadie run may cut in this long (÷ SpeedModifier)
    /// after landing (`AllowTransitionToAMove`)
    pub vault_roadie_after_land: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ground_speed: 300.0,
            accel_rate: 2048.0,
            dead_zone: 0.2,
            collision_radius: 34.0,
            walking_pct: 0.5,

            roadie_speed_mod: 1.6,
            roadie_steering_pct: 0.33,
            roadie_hold_time: 0.3,
            turn_rate: 6.0,

            run2cover_speed_mod: 2.3,
            run2cover_min_duration: 0.2,
            run2cover_mirror_hold: 0.25,
            run2cover_max_dist: 420.0,
            run2cover_side_max_dist: 240.0,
            roadie_to_cover_dist: 60.0,
            seamless_cover_dot: -0.83,

            roadie_cover_dir_dot: 0.6,
            roadie_cover_after_move: 0.4,

            a_press_keeps_cover_time: 0.75,
            cover_run_speed_mod: 1.5,
            cover_move_pct_mid: 1.0,
            cover_move_pct_std: 1.0,
            break_from_cover_hold: 0.05,
            push_out_speed_mod: 0.75,
            push_out_anim_length: 14.0 / 30.0,

            slip_speed_mod: 1.5,
            slip_max_edge_dist: 96.0,
            slip_stick_up: 0.5,
            slip_raw_stick_window: 0.3,
            slip_travel_side: 116.82,
            slip_travel_fwd: 75.18,
            slip_anim_length: 0.7,

            swat_speed_mod: 1.7,
            swat_max_dist: 512.0,
            swat_camera_dot: 0.85,
            swat_to_roadie_delay: 0.2,
            swat_end_dist: 256.0,
            swat_anim_rate: 1.3,
            swat_end_anim_length: 25.0 / 30.0,
            swat_edge_dist: 96.0,

            evade_speed_mod: 1.15,
            evade_retrigger_delay: 0.65,
            evade_chain_lead: 0.2,
            evade_cover_check_scale: 0.75,
            evade_fwd: EvadeAnim::new(12, 214.73, 22, 236.35),
            evade_bwd: EvadeAnim::new(12, 204.96, 22, 270.60),
            evade_lt: EvadeAnim::new(12, 214.24, 22, 257.37),
            evade_rt: EvadeAnim::new(12, 215.54, 21, 244.31),

            vault_speed_mod: 1.1,
            vault_default_dist: 141.0,
            vault_min_root_scale: 0.5,
            vault_max_dist: 300.0,
            vault_slide_slot_dist: 96.0,
            vault_jump_frames: 21.0,
            vault_up_frames: 15.0,
            vault_over_frames: 20.0,
            vault_rise: 100.8,
            vault_jump_travel: 141.4,
            vault_fall_fwd_vel: 125.0,
            vault_fall_start_dist: 100.0,
            gravity: 750.0,
            collision_height: 78.0,
            vault_land_frames: 10.0,
            vault_roadie_after_land: 0.1,
        }
    }
}

/// One evade direction: start and end anims (30 fps frame counts) and how far the
/// root bone carries the pawn in each.
#[derive(Clone, Copy, Debug)]
pub struct EvadeAnim {
    pub start_length: f32,
    pub start_dist: f32,
    pub end_length: f32,
    pub end_dist: f32,
}

impl EvadeAnim {
    pub const fn new(start_frames: u32, start_dist: f32, end_frames: u32, end_dist: f32) -> Self {
        Self {
            start_length: start_frames as f32 / 30.0,
            start_dist,
            end_length: end_frames as f32 / 30.0,
            end_dist,
        }
    }
}
