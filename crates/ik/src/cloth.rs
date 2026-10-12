//! Cloth as AC1 runs its soft bodies (Altaïr's robe tails): the mesh's vertices as Verlet particles stepped
//! 0.033333 s at a time. Each step a vertex keeps its velocity less the damping (an upward-moving one is damped by
//! its own amount, which falls as the character rises), is pulled by gravity and the wind, then is drawn toward
//! where the animation puts it by its own pull or the character's motion pull, whichever is more; pinned vertices
//! are where the animation puts them. Then, a few times over, vertices are pushed out of the capsules on the
//! character's bones and the mesh's edges are held to their lengths (a length between the rest pose's and the
//! animated pose's, by which of the two the cloth is nearer).
//!
//! The behaviour is AC1's (its `ClothSettings`, read by `forge::cloth`), as Banned445's AC1-Movement-Rewritten
//! (MIT, `src/cloth.rs`) recreated it; written fresh here. Works in any frame with `up` given.

use glam::Vec3;

/// AC1's cloth step (s).
pub const STEP: f32 = 0.033333;

/// The cloth's fixed settings, per vertex and global (see `forge::cloth::ClothSettings`).
#[derive(Debug, Clone)]
pub struct Params {
    pub pinned: Vec<bool>,
    /// Per vertex: how far each step draws it to its animated place (0 free, 1 locked).
    pub pull: Vec<f32>,
    /// Per vertex: its collision radius.
    pub radius: Vec<f32>,
    /// Edges held to their lengths, and the rest lengths squared.
    pub edges: Vec<[usize; 2]>,
    pub rest_len2: Vec<f32>,
    pub triangles: Vec<[usize; 3]>,
    pub damping: f32,
    pub upward_damping: f32,
    /// Gravity along `up` (m/s², negative down).
    pub gravity: f32,
    pub iterations: u32,
    /// Motion pull per m/s of speed and per unit of turn; how fast it may fall (per s).
    pub pull_motion: [f32; 2],
    pub pull_decay: f32,
    /// Upward damping added per m/s the character rises, and its cap.
    pub upward_motion: [f32; 2],
    /// The wind on the cloth: (share along the surface, random amplitude), or none.
    pub force: Option<(f32, f32)>,
}

/// A capsule to keep the cloth out of (world).
#[derive(Debug, Clone, Copy)]
pub struct Collider {
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
    /// 0: out on the side the vertex's animated place is (unless `threshold` is over its pull); 1: straight out.
    pub mode: u32,
    pub threshold: f32,
}

/// The character's motion over the last step.
#[derive(Debug, Clone, Copy, Default)]
pub struct Motion {
    /// m/s.
    pub speed: f32,
    /// How fast its facing turns (the change of a unit vector, per s).
    pub turn: f32,
    /// m/s up.
    pub rise: f32,
}

/// What a step reads besides the cloth's own state.
pub struct Inputs<'a> {
    /// Where the animation puts each vertex now.
    pub targets: &'a [Vec3],
    /// The rest pose carried rigidly with the character.
    pub rigid: &'a [Vec3],
    pub colliders: &'a [Collider],
    pub motion: Motion,
    /// How much of the motion pull the playing action lets off (1 none, 0 all: the cloth follows the animation).
    pub strength: f32,
    /// The wind where the character is (m/s²).
    pub wind: Vec3,
    pub up: Vec3,
}

#[derive(Debug, Clone)]
pub struct Cloth {
    pub pos: Vec<Vec3>,
    prev: Vec<Vec3>,
    normals: Vec<Vec3>,
    /// The motion pull now (after the action's strength).
    pub motion_pull: f32,
    seed: u32,
}

impl Cloth {
    /// A cloth at rest where the animation puts it.
    pub fn new(p: &Params, targets: &[Vec3]) -> Self {
        Cloth { pos: targets.to_vec(), prev: targets.to_vec(), normals: normals(&p.triangles, targets), motion_pull: 0.0, seed: 0x1234_5678 }
    }

