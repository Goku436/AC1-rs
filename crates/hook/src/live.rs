//! The live log: the player frame by frame into `ac1-live.txt` beside the exe, in the flight recorder's line format
//! (`livefmt`), and what happens into `ac1-live-events.txt` (context switches, takeoffs, landings, grabs, actions,
//! teleports). Off unless asked for: `live on [MB]` / `live off` in `ac1-hook.cmd`, or the file `ac1-hook.live` beside
//! the exe (its presence turns it on, its removal off; it may hold the size limit in MB). Off, a frame costs one atomic
//! load (and a file check a second).
//!
//! Each file starts over past its limit (50 MB for the frames, 8 MB for the events by default), the last two kept as
//! `.1` and `.2`. Writes are buffered and flushed twice a second, so a reader can follow the file live (the `ac1`
//! example `livelog tail`). Action names come from `ac1-hook.actions.tsv` beside the exe if it is there (`livelog names
//! <game dir>` writes it; without it the ids alone are written and the reader names them).

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::livefmt::{self, Events, Mem, Names, Rotating, Sample};
use crate::{sys, trace};

static ON: AtomicBool = AtomicBool::new(false);
static STATE: Mutex<Option<Live>> = Mutex::new(None);
/// Turned on by the flag file (and so off when it goes).
static BY_FILE: AtomicBool = AtomicBool::new(false);

const FRAMES: &str = "ac1-live.txt";
const EVENTS: &str = "ac1-live-events.txt";
const FLAG: &str = "ac1-hook.live";
const NAMES: &str = "ac1-hook.actions.tsv";
const MB: u64 = 1 << 20;
const DEFAULT_MB: u64 = 50;
const KEEP: u32 = 2;
const HEADER: &str = "# ac1-hook live log: the real AC1's player frame by frame, the flight recorder's format (Bevy space: game \
x = x, game y = -z, game z = y; time = game seconds since the log started)\n# time root x y z yaw <deg> vel <m/s> | <context> \
clip <the last action's clip> | bones <name x,y,z>... | targets <IK goals> | ac1 frame= wall= mode= dt= sub= speed= act= \
started=id:clip,... stick= latch= btn= high= legs= hand= cam= camdir=\n";

struct Live {
    frames: Rotating,
    events: Rotating,
    names: Names,
    ev: Events,
    start: std::time::Instant,
    t: f32,
    wall: f32,
    last_pos: Option<[f32; 3]>,
    playing: Option<u32>,
    /// Context classes by vtable (the RTTI walk once per class).
    classes: Vec<(u32, String)>,
    /// What was found the first time, said once in the hook's log.
    said: u8,
    unflushed: u32,
}

pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// The game's memory, each readable region asked of Windows once a frame.
#[derive(Default)]
struct GameMem {
    regions: std::cell::RefCell<Vec<(u32, u32)>>,
}

impl GameMem {
    fn region(&self, addr: u32) -> Option<(u32, u32)> {
        if let Some(r) = self.regions.borrow().iter().find(|r| addr >= r.0 && addr < r.1) {
            return Some(*r);
        }
        if addr < 0x10000 {
            return None;
        }
        let mut info = sys::MemoryInfo::default();
        // SAFETY: VirtualQuery only fills the struct.
        let n = unsafe { sys::VirtualQuery(addr as usize as *const std::ffi::c_void, &mut info, std::mem::size_of::<sys::MemoryInfo>()) };
        const MEM_COMMIT: u32 = 0x1000;
        const PAGE_NOACCESS: u32 = 0x01;
        const PAGE_GUARD: u32 = 0x100;
        if n == 0 || info.state != MEM_COMMIT || info.protect & (PAGE_NOACCESS | PAGE_GUARD) != 0 {
            return None;
        }
        let r = (info.base as u32, (info.base + info.size).min(u32::MAX as usize) as u32);
        let mut regions = self.regions.borrow_mut();
        if regions.len() > 64 {
            regions.clear();
        }
        regions.push(r);
        Some(r)
    }
}

