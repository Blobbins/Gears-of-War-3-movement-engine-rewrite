# AGENTS.md

## Project
**libgears** (current, since 2026-10-07): a Rust reimplementation of Gears of War 3's
third-person character movement and animation — roadie run, slide into cover, cover,
evade, mantle and the wallbouncing that emerges from them — as a library with a C API
that any (offline) game can host. Test sandbox first; first host game chosen later.

Approach: Gears 3 is reverse engineered from my own Xbox 360 dump. Its UnrealScript
is decoded locally to get the real logic and numbers; animations are exported locally
with UE Viewer. We write a clean Rust reimplementation from plain-language notes.
See `docs/DESIGN.md`.

Parked: **Locust der Untoten** (`locust_zm`), the WaW zombies mod. Its rules below
(WaW install, Mod Tools, T4M) still apply if it is resumed.

## Rust rules
- `crates/gears_core` stays engine-free and deterministic: no Bevy, no OS, no clock,
  no randomness without a seed. All world access goes through the collision trait.
- `web/gears_core.js` is a port of gears_core for the browser test ranges
  (`web/range3d.html` 3D, `web/sandbox.html` top-down). After any
  core change, update it too and run the parity check (`node tools/parity.js fuzz N`
  vs `cargo run -p gears_cli -- fuzz N`) before publishing the page.
- Every state transition logs `[GEARS] t=<time> <from> -> <to> (<reason>)`.
- Every mechanic gets a `cargo test` that replays a recorded input sequence and checks
  numbers against `docs/GEARS_VALUES.md`.
- Run `cargo fmt`, `cargo clippy` and `cargo test` before saying something works.
- Never embed Gears data (numbers are fine, with their source cited; anims, meshes and
  decompiled code are not) in the crates. Exported anims load from `_local/` at runtime.

## Hard rules, never break these

