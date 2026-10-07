//! Guidance: AC1's authored climbing markup. An entity that can be climbed, hung from, walked along or jumped to
//! carries a `GuidanceSystem` (class 55af1c3e) inside its body: the edges the player may grab, each between two
//! vertices with the normals of the two faces that meet there and a subtype (ledge grab, beam, ladder, pole, rope,
//! surface, kiosk). Format from Banned445's AC1-Movement-Rewritten (MIT), checked against the install here.
//!
//! Layout (little endian, in the entity body's own frame, game space Z up):
//! - u32 object id, u32 class 55af1c3e, u8 active, u32 edge count (14 bits used)
//! - per edge, 53 bytes: u32 id, u32 class 344fa659 (`GuidanceObject`), u8 enabled, u32 subtype, u32 vertex a,
//!   u32 vertex b, f32x4 normal a, f32x4 normal b (xyz quantized to 1/511 by the game; w `?`)
//! - u32 coordinate count, then that many u16: x, y, z per vertex, each `u16 * 0.005 - 163.84` m
//! - an `EdgeFilter` (class 509c4552): 7 u8 flags, 3 f32 (`?`, corner angle, max angle in radians), u8 check world
//!   orientation; then a `Partitioner` pointer (not read).
//!
//! Of the two normals, the one facing up is the top face's and the other the wall's: the wall normal is the way
//! out from a ledge.

use anyhow::{Result, bail, ensure};

pub const CLASS_GUIDANCE_SYSTEM: u32 = 0x55af_1c3e;
pub const CLASS_GUIDANCE_OBJECT: u32 = 0x344f_a659;
pub const CLASS_EDGE_FILTER: u32 = 0x509c_4552;
/// Bytes per edge record.
const EDGE_SIZE: usize = 53;

/// What an edge is for (`GuidanceObjectSubType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubType {
    None,
    LedgeGrab,
    Beam,
    Ladder,
    Pole,
    Rope,
    Surface,
    Quadruped,
    Kiosk,
}

