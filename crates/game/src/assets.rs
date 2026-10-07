//! Loads a character from the user's install: merged skeleton, skinned parts and diffuse textures.

use anyhow::{Context, Result};
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use forge::datafile::{CLASS_ENTITY, DataFile};
use forge::mesh::{CLASS_MESH, parse_mesh};
use forge::skeleton::{CLASS_SKELETON, parse_skeleton};
use ik::{Rig, Xform};
use std::collections::HashMap;
use std::path::Path;

/// One drawable piece: a submesh with its texture.
#[derive(Clone)]
pub struct Part {
    pub name: String,
    pub mesh: Mesh,
    /// Rig bone per mesh joint slot.
    pub joints: Vec<usize>,
    pub inverse_binds: Vec<Mat4>,
    pub texture: Option<u32>,
    /// Its normal map (a linear texture, in `CharacterData::textures`), if it has one.
    pub normal: Option<u32>,
    pub alpha: bool,
    /// Its material's name (what an NPC builder's texture choices apply to).
    pub material: String,
}

#[derive(Clone)]
pub struct CharacterData {
    pub rig: Rig,
    /// Bone name CRC32 per rig bone (what animation tracks are keyed by).
    pub hashes: Vec<u32>,
    pub parts: Vec<Part>,
    pub textures: HashMap<u32, Image>,
}

/// Merge the entity's skeletons into one rig: add-on skeletons (head, skirt, hood, sword tag) are
/// rooted at bones the main skeleton already has, matched by name hash.
fn merge_skeletons(skels: &[forge::skeleton::Skeleton]) -> (Rig, HashMap<u32, usize>) {
    let mut rig = Rig { parents: vec![], names: vec![], rest: vec![] };
    let mut by_hash = HashMap::new();
    for s in skels {
        let mut local_to_rig = vec![0usize; s.bones.len()];
        for (i, b) in s.bones.iter().enumerate() {
            if let Some(&r) = by_hash.get(&b.name_hash) {
                local_to_rig[i] = r;
                continue;
            }
            let r = rig.parents.len();
            rig.parents.push(b.parent.map(|p| local_to_rig[p]));
            rig.names.push(Some(b.name.map(str::to_owned).unwrap_or_else(|| format!("#{:08x}", b.name_hash))));
            let [x, y, z, w] = b.local_rot;
            rig.rest.push(Xform::new(Vec3::from(b.local_pos), Quat::from_xyzw(x, y, z, w).normalize()));
            by_hash.insert(b.name_hash, r);
            local_to_rig[i] = r;
        }
    }
    (rig, by_hash)
}

fn hashes_of(rig_len: usize, by_hash: &HashMap<u32, usize>) -> Vec<u32> {
    let mut h = vec![0; rig_len];
    for (&hash, &i) in by_hash {
        h[i] = hash;
    }
    h
}

fn xform_mat(x: &Xform) -> Mat4 {
    Mat4::from_rotation_translation(x.rot, x.pos)
}

/// Load `entity` (e.g. "UCMA_Altair_Rank_9") from data file `datafile` in `forge_file`.
pub fn load_character(game_dir: &Path, forge_file: &str, datafile: &str, entity: &str) -> Result<CharacterData> {
    let forge = forge::Forge::open(game_dir.join(forge_file))?;
    let entry = forge.entries.iter().find(|e| e.name == datafile).with_context(|| format!("no data file {datafile}"))?;
    let bytes = forge.read(entry)?;
    let df = DataFile::parse(&bytes)?;
    let ent = df.by_name(CLASS_ENTITY, entity).with_context(|| format!("no entity {entity}"))?;
    let refs = df.refs(ent);

    // Skeletons in reference order: main skeleton first.
    let mut skels = vec![];
    let mut seen = vec![];
    for (_, o) in refs.iter().filter(|(_, o)| o.class == CLASS_SKELETON) {
        if !seen.contains(&o.id) {
            seen.push(o.id);
            skels.push(parse_skeleton(o.body).with_context(|| format!("skeleton {}", o.name))?);
        }
    }
    let (rig, by_hash) = merge_skeletons(&skels);
    let overrides = df.material_overrides(ent);
    info!("{entity}: {} skeletons -> {} bones, {} material overrides", skels.len(), rig.len(), overrides.len());

    // Materials whose texture sets live in the game's shared bootstrap file (the throwing daggers).
    let shared_bytes = forge.entries.iter().find(|e| e.name == SHARED_DATAFILE).and_then(|e| forge.read(e).ok());
    let shared = shared_bytes.as_deref().and_then(|b| DataFile::parse(b).ok());
    let mut parts = vec![];
    let mut textures = HashMap::new();
    let mut seen_meshes = vec![];
    let src = Source { df: &df, shared: shared.as_ref(), overrides: &overrides };
    for (_, o) in refs.iter().filter(|(_, o)| o.class == CLASS_MESH) {
        // Skip duplicate refs and distant LODs.
        if seen_meshes.contains(&o.id) || o.name.contains("_LOD") {
            continue;
        }
        seen_meshes.push(o.id);
        add_mesh_parts(&src, o, &rig, &by_hash, None, &mut parts, &mut textures);
    }
    // Weapons carried on tag bones (rigid, no skin of their own; the entity does not list them).
    for (name, bone) in CARRIED {
        if let (Some(o), Some(&r)) = (df.by_name(CLASS_MESH, name), by_hash.get(&bone)) {
            add_mesh_parts(&src, o, &rig, &by_hash, Some(r), &mut parts, &mut textures);
        }
    }
    let hashes = hashes_of(rig.len(), &by_hash);
    Ok(CharacterData { rig, hashes, parts, textures })
}

