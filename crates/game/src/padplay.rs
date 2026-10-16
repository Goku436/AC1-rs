//! Pad files played into the player (`AC1_PADPLAY=<file>`): the real AC1's pad, frame by frame, as ac1-hook's `padrec`
//! writes it and its `padplay` plays it into the running game (`ac1lab pad v1`: an optional `start <x> <y> <z> <yaw>`
//! line in game space, Z up, yaw in degrees from +X toward +Y; then `<dt> <buttons hex> <lx> <ly> <rx> <ry>` a frame,
//! `#` comments). The same file played into both games at once is the live comparison (`tools/live-compare.sh`).
//!
//! The player is put at the start (fresh, as a control run starts him; the camera behind him, as the real game's
//! follows), then each frame of the file is played by game time: its buttons as the keys a controller's stand for
//! (bit 0 the legs: Space; bit 3 the empty hand: Left Shift; bit 12 high profile: the right mouse button), the left
//! stick through AC1's dead zone (`pad::shaped`), the right stick turning the camera (`pad::camera_turn`).
//!
//! `AC1_PADPLAY_GO=<file>` waits for that file to appear (and deletes it) before starting: the comparison script makes
//! it as it sends the real game its `padplay`, so both start within a frame or two; it may hold the real game's camera
//! yaw (ours' convention, radians), which ours' camera then starts at. `AC1_PADPLAY_RECORD=<file>`
//! writes the flight recorder's lines for every frame played (as a control run's `AC1_RECORD`), the ac1-rs side of
//! `livelog compare`. `AC1_PADPLAY_QUIT=1` closes the game when the file is played out.

use bevy::prelude::*;

use crate::character::{Character, Controller, Player};
use crate::{OrbitCam, pad, recorder};

/// One frame of a pad file: its game seconds, buttons (bit k = button k), left and right sticks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub dt: f32,
    pub buttons: u32,
    pub left: Vec2,
    pub right: Vec2,
}

/// A pad file: where it starts (game space, yaw in degrees) and its frames.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PadFile {
    pub start: Option<(Vec3, f32)>,
    pub frames: Vec<Frame>,
}

/// Parse an `ac1lab pad v1` file. Errors name the line.
pub fn parse(text: &str) -> Result<PadFile, String> {
    let mut lines = text.lines().enumerate().map(|(i, l)| (i + 1, l.split('#').next().unwrap_or("").trim())).filter(|(_, l)| !l.is_empty());
    match lines.next() {
        Some((_, "ac1lab pad v1")) => {}
        Some((n, l)) => return Err(format!("line {n}: want `ac1lab pad v1`, found `{l}`")),
        None => return Err("empty pad file".into()),
    }
    let mut out = PadFile::default();
    for (n, l) in lines {
        let w: Vec<&str> = l.split_whitespace().collect();
        let num = |i: usize| w.get(i).and_then(|v| v.parse::<f32>().ok()).ok_or_else(|| format!("line {n}: bad number in `{l}`"));
        if w[0] == "start" {
            out.start = Some((Vec3::new(num(1)?, num(2)?, num(3)?), num(4)?));
            continue;
        }
        let buttons =
            w.get(1).and_then(|b| u32::from_str_radix(b.trim_start_matches("0x"), 16).ok()).ok_or_else(|| format!("line {n}: bad buttons in `{l}`"))?;
        out.frames.push(Frame { dt: num(0)?, buttons, left: Vec2::new(num(2)?, num(3)?), right: Vec2::new(num(4)?, num(5)?) });
    }
    Ok(out)
}

/// Game space (Z up, yaw from +X toward +Y) to ours: the place, and the body's turn about Y (it faces -Z unturned).
pub fn from_game(p: Vec3, yaw_deg: f32) -> (Vec3, f32) {
    (Vec3::new(p.x, p.z, -p.y), (yaw_deg - 90.0).to_radians())
}

#[derive(Default, PartialEq)]
enum Stage {
    #[default]
    Off,
    Waiting,
    Playing,
    Done,
}

/// The playback (`AC1_PADPLAY`).
#[derive(Resource, Default)]
pub struct PadPlay {
    file: PadFile,
    stage: Stage,
    go: Option<std::path::PathBuf>,
    record: Option<String>,
    quit: bool,
    /// Game seconds since the start, the frame playing and the game time it began at, the buttons held.
    t: f32,
    at: usize,
    began: f32,
    held: u32,
}

impl PadPlay {
    /// Playing a file now (the player's input comes from it, scripted runs too).
    pub fn playing(&self) -> bool {
        self.stage == Stage::Playing
    }

    /// From the environment (`AC1_PADPLAY`, `AC1_PADPLAY_GO`, `AC1_PADPLAY_RECORD`, `AC1_PADPLAY_QUIT`).
    pub fn from_env() -> Self {
        let Ok(path) = std::env::var("AC1_PADPLAY") else { return Self::default() };
        let file = match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| parse(&t)) {
            Ok(f) => f,
            Err(e) => {
                error!("padplay: {path}: {e}");
                return Self::default();
            }
        };
        info!("padplay: {path}: {} frames, {:.2} s", file.frames.len(), file.frames.iter().map(|f| f.dt).sum::<f32>());
        Self {
            file,
            stage: Stage::Waiting,
            go: std::env::var("AC1_PADPLAY_GO").ok().map(Into::into),
            record: std::env::var("AC1_PADPLAY_RECORD").ok(),
            quit: std::env::var("AC1_PADPLAY_QUIT").is_ok(),
            ..default()
        }
    }
}

