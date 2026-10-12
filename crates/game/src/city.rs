//! A city from the game data (`AC1_LEVEL=<city>`: `DataPC_<City>.forge`, e.g. Masyaf or Damascus).
//!
//! Entity groups (class 3f742d26: souks, balconies, scaffolds, bridges, market stalls, rooftop
//! gardens...) start with the group's world transform and embed their member entities inline, each
//! `[u32 id][u32 entity class]` then the entity body (its own world transform first, then its
//! references) up to the next; those members are not in the data file's object table. Damascus
//! places a few thousand entities this way.
//!
//! AC1 places a city with entities spread over its cell data files (`Cell*_DataBlock`): an entity's
//! body starts with its world transform, a column-major 4x4 (Z up, metres), and refers by id to the
//! meshes it shows (the full detail one and `_LOD_*` versions), often stored in another data file of
//! the forge, and to `(placeholder, real)` material pairs. So the whole forge is read and indexed by
//! object id; every entity placing static meshes shows its full-detail ones (vegetation clutter and
//! skinned meshes, such as dead bodies, are left out), with diffuse textures found through
//! material -> texture set -> diffuse map spec -> texture across data files. Collision is AC1's: each entity's rigid
//! bodies, by their collision layer (see the collision part of `spawn_world`, docs/NOTES.md "What collides"); entities
//! with none collide with nothing but terrain.
//!
//! Props every city shares (bushes, palms, souk windows, beams, lanterns, ladders, hiding spots,
//! columns) live in `DataPC_Common.forge`, referred to by id from the city's entities: the data files
//! holding the ones the city uses, and their materials and textures, are read from it too (Damascus:
//! 251 of them, 240 meshes, placing 17474 meshes instead of 7362).
//!
//! Holds: AC1's own authored guidance is read where the city has it (`Level::use_authored`); without it, they are found
//! in the geometry of buildings
//! and walls (not terrain or rocks): every horizontal edge (30 cm or longer) where a walkable top
//! (normal up) meets a wall below it (normal sideways), convex, becomes a ledge facing out of the
//! wall: roof edges, wall tops, sills, balconies and beam ends. The climbing moves that grab one
//! hold line (jump-grabs, wall runs, catches, shimmying, top-outs, free hangs) work on them; climbing
//! a wall hand over hand needs holds every 0.6 m, which real walls only sometimes have.

use crate::level::{Ledge, Level, Tri};
use anyhow::{Context, Result};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use forge::Object;
use forge::datafile::{CLASS_ENTITY, CLASS_MAP_SPEC, CLASS_MATERIAL, CLASS_TEXTURE_SET};
use forge::mesh::{CLASS_MESH, parse_mesh};
use forge::shape::{
    CLASS_BARREL_SHAPE, CLASS_BOX_SHAPE, CLASS_CAPSULE_SHAPE, CLASS_LIST_SHAPE, CLASS_MESH_SHAPE, Primitive, parse_mesh_shape, parse_primitive,
};
use std::collections::HashMap;
use std::path::Path;

/// The forge holding a city (`masyaf` -> `DataPC_Masyaf.forge`).
pub fn forge_of(city: &str) -> String {
    let mut c = city.chars();
    let name: String = c.next().map(|f| f.to_ascii_uppercase()).into_iter().chain(c.map(|x| x.to_ascii_lowercase())).collect();
    format!("DataPC_{name}.forge")
}

/// Game (Z up) to Bevy (Y up).
fn game_to_bevy() -> Mat4 {
    Mat4::from_quat(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
}

fn u32_at(b: &[u8], k: usize) -> u32 {
    u32::from_le_bytes([b[k], b[k + 1], b[k + 2], b[k + 3]])
}

/// The forge with the props every city shares (bushes, palms, souk windows, beams, lanterns, ladders,
/// hiding spots...), which city entities refer to by id.
const SHARED_FORGE: &str = "DataPC_Common.forge";
/// Masyaf's village collision shapes are in the Kingdom's forge.
const MASYAF_COLLISION_FORGE: &str = "DataPC_Kingdom.forge";

/// The data files of other forges that hold what the city's entities use from them: the meshes, materials and
/// collision shapes they refer to that the city lacks, then what those refer to (materials, texture sets, maps,
/// textures). The shared forge for every city; Masyaf's village collision is in the Kingdom's. Found through
/// each forge's object index (`forge::index`, built once and cached), so only those data files are read.
fn shared_datas(game_dir: &Path, forge_file: &str, city: &[Vec<u8>]) -> Result<Vec<Vec<u8>>> {
    use forge::CLASS_TEXTURE;
    use std::collections::HashSet;
    let objects: Vec<Object> = city.iter().flat_map(|d| forge::parse_objects(d).unwrap_or_default()).collect();
    let city_ids: HashSet<u32> = objects.iter().map(|o| o.id).collect();
    let mut forges = vec![SHARED_FORGE];
    if forge_file.to_lowercase().contains("masyaf") {
        forges.push(MASYAF_COLLISION_FORGE);
    }
    let shapes = [CLASS_MESH_SHAPE, CLASS_BOX_SHAPE, CLASS_BARREL_SHAPE, CLASS_CAPSULE_SHAPE, CLASS_LIST_SHAPE];
    let mut out = vec![];
    let mut seen: HashSet<u32> = city_ids.clone();
    for name in forges {
        let path = game_dir.join(name);
        let forge = forge::Forge::open(&path).with_context(|| format!("open {name}"))?;
        let idx = forge::index::ForgeIndex::load(&path)?;
        // (Ids looked up by one multiply, not SipHash: every byte offset of every entity is one; and each data file parsed
        // once, its objects' places in it kept, not once per object followed. 5 s of the Damascus load.)
        let class_of: HashMap<u32, u32, std::hash::BuildHasherDefault<IdHasher>> = idx.objects.iter().map(|(id, p)| (*id, p.class)).collect();
        let ids_in = |b: &[u8], classes: &[u32]| -> Vec<u32> {
            (0..b.len().saturating_sub(3)).map(|k| u32_at(b, k)).filter(|id| class_of.get(id).is_some_and(|c| classes.contains(c))).collect()
        };
        let mut queue: Vec<u32> = objects
            .iter()
            .filter(|o| o.class == CLASS_ENTITY || o.class == CLASS_ENTITY_GROUP)
            .flat_map(|o| ids_in(o.body, &[&[CLASS_MESH, CLASS_MATERIAL][..], &shapes].concat()))
            .collect();
        let mut kept: HashMap<String, Vec<u8>> = HashMap::new();
        // Per data file: each object's body as a range of it.
        let mut places: HashMap<String, HashMap<u32, std::ops::Range<usize>>> = HashMap::new();
        while let Some(id) = queue.pop() {
            if !seen.insert(id) {
                continue;
            }
            let file = idx.objects[&id].datafile.clone();
            if !kept.contains_key(&file) {
                let Some(e) = forge.entries.iter().find(|e| e.name == file) else { continue };
                let data = forge.read(e)?;
                let base = data.as_ptr() as usize;
                let ranges = forge::parse_objects(&data)
                    .unwrap_or_default()
                    .iter()
                    .map(|o| {
                        let start = o.body.as_ptr() as usize - base;
                        (o.id, start..start + o.body.len())
                    })
                    .collect();
                places.insert(file.clone(), ranges);
                kept.insert(file.clone(), data);
            }
            if let Some(r) = places.get(&file).and_then(|m| m.get(&id)) {
                queue.extend(ids_in(&kept[&file][r.clone()], &[CLASS_MATERIAL, CLASS_TEXTURE_SET, CLASS_MAP_SPEC, CLASS_TEXTURE]));
            }
        }
        info!("city: {} data files from {name}", kept.len());
        out.extend(kept.into_values());
    }
    Ok(out)
}

/// Data files read and decompressed on all cores, in their order (Damascus' 1,387 took 7.5 s on one).
fn read_all(forge: &forge::Forge, entries: &[&forge::Entry]) -> Vec<Vec<u8>> {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = entries.len().div_ceil(threads).max(1);
    std::thread::scope(|sc| {
        let jobs: Vec<_> =
            entries.chunks(chunk).map(|part| sc.spawn(move || part.iter().map(|e| forge.read(e).unwrap_or_default()).collect::<Vec<_>>())).collect();
        jobs.into_iter().flat_map(|j| j.join().unwrap_or_default()).collect()
    })
}

/// Object ids (CRC32s) hashed by one multiply, not SipHash: the reference scans look up every byte offset of every
/// placed entity (with the default hasher, and a scan per class, 27 s of a 60 s Damascus load).
#[derive(Default)]
struct IdHasher(u64);

impl std::hash::Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    fn write_u32(&mut self, n: u32) {
        self.0 = (n as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
}

/// The loaded objects by id.
type Index<'a> = HashMap<u32, &'a Object<'a>, std::hash::BuildHasherDefault<IdHasher>>;

/// Objects of `class` an object refers to (ids anywhere in its body), with their offsets.
fn refs<'a>(index: &Index<'a>, o: &Object, class: u32) -> Vec<(usize, &'a Object<'a>)> {
    refs_in(index, o.body, o.id, class)
}

/// `refs` in the bytes of an object (`id`) that may be embedded in another.
fn refs_in<'a>(index: &Index<'a>, b: &[u8], id: u32, class: u32) -> Vec<(usize, &'a Object<'a>)> {
    refs_all(index, b, id).into_iter().filter(|(_, r)| r.class == class).collect()
}

/// Every object referred to in the bytes of an object (`id`), with their offsets: one scan, filtered by class after.
fn refs_all<'a>(index: &Index<'a>, b: &[u8], id: u32) -> Vec<(usize, &'a Object<'a>)> {
    (0..b.len().saturating_sub(3)).filter_map(|k| index.get(&u32_at(b, k)).filter(|r| r.id != id).map(|r| (k, *r))).collect()
}

