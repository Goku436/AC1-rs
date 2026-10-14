//! AC1's foot-level edge reports (IHuman vt132 `Human__BuildEdgeReports` 0xB1BC50 and its pick vt136 0xB17EF0, as
//! Banned445's AC1-Movement-Rewritten reads them, MIT, Copyright (c) 2026 Banned445; the thresholds checked in the
//! ground's input interpreter 0xEE65A0): the guidance edges near the feet with a drop past them, which the ground's edge
//! moves fire on: the free run's edge jump (0xEE7DCE), the ledge stop (event 69), the edge jump at a target (0xEE8679)
//! and the step off (event 68). docs/PARKOUR.md section 3, "The edge events".
//!
//! The edges are the city's authored guidance (every kind but ladders and beams, up to 40 degrees steep, with a wall to
//! face out from; `Level::report_edges`, made with the raycast grid). The test level has no guidance (its holds are
//! ours), so there the ground keeps ours' collision tests for those moves (`Level::guided`).

use bevy::prelude::*;

use crate::level::{FastMap, Ledge, Level};

/// The report edges are indexed by cells this wide (m).
pub(crate) const REPORT_CELL: f32 = 2.0;
/// The interpreter's report pick radius (`dword_17085E0`, 0.75 m) and the reports' least drop (0xB1BC50's caller, m).
pub const REPORT_RADIUS: f32 = 0.75;
pub const REPORT_MIN_DROP: f32 = 0.45;
/// An edge reports to feet within this of its height (m).
const REPORT_HEIGHT: f32 = 0.3;
/// `Human__MeasureDropBeyondEdge` 0xB19620: the deepest drop measured (m) and how far past the edge it starts (m).
const DROP_MAX: f32 = 7.0;
const DROP_STEP: f32 = 0.02;

/// An edge with a drop past it near the feet (0xB1BC50's 64-byte report).
#[derive(Clone, Copy, Debug)]
pub struct EdgeReport {
    /// +0x10: where the vertical plane through the feet across the edge meets it.
    pub point: Vec3,
    /// +0x20: the way out over the drop (flat).
    pub normal: Vec3,
    /// +0x30: the drop past the edge (m).
    pub drop: f32,
    /// The feet's flat distance to `point` (m), negative once past the edge line.
    pub dist: f32,
}

impl EdgeReport {
    /// +0x34 as AC1 keeps it: the squared distance, negative past the edge (its thresholds are squared: the free run's
    /// 0.25 is 0.5 m, the ledge stop's 0.0225 0.15 m).
    pub fn sq(&self) -> f32 {
        self.dist * self.dist.abs()
    }
}

/// The edges AC1's reports are built from, and their index by `REPORT_CELL` cells (`Level::build_grid`).
pub(crate) fn report_edges(level: &Level) -> Vec<Ledge> {
    if !level.authored.is_empty() {
        use forge::guidance::SubType;
        return level
            .authored
            .iter()
            .filter(|e| !matches!(e.kind, SubType::Ladder | SubType::Beam))
            .filter(|e| {
                let len = e.a.distance(e.b);
                len > 1e-4 && (e.b.y - e.a.y).abs() / len <= crate::level::AUTHORED_MAX_SLOPE
            })
            .filter_map(|e| Some(Ledge { a: e.a, b: e.b, out: e.out.with_y(0.0).try_normalize()? }))
            .collect();
    }
    vec![]
}

pub(crate) fn report_grid(edges: &[Ledge]) -> FastMap<(i32, i32), Vec<u32>> {
    let mut grid: FastMap<(i32, i32), Vec<u32>> = FastMap::default();
    let cell = |v: f32| (v / REPORT_CELL).floor() as i32;
    for (i, e) in edges.iter().enumerate() {
        let (lo, hi) = (e.a.min(e.b), e.a.max(e.b));
        for x in cell(lo.x)..=cell(hi.x) {
            for z in cell(lo.z)..=cell(hi.z) {
                grid.entry((x, z)).or_default().push(i as u32);
            }
        }
    }
    grid
}

impl Level {
    /// The level has AC1's guidance to build edge reports from (a city). `AC1_NO_EDGE_REPORTS` keeps ours' collision tests
    /// there too (to compare).
    pub fn guided(&self) -> bool {
        static OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        !self.report_edges.is_empty() && !*OFF.get_or_init(|| std::env::var_os("AC1_NO_EDGE_REPORTS").is_some())
    }

    /// The edges within `radius` of the feet `pos` (m) whose plane through the feet meets them within 0.3 m of their
    /// height, with at least `min_drop` past them (0xB1BC50).
    pub fn edge_reports(&self, pos: Vec3, radius: f32, min_drop: f32) -> Vec<EdgeReport> {
        self.edge_reports_at(pos, radius, min_drop, true)
    }

