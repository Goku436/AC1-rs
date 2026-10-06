//! ac1: Assassin's Creed (2007) runtime on Bevy, using data from the user's own install.
//!
//! Controls (AC1's puppet system): WASD move, the mouse looks (Esc frees the cursor, a click takes it back),
//! wheel zoom. Hold the right button for high profile. Space is the legs: low profile blends (the praying
//! walk; among scholars Altaïr hides, and with no direction walks along with them), high profile free-runs
//! (sprints, vaults, jumps off edges, runs up or grabs walls on its own), jumps, and leaps while climbing
//! (held, it leaps up a wall hold to hold and climbs over the top). Shift is the empty hand: a gentle push,
//! by a scholar a pickpocket, on a wall let go. (Combat is left out for now.) P free camera (WASD fly along the view, Space/E up, Ctrl/Q down,
//! Shift faster; P again returns to Altaïr),
//! On a wall WASD climbs, Space + WASD leaps, S at the bottom steps off, Shift lets go. On the ground a Space
//! press jumps only in high profile with a direction (running jump, or a wall run when sprinting at a wall);
//! standing or in low profile it only grabs what is in reach (a wall, up to a ledge, a ladder, a bar). Free
//! running into a wall grabs or runs up it at any speed; running vaults low walls and sprinting jumps off edges
//! on its own; on a post Space jumps to the next one, on a beam a direction walks along it; on a swing bar Space
//! lets go at the next forward swing (S held drops); on a ladder W/S climb, Shift lets go; Space under a kiosk
//! frame swings
//! across it. Pulling up onto a post or at the end of a wall uses one hand (pillars P and Q behind block A).
//! Ground moves: run stops, turning round, turns on the spot, ledge stops and looking down at big drops, low
//! profile + Space at an edge pulls down onto it, running into a tall wall leans on it, rolls off high landings,
//! Space by a haystack hides in it (platform E and its haystacks, north of block D).
//! F1 toggle IK, F2 skeleton,
//! F3 IK targets, F6 pause the pose gallery and NPC line-up (they only move near the camera anyway), F12 screenshot,
//! [ / ] step through every AC1 clip in the library (Backspace returns to locomotion).
//!
//! F9 writes the flight recorder (the last 10 s, `recorder`). Test hooks (env): AC1_RECORD_AT=secs, AC1_ROUTE="x1,z1,x2,z2" (draw a navigation route), AC1_NPCS / AC1_NO_NPCS (the NPC line-up), AC1_NO_CLOTH (the robe skinned, not cloth),
//! AC1_COLLISION=render / AC1_SHOW_COLLISION (cities: collide with render meshes / draw the collision shapes), AC1_NO_LIPS (cities: no holds from probed lips), AC1_PROBE_CLIMB=n (cities: climb n spots near the start and log how far, then exit; see `probe`),
//! AC1_RAGDOLL_LOG, AC1_LIMP=secs (the player goes limp: the ragdoll), AC1_EMBED_CHECK (warn when the body is inside geometry), AC1_EAGLE=secs (press Q), AC1_LOOK="eye x,y,z,target x,y,z" (a fixed camera), AC1_EDGES (outlines in shots), AC1_NO_GALLERY (no pose gallery; scripted runs leave it out unless AC1_GALLERY is set), AC1_GAME_DIR, AC1_START="x,z,yaw_deg", AC1_WALK=speed (AC1_STOP=secs lets go, AC1_TURN=secs turns round, AC1_CURVE=rad/s curves it, AC1_STEER=deg walks that far off the start facing, positive left), AC1_CLIMB=grab|<dir>[=secs],... (up/down/left/right/drop, leap-<dir>),
//! AC1_POSE_PROPS (city props skinned to their skeleton's pose, not bind pose), AC1_NO_CROWD=1, AC1_NO_PROPS=1 (no prop zone; scripted runs leave it out unless AC1_PROPS is set),
//! AC1_CROWD_AT=metres (where along its loop the scholar group starts), AC1_LEVEL=masyaf|damascus|... (a
//! city from the game data instead of the test level), AC1_FPS=1 (log the frame rate), AC1_FREECAM="x,y,z" (start in the free camera there),
//! AC1_JUMP=secs[,secs...] (press Space then; with AC1_WALK for a running jump), AC1_HIGH / AC1_LEGS=from-to
//! (hold high profile / the legs), AC1_HAND=secs[,..] (the empty hand),
//! AC1_CAM="yaw,pitch,dist[,focus height]", AC1_SHOT=path.png (saved after AC1_SHOT_SECS, then exit; scripted runs open in the background, as AC1_BACKGROUND=1 does:
//! on the second monitor if any, unfocused, behind other windows; AC1_WINDOW_AT="x,y" places it), AC1_NO_IK=1,
//! AC1_ANIM=<clip name> loops one clip, AC1_NO_ANIM=1 uses procedural locomotion only.

mod animation;
mod assets;
mod character;
mod city;
mod climb;
mod crowd;
mod gallery;
mod level;
mod nav;
mod npc;
mod pad;
mod probe;
mod ragdoll;
mod recorder;
mod robe;

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::light::GlobalAmbientLight;
use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use character::Player;
use character::{Character, Controller};
use std::path::PathBuf;

/// `name="a-b"`: a time range.
fn env_range(name: &str) -> Option<(f32, f32)> {
    std::env::var(name).ok().and_then(|s| s.split_once('-').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))))
}

fn env_f32s(name: &str) -> Option<Vec<f32>> {
    std::env::var(name).ok().map(|s| s.split(',').filter_map(|v| v.trim().parse().ok()).collect())
}

#[derive(Resource)]
struct OrbitCam {
    focus: f32,
    /// Free camera (P): where it is, while flying.
    free: Option<Vec3>,
    /// Where the camera is now (to start flying from).
    eye: Vec3,
    /// Smoothed point the camera looks at.
    at: Option<Vec3>,
    yaw: f32,
    pitch: f32,
    dist: f32,
}

#[derive(Resource, Default)]
struct Debug {
    skeleton: bool,
    targets: bool,
    /// G: hide the outlines of what can be grabbed or stood on.
    hide_edges: bool,
}

#[derive(Resource)]
struct Script {
    walk: Option<f32>,
    /// Go limp (the ragdoll) at this time.
    limp: Option<f32>,
    /// Press the head button (Q) at this time.
    eagle: Option<f32>,
    /// Walking: let go of the direction at this time, or turn it round.
    stop: Option<f32>,
    turn: Option<f32>,
    /// Walking: the direction turns this fast (rad/s, positive to the left).
    curve: f32,
    /// Walking: the direction is this far off the start facing (radians, positive to the left; `AC1_STEER`, degrees).
    steer: f32,
    dir: std::sync::OnceLock<Vec3>,
    climb: Option<String>,
    jump: Vec<f32>,
    shot: Option<(PathBuf, f32)>,
    taken: bool,
    /// Held high profile / legs (from-to), and empty hand presses.
    high: Option<(f32, f32)>,
    legs: Option<(f32, f32)>,
    hand: Vec<f32>,
}

