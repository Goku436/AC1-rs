//! City life: a city's streets peopled as AC1 lays them out (`forge::crowd`), around the player.
//!
//! AC1's (the data): where people may be and how many (the `CrowdRegion_<City>` layout's regions, each a
//! `CrowdDutyRegion` with a density in people per square metre: Damascus streets 0.07, souks 0.09-0.12, "No Spawn"
//! 0.001), who they are (the region's fractions: `MCxx_Poor` / `_MiddleClass` / `_Rich` townsfolk by percent, 3-10%
//! `MMMA_Damascus_Militia`, a few `MNMA_Thug` cliques; an `MCxx` builder is half its men's builder, half its women's), the
//! lanes they walk (the `CrowdFlow` polylines with their widths and the flows linked at each end), their clips (the
//! builders' clip sets, `cmma_anim_set` / `cfaa_anim_set`, over AC1's locomotion), the stumbles when bumped
//! (`HumanGround_Hurt`'s `xx_stumble_soft_<20|50>cm_<back|front>_<foot>` and `xx_stumble_<medium|strong>_<back|front>_to_
//! unbalance_*`), the charge's shove (`HumanGround_Hurt`'s `xx_h_charge_run_shove_*`, as `crowd`), and AC1's crowd cap
//! (the game settings' 120, `GameSetting` +0x28, which the crowd herder holds its count to).
//!
//! Ours: how the count is kept (the people the regions' densities give within `POP_RADIUS` of the player, at most
//! `MAX_PEOPLE` by the city detail: 12 / 24 / 32 / 48, well under AC1's 120 for the frame time; a person costs about
//! 0.15 ms of CPU a frame with the pose rebuilt less often farther off, `AnimLod`, and feet IK only near), where they appear (on a flow
//! `SPAWN_MIN`..`SPAWN_MAX` away, near the player only at the start) and go (parked beyond `DESPAWN`, reused for the
//! next of their kind), how they walk (along a flow at their own pace, on its right half, at its end on to a flow
//! linked there, sometimes stopping a while; stepping aside for each other and the player), the bumps (walking into
//! someone: their soft stumble; running: the shove and the 50 cm stumble; sprinting: their strong stumble), and the
//! militia (AC1's militia fraction as `combat`'s guards, `GUARD_MAX` at once, patrolling the flows with the player's
//! locomotion within `GUARD_NEAR`, where `combat` has them, along the lane beyond: a guard's probes cost a millisecond).
//! The thugs' cliques are left out.
//!
//! Script hooks (env): on in a city unless `AC1_NO_CROWD` (scripted runs set it); `AC1_CITY_LIFE=1` turns it on anyway
//! (`=0` off). `AC1_CROWD_MAX=n` (the cap), `AC1_CROWD_GUARDS=n` (militia at once), `AC1_CROWD_LOG=1` (every spawn and
//! park). Scripted runs (`AC1_SHOT`) load and build on the frame they need it, so a run is the same every time.

use crate::animation::AnimLib;
use crate::assets::CharacterData;
use crate::character::{Character, Controller, Player};
use crate::level::Level;
use bevy::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

pub struct CityLifePlugin;

impl Plugin for CityLifePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Life>()
            .add_systems(Update, (load, populate, patrol).chain().after(crate::combat::fight).before(crate::character::locomotion))
            .add_systems(Update, (walk, bump).chain().after(crate::character::locomotion).before(crate::character::animate));
    }
}

/// The most people and militia at once by the graphics menu's city detail (low, medium, high, ultra and full; AC1's own
/// cap is 120): `AC1_CROWD_MAX` / `AC1_CROWD_GUARDS` over them.
const MAX_PEOPLE: [usize; 4] = [12, 24, 32, 48];
const GUARD_MAX: [usize; 4] = [1, 2, 3, 3];

fn detail_index() -> usize {
    use crate::settings::DetailLevel as D;
    match crate::settings::started().detail {
        D::Low => 0,
        D::Medium => 1,
        D::High => 2,
        D::Ultra | D::Full => 3,
    }
}
/// The regions' people are counted within this of the player (m).
const POP_RADIUS: f32 = 26.0;
/// New people appear on a flow this far from the player (m), after the first fill.
const SPAWN_MIN: f32 = 22.0;
const SPAWN_MAX: f32 = 34.0;
/// Farther than this, a person is parked (m): out of the camera's view, or anywhere beyond `DESPAWN_SEEN`.
const DESPAWN: f32 = 42.0;
const DESPAWN_SEEN: f32 = 70.0;
/// A spot within this angle of the camera's forward (degrees) is in its view (wider than any screen's half width, so
/// people appear and go out of sight, as AC1's do).
const VIEW_HALF_ANGLE: f32 = 62.0;
/// Models built per builder (each its own pick of the builder's tables).
const VARIANTS: usize = 4;
/// Parked people kept for reuse at most.
const PARKED_MAX: usize = 12;
/// Walking pace (m/s), drawn per person.
const PACE: (f32, f32) = (0.95, 1.35);
/// A person stops at a flow's end this often, for this long (s).
const STOP_CHANCE: f32 = 0.2;
const STOP_TIME: (f32, f32) = (2.0, 7.0);
/// Bumps: the player within this of someone (m), and the speeds (m/s) a bump is a walk's, a run's, a sprint's.
const BUMP_REACH: f32 = 0.6;
const BUMP_WALK: f32 = 0.6;
const BUMP_RUN: f32 = 2.5;
const BUMP_SPRINT: f32 = 5.0;
/// The militia nearer the player than this (m) walk by the player's locomotion (and `combat` has them).
const GUARD_NEAR: f32 = 20.0;
/// Beyond each of these from the player (m), a person's pose is rebuilt one frame less often (`AnimLod`).
const LOD_STEPS: [f32; 2] = [10.0, 18.0];
/// People nearer the player than this (m) set their feet with IK.
const IK_NEAR: f32 = 8.0;

