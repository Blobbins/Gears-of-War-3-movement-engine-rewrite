//! libgears desktop range (Bevy 0.15): Gears of War 3 movement, animation and shooting.
//!
//! All the rules live in `gears_core` (movement, weapons) and `gears_play` (camera,
//! animation driver, combat, sound cues). This app only reads input, draws what
//! `gears_play` says, and plays the sounds it names.
//!
//! Your own export (never in the repo) is read from `GEARS_PACK` (default
//! `../_local/exports/marcus/pack`): marcus.glb, lancer.glb, gnasher.glb,
//! gears_sounds.json, marcus_aim.json. Without it you get a stand-in capsule.
//!
//! Controls: left stick / WASD move, right stick / mouse look (click to capture, Esc
//! frees), A / Space, LT / right mouse aim, RT / left mouse fire, RB / E reload (again in
//! the zone = active reload), Y / Q swap gun, 1 2 3 change range, R reset.

use bevy::audio::{PlaybackMode, Volume};
use bevy::input::mouse::MouseMotion;
use bevy::pbr::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use gears_core::Vec3 as GV;
use gears_play::level::{Level, LevelId, TARGET_H, TARGET_R};
use gears_play::qmath::Trs;
use gears_play::session::{Cue, Game, PadInput, WeaponKind2, DT};
use std::collections::HashMap;
use std::path::PathBuf;

/// Unreal (X forward, Y right, Z up) → Bevy (Y up, right-handed): same mapping as the web range.
fn to_b(v: GV) -> Vec3 {
    Vec3::new(v.y, v.z, -v.x)
}

fn pack_dir() -> PathBuf {
    let p = std::env::var("GEARS_PACK")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../_local/exports/marcus/pack")
        });
    std::fs::canonicalize(&p).unwrap_or(p)
}

fn main() {
    let pack = pack_dir();
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "libgears range".into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: pack.to_string_lossy().into_owned(),
                    ..default()
                }),
        )
        .insert_resource(ClearColor(Color::srgb(0.08, 0.09, 0.11)))
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 350.0,
        })
        .insert_resource(Pack(pack))
        .insert_resource(RigView(true))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                toggle_rig,
                grab_mouse,
                step_game,
                find_joints,
                draw,
                draw_rig,
            )
                .chain(),
        )
        .run();
}

#[derive(Resource)]
struct Pack(PathBuf);

/// Draw the animated skeleton as lines (no mesh). M toggles the full model.
#[derive(Resource)]
struct RigView(bool);

#[derive(Resource)]
struct Sim {
    game: Game,
    acc: f32,
    has_assets: bool,
    sounds: HashMap<String, Handle<AudioSource>>,
    loops: HashMap<u32, Entity>,
    level_entities: Vec<Entity>,
}

/// Joint entities by node name, once the glTF scenes have spawned.
#[derive(Resource, Default)]
struct Joints {
    marcus: Vec<Option<Entity>>,
    lancer: Vec<Option<Entity>>,
    gnasher: Vec<Option<Entity>>,
    socket_done: bool,
}

#[derive(Component)]
struct PawnRoot;
#[derive(Component)]
struct Wrap;
#[derive(Component)]
struct RigRoot;
#[derive(Component)]
struct RigSeg(usize);
#[derive(Component)]
struct RigGun;
#[derive(Component)]
struct StandIn;
#[derive(Component)]
struct GunRoot(WeaponKind2);
#[derive(Component)]
struct TargetMesh(usize);
#[derive(Component)]
struct Flash;
#[derive(Component)]
enum Hud {
    State,
    Ammo,
    Msg,
    Cross,
    Help,
}
#[derive(Component)]
enum Bar {
    Good,
    Perfect,
    Mark,
    Fill,
}
#[derive(Resource)]
struct Mats {
    wall: Handle<StandardMaterial>,
    edge: Handle<StandardMaterial>,
    target: Handle<StandardMaterial>,
}
#[derive(Resource)]
struct Scenes {
    marcus: Option<Handle<Scene>>,
    lancer: Option<Handle<Scene>>,
    gnasher: Option<Handle<Scene>>,
}

