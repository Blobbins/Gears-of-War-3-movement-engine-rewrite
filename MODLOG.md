# MODLOG

Newest entries at the top. One entry per change.

Format:
**Changed** / **Why** / **Tested how** / **Result** / **Still broken / not tested** / **Next**

---

## 2026-10-08 Fallout 4 proof of concept: owner calls it a success

**Changed (ports/fallout4, over several builds):** Gears movement through
`ApplyMovementDelta` (Fallout's delta is facing-relative: x right, y forward); cover,
slide-into-cover, vault and roll, with the Gears world answered by Fallout ray casts
(`bhkPickData` + `TESObjectCELL::Pick`, layer LOS, rays start 24 uu out); strict wall
test (one face, upright 25–85 uu, ≥ 60 uu wide) that steps past up to 4 hits of
clutter; during timed moves and in cover Gears leads (no position resync); speed cap
1000 uu/s; vault places the actor directly. Left Alt also acts as A.
**Failures on the way (fixed):** crash on load from `Actor::GetCollisionFilter`
(removed; the ID existed in the Address Library, the call shape is suspect); first
moves ran sideways (world vs facing-relative delta); only the first movement sub-step
was replaced (Fallout's own movement mixed in); roll launched the player (resyncing to
Fallout mid-roll made speeds oscillate to ±80,000 uu/s); cover dropped every time
(position resync read as stepping away); fake cover from clutter.
**Tested how:** in game by the owner, step by step, with the plugin log.
**Result:** movement, rolls and cover work "well enough to call it a success".
**Still broken / not tested:** W + Shift slides into cover less reliably than A/D +
Shift from a distance (look-past-clutter build installed, not yet tested); getting stuck
on small objects in Gears mode only (stuck logging in place, cause unknown); no swat
turns, no weapons / damage in Fallout yet, no gamepad.
**Next:** whenever the owner picks it up: stuck hunt from the log, swat turns, Fallout
weapons with Gears rules.

---

## 2026-10-08 Fallout 4: probe worked, puppet worked, step 3 (Gears movement) written

**Changed:** `ports/fallout4` step 1 (probe) ran: plugin, gears.dll and marcus.glb load
in Fallout 4 1.11.240 with F4SE 0.7.9; the player skeleton was logged (bone names,
axes: NiMatrix3 is stored transposed vs column-vector math). Step 2 (puppet: hook
`PlayerCharacter::UpdateAnimation` 0x9F, after the original pose Fallout's bones from
the rig in the body frame) **worked first time** per the owner: "the animations seem
to be working, not twisted bones". Step 3 written: Fallout input → libgears character,
Gears movement swapped into `ApplyMovementDelta` (0xAD), body turned to the Gears
facing, F8 toggle, logging of the first 30 movement calls to learn the delta's frame.
gears.dll moved to `Plugins/libgears/` (F4SE probed it as a plugin).
**Build problems solved on the PC:** xmake had a stale "no Visual Studio" cache
(`xmake global --clean`); CMake picked MinGW's g++ (`set CC=cl`, `set CXX=cl`).
**Tested how:** step 1 and 2 in game by the owner. Step 3 **not tested**.
**Next:** owner tests step 3 and sends the log.

---

## 2026-10-08 C API for the rig + Fallout 4 probe plugin

**Changed:** (1) `gears_ffi`: `gears_anim_new/update/free` (your marcus.glb → the
standard rig pose each frame, with reload / pump / aim / vault state) and
`gears_rig_bone_count/name/parent/rest`; `libgears.h` matches; `examples/rig_host.c`.
`WeaponAnim::names` shared by the session and the C API. (2) `ports/fallout4`: an F4SE
plugin (CommonLibF4, libxse fork, runtime 1.11.240) that loads gears.dll at runtime,
loads your marcus.glb and logs the player's skeleton after a load. Changes nothing.
**Why:** Fallout 4 is the first game port; we need its bone names and a working build.
**Tested how:** rig_host.c built against libgears.a here and run on the export (same pose
as the Bevy session); C API unit test; fmt / clippy / tests clean. The plugin is **not
tested** (needs MSVC + xmake on the PC; API names checked against the CommonLibF4 headers).
**Next:** build it on the PC, read the skeleton log, map our rig onto it, then drive bones.

---

## 2026-10-08 The libgears standard rig (shareable) + runtime retarget

**Changed:** `crates/gears_play/src/rig.rs`: our own 24-bone humanoid (pelvis, spine,
chest, neck, head, shoulders, arms, hands, legs, feet, toes + end markers; our names
and proportions, T-pose, 165 uu to the top of the head over a 156 uu pawn) and a
`Retargeter` that drives it from your loaded Marcus each frame: core bones copy his
turn from bind (with a per-bone alignment from his A-pose to our T-pose, computed at
load from his bind, nothing stored), arms and legs by two-bone IK to his hand / foot
targets scaled to our limb lengths, elbows / knees toward his, twist kept. Gun grip and
barrel from his weapon socket. `Frame.rig` carries it (mirrored with cover-left).
Bevy app: rig view (M) now draws our rig as capsules + a gun bar instead of Marcus's
skeleton.
**Why:** the user wants to share the rig; Marcus's skeleton can't be shared.
**Tested how:** `tests/rig.rs` (no stretching over a whole run clip, feet within 15°
of his, A-pose hands hang, two-bone IK lengths), session test (rig present, head up);
plotted idle / aim / run / cover poses from the session (looked right: gun held,
crouched cover, hunched run like his).
**Result:** cargo fmt / clippy clean, all tests pass.
**Still broken / not tested:** the Bevy capsule drawing is not tested (Bevy can't build
here) — needs a run on the PC. No fingers. Not in the C API yet.
**Next:** run on the PC; expose the rig pose through libgears.h.

---

## 2026-10-07 Desktop range: gears_play crate + Bevy app + one-step setup

**Changed:** (1) `crates/gears_play` (no dependencies): glb reader with its own JSON
parser, the animation driver ported from the browser range (clip choice per state, step
clocks, weighted blend, ADD layers, mirroring, aim offsets, vault root curve, gun anims),
the Gears camera, the three ranges + push-out + shot traces, a `Game` session (guns on
`gears_core::weapons` incl. active reload, Gnasher pump schedule, targets, tracers, sound
and chatter cues). (2) `game/`: Bevy 0.15.3 app (own workspace) that steps `Game` at 60 Hz,
drives the glTF joints, attaches the guns to the RightHand socket, plays the cues, HUD with
the active reload bar. (3) `tools/setup.py`: every extraction step from the dump in one go.
(4) `gears_core`: `MoveInfo` / `move_info()`, `is_at_cover_edge()`.
**Why:** owner: make it like ELDEN-RING-Combat-Rewrite (native Rust app, data from your own
files) — "start on the bevy app and everything else needed".
**Tested how:** 58 cargo tests (+7: JSON, glb on the real export, Rust vs browser pose parity,
session: slide + sounds + cover camera, Lancer 51 per hit + loop/stop, Gnasher 400 hp left
after a 600 close shot + pump sounds, poses / gun anims / footsteps with the export). Pose
parity: Spine_03, Weapon_R_End, Hand_L, Head equal to 4 decimals idle and aiming. setup.py
run end to end here (UE Viewer + ffmpeg) rebuilt all five pack files; tests pass on them.
**Still broken / not tested:** the Bevy app is not compiled (crates.io unreachable here):
syntax checked with rustfmt and the uncertain Bevy 0.15 calls checked on docs.rs only.
Expect a first round of compile fixes. Windows paths for setup.py not tried.
**Next:** owner builds `game/`, sends compile errors or a screenshot.
**Update 2026-10-08:** owner built and ran it on Windows (RTX 3080 Ti, Vulkan): compiles
first try, 61 sounds loaded; JPEG textures failed (Bevy's `jpeg` feature was off) → added.

---

## 2026-10-07 Gun anims, Gnasher pump, aim offsets, roadie lean, movement sounds, chatter

**Changed:** gun anims packed into lancer/gnasher.glb (`AR_Fire` per shot, `AR_Reload`,
`AR_Reload_Fumble` on jam; `SG_Chamber`, `SG_Reload*`); Gnasher pump on Gears' schedule
(0.25 × interval firing, 0.75 × interval cocking with Marcus's chamber layer, cock sound
+0.3 s, shell bounce +0.7 s); aim up/down from Gears' AimOffset data (new
`tools/gears_aimoffset.py` → `marcus_aim.json`, `ue3_props.py --object`); roadie steering
lean (`AnimSetMarcus_Overlay`); sounds for slide, cover in/out, roll, swat, vault + land,
Marcus's footsteps on the step clock; Marcus chatter on kills (two-in-a-row "That's a
twofer"), close Gnasher kills and swaps. 61 sounds now.
**Why:** owner: "voice lines, weapon firing animations, and the other stuff you caught".
**Tested how:** headless with the owner's export: AR_Fire playing during fire, SG_Chamber
runs to its end, sound log shows slide / cover / roll / fire / pump / shell / impact,
7 footsteps over 2.5 s of walking; aim offset barrel +20° / −29° at ±34°; no errors.
51 cargo tests unchanged.
**Still broken / not tested:** sounds not heard by me; chatter chance/cooldown and the
aim mapping are assumptions; no muzzle-flash particles (only a light), no pawn recoil,
no lean-out / blind fire, no efforts (grunts); gun textures low-res.
**Next:** owner listens and plays; lean-out and blind fire; efforts.

---

## 2026-10-07 Textures, sounds, active reload, strafe + cover aim anims

**Changed:** (1) Textures: UE Viewer patched to export Texture2D headless; Marcus (2048²
body, hair with alpha), Lancer and Gnasher (64², big mips not reachable) embedded in the
glbs (`gears_anim_pack.py --tex`); the page keeps them. (2) Sounds: 21 weapon / reload /
active reload / impact waves exported (`umodel -sounds`), XMA → MP3,
`tools/gears_sound_pack.py` → `gears_sounds.json`, loaded with the glbs; Lancer fire
loop + tail, Gnasher shot + pump at 0.3 s, reload, AR results, empty click, swap, hits.
(3) Active reload (Rust `weapons.rs` + C API + page): Gears' windows, tiers, speed-ups,
Gnasher success anim, 4 s jam, damage bonus; HUD bar with the zones. (4) Anims: jam /
success reload layers, strafing (4 aimed walk cycles blended by direction), rising to
aim over mid cover; cover-aim camera uses CoverTargeting's posture + pop-up.
**Why:** owner: "get the animations and reloading, textures, and sound all working".
**Tested how:** 51 cargo tests (active reload windows/timing, bonus + tier); fmt +
clippy clean. Headless page with the owner's export: 21 sounds decode, a perfect
reload at 0.783 s → tier 1, bonus, done at 1.47 s (same as Rust), textured Marcus,
strafe and cover aim screenshots, no errors.
**Still broken / not tested:** sound timing inside reloads approximate; the fail
sound wave is a guess (`ActiveReloadChunk`); gun textures low-res; no lean-out / blind
fire / standing-cover aim anims, no aim up/down (AimOffset); sounds not heard (headless).
**Next:** owner listens; then gun textures from Textures.tfc (needs a local export on
the PC), aim offsets, porting.

---

## 2026-10-07 C API (gears.dll) + weapon rules in Rust

**Changed:** (1) `gears_core::weapons`: Lancer / Gnasher rules in Rust (fire interval,
semi / full auto, magazines + reload from spare, range bands, Gnasher 75 % / × 1.2
rules, pellet pattern and spread scale, aim error, Accuracy easing). (2) New crate
`crates/gears_ffi` (lib name `gears`: cdylib + staticlib) with `include/libgears.h`:
character new / tick (host callbacks or built-in walls) / state / event log, weapons.
(3) `examples/host.c`. (4) `docs/PORTING.md` (build, per-frame use, units, WaW routes).
`Character::time_since_evade`. `.gitignore` whitelists `include/*.h`, `examples/*.c`.
**Why:** owner: wants to port the movement and damage to WaW zombies "or any game".
**Tested how:** 49 cargo tests (5 weapon rules, 2 C API); fmt + clippy clean. Built the
release static lib and compiled / ran `examples/host.c` with cc on Linux: slide at
0.067 s, cover 0.450, vault 1.067 → 2.533, lands x = 440.2 (= the Rust test);
Gnasher 600 close, 137 for 5 pellets at 1200 uu.
**Still broken / not tested:** Windows / 32-bit build not tried (rustup can't reach
its servers from here). The test range still runs its own JS copy of the weapon
rules (same numbers, not parity-checked). WaW: nothing ported yet; GSC route untested.
**Next:** WaW GSC damage port, then movement experiments in game.

---

## 2026-10-07 Shooting: Lancer + Gnasher, aiming

**Changed:** (1) Core (Rust + JS): `Input.aim`; aiming walks at 150 uu/s facing the camera
(strafe) and blocks the roadie run. (2) Test range: aim (LT / right mouse) with Gears'
targeting camera, fire (RT / left mouse), swap (Y / Q), reload (RB / E). Lancer
(auto, 550 rpm, 51 dmg, range bands) and Gnasher (10-pellet pattern, spread rules,
75 % rule, ×1.2 close, range bands), magazines and reloads with the real reload
layers, tracers, muzzle light, damage numbers, 1000-health targets that fall and
come back. Gnasher mesh exported (`COG_Gnasher_Skel` → `_local/.../gnasher.glb`).
Packer: 4 more clips (aim / reload layers). `tools/ue3_props.py`: `UE3_ARRAY_BYTES`.
**Why:** owner: "maybe shooting and adding the gnasher".
**Tested how:** 42 cargo tests (new: aim walk speed + facing, no roadie while aiming);
fmt + clippy clean; parity 199/200 event logs. Headless page with the owner's export:
Lancer 51 per hit at range, Gnasher 600 up close, reload 7 → 8 from spare, swap,
side view of the Gnasher hold, no errors.
**Still broken / not tested:** no active reload yet; aim error units, recoil and
Accuracy are assumptions; no strafe / pop-up / fire anims (walk-forward cycle while
strafing, stands up to aim from cover); not checked in Xenia.
**Next:** sounds; active reload.

---

## 2026-10-07 Vault over mid cover; roadie run phase sync

**Changed:** (1) Vault (`SM_MidLvlJumpOver`, Rust + JS): A in mid cover with the stick
toward the wall jumps over it, Gears' rules from `GSM_MantleOverCoverBase` (stick, slot
distance, order after the slip, chains, landing check, roadie run out of the landing
0.1 / 1.1 s after touching down with A held). Jump follows `AR_Mantle_Start`'s root
track (up 100.8 first, then 141.4 × MantleDistance / 141 over), physics fall with
Gears' fall velocity and −750 gravity, `AR_Mantle_End` landing. New host query
`World::mantle_distance` (Gears' is native; ours: far face + radius). (2) Packer: 6
mantle clips (41 total) and the jump's root track in the glb's animation extras.
(3) Page: vault anims (mirrored set when facing left), height and camera follow the
jump, wall push-out off while vaulting, a `TEST.speed` hook for slow-motion checks.
(4) Roadie jank: legs and the rifle layer (and walk/run blend) now share one phase
clock instead of drifting apart; removed my homemade roadie camera bob.
**Why:** owner: "maybe add vaulting over walls next"; "the roadie run still looks a
little janky".
**Tested how:** 40 cargo tests (new: vault timing and landing spot, up before over,
stick needed, no vault on standing cover, blocked landing, A held → roadie at landing
+ 0.091 s, vault queued from a slide, refused > 96 uu from the slot). fmt + clippy
clean. Parity 200 seeds: event logs identical in 199 (seed 72, known drift); 10 of
those differ only by 0.1 uu in the final position after 60 s. 50 vaults in the fuzz.
Headless page with the owner's export: both vault sides at 0.06× speed (plant, hop,
over, land; no pop at the jump → fall swap), samples 101 uu peak, lands 174 uu out;
previous scripts (real3d, new_moves, swat3d) no errors.
**Still broken / not tested:** not checked against Gears in Xenia. The far-side
distance is my approximation. Roadie phase sync not judged by eye yet (owner). Mantle
kick, mantle up, mantle down not built. Vault ignores ledges (flat ground only).
**Next:** shooting + Gnasher, then sounds.

