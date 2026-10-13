//! A city's crowds as AC1 lays them out: where people walk, how many, and who.
//!
//! - `RegionLayout` (17fb5aa8) `CrowdRegion_<City>` (in the data file named after the city): the city cut into crowd
//!   regions, one `RegionCell` per kind of place (Damascus: 29, "Poor Street", "Rich Souk", "No Spawn"...), each a 2D
//!   triangle mesh (a `MeshShape` in the layout's loading block, world X/Y in metres, Z 0) naming its
//!   `CrowdDutyRegion`.
//! - `CrowdDutyRegion` (5b8b9990), copies in the cell data files: the people of a kind of place. `CrowdRegion`'s
//!   `Density` (people per square metre: streets 0.07, souks 0.09-0.12, "No Spawn" 0.001), then its fractions:
//!   `CrowdFraction` {`Percent`, `CrowdFractionMembers`}: a `CrowdFractionCharacter` (a `SpawnedCharacterSpecification`
//!   naming an `EntityBuilder`) or a `CrowdFractionClique` (a group, the thugs).
//! - `CrowdFlow` (3bbecb2b), a component of the entities named `CrowdFlow_*` in the cell data files: the lanes
//!   people walk, each a polyline (points in the entity's frame) with a width, linked at its start and its end to the
//!   flows that go on from there (Damascus: 451 flows, 6.8 km).
//!
//! Byte layouts (little-endian, unaligned; names from the executable's reflection list where it has them, `?`
//! otherwise; the region cell and triangle have readers of their own, their order is the data's):
//!
//! `RegionLayout`: u32 `?` (layout type, 3), u32 cell count, cells; then an owned slot (u8 0) holding the
//! `RegionLayoutLoadingBlock`: u32 id, u32 class (f3f80267), u32 shape count, each u32 id, u32 class (`MeshShape`) and
//! its body (`shape::parse_mesh_shape`).
//!
//! `RegionCell`: u32 id, u32 class (19ced91f), f32 `?` (5), u32 the `CrowdDutyRegion`, u32 shape count + u32 shape ids,
//! f32 area (m²), f32x4 min, f32x4 max, a `RegionCellTriangleList` (u32 id, u32 class 88ade73c, u32 count, triangles),
//! u32 count of further triangle lists (0 in every layout), the user data (owned slot: u8 0 then u32 id, u32 class
//! and its body; 3 none): `CrowdDutyRegionUserData` (3caedba5) f32 `?` (the duty region's third float), u8, u8, u32
//! count + `CrowdFractionInfo` (u32 id, u32 class each, no body); `CrowdRegionUserData` (861f848d) f32, u8, u8.
//!
//! `RegionCellTriangle`: u32 id (0), u32 class (fb33e0f7), u16 its index, u8 2 + u32 its cell, 3 x i16 the neighbour
//! triangle across each edge (-32 none), 3 x the neighbour's cell (u8 2 + u32 id, or u8 3). Triangle k of a cell is
//! triangle k of its shape.
//!
//! `CrowdDutyRegion` body: f32 `Density`, f32 `?` (0), f32 `?` (1; 0.001 "No Spawn", 10 a "BlobBlackHole", 2 "Medium
//! Street"), u8, u8, u32 fraction count, fractions: u32 id, u32 class (737faf62), f32 `Percent`, the members (owned
//! slot: u8 0, u32 id, u32 class, body). The members' bodies are not walked: their builder is the `EntityBuilder` id
//! among their words (`Fraction::ids`, every u32 of the fraction's bytes; the caller keeps those that are builders).
//!
//! `CrowdFlow` component (after its u32 id and class): u8 `?`, u32 `Filter` (15; 9, 7, 8 on a few), f32 `Weight` (1),
//! u32 point count, points (u32 id, u32 class c8c23780, f32x4 in the entity's frame), u8 `?`, f32 width (m, 1.5-4),
//! u32 count + entity ids linked at the start, u32 count + entity ids linked at the end; then the meta link, the
//! navigation triangles and waypoints under it (not read).

use anyhow::{Result, bail, ensure};