/// Whether city life runs (see the module docs).
pub fn enabled() -> bool {
    match std::env::var("AC1_CITY_LIFE").as_deref() {
        Ok("0") => false,
        Ok(_) => true,
        Err(_) => std::env::var("AC1_NO_CROWD").is_err(),
    }
}

fn scripted() -> bool {
    std::env::var("AC1_SHOT").is_ok()
}

fn env_usize(k: &str, d: usize) -> usize {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

/// A small deterministic generator.
#[derive(Clone, Copy)]
struct Rng(u64);
impl Rng {
    fn f(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, r: (f32, f32)) -> f32 {
        r.0 + (r.1 - r.0) * self.f()
    }
}

// --- The city's crowd, in Bevy space ------------------------------------------------------------------------------

/// A kind of place: its density and who is there.
struct Region {
    name: String,
    density: f32,
    /// The duty region's third float: the larger wins where regions overlap (ours).
    weight: f32,
    /// (percent, builder, a clique).
    fractions: Vec<(f32, String, bool)>,
}

/// A lane: points (Bevy), the distance along it at each, its width and the flows linked at its start and end.
struct Lane {
    pts: Vec<Vec3>,
    cum: Vec<f32>,
    width: f32,
    weight: f32,
    ends: [Vec<usize>; 2],
    /// Per segment: the people per metre it is spawned with (its region's density x its width).
    seg_people: Vec<f32>,
}

impl Lane {
    fn len(&self) -> f32 {
        *self.cum.last().unwrap_or(&0.0)
    }
    /// How far along the lane its point nearest `p` is (m).
    fn nearest(&self, p: Vec3) -> f32 {
        let q = p.with_y(0.0);
        let mut best = (f32::MAX, 0.0);
        for (i, w) in self.pts.windows(2).enumerate() {
            let (a, b) = (w[0].with_y(0.0), w[1].with_y(0.0));
            let ab = b - a;
            let t = ((q - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            let d = q.distance(a + ab * t);
            if d < best.0 {
                best = (d, self.cum[i] + t * (self.cum[i + 1] - self.cum[i]));
            }
        }
        best.1
    }

    /// The point and direction `s` metres along.
    fn at(&self, s: f32) -> (Vec3, Vec3) {
        let s = s.clamp(0.0, self.len());
        let i = self.cum.partition_point(|&c| c <= s).clamp(1, self.pts.len().max(2) - 1);
        let (a, b) = (self.pts[i - 1], self.pts[i]);
        let l = (self.cum[i] - self.cum[i - 1]).max(1e-4);
        (a.lerp(b, (s - self.cum[i - 1]) / l), (b - a).with_y(0.0).normalize_or(Vec3::X))
    }
}

pub struct CrowdMap {
    regions: Vec<Region>,
    /// Region triangles (Bevy x, z).
    tris: Vec<(usize, [Vec2; 3])>,
    tri_grid: HashMap<(i32, i32), Vec<usize>>,
    /// People the densities give per `PEOPLE_CELL` cell.
    people: HashMap<(i32, i32), f32>,
    lanes: Vec<Lane>,
    /// Lane segments by `SEG_CELL` cell: (lane, segment).
    seg_grid: HashMap<(i32, i32), Vec<(usize, usize)>>,
}

const TRI_CELL: f32 = 8.0;
const PEOPLE_CELL: f32 = 4.0;
const SEG_CELL: f32 = 8.0;

fn cell(p: Vec2, size: f32) -> (i32, i32) {
    ((p.x / size).floor() as i32, (p.y / size).floor() as i32)
}

/// Game (x, y, z, Z up) to Bevy.
fn bevy_of(p: [f32; 3]) -> Vec3 {
    Vec3::new(p[0], p[2], -p[1])
}

fn inside(t: &[Vec2; 3], p: Vec2) -> bool {
    let s = |a: Vec2, b: Vec2| (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
    let (d0, d1, d2) = (s(t[0], t[1]), s(t[1], t[2]), s(t[2], t[0]));
    (d0 >= 0.0 && d1 >= 0.0 && d2 >= 0.0) || (d0 <= 0.0 && d1 <= 0.0 && d2 <= 0.0)
}

impl CrowdMap {
    pub fn new(c: &forge::crowd::CityCrowd) -> Self {
        let mut regions = vec![];
        let mut by_duty: HashMap<u32, usize> = HashMap::new();
        let mut tris = vec![];
        for cell_ in &c.cells {
            let Some((name, d)) = c.duty.get(&cell_.duty) else { continue };
            let r = *by_duty.entry(cell_.duty).or_insert_with(|| {
                let fractions =
                    d.fractions.iter().filter_map(|f| f.ids.iter().find_map(|id| c.builders.get(id)).map(|b| (f.percent, b.clone(), f.clique))).collect();
                regions.push(Region { name: name.clone(), density: d.density, weight: d.weight, fractions });
                regions.len() - 1
            });
            for t in &cell_.triangles {
                tris.push((r, t.map(|v| Vec2::new(v[0], -v[1]))));
            }
        }
        let mut tri_grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        let mut people: HashMap<(i32, i32), f32> = HashMap::new();
        for (i, (r, t)) in tris.iter().enumerate() {
            let (lo, hi) = (t[0].min(t[1]).min(t[2]), t[0].max(t[1]).max(t[2]));
            let (a, b) = (cell(lo, TRI_CELL), cell(hi, TRI_CELL));
            for x in a.0..=b.0 {
                for z in a.1..=b.1 {
                    tri_grid.entry((x, z)).or_default().push(i);
                }
            }
            let area = ((t[1] - t[0]).perp_dot(t[2] - t[0])).abs() * 0.5;
            *people.entry(cell((t[0] + t[1] + t[2]) / 3.0, PEOPLE_CELL)).or_default() += area * regions[*r].density;
        }
        let index: HashMap<u32, usize> = c.flows.iter().enumerate().map(|(i, f)| (f.0, i)).collect();
        let mut map = CrowdMap { regions, tris, tri_grid, people, lanes: vec![], seg_grid: HashMap::new() };
        for (_, _, f) in &c.flows {
            let pts: Vec<Vec3> = f.points.iter().map(|p| bevy_of(*p)).collect();
            let mut cum = vec![0.0];
            for w in pts.windows(2) {
                cum.push(cum.last().unwrap_or(&0.0) + (w[1] - w[0]).with_y(0.0).length());
            }
            let link = |ids: &[u32]| ids.iter().filter_map(|id| index.get(id).copied()).collect::<Vec<_>>();
            let seg_people = pts.windows(2).map(|w| map.region_at((w[0] + w[1]) * 0.5).map_or(0.0, |r| map.regions[r].density) * f.width).collect();
            map.lanes.push(Lane { pts, cum, width: f.width, weight: f.weight, ends: [link(&f.start), link(&f.end)], seg_people });
        }
        for (li, l) in map.lanes.iter().enumerate() {
            for (si, w) in l.pts.windows(2).enumerate() {
                let n = ((w[1] - w[0]).length() / (SEG_CELL * 0.5)).ceil().max(1.0) as usize;
                let mut cells: Vec<(i32, i32)> = (0..=n).map(|k| cell(w[0].lerp(w[1], k as f32 / n as f32).xz(), SEG_CELL)).collect();
                cells.dedup();
                for c in cells {
                    let v = map.seg_grid.entry(c).or_default();
                    if !v.contains(&(li, si)) {
                        v.push((li, si));
                    }
                }
            }
        }
        map
    }

    /// The region at `p` (the heaviest of those it is in).
    fn region_at(&self, p: Vec3) -> Option<usize> {
        let q = p.xz();
        self.tri_grid
            .get(&cell(q, TRI_CELL))?
            .iter()
            .map(|&i| &self.tris[i])
            .filter(|(_, t)| inside(t, q))
            .map(|(r, _)| *r)
            .max_by(|a, b| self.regions[*a].weight.total_cmp(&self.regions[*b].weight))
    }

    /// The people the densities give within `r` of `p`.
    fn people_near(&self, p: Vec3, r: f32) -> f32 {
        let (a, b) = (cell(p.xz() - Vec2::splat(r), PEOPLE_CELL), cell(p.xz() + Vec2::splat(r), PEOPLE_CELL));
        let mut n = 0.0;
        for x in a.0..=b.0 {
            for z in a.1..=b.1 {
                let c = Vec2::new((x as f32 + 0.5) * PEOPLE_CELL, (z as f32 + 0.5) * PEOPLE_CELL);
                if c.distance(p.xz()) <= r {
                    n += self.people.get(&(x, z)).copied().unwrap_or(0.0);
                }
            }
        }
        n
    }

    /// A place on a lane `lo`..`hi` from `p`, picked by its people, where `ok` takes it: (lane, metres along it).
    fn spawn_spot(&self, p: Vec3, lo: f32, hi: f32, rng: &mut Rng, ok: impl Fn(Vec3) -> bool) -> Option<(usize, f32)> {
        let (a, b) = (cell(p.xz() - Vec2::splat(hi), SEG_CELL), cell(p.xz() + Vec2::splat(hi), SEG_CELL));
        let mut cands: Vec<(usize, usize, f32)> = vec![];
        for x in a.0..=b.0 {
            for z in a.1..=b.1 {
                for &(li, si) in self.seg_grid.get(&(x, z)).into_iter().flatten() {
                    if cands.iter().any(|c| c.0 == li && c.1 == si) {
                        continue;
                    }
                    let l = &self.lanes[li];
                    let w = l.seg_people[si] * (l.cum[si + 1] - l.cum[si]);
                    if w > 0.0 {
                        cands.push((li, si, w));
                    }
                }
            }
        }
        // (By lane and segment: the same order every run.)
        cands.sort_by_key(|c| (c.0, c.1));
        let total: f32 = cands.iter().map(|c| c.2).sum();
        for _ in 0..8 {
            let mut pick = rng.f() * total;
            let Some(&(li, si, _)) = cands.iter().find(|c| {
                pick -= c.2;
                pick <= 0.0
            }) else {
                break;
            };
            let l = &self.lanes[li];
            let s = l.cum[si] + rng.f() * (l.cum[si + 1] - l.cum[si]);
            let at = l.at(s).0;
            let d = at.with_y(0.0).distance(p.with_y(0.0));
            if (lo..=hi).contains(&d) && ok(at) {
                return Some((li, s));
            }
        }
        None
    }
}

// --- People -------------------------------------------------------------------------------------------------------

/// A built model: its data, height scale and the builder it came from.
struct Built {
    data: CharacterData,
    scale: f32,
    concrete: String,
}

/// A builder people come from (a region fraction's: `MCxx_Poor`), and its built variants.
struct Kind {
    builder: String,
    variants: Vec<Arc<Built>>,
    failed: bool,
}

type Job<T> = Mutex<Option<JoinHandle<T>>>;

#[derive(Resource, Default)]
pub struct Life {
    started: bool,
    map: Option<Arc<CrowdMap>>,
    loading: Job<anyhow::Result<CrowdMap>>,
    kinds: Vec<Kind>,
    /// A model being built on a thread: (kind, the job).
    building: Option<(usize, Job<anyhow::Result<Built>>)>,
    /// Rigs shared by people with the same bones and clip set.
    rigs: HashMap<(Vec<u32>, &'static str), usize>,
    /// Altaïr's sword, for the militia's hip (as `combat`'s guards).
    sword: Option<Option<(crate::assets::Part, HashMap<u32, Image>)>>,
    rng: Option<Rng>,
    clock: f32,
    next_check: f32,
    next_log: f32,
    spawned: usize,
    /// The GPU's copies of each model's parts, by (kind, variant, militia): shared by its people.
    shared: HashMap<(usize, usize, bool), crate::character::SharedParts>,
}

/// A person of the city.
#[derive(Component)]
pub struct Citizen {
    kind: usize,
    lane: usize,
    s: f32,
    forward: bool,
    /// Sideways from the lane's middle (m, + right), and where it is heading to.
    side: f32,
    side_want: f32,
    pace: f32,
    /// Slowed this frame (0..1) for someone ahead.
    slow: f32,
    stop: f32,
    stumble: f32,
    /// Clips still to play after the stumble playing.
    then: Vec<Arc<crate::animation::Clip>>,
    knocked: Vec3,
    rng: Rng,
    pub parked: bool,
    guard: bool,
}

impl Citizen {
    fn new(kind: usize, lane: usize, s: f32, rng: &mut Rng, guard: bool) -> Self {
        let mut r = Rng(rng.0 ^ 0x5851_f42d_4c95_7f2d);
        r.f();
        let pace = r.range(PACE);
        // (Anywhere across the lane, more on its right.)
        let side = r.range((-0.5, 1.0));
        Citizen {
            kind,
            lane,
            s,
            forward: r.f() < 0.5,
            side,
            side_want: side,
            pace,
            slow: 1.0,
            stop: 0.0,
            stumble: 0.0,
            then: vec![],
            knocked: Vec3::ZERO,
            rng: r,
            parked: false,
            guard,
        }
    }

    /// On along its lane by `ds`; at an end, on to a lane linked there (or back the way it came), maybe stopping.
    fn advance(&mut self, map: &CrowdMap, ds: f32) {
        let l = &map.lanes[self.lane];
        self.s += if self.forward { ds } else { -ds };
        let over = if self.forward { self.s - l.len() } else { -self.s };
        if over <= 0.0 {
            return;
        }
        let end = l.at(if self.forward { l.len() } else { 0.0 }).0;
        let links: Vec<usize> = l.ends[self.forward as usize].iter().copied().filter(|&n| n != self.lane && map.lanes[n].len() > 0.5).collect();
        let total: f32 = links.iter().map(|&n| map.lanes[n].weight.max(0.01)).sum();
        let mut pick = self.rng.f() * total;
        let next = links.iter().copied().find(|&n| {
            pick -= map.lanes[n].weight.max(0.01);
            pick <= 0.0
        });
        if self.rng.f() < STOP_CHANCE {
            self.stop = self.rng.range(STOP_TIME);
        }
        match next {
            Some(n) => {
                let nl = &map.lanes[n];
                // (In at the end nearer the one it left by.)
                let from_start = nl.pts[0].distance(end) <= nl.pts[nl.pts.len() - 1].distance(end);
                self.lane = n;
                self.forward = from_start;
                self.s = if from_start { over.min(nl.len()) } else { (nl.len() - over).max(0.0) };
            }
            None => {
                self.forward = !self.forward;
                self.s = if self.forward { over.min(l.len()) } else { (l.len() - over).max(0.0) };
            }
        }
    }

    /// Where it stands: on its lane, to the side, knocked aside; and its walking direction.
    fn spot(&self, map: &CrowdMap) -> (Vec3, Vec3) {
        let l = &map.lanes[self.lane];
        let (p, d) = l.at(self.s);
        let dir = if self.forward { d } else { -d };
        (p + dir.cross(Vec3::Y) * self.side + self.knocked, dir)
    }
}

/// Start loading the city's crowd (on a thread; at once when scripted).
fn start(life: &mut Life, dir: &std::path::Path) {
    life.started = true;
    let Ok(city) = std::env::var("AC1_LEVEL") else { return };
    let forge_file = crate::city::forge_of(&city);
    let dir = dir.to_path_buf();
    let job = move || -> anyhow::Result<CrowdMap> {
        let t = std::time::Instant::now();
        let c = forge::crowd::load_city(&dir, &forge_file)?;
        let map = CrowdMap::new(&c);
        info!(
            "city life: {} regions, {} region triangles, {} flows, {} builders named ({:.1} s){}",
            map.regions.len(),
            map.tris.len(),
            map.lanes.len(),
            map.regions.iter().map(|r| r.fractions.len()).sum::<usize>(),
            t.elapsed().as_secs_f32(),
            if c.failed.is_empty() { String::new() } else { format!(", {} failed", c.failed.len()) }
        );
        for r in &map.regions {
            debug!("city life: region {}: density {} weight {}, {:?}", r.name, r.density, r.weight, r.fractions);
        }
        Ok(map)
    };
    if scripted() {
        match job() {
            Ok(m) => life.map = Some(Arc::new(m)),
            Err(e) => warn!("city life: no crowd data: {e:#}"),
        }
    } else {
        *life.loading.lock().unwrap_or_else(|e| e.into_inner()) = Some(std::thread::spawn(job));
    }
}

/// The crowd's data, once loaded.
fn load(mut life: ResMut<Life>, level: Res<Level>, dir: Res<crate::GameDir>) {
    if !life.started {
        if level.city && enabled() {
            start(&mut life, &dir.0);
        } else {
            life.started = true;
        }
        return;
    }
    let done = life.loading.lock().ok().and_then(|mut j| if j.as_ref().is_some_and(|h| h.is_finished()) { j.take() } else { None });
    if let Some(h) = done {
        match h.join() {
            Ok(Ok(m)) => life.map = Some(Arc::new(m)),
            Ok(Err(e)) => warn!("city life: no crowd data: {e:#}"),
            Err(_) => warn!("city life: the crowd load failed"),
        }
    }
}

/// Build one model of builder `builder` (variant `k`).
fn build(dir: &std::path::Path, builder: &str, k: usize) -> anyhow::Result<Built> {
    let seed = 0x3c6e_f372 ^ (k as u64 * 0x9e37_79b9) ^ builder.bytes().fold(0u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64));
    let (data, scale, concrete) = crate::npc::build_common(dir, builder, seed)?;
    Ok(Built { data, scale, concrete })
}

/// The kind for builder `name` (added if new).
fn kind_of(life: &mut Life, name: &str) -> usize {
    if let Some(i) = life.kinds.iter().position(|k| k.builder == name) {
        return i;
    }
    life.kinds.push(Kind { builder: name.to_string(), variants: vec![], failed: false });
    life.kinds.len() - 1
}

/// Ask for another model of kind `k` (one build at a time; at once when scripted).
fn want_model(life: &mut Life, k: usize, dir: &std::path::Path) {
    if life.building.is_some() || life.kinds[k].failed || life.kinds[k].variants.len() >= VARIANTS {
        return;
    }
    let (builder, n) = (life.kinds[k].builder.clone(), life.kinds[k].variants.len());
    if scripted() {
        finish_build(life, k, build(dir, &builder, n));
        return;
    }
    let dir = dir.to_path_buf();
    life.building = Some((k, Mutex::new(Some(std::thread::spawn(move || build(&dir, &builder, n))))));
}

fn finish_build(life: &mut Life, k: usize, r: anyhow::Result<Built>) {
    // (A model without the biped bones cannot be spawned: `spawn_character` needs them.)
    let r = r.and_then(|b| if crate::character::is_biped(&b.data.rig) { Ok(b) } else { Err(anyhow::anyhow!("{}: no biped bones", b.concrete)) });
    match r {
        Ok(b) => {
            info!("city life: built {} ({}, variant {})", life.kinds[k].builder, b.concrete, life.kinds[k].variants.len());
            life.kinds[k].variants.push(Arc::new(b));
        }
        Err(e) => {
            warn!("city life: {}: {e:#}", life.kinds[k].builder);
            life.kinds[k].failed = true;
        }
    }
}

/// The sword for the militia (Altaïr's, as `combat` hands its guards).
fn sword(dir: &std::path::Path) -> Option<(crate::assets::Part, HashMap<u32, Image>)> {
    let (f, d, n) = crate::preload::ALTAIR;
    let data = crate::assets::load_character(dir, f, d, n).map_err(|e| warn!("city life: no sword for the militia: {e:#}")).ok()?;
    let part = data.parts.iter().find(|p| p.name.contains("Sword"))?.clone();
    Some((part, data.textures))
}

type PeopleQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Citizen,
        &'static mut Transform,
        &'static mut Visibility,
        Has<Controller>,
        Option<&'static crate::combat::Guard>,
        Option<&'static crate::combat::Fighter>,
    ),
    Without<Player>,
>;
type MeshAssets<'w> = (
    ResMut<'w, Assets<Mesh>>,
    ResMut<'w, Assets<StandardMaterial>>,
    ResMut<'w, Assets<Image>>,
    ResMut<'w, Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>,
);

/// Keep the streets peopled round the player: park those far off, bring people in (parked ones of their kind first,
/// else new from a built model), as many as the regions' densities give near the player, up to the cap.
#[allow(clippy::too_many_arguments)]
fn populate(
    time: Res<Time>,
    mut commands: Commands,
    mut life: ResMut<Life>,
    dir: Res<crate::GameDir>,
    level: Res<Level>,
    mut lib: Option<ResMut<AnimLib>>,
    player: Query<&Transform, (With<Player>, Without<Citizen>)>,
    cams: Query<&GlobalTransform, With<Camera3d>>,
    mut people: PeopleQuery,
    (mut meshes, mut mats, mut images, mut bindposes): MeshAssets,
) {
    let Some(map) = life.map.clone() else { return };
    let (Some(lib), Ok(ptf)) = (lib.as_deref_mut(), player.single()) else { return };
    let dt = time.delta_secs().min(0.05);
    life.clock += dt;
    let first = life.rng.is_none();
    let mut rng = life.rng.unwrap_or(Rng(0x2545_f491_4f6c_dd1d));
    // A model finished on its thread.
    let finished = life.building.as_ref().and_then(|(k, j)| {
        let mut j = j.lock().ok()?;
        if j.as_ref().is_some_and(|h| h.is_finished()) { Some((*k, j.take()?)) } else { None }
    });
    if let Some((k, h)) = finished {
        life.building = None;
        let r = h.join().unwrap_or_else(|_| Err(anyhow::anyhow!("the build failed")));
        finish_build(&mut life, k, r);
    }
    if life.clock < life.next_check && !first {
        life.rng = Some(rng);
        return;
    }
    life.next_check = life.clock + 0.25;
    let at = ptf.translation;
    // (People appear and go out of the camera's view: popping in and out in plain sight read as teleporting.)
    let cam = cams.iter().next().map(|g| (g.translation(), g.forward().as_vec3()));
    let seen = |p: Vec3| cam.is_some_and(|(eye, fwd)| (p + Vec3::Y - eye).normalize_or_zero().dot(fwd) > VIEW_HALF_ANGLE.to_radians().cos());
    // Park those far off (the militia go: `combat` keeps fighting whoever exists).
    let mut live = 0;
    let mut guards = 0;
    let mut parked: Vec<(Entity, usize)> = vec![];
    for (e, mut c, tf, mut vis, stick, guard, fighter) in &mut people {
        // The militia walk by the player's locomotion near the player (so `combat` has them), along the lane
        // farther off (its probes cost a millisecond a guard).
        let calm = guard.is_none_or(|g| !g.hostile) && fighter.is_none_or(|f| !f.fight && f.idle());
        let d = tf.translation.distance(at);
        if c.guard && !stick && d < GUARD_NEAR {
            commands.entity(e).insert(Controller::default());
        } else if c.guard && stick && calm && d > GUARD_NEAR + 4.0 {
            commands.entity(e).remove::<Controller>();
            // (Back on its lane from where the locomotion left it: at the nearest point along the lane, the rest as a
            // knock that eases off, not a jump back to the lane spot it walked from.)
            c.s = map.lanes[c.lane].nearest(tf.translation);
            c.knocked = Vec3::ZERO;
            let (spot, _) = c.spot(&map);
            c.knocked = (tf.translation - spot).with_y(0.0);
        }
        if c.parked {
            parked.push((e, c.kind));
            continue;
        }
        if d > DESPAWN && c.stumble <= 0.0 && calm && (d > DESPAWN_SEEN || !seen(tf.translation)) {
            if c.guard {
                commands.entity(e).despawn();
                debug!("city life: militia gone");
                continue;
            }
            c.parked = true;
            *vis = Visibility::Hidden;
            parked.push((e, c.kind));
            if std::env::var("AC1_CROWD_LOG").is_ok() {
                info!("city life: parked {}", life.kinds[c.kind].builder);
            }
            continue;
        }
        live += 1;
        guards += c.guard as usize;
    }
    // (Too many parked: the oldest go.)
    while parked.len() > PARKED_MAX {
        let (e, _) = parked.remove(0);
        commands.entity(e).despawn();
    }
    let cap = env_usize("AC1_CROWD_MAX", MAX_PEOPLE[detail_index()]);
    let want = (map.people_near(at, POP_RADIUS).round() as usize).min(cap);
    if life.clock >= life.next_log {
        life.next_log = life.clock + 5.0;
        let region = map.region_at(at).map_or("none", |r| map.regions[r].name.as_str());
        if std::env::var("AC1_CROWD_LOG").is_ok() {
            let spots: Vec<String> = people
                .iter()
                .filter(|p| !p.1.parked)
                .map(|p| format!("[{:.1}, {:.1}, {:.1}]", p.2.translation.x, p.2.translation.y, p.2.translation.z))
                .collect();
            info!("city life: people at {}", spots.join(" "));
        }
        info!(
            "city life: {live} people near ({want} wanted, {} parked, {guards} militia), in {region} at [{:.1}, {:.1}, {:.1}]",
            parked.len(),
            at.x,
            at.y,
            at.z
        );
    }
    // Bring people in: all at the first fill (round the player), then a few at a time, out at a distance.
    let (lo, hi) = if first || live == 0 { (3.0, POP_RADIUS) } else { (SPAWN_MIN, SPAWN_MAX) };
    let mut budget = if first { want } else { 2 };
    let guard_max = env_usize("AC1_CROWD_GUARDS", GUARD_MAX[detail_index()]);
    while live < want && budget > 0 {
        budget -= 1;
        let Some((lane, s)) = map.spawn_spot(at, lo, hi, &mut rng, |p| first || !seen(p)) else { break };
        let Some(region) = map.region_at(map.lanes[lane].at(s).0) else { continue };
        let r = &map.regions[region];
        // Who: a fraction by percent (the cliques left out; the militia only while there are few).
        let total: f32 = r.fractions.iter().filter(|f| !f.2).map(|f| f.0).sum();
        let mut pick = rng.f() * total;
        let Some((_, builder, _)) = r.fractions.iter().filter(|f| !f.2).find(|f| {
            pick -= f.0;
            pick <= 0.0
        }) else {
            continue;
        };
        let militia = builder.starts_with("MMMA");
        if militia && guards >= guard_max {
            continue;
        }
        let builder = builder.clone();
        let k = kind_of(&mut life, &builder);
        // A parked one of its kind, moved here.
        if !militia && let Some(i) = parked.iter().position(|p| p.1 == k) {
            let (e, _) = parked.remove(i);
            if let Ok((_, mut c, mut tf, mut vis, ..)) = people.get_mut(e) {
                let kind = c.kind;
                *c = Citizen::new(kind, lane, s, &mut rng, false);
                let (p, dir) = c.spot(&map);
                place(&mut tf, &level, p, dir);
                *vis = Visibility::Inherited;
                live += 1;
                if std::env::var("AC1_CROWD_LOG").is_ok() {
                    info!("city life: back {} in {}", life.kinds[k].builder, r.name);
                }
            }
            continue;
        }
        // A new one from a built model (else the model is asked for, and the next check tries again).
        let n = life.kinds[k].variants.len();
        if n < VARIANTS {
            want_model(&mut life, k, &dir.0);
        }
        let n = life.kinds[k].variants.len();
        if n == 0 {
            continue;
        }
        let variant = life.spawned % n;
        let b = life.kinds[k].variants[variant].clone();
        life.spawned += 1;
        // (The GPU's copies of the model's parts are shared by everyone of it: once they are made, only what the body
        // needs is copied, not its meshes and textures again.)
        let shared = life.shared.entry((k, variant, militia)).or_insert_with(|| crate::character::SharedParts::new(true));
        let mut data = if shared.made() { light_copy(&b.data) } else { b.data.clone() };
        let set = crate::npc::anim_set_of(&b.concrete);
        if militia {
            if life.sword.is_none() {
                life.sword = Some(sword(&dir.0));
            }
            if let (Some(Some((part, tex))), Some(hip)) = (&life.sword, data.rig.find("P_LeftWaist_Tag").or_else(|| data.rig.find("#3a835926"))) {
                let mut part = part.clone();
                part.joints = vec![hip];
                for (id, img) in tex {
                    data.textures.entry(*id).or_insert_with(|| img.clone());
                }
                data.parts.push(part);
            }
        }
        let rig_key = (data.hashes.clone(), set);
        let rig_id = match life.rigs.get(&rig_key) {
            Some(&r) => r,
            None => {
                let r = lib.add_rig(&data.rig, data.hashes.clone());
                lib.use_set(r, set);
                life.rigs.insert(rig_key, r);
                r
            }
        };
        let anim = crate::animation::Animator::new_for(lib, rig_id);
        let mut c = Citizen::new(k, lane, s, &mut rng, militia);
        let (p, dir) = c.spot(&map);
        let mut tf = Transform::from_translation(p).with_scale(Vec3::splat(b.scale));
        place(&mut tf, &level, p, dir);
        let assets = (&mut *meshes, &mut *mats, &mut *images, &mut *bindposes);
        let shared = life.shared.get_mut(&(k, variant, militia));
        let e = crate::character::spawn_character_shared(&mut commands, data, tf.translation, assets, anim, rig_id, shared);
        if militia {
            c.side_want = 0.0;
            let hp = env_usize("AC1_GUARD_HP", 3) as i32;
            let lod = crate::character::AnimLod { every: 1, phase: life.spawned as u32 % 3, skipped: 0.0 };
            commands.entity(e).insert((
                tf,
                c,
                lod,
                crate::combat::Fighter::new(crate::combat::Weapon::Sword, hp),
                crate::combat::Guard::new(0x6d2b_79f5 ^ life.spawned as u32),
            ));
            guards += 1;
            info!("city life: militia ({}) in {} at [{:.1}, {:.1}, {:.1}]", b.concrete, r.name, p.x, p.y, p.z);
        } else {
            // (Moved along its lane here, not by the player's locomotion.)
            let lod = crate::character::AnimLod { every: 1, phase: life.spawned as u32 % 3, skipped: 0.0 };
            commands.entity(e).insert((tf, c, lod)).remove::<Controller>();
            if std::env::var("AC1_CROWD_LOG").is_ok() || first {
                debug!("city life: {} ({}) in {} at [{:.1}, {:.1}, {:.1}]", builder, b.concrete, r.name, p.x, p.y, p.z);
            }
        }
        live += 1;
    }
    life.rng = Some(rng);
}

/// A model's data for a body whose parts' GPU copies are made already (`SharedParts`): everything but the meshes (the
/// cloth's kept, it is simulated per body) and textures.
fn light_copy(d: &CharacterData) -> CharacterData {
    let cloth = d.cloth.as_ref().map(|c| c.mesh.as_str());
    let parts = d
        .parts
        .iter()
        .map(|p| crate::assets::Part {
            name: p.name.clone(),
            mesh: if cloth == p.name.split('#').next() { p.mesh.clone() } else { Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, default()) },
            joints: p.joints.clone(),
            inverse_binds: p.inverse_binds.clone(),
            texture: p.texture,
            normal: p.normal,
            alpha: p.alpha,
            material: p.material.clone(),
            look: p.look.clone(),
            inside: p.inside.clone(),
        })
        .collect();
    CharacterData {
        rig: d.rig.clone(),
        hashes: d.hashes.clone(),
        parts,
        textures: HashMap::new(),
        cloth: d.cloth.clone(),
        modifiers: d.modifiers.clone(),
        main_bones: d.main_bones,
        skeleton_wind: d.skeleton_wind,
    }
}

/// Stand a transform at `p` (on the ground under it), facing `dir`.
fn place(tf: &mut Transform, level: &Level, p: Vec3, dir: Vec3) {
    let y = level.ground(p, 1.5, 3.0).map_or(p.y, |h| h.point.y);
    tf.translation = p.with_y(y);
    tf.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, dir.with_y(0.0).normalize_or(Vec3::NEG_Z));
}

