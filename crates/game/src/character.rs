//! Altaïr: skinned character, procedural locomotion and the IK layers on top of it.
//!
//! Pose pipeline per frame (model space, Z up, +X forward):
//! idle base pose -> gait overlay (arm swing, shoulder counter-twist, hip bob, lean)
//! -> foot stepping + pelvis drop + two-bone leg IK with slope alignment
//! -> climbing hand/foot placement -> head/spine look-at -> joint transforms.

use crate::level::{Hit, Ledge, Level};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::prelude::*;
use ik::chain::{LookLink, look_at};
use ik::placement::{Arm, place_hand};
use ik::{Pose, Rig, limited_arc, two_bone_ik};

/// Model frame (X forward, Y left, Z up) -> Bevy frame (-Z forward, -X left, Y up).
pub fn model_to_bevy() -> Quat {
    Quat::from_mat3(&Mat3::from_cols(Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y))
}

#[derive(Clone, Copy)]
struct LegBones {
    upper: usize,
    lower: usize,
    foot: usize,
}

/// The player character.
#[derive(Component)]
pub struct Player;

/// A figure of the pose gallery: holds its clip where it stands (no movement, gravity or ground).
#[derive(Component)]
pub struct Statue;

/// F6 pauses the gallery and line-up figures; while they play, only those near the camera move (each one
/// samples its clip, and Altaïr's robe is cloth rebuilt every frame: a hundred of them cost most of a frame).
#[derive(Resource, Default)]
pub struct StatuePlay {
    pub paused: bool,
}

/// How near the camera a figure must be to move, and to swing its robe (m).
const STATUE_NEAR: f32 = 18.0;
pub const STATUE_CLOTH_NEAR: f32 = 6.0;
/// Frames every figure moves at the start, so the far ones settle into their pose and robes.
const STATUE_SETTLE: u32 = 60;

/// Whether a figure at `at` holds still this frame (its robe: with `near` = `STATUE_CLOTH_NEAR`).
pub fn statue_still(play: &StatuePlay, frame: u32, cam: Option<Vec3>, at: Vec3, near: f32) -> bool {
    frame >= STATUE_SETTLE && (play.paused || cam.is_some_and(|c| c.distance(at) > near))
}

/// Blend `layer` into `pose` (weight `w`) for the bones at or below any of `roots`.
fn layered(pose: &mut Pose, layer: &Pose, w: f32, rig: &Rig, roots: &[usize]) {
    for i in 0..rig.len() {
        if roots.iter().any(|&r| descends(rig, i, r)) {
            let (p, u) = (&mut pose.local[i], &layer.local[i]);
            p.pos = p.pos.lerp(u.pos, w);
            p.rot = p.rot.slerp(u.rot, w).normalize();
        }
    }
}

/// Whether bone `i` is `root` or below it.
fn descends(rig: &Rig, mut i: usize, root: usize) -> bool {
    loop {
        if i == root {
            return true;
        }
        match rig.parents[i] {
            Some(p) => i = p,
            None => return false,
        }
    }
}

struct Bones {
    hips: usize,
    spine: [usize; 3],
    neck: usize,
    head: usize,
    legs: [LegBones; 2],
    arms: [Arm; 2],
    clavicles: [usize; 2],
    /// Add-on bones under Spine2 that are not part of the biped (hood): they follow the head.
    hood: Vec<usize>,
}

impl Bones {
    fn find(rig: &Rig) -> Option<Bones> {
        let f = |n: &str| rig.find(n);
        let leg = |s: &str| Some(LegBones { upper: f(&format!("{s}UpLeg"))?, lower: f(&format!("{s}Leg"))?, foot: f(&format!("{s}Foot"))? });
        Some(Bones {
            hips: f("Hips")?,
            spine: [f("Spine")?, f("Spine1")?, f("Spine2")?],
            neck: f("Neck")?,
            head: f("Head")?,
            legs: [leg("Left")?, leg("Right")?],
            arms: [Arm::from_rig(rig, "Left")?, Arm::from_rig(rig, "Right")?],
            clavicles: [f("LeftClavicle")?, f("RightClavicle")?],
            hood: (0..rig.len())
                .filter(|&i| rig.parents[i] == f("Spine2") && rig.names[i].as_deref().is_some_and(|n| n.starts_with('#')))
                .filter(|&i| rig.rest[i].pos.z.abs() + rig.rest[i].pos.x.abs() > 0.0)
                .collect(),
        })
    }
}

#[derive(Clone, Copy)]
struct Swing {
    from: Vec3,
    to: Vec3,
    to_normal: Vec3,
    t: f32,
    dur: f32,
}

#[derive(Clone, Copy)]
struct Foot {
    /// World ground point under the sole.
    planted: Vec3,
    normal: Vec3,
    swing: Option<Swing>,
}

#[derive(Clone, Copy)]
struct HandMove {
    hand: usize,
    from: Vec3,
    to: Vec3,
    t: f32,
}

pub struct Climb {
    ledge_normal: Vec3,
    grips: [Vec3; 2],
    moving: Option<HandMove>,
    /// Blend in/out of the climbing pose.
    weight: f32,
    leaving: bool,
}

#[derive(Component)]
pub struct Character {
    /// Which rig the animation library binds this character's clips to.
    pub rig_id: usize,
    /// Blending into a crowd (see `crowd`).
    pub blend: bool,
    /// The last landing: drop (m) and damage, for the HUD.
    pub last_landing: Option<(f32, f32)>,
    /// Stopped at the edge of a big drop: its outward direction; walking on over it is held back.
    pub edge_lock: Option<Vec3>,
    /// Walking with a crowd it blends into (legs held, no direction): the crowd's velocity.
    pub follow: Option<Vec3>,
    rig: Rig,
    /// Left/right bone pairs, to play one-sided clips mirrored.
    mirror: ik::mirror::Mirror,
    bones: Bones,
    joints: Vec<Entity>,
    idle: Pose,
    /// Rest pose: the starting point clips are sampled onto.
    base: Pose,
    pub pose: Pose,
    /// AC1 animation playback; `None` falls back to procedural locomotion.
    pub animator: Option<crate::animation::Animator>,
    /// Animation-driven foot placement on uneven ground.
    foot_ik: Option<ik::placement::FootPlacement>,
    /// Clip-driven wall climbing (used when animation data is available).
    pub wall: Option<crate::climb::WallClimb>,
    /// The foot the next free-step jump (off a post or beam) takes off from: the other one from the last reception's.
    freestep_left: bool,
    climb_rig: crate::climb::ClimbRig,
    pub(crate) fall_v: f32,
    /// Pose to fade from after leaving the wall (or changing stand), the time left and the fade's length.
    pub(crate) exit_fade: Option<(Pose, f32, f32)>,
    /// The animated pose of the last frame, before the IK (what fades start from: from the final pose the IK would
    /// count twice through them).
    pub(crate) anim_pose: Pose,
    /// The robe tails as cloth (Altaïr's), see `robe`.
    pub robe: Option<crate::robe::Robe>,
    /// Limp (dead, its death clip over): AC1's ragdoll, and the pose it started from.
    pub ragdoll: Option<(ik::ragdoll::Ragdoll, Pose)>,
    ragdoll_def: Option<std::sync::Arc<ik::ragdoll::RagdollDef>>,
    /// Test hook: go limp now (`AC1_LIMP`).
    /// The ground speed value (see `gait`).
    pub gait: crate::gait::Gait,
    /// Climbing: each foot's smoothed distance onto the wall (see the climbing IK).
    pub wall_feet: [Option<f32>; 2],
    /// How much the climbing IK (hands on holds, feet on the wall) applies now (0..1, faded).
    pub wall_ik_w: f32,
    /// How much ground foot placement applies now (0..1, faded: see the IK).
    pub foot_ik_w: f32,
    pub limp: bool,
    /// Model-space height of the soles in the idle pose.
    floor: f32,
    ankle_height: f32,
    look_axis: Vec3,
    hand_basis: [Quat; 2],
    palm_offset: [Vec3; 2],
    feet: [Foot; 2],
    pelvis: f32,
    look_weight: f32,
    /// Per arm: how far (world) the hand is being pushed out of geometry it would pass through, smoothed.
    arm_push: [Vec3; 2],
    look_target: Vec3,
    pub climb: Option<Climb>,
    pub velocity: Vec3,
    pub ik_enabled: bool,
    /// Health (1 = full), regenerating; a fatal landing empties it and respawns after a while.
    pub health: f32,
    pub(crate) dead_time: f32,
    spawn: Vec3,
    /// Debug: world-space IK targets drawn as gizmos.
    pub debug_targets: Vec<(Vec3, Color)>,
}

/// Entity carrying the model->Bevy rotation and floor offset (child of the character root).
#[derive(Component)]
pub struct ModelRoot;

/// Input for the character this frame (from the player or a script).
#[derive(Component, Default)]
pub struct Controller {
    /// Desired world-space horizontal velocity.
    pub move_dir: Vec3,
    pub speed: f32,
    /// The stick (0..1) when the speed comes from the speed model (`gait`): the player's input. `None`: `speed` as
    /// given (scripts, the crowd).
    pub stick: Option<f32>,
    /// Climb input: x = right, y = up.
    pub climb_dir: Vec2,
    /// Leap modifier held: climb moves become leaps.
    pub leap: bool,
    /// Grab a wall, let go of it, or (running, nothing to grab) jump.
    pub toggle_climb: bool,
    /// Only grab or climb onto something in reach (no jump): the legs standing, or in low profile.
    pub grab_only: bool,
    /// The player's legs press: jump only with a drop just ahead (pressing them to start free running is not a jump).
    pub jump_needs_edge: bool,
    /// The legs pressed while climbing: jump off a perch, rebound off a wall run, fling off a bar.
    pub wall_legs: bool,
    /// High profile (AC1's right button): faster, louder moves.
    pub high: bool,
    /// Free running (high profile + legs): runs up, vaults and jumps on its own.
    pub free_run: bool,
    /// Blending (low profile + legs): the slow praying walk.
    pub blend_walk: bool,
    /// Low profile empty hand: a gentle push.
    pub push: bool,
    /// The empty hand next to a scholar: pickpocket it.
    pub pickpocket: bool,
}

