//! Cloth: what AC1 simulates Altaïr's robe tails from, and the wind that blows them.
//!
//! Class names are the CRC32s of names in the game's executable (docs/NOTES.md "Class names"). The layouts were
//! found by Banned445's AC1-Movement-Rewritten (MIT; its `src/cloth.rs`, `src/wind.rs`) and checked here against
//! the data. Offsets below are from the class id named.
//!
//! - The soft mesh (`DynamicMesh` `UCMA_Altair_Cloth_SoftBody` links it; the mesh `UCMA_Altair_Cloth` holds it):
//!   the mesh body starts `u32 4, u32 1, u8 0`, then an embedded `SubMesh` (u32 id, u32 class 0x5755de7f, u32 ?
//!   (0)) whose arrays follow, each a u32 count and its elements: u16 corner indices (3 a triangle), u32 ?
//!   (a vertex remapping), 7-byte records (none in AC1's robe; any other count is refused), u32 ? per triangle (1),
//!   u8 per triangle (bit k: the edge from corner k to k + 1 is a distance constraint), vec3 positions (mesh space,
//!   metres, Z up), vec3 normals, vec3 tangents, vec3 binormals, u32 uvs, u8x4 flags per vertex (byte 1: the
//!   collision radius between the cloth's minimum and maximum, /255; byte 2: the pull toward the skinned pose
//!   between its minimum and maximum, /255, 255 pinned to the body; bytes 0 and 3 `?`), u32 ? per vertex (0),
//!   f32x4 skin weights, u8x4 mesh-bone indices (255 none). The compiled render mesh follows (`mesh.rs`), its
//!   vertices in another order.
//! - The character entity's `Cloth` (0xe33044ba): +24 f32 upward damping (the damping of a vertex moving up), +28
//!   f32 damping, +32 f32 `?`, +36 / +40 f32 collision radius min / max, +44 an embedded `WindForce` (u32 id, u32
//!   class 0xe7f9a244, u8 enabled, f32 scale, u16 `?`, f32 the share of the force along the cloth's surface (the
//!   part along its normal is kept whole), f32 random amplitude), +79 u8 constraint iterations per step, +257 f32
//!   x 7: pull from motion (per m/s of speed, per unit of turn), the pull's decay per second, pull min / max,
//!   upward damping per m/s the character rises, its cap; +285 u8 `?`, +286 u32 the `ClothActionSettings` id.
//!   `ClothComponent` (0x8321699e): +13 f32 gravity (m/s², along up).
//! - `ClothRagdoll` (0xce38bb17, in the same entity): +4 u32 the ragdoll (`RagdollNew`, `humanragdoll_forcloth`),
//!   +8 u32 n and n bone name hashes (the skeleton side of the ragdoll's mapper), u32 count, then count
//!   `ClothRagdollCapsule` (0xd862b39b) of 52 bytes: u32 id 0, u32 class, u32 mode (0 pushes a vertex out on the
//!   side its skinned position is, unless the threshold is over the vertex's pull; 1 straight out; 2 off), f32
//!   threshold, vec4 end A, vec4 end B, f32 radius, in the ragdoll body's frame.
//! - `ClothActionSettings` (0x52e5aa85): u32 count, then count `ClothActionEntry` (0x4ea3953b) of 16 bytes: u32
//!   id 0, u32 class, u32 action key, f32 strength (how far the motion pull is let off while that action plays).
//! - `Wind` (0x8139fbc0, a component of a city's `Wind_<City>_001` entity): +4 u8 active, +5 f32 strength, +9 u8
//!   noise, +10 f32 x 4 noise percentages (direction, direction speed, magnitude floor, magnitude speed), +26 u32
//!   region count (0), +30 u32 mode (0), +34 f32 x 3 oscillation (0), +46 u32 kind (0), +90 f32 x 16 a local
//!   matrix; it blows along the X axis of the entity's placement times that matrix.

use crate::hkx::Packfile;
use crate::read::{f32_at, find_one, u8_at, u32_at, vec3_at, vec4_at};
use anyhow::{Context, Result, bail, ensure};

