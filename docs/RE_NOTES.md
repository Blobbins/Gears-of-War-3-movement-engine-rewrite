# Reverse engineering notes

What we learned from Gears' own code and data, in plain language. This file is the
shareable result. **No decompiled code here**: describe what it does, name the
class/function/property, give the numbers (numbers also go in `GEARS_VALUES.md`).
Decompressed packages, tables and unpacked config stay in `_local/re/`.

## Toolchain (working, 2026-10-07)
1. `tools/inspect_packages.py` — reads package headers, verifies against the disc's
   `Xbox360TOC.txt` MD5 list.
2. UE Viewer's `decompress` tool (built from source, github.com/gildor2/UEViewer,
   `Tools/PackageUnpack`; drop `-lGL -lSDL2` from the link line on a headless box) —
   `decompress -lzx -out=_local/re/gears3/unpacked <pkg>.xxx`.
3. `tools/ue3_tables.py` — lists every object (class, function, state, property) in an
   uncompressed package, with full paths. `--grep`, `--class`, `--tsv`.
4. `tools/unpack_coalesced.py` — turns `Coalesced_INT.bin` into 95 plain `.ini` files.
5. `tools/ue3_bytecode.py` — our UnrealScript bytecode decoder (v828, big-endian).
   Self-test: **17,419 of 17,419 script functions in GearGame decode with exact
   storage and memory sizes** (257 more are native-only stubs). Output is flat
   pseudo-code with jump labels, written to `_local/re/gears3/script/`.
6. `tools/ue3_props.py` — enums and class default properties (`--enum`, `--defaults`).

## Package map — Gears 3 (Xbox 360)
All `.xxx` under `GearGame/CookedXbox360/`. Version 828, licensee 0, engine 8653,
cooker 134, big-endian, LZX-compressed. **None of the checked packages are encrypted.**

| Package | Layout | Size (compressed → raw) | What's in it |
|---|---|---|---|
| `Core.xxx` | fully compressed | 26 KB → 193 KB | UnrealScript core |
| `Engine.xxx` | fully compressed | 1.1 MB → 5.5 MB | Unreal Engine 3 classes |
| `GameFramework.xxx` | fully compressed | 98 KB → 581 KB | shared game classes |
| `GearGame.xxx` | fully compressed | 32 MB → 86 MB | **all Gears gameplay script**: 2,370 classes, 17,676 functions, 461 states |
| `GEARGAMEHORDE_SF.xxx` | standard, 47 LZX chunks | 24 MB | Horde content (seek-free) |
| `Coalesced_INT.bin` | not a package | 2.7 MB | 95 config/localisation files, plain |

Naming: `GSM_*` = Gear Special Move (an action that takes over the pawn: roadie run,
cover slide, chainsaw). `GDT_*` = Gear Damage Type. `GearWeap_*` = weapon.
`*Base` classes live in `GearGame`; concrete content classes (e.g. `GearPawn_LocustDrone`)
belong to `GearGameContent*`, which is not a standalone file on the disc — most likely
cooked into the `_SF` (seek-free) packages. Not yet confirmed.

## Active reload
- **Where:** `GearWeapon` (every gun inherits it).
  - `StartActiveReload` — begins the reload bar.
  - `DetermineActiveReloadSuccessOrFailure` — judges the second button press.
  - `GetActiveReloadValues` — returns the timing window; probably where the three
    "super sweet spot" durations are chosen.
  - `ActiveReloadSuccess` / `FailedActiveReload` (+ `Local*` / `Server*` variants for
    networking) — outcomes.
  - `SetActiveReloadBonusActive` / `TurnOffActiveReloadBonus` /
    `GetActiveReloadDamageMultiplier` — the damage bonus and its timer.
  - HUD: `GearHUD_Base.ActiveReloadSuccess/Fail/ResetActiveReload`.
- **Rules from config** (see GEARS_VALUES): the reload has a fixed length (Lancer 2.5 s).
  A press before the success zone (Lancer: 0.75 s in) or after it fails. Inside the
  sweet spot (Lancer 0.30 s) = good: faster reload. Inside the smaller "super" window =
  perfect: faster still + damage bonus (Lancer 1.3×) for 8 s.
- **Mutators** confirm the behaviour space: `GearMut_AutoActiveReload` (always perfect),
  `GearMut_MustActiveReload`, `GearMut_SuperActiveReload`.
