//! The level's holds (`Level::ledges`) by grid cell, for searches that only look near a point: a city has some
//! 190,000 of them, and a walk of them all for every character every frame (the low-profile edge halt) cost a
//! millisecond a character. A search through it returns the same hold as the walk over the list, in the list's order.

use crate::level::{Ledge, Level};
use bevy::prelude::*;
use std::collections::HashMap;

/// Cell side (m, across the ground).
const CELL: f32 = 2.0;

#[derive(Resource, Default)]
pub struct LedgeGrid {
    cells: HashMap<(i32, i32), Vec<u32>>,
}

fn cell(x: f32, z: f32) -> (i32, i32) {
    ((x / CELL).floor() as i32, (z / CELL).floor() as i32)
}

impl LedgeGrid {
    pub fn new(ledges: &[Ledge]) -> Self {
        let mut cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for (i, l) in ledges.iter().enumerate() {
            let (lo, hi) = (l.a.min(l.b), l.a.max(l.b));
            if !lo.is_finite() || !hi.is_finite() {
                continue;
            }
            let (a, b) = (cell(lo.x, lo.z), cell(hi.x, hi.z));
            // (A hold spanning more cells than a city could: left to no cell rather than filling the map.)
            if (b.0 - a.0) as i64 * (b.1 - a.1) as i64 > 1 << 16 {
                continue;
            }
            for x in a.0..=b.0 {
                for z in a.1..=b.1 {
                    cells.entry((x, z)).or_default().push(i as u32);
                }
            }
        }
        Self { cells }
    }

    /// The indices of the holds that may pass within `r` of `p` across the ground, in the list's order.
    pub fn near(&self, p: Vec3, r: f32) -> Vec<usize> {
        let (a, b) = (cell(p.x - r, p.z - r), cell(p.x + r, p.z + r));
        let mut v: Vec<usize> = vec![];
        for x in a.0..=b.0 {
            for z in a.1..=b.1 {
                v.extend(self.cells.get(&(x, z)).into_iter().flatten().map(|&i| i as usize));
            }
        }
        v.sort_unstable();
        v.dedup();
        v
    }
}

/// Keep the grid on the level's holds (built again when the level changes).
pub fn update(mut commands: Commands, level: Option<Res<Level>>) {
    if let Some(level) = level.filter(|l| l.is_changed()) {
        commands.insert_resource(LedgeGrid::new(&level.ledges));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_finds_every_hold_the_list_does_in_its_order() {
        let ledges: Vec<Ledge> = (0..200)
            .map(|i| {
                let f = i as f32;
                let a = Vec3::new((f * 7.3) % 40.0 - 20.0, (f * 0.37) % 5.0, (f * 3.1) % 40.0 - 20.0);
                Ledge { a, b: a + Vec3::new((f * 0.9) % 6.0 - 3.0, 0.0, (f * 1.7) % 6.0 - 3.0), out: Vec3::X }
            })
            .collect();
        let grid = LedgeGrid::new(&ledges);
        for k in 0..50 {
            let p = Vec3::new((k as f32 * 5.7) % 44.0 - 22.0, 1.0, (k as f32 * 2.9) % 44.0 - 22.0);
            let r = 0.4 + (k % 5) as f32;
            let by_list: Vec<usize> = (0..ledges.len()).filter(|&i| (ledges[i].closest(p) - p).with_y(0.0).length() < r).collect();
            let by_grid: Vec<usize> = grid.near(p, r).into_iter().filter(|&i| (ledges[i].closest(p) - p).with_y(0.0).length() < r).collect();
            assert_eq!(by_list, by_grid, "at {p} within {r}");
        }
    }
}
