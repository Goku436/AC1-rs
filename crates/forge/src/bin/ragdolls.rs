//! ragdolls <game dir> [name]: read every `RagdollNew`'s Havok ragdoll (pass rate), and print the first
//! one named `name` (default `human_ragdoll`): bodies, joints, bones.
use anyhow::{Context, Result};
use forge::hkx::{Packfile, ragdoll};

const CLASS_RAGDOLL: u32 = 0xc8dbde7f;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let dir = args.get(1).context("game dir")?;
    let want = args.get(2).map_or("human_ragdoll", |s| s.as_str());
    let mut paths: Vec<_> = std::fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "forge") && !p.to_string_lossy().contains("Sound"))
        .collect();
    paths.sort();
    let (mut ok, mut bad, mut shown) = (0, 0, false);
    for p in paths {
        let f = forge::Forge::open(&p)?;
        for e in &f.entries {
            let Ok(data) = f.read(e) else { continue };
            let Ok(objs) = forge::parse_objects(&data) else { continue };
            for o in objs.iter().filter(|o| o.class == CLASS_RAGDOLL) {
                match Packfile::find(o.body).and_then(|pf| ragdoll(&pf)) {
                    Ok(r) => {
                        ok += 1;
                        if !shown && o.name == want {
                            shown = true;
                            println!(
                                "{} ({} / {}): {} bodies, {} joints, {} bones",
                                o.name,
                                p.display(),
                                e.name,
                                r.bodies.len(),
                                r.joints.len(),
                                r.bones.len()
                            );
                            for (i, b) in r.bodies.iter().enumerate() {
                                println!("  body {i} {}: {:?} at {:?}, mass {:.1}", b.name, b.shape, b.translation, b.mass);
                            }
                            for j in &r.joints {
                                println!("  joint {} - {}: {:?}, pivot in a {:?}", r.bodies[j.a].name, r.bodies[j.b].name, j.limit, j.frame_a.1);
                            }
                            for (n, b) in &r.bones {
                                println!("  bone {n} -> {b:?}");
                            }
                        }
                    }
                    Err(err) => {
                        bad += 1;
                        eprintln!("{} / {} / {}: {err:#}", p.display(), e.name, o.name);
                    }
                }
            }
        }
    }
    println!("ragdolls: {ok} read, {bad} fail");
    Ok(())
}
