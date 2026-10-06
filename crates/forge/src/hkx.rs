//! Havok 4.6 binary packfiles (`.hkx` images, magic 57e0e057 10c0c010), as embedded in AC1 objects
//! (`HavokChunk` fields: `RagdollNew`'s physics system), and the ragdoll in them.
//!
//! A packfile is a header (0x40 bytes: magic, user tag, file version, layout rules (pointer size 4, little
//! endian), section count, contents section and offset, contents class name, `Havok-4.6.1-r1`), then one
//! 48-byte header per section (`__classnames__`, `__data__`, `__types__`): name (20 bytes), and the
//! absolute data start, then offsets (from the data start) of the local, global and virtual fixup tables,
//! exports, imports and the end. Objects are 32-bit C++ memory images; pointers are written as 0 and the
//! fixups say where they point:
//! - local fixups: (i32 source, i32 destination) in the same section, -1 padded
//! - global fixups: (i32 source, i32 section, i32 destination)
//! - virtual fixups: (i32 object start, i32 class-name section, i32 name offset): where each object is,
//!   and its class
//!
//! `__types__` holds the `hkClass` definitions (48 bytes: name, parent, object size, interfaces, enums and
//! count, members and count, defaults, attributes, flags, version) with 20-byte `hkClassMember`s (name,
//! class, enum, u8 type, u8 subtype, i16 C array size, u16 flags, u16 offset). Fields are read by name
//! through them (a class's members include its parents'). Arrays are (pointer, i32 size, i32 capacity),
//! simple arrays (pointer, i32 size).

use anyhow::{Context, Result, bail, ensure};
use std::collections::HashMap;

const MAGIC: [u8; 8] = [0x57, 0xe0, 0xe0, 0x57, 0x10, 0xc0, 0xc0, 0x10];

/// A place in the packfile: (section, offset from its data start).
pub type Loc = (usize, usize);

struct Section {
    start: usize,
    end: usize,
}

struct Class {
    parent: Option<String>,
    members: HashMap<String, usize>,
}

pub struct Packfile<'a> {
    d: &'a [u8],
    sections: Vec<Section>,
    pointers: HashMap<Loc, Loc>,
    /// Objects and their classes.
    pub objects: Vec<(Loc, String)>,
    classes: HashMap<String, Class>,
}

fn i32_at(d: &[u8], o: usize) -> Result<i32> {
    Ok(i32::from_le_bytes(d.get(o..o + 4).with_context(|| format!("packfile ends at {o:#x}"))?.try_into()?))
}

impl<'a> Packfile<'a> {
    /// Find and read the packfile in `bytes`.
    pub fn find(bytes: &'a [u8]) -> Result<Self> {
        let at = bytes.windows(8).position(|w| w == MAGIC).context("no Havok packfile")?;
        Self::parse(&bytes[at..])
    }

