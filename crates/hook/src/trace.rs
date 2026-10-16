//! The player, frame by frame: where Altaïr is, which way he faces, his locomotion context and the actions started,
//! written to a file while tracing (`cmd`: `trace <path>` / `trace off`, `where` for one line in the log).
//!
//! Found in the running game (v1.02 DX9, by RTTI and Frida; addresses are the exe's, base 0x400000):
//! - the player's input interpreter (`GoAssassinActionInterpreter`, ground state 0xEE65A0, `thiscall`, 3 arguments;
//!   its `Update`, hooked in `control`, runs in every context) holds its `Human` at +0x10 and `Entity` at +0x14:
//!   hooked to learn which Human is the player;
//! - `Human` +0x48: the locomotion context now (`HumanGround`, `HumanClimb`, ...: its class names the mode); +0x104
//!   the `Entity`; +0x7DC its `ActionComponent`;
//! - `Entity` +0x10..+0x4C: its world matrix, rows X, Y, Z, then the position (game space, Z up; the character faces
//!   its Y row, the yaw logged is that row's);
//! - `ActionComponent::Play` (0x501190, `thiscall`, 4 arguments, the action id first): every action started, by id
//!   (the action blocks' ids: `forge` example `action_index` names them).
//!
//! A line per frame: `frame  seconds  x  y  z  yaw  context  actions` (yaw in degrees from +X toward +Y; the actions
//! started that frame, hex, comma-separated).

use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::{patch, sys};

const PLAY: usize = 0x0050_1190;
const PLAY_PROLOGUE: [u8; 8] = [0x8b, 0x44, 0x24, 0x10, 0x8b, 0x54, 0x24, 0x0c];
const GROUND_INTERPRETER: usize = 0x00ee_65a0;
const GROUND_PROLOGUE: [u8; 6] = [0x55, 0x8b, 0xec, 0x83, 0xe4, 0xf0];

const HUMAN_CONTEXT: usize = 0x48;
const HUMAN_ENTITY: usize = 0x104;
/// `HumanGround`'s heading: its angle (f32, radians), then the same as a direction (2 f32); read live 2026-10-16.
const GROUND_HEADING: usize = 0x5FC;
const HUMAN_ACTIONS: usize = 0x7dc;
const ENTITY_MATRIX: usize = 0x10;

static PLAY_STUB: AtomicUsize = AtomicUsize::new(0);
static GROUND_STUB: AtomicUsize = AtomicUsize::new(0);
static HUMAN: AtomicUsize = AtomicUsize::new(0);
/// The player's input interpreter (the ground state's `this`).
static INTERP: AtomicUsize = AtomicUsize::new(0);
/// The player's `ActionComponent` as of the last frame, while tracing or the live log is on (0 otherwise).
static PLAYER_ACTIONS: AtomicUsize = AtomicUsize::new(0);
static TRACING: AtomicBool = AtomicBool::new(false);

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
    let player = PLAYER_ACTIONS.load(Ordering::Relaxed);
    if player != 0 && this == player {
        STARTED.lock().unwrap().push(id);
    }
    // SAFETY: the original, through its stub, with the same arguments.
    unsafe { std::mem::transmute::<usize, Play>(PLAY_STUB.load(Ordering::Relaxed))(this, id, b, c, d) }
}

unsafe extern "thiscall" fn interpret(this: usize, a: u32, b: u32, c: u32) -> u32 {
    found_interp(this);
    // SAFETY: the original, through its stub, with the same arguments.
    unsafe { std::mem::transmute::<usize, Interpret>(GROUND_STUB.load(Ordering::Relaxed))(this, a, b, c) }
}

/// The player's input interpreter is `this` (from its ground state here, or its `Update` in `control`): it and its
/// `Human` (+0x10) are the player's.
pub fn found_interp(this: usize) {
    INTERP.store(this, Ordering::Relaxed);
    if let Some(h) = sys::peek(this + 0x10).filter(|&h| h != 0)
        && HUMAN.swap(h as usize, Ordering::Relaxed) != h as usize
    {
        sys::log(&format!("trace: the player's Human is at {h:#x}"));
    }
}

/// Where the player is (game space) and his yaw (degrees), if known.
pub fn player_at() -> Option<([f32; 3], f32)> {
    now().map(|(p, yaw, _)| (p, yaw))
}

/// The player's `Entity` (0 until known).
pub fn entity() -> usize {
    let human = HUMAN.load(Ordering::Relaxed);
    if human == 0 { 0 } else { sys::peek(human + HUMAN_ENTITY).unwrap_or(0) as usize }
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
    // (The character faces the matrix's Y row: walking toward -Y, its X row pointed -X.)
    let yaw = f(5).atan2(f(4)).to_degrees();
    let context = sys::peek(human + HUMAN_CONTEXT).and_then(|c| sys::class_of(c as usize)).unwrap_or_else(|| "?".into());
    Some(([f(12), f(13), f(14)], yaw, context))
}

