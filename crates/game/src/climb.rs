//! Wall climbing, and the jumps and falls around it, driven by AC1's clips.
//!
//! AC1 models a climber as a hand configuration ("1m", "1lu", "1ru", "2m", "2lu", "2ru": hands level,
//! left up, right up; 1 = narrow, 2 = wide) with a looping hang clip `xx_climb_wait_<state>` and moves
//! `xx_l_climb_<from>_<dir>_<to>` that carry the body with root motion (0.6 m up per hand, 0.38 m
//! sideways). From "1m" there are also corner moves (`xx_l_climb_1m_corner_<side>_090_<in|out>`, a
//! 90 degree turn in place or around the outside) and the top-out chain
//! `xx_l_climb_1m_tr_hangknee_footl_a` (pull up, +1.2 m) -> `_b` (knee onto the top, +0.6 m forward)
//! -> `xx_h_hangknee_footl_tr_h_wait_footr_a` (stand up).
//!
//! Free hang (no wall under the feet): the body hangs straight below the hands, its root 1.2 m lower
//! and 0.5 m nearer the wall than on the wall. `xx_l_climb_1m_<dir>_hangfree` swings off the wall
//! into it and `xx_h_hangfree_<dir>_climb_1m` brings the feet back. In free hang the hands are
//! together ("free_1m", `xx_h_hangfree_waitclose`) or apart ("free_open", `_waitopen`); the shimmy
//! alternates `xx_h_hangfree_strafe_<side>_050cm_open` (close -> open) and `_close` (open -> close),
//! 0.5 m each, and `xx_h_hangfree_climb_1m_<u|d>_1<l|r>u` then `_1<l|r>u_<u|d>_1m` move one hand at a
//! time (0.7 m). Every move is checked against the feet: a wall state needs a foot on the wall at the
//! end of the move, a free-hang state needs both feet clear of it. The free-hang top-out is
//! `xx_h_hangfree_tr_hangwaist_a/b` (pull up to the waist, +1.4 m) -> `xx_h_hangwaist_tr_hangknee_footl`
//! (knee onto the top, +1 m up, +0.6 m forward) -> stand up.
//!
//! Leaps (a held modifier while climbing): `xx_h_climbing_<from>_tr_<to>_<dir>_<2|3>_<a..e>` chains,
//! from/to `climb1m` or `hangfree`, dir `left`/`right` (1.5 m or 2.25 m) or, wall to wall,
//! `up_<l|r>_hand`/`down_<l|r>_hand` (1.2 m or 1.8 m). `_a` winds up, `_b` carries the flight and the
//! rest catch the holds.
//!
//! Letting go: from "1m" with the ground within reach, `xx_l_climb_1m_tr_h_wait_hipm_footl_a/b` steps
//! down onto it. Otherwise `xx_h_hangwall_tr_fall_a` (push off 0.3 m) or `xx_h_hangfree_tr_fall_a`
//! (release) plays, `_b` gives the falling pose while gravity moves the root, and
//! `xx_h_landing_straight_<soft|hard>_footr_tr_h_wait_footr_a/b` plays on touchdown.
//!
//! Running jump (from the ground): AC1's takeoff and flight blends (`crate::jump`, weighted by how far and how high
//! the target is: `xx_h_run_<front|down|up>_<dist>_footl_to_air`, `xx_h_air_<...>_footl_to_freestep*`). Onto a top, a
//! post or a beam the root follows the clips' own motion onto it, then the reception `_tr_freestep_entry_footr[_fast]`
//! (`jump_ac1`); at a hold, or with no target (weighted for a level one 2.5 m ahead), the takeoff plays, then the
//! flight over the air while gravity moves the root (walls stop the horizontal speed), and `xx_h_landing_forward_<soft|hard>_footr_tr_h_jog_footl_a/b` lands
//! into a jog. A running jump at a wall catches a hold on the way down, when the catch clip's end pose
//! puts both hands within reach of holds: `xx_fall_tr_climb_<min|max>_a/b` (hands up, drop 0.5 m into
//! the wide wall hang "2m") or, with no wall under the feet, `xx_fall_tr_hangfree_min_a/b` (into a free
//! hang); `max` is the faster fall.
//!
//! Standing jump: `xx_h_wait_hipm_footl_tr_impultionstraight_footl` (crouch) ->
//! `xx_h_impultionstraight_footl_to_jumpstraight_clear_footl` (push off) -> `xx_h_jumpstraight_clear_footl`
//! (rise, 1 m in 0.4 s) while gravity moves the root, then `xx_h_jumpstraight_clear_footall_tr_fall`
//! from the top of the jump down. It can catch holds like the running jump.
//!
//! Standing jump to a ledge: the same crouch, then `xx_h_jumpstraight_footl_to_<kind>_<h>cm` (the
//! flight, root up) and its `_tr_<kind>_a/b`. By what is in front: a walkable top at 1.5 or 2 m
//! (`hangknee_footl`, kneel on it, then stand up) or 2.5 m (`hangwaist`, then
//! `xx_h_hangwaist_tr_hangknee_footl` and stand up); a hold at about 2 or 2.5 m with wall under the
//! feet (`hangwall`, ending in the wide wall hang "2m"); a hold at about 2.5 or 3 m with nothing under
//! the feet (`hangfree`, ending in a free hang). The root is moved over the flight so the hands land
//! exactly.
//!
//! Wall run (sprinting at a wall): `xx_h_wallingfront_entry_footl_a/b` (first step up the wall, +1 m)
//! and `xx_h_wallingfront_step1_footr` (+0.5 m), then the highest thing in reach:
//! `step1_footr_tr_hangwall_430cm_000cm_a/b` (a hold at 4.3 m), `step1_footr_tr_hangknee_<250|201>cm_footr`
//! (onto a top), `step1_footr_tr_hangfree_<400|350>cm_swingback_min` (free hang),
//! `step1_footr_tr_hangwall_251cm_000cm_a/b`, or straight off the first step
//! `entry_footl_tr_hangknee_<200|131>cm_footl`. With nothing in reach, `step1_footr_tr_fall` pushes off and
//! the climber falls (and can still catch a hold).
//!
//! Rebound (Space again during a wall run, A/D for the side): `xx_h_wallingfront_<entry|step1>rebound_
//! <back|left|right>_<footr|footl>` kicks off the wall (about 2.5 m back, or sideways turning 90 degrees)
//! and the climber flies under gravity, turning with the clip, and can catch a hold on a facing wall.
//! The game's `HumanWallingData` has "rebound distances" and a `ReboundTransition` sub-state.
//!
//! Leap of faith (Space at the edge of a high roof with a haystack below and ahead):
//! `xx_h_freestep_footr_to_faith_jump_100cm_long_<300cm_down|3000cm_down_footr>` takes off, the matching
//! `xx_h_faith_jump_100cm_long_<..>_down` dive is stretched over the actual fall while gravity carries the
//! root (aimed at the haystack), and `xx_h_faith_jump_landing` (1.6 m down) ends in the hay.
//! Any other fall that comes down into a haystack lands with `xx_h_air_to_haystack` (1.2 m down). In the
//! hay `xx_h_haystack_wait` loops until any input, then `xx_l_haystack_hop_out` (+2 m) and
//! `_tr_l_wait`. (The game has `ActorStateID_LeapOfFaith`, `ActorStateID_InHayStack` and haystack entry
//! types Top, Ground, FreeStep and SideJump; only Top is recreated.)
//!
//! Side wall run (sprinting along a wall at a shallow angle, Space): AC1 has the code for it
//! (`WallingType_Horizontal`) but ships no clips, so this one is built from the sprint cycle
//! (`xx_h_run_hipm_footl/r`): the root runs along the wall in a 1.2 m arc, leaning about 23 degrees off
//! it with the hips 0.5 m out; IK plants the stance foot on the wall and puts the wall-side hand on the wall (see
//! `character::animate`); then it jumps off (Space jumps off early, away from the wall).
//!
//! Landing damage, by the height fallen (thresholds are guesses; the game's `LandingType` has Safe,
//! SmallDamage, HeavyDamage and Fatal): under 4.5 m safe; to 8 m `xx_h_landing_damage_footl` (20%);
//! to 13 m `xx_h_hurt_fall_balanced_front_short_500cm_landing` (50%); beyond, or when the damage would
//! empty the health, `xx_h_landing_death_back`. Haystacks are always safe.
//!
//! Posts and beams (AC1's "pilotis" and "beam" sets): a fall that lands on a narrow top lands with
//! `xx_h_beam_landing_soft_tr_pilotis_wait_a/b` and balances in `xx_h_beam_pilotis_wait`. Running jumps
//! are steered onto a post or beam whose top they would come down within 1.2 m of. From a post, Space
//! (or sprint + a direction) jumps to the next post or walkable top in that direction:
//! `xx_h_beam_pilotis_tr_impultionstraight_a` crouches, then the root flies a ballistic arc that lands on
//! it while the air clip is stretched over the flight. On a beam, a direction along it walks
//! (`xx_l_beam_crouchwalk_footl/r`, sprinting `xx_h_beam_crouchjog_footl/r`); at its end the climber
//! steps onto ground that carries on, or stops.
//!
//! Swing bars: a fall that brings the hands (about 2.3 m above the root) to a bar catches it and swings
//! through `xx_h_swing_cycle_front_up` (feet forward) -> `_front_down` -> `_back_up` -> `_back_down`.
//! Space lets go when the feet next swing forward: `xx_h_swing_cycle_front_300cm_to_air` flings the
//! climber forward (S held: `_down_050cm_to_air` drops), and the flight can catch the next bar.
//!
//! Low obstacles (see `vault`): running at one (top 0.35-1.3 m up) steps up onto it (to 0.85 m, AC1's
//! `xx_h_collide_full_footl_<050|070>cm_*` chain mixed by height) or jumps onto it (higher), and runs on along
//! its top. AC1 has no vault: its `passover` clips (`HumanLedge`) go over a ledge's edge downward.
//!
//! Ladders: Space at the foot of one plays `xx_l_wait_hipm_footl_tr_l_ladder_climb_up_r` onto it, with
//! the rungs 0.44 m in front of the root; the hands are in one of two configurations, `xx_l_ladder_wait_l`
//! / `_r`, and `xx_l_ladder_climb_<up|down>_<l|r>` move 0.5 m from one to the other; in high profile the
//! `xx_h_` set climbs 1 m a step. Near the top `xx_l_ladder_climb_up_<l|r>_tr_l_wait_hipm_foot<l|r>_a/b` climbs off
//! onto it (+1 m up and forward; high: `xx_h_ladder_climb_up_<l|r>_tr_freestep_foot<l|r>_a/b` runs off); at the
//! bottom `xx_<l|h>_ladder_climb_down_<l|r>_tr_<l|h>_wait_hipm_foot<r|l>` steps off. The legs with the stick pulled
//! back rebound off it backwards (`xx_h_ladder_wait_<l|r>_tr_rebound_foot<l|r>`). This is `HumanLadderData`'s table
//! of the actions AC1's code asks for (docs/NOTES.md "What the executable's code asks for").
//! From the top, Space at the ladder's edge turns and climbs down onto it
//! (`xx_h_wait_hipm_footl_tr_ladder_pulldown`, `xx_ladder_pulldown_tr_l_ladder_wait_r_a/b`). Space on the
//! ladder lets go (`xx_l_ladder_wait_<l|r>_tr_falling`).
//!
//! Kiosk frames (monkey bars): Space under the end of one jumps up to it
//! (`xx_h_kiosk_freestep_footr_tr_monkeybar_footr`, +1.9 m forward), `xx_h_kiosk_monkeybar_footr` swings
//! along it hand over hand (1.1 m a step, the hands 1.9 m above the root) to its far end, and
//! `xx_h_kiosk_monkeybar_footr_tr_fall` drops off (also on Space). The step clip only swings from the
//! right hand to the left (AC1 has no `_footl` one), so every other step plays it mirrored (see
//! `ik::mirror`), and the drop off plays mirrored after a mirrored step.
//!
//! Clip seams inside a move crossfade over 0.05-0.25 s by how far the hands and feet jump: some AC1
//! chains do not line up (the standing jump-up starts in the air, its crouch ends on the ground).
//!
//! Every move started from the ground eases in: the pose crossfades from what was shown, and the root
//! slides and turns from where it stood into the move's planned start over the first clip.
//!
//! Root motion that slides along the ground (takeoff, landings) stops at walls.
//!
//! A move is only taken if its end pose (root motion and rotation included) puts both hands on real
//! holds; then it plays with its root motion applied to the character, and IK snaps the hands onto
//! the holds and the feet onto the wall (see `character::animate`).

use crate::animation::{AnimLib, Clip, mix_name, root_motion_at, root_rotation_at, sample};
use crate::character::model_to_bevy;
use crate::level::{Ledge, Level, Line};
use bevy::prelude::*;
use forge::anim::FPS;
use ik::{Pose, Rig};
use std::sync::Arc;

pub const ENTRY: &str = "xx_h_wait_hipm_footl_tr_climbing_1m";
const ENTRY_STATE: &str = "1m";
const TOP_OUT: [&str; 3] = ["xx_l_climb_1m_tr_hangknee_footl_a", "xx_l_climb_1m_tr_hangknee_footl_b", "xx_h_hangknee_footl_tr_h_wait_footr_a"];
/// The same from the wide hang ("2m": mid-air catches, wall-run and jump-up grabs end there).
const TOP_OUT_2M: [&str; 3] = ["xx_l_climb_2m_tr_hangknee_footl_a", "xx_l_climb_2m_tr_hangknee_footl_b", "xx_h_hangknee_footl_tr_h_wait_footr_a"];
/// One-hand pull-ups, where the top drops away beside the hands (a post, the end of a wall): from the wall,
/// and from a free hang (to the waist, then the knee); both end on one knee stepping up (`freestep`).
const TOP_OUT_ONEHAND: [&str; 3] = [
    "xx_h_hangwall_onehand_to_hangknee_onehand_footl_a",
    "xx_h_hangwall_onehand_to_hangknee_onehand_footl_b",
    "xx_h_hangknee_onehand_footl_to_freestep_entry_footl",
];
const TOP_OUT_FREE_ONEHAND: [&str; 4] = [
    "xx_h_hangfree_onehand_to_hangwaist_onehand_a",
    "xx_h_hangfree_onehand_to_hangwaist_onehand_b",
    "xx_h_hangwaist_onehand_to_hangknee_onehand_footl",
    "xx_h_hangknee_onehand_footl_to_freestep_entry_footl",
];
/// A post's top line is this far in from the edge pulled up over (m; posts are about 0.6 m across).
const ONEHAND_POST_IN: f32 = 0.3;
/// The top drops away this far to a side of the hands (m, measured 0.15 m in from the edge): one hand.
const ONEHAND_SIDE: f32 = 0.35;
const TOP_OUT_FREE: [&str; 4] =
    ["xx_h_hangfree_tr_hangwaist_a", "xx_h_hangfree_tr_hangwaist_b", "xx_h_hangwaist_tr_hangknee_footl", "xx_h_hangknee_footl_tr_h_wait_footr_a"];
/// Pseudo-states: kneeling on the top edge, and standing on top (the climb is over).
const KNEEL: &str = "hangknee";
const ON_TOP: &str = "top";
/// Free-hang states with the hands together and apart.
const FREE: &str = "free_1m";
/// Hanging from a ledge with the feet on the wall (AC1's `hangwall`, not the climbing grid): hands together
/// (`xx_h_hangwall_waitclose`) or apart (`xx_h_hangwall_wait`, where catches end); it shimmies along the ledge
/// alternating the two, climbs onto wall holds (`xx_h_hangwall_<dir>_climb_1m`) and tops out.
const HANGWALL: &str = "hangwall";
const HANGWALL_OPEN: &str = "hangwall_open";
const HANGWALL_REST: &str = "xx_h_hangwall_wait";
/// The ledge hang sits this much further from the wall and lower than its clips put it (m); the hands stay on
/// the ledge.
const HANG_OUT: f32 = 0.08;
const HANG_DOWN: f32 = 0.05;

/// Where a move ending in `to` puts the root, on top of lining the hands up: the ledge hang's offset.
fn hang_offset(to: &str, normal: Vec3) -> Vec3 {
    if to == HANGWALL || to == HANGWALL_OPEN { normal * HANG_OUT - Vec3::Y * HANG_DOWN } else { Vec3::ZERO }
}
const TOP_OUT_HANGWALL: [&str; 3] = ["xx_h_hangwall_tr_hangknee_footl_a", "xx_h_hangwall_tr_hangknee_footl_b", "xx_h_hangknee_footl_tr_h_wait_footr_a"];
const FREE_OPEN: &str = "free_open";
/// Pseudo-states for leaving the wall: stepping down, pushing off, falling, landing, done.
const STEP_DOWN: &str = "stepdown";
const DROP: &str = "drop";
const FALL: &str = "fall";
const LAND: &str = "land";
const GROUND: &str = "ground";
/// Pseudo-states: in flight during a leap between holds, taking off for a running jump.
const LEAP: &str = "leap";
const JUMP: &str = "jump";
const STEP_OFF: [&str; 2] = ["xx_l_climb_1m_tr_h_wait_hipm_footl_a", "xx_l_climb_1m_tr_h_wait_hipm_footl_b"];
/// How far the step-off clip lowers the root, and the furthest ground it is used for.
const STEP_OFF_DROP: f32 = 0.6;
const STEP_OFF_MAX: f32 = 0.95;
/// A landing is a forward one when the stick is within this of the motion (cos 75 degrees, AC1's 0xE05940).
const LAND_FORWARD_COS: f32 = 0.258_819;
/// The speed a landing's speed bucket is measured against (m/s, the sprint's).
const LAND_SPRINT_SPEED: f32 = 6.2;

/// What a landing goes on into, by the stick and the profile (AC1's `xx_h_landing_..._tr_<wait|walk|jog|sprint>`).
#[derive(Clone, Copy, Debug, PartialEq)]
enum LandInto {
    Wait,
    Walk,
    Jog,
    Sprint,
}

impl LandInto {
    /// The stick let go: standing; held: walking in low profile, jogging in high, sprinting free running.
    fn from(stick: bool, high: bool, sprint: bool) -> Self {
        match (stick, sprint, high) {
            (false, _, _) => Self::Wait,
            (true, true, _) => Self::Sprint,
            (true, false, true) => Self::Jog,
            (true, false, false) => Self::Walk,
        }
    }

    /// The speed it goes on at (m/s).
    fn speed(self) -> f32 {
        match self {
            Self::Wait => 0.0,
            Self::Walk => TOP_OUT_WALK,
            Self::Jog => TOP_OUT_JOG,
            Self::Sprint => PERCH_OFF_RUN,
        }
    }
}

/// AC1's ground landing (`HumanInAir`): `xx_h_landing_<forward|straight>_<soft|hard>_footr_tr_<into>`, `_a` the impact and
/// `_b` going on: forward when coming down moving, straight when dropping; into the wait on the same foot, or the walk,
/// jog or sprint's takeoff on the other.
fn landing_names(forward: bool, hard: bool, into: LandInto) -> [String; 2] {
    let way = if forward { "forward" } else { "straight" };
    let force = if hard { "hard" } else { "soft" };
    let after = match into {
        LandInto::Wait => "h_wait_footr",
        LandInto::Walk => "l_walk_footl",
        LandInto::Jog => "h_jog_footl",
        LandInto::Sprint => "h_sprint_impultion_footl",
    };
    let stem = format!("xx_h_landing_{way}_{force}_footr_tr_{after}");
    [format!("{stem}_a"), format!("{stem}_b")]
}
/// Falling faster than this (m/s, about a 2.5 m drop) lands hard.
const HARD_LANDING_SPEED: f32 = 7.0;
/// Landing with more horizontal speed than this (m/s) rolls on into a jog.
const RUN_LANDING_SPEED: f32 = 2.0;
const MAX_EXIT_SPEED: f32 = 6.5;
/// Running landings from less than this (m) carry straight on at speed, without the landing clip (free running
/// keeps its momentum; the crossfade from the air pose covers it).
const FLOW_LANDING_DROP: f32 = 2.0;
const JUMP_TAKEOFF: &str = "xx_h_sprint_impultion_footl";
/// In-air pose: the first frame of the air-to-landing clip, held.
const JUMP_AIR: &str = "xx_h_air_front_050cm_footl_to_freestep";
/// Falling pose, looped when a fall outlasts the clip it started with.
const FALL_LOOP: &str = "xx_h_jumpfalling01";
/// Upward speed at takeoff (m/s): about 0.45 m of rise and 0.6 s back down to the same level.
const JUMP_UP_SPEED: f32 = 3.0;
/// Horizontal speed a jump leaves the ground with, at most (m/s).
const MAX_JUMP_SPEED: f32 = 6.5;
const STRAIGHT_JUMP: [&str; 3] =
    ["xx_h_wait_hipm_footl_tr_impultionstraight_footl", "xx_h_impultionstraight_footl_to_jumpstraight_clear_footl", "xx_h_jumpstraight_clear_footl"];
const STRAIGHT_JUMP_FALL: &str = "xx_h_jumpstraight_clear_footall_tr_fall";
/// Takeoff speed of the standing jump (m/s): the 1 m rise of `xx_h_jumpstraight_clear_footl`.
const STRAIGHT_JUMP_UP_SPEED: f32 = 4.43;
const STAND_UP: &str = "xx_h_hangknee_footl_tr_h_wait_footr_a";
/// Jump-up onto a ledge top: (clip kind, height in cm, clips after the jump's own).
const JUMP_ONTO: [(&str, u32, &[&str]); 3] =
    [("hangknee_footl", 150, &[STAND_UP]), ("hangknee_footl", 200, &[STAND_UP]), ("hangwaist", 250, &["xx_h_hangwaist_tr_hangknee_footl", STAND_UP])];
/// Closest the wall face may sit in front of the root when a move onto a ledge starts; otherwise the
/// root ends just past the edge.
const ONTO_MIN_WALL_DIST: f32 = 0.45;
const ONTO_PAST_EDGE: f32 = 0.15;
/// Jumping onto a top: the hang part is held this much further out than the clips put it (m).
const ONTO_BACK: f32 = 0.05;
/// Jump up to hang from a hold: (clip kind, height in cm, end state, hang loop whose hands must be on
/// the holds).
const JUMP_HANG: [(&str, u32, &str, &str); 4] = [
    ("hangwall", 200, HANGWALL_OPEN, HANGWALL_REST),
    ("hangwall", 250, HANGWALL_OPEN, HANGWALL_REST),
    ("hangfree", 250, FREE, "xx_h_hangfree_waitclose"),
    ("hangfree", 300, FREE, "xx_h_hangfree_waitclose"),
];
/// How far the hands of a jump up to a hold may miss it (the root is moved over the jump).
const JUMP_HANG_REACH: f32 = 0.45;
/// A wall run up with nothing to grab needs this much clear space above the root plus 1 m (m), this
/// far out from the wall (where the head goes).
const WALL_RUN_HEADROOM: f32 = 2.6;
const WALL_RUN_HEADROOM_OUT: f32 = 0.35;
/// How far a wall run may start from where its first step should be (the approach is pulled in).
const WALL_RUN_SLACK: f32 = 0.9;
/// How far the hands may be from holds for a mid-air catch (m; the body is pulled in over the catch).
const CATCH_REACH: f32 = 0.3;
/// Catching a wall's holds, the hands end this far out from its face (m).
const CATCH_HAND_OUT: f32 = 0.08;
/// Where a wall hang's hands hold, from its root: up, and in toward the wall (m).
const HANGWALL_HOLD_UP: f32 = 1.13;
const HANGWALL_HOLD_IN: f32 = 0.62;
/// A ledge shorter than this (m) is hung from with the hands together.
const NARROW_LEDGE: f32 = 1.0;
/// A ground move stops rather than step off a drop deeper than this (m).
const GROUND_MOVE_DROP: f32 = 0.6;
/// Running at a low obstacle (see `collide`): spotted this far ahead, met head on this squarely (cosine) or else
/// glanced off; stopped against it, the planted foot this far in from the top's edge.
const COLLIDE_REACH: f32 = 0.9;
const COLLIDE_HEAD_ON: f32 = 0.85;
const COLLIDE_FOOT_IN: f32 = 0.12;
/// A hang's hold is this far above its root (m, about); a catch after letting go takes holds at least this much
/// below the one let go of.
const HANG_HOLD_UP: f32 = 1.13;
const CATCH_BELOW_LET_GO: f32 = 0.45;
/// Diving into hay from a top beside it: its edge within this far (m); the dives taken in turn.
const HAY_DIVE_REACH: f32 = 2.2;
static HAY_DIVES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// Standing within this of a viewpoint (m), the leap of faith goes to its hay.
const VIEWPOINT_REACH: f32 = 2.0;
/// A beam this far to the side of the way ahead (m) is taken rather than leaping past it.
const FAITH_BEAM_SIDE: f32 = 0.45;
/// The hands may land this much further apart or closer together than the move puts them (m).
const HANDS_APART_SLACK: f32 = 0.35;
/// Falling faster than this (m/s) uses the hard catch clips.
const CATCH_FAST: f32 = 5.0;
const GRAVITY: f32 = 9.81;
/// Wrist relative to a ledge's grab line: in front of the edge and below the top, so the fingers hook over it.
const GRIP_OUT: f32 = 0.09;
const GRIP_DOWN: f32 = 0.08;
/// How far a hand may end from a hold and still count as on it.
const HOLD_TOLERANCE: f32 = 0.2;
/// Climbing up or down, how far (m) a hand may end from a hold: a city's holds are not on the clips' 0.6 m steps (AC1
/// bends its moves to the next hold), and the move's correction carries the hands onto it.
const HOLD_TOLERANCE_UP: f32 = 0.35;
/// AC1's climbing probe box round a grid cell (`BuildHoldGrid` 0xDF6A40, Banned445): 0.375 m along the wall (half a
/// 0.75 m column), 0.3 m up or down (ours 0.32: the clips' hands are not exactly on the 0.6 m rows), 1.0 m in or out
/// of the wall's plane (a storey set back, a sill sticking out).
const GRID_ALONG: f32 = 0.375;
const GRID_UP: f32 = 0.32;
const GRID_DEPTH: f32 = 1.0;
/// Room the body needs beside it climbing sideways (m), to its side's wall.
const BODY_SIDE: f32 = 0.3;
/// Corner moves steer onto the next face's holds this far from where their clips put the hands (m): a sidestep moves the
/// hands 0.375 m, so they stop up to that far short of the corner. (Corners are tried only once no sidestep fits.)
const CORNER_TOLERANCE: f32 = 0.6;
/// Leaps steer onto a hold this far from where their clips land (m).
const LEAP_TOLERANCE: f32 = 0.75;
/// A foot this close in front of a wall is braced on it.
/// (Hanging on a flush wall the ankles are 0.15-0.23 m from it; a Damascus wall set 0.24 m back under its top hold
/// leaves them 0.41 m off it, still braced in AC1.)
const FOOT_REACH: f32 = 0.45;
/// A wall gives footing under a hold only if it is at most this far behind the hold line (m; a flush wall's
/// holds stick out 0.06 m, a city wall leaning in under its cornice 0.18 m, a Damascus storey's top hold 0.29 m; a
/// pole under a cap is 0.28 m back, and hangs from the wall by the footholds rule, there being no holds under it).
const RECESS_MAX: f32 = 0.32;
const FADE: f32 = 0.12;
/// Crossfade from ground locomotion into a climb or jump, and the time the root takes to turn.
const ENTER_FADE: f32 = 0.25;
const EASE_TURN: f32 = 0.25;
/// Wall run: the steps up the wall, and its ending when nothing is in reach.
const WALL_RUN: [&str; 3] = ["xx_h_wallingfront_entry_footl_a", "xx_h_wallingfront_entry_footl_b", "xx_h_wallingfront_step1_footr"];
const WALL_RUN_FALL: &str = "xx_h_wallingfront_step1_footr_tr_fall";
/// A wall run rebounds until this share of its fall back off the wall has played.
const REBOUND_LATE: f32 = 0.6;
/// Sprinting at a wall closer than this (m) runs up it.
pub const WALL_RUN_REACH: f32 = 2.0;
/// A wall met up to this far off square (rad, 60°) is run up, turning to face it.
const WALL_RUN_ANGLE: f32 = 1.05;
/// Where the wall face sits in front of the root for the first step of a wall run.
const WALL_RUN_FOOT: f32 = 0.75;
/// Pseudo-state: running up a wall.
const WALL_RUN_STATE: &str = "wallrun";
/// Pseudo-states: taking off for a leap of faith, hidden in a haystack, hopping out of it.
const FAITH: &str = "faith";
const HAY: &str = "hay";
const HAY_OUT: &str = "hayout";
/// Sitting on a bench (hidden), and sitting down onto it.
const BENCH: &str = "bench";
const BENCH_SIT: &str = "bench_sit";
const BENCH_WAIT: &str = "xx_rest_sit_idle_040cm_01";
/// A bench within this (m) can be sat on; the sitter's root ends this far out from its seat line (m).
const BENCH_REACH: f32 = 1.3;
const BENCH_SEAT_OUT: f32 = 0.5;
/// Leap of faith (takeoff, dive) for drops under and over `FAITH_HIGH_DROP`.
const FAITH_LOW: [&str; 2] = ["xx_h_freestep_footr_to_faith_jump_100cm_long_300cm_down", "xx_h_faith_jump_100cm_long_300cm_down"];
const FAITH_HIGH: [&str; 2] = ["xx_h_freestep_footr_to_faith_jump_100cm_long_3000cm_down_footr", "xx_h_faith_jump_100cm_long_3000cm_down"];
const FAITH_HIGH_DROP: f32 = 10.0;
const FAITH_LAND: &str = "xx_h_faith_jump_landing";
const AIR_TO_HAY: &str = "xx_h_air_to_haystack";
const HAY_WAIT: &str = "xx_h_haystack_wait";
/// The legs jump into a haystack from this far outside its edge (m).
const HAY_REACH: f32 = 1.3;
const HAY_HOP_OUT: [&str; 2] = ["xx_l_haystack_hop_out", "xx_l_haystack_hop_out_tr_l_wait"];
/// Falling pose for walking off a ledge.
const FALL_POSE: &str = "xx_h_jumpstraight_clear_footall_tr_fall";
/// A leap of faith needs this much drop to the haystack (m) and the haystack within this distance.
const FAITH_MIN_DROP: f32 = 3.0;
const FAITH_MAX_DIST: f32 = 8.0;
/// Landing damage by the drop from the top of the fall (m), as AC1 types landings (`LandingType` Safe / SmallDamage /
/// HeavyDamage / Fatal): damaging from 3 m, heavy over 6.3 m, fatal over 7 m (the game's landing function, via
/// Banned445's AC1-Movement-Rewritten); and the share of health lost.
const SAFE_DROP: f32 = 3.0;
const HEAVY_DROP: f32 = 6.3;
const FATAL_DROP: f32 = 7.0;
const SMALL_DAMAGE: f32 = 0.2;
const HEAVY_DAMAGE: f32 = 0.5;
const LAND_DAMAGE: &str = "xx_h_landing_damage_footl";
/// Running into a damaging landing, or from a drop over `ROLL_LANDING_DROP`, rolls on (`xx_roll_hipm` recovery).
const LAND_DAMAGE_RUN: [&str; 2] = ["xx_h_landing_damage_footl_roll", "xx_roll_hipm_tr_h_jog_hipm_footr"];
const ROLL_LANDING_DROP: f32 = 3.0;
const LAND_HEAVY: [&str; 2] = ["xx_h_hurt_fall_balanced_front_short_500cm_landing", "xx_h_hurt_fall_balanced_front_short_500cm_landing_tr_h_wait_footr"];
const LAND_DEATH: &str = "xx_h_landing_death_back";
/// Pseudo-states: dead after a fall; running along a wall.
const DEAD: &str = "dead";
const SIDE_RUN: &str = "siderun";
/// Side wall run: the sprint half-cycles, length (s), arc height (m), most roll (rad), the speed of the
/// sprint clip's root motion (m/s), and the furthest the wall may be to the side.
const RUN_CYCLE: [&str; 2] = ["xx_h_run_hipm_footl", "xx_h_run_hipm_footr"];
const SIDE_RUN_TIME: f32 = 0.9;
const SIDE_RUN_ARC: f32 = 1.2;
const SIDE_RUN_ROLL: f32 = 0.4;
/// The root's distance from the wall at full lean (m); the stance foot reaches the rest of the way.
const SIDE_RUN_OFF: f32 = 0.5;
const SIDE_RUN_CLIP_SPEED: f32 = 5.2;
const SIDE_RUN_REACH: f32 = 1.2;

/// Rig bones the climber needs.
const PERCH: &str = "perch";
const SWING: &str = "swing";
const VAULT: &str = "vault";
/// Crossfade from the end of a move into a balancing, leaning or hiding loop (s).
const WAIT_FADE: f32 = 0.35;
/// Fade into and out of looking round on a wall (s): AC1's actions blend over 0.2-0.3 s.
const LOOK_FADE: f32 = 0.3;
/// The root's path onto a low obstacle: steady along the ground from the start to just in from its edge,
/// easing up to the top by the time the clips are up on it. The clips still play for the body.
#[derive(Clone, Copy, Debug)]
struct VaultPath {
    from: Vec3,
    to: Vec3,
    /// Seconds to being up on the top, and in all.
    plant: f32,
    dur: f32,
    t: f32,
}

impl VaultPath {
    fn at(&self, t: f32) -> Vec3 {
        let k = (t / self.dur).clamp(0.0, 1.0);
        let up = (t / self.plant).clamp(0.0, 1.0);
        let flat = self.from.lerp(self.to, k);
        flat.with_y(self.from.y + (self.to.y - self.from.y) * (up * std::f32::consts::FRAC_PI_2).sin())
    }
}

