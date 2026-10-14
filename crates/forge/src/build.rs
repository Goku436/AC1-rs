//! NPC build tables (`BuildTable`, 22ecbe63, and the `EntityBuilder`, 971a842e, that names them): how AC1 dresses a
//! crowd member. A table has columns, each a property path saying what its value sets, and weighted rows whose cells
//! give the values. Class and property names are CRC32s of the executable's strings (`BuildColumn` 36839608,
//! `PropertyPath` 3c00b2e0, `PropertyPathNode` 1a8750ba, `MaterialPropertyPathNodeSolver` bc5b87af, `BuildRow`
//! 348b28d6, `Type` 2cecf817); the material parameters a path ends at by CRC32 of the templates' own names
//! (`Color_R`, `Color_G`, `Color_B`, `Dirt`, `DiffuseMask`, `MultiplyColor`, ...; `docs/NOTES.md` "NPCs").
//!
//! Body (unaligned; checked over every table of the common forge and Damascus, `examples/buildcheck`):
//! - u32 column count, then the columns, each `[u32 id][u32 BuildColumn]` and its `PropertyPath` (an inline object
//!   with its nodes: `[u32 0][u32 PropertyPathNode][u32 property CRC32]...`, a node may hold a
//!   `MaterialPropertyPathNodeSolver` `[u32 id][u32 bc5b87af][u32 ?][u32 material id][u32 template id]...`: the
//!   material the rest of the path is on, by id, or when that is 0 the visual's material of that template); the column
//!   ends with `[u32 column number][u8 1][u32 table id][u32 1][u32 Type][u32 0][u32 kind code]` and a default value.
//!   What sits between the nodes' fields is `?`; the solver, the last node's property and the column's number and
//!   kind are found by those markers.
//! - the rows, each `[u32 id][u32 BuildRow][u8 ?][u32 table id][f32 weight][u32 cells]`, then per cell
//!   `[u32 column number][u32 value class (GraphicObject, TextureBase, TextureSet, BuildTable, Skeleton, or 0)]
//!   [u32 kind code]` and a value whose size follows the code's third byte: 0x1c a link (4 bytes), 0x0d a colour
//!   (16: RGBA floats), 0x19 an enum (4), 0x07 an integer (4), 0x0a a float (4), 0 a bool (1 byte: the materials'
//!   flags, `forge::material` +0x27).

use crate::u32_at;

pub const CLASS_BUILD_TABLE: u32 = 0x22ec_be63;
pub const CLASS_BUILDER: u32 = 0x971a_842e;
const CLASS_BUILD_ROW: u32 = 0x348b_28d6;
const CLASS_BUILD_COLUMN: u32 = 0x3683_9608;
const CLASS_PATH_NODE: u32 = 0x1a87_50ba;
const CLASS_MATERIAL_SOLVER: u32 = 0xbc5b_87af;
const TYPE: u32 = 0x2cec_f817;

/// Kinds of value (the code's third byte).
pub const KIND_LINK: u8 = 0x1c;
pub const KIND_COLOR: u8 = 0x0d;

/// A cell's value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    /// A link to an object: (value class, object id).
    Link(u32, u32),
    Color([f32; 4]),
    Float(f32),
    /// An integer or an enum.
    Int(u32),
    Bool(bool),
}

/// Where a column's values go: a property (by CRC32) of a material, when its path goes through a material.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Column {
    /// The number cells name it by.
    pub number: u32,
    /// The kind code's third byte (`KIND_*`).
    pub kind: u8,
    /// The material the path solves to (0: none named, see `template`).
    pub material: u32,
    /// With no material: the template of the visual's material it is on (0: the path has no material solver).
    pub template: u32,
    /// The last node's property (CRC32 of its name; 0 when the path names none).
    pub property: u32,
    /// Every node's property in order (the zeros left out): a texture slot's field comes after the slot.
    pub path: Vec<u32>,
}

/// A cell: the column it sets and its value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cell {
    pub column: u32,
    pub value: Value,
}

/// A row: its weight and cells.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Row {
    pub weight: f32,
    pub cells: Vec<Cell>,
}

/// A table's (or a builder's) columns and rows, and where in the body the rows span (start, end).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Table {
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    pub spans: Vec<(usize, usize)>,
}

