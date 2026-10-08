//! A flight recorder for bugs: the player's last ten seconds, frame by frame (where the root is and where it
//! faces, the speed, the controller's state, the move and clip playing, the fight action, IK targets and the
//! world positions of the hips, head, hands and feet). F9 writes it to `ac1-recording-<ms>.txt` with a
//! screenshot beside it (`AC1_RECORD_AT=secs` from a script); a glitch is then on record to look into.

use crate::character::Character;
use crate::character::Player;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use std::collections::VecDeque;
use std::fmt::Write;

/// Seconds kept.
const SPAN: f32 = 10.0;
const BONES: [&str; 6] = ["Hips", "Head", "LeftHand", "RightHand", "LeftFoot", "RightFoot"];

#[derive(Resource, Default)]
pub struct Recorder {
    frames: VecDeque<(f32, String)>,
    written: bool,
    /// The last frames' time, root, bones and what was playing, for the live pop check (`AC1_POP_CHECK`).
    recent: VecDeque<(f32, Vec3, Vec<Vec3>, String)>,
}

/// A pose pop: a bone, relative to the root, moving at least this far (m) in a frame...
const POP_MIN: f32 = 0.04;
/// ...and more than `POP_FACTOR` times and `POP_JUMP` m/s faster than the frame before (as the `pops` example).
const POP_FACTOR: f32 = 3.0;
const POP_JUMP: f32 = 1.5;

/// The live pop check (`AC1_POP_CHECK`): `POP <bone> <metres> | <playing> <- <playing before>` in the log for each pop,
/// judged as the `pops` example judges a recording.
fn pop_check(rec: &mut Recorder, t: f32, root: Vec3, bones: Vec<Vec3>, state: String) {
    rec.recent.push_back((t, root, bones, state));
    while rec.recent.len() > 4 {
        rec.recent.pop_front();
    }
    if rec.recent.len() < 4 {
        return;
    }
    let f = |k: usize| &rec.recent[k];
    // (The bones are as of the frame before the root: each taken relative to the root a frame earlier.)
    let rel = |k: usize, i: usize| f(k).2.get(i).copied().unwrap_or(Vec3::ZERO) - f(k - 1).1;
    let (dt, dtp) = (f(2).0 - f(1).0, f(1).0 - f(0).0);
    if dt <= 0.0 || dtp <= 0.0 || f(3).2.len() != f(2).2.len() || f(2).2.len() != f(1).2.len() {
        return;
    }
    for (i, name) in BONES.iter().enumerate().take(f(3).2.len()) {
        let d = rel(3, i).distance(rel(2, i));
        let vp = rel(2, i).distance(rel(1, i)) / dtp;
        if d > POP_MIN && d / dt > POP_FACTOR * vp + POP_JUMP {
            warn!("POP {name} {d:.3} m at {t:.2} | {} <- {}", f(3).3, f(2).3);
        }
    }
}

/// Keep this frame; write the recording on F9.
pub fn record(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut rec: ResMut<Recorder>,
    player: Query<(&Transform, &Character), With<Player>>,
    globals: Query<&GlobalTransform>,
) {
    let t = time.elapsed_secs();
    let Ok((tf, ch)) = player.single() else { return };
    let yaw = tf.rotation.to_euler(EulerRot::YXZ).0.to_degrees();
    let mut line =
        format!("{t:8.3} root {:7.2} {:7.2} {:7.2} yaw {yaw:6.1} vel {:5.2}", tf.translation.x, tf.translation.y, tf.translation.z, ch.velocity.length());
    match &ch.wall {
        Some(w) => {
            let _ = write!(line, " | climb {} clip {}", w.state, w.clip_name().unwrap_or("-"));
        }
        None => line.push_str(" | ground"),
    }
    if ch.ragdoll.is_some() {
        line.push_str(" | limp");
    }
    line.push_str(" | bones");
    for b in BONES {
        if let Some(p) = ch.joint_named(b).and_then(|e| globals.get(e).ok()).map(|g| g.translation()) {
            let _ = write!(line, " {b} {:.2},{:.2},{:.2}", p.x, p.y, p.z);
        }
    }
    if !ch.debug_targets.is_empty() {
        line.push_str(" | targets");
        for (p, _) in &ch.debug_targets {
            let _ = write!(line, " {:.2},{:.2},{:.2}", p.x, p.y, p.z);
        }
    }
    if std::env::var("AC1_POP_CHECK").is_ok() {
        let bones: Vec<Vec3> = BONES.iter().filter_map(|b| ch.joint_named(b).and_then(|e| globals.get(e).ok()).map(|g| g.translation())).collect();
        let state = line.split(" | ").nth(1).unwrap_or("").to_string();
        pop_check(&mut rec, t, tf.translation, bones, state);
    }
    rec.frames.push_back((t, line));
    while rec.frames.front().is_some_and(|f| f.0 < t - SPAN) {
        rec.frames.pop_front();
    }
    let scripted = std::env::var("AC1_RECORD_AT").ok().and_then(|s| s.parse::<f32>().ok()).is_some_and(|at| t > at && !rec.written);
    if keys.just_pressed(KeyCode::F9) || scripted {
        rec.written = true;
        let ms = time.elapsed().as_millis();
        let path = format!("ac1-recording-{ms}.txt");
        let mut out =
            String::from("# ac1-rs flight recorder: the player's last 10 s (time root yaw speed | move or fight | bone world positions | IK targets)\n");
        for (_, l) in &rec.frames {
            out.push_str(l);
            out.push('\n');
        }
        match std::fs::write(&path, out) {
            Ok(()) => info!("recording written to {path} ({} frames)", rec.frames.len()),
            Err(e) => warn!("recording {path}: {e}"),
        }
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(format!("ac1-recording-{ms}.png")));
    }
}
