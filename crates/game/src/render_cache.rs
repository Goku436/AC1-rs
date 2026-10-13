//! A city's drawn world saved on disk after its first load, beside its gameplay data (`level_cache`): what `city.rs`
//! finds to draw, before anything is made for the GPU. Its placed meshes (their bodies, as the game stores them, and the
//! skeletons skinned props stand in), each placement's levels of detail, cell, material slots and baked occlusion, the
//! grid's cells, the stand-ins' pieces (the blocks' merged meshes and each piece's triangles), and each material slot's
//! recipe (`city::SlotRecipe`) with the bodies of the textures they name. Finding it reads every data file of the city's
//! forge, the shared props' and DataPC's Game Fix and looks up every reference of every placed entity (Damascus: about
//! 12 s); reading it back takes a fraction of that.
//!
//! The textures, materials and stand-in meshes are made from it by the same code either way (`city::realize`): a load
//! from the cache and one from the game files draw the same city. Texture size and block compression are applied then,
//! so the detail setting (`AC1_DETAIL`, the graphics menu) does not change the file.
//!
//! Where it lives, how it is keyed (the code below in `SOURCES`, the city, `VARIANT_ENV`, the forges' sizes and
//! modification times), checked and written: as `level_cache` (`<exe dir>/ac1-cache/<city>-<variant>.draw`;
//! `AC1_NO_CACHE=1` neither reads nor writes it; `AC1_CACHE_CHECK=1` compares a fresh build with it byte for byte). A file
//! that does not match is not used: the city is read from the game's files and the file rewritten.
//!
//! Layout (little-endian): `b"AC1DRAWN"`, u32 format, u64 key, u64 payload length, u64 payload checksum, then the payload
//! (`write_draw`).

use crate::city::{Look, SlotRecipe, Water};
use crate::level_cache::{self as cache, CacheKind, R, W};
use crate::stream::{Cell, FakeJob, Instance, Lod, StoredMesh};
use anyhow::{Result, bail, ensure};
use bevy::prelude::*;
use std::path::Path;
use std::sync::Arc;

/// Bump when code outside `SOURCES` changes what is found to draw, or the payload's layout changes.
const VERSION: u32 = 1;

const MAGIC: &[u8; 8] = b"AC1DRAWN";

/// The code the drawn city comes from (a change to any of it changes the key).
const SOURCES: &[&str] = &[
    include_str!("city.rs"),
    include_str!("stream.rs"),
    include_str!("city_material.rs"),
    include_str!("render_cache.rs"),
    include_str!("level_cache.rs"),
    include_str!("../../forge/src/lib.rs"),
    include_str!("../../forge/src/read.rs"),
    include_str!("../../forge/src/datafile.rs"),
    include_str!("../../forge/src/index.rs"),
    include_str!("../../forge/src/mesh.rs"),
    include_str!("../../forge/src/lod.rs"),
    include_str!("../../forge/src/world.rs"),
    include_str!("../../forge/src/instance.rs"),
    include_str!("../../forge/src/material.rs"),
    include_str!("../../forge/src/skeleton.rs"),
    include_str!("../../forge/src/shape.rs"),
];

/// Env switches that change what is found to draw (a skinned prop's pose follows its collision, `AC1_COLLISION`;
/// `AC1_BIND_PROPS`; `AC1_PLAIN_CITY` drops the city shader).
const VARIANT_ENV: &[&str] = &["AC1_COLLISION", "AC1_GEOMETRY_HOLDS", "AC1_PAIRED_BEAMS", "AC1_NO_LIPS", "AC1_POSE_PROPS", "AC1_BIND_PROPS", "AC1_PLAIN_CITY"];

const KIND: CacheKind = CacheKind { ext: "draw", version: VERSION, sources: SOURCES, variant_env: VARIANT_ENV };

/// What a city draws, as found (`city::render_store`) or read back: see the module docs.
#[derive(Default)]
pub struct CityDraw {
    /// Each material slot's recipe, in slot order.
    pub recipes: Vec<SlotRecipe>,
    /// The textures the recipes name: id, the texture object's body.
    pub textures: Vec<(u32, Arc<[u8]>)>,
    pub meshes: Vec<StoredMesh>,
    pub instances: Vec<Instance>,
    pub cells: Vec<Cell>,
    /// The stand-ins' merged meshes (their bodies) and pieces.
    pub fake_blocks: Vec<Arc<[u8]>>,
    pub fake_jobs: Vec<FakeJob>,
    /// Every placement's baked occlusion, a byte per vertex.
    pub ao: Vec<u8>,
}

/// The city a forge holds (`DataPC_Damascus.forge` -> `damascus`).
fn city_of(forge_file: &str) -> String {
    forge_file.trim_start_matches("DataPC_").trim_end_matches(".forge").to_ascii_lowercase()
}