#[derive(Component)]
struct Hud;

fn main() {
    let game_dir = std::env::var("AC1_GAME_DIR").unwrap_or_else(|_| r"P:\SteamLibrary\steamapps\common\Assassins Creed".into());
    let cam = env_f32s("AC1_CAM").unwrap_or_default();
    let script = Script {
        walk: std::env::var("AC1_WALK").ok().and_then(|s| s.parse().ok()),
        curve: std::env::var("AC1_CURVE").ok().and_then(|s| s.parse().ok()).unwrap_or(0.0),
        steer: std::env::var("AC1_STEER").ok().and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0).to_radians(),
        stop: std::env::var("AC1_STOP").ok().and_then(|s| s.parse().ok()),
        eagle: std::env::var("AC1_EAGLE").ok().and_then(|s| s.parse().ok()),
        limp: std::env::var("AC1_LIMP").ok().and_then(|s| s.parse().ok()),
        turn: std::env::var("AC1_TURN").ok().and_then(|s| s.parse().ok()),
        dir: Default::default(),
        climb: std::env::var("AC1_CLIMB").ok(),
        jump: std::env::var("AC1_JUMP").map(|s| s.split(',').filter_map(|v| v.trim().parse().ok()).collect()).unwrap_or_default(),
        shot: std::env::var("AC1_SHOT").ok().map(|p| (PathBuf::from(p), std::env::var("AC1_SHOT_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(4.0))),
        taken: false,
        high: env_range("AC1_HIGH"),
        legs: env_range("AC1_LEGS"),
        hand: env_f32s("AC1_HAND").unwrap_or_default(),
    };
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin { primary_window: Some(game_window()), ..default() }))
        .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default())
        .insert_resource(ClearColor(Color::srgb(0.62, 0.72, 0.82)))
        // (Bright sky fill: faces under hoods and walls in shade read as in AC1, not black.)
        .insert_resource(GlobalAmbientLight { color: Color::srgb(0.93, 0.96, 1.0), brightness: 1500.0, ..default() })
        .insert_resource(OrbitCam {
            focus: cam.get(3).copied().unwrap_or(1.3),
            at: None,
            free: env_f32s("AC1_FREECAM").filter(|v| v.len() >= 3).map(|v| Vec3::new(v[0], v[1], v[2])),
            eye: Vec3::ZERO,
            yaw: cam.first().copied().unwrap_or(25.0).to_radians(),
            pitch: cam.get(1).copied().unwrap_or(-12.0).to_radians(),
            dist: cam.get(2).copied().unwrap_or(3.6),
        })
        .insert_resource(GameDir(PathBuf::from(game_dir)))
        .insert_resource(Debug {
            skeleton: std::env::var("AC1_DEBUG").is_ok(),
            targets: std::env::var("AC1_DEBUG").is_ok(),
            hide_edges: std::env::var("AC1_SHOT").is_ok() && std::env::var("AC1_EDGES").is_err(),
        })
        .insert_resource(script)
        .add_systems(Startup, (level::spawn_level, setup, grab_cursor).chain())
        .init_resource::<Eagle>()
        .init_resource::<recorder::Recorder>()
        .init_resource::<pad::PadStick>()
        .add_systems(Update, pad::log_connected)
        .add_systems(
            Update,
            (
                player_input,
                run_script,
                character::locomotion,
                crowd::walk_crowds,
                crowd::blend,
                crowd::charge,
                character::animate,
                fps_log,
                camera_follow,
                debug_draw,
                draw_edges,
                eagle,
                hud,
                place_labels,
                screenshot,
                recorder::record,
            )
                .chain(),
        )
        .add_systems(Update, pad_input.before(player_input))
        .init_resource::<probe::Probe>()
        .add_systems(Update, probe::probe_climbs.after(run_script).before(character::locomotion))
        .init_resource::<ScriptClock>()
        .add_systems(First, tick_script_clock)
        .init_resource::<character::StatuePlay>()
        .add_systems(Update, pause_statues)
        // The robe needs the bones' world transforms of this frame.
        .add_systems(PostUpdate, robe::robe_cloth.after(bevy::transform::TransformSystems::Propagate))
        .run();
}

#[derive(Resource)]
pub(crate) struct GameDir(pub(crate) PathBuf);

/// The NPC line-up: builders of a Damascus side-quest block (everything they use is in it), and where.
const NPC_FORGE: &str = "DataPC_Damascus.forge";
const NPC_BLOCK: &str = "Damascus_MB02 - Side Quest_DataBlock";
const NPC_LINEUP: [(&str, &str); 8] = [
    ("MNMT_Scholar", "Scholar"),
    ("MCMA_Poor", "Peasant"),
    ("MCFA_Poor", "Woman"),
    ("MNFT_Harasser", "Harasser"),
    ("MNMA_Vigilante", "Vigilante"),
    ("MCxx_Merchant", "Merchant"),
    ("MMMA_FreeMission_Militia", "Militia"),
    ("MMMA_FreeMission_Elite", "Elite guard"),
];
const NPC_LINEUP_AT: Vec3 = Vec3::new(-12.0, 0.0, -34.0);
/// The hooded scholars to blend with: Acre's builder, everything it uses in its data block.
const SCHOLAR_FORGE: &str = "DataPC_Acre.forge";
const SCHOLAR_BLOCK: &str = "Acre_Outskirts Setup Acre_DataBlock";

/// The window. Scripted runs (`AC1_SHOT`) and `AC1_BACKGROUND=1` stay out of the way of whatever else is on screen:
/// on the second monitor if there is one, unfocused, behind other windows.
fn game_window() -> Window {
    let window = Window { title: "ac1-rs".into(), resolution: (1280u32, 720u32).into(), ..default() };
    if std::env::var("AC1_SHOT").is_err() && std::env::var("AC1_BACKGROUND").is_err() {
        return window;
    }
    // (`AC1_WINDOW_AT="x,y"`: a desktop position, for a monitor layout the index doesn't match.)
    let position = match env_f32s("AC1_WINDOW_AT").filter(|v| v.len() >= 2) {
        Some(v) => bevy::window::WindowPosition::At(IVec2::new(v[0] as i32, v[1] as i32)),
        None => bevy::window::WindowPosition::Centered(bevy::window::MonitorSelection::Index(1)),
    };
    Window { position, focused: false, window_level: bevy::window::WindowLevel::AlwaysOnBottom, ..window }
}

