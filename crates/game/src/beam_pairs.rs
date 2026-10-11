//! AC1's beams and posts from its hold edges (its runtime beam contacts: pairing 0x680260 / 0x67C2F0, workspace planes
//! 0x67DA30 / 0x666340, surviving intervals 0x67FA60 / 0x67F6D0), after Banned445's decode in AC1-Movement-Rewritten
//! (`src/guidance/beam_contacts.rs`, MIT), run once over a whole city here instead of per query.
//!
//! Two LedgeGrab edges facing each other across a narrow top (within 30 degrees, the partner clipped to a 20 degree
//! wedge in front of the seed, at most 0.5 m half-width, a taper clipped where it passes that) make a beam along their
//! middle. Seeds go longest first, each with its best-overlap partner; the parts of both edges not used go back into the
//! pool. Each beam is then cut where geometry stands in its workspace (a box 0.5 m over it, two side planes 5 degrees
//! out), keeping pieces over 0.2 m. AC1 has no other narrow objects: no beams from mesh names, no single-hold tops.

use crate::level::{Line, Tri};
use bevy::prelude::*;
use std::collections::BinaryHeap;

const MAX_HALF_WIDTH: f32 = 0.5;
const COS_30: f32 = 0.866_025_4;
/// The partner search's cell (m) and reach round a seed (m: past the widest pair).
const CELL: f32 = 2.0;
const REACH: f32 = 1.2;

/// A hold edge: its ends and the wall's normal under it (out of the top).
#[derive(Clone, Copy, Debug)]
pub struct Edge {
    pub p0: Vec3,
    pub p1: Vec3,
    pub out: Vec3,
}

/// A beam: its middle line and half-widths at its ends.
#[derive(Clone, Copy, Debug)]
pub struct Beam {
    pub p0: Vec3,
    pub p1: Vec3,
    pub widths: [f32; 2],
}

impl Beam {
    fn slice(&self, a: f32, b: f32) -> Self {
        let w = |t: f32| self.widths[0] + (self.widths[1] - self.widths[0]) * t;
        Self { p0: self.p0.lerp(self.p1, a), p1: self.p0.lerp(self.p1, b), widths: [w(a), w(b)] }
    }
}

/// Two edges as a beam, with their overlap (0x679980 / 0x67B6D0): positive overlap on the mean axis, opposed walls.
fn pair(a: &Edge, b: &Edge) -> Option<(Beam, f32)> {
    let (u, v) = ((a.p1 - a.p0).normalize_or_zero(), (b.p1 - b.p0).normalize_or_zero());
    if (a.p1 - a.p0).abs().max_element() <= 0.001
        || (b.p1 - b.p0).abs().max_element() <= 0.001
        || u == Vec3::ZERO
        || v == Vec3::ZERO
        || u.dot(v).abs() < COS_30
        || -a.out.dot(b.out) < COS_30
    {
        return None;
    }
    let (start, end, v) = if u.dot(v) < 0.0 { (b.p1, b.p0, -v) } else { (b.p0, b.p1, v) };
    // The candidate clipped to the +-20 degree wedge in front of the seed (0x66C780).
    let inward = -a.out.normalize_or_zero();
    let across = u.cross(inward).normalize_or_zero();
    let tangent = 20f32.to_radians().tan();
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for n in [inward, inward * tangent + across, inward * tangent - across] {
        let (x, y) = ((start - a.p0).dot(n), (end - a.p0).dot(n));
        if x < 0.0 && y < 0.0 {
            return None;
        }
        if x < 0.0 {
            t0 = t0.max(x / (x - y));
        } else if y < 0.0 {
            t1 = t1.min(x / (x - y));
        }
    }
    if t0 > t1 {
        return None;
    }
    let (b0, b1) = (start.lerp(end, t0), start.lerp(end, t1));
    let axis = (u + v).normalize();
    let (da, db) = (u.dot(axis), v.dot(axis));
    let offset = (b0 - a.p0).dot(axis);
    let lo = 0.0f32.max(offset);
    let hi = (a.p1 - a.p0).dot(axis).min((b1 - a.p0).dot(axis));
    if hi <= lo {
        return None;
    }
    let at = |s: f32| a.p0 + u * (s / da);
    let bt = |s: f32| b0 + v * ((s - offset) / db);
    let (a0, a1, c0, c1) = (at(lo), at(hi), bt(lo), bt(hi));
    let beam = Beam { p0: (a0 + c0) * 0.5, p1: (a1 + c1) * 0.5, widths: [a0.distance(c0) * 0.5, a1.distance(c1) * 0.5] };
    let [w0, w1] = beam.widths;
    let (mut s, mut e) = (0.0, 1.0);
    if w0 > MAX_HALF_WIDTH + 0.0005 && w1 > MAX_HALF_WIDTH + 0.0005 {
        return None;
    }
    if w0 > MAX_HALF_WIDTH + 0.0005 {
        if w1 >= MAX_HALF_WIDTH - 0.0005 {
            return None;
        }
        s = (MAX_HALF_WIDTH - w0) / (w1 - w0);
    }
    if w1 > MAX_HALF_WIDTH + 0.0005 {
        if w0 >= MAX_HALF_WIDTH - 0.0005 {
            return None;
        }
        e = (MAX_HALF_WIDTH - w0) / (w1 - w0);
    }
    let beam = beam.slice(s, e);
    let overlap = (b1 - a.p0).dot(u).min(a.p0.distance(a.p1)) - (b0 - a.p0).dot(u).max(0.0);
    (beam.p0.distance_squared(beam.p1) > 0.0001).then_some((beam, overlap))
}

