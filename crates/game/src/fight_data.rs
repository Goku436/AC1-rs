//! AC1's fight rules and choices, as the executable holds them (AssassinsCreed_Dx9.exe v1.02, read in Ghidra; the
//! numbers are the game's, the code is ours). Pure: no Bevy, no game data; the action tables are in `fight_tables`,
//! `combat` turns their action ids into clips through the move graph (`forge::graph::MoveGraph::action_clips`).
//!
//! The fighter's attack runs through a state machine in the exe (the fight state's sub-states at +0xB17, dispatched by
//! 0xF4E6E0 / 0xF523A0):
//! - the wind-up (0xF3E2F0): kind 0 a first attack picked by `side_of` (0xF3E090), `distance_band` (0xF47760) and the
//!   foot ahead, the sword's straight-ahead medium and long ones by a random blade direction (`attack`); kind 2 a combo
//!   hit (`combo`); kind 3 one at a target over 0.5 m higher or lower (`high_low`);
//! - the strike (0xF40460): the weapon hand's press latch (+0x805) and the max flag are cleared as it starts, so a press
//!   during the wind-up is dropped (not held against the combo);
//! - what the strike meets: on the body the attacker plays its `hit` follow-through (0xF4A0A0, `fight_tables::HIT_OUT`),
//!   on a block its `blocked` recoil (0xF3B820, `BLOCKED_OUT`); either opens the combo (+0xA0C) unless the attack was a
//!   max one, `COMBO_HITS` combo hits have played, or the weapon is not a blade (0xF4A050 / 0xF4A0A0 / 0xF3B820);
//! - the combo (0xF50650 / 0xF506C0 / 0xF50710): when that clip may be left, a press latched since the strike started
//!   plays the next combo hit, its direction turned by `next_combo_dir`.
//!
//! The press itself (the input interpreter `GoAssassinActionInterpreter`, 0xEE1BE0): the weapon hand's press sends
//! the attack command (`HumanGround` +0x158, 0xF3A2F0: the latch +0x805, the max flag +0x804 cleared); held
//! `MAX_HOLD` it sends +0x164 (0xD939E0: the max flag set). The attack state's first sub-state (+0xB18, 0xF36F00)
//! plays the weapon's anticipation (`fight_tables::ANTICIPATION`); let go in it, the wind-up starts at once, strong
//! (0xF50CD0 on the release event 0x84); still held when it may be left, the wind-up takes the max flag (0xF41B30), never
//! for the short blade.
//!
//! Ours (not found in the exe): the counter window, the hurt reaction a blow picks on the victim, the guards' choices
//! and their hit points (AC1's damages are `forge::fight`'s `DamageData`, its fight manager's numbers its
//! `FightManagerData`).

use crate::fight_tables::{self as ft, NONE};

/// The four blade directions of AC1's tables, in their order: upper left, down left, down right, upper right.
pub const DIRS: [&str; 4] = ["ul", "dl", "dr", "ur"];

/// The sides of a target (`side_of`), in the tables' order.
pub const SIDES: [&str; 4] = ["front", "left", "right", "back"];

/// AC1's weapons by their index in the fight tables (the fight state's +0x970).
pub const FISTS: usize = 0;
pub const SWORD: usize = 1;
pub const SHORT_BLADE: usize = 2;
/// The hidden blade: only in the stance tables (its own waits, walks and turns).
pub const HIDDEN_BLADE: usize = 3;

/// The clip-name style of each weapon's fight clips (`xx_h_<style>_...`).
pub const STYLE: [&str; 4] = ["unarmed", "light", "knife", "blade"];

/// A strike: its action, and the direction the blade ends in.
pub type Strike = (u32, u8);

/// An action id from a table slot, `None` where the exe leaves it empty.
pub fn some(id: u32) -> Option<u32> {
    (id != NONE && id != 0).then_some(id)
}

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

