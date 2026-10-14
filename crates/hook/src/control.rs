//! Driving and timing the game from inside: the pad and the clock.
//!
//! - The pad (`pad`'s layout): `PadProxyPC::Update` (0x909C90, `thiscall`, no arguments: vtable slot 9 of the class at
//!   0x16B13FC) is detoured; after it has read the device, the player's proxy (the input interpreter's +0xD0) is given
//!   ours: a held state (`pad`), a recording played back (`padplay`), buttons pressed for some frames (`press`), and
//!   what it then holds is written to a pad file while recording (`padrec`). The game reads this, whatever window has
//!   the focus and whatever the keyboard does: the same input gives the same moves.
//! - The player's interpreter: `GoAssassinActionInterpreter::Update` (0xEEE700, `thiscall`, no arguments: slot 18 of
//!   its vtable at 0x16F52AC) runs each frame in every context (the ground state `trace` hooks only on the ground):
//!   it tells `trace` which interpreter and `Human` are the player's.
//! - The clock: the engine's frame tick (0x416420, `thiscall` on the clock, no arguments) works out the frame's game
//!   seconds and hands them on (0x93E0F0 writes them at 0x192DDA8); on the clock: +4 its mode (0 a fixed step of
//!   1/(+8) s; 1 steps from a table; 2 the real time capped at +0x2C; 3 the real time averaged over 15 frames), +8 the
//!   fixed step's rate (frames a second), +0xC a speed factor (the step times it, at least 0.1 ms), +0xC0 the game time
//!   (u64, 10 µs units). Detoured: for the tick, ours are put in (`speed`, `fixed`, `pause`, `step`) and the game's own
//!   put back after, so the game's own changes to them are kept. Paused, the game moves at the clock's floor, 0.1 ms
//!   a frame (all but still); `step n` lets n frames of 1/60 s (or the `fixed` rate) through.

use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use crate::livefmt::{FRAME_DT, Mem};
use crate::memcmd::Live;
use crate::pad::{self, Frame, Playback};
use crate::{patch, sys, trace};

const PAD_UPDATE: usize = 0x0090_9C90;
const PAD_PROLOGUE: [u8; 9] = [0x56, 0x8b, 0xf1, 0x8b, 0x86, 0x04, 0x05, 0x00, 0x00];
const INTERP_UPDATE: usize = 0x00EE_E700;
const INTERP_PROLOGUE: [u8; 6] = [0x56, 0x8b, 0xf1, 0x8b, 0x46, 0x0c];
const CLOCK_TICK: usize = 0x0041_6420;
const CLOCK_PROLOGUE: [u8; 5] = [0x83, 0xec, 0x10, 0x53, 0x56];
const INTERP_PAD: usize = 0xD0;

const CLOCK_MODE: usize = 0x4;
const CLOCK_RATE: usize = 0x8;
const CLOCK_SPEED: usize = 0xC;
const CLOCK_TIME: usize = 0xC0;

static PAD_STUB: AtomicUsize = AtomicUsize::new(0);
static INTERP_STUB: AtomicUsize = AtomicUsize::new(0);
static CLOCK_STUB: AtomicUsize = AtomicUsize::new(0);
/// The player's pad proxy and the clock, once seen.
static PAD: AtomicUsize = AtomicUsize::new(0);
static CLOCK: AtomicUsize = AtomicUsize::new(0);
/// Pad updates and clock ticks so far.
static PAD_UPDATES: AtomicU32 = AtomicU32::new(0);
static TICKS: AtomicU32 = AtomicU32::new(0);

/// What we give the pad.
#[derive(Default)]
struct PadState {
    /// Held: the frame, and the updates left (None: until `pad off`).
    hold: Option<(Frame, Option<u32>)>,
    play: Option<Playback>,
    /// Buttons pressed on top (bit, updates left).
    press: Vec<(u8, u32)>,
    rec: Option<(std::io::BufWriter<std::fs::File>, String, u32)>,
}

