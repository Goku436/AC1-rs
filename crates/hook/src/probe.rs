//! Probes: a breakpoint at a code address that logs some fields each time the game runs it, and lets it go on.
//! `probe <name> @<address> [hits=<n>] [field...]` (fields as `expr` takes them, the registers among the names; none:
//! the general registers), at most `n` hits logged (20 by default), then the probe takes itself out; `probe off
//! [name]`; `probes` lists them.
//!
//! How: an `int3` (0xCC) written over the instruction's first byte; a vectored exception handler (ours, first in
//! line) takes the breakpoint, logs, puts the byte back, sets the trap flag and runs the instruction once; the single
//! step after it writes the `int3` back. Another thread passing there in that moment is not seen. A breakpoint not at
//! an armed probe's address is left to the next handler.
//!
//! x86 `CONTEXT` offsets used: edi +0x9C, esi +0xA0, ebx +0xA4, edx +0xA8, ecx +0xAC, eax +0xB0, ebp +0xB4, eip +0xB8,
//! eflags +0xC0, esp +0xC4.

use std::cell::Cell;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::expr::{self, Kind};
use crate::memcmd::{self, Live};
use crate::sys;

const BREAKPOINT: u32 = 0x8000_0003;
const SINGLE_STEP: u32 = 0x8000_0004;
const CONTINUE_EXECUTION: i32 = -1;
const CONTINUE_SEARCH: i32 = 0;
const REGS: [(&str, usize); 9] =
    [("edi", 0x9C), ("esi", 0xA0), ("ebx", 0xA4), ("edx", 0xA8), ("ecx", 0xAC), ("eax", 0xB0), ("ebp", 0xB4), ("eip", 0xB8), ("esp", 0xC4)];
const EFLAGS: usize = 0xC0;
const TRAP: u32 = 0x100;

struct Probe {
    name: String,
    addr: usize,
    orig: u8,
    fields: Vec<(String, Kind)>,
    hits: u32,
    limit: u32,
    armed: bool,
}

static PROBES: Mutex<Vec<Probe>> = Mutex::new(Vec::new());
static HANDLER: AtomicBool = AtomicBool::new(false);
/// Every address a probe was ever put at: an `int3` taken there just as its probe went out is let through.
static PAST: Mutex<Vec<usize>> = Mutex::new(Vec::new());

thread_local! {
    /// The probe address this thread is stepping over (0: none).
    static STEPPING: Cell<usize> = const { Cell::new(0) };
}

/// # Safety
/// `ctx` is the thread's x86 CONTEXT.
unsafe fn reg(ctx: *mut u8, at: usize) -> *mut u32 {
    // SAFETY: inside the CONTEXT.
    unsafe { ctx.add(at) as *mut u32 }
}

unsafe extern "system" fn handler(info: *mut [*mut u8; 2]) -> i32 {
    // SAFETY: Windows hands us EXCEPTION_POINTERS: the record (code +0, address +12) and the thread's CONTEXT.
    unsafe {
        let (rec, ctx) = ((*info)[0], (*info)[1]);
        let code = *(rec as *const u32);
        if code == SINGLE_STEP {
            let at = STEPPING.with(|s| s.replace(0));
            if at == 0 {
                return CONTINUE_SEARCH;
            }
            *reg(ctx, EFLAGS) &= !TRAP;
            let probes = PROBES.lock().unwrap();
            if probes.iter().any(|p| p.addr == at && p.armed) {
                sys::write(at, &[0xCC]);
            }
            return CONTINUE_EXECUTION;
        }
        if code != BREAKPOINT {
            return CONTINUE_SEARCH;
        }
        let at = *(rec.add(12) as *const usize);
        let mut probes = PROBES.lock().unwrap();
        let Some(p) = probes.iter_mut().find(|p| p.addr == at && p.armed) else {
            if PAST.lock().unwrap().contains(&at) && *(at as *const u8) != 0xCC {
                *reg(ctx, 0xB8) = at as u32;
                return CONTINUE_EXECUTION;
            }
            return CONTINUE_SEARCH;
        };
        p.hits += 1;
        let regs: Vec<(&str, u32)> = REGS.iter().map(|&(n, o)| (n, *reg(ctx, o))).collect();
        let name = |n: &str| regs.iter().find(|r| r.0 == n).map(|r| r.1).or_else(|| memcmd::global(n));
        let said: Vec<String> = if p.fields.is_empty() {
            regs.iter().map(|(n, v)| format!("{n}={v:#x}")).collect()
        } else {
            p.fields
                .iter()
                .map(|(a, k)| {
                    let v = expr::eval(a, &name, &Live).map(|at| expr::show(*k, at, &Live, &memcmd::class));
                    format!("{a}={}", v.unwrap_or_else(|e| format!("({e})")))
                })
                .collect()
        };
        sys::log(&format!("[probe {} #{}] {}", p.name, p.hits, said.join(" ")));
        // The instruction back, run once (trap flag), the int3 back after it (unless that was the last hit).
        sys::write(at, &[p.orig]);
        if p.hits >= p.limit {
            p.armed = false;
            sys::log(&format!("[probe {}] {} hits: out", p.name, p.hits));
        } else {
            *reg(ctx, EFLAGS) |= TRAP;
            STEPPING.with(|s| s.set(at));
        }
        *reg(ctx, 0xB8) = at as u32;
        CONTINUE_EXECUTION
    }
}

