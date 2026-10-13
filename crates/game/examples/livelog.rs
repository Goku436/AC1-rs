//! livelog: reads ac1-hook's live log of the real AC1 (`ac1-live.txt` beside its exe, the flight recorder's line
//! format, see `crates/hook/src/livefmt.rs`) and the flight recorder's own files (F9, `ac1-recording-<ms>.txt`).
//!
//! - `livelog tail [file] [--for secs] [--game dir]`: follow the live file (and its events file) as the game writes
//!   them, a summary line a second and every event as it comes; follows the files starting over.
//! - `livelog summary [file] [--last secs] [--game dir]`: the whole file (or its last seconds): length, frame rate,
//!   distance, speeds, time in each mode, the mode switches, the actions most played, what the hook could read (bones,
//!   IK goals, input, camera) and checks of it (bones near the root, the camera looking at the player).
//! - `livelog compare <real live log> <ours recording> [--from s] [--to s] [--offset s] [--step s]`: the real game's
//!   run and ours' of the same route side by side: both from when the root first moves (or ours shifted by `--offset`),
//!   the gap between the roots, each one's mode and clip, then the mean / largest / final gaps and when each switch of
//!   mode happens in each (`--from` / `--to` cut the real log, in its own seconds), then the poses: for each clip both
//!   play (ours' named by their action with `--game`, as the live log names a blend by its first clip), the bones
//!   relative to the root in the body's frame and the limbs' distance to their IK goals. Ours' side is an F9 recording
//!   or a control run's `AC1_RECORD=<file>` (every frame of the run).
//! - `livelog runs [dir]`: the scripted runs in a folder of live logs (`ac1-live.2.txt`, `.1`, then `ac1-live.txt`,
//!   oldest first): the log split where the root jumps (a teleport), each part with its start (game space), facing,
//!   when the input starts and ends in it, where it ends and its modes; for finding a route's frames to `cut`.
//! - `livelog cut <out> --from s --to s [--dir dir]`: the frames (and events, into `<out>` with `.events` before its
//!   extension) between two times of the live logs in `dir` (default `AC1_GAME_DIR`), out of the rotated files: one
//!   route's real run, to `compare`.
//! - `livelog names [game dir] [out]`: the action names table for the hook (`ac1-hook.actions.tsv` in the game
//!   folder by default; with it the hook writes clip names, without it ids, named here with `--game`).
//!
//! The file defaults to `ac1-live.txt` in `AC1_GAME_DIR` (else the usual install path).
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const DEFAULT_GAME: &str = r"P:\SteamLibrary\steamapps\common\Assassins Creed";

/// One line of the recorder's format.
#[derive(Clone, Debug, Default, PartialEq)]
struct Row {
    t: f32,
    root: [f32; 3],
    yaw: f32,
    vel: f32,
    /// The state part (`ground`, `climb <state> clip <clip>`, `HumanGround clip <clip>`).
    state: String,
    bones: Vec<(String, [f32; 3])>,
    targets: Vec<[f32; 3]>,
    /// The live log's `ac1 key=value` part.
    kv: HashMap<String, String>,
    limp: bool,
}

fn v3(s: &str) -> Option<[f32; 3]> {
    let mut it = s.split(',').map(|x| x.parse::<f32>().ok());
    Some([it.next()??, it.next()??, it.next()??])
}

fn parse(line: &str) -> Option<Row> {
    let mut parts = line.split(" | ");
    let head: Vec<&str> = parts.next()?.split_whitespace().collect();
    if head.get(1) != Some(&"root") {
        return None;
    }
    let num = |k: usize| head.get(k).and_then(|w| w.parse::<f32>().ok());
    let mut row = Row {
        t: num(0)?,
        root: [num(2)?, num(3)?, num(4)?],
        yaw: if head.get(5) == Some(&"yaw") { num(6).unwrap_or(0.0) } else { 0.0 },
        vel: if head.get(7) == Some(&"vel") { num(8).unwrap_or(0.0) } else { 0.0 },
        state: parts.next()?.trim().to_string(),
        ..Default::default()
    };
    for part in parts {
        let part = part.trim();
        if let Some(b) = part.strip_prefix("bones") {
            let w: Vec<&str> = b.split_whitespace().collect();
            row.bones = w.chunks(2).filter_map(|c| Some((c.first()?.to_string(), v3(c.get(1)?)?))).collect();
        } else if let Some(t) = part.strip_prefix("targets") {
            row.targets = t.split_whitespace().filter_map(v3).collect();
        } else if let Some(a) = part.strip_prefix("ac1 ") {
            row.kv = a.split_whitespace().filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect();
        } else if part == "limp" {
            row.limp = true;
        }
    }
    Some(row)
}

