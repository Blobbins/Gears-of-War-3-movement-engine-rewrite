use gears_play::level::LevelId;
use gears_play::session::{Cue, Game, PadInput};

fn run(g: &mut Game, frames: usize, f: impl Fn(usize) -> PadInput) -> Vec<Cue> {
    let mut cues = Vec::new();
    for i in 0..frames {
        g.step(f(i));
        cues.extend(g.drain_cues());
    }
    cues
}
fn played(c: &[Cue], name: &str) -> bool {
    c.iter()
        .any(|c| matches!(c, Cue::Play { name: n, .. } | Cue::Loop { name: n, .. } if n == name))
}
/// point the camera at target `i`'s chest
fn aim_at(g: &mut Game, i: usize) {
    let (t, c) = (&g.targets[i], g.cam.pos);
    g.cam.yaw = (t.y - c.y).atan2(t.x - c.x);
    g.cam.pitch = (100.0 - c.z).atan2(((t.x - c.x).powi(2) + (t.y - c.y).powi(2)).sqrt());
}

#[test]
fn slide_into_cover_with_sounds_and_cover_camera() {
    let mut g = Game::new(LevelId::Lane);
    let cues = run(&mut g, 120, |i| PadInput {
        stick_y: 1.0,
        a: i < 3,
        ..Default::default()
    });
    assert_eq!(g.ch.state_name(), "InCover");
    assert!(played(&cues, "Footsteps_Marcus_ConcreteSlide01"));
    assert!(played(&cues, "CogBodyCoverLowIn01") || played(&cues, "CogBodyCoverLowIn03"));
    assert_eq!(g.frame().cam_mode, "GameplayCam_Cover");
}

#[test]
fn lancer_hits_for_51_and_loops_its_sound() {
    let mut g = Game::new(LevelId::Lane);
    // ~900 uu out: between the 750 short and 2048 medium bands, both × 1.0 (closer in,
    // ModifyDamage blends from the 200 uu melee × 1.3)
    g.ch.pos = gears_core::Vec3::new(700.0, -150.0, 0.0);
    g.targets[0].x = 1600.0;
    run(&mut g, 30, |_| PadInput {
        aim: true,
        ..Default::default()
    });
    let mut cues = Vec::new();
    for _ in 0..60 {
        aim_at(&mut g, 0);
        g.cam.place(&g.ch, &g.level);
        g.step(PadInput {
            aim: true,
            fire: true,
            ..Default::default()
        });
        cues.extend(g.drain_cues());
    }
    let hp = g.targets[0].hp;
    assert!(
        hp < 1000.0 && ((1000.0 - hp) % 51.0).abs() < 1e-3,
        "hp {hp}"
    );
    assert!(cues.iter().any(|c| matches!(c, Cue::Loop { .. })));
    g.step(PadInput {
        aim: true,
        ..Default::default()
    });
    let c = g.drain_cues();
    assert!(
        c.iter().any(|c| matches!(c, Cue::StopLoop { .. }))
            && played(&c, "CogARifleFireStopPlayer01")
    );
}

#[test]
fn gnasher_point_blank_is_600_then_pumps() {
    let mut g = Game::new(LevelId::Lane);
    g.ch.pos = gears_core::Vec3::new(720.0, -150.0, 0.0);
    run(&mut g, 2, |_| PadInput {
        swap: true,
        ..Default::default()
    });
    run(&mut g, 40, |_| PadInput {
        aim: true,
        ..Default::default()
    });
    aim_at(&mut g, 0);
    g.cam.place(&g.ch, &g.level);
    let mut cues = run(&mut g, 1, |_| PadInput {
        aim: true,
        fire: true,
        ..Default::default()
    });
    cues.extend(run(&mut g, 70, |_| PadInput {
        aim: true,
        ..Default::default()
    }));
    assert_eq!(
        g.targets[0].hp, 400.0,
        "one Gnasher shot from ~180 uu = 500 × 1.2"
    );
    assert!(
        played(&cues, "shotguncock01") && played(&cues, "ShotgunAmmoBounce01")
            || played(&cues, "ShotgunAmmoBounce02")
    );
}

#[test]
fn with_your_export_the_pose_and_gun_anims_run() {
    let p =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../_local/exports/marcus/pack");
    if !p.join("marcus.glb").exists() {
        return;
    }
    let rd = |n: &str| std::fs::read(p.join(n)).unwrap();
    let mut g = Game::new(LevelId::Lane);
    let aim = std::fs::read_to_string(p.join("marcus_aim.json")).ok();
    g.load_assets(
        &rd("marcus.glb"),
        aim.as_deref(),
        Some(&rd("lancer.glb")),
        Some(&rd("gnasher.glb")),
    )
    .unwrap();
    run(&mut g, 60, |_| PadInput {
        stick_y: 1.0,
        ..Default::default()
    });
    let f = g.frame();
    assert_eq!(f.pose.as_ref().unwrap().local.len(), 129);
    assert!(f.gun_pose.is_some());
    let rig = f.rig.as_ref().expect("standard rig pose");
    assert_eq!(rig.pos.len(), gears_play::rig::standard_rig().len());
    // running is hunched (his head ~124 uu up, 35 ahead); ours tracks it
    assert!(
        rig.pos[4].y > rig.pos[0].y + 30.0,
        "head up: {:?}",
        rig.pos[4]
    );
    // footsteps while running
    let cues = run(&mut g, 120, |_| PadInput {
        stick_y: 1.0,
        ..Default::default()
    });
    assert!(cues.iter().filter(|c| matches!(c, Cue::Play { name, .. } if name.starts_with("Footsteps_Marcus_Concrete0"))).count() >= 4);
}
