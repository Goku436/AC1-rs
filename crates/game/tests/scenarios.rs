//! Scripted scenarios: the game run with its test hooks (see the header of `src/main.rs`) through climbs,
//! jumps, the course, hiding and the ragdoll, each checked against what it logs. They open a window
//! and take a few minutes, so they run only with `AC1_SCENARIOS=1` and the game installed (`AC1_GAME_DIR` or
//! the default Steam path); otherwise the test passes without running them.
//!
//!     AC1_SCENARIOS=1 cargo test -p ac1 --test scenarios -- --nocapture

use std::path::PathBuf;
use std::process::Command;

const DEFAULT_GAME_DIR: &str = "P:/SteamLibrary/steamapps/common/Assassins Creed";

struct Scenario {
    name: &'static str,
    env: &'static [(&'static str, &'static str)],
    secs: f32,
    /// Every one of these must appear in the log.
    want: &'static [&'static str],
    /// None of these may.
    never: &'static [&'static str],
}

/// Games run at once by default (each its own process): the scenarios are independent. 6 measured fastest without
/// timing flakes on the dev machine (56 scenarios: 1 at a time 560 s, 4 161 s, 6 123 s, 8 104 s with one flake).
const JOBS: usize = 6;

const QUIET: &[(&str, &str)] = &[("AC1_NO_CROWD", "1")];