/// The attack distance bands (0xF47760 against per-weapon squared limits at 0x170A628 and 0x170A634: 2.25 for every
/// weapon, then 6.25 for the sword, 9 for the others): 0 short (to 1.5 m), 1 medium (to 2.5 m, 3 m), 2 long.
pub fn distance_band(d: f32, weapon: usize) -> usize {
    let d2 = d * d;
    let medium = if weapon == SWORD { 6.25 } else { 9.0 };
    if d2 <= 2.25 {
        0
    } else if d2 <= medium {
        1
    } else {
        2
    }
}

/// A first attack's choice (0xF3E2F0, kind 0): the weapon, strong or max, the target's side and distance band, the
/// foot ahead, and the blade direction the last random attack took (`None` for none).
#[derive(Clone, Copy, Debug)]
pub struct AttackAt {
    pub weapon: usize,
    pub max: bool,
    pub side: usize,
    pub dist: usize,
    pub foot: usize,
    pub last_random: Option<u8>,
}

/// An attack: its wind-up and strike actions, the direction its blade ends in, and the random direction it took (the
/// sword's straight-ahead medium and long attacks, to keep the next one off it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attack {
    pub start: u32,
    pub strike: u32,
    pub dir: u8,
    pub random: Option<u8>,
}

/// AC1's first attack (0xF3E2F0): the sword straight ahead past the short band picks a blade direction at random (`roll`
/// 0..4), moved on by one when it repeats the last; everything else comes from the side, distance and foot table.
pub fn attack(at: AttackAt, roll: u8) -> Option<Attack> {
    let (w, m, s, d, f) = (at.weapon.min(2), at.max as usize, at.side.min(3), at.dist.min(2), at.foot.min(1));
    if w == SWORD && s == 0 && d > 0 {
        let mut k = roll % 4;
        if Some(k) == at.last_random {
            k = (k + 1) % 4;
        }
        let (start, strike) = (some(ft::FRONT_START[m][f][d][k as usize])?, some(ft::FRONT_STRIKE[m][f][d][k as usize])?);
        return Some(Attack { start, strike, dir: k, random: Some(k) });
    }
    let (strike, dir) = ft::ATTACK_STRIKE[w][m][s][d][f];
    Some(Attack { start: some(ft::ATTACK_START[w][m][s][d][f])?, strike: some(strike)?, dir, random: None })
}

/// An attack at a target over `HIGH_LOW` higher (`high`) or lower (0xF3E2F0, kind 3): no combo follows it (+0xA0A).
pub fn high_low(weapon: usize, max: bool, dist: usize, high: bool, foot: usize) -> Option<Attack> {
    let (w, m, d, h, f) = (weapon.min(2), max as usize, dist.min(2), high as usize, foot.min(1));
    let (strike, dir) = ft::HIGHLOW_STRIKE[m][w][d][h][f];
    Some(Attack { start: some(ft::HIGHLOW_START[w][d][h][m][f])?, strike: some(strike)?, dir, random: None })
}

/// The height (m) past which an attack is the low or high one (0xF47760 sets kind 3).
pub const HIGH_LOW: f32 = 0.5;

/// A combo hit in direction `dir` (0xF3E2F0, kind 2): (wind-up, strike, the direction its blade ends in).
pub fn combo(weapon: usize, dir: u8, max: bool, foot: usize) -> Option<(u32, u32, u8)> {
    let (w, d, m, f) = (weapon.min(2), (dir as usize).min(3), max as usize, foot.min(1));
    let (strike, end) = ft::COMBO_STRIKE[w][d][m][f];
    Some((some(ft::COMBO_START[w][d][m][f])?, some(strike)?, end))
}