/// `probe <name> @<address> [hits=<n>] [field...]` / `probe off [name]`.
pub fn command(rest: &str) -> String {
    let w: Vec<&str> = rest.split_whitespace().collect();
    match w.first() {
        Some(&"off") => off(w.get(1).copied()),
        Some(name) => {
            let Some(at) = w.get(1).and_then(|a| a.strip_prefix('@')) else { return "probe: want <name> @<address> [hits=n] [field...]".into() };
            let addr = match expr::eval(at, &memcmd::global, &Live) {
                Ok(a) => a as usize,
                Err(e) => return format!("probe: {e}"),
            };
            let mut limit = 20;
            let mut fields = Vec::new();
            for f in &w[2..] {
                if let Some(n) = f.strip_prefix("hits=") {
                    limit = n.parse().unwrap_or(20u32).clamp(1, 100_000);
                    continue;
                }
                match expr::field(f) {
                    Ok(x) => fields.push(x),
                    Err(e) => return format!("probe: {e}"),
                }
            }
            arm(name, addr, fields, limit)
        }
        None => list(),
    }
}

fn arm(name: &str, addr: usize, fields: Vec<(String, Kind)>, limit: u32) -> String {
    if !sys::executable(addr) {
        return format!("probe {name}: {addr:#x} is not code");
    }
    if !HANDLER.swap(true, Ordering::SeqCst) {
        // SAFETY: our handler, with the signature Windows calls.
        let h = unsafe { sys::AddVectoredExceptionHandler(1, handler as *const () as usize) };
        if h.is_null() {
            HANDLER.store(false, Ordering::SeqCst);
            return "probe: AddVectoredExceptionHandler failed".into();
        }
    }
    let mut probes = PROBES.lock().unwrap();
    if probes.iter().any(|p| p.name == name && p.armed) {
        return format!("probe {name}: already armed (probe off {name} first)");
    }
    if probes.iter().any(|p| p.addr == addr && p.armed) {
        return format!("probe {name}: another probe is at {addr:#x}");
    }
    // SAFETY: executable, so committed and readable.
    let orig = unsafe { *(addr as *const u8) };
    if orig == 0xCC {
        return format!("probe {name}: {addr:#x} already holds an int3");
    }
    probes.retain(|p| p.name != name);
    let mut past = PAST.lock().unwrap();
    if !past.contains(&addr) {
        past.push(addr);
    }
    drop(past);
    let n = fields.len();
    probes.push(Probe { name: name.to_string(), addr, orig, fields, hits: 0, limit, armed: true });
    if !sys::write(addr, &[0xCC]) {
        probes.pop();
        return format!("probe {name}: cannot write at {addr:#x}");
    }
    format!("probe {name} at {addr:#x}: {n} fields, up to {limit} hits")
}

fn off(name: Option<&str>) -> String {
    let mut probes = PROBES.lock().unwrap();
    let mut n = 0;
    for p in probes.iter_mut().filter(|p| name.is_none_or(|w| p.name == w)) {
        if p.armed {
            sys::write(p.addr, &[p.orig]);
            p.armed = false;
        }
        n += 1;
    }
    probes.retain(|p| name.is_some_and(|w| p.name != w));
    format!("probe off: {n} taken out")
}

fn list() -> String {
    let probes = PROBES.lock().unwrap();
    let l: Vec<String> =
        probes.iter().map(|p| format!("{} @{:#x} {}/{} hits{}", p.name, p.addr, p.hits, p.limit, if p.armed { "" } else { " (out)" })).collect();
    format!("{} probes: {}", l.len(), l.join("; "))
}