/// Pseudo-states: a clip chain on the ground (stops, turns, ledge stops, rolls), and leaning on a wall.
const GROUND_ACT: &str = "ground_act";
const LEAN: &str = "lean";
/// Stopped against a low obstacle with a foot on it (`xx_h_collide_full_footl_<050|070>cm_wait`, see `collide`).
const COLLIDE: &str = "collide";
/// The lean clips come for walls with the hands at 70 cm and at 150 cm (`{h}` is `070` or `150`); AC1 mixes
/// the two by the wall's height.
const LEAN_WAIT: &str = "xx_h_lean_040cm_twohand_{h}cm_wait";
/// Running into a wall this tall stops on the hands and leans on it (m above the feet).
const LEAN_MIN_HEIGHT: f32 = 0.7;
/// Walls this tall or taller lean with the 150 cm clips; lower ones mix in the 70 cm ones.
const LEAN_TALL: f32 = 1.5;
/// The root stands this far out of the wall while leaning on it (m).
const LEAN_DIST: f32 = 0.5;
/// Pulling down onto a ledge: the edge within this far ahead of the feet (m), dropping at least this much.
const PULL_DOWN_REACH: f32 = 0.9;
const PULL_DOWN_DROP: f32 = 1.8;
const PERCH_WAIT: &str = "xx_h_beam_pilotis_wait";
const PERCH_LAND: [&str; 2] = ["xx_h_beam_landing_soft_tr_pilotis_wait_a", "xx_h_beam_landing_soft_tr_pilotis_wait_b"];
const PERCH_TAKEOFF: &str = "xx_h_beam_pilotis_tr_impultionstraight_a";
const PERCH_PUSH: &str = "xx_h_beam_impultionstraight_to_jumpstraight";
const BEAM_WALK: [&str; 2] = ["xx_l_beam_crouchwalk_footl", "xx_l_beam_crouchwalk_footr"];
const BEAM_JOG: [&str; 2] = ["xx_h_beam_crouchjog_footl", "xx_h_beam_crouchjog_footr"];
/// Crouched on a beam (`HumanNarrowObject`): facing along it on the left or right foot ahead, or across it.
const BEAM_WAIT: [&str; 2] = ["xx_l_beam_crouchwait_footl", "xx_l_beam_crouchwait_footr"];
const BEAM_WAIT_ACROSS: &str = "xx_l_beam_crouchwait_90";
/// Walking to a beam's end with nothing past it, the walk stops this far short of it (m; AC1's
/// `ConstrainRootMotionToBeam`; its `xx_l_beam_edge_stop` clips are not used by the game).
const BEAM_END_STOP: f32 = 0.3;

/// A wall to go over with a hand on it (AC1's passover): its top this high over the feet (m; lower ones are jumped
/// onto, higher ones caught), at most this deep, under `PASSOVER_THIN` the 30 cm clips (else the 1 m ones); the root
/// as the hand touches the top is this far under it (the hang's own passover, `xx_h_hangwall_tr_passover_handr`, rises
/// 1 m from the hang to it: the root about level with the top).
const PASSOVER_RISE: std::ops::RangeInclusive<f32> = 1.3..=1.9;
const PASSOVER_DEPTH: f32 = 1.2;
const PASSOVER_THIN: f32 = 0.65;
const PASSOVER_ROOT_DOWN: f32 = 0.4;
/// Over the wall, the root this far past its far edge (m) before falling down the other side.
const PASSOVER_CLEAR: f32 = 0.3;
/// Going over it no faster than this (m/s).
const PASSOVER_SPEED: f32 = 4.0;

/// A jump onto a roof lands this far in from its edge (m; Banned445's port of the game's target, which uses the
/// guidance contact).
const ROOF_EDGE_INSET: f32 = 0.45;

/// A swing bar this far ahead (m) and this high over the feet is what a jump flies at.
const BAR_JUMP_REACH: std::ops::RangeInclusive<f32> = 1.0..=6.5;
const BAR_JUMP_RISE: std::ops::RangeInclusive<f32> = 0.5..=4.0;

/// A running jump at a wall's ledge this fast or faster (m/s) is received hard (`_max`, swinging in), else softly.
const JUMP_RECEPTION_HARD_SPEED: f32 = 5.0;

/// A hold this close (m, across) to where he stands on a post is along its top edge, to pull down onto.
const POST_HOLD_REACH: f32 = 0.6;

/// `AC1_CLIMB_WHY`: log why each climbing move tried is turned down (for tracing a wall that will not climb).
fn why() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("AC1_CLIMB_WHY").is_ok())
}

/// How he stands on a beam: facing along it with that foot ahead (0 left, 1 right), or across it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum BeamStance {
    Along(usize),
    Across,
}
/// Standing on a perch: within this of its top line.
pub const PERCH_REACH: f32 = 0.3;
/// Running jumps are steered onto a perch they would land within this of.
const PERCH_MAGNET: f32 = 1.2;
/// Perch-to-perch jumps reach this far.
const PERCH_JUMP: std::ops::Range<f32> = 0.8..4.5;
/// A back eject leaves the wall at this speed (m/s), and this fast upward.
const EJECT_SPEED: f32 = 4.0;
const EJECT_UP: f32 = 2.5;
/// Wall runs need a wall at least this wide (m).
const WALL_RUN_WIDTH: f32 = 0.7;
/// Jump targets: this far across (m), and within this cosine of the wanted direction (45 degrees).
const JUMP_TARGET_REACH: std::ops::RangeInclusive<f32> = 1.0..=4.6;
const JUMP_TARGET_CONE: f32 = 0.707;
/// A running jump leaves the ground going up at least this fast (m/s), whatever it aims at.
const JUMP_UP_MIN: f32 = 1.5;
/// A jump up onto a top crosses its edge with the feet this far over it (m).
/// AC1's jump clips are used while the correction onto the target is at most this share of the distance.
const JUMP_AC1_SLACK: f32 = 0.5;
/// Topping out into the walk or the jog hands over at their speeds (m/s).
const TOP_OUT_WALK: f32 = 1.9;
const TOP_OUT_JOG: f32 = 3.5;
/// A rebound goes to a side when the stick leans along the wall at least this much.
const REBOUND_SIDE_MIN: f32 = 0.3;
/// Running on faster than this (m/s) after a jump, its reception ends once its step is taken.
const RECEPTION_CUT_SPEED: f32 = 2.0;
/// The directional grab in the air: the stick more than this far off the facing (cos) and not behind it (cos) looks for
/// a wall that way, this far (m).
const CATCH_SIDE_COS: f32 = 0.94;
const CATCH_BEHIND_COS: f32 = -0.5;
const CATCH_SIDE_REACH: f32 = 1.2;
/// A jump with no target is weighted for a level one this far ahead (m).
const FREE_JUMP_DIST: f32 = 2.5;
/// The way over a jump must be clear this high (m) above the higher of its two ends.
const JUMP_AC1_CLEAR: f32 = 0.6;
/// At this speed (m/s, the sprint) the reception is all its quick version.
const JUMP_AC1_FAST_SPEED: f32 = 6.2;
const JUMP_EDGE_CLEAR: f32 = 0.25;
/// A top jumped onto needs this much room over it (m).
const JUMP_TOP_HEADROOM: f32 = 1.7;
/// With no top in reach, a running jump aims at a hold to catch facing it: from this far over the feet to this far (m),
/// the root arriving this far under it (the catch onto the wall has the hands about 1.1 m over the root). AC1 takes an
/// edge at most 1.3 m up as a top to land on (`Human__ComputeJumpAnimBlend` 0xB1EC40, Banned445): only higher ones
/// are hung from (a waist-high wall's top is no hold to jump at).
const JUMP_HOLD_RISE: std::ops::RangeInclusive<f32> = 1.3..=2.6;
const JUMP_HOLD_HANG: f32 = 0.9;
/// ...this far in from the hold's ends (m), for both hands.
const JUMP_HOLD_INSET: f32 = 0.35;
/// A top jumped to may be up to 3 m below and 1.3 m above (AC1's candidate scorer and its jump bands for a top, via
/// Banned445's AC1-Movement-Rewritten).
const JUMP_TARGET_RISE: std::ops::RangeInclusive<f32> = -3.0..=1.3;
const SWING_CYCLE: [&str; 4] = ["xx_h_swing_cycle_front_up", "xx_h_swing_cycle_front_down", "xx_h_swing_cycle_back_up", "xx_h_swing_cycle_back_down"];
const SWING_LAUNCH: &str = "xx_h_swing_cycle_front_300cm_to_air";
const SWING_DROP: &str = "xx_h_swing_cycle_down_050cm_to_air";
/// Bar above the root while swinging (the hands in the swing cycle).
const SWING_HANG: f32 = 2.29;
/// A bar is caught when it comes within this of the root sideways, and this high above it.
const BAR_REACH: f32 = 0.7;
/// A bar is caught flying across it: its axis at most this cosine off square to the way (about 60°).
const BAR_ACROSS: f32 = 0.5;
const BAR_HEIGHT: std::ops::Range<f32> = 1.2..2.7;
/// Speed flung off a bar: forward and up.
const SWING_OFF: (f32, f32) = (4.5, 3.0);
/// Jumping onto a top takes off with AC1's takeoff from at least this far from it (m).
const VAULT_TAKEOFF_MIN: f32 = 1.3;
const LADDER_L: &str = "ladder_l";
const LADDER_R: &str = "ladder_r";
/// The rungs are this far in front of the root (m).
const LADDER_OFF: f32 = 0.44;
const LADDER_STEP: f32 = 0.5;
/// Across from a wall onto a ladder beside: the move's root ends this near its climbing spot (m), or a side leap's.
const LADDER_SIDE_REACH: f32 = 0.45;
/// Running, a turn on the spot hands over to the run once it and the stick are within this (rad) of each other.
const TURN_BREAK: f32 = 0.5;
/// A top-out needs room to stand: this high over the spot (m), that spot this far in from the edge.
const TOP_OUT_HEADROOM: f32 = 1.7;
const TOP_OUT_STAND_IN: f32 = 0.5;
/// A steered move bends toward the stick at up to this (rad/s) on top of its clips' own turn.
const STEER_RATE: f32 = 4.0;
/// Rebounding off a ladder: speed out from the wall and up (m/s), as the wall run's rebound.
const LADDER_REBOUND: (f32, f32) = (3.5, 2.5);
const LADDER_LEAP_REACH: f32 = 0.8;
const LADDER_ON: &str = "xx_l_wait_hipm_footl_tr_l_ladder_climb_up_r";
const LADDER_PULLDOWN: [&str; 3] =
    ["xx_h_wait_hipm_footl_tr_ladder_pulldown", "xx_ladder_pulldown_tr_l_ladder_wait_r_a", "xx_ladder_pulldown_tr_l_ladder_wait_r_b"];
const MONKEY: &str = "monkey";
const MONKEY_ON: &str = "xx_h_kiosk_freestep_footr_tr_monkeybar_footr";
const MONKEY_STEP: &str = "xx_h_kiosk_monkeybar_footr";
const MONKEY_OFF: &str = "xx_h_kiosk_monkeybar_footr_tr_fall";
/// Jumping up into a frame starts under it or at most this far short of its end (m).
const MONKEY_REACH: f32 = 1.0;
/// The frame is this far above the root while hanging from it (m).
const MONKEY_HANG: f32 = 1.9;
/// A low obstacle run onto (see `vault`): top height above the feet, and how far ahead it is spotted; tops
/// up to `STEP_UP_MAX` are stepped onto from stopping against them (`collide`); the root ends this far in from the edge.
/// (From 0.5 m: AC1's obstacle response starts there (event 42's guard 0xB25230, a contact at least 0.5 m over the
/// feet), lower steps are walked up by the capsule (0.37 m step); a roof's raised tile edges are run over.)
const VAULT_HEIGHT: std::ops::RangeInclusive<f32> = 0.5..=1.3;
const VAULT_REACH: f32 = 2.3;
const STEP_UP_MAX: f32 = 0.85;
/// On a beam the stick walks along it while it leans along it at least this much (cos: 60 degrees), as the walk
/// keeps going (`beam_step`).
const BEAM_WALK_COS: f32 = 0.5;
/// Ground this near a perch's line (m) is the perch itself, not ground to step off onto.
const PERCH_OWN: f32 = 0.35;
/// Stepping off a perch onto ground ahead: the speed it walks (or, free running, runs) off at (m/s).
const PERCH_OFF_WALK: f32 = 1.9;
const PERCH_OFF_RUN: f32 = 5.2;
const STEP_ONTO_IN: f32 = 0.35;

#[derive(Clone, Copy)]
pub struct ClimbRig {
    pub hands: [usize; 2],
    pub feet: [usize; 2],
    pub reference: Option<usize>,
}

struct Move {
    clip: Arc<Clip>,
    t: f32,
    to: String,
    start: Vec3,
    start_rot: Quat,
    /// World offset added over the move on top of its root motion (lines the hands up with the
    /// holds, or the feet with the ground).
    correct: Vec3,
    /// Playback speed (0 holds the first frame, for the in-air pose).
    rate: f32,
    /// World rotation the root starts off by, eased out over the start of the move.
    ease_rot: Quat,
}

/// A clip waiting to play after the current move.
struct Queued {
    clip: Arc<Clip>,
    to: String,
    correct: Vec3,
    rate: f32,
}

impl Queued {
    fn new(clip: Arc<Clip>, to: impl Into<String>) -> Self {
        Self { clip, to: to.into(), correct: Vec3::ZERO, rate: 1.0 }
    }
}

/// What a move needs under the feet when it ends.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Feet {
    Wall,
    Free,
}

/// A move to try: one clip, or a chain played back to back (leaps).
struct Cand {
    names: Vec<String>,
    to: String,
    feet: Feet,
}

impl Cand {
    fn new(name: impl Into<String>, to: impl Into<String>, feet: Feet) -> Self {
        Self { names: vec![name.into()], to: to.into(), feet }
    }
}

/// Walking along a beam: a two-step cycle played by phase while the root follows the beam.
struct Cycle {
    clips: [Arc<Clip>; 2],
    foot: usize,
    phase: f32,
    dir: Vec3,
    speed: f32,
}

/// A side wall run in progress (see the module docs).
struct SideRun {
    t: f32,
    start: Vec3,
    /// Along the wall, and out of it.
    along: Vec3,
    normal: Vec3,
    /// A point on the wall face, and the root's distance from it at the start.
    wall: Vec3,
    dist0: f32,
    speed: f32,
    clips: [Arc<Clip>; 2],
    foot: usize,
    phase: f32,
}

pub struct WallClimb {
    pub state: String,
    /// Out of the wall, horizontal.
    pub normal: Vec3,
    mv: Option<Move>,
    /// Clips to play after the current move (top-out, step-off, drop, leap and landing chains).
    queue: Vec<Queued>,
    wait: Option<Arc<Clip>>,
    /// The wait clip is the last move held on its final frame (states without a hang loop).
    wait_hold: bool,
    wait_t: f32,
    /// Looking round on the wall that way (`look_around`): "up", "down", "left" or "right".
    looking: Option<&'static str>,
    /// Pose we are fading from, the fade time left and its length.
    fade: Option<(Pose, f32, f32)>,
    last: Option<Pose>,
    /// Last vertical choice, so up/down alternates hands.
    last_hand_up: usize,
    /// Let go once the current move ends.
    want_drop: bool,
    /// Velocity while airborne (the root follows gravity, not the clip).
    fall_v: Option<Vec3>,
    /// Upward speed added when the next fall starts (running jump takeoff).
    launch: f32,
    /// Airborne from a jump: grab holds within reach on the way down.
    can_catch: bool,
    /// A fall off an edge walked off: it catches what is in reach while the legs are held (AC1's grab request).
    grab_on_legs: bool,
    /// The legs held now (set each update).
    legs_held: bool,
    /// A clip (by name) ended early, at this time into it: a reception whose step is over, running on.
    cut: Option<(String, f32)>,
    /// Pose clip to switch to once the jump starts coming down.
    descend: Option<Arc<Clip>>,
    /// Velocity to start the next fall with, instead of the previous clip's (rebound, leap of faith).
    fall_with: Option<Vec3>,
    /// Coming down in a leap of faith (lands in the hay with the faith landing).
    faith: bool,
    /// Highest point of the current fall (for landing damage).
    fall_top: f32,
    /// Health before this landing (set by the caller), damage it takes, and whether it was fatal.
    pub health: f32,
    pub damage: f32,
    pub dead: bool,
    side_run: Option<SideRun>,
    /// Set when the climber is back on its feet (on top of the wall or on the ground below).
    pub finished: bool,
    /// A ground action the player may break off by steering (a run stop).
    pub cancel: bool,
    /// A ground move that follows the stick (a turn round): broken off when the stick lets go of this
    /// direction (world), turns away from it, or the legs are pressed.
    pub steer: Option<Vec3>,
    /// Reaching to catch while falling: only holds below this height (world).
    catch_below: Option<f32>,
    /// The move whose blend time was taken from the move graph (see `update`).
    blended_for: Option<String>,
    /// Stopped against a low obstacle: its top's height (world).
    collide_top: f32,
    steer_stop: Option<String>,
    /// The last clip played to its end (where locomotion picks up after the move).
    pub last_clip: Option<String>,
    /// The legs pressed while sitting: get up.
    pub get_up: bool,
    /// Leaning: how much of the 150 cm lean clips (the rest the 70 cm ones).
    lean_mix: f32,
    /// Just landed: the drop (m) and the damage taken (for the HUD).
    pub landed: Option<(f32, f32)>,
    /// A vault's root path, followed instead of its clips' root motion (see `VaultPath`).
    vault_path: Option<VaultPath>,
    /// Ground velocity to hand back to locomotion when finished (running landings).
    pub exit_velocity: Vec3,
    /// Wanted world direction of travel (set by the caller; length 0..1), and sprinting.
    pub move_dir: Vec3,
    pub sprint: bool,
    /// High profile held (the ladder's `xx_h_` set).
    pub high: bool,
    /// Perch stood on, and the walk along it (beams).
    perch: Option<usize>,
    /// On a beam, crouched: along it or across (`None`: balancing as on a post, just landed).
    beam_stance: Option<BeamStance>,
    /// The foot the next free-step jump takes off from (the last one's reception lands on the other, so they alternate).
    pub freestep_left: bool,
    cycle: Option<Cycle>,
    /// Ladder or kiosk frame being climbed.
    ladder: Option<usize>,
    monkey: Option<usize>,
    /// The current move plays mirrored left to right.
    mirrored: bool,
    /// This fall was steered already (onto a perch, or aimed by a perch jump).
    aimed: bool,
    /// Bar swung on, letting go of it at the next forward swing (true: fling forward, false: drop),
    /// and the bar just let go of (not caught again for a moment).
    bar: Option<usize>,
    swing_off: Option<bool>,
    skip_bar: Option<(usize, f32)>,
}

pub fn grip_target(ledge_point: Vec3, out: Vec3) -> Vec3 {
    ledge_point + out * GRIP_OUT - Vec3::Y * GRIP_DOWN
}

/// Nearest hold for a wrist position: (ledge, wrist target on it, distance).
pub fn nearest_hold(level: &Level, wrist: Vec3, normal: Vec3) -> Option<(Ledge, Vec3, f32)> {
    level
        .ledges
        .iter()
        .filter(|l| l.out.dot(normal) > 0.7)
        .map(|l| {
            let line = wrist - l.out * GRIP_OUT + Vec3::Y * GRIP_DOWN;
            let target = grip_target(l.closest(line), l.out);
            (*l, target, (target - wrist).length())
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))
}

/// World rotation of the model frame for a root rotation.
fn world_rot(root_rot: Quat) -> Quat {
    root_rot * model_to_bevy()
}

/// A model-space rotation expressed as a change of the root's rotation.
fn root_delta(q_model: Quat) -> Quat {
    model_to_bevy() * q_model * model_to_bevy().inverse()
}

/// Where a clip leaves the climber: model-space wrists and ankles (relative to the end root), root
/// motion and root rotation (model space).
struct EndPose {
    hands: [Vec3; 2],
    feet: [Vec3; 2],
    motion: Vec3,
    turn: Quat,
}

fn end_pose(clip: &Clip, rig: &Rig, base: &Pose, cr: ClimbRig) -> EndPose {
    let mut pose = base.clone();
    let end = clip.frames();
    sample(clip, end, &mut pose, cr.reference);
    let m = pose.model(rig);
    EndPose { hands: cr.hands.map(|b| m[b].pos), feet: cr.feet.map(|b| m[b].pos), motion: root_motion_at(clip, end), turn: root_rotation_at(clip, end) }
}

/// End pose of clips played back to back: the last clip's limbs after the summed root motion
/// (each clip's motion is relative to the root rotation it starts from).
fn chain_end(clips: &[Arc<Clip>], rig: &Rig, base: &Pose, cr: ClimbRig) -> Option<EndPose> {
    let mut end = end_pose(clips.last()?, rig, base, cr);
    let (mut motion, mut turn) = (Vec3::ZERO, Quat::IDENTITY);
    for c in clips {
        let f = c.frames();
        motion += turn * root_motion_at(c, f);
        turn *= root_rotation_at(c, f);
    }
    end.motion = motion;
    end.turn = turn;
    Some(end)
}

/// `<base>_a`, `<base>_b`, ... as present in the library, in order.
fn chain_names(lib: &AnimLib, base: &str) -> Vec<String> {
    let prefix = format!("{base}_");
    let mut names: Vec<String> =
        lib.names.iter().filter(|n| n.strip_prefix(&prefix).is_some_and(|s| s.len() == 1 && s.chars().all(|c| c.is_ascii_lowercase()))).cloned().collect();
    names.sort();
    names
}

/// End of a move started from `root`: root rotation, world wrists and world ankles.
struct WorldEnd {
    pos: Vec3,
    rot: Quat,
    hands: [Vec3; 2],
    feet: [Vec3; 2],
}

impl EndPose {
    fn world(&self, root: &Transform) -> WorldEnd {
        let pos = root.translation + world_rot(root.rotation) * self.motion;
        let rot = root.rotation * root_delta(self.turn);
        let r = world_rot(rot);
        WorldEnd { pos, rot, hands: self.hands.map(|h| pos + r * h), feet: self.feet.map(|f| pos + r * f) }
    }
}

fn clip_state_names(lib: &AnimLib, from: &str, dir: &str) -> Vec<(String, String)> {
    let prefix = format!("xx_l_climb_{from}_{dir}_");
    lib.names
        .iter()
        .filter_map(|n| n.strip_prefix(&prefix).map(|to| (n.clone(), to.to_string())))
        .filter(|(_, to)| to.len() <= 3 && !to.contains('_'))
        .collect()
}

/// The hold for a wrist at `wrist` in AC1's climbing probe box (`BuildHoldGrid` 0xDF6A40, docs/PARKOUR.md section 4):
/// `GRID_ALONG` along the wall, `GRID_UP` up or down and `GRID_DEPTH` in or out of the wall; the nearest one in it.
fn hold_in_box(level: &Level, wrist: Vec3, normal: Vec3) -> Option<Vec3> {
    let along = Vec3::Y.cross(normal).normalize_or_zero();
    level
        .ledges
        .iter()
        .filter(|l| l.out.dot(normal) > 0.7)
        .map(|l| grip_target(l.closest(wrist - l.out * GRIP_OUT + Vec3::Y * GRIP_DOWN), l.out))
        .filter(|t| {
            let d = *t - wrist;
            d.dot(along).abs() <= GRID_ALONG && d.y.abs() <= GRID_UP && d.dot(normal).abs() <= GRID_DEPTH
        })
        .min_by(|a, b| (*a - wrist).length().total_cmp(&(*b - wrist).length()))
}

/// Both hands' holds in AC1's probe box (`hold_in_box`).
fn holds_in_box(level: &Level, hands: &[Vec3; 2], normal: Vec3) -> Option<[Vec3; 2]> {
    Some([hold_in_box(level, hands[0], normal)?, hold_in_box(level, hands[1], normal)?])
}

fn holds_within(level: &Level, hands: &[Vec3; 2], normal: Vec3, reach: f32) -> Option<[Vec3; 2]> {
    let t0 = nearest_hold(level, hands[0], normal).filter(|h| h.2 < reach)?;
    let t1 = nearest_hold(level, hands[1], normal).filter(|h| h.2 < reach)?;
    Some([t0.1, t1.1])
}

/// Holds under the hands (at `hands`, the middle of the two) for the feet to stand on, as AC1's climbing stance "1m"
/// needs: a hold on this wall `FOOTHOLD_BELOW` under them, about level with them along it. Without one (a lone ledge
/// at the top of a bare wall) he hangs with the feet braced on the wall (`hangwall`).
fn footholds(level: &Level, hands: Vec3, normal: Vec3) -> bool {
    let along = Vec3::Y.cross(normal).normalize_or_zero();
    // (Low on a wall the feet are on the ground, or near it: the lowest holds, as climbing on from the ground.)
    let ground = level.ground(hands + normal * 0.3 - Vec3::Y * 0.3, 0.0, *FOOTHOLD_BELOW.end()).is_some();
    ground
        || level.ledges.iter().filter(|l| l.out.dot(normal) > 0.7).any(|l| {
            let q = l.closest(hands - Vec3::Y * 1.1);
            let d = q - hands;
            FOOTHOLD_BELOW.contains(&-d.y) && d.dot(along).abs() < FOOTHOLD_SIDE && d.dot(normal).abs() < FOOTHOLD_DEPTH
        })
}

/// How far under the hands (m) a hold counts as one for the feet in "1m" (they stand about 1.1 m under), how far
/// along the wall to either side, and how far out of the wall's plane.
const FOOTHOLD_BELOW: std::ops::RangeInclusive<f32> = 0.5..=1.7;
const FOOTHOLD_SIDE: f32 = 0.45;
const FOOTHOLD_DEPTH: f32 = 0.3;

fn feet_fit(level: &Level, feet: &[Vec3; 2], normal: Vec3, want: Feet) -> bool {
    feet_fit_at(level, feet, normal, want, None)
}

/// `feet_fit`, where the hands hold at `hold`: a wall set back from the hold line by more than `RECESS_MAX`
/// (a pole under a cap) gives the feet nothing to stand on.
fn feet_fit_at(level: &Level, feet: &[Vec3; 2], normal: Vec3, want: Feet, hold: Option<Vec3>) -> bool {
    let flush = |f: Vec3| {
        level
            .raycast(f + normal * 0.3, -normal, 0.3 + FOOT_REACH)
            .is_some_and(|h| h.normal.dot(normal) > 0.7 && hold.is_none_or(|q| (q - h.point).dot(normal) < RECESS_MAX))
    };
    // (Between the feet too: a thin pole between them is footing.)
    let between = (feet[0] + feet[1]) * 0.5;
    let braced = feet.iter().filter(|f| flush(**f)).count() + usize::from(flush(between));
    match want {
        Feet::Wall => braced > 0,
        Feet::Free => braced == 0,
    }
}

/// A flight whose knees meet an edge with its top at most this far over the feet (m) lands on it.
const KNEE_TOP: f32 = 0.7;

/// Does a flight from `p` at velocity `v` (gravity bending it) pass `t` seconds without walking into a wall, at the
/// knees or the chest (where `WallClimb::fall` stops a flight)?
fn arc_clear(level: &Level, p: Vec3, v: Vec3, t: f32) -> bool {
    let at = |s: f32| p + v * s - Vec3::Y * (0.5 * GRAVITY * s * s);
    let n = ((t / 0.05).ceil() as usize).clamp(1, 80);
    (0..n).all(|i| {
        let (a, b) = (at(t * i as f32 / n as f32), at(t * (i + 1) as f32 / n as f32));
        let step = (b - a).with_y(0.0);
        let wall =
            |h: f32| step.length() >= 1e-4 && level.raycast(a + Vec3::Y * h, step.normalize(), step.length() + 0.3).is_some_and(|hit| hit.normal.y.abs() < 0.5);
        // (A top at the knees is landed on, not a wall: see `fall`.)
        !wall(1.0) && (!wall(0.3) || level.ground(b + Vec3::Y * 1.0, 0.0, 1.0).is_some_and(|g| (0.0..KNEE_TOP).contains(&(g.point.y - b.y))))
    })
}

/// Is `p` on a ledge shorter than `NARROW_LEDGE` (a pole's cap)?
fn on_narrow_ledge(level: &Level, p: Vec3) -> bool {
    level.ledges.iter().any(|l| l.a.distance(l.b) < NARROW_LEDGE && l.closest(p).distance(p) < 0.3)
}

/// Is there a wall at least `WALL_RUN_WIDTH` wide ahead of `from` along `fwd`, `dist` away (rays to either side
/// hit it too)? Not the side of a pole or post.
pub fn wide_wall(level: &Level, from: Vec3, fwd: Vec3, dist: f32) -> bool {
    let side = fwd.cross(Vec3::Y).normalize_or_zero() * WALL_RUN_WIDTH * 0.5;
    [side, -side].iter().all(|s| level.raycast(from + *s, fwd, dist + 0.3).is_some_and(|h| h.normal.y.abs() < 0.3))
}

/// Would moving the root by `slide` (horizontal) walk it into a wall? Probed at knee and chest height.
/// How far a top at height `top` met `dist` ahead along `dir` stands above the floor just in front of it: up stairs
/// or a ramp the floor rises toward the face, and a riser met at shin height is one step, not an obstacle.
fn rise(level: &Level, p: Vec3, dir: Vec3, dist: f32, top: f32) -> f32 {
    let before = level.ground(p + dir * (dist - 0.1).max(0.0), 0.5, 0.3).map_or(p.y, |g| g.point.y.max(p.y));
    top - before
}

fn blocked(level: &Level, root: Vec3, slide: Vec3) -> bool {
    let dir = slide.normalize();
    [0.5, 1.2].iter().any(|&h| level.raycast(root + Vec3::Y * h, dir, slide.length() + 0.35).is_some_and(|hit| hit.normal.y.abs() < 0.5))
}

/// Clips that pull up from the waist rise first and go over the edge at the end (`xx_h_hangwaist_*`): their
/// correction is applied late, so the body does not cut into the edge on the way up.
fn late_correction(name: &str) -> bool {
    name.starts_with("xx_h_hangwaist")
}

/// A collide clip mixed by height: `{h}` as `050` and `070`, weight `mix` on the 70 cm one.
fn collide_name(pattern: &str, mix: f32) -> String {
    mix_name(&[(&pattern.replace("{h}", "050"), 1.0 - mix), (&pattern.replace("{h}", "070"), mix)])
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// What a jump up or wall run ends on.
#[derive(Clone, Copy)]
enum GrabEnd {
    /// Kneeling on a walkable top this high above the feet, then standing up.
    Onto(f32),
    /// Hanging in this state, with the hands of this hang loop on holds.
    Hang(&'static str, &'static str),
}

/// A jump up or wall run: its clips (the last `catch` of them settle into the end), then `after`.
struct GrabOpt {
    names: Vec<String>,
    catch: usize,
    end: GrabEnd,
    after: Vec<String>,
}

fn strings(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| n.to_string()).collect()
}

/// A lean clip family (`{h}` for its height) mixed `mix` of the 150 cm one to the 70 cm one.
fn lean_name(pattern: &str, mix: f32) -> String {
    mix_name(&[(&pattern.replace("{h}", "070"), 1.0 - mix), (&pattern.replace("{h}", "150"), mix)])
}

/// AC1 blends a move's variants made for different heights (`hangknee_201cm` and `_250cm`, the entries'
/// `131cm` and `200cm`): for a top `height` above the feet between two options' heights, the mix of the two
/// made for exactly that height (clips that differ are mixed by weight, shared ones kept).
fn blend_onto(opts: &[GrabOpt], height: f32) -> Vec<GrabOpt> {
    let mut out = vec![];
    for a in opts {
        for b in opts {
            let (GrabEnd::Onto(ha), GrabEnd::Onto(hb)) = (a.end, b.end) else { continue };
            // Variants of one move: the same clips but for the numbers in their names.
            let shape = |n: &String| n.chars().filter(|c| !c.is_ascii_digit()).collect::<String>();
            let same = a.names.iter().zip(&b.names).all(|(x, y)| shape(x) == shape(y));
            if !(ha < height && height < hb) || a.names.len() != b.names.len() || !same || a.catch != b.catch || hb - ha > 0.8 {
                continue;
            }
            let w = (height - ha) / (hb - ha);
            let names = a.names.iter().zip(&b.names).map(|(x, y)| if x == y { x.clone() } else { mix_name(&[(x, 1.0 - w), (y, w)]) }).collect();
            let after = if w < 0.5 { a.after.clone() } else { b.after.clone() };
            out.push(GrabOpt { names, catch: a.catch, end: GrabEnd::Onto(height), after });
        }
    }
    out
}

/// Standing jumps up to a ledge (see the module docs).
fn jump_options() -> Vec<GrabOpt> {
    let jump = |kind: &str, cm: u32| {
        let main = format!("xx_h_jumpstraight_footl_to_{kind}_{cm}cm");
        vec![STRAIGHT_JUMP[0].to_string(), main.clone(), format!("{main}_tr_{kind}_a"), format!("{main}_tr_{kind}_b")]
    };
    let onto =
        JUMP_ONTO.iter().map(|&(kind, cm, after)| GrabOpt { names: jump(kind, cm), catch: 2, end: GrabEnd::Onto(cm as f32 / 100.0), after: strings(after) });
    let hang = JUMP_HANG.iter().map(|&(kind, cm, to, rest)| GrabOpt { names: jump(kind, cm), catch: 2, end: GrabEnd::Hang(to, rest), after: vec![] });
    onto.chain(hang).collect()
}