/// The next combo hit's direction after a strike that ended in `dir`: after one whose landing played a clip (a hit or
/// a block: 0xF4CDD0 / 0xF4CE50, the sword's table 0x170A640, the others' 0x170A660), or after one that met nothing
/// that answered (0xF4CD50, 0x170A650 / 0x170A670).
pub fn next_combo_dir(weapon: usize, dir: u8, landed: bool) -> u8 {
    let t: [u8; 4] = match (weapon == SWORD, landed) {
        (true, true) => [3, 2, 0, 1],
        (true, false) => [1, 0, 3, 0],
        (false, true) => [1, 2, 3, 1],
        (false, false) => [1, 3, 1, 2],
    };
    t[(dir as usize).min(3)]
}

/// A combo goes on while fewer than this many combo hits have played (the count at +0x9E0 under 2).
pub const COMBO_HITS: u32 = 2;

/// Whether a strike opens the combo (0xF4A050 / 0xF4A0A0 / 0xF3B820): not after a max attack, not past `COMBO_HITS`,
/// the blades only.
pub fn combo_opens(weapon: usize, max: bool, combos: u32) -> bool {
    !max && combos < COMBO_HITS && (weapon == SWORD || weapon == SHORT_BLADE)
}

/// What the attacker plays when its strike lands on the body (`hit`) or on a block: [the blade's direction], strong
/// or max.
pub fn landed(weapon: usize, blocked: bool, dir: u8, max: bool, foot: usize) -> Option<u32> {
    let (w, d, m, f) = (weapon.min(2), (dir as usize).min(3), max as usize, foot.min(1));
    let t = if blocked { &ft::BLOCKED_OUT } else { &ft::HIT_OUT };
    // (Some slots are empty in the exe: the other strength's then; none for this foot, nothing: the strike plays on.)
    some(t[w][d][m][f]).or(some(t[w][d][1 - m][f]))
}

/// The fists' max attack: a second attack within this long (s) of a strong one is a max one (0xF3E2F0's timer at +0xA88,
/// set to 2 s, the player's fists only).
pub const FIST_CHAIN: f32 = 2.0;

#[allow(dead_code)]
/// The 8 directions of the stance's walk blend spaces (`fight_tables::STRAFE`), in their items' order.
pub const STRAFE_DIRS: [&str; 8] = ["front", "frontright", "right", "backright", "back", "backleft", "left", "frontleft"];

/// Which of the 8 walks in stance goes `angle` (radians, positive to the left of the facing).
pub fn strafe_dir(angle: f32) -> usize {
    // (Clockwise from the front: front, front right, right ... as the blend space lists them.)
    let k = (-angle / std::f32::consts::FRAC_PI_4).round() as i32;
    k.rem_euclid(8) as usize
}

// --- Turns --------------------------------------------------------------------------------------------------------

/// The stance's turn on the spot for a target `angle` off the facing (radians, positive to the left; 0xF4BB60): none
/// within 30 degrees, the 45 degree one to 60, else the 90 degree one to 135, else the 180. Returns the turn's index in
/// `fight_tables::TURN` and the angle its clip turns (AC1 scales the clip's turn to the angle, 0xF38D10).
pub fn turn_for(angle: f32) -> Option<(usize, f32)> {
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};
    let a = angle.abs();
    if a <= PI / 6.0 {
        None
    } else if a < PI / 3.0 {
        Some((if angle >= 0.0 { 0 } else { 1 }, FRAC_PI_4))
    } else if (0.0..3.0 * FRAC_PI_4).contains(&angle) {
        Some((2, FRAC_PI_2))
    } else if angle < 0.0 && angle > -3.0 * FRAC_PI_4 {
        Some((3, FRAC_PI_2))
    } else {
        Some((4, PI))
    }
}

/// The turn's action: [weapon, 3 the hidden blade][turn][defending][foot].
pub fn turn_action(weapon: usize, turn: usize, defending: bool, foot: usize) -> Option<u32> {
    some(ft::TURN[weapon.min(3)][turn.min(4)][usize::from(!defending)][foot.min(1)])
}

// --- Paired kills -------------------------------------------------------------------------------------------------

