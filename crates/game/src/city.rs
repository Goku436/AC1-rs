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
//! material -> texture set -> diffuse map spec -> texture across data files. Solid meshes (not
//! foliage, cloth or distant backdrops) also become collision triangles. Masyaf: about 2300
//! entities placing 480 distinct static meshes, 4.3 million collision triangles.
//!
//! Props every city shares (bushes, palms, souk windows, beams, lanterns, ladders, hiding spots,
//! columns) live in `DataPC_Common.forge`, referred to by id from the city's entities: the data files
//! holding the ones the city uses, and their materials and textures, are read from it too (Damascus:
//! 251 of them, 240 meshes, placing 17474 meshes instead of 7362).
//!
//! Holds: the city carries no hold data we can read, so they are found in the geometry of buildings
//! and walls (not terrain or rocks): every horizontal edge (30 cm or longer) where a walkable top
//! (normal up) meets a wall below it (normal sideways), convex, becomes a ledge facing out of the
//! wall: roof edges, wall tops, sills, balconies and beam ends. The climbing moves that grab one
//! hold line (jump-grabs, wall runs, catches, shimmying, top-outs, free hangs) work on them; climbing
//! a wall hand over hand needs holds every 0.6 m, which real walls only sometimes have.

use crate::level::{Ledge, Level, Tri};
use anyhow::{Context, Result};
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
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
/// Largest texture side kept (the chain's bigger levels are dropped, to keep memory in check).
const MAX_TEXTURE: u32 = 512;

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
        let ids_in = |b: &[u8], classes: &[u32]| -> Vec<u32> {
            (0..b.len().saturating_sub(3)).map(|k| u32_at(b, k)).filter(|id| idx.objects.get(id).is_some_and(|p| classes.contains(&p.class))).collect()
        };
        let mut queue: Vec<u32> = objects
            .iter()
            .filter(|o| o.class == CLASS_ENTITY || o.class == CLASS_ENTITY_GROUP)
            .flat_map(|o| ids_in(o.body, &[&[CLASS_MESH, CLASS_MATERIAL][..], &shapes].concat()))
            .collect();
        let mut kept: HashMap<String, Vec<u8>> = HashMap::new();
        while let Some(id) = queue.pop() {
            if !seen.insert(id) {
                continue;
            }
            let file = idx.objects[&id].datafile.clone();
            if !kept.contains_key(&file) {
                let Some(e) = forge.entries.iter().find(|e| e.name == file) else { continue };
                kept.insert(file.clone(), forge.read(e)?);
            }
            if let Some(o) = forge::parse_objects(&kept[&file]).ok().and_then(|v| v.into_iter().find(|o| o.id == id)) {
                queue.extend(ids_in(o.body, &[CLASS_MATERIAL, CLASS_TEXTURE_SET, CLASS_MAP_SPEC, CLASS_TEXTURE]));
            }
        }
        info!("city: {} data files from {name}", kept.len());
        out.extend(kept.into_values());
    }
    Ok(out)
}

/// Objects of `class` an object refers to (ids anywhere in its body), with their offsets.
fn refs<'a>(index: &HashMap<u32, &'a Object<'a>>, o: &Object, class: u32) -> Vec<(usize, &'a Object<'a>)> {
    refs_in(index, o.body, o.id, class)
}

/// `refs` in the bytes of an object (`id`) that may be embedded in another.
fn refs_in<'a>(index: &HashMap<u32, &'a Object<'a>>, b: &[u8], id: u32, class: u32) -> Vec<(usize, &'a Object<'a>)> {
    (0..b.len().saturating_sub(3)).filter_map(|k| index.get(&u32_at(b, k)).filter(|r| r.class == class && r.id != id).map(|r| (k, *r))).collect()
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

/// A skinned prop's skeleton has at most this many bones (a character's has 70 and more).
const PROP_SKELETON_BONES: usize = 24;