fn base64(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut buf, mut bits) = (0u32, 0);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        } as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    out
}

fn setup(
    mut commands: Commands,
    pack: Res<Pack>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut audio: ResMut<Assets<AudioSource>>,
    asset_server: Res<AssetServer>,
    mut gizmo_store: ResMut<GizmoConfigStore>,
) {
    // rig lines: thick, and drawn over the model
    {
        let (cfg, _) = gizmo_store.config_mut::<DefaultGizmoConfigGroup>();
        cfg.line_width = 4.0;
        cfg.depth_bias = -0.5;
    }
    let mut game = Game::new(LevelId::Lane);
    let rd = |n: &str| std::fs::read(pack.0.join(n)).ok();
    let marcus = rd("marcus.glb");
    let mut has_assets = false;
    if let Some(m) = &marcus {
        let aim = std::fs::read_to_string(pack.0.join("marcus_aim.json")).ok();
        match game.load_assets(
            m,
            aim.as_deref(),
            rd("lancer.glb").as_deref(),
            rd("gnasher.glb").as_deref(),
        ) {
            Ok(()) => has_assets = true,
            Err(e) => eprintln!("[libgears] couldn't read your export: {e}"),
        }
    } else {
        eprintln!(
            "[libgears] no export at {} (set GEARS_PACK): using a stand-in",
            pack.0.display()
        );
    }
    // sounds: gears_sounds.json = {"sounds": {name: base64 mp3}}
    let mut sounds = HashMap::new();
    if let Ok(t) = std::fs::read_to_string(pack.0.join("gears_sounds.json")) {
        if let Ok(j) = gears_play::json::Json::parse(&t) {
            for (k, v) in j.get("sounds").and_then(|s| s.obj()).into_iter().flatten() {
                if let Some(b) = v.str() {
                    sounds.insert(
                        k.clone(),
                        audio.add(AudioSource {
                            bytes: base64(b).into(),
                        }),
                    );
                }
            }
        }
    }
    eprintln!(
        "[libgears] assets: model {has_assets}, {} sounds",
        sounds.len()
    );

    // camera, light, floor
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            near: 2.0,
            far: 60000.0,
            ..default()
        }),
        Transform::from_xyz(0.0, 200.0, 400.0),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 9000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(-500.0, 1600.0, 900.0).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 800.0,
            maximum_distance: 6000.0,
            ..default()
        }
        .build(),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(8000.0, 8000.0))),
        MeshMaterial3d(mats.add(StandardMaterial {
            base_color: Color::srgb(0.13, 0.14, 0.16),
            perceptual_roughness: 0.95,
            ..default()
        })),
    ));
    let m = Mats {
        wall: mats.add(StandardMaterial {
            base_color: Color::srgb(0.48, 0.5, 0.53),
            perceptual_roughness: 0.9,
            ..default()
        }),
        edge: mats.add(StandardMaterial {
            base_color: Color::srgb(0.91, 0.71, 0.3),
            unlit: true,
            ..default()
        }),
        target: mats.add(StandardMaterial {
            base_color: Color::srgb(0.48, 0.37, 0.31),
            perceptual_roughness: 0.8,
            ..default()
        }),
    };

    // the pawn: root (position, yaw) → wrap (glTF faces +Z, metres → uu, mirror) → scene
    let scenes = Scenes {
        marcus: has_assets
            .then(|| asset_server.load(GltfAssetLabel::Scene(0).from_asset("marcus.glb"))),
        lancer: (has_assets && pack.0.join("lancer.glb").exists())
            .then(|| asset_server.load(GltfAssetLabel::Scene(0).from_asset("lancer.glb"))),
        gnasher: (has_assets && pack.0.join("gnasher.glb").exists())
            .then(|| asset_server.load(GltfAssetLabel::Scene(0).from_asset("gnasher.glb"))),
    };
    let root = commands
        .spawn((PawnRoot, Transform::default(), Visibility::default()))
        .id();
    if let Some(h) = &scenes.marcus {
        let wrap = commands
            .spawn((
                Wrap,
                Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI))
                    .with_scale(Vec3::splat(100.0)),
                Visibility::default(),
            ))
            .id();
        let scene = commands.spawn(SceneRoot(h.clone())).id();
        commands.entity(wrap).add_child(scene);
        commands.entity(root).add_child(wrap);
        // the libgears standard rig (ours, shareable): one capsule per bone, driven by
        // the retargeted pose; same frame as the wrap but already in uu
        let rig_root = commands
            .spawn((
                RigRoot,
                Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
                Visibility::default(),
            ))
            .id();
        commands.entity(root).add_child(rig_root);
        let bones = gears_play::rig::standard_rig();
        let mut mat = |c: Color| {
            mats.add(StandardMaterial {
                base_color: c,
                perceptual_roughness: 0.6,
                ..default()
            })
        };
        let (mc, ml, mr) = (
            mat(Color::srgb(0.9, 0.88, 0.84)),
            mat(Color::srgb(0.43, 0.7, 0.78)),
            mat(Color::srgb(0.94, 0.49, 0.29)),
        );
        for (i, bn) in bones.iter().enumerate() {
            let Some(par) = bn.parent else { continue };
            let r = bones[par].radius.max(2.5);
            let len = bn.rest.sub(bones[par].rest).len();
            let m = if bn.name.ends_with("_l") || bones[par].name.ends_with("_l") {
                ml.clone()
            } else if bn.name.ends_with("_r") || bones[par].name.ends_with("_r") {
                mr.clone()
            } else {
                mc.clone()
            };
            let seg = commands
                .spawn((
                    RigSeg(i),
                    Mesh3d(meshes.add(Capsule3d::new(r, (len - r).max(0.5)))),
                    MeshMaterial3d(m),
                    Transform::default(),
                ))
                .id();
            commands.entity(rig_root).add_child(seg);
        }
        let gun = commands
            .spawn((
                RigGun,
                Mesh3d(meshes.add(Cuboid::new(3.0, 3.0, 1.0))),
                MeshMaterial3d(mat(Color::srgb(0.91, 0.71, 0.3))),
                Transform::default(),
            ))
            .id();
        commands.entity(rig_root).add_child(gun);
    } else {
        let capsule = commands
            .spawn((
                StandIn,
                Mesh3d(meshes.add(Capsule3d::new(34.0, 88.0))),
                MeshMaterial3d(mats.add(StandardMaterial {
                    base_color: Color::srgb(0.43, 0.7, 0.78),
                    ..default()
                })),
                Transform::from_xyz(0.0, 78.0, 0.0),
            ))
            .id();
        let nose = commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::new(10.0, 10.0, 40.0))),
                MeshMaterial3d(m.edge.clone()),
                Transform::from_xyz(0.0, 40.0, -40.0),
            ))
            .id();
        commands.entity(capsule).add_child(nose);
        commands.entity(root).add_child(capsule);
    }
    // muzzle flash (a small bright sphere)
    commands.spawn((
        Flash,
        Mesh3d(meshes.add(Sphere::new(9.0))),
        MeshMaterial3d(mats.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.5),
            emissive: LinearRgba::rgb(40.0, 25.0, 8.0),
            unlit: true,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
    ));

    // HUD
    let text = |c: &mut Commands, hud: Hud, node: Node, size: f32, s: &str| {
        c.spawn((
            Text::new(s),
            TextFont {
                font_size: size,
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.88, 0.84)),
            node,
            hud,
        ));
    };
    text(
        &mut commands,
        Hud::State,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            top: Val::Px(12.0),
            ..default()
        },
        26.0,
        "Walk",
    );
    text(&mut commands, Hud::Help, Node { position_type: PositionType::Absolute, left: Val::Px(16.0), bottom: Val::Px(12.0), ..default() }, 13.0, "WASD / stick move · mouse / right stick look (click to capture) · Space / A · RMB / LT aim · LMB / RT fire · E / RB reload · Q / Y swap · M rig / model · 1 2 3 range · R reset");
    text(
        &mut commands,
        Hud::Ammo,
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(16.0),
            bottom: Val::Px(54.0),
            ..default()
        },
        24.0,
        "",
    );
    text(
        &mut commands,
        Hud::Msg,
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(16.0),
            bottom: Val::Px(30.0),
            ..default()
        },
        16.0,
        "",
    );
    text(
        &mut commands,
        Hud::Cross,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(50.0),
            margin: UiRect {
                left: Val::Px(-7.0),
                top: Val::Px(-14.0),
                ..default()
            },
            ..default()
        },
        26.0,
        "+",
    );
    // reload bar: a track with the good / perfect zones, a fill and a marker
    let track = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(16.0),
                bottom: Val::Px(16.0),
                width: Val::Px(220.0),
                height: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.2, 0.23, 0.26, 0.9)),
        ))
        .id();
    for (bar, col) in [
        (Bar::Fill, Color::srgb(0.91, 0.71, 0.3)),
        (Bar::Good, Color::srgba(0.9, 0.88, 0.84, 0.55)),
        (Bar::Perfect, Color::srgb(0.95, 0.95, 0.95)),
        (Bar::Mark, Color::srgb(0.94, 0.49, 0.29)),
    ] {
        let e = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(0.0),
                    height: Val::Percent(100.0),
                    width: Val::Px(0.0),
                    ..default()
                },
                BackgroundColor(col),
                bar,
            ))
            .id();
        commands.entity(track).add_child(e);
    }

    let mut sim = Sim {
        game,
        acc: 0.0,
        has_assets,
        sounds,
        loops: HashMap::new(),
        level_entities: Vec::new(),
    };
    build_level(&mut commands, &mut sim, &mut meshes, &m);
    commands.insert_resource(m);
    commands.insert_resource(scenes);
    commands.insert_resource(sim);
    commands.insert_resource(Joints::default());
}

