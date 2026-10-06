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
