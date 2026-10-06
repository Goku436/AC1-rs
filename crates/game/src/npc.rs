//! NPCs assembled the way AC1 does it (`EntityBuilder`, 971a842e, and `BuildTable`, 22ecbe63): a builder
//! names a base entity (universal legs, hands, shoes) and tables; a table is weighted rows, each row cells
//! that set what goes in a column (a mesh through its `LODSelector`, a texture, a nested table to pick from
//! too, an add-on skeleton such as a hood or a head's face rig, a height scale). Mission data blocks carry a
//! builder with everything it uses (`Damascus_MB02 - Side Quest_DataBlock`: peasants, women, harassers,
//! vigilantes, scholars, merchants, militia); the body skeleton comes from `DataPC_Common.forge`.
//!
//! Bytes (unaligned): a row is `[u32 id][u32 BuildRow class][u8][u32 table][f32 weight][u32 cells]`, then per
//! cell `[u32 column][u32 value class (GraphicObject, TextureBase, BuildTable, Skeleton, or 0)][u32 code]` and
//! a value whose size follows the code's third byte (0x1c a link, 4 bytes; 0x0d a colour, 16; 0x19 an enum,
//! 4; 0x07 an integer, 4; 0x0a or 0 a float, 4). Rows are found by their header; links outside every row
//! (a builder's own tables) always apply, and one row of each table is picked by weight.

use anyhow::{Context, Result};
use bevy::prelude::*;
use forge::datafile::{CLASS_ENTITY, DataFile};
use std::collections::HashSet;
use std::path::Path;

const CLASS_BUILD_ROW: u32 = 0x348b28d6;
const COMMON: &str = "DataPC_Common.forge";
const CLASS_BUILD_TABLE: u32 = 0x22ecbe63;
const CLASS_BUILDER: u32 = 0x971a842e;
const CLASS_LOD_SELECTOR: u32 = 0x51dc6b80;
const CLASS_MESH: u32 = 0x415d9568;
const CLASS_SKELETON: u32 = 0x24aecb7c;
/// Value classes of cells.
const GRAPHIC_OBJECT: u32 = 0xec6ac357;
const SKELETON_VALUE: u32 = 0x24aecb7c;
const BUILD_TABLE_VALUE: u32 = 0x22ecbe63;

/// A row: its weight, its links (value class, object id) and float values.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Row {
    pub weight: f32,
    pub links: Vec<(u32, u32)>,
    pub floats: Vec<f32>,
}

fn u32_at(b: &[u8], p: usize) -> Option<u32> {
    b.get(p..p + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap()))
}

/// The rows of a table or builder body, and where they span (start, end).
pub fn rows(b: &[u8]) -> (Vec<Row>, Vec<(usize, usize)>) {
    let mut out = vec![];
    let mut spans = vec![];
    let starts: Vec<usize> = (4..b.len().saturating_sub(3)).filter(|&k| u32_at(b, k) == Some(CLASS_BUILD_ROW)).map(|k| k - 4).collect();
    for (i, &s) in starts.iter().enumerate() {
        let limit = starts.get(i + 1).copied().unwrap_or(b.len());
        let mut p = s + 8 + 5;
        let (Some(w), Some(n)) = (u32_at(b, p).map(f32::from_bits), u32_at(b, p + 4)) else { continue };
        p += 8;
        let mut row = Row { weight: if w.is_finite() && w >= 0.0 { w } else { 0.0 }, ..default() };
        for _ in 0..n.min(64) {
            let (Some(_col), Some(class), Some(code)) = (u32_at(b, p), u32_at(b, p + 4), u32_at(b, p + 8)) else { break };
            let size = match (code >> 16) & 0xff {
                0x1c | 0x19 | 0x07 => 4,
                0x0a | 0x00 => 4,
                0x0d => 16,
                _ => break,
            };
            if p + 12 + size > limit {
                break;
            }
            match (code >> 16) & 0xff {
                0x1c => row.links.push((class, u32_at(b, p + 12).unwrap_or(0))),
                0x0a | 0x00 => row.floats.push(f32::from_bits(u32_at(b, p + 12).unwrap_or(0))),
                _ => {}
            }
            p += 12 + size;
        }
        spans.push((s, p.max(s + 8)));
        out.push(row);
    }
    (out, spans)
}