/// The test scripts' clock (`AC1_*` hooks and `AC1_SHOT_SECS`): game time, stepped at most 0.05 s a frame as
/// movement is, so a slow frame (the first ones, while textures upload) cannot put a script ahead of the body.
/// The game's own clock is left alone.
#[derive(Resource, Default)]
struct ScriptClock {
    t: f32,
    dt: f32,
}

fn tick_script_clock(time: Res<Time>, mut clock: ResMut<ScriptClock>) {
    clock.dt = time.delta_secs().min(0.05);
    clock.t += clock.dt;
}

/// The mouse looks around from the start (not in scripted runs).
fn grab_cursor(script: Res<Script>, mut cursor: Query<&mut bevy::window::CursorOptions, With<bevy::window::PrimaryWindow>>) {
    if script.shot.is_none()
        && let Ok(mut c) = cursor.single_mut()
    {
        c.grab_mode = bevy::window::CursorGrabMode::Locked;
        c.visible = false;
    }
}

/// F6: pause or play the pose gallery and the NPC line-up.
fn pause_statues(keys: Res<ButtonInput<KeyCode>>, mut play: ResMut<character::StatuePlay>) {
    if keys.just_pressed(KeyCode::F6) {
        play.paused ^= true;
        info!("gallery figures {}", if play.paused { "paused" } else { "playing near the camera" });
    }
}

/// `AC1_FPS=1`: log the frame rate every two seconds.
fn fps_log(time: Res<Time>, mut acc: Local<(f32, u32)>) {
    if std::env::var("AC1_FPS").is_err() {
        return;
    }
    acc.0 += time.delta_secs();
    acc.1 += 1;
    if acc.0 >= 2.0 {
        info!("fps {:.0}", acc.1 as f32 / acc.0);
        *acc = (0.0, 0);
    }
}

