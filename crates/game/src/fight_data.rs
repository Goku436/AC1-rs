//! AC1's fight tables and choices, as the executable holds them (AssassinsCreed_Dx9.exe v1.02, read in Ghidra; the
//! numbers are the game's, the code is ours). Pure: no Bevy, no game data; `combat` turns the action ids into clips
//! through the move graph (`forge::graph::MoveGraph::action_clips`).
//!
//! Where they come from:
//! - The fight's action tables are statics the exe fills at start (initializers 0xF319A0, 0xF324E0): per weapon (the
//!   sword is weapon 1 of 0 fists, 1 sword, 2 short blade, 3 ...) the attack wind-ups by side, distance and foot, in a
//!   strong and a max (held) set (0x1A31920 / 0x1A31A48), the strikes after them with the direction the blade ends in
//!   (records {action, direction, 8} set by 0xF314C0), the combo wind-ups and strikes by direction (0x1A31B70,
//!   records at 0x1A34638), the hit reactions by direction (0x1A32398).
//! - The attack's start (0xF3E2F0): kind 0 a first attack, picked by `side_of` (0xF3E090: the target within 45 degrees
//!   of the facing is the front, to 135 the left or right, else the back), `distance_band` (0xF47760: the squared
//!   distance against 2.25 and, for the sword, 6.25: short, medium, long) and the foot ahead; kind 2 a combo hit, by the
//!   direction the last strike ended in, turned by `NEXT_DIR_AFTER_HIT` (0xF4CD50) or, after a blocked one,
//!   `NEXT_DIR_AFTER_BLOCK` (0xF4CDD0); the combo goes on while fewer than `COMBO_HITS` combo hits have played and the
//!   attack was not a max one (0xF3B820, the flag at +0xA0C).
//! - The assassinations (0xF6C480 picks the type, 0xF6D140 starts it, 0xF6CA30 the kill, 0xF6B520 / 0xF6B6F0 the
//!   victim's side): 14 types, each a lead-in and a kill for the assassin and for the victim (table 0x170A8D8), and the
//!   distance from the victim the lead-in steers the assassin's root to (0x170A8A0). See `assassination_type`.
//!
//! Ours (not found in the exe): the combo window (the strike item, the manual's "as soon as your weapon makes
//! contact"), the counter window, how long a press is held for the max attack, the guards' choices and the hit points.

/// The four blade directions of AC1's tables, in their order: upper left, down left, down right, upper right.
pub const DIRS: [&str; 4] = ["ul", "dl", "dr", "ur"];

/// The sides of a target (`side_of`), in the tables' order.
pub const SIDES: [&str; 4] = ["front", "left", "right", "back"];

/// Where a target is, by the signed angle (radians, positive to the left) from the facing to it (0xF3E090): front
/// within 45 degrees, left or right to 135, else back.
pub fn side_of(angle: f32) -> usize {
    let q = std::f32::consts::FRAC_PI_4;
    if (-q..q).contains(&angle) {
        0
    } else if angle > q && angle <= 3.0 * q {
        1
    } else if (-3.0 * q..=-q).contains(&angle) {
        2
    } else {
        3
    }
}

/// The sword's attack distance bands (0xF47760 against per-weapon squared limits at 0x170A628 and 0x170A634: 2.25 for every
/// weapon, then 6.25 for the sword, 9 for the others): 0 short (to 1.5 m), 1 medium (to 2.5 m), 2 long.
pub fn distance_band(d: f32) -> usize {
    let d2 = d * d;
    if d2 <= 2.25 {
        0
    } else if d2 <= 6.25 {
        1
    } else {
        2
    }
}

/// The sword's attack wind-ups: [strong, max][side][distance][foot left, right] (0x1A31980, 0x1A31AA8).
pub const ATTACK_START: [[[[u32; 2]; 3]; 4]; 2] = [
    [
        [[0x11228474, 0x11228478], [0x1122846c, 0x11228470], [0x11228464, 0x11228468]],
        [[0x1122848c, 0x11228490], [0x11228484, 0x11228488], [0x1122847c, 0x11228480]],
        [[0x112284a4, 0x112284a8], [0x1122849c, 0x112284a0], [0x11228494, 0x11228498]],
        [[0x1122845c, 0x11228460], [0x11228454, 0x11228458], [0x1122844c, 0x11228450]],
    ],
    [
        [[0x120fe5a9, 0x11a7019d], [0x11a70195, 0x11a70199], [0x11a7018d, 0x11a70191]],
        [[0x120fe5ac, 0x11a701b1], [0x11a701a9, 0x11a701ad], [0x11a701a1, 0x11a701a5]],
        [[0x120fe5af, 0x11a701c5], [0x11a701bd, 0x11a701c1], [0x11a701b5, 0x11a701b9]],
        [[0x120fe5a6, 0x11a70189], [0x11a70181, 0x11a70185], [0x11a70179, 0x11a7017d]],
    ],
];