---

## 2026-10-07 Gun hold, roadie loop, moving in cover, two-sided cover

**Changed:** (1) gun hold: the ADD weapon layers were applied in the wrong order
(three.js base × delta). An offline FK check showed Gears' order is delta × base: hands
then land exactly on the anims' IK targets. The page now applies the layers itself
(restoring each bone's base when three.js's mixer skips an unchanged bone, which
otherwise piled the layers up frame after frame), the Lancer goes on Gears' RightHand
socket with its barrel on bone +X, and my arm IK is gone. (2) Roadie / run / walk
"skip": looping clips now repeat frame 0 one frame after the last (packer). (3) Moving
in cover (Rust + JS): stick sideways → face that way and move along the wall at
300 uu/s (CoverMovementPct 1.0), stop at the ends; cover run cycle plays while moving.
(4) `WallWorld::two_sided`: every wall a 24 uu slab with cover on both faces; the 3D
range and the fuzz world use it; both faces painted.
**Why:** owner: "the way he holds his gun is strange", "a little skip in the run
cycle", "i cant move while in cover", "make both sides of the wall places where i can
go for cover".
**Tested how:** 32 cargo tests (new: moving along cover + stopping at both ends +
facing; back-face cover). Parity: scripted identical, 198/200 fuzz move logs (seeds 11,
72: late f32 drift). Page headless with the owner's export: hands on IK targets after
3 s (stable), barrel −28° / −8° at idle, cover move right 250 uu then left, back-face
slide to x = −42, no errors.
**Still not tested:** a human with a controller.

