//! Ghost mode: AC1's development flight, the Human Debug context (unreachable in the retail game: its input
//! interpreter only exists when loaded data asks for it, and none does). Behaviour as Banned445's MIT port
//! (AC1-Movement-Rewritten, `debug_fly.rs`) reads the executable; the code here is ours.
//!
//! - Enter (0xE46190): the capsule loses its collision, stick-to-ground off, action request 7 = action 0x0006F88B
//!   "Ghost mode" in HumanGround: `Ghost_mode01` (0.93 s), then `Ghost_mode02` (2.27 s) looping (the action's two
//!   items, a 0.30 s blend between them; 0.40 s into the action).
//! - Fly (0xECDC50, 0xE46DF0): the stick on the ground plane, relative to the camera, and up / down, all × 5 m/s;
//!   × 3 in high profile, × 10 with the faster key. The body turns toward the horizontal motion when its square
//!   exceeds 0.1, unless strafe is held. Nothing collides: walls, floors and holds are flown through.
//! - Leave (0xE467B0): action request 33 `xx_h_jump_falling01` and a fall from rest; with the empty hand held as it
//!   is left, the fall keeps the flight's velocity (thrown out of it).
//!
//! Keys (ours): F7 in / out, WASD the stick, Space up, Left Ctrl down, the right button (high profile) × 3, Left Alt
//! × 10, Q held strafes (the body keeps its facing), Shift held with F7 keeps the speed. Scripts: `AC1_FLY=from-to`,
//! `AC1_FLY_UP` / `AC1_FLY_DOWN=from-to` (see `main.rs`).

use crate::character::{Character, Controller, Player};
use bevy::prelude::*;

/// The Debug context's speed: the input × 5 m/s (0xECDC50).
const FLY_SPEED: f32 = 5.0;
/// × 3 in high profile, × 10 with the faster key (0xECDC50).
const FLY_FAST: f32 = 3.0;
const FLY_FASTER: f32 = 10.0;
/// The body turns toward the motion when its horizontal square exceeds this (0xE46DF0).
const TURN_MIN_SQ: f32 = 0.1;
/// How fast the body turns toward the motion (1/s, ours: the port sets the heading outright, which snapped the body
/// round in a frame).
const TURN_RATE: f32 = 12.0;
/// Action 0x0006F88B's items (HumanGround, request 7), and the leave's request 33.
const GHOST_IN: &str = "Ghost_mode01.man";
const GHOST_LOOP: &str = "Ghost_mode02.man";
const GHOST_OUT: &str = "xx_h_jump_falling01.man";
/// The action's blends: into it, and from its first item into the loop (s).
const BLEND_IN: f32 = 0.4;
const BLEND_LOOP: f32 = 0.3;

/// The flight and this frame's flight input (from the keys, or a script).
#[derive(Resource, Default)]
pub struct Fly {
    /// Flying now.
    pub active: bool,
    /// In or out, asked for this frame.
    pub toggle: bool,
    /// Up (1) or down (-1).
    pub lift: f32,
    /// The faster key (× 10).
    pub faster: bool,
    /// Strafe: the body keeps its facing.
    pub strafe: bool,
    /// The flight's velocity (m/s), carried out by a leave with the empty hand held.
    pub velocity: Vec3,
    /// Time in the flight (s): which Ghost mode clip plays.
    time: f32,
    /// Surfaces flown through (logged once each time the body goes through one).
    passed: u32,
}

