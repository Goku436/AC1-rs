//! Skeleton objects (class 0x24aecb7c).
//!
//! Body: u32 0, u32 bone count, then per bone (variable size):
//! `u8 0, u32 id, u32 class 0x95741049, u32 name hash (CRC32 of the bone name),
//!  parent ref (u8 2 + u32 bone id, or u8 3 = none),
//!  u8 0, u32 id, u32 class 0x6350e5a6, object-space {vec4 pos, quat xyzw}, local {vec4 pos, quat xyzw},
//!  u32 extra count, extras (constraint/IK objects), u8 bone index, u8 ?`.
//! Local transforms compose the usual way: world = parent.pos + parent.rot * pos, parent.rot * rot.
//! Bones are found by scanning for the bone class tag, since extras vary in size.

use crate::u32_at;
use anyhow::{Result, ensure};

pub const CLASS_SKELETON: u32 = 0x24aecb7c;
const CLASS_BONE: u32 = 0x95741049;
const CLASS_TRANSFORM: u32 = 0x6350e5a6;
/// `LookAtBoneModifier`: a bone turned to keep aiming at another (the hood following the head).
const CLASS_LOOK_AT: u32 = 0x08d774b2;

/// A look-at modifier of a skeleton: `target` turned so its axis `axis` (0 x, 1 y, 2 z) points at `aim`, `up` its
/// up axis; bones by name hash. Layout (found by Banned445's AC1-Movement-Rewritten, MIT): after the class id, a
/// bone reference (u8 2, u32 object id) to the target, a u8 0 or 1, u8 3, a reference to the aim bone, four u32
/// zeros (an offset), u32 axis, u32 up axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LookAt {
    pub target: u32,
    pub aim: u32,
    pub axis: u8,
    pub up: u8,
}

/// The look-at modifiers in a skeleton's body (a layout other than the one known is left out, not guessed).
pub fn look_ats(body: &[u8], skeleton: &Skeleton) -> Vec<LookAt> {
    let bone = |p: usize| -> Option<u32> {
        (body.get(p) == Some(&2)).then_some(())?;
        let id = u32_at(body, p + 1);
        skeleton.bones.iter().find(|b| b.id == id).map(|b| b.name_hash)
    };
    let word = |p: usize| (p + 4 <= body.len()).then(|| u32_at(body, p));
    let tag = CLASS_LOOK_AT.to_le_bytes();
    let mut out = vec![];
    for p in 5..body.len().saturating_sub(40) {
        if body[p - 5] != 0 || body[p..p + 4] != tag {
            continue;
        }
        let b = p + 4;
        let found = (|| {
            let target = bone(b)?;
            if !matches!(body.get(b + 5), Some(0 | 1)) || body.get(b + 6) != Some(&3) {
                return None;
            }
            let aim = bone(b + 7)?;
            if (0..4).any(|k| word(b + 12 + k * 4) != Some(0)) {
                return None;
            }
            let (axis, up) = (word(b + 28)?, word(b + 32)?);
            (target != aim && axis < 3 && up < 3 && axis != up).then_some(LookAt { target, aim, axis: axis as u8, up: up as u8 })
        })();
        out.extend(found);
    }
    out
}

#[derive(Debug, Clone)]
pub struct Bone {
    pub id: u32,
    pub name_hash: u32,
    /// Resolved name when the hash is in [`BONE_NAMES`].
    pub name: Option<&'static str>,
    pub parent: Option<usize>,
    pub local_pos: [f32; 3],
    pub local_rot: [f32; 4],
    pub model_pos: [f32; 3],
    pub model_rot: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct Skeleton {
    pub bones: Vec<Bone>,
}

/// Bone names recovered by matching CRC32 hashes against strings found in the game data.
pub const BONE_NAMES: &[&str] = &[
    "Reference",
    "Hips",
    "Spine",
    "Spine1",
    "Spine2",
    "Neck",
    "Head",
    "LeftUpLeg",
    "LeftLeg",
    "LeftFoot",
    "RightUpLeg",
    "RightLeg",
    "RightFoot",
    "LeftClavicle",
    "LeftArm",
    "LeftForeArm",
    "LeftHand",
    "RightClavicle",
    "RightArm",
    "RightForeArm",
    "RightHand",
    "LeftHandThumb1",
    "LeftHandThumb2",
    "LeftHandThumb3",
    "LeftHandIndex1",
    "LeftHandIndex2",
    "LeftHandIndex3",
    "LeftHandMiddle1",
    "LeftHandMiddle2",
    "LeftHandMiddle3",
    "LeftHandRing1",
    "LeftHandRing2",
    "LeftHandRing3",
    "LeftHandPinky1",
    "LeftHandPinky2",
    "LeftHandPinky3",
    "RightHandThumb1",
    "RightHandThumb2",
    "RightHandThumb3",
    "RightHandIndex1",
    "RightHandIndex2",
    "RightHandIndex3",
    "RightHandMiddle1",
    "RightHandMiddle2",
    "RightHandMiddle3",
    "RightHandRing1",
    "RightHandRing2",
    "RightHandRing3",
    "RightHandPinky1",
    "RightHandPinky2",
    "RightHandPinky3",
];

pub fn bone_name(hash: u32) -> Option<&'static str> {
    BONE_NAMES.iter().copied().find(|n| crc32fast::hash(n.as_bytes()) == hash)
}