fn setup(
    mut commands: Commands,
    dir: Res<GameDir>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    level: Res<level::Level>,
) {
    commands.spawn((
        DirectionalLight { illuminance: 9000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.8, -0.9, 0.0)),
    ));
    // (Cities are a couple of kilometres across.)
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection { far: 5000.0, ..default() }),
        Transform::from_xyz(0.0, 2.0, 4.0).looking_at(Vec3::Y, Vec3::Y),
    ));
    // Eagle Vision's dimming.
    commands.spawn((
        EagleTint,
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        BackgroundColor(Color::srgba(0.04, 0.08, 0.25, 0.45)),
        Visibility::Hidden,
        GlobalZIndex(-1),
    ));
    // Names over the test world's features.
    for (name, at) in &level.labels {
        spawn_label(&mut commands, name, *at, None);
    }
    commands.spawn((
        Hud,
        Text::new(""),
        TextFont { font_size: bevy::text::FontSize::Px(14.0), ..default() },
        // (A dark backing: readable over the bright city.)
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.4)),
        Node { position_type: PositionType::Absolute, left: px(6), top: px(4), padding: UiRect::all(px(4)), ..default() },
    ));

    let data = match assets::load_character(&dir.0, "DataPC.forge", "Rank 9", "UCMA_Altair_Rank_9") {
        Ok(d) => d,
        Err(e) => {
            error!("could not load Altaïr from {:?}: {e:#}. Set AC1_GAME_DIR to your Assassin's Creed folder.", dir.0);
            return;
        }
    };
    let mut lib = if std::env::var("AC1_NO_ANIM").is_ok() {
        None
    } else {
        match animation::AnimLib::load(&dir.0, "DataPC.forge", "Game Fix", &data.rig, data.hashes.clone()) {
            Ok(l) => Some(l),
            Err(e) => {
                warn!("no animation library ({e:#}); using procedural locomotion");
                None
            }
        }
    };
    match ragdoll::load(&dir.0) {
        Ok(r) => commands.insert_resource(r),
        Err(e) => warn!("no ragdoll ({e:#}); the dead keep their last pose"),
    }
    let mut animator = lib.as_mut().and_then(animation::Animator::new);
    if let (Some(a), Some(l), Ok(name)) = (animator.as_mut(), lib.as_mut(), std::env::var("AC1_ANIM")) {
        match l.get(&name) {
            Some(c) => a.forced = Some((c, 0.0)),
            None => warn!("AC1_ANIM: no clip named {name}"),
        }
    }
    let start = env_f32s("AC1_START").unwrap_or_default();
    let home = level.spawn.unwrap_or(Vec3::ZERO);
    let mut at = Vec3::new(start.first().copied().unwrap_or(home.x), 0.0, start.get(1).copied().unwrap_or(home.z));
    if let Some(g) = level.ground(at, 400.0, 500.0) {
        at.y = g.point.y;
    }
    // The pose gallery's figures share the player's model (test world only).
    // (Not in scripted runs unless asked for: a hundred more figures change their frame timing.)
    let scripted = std::env::var("AC1_SHOT").is_ok() && std::env::var("AC1_GALLERY").is_err();
    let gallery_data = (!level.city && !scripted && std::env::var("AC1_NO_GALLERY").is_err()).then(|| data.clone());
    let root = character::spawn_character(&mut commands, data, at, &mut meshes, &mut mats, &mut images, &mut bindposes, animator, 0);
    if let (Some(data), Some(lib)) = (gallery_data, lib.as_mut()) {
        let rows = gallery::poses(&lib.names);
        for (r, (category, on_wall, clips)) in rows.iter().enumerate() {
            let z = GALLERY.z + r as f32 * GALLERY_ROW;
            let lift = if *on_wall { 1.2 } else { 0.0 };
            spawn_label(&mut commands, category, Vec3::new(GALLERY.x + 1.5, 2.6 + lift, z), None);
            if *on_wall {
                // A wall they face (the figures look along -Z).
                let len = clips.len() as f32 * GALLERY_STEP + 1.0;
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(len, 4.5, 0.2))),
                    MeshMaterial3d(mats.add(StandardMaterial { base_color: Color::srgb(0.72, 0.68, 0.6), perceptual_roughness: 0.9, ..default() })),
                    Transform::from_translation(Vec3::new(GALLERY.x - len * 0.5 + 0.5, 2.25, z - 0.62)),
                ));
            }
            for (i, name) in clips.iter().enumerate() {
                let p = Vec3::new(GALLERY.x - i as f32 * GALLERY_STEP, lift, z);
                let mut anim = animation::Animator::new(lib);
                if let (Some(a), Some(c)) = (anim.as_mut(), lib.get(name)) {
                    a.forced = Some((c, 0.0));
                }
                let e = character::spawn_character(&mut commands, data.clone(), p, &mut meshes, &mut mats, &mut images, &mut bindposes, anim, 0);
                commands.entity(e).insert(character::Statue);
                spawn_label_near(&mut commands, name.trim_start_matches("xx_"), Vec3::Y * (2.1 - lift * 0.3), Some(e), 7.0);
            }
        }
    }
    commands.entity(root).insert(Player);
    // NPCs assembled from AC1's builders (`npc`): a line-up south of the flow lane, two of each.
    let lineup = !level.city && (std::env::var("AC1_NPCS").is_ok() || (!scripted && std::env::var("AC1_NO_NPCS").is_err()));
    if lineup && let Some(lib) = lib.as_mut() {
        for (i, (builder, label)) in NPC_LINEUP.iter().enumerate() {
            for v in 0..2u64 {
                let (g, scale) = match npc::build(&dir.0, NPC_FORGE, NPC_BLOCK, builder, 7 + i as u64 * 31 + v * 977) {
                    Ok(b) => b,
                    Err(e) => {
                        warn!("npc {builder}: {e:#}");
                        continue;
                    }
                };
                let rig_id = lib.add_rig(&g.rig, g.hashes.clone());
                lib.use_set(
                    rig_id,
                    if builder.starts_with("MMMA") {
                        "mmaa_anim_set"
                    } else if builder.starts_with("MCFA") {
                        "cfaa_anim_set"
                    } else {
                        "cmma_anim_set"
                    },
                );
                let anim = animation::Animator::new_for(lib, rig_id);
                let p = Vec3::new(NPC_LINEUP_AT.x + (i as f32 * 2.0 + v as f32) * 1.3, 0.0, NPC_LINEUP_AT.z);
                let e = character::spawn_character(&mut commands, g, p, &mut meshes, &mut mats, &mut images, &mut bindposes, anim, rig_id);
                commands.entity(e).insert((character::Statue, Transform::from_translation(p).with_scale(Vec3::splat(scale))));
                if v == 0 {
                    spawn_label(&mut commands, label, Vec3::new(p.x + 0.65, 2.4, p.z), None);
                }
            }
        }
    }
    // A group of scholars walking a loop, to blend into.
    let mut crowds = crowd::Crowds::default();
    // (In a city only where the ground suits a loop.)
    if std::env::var("AC1_NO_CROWD").is_err() && (!level.city || level.crowd_path.is_some()) {
        crowds.groups.push(crowd::Group {
            path: level
                .crowd_path
                .clone()
                .unwrap_or_else(|| vec![Vec3::new(-12.0, 0.0, -4.5), Vec3::new(-3.0, 0.0, -4.5), Vec3::new(-3.0, 0.0, -6.2), Vec3::new(-12.0, 0.0, -6.2)]),
            s: env_f32s("AC1_CROWD_AT").and_then(|v| v.first().copied()).unwrap_or(0.0),
        });
        for (k, slot) in [Vec2::new(-0.4, 0.0), Vec2::new(0.4, 0.0), Vec2::new(-0.4, 0.9), Vec2::new(0.4, 0.9)].into_iter().enumerate() {
            // AC1's hooded scholars (the `CNMA_Scholar` builder), each its own; the hand-made one if need be.
            let built =
                npc::build(&dir.0, SCHOLAR_FORGE, SCHOLAR_BLOCK, "CNMA_Scholar", 0x5c + k as u64 * 104_729).map_err(|e| warn!("scholar builder: {e:#}")).ok();
            let s = match built.map(|b| b.0).map_or_else(
                || {
                    let (skeletons, meshes_list) = crowd::scholar_recipe();
                    assets::load_assembled_from(&dir.0, &skeletons, &meshes_list)
                },
                Ok,
            ) {
                Ok(s) => s,
                Err(e) => {
                    warn!("no scholars: {e:#}");
                    break;
                }
            };
            let rig_id = lib.as_mut().map_or(0, |l| l.add_rig(&s.rig, s.hashes.clone()));
            let mut anim = lib.as_mut().and_then(|l| animation::Animator::new_for(l, rig_id));
            if let (Some(a), Some(clip)) = (anim.as_mut(), lib.as_mut().and_then(|l| l.get_for(rig_id, crowd::WALK_CLIP))) {
                // Out of step with each other.
                a.layer = Some((clip.clone(), clip.anim.duration * k as f32 * 0.37 % clip.anim.duration.max(1e-3)));
            }
            let e =
                character::spawn_character(&mut commands, s, Vec3::new(-12.0, 0.0, -4.5), &mut meshes, &mut mats, &mut images, &mut bindposes, anim, rig_id);
            commands.entity(e).insert(crowd::Scholar::new(0, slot));
            if k == 0 && !level.city {
                spawn_label(&mut commands, "Crowd A (blend)", Vec3::Y * 2.2, Some(e));
            }
        }
    }
    commands.insert_resource(crowds);
    if let Some(l) = lib {
        commands.insert_resource(l);
    }
    let yaw = start.get(2).copied().unwrap_or(0.0).to_radians();
    commands.entity(root).insert(Transform::from_translation(at).with_rotation(Quat::from_rotation_y(yaw)));
    if std::env::var("AC1_NO_IK").is_ok() {
        commands.queue(move |w: &mut World| {
            if let Some(mut c) = w.get_mut::<Character>(root) {
                c.ik_enabled = false;
            }
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn player_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    mut cam: ResMut<OrbitCam>,
    mut dbg: ResMut<Debug>,
    mut q: Query<(&mut Controller, &mut Character, &Transform), With<Player>>,
    script: Res<Script>,
    stick: Res<pad::PadStick>,
    mut lib: Option<ResMut<animation::AnimLib>>,
    time: Res<Time>,
    mut cursor: Query<&mut bevy::window::CursorOptions, With<bevy::window::PrimaryWindow>>,
    scholars: Query<&Transform, (With<crowd::Scholar>, Without<Player>)>,
) {
    // Scripted captures must not pick up stray keystrokes.
    if script.shot.is_some() {
        return;
    }
    // The mouse looks around while the cursor is held (Esc lets it go, a click takes it back).
    let mut grabbed = false;
    if let Ok(mut c) = cursor.single_mut() {
        if keys.just_pressed(KeyCode::Escape) {
            c.grab_mode = bevy::window::CursorGrabMode::None;
            c.visible = true;
        } else if mouse.just_pressed(MouseButton::Left) && c.visible {
            c.grab_mode = bevy::window::CursorGrabMode::Locked;
            c.visible = false;
            return;
        }
        grabbed = !c.visible;
    }
    if keys.just_pressed(KeyCode::KeyP) {
        cam.free = if cam.free.is_some() { None } else { Some(cam.eye) };
    }
    if grabbed {
        cam.yaw -= motion.delta.x * 0.003;
        let up = if cam.free.is_some() { 1.5 } else { 0.6 };
        cam.pitch = (cam.pitch - motion.delta.y * 0.003).clamp(-1.5, up);
    }
    // Free camera: WASD along the view, Space/E up, Ctrl/Q down, Shift faster; the player stands.
    if let Some(eye) = cam.free {
        let rot = Quat::from_euler(EulerRot::YXZ, cam.yaw, cam.pitch, 0.0);
        let axis = |k: KeyCode| -> f32 { if keys.pressed(k) { 1.0 } else { 0.0 } };
        let fwd = axis(KeyCode::KeyW) - axis(KeyCode::KeyS);
        let side = axis(KeyCode::KeyD) - axis(KeyCode::KeyA);
        let up = axis(KeyCode::Space).max(axis(KeyCode::KeyE)) - axis(KeyCode::ControlLeft).max(axis(KeyCode::KeyQ));
        let speed = if keys.pressed(KeyCode::ShiftLeft) { FREECAM_FAST } else { FREECAM_SPEED };
        let step = (rot * Vec3::NEG_Z * fwd + rot * Vec3::X * side + Vec3::Y * up).normalize_or_zero() * speed * time.delta_secs();
        cam.free = Some(eye + step);
        if let Ok((mut ctl, _, _)) = q.single_mut() {
            ctl.move_dir = Vec3::ZERO;
            ctl.climb_dir = Vec2::ZERO;
            ctl.leap = false;
        }
        return;
    }
    cam.dist = (cam.dist * (1.0 - scroll.delta.y * 0.1)).clamp(1.2, 15.0);
    if keys.just_pressed(KeyCode::F2) {
        dbg.skeleton ^= true;
    }
    if keys.just_pressed(KeyCode::F3) {
        dbg.targets ^= true;
    }
    if keys.just_pressed(KeyCode::KeyG) {
        dbg.hide_edges ^= true;
    }
    let Ok((mut ctl, mut ch, tf)) = q.single_mut() else { return };
    let by_scholar = scholars.iter().any(|s| (s.translation - tf.translation).with_y(0.0).length() < PICKPOCKET_REACH);
    if let (Some(lib), Some(a)) = (lib.as_mut(), ch.animator.as_mut()) {
        let step = keys.just_pressed(KeyCode::BracketRight) as i32 - keys.just_pressed(KeyCode::BracketLeft) as i32;
        if step != 0 && !lib.names.is_empty() {
            let cur = a.forced.as_ref().and_then(|(c, _)| lib.names.iter().position(|n| *n == c.name)).unwrap_or(0) as i32;
            let mut i = cur;
            // Skip clips that fail to parse.
            for _ in 0..lib.names.len() {
                i = (i + step).rem_euclid(lib.names.len() as i32);
                let name = lib.names[i as usize].clone();
                if let Some(c) = lib.get(&name) {
                    a.forced = Some((c, 0.0));
                    break;
                }
            }
        }
        if keys.just_pressed(KeyCode::Backspace) {
            a.forced = None;
        }
    }
    if keys.just_pressed(KeyCode::F1) {
        ch.ik_enabled ^= true;
    }
    let mut input = stick.0;
    if keys.pressed(KeyCode::KeyW) {
        input.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        input.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        input.x += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        input.x -= 1.0;
    }
    let fwd = Vec3::new(-cam.yaw.sin(), 0.0, -cam.yaw.cos());
    let right = Vec3::new(cam.yaw.cos(), 0.0, -cam.yaw.sin());
    ctl.move_dir = fwd * input.y + right * input.x;
    // AC1's puppet controls: the right button is the high profile, Space the legs, Shift the empty hand
    // (see the module docs).
    let high = mouse.pressed(MouseButton::Right) || keys.pressed(KeyCode::KeyK);
    let legs = keys.pressed(KeyCode::Space);
    let hand = keys.just_pressed(KeyCode::ShiftLeft) || keys.just_pressed(KeyCode::ShiftRight);
    ctl.high = high;
    let moving = input.length() > 0.1;
    ctl.free_run = high && legs;
    ctl.blend_walk = !high && legs;
    ctl.speed = if ctl.free_run {
        SPRINT
    } else if high {
        RUN
    } else if ctl.blend_walk {
        crowd::BLEND_SPEED
    } else {
        WALK
    };
    ctl.climb_dir = input;
    // On a wall the legs leap (with a direction), jump off a perch or rebound, and the hand lets go.
    // On the ground the legs jump only with a direction in high profile; else they only grab what is in reach.
    let press = keys.just_pressed(KeyCode::Space);
    ctl.leap = legs;
    if ch.wall.is_some() {
        ctl.wall_legs |= press;
        ctl.toggle_climb |= hand;
    } else if press {
        ctl.toggle_climb = true;
        ctl.grab_only = !(high && moving);
        // Starting to free-run is not a jump: a running jump needs a drop just ahead.
        ctl.jump_needs_edge = true;
    }
    ctl.push |= hand && !high && ch.wall.is_none() && !by_scholar;
    ctl.pickpocket |= hand && !high && ch.wall.is_none() && by_scholar;
}

/// A controller (see `pad`): its buttons as keys, its right stick turning the camera.
fn pad_input(
    time: Res<Time>,
    pads: Query<&bevy::input::gamepad::Gamepad>,
    keys: ResMut<ButtonInput<KeyCode>>,
    mouse: ResMut<ButtonInput<MouseButton>>,
    stick: ResMut<pad::PadStick>,
    mut cam: ResMut<OrbitCam>,
) {
    if let Some(turn) = pad::read(time, pads, keys, mouse, stick) {
        cam.yaw -= turn.x;
        let up = if cam.free.is_some() { 1.5 } else { 0.6 };
        cam.pitch = (cam.pitch + turn.y).clamp(-1.5, up);
    }
}

/// A name shown on screen over a point of the world, or over an entity (`at` is then its offset).
#[derive(Component)]
struct WorldLabel {
    at: Vec3,
    follow: Option<Entity>,
    /// Shown from this close (m).
    range: f32,
}

/// The pose gallery: its first row's first figure, the spacing between figures and between rows (m).
const GALLERY: Vec3 = Vec3::new(-40.0, 0.0, -14.0);
const GALLERY_STEP: f32 = 1.8;
const GALLERY_ROW: f32 = 3.6;

fn spawn_label(commands: &mut Commands, name: &str, at: Vec3, follow: Option<Entity>) {
    spawn_label_near(commands, name, at, follow, LABEL_RANGE);
}

fn spawn_label_near(commands: &mut Commands, name: &str, at: Vec3, follow: Option<Entity>, range: f32) {
    commands.spawn((
        WorldLabel { at, follow, range },
        Text::new(name),
        TextFont { font_size: bevy::text::FontSize::Px(13.0), ..default() },
        TextColor(Color::srgb(1.0, 0.95, 0.55)),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
        Node { position_type: PositionType::Absolute, padding: UiRect::axes(px(3), px(1)), ..default() },
        Visibility::Hidden,
    ));
}

/// Keep the labels over their points, hidden behind the camera or far off.
fn place_labels(
    cams: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    things: Query<&GlobalTransform, Without<Camera3d>>,
    mut labels: Query<(&WorldLabel, &mut Node, &mut Visibility, &ComputedNode)>,
) {
    let Ok((cam, cam_tf)) = cams.single() else { return };
    for (l, mut node, mut vis, size) in &mut labels {
        let at = match l.follow {
            Some(e) => match things.get(e) {
                Ok(t) => t.translation() + l.at,
                Err(_) => continue,
            },
            None => l.at,
        };
        let near = cam_tf.translation().distance(at) < l.range;
        match cam.world_to_viewport(cam_tf, at) {
            // (Written only when changed: a changed node lays the UI out again.)
            Ok(p) if near => {
                let w = size.size().x * size.inverse_scale_factor();
                let (left, top) = (px((p.x - w * 0.5).round()), px(p.y.round()));
                if node.left != left || node.top != top {
                    node.left = left;
                    node.top = top;
                }
                vis.set_if_neq(Visibility::Visible);
            }
            _ => {
                vis.set_if_neq(Visibility::Hidden);
            }
        }
    }
}

/// Labels further than this from the camera are hidden (m).
const LABEL_RANGE: f32 = 40.0;

/// AC1's head button (Q): at a viewpoint, synchronize (the camera sweeps round Altaïr); elsewhere, held,
/// Eagle Vision (the world dims and guards show red, the crowd pale blue).
#[derive(Resource, Default)]
struct Eagle {
    /// Seconds into a synchronization, and the viewpoints done.
    sync: Option<f32>,
    synced: Vec<usize>,
    vision: bool,
    /// The HUD's last notice and how long it shows yet.
    notice: Option<(String, f32)>,
}

/// The dimming over the screen in Eagle Vision.
#[derive(Component)]
struct EagleTint;

/// Synchronizing takes this long (s), the camera going round once.
const SYNC_TIME: f32 = 5.0;

/// The others Eagle Vision shows: the scholars.
type Others<'w, 's> = Query<'w, 's, &'static Transform, (With<crowd::Scholar>, Without<Player>)>;

/// Q: synchronize at a viewpoint (standing on its beam's end), else hold for Eagle Vision.
#[allow(clippy::too_many_arguments)]
fn eagle(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    script: Res<Script>,
    level: Res<level::Level>,
    mut eagle: ResMut<Eagle>,
    player: Query<(&Transform, &Character), With<Player>>,
    mut tint: Query<&mut Visibility, With<EagleTint>>,
    others: Others,
    mut gizmos: Gizmos,
    clock: Res<ScriptClock>,
) {
    let Ok((tf, ch)) = player.single() else { return };
    let t = clock.t;
    let scripted = script.eagle.is_some_and(|at| t > at && t - clock.dt <= at);
    let pressed = keys.just_pressed(KeyCode::KeyQ) || scripted;
    let held = keys.pressed(KeyCode::KeyQ) || script.eagle.is_some_and(|at| (at..at + 3.0).contains(&t));
    if let Some(s) = &mut eagle.sync {
        *s += time.delta_secs();
        if *s > SYNC_TIME {
            eagle.sync = None;
            eagle.notice = Some(("Viewpoint synchronized".to_string(), 3.0));
        }
    }
    // On the viewpoint's beam: balancing on it, or standing at its end.
    let on_perch = ch.wall.as_ref().is_none_or(|w| w.state == "perch");
    let viewpoint = level.viewpoints.iter().position(|v| v.distance(tf.translation) < 1.2);
    if pressed
        && on_perch
        && eagle.sync.is_none()
        && let Some(i) = viewpoint
    {
        eagle.sync = Some(0.0);
        if !eagle.synced.contains(&i) {
            eagle.synced.push(i);
        }
    }
    eagle.vision = held && eagle.sync.is_none() && !(on_perch && viewpoint.is_some());
    if let Some((_, left)) = &mut eagle.notice {
        *left -= time.delta_secs();
        if *left <= 0.0 {
            eagle.notice = None;
        }
    }
    for mut v in &mut tint {
        *v = if eagle.vision { Visibility::Visible } else { Visibility::Hidden };
    }
    if eagle.vision {
        for o in &others {
            let color = Color::srgb(0.55, 0.75, 1.0);
            let p = o.translation;
            for h in [0.05, 0.9, 1.75] {
                gizmos.circle(Isometry3d::new(p + Vec3::Y * h, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), 0.35, color);
            }
            gizmos.line(p, p + Vec3::Y * 1.8, color);
        }
        // Viewpoints, gold, from afar.
        for &v in &level.viewpoints {
            let gold = Color::srgb(1.0, 0.8, 0.2);
            gizmos.line(v, v + Vec3::Y * 25.0, gold);
            gizmos.circle(Isometry3d::new(v, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), 0.8, gold);
        }
    }
}

/// The empty hand within this of a scholar (m) picks its pocket.
const PICKPOCKET_REACH: f32 = 1.2;

/// Ground speeds (m/s), those of AC1's gait clips (root motion over the half-cycle): low profile walk
/// (`xx_l_walk_hipm`, 1.9), high profile run (`xx_h_run_hipm`, 5.2), free-run sprint (`xx_h_sprint_hipm`, 6.2).
const WALK: f32 = 1.9;
const RUN: f32 = 5.2;
const SPRINT: f32 = 6.2;

fn run_script(clock: Res<ScriptClock>, script: Res<Script>, mut q: Query<(&mut Controller, &Transform, &mut Character), With<Player>>) {
    let Ok((mut ctl, tf, mut ch)) = q.single_mut() else { return };
    let t = clock.t;
    if let Some(speed) = script.walk {
        // (The direction first faced, kept: turning round must not follow the body.)
        let dir = *script.dir.get_or_init(|| Quat::from_rotation_y(script.steer) * tf.rotation * Vec3::NEG_Z);
        let dir = Quat::from_rotation_y(script.curve * t) * dir;
        ctl.move_dir = if script.turn.is_some_and(|at| t > at) { -dir } else { dir };
        ctl.speed = speed;
        if script.stop.is_some_and(|at| t > at) {
            ctl.move_dir = Vec3::ZERO;
        }
    }
    if script.limp.is_some_and(|at| t > at) {
        ch.limp = true;
    }
    if script.jump.iter().any(|&at| t > at && t - clock.dt <= at) {
        ctl.toggle_climb = true;
    }
    // The puppet buttons, as the player would press them.
    let held = |r: Option<(f32, f32)>| r.is_some_and(|(a, b)| (a..b).contains(&t));
    if script.high.is_some() || script.legs.is_some() {
        let (high, legs) = (held(script.high), held(script.legs));
        // (No direction held unless walking: as the player's input would leave it.)
        if script.walk.is_none() {
            ctl.move_dir = Vec3::ZERO;
        }
        ctl.high = high;
        ctl.free_run = high && legs;
        ctl.blend_walk = !high && legs;
        // (Held, as the player's held legs: on a wall they leap, on a swing bar they fling.)
        if script.climb.is_none() {
            ctl.leap = legs;
        }
        if ctl.free_run {
            ctl.speed = SPRINT;
        } else if ctl.blend_walk {
            ctl.speed = crowd::BLEND_SPEED;
        }
        // Pressing the legs on a wall: its legs moves (jump off a perch, rebound, eject).
        if let Some((a, _)) = script.legs
            && t > a
            && t - clock.dt <= a
            && ch.wall.is_some()
        {
            ctl.wall_legs = true;
        }
        // Pressing the legs: in low profile (or standing), grab only (as the player's Space).
        if let Some((a, _)) = script.legs
            && t > a
            && t - clock.dt <= a
            && !(high && ctl.move_dir.length() > 0.1)
        {
            ctl.toggle_climb = true;
            ctl.grab_only = true;
        }
    }
    if script.hand.iter().any(|&at| t > at && t - clock.dt <= at) {
        ctl.push = !ctl.high;
    }
    if let Some(c) = &script.climb {
        // "grab" just grabs; otherwise a list of `dir[=seconds]` (up, down, left, right, `leap-<dir>`, or "drop" to
        // let go, then wait) run in order after the grab, e.g. "up=4,right=3,drop=3". A leading "nograb,"
        // skips the grab (steering only, e.g. with AC1_JUMP).
        let t = clock.t;
        let (c, mut at) = match c.strip_prefix("nograb,") {
            Some(steps) => (steps, 0.0),
            None => {
                if t > 0.5 && t - clock.dt <= 0.5 {
                    ctl.toggle_climb = true;
                }
                (c.as_str(), 1.5)
            }
        };
        ctl.climb_dir = Vec2::ZERO;
        ctl.leap = false;
        for step in c.split(',') {
            let (dir, secs) = step.split_once('=').map_or((step, f32::MAX), |(d, s)| (d, s.parse().unwrap_or(f32::MAX)));
            let (leap, dir) = dir.strip_prefix("leap-").map_or((false, dir), |d| (true, d));
            let v = match dir {
                "up" => Vec2::Y,
                "down" => Vec2::NEG_Y,
                "right" => Vec2::X,
                "left" => Vec2::NEG_X,
                _ => Vec2::ZERO,
            };
            if dir == "drop" && t > at && t - clock.dt <= at {
                ctl.toggle_climb = true;
            }
            if t > at && t <= at + secs {
                ctl.climb_dir = v;
                ctl.leap = leap;
                break;
            }
            at += secs;
        }
    }
}

/// Follow the player's body (its hips, at the root's height), smoothed a little.
fn camera_follow(
    time: Res<Time>,
    mut cam: ResMut<OrbitCam>,
    chars: Query<(&Transform, &Character), With<Player>>,
    joints: Query<&GlobalTransform>,
    mut cams: Query<&mut Transform, (With<Camera3d>, Without<Player>)>,
    eagle: Res<Eagle>,
) {
    let (Ok((target, ch)), Ok(mut tf)) = (chars.single(), cams.single_mut()) else { return };
    // Test hook: a fixed camera (eye, then the point it looks at), for comparable shots.
    if let Some(v) = env_f32s("AC1_LOOK").filter(|v| v.len() >= 6) {
        *tf = Transform::from_xyz(v[0], v[1], v[2]).looking_at(Vec3::new(v[3], v[4], v[5]), Vec3::Y);
        cam.eye = tf.translation;
        return;
    }
    if let Some(eye) = cam.free {
        tf.translation = eye;
        tf.rotation = Quat::from_euler(EulerRot::YXZ, cam.yaw, cam.pitch, 0.0);
        cam.eye = eye;
        return;
    }
    let body = joints.get(ch.hips_joint()).map_or(target.translation, |h| h.translation().with_y(target.translation.y));
    // Synchronizing at a viewpoint: once round Altaïr, from above.
    if let Some(s) = eagle.sync {
        let yaw = cam.yaw + s / SYNC_TIME * std::f32::consts::TAU;
        let rot = Quat::from_euler(EulerRot::YXZ, yaw, -0.35, 0.0);
        let focus = body + Vec3::Y * 1.0;
        tf.translation = focus + rot * Vec3::Z * 6.0;
        tf.look_at(focus, Vec3::Y);
        cam.eye = tf.translation;
        return;
    }
    let k = 1.0 - (-12.0 * time.delta_secs()).exp();
    let at = cam.at.map_or(body, |a| a.lerp(body, k));
    cam.at = Some(at);
    let focus = at + Vec3::Y * cam.focus;
    let rot = Quat::from_euler(EulerRot::YXZ, cam.yaw, cam.pitch, 0.0);
    tf.translation = focus + rot * Vec3::Z * cam.dist;
    tf.look_at(focus, Vec3::Y);
    cam.eye = tf.translation;
}

/// Free camera speed (m/s), and with Shift.
const FREECAM_SPEED: f32 = 12.0;
const FREECAM_FAST: f32 = 50.0;

/// Outline what can be grabbed (ledges and holds, yellow), stood on (posts and beams, orange) and swung on
/// (bars, brown) near the player (G hides them).
fn draw_edges(dbg: Res<Debug>, level: Res<level::Level>, player: Query<&Transform, With<Player>>, mut gizmos: Gizmos) {
    let Ok(p) = player.single() else { return };
    if dbg.hide_edges {
        return;
    }
    let p = p.translation;
    let near = |a: Vec3, b: Vec3| {
        let d = b - a;
        let t = if d.length_squared() > 1e-6 { ((p - a).dot(d) / d.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
        (a + d * t).distance(p) < EDGE_RANGE
    };
    let lift = Vec3::Y * 0.012;
    for l in level.ledges.iter().filter(|l| near(l.a, l.b)) {
        gizmos.line(l.a + lift, l.b + lift, Color::srgb(1.0, 0.82, 0.25));
    }
    for l in level.perches.iter().filter(|l| near(l.a, l.b)) {
        if (l.b - l.a).length() < 0.05 {
            gizmos.circle(Isometry3d::new(l.a + lift, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), 0.15, Color::srgb(1.0, 0.55, 0.15));
        } else {
            gizmos.line(l.a + lift, l.b + lift, Color::srgb(1.0, 0.55, 0.15));
        }
    }
    for l in level.bars.iter().chain(&level.monkey).filter(|l| near(l.a, l.b)) {
        gizmos.line(l.a, l.b, Color::srgb(0.75, 0.45, 0.2));
    }
}

/// Edges further than this from the player are not outlined (m).
const EDGE_RANGE: f32 = 30.0;

fn debug_draw(
    dbg: Res<Debug>,
    level: Res<level::Level>,
    mut gizmos: Gizmos,
    chars: Query<&Character>,
    joints: Query<&GlobalTransform>,
    mut route: Local<Option<Vec<Vec3>>>,
) {
    // `AC1_ROUTE="x1,z1,x2,z2"`: a walking route over the navigation mesh, logged once and drawn.
    if route.is_none()
        && let (Some(nav), Some(v)) = (level.nav.as_ref(), env_f32s("AC1_ROUTE"))
        && v.len() == 4
    {
        let at = |x: f32, z: f32| level.ground(Vec3::new(x, 0.0, z), 400.0, 500.0).map_or(Vec3::new(x, 0.0, z), |g| g.point);
        let (a, b) = (at(v[0], v[1]), at(v[2], v[3]));
        let (la, lb) = (nav.locate(a), nav.locate(b));
        info!("route ends on triangles {la:?} {lb:?}, reaching {:?} / {:?}", la.map(|i| nav.reach(i, 100_000)), lb.map(|i| nav.reach(i, 100_000)));
        let r = nav.route(a, b).unwrap_or_default();
        info!("route {a:.1} -> {b:.1}: {} points, {:.1} m", r.len(), r.windows(2).map(|w| (w[1] - w[0]).length()).sum::<f32>());
        let mut pts = vec![a];
        pts.extend(r);
        *route = Some(pts);
    }
    if let Some(r) = route.as_ref() {
        for w in r.windows(2) {
            gizmos.line(w[0] + Vec3::Y * 0.3, w[1] + Vec3::Y * 0.3, Color::srgb(0.2, 0.9, 1.0));
        }
    }
    for ch in &chars {
        if dbg.targets {
            for &(p, c) in &ch.debug_targets {
                gizmos.sphere(Isometry3d::from_translation(p), 0.04, c);
            }
        }
        if dbg.skeleton {
            for (i, e) in ch.joint_entities().iter().enumerate() {
                if let (Some(p), Ok(a)) = (ch.parent_of(i), joints.get(*e))
                    && let Ok(b) = joints.get(ch.joint_entities()[p])
                {
                    gizmos.line(a.translation(), b.translation(), Color::srgb(1.0, 0.9, 0.1));
                }
            }
        }
    }
}

fn hud(
    chars: Query<(&Character, &Controller, &Transform), With<Player>>,
    mut text: Query<&mut Text, With<Hud>>,
    cam: Res<OrbitCam>,
    diag: Res<bevy::diagnostic::DiagnosticsStore>,
    level: Res<level::Level>,
    mut contexts: Local<(String, String)>,
    eagle: Res<Eagle>,
) {
    let fps = diag.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()).unwrap_or(0.0);
    let (Ok((ch, ctl, tf)), Ok(mut t)) = (chars.single(), text.single_mut()) else { return };
    // The detail lines: what the body is doing (context, previous, the move), the ground gait, the input,
    // the air and the last landing.
    let context = match &ch.wall {
        Some(w) => format!("Climb ({})", w.state),
        None if ch.blend => "Blend".to_string(),
        None => "Ground".to_string(),
    };
    if context != contexts.0 {
        contexts.1 = std::mem::replace(&mut contexts.0, context.clone());
    }
    let speed = ch.velocity.with_y(0.0).length();
    let gait = match speed {
        s if s < 0.2 => "idle",
        s if s < 2.6 => "walk",
        s if s < 4.3 => "jog",
        s if s < 5.7 => "run",
        _ => "sprint",
    };
    let clip = ch.wall.as_ref().and_then(|w| w.clip_name()).map_or("-".to_string(), |n| n.to_string());
    let air = ch.wall.as_ref().and_then(|w| w.air_velocity()).map_or("-".to_string(), |v| format!("{:.1} m/s up {:.1}", v.with_y(0.0).length(), v.y));
    let height = level.ground(tf.translation + Vec3::Y * 0.1, 0.0, 60.0).map_or(0.0, |g| tf.translation.y - g.point.y);
    let landing = ch.last_landing.map_or("-".to_string(), |(d, dmg)| format!("{d:.1} m drop, damage {dmg:.2}"));
    let eagle_line = match (&eagle.notice, eagle.sync, eagle.vision) {
        (_, Some(_), _) => "   SYNCHRONIZING".to_string(),
        (Some((n, _)), _, _) => format!("   {n}"),
        (_, _, true) => "   EAGLE VISION".to_string(),
        _ => String::new(),
    };
    let details = format!(
        "context: {}   previous: {}   move: {clip}{eagle_line}\nground: {gait}   speed {speed:.1} m/s   stick {:.2}   legs: {}\nair: {air}   height {height:.2} m   last landing: {landing}",
        contexts.0,
        if contexts.1.is_empty() { "-" } else { &contexts.1 },
        ctl.move_dir.length().min(1.0),
        if ctl.leap { "held" } else { "-" },
    );
    let profile = if ctl.free_run {
        "free running"
    } else if ctl.high {
        "high profile"
    } else if ctl.blend_walk {
        "blending"
    } else {
        "low profile"
    };
    let mode = if cam.free.is_some() {
        "free camera (P to return)".to_string()
    } else if let Some(w) = &ch.wall {
        format!("climbing / jumping (AC1 clips) state {}", w.state)
    } else if ch.climb.is_some() {
        "climbing".to_string()
    } else if ch.blend {
        "blending into the crowd".to_string()
    } else if let Some((c, t)) = ch.animator.as_ref().and_then(|a| a.forced.as_ref()) {
        format!("clip {} {:.2}/{:.2}s", c.name, t, c.anim.duration)
    } else if ch.animator.is_some() {
        "ground (AC1 clips)".to_string()
    } else {
        "ground (procedural)".to_string()
    };
    t.0 = format!(
        "ac1-rs | {fps:.0} fps | {mode} | {profile} | health {} | speed {:.1} m/s | IK {} (F1)\nMouse look (Esc frees the cursor) | RMB high profile | Space legs: blend, or with RMB free-run / jump / leap | Shift hand: push, let go | P free cam | G edges | Q eagle | [ ] clips, F2 skeleton, F3 IK, F6 gallery, F12 shot\n{details}",
        if ch.health <= 0.0 {
            "DESYNCHRONISED".to_string()
        } else {
            (0..5).map(|i| if ch.health > i as f32 * 0.2 + 0.1 { '#' } else { '-' }).collect::<String>()
        },
        ch.velocity.length(),
        if ch.ik_enabled { "on" } else { "off" }
    );
}

#[allow(clippy::too_many_arguments)]
fn screenshot(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    clock: Res<ScriptClock>,
    mut script: ResMut<Script>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::F12) {
        let path = format!("ac1-shot-{}.png", time.elapsed().as_millis());
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
    if let Some((path, secs)) = script.shot.clone() {
        let t = clock.t;
        if !script.taken && t > secs {
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
            script.taken = true;
        } else if script.taken && t > secs + 1.5 {
            exit.write(AppExit::Success);
        }
    }
}
