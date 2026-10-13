//! Collision shapes (Havok-backed in the engine): `MeshShape` (b22b3e61) triangle soups, and the primitive
//! shapes. Entities place them: an entity with an `InertComponent` holding a `RigidBody` whose `Shape` links
//! one of these, in the entity's frame (its world transform).
//!
//! `MeshShape` body, in order (all little-endian, unaligned):
//! - vertices: u32 count, then count x (f32 x, y, z, w) (w is 0)
//! - MOPP code (Havok's bounding-volume tree as byte code): u32 length, bytes
//! - MOPP code info: f32 offset x, y, z, f32 scale
//! - triangles: u32 index count, then u16 indices (three per triangle)
//! - per-triangle material: u32 count (the triangle count), u8 index into the materials
//! - materials: u32 count, u32 `CollisionMaterial` ids
//! - `CollisionFilterInfo`, inline: u32 id, u32 class (43ef99c2), 12 bytes `?`
//! - bounds: min (f32 x, y, z, w), max (f32 x, y, z, w)
//!
//! Every `MeshShape` in the install parses to its last byte (see the `shapes` tool).

use anyhow::{Result, ensure};

pub const CLASS_MESH_SHAPE: u32 = 0xb22b3e61;
const CLASS_FILTER_INFO: u32 = 0x43ef99c2;

#[derive(Debug, Clone)]
pub struct MeshShape {
    /// In the placing entity's frame (Z up, metres).
    pub vertices: Vec<[f32; 3]>,
    pub triangles: Vec<[u16; 3]>,
    /// Per triangle: an index into `materials`.
    pub triangle_materials: Vec<u8>,
    /// `CollisionMaterial` object ids.
    pub materials: Vec<u32>,
    pub min: [f32; 3],
    pub max: [f32; 3],
}

struct R<'a> {
    d: &'a [u8],
    p: usize,
}

impl R<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        ensure!(self.p + n <= self.d.len(), "shape data ends at {:#x} (wanted {n} bytes)", self.p);
        let s = &self.d[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }
    fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.u32()?))
    }
    fn count(&mut self, size: usize) -> Result<usize> {
        let at = self.p;
        let n = self.u32()? as usize;
        ensure!(n.saturating_mul(size) <= self.d.len() - self.p, "bad count {n} at {at:#x}");
        Ok(n)
    }
    fn vec4(&mut self) -> Result<[f32; 3]> {
        let v = [self.f32()?, self.f32()?, self.f32()?];
        self.f32()?;
        Ok(v)
    }
}

/// Parse a `MeshShape` object's body (after its header).
pub fn parse_mesh_shape(body: &[u8]) -> Result<MeshShape> {
    let (s, len) = parse_mesh_shape_prefix(body)?;
    ensure!(len == body.len(), "{} bytes left at {len:#x}", body.len() - len);
    Ok(s)
}

/// A `MeshShape` body at the start of `body` (one stored inline, as a crowd region layout's), and its length.
pub fn parse_mesh_shape_prefix(body: &[u8]) -> Result<(MeshShape, usize)> {
    let mut r = R { d: body, p: 0 };
    let vertices = (0..r.count(16)?).map(|_| r.vec4()).collect::<Result<Vec<_>>>()?;
    let mopp = r.count(1)?;
    r.take(mopp)?;
    r.take(16)?;
    let n = r.count(2)?;
    let at = r.p;
    ensure!(n % 3 == 0, "{n} indices at {at:#x}, not whole triangles");
    let idx: Vec<u16> = r.take(n * 2)?.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    let triangles: Vec<[u16; 3]> = idx.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect();
    let at = r.p;
    ensure!(triangles.iter().flatten().all(|&i| (i as usize) < vertices.len()), "triangle index past {} vertices (indices at {at:#x})", vertices.len());
    let nt = r.count(1)?;
    let triangle_materials = r.take(nt)?.to_vec();
    let materials = (0..r.count(4)?).map(|_| r.u32()).collect::<Result<Vec<_>>>()?;
    r.u32()?;
    let at = r.p;
    let class = r.u32()?;
    ensure!(class == CLASS_FILTER_INFO, "class {class:08x} at {at:#x}, wanted CollisionFilterInfo");
    r.take(12)?;
    let (min, max) = (r.vec4()?, r.vec4()?);
    Ok((MeshShape { vertices, triangles, triangle_materials, materials, min, max }, r.p))
}

