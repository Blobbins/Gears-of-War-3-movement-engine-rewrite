//! Replays scripted input and checks the results against docs/GEARS_VALUES.md.

use gears_core::math::urot_to_rad;
use gears_core::testworld::{Wall, WallWorld};
use gears_core::{Character, Config, CoverHeight, EmptyWorld, Input, MoveKind, Vec3, World};

const DT: f32 = 1.0 / 60.0;

fn inp(x: f32, y: f32, a: bool) -> Input {
    Input {
        stick_x: x,
        stick_y: y,
        cam_yaw: 0.0,
        a,
        aim: false,
    }
}

/// Run `f(t)` inputs until `end` seconds; returns the character.
fn sim(world: &dyn World, start: Vec3, end: f32, f: impl Fn(f32) -> Input) -> Character {
    let mut c = Character::new(Config::default(), start, 0.0);
    while c.time < end {
        let i = f(c.time);
        c.tick(i, DT, world);
    }
    c
}

fn wall_ahead() -> WallWorld {
    WallWorld::new(vec![Wall {
        a: Vec3::new(300.0, -200.0, 0.0),
        b: Vec3::new(300.0, 200.0, 0.0),
        normal: Vec3::new(-1.0, 0.0, 0.0),
        height: CoverHeight::Mid,
    }])
}

fn first(c: &Character, to: &str) -> Option<f32> {
    c.events.iter().find(|e| e.to == to).map(|e| e.time)
}

#[test]
fn walking_tops_out_at_ground_speed() {
    // Default__GearPawn.GroundSpeed = 300, Pawn.AccelRate = 2048 → full speed in ~0.15 s
    let c = sim(&EmptyWorld, Vec3::ZERO, 1.0, |_| inp(0.0, 1.0, false));
    assert!(
        (c.vel.length_2d() - 300.0).abs() < 0.01,
        "speed {}",
        c.vel.length_2d()
    );
}

#[test]
fn roadie_run_starts_after_0_3s_and_runs_at_1_6x() {
    let c = sim(&EmptyWorld, Vec3::ZERO, 1.0, |_| inp(0.0, 1.0, true));
    let start = first(&c, "RoadieRun").expect("roadie run started");
    // first tick presses A at t=DT; RoadieRunTimer = 0.3 s
    assert!((start - (DT + 0.3)).abs() < DT + 1e-4, "roadie at {start}");
    assert!(
        (c.vel.length_2d() - 480.0).abs() < 0.01,
        "speed {}",
        c.vel.length_2d()
    );
}

#[test]
fn releasing_a_ends_roadie_run() {
    let c = sim(&EmptyWorld, Vec3::ZERO, 1.0, |t| inp(0.0, 1.0, t < 0.6));
    assert_eq!(c.state_name(), "Walk");
    assert!(c
        .events
        .iter()
        .any(|e| e.from == "RoadieRun" && e.reason == "A released"));
}

#[test]
fn tap_a_slides_into_cover_with_gears_timing() {
    let w = wall_ahead();
    let c = sim(&w, Vec3::ZERO, 1.5, |t| inp(0.0, 1.0, t < 0.05));
    let start = first(&c, "Run2Cover").expect("slide started");
    let arrive = c
        .events
        .iter()
        .find(|e| e.from == "Run2Cover")
        .expect("slide ended")
        .time;
    assert_eq!(c.state_name(), "InCover");
    // GSM_Run2Cover: duration = distance / (300 * 2.3), min 0.2 s.
    // Cover spot is 34 uu off the wall at x=300 → x=266; the character walked a bit first.
    let travelled = 266.0 - 0.0_f32; // upper bound
    let expected_max = travelled / (300.0 * 2.3);
    let took = arrive - start;
    assert!(
        took <= expected_max + DT && took >= 0.2,
        "slide took {took}"
    );
    assert!((c.pos.x - 266.0).abs() < 0.5, "ended at x={}", c.pos.x);
}

#[test]
fn short_slides_last_at_least_0_2s() {
    let w = wall_ahead();
    // start 40 uu from the cover spot: 40/690 = 0.058 s → clamped to 0.2 s
    let c = sim(&w, Vec3::new(226.0, 0.0, 0.0), 1.0, |t| {
        inp(0.0, 1.0, t < 0.02)
    });
    let start = first(&c, "Run2Cover").expect("slide");
    let end = c
        .events
        .iter()
        .find(|e| e.from == "Run2Cover")
        .unwrap()
        .time;
    assert!(
        end - start >= 0.2 - 1e-4 && end - start < 0.2 + 2.0 * DT,
        "took {}",
        end - start
    );
}

#[test]
fn cover_out_of_reach_is_ignored() {
    // Run2CoverMaxDist = 420: wall spot 520 uu away → no slide, evade instead.
    let w = wall_ahead();
    let c = sim(&w, Vec3::new(-254.0, 0.0, 0.0), 1.0, |t| {
        inp(0.0, 1.0, t < 0.05)
    });
    assert!(first(&c, "Run2Cover").is_none());
    assert!(first(&c, "EvadeFwd").is_some());
}

