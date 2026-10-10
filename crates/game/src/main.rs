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
//! press jumps only in high profile with a direction and only at a target (a top, post, beam, hold, thin wall to go
//! over or bar; AC1's `JumpToGuidanceTarget`, the press held 0.3 s), or runs up a wall when sprinting at it;
//! standing or in low profile it only grabs what is in reach (a wall, up to a ledge, a ladder, a bar). E is the head
//! (Eagle Vision), C centres the camera, as AC1's default keys (`DefaultBindings.map` and the manual). Free
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
//! F9 writes the flight recorder (the last 10 s, `recorder`). Test hooks (env): AC1_RECORD_AT=secs, AC1_ROUTE="x1,z1,x2,z2" (draw a navigation route), AC1_SURFACES="x,z,..." (log the collision surfaces down a line), AC1_RAYS="ox,oy,oz,dx,dy,dz,..." (log what each ray hits), AC1_NPCS / AC1_NO_NPCS (the NPC line-up), AC1_NO_CLOTH (the robe skinned, not cloth),
//! AC1_COLLISION=render / AC1_SHOW_COLLISION (cities: collide with render meshes / draw the collision shapes), AC1_NO_LIPS (cities: no holds from probed lips), AC1_PROBE_CLIMB=n (cities: climb n spots near the start and log how far, then exit; see `probe`),
//! AC1_RAGDOLL_LOG, AC1_LIMP=secs (the player goes limp: the ragdoll), AC1_EMBED_CHECK (warn when the body is inside geometry), AC1_EAGLE=secs (press E), AC1_SLOWMO=factor (the whole game slowed, 0.1 = a tenth), AC1_JUMP_CANDS (log AC1's jump candidates), AC1_OLD_TARGETS (the older jump target search), AC1_LOOK="eye x,y,z,target x,y,z" (a fixed camera), AC1_EDGES (outlines in shots), AC1_GALLERY (the pose gallery: every clip on a figure; off by default, slow to load), AC1_GAME_DIR, AC1_START="x,z,yaw_deg[,y]" (y: start on the ground under that height), AC1_WALK=speed (AC1_STICK=1 holds the stick over instead, the speed from the gait model; AC1_STOP=secs lets go, or from-to, AC1_TURN=secs turns round, AC1_CURVE=rad/s curves it, AC1_STEER=deg walks that far off the start facing, positive left, AC1_VEER=secs,deg turns it then, AC1_PATH="x,z;x,z;..[;stop]" steers it through waypoints, AC1_HEADINGS="t,deg;.." gives its game heading over time), AC1_CLIMB=grab|<dir>[=secs],... (up/down/left/right, upleft/upright/downleft/downright, drop, leap-<dir>),
//! AC1_POSE_PROPS (city props skinned to their skeleton's pose, not bind pose), AC1_NO_CROWD=1, AC1_NO_PROPS=1 (no prop zone; scripted runs leave it out unless AC1_PROPS is set),
//! AC1_CROWD_AT=metres (where along its loop the scholar group starts), AC1_LEVEL=masyaf|damascus|... (a
//! city from the game data instead of the test level), AC1_FPS=1 (log the frame rate), AC1_FREECAM="x,y,z" (start in the free camera there),
//! AC1_JUMP=secs[,secs...] (press Space then; with AC1_WALK for a running jump), AC1_HIGH / AC1_LEGS=from-to
//! (hold high profile / the legs), AC1_HAND=secs[,..] (the empty hand),
//! AC1_ORBIT="x,y,z,radius,height,secs" (a camera circling a point, for videos) with AC1_FRAMES=dir (every frame saved
//! as dir/frame_00000.png ..., then exit; encode with ffmpeg at 30 fps),
//! AC1_CAM="yaw,pitch,dist[,focus height]", AC1_SHOT=path.png (saved after AC1_SHOT_SECS, then exit; `-` exits then without one), AC1_BACKGROUND=1 (open
//! on the second monitor if any, unfocused, behind other windows), AC1_WINDOW_AT="x,y" and AC1_WINDOW_SIZE="w,h" (place the window), AC1_NO_IK=1,
//! AC1_ANIM=<clip name> loops one clip, AC1_NO_ANIM=1 uses procedural locomotion only. AC1_CONTROL=<file> stays open and runs
//! the scripts written to that file (`run <id> <secs> AC1_START=.. AC1_WALK=.. ...`, `quit`; see `control_runs`): the level
//! loads once, each run takes seconds. AC1_GROUND_LINE, AC1_HOLDS_AT and AC1_MESHES_AT log the ground along a line, the
//! holds and the city meshes near a point.

