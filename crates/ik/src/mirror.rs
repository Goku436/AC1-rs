//! Left/right mirroring of poses, to play a one-sided clip on the other side.
//!
//! A pose is mirrored across the model's sagittal plane (Y = 0: AC1 rigs face +X with their left at
//! +Y, in the rest pose and in animated poses alike). Each bone takes its mirror bone's deformation,
//! reflected: with `S = M * Bind⁻¹` a bone's skinning delta and `R` the reflection,
//! `M'_b = R * S_m * R * Bind_b`, which is a proper rigid transform again when the rest pose is
//! symmetric. Local transforms are rebuilt from the mirrored model transforms.

use crate::{Pose, Rig, Xform};
use glam::{Mat4, Vec3};

/// Mirror bones and rest (bind) model transforms of a rig.
#[derive(Debug, Clone)]
pub struct Mirror {
    /// Each bone's mirror image (itself for bones on the centre line).
    pub pairs: Vec<usize>,
    bind: Vec<Xform>,
}

/// Reflection across Y = 0.
const FLIP: Vec3 = Vec3::new(1.0, -1.0, 1.0);

fn mat(x: &Xform) -> Mat4 {
    Mat4::from_rotation_translation(x.rot, x.pos)
}

fn xform(m: Mat4) -> Xform {
    let (_, rot, pos) = m.to_scale_rotation_translation();
    Xform { pos, rot: rot.normalize() }
}

impl Mirror {
    /// Pair bones by name (`Left*` with `Right*`), then the unnamed ones by mirrored rest position
    /// (preferring a bone whose parent is the mirror of this one's parent).
    pub fn new(rig: &Rig) -> Self {
        let bind = rig.rest_pose().model(rig);
        let n = rig.len();
        let mut pairs: Vec<usize> = (0..n).collect();
        for (i, pair) in pairs.iter_mut().enumerate() {
            let Some(name) = rig.names[i].as_deref() else { continue };
            let other = if let Some(s) = name.strip_prefix("Left") {
                format!("Right{s}")
            } else if let Some(s) = name.strip_prefix("Right") {
                format!("Left{s}")
            } else {
                continue;
            };
            if let Some(j) = rig.find(&other) {
                *pair = j;
            }
        }
        for i in 0..n {
            let named = rig.names[i].as_deref().is_some_and(|s| !s.starts_with('#'));
            let p = bind[i].pos;
            if named || p.y.abs() < 0.02 {
                continue;
            }
            let want = p * FLIP;
            let parent_pair = rig.parents[i].map(|q| pairs[q]);
            let best = (0..n)
                .filter(|&j| j != i && (bind[j].pos - want).length() < 0.02)
                .min_by_key(|&j| (rig.parents[j] != parent_pair, ((bind[j].pos - want).length() * 1e4) as i32));
            if let Some(j) = best {
                pairs[i] = j;
            }
        }
        Self { pairs, bind }
    }

    /// `pose` mirrored left to right.
    pub fn apply(&self, rig: &Rig, pose: &Pose) -> Pose {
        let model = pose.model(rig);
        let r = Mat4::from_scale(FLIP);
        let mirrored: Vec<Xform> =
            (0..rig.len()).map(|b| xform(r * mat(&model[self.pairs[b]]) * mat(&self.bind[self.pairs[b]]).inverse() * r * mat(&self.bind[b]))).collect();
        let local = (0..rig.len()).map(|b| rig.parents[b].map_or(mirrored[b], |p| mirrored[p].inverse().mul(&mirrored[b]))).collect();
        Pose { local }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Quat;

    /// Spine with a left and a right arm (rest pose symmetric across Y = 0).
    fn rig() -> Rig {
        Rig {
            parents: vec![None, Some(0), Some(0), Some(1), Some(2)],
            names: ["Hips", "LeftArm", "RightArm", "LeftHand", "RightHand"].map(|s| Some(s.to_string())).to_vec(),
            rest: vec![
                Xform::new(Vec3::new(0.0, 0.0, 1.0), Quat::IDENTITY),
                Xform::new(Vec3::new(0.0, 0.2, 0.4), Quat::IDENTITY),
                Xform::new(Vec3::new(0.0, -0.2, 0.4), Quat::IDENTITY),
                Xform::new(Vec3::new(0.0, 0.3, 0.0), Quat::IDENTITY),
                Xform::new(Vec3::new(0.0, -0.3, 0.0), Quat::IDENTITY),
            ],
        }
    }

    #[test]
    fn pairs_by_name() {
        let m = Mirror::new(&rig());
        assert_eq!(m.pairs, vec![0, 2, 1, 4, 3]);
    }

    #[test]
    fn raised_left_arm_becomes_raised_right_arm() {
        let rig = rig();
        let m = Mirror::new(&rig);
        let mut pose = rig.rest_pose();
        // Left arm up (rotate about +X so the hand at +Y swings up to +Z), hips turned a little left.
        pose.local[1].rot = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        pose.local[0].rot = Quat::from_rotation_z(0.3);
        let out = m.apply(&rig, &pose).model(&rig);
        let before = pose.model(&rig);
        for (b, mb) in [(0, 0), (1, 2), (2, 1), (3, 4), (4, 3)] {
            let want = before[mb].pos * FLIP;
            assert!((out[b].pos - want).length() < 1e-4, "bone {b}: {} vs {want}", out[b].pos);
        }
        // Mirroring twice gives the pose back.
        let back = m.apply(&rig, &m.apply(&rig, &pose)).model(&rig);
        for (a, b) in back.iter().zip(&before) {
            assert!((a.pos - b.pos).length() < 1e-4);
            assert!(a.rot.dot(b.rot).abs() > 0.9999);
        }
    }
}