fn idle_pose(rig: &Rig, b: &Bones) -> Pose {
    let mut pose = rig.rest_pose();
    // Bind pose is an A-pose; lower the arms and bend the elbows slightly.
    for (i, arm) in b.arms.iter().enumerate() {
        let side = if i == 0 { 1.0 } else { -1.0 };
        let m = pose.model(rig);
        let upper_dir = m[arm.lower].pos - m[arm.upper].pos;
        pose.rotate_model(rig, arm.upper, limited_arc(upper_dir, Vec3::new(-0.05, 0.16 * side, -1.0), 3.0));
        let m = pose.model(rig);
        let fore_dir = m[arm.hand].pos - m[arm.lower].pos;
        pose.rotate_model(rig, arm.lower, limited_arc(fore_dir, Vec3::new(0.35, 0.05 * side, -1.0), 3.0));
    }
    // Feet closer together than the bind stance.
    for (i, leg) in b.legs.iter().enumerate() {
        let side = if i == 0 { 1.0 } else { -1.0 };
        let m = pose.model(rig);
        let dir = m[leg.foot].pos - m[leg.upper].pos;
        let want = Vec3::new(0.0, (0.12 - m[leg.upper].pos.y.abs()) * side, dir.z);
        pose.rotate_model(rig, leg.upper, limited_arc(dir, want, 0.5));
        let m = pose.model(rig);
        // Keep the foot flat.
        let rest_foot = rig.rest_pose().model(rig)[leg.foot].rot;
        pose.set_model_rot(rig, leg.foot, rest_foot);
        let _ = m;
    }
    pose
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_character(
    commands: &mut Commands,
    data: crate::assets::CharacterData,
    at: Vec3,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    bindposes: &mut Assets<SkinnedMeshInverseBindposes>,
    animator: Option<crate::animation::Animator>,
    rig_id: usize,
) -> Entity {
    let rig = data.rig;
    let bones = Bones::find(&rig).expect("rig lacks standard biped bones");
    let base = rig.rest_pose();
    let procedural_idle = idle_pose(&rig, &bones);
    // AC1 clips put the ground at model z = 0; the procedural fallback stands on the rest-pose soles.
    let idle = animator.as_ref().map_or_else(|| procedural_idle.clone(), |a| a.pose(&base));
    let rest_model = rig.rest_pose().model(&rig);
    let idle_model = procedural_idle.model(&rig);

    let toe_z = (0..rig.len())
        .filter(|&i| rig.parents[i] == Some(bones.legs[0].foot))
        .map(|i| idle_model[i].pos.z)
        .fold(idle_model[bones.legs[0].foot].pos.z, f32::min);
    let ankle_height = idle_model[bones.legs[0].foot].pos.z - (toe_z - 0.015);
    let floor = if animator.is_some() { 0.0 } else { toe_z - 0.015 };

    // Head forward axis in head-local space.
    let look_axis = rest_model[bones.head].rot.inverse() * Vec3::X;

    // Hand basis: fingers, side, back-of-hand, in hand-local space (see ik::placement::place_hand).
    let mut hand_basis = [Quat::IDENTITY; 2];
    let mut palm_offset = [Vec3::ZERO; 2];
    for (i, side) in ["Left", "Right"].iter().enumerate() {
        let hand = bones.arms[i].hand;
        let hm = rest_model[hand];
        let mid = rig.find(&format!("{side}HandMiddle1")).map(|b| rest_model[b].pos).unwrap_or(hm.pos + hm.rot * Vec3::X * 0.08);
        let index = rig.find(&format!("{side}HandIndex1")).map(|b| rest_model[b].pos).unwrap_or(mid);
        let pinky = rig.find(&format!("{side}HandPinky1")).map(|b| rest_model[b].pos).unwrap_or(mid);
        let inv = hm.rot.inverse();
        let fingers = (inv * (mid - hm.pos)).normalize();
        let across = (inv * (index - pinky)).normalize_or(Vec3::Y);
        let sign = if i == 0 { 1.0 } else { -1.0 };
        let palm = (fingers.cross(across) * sign).normalize();
        let side_axis = fingers.cross(palm).normalize();
        let fingers = palm.cross(side_axis);
        hand_basis[i] = Quat::from_mat3(&Mat3::from_cols(fingers, side_axis, -palm));
        palm_offset[i] = inv * (mid - hm.pos) * 0.6 + palm * 0.025;
    }

    let root = commands.spawn((Transform::from_translation(at), Visibility::default(), Controller::default())).id();
    let model = commands
        .spawn((ModelRoot, Transform { translation: Vec3::Y * -floor, rotation: model_to_bevy(), ..default() }, Visibility::default(), ChildOf(root)))
        .id();

    let mut joints = Vec::with_capacity(rig.len());
    for i in 0..rig.len() {
        let l = idle.local[i];
        let parent = rig.parents[i].map(|p| joints[p]).unwrap_or(model);
        let e = commands.spawn((Transform { translation: l.pos, rotation: l.rot, ..default() }, Visibility::default(), ChildOf(parent))).id();
        joints.push(e);
    }

    let textures: std::collections::HashMap<u32, Handle<Image>> = data.textures.into_iter().map(|(id, img)| (id, images.add(img))).collect();
    let mut robe = None;
    for part in data.parts {
        // Cloth is seen from inside too (AC1 gives its cloth an inside material): the robe's tails fold and swing
        // their backs into view, which back-face culling would leave as holes.
        let two_sided = part.alpha || part.material.contains("Cloth");
        let mat = mats.add(StandardMaterial {
            base_color_texture: part.texture.and_then(|t| textures.get(&t).cloned()),
            normal_map_texture: part.normal.and_then(|t| textures.get(&t).cloned()),
            flip_normal_map_y: true,
            base_color: if part.texture.is_some() { Color::WHITE } else { Color::srgb(0.85, 0.82, 0.78) },
            alpha_mode: if part.alpha { AlphaMode::Mask(0.5) } else { AlphaMode::Opaque },
            double_sided: two_sided,
            cull_mode: if two_sided { None } else { Some(bevy::render::render_resource::Face::Back) },
            perceptual_roughness: 0.8,
            ..default()
        });
        // The robe tails: simulated as cloth, drawn from a world-space copy (see `robe`).
        if part.name.split('#').next() == Some(crate::robe::ROBE) && std::env::var("AC1_NO_CLOTH").is_err() {
            let mut copy = part.mesh.clone();
            copy.remove_attribute(Mesh::ATTRIBUTE_JOINT_INDEX);
            copy.remove_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT);
            let shown = meshes.add(copy);
            let jents: Vec<Entity> = part.joints.iter().map(|&r| joints[r]).collect();
            if let Some(r) = crate::robe::Robe::new(&part.mesh, part.inverse_binds.clone(), jents, shown.clone()) {
                commands.spawn((Name::new("robe cloth"), Mesh3d(shown), MeshMaterial3d(mat), NoFrustumCulling, Transform::default()));
                robe = Some(r);
                continue;
            }
        }
        // Faces inside hoods: the hood's shadow-map shadow turns them black; AC1 keeps them lit.
        let face = part.name.contains("Head");
        let mut e = commands.spawn((
            Name::new(part.name),
            Mesh3d(meshes.add(part.mesh)),
            MeshMaterial3d(mat),
            SkinnedMesh {
                inverse_bindposes: bindposes.add(SkinnedMeshInverseBindposes::from(part.inverse_binds)),
                joints: part.joints.iter().map(|&r| joints[r]).collect(),
            },
            NoFrustumCulling,
            Transform::default(),
            ChildOf(model),
        ));
        if face {
            e.insert(bevy::light::NotShadowReceiver);
        }
    }

    let foot = Foot { planted: at, normal: Vec3::Y, swing: None };
    let foot_ik = animator.as_ref().and_then(|_| ik::placement::FootPlacement::new(&rig));
    commands.entity(root).insert(Character {
        rig_id,
        blend: false,
        follow: None,
        edge_lock: None,
        last_landing: None,
        pose: idle.clone(),
        anim_pose: idle.clone(),
        idle,
        base,
        animator,
        foot_ik,
        wall: None,
        climb_rig: crate::climb::ClimbRig {
            hands: [bones.arms[0].hand, bones.arms[1].hand],
            feet: [bones.legs[0].foot, bones.legs[1].foot],
            reference: rig.find("Reference"),
        },
        fall_v: 0.0,
        exit_fade: None,
        freestep_left: true,
        robe,
        ragdoll: None,
        ragdoll_def: None,
        limp: false,
        gait: Default::default(),
        wall_feet: [None; 2],
        foot_ik_w: 0.0,
        wall_ik_w: 0.0,
        mirror: ik::mirror::Mirror::new(&rig),
        rig,
        bones,
        joints,
        floor,
        ankle_height,
        look_axis,
        hand_basis,
        palm_offset,
        feet: [foot; 2],
        pelvis: 0.0,
        look_weight: 0.0,
        arm_push: [Vec3::ZERO; 2],
        look_target: Vec3::ZERO,
        climb: None,
        velocity: Vec3::ZERO,
        ik_enabled: true,
        health: 1.0,
        dead_time: 0.0,
        spawn: at,
        debug_targets: vec![],
    });
    root
}

impl Character {
    /// The joint entity of the rig bone named `name`.
    pub fn joint_named(&self, name: &str) -> Option<Entity> {
        self.rig.find(name).map(|b| self.joints[b])
    }

    pub fn joint_entities(&self) -> &[Entity] {
        &self.joints
    }
    /// The hips joint (where the body really is: paired clips can carry it away from the root).
    /// Capsules round the hips and legs (world), from the bones' world transforms: what the robe swings
    /// clear of (AC1's cloth ragdoll's sizes).
    pub fn leg_capsules(&self, globals: &Query<&GlobalTransform>) -> Vec<ik::cloth::Capsule> {
        let at = |b: usize| globals.get(self.joints[b]).map(|g| g.translation()).ok();
        let b = &self.bones;
        let mut out = vec![];
        if let Some(h) = at(b.hips) {
            out.push(ik::cloth::Capsule { a: h, b: h, r: 0.13 });
        }
        for leg in &b.legs {
            if let (Some(u), Some(l), Some(f)) = (at(leg.upper), at(leg.lower), at(leg.foot)) {
                out.push(ik::cloth::Capsule { a: u, b: l, r: 0.09 });
                out.push(ik::cloth::Capsule { a: l, b: f, r: 0.07 });
            }
        }
        out
    }

    pub fn hips_joint(&self) -> Entity {
        self.joints[self.bones.hips]
    }
    pub fn parent_of(&self, bone: usize) -> Option<usize> {
        self.rig.parents[bone]
    }

    /// World <- model transform for a root transform.
    fn world_from_model(&self, root: &Transform) -> Mat4 {
        root.to_matrix() * Mat4::from_rotation_translation(model_to_bevy(), Vec3::Y * -self.floor)
    }

    fn neutral_foot(&self, root: &Transform, i: usize) -> Vec3 {
        let side = if i == 0 { 1.0 } else { -1.0 };
        // Model +Y (left) in world.
        let left = root.rotation * (model_to_bevy() * Vec3::Y);
        root.translation + left * 0.12 * side
    }
}

const WALL_DIST: f32 = 0.34;
/// Running faster than this (m/s) and steered elsewhere, the run's direction swings round at this rate (rad/s)
/// keeping its speed (a reversal turns round in about 0.35 s without stopping).
const RUN_TURN_MIN_SPEED: f32 = 2.6;
const RUN_TURN_RATE: f32 = 9.0;
/// The root's height follows the ground averaged this far behind and ahead (m), over steps up to this high.
const STRIDE_SPAN: f32 = 0.35;
const STRIDE_STEP: f32 = 0.4;
/// Seconds to lie dead after a fatal fall before respawning, and health regained per second.
const RESPAWN_TIME: f32 = 3.0;
/// Hands pushed out of walls stop this far clear of them (m).
const ARM_CLEARANCE: f32 = 0.06;
const HEALTH_REGEN: f32 = 0.04;
/// Moving faster than this (m/s) is a sprint: Space at a wall runs up it.
/// Faster than the high profile run (5.2 m/s): free running (the sprint, 6.2 m/s) runs up walls and jumps
/// off edges on its own.
const SPRINT_SPEED: f32 = 5.5;
/// Letting go of the direction above this speed stops with a stop clip (m/s).
const STOP_SPEED: f32 = 2.2;
/// The jog, run and sprint stops' speeds (m/s): a stop between two mixes them.
const SPRINT_STOP_SPEED: f32 = 6.2;
const RUN_STOP_SPEED: f32 = 5.2;
/// The jog stop's speed (m/s, `xx_h_jogstop_<foot>`).
const JOG_STOP_SPEED: f32 = 3.5;
/// Walking at an edge dropping more than this stops at it (m; AC1's ledge stop, not when free running).
const LEDGE_STOP_DROP: f32 = 5.0;
/// Standing at an edge dropping more than this looks down over it (m).
const LOOK_DOWN_DROP: f32 = 2.0;

