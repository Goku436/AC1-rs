//! Combat on AC1's own data: weapons drawn and sheathed, the fight stance locked on a target with AC1's turns, the walk
//! in stance, attack chains with AC1's combo state machine, blocks, deflects, the counter kill and the counter dodge,
//! mobile attacks and throwing knives, the hidden blade's assassinations (on the ground and from the air), guards that
//! fight back in turns, the player's damage and desynchronisation. The moves are AC1's actions (`HumanWeapon`,
//! `HumanGround_Fight`, `_ComboKill`, `_CounterKills`, `_CounterDodge`, `_Fight_Light`, `_Fight_Unarmed`,
//! `Human_Ground_Fight_Dagger`, `_Assassination`, `_Fight_Hurt`, `HumanInAir`), picked by the executable's tables
//! (`fight_tables`, `fight_data`) and played as the move graph's items (`MoveGraph::action_clips`).
//!
//! How it plays (what is AC1's and what is ours is in docs/COMBAT.md):
//! - Weapons (the manual's keys): 1 short blade, 2 hidden blade, 3 sword, 4 fists. A press selects the weapon (the one
//!   in hand goes back first), a second press draws it, a third sheathes it. The sword draws with `HumanWeapon`'s
//!   0x0C1BB807 and sheathes with 0x0C1BB7D6, over the legs (upper body only); the short blade with its `knife` pair;
//!   fists with `xx_h_unarmed_select_<foot>`.
//! - F (target lock): the fight mode with the weapon in hand (the sword drawn if none is), locked on the nearest guard,
//!   into the weapon's stance; F again leaves it. In the stance the stick walks the weapon's 8-way walk facing the
//!   target; standing, a target over 30 degrees off is faced with AC1's turn clips (45, 90, 180, scaled to the angle);
//!   high profile held defends.
//! - The weapon hand: an attack picked by AC1's tables (side, distance, foot; the sword's straight-ahead medium and long
//!   ones by a random blade direction; a target over 0.5 m higher or lower the high or low one), strong on a tap, max
//!   when held. Its strike lands as the strike item starts: the target blocks, deflects, is hurt or dies; the attacker
//!   plays AC1's `hit` or `blocked` clip after the strike. A press from the strike's start through that clip (AC1's
//!   latch, cleared as the strike starts) plays the next combo hit when the clip ends, by AC1's direction tables, up to
//!   `COMBO_HITS`; the one that would kill is AC1's paired combo kill for its direction.
//! - Defending (high profile), as a guard's blow comes: the weapon hand counter-kills (AC1's `_01` / `_02` set by the
//!   foot ahead), the legs counter-dodge to the stick's side.
//! - Running with a blade, the weapon hand at a guard ahead: AC1's mobile attack (drawing the sword as it strikes if it
//!   is sheathed). The short blade at a target past its medium band: a throwing knife.
//! - The hidden blade selected, the weapon hand near a guard that is not fighting: the assassination AC1's chooser
//!   picks, played paired; in the air over a guard (a jump, a ledge, a beam): the air assassination.
//! - Guards (ours, simple; any entity with `Guard` and `Fighter`): stand until they see a weapon drawn near them, are
//!   locked on or attacked, or a guard near them fights; then fight: face the player, attack in turns (one at a time
//!   by default), block or deflect some first attacks, strike back at once after a deflect.
//! - The player hurt by a blow loses health (the HUD's bar); at none, the death clip, limp, and desynchronised: back at
//!   the spawn point with full health.
//!
//! Script hooks (env): `AC1_GUARDS="x,z,yaw;..."` (guards there; in the test level two stand in the fight yard by
//! default, scripted runs have none unless asked), `AC1_GUARD_BLOCK=0..1` (the chance a guard blocks a first attack,
//! 0.5), `AC1_GUARD_DEFLECT=0..1` (the chance it deflects one instead, 0), `AC1_GUARD_PASSIVE=1` (guards never attack),
//! `AC1_GUARD_HP=n`, `AC1_GUARD_AWARE=1` (hostile from the start), `AC1_FIGHT_ATTACKERS=n` (guards attacking at once, 1),
//! `AC1_PLAYER_HEALTH=0..1`, `AC1_WEAPON="secs:key,..."` (a weapon key pressed then), `AC1_LOCK="secs,..."` (F pressed),
//! `AC1_ATTACK="secs[:held],...,combo,counter,dodge"` (the weapon hand: at a time, held that long; `combo` presses in
//! the middle of the next combo window, `counter` as the next counter window opens, `dodge` presses the legs then),
//! `AC1_FIGHT_TRACE=1` (every fight frame's clip and root in the log).

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
    /// Its index in AC1's fight tables (`fight_data::FISTS` ...).
    pub fn index(self) -> usize {
        match self {
            Weapon::Fists => fd::FISTS,
            Weapon::Sword => fd::SWORD,
            Weapon::ShortBlade => fd::SHORT_BLADE,
            Weapon::HiddenBlade => fd::HIDDEN_BLADE,
        }
    }
    /// It fights in the stance (the hidden blade assassinates).
    fn fights(self) -> bool {
        self != Weapon::HiddenBlade
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
    /// The legs pressed this frame (the counter dodge; the ground's own legs input is not read in a fight).
    pub legs_press: bool,
}

/// The scripted fight input (`AC1_WEAPON`, `AC1_LOCK`, `AC1_ATTACK`) and its clock.
#[derive(Resource, Default)]
struct FightScript {
    loaded: bool,
    t: f32,
    weapons: Vec<(f32, u8)>,
    locks: Vec<f32>,
    /// The weapon hand: (when, how long held) or a cue (`combo`, `counter`, `dodge`), in order.
    hand: Vec<HandCue>,
    held_until: f32,
}

#[derive(Clone, Copy, Debug)]
enum HandCue {
    At(f32, f32),
    Combo,
    Counter,
    Dodge,
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
            "dodge" => HandCue::Dodge,
            _ => {
                let (t, h) = p.split_once(':').unwrap_or((p, "0.05"));
                HandCue::At(t.parse().unwrap_or(0.0), h.parse().unwrap_or(0.05))
            }
        });
    }
}

/// The fight's keys (AC1's PC defaults: the left button is the weapon hand, F the target lock, 1-4 the weapons, Space
/// the legs), or the script's presses.
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
        // The weapon hand (or the legs): the next cue, when it is due.
        let counter_open = || fighters.iter().any(|f| f.counter_open_for(0.25));
        let (due, legs) = match script.hand.first().copied() {
            Some(HandCue::At(t, h)) => ((t1 >= t).then_some(h), false),
            Some(HandCue::Combo) => (player.iter().next().is_some_and(|f| f.in_combo_window(0.5)).then_some(0.05), false),
            Some(HandCue::Counter) => (counter_open().then_some(0.05), false),
            Some(HandCue::Dodge) => (counter_open().then_some(0.05), true),
            None => (None, false),
        };
        if let Some(h) = due {
            script.hand.remove(0);
            if legs {
                input.legs_press = true;
                debug!("combat: script presses the legs at {t1:.2} s");
            } else {
                input.hand_press = true;
                script.held_until = t1 + h;
                debug!("combat: script presses the weapon hand at {t1:.2} s");
            }
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
    input.legs_press = keys.just_pressed(KeyCode::Space);
}

// --- Fighters -----------------------------------------------------------------------------------------------------

/// What a step of a move is, for the fight's rules.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// Into or out of the stance.
    Stance,
    /// AC1's turn on the spot in the stance.
    Turn,
    WindUp,
    Strike,
    /// What the attacker plays as its strike lands (AC1's `hit` or `blocked` clip): the combo window's end.
    Landed,
    Recover,
    Hit,
    Block,
    /// The counter dodge.
    Dodge,
    /// A throwing knife's throw (its release lets the knife go).
    Throw,
    /// A lead-in before a paired clip (steered to meet it).
    Lead,
    /// The air assassination's jump (steered onto the victim, through the air).
    Air,
    Paired,
    Death,
}

/// How a step moves the root.
#[derive(Clone, Copy, Debug)]
enum Root {
    /// The clip's root motion from where the step started.
    Clip,
    /// The clip's root motion with its turn scaled (AC1's turns on the spot, 0xF38D10).
    Turn(f32),
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
    /// The root both sides of a paired clip play from (the victim's), taken when the first paired step starts.
    pair_root: Option<Transform>,
    /// AC1 puts the root on the pair's as the paired clip starts (sub_52AAF0): ours slides it there over `SLIDE` s,
    /// the body kept on the pair's root all along (the offset), so neither jumps: where the root was in the pair's
    /// frame, and the time since.
    slide: Option<(Transform, f32)>,
    /// The step started this frame (its first frame's events).
    fresh: bool,
}

/// How long the root takes to slide onto the pair's (s, ours).
const SLIDE: f32 = 0.3;