- **Open:** why 3 super-window values (0.100/0.075/0.050)? Difficulty, or shrinking on
  consecutive perfects? What X vs Y mean in the multipliers. Jam duration on failure.
  → needs `GetActiveReloadValues` and `FailedActiveReload` decompiled.
- **WaW plan:** GSC thread on reload start → timer + HUD bar; detect the second press
  (button access TBD in M0); on perfect, scale damage via the zombie damage callback.

## Roadie run
- **Where:** `GSM_RoadieRun` (special move; `GetSpeedModifier`), started from
  `GearPC.TryToRoadieRun`, camera `GameplayCam_RoadieRun`, particles
  `GearPawn.StartRoadieRunPSC`, cover transition `GearPC.TransitionFromRoadieRunToCover`.
- **Rules from config:** 1.6× ground speed (300 → 480 uu/s); can't shoot for 0.22 s
  after stopping; shaky camera dampened by 0.6. Sliding into cover is 2.3×.
- **WaW plan:** while sprint is held, raise move speed scale, lower and shake the view,
  block firing; 0.22 s fire lockout on release.

## Lancer chainsaw
- **Where:** `GSM_ChainsawHold` (rev), `GSM_ChainsawAttack` (the kill),
  `GSM_ChainsawDuel_*` (duels), damage type `GDT_Chainsaw`, victim side
  `AICmd_React_ChainSawVictim`, gore `GearPawn.ChainsawGore`, eligibility
  `GearPawn.CanBeChainsawAttacked` (overridden per enemy, e.g. Corpser, Boomer).
- **Rules from config:** must rev 0.75 s before it can kill; camera FOV 70 while sawing.
- **WaW plan:** hold melee with the Lancer → 0.75 s rev (sound + slowed move) → next
  zombie in melee range is killed with a lock-in animation. Which Locust Gears lets you
  saw is decided per class by `CanBeChainsawAttacked` overrides: copy that list once
  decompiled, don't guess.

## Cover
- **Where:** `GearPawn` (dozens of functions: `FindCoverFromLocAndDir`, `ClaimCover`,
  `SetCoverType`, `IsProtectedByCover`, `LeaveCover`, `CoverAcquired`...). Gears cover
  relies on hand-placed **CoverLink / CoverSlot** actors in every map.
- **WaW plan (M6):** WaW maps have no cover markup. Either trace for nearby walls at
  runtime, or hand-place cover markers per map. Hardest mechanic; stays last.

## Horde 2.0
- **Where:** `GearGameHorde_Base` (`StartNextWave`, `SpawnEnemy`, `CalcMaxAlive`,
  `PrepareEnemyList`, `GetFortXPMultiplier`...), objectives `GearHordeObjective_*`,
  enemy lists `GearHordeEnemyList` (config).
- **Rules from config:** 10-wave loops, $1000 start, $1500 respawn, 30 s setup between
  waves, bonus objective every 4 waves (kill streak, chainsaw kills, headshots...).
- **WaW plan:** the zombie round system stays; Locust replace or mix into rounds, and
  the bonus objectives are an easy, very "Gears" add-on.

## Locust AI
- **Where:** `GearPawn_LocustBase`, `GearPawn_LocustWretchBase`,
  `GearPawn_LocustBoomerBase`, AI controllers `GearAI_*` (e.g. `GearAI_Speedy`,
  `GearAI_Boomer_Melee`), Wretch moves `GSM_Wretch_SwipeAttack/PounceAttack/Scream`.
- **First port: the Wretch** — 350 health, 585 uu/s (nearly 2× a Drone), melee only.
  Closest thing to a WaW zombie, so it's the first Locust to bring over.

## Movement chain (libgears L0) — read from Gears 3's own script

**How much is readable:** GearGame has 17,676 functions; 257 are native-only (C++ in
`default.xex`). All of `GSM_Run2Cover` (21), `GSM_Evade` (12), `GSM_CoverSlip` (9) and
10 of 11 in `GSM_RoadieRun` are UnrealScript. **Native (not readable):** the cover
*search* — `GearPawn.FindCoverFromLocAndDir`, `ValidatePotentialCover`,
`FillCoverPosInfo`, `IsAtCoverEdge`, `PickClosestCoverSlot`, `GetCoverNormal`,
`IsInCover` — plus `GSM_RoadieRun.GetSpeedModifier` and engine physics. These work on
Gears' hand-placed CoverLinks, so libgears replaces them with its own cover finder.

