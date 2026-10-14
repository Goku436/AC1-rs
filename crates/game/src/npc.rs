//! NPCs assembled the way AC1 does it (`EntityBuilder`, 971a842e, and `BuildTable`, 22ecbe63): a builder
//! names a base entity (universal legs, hands, shoes) and tables; a table is weighted rows, each row cells
//! that set what goes in a column (a mesh through its `LODSelector`, a texture, a nested table to pick from
//! too, an add-on skeleton such as a hood or a head's face rig, a height scale). Mission data blocks carry a
//! builder with everything it uses (`Damascus_MB02 - Side Quest_DataBlock`: peasants, women, harassers,
//! vigilantes, scholars, merchants, militia); the body skeleton comes from `DataPC_Common.forge`.
//!
//! The tables' bytes are read by `forge::build`. Links outside every row (a builder's own tables) always apply, and one
//! row of each table is picked by weight; a picked row's cells whose column is a material property (a mask's colours,
//! a skin or hair colour, a texture set or map, the hidden flag) are AC1's per-person choices of look, applied to the
//! parts' materials as they load (`assets::MaterialChoices`).

use anyhow::{Context, Result};
use bevy::prelude::*;
use forge::build::{CLASS_BUILD_TABLE, CLASS_BUILDER, Value};
use forge::datafile::{CLASS_ENTITY, DataFile};
use std::collections::HashSet;
use std::path::Path;

const COMMON: &str = "DataPC_Common.forge";
const CLASS_LOD_SELECTOR: u32 = 0x51dc6b80;
const CLASS_MESH: u32 = 0x415d9568;
const CLASS_SKELETON: u32 = 0x24aecb7c;
/// Value classes of cells.
const GRAPHIC_OBJECT: u32 = 0xec6ac357;
const SKELETON_VALUE: u32 = 0x24aecb7c;
const BUILD_TABLE_VALUE: u32 = 0x22ecbe63;

/// What an NPC is made of: skeletons and meshes as (forge, data file, name), and its height scale.
#[derive(Debug, Clone, Default)]
pub struct Recipe {
    pub skeletons: Vec<(String, String, String)>,
    pub meshes: Vec<(String, String, String)>,
    pub scale: f32,
    /// Textures put on parts by material name: (material, (forge, data file, texture)).
    pub textures: Vec<(String, (String, String, String))>,
    /// The picked rows' choices for the parts' materials.
    pub choices: crate::assets::MaterialChoices,
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
    // (What the choices link to is in the builder's data file.)
    r.choices.files.push((forge_file.to_string(), datafile.to_string()));
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
        // (A LOD selector here whose meshes are not: a copy of a common one, as in the common forge's own builders, each
        // in a data file of its own; its meshes are found through the index below.)
        let local = df.get(id).filter(|o| o.class != CLASS_LOD_SELECTOR || df.refs(o).iter().any(|(_, m)| m.class == CLASS_MESH));
        if let Some(o) = local {
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
        let table = forge::build::parse(o.body, o.id);
        let inside = |k: usize| table.spans.iter().any(|&(s, e)| (s..e).contains(&k));
        for (k, t) in df.refs(o) {
            if !inside(k) && t.class == CLASS_BUILD_TABLE && t.id != id {
                stack.push(t.id);
            }
        }
        if table.rows.is_empty() {
            continue;
        }
        let total: f32 = table.rows.iter().map(|r| r.weight).sum();
        let mut pick = rng.next() * total.max(1e-6);
        let row = table.rows.iter().find(|r| {
            pick -= r.weight;
            pick <= 0.0
        });
        let Some(row) = row.or(table.rows.last()) else { continue };
        for cell in &row.cells {
            match cell.value {
                Value::Link(GRAPHIC_OBJECT | SKELETON_VALUE, link) => add(link, &mut r),
                Value::Link(BUILD_TABLE_VALUE, link) => stack.push(link),
                _ => {}
            }
            // A material's property: the person's look.
            if let Some(c) = table.column(cell.column)
                && (c.material != 0 || c.template != 0)
                && !c.path.is_empty()
            {
                r.choices.sets.push(crate::assets::Choice { material: c.material, template: c.template, path: c.path.clone(), value: cell.value });
            }
        }
        // A size table: its row is the height scale.
        if o.name.starts_with("Size_")
            && let Some(s) = row.cells.iter().find_map(|c| if let Value::Float(s) = c.value { Some(s) } else { None })
            && (0.8..1.2).contains(&s)
        {
            r.scale = s;
        }
    }
    info!("npc {name}: {} skeletons, {} meshes, scale {:.2}, {} material choices", r.skeletons.len(), r.meshes.len(), r.scale, r.choices.sets.len());
    debug!("npc {name}: skeletons {:?} meshes {:?}", r.skeletons.iter().map(|s| &s.2).collect::<Vec<_>>(), r.meshes.iter().map(|s| &s.2).collect::<Vec<_>>());
    Ok(r)
}

