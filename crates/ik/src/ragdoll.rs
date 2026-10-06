//! A position-based (Verlet) ragdoll over a rig: particles at the driven bones' origins (and a few tips:
//! head top, fingers, toes), kept at their bone lengths, the pelvis and chest braced rigid, knees and
//! elbows held to their bend range, spine and neck to theirs, colliding through a callback. Bone rotations
//! are rebuilt from the particles: each bone keeps its twist from the moment the ragdoll took over and turns
//! to follow its particles.
//!
//! The sizes (particle radii, masses) and bend limits come from the game's ragdoll (`forge::hkx::Ragdoll`,
//! mapped onto the rig by bone name by the caller); the solver itself is generic.

use crate::{Pose, Rig};
use glam::{Quat, Vec3};

/// One driven bone: its rig index, its particle's radius and mass, and optionally a tip particle (an
/// offset in the bone's own frame: the far end of a hand, foot or head).
#[derive(Debug, Clone)]
pub struct BoneDef {
    pub bone: usize,
    pub radius: f32,
    pub mass: f32,
    pub tip: Option<Vec3>,
}

/// A bend limit at a joint: the angle between the segments (a -> b) and (b -> c), in radians, 0 when
/// straight; `relative`: the range is around the starting pose's bend (a spine's natural curve).
#[derive(Debug, Clone)]
pub struct Bend {
    pub a: usize,
    pub b: usize,
    pub c: usize,
    pub min: f32,
    pub max: f32,
    pub relative: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RagdollDef {
    pub bones: Vec<BoneDef>,
    /// Groups of bones held rigid together (pelvis, chest).
    pub rigid: Vec<Vec<usize>>,
    /// Bend limits, by bone (rig indices).
    pub bends: Vec<Bend>,
}

#[derive(Debug, Clone)]
struct Link {
    a: usize,
    b: usize,
    len: f32,
}

/// A running ragdoll (world space).
#[derive(Debug, Clone)]
pub struct Ragdoll {
    pub pos: Vec<Vec3>,
    prev: Vec<Vec3>,
    inv_mass: Vec<f32>,
    pub radius: Vec<f32>,
    links: Vec<Link>,
    /// Particle of each driven bone, and of its tip.
    bone_particle: Vec<(usize, usize, Option<usize>)>,
    /// The rig bone of each `bone_particle` entry.
    bone_ids: Vec<usize>,
    /// At the start: particle positions and the bones' world rotations.
    ref_pos: Vec<Vec3>,
    ref_rot: Vec<Quat>,
    bends: Vec<(usize, usize, usize, f32, f32)>,
    /// Seconds without moving much (settled).
    pub still: f32,
}

/// World transform of every bone for `pose` placed at (`root_pos`, `root_rot`).
fn world(rig: &Rig, pose: &Pose, root_pos: Vec3, root_rot: Quat) -> Vec<(Vec3, Quat)> {
    pose.model(rig).iter().map(|x| (root_pos + root_rot * x.pos, root_rot * x.rot)).collect()
}

impl Ragdoll {
    /// Start from `pose` placed at `root`, moving at `velocity` (m/s, world).
    pub fn new(def: &RagdollDef, rig: &Rig, pose: &Pose, root_pos: Vec3, root_rot: Quat, velocity: Vec3, dt: f32) -> Self {
        let w = world(rig, pose, root_pos, root_rot);
        let mut pos = vec![];
        let mut inv_mass = vec![];
        let mut radius = vec![];
        let mut bone_particle = vec![];
        for (k, b) in def.bones.iter().enumerate() {
            let (p, r) = w[b.bone];
            let i = pos.len();
            pos.push(p);
            inv_mass.push(1.0 / b.mass.max(0.1));
            radius.push(b.radius);
            let tip = b.tip.map(|t| {
                pos.push(p + r * t);
                inv_mass.push(1.0 / (b.mass * 0.5).max(0.1));
                radius.push(b.radius * 0.8);
                pos.len() - 1
            });
            bone_particle.push((k, i, tip));
        }
        let of = |bone: usize| def.bones.iter().position(|b| b.bone == bone).map(|k| bone_particle[k].1);
        let mut links = vec![];
        let mut link = |a: usize, b: usize, pos: &Vec<Vec3>| {
            if a != b && !links.iter().any(|l: &Link| (l.a == a && l.b == b) || (l.a == b && l.b == a)) {
                links.push(Link { a, b, len: (pos[a] - pos[b]).length() });
            }
        };
        // Each bone to its nearest driven ancestor, and to its tip.
        for (k, b) in def.bones.iter().enumerate() {
            let mut p = rig.parents[b.bone];
            let mut anc = None;
            while let Some(q) = p {
                if let Some(pi) = of(q) {
                    link(pi, bone_particle[k].1, &pos);
                    anc = Some(pi);
                    break;
                }
                p = rig.parents[q];
            }
            // A tip is held to the bone and its parent: wrists, ankles and the neck stay firm.
            if let Some(t) = bone_particle[k].2 {
                link(bone_particle[k].1, t, &pos);
                if let Some(pi) = anc {
                    link(pi, t, &pos);
                }
            }
        }
        for group in &def.rigid {
            let ps: Vec<usize> = group.iter().filter_map(|&b| of(b)).collect();
            for (i, &a) in ps.iter().enumerate() {
                for &b in &ps[i + 1..] {
                    link(a, b, &pos);
                }
            }
        }
        // Bends are kept within their range of the starting pose's (which is a natural one): spine and neck
        // curvature differ from rig to rig.
        let bends = def
            .bends
            .iter()
            .filter_map(|b| {
                let (a, bb, c) = (of(b.a)?, of(b.b)?, of(b.c)?);
                let a0 = bend_angle(&pos, a, bb, c);
                let (lo, hi) = if b.relative { ((a0 + b.min).max(0.0), a0 + b.max) } else { (b.min, b.max) };
                // (The starting bend is always allowed.)
                Some((a, bb, c, lo.min(a0), hi.max(a0)))
            })
            .collect();
        let prev = pos.iter().map(|p| *p - velocity * dt).collect();
        let ref_rot = def.bones.iter().map(|b| w[b.bone].1).collect();
        let bone_ids = def.bones.iter().map(|b| b.bone).collect();
        Ragdoll { ref_pos: pos.clone(), pos, prev, inv_mass, radius, links, bone_particle, bone_ids, ref_rot, bends, still: 0.0 }
    }

