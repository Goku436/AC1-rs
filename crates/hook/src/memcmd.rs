//! Memory commands (`expr` reads their addresses): `peek`, `poke`, `fields`, `classes`, `components`, and the
//! position bookmarks (`save`, `load`, `marks`, in `ac1-hook.bookmarks` beside the exe).
//!
//! The entity's components: `Entity` +0x74 an array of pointers, +0x78 its count (low 14 bits), as `Entity`'s reader
//! (0x450E90) walks it; each named by its RTTI class.

use crate::expr::{self, Kind};
use crate::livefmt::Mem;
use crate::{control, sys, trace};

const ENTITY_COMPONENTS: u32 = 0x74;
const MARKS: &str = "ac1-hook.bookmarks";

/// The game's memory, each read checked.
pub struct Live;
impl Mem for Live {
    fn bytes(&self, addr: u32, out: &mut [u8]) -> bool {
        if !sys::readable(addr as usize, out.len()) {
            return false;
        }
        // SAFETY: checked readable.
        unsafe { std::ptr::copy_nonoverlapping(addr as usize as *const u8, out.as_mut_ptr(), out.len()) };
        true
    }
}

/// The names an address can use: the player's objects, the pad, the clock.
pub fn global(name: &str) -> Option<u32> {
    let v = match name {
        "human" => trace::human(),
        "entity" => trace::entity(),
        "interp" => trace::interp(),
        "pad" => control::pad_proxy(),
        "clock" => control::clock(),
        "actions" => {
            let h = trace::human();
            if h == 0 { 0 } else { sys::peek(h + 0x7dc).unwrap_or(0) as usize }
        }
        _ => return None,
    };
    (v != 0).then_some(v as u32)
}

pub fn class(addr: u32) -> Option<String> {
    sys::class_of(addr as usize)
}

fn addr(text: &str) -> Result<u32, String> {
    expr::eval(text, &global, &Live)
}

/// `peek <address> [bytes]` (64 by default, at most 4096): a hex dump.
pub fn peek(rest: &str) -> String {
    let mut w = rest.split_whitespace();
    let Some(a) = w.next() else { return "peek: want an address".into() };
    let n: usize = w.next().and_then(|n| n.parse().ok()).unwrap_or(64).clamp(1, 4096);
    let at = match addr(a) {
        Ok(v) => v,
        Err(e) => return format!("peek: {e}"),
    };
    let mut buf = vec![0u8; n];
    if !Live.bytes(at, &mut buf) {
        return format!("peek {at:#x}: not readable");
    }
    format!("peek {a} = {at:#x} ({n} bytes):\n{}", expr::dump(at, &buf))
}

/// `poke <address> <u8|u16|u32|i32|f32|bytes> <value>...`.
pub fn poke(rest: &str) -> String {
    let w: Vec<&str> = rest.split_whitespace().collect();
    if w.len() < 3 {
        return "poke: want <address> <type> <value>...".into();
    }
    let at = match addr(w[0]) {
        Ok(v) => v,
        Err(e) => return format!("poke: {e}"),
    };
    let bytes = match expr::poke_bytes(w[1], &w[2..]) {
        Ok(b) => b,
        Err(e) => return format!("poke: {e}"),
    };
    let mut was = vec![0u8; bytes.len()];
    let had = Live.bytes(at, &mut was);
    if !had || !sys::write(at as usize, &bytes) {
        return format!("poke {at:#x}: not writable");
    }
    format!("poke {at:#x}: {} bytes (were {})", bytes.len(), was.iter().map(|b| format!("{b:02x}")).collect::<String>())
}

/// `fields <address[:type]>...`: each field's value.
pub fn fields(rest: &str) -> String {
    let mut out = Vec::new();
    for spec in rest.split_whitespace() {
        let v = expr::field(spec).and_then(|(a, k)| addr(&a).map(|at| expr::show(k, at, &Live, &class)));
        out.push(format!("{spec}={}", v.unwrap_or_else(|e| format!("({e})"))));
    }
    if out.is_empty() { "fields: want address[:type]...".into() } else { out.join(" ") }
}

/// `classes <address> [words]` (16 by default, at most 256): each u32 from there, and the class of what it points at.
pub fn classes(rest: &str) -> String {
    let mut w = rest.split_whitespace();
    let Some(a) = w.next() else { return "classes: want an address".into() };
    let n: u32 = w.next().and_then(|n| n.parse().ok()).unwrap_or(16).clamp(1, 256);
    let at = match addr(a) {
        Ok(v) => v,
        Err(e) => return format!("classes: {e}"),
    };
    let mut lines = vec![format!("classes from {at:#x}:")];
    for k in 0..n {
        lines.push(format!("  +{:#05x} {}", 4 * k, expr::show(Kind::PtrClass, at + 4 * k, &Live, &class)));
    }
    lines.join("\n")
}

/// `components [entity address]`: the entity's components (the player's by default).
pub fn components(rest: &str) -> String {
    let e = if rest.trim().is_empty() { global("entity").ok_or_else(|| "player not known yet".to_string()) } else { addr(rest.trim()) };
    let e = match e {
        Ok(v) => v,
        Err(e) => return format!("components: {e}"),
    };
    let m = Live;
    let (Some(list), Some(count)) = (m.u32(e + ENTITY_COMPONENTS), m.u32(e + ENTITY_COMPONENTS + 4)) else {
        return format!("components: entity {e:#x} not readable");
    };
    let n = (count & 0x3fff).min(256);
    let mut lines = vec![format!("entity {e:#x} ({}): {n} components", class(e).unwrap_or_else(|| "?".into()))];
    for k in 0..n {
        let c = m.u32(list + 4 * k).unwrap_or(0);
        lines.push(format!("  [{k}] {c:#x} {}", class(c).unwrap_or_else(|| "?".into())));
    }
    lines.join("\n")
}

/// `save <name>`: the player's place and yaw now into the bookmarks; `load <name>`: back there; `marks`: the list.
pub fn marks(word: &str, rest: &str) -> String {
    let path = sys::game_file(MARKS);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let name = rest.split_whitespace().next();
    match (word, name) {
        ("save", Some(n)) => match trace::player_at() {
            Some((p, yaw)) => match std::fs::write(&path, expr::set_mark(&text, n, [p[0], p[1], p[2], yaw])) {
                Ok(()) => format!("save {n}: ({:.3}, {:.3}, {:.3}) yaw {yaw:.1}", p[0], p[1], p[2]),
                Err(e) => format!("save: cannot write {MARKS}: {e}"),
            },
            None => "save: player not known yet".into(),
        },
        ("load", Some(n)) => match expr::marks(&text).into_iter().find(|(m, _)| m == n) {
            Some((_, p)) => format!("load {n}: {}", trace::teleport(&format!("{} {} {} {}", p[0], p[1], p[2], p[3]))),
            None => format!("load: no bookmark {n} in {MARKS}"),
        },
        ("marks", _) => {
            let all = expr::marks(&text);
            let list: Vec<String> = all.iter().map(|(n, p)| format!("{n} ({:.2}, {:.2}, {:.2}) {:.0}", p[0], p[1], p[2], p[3])).collect();
            format!("{} bookmarks: {}", all.len(), list.join("; "))
        }
        _ => format!("{word}: want a name"),
    }
}
