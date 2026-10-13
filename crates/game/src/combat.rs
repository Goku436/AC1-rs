//! Combat on AC1's own data: weapons drawn and sheathed, the fight stance locked on a target, the walk in stance,
//! attack chains with the combo window, blocks, the counter kill, the hidden blade's assassinations, and guards that
//! fight back. The moves are AC1's actions (`HumanWeapon`, `HumanGround_Fight`, `HumanGround_ComboKill`,
//! `HumanGround_CounterKills`, `HumanGround_Assassination`, `HumanGround_Fight_Hurt`), picked by the executable's
//! tables (`fight_data`) and played as the move graph's items (`MoveGraph::action_clips`).
//!
//! How it plays (what is AC1's and what is ours is in docs/COMBAT.md):
//! - Weapons (the manual's keys): 1 short blade, 2 hidden blade, 3 sword, 4 fists. A press selects the weapon (the one
//!   in hand goes back first), a second press draws it, a third sheathes it. The sword draws with `HumanWeapon`'s
//!   0x0C1BB807 (`xx_h_unsheath_grab_upper_body`, then `xx_h_unsheath_upper_body`, the sword moving from the hip's
//!   `P_LeftWaist_Tag` to the hand's `RightHand_Weapon_Ref` between them) and sheathes with 0x0C1BB7D6, over the
//!   legs (upper body only); the short blade with its `knife` pair from `P_Dagger_Tag`; fists with
//!   `xx_h_unarmed_select_<foot>`.
//! - F (target lock): the fight mode, locked on the nearest guard (the sword drawn if it is not), into the stance
//!   (`xx_h_wait_hipm_<foot>_tr_xx_h_light_wait_<foot>`, then the waits 0x00FD645B / 0x0108295C); F again leaves it
//!   (`xx_h_light_wait_<foot>_tr_xx_h_wait_hipm_<foot>`). The stick walks in stance facing the target (the 8-way blend
//!   space 0x0429C55B / 0x0429DBAE, the nearest of its 8 walks); high profile held defends (the defence's wait and walk,
//!   0x1122A264 / 0x1122A263).
//! - The weapon hand: an attack picked by side, distance and foot (`fight_data::attack`), strong on a tap, max when
//!   held; its strike lands as the strike item starts: the target blocks, is hit or dies. Pressed again while the
//!   strike lands (its item), the combo: the next hit by AC1's direction tables, up to `COMBO_HITS`; the one that would
//!   kill is the paired combo kill. A press during the wind-up spoils the combo.
//! - Defending (high profile), the weapon hand in the counter window of a guard's attack: the paired counter kill.
//! - The hidden blade selected, the weapon hand near a guard that is not fighting: the assassination AC1's chooser
//!   picks (`fight_data::assassination_type`), the assassin's lead-in steered so its body meets the kill, the kill
//!   played paired (both roots on the victim's), the victim limp after.
//! - Guards (ours, simple): a militia guard that stands until it is locked on, attacked or a drawn weapon comes within
//!   4 m in front of it; then it fights: faces the player, keeps 1.2-2 m, attacks every few seconds with AC1's max
//!   attacks, blocks some first attacks, takes 3 hits. Deaths go limp (the ragdoll) when their clip ends.
//!
//! Script hooks (env): `AC1_GUARDS="x,z,yaw;..."` (guards there; in the test level two stand in the fight yard by
//! default, scripted runs have none unless asked), `AC1_GUARD_BLOCK=0..1` (the chance a guard blocks a first attack,
//! 0.5), `AC1_GUARD_PASSIVE=1` (guards never attack), `AC1_GUARD_HP=n`, `AC1_WEAPON="secs:key,..."` (a weapon key
//! pressed then), `AC1_LOCK="secs,..."` (F pressed), `AC1_ATTACK="secs[:held],...,combo,counter"` (the weapon hand: at
//! a time, held that long; `combo` presses in the middle of the next combo window, `counter` as the next counter
//! window opens).

use crate::animation::{AnimLib, Clip, mix_name, root_motion_at, root_rotation_at};
use crate::character::{Character, Controller, Player, model_to_bevy};
use crate::fight_data as fd;
use crate::level::Level;
use bevy::prelude::*;
use forge::anim::FPS;
use ik::{Pose, Rig, Xform};
use std::sync::Arc;

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FightInput>().init_resource::<FightScript>().add_systems(
            Update,
            (spawn_guards, fight_input, fight).chain().after(crate::run_script).after(crate::fly::fly).before(crate::character::locomotion),
        );
    }
}

// --- Bones and weapons --------------------------------------------------------------------------------------------

/// Bone name hashes: the hand's weapon bone, the sword's hip tag and the short blade's back tag (CRC32 of their names,
/// matched against the exe's `BIPEDBONE_*` list and its tag table at 0xB11020).
const RIGHT_HAND_WEAPON_REF: u32 = 0x3fb2_56e5;
const P_LEFTWAIST_TAG: u32 = 0x3a83_5926;
const P_DAGGER_TAG: u32 = 0x685e_46b6;
const REFERENCE: u32 = 0x2c52_cbb0;

/// The four weapons by AC1's PC keys (the manual: 1 short blade and throwing knives, 2 hidden blade, 3 sword, 4 fists).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Weapon {
    ShortBlade,
    HiddenBlade,
    #[default]
    Sword,
    Fists,
}

impl Weapon {
    pub fn from_key(k: u8) -> Option<Weapon> {
        Some(match k {
            1 => Weapon::ShortBlade,
            2 => Weapon::HiddenBlade,
            3 => Weapon::Sword,
            4 => Weapon::Fists,
            _ => return None,
        })
    }
    fn name(self) -> &'static str {
        match self {
            Weapon::ShortBlade => "short blade",
            Weapon::HiddenBlade => "hidden blade",
            Weapon::Sword => "sword",
            Weapon::Fists => "fists",
        }
    }
    /// The bone it is carried on, which the draw moves to the hand.
    fn carry_bone(self) -> Option<u32> {
        match self {
            Weapon::Sword => Some(P_LEFTWAIST_TAG),
            Weapon::ShortBlade => Some(P_DAGGER_TAG),
            _ => None,
        }
    }
    /// Its draw and sheathe, upper body (`HumanWeapon`): the clips, and the one at whose start it changes hands.
    fn draw_clips(self, draw: bool, foot: &str) -> (Vec<String>, usize) {
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        match (self, draw) {
            (Weapon::Sword, true) => (v(&["xx_h_unsheath_grab_upper_body", "xx_h_unsheath_upper_body"]), 1),
            (Weapon::Sword, false) => (v(&["xx_h_sheath_upper_body", "xx_h_sheath_release_upper_body"]), 1),
            (Weapon::ShortBlade, true) => (v(&["xx_h_knife_unsheath_upper_body_start", "xx_h_knife_unsheath_upper_body"]), 1),
            (Weapon::ShortBlade, false) => (v(&["xx_h_knife_sheath_upper_body_start", "xx_h_knife_sheath_upper_body"]), 1),
            (Weapon::Fists, _) => (vec![format!("xx_h_unarmed_select_{foot}")], 0),
            (Weapon::HiddenBlade, _) => (vec![], 0),
        }
    }
}

/// A rig bone by its name hash (named bones by name, the rest are `#<hash>`).
fn bone(rig: &Rig, hash: u32, name: &str) -> Option<usize> {
    rig.find(name).or_else(|| rig.find(&format!("#{hash:08x}")))
}

/// A copy of a character part (to hand the player's sword to a guard).
fn clone_part(p: &crate::assets::Part) -> crate::assets::Part {
    crate::assets::Part {
        name: p.name.clone(),
        mesh: p.mesh.clone(),
        joints: p.joints.clone(),
        inverse_binds: p.inverse_binds.clone(),
        texture: p.texture,
        normal: p.normal,
        alpha: p.alpha,
        material: p.material.clone(),
        look: p.look.clone(),
        inside: p.inside.clone(),
    }
}

/// The weapon on carry bone `carry` drawn in the hand: the carry bone posed where the hand's weapon bone is (AC1
/// attaches the drawn weapon to `RightHand_Weapon_Ref`). Called on the final pose (`character::animate`).
pub fn hold_weapon(pose: &mut Pose, rig: &Rig, carry: u32) {
    let name = if carry == P_DAGGER_TAG { "P_Dagger_Tag" } else { "P_LeftWaist_Tag" };
    let (Some(t), Some(h)) = (bone(rig, carry, name), bone(rig, RIGHT_HAND_WEAPON_REF, "RightHand_Weapon_Ref")) else { return };
    let hand = pose.model_of(rig, h);
    let parent = rig.parents[t].map_or(Xform { pos: Vec3::ZERO, rot: Quat::IDENTITY }, |p| pose.model_of(rig, p));
    pose.local[t] = parent.inverse().mul(&hand);
}

// --- Input --------------------------------------------------------------------------------------------------------

/// This frame's fight input (the player's keys, or the script).
#[derive(Resource, Default)]
pub struct FightInput {
    /// A weapon key pressed (1-4).
    pub weapon_key: Option<u8>,
    /// F pressed: the target lock.
    pub lock: bool,
    /// The weapon hand: pressed this frame, held, let go this frame.
    pub hand_press: bool,
    pub hand_held: bool,
    pub hand_release: bool,
}

