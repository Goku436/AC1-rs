//! AC1's ground locomotion blend (`HumanGround__UpdateMoveBlend` 0xDA0810): one action (0x05923BDB) whose two items
//! (left foot, right foot) each blend 17 clips; every frame the 17 weights come from the speed value (0..1, four
//! bands), the hip lean (walk band; AC1's crowd avoidance, none here), the bank (jog and up: the angle to the heading
//! wanted), the jog's slow-down timer and the sprint's take-off weight. The clips share one clock: a step lasts
//! Σwᵢ·Tᵢ and the root covers Σwᵢ·dᵢ in it, so the speed is Σwᵢ·dᵢ / Σwᵢ·Tᵢ (slower between bands than a straight
//! mix of the band speeds).
//!
//! Model, slot order and the measured steps from Banned445's AC1-Movement-Rewritten (MIT, Copyright (c) 2026
//! Banned445), rewritten here.

/// The 17 slots: per gait its straight clip then its left and right ones (the walks' hip leans, the faster gaits'
/// banks; the sprint banks with the run's), the jog's slow-down and the sprint's take-off. `{foot}`: footl, footr.
pub const SLOTS: [&str; 17] = [
    "xx_l_walk_slow_hipm_{foot}",
    "xx_l_walk_slow_hipl_{foot}",
    "xx_l_walk_slow_hipr_{foot}",
    "xx_l_walk_hipm_{foot}",
    "xx_l_walk_hipl_{foot}",
    "xx_l_walk_hipr_{foot}",
    "xx_h_jog_hipm_{foot}",
    "xx_h_jog_bank_left_{foot}",
    "xx_h_jog_bank_right_{foot}",
    "xx_h_jog_slowdown_{foot}",
    "xx_h_run_hipm_{foot}",
    "xx_h_run_bank_left_{foot}",
    "xx_h_run_bank_right_{foot}",
    "xx_h_sprint_hipm_{foot}",
    "xx_h_run_bank_left_{foot}",
    "xx_h_run_bank_right_{foot}",
    "xx_h_sprint_impultion_{foot}",
];
const SLOW_WALK: usize = 0;
const WALK: usize = 3;
const JOG: usize = 6;
const JOG_SLOWDOWN: usize = 9;
const RUN: usize = 10;
const SPRINT: usize = 13;
const SPRINT_TAKEOFF: usize = 16;

/// Each slot's step for the left foot's item: (root distance m, duration s), measured from the game's clips.
const STEP: [(f32, f32); 17] = [
    (0.170, 1.6667),
    (0.269, 2.0),
    (0.269, 2.0),
    (1.012, 0.5333),
    (0.992, 0.6),
    (0.992, 0.6),
    (1.651, 0.4667),
    (1.707, 0.4667),
    (1.707, 0.4667),
    (1.651, 0.4667),
    (1.707, 0.3333),
    (1.707, 0.3333),
    (1.707, 0.3333),
    (1.674, 0.2667),
    (1.707, 0.3333),
    (1.707, 0.3333),
    (1.674, 0.2667),
];

/// What the weights depend on besides the speed value.
#[derive(Clone, Copy, Debug, Default)]
pub struct Shape {
    /// Hip lean (walk band), -1..1, positive to the left.
    pub lean: f32,
    /// Bank (jog and up), -1..1, positive to the left.
    pub bank: f32,
    /// The jog's slow-down timer, 0..1.
    pub slowdown: f32,
    /// The sprint take-off's weight, 0..1 (1 below the sprint band).
    pub settle: f32,
}

