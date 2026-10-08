//! Replays (branch `feature/multiplayer`, for showcase videos): a session recorded frame by frame and played back
//! with any camera.
//!
//! Recording: online (`AC1_HOST` / `AC1_JOIN`) or with `AC1_RECORD_REPLAY=1`, every game writes
//! `replays/ac1-replay-<unix secs>.bin` from the start; F10 stops it, and F10 again starts a new file. Each frame (60
//! a second at most) holds both players as shown on this side (root and every bone) and both cameras: ours, and the
//! friend's, sent with their packets. One recording has both points of view.
//!
//! Playback: `AC1_REPLAY=<file>` loads the recording's level and poses both Altaïrs from it (nothing is simulated).
//! Keys: Enter pause, Left / Right 5 s back / on (with Shift 1 s), `,` / `.` a frame back / on while paused, Up / Down
//! speed x2 / /2, Home the start; 1 our camera as recorded, 2 the friend's, 3 the free camera (WASD, Space/E up,
//! Ctrl/Q down, Shift fast, Alt slow, the mouse looks), 4 following us; H hides the HUD (in any game).
//! `AC1_REPLAY_VIEW=ours|friend|free|follow` picks the camera to start with.
//! `AC1_REPLAY_FRAMES=dir` renders it to dir/frame_00000.png ... at 30 frames a second of the recording (from
//! `AC1_REPLAY_FROM` to `AC1_REPLAY_TO` seconds, if given), whatever the frame rate, then exits; encode with ffmpeg at
//! 30 fps.
//!
//! File (little endian): "AC1R", version u8, level name (u16 length, bytes), bone count u16; then frames: t f32 and for
//! us then the friend a u8 (0 absent, 1 present, 2 present with a camera): root position 3 x f32, rotation 4 x f32,
//! each bone's position 3 x f32 and rotation 4 x f32, then the camera's position and rotation.

use bevy::prelude::*;
use std::io::{BufWriter, Read, Write};

use crate::character::{Character, Player};

const MAGIC: &[u8; 4] = b"AC1R";
const VERSION: u8 = 1;
/// Frames recorded a second at most.
const RECORD_HZ: f32 = 60.0;
/// Frames a second of a rendered replay (`AC1_REPLAY_FRAMES`).
pub const RENDER_FPS: f32 = 30.0;
/// Seconds a rendered replay waits for the level to load in before its first frame.
const RENDER_WARMUP: f32 = 3.0;

/// A player in one recorded frame.
#[derive(Clone)]
pub struct Actor {
    pub root: (Vec3, Quat),
    pub bones: Vec<(Vec3, Quat)>,
    pub cam: Option<(Vec3, Quat)>,
}

pub struct Frame {
    pub t: f32,
    /// Us, then the friend.
    pub actors: [Option<Actor>; 2],
}

/// The friend's figure as shown.
type FriendShown = (&'static Transform, &'static Character, &'static Visibility);
/// A figure a replay poses: us (`Has<Player>`) or the friend.
type Figure = (&'static mut Transform, &'static mut Character, &'static mut Visibility, Has<Player>);

/// Both figures in a replay.
type Puppets = Or<(With<Player>, With<crate::net::Remote>)>;

/// Recording now.
#[derive(Resource)]
pub struct Recorder {
    out: Option<BufWriter<std::fs::File>>,
    t: f32,
    last: f32,
    since_flush: f32,
    level: String,
    /// The file's header is written.
    headed: bool,
}

/// What the camera shows in a replay.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Ours,
    Friend,
    Free,
    Follow,
}

/// Playing a recording back.
#[derive(Resource)]
pub struct Replay {
    pub frames: Vec<Frame>,
    pub t: f32,
    pub playing: bool,
    pub speed: f32,
    pub view: View,
    /// Rendering to frames: (dir, frame number, end time).
    pub render: Option<(String, u32, f32)>,
}

impl Replay {
    pub fn end(&self) -> f32 {
        self.frames.last().map_or(0.0, |f| f.t)
    }
}

/// Is a recording to be made (online, or asked for)?
pub fn recording_wanted() -> bool {
    crate::net::enabled() || std::env::var("AC1_RECORD_REPLAY").is_ok()
}

/// Is a replay to be played (`AC1_REPLAY`)?
pub fn replaying() -> bool {
    std::env::var("AC1_REPLAY").is_ok()
}

/// A replay's level, read from its file before the app starts (so `AC1_LEVEL` can be set to it).
pub fn level_of(path: &str) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut head = [0u8; 7];
    f.read_exact(&mut head).ok()?;
    if &head[0..4] != MAGIC {
        return None;
    }
    let n = u16::from_le_bytes([head[5], head[6]]) as usize;
    let mut name = vec![0u8; n];
    f.read_exact(&mut name).ok()?;
    String::from_utf8(name).ok()
}

