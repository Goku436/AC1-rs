//! AC1's jump clips (`HumanInAir`): a jump plays a takeoff and a flight, each a blend of clips by how far and how high
//! the target is, then a reception on arriving. The root follows the blend's own motion, with the difference to the
//! target spread over it.
//!
//! - Takeoff from a run (actions 0x0a4c8c0e / 0x0a4c8c0f by foot): `xx_h_run_<front|down|up>_<dist>_foot?_to_air`,
//!   front 050/300/550, down 050/300/550, up 050/300 (cm).
//! - Flight onto a free-step target (a top to land on running, 0x010ddafa / 0x010df0d8): `xx_h_air_front_<dist>_
//!   foot?_to_freestep`, `xx_h_air_down_<dist>_foot?_to_freestep_down` (to 800 cm), `xx_h_air_up_<dist>_foot?_to_
//!   freestep`, going down `xx_h_air_front_<dist>_foot?_to_freestep_down`, and more than 3 m down
//!   `xx_h_air_down_<dist>_foot?_to_freestep_deep`.
//! - Reception (0x010de1fe / 0x010df150): the flight's name + `_tr_freestep_entry_foot<other>`.
//!
//! How the clips are weighted (bands of height and distance; the takeoff mirroring the flight's blend) is as
//! Banned445's AC1-Movement-Rewritten reads it from the game (MIT, Copyright (c) 2026 Banned445), written here anew.

/// Height and distance bands of a jump's target (m): the highest up and lowest down a jump blends to, and the near,
/// middle and far distances, by AC1's target type (0xB1EC40, read 2026-10-23; Banned445's `bands` agrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    pub up: f32,
    pub down: f32,
    pub near: f32,
    pub mid: f32,
    pub far: f32,
}

/// A free step, a pass-over, the ground's targets (types 1, 2, 0x100, 0x200, 0x400, 0x8000, 0x10000).
pub const FREE_STEP: Band = Band { up: 1.3, down: -3.0, near: 2.5, mid: 5.0, far: 7.0 };
/// A surface: a wall hang, a ladder, the climbing holds (types 0x40, 0x1000, 0x2000, 0x4000).
pub const SURFACE: Band = Band { up: 2.5, down: -3.0, near: 2.5, mid: 5.5, far: 7.5 };
/// The rest: a free hang, a swing bar (0x80 and others).
pub const HANG: Band = Band { up: 3.0, down: -3.0, near: 2.5, mid: 6.0, far: 8.0 };
/// The jump off a swing bar (AC1's takeoff kind 3) onto a free step: the band lifted 0.7 m, its top and its bottom.
pub const OFF_SWING: Band = Band { up: 1.3 + 0.7, down: -3.0 + 0.7, ..FREE_STEP };
/// Where a jump starts to count as going down (m).
const DOWN_FROM: f32 = -0.5;
/// More than this far down (m), the flight blends into its deep variants, fully at `DEEP_FULL` further.
const DEEP_FROM: f32 = 3.0;
const DEEP_FULL: f32 = 5.0;

const FRONT: [&str; 3] = ["050cm", "300cm", "550cm"];
const DOWN: [&str; 4] = ["050cm", "300cm", "550cm", "800cm"];
const UP: [&str; 2] = ["050cm", "300cm"];

/// Which part of the band a jump falls in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    /// Closer than the band's near distance.
    Near,
    /// Further, level or up.
    Up,
    /// Further, going down, short of its middle one.
    Down,
    /// Further, going down, past its middle one.
    Far,
}

/// One blended jump: (clip name, weight) for the takeoff, the flight and the reception.
#[derive(Clone, Debug, Default)]
pub struct Jump {
    pub takeoff: Vec<(String, f32)>,
    pub flight: Vec<(String, f32)>,
    pub reception: Vec<(String, f32)>,
}