/// AC1's ground moves, as clip chains with root motion (see the module docs): turning round while running,
/// the run and sprint stops, turns on the spot, the ledge stop (with the outward direction to hold back
/// from), and leaning on a wall run into.
fn ground_move(
    lib: &mut crate::animation::AnimLib,
    level: &Level,
    tf: &Transform,
    ch: &Character,
    ctl: &Controller,
) -> Option<(crate::climb::WallClimb, Option<Vec3>)> {
    use crate::climb::WallClimb;
    let v = ch.velocity.with_y(0.0);
    let speed = v.length();
    let input = ctl.move_dir.with_y(0.0).normalize_or_zero();
    let wants = input != Vec3::ZERO;
    level.ground(tf.translation, 0.3, 0.3)?;
    let fwd = (tf.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
    let (f, o) = if ch.animator.as_ref().is_some_and(|a| a.lead_left()) { ("footl", "footr") } else { ("footr", "footl") };
    let from = || Some(ch.pose.clone());
    let act = |lib: &mut crate::animation::AnimLib, names: Vec<String>, correct: Vec3, cancel: bool| {
        WallClimb::ground_action(lib, tf, &names, correct, cancel, from())
    };
    if speed > STOP_SPEED {
        let dir = v / speed;
        // Reversing: plant, stop and turn round into a jog.
        // Reversing: AC1's run turn-around (`HumanGround` actions 0x5e/0x5f, `xx_h_runturn180_<foot>`), then on into
        // the jog or walk (0x60/0x61), or into its stand if the stick lets go (0x68/0x69). Its clips carry no root
        // turn: the facing flips as it starts and the body comes round in the clip.
        // (Not in high profile or free running: there, reversing turns the run round and keeps going, as the
        // player wants the momentum; only a slower jog plants and turns.)
        if wants && input.dot(dir) < -0.6 && !ctl.high && !ctl.free_run {
            let gait = "walk";
            let names = vec![format!("xx_h_runturn180_{f}"), format!("xx_h_runturn180_{f}_tr_{gait}_hipm_{f}")];
            let stop = Some(format!("xx_h_runturn180_{f}_tr_h_wait_hipm_{o}"));
            let back = Transform { rotation: Quat::from_rotation_arc(Vec3::NEG_Z, -dir), ..*tf };
            // The fade into it starts from the pose shown, turned with the root (it flips round under the body): else
            // the old pose, now facing the new way, swung the whole body round in one frame. Turned a little short of
            // half round, so the fade comes round about the vertical one way (at exactly half a turn its way round was
            // undefined, and the body tipped over with the legs crossed).
            let shown = ch.rig.find("Reference").map(|r| {
                let mut p = ch.pose.clone();
                p.rotate_model(&ch.rig, r, Quat::from_rotation_z(std::f32::consts::PI - TURN_FLIP_SHORT));
                p
            });
            return WallClimb::ground_action(lib, &back, &names, Vec3::ZERO, false, shown.or_else(from)).map(|w| (w.steered(-dir, stop), None));
        }
        if !wants {
            // AC1 mixes its jog, run and sprint stops by speed (`HumanGround`'s stop item).
            let stops = [(JOG_STOP_SPEED, "jogstop"), (RUN_STOP_SPEED, "runstop"), (SPRINT_STOP_SPEED, "sprintstop")];
            let k = stops.iter().position(|s| s.0 >= speed).unwrap_or(stops.len() - 1).max(1);
            let ((sa, a), (sb, b)) = (stops[k - 1], stops[k]);
            let w = ((speed - sa) / (sb - sa)).clamp(0.0, 1.0);
            let stop = crate::animation::mix_name(&[(&format!("xx_h_{a}_{f}"), 1.0 - w), (&format!("xx_h_{b}_{f}"), w)]);
            let mut names = vec![stop];
            if (a == "runstop" && w < 0.5) || (b == "runstop" && w >= 0.5) {
                names.push(format!("xx_h_runstop_{f}_tr_h_wait_hipm_{o}"));
            }
            return act(lib, names, Vec3::ZERO, true).map(|w| (w, None));
        }
        // Running into a tall wall (not free running): hands on it, lean there.
        if !ctl.free_run
            && let Some(w) = WallClimb::lean(lib, level, tf, dir, from())
        {
            return Some((w, None));
        }
    }
    // Walking at the edge of a big drop (not free running): stop at it.
    if speed > 1.0 && wants && !ctl.free_run && ch.edge_lock.is_none() {
        let dir = v / speed;
        if let Some(d) = (1..=12).map(|k| k as f32 * 0.05).find(|&d| level.ground(tf.translation + dir * d, 0.3, LEDGE_STOP_DROP).is_none()) {
            let names = vec!["xx_h_ledge_stop_start_footl".to_string(), "xx_h_ledge_stop_end_footl".into(), "xx_h_ledge_stop_end_tr_h_wait_footr".into()];
            let at = Transform { rotation: Quat::from_rotation_arc(Vec3::NEG_Z, dir), ..*tf };
            let travel = WallClimb::travel(lib, &at, &names)?;
            let correct = (tf.translation + dir * (d - 0.35) - (tf.translation + travel)).with_y(0.0).clamp_length_max(1.0);
            debug!("climb: ledge stop {d:.2} m from the edge");
            return WallClimb::ground_action(lib, &at, &names, correct, false, from()).map(|w| (w, Some(dir)));
        }
    }
    // Standing, steered away from the facing: turn on the spot, a quarter or a half turn to that side (AC1's
    // `waitturn_<left|right>_<090|180>`, low or high profile), then on into the walk or jog (or, the stick let go
    // during the turn, its stand).
    if speed < 0.3 && wants && !ch.blend {
        let p = if ctl.high { "h" } else { "l" };
        let angle = fwd.cross(input).y.atan2(fwd.dot(input));
        let side = if angle > 0.0 { "left" } else { "right" };
        let turn = match angle.abs() {
            a if a < TURN_MIN => return None,
            a if a < TURN_HALF => format!("{side}_090"),
            _ => format!("{side}_180"),
        };
        let foot = if ch.animator.as_ref().is_none_or(|a| a.lead_left()) { "footl" } else { "footr" };
        let (names, stop) = turn_names(lib, p, &turn, foot)?;
        return act(lib, names, Vec3::ZERO, false).map(|w| (w.steered(input, stop), None));
    }
    None
}

/// Standing, steering this far off the facing (radians) turns on the spot; past `TURN_HALF`, a half turn.
const TURN_MIN: f32 = 0.8;
const TURN_HALF: f32 = 2.4;

/// A turn on the spot from standing on `foot` (`footl`/`footr`): `xx_<p>_wait_hipm_<foot>_to_<p>_waitturn_<turn>_<foot>`
/// and its exit into the walk (low profile) or jog (high), plus the exit into its stand
/// (`xx_<p>_waitturn_<turn>_<foot>_tr_<p>_wait_hipm_<foot>`, whichever foot it ends on) for letting go of the stick.
fn turn_names(lib: &crate::animation::AnimLib, p: &str, turn: &str, foot: &str) -> Option<(Vec<String>, Option<String>)> {
    let into = format!("xx_{p}_wait_hipm_{foot}_to_{p}_waitturn_{turn}_{foot}");
    if !lib.names.contains(&into) {
        return None;
    }
    // Only the turn itself: AC1's move graph goes from it straight into the walk, jog, run or sprint (the
    // `_tr_<gait>` exit that walks off forward is one of its options, not a must), so the gait the player is
    // asking for takes over as it ends (`Animator::resume_gait`).
    let stand = format!("xx_{p}_waitturn_{turn}_{foot}_tr_{p}_wait_hipm_");
    let stop = lib.names.iter().find(|n| n.starts_with(&stand)).cloned();
    Some((vec![into], stop))
}

/// Test hook (`AC1_EMBED_CHECK`): warn when the body at `root` is inside geometry (solid just above the
/// chest with a face right beside it).
fn embed_check(level: &Level, root: Vec3, what: &str) {
    if std::env::var("AC1_EMBED_CHECK").is_err() {
        return;
    }
    let chest = root + Vec3::Y * 1.0;
    for d in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
        if level.raycast(chest + d * 0.05, Vec3::Y, 0.6).is_some() && level.raycast(chest, d, 0.12).is_some() {
            warn!("EMBED {what} at {root:.2}");
            return;
        }
    }
}

/// A ragdoll still this long (s) stops being simulated.
const RAGDOLL_SETTLE: f32 = 2.0;

/// Keep a ragdoll particle (radius `r`, moving `prev` -> `p`) out of the world: on the ground under it and
/// out of walls it would pass into. Returns where it may be and whether it touches something.
fn ragdoll_collide(level: &Level, prev: Vec3, p: Vec3, r: f32) -> (Vec3, bool) {
    let mut p = p;
    let mut touch = false;
    let d = p - prev;
    if d.length() > 1e-5
        && let Some(hit) = level.raycast(prev, d.normalize(), d.length() + r)
    {
        p = hit.point + hit.normal * r - d.normalize() * 0.0;
        touch = true;
    }
    if let Some(g) = level.ground(p, r + 0.3, r + 0.5)
        && p.y < g.point.y + r
    {
        p.y = g.point.y + r;
        touch = true;
    }
    (p, touch)
}

/// A palm on a wall: the wrist this far out from it (m).
const PALM_OFF: f32 = 0.05;
/// A turn round's fade starts from the shown pose turned this much short of half round (rad).
const TURN_FLIP_SHORT: f32 = 0.08;
/// The climbing IK fades in or out over this long (s) as the hands take to the wall or leave it.
const WALL_IK_FADE: f32 = 0.2;
/// Ground foot placement fades in or out over this long (s) as moves off the ground start and end.
const FOOT_IK_FADE: f32 = 0.2;
/// Changing profile while standing (slower than this, m/s) fades between the stands over this long (s).
const STAND_FADE_SPEED: f32 = 0.5;
const STAND_FADE: f32 = 0.3;
/// Climbing feet follow the wall's distance at this rate (1/s).
const WALL_FOOT_RATE: f32 = 12.0;
/// The capsule moves in steps no longer than this (m).
const CAPSULE_STEP: f32 = 0.15;

/// The speed value starts over (from standing) when it asks for this much more speed (m/s) than the body has.
const GAIT_RESYNC: f32 = 1.5;
/// The ground this far under the feet (m) is still stood on, walking down a step or a slope (AC1's stick-to-ground
/// cast, 0.58 m); further down is a fall.
const STICK_TO_GROUND: f32 = 0.58;

/// Free running grabs or runs up a wall this close ahead (m).
const FREE_RUN_REACH: f32 = 1.3;
/// Running at least this fast vaults low obstacles on its own.
const RUN_VAULT_SPEED: f32 = 2.5;
const HANG_DROP: f32 = 1.75;