/// The scripted fight input (`AC1_WEAPON`, `AC1_LOCK`, `AC1_ATTACK`) and its clock.
#[derive(Resource, Default)]
struct FightScript {
    loaded: bool,
    t: f32,
    weapons: Vec<(f32, u8)>,
    locks: Vec<f32>,
    /// The weapon hand: (when, how long held) or a cue (`combo`, `counter`), in order.
    hand: Vec<HandCue>,
    held_until: f32,
}

#[derive(Clone, Copy, Debug)]
enum HandCue {
    At(f32, f32),
    Combo,
    Counter,
}

fn load_script(s: &mut FightScript) {
    s.loaded = true;
    let get = |k: &str| std::env::var(k).ok().unwrap_or_default();
    for p in get("AC1_WEAPON").split(',').filter(|p| !p.trim().is_empty()) {
        if let Some((t, k)) = p.split_once(':')
            && let (Ok(t), Ok(k)) = (t.trim().parse(), k.trim().parse())
        {
            s.weapons.push((t, k));
        }
    }
    s.locks = get("AC1_LOCK").split(',').filter_map(|p| p.trim().parse().ok()).collect();
    for p in get("AC1_ATTACK").split(',').map(str::trim).filter(|p| !p.is_empty()) {
        s.hand.push(match p {
            "combo" => HandCue::Combo,
            "counter" => HandCue::Counter,
            _ => {
                let (t, h) = p.split_once(':').unwrap_or((p, "0.05"));
                HandCue::At(t.parse().unwrap_or(0.0), h.parse().unwrap_or(0.05))
            }
        });
    }
}

/// The fight's keys (AC1's PC defaults: the left button is the weapon hand, F the target lock, 1-4 the weapons), or the
/// script's presses.
#[allow(clippy::too_many_arguments)]
fn fight_input(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    menu: Option<Res<crate::menu::Menu>>,
    mut input: ResMut<FightInput>,
    mut script: ResMut<FightScript>,
    fighters: Query<&Fighter>,
    player: Query<&Fighter, With<Player>>,
) {
    *input = FightInput::default();
    if !script.loaded {
        load_script(&mut script);
    }
    let scripted = std::env::var("AC1_SHOT").is_ok();
    if scripted {
        let dt = time.delta_secs().min(0.05);
        let (t0, t1) = (script.t, script.t + dt);
        script.t = t1;
        let now = |t: f32| t >= t0 && t < t1;
        input.weapon_key = script.weapons.iter().find(|(t, _)| now(*t)).map(|(_, k)| *k);
        input.lock = script.locks.iter().any(|t| now(*t));
        // The weapon hand: the next cue, when it is due.
        let due = match script.hand.first().copied() {
            Some(HandCue::At(t, h)) => (t1 >= t).then_some(h),
            Some(HandCue::Combo) => player.iter().next().is_some_and(|f| f.in_combo_window(0.5)).then_some(0.05),
            Some(HandCue::Counter) => fighters.iter().any(|f| f.counter_open_for(0.25)).then_some(0.05),
            None => None,
        };
        if let Some(h) = due {
            script.hand.remove(0);
            input.hand_press = true;
            script.held_until = t1 + h;
            debug!("combat: script presses the weapon hand at {t1:.2} s");
        }
        input.hand_held = t1 < script.held_until;
        input.hand_release = !input.hand_held && t0 < script.held_until;
        return;
    }
    if menu.is_some_and(|m| m.open.is_some()) {
        return;
    }
    for (k, n) in [(KeyCode::Digit1, 1), (KeyCode::Digit2, 2), (KeyCode::Digit3, 3), (KeyCode::Digit4, 4)] {
        if keys.just_pressed(k) {
            input.weapon_key = Some(n);
        }
    }
    input.lock = keys.just_pressed(KeyCode::KeyF);
    input.hand_press = mouse.just_pressed(MouseButton::Left);
    input.hand_held = mouse.pressed(MouseButton::Left);
    input.hand_release = mouse.just_released(MouseButton::Left);
}

// --- Fighters -----------------------------------------------------------------------------------------------------

/// What a step of a move is, for the fight's rules.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// Into or out of the stance.
    Stance,
    WindUp,
    Strike,
    Recover,
    Hit,
    Block,
    /// A lead-in before a paired clip (steered to meet it).
    Lead,
    Paired,
    Death,
}

/// How a step moves the root.
#[derive(Clone, Copy, Debug)]
enum Root {
    /// The clip's root motion from where the step started.
    Clip,
    /// From where the step started to this place and facing, linearly over the step (AC1's discrete moves, sub_711130).
    Steer(Vec3, Quat),
    /// From where the step started to this place along the clip's own progress, the facing kept (a wind-up steered so
    /// its strike lands).
    Line(Vec3),
    /// Not at all.
    Hold,
}

/// How far from its target a strike ends (m, ours).
const CONTACT: f32 = 0.95;
/// How near two fighters may stand (m, ours: AC1's character proxies keep them apart).
const KEEP_APART: f32 = 0.8;

#[derive(Clone)]
struct Step {
    clip: Arc<Clip>,
    kind: Kind,
    root: Root,
    /// How long it plays (s): the clip's length, or longer to hold its last frame (a victim waiting for its killer).
    len: f32,
    /// Shown at this time throughout (a pose held), instead of playing.
    frozen: Option<f32>,
    /// The blend into it (s).
    blend: f32,
}

/// What follows a move.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Then {
    Stance,
    Locomotion,
    Limp,
}

struct Act {
    steps: Vec<Step>,
    i: usize,
    t: f32,
    /// The root when the step started.
    anchor: Transform,
    then: Then,
    /// The root both sides of a paired clip start from (the victim's), taken when the first paired step starts.
    pair_root: Option<Transform>,
    /// Through a paired clip: the pair's root in this fighter's frame (AC1 puts the root on the victim's, sub_52AAF0;
    /// ours keeps the root where it is and moves the body by this, so nothing jumps), eased out after the last paired
    /// step.
    offset: Option<Transform>,
    /// The step started this frame (its first frame's events).
    fresh: bool,
}

impl Act {
    fn new(steps: Vec<Step>, anchor: Transform, then: Then, pair_root: Option<Transform>) -> Act {
        Act { steps, i: 0, t: 0.0, anchor, then, pair_root, offset: None, fresh: true }
    }

    /// The index of the last paired step.
    fn last_paired(&self) -> Option<usize> {
        self.steps.iter().rposition(|s| s.kind == Kind::Paired)
    }

    fn step(&self) -> Option<&Step> {
        self.steps.get(self.i)
    }
}

/// A swing in progress: what the fight rules need to know about it.
#[derive(Clone, Copy, Debug)]
struct Swing {
    target: Entity,
    /// The direction the blade ends in (the strike record's), a max attack, combo hits so far.
    dir: u8,
    max: bool,
    combos: u32,
    /// What the strike met: `None` not yet.
    outcome: Option<Outcome>,
    /// Pressed during the wind-up: no combo off this one. Pressed while the strike lands: the combo comes next.
    early: bool,
    queued: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Outcome {
    Hit,
    Blocked,
    Missed,
    Killed,
}

/// A draw or sheathe over the legs: its clips, the one playing, the time into it, the one the weapon changes hands at,
/// drawing (or sheathing), and the weapon.
type Drawing = (Vec<Arc<Clip>>, usize, f32, usize, bool, Weapon);

/// Anyone who fights: the player and the guards.
#[derive(Component)]
pub struct Fighter {
    pub selected: Weapon,
    pub drawn: Option<Weapon>,
    /// In the fight stance, locked on `target`.
    pub fight: bool,
    pub target: Option<Entity>,
    pub hp: i32,
    pub dead: bool,
    act: Option<Act>,
    swing: Option<Swing>,
    /// The foot ahead (0 left, 1 right).
    foot: usize,
    /// The stance's loop playing and the time into it.
    stance: Option<(Arc<Clip>, f32)>,
    /// A weapon drawn or sheathed over the legs: its clips, the one playing, the time, the one it changes hands at,
    /// drawing or sheathing, and which.
    draw: Option<Drawing>,
    /// The weapon hand held since (s), waiting to see whether the attack is a max one.
    hold: Option<f32>,
}

impl Fighter {
    pub fn new(selected: Weapon, hp: i32) -> Self {
        Fighter { selected, drawn: None, fight: false, target: None, hp, dead: false, act: None, swing: None, foot: 0, stance: None, draw: None, hold: None }
    }

    fn kind(&self) -> Option<Kind> {
        self.act.as_ref().and_then(|a| a.step()).map(|s| s.kind)
    }

    /// In the combo window: the strike landed (hit or blocked) and is still playing; `at` 0..1 of the way through it.
    fn in_combo_window(&self, at: f32) -> bool {
        let Some(a) = &self.act else { return false };
        let Some(s) = a.step() else { return false };
        s.kind == Kind::Strike
            && a.t >= s.len * at
            && self.swing.is_some_and(|w| matches!(w.outcome, Some(Outcome::Hit | Outcome::Blocked)) && !w.queued && !w.early)
    }