/// A strike: its action, and the direction the blade ends in.
pub type Strike = (u32, u8);

/// The strikes after them: same layout (records set by 0xF314C0).
pub const ATTACK_STRIKE: [[[[Strike; 2]; 3]; 4]; 2] = [
    [
        [[(0x11228516, 3), (0x11228517, 1)], [(0x11228514, 3), (0x11228515, 3)], [(0x11228512, 3), (0x11228513, 0)]],
        [[(0x1122851c, 3), (0x1122851d, 3)], [(0x1122851a, 2), (0x1122851b, 2)], [(0x11228518, 3), (0x11228519, 2)]],
        [[(0x11228523, 1), (0x11228522, 0)], [(0x11228520, 1), (0x11228521, 3)], [(0x1122851e, 0), (0x1122851f, 1)]],
        [[(0x11228510, 0), (0x11228511, 3)], [(0x1122850e, 0), (0x1122850f, 2)], [(0x1122850c, 0), (0x1122850d, 3)]],
    ],
    [
        [[(0x120fe5a8, 3), (0x11a701d0, 0)], [(0x11a701ce, 3), (0x11a701cf, 3)], [(0x11a701cc, 0), (0x11a701cd, 1)]],
        [[(0x120fe5ab, 0), (0x11a701d5, 2)], [(0x11a701d3, 2), (0x11a701d4, 3)], [(0x11a701d1, 2), (0x11a701d2, 3)]],
        [[(0x120fe5ae, 1), (0x11a701da, 3)], [(0x11a701d8, 1), (0x11a701d9, 3)], [(0x11a701d6, 1), (0x11a701d7, 0)]],
        [[(0x120fe5a5, 1), (0x11a701cb, 2)], [(0x11a701c9, 0), (0x11a701ca, 2)], [(0x11a701c7, 0), (0x11a701c8, 2)]],
    ],
];

/// The sword's combo wind-ups: [direction][strong, max][foot] (0x1A31BB0).
pub const COMBO_START: [[[u32; 2]; 2]; 4] = [
    [[0x1cd350cc, 0x1cd350cf], [0x27ce1494, 0x27ce1497]],
    [[0x1cd35058, 0x1cd3505b], [0x27ce1488, 0x27ce148b]],
    [[0x1cd3505e, 0x1cd35061], [0x27ce148e, 0x27ce1491]],
    [[0x1cd35177, 0x1cd3517a], [0x27ce149a, 0x27ce149d]],
];

/// Their strikes (records at 0x1A346F8; each ends in its own direction).
pub const COMBO_STRIKE: [[[u32; 2]; 2]; 4] = [
    [[0x1cd350cd, 0x1cd350d0], [0x27ce1495, 0x27ce1498]],
    [[0x1cd35059, 0x1cd3505c], [0x27ce1489, 0x27ce148c]],
    [[0x1cd3505f, 0x1cd35062], [0x27ce148f, 0x27ce1492]],
    [[0x1cd35178, 0x1cd3517b], [0x27ce149b, 0x27ce149e]],
];

/// The next combo hit's direction from the last strike's, after a hit (0xF4CD50, the sword's table 0x170A650) and after
/// a blocked strike (0xF4CDD0, 0x170A640).
pub const NEXT_DIR_AFTER_HIT: [u8; 4] = [1, 0, 3, 0];
pub const NEXT_DIR_AFTER_BLOCK: [u8; 4] = [3, 2, 0, 1];

/// A combo goes on while fewer than this many combo hits have played (0xF3B820: the count at +0x9E0 under 2).
pub const COMBO_HITS: u32 = 2;

