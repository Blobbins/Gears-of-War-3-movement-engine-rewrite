# Design: libgears — Gears of War 3 movement as a library

## The idea in one sentence
A Rust reimplementation of Gears of War 3's third-person character movement and
animation (walk/run, roadie run, slide into cover, cover, evade, mantle — and the
wallbouncing that emerges from them) packaged as a library any game can plug in.

Pattern: universal-modder `mashup-mods` pattern 3, "embed the guest as a library"
(like libsm64 for Super Mario 64: the host feeds collision and input, the library
returns the character's state and pose). Here the guest logic is a clean
reimplementation written from notes on Gears 3's own code, not the code itself.

Decided 2026-10-07: test sandbox first (no host game yet); animate with Gears 3's own
animations exported locally; the WaW zombies mod is parked (see bottom).

## Architecture

```
crates/
  gears_core/      pure movement + animation logic. No engine, no OS. Deterministic:
                   same inputs + same collision answers → same outputs.
                   Units: Unreal units (uu) internally, Gears' own tick rules.
  gears_ffi/       C ABI (cdylib + header) so any host can load it:
                     gears_create(config) -> handle
                     gears_tick(handle, input, dt, collision_callbacks) -> state
                     gears_get_pose(handle, bones_out)          (optional)
                     gears_destroy(handle)
  gears_sandbox/   Bevy app: box/ramp/wall test level, capsule + debug draw,
                   gamepad/keyboard input, on-screen state, recorded runs.
  gears_anim/      loads locally exported Gears 3 animations (from _local only) and
                   evaluates the blend tree the core asks for.
web/               browser test ranges until gears_sandbox exists: range3d.html
                   (third person, Gears 3 camera values) and sandbox.html (top-down),
                   both on gears_core.js, a JS port kept in parity with the crate.
                   The camera lives in the host page, not the core: a host game
                   brings its own camera, so the Gears camera is a reference rig.
tools/             our Python tools (package reader, bytecode reader, unpacker).
_local/            everything from the Gears dump: decompiled script, exported anims.
```

### What the host gives the library each frame
- Input: move stick (2D), look/camera yaw, buttons (A = roadie run / cover / slide,
  B = melee, evade direction, etc.), using Gears 3's default layout as the reference.
- Collision answers through callbacks: sweep a capsule, raycast, "find floor".
  The library never owns the host's world.
- Frame time.

### What the library gives back
- Position, rotation, velocity, grounded flag.
- Movement state: `Walk`, `RoadieRun`, `Run2Cover` (slide), `InCover{type, side}`,
  `CoverMove`, `LeaveCover`, `Evade{dir}`, `Mantle`, `SwatTurn`, `CoverSlip`…
- Animation request: which Gears AnimSequences, weights, play rates, times.
- Camera hints (roadie run shaky cam, cover camera offsets), optional.

### Cover in maps that weren't built for Gears
Gears maps have hand-placed CoverLink/CoverSlot actors. Other games don't. The
library needs a **runtime cover finder**: sweep the host's collision around the
player for walls, classify waist-high (mid) vs full-height (standing), find edges.
Hidden design question: how close we can get to Gears' slot-based feel without
slots. Read Gears' cover acquisition code first (`GearPawn.FindCoverFromLocAndDir`,
`ValidatePotentialCover`, `CoverAcquireFOV=0.7`, `RoadieRunCoverAcquireFOV=0.35`).

## Wallbouncing
Not a single move. It falls out of:
1. `GSM_Run2Cover` — sliding into cover at **2.3×** ground speed (300 uu/s base).
2. Leaving/cancelling cover early (evade, leave cover, swat turn) — exact windows TBD.
3. Generous cover acquisition (FOV dot products above, slide distance 60 uu).
So it must come from the real code paths, then be verified frame-for-frame against
Gears 3 in Xenia. No "wallbounce" feature to fake.

## Milestones (each done only when its check passes)

### L0 — Read the code (current)
- Bytecode reader for UE3 v828 big-endian packages (`tools/ue3_bytecode.py`): decode
  `UFunction`/`UState` bodies of the classes we need into readable pseudo-code in
  `_local/re/gears3/script/` (never committed).
- Classes: `GSM_Run2Cover`, `GSM_RoadieRun`, `GSM_Evade`, leave-cover / swat-turn /
  mantle special moves, `GearSpecialMove` base, `GearPawn` cover + movement functions,
  `GearPC` input → move code (`TryToRoadieRun`, `TransitionFromRoadieRunToCover`…).
- Note which functions are `native` (C++, not in the package). Those need a fallback:
  behaviour notes from frame counting, or (only with the owner's OK) Ghidra on the xex.
- **Check:** RE_NOTES.md explains slide → cover → cancel step by step with numbers.

### L1 — Core movement in the sandbox
- Walk/run, acceleration, roadie run, slide into cover (hand-placed test cover),
  leave cover, evade. Fixed timestep, deterministic.
- **Check:** recorded sandbox runs match the values in GEARS_VALUES.md (speeds,
  timings) within tolerance; unit tests for each state transition.

### L2 — Wallbounce
- Cancel windows from L0, chained slides between test walls.
- **Check:** compare with Gears 3 in Xenia: same course (two walls N uu apart), frame
  counts per bounce and distance covered within a stated tolerance.

### L3 — Runtime cover finder
- Replace hand-placed cover with collision sweeps. **Check:** sandbox levels with
  random boxes; cover found where a Gears player would expect it.

### L4 — Animation
- Export Marcus's skeleton + AnimSets with UE Viewer into `_local/anims/` (Gears 3
  animation export is reported as unreliable; test early).
- Rebuild the relevant AnimTree logic (blend by movement state, cover lean/pop-up)
  from the package data. **Check:** sandbox shows Marcus animating through every state.

### L5 — C API + first host
- Freeze the C interface, write a header and a host example. Host chosen later.

## Oracles
- Sandbox: every state change logs `[GEARS] t=… state A -> B reason`, and recorded
  runs can be replayed headless (`cargo test`) for regression checks.
- Gears 3 in Xenia Canary for side-by-side frame counts (owner records).
- Numbers only from GEARS_VALUES.md (cited) — no guessing.

## Rules
- Offline/single-player hosts only. Wallbounce movement in an online game is cheating.
- No Gears files, decompiled script or exported animations in the repo. Publish code
  and converters; users export from their own copies.

## Parked: Locust der Untoten (WaW zombies mod)
State when parked (2026-10-07): mod builds in the Mod Tools Launcher; first run hit a
stock developer-mode bug in `_debug.gsc`; second run loaded but our script didn't run,
most likely because a stock map's own scripts win over `mod.ff`. Fix deployed but
**not tested**: same fastfile copied as `mod_ex.ff` (T4M loads it first). Files stay
in `src/`, `patches/`, `tools/build.py`; details in MODLOG.md. WaW can become a host
for libgears later.

## Layers (2026-10-07)
- `crates/gears_core` — movement + weapon rules (deterministic, no deps).
- `crates/gears_play` — engine-free game layer: camera, animation driver, combat, levels,
  sound cues, glb/JSON readers (no deps). Hosts draw its `Frame` and play its `Cue`s.
- `crates/gears_ffi` — C API for other games.
- `game/` — Bevy desktop range on top of gears_play (separate workspace).
- `web/` — browser range (JS ports; the Rust is the reference).
