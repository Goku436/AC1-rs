//! AC1's secondary motion: a skeleton's bone modifiers run over the animated pose every frame (its
//! `SkeletonComponent`'s list, read by `forge::modifier`). Hinges swing a bone about one axis under gravity, the
//! bone's own motion and the wind, inside hard and soft angle limits (the skirt's panels, the hood's sides, the
//! sword tag); look-ats aim a bone at another (the hood at the head); compressions put a bone between two others;
//! rotation copies give a bone another's rotation; roll bones share a bone's twist; a spring box lets a bone's end
//! jiggle inside a box.
//!
//! The order is AC1's: by the bone's depth in the skeleton, parents first, then the bone list's order, then the
//! order written. How each behaves is as Banned445's AC1-Movement-Rewritten (MIT, `src/visual_pose.rs`,
//! `src/skirt_hinge.rs`) recreated it; written fresh here.
//!
//! Poses are in model space; `frame` places the model in the world (Z up), where the hinges and springs move.

use crate::{Pose, Rig, Xform};
use glam::{EulerRot, Mat3, Quat, Vec3};

#[derive(Debug, Clone, Copy)]
pub struct Limit {
    /// The bone whose frame the limit is measured from (none: the hinge's parent).
    pub reference: Option<usize>,
    pub min: f32,
    pub max: f32,
    pub soft_min: f32,
    pub soft_max: f32,
}

#[derive(Debug, Clone)]
pub struct Hinge {
    pub target: usize,
    pub inertia: f32,
    pub damping: f32,
    pub force: Vec3,
    pub force_reference: Option<usize>,
    pub wind: f32,
    pub axis: usize,
    pub direction: usize,
    pub limits: Vec<Limit>,
}