    fn edge_reports_at(&self, pos: Vec3, radius: f32, min_drop: f32, stepped: bool) -> Vec<EdgeReport> {
        let cell = |v: f32| (v / REPORT_CELL).floor() as i32;
        let mut seen: Vec<u32> = vec![];
        let mut out = vec![];
        for x in cell(pos.x - radius)..=cell(pos.x + radius) {
            for z in cell(pos.z - radius)..=cell(pos.z + radius) {
                for &i in self.report_grid.get(&(x, z)).into_iter().flatten() {
                    if seen.contains(&i) {
                        continue;
                    }
                    seen.push(i);
                    let e = &self.report_edges[i as usize];
                    if e.closest(pos).distance(pos) > radius {
                        continue;
                    }
                    let Some(u) = (e.b - e.a).with_y(0.0).try_normalize() else { continue };
                    let (sa, sb) = ((e.a - pos).dot(u), (e.b - pos).dot(u));
                    if sa * sb > 0.0 || (sa - sb).abs() < 1e-6 {
                        continue;
                    }
                    let p = e.a.lerp(e.b, sa / (sa - sb));
                    if (pos.y - p.y).abs() >= REPORT_HEIGHT {
                        continue;
                    }
                    let drop = self.drop_past(p, e.out, stepped.then_some(pos));
                    if drop < min_drop {
                        continue;
                    }
                    let h = (p - pos).with_y(0.0).length();
                    let dist = if e.out.dot(pos - p) > 0.0 { -h } else { h };
                    out.push(EdgeReport { point: p, normal: e.out, drop, dist });
                }
            }
        }
        out
    }