/// The flight's 16 slots: front 050/300/550, down 050/300/550/800 (to free step down), up 050/300, front going down
/// 050/300/550, down deep 050/300/550/800.
fn flight_names(foot: &str) -> Vec<String> {
    let mut n: Vec<String> = FRONT.iter().map(|d| format!("xx_h_air_front_{d}_{foot}_to_freestep")).collect();
    n.extend(DOWN.iter().map(|d| format!("xx_h_air_down_{d}_{foot}_to_freestep_down")));
    n.extend(UP.iter().map(|d| format!("xx_h_air_up_{d}_{foot}_to_freestep")));
    n.extend(FRONT.iter().map(|d| format!("xx_h_air_front_{d}_{foot}_to_freestep_down")));
    n.extend(DOWN.iter().map(|d| format!("xx_h_air_down_{d}_{foot}_to_freestep_deep")));
    n
}

/// The takeoff's 8 slots: front 050/300/550, down 050/300/550, up 050/300.
fn takeoff_names(foot: &str) -> Vec<String> {
    let mut n: Vec<String> = FRONT.iter().map(|d| format!("xx_h_run_front_{d}_{foot}_to_air")).collect();
    n.extend(FRONT.iter().map(|d| format!("xx_h_run_down_{d}_{foot}_to_air")));
    n.extend(UP.iter().map(|d| format!("xx_h_run_up_{d}_{foot}_to_air")));
    n
}

/// The free-step takeoff's 8 slots in one direction group (`front`, `left`, `right`, `backleft`, `backright`): front
/// 050/300/550, down 050/300/550, up 050/300. The side groups' far ones are 500 cm from the left foot, and AC1 names the
/// right foot's far one to the right without `_to_air`.
fn freestep_names(group: &str, foot: &str) -> Vec<String> {
    let side = group == "left" || group == "right";
    let far = if side && foot == "footl" { "500cm" } else { "550cm" };
    let dists = ["050cm", "300cm", far];
    let mut n: Vec<String> = dists.iter().map(|d| format!("xx_h_freestep_{group}_front_{d}_{foot}_to_air")).collect();
    if group == "right" && foot == "footr" {
        n[2] = "xx_h_freestep_right_front_550cm_footr".into();
    }
    n.extend(dists.iter().map(|d| format!("xx_h_freestep_{group}_down_{d}_{foot}_to_air")));
    n.extend(UP.iter().map(|d| format!("xx_h_freestep_{group}_up_{d}_{foot}_to_air")));
    n
}

/// A jump from a free step (a post, a beam, a roof's edge) onto a top `dz` above and `dist` away, `angle` (rad, positive
/// to the right) off the facing: the running jump's flight and reception, the takeoff from AC1's free-step groups,
/// the two around the angle blended (front, right, back right; front, left, back left). The side and back takeoffs turn
/// the body toward the jump.
pub fn freestep(dz: f32, dist: f32, angle: f32, left: bool, fast: f32) -> Jump {
    freestep_in(FREE_STEP, dz, dist, angle, left, fast)
}

/// `freestep` on another target type's band.
pub fn freestep_in(b: Band, dz: f32, dist: f32, angle: f32, left: bool, fast: f32) -> Jump {
    let mut j = running_in(b, dz, dist, left, fast);
    let foot = if left { "footl" } else { "footr" };
    // The running takeoff's weights, by slot, re-used in each group.
    let names = takeoff_names(foot);
    let slot_w: Vec<f32> = names.iter().map(|n| j.takeoff.iter().find(|(m, _)| m == n).map_or(0.0, |p| p.1)).collect();
    let a = angle.clamp(-std::f32::consts::PI, std::f32::consts::PI);
    let q = a.abs() / std::f32::consts::FRAC_PI_2;
    let (side, back) = if a >= 0.0 { ("right", "backright") } else { ("left", "backleft") };
    let groups: [(&str, f32); 2] = if q <= 1.0 { [("front", 1.0 - q), (side, q)] } else { [(side, 2.0 - q), (back, q - 1.0)] };
    j.takeoff = groups
        .iter()
        .filter(|(_, g)| *g > 0.01)
        .flat_map(|(group, g)| freestep_names(group, foot).into_iter().zip(slot_w.iter().map(move |w| w * g)))
        .filter(|(_, w)| *w > 0.01)
        .collect();
    j
}