impl Mem for GameMem {
    fn bytes(&self, addr: u32, out: &mut [u8]) -> bool {
        let Some(end) = addr.checked_add(out.len() as u32) else { return false };
        let Some((_, top)) = self.region(addr) else { return false };
        if end > top && self.region(top).is_none_or(|(_, t)| end > t) {
            return false;
        }
        // SAFETY: committed, readable memory of this process, checked above.
        unsafe { std::ptr::copy_nonoverlapping(addr as usize as *const u8, out.as_mut_ptr(), out.len()) };
        true
    }
}

fn start(limit_mb: u64) -> Result<Live, String> {
    let limit = limit_mb.clamp(1, 4096) * MB;
    let frames = Rotating::create(sys::game_file(FRAMES), HEADER, limit, KEEP).map_err(|e| format!("{FRAMES}: {e}"))?;
    let events = Rotating::create(sys::game_file(EVENTS), "# ac1-hook live events (Bevy space; game space in brackets)\n", (limit / 4).clamp(MB, 8 * MB), KEEP)
        .map_err(|e| format!("{EVENTS}: {e}"))?;
    let names = std::fs::read_to_string(sys::game_file(NAMES)).map(|t| Names::parse(&t)).unwrap_or_default();
    Ok(Live {
        frames,
        events,
        names,
        ev: Events::default(),
        start: std::time::Instant::now(),
        t: 0.0,
        wall: 0.0,
        last_pos: None,
        playing: None,
        classes: Vec::new(),
        said: 0,
        unflushed: 0,
    })
}

/// `live on [MB]` / `live off` / `live` (say whether it is on).
pub fn command(arg: &str) -> String {
    let mut words = arg.split_whitespace();
    match words.next() {
        Some("on") => {
            let mb = words.next().and_then(|w| w.parse().ok()).unwrap_or(DEFAULT_MB);
            BY_FILE.store(false, Ordering::SeqCst);
            turn_on(mb)
        }
        Some("off") => turn_off(),
        _ => format!("live log {}", if on() { "on" } else { "off" }),
    }
}

fn turn_on(mb: u64) -> String {
    let mut state = STATE.lock().unwrap();
    if state.is_some() {
        return "live log already on".into();
    }
    match start(mb) {
        Ok(l) => {
            let n = l.names.len();
            *state = Some(l);
            ON.store(true, Ordering::SeqCst);
            format!("live log on: {FRAMES} (limit {mb} MB, 2 kept) and {EVENTS}; {n} action names")
        }
        Err(e) => format!("live log: cannot start: {e}"),
    }
}

fn turn_off() -> String {
    ON.store(false, Ordering::SeqCst);
    let mut state = STATE.lock().unwrap();
    match state.take() {
        Some(mut l) => {
            let line = format!("{:8.3} live off", l.t);
            let _ = l.events.line(&line);
            "live log off".into()
        }
        None => "live log was off".into(),
    }
}

/// The flag file (`ac1-hook.live`), looked at about once a second: there, the log on (its first number the limit in
/// MB); gone, off again if it was the file that turned it on.
pub fn poll_flag(n: u32) {
    if !n.is_multiple_of(60) {
        return;
    }
    let flag = sys::game_file(FLAG);
    let there = flag.exists();
    if there && !on() {
        let mb = std::fs::read_to_string(&flag).ok().and_then(|t| t.split_whitespace().find_map(|w| w.parse().ok())).unwrap_or(DEFAULT_MB);
        BY_FILE.store(true, Ordering::SeqCst);
        sys::log(&format!("[frame {n}] {} (the flag file)", turn_on(mb)));
    } else if !there && on() && BY_FILE.load(Ordering::SeqCst) {
        BY_FILE.store(false, Ordering::SeqCst);
        sys::log(&format!("[frame {n}] {} (the flag file is gone)", turn_off()));
    }
}

