//! gears_core — Gears of War 3 style third-person movement, rebuilt from notes on
//! the game's own script (docs/RE_NOTES.md). Engine-free and deterministic: the host
//! supplies input, frame time and answers to world queries ([`World`]).
//!
//! No Gears of War code or data is included; numbers are cited in `config.rs`.

pub mod character;
pub mod config;
pub mod math;
pub mod testworld;
pub mod weapons;
pub mod world;

pub use character::{Character, Event, Input, MoveInfo, MoveKind};
pub use config::Config;
pub use math::Vec3;
pub use weapons::{Weapon, WeaponKind, WeaponSpec, WeaponTick};
pub use world::{CoverHeight, CoverSpot, EmptyWorld, World};