#[derive(Debug, Clone, Copy)]
pub struct Compress {
    pub target: usize,
    pub sources: [usize; 2],
    pub position_weight: f32,
    pub rotation_weight: f32,
    pub rotation_offsets: [Quat; 2],
    pub position_offsets: [Vec3; 2],
    pub rotation: bool,
    pub position: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Spring {
    pub target: usize,
    pub stiffness: f32,
    pub damping: f32,
    pub wind: f32,
    pub force: Vec3,
    pub min: Vec3,
    pub max: Vec3,
}

#[derive(Debug, Clone)]
pub enum Modifier {
    /// `target`'s axis `axis` aimed at `aim`, its axis `up` toward its current Z.
    LookAt {
        target: usize,
        aim: usize,
        axis: usize,
        up: usize,
    },
    Hinge(Hinge),
    Compress(Compress),
    /// `target` takes `source`'s rotation.
    Copy {
        target: usize,
        source: usize,
    },
    /// `target`'s twist (its middle Y-X-Z angle) from `source`'s: modes 0 and 2 counter it by 1 - weight, 1 and 3
    /// follow it a quarter turn on (+ for 1, - for 3), mirrored past a quarter turn, times the weight.
    Roll {
        target: usize,
        source: usize,
        mode: u32,
        weight: f32,
    },
    Spring(Spring),
}

impl Modifier {
    pub fn target(&self) -> usize {
        match self {
            Modifier::LookAt { target, .. } | Modifier::Copy { target, .. } | Modifier::Roll { target, .. } => *target,
            Modifier::Hinge(h) => h.target,
            Modifier::Compress(c) => c.target,
            Modifier::Spring(s) => s.target,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct HingeState {
    /// Last frame's solved world frame (rotation, position).
    cached: Option<(Quat, Vec3)>,
    velocity: f32,
}

#[derive(Debug, Clone, Default)]
struct SpringState {
    live: bool,
    pos: Vec3,
    prev: Vec3,
    anchor: Vec3,
}

pub struct Secondary {
    modifiers: Vec<Modifier>,
    /// Per limit of each hinge: the hinge bone's swung axis in the reference bone's rest frame.
    reference_dirs: Vec<Vec<Vec3>>,
    hinges: Vec<HingeState>,
    springs: Vec<SpringState>,
    /// Bones put back to their rest pose before the modifiers run (the add-on skeletons' modified bones: nothing
    /// animates them, and a modifier must not read its own last output).
    reset: Vec<usize>,
    last_anchor: Option<Vec3>,
}

fn world_of(pose: &[Xform], frame: Xform) -> Vec<Xform> {
    pose.iter().map(|x| frame.mul(x)).collect()
}

fn axis_of(q: Quat, k: usize) -> Vec3 {
    q * [Vec3::X, Vec3::Y, Vec3::Z][k]
}

/// The signed angle about `axis` from `a` to `b`, both seen along the axis (0 when either lies along it).
fn angle_about(a: Vec3, b: Vec3, axis: Vec3) -> f32 {
    if (a.dot(axis) - 1.0).abs() <= 0.0005 || (b.dot(axis) - 1.0).abs() <= 0.0005 {
        return 0.0;
    }
    let (Some(pa), Some(pb)) = ((a - axis * a.dot(axis)).try_normalize(), (b - axis * b.dot(axis)).try_normalize()) else { return 0.0 };
    let d = pa.dot(pb);
    if d >= 1.0 {
        return 0.0;
    }
    if d <= -1.0 {
        return std::f32::consts::PI;
    }
    if pa.cross(pb).dot(axis) < 0.0 { -d.acos() } else { d.acos() }
}

/// The limits in turn, each narrowed through its reference's offset from the one before: a swing past a hard limit
/// stops there (and its speed outward goes), in a soft zone its speed outward falls with the room left.
fn limit(limits: &[Limit], offsets: &[f32], mut angle: f32, mut v: f32, dt: f32) -> (f32, f32) {
    let Some(first) = limits.first() else { return (angle, v) };
    let (mut lo, mut hi) = (first.min, first.max);
    for (k, l) in limits.iter().enumerate() {
        let off = offsets[k];
        let mut at = v * dt + off + angle;
        let (lk, hk) = if k == 0 { (lo, hi) } else { ((off + lo).max(l.min), (off + hi).min(l.max)) };
        let before = v;
        if at < lk {
            at = lk;
            if before <= 0.0 {
                v = 0.0;
            }
        } else if at < lk + l.soft_min {
            if before < 0.0 {
                v = (at - lk) * before / l.soft_min;
            }
        } else if at > hk {
            at = hk;
            if before >= 0.0 {
                v = 0.0;
            }
        } else if at > hk - l.soft_max && before >= 0.0 {
            v = (hk - at) * before / l.soft_max;
        }
        angle = at - off;
        lo = lk - off;
        hi = hk - off;
    }
    (angle, v)
}

impl Secondary {
    /// From the modifiers in authored order (each skeleton's in turn), on `rig`; `main` is the main skeleton's bone
    /// count (bones past it belong to add-on skeletons).
    pub fn new(rig: &Rig, authored: Vec<Modifier>, main: usize) -> Self {
        let depth = |mut i: usize| {
            let mut d = 0;
            while let Some(p) = rig.parents[i] {
                d += 1;
                i = p;
            }
            d
        };
        let mut keyed: Vec<(usize, usize, usize, Modifier)> = authored.into_iter().enumerate().map(|(k, m)| (depth(m.target()), m.target(), k, m)).collect();
        keyed.sort_by_key(|&(d, t, k, _)| (d, t, k));
        let modifiers: Vec<Modifier> = keyed.into_iter().map(|k| k.3).collect();
        let rest = rig.rest_pose().model(rig);
        let reference_dirs = modifiers
            .iter()
            .map(|m| match m {
                Modifier::Hinge(h) => {
                    let swung = axis_of(rest[h.target].rot, h.direction);
                    h.limits.iter().map(|l| l.reference.map_or(swung, |r| rest[r].rot.inverse() * swung)).collect()
                }
                _ => vec![],
            })
            .collect();
        let mut reset: Vec<usize> = modifiers.iter().map(Modifier::target).filter(|&t| t >= main).collect();
        reset.sort_unstable();
        reset.dedup();
        let n = modifiers.len();
        Secondary { modifiers, reference_dirs, hinges: vec![HingeState::default(); n], springs: vec![SpringState::default(); n], reset, last_anchor: None }
    }

    pub fn len(&self) -> usize {
        self.modifiers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.modifiers.is_empty()
    }

    /// Run every modifier over `pose` for a frame of `dt`. `frame` puts the model in the world (Z up), `wind` is
    /// the force there (world), `gravity` the world's down pull (m/s²: hinge forces are authored in the model's
    /// frame). A jump of the model of over 2 m, or a long frame, restarts the swinging.
    pub fn apply(&mut self, pose: &mut Pose, rig: &Rig, frame: Xform, wind: Vec3, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        for &b in &self.reset {
            pose.local[b] = rig.rest[b];
        }
        let anchor = frame.pos;
        let restart = dt > 0.1 || self.last_anchor.is_none_or(|a| a.distance_squared(anchor) > 4.0);
        self.last_anchor = Some(anchor);
        let mut last_target = None;
        let mut repeat = 0;
        for i in 0..self.modifiers.len() {
            let target = self.modifiers[i].target();
            repeat = if last_target == Some(target) { repeat + 1 } else { 0 };
            last_target = Some(target);
            match self.modifiers[i].clone() {
                Modifier::LookAt { target, aim, axis, up } => {
                    let m = pose.model(rig);
                    let Some(dir) = (m[aim].pos - m[target].pos).try_normalize() else { continue };
                    let Some(side) = axis_of(m[target].rot, 2).cross(dir).try_normalize() else { continue };
                    let mut cols = [Vec3::ZERO; 3];
                    cols[axis] = dir;
                    cols[up] = dir.cross(side);
                    let third = 3 - axis - up;
                    cols[third] = cols[(third + 1) % 3].cross(cols[(third + 2) % 3]);
                    pose.set_model_rot(rig, target, Quat::from_mat3(&Mat3::from_cols(cols[0], cols[1], cols[2])));
                }
                Modifier::Copy { target, source } => {
                    let rot = pose.model_of(rig, source).rot;
                    pose.set_model_rot(rig, target, rot);
                }
                Modifier::Compress(c) => {
                    let m = pose.model(rig);
                    let src = |k: usize| (m[c.sources[k]].rot * c.rotation_offsets[k], m[c.sources[k]].point(c.position_offsets[k]));
                    let ((ra, pa), (rb, pb)) = (src(0), src(1));
                    let parent = rig.parents[c.target].map_or(Xform::IDENTITY, |p| m[p]);
                    if c.position {
                        pose.local[c.target].pos = parent.inverse().point(pb.lerp(pa, c.position_weight));
                    }
                    if c.rotation {
                        // Shortest way from B toward A.
                        let ra = if rb.dot(ra) < 0.0 { -ra } else { ra };
                        pose.local[c.target].rot = (parent.rot.inverse() * rb.slerp(ra, c.rotation_weight)).normalize();
                    }
                }
                Modifier::Roll { target, source, mode, weight } => {
                    let (sa, sb, sc) = pose.local[source].rot.to_euler(EulerRot::YXZ);
                    let (a, _, c) = pose.local[target].rot.to_euler(EulerRot::YXZ);
                    let quarter = std::f32::consts::FRAC_PI_2;
                    let b = match mode {
                        0 | 2 => -sb * (1.0 - weight),
                        _ => {
                            let on = if mode == 3 { sb - quarter } else { sb + quarter };
                            (if sc.abs() > quarter || sa.abs() > quarter { -on } else { on }) * weight
                        }
                    };
                    pose.local[target].rot = Quat::from_euler(EulerRot::YXZ, a, b, c);
                }
                Modifier::Hinge(h) => {
                    let world = world_of(&pose.model(rig), frame);
                    let parent = rig.parents[h.target].map_or(frame, |p| world[p]);
                    let base = parent.mul(&rig.rest[h.target]);
                    let owner = world[h.target];
                    let force = h.force_reference.map_or(frame.rot * h.force, |r| world[r].rot * h.force);
                    let refs: Vec<Option<Vec3>> = h.limits.iter().zip(&self.reference_dirs[i]).map(|(l, d)| l.reference.map(|r| world[r].rot * *d)).collect();
                    let solved = self.hinge(i, &h, base, owner, force, &refs, wind, dt, restart);
                    // A second modifier on the same bone works from the first's result.
                    let applied = if repeat > 0 { owner.mul(&base.inverse().mul(&solved)) } else { solved };
                    pose.local[h.target] = parent.inverse().mul(&applied);
                }
                Modifier::Spring(s) => {
                    let m = pose.model(rig);
                    let parent = frame.mul(&rig.parents[s.target].map_or(Xform::IDENTITY, |p| m[p]));
                    let rest = rig.rest[s.target].pos;
                    let anchor = parent.point(rest);
                    let st = &mut self.springs[i];
                    if restart || !st.live {
                        let at = rest.clamp(s.min, s.max);
                        pose.local[s.target].pos = at;
                        let w = parent.point(at);
                        *st = SpringState { live: true, pos: w, prev: w, anchor };
                        continue;
                    }
                    let v = ((st.pos - st.prev) - (anchor - st.anchor)) / dt;
                    let off = st.pos - anchor;
                    let mut total = frame.rot * s.force + wind * s.wind;
                    let d = off.length();
                    if d > 0.0 {
                        let n = off / d;
                        let radial = n.dot(v);
                        total += -(v - n * radial) * s.damping - n * (s.stiffness * d + radial * s.damping);
                    }
                    let next = total * dt * dt + st.pos * 2.0 - st.prev;
                    let local = parent.inverse().point(next).clamp(s.min, s.max);
                    pose.local[s.target].pos = local;
                    st.prev = st.pos;
                    st.pos = parent.point(local);
                    st.anchor = anchor;
                }
            }
        }
    }

    /// One hinge for a frame: its swing speed about the axis gains from the bone's own motion (`inertia`), the force
    /// and the wind, loses `damping` while it goes against the force, and is held by the limits. On a restart the
    /// speed is dropped and three longer steps settle it.
    #[allow(clippy::too_many_arguments)]
    fn hinge(&mut self, i: usize, h: &Hinge, base: Xform, owner: Xform, force: Vec3, refs: &[Option<Vec3>], wind: Vec3, dt: f32, restart: bool) -> Xform {
        let st = &mut self.hinges[i];
        let go_on = st.cached.is_some() && !restart;
        if !go_on {
            st.cached = Some((owner.rot, owner.pos));
            st.velocity = 0.0;
        }
        let inertia = match st.cached {
            Some((_, p)) if go_on => (p - base.pos) * (h.inertia / dt),
            _ => Vec3::ZERO,
        };
        let step = if go_on { dt } else { dt * 5.0 };
        let mut solved = base;
        for _ in 0..if go_on { 1 } else { 3 } {
            let (rot, _) = st.cached.unwrap_or((owner.rot, owner.pos));
            let axis = axis_of(rot, h.axis);
            let dir = axis_of(rot, h.direction);
            let base_dir = axis_of(base.rot, h.direction);
            let angle = angle_about(base_dir, dir, axis);
            let torque = dir.cross(force).dot(axis);
            st.velocity += dir.cross(inertia + force * step + wind * h.wind).dot(axis);
            if torque * st.velocity < 0.0 {
                st.velocity *= 1.0 - h.damping;
            }
            let offsets: Vec<f32> = refs.iter().map(|r| r.map_or(0.0, |r| -angle_about(base_dir, r, axis))).collect();
            let (angle, v) = limit(&h.limits, &offsets, angle, st.velocity, step);
            st.velocity = v;
            solved = Xform::new(base.pos, (base.rot * Quat::from_axis_angle([Vec3::X, Vec3::Y, Vec3::Z][h.axis], angle)).normalize());
            st.cached = Some((solved.rot, solved.pos));
        }
        solved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rig(n: usize) -> Rig {
        Rig {
            parents: (0..n).map(|i| i.checked_sub(1)).collect(),
            names: vec![None; n],
            rest: (0..n).map(|i| Xform::new(Vec3::X * (i as f32) * 0.3, Quat::IDENTITY)).collect(),
        }
    }

    fn hinge(limits: Vec<Limit>) -> Hinge {
        Hinge { target: 1, inertia: 0.2, damping: 0.5, force: Vec3::new(0.0, 0.0, -9.8), force_reference: None, wind: 0.0, axis: 1, direction: 0, limits }
    }

    #[test]
    fn soft_zones_slow_only_outward_swings() {
        let l = |soft| vec![Limit { reference: None, min: -1.0, max: 1.0, soft_min: soft, soft_max: soft }];
        let v = |angle, v, soft| limit(&l(soft), &[0.0], angle, v, 0.0).1;
        assert_eq!(v(-0.75, -4.0, 0.5), -2.0);
        assert_eq!(v(-0.75, 4.0, 0.5), 4.0);
        assert_eq!(v(0.75, 4.0, 0.5), 2.0);
        assert_eq!(v(0.75, -4.0, 0.5), -4.0);
        assert_eq!(v(-1.1, -4.0, 0.0), 0.0);
        assert_eq!(v(1.1, 4.0, 0.0), 0.0);
        assert_eq!(v(0.0, 4.0, 0.0), 4.0);
    }

    #[test]
    fn a_hinge_hangs_under_gravity_within_its_limits() {
        // A bone pointing along X, hinged about Y: gravity swings its X toward -Z, up to 0.6 rad.
        let r = rig(2);
        let mut s = Secondary::new(&r, vec![Modifier::Hinge(hinge(vec![Limit { reference: None, min: -0.6, max: 0.6, soft_min: 0.0, soft_max: 0.0 }]))], 0);
        let mut pose = r.rest_pose();
        for _ in 0..120 {
            pose = r.rest_pose();
            s.apply(&mut pose, &r, Xform::IDENTITY, Vec3::ZERO, 1.0 / 60.0);
        }
        let x = pose.model(&r)[1].rot * Vec3::X;
        assert!(x.z < -0.5 && x.z > -0.57 - 0.01, "{x}");
        assert!(pose.local[1].pos.distance(r.rest[1].pos) < 1e-6, "a hinge only turns");
    }

    #[test]
    fn a_hinge_swings_back_when_its_parent_moves() {
        let r = rig(2);
        let mut h = hinge(vec![Limit { reference: None, min: -1.0, max: 1.0, soft_min: 0.2, soft_max: 0.2 }]);
        h.force = Vec3::ZERO;
        let mut s = Secondary::new(&r, vec![Modifier::Hinge(h)], 0);
        let mut pose = r.rest_pose();
        s.apply(&mut pose, &r, Xform::IDENTITY, Vec3::ZERO, 1.0 / 60.0);
        // The model moves up: the bone's end lags, swinging down (X toward -Z).
        let mut lowest = 0.0f32;
        for f in 1..30 {
            pose = r.rest_pose();
            s.apply(&mut pose, &r, Xform::new(Vec3::Z * f as f32 * 0.05, Quat::IDENTITY), Vec3::ZERO, 1.0 / 60.0);
            lowest = lowest.min((pose.model(&r)[1].rot * Vec3::X).z);
        }
        assert!(lowest < -0.05, "{lowest}");
        assert!(pose.local.iter().all(|x| x.rot.is_finite()));
    }

    #[test]
    fn look_at_aims_its_axis_and_keeps_a_right_handed_frame() {
        let mut r = rig(3);
        r.parents[2] = Some(0);
        r.rest[2] = Xform::new(Vec3::new(0.0, 0.5, 0.2), Quat::from_rotation_x(0.7));
        for (axis, up) in [(2, 1), (0, 2)] {
            let mut s = Secondary::new(&r, vec![Modifier::LookAt { target: 2, aim: 1, axis, up }], 0);
            let mut pose = r.rest_pose();
            s.apply(&mut pose, &r, Xform::IDENTITY, Vec3::ZERO, 1.0 / 60.0);
            let m = pose.model(&r);
            let dir = (m[1].pos - m[2].pos).normalize();
            assert!(axis_of(m[2].rot, axis).distance(dir) < 1e-5);
            let basis = Mat3::from_quat(m[2].rot);
            assert!((basis.determinant() - 1.0).abs() < 1e-5);
            assert_eq!(m[2].pos, r.rest[2].pos, "it only turns");
        }
    }

    #[test]
    fn compression_puts_a_bone_between_its_sources() {
        let mut r = rig(4);
        for i in 1..4 {
            r.parents[i] = Some(0);
        }
        r.rest[1] = Xform::new(Vec3::X, Quat::from_rotation_z(0.8));
        r.rest[2] = Xform::new(-Vec3::X, Quat::from_rotation_z(-0.8));
        let c = Compress {
            target: 3,
            sources: [1, 2],
            position_weight: 0.25,
            rotation_weight: 0.5,
            rotation_offsets: [Quat::IDENTITY; 2],
            position_offsets: [Vec3::ZERO; 2],
            rotation: true,
            position: true,
        };
        let mut s = Secondary::new(&r, vec![Modifier::Compress(c)], 0);
        let mut pose = r.rest_pose();
        s.apply(&mut pose, &r, Xform::IDENTITY, Vec3::ZERO, 1.0 / 60.0);
        assert!(pose.local[3].pos.distance(Vec3::new(-0.5, 0.0, 0.0)) < 1e-5);
        assert!(pose.local[3].rot.angle_between(Quat::IDENTITY) < 1e-5);
    }

    #[test]
    fn roll_counters_the_sources_twist() {
        let mut r = rig(2);
        r.parents[1] = None;
        let mut s = Secondary::new(&r, vec![Modifier::Roll { target: 1, source: 0, mode: 0, weight: 0.25 }], 2);
        let mut pose = r.rest_pose();
        pose.local[0].rot = Quat::from_rotation_x(0.8);
        pose.local[1].rot = Quat::from_rotation_z(0.3);
        s.apply(&mut pose, &r, Xform::IDENTITY, Vec3::ZERO, 1.0 / 60.0);
        let (_, b, c) = pose.local[1].rot.to_euler(EulerRot::YXZ);
        assert!((b + 0.6).abs() < 1e-5 && (c - 0.3).abs() < 1e-5, "{b} {c}");
    }

    #[test]
    fn a_spring_lags_inside_its_box_and_settles() {
        let r = rig(2);
        let sp = Spring {
            target: 1,
            stiffness: 50.0,
            damping: 15.0,
            wind: 0.0,
            force: Vec3::ZERO,
            min: Vec3::new(0.25, -0.05, -0.05),
            max: Vec3::new(0.5, 0.05, 0.05),
        };
        let mut s = Secondary::new(&r, vec![Modifier::Spring(sp)], 0);
        let mut moved = 0.0f32;
        let mut pose = r.rest_pose();
        for f in 0..600 {
            pose = r.rest_pose();
            let x = if f < 10 { f as f32 * 0.05 } else { 0.5 };
            s.apply(&mut pose, &r, Xform::new(Vec3::Y * x, Quat::IDENTITY), Vec3::ZERO, 1.0 / 60.0);
            let p = pose.local[1].pos;
            assert!(p.cmpge(sp.min - 1e-6).all() && p.cmple(sp.max + 1e-6).all());
            moved = moved.max(p.y.abs());
        }
        assert!(moved > 0.01, "it lags inside the box");
        assert!(pose.local[1].pos.distance(r.rest[1].pos) < 1e-3, "and settles");
    }
}
