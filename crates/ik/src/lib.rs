//! Skeleton pose math and IK solvers for the AC1 runtime.
//!
//! Everything here works in *model space* (the skeleton's own frame: Z up, facing +X for AC1 rigs).
//! Callers convert world-space targets with the character's inverse transform.
//!
//! Solvers edit a [`Pose`] (local transforms) so children always follow their parents; each solver
//! recomputes the model-space transforms it needs.

pub mod chain;
pub mod cloth;
pub mod mirror;
pub mod placement;
pub mod ragdoll;

use glam::{Quat, Vec3};

/// Rigid transform: rotate, then translate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xform {
    pub pos: Vec3,
    pub rot: Quat,
}

impl Default for Xform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Xform {
    pub const IDENTITY: Self = Self { pos: Vec3::ZERO, rot: Quat::IDENTITY };

    pub fn new(pos: Vec3, rot: Quat) -> Self {
        Self { pos, rot }
    }
    /// `self * child`: express `child` (given in self's frame) in self's parent frame.
    pub fn mul(&self, child: &Xform) -> Xform {
        Xform { pos: self.pos + self.rot * child.pos, rot: (self.rot * child.rot).normalize() }
    }
    pub fn inverse(&self) -> Xform {
        let r = self.rot.inverse();
        Xform { pos: r * -self.pos, rot: r }
    }
    pub fn point(&self, p: Vec3) -> Vec3 {
        self.pos + self.rot * p
    }
}

/// Static description of a skeleton: hierarchy, names and rest (bind) pose.
#[derive(Debug, Clone)]
pub struct Rig {
    pub parents: Vec<Option<usize>>,
    pub names: Vec<Option<String>>,
    pub rest: Vec<Xform>,
}

impl Rig {
    pub fn len(&self) -> usize {
        self.parents.len()
    }
    pub fn is_empty(&self) -> bool {
        self.parents.is_empty()
    }
    pub fn find(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|n| n.as_deref() == Some(name))
    }
    pub fn rest_pose(&self) -> Pose {
        Pose { local: self.rest.clone() }
    }
}

/// Local (parent-relative) transform per bone.
#[derive(Debug, Clone, PartialEq)]
pub struct Pose {
    pub local: Vec<Xform>,
}

impl Pose {
    /// Model-space transforms. Parents must precede children (true for AC1 skeletons).
    pub fn model(&self, rig: &Rig) -> Vec<Xform> {
        let mut out: Vec<Xform> = Vec::with_capacity(self.local.len());
        for (i, l) in self.local.iter().enumerate() {
            let m = match rig.parents[i] {
                Some(p) => out[p].mul(l),
                None => *l,
            };
            out.push(m);
        }
        out
    }

    /// Model-space transform of one bone (walks up the parents).
    pub fn model_of(&self, rig: &Rig, bone: usize) -> Xform {
        let mut x = self.local[bone];
        let mut b = bone;
        while let Some(p) = rig.parents[b] {
            x = self.local[p].mul(&x);
            b = p;
        }
        x
    }

    /// Set a bone's model-space rotation, keeping its parent unchanged.
    pub fn set_model_rot(&mut self, rig: &Rig, bone: usize, rot: Quat) {
        let parent_rot = rig.parents[bone].map_or(Quat::IDENTITY, |p| self.model_of(rig, p).rot);
        self.local[bone].rot = (parent_rot.inverse() * rot).normalize();
    }

    /// Rotate a bone about its own pivot by a model-space delta.
    pub fn rotate_model(&mut self, rig: &Rig, bone: usize, delta: Quat) {
        let m = self.model_of(rig, bone);
        self.set_model_rot(rig, bone, delta * m.rot);
    }

    /// Move a bone (and everything under it) by a model-space offset.
    pub fn translate_model(&mut self, rig: &Rig, bone: usize, offset: Vec3) {
        let parent_rot = rig.parents[bone].map_or(Quat::IDENTITY, |p| self.model_of(rig, p).rot);
        self.local[bone].pos += parent_rot.inverse() * offset;
    }