type Walkers<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static mut Citizen, &'static mut Transform, &'static mut Character, Option<&'static mut crate::character::AnimLod>),
    (Without<Player>, Without<Controller>),
>;

/// Walk everyone along their lanes, stepping aside for each other and the player.
fn walk(time: Res<Time>, life: Res<Life>, level: Res<Level>, player: Query<(&Transform, &Character), With<Player>>, mut q: Walkers) {
    let Some(map) = life.map.as_deref() else { return };
    let dt = time.delta_secs().min(0.05);
    if dt <= 0.0 {
        return;
    }
    let me = player.single().ok().map(|(t, c)| (t.translation, c.velocity));
    let others: Vec<(Entity, Vec3, Vec3)> = q.iter().filter(|(_, c, ..)| !c.parked).map(|(e, _, t, ch, _)| (e, t.translation, ch.velocity)).collect();
    for (e, mut c, mut tf, mut ch, lod) in &mut q {
        if c.parked {
            continue;
        }
        // (The pose rebuilt less often farther off: every frame near the player, every second or third beyond.)
        if let Some(mut l) = lod {
            let d = me.map_or(0.0, |(p, _)| p.distance(tf.translation));
            l.every = LOD_STEPS.iter().filter(|&&r| d > r).count() as u32 + 1;
        }
        let c = &mut *c;
        // A stumble playing: it stands where it was knocked to, then plays what follows, then walks on.
        if c.stumble > 0.0 {
            c.stumble -= dt;
            if c.stumble <= 0.0 {
                let next = (!c.then.is_empty()).then(|| c.then.remove(0));
                if let Some(a) = &mut ch.animator {
                    a.forced = next.clone().map(|n| (n, 0.0));
                }
                match next {
                    Some(n) => c.stumble = n.anim.duration,
                    // (Out of the stumble into the walk, crossfaded.)
                    None => ch.exit_fade = Some((ch.anim_pose.clone(), 0.3, 0.3)),
                }
            }
            ch.velocity = Vec3::ZERO;
            let (p, _) = c.spot(map);
            tf.translation = tf.translation.lerp(p.with_y(tf.translation.y), 1.0 - (-6.0 * dt).exp());
            continue;
        }
        c.knocked *= (-0.8 * dt).exp();
        let (here, dir) = c.spot(map);
        let right = dir.cross(Vec3::Y);
        // Ahead: someone in the way slows it and moves it over (to its right for someone coming, round them
        // otherwise); the player too, more.
        c.slow = 1.0;
        let half = (map.lanes[c.lane].width * 0.5 - 0.3).max(0.2);
        let mut want = c.side_want;
        let ahead = others.iter().filter(|o| o.0 != e).map(|o| (o.1, o.2, 0.9)).chain(me.map(|(p, v)| (p, v, 1.3)));
        for (p, v, reach) in ahead {
            let d = (p - here).with_y(0.0);
            let along = d.dot(dir);
            let across = d.dot(right);
            if !(0.0..2.2).contains(&along) || across.abs() > reach {
                continue;
            }
            let coming = v.dot(dir) < -0.2;
            c.slow = c.slow.min(if coming { 0.8 } else { (along / 2.2).clamp(0.25, 1.0) });
            want = if coming { c.side + 0.5 } else { c.side - across.signum() * 0.6 };
        }
        // Close by (any side): apart.
        for o in others.iter().filter(|o| o.0 != e) {
            let d = (here - o.1).with_y(0.0);
            if d.length() < 0.55 && d.length() > 1e-3 {
                c.knocked += d.normalize() * (0.55 - d.length()) * 0.5;
            }
        }
        c.side += (want.clamp(-half, half) - c.side) * (1.0 - (-1.5 * dt).exp());
        if c.stop > 0.0 {
            c.stop -= dt;
        } else {
            let pace = c.pace * c.slow;
            c.advance(map, pace * dt);
        }
        let (p, dir) = c.spot(map);
        let y = level.ground(p.with_y(tf.translation.y.max(p.y)), 1.2, 3.0).map_or(tf.translation.y, |h| h.point.y);
        let to = p.with_y(y);
        let v = (to - tf.translation).with_y(0.0) / dt;
        tf.translation = to;
        // (The walk's speed for the clips; standing still while stopped.)
        ch.velocity = if c.stop > 0.0 { Vec3::ZERO } else { v.clamp_length_max(c.pace * 1.5) };
        // (Feet set on the ground by IK only near the player: its rays cost the most of a person's frame.)
        ch.ik_enabled = me.is_some_and(|(p, _)| p.distance(to) < IK_NEAR);
        let face = Quat::from_rotation_arc(Vec3::NEG_Z, dir.with_y(0.0).normalize_or(Vec3::NEG_Z));
        if c.stop <= 0.0 {
            tf.rotation = tf.rotation.slerp(face, 1.0 - (-5.0 * dt).exp());
        }
    }
}

