//! The grab data's audit (`AC1_GRAB_AUDIT=<file.tsv>`, cities): every authored guidance piece (`forge::guidance`: ledge
//! grabs, ladders, poles, kiosks) against what ours built from it once the city is loaded, and ours' holds, perches,
//! bars and ladders that no authored piece backs. Writes one line per piece to the file (its source: the group or
//! entity placing it and that entity's first mesh; its kind, length, slope, ends and fate) and logs the totals by fate
//! and by source. See docs/NOTES.md "The grab data against AC1's".
//!
//! Fates of a ledge grab, by AC1's rules (`Level::ac1_hold_pieces`): `kept` (all of it a hold), `kept in part` (cut by the
//! depth filter or a ladder's zone; `covered` is the part kept), or none: `steep` (past 40 degrees, 0x66C350), `no wall`,
//! `ladder` (in a ladder's zone, 0x1170880), `depth` (no metre of room in front anywhere along it, 0xB27E80); and `lost`
//! where AC1's rules keep some but ours' holds do not have it (half or more of its holds' points on none of ours').
//! Ours' own: holds, perches (AC1's beam contacts and posts, `beam_pairs` and `Level::add_pilotis`, are counted as
//! backed), bars and ladders with no authored piece under them.

use crate::level::{Authored, FastMap, HoldFate, Ledge, Level, Line};
use bevy::prelude::*;
use forge::guidance::SubType;
use std::collections::BTreeMap;
use std::fmt::Write;

/// A grid of segments by 1 m cell (x, z), each in every cell its bounds (widened by `pad`) touch.
struct SegGrid(FastMap<(i32, i32), Vec<u32>>);

impl SegGrid {
    fn new(segs: impl Iterator<Item = (Vec3, Vec3)>, pad: f32) -> Self {
        let mut g: FastMap<(i32, i32), Vec<u32>> = FastMap::default();
        for (i, (a, b)) in segs.enumerate() {
            let (lo, hi) = (a.min(b) - pad, a.max(b) + pad);
            for x in lo.x.floor() as i32..=hi.x.floor() as i32 {
                for z in lo.z.floor() as i32..=hi.z.floor() as i32 {
                    g.entry((x, z)).or_default().push(i as u32);
                }
            }
        }
        SegGrid(g)
    }
    fn at(&self, p: Vec3) -> &[u32] {
        self.0.get(&(p.x.floor() as i32, p.z.floor() as i32)).map_or(&[], |v| v.as_slice())
    }
}

fn seg_dist(a: Vec3, b: Vec3, p: Vec3) -> f32 {
    Line { a, b }.closest(p).distance(p)
}