    /// One step of `dt` (AC1's is `STEP`).
    pub fn step(&mut self, p: &Params, i: &Inputs, dt: f32) {
        let n = self.pos.len().min(i.targets.len()).min(i.rigid.len());
        // The motion pull: from speed and turn, falling no faster than its decay, eased by the action.
        let want = p.pull_motion[0] * i.motion.speed + p.pull_motion[1] * i.motion.turn;
        let want = want.max(self.motion_pull - p.pull_decay * dt).clamp(0.0, 1.0);
        self.motion_pull = 1.0 - (1.0 - want) * i.strength.clamp(0.0, 1.0);
        let upward = (p.upward_damping + i.motion.rise * p.upward_motion[0]).clamp(0.0, p.upward_motion[1].max(0.0));
        for k in 0..n {
            let old = self.pos[k];
            if p.pinned[k] {
                self.pos[k] = i.targets[k];
            } else {
                let v = old - self.prev[k];
                let along = v.dot(i.up);
                let vertical = if along >= 0.0 { upward } else { p.damping };
                let v = (v - i.up * along) * (1.0 - p.damping) + i.up * along * (1.0 - vertical);
                let a = self.accel(p, k, i.wind, i.up);
                let free = old + v + a * dt * dt;
                self.pos[k] = free.lerp(i.targets[k], p.pull[k].max(self.motion_pull));
            }
            self.prev[k] = old;
        }
        let len2: Vec<f32> = p
            .edges
            .iter()
            .zip(&p.rest_len2)
            .map(|(&[a, b], &base)| {
                let (pa, pb) = (self.pos[a], self.pos[b]);
                let from_rest = i.rigid[a].distance(pa) + i.rigid[b].distance(pb);
                let from_target = i.targets[a].distance(pa) + i.targets[b].distance(pb);
                let total = from_rest + from_target;
                if total <= 1e-12 { base } else { base + (i.targets[a].distance_squared(i.targets[b]) - base) * from_rest / total }
            })
            .collect();
        for _ in 0..p.iterations {
            for k in 0..n {
                if p.pinned[k] {
                    continue;
                }
                let pull = p.pull[k].max(self.motion_pull);
                for c in i.colliders {
                    self.pos[k] = c.push_out(self.pos[k], i.targets[k], p.radius[k], pull);
                }
            }
            for (&[a, b], &rest) in p.edges.iter().zip(&len2) {
                let d = self.pos[b] - self.pos[a];
                let denom = d.length_squared() + rest;
                if denom <= 1e-12 {
                    continue;
                }
                // Toward the length without a square root (exact at it, close near it).
                let c = d * (rest / denom - 0.5);
                match (p.pinned[a], p.pinned[b]) {
                    (false, false) => {
                        self.pos[a] -= c;
                        self.pos[b] += c;
                    }
                    (true, false) => self.pos[b] += 2.0 * c,
                    (false, true) => self.pos[a] -= 2.0 * c,
                    (true, true) => {}
                }
            }
        }
        self.normals = normals(&p.triangles, &self.pos);
    }