/// Skin `m` once to `skel`'s pose (each bone's model transform times its inverse bind, as characters are
/// skinned), so a skinned prop stands as placed; bones are matched by id, then by name hash.
fn pose_skinned(m: &mut forge::mesh::Mesh, skel: &forge::skeleton::Skeleton) {
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
/// a ladder (`Ladder_<h>m`), a bench (`Banc_*`), a pole (`Pole_*`: horizontal ones are swing bars).
fn city_object(name: &str) -> Option<crate::level::CityObject> {
    use crate::level::CityObject;
    let n = name.to_lowercase();
    if n.starts_with("hay_bale_charette") || n.starts_with("hay_bale_chariot") || n.starts_with("hay_bale_0") {
        Some(CityObject::Hay)
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
    !["outofbound", "camerabarrier", "camcollide"].iter().any(|k| n.contains(k))
}

/// Water surfaces (AC1 draws them with its own water shader; here translucent, not solid).
fn water(name: &str) -> bool {
    let n = name.to_lowercase();
    ["water", "lake", "puddle"].iter().any(|k| n.contains(k))
}

/// Vegetation, cloth and backdrops: drawn, not collided with.
fn solid(name: &str) -> bool {
    let n = name.to_lowercase();
    ![
        "veg", "forest", "fake", "banner", "carpet", "matte", "cloth", "dust", "rope", "grass", "leaf", "tree", "olivier", "cedre", "cypres", "palm", "flag",
        "water", "lake", "puddle",
    ]
    .iter()
    .any(|k| n.contains(k))
}

/// Foliage and the like drawn with alpha cut-outs, both sides.
fn cutout(name: &str) -> bool {
    let n = name.to_lowercase();
    ["veg", "forest", "fake", "leaf", "tree", "olivier", "cedre", "cypres", "palm", "grass", "trellis", "alpha"].iter().any(|k| n.contains(k))
}

fn image(mut t: forge::Texture) -> Image {
    while t.width.max(t.height) > MAX_TEXTURE && !t.mips.is_empty() {
        t.rgba = t.mips.remove(0);
        t.width = (t.width / 2).max(1);
        t.height = (t.height / 2).max(1);
    }
    // City UVs run past 0..1 to tile.
    let mut img = crate::assets::to_image(t);
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        anisotropy_clamp: 8,
        ..ImageSamplerDescriptor::linear()
    });
    img
}

