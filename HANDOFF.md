# Handoff: libgears (2026-10-08)

For the next agent. Read `AGENTS.md` first: its hard rules are not optional. Then this
file, then `MODLOG.md` (newest first) and `docs/DESIGN.md`.

## The project in one paragraph
**libgears** is a Rust rewrite of Gears of War 3's third-person movement, animation
and shooting, built as a library with a C API so it can be **ported into other
games**. That portable core is the main goal. Gears 3 is reverse engineered from the
owner's own Xbox 360 dump. Every number is cited in `docs/GEARS_VALUES.md`.
Animations, sounds and textures are exported locally from the owner's dump and loaded
at runtime, so the repo ships no Gears data. The Fallout 4 plugin (`ports/fallout4`)
was a proof of concept. It worked and the owner called it a success, but the owner
now wants to go back to **improving the core rewrite itself**.

## The owner
- Not a programmer. Explain terms the first time you use them, in plain language.
- Token-conscious: keep replies short and avoid re-reading big files.
- Tests on a Windows PC ("counterstrike-machine", RTX 3080 Ti) and reports back.
- Wants an exact feel of Gears 3: Marcus's movement, animation and shooting.

## Where things live
- **Sandbox (agent):** `/home/claude/locust_zm`. crates.io is blocked here, so the Bevy
  app (`game/`) can't be built in the sandbox. Everything else builds and tests here.
- **Owner's PC (the real copy):** `G:\I be merging my shi\gowwaw`. Write to it with
  `mcp__remote-devices__device_commit_files`, staging under
  `/mnt/user-data/outputs/<a NEW folder each time>/`. Reusing a staged path sometimes
  writes a stale copy, so always verify afterwards (sizes with `device_list_dir`, or
  re-stage and `md5sum`).
- The owner's Gears export is at `_local/exports/marcus/pack` (glbs, sounds, aim data).
  It exists both in the sandbox and on the PC. It is gitignored and must never be
  committed.
- Fallout 4 is at `G:\Steam\steamapps\common\Fallout 4` (F4SE 0.7.9, game 1.11.240).
  Its F4SE logs are in `%USERPROFILE%\OneDrive\Documents\My Games\Fallout4\F4SE`.
- Browser test range artifact: https://claude.ai/artifact/PncFJXA9vu92Z8LfZGi8cu
- The project is **not a git repo yet**. `.gitignore` is already a whitelist.

## Layout
- `crates/gears_core`: the deterministic movement and weapon rules: character state
  machine, config (cited values), world trait, test world, weapons.
- `crates/gears_play`: the engine-free game layer:
  - glb reader and animator (blending, additive layers, aim offsets)
  - Gears camera, test levels, session, sound cues
  - **`rig.rs`**: our own 24-bone shareable standard rig plus a retargeter from Marcus
- `crates/gears_ffi`: the C API (`include/libgears.h`): character, walls, weapons, and
  anim + rig (`gears_anim_*`, `gears_rig_*`). Examples are `examples/host.c` and
  `examples/rig_host.c`.
- `game/`: the Bevy desktop range, which builds on the PC with `cargo run --release`.
  M toggles between the rig capsules and Marcus.
- `web/range3d.html` and `web/gears_core.js` are the browser range, a JS port of
  gears_core. Check parity with `tools/parity.js`.
- `tools/setup.py` runs the whole export pipeline from a dump.
- `ports/fallout4`: the F4SE plugin. Its README and the project doc
  `claude/fallout4-port-status.md` cover it.

## State of the core
- `cargo fmt`, `cargo clippy --all-targets` and `cargo test` are clean; 62 tests pass.
  Run all three before claiming anything works.
- Working moves:
  - walk, roadie run
  - slide into cover, cover, cover run, slip, swat turn, push out
  - evade, vault
- Weapons: Lancer and Gnasher, with active reload.
- Assets: animations, sounds and aim offsets from the owner's export.

## Best next steps for the core (suggested order)
1. **Move the host lessons from Fallout 4 into the library.** Every port will hit the
   same problems, so solve them once in `gears_ffi`:
   - A **ray-cast cover finder**: the host gives only a raycast callback. Port
     `FindCover`, `CoverAt`, `MantleDistance` and `IsClear` from
     `ports/fallout4/src/main.cpp` into Rust with tests. The wall test: one face,
     upright from 25 to 85 uu, at least 60 uu wide, and it steps past up to 4 hits of
     clutter.
   - A **host sync helper**: give it the engine's actual position and get back the
     velocity to apply. Its rules:
     - Gears leads during timed moves and in cover, resyncing only on drift above
       150 / 40 uu.
     - Velocity is the per-frame displacement, capped at 1000 uu/s.
     - The vault places the actor directly.
   - This also covers milestone L3, "find cover automatically in any level".
2. **L2 wallbounce fidelity:** compare against real Gears 3, in Xenia or on the 360,
   with the owner's help. Frame-count the slide speed, the chain timing and the snap
   distances.
3. **Resolve the assumptions** in `docs/GEARS_VALUES.md`, each marked
   **assumption**:
   - active reload offset
   - aim error units
   - recoil
   - roadie hold-time meaning
   - speed after a cancelled slide
   - swat edge tolerance
   - the GUDS voice tables
4. **Missing mechanics:** lean out and blind fire from cover, mantle-kick, effects,
   gamepad on PC.
5. **Housekeeping:** README numbers are stale (it says 58 / 42 tests; there are 62
   now). For going public: `git init`, a LICENSE file (workspace says MIT; the
   Fallout plugin must be GPL-3.0 because of CommonLibF4), and an audit that no Gears
   data is in tracked files.

## Fallout 4 port: open items (low priority now)
- Sliding into cover with W from a distance: a look-past-clutter build is installed
  but untested.
- In Gears mode only, the player snags on small objects. The plugin logs a `stuck:`
  line with the angle between movement and facing. A theory to check: Fallout steps
  over obstacles worse when moving sideways or backwards relative to the actor's
  facing.
- Build steps on the PC:
  - Use the x64 Native Tools prompt.
  - Run `set CC=cl` and `set CXX=cl` (MinGW is on PATH).
  - Run `xmake`.
  - The agent copies the dll into `Data/F4SE/Plugins` and checks it by md5.
  - Fallout must be closed before copying.

## How the owner and agent work
- Write the code, then sync it to the PC and check it arrived. The owner builds and
  runs on the PC and reports what happened. The agent reads the logs through the
  device bridge.
- "Not tested" means exactly that. Say it.
- After two failed attempts at the same problem, stop and write `STATUS.md`.
- Log every change in `MODLOG.md`, including failures.
