//! ac1: Assassin's Creed (2007) runtime on Bevy, using data from the user's own install.
//!
//! Controls (AC1's puppet system): WASD move, the mouse looks (Esc frees the cursor, a click takes it back),
//! wheel zoom. Hold the right button for high profile. Space is the legs: low profile blends (the praying
//! walk; among scholars Altaïr hides, and with no direction walks along with them), high profile free-runs
//! (sprints, vaults, jumps off edges, runs up or grabs walls on its own), jumps, and leaps while climbing
//! (held, it leaps up a wall hold to hold and climbs over the top). Shift is the empty hand: a gentle push,
//! by a scholar a pickpocket, on a wall let go. (Combat is left out for now.) P free camera (WASD fly along the view, Space/E up, Ctrl/Q down,
//! Shift faster; P again returns to Altaïr), F7 AC1's Ghost mode, the developers' flight (`fly`: no collision, WASD
//! along the ground by the camera, Space up, Left Ctrl down, the right button × 3, Left Alt × 10, Q held keeps the
//! facing; F7 again falls out, from rest, or with Shift held at the flight's speed),
//! On a wall WASD climbs, Space + WASD leaps, S at the bottom steps off, Shift lets go. On the ground (AC1's buttons):
//! Space pressed in high profile at run speed runs up a wall ahead; let go within 0.3 s it is the tap jump (high
//! profile, moving: at a target, else the leap of faith or a haystack, else AC1's free jump; standing, the static
//! jump); in low profile it only sits on a bench. Shift (the empty hand) climbs onto a wall, pulls down over an edge,
//! and held in a fall catches a hold. E is the head
//! (Eagle Vision), C centres the camera, as AC1's default keys (`DefaultBindings.map` and the manual). Free
//! running into a wall grabs or runs up it at any speed; running vaults low walls and sprinting jumps off edges
//! on its own; on a post Space jumps to the next one, on a beam a direction walks along it; on a swing bar Space
//! lets go at the next forward swing (S held drops); on a ladder W/S climb, Shift lets go; the tap jump at a
//! market stall's counter (a kiosk) jumps onto it and runs along it or across it (kiosk K, south of block L). Pulling up onto a post or at the end of a wall uses one hand (pillars P and Q behind block A).
//! Ground moves: run stops, turning round, turns on the spot, ledge stops and looking down at big drops, the empty
//! hand at an edge pulls down onto it, running into a tall wall leans on it, rolls off high landings, walking into a
//! haystack dives in (platform E and its haystacks, north of block D).
//! F1 hides the text (the HUD and the labels), F2 the map menu (1 the test level, 2 Masyaf, 3 Damascus: the game starts
//! again in it), F8 the graphics settings (`settings`, `menu`: presets Low / Medium / High / Ultra, window mode,
//! resolution, vsync, frame cap, city detail, texture size, shadows, anti-aliasing, fog, view distance; saved to
//! `ac1-settings.txt` beside the executable), F10 online (`net`: hosting on / off and its port, joining an IP and port,
//! the replay recorder; the menus take the arrows, Enter, Esc, typing and the mouse), F4 toggle IK, F5 skeleton,
//! F3 IK targets, F6 pause the pose gallery and NPC line-up (they only move near the camera anyway), F12 screenshot, M
//! mutes the sound (AC1's own, played from the clips' event keys: steps, landings, grunts; `sound`),
//! [ / ] step through every AC1 clip in the library (Backspace returns to locomotion).
//!
//! F9 writes the flight recorder (the last 10 s, `recorder`). Test hooks (env): AC1_RECORD_AT=secs, AC1_ROUTE="x1,z1,x2,z2" (draw a navigation route), AC1_SURFACES="x,z,..." (log the collision surfaces down a line), AC1_RAYS="ox,oy,oz,dx,dy,dz,..." (log what each ray hits), AC1_NPCS / AC1_NO_NPCS (the NPC line-up), AC1_NO_CLOTH (the robe skinned, not cloth),
//! AC1_COLLISION=render / AC1_SHOW_COLLISION (cities: collide with render meshes / draw the collision shapes), AC1_NO_LIPS (cities: no holds from probed lips), AC1_PROBE_CLIMB=n (cities: climb n spots near the start and log how far, then exit; see `probe`),
//! AC1_RAGDOLL_LOG, AC1_LIMP=secs (the player goes limp: the ragdoll), AC1_EMBED_CHECK (warn when the body is inside geometry), AC1_EAGLE=secs (press E), AC1_GRASP=from-to (the empty hand held: the catch in a fall), AC1_FLY=from-to (the Ghost mode flight then, AC1_WALK its stick; AC1_FLY_UP / AC1_FLY_DOWN=from-to up / down held), AC1_PAD=1 (read the controller: off by default, as it takes the pad from other games), AC1_SLOWMO=factor (the whole game slowed, 0.1 = a tenth), AC1_HEADLESS=1 (no window, no GPU, no audio: the frames run back to back, for the scenarios), AC1_MUTE=1 (start muted), AC1_VOLUME=dB, AC1_STEP=hz (every frame 1/hz s of game time however long it takes; 60 when headless), AC1_JUMP_CANDS (log AC1's jump candidates), AC1_OLD_TARGETS (the older jump target search), AC1_LOOK="eye x,y,z,target x,y,z" (a fixed camera), AC1_EDGES (outlines in shots), AC1_GALLERY (the pose gallery: every clip on a figure; off by default, slow to load), AC1_GAME_DIR, AC1_START="x,z,yaw_deg[,y]" (y: start on the ground under that height), AC1_WALK=speed (AC1_STICK=1 holds the stick over instead, the speed from the gait model; AC1_STOP=secs lets go, or from-to, AC1_TURN=secs turns round, AC1_CURVE=rad/s curves it, AC1_STEER=deg walks that far off the start facing, positive left, AC1_VEER=secs,deg turns it then, AC1_PATH="x,z;x,z;..[;stop]" steers it through waypoints, AC1_HEADINGS="t,deg;.." gives its game heading over time), AC1_CLIMB=grab|<dir>[=secs],... (up/down/left/right, upleft/upright/downleft/downright, drop, leap-<dir>),
//! AC1_POSE_PROPS (city props skinned to their skeleton's pose, not bind pose), AC1_NO_CROWD=1, AC1_NO_PROPS=1 (no prop zone; scripted runs leave it out unless AC1_PROPS is set),
//! AC1_CROWD_AT=metres (where along its loop the scholar group starts), AC1_LEVEL=masyaf|damascus|... (a
//! city from the game data instead of the test level), AC1_FPS=1 (log the time to the first frame, then the frame rate, the GPU's time a frame and what the city draws), AC1_DETAIL=low|medium|high|ultra|full (how much a city draws: see `stream::Detail`; else the graphics menu's, medium by default; full draws everything at full detail as before streaming), AC1_FREECAM="x,y,z" (start in the free camera there),
//! AC1_JUMP=secs[,secs...] (press Space then; with AC1_WALK for a running jump), AC1_HIGH / AC1_LEGS=from-to
//! (hold high profile / the legs), AC1_HAND=secs[,..] (the empty hand),
//! AC1_ORBIT="x,y,z,radius,height,secs" (a camera circling a point, for videos) with AC1_FRAMES=dir (every frame saved
//! as dir/frame_00000.png ..., then exit; encode with ffmpeg at 30 fps),
//! AC1_CAM="yaw,pitch,dist[,focus height]", AC1_SHOT=path.png (saved after AC1_SHOT_SECS, then exit; `-` exits then without one), AC1_BACKGROUND=1 (open
//! on the second monitor if any, unfocused, behind other windows), AC1_WINDOW_AT="x,y" and AC1_WINDOW_SIZE="w,h" (place the window), AC1_NO_IK=1, AC1_PLAIN_LOOK=1 (Altaïr without AC1's character shader), AC1_NO_TEXT=1 (start with the text hidden, as F1),
//! AC1_ANIM=<clip name> loops one clip, AC1_NO_ANIM=1 uses procedural locomotion only. AC1_CONTROL=<file> stays open and runs
//! the scripts written to that file (`run <id> <secs> AC1_START=.. AC1_WALK=.. ...`, `quit`; see `control_runs`): the level
//! loads once, each run takes seconds. AC1_GROUND_LINE, AC1_HOLDS_AT and AC1_MESHES_AT log the ground along a line, the
//! holds and the city meshes near a point; AC1_GRAB_AUDIT=file.tsv writes the authored guidance against ours' holds
//! (`grab_audit`). Online: AC1_HOST=port / AC1_JOIN=ip:port (start a session, as the F10 menu
//! does), AC1_RECORD_REPLAY=1 (record a replay from the start), AC1_REPLAY=<file> (play one back; AC1_REPLAY_VIEW,
//! AC1_REPLAY_FRAMES=dir with AC1_REPLAY_FROM / _TO renders it, see `replay`). Settings: AC1_SETTINGS=<file> (read and
//! write that settings file, scripted runs too), AC1_NO_SETTINGS=1 (the defaults), AC1_MENU=graphics|online (open a menu
//! from the start, for screenshots).