/// The 17 weights for speed value `s` (0xDA18C0..0xDA1FC3). They sum to 1, except between the walk and the jog where
/// AC1 counts the side clips' share twice (kept: the clock and pose use them as they are).
pub fn weights(s: f32, shape: Shape) -> [f32; 17] {
    let s = s.clamp(0.0, 1.0);
    let (lower, upper, f) = if s <= 0.25 {
        (SLOW_WALK, WALK, s * 4.0)
    } else if s <= 0.5 {
        (WALK, JOG, (s - 0.25) * 4.0)
    } else if s <= 0.75 {
        (JOG, RUN, (s - 0.5) * 4.0)
    } else {
        (RUN, SPRINT, (s - 0.75) * 4.0)
    };
    let f = f.min(1.0);
    let lean = shape.lean.clamp(-1.0, 1.0);
    let bank = shape.bank.clamp(-1.0, 1.0);
    let side = |a: f32| if a > 0.0 { 1 } else { 2 };
    let (lower_side, upper_side, lower_k, upper_k) = if s <= 0.25 {
        (lean.abs(), lean.abs(), side(lean), side(lean))
    } else if s > 0.5 {
        (bank.abs(), bank.abs(), side(bank), side(bank))
    } else {
        // The walk keeps the hip lean, the jog takes the bank.
        ((1.0 - f) * lean.abs(), f * bank.abs(), side(lean), side(bank))
    };
    let total_side = if s > 0.25 && s <= 0.5 { lower_side + upper_side } else { lower_side };
    let straight = 1.0 - total_side;
    let mut w = [0.0f32; 17];
    let (lower_mid, upper_mid);
    if s <= 0.5 {
        let slowdown = if s <= 0.25 { 0.0 } else { shape.slowdown };
        w[JOG_SLOWDOWN] = slowdown * f * straight;
        lower_mid = (1.0 - f) * straight;
        upper_mid = (1.0 - slowdown) * f * straight;
    } else if s > 0.75 {
        w[SPRINT_TAKEOFF] = shape.settle * f * straight;
        lower_mid = (1.0 - f) * straight;
        upper_mid = (1.0 - shape.settle) * f * straight;
    } else {
        w[JOG_SLOWDOWN] = shape.slowdown * (1.0 - f) * straight;
        lower_mid = (1.0 - shape.slowdown) * (1.0 - f) * straight;
        upper_mid = f * straight;
    }
    w[lower] += lower_mid;
    w[upper] += upper_mid;
    w[lower + lower_k] += lower_side * (1.0 - f);
    w[upper + upper_k] += upper_side * f;
    w
}

/// A step's length (s) and the root's speed (m/s) under these weights: Σwᵢ·Tᵢ and Σwᵢ·dᵢ / Σwᵢ·Tᵢ.
pub fn step(w: &[f32; 17]) -> (f32, f32) {
    let (d, t) = w.iter().zip(STEP).fold((0.0, 0.0), |(d, t), (w, s)| (d + w * s.0, t + w * s.1));
    if t <= 1e-4 { (0.0, 0.0) } else { (t, d / t) }
}

/// The steady straight-line speed (m/s) at speed value `s`: no lean or bank, the timers settled (no slow-down, the
/// sprint's take-off done).
pub fn steady_speed(s: f32) -> f32 {
    if s <= 0.0 {
        return 0.0;
    }
    let settle = if s > 0.75 { 0.0 } else { 1.0 };
    step(&weights(s, Shape { settle, ..Shape::default() })).1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn band_tops_play_one_clip_at_its_speed() {
        for (s, slot, v) in [(0.25, WALK, 1.898), (0.5, JOG, 3.538), (0.75, RUN, 5.121)] {
            let w = weights(s, Shape { settle: 1.0, ..Shape::default() });
            assert!((w[slot] - 1.0).abs() < 1e-6, "{s}: {w:?}");
            assert!((steady_speed(s) - v).abs() < 0.01, "{s}: {}", steady_speed(s));
        }
        assert!((steady_speed(1.0) - 6.277).abs() < 0.01);
    }

    #[test]
    fn between_bands_slower_than_a_straight_mix() {
        let mid = steady_speed(0.625);
        assert!(mid > 3.9 && mid < (3.538 + 5.121) / 2.0, "{mid}");
        // and it rises all the way
        let mut last = 0.0;
        for i in 1..=100 {
            let v = steady_speed(i as f32 / 100.0);
            assert!(v > last, "{i}");
            last = v;
        }
    }

    #[test]
    fn weights_sum_to_one_outside_walk_to_jog() {
        let shape = Shape { lean: 0.3, bank: -0.45, slowdown: 0.3, settle: 0.6 };
        for i in 0..=100 {
            let s = i as f32 / 100.0;
            let sum: f32 = weights(s, shape).iter().sum();
            if !(s > 0.25 && s <= 0.5) {
                assert!((sum - 1.0).abs() < 1e-5, "{s}: {sum}");
            }
        }
    }

    #[test]
    fn walks_lean_and_faster_gaits_bank_to_the_side_wanted() {
        let w = weights(0.2, Shape { lean: 0.5, ..Shape::default() });
        assert!(w[WALK + 1] > 0.0 && w[WALK + 2] == 0.0);
        let w = weights(0.6, Shape { bank: -0.5, ..Shape::default() });
        assert!(w[JOG + 2] > 0.0 && w[RUN + 2] > 0.0 && w[JOG + 1] == 0.0);
        // no lean, no bank: the walk band plays straight clips only
        let w = weights(0.2, Shape { bank: 0.8, ..Shape::default() });
        assert_eq!(w[WALK + 1] + w[WALK + 2] + w[SLOW_WALK + 1] + w[SLOW_WALK + 2], 0.0);
    }
}