/// A triangle against one workspace plane's rectangle (0x666340 / 0x97A100): the stretch along the beam it blocks, and
/// the length of the cut.
#[allow(clippy::too_many_arguments)]
fn plane_hit(vertices: [Vec3; 3], origin: Vec3, normal: Vec3, along: Vec3, across: Vec3, length: f32, width: f32) -> Option<(f32, f32, f32)> {
    let signed = vertices.map(|p| (p - origin).dot(normal));
    if signed.iter().all(|d| *d >= 0.0) || signed.iter().all(|d| *d < 0.0) {
        return None;
    }
    let mut points = Vec::with_capacity(2);
    for i in 0..3 {
        let j = (i + 1) % 3;
        if (signed[i] >= 0.0) != (signed[j] >= 0.0) {
            points.push(vertices[i].lerp(vertices[j], signed[i] / (signed[i] - signed[j])));
        }
    }
    if points.len() != 2 {
        return None;
    }
    let (p, d) = (points[0] - origin, points[1] - points[0]);
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for (axis, max) in [(along, length), (across, width)] {
        let (v, dv) = (p.dot(axis), d.dot(axis));
        if dv.abs() < 1e-8 {
            if v < 0.0 || v > max {
                return None;
            }
        } else {
            let (a, b) = (-v / dv, (max - v) / dv);
            lo = lo.max(a.min(b));
            hi = hi.min(a.max(b));
            if lo > hi {
                return None;
            }
        }
    }
    let (a, b) = ((p + d * lo).dot(along), (p + d * hi).dot(along));
    Some((a.min(b), a.max(b), d.length() * (hi - lo)))
}

