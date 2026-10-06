//! world <forge file>: survey the entities of a city forge: their transforms (the column-major 4x4 at
//! the start of the body) and the meshes they reference anywhere in the forge.
use anyhow::Result;
use forge::datafile::CLASS_ENTITY;
use forge::mesh::CLASS_MESH;
use std::collections::{HashMap, HashSet};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let f = forge::Forge::open(&args[1])?;
    let t0 = std::time::Instant::now();
    let datas: Vec<(String, Vec<u8>)> = f.entries.iter().filter_map(|e| f.read(e).ok().map(|d| (e.name.clone(), d))).collect();
    // Every object's class and name by id, across the forge.
    let mut index: HashMap<u32, (u32, String, usize)> = HashMap::new();
    let parsed: Vec<Vec<forge::Object>> = datas.iter().map(|(_, d)| forge::parse_objects(d).unwrap_or_default()).collect();
    for (fi, objs) in parsed.iter().enumerate() {
        for o in objs {
            index.entry(o.id).or_insert((o.class, o.name.to_string(), fi));
        }
    }
    let (mut ents, mut local, mut remote) = (0, 0, 0);
    let mut meshes = HashSet::new();
    let mut by_name: HashMap<String, usize> = HashMap::new();
    for (fi, objs) in parsed.iter().enumerate() {
        for o in objs.iter().filter(|o| o.class == CLASS_ENTITY) {
            ents += 1;
            let b = o.body;
            let refs: Vec<(u32, usize)> = (0..b.len().saturating_sub(3))
                .map(|k| u32::from_le_bytes([b[k], b[k + 1], b[k + 2], b[k + 3]]))
                .filter_map(|id| index.get(&id).filter(|r| r.0 == CLASS_MESH).map(|r| (id, r.2)))
                .collect();
            if refs.is_empty() {
                continue;
            }
            if refs.iter().all(|r| r.1 == fi) {
                local += 1;
            } else {
                remote += 1;
            }
            for (id, _) in refs {
                meshes.insert(id);
            }
            let stem: String = o.name.trim_end_matches(|c: char| c.is_ascii_digit() || c == '_').to_string();
            *by_name.entry(stem).or_default() += 1;
        }
    }
    println!(
        "{} data files, {ents} entities: {local} with meshes in their file, {remote} elsewhere; {} unique meshes; {:.1}s",
        datas.len(),
        meshes.len(),
        t0.elapsed().as_secs_f32()
    );
    let mut names: Vec<_> = by_name.into_iter().collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.1));
    if args.get(2).is_some_and(|a| a == "all") {
        for (n, c) in &names {
            println!("{c:6} {n}");
        }
    } else {
        println!("most placed: {:?}", &names[..names.len().min(40)]);
    }
    Ok(())
}
