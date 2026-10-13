//! fxdump <game dir> <forge file> <name substring>: the particle systems of a forge whose names hold the substring,
//! every emitter's fields (`forge::fx::Emitter`) and where its texture is (looked up in that forge, `DataPC_Common` and
//! `DataPC` by their object indexes).
use anyhow::{Context, Result};

fn main() -> Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let dir = std::path::Path::new(a.get(1).context("game dir")?);
    let file = a.get(2).context("forge file")?;
    let pat = a.get(3).context("name substring")?.to_lowercase();
    let f = forge::Forge::open(dir.join(file))?;
    let idxs: Vec<(String, forge::index::ForgeIndex)> = [file.as_str(), "DataPC_Common.forge", "DataPC.forge"]
        .iter()
        .filter_map(|n| forge::index::ForgeIndex::load(&dir.join(n)).ok().map(|i| (n.to_string(), i)))
        .collect();
    let place = |id: u32| -> String {
        for (n, i) in &idxs {
            if let Some(p) = i.objects.get(&id) {
                return format!("{} ({:08x} in {n}: {})", p.name, p.class, p.datafile);
            }
        }
        if id == 0 { "-".into() } else { format!("{id:08x} not found") }
    };
    let mut seen = std::collections::HashSet::new();
    for e in &f.entries {
        let Ok(d) = f.read(e) else { continue };
        for o in forge::parse_objects(&d).unwrap_or_default() {
            if o.class != forge::fx::CLASS_PARTICLE_SYSTEM || !o.name.to_lowercase().contains(&pat) || !seen.insert(o.id) {
                continue;
            }
            println!("== {} ({:08x}, {})", o.name, o.id, e.name);
            match forge::fx::parse_particle_system(o.body) {
                Ok(ems) => {
                    for (k, em) in ems.iter().enumerate() {
                        println!("-- emitter {k}: texture {}, mesh {}", place(em.texture), place(em.mesh));
                        println!("{em:#?}");
                    }
                }
                Err(err) => println!("   {err:#}"),
            }
        }
    }
    Ok(())
}
