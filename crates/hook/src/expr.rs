//! Addresses and fields as the memory commands and probes take them (pure Rust, tested on any target), and the
//! position bookmarks' file.
//!
//! An address: terms joined by `+` and `-`; a term is a number (hex, with or without `0x`; `#` before it for decimal),
//! a name (`human`, `entity`, `interp`, `pad`, `clock`, `actions`; in a probe also the registers `eax` ... `edi`, `esp`,
//! `ebp`, `eip`), or `[address]`: the u32 there. E.g. `[human+48]` the context object, `[esp+4]` a probe's first
//! stack argument, `entity+10` the world matrix.
//!
//! A field: `address[:type]`, the type `u8`, `u16`, `u32` (the default), `i32`, `f32`, `x` (u32 in hex), `vec3`
//! (3 floats), `m4` (16 floats, a matrix), `class` (the RTTI class of the object there), `ptrclass` (the class of the
//! object the u32 there points at).

use crate::livefmt::Mem;

/// Evaluate an address: `name` gives a name's value (None: not known), `mem` reads the `[...]`.
pub fn eval(text: &str, name: &dyn Fn(&str) -> Option<u32>, mem: &dyn Mem) -> Result<u32, String> {
    let s: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
    let mut at = 0;
    let v = sum(&s, &mut at, name, mem)?;
    if at != s.len() {
        return Err(format!("`{text}`: unexpected `{}`", s[at..].iter().collect::<String>()));
    }
    Ok(v)
}

fn sum(s: &[char], at: &mut usize, name: &dyn Fn(&str) -> Option<u32>, mem: &dyn Mem) -> Result<u32, String> {
    let mut v = term(s, at, name, mem)?;
    while *at < s.len() && (s[*at] == '+' || s[*at] == '-') {
        let minus = s[*at] == '-';
        *at += 1;
        let t = term(s, at, name, mem)?;
        v = if minus { v.wrapping_sub(t) } else { v.wrapping_add(t) };
    }
    Ok(v)
}

fn term(s: &[char], at: &mut usize, name: &dyn Fn(&str) -> Option<u32>, mem: &dyn Mem) -> Result<u32, String> {
    if *at < s.len() && s[*at] == '[' {
        *at += 1;
        let a = sum(s, at, name, mem)?;
        if *at >= s.len() || s[*at] != ']' {
            return Err("missing `]`".into());
        }
        *at += 1;
        return mem.u32(a).ok_or_else(|| format!("[{a:#x}] not readable"));
    }
    let start = *at;
    while *at < s.len() && (s[*at].is_ascii_alphanumeric() || s[*at] == '_' || s[*at] == '#') {
        *at += 1;
    }
    let word: String = s[start..*at].iter().collect();
    if word.is_empty() {
        return Err(format!("want a number, name or [..] at `{}`", s[start..].iter().collect::<String>()));
    }
    if let Some(dec) = word.strip_prefix('#') {
        return dec.parse().map_err(|_| format!("bad decimal `{word}`"));
    }
    let lower = word.to_ascii_lowercase();
    let hex = lower.strip_prefix("0x").unwrap_or(&lower);
    if let Ok(v) = u32::from_str_radix(hex, 16)
        && name(&lower).is_none()
    {
        return Ok(v);
    }
    name(&lower).ok_or_else(|| format!("`{word}` is not known (yet)"))
}

/// A field's type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    U8,
    U16,
    U32,
    I32,
    F32,
    Hex,
    Vec3,
    M4,
    Class,
    PtrClass,
}

impl Kind {
    pub fn parse(word: &str) -> Option<Kind> {
        Some(match word {
            "u8" => Kind::U8,
            "u16" => Kind::U16,
            "u32" => Kind::U32,
            "i32" => Kind::I32,
            "f32" | "f" => Kind::F32,
            "x" | "hex" => Kind::Hex,
            "vec3" | "v3" => Kind::Vec3,
            "m4" => Kind::M4,
            "class" => Kind::Class,
            "ptrclass" => Kind::PtrClass,
            _ => return None,
        })
    }
}

/// `address[:type]`, split (the address left to evaluate when it is used).
pub fn field(spec: &str) -> Result<(String, Kind), String> {
    // (The type after the last `:` outside brackets.)
    match spec.rsplit_once(':') {
        Some((a, t)) if !t.contains(']') => Ok((a.to_string(), Kind::parse(t).ok_or_else(|| format!("unknown type `{t}` in `{spec}`"))?)),
        _ => Ok((spec.to_string(), Kind::U32)),
    }
}

