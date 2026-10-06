//! Walking routes over AC1's navigation meshes (`forge::navmesh`): every walkable triangle of a city's cells is
//! a node, joined to the triangles it shares an edge with (inside a piece and across pieces and cells, by the
//! edge's corners, to the centimetre). A* gives the triangles from one point to another; the route is their
//! shared edges' middles. Guards use it to come round walls instead of walking into them.

use bevy::prelude::*;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

pub struct NavGraph {
    tris: Vec<[Vec3; 3]>,
    centre: Vec<Vec3>,
    /// Per triangle: (neighbour, the shared edge's middle).
    links: Vec<Vec<(usize, Vec3)>>,
    /// Triangles by 4 m grid cell (x, z), for finding the one under a point.
    grid: HashMap<(i32, i32), Vec<usize>>,
}

const CELL: f32 = 4.0;

/// A corner to the centimetre.
type Corner = (i32, i32, i32);

fn key(p: Vec3) -> (i32, i32) {
    ((p.x / CELL).floor() as i32, (p.z / CELL).floor() as i32)
}

fn corner_key(p: Vec3) -> (i32, i32, i32) {
    ((p.x * 100.0).round() as i32, (p.y * 100.0).round() as i32, (p.z * 100.0).round() as i32)
}

impl NavGraph {
    /// From triangles (Y up, world).
    pub fn new(tris: Vec<[Vec3; 3]>) -> Self {
        let centre: Vec<Vec3> = tris.iter().map(|t| (t[0] + t[1] + t[2]) / 3.0).collect();
        let mut by_edge: HashMap<(Corner, Corner), Vec<usize>> = HashMap::new();
        for (i, t) in tris.iter().enumerate() {
            for k in 0..3 {
                let (a, b) = (corner_key(t[k]), corner_key(t[(k + 1) % 3]));
                by_edge.entry((a.min(b), a.max(b))).or_default().push(i);
            }
        }
        let mut links = vec![vec![]; tris.len()];
        for (&(a, b), ts) in &by_edge {
            let mid = (Vec3::new(a.0 as f32, a.1 as f32, a.2 as f32) + Vec3::new(b.0 as f32, b.1 as f32, b.2 as f32)) / 200.0;
            for &i in ts {
                for &j in ts {
                    if i != j {
                        links[i].push((j, mid));
                    }
                }
            }
        }
        // Pieces meet along border edges that do not share corners (one long edge against two short ones):
        // border edges on one line (within 10 cm), overlapping at least 20 cm, at heights within 30 cm.
        let border: Vec<(usize, Vec3, Vec3)> = by_edge
            .iter()
            .filter(|(_, ts)| ts.len() == 1)
            .map(|(&(a, b), ts)| {
                let p = |c: (i32, i32, i32)| Vec3::new(c.0 as f32, c.1 as f32, c.2 as f32) / 100.0;
                (ts[0], p(a), p(b))
            })
            .collect();
        let mut by_cell: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (k, &(_, a, b)) in border.iter().enumerate() {
            let n = ((b - a).length() / 0.5).ceil().max(1.0) as usize;
            let mut cells: Vec<(i32, i32)> =
                (0..=n).map(|s| a.lerp(b, s as f32 / n as f32)).map(|p| ((p.x / 0.5).floor() as i32, (p.z / 0.5).floor() as i32)).collect();
            cells.dedup();
            for c in cells {
                by_cell.entry(c).or_default().push(k);
            }
        }
        let mut joined = std::collections::HashSet::new();
        for ks in by_cell.values() {
            for (x, &i) in ks.iter().enumerate() {
                for &j in &ks[x + 1..] {
                    let ((ti, a, b), (tj, c, d)) = (border[i], border[j]);
                    if ti == tj || !joined.insert((i.min(j), i.max(j))) {
                        continue;
                    }
                    let dir = (b - a).with_y(0.0);
                    let len = dir.length();
                    if len < 0.05 {
                        continue;
                    }
                    let u = dir / len;
                    let off = |p: Vec3| ((p - a).with_y(0.0) - u * (p - a).with_y(0.0).dot(u)).length();
                    if off(c) > 0.1 || off(d) > 0.1 || ((a.y + b.y) - (c.y + d.y)).abs() * 0.5 > 0.3 {
                        continue;
                    }
                    let (sc, sd) = ((c - a).with_y(0.0).dot(u), (d - a).with_y(0.0).dot(u));
                    let (lo, hi) = (sc.min(sd).max(0.0), sc.max(sd).min(len));
                    if hi - lo < 0.2 {
                        continue;
                    }
                    let mid = a + (b - a) * ((lo + hi) * 0.5 / len);
                    links[ti].push((tj, mid));
                    links[tj].push((ti, mid));
                }
            }
        }
        let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (i, t) in tris.iter().enumerate() {
            let lo = t[0].min(t[1]).min(t[2]);
            let hi = t[0].max(t[1]).max(t[2]);
            for x in key(lo).0..=key(hi).0 {
                for z in key(lo).1..=key(hi).1 {
                    grid.entry((x, z)).or_default().push(i);
                }
            }
        }
        NavGraph { tris, centre, links, grid }
    }

