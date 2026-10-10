//! AC1's jump targets: its candidate query (`Human__QueryJumpCandidates`) and the scorer that picks one, as Banned445's
//! AC1-Movement-Rewritten reads them from the executable (MIT, Copyright (c) 2026 Banned445; its `jump_candidates.rs`,
//! rewritten here over our level's data).
//!
//! The query takes the hold edges inside a box in front of the jumper (1 m either side of the way, 0.5 to 9 m on, 5 m
//! down to 3 m up), joins touching pieces that face the same way into chains, and on each chain takes the grab point
//! where the forward line meets it (kept 0.5 m from the chain's ends). Each grab point is classified by three rays from
//! 0.5 m out and 5 cm over it: the room on top (back toward the wall), what lies below, and a clear line from the chest
//! (0.75 m up). It must lie in AC1's side-view reach zones (13 polygons, read from the running game); the zone it is in
//! decides which jumps it allows (onto it, over it, to hang from it). Beams lying along the way within 40 degrees give
//! the in-zone point nearest 4 m on; edges facing away give the far end of the top behind them.
//!
//! The scorer then prefers, in front of a plane through the hips tilted down ahead: a swing bar, the nearest roof edge
//! (a top at least 1 m deep, or a beam), the highest other target in a 45 degree cone, a far edge below; then behind it
//! the furthest roof edge, the highest other target, a far edge.
//!
//! Game Z-up (forward, up) is ours (forward, Y).

use crate::level::{Level, Line};
use bevy::prelude::*;

/// The reach zones (horizontal distance, height above the feet), by slot.
const ZONES: [&[[f32; 2]]; 13] = [
    &[[0.5, -5.0], [0.5, 0.2], [3.5, 0.6], [4.7, 0.3], [6.0, -0.8], [7.5, -3.0], [7.5, -5.0]],
    &[[0.5, -5.0], [0.5, 1.3], [3.5, 1.3], [4.7, 0.8], [6.0, -0.5], [8.0, -3.0], [8.0, -5.0]],
    &[[0.5, -3.0], [0.5, 2.5], [3.5, 2.5], [5.0, 1.5], [6.5, -0.5], [8.5, -3.0]],
    &[[0.5, -3.0], [0.5, 3.0], [3.5, 3.0], [5.3, 1.8], [7.0, -0.5], [9.0, -3.0]],
    &[[0.5, -5.0], [0.5, 1.3], [4.0, 1.3], [5.2, 0.8], [6.5, -0.5], [8.5, -3.0], [8.5, -5.0]],
    &[[0.5, -3.0], [0.5, 2.5], [4.0, 2.5], [5.5, 1.5], [7.0, -0.5], [9.0, -3.0]],
    &[[0.5, -3.0], [0.5, 3.0], [4.0, 3.0], [5.8, 1.8], [7.5, -0.5], [9.5, -3.0]],
    &[[0.5, -4.3], [0.5, 2.0], [3.5, 2.0], [4.7, 1.5], [6.0, 0.2], [7.0, -2.3], [7.0, -4.3]],
    &[[0.5, -2.3], [0.5, 3.2], [3.5, 3.2], [5.0, 2.3], [6.5, 0.2], [7.5, -2.3]],
    &[[0.5, -2.3], [0.5, 3.7], [3.5, 3.7], [5.3, 2.5], [7.0, 0.2], [8.0, -2.3]],
    &[[0.5, -5.0], [0.5, 0.0], [1.0, -0.2], [2.0, -1.0], [3.0, -3.0], [3.0, -5.0]],
    &[[0.5, -3.0], [0.5, 1.5], [1.0, 1.3], [2.25, -0.3], [3.5, -3.0]],
    &[[0.5, -3.0], [0.5, 2.5], [1.0, 2.3], [2.5, 1.8], [4.0, -3.0]],
];

