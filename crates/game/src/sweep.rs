//! `AC1_COLLISION_SWEEP=1` (cities): how the collision lines up with what is drawn. Every placed mesh's triangles and
//! every collision triangle are sampled (points spread over each face, about one per 0.25 m²); a drawn point with no
//! collision face within `NEAR` is drawn surface the body passes through, a collision point with no drawn face within
//! `NEAR` is an invisible surface. Logged as areas (m²), walls and floors apart, with the meshes and shapes that
//! account for most of each. Only inside the city (the houses' bounds and 10 m round), not the backdrop mountains.

use crate::level::Tri;
use bevy::prelude::*;
use std::collections::HashMap;

/// How far a face may be from the other set's and still count as matched (m).
const NEAR: f32 = 0.3;
/// Grid cell (m) the faces are indexed by, on the ground plane.
const CELL: f32 = 2.0;
/// About one sample per this much face (m²); at most `MAX_SPLIT`² samples a face.
const SAMPLE_AREA: f32 = 0.25;
const MAX_SPLIT: usize = 24;

/// Faces with a name for each (an index into `names`).
#[derive(Default)]
pub struct Set {
    pub tris: Vec<Tri>,
    pub owner: Vec<u32>,
    pub names: Vec<String>,
    by_name: HashMap<String, u32>,
}

impl Set {
    pub fn push(&mut self, name: &str, tris: impl IntoIterator<Item = Tri>) {
        let id = match self.by_name.get(name) {
            Some(&id) => id,
            None => {
                self.names.push(name.to_string());
                self.by_name.insert(name.to_string(), self.names.len() as u32 - 1);
                self.names.len() as u32 - 1
            }
        };
        for t in tris {
            self.tris.push(t);
            self.owner.push(id);
        }
    }
}

struct Grid<'a> {
    tris: &'a [Tri],
    cells: HashMap<(i32, i32), Vec<u32>>,
    big: Vec<u32>,
}

fn cell(v: f32) -> i32 {
    (v / CELL).floor() as i32
}

impl<'a> Grid<'a> {
    fn new(tris: &'a [Tri]) -> Self {
        let mut cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        let mut big = vec![];
        for (i, t) in tris.iter().enumerate() {
            let (lo, hi) = (t.a.min(t.b).min(t.c), t.a.max(t.b).max(t.c));
            let (x0, x1, z0, z1) = (cell(lo.x - NEAR), cell(hi.x + NEAR), cell(lo.z - NEAR), cell(hi.z + NEAR));
            if (x1 - x0 + 1) * (z1 - z0 + 1) > 4096 {
                big.push(i as u32);
                continue;
            }
            for x in x0..=x1 {
                for z in z0..=z1 {
                    cells.entry((x, z)).or_default().push(i as u32);
                }
            }
        }
        Grid { tris, cells, big }
    }

    /// A face within `NEAR` of `p`.
    fn near(&self, p: Vec3) -> bool {
        let close = |i: &u32| {
            let t = &self.tris[*i as usize];
            let (lo, hi) = (t.a.min(t.b).min(t.c) - Vec3::splat(NEAR), t.a.max(t.b).max(t.c) + Vec3::splat(NEAR));
            p.cmpge(lo).all() && p.cmple(hi).all() && closest_on_triangle(p, t).distance_squared(p) < NEAR * NEAR
        };
        self.cells.get(&(cell(p.x), cell(p.z))).is_some_and(|l| l.iter().any(close)) || self.big.iter().any(close)
    }
}

/// The closest point of a triangle to `p` (Ericson's region test).
fn closest_on_triangle(p: Vec3, t: &Tri) -> Vec3 {
    let (a, b, c) = (t.a, t.b, t.c);
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let inv = 1.0 / (va + vb + vc);
    a + ab * (vb * inv) + ac * (vc * inv)
}

/// Per face of `from`: its area and the part of it (m²) with no face of `to` near, split into walls and floors.
struct Tally {
    /// (walls, floors) area in all, and unmatched.
    all: (f64, f64),
    unmatched: (f64, f64),
    /// Unmatched area by owner.
    by_owner: HashMap<u32, f64>,
}

