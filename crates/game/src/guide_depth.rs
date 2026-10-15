//! AC1's guidance depth filter: the room in front of a hold, the filter its ledge report and climbing grid run on the
//! guidance edges before any hold is taken (`Human__ApplyDepthToEdges` 0xB264F0 -> `GuidanceDepth__FilterEdge`
//! 0x6744D0). Read in Ghidra, as Banned445's AC1-Movement-Rewritten has it (MIT; written here anew):
//! - `sub_B27E80` sets one detector 0.02 m out of the edge and 0.02 m up (0x3CA3D70A twice), the samples every 0.1 m
//!   (+0xD4) from 0.5 m before the edge's start to 0.5 m past its end (+0xD8);
//! - its callers give the detector's angle and length: the ledge report (`HumanLedge__BuildLedgeReport`, 0xDDCA8E) and the
//!   climbing grid (`HumanClimb__BuildHoldGrid` 0xDF6A40, at 0xDF719E: `0x3F800000, 0xBE32B8C2`) 10 degrees down from
//!   straight out, 1.0 m (`LEDGE`); the pull-down (0xDDCD2F) 60 degrees down, 1.2 m;
//! - each sample is the nearest collision face met along the detector's ray, found by slicing the collision with the
//!   plane through the edge and the ray (a face crossing a sample's 0.1 m band marks it), and a sample is free when that
//!   is no nearer than the length: an edge keeps only its runs of free samples, each a piece of its own.
//!
//! In BuildHoldGrid the report is cut round the ladders (0x1170880), its ladders taken out (0x66C220), its steep edges
//! dropped (0x66C350) and then this filter run (0xB27E80, 0xB28180), so a hold with a wall, a beam or the next plank less
//! than a metre in front of it is not one to hang from (`Level::ac1_holds`).

use bevy::prelude::*;

use crate::level::Level;

/// The workspace's sample spacing and its reach past each end of an edge (m; +0xD4, +0xD8).
pub const SPACING: f32 = 0.1;
pub const REACH: f32 = 0.5;

/// A depth detector: its ray starts `out` out of the wall and `up` up from the edge, points out turned `angle` toward up
/// (rad, negative down) about the edge, `length` long; a sample is free when nothing is met within `threshold` (at most
/// `length`; the caller's threshold array, `Human__DepthFilterReport` 0xB28260).
#[derive(Clone, Copy, Debug)]
pub struct Detector {
    pub out: f32,
    pub up: f32,
    pub angle: f32,
    pub length: f32,
    pub threshold: f32,
}

impl Detector {
    /// 2 cm out and up from the edge (`sub_B27E80`, `sub_B27F70`), its threshold its length.
    const fn edge(angle: f32, length: f32) -> Self {
        Detector { out: 0.02, up: 0.02, angle, length, threshold: length }
    }
    /// 0.27 m out and 0.23 m under the edge (`Human__DepthDetectReport` 0xB28090: the air catch, the jump query, the
    /// wall run).
    const fn lip(angle: f32, length: f32, threshold: f32) -> Self {
        Detector { out: 0.27, up: -0.23, angle, length, threshold }
    }
}

/// 135 degrees from straight out toward up: back over the edge's top (the second detector of the two-detector sets).
const BACK_UP: f32 = 2.356_194_5;

/// The ledge report's and the climbing grid's detector (`sub_B27E80` from 0xDDCA8E and 0xDF719E): 10 degrees down, 1 m.
pub const LEDGE: Detector = Detector::edge(-0.174_532_92, 1.0);
/// The pull-down's ledge report (`sub_DDCB20` 0xDDCD2F): 60 degrees down, 1.2 m.
pub const PULL_DOWN: Detector = Detector::edge(-std::f32::consts::FRAC_PI_3, 1.2);
/// The air catch's reports (`CheckAirCatch` 0xE0BC9D-0xE0BF12, Banned445's `air_catches::catch_reports`): 45 degrees down
/// x 1.2 m and back up x 0.38 m; the hands' searches (the ledge, climb and free-hang catches) want 1.2 / 0.38 of room,
/// the feet's 0.35 / 0.38 (ours' catch takes the feet from the climbing grid's holds).
pub const CATCH_HANDS: [Detector; 2] = [Detector::lip(-std::f32::consts::FRAC_PI_4, 1.2, 1.2), Detector::lip(BACK_UP, 0.38, 0.38)];
/// The jump query's (0xE19113-0xE1914F): 75 degrees down x 0.27 m and back up x 0.42 m.
pub const JUMP: [Detector; 2] = [Detector::lip(-1.308_997, 0.27, 0.27), Detector::lip(BACK_UP, 0.42, 0.42)];

