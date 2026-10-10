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
//! (No side wall run: AC1 has the code for one, `WallingType_Horizontal`, but ships no clips and the game never runs
//! along walls. A procedural one built here was taken out, 2026-10-08: it ran through walls in Damascus.)
//!
//! Landing damage, by the height fallen (AC1's landing type, 0xE00FE0, its limits set by `HumanInAir`'s constructor
//! 0xE0FE80: safe = never, heavy over 6.3 m, fatal over 7.0 m): from 3 m the damage landing plays
//! (`xx_h_landing_damage_footl`) but costs nothing; over 6.3 m it hurts (AC1 takes 10 health points); over 7 m, or
//! when the damage would empty the health, `xx_h_landing_death_back` (200 points). Haystacks are always safe.
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
const TOP_OUT: [&str; 3] = ["xx_l_climb_1m_tr_hangknee_footl_a", "xx_l_climb_1m_tr_hangknee_footl_b", STAND_UP];
/// The same from the wide hang ("2m": mid-air catches, wall-run and jump-up grabs end there).
const TOP_OUT_2M: [&str; 3] = ["xx_l_climb_2m_tr_hangknee_footl_a", "xx_l_climb_2m_tr_hangknee_footl_b", STAND_UP];
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
/// The top beside the hands is looked for this far in from where the top was found (m).
const ONEHAND_PROBE_IN: f32 = 0.2;
/// The top drops away this far to a side of the hands (m, measured 0.15 m in from the edge): one hand.
const ONEHAND_SIDE: f32 = 0.35;
const TOP_OUT_FREE: [&str; 4] = ["xx_h_hangfree_tr_hangwaist_a", "xx_h_hangfree_tr_hangwaist_b", "xx_h_hangwaist_tr_hangknee_footl", STAND_UP];
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
const TOP_OUT_HANGWALL: [&str; 3] = ["xx_h_hangwall_tr_hangknee_footl_a", "xx_h_hangwall_tr_hangknee_footl_b", STAND_UP];
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

/// A top-out onto a floor at most this far under the grip (m): the hands hold a lip in front of it.
const TOP_OUT_LIP: f32 = 0.45;

/// AC1's ground landing (`HumanInAir`): `xx_h_landing_<forward|straight>_<soft|hard>_footr_tr_<into>`, `_a` the impact and
/// `_b` going on: forward when coming down moving, straight when dropping; into the wait on the same foot, or the walk,
/// jog or sprint's takeoff on the other.
/// `hard` (0-1) blends the soft and hard ones, as AC1 does by the drop (`hard_share`).
fn landing_names(forward: bool, hard: f32, into: LandInto) -> [String; 2] {
    let way = if forward { "forward" } else { "straight" };
    let after = match into {
        LandInto::Wait => "h_wait_footr",
        LandInto::Walk => "l_walk_footl",
        LandInto::Jog => "h_jog_footl",
        LandInto::Sprint => "h_sprint_impultion_footl",
    };
    let name = |force: &str, part: &str| format!("xx_h_landing_{way}_{force}_footr_tr_{after}_{part}");
    let hard = hard.clamp(0.0, 1.0);
    ["a", "b"].map(|part| match hard {
        h if h < 0.01 => name("soft", part),
        h if h > 0.99 => name("hard", part),
        h => mix_name(&[(name("soft", part).as_str(), 1.0 - h), (name("hard", part).as_str(), h)]),
    })
}
/// The hard landing's share for a drop from the apex (AC1's 0xE05940, as Banned445's port reads it): drop / 2.5 m.
fn hard_share(drop: f32) -> f32 {
    (drop / HARD_LANDING_DROP).clamp(0.0, 1.0)
}
const HARD_LANDING_DROP: f32 = 2.5;
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
/// Up off the knee onto the top: AC1's code always stands up through the free step's entry (`HumanLedge__Pullup_Tick`
/// 0xDE2EE0, as Banned445's port reads it; the stand into the wait is only listed in the move graph), 0.27 s in place,
/// as the running game did topping out at the bureau; ours' stand into the wait took 0.87 s.
const STAND_UP: &str = "xx_h_hangknee_footl_tr_freestep_entry_footl";
/// Topped out, the root stands this far in from where the hands held (m).
const STAND_FROM_WRISTS: f32 = 0.3;
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
/// Where a wall run's entry ends, facing `fwd` into the wall: `WALL_RUN_OUT` out from where a probe `WALL_RUN_PROBE` over
/// the feet meets it, `WALL_RUN_UP` over the feet (AC1's 0xE18390, its wall hit at 1.3·h).
fn wall_run_contact(level: &Level, root: &Transform, fwd: Vec3) -> Option<Vec3> {
    let hit = level.raycast(root.translation + Vec3::Y * WALL_RUN_PROBE, fwd, WALL_RUN_REACH + 0.5).filter(|h| h.normal.y.abs() < 0.3)?;
    let n = hit.normal.with_y(0.0).normalize_or_zero();
    Some((hit.point + n * WALL_RUN_OUT).with_y(root.translation.y + WALL_RUN_UP))
}

/// How far the hands of a jump up to a hold may miss it (the root is moved over the jump).
const JUMP_HANG_REACH: f32 = 0.45;
/// A wall run up with nothing to grab needs this much clear space above the root plus 1 m (m), this
/// far out from the wall (where the head goes). (The real game runs up under a Damascus overhang 3.51 m over the feet
/// there, its run topping out with the root 1.95 m up: 2.6 refused it.)
const WALL_RUN_HEADROOM: f32 = 2.4;
const WALL_RUN_HEADROOM_OUT: f32 = 0.35;
/// How far a wall run may start from where its first step should be (the approach is pulled in).
const WALL_RUN_SLACK: f32 = 0.9;
/// How far the hands may be from holds for a mid-air catch (m; the body is pulled in over the catch): AC1's hand box,
/// 0.4 m across and 0.3 m up or down (0xE0A990 / 0xE0AC70, Banned445).
const CATCH_ACROSS: f32 = 0.4;
const CATCH_UP: f32 = 0.3;
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
/// Free running, the dive in is taken this close to the haystack's edge (m): AC1 dived from the hiding spot's rim, 1.4 m
/// from its middle (the running game, Damascus); from further, the dive went through the box it stood on.
const HAY_RUN_DIVE_REACH: f32 = 0.6;
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
/// AC1's climbing grid (`BuildHoldGrid` 0xDF6A40): a hold found in a cell's probe box (0.375 m along, 0.3 m up or down)
/// is kept only within 0.3 m along the wall and 0.15 m up or down of the cell's centre (here where the move's clip puts
/// the hand), and up to 1.0 m in or out of the wall's plane (a storey set back, a sill sticking out). Strict as AC1's
/// since 2026-10-08: the Damascus probe (9 of 20 over the top) and every climbing scenario are the same as with the
/// looser 0.375 × 0.32 box.
const GRID_ALONG: f32 = 0.3;
const GRID_UP: f32 = 0.15;
const GRID_DEPTH: f32 = 1.0;
/// A plain move's hand that goes up or down more than this (m) must change holds (half AC1's 0.6 m row).
const GRID_ROW_MIN: f32 = 0.3;
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
/// How fast the wall run's kick off the top carries him away from the wall (m/s; `xx_h_rebound_footr_tr_fall`'s root).
const REBOUND_OFF_SPEED: f32 = 1.7;
/// And down (m/s; the same clip's 3 m in 0.47 s).
const REBOUND_DROP_SPEED: f32 = 6.4;
/// The wall run's top with nothing to grab and the legs held: the kick off backwards (AC1, seen in the running game).
const WALL_RUN_REBOUND: [&str; 3] =
    ["xx_h_wallingfront_step1_footr_tr_rebound_footr_a", "xx_h_wallingfront_step1_footr_tr_rebound_footr_b", "xx_h_rebound_footr_tr_fall"];
/// A wall run rebounds until this share of its fall back off the wall has played.
const REBOUND_LATE: f32 = 0.6;
/// A wall run's rebound to the side goes at most this far off straight back (rad, 89 degrees); with no target it flies
/// to `REBOUND_FAR` out and `REBOUND_FAR_DOWN` down (AC1's fallback).
const REBOUND_SIDE_MAX: f32 = 1.553_343;
const REBOUND_FAR: f32 = 7.0;
const REBOUND_FAR_DOWN: f32 = 3.0;
/// Sprinting at a wall closer than this (m) runs up it.
pub const WALL_RUN_REACH: f32 = 2.0;
/// A wall met up to this far off square (rad, 60°) is run up, turning to face it.
const WALL_RUN_ANGLE: f32 = 1.05;
/// Where a wall run's entry ends (m): this far out from the wall and this high over the feet, as AC1 warps it (0xE18390:
/// 0.5·h out, h up, h = 1 m for Altaïr; Banned445's reading, and the running game's runs up the bureau's walls end
/// 0.50 m out, 1.00 m up).
const WALL_RUN_OUT: f32 = 0.5;
const WALL_RUN_UP: f32 = 1.0;
const WALL_RUN_PROBE: f32 = 1.3;
/// A cap or ledge sticking out over a wall stops a wall run up it when this close to its line along the wall (m).
const WALL_RUN_CAP_SIDE: f32 = 0.2;
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
/// HeavyDamage / Fatal): the damage landing from 3 m (no health lost), heavy over 6.3 m, fatal over 7 m (the game's landing function, via
/// Banned445's AC1-Movement-Rewritten); and the share of health lost.
const SAFE_DROP: f32 = 3.0;
const HEAVY_DROP: f32 = 6.3;
const FATAL_DROP: f32 = 7.0;
/// (AC1's limit for small damage is the largest float: from 3 to 6.3 m the damage landing plays, no health is lost.)
const SMALL_DAMAGE: f32 = 0.0;
const HEAVY_DAMAGE: f32 = 0.5;
const LAND_DAMAGE: &str = "xx_h_landing_damage_footl";
/// Running into a damaging landing, or from a drop over `ROLL_LANDING_DROP`, rolls on (`xx_roll_hipm` recovery).
const LAND_DAMAGE_RUN: [&str; 2] = ["xx_h_landing_damage_footl_roll", "xx_roll_hipm_tr_h_jog_hipm_footr"];
const ROLL_LANDING_DROP: f32 = 3.0;
const LAND_HEAVY: [&str; 2] = ["xx_h_hurt_fall_balanced_front_short_500cm_landing", "xx_h_hurt_fall_balanced_front_short_500cm_landing_tr_h_wait_footr"];
const LAND_DEATH: &str = "xx_h_landing_death_back";
/// Pseudo-state: dead after a fall.
const DEAD: &str = "dead";

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
/// Leaning on a wall and pushing on this long (s), a low one is climbed onto, the root ending on the first top found this
/// far in from its face (m).
const LEAN_CLIMB_AFTER: f32 = 0.2;
/// Out of a haystack over its rim: a hold along the rim within this of where the stick's way meets it (m), a drop of
/// more than `HAY_OVER_DROP` past it, the passover ending this far in from the rim's edge.
const HAY_OVER_REACH: f32 = 0.6;
const HAY_OVER_DROP: f32 = 1.5;
const HAY_OVER_IN: f32 = 0.1;
/// Leaning, the stick within this of straight into the wall (cos 72 degrees) keeps pushing; further off it leaves.
const LEAN_PUSH_COS: f32 = 0.3;
const LEAN_CLIMB_IN: [f32; 4] = [0.3, 0.2, 0.12, 0.06];
/// Pulling down onto a ledge: the edge within this far ahead of the feet (m), dropping at least this much.
const PULL_DOWN_REACH: f32 = 0.9;
/// (AC1's pull-down guard, 0xD9D6C0: more than 2 m under the edge.)
const PULL_DOWN_DROP: f32 = 2.0;
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

/// Where a reception's step, past its fastest, slows to the run's `speed` (frame): handed over there, the run goes on
/// at its pace.
fn reception_cut(clip: &Clip, speed: f32) -> Option<f32> {
    let pace = |f: f32| (root_motion_at(clip, f) - root_motion_at(clip, f - 1.0)).with_z(0.0).length() * FPS;
    let peak = (1..=clip.frames() as usize).map(|f| f as f32).max_by(|a, b| pace(*a).total_cmp(&pace(*b))).unwrap_or(1.0);
    (peak as usize..=clip.frames() as usize).map(|f| f as f32).find(|&f| pace(f) <= speed)
}

/// A wall run's catch is a hold at least this high over the floor it started from (m): the feet 1.6 m up at the end of
/// its vertical step (the running game, Damascus) and AC1's 1 m (`H`) over them.
const WALL_RUN_CATCH_MIN: f32 = 2.6;

/// Walked off a beam's end, the next beam within this of it (m) and along it within 35 degrees is walked on.
const BEAM_CHAIN_GAP: f32 = 0.45;
const BEAM_CHAIN_COS: f32 = 0.82;

/// A beam this close to the jump's way (cos 30 degrees) is along it, not across.
const JUMP_ALONG_BEAM_COS: f32 = 0.866;
/// The way forward comes near a beam along it when it passes this close to the beam's line (m).
const JUMP_ALONG_BEAM_MEET: f32 = 0.6;

/// A jump's target needs this much room past it at the chest (m).
const JUMP_LAND_ROOM: f32 = 0.6;

