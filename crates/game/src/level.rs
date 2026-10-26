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

    /// Where the vertical plane through `p` along `dir` meets the ledge, clamped to its ends: AC1's grab point, `p`
    /// projected onto the hold along the probe's direction, not across to it (audit GUIDE-8, reanalysis GUIDE-9). The
    /// closest point when `dir` runs along the ledge.
    pub fn project_along(&self, p: Vec3, dir: Vec3) -> Vec3 {
        let d = self.b - self.a;
        let n = dir.with_y(0.0).normalize_or_zero().cross(Vec3::Y);
        let denom = d.dot(n);
        if n == Vec3::ZERO || denom.abs() < 1e-4 * d.length() {
            return self.closest(p);
        }
        let t = ((p - self.a).dot(n) / denom).clamp(0.0, 1.0);
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
/// height of the walkable top it leads to, `out` points away from the wall, `lean` how far its axis runs level for each
/// metre up (AC1 climbs along its ladder's axis, the frame leaning with it: reanalysis LADDER-17; Damascus' authored
/// ladders lean up to 8 degrees, ladder 68 3.8, its top 0.5 m in from its foot).
#[derive(Clone, Copy, Debug)]
pub struct Ladder {
    pub base: Vec3,
    pub top: f32,
    pub out: Vec3,
    pub lean: Vec3,
}

impl Ladder {
    /// An upright ladder.
    pub fn upright(base: Vec3, top: f32, out: Vec3) -> Self {
        Ladder { base, top, out, lean: Vec3::ZERO }
    }

    /// The point of its axis at height `y`.
    pub fn at(&self, y: f32) -> Vec3 {
        (self.base + self.lean * (y - self.base.y)).with_y(y)
    }

    /// `p` from its axis at `p`'s height, its height from its base.
    pub fn off_axis(&self, p: Vec3) -> Vec3 {
        (p - self.at(p.y)).with_y(p.y - self.base.y)
    }
}

/// A haystack to dive into: square footprint around `centre` (on the ground), `height` tall. Not part
/// of the collision geometry (the climber sinks into it).
#[derive(Clone, Copy, Debug)]
pub struct HayStack {
    pub centre: Vec3,
    pub half: f32,
    pub height: f32,
    /// The way one waits in it facing (AC1 turns to the hiding spot's own facing at the wait); zero where unknown.
    pub facing: Vec3,
    /// Its own meshes' world bounds (min, max) where known: what the hop out's room test leaves out.
    pub own: Option<(Vec3, Vec3)>,
}

/// A haystack's or hiding spot's centre and facing: the placed entity's own position (AC1 dives to it, audit HAY-3;
/// the middle of the mesh bounds was 0.22 m off), on the floor under the bounds, unless it lies outside them (a
/// skinned pile's root); the facing its local `HAY_FACING_AXIS` (AC1's wait turns to it, HAY-2).
fn hay_place(lo: Vec3, hi: Vec3, tf: &Transform) -> (Vec3, Vec3) {
    let mid = Vec3::new((lo.x + hi.x) * 0.5, lo.y, (lo.z + hi.z) * 0.5);
    let at = tf.translation;
    let inside = at.x > lo.x && at.x < hi.x && at.z > lo.z && at.z < hi.z;
    let facing = (tf.rotation * HAY_FACING_AXIS).with_y(0.0).normalize_or_zero();
    let centre = if inside { Vec3::new(at.x, lo.y, at.z) } else { mid };
    (centre, facing)
}

impl HayStack {
    /// Where a dive in ends: `HAY_SEAT_ON` along its facing from its middle.
    pub fn seat(&self) -> Vec3 {
        self.centre + self.facing * HAY_SEAT_ON
    }
    pub fn top(&self) -> f32 {
        self.centre.y + self.height
    }
    /// Is `p` over the footprint, `margin` in from its edges?
    pub fn contains(&self, p: Vec3, margin: f32) -> bool {
        let d = (p - self.centre).abs();
        d.x <= self.half - margin && d.z <= self.half - margin
    }
}

/// A hold within `HAY_HEAP_MARGIN` outside a haystack's footprint (a beam inside it), `HAY_HEAP_ABOVE` to `HAY_HEAP_TOP`
/// above its rim, is on its heap (m).
pub(crate) const HAY_HEAP_MARGIN: f32 = 0.5;
pub(crate) const HAY_HEAP_ABOVE: f32 = 0.3;
/// The local axis a haystack entity faces along, and how far along it from the entity the dive ends (m): calibrated on
/// `hay70`, the running game's four dives all ending 0.21 m that way (0.08 m to the side unaccounted for), facing it.
const HAY_FACING_AXIS: Vec3 = Vec3::NEG_X;
const HAY_SEAT_ON: f32 = 0.21;
/// A ladder's walkable top is looked for this far in from it (m).
const LADDER_TOP_IN: f32 = 0.5;
/// Holds are carved out this far either side of a ladder and out from it (m).
pub(crate) const LADDER_CARVE_SIDE: f32 = 0.3;
pub(crate) const LADDER_CARVE_OUT: f32 = 0.75;
/// AC1's ladder zone runs this far under a ladder's foot and past its top (m; `sub_116F060`, Banned445's port).
const LADDER_ZONE_PAST: f32 = 0.2;
/// (Up to this over the rim: the Damascus bureau's roof heap stands 2.5 m over it; above, a balcony over the hay.)
pub(crate) const HAY_HEAP_TOP: f32 = 3.0;
/// A city haystack is at most this wide each side of its middle (m).
const HAY_HALF_MAX: f32 = 1.5;
/// A rooftop hiding spot's rim over the roof (m; `Hiding_Spot_P_01a` in Damascus, the running game: AC1 hopped up onto
/// it, 1.07 m, and dived in).
const HIDING_SPOT_RIM: f32 = 1.07;
/// Hopping out of the hay wants room in a box this far past its footprint's edge (m), this high over the root (m), of
/// these half extents across, along and up (m): AC1's `Guard_HopOut` 0xE434E0 (`Level::hay_hop_room`).
const HAY_HOP_ROOM: f32 = 0.5;
const HAY_HOP_UP: f32 = 1.25;
const HAY_HOP_BOX: Vec3 = Vec3::new(0.35, 0.5, 0.75);
/// A hay or hiding-place entity's collision shape this wide or less (m) is its own (`Level::add_city_objects`).
const HAY_SHAPE_MAX: f32 = 8.0;
/// A city haystack's height when its mesh gives none (m).
const CITY_HAY_HEIGHT: f32 = 1.8;

/// An authored guidance edge (`forge::guidance`), in the world: its kind, ends and the way out from its wall.
#[derive(Clone, Copy, Debug)]
pub struct Authored {
    pub kind: forge::guidance::SubType,
    pub a: Vec3,
    pub b: Vec3,
    pub out: Vec3,
    /// The placing entity's own +Y axis in the world (flat): a kiosk's way along its counter.
    pub axis: Vec3,
    /// Where it comes from: an index into `Level::authored_src` (the grab data's audit, `crate::grab_audit`).
    pub src: u32,
}

/// A kiosk (a market stall, `HumanKiosk`): one of its counter's `Kiosk` guidance pieces (the front and the two sides) and
/// the stall entity's own +Y, along its front: the entry side is taken against it (0xE08679 reads the entity's second
/// matrix row through 0x4C2F90).
#[derive(Clone, Copy, Debug)]
pub struct Kiosk {
    pub edge: Ledge,
    pub axis: Vec3,
}

/// An authored ledge edge steeper than this (rise over length, sin 40 degrees) is a stair or roof slope, not a hold: AC1
/// drops report edges past 40 degrees (0x66C350, Banned445's port; audit GUIDE-1: ours' 0.35, 20 degrees, threw away
/// 21,312 Damascus holds AC1 keeps).
pub(crate) const AUTHORED_MAX_SLOPE: f32 = 0.643;
/// AC1's hold grid shortens a guidance edge this much at its ends (m; `BuildHoldGrid` 0xDF6A40, docs/PARKOUR.md
/// section 4)...
const AUTHORED_TRIM: f32 = 0.1;
/// ...and a piece shorter than that holds at its middle: kept this long there (m).
const AUTHORED_POINT: f32 = 0.01;
/// A top behind a hold this deep or less (m), with a drop past it, is a narrow wall top to balance on, not to stand:
/// a beam's width. (At 0.6 m Damascus had 10,557, its parapets among them, and running along a roof switched in and
/// out of balancing at every one; AC1 runs on those as ground, its beams found by `GuidanceBeamDetectorAccurate`.)
const NARROW_TOP_MAX: f32 = 0.35;
/// Two holds facing away from each other this far apart or less (m) bound a beam (AC1's beam contacts: half-width at
/// most 0.5 m), when along each other within 30 degrees (`PAIR_BEAM_COS`) and facing away within 30.
const PAIR_BEAM_WIDTH: f32 = 1.0;
const PAIR_BEAM_COS: f32 = 0.866;
/// A paired beam needs a drop past both its edges: no floor within `PAIR_BEAM_DROP` (m) under its top, `PAIR_BEAM_OUT`
/// (m) out from each.
const PAIR_BEAM_DROP: f32 = 0.5;
const PAIR_BEAM_OUT: f32 = 0.3;
/// ...tested this often along it (m).
const PAIR_BEAM_STEP: f32 = 0.25;
/// Pieces of one paired beam in line this far apart or less (m), over the same top, are joined.
const PAIR_BEAM_GAP: f32 = 1.0;
/// A perch is backed by an authored ledge grab this far across (m) and up or down (m) from its middle (`drop_unbacked`).
const BACKED_ACROSS: f32 = 0.6;
const BACKED_UP: f32 = 0.3;
/// Rays this long (m) decide whether a point is inside a solid (`inside_solid`).
const INSIDE_PROBE: f32 = 12.0;

/// A vertex to the millimetre (`Level::authored_ends`).
pub(crate) fn vertex_key(p: Vec3) -> (i64, i64, i64) {
    ((p.x * 1000.0).round() as i64, (p.y * 1000.0).round() as i64, (p.z * 1000.0).round() as i64)
}

/// Is an authored edge steeper than AC1's reports take (`GuidanceReport__RemoveSteepEdges` 0x66C350: past 40 degrees, or
/// of no length)?
pub(crate) fn authored_steep(e: &Authored) -> bool {
    let len = e.a.distance(e.b);
    len <= 1e-4 || (e.b.y - e.a.y).abs() / len > AUTHORED_MAX_SLOPE
}

/// The hold a piece of an authored ledge edge gives, `from` to `to` metres from its start along it. Shortened
/// `AUTHORED_TRIM` at a free end (`ends`: `Level::authored_ends`) and where AC1's report cut it (round a ladder, the depth
/// filter), as AC1's hold grid takes an edge; not where it goes on into another piece: a sill in three pieces of
/// 0.15-0.23 m is one 0.87 m hold. A piece too short for that holds at its middle, as AC1's probes take one
/// (`sub_116D3C0`: shortened 0.1 m at both ends, the midpoint when it is 0.2 m or less; audit GUIDE-2): a railing's knob,
/// a lone window-frame piece.
pub(crate) fn authored_piece(e: &Authored, from: f32, to: f32, ends: &HashMap<(i64, i64, i64), u32>) -> Ledge {
    let len = e.a.distance(e.b).max(1e-6);
    let d = (e.b - e.a) / len;
    let free = |p: Vec3| if ends.get(&vertex_key(p)).copied().unwrap_or(0) <= 1 { AUTHORED_TRIM } else { 0.0 };
    let ta = if from <= 1e-4 { free(e.a) } else { AUTHORED_TRIM };
    let tb = if to >= len - 1e-4 { free(e.b) } else { AUTHORED_TRIM };
    let (pa, pb) = (e.a + d * from, e.a + d * to);
    if to - from > ta + tb + AUTHORED_POINT {
        return Ledge { a: pa + d * ta, b: pb - d * tb, out: e.out };
    }
    let mid = (pa + pb) * 0.5;
    Ledge { a: mid - d * AUTHORED_POINT * 0.5, b: mid + d * AUTHORED_POINT * 0.5, out: e.out }
}

/// What AC1's rules make of an authored ledge grab (`Level::ac1_hold_pieces`): holds kept (some of it), or none, and
/// the rule that took the last of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoldFate {
    Kept,
    /// Steeper than 40 degrees (0x66C350).
    Steep,
    /// No wall to face out from (no normal with a flat part).
    NoWall,
    /// Inside a ladder's zone (0x1170880).
    Ladder,
    /// Less than the depth detector's metre of room in front of it along all of it (`crate::guide_depth`).
    Depth,
}

/// A ladder's zone that AC1's reports cut the ledge grabs out of (`sub_1170880` -> `sub_116F060`, Banned445's port): a
/// box from the ladder edge's lower end, 0.3 m either side, 0.75 m out and in, from 0.2 m under it to 0.2 m past its top.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LadderZone {
    base: Vec3,
    right: Vec3,
    fwd: Vec3,
    up: Vec3,
    len: f32,
    lo: Vec3,
    hi: Vec3,
}