/// An NPC model from builder `name` (see `recipe`): its parts loaded, the builder's textures on, and its
/// height scale.
pub fn build(game_dir: &Path, forge_file: &str, datafile: &str, name: &str, seed: u64) -> Result<(crate::assets::CharacterData, f32)> {
    let r = recipe(game_dir, forge_file, datafile, name, seed)?;
    let mut g = crate::assets::load_assembled_with(game_dir, &r.skeletons, &r.meshes, false, &r.choices)?;
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

/// The common forge's builders by id (its index, cached).
fn common_builders(game_dir: &Path) -> Result<std::collections::HashMap<u32, String>> {
    static CACHE: std::sync::OnceLock<std::collections::HashMap<u32, String>> = std::sync::OnceLock::new();
    if let Some(c) = CACHE.get() {
        return Ok(c.clone());
    }
    let index = forge::index::ForgeIndex::load(&game_dir.join(COMMON))?;
    let m: std::collections::HashMap<u32, String> =
        index.objects.iter().filter(|(_, p)| p.class == CLASS_BUILDER).map(|(id, p)| (*id, p.name.clone())).collect();
    Ok(CACHE.get_or_init(|| m).clone())
}

/// The builder a crowd builder stands for: AC1's `MCxx_<class>` builders (each in its own data file of the common
/// forge) are a table of other builders (`MCxx_Poor`: half `MCMA_Poor`, half `MCFA_Poor`) and no base entity; one of
/// those is picked by weight with `seed`, and so on down. A builder with a base entity is itself.
pub fn concrete_builder(game_dir: &Path, name: &str, seed: u64) -> Result<String> {
    let builders = common_builders(game_dir)?;
    let f = forge::Forge::open(game_dir.join(COMMON))?;
    let mut rng = Rng(seed.wrapping_mul(0x2545_f491_4f6c_dd1d) | 1);
    let mut name = name.to_string();
    for _ in 0..4 {
        let Some(e) = f.entries.iter().find(|e| e.name == name) else { break };
        let data = f.read(e)?;
        let df = DataFile::parse(&data)?;
        let Some(b) = df.by_name(CLASS_BUILDER, &name) else { break };
        // (A builder with a base entity of its own is a person's: its rows pick their parts, not other builders.)
        if df.refs(b).iter().any(|(_, o)| o.class == CLASS_ENTITY) {
            break;
        }
        let table = forge::build::parse(b.body, b.id);
        let choices: Vec<(f32, &String)> = table
            .rows
            .iter()
            .filter_map(|r| r.cells.iter().find_map(|c| if let Value::Link(_, id) = c.value { builders.get(&id) } else { None }).map(|n| (r.weight, n)))
            .filter(|(_, n)| **n != name)
            .collect();
        let total: f32 = choices.iter().map(|c| c.0).sum();
        if choices.is_empty() || total <= 0.0 {
            break;
        }
        let mut pick = rng.next() * total;
        let next = choices.iter().find(|c| {
            pick -= c.0;
            pick <= 0.0
        });
        name = next.unwrap_or(&choices[choices.len() - 1]).1.clone();
    }
    Ok(name)
}

/// A crowd member from builder `name` of the common forge (`concrete_builder` first): its model, height scale and
/// the builder it came from.
pub fn build_common(game_dir: &Path, name: &str, seed: u64) -> Result<(crate::assets::CharacterData, f32, String)> {
    let concrete = concrete_builder(game_dir, name, seed)?;
    let (g, scale) = build(game_dir, COMMON, &concrete, &concrete, seed)?;
    Ok((g, scale, concrete))
}

/// The clip set a builder's people walk with, by AC1's naming (`MMMA` soldiers, `xCFx` women, other townsmen).
pub fn anim_set_of(builder: &str) -> &'static str {
    let b = builder.as_bytes();
    if b.get(1) == Some(&b'M') {
        "mmaa_anim_set"
    } else if b.get(2) == Some(&b'F') {
        "cfaa_anim_set"
    } else {
        "cmma_anim_set"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_sets_by_builder_name() {
        assert_eq!(anim_set_of("MMMA_Damascus_Militia"), "mmaa_anim_set");
        assert_eq!(anim_set_of("MCFA_Poor"), "cfaa_anim_set");
        assert_eq!(anim_set_of("MCMA_Rich"), "cmma_anim_set");
        assert_eq!(anim_set_of(""), "cmma_anim_set");
    }
}