impl Table {
    /// The column cells numbered `n` set.
    pub fn column(&self, n: u32) -> Option<&Column> {
        self.columns.iter().find(|c| c.number == n)
    }
}

fn get(b: &[u8], p: usize) -> Option<u32> {
    (p + 4 <= b.len()).then(|| u32_at(b, p))
}

/// Every offset of `tag` in `b[from..to]`.
fn find(b: &[u8], tag: u32, from: usize, to: usize) -> impl Iterator<Item = usize> + '_ {
    let t = tag.to_le_bytes();
    (from..to.min(b.len()).saturating_sub(3)).filter(move |&k| b[k..k + 4] == t)
}

/// One column from its `BuildColumn` tag at `k` to `end` (the next column or row); `id` the table's own.
fn column(b: &[u8], k: usize, end: usize, id: u32) -> Option<Column> {
    // The column's tail: [number][u8 1][table id][u32 1][Type][u32 0][code].
    let mut tail = id.to_le_bytes().to_vec();
    tail.extend(1u32.to_le_bytes());
    tail.extend(TYPE.to_le_bytes());
    let t = (k..end.min(b.len()).saturating_sub(tail.len())).find(|&p| p >= 5 && b[p - 1] == 1 && b[p..p + tail.len()] == tail[..])?;
    let number = get(b, t - 5)?;
    let kind = (get(b, t + tail.len() + 4)? >> 16) as u8;
    let solver = find(b, CLASS_MATERIAL_SOLVER, k, t).next();
    let (material, template) = match solver {
        Some(s) => (get(b, s + 8)?, get(b, s + 12)?),
        None => (0, 0),
    };
    let path: Vec<u32> = find(b, CLASS_PATH_NODE, k, t).filter_map(|n| get(b, n + 4)).filter(|&p| p != 0).collect();
    let property = find(b, CLASS_PATH_NODE, k, t).last().and_then(|n| get(b, n + 4)).unwrap_or(0);
    Some(Column { number, kind, material, template, property, path })
}