/// The beam cut where geometry stands in its workspace (0x678F80 / 0x67F6D0), by the triangles near it.
fn trim(beam: Beam, tris: &[[Vec3; 3]]) -> Vec<Beam> {
    let axis = (beam.p1 - beam.p0).normalize();
    let length = beam.p0.distance(beam.p1);
    let up = Vec3::Y;
    let side = (-axis.cross(up)).normalize_or_zero();
    let reach = beam.widths[0].max(beam.widths[1]) + 0.15;
    let (left, right) = (beam.p0 + up * 0.5 - side * reach, beam.p0 + up * 0.5 + side * reach);
    let (q0, q1) = (Quat::from_axis_angle(axis, -5f32.to_radians()), Quat::from_axis_angle(axis, 5f32.to_radians()));
    let planes = [(left, -up, side, 2.0 * reach), (left, q0 * (-side), q0 * (-up), 1.0), (right, q1 * side, q1 * (-up), 1.0)];
    let mut intervals = vec![];
    for t in tris {
        let normal = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
        for &(origin, n, across, width) in &planes {
            if let Some((a, b, cut)) = plane_hit(*t, origin, n, axis, across, length, width) {
                if cut <= 0.0005 {
                    continue;
                }
                let dot = normal.dot(axis);
                let tag = if dot > std::f32::consts::FRAC_1_SQRT_2 {
                    0
                } else if dot < -std::f32::consts::FRAC_1_SQRT_2 {
                    1
                } else {
                    2
                };
                intervals.push((a.max(0.0), b.min(length), tag));
            }
        }
    }
    if intervals.is_empty() {
        return vec![beam];
    }
    intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (mut out, mut cursor, mut tag) = (vec![], 0.0, 2);
    for (a, b, t) in intervals {
        if t != 0 && a > cursor && a - cursor > 0.2 {
            out.push(beam.slice(cursor / length, a / length));
        }
        if b > cursor {
            cursor = b;
            tag = t;
        }
    }
    if length > cursor + 0.0005 && tag != 1 && length - cursor > 0.2 {
        out.push(beam.slice(cursor / length, 1.0));
    }
    out
}

/// Longest first (a max-heap of edge lengths, ties by id).
#[derive(PartialEq)]
struct ByLen(f32, usize);
impl Eq for ByLen {}
impl PartialOrd for ByLen {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for ByLen {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&o.0).then(o.1.cmp(&self.1))
    }
}

fn cells(lo: Vec3, hi: Vec3) -> impl Iterator<Item = (i32, i32)> {
    let c = |v: f32| (v / CELL).floor() as i32;
    let (x0, x1, z0, z1) = (c(lo.x - REACH), c(hi.x + REACH), c(lo.z - REACH), c(hi.z + REACH));
    (x0..=x1).flat_map(move |x| (z0..=z1).map(move |z| (x, z)))
}

/// Every beam the edges make (their middle lines), cut by the geometry near them (`tris_near` gives a box's
/// triangles).
pub fn detect(edges: &[Edge], tris_near: impl Fn(Vec3, Vec3) -> Vec<[Vec3; 3]>) -> Vec<Beam> {
    let mut pool: Vec<Option<Edge>> = edges.iter().map(|e| Some(*e)).collect();
    let mut grid: std::collections::HashMap<(i32, i32), Vec<usize>> = std::collections::HashMap::new();
    let mut heap = BinaryHeap::new();
    let insert = |pool: &Vec<Option<Edge>>, grid: &mut std::collections::HashMap<(i32, i32), Vec<usize>>, heap: &mut BinaryHeap<ByLen>, i: usize| {
        let e = pool[i].expect("live edge");
        for k in cells(e.p0.min(e.p1), e.p0.max(e.p1)) {
            grid.entry(k).or_default().push(i);
        }
        heap.push(ByLen(e.p0.distance_squared(e.p1), i));
    };
    for i in 0..pool.len() {
        insert(&pool, &mut grid, &mut heap, i);
    }
    let mut report = vec![];
    while let Some(ByLen(_, i)) = heap.pop() {
        let Some(seed) = pool[i].take() else { continue };
        let mut seen = std::collections::HashSet::new();
        let best = cells(seed.p0.min(seed.p1), seed.p0.max(seed.p1))
            .flat_map(|k| grid.get(&k).into_iter().flatten().copied())
            .filter(|j| seen.insert(*j))
            .filter_map(|j| pool[j].as_ref().and_then(|e| pair(&seed, e)).map(|(b, overlap)| (j, b, overlap)))
            .max_by(|a, b| a.2.total_cmp(&b.2));
        let Some((j, beam, _)) = best else { continue };
        let Some(other) = pool[j].take() else { continue };
        // The parts of both not used go back into the pool (0x67C2F0).
        let u = (seed.p1 - seed.p0).normalize();
        let mut v = (other.p1 - other.p0).normalize();
        if u.dot(v) < 0.0 {
            v = -v;
        }
        let mean = (u + v).normalize();
        for e in [seed, other] {
            let delta = e.p1 - e.p0;
            let denom = delta.dot(mean);
            let (mut lo, mut hi) = ((beam.p0 - e.p0).dot(mean) / denom, (beam.p1 - e.p0).dot(mean) / denom);
            if lo > hi {
                std::mem::swap(&mut lo, &mut hi);
            }
            let (lo, hi) = (lo.clamp(0.0, 1.0), hi.clamp(0.0, 1.0));
            for (a, b) in [(0.0, lo), (hi, 1.0)] {
                let (p0, p1) = (e.p0 + delta * a, e.p0 + delta * b);
                if p0.distance_squared(p1) > 0.0025 {
                    pool.push(Some(Edge { p0, p1, out: e.out }));
                    insert(&pool, &mut grid, &mut heap, pool.len() - 1);
                }
            }
        }
        let (lo, hi) = (beam.p0.min(beam.p1) - Vec3::splat(2.0), beam.p0.max(beam.p1) + Vec3::splat(2.0));
        let kept = trim(beam, &tris_near(lo, hi));
        report.extend(kept);
    }
    report
}