fn build_level(commands: &mut Commands, sim: &mut Sim, meshes: &mut Assets<Mesh>, m: &Mats) {
    for e in sim.level_entities.drain(..) {
        commands.entity(e).despawn_recursive();
    }
    let level: &Level = &sim.game.level;
    for w in &level.walls {
        let h = Level::wall_height(w);
        let t = gears_play::level::WALL_T;
        let (a, b, n) = (w.a, w.b, w.normal.normalized());
        let len = (b - a).length();
        // the slab sits behind the cover face (normal points toward the character)
        let centre = (a + b) * 0.5 - n * (t * 0.5);
        let yaw = (b.y - a.y).atan2(b.x - a.x);
        // the cuboid's length is its Z; rotating it by −yaw lays it along the wall
        let e = commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::new(t, h, len))),
                MeshMaterial3d(m.wall.clone()),
                Transform::from_translation(to_b(GV::new(centre.x, centre.y, h * 0.5)))
                    .with_rotation(Quat::from_rotation_y(-yaw)),
            ))
            .id();
        let edge = commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::new(t + 2.0, 3.0, len + 2.0))),
                MeshMaterial3d(m.edge.clone()),
                Transform::from_xyz(0.0, h * 0.5, 0.0),
            ))
            .id();
        commands.entity(e).add_child(edge);
        sim.level_entities.push(e);
    }
    for (i, &(x, y)) in level.targets.iter().enumerate() {
        let e = commands
            .spawn((
                TargetMesh(i),
                Mesh3d(meshes.add(Cylinder::new(TARGET_R, TARGET_H))),
                MeshMaterial3d(m.target.clone()),
                Transform::from_translation(to_b(GV::new(x, y, TARGET_H * 0.5))),
            ))
            .id();
        sim.level_entities.push(e);
    }
}