/// A jump onto a roof ends its landing this far in from the edge (m): in the running game (Damascus, through ac1-hook)
/// the reception (`..._tr_freestep_entry`) starts 0.47 m short of the edge, over the gap, and its step ends on it.
const ROOF_EDGE_INSET: f32 = 0.05;

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
/// With no target, the free step off a beam goes when there is a floor this far on (m), at most `PERCH_FREE_STEP_DROP`
/// below.
const PERCH_FREE_STEP_ON: [f32; 3] = [2.0, 2.5, 3.0];
const PERCH_FREE_STEP_DROP: f32 = 1.2;
/// AC1's free step with no target: launched this fast on and up (m/s).
const FREE_STEP_SPEED: f32 = 5.4;
const FREE_STEP_UP: f32 = 1.0;
/// A back eject leaves the wall at this speed (m/s), and this fast upward.
const EJECT_SPEED: f32 = 4.0;
const EJECT_UP: f32 = 2.5;
/// Wall runs need a wall at least this wide (m).
const WALL_RUN_WIDTH: f32 = 0.7;
/// Jump targets: this far across (m), and within this cosine of the wanted direction (45 degrees).
/// A running jump's targets, from 1 m on, as far as AC1's reach zone goes (`in_jump_zone`).
const JUMP_TARGET_REACH: std::ops::RangeInclusive<f32> = 1.0..=8.0;
/// AC1's scorer's front plane: a target is in front when it is less than 0.5 m under the hips (0.5 m up) per metre
/// on × this (0.5 / 0.7).
const JUMP_FRONT_SLOPE: f32 = 0.5 / 0.7;
/// A top's far end within reach is landed on this far short of it (m).
const JUMP_FAR_END_SHORT: f32 = 0.2;
/// A running jump at a hold to catch, or over a thin wall: this far (m, flat).
const JUMP_HOLD_REACH: std::ops::RangeInclusive<f32> = 1.0..=4.6;
/// The deepest a jump's target is looked for below (m; the reach zone's floor).
const JUMP_TARGET_DEEP: f32 = 5.0;
/// An eject's targets, from nearer: the running game's eject to the right off the bureau's wall landed on a beam stuck out
/// of the wall 0.95 m away (ours went on 4 m to the next).
const EJECT_REACH: std::ops::RangeInclusive<f32> = 0.5..=4.6;
/// An eject's clips may land `JUMP_AC1_SLACK` of at least this far (m) off its target, made up over the jump.
const EJECT_SLACK_FROM: f32 = 1.4;
const JUMP_TARGET_CONE: f32 = 0.707;
/// ...and at most this far to either side of the wanted line (m): AC1's search box (0xE18970) is ±1 m across.
const JUMP_TARGET_ACROSS: f32 = 1.0;
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
/// A jump with no target comes down at this (m/s²) until falling at `FREE_JUMP_FAST` (m/s), then under gravity: the
/// running game's, measured off a Damascus roof (one jump: 20 m/s², about 8.5 m/s).
const FREE_JUMP_GRAVITY: f32 = 20.0;
const FREE_JUMP_FAST: f32 = 8.5;
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
/// A jump along a beam lands this far short of its far end (m).
const BEAM_FAR_END_SHORT: f32 = 0.27;
/// A free-step jump coming down this close to a beam (m, flat) lands on it.
const BEAM_ARRIVE: f32 = 0.55;
/// A jump onto a top: a step up past it within this (m) is more top, not something over it.
const STEP_PAST_UP: f32 = 0.6;
/// A shimmy along a hang needs the way clear at this height over the root (m, the chest) and this far past where it
/// ends (m).
const SHIMMY_CHEST: f32 = 0.45;
const SHIMMY_ROOM: f32 = 0.25;
/// A climb start from the ground reaches a wall this far in front of the root (m), running; standing, a little further.
const CLIMB_START_REACH: f32 = 1.0;
const CLIMB_START_STILL: f32 = 1.5;
/// A hold jumped at is at least this long (m).
const JUMP_HOLD_SHORT: f32 = 0.15;
const JUMP_HOLD_HANG: f32 = 0.9;
/// ...or this far under it for a free hang (the free-hang catch, `xx_fall_tr_hangfree_min`, ends with the hands 2.1 m
/// over the root).
const JUMP_HOLD_FREE_HANG: f32 = 2.1;
/// ...this far in from the hold's ends (m), for both hands.
const JUMP_HOLD_INSET: f32 = 0.35;
/// A top jumped to may be up to 3 m below and 1.3 m above (AC1's candidate scorer and its jump bands for a top, via
/// Banned445's AC1-Movement-Rewritten).
const JUMP_TARGET_RISE: std::ops::RangeInclusive<f32> = -3.0..=1.3;
const SWING_CYCLE: [&str; 4] = ["xx_h_swing_cycle_front_up", "xx_h_swing_cycle_front_down", "xx_h_swing_cycle_back_up", "xx_h_swing_cycle_back_down"];
const SWING_LAUNCH: &str = "xx_h_swing_cycle_front_300cm_to_air";
/// From the still hang on a bar, the stick forward swings up again.
const SWING_MOMENTUM: &str = "xx_h_swing_momentum_front_up";
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
/// The low obstacle is looked for this far either side of the run's middle too (m, the body's half-width).
const VAULT_SIDE: f32 = 0.3;
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
#[derive(Clone)]
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
    /// A start played first (AC1's `xx_<l|h>_beam_crouchwait_<foot>_tr_crouch<walk|jog>_<other foot>`), the time into
    /// it and its own speed; the cycle goes on from its end on the other foot.
    intro: Option<(Arc<Clip>, f32, f32)>,
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
    /// An eject off a wall hang waiting for the rebound pose before it to end (AC1's `xx_h_hangwall_tr_rebound_<foot>`):
    /// the stick and the foot it was asked with.
    eject_after: Option<(Vec2, bool)>,
    /// Going over a wall (a passover): the height of its top, which the fall after does not land on (with long frames
    /// the fall started over the top and came down on it, then jumped off it).
    clear_of: Option<f32>,
    /// Velocity while airborne (the root follows gravity, not the clip).
    fall_v: Option<Vec3>,
    /// Upward speed added when the next fall starts (running jump takeoff).
    launch: f32,
    /// A jump with no target (AC1's free jump): it comes down harder (`FREE_JUMP_GRAVITY`).
    free_jump: bool,
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
    /// How long it has leant on the wall, still (s).
    lean_t: f32,
    /// Going out of a haystack over its rim (AC1's passover): the rim's hold to hang from once over it.
    hay_over: Option<usize>,
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