    /// A vertex's acceleration: the wind, its random share and its part along the surface cut, then gravity.
    fn accel(&mut self, p: &Params, k: usize, wind: Vec3, up: Vec3) -> Vec3 {
        let gravity = up * p.gravity;
        let Some((tangent, random)) = p.force else { return gravity };
        if wind == Vec3::ZERO {
            return gravity;
        }
        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let r = (self.seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0;
        let f = wind * (1.0 + r * random);
        let n = self.normals.get(k).copied().unwrap_or(Vec3::ZERO);
        let normal = n * f.dot(n);
        (f - normal) * tangent + normal + gravity
    }
}

/// Per vertex, the sum of its triangles' (area-weighted) normals, unit.
fn normals(triangles: &[[usize; 3]], pos: &[Vec3]) -> Vec<Vec3> {
    let mut out = vec![Vec3::ZERO; pos.len()];
    for &[a, b, c] in triangles {
        if a.max(b).max(c) >= pos.len() {
            continue;
        }
        let n = (pos[b] - pos[a]).cross(pos[c] - pos[b]);
        for v in [a, b, c] {
            out[v] += n;
        }
    }
    out.iter().map(|n| n.normalize_or_zero()).collect()
}

impl Collider {
    /// `p` (radius `r`) moved out of the capsule: straight out (mode 1, or a threshold over the vertex's `pull`),
    /// else across the capsule toward the side `target` is on. Mode 2 does nothing.
    pub fn push_out(&self, p: Vec3, target: Vec3, r: f32, pull: f32) -> Vec3 {
        if self.mode == 2 {
            return p;
        }
        let radial = self.mode == 1 || self.threshold > pull;
        let r = self.radius + r;
        let axis = self.b - self.a;
        let len2 = axis.length_squared();
        let t = (p - self.a).dot(axis);
        let escape = (target - self.a).normalize_or_zero();
        // Past an end: out of the end's sphere.
        if t <= 0.0 || t >= len2 {
            let end = if t <= 0.0 { self.a } else { self.b };
            let d = p - end;
            let l = d.length();
            if l >= r {
                return p;
            }
            return p + if radial { d.normalize_or_zero() } else { escape } * (r - l);
        }
        let d = p - (self.a + axis * (t / len2));
        let l2 = d.length_squared();
        if l2 >= r * r || (l2 - r * r).abs() <= 0.0005 {
            return p;
        }
        let unit = axis / len2.sqrt();
        if radial || (escape.dot(unit).abs() - 1.0).abs() <= 0.0005 {
            return p + d.normalize_or_zero() * (r - l2.sqrt());
        }
        // Across: in the plane of the axis and the escape, onto the side the escape points to.
        let side = unit.cross(escape).normalize_or_zero();
        let out = side.cross(unit).normalize_or_zero();
        let across = d.dot(side);
        p + out * ((r * r - across * across).max(0.0).sqrt() - d.dot(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A strip of `rows` x 2 vertices 0.1 m apart hanging down -Y, the top row pinned.
    fn strip(rows: usize) -> (Params, Vec<Vec3>) {
        let rest: Vec<Vec3> = (0..rows * 2).map(|i| Vec3::new((i % 2) as f32 * 0.1, -((i / 2) as f32) * 0.1, 0.0)).collect();
        let mut edges = vec![];
        for r in 0..rows {
            edges.push([2 * r, 2 * r + 1]);
            if r + 1 < rows {
                edges.push([2 * r, 2 * r + 2]);
                edges.push([2 * r + 1, 2 * r + 3]);
            }
        }
        let triangles = (0..rows - 1).flat_map(|r| [[2 * r, 2 * r + 1, 2 * r + 2], [2 * r + 1, 2 * r + 3, 2 * r + 2]]).collect();
        let rest_len2 = edges.iter().map(|&[a, b]: &[usize; 2]| rest[a].distance_squared(rest[b])).collect();
        let n = rest.len();
        let p = Params {
            pinned: (0..n).map(|i| i < 2).collect(),
            pull: vec![0.0; n],
            radius: vec![0.01; n],
            edges,
            rest_len2,
            triangles,
            damping: 0.1,
            upward_damping: 0.75,
            gravity: -9.8,
            iterations: 3,
            pull_motion: [0.03, 0.015],
            pull_decay: 0.99,
            upward_motion: [-2.0, 5.0],
            force: Some((0.3, 1.0)),
        };
        (p, rest)
    }

    fn inputs<'a>(t: &'a [Vec3], c: &'a [Collider], motion: Motion) -> Inputs<'a> {
        Inputs { targets: t, rigid: t, colliders: c, motion, strength: 1.0, wind: Vec3::ZERO, up: Vec3::Y }
    }

    #[test]
    fn hangs_from_its_pins_follows_and_keeps_its_length() {
        let (p, rest) = strip(5);
        let mut c = Cloth::new(&p, &rest);
        let mut moved = vec![];
        let mut lagged = false;
        for frame in 0..90 {
            let x = (frame as f32 / 30.0).min(1.0) * 1.5;
            moved = rest.iter().map(|v| *v + Vec3::X * x).collect();
            c.step(&p, &inputs(&moved, &[], Motion { speed: 1.5, ..Default::default() }), STEP);
            assert_eq!(c.pos[0], moved[0], "pins follow");
            if frame == 15 {
                lagged = c.pos[9].x < moved[9].x - 0.02;
            }
        }
        assert!(lagged, "the free end lags the pins while they move");
        let len = c.pos[0].distance(c.pos[8]);
        assert!((0.36..0.44).contains(&len), "{len}");
        assert!(c.pos.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn motion_pull_rises_with_speed_falls_by_its_decay_and_the_action_can_lock_it() {
        let (p, rest) = strip(3);
        let mut c = Cloth::new(&p, &rest);
        c.step(&p, &inputs(&rest, &[], Motion { speed: 10.0, ..Default::default() }), STEP);
        assert!((c.motion_pull - 0.3).abs() < 1e-6);
        c.step(&p, &inputs(&rest, &[], Motion::default()), STEP);
        assert!((c.motion_pull - (0.3 - 0.99 * STEP)).abs() < 1e-6);
        let mut locked = inputs(&rest, &[], Motion::default());
        locked.strength = 0.0;
        c.step(&p, &locked, STEP);
        assert_eq!(c.motion_pull, 1.0);
        assert_eq!(c.pos, rest, "a pull of 1 holds the cloth on the animation");
    }

    #[test]
    fn capsules_push_free_vertices_out_and_leave_pins() {
        let (mut p, rest) = strip(4);
        p.gravity = 0.0;
        // A leg across the strip just in front of it: the strip bends back round it.
        let leg = Collider { a: Vec3::new(-0.5, -0.25, 0.05), b: Vec3::new(0.5, -0.25, 0.05), radius: 0.08, mode: 1, threshold: 0.0 };
        let mut c = Cloth::new(&p, &rest);
        for _ in 0..30 {
            c.step(&p, &inputs(&rest, &[leg], Motion::default()), STEP);
            assert_eq!(c.pos[0], rest[0]);
        }
        // (The last edge pass may take a vertex back in a little.)
        for v in &c.pos[2..] {
            let d = Vec3::new(0.0, v.y + 0.25, v.z - 0.05).length();
            assert!(d > 0.09 - 0.02, "{v} inside the leg ({d})");
        }
    }

    #[test]
    fn directed_contacts_go_out_on_the_animated_side() {
        let c = Collider { a: -Vec3::Y, b: Vec3::Y, radius: 0.1, mode: 0, threshold: 0.0 };
        let p = Vec3::new(-0.04, 0.0, 0.03);
        let out = c.push_out(p, Vec3::X * 0.3, 0.0, 0.015);
        assert!(out.x > 0.09 && (out.z - p.z).abs() < 1e-6, "{out}");
        assert!((Vec3::new(out.x, 0.0, out.z).length() - 0.1).abs() < 1e-5);
        let radial = Collider { threshold: 0.5, ..c };
        assert!(radial.push_out(p, Vec3::X * 0.3, 0.0, 0.015).x < 0.0, "a threshold over the pull pushes straight out");
        assert_eq!(Collider { mode: 2, ..c }.push_out(p, Vec3::X, 0.0, 0.0), p);
        assert_eq!(c.push_out(Vec3::new(0.2, 0.0, 0.0), Vec3::X, 0.0, 0.0), Vec3::new(0.2, 0.0, 0.0));
    }

    #[test]
    fn the_wind_pushes_along_the_normal_more_than_along_the_surface() {
        let (mut p, rest) = strip(3);
        p.gravity = 0.0;
        p.force = Some((0.3, 0.0));
        let mut c = Cloth::new(&p, &rest);
        // The strip lies in the XY plane: its normal is Z.
        let a = c.accel(&p, 3, Vec3::new(2.0, 0.0, 4.0), Vec3::Y);
        assert!(a.distance(Vec3::new(0.6, 0.0, 4.0)) < 1e-5, "{a}");
        p.force = None;
        assert_eq!(c.accel(&p, 3, Vec3::X, Vec3::Y), Vec3::ZERO);
    }

    #[test]
    fn rising_lets_the_cloth_go_up() {
        // Damping of upward motion: 0.75 at rest; rising at 0.375 m/s or more takes it off.
        let (mut p, rest) = strip(2);
        p.gravity = 0.0;
        let mut a = Cloth::new(&p, &rest);
        let mut b = a.clone();
        for c in [&mut a, &mut b] {
            c.prev[3] = rest[3] - Vec3::Y * 0.01;
            c.pos[3] = rest[3];
        }
        p.pinned[2] = true;
        a.step(&p, &inputs(&rest, &[], Motion::default()), STEP);
        b.step(&p, &inputs(&rest, &[], Motion { rise: 1.0, ..Default::default() }), STEP);
        assert!(b.pos[3].y > a.pos[3].y, "{} {}", b.pos[3], a.pos[3]);
    }
}