pub const CLASS_REGION_LAYOUT: u32 = 0x17fb5aa8;
pub const CLASS_DUTY_REGION: u32 = 0x5b8b9990;
pub const CLASS_CROWD_FLOW: u32 = 0x3bbecb2b;
const CLASS_REGION_CELL: u32 = 0x19ced91f;
const CLASS_TRIANGLE_LIST: u32 = 0x88ade73c;
const CLASS_TRIANGLE: u32 = 0xfb33e0f7;
const CLASS_LOADING_BLOCK: u32 = 0xf3f80267;
const CLASS_DUTY_USER_DATA: u32 = 0x3caedba5;
const CLASS_REGION_USER_DATA: u32 = 0x861f848d;
const CLASS_FRACTION: u32 = 0x737faf62;
const CLASS_FRACTION_CHARACTER: u32 = 0x87e5b138;
const CLASS_FRACTION_CLIQUE: u32 = 0xcae12172;
const CLASS_FLOW_POINT: u32 = 0xc8c23780;

struct R<'a> {
    d: &'a [u8],
    p: usize,
}

impl R<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        ensure!(self.p + n <= self.d.len(), "data ends at {:#x} (wanted {n} bytes)", self.p);
        let s = &self.d[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into()?))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }
    fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.u32()?))
    }
    fn count(&mut self, size: usize) -> Result<usize> {
        let at = self.p;
        let n = self.u32()? as usize;
        ensure!(n.saturating_mul(size) <= self.d.len() - self.p, "bad count {n} at {at:#x}");
        Ok(n)
    }
    fn class(&mut self, want: u32, what: &str) -> Result<u32> {
        let id = self.u32()?;
        let at = self.p;
        let c = self.u32()?;
        ensure!(c == want, "class {c:08x} at {at:#x}, wanted {what}");
        Ok(id)
    }
    /// A link slot: u8 2 + u32 id, or u8 3 (none).
    fn slot(&mut self) -> Result<Option<u32>> {
        let at = self.p;
        match self.u8()? {
            2 => Ok(Some(self.u32()?)),
            3 => Ok(None),
            k => bail!("slot kind {k} at {at:#x}"),
        }
    }
}

/// One crowd region of a layout: which duty region it is and the ground it covers.
#[derive(Debug, Clone, PartialEq)]
pub struct RegionCell {
    pub id: u32,
    /// The `CrowdDutyRegion` it is.
    pub duty: u32,
    /// Its area (m²), as stored.
    pub area: f32,
    /// The user data's float (the duty region's third float, see the module docs), if it has one.
    pub user: Option<f32>,
    /// Its triangles: world X, Y (game space, Z up; metres).
    pub triangles: Vec<[[f32; 2]; 3]>,
}

/// Parse a `RegionLayout` body (after its header): its cells, each with its shape's triangles.
pub fn parse_region_layout(body: &[u8]) -> Result<Vec<RegionCell>> {
    let mut r = R { d: body, p: 0 };
    r.u32()?;
    let n = r.count(8)?;
    // (Cell, its shape ids.)
    let mut cells = Vec::with_capacity(n);
    for _ in 0..n {
        let id = r.class(CLASS_REGION_CELL, "RegionCell")?;
        r.f32()?;
        let duty = r.u32()?;
        let shapes = (0..r.count(4)?).map(|_| r.u32()).collect::<Result<Vec<_>>>()?;
        let area = r.f32()?;
        r.take(32)?;
        let mut tris = triangle_list(&mut r)?;
        for _ in 0..r.count(12)? {
            tris += triangle_list(&mut r)?;
        }
        let at = r.p;
        let user = match r.u8()? {
            0 => {
                let _id = r.u32()?;
                let at = r.p;
                let class = r.u32()?;
                let v = r.f32()?;
                r.take(2)?;
                match class {
                    CLASS_DUTY_USER_DATA => {
                        let k = r.count(8)?;
                        r.take(k * 8)?;
                    }
                    CLASS_REGION_USER_DATA => {}
                    c => bail!("user data class {c:08x} at {at:#x}"),
                }
                Some(v)
            }
            3 => None,
            k => bail!("user data slot {k} at {at:#x}"),
        };
        cells.push((RegionCell { id, duty, area, user, triangles: vec![] }, shapes, tris));
    }
    let at = r.p;
    ensure!(r.u8()? == 0, "no loading block at {at:#x}");
    r.class(CLASS_LOADING_BLOCK, "RegionLayoutLoadingBlock")?;
    let n = r.count(8)?;
    let mut shapes = std::collections::HashMap::new();
    for _ in 0..n {
        let id = r.class(crate::shape::CLASS_MESH_SHAPE, "MeshShape")?;
        let (s, len) = crate::shape::parse_mesh_shape_prefix(&body[r.p..])?;
        r.p += len;
        shapes.insert(id, s);
    }
    ensure!(r.p == body.len(), "{} bytes left at {:#x}", body.len() - r.p, r.p);
    let mut out = Vec::with_capacity(cells.len());
    for (mut c, ids, tris) in cells {
        for id in ids {
            let Some(s) = shapes.get(&id) else { bail!("cell {:08x}: no shape {id:08x}", c.id) };
            let v = |i: u16| s.vertices.get(i as usize).map_or([0.0; 2], |p| [p[0], p[1]]);
            c.triangles.extend(s.triangles.iter().map(|t| [v(t[0]), v(t[1]), v(t[2])]));
        }
        ensure!(c.triangles.len() == tris, "cell {:08x}: {} shape triangles, {tris} region triangles", c.id, c.triangles.len());
        out.push(c);
    }
    Ok(out)
}

