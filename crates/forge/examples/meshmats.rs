//! meshmats <forge> [more forges..] -- <mesh name substring>: the meshes named so, their submeshes' materials (name and
//! template name), and the entities that place them with their (placeholder, real) material overrides, by name. Ids are
//! looked up across every data file of the forges given.
use std::collections::HashMap;

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let split = a.iter().position(|s| s == "--").unwrap_or(a.len() - 1);
    let pat = a.last().map(|s| s.to_lowercase()).unwrap_or_default();
    let mut names: HashMap<u32, (String, u32)> = HashMap::new();
    let mut bodies: HashMap<u32, Vec<u8>> = HashMap::new();
    let mut meshes = vec![];
    let mut entities = vec![];
    for path in &a[1..split] {
        let f = forge::Forge::open(path)?;
        for e in &f.entries {
            let Ok(d) = f.read(e) else { continue };
            let Ok(objs) = forge::parse_objects(&d) else { continue };
            for o in &objs {
                names.insert(o.id, (o.name.clone(), o.class));
                if o.class == forge::datafile::CLASS_MATERIAL {
                    bodies.insert(o.id, o.body.to_vec());
                }
                if o.class == forge::mesh::CLASS_MESH && o.name.to_lowercase().contains(&pat) {
                    meshes.push((o.id, o.name.clone(), o.body.to_vec(), e.name.clone()));
                }
                if o.class == forge::datafile::CLASS_ENTITY {
                    entities.push((o.name.clone(), o.body.to_vec()));
                }
            }
        }
    }
    let name = |id: u32| names.get(&id).map_or(format!("{id:08x}?"), |n| n.0.clone());
    let template = |id: u32| bodies.get(&id).and_then(|b| forge::material::Material::parse(b).ok()).map_or("?".to_string(), |m| name(m.template));
    let ids: std::collections::HashSet<u32> = meshes.iter().map(|m| m.0).collect();
    for (id, n, body, file) in &meshes {
        let Ok(pm) = forge::mesh::parse_mesh(body) else { continue };
        println!("{n} ({id:08x}, {file}): {} verts", pm.positions.len());
        for (k, s) in pm.submeshes.iter().enumerate() {
            let m = pm.materials.get(k).copied().unwrap_or(0);
            println!("  sub {k}: {} verts, material {} [{}]", s.vertex_count, name(m), template(m));
        }
    }
    // Entities naming one of the meshes: their overrides (adjacent material ids after the mesh list).
    for (en, body) in &entities {
        let words: Vec<u32> = body.chunks_exact(1).collect::<Vec<_>>().windows(4).map(|w| u32::from_le_bytes([w[0][0], w[1][0], w[2][0], w[3][0]])).collect();
        if !words.iter().any(|w| ids.contains(w)) {
            continue;
        }
        let mats: Vec<String> = words
            .iter()
            .filter(|w| names.get(w).is_some_and(|n| n.1 == forge::datafile::CLASS_MATERIAL))
            .map(|&w| format!("{} [{}]", name(w), template(w)))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        println!("entity {en}: materials named {mats:?}");
    }
    Ok(())
}
