//! objects <forge file> <data file> [name substring] [hex [bytes]]: the objects in one data file (class,
//! id, size, name), with a class histogram; `hex` dumps the matching objects' bytes (with floats), `dump [dir]`
//! writes them to files (default `out/objects`, git-ignored).
use anyhow::{Context, Result};
use std::collections::BTreeMap;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let (path, name) = (args.get(1).context("forge file")?, args.get(2).context("data file")?);
    let pat = args.get(3).map(|s| s.to_lowercase());
    let f = forge::Forge::open(path)?;
    let e = f.entries.iter().find(|e| e.name == *name).with_context(|| format!("no data file {name}"))?;
    let data = f.read(e)?;
    let objs = forge::parse_objects(&data)?;
    let mut hist: BTreeMap<u32, (usize, usize)> = BTreeMap::new();
    for o in &objs {
        let h = hist.entry(o.class).or_default();
        h.0 += 1;
        h.1 += o.bytes.len();
        if pat.as_ref().is_none_or(|p| o.name.to_lowercase().contains(p)) {
            println!("{:08x} {:08x} {:>9} {}", o.class, o.id, o.bytes.len(), o.name);
            // `dump <dir>`: write each matching object's whole bytes to <dir>/<class>_<id>_<name>.bin.
            if args.get(4).is_some_and(|a| a == "dump") {
                let dir = std::path::Path::new(args.get(5).map_or("out/objects", |s| s.as_str()));
                std::fs::create_dir_all(dir)?;
                let safe: String = o.name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
                std::fs::write(dir.join(format!("{:08x}_{:08x}_{safe}.bin", o.class, o.id)), o.bytes)?;
            }
            if args.get(4).is_some_and(|a| a == "hex") {
                let n = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(256).min(o.bytes.len());
                for (row, chunk) in o.bytes[..n].chunks(16).enumerate() {
                    let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
                    let floats: Vec<String> =
                        chunk.chunks(4).filter(|c| c.len() == 4).map(|c| format!("{:9.3}", f32::from_le_bytes([c[0], c[1], c[2], c[3]]))).collect();
                    println!("  {:06x}: {:<48} {}", row * 16, hex.join(" "), floats.join(" "));
                }
            }
        }
    }
    println!("-- {} objects, {} bytes", objs.len(), data.len());
    for (c, (n, b)) in hist {
        println!("class {c:08x}: {n:6} objects {b:10} bytes");
    }
    Ok(())
}