**Special moves** (`GearPawn.ESpecialMove`, values used below): 1 MidLvlJumpOver,
2 StdLvlSwatTurn, 3 CoverRun, 4 RoadieRun, 5 CoverSlip, 6 MantleUpLowCover,
8–11 Evade Fwd/Bwd/Lt/Rt, 12/13 Run2MidCov/Run2StdCov, 16 PushOutOfCover,
65/66 EvadeFromCover crouching/standing, 81 MantleKick.

### 1. Deciding to slide into cover — `GearPC.CanRunToCover`
- Not allowed if already in cover, or (default controls) while doing any special
  move except a move-to-idle transition or PushOutOfCover (`CanTryToRunToCover`).
  Also blocked while falling, carrying a shield, conversing, in slow volumes.
- Direction: the left stick (`RawJoyUp/RawJoyRight`, dead zone **0.2**) rotated into
  camera space. Stick pulled back past the dead zone → no slide. No stick → the
  pawn's facing.
- Stick more sideways than forward → "sideways entry": max distance
  **240 uu** (`Run2CoverPerpendicularMaxDist`); otherwise **420 uu**
  (`Run2CoverMaxDist`) × CheckScale. There is also `EndEvadeRun2CoverMaxDist`
  **150 uu** — by its name the reach at the end of an evade; *where it's used is not
  read yet*.
- Then `GearPawn.CanPrepareRun2Cover` → native cover search. If a friendly AI is
  in the spot, it's told to vacate.
- Acquire cones (dot products): `CoverAcquireFOV 0.7`, `RoadieRunCoverAcquireFOV 0.35`,
  `PawnAcquireCoverFOV 0.02`, `…RoadieRun 0.01`; `CoverSnapScale 1.5`.

### 2. The slide — `GSM_Run2Cover`
- Waits until the pawn has a cover type and the acquired link matches
  (re-checks every 1/60 s), then begins. 5 s safety timeout.
- Movement is **root motion** toward `AcquiredCoverInfo.Location`; rotation is
  locked and blends to face `-CoverNormal` over the slide.
- **Duration = distance / (GroundSpeed × max speed modifier)**, at least
  `MinTravelAnimDuration` **0.2 s**. Speed modifier **2.3** (1.9 with heavy weapon) →
  690 uu/s at 300 base. Blend-in = `min(0.2 / rate, duration / 2)`. Blend-out 0.1 s.
- Wall-hit effects / sliding sound only if the slide is long enough
  (distance² > 2500, i.e. > 50 uu; sound if duration > 0.25 s).
- Becomes "blockable" after `max(0.2/2, 0.1)` s; if blocked, the slide ends.
- While sliding, the stick is watched **once** (`bEnableJoystickMonitoring`, set in
  `BeginSpecialMove`, cleared by `JoystickHeldLongEnough`), on the cover-remapped
  stick. `JoystickHeldLongEnough` checks `HoldingBack > 0` but `HoldingLeft >=
  MinHoldTimeToSwitchMirroring`: **pulling away from the wall cancels at once**
  (`GearPC.AbortRun2Cover`: LeaveCover, clear the queued move, end the slide), while
  left / right only picks the side after **0.25 s**. If the slide ends while still
  watching, `SpecialMoveEnded` sets the side from any left/right hold. This instant
  cancel is the wall bounce (owner, from playing Gears 3: "anytime you press an
  opposite direction the cover animation gets cancelled immediately"). The native
  caller isn't readable; the asymmetric checks plus the owner's report decide it.
- The slide moves the pawn by velocity, not root motion (`Mesh.RootMotionMode = 2`,
  RMM_Ignore; `SetReachPreciseDestination`), so a cancelled slide keeps moving.
  We cap it to GroundSpeed when the 2.3× ends (**assumption**: UE3's walking physics
  is native).
- On landing: if A is held and nothing is queued → `TryASpecialMove` (the chain
  into slip / swat / cover run). An impact anim (`AR_Run_Fwd2Cov_Mid_Stop`, 0.37 s)
  plays but doesn't block anything: the special move has already ended.
- **Can be interrupted by (CanChainMove):** CoverSlip(5), CoverRun(3), MantleKick(81),
  MidLvlJumpOver(1), MantleUpLowCover(6), StdLvlSwatTurn(2). *Not* roadie or evade.

### 3. Roadie run into cover — `GearPC.TransitionFromRoadieRunToCover`
- "Seamless" if `dot(CoverNormal, facing) > -0.83` (not running head-on into it).
- Ends roadie run, acquires cover, plays wall-hit effects. With default controls, a
  seamless entry that isn't at an edge slot goes straight into **CoverRun** (keep
  moving along the cover); otherwise the A button is force-released.