/// The player's `Human` (0 until the ground interpreter first runs).
pub fn human() -> usize {
    HUMAN.load(Ordering::Relaxed)
}

/// The player's input interpreter (0 until it first runs on the ground).
pub fn interp() -> usize {
    INTERP.load(Ordering::Relaxed)
}

/// Once a frame (from `Present`): the actions the player started since the last frame, while tracing or the live log
/// is on (the `Play` hook keeps them only then).
pub fn take_started(live: bool) -> Vec<u32> {
    let human = HUMAN.load(Ordering::Relaxed);
    let wanted = human != 0 && (live || TRACING.load(Ordering::Relaxed));
    let actions = if wanted { sys::peek(human + HUMAN_ACTIONS).unwrap_or(0) as usize } else { 0 };
    PLAYER_ACTIONS.store(actions, Ordering::Relaxed);
    let mut started = STARTED.lock().unwrap();
    if started.is_empty() { Vec::new() } else { started.drain(..).collect() }
}

/// Once a frame (from `Present`): the frame's line while tracing.
pub fn frame(n: u32, started: &[u32]) {
    let mut out = OUT.lock().unwrap();
    let Some((file, start)) = out.as_mut() else { return };
    let Some((p, yaw, context)) = now() else { return };
    let ids: Vec<String> = started.iter().map(|id| format!("{id:08x}")).collect();
    let _ = writeln!(file, "{n}\t{:.4}\t{:.3}\t{:.3}\t{:.3}\t{yaw:.1}\t{context}\t{}", start.elapsed().as_secs_f32(), p[0], p[1], p[2], ids.join(","));
}

/// Start tracing into `path` (a header line first), or stop (`off`).
pub fn command(arg: &str) -> String {
    let mut out = OUT.lock().unwrap();
    if arg == "off" || arg.is_empty() {
        if let Some((mut f, _)) = out.take() {
            let _ = f.flush();
        }
        TRACING.store(false, Ordering::SeqCst);
        return "trace off".into();
    }
    match std::fs::File::create(arg) {
        Ok(f) => {
            let mut w = std::io::BufWriter::new(f);
            let _ = writeln!(w, "frame\tseconds\tx\ty\tz\tyaw\tcontext\tactions");
            *out = Some((w, std::time::Instant::now()));
            TRACING.store(true, Ordering::SeqCst);
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

/// `tp x y z [yaw]`: write the player's position (and facing) into his Entity's world matrix (game space).
pub fn teleport(arg: &str) -> String {
    let v: Vec<f32> = arg.split_whitespace().filter_map(|w| w.parse().ok()).collect();
    if v.len() < 3 {
        return format!("tp: want x y z [yaw], got {arg}");
    }
    let human = HUMAN.load(Ordering::Relaxed);
    let Some(entity) = sys::peek(human + HUMAN_ENTITY).filter(|_| human != 0) else { return "tp: player not known yet".into() };
    let m = entity as usize + ENTITY_MATRIX;
    if !sys::readable(m, 64) {
        return "tp: the entity's matrix is not readable".into();
    }
    // SAFETY: checked readable: the entity's 16 floats, rows X, Y, Z, position.
    unsafe {
        let f = m as *mut f32;
        if let Some(&yaw) = v.get(3) {
            let (s, c) = yaw.to_radians().sin_cos();
            // (Facing along the Y row; the X row its right-hand side, Y × Z.)
            for (k, x) in [s, -c, 0.0, 0.0, c, s, 0.0, 0.0].iter().enumerate() {
                *f.add(k) = *x;
            }
        }
        *f.add(12) = v[0];
        *f.add(13) = v[1];
        *f.add(14) = v[2];
    }
    // The ground's own heading too (`HumanGround` +0x5FC, its angle in radians from +X toward +Y, then the same as a
    // direction): moving on from a teleport, the ground turns the body from it, not from the matrix. Left alone, the first
    // step snapped the body back to the facing it had before the teleport (live comparison run3, 2026-10-16).
    let mut heading = "";
    if let (Some(&yaw), Some(ctx)) = (v.get(3), sys::peek(human + HUMAN_CONTEXT))
        && crate::memcmd::class(ctx).as_deref() == Some("HumanGround")
    {
        let a = yaw.to_radians();
        let bytes: Vec<u8> = [a, a.cos(), a.sin()].iter().flat_map(|x| x.to_le_bytes()).collect();
        if sys::write(ctx as usize + GROUND_HEADING, &bytes) {
            heading = ", the ground's heading too";
        }
    }
    format!("tp to ({}, {}, {}){heading}; now {}", v[0], v[1], v[2], here())
}
