//! The climbing sweep (`AC1_CLIMB_SWEEP=n`, any level): climbing as a player meets it, at many walls, scored. It picks
//! `n` walls from the level's holds (spread over it, 8 m apart: a hold 1.5-2.05 m over open ground in front, reached
//! from standing) and up to `n / 2` roof edges (a hold with a walkable top behind it and a drop of 2.5 m or more in front),
//! and runs scripted inputs at each through the control runs (`control_runs`: the level loads once, each run starts
//! fresh): from the ground grab, then `up` (16 s up, over the top where there is one), `side` (up a little, left 5 s,
//! right 9 s), `down` (up 3 s, down 6 s, onto the ground), `drop` (up, then let go) and `run` (from 4.5 m out, free
//! running into the wall, then up); at a roof edge `hang` (walk off it with the empty hand: the pull down, then left,
//! right and up). Each frame of a run is checked, and each run reports these failure classes once (the first time):
//! - `stuck`: the stick held 2.5 s, hanging, and no progress while holds go on that way (one of AC1's grid cells on:
//!   within 0.3 m along and 0.15 m up or down of a point 0.75 m to the side, or 0.6 or 1.2 m up or down).
//! - `topout`: the stick up held 2.5 s with no progress at a hold with a walkable top right behind it.
//! - `letgo`: off the wall into a fall without being asked (not the `drop` run, not stepping down at the bottom);
//!   `letgo-down`: the stick down lets go more than 1.2 m over the ground.
//! - `hands-off`: resting (a hang's wait loop, 0.3 s in), a hand's knuckle (AC1's hand effector) over 0.15 m from every hold for
//!   0.25 s; `no-hold`: over 0.4 m (climbed up into nothing).
//! - `embed`: the hips or the chest over 0.08 m inside a solid (`Level::inside_solid`) 3 frames running.
//! - `pop`: the pose check's pop (`recorder`, as `AC1_POP_CHECK`) over 0.15 m.
//! - `flipflop`: four state changes going back and forth between two states inside 3 s with the root staying within
//!   0.25 m.
//! - `feet-air`: resting in a wall stance where neither of AC1's foot rays for a hang (`sub_DE0A60`: 1 m under the
//!   hold, 0.5 m out, 0.1 m to either side, 1.2 m in) meets the wall; `free-on-wall`: resting in a free hang where both
//!   do (AC1 hangs on the wall there).
//! - `no-grab`: a ground or run start that never got onto the wall (nor 0.5 m up it); `death`: a landing that killed him.
//! Each run is a line in the TSV (`AC1_SWEEP_OUT=<file>`, default `climb-sweep.tsv`): its id, program, start (as
//! `AC1_START`, to replay it), the classes with where and when, how it ended; then the counts by class are logged and
//! the game exits. `AC1_SWEEP_NEAR="x,z,r"` keeps to the walls within `r` m of (x, z); `AC1_SWEEP_PROGS=up,side,..`
//! runs only those programs; `AC1_SWEEP_SEED=k` picks another spread; `AC1_SWEEP_RECORD=dir` writes each run's frames
//! (the flight recorder's lines) to `dir/<id>.txt`. Run it headless:
//! `AC1_HEADLESS=1 AC1_LEVEL=damascus AC1_CLIMB_SWEEP=30 cargo run --release -p ac1` (docs/PARKOUR.md "The climbing
//! sweep").

use crate::character::{Character, Controller, Player};
use crate::level::{Ledge, Level};
use crate::recorder::Recorder;
use bevy::prelude::*;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// A run's script, as a control run's settings.
struct Run {
    id: String,
    prog: &'static str,
    line: String,
    secs: f32,
    start: String,
}

#[derive(Default)]
struct Measure {
    t: f32,
    state: String,
    clip: String,
    clip_since: f32,
    climbed: bool,
    /// Hanging since last on something to stand on.
    on_wall: bool,
    /// The classes found, with the first time, where and a detail.
    found: BTreeMap<&'static str, (f32, Vec3, String)>,
    /// Root positions over the last seconds, with whether the stick was held (and which way).
    trail: Vec<(f32, Vec3, Vec2)>,
    changes: Vec<(f32, String, Vec3)>,
    hands_bad: f32,
    hands_worst: f32,
    embed_frames: u32,
    pops_seen: usize,
    best_up: f32,
    from_y: f32,
}

impl Measure {
    fn add(&mut self, class: &'static str, t: f32, at: Vec3, detail: String) {
        self.found.entry(class).or_insert((t, at, detail));
    }
}