/// An eject off a wall (AC1's rebound): from a hang or a climb, turned away from the wall, onto a top `dz` above and
/// `dist` away, `angle` (rad, positive to the right of the body turned away) off straight away: the running jump's flight
/// and reception, the takeoff from AC1's rebound groups (`xx_h_rebound_<frontleft|frontright|left|right>_<front|down|up>
/// _<cm>_<foot>_to_air`, the running takeoff's slots), straight away and to that side blended by the angle (the side
/// groups turn the body a quarter round, toward the jump). Measured in the running game (Damascus, the bureau's wall):
/// the stick back (or none) jumps away, the stick to a side jumps that way along the wall, each at the best place to land.
pub fn rebound(dz: f32, dist: f32, angle: f32, left: bool, fast: f32) -> Jump {
    rebound_in(FREE_STEP, dz, dist, angle, left, fast)
}

/// `rebound` on another target type's band.
pub fn rebound_in(b: Band, dz: f32, dist: f32, angle: f32, left: bool, fast: f32) -> Jump {
    let mut j = running_in(b, dz, dist, left, fast);
    let foot = if left { "footl" } else { "footr" };
    let names = takeoff_names(foot);
    let slot_w: Vec<f32> = names.iter().map(|n| j.takeoff.iter().find(|(m, _)| m == n).map_or(0.0, |p| p.1)).collect();
    let q = (angle.abs() / std::f32::consts::FRAC_PI_2).min(1.0);
    let (front, side) = if angle >= 0.0 { ("frontright", "right") } else { ("frontleft", "left") };
    j.takeoff = [(front, 1.0 - q), (side, q)]
        .iter()
        .filter(|(_, g)| *g > 0.01)
        .flat_map(|(group, g)| names.iter().map(move |n| n.replace("xx_h_run_", &format!("xx_h_rebound_{group}_"))).zip(slot_w.iter().map(move |w| w * g)))
        .filter(|(_, w)| *w > 0.01)
        .collect();
    j
}

/// A running jump at a wall's hold to catch (a lone ledge, the feet against the wall under it): the running jump's takeoff
/// for the hang's place `dz` above and `dist` away, its flight onto a surface (`xx_h_air_<front|down|up>_<dist>_foot?_to_
/// surface`: the flight's slots folded onto those: the down landings, deep or not, onto `down`, the front going down onto
/// `front`), then AC1's reception on the wall (`xx_h_air_surface_tr_hangwall_reception_front_straight_<min|max>` and its
/// `xx_hangwall_reception_..._a/b/c`), the hard one (`max`) when `fast`. (Action 0x011FF16A is the first three; the `_c`
/// is the connector on its way out to the wait, `reception_connector`, which a move cuts.) `tilt` (radians) is the wall's lean under the
/// edge, positive overhanging: AC1 mixes in the `30_out` receptions toward an overhang (full at 30 degrees) and the
/// `45_in` ones toward a slope (full at 45) (0xE02790).
pub fn surface(dz: f32, dist: f32, left: bool, fast: bool, tilt: f32) -> Jump {
    let j = running_in(SURFACE, dz, dist, left, 0.0);
    let mut flight: Vec<(String, f32)> = vec![];
    for (n, w) in &j.flight {
        let n = n.replace("_to_freestep_down", "_to_surface").replace("_to_freestep_deep", "_to_surface").replace("_to_freestep", "_to_surface");
        // (No deep 800 cm onto a surface beyond the down one's; no front 800.)
        match flight.iter_mut().find(|(m, _)| *m == n) {
            Some(p) => p.1 += w,
            None => flight.push((n, *w)),
        }
    }
    let kind = if fast { "max" } else { "min" };
    let (side, w) = if tilt >= 0.0 { ("30_out", (tilt / 30f32.to_radians()).min(1.0)) } else { ("45_in", (-tilt / 45f32.to_radians()).min(1.0)) };
    // One mixed clip per step of the chain (`animation::mix_name`).
    let step = |stem: &str| -> (String, f32) {
        let (straight, angled) = (stem.replace("{a}", "straight"), stem.replace("{a}", side));
        (crate::animation::mix_name(&[(straight.as_str(), 1.0 - w), (angled.as_str(), w)]), 1.0)
    };
    let reception = std::iter::once(step(&format!("xx_h_air_surface_tr_hangwall_reception_front_{{a}}_{kind}")))
        .chain(["a", "b", "c"].map(|k| step(&format!("xx_hangwall_reception_front_{{a}}_{kind}_{k}"))))
        .collect();
    Jump { takeoff: j.takeoff, flight, reception }
}

