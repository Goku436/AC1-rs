//! The ground speed model: AC1 moves on one speed value from 0 to 1, split into bands (walk up to 0.25, jog to
//! 0.5, run to 0.75, sprint above). The wanted value is a base by profile (low 0, high 0.5, free running 0.75)
//! plus a quarter of the stick, so a full stick walks in low profile, runs in high profile and sprints free
//! running, and a half stick goes a band slower. The value rises at 1 per second and falls along a response
//! curve; starting from standing it jumps straight to the walk (low profile) or the jog (high). In high profile,
//! steering more than 45 degrees off the facing holds the speed back while turning.
//!
//! Model and numbers from Banned445's AC1-Movement-Rewritten (MIT, Copyright (c) 2026 Banned445), rewritten
//! here; the speeds in m/s are those of our gait clips' root motion.

/// Upper ends of the walk, jog and run bands.
pub const BAND_WALK: f32 = 0.25;
pub const BAND_JOG: f32 = 0.5;
pub const BAND_RUN: f32 = 0.75;

/// Wanted value with no stick, by profile, and what a full stick adds.
const BASE_LOW: f32 = 0.0;
const BASE_HIGH: f32 = 0.5;
const BASE_SPRINT: f32 = 0.75;
const STICK_SPAN: f32 = 0.25;

/// Rise per second toward a faster value.
const RISE: f32 = 1.0;
/// Fall per second toward a slower value, by the value now (piecewise linear): quick out of a walk, slow through
/// the jog and run, quick again out of a sprint.
const FALL: [(f32, f32); 5] = [(0.0, 1.0), (0.333, 1.0), (0.4, 0.3), (0.666, 0.2), (1.0, 1.0)];

/// High profile turns: past this angle off the facing the wanted speed drops, reaching `TURN_FLOOR` of it
/// `TURN_RANGE` further on; the factor recovers and drops at these rates (per second).
const TURN_START: f32 = std::f32::consts::FRAC_PI_4;
const TURN_RANGE: f32 = std::f32::consts::FRAC_PI_4;
const TURN_FLOOR: f32 = 0.1;
const TURN_UP: f32 = 5.0;
const TURN_DOWN: f32 = 10.0;

/// Speeds (m/s) at the band ends 0, 0.25, 0.5, 0.75 and 1: the slow walk (`xx_l_walk_slow_hipm`, a shuffle: 0.17 m in
/// 1.67 s), walk
/// (`xx_l_walk_hipm`), jog (`xx_h_jog_hipm`), run (`xx_h_run_hipm`) and sprint (`xx_h_sprint_hipm`).
const SPEEDS: [f32; 5] = [0.1, 1.9, 3.5, 5.2, 6.2];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band {
    Stand,
    Walk,
    Jog,
    Run,
    Sprint,
}

pub fn band(v: f32) -> Band {
    match v {
        v if v <= 0.0 => Band::Stand,
        v if v <= BAND_WALK => Band::Walk,
        v if v <= BAND_JOG => Band::Jog,
        v if v <= BAND_RUN => Band::Run,
        _ => Band::Sprint,
    }
}

/// The wanted value for a stick of `stick` (0..1) in a profile; 0 with the stick let go.
pub fn wanted(stick: f32, high: bool, free_run: bool) -> f32 {
    if stick <= 0.0 {
        return 0.0;
    }
    let base = if free_run {
        BASE_SPRINT
    } else if high {
        BASE_HIGH
    } else {
        BASE_LOW
    };
    (base + STICK_SPAN * stick.min(1.0)).min(1.0)
}

/// Ground speed (m/s) for a value.
pub fn speed(v: f32) -> f32 {
    if v <= 0.0 {
        return 0.0;
    }
    let x = v.min(1.0) * 4.0;
    let i = (x.floor() as usize).min(3);
    SPEEDS[i] + (SPEEDS[i + 1] - SPEEDS[i]) * (x - i as f32)
}

/// The value for a ground speed (m/s): `speed`'s inverse (0 at or under the slow walk's).
pub fn value_at(s: f32) -> f32 {
    if s <= SPEEDS[0] {
        return 0.0;
    }
    let i = SPEEDS.windows(2).position(|w| s <= w[1]).unwrap_or(3);
    let x = i as f32 + ((s - SPEEDS[i]) / (SPEEDS[i + 1] - SPEEDS[i])).min(1.0);
    x / 4.0
}