/// What an NPC is made of: skeletons and meshes as (forge, data file, name), and its height scale.
#[derive(Debug, Clone, Default)]
pub struct Recipe {
    pub skeletons: Vec<(String, String, String)>,
    pub meshes: Vec<(String, String, String)>,
    pub scale: f32,
    /// Textures put on parts by material name: (material, (forge, data file, texture)).
    pub textures: Vec<(String, (String, String, String))>,
}

/// A small deterministic generator (each NPC its own seed).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Assemble builder `name` of data file `datafile` in `forge_file` (body skeletons from the common forge),
/// its choices made with `seed`.
pub fn recipe(game_dir: &Path, forge_file: &str, datafile: &str, name: &str, seed: u64) -> Result<Recipe> {
    let f = forge::Forge::open(game_dir.join(forge_file))?;
    let e = f.entries.iter().find(|e| e.name == datafile).with_context(|| format!("no data file {datafile}"))?;
    let data = f.read(e)?;
    let df = DataFile::parse(&data)?;
    let builder = df.by_name(CLASS_BUILDER, name).with_context(|| format!("no builder {name}"))?;
    let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    let mut r = Recipe { scale: 1.0, ..default() };
    // Links into the common forge (universal hands, trousers, the body skeletons) resolve through its index.
    let index = forge::index::ForgeIndex::load(&game_dir.join(COMMON))?;
    let common = |d: &str, n: &str| (COMMON.to_string(), d.to_string(), n.to_string());
    // (An object's own data file, named after it, holds its mesh; others carry only copies of its selector.)
    let files: HashSet<&str> = index.objects.values().map(|p| p.datafile.as_str()).collect();
    let file_of = |p: &forge::index::Place| if files.contains(p.name.as_str()) { p.name.clone() } else { p.datafile.clone() };
    let here = |n: &str| (forge_file.to_string(), datafile.to_string(), n.to_string());
    let mut seen = HashSet::new();
    let mut add = |id: u32, r: &mut Recipe| {
        if !seen.insert(id) {
            return;
        }
        if let Some(o) = df.get(id) {
            // A LOD selector: its full-detail mesh (the one without `_LOD` in its name).
            let mesh = match o.class {
                CLASS_LOD_SELECTOR => df.refs(o).into_iter().map(|(_, m)| m).find(|m| m.class == CLASS_MESH && !m.name.contains("_LOD")),
                CLASS_MESH => Some(o),
                _ => None,
            };
            if let Some(m) = mesh.filter(|m| !m.name.contains("_LOD") && !r.meshes.iter().any(|x| x.2 == m.name)) {
                r.meshes.push(here(&m.name));
            }
            if o.class == CLASS_SKELETON {
                r.skeletons.push(here(&o.name));
            }
        } else if let Some(p) = index.objects.get(&id) {
            // (A common LOD selector's full-detail mesh has its name, in its data file.)
            match p.class {
                CLASS_LOD_SELECTOR | CLASS_MESH if !p.name.contains("_LOD") && !r.meshes.iter().any(|x| x.2 == p.name) => {
                    r.meshes.push(common(&file_of(p), &p.name))
                }
                CLASS_SKELETON => r.skeletons.push(common(&p.datafile, &p.name)),
                _ => {}
            }
        }
    };
    // The base entity's parts: every link in it to a mesh, LOD selector or skeleton, here or in the common
    // forge (legs, hands, shoes, the face's eyes and mouth, the body and skirt skeletons).
    if let Some(base) = df.refs(builder).into_iter().map(|(_, o)| o).find(|o| o.class == CLASS_ENTITY) {
        let b = base.body;
        for k in 0..b.len().saturating_sub(3) {
            let id = u32::from_le_bytes(b[k..k + 4].try_into().unwrap());
            if id != base.id && (df.get(id).is_some() || index.objects.contains_key(&id)) {
                add(id, &mut r);
            }
        }
    }
    // A builder's own look for a material (the militia's robe): the one material it names, with the one
    // texture set it names; the set's diffuse map goes on the parts of that material.
    let named = |class: u32| -> Vec<&forge::Object> {
        let mut v: Vec<&forge::Object> = df.refs(builder).into_iter().map(|(_, o)| o).filter(|o| o.class == class).collect();
        v.dedup_by_key(|o| o.id);
        v.sort_by_key(|o| o.id);
        v.dedup_by_key(|o| o.id);
        v
    };
    if let ([m], [set]) = (named(forge::datafile::CLASS_MATERIAL).as_slice(), named(forge::datafile::CLASS_TEXTURE_SET).as_slice()) {
        let tex = df
            .refs(set)
            .into_iter()
            .map(|(_, s)| s)
            .filter(|s| s.class == forge::datafile::CLASS_MAP_SPEC && s.name.contains("Diffuse"))
            .find_map(|s| df.refs(s).into_iter().map(|(_, t)| t).find(|t| t.class == forge::CLASS_TEXTURE));
        if let Some(t) = tex {
            r.textures.push((m.name.clone(), here(&t.name)));
        }
    }
    if !r.skeletons.iter().any(|s| s.2 == "Human_Body") {
        r.skeletons.insert(0, common("MMMA_FreeMission_Leader", "Human_Body"));
    }
    // Tables, depth first: links outside rows always, one row per table by weight.
    let mut stack: Vec<u32> = vec![builder.id];
    let mut visited = HashSet::new();
    while let Some(id) = stack.pop() {
        if !visited.insert(id) {
            continue;
        }
        let Some(o) = df.get(id) else { continue };
        let (rows, spans) = rows(o.body);
        let inside = |k: usize| spans.iter().any(|&(s, e)| (s..e).contains(&k));
        for (k, t) in df.refs(o) {
            if !inside(k) && t.class == CLASS_BUILD_TABLE && t.id != id {
                stack.push(t.id);
            }
        }
        if rows.is_empty() {
            continue;
        }
        let total: f32 = rows.iter().map(|r| r.weight).sum();
        let mut pick = rng.next() * total.max(1e-6);
        let row = rows.iter().find(|r| {
            pick -= r.weight;
            pick <= 0.0
        });
        let Some(row) = row.or(rows.last()) else { continue };
        for &(class, link) in &row.links {
            match class {
                GRAPHIC_OBJECT | SKELETON_VALUE => add(link, &mut r),
                BUILD_TABLE_VALUE => stack.push(link),
                _ => {}
            }
        }
        // A size table: its row is the height scale.
        if o.name.starts_with("Size_")
            && let Some(&s) = row.floats.first()
            && (0.8..1.2).contains(&s)
        {
            r.scale = s;
        }
    }
    info!("npc {name}: {} skeletons, {} meshes, scale {:.2}", r.skeletons.len(), r.meshes.len(), r.scale);
    debug!("npc {name}: skeletons {:?} meshes {:?}", r.skeletons.iter().map(|s| &s.2).collect::<Vec<_>>(), r.meshes.iter().map(|s| &s.2).collect::<Vec<_>>());
    Ok(r)
}