#[derive(Resource, Default)]
pub struct Sweep {
    ready: bool,
    runs: Vec<Run>,
    i: usize,
    /// The run under way (its line handed to `control_runs`).
    going: bool,
    m: Measure,
    holds: Holds,
    tsv: String,
    counts: BTreeMap<&'static str, usize>,
    by_prog: BTreeMap<(&'static str, &'static str), usize>,
    /// The next control line, taken by `control_runs`.
    pub pending: Option<String>,
}

/// Seconds after a run's end before the next starts (the control run ends on its own clock).
const GAP: f32 = 0.1;
/// A wall's first hold this high over the ground in front (m): within reach of the climb's start from standing (ours:
/// the entry clip's hands 1.74 m up, a hold 0.3 m either way; Banned445's port reads AC1's as up to 2.6 m, unconfirmed).
const ENTRY_UP: (f32, f32) = (1.5, 2.05);
/// Walls this far apart at least (m).
const SPACING: f32 = 8.0;

/// How deep inside a solid counts as in it (m: grazing a face is not).
const EMBED_DEPTH: f32 = 0.08;

/// How far `p` is inside a solid (`Level::inside_solid`): the nearest way out, along the six axes (m); 0 outside.
fn depth_inside(level: &Level, p: Vec3) -> f32 {
    if !level.inside_solid(p) {
        return 0.0;
    }
    [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z]
        .iter()
        .map(|d| level.raycast(p, *d, 2.0).map_or(2.0, |h| h.dist))
        .fold(f32::MAX, f32::min)
}

/// Hanging by the hands (on the wall's holds, in a free hang or a wall hang).
fn hanging(state: &str) -> bool {
    matches!(state, "1m" | "2m" | "1lu" | "1ru" | "2lu" | "2ru") || state.starts_with("free_") || state.starts_with("hangwall")
}

fn wall_stance(state: &str) -> bool {
    matches!(state, "1m" | "2m" | "1lu" | "1ru" | "2lu" | "2ru")
}

/// Bevy yaw (about +Y) that faces `f`.
fn yaw_of(f: Vec3) -> f32 {
    (-f.x).atan2(-f.z).to_degrees()
}

/// A small deterministic hash (the spread of walls picked).
fn mix(i: usize, seed: u64) -> u64 {
    let mut x = (i as u64 ^ seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)).wrapping_add(0x632B_E59B_D9B4_E019);
    x ^= x >> 33;
    x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    x ^= x >> 33;
    x
}

/// The level's holds by 2 m cell (x, z), each in every cell within 1 m of it: a hold within 1 m of a point is in the
/// point's cell.
#[derive(Default)]
struct Holds(std::collections::HashMap<(i32, i32), Vec<u32>>);

impl Holds {
    fn new(level: &Level) -> Self {
        let mut g: std::collections::HashMap<(i32, i32), Vec<u32>> = Default::default();
        for (i, l) in level.ledges.iter().enumerate() {
            let (lo, hi) = (l.a.min(l.b) - Vec3::ONE, l.a.max(l.b) + Vec3::ONE);
            for x in (lo.x / 2.0).floor() as i32..=(hi.x / 2.0).floor() as i32 {
                for z in (lo.z / 2.0).floor() as i32..=(hi.z / 2.0).floor() as i32 {
                    g.entry((x, z)).or_default().push(i as u32);
                }
            }
        }
        Holds(g)
    }
    fn near<'a>(&'a self, level: &'a Level, p: Vec3) -> impl Iterator<Item = &'a Ledge> {
        self.0.get(&((p.x / 2.0).floor() as i32, (p.z / 2.0).floor() as i32)).into_iter().flatten().map(|&i| &level.ledges[i as usize])
    }
}

/// Distance from `p` to the nearest hold facing `n`'s way (m; a hold over 1 m away may be missed).
fn hold_dist(level: &Level, holds: &Holds, p: Vec3, n: Vec3) -> (f32, Option<Vec3>) {
    let mut best = (f32::MAX, None);
    for l in holds.near(level, p) {
        if l.out.dot(n) < 0.5 {
            continue;
        }
        let c = l.closest(p);
        let d = c.distance(p);
        if d < best.0 {
            best = (d, Some(c));
        }
    }
    best
}