    /// Advance by `dt` under `gravity`; `collide(prev, now, radius)` returns where a particle may be
    /// (pushed out of the world), and whether it touches it.
    #[allow(clippy::needless_range_loop)] // (Several arrays indexed in step.)
    pub fn step(&mut self, dt: f32, gravity: Vec3, mut collide: impl FnMut(Vec3, Vec3, f32) -> (Vec3, bool)) {
        let dt = dt.min(1.0 / 30.0);
        let n = self.pos.len();
        let mut moved = 0.0f32;
        let mut contact: Vec<Option<Vec3>> = vec![None; n];
        for i in 0..n {
            let v = (self.pos[i] - self.prev[i]) * 0.995;
            self.prev[i] = self.pos[i];
            self.pos[i] += v + gravity * dt * dt;
        }
        for _ in 0..10 {
            for l in &self.links {
                let (a, b) = (self.pos[l.a], self.pos[l.b]);
                let d = b - a;
                let len = d.length().max(1e-6);
                let (wa, wb) = (self.inv_mass[l.a], self.inv_mass[l.b]);
                let corr = d * ((len - l.len) / len / (wa + wb));
                self.pos[l.a] += corr * wa;
                self.pos[l.b] -= corr * wb;
            }
            // Bends as a distance range between the outer two (law of cosines: more bend, nearer), mass
            // weighted both ways, so they add no motion of their own.
            for &(a, b, c, lo, hi) in &self.bends {
                let (lu, lv) = ((self.pos[b] - self.pos[a]).length(), (self.pos[c] - self.pos[b]).length());
                let dist = |t: f32| (lu * lu + lv * lv + 2.0 * lu * lv * t.cos()).max(0.0).sqrt();
                let (near, far) = (dist(hi), dist(lo));
                let d = self.pos[c] - self.pos[a];
                let len = d.length().max(1e-6);
                let want = len.clamp(near, far);
                if (want - len).abs() > 1e-5 {
                    let (wa, wc) = (self.inv_mass[a], self.inv_mass[c]);
                    let corr = d * ((len - want) / len / (wa + wc));
                    self.pos[a] += corr * wa;
                    self.pos[c] -= corr * wc;
                }
            }
            for i in 0..n {
                let before = self.pos[i];
                let (p, touching) = collide(self.prev[i], before, self.radius[i]);
                let push = p - before;
                self.pos[i] = p;
                // (A push out of the world adds no speed: the previous position moves with it.)
                self.prev[i] += push;
                if touching {
                    contact[i] = Some(push.normalize_or(Vec3::ZERO));
                }
            }
        }
        // (Never faster than a hard fall: a safety net for the solver.)
        let cap = MAX_SPEED * dt;
        for i in 0..n {
            let v = self.pos[i] - self.prev[i];
            if v.length() > cap {
                self.prev[i] = self.pos[i] - v.normalize() * cap;
            }
        }
        // Contacts: no speed into the surface, and friction along it.
        for (i, c) in contact.iter().enumerate() {
            let Some(n) = *c else { continue };
            let v = self.pos[i] - self.prev[i];
            let n = if n == Vec3::ZERO { -gravity.normalize_or_zero() } else { n };
            let vn = v.dot(n);
            let vt = v - n * vn;
            let v2 = vt * 0.6 + n * vn.max(0.0) * 0.3;
            self.prev[i] = self.pos[i] - v2;
        }
        // Settled: the particles' mean speed low (contacts leave a little jitter).
        for i in 0..n {
            moved += (self.pos[i] - self.prev[i]).length() / dt.max(1e-4) / n.max(1) as f32;
        }
        self.still = if moved < 0.25 { self.still + dt } else { 0.0 };
    }

