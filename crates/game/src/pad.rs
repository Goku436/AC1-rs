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
    let l = pad.left_stick().clamp_length_max(1.0);
    stick.0 = if l.length() > DEAD { l.normalize() * ((l.length() - DEAD) / (1.0 - DEAD)) } else { Vec2::ZERO };
    let r = pad.right_stick();
    (r.length() > DEAD).then(|| r * TURN * time.delta_secs())
}

/// Log each controller as it connects (its name and USB ids), so a pad that isn't picked up can be told apart from one
/// that is picked up but mapped wrong.
pub fn log_connected(pads: Query<(&Gamepad, Option<&Name>), Added<Gamepad>>) {
    for (pad, name) in &pads {
        info!("controller connected: {} (vendor {:04x?}, product {:04x?})", name.map_or("?", |n| n.as_str()), pad.vendor_id(), pad.product_id());
    }
}
