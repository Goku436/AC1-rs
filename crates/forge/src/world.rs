//! A city's world file (the data file named after the city, e.g. `Damascus`): its `World`, the streaming grid
//! (`GridPartition`) and the low-detail stand-ins drawn for the distant city (`FakeEntities`).
//!
//! `World` (fbb63e47) holds an inline `GridLayout` (7db083ed): after its class, u8 the loading radius outside the grid
//! (95), then i32 cell size (32 m), i32 origin x, i32 origin y (game space, metres), i32 `?` (0 in Damascus, 0x029100ad in
//! Masyaf), i32 the finest level's width in cells. Damascus: 32 m cells from (-480, -512), 32 wide; Masyaf: from (-1024,
//! -1024), 64 wide.
//!
//! `GridPartition` (0e2f4444, `Grid - <City>`): u32 levels, u32 cells, then the cells. Each level halves the width and
//! doubles the cell size, level after level (Damascus 6 levels, 1365 cells; Masyaf 7, 5461); cell k of a level is at
//! (k % width, k / width). The cells' data files are `Cell<k>_DataBlock` (k over all levels), so a big object lives in a
//! coarse cell.
//!
//! `FakeEntities` (c69a7f31, `<City>_Fakes`): u32 `?`, u32 count (64), then one inline `FakeEntity` (d77fb524) per block
//! of the finest grid (8 x 8 blocks, Z-order: block k's column and row are the even and odd bits of k). A block with a
//! stand-in holds an inline entity (u32 id, u32 0984415e, its 4x4 world transform, Z up) whose `Visual` (ec658d29: u8
//! shown, u32 the mesh, `<City>_FakeMesh_<k>`: the block's buildings merged, low detail) is followed by one
//! `SubMeshFakeIndexSpans` (2afd2e35: u32 count) per submesh of that mesh: per finest cell of the block (Z-order again)
//! an inline `FakeIndexSpan` (e74772ba: u32 id, u32 class, u32 first index, u32 index count, into the submesh's indices):
//! the part of the stand-in that is that cell's buildings, which the game hides while the cell's own objects are loaded
//! (the layout and its meaning as Banned445's AC1-Movement-Rewritten (MIT) read them; checked here on the data, the
//! `lodcheck` example). Indices outside every span (a block's countryside) always draw.

use anyhow::{Context, Result, ensure};

pub const CLASS_WORLD: u32 = 0xfbb6_3e47;
pub const CLASS_GRID_PARTITION: u32 = 0x0e2f_4444;
pub const CLASS_FAKE_ENTITIES: u32 = 0xc69a_7f31;
const CLASS_GRID_LAYOUT: u32 = 0x7db0_83ed;
const CLASS_FAKE_ENTITY: u32 = 0xd77f_b524;
const CLASS_ENTITY: u32 = 0x0984_415e;
const CLASS_VISUAL: u32 = 0xec65_8d29;
const CLASS_SUBMESH_FAKE_INDEX_SPANS: u32 = 0x2afd_2e35;
const CLASS_FAKE_INDEX_SPAN: u32 = 0xe747_72ba;

fn u32_at(d: &[u8], p: usize) -> Option<u32> {
    d.get(p..p.checked_add(4)?).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn markers(d: &[u8], class: u32) -> Vec<usize> {
    let c = class.to_le_bytes();
    d.windows(4).enumerate().filter(|(_, w)| *w == c).map(|(p, _)| p).collect()
}

/// The streaming grid's layout (game space, Z up, metres).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridLayout {
    pub cell: f32,
    pub origin: [f32; 2],
    /// Cells across the finest level.
    pub width: u32,
    /// The loading radius outside the grid (m).
    pub outside_radius: u8,
}

impl GridLayout {
    /// A level's cell size (m) and width (cells).
    pub fn level(&self, level: u32) -> (f32, u32) {
        (self.cell * (1u32 << level.min(31)) as f32, (self.width >> level.min(31)).max(1))
    }

    /// Where cell `index` (over all levels, finest first) is: (level, column, row); None past the last level.
    pub fn cell(&self, index: usize, levels: u32) -> Option<(u32, u32, u32)> {
        let mut base = 0usize;
        for l in 0..levels {
            let w = self.level(l).1 as usize;
            if index < base + w * w {
                let k = index - base;
                return Some((l, (k % w) as u32, (k / w) as u32));
            }
            base += w * w;
        }
        None
    }

