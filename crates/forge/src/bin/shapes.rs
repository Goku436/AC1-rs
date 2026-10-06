//! shapes <game dir>: parse every collision `MeshShape` in the install (pass rate, triangle count).
use anyhow::{Context, Result};
use forge::shape::{CLASS_BARREL_SHAPE, CLASS_BOX_SHAPE, CLASS_CAPSULE_SHAPE, CLASS_LIST_SHAPE, CLASS_MESH_SHAPE, parse_mesh_shape, parse_primitive};

fn main() -> Result<()> {
    let dir = std::env::args().nth(1).context("game dir")?;
    let mut paths: Vec<_> = std::fs::read_dir(&dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "forge") && !p.to_string_lossy().contains("Sound"))
        .collect();
    paths.sort();
    let (mut ok, mut bad, mut tris) = (0, 0, 0usize);
    let (mut pok, mut pbad) = (0, 0);
    for p in paths {
        let f = forge::Forge::open(&p)?;
        for e in &f.entries {
            let Ok(data) = f.read(e) else { continue };
            let Ok(objs) = forge::parse_objects(&data) else { continue };
            for o in objs.iter().filter(|o| [CLASS_BOX_SHAPE, CLASS_CAPSULE_SHAPE, CLASS_BARREL_SHAPE, CLASS_LIST_SHAPE].contains(&o.class)) {
                match parse_primitive(o.class, o.body) {
                    Ok(_) => pok += 1,
                    Err(err) => {
                        pbad += 1;
                        if pbad <= 10 {
                            eprintln!("{} / {} / {} {:08x}: {err:#}", p.display(), e.name, o.name, o.class);
                        }
                    }
                }
            }
            for o in objs.iter().filter(|o| o.class == CLASS_MESH_SHAPE) {
                match parse_mesh_shape(o.body) {
                    Ok(s) => {
                        ok += 1;
                        tris += s.triangles.len();
                    }
                    Err(err) => {
                        bad += 1;
                        if bad <= 20 {
                            eprintln!("{} / {} / {}: {err:#}", p.display(), e.name, o.name);
                        }
                    }
                }
            }
        }
    }
    println!("mesh shapes: {ok} parse, {bad} fail ({tris} triangles)");
    println!("primitive shapes (box, capsule, hull, list): {pok} parse, {pbad} fail");
    Ok(())
}
