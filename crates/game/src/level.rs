//! Test level: triangle geometry shared by rendering and IK/locomotion raycasts.
//! Stairs, a ramp, rough ground and a climbable wall with ledges.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub struct Tri {
    pub a: Vec3,
    pub b: Vec3,
    pub c: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub point: Vec3,
    pub normal: Vec3,
    pub dist: f32,
}

/// A horizontal ledge on a wall: grab line from `a` to `b`, `out` points away from the wall.
#[derive(Clone, Copy, Debug)]
pub struct Ledge {
    pub a: Vec3,
    pub b: Vec3,
    pub out: Vec3,
}

impl Ledge {
    /// Closest point on the ledge to `p` (clamped to the ends).
    pub fn closest(&self, p: Vec3) -> Vec3 {
        let d = self.b - self.a;
        let t = ((p - self.a).dot(d) / d.length_squared()).clamp(0.0, 1.0);
        self.a + d * t
    }
}

/// Beam ends sticking out of a wall less than this are hand holds too (m).
const STUB_MAX: f32 = 0.8;

/// A line segment: the top of a post (`a == b`) or a beam to stand on, or a bar to swing on.
#[derive(Clone, Copy, Debug)]
pub struct Line {
    pub a: Vec3,
    pub b: Vec3,
}

impl Line {
    /// Closest point on the segment to `p`.
    pub fn closest(&self, p: Vec3) -> Vec3 {
        let d = self.b - self.a;
        if d.length_squared() < 1e-6 {
            return self.a;
        }
        let t = ((p - self.a).dot(d) / d.length_squared()).clamp(0.0, 1.0);
        self.a + d * t
    }
    /// Unit direction from `a` to `b` (zero for a post).
    pub fn axis(&self) -> Vec3 {
        (self.b - self.a).normalize_or_zero()
    }
}

/// A ladder on a wall: `base` is the foot of its rungs (on the ground, on the rung plane), `top` the
/// height of the walkable top it leads to, `out` points away from the wall.
#[derive(Clone, Copy, Debug)]
pub struct Ladder {
    pub base: Vec3,
    pub top: f32,
    pub out: Vec3,
}

/// A haystack to dive into: square footprint around `centre` (on the ground), `height` tall. Not part
/// of the collision geometry (the climber sinks into it).
#[derive(Clone, Copy, Debug)]
pub struct HayStack {
    pub centre: Vec3,
    pub half: f32,
    pub height: f32,
}

impl HayStack {
    pub fn top(&self) -> f32 {
        self.centre.y + self.height
    }
    /// Is `p` over the footprint, `margin` in from its edges?
    pub fn contains(&self, p: Vec3, margin: f32) -> bool {
        let d = (p - self.centre).abs();
        d.x <= self.half - margin && d.z <= self.half - margin
    }
}

/// A city haystack is at most this wide each side of its middle (m).
const HAY_HALF_MAX: f32 = 1.5;
/// A city haystack's height when its mesh gives none (m).
const CITY_HAY_HEIGHT: f32 = 1.8;

/// An authored guidance edge (`forge::guidance`), in the world: its kind, ends and the way out from its wall.
#[derive(Clone, Copy, Debug)]
pub struct Authored {
    pub kind: forge::guidance::SubType,
    pub a: Vec3,
    pub b: Vec3,
    pub out: Vec3,
}

/// An authored ledge edge sloping more than this (rise over length) is a stair or roof slope, not a hold.
const AUTHORED_MAX_SLOPE: f32 = 0.35;
/// AC1's hold grid shortens a guidance edge this much at its ends (m; `BuildHoldGrid` 0xDF6A40, docs/PARKOUR.md
/// section 4): a lone piece 0.2 m long or less holds nothing.
const AUTHORED_TRIM: f32 = 0.1;
/// A top behind a hold this deep or less (m), with a drop past it, is a narrow wall top to balance on, not to stand:
/// a beam's width. (At 0.6 m Damascus had 10,557, its parapets among them, and running along a roof switched in and
/// out of balancing at every one; AC1 runs on those as ground, its beams found by `GuidanceBeamDetectorAccurate`.)
const NARROW_TOP_MAX: f32 = 0.35;
/// Rays this long (m) decide whether a point is inside a solid (`inside_solid`).
const INSIDE_PROBE: f32 = 12.0;

/// A city mesh's part in parkour (see `city::city_object`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CityObject {
    Hay,
    Ladder,
    Bench,
    Pole,
}

/// A lip is a hold only if the wall under it is at most this far behind it (m).
const LIP_WALL_IN: f32 = 0.25;

#[derive(Resource, Default)]
pub struct Level {
    /// The city's navigation meshes, for walking routes (see `nav`).
    pub nav: Option<crate::nav::NavGraph>,
    pub tris: Vec<Tri>,
    pub ledges: Vec<Ledge>,
    /// AC1's authored guidance edges (cities): see `use_authored`.
    pub authored: Vec<Authored>,
    /// Ladders came from the authored edges (the name-based ones in `add_city_objects` are then left out).
    pub authored_ladders: bool,
    /// City meshes that are parkour objects, by kind, with their world bounds (see `add_city_objects`).
    pub city_objects: Vec<(CityObject, Vec3, Vec3)>,
    /// Where walkable tops end, not yet known to be holds (see `add_probed_ledges`).
    pub lip_candidates: Vec<Ledge>,
    /// Points of interest heads turn toward.
    pub interest: Vec<Vec3>,
    pub haystacks: Vec<HayStack>,
    /// Benches to sit on and hide: (seat line on the floor, from, to; the way a sitter faces).
    pub benches: Vec<(Vec3, Vec3, Vec3)>,
    /// Viewpoints: the perch at the top of a tower to synchronize from (and leap from).
    pub viewpoints: Vec<Vec3>,
    /// Narrow tops (posts and beams) the climber balances on; part of the collision geometry.
    pub perches: Vec<Line>,
    /// Horizontal bars to swing on (drawn, not part of the collision geometry).
    pub bars: Vec<Line>,
    pub ladders: Vec<Ladder>,
    /// Overhead frames to cross hand over hand (a kiosk's roof frame), along the line at its height.
    pub monkey: Vec<Line>,
    /// Triangles by `GRID` cell (x, z) they overlap, for raycasts in big levels (empty: test them all).
    pub(crate) grid: HashMap<(i32, i32), Vec<u32>>,
    /// Triangles too big for the grid (the ground plane around the test area), tested by every raycast.
    pub(crate) big: Vec<u32>,
    /// Where the player starts and a loop for a crowd, when the level says (a city).
    pub spawn: Option<Vec3>,
    pub crowd_path: Option<Vec<Vec3>>,
    /// A city from the game data (not the test level).
    pub city: bool,
    /// Beams' top lines from the city, before `add_beam_perches` keeps their open stretches as perches.
    pub beam_tops: Vec<Line>,
    /// Names shown over the test world's buildings and features (for finding things while testing).
    pub labels: Vec<(String, Vec3)>,
}

/// Test level holds: strips this deep (m) out from their wall, the grab line 6 cm out (where the hands hold; the strip's
/// outer edge, beyond it, is outlined as well: see `draw_edges`).
const HOLD_DEPTH: f32 = 0.12;

/// Where the player starts in Masyaf (Bevy x, _, z): the village below the fortress.
const MASYAF_SPAWN: [f32; 3] = [20.0, 0.0, 50.0];

/// Raycast grid cell size (m); triangles spanning more cells than `GRID_MAX_CELLS` (ground planes, distant
/// backdrops) are kept apart and tested by every raycast.
const GRID: f32 = 4.0;
const GRID_MAX_CELLS: i32 = 4096;

fn cell(v: f32) -> i32 {
    (v / GRID).floor() as i32
}

impl Level {
    /// Index the triangles by grid cell (call after adding geometry).
    pub fn build_grid(&mut self) {
        self.grid.clear();
        self.big.clear();
        for (i, t) in self.tris.iter().enumerate() {
            let (lo, hi) = (t.a.min(t.b).min(t.c), t.a.max(t.b).max(t.c));
            let (x0, x1, z0, z1) = (cell(lo.x), cell(hi.x), cell(lo.z), cell(hi.z));
            if (x1 - x0 + 1) * (z1 - z0 + 1) > GRID_MAX_CELLS {
                self.big.push(i as u32);
                continue;
            }
            for x in x0..=x1 {
                for z in z0..=z1 {
                    self.grid.entry((x, z)).or_default().push(i as u32);
                }
            }
        }
        debug!("level: {} triangles too big for the grid", self.big.len());
    }