/// The beams as perches (their middle lines), the pieces of one beam joined: a beam whose sides are kinked edges pairs
/// as several pieces with gaps between them where a partner edge ends, which AC1's walk crosses contact to contact;
/// ours walks one line (in Damascus a beam came as pieces of 0.1, 0.08, 1.05 and 1 m and the walk stopped at the
/// first gap).
pub fn lines(beams: &[Beam]) -> Vec<Line> {
    let mut lines: Vec<Line> = beams.iter().map(|b| Line { a: b.p0, b: b.p1 }).collect();
    let joinable = |l: &Line, m: &Line| -> Option<Line> {
        let (u, v) = ((l.b - l.a).normalize_or_zero(), (m.b - m.a).normalize_or_zero());
        if u.dot(v).abs() < JOIN_COS {
            return None;
        }
        // The nearest ends, close and level; the result from the far ends.
        let ends = [(l.a, m.a, l.b, m.b), (l.a, m.b, l.b, m.a), (l.b, m.a, l.a, m.b), (l.b, m.b, l.a, m.a)];
        let (near0, near1, far0, far1) = ends.into_iter().min_by(|x, y| x.0.distance(x.1).total_cmp(&y.0.distance(y.1)))?;
        let gap = near0 - near1;
        (gap.with_y(0.0).length() < JOIN_GAP && gap.y.abs() < JOIN_RISE && (far1 - far0).normalize_or_zero().dot(u).abs() >= JOIN_COS)
            .then_some(Line { a: far0, b: far1 })
    };
    let mut merged = true;
    while merged {
        merged = false;
        let mut i = 0;
        while i < lines.len() {
            let mut j = i + 1;
            while j < lines.len() {
                if let Some(l) = joinable(&lines[i], &lines[j]) {
                    lines[i] = l;
                    lines.swap_remove(j);
                    merged = true;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
    }
    lines
}

/// Beam pieces join end to end across a gap under `JOIN_GAP` (m), within `JOIN_RISE` (m) of level, and lined up within
/// 20 degrees.
const JOIN_GAP: f32 = 0.6;
const JOIN_RISE: f32 = 0.15;
const JOIN_COS: f32 = 0.94;

/// A level's triangles as vertex triples.
pub fn tri(t: &Tri) -> [Vec3; 3] {
    [t.a, t.b, t.c]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(a: Vec3, b: Vec3, out: Vec3) -> Edge {
        Edge { p0: a, p1: b, out }
    }

    #[test]
    fn two_facing_edges_make_a_beam_along_their_middle() {
        let a = edge(Vec3::new(0.0, 4.0, -0.2), Vec3::new(4.0, 4.0, -0.2), Vec3::NEG_Z);
        let b = edge(Vec3::new(0.0, 4.0, 0.2), Vec3::new(4.0, 4.0, 0.4), Vec3::Z);
        let (beam, _) = pair(&a, &b).unwrap();
        assert!((beam.widths[0] - 0.2).abs() < 0.01 && (beam.widths[1] - 0.3).abs() < 0.01);
    }

    #[test]
    fn too_wide_or_same_facing_edges_do_not_pair() {
        let a = edge(Vec3::ZERO, Vec3::X * 4.0, Vec3::NEG_Z);
        assert!(pair(&a, &edge(Vec3::Z * 1.1, Vec3::Z * 1.1 + Vec3::X * 4.0, Vec3::Z)).is_none());
        assert!(pair(&a, &edge(Vec3::Z * 0.2, Vec3::Z * 0.2 + Vec3::X * 4.0, Vec3::NEG_Z)).is_none());
    }

    #[test]
    fn a_wall_across_the_beam_cuts_it() {
        let beam = Beam { p0: Vec3::ZERO, p1: Vec3::X * 4.0, widths: [0.2; 2] };
        // A wall across it at x 2..3, facing -X and +X.
        let w = |x: f32, n: f32| {
            let (a, b, c, d) = (Vec3::new(x, -1.0, -1.0), Vec3::new(x, 2.0, -1.0), Vec3::new(x, 2.0, 1.0), Vec3::new(x, -1.0, 1.0));
            if n < 0.0 { [[a, c, b], [a, d, c]] } else { [[a, b, c], [a, c, d]] }
        };
        let tris: Vec<[Vec3; 3]> = w(2.0, -1.0).into_iter().chain(w(3.0, 1.0)).collect();
        let out = trim(beam, &tris);
        assert!(!out.is_empty() && out.iter().all(|b| b.p0.x >= 2.0 - 1e-3 || b.p1.x <= 2.0 + 1e-3));
    }

    #[test]
    fn damascus_roof2_north_beam_pairs() {
        let a = edge(Vec3::new(40.99, 11.69, -38.90), Vec3::new(40.99, 11.62, -37.49), Vec3::X);
        let b = edge(Vec3::new(40.72, 11.69, -38.90), Vec3::new(40.72, 11.62, -37.49), Vec3::NEG_X);
        let r = pair(&a, &b);
        eprintln!("{r:?}");
        assert!(r.is_some());
        assert_eq!(detect(&[a, b], |_, _| vec![]).len(), 1);
    }

    #[test]
    fn pieces_of_one_beam_join() {
        let b = |a: f32, c: f32| Beam { p0: Vec3::new(a, 1.0, 0.0), p1: Vec3::new(c, 1.0, 0.0), widths: [0.1; 2] };
        let out = lines(&[b(0.0, 1.0), b(1.3, 2.0), b(5.0, 6.0)]);
        assert_eq!(out.len(), 2);
        assert!(out.iter().any(|l| (l.a.x - 0.0).abs() < 1e-4 && (l.b.x - 2.0).abs() < 1e-4 || (l.b.x - 0.0).abs() < 1e-4 && (l.a.x - 2.0).abs() < 1e-4));
    }

    #[test]
    fn detect_pairs_and_returns_the_rest() {
        let edges =
            [edge(Vec3::new(0.0, 0.0, -0.2), Vec3::new(4.0, 0.0, -0.2), Vec3::NEG_Z), edge(Vec3::new(0.0, 0.0, 0.2), Vec3::new(4.0, 0.0, 0.2), Vec3::Z)];
        let beams = detect(&edges, |_, _| vec![]);
        assert_eq!(beams.len(), 1);
        assert!(beams[0].p0.z.abs() < 1e-4 && (beams[0].widths[0] - 0.2).abs() < 1e-4);
    }
}