    pub fn len(&self) -> usize {
        self.tris.len()
    }

    /// Triangles reachable from `i` (up to `cap`), for diagnostics.
    pub fn reach(&self, i: usize, cap: usize) -> usize {
        let mut seen = std::collections::HashSet::from([i]);
        let mut stack = vec![i];
        while let Some(k) = stack.pop() {
            for &(j, _) in &self.links[k] {
                if seen.len() < cap && seen.insert(j) {
                    stack.push(j);
                }
            }
        }
        seen.len()
    }

    /// The triangle under `p` (within 1.5 m below or 0.5 m above), else the nearest centre within 3 m.
    pub fn locate(&self, p: Vec3) -> Option<usize> {
        let cands = self.grid.get(&key(p))?;
        let inside = |t: &[Vec3; 3]| {
            let s = |a: Vec3, b: Vec3| (b.x - a.x) * (p.z - a.z) - (b.z - a.z) * (p.x - a.x);
            let (d0, d1, d2) = (s(t[0], t[1]), s(t[1], t[2]), s(t[2], t[0]));
            (d0 >= 0.0 && d1 >= 0.0 && d2 >= 0.0) || (d0 <= 0.0 && d1 <= 0.0 && d2 <= 0.0)
        };
        cands
            .iter()
            .copied()
            .filter(|&i| inside(&self.tris[i]) && (-1.5..0.5).contains(&(self.centre[i].y - p.y)))
            .min_by(|&a, &b| (self.centre[a].y - p.y).abs().total_cmp(&(self.centre[b].y - p.y).abs()))
            .or_else(|| {
                cands
                    .iter()
                    .copied()
                    .filter(|&i| (self.centre[i] - p).length() < 3.0)
                    .min_by(|&a, &b| (self.centre[a] - p).length().total_cmp(&(self.centre[b] - p).length()))
            })
    }

    /// A walking route from `from` to `to`: the points to head for in turn (edge middles, then `to`).
    pub fn route(&self, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
        let (s, g) = (self.locate(from)?, self.locate(to)?);
        if s == g {
            return Some(vec![to]);
        }
        #[derive(PartialEq)]
        struct Open(f32, usize);
        impl Eq for Open {}
        impl Ord for Open {
            fn cmp(&self, o: &Self) -> Ordering {
                o.0.total_cmp(&self.0)
            }
        }
        impl PartialOrd for Open {
            fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
                Some(self.cmp(o))
            }
        }
        let mut cost: HashMap<usize, f32> = HashMap::new();
        let mut came: HashMap<usize, (usize, Vec3)> = HashMap::new();
        let mut open = BinaryHeap::new();
        cost.insert(s, 0.0);
        open.push(Open(0.0, s));
        let mut expanded = 0;
        while let Some(Open(_, i)) = open.pop() {
            if i == g {
                let mut pts = vec![to];
                let mut k = g;
                while let Some(&(p, mid)) = came.get(&k) {
                    pts.push(mid);
                    k = p;
                }
                pts.reverse();
                return Some(pts);
            }
            expanded += 1;
            if expanded > 20_000 {
                return None;
            }
            let here = cost[&i];
            for &(j, mid) in &self.links[i] {
                let c = here + (self.centre[j] - self.centre[i]).length();
                if cost.get(&j).is_none_or(|&old| c < old) {
                    cost.insert(j, c);
                    came.insert(j, (i, mid));
                    open.push(Open(c + (self.centre[j] - to).length(), j));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An L of four triangles: the route goes round the corner through the shared edges.
    #[test]
    fn routes_round_a_corner() {
        let v = |x: f32, z: f32| Vec3::new(x, 0.0, z);
        let tris = vec![
            [v(0.0, 0.0), v(2.0, 0.0), v(0.0, 2.0)],
            [v(2.0, 0.0), v(2.0, 2.0), v(0.0, 2.0)],
            [v(2.0, 0.0), v(4.0, 0.0), v(2.0, 2.0)],
            [v(2.0, 2.0), v(2.0, 4.0), v(0.0, 2.0)],
        ];
        let g = NavGraph::new(tris);
        assert_eq!(g.locate(v(0.5, 0.5)), Some(0));
        let r = g.route(v(3.0, 0.5), v(1.0, 3.0)).unwrap();
        assert_eq!(*r.last().unwrap(), v(1.0, 3.0));
        assert!(r.len() >= 3, "{r:?}");
        assert!(g.route(v(3.0, 0.5), v(30.0, 30.0)).is_none());
    }
}