    pub fn parse(d: &'a [u8]) -> Result<Self> {
        ensure!(d.starts_with(&MAGIC), "not a Havok packfile");
        ensure!(d.get(16) == Some(&4), "pointer size {:?}, only 32-bit packfiles are read", d.get(16));
        let n = i32_at(d, 20)?;
        ensure!((1..16).contains(&n), "{n} sections");
        let mut sections = vec![];
        let mut fixups = vec![];
        for k in 0..n as usize {
            let o = 0x40 + 48 * k;
            let f: Vec<i32> = (0..7).map(|i| i32_at(d, o + 20 + 4 * i)).collect::<Result<_>>()?;
            let start = f[0] as usize;
            let end = start + f[6] as usize;
            ensure!(f.iter().all(|&x| x >= 0) && end <= d.len(), "section {k} at {o:#x} out of range");
            sections.push(Section { start, end });
            fixups.push(f);
        }
        let mut pointers = HashMap::new();
        let mut virt = vec![];
        for (si, f) in fixups.iter().enumerate() {
            let st = sections[si].start;
            let (lf, gf, vf, ex) = (f[1] as usize, f[2] as usize, f[3] as usize, f[4] as usize);
            for o in (lf..gf).step_by(8) {
                let (a, b) = (i32_at(d, st + o)?, i32_at(d, st + o + 4)?);
                if a >= 0 {
                    pointers.insert((si, a as usize), (si, b as usize));
                }
            }
            for o in (gf..vf).step_by(12) {
                let (a, s, b) = (i32_at(d, st + o)?, i32_at(d, st + o + 4)?, i32_at(d, st + o + 8)?);
                if a >= 0 && (s as usize) < sections.len() {
                    pointers.insert((si, a as usize), (s as usize, b as usize));
                }
            }
            for o in (vf..ex).step_by(12) {
                let (a, s, b) = (i32_at(d, st + o)?, i32_at(d, st + o + 4)?, i32_at(d, st + o + 8)?);
                if a >= 0 && (s as usize) < sections.len() {
                    virt.push(((si, a as usize), (s as usize, b as usize)));
                }
            }
        }
        let mut p = Packfile { d, sections, pointers, objects: vec![], classes: HashMap::new() };
        for (at, name) in virt {
            let name = p.cstr_at(name)?;
            p.objects.push((at, name));
        }
        p.objects.sort_by_key(|o| o.0);
        p.read_classes()?;
        Ok(p)
    }

    fn abs(&self, (s, o): Loc, n: usize) -> Result<usize> {
        let sec = self.sections.get(s).context("bad section")?;
        let a = sec.start + o;
        ensure!(a + n <= sec.end, "read past section {s} at {o:#x}");
        Ok(a)
    }

    fn cstr_at(&self, at: Loc) -> Result<String> {
        let a = self.abs(at, 0)?;
        let len = self.d[a..].iter().position(|&c| c == 0).context("unterminated string")?;
        Ok(String::from_utf8_lossy(&self.d[a..a + len]).into_owned())
    }

    pub fn u8(&self, at: Loc) -> Result<u8> {
        Ok(self.d[self.abs(at, 1)?])
    }
    pub fn i16(&self, at: Loc) -> Result<i16> {
        let a = self.abs(at, 2)?;
        Ok(i16::from_le_bytes([self.d[a], self.d[a + 1]]))
    }
    pub fn i32(&self, at: Loc) -> Result<i32> {
        i32_at(self.d, self.abs(at, 4)?)
    }
    pub fn f32(&self, at: Loc) -> Result<f32> {
        Ok(f32::from_bits(self.i32(at)? as u32))
    }
    pub fn vec4(&self, at: Loc) -> Result<[f32; 4]> {
        Ok([self.f32(at)?, self.f32((at.0, at.1 + 4))?, self.f32((at.0, at.1 + 8))?, self.f32((at.0, at.1 + 12))?])
    }
    /// Where the pointer at `at` points (None for null).
    pub fn ptr(&self, at: Loc) -> Option<Loc> {
        self.pointers.get(&at).copied()
    }
    pub fn cstr(&self, at: Loc) -> Result<Option<String>> {
        self.ptr(at).map(|p| self.cstr_at(p)).transpose()
    }
    /// An array or simple array field: (its data, its size).
    pub fn array(&self, at: Loc) -> Result<(Option<Loc>, usize)> {
        let n = self.i32((at.0, at.1 + 4))?;
        ensure!((0..1_000_000).contains(&n), "array size {n} at {:#x}", at.1);
        Ok((self.ptr(at), n as usize))
    }
    /// The pointers in an array of pointers.
    pub fn ptr_array(&self, at: Loc) -> Result<Vec<Loc>> {
        let (p, n) = self.array(at)?;
        let Some(p) = p else { return Ok(vec![]) };
        Ok((0..n).filter_map(|i| self.ptr((p.0, p.1 + 4 * i))).collect())
    }

    pub fn class_of(&self, at: Loc) -> Option<&str> {
        self.objects.iter().find(|o| o.0 == at).map(|o| o.1.as_str())
    }

