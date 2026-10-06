//! Climbing probe (`AC1_PROBE_CLIMB=n`, cities): how climbable the buildings near the start are. It takes `n` holds
//! within 120 m of the start (nearest first, spots 3 m apart, room to stand in front) that hang 1.6-2.6 m over the ground in front of them (a first hold reached from
//! standing), and for each puts Altaïr on the ground below it facing the wall, grabs, and holds "up" for 10 s,
//! logging how high he got and whether he climbed over the top; then a summary, and the game exits. The test world
//! is not probed.

use crate::character::{Character, Controller, Player};
use crate::level::Level;
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct Probe {
    spots: Vec<(Vec3, Vec3)>,
    i: usize,
    t: f32,
    from_y: f32,
    best: f32,
    results: Vec<(f32, bool)>,
    /// The grab is pressed (once: a second press would let go).
    pressed: bool,
    ready: bool,
}

/// Seconds per spot: the grab, then climbing up.
const PROBE_SECS: f32 = 10.0;

pub fn probe_climbs(
    time: Res<Time>,
    level: Res<Level>,
    mut probe: ResMut<Probe>,
    mut q: Query<(&mut Transform, &mut Controller, &mut Character), With<Player>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(n) = std::env::var("AC1_PROBE_CLIMB").ok().and_then(|s| s.parse::<usize>().ok()) else { return };
    if !level.city {
        return;
    }
    let Ok((mut tf, mut ctl, mut ch)) = q.single_mut() else { return };
    if !probe.ready {
        probe.ready = true;
        let home = level.spawn.unwrap_or(tf.translation);
        let mut spots = vec![];
        // Holds nearest the start first, spots at least 3 m apart, each with room to stand in front.
        let mut near: Vec<_> = level.ledges.iter().filter(|l| l.closest(home).with_y(0.0).distance(home.with_y(0.0)) < 120.0).collect();
        near.sort_by(|a, b| a.closest(home).distance(home).total_cmp(&b.closest(home).distance(home)));
        for l in near {
            let mid = (l.a + l.b) * 0.5;
            let stand = mid + l.out * 0.55;
            let Some(g) = level.ground(stand, 0.0, 3.0) else { continue };
            let up = mid.y - g.point.y;
            let room = level.raycast(g.point + Vec3::Y * 0.1, Vec3::Y, 1.8).is_none();
            let wall = level.raycast(g.point + Vec3::Y * 1.0, -l.out, 0.9).is_some();
            if (1.6..2.6).contains(&up) && room && wall && spots.iter().all(|(p, _): &(Vec3, Vec3)| p.distance(g.point) > 3.0) {
                spots.push((g.point, -l.out));
            }
            if spots.len() >= n {
                break;
            }
        }
        // (`AC1_PROBE_ONLY=k`: just that spot, to trace it.)
        if let Some(k) = std::env::var("AC1_PROBE_ONLY").ok().and_then(|s| s.parse::<usize>().ok()) {
            spots = spots.get(k).copied().into_iter().collect();
        }
        info!("probe: {} spots near {home:.1}", spots.len());
        probe.spots = spots;
        probe.t = 0.0;
    }
    let dt = time.delta_secs().min(0.05);
    let Some(&(at, face)) = probe.spots.get(probe.i) else {
        let n = probe.results.len().max(1) as f32;
        let mean = probe.results.iter().map(|r| r.0).sum::<f32>() / n;
        let tops = probe.results.iter().filter(|r| r.1).count();
        info!("probe: {} climbs, {:.1} m up on average, {tops} over the top", probe.results.len(), mean);
        exit.write(AppExit::Success);
        return;
    };
    if probe.t == 0.0 {
        tf.translation = at;
        tf.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, face.with_y(0.0).normalize_or(Vec3::NEG_Z));
        // (A fresh start: an earlier spot's fall may have hurt or killed him.)
        ch.wall = None;
        ch.velocity = Vec3::ZERO;
        ch.fall_v = 0.0;
        ch.health = 1.0;
        ch.dead_time = 0.0;
        ch.ragdoll = None;
        ch.edge_lock = None;
        ch.exit_fade = None;
        probe.from_y = at.y;
        probe.pressed = false;
        probe.best = 0.0;
    }
    probe.t += dt;
    ctl.move_dir = Vec3::ZERO;
    ctl.climb_dir = Vec2::ZERO;
    if probe.t >= 0.3 && !probe.pressed {
        probe.pressed = true;
        ctl.toggle_climb = true;
        ctl.grab_only = true;
    }
    if probe.t > 1.0 {
        ctl.climb_dir = Vec2::Y;
    }
    probe.best = probe.best.max(tf.translation.y - probe.from_y);
    if probe.t >= PROBE_SECS {
        // Over the top: standing (not climbing) well above where it started.
        let top = ch.wall.as_ref().is_none_or(|w| w.state == "top" || w.state == "perch") && tf.translation.y - probe.from_y > 1.5;
        let state = ch.wall.as_ref().map_or("ground".to_string(), |w| w.state.clone());
        info!("probe {}: at {at:.1}, up {:.1} m, {}, ends {state}", probe.i, probe.best, if top { "over the top" } else { "not over" });
        // What the wall is like: where its top is (just behind its face) and the holds up it, by height over the floor.
        if let Some(hit) = level.raycast(at + Vec3::Y * 1.0, face, 1.5) {
            let behind = hit.point + face * 0.3;
            let roof = level.ground(behind + Vec3::Y * 30.0, 0.0, 40.0).map(|g| g.point.y - at.y);
            let mut ups: Vec<f32> = level
                .ledges
                .iter()
                .filter(|l| l.out.dot(-face) > 0.7)
                .map(|l| l.closest(hit.point))
                .filter(|c| c.with_y(0.0).distance(hit.point.with_y(0.0)) < 0.7 && c.y > at.y + 0.5 && c.y < at.y + 12.0)
                .map(|c| c.y - at.y)
                .collect();
            ups.sort_by(f32::total_cmp);
            ups.dedup_by(|a, b| (*a - *b).abs() < 0.1);
            info!("probe {}: wall top {roof:.1?} m up, holds up it at {ups:.1?} m", probe.i);
            // (`AC1_PROBE_PROFILE`: the wall's depth every 0.1 m up, how far in from its face at 1 m.)
            if std::env::var("AC1_PROBE_PROFILE").is_ok() {
                let from = hit.point - face * 1.0;
                let profile: Vec<String> = (5..90)
                    .map(|k| {
                        let y = k as f32 * 0.1;
                        let d = level.raycast(from.with_y(at.y + y), face, 3.0).map(|h| (h.point - hit.point).dot(face));
                        format!("{y:.1}:{}", d.map_or("-".into(), |d| format!("{d:+.2}")))
                    })
                    .collect();
                info!("probe {}: profile {}", probe.i, profile.join(" "));
            }
        }
        let r = (probe.best, top);
        probe.results.push(r);
        probe.i += 1;
        probe.t = 0.0;
    }
}
