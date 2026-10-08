//! AC1's climbing move tables (docs/PARKOUR.md section 4): the six climbing poses, the stick split into ten
//! directions (`QuantizeStickDirection` 0xDEB8D0) and, per pose and direction, the move in the short table (0x1A2D070)
//! or the long one (0x1A2D7F0, hand over hand and both sides shuffling, tried first with the stick past half,
//! `ChooseMove` 0xDFDE90). The entries and their action ids are from Banned445's AC1-Movement-Rewritten (MIT; read
//! there by emulating `HumanClimb__StaticInitTables`); every action id names a clip in the game's action blocks
//! (`forge` example `action_clips`), which is how the moves play here.

/// The poses, by our climbing state names: hands level ("1m"), the left or right one a row higher, and the same a
/// column apart ("2m" ...).
pub const POSES: [&str; 6] = ["1m", "1lu", "1ru", "2m", "2lu", "2ru"];

/// Which side a move moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    L,
    R,
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    None,
    /// Look the move up again under another direction (the game's codes 9..14).
    Redirect(usize),
    /// The pose it ends in, the side that moves, and how far in grid cells (columns, rows).
    To(usize, Side, i32, i32),
}

use Entry::{None as N, Redirect as Rd, To};
use Side::{Both, L, R};

/// Directions: 0 up (left of straight), 1 up (right of it), 2 down (left), 3 down (right), 4 left, 5 right, 6 up-left,
/// 7 up-right, 8 down-left, 9 down-right.
pub const SHORT: [[Entry; 10]; 6] = [
    [
        To(1, L, 0, 1),
        To(2, R, 0, 1),
        To(2, L, 0, -1),
        To(1, R, 0, -1),
        To(3, L, -1, 0),
        To(3, R, 1, 0),
        To(4, L, -1, 1),
        To(5, R, 1, 1),
        To(5, L, -1, -1),
        To(4, R, 1, -1),
    ],
    [To(0, R, 0, 1), To(0, R, 0, 1), To(0, L, 0, -1), To(0, L, 0, -1), To(4, L, -1, 0), To(4, R, 1, 0), Rd(4), To(3, R, 1, 1), To(3, L, -1, -1), Rd(5)],
    [To(0, L, 0, 1), To(0, L, 0, 1), To(0, R, 0, -1), To(0, R, 0, -1), To(5, L, -1, 0), To(5, R, 1, 0), To(3, L, -1, 1), Rd(5), Rd(4), To(3, R, 1, -1)],
    [To(4, L, 0, 1), To(5, R, 0, 1), To(5, L, 0, -1), To(4, R, 0, -1), To(0, R, -1, 0), To(0, L, 1, 0), Rd(0), Rd(1), Rd(2), Rd(3)],
    [To(3, R, 0, 1), To(3, R, 0, 1), To(3, L, 0, -1), To(3, L, 0, -1), To(1, R, -1, 0), To(1, L, 1, 0), To(0, R, -1, 1), Rd(0), Rd(2), To(0, L, 1, -1)],
    [To(3, L, 0, 1), To(3, L, 0, 1), To(3, R, 0, -1), To(3, R, 0, -1), To(2, R, -1, 0), To(2, L, 1, 0), Rd(0), To(0, L, 1, 1), To(0, R, -1, -1), Rd(2)],
];

pub const LONG: [[Entry; 10]; 6] = [
    [N; 10],
    [To(2, R, 0, 2), To(2, R, 0, 2), To(2, L, 0, -2), To(2, L, 0, -2), N, N, N, N, N, N],
    [To(1, L, 0, 2), To(1, L, 0, 2), To(1, R, 0, -2), To(1, R, 0, -2), N, N, N, N, N, N],
    [N, N, N, N, To(3, Both, -1, 0), To(3, Both, 1, 0), N, N, N, N],
    [To(5, R, 0, 2), To(5, R, 0, 2), To(5, L, 0, -2), To(5, L, 0, -2), To(4, Both, -1, 0), To(4, Both, 1, 0), N, N, N, N],
    [To(4, L, 0, 2), To(4, L, 0, 2), To(4, R, 0, -2), To(4, R, 0, -2), To(5, Both, -1, 0), To(5, Both, 1, 0), N, N, N, N],
];

/// Each entry's action (0: none). Redirected entries play the action of the one they lead to.
pub const SHORT_ACTIONS: [[u32; 10]; 6] = [
    [0x019A05F1, 0x019A05F2, 0x019A05F4, 0x019A05F3, 0x019A05F5, 0x019A05F6, 0x019A05F7, 0x019A05F8, 0x019A05F9, 0x019A05FA],
    [0x019A36CF, 0x019A36CF, 0x019A36D0, 0x019A36D0, 0x019A36D1, 0x019A36D2, 0, 0x019A36D3, 0x019A36D4, 0],
    [0x019A36E7, 0x019A36E7, 0x019A36E8, 0x019A36E8, 0x019A36E9, 0x019A36EA, 0x019A36EB, 0, 0, 0x019A36EC],
    [0x01A267EC, 0x01A267ED, 0x01A267EF, 0x01A267EE, 0x01A267F0, 0x01A267F2, 0, 0, 0, 0],
    [0x01A26815, 0x01A26815, 0x01A26817, 0x01A26817, 0x01A26819, 0x01A2681B, 0x01A2681D, 0, 0, 0x01A2681E],
    [0x01A26833, 0x01A26833, 0x01A26835, 0x01A26835, 0x01A26837, 0x01A26839, 0, 0x01A2683B, 0x01A2683C, 0],
];

