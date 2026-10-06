//! Navigation meshes (`NavMeshManager`, 0b4ce0e0: one per world cell, `NavMeshManager_<cell>`). Its body starts
//! with the number of pieces, then the pieces back to back; a piece (inline: u32 id 0, u32 class 5c7b52be) is a
//! walkable polygon, triangulated:
//! - vertices: u32 count, then count x (f32 x, y, z, w) in world space (Z up, metres)
//! - triangles: u32 count, then per triangle an inline `NavMeshTriangle` (u32 id 0, u32 class cdd1ec12, then
//!   u16 index, 3 x i16 neighbouring triangle (negative: a border, `?` which kind), and a variable tail `?`)
//! - the triangles' corners: u32 count (3 per triangle), u16 vertex indices
//! - a 512-byte block `?` (a search structure, partly filled with 0xcd), the piece's bounds and more `?`
//!
//! Pieces are found by their headers and triangles by theirs; the corner list is the first u32 equal to three
//! times the triangle count followed by in-range indices. The `navmesh` tool parses every manager in the install.

use anyhow::{Result, ensure};

pub const CLASS_NAV_MESH_MANAGER: u32 = 0x0b4ce0e0;
const PIECE: [u8; 8] = [0, 0, 0, 0, 0xbe, 0x52, 0x7b, 0x5c];
const TRIANGLE: [u8; 8] = [0, 0, 0, 0, 0x12, 0xec, 0xd1, 0xcd];

/// A walkable polygon of a cell.
#[derive(Debug, Clone)]
pub struct NavPiece {
    pub vertices: Vec<[f32; 3]>,
    pub triangles: Vec<[u16; 3]>,
}

fn u32_at(d: &[u8], p: usize) -> Option<u32> {
    d.get(p..p + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap()))
}

fn find(d: &[u8], what: &[u8], from: usize, to: usize) -> Option<usize> {
    d.get(from..to)?.windows(what.len()).position(|w| w == what).map(|k| from + k)
}

/// Parse a `NavMeshManager` body (after its header).
pub fn parse_nav_mesh(d: &[u8]) -> Result<Vec<NavPiece>> {
    let declared = u32_at(d, 0).unwrap_or(0) as usize;
    let mut starts = vec![];
    let mut k = 0;
    while let Some(s) = find(d, &PIECE, k, d.len()) {
        starts.push(s);
        k = s + 8;
    }
    ensure!(starts.len() == declared, "{} pieces found, {declared} declared", starts.len());
    let mut out = vec![];
    for (i, &s) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(d.len());
        let mut p = s + 8;
        let nv = u32_at(d, p).unwrap_or(u32::MAX) as usize;
        ensure!(p + 4 + nv * 16 <= end, "piece {i} at {s:#x}: {nv} vertices overrun it");
        p += 4;
        let vertices = (0..nv)
            .map(|j| {
                let f = |o: usize| f32::from_bits(u32_at(d, p + 16 * j + o).unwrap_or(0));
                [f(0), f(4), f(8)]
            })
            .collect::<Vec<_>>();
        p += 16 * nv;
        let nt = u32_at(d, p).unwrap_or(u32::MAX) as usize;
        p += 4;
        let mut q = p;
        for t in 0..nt {
            let Some(at) = find(d, &TRIANGLE, q, end) else {
                anyhow::bail!("piece {i} at {s:#x}: triangle {t} of {nt} missing");
            };
            q = at + 8;
        }
        // The corners: the first count of three per triangle followed by indices into the vertices.
        let n = 3 * nt;
        let corners = (q..end.saturating_sub(4)).find_map(|c| {
            if u32_at(d, c) != Some(n as u32) || c + 4 + 2 * n > end {
                return None;
            }
            let idx: Vec<u16> = (0..n).map(|k| u16::from_le_bytes([d[c + 4 + 2 * k], d[c + 5 + 2 * k]])).collect();
            idx.iter().all(|&v| (v as usize) < nv).then_some(idx)
        });
        let Some(corners) = corners else { anyhow::bail!("piece {i} at {s:#x}: no corner list") };
        let triangles = corners.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
        out.push(NavPiece { vertices, triangles });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_piece() {
        let mut d = 1u32.to_le_bytes().to_vec();
        d.extend(PIECE);
        d.extend(3u32.to_le_bytes());
        for v in [[0.0f32, 0.0, 1.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]] {
            for x in v.iter().chain(&[0.0]) {
                d.extend(x.to_le_bytes());
            }
        }
        d.extend(1u32.to_le_bytes());
        d.extend(TRIANGLE);
        d.extend([0u8, 0, 0xe4, 0xff, 0xe4, 0xff, 0xe4, 0xff, 0, 0, 0, 0]);
        d.extend(3u32.to_le_bytes());
        for i in [0u16, 2, 1] {
            d.extend(i.to_le_bytes());
        }
        d.extend([0xcd; 16]);
        let p = parse_nav_mesh(&d).unwrap();
        assert_eq!(p[0].triangles, vec![[0, 2, 1]]);
        assert_eq!(p[0].vertices[1], [1.0, 0.0, 1.0]);
        assert!(parse_nav_mesh(&d[..30]).is_err());
    }
}