- `RoadieRun2CoverSlideDist` **60 uu**.

### 4. Pressing A while in cover — `GearPC.PlayerTakingCover.TryASpecialMove`
Priority order:
1. If mid **SwatTurn** → after **0.2 s** `TransitionFromSwatToRoadieRun` (swat chains
   straight into roadie run if A is still held).
2. "Monkey moves", first that's allowed (depends on stick + cover): MantleUpLowCover →
   **CoverSlip** → MantleKick → MidLvlJumpOver → **SwatTurn**.
3. Heavy weapon + break-from-cover option → break from cover.
4. At a cover edge (`IsAtCoverEdge`, 0.5) → try an **evade** out.
5. Otherwise start a **0.3 s** timer (`RoadieRunTimer`) → `TryToRoadieRun`: if A is
   held → **RoadieRun**, else CoverRun, else roadie jog.
- `TimeBeforeAButtonPressLeavesCover` **0.75 s**, `BreakFromCoverHoldTime` 0.05 s.

### 5. Why wallbouncing exists (working theory, to verify in Xenia)
Every link above lets the next one start early: slide (2.3×) → as soon as cover is
acquired, A + stick → CoverSlip (1.5×) or SwatTurn (1.7×), which can be chained even
*during* the slide → swat + A held → roadie (1.6×) after 0.2 s → roadie into the next
cover → slide again. Movement never drops to base speed, and the 420 uu reach plus
generous cones keep the next cover in range. To confirm: frame counts per bounce in
Gears 3 (Xenia) vs. the sandbox.

### 6. Cover slip — `GSM_CoverSlip`
- Allowed only in cover, at the cover **edge** on the side you face (mirrored = left),
  if that slot allows slipping, and within **96 uu** of the edge slot.
- Stick must push "up" ≥ **0.5**: raw stick if cover was taken < **0.3 s** ago
  (`LastCoverTime`), else the cover-remapped stick. Not during forced roadie.
- Movement is **root motion** from the slip animation (std or mid cover, mirrored or
  not), played at rate **1.5** (`SpeedModifier`); blend in 0.1/rate, out 0.2/rate.
- Can be interrupted by **RoadieRun, Run2MidCov, Run2StdCov** → slip straight into a
  new slide. Can override CoverRun.
- On end: velocity = direction of the root motion × **GroundSpeed**; if A is held and
  nothing else is queued → **queue roadie run**.

### 7. Swat turn — `GSM_StdLvlSwatTurn`
- Needs cover, at the left/right edge slot (per facing), slot allows swat turn, a
  swat target slot on the linked cover across the gap (`GetSwatTurnTarget`) not
  occupied, no heavy weapon.
- Stick: sideways past the dead zone **toward the open side**, more sideways than
  up/down; camera yaw within dot **0.85** (~32°) of the pawn's facing.
- Moves (precise destination) to target cover location + normal × collision radius,
  speed ×1.7, anims at rate **1.3**; end anim starts within **256 uu** of target.
- Chains into CoverSlip, MidLvlJumpOver, MantleKick.
- **A pressed during the swat** (a new press; the one that started it doesn't count)
  starts a **0.2 s** timer → `TransitionFromSwatToRoadieRun`: if A is still held and
  you're still > 2 × collision radius (68 uu) from the destination → **leave cover +
  forced roadie run** (cancels the swat); otherwise try slip → mantle kick → jump over.
- Max reach: `Engine Default__CoverLink.TurnDist` **512 uu**. The slide
  (`GSM_Run2Cover.CanChainMove`) and cover run (`GSM_CoverRun`) can queue a swat.
- libgears (built 2026-10-07): all of the above, plus two marked guesses: "at the edge
  slot" = within 96 uu of the end (the slip's tolerance; Gears has hand-placed
  slots), and you land facing back toward the gap (the end anim plays mirrored,
  `!bIsMirrored`). The end anim's root motion for the last 256 uu is a placeholder
  (constant 510 uu/s) until the anims are exported. Hosts answer
  `World::find_swat_target` (the test world: the next wall in line past the gap,
  same facing, within 64 uu of the same line).

