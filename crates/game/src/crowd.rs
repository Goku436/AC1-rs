//! Crowds: groups of scholars walking a loop in a tight formation with their hands together (AC1's
//! `_cpm_monk_pray_walking`), and the player blending into them.
//!
//! Walking (not running) among a group, two scholars within `BLEND_REACH`, the player blends: it walks
//! with them in the praying pose (`_cpm_monk_pray_walking` while moving, `xx_l_pray_wait` standing),
//! at most at `BLEND_SPEED`. Scholars are assembled like AC1 builds its crowds: the scholar robe, hood and
//! shoes from Acre's data with the shared human skeleton, limbs and head from the common data.
//!
//! Holding the legs (low profile) among them with no direction, the player walks along with the group
//! (its pace, steering into its middle). Running into them charges through: Altaïr shoulders them aside
//! (`xx_h_charge_run_shove_<left_handl|right_handr>_b`, upper body over the run) and they stumble out of the way
//! (`xx_stumble_soft_50cm_<back|front>_footl`), then walk back into their places.

use crate::animation::AnimLib;
use crate::character::Character;
use crate::character::Player;
use crate::level::Level;
use bevy::prelude::*;

pub const WALK_CLIP: &str = "_cpm_monk_pray_walking";
pub const PRAY_WAIT: &str = "xx_l_pray_wait";
/// Charging through the crowd: the shoulder shove on the side the scholar is on, its stumble (from the
/// front: back, from behind: front), and how far it is knocked aside (m).
const SHOVE: [&str; 2] = ["xx_h_charge_run_shove_left_handl_b", "xx_h_charge_run_shove_right_handr_b"];
const STUMBLE: [&str; 2] = ["xx_stumble_soft_50cm_back_footl", "xx_stumble_soft_50cm_front_footl"];
const KNOCK: f32 = 0.9;
/// Running at least this fast (m/s), the player charges through scholars within `CHARGE_REACH` ahead.
const CHARGE_SPEED: f32 = 2.5;
const CHARGE_REACH: f32 = 1.0;

