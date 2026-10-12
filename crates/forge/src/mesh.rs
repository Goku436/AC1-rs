//! Mesh objects (class 0x415d9568).
//!
//! Body layout, reversed from the user's install:
//! - u8 1, 7 zero bytes, u32 bone count, bones x 0x4C:
//!   { u32 id, u32 class, u32 name hash, f32[16] inverse bind matrix (row-major, row-vector, translation in row 3) }
//! - 6 bytes, u32 class 0xfc9e1595 (compiled mesh), u32 size, u32 ?, u32 vertex stride,
//!   u32 vertex bytes, u32 index bytes, 8 bytes, u32 submesh count, u32 submesh count (2nd table),
//!   u32 blob size, then vertices, u16 indices (u32 in a mesh of more than 65535 vertices), two submesh tables
//!   (20 B each: u32 4, u32 first vertex, u32 vertex count, u32 first index, u32 triangle count)
//! - per submesh bone palette: u8 3, u8 1, u8 n, u16 ?, u16 vertex count, u8[n] skeleton bone indices.
//! - ... trailer: u32 submesh count, u32 material id per submesh, 5 bytes.
//!
//! Cloth meshes (first byte 4, e.g. `UCMA_Altair_Cloth`, the long robe): u32 4, u32 1, u8 0, an embedded
//! cloth-simulation object (class 0x5755de7f, `SubMesh`: read by `cloth::soft_mesh`), then the usual u32 bone count, bone table
//! and compiled mesh. The bone table is found by walking back from the compiled-mesh tag to a count that
//! matches; the cloth is drawn as a plain skinned mesh.
//!
//! Static meshes (first byte 0) have the same layout with no bones and no palettes, and stride-24
//! vertices: the stride-32 layout without the palette indices and weights. The position's w is a
//! scale shared by the whole mesh (its sign varies): positions are `xyz / 32768 * |w| / 8` metres, so
//! every mesh uses the full i16 range whatever its size (Altaïr's sword: |w| = 8, 0.95 m long; a
//! Masyaf terrain tile: |w| = 3976, the tiles meeting exactly and spanning the map's 1 km; a house
//! lands on the ground it is placed on).
//! A static vertex is i16x4 position, u8x4 normal, u8x4 tangent, the vertex colour (a D3DCOLOR: bytes B, G, R, A), i16x2
//! uv: there is no binormal. The world's vertex shaders (`GEN_Standard`, `Env_MultiBlender2`, `GFX_Water_Puddle` in the
//! data's `MaterialTemplate`s) take the binormal as normal x tangent times the sign of the position's w, the colour as a
//! multiplier on the diffuse (and its alpha on the output's), and the normal's and tangent's fourth bytes (/ 255) as the
//! layered materials' blend weights (`COLOR1`). Over Damascus's 8.26 M static vertices (`examples/vtxw`): 7.39 M white,
//! 0.44 M grey, 0.43 M coloured (the stand-ins' baked tints, the map markers), 0.20 M with alpha under 250 (the puddles'
//! and rivers' faded edges), 3.05 M with a layer weight.
//!
//! Stride-32 vertex: i16x4 position (w = 32767, units of 1/2048 m), u8x4 normal, u8x4 tangent,
//! u8x4 binormal (the tangent frame the normal maps were baked in; read as `Mesh::tangents`), i16x2 uv (1/2048, v down as images are stored; values run past one tile, so samplers repeat), u8x4 palette indices, u8x4 weights (sum about 255;
//! normalized to 1 on load).

use crate::{u16_at, u32_at};
use anyhow::{Result, bail, ensure};

