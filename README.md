# libgears

Gears of War 3's third-person movement — roadie run, slide into cover, cover, evade,
mantle and wallbouncing — rebuilt in Rust as a library that other games can host.

> **Status:** core movement works in tests, the CLI and a playable browser test range.
> For offline / single-player games only. You need your own copy of Gears of War 3
> (Xbox 360) to export the animations; nothing from Gears ships with this project.

## What works
- `crates/gears_core`: walk, roadie run, slide into cover, cover, cover run, cover
  slip, swat turn, push out of cover, moving along cover, evade, vault over waist-high cover, with Gears 3's chaining rules (override / queue)
  and timings. Tested headless (`cargo test`, 62 tests across the workspace). Evade, slip, swat and vault distances and
  timings come from Marcus's own animations.
- Test range shooting: Lancer and Gnasher with Gears 3's damage, fire rate, pellet
  pattern, range falloff, magazines, reload times and active reload, aiming camera, targets, Gears' own weapon sounds.
- **C API** (`crates/gears_ffi` → `gears.dll` / `libgears.so`, header
  `include/libgears.h`): movement + Lancer / Gnasher rules for any game. A C host
  (`examples/host.c`) slides into cover, vaults and fires through it. How to port:
  [docs/PORTING.md](docs/PORTING.md), including the honest status for WaW zombies.
- `cargo run -p gears_cli -- all`: scripted runs with the `[GEARS]` state log.
- **3D test range** (`web/range3d.html`): third person with Gears 3's own camera
  numbers (FOV, shoulder offsets, follow lag, look speed); controller, mouse +
  keyboard or touch. A top-down version is in `web/sandbox.html`. Both run a
  JavaScript port of the core; build with `python tools/build_web.py`. The port is
  checked against Rust with `node tools/parity.js fuzz <seed>` vs
  `cargo run -p gears_cli -- fuzz <seed>` (199/200 seeds give identical move logs; in 10 more the final position differs by 0.1 uu of rounding).
- Tools that read Gears 3's packages and settings from your own dump
  (`tools/inspect_packages.py`, `tools/ue3_tables.py`, `tools/unpack_coalesced.py`).
- Real movement numbers recovered and cited in [docs/GEARS_VALUES.md](docs/GEARS_VALUES.md).

## Desktop range (native, Bevy)
Like the browser range, but a real program: `gears_core` + `gears_play` (engine-free:
camera, animation driver, combat, levels, sound cues; checked bone-for-bone against the
browser range) under a thin Bevy 0.15 app in `game/`.
1. Install Rust (rustup.rs), Python 3.10+ with Pillow, ffmpeg, and UE Viewer (umodel)
   plus its `decompress` tool.
2. Build your assets once from your own dump:
   `python tools/setup.py --dump "_local/dumps/gears3/Gears 3" --umodel <umodel.exe> --decompress <decompress.exe>`
3. `cd game` then `cargo run --release` (first build downloads Bevy and takes a while).
Controls: WASD / left stick, mouse (click to capture) / right stick, Space / A, right mouse
/ LT aim, left mouse / RT fire, E / RB reload (again in the zone = active reload), Q / Y
swap, 1 2 3 ranges, R reset. **Not tested yet: the Bevy app has not been compiled** (this
machine can't download crates); everything under it is tested.

## See Marcus in the test range
The page never contains Gears files. It loads *your* export from your PC:
1. Already done for you: `_local/exports/marcus/pack/marcus.glb` and `lancer.glb`
   (made from your own dump). To rebuild them: export with UE Viewer
   (`umodel -export -gltf GearGame.xxx COG_Marcus_Fenix_CamSkel SkeletalMesh`,
   `umodel -export GearGame.xxx AnimSetMarcus_CamSkel AnimSet`, same for
   `AnimSetMarcus_CamSkel_LayeredNew` and `-gltf ... COGAssaultRifle`), then run
   `python tools/gears_anim_pack.py` (usage at the top of the file).
2. Open the test range, press **Load your Marcus export**, pick the files in `_local/exports/marcus/pack` (`marcus.glb`, `lancer.glb`, `gnasher.glb`, `gears_sounds.json`, `marcus_aim.json`) (or
   drag them onto the view). Your browser reads them locally.

## What doesn't work yet
| Milestone | Feature | State |
|---|---|---|
| L0 | Read Gears 3's movement code (bytecode reader) | done for the core moves |
| L1 | Core movement (Rust): walk, roadie, slide, cover, cover run, slip, swat turn, push out, evade, vault over mid cover, aiming | tests pass; playable in the 3D range, the desktop range and Fallout 4 (proof of concept) |
| L2 | Wallbounce, checked against Gears 3 in Xenia | not started |
| L3 | Find cover automatically in any level | not started |
| L4 | Gears 3 animations (exported locally) | textured Marcus + Lancer + Gnasher, 56 clips + gun anims, aim offsets, 61 sounds incl. Marcus's chatter play in the 3D range from your own export; evade/slip/swat/vault distances in the core come from the anims |
| L5 | C API + first host game | C API done and tested from C; Windows build not tried (needs your PC); WaW port planned (docs/PORTING.md) |

See [docs/DESIGN.md](docs/DESIGN.md) for the plan.

## Parked
**Locust der Untoten**, a WaW zombies mod (`src/`, `patches/`, `tools/build.py`).
See the bottom of DESIGN.md and MODLOG.md.

## Not in this repo, ever
Gears of War files, decompiled script, exported animations or models, WaW files.

## Credits
- Epic Games / The Coalition / Xbox: Gears of War. Unofficial fan project, not
  affiliated with or endorsed by them.
- Gildor: UE Viewer (package decompressor, format reference).
- Eliot: UELib / UE Explorer (UE3 format reference).
- libsm64, for the "game as a library" idea.
- [AI Game Modding Guides](https://github.com/trevaintdead/ai-game-modding-guides),
  [universal-modder](https://github.com/rehan-remade/universal-modder),
  [2010 Rust Rewrite Mashup](https://github.com/chasmlol/2010-rust-rewrite-mashup).
