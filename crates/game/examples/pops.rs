//! pops <recording.txt> [min metres]: pose pops in a flight recording (F9 / `AC1_RECORD_AT`): frames where a bone,
//! relative to the root, moves much faster than it did the frame before (more than 3x and 1.5 m/s faster, and at
//! least `min` metres, 0.04 by default). Judged by speed, not distance, so a long frame (a hitch) is not a pop; the bones
//! are recorded a frame behind the root and are matched to it. Prints
//! each pop with what was playing, then a count by what was playing.
use std::collections::BTreeMap;

struct Row {
    t: f32,
    root: [f32; 3],
    state: String,
    bones: Vec<(String, [f32; 3])>,
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
    let t = head.first()?.parse().ok()?;
    let root = [head.get(2)?.parse().ok()?, head.get(3)?.parse().ok()?, head.get(4)?.parse().ok()?];
    let state = parts.next()?.trim().to_string();
    let bones_part = parts.next()?.trim().strip_prefix("bones ")?;
    let w: Vec<&str> = bones_part.split_whitespace().collect();
    let bones = w.chunks(2).filter_map(|c| Some((c.first()?.to_string(), v3(c.get(1)?)?))).collect();
    Some(Row { t, root, state, bones })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(path) = args.get(1) else {
        eprintln!("usage: pops <recording.txt> [min metres]");
        return;
    };
    let min: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.04);
    let text = std::fs::read_to_string(path).expect("read the recording");
    let rows: Vec<Row> = text.lines().filter_map(parse).collect();
    // (The recorder writes the bones as of the frame before, the root as of this one: each bone is taken relative to
    // the root a row earlier, or uneven frames show the body falling behind and catching up.)
    let rel = |k: usize, i: usize| {
        let b = rows[k].bones[i].1;
        let r = rows[k.saturating_sub(1)].root;
        [b[0] - r[0], b[1] - r[1], b[2] - r[2]]
    };
    let dist = |a: [f32; 3], b: [f32; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
    let mut by_state: BTreeMap<String, usize> = BTreeMap::new();
    let mut frames = 0;
    for k in 3..rows.len() {
        let (r, p, q) = (&rows[k], &rows[k - 1], &rows[k - 2]);
        // (The bones' moves between rows are the frame before's: timed by it.)
        let (dt, dtp) = (p.t - q.t, q.t - rows[k - 3].t);
        if dt <= 0.0 || dtp <= 0.0 || r.bones.len() != p.bones.len() || p.bones.len() != q.bones.len() {
            continue;
        }
        let mut hit = false;
        for i in 0..r.bones.len() {
            let d = dist(rel(k, i), rel(k - 1, i));
            let (v, vp) = (d / dt, dist(rel(k - 1, i), rel(k - 2, i)) / dtp);
            if d > min && v > 3.0 * vp + 1.5 {
                println!("{:.3} {} {d:.3} m, {v:.1} m/s (before {vp:.1}) | {} <- {}", r.t, r.bones[i].0, r.state, p.state);
                hit = true;
            }
        }
        if hit {
            frames += 1;
            let what = if r.state == p.state { r.state.clone() } else { format!("{} <- {}", r.state, p.state) };
            *by_state.entry(what).or_default() += 1;
        }
    }
    println!("{frames} frames with pops in {} frames", rows.len());
    for (s, n) in by_state.iter().filter(|(_, n)| **n > 0) {
        println!("{n:5}  {s}");
    }
}