/// Pose the standard rig's capsules from the retargeted pose (M toggles rig / Marcus).
fn draw_rig(
    sim: Res<Sim>,
    rig: Res<RigView>,
    mut root: Query<&mut Visibility, With<RigRoot>>,
    mut segs: Query<(&RigSeg, &mut Transform), Without<RigGun>>,
    mut gun: Query<&mut Transform, With<RigGun>>,
) {
    let f = sim.game.frame();
    let Some(pose) = f.rig.as_ref().filter(|_| rig.0) else {
        if let Ok(mut v) = root.get_single_mut() {
            *v = Visibility::Hidden;
        }
        return;
    };
    if let Ok(mut v) = root.get_single_mut() {
        *v = Visibility::Inherited;
    }
    let v = |p: gears_play::qmath::V3| Vec3::new(p.x, p.y, p.z);
    let bones = gears_play::rig::standard_rig();
    for (seg, mut t) in &mut segs {
        let Some(par) = bones[seg.0].parent else {
            continue;
        };
        let (a, b) = (v(pose.pos[par]), v(pose.pos[seg.0]));
        let d = b - a;
        if d.length_squared() > 1e-6 {
            t.translation = (a + b) * 0.5;
            t.rotation = Quat::from_rotation_arc(Vec3::Y, d.normalize());
        }
    }
    if let Ok(mut t) = gun.get_single_mut() {
        let len = if f.gun == WeaponKind2::Lancer {
            75.0
        } else {
            70.0
        };
        let dir = v(pose.gun_dir);
        t.translation = v(pose.gun_at) + dir * (len * 0.5);
        t.rotation = Quat::from_rotation_arc(Vec3::Z, dir);
        t.scale = Vec3::new(1.0, 1.0, len);
    }
}

