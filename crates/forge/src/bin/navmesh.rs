//! navmesh <game dir>: parse every navigation mesh in the install (pass rate, pieces, triangles).
use anyhow::{Context, Result};
use forge::navmesh::{CLASS_NAV_MESH_MANAGER, parse_nav_mesh};

fn main() -> Result<()> {
    let dir = std::env::args().nth(1).context("game dir")?;
    let mut paths: Vec<_> = std::fs::read_dir(&dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "forge") && !p.to_string_lossy().contains("Sound"))
        .collect();
    paths.sort();
    let (mut ok, mut bad, mut pieces, mut tris) = (0, 0, 0, 0);
    for p in paths {
        let f = forge::Forge::open(&p)?;
        for e in &f.entries {
            let Ok(data) = f.read(e) else { continue };
            let Ok(objs) = forge::parse_objects(&data) else { continue };
            for o in objs.iter().filter(|o| o.class == CLASS_NAV_MESH_MANAGER) {
                match parse_nav_mesh(o.body) {
                    Ok(v) => {
                        ok += 1;
                        pieces += v.len();
                        tris += v.iter().map(|p| p.triangles.len()).sum::<usize>();
                    }
                    Err(err) => {
                        bad += 1;
                        if bad <= 15 {
                            eprintln!("{} / {} / {}: {err:#}", p.display(), e.name, o.name);
                        }
                    }
                }
            }
        }
    }
    println!("navigation meshes: {ok} parse, {bad} fail ({pieces} pieces, {tris} triangles)");
    Ok(())
}