#[test]
fn stick_back_never_slides() {
    let w = wall_ahead();
    // facing the wall but pulling back: GearPC.CanRunToCover returns false
    let c = sim(&w, Vec3::ZERO, 1.0, |t| inp(0.0, -1.0, t < 0.05));
    assert!(first(&c, "Run2Cover").is_none());
    assert!(first(&c, "EvadeBwd").is_some());
}

#[test]
fn slip_requested_during_slide_is_queued_until_it_lands() {
    let w = wall_ahead();
    let c = sim(&w, Vec3::new(0.0, 150.0, 0.0), 2.0, |t| {
        inp(0.0, 1.0, !(0.05..=0.2).contains(&t))
    });
    let landed = c
        .events
        .iter()
        .find(|e| e.from == "Run2Cover")
        .expect("landed")
        .time;
    let slip = first(&c, "CoverSlip").expect("slip happened");
    assert!(
        (slip - landed).abs() < 1e-6,
        "slip at {slip}, landed {landed}"
    );
    // A still held when the slip ends → roadie run is queued (GSM_CoverSlip.SpecialMoveEnded)
    assert!(c
        .events
        .iter()
        .any(|e| e.to == "RoadieRun" && e.reason == "queued move"));
}

#[test]
fn slip_needs_the_cover_edge() {
    let w = wall_ahead();
    // centre of a 400 uu wall: 200 uu from either edge > 96 → no slip
    let c = sim(&w, Vec3::ZERO, 2.0, |t| {
        inp(0.0, 1.0, t < 0.05 || (t > 0.7 && t < 0.75))
    });
    assert!(first(&c, "CoverSlip").is_none());
}

#[test]
fn roadie_run_snaps_into_cover() {
    let w = wall_ahead();
    let c = sim(&w, Vec3::new(-200.0, 0.0, 0.0), 2.0, |_| {
        inp(0.0, 1.0, true)
    });
    assert!(c.events.iter().any(|e| e.reason == "roadie run into cover"));
}

#[test]
fn evades_follow_their_anims_root_motion() {
    // AR_Evade_Lt_Start (12 frames, 214.24 uu) then AR_Evade_Lt_End (22 frames,
    // 257.37 uu), both at GSM_Evade.SpeedModifier 1.15. The root motion is straight
    // sideways: the shipped 16834 is only in the clearance trace.
    let mut c = Character::new(Config::default(), Vec3::ZERO, 0.0);
    let (mut start, mut end, mut t0, mut t1) = (None, None, 0.0, 0.0);
    while c.time < 3.0 && end.is_none() {
        let t = c.time;
        c.tick(inp(-1.0, 0.0, t < 0.05), DT, &EmptyWorld);
        match (start, c.active_move()) {
            (None, Some(MoveKind::EvadeLt)) => {
                start = Some(c.pos);
                t0 = c.time;
            }
            (Some(_), None) => {
                end = Some(c.pos);
                t1 = c.time;
            }
            _ => {}
        }
    }
    let d = end.expect("evade ended") - start.expect("evaded left");
    assert!(
        (d.y.atan2(d.x) + std::f32::consts::FRAC_PI_2).abs() < 1e-4,
        "dir {d:?}"
    );
    // + up to one frame of walking after it ends (same tick)
    assert!(
        (d.length_2d() - (214.24 + 257.37)).abs() < 1.0,
        "dist {}",
        d.length_2d()
    );
    let expected = (12.0 + 22.0) / 30.0 / 1.15;
    assert!((t1 - t0 - expected).abs() < DT + 1e-4, "took {}", t1 - t0);
    let _ = urot_to_rad(16834);
}

#[test]
fn evade_retrigger_waits_0_65s_after_the_last_one_ends() {
    // GearPC.NotifyEvadeFinished → CannotRetriggerEvade timer (EvadeRetriggerDelay 0.65)
    let c = sim(&EmptyWorld, Vec3::ZERO, 4.0, |t| {
        inp(1.0, 0.0, (t % 0.25) < 0.05)
    });
    let starts: Vec<f32> = c
        .events
        .iter()
        .filter(|e| e.to == "EvadeRt")
        .map(|e| e.time)
        .collect();
    let ends: Vec<f32> = c
        .events
        .iter()
        .filter(|e| e.from == "EvadeRt")
        .map(|e| e.time)
        .collect();
    assert!(starts.len() >= 2, "evades: {starts:?}");
    for (end, next) in ends.iter().zip(starts.iter().skip(1)) {
        assert!(
            next - end >= 0.65 - 1e-4,
            "next evade {next} only {} after {end}",
            next - end
        );
    }
}