static PAD_STATE: Mutex<Option<PadState>> = Mutex::new(None);

/// The clock as we want it.
#[derive(Clone, Copy, Default)]
struct ClockState {
    speed: Option<f32>,
    fixed: Option<f32>,
    paused: bool,
    steps: u32,
}

static CLOCK_STATE: Mutex<ClockState> = Mutex::new(ClockState { speed: None, fixed: None, paused: false, steps: 0 });

type Method = unsafe extern "thiscall" fn(usize);

pub fn install() {
    // SAFETY: the exe's code, checked byte for byte before it is patched.
    unsafe {
        for (at, prologue, hook, stub, what) in [
            (PAD_UPDATE, &PAD_PROLOGUE[..], pad_update as *const () as usize, &PAD_STUB, "PadProxyPC::Update"),
            (INTERP_UPDATE, &INTERP_PROLOGUE[..], interp_update as *const () as usize, &INTERP_STUB, "GoAssassinActionInterpreter::Update"),
            (CLOCK_TICK, &CLOCK_PROLOGUE[..], clock_tick as *const () as usize, &CLOCK_STUB, "the clock's tick"),
        ] {
            match patch::detour(at, prologue, hook) {
                Some(s) => stub.store(s, Ordering::SeqCst),
                None => sys::log(&format!("control: {what} ({at:#x}) is not as expected (another exe build?): not hooked")),
            }
        }
    }
    sys::log("control: pad, interpreter and clock hooks in");
}

pub fn pad_proxy() -> usize {
    PAD.load(Ordering::Relaxed)
}

pub fn clock() -> usize {
    CLOCK.load(Ordering::Relaxed)
}

unsafe extern "thiscall" fn interp_update(this: usize) {
    // (The player's has the pad: checked by its class the first time an interpreter is seen; others remembered.)
    static OTHERS: Mutex<Vec<usize>> = Mutex::new(Vec::new());
    if this != trace::interp() && !OTHERS.lock().unwrap().contains(&this) {
        let pad = sys::peek(this + INTERP_PAD).unwrap_or(0);
        let class = if pad != 0 { sys::class_of(pad as usize) } else { None };
        if class.as_deref() == Some("PadProxyPC") {
            trace::found_interp(this);
        } else if class.is_some() {
            let mut others = OTHERS.lock().unwrap();
            if others.len() >= 256 {
                others.clear();
            }
            others.push(this);
        }
    }
    // SAFETY: the original, through its stub.
    unsafe { std::mem::transmute::<usize, Method>(INTERP_STUB.load(Ordering::Relaxed))(this) }
}

unsafe extern "thiscall" fn pad_update(this: usize) {
    // SAFETY: the original, through its stub.
    unsafe { std::mem::transmute::<usize, Method>(PAD_STUB.load(Ordering::Relaxed))(this) };
    // The player's proxy only (any, until the player is known).
    let interp = trace::interp();
    let player = if interp != 0 { sys::peek(interp + INTERP_PAD).unwrap_or(0) as usize } else { 0 };
    if player != 0 && this != player {
        return;
    }
    PAD.store(this, Ordering::Relaxed);
    PAD_UPDATES.fetch_add(1, Ordering::Relaxed);
    let mut guard = PAD_STATE.lock().unwrap();
    let Some(st) = guard.as_mut() else { return };
    if !sys::readable(this, pad::SIZE) {
        return;
    }
    // SAFETY: checked readable; the proxy's first bytes, which its Update just wrote, on the thread that wrote them.
    let image = unsafe { std::slice::from_raw_parts_mut(this as *mut u8, pad::SIZE) };
    let dt = Live.f32(FRAME_DT).filter(|d| d.is_finite() && *d >= 0.0).unwrap_or(0.0);
    let mut ours: Option<Frame> = None;
    if let Some(p) = st.play.as_mut() {
        match p.next(dt) {
            Some(f) => ours = Some(f),
            None => {
                let (_, n) = p.progress();
                st.play = None;
                sys::log(&format!("padplay: done ({n} records)"));
            }
        }
    }
    if ours.is_none()
        && let Some((f, left)) = st.hold.as_mut()
    {
        ours = Some(*f);
        if let Some(l) = left {
            *l = l.saturating_sub(1);
            if *l == 0 {
                st.hold = None;
            }
        }
    }
    if !st.press.is_empty() {
        let mut f = ours.or_else(|| pad::read(image)).unwrap_or_default();
        for &(b, _) in &st.press {
            f.buttons |= 1 << b;
        }
        ours = Some(f);
        st.press.retain_mut(|(_, left)| {
            *left -= 1;
            *left > 0
        });
    }
    if let Some(f) = ours {
        pad::write(image, &f);
    }
    if let Some((w, _, n)) = st.rec.as_mut()
        && let Some(mut f) = pad::read(image)
    {
        f.dt = dt;
        let _ = writeln!(w, "{}", pad::line(&f));
        *n += 1;
    }
}

