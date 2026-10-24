//! The player's abilities (`AssassinAbilitySet`, class ba317db0): which moves the input interpreter may ask for.
//!
//! This is the "move mask" the ground's interpreter (0xEE65A0) checks before a jump, the pull-down or the look-down: not
//! a word of the playing action item but the player's ability stack (`BhvAssassinData` +0x290). 0xEF0320 asks it with
//! a getter (0xD324C0 on: one per flag, `shrd` / `shr` by the bit) and allows the move when every set on the stack
//! (a base set and the top of a pushed list) has `AllActions` and the flag. The base set is the player's rank: the data
//! files "Rank 0" ... "Rank 9" in DataPC.forge each hold a rank object (class 76a91f10) with the set embedded. Zones and
//! mission clips push further sets (not read).
//!
//! In memory (reflection 0x19B4FF8, 38 fields): +8 a 64-bit set of flags, +0x10 `MaxSpeed` (0 no movement, 1 walk,
//! 2 jog, 3 run, 4 sprint), +0x14 bit 0 `HighProfile`, bit 1 `AllActions`, +0x15 `?` (hash 2a0224ca). The flags'
//! names are CRC32 hashes in the reflection; those found among the executable's strings are in `bit`.
//!
//! Serialized (the rank object's body: u32 `?`, then u32 id, u32 class ba317db0): u32 `MaxSpeed`, one byte per flag,
//! bits 32 and 33 first, then 0 ... 31 (Banned445's AC1-Movement-Rewritten, checked live there against a save's base
//! set; read straight, rank 0 would lack the fight and have the counters), then `HighProfile`, `AllActions`, `?`.
//!
//! Every rank allows the movement abilities (jump, pass-over, walling, climb, ladder, leap of faith, look-down) at
//! sprint; ranks 0 ... 3 lack `Grasp`, the empty hand's catch in a fall (0xEDBC26).

use anyhow::{Result, bail, ensure};

pub const CLASS_RANK: u32 = 0x76a91f10;
pub const CLASS_ABILITY_SET: u32 = 0xba317db0;

/// The flags' bits (getters 0xD324C0 on). Unnamed bits stay `?`: 8 (hash 7aded82a), 11 (be76e963), 21 (b5908a27),
/// 22 (47a0f50c), 25 (a39abf3f), 31 (06382f16, asked by the interpreter, 0xD32B40), 33 (d7c8ae5c).
pub mod bit {
    pub const JUMP: u32 = 0;
    pub const CROUCH: u32 = 1;
    pub const PASS_OVER: u32 = 2;
    pub const WALLING: u32 = 3;
    pub const CLIMB: u32 = 4;
    pub const LADDER: u32 = 5;
    pub const GRASP: u32 = 6;
    pub const PERSUASION: u32 = 7;
    pub const STEALTH_ASSASSINATION: u32 = 9;
    pub const AIR_ASSASSINATION: u32 = 10;
    pub const FIGHT: u32 = 12;
    pub const TARGETING: u32 = 13;
    pub const CHARGE: u32 = 14;
    pub const LEAP_OF_FAITH: u32 = 15;
    pub const COUNTER_GRAB: u32 = 16;
    pub const COUNTER_DODGE: u32 = 17;
    pub const COUNTER_KILL: u32 = 18;
    pub const COMBO_KILL: u32 = 19;
    pub const GRAB_BREAKER: u32 = 20;
    pub const PUSH: u32 = 23;
    pub const DISMOUNT: u32 = 24;
    pub const JUMP_FROM_HORSE: u32 = 26;
    pub const REARING: u32 = 27;
    pub const FIRST_PERSON_VIEW: u32 = 28;
    pub const LOOK_DOWN: u32 = 29;
    pub const EAGLE_VISION: u32 = 30;
    pub const BLEND_PRAY: u32 = 32;
}

/// The serialized flags.
const FLAG_COUNT: usize = 34;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AbilitySet {
    pub flags: u64,
    /// 0 no movement, 1 walk, 2 jog, 3 run, 4 sprint.
    pub max_speed: u32,
    pub high_profile: bool,
    pub all_actions: bool,
}

impl AbilitySet {
    /// Everything allowed (rank 9's set).
    pub const ALL: AbilitySet = AbilitySet { flags: (1 << FLAG_COUNT) - 1, max_speed: 4, high_profile: true, all_actions: true };

    /// 0xEF0320 with this set alone: `AllActions` and the flag.
    pub fn allows(&self, bit: u32) -> bool {
        self.all_actions && self.flags >> bit & 1 != 0
    }

    /// An embedded set: u32 id, u32 class, then the fields (see the module docs).
    pub fn parse(d: &[u8]) -> Result<AbilitySet> {
        ensure!(d.len() >= 12 + FLAG_COUNT + 3, "ability set: {} bytes", d.len());
        let class = u32::from_le_bytes(d[4..8].try_into().unwrap());
        if class != CLASS_ABILITY_SET {
            bail!("ability set: class {class:08x} at 0x4");
        }
        let max_speed = u32::from_le_bytes(d[8..12].try_into().unwrap());
        ensure!(max_speed <= 4, "ability set: max speed {max_speed} at 0x8");
        let bytes = &d[12..12 + FLAG_COUNT + 2];
        let flags = (0..FLAG_COUNT).filter(|&i| bytes[i] != 0).fold(0u64, |f, i| f | 1 << ((i + 32) % FLAG_COUNT));
        Ok(AbilitySet { flags, max_speed, high_profile: bytes[FLAG_COUNT] != 0, all_actions: bytes[FLAG_COUNT + 1] != 0 })
    }

    /// A rank object's body: u32 `?`, then its set.
    pub fn from_rank(body: &[u8]) -> Result<AbilitySet> {
        ensure!(body.len() >= 4, "rank: {} bytes", body.len());
        AbilitySet::parse(&body[4..]).map_err(|e| e.context("rank's ability set at 0x4"))
    }
}

/// Rank `rank`'s set from the install's DataPC.forge (the data file "Rank <n>").
pub fn rank_set(game: &std::path::Path, rank: u32) -> Result<AbilitySet> {
    let f = crate::Forge::open(game.join("DataPC.forge"))?;
    let name = format!("Rank {rank}");
    let Some(e) = f.entries.iter().find(|e| e.name == name) else { bail!("no data file {name}") };
    let data = f.read(e)?;
    let objs = crate::parse_objects(&data)?;
    let Some(o) = objs.iter().find(|o| o.class == CLASS_RANK) else { bail!("{name}: no rank object") };
    AbilitySet::from_rank(o.body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_are_read_with_bits_32_and_33_first() {
        let mut d = vec![0u8; 12 + FLAG_COUNT + 3];
        d[4..8].copy_from_slice(&CLASS_ABILITY_SET.to_le_bytes());
        d[8] = 4;
        d[12] = 1; // bit 32
        d[12 + 2 + bit::GRASP as usize] = 1;
        d[12 + FLAG_COUNT + 1] = 1;
        let s = AbilitySet::parse(&d).unwrap();
        assert_eq!(s.flags, 1 << 32 | 1 << bit::GRASP);
        assert!(s.allows(bit::GRASP) && !s.allows(bit::JUMP));
        assert_eq!(s.max_speed, 4);
        d[12 + FLAG_COUNT + 1] = 0;
        assert!(!AbilitySet::parse(&d).unwrap().allows(bit::GRASP), "without AllActions nothing is allowed");
        d[5] = 0;
        assert!(AbilitySet::parse(&d).is_err());
    }
}