### 8. Evade — `GSM_Evade`
- Not while in another special move (except move-to-idle / push-out-of-cover), not
  in the air, not with heavy weapon/shield. Direction check: trace 128 uu with a
  12×12×12 box in the evade direction; from cover only if that trace hits nothing.
  Shipped quirk: evade-left rotates by **16834** (not 16384) Unreal rotation units.
- Root motion start + end anims, played at **1.15**; 5× gravity.
- **0.2 s before the end anim finishes**, if A is held: try **run to cover at 0.75×
  reach** (≈315 uu forward), else **queue roadie run**.
- Chains into RoadieRun, Run2MidCov, Run2StdCov. Sets `LastEvadeTime`.

### 10. Leaving cover — `PlayerTakingCover.UpdatePlayerPosture`, `GSM_PushOutOfCover`
- Stick remapped to the cover (`RemapControlsByRotation` with `-CoverNormal`). If
  "up" is dominant and below −dead zone (pulling away from the wall), and no special
  move is running, `BreakFromCoverHoldTimer` counts up; at **0.05 s**
  (`BreakFromCoverHoldTime`) → `DoSpecialMove(SM_PushOutOfCover)`. This is the
  "cancel" off a wall: about 3 frames after landing at the earliest.
- `GSM_PushOutOfCover`: leaves cover as it starts; velocity = −GroundSpeed × facing ×
  **0.75** (225 uu/s straight back); plays the cover-break anim
  (`AR_Cov_Mid_Idle2Idle_Ready_Aim`, 0.4667 s) at 0.75 → 0.62 s.
- **It does not hold you.** Gears' code exempts it from every "busy" check, the same
  way it treats the Move2Idle transitions: `GearPC.CanTryToRunToCover`, `CanEvade`
  (even though movement input is "ignored"), `GSM_Evade.InternalCanDoSpecialMove`,
  `UpdatePlayerPosture`. The owner confirms that in Gears 3 you're off the wall with
  full control at once. libgears: the push out is a shove (225 back), walking runs
  underneath it, and any move (slide, evade, roadie) cuts it short immediately.
  (Its `bDisableMovement` and anim-end timing would say otherwise; the native
  Move2Idle handling is the likely reason and isn't readable.)
- A tap does **not** leave cover by default: `bPressAToBreakFromCover=FALSE`
  (GearPawn.ini); with it on, a neutral-stick tap > 0.75 s after taking cover would.
- `PlayerTakingCover.TryToEvade` needs |remapped stick right| > dead zone: evading
  out of cover is sideways only.

### 11. Vault — `GSM_MidLvlJumpOver` / `GSM_MantleOverCoverBase`
- **When:** A in cover on mid cover with the stick toward the wall (up past the dead
  zone and at least as strong as sideways), within 3 × radius of the cover slot (96 uu
  if still sliding), slots flagged `bCanMantle`, no heavy weapon, and the native
  `FindMantleDistance` finds the far side plus a clear landing (`VerifyLandingClear`:
  trace 100 uu up across, then down; fall between the GearScout limits). Tried after
  the cover slip and before the swat turn.
- **Jump:** faces straight over the cover, plays `AR_Mantle_Start` (mirrored set when
  facing left) at 1.1 with root motion. The anim goes straight up ~101 uu first, then
  forward 141.4; the forward part is scaled by `MantleDistance / 141` (min 0.5).
  Collision height halves while jumping. Physics `PHYS_Flying`.
- **Fall:** at the jump anim's end: physics falling, velocity = facing × horizontal
  distance to the estimated landing / sqrt(drop / |g|); fall anim's root motion is
  ignored; its rate is fitted to the time to the ground (+ 0.07 s).
- **Land:** `AR_Mantle_End` at 1.1, no root motion, ends the move. 0.1 / 1.1 s after
  touching down, holding A turns it into a roadie run (also at the end if A is held).
  `bBreakFromCoverOnEnd`: you leave cover when it ends.
- **Not readable (native):** `GearPawn.FindMantleDistance`,
  `SimulatedAndFindMantleFallEndPoint`. libgears asks the host (`World::mantle_distance`).
- **Not modelled yet:** MantleKick (`GSM_MantleKick`: vault into an enemy behind the
  cover, 200 damage), MantleUp (climb onto flagged cover), MantleDown (drop off a ledge).

### 12. Textures and sounds (export notes)
- Textures: most pixels live in `CharTextures.tfc` / `Textures.tfc` next to the
  packages; put the `.tfc` beside the unpacked package for UE Viewer. Our headless UE
  Viewer build compiled texture export out with `RENDERING 0`: patched
  `Exporters/ExportMaterial.cpp` to export `Texture2D` without the renderer.
  Marcus: `T_COG_Summer_Marcus_D` (2048², CharTextures.tfc), hair
  `T_Cog_Marcus_Summer_Hair_D`; Lancer `COG_AssaultRifle_II_D`, Gnasher
  `Tex_COG_Gnasher` (only 64² here: their big mips are in the 1.3 GB Textures.tfc).
- Sounds: UE Viewer needs `-sounds` (else "Unknown class SoundNodeWave"); 360 waves
  export as `.xma`; ffmpeg's xma2 decoder turns them into MP3 (one wave changes
  channel layout mid-stream: force stereo). Weapon cues: `FireSound_Player`
  (Lancer: a loop + `StopFireSound_Player`), Gnasher `ShotgunFirePlayerCue` + cock after
  `ShotgunCockDelay` 0.3 s, `AR_SuccessSound` / `AR_SuperSuccessSound` / `AR_FailSound`.
  Reload sound timings come from anim notifies we don't read yet (approximate).

