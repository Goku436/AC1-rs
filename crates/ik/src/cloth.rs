//! Cloth (AC1's robe tails are a soft body over the skinned mesh): the mesh's vertices as Verlet particles
//! under gravity, held to their edge lengths (and across neighbouring triangles, for some stiffness), each
//! kept within a distance of where the animation puts it (none at the waist, more toward the hem), and
//! pushed out of capsules round the legs and hips.

use glam::Vec3;

#[derive(Debug, Clone)]
struct Edge {
    a: usize,
    b: usize,
    len: f32,
    stiffness: f32,
}

#[derive(Debug, Clone)]
pub struct Cloth {
    pub pos: Vec<Vec3>,
    prev: Vec<Vec3>,
    edges: Vec<Edge>,
    /// Per vertex: how far it may stray from its animated position (0: follows it exactly).
    reach: Vec<f32>,
    /// How fast (1/s) free vertices are drawn back toward their animated positions: the cloth keeps its authored drape
    /// at rest and swings when moved (0: only the reach limit holds it).
    pub shape_rate: f32,
}

/// A capsule to keep the cloth out of: from `a` to `b`, radius `r` (world).
#[derive(Debug, Clone, Copy)]
pub struct Capsule {
    pub a: Vec3,
    pub b: Vec3,
    pub r: f32,
}

impl Capsule {
    fn push_out(&self, p: Vec3) -> Option<Vec3> {
        let ab = self.b - self.a;
        let t = if ab.length_squared() > 1e-8 { ((p - self.a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
        let c = self.a + ab * t;
        let d = p - c;
        let l = d.length();
        (l < self.r).then(|| c + if l > 1e-6 { d / l } else { Vec3::Y } * self.r)
    }
}

impl Cloth {
    /// A cloth over a triangle mesh whose vertices are now at `targets` (rest lengths from `rest`, the bind
    /// pose); `reach` per vertex (see the field).
    pub fn new(rest: &[Vec3], targets: &[Vec3], triangles: &[[usize; 3]], reach: Vec<f32>) -> Self {
        let mut edges: Vec<Edge> = vec![];
        let mut seen = std::collections::HashSet::new();
        let mut add = |a: usize, b: usize, stiffness: f32, edges: &mut Vec<Edge>| {
            let key = (a.min(b), a.max(b));
            if a != b && seen.insert(key) {
                edges.push(Edge { a, b, len: (rest[a] - rest[b]).length(), stiffness });
            }
        };
        for t in triangles {
            add(t[0], t[1], 1.0, &mut edges);
            add(t[1], t[2], 1.0, &mut edges);
            add(t[2], t[0], 1.0, &mut edges);
        }
        // Bending: across each edge two triangles share, the two far corners.
        let mut by_edge: std::collections::HashMap<(usize, usize), Vec<usize>> = std::collections::HashMap::new();
        for t in triangles {
            for k in 0..3 {
                let (a, b, c) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
                by_edge.entry((a.min(b), a.max(b))).or_default().push(c);
            }
        }
        for far in by_edge.values() {
            if let [x, y, ..] = far[..] {
                add(x, y, 0.3, &mut edges);
            }
        }
        Cloth { pos: targets.to_vec(), prev: targets.to_vec(), edges, reach, shape_rate: 0.0 }
    }

    /// Advance by `dt`: `targets` are where the animation puts the vertices now.
    #[allow(clippy::needless_range_loop)] // (Several arrays indexed in step.)
    pub fn step(&mut self, targets: &[Vec3], dt: f32, gravity: Vec3, damping: f32, capsules: &[Capsule]) {
        let dt = dt.min(1.0 / 30.0);
        let n = self.pos.len().min(targets.len());
        for i in 0..n {
            if self.reach[i] <= 0.0 {
                self.prev[i] = targets[i];
                self.pos[i] = targets[i];
                continue;
            }
            let v = (self.pos[i] - self.prev[i]) * (1.0 - damping);
            self.prev[i] = self.pos[i];
            self.pos[i] += v + gravity * dt * dt;
            if self.shape_rate > 0.0 {
                let pull = (targets[i] - self.pos[i]) * (1.0 - (-self.shape_rate * dt).exp());
                self.pos[i] += pull;
            }
        }
        for _ in 0..4 {
            for e in &self.edges {
                let (wa, wb) = (if self.reach[e.a] > 0.0 { 1.0 } else { 0.0 }, if self.reach[e.b] > 0.0 { 1.0 } else { 0.0 });
                if wa + wb == 0.0 {
                    continue;
                }
                let d = self.pos[e.b] - self.pos[e.a];
                let len = d.length().max(1e-6);
                let corr = d * ((len - e.len) / len / (wa + wb) * e.stiffness);
                self.pos[e.a] += corr * wa;
                self.pos[e.b] -= corr * wb;
            }
            for i in 0..n {
                if self.reach[i] <= 0.0 {
                    continue;
                }
                for c in capsules {
                    if let Some(p) = c.push_out(self.pos[i]) {
                        self.pos[i] = p;
                    }
                }
                // Within reach of the animated position.
                let off = self.pos[i] - targets[i];
                if off.length() > self.reach[i] {
                    self.pos[i] = targets[i] + off.normalize() * self.reach[i];
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A strip pinned at its top edge, moved sideways: the free end lags behind, swings back, stays within
    /// reach and keeps its length.
    #[test]
    #[allow(clippy::needless_range_loop)] // (Several arrays indexed in step.)
    fn hangs_from_its_pins_and_follows() {
        // Two columns of 5 vertices, 0.1 m apart, top row pinned.
        let rest: Vec<Vec3> = (0..10).map(|i| Vec3::new((i % 2) as f32 * 0.1, -((i / 2) as f32) * 0.1, 0.0)).collect();
        let tris: Vec<[usize; 3]> = (0..4).flat_map(|r| [[2 * r, 2 * r + 1, 2 * r + 2], [2 * r + 1, 2 * r + 3, 2 * r + 2]]).collect();
        let reach: Vec<f32> = (0..10).map(|i| if i < 2 { 0.0 } else { 0.3 }).collect();
        let mut c = Cloth::new(&rest, &rest, &tris, reach);
        let leg = Capsule { a: Vec3::new(0.05, -0.2, 0.08), b: Vec3::new(0.05, -0.5, 0.08), r: 0.05 };
        let mut moved = rest.clone();
        for frame in 0..120 {
            let x = (frame as f32 / 60.0).min(1.0) * 0.5;
            moved = rest.iter().map(|p| *p + Vec3::X * x).collect();
            c.step(&moved, 1.0 / 60.0, Vec3::new(0.0, -9.8, 0.0), 0.02, &[leg]);
        }
        for i in 0..10 {
            assert!((c.pos[i] - moved[i]).length() <= if i < 2 { 1e-5 } else { 0.3 + 1e-3 });
        }
        let len = (c.pos[8] - c.pos[0]).length();
        assert!(len < 0.45 && len > 0.3, "{len}");
    }
}