/// Is `p` inside reach zone `slot` of a jumper with the feet at `origin`?
fn in_zone(slot: usize, origin: Vec3, p: Vec3) -> bool {
    let x = (p - origin).with_y(0.0).length();
    let y = p.y - origin.y;
    let poly = ZONES[slot];
    let mut inside = false;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if (a[1] > y) != (b[1] > y) {
            let t = (y - a[1]) / (b[1] - a[1]);
            if x < a[0] + t * (b[0] - a[0]) {
                inside = !inside;
            }
        }
    }
    inside
}

/// A beam along the way is a candidate where the forward line passes this close to it (m).
const BEAM_MEET: f32 = 0.6;

/// Who asks: a jump from the ground (a press, or free running off an edge) or from a beam or post (outside the
/// ground: the narrow-object zones, only the first counts).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum From {
    Ground,
    Narrow,
}

struct Zones {
    primary: usize,
    secondary: usize,
    tertiary: usize,
    only_primary: bool,
    /// The box (across, on, up) from the feet.
    min: Vec3,
    max: Vec3,
}

fn zones(from: From) -> Zones {
    match from {
        From::Ground => Zones { primary: 1, secondary: 2, tertiary: 3, only_primary: false, min: Vec3::new(-1.0, 0.5, -5.0), max: Vec3::new(1.0, 9.0, 3.0) },
        From::Narrow => Zones { primary: 7, secondary: 8, tertiary: 9, only_primary: true, min: Vec3::new(-1.0, 0.5, -4.3), max: Vec3::new(1.0, 9.0, 3.7) },
    }
}

/// What a candidate is.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    /// A hold edge (a roof's, a wall top's, a post's).
    Edge,
    /// A beam lying along the way.
    Beam,
    /// The far end of a top, landed on short of it.
    FarEdge,
    /// A swing bar.
    Pole,
}

#[derive(Clone, Copy, Debug)]
pub struct Candidate {
    pub pos: Vec3,
    /// The wall's normal, flat, out of the solid (toward the jumper for an edge facing it).
    pub wall: Vec3,
    /// Room on top: 2 (1 m or more), 8 (0.3-1 m), 16 (under 0.3 m), 32 a pole, 4 a beam.
    pub ty: u32,
    /// Below: 2 a floor within 0.5 m, 4 within 2 m, 8 a deep drop with a wall under it, 16 without; 1 beams, far edges.
    pub sub: u32,
    /// The jumps it allows: 1 onto it, 2 over it, 0x40 a wall hang, 0x80 a free hang, 0x10000 a beam or far edge.
    pub flags: u32,
    pub kind: Kind,
    /// The perch it lies on (beams), to leave out the one stood on.
    pub perch: Option<usize>,
}

/// The jump chosen: the candidate and the jump type (1 onto a top, 2 over it, 0x40 / 0x80 to hang, 0x10000 onto a
/// beam or far edge).
pub type Choice = (Candidate, u32);

fn ray_distance(level: &Level, from: Vec3, dir: Vec3, max: f32) -> f32 {
    level.raycast(from, dir.normalize_or_zero(), max).map_or(max, |h| h.dist)
}

fn clear(level: &Level, from: Vec3, to: Vec3) -> bool {
    let d = to - from;
    let len = d.length();
    len < 1e-4 || ray_distance(level, from, d / len, len) >= len - 1e-4
}

/// Clip the segment `a`-`b` to the box of half extents `half` round the origin (Liang-Barsky).
fn clip_segment(a: Vec3, b: Vec3, half: Vec3) -> Option<(Vec3, Vec3)> {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for k in 0..3 {
        for (p, q) in [(-d[k], a[k] + half[k]), (d[k], half[k] - a[k])] {
            if p.abs() < 1e-9 {
                if q < 0.0 {
                    return None;
                }
            } else {
                let r = q / p;
                if p < 0.0 {
                    t0 = t0.max(r);
                } else {
                    t1 = t1.min(r);
                }
            }
        }
    }
    (t0 <= t1).then(|| (a + d * t0, a + d * t1))
}