/// A running jump over a thin wall, the hand on its top (AC1's passover), the root at the touch `dz` above and `dist`
/// away: the running jump's takeoff and its flight onto the edge (`xx_h_air_<front|up>_<dist>_foot?_to_passover`: front
/// 050/300/550 and up 300; the flight's down slots fold onto the front ones of their distance, the short up one onto
/// the short front one, whose AC1 reception steps onto the top rather than going over it).
pub fn passover(dz: f32, dist: f32, left: bool) -> Jump {
    let j = running(dz, dist, left, 0.0);
    let mut flight: Vec<(String, f32)> = vec![];
    for (n, w) in &j.flight {
        let n = n
            .replace("_to_freestep_down", "_to_passover")
            .replace("_to_freestep_deep", "_to_passover")
            .replace("_to_freestep", "_to_passover")
            .replace("_down_", "_front_")
            .replace("front_800cm", "front_550cm")
            .replace("up_050cm", "front_050cm");
        match flight.iter_mut().find(|(m, _)| *m == n) {
            Some(p) => p.1 += w,
            None => flight.push((n, *w)),
        }
    }
    Jump { takeoff: j.takeoff, flight, reception: vec![] }
}

/// A running jump at a swing bar, the hang under it `dz` above and `dist` away: the running jump's takeoff and its flight
/// onto the bar (`xx_h_air_<front|down|up>_<dist>_foot?_to_swing`; AC1's far ones are front 650 and down 900 cm, the
/// flight's 550 and 800 cm slots fold onto them).
pub fn swing(dz: f32, dist: f32, left: bool) -> Jump {
    let j = running_in(HANG, dz, dist, left, 0.0);
    let mut flight: Vec<(String, f32)> = vec![];
    for (n, w) in &j.flight {
        let n = n
            .replace("_to_freestep_down", "_to_swing")
            .replace("_to_freestep_deep", "_to_swing")
            .replace("_to_freestep", "_to_swing")
            .replace("front_550cm", "front_650cm")
            .replace("down_800cm", "down_900cm");
        match flight.iter_mut().find(|(m, _)| *m == n) {
            Some(p) => p.1 += w,
            None => flight.push((n, *w)),
        }
    }
    Jump { takeoff: j.takeoff, flight, reception: vec![] }
}

/// The jump off a swing bar at a target `dz` up and `dist` away: AC1's jump kind 3, its takeoff the one eight-clip
/// group `xx_h_swing_cycle_<front 050|300|550, down 050|300|550, up 100|300>cm_to_air` (action 0x0292655B) on the run's
/// takeoff slots and weights (Banned445's reading of 0xB1EC40, audit NARROW-14).
pub fn off_swing(dz: f32, dist: f32) -> Vec<(String, f32)> {
    const NAMES: [&str; 8] = ["front_050cm", "front_300cm", "front_550cm", "down_050cm", "down_300cm", "down_550cm", "up_100cm", "up_300cm"];
    let run = running_in(OFF_SWING, dz, dist, true, 0.0);
    let slots = takeoff_names("footl");
    run.takeoff.iter().filter_map(|(n, w)| slots.iter().position(|s| s == n).map(|i| (format!("xx_h_swing_cycle_{}_to_air", NAMES[i]), *w))).collect()
}

/// A free step's jump at a swing bar (or a free hang: AC1's target type 0x80) `dz` up and `dist` away: the free-step
/// takeoff, `swing`'s flight (`_to_swing`).
pub fn freestep_swing(dz: f32, dist: f32, angle: f32, left: bool) -> Jump {
    let mut j = freestep_in(HANG, dz, dist, angle, left, 0.0);
    j.flight = swing(dz, dist, left).flight;
    j.reception = vec![];
    j
}

