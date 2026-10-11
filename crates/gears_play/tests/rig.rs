use gears_play::glb::Model;
use gears_play::qmath::V3;
use gears_play::rig::{standard_rig, two_bone, Retargeter};

#[test]
fn rig_is_ours_and_156_tall() {
    let r = standard_rig();
    assert!(r.iter().all(|b| !b.name.contains("MF")));
    let top = r.iter().map(|b| b.rest.y).fold(0.0, f32::max);
    assert!((top - 165.0).abs() < 1e-3 && r[4].rest.y < 156.0);
}

#[test]
fn two_bone_keeps_lengths() {
    let root = V3::new(0.0, 0.0, 0.0);
    let (m, e) = two_bone(
        root,
        V3::new(30.0, -10.0, 5.0),
        26.0,
        23.0,
        V3::new(0.0, 0.0, 1.0),
    );
    assert!((m.sub(root).len() - 26.0).abs() < 1e-3 && (e.sub(m).len() - 23.0).abs() < 1e-3);
    assert!((e.sub(V3::new(30.0, -10.0, 5.0))).len() < 1e-3 && m.z > 0.0);
}

/// with your own export: our rig follows Marcus without stretching
#[test]
fn retarget_follows_marcus() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../_local/exports/marcus/pack/marcus.glb");
    let Ok(b) = std::fs::read(&p) else { return };
    let m = Model::parse(&b).unwrap();
    let rt = Retargeter::for_marcus(&m).unwrap();
    let bind = rt.retarget(&m, &m.bind_pose());
    // bind (A-pose): our hands hang below our shoulders like his
    assert!(bind.pos[14].y < bind.pos[12].y - 20.0, "{:?}", bind.pos[14]);
    let clip = m
        .clips
        .iter()
        .find(|c| c.name.contains("Run"))
        .unwrap_or(&m.clips[0]);
    let n = rt.bones.len();
    for k in 0..20 {
        let t = clip.duration * k as f32 / 20.0;
        let mut local = m.bind_pose();
        for tr in &clip.tracks {
            match tr.path {
                gears_play::glb::Path::Rotation => local[tr.node].r = tr.rot(t),
                gears_play::glb::Path::Translation => local[tr.node].t = tr.pos(t),
            }
        }
        let pose = rt.retarget(&m, &local);
        let ms = m.model_space(&local);
        for i in 1..n {
            let par = rt.bones[i].parent.unwrap();
            let l0 = rt.bones[i].rest.sub(rt.bones[par].rest).len();
            assert!(
                (pose.pos[i].sub(pose.pos[par]).len() - l0).abs() < 0.05,
                "bone {i} stretched"
            );
        }
        // feet go where his go (direction from pelvis, within 15°)
        for (ours, his) in [(18, "b_MF_Foot_L"), (22, "b_MF_Foot_R")] {
            let a = pose.pos[ours].sub(pose.pos[0]);
            let h = m.node(his).unwrap();
            let bb = ms[h].t.sub(ms[m.node("b_MF_Pelvis").unwrap()].t);
            let c = a.dot(bb) / (a.len() * bb.len());
            assert!(c > 0.966, "{} at t {t}: cos {c}", rt.bones[ours].name);
        }
    }
}