#[test]
fn deterministic() {
    let w = wall_ahead();
    let f = |t: f32| inp((t * 3.0).sin(), (t * 2.0).cos(), (t % 0.7) < 0.35);
    let a = sim(&w, Vec3::ZERO, 5.0, f);
    let b = sim(&w, Vec3::ZERO, 5.0, f);
    assert_eq!(a.pos, b.pos);
    assert_eq!(a.events, b.events);
    let _ = MoveKind::RoadieRun;
}

#[test]
fn aborting_a_slide_drops_a_queued_slip() {
    // Found by the Rust/JS fuzz parity check: slip queued during a slide, then the slide
    // is aborted by holding back → the queued slip must not start (GearPC.AbortRun2Cover).
    // A 416 uu slide lasts 0.6 s, long enough for the 0.25 s back-hold to fire.
    let w = wall_ahead();
    let c = sim(&w, Vec3::new(-150.0, 150.0, 0.0), 1.5, |t| {
        if t < 0.05 {
            inp(0.0, 1.0, true) // tap A: slide
        } else if t < 0.15 {
            inp(0.0, 1.0, false)
        } else if t < 0.2 {
            inp(0.0, 1.0, true) // A + stick up during the slide: slip queued
        } else {
            inp(0.0, -1.0, true) // hold back: abort
        }
    });
    assert!(c
        .events
        .iter()
        .any(|e| e.reason == "slide aborted (stick back)"));
    assert!(first(&c, "CoverSlip").is_none());
}

#[test]
fn pulling_away_from_the_wall_cancels_the_slide_at_once() {
    // GSM_Run2Cover.JoystickHeldLongEnough: HoldingBack > 0 → GearPC.AbortRun2Cover.
    // This is the wall-bounce cancel: a single frame of "back" is enough.
    let w = wall_ahead();
    let pull_at = 0.2;
    let c = sim(&w, Vec3::new(-150.0, 0.0, 0.0), 1.5, |t| {
        if t < 0.05 {
            inp(0.0, 1.0, true)
        } else if t < pull_at {
            inp(0.0, 0.0, false)
        } else {
            inp(0.0, -1.0, false)
        }
    });
    let abort = c
        .events
        .iter()
        .find(|e| e.reason == "slide aborted (stick back)")
        .expect("aborted");
    assert!(
        abort.time - pull_at <= 2.0 * DT,
        "aborted at {}",
        abort.time
    );
    assert_eq!(c.events.last().unwrap().to, "Walk");
}

#[test]
fn a_cancelled_slide_hands_off_at_run_speed() {
    let w = wall_ahead();
    let mut c = Character::new(Config::default(), Vec3::new(-150.0, 0.0, 0.0), 0.0);
    while c.time < 0.25 {
        let t = c.time;
        c.tick(inp(0.0, if t < 0.2 { 1.0 } else { 0.0 }, t < 0.05), DT, &w);
    }
    // one frame of pulling back
    c.tick(inp(0.0, -1.0, false), DT, &w);
    assert_eq!(c.state_name(), "Walk");
    // the stick is neutral from here: the slide's direction carries on at 300, then slows
    assert!(
        (c.vel.x - 300.0).abs() < 2048.0 * DT + 0.01,
        "vel {:?}",
        c.vel
    );
}

#[test]
fn sideways_into_cover_cancels_by_pulling_away_from_the_wall() {
    // The "back" that cancels is relative to the cover, not the camera.
    let w = WallWorld::new(vec![Wall {
        a: Vec3::new(-200.0, 200.0, 0.0),
        b: Vec3::new(200.0, 200.0, 0.0),
        normal: Vec3::new(0.0, -1.0, 0.0),
        height: CoverHeight::Mid,
    }]);
    // wall to the right of the camera; slide sideways (stick right), then stick left
    let c = sim(&w, Vec3::new(0.0, 0.0, 0.0), 1.0, |t| {
        if t < 0.05 {
            inp(1.0, 0.0, true)
        } else if t < 0.1 {
            inp(1.0, 0.0, false)
        } else {
            inp(-1.0, 0.0, false)
        }
    });
    assert!(first(&c, "Run2Cover").is_some(), "{:?}", c.events);
    assert!(c
        .events
        .iter()
        .any(|e| e.reason == "slide aborted (stick back)"));
}

#[test]
fn holding_left_during_a_slide_faces_left_in_cover() {
    let w = wall_ahead();
    let c = sim(&w, Vec3::new(-150.0, 0.0, 0.0), 1.5, |t| {
        if t < 0.1 {
            inp(0.0, 1.0, t < 0.05) // tap A toward the wall
        } else {
            inp(-1.0, 0.0, false) // then hold left during the slide
        }
    });
    assert_eq!(c.state_name(), "InCover");
    assert!(c.mirrored);
}

/// Slide into cover at the wall ahead, then run `after` from the moment cover is reached.
fn in_cover_then(w: &WallWorld, end: f32, after: impl Fn(f32) -> Input) -> (Character, f32) {
    let mut c = Character::new(Config::default(), Vec3::ZERO, 0.0);
    let mut landed = None;
    while c.time < end {
        let t = c.time;
        let i = match landed {
            None => inp(0.0, 1.0, t < 0.05),
            Some(l) => after(t - l),
        };
        c.tick(i, DT, w);
        if landed.is_none() && c.state_name() == "InCover" {
            landed = Some(c.time);
        }
    }
    (c, landed.expect("reached cover"))
}