/// A `RegionCellTriangleList`: its triangle count (the triangles' links are not kept).
fn triangle_list(r: &mut R) -> Result<usize> {
    r.class(CLASS_TRIANGLE_LIST, "RegionCellTriangleList")?;
    let n = r.count(30)?;
    for _ in 0..n {
        r.class(CLASS_TRIANGLE, "RegionCellTriangle")?;
        r.u16()?;
        r.slot()?;
        r.take(6)?;
        for _ in 0..3 {
            r.slot()?;
        }
    }
    Ok(n)
}

/// One fraction of a duty region's people.
#[derive(Debug, Clone, PartialEq)]
pub struct Fraction {
    pub percent: f32,
    /// A group (`CrowdFractionClique`) rather than one character.
    pub clique: bool,
    /// Every u32 of the fraction's bytes, in order and once each: its builder's id is among them.
    pub ids: Vec<u32>,
}

/// A `CrowdDutyRegion`: its density and fractions.
#[derive(Debug, Clone, PartialEq)]
pub struct DutyRegion {
    /// People per square metre.
    pub density: f32,
    /// The third float (see the module docs).
    pub weight: f32,
    pub fractions: Vec<Fraction>,
}

/// Parse a `CrowdDutyRegion` body (after its header).
pub fn parse_duty_region(body: &[u8]) -> Result<DutyRegion> {
    let mut r = R { d: body, p: 0 };
    let density = r.f32()?;
    r.f32()?;
    let weight = r.f32()?;
    r.take(2)?;
    let n = r.count(12)?;
    let starts: Vec<usize> = (r.p..body.len().saturating_sub(7)).filter(|&k| body[k + 4..k + 8] == CLASS_FRACTION.to_le_bytes()).collect();
    ensure!(starts.len() == n, "{n} fractions, {} found", starts.len());
    let mut fractions = Vec::with_capacity(n);
    for (i, &s) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(body.len());
        let mut f = R { d: &body[..end], p: s + 8 };
        let percent = f.f32()?;
        let at = f.p;
        ensure!(f.u8()? == 0, "fraction members slot at {at:#x}");
        f.u32()?;
        let clique = match f.u32()? {
            CLASS_FRACTION_CHARACTER => false,
            CLASS_FRACTION_CLIQUE => true,
            c => bail!("fraction members class {c:08x} at {:#x}", at + 5),
        };
        let mut ids = vec![];
        for k in f.p..end.saturating_sub(3) {
            let v = u32::from_le_bytes(body[k..k + 4].try_into()?);
            if v != 0 && !ids.contains(&v) {
                ids.push(v);
            }
        }
        fractions.push(Fraction { percent, clique, ids });
    }
    Ok(DutyRegion { density, weight, fractions })
}

/// A crowd flow: a lane people walk.
#[derive(Debug, Clone, PartialEq)]
pub struct Flow {
    pub filter: u32,
    pub weight: f32,
    /// World points (game space, Z up).
    pub points: Vec<[f32; 3]>,
    /// Its width (m).
    pub width: f32,
    /// The flows (entity ids) linked at its first point and at its last.
    pub start: Vec<u32>,
    pub end: Vec<u32>,
}

