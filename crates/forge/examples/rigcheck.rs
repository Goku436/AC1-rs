//! rigcheck <datafile.bin>: do mesh bone hashes resolve against the listed skeletons?
use std::collections::{HashMap, HashSet};
fn main() -> anyhow::Result<()> {
    let d = std::fs::read(std::env::args().nth(1).unwrap())?;
    let objs = forge::parse_objects(&d)?;
    let skel_ids = [0x029323a8u32, 0x0325daa6, 0x29f7df88, 0x1aee6696, 0x3d1a5c51];
    let mut hashes: HashMap<u32, String> = HashMap::new();
    for o in objs.iter().filter(|o| skel_ids.contains(&o.id)) {
        let s = forge::skeleton::parse_skeleton(o.body)?;
        let root = s.bones.iter().find(|b| b.parent.is_none()).unwrap();
        println!("skeleton {} {} bones, root {:08x} {:?}", o.name, s.bones.len(), root.name_hash, root.name);
        let first_children: Vec<_> = s
            .bones
            .iter()
            .filter(|b| b.parent == Some(0))
            .map(|b| format!("{:08x}{}", b.name_hash, if hashes.contains_key(&b.name_hash) { "*" } else { "" }))
            .collect();
        println!("   root children {first_children:?}");
        for b in &s.bones {
            hashes.entry(b.name_hash).or_insert(o.name.clone());
        }
    }
    for o in objs.iter().filter(|o| o.class == forge::mesh::CLASS_MESH) {
        let Ok(m) = forge::mesh::parse_mesh(o.body) else { continue };
        let missing: HashSet<_> = m.bones.iter().filter(|b| !hashes.contains_key(&b.name_hash)).map(|b| format!("{:08x}", b.name_hash)).collect();
        let used: HashSet<&String> = m.bones.iter().filter_map(|b| hashes.get(&b.name_hash)).collect();
        println!("{:40} bones {:3} missing {:?} from {:?}", o.name, m.bones.len(), missing, used);
    }
    Ok(())
}
