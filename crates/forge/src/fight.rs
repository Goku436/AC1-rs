//! AC1's fight settings in its data: the damage every attack does by who takes it, and the fight manager's numbers per
//! memory block. Read from the user's install (DataPC.forge); nothing of them is kept in the repo.
//!
//! - `DamageData` (314aecee), inline in DataPC's `Game Bootstrap Settings`, object `Game Settings` (52534498): u32 id,
//!   u32 class, u8 `?` (0x18), u8 `?` (0x11, the source count), u32 count (442) and that many u8: the damage table
//!   [CharacterType 26][DamageSource 17], row by the character that takes the blow (the exe's enums
//!   `DamageData::CharacterType` and `DamageData::DamageSource`, their value lists at 0x19A8530 and 0x19A8678: the
//!   bosses' rows take 20 from a throwing knife, the guards' 255, so a row is the victim's). 255 kills outright.
//! - `FightManagerData` (2d120d4f), inline in each `MissionDataMemoryBlock` of DataPC's `AssassinsCreed` (objects
//!   `Memory Block 1` .. `Memory Block 7`): u32 id, u32 class, u32 `Pad`, then two inline `FightManagerDifficultyData`
//!   (d6d56d1d: u32 id, u32 class, 44 bytes, the fields in the order of the exe's property list, read by the getters
//!   0xCD7430..0xCD74F0). What the fight manager (0xAFF460, 0xB00C30, 0xAF8510) does with them is in
//!   `FightDifficulty`'s fields.

use anyhow::{Context, Result, bail, ensure};

pub const CLASS_DAMAGE_DATA: u32 = 0x314a_ecee;
pub const CLASS_FIGHT_MANAGER_DATA: u32 = 0x2d12_0d4f;
pub const CLASS_FIGHT_DIFFICULTY: u32 = 0xd6d5_6d1d;

/// `DamageData::CharacterType` (the table's rows; the exe's value list at 0x19A8530).
pub mod character {
    pub const PLAYER_SWORD: usize = 0;
    pub const PLAYER_DAGGER: usize = 1;
    pub const PLAYER_SWORD_UPGRADE: usize = 2;
    pub const MILITIA: usize = 3;
    pub const ELITE: usize = 4;
    pub const LEADER: usize = 5;
    pub const CIVILIAN: usize = 23;
    pub const COUNT: usize = 26;
}

/// `DamageData::DamageSource` (the table's columns; the exe's value list at 0x19A8678).
pub mod source {
    pub const QUICK_ATK: usize = 0;
    pub const STRONG_ATK: usize = 1;
    pub const EXTRA_DAMAGE_HURT_ATK: usize = 2;
    pub const COUNTER_QUICK: usize = 3;
    pub const COUNTER_STRONG: usize = 4;
    pub const COMBO_KILL: usize = 5;
    pub const QUICK_PUNCH: usize = 6;
    pub const STRONG_PUNCH: usize = 7;
    pub const CLOTHESLINE: usize = 8;
    pub const SWORD_BLOCK: usize = 9;
    pub const THROWING_KNIFE: usize = 13;
    pub const ARROW: usize = 14;
    pub const ROCK: usize = 15;
    pub const THROW: usize = 16;
    pub const COUNT: usize = 17;
}

/// The damage table: `rows[victim][source]`.
#[derive(Debug, Clone)]
pub struct DamageTable {
    pub rows: Vec<[u8; source::COUNT]>,
}

impl DamageTable {
    /// The damage `source` does to a `victim` (0 for one out of range).
    pub fn get(&self, victim: usize, source: usize) -> u8 {
        self.rows.get(victim).and_then(|r| r.get(source)).copied().unwrap_or(0)
    }
}