fn f32s<const N: usize>(d: &[u8], o: usize) -> [f32; N] {
    std::array::from_fn(|k| f32::from_le_bytes(d[o + k * 4..o + k * 4 + 4].try_into().unwrap()))
}

pub fn parse_skeleton(body: &[u8]) -> Result<Skeleton> {
    let count = u32_at(body, 4) as usize;
    let tag = CLASS_BONE.to_le_bytes();
    let mut raw = Vec::with_capacity(count);
    let mut i = 8;
    while raw.len() < count {
        let Some(off) = body[i..].windows(4).position(|w| w == tag) else { break };
        let t = i + off;
        i = t + 4;
        ensure!(t >= 5 && body[t - 5] == 0, "bone tag without id at {t:#x}");
        let id = u32_at(body, t - 4);
        let name_hash = u32_at(body, t + 4);
        let mut p = t + 8;
        let parent_id = match body[p] {
            2 => {
                p += 5;
                Some(u32_at(body, p - 4))
            }
            3 => {
                p += 1;
                None
            }
            x => anyhow::bail!("bad parent ref tag {x} at {p:#x}"),
        };
        ensure!(body[p] == 0 && u32_at(body, p + 5) == CLASS_TRANSFORM, "transform missing at {p:#x}");
        let f: [f32; 16] = f32s(body, p + 9);
        raw.push((id, name_hash, parent_id, f));
    }
    ensure!(raw.len() == count, "found {} of {count} bones", raw.len());
    let bones = raw
        .iter()
        .map(|&(id, name_hash, parent_id, f)| Bone {
            id,
            name_hash,
            name: bone_name(name_hash),
            parent: parent_id.and_then(|pid| raw.iter().position(|r| r.0 == pid)),
            model_pos: [f[0], f[1], f[2]],
            model_rot: [f[4], f[5], f[6], f[7]],
            local_pos: [f[8], f[9], f[10]],
            local_rot: [f[12], f[13], f[14], f[15]],
        })
        .collect::<Vec<_>>();
    for (k, b) in bones.iter().enumerate() {
        ensure!(b.parent.is_none_or(|p| p < k), "bone {k} parent not before it");
    }
    Ok(Skeleton { bones })
}

#[cfg(test)]
mod look_at_tests {
    use super::*;

    #[test]
    fn the_look_at_class_id_is_its_names_crc() {
        assert_eq!(crc32fast::hash(b"LookAtBoneModifier"), CLASS_LOOK_AT);
    }

    #[test]
    fn a_look_at_names_its_bones_and_axes() {
        let bone = |id: u32, name_hash: u32| Bone {
            id,
            name_hash,
            name: None,
            parent: None,
            local_pos: [0.0; 3],
            local_rot: [0.0, 0.0, 0.0, 1.0],
            model_pos: [0.0; 3],
            model_rot: [0.0, 0.0, 0.0, 1.0],
        };
        let sk = Skeleton { bones: vec![bone(0x11, 0xaaaa), bone(0x22, 0xbbbb)] };
        let mut body = vec![0u8; 8];
        body.extend([0, 1, 2, 3, 4]); // a zero, then the object's id
        body.extend(CLASS_LOOK_AT.to_le_bytes());
        body.push(2);
        body.extend(0x11u32.to_le_bytes());
        body.extend([1, 3, 2]);
        body.extend(0x22u32.to_le_bytes());
        body.extend([0u8; 16]);
        body.extend(0u32.to_le_bytes());
        body.extend(2u32.to_le_bytes());
        body.extend([0u8; 16]);
        assert_eq!(look_ats(&body, &sk), vec![LookAt { target: 0xaaaa, aim: 0xbbbb, axis: 0, up: 2 }]);
        // A reference to a bone it does not have: left out.
        let other = Skeleton { bones: vec![bone(0x11, 0xaaaa)] };
        assert!(look_ats(&body, &other).is_empty());
    }
}