/// The hit reactions: [the blow's direction][quick, strong][foot] (0x1A323D8).
pub const HIT_REACTION: [[[u32; 2]; 2]; 4] = [
    [[0x01bb2ce5, 0x01bb2cf9], [0xcd651b90, 0xcd651b92]],
    [[0x0e46fc99, 0x0e46fc9a], [0xcd651b88, 0xcd651b8a]],
    [[0x0e46fc98, 0x01bb2cf7], [0xcd651b8c, 0xcd651b8e]],
    [[0x01bb2ce2, 0x01bb2cf5], [0xcd651b94, 0xcd651b96]],
];

/// The defence's waits [foot] (0x1A3178C; the light stance's, 0x00FD645B / 0x0108295C at 0x1A3176C, list no items, only
/// idle variants), the walks in stance, 8-way blend spaces (0x1A317C0, the defence's 0x1A317B8).
pub const DEFENCE_WAIT: [u32; 2] = [0x1122a264, 0x1122a265];
pub const STRAFE: [u32; 2] = [0x0429c55b, 0x0429dbae];
pub const DEFENCE_STRAFE: [u32; 2] = [0x1122a263, 0x1122a262];

/// The 8 directions of the stance's walk blend space, in its items' order.
pub const STRAFE_DIRS: [&str; 8] = ["front", "frontright", "right", "backright", "back", "backleft", "left", "frontleft"];

/// Which of the 8 walks in stance goes `angle` (radians, positive to the left of the facing).
pub fn strafe_dir(angle: f32) -> usize {
    // (Clockwise from the front: front, front right, right ... as the blend space lists them.)
    let k = (-angle / std::f32::consts::FRAC_PI_4).round() as i32;
    k.rem_euclid(8) as usize
}

/// The attack the sword plays at a target on `side`, `dist` away, the foot `foot` ahead (0 left, 1 right), strong or
/// max: (wind-up action, strike action, the direction its blade ends in).
pub fn attack(max: bool, side: usize, dist: usize, foot: usize) -> (u32, u32, u8) {
    let (m, s, d, f) = (max as usize, side.min(3), dist.min(2), foot.min(1));
    let (strike, dir) = ATTACK_STRIKE[m][s][d][f];
    (ATTACK_START[m][s][d][f], strike, dir)
}

/// A combo hit in direction `dir`: (wind-up, strike).
pub fn combo(dir: u8, max: bool, foot: usize) -> (u32, u32) {
    let (d, m, f) = ((dir as usize).min(3), max as usize, foot.min(1));
    (COMBO_START[d][m][f], COMBO_STRIKE[d][m][f])
}

/// The next combo hit's direction after a strike that ended in `dir`.
pub fn next_combo_dir(dir: u8, blocked: bool) -> u8 {
    let t = if blocked { NEXT_DIR_AFTER_BLOCK } else { NEXT_DIR_AFTER_HIT };
    t[(dir as usize).min(3)]
}

/// The hit reaction to a blow from `dir`.
pub fn hit_reaction(dir: u8, strong: bool, foot: usize) -> u32 {
    HIT_REACTION[(dir as usize).min(3)][strong as usize][foot.min(1)]
}

/// The paired combo kill for a combo hit in `dir` (`HumanGround_ComboKill`: light_01 kills from ur against a death
/// from ul, 02 ul / ur, 03 dr / dl, 04 dl / dr): its number, the killer's direction and the victim's. (Which one AC1
/// plays for which hit is not found: ours picks by the combo's direction.)
pub fn combo_kill(dir: u8) -> (u8, &'static str, &'static str) {
    match dir {
        0 => (2, "ul", "ur"),
        1 => (4, "dl", "dr"),
        2 => (3, "dr", "dl"),
        _ => (1, "ur", "ul"),
    }
}

// --- Assassinations -----------------------------------------------------------------------------------------------

/// The 14 assassination types (0xF6C480's result, the index of 0x170A8D8's tables).
pub const ASS_FRONT: u8 = 0;
pub const ASS_RUN_FRONT: u8 = 4;
pub const ASS_RUN_FRONT_GROUND: u8 = 2;
pub const ASS_STEALTH_FRONT: u8 = 10;
/// Odd types are the back variants of the even ones before them.
pub const ASS_BACK_BIT: u8 = 1;