/// The kind of movement, the same words for both games: AC1's context (the live log's `mode=`) or ours' state.
fn mode(row: &Row) -> String {
    if let Some(m) = row.kv.get("mode") {
        return m.clone();
    }
    if row.limp {
        return "limp".into();
    }
    let mut w = row.state.split_whitespace();
    match (w.next(), w.next()) {
        (Some("ground"), _) => "ground".into(),
        (Some("climb"), Some(s)) => mode_of_ours(s).into(),
        (Some(other), _) => other.into(),
        _ => "?".into(),
    }
}

/// Ours' climb states (`crates/game/src/climb.rs`) by the context AC1 plays them in.
fn mode_of_ours(state: &str) -> &'static str {
    match state {
        "ground" | "ground_act" | "collide" | "lean" | "land" | "hangknee" | "bench" | "bench_sit" | "kiosk" => "ground",
        "jump" | "fall" | "leap" | "faith" | "drop" | "vault" => "air",
        "free_1m" | "free_open" | "hangwall" | "hangwall_open" => "hang",
        "wallrun" => "wallrun",
        "perch" | "crouch" => "beam",
        "ladder_l" | "ladder_r" => "ladder",
        "hay" | "hayout" => "hay",
        "swing" => "swing",
        "dead" => "dead",
        s if s.starts_with("beam") => "beam",
        _ => "climb",
    }
}

/// The clip named in the state (`... clip <name>`), named from the table when the log has only an id.
fn clip(row: &Row, names: &Names) -> String {
    let c = row.state.split_once(" clip ").map_or("-", |(_, c)| c.trim());
    if c.len() == 8
        && let Ok(id) = u32::from_str_radix(c, 16)
        && let Some((_, n)) = names.get(&id)
    {
        return n.clone();
    }
    c.to_string()
}

type Names = HashMap<u32, (String, String)>;

fn parse_names(text: &str) -> Names {
    let mut map = Names::new();
    for l in text.lines() {
        let p: Vec<&str> = l.split('\t').collect();
        if let (4, Ok(id)) = (p.len(), u32::from_str_radix(p[0].trim(), 16)) {
            map.entry(id).or_insert_with(|| (p[2].to_string(), p[3].trim().to_string()));
        }
    }
    map
}

/// Every action of `Game Fix`'s blocks as `id<TAB>key<TAB>block<TAB>first clip` lines (as the `forge` example
/// `action_index` prints them).
fn names_table(game: &Path) -> anyhow::Result<String> {
    use anyhow::Context;
    let f = forge::Forge::open(game.join("DataPC.forge"))?;
    let e = f.entries.iter().find(|e| e.name == "Game Fix").context("no Game Fix")?;
    let data = f.read(e)?;
    let objs = forge::parse_objects(&data)?;
    let names: HashMap<u32, String> = objs.iter().map(|o| (o.id, o.name.clone())).collect();
    let mut out = String::new();
    for o in objs.iter().filter(|o| o.class == forge::action::CLASS_ACTION_BLOCK) {
        let Ok(b) = forge::action::parse_block(o.body) else { continue };
        for a in b.actions.iter().filter_map(|s| s.inline()) {
            let clip = a.items.first().and_then(|i| i.clips.first()).map_or("-".to_string(), |c| names.get(c).cloned().unwrap_or(format!("{c:#x}")));
            out.push_str(&format!("{:08x}\t{:08x}\t{}\t{clip}\n", a.id, a.key, o.name));
        }
    }
    Ok(out)
}