mod animation;
mod assets;
mod beam_pairs;
mod character;
mod character_material;
mod city;
mod city_material;
mod climb;
mod climb_table;
mod crowd;
mod fly;
mod gait;
mod gallery;
mod grab_audit;
mod jump;
mod jump_query;
mod level;
mod look;
mod menu;
mod move_blend;
mod nav;
mod net;
mod npc;
mod pad;
mod probe;
mod ragdoll;
mod recorder;
mod replay;
mod robe;
mod settings;
mod sound;
mod stream;
mod wind;

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
/// The camera's yaw jumping more than this in a frame (rad, 30 degrees) is a cut: the stick keeps the old frame.
const CAMERA_CUT: f32 = std::f32::consts::FRAC_PI_6;

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
    /// Press the head button (E) at this time.
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
    /// The empty hand held (from-to): AC1's manual catch in a fall.
    grasp: Option<(f32, f32)>,
    /// AC1's Ghost mode flight (from-to), and up / down held in it (`fly`).
    fly: Option<(f32, f32)>,
    fly_up: Option<(f32, f32)>,
    fly_down: Option<(f32, f32)>,
}

#[derive(Component)]
struct Hud;

/// F1 hides the text over the game (the HUD and the world labels); F2 opens the map menu.
#[derive(Resource, Default)]
struct Overlay {
    hidden: bool,
    menu: bool,
}

/// The map menu (F2).
#[derive(Component)]
struct MapMenu;

/// The maps F2 offers: (key, name, `AC1_LEVEL`; `None` the test level).
const MAPS: [(KeyCode, &str, Option<&str>); 3] =
    [(KeyCode::Digit1, "Test level", None), (KeyCode::Digit2, "Masyaf", Some("masyaf")), (KeyCode::Digit3, "Damascus", Some("damascus"))];

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
        grasp: range(get("AC1_GRASP")),
        fly: range(get("AC1_FLY")),
        fly_up: range(get("AC1_FLY_UP")),
        fly_down: range(get("AC1_FLY_DOWN")),
        hand: get("AC1_HAND").ok().map(|s| floats(&s)).unwrap_or_default(),
    }
}

/// When the process started (`AC1_FPS=1` logs the time to the first frame from it).
static LAUNCH: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