/// A primitive collision shape's triangles, in its entity's frame: a box's twelve, a convex hull's faces (the
/// vertices on each plane, in order round it), a capsule as an eight-sided prism reaching its radius past its
/// ends, a list's children's.
fn primitive_tris(p: &Primitive) -> Vec<[Vec3; 3]> {
    match p {
        Primitive::Box { half, transform } => {
            let m = Mat4::from_cols_array(transform);
            let h = Vec3::from(*half);
            let c = |x: f32, y: f32, z: f32| m.transform_point3(Vec3::new(x, y, z) * h);
            let v = [c(-1., -1., -1.), c(1., -1., -1.), c(1., 1., -1.), c(-1., 1., -1.), c(-1., -1., 1.), c(1., -1., 1.), c(1., 1., 1.), c(-1., 1., 1.)];
            let quads = [[0, 1, 2, 3], [4, 7, 6, 5], [0, 4, 5, 1], [1, 5, 6, 2], [2, 6, 7, 3], [3, 7, 4, 0]];
            quads.iter().flat_map(|q| [[v[q[0]], v[q[1]], v[q[2]]], [v[q[0]], v[q[2]], v[q[3]]]]).collect()
        }
        Primitive::Hull { transform, planes, vertices } => {
            let m = Mat4::from_cols_array(transform);
            let verts: Vec<Vec3> = vertices.iter().map(|v| Vec3::from(*v)).collect();
            let mut out = vec![];
            for pl in planes {
                let n = Vec3::new(pl[0], pl[1], pl[2]);
                let on: Vec<Vec3> = verts.iter().copied().filter(|v| (n.dot(*v) + pl[3]).abs() < 0.01).collect();
                if on.len() < 3 {
                    continue;
                }
                let c = on.iter().copied().sum::<Vec3>() / on.len() as f32;
                let u = (on[0] - c).normalize_or_zero();
                let w = n.cross(u);
                let mut ring = on.clone();
                ring.sort_by(|a, b| ((*a - c).dot(w)).atan2((*a - c).dot(u)).total_cmp(&((*b - c).dot(w)).atan2((*b - c).dot(u))));
                for k in 1..ring.len() - 1 {
                    out.push([ring[0], ring[k], ring[k + 1]].map(|v| m.transform_point3(v)));
                }
            }
            out
        }
        Primitive::Capsule { a, b, radius } => {
            let (a, b) = (Vec3::from(*a), Vec3::from(*b));
            let axis = (b - a).normalize_or(Vec3::Z);
            let (a, b) = (a - axis * *radius, b + axis * *radius);
            let u = axis.any_orthonormal_vector();
            let w = axis.cross(u);
            let ring = |c: Vec3| {
                (0..8)
                    .map(|k| c + (u * (k as f32 * std::f32::consts::FRAC_PI_4).cos() + w * (k as f32 * std::f32::consts::FRAC_PI_4).sin()) * *radius)
                    .collect::<Vec<_>>()
            };
            let (ra, rb) = (ring(a), ring(b));
            let mut out = vec![];
            for k in 0..8 {
                let j = (k + 1) % 8;
                out.push([ra[k], ra[j], rb[j]]);
                out.push([ra[k], rb[j], rb[k]]);
                out.push([a, ra[j], ra[k]]);
                out.push([b, rb[k], rb[j]]);
            }
            out
        }
        Primitive::List(kids) => kids.iter().flat_map(primitive_tris).collect(),
    }
}

/// The component that makes an entity a viewpoint.
const CLASS_REACH_HIGH_POINT: u32 = 0x1150330c;
/// The components that make an entity a haystack to dive into and a rooftop hiding place (CRC32 of `BhvHayStack`,
/// `BhvHidingPlace`; audit GUIDE-7).
const CLASS_BHV_HAYSTACK: u32 = 0x19f58e6a;
const CLASS_BHV_HIDING_PLACE: u32 = 0xdc3f581a;

/// Whether an entity's body holds an object of `class` (a component).
fn has_class(body: &[u8], class: u32) -> bool {
    (0..body.len().saturating_sub(3)).any(|k| u32_at(body, k) == class)
}

/// Hay or a hiding place, by the component an entity's (or a group's) body holds.
fn bhv_of(body: &[u8]) -> Option<crate::level::CityObject> {
    if has_class(body, CLASS_BHV_HAYSTACK) {
        Some(crate::level::CityObject::Hay)
    } else if has_class(body, CLASS_BHV_HIDING_PLACE) {
        Some(crate::level::CityObject::HidingSpot)
    } else {
        None
    }
}

/// Entity group class: entities embedded in its body (see the module docs).
const CLASS_ENTITY_GROUP: u32 = 0x3f742d26;

/// The world transform an entity's body starts with, if it is a plausible one.
fn transform(b: &[u8]) -> Option<Mat4> {
    if b.len() < 64 {
        return None;
    }
    let m: Vec<f32> = (0..16).map(|i| f32::from_bits(u32_at(b, i * 4))).collect();
    let ok = m.iter().all(|v| v.is_finite() && v.abs() < 1e5)
        && m[3].abs() < 1e-4
        && m[7].abs() < 1e-4
        && m[11].abs() < 1e-4
        && (m[15] - 1.0).abs() < 1e-4
        && (0..3).all(|c| (0.01..100.0).contains(&Vec3::new(m[c * 4], m[c * 4 + 1], m[c * 4 + 2]).length()));
    ok.then(|| Mat4::from_cols_slice(&m))
}

/// Ledges (a, b, out) of a mesh's geometry (its own space, Z up): see the module docs. `tris` come
/// with their face normals.
fn edge_ledges(tris: &[([Vec3; 3], Vec3)]) -> Vec<(Vec3, Vec3, Vec3)> {
    let q = |v: Vec3| (v * 100.0).round().as_ivec3();
    let mut edges: HashMap<(IVec3, IVec3), Vec<usize>> = HashMap::new();
    for (fi, (t, _)) in tris.iter().enumerate() {
        for (i, j) in [(0, 1), (1, 2), (2, 0)] {
            let (a, b) = (q(t[i]), q(t[j]));
            let key = if (a.x, a.y, a.z) <= (b.x, b.y, b.z) { (a, b) } else { (b, a) };
            edges.entry(key).or_default().push(fi);
        }
    }
    let centroid = |fi: usize| (tris[fi].0[0] + tris[fi].0[1] + tris[fi].0[2]) / 3.0;
    let mut out = vec![];
    for ((ka, kb), faces) in edges {
        let (a, b) = (ka.as_vec3() / 100.0, kb.as_vec3() / 100.0);
        if (a.z - b.z).abs() > 0.03 || (b - a).length() < 0.3 || faces.len() < 2 {
            continue;
        }
        let mid = (a + b) * 0.5;
        let top = faces.iter().copied().find(|&f| tris[f].1.z > 0.85);
        let wall = faces.iter().copied().find(|&f| tris[f].1.z.abs() < 0.35 && centroid(f).z < mid.z - 0.03);
        let (Some(top), Some(wall)) = (top, wall) else { continue };
        let n = tris[wall].1.with_z(0.0).normalize_or_zero();
        // Convex: the top runs back from the edge, away from where the wall faces.
        if n == Vec3::ZERO || (centroid(top) - mid).dot(n) > -0.01 {
            continue;
        }
        out.push((a, b, n));
    }
    out
}

/// A hay cart's stack, when its mesh can't give its size: half its footprint (m).
const HAY_CART_HALF: f32 = 0.9;

/// A skinned prop's skeleton has at most this many bones (a character's has 70 and more), unless the entity has collision
/// shapes (a scaffold's has 69).
const PROP_SKELETON_BONES: usize = 24;

/// Skin `m` once to `skel`'s pose (each bone's model transform times its inverse bind, as characters are
/// skinned), so a skinned prop stands as placed; bones are matched by id, then by name hash.
pub(crate) fn pose_skinned(m: &mut forge::mesh::Mesh, skel: &forge::skeleton::Skeleton) {
    let mats: Vec<Mat4> = m
        .bones
        .iter()
        .map(|b| {
            let s = skel.bones.iter().find(|s| s.id == b.id).or_else(|| skel.bones.iter().find(|s| s.name_hash == b.name_hash));
            let model = s.map_or(Mat4::IDENTITY, |s| Mat4::from_rotation_translation(Quat::from_array(s.model_rot), Vec3::from(s.model_pos)));
            model * Mat4::from_cols_array(&b.inverse_bind)
        })
        .collect();
    for (i, p) in m.positions.iter_mut().enumerate() {
        let (j, mut w) = (m.joints.get(i).copied().unwrap_or_default(), m.weights.get(i).copied().unwrap_or([1.0, 0.0, 0.0, 0.0]));
        // (No weights at all: the vertex is the first bone's.)
        if w.iter().all(|x| *x <= 0.0) {
            w = [1.0, 0.0, 0.0, 0.0];
        }
        let v = Vec3::from(*p);
        let out: Vec3 = (0..4).filter(|&k| w[k] > 0.0).map(|k| mats.get(j[k] as usize).map_or(v, |mm| mm.transform_point3(v)) * w[k]).sum();
        *p = out.to_array();
        if let Some(n) = m.normals.get_mut(i) {
            let nv: Vec3 =
                (0..4).filter(|&k| w[k] > 0.0).map(|k| mats.get(j[k] as usize).map_or(Vec3::from(*n), |mm| mm.transform_vector3(Vec3::from(*n))) * w[k]).sum();
            *n = nv.normalize_or_zero().to_array();
        }
    }
}

/// What a city mesh is for parkour, by its name: hay to jump into (`Hay_Bale_Charette`, `_Chariot`, `Hay_Bale_01`; not the
/// straw strewn on the ground, `Hay_Bale_Tile`),
/// a rooftop hiding spot (`Hiding_Spot_*`, not its `_Tarp`), a ladder (`Ladder_<h>m`), a bench (`Banc_*`), a pole (`Pole_*`: horizontal ones are swing bars).
fn city_object(name: &str) -> Option<crate::level::CityObject> {
    use crate::level::CityObject;
    let n = name.to_lowercase();
    if n.starts_with("hay_bale_charette") || n.starts_with("hay_bale_chariot") || n.starts_with("hay_bale_0") {
        Some(CityObject::Hay)
    } else if n.starts_with("hiding_spot_") && !n.contains("tarp") {
        Some(CityObject::HidingSpot)
    } else if n.starts_with("ladder_") {
        Some(CityObject::Ladder)
    } else if n.starts_with("banc_") {
        Some(CityObject::Bench)
    } else if n.starts_with("pole_") {
        Some(CityObject::Pole)
    } else {
        None
    }
}

/// Where a walkable top ends (an edge of a top face no other top face shares), with the way out from it: the
/// candidates for holds the edge test above misses (a roof on a separate wall mesh, a sill, an unwelded ledge).
/// `Level::add_probed_ledges` keeps those with a wall just under the lip once the city's collision is built.
fn lip_candidates(tris: &[([Vec3; 3], Vec3)]) -> Vec<(Vec3, Vec3, Vec3)> {
    let q = |v: Vec3| (v * 100.0).round().as_ivec3();
    let mut tops: HashMap<(IVec3, IVec3), usize> = HashMap::new();
    for (t, n) in tris.iter() {
        if n.z <= 0.85 {
            continue;
        }
        for (i, j) in [(0, 1), (1, 2), (2, 0)] {
            let (a, b) = (q(t[i]), q(t[j]));
            let key = if (a.x, a.y, a.z) <= (b.x, b.y, b.z) { (a, b) } else { (b, a) };
            *tops.entry(key).or_default() += 1;
        }
    }
    let mut out = vec![];
    for (t, n) in tris.iter() {
        if n.z <= 0.85 {
            continue;
        }
        let centroid = (t[0] + t[1] + t[2]) / 3.0;
        for (i, j) in [(0, 1), (1, 2), (2, 0)] {
            let (a, b) = (t[i], t[j]);
            let (qa, qb) = (q(a), q(b));
            let key = if (qa.x, qa.y, qa.z) <= (qb.x, qb.y, qb.z) { (qa, qb) } else { (qb, qa) };
            if tops.get(&key).copied().unwrap_or(0) > 1 || (a.z - b.z).abs() > 0.03 || (b - a).length() < LIP_MIN_LEN {
                continue;
            }
            let along = (b - a).with_z(0.0).normalize_or_zero();
            let mut away = Vec3::new(along.y, -along.x, 0.0);
            if away.dot((a + b) * 0.5 - centroid) < 0.0 {
                away = -away;
            }
            out.push((a, b, away));
        }
    }
    out
}