/// An NPC model from builder `name` (see `recipe`): its parts loaded, the builder's textures on, and its
/// height scale.
pub fn build(game_dir: &Path, forge_file: &str, datafile: &str, name: &str, seed: u64) -> Result<(crate::assets::CharacterData, f32)> {
    let r = recipe(game_dir, forge_file, datafile, name, seed)?;
    let mut g = crate::assets::load_assembled_with(game_dir, &r.skeletons, &r.meshes, false)?;
    for (material, (f, d, t)) in &r.textures {
        match crate::assets::load_texture(game_dir, f, d, t) {
            Ok((id, img)) => {
                for p in g.parts.iter_mut().filter(|p| p.material == *material) {
                    p.texture = Some(id);
                }
                g.textures.insert(id, img);
            }
            Err(e) => warn!("npc {name} texture {t}: {e:#}"),
        }
    }
    Ok((g, r.scale))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_rows_and_cells() {
        let mut b = vec![0u8; 6];
        for (w, link) in [(0.25f32, 0x1111u32), (0.75, 0x2222)] {
            b.extend(7u32.to_le_bytes());
            b.extend(CLASS_BUILD_ROW.to_le_bytes());
            b.push(1);
            b.extend(9u32.to_le_bytes());
            b.extend(w.to_le_bytes());
            b.extend(2u32.to_le_bytes());
            // A mesh link, then a float.
            b.extend(1u32.to_le_bytes());
            b.extend(GRAPHIC_OBJECT.to_le_bytes());
            b.extend(0x001c_0000u32.to_le_bytes());
            b.extend(link.to_le_bytes());
            b.extend(2u32.to_le_bytes());
            b.extend(0u32.to_le_bytes());
            b.extend(0x000a_0000u32.to_le_bytes());
            b.extend(1.02f32.to_le_bytes());
        }
        let (rows, spans) = rows(&b);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1], Row { weight: 0.75, links: vec![(GRAPHIC_OBJECT, 0x2222)], floats: vec![1.02] });
        assert_eq!(spans[0].1, spans[1].0);
    }
}