/// `RigidBody` (CRC32 of the name): inline in an entity's `InertComponent` (static bodies) or `RigidBodyComponent`
/// (props the game's physics moves).
pub const CLASS_RIGID_BODY: u32 = 0x228f402a;

/// An entity's rigid body, read inline from its body. From the `RigidBody` class word at `p`: u32 id before it, then
/// +4 u32 filter id (0), +8 `CollisionFilterInfo` class, +12 u32 filter word (bits 0-5 the collision layer, 6-9 body
/// part, 10-15 subsystem group, 16-31 system group: Banned445's AC1-Movement-Rewritten reads it so, MIT), +16 u32 `?`
/// (0), +20 u32 shape id, +24 f32 `?` (10 on static bodies: the mass), +28 f32, +32 f32 `?` (0.1, 0.1: friction,
/// restitution?), +36 u32 `?` flags (1, 256, 257), +40 a 4x4: identity on every static body, on the moving props a
/// diagonal (the inertia tensor) and the centre of mass, not a placement (Damascus: 14330 identity, 1670 props).
/// The shape sits in the entity's frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidBody {
    /// The collision layer (`LAYER_NAMES`).
    pub layer: u8,
    /// The `CollisionFilterInfo` word.
    pub filter: u32,
    /// The shape's object id (0: none).
    pub shape: u32,
}