/// A lip shorter than this (m) is not a hold.
const LIP_MIN_LEN: f32 = 0.3;

/// Meshes holds are looked for on (buildings, walls, props), not terrain or rocks.
fn has_holds(name: &str) -> bool {
    let n = name.to_lowercase();
    !["terrain", "ground", "rock", "cliff", "roche", "matte", "stone_debris", "stonedebris"].iter().any(|k| n.contains(k))
}

/// Meshes a city shows: the full-detail ones, but not ground clutter, a mission's dead bodies, the
/// invisible out-of-bounds walls, gameplay map markers (`GP_MARK_HP1`, ...: big floating letters),
/// editor helpers or light shafts (`PillarDust`).
fn shown_mesh(name: &str) -> bool {
    let n = name.to_lowercase();
    !["_lod", "veg_clutter", "deadbody", "outofbound", "oob_", "gp_mark", "helper", "pillardust"].iter().any(|k| n.contains(k))
}

/// Wooden beams to balance on and jump between (`WoodBeam_<len>`, 0.22 m square, and the beam ends stuck out of
/// walls, `WoodBeam_Fixture`; not the thin decorative ones).
fn beam(name: &str) -> bool {
    let n = name.to_lowercase();
    n.starts_with("woodbeam_") && !n.contains("5cm")
}

/// Does a collision shape stop the player? Not AC1's out-of-bounds walls (`OutOfBound_*`: the locked districts it
/// opens as the story goes on, mission-area fences `OutofBound_Invisible_*`; the game fades them in as a glitch wall
/// instead of a solid one) or the camera's own barriers (`*CameraBarrier*`).
fn blocks_player(name: &str) -> bool {
    let n = name.to_lowercase();
    !["outofbound", "oob_", "camerabarrier", "camcollide"].iter().any(|k| n.contains(k))
}

/// Terrain, rocks and cliffs: with no rigid body of their own their meshes still collide. AC1 keeps their collision
/// in data not loaded here (Masyaf's ground, rocks and cliffs, 3.1 M triangles, in the Kingdom's world; Damascus'
/// outskirt mountains), and nothing else would hold the player up there.
fn terrain(name: &str) -> bool {
    let n = name.to_lowercase();
    ["terrain", "ground", "rock", "cliff", "roche", "mountain"].iter().any(|k| n.contains(k))
}

/// Water surfaces (AC1 draws them with its own water shader; here translucent, not solid).
fn water(name: &str) -> bool {
    let n = name.to_lowercase();
    ["water", "lake", "puddle"].iter().any(|k| n.contains(k))
}

/// Vegetation, cloth and backdrops: drawn, not collided with (nor holds found on). Grass (`*Herbe*`), plants, ivy
/// (`Liere_*`) and hiding places' tarps have no rigid body in AC1 either.
fn solid(name: &str) -> bool {
    let n = name.to_lowercase();
    ![
        "veg", "forest", "fake", "banner", "carpet", "matte", "cloth", "dust", "rope", "grass", "leaf", "tree", "olivier", "cedre", "cypres", "palm", "flag",
        "water", "lake", "puddle", "herbe", "plant", "liere", "tarp",
    ]
    .iter()
    .any(|k| n.contains(k))
}

/// Foliage and the like drawn with alpha cut-outs, both sides.
fn cutout(name: &str) -> bool {
    let n = name.to_lowercase();
    ["veg", "forest", "fake", "leaf", "tree", "olivier", "cedre", "cypres", "palm", "grass", "trellis", "alpha"].iter().any(|k| n.contains(k))
}

/// A city texture for the GPU: kept block-compressed as stored (BC1/BC2/BC3, `bc`: the device takes them) with its
/// mip chain, else decoded to RGBA8 (`image`); levels over `max` px dropped either way (`stream::Detail`). Its sampler and colour space
/// from its header.
fn city_image(body: &[u8], bc: bool, max: u32) -> Option<Image> {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    use forge::PixelFormat;
    let t = forge::texture_levels(body).ok()?;
    if !bc || !t.pixels.is_block() {
        return forge::decode_texture(body).ok().map(|t| image(t, max));
    }
    let (mut w, mut h, mut levels) = (t.width, t.height, &t.levels[..]);
    while w.max(h) > max && levels.len() > 1 {
        (w, h, levels) = ((w / 2).max(1), (h / 2).max(1), &levels[1..]);
    }
    // (Block formats take whole blocks at the top level: every one in the install has them.)
    if w % 4 != 0 || h % 4 != 0 {
        return forge::decode_texture(body).ok().map(|t| image(t, max));
    }
    let srgb = t.sampling.srgb;
    let format = match t.pixels {
        PixelFormat::Bc1 if srgb => TextureFormat::Bc1RgbaUnormSrgb,
        PixelFormat::Bc1 => TextureFormat::Bc1RgbaUnorm,
        PixelFormat::Bc2 if srgb => TextureFormat::Bc2RgbaUnormSrgb,
        PixelFormat::Bc2 => TextureFormat::Bc2RgbaUnorm,
        PixelFormat::Bc3 if srgb => TextureFormat::Bc3RgbaUnormSrgb,
        _ => TextureFormat::Bc3RgbaUnorm,
    };
    let mut img = Image::new_uninit(Extent3d { width: w, height: h, depth_or_array_layers: 1 }, TextureDimension::D2, format, RenderAssetUsages::RENDER_WORLD);
    img.texture_descriptor.mip_level_count = levels.len() as u32;
    img.data = Some(levels.concat());
    img.sampler = bevy::image::ImageSampler::Descriptor(crate::assets::sampler(&t.sampling));
    Some(img)
}

fn image(mut t: forge::Texture, max: u32) -> Image {
    while t.width.max(t.height) > max && !t.mips.is_empty() {
        t.rgba = t.mips.remove(0);
        t.width = (t.width / 2).max(1);
        t.height = (t.height / 2).max(1);
    }
    // (City UVs run past 0..1 to tile: their textures' address modes wrap.)
    crate::assets::to_image(t)
}

/// Build the city into `level` (collision) and the world (meshes): see the module docs.
#[allow(clippy::too_many_arguments)]
pub fn spawn_city(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    level: &mut Level,
    game_dir: &Path,
    forge_file: &str,
    opts: RenderOpts,
) -> Result<Vec3> {
    spawn_world(commands, meshes, mats, images, level, game_dir, forge_file, None, opts)
}

/// How a city is drawn: `bc`, its textures stay block-compressed on the GPU; `render`, it is drawn at all (not
/// headless).
#[derive(Clone, Copy)]
pub struct RenderOpts {
    pub bc: bool,
    pub render: bool,
}

/// A prop taken from a city to place elsewhere: its entity's name, where it goes (Bevy, Y up) and its turn
/// about Y, or on top of an earlier pick (its index; `at` then gives only x and z).
pub struct PropPick {
    pub name: &'static str,
    pub at: Vec3,
    pub yaw: f32,
    pub on: Option<usize>,
}

/// Some of a city's entities placed where `picks` say, drawn and colliding as in the city (AC1's
/// collision shapes where they have them); only the data files they need are read.
#[allow(clippy::too_many_arguments)]
pub fn spawn_props(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    level: &mut Level,
    game_dir: &Path,
    forge_file: &str,
    picks: &[PropPick],
    opts: RenderOpts,
) -> Result<()> {
    spawn_world(commands, meshes, mats, images, level, game_dir, forge_file, Some(picks), opts).map(|_| ())
}

/// The data files a few entities need: from each, everything it links (meshes, materials, texture sets,
/// maps, textures, collision shapes), followed through the forge's object index.
fn files_for(forge: &forge::Forge, idx: &forge::index::ForgeIndex, names: &[&str]) -> Vec<Vec<u8>> {
    let mut files: HashMap<String, Vec<u8>> = HashMap::new();
    let load = |name: &str, files: &mut HashMap<String, Vec<u8>>| {
        if !files.contains_key(name)
            && let Some(e) = forge.entries.iter().find(|e| e.name == name)
        {
            files.insert(name.to_string(), forge.read(e).unwrap_or_default());
        }
    };
    // (An object's own data file, named after it, is small; others are mission blocks holding copies.)
    let own: std::collections::HashSet<&str> = forge.entries.iter().map(|e| e.name.as_str()).collect();
    let file_of = |p: &forge::index::Place| if own.contains(p.name.as_str()) { p.name.clone() } else { p.datafile.clone() };
    let mut queue: Vec<u32> = idx
        .objects
        .iter()
        .filter(|(_, p)| (p.class == CLASS_ENTITY || p.class == CLASS_ENTITY_GROUP) && names.contains(&p.name.as_str()))
        .map(|(id, _)| *id)
        .collect();
    let mut seen: std::collections::HashSet<u32> = queue.iter().copied().collect();
    while let Some(id) = queue.pop() {
        let Some(place) = idx.objects.get(&id) else { continue };
        let file = file_of(place);
        load(&file, &mut files);
        if place.class == forge::CLASS_TEXTURE {
            continue;
        }
        let Some(data) = files.get(&file) else { continue };
        let objs = forge::parse_objects(data).unwrap_or_default();
        let Some(o) = objs.iter().find(|o| o.id == id) else { continue };
        let b = o.body;
        for k in 0..b.len().saturating_sub(3) {
            let r = u32_at(b, k);
            if idx.objects.contains_key(&r) && seen.insert(r) {
                queue.push(r);
            }
        }
    }
    files.into_values().collect()
}