---

## 2026-10-07 Real Marcus animations (L4 start)

**Changed:** exported Marcus's mesh, both body AnimSets and the Lancer from the owner's
own GearGame.xxx with UE Viewer (built headless here), all into `_local/`. New
`tools/gears_anim_pack.py` merges psa clips onto the glTF skeleton → `marcus.glb`
(35 clips, 2.7 MB) + `lancer.glb` + `marcus_anims.json`. The 3D page gained "Load your
Marcus export" (file picker / drag-drop; read locally, never uploaded): clips are
driven by the movement state at Gears' play rates, additive `ADD_AR_*` weapon layers,
mirrored cover, gun on `b_MF_IK_Gun` with our two-bone arm IK. Core (Rust + JS): evade,
slip and swat now use the anims' real root motion and lengths (evade 451–476 uu over
≈0.99 s instead of 300 uu / 0.78 s; slip 117 uu sideways + 75 forward; swat = 510 uu/s
dash then the 256 uu end anim at 1.3). Evades go straight sideways (the 16834 was only
the trace). Fixed: reach label showed NaN while moving (no camera yaw passed).
**Why:** owner chose "real animations".
**Tested how:** 30 cargo tests (new: evade root motion, short and long swat timing);
parity 200/200 fuzz move logs identical. Page headless with the owner's export:
loads (35 clips + Lancer), no errors; idle / run / roadie / slide / cover both sides /
swat / push out / evade screenshots look right; measured right hand on the grip
(0 uu) and left hand 3.7 uu from the Lancer's front handle; Lancer barrel 18° off
forward at idle.
**Still not tested / known gaps:** no textures (flat colours; textures are in the
200 MB CharTextures.tfc); arm IK is ours, not Gears'; no blend between mirrored and
unmirrored poses (it flips); root rotation inside anims vs pawn yaw can pop at the
end of a slip. Not played by a human yet.