/// The drawn city from its cache file, if the file holds it as found from these game files with this code.
pub fn load(game_dir: &Path, city: &str) -> Option<CityDraw> {
    if !cache::enabled() || cache::check() {
        return None;
    }
    let (path, key) = cache::locate_kind(game_dir, city, &KIND)?;
    match cache::read_payload_as(MAGIC, &path, key).and_then(|p| read_draw(&p)) {
        Ok(d) => Some(d),
        Err(e) => {
            info!("render cache {}: not used ({e:#}); reading the city's world from the game files", path.display());
            None
        }
    }
}

/// Save the drawn city of `forge_file` for the next load (on a thread of its own); with `AC1_CACHE_CHECK=1`, compare it
/// with the file there first.
pub fn save(game_dir: &Path, forge_file: &str, draw: &CityDraw) {
    if !cache::enabled() {
        return;
    }
    let Some((path, key)) = cache::locate_kind(game_dir, &city_of(forge_file), &KIND) else { return };
    let payload = write_draw(draw);
    if cache::check() {
        match read_draw(&payload).map(|d| write_draw(&d)) {
            Ok(b) if b == payload => info!("render cache check: read back identical ({} bytes)", payload.len()),
            Ok(b) => warn!("render cache check: DIFFERENT read back ({} bytes against {})", b.len(), payload.len()),
            Err(e) => warn!("render cache check: DIFFERENT, not read back: {e:#}"),
        }
        match cache::read_payload_as(MAGIC, &path, key) {
            Ok(old) if old == payload => info!("render cache check: the cache file is identical to the fresh build ({} bytes)", payload.len()),
            Ok(old) => {
                let at = old.iter().zip(&payload).position(|(a, b)| a != b).unwrap_or(old.len().min(payload.len()));
                warn!("render cache check: the cache file is DIFFERENT from the fresh build from byte {at} ({} bytes against {})", old.len(), payload.len());
            }
            Err(e) => info!("render cache check: no cache file to compare ({e:#})"),
        }
    }
    cache::write_in_background(MAGIC, path, key, payload, "render cache".into());
}

fn write_mat4(w: &mut W, m: &Mat4) {
    m.to_cols_array().iter().for_each(|v| w.f32(*v));
}

fn read_mat4(r: &mut R) -> Result<Mat4> {
    let mut a = [0f32; 16];
    for v in &mut a {
        *v = r.f32_raw()?;
    }
    Ok(Mat4::from_cols_array(&a))
}

fn write_params(w: &mut W, p: &crate::city_material::CityParams) {
    // (Every field named: one added to the shader's values stops the build here until it is saved too.)
    let crate::city_material::CityParams { mode, tint, color1, color2, tile0, tile1, tile2, wave1a, wave1b, wave2a, wave2b, water, water_color, wet } = p;
    for v in [mode, tint, color1, color2, tile0, tile1, tile2, wave1a, wave1b, wave2a, wave2b, water, water_color, wet] {
        v.to_array().iter().for_each(|x| w.f32(*x));
    }
}

fn read_params(r: &mut R) -> Result<crate::city_material::CityParams> {
    let mut v = || -> Result<Vec4> { Ok(Vec4::new(r.f32_raw()?, r.f32_raw()?, r.f32_raw()?, r.f32_raw()?)) };
    Ok(crate::city_material::CityParams {
        mode: v()?,
        tint: v()?,
        color1: v()?,
        color2: v()?,
        tile0: v()?,
        tile1: v()?,
        tile2: v()?,
        wave1a: v()?,
        wave1b: v()?,
        wave2a: v()?,
        wave2b: v()?,
        water: v()?,
        water_color: v()?,
        wet: v()?,
    })
}

