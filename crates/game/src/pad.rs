//! Controller support, AC1's console layout, by button position (Xbox / PlayStation names): the left stick moves
//! (analog), the right stick turns the camera; A / Cross the legs, B / Circle the empty hand, Y / Triangle the head
//! (Eagle Vision, synchronizing); the right bumper or trigger (R1 / R2) high profile; Back / Share writes the flight
//! recorder; Start / Options toggles the free camera; the right stick's click (R3) centres the camera, the left's (L3)
//! shows the holds (G). Read only with `AC1_PAD=1`. (The weapon
//! hand, lock-on and weapons go with combat, left out for now.) Buttons are passed on as the keys and mouse buttons
//! they stand for, so everything reading those works with a pad too. A DualShock 4 is read through Windows' gamepad
//! API, no driver or remapper needed.

use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::prelude::*;

/// The left stick (x right, y forward), when pushed past its dead zone.
#[derive(Resource, Default)]
pub struct PadStick(pub Vec2);

/// AC1's radial dead zone (Pad +288); past it the stick's length maps onto 0..1 (0xEE65A0, audit INPUT-7).
const DEAD: f32 = 0.35;
/// The right trigger counts as pressed past this (AC1's 30/255, PadXenon 0x98CE60, audit INPUT-8; Bevy's default
/// press is at 0.75).
const TRIGGER: f32 = 30.0 / 255.0;
static TRIGGER_DOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Camera turn rate at full right stick (rad/s).
const TURN: f32 = 2.6;

/// AC1's stick tracker (`sub_55E7E0`): a stick within 5 degrees of straight ahead reads straight ahead (tracker +0xA8,
/// cos 5 degrees), and the angle off ahead then goes through the left stick's response curve (tracker +0x600, x the
/// angle over pi, y radians, piecewise linear): 10 degrees steers 5, 20 steers 10, 30 steers 20, from 40 on as pushed.
/// The curved direction is the ground's steering (+0x1270). (Read by Banned445, 576860d..c78db3a; ours steered by the
/// raw stick, a pad's small angles off ahead turning him twice as far.)
const SNAP_COS: f32 = 0.996_194_7;
const CURVE: [(f32, f32); 7] = [
    (0.0, 0.0),
    (0.027_777_78, 0.017_453_292),
    (0.055_555_56, 0.087_266_46),
    (0.111_111_11, 0.174_532_93),
    (0.166_666_67, 0.349_065_86),
    (0.222_222_22, 0.698_131_7),
    (1.0, std::f32::consts::PI),
];

/// The stick (x right, y forward) as AC1's tracker passes it on: snapped, then curved, its length kept.
pub fn tracked(stick: Vec2) -> Vec2 {
    let len = stick.length();
    if len < 1e-6 {
        return Vec2::ZERO;
    }
    let n = stick / len;
    if n.y > SNAP_COS {
        return Vec2::Y * len;
    }
    let a = n.x.atan2(n.y);
    let x = a.abs() / std::f32::consts::PI;
    let c =
        CURVE.windows(2).find(|w| x >= w[0].0 && x <= w[1].0).map_or(std::f32::consts::PI, |w| w[0].1 + (x - w[0].0) * (w[1].1 - w[0].1) / (w[1].0 - w[0].0))
            * a.signum();
    Vec2::new(c.sin(), c.cos()) * len
}

pub enum Target {
    Key(KeyCode),
    Mouse(MouseButton),
}

// (The right trigger is read by its value, below. R3 centres the camera, AC1's `StickRight` binding; ours had the
// sticks' clicks swapped, audit INPUT-9.)
const MAP: [(GamepadButton, Target); 8] = [
    (GamepadButton::South, Target::Key(KeyCode::Space)),
    (GamepadButton::East, Target::Key(KeyCode::ShiftLeft)),
    (GamepadButton::North, Target::Key(KeyCode::KeyE)),
    (GamepadButton::RightTrigger, Target::Mouse(MouseButton::Right)),
    (GamepadButton::Select, Target::Key(KeyCode::F9)),
    (GamepadButton::Start, Target::Key(KeyCode::KeyP)),
    (GamepadButton::RightThumb, Target::Key(KeyCode::KeyC)),
    (GamepadButton::LeftThumb, Target::Key(KeyCode::KeyG)),
];

/// Pass the first controller's buttons on and turn the camera (`yaw_pitch`: the orbit camera's).
pub fn read(
    time: Res<Time>,
    pads: Query<&Gamepad>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut stick: ResMut<PadStick>,
) -> Option<Vec2> {
    let pad = pads.iter().next()?;
    for (b, t) in &MAP {
        let (down, up) = (pad.just_pressed(*b), pad.just_released(*b));
        match t {
            Target::Key(k) if down => keys.press(*k),
            Target::Key(k) if up => keys.release(*k),
            Target::Mouse(m) if down => mouse.press(*m),
            Target::Mouse(m) if up => mouse.release(*m),
            _ => {}
        }
    }
    // The right trigger: high profile past AC1's 30/255.
    let down = pad.get(GamepadButton::RightTrigger2).unwrap_or(0.0) > TRIGGER;
    if down != TRIGGER_DOWN.swap(down, std::sync::atomic::Ordering::Relaxed) {
        if down {
            mouse.press(MouseButton::Right);
        } else {
            mouse.release(MouseButton::Right);
        }
    }
    stick.0 = shaped(pad.left_stick());
    camera_turn(pad.right_stick(), time.delta_secs())
}

/// The left stick past AC1's dead zone, its length mapped onto 0..1.
pub fn shaped(l: Vec2) -> Vec2 {
    let l = l.clamp_length_max(1.0);
    if l.length() > DEAD { l.normalize() * ((l.length() - DEAD) / (1.0 - DEAD)) } else { Vec2::ZERO }
}

/// The camera's turn this frame (yaw, pitch; rad) from the right stick, past the dead zone.
pub fn camera_turn(r: Vec2, dt: f32) -> Option<Vec2> {
    (r.length() > DEAD).then(|| r * TURN * dt)
}

/// Log each controller as it connects (its name and USB ids), so a pad that isn't picked up can be told apart from one
/// that is picked up but mapped wrong.
pub fn log_connected(pads: Query<(&Gamepad, Option<&Name>), Added<Gamepad>>) {
    for (pad, name) in &pads {
        info!("controller connected: {} (vendor {:04x?}, product {:04x?})", name.map_or("?", |n| n.as_str()), pad.vendor_id(), pad.product_id());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tracker_snaps_ahead_and_curves_small_angles() {
        let at = |deg: f32| Vec2::new(deg.to_radians().sin(), deg.to_radians().cos());
        let angle = |v: Vec2| v.x.atan2(v.y).to_degrees();
        assert_eq!(tracked(at(3.0)), Vec2::Y);
        assert!((angle(tracked(at(10.0))) - 5.0).abs() < 0.01);
        assert!((angle(tracked(at(-30.0))) + 20.0).abs() < 0.01);
        assert!((angle(tracked(at(90.0))) - 90.0).abs() < 0.01);
        assert!((tracked(at(60.0) * 0.5).length() - 0.5).abs() < 1e-5);
        assert_eq!(tracked(Vec2::ZERO), Vec2::ZERO);
    }
}