    pub fn raycast(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<Hit> {
        self.raycast_sided(origin, dir, max).map(|(h, _)| h)
    }

    /// `raycast`, and whether the face hit was met from behind (its winding faces away from the ray: the ray
    /// started inside a solid).
    pub fn raycast_sided(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<(Hit, bool)> {
        if self.grid.is_empty() {
            return self.raycast_tris(self.tris.iter(), origin, dir, max);
        }
        let end = origin + dir * max;
        let (lo, hi) = (origin.min(end), origin.max(end));
        let mut best = self.raycast_tris(self.big.iter().map(|&i| &self.tris[i as usize]), origin, dir, max);
        for x in cell(lo.x)..=cell(hi.x) {
            for z in cell(lo.z)..=cell(hi.z) {
                let Some(list) = self.grid.get(&(x, z)) else { continue };
                if let Some(h) = self.raycast_tris(list.iter().map(|&i| &self.tris[i as usize]), origin, dir, max)
                    && best.is_none_or(|b| h.0.dist < b.0.dist)
                {
                    best = Some(h);
                }
            }
        }
        best
    }

    fn raycast_tris<'t>(&self, tris: impl Iterator<Item = &'t Tri>, origin: Vec3, dir: Vec3, max: f32) -> Option<(Hit, bool)> {
        let mut best: Option<(Hit, bool)> = None;
        for t in tris {
            let e1 = t.b - t.a;
            let e2 = t.c - t.a;
            let p = dir.cross(e2);
            let det = e1.dot(p);
            if det.abs() < 1e-8 {
                continue;
            }
            let inv = 1.0 / det;
            let s = origin - t.a;
            let u = s.dot(p) * inv;
            if !(0.0..=1.0).contains(&u) {
                continue;
            }
            let q = s.cross(e1);
            let v = dir.dot(q) * inv;
            if v < 0.0 || u + v > 1.0 {
                continue;
            }
            let d = e2.dot(q) * inv;
            if d < 0.0 || d > max || best.is_some_and(|b| b.0.dist <= d) {
                continue;
            }
            let mut n = e1.cross(e2).normalize_or_zero();
            let behind = n.dot(dir) > 0.0;
            if behind {
                n = -n;
            }
            best = Some((Hit { point: origin + dir * d, normal: n, dist: d }, behind));
        }
        best
    }

    /// The stretches of the beams' top lines that are out in the open (a stub shorter than `STUB_MAX` sticking
    /// out of a wall also gets a hand hold across its end) (headroom above, the beam's top the
    /// first thing below; not inside the walls they are stuck into) become perches: beam ends sticking out
    /// of walls to jump between, and beams across streets to walk along.
    /// Keep the lip candidates that are holds, judged against the built collision: a wall right under the lip
    /// facing out (its face within `LIP_WALL_IN` of the lip, 0.3 and 0.9 m down), the top level with the lip
    /// just behind it, and room for the hands in front of it; not where a hold already runs. Returns how many.
    pub fn add_probed_ledges(&mut self) -> usize {
        let cands = std::mem::take(&mut self.lip_candidates);
        let mut found = vec![];
        for c in &cands {
            let out = c.out;
            if out == Vec3::ZERO {
                continue;
            }
            let held = |p: Vec3| {
                // (Down to 0.9 m under the lip too: a step's riser or a kerb is not a wall to hang on.)
                let face = |down: f32| {
                    self.raycast(p + out * 0.4 - Vec3::Y * down, -out, 0.7)
                        .is_some_and(|h| h.normal.dot(out) > 0.7 && (h.point - p).dot(out).abs() < LIP_WALL_IN)
                };
                let wall = face(0.3) && face(0.9);
                let top = self.ground(p - out * 0.12 + Vec3::Y * 0.3, 0.0, 0.45).is_some_and(|g| (g.point.y - p.y).abs() < 0.06);
                let room = self.raycast(p + out * 0.45 + Vec3::Y * 0.12, -out, 0.4).is_none();
                wall && top && room
            };
            let pts = [0.2, 0.5, 0.8].map(|t| c.a.lerp(c.b, t));
            if pts.iter().filter(|p| held(**p)).count() < 2 {
                continue;
            }
            let mid = (c.a + c.b) * 0.5;
            if self.ledges.iter().any(|l| l.closest(mid).distance(mid) < 0.15 && l.out.dot(out) > 0.7) {
                continue;
            }
            found.push(*c);
        }
        let n = found.len();
        self.ledges.extend(found);
        n
    }

    /// Set up the city's parkour objects from their bounds, against the built collision: haystacks (one per cart or
    /// pile, its parts merged), ladders against their wall (the side a probe finds it on), benches facing away from
    /// a wall behind them, horizontal poles as swing bars along their length. Returns (hay, ladders, benches, bars).
    pub fn add_city_objects(&mut self) -> (usize, usize, usize, usize) {
        let objs = std::mem::take(&mut self.city_objects);
        let (mut hay, mut ladders, mut benches, mut bars) = (0, 0, 0, 0);
        // Hay: each cart or pile's meshes merged.
        let mut piles: Vec<(Vec3, Vec3)> = vec![];
        for &(_, lo, hi) in objs.iter().filter(|o| o.0 == CityObject::Hay) {
            match piles.iter_mut().find(|(a, b)| lo.x < b.x + 0.3 && hi.x > a.x - 0.3 && lo.z < b.z + 0.3 && hi.z > a.z - 0.3) {
                Some((a, b)) => {
                    *a = a.min(lo);
                    *b = b.max(hi);
                }
                None => piles.push((lo, hi)),
            }
        }
        for (lo, hi) in piles {
            let size = hi - lo;
            if size.x.max(size.z) > 8.0 || size.x.min(size.z) < 0.8 {
                continue;
            }
            // (The hay meshes come flat out of their data; AC1's carts and piles stand about this high.)
            let height = if size.y < 0.4 { CITY_HAY_HEIGHT } else { size.y };
            // (The skinned hay meshes measure twice a cart's size, 6.2 m, their scale unsure: the stack is its middle.)
            let half = (size.x.min(size.z) * 0.5).min(HAY_HALF_MAX);
            self.haystacks.push(HayStack { centre: Vec3::new((lo.x + hi.x) * 0.5, lo.y, (lo.z + hi.z) * 0.5), half, height });
            hay += 1;
        }
        for &(kind, lo, hi) in objs.iter().filter(|o| o.0 != CityObject::Hay) {
            let size = hi - lo;
            let mid = (lo + hi) * 0.5;
            // The long horizontal axis, and across it.
            let (along, across) = if size.x >= size.z { (Vec3::X, Vec3::Z) } else { (Vec3::Z, Vec3::X) };
            let wall_side =
                |at: Vec3| -> Option<Vec3> { [across, -across].into_iter().find(|d| self.raycast(at, *d, 0.8).is_some_and(|h| h.normal.y.abs() < 0.3)) };
            match kind {
                // (Authored ladders, when the city has them, replace the ones guessed from meshes.)
                CityObject::Ladder if self.authored_ladders => {}
                CityObject::Ladder => {
                    // Thin across, tall; its wall on one side.
                    if size.y < 2.0 {
                        continue;
                    }
                    let thin = if size.x <= size.z { Vec3::X } else { Vec3::Z };
                    // The building is the side with a roof just under its top (it leans on that wall; the rails
                    // stand up past the roof edge); else the nearer wall (in an alley both sides have one).
                    let roof = |d: Vec3| self.ground(mid.with_y(hi.y + 0.3) + d * 0.8, 0.0, 1.6).filter(|g| g.normal.y > 0.7).map(|g| g.point.y);
                    let by_roof = [thin, -thin].into_iter().filter_map(|d| roof(d).map(|y| (d, y))).max_by(|a, b| a.1.total_cmp(&b.1));
                    let by_wall = || {
                        [thin, -thin]
                            .into_iter()
                            .filter_map(|d| self.raycast(mid.with_y(lo.y + 1.2), d, 1.0).filter(|h| h.normal.y.abs() < 0.3).map(|h| (d, h.dist)))
                            .min_by(|a, b| a.1.total_cmp(&b.1))
                            .map(|(d, _)| (d, hi.y))
                    };
                    let Some((to_wall, top)) = by_roof.or_else(by_wall) else { continue };
                    let out = -to_wall;
                    // The foot: toward the outer side of its bounds (a leaning ladder's foot stands off the wall).
                    let off = (size.dot(thin) * 0.5 - 0.25).max(0.0);
                    self.ladders.push(Ladder { base: (mid + out * off).with_y(lo.y), top, out });
                    ladders += 1;
                }
                CityObject::Bench => {
                    if size.y > 1.2 || size.x.max(size.z) < 1.0 {
                        continue;
                    }
                    let out = wall_side(mid.with_y(lo.y + 0.6)).map_or(across, |w| -w);
                    let half = along * (size.dot(along) * 0.5 - 0.2);
                    debug!("bench at {:.2} to {:.2} (bounds {lo:.2} to {hi:.2})", mid.with_y(lo.y) - half, mid.with_y(lo.y) + half);
                    self.benches.push((mid.with_y(lo.y) - half, mid.with_y(lo.y) + half, out));
                    benches += 1;
                }
                CityObject::Pole => {
                    // A horizontal pole (stuck out of a wall, or across a gap): a swing bar along it.
                    if size.x.max(size.z) < 1.0 || size.y > size.x.max(size.z) * 0.5 {
                        continue;
                    }
                    let half = along * (size.dot(along) * 0.5);
                    // (Not again where an authored pole already is.)
                    if !self.bars.iter().any(|b| b.closest(mid).distance(mid) < 0.3) {
                        self.bars.push(Line { a: mid - half, b: mid + half });
                        bars += 1;
                    }
                }
                CityObject::Hay => {}
            }
        }
        (hay, ladders, benches, bars)
    }

    /// Use AC1's own climbing markup where the city has it: its ledge grabs become the holds (instead of the edges and
    /// lips found in the geometry), its ladders the ladders, its horizontal poles the swing bars (each pole's two side
    /// edges as one bar). `AC1_GEOMETRY_HOLDS` keeps the geometry's holds. Returns (holds, ladders, bars), or None.
    pub fn use_authored(&mut self) -> Option<(usize, usize, usize)> {
        if self.authored.is_empty() || std::env::var("AC1_GEOMETRY_HOLDS").is_ok() {
            return None;
        }
        use forge::guidance::SubType;
        let mut ledges = vec![];
        let (mut ladders, mut bars) = (0, 0);
        // How many ledge pieces end at each vertex (to the millimetre): a piece goes on into another there.
        let key = |p: Vec3| ((p.x * 1000.0).round() as i64, (p.y * 1000.0).round() as i64, (p.z * 1000.0).round() as i64);
        let mut ends: std::collections::HashMap<(i64, i64, i64), u32> = std::collections::HashMap::new();
        for e in self.authored.iter().filter(|e| e.kind == SubType::LedgeGrab) {
            *ends.entry(key(e.a)).or_default() += 1;
            *ends.entry(key(e.b)).or_default() += 1;
        }
        for e in std::mem::take(&mut self.authored) {
            let len = e.a.distance(e.b);
            match e.kind {
                SubType::LedgeGrab => {
                    // (Shortened at its free ends, as AC1's hold grid takes an edge: the lone 8-12 cm window-frame
                    // pieces leave nothing, where leaping to one hung him from the air. Not where it goes on into
                    // another piece: a sill in three pieces of 0.15-0.23 m is one 0.87 m hold.)
                    let free = |p: Vec3| if ends.get(&key(p)).copied().unwrap_or(0) <= 1 { AUTHORED_TRIM } else { 0.0 };
                    let (ta, tb) = (free(e.a), free(e.b));
                    if len > ta + tb + 0.01 && (e.b.y - e.a.y).abs() / len <= AUTHORED_MAX_SLOPE && e.out != Vec3::ZERO {
                        let d = (e.b - e.a) / len;
                        ledges.push(Ledge { a: e.a + d * ta, b: e.b - d * tb, out: e.out });
                    }
                }
                SubType::Ladder => {
                    let (lo, hi) = if e.a.y < e.b.y { (e.a, e.b) } else { (e.b, e.a) };
                    if e.out != Vec3::ZERO {
                        self.ladders.push(Ladder { base: lo, top: hi.y, out: e.out });
                        ladders += 1;
                    }
                }
                SubType::Pole if (e.b.y - e.a.y).abs() < 0.3 * len.max(0.01) => {
                    let mid = (e.a + e.b) * 0.5;
                    if !self.bars.iter().any(|b| b.closest(mid).distance(mid) < 0.2) {
                        self.bars.push(Line { a: e.a, b: e.b });
                        bars += 1;
                    }
                }
                _ => {}
            }
        }
        let holds = ledges.len();
        self.ledges = ledges;
        self.authored_ladders = ladders > 0;
        Some((holds, ladders, bars))
    }

    /// Leave out the authored holds that can't be reached from in front: AC1's markup has edges of shapes buried in a
    /// wall (in Damascus a box's top 0.3 m behind a bare wall's face, grabbed through it). A hold stays where, at a
    /// third of the points along it, the way in from 0.45 m out is open. Needs the grid built.
    pub fn drop_buried_holds(&mut self) -> usize {
        let before = self.ledges.len();
        let ledges = std::mem::take(&mut self.ledges);
        let open = |l: &Ledge, t: f32| {
            let p = l.a.lerp(l.b, t) + Vec3::Y * 0.05;
            self.raycast(p + l.out * 0.45, -l.out, 0.42).is_none()
        };
        let kept: Vec<Ledge> = ledges.into_iter().filter(|l| [0.2, 0.5, 0.8].iter().filter(|&&t| open(l, t)).count() >= 1).collect();
        self.ledges = kept;
        before - self.ledges.len()
    }

    pub fn add_beam_perches(&mut self) {
        // (Not where the beam runs inside a wall it is stuck into, even if its top shows there.)
        let open = |p: Vec3| self.ground(p + Vec3::Y * 1.9, 0.0, 1.98).is_some_and(|h| (h.point.y - p.y).abs() < 0.08) && !self.inside_solid(p + Vec3::Y * 0.5);
        let mut found = vec![];
        let mut holds = vec![];
        for l in &self.beam_tops {
            let len = (l.b - l.a).length();
            if len < 0.2 || (l.b.y - l.a.y).abs() > 0.15 * len {
                continue;
            }
            let n = (len / 0.1).ceil() as usize;
            let mut run: Option<(Vec3, Vec3)> = None;
            for k in 0..=n {
                let p = l.a.lerp(l.b, k as f32 / n as f32);
                if open(p) {
                    run = Some(run.map_or((p, p), |(s, _)| (s, p)));
                } else if let Some((s, e)) = run.take() {
                    found.push(Line { a: s, b: e });
                }
            }
            if let Some((s, e)) = run {
                found.push(Line { a: s, b: e });
            }
            // A stub sticking out of a wall (open only at one end): a hand hold across its outer end.
            if let Some(stub) = found.last().filter(|r| (r.b - r.a).length() < STUB_MAX) {
                let axis = (l.b - l.a).with_y(0.0).normalize_or_zero();
                for (end, out) in [(l.a, -axis), (l.b, axis)] {
                    let free = (stub.a - end).length() < 0.15 || (stub.b - end).length() < 0.15;
                    let walled = self.raycast(end + Vec3::Y * -0.05, -out, (l.b - l.a).length() + 0.3).is_some();
                    if free && walled {
                        let side = out.cross(Vec3::Y) * 0.12;
                        holds.push(Ledge { a: end - side, b: end + side, out });
                    }
                }
            }
        }
        info!("{} beam perches and {} beam-end holds from {} beams", found.len(), holds.len(), self.beam_tops.len());
        self.perches.extend(found);
        self.ledges.extend(holds);
    }

    /// Perch under `p`: its index and the point on its top line, when `p` is within `r` of the line
    /// sideways and 0.3 m of it vertically.
    pub fn perch_at(&self, p: Vec3, r: f32) -> Option<(usize, Vec3)> {
        self.perches
            .iter()
            .enumerate()
            .map(|(i, l)| (i, l.closest(p)))
            .filter(|(_, q)| (q - p).with_y(0.0).length() < r && (q.y - p.y).abs() < 0.3)
            .min_by(|a, b| (a.1 - p).length().total_cmp(&(b.1 - p).length()))
    }

    /// `perch_at`, but not near a beam's ends (where it meets the ground it bridges).
    pub fn perch_inside(&self, p: Vec3, r: f32) -> Option<(usize, Vec3)> {
        self.perch_at(p, r).filter(|(i, q)| {
            let l = &self.perches[*i];
            l.axis() == Vec3::ZERO || ((*q - l.a).length() > 0.3 && (*q - l.b).length() > 0.3)
        })
    }

    /// Ground under `p`, searching from `up` above it down to `down` below.
    pub fn ground(&self, p: Vec3, up: f32, down: f32) -> Option<Hit> {
        self.raycast(p + Vec3::Y * up, Vec3::NEG_Y, up + down)
    }

    /// The triangles that may touch the box `lo`..`hi` (each once).
    fn tris_near(&self, lo: Vec3, hi: Vec3) -> Vec<u32> {
        if self.grid.is_empty() {
            return (0..self.tris.len() as u32).collect();
        }
        let mut out = self.big.clone();
        for x in cell(lo.x)..=cell(hi.x) {
            for z in cell(lo.z)..=cell(hi.z) {
                out.extend(self.grid.get(&(x, z)).into_iter().flatten());
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// How far (horizontally) to move the feet at `feet` so the body's capsule (`BODY_RADIUS`, `BODY_HEIGHT`,
    /// lifted `BODY_LIFT` off the feet) touches no wall. Floors and ceilings are the ground follow's; a contact
    /// below the bottom sphere's centre within 45 degrees of straight up is a step, walked up and not pushed against
    /// (AC1's proxy slides up those: a 0.45 m step is climbed, a 0.6 m block is a wall).
    pub fn capsule_push(&self, feet: Vec3) -> Vec3 {
        let r = BODY_RADIUS;
        let reach = Vec3::new(r, 0.0, r) + Vec3::splat(0.6);
        let near = self.tris_near(feet - reach, feet + reach + Vec3::Y * BODY_HEIGHT);
        let mut push = Vec3::ZERO;
        for _ in 0..4 {
            let a = feet + push + Vec3::Y * (BODY_LIFT + r);
            let b = feet + push + Vec3::Y * (BODY_HEIGHT - r);
            let mut deepest: Option<(f32, Vec3)> = None;
            for &i in &near {
                let t = &self.tris[i as usize];
                let n = (t.b - t.a).cross(t.c - t.a).normalize_or_zero();
                if n.y.abs() > WALL_MAX_UP {
                    continue;
                }
                let (p, q) = segment_triangle(a, b, t);
                let d = p - q;
                let dist = d.length();
                if dist >= r {
                    continue;
                }
                // Touching (or crossing) the face itself: out along its normal, toward the segment's side.
                let dir = if dist > 1e-4 {
                    d / dist
                } else if n.dot((a + b) * 0.5 - t.a) >= 0.0 {
                    n
                } else {
                    -n
                };
                // (Judged by the contact on first touch, at the radius: by the height of what it touches.)
                if q.y < a.y - r * std::f32::consts::FRAC_1_SQRT_2 && dir.y > 0.0 {
                    continue;
                }
                let flat = dir.with_y(0.0);
                if flat.length() < 0.2 {
                    continue;
                }
                let need = (r - dist) / flat.length();
                if deepest.is_none_or(|(k, _)| need > k) {
                    deepest = Some((need, flat.normalize()));
                }
            }
            let Some((need, out)) = deepest else { break };
            push += out * (need + 1e-3);
        }
        push
    }

    /// A ceiling within `extra` over the head of a body standing at `feet` (a face looking down).
    pub fn ceiling(&self, feet: Vec3, extra: f32) -> bool {
        self.raycast(feet + Vec3::Y * 1.0, Vec3::Y, BODY_HEIGHT - 1.0 + extra).is_some_and(|h| h.normal.y < -0.5)
    }

    /// Is `p` inside a solid? Three of four level rays from it meet faces from behind (AC1's collision shapes are
    /// closed and wound outward; a beam or ledge piece can stick into a building's wall, its top inside it).
    pub fn inside_solid(&self, p: Vec3) -> bool {
        [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z].iter().filter(|d| self.raycast_sided(p, **d, INSIDE_PROBE).is_some_and(|(_, behind)| behind)).count() >= 3
    }

    /// Narrow wall tops (a fence's or a parapet's, no deeper than `NARROW_TOP_MAX` behind a hold, with a drop past
    /// it and open above) become perches along their middle: climbing onto one ends balancing on it, as on a beam,
    /// not standing on a top too narrow to stand on. Holds next to each other along a top make one perch.
    pub fn add_narrow_tops(&mut self) -> usize {
        let key = |p: Vec3| ((p.x / 0.5).floor() as i32, (p.y / 0.5).floor() as i32, (p.z / 0.5).floor() as i32);
        let near = |cells: &HashMap<(i32, i32, i32), Vec<usize>>, p: Vec3| -> Vec<usize> {
            let (x, y, z) = key(p);
            let mut out = vec![];
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        out.extend(cells.get(&(x + dx, y + dy, z + dz)).into_iter().flatten().copied());
                    }
                }
            }
            out
        };
        let mut found: Vec<Line> = vec![];
        let mut centres: HashMap<(i32, i32, i32), Vec<usize>> = HashMap::new();
        for l in &self.ledges {
            let mid = (l.a + l.b) * 0.5;
            if l.a.distance(l.b) < 0.3 {
                continue;
            }
            let level_with = |d: f32| self.ground(mid - l.out * d + Vec3::Y * 0.3, 0.0, 0.45).is_some_and(|g| (g.point.y - mid.y).abs() < 0.08);
            let Some(depth) = (1..=(NARROW_TOP_MAX / 0.05) as usize + 1).map(|k| k as f32 * 0.05).find(|&d| !level_with(d)) else { continue };
            // (Past it a drop: not the step up to a roof.)
            let drops = self.ground(mid - l.out * (depth + 0.1) + Vec3::Y * 0.3, 0.0, 0.8).is_none();
            if !(0.15..=NARROW_TOP_MAX).contains(&depth) || !drops {
                continue;
            }
            let shift = -l.out * (depth * 0.5);
            let line = Line { a: l.a + shift, b: l.b + shift };
            let centre = (line.a + line.b) * 0.5;
            // (The hold on the far side of the same top finds it again.)
            if near(&centres, centre).iter().any(|&i| found[i].closest(centre).distance(centre) < 0.2) {
                continue;
            }
            let open = self.ground(centre + Vec3::Y * 1.9, 0.0, 1.98).is_some_and(|h| (h.point.y - centre.y).abs() < 0.08);
            if !open || self.inside_solid(centre + Vec3::Y * 0.5) {
                continue;
            }
            centres.entry(key(centre)).or_default().push(found.len());
            found.push(line);
        }
        // Join the pieces end to end along a top (each piece onto the line ending where it starts).
        let mut ends: HashMap<(i32, i32, i32), Vec<usize>> = HashMap::new();
        let mut joined: Vec<Line> = vec![];
        for l in found {
            let axis = l.axis();
            let hit = near(&ends, l.a).into_iter().chain(near(&ends, l.b)).find(|&j| {
                let k = joined[j];
                k.axis().dot(axis).abs() > 0.97 && (k.b.distance(l.a) < 0.15 || k.b.distance(l.b) < 0.15)
            });
            match hit {
                Some(j) => {
                    let k = &mut joined[j];
                    k.b = if k.b.distance(l.a) < 0.15 { l.b } else { l.a };
                    ends.entry(key(k.b)).or_default().push(j);
                }
                None => {
                    ends.entry(key(l.b)).or_default().push(joined.len());
                    joined.push(l);
                }
            }
        }
        let n = joined.len();
        self.perches.extend(joined);
        n
    }
}

/// Rows of test-level holds along a face: from `origin` along `along` for `len` m, facing `out`, at each height.
fn hold_rows(strips: &mut Builder, level: &mut Level, origin: Vec3, along: Vec3, len: f32, out: Vec3, heights: &[f32]) {
    for &y in heights {
        let a = origin + along * 0.08 + Vec3::Y * y;
        let b = origin + along * (len - 0.08) + Vec3::Y * y;
        strips.cuboid(a.min(b) + out.min(Vec3::ZERO) * HOLD_DEPTH - Vec3::Y * 0.07, a.max(b) + out.max(Vec3::ZERO) * HOLD_DEPTH);
        level.ledges.push(Ledge { a: a + out * 0.06, b: b + out * 0.06, out });
    }
}

/// The parkour gauntlet (north-west, x -100 to -55, z 30 to 130): ten lanes run along +X, each a chain of moves for
/// one scenario (tag `combo` in the scenarios), built from the spacings the course and the rooftops prove: 2 m
/// platforms, posts 2.4 m apart, swing bars 4.4 m up and 2 m out, 2 m roof gaps, AC1's 0.6 m hold rows.
fn gauntlet(walls: &mut Builder, strips: &mut Builder, level: &mut Level) -> Vec<(&'static str, Vec3)> {
    let top = |k: f32| 1.79 + 0.6 * k;
    let ramp = |w: &mut Builder, x0: f32, x1: f32, z: f32, h: f32| {
        w.quad(Vec3::new(x0, 0.0, z - 2.0), Vec3::new(x0, 0.0, z + 2.0), Vec3::new(x1, h, z + 2.0), Vec3::new(x1, h, z - 2.0));
        w.cuboid(Vec3::new(x0, 0.0, z - 2.0), Vec3::new(x1, 0.01, z + 2.0));
    };
    let platform = |w: &mut Builder, x0: f32, x1: f32, z: f32, h: f32| w.cuboid(Vec3::new(x0, 0.0, z - 2.0), Vec3::new(x1, h, z + 2.0));
    // A beam along X with its top at `y`, 0.3 m wide.
    let beam = |w: &mut Builder, level: &mut Level, x0: f32, x1: f32, z: f32, y: f32| {
        w.cuboid(Vec3::new(x0, y - 0.2, z - 0.15), Vec3::new(x1, y, z + 0.15));
        level.perches.push(Line { a: Vec3::new(x0, y, z), b: Vec3::new(x1, y, z) });
    };
    let post = |w: &mut Builder, level: &mut Level, x: f32, z: f32, y: f32| {
        w.cuboid(Vec3::new(x - 0.2, 0.0, z - 0.2), Vec3::new(x + 0.2, y, z + 0.2));
        level.perches.push(Line { a: Vec3::new(x, y, z), b: Vec3::new(x, y, z) });
    };
    // A swing bar across the lane (along Z), its uprights outside the path.
    let bar = |w: &mut Builder, level: &mut Level, x: f32, z: f32, y: f32| {
        for dz in [-1.7, 1.7] {
            w.cuboid(Vec3::new(x - 0.1, 0.0, z + dz - 0.1), Vec3::new(x + 0.1, y + 0.1, z + dz + 0.1));
        }
        level.bars.push(Line { a: Vec3::new(x, y, z - 1.6), b: Vec3::new(x, y, z + 1.6) });
    };
    let up = |x: f32, z: f32| Vec3::new(x, 0.0, z);

    // G1: up a ramp onto box L, then side beams (1.2 m, stuck out of a wall along the lane, AC1's hopping beams): two
    // 2.4 m apart, a swing bar, down onto a third lower down, on to a platform.
    let z = 35.0;
    ramp(walls, -96.0, -92.0, z, 2.0);
    platform(walls, -92.0, -89.0, z, 2.0);
    walls.cuboid(Vec3::new(-89.0, 0.0, z + 0.6), Vec3::new(-74.0, 3.0, z + 1.2));
    let side_beam = |w: &mut Builder, level: &mut Level, x: f32, y: f32| {
        w.cuboid(Vec3::new(x - 0.15, y - 0.2, z - 0.6), Vec3::new(x + 0.15, y, z + 0.6));
        level.perches.push(Line { a: Vec3::new(x, y, z - 0.55), b: Vec3::new(x, y, z + 0.55) });
    };
    side_beam(walls, level, -86.6, 2.0);
    side_beam(walls, level, -84.2, 2.0);
    bar(walls, level, -82.2, z - 0.2, 4.4);
    side_beam(walls, level, -79.2, 1.5);
    platform(walls, -77.0, -74.0, z - 0.8, 1.5);

    // G2: a ladder up a 5 m block, a beam across to a second block, its beam out over a haystack: the leap of faith.
    let z = 44.0;
    walls.cuboid(Vec3::new(-90.0, 0.0, z - 1.5), Vec3::new(-87.0, 5.0, z + 1.5));
    level.ladders.push(Ladder { base: Vec3::new(-90.08, 0.0, z), top: 5.0, out: Vec3::NEG_X });
    beam(walls, level, -87.0, -83.0, z, 5.0);
    walls.cuboid(Vec3::new(-83.0, 0.0, z - 1.5), Vec3::new(-79.0, 5.0, z + 1.5));
    beam(walls, level, -79.0, -77.6, z, 5.0);
    level.haystacks.push(HayStack { centre: Vec3::new(-76.2, 0.0, z), half: 1.3, height: 1.7 });

    // G3: a wall with holds to run up (its west face) and climb over, across its roof, a 2 m gap to the next roof, off
    // its far end to the ground.
    let z = 55.0;
    building(walls, strips, level, up(-90.0, z - 4.0), Vec3::new(-86.0, top(5.0), z + 4.0), &[Vec3::NEG_X]);
    building(walls, strips, level, up(-84.0, z - 4.0), Vec3::new(-80.0, top(5.0), z + 4.0), &[]);

    // G4: step up 0.6, jump onto a 1.2 m wall, up a ramp, a 2.5 m gap, down 1 m across a gap, down to the ground, over
    // a 1.4 m passover wall.
    let z = 64.0;
    platform(walls, -94.0, -92.8, z, 0.6);
    platform(walls, -89.0, -88.7, z, 1.2);
    ramp(walls, -86.0, -82.0, z, 2.0);
    platform(walls, -82.0, -78.0, z, 2.0);
    platform(walls, -75.5, -71.0, z, 2.0);
    platform(walls, -69.0, -64.0, z, 1.0);
    walls.cuboid(Vec3::new(-59.0, 0.0, z - 2.0), Vec3::new(-58.7, 1.4, z + 2.0));

    // G5: from box L, four posts zigzagging a metre off the line, a beam, a 2 m box at its end with a haystack 0.4 m
    // past it to dive into (AC1 dives into hay from a top beside it; it has no dive off a beam).
    let z = 74.0;
    ramp(walls, -96.0, -92.0, z, 2.0);
    platform(walls, -92.0, -89.0, z, 2.0);
    for (x, dz) in [(-86.6, 0.0), (-84.4, 1.0), (-82.2, 0.0), (-80.0, 1.0)] {
        post(walls, level, x, z + dz, 2.0);
    }
    beam(walls, level, -77.8, -73.8, z, 2.0);
    platform(walls, -73.8, -71.8, z, 2.0);
    level.haystacks.push(HayStack { centre: Vec3::new(-70.1, 0.0, z), half: 1.3, height: 1.7 });

    // G6: two swing bars off a platform, down onto a post, post to post, onto a beam, along it to a platform.
    let z = 84.0;
    ramp(walls, -96.0, -92.0, z, 2.0);
    platform(walls, -92.0, -89.0, z, 2.0);
    bar(walls, level, -87.0, z, 4.4);
    bar(walls, level, -83.6, z, 4.4);
    post(walls, level, -80.5, z, 2.0);
    post(walls, level, -78.1, z, 2.0);
    beam(walls, level, -75.7, -71.7, z, 2.0);
    platform(walls, -71.7, -68.0, z, 2.0);

    // G7: climb a wall's holds (its west face), leap right across a 1.2 m gap onto the next wall's holds, up to its top.
    let z = 94.0;
    building(walls, strips, level, up(-92.0, z - 3.0), Vec3::new(-88.0, top(5.0), z + 1.0), &[Vec3::NEG_X]);
    building(walls, strips, level, up(-92.0, z + 2.2), Vec3::new(-88.0, top(5.0), z + 6.0), &[Vec3::NEG_X]);

    // G8: on a 5.39 m roof (AC1's ledge stop wants a drop over 5 m): the ledge stop at its east edge, the pull down into
    // the hang, down its holds to the ground, on to a 1.2 m wall to jump onto.
    let z = 104.0;
    building(walls, strips, level, up(-96.0, z - 3.0), Vec3::new(-90.0, top(6.0), z + 3.0), &[Vec3::X]);
    platform(walls, -84.0, -83.7, z, 1.2);

    // G9: a 1.4 m passover wall, a wall with holds to run up and over, across its roof, a jump down onto a post.
    let z = 114.0;
    walls.cuboid(Vec3::new(-92.0, 0.0, z - 2.0), Vec3::new(-91.7, 1.4, z + 2.0));
    building(walls, strips, level, up(-86.0, z - 3.0), Vec3::new(-82.0, top(3.0), z + 3.0), &[Vec3::NEG_X]);
    post(walls, level, -79.0, z, 2.0);

    // G10: a swing bar off a platform flung at a wall's holds 3 m on, up them, across its roof, a jump down onto a lower
    // roof, off its end to the ground.
    let z = 124.0;
    ramp(walls, -96.0, -92.0, z, 2.0);
    platform(walls, -92.0, -89.0, z, 2.0);
    bar(walls, level, -87.0, z, 4.4);
    building(walls, strips, level, up(-84.0, z - 3.0), Vec3::new(-80.0, top(4.0), z + 3.0), &[Vec3::NEG_X]);
    building(walls, strips, level, up(-78.0, z - 3.0), Vec3::new(-74.0, top(2.0), z + 3.0), &[]);

    // A tall wall with holds up its west face and a lower roof flush with it to its right: climbed level with that roof
    // and on sideways, the turned step-off onto it (AC1's `groundentry_<side>_90`: the roof's edge faces out as the wall).
    let z = 134.0;
    building(walls, strips, level, up(-92.0, z - 4.0), Vec3::new(-88.0, top(4.0), z), &[Vec3::NEG_X]);
    walls.cuboid(Vec3::new(-92.0, 0.0, z), Vec3::new(-88.0, 1.8, z + 4.0));

    // A slab sticking out 0.6 m over a wall with holds 2.4 m and more below it: hanging from its edge the feet find no
    // wall (a free hang), and down drops onto the wall's holds (AC1's `hangfree_tr_climb2m_down_*` / `hangfree_tr_hangwall_down_*`).
    let z = 144.0;
    walls.cuboid(Vec3::new(-92.0, 0.0, z - 2.0), Vec3::new(-88.0, top(6.0), z + 2.0));
    hold_rows(strips, level, Vec3::new(-92.0, 0.0, z - 2.0), Vec3::Z, 4.0, Vec3::NEG_X, &[top(0.0), top(1.0), top(2.0)]);
    walls.cuboid(Vec3::new(-92.6, top(6.0) - 0.2, z - 2.0), Vec3::new(-92.0, top(6.0), z + 2.0));
    hold_rows(strips, level, Vec3::new(-92.6, 0.0, z - 2.0), Vec3::Z, 4.0, Vec3::NEG_X, &[top(6.0)]);

    vec![
        ("G1: box L, side beams, swing bar, side beam", Vec3::new(-90.0, 3.5, 35.0)),
        ("G2: ladder, beam bridge, leap of faith", Vec3::new(-85.0, 6.0, 44.0)),
        ("G3: wall run up, over, roof gap, drop", Vec3::new(-88.0, 6.5, 55.0)),
        ("G4: step, jump onto, gaps, passover", Vec3::new(-80.0, 3.0, 64.0)),
        ("G5: zigzag posts, beam, hay", Vec3::new(-83.0, 3.0, 74.0)),
        ("G6: two bars, posts, beam", Vec3::new(-85.0, 5.4, 84.0)),
        ("G7: climb, leap across, climb on", Vec3::new(-90.0, 6.5, 96.0)),
        ("G8: ledge stop, pull down, climb down, jump onto", Vec3::new(-93.0, 6.5, 104.0)),
        ("G9: passover, wall run up, roof, post", Vec3::new(-84.0, 5.5, 114.0)),
        ("G10: bar to a wall, climb, roof, down a roof", Vec3::new(-82.0, 5.5, 124.0)),
    ]
}

/// A test-level building from `lo` to `hi` (ground to roof): a hold along the roof's edge on every side, and on the
/// faces listed (by outward normal) rows of holds every 0.6 m from 1.79 m up to it, to climb.
fn building(walls: &mut Builder, strips: &mut Builder, level: &mut Level, lo: Vec3, hi: Vec3, climb: &[Vec3]) {
    walls.cuboid(lo, hi);
    let rows: Vec<f32> = (0..).map(|k| 1.79 + 0.6 * k as f32).take_while(|y| *y < hi.y - 0.3).collect();
    let faces = [
        (Vec3::new(lo.x, 0.0, lo.z), Vec3::X, hi.x - lo.x, Vec3::NEG_Z),
        (Vec3::new(lo.x, 0.0, hi.z), Vec3::X, hi.x - lo.x, Vec3::Z),
        (Vec3::new(lo.x, 0.0, lo.z), Vec3::Z, hi.z - lo.z, Vec3::NEG_X),
        (Vec3::new(hi.x, 0.0, lo.z), Vec3::Z, hi.z - lo.z, Vec3::X),
    ];
    for (origin, along, len, out) in faces {
        hold_rows(strips, level, origin, along, len, out, &[hi.y]);
        if climb.contains(&out) {
            hold_rows(strips, level, origin, along, len, out, &rows);
        }
    }
}

/// Rooftops (north-east of the start, x 40-70, z 8-36), to free run like a city's roofs: B1 to climb (holds up its
/// south and west faces), a 2 m gap to B2 at the same height, a 2.5 m gap down 1.5 m to B3 (a ladder up its south
/// face), a beam across a 3 m alley to B4, a 2.5 m gap up 1.2 m to B5, two swing bars over a 5 m gap down to B6, and
/// from B6 a running jump across 2 m to tower T2's holds; from T2's top a beam over a haystack (a leap of faith).
/// Every roof has a hold along its edges. And wall W (a rebound): a bare 6 m wall to run up and kick off backwards,
/// onto the holds over the doorway of a wall 2.2 m behind, run through on the way in. Returns its labels.
fn rooftops(walls: &mut Builder, strips: &mut Builder, level: &mut Level) -> Vec<(&'static str, Vec3)> {
    let up = |x: f32, z: f32| Vec3::new(x, 0.0, z);
    let top = |k: f32| 1.79 + 0.6 * k;
    let (b1, b2, b3) = (top(7.0), top(7.0), top(4.5));
    building(walls, strips, level, up(40.0, 30.0), Vec3::new(46.0, b1, 36.0), &[Vec3::NEG_Z, Vec3::NEG_X]);
    building(walls, strips, level, up(48.0, 30.0), Vec3::new(53.0, b2, 36.0), &[]);
    building(walls, strips, level, up(55.5, 30.0), Vec3::new(61.0, b3, 36.0), &[]);
    level.ladders.push(Ladder { base: Vec3::new(58.0, 0.0, 29.92), top: b3, out: Vec3::NEG_Z });
    // The beam across the alley to B4, its top level with the roofs.
    let b4 = b3;
    building(walls, strips, level, up(64.0, 30.0), Vec3::new(69.0, b4, 36.0), &[]);
    walls.cuboid(Vec3::new(61.0, b3 - 0.25, 32.85), Vec3::new(64.0, b3, 33.15));
    level.perches.push(Line { a: Vec3::new(61.0, b3, 33.0), b: Vec3::new(64.0, b3, 33.0) });
    let b5 = b4 + 1.2;
    building(walls, strips, level, up(64.0, 22.0), Vec3::new(69.0, b5, 27.5), &[]);
    // Swing bars 2 m and 5.4 m out from B5's edge, 2.4 m over its roof; B6 lower, 3.1 m past the second.
    let bar_y = b5 + 2.4;
    for z in [20.0, 16.6] {
        for x in [63.3, 69.7] {
            walls.cuboid(Vec3::new(x - 0.1, 0.0, z - 0.1), Vec3::new(x + 0.1, bar_y + 0.1, z + 0.1));
        }
        level.bars.push(Line { a: Vec3::new(63.4, bar_y, z), b: Vec3::new(69.6, bar_y, z) });
    }
    let b6 = b5 - 1.0;
    building(walls, strips, level, up(64.0, 8.0), Vec3::new(69.0, b6, 13.5), &[]);
    // Tower T2, 2 m west of B6, its east face climbable from the ground to the top.
    let t2 = top(18.0);
    building(walls, strips, level, up(58.0, 8.0), Vec3::new(62.0, t2, 12.0), &[Vec3::X]);
    walls.cuboid(Vec3::new(56.6, t2 - 0.25, 9.85), Vec3::new(58.0, t2, 10.15));
    level.perches.push(Line { a: Vec3::new(56.7, t2, 10.0), b: Vec3::new(58.0, t2, 10.0) });
    level.haystacks.push(HayStack { centre: Vec3::new(55.4, 0.0, 10.0), half: 1.3, height: 1.7 });
    // Wall W: run through the doorway (1.6 m wide, 2.4 m high) in the wall in front of it, up the bare wall (its face
    // at z = 22) and rebound back onto the holds over the doorway, 2.2 m behind (face at z = 19.8, at 2.99 and 3.59 m).
    walls.cuboid(Vec3::new(29.0, 0.0, 22.0), Vec3::new(37.0, 6.0, 22.6));
    let lintel = top(3.0);
    walls.cuboid(Vec3::new(30.5, 0.0, 19.2), Vec3::new(32.2, lintel, 19.8));
    walls.cuboid(Vec3::new(33.8, 0.0, 19.2), Vec3::new(35.5, lintel, 19.8));
    walls.cuboid(Vec3::new(32.2, 2.4, 19.2), Vec3::new(33.8, lintel, 19.8));
    hold_rows(strips, level, up(30.5, 19.8), Vec3::X, 5.0, Vec3::Z, &[top(2.0), lintel]);
    // Side grab (south, x -6 to 8, z -52 to -49.6): a 6 m wall with holds up its north face, and beside it a 1.79 m ledge
    // ending at x 1, 0.2 m out from the wall: running off its end, the wall goes on beside the fall, to be grabbed by
    // pushing the stick toward it.
    building(walls, strips, level, up(0.0, -52.5), Vec3::new(8.0, 6.0, -51.5), &[Vec3::Z]);
    walls.cuboid(Vec3::new(-6.0, 0.0, -51.3), Vec3::new(1.0, top(0.0), -49.6));
    // Passover walls (south-west, x -20 to -10, faces at z = -40): too high to jump onto, thin enough to go over with a
    // hand on the top (AC1's passover), flat ground both sides: 1.4 m high and 0.3 m deep, and 1.5 m high and 1 m deep.
    walls.cuboid(Vec3::new(-20.0, 0.0, -40.3), Vec3::new(-16.0, 1.4, -40.0));
    walls.cuboid(Vec3::new(-14.0, 0.0, -41.0), Vec3::new(-10.0, 1.5, -40.0));
    // A passover over a big drop (west of them, x -30 to -24): a 3 m platform, a 1.4 m wall 0.3 m deep along its south
    // edge, 4.4 m down beyond it, a hold along the wall's far top edge: over it and round into the hang on the far side.
    walls.cuboid(Vec3::new(-30.0, 0.0, -44.0), Vec3::new(-24.0, 3.0, -34.0));
    walls.cuboid(Vec3::new(-30.0, 3.0, -44.3), Vec3::new(-24.0, 4.4, -44.0));
    level.ledges.push(Ledge { a: Vec3::new(-29.9, 4.4, -44.36), b: Vec3::new(-24.1, 4.4, -44.36), out: Vec3::NEG_Z });
    vec![
        ("Rooftops: B1 (climb its south or west face)", Vec3::new(43.0, b1 + 0.6, 33.0)),
        ("B2 (2 m gap)", Vec3::new(50.5, b2 + 0.6, 33.0)),
        ("B3 (gap down, ladder)", Vec3::new(58.2, b3 + 0.6, 33.0)),
        ("Beam to B4", Vec3::new(62.5, b3 + 0.6, 33.0)),
        ("B5 (gap up 1.2 m)", Vec3::new(66.5, b5 + 0.6, 24.7)),
        ("Swing bars to B6", Vec3::new(66.5, bar_y + 0.6, 18.3)),
        ("Tower T2 (jump to its holds; leap of faith)", Vec3::new(60.0, t2 + 0.6, 10.0)),
        ("Wall W (run through the doorway, up the wall, legs: rebound onto the holds over the door)", Vec3::new(33.0, 6.6, 21.0)),
        ("Side grab (run off the ledge's end, stick toward the wall: grab it in the air)", Vec3::new(1.0, 6.8, -50.5)),
        ("Passover walls (jump over: 0.3 m, 1 m deep)", Vec3::new(-15.0, 2.2, -40.0)),
        ("Passover over a drop (into the far side's hang)", Vec3::new(-27.0, 3.6, -38.0)),
    ]
}

struct Builder {
    tris: Vec<Tri>,
}

impl Builder {
    fn quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
        self.tris.push(Tri { a, b, c });
        self.tris.push(Tri { a, b: c, c: d });
    }
    fn cuboid(&mut self, min: Vec3, max: Vec3) {
        let p = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
        let (a, b) = (min, max);
        self.quad(p(a.x, b.y, a.z), p(a.x, b.y, b.z), p(b.x, b.y, b.z), p(b.x, b.y, a.z)); // top
        self.quad(p(a.x, a.y, a.z), p(b.x, a.y, a.z), p(b.x, a.y, b.z), p(a.x, a.y, b.z)); // bottom
        self.quad(p(a.x, a.y, b.z), p(b.x, a.y, b.z), p(b.x, b.y, b.z), p(a.x, b.y, b.z)); // +z
        self.quad(p(a.x, a.y, a.z), p(a.x, b.y, a.z), p(b.x, b.y, a.z), p(b.x, a.y, a.z)); // -z
        self.quad(p(b.x, a.y, a.z), p(b.x, b.y, a.z), p(b.x, b.y, b.z), p(b.x, a.y, b.z)); // +x
        self.quad(p(a.x, a.y, a.z), p(a.x, a.y, b.z), p(a.x, b.y, b.z), p(a.x, b.y, a.z)); // -x
    }
    fn heightfield(&mut self, min: Vec2, max: Vec2, n: usize, h: impl Fn(f32, f32) -> f32) {
        let at = |i: usize, j: usize| {
            let x = min.x + (max.x - min.x) * i as f32 / n as f32;
            let z = min.y + (max.y - min.y) * j as f32 / n as f32;
            Vec3::new(x, h(x, z), z)
        };
        for i in 0..n {
            for j in 0..n {
                self.quad(at(i, j), at(i, j + 1), at(i + 1, j + 1), at(i + 1, j));
            }
        }
    }
    fn mesh(self) -> (Mesh, Vec<Tri>) {
        let mut pos = Vec::with_capacity(self.tris.len() * 3);
        let mut nrm = Vec::with_capacity(self.tris.len() * 3);
        let mut uv = Vec::with_capacity(self.tris.len() * 3);
        for t in &self.tris {
            let mut n = (t.b - t.a).cross(t.c - t.a).normalize_or_zero();
            // Builder winds faces inconsistently; make them face up/outward for lighting.
            let centre = (t.a + t.b + t.c) / 3.0;
            if n.y < -0.5 && centre.y > 0.05 {
                n = -n;
            }
            for v in [t.a, t.b, t.c] {
                pos.push(v.to_array());
                nrm.push(n.to_array());
                uv.push([v.x + v.y, v.z + v.y]);
            }
        }
        let count = pos.len() as u32;
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nrm);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
        // Two-sided by duplicating indices in both windings.
        let mut idx: Vec<u32> = (0..count).collect();
        idx.extend((0..count / 3).flat_map(|t| [t * 3, t * 3 + 2, t * 3 + 1]));
        mesh.insert_indices(Indices::U32(idx));
        (mesh, self.tris)
    }
}