/// The point of `a`-`b` shortened by `step` at both ends nearest `p` (its middle when shorter than two steps).
fn nearest_on_shortened(p: Vec3, a: Vec3, b: Vec3, step: f32) -> Vec3 {
    let len = a.distance(b);
    if len <= 2.0 * step + 0.0005 {
        return (a + b) * 0.5;
    }
    let d = (b - a) / len;
    Line { a: a + d * step, b: b - d * step }.closest(p)
}

/// The grab point on a chain: `centre` projected along `dir` onto the chain's vertical plane, kept 0.5 m from its ends.
fn grab_point(chain: &[(usize, Vec3, Vec3)], centre: Vec3, dir: Vec3) -> Option<(usize, Vec3)> {
    let s = chain.first()?.1;
    let e = chain.last()?.2;
    let u = (e - s).with_y(0.0).normalize_or_zero();
    if u == Vec3::ZERO {
        let (pi, a, b) = chain[0];
        return Some((pi, (a + b) * 0.5));
    }
    let n = u.cross(Vec3::Y);
    let denom = dir.dot(n);
    let proj = if denom.abs() > 1e-6 { centre + dir * ((s - centre).dot(n) / denom) } else { centre - n * (centre - s).dot(n) };
    let len = (e - s).with_y(0.0).length();
    let t = (proj - s).dot(u);
    let t = if len > 1.0 { t.clamp(0.5, len - 0.5) } else { len * 0.5 };
    let mut best: Option<(f32, usize, Vec3)> = None;
    for &(pi, a, b) in chain {
        let (ta, tb) = ((a - s).dot(u), (b - s).dot(u));
        if (tb - ta).abs() > 1e-6 && (ta.min(tb)..=ta.max(tb)).contains(&t) {
            return Some((pi, a.lerp(b, (t - ta) / (tb - ta))));
        }
        for (q, tq) in [(a, ta), (b, tb)] {
            let d = (tq - t).abs();
            if best.is_none_or(|x| d < x.0) {
                best = Some((d, pi, q));
            }
        }
    }
    best.map(|b| (b.1, b.2))
}

