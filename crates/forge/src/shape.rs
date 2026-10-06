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
    ensure!(r.p == body.len(), "{} bytes left at {:#x}", body.len() - r.p, r.p);
    Ok(MeshShape { vertices, triangles, triangle_materials, materials, min, max })
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
