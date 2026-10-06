//! Clip sets (`AnimSet`, 31984f55, and the military ones, 4e361614): which clips a kind of character plays in
//! place of the shared ones (`cmma_anim_set`, a Muslim townsman: `xx_l_walk_hipm_footl` ->
//! `cmma_walk_hipm_footl_16f`). Body: u32 BodyPartTemplate link, u32 count, then count inline `AnimPair`s,
//! each u32 id, u32 class (9de9a987), u32 base clip link, u32 replacement clip link. Pairs are read by their
//! class wherever they are (the military sets wrap them in more fields).

pub const CLASS_ANIM_SET: u32 = 0x31984f55;
pub const CLASS_MILITARY_ANIM_SET: u32 = 0x4e361614;
const CLASS_ANIM_PAIR: u32 = 0x9de9a987;

/// The (base clip id, replacement clip id) pairs of a clip set's body.
pub fn anim_pairs(body: &[u8]) -> Vec<(u32, u32)> {
    let at = |p: usize| body.get(p..p + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap()));
    (4..body.len().saturating_sub(11)).filter(|&k| at(k) == Some(CLASS_ANIM_PAIR)).filter_map(|k| Some((at(k + 4)?, at(k + 8)?))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_pairs() {
        let mut b = vec![0u8; 8];
        for (x, y) in [(10u32, 20u32), (11, 21)] {
            b.extend(7u32.to_le_bytes());
            b.extend(CLASS_ANIM_PAIR.to_le_bytes());
            b.extend(x.to_le_bytes());
            b.extend(y.to_le_bytes());
        }
        assert_eq!(anim_pairs(&b), vec![(10, 20), (11, 21)]);
    }
}
