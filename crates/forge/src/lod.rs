//! Level of detail (`LODSelector`, 51dc6b80): which of an object's meshes draws at a distance.
//!
//! An entity's visual points at either a mesh or a selector. Selector body (offsets from the body, after the object
//! header):
//! - +0x00 u32 `?` (0 in every selector seen)
//! - +0x04 five inline `LODDescriptor`s (a360319a), 21 bytes each: u32 id, u32 class, u32 graphic (a mesh id, 0 for an
//!   empty slot), f32 distance (m), f32 fade width (m), u8 `?` (1 on the fourth descriptor of most selectors)
//! - +0x6D u8 flags: bit 0 keeps empty slots (an empty slot then draws nothing over its band)
//! - +0x6E u8 flags: bit 0 no cross-fade between levels
//!
//! How the game reads it (as Banned445's AC1-Movement-Rewritten (MIT) found it, checked here on the data): only the first
//! four descriptors count. Each kept slot draws up to its distance, the next kept slot from there; past the last kept
//! slot's distance that one stays (there is no cut-off: the world's streaming grid unloads far objects). A slot with a
//! mesh is kept; an empty one is kept only with flag bit 0, otherwise an empty slot before the last mesh pushes the slot
//! before it out to its own distance, and empty slots at the start or after the last mesh are dropped. The distance is
//! the camera's to the object's box, times tan(fov_y / 2) / tan(22.5 deg) (1 at a 45 degree view). The cross-fade
//! between levels is off in the retail game, so a level switches at its distance. The `lodcheck` example reads every
//! selector in the install.

use anyhow::{Result, ensure};

pub const CLASS_LOD_SELECTOR: u32 = 0x51dc_6b80;
pub const CLASS_LOD_DESCRIPTOR: u32 = 0xa360_319a;

const SLOTS: usize = 5;
const SLOT_SIZE: usize = 21;
const FIRST_SLOT: usize = 4;
const FLAGS: usize = FIRST_SLOT + SLOTS * SLOT_SIZE;