fn toggle_rig(keys: Res<ButtonInput<KeyCode>>, mut rig: ResMut<RigView>) {
    if keys.just_pressed(KeyCode::KeyM) {
        rig.0 = !rig.0;
    }
}

fn grab_mouse(
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let Ok(mut w) = windows.get_single_mut() else {
        return;
    };
    if mouse.just_pressed(MouseButton::Left) && w.cursor_options.grab_mode == CursorGrabMode::None {
        w.cursor_options.grab_mode = CursorGrabMode::Locked;
        w.cursor_options.visible = false;
    }
    if keys.just_pressed(KeyCode::Escape) {
        w.cursor_options.grab_mode = CursorGrabMode::None;
        w.cursor_options.visible = true;
    }
}

#[allow(clippy::too_many_arguments)]
fn step_game(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut motion: EventReader<MouseMotion>,
    pads: Query<&Gamepad>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut sim: ResMut<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    mats: Res<Mats>,
) {
    let captured = windows
        .get_single()
        .map(|w| w.cursor_options.grab_mode != CursorGrabMode::None)
        .unwrap_or(false);
    let mut md = Vec2::ZERO;
    for e in motion.read() {
        md += e.delta;
    }
    if !captured {
        md = Vec2::ZERO;
    }
    // level / reset
    let lvl = if keys.just_pressed(KeyCode::Digit1) {
        Some(LevelId::Lane)
    } else if keys.just_pressed(KeyCode::Digit2) {
        Some(LevelId::Edges)
    } else if keys.just_pressed(KeyCode::Digit3) {
        Some(LevelId::Yard)
    } else if keys.just_pressed(KeyCode::KeyR) {
        Some(sim.game.level.id)
    } else {
        None
    };
    if let Some(id) = lvl {
        for (_, e) in sim.loops.drain() {
            commands.entity(e).despawn();
        }
        sim.game.load_level(id);
        build_level(&mut commands, &mut sim, &mut meshes, &mats);
    }

    let mut pad = PadInput::default();
    let k = |c: KeyCode| keys.pressed(c);
    let (mut x, mut y) = (0.0f32, 0.0f32);
    if k(KeyCode::KeyW) {
        y += 1.0;
    }
    if k(KeyCode::KeyS) {
        y -= 1.0;
    }
    if k(KeyCode::KeyD) {
        x += 1.0;
    }
    if k(KeyCode::KeyA) {
        x -= 1.0;
    }
    let l = (x * x + y * y).sqrt();
    if l > 1.0 {
        x /= l;
        y /= l;
    }
    pad.stick_x = x;
    pad.stick_y = y;
    pad.look_x = (k(KeyCode::ArrowRight) as i32 - k(KeyCode::ArrowLeft) as i32) as f32;
    pad.look_y = (k(KeyCode::ArrowUp) as i32 - k(KeyCode::ArrowDown) as i32) as f32;
    pad.a = k(KeyCode::Space);
    pad.aim = mouse.pressed(MouseButton::Right);
    pad.fire = captured && mouse.pressed(MouseButton::Left);
    pad.reload = k(KeyCode::KeyE);
    pad.swap = k(KeyCode::KeyQ);
    if let Some(gp) = pads.iter().next() {
        let ls = gp.left_stick();
        if ls.length() > 0.05 {
            pad.stick_x = ls.x.clamp(-1.0, 1.0);
            pad.stick_y = ls.y.clamp(-1.0, 1.0);
        }
        let rs = gp.right_stick();
        let dz = |v: f32| {
            if v.abs() < 0.12 {
                0.0
            } else {
                (v - v.signum() * 0.12) / 0.88
            }
        };
        if rs.length() > 0.12 {
            pad.look_x = dz(rs.x);
            pad.look_y = dz(rs.y);
        }
        pad.a |= gp.pressed(GamepadButton::South);
        pad.aim |= gp.get(GamepadButton::LeftTrigger2).unwrap_or(0.0) > 0.4;
        pad.fire |= gp.get(GamepadButton::RightTrigger2).unwrap_or(0.0) > 0.4;
        pad.reload |= gp.pressed(GamepadButton::RightTrigger);
        pad.swap |= gp.pressed(GamepadButton::North);
    }

    // fixed 60 Hz steps; mouse movement goes to the first step
    sim.acc += time.delta_secs().min(0.1);
    let mut first = true;
    while sim.acc >= DT {
        let mut p = pad;
        if first {
            p.mouse_dx = md.x;
            p.mouse_dy = md.y;
            first = false;
        }
        sim.game.step(p);
        sim.acc -= DT;
        for cue in sim.game.drain_cues() {
            match cue {
                Cue::Play { name, volume } => {
                    if let Some(h) = sim.sounds.get(&name) {
                        commands.spawn((
                            AudioPlayer::new(h.clone()),
                            PlaybackSettings {
                                mode: PlaybackMode::Despawn,
                                volume: Volume::new(volume * 0.6),
                                ..default()
                            },
                        ));
                    }
                }
                Cue::Loop { id, name, volume } => {
                    if let Some(h) = sim.sounds.get(&name) {
                        let e = commands
                            .spawn((
                                AudioPlayer::new(h.clone()),
                                PlaybackSettings {
                                    mode: PlaybackMode::Loop,
                                    volume: Volume::new(volume * 0.6),
                                    ..default()
                                },
                            ))
                            .id();
                        sim.loops.insert(id, e);
                    }
                }
                Cue::StopLoop { id } => {
                    if let Some(e) = sim.loops.remove(&id) {
                        commands.entity(e).despawn();
                    }
                }
            }
        }
    }
}

