//! fxcheck <game dir> [forge name substring]: every `ParticleSystem` and `FX` object of the install's forges, parsed
//! with `forge::fx`; the pass rate per forge and the failures (name, error).
use std::collections::HashMap;

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let dir = std::path::Path::new(&a[1]);
    let only = a.get(2).cloned().unwrap_or_default();
    let mut names: Vec<_> = std::fs::read_dir(dir)?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "forge")).collect();
    names.sort();
    let (mut tot_ok, mut tot_n) = (0, 0);
    for p in names {
        let fname = p.file_name().unwrap().to_string_lossy().to_string();
        if !fname.contains(&only) {
            continue;
        }
        let Ok(f) = forge::Forge::open(&p) else { continue };
        let (mut ok, mut n, mut fx) = (0, 0, 0);
        let mut errs: HashMap<String, (usize, String)> = HashMap::new();
        for e in &f.entries {
            let Ok(d) = f.read(e) else { continue };
            let Ok(objs) = forge::parse_objects(&d) else { continue };
            for o in &objs {
                if o.class == forge::fx::CLASS_FX {
                    fx += 1;
                }
                if o.class != forge::fx::CLASS_PARTICLE_SYSTEM {
                    continue;
                }
                n += 1;
                match forge::fx::parse_particle_system(o.body) {
                    Ok(_) => ok += 1,
                    Err(err) => {
                        let k = format!("{err:#}");
                        let k = k.split(" at ").next().unwrap_or("").to_string();
                        let s = errs.entry(k).or_insert((0, format!("{} ({}): {err:#}", o.name, e.name)));
                        s.0 += 1;
                    }
                }
            }
        }
        println!("{fname}: particle systems {ok}/{n}, FX {fx}");
        for (k, (c, ex)) in errs {
            println!("   {c} x {k}: e.g. {ex}");
        }
        tot_ok += ok;
        tot_n += n;
    }
    println!("all: {tot_ok}/{tot_n}");
    Ok(())
}
