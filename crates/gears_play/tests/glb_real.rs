//! Runs only when the owner's export is present (it is never in the repo).
use gears_play::glb::Model;

fn pack() -> Option<std::path::PathBuf> {
    let p =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../_local/exports/marcus/pack");
    p.join("marcus.glb").exists().then_some(p)
}

#[test]
fn reads_marcus_and_guns() {
    let Some(p) = pack() else { return };
    let m = Model::parse(&std::fs::read(p.join("marcus.glb")).unwrap()).unwrap();
    assert!(m.node("b_MF_Weapon_R_End").is_some());
    assert!(m.clip("Roadie_Run_Core").is_some());
    let jump = m.clip("AR_Mantle_Start").unwrap();
    assert_eq!(jump.root_curve.as_ref().unwrap().len(), 21);
    println!("marcus: {} nodes, {} clips", m.nodes.len(), m.clips.len());
    let l = Model::parse(&std::fs::read(p.join("lancer.glb")).unwrap()).unwrap();
    assert!(l.clip("AR_Fire").is_some());
}

/// Prints model-space rotations of a few bones for parity with the browser page.
#[test]
fn pose_dump_for_parity() {
    use gears_core::{Character, Config, EmptyWorld, Input, Vec3};
    use gears_play::anim::{AnimCtx, Animator, WeaponAnim};
    let Some(p) = pack() else { return };
    let m = Model::parse(&std::fs::read(p.join("marcus.glb")).unwrap()).unwrap();
    let mut an = Animator::new(m);
    an.load_aim(&std::fs::read_to_string(p.join("marcus_aim.json")).unwrap())
        .unwrap();
    let mut ch = Character::new(Config::default(), Vec3::ZERO, 0.0);
    for (label, aiming, pitch) in [("idle", false, 0.0f32), ("aim", true, 0.4)] {
        let mut pose = None;
        for _ in 0..120 {
            ch.tick(
                Input {
                    aim: aiming,
                    ..Default::default()
                },
                1.0 / 60.0,
                &EmptyWorld,
            );
            let cx = AnimCtx {
                ch: &ch,
                aiming,
                cam_pitch: pitch,
                cam_yaw: 0.0,
                stick_x: 0.0,
                weapon: WeaponAnim::default(),
            };
            pose = Some(an.update(&cx, 1.0 / 60.0));
        }
        let ms = an.model.model_space(&pose.unwrap().local);
        for b in [
            "b_MF_Spine_03",
            "b_MF_Weapon_R_End",
            "b_MF_Hand_L",
            "b_MF_Head",
        ] {
            let r = ms[an.model.node(b).unwrap()].r;
            println!(
                "PARITY {label} {b} {:.4} {:.4} {:.4} {:.4}",
                r.x, r.y, r.z, r.w
            );
        }
    }
}