#[allow(clippy::too_many_arguments)]
fn spawn_world(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    level: &mut Level,
    game_dir: &Path,
    forge_file: &str,
    picks: Option<&[PropPick]>,
    opts: RenderOpts,
) -> Result<Vec3> {
    let bc = opts.bc;
    let t0 = std::time::Instant::now();
    let forge = forge::Forge::open(game_dir.join(forge_file)).with_context(|| format!("open {forge_file}"))?;
    let mut datas: Vec<Vec<u8>> = match picks {
        Some(p) => {
            let idx = forge::index::ForgeIndex::load(&game_dir.join(forge_file))?;
            files_for(&forge, &idx, &p.iter().map(|p| p.name).collect::<Vec<_>>())
        }
        None => read_all(&forge, &forge.entries.iter().collect::<Vec<_>>()),
    };
    let shared = shared_datas(game_dir, forge_file, &datas).unwrap_or_else(|e| {
        warn!("city: shared props not loaded: {e:#}");
        vec![]
    });
    let shared_files = shared.len();
    datas.extend(shared);
    let objects: Vec<Vec<Object>> = datas.iter().map(|d| forge::parse_objects(d).unwrap_or_default()).collect();
    let mut index = Index::default();
    for o in objects.iter().flatten() {
        index.entry(o.id).or_insert(o);
    }
    // Drawn only: the shared forge's data files holding the levels of detail of the shared props (their `LODSelector`s,
    // `_LOD_*` meshes and those meshes' materials), in an index of their own over the gameplay one (which stays as it
    // is). Not when nothing is drawn (headless) or for picked props (drawn at full detail).
    let lod_files = if opts.render && picks.is_none() { lod_datas(game_dir, &objects, &index) } else { vec![] };
    let lod_objects: Vec<Vec<Object>> = lod_files.iter().map(|d| forge::parse_objects(d).unwrap_or_default()).collect();
    // (Headless: none, nothing is drawn.)
    let mut rindex = if opts.render { index.clone() } else { Index::default() };
    for o in lod_objects.iter().flatten() {
        rindex.entry(o.id).or_insert(o);
    }
    // Material -> texture set -> diffuse map spec -> texture; layered ("Multi") materials have no
    // plain diffuse spec, so then the first map spec with a texture.
    let diffuse = |material: u32| -> Option<&Object> {
        let m = rindex.get(&material)?;
        let sets: Vec<&Object> = refs(&rindex, m, CLASS_TEXTURE_SET).into_iter().map(|r| r.1).collect();
        let specs: Vec<&Object> = sets.iter().flat_map(|s| refs(&rindex, s, CLASS_MAP_SPEC)).map(|r| r.1).collect();
        let tex = |s: &&Object| refs(&rindex, s, forge::CLASS_TEXTURE).first().map(|r| r.1);
        specs
            .iter()
            .filter(|s| s.name.contains("Diffuse"))
            .find_map(tex)
            .or_else(|| specs.iter().filter(|s| !s.name.contains("Normal") && !s.name.contains("Specular")).find_map(tex))
    };
    // Its normal map (tangent space, RGB, DirectX's green; linear by its GammaSettings), if any.
    let normal_map = |material: u32| -> Option<&Object> {
        let m = rindex.get(&material)?;
        let sets: Vec<&Object> = refs(&rindex, m, CLASS_TEXTURE_SET).into_iter().map(|r| r.1).collect();
        sets.iter()
            .flat_map(|s| refs(&rindex, s, CLASS_MAP_SPEC))
            .map(|r| r.1)
            .filter(|s| s.name.contains("Normal"))
            .find_map(|s| refs(&rindex, s, forge::CLASS_TEXTURE).first().map(|r| r.1))
    };
    // A material slot for a mesh's material (`MatBank`); nothing is made when nothing is drawn.
    let detail = crate::stream::Detail::from_env();
    let mut bank = MatBank::new(bc, detail.max_texture);
    let slot_of = |bank: &mut MatBank, mats: &mut Assets<StandardMaterial>, images: &mut Assets<Image>, material: u32, cut: bool, is_water: bool| -> u32 {
        if !opts.render {
            return 0;
        }
        bank.slot(mats, images, diffuse(material), normal_map(material), cut, is_water)
    };

    let to_bevy = game_to_bevy();
    // Per mesh: its submeshes (Bevy mesh, material id), triangles and ledges (game space); None if not
    // static.
    // (Its drawn parts: material id and vertices per submesh, `stream::mesh_parts`.)
    type Parsed = Option<(Vec<(u32, usize)>, Vec<[Vec3; 3]>, Vec<(Vec3, Vec3, Vec3)>, Vec<(Vec3, Vec3, Vec3)>)>;
    let mut parsed: HashMap<u32, Parsed> = HashMap::new();
    let (mut placed, mut parts) = (0, 0);
    // (What the GPU would get at full detail: vertices of every drawn part, summed over placements.)
    let mut drawn_verts = 0usize;
    // What is drawn (`stream`): the placed meshes, with what their levels of detail need.
    let mut drawn: Vec<Drawn> = vec![];
    // (Per mesh: its index in `stored`.)
    let mut stored: Vec<crate::stream::StoredMesh> = vec![];
    let mut stored_at: HashMap<u32, usize> = HashMap::new();
    let mut houses: Vec<Vec3> = vec![];
    // What is placed: the entities, and the entities embedded in entity groups: `[id][entity class]`
    // followed by the entity's world transform and references, up to the next one.
    let mut placements: Vec<(u32, &[u8])> = vec![];
    // (The data file each one comes from, an index into `datas`; `usize::MAX` for picked props.)
    let mut placement_file: Vec<usize> = vec![];
    // Entities placed from a group marked hay or a hiding place (`bhv_of`), by id.
    let mut group_bhv: HashMap<u32, crate::level::CityObject> = HashMap::new();
    let mut vantage: Vec<Vec3> = vec![];
    let mut placed_ids = std::collections::HashSet::new();
    let mut grouped = 0;
    // Picked props: just those entities (or entity groups, members kept where they are in the group), moved to
    // their spots, their own turn and scale kept. `moved` runs alongside `placements`.
    let mut moved: Vec<Option<(Mat4, usize)>> = vec![];
    if let Some(picks) = picks {
        for (k, p) in picks.iter().enumerate() {
            let Some(o) = objects.iter().flatten().find(|o| (o.class == CLASS_ENTITY || o.class == CLASS_ENTITY_GROUP) && o.name == p.name) else {
                warn!("props: no entity or group {}", p.name);
                continue;
            };
            let Some(m) = transform(o.body) else { continue };
            let local = Mat4::from_cols(m.x_axis, m.y_axis, m.z_axis, Vec4::W);
            let at = game_to_bevy().inverse().transform_point3(p.at);
            let place = Mat4::from_translation(at) * Mat4::from_rotation_z(p.yaw);
            if o.class == CLASS_ENTITY {
                moved.push(Some((place * local, k)));
                placements.push((o.id, o.body));
                placement_file.push(usize::MAX);
                continue;
            }
            let b = o.body;
            let starts: Vec<usize> = (4..b.len().saturating_sub(68)).filter(|&j| u32_at(b, j) == CLASS_ENTITY && transform(&b[j + 4..]).is_some()).collect();
            for (i, &j) in starts.iter().enumerate() {
                let end = starts.get(i + 1).map_or(b.len(), |&n| n - 4);
                let member = &b[j + 4..end];
                let Some(mm) = transform(member) else { continue };
                moved.push(Some((place * local * m.inverse() * mm, k)));
                placements.push((u32_at(b, j - 4), member));
                placement_file.push(usize::MAX);
            }
        }
    }
    for (fi, o) in objects.iter().enumerate().flat_map(|(fi, v)| v.iter().map(move |o| (fi, o))).filter(|_| picks.is_none()) {
        if o.class == CLASS_ENTITY && placed_ids.insert(o.id) {
            // (A lone viewpoint entity, Masyaf's tutorial tower `M05_T40_Scene_TUT_ReachHighPoint`.)
            if o.name.contains("ReachHighPoint")
                && let Some(m) = transform(o.body)
            {
                vantage.push((to_bevy * m).w_axis.truncate());
            }
            placements.push((o.id, o.body));
            placement_file.push(fi);
        } else if o.class == CLASS_ENTITY_GROUP {
            let b = o.body;
            // A viewpoint tower or spire (`ReachHighPoint_*`).
            if o.name.starts_with("ReachHighPoint")
                && let Some(m) = transform(b)
            {
                vantage.push((to_bevy * m).w_axis.truncate());
            }
            let starts: Vec<usize> = (4..b.len().saturating_sub(68)).filter(|&k| u32_at(b, k) == CLASS_ENTITY && transform(&b[k + 4..]).is_some()).collect();
            // (A hiding place's component sits on a member of its group with no mesh of its own: the group's members
            // carry it.)
            let marked_group = bhv_of(b);
            for (i, &k) in starts.iter().enumerate() {
                let end = starts.get(i + 1).map_or(b.len(), |&n| n - 4);
                if placed_ids.insert(u32_at(b, k - 4)) {
                    if let Some(kind) = marked_group {
                        group_bhv.insert(u32_at(b, k - 4), kind);
                    }
                    placements.push((u32_at(b, k - 4), &b[k + 4..end]));
                    placement_file.push(fi);
                    grouped += 1;
                }
            }
        }
    }
    // Collision: AC1's own, the entities' rigid bodies by their layer (below; `AC1_COLLISION=render`: the solid render
    // meshes instead, `fallback`: the rigid bodies and, for entities with none, the solid render meshes).
    let render_collision = std::env::var("AC1_COLLISION").is_ok_and(|v| v == "render");
    let fallback_collision = std::env::var("AC1_COLLISION").is_ok_and(|v| v == "fallback");
    // (Rigid bodies whose shape is in no loaded data file; bodies left out by their layer, by layer.)
    let mut missing_shapes = 0usize;
    let mut passed_layers: std::collections::BTreeMap<u8, usize> = std::collections::BTreeMap::new();
    let show_collision = std::env::var("AC1_SHOW_COLLISION").is_ok();
    // `AC1_COLLISION_SWEEP`: what is drawn against what collides (`sweep`).
    let sweep = std::env::var("AC1_COLLISION_SWEEP").is_ok() && picks.is_none();
    let (mut sweep_drawn, mut sweep_solid) = (crate::sweep::Set::default(), crate::sweep::Set::default());
    let collision_mat = mats.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.1, 0.1, 0.35),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let mut shapes: HashMap<u32, Option<Vec<[Vec3; 3]>>> = HashMap::new();
    // (A mesh shape's triangles' sound materials, from their `CollisionMaterial`s: the footsteps' surface.)
    let mut shape_sound: HashMap<u32, Vec<u8>> = HashMap::new();
    let (mut shape_tris, mut shaped, mut prims) = (0usize, 0usize, 0usize);
    // Out-of-bounds and camera-only shapes left out of the collision.
    let mut skipped_bounds = 0usize;
    // (Entities marked hay or a hiding place by their component, those of them with no mesh named for it, and entities
    // with such a mesh name but no component.)
    let mut marked = (0usize, 0usize, 0usize);
    // (Entities collided with whose meshes are none of them drawn: solid thin air.)
    let mut undrawn = 0usize;
    // (Picks: each one's top, to stand later ones on.)
    let mut tops: HashMap<usize, f32> = HashMap::new();
    for (pi, &(ent_id, body)) in placements.iter().enumerate() {
        let picked = moved.get(pi).copied().flatten();
        let Some(mut m) = picked.map(|x| x.0).or_else(|| transform(body)) else { continue };
        let pick = picked.map(|x| x.1);
        if let Some(on) = pick.and_then(|k| picks.and_then(|p| p[k].on)).and_then(|o| tops.get(&o)) {
            // On top of an earlier pick: lifted onto its top (game Z is up).
            m.w_axis.z += *on;
        }
        let tris_before = level.tris.len();
        let world = to_bevy * m;
        // (What it refers to, scanned once, by class after.)
        let all_refs = refs_all(&index, body, ent_id);
        let refs_of = |class: u32| all_refs.iter().filter(|(_, r)| r.class == class).copied().collect::<Vec<_>>();
        // AC1's authored climbing markup on this entity (`forge::guidance`): its enabled edges, in the world.
        for g in forge::guidance::find_in_entity(body).into_iter().flatten().filter(|g| g.active) {
            for e in g.edges.iter().filter(|e| e.enabled) {
                let [n0, n1] = e.normals.map(|n| world.transform_vector3(Vec3::from(n)).normalize_or_zero());
                // (The wall's normal is the one nearer horizontal; the other is the top's.)
                let wall = if n0.y.abs() < n1.y.abs() { n0 } else { n1 };
                level.authored.push(crate::level::Authored {
                    kind: e.subtype,
                    a: world.transform_point3(Vec3::from(e.a)),
                    b: world.transform_point3(Vec3::from(e.b)),
                    out: wall.with_y(0.0).normalize_or_zero(),
                    // (The entity's own +Y, game space: along a market stall's front, its kiosk axis.)
                    axis: world.transform_vector3(Vec3::Y).with_y(0.0).normalize_or_zero(),
                });
            }
        }
        // A viewpoint (its `ReachHighPointComponent`): synchronize there (Q, standing on its perch).
        if (0..body.len().saturating_sub(3)).any(|k| u32_at(body, k) == CLASS_REACH_HIGH_POINT) {
            level.viewpoints.push(world.w_axis.truncate());
        }
        // AC1's collision (`Level::tris`): the entity's rigid bodies (`forge::shape::rigid_bodies`), each linking a shape
        // in the entity's frame, kept where the body's layer stops the player (`stops_player`: not the camera, horse,
        // projectile barriers a hiding place's box or a haystack is on) and not an out-of-bounds wall. An entity with no
        // rigid body collides with nothing in AC1 (foliage, tarps, awnings, cloth, clutter; Damascus: 3368 entities
        // with meshes), and its meshes do not stand in, but for terrain (`terrain`; `AC1_COLLISION=fallback` lets every
        // solid-named one, as before).
        let bodies = if render_collision { vec![] } else { forge::shape::rigid_bodies(body) };
        // (Its own collision, even if none of it stops the player: its meshes do not stand in.)
        let mut has_shape = false;
        let mut seen = vec![];
        for rb in bodies {
            if rb.shape == 0 || seen.contains(&rb.shape) {
                continue;
            }
            seen.push(rb.shape);
            let Some(so) = index.get(&rb.shape).copied() else {
                missing_shapes += 1;
                continue;
            };
            has_shape = true;
            if !forge::shape::stops_player(rb.layer) || !blocks_player(&so.name) {
                debug!(
                    "city: left out shape {} ({}) at {:.1}",
                    so.name,
                    forge::shape::LAYER_NAMES.get(rb.layer as usize).unwrap_or(&"?"),
                    world.w_axis.truncate()
                );
                *passed_layers.entry(rb.layer).or_default() += 1;
                skipped_bounds += 1;
                continue;
            }
            let tris = shapes.entry(so.id).or_insert_with(|| {
                let parsed = if so.class == CLASS_MESH_SHAPE {
                    parse_mesh_shape(so.body).map(|sh| {
                        // (Its triangles' sound materials, from their `CollisionMaterial`s.)
                        let sound = |k: &u8| {
                            sh.materials
                                .get(*k as usize)
                                .and_then(|id| index.get(id))
                                .filter(|o| o.class == forge::sound::CLASS_COLLISION_MATERIAL)
                                .and_then(|o| forge::sound::collision_sound_material(o.body))
                                .map_or(crate::level::NO_SOUND, |m| m as u8)
                        };
                        shape_sound.insert(so.id, sh.triangle_materials.iter().map(sound).collect());
                        sh.triangles.iter().map(|t| t.map(|i| Vec3::from(sh.vertices[i as usize]))).collect()
                    })
                } else {
                    parse_primitive(so.class, so.body).map(|p| primitive_tris(&p))
                };
                parsed.map_err(|e| warn!("city: collision shape {}: {e:#}", so.name)).ok()
            });
            let Some(tris) = tris else { continue };
            if so.class != CLASS_MESH_SHAPE {
                prims += 1;
            }
            shape_tris += tris.len();
            if let Some(sound) = shape_sound.get(&so.id).filter(|s| s.len() == tris.len()) {
                level.tri_sound.resize(level.tris.len(), crate::level::NO_SOUND);
                level.tri_sound.extend(sound);
            }
            level.tris.extend(tris.iter().map(|t| {
                let [a, b, c] = t.map(|v| world.transform_point3(v));
                Tri { a, b, c }
            }));
            if sweep {
                let name = format!("{} [{}]", so.name, forge::shape::LAYER_NAMES.get(rb.layer as usize).unwrap_or(&"?"));
                sweep_solid.push(&name, level.tris[level.tris.len() - tris.len()..].iter().copied());
            }
            if show_collision {
                let pos: Vec<Vec3> = tris.iter().flatten().copied().collect();
                let mut cm = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
                cm.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
                cm.compute_flat_normals();
                commands.spawn((Mesh3d(meshes.add(cm)), MeshMaterial3d(collision_mat.clone()), Transform::from_matrix(world)));
            }
        }
        if has_shape {
            shaped += 1;
        }
        // (No rigid body, or none loaded: its solid-named meshes collide as `AC1_COLLISION=render` / `fallback`, and its
        // terrain everywhere: `terrain`.)
        let stand_in = |name: &str| render_collision || (!has_shape && (fallback_collision || terrain(name)));
        let mut shown: Vec<u32> = vec![];
        let ent_meshes: Vec<&Object> = refs_of(CLASS_MESH)
            .into_iter()
            .map(|r| r.1)
            .filter(|r| shown_mesh(&r.name))
            .filter(|r| {
                let new = !shown.contains(&r.id);
                shown.push(r.id);
                new
            })
            .collect();
        if ent_meshes.is_empty() {
            continue;
        }
        // Hay and hiding places by AC1's own component (`BhvHayStack`, `BhvHidingPlace`, as Banned445's decode finds
        // them; audit GUIDE-7): on the entity, its meshes named for one, else its first mesh, give the object's bounds; on
        // another member of its group (a hiding place's), its meshes named for one. The mesh names alone only where
        // neither has a component (counted below).
        use crate::level::CityObject;
        let own = bhv_of(body);
        let bhv = own.or_else(|| group_bhv.get(&ent_id).copied());
        let volume = |k: Option<CityObject>| matches!(k, Some(CityObject::Hay | CityObject::HidingSpot));
        // (Named for it, or on a group member only: no first-mesh stand-in.)
        let named = bhv.is_some_and(|k| own.is_none() || ent_meshes.iter().any(|m| city_object(&m.name) == Some(k)));
        let kind_of = |i: usize, name: &str| -> Option<CityObject> {
            let by_name = city_object(name);
            match bhv {
                Some(k) if by_name == Some(k) || (!named && i == 0) => Some(k),
                Some(_) => by_name.filter(|_| !volume(by_name)),
                None => by_name,
            }
        };
        if ent_meshes.iter().enumerate().any(|(i, m)| volume(kind_of(i, &m.name))) {
            // (Its own collision's bounds, a hay cart's bed or a hiding place's box: the hop out's room test leaves them
            // out, as AC1's leaves out the haystack, `Level::hay_hop_room`.)
            let own = level.tris[tris_before..].iter().flat_map(|t| [t.a, t.b, t.c]).fold((Vec3::MAX, Vec3::MIN), |(a, b), v| (a.min(v), b.max(v)));
            if own.0.x <= own.1.x {
                level.hay_shapes.push(own);
            }
            if bhv.is_some() {
                marked.0 += 1;
                marked.1 += usize::from(!named);
            } else {
                marked.2 += 1;
            }
        }
        // (placeholder, real) material pairs: adjacent material ids.
        let mrefs = refs_of(CLASS_MATERIAL);
        let overrides: HashMap<u32, u32> = mrefs.windows(2).filter(|w| w[1].0 == w[0].0 + 4).map(|w| (w[0].1.id, w[1].1.id)).collect();
        // A skinned prop (a market stall's frame the game's physics can knock down, a haystack that gives when jumped
        // into) stands in its skeleton's pose.
        // (Props only: a character's skeleton, NPCs placed in the city, is not stood up here. A bigger skeleton on an
        // entity with collision is a prop too: Damascus's scaffolds (`Scaffold_2x3x8`, 69 bones) and storage racks (61),
        // left undrawn, were run up and climbed as thin air on their collision, user report #13.)
        let skel = refs_of(forge::skeleton::CLASS_SKELETON)
            .first()
            .and_then(|(_, o)| forge::skeleton::parse_skeleton(o.body).ok())
            .filter(|s| s.bones.len() <= PROP_SKELETON_BONES || has_shape);
        // `AC1_MESHES_AT="x,y,z,r"` (game Y-up space): the meshes of every entity standing within r of a point, logged
        // (what an unknown object in a recording is).
        if let Some(v) =
            std::env::var("AC1_MESHES_AT").ok().map(|s| s.split(',').filter_map(|v| v.trim().parse::<f32>().ok()).collect::<Vec<_>>()).filter(|v| v.len() == 4)
            && world.w_axis.truncate().distance(Vec3::new(v[0], v[1], v[2])) < v[3]
        {
            let names: Vec<&str> = ent_meshes.iter().map(|m| m.name.as_str()).collect();
            info!("meshes at {:.2}: {names:?}", world.w_axis.truncate());
        }
        let mut drawn_any = false;
        for (mi, mesh) in ent_meshes.iter().copied().enumerate() {
            // Hay whose mesh can't be read yet (its vertex format is one not decoded): a cart-sized stack where the
            // entity stands.
            if kind_of(mi, &mesh.name) == Some(CityObject::Hay) && parse_mesh(mesh.body).is_err() {
                let c = world.w_axis.truncate();
                level.city_objects.push((
                    crate::level::CityObject::Hay,
                    c - Vec3::new(HAY_CART_HALF, 0.0, HAY_CART_HALF),
                    c + Vec3::new(HAY_CART_HALF, 0.0, HAY_CART_HALF),
                    Transform::from_matrix(world),
                ));
                continue;
            }
            // A skinned parkour object with no prop skeleton to pose it (a haystack): only its bind-pose bounds,
            // for the object (not drawn).
            if skel.is_none()
                && let Some(kind) = kind_of(mi, &mesh.name)
                && let Ok(pm) = parse_mesh(mesh.body)
                && !pm.bones.is_empty()
            {
                let (lo, hi) = pm
                    .positions
                    .iter()
                    .map(|v| world.transform_point3(Vec3::from(*v)))
                    .fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), v| (a.min(v), b.max(v)));
                if lo.x <= hi.x {
                    level.city_objects.push((kind, lo, hi, Transform::from_matrix(world)));
                }
                continue;
            }
            let p = parsed.entry(mesh.id).or_insert_with(|| {
                let pm = match parse_mesh(mesh.body) {
                    Ok(pm) if pm.bones.is_empty() => pm,
                    Ok(mut pm) if skel.is_some() => {
                        if std::env::var("AC1_POSE_PROPS").is_ok() {
                            pose_skinned(&mut pm, skel.as_ref().expect("skeleton"));
                        }
                        pm
                    }
                    Ok(_) => return None,
                    Err(e) => {
                        debug!("city: mesh {} not shown: {e:#}", mesh.name);
                        return None;
                    }
                };
                let subs = crate::stream::mesh_parts(&pm).into_iter().map(|(_, m, v)| (m, v)).collect();
                let mut tris = vec![];
                let mut faces = vec![];
                for s in pm.submeshes.iter() {
                    let range = s.first_vertex as usize..(s.first_vertex + s.vertex_count) as usize;
                    let Some(idx) = pm.indices.get(s.first_index as usize..(s.first_index + s.tri_count * 3) as usize) else { continue };
                    let idx: Vec<u32> = idx.iter().map(|&i| i.saturating_sub(s.first_vertex)).collect();
                    if pm.positions.get(range.clone()).is_none() || idx.iter().any(|&i| i as usize >= range.len()) {
                        continue;
                    }
                    let pos = &pm.positions[range.clone()];
                    let nrm = &pm.normals[range.clone()];
                    for t in idx.chunks(3) {
                        let tri = [0, 1, 2].map(|k| Vec3::from(pos[t[k] as usize]));
                        tris.push(tri);
                        // Face normal, turned to agree with the vertex normals.
                        let n = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize_or_zero();
                        let vn: Vec3 = t.iter().map(|&i| Vec3::from(nrm[i as usize])).sum();
                        faces.push((tri, if n.dot(vn) < 0.0 { -n } else { n }));
                    }
                }
                let holds = has_holds(&mesh.name) && solid(&mesh.name);
                let ledges = if holds { edge_ledges(&faces) } else { vec![] };
                let lips = if holds { lip_candidates(&faces) } else { vec![] };
                Some((subs, tris, ledges, lips))
            });
            let Some((subs, tris, ledges, lips)) = p else { continue };
            drawn_any = true;
            let cut = cutout(&mesh.name);
            let is_water = water(&mesh.name);
            // What it draws: its material slot per part (the entity's overrides applied).
            let slots: Vec<u32> =
                subs.iter().map(|(m, _)| slot_of(&mut bank, &mut *mats, &mut *images, overrides.get(m).copied().unwrap_or(*m), cut, is_water)).collect();
            let verts: usize = subs.iter().map(|s| s.1).sum();
            parts += subs.len();
            drawn_verts += verts;
            if opts.render {
                let at = *stored_at.entry(mesh.id).or_insert_with(|| {
                    stored.push(crate::stream::StoredMesh {
                        body: std::sync::Arc::from(mesh.body),
                        verts,
                        materials: subs.iter().map(|s| s.0).collect(),
                        pose: skel.as_ref().filter(|_| std::env::var("AC1_POSE_PROPS").is_ok()).map(|s| std::sync::Arc::new(s.clone())),
                    });
                    stored.len() - 1
                });
                let (lo, hi) = tris
                    .iter()
                    .flatten()
                    .map(|v| world.transform_point3(*v))
                    .fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), v| (a.min(v), b.max(v)));
                drawn.push(Drawn {
                    world,
                    lo,
                    hi,
                    mesh: at,
                    mesh_id: mesh.id,
                    slots,
                    overrides: overrides.clone(),
                    cut,
                    water: is_water,
                    file: placement_file[pi],
                    entity: body,
                    entity_id: ent_id,
                });
            }
            for &(a, b, out) in ledges.iter() {
                let out = world.transform_vector3(out).with_y(0.0).normalize_or_zero();
                level.ledges.push(Ledge { a: world.transform_point3(a), b: world.transform_point3(b), out });
            }
            for &(a, b, out) in lips.iter() {
                let out = world.transform_vector3(out).with_y(0.0).normalize_or_zero();
                level.lip_candidates.push(Ledge { a: world.transform_point3(a), b: world.transform_point3(b), out });
            }
            // Wooden beams (stuck out of walls, or across streets): their top line, kept where it is out
            // in the open once the city is built (`Level::add_beam_perches`).
            if beam(&mesh.name) {
                let (lo, hi) = tris.iter().flatten().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), v| (a.min(*v), b.max(*v)));
                let size = hi - lo;
                if size.x > size.y.max(size.z) * 2.0 || mesh.name.to_lowercase().contains("fixture") {
                    let mid = (lo + hi) * 0.5;
                    let (a, b) = (Vec3::new(lo.x, mid.y, hi.z), Vec3::new(hi.x, mid.y, hi.z));
                    level.beam_tops.push(crate::level::Line { a: world.transform_point3(a), b: world.transform_point3(b) });
                }
            }
            // Gameplay objects by name (haystacks, ladders, benches, horizontal poles), by their placed bounds;
            // `Level::add_city_objects` sets them up against the built collision.
            if let Some(kind) = kind_of(mi, &mesh.name) {
                let (lo, hi) = tris
                    .iter()
                    .flatten()
                    .map(|v| world.transform_point3(*v))
                    .fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), v| (a.min(v), b.max(v)));
                if lo.x <= hi.x {
                    level.city_objects.push((kind, lo, hi, Transform::from_matrix(world)));
                }
            }
            if solid(&mesh.name) && stand_in(&mesh.name) {
                level.tris.extend(tris.iter().map(|t| {
                    let [a, b, c] = t.map(|v| world.transform_point3(v));
                    Tri { a, b, c }
                }));
                if sweep {
                    sweep_solid.push(&format!("{} (drawn)", mesh.name), level.tris[level.tris.len() - tris.len()..].iter().copied());
                }
            }
            if sweep && !is_water && solid(&mesh.name) {
                sweep_drawn.push(
                    &mesh.name,
                    tris.iter().map(|t| {
                        let [a, b, c] = t.map(|v| world.transform_point3(v));
                        Tri { a, b, c }
                    }),
                );
            }
            placed += 1;
            if mesh.name.starts_with("House") {
                houses.push(world.w_axis.truncate());
            }
        }
        if !drawn_any && level.tris.len() > tris_before {
            undrawn += 1;
            debug!(
                "city: collided with, nothing drawn: {:?} at {:.2}",
                ent_meshes.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
                world.w_axis.truncate()
            );
        }
        if let Some(k) = pick {
            let top = level.tris[tris_before..].iter().flat_map(|t| [t.a.y, t.b.y, t.c.y]).fold(f32::MIN, f32::max);
            if top > f32::MIN {
                let e = tops.entry(k).or_insert(top);
                *e = e.max(top);
            }
        }
    }
    info!("city {forge_file}: {} hay and hiding places by their component ({} with no mesh named for one), {} by mesh name only", marked.0, marked.1, marked.2);
    info!("city {forge_file}: {undrawn} entities collided with and none of their meshes drawn");
    if picks.is_some() {
        // Picked props: drawn at once, at full detail.
        for d in drawn.iter().filter(|_| opts.render) {
            let tf = Transform::from_matrix(d.world);
            for ((mesh, aabb), slot) in crate::stream::build_parts(&stored[d.mesh]).into_iter().zip(&d.slots) {
                commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(bank.list[*slot as usize].clone()), tf, aabb, bevy::camera::visibility::NoAutoAabb));
            }
        }
        info!("props from {forge_file}: {placed} meshes, {} collision triangles, {:.1}s", level.tris.len(), t0.elapsed().as_secs_f32());
        return Ok(Vec3::ZERO);
    }
    if opts.render {
        let t1 = std::time::Instant::now();
        let city_files: Vec<&str> = forge.entries.iter().map(|e| e.name.as_str()).collect();
        let store = render_store(RenderInput {
            drawn,
            stored,
            stored_at,
            rindex: &rindex,
            city_objects: &objects[..city_files.len().min(objects.len())],
            city_files: &city_files,
            mats: &mut *mats,
            images: &mut *images,
            bank: &mut bank,
            slot_of: &slot_of,
        });
        info!(
            "city {forge_file}: drawn: {} instances in {} cells, {} meshes ({:.0} MB of mesh data kept), {} stand-in pieces, {} textures {:.0} MB{}, {:.1}s",
            store.instances.len(),
            store.cells.len(),
            store.meshes.len(),
            store.meshes.iter().map(|m| m.body.len()).sum::<usize>() as f64 / 1048576.0,
            store.fakes.len(),
            bank.textures,
            bank.texture_bytes as f64 / 1048576.0,
            if bc { " (block-compressed)" } else { "" },
            t1.elapsed().as_secs_f32()
        );
        info!("city {forge_file}: detail {detail:?}");
        let render = crate::stream::CityRender::new(store, bank.list.clone(), detail, commands, meshes);
        commands.insert_resource(render);
    }
    // Navigation meshes: every cell's walkable triangles (already in world space).
    let mut nav_tris = vec![];
    for o in objects.iter().flatten().filter(|o| o.class == forge::navmesh::CLASS_NAV_MESH_MANAGER) {
        match forge::navmesh::parse_nav_mesh(o.body) {
            Ok(pieces) => {
                for p in pieces {
                    let v: Vec<Vec3> = p.vertices.iter().map(|v| to_bevy.transform_point3(Vec3::from(*v))).collect();
                    nav_tris.extend(p.triangles.iter().map(|t| t.map(|i| v[i as usize])));
                }
            }
            Err(e) => debug!("city: navigation mesh {}: {e:#}", o.name),
        }
    }
    if !nav_tris.is_empty() {
        let g = crate::nav::NavGraph::new(nav_tris);
        info!("city {forge_file}: navigation mesh of {} triangles", g.len());
        level.nav = Some(g);
    }
    // Viewpoints: on each viewpoint structure, the highest beam or post end within reach of it (where one
    // stands to synchronize), else the structure itself.
    for v in vantage {
        let near = |p: &Vec3| (*p - v).with_y(0.0).length() < 8.0;
        let top = level.beam_tops.iter().chain(level.perches.iter()).flat_map(|l| [l.a, l.b]).filter(near).max_by(|a, b| a.y.total_cmp(&b.y));
        level.viewpoints.push(top.unwrap_or(v));
    }
    info!("city {forge_file}: render: {parts} parts of {drawn_verts} vertices at full detail");
    info!(
        "city {forge_file}: {placed} meshes placed ({grouped} entities from groups, {shared_files} shared data files; {parts} parts, {} unique meshes, {} materials), {} collision triangles ({shape_tris} from {shaped} entities' collision shapes, {prims} of them primitives; {skipped_bounds} left out: out-of-bounds or on layers the player passes through {passed_layers:?}; {missing_shapes} shapes not loaded), {} ledges, {} viewpoints, {:.1}s",
        parsed.values().filter(|p| p.is_some()).count(),
        bank.textured,
        level.tris.len(),
        level.ledges.len(),
        level.viewpoints.len(),
        t0.elapsed().as_secs_f32()
    );
    if sweep {
        let (lo, hi) = houses.iter().fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(a, b), h| (a.min(h.xz()), b.max(h.xz())));
        crate::sweep::run(&sweep_drawn, &sweep_solid, (lo - Vec2::splat(10.0), hi + Vec2::splat(10.0)));
    }
    // The middle of the city: the median house.
    let median = |mut v: Vec<f32>| {
        v.sort_by(f32::total_cmp);
        v.get(v.len() / 2).copied().unwrap_or(0.0)
    };
    Ok(Vec3::new(median(houses.iter().map(|h| h.x).collect()), 0.0, median(houses.iter().map(|h| h.z).collect())))
}