impl Recorder {
    pub fn new(level: String) -> Self {
        let mut r = Recorder { out: None, t: 0.0, last: f32::NEG_INFINITY, since_flush: 0.0, level, headed: false };
        r.start();
        r
    }

    fn start(&mut self) {
        let _ = std::fs::create_dir_all("replays");
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let path = format!("replays/ac1-replay-{secs}.bin");
        match std::fs::File::create(&path) {
            Ok(f) => {
                info!("replay: recording to {path} (F10 stops)");
                self.out = Some(BufWriter::new(f));
                self.t = 0.0;
                self.last = f32::NEG_INFINITY;
                self.headed = false;
            }
            Err(e) => warn!("replay: cannot write {path}: {e}"),
        }
    }

    fn stop(&mut self) {
        if let Some(mut out) = self.out.take() {
            let _ = out.flush();
            info!("replay: recording stopped ({:.0} s)", self.t);
        }
    }
}

fn put_f32s(out: &mut Vec<u8>, v: &[f32]) {
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
}

fn put_actor(out: &mut Vec<u8>, a: Option<&Actor>) {
    let Some(a) = a else {
        out.push(0);
        return;
    };
    out.push(if a.cam.is_some() { 2 } else { 1 });
    let (p, r) = a.root;
    put_f32s(out, &[p.x, p.y, p.z, r.x, r.y, r.z, r.w]);
    for (p, r) in &a.bones {
        put_f32s(out, &[p.x, p.y, p.z, r.x, r.y, r.z, r.w]);
    }
    if let Some((p, r)) = a.cam {
        put_f32s(out, &[p.x, p.y, p.z, r.x, r.y, r.z, r.w]);
    }
}

fn actor_of(tf: &Transform, ch: &Character, cam: Option<(Vec3, Quat)>) -> Actor {
    Actor { root: (tf.translation, tf.rotation), bones: ch.pose.local.iter().map(|x| (x.pos, x.rot)).collect(), cam }
}

/// Record this frame (after the friend is posed and the camera placed). F10 stops and starts.
pub fn record(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut rec: ResMut<Recorder>,
    net: Option<Res<crate::net::Net>>,
    me: Query<(&Transform, &Character), With<Player>>,
    friend: Query<FriendShown, (With<crate::net::Remote>, Without<Player>)>,
    cams: Query<&Transform, (With<Camera3d>, Without<Character>)>,
) {
    if keys.just_pressed(KeyCode::F10) {
        if rec.out.is_some() {
            rec.stop();
        } else {
            rec.start();
        }
    }
    if rec.out.is_none() {
        return;
    }
    rec.t += time.delta_secs();
    rec.since_flush += time.delta_secs();
    if rec.t - rec.last < 1.0 / RECORD_HZ {
        return;
    }
    rec.last = rec.t;
    let Ok((tf, ch)) = me.single() else { return };
    let cam = cams.single().ok().map(|c| (c.translation, c.rotation));
    let ours = actor_of(tf, ch, cam);
    let theirs = friend.iter().find(|(_, _, v)| **v != Visibility::Hidden).map(|(tf, ch, _)| actor_of(tf, ch, net.as_ref().and_then(|n| n.friend_camera())));
    let mut buf = Vec::with_capacity(4096);
    if !rec.headed {
        rec.headed = true;
        buf.extend_from_slice(MAGIC);
        buf.push(VERSION);
        buf.extend_from_slice(&(rec.level.len() as u16).to_le_bytes());
        buf.extend_from_slice(rec.level.as_bytes());
        buf.extend_from_slice(&(ours.bones.len() as u16).to_le_bytes());
    }
    put_f32s(&mut buf, &[rec.t]);
    put_actor(&mut buf, Some(&ours));
    put_actor(&mut buf, theirs.as_ref().filter(|a| a.bones.len() == ours.bones.len()));
    let flush = rec.since_flush > 1.0;
    if let Some(out) = rec.out.as_mut() {
        let _ = out.write_all(&buf);
        if flush {
            let _ = out.flush();
        }
    }
    if flush {
        rec.since_flush = 0.0;
    }
}

