//! The player, frame by frame: where Altaïr is, which way he faces, his locomotion context and the actions started,
//! written to a file while tracing (`cmd`: `trace <path>` / `trace off`, `where` for one line in the log).
//!
//! Found in the running game (v1.02 DX9, by RTTI and Frida; addresses are the exe's, base 0x400000):
//! - the player's input interpreter (`GoAssassinActionInterpreter`, ground state 0xEE65A0, `thiscall`, 3 arguments)
//!   holds its `Human` at +0x10 and `Entity` at +0x14: hooked to learn which Human is the player;
//! - `Human` +0x48: the locomotion context now (`HumanGround`, `HumanClimb`, ...: its class names the mode); +0x104
//!   the `Entity`; +0x7DC its `ActionComponent`;
//! - `Entity` +0x10..+0x4C: its world matrix, rows X, Y, Z, then the position (game space, Z up; a character faces +X);
//! - `ActionComponent::Play` (0x501190, `thiscall`, 4 arguments, the action id first): every action started, by id
//!   (the action blocks' ids: `forge` example `action_index` names them).
//!
//! A line per frame: `frame  seconds  x  y  z  yaw  context  actions` (yaw in degrees from +X toward +Y; the actions
//! started that frame, hex, comma-separated).

use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::{patch, sys};

const PLAY: usize = 0x0050_1190;
const PLAY_PROLOGUE: [u8; 8] = [0x8b, 0x44, 0x24, 0x10, 0x8b, 0x54, 0x24, 0x0c];
const GROUND_INTERPRETER: usize = 0x00ee_65a0;
const GROUND_PROLOGUE: [u8; 6] = [0x55, 0x8b, 0xec, 0x83, 0xe4, 0xf0];

const HUMAN_CONTEXT: usize = 0x48;
const HUMAN_ENTITY: usize = 0x104;
const HUMAN_ACTIONS: usize = 0x7dc;
const ENTITY_MATRIX: usize = 0x10;

static PLAY_STUB: AtomicUsize = AtomicUsize::new(0);
static GROUND_STUB: AtomicUsize = AtomicUsize::new(0);
static HUMAN: AtomicUsize = AtomicUsize::new(0);

/// Actions the player started since the last frame line.
static STARTED: Mutex<Vec<u32>> = Mutex::new(Vec::new());
/// The trace file while tracing, and when it began.
static OUT: Mutex<Option<(std::io::BufWriter<std::fs::File>, std::time::Instant)>> = Mutex::new(None);

type Play = unsafe extern "thiscall" fn(usize, u32, u32, u32, u32) -> u32;
type Interpret = unsafe extern "thiscall" fn(usize, u32, u32, u32) -> u32;

pub fn install() {
    // SAFETY: the exe's code, checked byte for byte before it is patched.
    unsafe {
        match patch::detour(PLAY, &PLAY_PROLOGUE, play as *const () as usize) {
            Some(stub) => PLAY_STUB.store(stub, Ordering::SeqCst),
            None => sys::log("trace: ActionComponent::Play is not as expected (another exe build?): actions not traced"),
        }
        match patch::detour(GROUND_INTERPRETER, &GROUND_PROLOGUE, interpret as *const () as usize) {
            Some(stub) => GROUND_STUB.store(stub, Ordering::SeqCst),
            None => sys::log("trace: the ground interpreter is not as expected (another exe build?): no player found"),
        }
    }
    sys::log("trace: hooks in");
}

unsafe extern "thiscall" fn play(this: usize, id: u32, b: u32, c: u32, d: u32) -> u32 {
    let human = HUMAN.load(Ordering::Relaxed);
    if human != 0 && sys::peek(human + HUMAN_ACTIONS) == Some(this as u32) && OUT.lock().unwrap().is_some() {
        STARTED.lock().unwrap().push(id);
    }
    // SAFETY: the original, through its stub, with the same arguments.
    unsafe { std::mem::transmute::<usize, Play>(PLAY_STUB.load(Ordering::Relaxed))(this, id, b, c, d) }
}

unsafe extern "thiscall" fn interpret(this: usize, a: u32, b: u32, c: u32) -> u32 {
    if let Some(h) = sys::peek(this + 0x10).filter(|&h| h != 0)
        && HUMAN.swap(h as usize, Ordering::Relaxed) != h as usize
    {
        sys::log(&format!("trace: the player's Human is at {h:#x}"));
    }
    // SAFETY: the original, through its stub, with the same arguments.
    unsafe { std::mem::transmute::<usize, Interpret>(GROUND_STUB.load(Ordering::Relaxed))(this, a, b, c) }
}

/// The player now: position, yaw (degrees), context class. None until the player is known.
fn now() -> Option<([f32; 3], f32, String)> {
    let human = HUMAN.load(Ordering::Relaxed);
    if human == 0 {
        return None;
    }
    let entity = sys::peek(human + HUMAN_ENTITY)? as usize;
    let m = entity + ENTITY_MATRIX;
    if !sys::readable(m, 64) {
        return None;
    }
    // SAFETY: checked readable: 16 floats.
    let f = |k: usize| unsafe { ((m + 4 * k) as *const f32).read_unaligned() };
    let yaw = f(1).atan2(f(0)).to_degrees();
    let context = sys::peek(human + HUMAN_CONTEXT).and_then(|c| sys::class_of(c as usize)).unwrap_or_else(|| "?".into());
    Some(([f(12), f(13), f(14)], yaw, context))
}

/// Once a frame (from `Present`): the frame's line while tracing.
pub fn frame(n: u32) {
    let mut out = OUT.lock().unwrap();
    let Some((file, start)) = out.as_mut() else { return };
    let Some((p, yaw, context)) = now() else { return };
    let ids: Vec<String> = STARTED.lock().unwrap().drain(..).map(|id| format!("{id:08x}")).collect();
    let _ = writeln!(file, "{n}\t{:.4}\t{:.3}\t{:.3}\t{:.3}\t{yaw:.1}\t{context}\t{}", start.elapsed().as_secs_f32(), p[0], p[1], p[2], ids.join(","));
}

/// Start tracing into `path` (a header line first), or stop (`off`).
pub fn command(arg: &str) -> String {
    let mut out = OUT.lock().unwrap();
    if arg == "off" || arg.is_empty() {
        if let Some((mut f, _)) = out.take() {
            let _ = f.flush();
        }
        return "trace off".into();
    }
    match std::fs::File::create(arg) {
        Ok(f) => {
            let mut w = std::io::BufWriter::new(f);
            let _ = writeln!(w, "frame\tseconds\tx\ty\tz\tyaw\tcontext\tactions");
            *out = Some((w, std::time::Instant::now()));
            STARTED.lock().unwrap().clear();
            format!("trace into {arg}")
        }
        Err(e) => format!("trace: cannot write {arg}: {e}"),
    }
}

/// One line about the player now, for the log.
pub fn here() -> String {
    match now() {
        Some((p, yaw, context)) => format!("player at ({:.3}, {:.3}, {:.3}) yaw {yaw:.1} in {context}", p[0], p[1], p[2]),
        None => "player not known yet (found when the ground interpreter first runs)".into(),
    }
}
