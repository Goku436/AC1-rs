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
    /// The systems it goes through (`AC1_SCENARIO_TAGS` picks by these): ground, jump, climb, hang, beam, ladder, bars,
    /// hay, vault, wallrun, ragdoll, misc.
    tags: &'static [&'static str],
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
        tags: &["ground"],
        env: &[("AC1_START", "-5,-5,0"), ("AC1_WALK", "4.5"), ("AC1_STEER", "180"), ("AC1_HIGH", "0-5")],
        secs: 3.0,
        want: &["waitturn_right_180", "turn broken off into the run"],
        never: &["_tr_h_jog_hipm_"],
    },
    Scenario {
        name: "sprint stop on the course ramp stays on it (the stop follows the slope)",
        tags: &["ground"],
        env: &[("AC1_START", "24.5,-0.95,-90"), ("AC1_WALK", "6.2"), ("AC1_STOP", "0.55"), ("AC1_EMBED_CHECK", "1")],
        secs: 2.5,
        want: &["-> ground via xx_h_sprintstop_"],
        never: &["EMBED"],
    },
    Scenario {
        name: "running up the stairs in high profile: steps, not a low obstacle to collide with",
        tags: &["ground", "vault"],
        env: &[("AC1_START", "0,0.5,0"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-5")],
        secs: 3.0,
        want: &["xx_h_jumpstraight_clear_footall_tr_fall"],
        never: &["collide_full_footl_050cm_a", "EMBED"],
    },
    Scenario {
        name: "walking slowly along the beam to B4 steps off its end onto the roof",
        tags: &["beam"],
        env: &[("AC1_START", "62.0,33,-90,5"), ("AC1_WALK", "1.9")],
        secs: 3.0,
        want: &["onto perch", "off the beam's end onto the ground"],
        never: &["EMBED"],
    },
    Scenario {
        name: "free running at T2's beam a little off its line: onto the beam, the leap of faith from its end",
        tags: &["beam", "jump", "hay"],
        env: &[("AC1_START", "61.1,9.48,104,13"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-6"), ("AC1_LEGS", "0-6")],
        secs: 5.0,
        want: &["-> hay via xx_h_faith_jump_landing"],
        never: &["landing_death", "EMBED"],
    },
    Scenario {
        name: "two-step lane: onto the fence, on to the first post (not past it)",
        tags: &["jump", "beam", "vault"],
        env: &[("AC1_START", "37,-30,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "0-5")],
        secs: 4.0,
        want: &["jump onto 1.00 m", "aimed at [45.00, 1.80", "aimed at [47.60, 1.80", "via xx_h_air_up_300cm_footl_to_freestep_tr_freestep_entry_footr"],
        never: &["EMBED"],
    },
    Scenario {
        name: "two-step lane met 20 degrees off its line: fence, then both posts",
        tags: &["jump", "beam", "vault"],
        env: &[("AC1_START", "38.5,-28.6,-70"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "0-5")],
        secs: 4.5,
        want: &["jump onto 1.00 m", "aimed at [45.00, 1.80", "aimed at [47.60, 1.80"],
        never: &["EMBED"],
    },
    Scenario {
        name: "rooftops: B4 up to B5 with AC1's up jump (not the long planned arc)",
        tags: &["jump"],
        env: &[("AC1_START", "66.5,34,0,5"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-3"), ("AC1_LEGS", "0-3")],
        secs: 3.0,
        want: &["aimed at [66.50, 5.69,", "AC1's jump tables: xx_h_run_up_300cm_foot"],
        never: &["EMBED"],
    },
    Scenario {
        name: "wall W: the stick swung left along the wall (by the camera, not the body) rebounds left",
        tags: &["wallrun", "climb"],
        env: &[("AC1_START", "33,13,180,1"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "0-1.6"), ("AC1_JUMP", "1.75"), ("AC1_VEER", "1.6,90")],
        secs: 4.0,
        want: &["rebound xx_h_wallingfront_step1rebound_left_footl"],
        never: &["EMBED"],
    },
    Scenario {
        name: "side grab: free running off the ledge's end, the stick turned to the wall grabs it in the air",
        tags: &["jump", "hang"],
        env: &[("AC1_START", "-5,-50.5,-90,3"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-4"), ("AC1_LEGS", "0-4"), ("AC1_VEER", "1.6,90")],
        secs: 3.5,
        want: &["leap -> 2m via xx_fall_tr_climb_min_b"],
        never: &["landed from", "EMBED"],
    },
    Scenario {
        name: "side grab: walking off the ledge with the legs held and the stick to the wall grabs it",
        tags: &["jump", "hang"],
        // (He walks off 0.28 m past the ledge's end, AC1's rim rule: the stick turns as he does.)
        env: &[("AC1_START", "-5,-50.5,-90,3"), ("AC1_WALK", "1.9"), ("AC1_LEGS", "3.55-6"), ("AC1_VEER", "3.45,90")],
        secs: 6.0,
        want: &["leap -> 2m via xx_fall_tr_climb_min_b"],
        never: &["landed from", "EMBED"],
    },
    Scenario {
        name: "jog turn-around on open ground (no body swing in a frame: checked by the pops tool, not here)",
        tags: &["ground"],
        env: &[("AC1_START", "20,-45,-90"), ("AC1_WALK", "3.5"), ("AC1_TURN", "2")],
        secs: 3.5,
        // (Either foot: the one the turn comes on depends on where in the step it is pressed.)
        want: &["via xx_h_runturn180_foot", "_tr_walk_hipm_foot"],
        never: &["EMBED"],
    },
    Scenario {
        name: "balancing on a post, free running on: AC1's free-step jump to the next post",
        tags: &["beam", "jump"],
        env: &[("AC1_START", "38.6,0,-90,3"), ("AC1_WALK", "6.2"), ("AC1_STOP", "0.0-1.0")],
        secs: 3.0,
        want: &["via xx_h_freestep_front_front_300cm_footl_to_air", "onto perch 6 at [4"],
        never: &["EMBED"],
    },
    Scenario {
        name: "balancing on a post, the stick back: the free-step jump back to the last post without turning first",
        tags: &["beam", "jump"],
        env: &[("AC1_START", "38.6,0,-90,3"), ("AC1_WALK", "6.2"), ("AC1_STOP", "0.0-1.0"), ("AC1_STEER", "180")],
        secs: 3.0,
        want: &["via xx_h_freestep_backright_front_300cm_footl_to_air", "onto perch 4 at [3"],
        never: &["EMBED", "waitturn"],
    },
    Scenario {
        name: "running onto the beam to B4 at 15 degrees: onto it once, along it, off its end (it flickered ground/perch)",
        tags: &["beam", "ground"],
        env: &[("AC1_START", "57.5,32.06,-90,5"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-4"), ("AC1_STEER", "-15")],
        secs: 3.0,
        want: &["onto perch 9", "off the beam's end onto the ground"],
        never: &["stepped off the perch", "EMBED"],
    },
    Scenario {
        name: "standing turn round with the stick sweeping: one turn, then the walk",
        tags: &["ground"],
        env: &[("AC1_START", "-5,-5,0"), ("AC1_WALK", "1.5"), ("AC1_STEER", "180"), ("AC1_CURVE", "1.5")],
        secs: 4.5,
        want: &["resume into gait"],
        never: &["_tr_l_walk_hipm_"],
    },
    Scenario {
        name: "free running at block L's ladder: up the wall and onto the ladder (AC1's wall run before the ladder)",
        tags: &["ladder", "wallrun"],
        env: &[("AC1_START", "23.5,2.0,0"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "0-5")],
        secs: 3.0,
        want: &["wall run onto ladder", "-> ladder_l via xx_h_wallingfront_step1_footr_tr_h_ladder_up_l"],
        never: &["EMBED"],
    },
    Scenario {
        name: "up block L's ladder in high profile, off the top into a free step",
        tags: &["ladder"],
        env: &[("AC1_START", "23.5,-5.4,0"), ("AC1_CLIMB", "up=8"), ("AC1_HIGH", "0-9")],
        secs: 6.0,
        want: &["via xx_h_ladder_climb_up_", "via xx_h_ladder_climb_up_l_tr_freestep_footl_b"],
        never: &["EMBED"],
    },
    Scenario {
        name: "rebound off block L's ladder: the legs with the stick pulled back",
        tags: &["ladder", "jump"],
        env: &[("AC1_START", "23.5,-5.4,0"), ("AC1_CLIMB", "up=2.5,down=3"), ("AC1_LEGS", "4.2-4.3")],
        secs: 7.0,
        want: &["rebound off the ladder", "via xx_h_rebound_footr_tr_fall", "land -> ground"],
        never: &["EMBED", "landing_damage_footl_roll"],
    },
    Scenario {
        name: "free running through both swing bars, the legs only held",
        tags: &["bars", "jump"],
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
        tags: &["climb", "ladder"],
        env: &[("AC1_START", "11.3,16.7,0"), ("AC1_CLIMB", "left=2.5,up=2,right=3")],
        secs: 10.0,
        want: &["across onto ladder 0", "off ladder 0 onto the wall"],
        never: &["EMBED"],
    },
    Scenario {
        name: "climbing wall A with the stick held on tops out straight into the walk",
        tags: &["climb", "ground"],
        env: &[("AC1_START", "0,4.0,180"), ("AC1_CLIMB", "up=14"), ("AC1_WALK", "1.9")],
        secs: 14.0,
        want: &["topping out on into l_walk", "via xx_h_hangknee_footl_tr_l_walk_footr_b"],
        never: &["EMBED"],
    },
    Scenario {
        name: "at the bottom of E's wall, sideways past the last hold steps off onto the ground",
        tags: &["climb", "ground"],
        env: &[("AC1_START", "11.3,16.7,0"), ("AC1_CLIMB", "right=8")],
        secs: 8.0,
        want: &["stepped off the wall sideways (right)", "via xx_l_climb_1m_to_groundentry_right_tr_h_wait_footr"],
        never: &["EMBED"],
    },
    Scenario {
        name: "climb wall A to the top",
        tags: &["climb"],
        env: &[("AC1_START", "0,4.0,180"), ("AC1_CLIMB", "up=14")],
        secs: 14.0,
        want: &["-> top via"],
        never: &["EMBED"],
    },
    Scenario {
        name: "climbing along A turns the inside corner onto B, then round B's outside corner",
        tags: &["climb"],
        env: &[("AC1_START", "-1.4,4.0,180"), ("AC1_CLIMB", "right=6")],
        secs: 8.0,
        want: &["via xx_l_climb_1m_corner_right_090_in", "via xx_l_climb_1m_corner_right_090_out"],
        never: &["EMBED"],
    },
    Scenario {
        name: "up B1's west face, round its corner at the top onto the bare north face: the wall hang, not the climbing stance",
        tags: &["climb", "hang"],
        env: &[("AC1_START", "38.9,35.86,-90"), ("AC1_CLIMB", "up=4.1,none=1.5,right=3")],
        secs: 11.0,
        want: &["corner_right_090_out : no holds for the feet there", "via xx_h_hangwall_strafe_right_050cm"],
        never: &["EMBED", "via xx_l_climb_1m_r_2m at [4"],
    },
    Scenario {
        name: "climbing A with the stick up and right: AC1's diagonal moves (1m_ur_2ru, 2ru_ur_1m)",
        tags: &["climb"],
        env: &[("AC1_START", "1.0,4.0,180"), ("AC1_CLIMB", "upright=4")],
        secs: 6.0,
        want: &["via xx_l_climb_1m_ur_2ru", "via xx_l_climb_2ru_ur_1m"],
        never: &["EMBED"],
    },
    Scenario {
        name: "walking in low profile toward the 2.5 m block's edge: AC1's edge halt, stopped at it, not off it",
        tags: &["ground"],
        env: &[("AC1_START", "-5,-13,180,3"), ("AC1_WALK", "1.9"), ("AC1_STICK", "1")],
        secs: 4.0,
        want: &[],
        never: &["landed from", "fall -> fall", "EMBED"],
    },
    Scenario {
        name: "running jump at the 1.4 m passover wall (0.3 m deep): a hand on its top, over it (AC1's passover) and down",
        tags: &["jump", "hang"],
        env: &[("AC1_START", "-18,-34,0"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-4"), ("AC1_JUMP", "0.9")],
        secs: 3.5,
        want: &["_to_passover", "via xx_h_passover_hand", "_030cm_tr_fall", "landed from 1."],
        never: &["EMBED", "not in AC1's move graph", "landed from -"],
    },
    Scenario {
        name: "free running at the 1.4 m passover wall: AC1's wall run over it, a hand on its top, and down",
        tags: &["wallrun", "jump"],
        env: &[("AC1_START", "-18,-34,0"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-4"), ("AC1_LEGS", "0-4")],
        secs: 3.5,
        want: &["wall run over the 1.40 m wall", "via xx_h_passover_handl_030cm_tr_fall", "landed from 1."],
        never: &["EMBED", "landed from -"],
    },
    Scenario {
        name: "passover over a 4.4 m drop: over the wall and round into the wall hang on its far side",
        tags: &["jump", "hang"],
        env: &[("AC1_START", "-27,-40,0,3.6"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-4"), ("AC1_JUMP", "0.3")],
        secs: 3.0,
        want: &["into the hang on its far side", "-> hangwall via xx_h_passover_hand"],
        never: &["EMBED", "squaring up", "landed from"],
    },
    Scenario {
        name: "running jump at the 1.5 m passover wall (1 m deep): AC1's 1 m passover",
        tags: &["jump", "hang"],
        env: &[("AC1_START", "-12,-34,0"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-4"), ("AC1_JUMP", "0.9")],
        secs: 3.5,
        want: &["via xx_h_passover_hand", "_100cm_tr_fall", "landed from 1."],
        never: &["EMBED", "landed from -"],
    },
    Scenario {
        name: "running jump at G's lowest ledge (bare wall under it): AC1's flight onto the surface and hangwall reception",
        tags: &["jump", "hang"],
        env: &[("AC1_START", "-14.5,-3,0"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-4"), ("AC1_JUMP", "1.2")],
        secs: 4.0,
        want: &["_to_surface", "-> hangwall_open via xx_hangwall_reception_front_straight_max_c"],
        never: &["EMBED", "not in AC1's move graph"],
    },
    Scenario {
        name: "standing on pillar P facing away from its holds, let go: down over its side to hang (AC1's pilotis pull-down)",
        tags: &["beam", "hang"],
        env: &[("AC1_START", "-1.2,12.0,90,4"), ("AC1_CLIMB", "nograb,wait=1.5,drop=3")],
        secs: 4.5,
        want: &["via xx_l_beam_pilotis_to_pulldown_soft_right_orientation", "drop -> hangwall via xx_l_ledge_pulldown_soft_to_hangwall_straight_b"],
        never: &["EMBED"],
    },
    Scenario {
        name: "walking along T2's beam to its end over the hay: stopped 0.3 m short (as AC1), then turned round on it and back along",
        tags: &["beam"],
        env: &[("AC1_START", "59,10,90,13"), ("AC1_WALK", "1.9"), ("AC1_STOP", "3.5-4.5"), ("AC1_TURN", "4.5")],
        secs: 7.0,
        want: &["stopped short of the beam's end", "_turn180_tr_foot", "off the beam's end onto the ground"],
        never: &["EMBED", "not in AC1's move graph"],
    },
    Scenario {
        name: "crouched at T2's beam end, the stick across it: a quarter turn to face across",
        tags: &["beam"],
        env: &[("AC1_START", "59,10,90,13"), ("AC1_WALK", "1.9"), ("AC1_STOP", "2.0-3.0"), ("AC1_VEER", "3.0,90")],
        secs: 5.0,
        want: &["_turn_left_to_crouchwait_90"],
        never: &["EMBED"],
    },
    Scenario {
        name: "leaping left along A across the 1.5 m gap onto C: AC1's 2 m leap mixed toward its 3 m one",
        tags: &["climb"],
        env: &[("AC1_START", "1.6,4.0,180"), ("AC1_CLIMB", "up=1.5,none=1,leap-left=3")],
        secs: 8.0,
        want: &["leap mixed", "climb1m_left_2_c at [4.59"],
        never: &["EMBED"],
    },
    Scenario {
        name: "hanging from B1's bare north face, down held with nowhere to go: he looks down, then back",
        tags: &["hang"],
        env: &[("AC1_START", "38.9,35.86,-90"), ("AC1_CLIMB", "up=4.1,none=1.5,right=2,none=1,down=2,none=1")],
        secs: 15.0,
        want: &["looking down", "looking back"],
        never: &["EMBED"],
    },
    Scenario {
        name: "speed-stepping along A stops short of B, turns the corner outside B",
        tags: &["climb"],
        env: &[("AC1_START", "-1.4,4.0,180"), ("AC1_CLIMB", "leap-right=6")],
        secs: 8.0,
        want: &["via xx_l_climb_1m_corner_right_090_in"],
        never: &["EMBED"],
    },
    Scenario {
        name: "free running off D at an angle jumps to C's holds",
        tags: &["jump", "climb"],
        env: &[("AC1_START", "15,5.1,125,4.5"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "0-5")],
        secs: 4.0,
        want: &["running jump aimed at [10.96", "-> 2m via xx_fall_tr_climb"],
        never: &["landed from 3", "EMBED"],
    },
    Scenario {
        name: "rooftops: B1 to B2, down to B3, the beam to B4",
        tags: &["jump", "beam"],
        env: &[("AC1_START", "41,33,-90,8"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-4.6"), ("AC1_LEGS", "0-4.6")],
        secs: 5.0,
        want: &["5.99, 33.00] (takeoff and flight from AC1's jump tables", "4.49, 33.00] (takeoff and flight from AC1's jump tables", "onto perch"],
        never: &["damage 0.2", "EMBED"],
    },
    Scenario {
        name: "rooftops: B3 back up to B2 (1.5 m higher) catches its edge (a lone ledge: AC1's reception into the wall hang)",
        tags: &["jump", "hang"],
        env: &[("AC1_START", "60,33,90,5"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-3"), ("AC1_LEGS", "0-3")],
        secs: 3.0,
        want: &["running jump at the wall's ledge [53.06", "-> hangwall_open via xx_hangwall_reception_front_straight_max_c"],
        never: &["landed from", "EMBED"],
    },
    Scenario {
        name: "rooftops: B4 up to B5, both swing bars, B6, then the jump to tower T2's holds",
        tags: &["jump", "bars", "climb"],
        env: &[("AC1_START", "66.5,35,0,6"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-12"), ("AC1_LEGS", "0-12")],
        secs: 7.0,
        want: &["aimed at [66.50, 5.69", "caught bar 2", "caught bar 3", "landing_forward_"],
        never: &["EMBED"],
    },
    Scenario {
        name: "rooftops: from B6 across to T2's holds, hanging at the wall, up to its top",
        tags: &["jump", "hang", "climb"],
        env: &[("AC1_START", "68.5,10,90,6"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-3"), ("AC1_LEGS", "0-3"), ("AC1_CLIMB", "nograb,wait=2,up=11")],
        secs: 16.0,
        want: &["-> 2m via xx_fall_tr_climb_min_b at [62.55", "-> top via"],
        never: &["EMBED"],
    },
    Scenario {
        name: "wall W: through the doorway, up the wall, rebound back onto the holds over the door",
        tags: &["wallrun", "climb"],
        env: &[("AC1_START", "33,13,180,1"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "0-1.6"), ("AC1_JUMP", "1.75")],
        secs: 4.0,
        want: &["step1rebound_back", "-> free_1m via xx_fall_tr_hangfree"],
        never: &["EMBED"],
    },
    Scenario {
        name: "free running past hiding spot A in high profile runs on, not into the hay",
        tags: &["hay", "ground"],
        env: &[("AC1_START", "13.5,-12.5,180,1"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-2"), ("AC1_LEGS", "0.5-2")],
        secs: 2.5,
        want: &[],
        never: &["into the haystack"],
    },
    Scenario {
        name: "free hang on C, drop",
        tags: &["hang"],
        env: &[("AC1_START", "6.5,4.0,180"), ("AC1_CLIMB", "up=1.6,left=3.5,drop=4")],
        secs: 9.5,
        want: &["free_1m", "land -> ground"],
        never: &["EMBED"],
    },
    Scenario {
        name: "wall run on A to a ledge hang",
        tags: &["wallrun", "hang"],
        env: &[("AC1_START", "0,0.5,180"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-1.5"), ("AC1_LEGS", "0-1.5")],
        secs: 3.0,
        want: &["wallingfront", "hangwall_open"],
        never: &["EMBED"],
    },
    Scenario {
        name: "jump up to hang on G",
        tags: &["jump", "hang"],
        env: &[("AC1_START", "-14.5,-11.4,0"), ("AC1_JUMP", "0.5")],
        secs: 4.0,
        want: &["hangwall"],
        never: &[],
    },
    Scenario {
        name: "jump onto 1.75 m with a blend",
        tags: &["jump", "vault"],
        env: &[("AC1_START", "-2,-11.4,0"), ("AC1_JUMP", "0.5")],
        secs: 4.0,
        want: &["mixing xx_h_jumpstraight_footl_to_hangknee_footl_150cm", "-> top via"],
        never: &[],
    },
    Scenario {
        name: "leap of faith into hay",
        tags: &["hay", "jump"],
        env: &[("AC1_START", "13.5,-5.5,0"), ("AC1_WALK", "1.6"), ("AC1_JUMP", "0.8")],
        secs: 5.0,
        want: &["-> hay via xx_h_faith_jump_landing"],
        never: &[],
    },
    Scenario {
        name: "the free-run course reaches the bars",
        tags: &["vault", "jump", "beam", "bars"],
        env: &[("AC1_START", "18,0,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-14"), ("AC1_LEGS", "0-14")],
        secs: 14.0,
        want: &["jump onto", "caught bar"],
        never: &[],
    },
    Scenario {
        name: "the flow lane: jump onto 0.6 and 1.2 m, on to the last wall",
        tags: &["vault", "jump"],
        env: &[("AC1_START", "-20,-24.5,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-14"), ("AC1_LEGS", "0-14")],
        secs: 12.0,
        want: &["jump onto 0.60 m", "jump onto 1.00 m"],
        never: &["EMBED", "passover"],
    },
    Scenario {
        name: "running into the 0.6 m block stops against it, the legs step up onto it",
        tags: &["vault"],
        env: &[("AC1_START", "-20,-24.5,-90"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-8"), ("AC1_LEGS", "3.0-3.1")],
        secs: 5.0,
        want: &["stopped against a 0.60 m obstacle", "xx_h_collide_full_footl_050cm_wait=0.50", "step up from against the obstacle"],
        never: &["EMBED"],
    },
    Scenario {
        name: "running at the 0.6 m block at an angle glances off it",
        tags: &["vault"],
        env: &[("AC1_START", "-17.5,-23.6,-90"), ("AC1_STEER", "40"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-5")],
        secs: 3.0,
        want: &["glance off a 0.60 m obstacle"],
        never: &["EMBED"],
    },
    Scenario {
        name: "standing, steering left turns a quarter; letting go ends it standing",
        tags: &["ground"],
        env: &[("AC1_START", "-25,-8,0"), ("AC1_WALK", "1.9"), ("AC1_STEER", "90"), ("AC1_STOP", "0.25")],
        secs: 2.5,
        want: &["waitturn_left_090", "let go mid-turn: ends in xx_l_waitturn_left_090_footl_tr_l_wait_hipm_footl"],
        never: &[],
    },
    Scenario {
        name: "ladder to the top",
        tags: &["ladder"],
        env: &[("AC1_START", "23.5,-4.8,0"), ("AC1_JUMP", "0.5"), ("AC1_CLIMB", "nograb,up=14")],
        secs: 14.0,
        want: &["ladder", "-> top via"],
        never: &[],
    },
    Scenario {
        name: "pole P to its perch",
        tags: &["beam", "hang"],
        env: &[("AC1_START", "-1.2,10.9,180"), ("AC1_JUMP", "0.5"), ("AC1_CLIMB", "nograb,up=8")],
        secs: 8.0,
        want: &["-> perch via"],
        never: &[],
    },
    Scenario {
        name: "sit on the bench",
        tags: &["misc"],
        env: &[("AC1_START", "-9.7,-6.6,0"), ("AC1_WALK", "1.0"), ("AC1_STOP", "0.6"), ("AC1_LEGS", "0.9-0.95")],
        secs: 4.0,
        want: &["bench_sit -> bench"],
        never: &[],
    },
    Scenario {
        name: "stop from a run, blended",
        tags: &["ground"],
        env: &[("AC1_START", "-25,-8,0"), ("AC1_WALK", "4.3"), ("AC1_STOP", "1.5")],
        secs: 3.0,
        want: &["mixing xx_h_jogstop"],
        never: &[],
    },
    Scenario {
        name: "free running up a wall leaps hold to hold and climbs over the top",
        tags: &["wallrun", "climb"],
        env: &[("AC1_START", "0,0.5,180"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-14"), ("AC1_LEGS", "0-14"), ("AC1_CLIMB", "nograb,wait=3,leap-up=11")],
        secs: 9.0,
        want: &["climb1m_up_r_hand_2", "1m -> hangknee via xx_l_climb_1m_tr_hangknee_footl_a", "-> top via"],
        never: &["EMBED"],
    },
    Scenario {
        name: "risky leap from a low row of E catches the slab with one hand",
        tags: &["climb", "hang"],
        env: &[("AC1_START", "8.6,11.3,180"), ("AC1_CLIMB", "up=0.8,wait=5,leap-right=5")],
        secs: 10.5,
        want: &["leap -> free_1m via xx_h_climbing_climb1m_tr_hangfree_right_3_c"],
        never: &["climb1m_tr_hangfree_right_2"],
    },
    Scenario {
        name: "jump to pole Q hangs hands together, then pulls up onto its cap",
        tags: &["jump", "hang"],
        env: &[("AC1_START", "1.2,11.1,180"), ("AC1_JUMP", "0.5"), ("AC1_CLIMB", "nograb,wait=3.5,up=4")],
        secs: 9.0,
        want: &["-> hangwall via xx_h_jumpstraight_footl_to_hangwall_250cm", "xx_h_hangwall_onehand_to_hangknee_onehand_footl_a", "-> perch via"],
        never: &["hangfree"],
    },
    Scenario {
        name: "leap of faith off tower V's beam into the hay",
        tags: &["beam", "hay"],
        env: &[("AC1_START", "23.5,17.7,180"), ("AC1_LEGS", "1.5-1.6")],
        secs: 6.0,
        want: &["onto perch", "leap of faith off the perch", "-> hay via xx_h_faith_jump_landing"],
        never: &[],
    },
    Scenario {
        name: "free running past block L stops at it, not through it onto the kiosk",
        tags: &["ground", "vault"],
        env: &[("AC1_START", "14,-7.5,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-8"), ("AC1_LEGS", "0-8")],
        secs: 6.0,
        want: &["wallrun"],
        never: &["onto kiosk frame", "EMBED"],
    },
    Scenario {
        // (As the running AC1 does at a Damascus roof's edge: the ledge stop, then still pushing at the edge, down onto it.)
        name: "walking to block A's edge in high profile, still pushing: pull down and climb down to the ground",
        tags: &["hang", "climb"],
        env: &[("AC1_START", "0,5.6,0"), ("AC1_WALK", "1.5"), ("AC1_STOP", "2.4"), ("AC1_HIGH", "0-6"), ("AC1_CLIMB", "nograb,wait=4,down=6")],
        secs: 11.0,
        want: &["pull down onto the ledge", "stepdown -> ground"],
        never: &["EMBED"],
    },
    Scenario {
        name: "reversing on block A's top in high profile does not run off it",
        tags: &["ground"],
        env: &[("AC1_START", "0,6.6,0"), ("AC1_WALK", "4.5"), ("AC1_HIGH", "0-5"), ("AC1_TURN", "0.55")],
        secs: 4.0,
        want: &["ledge stop"],
        never: &["landed from 6"],
    },
    Scenario {
        name: "reversing a jog turns round with AC1's run turn-around",
        tags: &["ground"],
        env: &[("AC1_START", "-25,-8,0"), ("AC1_WALK", "4.0"), ("AC1_TURN", "1.2")],
        secs: 2.8,
        want: &["via xx_h_runturn180_", "_tr_walk_hipm_"],
        never: &["waitturn"],
    },
    Scenario {
        name: "reversing while free running swings round without stopping",
        tags: &["ground"],
        env: &[("AC1_START", "-25,-8,0"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "0-5"), ("AC1_TURN", "1.2")],
        secs: 2.6,
        want: &[],
        never: &["runturn180", "runstop", "waitturn"],
    },
    Scenario {
        name: "go limp and settle",
        tags: &["ragdoll"],
        env: &[("AC1_START", "-25,-8,0"), ("AC1_LIMP", "1.0")],
        // (Settling takes up to about 4 s after going limp: at 5 s it failed now and then, at 6 s under load.)
        secs: 8.0,
        want: &["went limp", "settled limp"],
        never: &[],
    },
    Scenario {
        name: "gauntlet G1: up box L, hopping two side beams stuck out of a wall, the swing bar, down onto a third, on to a platform",
        tags: &["combo", "beam", "bars"],
        env: &[("AC1_START", "-100,35,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-14"), ("AC1_LEGS", "0-14")],
        secs: 12.0,
        want: &[
            "running jump aimed at [-86.60, 2.00, 35.00]",
            "running jump aimed at [-84.20, 2.00, 35.00]",
            "jumping at the swing bar at [-82.20, 4.40",
            "caught bar 4",
            "steering onto the perch at [-79.20, 1.50",
            "-> perch via xx_h_beam_landing_soft_tr_pilotis_wait_a at [-79.",
            "perch -> jump via xx_h_beam_pilotis_tr_impultionstraight_a at [-79.20",
            "landed from 0.4 m",
        ],
        never: &["EMBED"],
    },
    Scenario {
        name: "gauntlet G2: free running onto the ladder, up it, across the beam bridge, the leap of faith into the hay",
        tags: &["combo", "ladder", "beam", "hay"],
        env: &[("AC1_START", "-96,44,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-20"), ("AC1_LEGS", "0-20"), ("AC1_CLIMB", "nograb,wait=1.5,up=8")],
        secs: 16.0,
        want: &[
            "wall run onto ladder 3",
            "-> ladder_l via xx_h_wallingfront_step1_footr_tr_h_ladder_up_l",
            "-> top via xx_h_ladder_climb_up_l_tr_freestep_footl_b",
            "onto perch 14",
            "onto perch 15",
            "-> faith via xx_h_freestep_footr_to_faith_jump",
            "-> hay via xx_h_faith_jump_landing",
            "hay -> hayout via xx_l_haystack_hop_out",
        ],
        never: &["EMBED"],
    },
    Scenario {
        name: "gauntlet G3: up the wall leaping hold to hold, over the top, across the roof gap, off the far end into the roll",
        tags: &["combo", "wallrun", "climb", "jump"],
        env: &[("AC1_START", "-96,55,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-20"), ("AC1_LEGS", "0-20"), ("AC1_CLIMB", "nograb,wait=2.5,leap-up=8")],
        secs: 16.0,
        want: &[
            "tr_hangwall_430cm_000cm_a",
            "climb1m_up_l_hand_2_c",
            "-> top via xx_h_hangknee_footl_tr_h_jog_footr_b",
            "running jump aimed at [-84.0",
            "-> land via xx_h_landing_damage_foot",
            "via xx_roll_hipm_tr_h_jog",
        ],
        never: &["EMBED"],
    },
    Scenario {
        name: "gauntlet G4: step up, jump onto 1.2 m, up the ramp, the 2.5 m gap, down across a gap, down, the passover",
        tags: &["combo", "vault", "jump"],
        env: &[("AC1_START", "-98,64,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-14"), ("AC1_LEGS", "0-14")],
        secs: 12.0,
        want: &["jump onto 0.60 m", "jump onto 0.9", "wall run over the 1.40 m wall", "_030cm_tr_fall"],
        never: &["EMBED", "landed from 2.", "landed from 3."],
    },
    Scenario {
        name: "gauntlet G5: zigzag post to post, onto the beam, along it onto a box, the dive into the hay beside it",
        tags: &["combo", "beam", "hay"],
        env: &[
            ("AC1_START", "-100,74,-90"),
            ("AC1_WALK", "6.2"),
            ("AC1_HIGH", "0-7"),
            ("AC1_LEGS", "0-7"),
            ("AC1_PATH", "-89.5,74;-86.6,74;-84.4,75;-82.2,74;-80,75;-77.8,74;-72.5,74;stop"),
            ("AC1_JUMP", "8.5"),
        ],
        secs: 12.0,
        want: &[
            "aimed at [-86.60, 2.00, 74.00]",
            "aimed at [-84.40, 2.00, 75.00]",
            "aimed at [-82.20, 2.00, 74.00]",
            "aimed at [-80.00, 2.00, 75.00]",
            "onto perch 20",
            "dive into the haystack at [-70.10",
        ],
        never: &["EMBED"],
    },
    Scenario {
        name: "gauntlet G6: two swing bars, down onto a post, post to post, onto the beam and along it",
        tags: &["combo", "bars", "beam"],
        env: &[("AC1_START", "-100,84,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-14"), ("AC1_LEGS", "0-14")],
        secs: 12.0,
        want: &[
            "caught bar 5",
            "caught bar 6",
            "steering onto the perch at [-80.50, 2.00",
            "-> perch via xx_h_beam_landing_soft_tr_pilotis_wait_a",
            "free-step jump aimed at [-78.10, 2.00",
            "running jump aimed at [-75.70, 2.00",
            "onto perch 23",
        ],
        never: &["EMBED"],
    },
    Scenario {
        name: "gauntlet G7: up a wall's holds, along to its edge, the leap across the gap onto the next wall, up to its top",
        tags: &["combo", "climb", "hang"],
        env: &[("AC1_START", "-92.6,93,-90"), ("AC1_CLIMB", "up=2.5,right=3,leap-right=2,right=1,up=8")],
        secs: 18.0,
        want: &["via xx_l_climb_1ru_r_2ru", "out of reach: jump to the next hold", "climb1m_tr_climb1m_right_2_c at [-92.59, 2.97, 96", "-> top via"],
        never: &["EMBED"],
    },
    Scenario {
        name: "gauntlet G8: the ledge stop at a roof's edge, the pull down, down its holds, on to jump onto a 1.2 m wall",
        tags: &["combo", "hang", "climb", "ground"],
        env: &[
            ("AC1_START", "-95.6,104,-90,5.5"),
            ("AC1_WALK", "5.2"),
            ("AC1_STOP", "2.0-12"),
            ("AC1_HIGH", "0-16"),
            ("AC1_JUMP", "2.6"),
            ("AC1_CLIMB", "nograb,wait=4,down=8"),
            ("AC1_LEGS", "12-16"),
        ],
        secs: 16.0,
        want: &[
            "ledge stop",
            "-> drop via xx_l_ledge_stop_start_footl_pulldown_front_orientation",
            "hangwall_open -> 1m via xx_h_hangwall_d_climb_1m",
            "stepdown -> ground",
            "turn broken off into the run",
            "jump onto 1.20 m",
        ],
        never: &["EMBED"],
    },
    Scenario {
        name: "gauntlet G9: over the passover wall, up the wall with holds and over, across the roof, down onto a post",
        tags: &["combo", "wallrun", "climb", "beam"],
        env: &[("AC1_START", "-98,114,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-14"), ("AC1_LEGS", "0-14"), ("AC1_CLIMB", "nograb,wait=3,leap-up=8")],
        secs: 12.0,
        want: &[
            "wall run over the 1.40 m wall",
            // (Its first hold row, 2.39 m, is under AC1's catch height for a wall run, 1 m over the feet at the end of
            // its step: the 2.99 m row, its 251 and 430 cm catches blended.)
            "tr_hangwall_251cm_000cm_a",
            "-> top via xx_h_hangknee_footl_tr_h_jog_footr_b",
            "running jump aimed at [-79.00, 2.00, 114.00]",
            "freestep_down_tr_freestep_entry",
        ],
        never: &["EMBED"],
    },
    Scenario {
        name: "gauntlet G10: the swing bar flung at a wall's holds, up them, across the roof, down onto the lower roof",
        tags: &["combo", "bars", "climb", "jump"],
        env: &[("AC1_START", "-100,124,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-16"), ("AC1_LEGS", "0-16"), ("AC1_CLIMB", "nograb,wait=4,leap-up=8")],
        secs: 14.0,
        want: &[
            "caught bar 7",
            "swing -> leap via xx_h_swing_cycle_front_300cm_to_air",
            "-> 2m via xx_fall_tr_climb_min_b",
            "-> top via xx_h_hangknee_footl_tr_h_jog_footr_b",
            "running jump aimed at [-78.0",
            "-> land via xx_h_landing_damage_foot",
        ],
        never: &["EMBED"],
    },
    Scenario {
        name: "level with a lower roof flush with the wall, on sideways: AC1's turned step-off onto it (groundentry _90)",
        tags: &["climb"],
        env: &[("AC1_START", "-92.6,132.6,-90"), ("AC1_CLIMB", "up=1.6,none=0.8,right=4")],
        secs: 7.0,
        want: &["stepped off the wall sideways (right, turned)", "-> ground via xx_l_climb_1m_to_groundentry_right_90_tr_h_wait_footl"],
        never: &["EMBED"],
    },
    Scenario {
        name: "pulled down off a slab into a free hang, down: AC1's drop onto the wall's holds 2.4 m below (hangfree_tr_climb2m_down)",
        tags: &["hang", "climb"],
        env: &[("AC1_START", "-90.5,144,90,6"), ("AC1_WALK", "1.5"), ("AC1_STOP", "2.5-30"), ("AC1_JUMP", "3.0"), ("AC1_CLIMB", "nograb,wait=5,down=1.5")],
        secs: 8.0,
        want: &["-> free_1m via xx_l_ledge_pulldown_soft_front_to_hangfree_b", "via xx_h_climbing_hangfree_tr_climb2m_down_", "leap -> 2m via"],
        never: &["EMBED"],
    },
    // The ejects measured in the running game at the Damascus bureau's climbable wall: from the wall hang, high profile
    // and Space, the stick back onto the beam 3 m behind (AC1 landed at x 46.64), to the left onto the beam 3.3 m along
    // the wall and 1.1 m up (AC1 y 23.31).
    Scenario {
        name: "Damascus: the back eject from the bureau wall's hang onto the beam behind (AC1's rebound)",
        tags: &["eject", "hang"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "47.6,-20,-90,5.55"),
            ("AC1_WALK", "6.2"),
            ("AC1_STOP", "1.0"),
            ("AC1_HIGH", "0-5"),
            ("AC1_LEGS", "0-1.0"),
            ("AC1_JUMP", "2.6"),
            ("AC1_CLIMB", "nograb,wait=2.4,down=1.2"),
        ],
        secs: 5.0,
        want: &["rebound off the wall at [46.", "via xx_h_rebound_frontleft_", "_tr_freestep_entry_footr at [46."],
        never: &["EMBED"],
    },
    Scenario {
        name: "Damascus: the side eject to the left from the bureau wall's hang onto the beam along the wall (AC1's rebound)",
        tags: &["eject", "hang"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "47.6,-20,-90,5.55"),
            ("AC1_WALK", "6.2"),
            ("AC1_STOP", "1.0"),
            ("AC1_HIGH", "0-5"),
            ("AC1_LEGS", "0-1.0"),
            ("AC1_JUMP", "2.6"),
            ("AC1_CLIMB", "nograb,wait=2.4,left=1.2"),
        ],
        secs: 5.0,
        want: &["xx_h_hangwall_strafe_left_050cm", "rebound off the wall at [49.", "via xx_h_rebound_right_", "_tr_freestep_entry_footl at [49.10, 8.0"],
        never: &["EMBED"],
    },
];

fn game_dir() -> Option<PathBuf> {
    let d = PathBuf::from(std::env::var("AC1_GAME_DIR").unwrap_or_else(|_| DEFAULT_GAME_DIR.into()));
    d.join("DataPC.forge").exists().then_some(d)
}

/// A pose pop this big (m: a bone jumping relative to the root in one frame) fails a scenario; smaller ones are reported.
const POP_FAIL: f32 = 0.4;

fn run(exe: &str, s: &Scenario, k: usize) -> Result<(), String> {
    // (No screenshot, `-`: the run ends at `secs`; only the log is checked. `AC1_SCENARIO_SHOTS=1` saves them.)
    let shot = if std::env::var("AC1_SCENARIO_SHOTS").is_ok() {
        std::env::temp_dir().join(format!("ac1-scenario-{}-{k}.png", std::process::id()))
    } else {
        std::path::PathBuf::from("-")
    };
    let mut c = Command::new(exe);
    c.env("RUST_LOG", "ac1=debug,wgpu=error")
        .env("NO_COLOR", "1")
        .env("AC1_SHOT", &shot)
        .env("AC1_SHOT_SECS", s.secs.to_string())
        .env("AC1_EMBED_CHECK", "1")
        .env("AC1_POP_CHECK", "1");
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
    // Pose pops (the recorder's live check, `AC1_POP_CHECK`): reported; one bigger than `POP_FAIL` fails the scenario.
    let pops: Vec<(f32, &str)> = log
        .lines()
        .filter_map(|l| l.split_once("POP ").map(|x| x.1))
        .filter_map(|l| l.split_whitespace().nth(1).and_then(|m| m.parse::<f32>().ok()).map(|m| (m, l)))
        .collect();
    for (_, l) in &pops {
        eprintln!("  pop: {l}");
    }
    let big: Vec<&str> = pops.iter().filter(|(m, _)| *m > POP_FAIL).map(|(_, l)| *l).collect();
    if !big.is_empty() {
        return Err(format!("pose pops over {POP_FAIL} m: {big:?}"));
    }
    let missing: Vec<&str> = s.want.iter().copied().filter(|w| !log.contains(w)).collect();
    let present: Vec<&str> = s.never.iter().copied().filter(|w| log.contains(w)).collect();
    if missing.is_empty() && present.is_empty() { Ok(()) } else { Err(format!("missing {missing:?}, unwanted {present:?}")) }
}

/// The systems scenarios are tagged with.
const TAGS: &[&str] = &["ground", "jump", "climb", "hang", "beam", "ladder", "bars", "hay", "vault", "wallrun", "ragdoll", "misc", "combo", "eject"];

/// Every scenario is tagged with known systems (no game needed). `AC1_SCENARIO_LIST=1` prints them: count, then
/// `tags<TAB>name` lines (for the pipeline page).
#[test]
fn scenario_tags() {
    for s in SCENARIOS {
        assert!(!s.tags.is_empty() && s.tags.iter().all(|t| TAGS.contains(t)), "{}: tags {:?}", s.name, s.tags);
    }
    if std::env::var("AC1_SCENARIO_LIST").is_ok() {
        println!("SCENARIOS {}", SCENARIOS.len());
        for s in SCENARIOS {
            println!("SCENARIO	{}	{}", s.tags.join(","), s.name);
        }
    }
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
    // `AC1_SCENARIO_TAGS=climb,hang`: only the scenarios through any of those systems (run what a change can affect).
    let tags: Vec<String> =
        std::env::var("AC1_SCENARIO_TAGS").map(|t| t.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default();
    let picked: Vec<&Scenario> = SCENARIOS
        .iter()
        .filter(|s| s.name.contains(filter.as_str()))
        .filter(|s| tags.is_empty() || s.tags.iter().any(|t| tags.iter().any(|w| w == t)))
        .collect();
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
