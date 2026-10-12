//! Bone modifiers: what a skeleton does to its own bones after the animation (AC1's secondary motion). Altaïr's
//! main skeleton has 14 hinges, 10 compressions, 6 roll bones and a spring box; the skirt 16 hinges, a look-at, a
//! compression and 2 rotation copies; the hood a look-at and 2 hinges; the sword tag 2 hinges.
//!
//! Each is an object embedded in the skeleton's body: u8 0, u32 id, u32 class (the CRC32 of the name), then its
//! fields. A bone reference is u8 2 and the bone's object id (a bone of the same skeleton), or u8 3 for none.
//! Layouts (offsets from the end of the class id) were found by Banned445's AC1-Movement-Rewritten (MIT; its
//! `src/visual_pose.rs`, `src/skirt_hinge.rs`) and checked against every skeleton of Altaïr's:
//! - `LookAtBoneModifier`: +0 the bone turned, +5 u8 0 or 1 `?`, +6 a null reference, +7 the bone aimed at, +12 four
//!   u32 0 (an offset), +28 u32 the axis aimed (0 x, 1 y, 2 z), +32 u32 the axis kept up.
//! - `HingeBoneModifier`: +0 the bone, +5 u8 0, +6 f32 inertia (how much of the bone's own motion swings it), f32
//!   damping (taken off the swing when it goes against the force), vec3 force (along the parent's frame unless a
//!   reference is given), f32 0, a reference (the bone whose frame the force is in), f32 the share of the wind,
//!   u32 the hinge axis, u32 the bone axis swung, u32 count, then count `Hinge` limits (0xbda2d281: u32 id,
//!   u32 class, a reference bone (none: the parent), f32 min, max angle, f32 soft zones inside them, f32 x 16 `?`
//!   (a matrix the game rebuilds from the rest pose)), then vec4 the hinge axis as a vector.
//! - `CompressBoneModifier`: +0 the bone, +5 u8 `?`, +6 f32 position weight, +10 f32 rotation weight, +14 and +19
//!   the two source bones, +24 / +40 quaternion offsets, +56 / +72 position offsets (vec4), +88 u8 rotation on, +89
//!   u8 position on. The bone takes source B's frame moved toward A's by the weights.
//! - `RotationPasteModifier`: +0 the bone, +5 u8 `?`, +6 the bone whose world rotation it takes.
//! - `RollBoneModifier`: +0 the bone, +5 u8 `?`, +6 u32 mode (0-3), +10 f32 weight, +14 the source bone (the bone's
//!   twist follows or counters the source's).
//! - `SpringBoxModifier`: +0 the bone, +5 u8 `?`, +6 f32 stiffness, +10 damping, +14 the share of the wind, +18 vec3
//!   force, +30 f32 `?`, +34 vec3 box min, +46 f32 `?`, +50 vec3 box max (in the parent's frame).

use crate::read::{f32_at, u8_at, u32_at, vec3_at, vec4_at};
use crate::skeleton::Skeleton;
use anyhow::{Result, bail, ensure};

const CLASS_LOOK_AT: u32 = 0x08d774b2;
const CLASS_HINGE: u32 = 0x324796c7;
const CLASS_HINGE_LIMIT: u32 = 0xbda2d281;
const CLASS_COMPRESS: u32 = 0x844987d6;
const CLASS_ROTATION_PASTE: u32 = 0x711759af;
const CLASS_ROLL: u32 = 0x76d082f6;
const CLASS_SPRING_BOX: u32 = 0xf9d25748;

/// A look-at: `target`'s axis `axis` turned to `aim`, its axis `up` kept toward its current Z. Bones by name hash.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LookAt {
    pub target: u32,
    pub aim: u32,
    pub axis: u8,
    pub up: u8,
}