impl Act {
    fn new(steps: Vec<Step>, anchor: Transform, then: Then, pair_root: Option<Transform>) -> Act {
        Act { steps, i: 0, t: 0.0, anchor, then, pair_root, slide: None, fresh: true }
    }

    fn step(&self) -> Option<&Step> {
        self.steps.get(self.i)
    }
}

/// A swing in progress: what the fight rules need to know about it.
#[derive(Clone, Copy, Debug)]
struct Swing {
    target: Entity,
    /// AC1's weapon index, the direction the blade ends in (the strike record's), a max attack, combo hits so far.
    weapon: usize,
    dir: u8,
    max: bool,
    combos: u32,
    /// What the strike met: `None` not yet.
    outcome: Option<Outcome>,
    /// The weapon hand pressed since the strike started (AC1's latch +0x805, cleared as the strike starts).
    latched: bool,
    /// No combo may follow (AC1's +0xA0A: a high or low attack, a mobile one, a throw).
    no_combo: bool,
    /// A throwing knife (its strike is the knife's flight).
    knife: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Outcome {
    Hit,
    Blocked,
    Deflected,
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
    /// The blade direction the sword's last random attack took (AC1's +0x9DC), and a dice for the next.
    last_random: Option<u8>,
    rng: u32,
    /// Since the last strong fist attack (s): a second one within `fight_data::FIST_CHAIN` is a max one.
    fist_chain: f32,
    /// Throwing knives left.
    pub knives: u32,
    /// A knife in flight: its target and the time to its landing.
    knife: Option<(Entity, f32)>,
    /// Lying dead (the player): how long, before the desynchronisation starts it again.
    dead_t: f32,
}

impl Fighter {
    pub fn new(selected: Weapon, hp: i32) -> Self {
        Fighter {
            selected,
            drawn: None,
            fight: false,
            target: None,
            hp,
            dead: false,
            act: None,
            swing: None,
            foot: 0,
            stance: None,
            draw: None,
            hold: None,
            last_random: None,
            rng: 0x2545_f491 ^ hp as u32,
            fist_chain: f32::MAX,
            knives: KNIVES,
            knife: None,
            dead_t: 0.0,
        }
    }

    /// Idle: no move playing (the stance or out of the fight). The street militia's patrol moves a guard only then.
    #[allow(dead_code)]
    pub fn idle(&self) -> bool {
        self.act.is_none() && !self.dead
    }

    fn kind(&self) -> Option<Kind> {
        self.act.as_ref().and_then(|a| a.step()).map(|s| s.kind)
    }

    fn roll(&mut self) -> u8 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 24) as u8
    }

    /// The weapon in hand's table index (the sword's when none is: the guards').
    fn weapon(&self) -> usize {
        self.drawn.filter(|w| w.fights()).unwrap_or(Weapon::Sword).index()
    }

    /// In the combo window, `at` 0..1 of the way through the landed clip: the strike landed (hit or blocked) and its
    /// `hit` / `blocked` clip plays, nothing pressed yet.
    fn in_combo_window(&self, at: f32) -> bool {
        let Some(a) = &self.act else { return false };
        let Some(s) = a.step() else { return false };
        s.kind == Kind::Landed && a.t >= s.len * at && self.swing.is_some_and(|w| matches!(w.outcome, Some(Outcome::Hit | Outcome::Blocked)) && !w.latched)
    }

    /// An attack in the counter window (or `lead` s into it).
    fn counter_open_for(&self, lead: f32) -> bool {
        let Some(a) = &self.act else { return false };
        let Some(s) = a.step() else { return false };
        if self.swing.is_none_or(|w| w.knife) {
            return false;
        }
        match s.kind {
            Kind::WindUp => a.t >= s.len - fd::COUNTER_BEFORE + lead,
            Kind::Strike => a.t <= fd::COUNTER_AFTER,
            _ => false,
        }
    }
}

/// Throwing knives the player carries (ours: AC1's count grows with the memory blocks, not read).
const KNIVES: u32 = 5;
/// A throwing knife's speed (m/s, ours).
const KNIFE_SPEED: f32 = 25.0;

/// A guard's mind (ours: AC1's fight AI, `FightManager` / `NPCFighterType` / the `GoalID_Fight*` goals, is not decoded).
#[derive(Component)]
pub struct Guard {
    /// Fighting the player.
    pub hostile: bool,
    next_attack: f32,
    /// How long since its last attack (s): the attack turn goes to the one that waited longest.
    waited: f32,
    rng: u32,
    block: f32,
    deflect: f32,
    /// Strike back at once (after deflecting a blow).
    riposte: bool,
    passive: bool,
}