unsafe extern "thiscall" fn clock_tick(this: usize) {
    CLOCK.store(this, Ordering::Relaxed);
    TICKS.fetch_add(1, Ordering::Relaxed);
    let want = {
        let mut c = CLOCK_STATE.lock().unwrap();
        let w = *c;
        if c.paused && c.steps > 0 {
            c.steps -= 1;
        }
        w
    };
    let call = || {
        // SAFETY: the original, through its stub.
        unsafe { std::mem::transmute::<usize, Method>(CLOCK_STUB.load(Ordering::Relaxed))(this) }
    };
    if want.speed.is_none() && want.fixed.is_none() && !want.paused || !sys::readable(this, CLOCK_TIME + 8) {
        return call();
    }
    // SAFETY: the clock object (checked readable): its mode, rate and speed, ours for the tick, its own put back.
    unsafe {
        let mode = (this + CLOCK_MODE) as *mut u32;
        let rate = (this + CLOCK_RATE) as *mut f32;
        let speed = (this + CLOCK_SPEED) as *mut f32;
        let saved = (*mode, *rate, *speed);
        if want.paused && want.steps == 0 {
            *speed = 0.0;
        } else {
            if let Some(s) = want.speed.filter(|_| !want.paused) {
                *speed = s;
            }
            if want.paused {
                *speed = 1.0;
            }
            if let Some(fps) = want.fixed.or(want.paused.then_some(60.0)) {
                *mode = 0;
                *rate = fps;
            }
        }
        call();
        *mode = saved.0;
        *rate = saved.1;
        *speed = saved.2;
    }
}

/// The clock's fields now, for the log.
fn clock_said() -> String {
    let c = CLOCK.load(Ordering::Relaxed) as u32;
    if c == 0 {
        return "clock not seen yet".into();
    }
    let m = Live;
    let st = *CLOCK_STATE.lock().unwrap();
    let time = m.u32(c + CLOCK_TIME as u32).zip(m.u32(c + CLOCK_TIME as u32 + 4)).map(|(lo, hi)| ((hi as u64) << 32 | lo as u64) as f64 / 1e5);
    format!(
        "clock at {c:#x}: mode {} rate {} speed {} (game's own); frame {} s, game time {} s, ticks {}; ours: speed {:?} fixed {:?} {}",
        m.u32(c + CLOCK_MODE as u32).map_or("?".into(), |v| v.to_string()),
        m.f32(c + CLOCK_RATE as u32).map_or("?".into(), |v| format!("{v:.2}")),
        m.f32(c + CLOCK_SPEED as u32).map_or("?".into(), |v| format!("{v:.3}")),
        m.f32(FRAME_DT).map_or("?".into(), |v| format!("{v:.5}")),
        time.map_or("?".into(), |t| format!("{t:.3}")),
        TICKS.load(Ordering::Relaxed),
        st.speed,
        st.fixed,
        if st.paused { format!("paused ({} steps queued)", st.steps) } else { "running".into() },
    )
}