/// An edge's frame (`sub_675350`): along it, out (flat, across it, to the side of `out`), and up (across both, upward).
fn frame(a: Vec3, b: Vec3, out: Vec3) -> Option<(Vec3, Vec3, Vec3, f32)> {
    let d = b - a;
    let len = d.length();
    if len <= 1e-4 {
        return None;
    }
    let t = d / len;
    let mut s = t.cross(Vec3::Y).normalize_or_zero();
    if s == Vec3::ZERO {
        return None;
    }
    if s.dot(out) < 0.0 {
        s = -s;
    }
    let mut z = s.cross(t);
    if z.y < 0.0 {
        z = -z;
    }
    Some((t, s, z, len))
}

/// The depth samples of a slice (`sub_670860`, `sub_6712D0`): for each sample k along `along` from `origin` (every
/// `SPACING` over `span`), the nearest distance along `dir` (at most `length`) at which a face of the level crosses the
/// plane of `along` and `dir` within the sample's band.
fn samples(level: &Level, origin: Vec3, along: Vec3, dir: Vec3, span: f32, length: f32) -> Vec<f32> {
    let count = (span / SPACING) as usize + 1;
    let mut out = vec![length; count];
    let m = along.cross(dir).normalize_or_zero();
    if m == Vec3::ZERO {
        return out;
    }
    let corners = [origin, origin + along * span, origin + dir * length, origin + along * span + dir * length];
    let (lo, hi) = corners.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(l, h), c| (l.min(*c), h.max(*c)));
    let pad = Vec3::splat(0.01);
    let (lo, hi) = (lo - pad, hi + pad);
    let last = count - 1;
    let mut write = |p: Vec3, q: Vec3| {
        let (a0, d0) = ((p - origin).dot(along), (p - origin).dot(dir));
        let (a1, d1) = ((q - origin).dot(along), (q - origin).dot(dir));
        // (Clipped to the slice's rectangle, [0, span] along by [0, length] out.)
        let (mut u0, mut u1) = (0.0f32, 1.0f32);
        let (da, dd) = (a1 - a0, d1 - d0);
        for (pp, qq) in [(-da, a0), (da, span - a0), (-dd, d0), (dd, length - d0)] {
            if pp.abs() < 1e-9 {
                if qq < 0.0 {
                    return;
                }
            } else {
                let r = qq / pp;
                if pp < 0.0 {
                    u0 = u0.max(r);
                } else {
                    u1 = u1.min(r);
                }
            }
        }
        if u0 > u1 {
            return;
        }
        let (mut a, mut b) = ((a0 + da * u0, d0 + dd * u0), (a0 + da * u1, d0 + dd * u1));
        if a.0 > b.0 {
            std::mem::swap(&mut a, &mut b);
        }
        // (Floored in double precision, as the game's x87 code: 0.5 / 0.1f is 4.)
        let i0 = ((a.0 as f64 / SPACING as f64) as usize).min(last);
        let i1 = ((b.0 as f64 / SPACING as f64) as usize).min(last);
        out[i0] = out[i0].min(a.1);
        out[i1] = out[i1].min(b.1);
        if i1 > i0 + 1 && b.0 - a.0 > 1e-9 {
            let slope = (b.1 - a.1) / (b.0 - a.0);
            for (k, v) in out.iter_mut().enumerate().take(i1).skip(i0 + 1) {
                *v = v.min(a.1 + slope * (k as f32 * SPACING - a.0));
            }
        }
    };
    for i in level.tris_near(lo, hi) {
        let t = &level.tris[i as usize];
        let vs = [t.a, t.b, t.c];
        if vs.iter().all(|v| v.x < lo.x) || vs.iter().all(|v| v.x > hi.x) || vs.iter().all(|v| v.y < lo.y) || vs.iter().all(|v| v.y > hi.y) {
            continue;
        }
        if vs.iter().all(|v| v.z < lo.z) || vs.iter().all(|v| v.z > hi.z) {
            continue;
        }
        let sd = vs.map(|v| (v - origin).dot(m));
        let mut pts: [Vec3; 2] = [Vec3::ZERO; 2];
        let mut n = 0;
        for i in 0..3 {
            let j = (i + 1) % 3;
            let p = if sd[i].abs() < 1e-7 {
                Some(vs[i])
            } else if (sd[i] > 0.0) != (sd[j] > 0.0) && sd[j].abs() >= 1e-7 {
                Some(vs[i] + (vs[j] - vs[i]) * (sd[i] / (sd[i] - sd[j])))
            } else {
                None
            };
            if let Some(p) = p {
                if n < 2 {
                    pts[n] = p;
                }
                n += 1;
            }
        }
        if n >= 2 {
            write(pts[0], pts[1]);
        }
    }
    out
}

