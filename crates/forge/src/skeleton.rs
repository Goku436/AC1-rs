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