mod animation;
mod assets;
mod character;
mod city;
mod climb;
mod climb_table;
mod crowd;
mod gait;
mod gallery;
mod jump;
mod jump_query;
mod level;
mod move_blend;
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
    /// Frames the video orbit (`AC1_ORBIT`) has gone round so far.
    orbit_frame: u32,
}

/// Frames a second of the video hooks (`AC1_ORBIT`, `AC1_FRAMES`).
const FRAMES_FPS: f32 = 30.0;
/// How long a Legs press stays good (s): AC1's input buffer.
const LEGS_BUFFER: f32 = 0.3;

#[derive(Resource, Default)]
struct Debug {
    skeleton: bool,
    targets: bool,
    /// G: hide the outlines of what can be grabbed or stood on.
    /// Outlines shown (G cycles them): 0 both the grab lines and the grabbable edges, 1 the grab lines, 2 the edges, 3 none.
    edges: u8,
}

#[derive(Resource, Default)]
struct Script {
    walk: Option<f32>,
    /// The walk holds the stick right over (the speed from `gait`, as the player's), not a fixed speed.
    stick: bool,
    /// Go limp (the ragdoll) at this time.
    limp: Option<f32>,
    /// Press the head button (Q) at this time.
    eagle: Option<f32>,
    /// Walking: let go of the direction at this time, or turn it round.
    stop: Option<(f32, f32)>,
    turn: Option<f32>,
    /// Walking: the direction turns this fast (rad/s, positive to the left).
    curve: f32,
    /// Walking: the direction is this far off the start facing (radians, positive to the left; `AC1_STEER`, degrees).
    steer: f32,
    /// Walking: from this time the direction is turned this far (radians, positive to the left; `AC1_VEER=secs,deg`).
    veer: Option<(f32, f32)>,
    /// Walking: the direction over time, (secs, game heading in degrees from +X toward +Y), blended between
    /// (`AC1_HEADINGS="t,deg;t,deg;.."`: another run's own, as the route library replays the real game's stick).
    headings: Vec<(f32, f32)>,
    /// Walking: waypoints (x, z) the direction points at in turn (`AC1_PATH="x,z;x,z;.."`), each passed within
    /// `PATH_REACH` (at any height); after the last, on the way the last leg went (or, ended by `;stop`, the stick let go).
    path: Vec<Vec2>,
    path_stop: bool,
    path_at: std::sync::Mutex<(usize, Option<Vec3>)>,
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

/// The test script from its settings (`AC1_WALK`, `AC1_HIGH`, ...): the environment at start, or a control file's run.
fn script_from(get: &dyn Fn(&str) -> Result<String, ()>) -> Script {
    let range = |v: Result<String, ()>| v.ok().and_then(|s| s.split_once('-').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))));
    let floats = |s: &str| s.split(',').filter_map(|v| v.trim().parse().ok()).collect::<Vec<f32>>();
    Script {
        walk: get("AC1_WALK").ok().and_then(|s| s.parse().ok()),
        stick: get("AC1_STICK").is_ok(),
        curve: get("AC1_CURVE").ok().and_then(|s| s.parse().ok()).unwrap_or(0.0),
        steer: get("AC1_STEER").ok().and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0).to_radians(),
        veer: get("AC1_VEER").ok().and_then(|s| {
            let (t, d) = s.split_once(',')?;
            Some((t.parse().ok()?, d.parse::<f32>().ok()?.to_radians()))
        }),
        path: get("AC1_PATH")
            .map(|s| {
                s.split(';')
                    .filter_map(|p| {
                        let (x, z) = p.split_once(',')?;
                        Some(Vec2::new(x.trim().parse().ok()?, z.trim().parse().ok()?))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        path_stop: get("AC1_PATH").is_ok_and(|s| s.trim_end().ends_with("stop")),
        headings: get("AC1_HEADINGS")
            .map(|s| {
                s.split(';')
                    .filter_map(|p| {
                        let (t, d) = p.split_once(',')?;
                        Some((t.trim().parse().ok()?, d.trim().parse().ok()?))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        path_at: Default::default(),
        // (`secs`, or `from-to`: the direction let go only then, taken up again after.)
        stop: get("AC1_STOP").ok().and_then(|s| match s.split_once('-') {
            Some((a, b)) => Some((a.parse().ok()?, b.parse().ok()?)),
            None => Some((s.parse().ok()?, f32::MAX)),
        }),
        eagle: get("AC1_EAGLE").ok().and_then(|s| s.parse().ok()),
        limp: get("AC1_LIMP").ok().and_then(|s| s.parse().ok()),
        turn: get("AC1_TURN").ok().and_then(|s| s.parse().ok()),
        dir: Default::default(),
        climb: get("AC1_CLIMB").ok(),
        jump: get("AC1_JUMP").map(|s| s.split(',').filter_map(|v| v.trim().parse().ok()).collect()).unwrap_or_default(),
        shot: get("AC1_SHOT").ok().map(|p| (PathBuf::from(p), get("AC1_SHOT_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(4.0))),
        taken: false,
        high: range(get("AC1_HIGH")),
        legs: range(get("AC1_LEGS")),
        hand: get("AC1_HAND").ok().map(|s| floats(&s)).unwrap_or_default(),
    }
}

fn main() {
    let game_dir = std::env::var("AC1_GAME_DIR").unwrap_or_else(|_| r"P:\SteamLibrary\steamapps\common\Assassins Creed".into());
    let cam = env_f32s("AC1_CAM").unwrap_or_default();
    let script = script_from(&|k| std::env::var(k).map_err(|_| ()));
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
            orbit_frame: 0,
        })
        .insert_resource(GameDir(PathBuf::from(game_dir)))
        .insert_resource(Debug {
            skeleton: std::env::var("AC1_DEBUG").is_ok(),
            targets: std::env::var("AC1_DEBUG").is_ok(),
            edges: if (std::env::var("AC1_SHOT").is_ok() || std::env::var("AC1_FRAMES").is_ok()) && std::env::var("AC1_EDGES").is_err() { 3 } else { 0 },
        })
        .insert_resource(script)
        .add_systems(Startup, (level::spawn_level, setup, grab_cursor, slow_motion).chain())
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
        .add_systems(Update, control_runs.before(run_script))
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
    // (`AC1_WINDOW_SIZE="w,h"`: its size.)
    let size = env_f32s("AC1_WINDOW_SIZE").filter(|v| v.len() >= 2).map_or((1280u32, 720u32), |v| (v[0] as u32, v[1] as u32));
    let window = Window { title: "ac1-rs".into(), resolution: size.into(), ..default() };
    // (`AC1_WINDOW_AT="x,y"`: a desktop position.)
    let window = match env_f32s("AC1_WINDOW_AT").filter(|v| v.len() >= 2) {
        Some(v) => Window { position: bevy::window::WindowPosition::At(IVec2::new(v[0] as i32, v[1] as i32)), ..window },
        None => window,
    };
    if std::env::var("AC1_BACKGROUND").is_err() {
        return window;
    }
    // In the background: unfocused; placed, where it was put (to be watched beside the real game), else on the second
    // monitor if there is one, behind other windows.
    if env_f32s("AC1_WINDOW_AT").is_some() {
        return Window { focused: false, ..window };
    }
    let position = bevy::window::WindowPosition::Centered(bevy::window::MonitorSelection::Index(1));
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

/// Control mode (`AC1_CONTROL=<file>`): the game stays open and runs the test scripts written to that file, one after
/// another, instead of one script at start (the level loads once: a test takes seconds, not a minute). A line
/// `run <id> <secs> AC1_START=x,z,yaw[,y] AC1_WALK=6.2 ...` (the usual script settings) puts the player at the start
/// (fresh: no move, no fall, full health), runs the script for `secs` from a clock at 0 and logs
/// `control: run <id> started` and `control: run <id> done, at [x, y, z] (game (x, y, z))`; `quit` closes the game.
/// A run is taken once (by its id).
#[derive(Default)]
struct Control {
    last: Option<String>,
    active: Option<(String, f32)>,
    checked: f32,
    /// `AC1_TRACE=<file>` on a run: the run frame by frame, as ac1-hook traces the real game (`seconds x y z yaw state
    /// clip`, game space; the clip when it changes), to set beside the real game's run.
    trace: Option<(std::io::BufWriter<std::fs::File>, String)>,
}

#[allow(clippy::too_many_arguments)]
fn control_runs(
    time: Res<Time>,
    level: Res<level::Level>,
    mut script: ResMut<Script>,
    mut clock: ResMut<ScriptClock>,
    mut q: Query<(&mut Transform, &mut Controller, &mut Character), With<Player>>,
    mut exit: MessageWriter<AppExit>,
    mut state: Local<Control>,
) {
    static FILE: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    let Some(file) = FILE.get_or_init(|| std::env::var("AC1_CONTROL").ok().map(PathBuf::from)) else { return };
    let Ok((mut tf, mut ctl, mut ch)) = q.single_mut() else { return };
    if let Some((id, secs)) = state.active.clone() {
        if let Some((out, last_clip)) = &mut state.trace {
            use std::io::Write;
            let p = tf.translation;
            let f = tf.rotation * Vec3::NEG_Z;
            // (Game yaw: from +X toward +Y; the game's Y is Bevy's -Z.)
            let yaw = (-f.z).atan2(f.x).to_degrees();
            let (st, clip) = match &ch.wall {
                Some(w) => (w.state.clone(), w.clip_name().unwrap_or("").to_string()),
                None => ("ground".to_string(), String::new()),
            };
            let started = if clip != *last_clip { clip.clone() } else { String::new() };
            *last_clip = clip;
            let _ = writeln!(out, "{:.4}	{:.3}	{:.3}	{:.3}	{yaw:.1}	{st}	{started}", clock.t, p.x, -p.z, p.y);
        }
        if clock.t > secs {
            let p = tf.translation;
            info!("control: run {id} done, at [{:.2}, {:.2}, {:.2}] (game ({:.2}, {:.2}, {:.2}))", p.x, p.y, p.z, p.x, -p.z, p.y);
            *script = Script::default();
            state.active = None;
            state.trace = None;
        }
        return;
    }
    // (The file looked at four times a second.)
    state.checked += time.delta_secs();
    if state.checked < 0.25 {
        return;
    }
    state.checked = 0.0;
    let Ok(text) = std::fs::read_to_string(file) else { return };
    let Some(line) = text.lines().map(str::trim).rfind(|l| !l.is_empty()) else { return };
    if state.last.as_deref() == Some(line) {
        return;
    }
    state.last = Some(line.to_string());
    let mut words = line.split_whitespace();
    match words.next() {
        Some("quit") => {
            exit.write(AppExit::Success);
        }
        Some("run") => {
            let (Some(id), Some(secs)) = (words.next(), words.next().and_then(|s| s.parse::<f32>().ok())) else {
                warn!("control: want `run <id> <secs> KEY=VALUE ...`, got {line}");
                return;
            };
            let vars: std::collections::HashMap<String, String> =
                words.filter_map(|w| w.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect();
            let mut s = script_from(&|k| vars.get(k).cloned().ok_or(()));
            s.shot = None;
            // The start: on the ground there (under `y`, else the highest), facing `yaw`.
            let start: Vec<f32> = vars.get("AC1_START").map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or_default();
            if start.len() >= 2 {
                let mut at = Vec3::new(start[0], 0.0, start[1]);
                let above = start.get(3).map_or(400.0, |y| y + 1.0);
                if let Some(g) = level.ground(at, above, 500.0) {
                    at.y = g.point.y;
                }
                tf.translation = at;
                tf.rotation = Quat::from_rotation_y(start.get(2).copied().unwrap_or(0.0).to_radians());
            }
            // (Fresh: as the climbing probe starts each spot.)
            ch.wall = None;
            ch.velocity = Vec3::ZERO;
            ch.fall_v = 0.0;
            ch.health = 1.0;
            ch.dead_time = 0.0;
            ch.ragdoll = None;
            ch.edge_lock = None;
            ch.exit_fade = None;
            // (Every input fresh too: a script sets the profile and free running only when it holds them, and a walk
            // after a free run went on free running, off a roof.)
            *ctl = Controller::default();
            *script = s;
            clock.t = 0.0;
            state.active = Some((id.to_string(), secs));
            state.trace = vars.get("AC1_TRACE").and_then(|p| std::fs::File::create(p).ok()).map(|f| {
                let mut w = std::io::BufWriter::new(f);
                use std::io::Write;
                let _ = writeln!(w, "seconds	x	y	z	yaw	state	clip");
                (w, String::new())
            });
            info!("control: run {id} started");
        }
        _ => warn!("control: unknown command {line}"),
    }
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

/// `AC1_SLOWMO=<factor>`: the whole game at that speed (0.1 = a tenth), to watch a move frame by frame.
fn slow_motion(mut time: ResMut<Time<Virtual>>) {
    if let Some(k) = std::env::var("AC1_SLOWMO").ok().and_then(|v| v.parse::<f32>().ok()).filter(|k| *k > 0.0) {
        time.set_relative_speed(k);
        info!("slow motion: {k}x");
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
    // AC1_SURFACES="x,z,x,z,...": log every collision surface down a vertical line at each point (for recordings).
    for p in env_f32s("AC1_SURFACES").unwrap_or_default().chunks_exact(2) {
        let mut from = Vec3::new(p[0], 400.0, p[1]);
        let mut found = vec![];
        while let Some(h) = level.raycast(from, Vec3::NEG_Y, from.y + 100.0) {
            found.push(format!("{:.2} (n.y {:.2})", h.point.y, h.normal.y));
            from = h.point - Vec3::Y * 0.01;
        }
        info!("surfaces at {:.2},{:.2}: {}", p[0], p[1], if found.is_empty() { "none".into() } else { found.join(", ") });
    }
    // AC1_RAYS="ox,oy,oz,dx,dy,dz,...": log the first hit of each ray (20 m).
    for r in env_f32s("AC1_RAYS").unwrap_or_default().chunks_exact(6) {
        let (o, d) = (Vec3::new(r[0], r[1], r[2]), Vec3::new(r[3], r[4], r[5]).normalize_or_zero());
        match level.raycast_sided(o, d, 20.0) {
            Some((h, behind)) => {
                info!("ray {o:.2} {d:.2}: hit at {:.2} ({:.2} m), normal {:.2}{}", h.point, h.dist, h.normal, if behind { ", from behind" } else { "" })
            }
            None => info!("ray {o:.2} {d:.2}: nothing"),
        }
    }
    let start = env_f32s("AC1_START").unwrap_or_default();
    let home = level.spawn.unwrap_or(Vec3::ZERO);
    let mut at = Vec3::new(start.first().copied().unwrap_or(home.x), 0.0, start.get(1).copied().unwrap_or(home.z));
    // (A fourth value: the height to find the ground under, not the highest roof.)
    let above = start.get(3).map_or(400.0, |y| y + 1.0);
    if let Some(g) = level.ground(at, above, 500.0) {
        at.y = g.point.y;
    }
    // The pose gallery's figures share the player's model (test world only).
    // (Off unless asked for: its hundred figures make the test world slow to load.)
    let scripted = std::env::var("AC1_SHOT").is_ok();
    let gallery_data = (!level.city && std::env::var("AC1_GALLERY").is_ok()).then(|| data.clone());
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
    mut legs_buffer: Local<f32>,
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
    // C centres the camera behind Altair (AC1's Center Camera).
    if keys.just_pressed(KeyCode::KeyC)
        && cam.free.is_none()
        && let Ok((_, _, tf)) = q.single()
    {
        cam.yaw = tf.rotation.to_euler(EulerRot::YXZ).0;
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
        dbg.edges = (dbg.edges + 1) % 4;
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
    // The speed comes from AC1's speed value (`gait`) by profile and how far the stick is pushed; blending walks at
    // the crowd's pace.
    ctl.stick = (!ctl.blend_walk).then_some(input.length().min(1.0));
    // (The profile's full speed, which the moves off the ground go by: sprinting along a beam, the speed a climb hands
    // back to the gait. On the ground the speed value replaces it.)
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
    // AC1 holds a Legs press for 0.3 s (`GoAssassinActionInterpreter`): on the ground a press just before a target
    // comes in reach still jumps at it.
    *legs_buffer = if press { LEGS_BUFFER } else { (*legs_buffer - time.delta_secs()).max(0.0) };
    ctl.leap = legs;
    // (A stop at an edge or leaning on a wall is not on a wall: the legs act as on the ground.)
    let on_wall = ch.wall.as_ref().is_some_and(|w| !w.legs_on_ground());
    if on_wall {
        ctl.wall_legs |= press;
        ctl.toggle_climb |= hand;
    } else if press || (*legs_buffer > 0.0 && ch.wall.is_none() && high && moving) {
        ctl.toggle_climb = true;
        ctl.grab_only = !(high && moving);
        // A press jumps only at a target (AC1's `JumpToGuidanceTarget`): starting to free-run in the open is no jump.
        ctl.jump_needs_target = true;
    }
    if ch.wall.is_some() {
        *legs_buffer = 0.0;
    }
    if hand && ch.wall.is_none() {
        ctl.hand_drop = character::HAND_DROP_BUFFER;
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
    cam: Res<OrbitCam>,
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
    // (The head is E, as AC1's default keys: `DefaultBindings.map`, the manual's controls page.)
    let head = cam.free.is_none();
    let pressed = (head && keys.just_pressed(KeyCode::KeyE)) || scripted;
    let held = (head && keys.pressed(KeyCode::KeyE)) || script.eagle.is_some_and(|at| (at..at + 3.0).contains(&t));
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

/// The profiles' full speeds (m/s), the gait clips' (root motion over the half-cycle): low profile walk
/// (`xx_l_walk_hipm`), high profile run (`xx_h_run_hipm`), free-run sprint (`xx_h_sprint_hipm`). On the ground the
/// player's speed comes from `gait`.
const WALK: f32 = 1.9;
/// A scripted waypoint (`AC1_PATH`) counts as passed this close (m, along the ground).
const PATH_REACH: f32 = 1.0;
const RUN: f32 = 5.12;
const SPRINT: f32 = 6.277;

fn run_script(clock: Res<ScriptClock>, script: Res<Script>, mut q: Query<(&mut Controller, &Transform, &mut Character), With<Player>>) {
    let Ok((mut ctl, tf, mut ch)) = q.single_mut() else { return };
    let t = clock.t;
    if let Some(speed) = script.walk {
        // (The direction first faced, kept: turning round must not follow the body.)
        let dir = *script.dir.get_or_init(|| {
            let d = Quat::from_rotation_y(script.steer) * tf.rotation * Vec3::NEG_Z;
            info!("script: walking toward [{:.2}, {:.2}] from [{:.2}, {:.2}, {:.2}]", d.x, d.z, tf.translation.x, tf.translation.y, tf.translation.z);
            d
        });
        let dir = Quat::from_rotation_y(script.curve * t) * dir;
        let mut dir = script.veer.filter(|(at, _)| t > *at).map_or(dir, |(_, a)| Quat::from_rotation_y(a) * dir);
        if let (Some(first), Some(last)) = (script.headings.first(), script.headings.last()) {
            // (Game degrees, Z up: +X toward +Y is Bevy's +X toward -Z.)
            let deg = match script.headings.windows(2).find(|w| t >= w[0].0 && t < w[1].0) {
                Some(w) => {
                    let k = (t - w[0].0) / (w[1].0 - w[0].0).max(1e-3);
                    w[0].1 + ((w[1].1 - w[0].1 + 540.0) % 360.0 - 180.0) * k
                }
                None if t < first.0 => first.1,
                None => last.1,
            };
            let r = deg.to_radians();
            dir = Vec3::new(r.cos(), 0.0, -r.sin());
        }
        if !script.path.is_empty()
            && let Ok(mut at) = script.path_at.lock()
        {
            let here = Vec2::new(tf.translation.x, tf.translation.z);
            while at.0 < script.path.len() && here.distance(script.path[at.0]) < PATH_REACH {
                info!("script: waypoint {} reached at [{:.2}, {:.2}]", at.0 + 1, here.x, here.y);
                at.0 += 1;
            }
            if let Some(w) = script.path.get(at.0) {
                let to = (*w - here).normalize_or_zero();
                at.1 = Some(Vec3::new(to.x, 0.0, to.y));
            }
            dir = at.1.unwrap_or(dir);
            if script.path_stop && at.0 >= script.path.len() {
                dir = Vec3::ZERO;
            }
        }
        ctl.move_dir = if script.turn.is_some_and(|at| t > at) { -dir } else { dir };
        ctl.speed = speed;
        ctl.stick = script.stick.then_some(1.0);
        if script.stop.is_some_and(|(a, b)| t >= a && t < b) {
            ctl.move_dir = Vec3::ZERO;
        }
    }
    if script.limp.is_some_and(|at| t > at) {
        ch.limp = true;
    }
    // (As the player's Space: on a wall its legs moves (rebound, jump off a perch), else a jump.)
    if script.jump.iter().any(|&at| t > at && t - clock.dt <= at) {
        if ch.wall.as_ref().is_some_and(|w| !w.legs_on_ground()) {
            ctl.wall_legs = true;
        } else {
            // (Exactly as a real press: grabbing only unless running in high profile, and a jump only at a target.)
            let high = script.high.is_some_and(|(a, b)| (a..b).contains(&t));
            ctl.toggle_climb = true;
            ctl.grab_only = !(high && script.walk.is_some());
            ctl.jump_needs_target = true;
        }
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
            ctl.stick = script.stick.then_some(1.0);
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
        ctl.hand_drop = character::HAND_DROP_BUFFER;
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
                "upleft" => Vec2::new(-0.7, 0.7),
                "upright" => Vec2::new(0.7, 0.7),
                "downleft" => Vec2::new(-0.7, -0.7),
                "downright" => Vec2::new(0.7, -0.7),
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
    // Video hook: a camera circling a point (`AC1_ORBIT="x,y,z,radius,height,secs"`, looking at it from `height` above
    // it), once round in `secs` of video at `FRAMES_FPS`, one step a rendered frame (smooth whatever the frame rate).
    if let Some(v) = env_f32s("AC1_ORBIT").filter(|v| v.len() >= 6) {
        let (centre, radius, height, secs) = (Vec3::new(v[0], v[1], v[2]), v[3], v[4], v[5].max(1.0));
        let a = cam.orbit_frame as f32 / (secs * FRAMES_FPS) * std::f32::consts::TAU;
        // (From when `AC1_FRAMES` starts saving, so the saved frames go round exactly once.)
        if time.elapsed_secs() > 1.0 {
            cam.orbit_frame += 1;
        }
        let eye = centre + Vec3::new(a.cos() * radius, height, a.sin() * radius);
        *tf = Transform::from_translation(eye).looking_at(centre, Vec3::Y);
        cam.eye = eye;
        return;
    }
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

/// Outline what can be grabbed (where the hands grab, yellow; the edge of the ledge they grab, white), stood on (posts
/// and beams, orange) and swung on (bars, brown) near the player. G cycles: both lines, the grab lines, the edges, none.
fn draw_edges(
    dbg: Res<Debug>,
    level: Res<level::Level>,
    player: Query<&Transform, With<Player>>,
    mut edges: Local<std::collections::HashMap<usize, Option<(Vec3, Vec3)>>>,
    mut gizmos: Gizmos,
) {
    let Ok(p) = player.single() else { return };
    if dbg.edges == 3 {
        return;
    }
    let p = p.translation;
    let near = |a: Vec3, b: Vec3| {
        let d = b - a;
        let t = if d.length_squared() > 1e-6 { ((p - a).dot(d) / d.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
        (a + d * t).distance(p) < EDGE_RANGE
    };
    let lift = Vec3::Y * 0.012;
    for (i, l) in level.ledges.iter().enumerate().filter(|(_, l)| near(l.a, l.b)) {
        if dbg.edges != 2 {
            gizmos.line(l.a + lift, l.b + lift, Color::srgb(1.0, 0.82, 0.25));
        }
        if dbg.edges != 1
            && let Some((a, b)) = *edges.entry(i).or_insert_with(|| ledge_edge(&level, l))
        {
            gizmos.line(a + lift, b + lift, Color::srgb(0.95, 0.95, 1.0));
        }
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

/// The outer edge of the ledge a hold runs along: its front face just under the hold's height, found from in front of
/// it (the hold line itself sits where the hands grab, a little in from the edge on the test level's strips).
fn ledge_edge(level: &level::Level, l: &level::Ledge) -> Option<(Vec3, Vec3)> {
    let face = |p: Vec3| level.raycast(p + l.out * 0.4 - Vec3::Y * 0.03, -l.out, 0.6).filter(|h| h.normal.dot(l.out) > 0.7).map(|h| h.point.with_y(p.y));
    Some((face(l.a)?, face(l.b)?))
}

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
    mut text: Query<(&mut Text, &mut Visibility), With<Hud>>,
    cam: Res<OrbitCam>,
    diag: Res<bevy::diagnostic::DiagnosticsStore>,
    level: Res<level::Level>,
    mut contexts: Local<(String, String)>,
    eagle: Res<Eagle>,
) {
    let fps = diag.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()).unwrap_or(0.0);
    let (Ok((ch, ctl, tf)), Ok((mut t, mut vis))) = (chars.single(), text.single_mut()) else { return };
    // (Recording a video: no HUD in the frames.)
    if std::env::var("AC1_FRAMES").is_ok() {
        *vis = Visibility::Hidden;
        return;
    }
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
    // AC1's speed band and value (`gait`).
    let gait = match gait::band(ch.gait.value) {
        gait::Band::Stand if speed >= 0.2 => "moving".to_string(),
        b => format!("{} {:.2}", format!("{b:?}").to_lowercase(), ch.gait.value),
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
        "ac1-rs | {fps:.0} fps | {mode} | {profile} | health {} | speed {:.1} m/s | IK {} (F1)\nMouse look (Esc frees the cursor) | RMB high profile | Space legs: blend, or with RMB free-run / jump / leap | Shift hand: push, let go | P free cam | G outlines (both, grab, edge, none) | E eagle | C centre camera | [ ] clips, F2 skeleton, F3 IK, F6 gallery, F12 shot\n{details}",
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
    mut frame: Local<u32>,
    player: Query<&Transform, With<Player>>,
) {
    // Video hook: every frame saved (`AC1_FRAMES=dir`: dir/frame_00000.png, ...), as many as the orbit's `secs` at
    // `FRAMES_FPS`, then exit (encode them with ffmpeg at that rate). The first second is left out (the city loading in).
    if let Ok(dir) = std::env::var("AC1_FRAMES") {
        let secs = env_f32s("AC1_ORBIT").and_then(|v| v.get(5).copied()).unwrap_or(10.0);
        let total = (secs * FRAMES_FPS) as u32;
        if time.elapsed_secs() > 1.0 {
            if *frame < total {
                commands.spawn(Screenshot::primary_window()).observe(save_to_disk(format!("{dir}/frame_{:05}.png", *frame)));
            } else if *frame > total + 60 {
                exit.write(AppExit::Success);
            }
            *frame += 1;
        }
        return;
    }
    if keys.just_pressed(KeyCode::F12) {
        let path = format!("ac1-shot-{}.png", time.elapsed().as_millis());
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
    if let Some((path, secs)) = script.shot.clone() {
        let t = clock.t;
        // (`AC1_SHOT=-`: no picture, just the end of the run: the scenarios read only the log.)
        if path.as_os_str() == "-" && t > secs {
            // (Where he ended, for comparing a run with the real game's.)
            if !std::mem::replace(&mut script.taken, true)
                && let Ok(tf) = player.single()
            {
                let p = tf.translation;
                info!("script: the run ends at [{:.2}, {:.2}, {:.2}] (game ({:.2}, {:.2}, {:.2}))", p.x, p.y, p.z, p.x, -p.z, p.y);
            }
            exit.write(AppExit::Success);
        } else if !script.taken && t > secs {
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
            script.taken = true;
        } else if script.taken && t > secs + 1.5 {
            exit.write(AppExit::Success);
        }
    }
}