/// The data files of the shared forge holding the levels of detail the city's entities use from it and the loaded files
/// lack: their `LODSelector`s, those selectors' meshes, and the meshes' materials, texture sets, maps and textures.
fn lod_datas(game_dir: &Path, objects: &[Vec<Object>], index: &Index) -> Vec<Vec<u8>> {
    use forge::lod::CLASS_LOD_SELECTOR;
    let path = game_dir.join(SHARED_FORGE);
    let (Ok(forge), Ok(idx)) = (forge::Forge::open(&path), forge::index::ForgeIndex::load(&path)) else { return vec![] };
    let class_of: HashMap<u32, u32, std::hash::BuildHasherDefault<IdHasher>> = idx.objects.iter().map(|(id, p)| (*id, p.class)).collect();
    let ids_in = |b: &[u8], classes: &[u32]| -> Vec<u32> {
        (0..b.len().saturating_sub(3))
            .map(|k| u32_at(b, k))
            .filter(|id| !index.contains_key(id) && class_of.get(id).is_some_and(|c| classes.contains(c)))
            .collect()
    };
    let mut queue: Vec<u32> = objects
        .iter()
        .flatten()
        .filter(|o| o.class == CLASS_ENTITY || o.class == CLASS_ENTITY_GROUP)
        .flat_map(|o| ids_in(o.body, &[CLASS_LOD_SELECTOR]))
        .collect();
    let mut seen = std::collections::HashSet::new();
    let mut kept: HashMap<String, Vec<u8>> = HashMap::new();
    let mut places: HashMap<String, HashMap<u32, std::ops::Range<usize>>> = HashMap::new();
    while let Some(id) = queue.pop() {
        if !seen.insert(id) {
            continue;
        }
        let Some(place) = idx.objects.get(&id) else { continue };
        let file = place.datafile.clone();
        if !kept.contains_key(&file) {
            let Some(e) = forge.entries.iter().find(|e| e.name == file) else { continue };
            let Ok(data) = forge.read(e) else { continue };
            let base = data.as_ptr() as usize;
            let ranges = forge::parse_objects(&data)
                .unwrap_or_default()
                .iter()
                .map(|o| {
                    let start = o.body.as_ptr() as usize - base;
                    (o.id, start..start + o.body.len())
                })
                .collect();
            places.insert(file.clone(), ranges);
            kept.insert(file.clone(), data);
        }
        let next: &[u32] = match place.class {
            CLASS_LOD_SELECTOR => &[CLASS_MESH],
            forge::CLASS_TEXTURE => &[],
            _ => &[CLASS_MATERIAL, CLASS_TEXTURE_SET, CLASS_MAP_SPEC, forge::CLASS_TEXTURE],
        };
        if let Some(r) = places.get(&file).and_then(|m| m.get(&id)) {
            queue.extend(ids_in(&kept[&file][r.clone()], next));
        }
    }
    info!("city: {} data files of levels of detail from {SHARED_FORGE}", kept.len());
    kept.into_values().collect()
}