#[test]
fn holding_back_in_cover_pushes_out_after_0_05s() {
    // GearPC.BreakFromCoverHoldTime 0.05 → SM_PushOutOfCover: -300 × 0.75 = 225 uu/s,
    // for the cover-break anim: 0.4667 s / 0.75 = 0.622 s.
    let w = wall_ahead();
    let (c, landed) = in_cover_then(&w, 3.0, |_| inp(0.0, -1.0, false));
    let start = first(&c, "PushOut").expect("pushed out");
    let held = start - landed;
    assert!(
        (0.05 - 1e-4..=0.05 + 2.0 * DT).contains(&held),
        "held {held}"
    );
    let end = c
        .events
        .iter()
        .find(|e| e.from == "PushOut")
        .expect("push out ended")
        .time;
    assert!(
        (end - start - (14.0 / 30.0) / 0.75).abs() < DT + 1e-4,
        "took {}",
        end - start
    );
    let e = c.events.iter().find(|e| e.from == "PushOut").unwrap();
    assert_eq!(e.to, "Walk");
}

#[test]
fn push_out_shoves_back_at_225_then_you_steer() {
    // GSM_PushOutOfCover.DoPushOutSpecifics: Velocity = -300 x 0.75 along the facing.
    // From there the stick is in control (walking runs underneath the push out).
    let w = wall_ahead();
    let mut c = Character::new(Config::default(), Vec3::ZERO, 0.0);
    let mut landed = None;
    let mut first_vel = None;
    let mut later_vel = None;
    while c.time < 3.0 {
        let t = c.time;
        let i = match landed {
            None => inp(0.0, 1.0, t < 0.05),
            Some(l) if t - l < 0.2 => inp(0.0, -1.0, false), // pull away: push out
            Some(_) => inp(1.0, 0.0, false),                 // then steer right
        };
        c.tick(i, DT, &w);
        if c.state_name() == "InCover" {
            landed.get_or_insert(c.time);
        }
        if c.active_move() == Some(MoveKind::PushOut) {
            first_vel.get_or_insert(c.vel);
            if landed.map(|l| c.time - l > 0.4).unwrap_or(false) {
                later_vel.get_or_insert(c.vel);
            }
        }
    }
    let v0 = first_vel.expect("pushed out");
    // one walking tick on top of the shove: 225 back, accelerating toward 300
    assert!(
        v0.y.abs() < 0.01 && v0.x < -225.0 && v0.x > -225.0 - 2048.0 * DT - 0.01,
        "v0 {v0:?}"
    );
    let v1 = later_vel.expect("still in the push out 0.4 s after landing");
    assert!(
        v1.y > 100.0,
        "stick right should steer during the push out: {v1:?}"
    );
}

#[test]
fn a_slide_asked_for_during_a_push_out_starts_at_once() {
    // GearPC.CanTryToRunToCover allows SM_PushOutOfCover, and the push out gives way
    // to the next move: off one wall, tap A, slide to the wall behind straight away.
    let w = WallWorld::new(vec![
        wall_ahead().walls[0],
        Wall {
            a: Vec3::new(-100.0, -200.0, 0.0),
            b: Vec3::new(-100.0, 200.0, 0.0),
            normal: Vec3::new(1.0, 0.0, 0.0),
            height: CoverHeight::Mid,
        },
    ]);
    let tap_at = 0.15;
    let (c, landed) = in_cover_then(&w, 3.0, |t| {
        if t < tap_at {
            inp(0.0, -1.0, false) // push out
        } else {
            // camera turned around, stick forward, tap A during the push out
            let mut i = inp(0.0, 1.0, (tap_at..tap_at + 0.05).contains(&t));
            i.cam_yaw = std::f32::consts::PI;
            i
        }
    });
    let slide = c
        .events
        .iter()
        .find(|e| e.from == "PushOut")
        .expect("push out ended");
    assert_eq!(slide.to, "Run2Cover", "{:?}", c.events);
    assert!(
        slide.time - (landed + tap_at) < 0.05 + 2.0 * DT,
        "slide at {}",
        slide.time
    );
    assert!((c.pos.x - -66.0).abs() < 0.5, "x {}", c.pos.x);
}