/// The drawn city as the cache's payload.
pub fn write_draw(d: &CityDraw) -> Vec<u8> {
    let CityDraw { recipes, textures, meshes, instances, cells, fake_blocks, fake_jobs, ao } = d;
    let size = textures.iter().map(|t| t.1.len()).sum::<usize>() + meshes.iter().map(|m| m.body.len()).sum::<usize>() + ao.len();
    let mut w = W(Vec::with_capacity(size + (16 << 20)));
    w.vec(recipes, |w, r| {
        let SlotRecipe { material, tex, nrm, look, water, params, maps, blend } = r;
        w.u32(*material);
        w.opt(tex.as_ref(), |w, t| w.u32(*t));
        w.opt(nrm.as_ref(), |w, t| w.u32(*t));
        w.opt(look.alpha.as_ref(), |w, a| w.u8(*a));
        w.bool(look.two_sided);
        w.u8(match water {
            None => 0,
            Some(Water::Open) => 1,
            Some(Water::Puddle) => 2,
        });
        write_params(w, params);
        w.opt(maps.as_ref(), |w, m| m.iter().for_each(|t| w.opt(t.as_ref(), |w, t| w.u32(*t))));
        w.bool(*blend);
    });
    w.vec(textures, |w, (id, body)| {
        w.u32(*id);
        w.bytes(body);
    });
    w.vec(meshes, |w, m| {
        w.bytes(&m.body);
        w.len(m.verts);
        w.u32s(&m.materials);
        w.opt(m.skeleton.as_ref(), |w, s| w.bytes(s));
    });
    w.vec(instances, |w, i| {
        let Instance { world, lo, hi, levels, parts, ao, shadow } = i;
        write_mat4(w, world);
        w.v3(*lo);
        w.v3(*hi);
        w.vec(levels, |w, l| {
            w.opt(l.mesh.as_ref(), |w, m| w.len(*m));
            w.f32(l.until);
        });
        w.vec(parts, |w, p| w.u32s(p));
        w.vec(ao, |w, a| w.opt(a.as_ref(), |w, a| w.u32(*a)));
        w.bool(*shadow);
    });
    w.vec(cells, |w, c| {
        w.f32(c.min.x);
        w.f32(c.min.y);
        w.f32(c.max.x);
        w.f32(c.max.y);
        w.vec(&c.instances, |w, i| w.len(*i));
    });
    w.vec(fake_blocks, |w, b| w.bytes(b));
    w.vec(fake_jobs, |w, j| {
        let FakeJob { block, idx, material, world, cell } = j;
        w.len(*block);
        w.u32s(idx);
        w.u32(*material);
        write_mat4(w, world);
        w.opt(cell.as_ref(), |w, c| w.len(*c));
    });
    w.bytes(ao);
    w.0
}