/// The candidates for a jump from the feet at `pos` along `dir`. `skip`: the perch stood on.
pub fn query(level: &Level, pos: Vec3, dir: Vec3, from: From, skip: Option<usize>) -> Vec<Candidate> {
    let dir = dir.with_y(0.0).normalize_or_zero();
    if dir == Vec3::ZERO {
        return vec![];
    }
    let up = Vec3::Y;
    let right = dir.cross(up).normalize();
    let z = zones(from);
    let centre = pos + dir * 0.15 - up * 5.0;
    let box_mid = (z.min + z.max) * 0.5;
    let half = (z.max - z.min) * 0.5;
    let local = |p: Vec3| {
        let d = p - pos;
        Vec3::new(d.dot(right), d.dot(dir), d.dot(up)) - box_mid
    };
    let world = |l: Vec3| {
        let l = l + box_mid;
        pos + right * l.x + dir * l.y + up * l.z
    };
    let in1 = |p: Vec3| in_zone(z.primary, pos, p);
    let eye = pos + up * 0.75;
    let mut out = vec![];

    // The hold edges in the box, clipped to it, by the way they face: toward the jumper (within 45 degrees), to either
    // side, away.
    struct Piece {
        a: Vec3,
        b: Vec3,
        wall: Vec3,
        sector: u8,
    }
    let reach = pos + dir * 5.0;
    let mut pieces: Vec<Piece> = vec![];
    for l in &level.ledges {
        let mid = (l.a + l.b) * 0.5;
        if (mid - reach).with_y(0.0).length() > 6.0 + l.a.distance(l.b) * 0.5 {
            continue;
        }
        let Some((a, b)) = clip_segment(local(l.a), local(l.b), half) else { continue };
        let (a, b) = (world(a), world(b));
        if a.distance(b) <= 0.0005 || (b - a).normalize().y.abs() > 0.643 {
            continue;
        }
        let w = l.out.with_y(0.0).normalize_or_zero();
        if w == Vec3::ZERO {
            continue;
        }
        let ang = w.angle_between(-dir);
        let sector = if ang <= std::f32::consts::FRAC_PI_4 + 1e-4 {
            0
        // (Ours: to either side only up to 90 degrees; an edge facing away is a far edge's. Banned445's port takes up
        // to 135 degrees, the chain builder being undecoded; that made a Damascus roof's own diagonal corner, 0.8 m on,
        // a target.)
        } else if ang <= std::f32::consts::FRAC_PI_2 + 1e-4 {
            if w.dot(right) > 0.0 { 1 } else { 2 }
        } else {
            3
        };
        pieces.push(Piece { a, b, wall: w, sector });
    }

    // Chains of touching pieces facing the same way.
    let mut used = vec![false; pieces.len()];
    let mut chains: Vec<Vec<(usize, Vec3, Vec3)>> = vec![];
    for s in 0..pieces.len() {
        if used[s] || pieces[s].sector == 3 {
            continue;
        }
        used[s] = true;
        let mut chain = std::collections::VecDeque::from([(s, pieces[s].a, pieces[s].b)]);
        loop {
            let tail = chain.back().unwrap().2;
            let head = chain.front().unwrap().1;
            let mut grew = false;
            for o in 0..pieces.len() {
                if used[o] || pieces[o].sector != pieces[s].sector {
                    continue;
                }
                let (a, b) = (pieces[o].a, pieces[o].b);
                if a.distance(tail) < 0.01 {
                    chain.push_back((o, a, b));
                } else if b.distance(tail) < 0.01 {
                    chain.push_back((o, b, a));
                } else if b.distance(head) < 0.01 {
                    chain.push_front((o, a, b));
                } else if a.distance(head) < 0.01 {
                    chain.push_front((o, b, a));
                } else {
                    continue;
                }
                used[o] = true;
                grew = true;
                break;
            }
            if !grew {
                break;
            }
        }
        chains.push(chain.into_iter().collect());
    }

    // A grab point per chain, classified.
    for chain in &chains {
        let Some((pi, g)) = grab_point(chain, centre, dir) else { continue };
        let piece = &pieces[pi];
        let w = piece.wall;
        // (Off the forward line, it must face the jumper within 45 degrees.)
        let (sa, sb) = ((piece.a - centre).dot(right), (piece.b - centre).dot(right));
        if sa * sb > 0.0 && dir.dot(w) > -std::f32::consts::FRAC_1_SQRT_2 {
            continue;
        }
        let (zone1, zone2) = if in1(g) {
            (true, true)
        } else if z.only_primary {
            continue;
        } else if in_zone(z.secondary, pos, g) {
            (false, true)
        } else if in_zone(z.tertiary, pos, g) {
            (false, false)
        } else {
            continue;
        };
        let o = g + w * 0.5 + up * 0.05;
        let room = ray_distance(level, o, -w, 1.75) - 0.5;
        let ty = if room < 0.03 {
            64
        } else if room < 0.3 {
            16
        } else if room < 1.0 {
            8
        } else {
            2
        };
        let below = ray_distance(level, o, -up, 3.05) - 0.05;
        let sub = if below < 0.5 {
            2
        } else if below < 2.0 {
            4
        } else if ray_distance(level, g + w * 0.5 - up, -w, 1.4) - 0.5 >= 0.7 {
            16
        } else {
            8
        };
        let mut flags: u32 = 0xC3;
        if g.y - pos.y < -0.5 {
            flags &= !2;
        }
        if !zone1 {
            flags &= !3;
        }
        if !zone2 {
            flags &= !0x40;
        }
        if ty & 0x40 != 0 {
            continue;
        }
        if ty & 0x10 != 0 {
            flags &= !1;
        }
        if ty & 2 == 0 {
            flags &= !2;
        }
        if below < 2.5 {
            flags &= !0x80;
        }
        if sub & 6 != 0 {
            flags &= !0xC0;
        }
        if sub & 0x10 != 0 {
            flags &= !0x40;
        }
        if flags & 3 != 0 && !clear(level, eye, g + w * 0.2) {
            flags &= !3;
        }
        if flags & 0xC0 != 0 && !clear(level, eye, g + w * 0.2 - up * 1.1) {
            flags &= !0xC0;
        }
        if flags != 0 {
            out.push(Candidate { pos: g, wall: w, ty, sub, flags, kind: Kind::Edge, perch: None });
        }
    }

    // Swing bars across the way: where the forward line meets them, a pole.
    for b in &level.bars {
        let along = b.axis().with_y(0.0);
        let (sa, sb) = ((b.a - centre).dot(right), (b.b - centre).dot(right));
        if along == Vec3::ZERO || sa * sb > 0.0 {
            continue;
        }
        let g = b.a.lerp(b.b, sa / (sa - sb));
        let l = local(g);
        if l.abs().cmpgt(half).any() {
            continue;
        }
        let zone = in1(g) || (!z.only_primary && (in_zone(z.secondary, pos, g) || in_zone(z.tertiary, pos, g)));
        if !zone || ray_distance(level, g - Vec3::Y * 0.05, -up, 3.05) < 2.5 || !clear(level, eye, g - up * 1.1) {
            continue;
        }
        let mut w = along.cross(up).normalize_or_zero();
        if w.dot(dir) > 0.0 {
            w = -w;
        }
        out.push(Candidate { pos: g, wall: w, ty: 32, sub: 16, flags: 0x80, kind: Kind::Pole, perch: None });
    }

    // Beams along the way (within 40 degrees): the in-zone point, 0.3 m from their ends, nearest 4 m on. Posts: their top.
    let ahead = pos + dir * 4.0;
    for (i, l) in level.perches.iter().enumerate() {
        if Some(i) == skip {
            continue;
        }
        let along = l.axis().with_y(0.0).normalize_or_zero();
        if along == Vec3::ZERO {
            // (A post: landed on in its middle; its edges read as a roof's to AC1, the room-on-top ray passing over it.)
            let p = l.a;
            if local(p).abs().cmpgt(half).any() || !in1(p) || !clear(level, eye, p + up * 0.05) {
                continue;
            }
            out.push(Candidate { pos: p, wall: -(p - eye).with_y(0.0).normalize_or_zero(), ty: 2, sub: 4, flags: 1, kind: Kind::Edge, perch: Some(i) });
            continue;
        }
        if along.dot(dir).abs() <= 0.766_044_4 {
            // (Across the way: where the forward line crosses it, as a roof's edge (as a post's). AC1 finds these by the hold edges
            // along the beam's sides; ours has beams without them, the gauntlet's stuck out of a wall.)
            let (sa, sb) = ((l.a - centre).dot(right), (l.b - centre).dot(right));
            if sa * sb > 0.0 {
                continue;
            }
            let p = l.a.lerp(l.b, sa / (sa - sb));
            if local(p).abs().cmpgt(half).any() || !in1(p) || !clear(level, eye, p + up * 0.05) {
                continue;
            }
            out.push(Candidate { pos: p, wall: -(p - eye).with_y(0.0).normalize_or_zero(), ty: 2, sub: 4, flags: 1, kind: Kind::Edge, perch: Some(i) });
            continue;
        }
        // (Ours: the forward line passing within `BEAM_MEET` of the beam somewhere on. AC1 clips the beam to the zone's
        // front (0x116E070, not decoded); taken over the whole box, a Damascus beam 0.68 m beside the way was jumped at
        // where the running game went on along the way onto a roof's edge.)
        if !(2..=36).any(|k| (l.closest(pos + dir * (k as f32 * 0.25)) - (pos + dir * (k as f32 * 0.25))).with_y(0.0).length() < BEAM_MEET) {
            continue;
        }
        let len = l.a.distance(l.b);
        let steps = (len / 0.1).ceil().max(1.0) as usize;
        let margin = (0.3 / len.max(1e-3)).min(0.5);
        let p = (0..=steps)
            .map(|s| s as f32 / steps as f32)
            .filter(|t| (margin..=1.0 - margin).contains(t))
            .map(|t| l.a.lerp(l.b, t))
            .filter(|p| local(*p).abs().cmple(half).all() && in1(*p))
            .min_by(|x, y| x.distance_squared(ahead).total_cmp(&y.distance_squared(ahead)));
        let Some(p) = p else { continue };
        if !clear(level, eye, p + up * 0.05) {
            continue;
        }
        out.push(Candidate { pos: p, wall: -(p - eye).with_y(0.0).normalize_or_zero(), ty: 4, sub: 1, flags: 0x10000, kind: Kind::Beam, perch: Some(i) });
    }

    // Far edges: an edge facing away, the top behind it landed on 0.5 m short of it.
    for piece in &pieces {
        let w = piece.wall;
        let facing = dir.dot(w);
        if facing <= 0.0 {
            continue;
        }
        let (sa, sb) = ((piece.a - centre).dot(right), (piece.b - centre).dot(right));
        let p = if sa * sb <= 0.0 {
            piece.a.lerp(piece.b, sa / (sa - sb))
        } else if facing >= 0.5 {
            let (near, far) = if sa.abs() <= sb.abs() { (piece.a, piece.b) } else { (piece.b, piece.a) };
            nearest_on_shortened(near, near, far, 0.5)
        } else {
            continue;
        };
        if !in1(p) {
            continue;
        }
        let beyond = p + w * 0.3 + up * 0.5;
        if !clear(level, beyond, beyond - up) {
            continue;
        }
        let onto = p - w * 0.5 + up * 0.5;
        let d = ray_distance(level, onto, -up, 1.0);
        if d >= 1.0 - 1e-4 {
            continue;
        }
        let top = onto - up * d;
        if !clear(level, beyond, beyond - w * 1.3) || !clear(level, eye, top + up * 0.2) {
            continue;
        }
        out.push(Candidate { pos: top, wall: w, ty: 2, sub: 1, flags: 0x10000, kind: Kind::FarEdge, perch: None });
    }
    out
}