/// Wall-run endings, highest first.
fn wall_run_options() -> Vec<GrabOpt> {
    let run = |tail: &[&str]| WALL_RUN.iter().chain(tail).map(|n| n.to_string()).collect::<Vec<_>>();
    let entry = |tail: &str| vec![WALL_RUN[0].to_string(), WALL_RUN[1].to_string(), tail.to_string()];
    let step = "xx_h_wallingfront_step1_footr_tr";
    let stand_r = vec!["xx_h_hangknee_footr_tr_h_wait_footl_a".to_string()];
    let stand_l = vec![STAND_UP.to_string()];
    let wall = GrabEnd::Hang(HANGWALL_OPEN, HANGWALL_REST);
    let free = GrabEnd::Hang(FREE, "xx_h_hangfree_waitclose");
    vec![
        GrabOpt { names: run(&[&format!("{step}_hangwall_430cm_000cm_a"), &format!("{step}_hangwall_430cm_000cm_b")]), catch: 2, end: wall, after: vec![] },
        GrabOpt { names: run(&[&format!("{step}_hangknee_250cm_footr")]), catch: 1, end: GrabEnd::Onto(2.5), after: stand_r.clone() },
        GrabOpt { names: run(&[&format!("{step}_hangknee_201cm_footr")]), catch: 1, end: GrabEnd::Onto(2.0), after: stand_r },
        GrabOpt { names: run(&[&format!("{step}_hangfree_400cm_swingback_min")]), catch: 1, end: free, after: vec![] },
        GrabOpt { names: run(&[&format!("{step}_hangfree_350cm_swingback_min")]), catch: 1, end: free, after: vec![] },
        GrabOpt { names: run(&[&format!("{step}_hangwall_251cm_000cm_a"), &format!("{step}_hangwall_251cm_000cm_b")]), catch: 2, end: wall, after: vec![] },
        GrabOpt { names: entry("xx_h_wallingfront_entry_footl_tr_hangknee_200cm_footl"), catch: 1, end: GrabEnd::Onto(2.0), after: stand_l.clone() },
        GrabOpt { names: entry("xx_h_wallingfront_entry_footl_tr_hangknee_131cm_footl"), catch: 1, end: GrabEnd::Onto(1.31), after: stand_l },
    ]
}

/// How far the hands and feet move between two poses (m), for sizing crossfades.
fn seam_gap(rig: &Rig, cr: ClimbRig, a: &Pose, b: &Pose) -> f32 {
    let (ma, mb) = (a.model(rig), b.model(rig));
    cr.hands.iter().chain(&cr.feet).map(|&i| (ma[i].pos - mb[i].pos).length()).fold(0.0, f32::max)
}

/// Health lost landing after a drop of `drop` metres with `health` left (all of it is fatal).
fn landing_damage(drop: f32, health: f32) -> f32 {
    if drop >= FATAL_DROP {
        health
    } else if drop >= HEAVY_DROP {
        HEAVY_DAMAGE
    } else if drop >= SAFE_DROP {
        SMALL_DAMAGE
    } else {
        0.0
    }
}

/// Airtime of a running jump landing at the height it left from (s).
fn jump_airtime() -> f32 {
    2.0 * JUMP_UP_SPEED / GRAVITY
}

/// Where a running jump along `dir` from `from` flies to catch a hold, when no top is in reach: the nearest hold within
/// AC1's 45 degree cone and reach, facing the jumper, `JUMP_HOLD_RISE` of the feet; the point the body hangs from it.
fn jump_hold_target(level: &Level, from: Vec3, dir: Vec3) -> Option<Vec3> {
    jump_hold(level, from, dir).map(|(q, out)| q + out * 0.4 - Vec3::Y * JUMP_HOLD_HANG)
}

/// The hold a running jump along `dir` from `from` reaches for (see `jump_hold_target`): the point on it the hands go to,
/// and its outward normal.
fn jump_hold(level: &Level, from: Vec3, dir: Vec3) -> Option<(Vec3, Vec3)> {
    let dir = dir.with_y(0.0).normalize_or_zero();
    level
        .ledges
        .iter()
        .filter(|l| l.out.dot(dir) < -0.5)
        .filter(|l| l.a.distance(l.b) > 2.0 * JUMP_HOLD_INSET)
        .map(|l| {
            // (Both hands on it: in from its ends.)
            let along = (l.b - l.a).normalize();
            let inner = Line { a: l.a + along * JUMP_HOLD_INSET, b: l.b - along * JUMP_HOLD_INSET };
            (inner.closest(from + dir * 2.5), l.out)
        })
        .filter(|(q, _)| {
            let flat = (*q - from).with_y(0.0);
            JUMP_TARGET_REACH.contains(&flat.length()) && flat.normalize().dot(dir) >= JUMP_TARGET_CONE && JUMP_HOLD_RISE.contains(&(q.y - from.y))
        })
        .min_by(|a, b| (a.0 - from).length().total_cmp(&(b.0 - from).length()))
}

/// A thin wall ahead for a running jump to go over with a hand on its top (AC1's passover): its face met within reach,
/// its top `PASSOVER_RISE` over the feet (too high to land on), at most `PASSOVER_DEPTH` deep with a drop beyond it. The
/// point on the top's near edge, the face's normal and the top's depth.
fn passover_target(level: &Level, from: Vec3, dir: Vec3) -> Option<(Vec3, Vec3, f32)> {
    let dir = dir.with_y(0.0).normalize_or_zero();
    let hit = [0.4, 0.9].iter().find_map(|h| level.raycast(from + Vec3::Y * *h, dir, *JUMP_TARGET_REACH.end()).filter(|h| h.normal.y.abs() < 0.3))?;
    let normal = hit.normal.with_y(0.0).normalize_or_zero();
    if normal.dot(dir) > -JUMP_TARGET_CONE || hit.dist < *JUMP_TARGET_REACH.start() {
        return None;
    }
    let fwd = -normal;
    let top = level.ground(hit.point.with_y(from.y + PASSOVER_RISE.end() + 0.3) + fwd * 0.05, 0.0, PASSOVER_RISE.end() + 0.3)?;
    if !PASSOVER_RISE.contains(&(top.point.y - from.y)) || top.normal.y < 0.8 {
        return None;
    }
    // (How deep the top runs before it drops away, at least half a metre, on the far side.)
    let on_top = |d: f32| level.ground(hit.point.with_y(top.point.y + 0.2) + fwd * d, 0.0, 0.4).is_some_and(|g| (g.point.y - top.point.y).abs() < 0.1);
    let depth = (1..=(PASSOVER_DEPTH / 0.05) as usize + 1).map(|k| k as f32 * 0.05).find(|&d| !on_top(d))?;
    if depth > PASSOVER_DEPTH || level.ground(hit.point.with_y(top.point.y) + fwd * (depth + 0.3), 0.0, 10.0).is_some_and(|g| top.point.y - g.point.y < 0.5) {
        return None;
    }
    // (Room over the top for the body going over.)
    if level.raycast(hit.point.with_y(top.point.y + 0.4) + normal * 0.3, fwd, depth + 1.0).is_some() {
        return None;
    }
    Some((hit.point.with_y(top.point.y), normal, depth))
}

/// A swing bar a jump along `dir` from `from` would reach: lying across the way, `BAR_JUMP_REACH` ahead and up to
/// `BAR_JUMP_RISE` over the feet; the point on it.
fn bar_ahead(level: &Level, from: Vec3, dir: Vec3) -> Option<Vec3> {
    let dir = dir.with_y(0.0).normalize_or_zero();
    level
        .bars
        .iter()
        .filter(|b| b.axis().dot(dir).abs() < BAR_ACROSS)
        .map(|b| b.closest(from + dir * 3.0 + Vec3::Y * 2.0))
        .filter(|q| {
            let flat = (*q - from).with_y(0.0);
            BAR_JUMP_REACH.contains(&flat.length()) && flat.normalize().dot(dir) > JUMP_TARGET_CONE && BAR_JUMP_RISE.contains(&(q.y - from.y))
        })
        .min_by(|a, b| (*a - from).length().total_cmp(&(*b - from).length()))
}

/// Velocity that flies from `from` to `to` under gravity, and the flight time (longer for longer jumps).
/// Where to jump to along `dir` from `from` (AC1's target choice, as the movement notes have it: within
/// 45 degrees of the wanted direction, at most 3 m down, the highest first, then the nearest): a post or
/// beam (`skip`: the one stood on), or a walkable top across a gap.
fn jump_target(level: &Level, from: Vec3, dir: Vec3, skip: Option<usize>) -> Option<Vec3> {
    let dir = dir.with_y(0.0).normalize_or_zero();
    let mut cands: Vec<Vec3> = vec![];
    for (i, l) in level.perches.iter().enumerate() {
        if Some(i) != skip {
            cands.extend([1.5, 2.5, 3.5, 4.5].map(|d| l.closest(from + dir * d)));
        }
    }
    // Tops beyond a drop: the first ground after a gap, a little in from its edge.
    for turn in [-0.35f32, 0.0, 0.35] {
        let d = Quat::from_rotation_y(turn) * dir;
        let mut gap = false;
        for k in 2..=18 {
            let t = k as f32 * 0.25;
            match level.ground(from + d * t, 1.2, -JUMP_TARGET_RISE.start() + 0.2) {
                None => gap = true,
                // (Room to stand over it, and not inside a block: a probe starting inside one finds the floor under it,
                // and looking up from there meets the block's roof from below.)
                Some(g)
                    if gap
                        && g.normal.y > 0.8
                        && level.ground(g.point + d * 0.4, 0.2, 0.2).is_some()
                        && level.raycast_sided(g.point + Vec3::Y * 0.05, Vec3::Y, 20.0).is_none_or(|(h, behind)| !behind && h.dist > JUMP_TOP_HEADROOM) =>
                {
                    cands.push(g.point + d * 0.4);
                    break;
                }
                _ => {}
            }
        }
    }
    // Roof edges from the climbing markup (AC1's candidates are guidance edges, `0xE96BF0`): an edge facing us, its top
    // at most 1.3 m up, landed on 0.45 m in from it, across a gap.
    for l in &level.ledges {
        let ahead = from + dir * 4.0;
        let on = l.closest(Vec3::new(ahead.x, l.a.y, ahead.z));
        if (on - from).with_y(0.0).length() > *JUMP_TARGET_REACH.end() + 1.0 || l.out.dot((from - on).with_y(0.0)) <= 0.0 || on.y - from.y > 1.3 {
            continue;
        }
        let land = on - l.out * ROOF_EDGE_INSET;
        let Some(g) = level.ground(land + Vec3::Y * 0.3, 0.0, 0.5).filter(|g| g.normal.y > 0.8) else { continue };
        // (A gap: no ground half a metre under the lower of the two anywhere between, looking from over the higher one: a
        // roof rising a little between is no gap.)
        let (low, high) = (from.y.min(g.point.y), from.y.max(g.point.y));
        let gap = [0.25, 0.5, 0.75].iter().any(|t| level.ground(from.lerp(g.point, *t).with_y(high + 0.3), 0.0, high + 0.3 - (low - 0.5)).is_none());
        if gap {
            cands.push(g.point);
        }
    }
    cands
        .into_iter()
        .filter(|q| {
            let flat = (*q - from).with_y(0.0);
            JUMP_TARGET_REACH.contains(&flat.length()) && flat.normalize().dot(dir) >= JUMP_TARGET_CONE && JUMP_TARGET_RISE.contains(&(q.y - from.y))
        })
        // (The way across clear over the higher of the two: a top under an awning or past a wall the flight meets is
        // no target, else the jump stops in the air and drops back, over and over.)
        .filter(|q| {
            let flat = (*q - from).with_y(0.0);
            // (At the knees and the chest too, where a flight is stopped: a low wall or a rail between is vaulted, not
            // jumped across.)
            let top = from.y.max(q.y);
            [0.3, JUMP_AC1_CLEAR, 1.0].iter().all(|h| level.raycast(from.with_y(top + h), flat.normalize_or_zero(), (flat.length() - 0.3).max(0.0)).is_none())
        })
        .max_by(|a, b| {
            // Higher, nearer, and on the line the player steers along.
            let score = |q: &Vec3| {
                let flat = (*q - from).with_y(0.0);
                (q.y - from.y) - 0.25 * flat.length() - 4.0 * (1.0 - flat.normalize().dot(dir))
            };
            score(a).total_cmp(&score(b))
        })
}

fn ballistic(from: Vec3, to: Vec3) -> (Vec3, f32) {
    let d = to - from;
    let t = 0.3 + 0.12 * d.with_y(0.0).length();
    (d.with_y(0.0) / t + Vec3::Y * (d.y + 0.5 * GRAVITY * t * t) / t, t)
}

/// Facing along a horizontal direction.
fn facing(dir: Vec3) -> Quat {
    Quat::from_rotation_arc(Vec3::NEG_Z, dir.with_y(0.0).normalize_or(Vec3::NEG_Z))
}

fn is_free(state: &str) -> bool {
    state.starts_with("free_")
}

/// States the climber rests in between moves with both hands level.
fn is_rest(state: &str) -> bool {
    matches!(state, "1m" | "2m" | FREE | FREE_OPEN | HANGWALL | HANGWALL_OPEN)
}

/// Not hanging by the hands: topping out, leaping, jumping or leaving the wall.
fn off_wall(state: &str) -> bool {
    matches!(
        state,
        KNEEL
            | ON_TOP
            | STEP_DOWN
            | DROP
            | FALL
            | LAND
            | GROUND
            | LEAP
            | JUMP
            | WALL_RUN_STATE
            | FAITH
            | HAY
            | HAY_OUT
            | DEAD
            | SIDE_RUN
            | PERCH
            | SWING
            | VAULT
            | GROUND_ACT
            | LEAN
            | BENCH
            | BENCH_SIT
            | LADDER_L
            | LADDER_R
            | MONKEY
    )
}

fn side_name(dir: &str) -> &'static str {
    if dir == "r" { "right" } else { "left" }
}

/// States a chain of `n` clips passes through: the first clip winds up in place (`from`), the
/// second is the flight, the rest catch and end in `to`.
fn chain_states(n: usize, from: &str, to: &str) -> Vec<String> {
    (0..n)
        .map(|i| match i {
            _ if i + 1 == n || i >= 2 => to,
            0 => from,
            _ => LEAP,
        })
        .map(String::from)
        .collect()
}

impl WallClimb {
    fn new(state: &str, normal: Vec3) -> Self {
        WallClimb {
            state: state.into(),
            normal,
            mv: None,
            queue: vec![],
            wait: None,
            wait_hold: false,
            wait_t: 0.0,
            looking: None,
            fade: None,
            last: None,
            last_hand_up: 0,
            want_drop: false,
            fall_v: None,
            launch: 0.0,
            can_catch: false,
            grab_on_legs: false,
            legs_held: false,
            cut: None,
            descend: None,
            fall_with: None,
            faith: false,
            fall_top: f32::NEG_INFINITY,
            health: 1.0,
            damage: 0.0,
            dead: false,
            side_run: None,
            finished: false,
            cancel: false,
            steer: None,
            catch_below: None,
            blended_for: None,
            collide_top: 0.0,
            steer_stop: None,
            last_clip: None,
            get_up: false,
            lean_mix: 1.0,
            landed: None,
            vault_path: None,
            exit_velocity: Vec3::ZERO,
            move_dir: Vec3::ZERO,
            sprint: false,
            high: false,
            perch: None,
            beam_stance: None,
            freestep_left: true,
            cycle: None,
            ladder: None,
            monkey: None,
            mirrored: false,
            aimed: false,
            bar: None,
            swing_off: None,
            skip_bar: None,
        }
    }

    /// Start climbing from the ground: the wall must be in front with a hold at the entry clip's reach.
    #[allow(clippy::too_many_arguments)]
    pub fn try_enter(lib: &mut AnimLib, level: &Level, root: &Transform, rig: &Rig, base: &Pose, cr: ClimbRig, from: Option<Pose>) -> Option<WallClimb> {
        let fwd = root.rotation * Vec3::NEG_Z;
        let Some(hit) = level.raycast(root.translation + Vec3::Y * 1.2, fwd, 1.5) else {
            info!("climb: no wall ahead");
            return None;
        };
        if hit.normal.y.abs() > 0.3 {
            info!("climb: surface ahead is not a wall");
            return None;
        }
        let normal = Vec3::new(hit.normal.x, 0.0, hit.normal.z).normalize();
        let clip = lib.get(ENTRY)?;
        let end = end_pose(&clip, rig, base, cr);
        let face = Quat::from_rotation_arc(Vec3::NEG_Z, -normal);
        let r = world_rot(face);
        // Hands at the end of the jump, relative to where it starts.
        let centre_model = (end.hands[0] + end.hands[1]) * 0.5 + end.motion;
        let reach = root.translation + r * centre_model;
        let Some((_, target, d)) = nearest_hold(level, reach, normal) else {
            info!("climb: no holds on this wall");
            return None;
        };
        if (target.y - reach.y).abs() > 0.3 || d > 0.8 {
            info!("climb: nearest hold {target} is out of reach of the jump ({reach}, {d:.2} m)");
            return None;
        }
        // Plan the start so the entry ends with the hands on that hold; ease in from where we stand.
        let mut start = target - r * centre_model;
        start.y = root.translation.y;
        let planned = Transform { translation: start, rotation: face, ..*root };
        let mut w = WallClimb::new(ENTRY_STATE, normal);
        w.start(clip, ENTRY_STATE.into(), &planned);
        // (Up or down onto the hold over the move: AC1 has the one entry, and a city's first hold is not at its height.)
        if let Some(m) = &mut w.mv {
            m.correct = Vec3::Y * (target.y - reach.y);
        }
        w.ease_in(from, root);
        w.wait = lib.get(&format!("xx_climb_wait_{ENTRY_STATE}"));
        Some(w)
    }

    /// Running jump from the ground along the way the root faces. `from` is the pose shown now, to
    /// fade from.
    pub fn jump(lib: &mut AnimLib, level: &Level, root: &Transform, dir: Vec3, left: bool, from: Option<Pose>) -> Option<WallClimb> {
        // AC1 always jumps at a target; with none in reach, AC1's takeoff and flight weighted for a level one
        // `FREE_JUMP_DIST` ahead, played out, then the fall (as Banned445's repo has it). Our sprint stride and air clip
        // if those are missing. At a swing bar ahead, AC1's flight onto it (`_to_swing`); the fall catches it.
        let p = root.translation;
        let j = match bar_ahead(level, p, dir) {
            Some(q) => {
                debug!("climb: jumping at the swing bar at {q:.2}");
                crate::jump::swing(q.y - SWING_HANG - p.y, (q - p).with_y(0.0).length(), left)
            }
            None => crate::jump::running(0.0, FREE_JUMP_DIST, left, dir.with_y(0.0).length() / JUMP_AC1_FAST_SPEED),
        };
        let mut mix = |parts: &[(String, f32)]| {
            let parts: Vec<(&str, f32)> = parts.iter().map(|(n, w)| (n.as_str(), *w)).collect();
            lib.get(&mix_name(&parts))
        };
        let ac1 = mix(&j.takeoff).zip(mix(&j.flight));
        let (takeoff, air) = match ac1 {
            Some(pair) => pair,
            None => (lib.get(JUMP_TAKEOFF)?, lib.get(JUMP_AIR)?),
        };
        let planned = Transform { rotation: Quat::from_rotation_arc(Vec3::NEG_Z, dir.with_y(0.0).normalize_or(Vec3::NEG_Z)), ..*root };
        let mut w = WallClimb::new(JUMP, planned.rotation * Vec3::Z);
        // The air clip runs from the takeoff tuck to reaching for the ground over the flight.
        let rate = air.anim.duration / jump_airtime();
        w.queue = vec![Queued { rate, ..Queued::new(air, FALL) }];
        w.launch = JUMP_UP_SPEED;
        w.can_catch = true;
        w.start(takeoff, JUMP.into(), &planned);
        w.ease_in(from, root);
        Some(w)
    }