pub const CLASS_DYNAMIC_MESH: u32 = 0xf4bbbb9e;
pub const CLASS_CLOTH_ACTION_SETTINGS: u32 = 0x52e5aa85;
pub const CLASS_WIND: u32 = 0x8139fbc0;
const CLASS_SUBMESH: u32 = 0x5755de7f;
const CLASS_CLOTH: u32 = 0xe33044ba;
const CLASS_CLOTH_COMPONENT: u32 = 0x8321699e;
const CLASS_CLOTH_RAGDOLL: u32 = 0xce38bb17;
const CLASS_CLOTH_RAGDOLL_CAPSULE: u32 = 0xd862b39b;
pub(crate) const CLASS_WIND_FORCE: u32 = 0xe7f9a244;
const CLASS_CLOTH_ACTION_ENTRY: u32 = 0x4ea3953b;

/// The soft mesh's own vertices (the `SubMesh`'s arrays, see the module docs).
#[derive(Debug, Clone)]
pub struct SoftMesh {
    pub positions: Vec<[f32; 3]>,
    pub weights: Vec<[f32; 4]>,
    /// Mesh-bone indices (255: none).
    pub bones: Vec<[u8; 4]>,
    pub flags: Vec<[u8; 4]>,
    pub triangles: Vec<[u16; 3]>,
    /// Per triangle: bit k, its edge from corner k to k + 1 is a distance constraint.
    pub edge_masks: Vec<u8>,
}

impl SoftMesh {
    /// The constrained edges, each once, in the triangles' order.
    pub fn edges(&self) -> Vec<[usize; 2]> {
        let mut seen = std::collections::HashSet::new();
        let mut out = vec![];
        for (t, &mask) in self.triangles.iter().zip(&self.edge_masks) {
            for k in 0..3 {
                let (a, b) = (t[k] as usize, t[(k + 1) % 3] as usize);
                if mask & (1 << k) != 0 && seen.insert((a.min(b), a.max(b))) {
                    out.push([a, b]);
                }
            }
        }
        out
    }

    /// Where each of its vertices is in a compiled mesh's vertices (the compiler reorders them): the one at the same
    /// place, within the compiled positions' rounding (1/2048 m an axis); each used once.
    pub fn order_in(&self, compiled: &[[f32; 3]]) -> Result<Vec<usize>> {
        ensure!(compiled.len() == self.positions.len(), "soft mesh has {} vertices, the compiled mesh {}", self.positions.len(), compiled.len());
        let mut taken = vec![false; compiled.len()];
        let mut out = Vec::with_capacity(self.positions.len());
        for (i, p) in self.positions.iter().enumerate() {
            let d2 = |q: &[f32; 3]| (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f32>();
            let (j, d) = compiled.iter().enumerate().map(|(j, q)| (j, d2(q))).min_by(|a, b| a.1.total_cmp(&b.1)).context("empty mesh")?;
            ensure!(d.sqrt() <= 2.0 / 2048.0, "soft vertex {i} is {:.4} m from any compiled vertex", d.sqrt());
            ensure!(!taken[j], "compiled vertex {j} matches two soft vertices");
            taken[j] = true;
            out.push(j);
        }
        Ok(out)
    }
}

/// The soft mesh of a cloth mesh's body (`mesh.rs`'s body, first byte 4).
pub fn soft_mesh(body: &[u8]) -> Result<SoftMesh> {
    ensure!(u32_at(body, 0)? == 4, "not a cloth mesh (kind {} at 0)", u32_at(body, 0)?);
    ensure!(u32_at(body, 4)? == 1, "{} submeshes at 4 (one is read)", u32_at(body, 4)?);
    ensure!(u32_at(body, 13)? == CLASS_SUBMESH, "no SubMesh at 13");
    let mut p = 21;
    let mut array = |width: usize| -> Result<(usize, usize)> {
        let n = u32_at(body, p)? as usize;
        let start = p + 4;
        let end = n.checked_mul(width).and_then(|b| b.checked_add(start)).with_context(|| format!("array at {p:#x} too long"))?;
        ensure!(end <= body.len(), "array of {n} at {p:#x} runs past the end");
        p = end;
        Ok((start, n))
    };
    let (ip, ni) = array(2)?;
    array(4)?;
    let (_, colours) = array(7)?;
    ensure!(colours == 0, "{colours} 7-byte records (not read)");
    array(4)?;
    let (mp, nm) = array(1)?;
    let (pp, n) = array(12)?;
    array(12)?;
    array(12)?;
    array(12)?;
    array(4)?;
    let (fp, nf) = array(4)?;
    array(4)?;
    let (wp, nw) = array(16)?;
    let (bp, nb) = array(1)?;
    ensure!(ni % 3 == 0 && ni / 3 == nm, "{ni} corners for {nm} triangles");
    ensure!(nf == n && nw == n && nb == 4 * n, "per-vertex arrays disagree ({n} positions, {nf} flags, {nw} weights, {nb} bones)");
    let triangles = (0..nm)
        .map(|t| {
            let c = |k: usize| -> Result<u16> {
                let o = ip + 6 * t + 2 * k;
                let v = u16::from_le_bytes([u8_at(body, o)?, u8_at(body, o + 1)?]);
                ensure!((v as usize) < n, "corner {v} at {o:#x} past {n} vertices");
                Ok(v)
            };
            Ok([c(0)?, c(1)?, c(2)?])
        })
        .collect::<Result<Vec<_>>>()?;
    let mut m = SoftMesh { positions: vec![], weights: vec![], bones: vec![], flags: vec![], triangles, edge_masks: body[mp..mp + nm].to_vec() };
    for i in 0..n {
        m.positions.push(vec3_at(body, pp + 12 * i)?);
        let w = vec4_at(body, wp + 16 * i)?;
        let b: [u8; 4] = std::array::from_fn(|k| body[bp + 4 * i + k]);
        ensure!(w.iter().all(|w| (0.0..=1.0).contains(w)), "weights {w:?} at {:#x}", wp + 16 * i);
        ensure!((w.iter().sum::<f32>() - 1.0).abs() < 1e-3, "weights at {:#x} sum to {}", wp + 16 * i, w.iter().sum::<f32>());
        ensure!((0..4).all(|k| b[k] != 255 || w[k] == 0.0), "weight on no bone at {:#x}", bp + 4 * i);
        m.weights.push(w);
        m.bones.push(b);
        m.flags.push(std::array::from_fn(|k| body[fp + 4 * i + k]));
    }
    Ok(m)
}

/// A force sampled from the global sources (the wind) for a cloth or a skeleton (`WindForce`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindForce {
    pub scale: f32,
    /// The share of the force along the surface (the part along its normal is kept whole).
    pub tangent: f32,
    /// Random amplitude: each vertex's force times 1 ± up to this.
    pub random: f32,
}

