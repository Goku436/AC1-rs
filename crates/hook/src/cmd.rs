//! Commands to the hook, from outside the game: lines in `ac1-hook.cmd` beside the exe, read (and the file deleted)
//! every few frames, from `Present`:
//! - `shot <path>`: save the frame just drawn as PNG;
//! - `key <DIK hex> [frames]`: hold a key that many frames (6 by default), e.g. `key 1c` Enter, `key 01` Escape;
//! - `log <text>`: write a line to the log (to mark places in it);
//! - `trace <path>` / `trace off`: the player frame by frame into a file (`trace`); `where`: one line to the log;
//! - `tp <x> <y> <z> [yaw]`: put the player there (game space, Z up; yaw in degrees from +X toward +Y);
//! - `mouse <button> [frames]`: hold a mouse button (0 left, 1 right = high profile); `look <dx> <dy> <frames>`: move
//!   the mouse that much each frame (the camera); `cursor <x> <y>`: the Windows cursor (the menus follow it).
//!
//! `ac1-hook.status` holds the frame count, rewritten every second or so: the game is alive and drawing.

use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::{d3d, input, sys, trace};

static FRAME: AtomicU32 = AtomicU32::new(0);

/// Once a frame, from `Present`.
pub fn frame(device: *mut c_void) {
    let n = FRAME.fetch_add(1, Ordering::SeqCst);
    input::tick();
    trace::frame(n);
    if !n.is_multiple_of(6) {
        return;
    }
    if n.is_multiple_of(60) {
        let _ = std::fs::write(sys::game_file("ac1-hook.status"), format!("frame {n}\n"));
    }
    let path = sys::game_file("ac1-hook.cmd");
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    let _ = std::fs::remove_file(&path);
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
        let rest = rest.trim();
        let said = match word {
            "shot" => match d3d::screenshot(device, rest) {
                Ok(()) => format!("shot {rest}"),
                Err(e) => format!("shot failed: {e}"),
            },
            "key" => {
                let mut it = rest.split_whitespace();
                match it.next().and_then(|k| u8::from_str_radix(k, 16).ok()) {
                    Some(k) => {
                        let frames = it.next().and_then(|f| f.parse().ok()).unwrap_or(6);
                        input::press(k, frames);
                        format!("key {k:02x} for {frames} frames")
                    }
                    None => format!("bad key: {rest}"),
                }
            }
            "log" => rest.to_string(),
            "trace" => trace::command(rest),
            "tp" => trace::teleport(rest),
            "mouse" => {
                // mouse <button 0|1|2> <frames>
                let v: Vec<u32> = rest.split_whitespace().filter_map(|w| w.parse().ok()).collect();
                input::button(*v.first().unwrap_or(&1) as u8, *v.get(1).unwrap_or(&6));
                format!("mouse button {rest}")
            }
            "cursor" => {
                // cursor <x> <y>: the Windows cursor, which the menus follow
                let v: Vec<i32> = rest.split_whitespace().filter_map(|w| w.parse().ok()).collect();
                // SAFETY: plain call.
                let ok = unsafe { sys::SetCursorPos(*v.first().unwrap_or(&0), *v.get(1).unwrap_or(&0)) } != 0;
                format!("cursor to {rest}: {}", if ok { "done" } else { "failed" })
            }
            "look" => {
                // look <dx> <dy> <frames>: mouse motion each frame
                let v: Vec<i32> = rest.split_whitespace().filter_map(|w| w.parse().ok()).collect();
                input::motion(*v.first().unwrap_or(&0), *v.get(1).unwrap_or(&0), *v.get(2).unwrap_or(&1) as u32);
                format!("look {rest}")
            }
            "where" => trace::here(),
            _ => format!("unknown command: {line}"),
        };
        sys::log(&format!("[frame {n}] {said}"));
    }
}
