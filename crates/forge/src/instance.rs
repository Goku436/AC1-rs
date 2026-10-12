//! A placed entity's per-vertex baked ambient occlusion (`MeshInstanceData`, 536e963b).
//!
//! An entity's visual (or its `LODSelectorInstance`) carries one `MeshInstanceData` per mesh it draws:
//! - the marker (u32 class), u8 1, u32 the mesh's id, ...; then per submesh a `SubMeshInstanceData` (u8 2, u32 id, ...)
//!   with a `SubMeshAOData` (u32 count, u8 per vertex: the source bytes);
//! - then a `CompiledMeshInstance` (4368101b): u32 size, then `size` bytes: u32 17, u32 18 (16 on some kiosk parts; 2 with
//!   n = 0 on placed characters), u32 n * 4, u32 n * 4, and n D3DCOLORs (B, G, R, A), one per vertex of the whole mesh: the
//!   same value in B, G and R, A 255. This is the stream the GPU reads.
//!
//! What the shaders do with it (the DX9 shaders inside the `MaterialTemplate`s, e.g. `GEN_Standard`): the vertex shader
//! reads the stream's first channel as `TEXCOORD4`, maps it to `saturate(max(v * g_AOParams.x + g_AOParams.y,
//! g_AOParams.z))` and the pixel shader multiplies only the ambient (the ambient cube) by it, not the sun. The values
//! read as how open a vertex is (255 open, 0 closed in): a rope in the air is near 255, the ground by a wall near 0.
//! `g_AOParams` is set by the engine at run time (not found yet: `?`).
//!
//! As Banned445's AC1-Movement-Rewritten (MIT) finds the stream (its `city/decode.rs`: the colours after each
//! `MeshInstanceData`); the meaning is from the shaders. `examples/aocheck` reads every entity of a forge.

pub const CLASS_MESH_INSTANCE_DATA: u32 = 0x536e_963b;
pub const CLASS_COMPILED_MESH_INSTANCE: u32 = 0x4368_101b;

fn u32_at(d: &[u8], p: usize) -> Option<u32> {
    d.get(p..p + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// One mesh's baked occlusion in an entity: a byte per vertex of the mesh (0 closed in, 255 open).
#[derive(Debug, Clone, PartialEq)]
pub struct MeshAo {
    pub mesh: u32,
    pub ao: Vec<u8>,
}

/// Why an entity's instance data was not read (counted by `aocheck`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AoError {
    /// A `CompiledMeshInstance` whose header is not `17, 18 (or 16), n * 4, n * 4` with `n * 4` bytes after it.
    BadHeader { at: usize },
    /// Its colours are not grey with A 255.
    NotGrey { at: usize },
}

/// The occlusion of every mesh an entity's body (its bytes up to the next entity) draws, in the order they come.
/// A `MeshInstanceData` with no `CompiledMeshInstance` before the next one is left out.
pub fn mesh_ao(body: &[u8]) -> Result<Vec<MeshAo>, AoError> {
    let mark = |class: u32| -> Vec<usize> { (0..body.len().saturating_sub(3)).filter(|&p| u32_at(body, p) == Some(class)).collect() };
    let datas = mark(CLASS_MESH_INSTANCE_DATA);
    let compiled = mark(CLASS_COMPILED_MESH_INSTANCE);
    let mut out = vec![];
    for (k, &p) in datas.iter().enumerate() {
        let next = datas.get(k + 1).copied().unwrap_or(body.len());
        let Some(&c) = compiled.iter().find(|&&c| c > p && c < next) else { continue };
        let Some(mesh) = u32_at(body, p + 5) else { continue };
        let size = u32_at(body, c + 4).ok_or(AoError::BadHeader { at: c })? as usize;
        let blob = body.get(c + 8..c + 8 + size).ok_or(AoError::BadHeader { at: c })?;
        let (a, b, n1, n2) = (u32_at(blob, 0), u32_at(blob, 4), u32_at(blob, 8), u32_at(blob, 12));
        if a != Some(17) || n1 != n2 || n1.map(|n| n as usize + 16) != Some(blob.len()) || !blob.len().is_multiple_of(4) {
            return Err(AoError::BadHeader { at: c });
        }
        // (An empty one, `17, 2, 0, 0`: placed characters.)
        if n1 == Some(0) {
            continue;
        }
        if !matches!(b, Some(16 | 18)) {
            return Err(AoError::BadHeader { at: c });
        }
        let colours = &blob[16..];
        if colours.chunks_exact(4).any(|c| c[0] != c[1] || c[1] != c[2] || c[3] != 255) {
            return Err(AoError::NotGrey { at: c });
        }
        out.push(MeshAo { mesh, ao: colours.chunks_exact(4).map(|c| c[0]).collect() });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blob(mesh: u32, ao: &[u8]) -> Vec<u8> {
        let mut b = CLASS_MESH_INSTANCE_DATA.to_le_bytes().to_vec();
        b.push(1);
        b.extend(mesh.to_le_bytes());
        b.extend([0u8; 9]);
        b.extend(CLASS_COMPILED_MESH_INSTANCE.to_le_bytes());
        let n = ao.len() as u32 * 4;
        b.extend((16 + n).to_le_bytes());
        for v in [17, 18, n, n] {
            b.extend(v.to_le_bytes());
        }
        for &v in ao {
            b.extend([v, v, v, 255]);
        }
        b
    }

    #[test]
    fn reads_each_mesh_and_its_bytes() {
        let mut body = vec![7u8; 5];
        body.extend(blob(0x1234, &[0, 128, 255]));
        body.extend(blob(0x99, &[5]));
        let got = mesh_ao(&body).unwrap();
        assert_eq!(got, vec![MeshAo { mesh: 0x1234, ao: vec![0, 128, 255] }, MeshAo { mesh: 0x99, ao: vec![5] }]);
    }

    #[test]
    fn bad_data_is_an_error_not_a_panic() {
        let mut body = blob(1, &[1, 2]);
        let n = body.len();
        body[n - 2] = 3; // not grey
        assert!(matches!(mesh_ao(&body), Err(AoError::NotGrey { .. })));
        let short = &blob(1, &[1, 2])[..30];
        assert!(matches!(mesh_ao(short), Err(AoError::BadHeader { .. })));
    }
}
