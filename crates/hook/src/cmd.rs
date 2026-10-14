//! Commands to the hook, from outside the game: lines in `ac1-hook.cmd` beside the exe, read (and the file deleted)
//! every few frames, from `Present`:
//! - `shot <path>`: save the frame just drawn as PNG;
//! - `key <DIK hex> [frames]`: hold a key that many frames (6 by default), e.g. `key 1c` Enter, `key 01` Escape;
//! - `log <text>`: write a line to the log (to mark places in it);
//! - `trace <path>` / `trace off`: the player frame by frame into a file (`trace`); `where`: one line to the log;
//! - `live on [MB]` / `live off` / `live`: the live log, the player frame by frame in the flight recorder's format plus
//!   an event log, beside the exe (`live`; also on while the file `ac1-hook.live` is there);
//! - `tp <x> <y> <z> [yaw]`: put the player there (game space, Z up; yaw in degrees from +X toward +Y);
//! - `mouse <button> [frames]`: hold a mouse button (0 left, 1 right = high profile); `look <dx> <dy> <frames>`: move
//!   the mouse that much each frame (the camera); `cursor <x> <y>`: the Windows cursor (the menus follow it);
//! - `background [on|off|auto]`: background mode (on by default, `ac1-hook.foreground` beside the exe turns it off):
//!   the game runs, draws and takes our input behind other windows (`input`, `d3d`);
//!
//! The pad, at the level the game reads it (`control`, `pad`): whatever has the focus, the keyboard left out:
//! - `pad <buttons hex> <lx> <ly> [<rx> <ry> [<updates>]]`: hold this pad state (sticks -1..1, +y forward; buttons
//!   bit k = button k: 0 legs, 3 empty hand, 12 high profile) for that many pad updates (a frame each; none or 0:
//!   until `pad off`), in place of the device's; `pad off`: the device's again; `pad`: what the pad holds now;
//! - `press <button> [updates]`: a button down for some updates (6 by default) on top (0..15, `legs`, `hand`, `high`);
//! - `padrec <path>` / `padrec off`: what the pad holds each update into a pad file (`ac1lab pad v1`: a start line with
//!   the player's place, then `dt buttons lx ly rx ry` a line); `padplay <path> [frames] [here]`: play one back (the
//!   player first put at its start unless `here`; by game time, or a record an update with `frames`), stopped by
//!   `pad off`;
//!
//! The clock (`control`): `pause`; `resume`; `step [n]`: n frames (1 by default) of 1/60 s (or the fixed rate), then
//! paused again; `speed <x>` (0..16) / `speed off`: the game's clock times x; `fixed <fps>` / `fixed off`: every frame
//! 1/fps s of game time, whatever the real time (the clock's own fixed-step mode); `clock`: its fields now.
//!
//! Places: `save <name>` / `load <name>`: the player's place and facing as a bookmark (`ac1-hook.bookmarks` beside the
//! exe) and back there; `marks`: the list.
//!
//! Memory (`memcmd`, addresses as `expr` takes them: hex numbers, the names `human`, `entity`, `interp`, `pad`,
//! `clock`, `actions`, `[...]` to read a pointer): `peek <address> [bytes]` (a hex dump); `poke <address> <u8|u16|u32|
//! i32|f32|bytes> <value>...`; `fields <address[:type]>...` (u8 u16 u32 i32 f32 x vec3 m4 class ptrclass);
//! `classes <address> [words]` (each word and the class of what it points at); `components [entity]` (the player's
//! by default). Probes (`probe`): `probe <name> @<address> [hits=<n>] [field...]` (a breakpoint logging those fields,
//! registers among the names, each time the game runs it, n times at most: 20 by default); `probe off [name]`;
//! `probes`.
//!
//! `ac1-hook.status` holds the frame count, rewritten every second or so: the game is alive and drawing.

use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::{control, d3d, input, live, memcmd, probe, sys, trace};

static FRAME: AtomicU32 = AtomicU32::new(0);

/// Once a frame, from `Present`.
pub fn frame(device: *mut c_void) {
    let n = FRAME.fetch_add(1, Ordering::SeqCst);
    input::tick();
    let started = trace::take_started(live::on());
    trace::frame(n, &started);
    live::frame(n, &started);
    live::poll_flag(n);
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
            "live" => live::command(rest),
            "background" => input::background_command(rest),
            "pad" => control::pad_command(rest),
            "press" => control::press_command(rest),
            "padrec" => control::rec_command(rest),
            "padplay" => control::play_command(rest),
            "pause" | "resume" | "step" | "speed" | "fixed" | "clock" => control::clock_command(word, rest),
            "save" | "load" | "marks" => memcmd::marks(word, rest),
            "peek" => memcmd::peek(rest),
            "poke" => memcmd::poke(rest),
            "fields" => memcmd::fields(rest),
            "classes" => memcmd::classes(rest),
            "components" => memcmd::components(rest),
            "probe" => probe::command(rest),
            "probes" => probe::command(""),
            _ => format!("unknown command: {line}"),
        };
        sys::log(&format!("[frame {n}] {said}"));
    }
}