### 9. Implication for libgears
Slip, swat and evade distances come from **animation root motion**, so exact
distances need the exported animations (L4). Until then the core uses clearly marked
placeholders. Every rule above (conditions, chains, timers, speeds) is in the core.

## Animations (libgears L4) — 2026-10-07
- Marcus's mesh (`COG_Marcus_Fenix_CamSkel`), his body AnimSets and the Lancer
  (`COGAssaultRifle`) are in **GearGame.xxx** itself (881 sequences, 30 AnimSets).
  "CamSkel" is the human skeleton's name. Cover/heavy-weapon extras are also in the
  game-mode packages (GEARGAMETDM_SF etc.).
- Export (all local): UE Viewer built headless (`RENDERING 0`, `SDL_Delay` →
  `usleep`, link without `-lGL -lSDL2`). Mesh → glTF (`-export -gltf`); AnimSets → psa
  (`-export ... AnimSet`; umodel only writes glTF anims from its viewer). Our
  `tools/gears_anim_pack.py` merges psa clips onto the glTF skeleton → `marcus.glb`.
- Conversions match UE Viewer: psa keys are mirrored on write (undo Y of positions,
  Y and W of quats); glTF swaps Y/Z, ×0.01, conjugates the root. The mesh faces
  +Y (engine) = +Z (glTF); actor right = −X (engine).
- Locomotion is layered: `*_Core` full-body + `ADD_AR_*` additive weapon layers.
  **Checked by forward kinematics** (offline, on the packed clips): applied as
  **delta × base** (pre-multiplied) the hands land exactly on `b_MF_IK_Hand_R/L` and
  `b_MF_Weapon_R_End` on `b_MF_IK_Gun` (0.0 uu); base × delta (three.js's additive)
  misses by 17–52 uu. So the page applies the ADD layers itself.
- Looping cycles have no duplicate end key: UE3 blends the last key back to the first
  over one more frame, so the packer repeats frame 0 at n / 30 for loops.
- Cover pose: the pawn faces the wall; the `Cov_*_Idle_Core` body turns ~80° toward
  the facing side (non-mirrored = right). Mirrored = the same anims mirrored
  (MirrorNode); we flip the model on X.
- Root motion: evade (RootMotionMode 3), slip and the swat end anim move the pawn by
  their root bone; Run2Cover, swat dash and push out are velocity-driven (no root
  travel). Numbers in GEARS_VALUES.md → Animations; the core now uses them.
- Weapon hold: Gears' `RightHand` socket (`b_MF_Weapon_R_End` + 1.48 uu). The barrel
  runs along that bone's +X (idle forward-left and down, roadie forward-down, mid cover
  along the wall); the Lancer mesh's barrel is its +Z, so it's turned 90° about Y. No
  IK needed once the additive order is right.
- Cover movement: `PlayerTakingCover.UpdatePlayerPosture` turns the pawn to the stick
  side at once (`SetMirroredSide`) and sets the cover direction; the native cover
  physics moves it along the cover at GroundSpeed × `CoverMovementPct[CoverType]`
  ([0, 1, 1] → 300 uu/s for mid and standing), stopping at the ends.