    /// A cell's rectangle (min, max), game x and y.
    pub fn rect(&self, level: u32, col: u32, row: u32) -> ([f32; 2], [f32; 2]) {
        let s = self.level(level).0;
        let min = [self.origin[0] + col as f32 * s, self.origin[1] + row as f32 * s];
        (min, [min[0] + s, min[1] + s])
    }
}

/// The `GridLayout` inside a `World` body.
pub fn grid_layout(world: &[u8]) -> Result<GridLayout> {
    let at = markers(world, CLASS_GRID_LAYOUT);
    ensure!(at.len() == 1, "{} GridLayouts in the World", at.len());
    let p = at[0];
    let i = |k: usize| u32_at(world, p + 5 + 4 * k).map(|v| v as i32).with_context(|| format!("GridLayout at {p:#x} cut short"));
    let (size, ox, oy, width) = (i(0)?, i(1)?, i(2)?, i(4)?);
    ensure!((1..=4096).contains(&size) && (1..=4096).contains(&width), "GridLayout at {p:#x}: cell {size} m, {width} wide");
    Ok(GridLayout { cell: size as f32, origin: [ox as f32, oy as f32], width: width as u32, outside_radius: world[p + 4] })
}

/// A `GridPartition`'s level and cell counts, checked against the layout.
pub fn grid_levels(partition: &[u8], layout: &GridLayout) -> Result<u32> {
    let (levels, cells) = (u32_at(partition, 0).unwrap_or(0), u32_at(partition, 4).unwrap_or(0) as usize);
    ensure!((1..=16).contains(&levels), "{levels} grid levels at 0x0");
    let want: usize = (0..levels).map(|l| (layout.level(l).1 as usize).pow(2)).sum();
    ensure!(cells == want, "{cells} grid cells at 0x4, {want} for {levels} levels of {}", layout.width);
    Ok(levels)
}

/// One block's stand-in.
#[derive(Debug, Clone)]
pub struct FakeBlock {
    /// The block (column, row) on the blocks' grid.
    pub block: (u32, u32),
    /// Its entity's world transform (column-major, Z up).
    pub transform: [f32; 16],
    pub mesh: u32,
    pub spans: Vec<FakeSpan>,
}

/// A finest cell's part of a stand-in: indices `first..first + count` of submesh `submesh`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FakeSpan {
    pub submesh: usize,
    /// The finest cell (column, row).
    pub cell: (u32, u32),
    pub first: u32,
    pub count: u32,
}

/// (column, row) of a Z-order index.
fn unmorton(k: usize) -> (u32, u32) {
    let (mut x, mut y) = (0, 0);
    for b in 0..16 {
        x |= ((k >> (2 * b)) & 1) << b;
        y |= ((k >> (2 * b + 1)) & 1) << b;
    }
    (x as u32, y as u32)
}