/// Read a recording.
pub fn load(path: &str) -> Result<Vec<Frame>, String> {
    let d = std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    if d.len() < 9 || &d[0..4] != MAGIC || d[4] != VERSION {
        return Err(format!("{path} is not an ac1-rs replay (version {VERSION})"));
    }
    let n = u16::from_le_bytes([d[5], d[6]]) as usize;
    let mut at = 7 + n;
    let bones = u16::from_le_bytes([*d.get(at).ok_or("truncated")?, *d.get(at + 1).ok_or("truncated")?]) as usize;
    at += 2;
    let f = |at: &mut usize| -> Option<f32> {
        let v = d.get(*at..*at + 4).map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]));
        *at += 4;
        v
    };
    let xf = |at: &mut usize| -> Option<(Vec3, Quat)> {
        let p = Vec3::new(f(at)?, f(at)?, f(at)?);
        let r = Quat::from_xyzw(f(at)?, f(at)?, f(at)?, f(at)?).normalize();
        Some((p, r))
    };
    let mut frames = vec![];
    // (A recording cut off mid-frame, the game closed: the frames before it.)
    'frames: while at < d.len() {
        let Some(t) = f(&mut at) else { break };
        let mut actors: [Option<Actor>; 2] = [None, None];
        for slot in &mut actors {
            let Some(&kind) = d.get(at) else { break 'frames };
            at += 1;
            if kind == 0 {
                continue;
            }
            let Some(root) = xf(&mut at) else { break 'frames };
            let mut bs = Vec::with_capacity(bones);
            for _ in 0..bones {
                let Some(b) = xf(&mut at) else { break 'frames };
                bs.push(b);
            }
            let cam = if kind == 2 {
                let Some(c) = xf(&mut at) else { break 'frames };
                Some(c)
            } else {
                None
            };
            *slot = Some(Actor { root, bones: bs, cam });
        }
        frames.push(Frame { t, actors });
    }
    if frames.is_empty() {
        return Err(format!("{path} holds no frames"));
    }
    Ok(frames)
}

/// The players at time `t`, between the frames around it.
fn sample(frames: &[Frame], t: f32) -> [Option<Actor>; 2] {
    let i = frames.partition_point(|f| f.t <= t).saturating_sub(1);
    let (a, b) = (&frames[i], frames.get(i + 1).unwrap_or(&frames[i]));
    let k = if b.t > a.t { ((t - a.t) / (b.t - a.t)).clamp(0.0, 1.0) } else { 0.0 };
    let mix = |x: &Option<Actor>, y: &Option<Actor>| -> Option<Actor> {
        match (x, y) {
            (Some(x), Some(y)) if x.bones.len() == y.bones.len() => Some(Actor {
                root: (x.root.0.lerp(y.root.0, k), x.root.1.slerp(y.root.1, k)),
                bones: x.bones.iter().zip(&y.bones).map(|(p, q)| (p.0.lerp(q.0, k), p.1.slerp(q.1, k))).collect(),
                cam: match (x.cam, y.cam) {
                    (Some(c), Some(d)) => Some((c.0.lerp(d.0, k), c.1.slerp(d.1, k))),
                    (c, d) => c.or(d),
                },
            }),
            (x, y) => {
                if k < 0.5 {
                    x.clone()
                } else {
                    y.clone()
                }
            }
        }
    };
    [mix(&a.actors[0], &b.actors[0]), mix(&a.actors[1], &b.actors[1])]
}

/// The replay's keys and clock.
pub fn controls(time: Res<Time>, keys: Res<ButtonInput<KeyCode>>, mut replay: ResMut<Replay>) {
    let end = replay.end();
    if replay.render.is_some() {
        // (After the level has loaded in.)
        if time.elapsed_secs() > RENDER_WARMUP {
            replay.t += 1.0 / RENDER_FPS;
        }
        return;
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if keys.just_pressed(KeyCode::Enter) {
        replay.playing = !replay.playing;
    }
    let jump = if shift { 1.0 } else { 5.0 };
    if keys.just_pressed(KeyCode::ArrowLeft) {
        replay.t -= jump;
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        replay.t += jump;
    }
    if !replay.playing {
        if keys.just_pressed(KeyCode::Comma) {
            replay.t -= 1.0 / RENDER_FPS;
        }
        if keys.just_pressed(KeyCode::Period) {
            replay.t += 1.0 / RENDER_FPS;
        }
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        replay.speed = (replay.speed * 2.0).min(8.0);
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        replay.speed = (replay.speed * 0.5).max(1.0 / 16.0);
    }
    if keys.just_pressed(KeyCode::Home) {
        replay.t = 0.0;
    }
    for (k, v) in [(KeyCode::Digit1, View::Ours), (KeyCode::Digit2, View::Friend), (KeyCode::Digit3, View::Free), (KeyCode::Digit4, View::Follow)] {
        if keys.just_pressed(k) {
            replay.view = v;
            info!("replay: camera {v:?}");
        }
    }
    if replay.playing {
        replay.t += time.delta_secs() * replay.speed;
    }
    replay.t = replay.t.clamp(0.0, end);
    if replay.t >= end {
        replay.playing = false;
    }
}

/// Pose both figures from the recording (after `character::animate`, which poses them as statues).
pub fn apply(replay: Res<Replay>, mut figures: Query<Figure, Puppets>, mut joints: Query<&mut Transform, Without<Character>>) {
    let now = sample(&replay.frames, replay.t.min(replay.end()));
    for (mut tf, mut ch, mut vis, ours) in &mut figures {
        let Some(a) = &now[if ours { 0 } else { 1 }] else {
            *vis = Visibility::Hidden;
            continue;
        };
        *vis = Visibility::Inherited;
        tf.translation = a.root.0;
        tf.rotation = a.root.1;
        let ch = &mut *ch;
        if ch.pose.local.len() != a.bones.len() {
            continue;
        }
        for (x, (p, r)) in ch.pose.local.iter_mut().zip(&a.bones) {
            x.pos = *p;
            x.rot = *r;
        }
        let pose = ch.pose.clone();
        ch.write_joints(&pose, &mut joints);
    }
}

/// The recorded cameras (views 1 and 2), after the game's own camera.
pub fn camera(replay: Res<Replay>, mut cams: Query<&mut Transform, (With<Camera3d>, Without<Character>)>) {
    let i = match replay.view {
        View::Ours => 0,
        View::Friend => 1,
        _ => return,
    };
    let now = sample(&replay.frames, replay.t.min(replay.end()));
    if let (Some((p, r)), Ok(mut tf)) = (now[i].as_ref().and_then(|a| a.cam), cams.single_mut()) {
        tf.translation = p;
        tf.rotation = r;
    }
}

/// Rendering a replay to frames (`AC1_REPLAY_FRAMES`): each frame saved, then exit at the end.
pub fn render(time: Res<Time>, mut commands: Commands, mut replay: ResMut<Replay>, mut exit: MessageWriter<AppExit>) {
    if time.elapsed_secs() <= RENDER_WARMUP {
        return;
    }
    let t = replay.t;
    let Some((dir, n, end)) = replay.render.as_mut() else { return };
    if t > *end + 1.0 / RENDER_FPS {
        exit.write(AppExit::Success);
        return;
    }
    let path = format!("{dir}/frame_{:05}.png", *n);
    *n += 1;
    commands.spawn(bevy::render::view::screenshot::Screenshot::primary_window()).observe(bevy::render::view::screenshot::save_to_disk(path));
}