    /// Pose the rig from the particles. `pose` should be the pose the ragdoll started from (bones it does
    /// not drive keep their local transforms); the root is at (`root_pos`, `root_rot`).
    pub fn apply(&self, def: &RagdollDef, rig: &Rig, pose: &mut Pose, root_pos: Vec3, root_rot: Quat) {
        let inv = root_rot.inverse();
        // Parent-first: def.bones is in rig order.
        for &(k, i, tip) in &self.bone_particle {
            let bone = def.bones[k].bone;
            // What this bone carries: its tip, or its nearest driven descendants.
            let mut kids: Vec<usize> = tip.into_iter().collect();
            if kids.is_empty() {
                kids = self
                    .bone_particle
                    .iter()
                    .filter(|&&(kk, _, _)| {
                        let mut p = rig.parents[def.bones[kk].bone];
                        while let Some(q) = p {
                            if q == bone {
                                return true;
                            }
                            if def.bones.iter().any(|b| b.bone == q) {
                                return false;
                            }
                            p = rig.parents[q];
                        }
                        false
                    })
                    .map(|&(_, ci, _)| ci)
                    .collect();
            }
            let Some(&c) = kids.first() else { continue };
            let (d_ref, d_now) = (self.ref_pos[c] - self.ref_pos[i], self.pos[c] - self.pos[i]);
            // Two or more: the whole turn from a frame on two of them (the pelvis on its spine and hips,
            // the chest on its neck and shoulders); one: turned to point at it, keeping its twist.
            let second = kids.iter().skip(1).copied().find(|&c2| {
                let r2 = self.ref_pos[c2] - self.ref_pos[i];
                d_ref.normalize_or_zero().cross(r2.normalize_or_zero()).length() > 0.2
            });
            let turn = match second {
                Some(c2) => {
                    let frame = |a: Vec3, b: Vec3| {
                        let x = a.normalize_or_zero();
                        let z = x.cross(b).normalize_or_zero();
                        glam::Mat3::from_cols(x, z.cross(x), z)
                    };
                    let f_ref = frame(d_ref, self.ref_pos[c2] - self.ref_pos[i]);
                    let f_now = frame(d_now, self.pos[c2] - self.pos[i]);
                    Quat::from_mat3(&(f_now * f_ref.transpose()))
                }
                None => Quat::from_rotation_arc(d_ref.normalize_or_zero(), d_now.normalize_or_zero()),
            };
            let rot = turn * self.ref_rot[k];
            pose.set_model_rot(rig, bone, (inv * rot).normalize());
        }
        // Each driven chain's first bone (one whose parent the ragdoll does not drive: the hips, and in AC1's
        // rigs the spine too) goes where its particle is, carrying the chain with it.
        for &(k, i, _) in &self.bone_particle {
            let bone = def.bones[k].bone;
            if rig.parents[bone].is_some_and(|p| def.bones.iter().any(|b| b.bone == p)) {
                continue;
            }
            let want = inv * (self.pos[i] - root_pos);
            let now = pose.model_of(rig, bone).pos;
            pose.translate_model(rig, bone, want - now);
        }
    }