impl SubType {
    fn from_u32(v: u32) -> Option<Self> {
        Some(match v {
            0 => Self::None,
            1 => Self::LedgeGrab,
            2 => Self::Beam,
            3 => Self::Ladder,
            4 => Self::Pole,
            5 => Self::Rope,
            6 => Self::Surface,
            7 => Self::Quadruped,
            8 => Self::Kiosk,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub enabled: bool,
    pub subtype: SubType,
    /// Ends, game space (the entity's frame), m.
    pub a: [f32; 3],
    pub b: [f32; 3],
    /// Normals of the two faces meeting at the edge, as stored.
    pub normals: [[f32; 3]; 2],
}

#[derive(Clone, Debug)]
pub struct Guidance {
    pub active: bool,
    pub edges: Vec<Edge>,
    /// The edge filter's max angle (radians) and whether it checks world orientation.
    pub max_angle: f32,
    pub check_orientation: bool,
}

fn u32_at(d: &[u8], o: usize) -> Result<u32> {
    let b = d.get(o..o + 4).ok_or_else(|| anyhow::anyhow!("guidance truncated at {o:#x}"))?;
    Ok(u32::from_le_bytes(b.try_into().expect("4 bytes")))
}

fn f32_at(d: &[u8], o: usize) -> Result<f32> {
    let v = f32::from_bits(u32_at(d, o)?);
    ensure!(v.is_finite(), "non-finite float at {o:#x}");
    Ok(v)
}

/// Parse a `GuidanceSystem` starting at `o` (its object id) in `d`.
pub fn parse_at(d: &[u8], o: usize) -> Result<Guidance> {
    ensure!(u32_at(d, o + 4)? == CLASS_GUIDANCE_SYSTEM, "no GuidanceSystem at {o:#x}");
    let active = *d.get(o + 8).ok_or_else(|| anyhow::anyhow!("truncated at {:#x}", o + 8))? != 0;
    let count = u32_at(d, o + 9)? as usize;
    ensure!(count <= 0x3fff && o + 13 + count * EDGE_SIZE <= d.len(), "bad edge count {count} at {:#x}", o + 9);
    let mut p = o + 13;
    let mut raw = Vec::with_capacity(count);
    for _ in 0..count {
        ensure!(u32_at(d, p + 4)? == CLASS_GUIDANCE_OBJECT, "no GuidanceObject at {p:#x}");
        let enabled = d[p + 8] != 0;
        let Some(subtype) = SubType::from_u32(u32_at(d, p + 9)?) else { bail!("unknown guidance subtype at {:#x}", p + 9) };
        let (va, vb) = (u32_at(d, p + 13)? as usize, u32_at(d, p + 17)? as usize);
        let n = |q: usize| -> Result<[f32; 3]> {
            let mut v = [0.0; 3];
            for (k, c) in v.iter_mut().enumerate() {
                *c = (f32_at(d, q + 4 * k)?.clamp(-1.0, 1.0) * 511.0).trunc() / 511.0;
            }
            Ok(v)
        };
        raw.push((enabled, subtype, va, vb, [n(p + 21)?, n(p + 37)?]));
        p += EDGE_SIZE;
    }
    let coords = u32_at(d, p)? as usize;
    ensure!(coords.is_multiple_of(3) && p + 4 + coords * 2 <= d.len(), "bad coordinate count {coords} at {p:#x}");
    let verts: Vec<[f32; 3]> = (0..coords / 3)
        .map(|i| {
            let c = |k: usize| u16::from_le_bytes([d[p + 4 + (i * 3 + k) * 2], d[p + 5 + (i * 3 + k) * 2]]) as f32 * 0.005 - 163.84;
            [c(0), c(1), c(2)]
        })
        .collect();
    p += 4 + coords * 2;
    let mut edges = Vec::with_capacity(raw.len());
    for (enabled, subtype, va, vb, normals) in raw {
        let (Some(a), Some(b)) = (verts.get(va), verts.get(vb)) else { bail!("guidance edge vertex out of range ({va}, {vb})") };
        edges.push(Edge { enabled, subtype, a: *a, b: *b, normals });
    }
    // The edge filter (its max angle, and whether to check world orientation), when it follows.
    let (mut max_angle, mut check_orientation) = (std::f32::consts::PI, false);
    if u32_at(d, p + 4).ok() == Some(CLASS_EDGE_FILTER) {
        max_angle = f32_at(d, p + 8 + 7 + 8)?;
        check_orientation = d.get(p + 8 + 7 + 12).is_some_and(|b| *b != 0);
    }
    Ok(Guidance { active, edges, max_angle, check_orientation })
}

/// The `GuidanceSystem`s inside an entity's body (usually one; an animated gate has two), each parsed.
pub fn find_in_entity(body: &[u8]) -> Vec<Result<Guidance>> {
    let marker = CLASS_GUIDANCE_SYSTEM.to_le_bytes();
    body.windows(4).enumerate().filter(|(o, w)| *w == marker && *o >= 4).map(|(o, _)| parse_at(body, o - 4)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn system(edges: &[(u8, u32, u32, u32)], verts: &[[u16; 3]]) -> Vec<u8> {
        let mut d = vec![];
        d.extend(7u32.to_le_bytes());
        d.extend(CLASS_GUIDANCE_SYSTEM.to_le_bytes());
        d.push(1);
        d.extend((edges.len() as u32).to_le_bytes());
        for &(en, sub, a, b) in edges {
            d.extend(9u32.to_le_bytes());
            d.extend(CLASS_GUIDANCE_OBJECT.to_le_bytes());
            d.push(en);
            d.extend(sub.to_le_bytes());
            d.extend(a.to_le_bytes());
            d.extend(b.to_le_bytes());
            for v in [0.0f32, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0] {
                d.extend(v.to_le_bytes());
            }
        }
        d.extend(((verts.len() * 3) as u32).to_le_bytes());
        for v in verts {
            for c in v {
                d.extend(c.to_le_bytes());
            }
        }
        d
    }

    #[test]
    fn reads_edges_and_vertices() {
        // 163.84 / 0.005 = 32768: the coordinate origin.
        let d = system(&[(1, 1, 0, 1), (0, 3, 1, 0)], &[[32768, 32768, 32768], [32968, 32768, 33168]]);
        let g = parse_at(&d, 0).expect("parses");
        assert!(g.active);
        assert_eq!(g.edges.len(), 2);
        assert_eq!(g.edges[0].subtype, SubType::LedgeGrab);
        assert!(!g.edges[1].enabled);
        assert_eq!(g.edges[1].subtype, SubType::Ladder);
        assert!((g.edges[0].b[0] - 1.0).abs() < 1e-3 && (g.edges[0].b[2] - 2.0).abs() < 1e-3);
        assert_eq!(g.edges[0].normals[0], [0.0, 0.0, 1.0]);
    }

    #[test]
    fn rejects_bad_data_without_panicking() {
        let mut d = system(&[(1, 1, 0, 5)], &[[0, 0, 0]]);
        assert!(parse_at(&d, 0).is_err());
        d.truncate(20);
        assert!(parse_at(&d, 0).is_err());
        assert!(find_in_entity(&[1, 2, 3]).is_empty());
    }
}