#[test]
fn evading_out_of_cover_needs_the_stick_along_the_wall() {
    // PlayerTakingCover.TryToEvade: |RemappedJoyRight| > DeadZoneThreshold.
    let w = wall_ahead();
    // at the right edge of the wall (y = 199), tap A with the stick neutral: nothing
    let mut c = Character::new(Config::default(), Vec3::new(0.0, 199.0, 0.0), 0.0);
    while c.time < 2.0 {
        let t = c.time;
        c.tick(
            inp(
                0.0,
                if t < 0.05 { 1.0 } else { 0.0 },
                t < 0.05 || (1.0..1.05).contains(&t),
            ),
            DT,
            &w,
        );
    }
    assert!(
        c.events.iter().all(|e| !e.to.starts_with("Evade")),
        "{:?}",
        c.events
    );
    // same with the stick pushed right: evades out
    let mut c = Character::new(Config::default(), Vec3::new(0.0, 199.0, 0.0), 0.0);
    while c.time < 2.0 {
        let t = c.time;
        let x = if t > 0.9 { 1.0 } else { 0.0 };
        c.tick(
            inp(
                x,
                if t < 0.05 { 1.0 } else { 0.0 },
                t < 0.05 || (1.0..1.05).contains(&t),
            ),
            DT,
            &w,
        );
    }
    assert!(c.events.iter().any(|e| e.to == "EvadeRt"), "{:?}", c.events);
}