    /// The report within `radius` of the feet whose normal is nearest `dir`, within `max_angle` of it (vt136 0xB17EF0).
    pub fn pick_report(&self, pos: Vec3, dir: Vec3, radius: f32, max_angle: f32) -> Option<EdgeReport> {
        let dir = dir.with_y(0.0).normalize_or_zero();
        self.edge_reports(pos, REPORT_RADIUS, REPORT_MIN_DROP)
            .into_iter()
            .filter(|r| (r.point - pos).with_y(0.0).length_squared() < radius * radius)
            .map(|r| (r.normal.dot(dir).clamp(-1.0, 1.0).acos(), r))
            .filter(|(a, _)| *a <= max_angle)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, r)| r)
    }

    /// Room for the body past the edge (event 68's guard, `sub_B2E7A0` -> `Human__BoxQueryEmpty` 0xB2D2A0): a box 0.5 m
    /// out over it and 0.7 m up, half extents 0.25 across, 0.3 out and 1.0 up (Banned445's reading).
    pub fn room_past_edge(&self, r: &EdgeReport) -> bool {
        let across = r.normal.cross(Vec3::Y).normalize_or_zero();
        self.box_free(r.point + r.normal * 0.5 + Vec3::Y * 0.7, [across, r.normal, Vec3::Y], Vec3::new(0.25, 0.3, 1.0))
    }

    /// The drop past an edge at `p` facing out `n` (`Human__MeasureDropBeyondEdge` 0xB19620): a ray 45 degrees out and
    /// down from `DROP_STEP` past it to 0.5 m under it, then one 10 degrees off straight down to `DROP_MAX` under it; a
    /// floor steeper than 45 degrees is slid down. A step under the edge adds the drop past the step (the reports at the
    /// floor within 0.2 m · drop / 3). First, with the feet given, the line from 1 m over them to the point past the
    /// edge: what it meets is the floor (a beam over a crack past a roof's end, its top over the edge, is no drop).
    /// (0xB19620's other check first, from 0.05 m in from the edge 50 degrees down, starts on the top: left out.)
    fn drop_past(&self, p: Vec3, n: Vec3, feet: Option<Vec3>) -> f32 {
        let stepped = feet.is_some();
        if let Some(f) = feet {
            let (from, to) = (f + Vec3::Y, p + n * DROP_STEP);
            if let Some(d) = (to - from).try_normalize()
                && let Some(h) = self.raycast(from, d, from.distance(to))
            {
                return p.y - h.point.y;
            }
        }
        let tilted = |axis: Vec3, deg: f32| Quat::from_axis_angle(axis, (-deg).to_radians()) * Vec3::NEG_Y;
        let to_plane = |o: Vec3, d: Vec3, h: f32| if d.y < -1e-6 { ((o.y - h) / -d.y).max(0.0) } else { 0.0 };
        let floor = |hit: Vec3| {
            let drop = p.y - hit.y;
            let r = 0.2 * (drop / 3.0).clamp(0.0, 1.0);
            let below = if stepped && r > 0.01 { self.edge_reports_at(hit, r, REPORT_MIN_DROP, false).iter().map(|e| e.drop).fold(0.0, f32::max) } else { 0.0 };
            drop + below
        };
        let mut axis = Vec3::Y.cross(n);
        let mut from = p + n * DROP_STEP;
        let mut dir = tilted(axis, 45.0);
        let mut len = to_plane(from, dir, p.y - DROP_MAX.min(0.5));
        for _ in 0..8 {
            if let Some(h) = self.raycast(from, dir, len) {
                return floor(h.point);
            }
            from += dir * len;
            dir = tilted(axis, 10.0);
            len = to_plane(from, dir, p.y - DROP_MAX);
            let Some(h) = self.raycast(from, dir, len) else { return DROP_MAX };
            let side = h.normal.with_y(0.0);
            if side.length() < 1e-4 || h.normal.y >= std::f32::consts::FRAC_1_SQRT_2 {
                return floor(h.point);
            }
            let side = side.normalize();
            axis = Vec3::Y.cross(side);
            from = h.point + side * 0.01;
            dir = tilted(axis, 45.0);
            len = to_plane(from, dir, h.point.y - 0.5);
        }
        DROP_MAX
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::Tri;

    /// A box from `lo` to `hi` as triangles (its top and sides, wound outward).
    fn cuboid(tris: &mut Vec<Tri>, lo: Vec3, hi: Vec3) {
        let c = |x: f32, y: f32, z: f32| Vec3::new(if x > 0.0 { hi.x } else { lo.x }, if y > 0.0 { hi.y } else { lo.y }, if z > 0.0 { hi.z } else { lo.z });
        let mut quad = |a: Vec3, b: Vec3, cc: Vec3, d: Vec3| {
            tris.push(Tri { a, b, c: cc });
            tris.push(Tri { a, b: cc, c: d });
        };
        quad(c(0., 1., 0.), c(0., 1., 1.), c(1., 1., 1.), c(1., 1., 0.));
        quad(c(0., 0., 0.), c(1., 0., 0.), c(1., 1., 0.), c(0., 1., 0.));
        quad(c(0., 0., 1.), c(0., 1., 1.), c(1., 1., 1.), c(1., 0., 1.));
        quad(c(0., 0., 0.), c(0., 1., 0.), c(0., 1., 1.), c(0., 0., 1.));
        quad(c(1., 0., 0.), c(1., 0., 1.), c(1., 1., 1.), c(1., 1., 0.));
    }

    /// A level of `tris` whose report edges are `edges` (as a city's guidance would give).
    fn level(tris: Vec<Tri>, edges: Vec<Ledge>) -> Level {
        let mut l = Level { tris, ..default() };
        l.build_grid();
        l.report_grid = report_grid(&edges);
        l.report_edges = edges;
        l
    }

    fn rim(a: Vec3, b: Vec3, out: Vec3) -> Ledge {
        Ledge { a, b, out }
    }

    #[test]
    fn a_platform_reports_its_rim_and_drop() {
        let mut tris = vec![];
        cuboid(&mut tris, Vec3::new(-10.0, -1.0, -10.0), Vec3::new(10.0, 0.0, 10.0));
        cuboid(&mut tris, Vec3::new(0.0, 0.0, -2.0), Vec3::new(3.0, 2.0, 2.0));
        let l = level(tris, vec![rim(Vec3::new(3.0, 2.0, -2.0), Vec3::new(3.0, 2.0, 2.0), Vec3::X)]);
        // On the platform 0.3 m in from its +X rim: the rim 0.3 m ahead, 2 m down.
        let r = l.pick_report(Vec3::new(2.7, 2.0, 0.0), Vec3::X, REPORT_RADIUS, 0.1).expect("a report at the rim");
        assert!((r.point.x - 3.0).abs() < 1e-3 && (r.dist - 0.3).abs() < 1e-3, "{r:?}");
        assert!((r.drop - 2.0).abs() < 0.05, "{r:?}");
        assert!((r.sq() - 0.09).abs() < 1e-3);
        // Past it: negative.
        let r = l.edge_reports(Vec3::new(3.1, 2.0, 0.0), REPORT_RADIUS, REPORT_MIN_DROP);
        assert!(r.iter().any(|r| r.dist < 0.0 && r.normal.x > 0.9), "{r:?}");
        // On the ground beside it: none (the feet 2 m under the rim).
        assert!(l.edge_reports(Vec3::new(3.3, 0.0, 0.0), REPORT_RADIUS, REPORT_MIN_DROP).is_empty());
    }

    #[test]
    fn a_narrow_step_under_a_rim_reports_the_drop_past_it() {
        let mut tris = vec![];
        cuboid(&mut tris, Vec3::new(-10.0, -1.0, -10.0), Vec3::new(10.0, 0.0, 10.0));
        cuboid(&mut tris, Vec3::new(0.0, 0.0, -2.0), Vec3::new(3.0, 3.0, 2.0));
        // A 0.3 m step on the platform's side, 0.2 m down: too small; past it the ground 3 m down.
        cuboid(&mut tris, Vec3::new(3.0, 0.0, -2.0), Vec3::new(3.1, 2.8, 2.0));
        let edges = vec![rim(Vec3::new(3.0, 3.0, -2.0), Vec3::new(3.0, 3.0, 2.0), Vec3::X), rim(Vec3::new(3.1, 2.8, -2.0), Vec3::new(3.1, 2.8, 2.0), Vec3::X)];
        let l = level(tris, edges);
        let r = l.pick_report(Vec3::new(2.8, 3.0, 0.0), Vec3::X, REPORT_RADIUS, 0.1).expect("the stepped edge reports");
        assert!(r.drop > 2.5, "{r:?}");
    }
}