/// The flight: in, out, and the body moved through the world. Runs before `character::locomotion`, which leaves a
/// flying character alone.
pub fn fly(
    time: Res<Time>,
    level: Res<crate::level::Level>,
    mut fly: ResMut<Fly>,
    mut lib: Option<ResMut<crate::animation::AnimLib>>,
    mut q: Query<(&mut Transform, &mut Character, &mut Controller), With<Player>>,
) {
    // (This frame's input, set again every frame by the keys or the script.)
    let toggle = std::mem::take(&mut fly.toggle);
    let lift = std::mem::take(&mut fly.lift).clamp(-1.0, 1.0);
    let faster = std::mem::take(&mut fly.faster);
    let strafe = std::mem::take(&mut fly.strafe);
    let Ok((mut tf, mut ch, mut ctl)) = q.single_mut() else { return };
    let ch = &mut *ch;
    let dt = time.delta_secs().min(0.05);
    if toggle && !fly.active && !ch.limp {
        // In (0xE46190), from whatever was playing: no climb, no fall, faded from the pose shown.
        fly.active = true;
        fly.time = 0.0;
        fly.velocity = Vec3::ZERO;
        ch.flying = true;
        ch.wall = None;
        ch.climb = None;
        ch.fall_v = 0.0;
        ch.edge_lock = None;
        ch.velocity = Vec3::ZERO;
        ch.exit_fade = Some((ch.anim_pose.clone(), BLEND_IN, BLEND_IN));
        if let (Some(a), Some(c)) = (ch.animator.as_mut(), lib.as_mut().and_then(|l| l.get(GHOST_IN))) {
            a.forced = Some((c, 0.0));
        }
        info!("fly: Ghost mode on at [{:.2}, {:.2}, {:.2}]", tf.translation.x, tf.translation.y, tf.translation.z);
        return;
    }
    if !fly.active {
        return;
    }
    if toggle {
        // Out (0xE467B0): AC1's falling clip and a fall, from rest or (the empty hand held) at the flight's velocity.
        fly.active = false;
        ch.flying = false;
        let v = if ctl.hand_held { fly.velocity } else { Vec3::ZERO };
        if let Some(a) = ch.animator.as_mut() {
            a.forced = None;
        }
        ch.velocity = v;
        ch.wall = lib.as_deref_mut().and_then(|l| crate::climb::WallClimb::falling_in(l, GHOST_OUT, &tf, v, Some(ch.anim_pose.clone())));
        if ch.wall.is_none() {
            ch.exit_fade = Some((ch.anim_pose.clone(), BLEND_IN, BLEND_IN));
        }
        // (The keys pressed in flight are not the ground's: Space was up, Shift the throw.)
        *ctl = Controller::default();
        info!("fly: Ghost mode off at [{:.2}, {:.2}, {:.2}], falling at {:.1} m/s", tf.translation.x, tf.translation.y, tf.translation.z, v.length());
        return;
    }
    // The clips: the first item once, then the loop, a blend between them (action 0x0006F88B's items).
    let before = fly.time;
    fly.time += dt;
    if let (Some(a), Some(lib)) = (ch.animator.as_mut(), lib.as_mut()) {
        let first = lib.get(GHOST_IN).map_or(0.0, |c| c.anim.duration);
        if before < first && fly.time >= first {
            ch.exit_fade = Some((ch.anim_pose.clone(), BLEND_LOOP, BLEND_LOOP));
        }
        let (name, t) = if fly.time < first { (GHOST_IN, fly.time) } else { (GHOST_LOOP, fly.time - first) };
        if let Some(c) = lib.get(name) {
            let t = t % c.anim.duration.max(1e-3);
            a.forced = Some((c, t));
        }
    }
    // The move: the stick (horizontal, by the camera) and the lift, × 5 m/s, × 3 high profile, × 10 the faster key.
    let scale = if faster {
        FLY_FASTER
    } else if ctl.high {
        FLY_FAST
    } else {
        1.0
    };
    let wish = ctl.move_dir.with_y(0.0).clamp_length_max(1.0) + Vec3::Y * lift;
    let v = wish * FLY_SPEED * scale;
    let from = tf.translation;
    tf.translation += v * dt;
    fly.velocity = v;
    ch.velocity = v;
    // (Through the world: noted in the log, for the scenarios.)
    let step = tf.translation - from;
    let mid = Vec3::Y * 1.0;
    if step.length() > 1e-4 && level.raycast(from + mid, step.normalize(), step.length()).is_some() {
        fly.passed += 1;
        debug!("fly: through a surface at [{:.2}, {:.2}, {:.2}] ({} so far)", tf.translation.x, tf.translation.y, tf.translation.z, fly.passed);
    }
    // The body turns toward the horizontal motion, unless strafing (0xE46DF0).
    let h = v.with_y(0.0);
    if !strafe && h.length_squared() > TURN_MIN_SQ {
        let want = Quat::from_rotation_y((-h.x).atan2(-h.z));
        tf.rotation = tf.rotation.slerp(want, 1.0 - (-TURN_RATE * dt).exp());
    }
    // (Nothing the ground takes from the keys while flying.)
    ctl.legs_press = false;
    ctl.legs_release = false;
    ctl.toggle_climb = false;
    ctl.hand_climb = false;
    ctl.wall_legs = false;
    ctl.push = false;
    ctl.pickpocket = false;
}