/// Where a hand at `wrist` grips ledge `l`, and how far that is (`nearest_hold` for one ledge).
pub fn hold_on(l: &Ledge, wrist: Vec3) -> (Vec3, f32) {
    let target = grip_target(l.closest(wrist - l.out * GRIP_OUT + Vec3::Y * GRIP_DOWN), l.out);
    (target, (target - wrist).length())
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

/// From a hang sideways onto climbing holds (`TrySideMoveToClimbHolds` 0xDD48B0, `xx_h_hang<wall|free>_<d|u>?<l|r>_climb_1m`):
/// lower down, level, higher up, in that order.
fn side_to_climb(hang: &str, dir: &str) -> Vec<Cand> {
    ["d", "", "u"].iter().map(|v| Cand::new(format!("xx_h_{hang}_{v}{dir}_climb_1m"), "1m", Feet::Wall)).collect()
}

/// AC1's side entry onto a beam (0xF7AAA0, actions 0xE6E0E9B8..BB; Banned445's port): met at 30-100 degrees off the way
/// the stick points along it, a free-step entry turning onto it into the walk or jog,
/// `xx_h_freestep_entry_<foot>_tr_crouch<walk|jog>_<foot after>_<left|right>_<30|90>`, the 30 and 90 degree clips
/// blended by the angle.
fn beam_side_entry(lib: &mut AnimLib, line: &Line, root: &Transform, stick: Vec3, sprint: bool, lead_left: bool) -> Option<Arc<Clip>> {
    let axis = line.axis();
    let stick = stick.with_y(0.0).normalize_or_zero();
    if axis == Vec3::ZERO || stick == Vec3::ZERO || stick.dot(axis).abs() < std::f32::consts::FRAC_1_SQRT_2 {
        return None;
    }
    let way = axis * stick.dot(axis).signum();
    let fwd = (root.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
    let ang = fwd.dot(way).clamp(-1.0, 1.0).acos().to_degrees();
    if !(30.0..=100.0).contains(&ang) {
        return None;
    }
    let left = fwd.cross(way).y > 0.0;
    let (side, after) = if left { ("left", "footr") } else { ("right", "footl") };
    let foot = if lead_left { "footl" } else { "footr" };
    let gait = if sprint { "crouchjog" } else { "crouchwalk" };
    let w90 = ((ang - 30.0) / 60.0).clamp(0.0, 1.0);
    let name = |deg: u32| format!("xx_h_freestep_entry_{foot}_tr_{gait}_{after}_{side}_{deg}");
    let (a, b) = (name(30), name(90));
    lib.get(&mix_name(&[(a.as_str(), 1.0 - w90), (b.as_str(), w90)]))
}

/// Weighted clip names, to mix (`mix_name`).
type Parts = Vec<(String, f32)>;

/// A coarse direction ("u", "d", "l", "r") as a full stick (x right, y up).
fn dir_vector(dir: &str) -> Vec2 {
    match dir {
        "u" => Vec2::Y,
        "d" => Vec2::NEG_Y,
        "l" => Vec2::NEG_X,
        _ => Vec2::X,
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

/// A hold (or the ground) where AC1's grid puts the feet for a hand on `hand`: `FOOT_ROWS` under it, within a cell
/// (`GRID_ALONG` along the wall, `FOOT_CELL_UP` up or down, `GRID_DEPTH` deep).
fn foot_cell(level: &Level, hand: Vec3, normal: Vec3) -> bool {
    let along = Vec3::Y.cross(normal).normalize_or_zero();
    let at = hand - Vec3::Y * FOOT_ROWS;
    level.ground(at + normal * 0.3 + Vec3::Y * FOOT_CELL_UP, 0.0, 2.0 * FOOT_CELL_UP + 0.3).is_some()
        || level.ledges.iter().filter(|l| l.out.dot(normal) > 0.7).any(|l| {
            let d = l.closest(at) - at;
            d.dot(along).abs() <= GRID_ALONG && d.y.abs() <= FOOT_CELL_UP && d.dot(normal).abs() <= GRID_DEPTH
        })
}
/// The feet's cell is two of AC1's 0.6 m rows under the hand's; a hold within this of it (m, up or down) counts.
const FOOT_ROWS: f32 = 1.2;
const FOOT_CELL_UP: f32 = 0.3;

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
/// Where the hands are at the end of a hang option's chain (its rest pose), from `root` facing the wall (`normal`).
fn hang_hands(lib: &mut AnimLib, root: &Transform, rig: &Rig, base: &Pose, cr: ClimbRig, normal: Vec3, opt: &GrabOpt) -> Option<[Vec3; 2]> {
    let GrabEnd::Hang(_, rest) = opt.end else { return None };
    let clips = opt.names.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>()?;
    let facing = Transform { translation: root.translation, rotation: Quat::from_rotation_arc(Vec3::NEG_Z, -normal), ..default() };
    let end = chain_end(&clips, rig, base, cr)?.world(&facing);
    let rest = lib.get(rest)?;
    let mut pose = base.clone();
    sample(&rest, 0.0, &mut pose, cr.reference);
    let m = pose.model(rig);
    let r = world_rot(end.rot);
    Some(cr.hands.map(|b| end.pos + r * m[b].pos))
}

/// The wall hang options of the same move at two heights (the wall run's 251 and 430 cm catches) blended for the
/// lowest hold on the wall ahead between their reaches that a wall run catches (`WALL_RUN_CATCH_MIN` over the floor).
#[allow(clippy::too_many_arguments)]
fn blend_hang(lib: &mut AnimLib, level: &Level, root: &Transform, rig: &Rig, base: &Pose, cr: ClimbRig, normal: Vec3, opts: &[GrabOpt]) -> Vec<GrabOpt> {
    let shape = |n: &String| n.chars().filter(|c| !c.is_ascii_digit()).collect::<String>();
    let hangs: Vec<&GrabOpt> = opts.iter().filter(|o| matches!(o.end, GrabEnd::Hang(..))).collect();
    let mut out = vec![];
    for (i, a) in hangs.iter().enumerate() {
        for b in &hangs[i + 1..] {
            let same = a.names.len() == b.names.len() && a.catch == b.catch && a.names.iter().zip(&b.names).all(|(x, y)| shape(x) == shape(y));
            if !same || a.names == b.names {
                continue;
            }
            let (Some(ha), Some(hb)) = (hang_hands(lib, root, rig, base, cr, normal, a), hang_hands(lib, root, rig, base, cr, normal, b)) else { continue };
            let (ya, yb) = ((ha[0].y + ha[1].y) * 0.5, (hb[0].y + hb[1].y) * 0.5);
            let (lo, hi, low, high) = if ya < yb { (ya, yb, a, b) } else { (yb, ya, b, a) };
            let mid = (ha[0] + ha[1] + hb[0] + hb[1]) * 0.25;
            // The hold on this wall in front between the two reaches that AC1's probe D takes first: the nearest out from the
            // wall, then the lowest (`HumanWalling`'s FindLedge order, its distance ahead + 0.01 × its height, as Banned445's
            // port reads it). (On a Damascus street wall the running game caught an edge 0.1 m proud of the wall 3.8 m up,
            // over the wall's own edge 3.1 m up.)
            // (Each in that order: one the grab turns down leaves the next.)
            let key = |q: &Vec3| (*q - root.translation).dot(-normal) + 0.01 * q.y;
            let mut holds: Vec<Vec3> = level
                .ledges
                .iter()
                .filter(|l| l.out.dot(normal) > 0.8)
                .map(|l| l.closest(mid.with_y((l.a.y + l.b.y) * 0.5)))
                .filter(|q| (*q - mid).with_y(0.0).length() < 0.6 && q.y - root.translation.y >= WALL_RUN_CATCH_MIN && (lo..=hi).contains(&q.y))
                .collect();
            holds.sort_by(|a, b| key(a).total_cmp(&key(b)));
            for y in holds.iter().map(|q| q.y) {
                let w = (y - lo) / (hi - lo);
                let names = low.names.iter().zip(&high.names).map(|(x, z)| if x == z { x.clone() } else { mix_name(&[(x, 1.0 - w), (z, w)]) }).collect();
                out.push(GrabOpt { names, catch: low.catch, end: if w < 0.5 { low.end } else { high.end }, after: vec![] });
            }
        }
    }
    out
}

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
    // In the order the run reaches them, lowest first: AC1 catches the first hold on the way up (in the running game, Damascus,
    // the hold 2.5 m up rather than the one at 4.3 m above it).
    vec![
        GrabOpt { names: entry("xx_h_wallingfront_entry_footl_tr_hangknee_131cm_footl"), catch: 1, end: GrabEnd::Onto(1.31), after: stand_l.clone() },
        GrabOpt { names: entry("xx_h_wallingfront_entry_footl_tr_hangknee_200cm_footl"), catch: 1, end: GrabEnd::Onto(2.0), after: stand_l.clone() },
        GrabOpt { names: run(&[&format!("{step}_hangknee_201cm_footr")]), catch: 1, end: GrabEnd::Onto(2.0), after: stand_r.clone() },
        GrabOpt { names: run(&[&format!("{step}_hangknee_250cm_footr")]), catch: 1, end: GrabEnd::Onto(2.5), after: stand_r.clone() },
        GrabOpt { names: run(&[&format!("{step}_hangwall_251cm_000cm_a"), &format!("{step}_hangwall_251cm_000cm_b")]), catch: 2, end: wall, after: vec![] },
        GrabOpt { names: run(&[&format!("{step}_hangfree_350cm_swingback_min")]), catch: 1, end: free, after: vec![] },
        GrabOpt { names: run(&[&format!("{step}_hangfree_400cm_swingback_min")]), catch: 1, end: free, after: vec![] },
        GrabOpt { names: run(&[&format!("{step}_hangwall_430cm_000cm_a"), &format!("{step}_hangwall_430cm_000cm_b")]), catch: 2, end: wall, after: vec![] },
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
    // (The hang the jump ends in decides where the root goes, as AC1's ledge targets (`hang_root`, 0xDD6730): under a
    // hold with a wall below for the feet, a wall hang; under one over nothing (a ledge over an arch), a free hang, the
    // root much lower. Aimed as for a wall hang there, the hands came by a metre over the hold and he flew on.)
    jump_hold(level, from, dir).map(|(q, out)| {
        let wall = level.raycast(q - Vec3::Y * 1.0 + out * 0.5, -out, 0.5 + RECESS_MAX).is_some_and(|h| h.normal.dot(out) > 0.7);
        if wall { q + out * 0.4 - Vec3::Y * JUMP_HOLD_HANG } else { q + out * 0.1 - Vec3::Y * JUMP_HOLD_FREE_HANG }
    })
}

/// How AC1's jump-target finder (0xE18970, docs/PARKOUR.md section 6) sees a guidance edge at `p` facing `out`: probed
/// from 5 cm over it and 0.5 m out, the top's depth behind it (a sweep back 1.75 m), the drop in front of it (down 3.05
/// m) and, over a tall drop, whether a wall stands under it (from 1 m under, within 0.7 m).
struct Ac1Edge {
    depth: f32,
    drop: f32,
    wall_under: bool,
}

impl Ac1Edge {
    fn probe(level: &Level, p: Vec3, out: Vec3) -> Self {
        let q = p + out * 0.5 + Vec3::Y * 0.05;
        let depth = level.raycast(q, -out, 1.75).map_or(1.75, |h| h.dist) - 0.5;
        let drop = level.raycast(q, Vec3::NEG_Y, 3.05).map_or(f32::INFINITY, |h| h.dist - 0.05);
        let wall_under = drop >= 2.0 && level.raycast(p + out * 0.5 - Vec3::Y, -out, 1.4).is_some_and(|h| h.dist - 0.5 < 0.7);
        Ac1Edge { depth, drop, wall_under }
    }

    /// A free step onto its top (move bit 0x01): a top at least 0.3 m deep.
    fn step_onto(&self) -> bool {
        self.depth >= 0.3
    }

    /// A hang from it (0x40 wall hang, 0x80 hang): at least 2 m over what is in front with a wall under it, or 2.5 m.
    fn hang(&self) -> bool {
        (self.drop >= 2.0 && self.wall_under) || self.drop >= 2.5
    }
}

/// AC1's clear way to a jump target (0xE18970): no wall on the line from the chest (0.75 m over the feet) to `to`.
fn ac1_clear_way(level: &Level, from: Vec3, to: Vec3) -> bool {
    let chest = from + Vec3::Y * 0.75;
    let d = to - chest;
    level.raycast(chest, d.normalize_or_zero(), d.length()).is_none()
}

/// The hold a running jump along `dir` from `from` reaches for (see `jump_hold_target`): the point on it the hands go to,
/// and its outward normal. As AC1: facing the jumper within 45 degrees, with room to hang under it (`Ac1Edge::hang`) and
/// a clear way from the chest to the hang's chest (0.2 m out, 1.1 m under).
fn jump_hold(level: &Level, from: Vec3, dir: Vec3) -> Option<(Vec3, Vec3)> {
    let dir = dir.with_y(0.0).normalize_or_zero();
    level
        .ledges
        .iter()
        .filter(|l| l.out.dot(dir) <= -JUMP_TARGET_CONE)
        // (Short pieces too, aimed at their middle: AC1 grabs any edge, the hands 0.2 m either side; off the bureau's street
        // a tap jump caught a 0.22 m piece of the wall 2.4 m up.)
        .filter(|l| l.a.distance(l.b) > JUMP_HOLD_SHORT)
        .map(|l| {
            // (Both hands on it: in from its ends.)
            let along = (l.b - l.a).normalize();
            if l.a.distance(l.b) <= 2.0 * JUMP_HOLD_INSET {
                return ((l.a + l.b) * 0.5, l.out);
            }
            let inner = Line { a: l.a + along * JUMP_HOLD_INSET, b: l.b - along * JUMP_HOLD_INSET };
            (inner.closest(from + dir * 2.5), l.out)
        })
        .filter(|(q, _)| {
            let flat = (*q - from).with_y(0.0);
            JUMP_HOLD_REACH.contains(&flat.length()) && flat.normalize().dot(dir) >= JUMP_TARGET_CONE && JUMP_HOLD_RISE.contains(&(q.y - from.y))
        })
        .filter(|(q, out)| Ac1Edge::probe(level, *q, *out).hang() && ac1_clear_way(level, from, *q + *out * 0.2 - Vec3::Y * 1.1))
        .min_by(|a, b| (a.0 - from).length().total_cmp(&(b.0 - from).length()))
}

/// A thin wall ahead for a running jump to go over with a hand on its top (AC1's passover): its face met within reach,
/// its top `PASSOVER_RISE` over the feet (too high to land on), at most `PASSOVER_DEPTH` deep with a drop beyond it. The
/// point on the top's near edge, the face's normal and the top's depth.
fn passover_target(level: &Level, from: Vec3, dir: Vec3) -> Option<(Vec3, Vec3, f32)> {
    let dir = dir.with_y(0.0).normalize_or_zero();
    let hit = [0.4, 0.9].iter().find_map(|h| level.raycast(from + Vec3::Y * *h, dir, *JUMP_HOLD_REACH.end()).filter(|h| h.normal.y.abs() < 0.3))?;
    let normal = hit.normal.with_y(0.0).normalize_or_zero();
    if normal.dot(dir) > -JUMP_TARGET_CONE || hit.dist < *JUMP_HOLD_REACH.start() {
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
    // (Room over the top for the body going over, open above it: probed from inside a tall wall, an inner face read
    // as a top.)
    if level.raycast(hit.point.with_y(top.point.y + 0.4) + normal * 0.3, fwd, depth + 1.0).is_some()
        || level.raycast(top.point + Vec3::Y * 0.05, Vec3::Y, PASSOVER_RISE.end() + 0.3).is_some()
    {
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
/// 45 degrees of the wanted direction, at most 3 m down, the nearest of the free-step ones): a post or
/// beam (`skip`: the one stood on), or a walkable top across a gap.
fn jump_target(level: &Level, from: Vec3, dir: Vec3, skip: Option<usize>) -> Option<Vec3> {
    jump_target_within(level, from, dir, skip, JUMP_TARGET_REACH, true)
}

/// Inside AC1's reach zone 1 (`JumpZones` 0x1A2BF40, the jump's own zone, read live by Banned445): a side view, `dist` on
/// and `rise` up from the feet. Up to 1.3 m up to 3.5 m on, then lower the further: 0.8 m at 4.7, -0.5 at 6, -3 at 8;
/// down to 5 m below.
fn in_jump_zone(dist: f32, rise: f32) -> bool {
    const ZONE: [[f32; 2]; 7] = [[0.5, -5.0], [0.5, 1.3], [3.5, 1.3], [4.7, 0.8], [6.0, -0.5], [8.0, -3.0], [8.0, -5.0]];
    let mut inside = false;
    for i in 0..ZONE.len() {
        let (a, b) = (ZONE[i], ZONE[(i + 1) % ZONE.len()]);
        if (a[1] > rise) != (b[1] > rise) {
            let t = (rise - a[1]) / (b[1] - a[1]);
            if dist < a[0] + t * (b[0] - a[0]) {
                inside = !inside;
            }
        }
    }
    inside
}

/// `jump_target` with targets `reach` (m, flat) away: in AC1's reach zone 1 when `zone`, else `JUMP_TARGET_RISE` up.
fn jump_target_within(level: &Level, from: Vec3, dir: Vec3, skip: Option<usize>, reach: std::ops::RangeInclusive<f32>, zone: bool) -> Option<Vec3> {
    let dir = dir.with_y(0.0).normalize_or_zero();
    let mut cands: Vec<Vec3> = vec![];
    for (i, l) in level.perches.iter().enumerate() {
        if Some(i) != skip {
            // (A beam along the way, within 30 degrees, that the way forward never comes near is no target: AC1's
            // candidates are where the forward line crosses an edge. Off a roof beside a beam running on from it, a metre
            // off the way, the running game jumped with no target and came down on it 2.9 m on, hurt; ours aimed 2 m
            // along it.)
            let axis = l.axis();
            let apart = |q: Vec3| (q - l.closest(q)).with_y(0.0).length();
            if axis != Vec3::ZERO
                && axis.dot(dir).abs() > JUMP_ALONG_BEAM_COS
                && (2..=(JUMP_TARGET_REACH.end() / 0.25) as usize).all(|k| apart(from + dir * (k as f32 * 0.25)) > JUMP_ALONG_BEAM_MEET)
            {
                continue;
            } else if axis != Vec3::ZERO && axis.dot(dir).abs() <= JUMP_ALONG_BEAM_COS {
                // (A beam across the way: where the way forward crosses it, as AC1's candidates are. Off a roof's edge
                // across a diagonal beam crossing 0.2 m past the edge, too near, the running game made its free jump, 7.8 m
                // on onto a lower roof; ours aimed at the beam 0.6 m off the way and walked along it.)
                let (u, w) = ((l.b - l.a).with_y(0.0), from - l.a);
                let den = dir.x * u.z - dir.z * u.x;
                if den.abs() > 1e-4 {
                    let t = (u.x * w.z - u.z * w.x) / den;
                    let s = (dir.x * w.z - dir.z * w.x) / den;
                    if t > 0.0 && (0.0..=1.0).contains(&s) {
                        cands.push(l.a.lerp(l.b, s));
                    }
                }
            } else if !zone {
                cands.extend([1.5, 2.5, 3.5, 4.5].map(|d| l.closest(from + dir * d)));
            } else {
                // (A beam along the way: where the way leaves it, `BEAM_FAR_END_SHORT` short of its far end, as AC1's
                // candidates are where the forward line meets an edge; off the open roof north the running game jumped
                // 3 m to 0.27 m short of a beam's end, ours to its nearest point 1 m on.)
                let (near_end, far_end) = if (l.b - l.a).dot(dir) >= 0.0 { (l.a, l.b) } else { (l.b, l.a) };
                let back = (near_end - far_end).with_y(0.0).normalize_or_zero();
                cands.push(far_end + back * BEAM_FAR_END_SHORT);
            }
        }
    }
    // Tops beyond a drop: the first ground after a gap, a little in from its edge.
    // (Straight on only, as AC1's candidates lie on the forward line: tops found 20 degrees to either side were taken
    // nearest, and a jump went off the way, 1.2 m to a top beside it, where the running game jumped on 6.5 m.)
    for turn in [0.0f32] {
        let d = Quat::from_rotation_y(turn) * dir;
        let mut gap = false;
        for k in 2..=(reach.end() / 0.25) as usize {
            let t = k as f32 * 0.25;
            match level.ground(from + d * t, 1.2, JUMP_TARGET_DEEP + 0.2) {
                None => gap = true,
                // (Room to stand over it, and not inside a block: a probe starting inside one finds the floor under it,
                // and looking up from there meets the block's roof from below.)
                // (Not a beam's top: beams are targets by their own rule above.)
                Some(g)
                    if gap
                        && g.normal.y > 0.8
                        && level.perch_at(g.point, 0.3).is_none()
                        && level.ground(g.point + d * 0.4, 0.2, 0.2).is_some()
                        && level.raycast_sided(g.point + Vec3::Y * 0.05, Vec3::Y, 20.0).is_none_or(|(h, behind)| !behind && h.dist > JUMP_TOP_HEADROOM) =>
                {
                    // (At the edge: back in 5 cm steps to where the top begins.)
                    let edge = (1..5)
                        .map(|k| t - k as f32 * 0.05)
                        .take_while(|&s| level.ground(from + d * s, 1.2, JUMP_TARGET_DEEP + 0.2).is_some_and(|h| (h.point.y - g.point.y).abs() < 0.15))
                        .last()
                        .unwrap_or(t);
                    cands.push(from + d * (edge + ROOF_EDGE_INSET) + Vec3::Y * (g.point.y - from.y));
                    // (And for a running jump its far end, where it drops away again within reach, landed on
                    // `JUMP_FAR_END_SHORT` short of it: AC1's candidates are on edges facing either way. Off a roof over a
                    // street onto one 3.7 m below, everything behind, the furthest taken, the running game landed 0.2 m
                    // short of that roof's far end, 4.6 m on.)
                    if zone {
                        let at = |s: f32| (from + d * s).with_y(g.point.y);
                        let on_top = |s: f32| level.ground(at(s), 0.3, 0.3).is_some_and(|h| (h.point.y - g.point.y).abs() < 0.15);
                        let mut end = t;
                        while end + 0.1 <= *reach.end() && on_top(end + 0.1) {
                            end += 0.1;
                        }
                        if end + 0.1 <= *reach.end() && end - JUMP_FAR_END_SHORT > edge + ROOF_EDGE_INSET + 0.5 {
                            let s = end - JUMP_FAR_END_SHORT;
                            if let Some(h) = level.ground(at(s), 0.3, 0.3) {
                                cands.push(h.point);
                            }
                        }
                    }
                    break;
                }
                _ => {}
            }
        }
    }
    // Roof edges from the climbing markup (AC1's candidates are guidance edges, `0xE96BF0`): an edge facing us, its top
    // at most 1.3 m up, landed on 0.45 m in from it, across a gap.
    // (Where the way forward crosses the edge, as AC1's candidates are: the edge's point nearest a spot ahead was its end
    // when the way crossed the next piece of it, and the jump went 0.7 m off the way; the running game's kept to it.)
    for l in &level.ledges {
        let (u, w) = ((l.b - l.a).with_y(0.0), from - l.a);
        let den = dir.x * u.z - dir.z * u.x;
        if den.abs() < 1e-4 {
            continue;
        }
        let (t, s) = ((u.x * w.z - u.z * w.x) / den, (dir.x * w.z - dir.z * w.x) / den);
        if t <= 0.0 || !(0.0..=1.0).contains(&s) {
            continue;
        }
        let on = l.a.lerp(l.b, s);
        // (Facing us, or for a running jump facing away too, the far edge of a top, landed on just short of it: AC1's
        // candidates are on edges facing any way. Off a roof over a street onto a roof 3.7 m below, where everything counts as
        // behind and the furthest is taken, the running game landed 0.2 m short of its far edge, 4.6 m on.)
        let facing_us = l.out.dot((from - on).with_y(0.0)) > 0.0;
        if (on - from).with_y(0.0).length() > *reach.end() + 1.0 || !(facing_us || zone) || on.y - from.y > 1.3 {
            continue;
        }
        // (AC1: a top at least 0.3 m deep behind the edge, the way to it clear from the chest.)
        let near = on + (from - on).with_y(0.0).normalize_or_zero() * 0.2;
        if !Ac1Edge::probe(level, on, l.out).step_onto() || !ac1_clear_way(level, from, near) {
            continue;
        }
        let land = on - l.out * ROOF_EDGE_INSET;
        // (Not a beam's side: beams are targets by their own rule above.)
        let Some(g) = level.ground(land + Vec3::Y * 0.3, 0.0, 0.5).filter(|g| g.normal.y > 0.8 && level.perch_at(g.point, 0.3).is_none()) else { continue };
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
            reach.contains(&flat.length())
                && flat.normalize().dot(dir) >= JUMP_TARGET_CONE
                && flat.cross(dir).y.abs() <= JUMP_TARGET_ACROSS
                && if zone { in_jump_zone(flat.length(), q.y - from.y) } else { JUMP_TARGET_RISE.contains(&(q.y - from.y)) }
        })
        // (The way across clear over the higher of the two: a top under an awning or past a wall the flight meets is
        // no target, else the jump stops in the air and drops back, over and over.)
        .filter(|q| {
            let flat = (*q - from).with_y(0.0);
            // (At the knees and the chest too, where a flight is stopped: a low wall or a rail between is vaulted, not
            // jumped across.)
            let top = from.y.max(q.y);
            let way = flat.normalize_or_zero();
            [0.3, JUMP_AC1_CLEAR, 1.0].iter().all(|h| level.raycast(from.with_y(top + h), way, (flat.length() - 0.3).max(0.0)).is_none())
                // (Room to land: no wall at the chest just past it, and the top going on past it: a sill under a wall
                // is no target, the flight meets the wall and drops off it.)
                && (level.perch_at(*q, 0.15).is_some()
                    || (level.raycast(*q + Vec3::Y * 1.0, way, JUMP_LAND_ROOM).is_none() && level.ground(*q + way * 0.4, 0.3, 0.3).is_some()))
        })
        // AC1's scorer (0xE96BF0, docs/PARKOUR.md section 6): of the free-step targets (tops, posts, beams) in front, the
        // nearest; with none, of those behind, the furthest. In front is over a plane through the hips tilted down ahead
        // (normal 0.5 on, 0.7 up): a beam 4.4 m down 1.5 m on is behind, and the running game jumped over it onto the
        // roof 2.8 m on; one 2.3 m down 6.5 m on is in front, and it jumped to it.
        .map(|q| {
            let d = (q - from).with_y(0.0).length();
            (q, d, q.y - from.y > 0.5 - JUMP_FRONT_SLOPE * d)
        })
        .min_by(|a, b| b.2.cmp(&a.2).then(if a.2 { a.1.total_cmp(&b.1) } else { b.1.total_cmp(&a.1) }))
        .map(|(q, _, _)| q)
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
            eject_after: None,
            lean_t: 0.0,
            hay_over: None,
            clear_of: None,
            fall_v: None,
            launch: 0.0,
            free_jump: false,
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
    pub fn try_enter(
        lib: &mut AnimLib,
        level: &Level,
        root: &Transform,
        rig: &Rig,
        base: &Pose,
        cr: ClimbRig,
        moving: bool,
        from: Option<Pose>,
    ) -> Option<WallClimb> {
        let fwd = root.rotation * Vec3::NEG_Z;
        // (Within AC1's grab probe, 0.75 m round the body (IHuman vt136): further off, a press jumps at the wall's hold;
        // the running game's tap 1.25 m from the bureau's wall did.)
        let Some(hit) = level.raycast(root.translation + Vec3::Y * 1.2, fwd, if moving { CLIMB_START_REACH } else { CLIMB_START_STILL }) else {
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
        w.free_jump = bar_ahead(level, p, dir).is_none();
        w.start(takeoff, JUMP.into(), &planned);
        w.ease_in(from, root);
        Some(w)
    }

    /// A free step off a post or beam along `dir` with no target: AC1's free-step takeoff (the group by the way it goes off
    /// the facing) and flight, launched `FREE_STEP_SPEED` on and `FREE_STEP_UP` up, then the fall.
    fn free_step(lib: &mut AnimLib, root: &Transform, dir: Vec3, left: bool, from: Option<Pose>) -> Option<WallClimb> {
        let fwd = (root.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
        let aim = dir.with_y(0.0).normalize_or_zero();
        let angle = aim.dot(fwd.cross(Vec3::Y)).atan2(aim.dot(fwd));
        let j = crate::jump::freestep(0.0, FREE_JUMP_DIST, angle, left, 0.0);
        let mut mix = |parts: &[(String, f32)]| {
            let parts: Vec<(&str, f32)> = parts.iter().map(|(n, w)| (n.as_str(), *w)).collect();
            lib.get(&mix_name(&parts))
        };
        let (takeoff, air) = (mix(&j.takeoff)?, mix(&j.flight)?);
        debug!("climb: free step off the perch with no target via {}", takeoff.name);
        let planned = Transform { rotation: facing(aim), ..*root };
        let mut w = WallClimb::new(JUMP, planned.rotation * Vec3::Z);
        w.queue = vec![Queued::new(air, FALL)];
        w.fall_with = Some(aim * FREE_STEP_SPEED + Vec3::Y * FREE_STEP_UP);
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

    /// Anything a Legs press in high profile would jump at from `from` along `dir` (AC1's `JumpToGuidanceTarget`: a
    /// press jumps only at a target): a top, post or beam, a hold to catch, a thin wall to go over, a swing bar. With a
    /// wall right ahead (`wall_ahead`) only going over it counts (else it is climbed or leant on).
    pub fn jump_reachable(level: &Level, from: Vec3, dir: Vec3, wall_ahead: bool) -> bool {
        if wall_ahead {
            return passover_target(level, from, dir).is_some();
        }
        jump_target(level, from, dir, None).is_some()
            || jump_hold(level, from, dir).is_some()
            || passover_target(level, from, dir).is_some()
            || bar_ahead(level, from, dir).is_some()
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
        // (A swing bar ahead as near and no lower than the top: AC1's scorer takes the hang target first (0xE96BF0, a pole
        // over a front ledge when nearer and not below it); the plain jump flies onto the bar.)
        if let (Some(t), Some(b)) = (top, bar_ahead(level, p, dir))
            && (b - p).with_y(0.0).length() <= (t - p).with_y(0.0).length()
            && b.y >= t.y
        {
            return Self::jump(lib, level, root, dir, left, from);
        }
        let at_hold = top.is_none();
        // A thin wall ahead, too high to land on: over it, a hand on its top (AC1's passover).
        if top.is_none()
            // (Off the other foot when this one's passover can't be made: the game has no right foot's
            // `xx_h_air_up_300cm_footr_to_passover`.)
            && let Some(w) = Self::jump_passover(lib, level, root, rig, base, cr, dir, left, from.clone())
                .or_else(|| Self::jump_passover(lib, level, root, rig, base, cr, dir, !left, from.clone()))
        {
            return Some(w);
        }
        let Some(mut to) = top.or_else(|| jump_hold_target(level, p, dir)) else { return Self::jump(lib, level, root, dir, left, from) };
        // (Arriving within reach of a beam, onto the beam: AC1's free-step arrival test (0xE0B890, as Banned445's port reads
        // it) puts the root on the nearest beam in a box round the feet. Off the bureau's roof south-west the running game
        // came down 0.36 m beside the beam along the next roof's edge, stood on it and walked it.)
        if !at_hold && let Some((_, q)) = level.perch_at(to, BEAM_ARRIVE) {
            to = q;
        }
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
        // Each flight's own reception onto the edge (`<flight>_tr_passover_[entry_]hand?`), mixed as the flights are; a
        // flight the game lacks (it has no `xx_h_air_up_300cm_footr_to_passover`, only its reception) is left out of both.
        let (flights, reception): (Parts, Parts) = j
            .flight
            .iter()
            .filter(|(n, _)| lib.names.iter().any(|m| m == n))
            .filter_map(|(n, w)| {
                let stem = format!("{n}_tr_passover");
                lib.names.iter().find(|m| m.starts_with(&stem) && m.ends_with(hand)).map(|m| ((n.clone(), *w), (m.clone(), *w)))
            })
            .unzip();
        let mut mix = |parts: &[(String, f32)]| {
            let parts: Vec<(&str, f32)> = parts.iter().map(|(n, w)| (n.as_str(), *w)).collect();
            lib.get(&mix_name(&parts))
        };
        let (takeoff, flight, touch_clip) = (mix(&j.takeoff)?, mix(&flights)?, mix(&reception)?);
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
        // A big drop beyond with a hold on the far edge: over it and round into the wall hang on the far side (AC1's
        // `xx_h_passover_<hand>_tr_hangwall_a/b`, 0x0109BB59: over, 1 m down, turned round), not over and down it.
        let far = edge + fwd * depth;
        let drop = level.ground(far.with_y(edge.y + 0.2) + fwd * 0.4, 0.0, 30.0).map_or(f32::INFINITY, |g| edge.y - g.point.y);
        if drop > PULL_DOWN_DROP
            && let (Some(a), Some(b)) = (lib.get(&format!("xx_h_passover_{hand}_tr_hangwall_a")), lib.get(&format!("xx_h_passover_{hand}_tr_hangwall_b")))
        {
            let chain = [takeoff.clone(), flight.clone(), touch_clip.clone(), a.clone(), b.clone()];
            if let Some(end) = chain_end(&chain, rig, base, cr).map(|e| e.world(&planned))
                && let Some(targets) = holds_within(level, &end.hands.map(|h| h + err), fwd, 0.8)
            {
                let hang = (targets[0] - end.hands[0] + targets[1] - end.hands[1]) * 0.5 - err;
                // (The near face's normal: the chain turns it with the root's half turn, onto the far face's.)
                let mut w = WallClimb::new(VAULT, normal);
                w.start_chain_carry(chain.to_vec(), vec![VAULT.into(), VAULT.into(), VAULT.into(), DROP.into(), HANGWALL.into()], &planned, err, Some(1));
                if let Some(q) = w.queue.get_mut(2) {
                    q.correct = hang;
                }
                w.ease_in(from, root);
                debug!("climb: running jump over the wall at {edge:.2} into the hang on its far side ({drop:.1} m drop)");
                return Some(w);
            }
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
        // (The wall's lean under the edge, positive overhanging: AC1's angled receptions. The hard one for a fast jump or
        // a long one, over 6 m, as 0xE02790 picks it.)
        let tilt = level.raycast(hold - Vec3::Y * 0.8 + out * 0.6, -out, 1.2).map_or(0.0, |h| (-h.normal.y).clamp(-1.0, 1.0).asin());
        let j = crate::jump::surface(hang.y - p.y, way.length(), left, speed > JUMP_RECEPTION_HARD_SPEED || way.length() > 6.0, tilt);
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
        // (Where the reception's step ends, handed over to the run: see `cut` below.)
        let cut = (speed > RECEPTION_CUT_SPEED).then(|| reception_cut(&reception, speed)).flatten();
        // The reception's own step lands on the target, as AC1 aims it: in the running game the reception starts about
        // half a metre short of the edge (or beam) and its step ends there.
        let step = root_motion_at(&reception, cut.unwrap_or(reception.frames())).with_z(0.0);
        let landed = p + rot * (root_motion_at(&takeoff, takeoff.frames()) + root_motion_at(&flight, flight.frames()) + step);
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
        // (Onto a beam: down on it in AC1's narrow-object stance, balancing, not run on from: the running game walked
        // along the beam it came down on.)
        let onto_beam = level.perch_at(to, 0.15).filter(|(i, _)| level.perches[*i].axis() != Vec3::ZERO);
        let last = if onto_beam.is_some() { PERCH } else { GROUND };
        w.start_chain_carry(vec![takeoff.clone(), flight.clone(), reception], vec![VAULT.into(), VAULT.into(), last.into()], &planned, correct, Some(1));
        match onto_beam {
            Some((i, q)) => {
                w.perch = Some(i);
                debug!("climb: onto perch {i} at {q:.2} (landing on it)");
            }
            // On at the run's speed once down (the reception's `_tr_freestep_entry` leads into the run in AC1's graph).
            None => w.exit_velocity = aim * speed,
        }
        // (The reception ends once its step is taken, not standing out the rest of it (played faster instead, its
        // front-loaded step lurched the root to 16 m/s); onto a beam too: the running game free-stepped on 0.09 s after
        // coming down on one.)
        if let (Some(f), Some(r)) = (cut, w.queue.last()) {
            w.cut = Some((r.clip.name.clone(), f / FPS));
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
        // (The reception's step ends on the target, as AC1 aims it: see `jump_ac1`.)
        let step = root_motion_at(&reception, reception.frames()).with_z(0.0);
        let landed = p + rot * root_motion_at(&takeoff, takeoff.frames()) + turned * (root_motion_at(&flight, flight.frames()) + step);
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
            debug!("climb: no wall run: the wall is {:.0} degrees off square", run.dot(-n).clamp(-1.0, 1.0).acos().to_degrees());
            return None;
        }
        let fwd = -n;
        let facing = Transform { rotation: Quat::from_rotation_arc(Vec3::NEG_Z, fwd), ..*root };
        let root = &facing;
        let hit = level.raycast(root.translation + Vec3::Y, fwd, WALL_RUN_REACH).filter(|h| h.normal.y.abs() < 0.3)?;
        // Not up a pole or post under a cap or ledge sticking out over it (nothing for the feet).
        // (Over the line run up, within `WALL_RUN_CAP_SIDE` along the wall: a pilaster's top 0.28 m beside it, on a Damascus
        // street wall the running game ran up, is not over it.)
        let along = Vec3::Y.cross(n).normalize_or_zero();
        let set_back = level.ledges.iter().filter(|l| l.out.dot(n) > 0.7 && l.a.y > hit.point.y && l.a.y < hit.point.y + 4.0).any(|l| {
            let q = l.closest(hit.point);
            (q - hit.point).with_y(0.0).length() < 0.6
                && (q - hit.point).dot(along).abs() < WALL_RUN_CAP_SIDE
                && level.raycast(q + n * 0.3 - Vec3::Y * 0.9, -n, 0.55).is_none()
        });
        // (A ladder up the wall in front counts as wall: the probe meets its rungs, narrower than a wall; the running game
        // ran up the wall onto one by the Damascus bureau.)
        let n_flat = hit.normal.with_y(0.0).normalize_or_zero();
        let along_wall = Vec3::Y.cross(n_flat).normalize_or_zero();
        let ladder_here = level
            .ladders
            .iter()
            .any(|l| l.out.dot(n_flat) > 0.9 && (l.base - hit.point).dot(along_wall).abs() < 0.5 && (l.base - hit.point).dot(n_flat).abs() < 0.5);
        if !ladder_here && (set_back || !wide_wall(level, root.translation + Vec3::Y, fwd, hit.dist)) {
            debug!("climb: no wall run: {}", if set_back { "a ledge sticks out over it" } else { "the wall is too narrow" });
            return None;
        }
        // A ladder up the wall in front: up it onto the ladder (AC1's `xx_h_wallingfront_step1_footr_tr_h_ladder_up_l`, on
        // into its left-hand climb), the root steered onto the ladder's climbing spot.
        let p = root.translation;
        if let Some((i, l)) = level
            .ladders
            .iter()
            .enumerate()
            // (On this wall: the distance off it too, or a ladder on a wall in line 94 m further on was run onto, in Damascus.)
            .find(|(_, l)| {
                l.out.dot(n) > 0.9
                    && (l.base - hit.point).dot(along).abs() < 0.5
                    && (l.base - hit.point).dot(n).abs() < 0.5
                    && l.base.y <= p.y + 0.5
                    && l.top >= p.y + 3.0
            })
            && let Some(clips) =
                WALL_RUN.iter().chain(["xx_h_wallingfront_step1_footr_tr_h_ladder_up_l"].iter()).map(|n| lib.get(n)).collect::<Option<Vec<_>>>()
        {
            let planned = *root;
            let travel: Vec3 = clips.iter().map(|c| world_rot(planned.rotation) * root_motion_at(c, c.frames())).sum();
            let spot = l.base + l.out * LADDER_OFF;
            let correct = (spot - (p + travel)).with_y(0.0);
            let mut w = WallClimb::new(WALL_RUN_STATE, n);
            w.ladder = Some(i);
            let k = clips.len();
            let tos = (0..k).map(|j| if j + 1 == k { LADDER_L } else { WALL_RUN_STATE }.to_string()).collect();
            w.start_chain(clips, tos, &planned, correct);
            w.ease_in(from, root);
            debug!("climb: wall run onto ladder {i}");
            return Some(w);
        }
        // A thin wall 1.3-2.5 m high with a drop beyond: up it, a hand on its top and over (AC1's
        // `xx_h_wallingfront_<entry_footl|step1_footr>_tr_passover_<cm>_<hand>` into `xx_h_passover_<hand>_<cm>cm`).
        if let Some(w) = Self::wall_run_passover(lib, level, root, rig, base, cr, hit.point, n) {
            let mut w = w;
            w.ease_in(from, root);
            return Some(w);
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
                // (The entry ends where AC1 warps it: `WALL_RUN_OUT` out from the wall, `WALL_RUN_UP` over the feet.)
                let travel = world_rot(planned.rotation) * root_motion_at(&first, first.frames());
                let contact = wall_run_contact(level, root, -normal).unwrap_or((hit.point + normal * WALL_RUN_OUT).with_y(root.translation.y + WALL_RUN_UP));
                w.start(first, WALL_RUN_STATE.into(), &planned);
                if let Some(m) = &mut w.mv {
                    m.correct = contact - (root.translation + travel);
                }
                w
            }
        };
        w.ease_in(from, root);
        Some(w)
    }

    /// A wall run over a thin wall (see `wall_run`): the top `h` 1.3-2.5 m over the feet at `face` (normal `n`), at most
    /// `PASSOVER_DEPTH` deep with a drop of 0.5 m or more beyond; the hand steered onto the top's near edge.
    #[allow(clippy::too_many_arguments)]
    fn wall_run_passover(lib: &mut AnimLib, level: &Level, root: &Transform, rig: &Rig, base: &Pose, cr: ClimbRig, face: Vec3, n: Vec3) -> Option<WallClimb> {
        let p = root.translation;
        let fwd = -n;
        let top = level.ground(face.with_y(p.y + 2.8) + fwd * 0.05, 0.0, 2.8)?;
        let h = top.point.y - p.y;
        if !(1.25..=2.55).contains(&h) || top.normal.y < 0.8 {
            return None;
        }
        // (Open over the top, and the way across it clear for the body: in Damascus a 4 m wall's inside read as a 1.4 m
        // top, the probe starting within it, and the passover went through the wall.)
        let open_above = level.raycast(top.point + Vec3::Y * 0.05, Vec3::Y, (p.y + 2.8 - top.point.y).max(0.1)).is_none();
        let across = level.raycast(face.with_y(top.point.y + 0.5) + n * 0.3, fwd, PASSOVER_DEPTH + 0.9).is_none();
        if !open_above || !across {
            debug!("climb: no wall run over the wall: not open above its top at {:.2}", top.point.y);
            return None;
        }
        let on_top = |d: f32| level.ground(face.with_y(top.point.y + 0.2) + fwd * d, 0.0, 0.4).is_some_and(|g| (g.point.y - top.point.y).abs() < 0.1);
        let depth = (1..=(PASSOVER_DEPTH / 0.05) as usize + 1).map(|k| k as f32 * 0.05).find(|&d| !on_top(d))?;
        let beyond = level.ground(face.with_y(top.point.y) + fwd * (depth + 0.3), 0.0, 10.0);
        if depth > PASSOVER_DEPTH || beyond.is_some_and(|g| top.point.y - g.point.y < 0.5) {
            return None;
        }
        let cm = if depth < PASSOVER_THIN { "030" } else { "100" };
        let (mut names, hand): (Vec<String>, &str) = if h < 2.0 {
            let w = ((h - 1.31) / 0.69).clamp(0.0, 1.0);
            let t =
                mix_name(&[("xx_h_wallingfront_entry_footl_tr_passover_131cm_handl", 1.0 - w), ("xx_h_wallingfront_entry_footl_tr_passover_200cm_handl", w)]);
            (vec![WALL_RUN[0].into(), WALL_RUN[1].into(), t], "handl")
        } else {
            let w = ((h - 2.01) / 0.49).clamp(0.0, 1.0);
            let t =
                mix_name(&[("xx_h_wallingfront_step1_footr_tr_passover_201cm_handr", 1.0 - w), ("xx_h_wallingfront_step1_footr_tr_passover_250cm_handr", w)]);
            (WALL_RUN.iter().map(|s| s.to_string()).chain([t]).collect(), "handr")
        };
        let touch = names.len();
        names.push(format!("xx_h_passover_{hand}_{cm}cm"));
        names.push(format!("xx_h_passover_{hand}_{cm}cm_tr_fall"));
        let clips = names.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>()?;
        // (The hand on the near edge: across the wall and up, wherever along the top.)
        let end = chain_end(&clips[..touch], rig, base, cr)?.world(root);
        let edge = face.with_y(top.point.y);
        let hand_at = end.hands[usize::from(hand == "handr")];
        let along = Vec3::Y.cross(n).normalize_or_zero();
        let err = edge - hand_at;
        let err = err - along * err.dot(along);
        if err.length() > 1.0 {
            debug!("climb: no wall run over the wall: the hand lands {:.2} m off", err.length());
            return None;
        }
        let k = clips.len();
        let tos = (0..k)
            .map(|i| {
                if i + 1 == k {
                    FALL
                } else if i >= touch {
                    VAULT
                } else {
                    WALL_RUN_STATE
                }
                .to_string()
            })
            .collect();
        // (Over it, the root clear of the far edge before the fall, as the running passover: else it comes down on the top.)
        let over_end = chain_end(&clips[..=touch], rig, base, cr)?.world(root).pos + err;
        let short = (edge + fwd * (depth + PASSOVER_CLEAR) - over_end).dot(fwd).max(0.0);
        let mut w = WallClimb::new(WALL_RUN_STATE, n);
        w.start_chain(clips, tos, root, err);
        if let Some(q) = w.queue.get_mut(touch - 1) {
            q.correct = fwd * short;
        }
        w.clear_of = Some(top.point.y);
        debug!("climb: wall run over the {h:.2} m wall ({cm} cm deep, {hand})");
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
        let mut blended = top.map_or(vec![], |(_, h)| blend_onto(opts, h));
        // A wall run's catch into the wall hang blends its 251 and 430 cm clips by the hold's height (AC1's probe D, as
        // Banned445's port reads it): any hold between the two is caught, not only those near one clip's reach.
        if carry.is_some() {
            blended.extend(blend_hang(lib, level, root, rig, base, cr, normal, opts));
        }
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
                    let Some(targets) = holds_within(level, &hands, normal, slack + 0.1) else {
                        if carry.is_some() {
                            debug!(
                                "climb: wall run catch {}: no holds for both hands near {:.2}",
                                opt.names.last().map_or("", |n| n.as_str()),
                                (hands[0] + hands[1]) * 0.5
                            );
                        }
                        continue;
                    };
                    // (Not the wall just under the hold: the feet's footing is checked where they go, below. AC1's probe D
                    // only asks for the edge; on a Damascus street wall it caught one over a window, the feet on the wall
                    // under the sill.)
                    // (A wall run catches a hold at least a body height over the feet where its vertical step ends: AC1's
                    // ledge probe D, 1-2.8 m over them, as Banned445's port reads it. In the running game the feet end that
                    // step 1.6 m up, and the hold 2.5 m over the floor was passed for the next row, 4.1 m up.)
                    if carry.is_some() && (targets[0].y + targets[1].y) * 0.5 - root.translation.y < WALL_RUN_CATCH_MIN {
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
                        if carry.is_some() {
                            debug!(
                                "climb: wall run catch at {:.2}: {}",
                                (targets[0] + targets[1]) * 0.5,
                                if (-0.6..=0.3).contains(&err.y) { "no footing" } else { "out of reach" }
                            );
                        }
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
            // A wall run's catch: the entry ends where AC1 warps it (`WALL_RUN_OUT`, `WALL_RUN_UP`), the rest of the way
            // to the hold taken in the catch (else the whole of it was in the entry, which ended 0.17 m off where the
            // running game's does).
            let catch_at = clips.len().checked_sub(opt.catch).filter(|&k| k > 0);
            let split = match (carry, hit, catch_at, &opt.end) {
                (Some(0), Some(_), Some(k), GrabEnd::Hang(..)) => {
                    let travel = world_rot(facing.rotation) * root_motion_at(&clips[0], clips[0].frames());
                    Some((k, wall_run_contact(level, root, fwd)? - (root.translation + travel)))
                }
                _ => None,
            };
            w.start_chain_carry(clips, tos, &facing, err, carry);
            if let Some((k, warp)) = split
                && let (Some(m), Some(q)) = (w.mv.as_mut(), w.queue.get_mut(k - 1))
            {
                q.correct += m.correct - warp;
                m.correct = warp;
            }
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

    /// An eject off the wall (AC1's rebound, as the running game does it: Damascus, the bureau's wall; see `legs`) at the
    /// best place to land in the stick's way: straight away from the wall (`normal`), or along it to the side the stick
    /// is held. Turned away from the wall, AC1's rebound takeoff for that way, height and distance (`crate::jump::rebound`),
    /// then the flight and the reception, its step ending on the target. `None` with nothing to land on that way.
    #[allow(clippy::too_many_arguments)]
    fn rebound_jump(lib: &mut AnimLib, level: &Level, root: &Transform, normal: Vec3, input: Vec2, left: bool, from: Option<Pose>) -> Option<WallClimb> {
        let away = normal.with_y(0.0).normalize_or_zero();
        // (The stick's right as he faces the wall.)
        let right = (-away).cross(Vec3::Y);
        let way = if input.x.abs() > 0.5 { right * input.x.signum() } else { away };
        let p = root.translation;
        let top = jump_target_within(level, p, way, None, EJECT_REACH, false);
        // A hold to hang from that way (a beam's end, a ledge), as near as the top and higher: AC1's scorer takes the hang
        // first (a pole over a front ledge, 0xE96BF0). Off a wall run the running game ejected back onto a beam's end 3 m
        // behind, 2.2 m up, and hung from it.
        if let Some(h) = jump_hold_target(level, p, way)
            && top.is_none_or(|t| (h - p).with_y(0.0).length() <= (t - p).with_y(0.0).length() && h.y >= t.y)
        {
            return Self::eject_to_hang(lib, root, h, away, left, from);
        }
        let to = top?;
        let flat = (to - p).with_y(0.0);
        let aim = flat.normalize_or_zero();
        // (Its angle off straight away, positive to the right of the body turned away.)
        let angle = aim.dot(away.cross(Vec3::Y)).atan2(aim.dot(away));
        let j = crate::jump::rebound(to.y - p.y, flat.length(), angle, left, 0.0);
        let mut mix = |parts: &[(String, f32)]| {
            let parts: Vec<(&str, f32)> = parts.iter().map(|(n, w)| (n.as_str(), *w)).collect();
            lib.get(&mix_name(&parts))
        };
        let (takeoff, flight, reception) = (mix(&j.takeoff)?, mix(&j.flight)?, mix(&j.reception)?);
        let planned = Transform { rotation: facing(away), ..*root };
        let rot = world_rot(planned.rotation);
        let turned = world_rot(planned.rotation * root_delta(root_rotation_at(&takeoff, takeoff.frames())));
        // (The reception's step ends on the target, as AC1 aims it: see `jump_ac1`.)
        let step = root_motion_at(&reception, reception.frames()).with_z(0.0);
        let landed = p + rot * root_motion_at(&takeoff, takeoff.frames()) + turned * (root_motion_at(&flight, flight.frames()) + step);
        let correct = to - landed;
        // (A near one too: the shortest flight lands about 0.5 m past a beam 0.95 m off, which the running game took.)
        if correct.with_y(0.0).length() > JUMP_AC1_SLACK * flat.length().max(EJECT_SLACK_FROM) {
            debug!("climb: AC1's rebound clips land {:.2} m off", correct.length());
            return None;
        }
        let mut w = WallClimb::new(VAULT, -aim);
        w.start_chain_carry(vec![takeoff.clone(), flight.clone(), reception], vec![LEAP.into(), VAULT.into(), GROUND.into()], &planned, correct, Some(1));
        // (Turned from facing the wall to facing away as the takeoff starts.)
        if let Some(m) = &mut w.mv {
            m.ease_rot = root.rotation * planned.rotation.inverse();
        }
        if level.perch_inside(to, PERCH_REACH).is_none() {
            w.exit_velocity = aim * PERCH_OFF_RUN;
        }
        w.ease_in(from, root);
        debug!("climb: rebound off the wall at {to:.2}, {:.0} degrees off straight away ({} then {})", angle.to_degrees(), takeoff.name, flight.name);
        Some(w)
    }

    /// An eject at a hold to hang from (`jump_hold_target`'s root `h`): AC1's rebound takeoff for it, then the flight
    /// there, the catch taking the hold as it comes.
    fn eject_to_hang(lib: &mut AnimLib, root: &Transform, h: Vec3, away: Vec3, left: bool, from: Option<Pose>) -> Option<WallClimb> {
        let p = root.translation;
        let flat = (h - p).with_y(0.0);
        let aim = flat.normalize_or_zero();
        let angle = aim.dot(away.cross(Vec3::Y)).atan2(aim.dot(away));
        let j = crate::jump::rebound(h.y - p.y, flat.length(), angle, left, 0.0);
        let parts: Vec<(&str, f32)> = j.takeoff.iter().map(|(n, w)| (n.as_str(), *w)).collect();
        let (takeoff, air) = (lib.get(&mix_name(&parts))?, lib.get(JUMP_AIR)?);
        let planned = Transform { rotation: facing(away), ..*root };
        // (From where the takeoff ends: it lifts and moves the root itself.)
        let lift = p + world_rot(planned.rotation) * root_motion_at(&takeoff, takeoff.frames());
        let (v, flight) = ballistic(lift, h);
        let mut w = WallClimb::new(JUMP, -aim);
        w.queue = vec![Queued { rate: air.anim.duration / flight.max(0.2), ..Queued::new(air, FALL) }];
        w.fall_with = Some(v);
        w.can_catch = true;
        w.aimed = true;
        w.start(takeoff, JUMP.into(), &planned);
        if let Some(m) = &mut w.mv {
            m.ease_rot = root.rotation * planned.rotation.inverse();
        }
        w.ease_in(from, root);
        debug!("climb: eject at the hold over {h:.2}");
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
                // (On the same floor: on a roof over a street bench, he sat on the roof above it.)
                ((q - p).with_y(0.0).length() < BENCH_REACH && (q.y - p.y).abs() < 0.4).then_some((q, f))
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

    /// Free running on a top level with a haystack just ahead (within 45 degrees of `dir`): dived into, as AC1 does off the
    /// hay box's rim it hopped onto (`xx_h_freestep_footr_to_haystack_02`, the running game, Damascus).
    pub fn hay_dive_ahead(lib: &mut AnimLib, level: &Level, root: &Transform, dir: Vec3, from: Option<Pose>) -> Option<WallClimb> {
        let p = root.translation;
        let dir = dir.with_y(0.0).normalize_or_zero();
        let ahead = level.haystacks.iter().any(|h| {
            let to = (h.centre - p).with_y(0.0);
            to.length() < h.half + HAY_RUN_DIVE_REACH && to.normalize_or_zero().dot(dir) > 0.7 && (-0.3..FAITH_MIN_DROP).contains(&(p.y - h.top()))
        });
        if !ahead {
            return None;
        }
        Self::into_hay(lib, level, root, true, from)
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

    /// The clip playing now: a walk along a beam (its start, then its steps), a move, or the hang loop.
    pub fn clip_name(&self) -> Option<&str> {
        let cycle = self.cycle.as_ref().map(|c| c.intro.as_ref().map_or(c.clips[c.foot].name.as_str(), |i| i.0.name.as_str()));
        cycle.or(self.mv.as_ref().map(|m| m.clip.name.as_str())).or(self.wait.as_ref().map(|w| w.name.as_str()))
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
        // (Met at the waist: a wall about a metre tall leans too, with AC1's 70 cm clips mixed in; lower ones are the
        // foot-up collide's.)
        let hit = level.raycast(p + Vec3::Y * (LEAN_MIN_HEIGHT + 0.1), dir, 1.0).filter(|h| h.normal.y.abs() < 0.3)?;
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
    fn lean_step(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform) {
        let n = self.normal;
        let d = self.move_dir.with_y(0.0);
        // (Pushing into it within 72 degrees: walking at a haystack's box about 60 degrees off square, the running game leant
        // and climbed it.)
        if d.length() > 0.3 && d.normalize().dot(-n) > LEAN_PUSH_COS {
            // Still pushing a while: up onto it, if it is low enough with room on top (AC1's
            // `xx_h_lean_wait_025cm_twohand_<070|150>cm_to_hangknee_footl_<070|150>cm` and its `_tr_hangknee_footl`, then
            // the stand: the running game, walking into a haystack's 1.06 m box at the Damascus bureau, leant on it and
            // after 0.2 s climbed onto its rim).
            if self.lean_t >= LEAN_CLIMB_AFTER && self.mv.is_none() {
                self.climb_from_lean(lib, level, root);
            }
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

    /// From leaning on a low wall, up onto its top: the lean's climb, blended by the top's height, then AC1's stand.
    fn climb_from_lean(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform) -> bool {
        let n = self.normal;
        let p = root.translation;
        // (The top nearest in from the face that is high enough: a haystack's box is a thin rim round hay that is not
        // solid, the running game stood on the rim.)
        let Some(face) = level.raycast(p + Vec3::Y * LEAN_MIN_HEIGHT, -n, LEAN_DIST + 0.5).map(|h| h.point.with_y(p.y)) else { return false };
        let Some(top) = LEAN_CLIMB_IN.iter().find_map(|&d| {
            level
                .ground(face - n * d + Vec3::Y * (LEAN_TALL + 0.3), 0.0, LEAN_TALL + 0.3)
                .filter(|t| (LEAN_MIN_HEIGHT - 0.1..=LEAN_TALL + 0.1).contains(&(t.point.y - p.y)) && t.normal.y > 0.8)
        }) else {
            return false;
        };
        let h = top.point.y - p.y;
        // (Room to stand on it.)
        if level.raycast(top.point + Vec3::Y * 0.1, Vec3::Y, 1.6).is_some() {
            return false;
        }
        let mix = ((h - LEAN_MIN_HEIGHT) / (LEAN_TALL - LEAN_MIN_HEIGHT)).clamp(0.0, 1.0);
        let names = [
            lean_name("xx_h_lean_wait_025cm_twohand_{h}cm_to_hangknee_footl_{h}cm", mix),
            lean_name("xx_h_lean_wait_025cm_twohand_{h}cm_to_hangknee_footl_{h}cm_tr_hangknee_footl", mix),
            STAND_UP.to_string(),
        ];
        let Some(mut clips) = names.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>() else { return false };
        let at = Transform { rotation: facing(-n), ..*root };
        // (A haystack's rim: on into the hay, as the running game went on from it, still pushing.)
        let hay = level.haystacks.iter().find(|hs| (hs.centre - top.point).with_y(0.0).length() < hs.half + 0.3 && (top.point.y - hs.top()).abs() < 0.3);
        let hay = hay.and_then(|hs| Some((hs.centre, lib.get("xx_h_freestep_footr_to_haystack_02")?)));
        let mut tos: Vec<String> = (0..clips.len()).map(|i| if i + 1 == clips.len() { ON_TOP } else { KNEEL }.to_string()).collect();
        let mut end = top.point;
        let into_hay = hay.is_some();
        if let Some((centre, dive)) = hay {
            clips.push(dive);
            tos.push(HAY.into());
            end = centre;
        }
        let travel: Vec3 = clips.iter().map(|c| world_rot(at.rotation) * root_motion_at(c, c.frames())).sum();
        let correct = end - (p + travel);
        debug!("climb: up onto the {h:.2} m wall from leaning on it{}", if into_hay { ", and into the haystack" } else { "" });
        self.start_chain(clips, tos, &at, correct);
        true
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
            // (Not with the body inside the wall under the edge: a narrow wall top's own wall, the feet inside it
            // seeing no wall and taking it for a free hang.)
            let hips = end_pos + err + hang_offset(to, normal) + Vec3::Y * 1.0;
            if level.inside_solid(hips) || level.inside_solid(hips - Vec3::Y * 0.6) {
                debug!("climb: no pull-down into {to}: the body would be inside the wall");
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

    /// Standing on a post or beam (stepped onto it from the ground). `from` is the pose shown now. `stick` (the wanted
    /// direction), `sprint` and `lead_left` (the foot it came down on): on a beam met across with the stick along it, AC1's
    /// side entry turns him onto it, walking or jogging (see `beam_side_entry`).
    #[allow(clippy::too_many_arguments)]
    pub fn perch(lib: &mut AnimLib, level: &Level, root: &Transform, stick: Vec3, sprint: bool, lead_left: bool, from: Option<Pose>) -> Option<WallClimb> {
        let (i, _) = level.perch_at(root.translation, PERCH_REACH)?;
        let line = level.perches[i];
        let wait = lib.get(PERCH_WAIT)?;
        let mut w = WallClimb::new(PERCH, root.rotation * Vec3::Z);
        w.perch = Some(i);
        w.wait = Some(wait);
        w.sprint = sprint;
        w.fade = from.map(|p| (p, ENTER_FADE, ENTER_FADE));
        debug!("climb: onto perch {i} at {:.2}", root.translation);
        if let Some(clip) = beam_side_entry(lib, &line, root, stick, sprint, lead_left) {
            debug!("climb: side entry onto the beam via {}", clip.name);
            w.beam_stance = None;
            let r = Transform { translation: line.closest(root.translation), ..*root };
            w.start_chain(vec![clip], vec![PERCH.to_string()], &r, Vec3::ZERO);
        }
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
        // (Or in high profile, the stick off its side: AC1's free step off it, as off the beam ending a Damascus roof.)
        let off_side = self.high && axis != Vec3::ZERO && dir.dot(axis).abs() <= BEAM_WALK_COS;
        if (self.sprint || off_side) && (self.perch_faith(lib, level, root, dir) || self.perch_jump(lib, level, root, dir)) {
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
            (BeamStance::Along(f), false) if !self.sprint && !self.high => {
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
        // From the crouch on a foot facing along the beam: AC1's start (0x34662CB4 / B5) on into the other foot's step.
        let feet = ["footl", "footr"];
        let intro = match self.beam_stance {
            Some(BeamStance::Along(f)) => {
                let (p, gait) = if self.sprint { ("h", "crouchjog") } else { ("l", "crouchwalk") };
                lib.get(&format!("xx_{p}_beam_crouchwait_{}_tr_{gait}_{}", feet[f], feet[1 - f])).map(|c| {
                    let v = root_motion_at(&c, c.frames()).length() / c.anim.duration.max(1e-3);
                    (f, c, v)
                })
            }
            _ => None,
        };
        let foot = intro.as_ref().map_or(0, |(f, _, _)| 1 - f);
        self.cycle = Some(Cycle { clips: [a, b], foot, phase: 0.0, dir, speed, intro: intro.map(|(_, c, v)| (c, 0.0, v)) });
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
            // Jogging, let go: AC1's jog stop on the foot it was on (0x3466339D / 9E), into the crouch on it.
            let foot = cy.foot;
            let feet = ["footl", "footr"];
            let names = [format!("xx_h_beam_crouchjog_stop_{}", feet[foot]), format!("xx_h_beam_crouchjog_stop_{0}_tr_crouchwait_{0}", feet[foot])];
            let clips = if self.sprint && cy.intro.is_none() { names.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>() } else { None };
            stop(self, lib);
            if let Some(clips) = clips {
                let r = Transform { translation: line.closest(root.translation), ..*root };
                self.start_chain(clips, vec![PERCH.to_string(); 2], &r, Vec3::ZERO);
            }
            return;
        }
        let speed = cy.intro.as_ref().map_or(cy.speed, |i| i.2.max(0.3));
        let next = root.translation + cy.dir * speed * dt;
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
            // Off the end onto the next beam going on from it, walked on (a plank bridge bent in the middle, two beams:
            // AC1 walked it as one narrow object, ours stepped off onto "ground" between them).
            let on = level.perches.iter().enumerate().find(|(k, l)| {
                Some(*k) != self.perch && l.axis().dot(cy.dir).abs() > BEAM_CHAIN_COS && (l.a.distance(q) < BEAM_CHAIN_GAP || l.b.distance(q) < BEAM_CHAIN_GAP)
            });
            if let Some((k, l)) = on {
                let axis = l.axis();
                cy.dir = if axis.dot(cy.dir) > 0.0 { axis } else { -axis }.with_y(0.0).normalize_or_zero();
                self.perch = Some(k);
                root.translation = l.closest(next);
                debug!("climb: on along the next beam {k} at {:.2}", root.translation);
                return;
            }
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
        if let Some((c, t, _)) = &mut cy.intro {
            *t += dt;
            if *t >= c.anim.duration {
                cy.intro = None;
            }
            return;
        }
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
        // (No target, a floor a step down ahead: AC1's free step off it all the same, low and fast. Off a beam ending a
        // Damascus roof it came down 0.74 m below 2.5 m on in 0.45 s, off a 0.57 m block 1.07 m below 3.2 m on in 0.6 s: both
        // a launch of `FREE_STEP_SPEED` on and `FREE_STEP_UP` up under gravity. A beam's only, over at most
        // `PERCH_FREE_STEP_DROP`: off a post, or over a deeper drop (G1's beam stuck out of a wall, the street 1.5 m below, a
        // platform further on), the jump below.)
        let beam = self.perch.and_then(|i| level.perches.get(i)).is_some_and(|l| l.axis() != Vec3::ZERO);
        let floor =
            beam && PERCH_FREE_STEP_ON.iter().any(|&d| level.ground(from + dir * d, 0.3, PERCH_FREE_STEP_DROP).is_some_and(|g| g.point.y < from.y - 0.2));
        if to.is_none()
            && floor
            && let Some(w) = Self::free_step(lib, root, dir, self.freestep_left, self.last.clone())
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
                // (On a beam, not within 0.4 m of its ends: a landing there is ground's, as a beam's end meets a top, and
                // a swing let go at a beam's near end came down there in a ground landing.)
                let len = (l.b - l.a).length();
                let q = if len > 1e-3 {
                    let k = 0.4f32.min(len * 0.5);
                    l.a + (l.b - l.a) / len * (q - l.a).dot((l.b - l.a) / len).clamp(k, len - k)
                } else {
                    q
                };
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
        // (Swinging up again from the still hang: AC1's `xx_h_swing_momentum_front_up` stands for the cycle's first.)
        let i = if done.name == SWING_MOMENTUM { 0 } else { SWING_CYCLE.iter().position(|n| *n == done.name).unwrap_or(SWING_CYCLE.len() - 1) };
        // The stick not held forward at the top of a swing: AC1's stop (`xx_h_swing_stop_<front|back>_a..d`, the last
        // items of action 0x023E0C61), into the still free hang from the bar.
        let fwd = (root.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or(Vec3::NEG_Z);
        if (i == 0 || i == 2) && self.swing_off.is_none() && self.move_dir.with_y(0.0).dot(fwd) < 0.3 {
            let side = if i == 0 { "front" } else { "back" };
            if let Some(clips) = ["a", "b", "c", "d"].iter().map(|k| lib.get(&format!("xx_h_swing_stop_{side}_{k}"))).collect::<Option<Vec<_>>>() {
                debug!("climb: the swing comes to rest ({side})");
                self.start_chain(clips, vec![SWING.into(), SWING.into(), SWING.into(), FREE.into()], root, Vec3::ZERO);
                // (Faded in: the stop's first pose is 7 cm off the swing's last at the feet.)
                self.fade = self.last.clone().map(|p| (p, ENTER_FADE, ENTER_FADE));
                return true;
            }
        }
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
        // (Down the middle, then at the body's sides: running past a corner, the middle missed it while the body ran into
        // it and stood there. AC1 looks for these tops in a box 0.5 m either side.)
        let side = dir.cross(Vec3::Y);
        let hit = [0.0, -VAULT_SIDE, VAULT_SIDE]
            .iter()
            .find_map(|&s| level.raycast(p + side * s + Vec3::Y * 0.25, dir, VAULT_REACH).filter(|h| h.normal.y.abs() < 0.3))?;
        let normal = hit.normal.with_y(0.0).normalize();
        let fwd = -normal;
        if fwd.dot(dir) < 0.7 {
            if why() {
                debug!(
                    "climb: no jump onto the obstacle {:.2} m ahead: its face is turned {:.0} degrees off the run",
                    hit.dist,
                    fwd.dot(dir).acos().to_degrees()
                );
            }
            return None;
        }
        // Its top just past the face, in range, walkable, with room above to stand on it.
        // (A little further in too: at a corner, or a face with a lip under its top, the top starts past where it was met.)
        let Some(top) =
            [0.08, 0.2, 0.35].iter().find_map(|&d| level.ground(hit.point.with_y(p.y + VAULT_HEIGHT.end() + 0.2) + fwd * d, 0.0, VAULT_HEIGHT.end() + 0.2))
        else {
            if why() {
                debug!("climb: no jump onto the obstacle {:.2} m ahead: no top over it within {:.1} m", hit.dist, VAULT_HEIGHT.end());
            }
            return None;
        };
        let h = top.point.y - p.y;
        // (A haystack's rim: hopped onto with no room to stand, under its cover, as AC1 does before diving in.)
        let hay = level.haystacks.iter().any(|y| y.contains(top.point + fwd * 0.6, -0.3) && y.top() < top.point.y + 0.3);
        // (Not a step up further on, with room to stand on it: off the open roof's south side, a top with a second step
        // 0.3 m higher 0.8 m past its edge, which the running game free-stepped onto.)
        let step_past = |d: f32| level.ground(p.with_y(top.point.y + STEP_PAST_UP) + fwd * (d + 0.15), 0.0, STEP_PAST_UP).is_some_and(|g| g.normal.y > 0.8);
        let (rose, blocked) = (
            rise(level, p, fwd, hit.dist, top.point.y),
            level.raycast(p.with_y(top.point.y + 0.3), fwd, hit.dist + 1.0).filter(|b| !hay && !step_past(b.dist)),
        );
        if !VAULT_HEIGHT.contains(&h) || top.normal.y < 0.8 || rose < *VAULT_HEIGHT.start() || blocked.is_some() {
            if why() {
                debug!(
                    "climb: no jump onto the {h:.2} m top {:.2} m ahead: its normal {:.2}, rise {rose:.2}, something over it {:?} m on (nearest haystack {:?})",
                    hit.dist,
                    top.normal.y,
                    blocked.map(|b| b.dist),
                    level.haystacks.iter().min_by(|a, b| (a.centre - top.point).length().total_cmp(&(b.centre - top.point).length())).map(|h| (
                        h.centre,
                        h.half,
                        h.top()
                    ))
                );
            }
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
        let mut exit = edge + fwd * STEP_ONTO_IN.min(depth * 0.5);
        // (Onto a post or a short beam: down on it to balance, as a free-step jump onto one does; the running game jumped
        // onto a 1 m post on a Damascus roof and free-stepped on from it, ours ran off its far side and fell.)
        let onto_perch = level.perch_at(exit, PERCH_REACH).map(|(_, q)| q);
        if let Some(q) = onto_perch {
            exit = q.with_y(exit.y);
        }
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
        if onto_perch.is_none() {
            w.exit_velocity = fwd * speed;
        }
        w.aimed = true;
        w.ease_in(from, root);
        debug!("climb: jump onto {h:.2} m{}", if onto_perch.is_some() { ", a post or beam" } else { "" });
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

    /// In the hay: hop out forward and stand up; toward a side of the box with a drop past it, out over its rim and
    /// down into a hang from it (`hay_over_rim`).
    fn hop_out(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform) -> bool {
        if self.state != HAY || self.mv.is_some() {
            return false;
        }
        if self.hay_over_rim(lib, level, root) {
            return true;
        }
        // (Into the stand of the profile held: AC1 has the hop out's end into either, `_tr_l_wait` / `_tr_h_wait`.)
        let end = if self.high { "xx_l_haystack_hop_out_tr_h_wait" } else { HAY_HOP_OUT[1] };
        let Some(clips) = [HAY_HOP_OUT[0], end].iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>() else { return false };
        self.start_chain(clips, vec![HAY_OUT.into(), GROUND.into()], root, Vec3::ZERO);
        true
    }

    /// In the hay, the stick toward a side of the box with a drop past it: AC1's `xx_l_haystack_wait_to_passover_handl`
    /// and `xx_h_passover_handl_030cm`, turning to the way out, a hand on the rim, then (once over, at the move's end) the
    /// passover's pull-down into a hang from the rim (the running game, the Damascus bureau's roof haystack: out over the
    /// side over the street, hanging there).
    fn hay_over_rim(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform) -> bool {
        let p = root.translation;
        let d = self.move_dir.with_y(0.0);
        if d.length() < 0.3 {
            return false;
        }
        let d = d.normalize();
        let Some(hs) = level.haystacks.iter().find(|h| h.contains(p, 0.3)) else { return false };
        let exit = hs.centre + d * (hs.half / d.x.abs().max(d.z.abs()).max(0.5));
        let found = level
            .ledges
            .iter()
            .enumerate()
            .filter(|(_, l)| l.out.dot(d) > 0.7 && (l.a.y - hs.top()).abs() < 0.3 && (l.closest(exit) - exit).with_y(0.0).length() < HAY_OVER_REACH)
            .min_by(|a, b| (a.1.closest(exit) - exit).length().total_cmp(&(b.1.closest(exit) - exit).length()));
        let Some((i, l)) = found else { return false };
        let rim = l.closest(exit);
        if level.ground(rim + l.out * 0.5 + Vec3::Y * 0.2, 0.3, HAY_OVER_DROP).is_some() {
            return false;
        }
        let Some(clips) = ["xx_l_haystack_wait_to_passover_handl", "xx_h_passover_handl_030cm"].iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>() else {
            return false;
        };
        let at = Transform { rotation: facing(l.out), ..*root };
        let travel: Vec3 = clips.iter().map(|c| world_rot(at.rotation) * root_motion_at(c, c.frames())).sum();
        let end = rim - l.out * HAY_OVER_IN;
        self.start_chain(clips, vec![KNEEL.into(), KNEEL.into()], &at, end - (p + travel));
        if let Some(m) = &mut self.mv {
            m.ease_rot = root.rotation * at.rotation.inverse();
        }
        self.hay_over = Some(i);
        debug!("climb: out of the haystack over its rim at {rim:.2}");
        true
    }

    /// Rebound during a wall run: kick off the wall back, or to the side the stick leans to, and fly.
    fn try_rebound(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform) -> bool {
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
        // AC1's own: the eject a hang makes (`rebound_jump`: its rebound takeoffs, at a top or a hold to hang from that way),
        // seen in the running game off the bureau's wall run, back onto a beam's end 3 m behind and right onto one beside.
        let right = (-out).cross(Vec3::Y);
        let side = if want == Vec3::ZERO { 0.0 } else { want.dot(right) };
        let input = Vec2::new(if side.abs() > 0.5 { side.signum() } else { 0.0 }, -1.0);
        if let Some(w) = Self::rebound_jump(lib, level, root, self.normal, input, input.x > -0.5, self.last.clone()) {
            debug!("climb: wall run eject ({phase})");
            *self = w;
            return true;
        }
        let pick = if want == Vec3::ZERO { options.into_iter().next() } else { options.into_iter().max_by(|a, b| a.1.total_cmp(&b.1)) };
        let Some((clip, _)) = pick else { return false };
        debug!("climb: rebound {}", clip.name);
        // At a target the way it pushes off (AC1's `HumanWalling__ReboundJump`, as Banned445's port reads it): straight
        // back off the wall with the stick into it, else along the stick, kept within 89 degrees of straight back; the
        // best target that way, else 7 m out and 3 m down.
        let push = if want == Vec3::ZERO || want.dot(out) > 0.99 {
            out
        } else {
            let side = if out.cross(want).y >= 0.0 { 1.0 } else { -1.0 };
            if want.dot(out) <= 0.0 { Quat::from_rotation_y(REBOUND_SIDE_MAX * side) * out } else { want }
        };
        let p = root.translation;
        let to = jump_target(level, p, push, None).unwrap_or(p + push * REBOUND_FAR - Vec3::Y * REBOUND_FAR_DOWN);
        let (v, _) = ballistic(p, to);
        debug!("climb: wall run rebound at {to:.2}");
        self.queue.clear();
        self.state = FALL.into();
        self.start(clip, FALL.into(), root);
        self.fall_v = Some(v);
        self.can_catch = true;
        true
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
        // Top surface just behind the holds, level with them (nearer in on a thin top: a haystack's rim, round hay that is
        // not solid; the running game pulled up onto it).
        let probe = wrist - self.normal * 0.45 + Vec3::Y * 0.5;
        let Some(top) = [0.45, 0.25, 0.12].iter().find_map(|d| level.ground(wrist - self.normal * *d + Vec3::Y * 0.5, 0.0, 0.8)) else {
            debug!("climb: no top-out: no top behind the hold (probed at {probe:.2})");
            return false;
        };
        // (The hands-together wall hang holds 0.15 m lower than the first frame of the pull-up puts them.) A top lower than
        // the grip, down to `TOP_OUT_LIP`, is a floor behind a lip the hands hold (a parapet's, in Damascus: the floor
        // 0.21 m under the edge refused every pull-up there); the pull-up comes down onto it.
        let below = (wrist.y + GRIP_DOWN) - top.point.y;
        if !(-0.2..=TOP_OUT_LIP).contains(&below) || top.normal.y < 0.8 {
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
        // (A haystack's rim: the hay heaped past it is in the way of the probes, not of the climb into it.)
        let hay = level.haystacks.iter().find(|h| (h.centre - top.point).with_y(0.0).length() < h.half + 0.3 && (top.point.y - h.top()).abs() < 0.3).copied();
        let open = hay.is_some() || [0.5, 1.2].iter().all(|h| level.raycast(front + Vec3::Y * *h, -self.normal, reach).is_none());
        if !headroom || !open || (hay.is_none() && level.inside_solid(stand + Vec3::Y * 0.5)) {
            let wall = [0.5, 1.2].map(|h| level.raycast(front + Vec3::Y * h, -self.normal, 3.0).map(|w| w.dist - 0.3));
            debug!("climb: no top-out: no room to stand at {stand:.2} (headroom {headroom}, open {open}: a wall {wall:.2?} m past the edge)");
            return false;
        }
        // Nothing to stand on beside the hands (a post, the end of a wall): pull up with one hand.
        let along = Vec3::Y.cross(self.normal).normalize_or_zero();
        // (Looked for from the top found to stand on, a little further in: from the wrists the probe stood on a roof's
        // rounded lip, and one side missed the roof there; the running game pulled up there with both hands.)
        let p = top.point - self.normal * ONEHAND_PROBE_IN;
        let drops = |s: f32| level.ground(p + along * ONEHAND_SIDE * s + Vec3::Y * 0.05, 0.0, 0.25).is_none();
        let one = if hay.is_none() && (drops(1.0) || drops(-1.0)) {
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
            // (Standing up in place, AC1's stand: the root steered to `STAND_FROM_WRISTS` in from where the hands held
            // (they grip about 0.15 m out from the edge), where the running game stood after topping out at the bureau.)
            None => {
                let edge = Vec3::new(wrist.x, root.translation.y, wrist.z);
                let to = (edge - self.normal * STAND_FROM_WRISTS - (root.translation + travel(&clips))).with_y(0.0);
                // (Onto a haystack's rim: on into the hay, as the running game went, `xx_h_freestep_footr_to_haystack_01`.)
                match hay.and_then(|h| Some((h.centre, lib.get("xx_h_freestep_footr_to_haystack_01")?))) {
                    Some((centre, dive)) => {
                        let mut clips = clips;
                        clips.push(dive);
                        let correct = centre - (root.translation + travel(&clips));
                        debug!("climb: pull-up onto a haystack's rim, and into it");
                        (clips, correct, HAY)
                    }
                    None => (clips, Vec3::NEG_Y * below.max(0.0) + to, ON_TOP),
                }
            }
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
    /// `xx_l_climb_1m_to_groundentry_<left|right>`, then its stand; or, where the ground stepped onto ends in an edge
    /// facing out from the wall (within 45 degrees of its normal), the turned step-off `..._<side>_90`, ending side on
    /// to the wall along that edge (AC1's choice, 0xDFA0C0: the edge report's normal against the facing, cos 45 degrees).
    fn try_step_off_side(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform, left: bool) -> bool {
        if self.state != "1m" {
            return false;
        }
        let side = if left { "left" } else { "right" };
        // Where a step-off clip lands: ground there, near the feet's height, room to stand and a clear way to it; else
        // up to a metre in toward the wall, the landing corrected onto it (the clips end in front of a roof flush with
        // the wall, the climber hanging 0.6 m out from it).
        let lands = |off: &Arc<Clip>| -> Option<(Vec3, crate::level::Hit)> {
            let end = root.translation + world_rot(root.rotation) * root_motion_at(off, off.frames()).with_z(0.0);
            let (to, ground) = (0..=20).map(|k| end - self.normal * (k as f32 * 0.05)).find_map(|to| level.ground(to, 0.3, STEP_OFF_MAX).map(|g| (to, g)))?;
            let to = to - self.normal * 0.15;
            let way = (to - root.translation).with_y(0.0);
            (!level.inside_solid(ground.point + Vec3::Y * 0.5)
                && level.raycast(root.translation + Vec3::Y * 0.5, way.normalize_or_zero(), way.length()).is_none())
            .then_some((to, ground))
        };
        // The nearer drop from where it lands: out from the wall, or back toward the climber (side on).
        let edge_out = |g: Vec3| {
            let back = (root.translation - g).with_y(0.0).normalize_or_zero();
            let drop = |dir: Vec3| (1..=12).map(|k| k as f32 * 0.05).find(|&d| level.ground(g + dir * d, 0.3, 0.6).is_none());
            match (drop(self.normal), drop(back)) {
                (Some(out), Some(b)) => out <= b,
                (Some(_), None) => true,
                _ => false,
            }
        };
        let plain = lib.get(&format!("xx_l_climb_1m_to_groundentry_{side}"));
        let turned = lib.get(&format!("xx_l_climb_1m_to_groundentry_{side}_90"));
        let pick = turned
            .as_ref()
            .and_then(|c| lands(c).filter(|(to, g)| edge_out(to.with_y(g.point.y))).map(|l| (c.clone(), l, true)))
            .or_else(|| plain.as_ref().and_then(|c| lands(c).map(|l| (c.clone(), l, false))));
        let Some((off, (to, ground), turn)) = pick else { return false };
        let end = root.translation + world_rot(root.rotation) * root_motion_at(&off, off.frames()).with_z(0.0);
        // (The turned step-off stands on the other foot.)
        let foot = if left != turn { "footl" } else { "footr" };
        let stand_name = if turn {
            format!("xx_l_climb_1m_to_groundentry_{side}_90_tr_h_wait_{foot}")
        } else {
            format!("xx_l_climb_1m_to_groundentry_{side}_tr_h_wait_{foot}")
        };
        let Some(stand) = lib.get(&stand_name) else { return false };
        self.queue = vec![Queued::new(stand, GROUND)];
        self.start(off, STEP_DOWN.into(), root);
        if let Some(m) = &mut self.mv {
            m.correct = (to - end).with_y(0.0) + Vec3::Y * (ground.point.y - to.y);
        }
        debug!("climb: stepped off the wall sideways ({side}{}) onto the ground", if turn { ", turned" } else { "" });
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
    /// out of hay. Returns false when none of those applies (leaps are the held legs).
    pub fn legs(&mut self, lib: &mut AnimLib, level: &Level, root: &Transform, input: Vec2) -> bool {
        // On a wall in high profile, the legs: eject off it (AC1's rebound) at the best place to land away from the wall,
        // or along it with the stick to a side (the stick up is a leap up the wall). Nothing to land on: with the stick
        // back, push off backwards and fall, catching what comes. (In low profile the running game does nothing.)
        // (Also as a wall run catches its hang: the running game broke off the catch for the eject, 0.02 s into it.)
        let catching = self.mv.as_ref().is_some_and(|m| m.clip.name.starts_with("xx_h_wallingfront_") && m.clip.name.contains("_tr_hangwall_"));
        let on_wall = catching || matches!(self.state.as_str(), HANGWALL | HANGWALL_OPEN) || self.state.starts_with('1') || self.state.starts_with('2');
        // (Also while looking round or shimmying along the hang: the running game ejected from both, the stick held.)
        let idle = catching || self.mv.as_ref().is_none_or(|m| m.clip.name.contains("lookaround") || m.clip.name.contains("_strafe_"));
        if on_wall && idle && self.high && input.y < 0.5 {
            // (Off the left foot, but to the left off the right: as the running game did.)
            let left = input.x > -0.5;
            // From a wall hang, AC1's rebound pose first, 0.2 s against the wall (`xx_h_hangwall_tr_rebound_<foot>`, the
            // takeoff on the same foot 0.22 s after it, in all four of the running game's ejects); the eject when it ends.
            let pose = (catching || matches!(self.state.as_str(), HANGWALL | HANGWALL_OPEN))
                .then(|| lib.get(&format!("xx_h_hangwall_tr_rebound_{}", if left { "footl" } else { "footr" })))
                .flatten();
            if let Some(pose) = pose
                && Self::rebound_jump(lib, level, root, self.normal, input, left, self.last.clone()).is_some()
            {
                let state = if catching { HANGWALL_OPEN.to_string() } else { self.state.clone() };
                self.queue.clear();
                self.start(pose, state, root);
                self.eject_after = Some((input, left));
                return true;
            }
            if let Some(w) = Self::rebound_jump(lib, level, root, self.normal, input, left, self.last.clone()) {
                *self = w;
                return true;
            }
            if input.y < -0.5 {
                return self.back_eject(lib, root, input.x);
            }
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
        if self.try_rebound(lib, level, root) || self.hop_out(lib, level, root) {
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
        if self.try_rebound(lib, level, root) || self.hop_out(lib, level, root) {
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
                    // AC1's order (`Movement_ChooseAction` 0xDE29E0, `TrySideMoveToClimbHolds` 0xDD48B0): a wall under
                    // the feet again, onto its holds lower down, then level, before the shimmy; up onto them after it.
                    out.extend(side_to_climb("hangfree", dir).into_iter().take(2));
                    out.push(Cand::new(format!("xx_h_hangfree_strafe_{side}_050cm_{strafe}"), to, Feet::Free));
                    out.extend(side_to_climb("hangfree", dir).into_iter().skip(2));
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
            if vertical {
                // Onto the wall's holds, up or down.
                out.push(Cand::new(format!("xx_h_hangwall_{dir}_climb_1m"), "1m", Feet::Wall));
            } else {
                // Sideways in AC1's order (as the free hang's above): onto the holds lower down, level, the shimmy, up.
                let (strafe, to) = if self.state == HANGWALL { ("open", HANGWALL_OPEN) } else { ("close", HANGWALL) };
                let onto = side_to_climb("hangwall", dir);
                out.extend(onto[..2].iter().cloned());
                out.push(Cand::new(format!("xx_h_hangwall_strafe_{}_050cm_{strafe}", side_name(dir)), to, Feet::Wall));
                out.push(onto[2].clone());
            }
            // Round a corner: AC1 has the climbing stance's only (it ends in the hang again where the feet find no holds).
            if !vertical {
                for kind in ["in", "out"] {
                    out.push(Cand::new(format!("xx_l_climb_1m_corner_{}_090_{kind}", side_name(dir)), "1m", Feet::Wall));
                }
            }
            return out;
        }
        // In the climbing stance AC1's move tables pick the move (`climb_table`, `ChooseMove` 0xDFDE90): the pose and the
        // stick's direction give the long move (stick past half) and the short one, each an action whose clip plays;
        // the pose it ends in is the table's (the clip's name says otherwise for `xx_l_climb_2ru_u_2ru`).
        let wall: Vec<(String, String)> = match crate::climb_table::pose_of(&self.state) {
            Some(pose) => {
                let stick = if input.length() > 0.3 { input } else { dir_vector(dir) };
                crate::climb_table::moves(pose, stick)
                    .into_iter()
                    .filter_map(|m| {
                        let clips = lib.graph.action_clips(m.action)?;
                        Some((clips.first()?.first()?.clone(), crate::climb_table::POSES[m.to].to_string()))
                    })
                    .collect()
            }
            None => {
                let mut wall = clip_state_names(lib, &self.state, dir);
                if vertical {
                    wall.sort_by_key(|(_, to)| !to.ends_with(prefer));
                }
                wall
            }
        };
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
                // From a wall hang AC1's ledge-jump table (0x1A2C780) has only the left hand's leap up onto climbing
                // holds, and none down.
                "u" if from == "hangwall" => bases.push((format!("xx_h_climbing_hangwall_tr_climb1m_up_l_hand_{n}"), "1m", Feet::Wall)),
                // From a free hang, down 2 or 3 m onto a wall below: onto its climbing holds (`tr_climb2m_down_`, the
                // hands apart) or a lone ledge's wall hang (`tr_hangwall_down_`); `min` and `max` both tried, the one
                // whose hands land on the holds taken.
                "d" if free => {
                    for k in ["min", "max"] {
                        bases.push((format!("xx_h_climbing_hangfree_tr_climb2m_down_{k}_{n}00"), "2m", Feet::Wall));
                        bases.push((format!("xx_h_climbing_hangfree_tr_hangwall_down_{k}_{n}00"), HANGWALL_OPEN, Feet::Wall));
                    }
                }
                _ if !free && from == "climb1m" => {
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
            // A plain move up or down goes a row of AC1's grid (0.6 m): a hand the clip moves that way more than `GRID_ROW_MIN`
            // takes a hold that far from where it was, not the same one again (the grid snaps holds to rows; off the
            // bureau's wall-run hang, the next hold 1.35 m up, the running game reached for it, ours stepped onto the
            // same hold and shuffled up).
            if plain && vertical {
                let mut start = base.clone();
                sample(&clips[0], 0.0, &mut start, cr.reference);
                let m = start.model(rig);
                let r = world_rot(root.rotation);
                let from = cr.hands.map(|b| root.translation + r * m[b].pos);
                let same = (0..2).any(|k| (end.hands[k].y - from[k].y).abs() > GRID_ROW_MIN && (targets[k].y - from[k].y).abs() < GRID_ROW_MIN);
                if same {
                    if why() {
                        debug!("climb: {} : a hand stays on its hold, no row up or down", c.names[0]);
                    }
                    continue;
                }
            }
            // A plain move needs a hold for each foot two rows (1.2 m) under its hand's, as AC1's grid cells go
            // (`IsGridMoveValid`), or the ground there: off the bureau's hang the holds 9.3 and 9.9 m up had nothing at 8.7
            // for the feet; the running game reached past them, ours stepped up them.
            let grid = plain && c.names.first().is_some_and(|n| n.starts_with("xx_l_climb_"));
            if grid && !targets.iter().all(|t| foot_cell(level, *t, normal)) {
                if why() {
                    debug!("climb: {} : no hold for the feet two rows under the hands", c.names[0]);
                }
                continue;
            }
            // A shimmy along a hang: room for the body on the way (a beam stuck out of the wall under the hold stopped the
            // running game's, at the bureau; ours went through it).
            // (And a leap to the side, past the same.)
            let sideways = c.names.first().is_some_and(|n| n.contains("_strafe_") || (leap && (n.contains("_right_") || n.contains("_left_"))));
            if sideways {
                let (from, to) = (root.translation + Vec3::Y * SHIMMY_CHEST, end.pos + land + Vec3::Y * SHIMMY_CHEST);
                let way = (to - from).with_y(0.0);
                if let Some(h) = level.raycast(from, way.normalize_or_zero(), way.length() + SHIMMY_ROOM).filter(|h| h.normal.dot(normal) < 0.7) {
                    debug!("climb: {} : something in the way {:.2} m along", c.names[0], h.dist);
                    continue;
                }
            }
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
            // (AC1's `IsGridMoveValid` 0xDECD70: each side's foot cell, two rows under its hand, must hold too; the one-hand
            // stances are turned down without, the level ones end in the hang.)
            let feet_held = targets.iter().all(|t| footholds(level, *t, normal));
            if !feet_held && c.to != "1m" && c.to != "2m" && (c.to.starts_with('1') || c.to.starts_with('2')) {
                if why() {
                    debug!("climb: {} : no hold for a foot under its hand", c.names[0]);
                }
                continue;
            }
            if (c.to == "1m" || c.to == "2m") && !feet_held {
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
        let found = wall(fwd, 1.0, 1.0).or_else(|| side.and_then(|d| wall(d, CATCH_SIDE_REACH, -0.5))).map(|h| (h.point, h.normal));
        // (No wall at the chest, as under a ledge over an arch: AC1's catch looks for edges round the hands, wall or
        // not; a hold facing us over the head, its line standing in for the wall's face.)
        let found = found.or_else(|| {
            let hands = root.translation + Vec3::Y * JUMP_HOLD_FREE_HANG;
            level
                .ledges
                .iter()
                .filter(|l| l.out.dot(fwd) < -0.7)
                .map(|l| (l.closest(hands), l.out))
                .filter(|(q, _)| (*q - hands).with_y(0.0).length() < 0.8 && (q.y - hands.y).abs() < 0.6)
                .min_by(|a, b| (a.0 - hands).length().total_cmp(&(b.0 - hands).length()))
        });
        let Some((wall_point, wall_normal)) = found else {
            return false;
        };
        let normal = wall_normal.with_y(0.0).normalize();
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
            let out = (((end.hands[0] + end.hands[1]) * 0.5 - wall_point).dot(normal) - CATCH_HAND_OUT).max(0.0);
            end.hands = end.hands.map(|h| h - normal * out);
            end.feet = end.feet.map(|f| f - normal * out);
            let in_box = |h: Vec3| {
                nearest_hold(level, h, normal).map(|n| n.1).filter(|t| (*t - h).with_y(0.0).length() <= CATCH_ACROSS && (t.y - h.y).abs() <= CATCH_UP)
            };
            let (Some(a), Some(b)) = (in_box(end.hands[0]), in_box(end.hands[1])) else {
                if why() {
                    let off = end.hands.map(|h| nearest_hold(level, h, normal).map(|n| n.1 - h));
                    debug!("climb: catch {} at {:.2}: hands off the holds ({off:.2?})", names[0], root.translation);
                }
                continue;
            };
            let targets = [a, b];
            if self.catch_below.is_some_and(|y| (targets[0].y + targets[1].y) * 0.5 > y) {
                continue;
            }
            let err = (targets[0] - end.hands[0] + targets[1] - end.hands[1]) * 0.5;
            if !feet_fit(level, &end.feet.map(|f| f + err), normal, feet) {
                if why() {
                    debug!("climb: catch {}: the feet do not fit ({feet:?})", names[0]);
                }
                continue;
            }
            let was = root.rotation;
            self.normal = normal;
            self.fall_v = None;
            self.state = LEAP.into();
            let tos = chain_states(clips.len(), LEAP, to);
            // (The pull in to the wall too: caught further out, he would hang that far off it from then on. The root
            // keeps its rotation this frame: the move turns it.)
            let at = Transform { rotation: facing.rotation, ..*root };
            self.start_chain(clips, tos, &at, err - normal * out);
            // (Turned to the wall over the catch's start, not at once: a side grab turned him up to 90 degrees in a
            // frame, the hands jumping a metre.)
            if let Some(m) = &mut self.mv {
                m.ease_rot = was * m.start_rot.inverse();
            }
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
        // (A free jump comes down harder until it falls at `FREE_JUMP_FAST`: off the Damascus bureau's roof the running
        // game's went from 1.9 to 8.6 m/s down in 0.33 s past its top, and ours, under gravity, landed 1.3 m further.)
        let g = if self.free_jump && v.y < 0.0 && v.y > -FREE_JUMP_FAST { FREE_JUMP_GRAVITY } else { GRAVITY };
        v.y -= g * dt;
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
        // (Not the top of the wall just gone over.)
        let ground = ground.filter(|g| self.clear_of.is_none_or(|t| g.point.y < t - 0.2));
        let Some(ground) = ground else {
            if root.translation.y < -100.0 {
                self.fall_v = None;
                self.finished = true;
            }
            return;
        };
        root.translation.y = ground.point.y;
        self.fall_v = None;
        let flat_speed = v.with_y(0.0).length();
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
        // degrees of the motion, the forward landing, else the straight one; either goes on by the speed against a sprint
        // while the stick is held (< 0.2 the wait, < 0.5 the walk, < 0.9 the jog, else the sprint's takeoff), into the
        // wait with it let go.
        let stick = self.move_dir.with_y(0.0);
        let along = v.with_y(0.0).try_normalize().unwrap_or((root.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero());
        let forward = stick.length() > 0.3 && stick.normalize().dot(along) >= LAND_FORWARD_COS;
        // (The straight one with the stick held goes on by the speed too: off a wall run's rebound, slow, AC1 landed straight
        // into the walk, `..._straight_soft_footr_tr_l_walk_footl`, free running, seen in the running game.)
        let into = if stick.length() > 0.3 {
            match flat_speed / LAND_SPRINT_SPEED {
                r if r < 0.2 => LandInto::Wait,
                r if r < 0.5 => LandInto::Walk,
                r if r < 0.9 => LandInto::Jog,
                _ => LandInto::Sprint,
            }
        } else {
            LandInto::from(false, self.high, self.sprint)
        };
        let names: [String; 2] = match running {
            _ if damage >= HEAVY_DAMAGE => LAND_HEAVY.map(String::from),
            // (The roll with the stick held only: AC1 picks it by the stick's speed, not the body's (over 0.2, 0xE05940).
            // Let go, it landed hurt and stood, coming down off a beam into a Damascus street at 7 m/s across.)
            true if (damage > 0.0 || drop > ROLL_LANDING_DROP) && self.move_dir.length() > 0.2 => LAND_DAMAGE_RUN.map(String::from),
            _ if damage > 0.0 => {
                // (AC1's damaging landing goes on into the wait, the walk or the jog.)
                let after = match into {
                    LandInto::Wait => "h_wait_footr",
                    LandInto::Walk => "h_walk_footl",
                    LandInto::Jog | LandInto::Sprint => "h_jog_footl",
                };
                [LAND_DAMAGE.to_string(), format!("{LAND_DAMAGE}_tr_{after}")]
            }
            _ => landing_names(forward, hard_share(drop), into),
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
        // A landing or a jump's reception, the stick held: left for running as soon as its item allows leaving for moving
        // in this profile (AC1's gate word, `forge::graph::Gate`; `HumanGround__AnimAllowsModeExit` 0xD80010), not played
        // out.
        if let Some(m) = &self.mv
            && (m.to == LAND || m.to == GROUND)
            && self.queue.iter().all(|q| q.to == GROUND || q.to == LAND)
            && self.move_dir.with_y(0.0).length() > 0.3
            // (Not before a jump onto a top has brought the root onto it: left early, he stood in the air short of a
            // low wall and walked on through its face.)
            && self.vault_path.is_none()
            // (Nor while still over the gap: AC1 aims the reception to start half a metre short of the edge, its step
            // carrying him onto it; left there, the run found no ground and fell.)
            && level.ground(root.translation, 0.3, 0.3).is_some()
            && lib.graph.gate(&m.clip.name).is_some_and(|g| g.may_leave(true, self.high))
        {
            debug!("climb: {} left for running (its gate allows it)", m.clip.name);
            self.last_clip = Some(m.clip.name.clone());
            if self.exit_velocity.length() < 0.1 {
                let fwd = (root.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
                self.exit_velocity = fwd
                    * if self.sprint {
                        PERCH_OFF_RUN
                    } else if self.high {
                        TOP_OUT_JOG
                    } else {
                        TOP_OUT_WALK
                    };
            }
            self.mv = None;
            self.queue.clear();
            self.state = GROUND.into();
            self.finished = true;
            return;
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
                // (Done by the end of a chain's last clip: a hang squares up to the wall at once after it.)
                let span = if self.queue.is_empty() { EASE_TURN.min(dur) } else { EASE_TURN };
                let k = smoothstep(m.t / span.max(1e-3));
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
            // Move done: the wall normal turns with the clip's own turn of the root (not with an ease still under way:
            // carried on into the next clip, it turned the normal off the wall and the hang squared up with a jump).
            let turn = done.start_rot * root_delta(root_rotation_at(&done.clip, done.clip.frames())) * done.start_rot.inverse();
            self.normal = turn * self.normal;
            self.state = done.to;
            // The ledge stop's start played with the stick still at the edge (within 70 degrees): back on the ground at
            // once, where the pull-down takes it (AC1 went from `xx_h_ledge_stop_start` straight into
            // `xx_l_ledge_stop_start_footl_pulldown_front_orientation`, 0.27 s after stopping; ours played the stop's end
            // first, a second).
            if done.clip.name.starts_with("xx_h_ledge_stop_start_")
                && self.move_dir.with_y(0.0).normalize_or_zero().dot(-self.normal.with_y(0.0).normalize_or_zero()) >= 0.342
            {
                self.queue.clear();
                self.state = GROUND.into();
                self.finished = true;
                return;
            }
            // (The rebound pose played: the eject asked for.)
            if let Some((input, left)) = self.eject_after.take()
                && let Some(w) = Self::rebound_jump(lib, level, root, self.normal, input, left, self.last.clone())
            {
                *self = w;
                return;
            }
            // Topping a wall run with nothing to grab and the legs held: AC1 kicks off it backwards
            // (`xx_h_wallingfront_step1_footr_tr_rebound_footr_a/b`, then `xx_h_rebound_footr_tr_fall`, seen in the
            // running game, Damascus), not the slide down (`..._step1_footr_tr_fall`): away from the wall, it catches
            // nothing on the way down.
            if done.clip.name == WALL_RUN[2]
                && self.legs_held
                && self.queue.first().is_some_and(|q| q.clip.name == WALL_RUN_FALL)
                && let Some(clips) = WALL_RUN_REBOUND.iter().map(|n| lib.get(n)).collect::<Option<Vec<_>>>()
            {
                let n = clips.len();
                self.queue = clips.into_iter().enumerate().map(|(k, c)| Queued::new(c, if k + 1 == n { FALL } else { WALL_RUN_STATE })).collect();
                self.can_catch = false;
                // (Flying off backwards and down as `rebound_footr_tr_fall` moves, 0.8 m back and 3 m down in 0.47 s: AC1
                // lands 0.65 m out, 1.96 m down in 0.27 s, falling at a steady 7 m/s; from rest ours took 0.65 s.)
                self.fall_with = Some(self.normal.with_y(0.0).normalize_or_zero() * REBOUND_OFF_SPEED + Vec3::NEG_Y * REBOUND_DROP_SPEED);
                debug!("climb: the wall run tops out with nothing to grab: rebound off it");
            }
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
                // (AC1's stand-up through the free step, the stick pushed on: off it at the gait's pace, as the running
                // game ran on from it at once after topping out at the bureau.)
                if on && next.clip.name == STAND_UP {
                    self.exit_velocity = (root.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero() * if self.sprint { TOP_OUT_JOG } else { TOP_OUT_WALK };
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
                // Crossfade the seam over the blend AC1's move graph gives that way (the transition's own time), else by how
                // far the hands and feet jump (AC1 chains do not all line up).
                if let Some(last) = &self.last {
                    let len = match lib.graph.transition_blend(&done.clip.name, &next.clip.name).filter(|t| *t > 0.0) {
                        Some(t) => t,
                        None => {
                            let mut next_pose = base.clone();
                            sample(&next.clip, 0.0, &mut next_pose, cr.reference);
                            (seam_gap(rig, cr, last, &next_pose) * 0.3).clamp(0.05, 0.25)
                        }
                    };
                    self.fade = Some((last.clone(), len, len));
                }
                // (A turn still easing in carries on into the next clip: cut at a short clip's end, a catch turned him
                // 90 degrees in four frames.)
                let left = if done.ease_rot != Quat::IDENTITY { done.ease_rot.slerp(Quat::IDENTITY, smoothstep(done.t / EASE_TURN)) } else { Quat::IDENTITY };
                self.mv = Some(Move {
                    clip: next.clip,
                    t: 0.0,
                    to: next.to,
                    start: root.translation,
                    start_rot: left.inverse() * root.rotation,
                    correct: next.correct,
                    rate: next.rate,
                    ease_rot: left,
                });
                return;
            }
            if self.state == ON_TOP || self.state == GROUND {
                self.finished = true;
                return;
            }
            // Over a haystack's rim: down into the hang from it.
            if let Some(i) = self.hay_over.take() {
                let l = level.ledges[i];
                let edge = l.closest(root.translation);
                let first = ["xx_h_passover_handl_030cm_pulldown_soft_orientation", "xx_h_passover_handl_030cm_pulldown_soft"];
                if let Some(w) = Self::pull_down_onto(lib, level, root, rig, base, cr, &l, edge, first, self.last.clone()) {
                    *self = w;
                    return;
                }
                debug!("climb: no hang from the haystack's rim");
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
            self.lean_t += dt;
            self.lean_step(lib, level, root);
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
        // Hanging still from a swing bar, the stick forward: swing up again.
        if self.bar.is_some()
            && is_free(&self.state)
            && self.mv.is_none()
            && input.y > 0.5
            && let Some(c) = lib.get(SWING_MOMENTUM)
        {
            debug!("climb: swinging up again on the bar");
            self.start(c, SWING.into(), root);
            return;
        }
        let moved = input.length() > 0.3 && !self.hop_out(lib, level, root) && self.try_move(lib, level, root, rig, base, cr, input, leap);
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
        if let Some(cy) = &self.cycle {
            match &cy.intro {
                Some((c, t, _)) => sample(c, t * FPS, &mut pose, cr.reference),
                None => sample(&cy.clips[cy.foot], cy.phase * cy.clips[cy.foot].frames(), &mut pose, cr.reference),
            }
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
        assert_eq!(landing_names(true, 0.0, LandInto::Wait)[0], "xx_h_landing_forward_soft_footr_tr_h_wait_footr_a");
        assert_eq!(landing_names(false, 1.0, LandInto::Sprint)[1], "xx_h_landing_straight_hard_footr_tr_h_sprint_impultion_footl_b");
        assert_eq!(landing_names(false, 0.0, LandInto::Walk)[0], "xx_h_landing_straight_soft_footr_tr_l_walk_footl_a");
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
        assert_eq!(landing_damage(5.0, 1.0), 0.0);
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