/// AC1's catch of a swing bar from the air, into the swing (`xx_h_air_<way>_to_swing_tr_swing_front_a`, then
/// `xx_h_swing_cycle_front_up`), by how the flight comes at it: `dz` the hang's height over where the flight started.
pub fn swing_entry(dz: f32) -> &'static str {
    if dz > 0.5 {
        "xx_h_air_up_050cm_to_swing_tr_swing_front_a"
    } else if dz < -1.0 {
        "xx_h_air_down_300cm_to_swing_tr_swing_front_a"
    } else {
        "xx_h_air_front_300cm_to_swing_tr_swing_front_a"
    }
}

/// The ground's impulsion into a free-step takeoff (AC1's ground actions 0x04A943DE / 0x040C793D, by the takeoff foot:
/// `xx_h_impultionstraight_<foot>_to_ground_to_air_<group>_<up|down>_<other>`): played before the free-step takeoff by a
/// jump from a standstill or soon after the run starts or a reception lands (audit JUMP-3). Straight ahead, the front
/// group, its `_down` and `_up` mixed by the target's height `dz` over the feet, all up from 1.3 m (0xD9BE90; the running
/// game's events name only an action's first clip, `_down`).
pub fn impulsion(dz: f32, left: bool) -> Vec<(String, f32)> {
    let (foot, other) = if left { ("footl", "footr") } else { ("footr", "footl") };
    let up = dz.clamp(0.0, IMPULSION_UP) / IMPULSION_UP;
    let name = |v: &str| format!("xx_h_impultionstraight_{foot}_to_ground_to_air_front_{v}_{other}");
    [(name("down"), 1.0 - up), (name("up"), up)].into_iter().filter(|(_, w)| *w > 0.0).collect()
}

/// The impulsion's height (m) where its `_up` set takes all the weight (0xD9BE90).
const IMPULSION_UP: f32 = 1.3;

/// A running jump onto a top `dz` above (negative: below) and `dist` away, taking off from the left foot or the right;
/// `fast` (0..1, the run's speed against the sprint) blends the reception into its quick version (`_fast`).
pub fn running(dz: f32, dist: f32, left: bool, fast: f32) -> Jump {
    running_in(FREE_STEP, dz, dist, left, fast)
}