    /// Per-bone blend: `self = lerp(self, other, w)`.
    pub fn blend(&mut self, other: &Pose, w: f32) {
        for (a, b) in self.local.iter_mut().zip(&other.local) {
            a.pos = a.pos.lerp(b.pos, w);
            a.rot = a.rot.slerp(b.rot, w).normalize();
        }
    }
}

/// Rotation that takes direction `from` to `to`, limited to `max_angle` radians.
pub fn limited_arc(from: Vec3, to: Vec3, max_angle: f32) -> Quat {
    let (Some(f), Some(t)) = (from.try_normalize(), to.try_normalize()) else { return Quat::IDENTITY };
    let q = Quat::from_rotation_arc(f, t);
    let (axis, angle) = q.to_axis_angle();
    if angle <= max_angle { q } else { Quat::from_axis_angle(axis, max_angle) }
}

/// Analytic two-bone IK (hip-knee-ankle, shoulder-elbow-wrist).
///
/// Rotates `upper` and `lower` so `end` reaches `target` (clamped to reach), bending toward
/// `pole` (a model-space point the middle joint should point at). `end` keeps its model rotation
/// unless `end_rot` is given. `weight` blends from the input pose.
/// Returns how far `end` still is from the target (0 when reachable).
#[allow(clippy::too_many_arguments)]
pub fn two_bone_ik(
    pose: &mut Pose,
    rig: &Rig,
    upper: usize,
    lower: usize,
    end: usize,
    target: Vec3,
    pole: Option<Vec3>,
    end_rot: Option<Quat>,
    weight: f32,
) -> f32 {
    if weight <= 0.0 {
        return (pose.model_of(rig, end).pos - target).length();
    }
    let before = pose.clone();
    let (ma, mb, mc) = (pose.model_of(rig, upper), pose.model_of(rig, lower), pose.model_of(rig, end));
    let (a, b, c) = (ma.pos, mb.pos, mc.pos);
    let end_rot = end_rot.unwrap_or(mc.rot);

    let lab = (b - a).length();
    let lcb = (c - b).length();
    let eps = 1e-4;
    let lat = (target - a).length().clamp(eps, lab + lcb - eps);

    let acos = |x: f32| x.clamp(-1.0, 1.0).acos();
    let ac = (c - a).normalize_or_zero();
    let ab = (b - a).normalize_or_zero();
    let bc = (c - b).normalize_or_zero();
    let at = (target - a).normalize_or_zero();

    let ac_ab_0 = acos(ac.dot(ab));
    let ba_bc_0 = acos((-ab).dot(bc));
    let ac_at_0 = acos(ac.dot(at));
    let ac_ab_1 = acos((lcb * lcb - lab * lab - lat * lat) / (-2.0 * lab * lat));
    let ba_bc_1 = acos((lat * lat - lab * lab - lcb * lcb) / (-2.0 * lab * lcb));

    // Bend axis: current bend plane, else the pole's plane, else any perpendicular.
    let bend_hint = pole.map(|p| p - a).unwrap_or(b - a);
    let axis0 = ac.cross(b - a).try_normalize().or_else(|| ac.cross(bend_hint).try_normalize()).unwrap_or_else(|| ac.any_orthonormal_vector());
    let axis1 = ac.cross(at).try_normalize().unwrap_or(axis0);

    let r0 = Quat::from_axis_angle(axis0, ac_ab_1 - ac_ab_0);
    let r1 = Quat::from_axis_angle(axis0, ba_bc_1 - ba_bc_0);
    let r2 = Quat::from_axis_angle(axis1, ac_at_0);

    let mut new_a = r2 * r0 * ma.rot;
    let mut new_b = r2 * r1 * r0 * mb.rot;

    // Swing the chain about the root->target axis so the middle joint faces the pole.
    if let Some(p) = pole {
        let axis = at;
        let mid = a + new_a * (ma.rot.inverse() * (b - a));
        let proj = |v: Vec3| v - axis * v.dot(axis);
        if let (Some(m), Some(pp)) = (proj(mid - a).try_normalize(), proj(p - a).try_normalize()) {
            let ang = m.cross(pp).dot(axis).atan2(m.dot(pp));
            let twist = Quat::from_axis_angle(axis, ang);
            new_a = twist * new_a;
            new_b = twist * new_b;
        }
    }

    pose.set_model_rot(rig, upper, new_a.normalize());
    pose.set_model_rot(rig, lower, new_b.normalize());
    pose.set_model_rot(rig, end, end_rot);
    if weight < 1.0 {
        let mut blended = before;
        for &bone in &[upper, lower, end] {
            blended.local[bone].rot = blended.local[bone].rot.slerp(pose.local[bone].rot, weight);
        }
        *pose = blended;
    }
    (pose.model_of(rig, end).pos - target).length()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Slightly bent 2-bone leg + foot along -Z, plus a spine/neck/head chain.
    pub fn test_rig() -> Rig {
        let names = ["root", "hip", "knee", "ankle", "toe", "spine", "spine1", "neck", "head"];
        let parents = vec![None, Some(0), Some(1), Some(2), Some(3), Some(0), Some(5), Some(6), Some(7)];
        let p = |x, y, z| Xform::new(Vec3::new(x, y, z), Quat::IDENTITY);
        let rest = vec![
            p(0.0, 0.0, 1.0),
            p(0.0, 0.1, 0.0),
            p(0.02, 0.0, -0.45),
            p(-0.02, 0.0, -0.45),
            p(0.12, 0.0, -0.08),
            p(0.0, 0.0, 0.1),
            p(0.0, 0.0, 0.25),
            p(0.0, 0.0, 0.25),
            p(0.0, 0.0, 0.1),
        ];
        Rig { parents, names: names.iter().map(|s| Some(s.to_string())).collect(), rest }
    }

    #[test]
    fn xform_inverse_roundtrip() {
        let x = Xform::new(Vec3::new(1.0, 2.0, 3.0), Quat::from_rotation_y(0.7) * Quat::from_rotation_x(0.3));
        let i = x.mul(&x.inverse());
        assert!(i.pos.length() < 1e-5 && i.rot.angle_between(Quat::IDENTITY) < 1e-3);
    }

    #[test]
    fn two_bone_reaches_target_and_keeps_lengths() {
        let rig = test_rig();
        let mut pose = rig.rest_pose();
        let rest = pose.model(&rig);
        let target = Vec3::new(0.25, 0.2, 0.35);
        let pole = Vec3::new(1.0, 0.1, 0.5);
        let err = two_bone_ik(&mut pose, &rig, 1, 2, 3, target, Some(pole), None, 1.0);
        assert!(err < 1e-3, "error {err}");
        let m = pose.model(&rig);
        for (a, b) in [(1, 2), (2, 3), (3, 4)] {
            let l0 = (rest[a].pos - rest[b].pos).length();
            let l1 = (m[a].pos - m[b].pos).length();
            assert!((l0 - l1).abs() < 1e-4);
        }
        let knee_dir = m[2].pos - (m[1].pos + m[3].pos) * 0.5;
        assert!(knee_dir.x > 0.0, "knee bends away from pole: {knee_dir}");
        assert!(m[3].rot.angle_between(rest[3].rot) < 1e-3, "foot rotation changed");
    }

    #[test]
    fn two_bone_unreachable_points_at_target() {
        let rig = test_rig();
        let mut pose = rig.rest_pose();
        let target = Vec3::new(3.0, 0.1, 1.0);
        let err = two_bone_ik(&mut pose, &rig, 1, 2, 3, target, None, None, 1.0);
        let m = pose.model(&rig);
        let dir = (m[3].pos - m[1].pos).normalize();
        assert!(dir.dot((target - m[1].pos).normalize()) > 0.999);
        assert!(err > 1.0);
    }

    #[test]
    fn two_bone_straight_chain_uses_pole() {
        let mut rig = test_rig();
        rig.rest[2].pos = Vec3::new(0.0, 0.0, -0.45);
        rig.rest[3].pos = Vec3::new(0.0, 0.0, -0.45);
        let mut pose = rig.rest_pose();
        let err = two_bone_ik(&mut pose, &rig, 1, 2, 3, Vec3::new(0.0, 0.1, 0.4), Some(Vec3::new(1.0, 0.1, 0.5)), None, 1.0);
        assert!(err < 1e-3, "error {err}");
        let m = pose.model(&rig);
        assert!(m[2].pos.x > 0.05, "knee did not bend toward pole: {}", m[2].pos);
    }
}