/// `speed <x>` / `speed off`; `fixed <fps>` / `fixed off`; `pause`; `resume`; `step [n]`; `clock`.
pub fn clock_command(word: &str, rest: &str) -> String {
    let arg = rest.split_whitespace().next();
    let mut c = CLOCK_STATE.lock().unwrap();
    let said = match (word, arg) {
        ("speed", Some("off")) => {
            c.speed = None;
            "speed: the game's own".to_string()
        }
        ("speed", Some(x)) => match x.parse::<f32>() {
            Ok(s) if s.is_finite() && (0.0..=16.0).contains(&s) => {
                c.speed = Some(s);
                format!("speed {s}")
            }
            _ => format!("speed: want 0..16 or off, got {x}"),
        },
        ("fixed", Some("off")) => {
            c.fixed = None;
            "fixed step off: the game's own clock mode".to_string()
        }
        ("fixed", Some(x)) => match x.parse::<f32>() {
            Ok(f) if f.is_finite() && (1.0..=1000.0).contains(&f) => {
                c.fixed = Some(f);
                format!("fixed step: 1/{f} s a frame")
            }
            _ => format!("fixed: want frames a second (1..1000) or off, got {x}"),
        },
        ("pause", _) => {
            c.paused = true;
            c.steps = 0;
            "paused".to_string()
        }
        ("resume", _) => {
            c.paused = false;
            c.steps = 0;
            "resumed".to_string()
        }
        ("step", n) => {
            let n: u32 = n.and_then(|n| n.parse().ok()).unwrap_or(1);
            c.paused = true;
            c.steps += n;
            format!("step {n} frame(s)")
        }
        _ => String::new(),
    };
    drop(c);
    if said.is_empty() { clock_said() } else { format!("{said}; {}", clock_said()) }
}

fn with_pad<R>(f: impl FnOnce(&mut PadState) -> R) -> R {
    let mut g = PAD_STATE.lock().unwrap();
    f(g.get_or_insert_with(PadState::default))
}

/// The pad now, for the log.
fn pad_said() -> String {
    let p = PAD.load(Ordering::Relaxed);
    let now = if p != 0 && sys::readable(p, pad::SIZE) {
        // SAFETY: checked readable.
        let image = unsafe { std::slice::from_raw_parts(p as *const u8, pad::SIZE) };
        pad::read(image).map(|f| format!("btn {:04x} sticks {:.2},{:.2} {:.2},{:.2}", f.buttons, f.sticks[0], f.sticks[1], f.sticks[2], f.sticks[3]))
    } else {
        None
    };
    let ours = with_pad(|s| {
        let mut v = Vec::new();
        if let Some((f, left)) = &s.hold {
            v.push(format!("holding {} ({})", pad::line(f), left.map_or("until pad off".into(), |l| format!("{l} updates left"))));
        }
        if let Some(p) = &s.play {
            let (at, n) = p.progress();
            v.push(format!("playing {at}/{n}"));
        }
        if !s.press.is_empty() {
            v.push(format!("pressing {:?}", s.press));
        }
        if let Some((_, path, n)) = &s.rec {
            v.push(format!("recording {n} frames into {path}"));
        }
        if v.is_empty() { "ours: nothing".to_string() } else { format!("ours: {}", v.join("; ")) }
    });
    format!(
        "pad {}: {}; {} updates; {ours}",
        if p == 0 { "not seen yet".into() } else { format!("{p:#x}") },
        now.unwrap_or_else(|| "?".into()),
        PAD_UPDATES.load(Ordering::Relaxed)
    )
}