1. **Never write game assets, decompiled code, or extracted game files into this
   repository.** That covers Gears of War files (from the owner's own 360 disc dumps),
   WaW stock files, and anything that came with the Mod Tools. They stay on this
   machine, untracked, in `_local/` (gitignored).
2. **Stock WaW scripts are never committed.** Our code lives in our own files
   (`src/maps/lz_*.gsc`). Where a stock script needs a hook, keep the edit as a small
   patch in `patches/` and apply it with `tools/build.py` against the owner's local
   Mod Tools copy.
3. **`.gitignore` is a whitelist.** It ignores everything and includes only our
   source. Do not switch it to a normal ignore list.
4. **Do not run `git commit` unless I asked.**
5. **Do not touch anything outside this project folder** unless I name the path.
   Allowed writes outside the project, only through `tools/build.py`:
   - `%LOCALAPPDATA%\Activision\CoDWaW\mods\locust_zm\` (the built mod)
   Read-only: the WaW install, the Mod Tools `raw/` folder, and the Gears dumps.
   Ask before installing anything into the WaW install folder (T4M, Mod Tools).
6. **Solo and offline only.** Test with solo zombies. Never join or host an online
   co-op lobby with this mod loaded. Gears games are never launched, patched or hooked
   by this project; their files are only read by the converter.
7. **Never put credentials in the repo** or in any file you can read.
8. **Decompiled code stays local.** Decompiled Gears UnrealScript, package dumps and
   any Ghidra databases go in `_local/re/` only. Never paste decompiled code into
   `src/`. Write what you learned in plain language in `docs/RE_NOTES.md` and
   reimplement it in our own code.
9. **Leave DRM and copy protection alone.** Work from the packages first. Only touch
   `default.xex` if a mechanic is native code, and stop and ask me before doing so.

## How to work

- **Plan before code.** Anything bigger than a small fix goes into `docs/DESIGN.md`
  first, then implement one step at a time.
- **One thing at a time.** One mechanic per change, so it can be reverted alone.
- **Find the real numbers first.** Gears values (active reload windows, roadie run
  speed, chainsaw timing, enemy health) come from a cited source: first the
  decompiled class and property name, then the Gears wiki, then frame counting. Record
  each value and its source in `docs/GEARS_VALUES.md`. No guessed numbers.
- **Read before writing.** Before changing zombie behaviour, read the stock
  `_zombiemode*.gsc` scripts in the local Mod Tools `raw/maps/` and the Nazi Zombies
  Remastered source to see how it is already done.
- **Log, don't look.** You cannot see the game. Every mechanic prints state changes
  with an `[LZ]` prefix to the console log (`logfile 1`). Verify from the numbers.
- **Every mechanic has a test command.** A dvar such as `lz_test active_reload` runs a
  scripted check and prints `[LZ][TEST] <name> PASS|FAIL <details>`. See
  `docs/DESIGN.md` → Oracles.
- **Tell me how to test it.** After every change: the exact command to launch, what to
  do in game, and what I should see in the log.
- **Ask before large refactors** and explain in plain language. I am not the
  programmer here; explain a term the first time you use it.

## Honesty

- If something is not tested, write **"not tested"**.
- If you are not sure, say so.
- After two real attempts at the same problem, stop and write `STATUS.md` (format in
  the ai-game-modding-guides `STATUS-handoff.md` template) instead of trying random
  variations.
- Record failures in `MODLOG.md`, not just successes.

## Keep these files updated

- `MODLOG.md`: an entry after every change, newest first.
- `docs/DESIGN.md`: how the mod works and the milestone plan.
- `docs/GEARS_VALUES.md`: every Gears number we use, with its source.
- `README.md`: the "what works / what doesn't" list. Test before claiming it works.

## Environment (fill in during recon)

- OS: Windows (PC "counterstrike-machine"), RTX 3080 Ti
- Host: Call of Duty: World at War, Steam, installed at
  `G:\Steam\steamapps\common\Call of Duty World at War`. `version.inf` says 1.6
  (IntVersion 47.1); confirm in game with the `version` console command.
  `CoDWaW.exe` differs from `CoDWaW.exe.bak` (same size, newer): already patched by
  something. Not by us; don't touch it.
- **T4M is installed:** `d3d9.dll` is T4Me r49 ("Call of Duty T4-SP (r49), built
  Feb 10 2026 by Clippy95 and Nazi Zombies Remastered developer JB"), config in
  `d3d9.ini` (`UseFixedXAudio=1`, `DXVK=0`), with optional DXVK (`dxvk.dll`). Its
  console prompt reads `T4Me r49>`. Don't replace or reconfigure it without asking.
- **Developer mode crashes Nacht** in stock `_debug.gsc` (`debug_show_viewpos`:
  `NewHudElem()` returns undefined → "undefined is not a field object", lines
  3143–3146 of the copy inside the game's fastfiles). Not our code. Test without
  `developer_script`; our log uses `iprintln` outside developer mode.
- Mod Tools: 1.4 installed (raw/, map_source/, zone_source/, sp_tool, Launcher), plus
  Sniperbolt's tutorial, Tombmx's zappers and UGX Script Placer.
- **`raw/maps` is NOT stock.** The owner's other mod (`nazi_zombie_ror2`, Der Riese)
  changed `_zombiemode.gsc` and `_debug.gsc` there, and UGX files replaced several
  `_zombiemode_*` scripts. Never treat `raw/` as a clean source: `tools/build.py`
  checks MD5s of every stock file it patches.
- **Mod Builder behaviour (observed 2026-10-07):** runs
  `linker_pc -nopause -language english -moddir locust_zm mod`, reads scripts from
  `mods\locust_zm\`, does **not** touch `raw\maps`. It overwrites the scratch file
  `zone_source\mod.csv` every build, moves the built `mod.ff` into `mods\locust_zm\`,
  and copies it to `%LOCALAPPDATA%\Activision\CoDWaW\mods\locust_zm\mod.ff`. It also
  copied `nazi_zombie_tutorial\mod.arena` into AppData mods (Launcher quirk, harmless).
  Our mod only overrides Nacht's map script, which RoR2 doesn't use.
- Mods: build folder `G:\...\Call of Duty World at War\mods\locust_zm\`; the game
  reads `%LOCALAPPDATA%\Activision\CoDWaW\mods\` (RoR2 uses a link from there to the
  install folder). Don't touch `nazi_zombie_ror2`.
- Backup: `_local/backups/codwaw_backup_2026-10-07.zip` (AppData profiles/configs +
  stock `nazi_zombie_prototype.gsc`, MD5 manifest inside).
- Guest data: Gears of War 3 (first), then 1 / 2, Xbox 360, dumped from my own discs,
  extracted to `_local/dumps/gears3/` etc. inside this project (gitignored)
- RE tools: UELib / UE Explorer [version] (UnrealScript decompile), UE Viewer (umodel)
  [version] (assets), Ghidra [version] (last resort, ask first)
- Asset tools: Blender [version] + WaW export plugin [name]
- Comparison: Gears in Xenia [version] or on my 360
- Not used: Gears of War: E-Day (UE5, online, too new — reference for feel only)
- Which game owns the player: WaW (the only game running)
- Language: GSC (WaW game scripts), Python 3.10+ for tools
- Agent: [Claude / Claude Code]

## References
- Guides: https://github.com/trevaintdead/ai-game-modding-guides
- Mashup patterns: https://github.com/rehan-remade/universal-modder (skills/mashup-mods)
- Rewrite mashup example: https://github.com/chasmlol/2010-rust-rewrite-mashup
- Stock-map overhaul example: Nazi Zombies Remastered (T4M) by JB Shady, UGX-Mods
- Asset reader: UE Viewer / umodel, https://www.gildor.org/en/projects/umodel