    /// A guard's attack in the counter window (or `lead` s into it).
    fn counter_open_for(&self, lead: f32) -> bool {
        let Some(a) = &self.act else { return false };
        let Some(s) = a.step() else { return false };
        match s.kind {
            Kind::WindUp => a.t >= s.len - fd::COUNTER_BEFORE + lead,
            Kind::Strike => a.t <= fd::COUNTER_AFTER,
            _ => false,
        }
    }
}

/// A guard's mind (ours: AC1's fight AI is not decoded).
#[derive(Component)]
pub struct Guard {
    /// Fighting the player.
    pub hostile: bool,
    next_attack: f32,
    rng: u32,
    block: f32,
    passive: bool,
}

impl Guard {
    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / (1u32 << 24) as f32
    }
}

// --- Clips --------------------------------------------------------------------------------------------------------

/// The clip names of an action's items (the first of a blend space's variants first).
fn items(lib: &AnimLib, action: u32) -> Vec<Vec<String>> {
    lib.graph.action_clips(action).map(|v| v.into_iter().map(|i| i.to_vec()).collect()).unwrap_or_default()
}

fn clip(lib: &mut AnimLib, rig: usize, name: &str) -> Option<Arc<Clip>> {
    let c = lib.get_for(rig, name);
    if c.is_none() {
        warn!("combat: no clip {name}");
    }
    c
}

fn step(lib: &mut AnimLib, rig: usize, name: &str, kind: Kind, root: Root) -> Option<Step> {
    let c = clip(lib, rig, name)?;
    // (The item's blend from the move graph, else AC1's default 0.2 s.)
    let blend = lib.graph.item_blend(name).map_or(0.2, |b| b.time()).clamp(0.1, 0.3);
    Some(Step { len: c.anim.duration, clip: c, kind, root, frozen: None, blend })
}

/// The foot a clip ends on, by AC1's names (`_to_footr` ends on the right; else the foot it names).
fn end_foot(name: &str, was: usize) -> usize {
    if let Some(i) = name.rfind("_to_foot") {
        return usize::from(name[i..].starts_with("_to_footr"));
    }
    if name.contains("footr") {
        1
    } else if name.contains("footl") {
        0
    } else {
        was
    }
}

const FOOT: [&str; 2] = ["footl", "footr"];

/// The first clip of `lib.names` that starts with `prefix` and passes `pick`.
fn find_clip(lib: &AnimLib, prefix: &str, pick: impl Fn(&str) -> bool) -> Option<String> {
    lib.names.iter().filter(|n| n.starts_with(prefix) && pick(&n[prefix.len()..])).min_by_key(|n| n.len()).cloned()
}

// --- Root and pose ------------------------------------------------------------------------------------------------

/// The root at time `t` of a clip played from `anchor` (its root motion turned into the world).
fn clip_root(anchor: &Transform, c: &Clip, t: f32) -> Transform {
    let m2b = model_to_bevy();
    let p = root_motion_at(c, t * FPS);
    let r = root_rotation_at(c, t * FPS);
    Transform {
        translation: anchor.translation + anchor.rotation * (m2b * p),
        rotation: (anchor.rotation * (m2b * r * m2b.inverse())).normalize(),
        scale: anchor.scale,
    }
}

/// Where a clip puts the body at time `t`, against its root: the Reference bone's place (horizontal, model space) and
/// its yaw about the vertical.
fn body_at(c: &Clip, t: f32) -> (Vec2, f32) {
    let fix = Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2);
    let f = t * FPS;
    let pos = c.anim.tracks.iter().filter(|k| k.bone_hash == REFERENCE).find_map(|k| k.translation_at(f)).map_or(Vec3::ZERO, |p| fix * Vec3::from(p));
    let rot = c
        .anim
        .tracks
        .iter()
        .filter(|k| k.bone_hash == REFERENCE)
        .find_map(|k| k.rotation_at(f))
        .map_or(Quat::IDENTITY, |[x, y, z, w]| fix * Quat::from_xyzw(x, y, z, w));
    let fwd = rot * Vec3::X;
    (pos.truncate(), fwd.y.atan2(fwd.x))
}

/// A yaw about Bevy's up as a rotation (0 faces -Z).
fn yaw_rot(yaw: f32) -> Quat {
    Quat::from_rotation_y(yaw)
}

/// The root a lead-in must end at so that its last frame's body is where the paired clip's first frame puts it, with
/// the pair's root at `pair`: (place, facing).
fn meet(pair: &Transform, paired: &Clip, lead: &Clip) -> (Vec3, Quat) {
    let m2b = model_to_bevy();
    let (p_body, p_yaw) = body_at(paired, 0.0);
    let (l_body, l_yaw) = body_at(lead, lead.anim.duration);
    // (Model yaw is about +Z, counter-clockwise from +X; turned into Bevy it is a turn about +Y the same way.)
    let rot = pair.rotation * yaw_rot(p_yaw - l_yaw);
    let world = |r: Quat, v: Vec2| r * (m2b * v.extend(0.0));
    let pos = pair.translation + world(pair.rotation, p_body) - world(rot, l_body);
    (pos.with_y(pair.translation.y), rot)
}

/// `a` then `b` (rigid; `a`'s scale kept).
fn iso_mul(a: &Transform, b: &Transform) -> Transform {
    Transform { translation: a.translation + a.rotation * b.translation, rotation: (a.rotation * b.rotation).normalize(), scale: a.scale }
}

fn iso_inv(a: &Transform) -> Transform {
    let r = a.rotation.inverse();
    Transform { translation: r * -a.translation, rotation: r, scale: Vec3::ONE }
}

/// The body offset that shows a pose played from root `pair` on a character whose root is `tf` (`Character::
/// body_offset`, model space).
fn body_offset(o: &Transform, scale: f32) -> Xform {
    let m2b = model_to_bevy();
    Xform { pos: m2b.inverse() * o.translation / scale.max(1e-3), rot: (m2b.inverse() * o.rotation * m2b).normalize() }
}

/// The pair's root after the victim's lead-in: the victim's root moved by its lead-in's root motion.
fn after(tf: &Transform, steps: &[Step]) -> Transform {
    let mut t = *tf;
    for s in steps {
        if matches!(s.root, Root::Clip) && s.frozen.is_none() {
            t = clip_root(&t, &s.clip, s.clip.anim.duration);
        }
    }
    t
}

// --- The fight ----------------------------------------------------------------------------------------------------

/// A guard to spawn, and whether the default ones are wanted.
fn guard_spots(city: bool) -> Vec<(Vec3, f32)> {
    if let Ok(v) = std::env::var("AC1_GUARDS") {
        return v
            .split(';')
            .filter_map(|g| {
                let n: Vec<f32> = g.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                (n.len() >= 2).then(|| (Vec3::new(n[0], 0.0, n[1]), n.get(2).copied().unwrap_or(0.0).to_radians()))
            })
            .collect();
    }
    if city || std::env::var("AC1_SHOT").is_ok() || std::env::var("AC1_NO_GUARDS").is_ok() {
        return vec![];
    }
    // The fight yard, east of the start on open ground.
    vec![(Vec3::new(24.0, 0.0, -45.0), 90f32.to_radians()), (Vec3::new(26.0, 0.0, -49.0), 60f32.to_radians())]
}

/// The assets a character spawn writes to.
type MeshAssets<'w> = (
    ResMut<'w, Assets<Mesh>>,
    ResMut<'w, Assets<StandardMaterial>>,
    ResMut<'w, Assets<Image>>,
    ResMut<'w, Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>,
);

/// Guards from AC1's militia builder (`npc`), with the player's sword on their hip; and the player a fighter. Once,
/// when the clip library is in.
#[allow(clippy::too_many_arguments)]
fn spawn_guards(
    mut done: Local<bool>,
    mut commands: Commands,
    dir: Res<crate::GameDir>,
    level: Res<Level>,
    mut lib: Option<ResMut<AnimLib>>,
    player: Query<Entity, (With<Player>, Without<Fighter>)>,
    (mut meshes, mut mats, mut images, mut bindposes): MeshAssets,
) {
    if *done {
        return;
    }
    let Some(lib) = lib.as_mut() else { return };
    let Ok(p) = player.single() else { return };
    *done = true;
    commands.entity(p).insert(Fighter::new(Weapon::Sword, 10));
    let spots = guard_spots(level.city);
    if spots.is_empty() {
        return;
    }
    // The sword: Altaïr's, the one AC1 loads for him (the guards' own are separate weapon entities, not read here).
    let sword = crate::assets::load_character(&dir.0, "DataPC.forge", "Rank 9", "UCMA_Altair_Rank_9")
        .map_err(|e| warn!("combat: no sword for the guards: {e:#}"))
        .ok()
        .and_then(|d| {
            let part = d.parts.iter().position(|p| p.name.contains("Sword"))?;
            Some((clone_part(&d.parts[part]), d.textures))
        });
    let block = std::env::var("AC1_GUARD_BLOCK").ok().and_then(|v| v.parse().ok()).unwrap_or(0.5);
    let passive = std::env::var("AC1_GUARD_PASSIVE").is_ok();
    let hp = std::env::var("AC1_GUARD_HP").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
    for (k, (at, yaw)) in spots.into_iter().enumerate() {
        let (mut g, scale) = match crate::npc::build(&dir.0, crate::NPC_FORGE, crate::NPC_BLOCK, "MMMA_FreeMission_Militia", 11 + k as u64 * 131) {
            Ok(b) => b,
            Err(e) => {
                warn!("combat: guard: {e:#}");
                continue;
            }
        };
        if let (Some((part, textures)), Some(hip)) = (&sword, bone(&g.rig, P_LEFTWAIST_TAG, "P_LeftWaist_Tag")) {
            let mut part = clone_part(part);
            part.joints = vec![hip];
            for (id, img) in textures {
                g.textures.entry(*id).or_insert_with(|| img.clone());
            }
            g.parts.push(part);
        }
        let rig_id = lib.add_rig(&g.rig, g.hashes.clone());
        lib.use_set(rig_id, "mmaa_anim_set");
        let anim = crate::animation::Animator::new_for(lib, rig_id);
        let mut p = at;
        if let Some(h) = level.ground(p, 3.0, 10.0) {
            p.y = h.point.y;
        }
        let e = crate::character::spawn_character(&mut commands, g, p, &mut meshes, &mut mats, &mut images, &mut bindposes, anim, rig_id);
        let mut f = Fighter::new(Weapon::Sword, hp);
        f.drawn = None;
        commands.entity(e).insert((
            Transform::from_translation(p).with_rotation(yaw_rot(yaw)).with_scale(Vec3::splat(scale)),
            f,
            Guard { hostile: std::env::var("AC1_GUARD_AWARE").is_ok(), next_attack: 2.0, rng: 0x9e37_79b9 ^ (k as u32 * 7919 + 1), block, passive },
        ));
        info!("combat: guard {k} at [{:.2}, {:.2}, {:.2}]", p.x, p.y, p.z);
    }
}