    /// Objects of class `name`.
    pub fn all(&self, name: &str) -> impl Iterator<Item = Loc> + '_ {
        let name = name.to_string();
        self.objects.iter().filter(move |o| o.1 == name).map(|o| o.0)
    }

    /// The offset of member `member` of class `class` (or of a parent class).
    pub fn member(&self, class: &str, member: &str) -> Result<usize> {
        let mut c = Some(class.to_string());
        while let Some(name) = c {
            let cl = self.classes.get(&name).with_context(|| format!("no class {name} in the packfile"))?;
            if let Some(&o) = cl.members.get(member) {
                return Ok(o);
            }
            c = cl.parent.clone();
        }
        bail!("{class} has no member {member}")
    }

    /// `at` moved to member `member` of `class`.
    pub fn field(&self, at: Loc, class: &str, member: &str) -> Result<Loc> {
        Ok((at.0, at.1 + self.member(class, member)?))
    }

    fn read_classes(&mut self) -> Result<()> {
        let defs: Vec<Loc> = self.all("hkClass").collect();
        let mut by_loc: HashMap<Loc, String> = HashMap::new();
        let mut raw = vec![];
        for at in defs {
            let name = self.cstr(at)?.context("hkClass without a name")?;
            by_loc.insert(at, name.clone());
            let parent = self.ptr((at.0, at.1 + 4));
            let mut members = HashMap::new();
            if let Some(mp) = self.ptr((at.0, at.1 + 24)) {
                let n = self.i32((at.0, at.1 + 28))?.max(0) as usize;
                for k in 0..n {
                    let m = (mp.0, mp.1 + 20 * k);
                    let Some(mname) = self.cstr(m)? else { continue };
                    let off = self.i32((m.0, m.1 + 16))? as u32 >> 16;
                    members.insert(mname, off as usize);
                }
            }
            raw.push((name, parent, members));
        }
        for (name, parent, members) in raw {
            let parent = parent.and_then(|p| by_loc.get(&p).cloned());
            self.classes.insert(name, Class { parent, members });
        }
        Ok(())
    }
}

/// A ragdoll body's collision shape, in the body's frame (Z up, metres).
#[derive(Debug, Clone, PartialEq)]
pub enum BodyShape {
    Capsule { a: [f32; 3], b: [f32; 3], radius: f32 },
    Box { half: [f32; 3], radius: f32 },
}

#[derive(Debug, Clone)]
pub struct Body {
    pub name: String,
    pub shape: BodyShape,
    /// Body to model space at the reference pose: rotation columns, then translation.
    pub rotation: [[f32; 3]; 3],
    pub translation: [f32; 3],
    /// 0 for keyframed (infinite mass) bodies.
    pub mass: f32,
}