#[test]
fn cover_is_never_found_through_another_wall() {
    // Found in the 3D test range: pressed against a wall (closer than the 34 uu standoff),
    // a tap of A slid through it into cover on the next wall. Gears traces for cover.
    let w = WallWorld::new(vec![
        Wall {
            a: Vec3::new(-100.0, -250.0, 0.0),
            b: Vec3::new(-100.0, 250.0, 0.0),
            normal: Vec3::new(-1.0, 0.0, 0.0),
            height: CoverHeight::Mid,
        },
        Wall {
            a: Vec3::new(250.0, -250.0, 0.0),
            b: Vec3::new(250.0, 250.0, 0.0),
            normal: Vec3::new(-1.0, 0.0, 0.0),
            height: CoverHeight::Mid,
        },
    ]);
    // 32 uu from the first wall (inside the standoff): cover is the wall you're touching
    let spot = w
        .find_cover(Vec3::new(-132.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 420.0)
        .expect("the wall you're pressed against");
    assert!(
        (spot.location.x - -134.0).abs() < 1e-3,
        "spot {:?}",
        spot.location
    );
    // between the walls, the second wall is still in reach (wall order doesn't matter)
    let behind = WallWorld::new(vec![w.walls[1], w.walls[0]]);
    let s2 = behind
        .find_cover(Vec3::new(-60.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 420.0)
        .expect("cover in front");
    assert!((s2.location.x - 216.0).abs() < 1e-3);
    assert!(w
        .find_cover(Vec3::new(-200.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 420.0)
        .map(|s| s.location.x < 0.0)
        .unwrap_or(false));
    let c = sim(&w, Vec3::new(-132.0, 0.0, 0.0), 1.0, |t| {
        inp(0.0, 1.0, t < 0.05)
    });
    assert!(c.pos.x < -100.0, "slid through the wall to x={}", c.pos.x);
}

/// Two waist-high walls in line at x = 300 with a gap between y = -20 and `gap_end`.
fn gap_walls(gap_end: f32) -> WallWorld {
    WallWorld::new(vec![
        Wall {
            a: Vec3::new(300.0, -200.0, 0.0),
            b: Vec3::new(300.0, -20.0, 0.0),
            normal: Vec3::new(-1.0, 0.0, 0.0),
            height: CoverHeight::Mid,
        },
        Wall {
            a: Vec3::new(300.0, gap_end, 0.0),
            b: Vec3::new(300.0, gap_end + 220.0, 0.0),
            normal: Vec3::new(-1.0, 0.0, 0.0),
            height: CoverHeight::Mid,
        },
    ])
}

/// Slide into cover 10 uu from the right end of the first wall, then run `after`.
fn at_gap_then(w: &WallWorld, end: f32, after: impl Fn(f32) -> Input) -> (Character, f32) {
    let mut c = Character::new(Config::default(), Vec3::new(0.0, -30.0, 0.0), 0.0);
    let mut landed = None;
    while c.time < end {
        let t = c.time;
        let i = match landed {
            None => inp(0.0, 1.0, t < 0.05),
            Some(l) => after(t - l),
        };
        c.tick(i, DT, w);
        if landed.is_none() && c.state_name() == "InCover" {
            landed = Some(c.time);
        }
    }
    (c, landed.expect("reached cover"))
}

#[test]
fn swat_turn_short_gap_is_all_end_anim() {
    // GSM_StdLvlSwatTurn: SpeedModifier 1.7 × 300; lands in cover at the far wall.
    let w = gap_walls(80.0);
    let (c, _) = at_gap_then(&w, 3.0, |t| {
        // stick toward the gap only for the press (holding it would walk along the new wall)
        inp(
            if t < 0.15 { 1.0 } else { 0.0 },
            0.0,
            (0.1..0.15).contains(&t),
        )
    });
    let start = first(&c, "SwatTurn").expect("swat turn");
    let end = c
        .events
        .iter()
        .find(|e| e.from == "SwatTurn")
        .expect("swat ended");
    assert_eq!(end.to, "InCover");
    // from (266, -30) to (266, 80): 110 uu, inside EndTurnAnimDist (256), so it's all
    // end anim: AR_Cov_Mid_SwatTurn_End (25 frames) at rate 1.3, entered 146/256 in
    let took = end.time - start;
    let expected = (25.0 / 30.0) / 1.3 * (110.0 / 256.0);
    assert!((took - expected).abs() < DT + 1e-4, "took {took}");
    assert!(
        (c.pos.y - 80.0).abs() < 0.01 && (c.pos.x - 266.0).abs() < 0.01,
        "{:?}",
        c.pos
    );
    assert!(c.mirrored, "lands facing back toward the gap (inferred)");
}

#[test]
fn swat_turn_needs_the_camera_over_the_cover() {
    // camera · facing must be ≥ 0.85; cos(1 rad) = 0.54
    let w = gap_walls(80.0);
    let (c, _) = at_gap_then(&w, 3.0, |t| {
        let mut i = inp(1.0, 0.0, (0.1..0.15).contains(&t));
        i.cam_yaw = 1.0;
        i
    });
    assert!(first(&c, "SwatTurn").is_none(), "{:?}", c.events);
}

#[test]
fn swat_turn_needs_a_target_within_512() {
    // CoverLink.TurnDist = 512: a 600 uu gap is too far
    let w = gap_walls(580.0);
    let (c, _) = at_gap_then(&w, 3.0, |t| inp(1.0, 0.0, (0.1..0.15).contains(&t)));
    assert!(first(&c, "SwatTurn").is_none(), "{:?}", c.events);
}

#[test]
fn swat_turn_needs_the_stick_toward_the_edge() {
    let w = gap_walls(80.0);
    // facing right (not mirrored) but pushing left: no swat
    let (c, _) = at_gap_then(&w, 3.0, |t| inp(-1.0, 0.0, (0.1..0.15).contains(&t)));
    assert!(first(&c, "SwatTurn").is_none(), "{:?}", c.events);
}

#[test]
fn a_held_during_a_swat_turns_into_a_roadie_run_after_0_2s() {
    // GearPC.PlayerTakingCover.TryASpecialMove → TransitionFromSwatToRoadieRun: still
    // > 2 × 34 uu from the target → leave cover, roadie run.
    let w = gap_walls(400.0); // 430 uu dash = 0.84 s
    let (c, _) = at_gap_then(&w, 3.0, |t| {
        inp(
            1.0,
            0.0,
            (0.1..0.15).contains(&t) || (0.2..1.5).contains(&t),
        )
    });
    let swat = first(&c, "SwatTurn").expect("swat");
    let roadie = c
        .events
        .iter()
        .find(|e| e.reason == "swat → roadie (A held)")
        .unwrap_or_else(|| panic!("{:?}", c.events));
    // second press 0.1 s after the first (which started the swat) → + 0.2
    assert!(
        (roadie.time - (swat + 0.3)).abs() < 2.0 * DT + 1e-4,
        "swat {swat}, roadie {}: {:?}",
        roadie.time,
        c.events
    );
}

#[test]
fn swat_asked_for_during_the_slide_starts_when_it_lands() {
    // GSM_Run2Cover.CanChainMove includes StdLvlSwatTurn
    let w = gap_walls(80.0);
    let mut c = Character::new(Config::default(), Vec3::new(0.0, -30.0, 0.0), 0.0);
    while c.time < 2.0 {
        let t = c.time;
        let i = if t < 0.05 {
            inp(0.0, 1.0, true) // tap A: slide (0.39 s)
        } else if t < 0.15 {
            inp(0.0, 1.0, false)
        } else {
            inp(1.0, 0.0, (0.15..0.2).contains(&t)) // A + stick right mid-slide
        };
        c.tick(i, DT, &w);
    }
    let landed = c
        .events
        .iter()
        .find(|e| e.from == "Run2Cover")
        .expect("landed")
        .time;
    let swat = c.events.iter().find(|e| e.to == "SwatTurn").expect("swat");
    assert_eq!(swat.reason, "queued move");
    assert!((swat.time - landed).abs() < 1e-6);
}

#[test]
fn a_long_swat_dashes_at_510_then_hands_over_256_uu_out() {
    // 430 uu gap: 174 uu of dash at 300 × 1.7, then the end anim for the last 256.
    let w = gap_walls(400.0);
    let (c, _) = at_gap_then(&w, 3.0, |t| inp(1.0, 0.0, (0.1..0.15).contains(&t)));
    let start = first(&c, "SwatTurn").expect("swat");
    let end = c
        .events
        .iter()
        .find(|e| e.from == "SwatTurn")
        .expect("ended")
        .time;
    let expected = (430.0 - 256.0) / 510.0 + (25.0 / 30.0) / 1.3;
    assert!(
        (end - start - expected).abs() < DT + 1e-4,
        "took {}",
        end - start
    );
}

#[test]
fn stick_sideways_moves_along_cover_and_stops_at_the_end() {
    // CoverMovementPct[CT_MidLevel] = 1.0 → 300 uu/s along the wall; facing follows the
    // stick (SetMirroredSide); the end of the cover stops you.
    let w = wall_ahead(); // y from -200 to 200, cover spot at x = 266
    let mut c = Character::new(Config::default(), Vec3::ZERO, 0.0);
    let mut landed = None;
    let mut top = 0.0f32;
    while c.time < 3.0 {
        let t = c.time;
        let i = match landed {
            None => inp(0.0, 1.0, t < 0.05),
            Some(l) if t - l < 1.0 => inp(1.0, 0.0, false), // right
            Some(_) => inp(-1.0, 0.0, false),               // then left
        };
        c.tick(i, DT, &w);
        if c.state_name() == "InCover" {
            landed.get_or_insert(c.time);
            top = top.max(c.vel.length_2d());
        }
        if landed
            .map(|l| c.time - l > 0.9 && c.time - l < 0.95)
            .unwrap_or(false)
        {
            assert!(
                (c.pos.y - 200.0).abs() < 0.01,
                "stopped at the right end, y={}",
                c.pos.y
            );
            assert!(!c.mirrored);
        }
    }
    assert!((top - 300.0).abs() < 0.01, "top speed {top}");
    assert_eq!(c.state_name(), "InCover");
    assert!(c.mirrored, "facing left after pushing left");
    assert!(
        (c.pos.y - -200.0).abs() < 0.01,
        "at the left end, y={}",
        c.pos.y
    );
    assert!((c.pos.x - 266.0).abs() < 0.01);
}

#[test]
fn two_sided_walls_give_cover_on_the_back_face() {
    // a 24 uu slab: from behind (x > 324) a tap slides to x = 324 + 34 = 358
    let w = WallWorld::two_sided(wall_ahead().walls, 24.0);
    let mut c = Character::new(
        Config::default(),
        Vec3::new(600.0, 0.0, 0.0),
        std::f32::consts::PI,
    );
    while c.time < 1.5 {
        let t = c.time;
        let mut i = inp(0.0, 1.0, t < 0.05);
        i.cam_yaw = std::f32::consts::PI;
        c.tick(i, DT, &w);
    }
    assert_eq!(c.state_name(), "InCover");
    assert!((c.pos.x - 358.0).abs() < 0.5, "x {}", c.pos.x);
}

// ------------------------------------------------------------------ vault

/// The wall ahead as a 24 uu thick slab: front face x = 300, back face x = 324.
fn slab_ahead(height: CoverHeight) -> WallWorld {
    WallWorld::two_sided(
        vec![Wall {
            a: Vec3::new(300.0, -200.0, 0.0),
            b: Vec3::new(300.0, 200.0, 0.0),
            normal: Vec3::new(-1.0, 0.0, 0.0),
            height,
        }],
        24.0,
    )
}

/// Slide into the slab's cover, then at `at` press A for 0.05 s with the stick
/// forward (toward the wall); `hold_a` keeps A down instead.
fn vault_from_cover(w: &WallWorld, at: f32, end: f32, stick_y: f32, hold_a: bool) -> Character {
    sim(w, Vec3::ZERO, end, |t| {
        let a = t < 0.05 || (t >= at && (hold_a || t < at + 0.05));
        // stick only while asking (the vault ignores it; afterwards it would walk on)
        let stick = if t >= at && t < at + 0.2 {
            stick_y
        } else {
            0.0
        };
        inp(0.0, stick, a)
    })
}

#[test]
fn vault_over_mid_cover_with_gears_timing() {
    // Expected numbers, worked from GEARS_VALUES.md → Vault: cover at x = 266, far face
    // 324 + radius 34 → MantleDistance 92; jump 141.4 × 92/141 forward, up first; fall
    // from 100.8 uu at 158.1 uu/s for sqrt(2 × 100.8 / 750) = 0.518 s; land 10/30 / 1.1.
    let w = slab_ahead(CoverHeight::Mid);
    let c = vault_from_cover(&w, 1.5, 3.5, 1.0, false);
    let start = first(&c, "Vault").expect("vaulted");
    let end = c
        .events
        .iter()
        .find(|e| e.from == "Vault")
        .map(|e| e.time)
        .expect("vault ended");
    let jump = 21.0 / 30.0 / 1.1;
    let fall = (2.0f32 * 100.8 / 750.0).sqrt();
    let land = 10.0 / 30.0 / 1.1;
    assert!(
        ((end - start) - (jump + fall + land)).abs() < 2.0 * DT,
        "vault took {} (expected {})",
        end - start,
        jump + fall + land
    );
    let travel = 141.4 * 92.0 / 141.0;
    let fwd_vel =
        (125.0 * (100.0f32 / 750.0).sqrt() - (travel - 92.0)) / ((100.8 - 39.0f32) / 750.0).sqrt();
    let expect_x = 266.0 + travel + fwd_vel * fall;
    assert!(
        (c.pos.x - expect_x).abs() < 1.0,
        "landed at {} (expected {expect_x})",
        c.pos.x
    );
    assert_eq!(c.state_name(), "Walk");
    assert!(c.cover_spot().is_none(), "left cover");
}

#[test]
fn vault_jump_goes_up_before_it_goes_over() {
    let w = slab_ahead(CoverHeight::Mid);
    // 0.40 s into the jump: still rising, not moved forward yet (frames 0–15 of 21).
    let c = vault_from_cover(&w, 1.5, 1.5 + DT + 0.40, 1.0, false);
    assert_eq!(c.state_name(), "Vault");
    assert!((c.pos.x - 266.0).abs() < 0.5, "x {}", c.pos.x);
    assert!(
        c.height() > 80.0 && c.height() < 100.8,
        "height {}",
        c.height()
    );
}

#[test]
fn vault_needs_the_stick_toward_the_wall() {
    let w = slab_ahead(CoverHeight::Mid);
    let c = vault_from_cover(&w, 1.5, 3.0, 0.0, false);
    assert!(first(&c, "Vault").is_none());
}

#[test]
fn no_vault_over_standing_cover() {
    let w = slab_ahead(CoverHeight::Standing);
    let c = vault_from_cover(&w, 1.5, 3.0, 1.0, false);
    assert!(first(&c, "Vault").is_none());
}

#[test]
fn no_vault_when_the_landing_is_blocked() {
    let mut walls = slab_ahead(CoverHeight::Mid).walls;
    walls.push(Wall {
        a: Vec3::new(400.0, -200.0, 0.0),
        b: Vec3::new(400.0, 200.0, 0.0),
        normal: Vec3::new(-1.0, 0.0, 0.0),
        height: CoverHeight::Standing,
    });
    let w = WallWorld::new(walls);
    let c = vault_from_cover(&w, 1.5, 3.0, 1.0, false);
    assert!(first(&c, "Vault").is_none());
}

#[test]
fn a_held_through_a_vault_roadie_runs_0_09s_after_landing() {
    let w = slab_ahead(CoverHeight::Mid);
    let c = vault_from_cover(&w, 1.5, 3.5, 1.0, true);
    let start = first(&c, "Vault").expect("vaulted");
    let roadie = first(&c, "RoadieRun").expect("roadie run");
    let landed = 21.0 / 30.0 / 1.1 + (2.0f32 * 100.8 / 750.0).sqrt();
    let expect = landed + 0.1 / 1.1;
    assert!(
        ((roadie - start) - expect).abs() < 2.0 * DT,
        "roadie {} after the vault (expected {expect})",
        roadie - start
    );
}

#[test]
fn vault_asked_for_during_the_slide_starts_when_it_lands() {
    let w = slab_ahead(CoverHeight::Mid);
    let c = sim(&w, Vec3::ZERO, 2.5, |t| {
        inp(0.0, 1.0, t < 0.05 || (0.36..0.40).contains(&t))
    });
    let slide_end = c
        .events
        .iter()
        .find(|e| e.from == "Run2Cover")
        .map(|e| e.time)
        .expect("slide ended");
    let vault = c
        .events
        .iter()
        .find(|e| e.to == "Vault")
        .expect("vault started");
    assert_eq!(vault.reason, "queued move");
    assert!((vault.time - slide_end).abs() < 1e-4);
}

#[test]
fn vault_too_early_in_the_slide_is_refused() {
    // More than 96 uu from the slot: GSM_MantleOverCoverBase refuses it.
    let w = slab_ahead(CoverHeight::Mid);
    let c = sim(&w, Vec3::ZERO, 2.5, |t| {
        inp(0.0, 1.0, t < 0.05 || (0.10..0.14).contains(&t))
    });
    assert!(first(&c, "Vault").is_none());
}

// ------------------------------------------------------------------ aiming

#[test]
fn aiming_walks_at_half_speed_and_faces_the_camera() {
    // Pawn.WalkingPct 0.5 with GearWeapon.bForceWalkWhenTargeting: 150 uu/s, strafing.
    let c = sim(&EmptyWorld, Vec3::ZERO, 1.0, |_| Input {
        stick_x: 1.0,
        cam_yaw: 0.3,
        aim: true,
        ..Default::default()
    });
    assert!(
        (c.vel.length_2d() - 150.0).abs() < 0.01,
        "speed {}",
        c.vel.length_2d()
    );
    assert!((c.yaw - 0.3).abs() < 1e-5, "yaw {}", c.yaw);
}

#[test]
fn no_roadie_run_while_aiming() {
    let c = sim(&EmptyWorld, Vec3::ZERO, 1.0, |_| Input {
        stick_y: 1.0,
        a: true,
        aim: true,
        ..Default::default()
    });
    assert!(first(&c, "RoadieRun").is_none());
}