fn load_names(game: Option<&Path>) -> Names {
    let Some(g) = game else { return Names::new() };
    if let Ok(t) = std::fs::read_to_string(g.join("ac1-hook.actions.tsv")) {
        return parse_names(&t);
    }
    names_table(g).map(|t| parse_names(&t)).unwrap_or_default()
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// The mode switches: (time, from, to).
fn switches(rows: &[Row]) -> Vec<(f32, String, String)> {
    let mut out = Vec::new();
    for w in rows.windows(2) {
        let (a, b) = (mode(&w[0]), mode(&w[1]));
        if a != b {
            out.push((w[1].t, a, b));
        }
    }
    out
}

/// The angle (degrees) between where the camera looks and the player, from the live log's `cam=` / `camdir=`.
fn camera_angle(row: &Row) -> Option<f32> {
    let cam = v3(row.kv.get("cam")?)?;
    let dir = v3(row.kv.get("camdir")?)?;
    let to = [row.root[0] - cam[0], row.root[1] + 1.0 - cam[1], row.root[2] - cam[2]];
    let (l1, l2) = (dist(to, [0.0; 3]), dist(dir, [0.0; 3]));
    (l1 > 1e-3 && l2 > 1e-3).then(|| ((to[0] * dir[0] + to[1] * dir[1] + to[2] * dir[2]) / (l1 * l2)).clamp(-1.0, 1.0).acos().to_degrees())
}

fn summary(rows: &[Row], names: &Names) -> String {
    let mut s = String::new();
    let (Some(first), Some(last)) = (rows.first(), rows.last()) else { return "no frames\n".into() };
    let secs = last.t - first.t;
    let wall = |r: &Row| r.kv.get("wall").and_then(|w| w.parse::<f32>().ok());
    let fps = match (wall(first), wall(last)) {
        (Some(a), Some(b)) if b > a => rows.len() as f32 / (b - a),
        _ if secs > 0.0 => rows.len() as f32 / secs,
        _ => 0.0,
    };
    let travelled: f32 = rows.windows(2).map(|w| dist(w[0].root, w[1].root)).filter(|d| *d < 3.0).sum();
    let top = rows.iter().map(|r| r.vel).fold(0.0, f32::max);
    s += &format!(
        "{} frames, {secs:.2} s ({:.2} to {:.2}), {fps:.1} frames/s; {travelled:.1} m travelled, top speed {top:.2} m/s\n",
        rows.len(),
        first.t,
        last.t
    );
    s += &format!(
        "from {:.2},{:.2},{:.2} to {:.2},{:.2},{:.2} (Bevy space; game {:.2} {:.2} {:.2})\n",
        first.root[0], first.root[1], first.root[2], last.root[0], last.root[1], last.root[2], last.root[0], -last.root[2], last.root[1]
    );
    let mut time_in: BTreeMap<String, f32> = BTreeMap::new();
    for w in rows.windows(2) {
        *time_in.entry(mode(&w[0])).or_default() += w[1].t - w[0].t;
    }
    let modes: Vec<String> = time_in.iter().map(|(m, t)| format!("{m} {t:.2} s")).collect();
    s += &format!("time in: {}\n", modes.join(", "));
    let sw = switches(rows);
    if !sw.is_empty() {
        s += "switches:\n";
        for (t, a, b) in sw.iter().take(40) {
            s += &format!("  {t:8.3} {a} -> {b}\n");
        }
        if sw.len() > 40 {
            s += &format!("  ... {} more\n", sw.len() - 40);
        }
    }
    let mut played: BTreeMap<String, usize> = BTreeMap::new();
    for r in rows {
        if let Some(st) = r.kv.get("started") {
            for a in st.split(',') {
                let (id, n) = a.split_once(':').unwrap_or((a, a));
                let name = u32::from_str_radix(id, 16).ok().and_then(|i| names.get(&i)).map_or(n.to_string(), |(_, c)| c.clone());
                *played.entry(name).or_default() += 1;
            }
        }
    }
    if !played.is_empty() {
        let mut v: Vec<_> = played.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let top: Vec<String> = v.iter().take(15).map(|(n, k)| format!("{n} x{k}")).collect();
        s += &format!("actions started: {}\n", top.join(", "));
    }
    let pct = |k: usize| 100.0 * k as f32 / rows.len() as f32;
    let with_bones = rows.iter().filter(|r| !r.bones.is_empty()).count();
    let with_targets = rows.iter().filter(|r| !r.targets.is_empty()).count();
    let with_input = rows.iter().filter(|r| r.kv.contains_key("stick")).count();
    let angles: Vec<f32> = rows.iter().filter_map(camera_angle).collect();
    s += &format!("read: bones {:.0}%, IK goals {:.0}%, input {:.0}%, camera {:.0}%\n", pct(with_bones), pct(with_targets), pct(with_input), pct(angles.len()));
    if with_bones > 0 {
        let far: Vec<f32> = rows.iter().flat_map(|r| r.bones.iter().map(|(_, b)| dist(*b, r.root))).collect();
        let mean = far.iter().sum::<f32>() / far.len() as f32;
        let hips: Vec<f32> = rows.iter().filter_map(|r| r.bones.iter().find(|b| b.0 == "Hips").map(|b| b.1[1] - r.root[1])).collect();
        let hip = if hips.is_empty() { f32::NAN } else { hips.iter().sum::<f32>() / hips.len() as f32 };
        s += &format!("check: bones {mean:.2} m from the root on average (expect about 1), the hips {hip:.2} m over it (expect about 0.95 standing)\n");
    }
    if !angles.is_empty() {
        let mean = angles.iter().sum::<f32>() / angles.len() as f32;
        s += &format!("check: the camera looks {mean:.1} degrees off the player on average (expect under about 20)\n");
    }
    s
}

/// The rows from when the root first moves 5 cm from where it starts, timed from then.
fn from_onset(rows: &[Row]) -> Vec<Row> {
    let Some(first) = rows.first() else { return Vec::new() };
    let k = rows.iter().position(|r| dist(r.root, first.root) > 0.05).unwrap_or(0).saturating_sub(1);
    let t0 = rows[k].t;
    rows[k..].iter().map(|r| Row { t: r.t - t0, ..r.clone() }).collect()
}

/// The row at time `t` (the last one at or before it), its root moved on linearly toward the next.
fn at(rows: &[Row], t: f32) -> Option<([f32; 3], &Row)> {
    let k = rows.partition_point(|r| r.t <= t);
    let a = rows.get(k.checked_sub(1)?)?;
    let Some(b) = rows.get(k) else { return Some((a.root, a)) };
    let f = if b.t > a.t { ((t - a.t) / (b.t - a.t)).clamp(0.0, 1.0) } else { 0.0 };
    Some((std::array::from_fn(|i| a.root[i] + (b.root[i] - a.root[i]) * f), a))
}

/// The table, and the gaps the tests check.
#[cfg_attr(not(test), allow(dead_code))]
struct Compare {
    table: String,
    mean: f32,
    max: (f32, f32),
    end: f32,
}

/// Every clip of `Game Fix`'s actions to the name the live log gives its action (the action's first clip): the live
/// log names a blend by its first clip (`..._251cm_000cm_a` for a wall run's catch blending 251 and 430 cm), ours names
/// the clip it plays most; set side by side, ours' are shown as their action's.
type ClipActions = HashMap<String, String>;

fn clip_actions(game: &Path) -> anyhow::Result<ClipActions> {
    use anyhow::Context;
    let f = forge::Forge::open(game.join("DataPC.forge"))?;
    let e = f.entries.iter().find(|e| e.name == "Game Fix").context("no Game Fix")?;
    let data = f.read(e)?;
    let objs = forge::parse_objects(&data)?;
    let names: HashMap<u32, String> = objs.iter().map(|o| (o.id, o.name.clone())).collect();
    let mut out = ClipActions::new();
    for o in objs.iter().filter(|o| o.class == forge::action::CLASS_ACTION_BLOCK) {
        let Ok(b) = forge::action::parse_block(o.body) else { continue };
        for a in b.actions.iter().filter_map(|s| s.inline()) {
            let Some(first) = a.items.first().and_then(|i| i.clips.first()).and_then(|c| names.get(c)) else { continue };
            for c in a.items.iter().flat_map(|i| i.clips.iter()).filter_map(|c| names.get(c)) {
                out.entry(c.clone()).or_insert_with(|| first.clone());
            }
        }
    }
    Ok(out)
}

/// The clip as the live log names it: ours' through its action (`clip_actions`).
fn shown(row: &Row, names: &Names, acts: &ClipActions) -> String {
    let c = clip(row, names);
    if row.kv.contains_key("mode") {
        return c;
    }
    acts.get(&c).cloned().unwrap_or(c)
}

const POSE_BONES: [&str; 6] = ["Hips", "Head", "LeftHand", "RightHand", "LeftFoot", "RightFoot"];

/// A world point in the body's frame (forward, up, right of the root, by its yaw: Bevy space, facing -Z at yaw 0).
fn body(root: [f32; 3], yaw: f32, p: [f32; 3]) -> [f32; 3] {
    let y = yaw.to_radians();
    let d = [p[0] - root[0], p[1] - root[1], p[2] - root[2]];
    [-d[0] * y.sin() - d[2] * y.cos(), d[1], d[0] * y.cos() - d[2] * y.sin()]
}

/// The poses side by side: for each clip (by action) both games play long enough, the mean of each bone relative to the
/// root in the body's frame, real and ours, and how far each limb is from its IK goal where a game logs them; the
/// clips whose bones differ most first. (Ours' recorder writes the bones a frame behind the root: taken against the
/// frame before's.)
fn poses(real: &[Row], ours: &[Row], names: &Names, acts: &ClipActions, min_frames: usize) -> String {
    #[derive(Default)]
    struct Acc {
        n: usize,
        b: [[f32; 3]; 6],
        goal: [(f32, usize); 4],
    }
    let gather = |rows: &[Row], behind: bool| {
        let mut m: BTreeMap<String, Acc> = BTreeMap::new();
        for (k, r) in rows.iter().enumerate() {
            let c = shown(r, names, acts);
            let bones: Vec<[f32; 3]> = POSE_BONES.iter().filter_map(|n| r.bones.iter().find(|b| b.0 == *n).map(|b| b.1)).collect();
            if c == "-" || bones.len() < 6 {
                continue;
            }
            let at = if behind && k > 0 { &rows[k - 1] } else { r };
            let e = m.entry(c).or_default();
            e.n += 1;
            for (i, p) in bones.iter().enumerate() {
                let v = body(at.root, at.yaw, *p);
                for (s, x) in e.b[i].iter_mut().zip(v) {
                    *s += x;
                }
            }
            for (i, g) in r.targets.iter().take(4).enumerate() {
                e.goal[i].0 += dist(bones[2 + i], *g);
                e.goal[i].1 += 1;
            }
        }
        m
    };
    let (a, b) = (gather(real, false), gather(ours, true));
    let mut found: Vec<(f32, String)> = Vec::new();
    for (c, ea) in &a {
        let Some(eb) = b.get(c).filter(|eb| eb.n >= min_frames && ea.n >= min_frames) else { continue };
        let mean = |e: &Acc, i: usize| e.b[i].map(|v| v / e.n as f32);
        let worst = (0..6).map(|i| dist(mean(ea, i), mean(eb, i))).fold(0.0, f32::max);
        let mut s = format!("  {c}: real {} frames, ours {}; worst bone {worst:.2} m\n", ea.n, eb.n);
        for (i, n) in POSE_BONES.iter().enumerate() {
            let (p, q) = (mean(ea, i), mean(eb, i));
            s += &format!("    {n:9} real {:+.2},{:+.2},{:+.2}  ours {:+.2},{:+.2},{:+.2}  {:.2}\n", p[0], p[1], p[2], q[0], q[1], q[2], dist(p, q));
        }
        let goals = |e: &Acc| e.goal.iter().map(|(s, k)| if *k > 0 { format!("{:.2}", s / *k as f32) } else { "-".into() }).collect::<Vec<_>>().join(" ");
        s += &format!("    limbs to IK goals (hands, feet): real {}  ours {}\n", goals(ea), goals(eb));
        found.push((worst, s));
    }
    found.sort_by(|x, y| y.0.total_cmp(&x.0));
    let mut out = String::from("poses by clip (bones from the root: forward, up, right; m):\n");
    for (_, s) in found {
        out += &s;
    }
    out
}

fn compare(real: &[Row], ours: &[Row], offset: Option<f32>, step: f32, names: &Names, acts: &ClipActions) -> Compare {
    let real = from_onset(real);
    let ours: Vec<Row> = match offset {
        Some(o) => ours.iter().map(|r| Row { t: r.t - ours[0].t + o, ..r.clone() }).collect(),
        None => from_onset(ours),
    };
    let end = real.last().map_or(0.0, |r| r.t).min(ours.last().map_or(0.0, |r| r.t));
    let mut table = format!("{:>7} {:>26} {:>26} {:>6}  real | ours\n", "t", "real x,y,z", "ours x,y,z", "gap");
    let (mut sum, mut n, mut max, mut last) = (0.0, 0, (0.0f32, 0.0), 0.0);
    let mut t = 0.0;
    while t <= end + 1e-4 {
        if let (Some((a, ra)), Some((b, rb))) = (at(&real, t), at(&ours, t)) {
            let gap = dist(a, b);
            sum += gap;
            n += 1;
            if gap > max.0 {
                max = (gap, t);
            }
            last = gap;
            let p = |v: [f32; 3]| format!("{:.2},{:.2},{:.2}", v[0], v[1], v[2]);
            table +=
                &format!("{t:7.2} {:>26} {:>26} {gap:6.2}  {} {} | {} {}\n", p(a), p(b), mode(ra), shown(ra, names, acts), mode(rb), shown(rb, names, acts));
        }
        t += step.max(0.01);
    }
    let (sr, so) = (switches(&real), switches(&ours));
    table += "mode switches (real | ours):\n";
    for k in 0..sr.len().max(so.len()) {
        let f = |s: Option<&(f32, String, String)>| s.map_or("-".to_string(), |(t, a, b)| format!("{t:7.2} {a} -> {b}"));
        let d = match (sr.get(k), so.get(k)) {
            (Some(a), Some(b)) if a.2 == b.2 => format!("  ({:+.2} s)", b.0 - a.0),
            _ => String::new(),
        };
        table += &format!("  {:<32} | {}{d}\n", f(sr.get(k)), f(so.get(k)));
    }
    let mean = if n > 0 { sum / n as f32 } else { 0.0 };
    table += &format!("gap: mean {mean:.2} m, largest {:.2} m at {:.2} s, at the end ({end:.2} s) {last:.2} m\n", max.0, max.1);
    table += &poses(&real, &ours, names, acts, 10);
    Compare { table, mean, max, end: last }
}

fn read_rows(path: &Path) -> Vec<Row> {
    match std::fs::read_to_string(path) {
        Ok(t) => t.lines().filter_map(parse).collect(),
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            std::process::exit(1)
        }
    }
}

