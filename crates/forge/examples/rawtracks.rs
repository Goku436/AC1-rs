//! rawtracks <game dir> <type hex> [max clips]: every track of that type (the type byte without its low two bits) in
//! every archive's clips, one line per track: clip name, bone hash, key frames, then each value as hex. For working out
//! codecs `anim.rs` does not decode yet.
use anyhow::{Context, Result};

fn u32_at(b: &[u8], p: usize) -> u32 {
    u32::from_le_bytes(b[p..p + 4].try_into().unwrap())
}

fn main() -> Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let dir = a.get(1).context("game dir")?;
    let want = u8::from_str_radix(a.get(2).context("type")?.trim_start_matches("0x"), 16)?;
    let max: usize = a.get(3).and_then(|m| m.parse().ok()).unwrap_or(usize::MAX);
    let mut shown = 0;
    let forges =
        std::fs::read_dir(dir)?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "forge") && !p.to_string_lossy().contains("Sound"));
    for path in forges {
        let f = forge::Forge::open(&path)?;
        for e in &f.entries {
            let Ok(d) = f.read(e) else { continue };
            let Ok(objs) = forge::parse_objects(&d) else { continue };
            for o in objs.iter().filter(|o| o.class == forge::anim::CLASS_ANIMATION) {
                if shown >= max {
                    return Ok(());
                }
                let obj = o.bytes;
                let name_len = u32_at(obj, 8) as usize;
                let class = forge::anim::CLASS_ANIMATION.to_le_bytes();
                let after = 12 + name_len;
                let Some(rel) = obj[after..].windows(4).position(|w| w == class) else { continue };
                let body = &obj[after + rel + 4..];
                let tag = 0x653caa76u32.to_le_bytes();
                let Some(first) = body.windows(4).position(|w| w == tag) else { continue };
                let count = u32_at(body, first - 8) as usize;
                let hashes: Vec<u32> = (0..count).map(|k| u32_at(body, first + 12 * k + 4)).collect();
                let mut p = first + 12 * (count - 1) + 8 + 2 + 4;
                let size = |k: u8| -> Option<usize> {
                    Some(match k {
                        0x04 => 2,
                        0x08 => 3,
                        0x0c | 0x20 | 0x28 => 4,
                        0x10 | 0x24 => 6,
                        0x14 => 8,
                        0x18 | 0x1c => 12,
                        0x2c | 0x34 | 0x38 => 1,
                        _ => return None,
                    })
                };
                let mut hit = false;
                for &h in &hashes {
                    let t = body[p];
                    let kind = t & !3;
                    let n = u32_at(body, p + 5) as usize;
                    let Some(vs) = size(kind) else { break };
                    let mut q = p + 9;
                    let mut keys = vec![0u16];
                    for _ in 1..n {
                        if t & 1 == 1 {
                            keys.push(u16::from_le_bytes([body[q], body[q + 1]]));
                            q += 2;
                        } else {
                            keys.push(body[q] as u16);
                            q += 1;
                        }
                    }
                    if kind == want {
                        let vals: Vec<String> = body[q..q + n * vs].chunks_exact(vs).map(|c| c.iter().rev().map(|b| format!("{b:02x}")).collect()).collect();
                        println!("{}\t{h:08x}\t{t:02x}\t{:?}\t{}", o.name, keys, vals.join(" "));
                        hit = true;
                    }
                    p = q + n * vs;
                }
                if hit {
                    shown += 1;
                }
            }
        }
    }
    Ok(())
}