/// The drawn city a payload holds (`write_draw`), checked: every index within what it points into, every texture a
/// recipe names there.
pub fn read_draw(d: &[u8]) -> Result<CityDraw> {
    let mut r = R { d, p: 0 };
    let recipes = r.vec(16, |r| {
        let material = r.u32()?;
        let tex = r.opt(R::u32)?;
        let nrm = r.opt(R::u32)?;
        let look = Look { alpha: r.opt(R::u8)?, two_sided: r.bool()? };
        let at = r.p;
        let water = match r.u8()? {
            0 => None,
            1 => Some(Water::Open),
            2 => Some(Water::Puddle),
            v => bail!("render cache water kind {v} at {at:#x}"),
        };
        let params = read_params(r)?;
        let maps = r.opt(|r| {
            let mut m = [None; 7];
            for t in &mut m {
                *t = r.opt(R::u32)?;
            }
            Ok(m)
        })?;
        Ok(SlotRecipe { material, tex, nrm, look, water, params, maps, blend: r.bool()? })
    })?;
    let textures = r.vec(16, |r| Ok((r.u32()?, Arc::from(r.bytes()?))))?;
    let meshes = r.vec(25, |r| {
        let body: Arc<[u8]> = Arc::from(r.bytes()?);
        let verts = r.u64()? as usize;
        let materials = r.u32s()?;
        let skeleton: Option<Arc<[u8]>> = r.opt(|r| Ok(Arc::from(r.bytes()?)))?;
        let pose = skeleton.as_ref().and_then(|s| forge::skeleton::parse_skeleton(s).ok()).map(Arc::new);
        Ok(StoredMesh { body, verts, materials, pose, skeleton })
    })?;
    let instances = r.vec(122, |r| {
        let world = read_mat4(r)?;
        let lo = Vec3::new(r.f32_raw()?, r.f32_raw()?, r.f32_raw()?);
        let hi = Vec3::new(r.f32_raw()?, r.f32_raw()?, r.f32_raw()?);
        let levels = r.vec(5, |r| {
            let mesh = r.opt(|r| r.index(meshes.len()))?;
            Ok(Lod { mesh, until: r.f32_raw()? })
        })?;
        let parts = r.vec(8, R::u32s)?;
        let ao = r.vec(1, |r| r.opt(R::u32))?;
        Ok(Instance { world, lo, hi, levels, parts, ao, shadow: r.bool()? })
    })?;
    let cells = r.vec(24, |r| {
        let min = Vec2::new(r.f32_raw()?, r.f32_raw()?);
        let max = Vec2::new(r.f32_raw()?, r.f32_raw()?);
        Ok(Cell { min, max, instances: r.vec(8, |r| r.index(instances.len()))? })
    })?;
    let fake_blocks = r.vec(8, |r| Ok(Arc::from(r.bytes()?)))?;
    let fake_jobs = r.vec(93, |r| {
        let block = r.index(fake_blocks.len())?;
        let idx = r.u32s()?;
        let material = r.u32()?;
        let world = read_mat4(r)?;
        Ok(FakeJob { block, idx, material, world, cell: r.opt(|r| r.index(cells.len()))? })
    })?;
    let ao = r.bytes()?.to_vec();
    ensure!(r.p == d.len(), "render cache has {} bytes past its end at {:#x}", d.len() - r.p, r.p);
    // (What the indices point into.)
    let ids: std::collections::HashSet<u32> = textures.iter().map(|t| t.0).collect();
    let named = |t: &Option<u32>| t.is_none_or(|t| ids.contains(&t));
    ensure!(
        recipes.iter().all(|r| named(&r.tex) && named(&r.nrm) && r.maps.is_none_or(|m| m.iter().all(named))),
        "render cache: a material names a texture it does not hold"
    );
    let slot = |s: &u32| (*s as usize) < recipes.len();
    for (k, i) in instances.iter().enumerate() {
        ensure!(i.parts.len() == i.levels.len() && i.ao.len() == i.levels.len(), "render cache: instance {k}'s levels, parts and occlusion differ in number");
        ensure!(i.parts.iter().flatten().all(slot), "render cache: instance {k} has a material slot past {}", recipes.len());
        ensure!(i.ao.iter().flatten().all(|a| *a as usize <= ao.len()), "render cache: instance {k}'s occlusion starts past its bytes");
    }
    ensure!(fake_jobs.iter().all(|j| slot(&j.material)), "render cache: a stand-in piece has a material slot past {}", recipes.len());
    Ok(CityDraw { recipes, textures, meshes, instances, cells, fake_blocks, fake_jobs, ao })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> CityDraw {
        let recipe = |material: u32, tex: Option<u32>| SlotRecipe {
            material,
            tex,
            nrm: None,
            look: Look { alpha: Some(102), two_sided: true },
            water: Some(Water::Puddle),
            params: crate::city_material::CityParams::default(),
            maps: Some([tex, None, None, None, None, None, None]),
            blend: false,
        };
        let mesh = |b: &[u8]| StoredMesh { body: Arc::from(b), verts: 3, materials: vec![7, 8], pose: None, skeleton: None };
        let lod = |mesh: Option<usize>, until: f32| Lod { mesh, until };
        CityDraw {
            recipes: vec![recipe(1, Some(10)), recipe(2, None)],
            textures: vec![(10, Arc::from(&[1u8, 2, 3][..]))],
            meshes: vec![mesh(&[4, 5, 6]), mesh(&[7])],
            instances: vec![Instance {
                world: Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0)),
                lo: Vec3::splat(f32::MAX),
                hi: Vec3::splat(f32::MIN),
                levels: vec![lod(Some(0), 20.0), lod(None, 50.0), lod(Some(1), f32::INFINITY)],
                parts: vec![vec![0, 1], vec![], vec![1]],
                ao: vec![Some(0), None, Some(2)],
                shadow: true,
            }],
            cells: vec![Cell { min: Vec2::splat(f32::MIN), max: Vec2::splat(f32::MAX), instances: vec![0] }],
            fake_blocks: vec![Arc::from(&[9u8; 5][..])],
            fake_jobs: vec![FakeJob { block: 0, idx: vec![0, 1, 2], material: 1, world: Mat4::IDENTITY, cell: Some(0) }],
            ao: vec![1, 2, 3],
        }
    }

    #[test]
    fn round_trip_is_identical() {
        let bytes = write_draw(&sample());
        let back = read_draw(&bytes).expect("reads back");
        assert_eq!(write_draw(&back), bytes);
        assert_eq!(back.recipes, sample().recipes);
        assert_eq!(back.fake_jobs, sample().fake_jobs);
        assert!(back.instances[0].levels[2].until.is_infinite());
    }

    #[test]
    fn short_or_corrupt_payloads_are_errors() {
        let bytes = write_draw(&sample());
        for cut in [0, 1, 7, 8, bytes.len() / 2, bytes.len() - 1] {
            assert!(read_draw(&bytes[..cut]).is_err(), "cut at {cut}");
        }
        let mut long = bytes.clone();
        long.push(0);
        assert!(read_draw(&long).is_err());
        // A material naming a texture the file lacks; an instance's mesh past the list.
        let mut d = sample();
        d.recipes[1].nrm = Some(99);
        assert!(read_draw(&write_draw(&d)).is_err());
        let mut d = sample();
        d.instances[0].levels[0].mesh = Some(5);
        assert!(read_draw(&write_draw(&d)).is_err());
        // Every single flipped byte reads as an error or as some city; never a panic.
        for i in 0..bytes.len() {
            let mut b = bytes.clone();
            b[i] ^= 0xa5;
            let _ = read_draw(&b);
        }
    }
}