/// Per type: the assassin's lead-in, the assassin's kill, the victim's lead-in, the victim's kill (0x170A8D8, 0x170A910,
/// 0x170A948, 0x170A980). Types 6 and 7 repeat 4 and 5 (the running ones from a horse or a crowd, AC1's code sets
/// them elsewhere); 8 and 9 finish a victim on the ground.
pub const ASSASSINATION: [[u32; 4]; 14] = [
    [0x019ef756, 0x1ac36311, 0x019ef757, 0x1ac36317],
    [0x019ef758, 0x1ac36310, 0x019ef759, 0x1ac36316],
    [0x01e32229, 0x1ac36315, 0x01e3223f, 0x1ac3631b],
    [0x01e31711, 0x1ac36314, 0x01e31712, 0x1ac3631a],
    [0x01e3123a, 0x1ac36313, 0x01e31277, 0x1ac36319],
    [0x01e30e2e, 0x1ac36312, 0x01e30e2f, 0x1ac36318],
    [0x01e3123a, 0x1ac36313, 0x01e31277, 0x1ac36319],
    [0x01e30e2e, 0x1ac36312, 0x01e30e2f, 0x1ac36318],
    [0x22cac204, 0x22cac205, 0x22cac206, 0x22cac207],
    [0x22cac200, 0x22cac201, 0x22cac202, 0x22cac203],
    [0x26ed3523, 0x26ed3525, 0x26ed3524, 0x26ed3526],
    [0x26ed3521, 0x26ed351f, 0x26ed3522, 0x26ed3520],
    [0xb67d2178, 0xb67d217b, 0xb67d217e, 0xb67d217f],
    [0xb67d20a0, 0xb67d20a1, 0xb67d212e, 0xb67d212f],
];

/// Per type: how far from the victim the lead-in brings the assassin (m, 0x170A8A0).
pub const ASS_DIST: [f32; 14] = [1.12, 0.407, 1.0, 1.0, 1.656, 1.515, 1.656, 1.515, 0.0, 0.0, 0.953, 1.0, 0.953, 1.0];

/// How far a high-profile assassination reaches (m: the leap of the running ones; 0xF6C480 reads it from an object at
/// +0x224, 3 when there is none).
pub const ASS_REACH: f32 = 3.0;

/// Where the assassin stands against the victim, for `assassination_type`.
#[derive(Clone, Copy, Debug)]
pub struct AssassinAt {
    /// Horizontal distance (m) and the height of the victim over the assassin (m).
    pub dist: f32,
    pub rise: f32,
    /// The assassin is in front of the victim: within 90 degrees of the way it faces.
    pub in_front: bool,
    /// Low profile (the stealth assassinations), and running (AC1's state check at the assassin's +0x2D4).
    pub low: bool,
    pub running: bool,
}

/// AC1's choice of assassination (0xF6C480), or `None` where none is possible:
/// - more than 1 m of height between them: none;
/// - high profile, not running: within 1.5 m the walking one (type 0), to `ASS_REACH` the running leap (4);
/// - high profile running: within 1 m the tackle to the ground (2), to `ASS_REACH` the running one (4);
/// - low profile: the stealth one (10) within 1.5 m, and not past its lead-in's distance + 0.5 (at least 1.5 m);
/// - from behind the victim (more than 90 degrees off its facing), the back variant (the type + 1).
///
/// (Not here: a victim on the ground, types 8 and 9; 12 and 13, which 0xF6C480 picks in low profile on a state check
/// not decoded.)
pub fn assassination_type(at: AssassinAt) -> Option<u8> {
    if at.rise.abs() > 1.0 {
        return None;
    }
    let d2 = at.dist * at.dist;
    let mut t = if at.low {
        if d2 > 2.25 {
            return None;
        }
        ASS_STEALTH_FRONT
    } else if at.running {
        if at.dist > ASS_REACH {
            return None;
        }
        if d2 > 1.0 { ASS_RUN_FRONT } else { ASS_RUN_FRONT_GROUND }
    } else if d2 > 2.25 {
        if at.dist >= ASS_REACH {
            return None;
        }
        ASS_RUN_FRONT
    } else {
        ASS_FRONT
    };
    if !at.in_front {
        t |= ASS_BACK_BIT;
    }
    if at.low && at.dist > (ASS_DIST[t as usize] + 0.5).max(1.5) {
        return None;
    }
    Some(t)
}