    /// The worst link stretch now (m).
    pub fn link_error(&self) -> f32 {
        self.links.iter().map(|l| ((self.pos[l.a] - self.pos[l.b]).length() - l.len).abs()).fold(0.0, f32::max)
    }

    /// The particle of rig bone `bone`, if it drives it.
    pub fn particle_of(&self, bone: usize) -> Option<Vec3> {
        self.bone_particle.iter().zip(&self.bone_ids).find(|(_, b)| **b == bone).map(|((_, i, _), _)| self.pos[*i])
    }

    /// Where the ragdoll's first (root) particle is.
    pub fn root(&self) -> Vec3 {
        self.pos.first().copied().unwrap_or(Vec3::ZERO)
    }
}

/// Particles never move faster than this (m/s).
const MAX_SPEED: f32 = 15.0;

/// The bend at `b` (between a->b and b->c), radians.
fn bend_angle(pos: &[Vec3], a: usize, b: usize, c: usize) -> f32 {
    let (u, v) = ((pos[b] - pos[a]).normalize_or_zero(), (pos[c] - pos[b]).normalize_or_zero());
    u.dot(v).clamp(-1.0, 1.0).acos()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_rig;

    /// Particles turned as a whole: the rebuilt pose puts every driven bone on its particle.
    #[test]
    fn apply_follows_the_particles() {
        let rig = test_rig();
        let pose = rig.rest_pose();
        let def = RagdollDef { bones: (0..rig.len()).map(|bone| BoneDef { bone, radius: 0.05, mass: 5.0, tip: None }).collect(), ..Default::default() };
        let (root_pos, root_rot) = (Vec3::new(1.0, 2.0, 3.0), Quat::from_rotation_z(0.7) * Quat::from_rotation_x(-1.57));
        let mut r = Ragdoll::new(&def, &rig, &pose, root_pos, root_rot, Vec3::ZERO, 1.0 / 60.0);
        // Turned as a whole, then the knee bent (the shin and foot turned about the knee), the root moved.
        let turn = Quat::from_rotation_x(1.5);
        for p in r.pos.iter_mut() {
            *p = turn * *p;
        }
        let knee = r.pos[2];
        let bend = Quat::from_rotation_y(0.8);
        for i in [3, 4] {
            r.pos[i] = knee + bend * (r.pos[i] - knee);
        }
        let (root_pos, root_rot) = (Vec3::new(0.5, -1.0, 0.0), Quat::from_rotation_y(0.4));
        let mut out = pose.clone();
        r.apply(&def, &rig, &mut out, root_pos, root_rot);
        let m = out.model(&rig);
        for (k, b) in def.bones.iter().enumerate() {
            let want = r.pos[r.bone_particle[k].1];
            let got = root_pos + root_rot * m[b.bone].pos;
            assert!((got - want).length() < 0.02, "bone {} at {got:?}, particle {want:?}", b.bone);
        }
    }

    /// A ragdoll dropped onto a floor falls, lands on it and keeps its bone lengths.
    #[test]
    fn falls_and_settles_on_the_floor() {
        let rig = test_rig();
        let pose = rig.rest_pose();
        let def = RagdollDef { bones: (0..rig.len()).map(|bone| BoneDef { bone, radius: 0.05, mass: 5.0, tip: None }).collect(), ..Default::default() };
        let mut r = Ragdoll::new(&def, &rig, &pose, Vec3::new(0.0, 0.0, 2.0), Quat::IDENTITY, Vec3::ZERO, 1.0 / 60.0);
        let lens: Vec<f32> = r.links.iter().map(|l| l.len).collect();
        for _ in 0..240 {
            r.step(1.0 / 60.0, Vec3::new(0.0, 0.0, -9.8), |_, p, rad| if p.z < rad { (p.with_z(rad), true) } else { (p, false) });
        }
        assert!(r.pos.iter().all(|p| p.z >= 0.049), "{:?}", r.pos);
        assert!(r.pos.iter().any(|p| p.z < 0.2));
        for (l, len) in r.links.iter().zip(lens) {
            assert!(((r.pos[l.a] - r.pos[l.b]).length() - len).abs() < 0.02);
        }
        let mut p = pose.clone();
        r.apply(&def, &rig, &mut p, Vec3::ZERO, Quat::IDENTITY);
        assert!(p.local.iter().all(|x| x.rot.is_finite() && x.pos.is_finite()));
    }
}