/// `running` on another target type's band.
pub fn running_in(b: Band, dz: f32, dist: f32, left: bool, fast: f32) -> Jump {
    let (foot, other) = if left { ("footl", "footr") } else { ("footr", "footl") };
    let v = dz.max(b.down);
    let rising = v >= DOWN_FROM;
    let down = dz < DOWN_FROM;
    let h = ((v - DOWN_FROM) / (if rising { b.up } else { b.down } - DOWN_FROM)).clamp(0.0, 1.0);
    let (class, d) = if dist < b.near {
        (Class::Near, dist / b.near)
    } else if rising {
        (Class::Up, (dist - b.near) / ((b.mid - b.near) * (1.0 - h)))
    } else if dist >= b.mid {
        (Class::Far, (dist - b.mid) / ((b.far - b.mid) * h))
    } else {
        (Class::Down, (dist - b.near) / (b.mid - b.near))
    };
    let d = if d.is_nan() { 1.0 } else { d.clamp(0.0, 1.0) };

    // Flight. Going down, the front slots are the ones landing lower (`_to_freestep_down`).
    let mut f = [0.0f32; 16];
    let fr = if down { [9, 10, 11] } else { [0, 1, 2] };
    match class {
        Class::Near => {
            f[fr[0]] = (1.0 - h) * (1.0 - d);
            f[fr[1]] = (1.0 - h) * d;
            if down {
                f[3] = h * (1.0 - d);
                f[4] = h * d;
            } else {
                f[7] = h * (1.0 - d);
                f[8] = h * d;
            }
        }
        Class::Up => {
            f[1] = (1.0 - h) * (1.0 - d);
            f[2] = (1.0 - h) * d;
            f[8] = h;
        }
        Class::Down => {
            f[fr[1]] = (1.0 - h) * (1.0 - d);
            f[fr[2]] = (1.0 - h) * d;
            f[4] = h * (1.0 - d);
            f[5] = h * d;
        }
        Class::Far => {
            f[5] = h * (1.0 - d);
            f[6] = h * d;
            f[fr[2]] = 1.0 - h;
        }
    }

    // Takeoff: the flight's blend on the takeoff's own slots (before the deep variants take any weight).
    let mut t = [0.0f32; 8];
    if class == Class::Near {
        t[0] = f[fr[0]];
        t[1] = f[fr[1]];
        if down {
            t[3] = f[3];
            t[4] = f[4];
        } else {
            t[6] = f[7];
            t[7] = f[8];
        }
    } else {
        t[1] = f[fr[1]];
        t[2] = f[fr[2]] + f[6];
        if class == Class::Up {
            t[7] = f[8];
        } else {
            t[4] = f[4];
            t[5] = f[5];
        }
    }

    // Far down: into the deep landings.
    if dz < -DEEP_FROM {
        let k = ((-dz - DEEP_FROM) / DEEP_FULL).clamp(0.0, 1.0);
        let moved = [(12, [9, 7, 3]), (13, [10, 8, 4])];
        for (to, from) in moved {
            f[to] = from.iter().map(|&i| f[i]).sum::<f32>() * k;
        }
        f[14] = (f[11] + f[5]) * k;
        f[15] = f[6] * k;
        for i in [3, 4, 5, 6, 7, 8, 9, 10, 11] {
            f[i] *= 1.0 - k;
        }
    }

    let pick = |names: Vec<String>, w: &[f32]| -> Vec<(String, f32)> { names.into_iter().zip(w.iter().copied()).filter(|(_, w)| *w > 0.01).collect() };
    let flight = pick(flight_names(foot), &f);
    // The reception goes on from the flights that land on the top (not the deep ones).
    let fast = fast.clamp(0.0, 1.0);
    let reception = flight
        .iter()
        .filter(|(n, _)| !n.ends_with("_deep"))
        .flat_map(|(n, w)| [(format!("{n}_tr_freestep_entry_{other}"), *w * (1.0 - fast)), (format!("{n}_tr_freestep_entry_{other}_fast"), *w * fast)])
        .filter(|(_, w)| *w > 0.01)
        .collect();
    Jump { takeoff: pick(takeoff_names(foot), &t), flight, reception }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn total(parts: &[(String, f32)]) -> f32 {
        parts.iter().map(|p| p.1).sum()
    }

    #[test]
    fn a_passover_flies_only_front_and_up_300() {
        for (dz, dist) in [(0.0, 1.5), (0.5, 3.5), (-1.0, 4.0), (1.0, 2.0)] {
            let j = passover(dz, dist, true);
            for (n, _) in &j.flight {
                let ok = ["front_050cm", "front_300cm", "front_550cm", "up_300cm"].iter().any(|d| n == &format!("xx_h_air_{d}_footl_to_passover"));
                assert!(ok, "{n}");
            }
            assert!((total(&j.flight) - 1.0).abs() < 1e-3);
        }
    }

    #[test]
    fn a_far_jump_at_a_bar_uses_the_650_cm_swing_flight() {
        let j = swing(0.0, 6.0, true);
        assert!(j.flight.iter().all(|(n, _)| n.ends_with("_footl_to_swing")), "{j:?}");
        assert!(j.flight.iter().any(|(n, _)| n.contains("front_650cm")), "{j:?}");
        assert!((total(&j.flight) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn wall_receptions_lean_with_the_wall() {
        // Square wall: the straight receptions alone; overhanging 15 degrees: half the 30 out ones; sloping 45: all 45 in.
        assert_eq!(surface(1.5, 3.0, false, false, 0.0).reception[1].0, "xx_hangwall_reception_front_straight_min_a");
        let half = &surface(1.5, 3.0, false, false, 15f32.to_radians()).reception[1].0;
        assert!(half.contains("straight_min_a=0.50") && half.contains("30_out_min_a=0.50"), "{half}");
        assert_eq!(surface(1.5, 3.0, false, true, -45f32.to_radians()).reception[0].0, "xx_h_air_surface_tr_hangwall_reception_front_45_in_max");
    }

    #[test]
    fn a_jump_at_a_hold_flies_onto_the_surface_and_is_received_on_the_wall() {
        let j = surface(1.5, 3.0, false, true, 0.0);
        assert!(j.flight.iter().all(|(n, _)| n.ends_with("_footr_to_surface")), "{j:?}");
        assert!((total(&j.flight) - 1.0).abs() < 1e-3);
        assert_eq!(j.reception[0].0, "xx_h_air_surface_tr_hangwall_reception_front_straight_max");
        assert_eq!(j.reception.len(), 4);
    }

    #[test]
    fn a_short_level_jump_is_the_short_front_clips() {
        let j = running(0.0, 1.25, true, 0.0);
        // Level is a little above the start of "down": mostly front, some up.
        assert!(j.flight.iter().any(|(n, _)| n == "xx_h_air_front_050cm_footl_to_freestep"));
        assert!(j.takeoff.iter().any(|(n, _)| n == "xx_h_run_front_050cm_footl_to_air"));
        assert!((total(&j.flight) - 1.0).abs() < 1e-4 && (total(&j.takeoff) - 1.0).abs() < 1e-4, "{j:?}");
        assert!(j.reception.iter().all(|(n, _)| n.ends_with("_tr_freestep_entry_footr")));
    }

    #[test]
    fn going_down_far_uses_the_down_and_deep_flights() {
        let j = running(-6.0, 6.0, false, 1.0);
        assert!(j.flight.iter().any(|(n, _)| n.contains("_down_") && n.ends_with("footr_to_freestep_deep")), "{j:?}");
        assert!((total(&j.flight) - 1.0).abs() < 1e-4);
        assert!(j.takeoff.iter().all(|(n, _)| n.contains("_footr_to_air")));
    }

    #[test]
    fn a_free_step_takeoff_blends_the_groups_around_its_angle() {
        let front = freestep(0.0, 3.0, 0.0, true, 0.0);
        assert!(front.takeoff.iter().all(|(n, _)| n.starts_with("xx_h_freestep_front_")), "{front:?}");
        let right = freestep(0.0, 3.0, std::f32::consts::FRAC_PI_4, true, 0.0);
        assert!(
            right.takeoff.iter().any(|(n, _)| n.starts_with("xx_h_freestep_right_"))
                && right.takeoff.iter().any(|(n, _)| n.starts_with("xx_h_freestep_front_"))
        );
        let back = freestep(0.0, 3.0, -3.0, true, 0.0);
        assert!(back.takeoff.iter().any(|(n, _)| n.starts_with("xx_h_freestep_backleft_")), "{back:?}");
        assert!((total(&back.takeoff) - 1.0).abs() < 1e-3);
        // The side groups' far takeoff is 500 cm.
        let far = freestep(0.0, 6.5, std::f32::consts::FRAC_PI_2, true, 0.0);
        assert!(far.takeoff.iter().any(|(n, _)| n == "xx_h_freestep_right_front_500cm_footl_to_air"), "{far:?}");
        let far_r = freestep(0.0, 6.5, std::f32::consts::FRAC_PI_2, false, 0.0);
        assert!(far_r.takeoff.iter().any(|(n, _)| n == "xx_h_freestep_right_front_550cm_footr"), "{far_r:?}");
        assert!(far_r.reception.iter().all(|(n, _)| n.contains("entry_footl")), "{far_r:?}");
    }

    #[test]
    fn up_onto_a_higher_top_uses_the_up_clips() {
        let j = running(1.3, 3.0, true, 0.5);
        assert!(j.flight.iter().all(|(n, _)| n.contains("_up_")), "{j:?}");
        assert!(j.takeoff.iter().all(|(n, _)| n.contains("_up_")));
    }

    #[test]
    fn a_hang_two_metres_up_is_not_all_the_way_up_on_its_band() {
        // (The free step's band tops out at 1.3 m, the surface's at 2.5 and the hang's at 3: AC1's 0xB1EC40.)
        let up = |j: &Jump| j.flight.iter().filter(|(n, _)| n.contains("_up_")).map(|p| p.1).sum::<f32>();
        assert!((up(&running(2.0, 4.0, true, 0.0)) - 1.0).abs() < 1e-4);
        let (surface, hang) = (up(&running_in(SURFACE, 2.0, 4.0, true, 0.0)), up(&running_in(HANG, 2.0, 4.0, true, 0.0)));
        assert!((surface - 2.5 / 3.0).abs() < 1e-4 && (hang - 2.5 / 3.5).abs() < 1e-4, "{surface} {hang}");
    }
}