/// An embedded `WindForce` whose class id is at `q`: None when disabled.
pub(crate) fn wind_force(d: &[u8], q: usize) -> Result<Option<WindForce>> {
    ensure!(u32_at(d, q)? == CLASS_WIND_FORCE, "no WindForce at {q:#x}");
    let on = match u8_at(d, q + 4)? {
        0 => false,
        1 => true,
        x => bail!("WindForce enabled flag {x} at {:#x}", q + 4),
    };
    Ok(on.then_some(WindForce { scale: f32_at(d, q + 5)?, tangent: f32_at(d, q + 11)?, random: f32_at(d, q + 15)? }))
}

/// A capsule of the cloth's ragdoll as the entity lists it (in its ragdoll body's frame).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RagdollCapsule {
    pub mode: u32,
    pub threshold: f32,
    pub a: [f32; 3],
    pub b: [f32; 3],
    pub radius: f32,
}

/// The robe's settings from the character entity (see the module docs).
#[derive(Debug, Clone)]
pub struct ClothSettings {
    pub damping: f32,
    pub upward_damping: f32,
    /// Collision radius of a vertex: min, max (by its flag byte 1).
    pub radius: [f32; 2],
    pub iterations: u32,
    pub force: Option<WindForce>,
    /// Pull toward the skinned pose per m/s of the character's speed, and per unit of its turn (the unit X axis's
    /// change per second).
    pub pull_motion: [f32; 2],
    /// How fast that pull may fall (per second).
    pub pull_decay: f32,
    /// A vertex's own pull: min, max (by its flag byte 2).
    pub pull: [f32; 2],
    /// Upward damping per m/s the character rises (added), and its cap.
    pub upward_motion: [f32; 2],
    pub gravity: f32,
    pub action_settings: u32,
    pub ragdoll: u32,
    /// The ragdoll mapper's skeleton side: bone name hashes.
    pub bones: Vec<u32>,
    pub capsules: Vec<RagdollCapsule>,
}