/// Once the glTF scenes are in the world: map node names → entities, put the guns in
/// Marcus's hand (GearPawn RightHand socket on b_MF_Weapon_R_End, +1.48 uu; the Lancer's
/// barrel is on its own +Z so it turns 90° about Y).
fn find_joints(
    mut commands: Commands,
    mut joints: ResMut<Joints>,
    sim: Res<Sim>,
    scenes: Res<Scenes>,
    names: Query<(Entity, &Name)>,
) {
    if !sim.has_assets || joints.socket_done {
        return;
    }
    let by_name: HashMap<&str, Entity> = names.iter().map(|(e, n)| (n.as_str(), e)).collect();
    let Some(anim) = sim.game.anim.as_ref() else {
        return;
    };
    let map = |nodes: &[gears_play::glb::Node]| {
        nodes
            .iter()
            .map(|n| by_name.get(n.name.as_str()).copied())
            .collect::<Vec<_>>()
    };
    let Some(&socket) = by_name.get("b_MF_Weapon_R_End") else {
        return;
    };
    joints.marcus = map(&anim.model.nodes);
    for (kind, scene, rot) in [
        (
            WeaponKind2::Lancer,
            &scenes.lancer,
            std::f32::consts::FRAC_PI_2,
        ),
        (WeaponKind2::Gnasher, &scenes.gnasher, 0.0),
    ] {
        if let Some(h) = scene {
            let g = commands
                .spawn((
                    GunRoot(kind),
                    SceneRoot(h.clone()),
                    Transform::from_xyz(0.0, 0.0148239, 0.0)
                        .with_rotation(Quat::from_rotation_y(rot)),
                    Visibility::default(),
                ))
                .id();
            commands.entity(socket).add_child(g);
        }
    }
    joints.socket_done = true;
}