/// The rigid bodies held in an entity's body (each `RigidBody` followed by its `CollisionFilterInfo`).
pub fn rigid_bodies(body: &[u8]) -> Vec<RigidBody> {
    let u32_at = |k: usize| body.get(k..k + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    (0..body.len().saturating_sub(3))
        .filter(|&p| u32_at(p) == Some(CLASS_RIGID_BODY) && u32_at(p + 8) == Some(CLASS_FILTER_INFO))
        .filter_map(|p| {
            let filter = u32_at(p + 12)?;
            Some(RigidBody { layer: (filter & 0x3f) as u8, filter, shape: u32_at(p + 20)? })
        })
        .collect()
}

/// AC1's collision layers (`CollisionFilterInfo` layer), by number: names as Banned445's AC1-Movement-Rewritten (MIT)
/// reads them from the executable.
pub const LAYER_NAMES: [&str; 50] = [
    "LayerNone",
    "Static",
    "Dynamic",
    "Debris",
    "DetectionZone",
    "Character",
    "Ragdoll",
    "Raycast",
    "NoCharacterCollision",
    "RaycastOnlyCharacters",
    "Nothing",
    "Interactive",
    "CharacterNoInteractive",
    "OnlyStatic",
    "RaycastNoCharacters",
    "Horse",
    "HorseBarrier",
    "Camera",
    "CameraBarrier",
    "StaticNoCamera",
    "CharacterNoStatic",
    "MainCharacter",
    "MainCharacterNoInteractive",
    "MainCharacterNoStatic",
    "FuzzyZone",
    "FuzzyZoneNoCamera",
    "FuzzyZoneAllCharacter",
    "OnlyStaticAndCameraRaycast",
    "HollywoodMode",
    "StaticNoLOS",
    "StaticNoCameraNoLOS",
    "StaticNoHorse",
    "OnlyLOS",
    "FuzzyZoneAllCharacterNoCamera",
    "NPCBarrier",
    "OcclusionHeightTest",
    "CameraBarrierPlusLOS",
    "Projectile",
    "ProjectileBarrier",
    "ProjectileCameraBarrier",
    "OnlyLOSAndFuzzy",
    "FuzzyZoneNoCameraAllowAssassination",
    "HorseNoMainCharacterCollision",
    "RaycastNoCharactersFuzzyZone",
    "OnlyDynamic",
    "OnlyFuzzy",
    "NPCSpawning",
    "RaycastFuzzyZone",
    "NoHorseCollision",
    "RagdollPenetrationSolver",
];

/// The layer of the player's body: `Human__CharacterLayer` (0xB0EAA0) turns the player's Character (5) into
/// MainCharacter (21).
pub const LAYER_MAIN_CHARACTER: u8 = 21;

/// The layers the player's body passes through: every pair the physics world's filter set-up (0x4DC8A0) turns off
/// with MainCharacter (21): ragdolls, no-character and nothing layers, camera, horse, NPC, line-of-sight, projectile
/// and fuzzy-only barriers. Everything else (the static world, props, interactive objects, fuzzy zones) stops it.
const PLAYER_PASSES: [u8; 22] = [6, 8, 10, 13, 14, 16, 17, 18, 27, 28, 32, 34, 35, 36, 38, 39, 40, 42, 43, 44, 45, 49];

/// Does a body on `layer` stop the player's body (AC1's filter for MainCharacter)? Layers past the table do not.
pub fn stops_player(layer: u8) -> bool {
    (layer as usize) < LAYER_NAMES.len() && !PLAYER_PASSES.contains(&layer)
}

/// The fuzzy-zone layers (24, 25, 26, 33, 41): they stop the body, but a hit on one is no wall to climb or run up and
/// no support (Banned445's port, `sub_E1C1F0`).
pub fn fuzzy(layer: u8) -> bool {
    matches!(layer, 24 | 25 | 26 | 33 | 41)
}

pub const CLASS_BOX_SHAPE: u32 = 0x4ec68e98;
pub const CLASS_CAPSULE_SHAPE: u32 = 0xb8599052;
/// Convex hulls (named "barrels" in the engine).
pub const CLASS_BARREL_SHAPE: u32 = 0x97cd8890;
pub const CLASS_LIST_SHAPE: u32 = 0x86ebfd8d;

/// A primitive collision shape, in its entity's frame. Transforms are column-major 4x4 (three axes, then
/// the translation).
#[derive(Debug, Clone)]
pub enum Primitive {
    /// `BoxShape` (84 bytes): f32 half extents x, y, z and convex radius, the transform, u32 material.
    Box { half: [f32; 3], transform: [f32; 16] },
    /// `CapsuleShape` (40 bytes): bottom (x, y, z, w), top (x, y, z, w), f32 radius, u32 material.
    Capsule { a: [f32; 3], b: [f32; 3], radius: f32 },
    /// `BarrelShape`: the transform, [face planes (normal x, y, z, distance)], [vertices (x, y, z, w)], u32 material.
    Hull { transform: [f32; 16], planes: Vec<[f32; 4]>, vertices: Vec<[f32; 3]> },
    /// `ListShape`: [inline child shape (u8 kind, u32 id, u32 class, body)].
    List(Vec<Primitive>),
}

/// Parse a primitive shape's body, by its class.
pub fn parse_primitive(class: u32, body: &[u8]) -> Result<Primitive> {
    let mut r = R { d: body, p: 0 };
    let p = read_primitive(&mut r, class)?;
    ensure!(r.p == body.len(), "{} bytes left at {:#x}", body.len() - r.p, r.p);
    Ok(p)
}

fn read_primitive(r: &mut R, class: u32) -> Result<Primitive> {
    let mat = |r: &mut R| -> Result<[f32; 16]> {
        let mut m = [0.0; 16];
        for x in &mut m {
            *x = r.f32()?;
        }
        Ok(m)
    };
    Ok(match class {
        CLASS_BOX_SHAPE => {
            let half = r.vec4()?;
            let transform = mat(r)?;
            r.u32()?;
            Primitive::Box { half, transform }
        }
        CLASS_CAPSULE_SHAPE => {
            let (a, b) = (r.vec4()?, r.vec4()?);
            let radius = r.f32()?;
            r.u32()?;
            Primitive::Capsule { a, b, radius }
        }
        CLASS_BARREL_SHAPE => {
            let transform = mat(r)?;
            let planes = (0..r.count(16)?).map(|_| Ok([r.f32()?, r.f32()?, r.f32()?, r.f32()?])).collect::<Result<Vec<_>>>()?;
            let vertices = (0..r.count(16)?).map(|_| r.vec4()).collect::<Result<Vec<_>>>()?;
            r.u32()?;
            Primitive::Hull { transform, planes, vertices }
        }
        CLASS_LIST_SHAPE => {
            let mut kids = vec![];
            for _ in 0..r.count(9)? {
                let at = r.p;
                ensure!(r.take(1)?[0] == 0, "list child at {at:#x} not inline");
                r.u32()?;
                let c = r.u32()?;
                kids.push(read_primitive(r, c)?);
            }
            Primitive::List(kids)
        }
        c => anyhow::bail!("shape class {c:08x} at {:#x} not read", r.p),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_triangle() {
        let mut d = 3u32.to_le_bytes().to_vec();
        for v in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            for x in v.iter().chain(&[0.0]) {
                d.extend(x.to_le_bytes());
            }
        }
        d.extend(2u32.to_le_bytes());
        d.extend([0x26, 0x00]); // MOPP bytes
        d.extend([0u8; 16]);
        d.extend(3u32.to_le_bytes());
        for i in [0u16, 1, 2] {
            d.extend(i.to_le_bytes());
        }
        d.extend(1u32.to_le_bytes());
        d.push(0);
        d.extend(1u32.to_le_bytes());
        d.extend(0xabcdu32.to_le_bytes());
        d.extend(0u32.to_le_bytes());
        d.extend(CLASS_FILTER_INFO.to_le_bytes());
        d.extend([0u8; 12]);
        for x in [0.0f32, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 1.0] {
            d.extend(x.to_le_bytes());
        }
        let s = parse_mesh_shape(&d).unwrap();
        assert_eq!(s.triangles, vec![[0, 1, 2]]);
        assert_eq!(s.vertices[1], [1.0, 0.0, 0.0]);
        assert_eq!(s.materials, vec![0xabcd]);
        assert_eq!(s.max, [1.0, 1.0, 0.0]);
        assert!(parse_mesh_shape(&d[..d.len() - 1]).is_err());
    }

    #[test]
    fn reads_rigid_bodies_and_their_layers() {
        let mut d = vec![0u8; 3];
        d.extend(7u32.to_le_bytes());
        d.extend(CLASS_RIGID_BODY.to_le_bytes());
        d.extend(0u32.to_le_bytes());
        d.extend(CLASS_FILTER_INFO.to_le_bytes());
        d.extend((0x0001_0000u32 | 39).to_le_bytes());
        d.extend(0u32.to_le_bytes());
        d.extend(0x1234u32.to_le_bytes());
        d.extend([0u8; 84]);
        // (A stray RigidBody word with no filter after it is not one.)
        d.extend(CLASS_RIGID_BODY.to_le_bytes());
        d.extend([0u8; 30]);
        let b = rigid_bodies(&d);
        assert_eq!(b, vec![RigidBody { layer: 39, filter: 0x0001_0027, shape: 0x1234 }]);
        assert!(!stops_player(b[0].layer), "a hiding place's ProjectileCameraBarrier");
        assert!(stops_player(1) && stops_player(2) && stops_player(11) && stops_player(24));
        assert!(!stops_player(16) && !stops_player(18) && !stops_player(60));
        assert!(rigid_bodies(&d[..20]).is_empty(), "cut before its filter word");
    }

    #[test]
    fn parses_a_list_of_boxes() {
        let mut d = 1u32.to_le_bytes().to_vec();
        d.push(0);
        d.extend(5u32.to_le_bytes());
        d.extend(CLASS_BOX_SHAPE.to_le_bytes());
        for x in [0.5f32, 0.25, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 2.0, 0.0, 0.0, 1.0] {
            d.extend(x.to_le_bytes());
        }
        d.extend(9u32.to_le_bytes());
        let Primitive::List(k) = parse_primitive(CLASS_LIST_SHAPE, &d).unwrap() else { panic!() };
        let Primitive::Box { half, transform } = &k[0] else { panic!() };
        assert_eq!(*half, [0.5, 0.25, 1.0]);
        assert_eq!(transform[12], 2.0);
    }
}