/// What one fighter sees of another this frame.
#[derive(Clone, Copy)]
struct Seen {
    e: Entity,
    pos: Vec3,
    rot: Quat,
    alive: bool,
    guard: bool,
    hostile: bool,
    player: bool,
    busy: bool,
    speed: f32,
    /// A weapon in hand (not the hidden blade), and who it is locked on.
    armed: bool,
    target: Option<Entity>,
}

/// Something between two fighters, settled after everyone has moved.
#[derive(Clone, Copy, Debug)]
enum Ev {
    /// A strike lands: by, on, its blade's direction, a max one, a combo hit.
    Strike(Entity, Entity, u8, bool, bool),
    /// A counter kill: the defender, the attacker it kills.
    Counter(Entity, Entity),
    /// An assassination: assassin, victim, type.
    Assassinate(Entity, Entity, u8),
    /// A combo kill: killer, victim, the combo's direction.
    ComboKill(Entity, Entity, u8),
}

type FighterQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static mut Transform, &'static mut Character, &'static mut Controller, &'static mut Fighter, Option<&'static mut Guard>, Has<Player>),
>;

/// The fight: everyone's choices, moves and what they do to each other.
pub fn fight(time: Res<Time>, level: Res<Level>, input: Res<FightInput>, mut lib: Option<ResMut<AnimLib>>, mut q: FighterQuery) {
    let Some(lib) = lib.as_deref_mut() else { return };
    let dt = time.delta_secs().min(0.05);
    let seen: Vec<Seen> = q
        .iter()
        .map(|(e, tf, ch, _, f, g, pl)| Seen {
            e,
            pos: tf.translation,
            rot: tf.rotation,
            alive: !f.dead,
            guard: g.is_some(),
            hostile: g.as_ref().is_some_and(|g| g.hostile),
            player: pl,
            busy: f.act.is_some(),
            speed: ch.velocity.with_y(0.0).length(),
            armed: f.drawn.is_some_and(|w| w != Weapon::HiddenBlade),
            target: f.target,
        })
        .collect();
    let mut evs: Vec<Ev> = vec![];
    for (me, mut tf, mut ch, mut ctl, mut f, guard, is_player) in &mut q {
        let (tf, ch, ctl, f) = (&mut *tf, &mut *ch, &mut *ctl, &mut *f);
        if f.dead {
            continue;
        }
        if is_player {
            player_choices(lib, &input, &seen, me, tf, ch, ctl, f, dt, &mut evs);
        } else if let Some(mut g) = guard {
            guard_choices(lib, &seen, me, tf, ch, f, &mut g, dt);
        }
        let aim = f.target.and_then(|t| seen.iter().find(|s| s.e == t)).map(|s| s.pos);
        tick(lib, &level, me, tf, ch, ctl, f, aim, dt, &mut evs);
    }
    for ev in evs {
        settle(lib, &mut q, ev);
    }
    // Fighters keep apart (not through a paired move, whose clips put them together).
    let mut pairs = q.iter_combinations_mut();
    while let Some([(_, mut a, _, _, fa, _, _), (_, mut b, _, _, fb, _, _)]) = pairs.fetch_next() {
        let close = |f: &Fighter| f.dead || matches!(f.kind(), Some(Kind::Paired | Kind::Lead | Kind::Death));
        if close(&fa) || close(&fb) {
            continue;
        }
        let d = (b.translation - a.translation).with_y(0.0);
        let n = d.length();
        if n < KEEP_APART && n > 1e-4 {
            let push = d / n * (KEEP_APART - n) * 0.5;
            a.translation -= push;
            b.translation += push;
        }
    }
    for (_, _, mut ch, _, mut f, _, _) in &mut q {
        show(lib, &mut ch, &mut f, dt);
    }
}

/// The nearest live guard within `reach` of `at`, hostile ones first.
fn nearest_guard(seen: &[Seen], at: Vec3, reach: f32, hostile_first: bool) -> Option<Seen> {
    let mut v: Vec<&Seen> = seen.iter().filter(|s| s.guard && s.alive && s.pos.distance(at) < reach).collect();
    v.sort_by(|a, b| (hostile_first && !a.hostile).cmp(&(hostile_first && !b.hostile)).then(a.pos.distance(at).total_cmp(&b.pos.distance(at))));
    v.first().copied().copied()
}