pub const CLASS_MESH: u32 = 0x415d9568;
const CLASS_COMPILED: u32 = 0xfc9e1595;
/// The cloth-simulation object (`SubMesh`) embedded in cloth, vegetation and soft-body meshes (`cloth::soft_mesh`).
const CLASS_CLOTH_SIM: u32 = 0x5755de7f;
pub const POS_SCALE: f32 = 1.0 / 2048.0;
/// Static (stride-24) vertex positions: i16 fractions of `|w| / 8` metres, `w` the position's fourth
/// component (see the module docs).
/// UV units per texture width (i16 / 2048). (Read as 1/4096 with v flipped before 2026-10-06: every texture showed at
/// half scale; Banned445's AC1-Movement-Rewritten had it right.)
pub const UV_SCALE: f32 = 2048.0;
pub const STATIC_POS_SCALE: f32 = 1.0 / 32768.0;

#[derive(Debug, Clone)]
pub struct MeshBone {
    pub id: u32,
    pub name_hash: u32,
    pub inverse_bind: [f32; 16],
}

#[derive(Debug, Clone, Copy)]
pub struct Submesh {
    pub first_vertex: u32,
    pub vertex_count: u32,
    pub first_index: u32,
    pub tri_count: u32,
}

#[derive(Debug, Clone)]
pub struct Mesh {
    pub bones: Vec<MeshBone>,
    pub stride: usize,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// The stored tangent (xyz, unit) and its handedness (w, +-1: the stored binormal's side of normal x tangent),
    /// as Bevy's `ATTRIBUTE_TANGENT` takes them.
    pub tangents: Vec<[f32; 4]>,
    pub uvs: Vec<[f32; 2]>,
    /// Skeleton (mesh bone table) indices, already resolved through the submesh palette.
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    pub submeshes: Vec<Submesh>,
    /// Material object id per submesh (often an `_Empty` placeholder the entity overrides).
    pub materials: Vec<u32>,
    /// Static meshes: the vertex colour, RGBA (empty for other strides).
    pub colors: Vec<[u8; 4]>,
    /// Static meshes: the layered materials' blend weights, the normal's and the tangent's fourth bytes (empty for other
    /// strides).
    pub layers: Vec<[u8; 2]>,
}

fn f32_at(d: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}

fn unpack_normal(b: &[u8]) -> [f32; 3] {
    let f = |x: u8| x as f32 / 127.5 - 1.0;
    let n = [f(b[0]), f(b[1]), f(b[2])];
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
    [n[0] / l, n[1] / l, n[2] / l]
}

/// A tangent with its handedness: w = -1 where the binormal is on the far side of normal x tangent.
fn tangent_frame(n: [f32; 3], t: [f32; 3], b: [f32; 3]) -> [f32; 4] {
    let c = [n[1] * t[2] - n[2] * t[1], n[2] * t[0] - n[0] * t[2], n[0] * t[1] - n[1] * t[0]];
    let w = if c[0] * b[0] + c[1] * b[1] + c[2] * b[2] < 0.0 { -1.0 } else { 1.0 };
    [t[0], t[1], t[2], w]
}

/// Offset of the bone count in a cloth mesh (type 4): the count `n` followed by `n` bone entries
/// that end where the compiled mesh starts.
fn cloth_bone_table(body: &[u8]) -> Result<usize> {
    let tag = CLASS_COMPILED.to_le_bytes();
    let Some(c) = body.windows(4).position(|w| w == tag) else { bail!("cloth mesh: no compiled mesh tag") };
    ensure!(c >= 6, "cloth mesh: compiled mesh tag at {c:#x}");
    let h = c - 6;
    // (From no bones: static vegetation LODs carry the simulation object and an empty bone table.)
    (0..1024)
        .map(|n| (n, h.checked_sub(n * 0x4C + 4)))
        .take_while(|(_, o)| o.is_some())
        .find_map(|(n, o)| o.filter(|&o| u32_at(body, o) as usize == n))
        .ok_or_else(|| anyhow::anyhow!("cloth mesh: no bone table before {h:#x}"))
}

/// What kind of mesh a body holds, from its header (no vertices read): see the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MeshKind {
    /// Type 0 or 1 with no bones.
    Static,
    /// Type 0 or 1 with bones: skinned to a skeleton (characters, and props the game's physics or animations move:
    /// haystacks, stalls, pigeons).
    Skinned,
    /// Type 0 or 1 with an embedded soft body (`Veg_Olivier_*`, `Herbe_Scatter_*`, `*_Softbody`).
    Simulated,
    /// Type 2: effects (`FX_*`).
    Effects,
    /// Type 4: cloth (the robe, awnings, flags).
    Cloth,
    /// Any other first byte.
    Unknown,
}