/// The `Cloth`, `ClothComponent` and `ClothRagdoll` of a character entity's body.
pub fn cloth_settings(entity: &[u8]) -> Result<ClothSettings> {
    let c = find_one(entity, CLASS_CLOTH, "Cloth")?;
    let cc = find_one(entity, CLASS_CLOTH_COMPONENT, "ClothComponent")?;
    let f = |o: usize| f32_at(entity, c + o);
    let unit = |v: f32, o: usize| -> Result<f32> {
        ensure!((0.0..=1.0).contains(&v), "{v} at {:#x} outside 0..1", c + o);
        Ok(v)
    };
    let iterations = u8_at(entity, c + 79)? as u32;
    ensure!((1..=8).contains(&iterations), "{iterations} iterations at {:#x}", c + 79);
    let k = find_one(entity, CLASS_CLOTH_RAGDOLL, "ClothRagdoll")?;
    let n = u32_at(entity, k + 8)? as usize;
    ensure!(n <= 255, "{n} ragdoll bones at {:#x}", k + 8);
    let bones = (0..n).map(|i| u32_at(entity, k + 12 + 4 * i)).collect::<Result<Vec<_>>>()?;
    let cp = k + 12 + 4 * n;
    let count = u32_at(entity, cp)? as usize;
    ensure!(count <= 255, "{count} capsules at {cp:#x}");
    let capsules = (0..count)
        .map(|i| {
            let r = cp + 4 + 52 * i;
            ensure!(u32_at(entity, r + 4)? == CLASS_CLOTH_RAGDOLL_CAPSULE, "no ClothRagdollCapsule at {:#x}", r + 4);
            let mode = u32_at(entity, r + 8)?;
            ensure!(mode <= 2, "capsule mode {mode} at {:#x}", r + 8);
            Ok(RagdollCapsule {
                mode,
                threshold: f32_at(entity, r + 12)?,
                a: vec3_at(entity, r + 16)?,
                b: vec3_at(entity, r + 32)?,
                radius: f32_at(entity, r + 48)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ClothSettings {
        upward_damping: unit(f(24)?, 24)?,
        damping: unit(f(28)?, 28)?,
        radius: [f(36)?, f(40)?],
        iterations,
        force: wind_force(entity, c + 48)?,
        pull_motion: [f(257)?, f(261)?],
        pull_decay: f(265)?,
        pull: [unit(f(269)?, 269)?, unit(f(273)?, 273)?],
        upward_motion: [f(277)?, f(281)?],
        gravity: f32_at(entity, cc + 13)?,
        action_settings: u32_at(entity, c + 286)?,
        ragdoll: u32_at(entity, k + 4)?,
        bones,
        capsules,
    })
}

/// A capsule of the cloth in the frame of one skeleton bone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Collider {
    /// The bone's name hash.
    pub bone: u32,
    pub a: [f32; 3],
    pub b: [f32; 3],
    pub radius: f32,
    pub mode: u32,
    pub threshold: f32,
}

/// The settings' capsules moved onto skeleton bones through the ragdoll's `hkSkeletonMapper` (`ragdoll` is the
/// `RagdollNew` object's body): each capsule from its ragdoll bone's frame into the mapped skeleton bone's. Capsules
/// with mode 2 (off) are left out.
pub fn colliders(settings: &ClothSettings, ragdoll: &[u8]) -> Result<Vec<Collider>> {
    let p = Packfile::find(ragdoll)?;
    // Per capsule: (its bone in `settings.bones`, rotation, translation, scale).
    type Mapped = (usize, [f32; 4], [f32; 3], [f32; 3]);
    let mut mapped: Vec<Option<Mapped>> = vec![None; settings.capsules.len()];
    let mut found = false;
    for m in p.all("hkSkeletonMapper").collect::<Vec<_>>() {
        let data = p.field(m, "hkSkeletonMapper", "mapping")?;
        let count = |member: &str| -> Result<usize> {
            let sk = p.ptr(p.field(data, "hkSkeletonMapperData", member)?).context("mapper without a skeleton")?;
            Ok(p.array(p.field(sk, "hkSkeleton", "bones")?)?.1)
        };
        if count("skeletonA")? != settings.bones.len() || count("skeletonB")? != settings.capsules.len() {
            continue;
        }
        found = true;
        let (at, n) = p.array(p.field(data, "hkSkeletonMapperData", "simpleMappings")?)?;
        let Some(at) = at else { continue };
        // (64 bytes a mapping: two i16 bones, padding, the transform at 16.)
        for i in 0..n {
            let r = (at.0, at.1 + 64 * i);
            let a = p.i16(p.field(r, "hkSkeletonMapperDataSimpleMapping", "boneA")?)? as usize;
            let b = p.i16(p.field(r, "hkSkeletonMapperDataSimpleMapping", "boneB")?)? as usize;
            let t = p.field(r, "hkSkeletonMapperDataSimpleMapping", "aFromBTransform")?;
            // An `hkQsTransform`: translation, rotation (x, y, z, w), scale, a vec4 each.
            let (tr, q, s) = (p.vec4(t)?, p.vec4((t.0, t.1 + 16))?, p.vec4((t.0, t.1 + 32))?);
            ensure!(a < settings.bones.len() && b < settings.capsules.len(), "mapping {i} pairs bone {a} with capsule {b}");
            let len = q.iter().map(|v| v * v).sum::<f32>().sqrt();
            ensure!((len - 1.0).abs() < 0.01 && tr.iter().chain(&s).all(|v| v.is_finite()), "mapping {i}: not a transform");
            ensure!(mapped[b].is_none(), "capsule {b} mapped twice");
            mapped[b] = Some((a, q.map(|v| v / len), [tr[0], tr[1], tr[2]], [s[0], s[1], s[2]]));
        }
    }
    ensure!(found, "no hkSkeletonMapper from {} bones to {} capsules", settings.bones.len(), settings.capsules.len());
    let mut out = vec![];
    for (i, c) in settings.capsules.iter().enumerate().filter(|(_, c)| c.mode != 2) {
        let (bone, q, t, s) = mapped[i].with_context(|| format!("capsule {i} has no bone"))?;
        let place = |v: [f32; 3]| add(rotate(q, [v[0] * s[0], v[1] * s[1], v[2] * s[2]]), t);
        let scale = s.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        out.push(Collider { bone: settings.bones[bone], a: place(c.a), b: place(c.b), radius: c.radius * scale, mode: c.mode, threshold: c.threshold });
    }
    Ok(out)
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// `v` turned by the unit quaternion `q` (x, y, z, w).
fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let (u, w) = ([q[0], q[1], q[2]], q[3]);
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let t = cross(u, v).map(|x| 2.0 * x);
    let ut = cross(u, t);
    [v[0] + w * t[0] + ut[0], v[1] + w * t[1] + ut[1], v[2] + w * t[2] + ut[2]]
}

/// A `ClothActionSettings` body: (action key, strength) pairs.
pub fn action_settings(body: &[u8]) -> Result<Vec<(u32, f32)>> {
    let n = u32_at(body, 0)? as usize;
    ensure!(body.len() == 4 + 16 * n, "{n} entries in {} bytes", body.len());
    (0..n)
        .map(|i| {
            let p = 4 + 16 * i;
            ensure!(u32_at(body, p + 4)? == CLASS_CLOTH_ACTION_ENTRY, "no ClothActionEntry at {:#x}", p + 4);
            let s = f32_at(body, p + 12)?;
            ensure!((0.0..=1.0).contains(&s), "strength {s} at {:#x}", p + 12);
            Ok((u32_at(body, p + 8)?, s))
        })
        .collect()
}

/// A city's wind (`Wind`), placed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wind {
    pub strength: f32,
    pub noise: bool,
    /// Noise percentages: direction, direction speed, magnitude floor, magnitude speed.
    pub percent: [f32; 4],
    /// Where it blows (game world, Z up, unit).
    pub direction: [f32; 3],
}

/// The `Wind` component of an entity's body, if it has one (the body starts with the entity's placement, a
/// column-major 4x4 matrix). Layouts other than AC1's cities' (regions, oscillation, other modes) are refused.
pub fn wind(entity: &[u8]) -> Result<Option<Wind>> {
    let at = crate::read::find_u32(entity, CLASS_WIND);
    let Some(&p) = at.iter().find(|&&p| p >= 64) else { return Ok(None) };
    let b = p + 4;
    ensure!(u8_at(entity, b)? == 1, "inactive wind at {b:#x}");
    let noise = match u8_at(entity, b + 5)? {
        0 => false,
        1 => true,
        x => bail!("wind noise flag {x} at {:#x}", b + 5),
    };
    ensure!(u32_at(entity, b + 22)? == 0, "wind regions at {:#x} (not read)", b + 22);
    ensure!(u32_at(entity, b + 26)? == 0, "wind mode {} at {:#x} (not read)", u32_at(entity, b + 26)?, b + 26);
    ensure!(vec3_at(entity, b + 30)? == [0.0; 3], "wind oscillation at {:#x} (not read)", b + 30);
    ensure!(u32_at(entity, b + 42)? == 0, "wind kind at {:#x} (not read)", b + 42);
    let m = |d: &[u8], o: usize| -> Result<[f32; 16]> {
        let mut v = [0.0; 16];
        for (k, x) in v.iter_mut().enumerate() {
            *x = f32_at(d, o + 4 * k)?;
        }
        Ok(v)
    };
    let (place, local) = (m(entity, 0)?, m(entity, b + 86)?);
    // The local matrix's X column, through the placement's rotation.
    let x = [local[0], local[1], local[2]];
    let d: [f32; 3] = std::array::from_fn(|r| (0..3).map(|c| place[c * 4 + r] * x[c]).sum());
    let len = d.iter().map(|v| v * v).sum::<f32>().sqrt();
    ensure!(len > 1e-3, "wind without a direction at {b:#x}");
    let percent = [f32_at(entity, b + 6)?, f32_at(entity, b + 10)?, f32_at(entity, b + 14)?, f32_at(entity, b + 18)?];
    Ok(Some(Wind { strength: f32_at(entity, b + 1)?, noise, percent, direction: d.map(|v| v / len) }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_ids_are_their_names_crcs() {
        for (name, id) in [
            ("DynamicMesh", CLASS_DYNAMIC_MESH),
            ("ClothActionSettings", CLASS_CLOTH_ACTION_SETTINGS),
            ("Wind", CLASS_WIND),
            ("SubMesh", CLASS_SUBMESH),
            ("Cloth", CLASS_CLOTH),
            ("ClothComponent", CLASS_CLOTH_COMPONENT),
            ("ClothRagdoll", CLASS_CLOTH_RAGDOLL),
            ("ClothRagdollCapsule", CLASS_CLOTH_RAGDOLL_CAPSULE),
            ("WindForce", CLASS_WIND_FORCE),
            ("ClothActionEntry", CLASS_CLOTH_ACTION_ENTRY),
        ] {
            assert_eq!(crc32fast::hash(name.as_bytes()), id, "{name}");
        }
    }

    #[test]
    fn bad_data_is_an_error_not_a_panic() {
        for n in 0..400 {
            let junk: Vec<u8> = (0..n).map(|i| (i * 37 % 251) as u8).collect();
            assert!(soft_mesh(&junk).is_err());
            assert!(cloth_settings(&junk).is_err());
            assert!(wind(&junk).is_ok_and(|w| w.is_none()) || wind(&junk).is_err());
            let _ = action_settings(&junk);
        }
        let mut entries = vec![1u8, 0, 0, 0, 0, 0, 0, 0];
        entries.extend(CLASS_CLOTH_ACTION_ENTRY.to_le_bytes());
        entries.extend(42u32.to_le_bytes());
        entries.extend(0.5f32.to_le_bytes());
        assert_eq!(action_settings(&entries).unwrap(), vec![(42, 0.5)]);
        assert!(action_settings(&entries[..19]).is_err());
        entries[16..20].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(action_settings(&entries).is_err());
    }

    #[test]
    fn quaternions_turn_vectors() {
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let v = rotate([0.0, 0.0, h, h], [1.0, 0.0, 0.0]);
        assert!((v[0]).abs() < 1e-6 && (v[1] - 1.0).abs() < 1e-6, "{v:?}");
    }

    #[test]
    fn edges_follow_the_masks() {
        let m = SoftMesh {
            positions: vec![[0.0; 3]; 4],
            weights: vec![],
            bones: vec![],
            flags: vec![],
            triangles: vec![[0, 1, 2], [2, 1, 3]],
            edge_masks: vec![0b011, 0b101],
        };
        assert_eq!(m.edges(), vec![[0, 1], [1, 2], [3, 2]]);
        let compiled = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0]];
        let mut moved = m.clone();
        moved.positions = vec![compiled[2], compiled[0], compiled[3], compiled[1]];
        assert_eq!(moved.order_in(&compiled).unwrap(), vec![2, 0, 3, 1]);
        moved.positions[1] = compiled[2];
        assert!(moved.order_in(&compiled).is_err(), "two soft vertices on one compiled vertex");
    }
}
