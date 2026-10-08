//! entryhex <file.forge> <entry name> [bytes]: an entry's decompressed bytes as hex, with u32 and f32 readings.
use anyhow::{Context, Result};

fn main() -> Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let f = forge::Forge::open(a.get(1).context("forge file")?)?;
    let name = a.get(2).context("entry name")?;
    let e = f.entries.iter().find(|e| &e.name == name).with_context(|| format!("no entry {name}"))?;
    let d = f.read(e)?;
    let n = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(256).min(d.len());
    println!("{} bytes", d.len());
    for (row, c) in d[..n].chunks(16).enumerate() {
        let hex: Vec<String> = c.iter().map(|b| format!("{b:02x}")).collect();
        let words: Vec<String> = c.chunks(4).filter(|w| w.len() == 4).map(|w| format!("{:>10}", u32::from_le_bytes([w[0], w[1], w[2], w[3]]))).collect();
        let text: String = c.iter().map(|&b| if (32..127).contains(&b) { b as char } else { '.' }).collect();
        println!("{:06x}: {:<48} {} {text}", row * 16, hex.join(" "), words.join(" "));
    }
    Ok(())
}