fn main() {
    LAUNCH.get_or_init(std::time::Instant::now);
    // A replay plays in the level it was recorded in.
    if let Ok(path) = std::env::var("AC1_REPLAY") {
        match replay::level_of(&path) {
            // SAFETY: before any other thread starts.
            Some(level) if !level.is_empty() => unsafe { std::env::set_var("AC1_LEVEL", level) },
            Some(_) => unsafe { std::env::remove_var("AC1_LEVEL") },
            None => eprintln!("replay: {path} is not an ac1-rs replay"),
        }
    }
    let game_dir = std::env::var("AC1_GAME_DIR").unwrap_or_else(|_| r"P:\SteamLibrary\steamapps\common\Assassins Creed".into());
    let cam = env_f32s("AC1_CAM").unwrap_or_default();
    let script = script_from(&|k| std::env::var(k).map_err(|_| ()));
    let headless = std::env::var("AC1_HEADLESS").is_ok();
    let mut app = App::new();
    // (The network test, `net`: only with AC1_HOST or AC1_JOIN.)
    if let Some(n) = net::open() {
        app.insert_resource(n);
    }
    // Replays (`replay`): recording online (or asked for), or playing one back.
    if let Ok(path) = std::env::var("AC1_REPLAY") {
        match replay::load(&path) {
            Ok(frames) => {
                eprintln!("replay: {path}, {} frames, {:.0} s", frames.len(), frames.last().map_or(0.0, |f| f.t));
                let from: f32 = std::env::var("AC1_REPLAY_FROM").ok().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let to: f32 = std::env::var("AC1_REPLAY_TO").ok().and_then(|s| s.parse().ok()).unwrap_or(f32::MAX);
                let render = std::env::var("AC1_REPLAY_FRAMES").ok().map(|dir| {
                    let _ = std::fs::create_dir_all(&dir);
                    (dir, 0, to)
                });
                let view = match std::env::var("AC1_REPLAY_VIEW").unwrap_or_default().as_str() {
                    "ours" => replay::View::Ours,
                    "friend" => replay::View::Friend,
                    "free" => replay::View::Free,
                    _ => replay::View::Follow,
                };
                app.insert_resource(replay::Replay { frames, t: from, playing: true, speed: 1.0, view, render });
            }
            Err(e) => eprintln!("replay: {e}"),
        }
    } else if replay::recording_wanted() {
        app.insert_resource(replay::Recorder::new(std::env::var("AC1_LEVEL").unwrap_or_default()));
    }
    // (The controller only with `AC1_PAD=1`: Bevy's gamepad backend takes the pad over even with the window in the
    // background, from every copy the tests start, and another game could not use it.)
    let plugins = DefaultPlugins.build();
    let plugins = if std::env::var("AC1_PAD").is_ok() { plugins } else { plugins.disable::<bevy::gilrs::GilrsPlugin>() };
    if headless {
        // `AC1_HEADLESS=1`: no window and no GPU (the scenarios read only the log): the frames run back to back.
        app.add_plugins(
            plugins
                .set(WindowPlugin { primary_window: None, exit_condition: bevy::window::ExitCondition::DontExit, ..default() })
                .set(bevy::render::RenderPlugin { render_creation: bevy::render::settings::WgpuSettings { backends: None, ..default() }.into(), ..default() })
                .disable::<bevy::winit::WinitPlugin>()
                // (No audio device needed: nothing plays headless.)
                .disable::<bevy::audio::AudioPlugin>(),
        )
        .add_plugins(bevy::app::ScheduleRunnerPlugin::run_loop(std::time::Duration::ZERO));
    } else {
        app.add_plugins(plugins.set(WindowPlugin { primary_window: Some(game_window()), ..default() })).add_plugins(sound::SoundPlugin);
    }
    // `AC1_STEP=hz` (60 when headless): every frame is that long in game time, however long it takes to run, so a run is
    // the same every time and as fast as the machine goes (a loaded machine slows it down, it no longer changes it).
    let step = std::env::var("AC1_STEP").ok().and_then(|v| v.parse::<f64>().ok()).or(headless.then_some(60.0));
    if let Some(hz) = step.filter(|hz| *hz > 0.0) {
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f64(1.0 / hz)));
    }
    if std::env::var("AC1_FPS").is_ok() && !headless {
        app.add_plugins(bevy::render::diagnostic::RenderDiagnosticsPlugin);
    }
    // The level's light and grade from AC1's data (`look`): the city's own, Masyaf's for the test level.
    let level_name = std::env::var("AC1_LEVEL").ok();
    let world_look = look::Look::load(std::path::Path::new(&game_dir), level_name.as_deref()).unwrap_or_default();
    let f = world_look.world.fog_color;
    let env = world_look.world.env_cube;
    let ambient = env.iter().map(|c| c.truncate()).sum::<Vec3>() / 6.0;
    app.add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default())
        // The sky: AC1 draws a sky dome (`LayeredSky`'s layers, not drawn here); in its place the fog's colour halfway to
        // white, graded, as pale as the running game's haze (tuned by eye).
        .insert_resource(ClearColor(world_look.graded([(1.0 + f.x) * 0.5, (1.0 + f.y) * 0.5, (1.0 + f.z) * 0.5])))
        // (What Bevy's own materials get for ambient: the environment cube's mean; AC1's shaders use the cube.)
        .insert_resource(GlobalAmbientLight { color: Color::linear_rgb(ambient.x, ambient.y, ambient.z), brightness: 1.0, ..default() })
        .insert_resource(world_look)
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
            edges: if (std::env::var("AC1_SHOT").is_ok() || std::env::var("AC1_FRAMES").is_ok() || replay::replaying()) && std::env::var("AC1_EDGES").is_err() {
                3
            } else {
                0
            },
        })
        .insert_resource(script)
        .add_plugins((look::LookPlugin, character_material::CharacterMaterialPlugin, city_material::CityMaterialPlugin))
        .add_systems(Startup, (level::spawn_level, setup, grab_cursor, slow_motion).chain())
        .init_resource::<Eagle>()
        .init_resource::<fly::Fly>()
        .init_resource::<recorder::Recorder>()
        .init_resource::<pad::PadStick>()
        .add_systems(Update, place_background_window.run_if(|| std::env::var("AC1_BACKGROUND").is_ok() && std::env::var("AC1_WINDOW_AT").is_err()))
        .add_systems(Update, pad::log_connected)
        .add_systems(
            Update,
            (
                player_input,
                run_script,
                fly::fly,
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
        .insert_resource(Overlay { hidden: std::env::var("AC1_NO_TEXT").is_ok(), ..default() })
        // The graphics (F8) and online (F10) menus, and the settings they keep (`settings`, `menu`).
        .insert_resource(settings::started().clone())
        .init_resource::<menu::Menu>()
        .add_systems(Startup, menu::spawn.run_if(move || !headless))
        .add_systems(PreUpdate, menu::input.after(bevy::ui::UiSystems::Focus).run_if(move || !headless))
        .add_systems(Update, (settings::apply, menu::draw, menu::restart).run_if(move || !headless))
        .add_systems(Last, settings::cap_frames.run_if(move || !headless))
        .add_systems(Update, overlay_keys)
        .init_resource::<probe::Probe>()
        .add_systems(Update, probe::probe_climbs.after(run_script).before(character::locomotion))
        .init_resource::<ScriptClock>()
        .add_systems(First, tick_script_clock)
        .add_systems(Update, control_runs.before(run_script))
        .init_resource::<character::StatuePlay>()
        .add_systems(Update, pause_statues)
        // (What a city draws follows the camera.)
        .add_systems(Update, stream::stream_city.after(camera_follow))
        .add_systems(
            Update,
            (net::spawn_friend, net::sync, net::ground_remote, net::apply)
                .chain()
                .after(character::animate)
                .before(camera_follow)
                .run_if(resource_exists::<net::Net>),
        )
        .add_systems(Update, net::hide_friend.run_if(not(resource_exists::<net::Net>).and_then(not(resource_exists::<replay::Replay>))))
        .add_systems(Update, replay::record.after(camera_follow).after(net::apply).run_if(resource_exists::<replay::Recorder>))
        .add_systems(
            Update,
            (replay::controls, net::ground_remote, replay::apply, replay_view)
                .chain()
                .after(character::animate)
                .before(camera_follow)
                .run_if(resource_exists::<replay::Replay>),
        )
        .add_systems(Update, (replay::camera, replay::render).chain().after(camera_follow).run_if(resource_exists::<replay::Replay>))
        // The robe needs the bones' world transforms of this frame.
        .add_systems(PostUpdate, robe::robe_cloth.after(bevy::transform::TransformSystems::Propagate))
        .add_systems(First, wind::advance)
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
    // (`AC1_WINDOW_SIZE="w,h"`: its size; else the graphics menu's, as its window mode and vsync.)
    let s = settings::started();
    let size = env_f32s("AC1_WINDOW_SIZE").filter(|v| v.len() >= 2).map_or(s.resolution, |v| (v[0] as u32, v[1] as u32));
    let window = Window {
        title: "ac1-rs".into(),
        resolution: size.into(),
        mode: if std::env::var("AC1_BACKGROUND").is_ok() { bevy::window::WindowMode::Windowed } else { settings::window_mode(s.window) },
        present_mode: settings::present_mode(s.vsync),
        ..default()
    };
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
    // (Made hidden, and shown once `place_background_window` has put it on a monitor other than the main one: asked for
    // by index, winit had no monitors yet as the window was made and left it in front on the main screen.)
    Window { focused: false, visible: false, window_level: bevy::window::WindowLevel::AlwaysOnBottom, ..window }
}

/// In the background (`AC1_BACKGROUND`, no `AC1_WINDOW_AT`): move the hidden window to the centre of a monitor that is
/// not the main one (the main one if there is no other) once the monitors are known, then show it.
fn place_background_window(
    mut frames: Local<u32>,
    monitors: Query<(&bevy::window::Monitor, Has<bevy::window::PrimaryMonitor>)>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
) {
    let Ok(mut w) = windows.single_mut() else { return };
    if w.visible {
        return;
    }
    *frames += 1;
    let mons: Vec<_> = monitors.iter().collect();
    if mons.is_empty() && *frames < 60 {
        return;
    }
    if let Some((m, _)) = mons.iter().find(|(_, primary)| !primary).or(mons.first()) {
        let (ww, wh) = (w.resolution.physical_width() as i32, w.resolution.physical_height() as i32);
        let at = m.physical_position + IVec2::new(((m.physical_width as i32 - ww) / 2).max(0), ((m.physical_height as i32 - wh) / 2).max(0));
        w.position = bevy::window::WindowPosition::At(at);
    }
    w.visible = true;
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

/// `AC1_FPS=1`: log the time from launch to the first frame, then the frame rate every two seconds.
fn fps_log(time: Res<Time>, mut acc: Local<(f32, u32)>, mut first: Local<bool>, diag: Res<bevy::diagnostic::DiagnosticsStore>) {
    if std::env::var("AC1_FPS").is_err() {
        return;
    }
    if !std::mem::replace(&mut *first, true) {
        info!("first frame {:.2} s after launch", LAUNCH.get().map_or(0.0, |t| t.elapsed().as_secs_f32()));
    }
    acc.0 += time.delta_secs();
    acc.1 += 1;
    if acc.0 >= 2.0 {
        // (With the render diagnostics: the GPU's time a frame, its top-level passes summed, which the display's refresh
        // does not cap.)
        let gpu: f64 = diag
            .iter()
            .filter(|d| d.path().as_str().ends_with("/elapsed_gpu") && d.path().as_str().matches('/').count() == 2)
            .filter_map(|d| d.average())
            .sum();
        for d in diag.iter().filter(|d| d.path().as_str().ends_with("elapsed_gpu")) {
            debug!("{} {:?}", d.path().as_str(), d.average());
        }
        info!("fps {:.0}, GPU {gpu:.2} ms a frame", acc.1 as f32 / acc.0);
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

#[allow(clippy::too_many_arguments)]
fn setup(
    mut commands: Commands,
    dir: Res<GameDir>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    level: Res<level::Level>,
    look: Res<look::Look>,
) {
    // The sun where the level's `LayeredSky` puts it (`look`), with AC1's colour; a city's shadows in three cascades kept
    // near the player (the first 12 m, none past 120 m: the default four spread over the whole view and blur his
    // shadow). The camera writes the shaders' colours as they are (exposure 1, no tone mapping: AC1's frame is
    // the lit colours, clamped), and Bevy's own materials get the level's fog.
    let city = std::env::var("AC1_LEVEL").is_ok();
    let s = look.world.sun;
    let lum = s.x.max(s.y).max(s.z).max(1e-3);
    let mut sun = commands.spawn((
        DirectionalLight {
            // (Bevy's Lambert divides by pi.)
            illuminance: lum * std::f32::consts::PI,
            color: Color::srgb(s.x / lum, s.y / lum, s.z / lum),
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_translation(look.sun_dir).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    if city {
        sun.insert(bevy::light::CascadeShadowConfigBuilder { num_cascades: 3, first_cascade_far_bound: 12.0, maximum_distance: 120.0, ..default() }.build());
    }
    // (Cities are a couple of kilometres across.)
    let f = look.world.fog_color;
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection { far: 5000.0, ..default() }),
        Transform::from_xyz(0.0, 2.0, 4.0).looking_at(Vec3::Y, Vec3::Y),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        bevy::camera::Exposure { ev100: -(1.2f32).log2() },
        DistanceFog {
            color: Color::srgba(f.x, f.y, f.z, look.world.fog.z * look.world.fog.w),
            falloff: FogFalloff::Linear { start: look.fog_near, end: look.fog_far },
            ..default()
        },
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
    let now = std::env::var("AC1_LEVEL").ok().map(|l| l.to_lowercase());
    let mut menu = String::from("Maps (F2 or Esc closes)\n");
    for (i, (_, name, level)) in MAPS.iter().enumerate() {
        let here = if now.as_deref() == *level { "  (now)" } else { "" };
        menu += &format!("\n{}  {name}{here}", i + 1);
    }
    commands.spawn((
        MapMenu,
        Text::new(menu),
        TextFont { font_size: bevy::text::FontSize::Px(22.0), ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        Node { position_type: PositionType::Absolute, left: percent(38), top: percent(35), padding: UiRect::all(px(16)), ..default() },
        Visibility::Hidden,
        GlobalZIndex(10),
    ));

    // The city's wind (the test level borrows Masyaf's): the robe and the skeleton's hinges feel it.
    commands.insert_resource(wind::load(&dir.0, now.as_deref()));
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
    // (A replay's friend: a second Altaïr, posed from the recording; online, `net::spawn_friend` makes theirs.)
    if replay::replaying() {
        let e = character::spawn_character(&mut commands, data.clone(), at, &mut meshes, &mut mats, &mut images, &mut bindposes, None, 0);
        commands.entity(e).insert((character::Statue, net::Remote::default(), Visibility::Hidden));
    }
    let root = character::spawn_character(&mut commands, data, at, &mut meshes, &mut mats, &mut images, &mut bindposes, animator, 0);
    // (Playing a replay, the player is a puppet too: posed from the recording, not simulated.)
    if replay::replaying() {
        commands.entity(root).insert(character::Statue);
    }
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

/// The arrow keys turn the camera this fast (rad/s): AC1's keyboard binds them to the right stick.
const ARROW_YAW_RATE: f32 = 2.5;
const ARROW_PITCH_RATE: f32 = 1.5;
/// A press of the high-profile button shorter than this (s) flips it, with AC1's `HighProfileToggle` (0x90B080).
const HIGH_TOGGLE_TAP: f32 = 0.5;

/// AC1's high-profile toggle: `HighProfileToggle=1` under `[Input]` in the player's own `Assassin.ini` (in
/// `%APPDATA%\Ubisoft\Assassin's Creed`), or `AC1_HIGH_TOGGLE=1`. Off (the button held), as AC1's default.
fn high_profile_toggle() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        if let Ok(v) = std::env::var("AC1_HIGH_TOGGLE") {
            return v == "1";
        }
        let Some(ini) = std::env::var_os("APPDATA").map(|d| std::path::PathBuf::from(d).join("Ubisoft").join("Assassin's Creed").join("Assassin.ini")) else {
            return false;
        };
        std::fs::read_to_string(ini).is_ok_and(|s| ini_flag(&s, "Input", "HighProfileToggle"))
    })
}

/// Whether `key` in `[section]` of an ini file's text is set to a non-zero number.
fn ini_flag(text: &str, section: &str, key: &str) -> bool {
    let mut inside = false;
    for line in text.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            inside = name.eq_ignore_ascii_case(section);
        } else if inside
            && let Some((k, v)) = line.split_once('=')
            && k.trim().eq_ignore_ascii_case(key)
        {
            return v.trim().parse::<i32>().is_ok_and(|n| n != 0);
        }
    }
    false
}

/// The high-profile button with the toggle on: held, on; a tap under `HIGH_TOGGLE_TAP` flips it on or off.
#[derive(Default)]
struct HighToggle {
    on: bool,
    was: bool,
    held: f32,
}

impl HighToggle {
    fn update(&mut self, pressed: bool, dt: f32) -> bool {
        if pressed {
            self.held = if self.was { self.held + dt } else { 0.0 };
        } else if self.was && self.held < HIGH_TOGGLE_TAP {
            self.on = !self.on;
        }
        self.was = pressed;
        pressed || self.on
    }
}

/// `player_input`'s latches: the legs' press buffer, the high-profile toggle, and (once set) whether the player wants
/// the cursor held with whether the window had focus last frame.
type InputLatches = (f32, HighToggle, Option<(bool, bool)>);

#[allow(clippy::too_many_arguments)]
fn player_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    mut cam: ResMut<OrbitCam>,
    mut dbg: ResMut<Debug>,
    mut q: Query<(&mut Controller, &mut Character, &Transform), With<Player>>,
    (script, mut fly): (Res<Script>, ResMut<fly::Fly>),
    stick: Res<pad::PadStick>,
    mut lib: Option<ResMut<animation::AnimLib>>,
    time: Res<Time>,
    mut cursor: Query<(&mut bevy::window::CursorOptions, &bevy::window::Window), With<bevy::window::PrimaryWindow>>,
    scholars: Query<&Transform, (With<crowd::Scholar>, Without<Player>)>,
    mut buttons: Local<InputLatches>,
    mut climb_latch: Local<(Vec2, f32)>,
    // (The frame the stick is held in after a camera cut, with the stick it was held at.)
    mut stick_yaw: Local<Option<(f32, Vec2)>>,
) {
    let (legs_buffer, high_toggle, cursor_state) = &mut *buttons;
    // Scripted captures must not pick up stray keystrokes.
    if script.shot.is_some() {
        return;
    }
    // The mouse looks around while the cursor is held (Esc lets it go, a click takes it back). Windows lets the cursor
    // out of the window whenever the window loses focus (and a lock asked for before the window has focus never
    // takes), so the lock is asked for again each time the window gets focus back.
    let (want_held, had_focus) = cursor_state.get_or_insert((true, false));
    let mut grabbed = false;
    if let Ok((mut c, window)) = cursor.single_mut() {
        let focused = window.focused;
        let lock = |c: &mut bevy::window::CursorOptions| {
            c.grab_mode = bevy::window::CursorGrabMode::Locked;
            c.visible = false;
        };
        if keys.just_pressed(KeyCode::Escape) {
            *want_held = false;
            c.grab_mode = bevy::window::CursorGrabMode::None;
            c.visible = true;
        } else if focused && c.visible && (mouse.just_pressed(MouseButton::Left) || mouse.just_pressed(MouseButton::Right)) {
            *want_held = true;
            *had_focus = true;
            lock(&mut c);
            return;
        } else if focused && !*had_focus && *want_held {
            lock(&mut c);
        }
        *had_focus = focused;
        grabbed = !c.visible;
    }
    if keys.just_pressed(KeyCode::KeyP) {
        cam.free = if cam.free.is_some() { None } else { Some(cam.eye) };
    }
    // C centres the camera behind Altair (AC1's Center Camera).
    let before_centre = cam.yaw;
    if keys.just_pressed(KeyCode::KeyC)
        && cam.free.is_none()
        && let Ok((_, _, tf)) = q.single()
    {
        cam.yaw = tf.rotation.to_euler(EulerRot::YXZ).0;
    }
    // (How far C's centring turned it: a cut, by the code, not the player's turn.)
    let centred = (cam.yaw - before_centre + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
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
        let speed = if keys.pressed(KeyCode::ShiftLeft) {
            FREECAM_FAST
        } else if keys.pressed(KeyCode::AltLeft) {
            FREECAM_SLOW
        } else {
            FREECAM_SPEED
        };
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
    // The arrow keys turn the camera as the mouse does: AC1's keyboard right stick (`DefaultBindings.map`, audit INPUT-10).
    let key = |k: KeyCode| -> f32 { if keys.pressed(k) { 1.0 } else { 0.0 } };
    let arrows = Vec2::new(key(KeyCode::ArrowRight) - key(KeyCode::ArrowLeft), key(KeyCode::ArrowUp) - key(KeyCode::ArrowDown));
    if arrows != Vec2::ZERO {
        cam.yaw -= arrows.x * ARROW_YAW_RATE * time.delta_secs();
        cam.pitch = (cam.pitch + arrows.y * ARROW_PITCH_RATE * time.delta_secs()).clamp(-1.5, 0.6);
    }
    if keys.just_pressed(KeyCode::F5) {
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
    if keys.just_pressed(KeyCode::F4) {
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
    // (After a camera cut, a jump of the camera's yaw over 30 degrees in a frame, the stick keeps the old camera's frame
    // till it moves more than 0.3: AC1's, reanalysis INPUT-8, audit INPUT-14; ours re-aimed a running Altaïr at once.)
    // (A cut is the camera jumping by itself, C's centring; the player's own turns (the mouse, the arrows, the pad's
    // right stick) re-aim the stick at once, as in AC1: latched on any change of the camera's yaw, a held W ran on along
    // the camera of the moment it was pressed, and the other keys with it.)
    if stick_yaw.is_some_and(|(_, at)| (input - at).length() > 0.3 || input == Vec2::ZERO) {
        *stick_yaw = None;
    }
    if stick_yaw.is_none() && centred.abs() > CAMERA_CUT && input != Vec2::ZERO {
        *stick_yaw = Some((before_centre, input));
    }
    let yaw = stick_yaw.map_or(cam.yaw, |(y, _)| y);
    let fwd = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    ctl.move_dir = fwd * input.y + right * input.x;
    // AC1's puppet controls: the right button is the high profile, Space the legs, Shift the empty hand
    // (see the module docs).
    // (With AC1's `HighProfileToggle` set, a tap of under 0.5 s flips it on or off, held it is on as ever.)
    let high_button = mouse.pressed(MouseButton::Right) || keys.pressed(KeyCode::KeyK);
    let high = if high_profile_toggle() { high_toggle.update(high_button, time.delta_secs()) } else { high_button };
    let legs = keys.pressed(KeyCode::Space);
    let hand = keys.just_pressed(KeyCode::ShiftLeft) || keys.just_pressed(KeyCode::ShiftRight);
    ctl.hand_held = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    ctl.high = high;
    // F7: AC1's Ghost mode flight (`fly`), in or out. Flying, Space is up, Left Ctrl down, Left Alt × 10 (high profile
    // × 3), Q held strafes; nothing else of the ground's.
    fly.toggle |= keys.just_pressed(KeyCode::F7);
    if fly.active || fly.toggle {
        fly.lift = key(KeyCode::Space) - key(KeyCode::ControlLeft);
        fly.faster = keys.pressed(KeyCode::AltLeft);
        fly.strafe = keys.pressed(KeyCode::KeyQ);
        ctl.free_run = false;
        ctl.blend_walk = false;
        ctl.leap = false;
        ctl.climb_dir = Vec2::ZERO;
        *legs_buffer = 0.0;
        return;
    }
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
    // The climbing stick (ledge, climb, ladder) in the character's frame, its left and right mirrored while the camera
    // faces him (AC1's `sub_EDB450`: the camera's forward against his under -0.1736), the mirroring kept until the stick
    // moves more than 0.3 (AC1's latch at +0x1310; reanalysis, audit INPUT-6 / LEDGE-11).
    let facing = (tf.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
    if (input - climb_latch.0).length() > 0.3 || climb_latch.1 == 0.0 {
        let mirror = fwd.dot(facing) < -0.1736;
        *climb_latch = (input, if mirror { -1.0 } else { 1.0 });
    }
    ctl.climb_dir = Vec2::new(input.x * climb_latch.1, input.y);
    // On a wall the legs leap (with a direction), jump off a perch or rebound, and the hand lets go.
    // On the ground the legs jump only with a direction in high profile; else they only grab what is in reach.
    let press = keys.just_pressed(KeyCode::Space);
    // AC1 holds a Legs press for 0.3 s (`GoAssassinActionInterpreter`): on the ground a press just before a target
    // comes in reach still jumps at it.
    *legs_buffer = if press { LEGS_BUFFER } else { (*legs_buffer - time.delta_secs()).max(0.0) };
    ctl.legs_fresh = legs && *legs_buffer > 0.0;
    ctl.leap = legs;
    // (A stop at an edge or leaning on a wall is not on a wall: the legs act as on the ground.)
    let on_wall = ch.wall.as_ref().is_some_and(|w| !w.legs_on_ground());
    if on_wall {
        ctl.wall_legs |= press;
        ctl.toggle_climb |= hand;
    } else {
        // (On the ground: AC1's press runs up a wall ahead; its release within the buffer is the tap jump.)
        ctl.legs_press |= press;
        // (Or crouched for the static jump, whose own window is 0.5 s.)
        let crouched = ch.wall.as_ref().is_some_and(|w| w.crouch_held().is_some());
        if keys.just_released(KeyCode::Space) && (*legs_buffer > 0.0 || crouched) {
            ctl.legs_release = true;
            *legs_buffer = 0.0;
        }
        ctl.hand_climb |= hand;
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
/// F1 hides the text over the game; F2 opens the map menu, where 1-3 start the game again in that map (the level is
/// loaded once, at start: a new process, with `AC1_LEVEL` set, is the clean way to another one).
fn overlay_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut overlay: ResMut<Overlay>,
    mut menu: Query<&mut Visibility, With<MapMenu>>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::F1) {
        overlay.hidden ^= true;
    }
    if keys.just_pressed(KeyCode::F2) {
        overlay.menu ^= true;
    } else if overlay.menu && keys.just_pressed(KeyCode::Escape) {
        overlay.menu = false;
    }
    if overlay.menu
        && let Some((_, name, level)) = MAPS.iter().find(|(k, _, _)| keys.just_pressed(*k))
    {
        let Ok(exe) = std::env::current_exe() else { return };
        let mut c = std::process::Command::new(exe);
        c.args(std::env::args_os().skip(1));
        match level {
            Some(l) => c.env("AC1_LEVEL", l),
            None => c.env_remove("AC1_LEVEL"),
        };
        // (A start or free-camera spot belongs to the map it was given for.)
        c.env_remove("AC1_START").env_remove("AC1_FREECAM");
        match c.spawn() {
            Ok(_) => {
                info!("map menu: starting again in {name}");
                exit.write(AppExit::Success);
            }
            Err(e) => warn!("map menu: could not start the game again: {e}"),
        }
        overlay.menu = false;
    }
    if let Ok(mut vis) = menu.single_mut() {
        vis.set_if_neq(if overlay.menu { Visibility::Visible } else { Visibility::Hidden });
    }
}

fn place_labels(
    cams: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    things: Query<&GlobalTransform, Without<Camera3d>>,
    mut labels: Query<(&WorldLabel, &mut Node, &mut Visibility, &ComputedNode)>,
    overlay: Res<Overlay>,
) {
    if overlay.hidden {
        for (_, _, mut vis, _) in &mut labels {
            vis.set_if_neq(Visibility::Hidden);
        }
        return;
    }
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

/// AC1's head button (E): at a viewpoint, synchronize (the camera sweeps round Altaïr); elsewhere, held,
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

fn run_script(
    clock: Res<ScriptClock>,
    script: Res<Script>,
    mut fly: ResMut<fly::Fly>,
    mut q: Query<(&mut Controller, &Transform, &mut Character), With<Player>>,
) {
    let Ok((mut ctl, tf, mut ch)) = q.single_mut() else { return };
    let t = clock.t;
    // The Ghost mode flight over its window, up and down held in it.
    let within = |r: Option<(f32, f32)>| r.is_some_and(|(a, b)| (a..b).contains(&t));
    if script.fly.is_some() {
        fly.toggle = within(script.fly) != fly.active;
        fly.lift = within(script.fly_up) as i32 as f32 - within(script.fly_down) as i32 as f32;
    }
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
            // (A tap, as a real one: pressed and let go, AC1's jump firing on the release.)
            ctl.legs_press = true;
            ctl.legs_release = true;
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
        // Pressing the legs on the ground (as the player's Space); let go within 0.3 s, the tap.
        // (The release whatever is playing: it ends the static jump's crouch.)
        if let Some((a, b)) = script.legs
            && (ch.wall.is_none() || ch.wall.as_ref().is_some_and(|w| w.crouch_held().is_some()))
        {
            if t > a && t - clock.dt <= a && ch.wall.is_none() {
                ctl.legs_press = true;
            }
            if b - a <= LEGS_BUFFER && t > b && t - clock.dt <= b {
                ctl.legs_release = true;
            }
            ctl.legs_fresh = (a..a + LEGS_BUFFER).contains(&t) && t < b;
        }
    }
    if let Some((a, b)) = script.grasp {
        ctl.hand_held = (a..b).contains(&t);
    }
    if script.hand.iter().any(|&at| t > at && t - clock.dt <= at) {
        ctl.push = !ctl.high;
        ctl.hand_drop = character::HAND_DROP_BUFFER;
        ctl.hand_climb = ch.wall.is_none();
        // (On a wall, a beam or a post, as the key: the hand's let go there.)
        ctl.toggle_climb |= ch.wall.as_ref().is_some_and(|w| !w.legs_on_ground());
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
/// With Alt (m/s): slow passes for drone shots.
const FREECAM_SLOW: f32 = 2.5;

/// A replay's camera views: the free camera (3) flies from where the camera is; the others give it back.
fn replay_view(replay: Res<replay::Replay>, mut cam: ResMut<OrbitCam>) {
    match replay.view {
        replay::View::Free if cam.free.is_none() => cam.free = Some(cam.eye),
        replay::View::Free => {}
        _ => cam.free = None,
    }
}

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
    for l in level.bars.iter().filter(|l| near(l.a, l.b)) {
        gizmos.line(l.a, l.b, Color::srgb(0.75, 0.45, 0.2));
    }
    for k in level.kiosks.iter().filter(|k| near(k.edge.a, k.edge.b)) {
        gizmos.line(k.edge.a, k.edge.b, Color::srgb(0.75, 0.45, 0.2));
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

#[allow(clippy::too_many_arguments)]
fn hud(
    replay: Option<Res<replay::Replay>>,
    chars: Query<(&Character, &Controller, &Transform), With<Player>>,
    mut text: Query<(&mut Text, &mut Visibility), With<Hud>>,
    cam: Res<OrbitCam>,
    diag: Res<bevy::diagnostic::DiagnosticsStore>,
    level: Res<level::Level>,
    mut contexts: Local<(String, String)>,
    eagle: Res<Eagle>,
    overlay: Res<Overlay>,
    net: Option<Res<net::Net>>,
) {
    let fps = diag.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()).unwrap_or(0.0);
    let (Ok((ch, ctl, tf)), Ok((mut t, mut vis))) = (chars.single(), text.single_mut()) else { return };
    // (Recording a video, or rendering a replay: no HUD in the frames. F1: hidden.)
    if std::env::var("AC1_FRAMES").is_ok() || overlay.hidden || replay.as_ref().is_some_and(|r| r.render.is_some()) {
        vis.set_if_neq(Visibility::Hidden);
        return;
    }
    vis.set_if_neq(Visibility::Inherited);
    if let Some(r) = &replay {
        t.0 = format!(
            "replay {:.1} / {:.1} s | {} x{} | camera {:?}\nEnter pause | Left/Right 5 s (Shift 1 s) | , . a frame | Up/Down speed | Home start | 1 ours 2 friend's 3 free (Alt slow, Shift fast) 4 follow | F1 hide",
            r.t,
            r.end(),
            if r.playing { "playing" } else { "paused" },
            r.speed,
            r.view
        );
        return;
    }
    // The detail lines: what the body is doing (context, previous, the move), the ground gait, the input,
    // the air and the last landing.
    let context = match &ch.wall {
        Some(w) => format!("Climb ({})", w.state),
        None if ch.flying => "Debug (Ghost mode)".to_string(),
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
    } else if ch.flying {
        "Ghost mode flight (F7 lands): Space up, Ctrl down, RMB x3, Alt x10, Q strafe".to_string()
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
        "ac1-rs | {fps:.0} fps | {mode} | {profile} | health {} | speed {:.1} m/s | IK {} (F4)\nMouse look (Esc frees the cursor) | RMB high profile | Space legs: blend, or with RMB free-run / jump / leap | Shift hand: push, let go | P free cam | F7 fly (Ghost mode) | G outlines (both, grab, edge, none) | E eagle | C centre camera | [ ] clips, F1 hide text, F2 maps, F7 fly, F8 graphics, F10 online, M mute, F5 skeleton, F3 IK targets, F6 gallery, F12 shot\n{details}{online}",
        if ch.health <= 0.0 {
            "DESYNCHRONISED".to_string()
        } else {
            (0..5).map(|i| if ch.health > i as f32 * 0.2 + 0.1 { '#' } else { '-' }).collect::<String>()
        },
        ch.velocity.length(),
        if ch.ik_enabled { "on" } else { "off" },
        online = net.map_or(String::new(), |n| format!("\nonline: {}", n.status())),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_toggle_flag_is_read_from_its_section() {
        let ini = "[Video]\nHighProfileToggle=1\n[Input]\nSelectedInput=KeyboardMouse2\n HighProfileToggle = 1 \n";
        assert!(ini_flag(ini, "Input", "HighProfileToggle"));
        assert!(!ini_flag("[Input]\nHighProfileToggle=0\n", "Input", "HighProfileToggle"));
        assert!(!ini_flag("[Video]\nHighProfileToggle=1\n", "Input", "HighProfileToggle"));
    }

    #[test]
    fn a_tap_flips_high_profile_and_a_hold_does_not() {
        let mut t = HighToggle::default();
        let dt = 1.0 / 60.0;
        // A tap: on, and stays on.
        assert!(t.update(true, dt));
        assert!(t.update(false, dt));
        assert!(t.update(false, dt));
        // Another tap: off.
        t.update(true, dt);
        assert!(!t.update(false, dt));
        // Held a second: on while held, off after.
        for _ in 0..60 {
            assert!(t.update(true, dt));
        }
        assert!(!t.update(false, dt));
    }
}
