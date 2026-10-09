//! Commands to the hook, from outside the game: lines in `ac1-hook.cmd` beside the exe, read (and the file deleted)
//! every few frames, from `Present`:
//! - `shot <path>`: save the frame just drawn as PNG;
//! - `key <DIK hex> [frames]`: hold a key that many frames (6 by default), e.g. `key 1c` Enter, `key 01` Escape;
//! - `log <text>`: write a line to the log (to mark places in it);
//! - `trace <path>` / `trace off`: the player frame by frame into a file (`trace`); `where`: one line to the log.
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
            "where" => trace::here(),
            _ => format!("unknown command: {line}"),
        };
        sys::log(&format!("[frame {n}] {said}"));
    }
}