/// The pieces of the edge `a`-`b` (its wall facing `out`) that every detector finds free (`sub_671740`, `sub_671910`):
/// each as (from, to), metres from `a` along it.
pub fn free_pieces(level: &Level, a: Vec3, b: Vec3, out: Vec3, detectors: &[Detector]) -> Vec<(f32, f32)> {
    let Some((t, s, z, len)) = frame(a, b, out) else { return vec![] };
    let span = len + 2.0 * REACH;
    let start = a - t * REACH;
    let free: Vec<Vec<bool>> = detectors
        .iter()
        .map(|d| {
            let origin = start + s * d.out + z * d.up;
            let dir = s * d.angle.cos() + z * d.angle.sin();
            samples(level, origin, t, dir, span, d.length).into_iter().map(|v| v >= d.threshold).collect()
        })
        .collect();
    let count = free.first().map_or(0, |f| f.len());
    let all = |k: usize| free.iter().all(|f| f[k]);
    let mut pieces = vec![];
    let mut k = 0;
    while k < count {
        if !all(k) {
            k += 1;
            continue;
        }
        let mut j = k + 1;
        while j < count && all(j) {
            j += 1;
        }
        let p0 = (k as f32 * SPACING).clamp(REACH, REACH + len) - REACH;
        let p1 = (j as f32 * SPACING).clamp(REACH, REACH + len) - REACH;
        if p1 - p0 > 0.0005 {
            pieces.push((p0, p1));
        }
        k = j + 1;
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::Tri;

    fn quad(tris: &mut Vec<Tri>, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
        tris.push(Tri { a, b, c });
        tris.push(Tri { a, b: c, c: d });
    }

    /// A wall facing +z with its top at y = 2 along x 0..4; `front`: a post 0.5 m out in front of it at x 1.0..1.4.
    fn wall(front: bool) -> Level {
        let mut tris = vec![];
        quad(&mut tris, Vec3::new(0.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 0.0), Vec3::new(4.0, 2.0, 0.0), Vec3::new(0.0, 2.0, 0.0));
        quad(&mut tris, Vec3::new(0.0, 2.0, 0.0), Vec3::new(4.0, 2.0, 0.0), Vec3::new(4.0, 2.0, -1.0), Vec3::new(0.0, 2.0, -1.0));
        if front {
            // (Its face toward the wall, from the ground to 2.5 m.)
            quad(&mut tris, Vec3::new(1.0, 0.0, 0.5), Vec3::new(1.4, 0.0, 0.5), Vec3::new(1.4, 2.5, 0.5), Vec3::new(1.0, 2.5, 0.5));
        }
        let mut level = Level { tris, ..default() };
        level.build_grid();
        level
    }

    #[test]
    fn an_open_edge_is_free_its_whole_length() {
        let level = wall(false);
        let p = free_pieces(&level, Vec3::new(0.0, 2.0, 0.0), Vec3::new(4.0, 2.0, 0.0), Vec3::Z, &[LEDGE]);
        assert_eq!(p.len(), 1);
        assert!(p[0].0.abs() < 1e-4 && (p[0].1 - 4.0).abs() < 1e-4, "{p:?}");
    }

    #[test]
    fn something_in_front_cuts_the_edge_where_it_stands() {
        let level = wall(true);
        let p = free_pieces(&level, Vec3::new(0.0, 2.0, 0.0), Vec3::new(4.0, 2.0, 0.0), Vec3::Z, &[LEDGE]);
        assert_eq!(p.len(), 2, "{p:?}");
        // (The post, x 1.0..1.4, is 1.5..1.9 m from the samples' start, 0.5 m before the edge: floored by 0.1f in double
        // precision as the game does, samples 14..18 are blocked, so the edge is free up to 0.9 m and from 1.4 m.)
        assert!((p[0].0).abs() < 1e-4 && (p[0].1 - 0.9).abs() < 1e-3, "{p:?}");
        assert!((p[1].0 - 1.4).abs() < 1e-3 && (p[1].1 - 4.0).abs() < 1e-4, "{p:?}");
    }

    #[test]
    fn a_post_further_out_than_the_ray_leaves_it_free() {
        let mut level = wall(false);
        let mut tris = std::mem::take(&mut level.tris);
        quad(&mut tris, Vec3::new(1.0, 0.0, 1.2), Vec3::new(1.4, 0.0, 1.2), Vec3::new(1.4, 2.5, 1.2), Vec3::new(1.0, 2.5, 1.2));
        let mut level = Level { tris, ..default() };
        level.build_grid();
        let p = free_pieces(&level, Vec3::new(0.0, 2.0, 0.0), Vec3::new(4.0, 2.0, 0.0), Vec3::Z, &[LEDGE]);
        assert_eq!(p.len(), 1, "{p:?}");
    }
}