pub fn spawn_level(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    dir: Res<crate::GameDir>,
) {
    let mut level = Level::default();
    if let Ok(city) = std::env::var("AC1_LEVEL") {
        let forge = crate::city::forge_of(&city);
        let centre = match crate::city::spawn_city(&mut commands, &mut meshes, &mut mats, &mut images, &mut level, &dir.0, &forge) {
            Ok(c) => c,
            Err(e) => {
                warn!("no city {city}: {e:#}");
                Vec3::ZERO
            }
        };
        level.city = true;
        // AC1's own holds, ladders and poles where the city has them; else the edges found in its geometry.
        let authored = level.use_authored();
        if let Some((holds, ladders, bars)) = authored {
            info!("authored guidance: {holds} holds, {ladders} ladders, {bars} swing bars");
        }
        level.build_grid();
        if authored.is_some() {
            let buried = level.drop_buried_holds();
            info!("{buried} authored holds left out: inside a wall, no way to them from in front");
        }
        level.add_beam_perches();
        let narrow = level.add_narrow_tops();
        info!("{narrow} narrow wall tops to balance on");
        let probed = if authored.is_none() && std::env::var("AC1_NO_LIPS").is_err() { level.add_probed_ledges() } else { 0 };
        // A viewpoint's entity sits partway up its tower: the spot to stand on is the top above it.
        for i in 0..level.viewpoints.len() {
            let v = level.viewpoints[i];
            if let Some(top) = level.ground(v + Vec3::Y * 80.0, 0.0, 80.0).filter(|g| g.point.y > v.y) {
                level.viewpoints[i] = top.point;
            }
        }
        let (hay, ladders, benches, bars) = level.add_city_objects();
        info!("{hay} haystacks, {ladders} ladders, {benches} benches, {bars} swing bars");
        for v in &level.viewpoints {
            let near = level.haystacks.iter().map(|h| ((h.centre - *v).with_y(0.0).length(), v.y - h.top())).min_by(|a, b| a.0.total_cmp(&b.0));
            debug!("viewpoint {v:.1}: nearest hay {near:.1?} (out, down)");
        }
        level.build_grid();
        info!("{probed} more holds where walkable tops end over a wall (lips the edge test missed)");
        // `AC1_HOLDS_AT="x,y,z,r"`: every hold within r of a point, logged (what a spot in a recording could grab).
        if let Some(v) =
            std::env::var("AC1_HOLDS_AT").ok().map(|s| s.split(',').filter_map(|v| v.trim().parse::<f32>().ok()).collect::<Vec<_>>()).filter(|v| v.len() == 4)
        {
            let p = Vec3::new(v[0], v[1], v[2]);
            for (i, l) in level.ledges.iter().enumerate() {
                let d = Line { a: l.a, b: l.b }.closest(p).distance(p);
                if d < v[3] {
                    info!("hold {i}: {:.2} to {:.2}, out {:.2}, {d:.2} m away", l.a, l.b, l.out);
                }
            }
        }
        // `AC1_GROUND_LINE="x0,z0,x1,z1,y"`: the ground every 10 cm along a line, looking down from y (game Y-up
        // space), logged (where a top ends, to set beside the real game's trace walked along it).
        if let Some(v) = std::env::var("AC1_GROUND_LINE")
            .ok()
            .map(|s| s.split(',').filter_map(|v| v.trim().parse::<f32>().ok()).collect::<Vec<_>>())
            .filter(|v| v.len() == 5)
        {
            let (a, b) = (Vec3::new(v[0], v[4], v[1]), Vec3::new(v[2], v[4], v[3]));
            let n = ((b - a).length() / 0.1).ceil() as usize;
            for k in 0..=n {
                let p = a.lerp(b, k as f32 / n.max(1) as f32);
                let g = level.ground(p, 0.0, 30.0).map(|h| h.point.y);
                info!("ground line [{:.2}, {:.2}]: {}", p.x, p.z, g.map_or("none".into(), |y| format!("{y:.2}")));
            }
        }
        // The player on a street (Masyaf: in the village below the fortress) and a crowd loop nearby where
        // the ground is at the same height.
        let on_ground = |level: &Level, x: f32, z: f32| level.ground(Vec3::new(x, 0.0, z), 400.0, 500.0).map(|h| h.point);
        let home = if city.eq_ignore_ascii_case("masyaf") {
            on_ground(&level, MASYAF_SPAWN[0], MASYAF_SPAWN[2]).unwrap_or(Vec3::ZERO)
        } else {
            crate::city::street_near(&level, centre)
        };
        level.spawn = Some(home);
        // (Not up on a roof or down in a river bed.)
        let level_with = |p: Option<Vec3>| p.filter(|p| (p.y - home.y).abs() < 3.0);
        let corners: Option<Vec<Vec3>> =
            [(-6.0, -4.0), (6.0, -4.0), (6.0, -6.0), (-6.0, -6.0)].iter().map(|&(dx, dz)| level_with(on_ground(&level, home.x + dx, home.z + dz))).collect();
        level.crowd_path = corners;
        info!("{city}: start at {home:.1}, crowd {}", level.crowd_path.is_some());
        commands.insert_resource(level);
        return;
    }
    let mut add = |b: Builder, color: Color, level: &mut Level, commands: &mut Commands| {
        let (mesh, tris) = b.mesh();
        level.tris.extend(tris);
        commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mats.add(StandardMaterial { base_color: color, perceptual_roughness: 0.9, ..default() }))));
    };

    // Ground with a rough patch on the left.
    let mut g = Builder { tris: vec![] };
    g.heightfield(Vec2::new(-20.0, -20.0), Vec2::new(20.0, 20.0), 80, |x, z| {
        let rough = ((-x - 3.0).min(x + 12.0).min(z + 10.0).min(-z - 1.0)).clamp(0.0, 1.0);
        rough * (0.12 * (x * 2.1).sin() * (z * 1.7).cos() + 0.06 * (x * 4.3 + z * 3.1).sin())
    });
    add(g, Color::srgb(0.72, 0.64, 0.5), &mut level, &mut commands);
    // Flat plain all around the test area, so nothing falls off the world.
    let mut plain = Builder { tris: vec![] };
    for (x0, x1, z0, z1) in [(-500.0, 500.0, 20.0, 500.0), (-500.0, 500.0, -500.0, -20.0), (-500.0, -20.0, -20.0, 20.0), (20.0, 500.0, -20.0, 20.0)] {
        plain.quad(Vec3::new(x0, 0.0, z0), Vec3::new(x0, 0.0, z1), Vec3::new(x1, 0.0, z1), Vec3::new(x1, 0.0, z0));
    }
    add(plain, Color::srgb(0.7, 0.62, 0.48), &mut level, &mut commands);

    // Stairs up toward -Z onto a platform.
    let mut s = Builder { tris: vec![] };
    let (step_h, step_d, steps) = (0.17, 0.32, 8);
    for i in 0..steps {
        let z0 = -3.0 - step_d * i as f32;
        s.cuboid(Vec3::new(-1.6, 0.0, z0 - step_d), Vec3::new(1.6, step_h * (i + 1) as f32, z0));
    }
    let top = step_h * steps as f32;
    let zt = -3.0 - step_d * steps as f32;
    s.cuboid(Vec3::new(-1.6, 0.0, zt - 4.0), Vec3::new(1.6, top, zt));
    add(s, Color::srgb(0.8, 0.76, 0.66), &mut level, &mut commands);

    // Ramp on the right, ~18 degrees.
    let mut r = Builder { tris: vec![] };
    let (rx0, rx1, rz0, rz1, rh) = (3.0, 6.0, -2.0, -7.0, 1.6);
    r.quad(Vec3::new(rx0, 0.0, rz0), Vec3::new(rx1, 0.0, rz0), Vec3::new(rx1, rh, rz1), Vec3::new(rx0, rh, rz1));
    r.cuboid(Vec3::new(rx0, 0.0, rz1 - 3.0), Vec3::new(rx1, rh, rz1));
    add(r, Color::srgb(0.66, 0.6, 0.52), &mut level, &mut commands);

    // Climbing blocks behind the start, sized to AC1's climbing grid: the ground entry reaches a hold
    // at 1.79 m and each hand move climbs 0.6 m, so the top edge (also a hold) sits at 5.99 m.
    // Block A faces -Z at z = 5; block B sticks out from its left end, making an inside corner
    // (B's +X face meets A's front) and an outside corner at B's front.
    // Block C, right of A, has an overhang: its right half is cut back 1 m below `overhang`, so a
    // climber moving right along it loses the wall under the feet and free-hangs. A and C stand 1.5 m
    // apart (a leap on the wall, or a running jump between the roofs).
    // C's right side has holds above the overhang: a running jump off D's lower roof catches them.
    // Block D, 1.5 m right of C, is a low building whose right half is a roof slab over a 1 m recess:
    // hanging from its top edge there is a free hang, and the top-out pulls up from it.
    let wall_top = 1.79 + 0.6 * 7.0;
    let overhang = 2.69;
    let mut w = Builder { tris: vec![] };
    w.cuboid(Vec3::new(-3.0, 0.0, 5.0), Vec3::new(3.0, wall_top, 7.0));
    w.cuboid(Vec3::new(-6.0, 0.0, 3.0), Vec3::new(-3.0, wall_top, 7.0));
    w.cuboid(Vec3::new(4.5, overhang, 5.0), Vec3::new(10.5, wall_top, 7.0));
    w.cuboid(Vec3::new(4.5, 0.0, 5.0), Vec3::new(7.5, overhang, 7.0));
    w.cuboid(Vec3::new(7.5, 0.0, 6.0), Vec3::new(10.5, overhang, 7.0));
    let low_top = 1.79 + 0.6 * 3.0;
    w.cuboid(Vec3::new(12.0, 0.0, 5.0), Vec3::new(15.0, low_top, 7.0));
    w.cuboid(Vec3::new(15.0, overhang, 5.0), Vec3::new(18.0, low_top, 7.0));
    w.cuboid(Vec3::new(15.0, 0.0, 6.0), Vec3::new(18.0, overhang, 7.0));
    let mut ledge_geo = Builder { tris: vec![] };
    let rows: Vec<f32> = (0..7).map(|k| 1.79 + 0.6 * k as f32).chain([wall_top]).collect();
    let (low, high): (Vec<f32>, Vec<f32>) = rows.iter().partition(|&&y| y < overhang);
    let (d_low, d_high): (Vec<f32>, Vec<f32>) = rows.iter().filter(|&&y| y <= low_top + 0.01).partition(|&&y| y < overhang);
    // (face point at one end, along-face direction, length, outward normal, hold heights)
    let faces = [
        (Vec3::new(-3.0, 0.0, 5.0), Vec3::X, 6.0, Vec3::NEG_Z, &rows),   // A front
        (Vec3::new(3.0, 0.0, 5.0), Vec3::Z, 2.0, Vec3::X, &rows),        // A right side
        (Vec3::new(-3.0, 0.0, 7.0), Vec3::X, 6.0, Vec3::Z, &rows),       // A back
        (Vec3::new(-3.0, 0.0, 3.0), Vec3::Z, 2.0, Vec3::X, &rows),       // B side, inside corner with A front
        (Vec3::new(-6.0, 0.0, 3.0), Vec3::X, 3.0, Vec3::NEG_Z, &rows),   // B front
        (Vec3::new(-6.0, 0.0, 3.0), Vec3::Z, 4.0, Vec3::NEG_X, &rows),   // B left side
        (Vec3::new(4.5, 0.0, 5.0), Vec3::X, 3.0, Vec3::NEG_Z, &low),     // C front, left half below the overhang
        (Vec3::new(4.5, 0.0, 5.0), Vec3::X, 6.0, Vec3::NEG_Z, &high),    // C front above the overhang
        (Vec3::new(10.5, 0.0, 5.0), Vec3::Z, 2.0, Vec3::X, &high),       // C right side above the overhang, facing D
        (Vec3::new(12.0, 0.0, 5.0), Vec3::X, 3.0, Vec3::NEG_Z, &d_low),  // D front, left half below the slab
        (Vec3::new(12.0, 0.0, 5.0), Vec3::X, 6.0, Vec3::NEG_Z, &d_high), // D front at the slab
    ];
    for (origin, along, len, out, heights) in faces {
        for &y in heights {
            let a = origin + along * 0.08 + Vec3::Y * y;
            let b = origin + along * (len - 0.08) + Vec3::Y * y;
            // Hold: 12 cm deep, 7 cm tall, top flush with `y`; the grab line runs along its middle.
            let lo = a.min(b) + out.min(Vec3::ZERO) * HOLD_DEPTH - Vec3::Y * 0.07;
            let hi = a.max(b) + out.max(Vec3::ZERO) * HOLD_DEPTH;
            ledge_geo.cuboid(lo, hi);
            level.ledges.push(Ledge { a: a + out * 0.06, b: b + out * 0.06, out });
        }
    }
    // Jump-up blocks on flat ground behind the stairs, fronts facing +Z at z = -12: walkable tops at
    // 1.5, 2 and 2.5 m (jump onto), and G, 4 m tall with holds only from 2.39 m up (jump up to hang).
    // 1.75 and 2.25 m sit between the clips' heights (blended variants reach them exactly).
    for (x, top) in [(-11.0, 1.5), (-8.0, 2.0), (-5.0, 2.5), (-2.0, 1.75), (3.0, 2.25)] {
        w.cuboid(Vec3::new(x - 1.0, 0.0, -14.0), Vec3::new(x + 1.0, top, -12.0));
    }
    w.cuboid(Vec3::new(-16.0, 0.0, -14.0), Vec3::new(-13.0, 4.19, -12.0));
    for y in [2.39, 2.99, 3.59, 4.19] {
        let (a, b) = (Vec3::new(-15.92, y, -12.0), Vec3::new(-13.08, y, -12.0));
        ledge_geo.cuboid(a - Vec3::Y * 0.07, b + Vec3::Z * HOLD_DEPTH);
        level.ledges.push(Ledge { a: a + Vec3::Z * 0.06, b: b + Vec3::Z * 0.06, out: Vec3::Z });
    }
    // Pillars behind A, fronts facing -Z at z = 11.7, 2.4 m apart: their tops (posts to stand on) drop away
    // beside the hands, so topping out pulls up with one hand. Pillar P (x -1.2) is 0.6 m wide with holds up
    // its front to its top at 3.59 m (feet on the wall); pillar Q (x 1.2) is a 0.6 m cap on a thin pole
    // flush with its front, its edge at 2.39 m (hanging from it, hands together, the feet on the pole).
    let pillar_top = 1.79 + 0.6 * 3.0;
    w.cuboid(Vec3::new(-1.5, 0.0, 11.7), Vec3::new(-0.9, pillar_top, 12.3));
    let cap = 2.39;
    w.cuboid(Vec3::new(0.9, cap - 0.2, 11.7), Vec3::new(1.5, cap, 12.3));
    w.cuboid(Vec3::new(1.1, 0.0, 11.7), Vec3::new(1.3, cap - 0.2, 11.9));
    for (x, ys) in [(-1.2, (0..4).map(|k| 1.79 + 0.6 * k as f32).collect::<Vec<_>>()), (1.2, vec![cap])] {
        for y in ys {
            let (a, b) = (Vec3::new(x - 0.22, y, 11.7), Vec3::new(x + 0.22, y, 11.7));
            ledge_geo.cuboid(a - Vec3::Y * 0.07 - Vec3::Z * HOLD_DEPTH, b);
            level.ledges.push(Ledge { a: a - Vec3::Z * 0.06, b: b - Vec3::Z * 0.06, out: Vec3::NEG_Z });
        }
    }
    for (x, y) in [(-1.2, pillar_top), (1.2, cap)] {
        level.perches.push(Line { a: Vec3::new(x, y, 12.0), b: Vec3::new(x, y, 12.0) });
    }
    // Platform E (6 m, north of block D's end) for the ground moves: a ladder up its north face, holds up
    // its south face to its top edge (pull down onto them from the top; the top is a big drop, so walking
    // at the edge stops there and standing at it looks down), a bare west face to run into and lean on,
    // and a haystack off its east face (a leap of faith; another stands on the ground west of it to jump
    // into and hide).
    let e_top = 6.0;
    w.cuboid(Vec3::new(8.0, 0.0, 12.0), Vec3::new(12.0, e_top, 16.0));
    for k in 0..8 {
        let y = e_top - 0.6 * k as f32;
        let (a, b) = (Vec3::new(8.08, y, 12.0), Vec3::new(11.92, y, 12.0));
        ledge_geo.cuboid(a - Vec3::Y * 0.07 - Vec3::Z * HOLD_DEPTH, b);
        level.ledges.push(Ledge { a: a - Vec3::Z * 0.06, b: b - Vec3::Z * 0.06, out: Vec3::NEG_Z });
    }
    level.ladders.push(Ladder { base: Vec3::new(10.0, 0.0, 16.08), top: e_top, out: Vec3::Z });
    // Holds up E's back to the right of its ladder (seen from the ladder): hanging on them, sideways goes across onto
    // the ladder, and off it back onto them.
    for k in 0..7 {
        let y = 1.8 + 0.6 * k as f32;
        let (a, b) = (Vec3::new(10.75, y, 16.0), Vec3::new(11.92, y, 16.0));
        ledge_geo.cuboid(a - Vec3::Y * 0.07, b + Vec3::Z * HOLD_DEPTH);
        level.ledges.push(Ledge { a: a + Vec3::Z * 0.06, b: b + Vec3::Z * 0.06, out: Vec3::Z });
    }
    level.haystacks.push(HayStack { centre: Vec3::new(13.8, 0.0, 14.0), half: 1.3, height: 1.7 });
    // A ledge hanging in the air west of E (a slab on a post set back under it), 2.4 m to the right of E's
    // west holds and 1.2 m below its 3.6 m row: the long leap from E's face into a free hang catches it
    // with one hand and swings until steady.
    let slab = 2.4;
    w.cuboid(Vec3::new(4.6, slab - 0.2, 12.0), Vec3::new(6.4, slab, 12.6));
    w.cuboid(Vec3::new(5.42, 0.0, 12.42), Vec3::new(5.58, slab - 0.2, 12.58));
    let (a, b) = (Vec3::new(4.68, slab, 12.0), Vec3::new(6.32, slab, 12.0));
    ledge_geo.cuboid(a - Vec3::Y * 0.07 - Vec3::Z * HOLD_DEPTH, b);
    level.ledges.push(Ledge { a: a - Vec3::Z * 0.06, b: b - Vec3::Z * 0.06, out: Vec3::NEG_Z });
    level.haystacks.push(HayStack { centre: Vec3::new(5.0, 0.0, 15.5), half: 1.3, height: 1.7 });
    // Tower T: 15 m, climbable on its +Z face (holds every 0.6 m from 1.79 m), with a haystack 2 m off
    // its back edge for a leap of faith.
    let tower_top = 1.79 + 0.6 * 22.0;
    w.cuboid(Vec3::new(12.0, 0.0, -7.0), Vec3::new(15.0, tower_top, -4.0));
    for k in 0..=22 {
        let y = 1.79 + 0.6 * k as f32;
        let (a, b) = (Vec3::new(12.08, y, -4.0), Vec3::new(14.92, y, -4.0));
        ledge_geo.cuboid(a - Vec3::Y * 0.07, b + Vec3::Z * HOLD_DEPTH);
        level.ledges.push(Ledge { a: a + Vec3::Z * 0.06, b: b + Vec3::Z * 0.06, out: Vec3::Z });
    }
    let hay = HayStack { centre: Vec3::new(13.5, 0.0, -9.2), half: 1.3, height: 1.7 };
    level.haystacks.push(hay);
    // Two-step lane (south-east, run along +X at z = -30): a 1 m fence, then two posts 1.8 m tall: free running
    // jumps onto the fence and springs straight on to the first post, then post to post.
    w.cuboid(Vec3::new(42.0, 0.0, -31.5), Vec3::new(42.3, 1.0, -28.5));
    for x in [45.0, 47.6] {
        w.cuboid(Vec3::new(x - 0.2, 0.0, -30.2), Vec3::new(x + 0.2, 1.8, -29.8));
        level.perches.push(Line { a: Vec3::new(x, 1.8, -30.0), b: Vec3::new(x, 1.8, -30.0) });
    }
    // A 2 m box beside it, west: from its top the legs dive into the hay (AC1's free-step entry).
    w.cuboid(Vec3::new(9.6, 0.0, -10.4), Vec3::new(11.8, 2.0, -8.0));
    add(w, Color::srgb(0.75, 0.7, 0.6), &mut level, &mut commands);

    // Free-running course east of the blocks, run toward +X along z = 0: a low wall to vault (1 m),
    // a ramp up to platform P1 (2 m), three posts 2.4 m apart (the first where a running jump off P1
    // comes down), platform P2, a 5 m beam to P3, two swing bars and a low landing platform P4.
    let mut c = Builder { tris: vec![] };
    c.cuboid(Vec3::new(21.5, 0.0, -2.0), Vec3::new(21.9, 1.0, 2.0));
    let ramp = (26.0, 30.0, 2.0);
    c.quad(Vec3::new(ramp.0, 0.0, -2.0), Vec3::new(ramp.0, 0.0, 2.0), Vec3::new(ramp.1, ramp.2, 2.0), Vec3::new(ramp.1, ramp.2, -2.0));
    c.cuboid(Vec3::new(ramp.0, 0.0, -2.0), Vec3::new(ramp.1, 0.01, 2.0));
    for (x0, x1, h) in [(30.0, 33.0, 2.0), (42.4, 45.0, 2.0), (50.0, 53.0, 2.0), (61.5, 65.0, 1.0)] {
        c.cuboid(Vec3::new(x0, 0.0, -2.0), Vec3::new(x1, h, 2.0));
    }
    for x in [36.2, 38.6, 41.0] {
        c.cuboid(Vec3::new(x - 0.2, 0.0, -0.2), Vec3::new(x + 0.2, 2.0, 0.2));
        level.perches.push(Line { a: Vec3::new(x, 2.0, 0.0), b: Vec3::new(x, 2.0, 0.0) });
    }
    c.cuboid(Vec3::new(45.0, 1.8, -0.15), Vec3::new(50.0, 2.0, 0.15));
    level.perches.push(Line { a: Vec3::new(45.0, 2.0, 0.0), b: Vec3::new(50.0, 2.0, 0.0) });
    let bar_y = 4.4;
    for x in [55.0, 58.4] {
        // Uprights at the bar's ends, outside the path.
        for z in [-1.7, 1.7] {
            c.cuboid(Vec3::new(x - 0.1, 0.0, z - 0.1), Vec3::new(x + 0.1, bar_y + 0.1, z + 0.1));
        }
        level.bars.push(Line { a: Vec3::new(x, bar_y, -1.6), b: Vec3::new(x, bar_y, 1.6) });
    }
    // Block L (5 m) south of the course with a ladder up its north face at x = 23.5, and a kiosk frame
    // (overhead bars at 2.1 m, 7 m long) east of it to cross hand over hand.
    c.cuboid(Vec3::new(22.0, 0.0, -9.0), Vec3::new(25.0, 5.0, -6.0));
    level.ladders.push(Ladder { base: Vec3::new(23.5, 0.0, -5.92), top: 5.0, out: Vec3::Z });
    let kiosk = (27.0, 34.0, -7.5, 2.1);
    for x in [kiosk.0, kiosk.1] {
        for z in [kiosk.2 - 0.7, kiosk.2 + 0.7] {
            c.cuboid(Vec3::new(x - 0.08, 0.0, z - 0.08), Vec3::new(x + 0.08, kiosk.3 + 0.1, z + 0.08));
        }
    }
    level.monkey.push(Line { a: Vec3::new(kiosk.0, kiosk.3, kiosk.2), b: Vec3::new(kiosk.1, kiosk.3, kiosk.2) });
    add(c, Color::srgb(0.7, 0.66, 0.6), &mut level, &mut commands);
    // A bench by the scholars' walk (sit on it to hide), and viewpoint tower V east of platform E: holds up its
    // west face to the top (15.6 m), a beam sticking out of its north side at the top to synchronize from,
    // and a haystack below the beam's end for the leap of faith.
    let mut bench = Builder { tris: vec![] };
    bench.cuboid(Vec3::new(-11.0, 0.0, -8.45), Vec3::new(-8.5, 0.42, -8.0));
    add(bench, Color::srgb(0.5, 0.36, 0.22), &mut level, &mut commands);
    level.benches.push((Vec3::new(-10.8, 0.0, -8.2), Vec3::new(-8.7, 0.0, -8.2), Vec3::Z));
    let v_top = 1.79 + 0.6 * 23.0;
    let mut v = Builder { tris: vec![] };
    v.cuboid(Vec3::new(22.0, 0.0, 14.0), Vec3::new(25.0, v_top, 17.0));
    v.cuboid(Vec3::new(23.35, v_top - 0.25, 17.0), Vec3::new(23.65, v_top, 18.4));
    add(v, Color::srgb(0.78, 0.72, 0.62), &mut level, &mut commands);
    let mut v_holds = Builder { tris: vec![] };
    for k in 0..=23 {
        let y = 1.79 + 0.6 * k as f32;
        let (a, b) = (Vec3::new(22.0, y, 14.08), Vec3::new(22.0, y, 16.92));
        v_holds.cuboid(a - Vec3::Y * 0.07 - Vec3::X * HOLD_DEPTH, b);
        level.ledges.push(Ledge { a: a - Vec3::X * 0.06, b: b - Vec3::X * 0.06, out: Vec3::NEG_X });
    }
    add(v_holds, Color::srgb(0.55, 0.5, 0.42), &mut level, &mut commands);
    level.perches.push(Line { a: Vec3::new(23.5, v_top, 17.1), b: Vec3::new(23.5, v_top, 18.3) });
    level.viewpoints.push(Vec3::new(23.5, v_top, 18.2));
    level.haystacks.push(HayStack { centre: Vec3::new(23.5, 0.0, 20.6), half: 1.3, height: 1.7 });
    // Flow lane: a straight, walled run south of the test area (along +X at z -26..-23) to free-run through
    // without losing speed: a 0.6 m block to step up onto and a 1.2 m wall to jump onto, a ramp up to a 2 m
    // walkway, a 2.5 m gap to jump, a jump down across a gap onto a 1 m walkway, a drop back to the ground, a
    // last 1 m wall to jump onto.
    let mut f = Builder { tris: vec![] };
    let (z0, z1) = (-26.0, -23.0);
    f.cuboid(Vec3::new(-22.0, 0.0, z0 - 0.3), Vec3::new(40.0, 3.0, z0));
    f.cuboid(Vec3::new(-22.0, 0.0, z1), Vec3::new(40.0, 3.0, z1 + 0.3));
    for (x, h, d) in [(-14.0, 0.6, 1.2), (-9.0, 1.2, 0.3), (31.0, 1.0, 0.3)] {
        f.cuboid(Vec3::new(x, 0.0, z0), Vec3::new(x + d, h, z1));
    }
    f.quad(Vec3::new(-5.0, 0.0, z0), Vec3::new(-5.0, 0.0, z1), Vec3::new(0.0, 2.0, z1), Vec3::new(0.0, 2.0, z0));
    f.cuboid(Vec3::new(-5.0, 0.0, z0), Vec3::new(0.0, 0.01, z1));
    for (x0, x1, h) in [(0.0, 6.0, 2.0), (8.5, 14.0, 2.0), (16.0, 25.0, 1.0)] {
        f.cuboid(Vec3::new(x0, 0.0, z0), Vec3::new(x1, h, z1));
    }
    add(f, Color::srgb(0.72, 0.68, 0.62), &mut level, &mut commands);
    let mut roofs = Builder { tris: vec![] };
    let mut roof_labels = rooftops(&mut roofs, &mut ledge_geo, &mut level);
    roof_labels.extend(gauntlet(&mut roofs, &mut ledge_geo, &mut level));
    add(roofs, Color::srgb(0.8, 0.74, 0.64), &mut level, &mut commands);
    add(ledge_geo, Color::srgb(0.55, 0.5, 0.42), &mut level, &mut commands);
    // Haystacks are drawn but not part of the collision geometry.
    for hay in &level.haystacks {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(hay.half * 2.0, hay.height, hay.half * 2.0))),
            MeshMaterial3d(mats.add(StandardMaterial { base_color: Color::srgb(0.86, 0.72, 0.33), perceptual_roughness: 1.0, ..default() })),
            Transform::from_translation(hay.centre + Vec3::Y * hay.height * 0.5),
        ));
    }

    for bar in &level.bars {
        let len = (bar.b - bar.a).length();
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.04, len))),
            MeshMaterial3d(mats.add(StandardMaterial { base_color: Color::srgb(0.45, 0.32, 0.2), perceptual_roughness: 0.8, ..default() })),
            Transform::from_translation((bar.a + bar.b) * 0.5).with_rotation(Quat::from_rotation_arc(Vec3::Y, bar.axis())),
        ));
    }

    // Ladders and kiosk frames are drawn, not collided with.
    let wood = mats.add(StandardMaterial { base_color: Color::srgb(0.42, 0.3, 0.18), perceptual_roughness: 0.85, ..default() });
    for l in &level.ladders {
        let side = l.out.cross(Vec3::Y).normalize();
        let h = l.top - l.base.y + 0.9;
        for s in [-0.25, 0.25] {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.05, h, 0.05))),
                MeshMaterial3d(wood.clone()),
                Transform::from_translation(l.base + side * s + Vec3::Y * h * 0.5),
            ));
        }
        let mut y = 0.28;
        while y < h - 0.1 {
            commands.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.02, 0.5))),
                MeshMaterial3d(wood.clone()),
                Transform::from_translation(l.base + Vec3::Y * y).with_rotation(Quat::from_rotation_arc(Vec3::Y, side)),
            ));
            y += 0.25;
        }
    }
    for m in &level.monkey {
        let (axis, len) = (m.axis(), (m.b - m.a).length());
        let side = axis.cross(Vec3::Y).normalize();
        for s in [-0.7, 0.7] {
            commands.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.04, len))),
                MeshMaterial3d(wood.clone()),
                Transform::from_translation((m.a + m.b) * 0.5 + side * s + Vec3::Y * 0.05).with_rotation(Quat::from_rotation_arc(Vec3::Y, axis)),
            ));
        }
        let mut t = 0.3;
        while t < len {
            commands.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.035, 1.4))),
                MeshMaterial3d(wood.clone()),
                Transform::from_translation(m.a + axis * t + Vec3::Y * 0.05).with_rotation(Quat::from_rotation_arc(Vec3::Y, side)),
            ));
            t += 0.55;
        }
    }

    // Names over everything, to tell what is what in screenshots (the crowd gets its own at spawn).
    let names: [(&str, [f32; 3]); 46] = [
        ("Stairs", [0.0, 1.9, -6.5]),
        ("Ramp", [4.5, 2.1, -8.0]),
        ("Block A (climb)", [0.0, 6.5, 6.0]),
        ("Block B (corners)", [-4.5, 6.5, 5.0]),
        ("Block C (overhang)", [7.5, 6.5, 6.0]),
        ("Block D (free hang)", [15.0, 4.1, 6.0]),
        ("Jump 1.5 m", [-11.0, 2.0, -13.0]),
        ("Jump 2 m", [-8.0, 2.5, -13.0]),
        ("Jump 2.5 m", [-5.0, 3.0, -13.0]),
        ("Jump 1.75 m (blend)", [-2.0, 2.25, -13.0]),
        ("Jump 2.25 m (blend)", [3.0, 2.75, -13.0]),
        ("Block G (jump to hang)", [-14.5, 4.7, -13.0]),
        ("Tower T (leap of faith)", [13.5, 15.5, -5.5]),
        ("Hiding spot A (hay)", [13.5, 2.2, -9.2]),
        ("Hay dive box", [10.7, 2.6, -9.2]),
        ("Two-step: fence, posts", [44.0, 2.6, -30.0]),
        ("Pole P (one-hand pull-up)", [-1.2, 4.1, 12.0]),
        ("Pole Q (cap, wall hang)", [1.2, 2.9, 12.0]),
        ("Platform E (ledge stop, pull down, lean)", [10.0, 6.5, 14.0]),
        ("Hiding spot B (hay)", [13.8, 2.2, 14.0]),
        ("Hiding spot C (hay)", [5.0, 2.2, 15.5]),
        ("Risky ledge (one-hand leap from E)", [5.5, 2.9, 12.3]),
        ("Low wall (jump onto)", [21.7, 1.5, 0.0]),
        ("Course ramp", [28.0, 2.5, 0.0]),
        ("P1", [31.5, 2.5, 0.0]),
        ("Posts", [38.6, 2.5, 0.0]),
        ("P2", [43.7, 2.5, 0.0]),
        ("Beam", [47.5, 2.5, 0.0]),
        ("P3", [51.5, 2.5, 0.0]),
        ("Swing bars", [56.7, 4.9, 0.0]),
        ("P4", [63.0, 1.5, 0.0]),
        ("Block L (ladder)", [23.5, 5.5, -7.5]),
        ("Kiosk (monkey bars)", [30.5, 2.6, -7.5]),
        ("Start", [0.0, 0.3, 0.0]),
        ("Flow lane start (free-run along +X)", [-20.0, 1.0, -24.5]),
        ("Flow: step up 0.6", [-13.4, 1.1, -24.5]),
        ("Flow: jump onto 1.2", [-9.0, 1.7, -24.5]),
        ("Flow: ramp", [-2.5, 2.0, -24.5]),
        ("Flow: gap 2.5 m", [7.2, 2.5, -24.5]),
        ("Flow: down 1 m", [15.0, 2.5, -24.5]),
        ("Flow: drop", [25.0, 1.5, -24.5]),
        ("Flow: jump onto 1.0", [31.0, 1.5, -24.5]),
        ("Pose gallery (west)", [-38.0, 3.5, -16.0]),
        ("Bench (sit to hide)", [-9.7, 1.2, -8.2]),
        ("Viewpoint tower V (Q on the beam: synchronize)", [23.5, 16.2, 15.5]),
        ("Hiding spot D (hay, under the viewpoint)", [23.5, 2.2, 20.6]),
    ];
    level.labels = names.iter().map(|(n, p)| (n.to_string(), Vec3::from(*p))).chain(roof_labels.into_iter().map(|(n, p)| (n.to_string(), p))).collect();

    // A prop zone: Damascus's tables, vases, crates and baskets, colliding by AC1's own shapes (boxes, hulls).
    // (Scripted runs leave it out unless asked for, as the pose gallery: its load slows the first frames, and
    // scripts run on the clock.)
    let scripted = std::env::var("AC1_SHOT").is_ok() && std::env::var("AC1_PROPS").is_err();
    if std::env::var("AC1_NO_PROPS").is_err() && !scripted {
        use crate::city::PropPick;
        let at = |x: f32, z: f32| Vec3::new(PROPS.x + x, 0.0, PROPS.z + z);
        let picks = [
            // Two tables with vases on them, a rack of floor vases, a crate and baskets. (Market stalls are
            // skinned, knocked down by physics in the game, and left out.)
            PropPick { name: "Public_Kitchen_Table_01a_002", at: at(0.0, 0.0), yaw: 0.0, on: None },
            PropPick { name: "Vase_Dame", at: at(-0.3, 0.0), yaw: 0.0, on: Some(0) },
            PropPick { name: "Vase_Dame", at: at(0.35, 0.1), yaw: 1.0, on: Some(0) },
            PropPick { name: "Public_Kitchen_Table_01a_004", at: at(3.5, 0.0), yaw: 0.3, on: None },
            PropPick { name: "Vase_Dame", at: at(3.5, 0.0), yaw: 2.0, on: Some(3) },
            PropPick { name: "Merchant_Static_Floor_Vases_R_01a_001", at: at(-3.5, 0.5), yaw: 0.0, on: None },
            PropPick { name: "Crate_1m_01A_202", at: at(7.0, 0.5), yaw: 0.4, on: None },
            PropPick { name: "Crate_1m_01A_203", at: at(7.2, 0.5), yaw: 0.1, on: Some(6) },
            PropPick { name: "Basket_Group_01a_080", at: at(1.5, 3.5), yaw: 0.0, on: None },
        ];
        if let Err(e) = crate::city::spawn_props(&mut commands, &mut meshes, &mut mats, &mut images, &mut level, &dir.0, "DataPC_Damascus.forge", &picks) {
            warn!("no prop zone: {e:#}");
        }
        level.labels.push(("Props (AC1 collision shapes)".to_string(), PROPS + Vec3::Y * 2.2));
    }

    level.build_grid();
    commands.insert_resource(level);
}