/// One `FightManagerDifficultyData`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FightDifficulty {
    /// +0 / +4: how many fighters the manager keeps in the fight, ramped from the first to the second over `ramp`
    /// seconds of fighting (0xAFF460 -> +0x2A8; 0xB00C30 sets the rest aside, retrying 0.9..1.2 s later).
    pub engaged: [u32; 2],
    /// +8 / +0xC: the share of the fighters sent at the player at once (a group attack, 0xAFEBB0 with round(n x
    /// share)), ramped the same way.
    pub group_share: [f32; 2],
    /// +0x10 / +0x14: the seconds between group attacks, ramped from the first down to the second.
    pub group_interval: [f32; 2],
    /// +0x18: the ramp's length (s; the ramp advances 1 s a second).
    pub ramp: f32,
    /// +0x1C: the wait (s) before the ramp starts again, per fighter counted at +0x2B8 (0xAF4E40).
    pub cooldown: f32,
    /// +0x20: how many fighters hold an attack token at once (+0x30C against the count +0x324: 0xAF8510 grants one,
    /// 0xAFBD20 gives it back).
    pub attackers: u32,
    /// +0x24: the most fighters the fight draws in (0xB00580 lets the farthest go past it).
    pub drawn_in: u32,
    /// +0x28: `?` (3 everywhere; read by 0xAFE990).
    pub unknown: u32,
}