/// Run the audit, write the file at `path` and log the totals.
pub fn run(level: &Level, path: &str) {
    let ends = level.authored_ends();
    let zones = level.ladder_zones();
    let holds = SegGrid::new(level.ledges.iter().map(|l| (l.a, l.b)), 0.1);
    let on_hold = |p: Vec3, out: Vec3| holds.at(p).iter().map(|&i| &level.ledges[i as usize]).any(|l| l.closest(p).distance(p) < 0.03 && l.out.dot(out) > 0.7);
    let fate = |e: &Authored| -> (&'static str, f32) {
        let len = e.a.distance(e.b);
        match e.kind {
            SubType::LedgeGrab => {
                let (fate, pieces, kept) = level.ac1_hold_pieces(e, &zones, &ends);
                match fate {
                    HoldFate::Steep => ("steep", 0.0),
                    HoldFate::NoWall => ("no wall", 0.0),
                    HoldFate::Ladder => ("ladder", 0.0),
                    HoldFate::Depth => ("depth", 0.0),
                    HoldFate::Kept => {
                        // (Ours' holds must have AC1's: its holds' points on a hold of ours.)
                        let pts: Vec<(Vec3, Vec3)> = pieces
                            .iter()
                            .flat_map(|h| {
                                let n = (h.a.distance(h.b) / 0.1).ceil().max(1.0) as usize;
                                (0..=n).map(move |k| (h.a.lerp(h.b, k as f32 / n as f32), h.out))
                            })
                            .collect();
                        let on = pts.iter().filter(|(p, o)| on_hold(*p, *o)).count() as f32 / pts.len().max(1) as f32;
                        let frac = if len > 1e-4 { (kept / len).min(1.0) } else { 1.0 };
                        if on < 0.5 {
                            ("lost", frac)
                        } else if frac < 0.999 {
                            ("kept in part", frac)
                        } else {
                            ("kept", frac)
                        }
                    }
                }
            }
            SubType::Ladder => {
                let lo = if e.a.y < e.b.y { e.a } else { e.b };
                if level.ladders.iter().any(|l| l.base.distance(lo) < 0.1) { ("kept", 1.0) } else { ("lost", 0.0) }
            }
            SubType::Pole => {
                let mid = (e.a + e.b) * 0.5;
                if level.bars.iter().any(|b| b.closest(mid).distance(mid) < 0.2) {
                    ("kept", 1.0)
                } else if (e.b.y - e.a.y).abs() >= 0.3 * len.max(0.01) {
                    ("steep", 0.0)
                } else {
                    ("lost", 0.0)
                }
            }
            SubType::Kiosk => {
                let mid = (e.a + e.b) * 0.5;
                if level.kiosk_at(mid, 0.01).is_some() { ("kept", 1.0) } else { ("lost", 0.0) }
            }
            _ => ("unused kind", 0.0),
        }
    };
    let fates: Vec<(&'static str, f32)> = level.authored.iter().map(fate).collect();

    // Ours against the authored pieces: a hold on an authored ledge grab (within 5 cm, facing the same way), a perch with
    // an authored ledge grab within 0.6 m across and 0.3 m up or down of its middle, a bar on an authored pole (else on
    // ours' holds: a thin beam), a ladder at an authored one's foot.
    let grabs: Vec<&Authored> = level.authored.iter().filter(|e| e.kind == SubType::LedgeGrab).collect();
    let agrid = SegGrid::new(grabs.iter().map(|e| (e.a, e.b)), 0.7);
    let near_grab = |p: Vec3, r: f32, dy: f32, out: Option<Vec3>| {
        agrid.at(p).iter().map(|&i| grabs[i as usize]).any(|e| {
            let q = Line { a: e.a, b: e.b }.closest(p);
            (q - p).with_y(0.0).length() < r && (q.y - p.y).abs() < dy && out.is_none_or(|o| e.out.dot(o) > 0.7)
        })
    };
    let geometry = |l: &Ledge| {
        let mid = (l.a + l.b) * 0.5;
        level.ground(mid - l.out * 0.05 + Vec3::Y * 0.1, 0.0, 0.25).is_some() || level.raycast(mid + l.out * 0.3, -l.out, 0.45).is_some()
    };
    let mut ours: Vec<(&'static str, Vec3, Vec3, &'static str)> = vec![];
    for l in &level.ledges {
        let mid = (l.a + l.b) * 0.5;
        if !near_grab(mid, 0.05, 0.05, Some(l.out)) {
            ours.push(("hold", l.a, l.b, if geometry(l) { "unbacked, on geometry" } else { "unbacked, no geometry" }));
        }
    }
    for p in &level.perches {
        let mid = (p.a + p.b) * 0.5;
        if !near_grab(mid, 0.6, 0.3, None) {
            let top = level.ground(mid + Vec3::Y * 0.1, 0.0, 0.25).is_some();
            ours.push(("perch", p.a, p.b, if top { "unbacked, on geometry" } else { "unbacked, no geometry" }));
        }
    }
    let poles: Vec<&Authored> = level.authored.iter().filter(|e| e.kind == SubType::Pole).collect();
    for b in &level.bars {
        let mid = (b.a + b.b) * 0.5;
        if !poles.iter().any(|e| seg_dist(e.a, e.b, mid) < 0.3) {
            ours.push(("bar", b.a, b.b, if near_grab(mid, 0.4, 0.2, None) { "from holds (thin beam)" } else { "unbacked (mesh name)" }));
        }
    }
    let ladders: Vec<&Authored> = level.authored.iter().filter(|e| e.kind == SubType::Ladder).collect();
    for l in &level.ladders {
        if !ladders.iter().any(|e| e.a.distance(l.base) < 0.1 || e.b.distance(l.base) < 0.1) {
            ours.push(("ladder", l.base, l.base.with_y(l.top), "unbacked (mesh name)"));
        }
    }
    // (What ours made, by where it comes from.)
    let posts = level.perches.iter().filter(|p| p.a == p.b).count();
    let thin = ours.iter().filter(|o| o.0 == "bar").count();
    info!(
        "grab audit: made: {} holds from AC1's ledge grabs, {} beams (AC1's beam contacts) and {posts} posts (AC1's pilotis), {} swing bars \
         from authored poles and {thin} not, {} ladders, {} kiosk pieces, {} haystacks and hiding places",
        level.ledges.len(),
        level.perches.len() - posts,
        level.bars.len() - thin,
        level.ladders.len(),
        level.kiosks.len(),
        level.haystacks.len()
    );

    // The file.
    let mut out = String::from("what\tsource\tmesh\tkind\tlen\tslope\tax\tay\taz\tbx\tby\tbz\tfate\tcovered\toutx\toutz\tblock_at\tblock_ny\n");
    for (e, (f, cov)) in level.authored.iter().zip(&fates) {
        let len = e.a.distance(e.b);
        let src = level.authored_src.get(e.src as usize).map_or("-\t-", |s| s.as_str());
        let slope = if len > 1e-4 { (e.b.y - e.a.y).abs() / len } else { 0.0 };
        // (One with no room in front: how far out from its middle the depth detector's ray meets a face, and the up part
        // of that face's normal.)
        let block = if *f == "depth" {
            let p = (e.a + e.b) * 0.5 + e.out * 0.02 + Vec3::Y * 0.02;
            let d = crate::guide_depth::LEDGE;
            let dir = e.out.with_y(0.0).normalize_or_zero() * d.angle.cos() + Vec3::Y * d.angle.sin();
            level.raycast(p, dir, d.length).map_or("-\t-".to_string(), |h| format!("{:.2}\t{:.2}", h.dist, h.normal.y))
        } else {
            "-\t-".to_string()
        };
        let _ = writeln!(
            out,
            "authored\t{src}\t{:?}\t{len:.3}\t{slope:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{f}\t{cov:.2}\t{:.2}\t{:.2}\t{block}",
            e.kind, e.a.x, e.a.y, e.a.z, e.b.x, e.b.y, e.b.z, e.out.x, e.out.z
        );
    }
    for (what, a, b, f) in &ours {
        let _ = writeln!(
            out,
            "ours\t-\t-\t{what}\t{:.3}\t-\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{f}\t-\t-\t-\t-\t-",
            a.distance(*b),
            a.x,
            a.y,
            a.z,
            b.x,
            b.y,
            b.z
        );
    }
    match std::fs::write(path, out) {
        Ok(()) => info!("grab audit written to {path}"),
        Err(e) => warn!("grab audit: {path}: {e}"),
    }

    // The log: by kind and fate, the sources with the most pieces not kept, ours' unbacked.
    let mut by_fate: BTreeMap<(String, &str), usize> = BTreeMap::new();
    let mut by_src: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for (e, (f, _)) in level.authored.iter().zip(&fates) {
        *by_fate.entry((format!("{:?}", e.kind), f)).or_default() += 1;
        if *f != "kept" {
            *by_src.entry((level.authored_src.get(e.src as usize).map_or("-", |s| s.as_str()), f)).or_default() += 1;
        }
    }
    for ((k, f), n) in &by_fate {
        info!("grab audit: authored {k} {f}: {n}");
    }
    let mut srcs: Vec<_> = by_src.into_iter().collect();
    srcs.sort_by_key(|s| std::cmp::Reverse(s.1));
    for ((s, f), n) in srcs.iter().take(40) {
        info!("grab audit: not kept, {f}: {n} from {}", s.replace('\t', " / "));
    }
    let mut by_ours: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for (what, _, _, f) in &ours {
        *by_ours.entry((what, f)).or_default() += 1;
    }
    for ((w, f), n) in &by_ours {
        info!(
            "grab audit: ours {w} {f}: {n} (of {})",
            match *w {
                "hold" => level.ledges.len(),
                "perch" => level.perches.len(),
                "bar" => level.bars.len(),
                _ => level.ladders.len(),
            }
        );
    }
}