type Charger<'w, 's> = Query<'w, 's, (&'static Transform, &'static mut Character), (With<Player>, Without<Scholar>)>;
type Charged<'w, 's> = Query<'w, 's, (&'static mut Scholar, &'static Transform, &'static mut Character), Without<Player>>;

/// The player running into scholars shoves them aside (AC1's charge): see the module docs.
pub fn charge(mut lib: Option<ResMut<AnimLib>>, mut player: Charger, mut scholars: Charged) {
    let (Some(lib), Ok((tf, mut ch))) = (lib.as_deref_mut(), player.single_mut()) else { return };
    let v = ch.velocity.with_y(0.0);
    if v.length() < CHARGE_SPEED || ch.wall.is_some() {
        return;
    }
    let fwd = v.normalize();
    for (mut sc, stf, mut sch) in &mut scholars {
        let d = (stf.translation - tf.translation).with_y(0.0);
        if sc.stumble > 0.0 || d.length() > CHARGE_REACH || d.normalize_or_zero().dot(fwd) < 0.3 {
            continue;
        }
        // Out of the way, to the side it is on, and on a little.
        let side = if fwd.cross(Vec3::Y).dot(d) >= 0.0 { fwd.cross(Vec3::Y) } else { -fwd.cross(Vec3::Y) };
        sc.knocked += side * KNOCK + fwd * 0.3;
        let facing = (stf.rotation * Vec3::NEG_Z).with_y(0.0);
        let rig = sch.rig_id;
        if let (Some(a), Some(clip)) = (sch.animator.as_mut(), lib.get_for(rig, STUMBLE[(facing.dot(fwd) > 0.0) as usize])) {
            sc.stumble = clip.anim.duration;
            a.forced = Some((clip, 0.0));
        }
        let right = fwd.cross(Vec3::Y).dot(d) > 0.0;
        if let (Some(a), Some(clip)) = (ch.animator.as_mut(), lib.get(SHOVE[right as usize]))
            && a.oneshot.is_none()
        {
            a.oneshot = Some((clip, 0.0));
        }
        debug!("crowd: charged through a scholar");
    }
}
/// Blending needs this many scholars this close (m), and walks at most this fast (m/s).
const BLEND_COUNT: usize = 2;
const BLEND_REACH: f32 = 1.4;
const FOLLOW_REACH: f32 = 2.5;
pub const BLEND_SPEED: f32 = 1.0;
const SCHOLAR_SPEED: f32 = 0.8;

/// The parts of a scholar: (forge, data file, object) for the skeletons and the meshes.
/// (forge, data file, object) of an assembled character's part.
pub type PartRef = (String, String, String);

pub fn scholar_recipe() -> (Vec<PartRef>, Vec<PartRef>) {
    let c = |f: &str, d: &str, n: &str| (f.to_string(), d.to_string(), n.to_string());
    let (common, acre, block) = ("DataPC_Common.forge", "DataPC_Acre.forge", "Acre_Outskirts Setup Acre_DataBlock");
    (
        // (With the head's own face rig: its face, eyes and mouth are skinned to it. A head on another head's
        // rig shows its teeth through the face.)
        vec![
            c(common, "MMMA_FreeMission_Leader", "Human_Body"),
            c(common, "MMMA_FreeMission_Leader", "Human_Skirt_Long"),
            c(common, "MMMA_Damascus_Militia", "ACMA_Rafic_A_Head"),
        ],
        vec![
            c(acre, block, "CNMT_Scholar_B_Body"),
            c(acre, block, "CNMT_Scholar_Hood"),
            c(acre, block, "CNMT_Scholar_Shoes"),
            c(acre, block, "Universal_Arm_M_Thin_Hand"),
            c(common, "Universal_Head_Obj", "Universal_Head_Obj"),
            c(common, "ACMA_Rafic_A_Head", "ACMA_Rafic_A_Head"),
        ],
    )
}

/// A group walking a closed loop of waypoints, `s` metres along it.
pub struct Group {
    pub path: Vec<Vec3>,
    pub s: f32,
}

impl Group {
    fn len(&self) -> f32 {
        (0..self.path.len()).map(|i| (self.path[(i + 1) % self.path.len()] - self.path[i]).length()).sum()
    }

    /// Point and heading `s` metres along the loop.
    pub fn at(&self, s: f32) -> (Vec3, Vec3) {
        let mut s = s.rem_euclid(self.len().max(1e-3));
        for i in 0..self.path.len() {
            let (a, b) = (self.path[i], self.path[(i + 1) % self.path.len()]);
            let l = (b - a).length();
            if s <= l {
                return (a + (b - a) * (s / l.max(1e-3)), (b - a).normalize_or(Vec3::X));
            }
            s -= l;
        }
        (self.path[0], Vec3::X)
    }
}

#[derive(Resource, Default)]
pub struct Crowds {
    pub groups: Vec<Group>,
}

/// A scholar's place in its group: sideways and behind the group's centre (m).
#[derive(Component)]
pub struct Scholar {
    pub group: usize,
    pub slot: Vec2,
    /// Knocked out of its place by the player charging through (world offset, easing back), and the
    /// stumble's time left.
    pub knocked: Vec3,
    pub stumble: f32,
}

impl Scholar {
    pub fn new(group: usize, slot: Vec2) -> Self {
        Scholar { group, slot, knocked: Vec3::ZERO, stumble: 0.0 }
    }
}

/// Walk the groups along their loops; members keep their places, turned along the way.
pub fn walk_crowds(time: Res<Time>, level: Res<Level>, mut crowds: ResMut<Crowds>, mut q: Query<(&mut Scholar, &mut Transform, &mut Character)>) {
    let dt = time.delta_secs().min(0.05);
    for g in &mut crowds.groups {
        g.s += SCHOLAR_SPEED * dt;
    }
    for (mut sc, mut tf, mut ch) in &mut q {
        let Some(g) = crowds.groups.get(sc.group) else { continue };
        let (centre, dir) = g.at(g.s - sc.slot.y);
        let side = dir.cross(Vec3::Y);
        // Knocked aside: stumble, then walk back into place.
        if sc.stumble > 0.0 {
            sc.stumble -= dt;
            if sc.stumble <= 0.0
                && let Some(a) = &mut ch.animator
            {
                a.forced = None;
            }
        } else {
            sc.knocked *= (-1.5 * dt).exp();
        }
        let mut p = centre + side * sc.slot.x + sc.knocked;
        p.y = level.ground(p, 3.0, 6.0).map_or(tf.translation.y, |h| h.point.y);
        tf.translation = p;
        let face = Quat::from_rotation_arc(Vec3::NEG_Z, dir.with_y(0.0).normalize_or(Vec3::NEG_Z));
        tf.rotation = tf.rotation.slerp(face, 1.0 - (-4.0 * dt).exp());
        ch.velocity = dir * SCHOLAR_SPEED;
    }
}

type ScholarQuery<'w, 's> = Query<'w, 's, (&'static Transform, &'static Character), (With<Scholar>, Without<Player>)>;
type PlayerQuery<'w, 's> = Query<'w, 's, (&'static Transform, &'static mut Character, &'static crate::character::Controller), With<Player>>;

/// The player blends while walking among scholars (see the module docs).
pub fn blend(mut lib: Option<ResMut<AnimLib>>, scholars: ScholarQuery, mut player: PlayerQuery) {
    let (Some(lib), Ok((tf, mut ch, ctl))) = (lib.as_deref_mut(), player.single_mut()) else { return };
    // (Following, it stays with a group a little further off.)
    let reach = if ctl.blend_walk { FOLLOW_REACH } else { BLEND_REACH };
    let near: Vec<(Vec3, Vec3)> =
        scholars.iter().filter(|(s, _)| (s.translation - tf.translation).with_y(0.0).length() < reach).map(|(s, c)| (s.translation, c.velocity)).collect();
    let moving = ch.velocity.length() > 0.3;
    let calm = ch.velocity.length() <= BLEND_SPEED + 0.3 && ch.wall.is_none();
    // Hidden in the crowd, or (legs held, low profile) the praying walk on its own.
    let blend = near.len() >= BLEND_COUNT && calm;
    let posture = calm && (blend || ctl.blend_walk);
    // Legs held among the scholars: walk along with them.
    // (Their pace, steering into the middle of the group.)
    if blend && ctl.blend_walk {
        let n = near.len() as f32;
        let (centre, pace) = near.iter().fold((Vec3::ZERO, Vec3::ZERO), |(c, v), (p, w)| (c + *p / n, v + *w / n));
        ch.follow = Some((pace + (centre - tf.translation).with_y(0.0) * 1.2).clamp_length_max(BLEND_SPEED * 1.5));
    }
    let clip = posture.then(|| lib.get(if moving { WALK_CLIP } else { PRAY_WAIT })).flatten();
    let want = clip.as_ref().map(|c| c.name.clone());
    let now = ch.animator.as_ref().and_then(|a| a.layer.as_ref().map(|f| f.0.name.clone()));
    let showing_blend = now.as_deref().is_some_and(|n| n == WALK_CLIP || n == PRAY_WAIT);
    if want != now && (want.is_some() || showing_blend) {
        // Crossfade in and out of the praying walk.
        ch.exit_fade = Some((ch.anim_pose.clone(), 0.3, 0.3));
        if let Some(a) = &mut ch.animator {
            a.layer = clip.map(|c| (c, 0.0));
        }
    }
    ch.blend = blend;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_round_the_loop() {
        let g = Group { path: vec![Vec3::ZERO, Vec3::new(4.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 2.0), Vec3::new(0.0, 0.0, 2.0)], s: 0.0 };
        let (p, d) = g.at(1.0);
        assert!((p - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-5 && (d - Vec3::X).length() < 1e-5);
        let (p, d) = g.at(5.0);
        assert!((p - Vec3::new(4.0, 0.0, 1.0)).length() < 1e-5 && (d - Vec3::Z).length() < 1e-5);
        // Wraps around (the loop is 12 m).
        assert!((g.at(13.0).0 - g.at(1.0).0).length() < 1e-5);
        assert!((g.at(-1.0).0 - Vec3::new(0.0, 0.0, 1.0)).length() < 1e-5);
    }
}