const SCENARIOS: &[Scenario] = &[
    Scenario {
        name: "standing turn round in high profile breaks off into the run",
        env: &[("AC1_START", "-5,-5,0"), ("AC1_WALK", "4.5"), ("AC1_STEER", "180"), ("AC1_HIGH", "0-5")],
        secs: 3.0,
        want: &["waitturn_right_180", "turn broken off into the run"],
        never: &["_tr_h_jog_hipm_"],
    },
    Scenario {
        name: "sprint stop on the course ramp stays on it (the stop follows the slope)",
        env: &[("AC1_START", "24.5,-0.95,-90"), ("AC1_WALK", "6.2"), ("AC1_STOP", "0.55"), ("AC1_EMBED_CHECK", "1")],
        secs: 2.5,
        want: &["-> ground via xx_h_sprintstop_"],
        never: &["EMBED"],
    },
    Scenario {
        name: "running up the stairs in high profile: steps, not a low obstacle to collide with",
        env: &[("AC1_START", "0,0.5,0"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-5")],
        secs: 3.0,
        want: &["xx_h_jumpstraight_clear_footall_tr_fall"],
        never: &["collide_full_footl_050cm_a", "EMBED"],
    },
    Scenario {
        name: "walking slowly along the beam to B4 steps off its end onto the roof",
        env: &[("AC1_START", "62.0,33,-90,5"), ("AC1_WALK", "1.9")],
        secs: 3.0,
        want: &["onto perch", "off the beam's end onto the ground"],
        never: &["EMBED"],
    },
    Scenario {
        name: "free running at T2's beam a little off its line: onto the beam, the leap of faith from its end",
        env: &[("AC1_START", "61.1,9.48,104,13"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-6"), ("AC1_LEGS", "0-6")],
        secs: 5.0,
        want: &["-> hay via xx_h_faith_jump_landing"],
        never: &["landing_death", "EMBED"],
    },
    Scenario {
        name: "two-step lane: onto the fence, on to the first post (not past it)",
        env: &[("AC1_START", "37,-30,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "0-5")],
        secs: 4.0,
        want: &["jump onto 1.00 m", "aimed at [45.00, 1.80", "aimed at [47.60, 1.80", "via xx_h_air_up_300cm_footl_to_freestep_tr_freestep_entry_footr"],
        never: &["EMBED"],
    },
    Scenario {
        name: "two-step lane met 20 degrees off its line: fence, then both posts",
        env: &[("AC1_START", "38.5,-28.6,-70"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "0-5")],
        secs: 4.5,
        want: &["jump onto 1.00 m", "aimed at [45.00, 1.80", "aimed at [47.60, 1.80"],
        never: &["EMBED"],
    },
    Scenario {
        name: "rooftops: B4 up to B5 with AC1's up jump (not the long planned arc)",
        env: &[("AC1_START", "66.5,34,0,5"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-3"), ("AC1_LEGS", "0-3")],
        secs: 3.0,
        want: &["aimed at [66.50, 5.69,", "AC1's jump tables: xx_h_run_up_300cm_footl_to_air"],
        never: &["EMBED"],
    },
    Scenario {
        name: "wall W: the stick swung left along the wall (by the camera, not the body) rebounds left",
        env: &[("AC1_START", "33,13,180,1"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "0-1.6"), ("AC1_JUMP", "1.75"), ("AC1_VEER", "1.6,90")],
        secs: 4.0,
        want: &["rebound xx_h_wallingfront_step1rebound_left_footl"],
        never: &["EMBED"],
    },
    Scenario {
        name: "side grab: free running off the ledge's end, the stick turned to the wall grabs it in the air",
        env: &[("AC1_START", "-5,-50.5,-90,3"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-4"), ("AC1_LEGS", "0-4"), ("AC1_VEER", "1.6,90")],
        secs: 3.5,
        want: &["leap -> 2m via xx_fall_tr_climb_min_b"],
        never: &["landed from", "EMBED"],
    },
    Scenario {
        name: "side grab: walking off the ledge with the legs held and the stick to the wall grabs it",
        env: &[("AC1_START", "-5,-50.5,-90,3"), ("AC1_WALK", "1.9"), ("AC1_LEGS", "3.4-6"), ("AC1_VEER", "3.3,90")],
        secs: 6.0,
        want: &["leap -> 2m via xx_fall_tr_climb_min_b"],
        never: &["landed from", "EMBED"],
    },
    Scenario {
        name: "jog turn-around on open ground (no body swing in a frame: checked by the pops tool, not here)",
        env: &[("AC1_START", "20,-45,-90"), ("AC1_WALK", "3.5"), ("AC1_TURN", "2")],
        secs: 3.5,
        want: &["via xx_h_runturn180_footl_tr_walk_hipm_footl"],
        never: &["EMBED"],
    },
    Scenario {
        name: "balancing on a post, free running on: AC1's free-step jump to the next post",
        env: &[("AC1_START", "38.6,0,-90,3"), ("AC1_WALK", "6.2"), ("AC1_STOP", "0.0-1.0")],
        secs: 3.0,
        want: &["via xx_h_freestep_front_front_300cm_footl_to_air", "onto perch 6 at [41.00, 2.00"],
        never: &["EMBED"],
    },
    Scenario {
        name: "balancing on a post, the stick back: the free-step jump back to the last post without turning first",
        env: &[("AC1_START", "38.6,0,-90,3"), ("AC1_WALK", "6.2"), ("AC1_STOP", "0.0-1.0"), ("AC1_STEER", "180")],
        secs: 3.0,
        want: &["via xx_h_freestep_backright_front_300cm_footl_to_air", "onto perch 4 at [36.20, 2.00"],
        never: &["EMBED", "waitturn"],
    },
    Scenario {
        name: "running onto the beam to B4 at 15 degrees: onto it once, along it, off its end (it flickered ground/perch)",
        env: &[("AC1_START", "57.5,32.06,-90,5"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-4"), ("AC1_STEER", "-15")],
        secs: 3.0,
        want: &["onto perch 9", "off the beam's end onto the ground"],
        never: &["stepped off the perch", "EMBED"],
    },
    Scenario {
        name: "standing turn round with the stick sweeping: one turn, then the walk",
        env: &[("AC1_START", "-5,-5,0"), ("AC1_WALK", "1.5"), ("AC1_STEER", "180"), ("AC1_CURVE", "1.5")],
        secs: 4.5,
        want: &["resume into gait"],
        never: &["_tr_l_walk_hipm_"],
    },
    Scenario {
        name: "up block L's ladder in high profile, off the top into a free step",
        env: &[("AC1_START", "23.5,-5.4,0"), ("AC1_CLIMB", "up=8"), ("AC1_HIGH", "0-9")],
        secs: 6.0,
        want: &["via xx_h_ladder_climb_up_", "via xx_h_ladder_climb_up_l_tr_freestep_footl_b"],
        never: &["EMBED"],
    },
    Scenario {
        name: "rebound off block L's ladder: the legs with the stick pulled back",
        env: &[("AC1_START", "23.5,-5.4,0"), ("AC1_CLIMB", "up=2.5,down=3"), ("AC1_LEGS", "4.2-4.3")],
        secs: 7.0,
        want: &["rebound off the ladder", "via xx_h_rebound_footr_tr_fall", "land -> ground"],
        never: &["EMBED", "landing_damage_footl_roll"],
    },
    Scenario {
        name: "free running through both swing bars, the legs only held",
        env: &[("AC1_START", "50.3,0,-90"), ("AC1_WALK", "6"), ("AC1_HIGH", "0-7"), ("AC1_LEGS", "0-7")],
        secs: 6.0,
        want: &[
            "caught bar 0",
            "caught bar 1",
            "swing -> leap via xx_h_swing_cycle_front_300cm_to_air",
            "land -> ground",
            "jumping at the swing bar",
            "_to_swing_tr_swing_front_a",
        ],
        never: &["EMBED"],
    },
    Scenario {
        name: "across the wall onto E's ladder, up it, off it onto the wall",
        env: &[("AC1_START", "11.3,16.7,0"), ("AC1_CLIMB", "left=2.5,up=2,right=3")],
        secs: 10.0,
        want: &["across onto ladder 0", "off ladder 0 onto the wall"],
        never: &["EMBED"],
    },
    Scenario {
        name: "climbing wall A with the stick held on tops out straight into the walk",
        env: &[("AC1_START", "0,4.0,180"), ("AC1_CLIMB", "up=14"), ("AC1_WALK", "1.9")],
        secs: 14.0,
        want: &["topping out on into l_walk", "via xx_h_hangknee_footl_tr_l_walk_footr_b"],
        never: &["EMBED"],
    },
    Scenario {
        name: "at the bottom of E's wall, sideways past the last hold steps off onto the ground",
        env: &[("AC1_START", "11.3,16.7,0"), ("AC1_CLIMB", "right=8")],
        secs: 8.0,
        want: &["stepped off the wall sideways (right)", "via xx_l_climb_1m_to_groundentry_right_tr_h_wait_footr"],
        never: &["EMBED"],
    },
    Scenario {
        name: "climb wall A to the top",
        env: &[("AC1_START", "0,4.0,180"), ("AC1_CLIMB", "up=14")],
        secs: 14.0,
        want: &["-> top via"],
        never: &["EMBED"],
    },
    Scenario {
        name: "climbing along A turns the inside corner onto B, then round B's outside corner",
        env: &[("AC1_START", "-1.4,4.0,180"), ("AC1_CLIMB", "right=6")],
        secs: 8.0,
        want: &["via xx_l_climb_1m_corner_right_090_in", "via xx_l_climb_1m_corner_right_090_out"],
        never: &["EMBED"],
    },
    Scenario {
        name: "up B1's west face, round its corner at the top onto the bare north face: the wall hang, not the climbing stance",
        env: &[("AC1_START", "38.9,35.86,-90"), ("AC1_CLIMB", "up=4.1,none=1.5,right=3")],
        secs: 11.0,
        want: &["corner_right_090_out : no holds for the feet there", "via xx_h_hangwall_strafe_right_050cm"],
        never: &["EMBED", "via xx_l_climb_1m_r_2m at [4"],
    },
    Scenario {
        name: "running jump at the 1.4 m passover wall (0.3 m deep): a hand on its top, over it (AC1's passover) and down",
        env: &[("AC1_START", "-18,-34,0"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-4"), ("AC1_JUMP", "0.9")],
        secs: 3.5,
        want: &["_to_passover", "via xx_h_passover_handr_030cm_tr_fall", "landed from 1."],
        never: &["EMBED", "not in AC1's move graph", "landed from -"],
    },
    Scenario {
        name: "running jump at the 1.5 m passover wall (1 m deep): AC1's 1 m passover",
        env: &[("AC1_START", "-12,-34,0"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-4"), ("AC1_JUMP", "0.9")],
        secs: 3.5,
        want: &["via xx_h_passover_handr_100cm_tr_fall", "landed from 1."],
        never: &["EMBED", "landed from -"],
    },
    Scenario {
        name: "running jump at G's lowest ledge (bare wall under it): AC1's flight onto the surface and hangwall reception",
        env: &[("AC1_START", "-14.5,-3,0"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-4"), ("AC1_JUMP", "1.2")],
        secs: 4.0,
        want: &["_to_surface", "-> hangwall_open via xx_hangwall_reception_front_straight_max_c"],
        never: &["EMBED", "not in AC1's move graph"],
    },
    Scenario {
        name: "standing on pillar P facing away from its holds, let go: down over its side to hang (AC1's pilotis pull-down)",
        env: &[("AC1_START", "-1.2,12.0,90,4"), ("AC1_CLIMB", "nograb,wait=1.5,drop=3")],
        secs: 4.5,
        want: &["via xx_l_beam_pilotis_to_pulldown_soft_right_orientation", "drop -> hangwall via xx_l_ledge_pulldown_soft_to_hangwall_straight_b"],
        never: &["EMBED"],
    },
    Scenario {
        name: "walking along T2's beam to its end over the hay: AC1's edge stop, then turned round on it and back along",
        env: &[("AC1_START", "59,10,90,13"), ("AC1_WALK", "1.9"), ("AC1_STOP", "3.5-4.5"), ("AC1_TURN", "4.5")],
        secs: 7.0,
        want: &["via xx_l_beam_edge_stop_tr_crouchwait_footr_b", "via xx_l_beam_crouchwait_footr_turn180_tr_footl", "off the beam's end onto the ground"],
        never: &["EMBED", "not in AC1's move graph"],
    },
    Scenario {
        name: "crouched at T2's beam end, the stick across it: a quarter turn to face across",
        env: &[("AC1_START", "59,10,90,13"), ("AC1_WALK", "1.9"), ("AC1_STOP", "2.0-3.0"), ("AC1_VEER", "3.0,90")],
        secs: 5.0,
        want: &["via xx_l_beam_crouchwait_footr_turn_left_to_crouchwait_90"],
        never: &["EMBED"],
    },
    Scenario {
        name: "leaping left along A across the 1.5 m gap onto C: AC1's 2 m leap mixed toward its 3 m one",
        env: &[("AC1_START", "1.6,4.0,180"), ("AC1_CLIMB", "up=1.5,none=1,leap-left=3")],
        secs: 8.0,
        want: &["leap mixed", "climb1m_left_2_c at [4.59"],
        never: &["EMBED"],
    },
    Scenario {
        name: "hanging from B1's bare north face, down held with nowhere to go: he looks down, then back",
        env: &[("AC1_START", "38.9,35.86,-90"), ("AC1_CLIMB", "up=4.1,none=1.5,right=2,none=1,down=2,none=1")],
        secs: 15.0,
        want: &["looking down", "looking back"],
        never: &["EMBED"],
    },
    Scenario {
        name: "speed-stepping along A stops short of B, turns the corner outside B",
        env: &[("AC1_START", "-1.4,4.0,180"), ("AC1_CLIMB", "leap-right=6")],
        secs: 8.0,
        want: &["via xx_l_climb_1m_corner_right_090_in"],
        never: &["EMBED"],
    },
    Scenario {
        name: "free running off D at an angle jumps to C's holds",
        env: &[("AC1_START", "15,5.1,125,4.5"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "0-5")],
        secs: 4.0,
        want: &["running jump aimed at [10.96", "-> 2m via xx_fall_tr_climb"],
        never: &["landed from 3", "EMBED"],
    },
    Scenario {
        name: "rooftops: B1 to B2, down to B3, the beam to B4",
        env: &[("AC1_START", "41,33,-90,8"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-4.6"), ("AC1_LEGS", "0-4.6")],
        secs: 5.0,
        want: &["5.99, 33.00] (takeoff and flight from AC1's jump tables", "4.49, 33.00] (takeoff and flight from AC1's jump tables", "onto perch"],
        never: &["damage 0.2", "EMBED"],
    },
    Scenario {
        name: "rooftops: B3 back up to B2 (1.5 m higher) catches its edge (a lone ledge: AC1's reception into the wall hang)",
        env: &[("AC1_START", "60,33,90,5"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-3"), ("AC1_LEGS", "0-3")],
        secs: 3.0,
        want: &["running jump at the wall's ledge [53.06", "-> hangwall_open via xx_hangwall_reception_front_straight_max_c"],
        never: &["landed from", "EMBED"],
    },
    Scenario {
        name: "rooftops: B4 up to B5, both swing bars, B6, then the jump to tower T2's holds",
        env: &[("AC1_START", "66.5,35,0,6"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-12"), ("AC1_LEGS", "0-12")],
        secs: 7.0,
        want: &["aimed at [66.50, 5.69", "caught bar 2", "caught bar 3", "landing_forward_soft"],
        never: &["EMBED"],
    },
    Scenario {
        name: "rooftops: from B6 across to T2's holds, hanging at the wall, up to its top",
        env: &[("AC1_START", "68.5,10,90,6"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-3"), ("AC1_LEGS", "0-3"), ("AC1_CLIMB", "nograb,wait=2,up=11")],
        secs: 16.0,
        want: &["-> 2m via xx_fall_tr_climb_min_b at [62.55", "-> top via"],
        never: &["EMBED"],
    },
    Scenario {
        name: "wall W: through the doorway, up the wall, rebound back onto the holds over the door",
        env: &[("AC1_START", "33,13,180,1"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "0-1.6"), ("AC1_JUMP", "1.75")],
        secs: 4.0,
        want: &["step1rebound_back", "-> free_1m via xx_fall_tr_hangfree"],
        never: &["EMBED"],
    },
    Scenario {
        name: "free running past hiding spot A in high profile runs on, not into the hay",
        env: &[("AC1_START", "13.5,-12.5,180,1"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-2"), ("AC1_LEGS", "0.5-2")],
        secs: 2.5,
        want: &[],
        never: &["into the haystack"],
    },
    Scenario {
        name: "free hang on C, drop",
        env: &[("AC1_START", "6.5,4.0,180"), ("AC1_CLIMB", "up=1.6,left=3.5,drop=4")],
        secs: 9.5,
        want: &["free_1m", "land -> ground"],
        never: &["EMBED"],
    },
    Scenario {
        name: "wall run on A to a ledge hang",
        env: &[("AC1_START", "0,0.5,180"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-1.5"), ("AC1_LEGS", "0-1.5")],
        secs: 3.0,
        want: &["wallingfront", "hangwall_open"],
        never: &["EMBED"],
    },
    Scenario { name: "jump up to hang on G", env: &[("AC1_START", "-14.5,-11.4,0"), ("AC1_JUMP", "0.5")], secs: 4.0, want: &["hangwall"], never: &[] },
    Scenario {
        name: "jump onto 1.75 m with a blend",
        env: &[("AC1_START", "-2,-11.4,0"), ("AC1_JUMP", "0.5")],
        secs: 4.0,
        want: &["mixing xx_h_jumpstraight_footl_to_hangknee_footl_150cm", "-> top via"],
        never: &[],
    },
    Scenario {
        name: "leap of faith into hay",
        env: &[("AC1_START", "13.5,-5.5,0"), ("AC1_WALK", "1.6"), ("AC1_JUMP", "0.8")],
        secs: 5.0,
        want: &["-> hay via xx_h_faith_jump_landing"],
        never: &[],
    },
    Scenario {
        name: "the free-run course reaches the bars",
        env: &[("AC1_START", "18,0,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-14"), ("AC1_LEGS", "0-14")],
        secs: 14.0,
        want: &["jump onto", "caught bar"],
        never: &[],
    },
    Scenario {
        name: "the flow lane: jump onto 0.6 and 1.2 m, on to the last wall",
        env: &[("AC1_START", "-20,-24.5,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-14"), ("AC1_LEGS", "0-14")],
        secs: 12.0,
        want: &["jump onto 0.60 m", "jump onto 1.00 m"],
        never: &["EMBED", "passover"],
    },
    Scenario {
        name: "running into the 0.6 m block stops against it, the legs step up onto it",
        env: &[("AC1_START", "-20,-24.5,-90"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-8"), ("AC1_LEGS", "3.0-3.1")],
        secs: 5.0,
        want: &["stopped against a 0.60 m obstacle", "xx_h_collide_full_footl_050cm_wait=0.50", "step up from against the obstacle"],
        never: &["EMBED"],
    },
    Scenario {
        name: "running at the 0.6 m block at an angle glances off it",
        env: &[("AC1_START", "-17.5,-23.6,-90"), ("AC1_STEER", "40"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-5")],
        secs: 3.0,
        want: &["glance off a 0.60 m obstacle"],
        never: &["EMBED"],
    },
    Scenario {
        name: "standing, steering left turns a quarter; letting go ends it standing",
        env: &[("AC1_START", "-25,-8,0"), ("AC1_WALK", "1.9"), ("AC1_STEER", "90"), ("AC1_STOP", "0.25")],
        secs: 2.5,
        want: &["waitturn_left_090", "let go mid-turn: ends in xx_l_waitturn_left_090_footl_tr_l_wait_hipm_footl"],
        never: &[],
    },
    Scenario {
        name: "ladder to the top",
        env: &[("AC1_START", "23.5,-4.8,0"), ("AC1_JUMP", "0.5"), ("AC1_CLIMB", "nograb,up=14")],
        secs: 14.0,
        want: &["ladder", "-> top via"],
        never: &[],
    },
    Scenario {
        name: "pole P to its perch",
        env: &[("AC1_START", "-1.2,10.9,180"), ("AC1_JUMP", "0.5"), ("AC1_CLIMB", "nograb,up=8")],
        secs: 8.0,
        want: &["-> perch via"],
        never: &[],
    },
    Scenario {
        name: "sit on the bench",
        env: &[("AC1_START", "-9.7,-6.6,0"), ("AC1_WALK", "1.0"), ("AC1_STOP", "0.6"), ("AC1_LEGS", "0.9-0.95")],
        secs: 4.0,
        want: &["bench_sit -> bench"],
        never: &[],
    },
    Scenario {
        name: "stop from a run, blended",
        env: &[("AC1_START", "-25,-8,0"), ("AC1_WALK", "4.3"), ("AC1_STOP", "1.5")],
        secs: 3.0,
        want: &["mixing xx_h_jogstop"],
        never: &[],
    },
    Scenario {
        name: "free running up a wall leaps hold to hold and climbs over the top",
        env: &[("AC1_START", "0,0.5,180"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-14"), ("AC1_LEGS", "0-14"), ("AC1_CLIMB", "nograb,wait=3,leap-up=11")],
        secs: 9.0,
        want: &["climb1m_up_r_hand_2", "1m -> hangknee via xx_l_climb_1m_tr_hangknee_footl_a", "-> top via"],
        never: &["EMBED"],
    },
    Scenario {
        name: "risky leap from a low row of E catches the slab with one hand",
        env: &[("AC1_START", "8.6,11.3,180"), ("AC1_CLIMB", "up=0.8,wait=5,leap-right=5")],
        secs: 10.5,
        want: &["leap -> free_1m via xx_h_climbing_climb1m_tr_hangfree_right_3_c"],
        never: &["climb1m_tr_hangfree_right_2"],
    },
    Scenario {
        name: "jump to pole Q hangs hands together, then pulls up onto its cap",
        env: &[("AC1_START", "1.2,11.1,180"), ("AC1_JUMP", "0.5"), ("AC1_CLIMB", "nograb,wait=3.5,up=4")],
        secs: 9.0,
        want: &["-> hangwall via xx_h_jumpstraight_footl_to_hangwall_250cm", "xx_h_hangwall_onehand_to_hangknee_onehand_footl_a", "-> perch via"],
        never: &["hangfree"],
    },
    Scenario {
        name: "leap of faith off tower V's beam into the hay",
        env: &[("AC1_START", "23.5,17.7,180"), ("AC1_LEGS", "1.5-1.6")],
        secs: 6.0,
        want: &["onto perch", "leap of faith off the perch", "-> hay via xx_h_faith_jump_landing"],
        never: &[],
    },
    Scenario {
        name: "free running past block L stops at it, not through it onto the kiosk",
        env: &[("AC1_START", "14,-7.5,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-8"), ("AC1_LEGS", "0-8")],
        secs: 6.0,
        want: &["wallrun"],
        never: &["onto kiosk frame", "EMBED"],
    },
    Scenario {
        name: "standing at block A's edge in high profile, pull down and climb down to the ground",
        env: &[
            ("AC1_START", "0,5.6,0"),
            ("AC1_WALK", "1.5"),
            ("AC1_STOP", "1.2"),
            ("AC1_HIGH", "0-6"),
            ("AC1_LEGS", "2.6-2.7"),
            ("AC1_CLIMB", "nograb,wait=4,down=6"),
        ],
        secs: 11.0,
        want: &["pull down onto the ledge", "stepdown -> ground"],
        never: &["EMBED"],
    },
    Scenario {
        name: "reversing on block A's top in high profile does not run off it",
        env: &[("AC1_START", "0,6.6,0"), ("AC1_WALK", "4.5"), ("AC1_HIGH", "0-5"), ("AC1_TURN", "0.55")],
        secs: 4.0,
        want: &["ledge stop"],
        never: &["landed from 6"],
    },
    Scenario {
        name: "reversing a jog turns round with AC1's run turn-around",
        env: &[("AC1_START", "-25,-8,0"), ("AC1_WALK", "4.0"), ("AC1_TURN", "1.2")],
        secs: 2.8,
        want: &["via xx_h_runturn180_", "_tr_walk_hipm_"],
        never: &["waitturn"],
    },
    Scenario {
        name: "reversing while free running swings round without stopping",
        env: &[("AC1_START", "-25,-8,0"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "0-5"), ("AC1_TURN", "1.2")],
        secs: 2.6,
        want: &[],
        never: &["runturn180", "runstop", "waitturn"],
    },
    Scenario {
        name: "go limp and settle",
        env: &[("AC1_START", "-25,-8,0"), ("AC1_LIMP", "1.0")],
        // (Settling takes up to about 4 s after going limp: at 5 s it failed now and then.)
        secs: 6.0,
        want: &["went limp", "settled limp"],
        never: &[],
    },
];

fn game_dir() -> Option<PathBuf> {
    let d = PathBuf::from(std::env::var("AC1_GAME_DIR").unwrap_or_else(|_| DEFAULT_GAME_DIR.into()));
    d.join("DataPC.forge").exists().then_some(d)
}

fn run(exe: &str, s: &Scenario, k: usize) -> Result<(), String> {
    // (No screenshot, `-`: the run ends at `secs`; only the log is checked. `AC1_SCENARIO_SHOTS=1` saves them.)
    let shot = if std::env::var("AC1_SCENARIO_SHOTS").is_ok() {
        std::env::temp_dir().join(format!("ac1-scenario-{}-{k}.png", std::process::id()))
    } else {
        std::path::PathBuf::from("-")
    };
    let mut c = Command::new(exe);
    c.env("RUST_LOG", "ac1=debug,wgpu=error").env("NO_COLOR", "1").env("AC1_SHOT", &shot).env("AC1_SHOT_SECS", s.secs.to_string()).env("AC1_EMBED_CHECK", "1");
    c.envs(QUIET.iter().copied());
    c.envs(s.env.iter().copied());
    let out = c.output().map_err(|e| format!("could not run the game: {e}"))?;
    let log = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    // (`AC1_SCENARIO_LOGS=dir` keeps each game's log there, by the scenario's number.)
    if let Ok(dir) = std::env::var("AC1_SCENARIO_LOGS") {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(PathBuf::from(dir).join(format!("{k:02}.log")), format!("{}\n{log}", s.name));
    }
    if log.contains("panicked") {
        return Err("the game panicked".into());
    }
    // Steps between clips that AC1's move graph does not have (see `forge::graph`): reported, not failed.
    let mut off_graph: Vec<&str> = log.lines().filter(|l| l.contains("not in AC1's move graph")).filter_map(|l| l.split_once("graph: ").map(|x| x.1)).collect();
    off_graph.sort();
    off_graph.dedup();
    for l in off_graph {
        eprintln!("  off the move graph: {l}");
    }
    let missing: Vec<&str> = s.want.iter().copied().filter(|w| !log.contains(w)).collect();
    let present: Vec<&str> = s.never.iter().copied().filter(|w| log.contains(w)).collect();
    if missing.is_empty() && present.is_empty() { Ok(()) } else { Err(format!("missing {missing:?}, unwanted {present:?}")) }
}

#[test]
fn scenarios() {
    if std::env::var("AC1_SCENARIOS").is_err() {
        eprintln!("scenarios skipped (set AC1_SCENARIOS=1 to run them)");
        return;
    }
    let Some(dir) = game_dir() else {
        eprintln!("scenarios skipped: no game install found");
        return;
    };
    // SAFETY: set before any thread of this test starts a game.
    unsafe { std::env::set_var("AC1_GAME_DIR", &dir) };
    let exe = env!("CARGO_BIN_EXE_ac1");
    // `AC1_SCENARIO_FILTER`: only the scenarios whose name contains it. `AC1_SCENARIO_JOBS`: how many games run at once
    // (each its own process and window; default `JOBS`).
    let filter = std::env::var("AC1_SCENARIO_FILTER").unwrap_or_default();
    let picked: Vec<&Scenario> = SCENARIOS.iter().filter(|s| s.name.contains(filter.as_str())).collect();
    let jobs = std::env::var("AC1_SCENARIO_JOBS").ok().and_then(|j| j.parse().ok()).unwrap_or(JOBS).max(1);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<std::sync::Mutex<Option<Result<(), String>>>> = picked.iter().map(|_| std::sync::Mutex::new(None)).collect();
    let started = std::time::Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..jobs {
            scope.spawn(|| {
                loop {
                    let k = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some(s) = picked.get(k) else { break };
                    let r = run(exe, s, k);
                    eprintln!("{} {}{}", if r.is_ok() { "PASS" } else { "FAIL" }, s.name, r.as_ref().err().map_or(String::new(), |e| format!(": {e}")));
                    *results[k].lock().unwrap() = Some(r);
                }
            });
        }
    });
    let failed: Vec<&str> = picked.iter().zip(&results).filter(|(_, r)| !matches!(*r.lock().unwrap(), Some(Ok(())))).map(|(s, _)| s.name).collect();
    eprintln!("{} scenarios, {jobs} at a time, in {:.0} s", picked.len(), started.elapsed().as_secs_f32());
    assert!(failed.is_empty(), "{} of {} scenarios failed: {failed:?}", failed.len(), picked.len());
}