/// The jump type of a candidate's flags, by priority.
fn first_type(flags: u32) -> u32 {
    for t in [0x10000, 1, 2, 4, 8, 0x40, 0x80] {
        if flags & t != 0 {
            return t;
        }
    }
    1
}

/// AC1's scorer: the candidate to jump at from the feet at `actor`, facing `facing`, the stick `want`. `behind_far`:
/// far edges behind count too (the jump off an edge).
pub fn select(cands: &[Candidate], actor: Vec3, facing: Vec3, want: Vec3, behind_far: bool) -> Option<Choice> {
    let flat = |v: Vec3| v.with_y(0.0).normalize_or_zero();
    let want = if want.length_squared() > 2.5e-7 { flat(want) } else { flat(facing) };
    let plane_n = (flat(facing) * 0.5 + Vec3::Y * 0.7).normalize();
    let ledge_like = |c: &Candidate| (c.flags & 1 != 0 && c.ty == 2) || (c.flags & 0x10000 != 0 && c.sub == 1 && c.ty == 4);
    let is_pole = |c: &Candidate| c.flags & 0x80 != 0 && c.sub == 16 && c.ty == 32;
    let is_far = |c: &Candidate| c.flags & 0x10000 != 0 && c.sub == 1 && c.ty == 2;
    let (mut front_ledge, mut front_ledge_d, mut front_ledge_dz) = (None, 1000.0f32, -1000.0f32);
    let (mut pole, mut pole_d, mut pole_dz) = (None, 1000.0f32, -1000.0f32);
    let (mut front_far, mut front_far_d) = (None, 0.0f32);
    let (mut front_other, mut front_other_dz) = (None, -1000.0f32);
    let (mut back_ledge, mut back_ledge_d) = (None, -1000.0f32);
    let (mut back_far, mut back_far_d) = (None, 0.0f32);
    let (mut back_other, mut back_other_dz) = (None, -1000.0f32);
    for (i, c) in cands.iter().enumerate() {
        if c.sub == 2 {
            continue;
        }
        let theta = (-flat(c.wall)).dot(want).clamp(-1.0, 1.0).acos();
        let d = (c.pos - actor).with_y(0.0).length();
        let dz = c.pos.y - actor.y;
        let front = (c.pos - (actor + Vec3::Y * 0.5)).dot(plane_n) > 0.0;
        let cone = theta < std::f32::consts::FRAC_PI_4 && dz > -3.0;
        if !front {
            if ledge_like(c) {
                if d > back_ledge_d {
                    (back_ledge, back_ledge_d) = (Some(i), d);
                }
            } else if is_far(c) {
                if behind_far && d > back_far_d {
                    (back_far, back_far_d) = (Some(i), d);
                }
            } else if cone && dz > back_other_dz {
                (back_other, back_other_dz) = (Some(i), dz);
            }
        } else {
            if ledge_like(c) && d < front_ledge_d {
                (front_ledge, front_ledge_d, front_ledge_dz) = (Some(i), d, dz);
            }
            if is_pole(c) {
                if cone && ((pole_d > d && pole_dz - dz < 2.5) || dz - pole_dz > 2.5) {
                    (pole, pole_d, pole_dz) = (Some(i), d, dz);
                }
            } else if is_far(c) {
                if dz < 0.5 && (dz < -0.5 || d > 3.0) && d > front_far_d {
                    (front_far, front_far_d) = (Some(i), d);
                }
            } else if !ledge_like(c) && cone && dz > front_other_dz {
                (front_other, front_other_dz) = (Some(i), dz);
            }
        }
    }
    let ledge_type = |c: &Candidate| if c.flags & 1 != 0 { 1 } else { 0x10000 };
    let pick = |i: usize, t: u32| Some((cands[i], t));
    if let Some(p) = pole {
        match front_ledge {
            None => return pick(p, 0x80),
            Some(l) if l != p && front_ledge_dz <= pole_dz && (pole_d <= front_ledge_d || pole_dz - front_ledge_dz >= 2.5) => return pick(p, 0x80),
            _ => {}
        }
    }
    if let Some(l) = front_ledge {
        return pick(l, ledge_type(&cands[l]));
    }
    if let Some(i) = front_other {
        return pick(i, first_type(cands[i].flags));
    }
    if let Some(i) = front_far {
        return pick(i, 0x10000);
    }
    if let Some(i) = back_ledge {
        return pick(i, ledge_type(&cands[i]));
    }
    if let Some(i) = back_other {
        return pick(i, first_type(cands[i].flags));
    }
    back_far.map(|i| (cands[i], 0x10000))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reach_zone_runs_from_half_a_metre_and_drops_away_far_on() {
        let o = Vec3::ZERO;
        assert!(in_zone(1, o, Vec3::new(0.0, 1.0, -2.0)));
        assert!(!in_zone(1, o, Vec3::new(0.0, 1.0, -0.3)));
        // (6 m on, level: past where zone 1 has dropped to -0.5 m.)
        assert!(!in_zone(1, o, Vec3::new(0.0, 0.0, -6.0)));
        assert!(in_zone(1, o, Vec3::new(0.0, -1.0, -6.0)));
        // From a beam (zone 7) level at 5.6 m is in reach.
        assert!(in_zone(7, o, Vec3::new(0.0, 0.0, -5.6)));
    }

    #[test]
    fn clipping_keeps_the_part_inside_the_box() {
        let (a, b) = clip_segment(Vec3::new(-3.0, 0.0, 0.0), Vec3::new(3.0, 0.0, 0.0), Vec3::ONE).unwrap();
        assert!((a.x + 1.0).abs() < 1e-5 && (b.x - 1.0).abs() < 1e-5);
        assert!(clip_segment(Vec3::new(-3.0, 2.0, 0.0), Vec3::new(3.0, 2.0, 0.0), Vec3::ONE).is_none());
    }
}