---

## 2026-10-07 Swat turn

**Changed (Rust + JS):** new move `SwatTurn` (`SM_StdLvlSwatTurn`): in cover at the
end you're facing, stick toward the gap + A → dash at 510 uu/s to the cover across
it (≤ 512 uu), land in cover there facing back toward the gap. Needs the camera over
the cover (dot ≥ 0.85). Tap A again during it and hold → after 0.2 s, if still > 68 uu
out, it turns into a roadie run. The slide and cover run can queue a swat. New
`World::find_swat_target` (default: none; the test world finds the next wall in
line). Cover-run edge check now has 0.01 uu slack so Rust and JS agree. Fuzz world
gained a wall past a 60 uu gap. Both pages: swat pose, controls, speed readout.
**Why:** next link in the wall-bounce chain; owner: "can we keep goin".
**Tested how:** 29 cargo tests (6 new: dash speed and landing, camera check, 512 uu
reach, stick direction, swat → roadie timing, swat queued from a slide); fmt and
clippy clean. Parity: scripted identical; 199/200 fuzz move logs identical (seed 106:
f32 drift 38 s in), 15 swats exercised. 3D page headless (Short walls): slide in,
D + Space → SwatTurn → InCover at the next wall, facing back; no errors.
**Guesses (marked in code):** 96 uu "at the edge"; landing facing the gap; constant
510 uu/s instead of the end anim's root motion.
**Still not tested:** played by a human.

---

## 2026-10-07 Push out no longer holds you

**Changed (Rust + JS):** `PushOut` is now just a 225 uu/s shove off the wall: walking
runs underneath it (the stick steers at once) and any move (slide, evade, roadie)
cuts it short instead of waiting 0.62 s. Evades out of it face the camera like any
other evade.
**Why:** owner: "it does this weird pushout thing where it walks away from the cover";
asked what Gears does when you pull away from the wall after the slide: "Off the
wall, full control". The code backs it: CanTryToRunToCover, CanEvade, GSM_Evade and
UpdatePlayerPosture all exempt SM_PushOutOfCover from their "busy" checks, like the
Move2Idle transitions.
**Tested how:** 23 cargo tests (push out: 225 shove then the stick steers it; a slide
asked for during it starts within an A tap). Parity: scripted identical; 198/200
fuzz move logs identical (seed 18 cover-run edge, seed 106 reach threshold 38 s in,
both f32 drift), 184 push outs exercised. 3D page headless: slide in, S then D →
push out steers right; no errors.
**Still not tested:** played by a human.