/// City materials made so far: a slot per (diffuse map, normal map, cut-out, water).
struct MatBank {
    slots: HashMap<(u32, u32, bool, bool), u32>,
    list: Vec<Handle<StandardMaterial>>,
    bc: bool,
    /// The largest texture side kept.
    max_texture: u32,
    /// (Textured materials, textures, their bytes.)
    textured: usize,
    textures: usize,
    texture_bytes: usize,
}

impl MatBank {
    fn new(bc: bool, max_texture: u32) -> Self {
        Self { slots: HashMap::new(), list: vec![], bc, max_texture, textured: 0, textures: 0, texture_bytes: 0 }
    }

    /// The slot of a material with these maps (made on first use).
    fn slot(
        &mut self,
        mats: &mut Assets<StandardMaterial>,
        images: &mut Assets<Image>,
        tex: Option<&Object>,
        nrm: Option<&Object>,
        cut: bool,
        water: bool,
    ) -> u32 {
        let key = match tex {
            _ if water => (0, 0, false, true),
            None => (0, 0, cut, false),
            Some(t) => (t.id, nrm.map_or(0, |n| n.id), cut, false),
        };
        if let Some(&s) = self.slots.get(&key) {
            return s;
        }
        let material = match tex {
            _ if water => StandardMaterial {
                base_color: Color::srgba(0.2, 0.38, 0.42, 0.75),
                perceptual_roughness: 0.08,
                reflectance: 0.6,
                alpha_mode: AlphaMode::Blend,
                double_sided: true,
                cull_mode: None,
                ..default()
            },
            None if cut => {
                StandardMaterial { base_color: Color::srgb(0.36, 0.45, 0.28), perceptual_roughness: 0.9, double_sided: true, cull_mode: None, ..default() }
            }
            None => StandardMaterial { base_color: Color::srgb(0.7, 0.66, 0.58), perceptual_roughness: 0.9, ..default() },
            Some(t) => {
                let mut add = |img: Image| {
                    self.textures += 1;
                    self.texture_bytes += img.data.as_ref().map_or(0, Vec::len);
                    images.add(img)
                };
                let img = city_image(t.body, self.bc, self.max_texture).map(&mut add);
                let normal = nrm.and_then(|n| city_image(n.body, self.bc, self.max_texture)).map(&mut add);
                self.textured += 1;
                StandardMaterial {
                    base_color_texture: img,
                    normal_map_texture: normal,
                    flip_normal_map_y: true,
                    perceptual_roughness: 0.9,
                    alpha_mode: if cut { AlphaMode::Mask(0.4) } else { AlphaMode::Opaque },
                    double_sided: cut,
                    cull_mode: if cut { None } else { Some(bevy::render::render_resource::Face::Back) },
                    ..default()
                }
            }
        };
        self.list.push(mats.add(material));
        let s = self.list.len() as u32 - 1;
        self.slots.insert(key, s);
        s
    }
}