/// A hold facing `n`'s way that the hands at `mid` could go on to along `dir` (world), as AC1's climbing grid keeps a cell's
/// hold: within 0.3 m along the wall and 0.15 m up or down of a point a column (0.75 m) to the side, or one or two rows
/// (0.6, 1.2 m) up or down.
fn hold_ahead(level: &Level, holds: &Holds, mid: Vec3, n: Vec3, dir: Vec3) -> bool {
    let along = Vec3::Y.cross(n).normalize_or_zero();
    let steps: &[f32] = if dir.y.abs() > 0.7 { &[0.6, 1.2] } else { &[0.75] };
    steps.iter().map(|k| mid + dir * *k).any(|s| {
        holds.near(level, s).filter(|l| l.out.dot(n) > 0.7).any(|l| {
            let d = l.closest(s) - s;
            d.dot(along).abs() <= 0.3 && d.y.abs() <= 0.15 && d.dot(n).abs() <= 0.5
        })
    })
}

/// A walkable top right behind the hold at `hold` (the wall's top: a roof or a ledge to climb onto).
fn top_behind(level: &Level, hold: Vec3, n: Vec3) -> bool {
    let behind = hold - n * 0.5;
    level.ground(behind.with_y(hold.y + 1.0), 0.0, 1.4).is_some_and(|g| (g.point.y - hold.y).abs() < 0.3 && g.normal.y > 0.7)
}

