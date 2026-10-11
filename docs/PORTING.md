# Porting libgears into a game

libgears is the Gears of War 3 movement (roadie run, cover, slide, slip, swat turn,
push out, evade, vault, aiming walk) and the Lancer / Gnasher weapon rules as a
library. It never owns your world: you give it input and answer a few traces; it
tells you where the player is, what state it's in and what each shot does.

## 1. Build the library
On the machine you build your game on:
```
rustup target add i686-pc-windows-msvc        # 32-bit games (WaW, most older games)
cargo build --release -p gears_ffi --target i686-pc-windows-msvc
cargo build --release -p gears_ffi             # your own machine (64-bit)
```
Output: `gears.dll` (+ `gears.dll.lib`) or `libgears.so` / `libgears.a`. Header:
`include/libgears.h`. A complete C host: `examples/host.c` (slides into cover, vaults,
fires the Gnasher; run it to check your build).

## 2. Each frame
1. Read your controller into `GearsInput` (left stick, camera yaw, A, left trigger).
2. `gears_character_tick(c, &input, 1/60, &world)` — fixed 1/60 s steps are best.
3. `gears_character_state(c, &s)` → put your player at `s.pos` (+ `s.height` up),
   facing `s.yaw`; pick animations from `s.state`; move the camera.
4. Weapons: `gears_weapon_tick` says how many shots to fire this frame. For each:
   trace from the camera along its forward, plus a random direction inside
   `gears_weapon_aim_error` (Gnasher: one trace per `gears_weapon_pellets` offset),
   then apply `gears_weapon_damage(kind, hits, distance)`.

## 3. Your world
Either answer the four callbacks in `GearsWorld` with your engine's traces, or
describe cover as straight walls with `gears_walls_add` and call
`gears_character_tick_walls` (the easiest start: hand-place walls like Gears' cover
links). The library does no collision while walking: keep your engine's own
collision and use `gears_character_set_pos` to push the player back out of walls.

## 3b. The standard rig
`gears_play::rig` is libgears' own skeleton (24 bones, our names, T-pose, in the
Marcus model frame: Y up, +Z forward, +X left, uu). Each frame `Frame.rig` gives every
bone's position, model rotation (relative to the T-pose) and local rotation. Map it
onto your game's character once, like any retarget (a T-posed humanoid lines up
directly). The motion comes from each player's own Gears export at runtime; the rig
itself contains nothing from Gears, so it can be shared.

## 4. Units
Everything is Gears units: the Gears pawn is 156 uu tall (2 × 78), radius 34, walks
at 300 uu/s. Scale positions and speeds by *your player height / 156*. Example:
a 72-unit-tall player → × 0.46.

## 5. Call of Duty: World at War zombies — honest status
WaW's game logic is GSC script, and GSC cannot load a DLL. So there are two routes:

**A. GSC port (recommended first).** Rewrite the rules in GSC, reading the numbers
from `docs/GEARS_VALUES.md` (the Rust code is the reference to copy). Damage is the
easy half: `_zombiemode`'s damage callbacks can apply Gears' range bands, the
Gnasher's 75 % / 1.2× rules and the Lancer's per-bullet damage. Movement is the hard
half, because WaW's engine owns player physics: the usual WaW-mod trick is to link
the player to a script origin and move that every frame while a special move runs
(roadie run, slide, evade, vault), and leave normal walking to the engine. Third
person needs `cg_thirdPerson`, which is cheat-protected outside a dev map. All of
this is **untested** and has to be checked in game, one move at a time (the
`[LZ]` log + `lz_test` dvar plan in DESIGN.md).

**B. Native plugin.** A DLL loaded next to the game (for example through the T4M
setup you already have, if it supports plugins — not checked) that hosts `gears.dll`
and drives the player directly. Most faithful, but it needs reverse engineering of
`CoDWaW.exe`'s player movement, which this project's rules say to ask about first.
Single-player / solo zombies only, never online.

## 6. Not in the library yet
Active reload, animation selection (your engine plays its own anims from `state`),
mantle kick / mantle up / mantle down, the camera (its Gears numbers are in
`docs/GEARS_VALUES.md` → Camera and implemented in `web/range3d.html`).
