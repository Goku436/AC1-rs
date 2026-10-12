//! placed <forge> <name substring>: the entities named so (in the data files' object tables), where they stand (game
//! space, the translation of the 4x4 their body starts with) and the world box of the meshes they name that are in the
//! same data file (Bevy space as well, for the shot lists: x, z, -y).
fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let f = forge::Forge::open(&a[1])?;
    let pat = a[2].to_lowercase();
    for e in &f.entries {
        let Ok(d) = f.read(e) else { continue };
        let Ok(objs) = forge::parse_objects(&d) else { continue };
        let meshes: std::collections::HashMap<u32, &forge::Object> = objs.iter().filter(|o| o.class == forge::mesh::CLASS_MESH).map(|o| (o.id, o)).collect();
        for o in objs.iter().filter(|o| o.class == forge::datafile::CLASS_ENTITY && o.name.to_lowercase().contains(&pat) && o.body.len() >= 64) {
            let m: Vec<f32> = (0..16).map(|k| f32::from_le_bytes(o.body[k * 4..k * 4 + 4].try_into().unwrap())).collect();
            print!("{} in {}: ({:.1}, {:.1}, {:.1})", o.name, e.name, m[12], m[13], m[14]);
            for k in 0..o.body.len().saturating_sub(3) {
                let id = u32::from_le_bytes(o.body[k..k + 4].try_into()?);
                let Some(pm) = meshes.get(&id).and_then(|mo| forge::mesh::parse_mesh(mo.body).ok()) else { continue };
                let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                for p in &pm.positions {
                    let w = [0, 1, 2].map(|r| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r]);
                    for r in 0..3 {
                        lo[r] = lo[r].min(w[r]);
                        hi[r] = hi[r].max(w[r]);
                    }
                }
                let c = [0, 1, 2].map(|r| (lo[r] + hi[r]) / 2.0);
                print!(
                    "\n    mesh {:08x}: x {:.1}..{:.1} y {:.1}..{:.1} z {:.1}..{:.1}; centre Bevy {:.1},{:.1},{:.1}",
                    id, lo[0], hi[0], lo[1], hi[1], lo[2], hi[2], c[0], c[2], -c[1]
                );
            }
            println!();
        }
    }
    Ok(())
}