impl Guard {
    /// A guard with the defaults (`AC1_GUARD_BLOCK`, `AC1_GUARD_DEFLECT`, `AC1_GUARD_PASSIVE`, `AC1_GUARD_AWARE`), its
    /// dice seeded by `seed`. A guard only fights while it has a `Fighter` too.
    pub fn new(seed: u32) -> Guard {
        let env = |k: &str, d: f32| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
        Guard {
            hostile: std::env::var("AC1_GUARD_AWARE").is_ok(),
            next_attack: 2.0,
            waited: 0.0,
            rng: 0x9e37_79b9 ^ seed.wrapping_mul(7919).wrapping_add(1),
            block: env("AC1_GUARD_BLOCK", 0.5),
            deflect: env("AC1_GUARD_DEFLECT", 0.0),
            riposte: false,
            passive: std::env::var("AC1_GUARD_PASSIVE").is_ok(),
        }
    }

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

/// The first clip of an action's first item.
fn first_clip(lib: &AnimLib, action: u32) -> Option<String> {
    items(lib, action).first().and_then(|i| i.first()).cloned()
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

/// A step and the way back to the stance after it (`<clip>_tr_wait`, or the `_a` clip's), when there is one.
fn with_way_back(lib: &mut AnimLib, rig: usize, name: &str, kind: Kind) -> Vec<Step> {
    let mut out: Vec<Step> = step(lib, rig, name, kind, Root::Clip).into_iter().collect();
    let back = [format!("{name}_tr_wait"), name.strip_suffix("_a").map(|n| format!("{n}_tr_wait")).unwrap_or_default()];
    if let Some(b) = back.iter().find(|b| !b.is_empty() && lib.names.contains(b)) {
        out.extend(step(lib, rig, b, Kind::Recover, Root::Clip));
    }
    out
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

/// How far round (rad, about the vertical, positive left) a clip's root has turned by time `t`, followed key by key from
/// its start: a turn round goes past half a turn (taken the shorter way, the 180 degree turn's root jumped 27 degrees
/// back the other way in a frame as it passed it).
fn clip_yaw(c: &Clip, t: f32) -> f32 {
    let yaw = |f: f32| {
        let q = model_to_bevy() * root_rotation_at(c, f) * model_to_bevy().inverse();
        let d = q * Vec3::NEG_Z;
        (-d.x).atan2(-d.z)
    };
    let end = t * FPS;
    let (mut f, mut last, mut sum) = (0.0, yaw(0.0), 0.0);
    while f < end {
        f = (f + 1.0).min(end);
        let y = yaw(f);
        sum += (y - last + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        last = y;
    }
    sum
}

/// Where a clip puts the body at time `t`, against its root: the Reference bone's place (model space) and its yaw
/// about the vertical.
fn body_at(c: &Clip, t: f32) -> (Vec3, f32) {
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
    (pos, fwd.y.atan2(fwd.x))
}

/// A yaw about Bevy's up as a rotation (0 faces -Z).
fn yaw_rot(yaw: f32) -> Quat {
    Quat::from_rotation_y(yaw)
}

/// The root a lead-in must end at so that its last frame's body is where the paired clip's first frame puts it, with
/// the pair's root at `pair`: (place, facing). `height`: match the body's height too (a jump through the air).
fn meet(pair: &Transform, paired: &Clip, lead: &Clip, height: bool) -> (Vec3, Quat) {
    let m2b = model_to_bevy();
    let (p_body, p_yaw) = body_at(paired, 0.0);
    let (l_body, l_yaw) = body_at(lead, lead.anim.duration);
    // (Model yaw is about +Z, counter-clockwise from +X; turned into Bevy it is a turn about +Y the same way.)
    let rot = pair.rotation * yaw_rot(p_yaw - l_yaw);
    let world = |r: Quat, v: Vec3| r * (m2b * v);
    let flat = |v: Vec3| if height { v } else { v.with_z(0.0) };
    let pos = pair.translation + world(pair.rotation, flat(p_body)) - world(rot, flat(l_body));
    (if height { pos } else { pos.with_y(pair.translation.y) }, rot)
}

/// `a` then `b` (rigid; `a`'s scale kept).
fn iso_mul(a: &Transform, b: &Transform) -> Transform {
    Transform { translation: a.translation + a.rotation * b.translation, rotation: (a.rotation * b.rotation).normalize(), scale: a.scale }
}

fn iso_inv(a: &Transform) -> Transform {
    let r = a.rotation.inverse();
    Transform { translation: r * -a.translation, rotation: r, scale: Vec3::ONE }
}

/// The body offset that shows the body at `o` in the root's frame (`Character::body_offset`, model space).
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
                (n.len() >= 2).then(|| (Vec3::new(n[0], n.get(3).copied().unwrap_or(0.0), n[1]), n.get(2).copied().unwrap_or(0.0).to_radians()))
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
        f.rng ^= k as u32 * 0x9e37;
        commands.entity(e).insert((Transform::from_translation(p).with_rotation(yaw_rot(yaw)).with_scale(Vec3::splat(scale)), f, Guard::new(k as u32)));
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
    /// In an attack (wind-up to landing): holds the guards' attack turn.
    attacking: bool,
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
    /// An air assassination: assassin, victim.
    AirAssassinate(Entity, Entity),
    /// A combo kill: killer, victim, the combo's direction.
    ComboKill(Entity, Entity, u8),
    /// A throwing knife lands: by, on.
    Knife(Entity, Entity),
}

type FighterQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static mut Transform, &'static mut Character, &'static mut Controller, &'static mut Fighter, Option<&'static mut Guard>, Has<Player>),
>;

/// How many guards attack at once (ours, `AC1_FIGHT_ATTACKERS`: AC1's fight manager lets a memory block's share of the
/// fighters take part, `fight_data::FIGHT_MANAGER`; the first blocks' one).
fn attackers_at_once() -> usize {
    std::env::var("AC1_FIGHT_ATTACKERS").ok().and_then(|v| v.parse().ok()).unwrap_or(fd::FIGHT_MANAGER[0].0 as usize).max(1)
}

/// The fight: everyone's choices, moves and what they do to each other.
pub fn fight(time: Res<Time>, level: Res<Level>, input: Res<FightInput>, mut lib: Option<ResMut<AnimLib>>, mut q: FighterQuery, mut started: Local<bool>) {
    let Some(lib) = lib.as_deref_mut() else { return };
    // (`AC1_PLAYER_HEALTH`: the player's health at the start, for the scripts.)
    if !*started {
        *started = true;
        if let Some(h) = std::env::var("AC1_PLAYER_HEALTH").ok().and_then(|v| v.parse::<f32>().ok()) {
            for (_, _, mut ch, _, _, _, _) in q.iter_mut().filter(|x| x.6) {
                ch.health = h.clamp(0.01, 1.0);
            }
        }
    }
    let dt = time.delta_secs().min(0.05);
    let seen: Vec<Seen> = q
        .iter()
        .map(|(e, tf, ch, _, f, g, pl)| Seen {
            e,
            pos: tf.translation,
            rot: tf.rotation,
            // (Dying, its kill or death playing, counts as gone.)
            alive: !f.dead && (f.hp > 0 || pl),
            guard: g.is_some(),
            hostile: g.as_ref().is_some_and(|g| g.hostile),
            player: pl,
            busy: f.act.is_some(),
            speed: ch.velocity.with_y(0.0).length(),
            armed: f.drawn.is_some_and(|w| w != Weapon::HiddenBlade),
            target: f.target,
            attacking: f.swing.is_some() && matches!(f.kind(), Some(Kind::WindUp | Kind::Strike | Kind::Landed)),
        })
        .collect();
    // The guards' attack turn (ours): free while fewer than `attackers_at_once` attack; it goes to the hostile guard
    // that has waited longest among those ready.
    let attacking = seen.iter().filter(|s| s.guard && s.attacking).count();
    let turn: Option<Entity> = if attacking < attackers_at_once() {
        // (Logged by the guard that takes it.)
        q.iter()
            .filter(|(_, _, _, _, f, g, _)| {
                !f.dead && f.fight && f.act.is_none() && g.as_ref().is_some_and(|g| g.hostile && !g.passive && g.next_attack <= 0.0)
            })
            .max_by(|a, b| a.5.as_ref().map_or(0.0, |g| g.waited).total_cmp(&b.5.as_ref().map_or(0.0, |g| g.waited)))
            .map(|x| x.0)
    } else {
        None
    };
    let mut evs: Vec<Ev> = vec![];
    for (me, mut tf, mut ch, mut ctl, mut f, guard, is_player) in &mut q {
        let (tf, ch, ctl, f) = (&mut *tf, &mut *ch, &mut *ctl, &mut *f);
        if f.dead {
            if is_player {
                desync(tf, ch, f, dt);
            }
            continue;
        }
        if is_player {
            player_choices(lib, &input, &seen, me, tf, ch, ctl, f, dt, &mut evs);
        } else if let Some(mut g) = guard {
            guard_choices(lib, &seen, me, tf, ch, f, &mut g, turn == Some(me), dt);
        }
        let aim = f.target.and_then(|t| seen.iter().find(|s| s.e == t && s.alive)).map(|s| s.pos);
        tick(lib, &level, me, tf, ch, ctl, f, aim, dt, &mut evs);
    }
    for ev in evs {
        settle(lib, &mut q, ev);
    }
    // Fighters keep apart (not through a paired move, whose clips put them together).
    let mut pairs = q.iter_combinations_mut();
    while let Some([(_, mut a, _, _, fa, _, _), (_, mut b, _, _, fb, _, _)]) = pairs.fetch_next() {
        let close = |f: &Fighter| f.dead || matches!(f.kind(), Some(Kind::Paired | Kind::Lead | Kind::Death | Kind::Air));
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

/// The player lying dead: desynchronised after a while, back at the spawn point with full health (ours: AC1's
/// desynchronisation restarts at the memory's last checkpoint).
fn desync(tf: &mut Transform, ch: &mut Character, f: &mut Fighter, dt: f32) {
    if f.act.is_some() {
        return;
    }
    f.dead_t += dt;
    if f.dead_t < DESYNC_AFTER {
        return;
    }
    info!("combat: the player starts again");
    let at = ch.spawn_point();
    tf.translation = at;
    ch.limp = false;
    ch.health = 1.0;
    ch.fighting = false;
    ch.body_offset = None;
    ch.velocity = Vec3::ZERO;
    ch.weapon_in_hand = None;
    *f = Fighter { selected: f.selected, ..Fighter::new(f.selected, 10) };
}

/// Seconds the player lies dead before the desynchronisation (ours; the fall death's `RESPAWN_TIME`).
const DESYNC_AFTER: f32 = 3.0;

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
    f.fist_chain += dt;
    // (On the ground: a ground move playing counts, the fight cuts it.)
    let on_ground = ch.wall.as_ref().is_none_or(|w| w.legs_on_ground()) && ch.fall_v == 0.0 && !ch.flying;
    // Weapons: a press selects, a second draws, a third sheathes.
    if let Some(w) = input.weapon_key.and_then(Weapon::from_key)
        && on_ground
        && f.draw.is_none()
        && f.act.is_none()
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
            // (With the weapon in hand; with none, or the hidden blade, the sword.)
            let w = f.drawn.filter(|w| w.fights()).unwrap_or(Weapon::Sword);
            enter_fight(lib, ch, tf, f, t.e, w);
            if f.drawn != Some(w) && f.draw.is_none() {
                f.selected = w;
                start_draw(lib, ch, f, w, true);
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
    // The legs while defending, as a guard's blow comes: the counter dodge to the stick's side (back with none).
    if input.legs_press
        && f.fight
        && ctl.high
        && f.act.is_none()
        && seen.iter().any(|s| s.guard && s.alive && s.hostile && s.attacking && s.pos.distance(tf.translation) < 3.5)
    {
        let stick = ctl.move_dir.with_y(0.0);
        let side = if stick.length() < 0.3 {
            3
        } else {
            match fd::side_of(angle_to(tf.rotation, Vec3::ZERO, stick)) {
                0 | 3 => 3,
                s => s,
            }
        };
        if let Some(n) = fd::counter_dodge(f.weapon(), side, f.foot).and_then(|id| first_clip(lib, id)) {
            info!("combat: counter dodge {n}");
            let steps = with_way_back(lib, ch.rig_id, &n, Kind::Dodge);
            f.act = Some(Act::new(steps, *tf, Then::Stance, None));
            return;
        }
    }
    // The weapon hand.
    if input.hand_press {
        // The hidden blade, out of the fight: an assassination, if AC1's chooser finds one; in the air, the air one.
        if f.selected == Weapon::HiddenBlade && !f.fight && f.act.is_none() {
            // (A guard well below, from a jump, a ledge, a beam or a roof's edge: the air assassination.)
            if let Some(v) = air_victim(seen, tf.translation, ch.velocity) {
                info!("combat: air assassination from {:.1} m up", tf.translation.y - v.pos.y);
                evs.push(Ev::AirAssassinate(me, v.e));
                return;
            }
            if on_ground {
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
        }
        let armed = f.drawn.is_some_and(|w| w.fights());
        if f.fight && armed {
            if ctl.high {
                // Defending: the counter, if a guard's attack on us is in its window.
                if let Some(a) = seen.iter().find(|s| s.guard && s.alive && s.busy && s.hostile && s.pos.distance(tf.translation) < 3.5) {
                    evs.push(Ev::Counter(me, a.e));
                }
            } else {
                match f.kind() {
                    // (AC1's latch: kept from the strike's start through the clip it lands with; a press in the
                    // wind-up is dropped when the strike starts.)
                    Some(Kind::Strike | Kind::Landed | Kind::WindUp) => {
                        if let Some(s) = &mut f.swing {
                            s.latched = true;
                        }
                    }
                    None | Some(Kind::Stance | Kind::Turn) => f.hold = Some(0.0),
                    _ => {}
                }
            }
        } else if on_ground
            && f.act.is_none()
            && f.draw.is_none()
            && matches!(f.selected, Weapon::Sword | Weapon::ShortBlade | Weapon::Fists)
            && let Some(t) = mobile_target(seen, tf, ch)
        {
            // Running at a guard: AC1's mobile attack (the sword drawn as it strikes if it is sheathed).
            mobile_attack(lib, ch, tf, f, t);
            return;
        } else if armed
            && on_ground
            && let Some(t) = nearest_guard(seen, tf.translation, 3.0, false)
        {
            // An attack near a guard out of the fight starts it.
            let w = f.drawn.unwrap_or(Weapon::Sword);
            enter_fight(lib, ch, tf, f, t.e, w);
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
            if matches!(f.kind(), None | Some(Kind::Stance | Kind::Turn)) {
                attack(lib, ch, tf, f, t, max);
            }
        } else if target.is_none() {
            f.hold = None;
        }
    }
}

/// A guard ahead of a running player, in reach of a mobile attack (ours: 2.5 m, within 45 degrees, running over 4 m/s).
fn mobile_target(seen: &[Seen], tf: &Transform, ch: &Character) -> Option<Seen> {
    if ch.velocity.with_y(0.0).length() < 4.0 {
        return None;
    }
    seen.iter()
        .filter(|s| {
            s.guard && s.alive && s.pos.distance(tf.translation) < 2.5 && angle_to(tf.rotation, tf.translation, s.pos).abs() < std::f32::consts::FRAC_PI_4
        })
        .min_by(|a, b| a.pos.distance(tf.translation).total_cmp(&b.pos.distance(tf.translation)))
        .copied()
}

/// A guard under a player in the air, in reach of the air assassination (ours: 1.5..7 m below, within 3.5 m ahead of
/// where the player is going).
fn air_victim(seen: &[Seen], at: Vec3, vel: Vec3) -> Option<Seen> {
    let ahead = at + vel.with_y(0.0) * 0.3;
    seen.iter()
        .filter(|s| s.guard && s.alive && !s.busy && (1.5..7.0).contains(&(at.y - s.pos.y)) && (s.pos - ahead).with_y(0.0).length() < 3.5)
        .min_by(|a, b| (a.pos - ahead).with_y(0.0).length().total_cmp(&(b.pos - ahead).with_y(0.0).length()))
        .copied()
}

/// A guard's choices (ours): turn hostile on what it sees, then fight, attacking when it holds the attack turn.
#[allow(clippy::too_many_arguments)]
fn guard_choices(lib: &mut AnimLib, seen: &[Seen], me: Entity, tf: &mut Transform, ch: &mut Character, f: &mut Fighter, g: &mut Guard, turn: bool, dt: f32) {
    let Some(player) = seen.iter().find(|s| s.player) else { return };
    let d = player.pos.distance(tf.translation);
    if !g.hostile {
        // A weapon drawn within 4 m in front of it, the player fighting it, or a guard fighting within 15 m (the alarm).
        let facing = angle_to(tf.rotation, tf.translation, player.pos).abs() < 1.2;
        let alarm = seen.iter().any(|s| s.guard && s.e != me && s.alive && s.hostile && s.pos.distance(tf.translation) < 15.0);
        if player.target == Some(me) || (d < 4.0 && facing && player.armed) || (alarm && player.alive && d < 20.0) {
            g.hostile = true;
        }
    }
    if !g.hostile || !player.alive || f.act.is_some() && !f.fight {
        if !player.alive && f.fight && f.act.is_none() {
            // (The player down: back out of the fight.)
            leave_fight(lib, ch, tf, f);
            g.hostile = false;
        }
        return;
    }
    if !f.fight {
        info!("combat: guard turns hostile");
        f.drawn = Some(Weapon::Sword);
        ch.weapon_in_hand = Some(P_LEFTWAIST_TAG);
        enter_fight(lib, ch, tf, f, player.e, Weapon::Sword);
    }
    f.target = Some(player.e);
    if f.act.is_none() && !g.passive {
        g.next_attack -= dt;
        g.waited += dt;
        if g.riposte && d < 2.6 {
            g.riposte = false;
            g.next_attack = 2.2 + g.rand() * 1.6;
            g.waited = 0.0;
            info!("combat: the guard strikes back");
            attack(lib, ch, tf, f, *player, true);
        } else if turn && g.next_attack <= 0.0 && d < 2.6 {
            info!("combat: the guard takes the attack turn ({} attacking)", seen.iter().filter(|s| s.guard && s.attacking).count() + 1);
            g.next_attack = 2.2 + g.rand() * 1.6;
            g.waited = 0.0;
            attack(lib, ch, tf, f, *player, true);
        }
    }
}

/// Into the fight stance with `w`, on `target`.
fn enter_fight(lib: &mut AnimLib, ch: &mut Character, tf: &Transform, f: &mut Fighter, target: Entity, w: Weapon) {
    f.fight = true;
    f.target = Some(target);
    f.foot = usize::from(!ch.animator.as_ref().is_none_or(|a| a.lead_left()));
    let fo = FOOT[f.foot];
    let style = fd::STYLE[w.index()];
    let mut steps = vec![];
    if let Some(n) = find_clip(lib, &format!("xx_h_wait_hipm_{fo}_tr_xx_h_{style}_wait"), |_| true) {
        steps.extend(step(lib, ch.rig_id, &n, Kind::Stance, Root::Clip));
    }
    f.act = Some(Act::new(steps, *tf, Then::Stance, None));
    ch.fighting = true;
    info!("combat: fight mode on ({})", w.name());
}

/// Out of the fight stance, into the walk.
fn leave_fight(lib: &mut AnimLib, ch: &mut Character, tf: &Transform, f: &mut Fighter) {
    f.fight = false;
    f.target = None;
    f.swing = None;
    let fo = FOOT[f.foot];
    let style = fd::STYLE[f.weapon()];
    let steps: Vec<Step> = find_clip(lib, &format!("xx_h_{style}_wait_{fo}_tr_xx_h_wait_hipm"), |_| true)
        .and_then(|n| step(lib, ch.rig_id, &n, Kind::Stance, Root::Clip))
        .into_iter()
        .collect();
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

/// An attack on `t` (AC1's attack start, 0xF3E2F0): the short blade past its medium band throws a knife; a target over
/// `HIGH_LOW` higher or lower gets the high or low attack; else AC1's table pick (side, distance, foot; strong or max;
/// the fists' second attack within `FIST_CHAIN` a max one). Then the wind-up, the strike, and the follow-through with
/// its way back to the stance.
fn attack(lib: &mut AnimLib, ch: &mut Character, tf: &mut Transform, f: &mut Fighter, t: Seen, mut max: bool) {
    let w = f.weapon();
    let d = t.pos.distance(tf.translation);
    let side = fd::side_of(angle_to(tf.rotation, tf.translation, t.pos));
    let dist = fd::distance_band(d, w);
    if w == fd::SHORT_BLADE && dist == 2 && f.knives > 0 {
        throw_knife(lib, ch, tf, f, t, side);
        return;
    }
    if w == fd::FISTS && !max && f.fist_chain < fd::FIST_CHAIN {
        max = true;
    }
    let rise = t.pos.y - tf.translation.y;
    let roll = f.roll();
    let (a, no_combo) = if rise.abs() > fd::HIGH_LOW && side == 0 {
        (fd::high_low(w, max, dist, rise > 0.0, f.foot), true)
    } else {
        (fd::attack(fd::AttackAt { weapon: w, max, side, dist, foot: f.foot, last_random: f.last_random }, roll), false)
    };
    let Some(a) = a else { return };
    let steps = swing_steps(lib, ch.rig_id, a.start, a.strike);
    if steps.is_empty() {
        return;
    }
    if a.random.is_some() {
        f.last_random = a.random;
    }
    if w == fd::FISTS && !max {
        f.fist_chain = 0.0;
    }
    info!("combat: attack {} ({}, band {dist}, {})", steps[0].clip.name, fd::SIDES[side], if max { "max" } else { "strong" });
    f.swing = Some(Swing { target: t.e, weapon: w, dir: a.dir, max, combos: 0, outcome: None, latched: false, no_combo, knife: false });
    f.act = Some(Act::new(steps, *tf, Then::Stance, None));
}

/// AC1's mobile attack (0xF49DB0): running at `t`, with the weapon drawn (kind 0) or drawing it as it strikes (kind 3,
/// the sword only).
fn mobile_attack(lib: &mut AnimLib, ch: &mut Character, tf: &mut Transform, f: &mut Fighter, t: Seen) {
    let drawing = f.drawn != Some(f.selected);
    let w = f.selected.index();
    let Some(a) = fd::mobile(w, drawing, f.foot) else { return };
    let steps = swing_steps(lib, ch.rig_id, a.start, a.strike);
    if steps.is_empty() {
        return;
    }
    if drawing {
        f.drawn = Some(f.selected);
        ch.weapon_in_hand = f.selected.carry_bone();
    }
    info!("combat: mobile attack {}", steps[0].clip.name);
    f.target = Some(t.e);
    ch.fighting = true;
    f.swing = Some(Swing { target: t.e, weapon: w, dir: a.dir, max: true, combos: 0, outcome: None, latched: false, no_combo: true, knife: false });
    f.act = Some(Act::new(steps, *tf, Then::Stance, None));
    f.fight = true;
}

/// A throwing knife at `t` (AC1's throw tables 0x1A34EF0 / 0x1A34F58 by the knife's place and the target's side: the
/// belt's): the throw, its release, and the knife flying at `KNIFE_SPEED`.
fn throw_knife(lib: &mut AnimLib, ch: &mut Character, tf: &mut Transform, f: &mut Fighter, t: Seen, side: usize) {
    let fo = FOOT[f.foot];
    let sd = fd::SIDES[side];
    let throw = find_clip(lib, &format!("xx_h_knife_belt_{sd}_long_strong_throw_{fo}"), |r| r.contains("start"));
    let release = find_clip(lib, &format!("xx_h_knife_belt_{sd}_long_strong_release_{fo}"), |_| true);
    let (Some(throw), Some(release)) = (throw, release) else {
        warn!("combat: no throw clips for {sd} {fo}");
        return;
    };
    let mut steps: Vec<Step> = step(lib, ch.rig_id, &throw, Kind::WindUp, Root::Clip).into_iter().collect();
    steps.extend(step(lib, ch.rig_id, &release, Kind::Throw, Root::Clip));
    if steps.len() < 2 {
        return;
    }
    f.knives -= 1;
    info!("combat: throwing knife {throw} ({} left)", f.knives);
    f.swing = Some(Swing { target: t.e, weapon: fd::SHORT_BLADE, dir: 3, max: true, combos: 0, outcome: None, latched: false, no_combo: true, knife: true });
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

/// What follows a strike that has played: the clip it lands with and the combo after it (AC1's 0xF4A0A0 / 0xF3B820
/// and 0xF506C0 / 0xF50710). Returns the steps to put after the current one, if anything changes.
fn after_strike(lib: &mut AnimLib, me: Entity, ch: &Character, f: &mut Fighter, ended: Kind, evs: &mut Vec<Ev>) -> Option<Vec<Step>> {
    let s = f.swing?;
    let landed = matches!(s.outcome, Some(Outcome::Hit | Outcome::Blocked));
    if ended == Kind::Strike && matches!(s.outcome, Some(Outcome::Hit | Outcome::Blocked | Outcome::Killed)) {
        // The clip it lands with (on a killed target the hit's), then the way back.
        let blocked = s.outcome == Some(Outcome::Blocked);
        if let Some(n) = fd::landed(s.weapon, blocked, s.dir, s.max || s.combos > 0, f.foot).and_then(|id| first_clip(lib, id)) {
            debug!("combat: landed {n}");
            return Some(with_way_back(lib, ch.rig_id, &n, Kind::Landed));
        }
    }
    if s.outcome == Some(Outcome::Deflected) && ended == Kind::Strike {
        // Deflected: AC1's recoil (0xF46090), no combo.
        f.swing = Some(Swing { latched: false, ..s });
        return Some(
            fd::deflected(s.weapon, s.dir, f.foot)
                .and_then(|id| first_clip(lib, id))
                .map(|n| with_way_back(lib, ch.rig_id, &n, Kind::Recover))
                .unwrap_or_default(),
        );
    }
    // The combo: the landed clip over (or a strike that landed with none), a press latched since the strike started.
    let window_end = ended == Kind::Landed || (ended == Kind::Strike && landed);
    if window_end && landed && s.latched && !s.no_combo && fd::combo_opens(s.weapon, s.max, s.combos) {
        let next = fd::next_combo_dir(s.weapon, s.dir, true);
        let (start, strike, end) = fd::combo(s.weapon, next, false, f.foot)?;
        // The one that would kill is the paired combo kill (settled with the victim).
        evs.push(Ev::ComboKill(me, s.target, next));
        let more = swing_steps(lib, ch.rig_id, start, strike);
        info!("combat: combo hit {} {}", s.combos + 1, more.first().map_or("?", |m| m.clip.name.as_str()));
        f.swing = Some(Swing { dir: end, combos: s.combos + 1, outcome: None, latched: false, ..s });
        return Some(more);
    }
    // (A strike that met its target with no clip to land with: back to the stance by the strike action's own connector,
    // `<strike>_tr_wait`, the follow-through is a miss's.)
    if ended == Kind::Strike && matches!(s.outcome, Some(Outcome::Hit | Outcome::Blocked | Outcome::Killed)) {
        let back = ch.animator.as_ref().and_then(|a| a.forced.as_ref()).map(|(c, _)| format!("{}_tr_wait", c.name));
        return Some(back.filter(|b| lib.names.contains(b)).and_then(|b| step(lib, ch.rig_id, &b, Kind::Recover, Root::Clip)).into_iter().collect());
    }
    None
}

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
    // A knife in flight.
    if let Some((t, left)) = &mut f.knife {
        *left -= dt;
        if *left <= 0.0 {
            evs.push(Ev::Knife(me, *t));
            f.knife = None;
        }
    }
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
        ch.fall_v = 0.0;
    }
    // (Nothing the ground takes from the keys while fighting.)
    ctl.legs_press = false;
    ctl.legs_release = false;
    ctl.toggle_climb = false;
    ctl.hand_climb = false;
    ctl.push = false;
    ctl.pickpocket = false;
    let from = tf.translation;
    ch.body_offset = None;
    if let Some(mut act) = f.act.take() {
        act.t += dt;
        // Into the next step at the end of this one.
        while act.step().is_some_and(|s| act.t >= s.len) {
            let len = act.step().map_or(0.0, |s| s.len);
            let ended = act.step().map(|s| s.kind);
            // A knife's release over: it flies.
            if ended == Some(Kind::Throw)
                && let Some(s) = f.swing
                && let Some(to) = aim
            {
                f.knife = Some((s.target, to.distance(tf.translation) / KNIFE_SPEED));
            }
            if let Some(k) = ended
                && matches!(k, Kind::Strike | Kind::Landed)
                && let Some(next) = after_strike(lib, me, ch, f, k, evs)
            {
                act.steps.truncate(act.i + 1);
                act.steps.extend(next);
            }
            // (The step's root motion to its end, not to the frame before: the next starts where this one ends.)
            if let Some(st) = act.step()
                && matches!(st.root, Root::Clip)
                && st.frozen.is_none()
                && act.slide.is_none()
            {
                let r = clip_root(&act.anchor, &st.clip, st.clip.anim.duration);
                tf.translation = r.translation.with_y(tf.translation.y);
                tf.rotation = r.rotation;
            }
            act.t -= len;
            act.i += 1;
            act.anchor = *tf;
            act.fresh = true;
        }
        let Some(s) = act.step().cloned() else {
            // The move is over.
            let then = act.then;
            if let Some(w) = &f.swing
                && w.outcome != Some(Outcome::Killed)
                && !w.knife
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
            if !f.fight {
                // (A mobile attack or a kill out of the stance: back to the walk.)
                ch.fighting = false;
                ch.body_offset = None;
                if let Some(a) = ch.animator.as_mut() {
                    a.forced = None;
                }
                ch.exit_fade = Some((ch.anim_pose.clone(), 0.2, 0.2));
                return;
            }
            // (Back into the stance, faded from the move's last pose.)
            f.stance = None;
            stance(lib, tf, ch, ctl, f, aim, dt);
            return;
        };
        if act.fresh {
            act.fresh = false;
            // (The blend into the step, from the pose shown.)
            if act.i > 0 || s.blend > 0.0 {
                ch.exit_fade = Some((ch.anim_pose.clone(), s.blend, s.blend));
            }
            if s.kind == Kind::Paired
                && let Some(r) = act.pair_root.take()
            {
                // The pair's root: the paired clip plays from it; this fighter's root slides onto it.
                let pair = Transform { scale: tf.scale, ..r };
                let was = iso_mul(&iso_inv(&pair), tf);
                debug!("combat: root onto the pair's, {:.2} m", was.translation.length());
                act.slide = Some((was, 0.0));
                act.anchor = pair;
            }
            if s.kind == Kind::Strike
                && let Some(w) = &mut f.swing
            {
                // (AC1's strike state clears the press latch as it starts, 0xF40460.)
                w.latched = false;
                if !w.knife {
                    evs.push(Ev::Strike(me, w.target, w.dir, w.max, w.combos > 0));
                }
            }
            // A wind-up closes or opens the distance so that its strike ends `CONTACT` from the target (ours: AC1 steers
            // its discrete moves to a computed place, sub_711130; the place for an attack is not found).
            if s.kind == Kind::WindUp
                && f.swing.is_some_and(|w| !w.knife)
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
            Root::Turn(k) => {
                let c = clip_root(&act.anchor, &s.clip, t);
                Transform { rotation: (act.anchor.rotation * Quat::from_rotation_y(clip_yaw(&s.clip, t) * k)).normalize(), ..c }
            }
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
        let r = match &mut act.slide {
            Some((was, t)) => {
                *t += dt;
                let k = (*t / SLIDE).clamp(0.0, 1.0);
                let k = k * k * (3.0 - 2.0 * k);
                let o = Transform { translation: was.translation.lerp(Vec3::ZERO, k), rotation: was.rotation.slerp(Quat::IDENTITY, k), scale: Vec3::ONE };
                ch.body_offset = (k < 1.0).then(|| body_offset(&iso_inv(&o), tf.scale.x));
                if k >= 1.0 {
                    act.slide = None;
                }
                iso_mul(&r, &o)
            }
            None => r,
        };
        if std::env::var("AC1_FIGHT_TRACE").is_ok() {
            debug!("combat: root {:?} {} t {:.3} at {:.2}", s.kind, s.clip.name, act.t, r.translation);
        }
        tf.translation = r.translation;
        tf.rotation = r.rotation;
        // Facing the target through swings' wind-ups (they aim at it): the step's anchor turns with it. (Into the stance,
        // AC1's turns face it after.)
        if s.kind == Kind::WindUp
            && f.fight
            && let Some(to) = aim
        {
            let before = tf.rotation;
            face(tf, to, TURN_RATE, dt);
            let turn = tf.rotation * before.inverse();
            act.anchor.rotation = (turn * act.anchor.rotation).normalize();
            act.anchor.translation = tf.translation + turn * (act.anchor.translation - tf.translation);
        }
        f.act = Some(act);
    } else {
        stance(lib, tf, ch, ctl, f, aim, dt);
    }
    // On the ground, out of walls (not through a paired clip: the pair would come apart; nor through the air).
    let kind = f.kind();
    let paired = matches!(kind, Some(Kind::Paired | Kind::Lead | Kind::Death | Kind::Air));
    if kind != Some(Kind::Air)
        && let Some(g) = level.ground(tf.translation, 1.0, 2.0)
    {
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

/// The stance: the wait, or the walk in it the stick asks for, facing the target (standing, with AC1's turns); the
/// defence's in high profile. The weapon's own (AC1's stance tables by weapon).
fn stance(lib: &mut AnimLib, tf: &mut Transform, ch: &mut Character, ctl: &Controller, f: &mut Fighter, aim: Option<Vec3>, dt: f32) {
    let defend = ctl.high;
    let stick = ctl.move_dir.with_y(0.0);
    let moving = stick.length() > 0.3;
    let w = f.weapon();
    let fo = FOOT[f.foot];
    // Standing with the target over 30 degrees off: AC1's turn on the spot (0xF4BB60), its clip's turn scaled to the
    // angle (0xF38D10). Walking, or within 30 degrees, the facing follows the target (ours).
    if let Some(to) = aim {
        let a = angle_to(tf.rotation, tf.translation, to);
        if !moving
            && let Some((k, nominal)) = fd::turn_for(a)
            && let Some(n) = fd::turn_action(w, k, defend, f.foot).and_then(|id| first_clip(lib, id))
            && let Some(mut s) = step(lib, ch.rig_id, &n, Kind::Turn, Root::Turn(a.abs() / nominal))
        {
            debug!("combat: turn {n} for {:.0} degrees", a.to_degrees());
            s.blend = s.blend.min(0.15);
            f.act = Some(Act::new(vec![s], *tf, Then::Stance, None));
            return;
        }
        face(tf, to, TURN_RATE, dt);
    }
    // AC1's stance actions by weapon: the walk's blend space by the stick's way round the facing, the defence's
    // wait; the sword's light wait lists no items, only its idle variants: its clip by name.
    let from_table = |lib: &AnimLib, id: u32, k: usize| items(lib, id).first().and_then(|i| i.get(k).or(i.first()).cloned());
    let name = if moving {
        let k = fd::strafe_dir(angle_to(tf.rotation, Vec3::ZERO, stick));
        from_table(lib, if defend { fd_t::DEFENCE_STRAFE[w][f.foot] } else { fd_t::STRAFE[w][f.foot] }, k)
    } else if defend {
        from_table(lib, fd_t::DEFENCE_WAIT[w][f.foot], 0)
    } else {
        from_table(lib, fd_t::WAIT[w][f.foot], 0)
    };
    // (Waits that list only their idle variants, the light and unarmed ones: the plain wait by name.)
    let name = name.or_else(|| {
        let plain = format!("xx_h_{}_wait_{fo}", fd::STYLE[w]);
        if lib.names.contains(&plain) { Some(plain) } else { find_clip(lib, &plain, |r| !r.contains("_tr_") && !r.contains("mobile")) }
    });
    let Some(name) = name else { return };
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

use crate::fight_tables as fd_t;

/// How fast a fighter's facing follows its target while walking in the stance or within 30 degrees (rad/s; ours).
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
        Ev::AirAssassinate(by, on) => air_assassinate(lib, q, by, on),
        Ev::ComboKill(by, on, dir) => combo_kill(lib, q, by, on, dir),
        Ev::Knife(by, on) => knife_lands(lib, q, by, on),
    }
}

/// The player's health a guard's blow takes (of 1; ours: AC1's `DamageData` table, DataPC `Game Bootstrap Settings`,
/// holds per-attack damages of 10, 20 and 30, by rows not decoded).
const DAMAGE_STRONG: f32 = 0.1;
const DAMAGE_MAX: f32 = 0.15;

/// The victim's hurt reaction to a blow from `dir` (AC1's `HumanGround_Fight_Hurt`, the choice ours): from behind the
/// back one; a strong attack the quick vertical one; a max or combo hit the strong one from the blade's side.
fn hurt_clip(lib: &AnimLib, w: usize, dir: u8, strong: bool, behind: bool, foot: usize) -> Option<String> {
    let fo = FOOT[foot];
    let side = if dir == 2 || dir == 3 { "sl" } else { "sr" };
    let prefix = match (w == fd::FISTS, behind, strong) {
        (true, true, _) => "xx_h_unarmed_hurt_quick_back_punch".to_string(),
        (true, false, false) => format!("xx_h_unarmed_hurt_quick_front_punch_{}", if dir == 2 || dir == 3 { "ul" } else { "ur" }),
        (true, false, true) => "xx_h_unarmed_hurt_strong_front_kick_dm".to_string(),
        (false, true, _) => "xx_h_light_hurt_strong_back_common".to_string(),
        (false, false, false) => "xx_h_light_hurt_quick_front_light_vertical".to_string(),
        (false, false, true) => format!("xx_h_light_hurt_strong_front_light_{side}"),
    };
    find_clip(lib, &prefix, |r| r.ends_with(fo) && !r.contains("tr_wait")).or_else(|| find_clip(lib, &prefix, |r| r.contains("foot") && !r.contains("tr_wait")))
}

/// A strike lands on `on`: blocked, deflected, a hit, or its death.
#[allow(clippy::too_many_arguments)]
fn strike(lib: &mut AnimLib, q: &mut FighterQuery, by: Entity, on: Entity, dir: u8, max: bool, combo: bool) {
    let Ok([(_, atf, _, _, mut af, mut ag, _), (_, mut vtf, mut vch, vctl, mut vf, vg, v_player)]) = q.get_many_mut([by, on]) else { return };
    let reach = vtf.translation.distance(atf.translation);
    let angle = angle_to(atf.rotation, atf.translation, vtf.translation);
    let ahead = angle.abs() < 1.0;
    let behind = angle_to(vtf.rotation, vtf.translation, atf.translation).abs() > 2.0;
    let weapon = af.weapon();
    debug!("combat: strike at {reach:.2} m, {:.0} degrees off", angle.to_degrees());
    let outcome = if vf.dead || reach > 3.2 || !ahead || vf.kind() == Some(Kind::Dodge) {
        Outcome::Missed
    } else {
        // Blocking: the player defending in the stance, a guard by chance (never a timed combo hit, ours); a guard may
        // deflect instead, and strike back at once.
        let free = matches!(vf.kind(), None | Some(Kind::Stance | Kind::Block | Kind::Turn)) && !behind;
        let (blocks, deflects) = if v_player {
            (vctl.high && vf.fight && free, false)
        } else if let Some(mut g) = vg {
            g.hostile = true;
            let roll = g.rand();
            let ok = free && !combo && vf.fight;
            let deflects = ok && roll < g.deflect;
            if deflects {
                g.riposte = true;
                g.next_attack = 0.0;
            }
            (ok && !deflects && roll < g.deflect + g.block, deflects)
        } else {
            (false, false)
        };
        if deflects {
            Outcome::Deflected
        } else if blocks {
            Outcome::Blocked
        } else {
            if !v_player {
                vf.hp -= 1;
            }
            if v_player {
                vch.health = (vch.health - if max || combo { DAMAGE_MAX } else { DAMAGE_STRONG }).max(0.0);
                if vch.health <= 0.0 { Outcome::Killed } else { Outcome::Hit }
            } else if vf.hp <= 0 {
                Outcome::Killed
            } else {
                Outcome::Hit
            }
        }
    };
    if let Some(s) = af.swing.as_mut() {
        s.outcome = Some(outcome);
    }
    if let Some(g) = ag.as_mut() {
        g.waited = 0.0;
    }
    let (vtf, vch, vf) = (&mut *vtf, &mut *vch, &mut *vf);
    let rig = vch.rig_id;
    let fo = FOOT[vf.foot];
    let d = fd::DIRS[dir as usize];
    match outcome {
        Outcome::Missed => info!("combat: the strike misses"),
        Outcome::Blocked | Outcome::Deflected => {
            let name = if outcome == Outcome::Deflected {
                info!("combat: the strike is deflected");
                format!("xx_h_light_deflect_{d}_{fo}")
            } else {
                info!("combat: the strike is blocked");
                let set = if max { "max" } else { "strong" };
                let name = format!("xx_h_light_block_{set}_rec_{d}_{fo}");
                if lib.names.contains(&name) { name } else { format!("xx_h_light_block_strong_rec_{d}_{fo}") }
            };
            let steps = with_way_back(lib, rig, &name, Kind::Block);
            vf.act = Some(Act::new(steps, *vtf, Then::Stance, None));
        }
        Outcome::Hit => {
            let first = hurt_clip(lib, weapon, dir, max || combo, behind, vf.foot);
            info!(
                "combat: the strike hits ({} left) {}",
                if v_player { format!("{:.2}", vch.health) } else { vf.hp.to_string() },
                first.as_deref().unwrap_or("?")
            );
            let steps: Vec<Step> = first.map(|n| with_way_back(lib, rig, &n, Kind::Hit)).unwrap_or_default();
            vf.swing = None;
            vf.hold = None;
            vf.act = Some(Act::new(steps, *vtf, Then::Stance, None));
        }
        Outcome::Killed => {
            // AC1's wound deaths (`HumanGround_Fight_Hurt`): the blow from the attacker's right lands on the left (`sl`).
            let side = if dir == 2 || dir == 3 { "sl" } else { "sr" };
            let name = format!("xx_h_light_death_wound_front_{side}_{fo}");
            info!("combat: the strike kills: {name}");
            if v_player {
                info!("combat: the player dies: desynchronised, starting again in {DESYNC_AFTER} s");
            }
            let steps: Vec<Step> = step(lib, rig, &name, Kind::Death, Root::Clip).into_iter().collect();
            vf.swing = None;
            vf.fight = false;
            vf.act = Some(Act::new(steps, *vtf, Then::Limp, None));
        }
    }
}

/// A throwing knife lands on `on`: a guard dies of it (ours: AC1's knives kill most guards in one).
fn knife_lands(lib: &mut AnimLib, q: &mut FighterQuery, by: Entity, on: Entity) {
    let Ok([(_, _, _, _, mut af, _, _), (_, vtf, vch, _, mut vf, mut vg, v_player)]) = q.get_many_mut([by, on]) else { return };
    if let Some(s) = af.swing.as_mut().filter(|s| s.knife) {
        s.outcome = Some(Outcome::Killed);
    }
    af.swing = None;
    if vf.dead || v_player {
        return;
    }
    if let Some(g) = vg.as_mut() {
        g.hostile = true;
    }
    let name = format!("xx_h_light_death_wound_front_vertical_{}", FOOT[vf.foot]);
    let name = if lib.names.contains(&name) { name } else { format!("xx_h_light_death_wound_front_sl_{}", FOOT[vf.foot]) };
    info!("combat: the throwing knife kills: {name}");
    let steps: Vec<Step> = step(lib, vch.rig_id, &name, Kind::Death, Root::Clip).into_iter().collect();
    vf.hp = 0;
    vf.swing = None;
    vf.fight = false;
    vf.act = Some(Act::new(steps, *vtf, Then::Limp, None));
}

/// A timed combo hit on a guard that it would kill: AC1's paired combo kill for the hit's direction (the killer's
/// tables by the direction, the victim's by 3 - the direction) in place of the hit; else nothing (the combo hit plays).
fn combo_kill(lib: &mut AnimLib, q: &mut FighterQuery, by: Entity, on: Entity, dir: u8) {
    let Ok([(_, atf, ach, _, mut af, _, _), (_, vtf, vch, _, mut vf, _, v_player)]) = q.get_many_mut([by, on]) else { return };
    if v_player || vf.dead || vf.hp > 1 || vtf.translation.distance(atf.translation) > 3.0 {
        return;
    }
    let Some(ids) = fd::combo_kill(af.weapon(), dir) else { return };
    let [start, pos, ka, pose, da] = ids.map(|id| first_clip(lib, id));
    let (Some(start), Some(pos), Some(ka), Some(pose), Some(da)) = (start, pos, ka, pose, da) else {
        warn!("combat: no combo kill clips for direction {dir}");
        return;
    };
    // The `_b` halves follow the `_a` ones (the kill's next item, the victim's death's).
    let kb = items(lib, fd::combo_kill(af.weapon(), dir).map_or(0, |i| i[2])).get(1).and_then(|i| i.first()).cloned().or_else(|| {
        let n = ka.strip_suffix("_a").map(|n| format!("{n}_b"))?;
        lib.names.contains(&n).then_some(n)
    });
    let db = da.strip_suffix("_a").map(|n| format!("{n}_b")).filter(|n| lib.names.contains(n));
    let tr = find_clip(lib, ka.trim_end_matches("_a"), |r| r.contains("tr_wait"));
    let (ar, vr) = (ach.rig_id, vch.rig_id);
    let (Some(ka_c), Some(pos_c)) = (clip(lib, ar, &ka), clip(lib, ar, &pos)) else { return };
    // The victim waits in its death pose while the killer winds up and steps in; the pair's root is the victim's.
    let start_s = step(lib, ar, &start, Kind::Lead, Root::Clip);
    let lead_len = start_s.as_ref().map_or(0.0, |s| s.len) + pos_c.anim.duration;
    let pair = *vtf;
    // (The killer's lead-in: its wind-up as it is, then the step in steered so the body meets the kill.)
    let (to, rot) = meet(&pair, &ka_c, &pos_c, false);
    let mut ksteps: Vec<Step> = start_s.into_iter().collect();
    ksteps.push(Step { clip: pos_c.clone(), kind: Kind::Lead, root: Root::Steer(to, rot), len: pos_c.anim.duration, frozen: None, blend: 0.1 });
    ksteps.extend(step(lib, ar, &ka, Kind::Paired, Root::Clip));
    if let Some(kb) = kb {
        ksteps.extend(step(lib, ar, &kb, Kind::Paired, Root::Clip));
    }
    if let Some(tr) = tr {
        ksteps.extend(step(lib, ar, &tr, Kind::Recover, Root::Clip));
    }
    let mut vsteps: Vec<Step> = vec![];
    if let Some(mut s) = step(lib, vr, &pose, Kind::Lead, Root::Hold) {
        s.len = lead_len;
        vsteps.push(s);
    }
    vsteps.extend(step(lib, vr, &da, Kind::Paired, Root::Clip));
    if let Some(db) = db {
        vsteps.extend(step(lib, vr, &db, Kind::Death, Root::Clip));
    }
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

/// A counter kill (`HumanGround_CounterKills`, AC1's tables by the defender's foot: `_01` off the left, `_02` off the
/// right): the defender steps in on the attacker's swing; the pair plays from the attacker's root.
fn counter(lib: &mut AnimLib, q: &mut FighterQuery, by: Entity, on: Entity) {
    let Ok([(_, atf, ach, _, mut af, _, _), (_, vtf, vch, _, mut vf, _, _)]) = q.get_many_mut([by, on]) else { return };
    if !vf.counter_open_for(0.0) || af.act.as_ref().is_some_and(|a| a.step().is_some_and(|s| !matches!(s.kind, Kind::Stance | Kind::Turn))) {
        info!("combat: the counter is mistimed");
        return;
    }
    let dir = vf.swing.map_or(3, |s| s.dir);
    let (w, foot) = (af.weapon(), af.foot);
    let Some((step_id, kill_id)) = fd::counter_kill(w, vf.weapon(), dir, foot) else { return };
    let (Some(st), Some(ka)) = (first_clip(lib, step_id), first_clip(lib, kill_id)) else {
        warn!("combat: no counter clips for direction {dir}");
        return;
    };
    let set = if foot == 0 { "01" } else { "02" };
    let d = fd::DIRS[dir as usize];
    let kb = ka.strip_suffix("_a").map(|n| format!("{n}_b")).filter(|n| lib.names.contains(n));
    let vp = format!("xx_h_light_countered_light_{set}_death_{d}_");
    let pick = |pre: &str, f: &dyn Fn(&str) -> bool| find_clip(lib, pre, f);
    let (Some(da), Some(db)) = (pick(&vp, &|r: &str| r.ends_with("_a")), pick(&vp, &|r: &str| r.ends_with("_b"))) else {
        warn!("combat: no countered clips for {vp}");
        return;
    };
    let tr = find_clip(lib, ka.trim_end_matches("_a"), |r| r.contains("tr_wait") || r.contains("to_wait"));
    let (ar, vr) = (ach.rig_id, vch.rig_id);
    let (Some(st_c), Some(ka_c)) = (clip(lib, ar, &st), clip(lib, ar, &ka)) else { return };
    let pair = *vtf;
    let (to, rot) = meet(&pair, &ka_c, &st_c, false);
    let mut ksteps = vec![Step { len: st_c.anim.duration, clip: st_c.clone(), kind: Kind::Lead, root: Root::Steer(to, rot), frozen: None, blend: 0.1 }];
    ksteps.extend(step(lib, ar, &ka, Kind::Paired, Root::Clip));
    if let Some(kb) = kb {
        ksteps.extend(step(lib, ar, &kb, Kind::Paired, Root::Clip));
    }
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
    let (to, rot) = meet(&pair, &kill_c, &lead_c, false);
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

/// The air assassination: AC1's jump onto an assassination target (`HumanInAir` 0x21B4DC3D / 0x21B4DC3E, the 9-way
/// blend of `xx_h_air_<front|down|up>_<cm>_<foot>_to_assassinate`, picked by the jump code 0xB1EC40 for a target of
/// kind 0x8000) steered through the air so its body lands where the kill starts; the kill is ours: AC1's finish on the
/// ground (types 8 and 9), the victim knocked down under it.
fn air_assassinate(lib: &mut AnimLib, q: &mut FighterQuery, by: Entity, on: Entity) {
    let Ok([(_, atf, mut ach, _, mut af, _, _), (_, vtf, vch, _, mut vf, _, _)]) = q.get_many_mut([by, on]) else { return };
    let (ar, vr) = (ach.rig_id, vch.rig_id);
    let drop = atf.translation.y - vtf.translation.y;
    let fo = FOOT[af.foot];
    // The jump by the drop: the nearest of AC1's heights (0.5, 3, 5.5, 6.5 m down).
    let cm = [(0.5, "050cm"), (3.0, "300cm"), (5.5, "550cm"), (6.5, "650cm")]
        .into_iter()
        .min_by(|a, b| (a.0 - drop).abs().total_cmp(&(b.0 - drop).abs()))
        .map_or("300cm", |c| c.1);
    let jump = format!("xx_h_air_down_{cm}_{fo}_to_assassinate");
    // From behind the victim the back finish, else the front one.
    let back = angle_to(vtf.rotation, vtf.translation, atf.translation).abs() > std::f32::consts::FRAC_PI_2;
    let [a_lead, a_kill, v_lead, v_kill] = fd::ASSASSINATION[if back { 9 } else { 8 }];
    let flat = |id: u32| -> Vec<String> { items(lib, id).into_iter().filter_map(|i| i.first().cloned()).collect() };
    let (al, ak, vl, vk) = (flat(a_lead), flat(a_kill), flat(v_lead), flat(v_kill));
    let (Some(jump_c), Some(kill_a)) = (clip(lib, ar, &jump), al.first().cloned()) else {
        warn!("combat: no air assassination clips");
        return;
    };
    let Some(kill_c) = clip(lib, ar, &kill_a) else { return };
    let pair = *vtf;
    let (to, rot) = meet(&pair, &kill_c, &jump_c, true);
    // (The jump stretched to the fall's time from this height, at least its own.)
    let len = jump_c.anim.duration.max((2.0 * drop.max(0.0) / 9.81).sqrt());
    let mut asteps = vec![Step { len, clip: jump_c.clone(), kind: Kind::Air, root: Root::Steer(to, rot), frozen: None, blend: 0.15 }];
    for (k, n) in al.iter().chain(ak.iter()).enumerate() {
        let last = k + 1 == al.len() + ak.len();
        asteps.extend(step(lib, ar, n, if last { Kind::Recover } else { Kind::Paired }, Root::Clip));
    }
    // The victim stands as it is until the landing (its pose held), then is knocked down under it.
    let mut vsteps: Vec<Step> = vch
        .playing
        .first()
        .map(|p| Step { len, clip: p.clip.clone(), kind: Kind::Lead, root: Root::Hold, frozen: Some(p.t.min(p.clip.anim.duration)), blend: 0.0 })
        .into_iter()
        .collect();
    for (k, n) in vl.iter().chain(vk.iter()).enumerate() {
        vsteps.extend(step(lib, vr, n, if k == 0 { Kind::Paired } else { Kind::Death }, Root::Clip));
    }
    info!("combat: air assassination {jump} then {kill_a}");
    vf.hp = 0;
    vf.fight = false;
    vf.act = Some(Act::new(vsteps, *vtf, Then::Limp, Some(pair)));
    ach.fighting = true;
    ach.fall_v = 0.0;
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
    if !ch.fighting || f.dead && f.act.is_none() {
        return;
    }
    if f.act.as_ref().is_some_and(|a| a.fresh) {
        // (Held on the frame shown meanwhile, the move's fade starting from it: run on, the stance's transition into the
        // light wait went past its end and wrapped to its first frame, the sword hand 0.34 m off as a turn began.)
        if let Some((_, t)) = ch.animator.as_mut().and_then(|a| a.forced.as_mut()) {
            *t = (*t - back).max(0.0);
        }
        return;
    }
    let shown = match &f.act {
        Some(a) => a.step().map(|s| {
            // (The air jump is stretched over its flight; the rest hold their last frame when longer than their clip.)
            let t = if s.kind == Kind::Air { a.t * s.clip.anim.duration / s.len.max(1e-3) } else { a.t };
            // (Not the very last key: some strikes' last one is already the next item's pose, the knife's lunges jump
            // their hand half a metre into it.)
            (s.clip.clone(), s.frozen.unwrap_or(t.min((s.clip.anim.duration - 1.0 / FPS).max(0.0))))
        }),
        None => f.stance.as_ref().map(|(c, t)| (c.clone(), *t)),
    };
    if let (Some((c, t)), Some(a)) = (shown, ch.animator.as_mut()) {
        // (Its foot, for the next pick.)
        f.foot = end_foot(&c.name, f.foot);
        if std::env::var("AC1_FIGHT_TRACE").is_ok() {
            debug!("combat: shows {} at {:.3} (fade {:?})", c.name, t, ch.exit_fade.as_ref().map(|e| e.1));
        }
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

    #[test]
    fn weapons_index_the_tables() {
        assert_eq!(Weapon::Fists.index(), fd::FISTS);
        assert_eq!(Weapon::Sword.index(), fd::SWORD);
        assert_eq!(Weapon::ShortBlade.index(), fd::SHORT_BLADE);
        assert!(!Weapon::HiddenBlade.fights());
    }
}