/// Every line of a live log's files in `dir`, oldest first: `<stem>.2.txt`, `<stem>.1.txt`, `<stem>.txt` (the hook
/// starts a file over past its limit, keeping two old ones).
fn live_lines(dir: &Path, stem: &str) -> impl Iterator<Item = String> {
    use std::io::BufRead;
    let files = [format!("{stem}.2.txt"), format!("{stem}.1.txt"), format!("{stem}.txt")];
    files.into_iter().filter_map(|f| std::fs::File::open(dir.join(f)).ok()).flat_map(|f| std::io::BufReader::new(f).lines().map_while(Result::ok))
}

/// A live log line's time (frames and events both start with it).
fn line_time(l: &str) -> Option<f32> {
    l.split_whitespace().next()?.parse().ok()
}

/// The live log split at its teleports (the root moving over 0.6 m in a frame), the parts with any input: when, where
/// (game space), the facing, the input's span from the part's start, where it ends and its modes.
fn runs(rows: &[Row]) -> String {
    let mut s = String::new();
    let mut parts = Vec::new();
    let mut start = 0;
    for k in 1..rows.len() {
        if dist(rows[k].root, rows[k - 1].root) > 0.6 {
            parts.push(&rows[start..k]);
            start = k;
        }
    }
    parts.push(&rows[start..]);
    let held = |r: &Row| {
        let get = |k: &str| r.kv.get(k).map(String::as_str);
        get("stick").is_some_and(|v| v != "0.00,0.00") || get("btn").is_some_and(|v| v != "0000")
    };
    let game = |p: [f32; 3]| format!("{:.2},{:.2},{:.2}", p[0], -p[2], p[1]);
    for part in parts {
        let (Some(first), Some(last)) = (part.first(), part.last()) else { continue };
        let (Some(a), Some(b)) = (part.iter().find(|r| held(r)), part.iter().rev().find(|r| held(r))) else { continue };
        let mut modes: Vec<String> = Vec::new();
        for r in part {
            let m = mode(r);
            if modes.last() != Some(&m) {
                modes.push(m);
            }
        }
        // (The pad's buttons: bit 0 the legs, bit 12 high profile.)
        let flag = |bit: u16| part.iter().any(|r| r.kv.get("btn").and_then(|v| u16::from_str_radix(v, 16).ok()).is_some_and(|b| b & bit != 0)) as u8;
        s += &format!(
            "{:9.2} to {:9.2}: from game {} facing {:.0} (game {:.0}), input {:.2} to {:.2} s (high {} legs {}), ends {} {:.1} m off | {}
",
            first.t,
            last.t,
            game(first.root),
            first.yaw,
            first.yaw + 90.0,
            a.t - first.t,
            b.t - first.t,
            flag(0x1000),
            flag(1),
            game(last.root),
            dist(first.root, last.root),
            modes.join(" ")
        );
    }
    s
}