---

## 2026-10-07 Slide cancel is instant (the wall bounce)

**Changed (Rust + JS):** pulling the stick away from the wall during a slide now
cancels it on the first frame (was: after a 0.25 s hold, my wrong inference from the
last change). "Away" is relative to the cover, not the camera, so sideways slides
cancel too. A cancelled slide carries on at 300 uu/s along the slide instead of
keeping the old walking velocity. Left / right still take 0.25 s to pick the side.
**Why:** owner: "anytime you press a opposite direction in gears the cover animation
gets cancelled immediately and thats how you can wall bounce and right now its not
instant". Re-reading `JoystickHeldLongEnough`: `HoldingBack > 0` vs `HoldingLeft >=
MinHold` — the asymmetry fits his report. Owner also sent a Gears 3 wallbounce
video (youtube rBKEekR3Pqc); I can't watch video and the page refused my request
(429), so it isn't used yet.
**Tested how:** 23 cargo tests (replaced the "short pull back doesn't abort" test
with: one frame of back cancels within 2 frames; cancelled slide hands off at 300;
sideways slide cancels by pulling away from the wall). Parity: scripted identical;
198/200 fuzz move logs identical (seed 18: cover-run edge drift; seed 86: f32
position drift 22 s in crossing the roadie→cover reach), 212 cancels exercised. 3D
page headless: W + tap Space, then S → "slide aborted" 0.09 s into the slide.
**Still not tested:** played by a human. The 300 uu/s hand-off is an assumption.
**Next:** owner plays; compare against the video frame by frame if he can describe
or clip what happens right after the cancel.

---

## 2026-10-07 Wall cancels: push out of cover, real slide-abort rule

**Changed (Rust core + JS port):** new move `PushOut` (`SM_PushOutOfCover`): in cover,
pull the stick away from the wall for 0.05 s → step straight back at 225 uu/s for
0.62 s; slides, evades and roadie runs asked for during it are queued until it ends.
Slide stick watching now decides once, when a direction has been held 0.25 s (back =
abort, left/right = side), instead of aborting on the first frame of pulling back.
Evading out of cover needs the stick along the wall; the evade picks its direction
from the cover-relative stick. Cover slip anim length is now the real 0.7 s (was a
0.6 s placeholder). Both test pages show the new move.
**Why:** owner: "the wall canceling feels weird". Before this you couldn't leave cover
by pulling back at all, and any pull back during a slide cancelled it instantly.
**How I found it:** `GearPC.PlayerTakingCover.UpdatePlayerPosture`,
`GSM_PushOutOfCover`, `GSM_Run2Cover.BeginSpecialMove/JoystickHeldLongEnough/
SpecialMoveEnded`, `GearPC.CanTryToRunToCover`, `CanEvade`,
`PlayerTakingCover.TryToEvade`, `GearPlayerInput.HandleActionButton`; anim lengths
from GEARGAMETDM_SF.xxx (staged from the owner's dump, unpacked in `_local/`).
**Tested how:** 21 cargo tests (6 new: push out timing and speed, queued slide during
push out, short pull back keeps the slide, left hold faces left, cover evade needs
the stick along the wall); fmt/clippy clean. Parity: scripted identical; 200 fuzz
seeds: 199 move logs identical (seed 3: one frame at a cover-run edge 51 s in, f32
drift), 247 push outs exercised. 3D page in headless Chromium: slide in, hold S →
PushOut 225 uu/s → Walk after 0.63 s.
**Still broken / not tested:** the 0.25 s slide-watch threshold is inferred (native
code). Not played by a human yet. Swat turn still missing (it's the other big wall
cancel).
**Next:** owner plays; swat turn + swat → roadie.

---

## 2026-10-07 3D third-person test range with Gears 3's camera

**Changed:** `web/range3d.html` (new main page: Three.js r128 from cdnjs, over-the-shoulder
camera, camera-relative stick, posed stand-in soldier, 3D walls, map, both sticks in
the HUD); `tools/build_web.py` builds both pages; published to the same artifact.
Camera numbers read from my dump (`GearCamera.ini`, `GameplayCam_*` defaults,
`GearPlayerInput`, `PlayerInput.PlayerInput`, `GearPC.GetSensitivityMultiplier`) and
listed in GEARS_VALUES.md → Camera, plus the pawn cylinder (radius 34 is now cited, so
`collision_radius` is no longer a placeholder). Core fix in Rust and JS: the test
world's cover search returns the wall you're pressed against (closer than the 34 uu
standoff) and never finds cover through another wall. New test
`cover_is_never_found_through_another_wall` (15 tests). A presses are latched in the
page so a tap shorter than one movement step still counts.
**Why:** owner: "hard to compare a 2d top down plane to a 3d game".
**Tested how:** cargo fmt/clippy/test clean. Parity: scripted scenarios identical;
200 fuzz seeds: 199 move logs identical (seed 88 differs by one frame at a cover edge,
f32 drift), 3 more differ only in the final position's last decimal. Page: headless
Chromium with software WebGL (runs at ~0.3–0.5× speed there): tap A slid into cover
(FOV went 80 → 95), hold A roadie-ran into cover, D + tap A evaded right; no layout
overflow at 1400 or 400 wide; no script errors.
**Bugs found:** pressed against a wall, tap A slid *through* it into cover on the next
wall (found in the 3D view). Fixed + regression test.
**Still broken / not tested:** not played by a human; controller and mouse look
untested (no devices here). Camera pivot height (pawn centre + eye height), mode
blending (0.15 s), pitch blend rule and ±60° pitch limit are my approximations. No
roadie camera shake from Gears (a small cosmetic bob instead). Poses are hand-made
stand-ins.
**Next:** owner plays it; then swat turn + swat→roadie cancel.

---

## 2026-10-07 Playable browser test range + Rust/JS parity check

**Changed:** `web/gears_core.js` (line-for-line JS port of gears_core),
`web/sandbox.html` + `tools/build_web.py` (single-file test range, published as an
artifact), `tools/parity.js` + `gears_cli fuzz <seed>` (same inputs through both).
Core fixes in Rust and JS: queued CoverSlip can't start without cover; AbortRun2Cover
drops the queued move; camera-right vector built exactly (no cos(90°) rounding).
New test `aborting_a_slide_drops_a_queued_slip` (14 tests).
**Why:** owner wants to test it himself. My workspace can't download crates
(crates.io blocked; WebAssembly target not installable), so a native Rust window
couldn't be built or checked here; a browser page runs anywhere with a controller.
**Tested how:** parity: 3 scripted scenarios identical; 200 fuzz seeds × 3,600 frames:
**199/200 move logs identical (29,080 state changes)**; the one difference appears
8 s into a run (f32 vs f64 position drift crossing a reach threshold). Page: headless
Chromium at 1400×860 and 400×860 — W + tap Space slid into cover, no layout overflow,
no script errors (fonts can't load in my workspace; fine on the real page).
**Bugs found:** the fuzz found a crash in both versions (slip queued during a slide,
then slide aborted → slip started without cover). Fixed + regression test.
**Still broken / not tested:**
- Not played by a human yet. Gamepad path untested (no controller here).
- Walls block walking only via a host-side push-out in the page; the core has no
  collision sweep yet.
- The browser runs the JS port, not the Rust crate itself. Parity check keeps them
  together; rerun `node tools/parity.js` + `gears_cli fuzz` after every core change.
**Next:** owner plays it and reports how it feels; then swat turn.

## 2026-10-07 L1 started: gears_core state machine, 13 tests passing

**Changed:** new Rust workspace (`Cargo.toml`, `crates/gears_core`, `crates/gears_cli`);
RE_NOTES sections 6–9 (cover slip, swat turn, evade, A-button press/release rules,
roadie auto-cover, special-move override/queue rules); GEARS_VALUES additions;
`.gitignore` allows Rust sources, ignores `target/`.
**Why:** L1: Gears 3 movement rules running in Rust before any visuals.
**Tested how:** `cargo test` (13 tests), `cargo clippy --all-targets` (clean),
`cargo run -p gears_cli -- all` (scripted scenarios).
**Result:**
- Tap A toward a wall 252 uu away → slide 0.383 s (= 252 / 690 + frame), lands at the
  cover spot. Short slides clamp to 0.2 s.
- A + stick up during a slide near the edge → slip queued, starts the frame the slide
  lands; A still held → roadie queued when the slip ends.
- Hold A → roadie at press + 0.3 s, 480 uu/s; releasing A ends it; roadie run into a
  wall snaps into cover.
- Out-of-range cover (> 420 uu) or stick pulled back → no slide; the tap evades instead.
- Evade-left keeps Gears' shipped 16834 rotation (≈2.5° off square).
**Bugs found and fixed while testing:**
- Moves advanced on the frame they started → slides one frame short. Moves now start
  moving on the next tick.
- Evade rate limit: I'd used `MinTimeBetweenEvades` (2.0 s). The player's script never
  checks it; `NotifyEvadeFinished` starts a 0.65 s `CannotRetriggerEvade` timer.
- Evade kept the pre-evade walking velocity; now root motion only.
**Still broken / not tested:**
- Slip / evade / swat distances are PLACEHOLDERS (they're animation root motion).
- Not modelled yet: swat turn, mantle up/over/kick, cover-remapped stick details,
  heavy weapons, AI. Roadie steering uses a placeholder turn rate.
- Collision radius placeholder (34 uu); test world is straight walls only.
- No visual sandbox yet; nothing compared with the real game.
**Next:** swat turn + the swat→roadie cancel; then the Bevy sandbox so it can be seen
and played; then animation export (L4) to replace the placeholders.

## 2026-10-07 Pivot to libgears + bytecode decoder working

**Changed:** project re-planned as **libgears** (Rust library of Gears 3 movement;
sandbox first; Gears 3 animations exported locally; WaW mod parked). New tools
`tools/ue3_bytecode.py` (UnrealScript bytecode decoder) and `tools/ue3_props.py`
(enums + class defaults). DESIGN.md, AGENTS.md, README.md rewritten; RE_NOTES.md and
GEARS_VALUES.md gained the movement chain.
**Why:** owner wants Gears 3's movement — especially wallbouncing — usable in any game.
**Tested how:** decoder self-test over every function in GearGame.xxx; spot-read
GSM_Run2Cover, GearPC cover/roadie code; property reader on 7 classes.
**Result:** 17,419/17,419 script functions decode with exact storage *and* memory
sizes (first try); 257 native-only. Cast-table bug found by reading output
(`bool(-Normal)` should be `rotator`) and fixed from UELib's CastToken enum. Movement
chain documented: CanRunToCover rules, slide timing formula, chainable moves, the A
button priority in cover, roadie→cover seamless rule, ~25 new cited values.
**Still broken / not tested:**
- Pseudo-code is flat (labels/gotos), no if/else rebuilding. Readable enough.
- Some struct defaults (`AcquiredCoverInfo`, `SWATTurnTargetCovInfo`) fail to parse
  (52-byte struct read past its data); the reader stays aligned and skips them.
- Wallbounce explanation is a working theory until compared with Gears 3 in Xenia.
- Cover search is native C++ (expected; libgears replaces it).
**Next:** read CoverSlip, SwatTurn, Evade, TransitionFromSwatToRoadieRun; then scaffold
the Rust workspace (gears_core + sandbox) and implement walk → roadie → slide.

## 2026-10-07 M0 second run: our script never ran → ship as mod_ex.ff

**Changed:** copied the built `mod.ff` as `mod_ex.ff` into both
`mods\locust_zm\` folders (install + AppData). Identical file (MD5 7326a66c…).
**Why:** Second run (no developer mode, `/map nazi_zombie_prototype`) loaded fine and
reached round 1, but no `[LZ]` lines and `lz_test hello` did nothing. The fastfile
does contain both scripts (checked by decompressing it), so the likely cause is load
order: the stock map's own copy of `maps/nazi_zombie_prototype.gsc` wins over
`mod.ff`. T4M-Enhanced (installed, r49): "An extra fastfile named `mod_ex` can load
into a mod if the file exists and has priority over all other fastfiles" (r42+).
The zone name isn't stored inside the fastfile, so renaming should be safe.
**Tested how:** not yet in game.
**Still broken / not tested:** whether mod_ex fixes it; why no `console.log` was
written with `/logfile 1`.
**Build workflow until we can build mod_ex directly:** Launcher Mod Builder →
then copy `mod.ff` → `mod_ex.ff` in both mod folders.

## 2026-10-07 M0 first run: stock developer-mode crash, logging changed

**Changed:** `lz_raw` in `src/maps/lz_main.gsc` now prints with `iprintln` (game
message → screen + console) unless `lz_onscreen` is "0", and `println` only in
developer mode. Rebuilt in the Launcher: Success, AppData `mod.ff` 4,128 bytes.
**Why:** Owner ran `/developer 1 /developer_script 1 /logfile 1 /devmap
nazi_zombie_prototype` → "script runtime error: undefined is not a field object".
**Tested how:** Owner's console screenshot.
**Result:** all errors in `maps/_debug.gsc` lines 3143–3149 (`debug_show_viewpos`,
`hud_title.alpha = 0` on an undefined HUD element), repeating every 0.5 s. That's the
stock dev-only debug code (line numbers don't match the RoR2-edited raw copy), so it's
not ours. Console banner revealed **T4Me r49 is installed** (the `d3d9.dll`).
No `console.log` was written for that run.
**Still broken / not tested:** Our `init` still not seen running. Whether `iprintln`
lines reach `console.log`.
**Next:** relaunch without developer mode, `/set lz_test hello`.

## 2026-10-07 M0: mod.ff built with the Mod Tools

**Changed:** nothing in source. Built `locust_zm` in the Launcher's Mod Builder
(only "Build mod.ff Fastfile" ticked), driven by Claude through computer use.
**Tested how:** Launcher log + file checks afterwards.
**Result:** `linker_pc ... -moddir locust_zm mod` → `Fastfile 1 of 1, "mod": [ver. 387]
process...link...compress...save...done.` Status **Success**. `mod.ff` (3,968 bytes) in
`mods\locust_zm\` and copied to AppData `mods\locust_zm\`. `raw\maps\nazi_zombie_prototype.gsc`
unchanged (9,745 bytes, 2009 date): the Mod Builder does not copy into raw (earlier
assumption was wrong). Side effects: `zone_source\mod.csv` overwritten (scratch file),
`AppData\...\mods\nazi_zombie_tutorial\mod.arena` created by the Launcher.
**Still broken / not tested:** In-game run: does `lz_main::init` run, does `println`
reach the console.
**Next:** launch the mod, `/devmap nazi_zombie_prototype`, `/set lz_test hello`.

## 2026-10-07 M0: WaW recon, backup, empty mod built

**Changed:** `src/maps/lz_main.gsc` (entry point, logging, `lz_test` command
watcher, `hello` test), `patches/nazi_zombie_prototype.patch` (one-line hook after
`maps\_zombiemode_prototype::main();`), `tools/build.py`, AGENTS.md environment.
**Why:** M0: prove a mod can run our script inside Nacht der Untoten.
**Tested how:** Build only (in the cloud workspace): `build.py` checked the stock
Nacht script's MD5 (`2524885e...`), applied the patch, wrote `mod.csv` + 2 scripts;
brace/paren balance checked. **Not compiled with the Mod Tools and not run in game.**
**Result / findings:**
- WaW Steam, `version.inf` 1.6; Mod Tools 1.4 + Sniperbolt + UGX Script Placer present.
- Third-party `d3d9.dll` wrapper + DXVK installed → T4M (also `d3d9.dll`) would clash.
- `raw/maps` already modified by the owner's RoR2 mod (`_zombiemode.gsc`, `_debug.gsc`)
  and UGX files. Nacht's own scripts (`nazi_zombie_prototype.gsc`,
  `_zombiemode_prototype.gsc`, `_zombiemode_spawner_prototype.gsc`) are stock-dated.
- Nacht hook points: `level.global_damage_func` (zombie damage),
  `level.overridePlayerDamage`, `onPlayerSpawned` — all in `_zombiemode_prototype.gsc`.
- Backup made: `_local/backups/codwaw_backup_2026-10-07.zip` (17 files, MD5 manifest).
**Still broken / not tested:** Everything in game. Whether `println` reaches
`console.log` with these launch flags.
**Next:** owner approves writing `mods\locust_zm` into the install folder → build in
Launcher → run `lz_test hello`.

## 2026-10-07 Gears 3 opened up: code map + real numbers

**Changed:** new tools `tools/inspect_packages.py`, `tools/ue3_tables.py`,
`tools/unpack_coalesced.py`; filled `docs/GEARS_VALUES.md` and `docs/RE_NOTES.md`.
**Why:** M0 Gears side: find where the mechanics live and get real values.
**Tested how:** Ran all three tools on my Gears 3 dump (core packages + config only).
**Result:**
- Core, Engine, GameFramework, GearGame, GEARGAMEHORDE_SF: **MD5 match** the disc's
  `Xbox360TOC.txt`. Version 828/0, big-endian, LZX. **Not encrypted.**
- UE Viewer's `decompress` (built from source) unpacked all four script packages to full
  size. `GearGame.xxx`: 54,434 names, 4,776 imports, 123,176 exports, 2,370 classes,
  17,676 functions, 461 states — table reader read every row.
- `Coalesced_INT.bin`: plain (not encrypted), 95 files, 100% of bytes parsed.
- Found every target: active reload (`GearWeapon`), roadie run (`GSM_RoadieRun`),
  chainsaw (`GSM_ChainsawHold/Attack`, `GDT_Chainsaw`), cover (`GearPawn`), Horde
  (`GearGameHorde_Base`), Wretch/Boomer/Drone pawns.
- Real numbers, e.g. Lancer: 2.5 s reload, success zone from 0.75 s, 0.30 s sweet spot,
  1.3× damage for 8 s; roadie run 1.6×; chainsaw rev 0.75 s; Wretch 350 hp at 585 uu/s.
**Still broken / not tested:**
- No bytecode decompiler yet (UELib needs .NET; not reachable from the cloud workspace).
  Open questions that need function bodies: the 3 perfect-reload windows, X/Y in the
  multipliers, jam time, chainsaw-able enemy list.
- Unit conversion Gears uu → WaW units not measured.
- The `_SF` content packages beyond Horde are not inspected yet.
**Next:** decompile `GearWeapon` active reload functions (UE Explorer on my PC, or a
small bytecode reader), then start the WaW side (needs the WaW install folder).

## 2026-10-07 Gears 3 becomes the first RE target

**Changed:** DESIGN.md (M0 Gears 3 steps, Horde 2.0), AGENTS.md (dump location),
RE_NOTES.md (package table order, Horde section).
**Why:** I'm ripping my Gears 3 disc first. Gears 3 has every mechanic we want plus
Horde 2.0, and runs well in Xenia for side-by-side comparison.
**Tested how:** Not tested. Rip in progress.
**Still broken / not tested:** Which Gears 3 packages open in UELib/umodel. Some are
reported encrypted (umodel "Wrong package tag"); we skip those, not crack them.
**Next:** Extract the rip into `_local/dumps/gears3/`, then list packages.

## 2026-10-06 Switched to a reverse-engineered merge

**Changed:** AGENTS.md (approach, rules 8–9, RE tools), docs/DESIGN.md (route, RE
workflow, M0 Gears recon), new docs/RE_NOTES.md.
**Why:** I want a real merge of both games, not lookalike mechanics. Gears' gameplay
code will be decompiled from my own dumps and reimplemented in WaW.
**Tested how:** Not tested. No tools run yet.
**Result:** n/a
**Still broken / not tested:** Whether UELib opens my Gears 360 packages cleanly
(its own docs say quality isn't verified for some games).
**Next:** M0: add the Gears dump folder and WaW install to the session, open Gears 1's
gameplay package in UELib, map the classes for active reload / roadie run / chainsaw.

### Options considered
- Full Rust rewrite fusion (like the 2010 mashup): not now, months of work and no
  existing WaW rewrite to build on.
- Passthrough with Gears in Xenia: not now, far harder than SkyCraft-style projects.

## 2026-10-06 Project setup

**Changed:** Created AGENTS.md, .gitignore (whitelist), README.md, MODLOG.md,
docs/DESIGN.md, docs/GEARS_VALUES.md.
**Why:** Start of the project. Route chosen: WaW Mod Tools mod that ports Gears content
into stock zombies ("port the content" pattern), not passthrough.
**Tested how:** Not tested. No code yet.
**Result:** n/a
**Still broken / not tested:** Everything. WaW version, Mod Tools and T4M status unknown.
**Next:** M0 recon: confirm WaW install and version, Mod Tools, back up the WaW
AppData folder, build an empty mod that logs `[LZ] locust_zm loaded` on Nacht.

### Decisions so far
- **Not passthrough.** Neither WaW nor Gears has a script extender, and both are
  shooters, so running both at once would mean reverse engineering two engines for no
  gameplay gain. Porting Gears mechanics into WaW gets the same result in-game.
- **Not E-Day.** UE5, released today, online multiplayer, no stable tools.
- **Gears data source:** owner's own Gears 1/2/3 Xbox 360 discs, dumped locally.