/// The assassin's lead-in blend for type `t` (0xF6D140 weighs it; the victim's plays as it is): its three variants in
/// the item's order (f, r, l) weighted by `angle`, the angle (radians, 0..PI) between the victim's facing and the way
/// from it to the assassin, and `left` (the assassin to its left). Front types weigh the straight one by how near
/// square in front the assassin stands, back types by how near square behind; the rest goes to the side it is on.
pub fn lead_in_weights(t: u8, angle: f32, left: bool) -> [f32; 3] {
    let h = std::f32::consts::FRAC_PI_2;
    let f = if t & ASS_BACK_BIT == 0 { (h - angle) / h } else { (angle - h) / h }.clamp(0.0, 1.0);
    if left { [f, 0.0, 1.0 - f] } else { [f, 1.0 - f, 0.0] }
}

// --- Ours ---------------------------------------------------------------------------------------------------------

/// A press held this long (s) asks for the max attack (the manual: "hold for a slower but stronger attack"; AC1's
/// timer at +0xA90 against the clock, its length not found). Ours.
pub const MAX_HOLD: f32 = 0.3;
/// The counter: the weapon hand pressed while defending, from this long (s) before the enemy's strike lands to this
/// long after (the manual: "with the right timing"). Ours.
pub const COUNTER_BEFORE: f32 = 0.4;
pub const COUNTER_AFTER: f32 = 0.12;

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn sides_by_quadrant() {
        assert_eq!(side_of(0.0), 0);
        assert_eq!(side_of(0.7), 0);
        assert_eq!(side_of(1.0), 1);
        assert_eq!(side_of(-1.0), 2);
        assert_eq!(side_of(PI), 3);
        assert_eq!(side_of(-2.5), 3);
    }

    #[test]
    fn distance_bands() {
        assert_eq!(distance_band(1.2), 0);
        assert_eq!(distance_band(1.5), 0);
        assert_eq!(distance_band(2.0), 1);
        assert_eq!(distance_band(2.6), 2);
    }

    #[test]
    fn strafe_directions_go_clockwise() {
        assert_eq!(STRAFE_DIRS[strafe_dir(0.0)], "front");
        assert_eq!(STRAFE_DIRS[strafe_dir(-PI / 2.0)], "right");
        assert_eq!(STRAFE_DIRS[strafe_dir(PI / 2.0)], "left");
        assert_eq!(STRAFE_DIRS[strafe_dir(PI)], "back");
        assert_eq!(STRAFE_DIRS[strafe_dir(PI / 4.0)], "frontleft");
    }

    #[test]
    fn combo_strikes_follow_their_wind_ups() {
        // (Every combo's strike is the action after its wind-up in the block.)
        for d in 0..4 {
            for m in 0..2 {
                for f in 0..2 {
                    assert_eq!(COMBO_STRIKE[d][m][f], COMBO_START[d][m][f] + 1);
                }
            }
        }
    }

    #[test]
    fn combo_directions_stay_in_range() {
        for d in 0..4u8 {
            assert!(next_combo_dir(d, false) < 4 && next_combo_dir(d, true) < 4);
        }
        // (After a hit from upper right, the next one comes from upper left.)
        assert_eq!(next_combo_dir(3, false), 0);
    }

    #[test]
    fn assassination_types() {
        let at = |dist, in_front, low, running| AssassinAt { dist, rise: 0.0, in_front, low, running };
        assert_eq!(assassination_type(at(1.2, false, false, false)), Some(1));
        assert_eq!(assassination_type(at(1.2, true, false, false)), Some(0));
        assert_eq!(assassination_type(at(2.5, false, false, false)), Some(5));
        assert_eq!(assassination_type(at(3.5, false, false, false)), None);
        assert_eq!(assassination_type(at(0.8, false, false, true)), Some(3));
        assert_eq!(assassination_type(at(1.4, false, true, false)), Some(11));
        assert_eq!(assassination_type(at(1.6, false, true, false)), None);
        assert_eq!(assassination_type(AssassinAt { rise: 1.5, ..at(1.0, false, false, false) }), None);
    }

    #[test]
    fn lead_in_blends_by_the_side() {
        // Straight behind a back assassination's victim: the straight lead-in; square to its left: the left one.
        assert_eq!(lead_in_weights(1, PI, true), [1.0, 0.0, 0.0]);
        assert_eq!(lead_in_weights(1, PI / 2.0, true), [0.0, 0.0, 1.0]);
        assert_eq!(lead_in_weights(0, 0.0, false), [1.0, 0.0, 0.0]);
    }
}
