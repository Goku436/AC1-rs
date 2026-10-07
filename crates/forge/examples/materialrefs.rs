//! What the materials whose names contain a pattern reference: objects in the same data file (by class and name), and
//! the texture set -> map spec -> texture chain where it resolves.
//! Usage: materialrefs <game dir> <forge file> <data file> <name pattern>
use forge::Forge;
use forge::datafile::{CLASS_MATERIAL, DataFile};

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let forge = Forge::open(std::path::Path::new(&a[1]).join(&a[2]))?;
    let entry = forge.entries.iter().find(|e| e.name == a[3]).ok_or_else(|| anyhow::anyhow!("no data file {}", a[3]))?;
    let bytes = forge.read(entry)?;
    let df = DataFile::parse(&bytes)?;
    let pat = a[4].to_lowercase();
    for m in df.objects.iter().filter(|o| o.class == CLASS_MATERIAL && o.name.to_lowercase().contains(&pat)) {
        println!("{} ({:08x}), {} bytes", m.name, m.id, m.body.len());
        let words: Vec<String> = m.body.chunks(4).take(24).map(|c| c.iter().map(|b| format!("{b:02x}")).collect()).collect();
        println!("  head {}", words.join(" "));
        for (k, r) in df.refs(m) {
            println!("  @{k:#x} -> {} {:08x} class {:08x}", r.name, r.id, r.class);
            for (k2, r2) in df.refs(r) {
                println!("      @{k2:#x} -> {} {:08x} class {:08x}", r2.name, r2.id, r2.class);
            }
        }
    }
    Ok(())
}
