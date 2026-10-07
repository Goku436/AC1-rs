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

/// Height and distance bands of a free-step target (m): the highest up and lowest down a jump blends to, where a
/// jump starts to count as going down, and the near, middle and far distances.
const UP_MAX: f32 = 1.3;
const DOWN_MAX: f32 = -3.0;
const DOWN_FROM: f32 = -0.5;
const NEAR: f32 = 2.5;
const MID: f32 = 5.0;
const FAR: f32 = 7.0;
/// More than this far down (m), the flight blends into its deep variants, fully at `DEEP_FULL` further.
const DEEP_FROM: f32 = 3.0;
const DEEP_FULL: f32 = 5.0;

const FRONT: [&str; 3] = ["050cm", "300cm", "550cm"];
const DOWN: [&str; 4] = ["050cm", "300cm", "550cm", "800cm"];
const UP: [&str; 2] = ["050cm", "300cm"];

/// Which part of the band a jump falls in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    /// Closer than `NEAR`.
    Near,
    /// Further, level or up.
    Up,
    /// Further, going down, short of `MID`.
    Down,
    /// Further, going down, past `MID`.
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
    let mut j = running(dz, dist, left, fast);
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

/// A running jump at a wall's hold to catch (a lone ledge, the feet against the wall under it): the running jump's takeoff
/// for the hang's place `dz` above and `dist` away, its flight onto a surface (`xx_h_air_<front|down|up>_<dist>_foot?_to_
/// surface`: the flight's slots folded onto those: the down landings, deep or not, onto `down`, the front going down onto
/// `front`), then AC1's reception on the wall (`xx_h_air_surface_tr_hangwall_reception_front_straight_<min|max>` and its
/// `xx_hangwall_reception_..._a/b/c`), the hard one (`max`) when `fast`.
pub fn surface(dz: f32, dist: f32, left: bool, fast: bool) -> Jump {
    let j = running(dz, dist, left, 0.0);
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
    let reception = std::iter::once(format!("xx_h_air_surface_tr_hangwall_reception_front_straight_{kind}"))
        .chain(["a", "b", "c"].map(|k| format!("xx_hangwall_reception_front_straight_{kind}_{k}")))
        .map(|n| (n, 1.0))
        .collect();
    Jump { takeoff: j.takeoff, flight, reception }
}

/// A running jump at a swing bar, the hang under it `dz` above and `dist` away: the running jump's takeoff and its flight
/// onto the bar (`xx_h_air_<front|down|up>_<dist>_foot?_to_swing`; AC1's far ones are front 650 and down 900 cm, the
/// flight's 550 and 800 cm slots fold onto them).
pub fn swing(dz: f32, dist: f32, left: bool) -> Jump {
    let j = running(dz, dist, left, 0.0);
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

/// A running jump onto a top `dz` above (negative: below) and `dist` away, taking off from the left foot or the right;
/// `fast` (0..1, the run's speed against the sprint) blends the reception into its quick version (`_fast`).
pub fn running(dz: f32, dist: f32, left: bool, fast: f32) -> Jump {
    let (foot, other) = if left { ("footl", "footr") } else { ("footr", "footl") };
    let v = dz.max(DOWN_MAX);
    let rising = v >= DOWN_FROM;
    let down = dz < DOWN_FROM;
    let h = ((v - DOWN_FROM) / (if rising { UP_MAX } else { DOWN_MAX } - DOWN_FROM)).clamp(0.0, 1.0);
    let (class, d) = if dist < NEAR {
        (Class::Near, dist / NEAR)
    } else if rising {
        (Class::Up, (dist - NEAR) / ((MID - NEAR) * (1.0 - h)))
    } else if dist >= MID {
        (Class::Far, (dist - MID) / ((FAR - MID) * h))
    } else {
        (Class::Down, (dist - NEAR) / (MID - NEAR))
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
    fn a_far_jump_at_a_bar_uses_the_650_cm_swing_flight() {
        let j = swing(0.0, 6.0, true);
        assert!(j.flight.iter().all(|(n, _)| n.ends_with("_footl_to_swing")), "{j:?}");
        assert!(j.flight.iter().any(|(n, _)| n.contains("front_650cm")), "{j:?}");
        assert!((total(&j.flight) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn a_jump_at_a_hold_flies_onto_the_surface_and_is_received_on_the_wall() {
        let j = surface(1.5, 3.0, false, true);
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
}