pub fn mesh_kind(body: &[u8]) -> MeshKind {
    let simulated = body.len() > 17 && body[8] == 0 && u32_at(body, 13) == CLASS_CLOTH_SIM;
    match body.first() {
        Some(0 | 1) if simulated => MeshKind::Simulated,
        Some(0 | 1) if body.len() >= 12 && u32_at(body, 8) > 0 => MeshKind::Skinned,
        Some(0 | 1) => MeshKind::Static,
        Some(2) => MeshKind::Effects,
        Some(4) => MeshKind::Cloth,
        _ => MeshKind::Unknown,
    }
}

pub fn parse_mesh(body: &[u8]) -> Result<Mesh> {
    ensure!(body.len() > 12, "mesh body too short ({} bytes)", body.len());
    // (The cloth-simulation object embedded after `u32 type, u32 1 or 5, u8 0`: on the robe (type 4), and on vegetation and
    // soft bodies with type 0 or 1 (`Veg_Olivier_*`, `Herbe_Scatter_*`, `UCMA_*_Softbody`).)
    let simulated = body.len() > 17 && body[8] == 0 && u32_at(body, 13) == CLASS_CLOTH_SIM;
    let count_at = match body[0] {
        0 | 1 if simulated => cloth_bone_table(body)?,
        // (Type 2: effects meshes, `FX_*`, laid out as the static ones.)
        0..=2 => 8,
        4 => cloth_bone_table(body)?,
        t => bail!("unexpected mesh header (type {t}) at 0x0"),
    };
    let nb = u32_at(body, count_at) as usize;
    ensure!(nb < 1024, "bone count {nb} at {count_at:#x}");
    let mut bones = Vec::with_capacity(nb);
    for i in 0..nb {
        let o = count_at + 4 + i * 0x4C;
        let mut m = [0f32; 16];
        for (k, v) in m.iter_mut().enumerate() {
            *v = f32_at(body, o + 12 + k * 4);
        }
        bones.push(MeshBone { id: u32_at(body, o), name_hash: u32_at(body, o + 8), inverse_bind: m });
    }
    let h = count_at + 4 + nb * 0x4C;
    ensure!(u32_at(body, h + 6) == CLASS_COMPILED, "compiled mesh tag missing ({:08x})", u32_at(body, h + 6));
    let stride = u32_at(body, h + 18) as usize;
    let vbytes = u32_at(body, h + 22) as usize;
    let ibytes = u32_at(body, h + 26) as usize;
    let nsub = u32_at(body, h + 38) as usize;
    let nsub2 = u32_at(body, h + 42) as usize;
    let blob = u32_at(body, h + 46) as usize;
    ensure!(stride > 0 && vbytes.is_multiple_of(stride), "stride {stride} vbytes {vbytes} h {h:#x}");
    ensure!(blob == vbytes + ibytes + 20 * (nsub + nsub2), "blob size mismatch");
    let v0 = h + 50;
    let i0 = v0 + vbytes;
    let s0 = i0 + ibytes;
    let nv = vbytes / stride;

    let mut submeshes = Vec::with_capacity(nsub);
    for k in 0..nsub {
        let o = s0 + k * 20;
        submeshes.push(Submesh {
            first_vertex: u32_at(body, o + 4),
            vertex_count: u32_at(body, o + 8),
            first_index: u32_at(body, o + 12),
            tri_count: u32_at(body, o + 16),
        });
    }
    // Bone palettes follow both submesh tables.
    let mut p = s0 + 20 * (nsub + nsub2);
    let mut palettes = Vec::with_capacity(nsub);
    // Static meshes (no bones) have no palettes.
    for _ in 0..if nb == 0 { 0 } else { nsub } {
        // 3, a flag (1 on the first palette, 0 on the rest), the bone count, the submesh index, a flag, the
        // submesh's vertex count (u16), then the bones.
        ensure!(body[p] == 3 && body[p + 1] <= 1, "palette tag missing at {p:#x}");
        let n = body[p + 2] as usize;
        palettes.push(body[p + 7..p + 7 + n].to_vec());
        p += 7 + n;
    }

    // (Over 65535 vertices the indices are u32: six meshes in the install, the merged stand-ins of the distant city,
    // `Damascus_FakeMesh_26`, `_48`, `_49` and three of Jerusalem's.)
    let tris: usize = submeshes.iter().map(|s| s.tri_count as usize).sum();
    let wide = nv > 0xFFFF && ibytes == tris * 12;
    let mut m = Mesh {
        bones,
        stride,
        positions: Vec::with_capacity(nv),
        normals: Vec::with_capacity(nv),
        tangents: Vec::with_capacity(nv),
        uvs: Vec::with_capacity(nv),
        joints: vec![[0; 4]; nv],
        weights: vec![[1.0, 0.0, 0.0, 0.0]; nv],
        indices: if wide {
            (0..ibytes / 4).map(|i| u32_at(body, i0 + i * 4)).collect()
        } else {
            (0..ibytes / 2).map(|i| u16_at(body, i0 + i * 2) as u32).collect()
        },
        submeshes,
        materials: Vec::new(),
        colors: Vec::new(),
        layers: Vec::new(),
    };
    // Trailer: u32 n, n material ids, 5 bytes.
    let n = m.submeshes.len();
    if body.len() >= 9 + 4 * n {
        let end = body.len() - 5;
        let start = end - 4 * n;
        if start >= 4 && u32_at(body, start - 4) as usize == n {
            m.materials = (0..n).map(|k| u32_at(body, start + 4 * k)).collect();
        }
    }
    for i in 0..nv {
        let o = v0 + i * stride;
        let q = |k| i16::from_le_bytes([body[o + k], body[o + k + 1]]) as f32;
        let scale = if stride == 24 { STATIC_POS_SCALE * q(6).abs().max(1.0) / 8.0 } else { POS_SCALE };
        m.positions.push([q(0) * scale, q(2) * scale, q(4) * scale]);
        if stride >= 24 {
            let n = unpack_normal(&body[o + 8..o + 12]);
            m.normals.push(n);
            let t = unpack_normal(&body[o + 12..o + 16]);
            if stride == 24 {
                // (No binormal: normal x tangent on the position w's side; bytes 16..20 are the colour.)
                m.tangents.push([t[0], t[1], t[2], if q(6) < 0.0 { -1.0 } else { 1.0 }]);
                m.colors.push([body[o + 18], body[o + 17], body[o + 16], body[o + 19]]);
                m.layers.push([body[o + 11], body[o + 15]]);
            } else {
                m.tangents.push(tangent_frame(n, t, unpack_normal(&body[o + 16..o + 20])));
            }
            m.uvs.push([q(20) / UV_SCALE, q(22) / UV_SCALE]);
        }
    }
    if stride == 32 {
        for (s, pal) in m.submeshes.iter().zip(&palettes) {
            for i in s.first_vertex as usize..(s.first_vertex + s.vertex_count) as usize {
                let o = v0 + i * stride;
                let mut j = [0u16; 4];
                let mut w = [0f32; 4];
                for k in 0..4 {
                    w[k] = body[o + 28 + k] as f32 / 255.0;
                    j[k] = if w[k] > 0.0 { *pal.get(body[o + 24 + k] as usize).unwrap_or(&0) as u16 } else { 0 };
                }
                // The bytes sum to about 255 (254-256 after rounding): normalize so the weights sum to
                // exactly 1, or skinned positions scale with their distance from the origin.
                let sum: f32 = w.iter().sum();
                if sum > 0.0 {
                    w = w.map(|x| x / sum);
                }
                m.joints[i] = j;
                m.weights[i] = w;
            }
        }
    }
    Ok(m)
}
