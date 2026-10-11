# Gears values

Every Gears of War number the mod uses, with where it came from. **No guessed numbers.**
A value without a source stays `TBD`.

Sources, in order of preference:
1. The game's own data from my dump: config (`Coalesced_INT.bin`, unpacked with
   `tools/unpack_coalesced.py`) or decompiled class defaults.
2. The Gears wiki or another documented source (link it).
3. Frame counting from my own gameplay footage (say which game, how many samples).

Source shorthand: `GearWeapon.ini [GearGame.GearWeap_AssaultRifle] ReloadDuration`
means file / section / key inside Gears 3's unpacked `Coalesced_INT.bin`
(Xbox 360, version 828/0, MD5 verified against the disc's `Xbox360TOC.txt`).

## Units — open question
Gears uses Unreal units (uu), WaW uses its own units (1 unit ≈ 1 inch). Speeds and
ranges below are in **uu**. Conversion is **TBD**: measure a Gears pawn's collision
height (from class defaults) against WaW's player height before converting anything.

## Active reload (M1) — Gears 3
| Value | Base weapon | Lancer | Source |
|---|---|---|---|
| Reload duration (s) | 0.01 (placeholder) | **2.5** | `GearWeapon.ini [GearWeap_AssaultRifle] ReloadDuration` |
| Success zone starts at (s into reload) | halfway (−1.0 = midpoint) | **0.75** | `AR_PossibleSuccessStartPoint` |
| Sweet spot ("good") duration (s) | 0.25 | **0.30** | `AR_SweetSpotDuration` |
| Perfect ("super") windows (s) | 0.100 / 0.075 / 0.050 | inherits | `[GearGame.GearWeapon] ActiveReload_SuperSweetSpotDurations` — three values; **why three is TBD** (needs `GearWeapon.GetActiveReloadValues`) |
| Damage multiplier on perfect | 1.2 / 1.2 | **1.3 / 1.3** | `ActiveReload_DamageMultiplier=(X,Y)` — meaning of X vs Y **TBD** |
| Bonus duration (s) | 7.0 | **8.0** | `AR_MagicBulletsTimeoutDuration` |
| Explosive radius multiplier | 1.25 | inherits | `ActiveReload_ExploRadiusMultiplier` |
| Reload anim speed-up, good / perfect | 1.66 / 2.5 | inherits | `ActiveReloadAnimationSpeedUp`, `SuperActiveReloadAnimationSpeedUp` |
| Jam on fail | the reload switches to the jam anim at the same point and ends with it: 4.0 s total | `PlayPawnActiveReloadFailAnimation`; `ADD_AR_Reload_Fail` / `ADD_SG_Reload_Fail` = 120 frames |
| Window layout | zone starts at `AR_PossibleSuccessStartPoint`; perfect window first (tier 0/1/2 = 0.100 / 0.075 / 0.050 s), then the good window; one try per reload; anywhere else = jam | AI path in `Reloading.BeginState` (`ARStartTime + SuperSweetSpotOffset`, `+ SuperSweetSpotDuration + SweetSpotDuration`); offset **assumed 0** (native `GetActiveReloadValues`); the three super values are tiers (`GearPawn.ActiveReload_CurrTier`, reset by `FailedActiveReload`) |
| Good / perfect | rest of the reload × 1/1.66 / × 1/2.5; perfect also starts the damage bonus and raises the tier | `PlayActiveReloadSuccess` (`BS_ScalePlayRate`), `ActiveReloadSuccess` |
| Gnasher | zone at 0.85 s, good 0.25 s, bonus 9.75 s × 1.2; success plays `ADD_SG_Reload_Success` (1.833 s) instead of speeding up | `[GearWeap_Shotgun]`, `BS_PawnWeaponReloadSuccess` |

## Lancer (GearWeap_AssaultRifle) — Gears 3
| Value | Gears 3 | Source |
|---|---|---|
| Damage per bullet (player / AI) | 51 / 25 | `WeaponDamage`, `WeaponDamageAI` |
| Magazine / max spare | 60 / 360 | `WeaponMagSize`, `WeaponMaxSpareAmmo` |
| Starting magazines | 3 | `InitialMagazines` |
| Rate of fire (rounds/min) | 550 | `WeaponRateOfFire` |
| Recoil | 35 | `WeaponRecoil` |
| Range bands short / medium / long (uu) | 750 / 2048 / 4096 | `Range_*` |
| Long-range damage scale | 0.8 | `DamageMod_LongRange` |

## Gnasher (GearWeap_Shotgun) — Gears 3
| Value | Gears 3 | Source |
|---|---|---|
| Damage per shot (player / AI) | 500 / 350 | `GearWeapon.ini [GearWeap_Shotgun] WeaponDamage`, `WeaponDamageAI` |
| Pellets | 10, fixed pattern (yaw, pitch URot): (0,0) (0,284) (−350,−256) (350,−256) (0,900) (875,425) (−875,425) (−875,−425) (875,−425) (0,−900) | `Default__GearWeap_Shotgun.PelletSpread` (read with `UE3_ARRAY_BYTES=80`) |
| Damage split | full if ≥ 75 % of pellets hit one target, else damage × hits / 10; × 1.2 if the target is < 256 uu away | `GearWeap_Shotgun.SetTransientShotgunDamage` |
| Spread × | 0.5 aiming (and > 1.5 s since an evade), 1.1 within 1.5 s of an evade, else 0.9 + 0.2 × speed / GroundSpeed | `GearWeap_Shotgun.FireAllPellets` |
| Rate of fire | 60 rounds/min (1 s) | `WeaponRateOfFire`; interval = 60 / rate (`GearWeapon.GetFireInterval`) |
| Magazine / spare / reload | 8 / 31 / 3.0 s | `WeaponMagSize`, `WeaponMaxSpareAmmo`, `ReloadDuration` (= `ADD_SG_Reload` length) |
| Range bands / damage × | 300 / 1000 / 1300 / 1650 uu → 1.0 / 0.75 / 0.45 / 0.35, lerped band to band | `Range_*`, `DamageMod_*`, `GearWeapon.ModifyDamage` |
| Aim error (aiming → hip) | 0 → 8 (units: **assumed degrees**) | `WeaponAimError`, `GearWeapon.GetPlayerAimError` = Lerp(X, Y, Accuracy) |
| Recoil | 300 (Lancer 35) — **used as an upward camera kick in URot, assumption** | `WeaponRecoil` |
| Pump (cocking) | firing state lasts 0.25 × interval, then `ShotgunCocking` 0.75 × interval: pawn `BS_PawnShotgunCock` (`ADD_SG_Idle_Ready_Chamber`) and gun `SG_Chamber` played by duration; cock sound `ShotgunCockDelay` 0.3 s and shell bounce `ShotgunCockShellImpactDelay` 0.7 s into it | `GearWeap_Shotgun.TimeWeaponFiring`, `ShotgunCocking.BeginState`, `PlayShotgunCocking` |

Shared: aiming forces a walk at GroundSpeed × `Pawn.WalkingPct` 0.5 = 150 uu/s
(`GearWeapon.bForceWalkWhenTargeting`, `GearPC.HandleWalking`), no roadie run
(`PlayerWalking.TryASpecialMove`); `Inaccuracy_Interp_Base_Speed` 4.5. `Accuracy` itself
is native (`GearInventoryManager`): the range uses 0 aiming and still, else 1
(**assumption**). Lancer melee band: 200 uu × 1.3 (`DamageMod_MeleeRange`). Aiming
camera `GameplayCam_Targeting`: FOV 50, offsets high (−40, 40, 24) / mid (−70, 40, 6) /
low (−125, 45, 20), BlendTime 0.067; over mid cover `PawnRel_Mod_PopUp_MidLevel`
(15, 34, 86). Range targets use Locust Drone health 1000.

## Chainsaw (M3) — Gears 3
| Value | Gears 3 | Source |
|---|---|---|
| Rev-up before it can kill (s) | **0.75** | `GearPawn.ini [GearGame.GSM_ChainsawHold] ChainsawWarmupTime` |
| Delay between chainsaw vs normal melee (s) | 0.25 | `GearWeapon.ini [GearWeap_AssaultRifle] ChainsawVsNormalDelayTimer` |
| Camera FOV while sawing | 70 | `CameraFOVWhileChainsawing` |
| Charge range (uu) | 1200 | `Range_MeleeCharge` |
| Kill animation length | TBD | animation data (`GSM_ChainsawAttack` uses an AnimSequence; read with umodel) |

## Roadie run (M2) — Gears 3
| Value | Gears 3 | Source |
|---|---|---|
| Base run speed (uu/s) | 300 | `GearPawn.ini [GearGame.GearPawn] DefaultGroundSpeed` |
| Roadie run speed multiplier | **1.6×** (→ 480 uu/s) | `[GearGame.GSM_RoadieRun] SpeedModifier` |
| Hold time before roadie run starts (s) | 0.3 | `[GearGame.GearPawn] RoadieRunTimer` (meaning to confirm in script) |
| Can't fire after stopping for (s) | 0.22 | `PostRoadieRunFireDelay` |
| Shaky cam dampening | 0.6 | `RoadieRunShakyCamDampeningFactor` |
| Slide-into-cover speed multiplier | 2.3× | `[GearGame.GSM_Run2Cover] SpeedModifier` |

## Player (COG) — Gears 3
| Value | Gears 3 | Source |
|---|---|---|
| Health regen | 20%/s after 3 s | `HealthRechargePercentPerSecond`, `HealthRechargeDelay` |
| Downed (DBNO) death threshold | −200 | `DBNODeathThreshold` |
| Stopping power (max slow when shot) | 0.67 | `MaxStoppingPower` |
| Base health | TBD (`DefaultHealth=1` in base class; real value set per pawn/difficulty) | |

## Locust (M4) — Gears 3
| Enemy | Health | Speed (uu/s) | Damage | Source |
|---|---|---|---|---|
| Drone | 1000 | 300 | melee 80–100 | `[GearGame.GearPawn_LocustBase]`, `[GearGameContent.GearPawn_LocustDrone]` |
| Wretch | **350** | **585** | swipe/pounce speed 1.15× | `[GearGame.GearPawn_LocustWretchBase]`, `[GSM_Wretch_SwipeAttack]` |
| Dark Wretch | 300 | 585 | explodes on death: 175 dmg, 350 uu radius | `[GearGameContent.GearPawn_LocustDarkWretch]` |
| Boomer | **2500** | **150** | melee 250; snipers do 0.65× | `[GearGame.GearPawn_LocustBoomerBase]` |
| Locust downed chance | 40% | | | `[GearGame.GearPawn] LocustChanceToDBNO` |

## Horde 2.0 (reference for Locust rounds) — Gears 3
| Value | Gears 3 | Source |
|---|---|---|
| Waves per loop | 10 | `Xbox360-GearGame.ini [GearGame.GearGameHorde_Base] MaxWaves` |
| Starting cash | 1000 | `InitialCash` |
| Respawn cost | 1500 | `RespawnCost` |
| Setup phase between waves (s) | 30 | `DeploymentPhaseDuration` |
| Revive points | 100 | `PointsFor_Revives` |
| Bonus objective every N waves | 4 | `BonusObjectiveInterval` |
| Extended waves scale Locust health / accuracy / damage taken by COG | 2.0× each, then 2.5× all three | `ExtendedWaveList` (which wave they start on: TBD) |
| Enemy sets | CoreSet, Queen, Savage, LambentWithZerker, LambentWithGunker | `[... GearHordeEnemyList]` sections |

## libgears movement (Gears 3, class defaults read with `tools/ue3_props.py` and config)
| Value | Gears 3 | Source |
|---|---|---|
| Base ground speed | 300 uu/s | `Default__GearPawn.GroundSpeed`, `DefaultGroundSpeed` |
| Slide into cover speed × (heavy weapon) | 2.3 (1.9) | `Default__GSM_Run2Cover.SpeedModifier/HeavyWeaponSpeedModifier` |
| Slide min duration | 0.2 s | `GSM_Run2Cover.MinTravelAnimDuration` |
| Slide: hold left to mirror | 0.25 s | `GSM_Run2Cover.MinHoldTimeToSwitchMirroring` |
| Slide reach forward / sideways / after evade | 420 / 240 / 150 uu | `Default__GearPawn.Run2CoverMaxDist / Run2CoverPerpendicularMaxDist / EndEvadeRun2CoverMaxDist` |
| Roadie → cover slide distance | 60 uu | `RoadieRun2CoverSlideDist` |
| Seamless roadie→cover threshold | dot > −0.83 | `GearPC.TransitionFromRoadieRunToCover` (script) |
| Cover acquire cones | 0.7 / roadie 0.35; pawn 0.02 / 0.01 | `CoverAcquireFOV`, `RoadieRunCoverAcquireFOV`, `PawnAcquireCoverFOV*` |
| Cover snap scale | 1.5 | `CoverSnapScale` |
| Roadie run speed ×, steering, min speed, max slow time | 1.6, 0.33, 0.3, 0.5 s | `Default__GSM_RoadieRun` |
| Roadie camera turn time | (0.15, 0.9) | `GSM_RoadieRun.RRCameraTurnTime` |
| A-held delay before roadie | 0.3 s | `GearPC.RoadieRunTimer` |
| Swat → roadie delay | 0.2 s | `PlayerTakingCover.TryASpecialMove` (script) |
| A press ignored for leaving cover within | 0.75 s | `GearPC.TimeBeforeAButtonPressLeavesCover` |
| Break from cover hold time | 0.05 s | `GearPC.BreakFromCoverHoldTime` |
| Push out of cover: shove, length | 0.75 (225 uu/s back), 0.4667 s anim / 0.75 = 0.62 s; full control throughout (owner + GearPC exemptions) | `Default__GSM_PushOutOfCover.SpeedModifier`, `DoPushOutSpecifics`; anim `AR_Cov_Mid_Idle2Idle_Ready_Aim` 14 frames (`AnimSetMarcus_CamSkel`, GEARGAMETDM_SF) |
| Cover slip anim length | 0.7 s (21 frames) → 0.467 s at 1.5× | `AR_Cov_Mid_Slip` (`AnimSetMarcus_CamSkel`, GEARGAMETDM_SF) |
| Slide → cover impact anim | 0.367 s (doesn't block input) | `AR_Run_Fwd2Cov_Mid_Stop` (same set) |
| Slide cancel (stick away from the wall) | instant (`HoldingBack > 0`) | `GSM_Run2Cover.JoystickHeldLongEnough` → `GearPC.AbortRun2Cover`; confirmed by the owner's play experience |
| Slide side pick (stick left / right) | 0.25 s | same function, `MinHoldTimeToSwitchMirroring` |
| Speed after a cancelled slide | 300 uu/s along the slide (**assumption**: capped to GroundSpeed) | slide is velocity-driven (`RootMotionMode = 2`); cap is UE3 native physics |
| A tap leaves cover | off by default | GearPawn.ini `bPressAToBreakFromCover=FALSE` |
| Stick dead zone | 0.2 | `GearPC.DeadZoneThreshold` |
| Cover slip speed ×, check offset | 1.5, (100, 120, 0) uu | `Default__GSM_CoverSlip` |
| Cover run speed × | 1.5 | `Default__GSM_CoverRun`, GearPawn.ini |
| Moving along cover (no A) | × 1.0 of GroundSpeed (300 uu/s), mid and standing | `Default__GearPawn.CoverMovementPct` = [0.0, 1.0, 1.0] by `ECoverType` |
| Roadie auto-cover: check dir · facing, time since last move | > 0.6, > 0.4 s | `GearPC.PlayerTick` |
| Evade end anim speed modifier | 0.75 | GearPawn.ini `[GSM_Evade]` — **not used by the script read so far** |
| Swat turn speed ×, anim rate, end distance | 1.7 (510 uu/s), 1.3, 256 uu | `Default__GSM_StdLvlSwatTurn` |
| Swat turn max reach | 512 uu | `Engine Default__CoverLink.TurnDist` |
| Swat turn camera check | camera · facing ≥ 0.85 | `GSM_StdLvlSwatTurn.InternalCanDoSpecialMove` |
| Swat → roadie | A pressed during the swat, 0.2 s later still held and > 68 uu (2 × radius) from the target | `GearPC.PlayerTakingCover.TryASpecialMove`, `TransitionFromSwatToRoadieRun` |
| Swat "at the edge" tolerance | 96 uu (**assumption**, reuses the slip's) | Gears uses hand-placed edge slots |
| Mid / standing cover heights | > 70 uu mid, > 160 uu standing | `Engine Default__CoverLink.MidHeight / StandHeight` |
| Evade speed ×, gravity ×, accel ×, cam rotation | 1.15, 5.0, 100, 0.5 | `Default__GSM_Evade` |
| Evade retrigger delay (after the evade ends) | 0.65 s | `GearPC.EvadeRetriggerDelay`, set by `GearPC.NotifyEvadeFinished` → `CannotRetriggerEvade` timer, checked in `CanEvade` |
| `MinTimeBetweenEvades` | 2.0 s | `Default__GearPawn` — **not used by the player's script** (AI only?) |
| Post-evade / post-roadie fire delay | 0.12 / 0.22 s | `PostEvadeFireDelay`, `PostRoadieRunFireDelay` |

## Vault over mid cover (Gears 3) — `SM_MidLvlJumpOver`
`GSM_MidLvlJumpOver` is a thin subclass of `GSM_MantleOverCoverBase` (mid cover only:
`FindCoverType() == CT_MidLevel`). Logic read with `tools/ue3_bytecode.py`.
| Value | Gears 3 | Source |
|---|---|---|
| Play rate | 1.1 | `Default__GSM_MidLvlJumpOver.SpeedModifier` |
| Stick | toward the wall, past the dead zone, up ≥ sideways | `GSM_MantleOverCoverBase.InternalCanDoSpecialMove` (RemappedJoyUp/Right) |
| Distance to the slot | ≤ 3 × radius (102 uu); during a slide ≤ 96 uu | same function (`IsDoingSpecialMove(SM_Run2MidCov)`) |
| Order on A in cover | MantleUp → CoverSlip → MantleKick → **JumpOver** → SwatTurn | `GearPC.PlayerTakingCover.TryASpecialMove` |
| Chains into it | Run2Cover, SwatTurn | `GSM_Run2Cover.CanChainMove`, `GSM_StdLvlSwatTurn.CanChainMove` |
| Chains after it | RoadieRun, Run2MidCov, Run2StdCov | `GSM_MantleOverCoverBase.CanChainMove` |
| Default mantle distance | 141 uu (= the jump anim's 141.4) | `DefaultMantleDistance`; `RootMotionScaleFactor = max(dist / 141, 0.5)` (`FindMantleDistance`) |
| Max / min lateral distance | 300 / 16 uu | `Default__GearScout.MaxMantleLateralDist / MinMantleLateralDist` |
| Fall distance allowed | 37–196 uu (+ height/2 + 10) | `GearScout.MinMantleFallDist / MaxMantleFallDist`, `VerifyLandingClear` |
| Fall forward velocity / fall trace start | 125 uu/s / 100 uu | `FallForwardVelocity`, `VerticalFallStartDist` |
| Fall velocity used | facing × 2D distance to the estimated landing / sqrt(drop / \|g\|) | `GetFallVelocityToHitEstimatedLandingPos` |
| Gravity | −750 uu/s² | `Engine Default__WorldInfo.DefaultGravityZ` (a map can override it) |
| Fall anim timing | rate 1.1 × 0.3, then rescaled to sqrt(dist to ground / \|g\|) + 0.07 s | `PlayFallAnimation`, `PlayFall`, `AdjustFallingPlayRate` |
| Roadie run out of the landing | A held, 0.1 / 1.1 s after landing; or A held when it ends | `Landed` → `AllowTransitionToAMove`, `CheckTransitionToAMove`, `SpecialMoveEnded` |
| Leaves cover | at the end | `bBreakFromCoverOnEnd` |
| Far side of the cover | far face + collision radius (**my approximation**) | `GearPawn.FindMantleDistance` is native |
| Mantle up onto cover | only where slots are flagged `bCanClimbUp && bCanPopUp && !bCanMantle` — **not modelled** | `GSM_MantleUpBase.InternalCanDoSpecialMove` |

Worked example (24 uu slab, cover 34 uu off it): MantleDistance 92 → jump 92.3 uu
forward, rising 100.8; fall 0.518 s at 158 uu/s → 82 uu; total 174 uu in 1.458 s.

## Aim offsets (Gears 3) — test range only
`GearAnim_AimOffset` nodes in `AT_MarcusLayered` hold, per bone, 9 rotations + translations
(left/centre/right × up/centre/down) and the aim ranges. Standing rifle: node `_2`
(`AR_Idle_Ready_Aim_*`, 42 bones, vertical −1..1); over mid cover: node `_23`
(`AR_Cov_Mid_Downsights_*`, 51 bones). Read by `tools/gears_aimoffset.py`. **Assumptions:**
Aim.Y = view pitch / 90°, Aim.X = 0 (the pawn faces the aim); rotations only. The skeleton
frame needs the inverse rotation (found by test: aiming up raises the barrel ~20° at 34°).

Chatter (`GUD_COGMarcusG*_SF_LOC_INT`): kill / double kill / close Gnasher kill / weapon
swap lines from Marcus's own banks; chance 60 % (30 % on swap) and a 4 s gap are
**assumptions** (the GUDS tables aren't decoded).

## Pawn size (Gears 3)
| Value | Gears 3 | Source |
|---|---|---|
| Collision radius / half-height | 34 / 78 uu (pawn 156 uu tall) | `Default__GearPawn.CollisionCylinder` sets neither, so they come from `Engine Default__Pawn.CollisionCylinder` (`CollisionRadius`, `CollisionHeight`). `Default__GearPawn_COGGear` sets radius 34 again. |
| Eye height | 60 uu above the pawn's centre | `Default__GearPawn.BaseEyeHeight` / `EyeHeight` |

This also answers the "Units" question above: a Gears pawn is 156 uu tall, so for a
~1.8 m soldier 1 uu ≈ 1.15 cm.

## Camera (Gears 3) — used by the 3D test range, not by gears_core
UE3 offsets are (X forward, Y right, Z up) in the camera's own frame; `Y` flips when
the pawn is mirrored. "High / Mid / Low" are blended by camera pitch.
| Value | Gears 3 | Source |
|---|---|---|
| FOV (horizontal): default / roadie / cover | 80° / 105° / 95° | `GearCamera.ini [GameplayCam_Default] FOVAngle`, `[GameplayCam_RoadieRun]`, `[GameplayCam_Cover]` |
| FOV scaling of look speed | look × FOV / 90 | `PlayerInput.PlayerInput` (`FOVScale = FOV * 0.01111`), `bEnableFOVScaling = True` |
| Default view offset High / Mid / Low | (−128, 56, 40) / (−160, 48, 16) / (−160, 48, 56) | `Default__GameplayCam_Default.ViewOffset` |
| Roadie view offset High / Mid / Low | (0, 56, 50) / (−88, 48, −40) / (−88, 56, −40) | `Default__GameplayCam_RoadieRun.ViewOffset` |
| Cover view offset High / Mid / Low | (−128, 0, 24) / (−160, 0, 0) / (−160, 0, 32) | `Default__GameplayCam_Cover.ViewOffset` |
| Cover pawn-relative shift: standing / mid-level / at the edge | (0, 40, 0) / (−32, 40, −12) / +(0, 84, 0) | `GameplayCam_Cover.PawnRel_Posture_Standing / _MidLevel / PawnRel_Mod_CoverEdge` |
| Cover camera yaw limit | ±13650 URot (±75°) | `GameplayCam_Cover.ViewMaxLimit / ViewMinLimit` |
| Evade view offset adjustment | (0, 0, −28) | `GameplayCam_Default.ViewOffsetAdjustment_Evade`, applied in `AdjustViewOffset` when `IsEvading()` |
| Camera pivot ("worst location") | pawn + (−8, 1, 95) rotated by pawn (roadie (−20, 10, 60), cover (−8, 10, 90)) | `WorstLocOffset`, used by `GearGameplayCameraMode.GetCameraWorstCaseLoc` |
| Origin follow speed per axis (fwd, side, up) | default (5, 20, 20), roadie (8, 20, 20); cover 12 | `PerAxisOriginLocInterpSpeed`, `GameplayCam_Cover.OriginLocInterpSpeed` |
| Cover camera blend time | 0.15 s | `GameplayCam_Cover.BlendTime` |
| Roadie camera turn time | (0.15, 0.9) | `GSM_RoadieRun.RRCameraTurnTime` (we use 0.15) |
| Look speed at full stick | yaw 100 × 300 = 30000 URot/s, pitch 100 × 250 = 25000 URot/s | `PlayerInput.PlayerInput` (`TimeScale = 100 * DeltaTime`), `Xbox360-GearInput.ini LookRightScale=300, LookUpScale=-250` |
| Look sensitivity (default "medium") | × 0.8 | `GearPC.GetSensitivityMultiplier` → `RotationSensitivityMedium` |
| Pitch sensitivity ratio | × 0.75 | `GearPlayerInput.LookPitchSensivityRatio` |
| Yaw view acceleration | over 0.5 stick, ramps +1/1.3 s to 0.6 max, look × (1 + 2.5 × pct); decays 4× as fast | `GearPlayerInput.ViewAcceleration`, `YawAccelThreshold/RampUpTime/PctMax/Multiplier` |
| Pitch auto-centre | after 2.0 s idle at 0.5; roadie 1.75 toward −3000 URot | `GearPlayerInput.PitchAutoCenter*` (the function itself not read yet) |

**Assumptions in the 3D range (not from Gears):** the camera origin is the pawn's
centre plus eye height (the native `GetCameraOrigin` isn't readable script); pitch
blends Mid→Low looking up and Mid→High looking down over ±90° (the UDK
`GameThirdPersonCameraMode` rule); mode changes blend over 0.15 s; the pitch limit is
±60°; camera collision is a simple wall test. Cover-run, slip and roadie shake have
no camera of their own yet.

Anim lengths come from Marcus's *camera-skeleton* anim set (`AnimSetMarcus_CamSkel*`
in GEARGAMETDM_SF.xxx), which carries the camera-bone track of each body animation;
assumed the same length as the body anim. Read with our own property reader; no
animation data is copied.

## Animations (Marcus, `AnimSetMarcus_CamSkel*` in GearGame.xxx, read from my export)
Lengths are NumFrames / 30 fps; travel is the root bone (`b_MF_Root`) from first to
last key, in the pawn's frame (forward, right). Exported with UE Viewer, read by
`tools/gears_anim_pack.py` (writes `_local/exports/marcus/pack/marcus_anims.json`).
| Anim | Frames | Root travel (uu) | Played at | Used for |
|---|---|---|---|---|
| `AR_Evade_Fwd_Start` / `_End` | 12 / 22 | 214.73 / 236.35 forward | 1.15 (`GSM_Evade.SpeedModifier`) | EvadeFwd: 451 uu in 0.986 s |
| `AR_Evade_Bwd_Start` / `_End` | 12 / 22 | 204.96 / 270.60 back | 1.15 | EvadeBwd |
| `AR_Evade_Lt_Start` / `_End` | 12 / 22 | 214.24 / 257.37 left | 1.15 | EvadeLt (straight left; the 16834 is only the trace) |
| `AR_Evade_Rt_Start` / `_End` | 12 / 21 | 215.54 / 244.31 right | 1.15 | EvadeRt |
| `AR_Cov_Mid_Slip` | 21 | 116.82 to the side, 75.18 forward | 1.5 | CoverSlip |
| `AR_Cov_Mid_SwatTurn_Start` / `_End` | 6 / 25 | 0 / 254.24 to the side | 1.3 (`AnimRateScale`) | SwatTurn: end anim covers the last 256 uu (`EndTurnAnimDist`) |
| `AR_Cov_Mid_Idle2Idle_Ready_Aim` | 14 | 0 (velocity-driven) | 0.75 | PushOut |
| `AR_Run_Fwd2Cov_Mid_Start` / `_Stop` | 11 / 13 | 0 (precise destination) | by duration / 1.0 | Run2Cover / landing |
| `Roadie_Run_Core`, `AR_Run_Fwd_Ready_Core`, `AR_Walk_Fwd_Ready_Core` | 16 / 20 / 40 | 0 (in place) | — | locomotion |
| `AR_Mantle_Start` (+ `_Mirrored_`) | 21 | straight up to 101.2 by frame 15, then 141.4 forward by frame 20 (ends 100.8 up) | 1.1 | Vault jump (root motion × RootMotionScaleFactor) |
| `AR_Mantle_Mid` (+ `_Mirrored_`) | 4 | 51.7 fwd, −79.8 (discarded: physics fall) | 0.33, rescaled | Vault fall |
| `AR_Mantle_End` (+ `_Mirrored_`) | 10 | 0 | 1.1 | Vault landing |
Additive layers (`ADD_AR_*`, in `AnimSetMarcus_CamSkel_LayeredNew`) hold the rifle
on top of the `*_Core` locomotion. `bAnimRotationOnly = true`: only
`UseTranslationBoneNames` (pelvis, weapon, IK, armour and finger bones) take the
animation's translation. Weapon: GearPawn `RightHandSocketName = RightHand`
(socket on `b_MF_Weapon_R_End`, +1.48 uu); with the ADD layers applied as delta ×
base, the anims put that bone exactly on `b_MF_IK_Gun` and the hands on
`b_MF_IK_Hand_R/L`. Barrel along the socket bone's +X.