/// Parse a table or builder body whose object id is `id`. Never panics; what does not read is left out (a row that
/// stops short keeps the cells before).
pub fn parse(b: &[u8], id: u32) -> Table {
    let mut out = Table::default();
    let starts: Vec<usize> = find(b, CLASS_BUILD_ROW, 4, b.len()).map(|k| k - 4).collect();
    let first_row = starts.first().copied().unwrap_or(b.len());
    let cols: Vec<usize> = find(b, CLASS_BUILD_COLUMN, 4, first_row).collect();
    for (i, &k) in cols.iter().enumerate() {
        let end = cols.get(i + 1).copied().unwrap_or(first_row);
        if let Some(c) = column(b, k, end, id) {
            out.columns.push(c);
        }
    }
    for (i, &s) in starts.iter().enumerate() {
        let limit = starts.get(i + 1).copied().unwrap_or(b.len());
        let mut p = s + 8 + 5;
        let (Some(w), Some(n)) = (get(b, p).map(f32::from_bits), get(b, p + 4)) else { continue };
        p += 8;
        let mut row = Row { weight: if w.is_finite() && w >= 0.0 { w } else { 0.0 }, cells: vec![] };
        for _ in 0..n.min(64) {
            let (Some(column), Some(class), Some(code)) = (get(b, p), get(b, p + 4), get(b, p + 8)) else { break };
            let kind = (code >> 16) as u8;
            let size = match kind {
                KIND_LINK | 0x19 | 0x07 | 0x0a => 4,
                KIND_COLOR => 16,
                0x00 => 1,
                _ => break,
            };
            if p + 12 + size > limit {
                break;
            }
            let v = |o: usize| get(b, p + 12 + o).unwrap_or(0);
            let value = match kind {
                KIND_LINK => Value::Link(class, v(0)),
                KIND_COLOR => Value::Color([0, 4, 8, 12].map(|o| f32::from_bits(v(o)))),
                0x0a => Value::Float(f32::from_bits(v(0))),
                0x00 => Value::Bool(b.get(p + 12).is_some_and(|&x| x != 0)),
                _ => Value::Int(v(0)),
            };
            row.cells.push(Cell { column, value });
            p += 12 + size;
        }
        out.spans.push((s, p.max(s + 8)));
        out.rows.push(row);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRAPHIC_OBJECT: u32 = 0xec6a_c357;

    /// A column as the data writes it (the fields not read left zero).
    fn column_bytes(b: &mut Vec<u8>, table: u32, number: u32, material: u32, property: u32, kind: u8) {
        b.extend(0x77u32.to_le_bytes());
        b.extend(CLASS_BUILD_COLUMN.to_le_bytes());
        b.extend([4, 0, 0, 0, 0, 0, 0, 0]);
        b.extend(0x3c00_b2e0u32.to_le_bytes());
        b.extend(2u32.to_le_bytes());
        b.extend(0u32.to_le_bytes());
        b.extend(CLASS_PATH_NODE.to_le_bytes());
        b.extend([0; 17]);
        b.extend(0x78u32.to_le_bytes());
        b.extend(CLASS_MATERIAL_SOLVER.to_le_bytes());
        b.extend(0u32.to_le_bytes());
        b.extend(material.to_le_bytes());
        b.extend([0; 20]);
        b.extend(0u32.to_le_bytes());
        b.extend(CLASS_PATH_NODE.to_le_bytes());
        b.extend(property.to_le_bytes());
        b.extend([0; 16]);
        b.extend([3, 0xff, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0]);
        b.extend(number.to_le_bytes());
        b.push(1);
        b.extend(table.to_le_bytes());
        b.extend(1u32.to_le_bytes());
        b.extend(TYPE.to_le_bytes());
        b.extend(0u32.to_le_bytes());
        b.extend((u32::from(kind) << 16).to_le_bytes());
        b.extend([0; 16]);
    }

    #[test]
    fn reads_columns_rows_and_cells() {
        let table = 0x5555;
        let mut b = 2u32.to_le_bytes().to_vec();
        column_bytes(&mut b, table, 1, 0x1ce1_5c23, 0xc199_bd71, KIND_COLOR);
        column_bytes(&mut b, table, 3, 0, 0, KIND_LINK);
        b.extend(1u32.to_le_bytes());
        for (w, link) in [(0.25f32, 0x1111u32), (0.75, 0x2222)] {
            b.extend(7u32.to_le_bytes());
            b.extend(CLASS_BUILD_ROW.to_le_bytes());
            b.push(1);
            b.extend(table.to_le_bytes());
            b.extend(w.to_le_bytes());
            b.extend(4u32.to_le_bytes());
            // A mesh link, a float, a bool, a colour.
            b.extend(3u32.to_le_bytes());
            b.extend(GRAPHIC_OBJECT.to_le_bytes());
            b.extend(0x001c_0000u32.to_le_bytes());
            b.extend(link.to_le_bytes());
            b.extend(2u32.to_le_bytes());
            b.extend(0u32.to_le_bytes());
            b.extend(0x000a_0000u32.to_le_bytes());
            b.extend(1.02f32.to_le_bytes());
            b.extend(4u32.to_le_bytes());
            b.extend(0u32.to_le_bytes());
            b.extend(0u32.to_le_bytes());
            b.push(1);
            b.extend(1u32.to_le_bytes());
            b.extend(0u32.to_le_bytes());
            b.extend(0x000d_0000u32.to_le_bytes());
            for c in [0.5f32, 0.25, 0.125, 0.0] {
                b.extend(c.to_le_bytes());
            }
        }
        let t = parse(&b, table);
        assert_eq!(
            t.columns,
            vec![
                Column { number: 1, kind: KIND_COLOR, material: 0x1ce1_5c23, template: 0, property: 0xc199_bd71, path: vec![0xc199_bd71] },
                Column { number: 3, kind: KIND_LINK, material: 0, template: 0, property: 0, path: vec![] },
            ]
        );
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[1].weight, 0.75);
        assert_eq!(
            t.rows[1].cells,
            vec![
                Cell { column: 3, value: Value::Link(GRAPHIC_OBJECT, 0x2222) },
                Cell { column: 2, value: Value::Float(1.02) },
                Cell { column: 4, value: Value::Bool(true) },
                Cell { column: 1, value: Value::Color([0.5, 0.25, 0.125, 0.0]) },
            ]
        );
        assert_eq!(t.spans[0].1, t.spans[1].0);
        // Cut short anywhere, it does not panic.
        for n in 0..b.len() {
            let _ = parse(&b[..n], table);
        }
    }
}