/// Movement: ground walking or climbing.
pub fn locomotion(
    time: Res<Time>,
    level: Res<Level>,
    mut lib: Option<ResMut<crate::animation::AnimLib>>,
    mut q: Query<(&mut Transform, &mut Character, &mut Controller), Without<Statue>>,
) {
    let dt = time.delta_secs().min(0.05);
    for (mut tf, mut ch, mut ctl) in &mut q {
        let ch = &mut *ch;
        if ch.health > 0.0 {
            ch.health = (ch.health + HEALTH_REGEN * dt).min(1.0);
        }
        // Limp (the ragdoll): the root goes with the body, on the ground under its hips.
        if let Some((r, _)) = &ch.ragdoll {
            let hips = r.root();
            let floor = level.ground(hips, 0.2, 3.0).map_or(hips.y - 0.9, |g| g.point.y);
            tf.translation = hips.with_y(floor);
            ch.velocity = Vec3::ZERO;
            if ch.limp {
                continue;
            }
        }
        // Free running into a wall: run up it or grab on, as if the legs button were pressed.
        if ctl.free_run && ch.wall.is_none() {
            let v = ch.velocity.with_y(0.0);
            let dir = ctl.move_dir.with_y(0.0).normalize_or_zero();
            if v.length() > 0.5
                && dir != Vec3::ZERO
                && level
                    .raycast(tf.translation + Vec3::Y, dir, FREE_RUN_REACH)
                    .is_some_and(|h| h.normal.y.abs() < 0.3 && crate::climb::wide_wall(&level, tf.translation + Vec3::Y, dir, h.dist))
            {
                ctl.toggle_climb = true;
                ctl.grab_only = true;
            }
        }
        // A gentle push (low profile empty hand), played over the walk.
        // Pickpocketing (`xx_pickpocket_attempt_walk_<foot>`, `_success_finish_<foot>`), a short ground move.
        if std::mem::take(&mut ctl.pickpocket)
            && ch.wall.is_none()
            && let Some(lib) = lib.as_deref_mut()
        {
            let names = vec!["xx_pickpocket_attempt_walk_footl".to_string(), "xx_pickpocket_attempt_success_finish_footl".to_string()];
            if let Some(w) = crate::climb::WallClimb::ground_action(lib, &tf, &names, Vec3::ZERO, true, Some(ch.pose.clone())) {
                debug!("crowd: pickpocket");
                ch.wall = Some(w);
                ch.velocity = Vec3::ZERO;
                ctl.push = false;
                continue;
            }
        }
        if std::mem::take(&mut ctl.push)
            && ch.wall.is_none()
            && let (Some(lib), Some(a)) = (lib.as_deref_mut(), ch.animator.as_mut())
            && a.oneshot.is_none()
        {
            a.oneshot = lib.get(if ch.velocity.length() > 0.2 { "xx_l_push_attemp_righthand" } else { "xx_l_push_attemp_lefthand" }).map(|c| (c, 0.0));
        }
        // Walking with the crowd (legs held, no direction).
        if let Some(v) = ch.follow.take()
            && ctl.move_dir.length() < 0.1
        {
            ctl.move_dir = v.normalize_or_zero();
            ctl.speed = v.length();
        }
        // AC1 clip-driven climbing.
        if let (Some(lib), true) = (lib.as_deref_mut(), ch.animator.is_some()) {
            if let Some(w) = &mut ch.wall {
                w.move_dir = ctl.move_dir.with_y(0.0).clamp_length_max(1.0);
                w.sprint = ctl.speed > SPRINT_SPEED;
                w.high = ctl.high;
            }
            // Steering out of a run stop, or the legs during a ground action: back to moving.
            // (A turn round follows the stick wherever it goes, bending the turn toward it (`WallClimb::update`);
            // letting go mid-turn, before its exit plays, ends it in its stand instead.)
            if ctl.move_dir.length() < 0.1
                && let Some(w) = ch.wall.as_mut()
            {
                w.release_steer(lib);
            }
            let off_course = |_: Vec3| ctl.move_dir.length() < 0.1 || ctl.toggle_climb;
            if ch
                .wall
                .as_ref()
                .is_some_and(|w| (w.cancel && ctl.move_dir.length() > 0.1) || (w.on_ground() && ctl.toggle_climb) || w.steer.is_some_and(off_course))
            {
                ch.exit_fade = Some((ch.anim_pose.clone(), 0.2, 0.2));
                ch.wall = None;
            }
            if std::mem::take(&mut ctl.wall_legs)
                && let Some(w) = &mut ch.wall
            {
                w.legs(lib, &level, &tf, ctl.climb_dir);
            }
            let grab_only = std::mem::take(&mut ctl.grab_only);
            let needs_edge = std::mem::take(&mut ctl.jump_needs_edge);
            if std::mem::take(&mut ctl.toggle_climb) {
                match ch.wall.as_mut().map(|w| w.let_go(lib, &level, &tf, ctl.climb_dir)) {
                    // Letting go plays out in the climber (step down, or drop, fall and land).
                    Some(true) => {}
                    Some(false) => {
                        // No drop clips: push off the wall and fall.
                        let w = ch.wall.take().expect("climbing");
                        tf.translation += w.normal * 0.3;
                        ch.fall_v = 0.0;
                        ch.exit_fade = w.last_pose().cloned().map(|p| (p, 0.25, 0.25));
                    }
                    None => {
                        use crate::climb::WallClimb;
                        let v = ch.velocity.with_y(0.0);
                        let pose = || Some(ch.pose.clone());
                        let wall_ahead = |dir: Vec3, reach: f32| level.raycast(tf.translation + Vec3::Y, dir, reach).is_some_and(|h| h.normal.y.abs() < 0.3);
                        // At a high edge over a haystack: leap of faith. Sprinting at a wall: run up it. Else
                        // climb on from the ground, else a running jump (unless a wall is right ahead), else a
                        // standing jump up to a ledge, else straight up.
                        let facing = if v.length() > 0.5 { v } else { tf.rotation * Vec3::NEG_Z };
                        // (Hiding on a bench or in hay from the ground is low profile's; in high profile the legs free
                        // run past them.)
                        ch.wall = (!ctl.high)
                            .then(|| WallClimb::sit(lib, &level, &tf, pose()))
                            .flatten()
                            .or_else(|| WallClimb::into_hay(lib, &level, &tf, ctl.high, pose()))
                            .or_else(|| WallClimb::leap_of_faith(lib, &level, &tf, facing, pose()));
                        // A ladder's foot or top: onto it (before the edge it stands at).
                        ch.wall = ch.wall.take().or_else(|| WallClimb::ladder(lib, &level, &tf, &ch.rig, &ch.base, ch.climb_rig, pose()));
                        // At an edge in low profile, or standing there in high profile: lower onto it and hang.
                        if ch.wall.is_none() && (!ctl.high || v.length() < 0.5) {
                            let looking = ch.animator.as_ref().is_some_and(|a| a.idle_alt.is_some());
                            ch.wall = WallClimb::pull_down(lib, &level, &tf, &ch.rig, &ch.base, ch.climb_rig, looking, pose());
                        }
                        if ch.wall.is_none() && v.length() > 0.5 {
                            ch.wall = WallClimb::vault(lib, &level, &tf, v, v.length(), pose());
                        }
                        ch.wall = ch.wall.take().or_else(|| WallClimb::monkey_bars(lib, &level, &tf, pose()));
                        ch.wall = ch.wall.take().or_else(|| {
                            (v.length() > SPRINT_SPEED && wall_ahead(v.normalize(), crate::climb::WALL_RUN_REACH))
                                .then(|| WallClimb::wall_run(lib, &level, &tf, &ch.rig, &ch.base, ch.climb_rig, v, pose()))
                                .flatten()
                                .or_else(|| (v.length() > SPRINT_SPEED).then(|| WallClimb::side_run(lib, &level, &tf, v, pose())).flatten())
                                .or_else(|| WallClimb::try_enter(lib, &level, &tf, &ch.rig, &ch.base, ch.climb_rig, pose()))
                        });
                        let edge_near = || level.ground(tf.translation + v.normalize_or_zero() * 1.2, 0.5, 1.5).is_none();
                        if ch.wall.is_none() && !grab_only && v.length() > 1.0 && !wall_ahead(v.normalize(), 1.2) && (!needs_edge || edge_near()) {
                            // (Off the foot the run is on: AC1's takeoffs are by foot.)
                            let lead_left = ch.animator.as_ref().is_none_or(|a| a.lead_left());
                            ch.wall = WallClimb::jump_aimed(lib, &level, &tf, &ch.rig, &ch.base, ch.climb_rig, v, lead_left, pose());
                        } else if ch.wall.is_none() {
                            ch.wall = WallClimb::jump_grab(lib, &level, &tf, &ch.rig, &ch.base, ch.climb_rig, pose())
                                .or_else(|| (!grab_only && !needs_edge).then(|| WallClimb::jump_straight(lib, &tf, pose())).flatten());
                        }
                    }
                }
            }
            if let Some(l) = ch.wall.as_mut().and_then(|w| w.landed.take()) {
                ch.last_landing = Some(l);
            }
            if let Some(w) = &ch.wall {
                embed_check(&level, tf.translation, &w.state);
            }
            if let Some(w) = &mut ch.wall {
                w.health = ch.health;
                w.update(lib, &level, &mut tf, &ch.rig, &ch.base, ch.climb_rig, ctl.climb_dir, ctl.leap, dt);
                ch.health = (ch.health - std::mem::take(&mut w.damage)).max(0.0);
                ch.velocity = Vec3::ZERO;
                if w.dead {
                    // Desynchronised: lie there, then start again at the spawn point.
                    ch.dead_time += dt;
                    if ch.dead_time > RESPAWN_TIME {
                        ch.wall = None;
                        ch.dead_time = 0.0;
                        ch.health = 1.0;
                        ch.fall_v = 0.0;
                        tf.translation = ch.spawn;
                        if let Some(g) = level.ground(ch.spawn, 50.0, 60.0) {
                            tf.translation.y = g.point.y;
                        }
                    }
                    continue;
                }
                if w.finished {
                    // Back on our feet (on top, or landed): ground locomotion, fading from the last climb pose,
                    // picking up on the foot (or into the gait) the last clip ends on.
                    ch.exit_fade = w.last_pose().cloned().map(|p| (p, 0.25, 0.25));
                    ch.velocity = w.exit_velocity;
                    // (A free-step reception, `..._tr_freestep_entry_<foot>`: the next jump takes off from that foot.)
                    if let Some(name) = w.last_clip.as_deref() {
                        if name.contains("freestep_entry_footr") {
                            ch.freestep_left = false;
                        } else if name.contains("freestep_entry_footl") {
                            ch.freestep_left = true;
                        }
                    }
                    if let (Some(a), Some(name)) = (ch.animator.as_mut(), w.last_clip.as_deref()) {
                        // (Into a gait: the step and the point in it whose legs match the pose he is in, going on at
                        // its speed the way he faces.)
                        let legs: Vec<usize> = ch.bones.legs.iter().flat_map(|l| [l.upper, l.lower, l.foot]).collect();
                        let pose = w.last_pose().unwrap_or(&ch.pose);
                        // (Out of a turn on the spot with the stick held: straight into the gait asked for.)
                        let speed = if name.contains("_to_") && name.contains("_waitturn_") && ctl.move_dir.length() > 0.1 {
                            let foot = if name.ends_with("footl") { 0 } else { 1 };
                            Some(a.resume_gait(ctl.speed, Some(foot), pose, &ch.base, &legs).min(ctl.speed))
                        } else {
                            a.resume_matching(name, pose, &ch.base, &legs)
                        };
                        if let Some(speed) = speed
                            && ch.velocity.length() < 0.1
                        {
                            ch.velocity = (tf.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero() * speed;
                        }
                    }
                    ch.wall = None;
                    ch.fall_v = 0.0;
                }
                continue;
            }
        }
        if std::mem::take(&mut ctl.toggle_climb) {
            match &mut ch.climb {
                Some(c) => c.leaving = true,
                None => ch.climb = try_grab(&level, &tf),
            }
        }

        if let Some(c) = &mut ch.climb {
            climb_step(c, &level, ctl.climb_dir, dt);
            // Body hangs below the grips, off the wall.
            let centre = (c.grips[0] + c.grips[1]) * 0.5;
            let want = centre + c.ledge_normal * WALL_DIST - Vec3::Y * HANG_DROP;
            let k = 1.0 - (-8.0 * dt).exp();
            tf.translation = tf.translation.lerp(want, k);
            let face = Quat::from_rotation_arc(Vec3::NEG_Z, Vec3::new(-c.ledge_normal.x, 0.0, -c.ledge_normal.z).normalize());
            tf.rotation = tf.rotation.slerp(face, k);
            c.weight = if c.leaving { (c.weight - dt * 3.0).max(0.0) } else { (c.weight + dt * 3.0).min(1.0) };
            if c.leaving && c.weight <= 0.0 {
                // Drop: step back off the wall.
                let n = c.ledge_normal;
                tf.translation += n * 0.25;
                ch.climb = None;
            }
            ch.velocity = Vec3::ZERO;
            continue;
        }

        // The player's speed: AC1's speed value (`gait`). After a climb, a vault or a wall stopping him it picks up
        // from the speed he has, not the one he had: held down to that (not back to the start of the jog, which pinned it
        // there until the body caught up). Landings hand on the speed they land with.
        if let Some(stick) = ctl.stick {
            let have = ch.velocity.with_y(0.0).length();
            if crate::gait::speed(ch.gait.value) > have + GAIT_RESYNC {
                // (Not under where a start from standing goes, the jog in high profile.)
                let start = if ctl.high || ctl.free_run { crate::gait::BAND_JOG } else { crate::gait::BAND_WALK };
                ch.gait.value = crate::gait::value_at(have + GAIT_RESYNC).max(start).min(ch.gait.value);
            }
            let facing = (tf.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
            let off = if ctl.move_dir.length() > 0.01 { facing.angle_between(ctl.move_dir.with_y(0.0)) } else { 0.0 };
            ctl.speed = ch.gait.update(stick, off, ctl.high, ctl.free_run, dt);
        }
        // Blending into a crowd: walk at its pace.
        if ch.blend && ctl.speed <= 2.0 {
            ctl.speed = ctl.speed.min(crate::crowd::BLEND_SPEED);
        }
        // AC1's ground moves: run stops, turning round, turns on the spot, ledge stops, leaning on walls.
        if ch.fall_v == 0.0
            && !ctl.blend_walk
            && let (Some(lib), true) = (lib.as_deref_mut(), ch.animator.is_some())
            && let Some((w, lock)) = ground_move(lib, &level, &tf, ch, &ctl)
        {
            // Facing as the move starts, this frame: it is posed this frame (a turn round flips the root under the body,
            // and a frame with the new pose on the old facing showed the body swung round).
            if let Some(r) = w.start_facing() {
                tf.rotation = r;
            }
            ch.wall = Some(w);
            ch.edge_lock = lock.or(ch.edge_lock);
            ch.velocity = Vec3::ZERO;
            continue;
        }
        // Looking down over an edge while standing at it.
        let edge_ahead =
            |d: f32, drop: f32| level.ground(tf.translation + (tf.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero() * d, 0.3, drop).is_none();
        let look_down = ch.velocity.length() < 0.2 && edge_ahead(0.6, LOOK_DOWN_DROP);
        if let (Some(lib), Some(a)) = (lib.as_deref_mut(), ch.animator.as_mut()) {
            // Standing, the low and high profile stands put the feet and hands elsewhere: fade between them (switched at
            // once, the whole body jumped up to 13 cm as the right button went down or up).
            if a.high != ctl.high && ch.velocity.length() < STAND_FADE_SPEED {
                ch.exit_fade = Some((ch.anim_pose.clone(), STAND_FADE, STAND_FADE));
            }
            a.high = ctl.high;
            // (Keep the foot it started on: the walk's half-cycles go on alternating while standing.)
            let want = match &a.idle_alt {
                Some(c) if look_down => Some(c.clone()),
                _ => look_down.then(|| lib.get(if a.lead_left() { "xx_l_ledge_lookdown_front_footl" } else { "xx_l_ledge_lookdown_front_footr" })).flatten(),
            };
            if want.as_ref().map(|c| &c.name) != a.idle_alt.as_ref().map(|c| &c.name) {
                ch.exit_fade = Some((ch.anim_pose.clone(), 0.4, 0.4));
                a.idle_alt = want;
            }
        }
        // Ground: accelerate toward desired velocity, turn to face it, follow the ground height.
        let mut target_v = ctl.move_dir.normalize_or_zero() * ctl.speed;
        // After a ledge stop, hold back from the edge until steering away or free running.
        if let Some(n) = ch.edge_lock {
            if ctl.free_run || ctl.move_dir.normalize_or_zero().dot(n) < 0.3 {
                ch.edge_lock = None;
            } else {
                target_v -= n * target_v.dot(n).max(0.0);
            }
        }
        let k = 1.0 - (-6.0 * dt).exp();
        let (cur, want) = (ch.velocity.with_y(0.0), target_v.with_y(0.0));
        if cur.length() > RUN_TURN_MIN_SPEED && want.length() > RUN_TURN_MIN_SPEED {
            // Running, steered elsewhere (even right round): the run swings round at a turn rate, keeping its speed,
            // rather than easing through a stop.
            let (a, b) = (cur.normalize(), want.normalize());
            let angle = a.angle_between(b);
            let step = (RUN_TURN_RATE * dt).min(angle);
            let axis = if a.cross(b).y >= 0.0 { Vec3::Y } else { Vec3::NEG_Y };
            let dir = if angle > 1e-4 { Quat::from_axis_angle(axis, step) * a } else { b };
            let speed = cur.length() + (want.length() - cur.length()) * k;
            ch.velocity = dir * speed + Vec3::Y * ch.velocity.y;
        } else {
            ch.velocity = ch.velocity.lerp(target_v, k);
        }
        let v = ch.velocity;
        if v.length() > 0.2 {
            let face = Quat::from_rotation_arc(Vec3::NEG_Z, Vec3::new(v.x, 0.0, v.z).normalize());
            tf.rotation = tf.rotation.slerp(face, 1.0 - (-8.0 * dt).exp());
        }
        // Running: vault a low obstacle ahead, and jump off an edge (AC1's free running).
        if v.length() > RUN_VAULT_SPEED
            && let (Some(lib), true) = (lib.as_deref_mut(), ch.animator.is_some())
        {
            use crate::climb::WallClimb;
            let dir = v.normalize();
            let on_ground = level.ground(tf.translation, 0.3, 0.3).is_some();
            // Sprinting (or free running at any speed, as AC1 does) off an edge jumps. Free running along a top that ends in a drop of any size (a fence, a low
            // wall run onto) with something to jump to ahead springs on to it straight away (AC1's two-step: onto the
            // fence, on to the post).
            let edge = on_ground
                && (v.length() > SPRINT_SPEED || ctl.free_run)
                && (level.ground(tf.translation + dir * 0.6, 0.8, 1.2).is_none()
                    || (ctl.free_run
                        && level.ground(tf.translation + dir * 0.6, 0.3, 0.4).is_none()
                        && WallClimb::jump_target_ahead(&level, tf.translation, dir)));
            // Free running steps or jumps onto a low obstacle; otherwise running stops against it or glances off.
            // (Toward where the stick points: sliding along a wall, the body's own velocity runs along it.)
            let toward = target_v.with_y(0.0).try_normalize().map_or(v, |d| d * v.length());
            let lead_left = ch.animator.as_ref().is_none_or(|a| a.lead_left());
            let low = if ctl.free_run {
                WallClimb::vault(lib, &level, &tf, toward, v.length(), Some(ch.pose.clone()))
            } else {
                WallClimb::collide(lib, &level, &tf, &ch.rig, &ch.base, ch.climb_rig, toward, Some(ch.pose.clone()))
            };
            ch.wall = low.or_else(|| edge.then(|| WallClimb::leap_of_faith(lib, &level, &tf, v, Some(ch.pose.clone()))).flatten()).or_else(|| {
                edge.then(|| WallClimb::jump_aimed(lib, &level, &tf, &ch.rig, &ch.base, ch.climb_rig, v, lead_left, Some(ch.pose.clone()))).flatten()
            });
            if ch.wall.is_some() {
                ch.velocity = Vec3::ZERO;
                continue;
            }
        }
        // Move with the body's capsule (AC1's character proxy, `Level::capsule_push`): out of walls, sliding along
        // them, in steps short enough not to pass through a thin one.
        let steps = ((v.with_y(0.0).length() * dt) / CAPSULE_STEP).ceil().max(1.0) as usize;
        let mut pushed = Vec3::ZERO;
        for _ in 0..steps {
            let was = tf.translation;
            tf.translation += v.with_y(0.0) * (dt / steps as f32);
            let out = level.capsule_push(tf.translation);
            tf.translation += out;
            pushed += out;
            // (Not under a ceiling lower than the body: the step is a wall.)
            if level.ceiling(tf.translation, 0.0) && !level.ceiling(was, 0.0) {
                pushed += was - tf.translation;
                tf.translation = was;
            }
        }
        // Into a wall: the speed into it stops, the rest slides on.
        if let Some(n) = pushed.try_normalize() {
            let into = ch.velocity.dot(n);
            if into < 0.0 {
                ch.velocity -= n * into;
            }
        }
        embed_check(&level, tf.translation, "ground");
        // (The fall starts at the edge line, the feet past it: AC1's ground loss, `HumanGround__CheckGroundLoss`, not the
        // capsule sliding off the rim.)
        if let Some(g) = level.ground(tf.translation, 0.6, 20.0) {
            if tf.translation.y > g.point.y + 0.6
                && ch.fall_v == 0.0
                && let (Some(lib), true) = (lib.as_deref_mut(), ch.animator.is_some())
            {
                // Off a ledge: an animated fall (lands, catches holds, or drops into hay).
                ch.wall = crate::climb::WallClimb::falling(lib, &tf, ch.velocity, Some(ch.pose.clone()));
                if ch.wall.is_some() {
                    ch.velocity = Vec3::ZERO;
                    continue;
                }
            }
            // Stepped onto a post, or onto a beam away from its ends: balance on it.
            if ch.fall_v == 0.0
                && level.perch_inside(tf.translation, crate::climb::PERCH_REACH).is_some()
                && let (Some(lib), true) = (lib.as_deref_mut(), ch.animator.is_some())
            {
                ch.wall = crate::climb::WallClimb::perch(lib, &level, &tf, Some(ch.pose.clone()));
                if let Some(w) = &mut ch.wall {
                    w.freestep_left = ch.freestep_left;
                    ch.velocity = Vec3::ZERO;
                    continue;
                }
            }
            // (Down a step of up to `STICK_TO_GROUND` the feet stay on the ground, as AC1's proxy casts down for it.)
            if tf.translation.y > g.point.y + STICK_TO_GROUND || ch.fall_v < 0.0 {
                // Airborne: fall until we reach the ground.
                ch.fall_v -= 9.81 * dt;
                tf.translation.y = (tf.translation.y + ch.fall_v * dt).max(g.point.y);
                if tf.translation.y <= g.point.y {
                    ch.fall_v = 0.0;
                }
            } else {
                // Follow the ground averaged over a stride along the way it moves (stairs become a steady climb,
                // the feet finding each step with IK); heights more than `STRIDE_STEP` off are left out (an edge
                // or a wall is not ground to average). Up a little faster than down (a slope must not swallow the
                // feet), and never far below the ground underfoot.
                let along = if v.length() > 0.3 { v.normalize() } else { (tf.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero() };
                let heights: Vec<f32> = [-STRIDE_SPAN, STRIDE_SPAN]
                    .iter()
                    .filter_map(|&d| level.ground(tf.translation + along * d, 0.5, 0.6).map(|h| h.point.y))
                    .filter(|y| (y - g.point.y).abs() < STRIDE_STEP)
                    .chain([g.point.y])
                    .collect();
                let want = heights.iter().sum::<f32>() / heights.len() as f32;
                let rate = if want > tf.translation.y { 16.0 } else { 10.0 };
                tf.translation.y += (want - tf.translation.y) * (1.0 - (-rate * dt).exp());
                tf.translation.y = tf.translation.y.max(g.point.y - STRIDE_STEP * 0.6);
            }
        }
    }
}

fn try_grab(level: &Level, tf: &Transform) -> Option<Climb> {
    let fwd = tf.rotation * Vec3::NEG_Z;
    let hit = level.raycast(tf.translation + Vec3::Y * 1.2, fwd, 1.2)?;
    if hit.normal.y.abs() > 0.3 {
        return None;
    }
    let reach_min = tf.translation.y + 1.3;
    let ledge = level.ledges.iter().filter(|l| l.out.dot(hit.normal) > 0.7 && l.a.y >= reach_min && l.a.y <= reach_min + 1.2).min_by(|a, b| {
        let da = (a.closest(hit.point) - hit.point).length();
        let db = (b.closest(hit.point) - hit.point).length();
        da.total_cmp(&db)
    })?;
    let centre = ledge.closest(hit.point);
    let along = (ledge.b - ledge.a).normalize();
    let grips = [ledge.closest(centre - along * 0.22), ledge.closest(centre + along * 0.22)];
    // Left hand is on the character's left: the side of -along when facing the wall.
    let left = tf.rotation * (model_to_bevy() * Vec3::Y);
    let grips = if (grips[0] - centre).dot(left) > 0.0 { grips } else { [grips[1], grips[0]] };
    Some(Climb { ledge_normal: ledge.out, grips, moving: None, weight: 0.0, leaving: false })
}

fn ledge_at(level: &Level, near: Vec3, dy: f32, normal: Vec3) -> Option<(Ledge, Vec3)> {
    level
        .ledges
        .iter()
        .filter(|l| l.out.dot(normal) > 0.7 && (l.a.y - (near.y + dy)).abs() < 0.2)
        .map(|l| (*l, l.closest(near)))
        .filter(|(_, p)| (Vec2::new(p.x, p.z) - Vec2::new(near.x, near.z)).length() < 0.35)
        .min_by(|a, b| (a.1 - near).length().total_cmp(&(b.1 - near).length()))
}

/// Hand-over-hand movement between ledges.
fn climb_step(c: &mut Climb, level: &Level, dir: Vec2, dt: f32) {
    if let Some(m) = &mut c.moving {
        m.t += dt / 0.38;
        if m.t >= 1.0 {
            c.grips[m.hand] = m.to;
            c.moving = None;
        }
        return;
    }
    if c.leaving {
        return;
    }
    // A hand that lags a level behind the other catches up first.
    let dy = c.grips[0].y - c.grips[1].y;
    if dy.abs() > 0.3 {
        let lag = if dy > 0.0 { 1 } else { 0 };
        let lead = 1 - lag;
        let along = (c.grips[lag] - c.grips[lead]).with_y(0.0).normalize_or_zero();
        if let Some((_, p)) = ledge_at(level, c.grips[lead] + along * 0.44, 0.0, c.ledge_normal) {
            c.moving = Some(HandMove { hand: lag, from: c.grips[lag], to: p, t: 0.0 });
        }
        return;
    }
    if dir.length() < 0.2 {
        return;
    }
    let right = c.ledge_normal.cross(Vec3::Y).normalize() * -1.0;
    if dir.y.abs() >= dir.x.abs() {
        // Up/down: the hand on the side we lean toward reaches first; alternate otherwise.
        let hand = if dir.x > 0.1 {
            1
        } else if dir.x < -0.1 || c.grips[0].y <= c.grips[1].y {
            0
        } else {
            1
        };
        let dy = 0.62 * dir.y.signum();
        if let Some((_, p)) = ledge_at(level, c.grips[hand], dy, c.ledge_normal) {
            c.moving = Some(HandMove { hand, from: c.grips[hand], to: p, t: 0.0 });
        }
    } else {
        // Shimmy: lead hand moves out, trailing hand follows (via the catch-up rule on widths).
        let s = dir.x.signum();
        let lead = if s > 0.0 { 1 } else { 0 };
        let to = c.grips[lead] + right * s * 0.3;
        if let Some((_, p)) = ledge_at(level, to, 0.0, c.ledge_normal)
            && (p - c.grips[lead]).length() > 0.05
        {
            c.moving = Some(HandMove { hand: lead, from: c.grips[lead], to: p, t: 0.0 });
        } else {
            let trail = 1 - lead;
            let gap = (c.grips[lead] - c.grips[trail]).length();
            if gap > 0.5 {
                let p = c.grips[trail] + (c.grips[lead] - c.grips[trail]).normalize() * (gap - 0.44);
                if let Some((_, p)) = ledge_at(level, p, 0.0, c.ledge_normal) {
                    c.moving = Some(HandMove { hand: trail, from: c.grips[trail], to: p, t: 0.0 });
                }
            }
        }
    }
    // Keep hands from crossing or spreading too far.
    let gap = (c.grips[0] - c.grips[1]).length();
    if gap > 0.8 {
        let trail = if dir.x > 0.0 { 0 } else { 1 };
        let lead = 1 - trail;
        let p = c.grips[lead] + (c.grips[trail] - c.grips[lead]).normalize() * 0.44;
        if let Some((_, p)) = ledge_at(level, p, 0.0, c.ledge_normal) {
            c.moving = Some(HandMove { hand: trail, from: c.grips[trail], to: p, t: 0.0 });
        }
    }
}

/// Build this frame's pose.
#[allow(clippy::too_many_arguments)]
pub fn animate(
    time: Res<Time>,
    level: Res<Level>,
    human_ragdoll: Option<Res<crate::ragdoll::HumanRagdoll>>,
    play: Res<StatuePlay>,
    frame: Res<bevy::diagnostic::FrameCount>,
    cams: Query<&GlobalTransform, With<Camera3d>>,
    mut q: Query<(&Transform, &mut Character, Has<Statue>)>,
    mut joints: Query<&mut Transform, Without<Character>>,
) {
    let dt = time.delta_secs().clamp(1e-4, 0.05);
    let cam = cams.iter().next().map(|g| g.translation());
    for (root, mut ch, statue) in &mut q {
        if statue && statue_still(&play, frame.0, cam, root.translation, STATUE_NEAR) {
            continue;
        }

        let ch = &mut *ch;
        ch.debug_targets.clear();
        let climb_w = ch.climb.as_ref().map_or(0.0, |c| c.weight);
        let speed = ch.velocity.length();
        let fwd_w = root.rotation * Vec3::NEG_Z;

        let climbing = ch.climb.is_some();
        if let Some(a) = &mut ch.animator {
            a.advance(dt, if climbing { 0.0 } else { speed }, root.rotation.to_euler(EulerRot::YXZ).0);
        }
        // Clips drive the body on the ground; climbing still uses procedural IK on the idle clip.
        let animated = ch.animator.is_some();

        // --- Feet: procedural stepping in world space (fallback without animation data).
        if !climbing && !animated {
            step_feet(ch, root, &level, dt, speed);
        }

        let on_wall = ch.wall.is_some();
        let rig = &ch.rig;
        let b = &ch.bones;
        let mut pose = match (&mut ch.wall, &ch.animator) {
            (Some(w), _) => w.pose(&ch.base, ch.climb_rig, &ch.rig, &ch.mirror),
            (None, Some(a)) => a.pose(&ch.base),
            (None, None) => ch.idle.clone(),
        };
        // Clips that move only part of the body, layered (see `layered`): upper-body ones (the praying
        // hands, a push, a shove) over the spine and up, legs-only ones under the hips.
        if !on_wall && let Some(a) = &ch.animator {
            let upper = a.layer_pose(&ch.base).map(|p| (p, 1.0)).into_iter().chain(a.oneshot_pose(&ch.base));
            for (layer, w) in upper {
                layered(&mut pose, &layer, w, rig, &[b.spine[0]]);
            }
            if let Some((layer, w)) = a.lower_pose(&ch.base) {
                layered(&mut pose, &layer, w, rig, &[b.legs[0].upper, b.legs[1].upper]);
            }
        }
        if let Some((from, t, len)) = &mut ch.exit_fade {
            *t -= dt;
            let mut faded = from.clone();
            faded.blend(&pose, 1.0 - (*t / len.max(1e-3)).clamp(0.0, 1.0));
            pose = faded;
            if *t <= 0.0 {
                ch.exit_fade = None;
            }
        }
        ch.anim_pose = pose.clone();
        // Limp: AC1's ragdoll settling the body on what is under it.
        let limp = ch.limp;
        if !limp {
            ch.ragdoll = None;
        }
        let (_, model_rot, model_pos) = ch.world_from_model(root).to_scale_rotation_translation();
        if limp
            && ch.ragdoll.is_none()
            && let Some(h) = &human_ragdoll
        {
            let def = ch.ragdoll_def.get_or_insert_with(|| std::sync::Arc::new(crate::ragdoll::def_for(h, &ch.rig))).clone();
            let r = ik::ragdoll::Ragdoll::new(&def, &ch.rig, &pose, model_pos, model_rot, ch.velocity, dt);
            debug!("character: went limp at {model_pos:.2}");
            ch.ragdoll = Some((r, pose.clone()));
        }
        if let (Some((r, start)), Some(def)) = (&mut ch.ragdoll, &ch.ragdoll_def) {
            if r.still < RAGDOLL_SETTLE {
                r.step(dt, Vec3::NEG_Y * 9.81, |prev, p, rad| ragdoll_collide(&level, prev, p, rad));
                if r.still >= RAGDOLL_SETTLE {
                    debug!("character: settled limp, hips at {:.2}", r.root());
                }
                if std::env::var("AC1_RAGDOLL_LOG").is_ok() {
                    let (lo, hi) = r.pos.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
                    debug!("ragdoll: root {:.2} y {lo:.2}..{hi:.2} still {:.2} model {:.2}", r.root(), r.still, model_pos);
                }
            }
            let mut limp_pose = start.clone();
            r.apply(def, &ch.rig, &mut limp_pose, model_pos, model_rot);
            pose = limp_pose;
            ch.debug_targets.extend(r.pos.iter().map(|p| (*p, Color::srgb(0.9, 0.2, 0.9))));
        }
        let ik_on = ch.ik_enabled && ch.ragdoll.is_none();
        let world_from_model = ch.world_from_model(root);
        let model_from_world = world_from_model.inverse();
        let to_model = |p: Vec3| model_from_world.transform_point3(p);
        let dir_to_model = |d: Vec3| model_from_world.transform_vector3(d);

        // --- Side wall run: the run cycle's feet step onto the wall while they are down, and the
        // wall-side hand reaches for it.
        if let Some((wall, n, u, w)) = ch.wall.as_ref().and_then(|wc| wc.side_wall()).filter(|_| ik_on) {
            let off = |p: Vec3| (p - wall).dot(n);
            // The stance foot (the lower one in the run cycle) steps out onto the wall.
            let low = if pose.model_of(rig, b.legs[0].foot).pos.z <= pose.model_of(rig, b.legs[1].foot).pos.z { 0 } else { 1 };
            let heights = [pose.model_of(rig, b.legs[0].foot).pos.z, pose.model_of(rig, b.legs[1].foot).pos.z];
            let stance = ((heights[1 - low] - heights[low]) / 0.15).clamp(0.0, 1.0);
            let leg = b.legs[low];
            let ankle = world_from_model.transform_point3(pose.model_of(rig, leg.foot).pos);
            let d = off(ankle);
            if d < 1.0 {
                let target = ankle - n * (d - 0.1);
                let knee = pose.model_of(rig, leg.lower).pos;
                ch.debug_targets.push((target, Color::srgb(0.2, 0.9, 0.3)));
                two_bone_ik(&mut pose, rig, leg.upper, leg.lower, leg.foot, to_model(target), Some(knee + Vec3::new(0.4, 0.0, 0.0)), None, smooth(w) * stance);
            }
            // The hand nearer the wall touches it a little ahead of the shoulder, mid-run.
            let hand_i = if off(world_from_model.transform_point3(pose.model_of(rig, b.arms[0].hand).pos))
                < off(world_from_model.transform_point3(pose.model_of(rig, b.arms[1].hand).pos))
            {
                0
            } else {
                1
            };
            let arm = b.arms[hand_i];
            let shoulder = world_from_model.transform_point3(pose.model_of(rig, arm.upper).pos);
            let ahead = root.rotation * Vec3::NEG_Z;
            let touch = shoulder - n * (off(shoulder) - 0.05) + ahead * 0.25 - Vec3::Y * 0.1;
            let wgt = smooth(w) * (u * std::f32::consts::PI).sin();
            if (touch - shoulder).length() < 0.75 {
                ch.debug_targets.push((touch, Color::srgb(0.95, 0.75, 0.2)));
                let elbow = pose.model_of(rig, arm.lower).pos;
                two_bone_ik(&mut pose, rig, arm.upper, arm.lower, arm.hand, to_model(touch), Some(elbow + Vec3::new(-0.3, 0.0, -0.3)), None, wgt);
            }
        }

        // --- Leaning on a wall: the palms on it, where the clip holds them.
        if let Some(w) = ch.wall.as_ref().filter(|w| ik_on && w.state == "lean") {
            let n = w.normal;
            for i in 0..2 {
                let arm = b.arms[i];
                let hand = world_from_model.transform_point3(pose.model_of(rig, arm.hand).pos);
                if let Some(hit) = level.raycast(hand + n * 0.3, -n, 1.3).filter(|h| h.normal.dot(n) > 0.7) {
                    let target = hit.point + n * PALM_OFF;
                    ch.debug_targets.push((target, Color::srgb(0.4, 0.9, 0.5)));
                    let elbow = pose.model_of(rig, arm.lower).pos;
                    two_bone_ik(&mut pose, rig, arm.upper, arm.lower, arm.hand, to_model(target), Some(elbow + Vec3::new(0.0, 0.0, -0.3)), None, 1.0);
                }
            }
        }
        // --- Clip-driven climbing: snap hands onto holds and feet onto the wall. Faded in and out over `WALL_IK_FADE` as
        // the hands take to the wall or leave it (a corner, a top out): switched at once, the feet jumped 10-13 cm.
        let hands_on = ch.wall.as_ref().is_some_and(|w| ik_on && w.hands_on_wall());
        ch.wall_ik_w = if hands_on { (ch.wall_ik_w + dt / WALL_IK_FADE).min(1.0) } else { (ch.wall_ik_w - dt / WALL_IK_FADE).max(0.0) };
        let ww = smooth(ch.wall_ik_w);
        if let Some(w) = ch.wall.as_ref().filter(|_| ww > 0.0) {
            let n = w.normal;
            // A hold out of the arm's reach: the shoulder and chest go toward it first (HumanIK, which AC1 used for
            // climbing, reaches with the whole upper body), the clavicle lifting the shoulder, the chest bending.
            for i in 0..2 {
                let arm = b.arms[i];
                let wrist = world_from_model.transform_point3(pose.model_of(rig, arm.hand).pos);
                let Some((_, target, d)) = crate::climb::nearest_hold(&level, wrist, n) else { continue };
                if d > 0.2 {
                    continue;
                }
                let (shoulder, elbow, hand) = (pose.model_of(rig, arm.upper).pos, pose.model_of(rig, arm.lower).pos, pose.model_of(rig, arm.hand).pos);
                let length = shoulder.distance(elbow) + elbow.distance(hand);
                let to = to_model(target) - shoulder;
                let short = to.length() - length * 0.98;
                if short <= 0.0 {
                    continue;
                }
                // Faded in with the hold's distance like the arm IK below, or the shoulder jumps as a hold comes in range.
                let near = 1.0 - ((d - 0.06) / 0.14).clamp(0.0, 1.0);
                let k = (short / 0.15).min(1.0) * smooth(near) * ww;
                let side = if i == 0 { 1.0 } else { -1.0 };
                let reach = to.normalize_or_zero();
                pose.rotate_model(rig, b.clavicles[i], limited_arc(Vec3::new(0.0, side, 0.0), Vec3::new(0.0, side, 0.0) + reach, 0.45 * k));
                pose.rotate_model(rig, b.spine[1], limited_arc(Vec3::Z, Vec3::Z + reach * 0.5, 0.18 * k));
            }
            for i in 0..2 {
                let arm = b.arms[i];
                let wrist = world_from_model.transform_point3(pose.model_of(rig, arm.hand).pos);
                if let Some((_, target, d)) = crate::climb::nearest_hold(&level, wrist, n) {
                    // Full correction near a hold, none while the hand travels between holds.
                    let wgt = 1.0 - ((d - 0.06) / 0.14).clamp(0.0, 1.0);
                    if wgt > 0.0 {
                        ch.debug_targets.push((target, Color::srgb(0.95, 0.75, 0.2)));
                        let elbow = pose.model_of(rig, arm.lower).pos;
                        let pole = elbow + Vec3::new(-0.3, 0.0, -0.3);
                        two_bone_ik(&mut pose, rig, arm.upper, arm.lower, arm.hand, to_model(target), Some(pole), None, smooth(wgt) * ww);
                    }
                }
            }
            for i in 0..2 {
                let leg = b.legs[i];
                let ankle = world_from_model.transform_point3(pose.model_of(rig, leg.foot).pos);
                let Some(hit) = level.raycast(ankle + n * 0.3, -n, 0.8) else {
                    ch.wall_feet[i] = None;
                    continue;
                };
                // How far to move the foot onto the wall, smoothed: a foot passing a hold meets its front 12 cm out
                // from the wall for a moment, and a target jumping out and back twitches the leg.
                let raw = (hit.point + n * 0.11 - ankle).dot(-n);
                let d = match ch.wall_feet[i] {
                    Some(prev) => prev + (raw - prev) * (1.0 - (-WALL_FOOT_RATE * dt).exp()),
                    None => raw,
                };
                ch.wall_feet[i] = Some(d);
                // Only pull feet that are near the wall onto it (not dangling ones), faded in by distance.
                let wgt = 0.8 * (1.0 - ((d.abs() - 0.2) / 0.1).clamp(0.0, 1.0));
                if wgt > 0.0 {
                    let t = ankle - n * d;
                    ch.debug_targets.push((t, Color::srgb(0.2, 0.9, 0.3)));
                    let knee = pose.model_of(rig, leg.lower).pos;
                    two_bone_ik(
                        &mut pose,
                        rig,
                        leg.upper,
                        leg.lower,
                        leg.foot,
                        to_model(t),
                        Some(knee + Vec3::new(0.3, 0.0, 0.2)),
                        None,
                        smooth(wgt / 0.8) * 0.8 * ww,
                    );
                }
            }
        } else {
            ch.wall_feet = [None; 2];
        }

        // --- Animated: AC1 clip + foot placement onto the level. Also through the moves made standing on the ground (a stop,
        // a turn on the spot, leaning on a wall), and faded in and out over `FOOT_IK_FADE` where it starts or stops (off
        // and on at once, the feet popped by its correction at every move's seam).
        let feet_down = animated && !climbing && ik_on && ch.wall.as_ref().is_none_or(|w| w.legs_on_ground());
        ch.foot_ik_w = if feet_down { (ch.foot_ik_w + dt / FOOT_IK_FADE).min(1.0) } else { (ch.foot_ik_w - dt / FOOT_IK_FADE).max(0.0) };
        let foot_w = smooth(ch.foot_ik_w);
        if foot_w > 0.0
            && let Some(fp) = &mut ch.foot_ik
        {
            let ground = |p: Vec3| {
                let w = world_from_model.transform_point3(p);
                level.raycast(w, Vec3::NEG_Y, 1.8).map(|h| ik::placement::GroundHit { point: to_model(h.point), normal: dir_to_model(h.normal) })
            };
            fp.solve(&mut pose, rig, 0.0, dt, foot_w, ground);
        }

        // --- Gait overlay: arms swing with the opposite foot, shoulders counter-twist.
        let fwdness: Vec<f32> = (0..2).map(|i| (foot_world(&ch.feet[i]) - root.translation).dot(fwd_w)).collect();
        let gait = if animated { 0.0 } else { (speed / 2.5).clamp(0.0, 1.0) * (1.0 - climb_w) };
        for i in 0..2 {
            let opp = fwdness[1 - i];
            let ang = (opp * 1.3).clamp(-0.6, 0.6) * gait;
            pose.rotate_model(rig, b.arms[i].upper, Quat::from_rotation_y(-ang));
            pose.rotate_model(rig, b.arms[i].lower, Quat::from_rotation_y(-ang.max(0.0) * 0.6));
        }
        let twist = (fwdness[0] - fwdness[1]) * 0.5 * gait;
        pose.rotate_model(rig, b.spine[2], Quat::from_rotation_z(twist));
        pose.rotate_model(rig, b.spine[0], Quat::from_rotation_y(0.12 * gait));

        // --- Pelvis: drop for the lower foot and with stride length (bob).
        let root_y = root.translation.y;
        let feet_y = [foot_world(&ch.feet[0]).y, foot_world(&ch.feet[1]).y];
        let spread = (fwdness[0] - fwdness[1]).abs();
        let mut pelvis_want = (feet_y[0].min(feet_y[1]) - root_y).clamp(-0.45, 0.0) - spread * 0.06;
        if climb_w > 0.0 {
            pelvis_want *= 1.0 - climb_w;
        }
        let k = 1.0 - (-14.0 * dt).exp();
        ch.pelvis += (pelvis_want - ch.pelvis) * k;
        if ik_on && !animated {
            pose.translate_model(rig, b.hips, Vec3::Z * ch.pelvis);
            // (AC1's spine hangs beside the hips, off `Reference`: it drops with them.)
            if rig.parents[b.spine[0]] != Some(b.hips) {
                pose.translate_model(rig, b.spine[0], Vec3::Z * ch.pelvis);
            }
        }

        // --- Legs: two-bone IK onto the stepped feet (or the wall), sole aligned to the surface.
        if ik_on && (!animated || climbing) {
            for i in 0..2 {
                let leg = b.legs[i];
                let f = ch.feet[i];
                let (sole, n) = match ch.climb.as_ref().filter(|c| c.weight > 0.0) {
                    Some(c) => wall_foot(ch, root, &level, c, i),
                    None => (foot_world(&f), foot_normal(&f)),
                };
                let ankle_w = sole + n * ch.ankle_height;
                let target = to_model(ankle_w);
                ch.debug_targets.push((sole, Color::srgb(0.2, 0.9, 0.3)));
                let foot_rot = pose.model_of(rig, leg.foot).rot;
                let n_model = dir_to_model(n).normalize_or(Vec3::Z);
                let tilt = limited_arc(Vec3::Z, n_model, if climb_w > 0.0 { 1.4 } else { 0.6 });
                let knee = pose.model_of(rig, leg.lower).pos;
                let pole = knee + Vec3::X * 0.6 + Vec3::Y * if i == 0 { 0.08 } else { -0.08 };
                two_bone_ik(&mut pose, rig, leg.upper, leg.lower, leg.foot, target, Some(pole), Some(tilt * foot_rot), 1.0);
            }
        }

        // --- Climbing: hands on the ledge, body pulled in.
        if let Some(c) = ch.climb.as_ref().filter(|c| c.weight > 0.0) {
            let w = smooth(c.weight);
            // Lean the chest toward the wall and look up a little.
            pose.rotate_model(rig, b.spine[1], Quat::from_rotation_y(0.18 * w));
            for i in 0..2 {
                let mut grip = c.grips[i];
                if let Some(m) = c.moving.filter(|m| m.hand == i) {
                    // Arc away from the wall between grips.
                    let t = smooth(m.t);
                    grip = m.from.lerp(m.to, t) + c.ledge_normal * (t * std::f32::consts::PI).sin() * 0.12 + Vec3::Y * (t * std::f32::consts::PI).sin() * 0.05;
                }
                ch.debug_targets.push((grip, Color::srgb(0.95, 0.75, 0.2)));
                let point = to_model(grip);
                let normal = dir_to_model(c.ledge_normal);
                let up = dir_to_model(Vec3::Y);
                let shoulder = pose.model_of(rig, b.arms[i].upper).pos;
                let side = if i == 0 { 1.0 } else { -1.0 };
                let elbow_hint = (shoulder + point) * 0.5 + Vec3::new(-0.3, 0.45 * side, -0.4);
                // Clavicle helps reach upward.
                let clav = b.clavicles[i];
                let reach = (point - shoulder).normalize_or_zero();
                pose.rotate_model(rig, clav, Quat::IDENTITY.slerp(limited_arc(Vec3::new(0.0, side, 0.0), Vec3::new(0.0, side, 0.0) + reach * 0.6, 0.35), w));
                place_hand(&mut pose, rig, b.arms[i], point, normal, up, elbow_hint, ch.hand_basis[i], ch.palm_offset[i], w);
            }
        }

        // --- Arms out of walls: where the upper arm or forearm would pass through the level, push the
        // hand out along the surface to just clear of it (on the ground and fighting; climbing places
        // the hands itself).
        if ik_on && !on_wall {
            for i in 0..2 {
                let arm = b.arms[i];
                let m = |bone: usize| world_from_model.transform_point3(pose.model_of(rig, bone).pos);
                let (s, e, h) = (m(arm.upper), m(arm.lower), m(arm.hand));
                let pierce = |a: Vec3, z: Vec3| level.raycast(a, (z - a).normalize_or_zero(), (z - a).length()).filter(|hit| hit.normal.y.abs() < 0.7);
                // How deep the hand is behind the surface it crosses.
                let want = pierce(s, e).or_else(|| pierce(e, h)).map_or(Vec3::ZERO, |hit| {
                    let n = hit.normal.with_y(0.0).normalize_or_zero();
                    n * ((hit.point - h).dot(n) + ARM_CLEARANCE).max(0.0)
                });
                ch.arm_push[i] = ch.arm_push[i].lerp(want, 1.0 - (-18.0 * dt).exp());
                let push = ch.arm_push[i];
                if push.length() > 0.005 {
                    let side = if i == 0 { 1.0 } else { -1.0 };
                    let elbow_hint = to_model(e + push.normalize() * 0.3) + Vec3::new(-0.1, 0.2 * side, 0.0);
                    two_bone_ik(&mut pose, rig, arm.upper, arm.lower, arm.hand, to_model(h + push), Some(elbow_hint), None, 1.0);
                }
            }
        }

        // --- Head and spine track the nearest point of interest in front.
        let head_w = pose.model_of(rig, b.head).pos;
        let head_world = world_from_model.transform_point3(head_w);
        let target = level
            .interest
            .iter()
            .copied()
            .filter(|p| {
                let d = *p - head_world;
                d.length() < 9.0 && d.normalize().dot(fwd_w) > -0.1
            })
            .min_by(|a, b| (*a - head_world).length().total_cmp(&(*b - head_world).length()));
        let want = if target.is_some() && climb_w < 0.5 && !on_wall { 1.0 } else { 0.0 };
        ch.look_weight += (want - ch.look_weight) * (1.0 - (-3.0 * dt).exp());
        if let Some(t) = target {
            ch.look_target = ch.look_target.lerp(t, 1.0 - (-6.0 * dt).exp());
        }
        if ik_on && ch.look_weight > 0.01 {
            let links = [
                LookLink { bone: b.spine[0], weight: 0.1, limit: 0.12 },
                LookLink { bone: b.spine[1], weight: 0.12, limit: 0.15 },
                LookLink { bone: b.spine[2], weight: 0.15, limit: 0.2 },
                LookLink { bone: b.neck, weight: 0.4, limit: 0.45 },
                LookLink { bone: b.head, weight: 1.0, limit: 0.75 },
            ];
            let t = to_model(ch.look_target);
            let head_before = pose.model_of(rig, b.head).rot;
            let spine2_before = pose.model_of(rig, b.spine[2]).rot;
            look_at(&mut pose, rig, &links, b.head, ch.look_axis, t, smooth(ch.look_weight));
            // Hood constraint: hood bones hang off Spine2 but follow the head's turn beyond it.
            let head_delta = pose.model_of(rig, b.head).rot * head_before.inverse();
            let spine_delta = pose.model_of(rig, b.spine[2]).rot * spine2_before.inverse();
            let extra = head_delta * spine_delta.inverse();
            for &h in &b.hood {
                pose.rotate_model(rig, h, extra);
            }
            ch.debug_targets.push((ch.look_target, Color::srgb(0.3, 0.6, 1.0)));
        }

        // --- Write joints.
        for (i, &e) in ch.joints.iter().enumerate() {
            if let Ok(mut t) = joints.get_mut(e) {
                t.translation = pose.local[i].pos;
                t.rotation = pose.local[i].rot;
            }
        }
        ch.pose = pose;
    }
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn foot_world(f: &Foot) -> Vec3 {
    match f.swing {
        Some(s) => {
            let t = smooth(s.t);
            let lift = 0.09 + (s.to.y - s.from.y).max(0.0) * 1.2;
            s.from.lerp(s.to, t) + Vec3::Y * (s.t * std::f32::consts::PI).sin() * lift
        }
        None => f.planted,
    }
}

fn foot_normal(f: &Foot) -> Vec3 {
    match f.swing {
        Some(s) => f.normal.lerp(s.to_normal, smooth(s.t)).lerp(Vec3::Y, (s.t * std::f32::consts::PI).sin()).normalize(),
        None => f.normal,
    }
}

fn ground_at(level: &Level, p: Vec3) -> Hit {
    level.ground(p, 0.7, 1.5).unwrap_or(Hit { point: p, normal: Vec3::Y, dist: 0.0 })
}

/// Alternate steps: a foot lifts when it lags too far behind where it should be.
fn step_feet(ch: &mut Character, root: &Transform, level: &Level, dt: f32, speed: f32) {
    let step_dur = (0.42 - speed * 0.05).clamp(0.24, 0.42);
    for i in 0..2 {
        if let Some(s) = &mut ch.feet[i].swing {
            s.t += dt / s.dur;
            if s.t >= 1.0 {
                ch.feet[i] = Foot { planted: s.to, normal: s.to_normal, swing: None };
            }
        }
    }
    if ch.feet.iter().any(|f| f.swing.is_some_and(|s| s.t < 0.75)) {
        return;
    }
    let lead = ch.velocity * step_dur * 0.9;
    let threshold = if speed > 0.3 { (speed * step_dur * 0.85).max(0.18) } else { 0.1 };
    let mut best: Option<(usize, f32, Vec3)> = None;
    for i in 0..2 {
        if ch.feet[i].swing.is_some() {
            continue;
        }
        let want = ch.neutral_foot(root, i) + lead;
        let d = (want - ch.feet[i].planted).with_y(0.0).length();
        if d > threshold && best.is_none_or(|b| d > b.1) {
            best = Some((i, d, want));
        }
    }
    if let Some((i, _, want)) = best {
        let g = ground_at(level, want);
        ch.feet[i].swing = Some(Swing { from: ch.feet[i].planted, to: g.point, to_normal: g.normal, t: 0.0, dur: step_dur });
    }
}

/// Feet braced on the wall below the hands.
fn wall_foot(ch: &Character, root: &Transform, level: &Level, c: &Climb, i: usize) -> (Vec3, Vec3) {
    let side = if i == 0 { 1.0 } else { -1.0 };
    let left = root.rotation * (model_to_bevy() * Vec3::Y);
    let hip_height = 0.42 + if i == 0 { 0.0 } else { 0.12 };
    let from = root.translation + left * 0.14 * side + Vec3::Y * hip_height;
    let hit = level.raycast(from, -c.ledge_normal, 1.2);
    let wall = hit.map(|h| h.point).unwrap_or(from - c.ledge_normal * WALL_DIST);
    let free = (foot_world(&ch.feet[i]), foot_normal(&ch.feet[i]));
    let w = smooth(c.weight);
    // Sole against the wall: the "ground" normal is the wall normal tilted up.
    let n = (c.ledge_normal + Vec3::Y * 0.6).normalize();
    (free.0.lerp(wall + c.ledge_normal * 0.04, w), free.1.lerp(n, w).normalize())
}
