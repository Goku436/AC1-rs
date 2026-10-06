//! Multi-bone chain solvers: look-at, FABRIK and CCD.

use crate::{Pose, Rig, limited_arc};
use glam::{Quat, Vec3};

/// One link of a look-at chain (e.g. Spine, Spine1, Spine2, Neck, Head).
#[derive(Debug, Clone, Copy)]
pub struct LookLink {
    pub bone: usize,
    /// Share of the remaining correction this bone takes (0..1).
    pub weight: f32,
    /// Max rotation this bone may contribute, radians.
    pub limit: f32,
}

/// Turn a chain so that `aim_bone`'s `aim_axis` (in that bone's local frame) points at `target`.
///
/// Links are processed root to tip; each one rotates by its share of the remaining error, so the
/// head ends up doing the rest. `weight` fades the whole effect. Returns the leftover angle.
pub fn look_at(pose: &mut Pose, rig: &Rig, links: &[LookLink], aim_bone: usize, aim_axis: Vec3, target: Vec3, weight: f32) -> f32 {
    if weight <= 0.0 {
        return 0.0;
    }
    for link in links {
        let aim = pose.model_of(rig, aim_bone);
        let fwd = aim.rot * aim_axis;
        let want = target - aim.pos;
        let full = limited_arc(fwd, want, std::f32::consts::PI);
        let (axis, angle) = full.to_axis_angle();
        let step = (angle * link.weight * weight).min(link.limit);
        if step > 1e-5 {
            pose.rotate_model(rig, link.bone, Quat::from_axis_angle(axis, step));
        }
    }
    let aim = pose.model_of(rig, aim_bone);
    (aim.rot * aim_axis).angle_between(target - aim.pos)
}

/// Bones from `root` down to `tip` (inclusive), root first. `None` if `tip` is not under `root`.
pub fn chain_between(rig: &Rig, root: usize, tip: usize) -> Option<Vec<usize>> {
    let mut out = vec![tip];
    let mut b = tip;
    while b != root {
        b = rig.parents[b]?;
        out.push(b);
    }
    out.reverse();
    Some(out)
}

/// FABRIK on a bone chain (root first). The root stays put; the tip reaches for `target`.
/// Joint positions are solved first, then each bone is rotated to aim at its solved child.
/// `pole` biases the initial bend so knees/elbows choose a consistent side.
pub fn fabrik(pose: &mut Pose, rig: &Rig, chain: &[usize], target: Vec3, pole: Option<Vec3>, iterations: usize, tolerance: f32) -> f32 {
    let n = chain.len();
    if n < 2 {
        return 0.0;
    }
    let model: Vec<_> = chain.iter().map(|&b| pose.model_of(rig, b)).collect();
    let mut p: Vec<Vec3> = model.iter().map(|m| m.pos).collect();
    let lens: Vec<f32> = (0..n - 1).map(|i| (p[i + 1] - p[i]).length()).collect();
    let root = p[0];

    if let Some(pole) = pole {
        // Nudge interior joints toward the pole so the solve starts on the right side.
        for joint in p.iter_mut().take(n - 1).skip(1) {
            *joint += (pole - *joint).normalize_or_zero() * 0.01;
        }
    }

    let reach: f32 = lens.iter().sum();
    if (target - root).length() >= reach {
        let dir = (target - root).normalize_or_zero();
        for i in 1..n {
            p[i] = p[i - 1] + dir * lens[i - 1];
        }
    } else {
        for _ in 0..iterations {
            p[n - 1] = target;
            for i in (0..n - 1).rev() {
                p[i] = p[i + 1] + (p[i] - p[i + 1]).normalize_or_zero() * lens[i];
            }
            p[0] = root;
            for i in 1..n {
                p[i] = p[i - 1] + (p[i] - p[i - 1]).normalize_or_zero() * lens[i - 1];
            }
            if (p[n - 1] - target).length() < tolerance {
                break;
            }
        }
    }

    // Convert positions back to rotations, root to tip.
    for i in 0..n - 1 {
        let cur = pose.model_of(rig, chain[i]);
        let child = pose.model_of(rig, chain[i + 1]);
        let q = Quat::from_rotation_arc((child.pos - cur.pos).normalize_or_zero(), (p[i + 1] - p[i]).normalize_or_zero());
        // from_rotation_arc of zero vectors is identity-ish; guard against NaN.
        if q.is_finite() {
            pose.rotate_model(rig, chain[i], q);
        }
    }
    (pose.model_of(rig, chain[n - 1]).pos - target).length()
}

/// Cyclic coordinate descent. `limits[i]` caps the per-iteration rotation of `chain[i]`.
pub fn ccd(pose: &mut Pose, rig: &Rig, chain: &[usize], target: Vec3, limits: &[f32], iterations: usize, tolerance: f32) -> f32 {
    let n = chain.len();
    if n < 2 {
        return 0.0;
    }
    let tip = chain[n - 1];
    for _ in 0..iterations {
        for i in (0..n - 1).rev() {
            let joint = pose.model_of(rig, chain[i]).pos;
            let end = pose.model_of(rig, tip).pos;
            let q = limited_arc(end - joint, target - joint, limits.get(i).copied().unwrap_or(std::f32::consts::PI));
            pose.rotate_model(rig, chain[i], q);
        }
        if (pose.model_of(rig, tip).pos - target).length() < tolerance {
            break;
        }
    }
    (pose.model_of(rig, tip).pos - target).length()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_rig;

    #[test]
    fn look_at_turns_head() {
        let rig = test_rig();
        let mut pose = rig.rest_pose();
        let links = [
            LookLink { bone: 5, weight: 0.2, limit: 0.3 },
            LookLink { bone: 6, weight: 0.3, limit: 0.4 },
            LookLink { bone: 7, weight: 0.5, limit: 0.6 },
            LookLink { bone: 8, weight: 1.0, limit: 1.0 },
        ];
        let target = Vec3::new(2.0, 2.0, 1.8);
        let left = look_at(&mut pose, &rig, &links, 8, Vec3::X, target, 1.0);
        assert!(left < 0.01, "leftover {left}");
        // Spine took part, not only the head.
        assert!(pose.local[5].rot.angle_between(Quat::IDENTITY) > 0.05);
    }

    #[test]
    fn look_at_respects_limits() {
        let rig = test_rig();
        let mut pose = rig.rest_pose();
        let links = [LookLink { bone: 8, weight: 1.0, limit: 0.5 }];
        let left = look_at(&mut pose, &rig, &links, 8, Vec3::X, Vec3::new(-2.0, 0.0, 1.8), 1.0);
        assert!(left > 2.0);
        assert!((pose.local[8].rot.angle_between(Quat::IDENTITY) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn fabrik_and_ccd_reach() {
        let rig = test_rig();
        let chain = chain_between(&rig, 1, 4).unwrap();
        assert_eq!(chain, vec![1, 2, 3, 4]);
        let target = Vec3::new(0.3, 0.2, 0.3);
        let mut a = rig.rest_pose();
        let e = fabrik(&mut a, &rig, &chain, target, Some(Vec3::new(1.0, 0.0, 0.5)), 32, 1e-4);
        assert!(e < 1e-3, "fabrik error {e}");
        let mut b = rig.rest_pose();
        let e = ccd(&mut b, &rig, &chain, target, &[1.0; 4], 64, 1e-4);
        assert!(e < 1e-3, "ccd error {e}");
    }
}