/// The `CrowdFlow` of an entity body (after its header: the world transform first, a column-major 4x4), if it has one.
pub fn parse_flow(entity: &[u8]) -> Result<Option<Flow>> {
    ensure!(entity.len() >= 64, "entity body of {} bytes", entity.len());
    let m: Vec<f32> = entity[..64].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
    let Some(k) = (64..entity.len().saturating_sub(3)).find(|&k| entity[k..k + 4] == CLASS_CROWD_FLOW.to_le_bytes()) else {
        return Ok(None);
    };
    let mut r = R { d: entity, p: k + 4 };
    r.u8()?;
    let filter = r.u32()?;
    let weight = r.f32()?;
    let n = r.count(24)?;
    let mut points = Vec::with_capacity(n);
    for _ in 0..n {
        r.class(CLASS_FLOW_POINT, "FlowPointPos")?;
        let (x, y, z) = (r.f32()?, r.f32()?, r.f32()?);
        r.f32()?;
        points.push([m[0] * x + m[4] * y + m[8] * z + m[12], m[1] * x + m[5] * y + m[9] * z + m[13], m[2] * x + m[6] * y + m[10] * z + m[14]]);
    }
    r.u8()?;
    let width = r.f32()?;
    let start = (0..r.count(4)?).map(|_| r.u32()).collect::<Result<Vec<_>>>()?;
    let end = (0..r.count(4)?).map(|_| r.u32()).collect::<Result<Vec<_>>>()?;
    ensure!(width.is_finite() && points.iter().flatten().all(|v| v.is_finite()), "bad flow numbers at {k:#x}");
    Ok(Some(Flow { filter, weight, points, width, start, end }))
}

/// A city's crowd data, all of it (`load_city`).
#[derive(Debug, Clone, Default)]
pub struct CityCrowd {
    pub cells: Vec<RegionCell>,
    /// The duty regions the cells name: id -> (name, region).
    pub duty: std::collections::HashMap<u32, (String, DutyRegion)>,
    /// Flows: (entity id, entity name, flow).
    pub flows: Vec<(u32, String, Flow)>,
    /// `EntityBuilder` names by id (the common forge's and the city's).
    pub builders: std::collections::HashMap<u32, String>,
    /// What did not parse: (what, why).
    pub failed: Vec<(String, String)>,
}