    /// A running jump aimed at the best place to land ahead (`jump_target`: a post, a beam, a top across a
    /// gap), flying an arc that comes down on it; with none, the plain running jump.
    /// Something to jump to from `from` along `dir` (a post, a beam, a top across a gap): see `jump_target`.
    pub fn jump_target_ahead(level: &Level, from: Vec3, dir: Vec3) -> bool {
        jump_target(level, from, dir, None).is_some()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn jump_aimed(
        lib: &mut AnimLib,
        level: &Level,
        root: &Transform,
        rig: &Rig,
        base: &Pose,
        cr: ClimbRig,
        dir: Vec3,
        left: bool,
        from: Option<Pose>,
    ) -> Option<WallClimb> {
        let speed = dir.with_y(0.0).length();
        let p = root.translation;
        let top = jump_target(level, p, dir, None);
        let at_hold = top.is_none();
        // A thin wall ahead, too high to land on: over it, a hand on its top (AC1's passover).
        if top.is_none()
            && let Some(w) = Self::jump_passover(lib, level, root, rig, base, cr, dir, left, from.clone())
        {
            return Some(w);
        }
        let Some(mut to) = top.or_else(|| jump_hold_target(level, p, dir)) else { return Self::jump(lib, level, root, dir, left, from) };
        let aim = (to - p).with_y(0.0).normalize_or(dir);
        // Onto a top, a post or a beam (not a hold): AC1's own jump clips, when they fit (a post's and a beam's flight is
        // the same free-step one; landed, the ground hands over to balancing on it).
        if !at_hold && let Some(w) = Self::jump_ac1(lib, level, root, to, aim, speed, left, from.clone()) {
            return Some(w);
        }
        // At a lone ledge with the wall under it (no holds for the feet): AC1's flight onto the surface and its reception
        // on the wall, into the wall hang.
        if at_hold && let Some(w) = Self::jump_hangwall(lib, level, root, rig, base, cr, dir, left, from.clone()) {
            return Some(w);
        }
        // (At a hold: the planned arc, to catch it, with AC1's takeoff and flight for that height and distance.)
        let j = crate::jump::running(to.y - p.y, (to - p).with_y(0.0).length(), left, speed / JUMP_AC1_FAST_SPEED);
        let mut mix = |parts: &[(String, f32)]| {
            let parts: Vec<(&str, f32)> = parts.iter().map(|(n, w)| (n.as_str(), *w)).collect();
            lib.get(&mix_name(&parts))
        };
        let (takeoff, air) = match mix(&j.takeoff).zip(mix(&j.flight)) {
            Some(pair) => pair,
            None => (lib.get(JUMP_TAKEOFF)?, lib.get(JUMP_AIR)?),
        };
        let planned = Transform { rotation: facing(aim), ..*root };
        // The arc starts where the takeoff leaves the ground: at the edge at the latest. The takeoff clip runs on about
        // 1.7 m; pressed closer to the edge than that, it starts partway in (at its own pace), not run on over the drop.
        let rot = world_rot(planned.rotation);
        let end_motion = root_motion_at(&takeoff, takeoff.frames());
        let edge = (1..=40).map(|k| k as f32 * 0.05).find(|&d| level.ground(p + aim * d, 0.3, 0.5).is_none());
        let skip = edge.and_then(|e| {
            let left = |f: f32| (rot * (end_motion - root_motion_at(&takeoff, f))).with_y(0.0).dot(aim);
            (left(0.0) > e - 0.1).then(|| (0..takeoff.frames() as usize).map(|f| f as f32).find(|&f| left(f) <= e - 0.1).unwrap_or(takeoff.frames()))
        });
        let skipped = skip.map_or(Vec3::ZERO, |f| rot * root_motion_at(&takeoff, f));
        let lift = p + rot * end_motion - skipped;
        // At the speed it runs (a low, quick arc), and no slower than a standing leap.
        let mut d = (to - lift).with_y(0.0).length();
        // A short hop at a run lands further on (where the ground goes on), not slower.
        let reach = speed.max(4.0) * 0.42;
        if d < reach
            && !at_hold
            && let Some(g) = level.ground(lift + aim * reach + Vec3::Y * (to.y - lift.y), 0.5, 0.5).filter(|g| (g.point.y - to.y).abs() < 0.15)
        {
            to = g.point;
            d = reach;
        }
        // Across at the speed it runs (a low, quick arc), and no slower than a standing leap; only the push up changes.
        let flight = (d / speed.max(4.0)).max(0.42);
        let dy = to.y - lift.y;
        let mut up = (dy + 0.5 * GRAVITY * flight * flight) / flight;
        // Up onto a higher top: pushed up enough to cross its edge, about 0.65 m short of the aim point, with the feet
        // clear of it (lower, the knees meet the edge and step up onto it in a jolt).
        if !at_hold && dy > 0.1 && d > 0.8 {
            let ts = flight * ((d - 0.65) / d).clamp(0.1, 0.95);
            up = up.max((dy + JUMP_EDGE_CLEAR + 0.5 * GRAVITY * ts * ts) / ts);
        }
        // Always a jump up first, even down to a lower top (it lands further on); off an edge it would fall back onto it.
        up = up.max(JUMP_UP_MIN);
        // The real airtime, down to the target's height: the air clip plays over it, and the way across is spread over it
        // (a push raised to clear an edge stays up longer; at the run's speed it overshot a post onto the next one).
        let airtime = ((up + (up * up - 2.0 * GRAVITY * dy).max(0.0).sqrt()) / GRAVITY).max(flight);
        let v = (to - lift).with_y(0.0) / airtime + Vec3::Y * up;
        let mut w = WallClimb::new(JUMP, -aim);
        w.queue = vec![Queued { rate: air.anim.duration / airtime, ..Queued::new(air, FALL) }];
        w.fall_with = Some(v);
        w.can_catch = true;
        w.aimed = true;
        w.start(takeoff, JUMP.into(), &planned);
        if let (Some(m), Some(f)) = (&mut w.mv, skip) {
            m.t = f / FPS;
            m.start -= skipped;
        }
        w.ease_in(from, root);
        debug!("climb: running jump aimed at {to:.2} (takeoff from frame {:.0}, {airtime:.2} s in the air, up at {up:.1} m/s)", skip.unwrap_or(0.0));
        Some(w)
    }

    /// A running jump over a thin wall ahead (`passover_target`): AC1's takeoff, its flight onto the edge
    /// (`crate::jump::passover`), the hand on the top (`_tr_passover_hand?`, the other hand from the takeoff foot), over it
    /// (`xx_h_passover_hand?_<030|100>cm` by its depth) and down the far side (`_tr_fall`, then the fall and landing), the
    /// hand steered onto the edge over the flight.
    #[allow(clippy::too_many_arguments)]
    fn jump_passover(
        lib: &mut AnimLib,
        level: &Level,
        root: &Transform,
        rig: &Rig,
        base: &Pose,
        cr: ClimbRig,
        dir: Vec3,
        left: bool,
        from: Option<Pose>,
    ) -> Option<WallClimb> {
        let p = root.translation;
        let (edge, normal, depth) = passover_target(level, p, dir)?;
        let fwd = -normal;
        let hand = if left { "handr" } else { "handl" };
        let cm = if depth < PASSOVER_THIN { "030" } else { "100" };
        // (The root as the hand touches: a little under the top, short of the wall.)
        let touch = edge - Vec3::Y * PASSOVER_ROOT_DOWN + normal * 0.4;
        let way = (touch - p).with_y(0.0);
        let j = crate::jump::passover(touch.y - p.y, way.length(), left);
        // Each flight's own reception onto the edge (`<flight>_tr_passover_[entry_]hand?`), mixed as the flights are.
        let reception: Vec<(String, f32)> = j
            .flight
            .iter()
            .filter_map(|(n, w)| {
                let stem = format!("{n}_tr_passover");
                lib.names.iter().find(|m| m.starts_with(&stem) && m.ends_with(hand)).map(|m| (m.clone(), *w))
            })
            .collect();
        let mut mix = |parts: &[(String, f32)]| {
            let parts: Vec<(&str, f32)> = parts.iter().map(|(n, w)| (n.as_str(), *w)).collect();
            lib.get(&mix_name(&parts))
        };
        let (takeoff, flight, touch_clip) = (mix(&j.takeoff)?, mix(&j.flight)?, mix(&reception)?);
        let (over, down) = (lib.get(&format!("xx_h_passover_{hand}_{cm}cm"))?, lib.get(&format!("xx_h_passover_{hand}_{cm}cm_tr_fall"))?);
        let planned = Transform { rotation: facing(fwd), ..*root };
        let to_touch = [takeoff.clone(), flight.clone(), touch_clip.clone()];
        let end = chain_end(&to_touch, rig, base, cr)?.world(&planned);
        let h = end.hands[usize::from(hand == "handr")];
        // (Across the wall and up only: the hand is to its side of the body, wherever along the top that falls.)
        let along = Vec3::Y.cross(normal).normalize_or_zero();
        let err = edge - h;
        let err = err - along * err.dot(along);
        if err.with_y(0.0).length() > JUMP_AC1_SLACK * way.length().max(1.0) + 0.3 || err.y.abs() > 1.0 {
            debug!("climb: AC1's passover lands the hand {:.2} m off ({err:.2}, hand at {h:.2}, edge {edge:.2}, flight {})", err.length(), flight.name);
            return None;
        }
        // (Over it, the root clear of the far edge before the fall: else it comes down on the top.)
        let over_end = chain_end(&[takeoff.clone(), flight.clone(), touch_clip.clone(), over.clone()], rig, base, cr)?.world(&planned).pos + err;
        let short = (edge + fwd * (depth + PASSOVER_CLEAR) - over_end).dot(fwd).max(0.0);
        let mut w = WallClimb::new(VAULT, normal);
        w.start_chain_carry(
            vec![takeoff, flight, touch_clip, over, down],
            vec![VAULT.into(), VAULT.into(), VAULT.into(), VAULT.into(), FALL.into()],
            &planned,
            err,
            Some(1),
        );
        // (At the run's pace: AC1's clip over it is 0.07 s long.)
        if let Some(q) = w.queue.get_mut(2) {
            let d = q.clip.anim.duration.max(1e-3);
            q.correct = fwd * short;
            q.rate = (d / (short / PASSOVER_SPEED)).min(1.0);
        }
        w.ease_in(from, root);
        debug!("climb: running jump over the wall at {edge:.2} ({cm} cm deep, {hand})");
        Some(w)
    }

    /// A running jump at a lone ledge (`jump_hold`) with the wall under it for the feet and no holds there: AC1's takeoff,
    /// its flight onto a surface and its reception on the wall (`crate::jump::surface`), into the wall hang, the hands
    /// steered onto the hold over the flight. `None` when the hold has holds under it (caught into the climb instead), no
    /// wall for the feet, the clips are missing or land too far off, or the way is not clear.
    #[allow(clippy::too_many_arguments)]
    fn jump_hangwall(
        lib: &mut AnimLib,
        level: &Level,
        root: &Transform,
        rig: &Rig,
        base: &Pose,
        cr: ClimbRig,
        dir: Vec3,
        left: bool,
        from: Option<Pose>,
    ) -> Option<WallClimb> {
        let p = root.translation;
        let speed = dir.with_y(0.0).length();
        let (hold, out) = jump_hold(level, p, dir)?;
        if footholds(level, hold, out) {
            return None;
        }
        // (Wall under it for the feet.)
        let under = hold - Vec3::Y * 1.2 + out * 0.4;
        level.raycast(under, -out, 0.8).filter(|h| h.normal.dot(out) > 0.7)?;
        let hang = hold + out * 0.4 - Vec3::Y * JUMP_HOLD_HANG;
        let way = (hang - p).with_y(0.0);
        // (Nothing in the way at the chest before the wall.)
        if level.raycast(p + Vec3::Y * 1.2, way.normalize_or_zero(), (way.length() - 0.8).max(0.0)).is_some() {
            return None;
        }
        let j = crate::jump::surface(hang.y - p.y, way.length(), left, speed > JUMP_RECEPTION_HARD_SPEED);
        let mut mix = |parts: &[(String, f32)]| {
            let parts: Vec<(&str, f32)> = parts.iter().map(|(n, w)| (n.as_str(), *w)).collect();
            lib.get(&mix_name(&parts))
        };
        let (takeoff, flight) = (mix(&j.takeoff)?, mix(&j.flight)?);
        let reception = j.reception.iter().map(|(n, _)| lib.get(n)).collect::<Option<Vec<_>>>()?;
        // (Square to the wall: the reception is the straight one.)
        let planned = Transform { rotation: facing(-out), ..*root };
        let clips: Vec<Arc<Clip>> = [takeoff, flight].into_iter().chain(reception).collect();
        let end = chain_end(&clips, rig, base, cr)?.world(&planned);
        let targets = holds_within(level, &end.hands, out, 3.0)?;
        let err = (targets[0] - end.hands[0] + targets[1] - end.hands[1]) * 0.5;
        if err.with_y(0.0).length() > JUMP_AC1_SLACK * way.length().max(1.0) + 0.5 {
            debug!("climb: AC1's jump to the wall lands {:.2} m off", err.length());
            return None;
        }
        let n = clips.len();
        let tos = (0..n)
            .map(|i| {
                if i < 2 {
                    VAULT
                } else if i + 1 == n {
                    HANGWALL_OPEN
                } else {
                    DROP
                }
                .to_string()
            })
            .collect();
        let mut w = WallClimb::new(VAULT, out);
        w.start_chain_carry(clips, tos, &planned, err + hang_offset(HANGWALL_OPEN, out), Some(1));
        w.ease_in(from, root);
        debug!("climb: running jump at the wall's ledge {hold:.2}, onto the wall hang ({})", j.reception[0].0);
        Some(w)
    }

    /// A running jump onto a top at `to` with AC1's jump clips (`crate::jump`): the takeoff and flight blended by how far
    /// and how high it is, then the reception onto the top, the root following their motion with the difference to the
    /// target spread over the flight (as AC1 moves it). `None` when the clips are missing or the way is not clear.
    #[allow(clippy::too_many_arguments)]
    fn jump_ac1(lib: &mut AnimLib, level: &Level, root: &Transform, to: Vec3, aim: Vec3, speed: f32, left: bool, from: Option<Pose>) -> Option<WallClimb> {
        let p = root.translation;
        let j = crate::jump::running(to.y - p.y, (to - p).with_y(0.0).length(), left, speed / JUMP_AC1_FAST_SPEED);
        let mut mix = |parts: &[(String, f32)]| {
            let parts: Vec<(&str, f32)> = parts.iter().map(|(n, w)| (n.as_str(), *w)).collect();
            lib.get(&mix_name(&parts))
        };
        let (takeoff, flight, reception) = (mix(&j.takeoff)?, mix(&j.flight)?, mix(&j.reception)?);
        let planned = Transform { rotation: facing(aim), ..*root };
        let rot = world_rot(planned.rotation);
        let landed = p + rot * (root_motion_at(&takeoff, takeoff.frames()) + root_motion_at(&flight, flight.frames()));
        let correct = to - landed;
        // (The clips' own way should be most of it: a correction this big would slide through the air.)
        if correct.with_y(0.0).length() > JUMP_AC1_SLACK * (to - p).with_y(0.0).length().max(1.0) {
            debug!("climb: AC1's jump clips land {:.2} m off; the planned arc instead", correct.length());
            return None;
        }
        // The way over must be clear above the higher of the two (the clips do not climb over what is in between; from
        // the chest, up onto a higher top met its wall).
        let over = p.with_y(p.y.max(to.y) + JUMP_AC1_CLEAR);
        if level.raycast(over, (to - p).with_y(0.0).normalize_or_zero(), (to - p).with_y(0.0).length()).is_some() {
            debug!("climb: AC1's jump clips: the way over is not clear");
            return None;
        }
        let mut w = WallClimb::new(VAULT, -aim);
        w.start_chain_carry(vec![takeoff.clone(), flight.clone(), reception], vec![VAULT.into(), VAULT.into(), GROUND.into()], &planned, correct, Some(1));
        // On at the run's speed once down (the reception's `_tr_freestep_entry` leads into the run in AC1's graph): the
        // reception ends once its step is taken, not standing out the rest of it (played faster instead, its front-loaded
        // step lurched the root to 16 m/s).
        w.exit_velocity = aim * speed;
        if speed > RECEPTION_CUT_SPEED
            && let Some(r) = w.queue.last()
        {
            // (Where the step, past its fastest, slows to the run's speed: handed over there, the run goes on at its pace.)
            let pace = |f: f32| (root_motion_at(&r.clip, f) - root_motion_at(&r.clip, f - 1.0)).with_z(0.0).length() * FPS;
            let peak = (1..=r.clip.frames() as usize).map(|f| f as f32).max_by(|a, b| pace(*a).total_cmp(&pace(*b))).unwrap_or(1.0);
            let done = (peak as usize..=r.clip.frames() as usize).map(|f| f as f32).find(|&f| pace(f) <= speed);
            if let Some(f) = done {
                w.cut = Some((r.clip.name.clone(), f / FPS));
            }
        }
        w.ease_in(from, root);
        debug!("climb: running jump aimed at {to:.2} (takeoff and flight from AC1's jump tables: {} then {})", takeoff.name, flight.name);
        Some(w)
    }

    /// A jump from a free step (a post, a beam) onto `to` with AC1's free-step clips (`crate::jump::freestep`): the takeoff
    /// from the group the jump's way falls in off the facing (it turns the body toward it), then the flight and the
    /// reception, the difference to the target spread over the flight. `None` when the clips are missing, their own way
    /// is too far off, or the way over is not clear.
    fn jump_freestep(lib: &mut AnimLib, level: &Level, root: &Transform, to: Vec3, left: bool, from: Option<Pose>) -> Option<WallClimb> {
        let p = root.translation;
        let fwd = (root.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
        let way = (to - p).with_y(0.0);
        let aim = way.normalize_or_zero();
        let angle = aim.dot(fwd.cross(Vec3::Y)).atan2(aim.dot(fwd));
        let j = crate::jump::freestep(to.y - p.y, way.length(), angle, left, 0.0);
        let mut mix = |parts: &[(String, f32)]| {
            let parts: Vec<(&str, f32)> = parts.iter().map(|(n, w)| (n.as_str(), *w)).collect();
            lib.get(&mix_name(&parts))
        };
        let (takeoff, flight, reception) = (mix(&j.takeoff)?, mix(&j.flight)?, mix(&j.reception)?);
        let planned = Transform { rotation: root.rotation, ..*root };
        let rot = world_rot(planned.rotation);
        // (The flight goes on from the takeoff's end, turned with it.)
        let turned = world_rot(planned.rotation * root_delta(root_rotation_at(&takeoff, takeoff.frames())));
        let landed = p + rot * root_motion_at(&takeoff, takeoff.frames()) + turned * root_motion_at(&flight, flight.frames());
        let correct = to - landed;
        if correct.with_y(0.0).length() > JUMP_AC1_SLACK * way.length().max(1.0) {
            debug!("climb: AC1's free-step clips land {:.2} m off", correct.length());
            return None;
        }
        if level.raycast(p.with_y(p.y.max(to.y) + JUMP_AC1_CLEAR), aim, way.length()).is_some() {
            return None;
        }
        let mut w = WallClimb::new(VAULT, -aim);
        w.start_chain_carry(vec![takeoff.clone(), flight.clone(), reception], vec![VAULT.into(), VAULT.into(), GROUND.into()], &planned, correct, Some(1));
        // (Down on a top, on at a run the way it jumped: the reception leads into the run in AC1's graph. Onto another post
        // or beam it balances there first.)
        if level.perch_inside(to, PERCH_REACH).is_none() {
            w.exit_velocity = aim * PERCH_OFF_RUN;
        }
        w.ease_in(from, root);
        debug!("climb: free-step jump aimed at {to:.2}, {:.0} degrees off the facing ({} then {})", angle.to_degrees(), takeoff.name, flight.name);
        Some(w)
    }

    /// Standing jump straight up. `from` is the pose shown now, to fade from.
    pub fn jump_straight(lib: &mut AnimLib, root: &Transform, from: Option<Pose>) -> Option<WallClimb> {
        let [crouch, takeoff, rise] = STRAIGHT_JUMP.map(|n| lib.get(n));
        let (crouch, takeoff, rise) = (crouch?, takeoff?, rise?);
        let mut w = WallClimb::new(JUMP, root.rotation * Vec3::Z);
        w.queue = vec![Queued::new(takeoff, JUMP), Queued::new(rise, FALL)];
        w.launch = STRAIGHT_JUMP_UP_SPEED;
        w.can_catch = true;
        w.descend = lib.get(STRAIGHT_JUMP_FALL);
        w.start(crouch, JUMP.into(), root);
        w.ease_in(from, root);
        Some(w)
    }

    /// Standing jump up to a ledge in front: onto a walkable top at 1.5-2.5 m, or to hang from a hold
    /// at 2-3 m (feet on the wall or free). `from` is the pose shown now, to fade from.
    #[allow(clippy::too_many_arguments)]
    pub fn jump_grab(lib: &mut AnimLib, level: &Level, root: &Transform, rig: &Rig, base: &Pose, cr: ClimbRig, from: Option<Pose>) -> Option<WallClimb> {
        let fwd = (root.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
        let mut w = Self::grab(lib, level, root, rig, base, cr, fwd, &jump_options(), 1.2, None)?;
        w.ease_in(from, root);
        Some(w)
    }

    /// Wall run: sprinting along `dir` at a wall, run up it and grab the highest ledge in reach (see
    /// the module docs); with nothing in reach, run up and fall back. `from` is the pose shown now.
    #[allow(clippy::too_many_arguments)]
    pub fn wall_run(
        lib: &mut AnimLib,
        level: &Level,
        root: &Transform,
        rig: &Rig,
        base: &Pose,
        cr: ClimbRig,
        dir: Vec3,
        from: Option<Pose>,
    ) -> Option<WallClimb> {
        let run = dir.with_y(0.0).normalize_or_zero();
        let hit = level.raycast(root.translation + Vec3::Y, run, WALL_RUN_REACH).filter(|h| h.normal.y.abs() < 0.3)?;
        let n = hit.normal.with_y(0.0).normalize_or_zero();
        // Met at an angle (up to `WALL_RUN_ANGLE` off square), AC1 turns square to the wall and runs up it: the checks
        // and the grab go straight at it from here, the root turning to face it.
        if run.dot(-n) < WALL_RUN_ANGLE.cos() {
            return None;
        }
        let fwd = -n;
        let facing = Transform { rotation: Quat::from_rotation_arc(Vec3::NEG_Z, fwd), ..*root };
        let root = &facing;
        let hit = level.raycast(root.translation + Vec3::Y, fwd, WALL_RUN_REACH).filter(|h| h.normal.y.abs() < 0.3)?;
        // Not up a pole or post under a cap or ledge sticking out over it (nothing for the feet).
        let set_back = level.ledges.iter().filter(|l| l.out.dot(n) > 0.7 && l.a.y > hit.point.y && l.a.y < hit.point.y + 4.0).any(|l| {
            let q = l.closest(hit.point);
            (q - hit.point).with_y(0.0).length() < 0.6 && level.raycast(q + n * 0.3 - Vec3::Y * 0.9, -n, 0.55).is_none()
        });
        if set_back || !wide_wall(level, root.translation + Vec3::Y, fwd, hit.dist) {
            return None;
        }
        let mut w = match Self::grab(lib, level, root, rig, base, cr, fwd, &wall_run_options(), WALL_RUN_REACH, Some(0)) {
            Some(w) => w,
            None => {
                // Nothing to grab: run up, push off and fall (catching a hold if one comes by); not under
                // an awning or balcony (the run up would go into it).
                let normal = hit.normal.with_y(0.0).normalize();
                let column = hit.point + normal * WALL_RUN_HEADROOM_OUT;
                if level.raycast(column.with_y(root.translation.y + 1.0), Vec3::Y, WALL_RUN_HEADROOM).is_some() {
                    debug!("climb: no wall run, something overhead");
                    return None;
                }
                let clips = WALL_RUN.iter().chain([&WALL_RUN_FALL]).map(|n| lib.get(n)).collect::<Option<Vec<_>>>()?;
                let planned = Transform { rotation: Quat::from_rotation_arc(Vec3::NEG_Z, -normal), ..*root };
                let mut w = WallClimb::new(WALL_RUN_STATE, normal);
                let mut items = clips.into_iter();
                let first = items.next()?;
                w.queue = items.map(|c| Queued::new(c, WALL_RUN_STATE)).collect();
                if let Some(last) = w.queue.last_mut() {
                    last.to = FALL.into();
                }
                w.can_catch = true;
                w.start(first, WALL_RUN_STATE.into(), &planned);
                if let Some(m) = &mut w.mv {
                    m.correct = -normal * (hit.dist - WALL_RUN_FOOT);
                }
                w
            }
        };
        w.ease_in(from, root);
        Some(w)
    }

    /// Pick the first option whose end fits what is in front along `fwd`: a walkable top at its
    /// height, or holds under its hang loop's hands with the feet on the wall or free. The chain's root
    /// motion is corrected to land exactly (on clip `carry`, else the one that moves the most). The
    /// root is not moved; `ease_in` blends from it.
    #[allow(clippy::too_many_arguments)]
    fn grab(
        lib: &mut AnimLib,
        level: &Level,
        root: &Transform,
        rig: &Rig,
        base: &Pose,
        cr: ClimbRig,
        fwd: Vec3,
        opts: &[GrabOpt],
        reach: f32,
        carry: Option<usize>,
    ) -> Option<WallClimb> {
        let hit = level.raycast(root.translation + Vec3::Y, fwd, reach.max(1.6)).filter(|h| h.normal.y.abs() < 0.3);
        let normal = hit.map_or(-fwd, |h| h.normal.with_y(0.0).normalize());
        if fwd.dot(-normal) < 0.5 {
            return None;
        }
        let facing = Transform { translation: root.translation, rotation: Quat::from_rotation_arc(Vec3::NEG_Z, -normal), ..default() };
        let fwd = -normal;
        // Walkable top just behind the wall face, with open space above it (the probe may start inside
        // a taller wall and find a seam).
        let top = hit.filter(|h| h.dist < reach).and_then(|hit| {
            let inside = (hit.point - normal * 0.3).with_y(root.translation.y + 4.0);
            let open = |top: &crate::level::Hit| level.raycast(root.translation.with_y(top.point.y + 0.3), fwd, hit.dist + 0.6).is_none();
            level.ground(inside, 0.0, 4.0).filter(|t| t.normal.y > 0.8 && open(t) && !level.inside_solid(t.point + Vec3::Y * 0.5)).map(|t| {
                // Its edge: a cap or sill sticking out over the wall is nearer than the wall.
                let y = t.point.y;
                let edge = (1..=80)
                    .map(|k| k as f32 * 0.02)
                    .take_while(|&d| d < hit.dist)
                    .find(|&d| level.ground((root.translation + fwd * d).with_y(y + 0.3), 0.0, 0.45).is_some_and(|g| (g.point.y - y).abs() < 0.1));
                (edge.unwrap_or(hit.dist), y - root.translation.y)
            })
        });
        // A wall run steps up the wall, so it needs one flush under the edge or the holds.
        let flush_under = |p: Vec3| level.raycast(p + normal * 0.3 - Vec3::Y * 0.9, -normal, 0.55).is_some();
        let slack = if carry.is_some() { WALL_RUN_SLACK } else { JUMP_HANG_REACH };
        let blended = top.map_or(vec![], |(_, h)| blend_onto(opts, h));
        for opt in blended.iter().chain(opts) {
            let Some(clips) = opt.names.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>() else { continue };
            let Some(end) = chain_end(&clips, rig, base, cr) else { continue };
            let mut after = opt.after.clone();
            let mut onto_perch = None;
            // Correction for the last clip (onto the top): the hang part stays in front of the edge.
            let mut end_fix = Vec3::ZERO;
            let (err, tos) = match opt.end {
                GrabEnd::Onto(height) => {
                    let Some((dist, top_h)) = top.filter(|(_, h)| (h - height).abs() < 0.26) else { continue };
                    if carry.is_some() && !flush_under(root.translation + fwd * dist + Vec3::Y * top_h) {
                        continue;
                    }
                    let wall_dist = (end.motion.x - ONTO_PAST_EDGE).max(ONTO_MIN_WALL_DIST);
                    if (dist - wall_dist).abs() > slack.max(0.75) {
                        continue;
                    }
                    // Onto a post: up on one hand to the knee, then balancing on it.
                    let edge = root.translation + fwd * (dist + ONEHAND_POST_IN) + Vec3::Y * top_h;
                    let perch = level.perch_at(edge, ONEHAND_POST_IN + 0.05);
                    // A standing jump at a capped pole hangs from its cap (hands together, feet on the
                    // pole); climbing up from there pulls up onto it.
                    if perch.is_some() && carry.is_none() && on_narrow_ledge(level, edge - fwd * ONEHAND_POST_IN) {
                        continue;
                    }
                    if perch.is_some() {
                        after = if height > 2.2 { strings(&TOP_OUT_FREE_ONEHAND[2..]) } else { strings(&TOP_OUT_ONEHAND[2..]) };
                    }
                    let n = clips.len() + after.len();
                    let last = if perch.is_some() { PERCH } else { ON_TOP };
                    let tos: Vec<String> = (0..n).map(|i| if i + 1 == n { last } else { KNEEL }.to_string()).collect();
                    // The jump and the hang are steered so the hands meet the edge with the body kept
                    // `ONTO_BACK` in front of it (the clips put the hips into a wall flush with it); the step
                    // up onto the top takes the rest (onto a post: its middle).
                    let err = fwd * (dist - wall_dist - ONTO_BACK) + Vec3::Y * (top_h - height);
                    end_fix = match perch {
                        Some((i, q)) => {
                            onto_perch = Some(i);
                            let all: Vec<Arc<Clip>> = clips.iter().cloned().chain(after.iter().filter_map(|n| lib.get(n))).collect();
                            let travel = world_rot(facing.rotation) * all.iter().map(|c| root_motion_at(c, c.frames())).sum::<Vec3>();
                            q - (facing.translation + travel + err)
                        }
                        None => fwd * ONTO_BACK,
                    };
                    (err, tos)
                }
                GrabEnd::Hang(to, rest) => {
                    let Some(rest) = lib.get(rest) else { continue };
                    let end = end.world(&facing);
                    let mut pose = base.clone();
                    sample(&rest, 0.0, &mut pose, cr.reference);
                    let m = pose.model(rig);
                    let r = world_rot(end.rot);
                    let hands = cr.hands.map(|b| end.pos + r * m[b].pos);
                    let Some(targets) = holds_within(level, &hands, normal, slack + 0.1) else { continue };
                    if carry.is_some() && !flush_under((targets[0] + targets[1]) * 0.5) {
                        continue;
                    }
                    let err = (targets[0] - hands[0] + targets[1] - hands[1]) * 0.5;
                    let feet = if to == FREE { Feet::Free } else { Feet::Wall };
                    // (A pole's cap is hung from with the hands together.)
                    let to = if to == HANGWALL_OPEN && on_narrow_ledge(level, (targets[0] + targets[1]) * 0.5) { HANGWALL } else { to };
                    // (A hold up to 0.6 m lower than the clip's is reached by climbing less; real buildings
                    // fall between the heights the clips were made for.)
                    let hold = Some((targets[0] + targets[1]) * 0.5);
                    if !(-0.6..=0.3).contains(&err.y) || !feet_fit_at(level, &end.feet.map(|f| f + err), normal, feet, hold) {
                        continue;
                    }
                    let lead = if carry.is_some() { WALL_RUN_STATE } else { JUMP };
                    let n = clips.len();
                    let tos = (0..n)
                        .map(|i| {
                            if i + opt.catch >= n {
                                to
                            } else if i == 0 {
                                lead
                            } else {
                                LEAP
                            }
                            .to_string()
                        })
                        .collect();
                    (err + hang_offset(to, normal), tos)
                }
            };
            let mut clips = clips;
            if !after.is_empty() {
                let Some(after) = after.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>() else { continue };
                clips.extend(after);
            }
            let mut w = WallClimb::new(JUMP, normal);
            if let Some(i) = onto_perch {
                w.perch = Some(i);
                debug!("climb: one-hand pull-up onto a post");
            }
            w.start_chain_carry(clips, tos, &facing, err, carry);
            if end_fix != Vec3::ZERO {
                let over = w.queue.iter().position(|q| late_correction(&q.clip.name)).unwrap_or(w.queue.len().saturating_sub(1));
                match w.queue.get_mut(over) {
                    Some(last) => last.correct += end_fix,
                    None => {
                        if let Some(m) = &mut w.mv {
                            m.correct += end_fix;
                        }
                    }
                }
            }
            return Some(w);
        }
        None
    }

    /// Clips played back to back on the ground from where the character stands (a run stop, a turn on the
    /// spot, a ledge stop), then back to walking. `correct` moves the root over the chain (world).
    pub fn ground_action(lib: &mut AnimLib, root: &Transform, names: &[String], correct: Vec3, cancel: bool, from: Option<Pose>) -> Option<WallClimb> {
        let clips = names.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>()?;
        let mut w = WallClimb::new(GROUND_ACT, root.rotation * Vec3::Z);
        let n = clips.len();
        let tos = (0..n).map(|i| if i + 1 == n { GROUND } else { GROUND_ACT }.to_string()).collect();
        w.start_chain(clips, tos, root, correct);
        w.fade = from.map(|p| (p, ENTER_FADE, ENTER_FADE));
        w.cancel = cancel;
        Some(w)
    }

    /// Eject backwards off the wall (`xx_h_rebound_<frontleft|frontright>_front_300cm_footl_to_air`): turned to
    /// face away from it, pushing off into the air, catching what comes (a beam or ledge behind).
    fn back_eject(&mut self, lib: &mut AnimLib, root: &Transform, side: f32) -> bool {
        let way = if side > 0.3 { "frontright" } else { "frontleft" };
        let (Some(push), Some(air)) = (lib.get(&format!("xx_h_rebound_{way}_front_300cm_footl_to_air")), lib.get(JUMP_AIR)) else { return false };
        let away = self.normal.with_y(0.0).normalize_or_zero();
        let turned = Transform { rotation: facing(away), ..*root };
        let across = away.cross(Vec3::Y) * side.clamp(-1.0, 1.0);
        self.queue = vec![Queued { rate: air.anim.duration / 0.7, ..Queued::new(air, FALL) }];
        self.fall_with = Some(away * EJECT_SPEED + across * 1.5 + Vec3::Y * EJECT_UP);
        self.can_catch = true;
        self.start(push, LEAP.into(), &turned);
        if let Some(m) = &mut self.mv {
            m.ease_rot = root.rotation * turned.rotation.inverse();
        }
        debug!("climb: back eject ({way})");
        true
    }

    /// Low profile at a bench: turn and sit on it (`xx_rest_sit_<left|right>_<000|090|180>_to_sitting` by how
    /// far round, `xx_rest_sit_sitting_tr_sit_idle_040cm_01`), steered onto the seat, and hide there.
    pub fn sit(lib: &mut AnimLib, level: &Level, root: &Transform, from: Option<Pose>) -> Option<WallClimb> {
        let p = root.translation;
        let (seat, facing) = level
            .benches
            .iter()
            .filter_map(|&(a, b, f)| {
                let q = crate::level::Line { a, b }.closest(p);
                ((q - p).with_y(0.0).length() < BENCH_REACH).then_some((q, f))
            })
            .next()?;
        let fwd = (root.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
        // How far round to the way a sitter faces, and which way.
        let turn = fwd.angle_between(facing).to_degrees();
        let side = if fwd.cross(facing).y >= 0.0 { "left" } else { "right" };
        let how = if turn < 45.0 {
            "000"
        } else if turn < 135.0 {
            "090"
        } else {
            "180"
        };
        let names = vec![format!("xx_rest_sit_{side}_{how}_to_sitting"), "xx_rest_sit_sitting_tr_sit_idle_040cm_01".to_string()];
        let clips = names.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>()?;
        let travel = Self::travel(lib, root, &names)?;
        let mut w = WallClimb::new(BENCH, -facing);
        let n = clips.len();
        let tos = (0..n).map(|i| if i + 1 == n { BENCH } else { BENCH_SIT }.to_string()).collect();
        w.start_chain(clips, tos, root, (seat + facing * BENCH_SEAT_OUT - (p + travel)).with_y(0.0));
        w.fade = from.map(|p| (p, ENTER_FADE, ENTER_FADE));
        debug!("climb: sit on the bench ({side} {how})");
        Some(w)
    }

    /// Sitting on a bench: steering or the legs get up.
    fn bench_step(&mut self, lib: &mut AnimLib, root: &Transform) {
        if self.move_dir.length() < 0.3 && !std::mem::take(&mut self.get_up) {
            return;
        }
        if let Some(c) = lib.get("xx_rest_sit_idle_040cm_01_tr_l_wait_footl") {
            self.start_chain(vec![c], vec![GROUND.into()], root, Vec3::ZERO);
        }
    }

    /// Next to a haystack, the legs: jump into it and hide (`xx_h_air_to_haystack`, steered into its middle); in
    /// low profile only (`high`: free running goes past it), except the dive in from a top beside it.
    pub fn into_hay(lib: &mut AnimLib, level: &Level, root: &Transform, high: bool, from: Option<Pose>) -> Option<WallClimb> {
        let p = root.translation;
        // From a top beside it, level with its top or a little above (AC1's `FreeStep` entry): a dive in,
        // `xx_h_freestep_footr_to_haystack_01` or `_02` (taken in turn), steered into its middle.
        if let Some(hay) = level.haystacks.iter().find(|h| {
            let up = p.y - h.top();
            (h.centre - p).with_y(0.0).length() < h.half + HAY_DIVE_REACH && (-0.3..FAITH_MIN_DROP).contains(&up)
        }) {
            let dir = (hay.centre - p).with_y(0.0).normalize_or(root.rotation * Vec3::NEG_Z);
            let at = Transform { rotation: facing(dir), ..*root };
            let n = HAY_DIVES.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 2 + 1;
            let dive = lib.get(&format!("xx_h_freestep_footr_to_haystack_0{n}"))?;
            let travel = world_rot(at.rotation) * root_motion_at(&dive, dive.frames());
            let mut w = WallClimb::new(HAY, -dir);
            w.start_chain(vec![dive], vec![HAY.into()], &at, hay.centre - (p + travel));
            w.ease_in(from, root);
            debug!("climb: dive into the haystack at {:.2} from a top {:.2} m above it", hay.centre, p.y - hay.top());
            return Some(w);
        }
        if high {
            return None;
        }
        let hay = level.haystacks.iter().find(|h| (h.centre - p).with_y(0.0).length() < h.half + HAY_REACH && (p.y - h.centre.y).abs() < 0.5)?;
        let dir = (hay.centre - p).with_y(0.0).normalize_or(root.rotation * Vec3::NEG_Z);
        let at = Transform { rotation: facing(dir), ..*root };
        let clips = vec![lib.get(STRAIGHT_JUMP[0])?, lib.get(AIR_TO_HAY)?];
        let travel: Vec3 = clips.iter().map(|c| world_rot(at.rotation) * root_motion_at(c, c.frames())).sum();
        let mut w = WallClimb::new(HAY, -dir);
        w.start_chain(clips, vec![JUMP.into(), HAY.into()], &at, hay.centre - (p + travel));
        w.ease_in(from, root);
        debug!("climb: into the haystack at {:.2}", hay.centre);
        Some(w)
    }

    /// The clip playing now (a move), or the hang loop.
    pub fn clip_name(&self) -> Option<&str> {
        self.mv.as_ref().map(|m| m.clip.name.as_str()).or(self.wait.as_ref().map(|w| w.name.as_str()))
    }

    /// Velocity in the air (m/s), if falling.
    pub fn air_velocity(&self) -> Option<Vec3> {
        self.fall_v
    }

    /// This move, broken off when the stick leaves `dir` (see `steer`); letting go of the stick before its last
    /// clip plays swaps that clip for `stop` (a turn's exit into its stand).
    /// The root's facing on the move's first frame, as `update` sets it, for a move that does not ease into its facing.
    pub fn start_facing(&self) -> Option<Quat> {
        let m = self.mv.as_ref().filter(|m| m.ease_rot == Quat::IDENTITY)?;
        Some(m.start_rot * root_delta(root_rotation_at(&m.clip, m.t * FPS)))
    }

    pub fn steered(mut self, dir: Vec3, stop: Option<String>) -> Self {
        self.steer = Some(dir.with_y(0.0).normalize_or_zero());
        self.steer_stop = stop;
        self
    }

    /// The stick let go during a steered move: its last clip becomes the stop one, and it plays out.
    pub fn release_steer(&mut self, lib: &mut AnimLib) {
        let Some(stop) = self.steer_stop.take().and_then(|n| lib.get(&n)) else { return };
        if let Some(last) = self.queue.last_mut() {
            debug!("climb: let go mid-turn: ends in {}", stop.name);
            last.clip = stop;
            self.steer = None;
        } else if let Some(m) = &mut self.mv {
            // (A turn alone: its stand follows it.)
            debug!("climb: let go mid-turn: ends in {}", stop.name);
            m.to = GROUND_ACT.into();
            self.queue.push(Queued::new(stop, GROUND));
            self.steer = None;
        }
    }

    /// A ground action or leaning on a wall (the legs, or steering, may break it off).
    pub fn on_ground(&self) -> bool {
        self.state == GROUND_ACT || self.state == LEAN || self.state == COLLIDE
    }

    /// Does a press of the legs act as on the ground (a stop at an edge, leaning on a wall), not as a wall move? (Stopped
    /// against a low block, the legs step up onto it: that is the climber's.)
    pub fn legs_on_ground(&self) -> bool {
        self.state == GROUND_ACT || self.state == LEAN
    }

    /// Root motion of clips played back to back from `root` (world).
    pub fn travel(lib: &mut AnimLib, root: &Transform, names: &[String]) -> Option<Vec3> {
        let clips = names.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>()?;
        let (mut motion, mut turn) = (Vec3::ZERO, Quat::IDENTITY);
        for c in &clips {
            let f = c.frames();
            motion += turn * root_motion_at(c, f);
            turn *= root_rotation_at(c, f);
        }
        Some(world_rot(root.rotation) * motion)
    }

    /// Running (not free running) at a low obstacle (top 0.35-0.85 m up, AC1's `HumanGround` collide item): head on,
    /// the body stops against it with a foot up on it (`xx_h_collide_full_footl_<050|070>cm_a`, `_b`, `_tr_..._wait`, the
    /// 50 and 70 cm clips mixed by its height) and waits there (`_wait`, see `collide_step`); at an angle it glances off
    /// and runs on along it (`_<left|right>_run`, `_tr_h_run_hipm_<foot>`). `rig`, `base`, `cr`: to place the foot.
    #[allow(clippy::too_many_arguments)]
    pub fn collide(
        lib: &mut AnimLib,
        level: &Level,
        root: &Transform,
        rig: &Rig,
        base: &Pose,
        cr: ClimbRig,
        dir: Vec3,
        from: Option<Pose>,
    ) -> Option<WallClimb> {
        let dir = dir.with_y(0.0).normalize_or_zero();
        let p = root.translation;
        let hit = level.raycast(p + Vec3::Y * 0.25, dir, COLLIDE_REACH).filter(|h| h.normal.y.abs() < 0.3)?;
        let normal = hit.normal.with_y(0.0).normalize();
        let head_on = dir.dot(-normal);
        if head_on < 0.35 {
            return None;
        }
        let top = level.ground(hit.point.with_y(p.y + STEP_UP_MAX + 0.2) - normal * 0.08, 0.0, STEP_UP_MAX + 0.2)?;
        let h = top.point.y - p.y;
        if !(*VAULT_HEIGHT.start()..=STEP_UP_MAX).contains(&h) || top.normal.y < 0.8 || rise(level, p, dir, hit.dist, top.point.y) < *VAULT_HEIGHT.start() {
            return None;
        }
        let mix = ((h - 0.5) / 0.2).clamp(0.0, 1.0);
        if head_on < COLLIDE_HEAD_ON {
            // Glancing off: turned along it, away from the side it was met on.
            let right = (-normal).cross(Vec3::Y);
            let (side, foot) = if dir.dot(right) >= 0.0 { ("right", "footl") } else { ("left", "footr") };
            let names = [
                collide_name(&format!("xx_h_collide_full_footl_{{h}}cm_{side}_run"), mix),
                collide_name(&format!("xx_h_collide_full_footl_{{h}}cm_{side}_run_tr_h_run_hipm_{foot}"), mix),
            ];
            let clips = names.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>()?;
            let at = Transform { translation: p, rotation: facing(-normal), ..*root };
            let mut w = WallClimb::new(GROUND_ACT, normal);
            w.start_chain(clips, vec![GROUND_ACT.into(), GROUND.into()], &at, Vec3::ZERO);
            w.ease_in(from, root);
            w.exit_velocity =
                (at.rotation * Quat::from_rotation_y(if side == "right" { -std::f32::consts::FRAC_PI_2 } else { std::f32::consts::FRAC_PI_2 }) * Vec3::NEG_Z)
                    * 5.0;
            debug!("climb: glance off a {h:.2} m obstacle ({side})");
            return Some(w);
        }
        let names = [
            collide_name("xx_h_collide_full_footl_{h}cm_a", mix),
            collide_name("xx_h_collide_full_footl_{h}cm_b", mix),
            collide_name("xx_h_collide_full_footl_{h}cm_tr_collide_full_footl_{h}cm_wait", mix),
        ];
        let clips = names.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>()?;
        // Stop where the planted (left) foot ends just in from the top's edge.
        let end = chain_end(&clips[..2], rig, base, cr)?;
        let foot_ahead = end.feet[0].x;
        let at = Transform { translation: p, rotation: facing(-normal), ..*root };
        let stand = hit.point.with_y(p.y) + normal * (foot_ahead - COLLIDE_FOOT_IN);
        let mut w = WallClimb::new(COLLIDE, normal);
        w.lean_mix = mix;
        w.collide_top = top.point.y;
        let n = clips.len();
        w.start_chain(clips, (0..n).map(|i| if i + 1 == n { COLLIDE } else { GROUND_ACT }.to_string()).collect(), &at, (stand - p).with_y(0.0));
        w.ease_in(from, root);
        debug!("climb: stopped against a {h:.2} m obstacle");
        Some(w)
    }

    /// Stopped against a low obstacle: pushing into it free running (`self.sprint`) or with the legs (`legs`) steps up
    /// onto it (`_to_freestep_<050|070>cm`, `_tr_freestep_entry_footl`) and runs on; steering away pushes off and runs
    /// back (`_backleft_run` / `_backright_run`, AC1's 70 cm one is `_to_backleft_run`); otherwise it waits.
    fn collide_step(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform, legs: bool) {
        let n = self.normal;
        let d = self.move_dir.with_y(0.0);
        let mix = self.lean_mix;
        let into = d.length() > 0.3 && d.normalize().dot(-n) > 0.7;
        let names: Vec<String> = if into && (legs || self.sprint) {
            vec![
                collide_name("xx_h_collide_full_footl_{h}cm_to_freestep_{h}cm", mix),
                collide_name("xx_h_collide_full_footl_{h}cm_to_freestep_{h}cm_tr_freestep_entry_footl", mix),
            ]
        } else if d.length() > 0.3 && d.normalize().dot(-n) < 0.0 {
            let right = (-n).cross(Vec3::Y);
            let (side, foot) = if d.dot(right) >= 0.0 { ("backright", "footl") } else { ("backleft", "footr") };
            let turn = if side == "backleft" { "to_backleft_run".to_string() } else { "backright_run".to_string() };
            vec![
                mix_name(&[(&format!("xx_h_collide_full_footl_050cm_{side}_run"), 1.0 - mix), (&format!("xx_h_collide_full_footl_070cm_{turn}"), mix)]),
                mix_name(&[
                    (&format!("xx_h_collide_full_footl_050cm_{side}_run_tr_h_run_hipm_{foot}"), 1.0 - mix),
                    (&format!("xx_h_collide_full_footl_070cm_{turn}_tr_h_run_hipm_{foot}"), mix),
                ]),
            ]
        } else {
            return;
        };
        let Some(clips) = names.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>() else { return };
        let up = into;
        let k = clips.len();
        if up {
            // Up onto it along a steady path (as the free-running step-up): rising onto the top by the end of
            // `_to_freestep`, ending just in from the edge (ground moves would follow the floor in front of it).
            let face = level.raycast(root.translation + Vec3::Y * 0.25, -n, 1.5).map_or(0.3, |h| h.dist);
            let exit = (root.translation - n * (face + STEP_ONTO_IN)).with_y(self.collide_top);
            let dur: f32 = clips.iter().map(|c| c.anim.duration).sum();
            let tos = (0..k).map(|i| if i + 1 == k { GROUND } else { VAULT }.to_string()).collect();
            self.start_chain(clips.clone(), tos, root, Vec3::ZERO);
            self.vault_path = Some(VaultPath { from: root.translation, to: exit, plant: clips[0].anim.duration, dur, t: 0.0 });
            self.exit_velocity = -n * 3.5;
            debug!("climb: step up from against the obstacle");
            return;
        }
        let tos = (0..k).map(|i| if i + 1 == k { GROUND } else { GROUND_ACT }.to_string()).collect();
        self.exit_velocity = (root.rotation * Quat::from_rotation_y(std::f32::consts::PI) * Vec3::NEG_Z) * 5.0;
        let correct = Vec3::ZERO;
        self.start_chain(clips, tos, root, correct);
    }

    /// Running into a tall wall (`xx_h_collide_full_hand_150cm`): the hands go onto it and the body leans
    /// there (`xx_h_lean_040cm_twohand_150cm_wait`) while the player keeps pushing into it.
    pub fn lean(lib: &mut AnimLib, level: &Level, root: &Transform, dir: Vec3, from: Option<Pose>) -> Option<WallClimb> {
        let dir = dir.with_y(0.0).normalize_or_zero();
        let p = root.translation;
        let hit = level.raycast(p + Vec3::Y * 1.2, dir, 1.0).filter(|h| h.normal.y.abs() < 0.3)?;
        let normal = hit.normal.with_y(0.0).normalize();
        if dir.dot(-normal) < 0.8 || level.raycast(p + Vec3::Y * LEAN_MIN_HEIGHT, dir, hit.dist + 0.3).is_none() {
            return None;
        }
        // The wall's height: how far up it still stops a probe.
        let top = (0..=16).map(|k| LEAN_MIN_HEIGHT + k as f32 * 0.05).take_while(|&h| level.raycast(p + Vec3::Y * h, dir, hit.dist + 0.3).is_some()).last()?;
        let mix = ((top - LEAN_MIN_HEIGHT) / (LEAN_TALL - LEAN_MIN_HEIGHT)).clamp(0.0, 1.0);
        let names = [
            lean_name("xx_h_collide_full_hand_{h}cm", mix),
            mix_name(&[
                ("xx_h_collide_full_hand_070cm_tr_h_lean_025cm_twohand_70cm_wait", 1.0 - mix),
                ("xx_h_collide_full_hand_150cm_tr_h_lean_025cm_twohand_150cm_wait", mix),
            ]),
        ];
        let clips = names.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>()?;
        let at = Transform { rotation: facing(-normal), ..*root };
        let want = hit.point.with_y(p.y) + normal * LEAN_DIST;
        let travel: Vec3 = clips.iter().map(|c| world_rot(at.rotation) * root_motion_at(c, c.frames())).sum();
        let mut w = WallClimb::new(LEAN, normal);
        w.lean_mix = mix;
        w.start_chain(clips, vec![LEAN.into(), LEAN.into()], &at, (want - (p + travel)).with_y(0.0));
        w.ease_in(from, root);
        debug!("climb: lean on the wall at {:.2}", hit.point);
        Some(w)
    }

    /// Leaning on a wall: keep leaning while pushing into it; steering away pushes off it to that side
    /// (or back) into a jog, letting go stands up.
    fn lean_step(&mut self, lib: &mut AnimLib, root: &Transform) {
        let n = self.normal;
        let d = self.move_dir.with_y(0.0);
        if d.length() > 0.3 && d.normalize().dot(-n) > 0.7 {
            return;
        }
        let mix = self.lean_mix;
        let names: Vec<String> = if d.length() < 0.3 {
            vec![lean_name("xx_h_lean_040cm_twohand_{h}cm_wait_tr_h_wait_hipm_footr", mix)]
        } else {
            let right = (-n).cross(Vec3::Y);
            let side = if d.dot(right) >= 0.0 { "right" } else { "left" };
            let dir = if d.normalize().dot(n) > 0.5 { format!("back{side}") } else { side.to_string() };
            let foot = if side == "left" { "footr" } else { "footl" };
            vec![lean_name(&format!("xx_h_lean_040cm_twohand_{{h}}cm_to_{dir}_jog"), mix), format!("xx_h_lean_040cm_twohand_to_{dir}_jog_tr_h_jog_hipm_{foot}")]
        };
        let Some(clips) = names.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>() else { return };
        let k = clips.len();
        let tos = (0..k).map(|i| if i + 1 == k { GROUND } else { GROUND_ACT }.to_string()).collect();
        self.start_chain(clips, tos, root, Vec3::ZERO);
    }

    /// Low profile at the edge of a drop, the legs pressed: lower onto the edge and hang from it (turning
    /// round over it, `xx_l_ledge_pulldown_soft_front`, then `_to_hangwall_straight_a/b` with the feet on the wall below,
    /// or `_front_to_hangfree_a/b`). Needs a hold along the edge; the chain is steered so the hands end on it.
    #[allow(clippy::too_many_arguments)]
    pub fn pull_down(
        lib: &mut AnimLib,
        level: &Level,
        root: &Transform,
        rig: &Rig,
        base: &Pose,
        cr: ClimbRig,
        looking: bool,
        from: Option<Pose>,
    ) -> Option<WallClimb> {
        let p = root.translation;
        let fwd = (root.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
        let edge_d =
            (1..=18).map(|k| k as f32 * 0.05).take_while(|&d| d <= PULL_DOWN_REACH).find(|&d| level.ground(p + fwd * d, 0.3, PULL_DOWN_DROP).is_none())?;
        let edge = p + fwd * edge_d;
        // (A wall ahead, not an edge: the floor stops there because the wall begins.)
        if level.raycast(p + Vec3::Y * 0.5, fwd, edge_d + 0.3).is_some() {
            return None;
        }
        let ledge = level
            .ledges
            .iter()
            .filter(|l| l.out.dot(fwd) > 0.7 && (l.a.y - p.y).abs() < 0.2)
            .min_by(|a, b| (a.closest(edge) - edge).with_y(0.0).length().total_cmp(&(b.closest(edge) - edge).with_y(0.0).length()))?;
        if (ledge.closest(edge) - edge).with_y(0.0).length() > 0.45 {
            return None;
        }
        // The root faces the wall throughout; the body starts turned round, facing the drop, and comes
        // round as it steps back over the edge (`*_pulldown_front_orientation`, from looking down or from
        // standing), then lowers (`xx_l_ledge_pulldown_soft_front`) onto the hang.
        let turn = if looking { "xx_l_ledge_lookdown_front_pulldown_front_orientation" } else { "xx_l_ledge_stop_start_footl_pulldown_front_orientation" };
        let w = Self::pull_down_onto(lib, level, root, rig, base, cr, ledge, edge, [turn, "xx_l_ledge_pulldown_soft_front"], from)?;
        debug!("climb: pull down onto the ledge ({})", w.queue.last().map_or("", |q| q.to.as_str()));
        Some(w)
    }

    /// Standing on a post, let go: lower over its side onto a hold just under its top, on the side the stick points
    /// (else the one ahead), AC1's `xx_l_beam_pilotis_to_pulldown_soft_<side>_orientation` (turned toward the wall) and
    /// `_<side>` (lowering: front, back, left or right of the way he faces), then the ledge pull-down's hang.
    #[allow(clippy::too_many_arguments)]
    fn post_pull_down(
        lib: &mut AnimLib,
        level: &Level,
        root: &Transform,
        rig: &Rig,
        base: &Pose,
        cr: ClimbRig,
        dir: Vec3,
        from: Option<Pose>,
    ) -> Option<WallClimb> {
        let p = root.translation;
        let fwd = (root.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
        let stick = dir.with_y(0.0).length() > 0.3;
        let want = if stick { dir.with_y(0.0).normalize() } else { fwd };
        // (A hold along the post's top edge, its side facing the way he wants to go down; with the stick let go, any side,
        // ahead first.)
        let near = |l: &&Ledge| (l.closest(p) - p).with_y(0.0).length() < POST_HOLD_REACH && (p.y - l.a.y) > -0.1 && (p.y - l.a.y) < 0.4;
        let ledge = level.ledges.iter().filter(near).max_by(|a, b| a.out.dot(want).total_cmp(&b.out.dot(want)))?;
        if stick && ledge.out.dot(want) < 0.3 {
            return None;
        }
        let d = ledge.out;
        let side = if d.dot(fwd) > 0.7 {
            "front"
        } else if d.dot(fwd) < -0.7 {
            "back"
        } else if d.dot(fwd.cross(Vec3::Y)) > 0.0 {
            "right"
        } else {
            "left"
        };
        let names = [format!("xx_l_beam_pilotis_to_pulldown_soft_{side}_orientation"), format!("xx_l_beam_pilotis_to_pulldown_soft_{side}")];
        let edge = ledge.closest(p).with_y(p.y);
        let w = Self::pull_down_onto(lib, level, root, rig, base, cr, ledge, edge, [&names[0], &names[1]], from)?;
        debug!("climb: down off the post to hang ({side})");
        Some(w)
    }

    /// The pull-down's hang from `ledge` at `edge`: `first` (the turn toward the wall and the lowering), then onto the
    /// hang with the feet on the wall below (`xx_l_ledge_pulldown_soft_to_hangwall_straight_a/b`) or free
    /// (`_front_to_hangfree_a/b`), steered so the hands end on the hold.
    #[allow(clippy::too_many_arguments)]
    fn pull_down_onto(
        lib: &mut AnimLib,
        level: &Level,
        root: &Transform,
        rig: &Rig,
        base: &Pose,
        cr: ClimbRig,
        ledge: &Ledge,
        edge: Vec3,
        first: [&str; 2],
        from: Option<Pose>,
    ) -> Option<WallClimb> {
        let normal = ledge.out;
        let start = Transform { translation: root.translation, rotation: facing(-normal), ..*root };
        // On a short ledge (a pole's cap) the hands hang together.
        let (wall, wall_rest) =
            if on_narrow_ledge(level, ledge.closest(edge)) { (HANGWALL, "xx_h_hangwall_waitclose") } else { (HANGWALL_OPEN, HANGWALL_REST) };
        let options = [
            (["xx_l_ledge_pulldown_soft_to_hangwall_straight_a", "xx_l_ledge_pulldown_soft_to_hangwall_straight_b"], wall, wall_rest, Feet::Wall),
            (["xx_l_ledge_pulldown_soft_front_to_hangfree_a", "xx_l_ledge_pulldown_soft_front_to_hangfree_b"], FREE, "xx_h_hangfree_waitclose", Feet::Free),
        ];
        for (names, to, rest, feet) in options {
            let Some(clips) = first.iter().chain(&names).map(|n| lib.get(n)).collect::<Option<Vec<_>>>() else { continue };
            let (Some(end), Some(rest)) = (chain_end(&clips, rig, base, cr), lib.get(rest)) else { continue };
            let mut pose = base.clone();
            sample(&rest, 0.0, &mut pose, cr.reference);
            let m = pose.model(rig);
            let end = end.world(&start);
            let (end_pos, r) = (end.pos, world_rot(end.rot));
            let hands = cr.hands.map(|b| end_pos + r * m[b].pos);
            let Some(targets) = holds_within(level, &hands, normal, 1.0) else { continue };
            let err = (targets[0] - hands[0] + targets[1] - hands[1]) * 0.5;
            let feet_end = cr.feet.map(|b| end_pos + r * m[b].pos + err);
            if err.length() > 1.0 || !feet_fit_at(level, &feet_end, normal, feet, Some((targets[0] + targets[1]) * 0.5)) {
                continue;
            }
            let mut w = WallClimb::new(to, normal);
            let k = clips.len();
            let tos = (0..k).map(|i| if i + 1 == k { to } else { DROP }.to_string()).collect();
            w.start_chain(clips, tos, &start, err + hang_offset(to, normal));
            w.ease_in(from, root);
            debug!("climb: pulling down ({to}), steering {:.2} m", err.length());
            return Some(w);
        }
        None
    }

    /// Standing on a post or beam (stepped onto it from the ground). `from` is the pose shown now.
    pub fn perch(lib: &mut AnimLib, level: &Level, root: &Transform, from: Option<Pose>) -> Option<WallClimb> {
        let (i, _) = level.perch_at(root.translation, PERCH_REACH)?;
        let wait = lib.get(PERCH_WAIT)?;
        let mut w = WallClimb::new(PERCH, root.rotation * Vec3::Z);
        w.perch = Some(i);
        w.wait = Some(wait);
        w.fade = from.map(|p| (p, ENTER_FADE, ENTER_FADE));
        debug!("climb: onto perch {i} at {:.2}", root.translation);
        Some(w)
    }

    /// On a perch: stay on its top, turn toward the wanted direction, walk along a beam, or (sprinting)
    /// jump on.
    fn perch_step(&mut self, lib: &mut AnimLib, level: &Level, root: &mut Transform, dt: f32) {
        let Some(line) = self.perch.and_then(|i| level.perches.get(i).copied()) else { return };
        let q = line.closest(root.translation);
        root.translation = root.translation.lerp(q, 1.0 - (-10.0 * dt).exp());
        let dir = self.move_dir.with_y(0.0);
        if dir.length() < 0.3 {
            return;
        }
        let dir = dir.normalize();
        let axis = line.axis();
        // On a beam, crouched: turn round on it, or to face across it and back (AC1's beam turns), before walking or
        // jumping that way.
        if axis != Vec3::ZERO && self.beam_turn(lib, root, dir, axis) {
            return;
        }
        // (Along the beam while the stick leans along it, as the walk keeps going: at an angle to it too.)
        if axis != Vec3::ZERO && dir.dot(axis).abs() > BEAM_WALK_COS {
            let along = axis * dir.dot(axis).signum();
            // Walk unless already at that end.
            let end = if along.dot(axis) > 0.0 { line.b } else { line.a };
            // (Not from where the walk stops short of an end, `BEAM_END_STOP`: held on, it would start and stop again.)
            let beyond = level.ground(end + along * 0.5, 0.4, 0.4).is_some_and(|g| g.normal.y > 0.8);
            if (end - root.translation).with_y(0.0).length() > if beyond || self.sprint { 0.15 } else { BEAM_END_STOP + 0.05 } {
                self.start_beam_walk(lib, along);
                return;
            }
        }
        // At a beam's end (or a post) with ground carrying on at this height ahead: step off onto it, as walking
        // off the end does; no jump.
        // (Not the beam's own top: at an angle to it, that read as ground ahead, and he stepped off onto the beam and
        // back onto it as a perch every frame.)
        let ahead = root.translation + dir * 0.5;
        if level.perch_at(ahead, PERCH_OWN).is_none() && level.ground(ahead, 0.4, 0.4).is_some_and(|g| g.normal.y > 0.8) {
            debug!("climb: stepped off the perch onto the ground at {:.2}", root.translation);
            self.exit_velocity = dir * if self.sprint { PERCH_OFF_RUN } else { PERCH_OFF_WALK };
            self.finished = true;
            return;
        }
        if self.sprint && (self.perch_faith(lib, level, root, dir) || self.perch_jump(lib, level, root, dir)) {
            return;
        }
        // (On a post he turns on the spot; on a beam the turns above face him.)
        if axis == Vec3::ZERO || self.beam_stance.is_none() {
            root.rotation = root.rotation.slerp(facing(dir), 1.0 - (-6.0 * dt).exp());
        }
    }

    /// A turn on a beam toward `dir` (stick) from how he stands on it, `axis` the beam's: facing along it the other way,
    /// right round (`xx_l_beam_crouchwait_foot?_turn180`); the stick across it, a quarter turn to face across
    /// (`_turn_<side>_to_crouchwait_90`); facing across, the stick along it, back to facing along (`xx_l_beam_crouchwait_
    /// 90_turn_<side>`), or across the other way, round (`_90_turn180`). Free running across it jumps off instead. True
    /// when a turn started.
    fn beam_turn(&mut self, lib: &mut AnimLib, root: &Transform, dir: Vec3, axis: Vec3) -> bool {
        let fwd = (root.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
        let right = fwd.cross(Vec3::Y);
        let stance = self.beam_stance.unwrap_or(if fwd.dot(axis).abs() > 0.7 { BeamStance::Along(0) } else { BeamStance::Across });
        let side = if dir.dot(right) > 0.0 { "right" } else { "left" };
        let feet = ["footl", "footr"];
        let along = dir.dot(axis).abs() > BEAM_WALK_COS;
        let (names, to): (Vec<String>, BeamStance) = match (stance, along) {
            (BeamStance::Along(f), true) if fwd.dot(dir) < 0.0 => {
                let n = format!("xx_l_beam_crouchwait_{}_turn180", feet[f]);
                (vec![n.clone(), format!("{n}_tr_{}", feet[1 - f])], BeamStance::Along(1 - f))
            }
            (BeamStance::Along(f), false) if !self.sprint => {
                // (AC1 names the right turns with a space.)
                let sep = if side == "right" { " " } else { "_" };
                (vec![format!("xx_l_beam_crouchwait_{}_turn_{side}{sep}to_crouchwait_90", feet[f])], BeamStance::Across)
            }
            (BeamStance::Across, true) => {
                let f = usize::from(side == "right");
                let n = format!("xx_l_beam_crouchwait_90_turn_{side}");
                (vec![n.clone(), format!("{n}_tr_crouchwait_{}", feet[f])], BeamStance::Along(f))
            }
            (BeamStance::Across, false) if fwd.dot(dir) < -0.5 => {
                let n = "xx_l_beam_crouchwait_90_turn180".to_string();
                (vec![n.clone(), format!("{n}_tr_crouchwait_90")], BeamStance::Across)
            }
            _ => return false,
        };
        let Some(clips) = names.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>() else { return false };
        debug!("climb: turning on the beam ({stance:?} to {to:?})");
        self.beam_stance = Some(to);
        let n = clips.len();
        self.start_chain(clips, vec![PERCH.to_string(); n], root, Vec3::ZERO);
        true
    }

    /// Free running off a perch over hay: the leap of faith (a tower's beam), rather than a jump to the next post.
    fn perch_faith(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform, dir: Vec3) -> bool {
        match Self::leap_of_faith(lib, level, root, dir, self.last.clone()) {
            Some(w) => {
                *self = w;
                true
            }
            None => false,
        }
    }

    fn start_beam_walk(&mut self, lib: &mut AnimLib, dir: Vec3) {
        let names = if self.sprint { BEAM_JOG } else { BEAM_WALK };
        let (Some(a), Some(b)) = (lib.get(names[0]), lib.get(names[1])) else { return };
        let speed = root_motion_at(&a, a.frames()).length() / a.anim.duration.max(1e-3);
        self.cycle = Some(Cycle { clips: [a, b], foot: 0, phase: 0.0, dir, speed });
        self.fade = self.last.clone().map(|p| (p, ENTER_FADE, ENTER_FADE));
    }

    /// Walking along a beam: follow its top line; at its end step onto ground that carries on, else
    /// stop. Letting go of the direction stops.
    fn beam_step(&mut self, lib: &mut AnimLib, level: &Level, root: &mut Transform, dt: f32) {
        let Some(line) = self.perch.and_then(|i| level.perches.get(i).copied()) else {
            self.cycle = None;
            return;
        };
        let Some(cy) = &mut self.cycle else { return };
        // (Stopped: crouched on the foot it was on.)
        let stop = |w: &mut Self, lib: &mut AnimLib| {
            let foot = w.cycle.take().map_or(0, |c| c.foot);
            w.beam_stance = Some(BeamStance::Along(foot));
            w.wait = lib.get(BEAM_WAIT[foot]).or(w.wait.take());
            w.wait_hold = false;
            w.wait_t = 0.0;
            w.fade = w.last.clone().map(|p| (p, ENTER_FADE, ENTER_FADE));
        };
        if self.move_dir.with_y(0.0).normalize_or_zero().dot(cy.dir) < BEAM_WALK_COS {
            stop(self, lib);
            return;
        }
        let next = root.translation + cy.dir * cy.speed * dt;
        let q = line.closest(next);
        root.rotation = root.rotation.slerp(facing(cy.dir), 1.0 - (-10.0 * dt).exp());
        // Walking to an end with nothing past it to step onto: AC1 stops `BEAM_END_STOP` short of it
        // (`ConstrainRootMotionToBeam`), crouched there; free running goes on to jump from the end.
        let end = if cy.dir.dot(line.axis()) > 0.0 { line.b } else { line.a };
        let beyond = level.ground(end + cy.dir * 0.5, 0.4, 0.4).is_some_and(|g| g.normal.y > 0.8);
        if !self.sprint && !beyond && (end - q).with_y(0.0).dot(cy.dir) < BEAM_END_STOP {
            debug!("climb: stopped short of the beam's end at {:.2}", root.translation);
            stop(self, lib);
            return;
        }
        // (Any step past the end, along the beam: a fixed margin pinned a slow walk at high frame rates, each step shorter
        // than it; and the root a centimetre off the line is not past an end.)
        if (next - q).with_y(0.0).dot(cy.dir) > 1e-3 {
            // Off the end: onto ground that carries on at this height, else wait there.
            let (dir, speed) = (cy.dir, cy.speed);
            root.translation = q;
            if level.ground(q + dir * 0.5, 0.4, 0.4).is_some_and(|g| g.normal.y > 0.8) {
                debug!("climb: off the beam's end onto the ground at {q:.2}");
                self.cycle = None;
                self.exit_velocity = dir * speed;
                self.finished = true;
            } else if !(self.sprint && (self.perch_faith(lib, level, root, dir) || self.perch_jump(lib, level, root, dir))) {
                stop(self, lib);
            }
            return;
        }
        root.translation = q;
        cy.phase += dt / cy.clips[cy.foot].anim.duration.max(1e-3);
        if cy.phase >= 1.0 {
            cy.phase -= 1.0;
            cy.foot ^= 1;
        }
    }

    /// Jump from a perch along `dir`: onto the nearest perch that way in reach, else onto the first
    /// walkable top at about this height, else just off it.
    fn perch_jump(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform, dir: Vec3) -> bool {
        let from = root.translation;
        let to = jump_target(level, from, dir, self.perch).filter(|q| PERCH_JUMP.contains(&(*q - from).with_y(0.0).length()));
        // To a target: AC1's free-step jump, the takeoff group by the way it goes off the facing (sideways or back off a
        // post without turning first: the takeoff turns the body).
        if let Some(to) = to
            && let Some(w) = Self::jump_freestep(lib, level, root, to, self.freestep_left, self.last.clone())
        {
            *self = w;
            return true;
        }
        // Face where it goes.
        let dir = to.map_or(dir, |q| (q - from).with_y(0.0).normalize_or(dir));
        let (v, flight) = match to {
            Some(to) => ballistic(from, to),
            None => (dir * 3.5 + Vec3::Y * 3.0, jump_airtime()),
        };
        let (Some(takeoff), Some(push), Some(air)) = (lib.get(PERCH_TAKEOFF), lib.get(PERCH_PUSH), lib.get(JUMP_AIR)) else { return false };
        let planned = Transform { rotation: facing(dir), ..*root };
        // The crouch, the push off (AC1's `HumanNarrowObject` starts it from the crouch), then the flight.
        let flight = (flight - push.anim.duration).max(0.1);
        self.queue = vec![Queued::new(push, JUMP), Queued { rate: air.anim.duration / flight, ..Queued::new(air, FALL) }];
        self.fall_with = Some(v);
        self.can_catch = true;
        self.aimed = true;
        self.perch = None;
        self.cycle = None;
        self.start(takeoff, JUMP.into(), &planned);
        if let Some(m) = &mut self.mv {
            m.ease_rot = root.rotation * planned.rotation.inverse();
        }
        true
    }

    /// Falling: steer onto the first perch along the way, from just ahead to a little past where the
    /// fall would come down (once).
    fn aim_at_perch(&mut self, level: &Level, root: &Transform) {
        let Some(v) = &mut self.fall_v else { return };
        let p = root.translation;
        let best = level
            .perches
            .iter()
            .filter_map(|l| {
                let y = l.a.y;
                let disc = v.y * v.y + 2.0 * GRAVITY * (p.y - y);
                if p.y < y || disc < 0.0 {
                    return None;
                }
                let t = (v.y + disc.sqrt()) / GRAVITY;
                let flat = v.with_y(0.0);
                let (dir, reach) = (flat.normalize_or_zero(), flat.length() * t);
                let q = l.closest((p + dir * reach.min(1.0)).with_y(y));
                let q = if (q - p).with_y(0.0).dot(dir) < 0.5 { l.closest((p + dir * reach).with_y(y)) } else { q };
                let d = (q - p).with_y(0.0);
                let (along, side) = (d.dot(dir), (d - dir * d.dot(dir)).length());
                (t > 0.15 && along > 0.5 && along < reach + PERCH_MAGNET && side < 0.8 && arc_clear(level, p, (q - p).with_y(0.0) / t + Vec3::Y * v.y, t))
                    .then_some((q, t, along))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2));
        if let Some((q, t, _)) = best {
            let flat = (q - p).with_y(0.0) / t;
            v.x = flat.x;
            v.z = flat.z;
            self.aimed = true;
            debug!("climb: steering onto the perch at {q:.2}");
        }
    }

    /// A fall that brings the hands to a bar catches it and swings.
    fn try_bar(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform) -> bool {
        let Some(v) = self.fall_v else { return false };
        let p = root.translation;
        let skip = self.skip_bar.map(|(i, _)| i);
        let Some((i, q)) = level
            .bars
            .iter()
            .enumerate()
            .filter(|(i, _)| Some(*i) != skip)
            // (Moving along a bar is not swinging on it: only one lying across the way, as AC1 catches them.)
            .filter(|(_, b)| v.with_y(0.0).length() < 0.5 || b.axis().dot(v.with_y(0.0).normalize()).abs() < BAR_ACROSS)
            .map(|(i, b)| (i, b.closest(p + Vec3::Y * SWING_HANG)))
            .filter(|(_, q)| (*q - p).with_y(0.0).length() < BAR_REACH && BAR_HEIGHT.contains(&(q.y - p.y)))
            .min_by(|a, b| (a.1 - p).length().total_cmp(&(b.1 - p).length()))
        else {
            return false;
        };
        // (AC1's catch from the air into the swing, by how high the flight started, then the swing; else the swing.)
        let dz = if self.fall_top.is_finite() { q.y - SWING_HANG - self.fall_top } else { 0.0 };
        let entry = lib.get(crate::jump::swing_entry(dz));
        let Some(first) = lib.get(SWING_CYCLE[0]) else { return false };
        // Face across the bar, the way we are going.
        let mut out = level.bars[i].axis().cross(Vec3::Y).normalize_or(Vec3::X);
        let going = if v.with_y(0.0).length() > 0.3 { v } else { root.rotation * Vec3::NEG_Z };
        if out.dot(going) < 0.0 {
            out = -out;
        }
        let target = q - Vec3::Y * SWING_HANG;
        let rot = facing(out);
        self.fall_v = None;
        self.queue.clear();
        self.descend = None;
        self.bar = Some(i);
        self.swing_off = None;
        let caught = Transform { rotation: rot, ..*root };
        let moved = entry.as_ref().map_or(Vec3::ZERO, |e| world_rot(rot) * root_motion_at(e, e.frames()));
        match entry {
            Some(e) => {
                self.queue = vec![Queued::new(first, SWING)];
                self.start(e, SWING.into(), &caught);
            }
            None => self.start(first, SWING.into(), &caught),
        }
        if let Some(m) = &mut self.mv {
            m.correct = target - p - moved;
            m.ease_rot = root.rotation * rot.inverse();
        }
        self.fade = self.last.clone().map(|p| (p, ENTER_FADE, ENTER_FADE));
        debug!("climb: caught bar {i} at {q:.2}");
        true
    }

    /// Swinging: the next clip of the cycle, or let go when the feet have swung forward.
    fn swing_next(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform, done: &Clip) -> bool {
        let i = SWING_CYCLE.iter().position(|n| *n == done.name).unwrap_or(SWING_CYCLE.len() - 1);
        if i == 0
            && let Some(fling) = self.swing_off
        {
            let fwd = (root.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or(Vec3::NEG_Z);
            let (name, v) = if fling { (SWING_LAUNCH, fwd * SWING_OFF.0 + Vec3::Y * SWING_OFF.1) } else { (SWING_DROP, fwd) };
            // (Flung at another bar: AC1's flight onto it.)
            let p = root.translation;
            let at_bar = fling.then(|| bar_ahead(level, p, fwd)).flatten().and_then(|q| {
                let j = crate::jump::swing(q.y - SWING_HANG - p.y, (q - p).with_y(0.0).length(), true);
                let parts: Vec<(&str, f32)> = j.flight.iter().map(|(n, w)| (n.as_str(), *w)).collect();
                lib.get(&mix_name(&parts))
            });
            if let (Some(off), Some(air)) = (lib.get(name), at_bar.or_else(|| lib.get(JUMP_AIR))) {
                self.skip_bar = self.bar.take().map(|b| (b, 0.6));
                self.swing_off = None;
                self.queue = vec![Queued { rate: air.anim.duration / jump_airtime(), ..Queued::new(air, FALL) }];
                self.fall_with = Some(v);
                self.fall_top = root.translation.y;
                self.can_catch = true;
                self.aimed = false;
                self.start(off, LEAP.into(), root);
                return true;
            }
        }
        let Some(next) = lib.get(SWING_CYCLE[(i + 1) % SWING_CYCLE.len()]) else { return false };
        self.start(next, SWING.into(), root);
        true
    }

    /// Free running at a low obstacle (top 0.35-1.3 m up): jumped onto, AC1's way: the sprint takeoff (from 1.3 m off
    /// or more), `xx_h_air_front_050cm_footl_to_freestep` (landing on it) and `_tr_freestep_entry_footr`, then on along
    /// its top at the run's speed (the root follows a steady path, `VaultPath`, the clips timed to it). (AC1 has no
    /// vault: its `passover` clips go over a ledge's edge downward. Its step-up, `collide_full_*_to_freestep`, starts
    /// from stopping against the obstacle, see `collide`.)
    pub fn vault(lib: &mut AnimLib, level: &Level, root: &Transform, dir: Vec3, speed: f32, from: Option<Pose>) -> Option<WallClimb> {
        let dir = dir.with_y(0.0).normalize_or_zero();
        let p = root.translation;
        let hit = level.raycast(p + Vec3::Y * 0.25, dir, VAULT_REACH).filter(|h| h.normal.y.abs() < 0.3)?;
        let normal = hit.normal.with_y(0.0).normalize();
        let fwd = -normal;
        if fwd.dot(dir) < 0.7 {
            return None;
        }
        // Its top just past the face, in range, walkable, with room above to stand on it.
        let top = level.ground(hit.point.with_y(p.y + VAULT_HEIGHT.end() + 0.2) + fwd * 0.08, 0.0, VAULT_HEIGHT.end() + 0.2)?;
        let h = top.point.y - p.y;
        if !VAULT_HEIGHT.contains(&h)
            || top.normal.y < 0.8
            || rise(level, p, fwd, hit.dist, top.point.y) < *VAULT_HEIGHT.start()
            || level.raycast(p.with_y(top.point.y + 0.3), fwd, hit.dist + 1.0).is_some()
        {
            return None;
        }
        let edge = hit.point.with_y(top.point.y);
        let dist = (edge - p).with_y(0.0).length();
        // AC1's jump onto a free-step target this high and far (`crate::jump`): its flight and reception, and its takeoff
        // from far enough off. (The sprint stride `JUMP_TAKEOFF` straight into `JUMP_AIR` is not a step in AC1's move graph.)
        let j = crate::jump::running(h, dist + STEP_ONTO_IN * 0.5, true, speed / JUMP_AC1_FAST_SPEED);
        let mut mix = |parts: &[(String, f32)]| {
            let parts: Vec<(&str, f32)> = parts.iter().map(|(n, w)| (n.as_str(), *w)).collect();
            lib.get(&mix_name(&parts))
        };
        let mut clips = vec![mix(&j.flight)?, mix(&j.reception)?];
        if dist >= VAULT_TAKEOFF_MIN {
            clips.insert(0, mix(&j.takeoff)?);
        }
        // Up by the landing.
        let up_by: f32 = clips.iter().take(clips.len() - 1).map(|c| c.anim.duration).sum();
        let rot = facing(fwd);
        let at = Transform { translation: p, rotation: rot, ..*root };
        let mut w = WallClimb::new(VAULT, normal);
        let n = clips.len();
        let tos: Vec<String> = (0..n).map(|i| if i + 1 == n { GROUND } else { VAULT }.to_string()).collect();
        w.start_chain(clips.clone(), tos, &at, Vec3::ZERO);
        // The root's path: onto the top just in from the edge, at the run's speed; on a thin top (a fence, a low
        // wall) no further than its middle, so a jump on from it can follow.
        let depth = (1..=12).map(|k| k as f32 * 0.1).find(|&d| level.ground(edge + fwd * d + Vec3::Y * 0.3, 0.0, 0.4).is_none()).unwrap_or(1.3);
        let exit = edge + fwd * STEP_ONTO_IN.min(depth * 0.5);
        let natural: f32 = clips.iter().map(|c| c.anim.duration).sum();
        let run = speed.max(3.5);
        let rate = (natural / ((exit - p).with_y(0.0).length() / run)).clamp(0.7, 1.8);
        if let Some(m) = &mut w.mv {
            m.rate = rate;
        }
        for q in w.queue.iter_mut() {
            q.rate = rate;
        }
        w.vault_path = Some(VaultPath { from: p, to: exit, plant: (up_by / rate).max(0.1), dur: natural / rate, t: 0.0 });
        // On along the top at the run's speed (off a thin wall, a running drop).
        w.exit_velocity = fwd * speed;
        w.aimed = true;
        w.ease_in(from, root);
        debug!("climb: jump onto {h:.2} m");
        Some(w)
    }

    /// Get on a ladder: from its foot (facing it), or from the top at its edge (climbing down onto it).
    /// `from` is the pose shown now.
    pub fn ladder(lib: &mut AnimLib, level: &Level, root: &Transform, rig: &Rig, base: &Pose, cr: ClimbRig, from: Option<Pose>) -> Option<WallClimb> {
        let p = root.translation;
        let (i, l) = level.ladders.iter().enumerate().find(|(_, l)| {
            let d = p - l.base;
            let side = l.out.cross(Vec3::Y);
            d.dot(side).abs() < 0.5
                && ((d.y.abs() < 0.4 && (0.0..1.4).contains(&d.dot(l.out))) || ((p.y - l.top).abs() < 0.4 && (-1.6..0.2).contains(&d.dot(l.out))))
        })?;
        let rot = facing(-l.out);
        let spot = l.base + l.out * LADDER_OFF;
        let mut w = WallClimb::new(LADDER_R, l.out);
        w.ladder = Some(i);
        if (p.y - l.base.y).abs() < 0.4 {
            // From the foot: the step on ends with the rungs in reach.
            let clip = lib.get(LADDER_ON)?;
            let at = Transform { translation: spot.with_y(p.y) - world_rot(rot) * root_motion_at(&clip, clip.frames()).with_z(0.0), rotation: rot, ..*root };
            w.start(clip, LADDER_R.into(), &at);
        } else {
            // From the top, facing out over the edge: turn and climb down onto it.
            let clips = LADDER_PULLDOWN.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>()?;
            let end = chain_end(&clips, rig, base, cr)?;
            let out_rot = facing(l.out);
            let start = spot - world_rot(out_rot) * end.motion;
            let at = Transform { translation: start.with_y(p.y), rotation: out_rot, ..*root };
            w.start_chain(clips, vec![LADDER_R.into(), LADDER_R.into(), LADDER_R.into()], &at, Vec3::ZERO);
        }
        w.ease_in(from, root);
        debug!("climb: onto ladder {i}");
        Some(w)
    }

    /// Hanging at rest on a wall, sideways with a ladder beside on the same wall: AC1's move across onto it (from the
    /// wall `xx_l_climbing_1m_tr_l_ladder_wait_*`, a wall hang `xx_h_hangwall_tr_l_ladder_*`, a free hang
    /// `xx_h_hangfree_tr_l_ladder_*`), or its side leap onto one further off (`xx_h_climb_{l|r}_{hand}_{2|3}_tr_ladder_*`).
    /// The clip whose root motion ends nearest the ladder's climbing spot is played, the rest made up over it.
    #[allow(clippy::too_many_arguments)]
    fn try_onto_ladder(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform, rig: &Rig, base: &Pose, cr: ClimbRig, dir: &str) -> bool {
        if !is_rest(&self.state) {
            return false;
        }
        let side = side_name(dir);
        let mut names: Vec<(String, f32)> = match self.state.as_str() {
            HANGWALL | HANGWALL_OPEN => ["l", "r"].map(|h| (format!("xx_h_hangwall_tr_l_ladder_wait_{h}_{side}"), LADDER_SIDE_REACH)).to_vec(),
            FREE | FREE_OPEN => ["l", "r"].map(|h| (format!("xx_h_hangfree_tr_l_ladder_wait_{h}_{side}"), LADDER_SIDE_REACH)).to_vec(),
            _ => ["l", "r"].map(|h| (format!("xx_l_climbing_1m_tr_l_ladder_wait_{h}_{side}"), LADDER_SIDE_REACH)).to_vec(),
        };
        if !is_free(&self.state) {
            let (d, hand, wait) = if dir == "r" { ("r", "rhand", "l") } else { ("l", "lhand", "r") };
            names.extend([2, 3].map(|n| (format!("xx_h_climb_{d}_{hand}_{n}_tr_ladder_wait_{wait}"), LADDER_LEAP_REACH)));
        }
        let right = self.normal.cross(Vec3::Y).normalize_or_zero() * -1.0;
        let way = if dir == "r" { right } else { -right };
        let mut best: Option<(f32, Arc<Clip>, String, usize, Vec3)> = None;
        for (name, reach) in names {
            let Some(clip) = lib.get(&name) else { continue };
            // Where the hands end, by the pose: the side leaps carry the body in the pose, not the root track. On a
            // ladder they hold its rails either side of its middle.
            let Some(ce) = chain_end(std::slice::from_ref(&clip), rig, base, cr) else { continue };
            let end = root.translation + world_rot(root.rotation) * ce.motion;
            let ce = ce.world(root);
            let hands = (ce.hands[0] + ce.hands[1]) * 0.5;
            // (The clip must go the way asked: some are named for the hand, not the way.)
            if (hands - root.translation).dot(way) < 0.1 {
                continue;
            }
            for (i, l) in level.ladders.iter().enumerate() {
                if l.out.dot(self.normal) < 0.9 || end.y < l.base.y + 0.3 || end.y > l.top - 0.6 {
                    continue;
                }
                let spot = l.base + l.out * LADDER_OFF;
                let off = right * (l.base - hands).dot(right) + l.out * (spot - end).dot(l.out);
                if off.length() < reach && best.as_ref().is_none_or(|b| off.length() < b.0) {
                    let to = if name.contains("ladder_wait_l") { LADDER_L } else { LADDER_R };
                    best = Some((off.length(), clip.clone(), to.to_string(), i, off));
                }
            }
        }
        let Some((_, clip, to, i, off)) = best else { return false };
        debug!("climb: across onto ladder {i} via {}", clip.name);
        let l = level.ladders[i];
        self.start(clip, to, root);
        if let Some(m) = &mut self.mv {
            m.correct = off;
        }
        self.normal = l.out;
        self.ladder = Some(i);
        true
    }

    /// On a ladder, sideways: across onto the wall's holds beside it, into a wall hang or a free hang
    /// (`xx_h_ladder_wait_tr_{hangwall|hangfree}_{left|right}`), whichever the holds and footing there allow.
    #[allow(clippy::too_many_arguments)]
    fn off_ladder_side(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform, rig: &Rig, base: &Pose, cr: ClimbRig, x: f32) {
        if self.mv.is_some() {
            return;
        }
        let side = if x > 0.0 { "right" } else { "left" };
        let cands = vec![
            Cand { names: vec![format!("xx_h_ladder_wait_tr_hangwall_{side}")], to: HANGWALL_OPEN.into(), feet: Feet::Wall },
            Cand { names: vec![format!("xx_h_ladder_wait_tr_hangfree_{side}")], to: FREE.into(), feet: Feet::Free },
        ];
        let ladder = self.ladder.take();
        if self.take_first(lib, level, root, rig, base, cr, cands) {
            debug!("climb: off ladder {} onto the wall, {side}", ladder.unwrap_or(0));
        } else {
            self.ladder = ladder;
        }
    }

    /// On a ladder between moves: up or down a step, off the top or the bottom.
    fn ladder_step(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform, input: Vec2) {
        let Some(l) = self.ladder.and_then(|i| level.ladders.get(i).copied()) else { return };
        let hand = if self.state == LADDER_L { "l" } else { "r" };
        // In high profile, AC1's high-profile ladder set (`HumanLadderData` asks for `xx_h_ladder_*` then): quicker
        // steps, running off the top into a free step and stepping off the bottom into the high wait.
        let p = if self.high { "h" } else { "l" };
        let rate = 1.0;
        let play = |w: &mut Self, clip: Arc<Clip>, to: &str, correct: Vec3| {
            w.start(clip, to.into(), root);
            if let Some(m) = &mut w.mv {
                m.correct = correct;
                m.rate = rate;
            }
        };
        if input.y > 0.3 {
            // Off the top when it is a step away, else up a step.
            let foot = if hand == "l" { "footl" } else { "footr" };
            let names = if self.high {
                [format!("xx_h_ladder_climb_up_{hand}_tr_freestep_{foot}_a"), format!("xx_h_ladder_climb_up_{hand}_tr_freestep_{foot}_b")]
            } else {
                [format!("xx_l_ladder_climb_up_{hand}_tr_l_wait_hipm_{foot}_a"), format!("xx_l_ladder_climb_up_{hand}_tr_l_wait_hipm_{foot}_b")]
            };
            if let [Some(a), Some(b)] = names.map(|n| lib.get(&n)) {
                let up = root_motion_at(&a, a.frames()).z + root_motion_at(&b, b.frames()).z;
                let need = l.top - root.translation.y;
                // (The high steps are 1 m: the exit may be up to half a step off, made up over it.)
                if (need - up).abs() < if self.high { 0.55 } else { 0.3 } {
                    self.start_chain(vec![a, b], vec![KNEEL.into(), ON_TOP.into()], root, Vec3::Y * (need - up));
                    self.ladder = None;
                    return;
                }
            }
            // A high step where a whole one fits below the top, else a low one.
            let room = l.top - 0.7 - root.translation.y;
            let p = if p == "h" && room >= 2.0 * LADDER_STEP { "h" } else { "l" };
            if room >= LADDER_STEP
                && let Some(c) = lib.get(&format!("xx_{p}_ladder_climb_up_{hand}"))
            {
                play(self, c, if hand == "l" { LADDER_R } else { LADDER_L }, Vec3::ZERO);
            }
        } else if input.y < -0.3 {
            let ground = level.ground(root.translation, 0.3, 6.0).map_or(l.base.y, |g| g.point.y);
            let above = root.translation.y - ground;
            if above < LADDER_STEP - 0.05 {
                // Step off at the bottom (onto the ground under the root).
                let foot = if hand == "l" { "footr" } else { "footl" };
                if let Some(c) = lib
                    .get(&format!("xx_{p}_ladder_climb_down_{hand}_tr_{p}_wait_hipm_{foot}"))
                    .or_else(|| lib.get(&format!("xx_l_ladder_climb_down_{hand}_tr_l_wait_hipm_{foot}")))
                {
                    self.ladder = None;
                    play(self, c, GROUND, Vec3::Y * -above);
                }
            } else if let Some(c) = lib.get(&format!("xx_{p}_ladder_climb_down_{hand}")).or_else(|| lib.get(&format!("xx_l_ladder_climb_down_{hand}"))) {
                play(self, c, if hand == "l" { LADDER_R } else { LADDER_L }, Vec3::ZERO);
            }
        }
    }

    /// Space under the end of a kiosk frame, facing along it: jump up to it and cross it.
    pub fn monkey_bars(lib: &mut AnimLib, level: &Level, root: &Transform, from: Option<Pose>) -> Option<WallClimb> {
        let p = root.translation;
        let fwd = (root.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
        let clip = lib.get(MONKEY_ON)?;
        let reach = root_motion_at(&clip, clip.frames());
        let (i, dir, q) = level.monkey.iter().enumerate().find_map(|(i, m)| {
            let dir = m.axis() * m.axis().dot(fwd).signum();
            let q = m.closest(p.with_y(m.a.y));
            let lateral = (q - p).with_y(0.0) - dir * (q - p).with_y(0.0).dot(dir);
            let end = if dir.dot(m.axis()) > 0.0 { m.b } else { m.a };
            let left = (end - p).with_y(0.0).dot(dir);
            // Under the frame, or at most `MONKEY_REACH` short of its end (the clip jumps up into it).
            let along = (p - m.a).with_y(0.0).dot(m.axis());
            let under = (-MONKEY_REACH..=(m.b - m.a).with_y(0.0).length() + MONKEY_REACH).contains(&along);
            (dir.dot(fwd) > 0.7 && under && lateral.length() < 0.6 && (1.8..2.5).contains(&(m.a.y - p.y)) && left > reach.x + 1.2).then_some((i, dir, q))
        })?;
        let rot = facing(dir);
        let natural = p + world_rot(rot) * reach;
        // Land under the frame's line, hanging from it.
        let target = q + dir * (natural - q).with_y(0.0).dot(dir) - Vec3::Y * MONKEY_HANG;
        // Nothing in the way (the move slides the root there).
        let to = (target - p).with_y(0.0);
        if to.length() > 0.05 && level.raycast(p + Vec3::Y * 1.0, to.normalize(), to.length() + 0.3).is_some_and(|h| h.normal.y.abs() < 0.5) {
            return None;
        }
        let mut w = WallClimb::new(MONKEY, -dir);
        w.monkey = Some(i);
        w.start(clip, MONKEY.into(), &Transform { translation: p, rotation: rot, ..*root });
        if let Some(m) = &mut w.mv {
            m.correct = target - natural;
        }
        w.ease_in(from, root);
        debug!("climb: onto kiosk frame {i}");
        Some(w)
    }

    /// Hanging from a kiosk frame: another step along it, or drop off at its end (or when asked).
    fn monkey_next(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform, done: &Clip) -> bool {
        let Some(m) = self.monkey.and_then(|i| level.monkey.get(i).copied()) else { return false };
        // (`mirrored` still describes the move that just ended.)
        let (was_step, was_mirrored) = (done.name == MONKEY_STEP, self.mirrored);
        let dir = (root.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
        let end = if dir.dot(m.axis()) > 0.0 { m.b } else { m.a };
        let step = lib.get(MONKEY_STEP);
        let len = step.as_ref().map_or(0.0, |c| root_motion_at(c, c.frames()).x);
        if !std::mem::take(&mut self.want_drop)
            && (end - root.translation).with_y(0.0).dot(dir) > len + 0.3
            && let Some(step) = step
        {
            // Keep under the line.
            let natural = root.translation + world_rot(root.rotation) * root_motion_at(&step, step.frames());
            let q = m.closest(natural.with_y(m.a.y)) - Vec3::Y * MONKEY_HANG;
            // Hands alternate: after a step, the next one is its mirror image.
            let mirror = was_step && !was_mirrored;
            self.start(step, MONKEY.into(), root);
            self.mirrored = mirror;
            // The step ends in the other hand's pose: no crossfade needed between halves.
            self.fade = None;
            if let Some(mv) = &mut self.mv {
                mv.correct = q - natural;
            }
            return true;
        }
        let Some(off) = lib.get(MONKEY_OFF) else { return false };
        let mirror = was_step && was_mirrored;
        self.monkey = None;
        self.let_fall(off, root, dir * 1.5);
        self.mirrored = mirror;
        true
    }

    /// Let go into a fall with `velocity`, `clip` posing it (its root motion is ignored: gravity moves
    /// the root).
    fn let_fall(&mut self, clip: Arc<Clip>, root: &Transform, velocity: Vec3) {
        self.queue.clear();
        self.fall_top = root.translation.y;
        self.state = FALL.into();
        self.start(clip, FALL.into(), root);
        self.fall_v = Some(velocity);
    }

    /// Walked or ran off a ledge: fall with `velocity`, landing (or catching a hold, or into hay) on the
    /// way down. `from` is the pose shown now.
    pub fn falling(lib: &mut AnimLib, root: &Transform, velocity: Vec3, from: Option<Pose>) -> Option<WallClimb> {
        let pose = lib.get(FALL_POSE)?;
        let mut w = WallClimb::new(FALL, root.rotation * Vec3::Z);
        w.start(pose, FALL.into(), root);
        w.fall_v = Some(velocity);
        w.grab_on_legs = true;
        w.ease_in(from, root);
        Some(w)
    }

    /// Leap of faith: at the edge of a high roof with a haystack below and ahead (along `dir`), dive
    /// into it. `from` is the pose shown now.
    pub fn leap_of_faith(lib: &mut AnimLib, level: &Level, root: &Transform, dir: Vec3, from: Option<Pose>) -> Option<WallClimb> {
        let fwd = dir.with_y(0.0).normalize_or_zero();
        // An edge just ahead (no floor under it); at a viewpoint the hay's way is taken below.
        let at_viewpoint = level.viewpoints.iter().any(|v| v.distance(root.translation) < VIEWPOINT_REACH);
        if !at_viewpoint && level.ground(root.translation + fwd * 1.2, 0.5, 2.0).is_some() {
            return None;
        }
        // A beam going on from this edge (run at a little off its line, the floor probe misses it): out along it,
        // and the leap from its end; not through it from here.
        let p = root.translation;
        if level.perch_at(p, 0.3).is_none()
            && (1..=4).any(|k| level.perch_at(p + fwd * (0.3 * k as f32), FAITH_BEAM_SIDE).is_some_and(|(_, q)| (q - p).dot(fwd) > 0.2))
        {
            return None;
        }
        // (At a viewpoint, its hay whichever way he faces: AC1 builds each viewpoint facing the hay below it.)
        let at_viewpoint = level.viewpoints.iter().any(|v| v.distance(root.translation) < VIEWPOINT_REACH);
        let hay = level
            .haystacks
            .iter()
            .filter(|h| {
                let d = (h.centre - root.translation).with_y(0.0);
                let ahead = at_viewpoint || d.normalize_or_zero().dot(fwd) > 0.7;
                d.length() < FAITH_MAX_DIST * if at_viewpoint { 1.6 } else { 1.0 } && ahead && root.translation.y - h.top() > FAITH_MIN_DROP
            })
            .min_by(|a, b| (a.centre - root.translation).length().total_cmp(&(b.centre - root.translation).length()))?;
        let drop = root.translation.y - hay.top();
        // The high dive's takeoff drops the root about 8 m with little way on: only where nothing is under that drop
        // (a minaret's balcony is); else the low one, the flight carrying it clear.
        let toward = (hay.centre - root.translation).with_y(0.0).normalize_or(fwd);
        let clear_below = level.raycast(root.translation + toward * 1.2 - Vec3::Y * 0.3, Vec3::NEG_Y, (FAITH_HIGH_DROP - 1.0).min(drop - 0.5)).is_none();
        let names = if drop > FAITH_HIGH_DROP && clear_below { FAITH_HIGH } else { FAITH_LOW };
        let (takeoff, dive) = (lib.get(names[0])?, lib.get(names[1])?);
        lib.get(FAITH_LAND)?;
        let planned = Transform { rotation: Quat::from_rotation_arc(Vec3::NEG_Z, toward), ..*root };
        // Where the takeoff leaves us and how fast it is going down; then the time to fall onto the hay,
        // the sideways speed that lands in its middle, and the dive stretched over that time.
        let r = world_rot(planned.rotation);
        let end = takeoff.frames();
        let k = end.min(3.0);
        let leave = planned.translation + r * root_motion_at(&takeoff, end);
        let v0 = (r * (root_motion_at(&takeoff, end) - root_motion_at(&takeoff, end - k)) * (FPS / k)).y.min(0.0);
        let h = (leave.y - hay.top()).max(0.1);
        let t = (v0 + (v0 * v0 + 2.0 * GRAVITY * h).sqrt()) / GRAVITY;
        let flat = (hay.centre - leave).with_y(0.0) / t;
        let mut w = WallClimb::new(FAITH, -toward);
        w.queue = vec![Queued { rate: dive.anim.duration / t, ..Queued::new(dive, FALL) }];
        w.fall_with = Some(flat.with_y(v0));
        w.faith = true;
        w.start(takeoff, FAITH.into(), &planned);
        w.ease_in(from, root);
        Some(w)
    }

    /// Coming down into a haystack: the hay entry clip, corrected to end in the middle of it at the
    /// bottom. `step_y` is this frame's vertical step (to tell that the entry height was just crossed).
    fn try_hay(&mut self, lib: &mut AnimLib, level: &Level, root: &mut Transform, step_y: f32) -> bool {
        let (name, depth) = if self.faith { (FAITH_LAND, 1.6) } else { (AIR_TO_HAY, 1.2) };
        let Some(hay) = level.haystacks.iter().find(|h| {
            let entry = h.centre.y + depth;
            h.contains(root.translation, 0.3) && root.translation.y <= entry && root.translation.y - step_y > entry
        }) else {
            return false;
        };
        let Some(clip) = lib.get(name) else { return false };
        let motion = world_rot(root.rotation) * root_motion_at(&clip, clip.frames());
        let correct = hay.centre - (root.translation + motion);
        self.fall_v = None;
        self.queue.clear();
        self.start(clip, HAY.into(), root);
        if let Some(m) = &mut self.mv {
            m.correct = correct;
        }
        true
    }

    /// In the hay: hop out forward and stand up.
    fn hop_out(&mut self, lib: &mut AnimLib, root: &Transform) -> bool {
        if self.state != HAY || self.mv.is_some() {
            return false;
        }
        let Some(clips) = HAY_HOP_OUT.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>() else { return false };
        self.start_chain(clips, vec![HAY_OUT.into(), GROUND.into()], root, Vec3::ZERO);
        true
    }

    /// Rebound during a wall run: kick off the wall back, or to the side the stick leans to, and fly.
    fn try_rebound(&mut self, lib: &mut AnimLib, root: &Transform) -> bool {
        let Some(m) = &self.mv else { return false };
        let phase = if m.clip.name == WALL_RUN[0] || m.clip.name == WALL_RUN[1] {
            "entryrebound"
        } else if m.clip.name == WALL_RUN[2] || (m.clip.name == WALL_RUN_FALL && m.t < m.clip.anim.duration * REBOUND_LATE) {
            // (And at the top of the step, as it starts back down: AC1 rebounds from there too,
            // `xx_h_wallingfront_step1_footr_tr_rebound_footr`.)
            "step1rebound"
        } else {
            return false;
        };
        // The way the stick points on screen (the camera's): of the rebounds back, left and right, the one whose kick-off
        // heads nearest it (by the stick's own left and right, a side camera turned them: left took the stick back).
        let foot = if phase == "entryrebound" { "footr" } else { "footl" };
        // (Pushing into the wall says nothing about the way off it: only the stick's part along the wall counts then.)
        let stick = self.move_dir.with_y(0.0);
        let out = self.normal.with_y(0.0).normalize_or_zero();
        let along = stick - out * stick.dot(out);
        let want = if stick.dot(out) >= 0.0 {
            stick.normalize_or_zero()
        } else if along.length() > REBOUND_SIDE_MIN {
            along.normalize()
        } else {
            Vec3::ZERO
        };
        let heading = |c: &Arc<Clip>| (world_rot(root.rotation) * root_motion_at(c, c.frames())).with_y(0.0).normalize_or_zero();
        let options: Vec<(Arc<Clip>, f32)> = ["back", "left", "right"]
            .iter()
            .filter_map(|side| lib.get(&format!("xx_h_wallingfront_{phase}_{side}_{foot}")))
            .map(|c| {
                let d = heading(&c).dot(want);
                (c, d)
            })
            .collect();
        let pick = if want == Vec3::ZERO { options.into_iter().next() } else { options.into_iter().max_by(|a, b| a.1.total_cmp(&b.1)) };
        let Some((clip, _)) = pick else { return false };
        debug!("climb: rebound {}", clip.name);
        // Fly along the clip's sideways motion; gravity does the falling.
        let motion = world_rot(root.rotation) * root_motion_at(&clip, clip.frames());
        let flat = motion.with_y(0.0) / clip.anim.duration.max(0.1);
        self.queue.clear();
        self.state = FALL.into();
        self.start(clip, FALL.into(), root);
        self.fall_v = Some(flat + Vec3::Y * 2.0);
        self.can_catch = true;
        true
    }

    /// Side wall run: sprinting along `velocity` with a wall to one side at a shallow angle and more of it
    /// ahead. `from` is the pose shown now.
    pub fn side_run(lib: &mut AnimLib, level: &Level, root: &Transform, velocity: Vec3, from: Option<Pose>) -> Option<WallClimb> {
        let dir = velocity.with_y(0.0).normalize_or_zero();
        let side = dir.cross(Vec3::Y);
        let hit = [side, -side]
            .into_iter()
            .filter_map(|s| level.raycast(root.translation + Vec3::Y, s, SIDE_RUN_REACH))
            .filter(|h| h.normal.y.abs() < 0.3 && dir.dot(h.normal).abs() < 0.5)
            .min_by(|a, b| a.dist.total_cmp(&b.dist))?;
        let normal = hit.normal.with_y(0.0).normalize();
        let along = (dir - normal * dir.dot(normal)).normalize();
        let speed = velocity.length().max(SIDE_RUN_CLIP_SPEED);
        // The wall must go on for the whole run.
        let ahead = root.translation + along * speed * SIDE_RUN_TIME * 0.5 + Vec3::Y + normal * 0.5;
        level.raycast(ahead, -normal, hit.dist + 1.0)?;
        let clips = [lib.get(RUN_CYCLE[0])?, lib.get(RUN_CYCLE[1])?];
        let mut w = WallClimb::new(SIDE_RUN, normal);
        w.side_run = Some(SideRun { t: 0.0, start: root.translation, along, normal, wall: hit.point, dist0: hit.dist, speed, clips, foot: 0, phase: 0.0 });
        w.fade = from.map(|p| (p, ENTER_FADE, ENTER_FADE));
        debug!("climb: side wall run along {along:.2} at {:.2}", root.translation);
        Some(w)
    }

    /// During a side wall run: a point on the wall, its outward normal, how far into the run (0..1)
    /// and how far rolled onto the wall (0..1), for planting the feet and the wall-side hand.
    pub fn side_wall(&self) -> Option<(Vec3, Vec3, f32, f32)> {
        let sr = self.side_run.as_ref()?;
        let u = (sr.t / SIDE_RUN_TIME).min(1.0);
        Some((sr.wall, sr.normal, u, smoothstep(u * 4.0) * smoothstep((1.0 - u) * 4.0)))
    }

    fn side_run_step(&mut self, lib: &mut AnimLib, level: &Level, root: &mut Transform, dt: f32) {
        let Some(sr) = &mut self.side_run else { return };
        sr.t += dt;
        let u = (sr.t / SIDE_RUN_TIME).min(1.0);
        // Roll onto the wall and back off it; rise and fall in an arc.
        let roll = smoothstep(u * 4.0) * smoothstep((1.0 - u) * 4.0);
        let base = sr.start + sr.along * sr.speed * sr.t;
        let off = sr.dist0 + (SIDE_RUN_OFF - sr.dist0) * roll;
        let pos = base + sr.normal * (off - (base - sr.wall).dot(sr.normal)) + Vec3::Y * 4.0 * SIDE_RUN_ARC * u * (1.0 - u);
        let face = Quat::from_rotation_arc(Vec3::NEG_Z, sr.along);
        let tilt = Quat::from_axis_angle(sr.along, SIDE_RUN_ROLL * roll);
        // Tilt the body's up away from the wall.
        let tilt = if (tilt * Vec3::Y).dot(sr.normal) >= 0.0 { tilt } else { tilt.inverse() };
        root.translation = pos;
        root.rotation = tilt * face;
        sr.phase += dt * (sr.speed / SIDE_RUN_CLIP_SPEED) / sr.clips[sr.foot].anim.duration.max(1e-3);
        if sr.phase >= 1.0 {
            sr.phase -= 1.0;
            sr.foot ^= 1;
        }
        let wall_gone = level.raycast(pos + sr.normal * 0.6 + Vec3::Y, -sr.normal, 1.6).is_none();
        if u >= 1.0 || wall_gone {
            let sr = self.side_run.take().expect("side run");
            let v = sr.along * sr.speed + sr.normal + Vec3::Y * (4.0 * SIDE_RUN_ARC / SIDE_RUN_TIME) * (1.0 - 2.0 * u);
            self.leave_side_run(lib, root, v);
        }
    }

    /// End a side wall run in the air: upright, facing the way we fly, falling with `velocity`.
    fn leave_side_run(&mut self, lib: &mut AnimLib, root: &Transform, velocity: Vec3) {
        self.side_run = None;
        // The fall turns the root to the move's start rotation.
        let upright = Transform { rotation: Quat::from_rotation_arc(Vec3::NEG_Z, velocity.with_y(0.0).normalize_or(Vec3::NEG_Z)), ..*root };
        if let Some(air) = lib.get(JUMP_AIR) {
            let rate = air.anim.duration / jump_airtime();
            self.start(air, FALL.into(), &upright);
            if let Some(m) = &mut self.mv {
                m.rate = rate;
            }
        }
        self.state = FALL.into();
        self.fall_v = Some(velocity);
        self.can_catch = true;
    }

    /// Blend in from the ground: crossfade from the pose shown and move the root from where it stood
    /// (`was`) into the first move's planned start over that move.
    fn ease_in(&mut self, from: Option<Pose>, was: &Transform) {
        self.fade = from.map(|p| (p, ENTER_FADE, ENTER_FADE));
        if let Some(m) = &mut self.mv {
            m.correct += m.start - was.translation;
            m.start = was.translation;
            m.ease_rot = was.rotation * m.start_rot.inverse();
        }
    }

    fn start(&mut self, clip: Arc<Clip>, to: String, root: &Transform) {
        debug!("climb: {} -> {to} via {} at {:.2} yaw {:.0}", self.state, clip.name, root.translation, root.rotation.to_euler(EulerRot::YXZ).0.to_degrees());
        self.fade = self.last.clone().map(|p| (p, FADE, FADE));
        self.mirrored = false;
        self.mv = Some(Move { clip, t: 0.0, to, start: root.translation, start_rot: root.rotation, correct: Vec3::ZERO, rate: 1.0, ease_rot: Quat::IDENTITY });
    }

    /// Play clips back to back, ending each in the matching state of `tos`; `correct` is spread over
    /// the clip that moves the root the most.
    fn start_chain(&mut self, clips: Vec<Arc<Clip>>, tos: Vec<String>, root: &Transform, correct: Vec3) {
        self.start_chain_carry(clips, tos, root, correct, None);
    }

    /// `start_chain` with the correction on clip `carry` (by default the one that moves the most).
    fn start_chain_carry(&mut self, clips: Vec<Arc<Clip>>, tos: Vec<String>, root: &Transform, correct: Vec3, carry: Option<usize>) {
        let reach = |c: &Arc<Clip>| root_motion_at(c, c.frames()).length();
        let carry = carry.or_else(|| (0..clips.len()).max_by(|&a, &b| reach(&clips[a]).total_cmp(&reach(&clips[b]))));
        let mut items = clips
            .into_iter()
            .zip(tos)
            .enumerate()
            .map(|(i, (clip, to))| Queued { correct: if Some(i) == carry { correct } else { Vec3::ZERO }, ..Queued::new(clip, to) });
        let Some(first) = items.next() else { return };
        self.queue = items.collect();
        self.start(first.clip, first.to, root);
        if let Some(m) = &mut self.mv {
            m.correct = first.correct;
        }
    }

    /// A hang settling after a move: square to the wall (a catch at an angle must not stay at it), and
    /// hands together or apart as they are (the closed or the open wait, whichever the hands match).
    fn settle_hang(&mut self, lib: &mut AnimLib, level: &Level, root: &mut Transform, rig: &Rig, base: &Pose, cr: ClimbRig) {
        let Some(last) = self.last.clone() else { return };
        if is_rest(&self.state) {
            let square = Quat::from_rotation_arc(Vec3::NEG_Z, -self.normal.with_y(0.0).normalize_or(Vec3::Z));
            if root.rotation.angle_between(square) > 0.08 {
                debug!("climb: squaring up to the wall ({:.0} degrees off)", root.rotation.angle_between(square).to_degrees());
                root.rotation = square;
                self.fade = Some((last.clone(), WAIT_FADE, WAIT_FADE));
            }
        }
        let pair = match self.state.as_str() {
            FREE | FREE_OPEN => Some([(FREE, "xx_h_hangfree_waitclose"), (FREE_OPEN, "xx_h_hangfree_waitopen")]),
            HANGWALL | HANGWALL_OPEN => Some([(HANGWALL, "xx_h_hangwall_waitclose"), (HANGWALL_OPEN, HANGWALL_REST)]),
            _ => None,
        };
        let width = |p: &Pose| {
            let m = p.model(rig);
            (m[cr.hands[0]].pos - m[cr.hands[1]].pos).length()
        };
        if let Some(pair) = pair {
            let now = width(&last);
            let mut off = |name: &str| {
                lib.get(name).map_or(f32::MAX, |c| {
                    let mut p = base.clone();
                    sample(&c, 0.0, &mut p, cr.reference);
                    (width(&p) - now).abs()
                })
            };
            let (a, b) = (off(pair[0].1), off(pair[1].1));
            self.state = (if a <= b { pair[0].0 } else { pair[1].0 }).to_string();
            // From a pole's cap the hands hang together (where they hold, from a wall hang's root).
            let hold = root.translation + Vec3::Y * HANGWALL_HOLD_UP - self.normal * HANGWALL_HOLD_IN;
            if self.state == HANGWALL_OPEN && on_narrow_ledge(level, hold) {
                self.state = HANGWALL.into();
            }
        }
    }

    /// Hang loop for the current state, or the last move held on its final frame if there is none.
    fn set_wait(&mut self, lib: &mut AnimLib, last: Arc<Clip>) {
        let name = match self.state.as_str() {
            FREE => Some("xx_h_hangfree_waitclose".to_string()),
            FREE_OPEN => Some("xx_h_hangfree_waitopen".to_string()),
            HAY => Some(HAY_WAIT.to_string()),
            PERCH => Some(match self.beam_stance {
                Some(BeamStance::Along(f)) => BEAM_WAIT[f].to_string(),
                Some(BeamStance::Across) => BEAM_WAIT_ACROSS.to_string(),
                None => PERCH_WAIT.to_string(),
            }),
            LEAN => Some(lean_name(LEAN_WAIT, self.lean_mix)),
            COLLIDE => Some(collide_name("xx_h_collide_full_footl_{h}cm_wait", self.lean_mix)),
            BENCH => Some(BENCH_WAIT.to_string()),
            HANGWALL => Some("xx_h_hangwall_waitclose".to_string()),
            HANGWALL_OPEN => Some(HANGWALL_REST.to_string()),
            LADDER_L => Some("xx_l_ladder_wait_l".to_string()),
            LADDER_R => Some("xx_l_ladder_wait_r".to_string()),
            DEAD => None,
            s if is_free(s) => None,
            s => Some(format!("xx_climb_wait_{s}")),
        };
        match name.and_then(|n| lib.get(&n)) {
            Some(w) => {
                self.wait = Some(w);
                self.wait_hold = false;
                self.wait_t = 0.0;
            }
            None => {
                self.wait_t = last.anim.duration;
                self.wait = Some(last);
                self.wait_hold = true;
            }
        }
    }

    /// Pull up onto the top: the hands must hold the top edge with walkable ground beyond it. From
    /// the wall ("1m") the feet push up the wall; from a free hang the body pulls up to the waist.
    fn try_top_out(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform, rig: &Rig, base: &Pose, cr: ClimbRig) -> bool {
        let names: &[&str] = match self.state.as_str() {
            "1m" => &TOP_OUT,
            "2m" => &TOP_OUT_2M,
            HANGWALL | HANGWALL_OPEN => &TOP_OUT_HANGWALL,
            FREE | FREE_OPEN => &TOP_OUT_FREE,
            _ => return false,
        };
        let Some(clips) = names.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>() else { return false };
        let mut pose = base.clone();
        sample(&clips[0], 0.0, &mut pose, cr.reference);
        let m = pose.model(rig);
        let wrist = root.translation + world_rot(root.rotation) * ((m[cr.hands[0]].pos + m[cr.hands[1]].pos) * 0.5);
        // Top surface just behind the holds, level with them.
        let probe = wrist - self.normal * 0.45 + Vec3::Y * 0.5;
        let Some(top) = level.ground(probe, 0.0, 0.8) else {
            debug!("climb: no top-out: no top behind the hold (probed at {probe:.2})");
            return false;
        };
        // (The hands-together wall hang holds 0.15 m lower than the first frame of the pull-up puts them.)
        if (top.point.y - (wrist.y + GRIP_DOWN)).abs() > 0.2 || top.normal.y < 0.8 {
            debug!("climb: no top-out: top at {:.2}, grip at {:.2}, slope {:.2}", top.point.y, wrist.y + GRIP_DOWN, top.normal.y);
            return false;
        }
        // Room to stand on it: headroom over where the body comes up, and no wall just past the edge (a window
        // sill's top is a floor behind a hold, with the window's opening or the room's wall right behind it; AC1 does
        // not climb in through those).
        // (The open probe starts in front of the wall: from behind the hold it may start inside the wall, when the top
        // found is a beam's or a ledge piece's stuck into a building, and see nothing.)
        let stand = top.point - self.normal * TOP_OUT_STAND_IN;
        let headroom = level.raycast(stand + Vec3::Y * 0.1, Vec3::Y, TOP_OUT_HEADROOM).is_none();
        let front = Vec3::new(wrist.x, top.point.y, wrist.z) + self.normal * 0.3;
        let reach = (top.point - front).with_y(0.0).length() + TOP_OUT_STAND_IN + 0.3;
        let open = [0.5, 1.2].iter().all(|h| level.raycast(front + Vec3::Y * *h, -self.normal, reach).is_none());
        if !headroom || !open || level.inside_solid(stand + Vec3::Y * 0.5) {
            let wall = [0.5, 1.2].map(|h| level.raycast(front + Vec3::Y * h, -self.normal, 3.0).map(|w| w.dist - 0.3));
            debug!("climb: no top-out: no room to stand at {stand:.2} (headroom {headroom}, open {open}: a wall {wall:.2?} m past the edge)");
            return false;
        }
        // Nothing to stand on beside the hands (a post, the end of a wall): pull up with one hand.
        let along = Vec3::Y.cross(self.normal).normalize_or_zero();
        let p = Vec3::new(wrist.x, top.point.y, wrist.z) - self.normal * 0.15;
        let drops = |s: f32| level.ground(p + along * ONEHAND_SIDE * s + Vec3::Y * 0.05, 0.0, 0.25).is_none();
        let one = if drops(1.0) || drops(-1.0) {
            let names: &[&str] = if is_free(&self.state) { &TOP_OUT_FREE_ONEHAND } else { &TOP_OUT_ONEHAND };
            names.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>()
        } else {
            None
        };
        let travel = |c: &[Arc<Clip>]| world_rot(root.rotation) * c.iter().map(|c| root_motion_at(c, c.frames())).sum::<Vec3>();
        let (clips, correct, end) = match one {
            Some(one) => {
                // Onto a post: end on it, balancing; else where the two-hand pull-up would have.
                let perch = level.perch_at(top.point, PERCH_REACH);
                let want = perch.map_or(root.translation + travel(&clips), |(_, q)| q);
                let correct = want - (root.translation + travel(&one));
                self.perch = perch.map(|(i, _)| i);
                debug!("climb: one-hand pull-up{}", if perch.is_some() { " onto a post" } else { "" });
                (one, correct, if perch.is_some() { PERCH } else { ON_TOP })
            }
            None => (clips, Vec3::ZERO, ON_TOP),
        };
        let n = clips.len();
        let tos = (0..n).map(|i| if i + 1 == n { end } else { KNEEL }.to_string()).collect();
        self.start_chain(clips, tos, root, correct);
        true
    }

    /// Climb down off the wall onto ground just below the feet (from "1m" only).
    fn try_step_off(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform) -> bool {
        if self.state != "1m" {
            return false;
        }
        let Some(ground) = level.ground(root.translation, 0.3, STEP_OFF_MAX) else { return false };
        let (Some(down), Some(stand)) = (lib.get(STEP_OFF[0]), lib.get(STEP_OFF[1])) else { return false };
        self.queue = vec![Queued::new(stand, GROUND)];
        self.start(down, STEP_DOWN.into(), root);
        // The clip lowers the root a fixed amount; make up the rest so the feet meet the ground.
        let height = root.translation.y - ground.point.y;
        if let Some(m) = &mut self.mv {
            m.correct = Vec3::Y * (STEP_OFF_DROP - height);
        }
        true
    }

    /// Climb off the wall sideways onto ground just below the feet (from "1m" only):
    /// `xx_l_climb_1m_to_groundentry_<left|right>`, then its stand.
    fn try_step_off_side(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform, left: bool) -> bool {
        if self.state != "1m" {
            return false;
        }
        let side = if left { "left" } else { "right" };
        let foot = if left { "footl" } else { "footr" };
        let (Some(off), Some(stand)) =
            (lib.get(&format!("xx_l_climb_1m_to_groundentry_{side}")), lib.get(&format!("xx_l_climb_1m_to_groundentry_{side}_tr_h_wait_{foot}")))
        else {
            return false;
        };
        // Where it steps to: ground there, near the feet's height, and room to stand.
        let to = root.translation + world_rot(root.rotation) * root_motion_at(&off, off.frames()).with_z(0.0);
        let Some(ground) = level.ground(to, 0.3, STEP_OFF_MAX) else { return false };
        if level.inside_solid(ground.point + Vec3::Y * 0.5)
            || level
                .raycast(
                    root.translation + Vec3::Y * 0.5,
                    (to - root.translation).with_y(0.0).normalize_or_zero(),
                    (to - root.translation).with_y(0.0).length(),
                )
                .is_some()
        {
            return false;
        }
        self.queue = vec![Queued::new(stand, GROUND)];
        self.start(off, STEP_DOWN.into(), root);
        if let Some(m) = &mut self.mv {
            m.correct = Vec3::Y * (ground.point.y - to.y);
        }
        debug!("climb: stepped off the wall sideways ({side}) onto the ground");
        true
    }

    /// Let go of the wall now: step down if the ground is close, else push off (or release from a
    /// free hang) and fall.
    fn drop_now(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform) -> bool {
        if self.try_step_off(lib, level, root) {
            return true;
        }
        // The push-off already drops the root; the fall counts from here.
        self.fall_top = root.translation.y;
        let names =
            if is_free(&self.state) { ["xx_h_hangfree_tr_fall_a", "xx_h_hangfree_tr_fall_b"] } else { ["xx_h_hangwall_tr_fall_a", "xx_h_hangwall_tr_fall_b"] };
        let (Some(release), Some(falling)) = (lib.get(names[0]), lib.get(names[1])) else { return false };
        self.queue = vec![Queued::new(falling, FALL)];
        self.start(release, DROP.into(), root);
        true
    }

    /// Player asked to let go. Returns false if this climber can't (missing clips), so the caller
    /// should detach it directly.
    /// The legs pressed while on the wall: jump off a perch, fling off a bar, rebound off a wall run, hop
    /// out of hay or kick off a side run. Returns false when none of those applies (leaps are the held legs).
    pub fn legs(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform, input: Vec2) -> bool {
        // Hanging on a wall, the legs with the stick pulled back: eject off it backwards.
        let on_wall = matches!(self.state.as_str(), HANGWALL | HANGWALL_OPEN) || self.state.starts_with('1') || self.state.starts_with('2');
        if on_wall && self.mv.is_none() && input.y < -0.5 {
            return self.back_eject(lib, root, input.x);
        }
        if self.state == BENCH {
            self.get_up = true;
            return true;
        }
        if self.state == COLLIDE && self.mv.is_none() {
            self.collide_step(lib, level, root, true);
            return true;
        }
        if self.state == PERCH && self.mv.is_none() {
            let dir =
                self.cycle.as_ref().map(|c| c.dir).or_else(|| (self.move_dir.length() > 0.3).then_some(self.move_dir)).unwrap_or(root.rotation * Vec3::NEG_Z);
            let dir = dir.with_y(0.0).normalize_or(Vec3::NEG_Z);
            // High up with hay below that way (a viewpoint's beam): the leap of faith, aimed at the hay.
            if let Some(w) = Self::leap_of_faith(lib, level, root, dir, self.last.clone()) {
                debug!("climb: leap of faith off the perch");
                *self = w;
                return true;
            }
            self.perch_jump(lib, level, root, dir);
            return true;
        }
        if self.bar.is_some() {
            self.swing_off = Some(self.move_dir.dot(root.rotation * Vec3::NEG_Z) > -0.3);
            return true;
        }
        if self.try_rebound(lib, root) || self.hop_out(lib, root) {
            return true;
        }
        if let Some(sr) = self.side_run.take() {
            self.leave_side_run(lib, root, sr.along * sr.speed * 0.7 + sr.normal * 3.5 + Vec3::Y * 3.0);
            return true;
        }
        false
    }

    pub fn let_go(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform, input: Vec2) -> bool {
        if self.state == BENCH {
            self.get_up = true;
            return true;
        }
        if self.state == PERCH && self.mv.is_none() {
            // Not walking a beam: down over the side to hang (done in `update`, which has the rig), else jump off.
            if self.cycle.is_none() {
                self.want_drop = true;
                return true;
            }
            let dir =
                self.cycle.as_ref().map(|c| c.dir).or_else(|| (self.move_dir.length() > 0.3).then_some(self.move_dir)).unwrap_or(root.rotation * Vec3::NEG_Z);
            self.perch_jump(lib, level, root, dir.with_y(0.0).normalize_or(Vec3::NEG_Z));
            return true;
        }
        if self.monkey.is_some() {
            self.want_drop = true;
            return true;
        }
        let pull_back = input.y < -0.5 || self.move_dir.dot(self.normal) > 0.5;
        if self.ladder.is_some() && (self.mv.is_none() || pull_back) {
            let hand = if self.state == LADDER_L { "l" } else { "r" };
            let foot = if hand == "l" { "footl" } else { "footr" };
            // Pulling away from the wall: jump off it backwards (`xx_h_ladder_wait_<l|r>_tr_rebound_foot<l|r>`),
            // catching what is behind on the way; else let go and drop.
            if pull_back && let Some(off) = lib.get(&format!("xx_h_ladder_wait_{hand}_tr_rebound_{foot}")) {
                self.ladder = None;
                self.let_fall(off, root, self.normal * LADDER_REBOUND.0 + Vec3::Y * LADDER_REBOUND.1);
                // Then AC1's rebound flight as it comes down, still facing the wall (`xx_h_rebound_foot<l|r>_tr_fall`).
                self.descend = lib.get(&format!("xx_h_rebound_{foot}_tr_fall"));
                self.can_catch = true;
                self.aimed = false;
                debug!("climb: rebound off the ladder");
            } else if let Some(off) = lib.get(&format!("xx_{}_ladder_wait_{hand}_tr_falling", if self.high { "h" } else { "l" })) {
                self.ladder = None;
                self.let_fall(off, root, self.normal * 1.0);
            }
            return true;
        }
        if self.bar.is_some() {
            // Fling forward unless pulling back.
            self.swing_off = Some(self.move_dir.dot(root.rotation * Vec3::NEG_Z) > -0.3);
            return true;
        }
        if self.try_rebound(lib, root) || self.hop_out(lib, root) {
            return true;
        }
        if let Some(sr) = self.side_run.take() {
            // Kick off the wall: away from it, still going along it.
            self.leave_side_run(lib, root, sr.along * sr.speed * 0.7 + sr.normal * 3.5 + Vec3::Y * 3.0);
            return true;
        }
        // Falling (or dropping off): the empty hand grabs the next hold the hands come to on the way down (AC1's catch
        // while falling), not the one just let go of.
        let falling = self.fall_v.is_some() || self.state == DROP || self.state == FALL || self.mv.as_ref().is_some_and(|m| m.to == DROP || m.to == FALL);
        if falling {
            self.can_catch = true;
            self.aimed = false;
            self.catch_below = Some(self.fall_top + HANG_HOLD_UP - CATCH_BELOW_LET_GO);
            debug!("climb: reaching to catch a hold on the way down");
            return true;
        }
        match &self.mv {
            // Already leaving the wall, in the air or topping out.
            Some(m) if off_wall(&m.to) => true,
            Some(_) => {
                self.want_drop = true;
                true
            }
            None if off_wall(&self.state) => true,
            None => self.drop_now(lib, level, root),
        }
    }

    /// Hand configuration to lead with going up or down: the side we lean toward, else alternate.
    fn prefer_hand(&self, input: Vec2) -> &'static str {
        if input.x > 0.2 {
            "ru"
        } else if input.x < -0.2 || self.last_hand_up != 0 {
            "lu"
        } else {
            "ru"
        }
    }

    /// Moves to try for an input direction from the current state, best first.
    fn candidates(&self, lib: &AnimLib, dir: &str, input: Vec2) -> Vec<Cand> {
        let vertical = dir == "u" || dir == "d";
        let prefer = self.prefer_hand(input);
        let mut out = vec![];
        if let Some(hands) = self.state.strip_prefix("free_") {
            let side = side_name(dir);
            match (hands, vertical) {
                ("1m" | "open", false) => {
                    let (strafe, to) = if hands == "1m" { ("open", FREE_OPEN) } else { ("close", FREE) };
                    out.push(Cand::new(format!("xx_h_hangfree_strafe_{side}_050cm_{strafe}"), to, Feet::Free));
                    // Wall under the feet again: swing back onto it.
                    out.push(Cand::new(format!("xx_h_hangfree_{dir}_climb_1m"), "1m", Feet::Wall));
                    for kind in ["in", "out"] {
                        out.push(Cand::new(format!("xx_h_hangfree_corner_{side}_090_{kind}"), FREE, Feet::Free));
                    }
                }
                ("1m" | "open", true) => {
                    out.push(Cand::new(format!("xx_h_hangfree_{dir}_climb_1m"), "1m", Feet::Wall));
                    let mut hands = ["lu", "ru"];
                    hands.sort_by_key(|h| *h != prefer);
                    for h in hands {
                        out.push(Cand::new(format!("xx_h_hangfree_climb_1m_{dir}_1{h}"), format!("free_1{h}"), Feet::Free));
                    }
                }
                (h, true) => out.push(Cand::new(format!("xx_h_hangfree_climb_{h}_{dir}_1m"), FREE, Feet::Free)),
                _ => {}
            }
            return out;
        }
        if self.state == HANGWALL || self.state == HANGWALL_OPEN {
            if !vertical {
                let (strafe, to) = if self.state == HANGWALL { ("open", HANGWALL_OPEN) } else { ("close", HANGWALL) };
                out.push(Cand::new(format!("xx_h_hangwall_strafe_{}_050cm_{strafe}", side_name(dir)), to, Feet::Wall));
            }
            // Onto the wall's holds (up, down, or along).
            out.push(Cand::new(format!("xx_h_hangwall_{dir}_climb_1m"), "1m", Feet::Wall));
            // Round a corner: AC1 has the climbing stance's only (it ends in the hang again where the feet find no holds).
            if !vertical {
                for kind in ["in", "out"] {
                    out.push(Cand::new(format!("xx_l_climb_1m_corner_{}_090_{kind}", side_name(dir)), "1m", Feet::Wall));
                }
            }
            return out;
        }
        let mut wall: Vec<(String, String)> = clip_state_names(lib, &self.state, dir);
        if vertical {
            wall.sort_by_key(|(_, to)| !to.ends_with(prefer));
        }
        out.extend(wall.into_iter().map(|(name, to)| Cand::new(name, to, Feet::Wall)));
        if self.state == "1m" || self.state == "2m" {
            if !vertical {
                // At the end of a wall, turn the corner (inside corners first, then around the outside).
                // The corner clips start from "1m"; from "2m" the crossfade brings the hands together.
                for kind in ["in", "out"] {
                    out.push(Cand::new(format!("xx_l_climb_1m_corner_{}_090_{kind}", side_name(dir)), "1m", Feet::Wall));
                }
            }
            // No holds for the feet there (a lone ledge along the top of a bare wall): into the hang, the feet braced.
            out.push(Cand::new(format!("xx_l_climb_1m_{dir}_hangwall"), HANGWALL_OPEN, Feet::Wall));
            // No wall for the feet: swing off into a free hang.
            out.push(Cand::new(format!("xx_l_climb_1m_{dir}_hangfree"), FREE, Feet::Free));
        }
        out
    }

    /// Leaps for an input direction, shortest first. Only from a rest state (hands level); the
    /// clips start from "1m" or the closed free hang, the crossfade covers wide hands.
    fn leap_candidates(&self, lib: &AnimLib, dir: &str, input: Vec2) -> Vec<Cand> {
        if !is_rest(&self.state) {
            return vec![];
        }
        let side = side_name(dir);
        let free = is_free(&self.state);
        let from = if free {
            "hangfree"
        } else if self.state == HANGWALL || self.state == HANGWALL_OPEN {
            "hangwall"
        } else {
            "climb1m"
        };
        let mut bases: Vec<(String, &str, Feet)> = vec![];
        for n in [2, 3] {
            match dir {
                // Side leaps: onto the wall (`tr_climb1m`, `tr_hangwall`), or into a free hang. The long one
                // into a free hang (`_3_a..e`) catches the ledge with one hand, the feet finding nothing,
                // swings back and steadies before the other hand joins.
                "l" | "r" => {
                    bases.push((format!("xx_h_climbing_{from}_tr_climb1m_{side}_{n}"), "1m", Feet::Wall));
                    bases.push((format!("xx_h_climbing_{from}_tr_hangwall_{side}_{n}"), HANGWALL_OPEN, Feet::Wall));
                    bases.push((format!("xx_h_climbing_{from}_tr_hangfree_{side}_{n}"), FREE, Feet::Free));
                }
                _ if !free => {
                    let way = if dir == "u" { "up" } else { "down" };
                    let mut hands = ["l", "r"];
                    hands.sort_by_key(|h| !self.prefer_hand(input).starts_with(h));
                    for h in hands {
                        bases.push((format!("xx_h_climbing_climb1m_tr_climb1m_{way}_{h}_hand_{n}"), "1m", Feet::Wall));
                    }
                }
                _ => {}
            }
        }
        bases.into_iter().map(|(base, to, feet)| Cand { names: chain_names(lib, &base), to: to.into(), feet }).filter(|c| !c.names.is_empty()).collect()
    }

    /// Take the first candidate whose end pose has both hands on holds and the feet as it needs.
    #[allow(clippy::too_many_arguments)]
    fn take_first(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform, rig: &Rig, base: &Pose, cr: ClimbRig, cands: Vec<Cand>) -> bool {
        let to_hangwall: Vec<bool> = cands.iter().map(|c| c.to == HANGWALL || c.to == HANGWALL_OPEN).collect();
        for (k, c) in cands.into_iter().enumerate() {
            let Some(clips) = c.names.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>() else {
                if why() {
                    debug!("climb: {} : missing clips", c.names.join(" + "));
                }
                continue;
            };
            let Some(end) = chain_end(&clips, rig, base, cr) else { continue };
            let end = end.world(root);
            let normal = (end.rot * root.rotation.inverse()) * self.normal;
            // Leaps are aimed: they steer onto a hold within reach of where the clip lands (and so are the moves off a
            // ladder sideways, its rungs not level with the wall's holds).
            let leap = c.names.first().is_some_and(|n| n.starts_with("xx_h_climbing_") || n.starts_with("xx_h_ladder_wait_tr_"));
            // (Corners too: where their hands land on the next face depends on how far along the wall the hands were.)
            let corner = c.names.first().is_some_and(|n| n.contains("_corner_"));
            let vertical = c.names.first().is_some_and(|n| n.starts_with("xx_l_climb_") && (n.contains("_u_") || n.contains("_d_")));
            let reach = if leap {
                LEAP_TOLERANCE
            } else if corner {
                CORNER_TOLERANCE
            } else if vertical {
                HOLD_TOLERANCE_UP
            } else {
                HOLD_TOLERANCE
            };
            // (Side and down leaps also go diagonally down onto a hold up to 1.2 m below where they land; an up
            // leap must land up: dropped back, it would catch the ledge it leapt from, over and over at a top.)
            let up = c.names.first().is_some_and(|n| n.contains("_up_"));
            let drops: &[f32] = if leap && !up { &[0.0, 0.6, 1.2] } else { &[0.0] };
            // (Both hands as far apart on the holds as in the clip: not one on each side of a gap.)
            let apart = end.hands[0].distance(end.hands[1]);
            // (A plain climbing move finds its holds as AC1's grid does, in a box round where the clip puts each hand;
            // leaps and corners steer onto the nearest within their reach.)
            let plain = !leap && !corner;
            let Some(targets) = drops
                .iter()
                .filter_map(|d| {
                    let hands = end.hands.map(|h| h - Vec3::Y * d);
                    if plain { holds_in_box(level, &hands, normal) } else { holds_within(level, &hands, normal, reach) }
                })
                .find(|t| (t[0].distance(t[1]) - apart).abs() < HANDS_APART_SLACK)
            else {
                if corner || why() {
                    // (By how much: out from the wall, up, along it.)
                    let along = Vec3::Y.cross(normal).normalize_or_zero();
                    let off = end.hands.map(|h| {
                        nearest_hold(level, h, normal).map(|n| {
                            let d = n.1 - h;
                            Vec3::new(d.dot(normal), d.y, d.dot(along))
                        })
                    });
                    debug!("climb: {} : the hands land off the holds (out, up, along: {:.2?})", c.names[0], off);
                }
                continue;
            };
            // A leap between its short and long variant's reach: AC1 mixes the two to land on the hold.
            let (c, clips, end) = match leap.then(|| self.leap_mix(lib, root, rig, base, cr, &c, &end, &targets)).flatten() {
                Some((mixed, clips, end)) => (mixed, clips, end),
                None => (c, clips, end),
            };
            let land = (targets[0] - end.hands[0] + targets[1] - end.hands[1]) * 0.5;
            if !feet_fit_at(level, &end.feet.map(|f| f + land), normal, c.feet, Some((targets[0] + targets[1]) * 0.5)) {
                if corner || why() {
                    // (What is there: each foot's wall, how far in from the foot and how far behind the hold line.)
                    let hold = (targets[0] + targets[1]) * 0.5;
                    let seen: Vec<String> = end
                        .feet
                        .iter()
                        .map(|f| {
                            let f = *f + land;
                            level
                                .raycast(f + normal * 0.3, -normal, 1.5)
                                .map_or("none".into(), |h| format!("{:.2} in, {:.2} behind the hold", h.dist - 0.3, (hold - h.point).dot(normal)))
                        })
                        .collect();
                    debug!("climb: {} : no wall for the feet ({})", c.names[0], seen.join("; "));
                }
                continue;
            }
            // Into the climbing stance with no holds under the hands for the feet: a move into the hang instead (one
            // further down the list), else this one ending in the hang (the corners: AC1 has no hang's own).
            let mut c = c;
            // (Ending in the hang's pose: the root where the hang's hands are on the holds, not the stance's.)
            let mut to_hang = Vec3::ZERO;
            if (c.to == "1m" || c.to == "2m") && !footholds(level, (targets[0] + targets[1]) * 0.5, normal) {
                if !corner && to_hangwall[k + 1..].iter().any(|&h| h) {
                    if why() {
                        debug!("climb: {} : no footholds there (a hang move further on)", c.names[0]);
                    }
                    continue;
                }
                let (Some(rest), Some(last)) = (lib.get(HANGWALL_REST), clips.last()) else { continue };
                let (hang, stance) = (end_pose(&rest, rig, base, cr), end_pose(last, rig, base, cr));
                let mid = |e: &EndPose| (e.hands[0] + e.hands[1]) * 0.5;
                to_hang = world_rot(end.rot) * (mid(&stance) - mid(&hang));
                debug!("climb: {} : no holds for the feet there, into the hang", c.names[0]);
                c.to = HANGWALL_OPEN.into();
            }
            // Sideways along the wall, the body must fit where it ends: not into a wall meeting this one (an inside
            // corner; the holds run on to its end, under the other wall). Climbing jumps too. (Corners go round.)
            let slide = (end.pos + land - root.translation).with_y(0.0);
            let side = slide - normal * slide.dot(normal);
            if !corner && side.length() > 0.05 {
                let into = [0.5, 1.2].iter().any(|h| {
                    level.raycast(root.translation + Vec3::Y * *h, side.normalize(), side.length() + BODY_SIDE).is_some_and(|w| w.normal.y.abs() < 0.5)
                });
                if into {
                    if why() {
                        debug!("climb: {} : the body would go into a wall beside", c.names[0]);
                    }
                    continue;
                }
            }
            if c.to.ends_with("ru") {
                self.last_hand_up = 1;
            } else if c.to.ends_with("lu") {
                self.last_hand_up = 0;
            }
            // Free-hang climbs reach 0.7 m with holds 0.6 m apart, and leaps land near a hold rather
            // than on it: line the hands up with the holds (in the wall plane) over the move so the
            // error doesn't build up.
            let err = (targets[0] - end.hands[0] + targets[1] - end.hands[1]) * 0.5;
            // (In the wall plane; the ledge hang's offset out from the wall is set going into it, and dropped
            // leaving it.)
            // (A corner's correction already puts the hands on the holds out from the new wall: only the hang's own.)
            let from_out = if corner { Vec3::ZERO } else { hang_offset(&self.state, normal) };
            let out = (hang_offset(&c.to, normal) - from_out).dot(normal);
            let down = if hang_offset(&c.to, normal) != Vec3::ZERO { HANG_DOWN } else { 0.0 };
            // (A corner keeps its error out from the new wall too: stopped short of the corner, the turn would end
            // inside the next wall.)
            // (Onto the wall, the hands go onto the holds in depth too: a storey set back above, a hold recessed into the
            // wall, and the body follows the wall in, the feet still on it.)
            let on_wall = !is_free(&c.to);
            let flat = if corner || on_wall { err } else { err - normal * err.dot(normal) };
            // Between free hangs he stays as far out from the wall: a clip that drifts in (AC1's free-hang leaps, made for
            // a flat wall) would put the body into a cornice over the hold.
            let keep = if !corner && !on_wall && is_free(&self.state) { -normal * (end.pos - root.translation).dot(normal) } else { Vec3::ZERO };
            let err = flat + keep + normal * out - Vec3::Y * down + to_hang;
            let tos = chain_states(clips.len(), &self.state, &c.to);
            self.start_chain(clips, tos, root, err);
            return true;
        }
        false
    }

    /// The mix of leap `c` with its other-length variant (`_2` and `_3`) that lands its hands nearest
    /// `targets`, if that is nearer than `c` alone: (the mixed candidate, its clips, where it ends).
    #[allow(clippy::too_many_arguments)]
    fn leap_mix(
        &self,
        lib: &mut AnimLib,
        root: &Transform,
        rig: &Rig,
        base: &Pose,
        cr: ClimbRig,
        c: &Cand,
        end: &WorldEnd,
        targets: &[Vec3; 2],
    ) -> Option<(Cand, Vec<Arc<Clip>>, WorldEnd)> {
        let swap = |n: &String| if n.contains("_2_") { n.replacen("_2_", "_3_", 1) } else { n.replacen("_3_", "_2_", 1) };
        let other: Vec<String> = c.names.iter().map(swap).collect();
        if other == c.names {
            return None;
        }
        let oclips = other.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>()?;
        let oend = chain_end(&oclips, rig, base, cr)?.world(root);
        let mid = |e: &WorldEnd| (e.hands[0] + e.hands[1]) * 0.5;
        let (a, b, t) = (mid(end), mid(&oend), (targets[0] + targets[1]) * 0.5);
        let d = b - a;
        let w = ((t - a).dot(d) / d.length_squared().max(1e-4)).clamp(0.0, 1.0);
        if w < 0.05 {
            return None;
        }
        let names: Vec<String> = c.names.iter().zip(&other).map(|(x, y)| if x == y { x.clone() } else { mix_name(&[(x, 1.0 - w), (y, w)]) }).collect();
        let clips = names.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>()?;
        let mend = chain_end(&clips, rig, base, cr)?.world(root);
        if (mid(&mend) - t).length() >= (a - t).length() {
            return None;
        }
        debug!("climb: leap mixed {:.2} toward its other length, landing {:.2} m off (was {:.2})", w, (mid(&mend) - t).length(), (a - t).length());
        Some((Cand { names, to: c.to.clone(), feet: c.feet }, clips, mend))
    }

    #[allow(clippy::too_many_arguments)]
    fn try_move(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform, rig: &Rig, base: &Pose, cr: ClimbRig, input: Vec2, leap: bool) -> bool {
        // AC1 splits the stick into diagonals too (`QuantizeStickDirection` 0xDEB8D0: 22.5-67.5 degrees off up or
        // down): the climbing stance's diagonal moves (`xx_l_climb_<pose>_<ul|ur|dl|dr>_<pose>`) first, else on as for
        // the nearer straight way (the move tables' redirects).
        if !leap && (self.state.starts_with('1') || self.state.starts_with('2')) && self.mv.is_none() {
            let a = input.x.atan2(input.y).to_degrees();
            let diag = match a {
                a if (22.5..67.5).contains(&a) => Some("ur"),
                a if (112.5..157.5).contains(&a) => Some("dr"),
                a if (-67.5..-22.5).contains(&a) => Some("ul"),
                a if (-157.5..-112.5).contains(&a) => Some("dl"),
                _ => None,
            };
            if let Some(d) = diag {
                let cands: Vec<Cand> = clip_state_names(lib, &self.state, d).into_iter().map(|(name, to)| Cand::new(name, to, Feet::Wall)).collect();
                if self.take_first(lib, level, root, rig, base, cr, cands) {
                    return true;
                }
            }
        }
        let vertical = input.y.abs() >= input.x.abs();
        let dir = if vertical {
            if input.y > 0.0 { "u" } else { "d" }
        } else if input.x > 0.0 {
            "r"
        } else {
            "l"
        };
        if leap {
            let cands = self.leap_candidates(lib, dir, input);
            if self.take_first(lib, level, root, rig, base, cr, cands) {
                return true;
            }
            // Leaps start with the hands level: bring them level first.
            if !is_rest(&self.state) {
                for d in ["u", "d"] {
                    let back: Vec<_> = self.candidates(lib, d, Vec2::ZERO).into_iter().filter(|c| is_rest(&c.to)).collect();
                    if self.take_first(lib, level, root, rig, base, cr, back) {
                        return true;
                    }
                }
            }
            // Nothing to leap to (at the top, or the end of the holds): the plain moves, so free running
            // up a wall still climbs over its top.
        }
        if !vertical && self.try_onto_ladder(lib, level, root, rig, base, cr, dir) {
            return true;
        }
        let cands = self.candidates(lib, dir, input);
        if self.take_first(lib, level, root, rig, base, cr, cands) {
            return true;
        }
        if dir == "u" && self.try_top_out(lib, level, root, rig, base, cr) {
            return true;
        }
        // Nothing within a plain move's reach that way: jump to the next hold (AC1's climbing jumps, as with the
        // legs held; a city's holds are further apart than one step).
        if !leap && is_rest(&self.state) {
            let cands = self.leap_candidates(lib, dir, input);
            if self.take_first(lib, level, root, rig, base, cr, cands) {
                debug!("climb: out of reach: jump to the next hold");
                return true;
            }
        }
        // Up with nothing above and a hand raised (at the top edge): bring the hands level first, the
        // top-out follows on the next press.
        if dir == "u" && !is_rest(&self.state) && !is_free(&self.state) {
            for d in ["u", "d", "l", "r"] {
                let back: Vec<_> = self.candidates(lib, d, Vec2::ZERO).into_iter().filter(|c| is_rest(&c.to)).collect();
                if self.take_first(lib, level, root, rig, base, cr, back) {
                    return true;
                }
            }
        }
        // Down at the bottom of the wall: climb off onto the ground.
        if dir == "d" && self.try_step_off(lib, level, root) {
            return true;
        }
        // At the bottom, sideways with no holds that way: step off sideways onto the ground.
        if (dir == "l" || dir == "r") && self.try_step_off_side(lib, level, root, dir == "l") {
            return true;
        }
        // Sideways but stuck (end of the holds): settle back to hands level, from where corners start.
        if !vertical && !is_rest(&self.state) {
            for d in [dir, "d", "u"] {
                let back: Vec<_> = self.candidates(lib, d, Vec2::ZERO).into_iter().filter(|c| is_rest(&c.to)).collect();
                if self.take_first(lib, level, root, rig, base, cr, back) {
                    return true;
                }
            }
        }
        false
    }

    /// Mid-air catch: a wall ahead with holds where the catch clip puts the hands. The root turns
    /// to face the wall and is pulled onto the holds over the catch.
    #[allow(clippy::too_many_arguments)]
    fn try_catch(&mut self, lib: &mut AnimLib, level: &Level, root: &mut Transform, rig: &Rig, base: &Pose, cr: ClimbRig, down_speed: f32) -> bool {
        let fwd = (root.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
        // Ahead, or where the stick points (the directional grab: a wall to the side is reached for and caught, turning
        // to it), not behind.
        let stick = self.move_dir.with_y(0.0).normalize_or_zero();
        let side = (stick != Vec3::ZERO && stick.dot(fwd) < CATCH_SIDE_COS && stick.dot(fwd) > CATCH_BEHIND_COS).then_some(stick);
        // (At chest height, or lower: caught at a roof's edge, the chest is level with the roof.)
        let wall = |dir: Vec3, reach: f32, square: f32| {
            [1.2, 0.7]
                .iter()
                .find_map(|h| level.raycast(root.translation + Vec3::Y * *h, dir, reach).filter(|h| h.normal.y.abs() < 0.3 && h.normal.dot(dir) < square))
        };
        // (To the side, a wall met at most 60 degrees off square: one grazed along is not turned to.)
        let Some(hit) = wall(fwd, 1.0, 1.0).or_else(|| side.and_then(|d| wall(d, CATCH_SIDE_REACH, -0.5))) else {
            return false;
        };
        let normal = hit.normal.with_y(0.0).normalize();
        let facing = Transform { translation: root.translation, rotation: Quat::from_rotation_arc(Vec3::NEG_Z, -normal), ..default() };
        let speed = if down_speed > CATCH_FAST { "max" } else { "min" };
        let catches = [
            (vec![format!("xx_fall_tr_climb_{speed}_a"), format!("xx_fall_tr_climb_{speed}_b")], "2m", Feet::Wall),
            (vec!["xx_fall_tr_hangfree_min_a".to_string(), "xx_fall_tr_hangfree_min_b".to_string()], FREE, Feet::Free),
        ];
        for (names, to, feet) in catches {
            let Some(clips) = names.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>() else { continue };
            let Some(end) = chain_end(&clips, rig, base, cr) else { continue };
            let mut end = end.world(&facing);
            // (Pushed off the wall or falling clear of it: the hands are judged as if at the wall, where the catch
            // pulls the root; only along the wall and in height must they be near a hold.)
            let out = (((end.hands[0] + end.hands[1]) * 0.5 - hit.point).dot(normal) - CATCH_HAND_OUT).max(0.0);
            end.hands = end.hands.map(|h| h - normal * out);
            end.feet = end.feet.map(|f| f - normal * out);
            let Some(targets) = holds_within(level, &end.hands, normal, CATCH_REACH) else { continue };
            if self.catch_below.is_some_and(|y| (targets[0].y + targets[1].y) * 0.5 > y) {
                continue;
            }
            let err = (targets[0] - end.hands[0] + targets[1] - end.hands[1]) * 0.5;
            if !feet_fit(level, &end.feet.map(|f| f + err), normal, feet) {
                continue;
            }
            root.rotation = facing.rotation;
            self.normal = normal;
            self.fall_v = None;
            self.state = LEAP.into();
            let tos = chain_states(clips.len(), LEAP, to);
            // (The pull in to the wall too: caught further out, he would hang that far off it from then on.)
            self.start_chain(clips, tos, root, err - normal * out);
            return true;
        }
        false
    }

    /// Airborne: gravity moves the root, walls stop it sideways, the clip plays out (or holds); catch
    /// a hold after a jump, or land on touchdown.
    #[allow(clippy::too_many_arguments)]
    fn fall(&mut self, lib: &mut AnimLib, level: &Level, root: &mut Transform, rig: &Rig, base: &Pose, cr: ClimbRig, dt: f32) {
        if let Some((_, t)) = &mut self.skip_bar {
            *t -= dt;
            if *t <= 0.0 {
                self.skip_bar = None;
            }
        }
        if self.can_catch && self.try_bar(lib, level, root) {
            return;
        }
        if self.can_catch && !self.aimed && !self.faith {
            self.aim_at_perch(level, root);
        }
        if let Some(v) = self.fall_v
            && (self.can_catch || (self.grab_on_legs && self.legs_held))
            && v.y < 1.0
            && self.try_catch(lib, level, root, rig, base, cr, -v.y)
        {
            self.can_catch = false;
            return;
        }
        self.fall_top = self.fall_top.max(root.translation.y);
        let Some(v) = &mut self.fall_v else { return };
        v.y -= GRAVITY * dt;
        let flat = v.with_y(0.0);
        // Walls stop the flight, at the chest or the knees (a low jump must not slide into a block).
        let wall = |h: f32| level.raycast(root.translation + Vec3::Y * h, flat.normalize(), flat.length() * dt + 0.35).is_some_and(|h| h.normal.y.abs() < 0.5);
        // An edge at the knees only, its top just above the feet (the next roof, a parapet): the feet catch it and
        // the flight goes on over it, landing there.
        let ahead = flat.normalize_or_zero() * (flat.length() * dt + 0.35);
        let top_at_knees = (flat.length() > 0.01 && !wall(1.0) && wall(0.3))
            .then(|| level.ground(root.translation + ahead + Vec3::Y * 1.0, 0.0, 1.0))
            .flatten()
            .filter(|g| g.normal.y > 0.7 && (0.0..KNEE_TOP).contains(&(g.point.y - root.translation.y)));
        if let Some(g) = top_at_knees {
            root.translation.y = g.point.y;
            v.y = v.y.max(0.0);
        } else if flat.length() > 0.01 && (wall(1.0) || wall(0.3)) {
            if why() {
                let hit =
                    [1.0, 0.3].map(|h| level.raycast(root.translation + Vec3::Y * h, flat.normalize(), flat.length() * dt + 0.35).map(|w| (w.point, w.normal)));
                debug!("climb: a wall stops the flight at {:.2} ({hit:.2?})", root.translation);
            }
            v.x = 0.0;
            v.z = 0.0;
        }
        // A ceiling over the head stops the rise (AC1's proxy against a ceiling).
        if v.y > 0.0 && level.ceiling(root.translation, v.y * dt) {
            debug!("climb: a ceiling stops the rise at {:.2}", root.translation);
            v.y = 0.0;
        }
        let v = *v;
        let step = v * dt;
        root.translation += step;
        if v.y < 0.0
            && let Some(clip) = self.descend.take()
        {
            // Over the top: the falling pose (its root motion is ignored, gravity moves the root).
            self.start(clip, FALL.into(), root);
        }
        if let Some(m) = &mut self.mv {
            let dur = m.clip.anim.duration;
            m.t = if m.clip.name == FALL_LOOP { (m.t + dt) % dur.max(1e-3) } else { (m.t + dt * m.rate).min(dur) };
            // Clips that turn in the air (rebounds) turn the root.
            root.rotation = m.start_rot * root_delta(root_rotation_at(&m.clip, m.t * FPS));
        }
        // A fall that outlasts its clip goes on in the falling loop (not the leap of faith's dive).
        if !self.faith
            && self.descend.is_none()
            && self.mv.as_ref().is_some_and(|m| m.clip.name != FALL_LOOP && m.rate > 0.0 && m.t >= m.clip.anim.duration)
            && let Some(fall_loop) = lib.get(FALL_LOOP)
        {
            self.start(fall_loop, FALL.into(), root);
            self.fade = self.last.clone().map(|p| (p, ENTER_FADE, ENTER_FADE));
        }
        if v.y < 0.0 && self.try_hay(lib, level, root, step.y) {
            return;
        }
        // Ground crossed this frame on the way down (searching from a little above where we were).
        let ground = if v.y <= 0.0 { level.ground(root.translation, 0.3 - step.y, 0.0) } else { None };
        let Some(ground) = ground else {
            if root.translation.y < -100.0 {
                self.fall_v = None;
                self.finished = true;
            }
            return;
        };
        root.translation.y = ground.point.y;
        self.fall_v = None;
        let (flat_speed, down_speed) = (v.with_y(0.0).length(), -v.y);
        // (Running means going the way he faces: flying backward off a rebound lands standing, not rolling on.)
        let fwd = (root.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
        let running = flat_speed > RUN_LANDING_SPEED && v.with_y(0.0).dot(fwd) > 0.0;
        let drop = std::mem::replace(&mut self.fall_top, f32::NEG_INFINITY) - ground.point.y;
        let damage = landing_damage(drop, self.health);
        debug!("climb: landed from {drop:.1} m, damage {damage:.2}");
        self.landed = Some((drop, damage));
        if damage >= self.health
            && let Some(death) = lib.get(LAND_DEATH)
        {
            self.damage = self.health;
            self.dead = true;
            self.queue.clear();
            self.start(death, DEAD.into(), root);
            return;
        }
        self.damage = damage;
        // Onto a post or beam: balance on it.
        if damage == 0.0
            && let Some((i, q)) = level.perch_inside(ground.point, PERCH_REACH)
            && let (Some(impact), Some(recover)) = (lib.get(PERCH_LAND[0]), lib.get(PERCH_LAND[1]))
        {
            self.perch = Some(i);
            self.queue = vec![Queued::new(recover, PERCH)];
            let upright = Transform { rotation: facing(root.rotation * Vec3::NEG_Z), ..*root };
            self.start(impact, PERCH.into(), &upright);
            if let Some(m) = &mut self.mv {
                m.correct = (q - root.translation).with_y(0.0);
            }
            return;
        }
        if running && damage == 0.0 && drop < FLOW_LANDING_DROP {
            self.exit_velocity = v.with_y(0.0).normalize() * flat_speed.min(MAX_EXIT_SPEED);
            self.state = GROUND.into();
            self.finished = true;
            return;
        }
        // AC1's choice (`HumanInAir__SetupToGround_Landing` 0xE05940, docs/PARKOUR.md): moving with the stick within 75
        // degrees of the motion, the forward landing, going on by the speed against a sprint (< 0.2 the wait, < 0.5
        // the walk, < 0.9 the jog, else the sprint's takeoff); no stick, or turned further, the straight one (into the
        // wait with the stick let go, else as the profile asks).
        let stick = self.move_dir.with_y(0.0);
        let along = v.with_y(0.0).try_normalize().unwrap_or((root.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero());
        let forward = stick.length() > 0.3 && stick.normalize().dot(along) >= LAND_FORWARD_COS;
        let into = if forward {
            match flat_speed / LAND_SPRINT_SPEED {
                r if r < 0.2 => LandInto::Wait,
                r if r < 0.5 => LandInto::Walk,
                r if r < 0.9 => LandInto::Jog,
                _ => LandInto::Sprint,
            }
        } else {
            LandInto::from(stick.length() > 0.3, self.high, self.sprint)
        };
        let names: [String; 2] = match running {
            _ if damage >= HEAVY_DAMAGE => LAND_HEAVY.map(String::from),
            true if damage > 0.0 || drop > ROLL_LANDING_DROP => LAND_DAMAGE_RUN.map(String::from),
            _ if damage > 0.0 => {
                // (AC1's damaging landing goes on into the wait, the walk or the jog.)
                let after = match into {
                    LandInto::Wait => "h_wait_footr",
                    LandInto::Walk => "h_walk_footl",
                    LandInto::Jog | LandInto::Sprint => "h_jog_footl",
                };
                [LAND_DAMAGE.to_string(), format!("{LAND_DAMAGE}_tr_{after}")]
            }
            _ => landing_names(forward, down_speed > HARD_LANDING_SPEED, into),
        };
        debug!("climb: landing into {into:?} ({})", names[0]);
        // On the way it faces: at the run's speed when running in, else at the speed of what it goes into.
        let fwd = (root.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
        self.exit_velocity = match into {
            LandInto::Wait => Vec3::ZERO,
            _ if running => v.with_y(0.0).normalize() * flat_speed.min(MAX_EXIT_SPEED),
            _ => fwd * into.speed(),
        };
        match (lib.get(&names[0]), lib.get(&names[1])) {
            (Some(impact), Some(recover)) => {
                self.queue = vec![Queued::new(recover, GROUND)];
                self.start(impact, LAND.into(), root);
            }
            _ => {
                self.state = GROUND.into();
                self.finished = true;
            }
        }
    }

    /// Advance clips and move the root along the current move's root motion and rotation. `leap`
    /// is the held leap modifier.
    #[allow(clippy::too_many_arguments)]
    pub fn update(&mut self, lib: &mut AnimLib, level: &Level, root: &mut Transform, rig: &Rig, base: &Pose, cr: ClimbRig, input: Vec2, leap: bool, dt: f32) {
        self.legs_held = leap;
        // A steered move (a turn round) follows the stick: the facing it will end on is bent toward where the stick
        // points now, at up to `STEER_RATE` on top of the clips' own turn, so the camera and the stick stay free
        // through it. (The move's start is swung round the root too, so the root does not slide.)
        if self.steer.is_some()
            && self.move_dir.length() > 0.1
            && let Some(m) = &mut self.mv
        {
            let end = root_rotation_at(&m.clip, m.clip.frames());
            let mut planned = m.start_rot * root_delta(end);
            for q in &self.queue {
                planned *= root_delta(root_rotation_at(&q.clip, q.clip.frames()));
            }
            let ends = (planned * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
            let want = self.move_dir.with_y(0.0).normalize_or_zero();
            let err = ends.cross(want).y.atan2(ends.dot(want));
            let step = err.clamp(-STEER_RATE * dt, STEER_RATE * dt);
            if step.abs() > 1e-4 {
                let q = Quat::from_rotation_y(step);
                m.start = root.translation - q * (root.translation - m.start);
                m.start_rot = q * m.start_rot;
            }
            self.steer = Some(want);
            // Running (high profile) and nearly round: break off into the run now, AC1's turn going straight into its
            // jog, run or sprint (the rest of the turn is the gait's to finish).
            let left = root_delta(end) * root_delta(root_rotation_at(&m.clip, m.t * FPS)).inverse();
            if self.high
                && self.queue.is_empty()
                && m.clip.name.contains("_waitturn_")
                && (err - step).abs() < TURN_BREAK
                && left.to_euler(EulerRot::YXZ).0.abs() < TURN_BREAK
            {
                debug!("climb: turn broken off into the run");
                self.last_clip = Some(m.clip.name.clone());
                self.mv = None;
                self.steer = None;
                self.state = GROUND.into();
                self.finished = true;
                return;
            }
        }
        // Free running through a swing bar (the legs held, pushing on): fling off at the next forward swing, as
        // AC1 swings through without a press.
        if self.bar.is_some() && self.swing_off.is_none() && leap && self.move_dir.dot(root.rotation * Vec3::NEG_Z) > 0.3 {
            self.swing_off = Some(true);
        }
        // A move starting: fade into it over its item's blend time in AC1's move graph, where it gives one.
        if let Some(m) = &self.mv
            && self.blended_for.as_deref() != Some(m.clip.name.as_str())
        {
            self.blended_for = Some(m.clip.name.clone());
            if let (Some(b), Some(f)) = (lib.graph.blend_in(&m.clip.name).filter(|b| *b > 0.0), &mut self.fade) {
                f.1 = b;
                f.2 = b;
            }
        }
        if let Some((_, t, _)) = &mut self.fade {
            *t -= dt;
            if *t <= 0.0 {
                self.fade = None;
            }
        }
        if self.fall_v.is_some() {
            self.fall(lib, level, root, rig, base, cr, dt);
            return;
        }
        if self.side_run.is_some() {
            self.side_run_step(lib, level, root, dt);
            return;
        }
        if self.cycle.is_some() {
            self.beam_step(lib, level, root, dt);
            return;
        }
        if let Some(m) = &mut self.mv {
            m.t += dt * m.rate;
            let dur = m.clip.anim.duration;
            let frame = m.t.min(dur) * FPS;
            let late = late_correction(&m.clip.name);
            let at = |m: &Move| {
                let k = (m.t / dur.max(1e-3)).min(1.0);
                m.start + world_rot(m.start_rot) * root_motion_at(&m.clip, frame) + m.correct * if late { k * k * k } else { k }
            };
            let mut next = at(m);
            if matches!(m.to.as_str(), JUMP | LAND | GROUND | GROUND_ACT | HAY_OUT) {
                // Sliding along the ground (takeoff, landing): stop at walls, for the rest of the clip too; a
                // ground move (a stop, a turn round, a landing) stops at an edge too rather than walk off it.
                let slide = (next - root.translation).with_y(0.0);
                let ledge = m.to != JUMP
                    && level.ground(root.translation, 0.3, 0.3).is_some()
                    && level.ground(next + slide.normalize_or_zero() * 0.25, 0.3, GROUND_MOVE_DROP).is_none();
                if slide.length() > 1e-4 && (blocked(level, root.translation, slide) || ledge) {
                    m.start -= slide;
                    next = at(m);
                }
            }
            root.translation = next;
            if let Some(vp) = &mut self.vault_path {
                vp.t += dt;
                root.translation = vp.at(vp.t);
                if vp.t >= vp.dur {
                    self.vault_path = None;
                }
            }
            root.rotation = m.start_rot * root_delta(root_rotation_at(&m.clip, frame));
            if m.ease_rot != Quat::IDENTITY {
                let k = smoothstep(m.t / EASE_TURN.min(dur).max(1e-3));
                root.rotation = m.ease_rot.slerp(Quat::IDENTITY, k) * root.rotation;
            }
            match m.to.as_str() {
                // Pushing off near the ground: don't sink into it.
                DROP => {
                    if let Some(g) = level.ground(root.translation, 0.5, 0.5) {
                        root.translation.y = root.translation.y.max(g.point.y);
                    }
                }
                // Landing and getting up (a running landing carries on forward), a run stop: follow the ground, kept
                // for the rest of the move (up a ramp the clip's flat path would end inside it).
                LAND | GROUND => {
                    if let Some(g) = level.ground(root.translation, 0.5, 0.6) {
                        m.start.y += g.point.y - root.translation.y;
                        root.translation.y = g.point.y;
                    }
                }
                _ => {}
            }
            let end = self.cut.as_ref().filter(|(n, _)| *n == m.clip.name).map_or(dur, |(_, t)| t.min(dur));
            if m.t < end {
                return;
            }
            let done = self.mv.take().expect("move in progress");
            self.last_clip = Some(done.clip.name.clone());
            // Move done: the wall normal turns with the root.
            self.normal = (root.rotation * done.start_rot.inverse()) * self.normal;
            self.state = done.to;
            if !self.queue.is_empty() {
                let mut next = self.queue.remove(0);
                // Topping out with the stick pushed on: up off the knee straight into walking (or, free running, jogging),
                // AC1's `xx_h_hangknee_foot?_tr_<l_walk|h_jog>_foot?_a/b`, rather than standing up first.
                let on = self.move_dir.with_y(0.0).normalize_or_zero().dot((root.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero()) > 0.5;
                if on && next.clip.name.starts_with("xx_h_hangknee_foot") && next.clip.name.contains("_tr_h_wait_") {
                    let gait = if self.sprint { "_tr_h_jog_" } else { "_tr_l_walk_" };
                    let stem = next.clip.name.trim_end_matches("_a").replace("_tr_h_wait_", gait);
                    if let (Some(a), Some(b)) = (lib.get(&format!("{stem}_a")), lib.get(&format!("{stem}_b"))) {
                        debug!("climb: topping out on into {}", stem.rsplit("_tr_").next().unwrap_or(""));
                        let to = std::mem::replace(&mut next.to, GROUND_ACT.into());
                        // (What came after the stand, the rest of its chain, is the gait's now.)
                        self.queue.retain(|q| !q.clip.name.contains("_tr_h_wait_"));
                        self.queue.insert(0, Queued { to, ..Queued::new(b, GROUND) });
                        next.clip = a;
                        self.exit_velocity =
                            (root.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero() * if self.sprint { TOP_OUT_JOG } else { TOP_OUT_WALK };
                    }
                }
                // (A step AC1's move graph does not have: logged, to find chains built wrong.)
                if !lib.graph.allows(&done.clip.name, &next.clip.name) {
                    debug!("graph: {} -> {} is not in AC1's move graph", done.clip.name, next.clip.name);
                }
                if next.to == FALL {
                    // Keep the release's (or takeoff's) speed; gravity takes over from here.
                    let end = done.clip.frames();
                    let k = end.min(3.0);
                    let v = if k > 0.0 {
                        world_rot(done.start_rot) * (root_motion_at(&done.clip, end) - root_motion_at(&done.clip, end - k)) * (FPS / k)
                    } else {
                        Vec3::ZERO
                    };
                    // (Takeoff clips leave faster than the run; jumps carry the run's speed at most.)
                    let v = v.with_y(0.0).clamp_length_max(MAX_JUMP_SPEED).with_y(v.y.min(0.0) + std::mem::take(&mut self.launch));
                    self.fall_v = Some(self.fall_with.take().unwrap_or(v));
                    self.state = FALL.into();
                }
                debug!("climb: {} -> {} via {} at {:.2}", self.state, next.to, next.clip.name, root.translation);
                // Crossfade the seam by how far the hands and feet jump (AC1 chains do not all line up).
                if let Some(last) = &self.last {
                    let mut next_pose = base.clone();
                    sample(&next.clip, 0.0, &mut next_pose, cr.reference);
                    let gap = seam_gap(rig, cr, last, &next_pose);
                    let len = (gap * 0.3).clamp(0.05, 0.25);
                    self.fade = Some((last.clone(), len, len));
                }
                self.mv = Some(Move {
                    clip: next.clip,
                    t: 0.0,
                    to: next.to,
                    start: root.translation,
                    start_rot: root.rotation,
                    correct: next.correct,
                    rate: next.rate,
                    ease_rot: Quat::IDENTITY,
                });
                return;
            }
            if self.state == ON_TOP || self.state == GROUND {
                self.finished = true;
                return;
            }
            if self.state == SWING && self.swing_next(lib, level, root, &done.clip) {
                return;
            }
            if self.state == MONKEY && self.monkey_next(lib, level, root, &done.clip) {
                return;
            }
            // Balancing on a post, leaning, hiding: their loops start in another pose than the move
            // ends in (a one-hand pull-up ends stepping up), so crossfade into them.
            if matches!(self.state.as_str(), PERCH | LEAN | COLLIDE | HAY | BENCH)
                && let Some(last) = &self.last
            {
                self.fade = Some((last.clone(), WAIT_FADE, WAIT_FADE));
            }
            self.settle_hang(lib, level, root, rig, base, cr);
            self.set_wait(lib, done.clip);
            // Into the loop: crossfade by how far its first frame is from where the move ended (a seam).
            if self.fade.is_none()
                && let (Some(last), Some(w)) = (&self.last, &self.wait)
            {
                let mut first = base.clone();
                sample(w, 0.0, &mut first, cr.reference);
                let gap = seam_gap(rig, cr, last, &first);
                if gap > 0.03 {
                    let len = (gap * 0.4).clamp(0.08, 0.3);
                    self.fade = Some((last.clone(), len, len));
                }
            }
            if std::mem::take(&mut self.want_drop) && self.drop_now(lib, level, root) {
                return;
            }
        }
        if let Some(w) = &self.wait
            && !self.wait_hold
        {
            self.wait_t = (self.wait_t + dt) % w.anim.duration.max(1e-3);
        }
        if self.dead {
            return;
        }
        if self.state == PERCH {
            if self.mv.is_none() && std::mem::take(&mut self.want_drop) {
                if let Some(w) = Self::post_pull_down(lib, level, root, rig, base, cr, self.move_dir, self.last.clone()) {
                    *self = w;
                    return;
                }
                let dir = if self.move_dir.length() > 0.3 { self.move_dir } else { root.rotation * Vec3::NEG_Z };
                self.perch_jump(lib, level, root, dir.with_y(0.0).normalize_or(Vec3::NEG_Z));
                return;
            }
            self.perch_step(lib, level, root, dt);
            return;
        }
        if self.state == LEAN {
            self.lean_step(lib, root);
            return;
        }
        if self.state == COLLIDE {
            self.collide_step(lib, level, root, false);
            return;
        }
        if self.state == BENCH {
            self.bench_step(lib, root);
            return;
        }
        if self.ladder.is_some() {
            if input.x.abs() > input.y.abs() && input.x.abs() > 0.3 {
                self.off_ladder_side(lib, level, root, rig, base, cr, input.x);
            } else {
                self.ladder_step(lib, level, root, input);
            }
            return;
        }
        let moved = input.length() > 0.3 && !self.hop_out(lib, root) && self.try_move(lib, level, root, rig, base, cr, input, leap);
        self.look_around(lib, (!moved && input.length() > 0.3).then_some(input));
    }

    /// AC1's look round on a wall (`xx_l_climb_1m_lookaround_<side>`, `xx_h_hangwall_wait_lookaround_<side>`, actions
    /// with no move after them): a direction held where no move goes, he looks that way, faded in over `LOOK_FADE`; let
    /// go, back to the wait.
    fn look_around(&mut self, lib: &mut AnimLib, input: Option<Vec2>) {
        let set = match self.state.as_str() {
            "1m" => "xx_l_climb_1m_lookaround",
            HANGWALL | HANGWALL_OPEN => "xx_h_hangwall_wait_lookaround",
            _ => {
                self.looking = None;
                return;
            }
        };
        if self.mv.is_some() {
            self.looking = None;
            return;
        }
        let side = input.map(|i| match (i.y.abs() >= i.x.abs(), i.y > 0.0, i.x > 0.0) {
            (true, true, _) => "up",
            (true, false, _) => "down",
            (false, _, true) => "right",
            (false, _, false) => "left",
        });
        if side == self.looking {
            return;
        }
        let (Some(last), Some(now)) = (self.last.clone(), self.wait.clone()) else { return };
        match side {
            Some(s) => {
                let Some(look) = lib.get(&format!("{set}_{s}")) else { return };
                self.wait = Some(look);
                self.wait_hold = false;
                self.wait_t = 0.0;
            }
            None => self.set_wait(lib, now),
        }
        debug!("climb: looking {}", side.unwrap_or("back"));
        self.fade = Some((last, LOOK_FADE, LOOK_FADE));
        self.looking = side;
    }

    /// Body pose for this frame (before IK).
    pub fn pose(&mut self, base: &Pose, cr: ClimbRig, rig: &Rig, mirror: &ik::mirror::Mirror) -> Pose {
        let mut pose = base.clone();
        if let Some(sr) = &self.side_run {
            let clip = &sr.clips[sr.foot];
            sample(clip, sr.phase * clip.frames(), &mut pose, cr.reference);
        } else if let Some(cy) = &self.cycle {
            let clip = &cy.clips[cy.foot];
            sample(clip, cy.phase * clip.frames(), &mut pose, cr.reference);
        } else {
            match (&self.mv, &self.wait) {
                (Some(m), _) => {
                    sample(&m.clip, m.t.min(m.clip.anim.duration) * FPS, &mut pose, cr.reference);
                    if self.mirrored {
                        pose = mirror.apply(rig, &pose);
                    }
                }
                (None, Some(w)) => sample(w, self.wait_t * FPS, &mut pose, cr.reference),
                _ => {}
            }
        }
        if let Some((from, t, len)) = &self.fade {
            let w = smoothstep(1.0 - t / len);
            let mut faded = from.clone();
            faded.blend(&pose, w);
            // The root bone turns the body in some clips (a turn round's): blended the shortest way, it switched round
            // the other way as its gap to the faded-from pose passed half a turn, the body swinging in a frame. It goes
            // whichever way round stays nearest last frame's.
            if let (Some(r), Some(last)) = (cr.reference, &self.last) {
                let (a, b) = (from.local[r].rot, pose.local[r].rot);
                let (axis, angle) = (a.inverse() * b).to_axis_angle();
                let other = if angle > 0.0 { angle - std::f32::consts::TAU } else { angle + std::f32::consts::TAU };
                let way = |ang: f32| (a * Quat::from_axis_angle(axis, ang * w)).normalize();
                let near = |q: Quat| q.dot(last.local[r].rot).abs();
                let (short, long) = (way(angle), way(other));
                faded.local[r].rot = if near(long) > near(short) { long } else { short };
            }
            pose = faded;
        }
        self.last = Some(pose.clone());
        pose
    }

    /// True while the hands hang on holds (not while pulling up over the top, turning a corner,
    /// leaping or leaving the wall, where the clip alone is right).
    pub fn hands_on_wall(&self) -> bool {
        !off_wall(&self.state) && !self.mv.as_ref().is_some_and(|m| off_wall(&m.to) || m.clip.name.contains("corner"))
    }

    /// Last pose shown, to fade from when the climb ends.
    pub fn last_pose(&self) -> Option<&Pose> {
        self.last.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn landings_go_on_by_the_stick_and_profile() {
        assert_eq!(LandInto::from(false, true, true), LandInto::Wait);
        assert_eq!(LandInto::from(true, false, false), LandInto::Walk);
        assert_eq!(LandInto::from(true, true, false), LandInto::Jog);
        assert_eq!(LandInto::from(true, true, true), LandInto::Sprint);
        assert_eq!(landing_names(true, false, LandInto::Wait)[0], "xx_h_landing_forward_soft_footr_tr_h_wait_footr_a");
        assert_eq!(landing_names(false, true, LandInto::Sprint)[1], "xx_h_landing_straight_hard_footr_tr_h_sprint_impultion_footl_b");
        assert_eq!(landing_names(false, false, LandInto::Walk)[0], "xx_h_landing_straight_soft_footr_tr_l_walk_footl_a");
    }

    #[test]
    fn state_classes() {
        assert!(is_free(FREE) && is_free(FREE_OPEN) && is_free("free_1lu"));
        assert!(!is_free("1m") && !is_free(FALL));
        assert!(is_rest("1m") && is_rest(FREE_OPEN) && !is_rest("1lu") && !is_rest("free_1ru"));
        for s in [
            KNEEL,
            ON_TOP,
            STEP_DOWN,
            DROP,
            FALL,
            LAND,
            GROUND,
            LEAP,
            JUMP,
            WALL_RUN_STATE,
            FAITH,
            HAY,
            HAY_OUT,
            DEAD,
            SIDE_RUN,
            PERCH,
            SWING,
            VAULT,
            LADDER_L,
            LADDER_R,
            MONKEY,
        ] {
            assert!(off_wall(s), "{s}");
        }
        assert!(!off_wall("1m") && !off_wall(FREE));
    }

    #[test]
    fn ballistic_lands_on_target() {
        for (from, to) in
            [(Vec3::ZERO, Vec3::new(2.4, 0.0, 0.0)), (Vec3::new(1.0, 2.0, 0.0), Vec3::new(1.0, 0.5, 3.0)), (Vec3::ZERO, Vec3::new(-1.0, 1.0, 1.0))]
        {
            let (v, t) = ballistic(from, to);
            let end = from + v * t - Vec3::Y * 0.5 * GRAVITY * t * t;
            assert!((end - to).length() < 1e-4, "{end} vs {to}");
            assert!(t > 0.3);
        }
    }

    #[test]
    fn landing_damage_by_height() {
        assert_eq!(landing_damage(2.5, 1.0), 0.0);
        assert_eq!(landing_damage(5.0, 1.0), SMALL_DAMAGE);
        assert_eq!(landing_damage(6.5, 1.0), HEAVY_DAMAGE);
        assert_eq!(landing_damage(7.5, 0.7), 0.7);
    }

    #[test]
    fn leap_chain_states() {
        // Wind-up in place, flight, then the catch clips all end on the target.
        assert_eq!(chain_states(3, "1m", FREE), ["1m", LEAP, FREE]);
        assert_eq!(chain_states(5, "1m", "1m"), ["1m", LEAP, "1m", "1m", "1m"]);
        assert_eq!(chain_states(1, "1m", "2m"), ["2m"]);
    }

    #[test]
    fn ground_slides_stop_at_walls() {
        // Wall face at z = 0 facing -Z, 3 m tall.
        let tri = |a, b, c| crate::level::Tri { a, b, c };
        let (p0, p1, p2, p3) = (Vec3::new(-5.0, 0.0, 0.0), Vec3::new(5.0, 0.0, 0.0), Vec3::new(5.0, 3.0, 0.0), Vec3::new(-5.0, 3.0, 0.0));
        let level = Level { tris: vec![tri(p0, p1, p2), tri(p0, p2, p3)], ..default() };
        let root = Vec3::new(0.0, 0.0, -0.5);
        assert!(blocked(&level, root, Vec3::Z * 0.2));
        assert!(!blocked(&level, root, Vec3::NEG_Z * 0.2));
        assert!(!blocked(&level, root, Vec3::X * 0.2));
        assert!(!blocked(&level, Vec3::new(0.0, 0.0, -2.0), Vec3::Z * 0.2));
    }

    #[test]
    fn feet_need_wall_or_air() {
        // Wall face at z = 0 facing -Z, from y = 2 up (an overhang over open ground below).
        let n = Vec3::NEG_Z;
        let tri = |a, b, c| crate::level::Tri { a, b, c };
        let (p0, p1, p2, p3) = (Vec3::new(-5.0, 2.0, 0.0), Vec3::new(5.0, 2.0, 0.0), Vec3::new(5.0, 6.0, 0.0), Vec3::new(-5.0, 6.0, 0.0));
        let level = Level { tris: vec![tri(p0, p1, p2), tri(p0, p2, p3)], ..default() };
        let braced = [Vec3::new(-0.1, 3.0, -0.2), Vec3::new(0.1, 3.0, -0.2)];
        let dangling = [Vec3::new(-0.1, 1.0, -0.2), Vec3::new(0.1, 1.0, -0.2)];
        assert!(feet_fit(&level, &braced, n, Feet::Wall));
        assert!(!feet_fit(&level, &braced, n, Feet::Free));
        assert!(feet_fit(&level, &dangling, n, Feet::Free));
        assert!(!feet_fit(&level, &dangling, n, Feet::Wall));
    }
}