/// The militia walk the lanes with the stick (the player's locomotion) while they are not fighting.
fn patrol(
    time: Res<Time>,
    life: Res<Life>,
    mut q: Query<(&mut Citizen, &Transform, &mut Controller, &Character, &crate::combat::Fighter, &crate::combat::Guard)>,
) {
    let Some(map) = life.map.as_deref() else { return };
    let dt = time.delta_secs().min(0.05);
    for (mut c, tf, mut ctl, ch, f, g) in &mut q {
        if !c.guard {
            continue;
        }
        if g.hostile || f.fight || !f.idle() || ch.fighting || ch.limp {
            ctl.move_dir = Vec3::ZERO;
            continue;
        }
        if c.stop > 0.0 {
            c.stop -= dt;
            ctl.move_dir = Vec3::ZERO;
            continue;
        }
        // Head for 2 m on along the lane; on once there.
        let target = {
            let mut ahead = Citizen { then: vec![], ..*c };
            ahead.advance(map, 2.0);
            ahead.spot(map).0
        };
        let to = (target - tf.translation).with_y(0.0);
        if to.length() < 1.6 {
            c.advance(map, 0.5);
        }
        ctl.move_dir = to.normalize_or_zero();
        ctl.speed = c.pace;
        ctl.stick = None;
    }
}