/// A joint between two bodies, with each body's joint frame (rotation columns, pivot) in that body.
#[derive(Debug, Clone)]
pub struct Joint {
    pub a: usize,
    pub b: usize,
    pub frame_a: ([[f32; 3]; 3], [f32; 3]),
    pub frame_b: ([[f32; 3]; 3], [f32; 3]),
    pub limit: JointLimit,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JointLimit {
    /// Ball and socket: twist about the twist axis, a cone round it, and planes (radians).
    Ragdoll { twist: (f32, f32), cone: f32, planes: (f32, f32) },
    /// A hinge (elbows, knees): the angle about its axis (radians).
    Hinge { min: f32, max: f32 },
}

#[derive(Debug, Clone)]
pub struct Ragdoll {
    pub bodies: Vec<Body>,
    pub joints: Vec<Joint>,
    /// The ragdoll skeleton's bones (names like `Hips`, `LeftForeArm`) and each one's body, if any.
    pub bones: Vec<(String, Option<usize>)>,
}

fn transform(p: &Packfile, at: Loc) -> Result<([[f32; 3]; 3], [f32; 3])> {
    let col = |k: usize| -> Result<[f32; 3]> {
        let v = p.vec4((at.0, at.1 + 16 * k))?;
        Ok([v[0], v[1], v[2]])
    };
    Ok(([col(0)?, col(1)?, col(2)?], col(3)?))
}

/// The ragdoll in a packfile (`hkRagdollInstance`): its bodies, the joints between them, its bones.
pub fn ragdoll(p: &Packfile) -> Result<Ragdoll> {
    let inst = p.all("hkRagdollInstance").next().context("no hkRagdollInstance")?;
    let body_locs = p.ptr_array(p.field(inst, "hkRagdollInstance", "rigidBodies")?)?;
    let mut bodies = vec![];
    for &rb in &body_locs {
        let name = p.cstr(p.field(rb, "hkRigidBody", "name")?)?.unwrap_or_default();
        let collidable = p.field(rb, "hkRigidBody", "collidable")?;
        let mut shape_at = p.ptr(p.field(collidable, "hkCdBody", "shape")?).context("body without a shape")?;
        // A translated shape: its child, moved.
        let mut offset = [0.0f32; 3];
        if p.class_of(shape_at) == Some("hkConvexTranslateShape") {
            let t = p.vec4(p.field(shape_at, "hkConvexTranslateShape", "translation")?)?;
            offset = [t[0], t[1], t[2]];
            let child = p.field(shape_at, "hkConvexTranslateShape", "childShape")?;
            shape_at = p.ptr(child).context("translated shape without a child")?;
        }
        let radius = p.f32(p.field(shape_at, "hkConvexShape", "radius")?)?;
        let add = |v: [f32; 4]| [v[0] + offset[0], v[1] + offset[1], v[2] + offset[2]];
        let shape = match p.class_of(shape_at) {
            Some("hkCapsuleShape") => BodyShape::Capsule {
                a: add(p.vec4(p.field(shape_at, "hkCapsuleShape", "vertexA")?)?),
                b: add(p.vec4(p.field(shape_at, "hkCapsuleShape", "vertexB")?)?),
                radius,
            },
            Some("hkBoxShape") => {
                let h = p.vec4(p.field(shape_at, "hkBoxShape", "halfExtents")?)?;
                BodyShape::Box { half: [h[0], h[1], h[2]], radius }
            }
            other => bail!("body {name}: shape {other:?} not read"),
        };
        let motion = p.field(rb, "hkRigidBody", "motion")?;
        let state = p.field(motion, "hkMotion", "motionState")?;
        let (rotation, translation) = transform(p, p.field(state, "hkMotionState", "transform")?)?;
        let inv = p.vec4(p.field(motion, "hkMotion", "inertiaAndMassInv")?)?;
        let mass = if inv[3] > 0.0 { 1.0 / inv[3] } else { 0.0 };
        bodies.push(Body { name, shape, rotation, translation, mass });
    }
    let mut joints = vec![];
    for ci in p.ptr_array(p.field(inst, "hkRagdollInstance", "constraints")?)? {
        let ents = p.field(ci, "hkConstraintInstance", "entities")?;
        let (Some(ea), Some(eb)) = (p.ptr(ents), p.ptr((ents.0, ents.1 + 4))) else { continue };
        let (Some(a), Some(b)) = (body_locs.iter().position(|&l| l == ea), body_locs.iter().position(|&l| l == eb)) else { continue };
        let Some(data) = p.ptr(p.field(ci, "hkConstraintInstance", "data")?) else { continue };
        let class = p.class_of(data).unwrap_or("").to_string();
        let atoms_class = format!("{class}Atoms");
        let atoms = p.field(data, &class, "atoms")?;
        let tr = p.field(atoms, &atoms_class, "transforms")?;
        let frame_a = transform(p, p.field(tr, "hkSetLocalTransformsConstraintAtom", "transformA")?)?;
        let frame_b = transform(p, p.field(tr, "hkSetLocalTransformsConstraintAtom", "transformB")?)?;
        let range = |at: Loc, class: &str| -> Result<(f32, f32)> { Ok((p.f32(p.field(at, class, "minAngle")?)?, p.f32(p.field(at, class, "maxAngle")?)?)) };
        let limit = match class.as_str() {
            "hkRagdollConstraintData" => JointLimit::Ragdoll {
                twist: range(p.field(atoms, &atoms_class, "twistLimit")?, "hkTwistLimitConstraintAtom")?,
                cone: range(p.field(atoms, &atoms_class, "coneLimit")?, "hkConeLimitConstraintAtom")?.1,
                planes: range(p.field(atoms, &atoms_class, "planesLimit")?, "hkConeLimitConstraintAtom")?,
            },
            "hkLimitedHingeConstraintData" => {
                let (min, max) = range(p.field(atoms, &atoms_class, "angLimit")?, "hkAngLimitConstraintAtom")?;
                JointLimit::Hinge { min, max }
            }
            _ => continue,
        };
        joints.push(Joint { a, b, frame_a, frame_b, limit });
    }
    let mut bones = vec![];
    if let Some(sk) = p.ptr(p.field(inst, "hkRagdollInstance", "skeleton")?) {
        let (bp, n) = p.array(p.field(sk, "hkSkeleton", "bones")?)?;
        let (map_p, map_n) = p.array(p.field(inst, "hkRagdollInstance", "boneToRigidBodyMap")?)?;
        for i in 0..n {
            let Some(bp) = bp else { break };
            let name = match p.ptr((bp.0, bp.1 + 4 * i)) {
                Some(bone) => p.cstr(bone)?.unwrap_or_default(),
                None => String::new(),
            };
            let body = match map_p {
                Some(m) if i < map_n => usize::try_from(p.i32((m.0, m.1 + 4 * i))?).ok().filter(|&b| b < bodies.len()),
                _ => None,
            };
            bones.push((name, body));
        }
    }
    Ok(Ragdoll { bodies, joints, bones })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-section packfile with a single pointer fixup and a virtual fixup, built by hand.
    #[test]
    fn reads_fixups() {
        let mut d = MAGIC.to_vec();
        d.extend([0u8; 8]);
        d.extend([4, 1, 0, 1]);
        d.extend(2i32.to_le_bytes()); // sections
        d.resize(0x40, 0xff);
        let names_start = 0x40 + 96;
        // section 0: class names "hkFoo"
        let mut s0 = vec![0u8; 20];
        s0[..14].copy_from_slice(b"__classnames__");
        let names = b"\0\0\0\0\x09hkFoo\0";
        for v in [names_start as i32, names.len() as i32, names.len() as i32, names.len() as i32, names.len() as i32, names.len() as i32, names.len() as i32] {
            s0.extend(v.to_le_bytes());
        }
        // section 1: an 8-byte object whose first field points at its second; fixups after
        let data_start = names_start + names.len();
        let obj = [0u8, 0, 0, 0, 7, 0, 0, 0];
        let mut fix = vec![];
        fix.extend(0i32.to_le_bytes());
        fix.extend(4i32.to_le_bytes()); // local: 0 -> 4
        let gf = obj.len() + fix.len();
        let mut virt = vec![];
        virt.extend(0i32.to_le_bytes());
        virt.extend(0i32.to_le_bytes());
        virt.extend(5i32.to_le_bytes()); // object at 0 is "hkFoo"
        let vf = gf;
        let ex = vf + virt.len();
        let mut s1 = vec![0u8; 20];
        s1[..8].copy_from_slice(b"__data__");
        for v in [data_start as i32, obj.len() as i32, gf as i32, vf as i32, ex as i32, ex as i32, ex as i32] {
            s1.extend(v.to_le_bytes());
        }
        d.extend(s0);
        d.extend(s1);
        d.extend(names);
        d.extend(obj);
        d.extend(fix);
        d.extend(virt);
        let p = Packfile::parse(&d).unwrap();
        assert_eq!(p.ptr((1, 0)), Some((1, 4)));
        assert_eq!(p.i32((1, 4)).unwrap(), 7);
        assert_eq!(p.class_of((1, 0)), Some("hkFoo"));
        assert!(Packfile::parse(&d[..0x50]).is_err());
    }
}