/// Assemble a character from named parts spread over data files and forges: `skeletons` (main first) and
/// `meshes` as (forge file, data file, object name). AC1 builds its crowds this way: shared body parts,
/// heads and skeletons live in their own data files (crowd characters keep their clothes in a city's forge
/// and share skeletons and limbs from the common one). Skeletons stored next to the meshes (head face
/// bones, ...) are merged in after the listed ones.
pub fn load_assembled_from(game_dir: &Path, skeletons: &[(String, String, String)], meshes: &[(String, String, String)]) -> Result<CharacterData> {
    load_assembled_with(game_dir, skeletons, meshes, true)
}

/// `load_assembled_from`; `auto_skeletons`: also every skeleton in the meshes' data files (hand-made recipes
/// rely on it for hoods and such; an NPC builder names its own).
pub fn load_assembled_with(
    game_dir: &Path,
    skeletons: &[(String, String, String)],
    meshes: &[(String, String, String)],
    auto_skeletons: bool,
) -> Result<CharacterData> {
    let mut forges: HashMap<String, forge::Forge> = HashMap::new();
    let mut files: HashMap<String, Vec<u8>> = HashMap::new();
    for (ff, df, _) in skeletons.iter().chain(meshes) {
        if !files.contains_key(df) {
            if !forges.contains_key(ff) {
                forges.insert(ff.clone(), forge::Forge::open(game_dir.join(ff))?);
            }
            let forge = &forges[ff];
            let entry = forge.entries.iter().find(|e| e.name == *df).with_context(|| format!("no data file {df} in {ff}"))?;
            files.insert(df.clone(), forge.read(entry)?);
        }
    }
    let skeletons: Vec<(&str, &str)> = skeletons.iter().map(|(_, d, n)| (d.as_str(), n.as_str())).collect();
    let meshes: Vec<(&str, &str)> = meshes.iter().map(|(_, d, n)| (d.as_str(), n.as_str())).collect();
    let dfs: HashMap<&str, DataFile> = files.iter().map(|(k, v)| DataFile::parse(v).map(|d| (k.as_str(), d))).collect::<Result<_>>()?;
    let mut skels = vec![];
    let mut seen = vec![];
    for &(df, name) in &skeletons {
        let o = dfs[df].by_name(CLASS_SKELETON, name).with_context(|| format!("no skeleton {name} in {df}"))?;
        seen.push(o.id);
        skels.push(parse_skeleton(o.body).with_context(|| format!("skeleton {name}"))?);
    }
    for &(df, _) in meshes.iter().filter(|_| auto_skeletons) {
        for o in dfs[df].objects.iter().filter(|o| o.class == CLASS_SKELETON) {
            if !seen.contains(&o.id) {
                seen.push(o.id);
                skels.push(parse_skeleton(o.body).with_context(|| format!("skeleton {}", o.name))?);
            }
        }
    }
    let (rig, by_hash) = merge_skeletons(&skels);
    let mut parts = vec![];
    let mut textures = HashMap::new();
    for &(df, name) in &meshes {
        let d = &dfs[df];
        let Some(o) = d.by_name(CLASS_MESH, name) else {
            warn!("no mesh {name} in {df}");
            continue;
        };
        add_mesh_parts(&Source { df: d, shared: None, overrides: &HashMap::new() }, o, &rig, &by_hash, None, &mut parts, &mut textures);
    }
    info!("assembled {} skeletons -> {} bones, {} parts", skels.len(), rig.len(), parts.len());
    let hashes = hashes_of(rig.len(), &by_hash);
    Ok(CharacterData { rig, hashes, parts, textures })
}

