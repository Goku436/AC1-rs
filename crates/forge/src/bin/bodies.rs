//! bodies <forge file> [more forge files]: the rigid bodies of the entities in the forges (entity groups' members
//! too): how many on each collision layer, by shape class, whether they stop the player, and the entity names on each
//! layer but Static and Dynamic; then the meshes of the entities with no rigid body (they collide with nothing in AC1),
//! commonest first. `forge::shape::rigid_bodies` reads them.
use anyhow::{Context, Result};
use forge::datafile::CLASS_ENTITY;
use forge::shape::{LAYER_NAMES, rigid_bodies, stops_player};
use std::collections::{BTreeMap, HashMap};

const CLASS_ENTITY_GROUP: u32 = 0x3f742d26;

fn u32_at(b: &[u8], k: usize) -> u32 {
    b.get(k..k + 4).map_or(0, |s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// A name without its instance number (`House_3x6x4_01a_012` -> `House_3x6x4_01a`).
fn stem(n: &str) -> String {
    n.trim_end_matches(|c: char| c.is_ascii_digit() || c == '_').to_string()
}

fn main() -> Result<()> {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    anyhow::ensure!(!paths.is_empty(), "usage: bodies <forge file> [more forge files]");
    let mut datas = vec![];
    for p in &paths {
        let f = forge::Forge::open(p).with_context(|| p.clone())?;
        datas.extend(f.entries.iter().filter_map(|e| f.read(e).ok()));
    }
    let parsed: Vec<Vec<forge::Object>> = datas.iter().map(|d| forge::parse_objects(d).unwrap_or_default()).collect();
    let mut index: HashMap<u32, &forge::Object> = HashMap::new();
    for o in parsed.iter().flatten() {
        index.entry(o.id).or_insert(o);
    }
    // Every entity body, and every member of every group (`[u32 id][u32 Entity]` then its body).
    let mut bodies: Vec<(&str, &[u8])> = vec![];
    for o in parsed.iter().flatten() {
        if o.class == CLASS_ENTITY {
            bodies.push((&o.name, o.body));
        } else if o.class == CLASS_ENTITY_GROUP {
            let b = o.body;
            let starts: Vec<usize> = (4..b.len().saturating_sub(68)).filter(|&k| u32_at(b, k) == CLASS_ENTITY).collect();
            for (i, &k) in starts.iter().enumerate() {
                let end = starts.get(i + 1).map_or(b.len(), |&n| n - 4);
                bodies.push((&o.name, &b[k + 4..end]));
            }
        }
    }
    let mut by_layer: BTreeMap<(u8, u32), usize> = BTreeMap::new();
    let mut names: BTreeMap<(u8, String), usize> = BTreeMap::new();
    let mut no_body: HashMap<String, usize> = HashMap::new();
    let (mut with, mut without, mut unresolved) = (0, 0, 0);
    // Entities with several rigid bodies: their shapes' names, in order.
    let mut several: BTreeMap<String, usize> = BTreeMap::new();
    for (name, b) in bodies {
        let rbs = rigid_bodies(b);
        if rbs.len() > 1 {
            let list: Vec<String> = rbs.iter().map(|r| format!("{}:{}", index.get(&r.shape).map_or("?".into(), |o| stem(&o.name)), r.layer)).collect();
            *several.entry(format!("{} = {}", stem(name), list.join(" + "))).or_default() += 1;
        }
        for rb in &rbs {
            let class = index.get(&rb.shape).map(|o| o.class);
            unresolved += usize::from(class.is_none());
            *by_layer.entry((rb.layer, class.unwrap_or(0))).or_default() += 1;
            if rb.layer > 2 {
                *names.entry((rb.layer, stem(name))).or_default() += 1;
            }
        }
        let meshes: Vec<&str> = (0..b.len().saturating_sub(3))
            .filter_map(|k| index.get(&u32_at(b, k)).filter(|o| o.class == forge::mesh::CLASS_MESH))
            .map(|o| o.name.as_str())
            .filter(|n| !n.to_lowercase().contains("_lod"))
            .collect();
        if meshes.is_empty() {
            continue;
        }
        if rbs.is_empty() {
            without += 1;
            for m in meshes {
                *no_body.entry(stem(m)).or_default() += 1;
            }
        } else {
            with += 1;
        }
    }
    println!("rigid bodies by layer and shape class ({unresolved} shapes not in these forges):");
    for ((layer, class), n) in &by_layer {
        let name = LAYER_NAMES.get(*layer as usize).unwrap_or(&"?");
        let stops = if stops_player(*layer) { "stops the player" } else { "the player passes" };
        println!("{n:8} layer {layer:2} {name} ({stops}), shape class {class:08x}");
    }
    for ((layer, name), n) in &names {
        println!("layer {layer:2} {n:5} {name}");
    }
    println!("entities with several rigid bodies (shape:layer, in order):");
    for (k, n) in &several {
        println!("{n:6} {k}");
    }
    println!("entities with meshes: {with} with a rigid body, {without} without; their meshes:");
    let mut v: Vec<_> = no_body.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (n, c) in v.iter().take(120) {
        println!("{c:6} {n}");
    }
    Ok(())
}