/// Build the city into `level` (collision) and the world (meshes): see the module docs.
pub fn spawn_city(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    level: &mut Level,
    game_dir: &Path,
    forge_file: &str,
) -> Result<Vec3> {
    spawn_world(commands, meshes, mats, images, level, game_dir, forge_file, None)
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
) -> Result<()> {
    spawn_world(commands, meshes, mats, images, level, game_dir, forge_file, Some(picks)).map(|_| ())
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
) -> Result<Vec3> {
    let t0 = std::time::Instant::now();
    let forge = forge::Forge::open(game_dir.join(forge_file)).with_context(|| format!("open {forge_file}"))?;
    let mut datas: Vec<Vec<u8>> = match picks {
        Some(p) => {
            let idx = forge::index::ForgeIndex::load(&game_dir.join(forge_file))?;
            files_for(&forge, &idx, &p.iter().map(|p| p.name).collect::<Vec<_>>())
        }
        None => forge.entries.iter().map(|e| forge.read(e).unwrap_or_default()).collect(),
    };
    let shared = shared_datas(game_dir, forge_file, &datas).unwrap_or_else(|e| {
        warn!("city: shared props not loaded: {e:#}");
        vec![]
    });
    let shared_files = shared.len();
    datas.extend(shared);
    let objects: Vec<Vec<Object>> = datas.iter().map(|d| forge::parse_objects(d).unwrap_or_default()).collect();
    let mut index: HashMap<u32, &Object> = HashMap::new();
    for o in objects.iter().flatten() {
        index.entry(o.id).or_insert(o);
    }
    // Material -> texture set -> diffuse map spec -> texture; layered ("Multi") materials have no
    // plain diffuse spec, so then the first map spec with a texture.
    let diffuse = |material: u32| -> Option<&Object> {
        let m = index.get(&material)?;
        let sets: Vec<&Object> = refs(&index, m, CLASS_TEXTURE_SET).into_iter().map(|r| r.1).collect();
        let specs: Vec<&Object> = sets.iter().flat_map(|s| refs(&index, s, CLASS_MAP_SPEC)).map(|r| r.1).collect();
        let tex = |s: &&Object| refs(&index, s, forge::CLASS_TEXTURE).first().map(|r| r.1);
        specs
            .iter()
            .filter(|s| s.name.contains("Diffuse"))
            .find_map(tex)
            .or_else(|| specs.iter().filter(|s| !s.name.contains("Normal") && !s.name.contains("Specular")).find_map(tex))
    };
    // Its normal map (tangent space, RGB, DirectX's green), if any.
    let normal_map = |material: u32| -> Option<&Object> {
        let m = index.get(&material)?;
        let sets: Vec<&Object> = refs(&index, m, CLASS_TEXTURE_SET).into_iter().map(|r| r.1).collect();
        sets.iter()
            .flat_map(|s| refs(&index, s, CLASS_MAP_SPEC))
            .map(|r| r.1)
            .filter(|s| s.name.contains("Normal"))
            .find_map(|s| refs(&index, s, forge::CLASS_TEXTURE).first().map(|r| r.1))
    };

    let to_bevy = game_to_bevy();
    // Per mesh: its submeshes (Bevy mesh, material id), triangles and ledges (game space); None if not
    // static.
    type Parsed = Option<(Vec<(Handle<Mesh>, u32)>, Vec<[Vec3; 3]>, Vec<(Vec3, Vec3, Vec3)>, Vec<(Vec3, Vec3, Vec3)>)>;
    let mut parsed: HashMap<u32, Parsed> = HashMap::new();
    let mut materials: HashMap<(u32, u32, bool), Handle<StandardMaterial>> = HashMap::new();
    let plain = mats.add(StandardMaterial { base_color: Color::srgb(0.7, 0.66, 0.58), perceptual_roughness: 0.9, ..default() });
    let plain_veg =
        mats.add(StandardMaterial { base_color: Color::srgb(0.36, 0.45, 0.28), perceptual_roughness: 0.9, double_sided: true, cull_mode: None, ..default() });
    let water_mat = mats.add(StandardMaterial {
        base_color: Color::srgba(0.2, 0.38, 0.42, 0.75),
        perceptual_roughness: 0.08,
        reflectance: 0.6,
        alpha_mode: AlphaMode::Blend,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let (mut placed, mut parts) = (0, 0);
    let mut houses: Vec<Vec3> = vec![];
    // What is placed: the entities, and the entities embedded in entity groups: `[id][entity class]`
    // followed by the entity's world transform and references, up to the next one.
    let mut placements: Vec<(u32, &[u8])> = vec![];
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
            }
        }
    }
    for o in objects.iter().flatten().filter(|_| picks.is_none()) {
        if o.class == CLASS_ENTITY && placed_ids.insert(o.id) {
            // (A lone viewpoint entity, Masyaf's tutorial tower `M05_T40_Scene_TUT_ReachHighPoint`.)
            if o.name.contains("ReachHighPoint")
                && let Some(m) = transform(o.body)
            {
                vantage.push((to_bevy * m).w_axis.truncate());
            }
            placements.push((o.id, o.body));
        } else if o.class == CLASS_ENTITY_GROUP {
            let b = o.body;
            // A viewpoint tower or spire (`ReachHighPoint_*`).
            if o.name.starts_with("ReachHighPoint")
                && let Some(m) = transform(b)
            {
                vantage.push((to_bevy * m).w_axis.truncate());
            }
            let starts: Vec<usize> = (4..b.len().saturating_sub(68)).filter(|&k| u32_at(b, k) == CLASS_ENTITY && transform(&b[k + 4..]).is_some()).collect();
            for (i, &k) in starts.iter().enumerate() {
                let end = starts.get(i + 1).map_or(b.len(), |&n| n - 4);
                if placed_ids.insert(u32_at(b, k - 4)) {
                    placements.push((u32_at(b, k - 4), &b[k + 4..end]));
                    grouped += 1;
                }
            }
        }
    }
    // Collision: AC1's own shapes (`MeshShape`) where an entity has one, else the solid render meshes
    // (`AC1_COLLISION=render`: always the render meshes).
    let render_collision = std::env::var("AC1_COLLISION").is_ok_and(|v| v == "render");
    let show_collision = std::env::var("AC1_SHOW_COLLISION").is_ok();
    let collision_mat = mats.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.1, 0.1, 0.35),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let mut shapes: HashMap<u32, Option<Vec<[Vec3; 3]>>> = HashMap::new();
    let (mut shape_tris, mut shaped, mut prims) = (0usize, 0usize, 0usize);
    // Out-of-bounds and camera-only shapes left out of the collision.
    let mut skipped_bounds = 0usize;
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
                });
            }
        }
        // A viewpoint (its `ReachHighPointComponent`): synchronize there (Q, standing on its perch).
        if (0..body.len().saturating_sub(3)).any(|k| u32_at(body, k) == CLASS_REACH_HIGH_POINT) {
            level.viewpoints.push(world.w_axis.truncate());
        }
        let mut has_shape = false;
        if !render_collision {
            let mut seen = vec![];
            for (_, so) in refs_in(&index, body, ent_id, CLASS_MESH_SHAPE) {
                if seen.contains(&so.id) {
                    continue;
                }
                // (Skipped, and its render meshes not standing in for it.)
                if !blocks_player(&so.name) {
                    debug!("city: left out shape {} at {:.1}", so.name, world.w_axis.truncate());
                    has_shape = true;
                    skipped_bounds += 1;
                    seen.push(so.id);
                    continue;
                }
                seen.push(so.id);
                let tris = shapes.entry(so.id).or_insert_with(|| match parse_mesh_shape(so.body) {
                    Ok(sh) => Some(sh.triangles.iter().map(|t| t.map(|i| Vec3::from(sh.vertices[i as usize]))).collect()),
                    Err(e) => {
                        warn!("city: collision shape {}: {e:#}", so.name);
                        None
                    }
                });
                let Some(tris) = tris else { continue };
                has_shape = true;
                shape_tris += tris.len();
                level.tris.extend(tris.iter().map(|t| {
                    let [a, b, c] = t.map(|v| world.transform_point3(v));
                    Tri { a, b, c }
                }));
                if show_collision {
                    let pos: Vec<Vec3> = tris.iter().flatten().copied().collect();
                    let mut cm = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
                    cm.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
                    cm.compute_flat_normals();
                    commands.spawn((Mesh3d(meshes.add(cm)), MeshMaterial3d(collision_mat.clone()), Transform::from_matrix(world)));
                }
            }
            // Boxes, convex hulls, capsules and lists of them.
            for (_, so) in [CLASS_BOX_SHAPE, CLASS_BARREL_SHAPE, CLASS_CAPSULE_SHAPE, CLASS_LIST_SHAPE].iter().flat_map(|&c| refs_in(&index, body, ent_id, c)) {
                if seen.contains(&so.id) {
                    continue;
                }
                if !blocks_player(&so.name) {
                    debug!("city: left out shape {} at {:.1}", so.name, world.w_axis.truncate());
                    has_shape = true;
                    skipped_bounds += 1;
                    seen.push(so.id);
                    continue;
                }
                seen.push(so.id);
                let tris = shapes.entry(so.id).or_insert_with(|| match parse_primitive(so.class, so.body) {
                    Ok(p) => Some(primitive_tris(&p)),
                    Err(e) => {
                        warn!("city: collision shape {}: {e:#}", so.name);
                        None
                    }
                });
                let Some(tris) = tris else { continue };
                has_shape = true;
                prims += 1;
                if prims % 300 == 1 {
                    debug!("city: primitive collision {} at {:.1}", so.name, world.w_axis.truncate());
                }
                shape_tris += tris.len();
                level.tris.extend(tris.iter().map(|t| {
                    let [a, b, c] = t.map(|v| world.transform_point3(v));
                    Tri { a, b, c }
                }));
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
        }
        let mut shown: Vec<u32> = vec![];
        let ent_meshes: Vec<&Object> = refs_in(&index, body, ent_id, CLASS_MESH)
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
        // (placeholder, real) material pairs: adjacent material ids.
        let mrefs = refs_in(&index, body, ent_id, CLASS_MATERIAL);
        let overrides: HashMap<u32, u32> = mrefs.windows(2).filter(|w| w[1].0 == w[0].0 + 4).map(|w| (w[0].1.id, w[1].1.id)).collect();
        let tf = Transform::from_matrix(world);
        // A skinned prop (a market stall's frame the game's physics can knock down, a haystack that gives when jumped
        // into) stands in its skeleton's pose.
        // (Props only: a character's skeleton, NPCs placed in the city, is not stood up here.)
        let skel = refs_in(&index, body, ent_id, forge::skeleton::CLASS_SKELETON)
            .first()
            .and_then(|(_, o)| forge::skeleton::parse_skeleton(o.body).ok())
            .filter(|s| s.bones.len() <= PROP_SKELETON_BONES);
        for mesh in ent_meshes {
            // Hay whose mesh can't be read yet (its vertex format is one not decoded): a cart-sized stack where the
            // entity stands.
            if city_object(&mesh.name) == Some(crate::level::CityObject::Hay) && parse_mesh(mesh.body).is_err() {
                let c = world.w_axis.truncate();
                level.city_objects.push((
                    crate::level::CityObject::Hay,
                    c - Vec3::new(HAY_CART_HALF, 0.0, HAY_CART_HALF),
                    c + Vec3::new(HAY_CART_HALF, 0.0, HAY_CART_HALF),
                ));
                continue;
            }
            // A skinned parkour object with no prop skeleton to pose it (a haystack): only its bind-pose bounds,
            // for the object (not drawn).
            if skel.is_none()
                && let Some(kind) = city_object(&mesh.name)
                && let Ok(pm) = parse_mesh(mesh.body)
                && !pm.bones.is_empty()
            {
                let (lo, hi) = pm
                    .positions
                    .iter()
                    .map(|v| world.transform_point3(Vec3::from(*v)))
                    .fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), v| (a.min(v), b.max(v)));
                if lo.x <= hi.x {
                    level.city_objects.push((kind, lo, hi));
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
                let mut subs = vec![];
                let mut tris = vec![];
                let mut faces = vec![];
                for (si, s) in pm.submeshes.iter().enumerate() {
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
                    let mut bm = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
                    bm.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos.to_vec());
                    bm.insert_attribute(Mesh::ATTRIBUTE_NORMAL, pm.normals[range.clone()].to_vec());
                    bm.insert_attribute(Mesh::ATTRIBUTE_UV_0, pm.uvs[range].to_vec());
                    bm.insert_indices(Indices::U32(idx));
                    // (Tangents for normal maps.)
                    let _ = bm.generate_tangents();
                    subs.push((meshes.add(bm), pm.materials.get(si).copied().unwrap_or(0)));
                }
                let holds = has_holds(&mesh.name) && solid(&mesh.name);
                let ledges = if holds { edge_ledges(&faces) } else { vec![] };
                let lips = if holds { lip_candidates(&faces) } else { vec![] };
                Some((subs, tris, ledges, lips))
            });
            let Some((subs, tris, ledges, lips)) = p else { continue };
            let cut = cutout(&mesh.name);
            for (h, mat) in subs.iter() {
                let mat = overrides.get(mat).copied().unwrap_or(*mat);
                let tex = diffuse(mat);
                let nrm = normal_map(mat);
                let key = (tex.map_or(0, |t| t.id), nrm.map_or(0, |t| t.id), cut);
                let material = match tex {
                    _ if water(&mesh.name) => water_mat.clone(),
                    None if cut => plain_veg.clone(),
                    None => plain.clone(),
                    Some(t) => materials
                        .entry(key)
                        .or_insert_with(|| {
                            let img = forge::decode_texture(t.body).ok().map(|t| images.add(image(t)));
                            let normal = nrm.and_then(|n| forge::decode_texture(n.body).ok()).map(|n| {
                                let mut i = image(n);
                                i.texture_descriptor.format = bevy::render::render_resource::TextureFormat::Rgba8Unorm;
                                images.add(i)
                            });
                            mats.add(StandardMaterial {
                                base_color_texture: img,
                                normal_map_texture: normal,
                                flip_normal_map_y: true,
                                perceptual_roughness: 0.9,
                                alpha_mode: if cut { AlphaMode::Mask(0.4) } else { AlphaMode::Opaque },
                                double_sided: cut,
                                cull_mode: if cut { None } else { Some(bevy::render::render_resource::Face::Back) },
                                ..default()
                            })
                        })
                        .clone(),
                };
                commands.spawn((Mesh3d(h.clone()), MeshMaterial3d(material), tf));
                parts += 1;
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
            if let Some(kind) = city_object(&mesh.name) {
                let (lo, hi) = tris
                    .iter()
                    .flatten()
                    .map(|v| world.transform_point3(*v))
                    .fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), v| (a.min(v), b.max(v)));
                if lo.x <= hi.x {
                    level.city_objects.push((kind, lo, hi));
                }
            }
            if solid(&mesh.name) && !has_shape {
                level.tris.extend(tris.iter().map(|t| {
                    let [a, b, c] = t.map(|v| world.transform_point3(v));
                    Tri { a, b, c }
                }));
            }
            placed += 1;
            if mesh.name.starts_with("House") {
                houses.push(world.w_axis.truncate());
            }
        }
        if let Some(k) = pick {
            let top = level.tris[tris_before..].iter().flat_map(|t| [t.a.y, t.b.y, t.c.y]).fold(f32::MIN, f32::max);
            if top > f32::MIN {
                let e = tops.entry(k).or_insert(top);
                *e = e.max(top);
            }
        }
    }
    if picks.is_some() {
        info!("props from {forge_file}: {placed} meshes, {} collision triangles, {:.1}s", level.tris.len(), t0.elapsed().as_secs_f32());
        return Ok(Vec3::ZERO);
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
    info!(
        "city {forge_file}: {placed} meshes placed ({grouped} entities from groups, {shared_files} shared data files; {parts} parts, {} unique meshes, {} materials), {} collision triangles ({shape_tris} from {shaped} entities' collision shapes, {prims} of them primitives; {skipped_bounds} out-of-bounds or camera-only shapes left out), {} ledges, {} viewpoints, {:.1}s",
        parsed.values().filter(|p| p.is_some()).count(),
        materials.len(),
        level.tris.len(),
        level.ledges.len(),
        level.viewpoints.len(),
        t0.elapsed().as_secs_f32()
    );
    // The middle of the city: the median house.
    let median = |mut v: Vec<f32>| {
        v.sort_by(f32::total_cmp);
        v.get(v.len() / 2).copied().unwrap_or(0.0)
    };
    Ok(Vec3::new(median(houses.iter().map(|h| h.x).collect()), 0.0, median(houses.iter().map(|h| h.z).collect())))
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