/// The paired combo kill for a combo hit in `dir` (0xF3B9C0 / 0xF37570 / 0xF3BCC0 for the killer, by the direction; the
/// victim's by 3 - the direction): the killer's wind-up, step in and kill, the victim's death pose and death.
pub fn combo_kill(weapon: usize, dir: u8) -> Option<[u32; 5]> {
    let (w, d) = (weapon.min(2), (dir as usize).min(3));
    Some([some(ft::CK_START[w][d])?, some(ft::CK_POSITION[w][d])?, some(ft::CK_KILL[w][d])?, some(ft::CK_DEATH_POSE[w][3 - d])?, some(ft::CK_DEATH[w][3 - d])?])
}

/// The counter kill's step in and kill for a defender with `weapon` and `foot` ahead against an attacker with
/// `attacker` whose blade comes from `dir` (0xF4A2B0 / 0xF4A9A0): its `_01` set off the left foot, `_02` off the right.
pub fn counter_kill(weapon: usize, attacker: usize, dir: u8, foot: usize) -> Option<(u32, u32)> {
    let (w, a, d, f) = (weapon.min(2), attacker.min(2), (dir as usize).min(3), foot.min(1));
    Some((some(ft::COUNTER_STEP[w][0][a][d][f])?, some(ft::COUNTER_KILL[w][0][a][d][f])?))
}

/// The counter dodge to `side` (1 left, 2 right, 3 back; 0xF45CF0).
pub fn counter_dodge(weapon: usize, side: usize, foot: usize) -> Option<u32> {
    some(ft::COUNTER_DODGE[weapon.min(2)][side.min(3)][foot.min(1)])
}

/// The attacker's recoil when its strike from `dir` is deflected (0xF46090).
pub fn deflected(weapon: usize, dir: u8, foot: usize) -> Option<u32> {
    some(ft::DEFLECTED[weapon.min(2)][(dir as usize).min(3)][foot.min(1)])
}

/// A mobile attack (on the run, 0xF49DB0): kind 0 with the weapon drawn, 3 drawing it as it strikes.
pub fn mobile(weapon: usize, drawing: bool, foot: usize) -> Option<Attack> {
    let (w, k, f) = (weapon.min(2), if drawing { 3 } else { 0 }, foot.min(1));
    let (strike, dir) = ft::MOBILE_STRIKE[w][k][f];
    Some(Attack { start: some(ft::MOBILE_START[w][k][f])?, strike: some(strike)?, dir, random: None })
}

// --- Assassinations -----------------------------------------------------------------------------------------------

/// The 14 assassination types (0xF6C480's result, the index of 0x170A8D8's tables).
pub const ASS_FRONT: u8 = 0;
pub const ASS_RUN_FRONT: u8 = 4;
pub const ASS_RUN_FRONT_GROUND: u8 = 2;
pub const ASS_STEALTH_FRONT: u8 = 10;
/// From the air (the jump onto a target, `HumanInAir`'s `to_assassinate`): type 6 (7 from behind), whose kill is the
/// running one's (0x170A910 lists 4's and 5's actions again) and whose failed kill is `xx_h_assassina_failure_air_*`
/// (0xF6D970 and its kin pick 0x22009D3D.. for type 6 when the victim may not die).
pub const ASS_AIR_FRONT: u8 = 6;
/// Odd types are the back variants of the even ones before them.
pub const ASS_BACK_BIT: u8 = 1;

/// Per type: the assassin's lead-in, the assassin's kill, the victim's lead-in, the victim's kill (0x170A8D8, 0x170A910,
/// 0x170A948, 0x170A980). Types 6 and 7 are the air assassination's (`ASS_AIR_FRONT`, the running ones' actions); 8
/// and 9 finish a victim on the ground.
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

// --- The press -----------------------------------------------------------------------------------------------