fn trs(t: &Trs) -> Transform {
    Transform {
        translation: Vec3::new(t.t.x, t.t.y, t.t.z),
        rotation: Quat::from_xyzw(t.r.x, t.r.y, t.r.z, t.r.w).normalize(),
        scale: Vec3::new(t.s.x, t.s.y, t.s.z),
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw(
    sim: Res<Sim>,
    mut joints: ResMut<Joints>,
    names: Query<(Entity, &Name)>,
    mut tf: ParamSet<(
        Query<&mut Transform, With<PawnRoot>>,
        Query<&mut Transform, With<Wrap>>,
        Query<(&mut Transform, &mut Projection), With<Camera3d>>,
        Query<&mut Transform>,
        Query<(&mut Transform, &mut Visibility), With<Flash>>,
        Query<(&TargetMesh, &mut Transform)>,
        Query<&mut Visibility, With<Wrap>>,
    )>,
    mut guns: Query<(&GunRoot, &mut Visibility), (Without<Flash>, Without<Wrap>)>,
    rig: Res<RigView>,
    mut hud: Query<(&Hud, &mut Text, &mut TextColor)>,
    mut bars: Query<(&Bar, &mut Node)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut gizmos: Gizmos,
) {
    let g = &sim.game;
    let f = g.frame();
    // pawn
    let dir = GV::new(f.draw_yaw.cos(), f.draw_yaw.sin(), 0.0);
    let p = f.pawn_pos + GV::new(g.ch.yaw.cos(), g.ch.yaw.sin(), 0.0) * f.forward_offset;
    if let Ok(mut t) = tf.p0().get_single_mut() {
        t.translation = to_b(GV::new(p.x, p.y, f.height));
        t.rotation = Quat::from_rotation_y(-f.draw_yaw);
    }
    let _ = dir;
    if let (Ok(mut t), Some(pose)) = (tf.p1().get_single_mut(), f.pose.as_ref()) {
        t.scale = Vec3::new(if pose.mirror { -100.0 } else { 100.0 }, 100.0, 100.0);
    }
    // camera: UE3 FOVAngle is horizontal
    let aspect = windows
        .get_single()
        .map(|w| w.width() / w.height().max(1.0))
        .unwrap_or(16.0 / 9.0);
    if let Ok((mut t, mut proj)) = tf.p2().get_single_mut() {
        let pos = to_b(f.cam_pos);
        *t = Transform::from_translation(pos).looking_to(to_b(f.cam_forward), Vec3::Y);
        if let Projection::Perspective(pp) = proj.as_mut() {
            pp.fov = 2.0 * ((f.cam_fov.to_radians() * 0.5).tan() / aspect).atan();
        }
    }
    // skeleton pose + gun pose
    if let Some(pose) = f.pose.as_ref() {
        let ents: Vec<Option<Entity>> = joints.marcus.clone();
        let mut q = tf.p3();
        for (i, e) in ents.iter().enumerate() {
            if let (Some(e), Some(l)) = (e, pose.local.get(i)) {
                if let Ok(mut t) = q.get_mut(*e) {
                    *t = trs(l);
                }
            }
        }
    }
    if let (Some(gp), Some(gm)) = (f.gun_pose.as_ref(), gun_model(g, f.gun)) {
        let cache = if f.gun == WeaponKind2::Lancer {
            &mut joints.lancer
        } else {
            &mut joints.gnasher
        };
        if cache.len() != gm.nodes.len() || cache.iter().all(|e| e.is_none()) {
            let by_name: HashMap<&str, Entity> =
                names.iter().map(|(e, n)| (n.as_str(), e)).collect();
            *cache = gm
                .nodes
                .iter()
                .map(|n| by_name.get(n.name.as_str()).copied())
                .collect();
        }
        let ents = cache.clone();
        let mut q = tf.p3();
        for (i, e) in ents.iter().enumerate() {
            // skip the glTF scene root nodes (their transform holds the export's axis fix)
            if gm.nodes[i].parent.is_none() {
                continue;
            }
            if let (Some(e), Some(l)) = (e, gp.get(i)) {
                if let Ok(mut t) = q.get_mut(*e) {
                    *t = trs(l);
                }
            }
        }
    }
    for (k, mut v) in &mut guns {
        *v = if k.0 == f.gun {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    // muzzle flash: in front of the pawn's hands toward the aim
    if let Ok((mut t, mut v)) = tf.p4().get_single_mut() {
        *v = if f.muzzle_flash {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        let m = GV::new(
            f.pawn_pos.x + g.ch.yaw.cos() * 70.0,
            f.pawn_pos.y + g.ch.yaw.sin() * 70.0,
            128.0 + f.height,
        );
        t.translation = to_b(m);
    }
    // targets: knocked down when dead
    for (tm, mut t) in &mut tf.p5() {
        if let Some(tg) = g.targets.get(tm.0) {
            let down = tg.dead > 0.0;
            t.rotation = if down {
                Quat::from_rotation_x(((3.0 - tg.dead) * 6.0).min(std::f32::consts::FRAC_PI_2))
            } else {
                Quat::IDENTITY
            };
            t.translation = to_b(GV::new(
                tg.x,
                tg.y,
                if down { TARGET_R } else { TARGET_H * 0.5 },
            ));
        }
    }
    // rig view: the skeleton as lines (joint → parent), the gun as a line along its barrel
    for mut v in &mut tf.p6() {
        *v = if rig.0 {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    // tracers
    for tr in &g.tracers {
        gizmos.line(
            to_b(tr.from),
            to_b(tr.to),
            Color::srgba(1.0, 0.82, 0.54, 1.0 - tr.age / 0.07),
        );
    }
    // HUD
    let w = g.weapon();
    for (h, mut text, mut col) in &mut hud {
        match h {
            Hud::State => **text = format!("{}  ·  {}", f.state, f.cam_mode),
            Hud::Ammo => {
                **text = format!(
                    "{}  {}",
                    if f.gun == WeaponKind2::Lancer {
                        "LANCER"
                    } else {
                        "GNASHER"
                    },
                    f.hud_ammo
                )
            }
            Hud::Msg => **text = f.hud_msg.clone(),
            Hud::Cross => {
                col.0 = if f.hit_marker {
                    Color::srgb(0.94, 0.49, 0.29)
                } else if f.aiming {
                    Color::WHITE
                } else {
                    Color::srgba(1.0, 1.0, 1.0, 0.5)
                };
            }
            Hud::Help => {}
        }
        if matches!(h, Hud::Ammo) && f.bonus {
            col.0 = Color::srgb(0.91, 0.71, 0.3);
        } else if matches!(h, Hud::Ammo) {
            col.0 = Color::srgb(0.9, 0.88, 0.84);
        }
    }
    let pct = |x: f32, len: f32| Val::Percent((x / len * 100.0).clamp(0.0, 100.0));
    for (b, mut n) in &mut bars {
        match (b, f.reload_bar) {
            (Bar::Fill, None) => {
                n.left = Val::Px(0.0);
                n.width = Val::Percent(w.mag as f32 / w.spec.mag_size as f32 * 100.0);
            }
            (Bar::Fill, Some(_)) => n.width = Val::Px(0.0),
            (Bar::Good, Some((_, len, _, p1, g1, 0))) => {
                n.left = pct(p1, len);
                n.width = pct(g1 - p1, len);
            }
            (Bar::Perfect, Some((_, len, p0, p1, _, 0))) => {
                n.left = pct(p0, len);
                n.width = pct(p1 - p0, len);
            }
            (Bar::Mark, Some((el, len, ..))) => {
                n.left = pct(el, len);
                n.width = Val::Px(2.0);
            }
            _ => n.width = Val::Px(0.0),
        }
    }
}

fn gun_model(g: &Game, k: WeaponKind2) -> Option<&gears_play::glb::Model> {
    g.gun_model(if k == WeaponKind2::Lancer { 0 } else { 1 })
}