/// `--name value` options and the plain arguments.
fn options(args: &[String]) -> (Vec<String>, HashMap<String, String>) {
    let (mut plain, mut opts) = (Vec::new(), HashMap::new());
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.strip_prefix("--") {
            Some(k) => {
                opts.insert(k.to_string(), it.next().cloned().unwrap_or_default());
            }
            None => plain.push(a.clone()),
        }
    }
    (plain, opts)
}

fn game_dir(opts: &HashMap<String, String>) -> PathBuf {
    opts.get("game").cloned().or_else(|| std::env::var("AC1_GAME_DIR").ok()).unwrap_or_else(|| DEFAULT_GAME.into()).into()
}

/// A file followed as it grows; started again from its top when it shrinks (the hook started it over).
struct Follow {
    path: PathBuf,
    at: u64,
    partial: String,
}

impl Follow {
    fn lines(&mut self) -> Vec<String> {
        let Ok(mut f) = std::fs::File::open(&self.path) else { return Vec::new() };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        if len < self.at {
            self.at = 0;
            self.partial.clear();
        }
        let mut buf = Vec::new();
        if f.seek(SeekFrom::Start(self.at)).is_err() || f.read_to_end(&mut buf).is_err() {
            return Vec::new();
        }
        self.at += buf.len() as u64;
        self.partial.push_str(&String::from_utf8_lossy(&buf));
        let mut out: Vec<String> = self.partial.split('\n').map(|l| l.trim_end_matches('\r').to_string()).collect();
        self.partial = out.pop().unwrap_or_default();
        out
    }
}