/// Once a frame (from `Present`): the frame's line and its events.
pub fn frame(n: u32, started: &[u32]) {
    if !on() {
        return;
    }
    let mut state = STATE.lock().unwrap();
    let Some(l) = state.as_mut() else { return };
    let human = trace::human() as u32;
    if human == 0 {
        return;
    }
    let m = GameMem::default();
    let Some(entity) = m.u32(human + 0x104) else { return };
    let Some(mat) = m.matrix(entity + 0x10) else { return };
    let pos = [mat[12], mat[13], mat[14]];
    let yaw = mat[5].atan2(mat[4]).to_degrees();

    let wall = l.start.elapsed().as_secs_f32();
    let wall_dt = wall - l.wall;
    l.wall = wall;
    let dt = m.f32(livefmt::FRAME_DT).filter(|d| d.is_finite() && *d > 0.0 && *d < 0.5);
    let step = dt.unwrap_or(wall_dt);
    if l.last_pos.is_some() {
        l.t += step;
    }
    let speed = match l.last_pos {
        Some(p) if step > 1e-4 => ((pos[0] - p[0]).powi(2) + (pos[1] - p[1]).powi(2) + (pos[2] - p[2]).powi(2)).sqrt() / step,
        _ => 0.0,
    };
    l.last_pos = Some(pos);

    let context = context_of(l, &m, human);
    if let Some(&last) = started.last() {
        l.playing = Some(last);
    }
    let (sub, ground_speed) = livefmt::sub_state(&m, human, &context);
    let bones: Vec<_> = livefmt::bones(&m, human).into_iter().filter(|(_, b)| near(*b, pos)).collect();
    let targets: Vec<_> = (0..4).filter_map(|k| livefmt::limb_goal(&m, human, k)).filter(|t| near(*t, pos)).collect();
    let interp = trace::interp() as u32;
    let input = if interp != 0 { livefmt::input(&m, interp) } else { None };
    let camera = livefmt::camera(&m);
    say_found(l, n, &m, human, interp, !bones.is_empty(), input.is_some(), camera.is_some());

    let s = Sample {
        t: l.t,
        wall,
        frame: n,
        pos,
        yaw,
        speed,
        context,
        sub,
        ground_speed,
        playing: l.playing,
        started: started.to_vec(),
        bones,
        targets,
        input,
        camera,
        dt,
    };
    let line = s.line(&l.names);
    let _ = l.frames.line(&line);
    for e in l.ev.next(&s, &l.names) {
        let _ = l.events.line(&e);
    }
    l.unflushed += 1;
    if l.unflushed >= 30 {
        l.unflushed = 0;
        l.frames.flush();
        l.events.flush();
    }
}

/// Within 5 m of the root: a bone or goal read from a stale or moved structure is left out, not logged as garbage.
fn near(p: [f32; 3], root: [f32; 3]) -> bool {
    (p[0] - root[0]).powi(2) + (p[1] - root[1]).powi(2) + (p[2] - root[2]).powi(2) < 25.0
}

/// The locomotion context's class name (`Human` +0x48), by its vtable.
fn context_of(l: &mut Live, m: &GameMem, human: u32) -> String {
    let Some(ctx) = m.u32(human + 0x48).filter(|&c| c != 0) else { return "?".into() };
    let Some(vtable) = m.u32(ctx) else { return "?".into() };
    if let Some((_, name)) = l.classes.iter().find(|(v, _)| *v == vtable) {
        return name.clone();
    }
    let name = sys::class_of(ctx as usize).unwrap_or_else(|| "?".into());
    l.classes.push((vtable, name.clone()));
    name
}

/// The first time each part is found, the classes it was found in, in the hook's log (to check the layouts live).
#[allow(clippy::too_many_arguments)]
fn say_found(l: &mut Live, n: u32, m: &GameMem, human: u32, interp: u32, bones: bool, input: bool, camera: bool) {
    let class = |at: Option<u32>| at.filter(|&a| a != 0).and_then(|a| sys::class_of(a as usize)).unwrap_or_else(|| "?".into());
    if bones && l.said & 1 == 0 {
        l.said |= 1;
        let comp = livefmt::handle(m, human + livefmt::BONE_HANDLES[0].0).map(|h| h.0);
        sys::log(&format!("[frame {n}] live: bones read, skeleton component {}", class(comp)));
    }
    if input && l.said & 2 == 0 {
        l.said |= 2;
        sys::log(&format!("[frame {n}] live: input read, pad {}", class(m.u32(interp + 0xD0))));
    }
    if camera && l.said & 4 == 0 {
        l.said |= 4;
        let manager = m.u32(livefmt::CAMERA_MANAGER);
        let switcher = manager.and_then(|c| m.u32(c + 0xC)).and_then(|r| livefmt::deref(m, r));
        sys::log(&format!("[frame {n}] live: camera read: {} -> {} -> {}", class(manager), class(switcher), class(switcher.and_then(|s| m.u32(s + 0x74)))));
    }
}