/// The value of a field at `addr`, as text; `class` names an object's class (Windows-side: RTTI).
pub fn show(kind: Kind, addr: u32, mem: &dyn Mem, class: &dyn Fn(u32) -> Option<String>) -> String {
    let floats = |n: usize| -> Option<String> {
        let v: Option<Vec<String>> = (0..n as u32).map(|k| mem.f32(addr + 4 * k).map(|f| format!("{f:.4}"))).collect();
        v.map(|v| v.join(","))
    };
    let s = match kind {
        Kind::U8 => mem.u8(addr).map(|v| v.to_string()),
        Kind::U16 => mem.u16(addr).map(|v| v.to_string()),
        Kind::U32 => mem.u32(addr).map(|v| v.to_string()),
        Kind::I32 => mem.u32(addr).map(|v| (v as i32).to_string()),
        Kind::F32 => mem.f32(addr).map(|v| format!("{v:.5}")),
        Kind::Hex => mem.u32(addr).map(|v| format!("{v:#x}")),
        Kind::Vec3 => floats(3),
        Kind::M4 => floats(16),
        Kind::Class => Some(class(addr).unwrap_or_else(|| "?".into())),
        Kind::PtrClass => mem.u32(addr).map(|p| format!("{p:#x} {}", class(p).unwrap_or_else(|| "?".into()))),
    };
    s.unwrap_or_else(|| "(not readable)".into())
}

/// The bytes a `poke` writes: `<type> <values...>` (u8, u16, u32, i32, f32 one value a word; `bytes` hex bytes).
pub fn poke_bytes(kind: &str, values: &[&str]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for v in values {
        let bad = || format!("bad {kind} value `{v}`");
        let hexnum = |v: &str| u32::from_str_radix(v.trim_start_matches("0x"), 16);
        let int = |v: &str| if v.starts_with("0x") { hexnum(v).ok().map(|x| x as i64) } else { v.parse::<i64>().ok() };
        match kind {
            "u8" => out.push(int(v).filter(|x| (0..256).contains(x)).ok_or_else(bad)? as u8),
            "u16" => out.extend_from_slice(&(int(v).filter(|x| (0..65536).contains(x)).ok_or_else(bad)? as u16).to_le_bytes()),
            "u32" | "i32" => out.extend_from_slice(&(int(v).filter(|x| (i32::MIN as i64..=u32::MAX as i64).contains(x)).ok_or_else(bad)? as u32).to_le_bytes()),
            "f32" => out.extend_from_slice(&v.parse::<f32>().map_err(|_| bad())?.to_le_bytes()),
            "bytes" => {
                let h = v.trim_start_matches("0x");
                if h.len() % 2 != 0 {
                    return Err(bad());
                }
                for k in (0..h.len()).step_by(2) {
                    out.push(u8::from_str_radix(&h[k..k + 2], 16).map_err(|_| bad())?);
                }
            }
            _ => return Err(format!("unknown poke type `{kind}` (u8, u16, u32, i32, f32, bytes)")),
        }
    }
    if out.is_empty() {
        return Err("poke: nothing to write".into());
    }
    Ok(out)
}

