//! idx <forge file> <hex id>...: where objects are (builds or reads the cached index).
use anyhow::{Context, Result};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let t = std::time::Instant::now();
    let idx = forge::index::ForgeIndex::load(std::path::Path::new(args.get(1).context("forge file")?))?;
    println!("{} objects ({:.1}s)", idx.objects.len(), t.elapsed().as_secs_f32());
    for h in &args[2..] {
        let id = u32::from_str_radix(h.trim_start_matches("0x"), 16)?;
        match idx.objects.get(&id) {
            Some(p) => println!("{id:08x}: {:08x} {} in {}", p.class, p.name, p.datafile),
            None => println!("{id:08x}: not here"),
        }
    }
    Ok(())
}