fn u32_at(d: &[u8], p: usize) -> Option<u32> {
    d.get(p..p + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// One descriptor.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LodSlot {
    /// The mesh it draws; 0 for an empty slot.
    pub mesh: u32,
    /// How far it draws (m).
    pub distance: f32,
    /// The cross-fade's width (m).
    pub fade: f32,
    /// The trailing byte `?`.
    pub tail: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LodSelector {
    pub slots: [LodSlot; SLOTS],
    pub keep_empty: bool,
    pub no_fade: bool,
}

/// A level as drawn: its mesh (None draws nothing) up to `until` metres (the last one: infinity).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LodLevel {
    pub mesh: Option<u32>,
    pub until: f32,
}

/// Parse a `LODSelector` body.
pub fn parse_lod_selector(body: &[u8]) -> Result<LodSelector> {
    ensure!(body.len() >= FLAGS + 2, "LOD selector of {} bytes, wanted {}", body.len(), FLAGS + 2);
    let mut slots = [LodSlot::default(); SLOTS];
    for (k, s) in slots.iter_mut().enumerate() {
        let p = FIRST_SLOT + k * SLOT_SIZE;
        let class = u32_at(body, p + 4).unwrap_or(0);
        ensure!(class == CLASS_LOD_DESCRIPTOR, "class {class:08x} at {:#x}, wanted LODDescriptor", p + 4);
        let f = |o: usize| f32::from_bits(u32_at(body, p + o).unwrap_or(0));
        *s = LodSlot { mesh: u32_at(body, p + 8).unwrap_or(0), distance: f(12), fade: f(16), tail: body[p + 20] };
        ensure!(s.distance.is_finite() && s.distance >= 0.0, "distance {} at {:#x}", s.distance, p + 12);
    }
    Ok(LodSelector { slots, keep_empty: body[FLAGS] & 1 != 0, no_fade: body[FLAGS + 1] & 1 != 0 })
}

impl LodSelector {
    /// The levels the game draws, nearest first (see the module docs).
    pub fn levels(&self) -> Vec<LodLevel> {
        let used = &self.slots[..4];
        let Some(last_mesh) = used.iter().rposition(|s| s.mesh != 0) else { return vec![] };
        let mut out: Vec<LodLevel> = vec![];
        for (i, s) in used.iter().enumerate() {
            if s.mesh != 0 || self.keep_empty {
                out.push(LodLevel { mesh: (s.mesh != 0).then_some(s.mesh), until: s.distance });
            } else if i < last_mesh
                && let Some(prev) = out.last_mut()
            {
                prev.until = s.distance;
            }
        }
        // (Kept empty slots after the last mesh draw nothing from there on.)
        if let Some(l) = out.last_mut() {
            l.until = f32::INFINITY;
        }
        out
    }
}

/// The level drawn at LOD distance `d`: the first whose `until` reaches it.
pub fn pick(levels: &[LodLevel], d: f32) -> Option<usize> {
    levels.iter().position(|l| d <= l.until).or(levels.len().checked_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(slots: [(u32, f32); 5], keep_empty: bool) -> Vec<u8> {
        let mut b = vec![0u8; 4];
        for (k, (mesh, d)) in slots.iter().enumerate() {
            b.extend((100 + k as u32).to_le_bytes());
            b.extend(CLASS_LOD_DESCRIPTOR.to_le_bytes());
            b.extend(mesh.to_le_bytes());
            b.extend(d.to_le_bytes());
            b.extend(0f32.to_le_bytes());
            b.push(0);
        }
        b.extend([keep_empty as u8, 0]);
        b
    }

    fn levels(slots: [(u32, f32); 5], keep_empty: bool) -> Vec<(Option<u32>, f32)> {
        parse_lod_selector(&body(slots, keep_empty)).unwrap().levels().iter().map(|l| (l.mesh, l.until)).collect()
    }

    #[test]
    fn an_empty_slot_pushes_the_one_before_it_out() {
        // Full detail to 20 m, the first LOD to 80 m (the empty slot's distance), the third from there on.
        assert_eq!(levels([(1, 20.0), (2, 40.0), (0, 80.0), (4, 160.0), (0, 320.0)], false), [(Some(1), 20.0), (Some(2), 80.0), (Some(4), f32::INFINITY)]);
        // Leading empty slots and the fifth descriptor are not used.
        assert_eq!(levels([(0, 10.0), (1, 20.0), (0, 30.0), (0, 40.0), (5, 50.0)], false), [(Some(1), f32::INFINITY)]);
        assert_eq!(levels([(0, 10.0), (0, 20.0), (0, 30.0), (0, 40.0), (0, 50.0)], false), []);
    }

    #[test]
    fn kept_empty_slots_draw_nothing() {
        let l = parse_lod_selector(&body([(1, 50.0), (0, 51.0), (0, 80.0), (0, 160.0), (0, 320.0)], true)).unwrap().levels();
        assert_eq!([pick(&l, 40.0), pick(&l, 60.0), pick(&l, 1e5)].map(|i| i.and_then(|i| l[i].mesh)), [Some(1), None, None]);
    }

    #[test]
    fn picks_by_distance() {
        let l = parse_lod_selector(&body([(1, 20.0), (2, 40.0), (0, 80.0), (4, 160.0), (0, 320.0)], false)).unwrap().levels();
        assert_eq!([0.0, 19.0, 30.0, 70.0, 100.0, 1e5].map(|d| pick(&l, d)), [Some(0), Some(0), Some(1), Some(1), Some(2), Some(2)]);
        assert_eq!(pick(&[], 5.0), None);
    }

    #[test]
    fn rejects_short_or_foreign_bodies() {
        assert!(parse_lod_selector(&[0; 20]).is_err());
        let mut b = body([(1, 20.0), (0, 0.0), (0, 0.0), (0, 0.0), (0, 0.0)], false);
        b[8] ^= 1;
        assert!(parse_lod_selector(&b).is_err());
    }

    #[test]
    fn class_ids_are_their_names() {
        assert_eq!(crc32fast::hash(b"LODSelector"), CLASS_LOD_SELECTOR);
        assert_eq!(crc32fast::hash(b"LODDescriptor"), CLASS_LOD_DESCRIPTOR);
    }
}