/// The weapon hand held this long (s) asks for the max attack: the input interpreter's hold time (0xEE1BE0 tests the
/// button held `DAT_0170861c`, 0.2 s, and sends +0x164, which sets the max flag), as long as the anticipation clip.
pub const MAX_HOLD: f32 = 0.2;

/// The anticipation played as the weapon hand is pressed in the stance (0xF36F00).
pub fn anticipation(weapon: usize, foot: usize) -> Option<u32> {
    some(ft::ANTICIPATION[weapon.min(2)][foot.min(1)])
}

/// Whether a weapon's attack can be a max one by holding (0xF41B30 clears it for the short blade; the fists' second
/// attack within `FIST_CHAIN` is a max one as well).
pub fn max_by_hold(weapon: usize) -> bool {
    weapon != SHORT_BLADE
}

// --- Counters on max attacks and with the hidden blade --------------------------------------------------------------

/// The counter on a max attack (0xF44B40 picks it, the counter state's type 3, for a max attack the defender may kill
/// and no hidden blade out; 0xF4A2B0 plays it): the defender's step in, parry and branch, by the attacker's blade
/// direction and the defender's foot.
pub fn counter_max(dir: u8, foot: usize) -> Option<[u32; 3]> {
    let (d, f) = ((dir as usize).min(3), foot.min(1));
    Some([some(ft::COUNTER_MAX_START[d][f])?, some(ft::COUNTER_MAX_PARRY[d][f])?, some(ft::COUNTER_MAX_BRANCH[d][f])?])
}

/// The attacker's side of it: countered and stunned (its set is the defender's: `01` for the left foot).
pub fn countered_max(dir: u8, foot: usize) -> Option<[u32; 2]> {
    let (d, f) = ((dir as usize).min(3), foot.min(1));
    Some([some(ft::COUNTERED_MAX[d][f])?, some(ft::COUNTERED_MAX_STUNNED[d][f])?])
}

/// The finish after the branch by the defender's weapon (the fists kick), and the attacker's death to it.
pub fn counter_max_finish(weapon: usize, foot: usize) -> Option<(u32, u32)> {
    let (w, f) = (weapon.min(2), foot.min(1));
    Some((some(ft::COUNTER_MAX_FINISH[w][f][0])?, some(ft::COUNTERED_MAX_FINISH[w][f][0])?))
}

/// The hidden blade's counter kill: step in, kill, and the victim's death, by the attacker's blade direction and the
/// defender's foot.
pub fn blade_counter(dir: u8, foot: usize) -> Option<[u32; 3]> {
    let (d, f) = ((dir as usize).min(3), foot.min(1));
    Some([some(ft::BLADE_COUNTER_STEP[d][f])?, some(ft::BLADE_COUNTER_KILL[d][f])?, some(ft::BLADE_COUNTERED[d][f])?])
}

// --- Ours ---------------------------------------------------------------------------------------------------------