fn tail(path: PathBuf, secs: Option<f32>, names: &Names) {
    let events = path.with_file_name("ac1-live-events.txt");
    let start_at = |p: &Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    let mut frames = Follow { at: start_at(&path), path: path.clone(), partial: String::new() };
    let mut ev = Follow { at: start_at(&events), path: events, partial: String::new() };
    println!("following {} (Ctrl-C stops)", path.display());
    let begun = std::time::Instant::now();
    let mut window: Vec<Row> = Vec::new();
    loop {
        for e in ev.lines() {
            if !e.starts_with('#') && !e.is_empty() {
                println!("event {e}");
            }
        }
        window.extend(frames.lines().iter().filter_map(|l| parse(l)));
        if let Some(r) = window.last() {
            let n = window.len();
            let i = &r.kv;
            let get = |k: &str| i.get(k).map_or("-", |s| s.as_str());
            let cam = camera_angle(r).map_or("-".to_string(), |a| format!("{a:.0}deg"));
            println!(
                "{:8.2} {:2} frames | {:<8} {:<28} | root {:.2},{:.2},{:.2} yaw {:6.1} vel {:5.2} | stick {} btn {} high {} | bones {} goals {} cam {cam}",
                r.t,
                n,
                mode(r),
                clip(r, names),
                r.root[0],
                r.root[1],
                r.root[2],
                r.yaw,
                r.vel,
                get("stick"),
                get("btn"),
                get("high"),
                r.bones.len(),
                r.targets.len()
            );
            window.clear();
        }
        if secs.is_some_and(|s| begun.elapsed().as_secs_f32() >= s) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (plain, opts) = options(&args);
    let game = game_dir(&opts);
    let names_from = |o: &HashMap<String, String>| load_names(o.contains_key("game").then_some(game.as_path()));
    let file = |k: usize| plain.get(k).map(PathBuf::from).unwrap_or_else(|| game.join("ac1-live.txt"));
    match plain.first().map(String::as_str) {
        Some("tail") => tail(file(1), opts.get("for").and_then(|s| s.parse().ok()), &names_from(&opts)),
        Some("summary") => {
            let mut rows = read_rows(&file(1));
            if let Some(last) = opts.get("last").and_then(|s| s.parse::<f32>().ok()) {
                let end = rows.last().map_or(0.0, |r| r.t);
                rows.retain(|r| r.t >= end - last);
            }
            print!("{}", summary(&rows, &names_from(&opts)));
        }
        Some("compare") => {
            let (Some(real), Some(ours)) = (plain.get(1), plain.get(2)) else {
                eprintln!("usage: livelog compare <real live log> <ours recording> [--from s] [--to s] [--offset s] [--step s]");
                return;
            };
            let num = |k: &str| opts.get(k).and_then(|s| s.parse::<f32>().ok());
            let mut r = read_rows(Path::new(real));
            r.retain(|x| num("from").is_none_or(|f| x.t >= f) && num("to").is_none_or(|t| x.t <= t));
            let o = read_rows(Path::new(ours));
            // (With the game: ours' clips by their actions, as the live log names them.)
            let acts = if opts.contains_key("game") { clip_actions(&game).unwrap_or_default() } else { ClipActions::new() };
            print!("{}", compare(&r, &o, num("offset"), num("step").unwrap_or(0.1), &names_from(&opts), &acts).table);
        }
        Some("runs") => {
            let dir = plain.get(1).map(PathBuf::from).unwrap_or_else(|| game.clone());
            print!("{}", runs(&live_lines(&dir, "ac1-live").filter_map(|l| parse(&l)).collect::<Vec<_>>()));
        }
        Some("cut") => {
            let num = |k: &str| opts.get(k).and_then(|s| s.parse::<f32>().ok());
            let (Some(out), Some(from), Some(to)) = (plain.get(1), num("from"), num("to")) else {
                eprintln!("usage: livelog cut <out> --from s --to s [--dir dir]");
                return;
            };
            let dir = opts.get("dir").map(PathBuf::from).unwrap_or_else(|| game.clone());
            let out = PathBuf::from(out);
            let events = out.with_extension(format!("events.{}", out.extension().and_then(|e| e.to_str()).unwrap_or("txt")));
            for (stem, path) in [("ac1-live", &out), ("ac1-live-events", &events)] {
                let lines: Vec<String> = live_lines(&dir, stem).filter(|l| line_time(l).is_some_and(|t| t >= from && t <= to)).collect();
                let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
                match std::fs::write(path, text) {
                    Ok(()) => println!("{} lines to {}", lines.len(), path.display()),
                    Err(e) => eprintln!("{}: {e}", path.display()),
                }
            }
        }
        Some("names") => {
            let dir = plain.get(1).map(PathBuf::from).unwrap_or(game);
            let out = plain.get(2).map(PathBuf::from).unwrap_or_else(|| dir.join("ac1-hook.actions.tsv"));
            match names_table(&dir).and_then(|t| std::fs::write(&out, &t).map(|_| t.lines().count()).map_err(Into::into)) {
                Ok(n) => println!("{n} actions written to {}", out.display()),
                Err(e) => eprintln!("names: {e:#}"),
            }
        }
        _ => eprintln!("usage: livelog tail|summary|compare|runs|cut|names ... (see the file's header)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &str = "   0.000 root   52.000   11.812  -33.400 yaw  -31.0 vel  0.00 | HumanGround clip xx_h_wait | bones Hips 52.000,12.760,-33.400 | ac1 frame=10 wall=0.000 mode=ground stick=0.00,0.00 cam=50.000,14.000,-30.000 camdir=0.500,-0.300,-0.800";

    fn row(t: f32, x: f32, state: &str) -> Row {
        Row { t, root: [x, 0.0, 0.0], state: state.into(), ..Default::default() }
    }

    #[test]
    fn parses_both_kinds_of_line() {
        let r = parse(REAL).unwrap();
        assert_eq!(r.root, [52.0, 11.812, -33.4]);
        assert_eq!(r.yaw, -31.0);
        assert_eq!(mode(&r), "ground");
        assert_eq!(clip(&r, &Names::new()), "xx_h_wait");
        assert_eq!(r.bones[0].0, "Hips");
        assert_eq!(r.kv.get("frame").map(String::as_str), Some("10"));
        assert!(camera_angle(&r).is_some());
        let ours =
            "   0.549 root    0.00    0.00    4.02 yaw -180.0 vel  0.00 | climb free_1m clip xx_h_hang | bones Hips -0.03,0.96,3.98 | targets 0.10,1.90,4.20";
        let o = parse(ours).unwrap();
        assert_eq!(mode(&o), "hang");
        assert_eq!(o.targets.len(), 1);
        assert_eq!(mode(&parse("   1.000 root 0 0 0 yaw 0 vel 0 | ground | limp | bones").unwrap()), "limp");
        assert!(parse("# header").is_none());
        // a hex clip named from the table
        let names = parse_names("0000abcd\t0\tHumanGround\txx_h_run\n");
        assert_eq!(clip(&row(0.0, 0.0, "HumanGround clip 0000abcd"), &names), "xx_h_run");
    }

    #[test]
    fn compares_from_the_onset() {
        // the real run waits 1 s, then moves 1 m/s; ours starts at once at 1 m/s and switches to the air 0.2 s later
        let real: Vec<Row> = (0..40).map(|k| k as f32 * 0.05).map(|t| row(t, (t - 1.0).max(0.0), "HumanGround clip a")).collect();
        let mut ours: Vec<Row> = (0..20).map(|k| k as f32 * 0.05).map(|t| row(t, t, "ground")).collect();
        for r in ours.iter_mut().filter(|r| r.t > 0.5) {
            r.state = "climb jump clip b".into();
        }
        let c = compare(&real, &ours, None, 0.1, &Names::new(), &ClipActions::new());
        assert!(c.mean < 0.06, "{}", c.table);
        assert!(c.end < 0.06 && c.max.0 < 0.06, "{}", c.table);
        assert!(c.table.contains("ground -> air"), "{}", c.table);
        let s = summary(&real, &Names::new());
        assert!(s.contains("40 frames"), "{s}");
    }

    #[test]
    fn follows_a_file_that_starts_over() {
        let p = std::env::temp_dir().join(format!("livelog-follow-{}.txt", std::process::id()));
        std::fs::write(&p, "a\nb\npart").unwrap();
        let mut f = Follow { path: p.clone(), at: 0, partial: String::new() };
        assert_eq!(f.lines(), vec!["a", "b"]);
        std::fs::write(&p, "a\nb\npartial\n").unwrap();
        assert_eq!(f.lines(), vec!["partial"]);
        std::fs::write(&p, "new\n").unwrap();
        assert_eq!(f.lines(), vec!["new"]);
        let _ = std::fs::remove_file(&p);
    }
}