fn pick_spots(level: &Level, n: usize) -> (Vec<(Vec3, Vec3, Ledge)>, Vec<(Vec3, Vec3)>) {
    let seed = std::env::var("AC1_SWEEP_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
    let near: Option<(Vec2, f32)> = std::env::var("AC1_SWEEP_NEAR")
        .ok()
        .map(|s| s.split(',').filter_map(|v| v.trim().parse().ok()).collect::<Vec<f32>>())
        .filter(|v| v.len() >= 3)
        .map(|v| (Vec2::new(v[0], v[1]), v[2]));
    let mut order: Vec<usize> = (0..level.ledges.len()).collect();
    order.sort_by_key(|&i| mix(i, seed));
    let (mut walls, mut edges): (Vec<(Vec3, Vec3, Ledge)>, Vec<(Vec3, Vec3)>) = (vec![], vec![]);
    for i in order {
        if walls.len() >= n && edges.len() >= n / 2 {
            break;
        }
        let l = &level.ledges[i];
        if l.a.distance(l.b) < 0.6 || l.out.y.abs() > 0.3 {
            continue;
        }
        let mid = (l.a + l.b) * 0.5;
        if near.is_some_and(|(c, r)| Vec2::new(mid.x, mid.z).distance(c) > r) {
            continue;
        }
        let out = l.out.with_y(0.0).normalize_or_zero();
        // A wall: stood on the ground below the hold, facing it.
        if walls.len() < n
            && let Some(g) = level.ground(mid + out * 0.55, 0.0, 3.0)
        {
            let up = mid.y - g.point.y;
            let room = level.raycast(g.point + Vec3::Y * 0.1, Vec3::Y, 1.8).is_none();
            let wall = level.raycast(g.point + Vec3::Y * 1.0, -out, 0.9).is_some();
            let free = !level.inside_solid(g.point + Vec3::Y * 1.0) && level.capsule_push(g.point).length() < 0.05;
            if (ENTRY_UP.0..ENTRY_UP.1).contains(&up) && room && wall && free && walls.iter().all(|w| w.0.distance(g.point) > SPACING) {
                walls.push((g.point, out, l.clone()));
                continue;
            }
        }
        // A roof edge: walkable top behind the hold, and a drop in front.
        if edges.len() < n / 2 && top_behind(level, mid, out) {
            let Some(top) = level.ground((mid - out * 1.0).with_y(mid.y + 1.0), 0.0, 1.4) else { continue };
            let below = level.ground(mid + out * 0.6, -0.2, 30.0).map_or(30.0, |g| mid.y - g.point.y);
            let clear = level.raycast(top.point + Vec3::Y * 1.0, out, 1.2).is_none() && level.raycast(top.point + Vec3::Y * 0.1, Vec3::Y, 1.8).is_none();
            if below > 2.5
                && clear
                && !level.inside_solid(top.point + Vec3::Y * 1.0)
                && (top.point.y - mid.y).abs() < 0.3
                && edges.iter().all(|e| e.0.distance(top.point) > SPACING)
            {
                edges.push((top.point, out));
            }
        }
    }
    (walls, edges)
}

fn build_runs(level: &Level, n: usize) -> Vec<Run> {
    let (walls, edges) = pick_spots(level, n);
    let only: Option<Vec<String>> = std::env::var("AC1_SWEEP_PROGS").ok().map(|s| s.split(',').map(|p| p.trim().to_string()).collect());
    let want = |p: &str| only.as_ref().is_none_or(|o| o.iter().any(|x| x == p));
    let mut runs = vec![];
    let start = |p: Vec3, f: Vec3| format!("{:.2},{:.2},{:.1},{:.2}", p.x, p.z, yaw_of(f), p.y + 0.3);
    for (k, (at, out, _)) in walls.iter().enumerate() {
        let face = -*out;
        let s = start(*at, face);
        let progs: [(&'static str, f32, String); 4] = [
            ("up", 18.0, "AC1_CLIMB=up=16".into()),
            ("side", 17.0, "AC1_CLIMB=up=1.2,left=5,right=9".into()),
            ("down", 11.0, "AC1_CLIMB=up=3,down=6".into()),
            ("drop", 7.5, "AC1_CLIMB=up=2.5,drop=3".into()),
        ];
        for (prog, secs, rest) in progs {
            if want(prog) {
                let id = format!("w{k}-{prog}");
                runs.push(Run { line: format!("run {id} {secs} AC1_START={s} {rest}"), id, prog, secs, start: s.clone() });
            }
        }
        // From 4.5 m out at a run (open, level ground there and nothing in the way).
        let back = *at + *out * 4.5;
        let open = level.raycast(*at + Vec3::Y * 1.0 + *out * 0.3, *out, 4.3).is_none()
            && level.ground(back, 0.5, 0.5).is_some_and(|g| (g.point.y - at.y).abs() < 0.3);
        if want("run") && open {
            let id = format!("w{k}-run");
            let s = start(back, face);
            runs.push(Run {
                line: format!("run {id} 13 AC1_START={s} AC1_WALK=6.2 AC1_HIGH=0-1.8 AC1_LEGS=0-1.8 AC1_STOP=1.8 AC1_CLIMB=nograb,wait=2.5,up=10"),
                id,
                prog: "run",
                secs: 13.0,
                start: s,
            });
        }
    }
    for (k, (top, out)) in edges.iter().enumerate() {
        if want("hang") {
            let id = format!("e{k}-hang");
            let s = start(*top, *out);
            runs.push(Run {
                line: format!("run {id} 16 AC1_START={s} AC1_WALK=1.5 AC1_STOP=1.0-60 AC1_HAND=1.8 AC1_CLIMB=nograb,wait=3.5,left=3,right=5,up=4"),
                id,
                prog: "hang",
                secs: 16.0,
                start: s,
            });
        }
    }
    info!("sweep: {} walls, {} roof edges, {} runs", walls.len(), edges.len(), runs.len());
    runs
}

/// Hands the next run to `control_runs` (before it in the frame).
pub fn sweep_issue(level: Res<Level>, mut sweep: ResMut<Sweep>, mut exit: MessageWriter<AppExit>) {
    let Some(n) = std::env::var("AC1_CLIMB_SWEEP").ok().and_then(|s| s.parse::<usize>().ok()) else { return };
    if !sweep.ready {
        sweep.ready = true;
        sweep.runs = build_runs(&level, n);
        sweep.holds = Holds::new(&level);
        sweep.tsv = "id\tprog\tstart\tclasses\tend\tup\tdetails\n".into();
    }
    if sweep.going || sweep.pending.is_some() {
        return;
    }
    let Some(run) = sweep.runs.get(sweep.i) else {
        let path = std::env::var("AC1_SWEEP_OUT").unwrap_or_else(|_| "climb-sweep.tsv".into());
        if let Err(e) = std::fs::write(&path, &sweep.tsv) {
            warn!("sweep: {path}: {e}");
        }
        let mut s = String::new();
        for (c, k) in &sweep.counts {
            let _ = write!(s, " {c} {k}");
        }
        info!("sweep: {} runs done, written to {path}; runs with each class:{s}", sweep.runs.len());
        for ((p, c), k) in &sweep.by_prog {
            info!("sweep: {p} {c} {k}");
        }
        exit.write(AppExit::Success);
        return;
    };
    // (`AC1_SWEEP_RECORD=dir`: every run's frames written there by the flight recorder, `<id>.txt`.)
    let rec = std::env::var("AC1_SWEEP_RECORD").map(|d| format!(" AC1_RECORD={d}/{}.txt", run.id)).unwrap_or_default();
    sweep.pending = Some(format!("{}{rec}", run.line));
    sweep.going = true;
    sweep.m = Measure { t: -GAP, ..default() };
}

/// Checks each frame of the run under way (after the bones are placed), and closes it at its end.
#[allow(clippy::too_many_arguments)]
pub fn sweep_measure(
    time: Res<Time>,
    level: Res<Level>,
    mut sweep: ResMut<Sweep>,
    rec: Res<Recorder>,
    q: Query<(&Transform, &Controller, &Character), With<Player>>,
    globals: Query<&GlobalTransform>,
) {
    if !sweep.going || sweep.pending.is_some() {
        return;
    }
    let Ok((tf, ctl, ch)) = q.single() else { return };
    let dt = time.delta_secs().min(0.05);
    let Some(run) = sweep.runs.get(sweep.i) else { return };
    let (prog, secs, id, start) = (run.prog, run.secs, run.id.clone(), run.start.clone());
    let Sweep { m, holds, .. } = &mut *sweep;
    let holds = &*holds;
    m.t += dt;
    let t = m.t;
    let root = tf.translation;
    if t <= 0.05 {
        m.from_y = root.y;
        m.pops_seen = rec.pops.len();
        m.clip_since = t;
    }
    let bone = |name: &str| ch.joint_named(name).and_then(|e| globals.get(e).ok()).map(|g| g.translation());
    let (state, clip) = match &ch.wall {
        Some(w) => (w.state.clone(), w.clip_name().unwrap_or("").to_string()),
        None => ("ground".to_string(), String::new()),
    };
    let n = ch.wall.as_ref().map_or(Vec3::Z, |w| w.normal);
    m.best_up = m.best_up.max(root.y - m.from_y);
    if clip != m.clip {
        m.clip = clip.clone();
        m.clip_since = t;
    }
    if state != m.state {
        // Off the wall into a fall, unasked (a fall after the last hang with nothing stood on between: the let go's own
        // clip is `drop`, which a pull-down passes through too).
        if hanging(&state) {
            m.on_wall = true;
        } else if matches!(state.as_str(), "ground" | "top" | "perch" | "land" | "ground_act" | "hay" | "kneel" | "hangknee" | "ladder_l" | "ladder_r") {
            m.on_wall = false;
        }
        if m.on_wall && state == "fall" {
            let over = level.ground(root, 0.0, 60.0).map_or(60.0, |g| root.y - g.point.y);
            let asked = prog == "drop" && t > 3.8;
            if !asked {
                let down = ctl.climb_dir.y < -0.5;
                if !down {
                    m.add("letgo", t, root, format!("{} -> {state} via {clip}, {over:.1} m up", m.state));
                    m.on_wall = false;
                } else if over > 1.2 {
                    m.add("letgo-down", t, root, format!("{} -> {state} via {clip}, {over:.1} m up", m.state));
                }
            }
        }
        m.changes.push((t, state.clone(), root));
        if m.changes.len() > 5 {
            m.changes.remove(0);
        }
        let c = &m.changes;
        if c.len() == 5 && c[4].0 - c[0].0 < 3.0 && c[4].1 == c[2].1 && c[2].1 == c[0].1 && c[3].1 == c[1].1 && c.iter().all(|x| x.2.distance(c[0].2) < 0.25) {
            let detail = format!("{} <-> {}", c[0].1, c[1].1);
            m.add("flipflop", t, root, detail);
        }
        m.state = state.clone();
    }
    if hanging(&state) {
        m.climbed = true;
    }
    if ch.wall.as_ref().is_some_and(|w| w.dead) {
        m.add("death", t, root, clip.clone());
    }
    // The pose check's pops.
    if rec.pops.len() > m.pops_seen {
        for (d, text) in &rec.pops[m.pops_seen..] {
            if *d > 0.15 {
                m.found.entry("pop").or_insert((t, root, text.clone()));
            }
        }
        m.pops_seen = rec.pops.len();
    }
    // Inside a solid.
    let hips = bone("Hips").unwrap_or(root + Vec3::Y);
    if [hips, root + Vec3::Y * 1.2].iter().any(|p| depth_inside(&level, *p) > EMBED_DEPTH) {
        m.embed_frames += 1;
        if m.embed_frames >= 3 {
            m.add("embed", t, root, format!("{state} {clip}"));
        }
    } else {
        m.embed_frames = 0;
    }
    let knuckles = [bone("LeftHandMiddle1"), bone("RightHandMiddle1")];
    // (Resting: a hang's wait loop, `xx_climb_wait_1m`, `xx_h_hangwall_wait`, `xx_h_hangfree_waitclose`; not a move into a
    // wait, `..._tr_l_ladder_wait_r_left`, nor a look round, which takes a hand off.)
    let resting = hanging(&state) && clip.contains("wait") && !clip.contains("_tr_") && !clip.contains("lookaround") && t - m.clip_since > 0.3;
    if resting && let [Some(l), Some(r)] = knuckles {
        let worst = [l, r].iter().map(|k| hold_dist(&level, holds, *k, n).0).fold(0.0, f32::max);
        if worst > 0.15 {
            m.hands_bad += dt;
            m.hands_worst = m.hands_worst.max(worst);
            if m.hands_bad > 0.25 {
                let class = if worst > 0.4 { "no-hold" } else { "hands-off" };
                m.add(class, t, root, format!("{worst:.2} m in {state} {clip}"));
            }
        } else {
            m.hands_bad = 0.0;
        }
        // The feet against the wall (or not), by AC1's foot rays for a hang (`sub_DE0A60`): from 1 m under the hands'
        // hold, 0.5 m out and 0.1 m to either side, 1.2 m in.
        let hold = hold_dist(&level, holds, (l + r) * 0.5, n).1.unwrap_or((l + r) * 0.5);
        let along = Vec3::Y.cross(n).normalize_or_zero();
        let ray = |s: f32| level.raycast(hold - Vec3::Y + n * 0.5 + along * (0.1 * s), -n, 1.2).is_some_and(|h| h.normal.y.abs() < 0.7);
        let walls = usize::from(ray(-1.0)) + usize::from(ray(1.0));
        if wall_stance(&state) && walls == 0 {
            m.add("feet-air", t, root, format!("{state} {clip}"));
        }
        if state.starts_with("free_") && walls == 2 {
            m.add("free-on-wall", t, root, format!("{state} {clip}"));
        }
    } else {
        m.hands_bad = 0.0;
    }
    // Stuck: the stick held, hanging, no progress.
    m.trail.push((t, root, if hanging(&state) { ctl.climb_dir } else { Vec2::ZERO }));
    m.trail.retain(|(s, _, _)| t - s <= 2.5);
    let held = m.trail.first().is_some_and(|f| t - f.0 >= 2.45) && m.trail.iter().all(|x| x.2.length() > 0.5 && x.2.distance(ctl.climb_dir) < 0.1);
    if held && let [Some(l), Some(r)] = knuckles {
        let moved = m.trail.iter().map(|x| x.1.distance(root)).fold(0.0, f32::max);
        if moved < 0.15 {
            let right = (-n).cross(Vec3::Y).normalize_or_zero();
            let dir = (right * ctl.climb_dir.x + Vec3::Y * ctl.climb_dir.y).normalize_or_zero();
            let mid = (l + r) * 0.5;
            let at = hold_dist(&level, holds, mid, n).1.unwrap_or(mid);
            if ctl.climb_dir.y > 0.5 && top_behind(&level, at, n) {
                m.add("topout", t, root, format!("{state} {clip}, hold at {at:.2}"));
            } else if hold_ahead(&level, holds, mid, n, dir) {
                m.add("stuck", t, root, format!("{state} {clip}, stick {:.1},{:.1}", ctl.climb_dir.x, ctl.climb_dir.y));
            }
        }
    }
    if t < secs {
        return;
    }
    // The run's end.
    if !m.climbed && prog != "hang" && m.best_up < 0.5 {
        m.add("no-grab", t, root, format!("ends {state}"));
    }
    let classes: Vec<&str> = m.found.keys().copied().collect();
    let mut details = String::new();
    for (c, (ft, at, d)) in &m.found {
        let _ = write!(details, "{c} at {ft:.1} s [{:.2}, {:.2}, {:.2}]: {d}; ", at.x, at.y, at.z);
    }
    let line = format!("{}\t{}\t{}\t{}\t{}\t{:.1}\t{}\n", id, prog, start, classes.join(","), state, m.best_up, details);
    info!("sweep {id}: {} | ends {state} {:.1} m up | {details}", if classes.is_empty() { "ok".into() } else { classes.join(",") }, m.best_up);
    let found: Vec<&'static str> = m.found.keys().copied().collect();
    sweep.tsv.push_str(&line);
    for c in found {
        *sweep.counts.entry(c).or_default() += 1;
        *sweep.by_prog.entry((prog, c)).or_default() += 1;
    }
    sweep.i += 1;
    sweep.going = false;
}