const BUTTONS: [(u32, Button); 3] = [(0, Button::Key(KeyCode::Space)), (3, Button::Key(KeyCode::ShiftLeft)), (12, Button::Mouse(MouseButton::Right))];

#[derive(Clone, Copy)]
enum Button {
    Key(KeyCode),
    Mouse(MouseButton),
}

/// Play the pad file into the player (before `player_input`, after the controller's `pad_input`).
#[allow(clippy::too_many_arguments)]
pub fn play(
    time: Res<Time>,
    mut play: ResMut<PadPlay>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut stick: ResMut<pad::PadStick>,
    mut cam: ResMut<OrbitCam>,
    mut recorder: ResMut<recorder::Recorder>,
    mut q: Query<(&mut Transform, &mut Controller, &mut Character), With<Player>>,
    mut exit: MessageWriter<AppExit>,
) {
    let play = &mut *play;
    match play.stage {
        Stage::Off | Stage::Done => return,
        Stage::Waiting => {
            // (The go file may hold the real game's camera yaw (ours' convention, rad): the stick is the camera's,
            // so both games' cameras start the same way round the player.)
            let mut cam_yaw = None;
            if let Some(go) = &play.go {
                if !go.exists() {
                    return;
                }
                cam_yaw = std::fs::read_to_string(go).ok().and_then(|t| t.trim().parse::<f32>().ok());
                let _ = std::fs::remove_file(go);
            }
            let Ok((mut tf, mut ctl, mut ch)) = q.single_mut() else { return };
            if let Some((p, yaw)) = play.file.start {
                let (at, turn) = from_game(p, yaw);
                tf.translation = at;
                tf.rotation = Quat::from_rotation_y(turn);
                cam.yaw = turn;
            }
            if let Some(y) = cam_yaw {
                cam.yaw = y;
            }
            // (Fresh, as a control run starts him.)
            ch.wall = None;
            ch.velocity = Vec3::ZERO;
            ch.fall_v = 0.0;
            ch.ragdoll = None;
            ch.edge_lock = None;
            *ctl = Controller::default();
            if let Some(p) = &play.record {
                recorder.start_run(p.clone());
            }
            (play.t, play.at, play.began, play.held) = (0.0, 0, 0.0, 0);
            play.stage = Stage::Playing;
            info!("padplay: started at {:.2}", tf.translation);
        }
        Stage::Playing => play.t += time.delta_secs(),
    }
    // The frame playing now, by game time.
    while play.at < play.file.frames.len() && play.t >= play.began + play.file.frames[play.at].dt {
        play.began += play.file.frames[play.at].dt;
        play.at += 1;
    }
    let frame = play.file.frames.get(play.at).copied();
    let now = frame.map_or(0, |f| f.buttons);
    for (bit, b) in BUTTONS {
        let (was, is) = (play.held >> bit & 1 != 0, now >> bit & 1 != 0);
        match (b, was, is) {
            (Button::Key(k), false, true) => keys.press(k),
            (Button::Key(k), true, false) => keys.release(k),
            (Button::Mouse(m), false, true) => mouse.press(m),
            (Button::Mouse(m), true, false) => mouse.release(m),
            _ => {}
        }
    }
    play.held = now;
    let Some(f) = frame else {
        stick.0 = Vec2::ZERO;
        recorder.end_run();
        play.stage = Stage::Done;
        info!("padplay: done");
        if play.quit {
            exit.write(AppExit::Success);
        }
        return;
    };
    stick.0 = pad::shaped(f.left);
    if let Some(turn) = pad::camera_turn(f.right, time.delta_secs()) {
        cam.yaw -= turn.x;
        cam.pitch = (cam.pitch + turn.y).clamp(-1.5, 0.6);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_pad_file() {
        let f = parse("ac1lab pad v1\n# a comment\nstart 46.389 6.126 4.139 -92.6\n0.0167 1001 0 1 0 0\n0.0167 0 -0.5 0.25 0.1 0\n").unwrap();
        assert_eq!(f.start, Some((Vec3::new(46.389, 6.126, 4.139), -92.6)));
        assert_eq!(f.frames.len(), 2);
        assert_eq!(f.frames[0].buttons, 0x1001);
        assert_eq!(f.frames[1].left, Vec2::new(-0.5, 0.25));
        assert!(parse("nope\n").is_err());
        assert!(parse("ac1lab pad v1\n0.0167 zz 0 0 0 0\n").is_err());
    }

    #[test]
    fn game_space_to_ours() {
        // (Game +X, yaw 0, is ours +X: the body unturned faces -Z, so a quarter turn clockwise.)
        let (p, turn) = from_game(Vec3::new(1.0, 2.0, 3.0), 0.0);
        assert_eq!(p, Vec3::new(1.0, 3.0, -2.0));
        let fwd = Quat::from_rotation_y(turn) * Vec3::NEG_Z;
        assert!((fwd - Vec3::X).length() < 1e-5);
        let (_, turn) = from_game(Vec3::ZERO, 90.0);
        assert!((Quat::from_rotation_y(turn) * Vec3::NEG_Z - Vec3::NEG_Z).length() < 1e-5);
    }
}
