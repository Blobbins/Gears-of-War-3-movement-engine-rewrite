//! gears_play — the engine-free game layer on top of gears_core: Gears 3 camera,
//! animation driver (your own exported clips), combat, test levels and sound cues.
//! A host (the Bevy app in `game/`, or any engine) feeds input and draws the output.
//!
//! No Gears data is in this crate: clips, sounds and aim data load at runtime from the
//! files you export from your own copy (see tools/setup.py).

pub mod anim;
pub mod camera;
pub mod glb;
pub mod json;
pub mod level;
pub mod qmath;
pub mod rig;
pub mod session;
