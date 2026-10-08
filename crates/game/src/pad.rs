//! Controller support, AC1's console layout, by button position (Xbox / PlayStation names): the left stick moves
//! (analog), the right stick turns the camera; A / Cross the legs, B / Circle the empty hand, Y / Triangle the head
//! (Eagle Vision, synchronizing); the right bumper or trigger (R1 / R2) high profile; Back / Share writes the flight
//! recorder; Start / Options toggles the free camera; the right stick's click (R3) shows the holds (G). (The weapon
//! hand, lock-on and weapons go with combat, left out for now.) Buttons are passed on as the keys and mouse buttons
//! they stand for, so everything reading those works with a pad too. A DualShock 4 is read through Windows' gamepad
//! API, no driver or remapper needed.

use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::prelude::*;

/// The left stick (x right, y forward), when pushed past its dead zone.
#[derive(Resource, Default)]
pub struct PadStick(pub Vec2);

const DEAD: f32 = 0.2;
/// Camera turn rate at full right stick (rad/s).
const TURN: f32 = 2.6;

pub enum Target {
    Key(KeyCode),
    Mouse(MouseButton),
}

const MAP: [(GamepadButton, Target); 9] = [
    (GamepadButton::South, Target::Key(KeyCode::Space)),
    (GamepadButton::East, Target::Key(KeyCode::ShiftLeft)),
    (GamepadButton::North, Target::Key(KeyCode::KeyE)),
    (GamepadButton::RightTrigger2, Target::Mouse(MouseButton::Right)),
    (GamepadButton::RightTrigger, Target::Mouse(MouseButton::Right)),
    (GamepadButton::Select, Target::Key(KeyCode::F9)),
    (GamepadButton::Start, Target::Key(KeyCode::KeyP)),
    (GamepadButton::RightThumb, Target::Key(KeyCode::KeyG)),
    (GamepadButton::LeftThumb, Target::Key(KeyCode::KeyC)),
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
    let l = pad.left_stick();
    stick.0 = if l.length() > DEAD { l.clamp_length_max(1.0) } else { Vec2::ZERO };
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