/// The signed angle (radians, positive to the left) from the facing `rot` to the point `to` seen from `from`.
fn angle_to(rot: Quat, from: Vec3, to: Vec3) -> f32 {
    let fwd = (rot * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
    let d = (to - from).with_y(0.0).normalize_or_zero();
    fwd.cross(d).y.atan2(fwd.dot(d))
}

#[allow(clippy::too_many_arguments)]
fn player_choices(
    lib: &mut AnimLib,
    input: &FightInput,
    seen: &[Seen],
    me: Entity,
    tf: &mut Transform,
    ch: &mut Character,
    ctl: &Controller,
    f: &mut Fighter,
    dt: f32,
    evs: &mut Vec<Ev>,
) {
    // (On the ground: a ground move playing counts, the fight cuts it.)
    let on_ground = ch.wall.as_ref().is_none_or(|w| w.legs_on_ground()) && ch.fall_v == 0.0 && !ch.flying;
    // Weapons: a press selects, a second draws, a third sheathes.
    if let Some(w) = input.weapon_key.and_then(Weapon::from_key)
        && on_ground
        && f.draw.is_none()
    {
        if w != f.selected {
            if let Some(d) = f.drawn {
                start_draw(lib, ch, f, d, false);
            }
            info!("combat: selected the {}", w.name());
            f.selected = w;
        } else if f.drawn == Some(w) {
            start_draw(lib, ch, f, w, false);
        } else if f.drawn.is_none() {
            start_draw(lib, ch, f, w, true);
        }
    }
    // The target lock: into the fight on the nearest guard, or out of it.
    if input.lock && on_ground {
        if f.fight {
            leave_fight(lib, ch, tf, f);
        } else if let Some(t) = nearest_guard(seen, tf.translation, 10.0, true) {
            enter_fight(lib, ch, tf, f, t.e);
            if f.drawn != Some(Weapon::Sword) && f.draw.is_none() {
                f.selected = Weapon::Sword;
                start_draw(lib, ch, f, Weapon::Sword, true);
            }
        }
    }
    // A lost target: the next hostile one, else out of the fight.
    if f.fight && f.act.is_none() {
        let gone = f.target.is_none_or(|t| seen.iter().find(|s| s.e == t).is_none_or(|s| !s.alive));
        if gone {
            match nearest_guard(seen, tf.translation, 10.0, true).filter(|s| s.hostile) {
                Some(t) => f.target = Some(t.e),
                None => leave_fight(lib, ch, tf, f),
            }
        }
    }
    let target = f.target.and_then(|t| seen.iter().find(|s| s.e == t)).copied();
    // The weapon hand.
    if input.hand_press {
        // The hidden blade, out of the fight: an assassination, if AC1's chooser finds one.
        if f.selected == Weapon::HiddenBlade && !f.fight && on_ground && f.act.is_none() {
            let me_pos = tf.translation;
            let pick = seen
                .iter()
                .filter(|s| s.guard && s.alive && !s.hostile && !s.busy)
                .filter_map(|s| {
                    let d = (s.pos - me_pos).with_y(0.0);
                    let at = fd::AssassinAt {
                        dist: d.length(),
                        rise: s.pos.y - me_pos.y,
                        in_front: angle_to(s.rot, s.pos, me_pos).abs() <= std::f32::consts::FRAC_PI_2,
                        low: !ctl.high,
                        running: seen.iter().find(|x| x.e == me).is_some_and(|x| x.speed > 4.0),
                    };
                    fd::assassination_type(at).map(|t| (s.e, t, d.length()))
                })
                .min_by(|a, b| a.2.total_cmp(&b.2));
            if let Some((v, t, d)) = pick {
                info!("combat: assassination type {t} at {d:.2} m");
                evs.push(Ev::Assassinate(me, v, t));
                return;
            }
        }
        if f.fight && f.drawn == Some(Weapon::Sword) {
            if ctl.high {
                // Defending: the counter, if a guard's attack on us is in its window.
                if let Some(a) = seen.iter().find(|s| s.guard && s.alive && s.busy && s.hostile && s.pos.distance(tf.translation) < 3.5) {
                    evs.push(Ev::Counter(me, a.e));
                }
            } else {
                match f.kind() {
                    Some(Kind::WindUp) => {
                        if let Some(s) = &mut f.swing {
                            s.early = true;
                        }
                    }
                    Some(Kind::Strike) => {
                        if f.in_combo_window(0.0) && f.swing.is_some_and(|s| !s.max && s.combos < fd::COMBO_HITS) {
                            if let Some(s) = &mut f.swing {
                                s.queued = true;
                            }
                            debug!("combat: combo press in the window");
                        }
                    }
                    None | Some(Kind::Stance) => f.hold = Some(0.0),
                    _ => {}
                }
            }
        } else if f.drawn == Some(Weapon::Sword)
            && on_ground
            && let Some(t) = nearest_guard(seen, tf.translation, 3.0, false)
        {
            // An attack near a guard out of the fight starts it.
            enter_fight(lib, ch, tf, f, t.e);
            f.act = None;
            f.hold = Some(0.0);
        }
    }
    // The attack itself: on the release (strong), or held long enough (max).
    if let Some(h) = f.hold.as_mut() {
        *h += dt;
        let max = *h >= fd::MAX_HOLD && input.hand_held;
        if (input.hand_release || !input.hand_held || max)
            && let Some(t) = target
        {
            f.hold = None;
            if matches!(f.kind(), None | Some(Kind::Stance)) {
                attack(lib, ch, tf, f, t, max);
            }
        } else if target.is_none() {
            f.hold = None;
        }
    }
}

/// A guard's choices (ours).
#[allow(clippy::too_many_arguments)]
fn guard_choices(lib: &mut AnimLib, seen: &[Seen], me: Entity, tf: &mut Transform, ch: &mut Character, f: &mut Fighter, g: &mut Guard, dt: f32) {
    let Some(player) = seen.iter().find(|s| s.player) else { return };
    let d = player.pos.distance(tf.translation);
    if !g.hostile {
        // A weapon drawn within 4 m in front of it, or the player fighting it: it turns hostile.
        let facing = angle_to(tf.rotation, tf.translation, player.pos).abs() < 1.2;
        if player.target == Some(me) || (d < 4.0 && facing && player.armed) {
            g.hostile = true;
        }
    }
    if !g.hostile || f.act.is_some() && !f.fight {
        return;
    }
    if !f.fight {
        info!("combat: guard turns hostile");
        f.drawn = Some(Weapon::Sword);
        ch.weapon_in_hand = Some(P_LEFTWAIST_TAG);
        enter_fight(lib, ch, tf, f, player.e);
    }
    f.target = Some(player.e);
    if f.act.is_none() && !g.passive {
        g.next_attack -= dt;
        if g.next_attack <= 0.0 && d < 2.6 {
            g.next_attack = 2.2 + g.rand() * 1.6;
            attack(lib, ch, tf, f, *player, true);
        }
    }
}

/// Into the fight stance, on `target`.
fn enter_fight(lib: &mut AnimLib, ch: &mut Character, tf: &Transform, f: &mut Fighter, target: Entity) {
    f.fight = true;
    f.target = Some(target);
    f.foot = usize::from(!ch.animator.as_ref().is_none_or(|a| a.lead_left()));
    let fo = FOOT[f.foot];
    let mut steps = vec![];
    steps.extend(step(lib, ch.rig_id, &format!("xx_h_wait_hipm_{fo}_tr_xx_h_light_wait_{fo}"), Kind::Stance, Root::Clip));
    f.act = Some(Act::new(steps, *tf, Then::Stance, None));
    ch.fighting = true;
    info!("combat: fight mode on");
}

/// Out of the fight stance, into the walk.
fn leave_fight(lib: &mut AnimLib, ch: &mut Character, tf: &Transform, f: &mut Fighter) {
    f.fight = false;
    f.target = None;
    f.swing = None;
    let fo = FOOT[f.foot];
    let steps: Vec<Step> = step(lib, ch.rig_id, &format!("xx_h_light_wait_{fo}_tr_xx_h_wait_hipm_{fo}"), Kind::Stance, Root::Clip).into_iter().collect();
    f.act = Some(Act::new(steps, *tf, Then::Locomotion, None));
    info!("combat: fight mode off");
}

/// Draw or sheathe `w` over the legs.
fn start_draw(lib: &mut AnimLib, ch: &mut Character, f: &mut Fighter, w: Weapon, draw: bool) {
    let foot = if ch.animator.as_ref().is_none_or(|a| a.lead_left()) { "footl" } else { "footr" };
    let (names, swap) = w.draw_clips(draw, foot);
    let clips: Vec<Arc<Clip>> = names.iter().filter_map(|n| clip(lib, ch.rig_id, n)).collect();
    info!("combat: {} the {}", if draw { "drawing" } else { "sheathing" }, w.name());
    if clips.is_empty() {
        f.drawn = draw.then_some(w);
        return;
    }
    ch.exit_fade = Some((ch.anim_pose.clone(), 0.15, 0.15));
    f.draw = Some((clips, 0, 0.0, swap, draw, w));
}

/// An attack on `t`: AC1's table pick (side, distance, foot; strong or max), the wind-up, the strike, and the follow
/// through with its way back to the stance.
fn attack(lib: &mut AnimLib, ch: &mut Character, tf: &mut Transform, f: &mut Fighter, t: Seen, max: bool) {
    let side = fd::side_of(angle_to(tf.rotation, tf.translation, t.pos));
    let dist = fd::distance_band(t.pos.distance(tf.translation));
    let (start, strike, dir) = fd::attack(max, side, dist, f.foot);
    let steps = swing_steps(lib, ch.rig_id, start, strike);
    if steps.is_empty() {
        return;
    }
    info!("combat: attack {} ({}, band {dist}, {})", steps[0].clip.name, fd::SIDES[side], if max { "max" } else { "strong" });
    f.swing = Some(Swing { target: t.e, dir, max, combos: 0, outcome: None, early: false, queued: false });
    f.act = Some(Act::new(steps, *tf, Then::Stance, None));
}

/// A swing's steps from its wind-up and strike actions: the wind-up, the strike, the follow-through (the strike's
/// second item, `_miss`) and its way back (`_miss_tr_wait`, the item's connector).
fn swing_steps(lib: &mut AnimLib, rig: usize, start: u32, strike: u32) -> Vec<Step> {
    let a = items(lib, start);
    let b = items(lib, strike);
    let mut out = vec![];
    if let Some(n) = a.first().and_then(|i| i.first()) {
        out.extend(step(lib, rig, n, Kind::WindUp, Root::Clip));
    }
    if let Some(n) = b.first().and_then(|i| i.first()) {
        out.extend(step(lib, rig, n, Kind::Strike, Root::Clip));
    }
    if let Some(n) = b.get(1).and_then(|i| i.first()) {
        out.extend(step(lib, rig, n, Kind::Recover, Root::Clip));
        let back = format!("{n}_tr_wait");
        if lib.names.contains(&back) {
            out.extend(step(lib, rig, &back, Kind::Recover, Root::Clip));
        }
    }
    out
}

// --- Moving -------------------------------------------------------------------------------------------------------

/// A fighter's frame: its move or stance, its root, the strikes it lands.
#[allow(clippy::too_many_arguments)]
fn tick(
    lib: &mut AnimLib,
    level: &Level,
    me: Entity,
    tf: &mut Transform,
    ch: &mut Character,
    ctl: &mut Controller,
    f: &mut Fighter,
    aim: Option<Vec3>,
    dt: f32,
    evs: &mut Vec<Ev>,
) {
    // The draw over the legs.
    if let Some((clips, i, t, swap, draw, w)) = &mut f.draw {
        if *i == *swap && *t == 0.0 {
            ch.weapon_in_hand = if *draw { w.carry_bone() } else { None };
            f.drawn = draw.then_some(*w);
            if !*draw && f.fight {
                f.fight = false;
            }
        }
        *t += dt;
        if *t >= clips[*i].anim.duration {
            *t = 0.0;
            *i += 1;
            if *i >= clips.len() {
                info!("combat: {} the {}", if *draw { "drew" } else { "sheathed" }, w.name());
                f.draw = None;
                if let Some(a) = ch.animator.as_mut() {
                    a.layer = None;
                }
                ch.exit_fade = Some((ch.anim_pose.clone(), 0.2, 0.2));
            }
        }
    }
    if !f.fight && f.act.is_none() {
        ch.fighting = false;
        return;
    }
    ch.fighting = true;
    // (A ground move playing is cut: the fight's clips take over, faded from it.)
    if ch.wall.take().is_some() {
        ch.exit_fade = Some((ch.anim_pose.clone(), 0.2, 0.2));
    }
    // (Nothing the ground takes from the keys while fighting.)
    ctl.legs_press = false;
    ctl.legs_release = false;
    ctl.toggle_climb = false;
    ctl.hand_climb = false;
    ctl.push = false;
    ctl.pickpocket = false;
    let from = tf.translation;
    if f.act.as_ref().is_none_or(|a| a.offset.is_none()) {
        ch.body_offset = None;
    }
    if let Some(act) = &mut f.act {
        act.t += dt;
        // Into the next step at the end of this one.
        while act.step().is_some_and(|s| act.t >= s.len) {
            let len = act.step().map_or(0.0, |s| s.len);
            // (A strike that landed, nothing queued: back to the stance at once, the follow-through is a miss's.)
            let ended = act.step().map(|s| s.kind);
            if ended == Some(Kind::Strike)
                && let Some(s) = f.swing
                && s.outcome.is_some_and(|o| o != Outcome::Missed)
            {
                if s.queued && s.outcome != Some(Outcome::Killed) {
                    act.steps.truncate(act.i + 1);
                    let next = fd::next_combo_dir(s.dir, s.outcome == Some(Outcome::Blocked));
                    act.t -= len;
                    act.i += 1;
                    // The one that would kill is the paired combo kill (settled with the victim).
                    evs.push(Ev::ComboKill(me, s.target, next));
                    let (start, strike) = fd::combo(next, s.max, f.foot);
                    let more = swing_steps(lib, ch.rig_id, start, strike);
                    info!("combat: combo hit {} {}", s.combos + 1, more.first().map_or("?", |m| m.clip.name.as_str()));
                    act.steps.extend(more);
                    f.swing = Some(Swing { dir: next, combos: s.combos + 1, outcome: None, early: false, queued: false, ..s });
                    act.anchor = *tf;
                    act.fresh = true;
                    continue;
                }
                act.steps.truncate(act.i + 1);
            }
            act.t -= len;
            act.i += 1;
            act.anchor = *tf;
            act.fresh = true;
        }
        let Some(s) = act.step().cloned() else {
            // The move is over.
            let then = act.then;
            f.act = None;
            if let Some(w) = &f.swing
                && w.outcome != Some(Outcome::Killed)
            {
                f.swing = None;
            }
            match then {
                Then::Stance => {}
                Then::Locomotion => {
                    f.fight = false;
                    ch.body_offset = None;
                    ch.fighting = false;
                    if let Some(a) = ch.animator.as_mut() {
                        a.forced = None;
                    }
                    ch.exit_fade = Some((ch.anim_pose.clone(), 0.2, 0.2));
                    return;
                }
                Then::Limp => {
                    f.dead = true;
                    ch.body_offset = None;
                    ch.fighting = false;
                    ch.limp = true;
                    if let Some(a) = ch.animator.as_mut() {
                        a.forced = None;
                    }
                    info!("combat: limp (the ragdoll)");
                    return;
                }
            }
            // (Back into the stance, faded from the move's last pose.)
            f.stance = None;
            stance(lib, tf, ch, ctl, f, aim, dt);
            return;
        };
        if act.fresh {
            act.fresh = false;
            if s.kind == Kind::Paired
                && let Some(r) = act.pair_root.take()
            {
                // The pair's root: both sides play the paired clip from the victim's (the offset carries the body
                // there, the root stays put).
                act.offset = Some(iso_mul(&iso_inv(tf), &Transform { scale: Vec3::ONE, ..r }));
            }
            // (Steps played in the pair's frame start from the root moved there.)
            if let Some(o) = act.offset
                && act.i <= act.last_paired().unwrap_or(0) + 1
            {
                act.anchor = iso_mul(tf, &o);
            }
            if s.kind == Kind::Strike
                && let Some(w) = &f.swing
            {
                evs.push(Ev::Strike(me, w.target, w.dir, w.max, w.combos > 0));
            }
            if act.i > 0 || s.blend > 0.0 {
                ch.exit_fade = Some((ch.anim_pose.clone(), s.blend, s.blend));
            }
            // A wind-up closes or opens the distance so that its strike ends `CONTACT` from the target (ours: AC1 steers
            // its discrete moves to a computed place, sub_711130; the place for an attack is not found).
            if s.kind == Kind::WindUp
                && let (Some(to), Some(next)) = (aim, act.steps.get(act.i + 1))
            {
                let reach = root_motion_at(&next.clip, next.clip.frames()).truncate().length();
                let way = (to - tf.translation).with_y(0.0);
                let goal = to - way.normalize_or_zero() * (CONTACT + reach);
                act.steps[act.i].root = Root::Line(goal.with_y(tf.translation.y));
            }
            debug!("combat: {:?} {}", s.kind, s.clip.name);
        }
        let root = act.steps[act.i].root;
        let t = s.frozen.unwrap_or(act.t.min(s.clip.anim.duration));
        let r = match root {
            Root::Clip => clip_root(&act.anchor, &s.clip, t),
            Root::Steer(to, rot) => {
                let k = (act.t / s.len.max(1e-3)).clamp(0.0, 1.0);
                Transform { translation: act.anchor.translation.lerp(to, k), rotation: act.anchor.rotation.slerp(rot, k), scale: tf.scale }
            }
            Root::Line(goal) => {
                // (Along the clip's own progress, so it keeps its timing; a clip that stands still, evenly.)
                let all = root_motion_at(&s.clip, s.clip.frames()).truncate().length();
                let k = if all > 0.05 {
                    (root_motion_at(&s.clip, t * FPS).truncate().length() / all).clamp(0.0, 1.0)
                } else {
                    (act.t / s.len.max(1e-3)).clamp(0.0, 1.0)
                };
                Transform { translation: act.anchor.translation.lerp(goal, k), ..*tf }
            }
            Root::Hold => *tf,
        };
        // Played in the pair's frame: the root is the pair's root moved back by the offset, which eases out over the
        // step after the last paired one.
        let r = match act.offset {
            Some(o) => {
                let last = act.last_paired().unwrap_or(0);
                let k = if act.i <= last {
                    0.0
                } else if act.i == last + 1 {
                    (act.t / s.len.max(1e-3)).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let o = Transform { translation: o.translation.lerp(Vec3::ZERO, k), rotation: o.rotation.slerp(Quat::IDENTITY, k), scale: Vec3::ONE };
                ch.body_offset = Some(body_offset(&o, tf.scale.x));
                iso_mul(&r, &iso_inv(&o))
            }
            None => r,
        };
        tf.translation = r.translation;
        tf.rotation = r.rotation;
        // Facing the target through stance moves and swings' wind-ups (they aim at it): the step's anchor turns with it.
        if matches!(s.kind, Kind::Stance | Kind::WindUp)
            && f.fight
            && let Some(to) = aim
        {
            let before = tf.rotation;
            face(tf, to, TURN_RATE, dt);
            let turn = tf.rotation * before.inverse();
            act.anchor.rotation = (turn * act.anchor.rotation).normalize();
            act.anchor.translation = tf.translation + turn * (act.anchor.translation - tf.translation);
        }
    } else {
        stance(lib, tf, ch, ctl, f, aim, dt);
    }
    // On the ground, out of walls (not through a paired clip: the pair would come apart).
    let paired = matches!(f.kind(), Some(Kind::Paired | Kind::Lead | Kind::Death));
    if let Some(g) = level.ground(tf.translation, 1.0, 2.0) {
        tf.translation.y = g.point.y;
    }
    if !paired {
        let push = level.capsule_push(tf.translation).with_y(0.0);
        tf.translation += push;
        if let Some(a) = &mut f.act {
            a.anchor.translation += push;
        }
    }
    ch.velocity = (tf.translation - from) / dt.max(1e-4);
}

/// The stance: the wait, or the walk in it the stick asks for, facing the target; the defence's in high profile.
fn stance(lib: &mut AnimLib, tf: &mut Transform, ch: &mut Character, ctl: &Controller, f: &mut Fighter, aim: Option<Vec3>, dt: f32) {
    if let Some(to) = aim {
        face(tf, to, TURN_RATE, dt);
    }
    let defend = ctl.high;
    let stick = ctl.move_dir.with_y(0.0);
    let moving = stick.length() > 0.3;
    let fo = FOOT[f.foot];
    // AC1's stance actions (`fight_data`): the walk's blend space by the stick's way round the facing, the defence's
    // wait; the light wait (0x00FD645B / 0x0108295C) lists no items, only its idle variants: its first clip by name.
    let from_table = |lib: &AnimLib, id: u32, k: usize| items(lib, id).first().and_then(|i| i.get(k).or(i.first()).cloned());
    let name = if moving {
        let k = fd::strafe_dir(angle_to(tf.rotation, Vec3::ZERO, stick));
        from_table(lib, if defend { fd::DEFENCE_STRAFE[f.foot] } else { fd::STRAFE[f.foot] }, k)
            .or_else(|| Some(format!("xx_h_light_strafe_{}_{fo}_MOCAP", fd::STRAFE_DIRS[k])))
    } else if defend {
        from_table(lib, fd::DEFENCE_WAIT[f.foot], 0)
    } else {
        None
    };
    let name = name.unwrap_or_else(|| format!("xx_h_light_wait_{fo}"));
    if f.stance.as_ref().is_none_or(|(c, _)| c.name != name)
        && let Some(c) = clip(lib, ch.rig_id, &name)
    {
        ch.exit_fade = Some((ch.anim_pose.clone(), 0.2, 0.2));
        f.stance = Some((c, 0.0));
    }
    if let Some((c, t)) = &mut f.stance {
        let d = c.anim.duration.max(1e-3);
        let t0 = *t;
        let t1 = t0 + dt;
        // The walk's root motion (wrapping round the loop).
        let p = |x: f32| root_motion_at(c, x * FPS);
        let step = if t1 < d { p(t1) - p(t0) } else { p(d) - p(t0) + p(t1 - d) - p(0.0) };
        *t = t1 % d;
        tf.translation += tf.rotation * (model_to_bevy() * step);
    }
}

/// How fast a fighter turns to face its target (rad/s; ours: AC1 turns with its 45/90/180 degree clips).
const TURN_RATE: f32 = 6.0;

/// Face `to` from the fighter's root, at most `rate` rad/s.
fn face(tf: &mut Transform, to: Vec3, rate: f32, dt: f32) {
    let a = angle_to(tf.rotation, tf.translation, to);
    let turn = a.clamp(-rate * dt, rate * dt);
    tf.rotation = (Quat::from_rotation_y(turn) * tf.rotation).normalize();
}

// --- Between two ------------------------------------------------------------------------------------------------

fn settle(lib: &mut AnimLib, q: &mut FighterQuery, ev: Ev) {
    match ev {
        Ev::Strike(by, on, dir, max, combo) => strike(lib, q, by, on, dir, max, combo),
        Ev::Counter(by, on) => counter(lib, q, by, on),
        Ev::Assassinate(by, on, t) => assassinate(lib, q, by, on, t),
        Ev::ComboKill(by, on, dir) => combo_kill(lib, q, by, on, dir),
    }
}

/// A strike lands on `on`: blocked, a hit, or its death.
#[allow(clippy::too_many_arguments)]
fn strike(lib: &mut AnimLib, q: &mut FighterQuery, by: Entity, on: Entity, dir: u8, max: bool, combo: bool) {
    let Ok([(_, atf, _, _, mut af, _, _), (_, mut vtf, mut vch, vctl, mut vf, vg, v_player)]) = q.get_many_mut([by, on]) else { return };
    let reach = vtf.translation.distance(atf.translation);
    let angle = angle_to(atf.rotation, atf.translation, vtf.translation);
    let ahead = angle.abs() < 1.0;
    debug!("combat: strike at {reach:.2} m, {:.0} degrees off", angle.to_degrees());
    let outcome = if vf.dead || reach > 3.2 || !ahead {
        Outcome::Missed
    } else {
        // Blocking: the player defending in the stance, a guard by chance (never a timed combo hit, ours).
        let free = matches!(vf.kind(), None | Some(Kind::Stance | Kind::Block));
        let blocks = if v_player {
            vctl.high && vf.fight && free
        } else if let Some(mut g) = vg {
            g.hostile = true;
            let roll = g.rand();
            free && !combo && roll < g.block
        } else {
            false
        };
        if blocks {
            Outcome::Blocked
        } else {
            vf.hp -= 1;
            if vf.hp <= 0 && !v_player { Outcome::Killed } else { Outcome::Hit }
        }
    };
    if let Some(s) = af.swing.as_mut() {
        s.outcome = Some(outcome);
    }
    let (vtf, vch, vf) = (&mut *vtf, &mut *vch, &mut *vf);
    let rig = vch.rig_id;
    let fo = FOOT[vf.foot];
    let d = fd::DIRS[dir as usize];
    match outcome {
        Outcome::Missed => info!("combat: the strike misses"),
        Outcome::Blocked => {
            info!("combat: the strike is blocked");
            let set = if max { "max" } else { "strong" };
            let name = format!("xx_h_light_block_{set}_rec_{d}_{fo}");
            let name = if lib.names.contains(&name) { name } else { format!("xx_h_light_block_strong_rec_{d}_{fo}") };
            let mut steps: Vec<Step> = step(lib, rig, &name, Kind::Block, Root::Clip).into_iter().collect();
            steps.extend(step(lib, rig, &format!("{name}_tr_wait"), Kind::Recover, Root::Clip));
            vf.act = Some(Act::new(steps, *vtf, Then::Stance, None));
        }
        Outcome::Hit => {
            let id = fd::hit_reaction(dir, max || combo, vf.foot);
            let names = items(lib, id);
            let first = names.first().and_then(|i| i.first()).cloned();
            info!("combat: the strike hits ({} left) {}", vf.hp, first.as_deref().unwrap_or("?"));
            if v_player {
                vch.health = (vch.health - 0.15).max(0.1);
            }
            let mut steps: Vec<Step> = first.iter().filter_map(|n| step(lib, rig, n, Kind::Hit, Root::Clip)).collect();
            if let Some(n) = first.map(|n| n.trim_end_matches("_a").to_string() + "_tr_wait")
                && lib.names.contains(&n)
            {
                steps.extend(step(lib, rig, &n, Kind::Recover, Root::Clip));
            }
            vf.swing = None;
            vf.act = Some(Act::new(steps, *vtf, Then::Stance, None));
        }
        Outcome::Killed => {
            // AC1's wound deaths (`HumanGround_Fight_Hurt`): the blow from the attacker's right lands on the left (`sl`).
            let side = if dir == 2 || dir == 3 { "sl" } else { "sr" };
            let name = format!("xx_h_light_death_wound_front_{side}_{fo}");
            info!("combat: the strike kills: {name}");
            let steps: Vec<Step> = step(lib, rig, &name, Kind::Death, Root::Clip).into_iter().collect();
            vf.swing = None;
            vf.fight = false;
            vf.act = Some(Act::new(steps, *vtf, Then::Limp, None));
        }
    }
}

/// A timed combo hit on a guard that it would kill: AC1's paired combo kill (`HumanGround_ComboKill`) in place of the
/// hit; else nothing (the combo hit plays).
fn combo_kill(lib: &mut AnimLib, q: &mut FighterQuery, by: Entity, on: Entity, dir: u8) {
    let Ok([(_, atf, ach, _, mut af, _, _), (_, vtf, vch, _, mut vf, _, v_player)]) = q.get_many_mut([by, on]) else { return };
    if v_player || vf.dead || vf.hp > 1 || vtf.translation.distance(atf.translation) > 3.0 {
        return;
    }
    let (n, kd, vd) = fd::combo_kill(dir);
    let kp = format!("xx_h_light_combo_light_0{n}_kill_{kd}_");
    let vp = format!("xx_h_light_combo_light_0{n}_");
    let pick = |pre: &str, end: &str| find_clip(lib, pre, |rest| rest.ends_with(end));
    let (Some(start), Some(pos), Some(ka), Some(kb), Some(pose), Some(da), Some(db)) = (
        pick(&kp, "start_footl").or_else(|| pick(&kp, "start_footr")),
        pick(&kp, "position_footl").or_else(|| pick(&kp, "position_footr")),
        pick(&kp, "_a"),
        pick(&kp, "_b"),
        pick(&format!("{vp}death_pose_{vd}_"), ""),
        pick(&format!("{vp}death_{vd}_"), "_a"),
        pick(&format!("{vp}death_{vd}_"), "_b"),
    ) else {
        warn!("combat: no combo kill clips for {kp}");
        return;
    };
    let tr = pick(&kp, "tr_wait");
    let (ar, vr) = (ach.rig_id, vch.rig_id);
    let (Some(ka_c), Some(pos_c)) = (clip(lib, ar, &ka), clip(lib, ar, &pos)) else { return };
    // The victim waits in its death pose while the killer winds up and steps in; the pair's root is the victim's.
    let start_s = step(lib, ar, &start, Kind::Lead, Root::Clip);
    let lead_len = start_s.as_ref().map_or(0.0, |s| s.len) + pos_c.anim.duration;
    let pair = *vtf;
    // (The killer's lead-in: its wind-up as it is, then the step in steered so the body meets the kill.)
    let (to, rot) = meet(&pair, &ka_c, &pos_c);
    let mut ksteps: Vec<Step> = start_s.into_iter().collect();
    ksteps.push(Step { clip: pos_c.clone(), kind: Kind::Lead, root: Root::Steer(to, rot), len: pos_c.anim.duration, frozen: None, blend: 0.1 });
    ksteps.extend(step(lib, ar, &ka, Kind::Paired, Root::Clip));
    ksteps.extend(step(lib, ar, &kb, Kind::Paired, Root::Clip));
    if let Some(tr) = tr {
        ksteps.extend(step(lib, ar, &tr, Kind::Recover, Root::Clip));
    }
    let mut vsteps: Vec<Step> = vec![];
    if let Some(mut s) = step(lib, vr, &pose, Kind::Lead, Root::Hold) {
        s.len = lead_len;
        vsteps.push(s);
    }
    vsteps.extend(step(lib, vr, &da, Kind::Paired, Root::Clip));
    vsteps.extend(step(lib, vr, &db, Kind::Death, Root::Clip));
    info!("combat: combo kill {ka} against {da}");
    vf.hp = 0;
    vf.fight = false;
    vf.swing = None;
    vf.act = Some(Act::new(vsteps, *vtf, Then::Limp, Some(pair)));
    if let Some(s) = af.swing.as_mut() {
        s.outcome = Some(Outcome::Killed);
    }
    let anchor = *atf;
    af.act = Some(Act::new(ksteps, anchor, Then::Stance, Some(pair)));
}

/// A counter kill (`HumanGround_CounterKills`): the defender steps in on the attacker's swing; the pair plays from the
/// attacker's root.
fn counter(lib: &mut AnimLib, q: &mut FighterQuery, by: Entity, on: Entity) {
    let Ok([(_, atf, ach, _, mut af, _, _), (_, vtf, vch, _, mut vf, _, _)]) = q.get_many_mut([by, on]) else { return };
    if !vf.counter_open_for(0.0) || af.act.as_ref().is_some_and(|a| a.step().is_some_and(|s| s.kind != Kind::Stance)) {
        info!("combat: the counter is mistimed");
        return;
    }
    let dir = vf.swing.map_or(3, |s| s.dir);
    let d = fd::DIRS[dir as usize];
    let kp = format!("xx_h_light_counter_light_01_kill_{d}_");
    let vp = format!("xx_h_light_countered_light_01_death_{d}_");
    let pick = |pre: &str, f: &dyn Fn(&str) -> bool| find_clip(lib, pre, f);
    let (Some(st), Some(ka), Some(kb), Some(da), Some(db)) = (
        pick(&kp, &|r: &str| r.starts_with("step")),
        pick(&kp, &|r: &str| r.starts_with("position") && r.ends_with("_a")),
        pick(&kp, &|r: &str| r.starts_with("position") && r.ends_with("_b")),
        pick(&vp, &|r: &str| r.ends_with("_a")),
        pick(&vp, &|r: &str| r.ends_with("_b")),
    ) else {
        warn!("combat: no counter clips for {kp}");
        return;
    };
    let tr = pick(&kp, &|r: &str| r.contains("tr_wait") || r.contains("to_wait"));
    let (ar, vr) = (ach.rig_id, vch.rig_id);
    let (Some(st_c), Some(ka_c)) = (clip(lib, ar, &st), clip(lib, ar, &ka)) else { return };
    let pair = *vtf;
    let (to, rot) = meet(&pair, &ka_c, &st_c);
    let mut ksteps = vec![Step { len: st_c.anim.duration, clip: st_c.clone(), kind: Kind::Lead, root: Root::Steer(to, rot), frozen: None, blend: 0.1 }];
    ksteps.extend(step(lib, ar, &ka, Kind::Paired, Root::Clip));
    ksteps.extend(step(lib, ar, &kb, Kind::Paired, Root::Clip));
    if let Some(tr) = tr {
        ksteps.extend(step(lib, ar, &tr, Kind::Recover, Root::Clip));
    }
    // The attacker holds its swing's pose while the defender steps in.
    let mut vsteps = vec![];
    if let Some((c, t)) = vf.act.as_ref().and_then(|a| a.step().map(|s| (s.clip.clone(), a.t.min(s.clip.anim.duration)))) {
        vsteps.push(Step { len: st_c.anim.duration, clip: c, kind: Kind::Lead, root: Root::Hold, frozen: Some(t), blend: 0.0 });
    }
    vsteps.extend(step(lib, vr, &da, Kind::Paired, Root::Clip));
    vsteps.extend(step(lib, vr, &db, Kind::Death, Root::Clip));
    info!("combat: counter kill {ka} against {da}");
    vf.hp = 0;
    vf.fight = false;
    vf.swing = None;
    vf.act = Some(Act::new(vsteps, *vtf, Then::Limp, Some(pair)));
    af.swing = None;
    af.act = Some(Act::new(ksteps, *atf, Then::Stance, Some(pair)));
}

/// An assassination of type `t` (`fight_data::ASSASSINATION`): the assassin's lead-in (its variants blended by the
/// side it comes from, steered so the body meets the kill), the victim's; then both kills from the victim's root.
fn assassinate(lib: &mut AnimLib, q: &mut FighterQuery, by: Entity, on: Entity, t: u8) {
    let Ok([(_, atf, mut ach, _, mut af, _, _), (_, vtf, vch, _, mut vf, _, _)]) = q.get_many_mut([by, on]) else { return };
    let [a_lead, a_kill, v_lead, v_kill] = fd::ASSASSINATION[t as usize];
    let (ar, vr) = (ach.rig_id, vch.rig_id);
    let al = items(lib, a_lead);
    let ak = items(lib, a_kill);
    let vl = items(lib, v_lead);
    let vk = items(lib, v_kill);
    // The assassin's lead-in blend (f, r, l) by where it stands against the victim (0xF6D140).
    let ang = angle_to(vtf.rotation, vtf.translation, atf.translation);
    let w = fd::lead_in_weights(t, ang.abs(), ang > 0.0);
    let lead_name = al.first().map(|v| {
        let parts: Vec<(&str, f32)> = v.iter().map(String::as_str).zip(w).collect();
        mix_name(&parts)
    });
    let (Some(lead_name), Some(kill_a)) = (lead_name, ak.first().and_then(|i| i.first()).cloned()) else {
        warn!("combat: assassination {t}: no clips");
        return;
    };
    let (Some(lead_c), Some(kill_c)) = (clip(lib, ar, &lead_name), clip(lib, ar, &kill_a)) else { return };
    // The victim's lead-in, then where its root is when the kill starts.
    let mut vsteps: Vec<Step> = vl.first().and_then(|i| i.first()).and_then(|n| step(lib, vr, n, Kind::Lead, Root::Clip)).into_iter().collect();
    if let Some(s) = vsteps.first_mut() {
        s.len = s.len.max(lead_c.anim.duration);
    }
    let pair = after(&vtf, &vsteps);
    let (to, rot) = meet(&pair, &kill_c, &lead_c);
    let mut asteps = vec![Step { len: lead_c.anim.duration, clip: lead_c.clone(), kind: Kind::Lead, root: Root::Steer(to, rot), frozen: None, blend: 0.15 }];
    for (k, i) in ak.iter().enumerate() {
        if let Some(n) = i.first() {
            asteps.extend(step(lib, ar, n, if k == 0 { Kind::Paired } else { Kind::Recover }, Root::Clip));
        }
    }
    for (k, i) in vk.iter().enumerate() {
        if let Some(n) = i.first() {
            vsteps.extend(step(lib, vr, n, if k == 0 { Kind::Paired } else { Kind::Death }, Root::Clip));
        }
    }
    info!("combat: assassination {} then {} against {}", lead_c.name, kill_a, vk.first().and_then(|i| i.first()).map_or("?", String::as_str));
    vf.hp = 0;
    vf.fight = false;
    vf.act = Some(Act::new(vsteps, *vtf, Then::Limp, Some(pair)));
    ach.fighting = true;
    af.act = Some(Act::new(asteps, *atf, Then::Locomotion, Some(pair)));
}

// --- Showing ------------------------------------------------------------------------------------------------------

/// What the fighter shows this frame: its move or stance clip (whole body) and the draw over it.
fn show(_lib: &mut AnimLib, ch: &mut Character, f: &mut Fighter, dt: f32) {
    // (The animator adds the frame's time before it samples: set back by it.)
    let back = dt.clamp(1e-4, 0.05);
    if let Some((clips, i, t, _, _, _)) = &f.draw
        && let (Some(c), Some(a)) = (clips.get(*i), ch.animator.as_mut())
    {
        a.layer = Some((c.clone(), (*t - back).max(0.0)));
    }
    // (A move started after the moves this frame, a reaction or a paired one: shown from the next frame, when its fade
    // starts.)
    if !ch.fighting || f.dead && f.act.is_none() || f.act.as_ref().is_some_and(|a| a.fresh) {
        return;
    }
    let shown = match &f.act {
        Some(a) => a.step().map(|s| (s.clip.clone(), s.frozen.unwrap_or(a.t.min(s.clip.anim.duration - 1e-3)))),
        None => f.stance.as_ref().map(|(c, t)| (c.clone(), *t)),
    };
    if let (Some((c, t)), Some(a)) = (shown, ch.animator.as_mut()) {
        // (Its foot, for the next pick.)
        f.foot = end_foot(&c.name, f.foot);
        a.forced = Some((c, (t - back).max(0.0)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yaw_of(q: Quat) -> f32 {
        let f = q * Vec3::NEG_Z;
        (-f.x).atan2(-f.z)
    }

    #[test]
    fn feet_from_names() {
        assert_eq!(end_foot("xx_h_light_combo_ur_footl_to_footr", 0), 1);
        assert_eq!(end_foot("xx_h_light_combo_dr_footl", 1), 0);
        assert_eq!(end_foot("xx_h_light_wait_footr", 0), 1);
        assert_eq!(end_foot("xx_h_unsheath_upper_body", 1), 1);
    }

    #[test]
    fn yaw_round_trip() {
        for y in [-2.0f32, -0.5, 0.0, 1.0, 3.0] {
            assert!((yaw_of(yaw_rot(y)) - y).abs() < 1e-4);
        }
    }

    #[test]
    fn angles_to_the_side() {
        // Facing -Z: a point at -X is to the left.
        let a = angle_to(Quat::IDENTITY, Vec3::ZERO, Vec3::new(-1.0, 0.0, 0.0));
        assert!((a - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
    }
}
