//! Mesh objects (class 0x415d9568).
//!
//! Body layout, reversed from the user's install:
//! - u8 1, 7 zero bytes, u32 bone count, bones x 0x4C:
//!   { u32 id, u32 class, u32 name hash, f32[16] inverse bind matrix (row-major, row-vector, translation in row 3) }
//! - 6 bytes, u32 class 0xfc9e1595 (compiled mesh), u32 size, u32 ?, u32 vertex stride,
//!   u32 vertex bytes, u32 index bytes, 8 bytes, u32 submesh count, u32 submesh count (2nd table),
//!   u32 blob size, then vertices, u16 indices, two submesh tables
//!   (20 B each: u32 4, u32 first vertex, u32 vertex count, u32 first index, u32 triangle count)
//! - per submesh bone palette: u8 3, u8 1, u8 n, u16 ?, u16 vertex count, u8[n] skeleton bone indices.
//! - ... trailer: u32 submesh count, u32 material id per submesh, 5 bytes.
//!
//! Cloth meshes (first byte 4, e.g. `UCMA_Altair_Cloth`, the long robe): u32 4, u32 1, u8 0, an embedded
//! cloth-simulation object (class 0x5755de7f, not decoded: `?`), then the usual u32 bone count, bone table
//! and compiled mesh. The bone table is found by walking back from the compiled-mesh tag to a count that
//! matches; the cloth is drawn as a plain skinned mesh.
//!
//! Static meshes (first byte 0) have the same layout with no bones and no palettes, and stride-24
//! vertices: the stride-32 layout without the palette indices and weights. The position's w is a
//! scale shared by the whole mesh (its sign varies): positions are `xyz / 32768 * |w| / 8` metres, so
//! every mesh uses the full i16 range whatever its size (Altaïr's sword: |w| = 8, 0.95 m long; a
//! Masyaf terrain tile: |w| = 3976, the tiles meeting exactly and spanning the map's 1 km; a house
//! lands on the ground it is placed on).
//!
//! Stride-32 vertex: i16x4 position (w = 32767, units of 1/2048 m), u8x4 normal, u8x4 tangent,
//! u8x4 binormal, i16x2 uv (1/2048, v down as images are stored; values run past one tile, so samplers repeat), u8x4 palette indices, u8x4 weights (sum about 255;
//! normalized to 1 on load).

use crate::{u16_at, u32_at};
use anyhow::{Result, bail, ensure};

pub const CLASS_MESH: u32 = 0x415d9568;
const CLASS_COMPILED: u32 = 0xfc9e1595;
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
    pub uvs: Vec<[f32; 2]>,
    /// Skeleton (mesh bone table) indices, already resolved through the submesh palette.
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    pub submeshes: Vec<Submesh>,
    /// Material object id per submesh (often an `_Empty` placeholder the entity overrides).
    pub materials: Vec<u32>,
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

/// Offset of the bone count in a cloth mesh (type 4): the count `n` followed by `n` bone entries
/// that end where the compiled mesh starts.
fn cloth_bone_table(body: &[u8]) -> Result<usize> {
    let tag = CLASS_COMPILED.to_le_bytes();
    let Some(c) = body.windows(4).position(|w| w == tag) else { bail!("cloth mesh: no compiled mesh tag") };
    ensure!(c >= 6, "cloth mesh: compiled mesh tag at {c:#x}");
    let h = c - 6;
    (1..1024)
        .map(|n| (n, h.checked_sub(n * 0x4C + 4)))
        .take_while(|(_, o)| o.is_some())
        .find_map(|(n, o)| o.filter(|&o| u32_at(body, o) as usize == n))
        .ok_or_else(|| anyhow::anyhow!("cloth mesh: no bone table before {h:#x}"))
}

pub fn parse_mesh(body: &[u8]) -> Result<Mesh> {
    ensure!(body.len() > 12, "mesh body too short ({} bytes)", body.len());
    let count_at = match body[0] {
        0 | 1 => 8,
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

    let mut m = Mesh {
        bones,
        stride,
        positions: Vec::with_capacity(nv),
        normals: Vec::with_capacity(nv),
        uvs: Vec::with_capacity(nv),
        joints: vec![[0; 4]; nv],
        weights: vec![[1.0, 0.0, 0.0, 0.0]; nv],
        indices: (0..ibytes / 2).map(|i| u16_at(body, i0 + i * 2) as u32).collect(),
        submeshes,
        materials: Vec::new(),
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
            m.normals.push(unpack_normal(&body[o + 8..o + 12]));
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