/// A texture by name from a data file (its id and image): to put the real one in for a placeholder an
/// entity overrides (a guard's robe).
pub fn load_texture(game_dir: &Path, forge_file: &str, datafile: &str, name: &str) -> Result<(u32, Image)> {
    let forge = forge::Forge::open(game_dir.join(forge_file))?;
    let entry = forge.entries.iter().find(|e| e.name == datafile).with_context(|| format!("no data file {datafile}"))?;
    let data = forge.read(entry)?;
    let objects = forge::parse_objects(&data)?;
    let t = objects.iter().find(|o| o.class == forge::CLASS_TEXTURE && o.name == name).with_context(|| format!("no texture {name} in {datafile}"))?;
    let img = forge::decode_texture(t.body).with_context(|| format!("decode {name}"))?;
    Ok((t.id, to_image(img)))
}

/// Upload with the texture's own mip chain and trilinear, anisotropic filtering (without mips the
/// textures shimmer as the character moves). A full chain goes down to 1x1; a partial one is dropped.
/// Data file `"Game Bootstrap Settings"` (in DataPC.forge) holds texture sets other files' materials name.
const SHARED_DATAFILE: &str = "Game Bootstrap Settings";

/// Weapons Altaïr carries, each on its tag bone (by name hash): the sword in its sheath (the `UCMA_Sword_Tag`
/// skeleton's bone) and the short blade in the back sheath. The pairs are from Banned445's AC1-Movement-Rewritten.
const CARRIED: [(&str, u32); 2] = [("ARCM_Altair_Sword_D", 0x3a83_5926), ("UCMA_Altair_Dagger", 0x685e_46b6)];

/// Where a character's meshes and materials come from.
struct Source<'s, 'a> {
    df: &'s DataFile<'a>,
    /// The shared bootstrap file, for materials whose texture sets are not in `df`.
    shared: Option<&'s DataFile<'s>>,
    /// Material overrides (placeholder -> real).
    overrides: &'s HashMap<u32, u32>,
}

/// Skinned submeshes of mesh object `o` bound to `rig`, with their diffuse textures. A mesh with no skin of its own
/// is drawn on bone `tag` (its vertices in that bone's frame), or left out without one.
fn add_mesh_parts(
    src: &Source,
    o: &forge::Object,
    rig: &Rig,
    by_hash: &HashMap<u32, usize>,
    tag: Option<usize>,
    parts: &mut Vec<Part>,
    textures: &mut HashMap<u32, Image>,
) {
    let rest_model = rig.rest_pose().model(rig);
    let mut m = match parse_mesh(o.body) {
        Ok(m) if !m.bones.is_empty() || tag.is_some() => m,
        Ok(_) => return,
        Err(e) => {
            warn!("mesh {}: {e:#}", o.name);
            return;
        }
    };
    if let (true, Some(r)) = (m.bones.is_empty(), tag) {
        m.joints = vec![[0; 4]; m.positions.len()];
        m.weights = vec![[1.0, 0.0, 0.0, 0.0]; m.positions.len()];
        add_submeshes(src, o, &m, vec![r], vec![Mat4::IDENTITY], parts, textures);
        return;
    }
    let joints: Vec<usize> = m.bones.iter().map(|b| by_hash.get(&b.name_hash).copied().unwrap_or(0)).collect();
    let missing: Vec<String> = m.bones.iter().filter(|b| !by_hash.contains_key(&b.name_hash)).map(|b| format!("{:08x}", b.name_hash)).collect();
    if !missing.is_empty() {
        warn!("mesh {}: {} of {} bones not in the rig: {missing:?}", o.name, missing.len(), m.bones.len());
    }

    // Mesh space -> rig model space, averaged over bones (each gives rest_model * inverse_bind).
    let mut rot_sum = Vec4::ZERO;
    let mut first: Option<Quat> = None;
    let mut pos_sum = Vec3::ZERO;
    for (b, &r) in m.bones.iter().zip(&joints) {
        let s = xform_mat(&rest_model[r]) * Mat4::from_cols_array(&b.inverse_bind);
        let (_, q, t) = s.to_scale_rotation_translation();
        let q = match first {
            Some(f) if f.dot(q) < 0.0 => -q,
            Some(_) => q,
            None => {
                first = Some(q);
                q
            }
        };
        rot_sum += Vec4::from(q);
        pos_sum += t;
    }
    let n = m.bones.len() as f32;
    let mesh_to_model = Mat4::from_rotation_translation(Quat::from_vec4(rot_sum).normalize(), pos_sum / n);
    // The shared eyes and mouth (`Universal_Head_Obj*`) are modelled for a generic face and skinned to each
    // head's face bones: their own bind matrices put them on this face (a rigid placement shows the teeth
    // through the chin).
    let shared_face = o.name.starts_with("Universal_Head_Obj");
    let inverse_binds: Vec<Mat4> = if shared_face {
        m.bones.iter().map(|b| Mat4::from_cols_array(&b.inverse_bind)).collect()
    } else {
        joints.iter().map(|&r| xform_mat(&rest_model[r]).inverse() * mesh_to_model).collect()
    };

    add_submeshes(src, o, &m, joints, inverse_binds, parts, textures);
}