/// One angle limit of a hinge, measured about the hinge axis from the parent's frame (or `reference`'s).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HingeLimit {
    pub reference: Option<u32>,
    pub min: f32,
    pub max: f32,
    /// Zones inside min and max where the swing toward the limit slows (radians).
    pub soft_min: f32,
    pub soft_max: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hinge {
    pub target: u32,
    pub inertia: f32,
    pub damping: f32,
    pub force: [f32; 3],
    pub force_reference: Option<u32>,
    pub wind: f32,
    /// The bone's local axis it swings about, and the one that swings (0 x, 1 y, 2 z).
    pub axis: u8,
    pub direction: u8,
    pub limits: Vec<HingeLimit>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Compress {
    pub target: u32,
    /// Sources A and B.
    pub sources: [u32; 2],
    pub position_weight: f32,
    pub rotation_weight: f32,
    /// Quaternions (x, y, z, w) and positions in each source's frame.
    pub rotation_offsets: [[f32; 4]; 2],
    pub position_offsets: [[f32; 3]; 2],
    pub rotation: bool,
    pub position: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Roll {
    pub target: u32,
    pub source: u32,
    pub mode: u32,
    pub weight: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringBox {
    pub target: u32,
    pub stiffness: f32,
    pub damping: f32,
    pub wind: f32,
    pub force: [f32; 3],
    pub min: [f32; 3],
    pub max: [f32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub enum Modifier {
    LookAt(LookAt),
    Hinge(Hinge),
    Compress(Compress),
    /// (bone, source): the bone takes the source's world rotation.
    RotationPaste(u32, u32),
    Roll(Roll),
    SpringBox(SpringBox),
}

impl Modifier {
    /// The bone it writes (name hash).
    pub fn target(&self) -> u32 {
        match self {
            Modifier::LookAt(m) => m.target,
            Modifier::Hinge(m) => m.target,
            Modifier::Compress(m) => m.target,
            Modifier::RotationPaste(t, _) => *t,
            Modifier::Roll(m) => m.target,
            Modifier::SpringBox(m) => m.target,
        }
    }
}

/// The modifiers in a skeleton's body, in the order written, and an error for each one whose layout is not the
/// one known (left out, not guessed).
pub fn modifiers(body: &[u8], skeleton: &Skeleton) -> (Vec<Modifier>, Vec<anyhow::Error>) {
    let (mut out, mut errors) = (vec![], vec![]);
    let classes = [CLASS_LOOK_AT, CLASS_HINGE, CLASS_COMPRESS, CLASS_ROTATION_PASTE, CLASS_ROLL, CLASS_SPRING_BOX];
    for p in 5..body.len().saturating_sub(4) {
        let class = u32::from_le_bytes([body[p], body[p + 1], body[p + 2], body[p + 3]]);
        if body[p - 5] != 0 || !classes.contains(&class) {
            continue;
        }
        match read(body, p + 4, class, skeleton) {
            Ok(m) => out.push(m),
            Err(e) => errors.push(e.context(format!("modifier {class:08x} at {p:#x}"))),
        }
    }
    (out, errors)
}

/// A bone reference at `p`: (bone name hash or none, its size).
fn bone_ref(d: &[u8], p: usize, sk: &Skeleton) -> Result<(Option<u32>, usize)> {
    match u8_at(d, p)? {
        3 => Ok((None, 1)),
        2 => {
            let id = u32_at(d, p + 1)?;
            match sk.bones.iter().find(|b| b.id == id) {
                Some(b) => Ok((Some(b.name_hash), 5)),
                None => bail!("bone {id:08x} at {:#x} not in the skeleton", p + 1),
            }
        }
        x => bail!("bone reference tag {x} at {p:#x}"),
    }
}

fn bone(d: &[u8], p: usize, sk: &Skeleton) -> Result<u32> {
    match bone_ref(d, p, sk)?.0 {
        Some(b) => Ok(b),
        None => bail!("no bone at {p:#x}"),
    }
}

fn flag(d: &[u8], p: usize) -> Result<bool> {
    match u8_at(d, p)? {
        0 => Ok(false),
        1 => Ok(true),
        x => bail!("flag {x} at {p:#x}"),
    }
}

fn axis(d: &[u8], p: usize) -> Result<u8> {
    let a = u32_at(d, p)?;
    ensure!(a < 3, "axis {a} at {p:#x}");
    Ok(a as u8)
}

fn read(d: &[u8], b: usize, class: u32, sk: &Skeleton) -> Result<Modifier> {
    let target = bone(d, b, sk)?;
    Ok(match class {
        CLASS_LOOK_AT => {
            flag(d, b + 5)?;
            ensure!(u8_at(d, b + 6)? == 3, "look-at reference at {:#x} not null", b + 6);
            let aim = bone(d, b + 7, sk)?;
            ensure!((0..4).all(|k| u32_at(d, b + 12 + 4 * k).is_ok_and(|v| v == 0)), "look-at offset at {:#x}", b + 12);
            let (axis, up) = (axis(d, b + 28)?, axis(d, b + 32)?);
            ensure!(axis != up && aim != target, "look-at axes {axis} {up}");
            Modifier::LookAt(LookAt { target, aim, axis, up })
        }
        CLASS_HINGE => {
            ensure!(u8_at(d, b + 5)? == 0, "hinge byte at {:#x}", b + 5);
            let (inertia, damping) = (f32_at(d, b + 6)?, f32_at(d, b + 10)?);
            let force = vec3_at(d, b + 14)?;
            ensure!(f32_at(d, b + 26)? == 0.0 && inertia >= 0.0 && (0.0..=1.0).contains(&damping), "hinge values at {:#x}", b + 6);
            let (force_reference, n) = bone_ref(d, b + 30, sk)?;
            let mut r = b + 30 + n;
            let wind = f32_at(d, r)?;
            let (ax, direction) = (axis(d, r + 4)?, axis(d, r + 8)?);
            let count = u32_at(d, r + 12)? as usize;
            ensure!(ax != direction && (1..=4).contains(&count), "hinge axes {ax} {direction}, {count} limits at {:#x}", r + 4);
            r += 16;
            let mut limits = vec![];
            for _ in 0..count {
                ensure!(u32_at(d, r + 4)? == CLASS_HINGE_LIMIT, "no Hinge limit at {:#x}", r + 4);
                let (reference, n) = bone_ref(d, r + 8, sk)?;
                r += 8 + n;
                let [min, max, soft_min, soft_max] = vec4_at(d, r)?;
                ensure!(min <= max && soft_min >= 0.0 && soft_max >= 0.0, "hinge limits at {r:#x}");
                for k in 0..16 {
                    f32_at(d, r + 16 + 4 * k)?;
                }
                r += 80;
                limits.push(HingeLimit { reference, min, max, soft_min, soft_max });
            }
            let v = vec4_at(d, r)?;
            let mut unit = [0.0; 4];
            unit[ax as usize] = 1.0;
            ensure!(v.iter().zip(&unit).all(|(a, b)| (a - b).abs() < 1e-3), "hinge axis vector {v:?} at {r:#x}");
            Modifier::Hinge(Hinge { target, inertia, damping, force, force_reference, wind, axis: ax, direction, limits })
        }
        CLASS_COMPRESS => {
            let quat = |o: usize| -> Result<[f32; 4]> {
                let q = vec4_at(d, b + o)?;
                let len = q.iter().map(|v| v * v).sum::<f32>().sqrt();
                ensure!((len - 1.0).abs() < 1e-3, "not a rotation at {:#x}", b + o);
                Ok(q.map(|v| v / len))
            };
            let pos = |o: usize| -> Result<[f32; 3]> { vec3_at(d, b + o) };
            let sources = [bone(d, b + 14, sk)?, bone(d, b + 19, sk)?];
            ensure!(!sources.contains(&target), "compression of a bone by itself");
            Modifier::Compress(Compress {
                target,
                sources,
                position_weight: f32_at(d, b + 6)?,
                rotation_weight: f32_at(d, b + 10)?,
                rotation_offsets: [quat(24)?, quat(40)?],
                position_offsets: [pos(56)?, pos(72)?],
                rotation: flag(d, b + 88)?,
                position: flag(d, b + 89)?,
            })
        }
        CLASS_ROTATION_PASTE => {
            let source = bone(d, b + 6, sk)?;
            ensure!(source != target, "rotation copy from itself");
            Modifier::RotationPaste(target, source)
        }
        CLASS_ROLL => {
            let (mode, weight) = (u32_at(d, b + 6)?, f32_at(d, b + 10)?);
            let source = bone(d, b + 14, sk)?;
            ensure!(mode <= 3 && source != target, "roll mode {mode} at {:#x}", b + 6);
            Modifier::Roll(Roll { target, source, mode, weight })
        }
        CLASS_SPRING_BOX => {
            let (min, max) = (vec3_at(d, b + 34)?, vec3_at(d, b + 50)?);
            ensure!((0..3).all(|k| min[k] <= max[k]), "spring box {min:?} {max:?} at {:#x}", b + 34);
            Modifier::SpringBox(SpringBox {
                target,
                stiffness: f32_at(d, b + 6)?,
                damping: f32_at(d, b + 10)?,
                wind: f32_at(d, b + 14)?,
                force: vec3_at(d, b + 18)?,
                min,
                max,
            })
        }
        _ => bail!("not a bone modifier"),
    })
}

/// The `WindForce` embedded in a character entity's `SkeletonComponent` (what its hinges and springs feel of the
/// wind), if enabled.
pub fn skeleton_wind(entity: &[u8]) -> Result<Option<crate::cloth::WindForce>> {
    const CLASS_SKELETON_COMPONENT: u32 = 0x71fda747;
    let s = crate::read::find_one(entity, CLASS_SKELETON_COMPONENT, "SkeletonComponent")?;
    let at: Vec<usize> = crate::read::find_u32(entity, crate::cloth::CLASS_WIND_FORCE).into_iter().filter(|&q| q > s && q < s + 400).collect();
    ensure!(at.len() == 1, "{} WindForce in the SkeletonComponent at {s:#x}", at.len());
    crate::cloth::wind_force(entity, at[0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skeleton::Bone;

    #[test]
    fn class_ids_are_their_names_crcs() {
        for (name, id) in [
            ("LookAtBoneModifier", CLASS_LOOK_AT),
            ("HingeBoneModifier", CLASS_HINGE),
            ("Hinge", CLASS_HINGE_LIMIT),
            ("CompressBoneModifier", CLASS_COMPRESS),
            ("RotationPasteModifier", CLASS_ROTATION_PASTE),
            ("RollBoneModifier", CLASS_ROLL),
            ("SpringBoxModifier", CLASS_SPRING_BOX),
            ("SkeletonComponent", 0x71fda747),
        ] {
            assert_eq!(crc32fast::hash(name.as_bytes()), id, "{name}");
        }
    }

    fn bone(id: u32, name_hash: u32) -> Bone {
        Bone {
            id,
            name_hash,
            name: None,
            parent: None,
            local_pos: [0.0; 3],
            local_rot: [0.0, 0.0, 0.0, 1.0],
            model_pos: [0.0; 3],
            model_rot: [0.0, 0.0, 0.0, 1.0],
        }
    }

    #[test]
    fn a_look_at_names_its_bones_and_axes() {
        let sk = Skeleton { bones: vec![bone(0x11, 0xaaaa), bone(0x22, 0xbbbb)] };
        let mut body = vec![0u8; 8];
        body.extend([0, 1, 2, 3, 4]);
        body.extend(CLASS_LOOK_AT.to_le_bytes());
        body.push(2);
        body.extend(0x11u32.to_le_bytes());
        body.extend([1, 3, 2]);
        body.extend(0x22u32.to_le_bytes());
        body.extend([0u8; 16]);
        body.extend(0u32.to_le_bytes());
        body.extend(2u32.to_le_bytes());
        body.extend([0u8; 16]);
        let (m, e) = modifiers(&body, &sk);
        assert!(e.is_empty());
        assert_eq!(m, vec![Modifier::LookAt(LookAt { target: 0xaaaa, aim: 0xbbbb, axis: 0, up: 2 })]);
        // A reference to a bone it does not have: an error, the modifier left out.
        let other = Skeleton { bones: vec![bone(0x11, 0xaaaa)] };
        let (m, e) = modifiers(&body, &other);
        assert!(m.is_empty() && e.len() == 1);
        // Cut short anywhere: errors, never a panic.
        for n in 0..body.len() {
            let _ = modifiers(&body[..n], &sk);
        }
    }

    #[test]
    fn a_hinge_reads_its_limits() {
        let sk = Skeleton { bones: vec![bone(0x11, 0xaaaa)] };
        let mut body = vec![0u8; 5];
        body.extend(CLASS_HINGE.to_le_bytes());
        body.push(2);
        body.extend(0x11u32.to_le_bytes());
        body.push(0);
        for v in [0.5f32, 0.2, 0.0, 0.0, -9.8, 0.0] {
            body.extend(v.to_le_bytes());
        }
        body.push(3);
        body.extend(0.25f32.to_le_bytes());
        for v in [2u32, 0, 1] {
            body.extend(v.to_le_bytes());
        }
        body.extend([0u8; 4]);
        body.extend(CLASS_HINGE_LIMIT.to_le_bytes());
        body.push(3);
        for v in [-0.4f32, 0.6, 0.1, 0.2] {
            body.extend(v.to_le_bytes());
        }
        body.extend([0u8; 64]);
        for v in [0.0f32, 0.0, 1.0, 0.0] {
            body.extend(v.to_le_bytes());
        }
        let (m, e) = modifiers(&body, &sk);
        assert!(e.is_empty(), "{e:?}");
        let Modifier::Hinge(h) = &m[0] else { panic!() };
        assert_eq!((h.axis, h.direction, h.wind, h.force[2]), (2, 0, 0.25, -9.8));
        assert_eq!(h.limits, vec![HingeLimit { reference: None, min: -0.4, max: 0.6, soft_min: 0.1, soft_max: 0.2 }]);
        for n in 0..body.len() {
            let _ = modifiers(&body[..n], &sk);
        }
    }
}