/// Read city `forge_file`'s crowd: its `CrowdRegion_<City>` layout, the duty regions it names, every `CrowdFlow_*`
/// entity, and the builder names (from `DataPC_Common.forge` and the city's own index). Objects are found through
/// `index::ForgeIndex` (cached), and only their data files are read, on all cores.
pub fn load_city(game_dir: &std::path::Path, forge_file: &str) -> Result<CityCrowd> {
    use crate::index::ForgeIndex;
    use anyhow::Context;
    let path = game_dir.join(forge_file);
    let idx = ForgeIndex::load(&path)?;
    let common = ForgeIndex::load(&game_dir.join("DataPC_Common.forge"))?;
    let mut out = CityCrowd::default();
    for i in [&common, &idx] {
        out.builders.extend(i.objects.iter().filter(|(_, p)| p.class == 0x971a842e).map(|(id, p)| (*id, p.name.clone())));
    }
    let layout = idx
        .objects
        .iter()
        .find(|(_, p)| p.class == CLASS_REGION_LAYOUT && p.name.starts_with("CrowdRegion_"))
        .map(|(id, p)| (*id, p.datafile.clone()))
        .context("no CrowdRegion_ layout")?;
    let duty: Vec<(u32, String)> = idx.objects.iter().filter(|(_, p)| p.class == CLASS_DUTY_REGION).map(|(id, p)| (*id, p.datafile.clone())).collect();
    let flows: Vec<(u32, String)> = idx
        .objects
        .iter()
        .filter(|(_, p)| p.class == crate::datafile::CLASS_ENTITY && p.name.starts_with("CrowdFlow_"))
        .map(|(id, p)| (*id, p.datafile.clone()))
        .collect();
    let mut files: Vec<String> = std::iter::once(layout.1.clone()).chain(duty.iter().map(|d| d.1.clone())).chain(flows.iter().map(|f| f.1.clone())).collect();
    files.sort();
    files.dedup();
    let forge = crate::Forge::open(&path)?;
    let entries: Vec<&crate::Entry> = files.iter().filter_map(|f| forge.entries.iter().find(|e| e.name == *f)).collect();
    // (On all cores, in the files' order.)
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(entries.len().max(1));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let read: Vec<std::sync::Mutex<Option<Result<Vec<u8>>>>> = entries.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(e) = entries.get(k) else { break };
                    if let Ok(mut slot) = read[k].lock() {
                        *slot = Some(forge.read(e));
                    }
                }
            });
        }
    });
    let wanted_duty: std::collections::HashSet<u32> = duty.iter().map(|d| d.0).collect();
    let wanted_flow: std::collections::HashSet<u32> = flows.iter().map(|f| f.0).collect();
    for (e, data) in entries.iter().zip(read) {
        let data = match data.into_inner().ok().flatten() {
            Some(Ok(d)) => d,
            Some(Err(err)) => {
                out.failed.push((e.name.clone(), format!("{err:#}")));
                continue;
            }
            None => continue,
        };
        let Ok(objects) = crate::parse_objects(&data) else { continue };
        for o in objects {
            if o.id == layout.0 && o.class == CLASS_REGION_LAYOUT {
                match parse_region_layout(o.body) {
                    Ok(c) => out.cells = c,
                    Err(err) => out.failed.push((o.name.clone(), format!("{err:#}"))),
                }
            } else if o.class == CLASS_DUTY_REGION && wanted_duty.contains(&o.id) && !out.duty.contains_key(&o.id) {
                match parse_duty_region(o.body) {
                    Ok(d) => {
                        out.duty.insert(o.id, (o.name.clone(), d));
                    }
                    Err(err) => out.failed.push((o.name.clone(), format!("{err:#}"))),
                }
            } else if o.class == crate::datafile::CLASS_ENTITY && wanted_flow.contains(&o.id) && !out.flows.iter().any(|f| f.0 == o.id) {
                match parse_flow(o.body) {
                    Ok(Some(f)) => out.flows.push((o.id, o.name.clone(), f)),
                    Ok(None) => out.failed.push((o.name.clone(), "no CrowdFlow component".into())),
                    Err(err) => out.failed.push((o.name.clone(), format!("{err:#}"))),
                }
            }
        }
    }
    // (By name: the same order every load.)
    out.flows.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(b: &mut Vec<u8>, v: u32) {
        b.extend(v.to_le_bytes());
    }
    fn putf(b: &mut Vec<u8>, v: f32) {
        b.extend(v.to_le_bytes());
    }

    /// A duty region of two fractions, each naming a builder.
    #[test]
    fn reads_a_duty_region() {
        let mut b = vec![];
        for v in [0.07f32, 0.0, 1.0] {
            putf(&mut b, v);
        }
        b.extend([1, 1]);
        put(&mut b, 2);
        for (pct, class, builder) in [(90.0f32, CLASS_FRACTION_CHARACTER, 0x281c891d), (10.0, CLASS_FRACTION_CLIQUE, 0x9269d7e2)] {
            put(&mut b, 0x11);
            put(&mut b, CLASS_FRACTION);
            putf(&mut b, pct);
            b.push(0);
            put(&mut b, 0x12);
            put(&mut b, class);
            b.extend([0; 8]);
            put(&mut b, builder);
            b.push(3);
        }
        let d = parse_duty_region(&b).unwrap();
        assert_eq!(d.density, 0.07);
        assert_eq!(d.fractions.len(), 2);
        assert!(!d.fractions[0].clique && d.fractions[1].clique);
        assert_eq!(d.fractions[1].percent, 10.0);
        assert!(d.fractions[0].ids.contains(&0x281c891d) && !d.fractions[0].ids.contains(&0x9269d7e2));
        // (Cut short: an error, not a panic.)
        assert!(parse_duty_region(&b[..20]).is_err());
    }

    /// A flow of two points on an entity moved to (100, 50, 2), linked at both ends.
    #[test]
    fn reads_a_flow() {
        let mut b = vec![];
        for v in [1.0f32, 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 100., 50., 2., 1.] {
            putf(&mut b, v);
        }
        b.extend([3, 0, 0, 0, 0, 0, 0, 0]);
        put(&mut b, 0x77);
        put(&mut b, CLASS_CROWD_FLOW);
        b.push(1);
        put(&mut b, 15);
        putf(&mut b, 1.0);
        put(&mut b, 2);
        for p in [[1.0f32, 2.0], [5.0, 2.0]] {
            put(&mut b, 0);
            put(&mut b, CLASS_FLOW_POINT);
            for v in [p[0], p[1], 0.0, 0.0] {
                putf(&mut b, v);
            }
        }
        b.push(1);
        putf(&mut b, 3.0);
        put(&mut b, 1);
        put(&mut b, 0xaaaa);
        put(&mut b, 2);
        put(&mut b, 0xbbbb);
        put(&mut b, 0xcccc);
        let f = parse_flow(&b).unwrap().unwrap();
        assert_eq!(f.points, vec![[101.0, 52.0, 2.0], [105.0, 52.0, 2.0]]);
        assert_eq!((f.width, f.filter, f.start.clone(), f.end.clone()), (3.0, 15, vec![0xaaaa], vec![0xbbbb, 0xcccc]));
        assert!(parse_flow(&b[..b.len() - 6]).is_err());
        assert_eq!(parse_flow(&b[..74]).unwrap(), None);
    }
}