/// A placed mesh to draw, as the loader found it.
struct Drawn<'a> {
    /// Bevy space.
    world: Mat4,
    lo: Vec3,
    hi: Vec3,
    /// Its full-detail mesh: an index into the stored meshes, and its id.
    mesh: usize,
    mesh_id: u32,
    /// Its parts' material slots.
    slots: Vec<u32>,
    /// The entity's (placeholder, real) materials.
    overrides: HashMap<u32, u32>,
    cut: bool,
    water: bool,
    /// The data file (index into the loaded ones) its entity comes from.
    file: usize,
    entity: &'a [u8],
    entity_id: u32,
}

struct RenderInput<'a, F> {
    drawn: Vec<Drawn<'a>>,
    stored: Vec<crate::stream::StoredMesh>,
    stored_at: HashMap<u32, usize>,
    rindex: &'a Index<'a>,
    /// The city forge's data files' objects (in `city_files`' order) and names.
    city_objects: &'a [Vec<Object<'a>>],
    city_files: &'a [&'a str],
    mats: &'a mut Assets<StandardMaterial>,
    images: &'a mut Assets<Image>,
    bank: &'a mut MatBank,
    slot_of: &'a F,
}

/// A compact mesh of some triangles of a parsed mesh (`idx`: indices into all its vertices), render-world only.
fn piece_mesh(pm: &forge::mesh::Mesh, idx: &[u32]) -> Option<Mesh> {
    let mut map: HashMap<u32, u32> = HashMap::new();
    let (mut pos, mut nrm, mut uv, mut out) = (vec![], vec![], vec![], Vec::with_capacity(idx.len()));
    for &i in idx {
        let k = *map.entry(i).or_insert_with(|| {
            let j = i as usize;
            pos.push(pm.positions.get(j).copied().unwrap_or_default());
            nrm.push(pm.normals.get(j).copied().unwrap_or([0.0, 0.0, 1.0]));
            uv.push(pm.uvs.get(j).copied().unwrap_or_default());
            pos.len() as u32 - 1
        });
        out.push(k);
    }
    if out.len() < 3 {
        return None;
    }
    let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nrm);
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
    m.insert_indices(Indices::U32(out));
    let _ = m.generate_tangents();
    Some(m)
}