impl LadderZone {
    fn of(e: &Authored) -> Option<Self> {
        let (base, top) = if e.a.y <= e.b.y { (e.a, e.b) } else { (e.b, e.a) };
        let up = (top - base).normalize_or_zero();
        let fwd = e.out.with_y(0.0).normalize_or_zero();
        let right = fwd.cross(Vec3::Y).normalize_or_zero();
        if up == Vec3::ZERO || right == Vec3::ZERO {
            return None;
        }
        let len = top.distance(base);
        let corner = |r: f32, f: f32, u: f32| base + right * r + fwd * f + up * u;
        let pts = [-1.0f32, 1.0].iter().flat_map(|&r| [-1.0f32, 1.0].map(move |f| (r, f))).flat_map(|(r, f)| {
            [
                corner(r * LADDER_CARVE_SIDE, f * LADDER_CARVE_OUT, -LADDER_ZONE_PAST),
                corner(r * LADDER_CARVE_SIDE, f * LADDER_CARVE_OUT, len + LADDER_ZONE_PAST),
            ]
        });
        let (lo, hi) = pts.fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(l, h), p| (l.min(p), h.max(p)));
        Some(LadderZone { base, right, fwd, up, len, lo, hi })
    }

    /// The part of the segment `a`-`b` inside the zone, as parameters (t0, t1) along it, if any.
    fn cut(&self, a: Vec3, b: Vec3) -> Option<(f32, f32)> {
        if a.max(b).cmplt(self.lo).any() || a.min(b).cmpgt(self.hi).any() {
            return None;
        }
        let (mut t0, mut t1) = (0.0f32, 1.0f32);
        let half = [LADDER_CARVE_SIDE, LADDER_CARVE_OUT, self.len * 0.5 + LADDER_ZONE_PAST];
        let mid = self.base + self.up * self.len * 0.5;
        for (axis, h) in [self.right, self.fwd, self.up].into_iter().zip(half) {
            let (u, v) = ((a - mid).dot(axis), (b - mid).dot(axis));
            if (v - u).abs() < 1e-9 {
                if u.abs() > h {
                    return None;
                }
                continue;
            }
            let (s0, s1) = ((-h - u) / (v - u), (h - u) / (v - u));
            t0 = t0.max(s0.min(s1));
            t1 = t1.min(s0.max(s1));
        }
        (t0 < t1).then_some((t0, t1))
    }
}

/// A city mesh's part in parkour (see `city::city_object`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CityObject {
    Hay,
    /// A rooftop hiding spot (`Hiding_Spot_*`: a plank box under a tarp): dived into as hay, from its rim.
    HidingSpot,
    Ladder,
    Bench,
    Pole,
}

/// A lip is a hold only if the wall under it is at most this far behind it (m).
const LIP_WALL_IN: f32 = 0.25;

/// Which of AC1's other searches looks at the guidance (`Level::holds_of`; the climbing grid's and the ledge report's
/// holds, the ladders' zones cut out and `guide_depth::LEDGE`, are `Level::ledges`): each runs its own report and depth
/// filter over the same ledge grabs (steep ones out, ladders set aside), so each sees its own pieces of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoldView {
    /// The air catch's hand searches (`CheckAirCatch` 0xE0BB70's pre-filtered report): `guide_depth::CATCH_HANDS`, no
    /// ladder zones.
    Catch,
    /// The jump query (0xE18970): `guide_depth::JUMP`, no ladder zones.
    Jump,
    /// The pull-down's ledge report (0xDDCD2F): the ladders' zones cut out, `guide_depth::PULL_DOWN`.
    PullDown,
}

impl HoldView {
    /// Its index in `Level::views`.
    fn slot(self) -> usize {
        match self {
            HoldView::Catch => 0,
            HoldView::Jump => 1,
            HoldView::PullDown => 2,
        }
    }
}

#[derive(Resource, Default)]
pub struct Level {
    /// The city's navigation meshes, for walking routes (see `nav`).
    pub nav: Option<crate::nav::NavGraph>,
    pub tris: Vec<Tri>,
    pub ledges: Vec<Ledge>,
    /// AC1's authored guidance edges (cities): see `use_authored`.
    pub authored: Vec<Authored>,
    /// The authored edges' sources, "group or entity, a tab, its first mesh" (`Authored::src`), and their indices.
    pub authored_src: Vec<String>,
    pub authored_src_at: HashMap<String, u32>,
    /// Ladders came from the authored edges (the name-based ones in `add_city_objects` are then left out).
    pub authored_ladders: bool,
    /// The holds are AC1's authored ledge grabs (`use_authored`), not edges found in the geometry.
    pub authored_holds: bool,
    /// City meshes that are parkour objects, by kind, with their world bounds (see `add_city_objects`).
    pub city_objects: Vec<(CityObject, Vec3, Vec3, Transform)>,
    /// The world bounds of the collision shapes of entities that are hay or hiding places (see `HayStack::own`).
    pub hay_shapes: Vec<(Vec3, Vec3)>,
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
    /// Kiosks' counters, jumped onto (see `Kiosk`).
    pub kiosks: Vec<Kiosk>,
    /// Triangles by `GRID` cell (x, z) they overlap, for raycasts in big levels (empty: test them all).
    pub(crate) grid: FastMap<(i32, i32), Vec<u32>>,
    /// Triangles too big for the grid (the ground plane around the test area), tested by every raycast.
    pub(crate) big: Vec<u32>,
    /// The edges AC1's foot-level edge reports come from (`crate::edge_report`), and their index by cell; made with the
    /// raycast grid.
    pub(crate) report_edges: Vec<Ledge>,
    pub(crate) report_grid: FastMap<(i32, i32), Vec<u32>>,
    /// The holds by cell (`crate::ledge_grid`), made with the raycast grid, and how many holds it was made for.
    pub(crate) hold_grid: crate::ledge_grid::LedgeGrid,
    pub(crate) hold_grid_len: usize,
    /// AC1's other searches' own views of the guidance (`HoldView`; `ledges` is the climbing grid's), each by AC1's own
    /// filter for that search (`ac1_views`); empty without authored guidance (they read `ledges` then).
    pub views: [Vec<Ledge>; 3],
    /// Where the player starts and a loop for a crowd, when the level says (a city).
    pub spawn: Option<Vec3>,
    pub crowd_path: Option<Vec<Vec3>>,
    /// A city from the game data (not the test level).
    pub city: bool,
    /// Beams' top lines from the city, before `add_beam_perches` keeps their open stretches as perches.
    pub beam_tops: Vec<Line>,
    /// Names shown over the test world's buildings and features (for finding things while testing).
    pub labels: Vec<(String, Vec3)>,
    /// Per triangle of `tris` (cities): its surface's sound material (AC1's `CollisionMaterial`, see
    /// `sound_material_at`), `NO_SOUND` where not known; shorter than `tris` when the last ones have none.
    pub tri_sound: Vec<u8>,
    /// The runs of `tris` (start, end; in order) on AC1's fuzzy-zone layers (`forge::shape::fuzzy`: tents, big trees,
    /// out-of-bounds walls): they stop the body, but are no wall to run up and no support for a foot (`sub_E1C1F0`).
    pub fuzzy_tris: Vec<(u32, u32)>,
}

/// `Level::tri_sound` of a triangle whose surface has no sound material.
pub const NO_SOUND: u8 = u8::MAX;

/// Test level holds: strips this deep (m) out from their wall, the grab line 6 cm out (where the hands hold; the strip's
/// outer edge, beyond it, is outlined as well: see `draw_edges`).
const HOLD_DEPTH: f32 = 0.12;

/// Where the player starts in Masyaf (Bevy x, _, z): the village below the fortress.
const MASYAF_SPAWN: [f32; 3] = [20.0, 0.0, 50.0];

/// A hasher for small keys (grid cells, ids) by multiplies, not SipHash: every raycast looks cells up (with the default
/// hasher the Damascus load's hold and beam checks, hundreds of thousands of rays, took 10 s).
#[derive(Default)]
pub struct FastHasher(u64);