fn tally(from: &Set, to: &Grid, region: (Vec2, Vec2)) -> Tally {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = from.tris.len().div_ceil(threads).max(1);
    let parts: Vec<Tally> = std::thread::scope(|sc| {
        let jobs: Vec<_> = (0..from.tris.len())
            .step_by(chunk)
            .map(|start| {
                sc.spawn(move || {
                    let mut t = Tally { all: (0.0, 0.0), unmatched: (0.0, 0.0), by_owner: HashMap::new() };
                    for i in start..(start + chunk).min(from.tris.len()) {
                        let tri = &from.tris[i];
                        let cross = (tri.b - tri.a).cross(tri.c - tri.a);
                        let area = cross.length() * 0.5;
                        if area < 1e-6 {
                            continue;
                        }
                        let wall = (cross.y / (2.0 * area)).abs() < 0.7;
                        let k = ((area / SAMPLE_AREA).sqrt().ceil() as usize).clamp(1, MAX_SPLIT);
                        // The centres of the k² sub-triangles of a k-split (each 1/k² of the face).
                        let (mut missed, mut inside) = (0usize, 0usize);
                        for u in 0..k {
                            for v in 0..k - u {
                                let mut pts = vec![(u as f32 + 1.0 / 3.0, v as f32 + 1.0 / 3.0)];
                                if u + v + 1 < k {
                                    pts.push((u as f32 + 2.0 / 3.0, v as f32 + 2.0 / 3.0));
                                }
                                for (s, w) in pts {
                                    let p = tri.a + (tri.b - tri.a) * (s / k as f32) + (tri.c - tri.a) * (w / k as f32);
                                    if p.x < region.0.x || p.z < region.0.y || p.x > region.1.x || p.z > region.1.y {
                                        continue;
                                    }
                                    inside += 1;
                                    if !to.near(p) {
                                        missed += 1;
                                    }
                                }
                            }
                        }
                        let a = area as f64 * inside as f64 / (k * k) as f64;
                        let m = area as f64 * missed as f64 / (k * k) as f64;
                        if wall {
                            t.all.0 += a;
                            t.unmatched.0 += m;
                        } else {
                            t.all.1 += a;
                            t.unmatched.1 += m;
                        }
                        if m > 0.0 {
                            *t.by_owner.entry(from.owner[i]).or_default() += m;
                        }
                    }
                    t
                })
            })
            .collect();
        jobs.into_iter().filter_map(|j| j.join().ok()).collect()
    });
    let mut out = Tally { all: (0.0, 0.0), unmatched: (0.0, 0.0), by_owner: HashMap::new() };
    for p in parts {
        out.all.0 += p.all.0;
        out.all.1 += p.all.1;
        out.unmatched.0 += p.unmatched.0;
        out.unmatched.1 += p.unmatched.1;
        for (k, v) in p.by_owner {
            *out.by_owner.entry(k).or_default() += v;
        }
    }
    out
}

fn report(what: &str, unmatched: &str, set: &Set, t: &Tally) {
    let pct = |u: f64, a: f64| if a > 0.0 { 100.0 * u / a } else { 0.0 };
    info!(
        "sweep: {what}: {:.0} m² of walls, {:.0} m² of floors and tops; {unmatched}: walls {:.0} m² ({:.1} %), floors {:.0} m² ({:.1} %)",
        t.all.0,
        t.all.1,
        t.unmatched.0,
        pct(t.unmatched.0, t.all.0),
        t.unmatched.1,
        pct(t.unmatched.1, t.all.1)
    );
    let mut top: Vec<(&u32, &f64)> = t.by_owner.iter().collect();
    top.sort_by(|a, b| b.1.total_cmp(a.1));
    let list: Vec<String> = top.iter().take(30).map(|(k, v)| format!("{} {:.0}", set.names[**k as usize], v)).collect();
    info!("sweep: {what}, most {unmatched} (m²): {}", list.join(", "));
}

/// Compare what is drawn (`drawn`: the solid-named meshes, not foliage, cloth, water or backdrops) with the collision
/// (`solid`) over `region` (Bevy x, z: the city inside its walls, the houses' bounds), and log it.
pub fn run(drawn: &Set, solid: &Set, region: (Vec2, Vec2)) {
    let t0 = std::time::Instant::now();
    info!("sweep: x {:.0}..{:.0}, z {:.0}..{:.0}", region.0.x, region.1.x, region.0.y, region.1.y);
    let (gd, gs) = (Grid::new(&drawn.tris), Grid::new(&solid.tris));
    let d = tally(drawn, &gs, region);
    report("drawn", "with no collision", drawn, &d);
    let s = tally(solid, &gd, region);
    report("collision", "with nothing drawn", solid, &s);
    info!("sweep: {} drawn and {} collision triangles, {:.1}s", drawn.tris.len(), solid.tris.len(), t0.elapsed().as_secs_f32());
}