/// What a city draws (`stream`): its placed meshes with their levels of detail (the entities' `LODSelector`s), in the
/// cells of the world's streaming grid, and the world's stand-ins for the distant city.
fn render_store<F: Fn(&mut MatBank, &mut Assets<StandardMaterial>, &mut Assets<Image>, u32, bool, bool) -> u32>(
    inp: RenderInput<F>,
) -> crate::stream::RenderStore {
    use crate::stream::{Cell, FakePiece, Instance, Lod, RenderStore, StoredMesh};
    use forge::lod::{CLASS_LOD_SELECTOR, LodLevel, parse_lod_selector};
    let RenderInput { drawn, mut stored, mut stored_at, rindex, city_objects, city_files, mats, images, bank, slot_of } = inp;
    // Each entity's selectors' levels.
    let mut selectors: HashMap<u32, Option<Vec<LodLevel>>> = HashMap::new();
    let mut chains: HashMap<u32, Vec<Vec<LodLevel>>> = HashMap::new();
    for d in &drawn {
        chains.entry(d.entity_id).or_insert_with(|| {
            refs_in(rindex, d.entity, d.entity_id, CLASS_LOD_SELECTOR)
                .into_iter()
                .filter_map(|(_, o)| selectors.entry(o.id).or_insert_with(|| parse_lod_selector(o.body).ok().map(|s| s.levels())).clone())
                .collect()
        });
    }
    // The levels' meshes not stored yet, parsed on all cores: their parts (material, vertices).
    let mut lod_ids: Vec<u32> = chains.values().flatten().flatten().filter_map(|l| l.mesh).filter(|id| !stored_at.contains_key(id)).collect();
    lod_ids.sort_unstable();
    lod_ids.dedup();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = lod_ids.len().div_ceil(threads).max(1);
    let lod_parts: HashMap<u32, Vec<(u32, usize)>> = std::thread::scope(|sc| {
        let jobs: Vec<_> = lod_ids
            .chunks(chunk)
            .map(|part| {
                sc.spawn(move || {
                    part.iter()
                        .filter_map(|id| {
                            let o = rindex.get(id).filter(|o| o.class == CLASS_MESH)?;
                            let pm = parse_mesh(o.body).ok()?;
                            let parts: Vec<(u32, usize)> = crate::stream::mesh_parts(&pm).into_iter().map(|(_, m, v)| (m, v)).collect();
                            (!parts.is_empty()).then_some((*id, parts))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        jobs.into_iter().flat_map(|j| j.join().unwrap_or_default()).collect()
    });
    let mut store = RenderStore::default();
    // The grid: the world's layout and levels, if the city has them.
    let grid = city_objects.iter().flatten().find(|o| o.class == forge::world::CLASS_WORLD).and_then(|w| forge::world::grid_layout(w.body).ok());
    let levels = grid.and_then(|g| {
        city_objects.iter().flatten().find(|o| o.class == forge::world::CLASS_GRID_PARTITION).and_then(|p| forge::world::grid_levels(p.body, &g).ok())
    });
    let mut cells: HashMap<(u32, u32, u32), usize> = HashMap::new();
    let mut cell_of = |store: &mut RenderStore, key: (u32, u32, u32)| -> usize {
        *cells.entry(key).or_insert_with(|| {
            let (min, max) = match (grid, levels) {
                (Some(g), Some(_)) => {
                    let (a, b) = g.rect(key.0, key.1, key.2);
                    (Vec2::from(a), Vec2::from(b))
                }
                _ => (Vec2::splat(f32::MIN), Vec2::splat(f32::MAX)),
            };
            store.cells.push(Cell { min, max, instances: vec![] });
            store.cells.len() - 1
        })
    };
    let mut by_position = 0;
    for d in drawn {
        // Its levels: those of the entity's selector that has its mesh, else the mesh alone. A level whose mesh can't
        // be drawn draws the one before it.
        let chain = chains.get(&d.entity_id).and_then(|c| c.iter().find(|l| l.iter().any(|l| l.mesh == Some(d.mesh_id))));
        let mut lods: Vec<Lod> = vec![];
        let mut parts: Vec<Vec<u32>> = vec![];
        match chain {
            None => {
                lods.push(Lod { mesh: Some(d.mesh), until: f32::INFINITY });
                parts.push(d.slots.clone());
            }
            Some(chain) => {
                for l in chain {
                    let (mesh, slots) = match l.mesh {
                        None => (None, vec![]),
                        Some(id) if id == d.mesh_id => (Some(d.mesh), d.slots.clone()),
                        Some(id) => {
                            let at = stored_at.get(&id).copied().or_else(|| {
                                let p = lod_parts.get(&id)?;
                                let pose = stored[d.mesh].pose.clone();
                                stored.push(StoredMesh {
                                    body: std::sync::Arc::from(rindex[&id].body),
                                    verts: p.iter().map(|x| x.1).sum(),
                                    materials: p.iter().map(|x| x.0).collect(),
                                    pose,
                                });
                                stored_at.insert(id, stored.len() - 1);
                                Some(stored.len() - 1)
                            });
                            match at {
                                Some(at) => {
                                    let ms = stored[at].materials.clone();
                                    (
                                        Some(at),
                                        ms.iter()
                                            .map(|m| slot_of(bank, &mut *mats, &mut *images, d.overrides.get(m).copied().unwrap_or(*m), d.cut, d.water))
                                            .collect(),
                                    )
                                }
                                None => (lods.last().map_or(Some(d.mesh), |l| l.mesh), parts.last().cloned().unwrap_or_else(|| d.slots.clone())),
                            }
                        }
                    };
                    lods.push(Lod { mesh, until: l.until });
                    parts.push(slots);
                }
            }
        }
        // Its cell: that of the grid cell data file it comes from (`Cell<k>_DataBlock`), else the finest under it.
        let key = match (grid, levels) {
            (Some(g), Some(n)) => {
                let k = city_files
                    .get(d.file)
                    .and_then(|f| f.strip_prefix("Cell"))
                    .and_then(|f| f.strip_suffix("_DataBlock"))
                    .and_then(|k| k.parse::<usize>().ok());
                match k.and_then(|k| g.cell(k, n)) {
                    Some(c) => c,
                    None => {
                        by_position += 1;
                        let p = d.world.w_axis;
                        let (col, row) = (((p.x - g.origin[0]) / g.cell).floor(), ((-p.z - g.origin[1]) / g.cell).floor());
                        if col >= 0.0 && row >= 0.0 && (col as u32) < g.width && (row as u32) < g.width { (0, col as u32, row as u32) } else { (n - 1, 0, 0) }
                    }
                }
            }
            _ => (0, 0, 0),
        };
        let cell = cell_of(&mut store, key);
        store.cells[cell].instances.push(store.instances.len());
        store.instances.push(Instance { world: d.world, lo: d.lo, hi: d.hi, levels: lods, parts });
    }
    // The stand-ins: each block's mesh, a piece per finest cell it stands for, and the rest (always drawn).
    if let (Some(g), Some(_)) = (grid, levels)
        && let Some(fakes) = city_objects.iter().flatten().find(|o| o.class == forge::world::CLASS_FAKE_ENTITIES)
    {
        match forge::world::fake_entities(fakes.body, &g) {
            Ok(blocks) => {
                // (Pieces listed here, their meshes built on all cores: block, triangles, material, world, cell.)
                let mut pms = vec![];
                type Job = (usize, Vec<u32>, u32, Mat4, Option<usize>);
                let mut jobs: Vec<Job> = vec![];
                for b in blocks {
                    let Some(pm) = rindex.get(&b.mesh).and_then(|o| parse_mesh(o.body).ok().map(|pm| (pm, o.name.clone()))) else { continue };
                    let (pm, name) = pm;
                    let world = game_to_bevy() * Mat4::from_cols_array(&b.transform);
                    for (si, s) in pm.submeshes.iter().enumerate() {
                        let Some(all) = pm.indices.get(s.first_index as usize..(s.first_index + s.tri_count * 3) as usize) else { continue };
                        let material = slot_of(bank, &mut *mats, &mut *images, pm.materials.get(si).copied().unwrap_or(0), cutout(&name), false);
                        let mut covered = vec![false; all.len()];
                        for sp in b.spans.iter().filter(|sp| sp.submesh == si) {
                            let r = sp.first as usize..((sp.first + sp.count) as usize).min(all.len());
                            if r.is_empty() {
                                continue;
                            }
                            covered[r.clone()].iter_mut().for_each(|c| *c = true);
                            let cell = cell_of(&mut store, (0, sp.cell.0, sp.cell.1));
                            jobs.push((pms.len(), all[r].to_vec(), material, world, Some(cell)));
                        }
                        let rest: Vec<u32> = all.chunks_exact(3).enumerate().filter(|(t, _)| !covered[3 * t]).flat_map(|(_, t)| t.iter().copied()).collect();
                        jobs.push((pms.len(), rest, material, world, None));
                    }
                    pms.push(pm);
                }
                let chunk = jobs.len().div_ceil(threads).max(1);
                let pms = &pms;
                let pieces: Vec<FakePiece> = std::thread::scope(|sc| {
                    let handles: Vec<_> = jobs
                        .chunks(chunk)
                        .map(|part| {
                            sc.spawn(move || {
                                part.iter()
                                    .filter_map(|(b, idx, material, world, cell)| {
                                        piece_mesh(&pms[*b], idx).map(|mesh| FakePiece { mesh, material: *material, world: *world, cell: *cell })
                                    })
                                    .collect::<Vec<_>>()
                            })
                        })
                        .collect();
                    handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
                });
                store.fakes = pieces;
            }
            Err(e) => warn!("city: no stand-ins: {e:#}"),
        }
    }
    if by_position > 0 {
        debug!("city: {by_position} drawn meshes placed in the grid by position (not from a cell's data file)");
    }
    store.meshes = stored;
    store
}

/// A street-level spot near `centre`: flat ground among the lowest quarter of surfaces within 60 m
/// (roofs are higher, and the city's lowest parts may be a river bed), nearest the centre.
pub fn street_near(level: &Level, centre: Vec3) -> Vec3 {
    let top = |p: Vec3| level.ground(p.with_y(0.0), 400.0, 500.0);
    let mut samples = vec![];
    for r in 0..20 {
        let n = (r * 8).max(1);
        for k in 0..n {
            let a = k as f32 / n as f32 * std::f32::consts::TAU;
            let p = centre + Vec3::new(a.cos(), 0.0, a.sin()) * (r as f32 * 3.0);
            if let Some(h) = top(p).filter(|h| h.normal.y > 0.9) {
                samples.push(h.point);
            }
        }
    }
    let mut heights: Vec<f32> = samples.iter().map(|p| p.y).collect();
    heights.sort_by(f32::total_cmp);
    let (Some(&low), Some(&quarter)) = (heights.first(), heights.get(heights.len() / 4)) else { return centre };
    // Flat around it too (not the bottom of a narrow trench).
    let flat = |p: Vec3| [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z].iter().all(|d| top(p + *d * 1.5).is_some_and(|h| (h.point.y - p.y).abs() < 0.4));
    samples
        .iter()
        .copied()
        .filter(|p| p.y > low + 0.5 && p.y <= quarter + 1.0 && flat(*p))
        .min_by(|a, b| (*a - centre).length().total_cmp(&(*b - centre).length()))
        .unwrap_or(centre)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2 x 2 x 3 m box without a bottom (its top at z = 3) as triangles with outward normals.
    fn boxed() -> Vec<([Vec3; 3], Vec3)> {
        let p = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
        let quads = [
            ([p(0.0, 0.0, 3.0), p(2.0, 0.0, 3.0), p(2.0, 2.0, 3.0), p(0.0, 2.0, 3.0)], Vec3::Z),
            ([p(0.0, 0.0, 0.0), p(2.0, 0.0, 0.0), p(2.0, 0.0, 3.0), p(0.0, 0.0, 3.0)], Vec3::NEG_Y),
            ([p(0.0, 2.0, 0.0), p(2.0, 2.0, 0.0), p(2.0, 2.0, 3.0), p(0.0, 2.0, 3.0)], Vec3::Y),
            ([p(0.0, 0.0, 0.0), p(0.0, 2.0, 0.0), p(0.0, 2.0, 3.0), p(0.0, 0.0, 3.0)], Vec3::NEG_X),
            ([p(2.0, 0.0, 0.0), p(2.0, 2.0, 0.0), p(2.0, 2.0, 3.0), p(2.0, 0.0, 3.0)], Vec3::X),
        ];
        quads.iter().flat_map(|(q, n)| [([q[0], q[1], q[2]], *n), ([q[0], q[2], q[3]], *n)]).collect()
    }

    #[test]
    fn box_top_edges_are_ledges() {
        let ledges = edge_ledges(&boxed());
        // The four top edges, each facing out of its wall; no bottom or vertical edges.
        assert_eq!(ledges.len(), 4);
        for (a, b, out) in ledges {
            assert!((a.z - 3.0).abs() < 1e-4 && (b.z - 3.0).abs() < 1e-4);
            let mid = (a + b) * 0.5 - Vec3::new(1.0, 1.0, 3.0);
            assert!(mid.dot(out) > 0.9, "{mid} {out}");
        }
    }
}