/// The stand-ins of a `FakeEntities` body: the blocks that have one.
pub fn fake_entities(body: &[u8], layout: &GridLayout) -> Result<Vec<FakeBlock>> {
    let count = u32_at(body, 4).unwrap_or(0) as usize;
    let records = markers(body, CLASS_FAKE_ENTITY);
    ensure!(records.len() == count, "{} FakeEntity records, {count} declared at 0x4", records.len());
    let side = (count as f64).sqrt() as u32;
    ensure!(side * side == count as u32 && side > 0 && layout.width.is_multiple_of(side), "{count} blocks do not tile a grid {} wide", layout.width);
    let per = layout.width / side;
    let mut out = vec![];
    for (k, &p) in records.iter().enumerate() {
        let end = records.get(k + 1).map_or(body.len(), |&n| n.saturating_sub(4).max(p));
        let rec = &body[p..end];
        let Some(&v) = markers(rec, CLASS_VISUAL).first() else { continue };
        if rec.get(v + 4) != Some(&1) {
            continue;
        }
        let mesh = u32_at(rec, v + 5).with_context(|| format!("FakeEntity {k}: Visual at {:#x} cut short", p + v))?;
        let e = *markers(&rec[..v], CLASS_ENTITY).first().with_context(|| format!("FakeEntity {k} at {p:#x}: no entity"))?;
        let mut transform = [0f32; 16];
        for (i, t) in transform.iter_mut().enumerate() {
            *t = f32::from_bits(u32_at(rec, e + 4 + 4 * i).with_context(|| format!("FakeEntity {k}: transform cut short"))?);
        }
        ensure!(transform.iter().all(|t| t.is_finite()) && transform[15] == 1.0, "FakeEntity {k}: transform at {:#x} not affine", p + e + 4);
        let block = unmorton(k);
        let mut spans = vec![];
        for (submesh, &s) in markers(rec, CLASS_SUBMESH_FAKE_INDEX_SPANS).iter().enumerate() {
            let n = u32_at(rec, s + 4).unwrap_or(u32::MAX) as usize;
            ensure!(n == (per * per) as usize, "FakeEntity {k}: {n} spans at {:#x}, {} cells a block", p + s + 4, per * per);
            for c in 0..n {
                let q = s + 8 + 16 * c;
                ensure!(u32_at(rec, q + 4) == Some(CLASS_FAKE_INDEX_SPAN), "FakeEntity {k}: no FakeIndexSpan at {:#x}", p + q + 4);
                let (first, count) = (u32_at(rec, q + 8).unwrap_or(0), u32_at(rec, q + 12).unwrap_or(0));
                if count == 0 {
                    continue;
                }
                let (ix, iy) = unmorton(c);
                spans.push(FakeSpan { submesh, cell: (block.0 * per + ix, block.1 * per + iy), first, count });
            }
        }
        out.push(FakeBlock { block, transform, mesh, spans });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_ids_are_their_names() {
        for (name, id) in [
            ("World", CLASS_WORLD),
            ("GridPartition", CLASS_GRID_PARTITION),
            ("FakeEntities", CLASS_FAKE_ENTITIES),
            ("GridLayout", CLASS_GRID_LAYOUT),
            ("FakeEntity", CLASS_FAKE_ENTITY),
            ("Entity", CLASS_ENTITY),
            ("Visual", CLASS_VISUAL),
            ("SubMeshFakeIndexSpans", CLASS_SUBMESH_FAKE_INDEX_SPANS),
            ("FakeIndexSpan", CLASS_FAKE_INDEX_SPAN),
        ] {
            assert_eq!(crc32fast::hash(name.as_bytes()), id, "{name}");
        }
    }

    #[test]
    fn grid_cells_go_level_by_level() {
        let mut w = vec![0u8; 3];
        w.extend(CLASS_GRID_LAYOUT.to_le_bytes());
        w.push(95);
        for v in [32i32, -480, -512, 0, 4] {
            w.extend(v.to_le_bytes());
        }
        let g = grid_layout(&w).unwrap();
        assert_eq!((g.cell, g.origin, g.width, g.outside_radius), (32.0, [-480.0, -512.0], 4, 95));
        // 4x4, then 2x2, then 1.
        assert_eq!(
            [0, 5, 15, 16, 19, 20, 21].map(|k| g.cell(k, 3)),
            [Some((0, 0, 0)), Some((0, 1, 1)), Some((0, 3, 3)), Some((1, 0, 0)), Some((1, 1, 1)), Some((2, 0, 0)), None]
        );
        assert_eq!(g.rect(1, 1, 0), ([-416.0, -512.0], [-352.0, -448.0]));
        let mut p = vec![];
        p.extend(3u32.to_le_bytes());
        p.extend(21u32.to_le_bytes());
        assert_eq!(grid_levels(&p, &g).unwrap(), 3);
        p[4] = 20;
        assert!(grid_levels(&p, &g).is_err());
        assert!(grid_layout(&w[..10]).is_err());
    }

    #[test]
    fn z_order() {
        assert_eq!([0, 1, 2, 3, 4, 9, 15].map(unmorton), [(0, 0), (1, 0), (0, 1), (1, 1), (2, 0), (1, 2), (3, 3)]);
    }

    #[test]
    fn bad_fakes_are_errors() {
        assert!(fake_entities(&[0; 3], &GridLayout { cell: 32.0, origin: [0.0; 2], width: 32, outside_radius: 0 }).is_err());
    }
}
