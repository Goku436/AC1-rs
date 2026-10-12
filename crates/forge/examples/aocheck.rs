//! aocheck <forge file>: every entity's baked occlusion (`forge::instance`): how many read, whether each stream has a
//! byte per vertex of its mesh (meshes found in the same forge), and how it runs with height (the top quarter of a
//! mesh's vertices against the bottom quarter): open above, closed in below if it is occlusion.
use anyhow::{Context, Result};
use std::collections::HashMap;

fn main() -> Result<()> {
    let path = std::env::args().nth(1).context("forge file")?;
    let f = forge::Forge::open(&path)?;
    let datas: Vec<Vec<u8>> = f.entries.iter().filter_map(|e| f.read(e).ok()).collect();
    let objects: Vec<Vec<forge::Object>> = datas.iter().map(|d| forge::parse_objects(d).unwrap_or_default()).collect();
    let meshes: HashMap<u32, &forge::Object> = objects.iter().flatten().filter(|o| o.class == forge::mesh::CLASS_MESH).map(|o| (o.id, o)).collect();
    let (mut ok, mut bad, mut streams, mut matched, mut mismatched, mut unknown) = (0, 0, 0, 0, 0, 0);
    let (mut up, mut down, mut sampled) = (0f64, 0f64, 0usize);
    let mut hist = [0usize; 8];
    for o in objects.iter().flatten().filter(|o| o.class == 0x0984_415e || o.class == 0x3f74_2d26) {
        match forge::instance::mesh_ao(o.body) {
            Ok(v) => {
                ok += 1;
                for m in v {
                    streams += 1;
                    for &a in &m.ao {
                        hist[a as usize / 32] += 1;
                    }
                    let Some(pm) = meshes.get(&m.mesh).and_then(|mo| forge::mesh::parse_mesh(mo.body).ok()) else {
                        unknown += 1;
                        continue;
                    };
                    if pm.positions.len() != m.ao.len() {
                        mismatched += 1;
                        continue;
                    }
                    matched += 1;
                    if pm.positions.len() >= 8 {
                        let mut by_z: Vec<(f32, u8)> = pm.positions.iter().zip(&m.ao).map(|(p, &a)| (p[2], a)).collect();
                        by_z.sort_by(|a, b| a.0.total_cmp(&b.0));
                        let q = by_z.len() / 4;
                        let mean = |s: &[(f32, u8)]| s.iter().map(|x| x.1 as f64).sum::<f64>() / s.len() as f64;
                        down += mean(&by_z[..q]);
                        up += mean(&by_z[by_z.len() - q..]);
                        sampled += 1;
                    }
                }
            }
            Err(e) => {
                bad += 1;
                if bad <= 5 {
                    println!("{}: {e:?}", o.name);
                }
            }
        }
    }
    println!("entities and groups read {ok}/{}, streams {streams}: {matched} a byte per vertex, {mismatched} not, {unknown} mesh elsewhere", ok + bad);
    if sampled > 0 {
        println!("mean byte, top quarter of a mesh {:.1}, bottom quarter {:.1} ({sampled} meshes)", up / sampled as f64, down / sampled as f64);
    }
    let total: usize = hist.iter().sum();
    println!("bytes by eighths: {:?}", hist.iter().map(|h| format!("{:.2}", *h as f64 / total.max(1) as f64)).collect::<Vec<_>>());
    Ok(())
}