fn u32_at(d: &[u8], o: usize) -> Result<u32> {
    let b = d.get(o..o + 4).with_context(|| format!("truncated at {o:#x}"))?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn f32_at(d: &[u8], o: usize) -> Result<f32> {
    Ok(f32::from_bits(u32_at(d, o)?))
}

/// The offsets in `d` where an inline object of `class` starts (its u32 id before the class).
fn inline_objects(d: &[u8], class: u32) -> Vec<usize> {
    let pat = class.to_le_bytes();
    (4..d.len().saturating_sub(3)).filter(|&i| d[i..i + 4] == pat).map(|i| i - 4).collect()
}

/// The `DamageData` inline in an object's bytes (the `Game Settings` object), if it holds one.
pub fn find_damage_data(d: &[u8]) -> Result<Option<DamageTable>> {
    let Some(&at) = inline_objects(d, CLASS_DAMAGE_DATA).first() else { return Ok(None) };
    let body = at + 8;
    let sources = *d.get(body + 1).with_context(|| format!("DamageData truncated at {:#x}", body + 1))? as usize;
    let n = u32_at(d, body + 2)? as usize;
    ensure!(sources == source::COUNT, "DamageData at {at:#x}: {sources} sources, not {}", source::COUNT);
    ensure!(n == character::COUNT * source::COUNT, "DamageData at {at:#x}: {n} entries, not {}", character::COUNT * source::COUNT);
    let t = d.get(body + 6..body + 6 + n).with_context(|| format!("DamageData at {at:#x}: table truncated"))?;
    let rows = t
        .chunks_exact(source::COUNT)
        .map(|r| {
            let mut a = [0u8; source::COUNT];
            a.copy_from_slice(r);
            a
        })
        .collect();
    Ok(Some(DamageTable { rows }))
}

/// The `FightManagerData` inline in a memory block's bytes: its two difficulty sets.
pub fn find_fight_manager(d: &[u8]) -> Result<Option<[FightDifficulty; 2]>> {
    let Some(&at) = inline_objects(d, CLASS_FIGHT_MANAGER_DATA).first() else { return Ok(None) };
    // (u32 id, u32 class, u32 Pad, then the two sets, each u32 id, u32 class and 44 bytes.)
    let mut sets = [None, None];
    for (k, s) in sets.iter_mut().enumerate() {
        let o = at + 12 + k * 52;
        let class = u32_at(d, o + 4)?;
        if class != CLASS_FIGHT_DIFFICULTY {
            bail!("FightManagerData at {at:#x}: set {k} at {o:#x} is class {class:08x}, not FightManagerDifficultyData");
        }
        let b = o + 8;
        *s = Some(FightDifficulty {
            engaged: [u32_at(d, b)?, u32_at(d, b + 4)?],
            group_share: [f32_at(d, b + 8)?, f32_at(d, b + 0xc)?],
            group_interval: [f32_at(d, b + 0x10)?, f32_at(d, b + 0x14)?],
            ramp: f32_at(d, b + 0x18)?,
            cooldown: f32_at(d, b + 0x1c)?,
            attackers: u32_at(d, b + 0x20)?,
            drawn_in: u32_at(d, b + 0x24)?,
            unknown: u32_at(d, b + 0x28)?,
        });
    }
    match sets {
        [Some(a), Some(b)] => Ok(Some([a, b])),
        _ => Ok(None),
    }
}

/// What the fight reads from the install.
#[derive(Debug, Clone)]
pub struct FightSettings {
    pub damage: DamageTable,
    /// Memory blocks 1..7 (index 0 is block 1): the two difficulty sets each.
    pub blocks: Vec<[FightDifficulty; 2]>,
}

/// AC1's fight settings from DataPC.forge.
pub fn read_settings(forge: &crate::Forge) -> Result<FightSettings> {
    let data = |name: &str| -> Result<Vec<u8>> {
        let e = forge.entries.iter().find(|e| e.name == name).with_context(|| format!("no data file {name}"))?;
        forge.read(e)
    };
    let gbs = data("Game Bootstrap Settings")?;
    let objs = crate::parse_objects(&gbs)?;
    let damage =
        objs.iter().filter(|o| o.name == "Game Settings").find_map(|o| find_damage_data(o.bytes).transpose()).context("no DamageData in Game Settings")??;
    let ac = data("AssassinsCreed")?;
    let objs = crate::parse_objects(&ac)?;
    let mut blocks = vec![];
    for n in 1..=7 {
        let name = format!("Memory Block {n}");
        let o = objs.iter().find(|o| o.name == name).with_context(|| format!("no {name}"))?;
        blocks.push(find_fight_manager(o.bytes)?.with_context(|| format!("no FightManagerData in {name}"))?);
    }
    Ok(FightSettings { damage, blocks })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_damage_table_parses() {
        // (id, class, the two bytes, the count, then the table: the first row's first two set.)
        let mut d = vec![0u8; 3];
        d.extend(7u32.to_le_bytes());
        d.extend(CLASS_DAMAGE_DATA.to_le_bytes());
        d.extend([0x18, 0x11]);
        d.extend(442u32.to_le_bytes());
        let mut t = vec![0u8; 442];
        t[0] = 10;
        t[17 + 1] = 14;
        d.extend(t);
        let dt = find_damage_data(&d).unwrap().unwrap();
        assert_eq!(dt.rows.len(), 26);
        assert_eq!(dt.get(0, 0), 10);
        assert_eq!(dt.get(1, 1), 14);
        assert_eq!(dt.get(99, 0), 0);
    }

    #[test]
    fn a_short_table_is_an_error() {
        let mut d = vec![];
        d.extend(7u32.to_le_bytes());
        d.extend(CLASS_DAMAGE_DATA.to_le_bytes());
        d.extend([0x18, 0x11]);
        d.extend(442u32.to_le_bytes());
        d.extend([0u8; 20]);
        assert!(find_damage_data(&d).is_err());
        assert!(find_damage_data(&[1, 2, 3]).unwrap().is_none());
    }

    #[test]
    fn fight_manager_sets_parse() {
        let mut d = vec![];
        d.extend(1u32.to_le_bytes());
        d.extend(CLASS_FIGHT_MANAGER_DATA.to_le_bytes());
        d.extend(0u32.to_le_bytes());
        for k in 0..2u32 {
            d.extend((10 + k).to_le_bytes());
            d.extend(CLASS_FIGHT_DIFFICULTY.to_le_bytes());
            for v in [1u32, 2] {
                d.extend(v.to_le_bytes());
            }
            for f in [0.15f32, 0.25, 60.0, 30.0, 1.0, 8.0] {
                d.extend(f.to_bits().to_le_bytes());
            }
            for v in [2u32 + k, 8, 3] {
                d.extend(v.to_le_bytes());
            }
        }
        let s = find_fight_manager(&d).unwrap().unwrap();
        assert_eq!(s[0].engaged, [1, 2]);
        assert_eq!(s[1].attackers, 3);
        assert_eq!(s[0].group_interval, [60.0, 30.0]);
        // (Cut short: an error, not a panic.)
        assert!(find_fight_manager(&d[..60]).is_err());
    }
}