/// The counter: the weapon hand pressed while defending, from this long (s) before the enemy's strike lands to this
/// long after (the manual: "with the right timing"). Ours.
pub const COUNTER_BEFORE: f32 = 0.4;
pub const COUNTER_AFTER: f32 = 0.12;

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn at(weapon: usize, max: bool, side: usize, dist: usize, foot: usize) -> AttackAt {
        AttackAt { weapon, max, side, dist, foot, last_random: None }
    }

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
    fn distance_bands_by_weapon() {
        assert_eq!(distance_band(1.2, SWORD), 0);
        assert_eq!(distance_band(1.5, SWORD), 0);
        assert_eq!(distance_band(2.0, SWORD), 1);
        assert_eq!(distance_band(2.6, SWORD), 2);
        // (The others' medium band reaches 3 m.)
        assert_eq!(distance_band(2.6, SHORT_BLADE), 1);
    }

    #[test]
    fn every_weapon_has_its_attacks() {
        for w in [FISTS, SWORD, SHORT_BLADE] {
            for side in 0..4 {
                for dist in 0..3 {
                    for foot in 0..2 {
                        for max in [false, true] {
                            assert!(attack(at(w, max, side, dist, foot), 0).is_some(), "{w} {side} {dist} {foot} {max}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_swords_straight_attacks_take_a_new_direction() {
        let a = attack(at(SWORD, false, 0, 1, 0), 2).unwrap();
        assert_eq!(a.random, Some(2));
        let b = attack(AttackAt { last_random: Some(2), ..at(SWORD, false, 0, 1, 0) }, 2).unwrap();
        assert_eq!(b.random, Some(3));
        // (Short and to the side: the table's.)
        assert_eq!(attack(at(SWORD, false, 0, 0, 0), 2).unwrap().random, None);
        assert_eq!(attack(at(SWORD, false, 0, 0, 0), 0).unwrap(), Attack { start: 0x11228474, strike: 0x11228516, dir: 3, random: None });
    }

    #[test]
    fn combo_strikes_follow_their_wind_ups() {
        // (Every sword combo's strike is the action after its wind-up in the block, and ends in its own direction.)
        for d in 0..4u8 {
            for max in [false, true] {
                for f in 0..2 {
                    let (s, k, end) = combo(SWORD, d, max, f).unwrap();
                    assert_eq!(k, s + 1);
                    assert_eq!(end, d);
                }
            }
        }
    }

    #[test]
    fn combo_directions_stay_in_reach() {
        for w in [SWORD, SHORT_BLADE] {
            for d in 0..4u8 {
                for landed in [false, true] {
                    let n = next_combo_dir(w, d, landed);
                    assert!(n < 4);
                    // (The short blade's combos have no upper-left hit: its tables never turn to it.)
                    if w == SHORT_BLADE {
                        assert!(combo(w, n, false, 0).is_some() || combo(w, n, false, 1).is_some(), "{d} -> {n}");
                    }
                }
            }
        }
        assert_eq!(next_combo_dir(SWORD, 3, true), 1);
        assert_eq!(next_combo_dir(SWORD, 3, false), 0);
    }

    #[test]
    fn combos_open_for_blades_only() {
        assert!(combo_opens(SWORD, false, 0));
        assert!(combo_opens(SHORT_BLADE, false, 1));
        assert!(!combo_opens(SWORD, true, 0));
        assert!(!combo_opens(SWORD, false, 2));
        assert!(!combo_opens(FISTS, false, 0));
    }

    #[test]
    fn landings_play_something() {
        for d in 0..4u8 {
            for f in 0..2 {
                assert!(landed(SWORD, false, d, false, f).is_some());
            }
        }
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
    fn turns_by_angle() {
        assert_eq!(turn_for(0.3), None);
        assert_eq!(turn_for(0.8).map(|t| t.0), Some(0));
        assert_eq!(turn_for(-0.8).map(|t| t.0), Some(1));
        assert_eq!(turn_for(1.5).map(|t| t.0), Some(2));
        assert_eq!(turn_for(-1.5).map(|t| t.0), Some(3));
        assert_eq!(turn_for(3.0).map(|t| t.0), Some(4));
        assert_eq!(turn_for(-2.5).map(|t| t.0), Some(4));
        for t in 0..5 {
            assert!(turn_action(SWORD, t, false, 0).is_some() && turn_action(SWORD, t, true, 1).is_some());
        }
    }

    #[test]
    fn paired_kills_found() {
        for d in 0..4u8 {
            assert!(combo_kill(SWORD, d).is_some());
            assert!(combo_kill(SHORT_BLADE, d).is_some());
            for f in 0..2 {
                assert!(counter_kill(SWORD, SWORD, d, f).is_some());
            }
        }
        assert!(counter_dodge(SWORD, 1, 0).is_some() && counter_dodge(SWORD, 0, 0).is_none());
        assert!(mobile(SWORD, false, 0).is_some() && mobile(SWORD, true, 1).is_some());
        assert!(high_low(SWORD, false, 0, true, 0).is_some());
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