fn fall_rate(v: f32) -> f32 {
    FALL.windows(2).find(|w| v >= w[0].0 && v <= w[1].0).map_or(1.0, |w| w[0].1 + (w[1].1 - w[0].1) * (v - w[0].0) / (w[1].0 - w[0].0))
}

/// The speed value of a character on the ground.
#[derive(Clone, Copy, Debug)]
pub struct Gait {
    pub value: f32,
    /// The high profile turn factor (1: not held back).
    turn: f32,
}

impl Default for Gait {
    fn default() -> Self {
        Self { value: 0.0, turn: 1.0 }
    }
}

impl Gait {
    /// One step: `stick` 0..1, `off` the angle (rad) between where the stick points and the facing. Returns the
    /// ground speed (m/s).
    pub fn update(&mut self, stick: f32, off: f32, high: bool, free_run: bool, dt: f32) -> f32 {
        let turn = if !high || off <= TURN_START { 1.0 } else { (1.0 - (off - TURN_START) / TURN_RANGE).max(TURN_FLOOR) };
        self.turn = if turn > self.turn { (self.turn + TURN_UP * dt).min(turn) } else { (self.turn - TURN_DOWN * dt).max(turn) };
        let want = if stick > 0.0 { wanted(stick * self.turn, high, free_run) } else { 0.0 };
        if want <= 0.0 {
            // Let go: AC1 leaves the move at once (the run stop's clip carries the slide).
            self.value = 0.0;
        } else if self.value <= 0.0 {
            // Starting from standing: straight into the walk or the jog.
            self.value = want.min(if high || free_run { BAND_JOG } else { BAND_WALK });
        } else if want > self.value {
            self.value = (self.value + RISE * dt).min(want);
        } else {
            self.value = (self.value - fall_rate(self.value) * dt).max(want);
        }
        speed(self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_stick_walks_runs_or_sprints_by_profile() {
        assert_eq!(band(wanted(1.0, false, false)), Band::Walk);
        assert_eq!(band(wanted(1.0, true, false)), Band::Run);
        assert_eq!(band(wanted(1.0, true, true)), Band::Sprint);
        assert_eq!(band(wanted(0.5, true, false)), Band::Run);
        assert!((speed(1.0) - 6.2).abs() < 1e-5 && (speed(0.75) - 5.2).abs() < 1e-5 && (speed(0.25) - 1.9).abs() < 1e-5);
    }

    #[test]
    fn value_at_inverts_speed() {
        for v in [0.1, 0.25, 0.4, 0.5, 0.62, 0.75, 0.9, 1.0] {
            assert!((value_at(speed(v)) - v).abs() < 1e-4, "{v}");
        }
        assert_eq!(value_at(0.0), 0.0);
        assert_eq!(value_at(9.0), 1.0);
    }

    #[test]
    fn the_speed_starts_at_the_jog_and_rises_to_the_sprint() {
        let mut g = Gait::default();
        g.update(1.0, 0.0, true, true, 0.01);
        assert!((g.value - BAND_JOG).abs() < 1e-5, "{g:?}");
        for _ in 0..30 {
            g.update(1.0, 0.0, true, true, 0.01);
        }
        assert!((g.value - 0.8).abs() < 1e-3, "rises at 1/s: {g:?}");
        for _ in 0..100 {
            g.update(1.0, 0.0, true, true, 0.01);
        }
        assert!((g.value - 1.0).abs() < 1e-5);
        // Out of free running: down to the run, slowly through the run band.
        g.update(1.0, 0.0, true, false, 0.05);
        assert!(g.value > 0.9 && g.value < 1.0, "{g:?}");
        // Let go: stops.
        assert_eq!(g.update(0.0, 0.0, true, false, 0.01), 0.0);
    }

    #[test]
    fn turning_hard_in_high_profile_holds_the_speed_back() {
        let mut g = Gait::default();
        for _ in 0..100 {
            g.update(1.0, 0.0, true, false, 0.01);
        }
        for _ in 0..30 {
            g.update(1.0, 2.0, true, false, 0.01);
        }
        assert!(g.value < 0.7, "{g:?}");
        // Low profile: no hold back.
        let mut g = Gait::default();
        for _ in 0..30 {
            g.update(1.0, 2.0, false, false, 0.01);
        }
        assert!((g.value - BAND_WALK).abs() < 1e-5);
    }
}