impl std::hash::Hasher for FastHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u32(b as u32);
        }
    }
    fn write_u32(&mut self, n: u32) {
        self.0 = (self.0.rotate_left(5) ^ n as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
    fn write_i32(&mut self, n: i32) {
        self.write_u32(n as u32);
    }
}

pub type FastMap<K, V> = HashMap<K, V, std::hash::BuildHasherDefault<FastHasher>>;

/// `f` over `items` on all cores, the results in the items' order (the level's checks each read the level only).
pub(crate) fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = items.len().div_ceil(threads).max(1);
    let f = &f;
    std::thread::scope(|sc| {
        let jobs: Vec<_> = items.chunks(chunk).map(|part| sc.spawn(move || part.iter().map(f).collect::<Vec<R>>())).collect();
        jobs.into_iter().flat_map(|j| j.join().unwrap_or_default()).collect()
    })
}

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
        self.report_edges = crate::edge_report::report_edges(self);
        self.report_grid = crate::edge_report::report_grid(&self.report_edges);
        self.hold_grid = crate::ledge_grid::LedgeGrid::new(&self.ledges);
        self.hold_grid_len = self.ledges.len();
    }

    pub fn raycast(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<Hit> {
        self.raycast_sided(origin, dir, max).map(|(h, _)| h)
    }

    /// `raycast`, and whether the face hit was met from behind (its winding faces away from the ray: the ray
    /// started inside a solid).
    pub fn raycast_sided(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<(Hit, bool)> {
        self.raycast_at(origin, dir, max).map(|(h, behind, _)| (h, behind))
    }

    /// `raycast`, and whether what it hit is a fuzzy zone (`fuzzy_tris`).
    pub fn raycast_fuzzy(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<(Hit, bool)> {
        self.raycast_at(origin, dir, max).map(|(h, _, i)| (h, self.is_fuzzy(i)))
    }

    /// Is triangle `i` of `tris` on a fuzzy-zone layer?
    pub fn is_fuzzy(&self, i: u32) -> bool {
        let k = self.fuzzy_tris.partition_point(|r| r.1 <= i);
        self.fuzzy_tris.get(k).is_some_and(|r| r.0 <= i)
    }

    /// The nearest hit, whether met from behind, and the triangle's index.
    fn raycast_at(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<(Hit, bool, u32)> {
        let at = |i: &u32| (*i, &self.tris[*i as usize]);
        if self.grid.is_empty() {
            return self.raycast_tris((0..self.tris.len() as u32).map(|i| at(&i)), origin, dir, max);
        }
        let end = origin + dir * max;
        let (lo, hi) = (origin.min(end), origin.max(end));
        let mut best = self.raycast_tris(self.big.iter().map(at), origin, dir, max);
        for x in cell(lo.x)..=cell(hi.x) {
            for z in cell(lo.z)..=cell(hi.z) {
                let Some(list) = self.grid.get(&(x, z)) else { continue };
                if let Some(h) = self.raycast_tris(list.iter().map(at), origin, dir, max)
                    && best.is_none_or(|b| h.0.dist < b.0.dist)
                {
                    best = Some(h);
                }
            }
        }
        best
    }

    fn raycast_tris<'t>(&self, tris: impl Iterator<Item = (u32, &'t Tri)>, origin: Vec3, dir: Vec3, max: f32) -> Option<(Hit, bool, u32)> {
        let mut best: Option<(Hit, bool, u32)> = None;
        for (i, t) in tris {
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
            best = Some((Hit { point: origin + dir * d, normal: n, dist: d }, behind, i));
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
        // (A haystack's own bounds: its meshes', with the collision shapes of hay and hiding-place entities touching them
        // (a cart's bed, a hiding place's box), each no bigger than `HAY_SHAPE_MAX` across.)
        let shapes = std::mem::take(&mut self.hay_shapes);
        let own_of = |lo: Vec3, hi: Vec3| {
            shapes
                .iter()
                .filter(|(a, b)| (*b - *a).with_y(0.0).max_element() <= HAY_SHAPE_MAX && a.cmple(hi).all() && b.cmpge(lo).all())
                .fold((lo, hi), |(l, h), (a, b)| (l.min(*a), h.max(*b)))
        };
        // Hay: each cart or pile's meshes merged.
        let mut piles: Vec<(Vec3, Vec3, Transform)> = vec![];
        for &(_, lo, hi, tf) in objs.iter().filter(|o| o.0 == CityObject::Hay) {
            match piles.iter_mut().find(|(a, b, _)| lo.x < b.x + 0.3 && hi.x > a.x - 0.3 && lo.z < b.z + 0.3 && hi.z > a.z - 0.3) {
                Some((a, b, _)) => {
                    *a = a.min(lo);
                    *b = b.max(hi);
                }
                None => piles.push((lo, hi, tf)),
            }
        }
        for (lo, hi, tf) in piles {
            let size = hi - lo;
            if size.x.max(size.z) > 8.0 || size.x.min(size.z) < 0.8 {
                continue;
            }
            // (The hay meshes come flat out of their data; AC1's carts and piles stand about this high.)
            let height = if size.y < 0.4 { CITY_HAY_HEIGHT } else { size.y };
            // (The skinned hay meshes measure twice a cart's size, 6.2 m, their scale unsure: the stack is its middle.)
            let half = (size.x.min(size.z) * 0.5).min(HAY_HALF_MAX);
            let (centre, facing) = hay_place(lo, hi, &tf);
            // (Its own bounds up to its height: the flat meshes leave out the cart's bed under the hay.)
            let own = own_of(lo.min(centre - Vec3::new(half, 0.0, half)), hi.max(centre + Vec3::new(half, height, half)));
            self.haystacks.push(HayStack { centre, half, height, facing, own: Some(own) });
            hay += 1;
        }
        for &(kind, lo, hi, tf) in objs.iter().filter(|o| o.0 != CityObject::Hay) {
            let size = hi - lo;
            let mid = (lo + hi) * 0.5;
            // The long horizontal axis, and across it.
            let (along, across) = if size.x >= size.z { (Vec3::X, Vec3::Z) } else { (Vec3::Z, Vec3::X) };
            let wall_side =
                |at: Vec3| -> Option<Vec3> { [across, -across].into_iter().find(|d| self.raycast(at, *d, 0.8).is_some_and(|h| h.normal.y.abs() < 0.3)) };
            match kind {
                // (Authored ladders, when the city has them, replace the ones guessed from meshes.)
                CityObject::Ladder if self.authored_ladders => {}
                // (Nor swing poles by name where the holds are AC1's: its poles are authored, and a `Pole_*` mesh without
                // one has none in AC1; Damascus had 11 such bars, laundry poles among them.)
                CityObject::Pole if self.authored_holds => {}
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
                    self.ladders.push(Ladder::upright((mid + out * off).with_y(lo.y), top, out));
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
                CityObject::HidingSpot => {
                    // (Its top for diving in is the box's rim, not the tarp's frame over it.)
                    let half = (size.x.min(size.z) * 0.5).min(HAY_HALF_MAX);
                    let (centre, facing) = hay_place(lo, hi, &tf);
                    debug!("hiding place at {centre:.2}, half {half:.2}, facing {facing:.2} (bounds {lo:.2} to {hi:.2})");
                    self.haystacks.push(HayStack { centre, half, height: HIDING_SPOT_RIM, facing, own: Some(own_of(lo, hi)) });
                    hay += 1;
                }
            }
        }
        (hay, ladders, benches, bars)
    }

    /// The collision triangles whose bounds meet the box `lo`..`hi`, as `crate::beam_pairs` takes them.
    pub fn tris_in(&self, lo: Vec3, hi: Vec3) -> Vec<[Vec3; 3]> {
        let mut ids: Vec<u32> = (cell(lo.x)..=cell(hi.x))
            .flat_map(|x| (cell(lo.z)..=cell(hi.z)).map(move |z| (x, z)))
            .flat_map(|k| self.grid.get(&k).into_iter().flatten().copied())
            .collect();
        ids.extend(self.big.iter().copied());
        ids.sort_unstable();
        ids.dedup();
        ids.iter()
            .map(|&i| &self.tris[i as usize])
            .filter(|t| t.a.min(t.b).min(t.c).cmple(hi).all() && t.a.max(t.b).max(t.c).cmpge(lo).all())
            .map(crate::beam_pairs::tri)
            .collect()
    }

    /// AC1's beams and posts: the pairs of its authored hold edges facing each other across a narrow top, cut where
    /// geometry stands over them (`crate::beam_pairs`). Returns how many.
    pub fn pair_beams(&mut self) -> usize {
        let edges: Vec<crate::beam_pairs::Edge> = self
            .authored
            .iter()
            .filter(|e| e.kind == forge::guidance::SubType::LedgeGrab && e.out != Vec3::ZERO)
            .map(|e| crate::beam_pairs::Edge { p0: e.a, p1: e.b, out: e.out })
            .collect();
        let beams = crate::beam_pairs::detect(&edges, |lo, hi| self.tris_in(lo, hi));
        let lines = crate::beam_pairs::lines(&beams);
        let n = lines.len();
        self.perches.extend(lines);
        n
    }

    /// AC1's posts (pilotis): `sub_B2B600` over every authored hold edge, as the narrow object's search runs it from each
    /// (read in Ghidra; Banned445's AC1-Movement-Rewritten `find_pilotis`, MIT, written here anew). From the edge's middle,
    /// facing into its top: the edge itself (an outward normal within 45 degrees of back, within 0.3 m), one facing the
    /// other way within 1.3 m ahead, and from between those two one each side within 0.6 m, all within 0.5 m in height and
    /// 30 degrees of their search's way; the front and back at most 0.7 m apart and the two sides too; the top the mean of
    /// the four, kept only where a capsule of 0.4 m from 0.6 to 1.4 m over it is free (`sub_B2A290`). So a post is a top at
    /// most 0.7 m square, open over it: never one against a wall. The paired beams lying on a post's own top (its edges
    /// pair across it too) go: the post is what AC1 stands on there. Returns how many.
    pub fn add_pilotis(&mut self) -> usize {
        let edges: Vec<&Authored> = self.authored.iter().filter(|e| e.kind == forge::guidance::SubType::LedgeGrab && e.out != Vec3::ZERO).collect();
        let key = |p: Vec3| ((p.x / 2.0).floor() as i32, (p.z / 2.0).floor() as i32);
        let mut grid: FastMap<(i32, i32), Vec<usize>> = FastMap::default();
        for (i, e) in edges.iter().enumerate() {
            let (lo, hi) = (e.a.min(e.b), e.a.max(e.b));
            for x in key(lo).0..=key(hi).0 {
                for z in key(lo).1..=key(hi).1 {
                    grid.entry((x, z)).or_default().push(i);
                }
            }
        }
        let line = |e: &Authored| Line { a: e.a, b: e.b };
        let flat = |a: Vec3, b: Vec3| (a - b).with_y(0.0).length();
        let found = par_map(&edges, |seed| -> Option<Vec3> {
            let mid = (seed.a + seed.b) * 0.5;
            let f = -seed.out.with_y(0.0).normalize_or_zero();
            let r = f.cross(Vec3::Y);
            let (k0, k1) = (key(mid - Vec3::splat(1.5)), key(mid + Vec3::splat(1.5)));
            let mut near: Vec<usize> =
                (k0.0..=k1.0).flat_map(|x| (k0.1..=k1.1).map(move |z| (x, z))).flat_map(|k| grid.get(&k).into_iter().flatten().copied()).collect();
            near.sort_unstable();
            near.dedup();
            // (The edges whose nearest point to the edge's middle lies in the box: 0.6 m to either side, 0.6 m back or on,
            // 0.5 m up or down.)
            let boxed: Vec<&Authored> = near
                .into_iter()
                .map(|i| edges[i])
                .filter(|e| {
                    let d = line(e).closest(mid) - mid;
                    d.dot(r).abs() <= 0.6 && d.dot(f).abs() <= 0.6 && d.y.abs() <= 0.5
                })
                .collect();
            let find = |from: Vec3, d: Vec3, max: f32| -> Option<Vec3> {
                boxed
                    .iter()
                    .filter(|e| e.out.with_y(0.0).normalize_or_zero().dot(d) >= std::f32::consts::FRAC_PI_4.cos())
                    .map(|e| line(e).closest(from))
                    .filter(|q| {
                        let w = (*q - from).with_y(0.0);
                        w.length() < max && (q.y - from.y).abs() <= 0.5 && w.normalize_or(d).dot(d) >= std::f32::consts::FRAC_PI_6.cos()
                    })
                    .min_by(|a, b| flat(*a, from).total_cmp(&flat(*b, from)))
            };
            let back = find(mid, -f, 0.3)?;
            let front = find(mid, f, 1.3)?;
            let m = (back + front) * 0.5;
            let left = find(m, -r, 0.6)?;
            let right = find(m, r, 0.6)?;
            let near = [back, front, left, right].iter().map(|q| flat(*q, mid)).fold(f32::INFINITY, f32::min);
            if near > 0.375 || flat(back, front) > 0.7 || flat(left, right) > 0.7 {
                return None;
            }
            let top = (back + front + left + right) * 0.25;
            self.capsule_free(top + Vec3::Y * 0.6, top + Vec3::Y * 1.4, 0.4).then_some(top)
        });
        let mut tops: Vec<Vec3> = vec![];
        for t in found.into_iter().flatten() {
            if !tops.iter().any(|q| q.distance(t) < 0.1) {
                tops.push(t);
            }
        }
        // (A paired beam lying on a post's top, within its 0.7 m: the post's own.)
        self.perches.retain(|l| !tops.iter().any(|t| l.a.distance(*t) < 0.5 && l.b.distance(*t) < 0.5));
        let n = tops.len();
        self.perches.extend(tops.into_iter().map(|t| Line { a: t, b: t }));
        n
    }

    /// The half-width of the beam whose top line passes `q` along `axis` (m): how far its top goes on to either side, level
    /// with `q`, probed every 5 cm up to AC1's widest beam (0.5 m); the wider side.
    pub fn beam_half_width(&self, q: Vec3, axis: Vec3) -> f32 {
        let side = axis.with_y(0.0).normalize_or_zero().cross(Vec3::Y);
        let on = |p: Vec3| self.ground(p + Vec3::Y * 0.1, 0.0, 0.2).is_some_and(|g| (g.point.y - q.y).abs() < 0.08);
        [side, -side].iter().map(|s| (1..=10).map(|k| k as f32 * 0.05).take_while(|&d| on(q + *s * d)).last().unwrap_or(0.0)).fold(0.0, f32::max)
    }

    /// May a beam be caught at `q` from the air (AC1's `sub_116D960`, the beam test of the air catch 0xE0B890 and of the
    /// free-step arrival; read in Ghidra, as Banned445's port has it): `width + 0.2` m out to either side, a ray from 0.5 m
    /// over the top 1.2 m down meets nothing within 0.5 m under the top (no floor beside it: a beam in a roof is landed on as
    /// ground), and a ray across from one side to the other 0.2 m over the top meets nothing (no wall beside it). A post
    /// (`axis` zero) has its own room test (`add_pilotis`).
    pub fn beam_catch_clear(&self, q: Vec3, axis: Vec3) -> bool {
        let axis = axis.with_y(0.0).normalize_or_zero();
        if axis == Vec3::ZERO {
            return true;
        }
        let side = axis.cross(Vec3::Y) * (self.beam_half_width(q, axis) + 0.2);
        [side, -side].iter().all(|s| self.raycast(q + *s + Vec3::Y * 0.5, Vec3::NEG_Y, 1.2).is_none_or(|h| h.dist - 0.5 >= 0.5))
            && self.raycast(q - side + Vec3::Y * 0.2, side.normalize(), side.length() * 2.0).is_none()
    }

    /// No face within `r` of the segment `a`-`b` (a capsule's room).
    pub fn capsule_free(&self, a: Vec3, b: Vec3, r: f32) -> bool {
        let pad = Vec3::splat(r);
        self.tris_near(a.min(b) - pad, a.max(b) + pad).into_iter().all(|i| {
            let (p, q) = segment_triangle(a, b, &self.tris[i as usize]);
            p.distance_squared(q) >= r * r
        })
    }

    /// A sphere of radius `r` moved from `from` to `to`, through fuzzy zones (`fuzzy_tris`) as AC1's camera goes (its
    /// eye stayed 3.5 m out through the bureau courtyard's palm, live 2026-10-23): its centre where it first touches a
    /// face (found by halving the way, to about 1 mm on a 4 m cast), `from` when it starts touching one, None when the
    /// way is clear.
    pub fn sphere_cast(&self, from: Vec3, to: Vec3, r: f32) -> Option<Vec3> {
        let free = |b: Vec3| {
            let pad = Vec3::splat(r);
            self.tris_near(from.min(b) - pad, from.max(b) + pad).into_iter().all(|i| {
                let (p, q) = segment_triangle(from, b, &self.tris[i as usize]);
                p.distance_squared(q) >= r * r || self.is_fuzzy(i)
            })
        };
        if free(to) {
            return None;
        }
        if !free(from) {
            return Some(from);
        }
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for _ in 0..12 {
            let mid = (lo + hi) * 0.5;
            if free(from.lerp(to, mid)) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Some(from.lerp(to, lo))
    }

    /// Each ladder's top as the walkable top it leads onto (a probe down just in from its top), not its rails' top: the
    /// rails stood 0.33 m over ladder 68's roof and ours climbed a step too many (audit LADDER-3). Returns how many moved.
    pub fn settle_ladder_tops(&mut self) -> usize {
        let mut moved = 0;
        for i in 0..self.ladders.len() {
            let l = self.ladders[i];
            let p = l.at(l.top + 0.5) - l.out * LADDER_TOP_IN;
            if let Some(g) = self.ground(p, 0.0, 1.5).filter(|g| g.normal.y > 0.7 && (g.point.y - l.top).abs() > 0.05) {
                self.ladders[i].top = g.point.y;
                moved += 1;
            }
        }
        moved
    }

    /// Carve the holds out round each ladder, as AC1's hold reports do (BuildLedgeReport, Banned445's port and reanalysis
    /// GUIDE-2: a box 0.3 m either side of the ladder and 0.75 m out and in from it): a ledge crossing a ladder keeps
    /// its parts either side. Returns how many pieces were cut.
    pub fn carve_ladders(&mut self) -> usize {
        // (AC1's holds are cut round its ladders' own zones already: `ac1_holds`.)
        if self.authored_holds {
            return 0;
        }
        // (Each ledge through the ladders in turn, its pieces on through the ladders after the one that cut it: the same
        // pieces in the same order as ladder by ladder over all the ledges, without copying the 190,000 Damascus holds
        // once per ladder, 2 s of its load.)
        let cuts: Vec<(f32, f32, Vec3)> =
            self.ladders.iter().map(|lad| (lad.base.y - 0.5, lad.top + 0.5, lad.out.cross(Vec3::Y).normalize_or_zero())).collect();
        let carve = |l: &Ledge, k: usize| -> Option<Vec<Ledge>> {
            let (lad, (lo, hi, side)) = (&self.ladders[k], cuts[k]);
            if l.a.y.max(l.b.y) < lo || l.a.y.min(l.b.y) > hi {
                return None;
            }
            // The part of the ledge inside the box, as an interval of its parameter.
            let (pa, pb) = (l.a - lad.base, l.b - lad.base);
            let mut range = (0.0f32, 1.0f32);
            for (axis, half) in [(side, LADDER_CARVE_SIDE), (lad.out, LADDER_CARVE_OUT)] {
                let (u, v) = (pa.dot(axis), pb.dot(axis));
                if (v - u).abs() < 1e-6 {
                    if u.abs() > half {
                        range = (1.0, 0.0);
                    }
                    continue;
                }
                let (t0, t1) = ((-half - u) / (v - u), (half - u) / (v - u));
                range = (range.0.max(t0.min(t1)), range.1.min(t0.max(t1)));
            }
            if range.0 >= range.1 {
                return None;
            }
            let at = |t: f32| l.a.lerp(l.b, t);
            let len = l.a.distance(l.b);
            let mut pieces = vec![];
            if range.0 * len > 0.05 {
                pieces.push(Ledge { a: l.a, b: at(range.0), out: l.out });
            }
            if (1.0 - range.1) * len > 0.05 {
                pieces.push(Ledge { a: at(range.1), b: l.b, out: l.out });
            }
            Some(pieces)
        };
        let mut cut = 0;
        let mut kept = Vec::with_capacity(self.ledges.len());
        // (A piece and the first ladder still to cut it, in order.)
        let mut todo: Vec<(Ledge, usize)> = vec![];
        for l in &self.ledges {
            todo.push((*l, 0));
            while let Some((piece, from)) = todo.pop() {
                match (from..cuts.len()).find_map(|k| carve(&piece, k).map(|p| (k, p))) {
                    Some((k, pieces)) => {
                        cut += 1;
                        // (Reversed onto the stack: the first piece goes on first.)
                        todo.extend(pieces.into_iter().rev().map(|p| (p, k + 1)));
                    }
                    None => kept.push(piece),
                }
            }
        }
        self.ledges = kept;
        cut
    }

    /// Leave out the holds and beams found on a haystack's heap (over its footprint, above its rim): the hay is solid in
    /// the collision built from its mesh but is sunk into, not climbed; ours climbed from the rim to the heap's top.
    /// Not AC1's authored holds: its hay has none, and those over a hiding place are its tarp frame's top, a ledge grab
    /// in AC1's markup (Damascus: 1,349 pieces were dropped, on the frames standing square to the city's axes only).
    /// Returns how many.
    pub fn drop_hay_holds(&mut self) -> usize {
        let hay = self.haystacks.clone();
        // (A hold on the heap faces out from its middle; a wall's beside the hay faces toward it, and stays.)
        let on_heap = |p: Vec3, margin: f32, out: Option<Vec3>| {
            hay.iter().any(|h| {
                h.contains(p, -margin)
                    && (HAY_HEAP_ABOVE..HAY_HEAP_TOP).contains(&(p.y - h.top()))
                    && out.is_none_or(|o| (p - h.centre).with_y(0.0).dot(o) > 0.0)
            })
        };
        let (ledges, perches) = (self.ledges.len(), self.perches.len());
        if !self.authored_holds {
            self.ledges.retain(|l| !(on_heap(l.a, HAY_HEAP_MARGIN, Some(l.out)) && on_heap(l.b, HAY_HEAP_MARGIN, Some(l.out))));
        }
        self.perches.retain(|l| !(on_heap(l.a, 0.0, None) && on_heap(l.b, 0.0, None)));
        ledges - self.ledges.len() + perches - self.perches.len()
    }

    /// The index of an authored edge's source (`Authored::src`): the group or entity placing it and that entity's first
    /// mesh, with the digits ending a placed name left off (`Kiosk_Carpets_008` is a `Kiosk_Carpets`).
    pub fn authored_source(&mut self, owner: &str, mesh: &str) -> u32 {
        let trim = |s: &str| s.trim_end_matches(|c: char| c.is_ascii_digit() || c == '_').to_string();
        let key = format!("{}\t{}", trim(owner), trim(mesh));
        if let Some(&i) = self.authored_src_at.get(&key) {
            return i;
        }
        let i = self.authored_src.len() as u32;
        self.authored_src.push(key.clone());
        self.authored_src_at.insert(key, i);
        i
    }

    /// Use AC1's own climbing markup where the city has it: its ledge grabs become the holds (instead of the edges and
    /// lips found in the geometry; made by `ac1_holds` once the collision is built), its ladders the ladders, its
    /// horizontal poles the swing bars (each pole's two side edges as one bar). `AC1_GEOMETRY_HOLDS` keeps the geometry's
    /// holds. Returns (ledge grabs, ladders, bars), or None.
    pub fn use_authored(&mut self) -> Option<(usize, usize, usize)> {
        if self.authored.is_empty() || std::env::var("AC1_GEOMETRY_HOLDS").is_ok() {
            return None;
        }
        use forge::guidance::SubType;
        let (mut holds, mut ladders, mut bars) = (0, 0, 0);
        for e in self.authored.clone() {
            let len = e.a.distance(e.b);
            match e.kind {
                SubType::LedgeGrab => holds += 1,
                SubType::Ladder => {
                    let (lo, hi) = if e.a.y < e.b.y { (e.a, e.b) } else { (e.b, e.a) };
                    if e.out != Vec3::ZERO {
                        let rise = (hi.y - lo.y).max(0.01);
                        let lean = (hi - lo).with_y(0.0) / rise;
                        // (Leaning over its climbing side by 10 degrees or more, AC1 takes the climber round to its other
                        // side, or lets go (Banned445's ladder.rs, reanalysis LADDER-4): ours climbs that side. Masyaf
                        // has one, 11 degrees.)
                        let out = if lean.dot(e.out) >= 10f32.to_radians().tan() { -e.out } else { e.out };
                        self.ladders.push(Ladder { base: lo, top: hi.y, out, lean });
                        ladders += 1;
                    }
                }
                SubType::Kiosk if e.out != Vec3::ZERO && len > 0.01 => {
                    self.kiosks.push(Kiosk { edge: Ledge { a: e.a, b: e.b, out: e.out }, axis: e.axis });
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
        // (The holds are made from them once the collision is built: `ac1_holds`.)
        self.ledges.clear();
        self.authored_ladders = ladders > 0;
        self.authored_holds = true;
        Some((holds, ladders, bars))
    }

    /// How many authored ledge pieces end at each vertex (to the millimetre): a piece goes on into another there.
    pub(crate) fn authored_ends(&self) -> HashMap<(i64, i64, i64), u32> {
        let mut ends = HashMap::new();
        for e in self.authored.iter().filter(|e| e.kind == forge::guidance::SubType::LedgeGrab) {
            *ends.entry(vertex_key(e.a)).or_default() += 1;
            *ends.entry(vertex_key(e.b)).or_default() += 1;
        }
        ends
    }

    /// The kiosk piece `p` lies on, within `within` (m).
    pub fn kiosk_at(&self, p: Vec3, within: f32) -> Option<Kiosk> {
        self.kiosks.iter().find(|k| k.edge.closest(p).distance(p) <= within).copied()
    }

    /// AC1's ladder zones (`LadderZone`), from the authored ladder edges.
    pub(crate) fn ladder_zones(&self) -> Vec<LadderZone> {
        self.authored.iter().filter(|e| e.kind == forge::guidance::SubType::Ladder).filter_map(LadderZone::of).collect()
    }

    /// The holds an authored ledge grab gives under AC1's rules, as its climbing grid and ledge report take the guidance
    /// (`HumanClimb__BuildHoldGrid` 0xDF6A40, `crate::guide_depth`): none past 40 degrees or with no wall to face out
    /// from; cut out of the ladders' zones (0x1170880); then only the runs with the depth detector's metre of room in front
    /// (0xB27E80); each piece shortened as `authored_piece`. Needs the grid built. Returns the fate, the holds and the
    /// length kept before shortening (m).
    pub(crate) fn ac1_hold_pieces(&self, e: &Authored, zones: &[LadderZone], ends: &HashMap<(i64, i64, i64), u32>) -> (HoldFate, Vec<Ledge>, f32) {
        self.ac1_pieces_by(e, zones, &[crate::guide_depth::LEDGE], ends)
    }

    /// `ac1_hold_pieces` with the zones cut out and the depth detectors a search of AC1's uses (`HoldView`).
    pub(crate) fn ac1_pieces_by(
        &self,
        e: &Authored,
        zones: &[LadderZone],
        detectors: &[crate::guide_depth::Detector],
        ends: &HashMap<(i64, i64, i64), u32>,
    ) -> (HoldFate, Vec<Ledge>, f32) {
        if authored_steep(e) {
            return (HoldFate::Steep, vec![], 0.0);
        }
        if e.out.with_y(0.0) == Vec3::ZERO {
            return (HoldFate::NoWall, vec![], 0.0);
        }
        let len = e.a.distance(e.b);
        // (Pieces as metres from its start along it.)
        let mut pieces = vec![(0.0f32, len)];
        for z in zones {
            let mut next = Vec::with_capacity(pieces.len());
            for (p0, p1) in pieces {
                let (a, b) = (e.a.lerp(e.b, p0 / len), e.a.lerp(e.b, p1 / len));
                match z.cut(a, b) {
                    Some((t0, t1)) => {
                        let span = p1 - p0;
                        if t0 > 1e-4 {
                            next.push((p0, p0 + span * t0));
                        }
                        if t1 < 1.0 - 1e-4 {
                            next.push((p0 + span * t1, p1));
                        }
                    }
                    None => next.push((p0, p1)),
                }
            }
            pieces = next;
        }
        if pieces.is_empty() {
            return (HoldFate::Ladder, vec![], 0.0);
        }
        let mut free = vec![];
        for (p0, p1) in pieces {
            let (a, b) = (e.a.lerp(e.b, p0 / len), e.a.lerp(e.b, p1 / len));
            free.extend(crate::guide_depth::free_pieces(self, a, b, e.out, detectors).into_iter().map(|(f, t)| (p0 + f, p0 + t)));
        }
        if free.is_empty() {
            return (HoldFate::Depth, vec![], 0.0);
        }
        let kept = free.iter().map(|(f, t)| t - f).sum();
        (HoldFate::Kept, free.into_iter().map(|(f, t)| authored_piece(e, f, t, ends)).collect(), kept)
    }

    /// The holds of a city with authored guidance: every ledge grab under AC1's rules (`ac1_hold_pieces`). Needs the grid
    /// built. Returns how many ledge grabs gave none, by the rule that took them: (steep or no wall, ladder, depth).
    pub fn ac1_holds(&mut self) -> (usize, usize, usize) {
        let zones = self.ladder_zones();
        let ends = self.authored_ends();
        let grabs: Vec<&Authored> = self.authored.iter().filter(|e| e.kind == forge::guidance::SubType::LedgeGrab).collect();
        let each = par_map(&grabs, |e| self.ac1_hold_pieces(e, &zones, &ends));
        let (mut steep, mut ladder, mut depth) = (0, 0, 0);
        let mut ledges = vec![];
        for (fate, holds, _) in each {
            match fate {
                HoldFate::Steep | HoldFate::NoWall => steep += 1,
                HoldFate::Ladder => ladder += 1,
                HoldFate::Depth => depth += 1,
                HoldFate::Kept => {}
            }
            ledges.extend(holds);
        }
        self.ledges = ledges;
        (steep, ladder, depth)
    }

    /// AC1's other searches' views of the ledge grabs (`HoldView`: the air catch's, the jump query's, the pull-down's),
    /// each by its own report's rules: steep ones out as for the grid, the ladders' zones cut only by the ledge reports
    /// (the pull-down's), then that search's depth detectors (`crate::guide_depth`). Needs the grid built. Returns how
    /// many holds each has.
    pub fn ac1_views(&mut self) -> [usize; 3] {
        use crate::guide_depth::{CATCH_HANDS, JUMP, PULL_DOWN};
        let zones = self.ladder_zones();
        let ends = self.authored_ends();
        let grabs: Vec<&Authored> = self.authored.iter().filter(|e| e.kind == forge::guidance::SubType::LedgeGrab).collect();
        let sets: [(&[LadderZone], &[crate::guide_depth::Detector]); 3] = [(&[], &CATCH_HANDS), (&[], &JUMP), (&zones, &[PULL_DOWN])];
        let views = sets.map(|(z, d)| par_map(&grabs, |e| self.ac1_pieces_by(e, z, d, &ends).1).into_iter().flatten().collect::<Vec<_>>());
        self.views = views;
        [0, 1, 2].map(|i| self.views[i].len())
    }

    /// The holds the search `view` sees (`HoldView`); the grid's where there are no views (no authored guidance).
    pub fn holds_of(&self, view: HoldView) -> &[Ledge] {
        let v = &self.views[view.slot()];
        if v.is_empty() { &self.ledges } else { v }
    }

    /// Where the holds are AC1's: leave out the perches with no authored ledge grab along them (within `BACKED_ACROSS`
    /// across and `BACKED_UP` up or down of their middle: AC1's beams are pairs of its ledge grabs) and the holds lying
    /// on none (within 5 cm): the beam perches and beam-end holds made from `WoodBeam_*` meshes where AC1 marks nothing
    /// (Damascus: 24 perches on beams lying in the street 0.1-0.2 m up, 16 beam-end holds). Returns (perches, holds).
    pub fn drop_unbacked(&mut self) -> (usize, usize) {
        use forge::guidance::SubType;
        let grabs: Vec<&Authored> = self.authored.iter().filter(|e| e.kind == SubType::LedgeGrab).collect();
        let mut grid: FastMap<(i32, i32), Vec<u32>> = FastMap::default();
        for (i, e) in grabs.iter().enumerate() {
            let (lo, hi) = (e.a.min(e.b) - BACKED_ACROSS, e.a.max(e.b) + BACKED_ACROSS);
            for x in lo.x.floor() as i32..=hi.x.floor() as i32 {
                for z in lo.z.floor() as i32..=hi.z.floor() as i32 {
                    grid.entry((x, z)).or_default().push(i as u32);
                }
            }
        }
        let backed = |p: Vec3, across: f32, up: f32, out: Option<Vec3>| {
            grid.get(&(p.x.floor() as i32, p.z.floor() as i32)).into_iter().flatten().map(|&i| grabs[i as usize]).any(|e| {
                let q = Line { a: e.a, b: e.b }.closest(p);
                (q - p).with_y(0.0).length() < across && (q.y - p.y).abs() < up && out.is_none_or(|o| e.out.dot(o) > 0.7)
            })
        };
        let keep_perch: Vec<bool> = self.perches.iter().map(|l| backed((l.a + l.b) * 0.5, BACKED_ACROSS, BACKED_UP, None)).collect();
        let keep_hold: Vec<bool> = self.ledges.iter().map(|l| backed((l.a + l.b) * 0.5, 0.05, 0.05, Some(l.out))).collect();
        let (p0, h0) = (self.perches.len(), self.ledges.len());
        let mut k = keep_perch.into_iter();
        self.perches.retain(|_| k.next().unwrap_or(true));
        let mut k = keep_hold.into_iter();
        self.ledges.retain(|_| k.next().unwrap_or(true));
        (p0 - self.perches.len(), h0 - self.ledges.len())
    }

    pub fn add_beam_perches(&mut self) {
        // (Not where the beam runs inside a wall it is stuck into, even if its top shows there.)
        let open = |p: Vec3| self.ground(p + Vec3::Y * 1.9, 0.0, 1.98).is_some_and(|h| (h.point.y - p.y).abs() < 0.08) && !self.inside_solid(p + Vec3::Y * 0.5);
        // (Each beam on its own core, in order.)
        let each = par_map(&self.beam_tops, |l| {
            let mut found = vec![];
            let mut holds = vec![];
            let len = (l.b - l.a).length();
            if len < 0.2 || (l.b.y - l.a.y).abs() > 0.15 * len {
                return (found, holds);
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
            (found, holds)
        });
        let (mut found, mut holds) = (vec![], vec![]);
        for (f, h) in each {
            found.extend(f);
            holds.extend(h);
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
        // (A beam too short to keep 0.3 m from both ends is a post: a 0.57 m block on a Damascus roof, which the running
        // game jumped onto and balanced on, could never be got onto.)
        self.perch_at(p, r).filter(|(i, q)| {
            let l = &self.perches[*i];
            l.axis() == Vec3::ZERO || (l.b - l.a).length() < 0.6 || ((*q - l.a).length() > 0.3 && (*q - l.b).length() > 0.3)
        })
    }

    /// Ground under `p`, searching from `up` above it down to `down` below.
    pub fn ground(&self, p: Vec3, up: f32, down: f32) -> Option<Hit> {
        self.raycast(p + Vec3::Y * up, Vec3::NEG_Y, up + down)
    }

    /// The sound material of the ground under `p` (the footstep events' switch 0, see `sound`): the nearest triangle a
    /// ray down from 0.5 m over it meets within 1.5 m. `None` where that surface has none (the test level).
    pub fn sound_material_at(&self, p: Vec3) -> Option<u32> {
        if self.tri_sound.is_empty() {
            return None;
        }
        let origin = p + Vec3::Y * 0.5;
        let mut best: Option<(f32, u32)> = None;
        for i in self.tris_near(p - Vec3::new(0.01, 1.0, 0.01), origin + Vec3::new(0.01, 0.0, 0.01)) {
            if let Some((h, _, _)) = self.raycast_tris(std::iter::once((i, &self.tris[i as usize])), origin, Vec3::NEG_Y, 1.5)
                && best.is_none_or(|b| h.dist < b.0)
            {
                best = Some((h.dist, i));
            }
        }
        best.and_then(|(_, i)| self.tri_sound.get(i as usize)).filter(|&&m| m != NO_SOUND).map(|&m| m as u32)
    }

    /// The triangles that may touch the box `lo`..`hi` (each once).
    pub(crate) fn tris_near(&self, lo: Vec3, hi: Vec3) -> Vec<u32> {
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

    /// Nothing in the oriented box at `centre` with unit `axes` and `half` extents along them: no face crosses it (AC1's
    /// box queries, 0xB2D2A0, under `Human__ClimbBodyBoxesFree` 0xB2DF90 and the climb-out's room test 0xB2E240). A face
    /// only touching a side (within 1 mm) does not count.
    pub fn box_free(&self, centre: Vec3, axes: [Vec3; 3], half: Vec3) -> bool {
        self.box_free_but(centre, axes, half, None)
    }

    /// The faces crossing the oriented box (as `box_free` tests it): for tracing why a box is not free.
    pub fn box_hits(&self, centre: Vec3, axes: [Vec3; 3], half: Vec3) -> Vec<Tri> {
        let reach = axes[0].abs() * half.x + axes[1].abs() * half.y + axes[2].abs() * half.z;
        let local = |p: Vec3| {
            let d = p - centre;
            Vec3::new(d.dot(axes[0]), d.dot(axes[1]), d.dot(axes[2]))
        };
        let half = (half - Vec3::splat(1e-3)).max(Vec3::ZERO);
        self.tris_near(centre - reach, centre + reach)
            .into_iter()
            .map(|i| self.tris[i as usize])
            .filter(|t| tri_crosses_box([local(t.a), local(t.b), local(t.c)], half))
            .collect()
    }

    /// `box_free`, the faces lying wholly inside the box `ignore` (world, min and max) left out.
    pub fn box_free_but(&self, centre: Vec3, axes: [Vec3; 3], half: Vec3, ignore: Option<(Vec3, Vec3)>) -> bool {
        let inside = |p: Vec3| ignore.is_some_and(|(lo, hi)| p.cmpge(lo - Vec3::splat(0.05)).all() && p.cmple(hi + Vec3::splat(0.05)).all());
        let reach = axes[0].abs() * half.x + axes[1].abs() * half.y + axes[2].abs() * half.z;
        let near = self.tris_near(centre - reach, centre + reach);
        let local = |p: Vec3| {
            let d = p - centre;
            Vec3::new(d.dot(axes[0]), d.dot(axes[1]), d.dot(axes[2]))
        };
        let half = (half - Vec3::splat(1e-3)).max(Vec3::ZERO);
        !near.iter().any(|&i| {
            let t = &self.tris[i as usize];
            !(inside(t.a) && inside(t.b) && inside(t.c)) && tri_crosses_box([local(t.a), local(t.b), local(t.c)], half)
        })
    }

    /// Room to hop out of haystack `hs` from `p` (in it) along `d` (flat, unit): AC1's `Guard_HopOut` 0xE434E0 (Banned445's
    /// port, audit HAY-6): the box `HAY_HOP_BOX` (half extents across, along, up) `HAY_HOP_ROOM` past where the way out
    /// leaves the footprint, `HAY_HOP_UP` over the root, empty, the haystack itself left out (AC1's query skips it): its
    /// own bounds (`HayStack::own`), a cart's bed, a hiding place's plank box (3.1 m across in some, past the footprint)
    /// and the tarp's frame over it.
    pub fn hay_hop_room(&self, hs: &HayStack, p: Vec3, d: Vec3) -> bool {
        let reach = |dc: f32, pc: f32, cc: f32| if dc.abs() < 1e-4 { f32::INFINITY } else { (cc + hs.half * dc.signum() - pc) / dc };
        let t = reach(d.x, p.x, hs.centre.x).min(reach(d.z, p.z, hs.centre.z)).max(0.0);
        // (Over what he sits on: a cart's bed stands over the hay entity's own height, where ours seats him.)
        let base = self.ground(p + Vec3::Y * hs.height, 0.0, hs.height + 0.5).map_or(p.y, |g| g.point.y.max(p.y));
        let room = (p + d * (t + HAY_HOP_ROOM)).with_y(base + HAY_HOP_UP);
        // (Or, as ours had it before, a clear way out over its top from over its middle: a pile in a yard, its walls 2.5 m
        // round its middle and its own shape not told apart, has room only by that.)
        let over = Vec3::new(hs.centre.x, p.y.max(hs.top()), hs.centre.z) + Vec3::Y * HAY_HOP_UP;
        let out = t + HAY_HOP_ROOM;
        self.box_free_but(room, [d.cross(Vec3::Y), d, Vec3::Y], HAY_HOP_BOX, hs.own)
            || (self.raycast(over, d, out).is_none() && !self.inside_solid(over + d * out))
    }

    /// The haystacks and hiding places with no way out: from the seat, no room to hop out any of 8 ways (`hay_hop_room`).
    pub fn hay_with_no_way_out(&self) -> Vec<Vec3> {
        self.haystacks
            .iter()
            .filter(|h| {
                (0..8).all(|k| {
                    let a = k as f32 * std::f32::consts::FRAC_PI_4;
                    !self.hay_hop_room(h, h.seat(), Vec3::new(a.cos(), 0.0, a.sin()))
                })
            })
            .map(|h| h.centre)
            .collect()
    }

    /// Narrow wall tops (a fence's or a parapet's, no deeper than `NARROW_TOP_MAX` behind a hold, with a drop past
    /// it and open above) become perches along their middle: climbing onto one ends balancing on it, as on a beam,
    /// not standing on a top too narrow to stand on. Holds next to each other along a top make one perch.
    pub fn add_narrow_tops(&mut self) -> usize {
        let key = |p: Vec3| ((p.x / 0.5).floor() as i32, (p.y / 0.5).floor() as i32, (p.z / 0.5).floor() as i32);
        let near = |cells: &FastMap<(i32, i32, i32), Vec<usize>>, p: Vec3| -> Vec<usize> {
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
        let mut centres: FastMap<(i32, i32, i32), Vec<usize>> = FastMap::default();
        // Each hold's top, probed on all cores (each reads the level only; 151,000 holds in Damascus took 4.8 s on one),
        // in the holds' order: (the line along the top's middle, room above it).
        let narrow = |l: &Ledge| -> Vec<(Line, bool)> {
            let mid = (l.a + l.b) * 0.5;
            let len = l.a.distance(l.b);
            if len < 0.3 {
                return vec![];
            }
            let level_with = |d: f32| self.ground(mid - l.out * d + Vec3::Y * 0.3, 0.0, 0.45).is_some_and(|g| (g.point.y - mid.y).abs() < 0.08);
            let Some(depth) = (1..=(NARROW_TOP_MAX / 0.05) as usize + 1).map(|k| k as f32 * 0.05).find(|&d| !level_with(d)) else { return vec![] };
            if !(0.15..=NARROW_TOP_MAX).contains(&depth) {
                return vec![];
            }
            // (Past it a drop: not the step up to a roof. Along all of it, every `PAIR_BEAM_STEP`, the top kept only where
            // it drops past: tested at the middle alone, a roof edge's strip over a drop for half its length was a top to
            // balance on its whole length, the roof flush past the rest; the running game stepped off it there.)
            // (A roof's parapet, the roof just under it on one side, stays a top: the running game jumps onto one as a
            // free-step target off the bureau's roof, audit JUMP-3; walking or standing by it never gets onto it as a beam
            // (`locomotion`'s ground-to-perch rule), user report #12.)
            let drops = |s: f32| self.ground(l.a.lerp(l.b, s) - l.out * (depth + 0.1) + Vec3::Y * 0.3, 0.0, 0.8).is_none();
            let n = (len / PAIR_BEAM_STEP).ceil().max(1.0) as usize;
            let shift = -l.out * (depth * 0.5);
            let mut out = vec![];
            let mut run: Option<usize> = None;
            for k in 0..=n + 1 {
                let ok = k <= n && drops(k as f32 / n as f32);
                match (ok, run) {
                    (true, None) => run = Some(k),
                    (false, Some(a)) => {
                        let (sa, sb) = (a as f32 / n as f32, (k - 1) as f32 / n as f32);
                        if (sb - sa) * len >= 0.3 {
                            let line = Line { a: l.a.lerp(l.b, sa) + shift, b: l.a.lerp(l.b, sb) + shift };
                            let centre = (line.a + line.b) * 0.5;
                            let open = self.ground(centre + Vec3::Y * 1.9, 0.0, 1.98).is_some_and(|h| (h.point.y - centre.y).abs() < 0.08);
                            out.push((line, open && !self.inside_solid(centre + Vec3::Y * 0.5)));
                        }
                        run = None;
                    }
                    _ => {}
                }
            }
            out
        };
        let tops: Vec<(Line, bool)> = par_map(&self.ledges, narrow).into_iter().flatten().collect();
        for (line, open) in tops {
            let centre = (line.a + line.b) * 0.5;
            // (The hold on the far side of the same top finds it again.)
            if near(&centres, centre).iter().any(|&i| found[i].closest(centre).distance(centre) < 0.2) || !open {
                continue;
            }
            centres.entry(key(centre)).or_default().push(found.len());
            found.push(line);
        }
        // AC1's runtime beams (`HumanGuidance`'s beam contacts, 0x680260, as Banned445's AC1-Movement-Rewritten reads
        // them, MIT, written here anew): two holds along each other (within 30 degrees), facing away from each other,
        // overlapping, at most `PAIR_BEAM_WIDTH` apart across: a beam along their middle. (A plank bridge 0.9 m wide in
        // Damascus: the running game walked it as a narrow object, crouched.)
        let mut along: FastMap<(i32, i32, i32), Vec<usize>> = FastMap::default();
        for (i, l) in self.ledges.iter().enumerate() {
            let n = (l.a.distance(l.b) / 0.5).ceil().max(1.0) as usize;
            for k in 0..=n {
                along.entry(key(l.a.lerp(l.b, k as f32 / n as f32))).or_default().push(i);
            }
        }
        let mut pairs: Vec<Line> = vec![];
        for (i, l) in self.ledges.iter().enumerate() {
            let len = l.a.distance(l.b);
            if len < 0.3 {
                continue;
            }
            let u = (l.b - l.a) / len;
            let n = (len / 0.5).ceil() as usize;
            let mut others: Vec<usize> = (0..=n).flat_map(|k| near(&along, l.a.lerp(l.b, k as f32 / n as f32))).filter(|&j| j > i).collect();
            others.sort_unstable();
            others.dedup();
            for j in others {
                let m = &self.ledges[j];
                let mlen = m.a.distance(m.b);
                if mlen < 0.3 || l.out.dot(m.out) > -PAIR_BEAM_COS {
                    continue;
                }
                let v = (m.b - m.a) / mlen;
                if u.dot(v).abs() < PAIR_BEAM_COS {
                    continue;
                }
                // Across: the other hold behind this one (on its top's side), at most the width away, level with it.
                let across = (m.closest(l.a.lerp(l.b, 0.5)) - l.a.lerp(l.b, 0.5)).dot(-l.out);
                if !(0.1..=PAIR_BEAM_WIDTH).contains(&across) || (m.a.y + m.b.y - l.a.y - l.b.y).abs() * 0.5 > 0.3 {
                    continue;
                }
                // Along: the overlap of the two, measured on this one.
                let (s0, s1) = ((m.a - l.a).dot(u), (m.b - l.a).dot(u));
                let (lo, hi) = (s0.min(s1).max(0.0), s0.max(s1).min(len));
                if hi - lo < 0.3 {
                    continue;
                }
                let mid = |s: f32| {
                    let p = l.a + u * s;
                    (p + m.closest(p)) * 0.5
                };
                // (A drop past both edges, as off the Damascus plank bridge (8 m): not a strip or a seam in a roof, the
                // floor going on level either side, where running across one he was caught balancing on it.)
                let drop = |h: &Ledge, at: Vec3| self.ground(at + h.out * PAIR_BEAM_OUT + Vec3::Y * 0.1, 0.0, PAIR_BEAM_DROP).is_none();
                // (Along all of it, every `PAIR_BEAM_STEP`: only the stretches with the drop both sides. Tested at the
                // middle alone, a roof edge's strip over a drop for half its length was a beam its whole length, the roof
                // flush either side of the rest; the running game stepped off it there.)
                let ok = |s: f32| {
                    let at = l.a + u * s;
                    drop(l, at) && drop(m, m.closest(at))
                };
                let n = ((hi - lo) / PAIR_BEAM_STEP).ceil().max(1.0) as usize;
                let mut run: Option<f32> = None;
                for k in 0..=n {
                    let s = lo + (hi - lo) * k as f32 / n as f32;
                    match (ok(s), run) {
                        (true, None) => run = Some(s),
                        (false, Some(a)) => {
                            let b = lo + (hi - lo) * (k - 1) as f32 / n as f32;
                            if b - a >= 0.3 {
                                pairs.push(Line { a: mid(a), b: mid(b) });
                            }
                            run = None;
                        }
                        _ => {}
                    }
                }
                if let Some(a) = run
                    && hi - a >= 0.3
                {
                    pairs.push(Line { a: mid(a), b: mid(hi) });
                }
            }
        }
        // (The holds either side are broken at different places: the pieces of one beam, in line and at most
        // `PAIR_BEAM_GAP` apart over the same top, are one beam. In pieces, he stepped off at every gap onto "ground".)
        let mut merged = true;
        while merged {
            merged = false;
            let mut ends: FastMap<(i32, i32, i32), Vec<usize>> = FastMap::default();
            for (i, l) in pairs.iter().enumerate() {
                ends.entry(key(l.a)).or_default().push(i);
                ends.entry(key(l.b)).or_default().push(i);
            }
            let mut gone = vec![false; pairs.len()];
            for i in 0..pairs.len() {
                if gone[i] {
                    continue;
                }
                let l = pairs[i];
                let axis = l.axis();
                let cands: Vec<usize> = near(&ends, l.a).into_iter().chain(near(&ends, l.b)).filter(|&j| j != i && !gone[j]).collect();
                for j in cands {
                    let m = pairs[j];
                    let in_line = m.axis().dot(axis).abs() > 0.97
                        && [m.a, m.b].iter().all(|p| Line { a: l.a - axis * 50.0, b: l.b + axis * 50.0 }.closest(*p).distance(*p) < 0.15);
                    let (sa, sb) = ((m.a - l.a).dot(axis), (m.b - l.a).dot(axis));
                    let (mlo, mhi) = (sa.min(sb), sa.max(sb));
                    let len = l.a.distance(l.b);
                    let gap = (mlo - len).max(-mhi);
                    let at = l.a + axis * if mlo > len { (len + mlo) * 0.5 } else { mhi * 0.5 };
                    let same_top = gap <= 0.0 || self.ground(at + Vec3::Y * 0.3, 0.0, 0.45).is_some_and(|g| (g.point.y - at.y).abs() < 0.15);
                    if in_line && gap <= PAIR_BEAM_GAP && same_top {
                        let (lo, hi) = (mlo.min(0.0), mhi.max(len));
                        pairs[i] = Line { a: l.a + axis * lo, b: l.a + axis * hi };
                        gone[j] = true;
                        merged = true;
                        break;
                    }
                }
            }
            pairs = pairs.into_iter().zip(gone).filter(|(_, g)| !g).map(|(l, _)| l).collect();
        }
        for line in pairs {
            let centre = (line.a + line.b) * 0.5;
            if near(&centres, centre).iter().any(|&k| found[k].closest(centre).distance(centre) < 0.2) {
                continue;
            }
            let open = self.ground(centre + Vec3::Y * 1.9, 0.0, 1.98).is_some_and(|h| (h.point.y - centre.y).abs() < 0.15);
            if !open || self.inside_solid(centre + Vec3::Y * 0.5) {
                continue;
            }
            centres.entry(key(centre)).or_default().push(found.len());
            found.push(line);
        }
        // Join the pieces end to end along a top (each piece onto the line ending where it starts).
        let mut ends: FastMap<(i32, i32, i32), Vec<usize>> = FastMap::default();
        let mut joined: Vec<Line> = vec![];
        for l in found {
            let axis = l.axis();
            // (Either end of the one met either end of this: pieces of one top found in either direction.)
            let hit = near(&ends, l.a).into_iter().chain(near(&ends, l.b)).find(|&j| {
                let k = joined[j];
                k.axis().dot(axis).abs() > 0.97 && [k.a, k.b].iter().any(|e| e.distance(l.a) < 0.15 || e.distance(l.b) < 0.15)
            });
            match hit {
                Some(j) => {
                    let k = &mut joined[j];
                    // (The line from its far end to this one's far end.)
                    let pts = [k.a, k.b, l.a, l.b];
                    let (mut best, mut far) = ((k.a, k.b), 0.0f32);
                    for x in 0..4 {
                        for y in x + 1..4 {
                            if pts[x].distance(pts[y]) > far {
                                far = pts[x].distance(pts[y]);
                                best = (pts[x], pts[y]);
                            }
                        }
                    }
                    (k.a, k.b) = best;
                    ends.entry(key(k.a)).or_default().push(j);
                    ends.entry(key(k.b)).or_default().push(j);
                }
                None => {
                    ends.entry(key(l.a)).or_default().push(joined.len());
                    ends.entry(key(l.b)).or_default().push(joined.len());
                    joined.push(l);
                }
            }
        }
        // (Not along one of AC1's authored beams, in line with it and overlapping it: AC1's own says where the beam is.
        // North of the open roof a top found along an authored beam ran on 2.5 m over the roof past its end, where the
        // running game stepped off.)
        let authored = self.perches.clone();
        let along_authored = |l: &Line| {
            let axis = l.axis();
            axis != Vec3::ZERO
                && authored.iter().any(|k| {
                    let ka = k.axis();
                    let len = k.a.distance(k.b);
                    let (sa, sb) = ((l.a - k.a).dot(ka), (l.b - k.a).dot(ka));
                    let overlap = sa.max(sb).min(len) - sa.min(sb).max(0.0);
                    // (In line: within 15 degrees, a line through the other's middle within 0.15 m of it.)
                    let m = (l.a + l.b) * 0.5;
                    let off = (m - k.a) - ka * (m - k.a).dot(ka);
                    ka != Vec3::ZERO && ka.dot(axis).abs() > 0.97 && off.length() < 0.15 && overlap > 0.3
                })
        };
        joined.retain(|l| !along_authored(l));
        let n = joined.len();
        self.perches.extend(joined);
        n
    }
}

/// Rows of test-level holds along a face: from `origin` along `along` for `len` m, facing `out`, at each height.
pub(crate) fn hold_rows(strips: &mut Builder, level: &mut Level, origin: Vec3, along: Vec3, len: f32, out: Vec3, heights: &[f32]) {
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
    level.ladders.push(Ladder::upright(Vec3::new(-90.08, 0.0, z), 5.0, Vec3::NEG_X));
    beam(walls, level, -87.0, -83.0, z, 5.0);
    walls.cuboid(Vec3::new(-83.0, 0.0, z - 1.5), Vec3::new(-79.0, 5.0, z + 1.5));
    beam(walls, level, -79.0, -77.6, z, 5.0);
    level.haystacks.push(HayStack { centre: Vec3::new(-76.2, 0.0, z), half: 1.3, height: 1.7, facing: Vec3::ZERO, own: None });

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
    level.haystacks.push(HayStack { centre: Vec3::new(-70.1, 0.0, z), half: 1.3, height: 1.7, facing: Vec3::ZERO, own: None });

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

    // G11: a beam off a platform, a second beam starting 0.5 m to its right and running off to the right, a platform at
    // its end: jogging the first, the stick to the right hops onto the second (AC1's corner hop) and on along it.
    let z = 154.0;
    ramp(walls, -96.0, -92.0, z, 2.0);
    platform(walls, -92.0, -89.0, z, 2.0);
    beam(walls, level, -89.0, -83.0, z, 2.0);
    walls.cuboid(Vec3::new(-85.65, 1.8, z + 0.5), Vec3::new(-85.35, 2.0, z + 4.5));
    level.perches.push(Line { a: Vec3::new(-85.5, 2.0, z + 0.5), b: Vec3::new(-85.5, 2.0, z + 4.5) });
    walls.cuboid(Vec3::new(-87.5, 0.0, z + 4.5), Vec3::new(-83.5, 2.0, z + 7.5));

    // G12: a wall with holds up to 3 m and bare above, a slab sticking out 0.8 m from it with its edge 1 m over the top
    // row: up from the top row, AC1's jump up from the climb (TryBackEject) into a free hang from the slab's edge.
    let z = 164.0;
    walls.cuboid(Vec3::new(-92.0, 0.0, z - 2.0), Vec3::new(-88.0, top(5.0), z + 2.0));
    hold_rows(strips, level, Vec3::new(-92.0, 0.0, z - 2.0), Vec3::Z, 4.0, Vec3::NEG_X, &[top(0.0), top(1.0), top(2.0)]);
    walls.cuboid(Vec3::new(-92.8, top(2.0) + 0.8, z - 2.0), Vec3::new(-92.0, top(2.0) + 1.0, z + 2.0));
    hold_rows(strips, level, Vec3::new(-92.8, 0.0, z - 2.0), Vec3::Z, 4.0, Vec3::NEG_X, &[top(2.0) + 1.0]);

    // G13: a wall with holds up to its top edge, an awning over it 1.5 m above the edge, 0.2 to 1 m out: no climbing out
    // (AC1's room boxes over the edge reach 1 m out and 1.8 m up, 0xB2E240; audit CLIMB-E6).
    let z = 174.0;
    walls.cuboid(Vec3::new(-92.0, 0.0, z - 2.0), Vec3::new(-88.0, top(3.0), z + 2.0));
    hold_rows(strips, level, Vec3::new(-92.0, 0.0, z - 2.0), Vec3::Z, 4.0, Vec3::NEG_X, &[top(0.0), top(1.0), top(2.0), top(3.0)]);
    walls.cuboid(Vec3::new(-93.0, top(3.0) + 1.5, z - 2.0), Vec3::new(-92.2, top(3.0) + 1.6, z + 2.0));

    // G14: a wall with holds up to 4.8 m, a block sticking 0.6 m out of it between two rows: the climbing move up into it
    // is refused (AC1's body boxes at the pose a move ends in, 0xB2DF90; audit CLIMB-E13).
    let z = 184.0;
    walls.cuboid(Vec3::new(-92.0, 0.0, z - 2.0), Vec3::new(-88.0, top(5.0), z + 2.0));
    hold_rows(strips, level, Vec3::new(-92.0, 0.0, z - 2.0), Vec3::Z, 4.0, Vec3::NEG_X, &(0..6).map(|k| top(k as f32)).collect::<Vec<_>>());
    walls.cuboid(Vec3::new(-92.6, top(2.0) + 0.16, z - 2.0), Vec3::new(-92.0, top(2.0) + 0.36, z + 2.0));

    // G16: a ladder standing free in the open (its front to -X): climbed from either side, and round it from one side to
    // the other (AC1's revolve; audit LADDER-7).
    level.ladders.push(Ladder::upright(Vec3::new(-90.0, 0.0, 204.0), 4.5, Vec3::NEG_X));

    // G17: a beam run up to a wall with holds up it: standing on the beam, the empty hand climbs onto the wall (AC1's
    // climb from a beam, event 16).
    let z = 214.0;
    walls.cuboid(Vec3::new(-92.0, 0.0, z - 2.0), Vec3::new(-88.0, top(6.0), z + 2.0));
    hold_rows(strips, level, Vec3::new(-92.0, 0.0, z - 2.0), Vec3::Z, 4.0, Vec3::NEG_X, &[top(0.0), top(1.0), top(2.0), top(3.0), top(4.0), top(5.0)]);
    platform(walls, -99.0, -96.0, z, 2.0);
    beam(walls, level, -96.0, -92.0, z, 2.0);

    // G24 (the hangs, audit LEDGE-15): a bare wall 4.4 m tall. Along its first 3.5 m one hold row 2.75 m up: the standing
    // jump's band into AC1's wall-free hang, then up onto the wall in place. Along its last 3.5 m a row 2.4 m up under a slab
    // sticking out 0.8 m, its edge 3.9 m up: from the wall hang, up, AC1's swing back onto the slab's edge.
    let z = 264.0;
    walls.cuboid(Vec3::new(-92.0, 0.0, z - 4.0), Vec3::new(-88.0, 4.4, z + 4.0));
    hold_rows(strips, level, Vec3::new(-92.0, 0.0, z - 4.0), Vec3::Z, 3.5, Vec3::NEG_X, &[2.75]);
    hold_rows(strips, level, Vec3::new(-92.0, 0.0, z + 0.5), Vec3::Z, 3.5, Vec3::NEG_X, &[2.4]);
    walls.cuboid(Vec3::new(-92.8, 3.7, z + 0.5), Vec3::new(-92.0, 3.9, z + 4.0));
    hold_rows(strips, level, Vec3::new(-92.8, 0.0, z + 0.5), Vec3::Z, 3.5, Vec3::NEG_X, &[3.9]);
    // Beside it a slab 1.6 m thick, 3 m up, holds along its top edge and 0.65 m under it: pulled down off its top into the
    // wall hang, down: the feet find no wall under the lower hold, AC1's switch into the free hang.
    walls.cuboid(Vec3::new(-92.0, 3.0, z + 6.0), Vec3::new(-88.0, 4.6, z + 10.0));
    hold_rows(strips, level, Vec3::new(-92.0, 0.0, z + 6.0), Vec3::Z, 4.0, Vec3::NEG_X, &[4.6, 3.95]);
    // Past it (audit LEDGE-15, the switch a row up or down): a block 2.55-4.4 m up with hold rows 2.7 and 3.4 m up on its
    // face, the wall under it stepped back 0.5 m from 2 to 2.55 m and 1 m below: from the lower row (a free hang, the foot rays
    // finding nothing) up onto the upper (a foot on the stepped wall: AC1's switch into the wall hang a row up), and back down.
    let z = 284.0;
    walls.cuboid(Vec3::new(-92.0, 2.55, z - 4.0), Vec3::new(-88.0, 4.4, z + 4.0));
    walls.cuboid(Vec3::new(-91.5, 2.0, z - 4.0), Vec3::new(-88.0, 2.55, z + 4.0));
    walls.cuboid(Vec3::new(-91.0, 0.0, z - 4.0), Vec3::new(-88.0, 2.0, z + 4.0));
    hold_rows(strips, level, Vec3::new(-92.0, 0.0, z - 4.0), Vec3::Z, 8.0, Vec3::NEG_X, &[2.7, 3.4]);

    // G25: a beam from a platform running into a step 1 m higher, its edge a hold: walked into, AC1's beam autoclimb
    // onto it (NarrowObject event 17).
    let z = 294.0;
    ramp(walls, -96.0, -92.0, z, 2.0);
    platform(walls, -92.0, -89.0, z, 2.0);
    beam(walls, level, -89.0, -85.0, z, 2.0);
    walls.cuboid(Vec3::new(-85.0, 0.0, z - 2.0), Vec3::new(-82.0, 3.0, z + 2.0));
    level.ledges.push(Ledge { a: Vec3::new(-85.0, 3.0, z - 1.9), b: Vec3::new(-85.0, 3.0, z + 1.9), out: Vec3::NEG_X });

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
        ("G11: beam corner hop", Vec3::new(-86.0, 3.5, 154.0)),
        ("G12: the jump up from the climb", Vec3::new(-95.0, 4.0, 164.0)),
        ("G13: no climbing out under an awning", Vec3::new(-95.0, 4.0, 174.0)),
        ("G14: a block in the way up the wall", Vec3::new(-95.0, 4.0, 184.0)),
        ("G16: a free-standing ladder", Vec3::new(-92.0, 4.0, 204.0)),
        ("G17: a beam to a wall, climbed from", Vec3::new(-95.0, 4.0, 214.0)),
        ("G24: the wall-free hang, the swing back", Vec3::new(-95.0, 4.0, 264.0)),
        ("G24: the free hang onto the wall a row up", Vec3::new(-95.0, 4.0, 284.0)),
        ("G25: a beam into a step up", Vec3::new(-87.0, 3.0, 294.0)),
    ]
}

/// A test-level building from `lo` to `hi` (ground to roof): a hold along the roof's edge on every side, and on the
/// faces listed (by outward normal) rows of holds every 0.6 m from 1.79 m up to it, to climb.
pub(crate) fn building(walls: &mut Builder, strips: &mut Builder, level: &mut Level, lo: Vec3, hi: Vec3, climb: &[Vec3]) {
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
    level.ladders.push(Ladder::upright(Vec3::new(58.0, 0.0, 29.92), b3, Vec3::NEG_Z));
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
    level.haystacks.push(HayStack { centre: Vec3::new(55.4, 0.0, 10.0), half: 1.3, height: 1.7, facing: Vec3::ZERO, own: None });
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
    // A leaning wall (south, x -20 to -16, its face toward +Z, its foot at z = -58): 4 m high, leaning back 20 degrees, a
    // wall AC1's wall run takes (up to 45 degrees; audit WALL-21).
    {
        let (x0, x1, lean, h, z0) = (-20.0f32, -16.0f32, 20f32, 4.0, -58.0);
        let z1 = z0 - h * lean.to_radians().tan();
        let back = z1 - 0.5;
        let p = Vec3::new;
        walls.quad(p(x0, 0.0, z0), p(x1, 0.0, z0), p(x1, h, z1), p(x0, h, z1));
        walls.quad(p(x0, h, z1), p(x1, h, z1), p(x1, h, back), p(x0, h, back));
        walls.quad(p(x0, 0.0, back), p(x0, h, back), p(x1, h, back), p(x1, 0.0, back));
        walls.quad(p(x0, 0.0, z0), p(x0, h, z1), p(x0, h, back), p(x0, 0.0, back));
        walls.quad(p(x1, 0.0, z0), p(x1, 0.0, back), p(x1, h, back), p(x1, h, z1));
    }
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
        ("Leaning wall (20 degrees: run up it)", Vec3::new(-18.0, 4.6, -57.0)),
    ]
}

#[derive(Default)]
pub(crate) struct Builder {
    pub(crate) tris: Vec<Tri>,
}

impl Builder {
    pub(crate) fn quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
        self.tris.push(Tri { a, b, c });
        self.tris.push(Tri { a, b: c, c: d });
    }
    pub(crate) fn cuboid(&mut self, min: Vec3, max: Vec3) {
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
    pub(crate) fn mesh(self) -> (Mesh, Vec<Tri>) {
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

/// Build a city into `level` (`AC1_LEVEL`): its world (`city::spawn_city`: drawn, and its collision, guidance and
/// objects), then the holds, beams, perches and objects set up against the built collision, and where the player starts.
/// `lap` logs each phase's time.
#[allow(clippy::too_many_arguments)]
fn build_city(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    level: &mut Level,
    game_dir: &std::path::Path,
    city: &str,
    opts: crate::city::RenderOpts,
    lap: &mut crate::level_cache::Lap,
) {
    let forge = crate::city::forge_of(city);
    let centre = match crate::city::spawn_city(commands, meshes, mats, images, level, game_dir, &forge, opts) {
        Ok(c) => c,
        Err(e) => {
            warn!("no city {city}: {e:#}");
            Vec3::ZERO
        }
    };
    lap.lap("the city (spawn_city)");
    level.city = true;
    // AC1's own holds, ladders and poles where the city has them; else the edges found in its geometry.
    let authored = level.use_authored();
    if let Some((holds, ladders, bars)) = authored {
        info!("authored guidance: {holds} holds, {ladders} ladders, {bars} swing bars, {} kiosk pieces", level.kiosks.len());
    }
    lap.lap("authored guidance");
    level.build_grid();
    lap.lap("raycast grid");
    if authored.is_some() {
        let (steep, ladder, depth) = level.ac1_holds();
        info!(
            "AC1's holds: {} from the ledge grabs; none from {steep} steep, {ladder} in a ladder's zone, {depth} with no room in front (the depth filter)",
            level.ledges.len()
        );
        lap.lap("AC1's holds");
        let [catch, jump, pull] = level.ac1_views();
        info!("AC1's searches' own holds: the air catch {catch}, the jump query {jump}, the pull-down {pull}");
        lap.lap("AC1's searches' holds");
    }
    // AC1's beams and posts: the pairs of its own holds as its runtime finds them (`beam_pairs`) and its posts
    // (`add_pilotis`), no beams from mesh names or single-hold tops (audit GUIDE-5): those stood him balancing on a
    // drum's plinth against its wall and inside a window box, where the online friend walked into the walls (user report
    // 2026-10-13). `AC1_OLD_BEAMS=1` keeps ours' own, to compare.
    if authored.is_some() && std::env::var("AC1_OLD_BEAMS").is_err() {
        let n = level.pair_beams();
        info!("{n} beams paired from the authored holds");
        let posts = level.add_pilotis();
        info!("{posts} posts (pilotis) from the authored holds");
        lap.lap("paired beams, posts");
    } else {
        level.add_beam_perches();
        lap.lap("beam perches");
        let narrow = level.add_narrow_tops();
        info!("{narrow} narrow wall tops to balance on");
        lap.lap("narrow tops");
        if authored.is_some() {
            let (perches, holds) = level.drop_unbacked();
            info!("{perches} perches and {holds} holds left out: no authored ledge grab along them");
            lap.lap("unbacked perches");
        }
    }
    let probed = if authored.is_none() && std::env::var("AC1_NO_LIPS").is_err() { level.add_probed_ledges() } else { 0 };
    // A viewpoint's entity sits partway up its tower: the spot to stand on is the top above it.
    for i in 0..level.viewpoints.len() {
        let v = level.viewpoints[i];
        if let Some(top) = level.ground(v + Vec3::Y * 80.0, 0.0, 80.0).filter(|g| g.point.y > v.y) {
            level.viewpoints[i] = top.point;
        }
    }
    lap.lap("probed lips, viewpoints");
    let (hay, ladders, benches, bars) = level.add_city_objects();
    info!("{hay} haystacks, {ladders} ladders, {benches} benches, {bars} swing bars");
    lap.lap("city objects");
    let tops = level.settle_ladder_tops();
    info!("{tops} ladder tops set onto the walkable top they lead to");
    let carved = level.carve_ladders();
    info!("{carved} hold pieces carved out round ladders");
    lap.lap("ladder tops, carving");
    let heaped = level.drop_hay_holds();
    info!("{heaped} holds and beams left out: on a haystack's heap");
    // (No bars along thin beams: AC1 has none there, only the two ledge grabs, and swings from one as from any free
    // hang a jump aims at, its target type 0x80: `climb::swing_hold_ahead`.)
    lap.lap("hay holds");
    for v in &level.viewpoints {
        let near = level.haystacks.iter().map(|h| ((h.centre - *v).with_y(0.0).length(), v.y - h.top())).min_by(|a, b| a.0.total_cmp(&b.0));
        debug!("viewpoint {v:.1}: nearest hay {near:.1?} (out, down)");
    }
    level.build_grid();
    lap.lap("raycast grid");
    info!("{probed} more holds where walkable tops end over a wall (lips the edge test missed)");
    // The player on a street (Masyaf: in the village below the fortress) and a crowd loop nearby where
    // the ground is at the same height.
    let on_ground = |level: &Level, x: f32, z: f32| level.ground(Vec3::new(x, 0.0, z), 400.0, 500.0).map(|h| h.point);
    let home = if city.eq_ignore_ascii_case("masyaf") {
        on_ground(level, MASYAF_SPAWN[0], MASYAF_SPAWN[2]).unwrap_or(Vec3::ZERO)
    } else {
        crate::city::street_near(level, centre)
    };
    level.spawn = Some(home);
    // (Not up on a roof or down in a river bed.)
    let level_with = |p: Option<Vec3>| p.filter(|p| (p.y - home.y).abs() < 3.0);
    let corners: Option<Vec<Vec3>> =
        [(-6.0, -4.0), (6.0, -4.0), (6.0, -6.0), (-6.0, -6.0)].iter().map(|&(dx, dz)| level_with(on_ground(level, home.x + dx, home.z + dz))).collect();
    level.crowd_path = corners;
    info!("{city}: start at {home:.1}, crowd {}", level.crowd_path.is_some());
    lap.lap("start, crowd");
}

/// `AC1_CACHE_CHECK=1`: the city just built (`payload`, `level_cache::write_level` of it) against itself read back and
/// against what the cache file holds, byte for byte (logged `level cache check: ...`).
fn cache_check(level: &Level, payload: &[u8], path: &std::path::Path, key: u64) {
    use crate::level_cache as cache;
    let fresh = cache::summary(level, payload);
    match cache::read_level(payload).map(|l| cache::write_level(&l)) {
        Ok(b) if b == payload => info!("level cache check: read back identical ({fresh})"),
        Ok(b) => warn!("level cache check: DIFFERENT read back ({} bytes against {})", b.len(), payload.len()),
        Err(e) => warn!("level cache check: DIFFERENT, not read back: {e:#}"),
    }
    match cache::read_payload(path, key) {
        Ok(old) if old == payload => info!("level cache check: the cache file is identical to the fresh build"),
        Ok(old) => {
            let at = old.iter().zip(payload).position(|(a, b)| a != b).unwrap_or(old.len().min(payload.len()));
            let other = cache::read_level(&old).map_or_else(|e| format!("unreadable: {e:#}"), |l| cache::summary(&l, &old));
            warn!("level cache check: the cache file is DIFFERENT from the fresh build from byte {at}: cache {other}");
        }
        Err(e) => info!("level cache check: no cache file to compare ({e:#})"),
    }
}

pub fn spawn_level(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    dir: Res<crate::GameDir>,
    formats: Option<Res<bevy::image::CompressedImageFormatSupport>>,
) {
    let mut level = Level::default();
    // City textures stay block-compressed on the GPU where it takes them (every desktop GPU does).
    let bc = formats.is_some_and(|f| f.0.contains(bevy::image::CompressedImageFormats::BC));
    // (Headless runs draw nothing: no city meshes or textures are made.)
    let opts = crate::city::RenderOpts { bc, render: std::env::var("AC1_HEADLESS").is_err(), gameplay: true };
    if let Ok(city) = std::env::var("AC1_LEVEL") {
        // The gameplay data from the cache when it holds this city as built from these game files and this code
        // (`level_cache`); else built, and saved for next time. Drawn: the world is still read from the game's files.
        use crate::level_cache as cache;
        let begun = std::time::Instant::now();
        let mut lap = cache::Lap::start();
        let file = if cache::enabled() { cache::locate(&dir.0, &city) } else { None };
        let check = cache::check();
        // (Read on a thread from the launch, `preload`.)
        let cached = file.as_ref().filter(|_| !check).and_then(|(path, key)| match crate::preload::cached_level(path, *key) {
            Ok(l) => Some(l),
            Err(e) => {
                info!("level cache {}: not used ({e:#}); building the city", path.display());
                None
            }
        });
        let from_cache = cached.is_some();
        if let Some(l) = cached {
            level = l;
            lap.lap("the city's gameplay data from the cache");
            if opts.render {
                let forge = crate::city::forge_of(&city);
                // The drawn world from its own cache (`render_cache`), else read from the game's files (only what is
                // drawn: the gameplay data built then is thrown away).
                if let Some(draw) = crate::preload::draw(&dir.0, &city) {
                    lap.lap("the city's world from the render cache");
                    let detail = crate::stream::Detail::from_env();
                    let render = crate::city::realize(draw, opts.bc, detail, &mut commands, &mut meshes, &mut mats, &mut images, &forge);
                    commands.insert_resource(render);
                    lap.lap("the city's world made (drawn)");
                } else {
                    let mut drawn_only = Level::default();
                    let world = crate::city::RenderOpts { gameplay: false, ..opts };
                    if let Err(e) = crate::city::spawn_city(&mut commands, &mut meshes, &mut mats, &mut images, &mut drawn_only, &dir.0, &forge, world) {
                        warn!("no city {city}: {e:#}");
                    }
                    lap.lap("the city's world (drawn)");
                }
            }
        } else {
            build_city(&mut commands, &mut meshes, &mut mats, &mut images, &mut level, &dir.0, &city, opts, &mut lap);
            if let Some((path, key)) = &file {
                let payload = cache::write_level(&level);
                if check {
                    cache_check(&level, &payload, path, *key);
                }
                // (Written on a thread of its own: 200 MB took seconds of the first load.)
                info!("level cache {}: saving, {}", path.display(), cache::summary(&level, &payload));
                cache::write_in_background(cache::MAGIC, path.clone(), *key, payload, "level cache".into());
                lap.lap("level cache made");
            }
        }
        // `AC1_GRAB_AUDIT=<file.tsv>`: the authored guidance against what was built from it (`grab_audit`).
        if let Ok(path) = std::env::var("AC1_GRAB_AUDIT") {
            crate::grab_audit::run(&level, &path);
        }
        let shut = level.hay_with_no_way_out();
        info!("{} haystacks and hiding places with no way out{}", shut.len(), if shut.is_empty() { String::new() } else { format!(": {shut:.1?}") });
        // `AC1_JUMP_QUERY_AT="x,y,z,deg[,ground|narrow|edge]"`: AC1's jump query from the feet at a point toward a heading
        // (game degrees, 0 = +x, 90 = -z), its candidates and the scorer's pick, logged (why a jump in a recording went where it did).
        if let Ok(v) = std::env::var("AC1_JUMP_QUERY_AT") {
            let f: Vec<&str> = v.split(',').map(str::trim).collect();
            let n: Vec<f32> = f.iter().take(4).filter_map(|x| x.parse().ok()).collect();
            if n.len() == 4 {
                let (p, a) = (Vec3::new(n[0], n[1], n[2]), n[3].to_radians());
                let dir = Vec3::new(a.cos(), 0.0, -a.sin());
                let from = match f.get(4).copied() {
                    Some("narrow") => crate::jump_query::From::Narrow,
                    Some("edge") => crate::jump_query::From::Edge,
                    _ => crate::jump_query::From::Ground,
                };
                let cands = crate::jump_query::query(&level, p, dir, from, None);
                for c in &cands {
                    info!(
                        "jump query: {:?} at {:.2} ({:.2} m on, {:+.2} m up), wall {:.2}, ty {} sub {} flags {:#x}, perch {:?}",
                        c.kind,
                        c.pos,
                        (c.pos - p).with_y(0.0).length(),
                        c.pos.y - p.y,
                        c.wall,
                        c.ty,
                        c.sub,
                        c.flags,
                        c.perch
                    );
                }
                info!("jump query: {} candidates, the pick {:?}", cands.len(), crate::jump_query::select(&cands, p, dir, dir, false));
            }
        }
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
            for (i, l) in level.perches.iter().enumerate() {
                let d = l.closest(p).distance(p);
                if d < v[3] {
                    info!("perch {i}: {:.2} to {:.2}, {d:.2} m away", l.a, l.b);
                }
            }
            // (And the authored guidance pieces as the data has them, before ours' filters.)
            for (i, g) in level.authored.iter().enumerate() {
                let d = Line { a: g.a, b: g.b }.closest(p).distance(p);
                if d < v[3] {
                    info!("authored {i} ({:?}): {:.2} to {:.2}, out {:.2}, {d:.2} m away", g.kind, g.a, g.b, g.out);
                }
            }
            for (i, k) in level.kiosks.iter().enumerate() {
                let d = k.edge.closest(p).distance(p);
                if d < v[3] {
                    info!("kiosk piece {i}: {:.2} to {:.2}, out {:.2}, axis {:.2}, {d:.2} m away", k.edge.a, k.edge.b, k.edge.out, k.axis);
                }
            }
            // (And the ladders, swing bars and haystacks, flat distance: to find one to send the running game to.)
            for (i, l) in level.ladders.iter().enumerate() {
                let d = (l.base - p).with_y(0.0).length();
                if d < v[3] {
                    info!("ladder {i}: base {:.2}, top {:.2}, out {:.2}, {d:.2} m away", l.base, l.top, l.out);
                }
            }
            for (i, b) in level.bars.iter().enumerate() {
                let d = (b.closest(p) - p).with_y(0.0).length();
                if d < v[3] {
                    info!("bar {i}: {:.2} to {:.2}, {d:.2} m away", b.a, b.b);
                }
            }
            for (i, e) in level.authored.iter().enumerate() {
                let d = Line { a: e.a, b: e.b }.closest(p).distance(p);
                if d < v[3] {
                    info!("authored {i} {:?}: {:.2} to {:.2}, out {:.2}, {d:.2} m away", e.kind, e.a, e.b, e.out);
                }
            }
            for (i, h) in level.haystacks.iter().enumerate() {
                let d = (h.centre - p).with_y(0.0).length();
                if d < v[3] {
                    info!("haystack {i}: centre {:.2}, top {:.2}, {d:.2} m away", h.centre, h.top());
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
        info!("load: {city} ready in {:.2} s{}", begun.elapsed().as_secs_f32(), if from_cache { " (gameplay data from the cache)" } else { "" });
        commands.insert_resource(level);
        return;
    }
    // An editor level on empty ground (`AC1_LEVEL=empty`, or a saved level whose base is `empty`): flat ground and
    // nothing else; the pieces are added by `editor::start`.
    if crate::level_file::start_base() == crate::level_file::Base::Empty {
        let mut plain = Builder::default();
        plain.quad(Vec3::new(-500.0, 0.0, -500.0), Vec3::new(-500.0, 0.0, 500.0), Vec3::new(500.0, 0.0, 500.0), Vec3::new(500.0, 0.0, -500.0));
        let (mesh, tris) = plain.mesh();
        level.tris.extend(tris);
        let ground = StandardMaterial { base_color: Color::srgb(0.7, 0.62, 0.48), perceptual_roughness: 0.9, ..default() };
        commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mats.add(ground))));
        level.build_grid();
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
    level.ladders.push(Ladder::upright(Vec3::new(10.0, 0.0, 16.08), e_top, Vec3::Z));
    // Holds up E's back to the right of its ladder (seen from the ladder): hanging on them, sideways goes across onto
    // the ladder, and off it back onto them.
    for k in 0..7 {
        let y = 1.8 + 0.6 * k as f32;
        let (a, b) = (Vec3::new(10.75, y, 16.0), Vec3::new(11.92, y, 16.0));
        ledge_geo.cuboid(a - Vec3::Y * 0.07, b + Vec3::Z * HOLD_DEPTH);
        level.ledges.push(Ledge { a: a + Vec3::Z * 0.06, b: b + Vec3::Z * 0.06, out: Vec3::Z });
    }
    level.haystacks.push(HayStack { centre: Vec3::new(13.8, 0.0, 14.0), half: 1.3, height: 1.7, facing: Vec3::ZERO, own: None });
    // A ledge hanging in the air west of E (a slab on a post set back under it), 2.4 m to the right of E's
    // west holds and 1.2 m below its 3.6 m row: the long leap from E's face into a free hang catches it
    // with one hand and swings until steady.
    let slab = 2.4;
    w.cuboid(Vec3::new(4.6, slab - 0.2, 12.0), Vec3::new(6.4, slab, 12.6));
    w.cuboid(Vec3::new(5.42, 0.0, 12.42), Vec3::new(5.58, slab - 0.2, 12.58));
    let (a, b) = (Vec3::new(4.68, slab, 12.0), Vec3::new(6.32, slab, 12.0));
    ledge_geo.cuboid(a - Vec3::Y * 0.07 - Vec3::Z * HOLD_DEPTH, b);
    level.ledges.push(Ledge { a: a - Vec3::Z * 0.06, b: b - Vec3::Z * 0.06, out: Vec3::NEG_Z });
    level.haystacks.push(HayStack { centre: Vec3::new(5.0, 0.0, 15.5), half: 1.3, height: 1.7, facing: Vec3::ZERO, own: None });
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
    let hay = HayStack { centre: Vec3::new(13.5, 0.0, -9.2), half: 1.3, height: 1.7, facing: Vec3::ZERO, own: None };
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
    // Block L (5 m) south of the course with a ladder up its north face at x = 23.5, and kiosk K south-east of it: a
    // market stall's counter as AC1's (`Kiosk_*`: 3.7 m along its front, 2.2 m deep, its top 0.74 m up), its front facing
    // south (-Z) and its kiosk pieces along the front and the two sides, as the cities' stalls carry them.
    c.cuboid(Vec3::new(22.0, 0.0, -9.0), Vec3::new(25.0, 5.0, -6.0));
    level.ladders.push(Ladder::upright(Vec3::new(23.5, 0.0, -5.92), 5.0, Vec3::Z));
    let (kx0, kx1, kz_front, kz_back, kh) = (28.0, 31.7, -10.2, -8.0, 0.74);
    c.cuboid(Vec3::new(kx0, 0.0, kz_front), Vec3::new(kx1, kh, kz_back));
    for (a, b, out) in [
        (Vec3::new(kx0, kh, kz_front), Vec3::new(kx1, kh, kz_front), Vec3::NEG_Z),
        (Vec3::new(kx0, kh, kz_front), Vec3::new(kx0, kh, kz_back - 0.1), Vec3::NEG_X),
        (Vec3::new(kx1, kh, kz_front), Vec3::new(kx1, kh, kz_back - 0.1), Vec3::X),
    ] {
        level.kiosks.push(Kiosk { edge: Ledge { a, b, out }, axis: Vec3::X });
    }
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
    level.haystacks.push(HayStack { centre: Vec3::new(23.5, 0.0, 20.6), half: 1.3, height: 1.7, facing: Vec3::ZERO, own: None });
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

    // Ladders are drawn, not collided with.
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
        ("Kiosk K (market stall)", [29.85, 1.6, -9.1]),
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
        if let Err(e) = crate::city::spawn_props(&mut commands, &mut meshes, &mut mats, &mut images, &mut level, &dir.0, "DataPC_Damascus.forge", &picks, opts)
        {
            warn!("no prop zone: {e:#}");
        }
        // (Not the props' guidance: the test level's holds are ours, and AC1's authored edges turn on the city's rules
        // (`Level::guided`, the jump target search). With the tables' and crates' edges in, free running off every
        // platform here waited for an edge report and fell, user recording 17480; scripted runs, without the props, jumped.)
        level.authored.clear();
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
/// (AC1's controller walks up slopes to 53 degrees (cos 0.602, reanalysis GUIDE-6), but at 0.602 ours' wall run up the
/// Damascus street wall west of the bureau found nothing to catch and rebounded, where the running game hung from it:
/// kept at 0.7 till the wall tests are separated from the floor test, audit GUIDE-6.)
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

/// Does the triangle `v` cross the box round the origin with `half` extents along x, y, z (separating axes: the box's
/// faces, the triangle's plane and the nine edge crossings)?
fn tri_crosses_box(v: [Vec3; 3], half: Vec3) -> bool {
    let apart = |axis: Vec3| {
        if axis.length_squared() < 1e-12 {
            return false;
        }
        let p = v.map(|q| q.dot(axis));
        let r = half.x * axis.x.abs() + half.y * axis.y.abs() + half.z * axis.z.abs();
        p[0].min(p[1]).min(p[2]) > r || p[0].max(p[1]).max(p[2]) < -r
    };
    let edges = [v[1] - v[0], v[2] - v[1], v[0] - v[2]];
    if [Vec3::X, Vec3::Y, Vec3::Z].into_iter().any(apart) || apart(edges[0].cross(edges[1])) {
        return false;
    }
    !edges.iter().any(|e| [Vec3::X, Vec3::Y, Vec3::Z].into_iter().any(|a| apart(a.cross(*e))))
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
    fn a_hold_is_met_along_the_probe_not_across() {
        let l = Ledge { a: Vec3::new(-5.0, 2.0, 0.0), b: Vec3::new(5.0, 2.0, 0.0), out: Vec3::Z };
        // (From 1 m out, coming in 45 degrees off square: 1 m along it, where the closest point is straight in front.)
        let p = Vec3::new(0.0, 2.0, 1.0);
        let q = l.project_along(p, Vec3::new(1.0, 0.0, -1.0));
        assert!((q - Vec3::new(1.0, 2.0, 0.0)).length() < 1e-4, "{q}");
        assert!((l.closest(p) - Vec3::new(0.0, 2.0, 0.0)).length() < 1e-4);
        // (Along the hold: the closest point; past its end: the end.)
        assert!((l.project_along(p, Vec3::X) - Vec3::new(0.0, 2.0, 0.0)).length() < 1e-4);
        assert!((l.project_along(Vec3::new(9.0, 2.0, 1.0), Vec3::NEG_Z) - l.b).length() < 1e-4);
    }

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
        let h = HayStack { centre: Vec3::new(1.0, 0.0, 2.0), half: 1.0, height: 1.5, facing: Vec3::ZERO, own: None };
        assert!(h.contains(Vec3::new(1.5, 3.0, 2.5), 0.3));
        assert!(!h.contains(Vec3::new(1.8, 3.0, 2.0), 0.3));
        assert!((h.top() - 1.5).abs() < 1e-6);
    }

    #[test]
    fn sphere_cast_stops_short_of_a_wall() {
        let mut b = Builder { tris: vec![] };
        b.cuboid(Vec3::new(2.0, 0.0, -2.0), Vec3::new(3.0, 3.0, 2.0));
        let mut level = Level { tris: b.tris, ..default() };
        level.build_grid();
        let hit = level.sphere_cast(Vec3::new(0.0, 1.0, 0.0), Vec3::new(4.0, 1.0, 0.0), 0.2).expect("the wall");
        assert!((hit.x - 1.8).abs() < 0.01, "{hit}");
        assert!(level.sphere_cast(Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 1.0, 4.0), 0.2).is_none());
        assert_eq!(level.sphere_cast(Vec3::new(1.9, 1.0, 0.0), Vec3::ZERO, 0.2), Some(Vec3::new(1.9, 1.0, 0.0)));
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
    fn box_free_sees_faces_crossing_it() {
        let level = boxed(Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        let axes = [Vec3::X, Vec3::Y, Vec3::Z];
        // Over the top, clear; dipping 0.1 m into it, not; resting on it, clear (touching only).
        assert!(level.box_free(Vec3::new(0.0, 1.5, 0.0), axes, Vec3::splat(0.4)));
        assert!(!level.box_free(Vec3::new(0.0, 1.3, 0.0), axes, Vec3::splat(0.4)));
        assert!(level.box_free(Vec3::new(0.0, 1.4, 0.0), axes, Vec3::splat(0.4)));
        // Off a corner, turned 45 degrees to face it: a face 0.3 m from its centre along the diagonal.
        let r = Quat::from_rotation_y(std::f32::consts::FRAC_PI_4);
        let turned = [r * Vec3::X, Vec3::Y, r * Vec3::Z];
        assert!(level.box_free(Vec3::new(1.45, 0.5, 1.45), turned, Vec3::splat(0.3)));
        assert!(!level.box_free(Vec3::new(1.15, 0.5, 1.15), turned, Vec3::splat(0.3)));
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