/// A hex dump, 16 bytes a line, each line with its address.
pub fn dump(addr: u32, bytes: &[u8]) -> String {
    bytes
        .chunks(16)
        .enumerate()
        .map(|(k, c)| {
            let hex: Vec<String> = c.iter().map(|b| format!("{b:02x}")).collect();
            format!("{:08x}  {}", addr as usize + 16 * k, hex.join(" "))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The bookmarks file: `name x y z yaw` a line. Returns them in file order.
pub fn marks(text: &str) -> Vec<(String, [f32; 4])> {
    text.lines()
        .filter_map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            let v: Vec<f32> = w.get(1..)?.iter().filter_map(|x| x.parse().ok()).collect();
            (w.len() >= 5 && v.len() == 4 && !w[0].starts_with('#')).then(|| (w[0].to_string(), [v[0], v[1], v[2], v[3]]))
        })
        .collect()
}

/// The bookmarks file with `name` set to `at` (replaced where it was, else added at the end).
pub fn set_mark(text: &str, name: &str, at: [f32; 4]) -> String {
    let mut all = marks(text);
    match all.iter_mut().find(|(n, _)| n == name) {
        Some(m) => m.1 = at,
        None => all.push((name.to_string(), at)),
    }
    all.iter().map(|(n, p)| format!("{n} {:.4} {:.4} {:.4} {:.2}\n", p[0], p[1], p[2], p[3])).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(Vec<(u32, Vec<u8>)>);
    impl Mem for Fake {
        fn bytes(&self, addr: u32, out: &mut [u8]) -> bool {
            for (base, data) in &self.0 {
                if addr >= *base && (addr - base) as usize + out.len() <= data.len() {
                    let o = (addr - base) as usize;
                    out.copy_from_slice(&data[o..o + out.len()]);
                    return true;
                }
            }
            false
        }
    }

    fn names(n: &str) -> Option<u32> {
        match n {
            "human" => Some(0x1000),
            "ecx" => Some(0x2000),
            "add" => Some(7),
            _ => None,
        }
    }

    #[test]
    fn addresses() {
        let mut block = vec![0u8; 0x100];
        block[0x48..0x4C].copy_from_slice(&0x2010u32.to_le_bytes());
        block[0x10..0x14].copy_from_slice(&0x55u32.to_le_bytes());
        let m = Fake(vec![(0x1000, block), (0x2000, vec![0x10, 0, 0, 0, 0, 0, 0, 0])]);
        let e = |t: &str| eval(t, &names, &m);
        assert_eq!(e("1234"), Ok(0x1234));
        assert_eq!(e("0x10 + #16"), Ok(0x20));
        assert_eq!(e("human+48"), Ok(0x1048));
        assert_eq!(e("[human+48]"), Ok(0x2010));
        assert_eq!(e("[[ecx]+human]"), Ok(0x55));
        assert_eq!(e("human-0x10"), Ok(0xFF0));
        // A name that also reads as hex is the name.
        assert_eq!(e("add+1"), Ok(8));
        assert!(e("[5]").is_err());
        assert!(e("entity").is_err());
        assert!(e("[human").is_err());
        assert!(e("human)").is_err());
        assert!(e("").is_err());
    }

    #[test]
    fn fields() {
        assert_eq!(field("[esp+4]:f32"), Ok(("[esp+4]".into(), Kind::F32)));
        assert_eq!(field("ecx+48"), Ok(("ecx+48".into(), Kind::U32)));
        assert!(field("ecx:what").is_err());
        let mut d = vec![0u8; 64];
        d[0..4].copy_from_slice(&1.5f32.to_le_bytes());
        d[4..8].copy_from_slice(&(-2.0f32).to_le_bytes());
        d[8..12].copy_from_slice(&0.25f32.to_le_bytes());
        d[12..16].copy_from_slice(&0x1000u32.to_le_bytes());
        let m = Fake(vec![(0x1000, d)]);
        let class = |a: u32| (a == 0x1000).then(|| "Human".to_string());
        assert_eq!(show(Kind::Vec3, 0x1000, &m, &class), "1.5000,-2.0000,0.2500");
        assert_eq!(show(Kind::F32, 0x1004, &m, &class), "-2.00000");
        assert_eq!(show(Kind::U8, 0x100C, &m, &class), "0");
        assert_eq!(show(Kind::Hex, 0x100C, &m, &class), "0x1000");
        assert_eq!(show(Kind::PtrClass, 0x100C, &m, &class), "0x1000 Human");
        assert_eq!(show(Kind::Class, 0x1004, &m, &class), "?");
        assert_eq!(show(Kind::U32, 0x9000, &m, &class), "(not readable)");
    }

    #[test]
    fn pokes_and_dumps() {
        assert_eq!(poke_bytes("u8", &["255", "0x10"]), Ok(vec![255, 16]));
        assert_eq!(poke_bytes("u32", &["-1"]), Ok(vec![255; 4]));
        assert_eq!(poke_bytes("f32", &["1"]), Ok(1.0f32.to_le_bytes().to_vec()));
        assert_eq!(poke_bytes("bytes", &["90c3", "cc"]), Ok(vec![0x90, 0xc3, 0xcc]));
        assert!(poke_bytes("u8", &["256"]).is_err());
        assert!(poke_bytes("bytes", &["abc"]).is_err());
        assert!(poke_bytes("u64", &["1"]).is_err());
        assert!(poke_bytes("u8", &[]).is_err());
        let d = dump(0x400000, &(0..20).collect::<Vec<u8>>());
        assert_eq!(d.lines().count(), 2);
        assert!(d.starts_with("00400000  00 01 02"));
        assert!(d.lines().nth(1).unwrap().starts_with("00400010  10 11 12 13"));
    }

    #[test]
    fn bookmarks() {
        let t = set_mark("", "bureau", [46.39, 6.13, 4.14, 90.0]);
        let t = set_mark(&t, "roof", [1.0, 2.0, 3.0, 0.0]);
        let t = set_mark(&t, "bureau", [1.0, 1.0, 1.0, 45.0]);
        let m = marks(&format!("# comment\nbad line\n{t}"));
        assert_eq!(m.len(), 2);
        assert_eq!(m[0], ("bureau".to_string(), [1.0, 1.0, 1.0, 45.0]));
        assert_eq!(m[1].0, "roof");
    }
}