/// `pad <buttons hex> <lx> <ly> [<rx> <ry> [<updates>]]` (hold; 0 or no count: until `pad off`), `pad off`, `pad`.
pub fn pad_command(rest: &str) -> String {
    let w: Vec<&str> = rest.split_whitespace().collect();
    match w.first() {
        None => pad_said(),
        Some(&"off") => {
            with_pad(|s| {
                s.hold = None;
                s.play = None;
                s.press.clear();
            });
            "pad: ours off (the device's input again)".into()
        }
        Some(b) => {
            let Ok(buttons) = u16::from_str_radix(b.trim_start_matches("0x"), 16) else { return format!("pad: bad buttons {b}") };
            let v: Vec<f32> = w[1..].iter().filter_map(|x| x.parse().ok()).collect();
            if v.len() + 1 != w.len() || v.len() < 2 {
                return format!("pad: want <buttons hex> <lx> <ly> [<rx> <ry> [<updates>]], got {rest}");
            }
            let f = Frame { dt: 0.0, buttons, sticks: [v[0], v[1], v.get(2).copied().unwrap_or(0.0), v.get(3).copied().unwrap_or(0.0)] };
            let left = v.get(4).map(|&n| n as u32).filter(|&n| n > 0);
            with_pad(|s| s.hold = Some((f, left)));
            format!("pad: holding {} for {}", pad::line(&f), left.map_or("ever (pad off)".into(), |n| format!("{n} updates")))
        }
    }
}

/// `press <button> [updates]`: a button down for some updates (6 by default) on top of the rest.
pub fn press_command(rest: &str) -> String {
    let mut w = rest.split_whitespace();
    let Some(b) = w.next().and_then(pad::button) else { return format!("press: want a button (0..15, legs, hand, high), got {rest}") };
    let n = w.next().and_then(|n| n.parse().ok()).unwrap_or(6u32).max(1);
    with_pad(|s| s.press.push((b, n)));
    format!("press button {b} for {n} updates")
}

/// `padrec <path>` (from the next update, with the player's place as the start line) / `padrec off`.
pub fn rec_command(rest: &str) -> String {
    let path = rest.trim();
    if path.is_empty() || path == "off" {
        return with_pad(|s| match s.rec.take() {
            Some((mut w, p, n)) => {
                let _ = w.flush();
                format!("padrec: {n} frames into {p}")
            }
            None => "padrec: was not recording".into(),
        });
    }
    let file = match std::fs::File::create(path) {
        Ok(f) => f,
        Err(e) => return format!("padrec: cannot write {path}: {e}"),
    };
    let mut w = std::io::BufWriter::new(file);
    let _ = writeln!(w, "{}", pad::HEADER);
    let start = trace::player_at();
    if let Some((p, yaw)) = start {
        let _ = writeln!(w, "{}", pad::start_line(p, yaw));
    }
    let _ = writeln!(w, "# dt buttons lx ly rx ry (a line each pad update; ac1-hook)");
    with_pad(|s| s.rec = Some((w, path.to_string(), 0)));
    format!("padrec into {path}{}", if start.is_some() { "" } else { " (player not known: no start line)" })
}

/// `padplay <path> [frames] [here]`: play a pad file back (by game time, or a record an update with `frames`), first
/// putting the player at its start (unless `here`).
pub fn play_command(rest: &str) -> String {
    let mut w = rest.split_whitespace();
    let Some(path) = w.next() else { return "padplay: want a path".into() };
    let opts: Vec<&str> = w.collect();
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => return format!("padplay: cannot read {path}: {e}"),
    };
    let rec = match pad::parse(&text) {
        Ok(r) => r,
        Err(e) => return format!("padplay: {path}: {e}"),
    };
    let mut said = String::new();
    if let Some(s) = rec.start.filter(|_| !opts.contains(&"here")) {
        let yaw = if s[3].is_finite() { format!(" {}", s[3]) } else { String::new() };
        said = format!("; {}", trace::teleport(&format!("{} {} {}{yaw}", s[0], s[1], s[2])));
    }
    let n = rec.frames.len();
    let by_frame = opts.contains(&"frames");
    with_pad(|s| s.play = Some(Playback::new(rec.frames, by_frame)));
    format!("padplay {path}: {n} records{}{said}", if by_frame { ", one an update" } else { ", by game time" })
}
