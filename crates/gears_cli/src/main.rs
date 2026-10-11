//! Headless runner: plays a scripted input sequence through gears_core at a fixed
//! 60 Hz tick and prints the `[GEARS]` state log plus a position trace.
//!
//!   cargo run -p gears_cli -- slide      # tap A toward a wall
//!   cargo run -p gears_cli -- chain      # slide → slip → roadie (A held)
//!   cargo run -p gears_cli -- roadie     # hold A, roadie run into cover

use gears_core::testworld::{Wall, WallWorld};
use gears_core::{Character, Config, CoverHeight, Input, Vec3, World};

const DT: f32 = 1.0 / 60.0;

struct Step {
    until: f32,
    input: Input,
}

fn inp(x: f32, y: f32, a: bool) -> Input {
    Input {
        stick_x: x,
        stick_y: y,
        cam_yaw: 0.0,
        a,
        aim: false,
    }
}

fn run(name: &str, world: &dyn World, start: Vec3, steps: &[Step], end: f32) {
    let mut c = Character::new(Config::default(), start, 0.0);
    let mut next_trace = 0.0;
    println!("== scenario: {name}");
    while c.time < end {
        let t = c.time;
        let input = steps
            .iter()
            .find(|s| t < s.until)
            .map(|s| s.input)
            .unwrap_or_default();
        let before = c.events.len();
        c.tick(input, DT, world);
        for e in &c.events[before..] {
            println!(
                "[GEARS] t={:6.3} {} -> {} ({})",
                e.time, e.from, e.to, e.reason
            );
        }
        if c.time >= next_trace {
            println!(
                "        t={:6.3} pos=({:7.1},{:7.1}) speed={:6.1} yaw={:6.1}° {}",
                c.time,
                c.pos.x,
                c.pos.y,
                c.vel.length_2d(),
                c.yaw.to_degrees(),
                c.state_name()
            );
            next_trace += 0.1;
        }
    }
}

/// Deterministic input fuzz for parity checks with web/gears_core.js.
/// Stick values are multiples of 1/4 so f32 (Rust) and f64 (JS) hold them exactly.
fn fuzz(seed: u32, frames: u32) {
    let world = WallWorld::two_sided(
        vec![
            Wall {
                a: Vec3::new(300.0, -200.0, 0.0),
                b: Vec3::new(300.0, 200.0, 0.0),
                normal: Vec3::new(-1.0, 0.0, 0.0),
                height: CoverHeight::Mid,
            },
            Wall {
                a: Vec3::new(-300.0, -200.0, 0.0),
                b: Vec3::new(-300.0, 200.0, 0.0),
                normal: Vec3::new(1.0, 0.0, 0.0),
                height: CoverHeight::Mid,
            },
            Wall {
                a: Vec3::new(-150.0, 400.0, 0.0),
                b: Vec3::new(150.0, 400.0, 0.0),
                normal: Vec3::new(0.0, -1.0, 0.0),
                height: CoverHeight::Standing,
            },
            // in line with the first wall past a 60 uu gap: swat turns get exercised
            Wall {
                a: Vec3::new(300.0, 260.0, 0.0),
                b: Vec3::new(300.0, 460.0, 0.0),
                normal: Vec3::new(-1.0, 0.0, 0.0),
                height: CoverHeight::Mid,
            },
        ],
        24.0,
    );
    let mut c = Character::new(Config::default(), Vec3::ZERO, 0.0);
    let mut state = seed;
    let mut next = || {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        state >> 16
    };
    let mut input = inp(0.0, 0.0, false);
    for frame in 0..frames {
        if frame % 6 == 0 {
            let r = next();
            input = inp(
                (r % 9) as f32 / 4.0 - 1.0,
                ((r / 9) % 9) as f32 / 4.0 - 1.0,
                (r / 81) % 3 != 0,
            );
        }
        let before = c.events.len();
        c.tick(input, DT, &world);
        for e in &c.events[before..] {
            println!(
                "[GEARS] t={:6.3} {} -> {} ({})",
                e.time, e.from, e.to, e.reason
            );
        }
    }
    println!(
        "final pos=({:.1},{:.1}) state={}",
        c.pos.x,
        c.pos.y,
        c.state_name()
    );
}

fn main() {
    let which = std::env::args().nth(1).unwrap_or_else(|| "all".into());
    if which == "fuzz" {
        let seed = std::env::args()
            .nth(2)
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);
        fuzz(seed, 3600);
        return;
    }
    // A waist-high wall 300 uu ahead, 400 uu long, facing back toward the start.
    let wall = WallWorld::new(vec![Wall {
        a: Vec3::new(300.0, -200.0, 0.0),
        b: Vec3::new(300.0, 200.0, 0.0),
        normal: Vec3::new(-1.0, 0.0, 0.0),
        height: CoverHeight::Mid,
    }]);

    if which == "slide" || which == "all" {
        run(
            "tap A toward a wall 266 uu away",
            &wall,
            Vec3::new(0.0, 0.0, 0.0),
            &[
                Step {
                    until: 0.1,
                    input: inp(0.0, 1.0, true),
                },
                Step {
                    until: 2.0,
                    input: inp(0.0, 0.0, false),
                },
            ],
            1.0,
        );
    }
    if which == "chain" || which == "all" {
        run(
            "slide near the right edge, A + stick up during the slide, keep A held",
            &wall,
            Vec3::new(0.0, 150.0, 0.0),
            &[
                Step {
                    until: 0.1,
                    input: inp(0.0, 1.0, true),
                }, // tap A: slide
                Step {
                    until: 0.2,
                    input: inp(0.0, 1.0, false),
                },
                Step {
                    until: 3.0,
                    input: inp(0.0, 1.0, true),
                }, // A held + stick up
            ],
            2.0,
        );
    }
    if which == "roadie" || which == "all" {
        run(
            "hold A from 500 uu away: roadie run into cover",
            &wall,
            Vec3::new(-200.0, 0.0, 0.0),
            &[Step {
                until: 3.0,
                input: inp(0.0, 1.0, true),
            }],
            1.6,
        );
    }
}