pub const LONG_ACTIONS: [[u32; 10]; 6] = [
    [0; 10],
    [0x019A36D5, 0x019A36D5, 0x019A36D6, 0x019A36D6, 0, 0, 0, 0, 0, 0],
    [0x019A36ED, 0x019A36ED, 0x019A36EE, 0x019A36EE, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0x01A267F1, 0x01A267F3, 0, 0, 0, 0],
    [0x01A26816, 0x01A26816, 0x01A26818, 0x01A26818, 0x01A2681A, 0x01A2681C, 0, 0, 0, 0],
    [0x01A26834, 0x01A26834, 0x01A26836, 0x01A26836, 0x01A26838, 0x01A2683A, 0, 0, 0, 0],
];

/// The stick past this (0..1) tries the long table first.
pub const LONG_STICK: f32 = 0.5;

/// The stick (x right, y up the wall) as one of the ten directions.
pub fn quantize(input: bevy::math::Vec2) -> usize {
    let a = input.x.atan2(input.y).to_degrees();
    match a {
        a if (-22.5..0.0).contains(&a) => 0,
        a if (0.0..22.5).contains(&a) => 1,
        a if (-67.5..-22.5).contains(&a) => 6,
        a if (-112.5..-67.5).contains(&a) => 4,
        a if (-157.5..-112.5).contains(&a) => 8,
        a if (22.5..67.5).contains(&a) => 7,
        a if (67.5..112.5).contains(&a) => 5,
        a if (112.5..157.5).contains(&a) => 9,
        a if a < 0.0 => 2,
        _ => 3,
    }
}

/// A move found in a table: the pose it ends in, the side, the cells, and its action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub to: usize,
    pub side: Side,
    pub cols: i32,
    pub rows: i32,
    pub action: u32,
}

/// The move from `pose` toward `dir` in one table, following redirects.
pub fn lookup(table: &[[Entry; 10]; 6], actions: &[[u32; 10]; 6], pose: usize, dir: usize) -> Option<Move> {
    let mut d = dir;
    for _ in 0..3 {
        match table[pose][d] {
            Entry::None => return None,
            Entry::Redirect(nd) => d = nd,
            Entry::To(to, side, cols, rows) => {
                return (actions[pose][d] != 0).then_some(Move { to, side, cols, rows, action: actions[pose][d] });
            }
        }
    }
    None
}

/// The moves to try from `pose` for the stick, in AC1's order: the long one first with the stick past half, then the
/// short one.
pub fn moves(pose: usize, input: bevy::math::Vec2) -> Vec<Move> {
    let dir = quantize(input);
    let mut out = vec![];
    if input.length() > LONG_STICK {
        out.extend(lookup(&LONG, &LONG_ACTIONS, pose, dir));
    }
    out.extend(lookup(&SHORT, &SHORT_ACTIONS, pose, dir));
    out
}

/// The pose index of a climbing state name.
pub fn pose_of(state: &str) -> Option<usize> {
    POSES.iter().position(|p| *p == state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Vec2;

    #[test]
    fn quantize_splits_up_and_down_by_side() {
        assert_eq!(quantize(Vec2::new(-0.1, 1.0)), 0);
        assert_eq!(quantize(Vec2::new(0.0, 1.0)), 1);
        assert_eq!(quantize(Vec2::new(-1.0, 0.0)), 4);
        assert_eq!(quantize(Vec2::new(1.0, 1.0)), 7);
        assert_eq!(quantize(Vec2::new(0.1, -1.0)), 3);
    }

    #[test]
    fn hand_over_hand_from_a_raised_hand() {
        // Left hand up, pushed up hard: the right hand passes it (long), else comes level (short).
        let m = moves(1, Vec2::new(0.0, 1.0));
        assert_eq!(m.iter().map(|m| POSES[m.to]).collect::<Vec<_>>(), ["1ru", "1m"]);
        assert_eq!(moves(1, Vec2::new(0.0, 0.4)).len(), 1);
    }

    #[test]
    fn redirects_play_the_entry_led_to() {
        // From "2m" up-left is up (left).
        let m = lookup(&SHORT, &SHORT_ACTIONS, 3, 6).unwrap();
        assert_eq!((POSES[m.to], m.action), ("2lu", 0x01A267EC));
    }

    #[test]
    fn every_move_has_an_action() {
        for pose in 0..6 {
            for dir in 0..10 {
                for (t, a) in [(&SHORT, &SHORT_ACTIONS), (&LONG, &LONG_ACTIONS)] {
                    if !matches!(t[pose][dir], Entry::None) {
                        assert!(lookup(t, a, pose, dir).is_some(), "pose {pose} dir {dir}");
                    }
                }
            }
        }
    }
}