type Bumper<'w, 's> = Query<'w, 's, (&'static Transform, &'static mut Character), (With<Player>, Without<Citizen>)>;

/// The player bumping into people: walking, their soft stumble; running, the shove and their 50 cm stumble;
/// sprinting, their strong stumble (AC1's `HumanGround_Hurt` clips, see the module docs).
fn bump(mut lib: Option<ResMut<AnimLib>>, life: Res<Life>, mut player: Bumper, mut q: Walkers) {
    let (Some(lib), Ok((ptf, mut pch))) = (lib.as_deref_mut(), player.single_mut()) else { return };
    let v = pch.velocity.with_y(0.0);
    let speed = v.length();
    if speed < BUMP_WALK || pch.wall.is_some() {
        return;
    }
    let fwd = v / speed;
    for (_, mut c, tf, mut ch, _) in &mut q {
        if c.parked || c.stumble > 0.0 {
            continue;
        }
        let d = (tf.translation - ptf.translation).with_y(0.0);
        if d.length() > BUMP_REACH || d.normalize_or_zero().dot(fwd) < 0.2 || (tf.translation.y - ptf.translation.y).abs() > 1.0 {
            continue;
        }
        let side = if fwd.cross(Vec3::Y).dot(d) >= 0.0 { fwd.cross(Vec3::Y) } else { -fwd.cross(Vec3::Y) };
        let facing = (tf.rotation * Vec3::NEG_Z).with_y(0.0);
        // (Hit from the front it goes back, from behind forward.)
        let from = if facing.dot(fwd) < 0.0 { "back" } else { "front" };
        let foot = if c.rng.f() < 0.5 { "footl" } else { "footr" };
        let (how, clips, knock) = if speed >= BUMP_SPRINT {
            ("sprint", vec![format!("xx_stumble_strong_{from}_to_unbalance_{from}"), format!("xx_stumble_unbalance_{from}_tr_h_wait_footl")], 1.4)
        } else if speed >= BUMP_RUN {
            ("run", vec![format!("xx_stumble_soft_50cm_{from}_{foot}")], 0.9)
        } else {
            (
                "walk",
                vec![
                    format!("xx_stumble_soft_20cm_{from}_{foot}"),
                    format!("xx_stumble_soft_20cm_{from}_{foot}_tr_l_wait_{}", if (from == "back") == (foot == "footl") { "footl" } else { "footr" }),
                ],
                0.4,
            )
        };
        let rig = ch.rig_id;
        let mut got: Vec<Arc<crate::animation::Clip>> = clips.iter().filter_map(|n| lib.get_for(rig, n)).collect();
        if got.is_empty() {
            continue;
        }
        let first = got.remove(0);
        c.stumble = first.anim.duration;
        c.then = got;
        c.knocked += side * knock + fwd * knock * 0.3;
        c.stop = c.stop.max(0.5);
        if let Some(a) = ch.animator.as_mut() {
            a.forced = Some((first.clone(), 0.0));
        }
        // Running: Altaïr shoulders them aside (the charge's shove, over the run).
        if speed >= BUMP_RUN {
            let right = fwd.cross(Vec3::Y).dot(d) > 0.0;
            let shove = if right { "xx_h_charge_run_shove_right_handr_b" } else { "xx_h_charge_run_shove_left_handl_b" };
            if let (Some(a), Some(clip)) = (pch.animator.as_mut(), lib.get(shove))
                && a.oneshot.is_none()
            {
                a.oneshot = Some((clip, 0.0));
            }
        }
        info!("city life: bump ({how}, {speed:.1} m/s) {} plays {}", life.kinds.get(c.kind).map_or("?", |k| k.builder.as_str()), first.name);
    }
}