/// One part per submesh of `m`, skinned to rig bones `joints` with `inverse_binds`.
fn add_submeshes(
    src: &Source,
    o: &forge::Object,
    m: &forge::mesh::Mesh,
    joints: Vec<usize>,
    inverse_binds: Vec<Mat4>,
    parts: &mut Vec<Part>,
    textures: &mut HashMap<u32, Image>,
) {
    let (df, overrides) = (src.df, src.overrides);
    for (si, sub) in m.submeshes.iter().enumerate() {
        let v0 = sub.first_vertex as usize;
        let vn = sub.vertex_count as usize;
        let range = v0..v0 + vn;
        let idx: Vec<u32> = m.indices[sub.first_index as usize..(sub.first_index + sub.tri_count * 3) as usize].iter().map(|&i| i - sub.first_vertex).collect();
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, m.positions[range.clone()].to_vec());
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals[range.clone()].to_vec());
        let uvs: Vec<[f32; 2]> = m.uvs[range.clone()].to_vec();
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_INDEX, VertexAttributeValues::Uint16x4(m.joints[range.clone()].to_vec()));
        mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, m.weights[range.clone()].to_vec());
        mesh.insert_indices(Indices::U32(idx));

        let mat_id = m.materials.get(si).copied();
        let real = mat_id.map(|id| *overrides.get(&id).unwrap_or(&id));
        let mat_name =
            real.and_then(|id| df.get(id).map(|o| o.name.clone()).or_else(|| src.shared.and_then(|sh| sh.get(id)).map(|o| o.name.clone()))).unwrap_or_default();
        let tex = real.and_then(|id| df.material_diffuse(id).or_else(|| src.shared.and_then(|sh| df.material_map_in(sh, id, "Diffuse"))));
        if let Some(t) = tex
            && !textures.contains_key(&t.id)
        {
            match forge::decode_texture(t.body) {
                Ok(img) => {
                    textures.insert(t.id, to_image(img));
                }
                Err(e) => warn!("texture {}: {e:#}", t.name),
            }
        }
        // The normal map (tangent space, RGB, DirectX's green), with tangents for it.
        let nrm = real.and_then(|id| df.material_map(id, "Normal").or_else(|| src.shared.and_then(|sh| df.material_map_in(sh, id, "Normal"))));
        if let Some(n) = nrm
            && !textures.contains_key(&n.id)
        {
            match forge::decode_texture(n.body) {
                Ok(img) => {
                    let mut image = to_image(img);
                    image.texture_descriptor.format = TextureFormat::Rgba8Unorm;
                    textures.insert(n.id, image);
                }
                Err(e) => warn!("normal map {}: {e:#}", n.name),
            }
        }
        let nrm = nrm.filter(|_| mesh.generate_tangents().is_ok());
        info!("  {} sub {si}: {} verts, material {mat_name}, texture {:?}", o.name, vn, tex.map(|t| &t.name));
        parts.push(Part {
            name: format!("{}#{si}", o.name),
            mesh,
            joints: joints.clone(),
            inverse_binds: inverse_binds.clone(),
            texture: tex.map(|t| t.id),
            normal: nrm.map(|t| t.id),
            alpha: mat_name.contains("Alpha") || mat_name.starts_with("Eye"),
            material: mat_name.clone(),
        });
    }
}

pub(crate) fn to_image(t: forge::Texture) -> Image {
    let full = 32 - t.width.max(t.height).leading_zeros();
    let levels = if t.mips.len() as u32 + 1 == full { full } else { 1 };
    let mut image = Image::new(
        Extent3d { width: t.width, height: t.height, depth_or_array_layers: 1 },
        TextureDimension::D2,
        t.rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    if levels > 1
        && let Some(data) = &mut image.data
    {
        data.extend(t.mips.into_iter().flatten());
        image.texture_descriptor.mip_level_count = levels;
    }
    debug!("texture {}x{}: {levels} mip levels", t.width, t.height);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        anisotropy_clamp: 8,
        address_mode_u: bevy::image::ImageAddressMode::Repeat,
        address_mode_v: bevy::image::ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    image
}