/// Where the prop zone is (test world).
const PROPS: Vec3 = Vec3::new(20.0, 0.0, -33.0);

/// The body's collision capsule, as AC1's character proxy: radius 0.4 m (0.35 and its 0.05 m keep distance), 1.8 m
/// tall, lifted 0.37 m off the feet on the ground so lower things pass under it. (From Banned445's
/// AC1-Movement-Rewritten, MIT, Copyright (c) 2026 Banned445.)
pub const BODY_RADIUS: f32 = 0.4;
pub const BODY_HEIGHT: f32 = 1.8;
pub const BODY_LIFT: f32 = 0.37;
/// Faces whose normal is further than this from level (its up component) are floors or ceilings, not walls.
const WALL_MAX_UP: f32 = 0.7;

/// Closest point on triangle `t` to `p`.
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
    let den = 1.0 / (va + vb + vc);
    a + ab * (vb * den) + ac * (vc * den)
}

/// Closest points between segments `p1`-`q1` and `p2`-`q2`.
fn segment_segment(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> (Vec3, Vec3) {
    let (d1, d2, r) = (q1 - p1, q2 - p2, p1 - p2);
    let (a, e, f) = (d1.dot(d1), d2.dot(d2), d2.dot(r));
    let (s, t) = if a <= 1e-8 && e <= 1e-8 {
        (0.0, 0.0)
    } else if a <= 1e-8 {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(r);
        if e <= 1e-8 {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = d1.dot(d2);
            let den = a * e - b * b;
            let mut s = if den > 1e-8 { ((b * f - c * e) / den).clamp(0.0, 1.0) } else { 0.0 };
            let mut t = (b * s + f) / e;
            if t < 0.0 {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            }
            (s, t)
        }
    };
    (p1 + d1 * s, p2 + d2 * t)
}

/// Closest points between segment `a`-`b` and triangle `t` (segment point, triangle point).
fn segment_triangle(a: Vec3, b: Vec3, t: &Tri) -> (Vec3, Vec3) {
    // Crossing the triangle: touching.
    let n = (t.b - t.a).cross(t.c - t.a);
    let (da, db) = (n.dot(a - t.a), n.dot(b - t.a));
    if da * db < 0.0 {
        let x = a + (b - a) * (da / (da - db));
        if closest_on_triangle(x, t).distance_squared(x) < 1e-8 {
            return (x, x);
        }
    }
    let mut best = (a, closest_on_triangle(a, t));
    let mut keep = |p: Vec3, q: Vec3| {
        if p.distance_squared(q) < best.0.distance_squared(best.1) {
            best = (p, q);
        }
    };
    keep(b, closest_on_triangle(b, t));
    for (u, v) in [(t.a, t.b), (t.b, t.c), (t.c, t.a)] {
        let (p, q) = segment_segment(a, b, u, v);
        keep(p, q);
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perch_lookup() {
        let level = Level { perches: vec![Line { a: Vec3::new(0.0, 2.0, 0.0), b: Vec3::new(4.0, 2.0, 0.0) }], ..default() };
        let (i, q) = level.perch_at(Vec3::new(1.0, 2.1, 0.1), 0.3).unwrap();
        assert_eq!(i, 0);
        assert!((q - Vec3::new(1.0, 2.0, 0.0)).length() < 1e-5);
        assert!(level.perch_at(Vec3::new(1.0, 2.0, 0.5), 0.3).is_none());
        assert!(level.perch_at(Vec3::new(1.0, 0.0, 0.0), 0.3).is_none());
        let post = Line { a: Vec3::ONE, b: Vec3::ONE };
        assert_eq!(post.closest(Vec3::ZERO), Vec3::ONE);
        assert_eq!(post.axis(), Vec3::ZERO);
    }

    #[test]
    fn haystack_footprint() {
        let h = HayStack { centre: Vec3::new(1.0, 0.0, 2.0), half: 1.0, height: 1.5 };
        assert!(h.contains(Vec3::new(1.5, 3.0, 2.5), 0.3));
        assert!(!h.contains(Vec3::new(1.8, 3.0, 2.0), 0.3));
        assert!((h.top() - 1.5).abs() < 1e-6);
    }

    #[test]
    fn grid_raycast_matches_brute_force() {
        let mut b = Builder { tris: vec![] };
        for i in 0..20 {
            let x = i as f32 * 3.0 - 30.0;
            b.cuboid(Vec3::new(x, 0.0, -1.0), Vec3::new(x + 1.5, 1.0 + i as f32 * 0.3, 1.0));
        }
        let plain = Level { tris: b.tris.clone(), ..default() };
        let mut gridded = Level { tris: b.tris, ..default() };
        gridded.build_grid();
        for (o, d, m) in [
            (Vec3::new(-29.0, 10.0, 0.0), Vec3::NEG_Y, 20.0),
            (Vec3::new(-40.0, 0.5, 0.2), Vec3::X, 80.0),
            (Vec3::new(5.0, 3.0, -5.0), Vec3::new(0.1, -0.2, 1.0).normalize(), 12.0),
            (Vec3::new(100.0, 1.0, 0.0), Vec3::NEG_Y, 5.0),
        ] {
            let (a, b) = (plain.raycast(o, d, m), gridded.raycast(o, d, m));
            assert_eq!(a.is_some(), b.is_some());
            if let (Some(a), Some(b)) = (a, b) {
                assert!((a.dist - b.dist).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn raycast_hits_box_top() {
        let mut b = Builder { tris: vec![] };
        b.cuboid(Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 0.5, 1.0));
        let level = Level { tris: b.tris, ..default() };
        let hit = level.ground(Vec3::new(0.2, 0.0, 0.3), 2.0, 2.0).unwrap();
        assert!((hit.point.y - 0.5).abs() < 1e-4);
        assert!(hit.normal.y > 0.99);
        assert!(level.ground(Vec3::new(3.0, 0.0, 0.0), 2.0, 2.0).is_none());
    }

    /// A box from `lo` to `hi` as a level.
    fn boxed(lo: Vec3, hi: Vec3) -> Level {
        let mut b = Builder { tris: vec![] };
        b.cuboid(lo, hi);
        let mut l = Level { tris: b.tris, ..default() };
        l.build_grid();
        l
    }

    #[test]
    fn the_capsule_is_pushed_off_walls_but_walks_up_steps() {
        // A wall 2 m high from x = 1: feet 0.2 m from it are pushed back to the radius.
        let wall = boxed(Vec3::new(1.0, 0.0, -2.0), Vec3::new(2.0, 2.0, 2.0));
        let push = wall.capsule_push(Vec3::new(0.8, 0.0, 0.0));
        assert!((push.x + (BODY_RADIUS - 0.2)).abs() < 0.01 && push.y == 0.0 && push.z.abs() < 1e-4, "{push:?}");
        assert_eq!(wall.capsule_push(Vec3::new(0.5, 0.0, 0.0)), Vec3::ZERO);
        // Low things pass under the lifted capsule; a 0.45 m step is walked up; 0.6 m is a wall.
        for (h, blocks) in [(0.3, false), (0.45, false), (0.6, true)] {
            let step = boxed(Vec3::new(1.0, 0.0, -2.0), Vec3::new(2.0, h, 2.0));
            let push = step.capsule_push(Vec3::new(0.85, 0.0, 0.0));
            assert_eq!(push.length() > 0.01, blocks, "{h} m: {push:?}");
        }
        // Into a corner: out of both walls.
        let mut corner = boxed(Vec3::new(1.0, 0.0, -2.0), Vec3::new(2.0, 2.0, 2.0));
        let other = boxed(Vec3::new(-2.0, 0.0, 1.0), Vec3::new(2.0, 2.0, 2.0));
        corner.tris.extend(other.tris);
        corner.build_grid();
        let push = corner.capsule_push(Vec3::new(0.8, 0.0, 0.8));
        assert!(push.x < -0.15 && push.z < -0.15, "{push:?}");
    }
}
