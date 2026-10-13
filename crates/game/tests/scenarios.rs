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
    /// hay, vault, wallrun, ragdoll, misc, fight.
    tags: &'static [&'static str],
    env: &'static [(&'static str, &'static str)],
    secs: f32,
    /// Every one of these must appear in the log.
    want: &'static [&'static str],
    /// None of these may.
    never: &'static [&'static str],
}

/// Games run at once by default (each its own process): the scenarios are independent. 4 since 2026-10-10: with 6
/// (and the real AC1 open beside them) frames grew long enough for a swing's foot to move 0.45 m in one and fail the pop
/// check, and a ragdoll not to settle in time (84 scenarios: 4 at a time 262 s, all passing; 6 about 195 s, 1-2 flakes).
const JOBS: usize = 4;
/// Games run at once headless (the default since 2026-10-11, `AC1_HEADLESS`): no window or GPU, every frame 1/60 s of
/// game time however long it takes, so a loaded machine only slows the runs down and does not change them.
const HEADLESS_JOBS: usize = 6;

/// Windowed runs (`AC1_SCENARIO_WINDOWED=1`, or `AC1_SCENARIO_SHOTS=1` for the screenshots): to watch them.
fn windowed() -> bool {
    std::env::var("AC1_SCENARIO_WINDOWED").is_ok() || std::env::var("AC1_SCENARIO_SHOTS").is_ok()
}

const QUIET: &[(&str, &str)] = &[("AC1_NO_CROWD", "1")];

const SCENARIOS: &[Scenario] = &[
    Scenario {
        name: "standing turn round in high profile: the turn, then its own exit into the jog (AC1's, audit GROUND-5)",
        tags: &["ground"],
        env: &[("AC1_START", "-5,-5,0"), ("AC1_WALK", "4.5"), ("AC1_STEER", "180"), ("AC1_HIGH", "0-5")],
        secs: 3.0,
        want: &["waitturn_right_180", "_tr_h_jog_hipm_"],
        // (The turn turns the body itself and is locked: played out, not broken off; only its exit may be, audit ANIM-8.)
        never: &["off into the run (out of xx_h_wait_hipm_"],
    },
    Scenario {
        name: "sprint stop on the course ramp stays on it (the stop follows the slope)",
        tags: &["ground"],
        env: &[("AC1_START", "24.5,-0.95,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-3"), ("AC1_STOP", "0.55"), ("AC1_EMBED_CHECK", "1")],
        secs: 2.5,
        want: &["xx_h_sprintstop_", "-> ground via xx_h_runstop_"],
        never: &["EMBED"],
    },
    Scenario {
        name: "running up the stairs in high profile: steps, not a low obstacle to collide with",
        tags: &["ground", "vault"],
        env: &[("AC1_START", "0,0.5,0"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-5")],
        secs: 3.0,
        want: &["step off the edge"],
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
        // (Audit WALL-21: AC1's wall run takes a wall leaning up to 45 degrees, its contact out along the lean.)
        name: "free running at the wall leaning back 20 degrees: up it (AC1's wall run takes walls to 45 degrees)",
        tags: &["wallrun"],
        env: &[("AC1_START", "-18,-53,0"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-3"), ("AC1_LEGS", "0-3")],
        secs: 3.0,
        // (The entry ends 0.17 m higher than on an upright wall, 0.47 m out along the slope from where the probe met it.)
        want: &["wallrun via xx_h_wallingfront_entry_footl_b at [-18.00, 1.17, -58.00]"],
        never: &["EMBED", "no wall run"],
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
        name: "side grab: walking off the ledge with the empty hand and the stick to the wall grabs it",
        tags: &["jump", "hang"],
        // (He walks off 0.28 m past the ledge's end, AC1's rim rule: the stick turns as he does.)
        env: &[("AC1_START", "-5,-50.5,-90,3"), ("AC1_WALK", "1.9"), ("AC1_GRASP", "3.55-6"), ("AC1_VEER", "3.45,90")],
        secs: 6.0,
        // (Caught by the empty hand, as AC1's manual catch; into the wall hang, or the climb only with both feet on holds.)
        want: &["via xx_fall_tr_"],
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
        name: "standing on a post, the legs tapped with no stick: AC1's impulsion, straight up and back onto it",
        tags: &["beam", "jump"],
        env: &[("AC1_START", "38.6,0,-90,3"), ("AC1_HIGH", "0-3"), ("AC1_LEGS", "1.0-1.6")],
        secs: 3.5,
        want: &["crouched to jump straight up", "straight up off the perch", "-> perch via xx_h_beam_landing_soft_tr_pilotis_wait_a"],
        never: &["EMBED", "landed from 2", "-> ground"],
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
        // (Through the turn's own exit into the walk, AC1's, audit GROUND-5.)
        want: &["_tr_l_walk_hipm_"],
        never: &[],
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
        name: "a tap jump at block L's ladder from 4 m: flown at it and caught (AC1's ladder jump target)",
        tags: &["ladder", "jump"],
        env: &[("AC1_START", "23.5,-1.4,0"), ("AC1_WALK", "3.5"), ("AC1_STOP", "0.5"), ("AC1_HIGH", "0-3"), ("AC1_JUMP", "0.4")],
        secs: 3.0,
        want: &["caught ladder"],
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
        env: &[("AC1_START", "23.5,-5.4,0"), ("AC1_CLIMB", "up=2.5,down=3"), ("AC1_HIGH", "4.0-4.6"), ("AC1_LEGS", "4.2-4.3")],
        secs: 7.0,
        want: &["rebound off the ladder", "_tr_fall", "land -> ground"],
        never: &["EMBED", "landing_damage_footl_roll"],
    },
    Scenario {
        name: "free running through both swing bars, the legs only held",
        tags: &["bars", "jump"],
        // (Each bar swung a whole cycle: AC1's swing jump comes at the end of the back swing's way down.)
        env: &[("AC1_START", "50.3,0,-90"), ("AC1_WALK", "6"), ("AC1_HIGH", "0-10"), ("AC1_LEGS", "0-10")],
        secs: 9.0,
        want: &[
            "caught bar 0",
            "caught bar 1",
            "swing -> leap via xx_h_swing_cycle_",
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
        name: "climbing wall A with the stick held tops out through AC1's free-step stand into the walk",
        tags: &["climb", "ground"],
        env: &[("AC1_START", "0,4.0,180"), ("AC1_CLIMB", "up=14"), ("AC1_WALK", "1.9")],
        secs: 14.0,
        want: &["-> top via xx_h_hangknee_footl_tr_freestep_entry_footl"],
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
        // (A row up first: at the bottom AC1's ChooseMove steps off sideways onto the ground before any corner, audit
        // CLIMB-E10.)
        env: &[("AC1_START", "-1.4,4.0,180"), ("AC1_CLIMB", "up=1.2,right=6")],
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
        // (The user's "turning corners in a crouch hang is weird": the root turned 90 degrees in place in 0.25 s, then slid
        // 0.85 m. AC1's interpolator turns it over both strafe clips; ours about the hands, which stay on the holds.)
        name: "wall hang on the beam block's south face, right round its west corner and back: the turn about the hands",
        tags: &["hang", "climb"],
        env: &[
            ("AC1_START", "64.35,30.6,0,6"),
            ("AC1_WALK", "1.5"),
            ("AC1_STOP", "1.2-30"),
            ("AC1_HAND", "2.0"),
            ("AC1_CLIMB", "nograb,wait=4.5,right=1.0,wait=2.5,left=1.0"),
        ],
        secs: 10.0,
        want: &[
            "-> hangwall_open via xx_l_ledge_pulldown_soft_to_hangwall_straight_b",
            "round the outside corner from the wall hang onto [63.9",
            "round the outside corner from the wall hang onto [64.2",
            "via xx_h_hangwall_strafe_left_050cm_close",
        ],
        never: &["EMBED", "hangfree"],
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
        name: "free running at the 1.4 m thin wall: AC1's wall run onto its top (no pass-over: audit WALL-19), and off",
        tags: &["wallrun", "jump"],
        env: &[("AC1_START", "-18,-34,0"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-4"), ("AC1_LEGS", "0-4")],
        secs: 3.5,
        want: &["tr_hangknee_131cm_footl", "-> top via xx_h_hangknee_footl_tr_freestep_entry_footl", "-> land via"],
        never: &["EMBED", "landed from -", "passover"],
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
        // (Audit JUMP-6: AC1's running jump keeps the takeoff facing as its clips' frame and turns the facing toward the
        // target linearly over the takeoff, at most 0.4 s.)
        name: "rooftops: B1 to B2 and down to B3 run 12 degrees off the roofs' line: the jump down turns toward its target over the takeoff",
        tags: &["jump"],
        env: &[("AC1_START", "41,33,-90,8"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-4.6"), ("AC1_LEGS", "0-4.6"), ("AC1_STEER", "12")],
        secs: 3.2,
        want: &["4.49, 30.53] (takeoff and flight from AC1's jump tables", "the running jump turns 11 degrees toward its target over 0.20 s"],
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
        env: &[("AC1_START", "66.5,35,0,6"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-14"), ("AC1_LEGS", "0-14")],
        secs: 10.0,
        want: &["aimed at [66.50, 5.69", "caught bar 2", "caught bar 3", "-> land via xx_h_landing_"],
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
        // (AC1's eject off a wall run is a hang's: its rebound takeoff at the hold, as the running game's off the bureau.)
        want: &["wall run eject (step1rebound)", "xx_h_rebound_front", "-> free_1m via xx_fall_tr_hangfree"],
        never: &["EMBED"],
    },
    Scenario {
        name: "free running past hiding spot A in high profile runs on, not into the hay",
        tags: &["hay", "ground"],
        env: &[("AC1_START", "13.5,-12.5,180,1"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-2"), ("AC1_LEGS", "0-2")],
        secs: 2.5,
        want: &[],
        never: &["into the haystack"],
    },
    // Audit CLIMB-E16: AC1's empty hand releases into the air, near the ground too (the step down is the stick's).
    Scenario {
        name: "on wall A just off the ground, let go: the release into a fall, not the step down",
        tags: &["climb"],
        env: &[("AC1_START", "0,4.0,180"), ("AC1_CLIMB", "wait=1.5,drop=2")],
        secs: 6.0,
        want: &["via xx_h_hangwall_tr_fall_a", "land -> ground"],
        never: &["xx_l_climb_1m_tr_h_wait_hipm_footl", "EMBED"],
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
        env: &[("AC1_START", "-14.5,-11.4,0"), ("AC1_HIGH", "0-4"), ("AC1_JUMP", "0.5")],
        secs: 4.0,
        want: &["hangwall"],
        never: &[],
    },
    Scenario {
        name: "standing in high profile, the legs held 0.2 s with nothing overhead: AC1's static jump's crouch, then straight up",
        tags: &["jump", "ground"],
        env: &[("AC1_START", "-100,84,-90"), ("AC1_HIGH", "0-3.5"), ("AC1_LEGS", "1.2-1.4")],
        secs: 3.5,
        want: &["crouched for the static jump", "via xx_h_jumpstraight_clear_foot"],
        never: &["EMBED"],
    },
    Scenario {
        name: "jump onto 1.75 m with a blend",
        tags: &["jump", "vault"],
        env: &[("AC1_START", "-2,-11.4,0"), ("AC1_HIGH", "0-4"), ("AC1_JUMP", "0.5")],
        secs: 4.0,
        want: &["mixing xx_h_jumpstraight_footl_to_hangknee_footl_150cm", "-> top via"],
        never: &[],
    },
    Scenario {
        name: "leap of faith into hay",
        tags: &["hay", "jump"],
        env: &[("AC1_START", "13.5,-5.5,0"), ("AC1_WALK", "1.6"), ("AC1_HIGH", "0-5"), ("AC1_JUMP", "0.8")],
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
        // (Free running off the 0.6 m box's far edge, a 0.6 m drop: AC1's edge jump (over 0.5 m, within 0.25 m), the free
        // jump with no target, over the thin wall; then on along the lane's tops. Ours dropped off it and vaulted the wall.)
        want: &["jump onto 0.60 m", "running jump aimed at [8.", "running jump aimed at [16."],
        never: &["EMBED", "passover"],
    },
    Scenario {
        // (High profile hops onto a top 0.45-1.3 m up within 1.3 m, the legs held or not: AC1's free-run target jump, as
        // the running game hopped onto a 1.2 m box in the Damascus street.)
        name: "running into the 0.6 m block in high profile hops onto it",
        tags: &["vault"],
        env: &[("AC1_START", "-20,-24.5,-90"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-8")],
        secs: 5.0,
        want: &["jump onto 0.60 m"],
        never: &["EMBED"],
    },
    Scenario {
        name: "running at the 0.6 m block at an angle in high profile hops onto it too",
        tags: &["vault"],
        env: &[("AC1_START", "-17.5,-23.6,-90"), ("AC1_STEER", "40"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-5")],
        secs: 3.0,
        // (Slowed by the turn, it may be the standing free step onto it, AC1's below the walk band's speed.)
        want: &["0.60 m", "_tr_freestep_entry"],
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
        name: "walking out over block L's ladder at its top: onto it and down, no button (AC1's top entry, low profile)",
        tags: &["ladder"],
        env: &[("AC1_START", "23.5,-6.48,180,5"), ("AC1_WALK", "1.5")],
        secs: 4.0,
        want: &["onto ladder 1", "via xx_l_wait_hipm_foot", "_tr_ladder_pulldown", "xx_ladder_pulldown_tr_l_ladder_wait_r_a"],
        never: &["EMBED", "landed from"],
    },
    Scenario {
        name: "ladder to the top",
        tags: &["ladder"],
        env: &[("AC1_START", "23.5,-4.8,0"), ("AC1_WALK", "1.0"), ("AC1_STOP", "1.0-14"), ("AC1_CLIMB", "nograb,up=14")],
        secs: 14.0,
        want: &["ladder", "-> top via"],
        never: &[],
    },
    // The standing jump goes for the highest hold in reach (AC1's chooser 0xE97720, the higher of two reports; the running
    // game's live log on the Damascus bureau's wall, 2026-10-12): at pillar P its third hold, 2.99 m up, in AC1's 2.5-3 m
    // band with the pillar under it (0xB21DA0: the wall-free hang, audit LEDGE-15; ours' wall hang jump reached only the
    // second, 2.39 m up, and climbed from there), then up: onto the pillar in place, the climbing stance, up onto its top.
    Scenario {
        name: "pole P: the standing jump to the higher cap in reach, as AC1's, then up the caps",
        tags: &["hang", "jump"],
        env: &[("AC1_START", "-1.2,10.9,180"), ("AC1_HIGH", "0-1"), ("AC1_JUMP", "0.5"), ("AC1_CLIMB", "nograb,up=8")],
        secs: 4.0,
        want: &["-> leap via xx_h_jumpstraight_footl_to_hangwallfree_300cm", "-> hangwall_open via xx_h_hangfree_tr_hangwall at", "hangwall -> 1m"],
        never: &["EMBED", "xx_h_jumpstraight_footl_to_hangwall_200cm"],
    },
    Scenario {
        name: "pole P to its perch",
        tags: &["beam", "hang"],
        env: &[("AC1_START", "-1.2,10.9,180"), ("AC1_HIGH", "0-1"), ("AC1_JUMP", "0.5"), ("AC1_CLIMB", "nograb,up=8")],
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
        name: "stop from a run in high profile: AC1's run stop, into the stand",
        tags: &["ground"],
        env: &[("AC1_START", "-25,-8,0"), ("AC1_WALK", "4.3"), ("AC1_HIGH", "0-3"), ("AC1_STOP", "1.5")],
        secs: 3.0,
        want: &["xx_h_runstop_foot", "_tr_h_wait_hipm_"],
        never: &[],
    },
    Scenario {
        name: "free running up a wall leaps hold to hold and climbs over the top",
        tags: &["wallrun", "climb"],
        env: &[("AC1_START", "0,0.5,180"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-14"), ("AC1_LEGS", "0-14"), ("AC1_CLIMB", "nograb,wait=3,leap-up=11")],
        secs: 9.0,
        // (The grid's moves before the reaches, the legs held or not: AC1's ChooseMove, audit CLIMB-E3.)
        want: &["1m -> hangknee via xx_l_climb_1m_tr_hangknee_footl_a", "-> top via"],
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
        env: &[("AC1_START", "1.2,11.1,180"), ("AC1_HIGH", "0-1"), ("AC1_JUMP", "0.5"), ("AC1_CLIMB", "nograb,wait=3.5,up=4")],
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
        name: "free running at block L runs up it, not through it",
        tags: &["ground", "vault"],
        env: &[("AC1_START", "14,-7.5,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-8"), ("AC1_LEGS", "0-8")],
        secs: 6.0,
        want: &["wallrun"],
        never: &["onto kiosk", "EMBED"],
    },
    Scenario {
        // (Audit NARROW-11: AC1's climb from a beam, event 16, after the pull-down finds nothing.)
        name: "standing on G17's beam at its wall, the empty hand climbs onto the wall",
        tags: &["beam", "climb"],
        env: &[("AC1_START", "-92.7,214,-90,2.5"), ("AC1_HAND", "1.5")],
        secs: 3.5,
        want: &["onto perch", "from the beam onto the wall ahead", "via xx_h_wait_hipm_footl_tr_climbing_1m"],
        never: &["EMBED"],
    },
    Scenario {
        // (Audit NARROW-16: AC1's kiosk, a market stall's counter, jumped onto across its front: the entry, one run along
        // the counter and off its end into the air.)
        name: "the tap jump at kiosk K's counter across its front: onto it, along it and off its end",
        tags: &["jump", "misc"],
        env: &[("AC1_START", "29.85,-14,180"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "0.3-0.4")],
        secs: 4.0,
        want: &["jump onto kiosk at", "along the kiosk's counter", "off the kiosk's counter", "-> ground"],
        never: &["EMBED", "landing_death"],
    },
    Scenario {
        // (Audit NARROW-16: along the stall's axis, onto a side: the side free step, then a free-step jump on out.)
        name: "the tap jump at kiosk K along its axis: onto its side and a free step on out",
        tags: &["jump", "misc"],
        env: &[("AC1_START", "24.5,-9.6,-90"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "0.2-0.3")],
        secs: 4.0,
        want: &["side Lateral", "off the kiosk's side"],
        never: &["EMBED", "landing_death"],
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
        // (Audit EXTRA-16: the running AC1's ledge stop broke into its standing turn toward the edge, 0.084 s in.)
        name: "standing at block A's edge facing away, turning round toward it in high profile: the ledge stop breaks the turn",
        tags: &["ground"],
        env: &[("AC1_START", "0,5.1,180"), ("AC1_WALK", "4.5"), ("AC1_STEER", "180"), ("AC1_HIGH", "0-5")],
        secs: 3.0,
        want: &["_to_h_waitturn_", "ledge stop"],
        never: &["_tr_h_jog_hipm_", "landed from 6"],
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
            "jump aimed at [-84.20, 2.00, 35.00]",
            "free-step jump at the swing bar at [-82.20, 4.40",
            "caught bar 4",
            "steering onto the perch at [-79.20, 1.50",
            "-> perch via xx_h_beam_landing_soft_tr_pilotis_wait_a at [-79.",
            // (On to the platform 2.35 m off by the free-step jump at it: the perch's jumps reach as far as AC1's zone 7
            // allows, audit JUMP-7; ours' 0.8-4.5 m cut had it jump blind.)
            "free-step jump aimed at [-76.8",
            "_tr_freestep_entry_footr at [-77.3",
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
            "-> top via xx_h_hangknee_footl_tr_freestep_entry_footl",
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
        // (Off the boxes AC1's step off, and off the 2 m platform its free jump, 8 m on and 3 m down (audits AIR-6, JUMP-1):
        // over the next box, down to the ground.)
        // (Off each box's edge free running, AC1's edge jump; then the jumps along the platforms.)
        want: &["jump onto 0.60 m", "running jump aimed at [-75.", "running jump aimed at [-68."],
        never: &["EMBED"],
    },
    Scenario {
        name: "gauntlet G5: zigzag post to post, onto the beam, along it onto a box, the dive into the hay beside it",
        tags: &["combo", "beam", "hay"],
        env: &[
            ("AC1_START", "-100,74,-90"),
            ("AC1_WALK", "6.2"),
            ("AC1_HIGH", "0-9"),
            ("AC1_LEGS", "0-6"),
            ("AC1_PATH", "-89.5,74;-86.6,74;-84.4,75;-82.2,74;-80,75;-77.8,74;-72.5,74;-70.1,74;stop"),
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
            // (On to the beam along the way: aimed at its far end as AC1's are, out of reach, the jump steers onto it.)
            "onto perch 23 at [-74.",
            "onto perch 23",
        ],
        never: &["EMBED"],
    },
    Scenario {
        name: "gauntlet G11: free running a beam, the stick to the right: the corner hop onto the beam beside, on along it",
        tags: &["combo", "beam"],
        env: &[("AC1_START", "-100,154,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-10"), ("AC1_LEGS", "0-10"), ("AC1_VEER", "2.6,-90")],
        secs: 6.0,
        want: &["corner hop onto beam", "along [0.00, 0.00, 1.00]"],
        never: &["EMBED"],
    },
    Scenario {
        name: "gauntlet G12: up a wall's holds to the top row, up again: AC1's jump up from the climb to the slab above, out from the wall",
        tags: &["climb", "hang"],
        env: &[("AC1_START", "-92.6,164,-90,1"), ("AC1_CLIMB", "up=8")],
        secs: 9.0,
        want: &["jump up from the climb", "via xx_h_climbing_1m_impultion"],
        never: &["EMBED"],
    },
    // Audit LEDGE-13 (the blend): from the free hang off G12's slab, down onto the holds 1 m under it, AC1's drop mixed from
    // its four clips by the exe's weights (TryFreeHangDropToClimb 0xDDF390).
    Scenario {
        name: "gauntlet G12's slab, then down: AC1's free-hang drop onto the holds below, its four clips mixed",
        tags: &["climb", "hang"],
        // (Up only into the free hang under the slab: held on, it pulls up onto the slab, a cornice 0.8 m deep in front of the
        // wall going on up, which AC1's room boxes allow.)
        env: &[("AC1_START", "-92.6,164,-90,1"), ("AC1_CLIMB", "up=3.5,none=2.5,down=4")],
        secs: 12.0,
        want: &["drop onto the wall below mixed", "-> 2m via xx_h_climbing_hangfree_tr_climb2m_down_"],
        never: &["EMBED"],
    },
    // Audit CLIMB-E6: AC1's room test over the edge is three empty boxes (0xB2E240), the first 1 m out and 1.8 m up.
    Scenario {
        name: "gauntlet G13: up a wall's holds to its top under an awning 1.5 m over it: no climbing out (AC1's room boxes)",
        tags: &["climb"],
        env: &[("AC1_START", "-92.6,174,-90,1"), ("AC1_CLIMB", "up=8")],
        secs: 9.0,
        want: &["no room over the edge"],
        never: &["hangknee", "EMBED"],
    },
    // Audit CLIMB-E13: AC1's body boxes at the pose a climbing move ends in (0xB2DF90).
    Scenario {
        name: "gauntlet G14: up a wall's holds to a block sticking out of it: the climbing move up into it refused (AC1's body boxes)",
        tags: &["climb"],
        env: &[("AC1_START", "-92.6,184,-90,1"), ("AC1_CLIMB", "up=6"), ("AC1_CLIMB_WHY", "1")],
        secs: 8.0,
        want: &["no room for the body there (AC1's body boxes)"],
        never: &["EMBED"],
    },
    // Audit LADDER-7: AC1's ladders are climbed from either side; the revolve round one (event 5) where there is room.
    Scenario {
        name: "gauntlet G16: up a free-standing ladder, the stick to the right: round it onto its other side (AC1's revolve), down",
        tags: &["ladder"],
        env: &[("AC1_START", "-90.44,204,-90"), ("AC1_CLIMB", "up=1.5,wait=0.5,right=0.5,wait=1,down=5")],
        secs: 9.0,
        want: &["round ladder", "via xx_h_ladder_turn_r_a", "via xx_h_ladder_turn_r_b_tr_h_wait_r", "-> ground via"],
        never: &["EMBED"],
    },
    Scenario {
        name: "gauntlet G16: at a free-standing ladder's back: onto it from there (AC1's ladders are two-sided), up, down",
        tags: &["ladder"],
        env: &[("AC1_START", "-89.56,204,90"), ("AC1_CLIMB", "up=1.5,down=4")],
        secs: 7.0,
        want: &["onto ladder", "from its back", "via xx_l_ladder_climb_up_", "-> ground via"],
        never: &["EMBED"],
    },
    Scenario {
        name: "block L's ladder against its wall, the stick to the side: no room to go round it (the wall)",
        tags: &["ladder"],
        env: &[("AC1_START", "23.5,-5.4,0"), ("AC1_CLIMB", "up=1.5,wait=0.5,right=0.5")],
        secs: 5.0,
        want: &["via xx_l_ladder_climb_up_"],
        never: &["round ladder", "EMBED"],
    },
    // The user's report (2026-10-11): onto E's ladder from its top, a step up, the stick to the right: E's wall stands
    // behind it, no revolve (it took him through the wall), but off it onto E's holds on its right.
    Scenario {
        name: "E's ladder from its top, a step up, the stick to the right: no room to go round it (the wall), onto the holds there",
        tags: &["ladder"],
        env: &[("AC1_START", "10,15.5,180,6"), ("AC1_WALK", "1.5"), ("AC1_CLIMB", "nograb,wait=3,up=0.4,wait=1,right=0.6")],
        secs: 7.0,
        want: &["_tr_ladder_pulldown", "_ladder_climb_up_", "off ladder 0 onto the wall, right"],
        never: &["round ladder", "EMBED"],
    },
    // (The same entry's frame: facing the rungs on the ladder's front, its top exit is there, not taken for its back.)
    Scenario {
        name: "E's ladder from its top, down a step, back up: off its top again (the entry's frame is the ladder's front)",
        tags: &["ladder"],
        env: &[("AC1_START", "10,15.5,180,6"), ("AC1_WALK", "1.5"), ("AC1_CLIMB", "nograb,wait=3,down=0.4,wait=0.6,up=3")],
        secs: 8.0,
        want: &["_tr_ladder_pulldown", "_ladder_climb_down_", "-> top via"],
        never: &["round ladder", "EMBED"],
    },
    Scenario {
        name: "gauntlet G7: up a wall's holds, along to its edge, the leap across the gap onto the next wall, up to its top",
        tags: &["combo", "climb", "hang"],
        env: &[("AC1_START", "-92.6,93,-90"), ("AC1_CLIMB", "up=2.5,right=3,leap-right=2,right=1,up=8")],
        secs: 18.0,
        want: &["via xx_l_climb_1ru_r_2ru", "out of reach: jump to the next hold", "climb1m_tr_climb1m_right_2_c at [-92.6", "-> top via"],
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
            // (No legs press at the edge: the pull-down is pushing through the ledge stop; a press in the hang is an eject.)
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
            // (Onto the thin wall's top, then AC1's free jump off it catches the wall of holds in the air.)
            "tr_hangknee_131cm_footl",
            "-> 2m via xx_fall_tr_climb_min_b",
            "-> top via xx_h_hangknee_footl_tr_freestep_entry_footl",
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
            "swing -> leap via xx_h_swing_cycle_",
            "-> 2m via xx_fall_tr_climb_min_b",
            "-> top via xx_h_hangknee_footl_tr_freestep_entry_footl",
            "running jump aimed at [-7",
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
        env: &[("AC1_START", "-90.5,144,90,6"), ("AC1_WALK", "1.5"), ("AC1_STOP", "2.5-30"), ("AC1_HAND", "3.0"), ("AC1_CLIMB", "nograb,wait=5,down=1.5")],
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
        // (Down on the beam as AC1's NarrowObject, not run on along it as ground: live log route wall1_hang_down.)
        want: &["rebound off the wall at [46.", "via xx_h_rebound_frontleft_", "onto perch", "_tr_freestep_entry_footr at [46."],
        never: &["EMBED"],
    },
    // The live log's haystack route (2026-10-12): free running north-east onto the hay box's rim, into the hay, out over
    // its far rim and down into the hang on the roof's edge; W and the legs still held, the running game pulled up onto
    // the edge (AC1's pull-up first from a ledge hang, CanPullup 0xDE2270), where ours leapt to a hold 2.4 m higher.
    Scenario {
        name: "Damascus: up from the hang on a roof's edge with the legs held pulls up, as AC1's ledge does, not a leap",
        tags: &["hang", "hay"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "52.0,-33.4,-31.0,13.45"),
            ("AC1_WALK", "6.2"),
            ("AC1_STICK", "1"),
            ("AC1_STOP", "4.44"),
            ("AC1_HEADINGS", "0.14,58.9"),
            ("AC1_HIGH", "0-4.44"),
            ("AC1_LEGS", "0-4.44"),
            ("AC1_CLIMB", "nograb,leap-up=4.43"),
        ],
        secs: 4.5,
        want: &["-> hangwall_open via xx_l_ledge_pulldown_soft_to_hangwall_straight_b", "hangwall_open -> hangknee via xx_h_hangwall_tr_hangknee_footl_a"],
        never: &["xx_h_climbing_hangwall_tr_climb1m_up"],
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
        want: &["xx_h_hangwall_strafe_left_050cm", "rebound off the wall at [49.", "via xx_h_rebound_right_", "_tr_freestep_entry_footl at [49.1"],
        never: &["EMBED"],
    },
    // Audit JUMP-10: the tap jump at the bureau's wall from 1.5 m off. AC1's scorer picks the hold 2.4 m up as a wall-hang
    // target (0x40): the running game flew `_to_surface` into `xx_h_air_surface_tr_hangwall_reception_front_straight_min`,
    // hanging at x 49.17, 5.36 m up (route wall1_tapjump). Ours ran up the wall and rebounded off it.
    Scenario {
        name: "Damascus: the tap jump at the bureau's wall flies at AC1's wall-hang target (the scorer's pick) into the wall hang",
        tags: &["jump", "hang"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "47.6,-20,-90,5.55"),
            ("AC1_WALK", "2"),
            ("AC1_STOP", "1.2"),
            ("AC1_HIGH", "0-4"),
            ("AC1_LEGS", "0-0.3"),
        ],
        secs: 3.0,
        want: &["running jump at the wall's ledge [49.6", "_to_surface at", "-> hangwall_open via xx_hangwall_reception_front_straight_min_c at [49.0"],
        never: &["EMBED", "wallingfront"],
    },
    // Audit JUMP-3: free running south over the bureau's roofs (route roof1_free_south). The running game's first jump,
    // 0.59 s into the run, took the running takeoff; its second, 0.31 s after the first reception handed over to the
    // ground, the impulsion (`xx_h_impultionstraight_footr_to_ground_to_air_front_down_footl`, at y 14.35) and the free-step
    // takeoff. (The first jump goes 2.3 m onto the next roof's rim, `running jump aimed at [53.00, 11.89, -15.88]`, as the
    // live log's: ours had jumped onto the parapet 0.3 m on and from it again.)
    Scenario {
        name: "Damascus: free running over the bureau's roofs, the jump soon after a reception takes off with AC1's impulsion",
        tags: &["jump", "combo"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "53,-20.4,-180,12.5"),
            ("AC1_WALK", "6.2"),
            ("AC1_HIGH", "0-5"),
            ("AC1_LEGS", "0-5"),
        ],
        secs: 3.0,
        want: &[
            "via xx_h_run_front_300cm_footr_to_air at [53.00, 11.8",
            "running jump aimed at [53.00, 11.89, -15.8",
            "via xx_h_impultionstraight_",
            "_to_ground_to_air_front_down_foot",
            "at [53.00, 11.89, -14.",
            "via xx_h_freestep_front_front_300cm_",
        ],
        never: &["EMBED"],
    },
    // Live log route wall1_hang_right (2026-10-12): from the wall run's catch on the bureau's wall, right: the running game
    // took the closing step and stopped (a beam sticks out of the wall 0.6 m under the hold 0.9 m on: AC1's lead-hand sweep,
    // a capsule from over the hand down to the hang's root, 0xDD9640); ours reached on 0.43 m more.
    Scenario {
        name: "Damascus: along the bureau wall's hang to the right stops short of the beam under it (AC1's shimmy sweep)",
        tags: &["hang"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "47.6,-20,-90,5.55"),
            ("AC1_WALK", "6.2"),
            ("AC1_STOP", "1.0"),
            ("AC1_HIGH", "0-5"),
            ("AC1_LEGS", "0-1.0"),
            ("AC1_CLIMB", "nograb,wait=1.6,right=3.0"),
        ],
        secs: 5.0,
        want: &["via xx_h_hangwall_strafe_right_050cm_close", "no room for the lead hand's reach"],
        never: &["EMBED", "via xx_h_hangwall_strafe_right_050cm_open"],
    },
    // Live log route seams_free_west (2026-10-12): free running west over the roofs south of the hiding spot, the running
    // game ran on over a 2 cm crack between two roofs' collision at x 37.99; ours lost the ground there, stepped off and
    // fell 8 m into the street.
    Scenario {
        name: "Damascus: free running over a crack between two roofs runs on (the ground is not lost in it)",
        tags: &["ground", "combo"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "52,-33.4,90,13.3"),
            ("AC1_WALK", "6.2"),
            ("AC1_HIGH", "0-4"),
            ("AC1_LEGS", "0-4"),
        ],
        secs: 3.5,
        want: &["walking toward [-1.00, 0.00]"],
        never: &["EMBED", "step off the edge", "landed from"],
    },
    // Live log route wall1_standjump: high profile pressed, then 0.37 s later the legs, held 0.34 s: the running game
    // crouched and jumped to the bureau wall's hang (the press came during the high profile's stance change, which ours'
    // script did not pass on).
    Scenario {
        name: "Damascus: the standing jump pressed during the high profile's stance change jumps to the bureau wall's hold",
        tags: &["jump", "hang"],
        env: &[("AC1_LEVEL", "damascus"), ("AC1_NO_CROWD", "1"), ("AC1_START", "48.75,-20,-90,5.55"), ("AC1_HIGH", "0-3.2"), ("AC1_LEGS", "0.37-0.71")],
        secs: 2.5,
        want: &["crouched for the static jump", "-> leap via xx_h_jumpstraight_footl_to_hangwall_"],
        never: &["EMBED"],
    },
    // Live log route ladder68_walk: walking onto ladder 68 in low profile, the running game stepped on with the low
    // profile's entry (0x010A34A7); ours played the high profile's.
    Scenario {
        name: "Damascus: walking onto a ladder in low profile steps on with AC1's low profile entry",
        tags: &["ladder"],
        env: &[("AC1_LEVEL", "damascus"), ("AC1_NO_CROWD", "1"), ("AC1_START", "33.6,-16.82,-90,5.5"), ("AC1_WALK", "1.9"), ("AC1_STICK", "1")],
        secs: 4.0,
        want: &["via xx_l_wait_hipm_footl_tr_l_ladder_climb_up_r", "onto ladder 68"],
        never: &["EMBED", "tr_h_ladder_climb_up"],
    },
    // Audit ANIM-4 to 6: blends as AC1's move graph gives them. The running jump's takeoff is started by the code: it
    // blends in over its action's in-transition (AROLLBROLL 0.1 s); the swing cycle's items follow one another by cuts
    // where they line up.
    Scenario {
        name: "blends from AC1's move graph: the takeoff's in-transition, the swing's cuts (audit ANIM-4 to 6)",
        tags: &["bars", "jump"],
        env: &[("AC1_START", "50.3,0,-90"), ("AC1_WALK", "6"), ("AC1_HIGH", "0-10"), ("AC1_LEGS", "0-10")],
        secs: 3.0,
        want: &["blend into xx_h_run_front_300cm_footr_to_air: kind 1 0.10 s", "blend into xx_h_swing_cycle_back_down: kind 0 0.00 s -> None"],
        never: &["EMBED"],
    },
    // The eject with nothing to land on (`back_eject`), from a wall run's catch at the hang on the test level's wall: the
    // stick to the left of the wall goes left (toward -Z here), to the right right. (The climbing stick is read facing the
    // wall; passed unturned into the facing-away side, A ejected right and D left: user report, 2026-10-12.) It goes the
    // stick's way, AC1's `jump_off_wall_dir`: straight to the side is turned in to 89 degrees off straight away (it was
    // always 45 degrees: user report #10, 2026-10-12).
    Scenario {
        name: "eject from a wall run's hang with the stick left: off to the left (the facing-away side turned)",
        tags: &["hang", "wallrun"],
        env: &[
            ("AC1_START", "19.5,15.3,-90"),
            ("AC1_WALK", "6.2"),
            ("AC1_STOP", "1.0"),
            ("AC1_HIGH", "0-5"),
            ("AC1_LEGS", "0-1.0"),
            ("AC1_JUMP", "2.6"),
            ("AC1_CLIMB", "nograb,wait=2.4,left=1.2"),
        ],
        secs: 4.5,
        want: &["back eject, 89 degrees off straight away", "the run ends at [21.3", "0.00, 7.37] (game"],
        never: &["EMBED"],
    },
    Scenario {
        name: "eject from a wall run's hang with the stick right: off to the right",
        tags: &["hang", "wallrun"],
        env: &[
            ("AC1_START", "19.5,15.3,-90"),
            ("AC1_WALK", "6.2"),
            ("AC1_STOP", "1.0"),
            ("AC1_HIGH", "0-5"),
            ("AC1_LEGS", "0-1.0"),
            ("AC1_JUMP", "2.6"),
            ("AC1_CLIMB", "nograb,wait=2.4,right=1.2"),
        ],
        secs: 4.5,
        want: &["back eject, -89 degrees off straight away", "the run ends at [21.3", "0.00, 23.23] (game"],
        never: &["EMBED"],
    },
    // User report #11 (Damascus, by a market stall): the running jump at a house's roof edge flew the surface flight into
    // the wall hang, its hands taking each the hold nearest where the clips ended: one the roof edge 8 m up, the other the
    // stall's awning 5 m up, and he hung from the air between them (hands 6.4 m up at z -4.6). Now the hands land on the
    // hold aimed at.
    Scenario {
        name: "Damascus report #11: the running jump at the roof edge by the market stall hangs from the edge (not the air)",
        tags: &["jump", "hang"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "178.04,-2.44,-69.2,10.5"),
            ("AC1_WALK", "6.2"),
            ("AC1_HIGH", "0-4"),
            ("AC1_LEGS", "0-4"),
        ],
        secs: 3.5,
        want: &["running jump at the wall's ledge [188.38, 8.00, -6.37]", "-> hangwall_open via xx_hangwall_reception_front_straight_max_c at [187.7"],
        never: &["EMBED", "reception_front_straight_max_c at [187.67, 5.4"],
    },
    // User report #13 (Damascus): a wall run up nothing drawn, on the collision of a scaffold (`Scaffold_2x3x8_01A01`, a
    // skinned prop with 69 bones) left undrawn as a character. Now drawn; no entity is collided with and not drawn.
    Scenario {
        name: "Damascus report #13: the wall run by the crates runs up the scaffold, drawn (nothing collided with is undrawn)",
        tags: &["wallrun"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "226.82,-13.47,41.8,4.5"),
            ("AC1_WALK", "6.2"),
            ("AC1_HIGH", "0-5"),
            ("AC1_LEGS", "0-5"),
            ("AC1_PATH", "225.37,-15.75;225.6,-18.2;226.3,-19.4;227.5,-20.6"),
            // (The city built, not read from the level cache: the undrawn count is the build's log.)
            ("AC1_NO_CACHE", "1"),
        ],
        secs: 3.0,
        want: &["0 entities collided with and none of their meshes drawn", "via xx_h_wallingfront_entry_footl_a at [226."],
        never: &["EMBED"],
    },
    // User report #6 (Damascus): free running off a roof at a building's ledge 7.8 m up, he took a window 2 m below it (the
    // per-hand hold search of #11) and, leaping to the side from there, went 2.4 m down through the porch roof he stood on.
    // Now he catches the ledge.
    Scenario {
        name: "Damascus report #6: free running off the roof at the building's ledge catches the ledge (not a window below)",
        tags: &["jump", "hang"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "103.14,-51.31,-99.6,8.5"),
            ("AC1_WALK", "6.2"),
            ("AC1_HIGH", "0-4"),
            ("AC1_LEGS", "0-4"),
            ("AC1_PATH", "105.2,-50.21;106.08,-48.85;106.2,-47.6;106.0,-40"),
        ],
        secs: 3.0,
        want: &["found the hold at [106.", ", 7.77, -42.8", "-> free_1m via xx_fall_tr_hangfree_min_b"],
        never: &["EMBED", "hangwall_reception"],
    },
    Scenario {
        // (AC1's Ghost mode, the Human Debug context: no collision, `Ghost_mode01` then the loop; out, request 33's
        // `xx_h_jump_falling01` and a fall from rest. Banned445's port.)
        name: "Ghost mode: up 2.5 m, through block L and out its far side, then the fall out of the flight to the ground",
        tags: &["misc"],
        env: &[("AC1_START", "14,-7.5,-90"), ("AC1_WALK", "1"), ("AC1_STOP", "2.9"), ("AC1_FLY", "0.3-3.0"), ("AC1_FLY_UP", "0.3-0.8")],
        secs: 4.5,
        want: &[
            "fly: Ghost mode on",
            "fly: through a surface at [22.",
            "fly: through a surface at [25.",
            "fall via xx_h_jump_falling01.man",
            "Ghost mode off at [27.",
            "falling at 0.0 m/s",
            "landed from 2.5 m",
            "-> ground via",
        ],
        never: &["EMBED", "landing_death"],
    },
    Scenario {
        // (Leaving with the empty hand held: the fall keeps the flight's velocity, here × 3 in high profile.)
        name: "Ghost mode: up 5 m, forward in high profile (x 3), thrown out of the flight with the empty hand held",
        tags: &["misc"],
        env: &[
            ("AC1_START", "-25,-8,0"),
            ("AC1_WALK", "1"),
            ("AC1_STOP", "0-1.3"),
            ("AC1_FLY", "0.3-2.0"),
            ("AC1_FLY_UP", "0.3-1.3"),
            ("AC1_HIGH", "1.3-2.1"),
            ("AC1_GRASP", "1.9-2.1"),
        ],
        secs: 4.0,
        want: &["Ghost mode off at [-25.00, 5.00, -18.50], falling at 15.0 m/s", "landed from 5.0 m", "-> ground via"],
        never: &["EMBED", "landing_death"],
    },
    // Collision as AC1's (docs/NOTES.md "What collides"): only the rigid bodies whose layer stops the player. The
    // bureau's `BureauCamCol01` is a CameraBarrier, a 0.5 m block on the bureau's floor the body stood on (at 4.64 m).
    Scenario {
        name: "Damascus: in the bureau, standing on its floor where AC1's camera barrier block is (passed through, not stood on)",
        tags: &["ground"],
        env: &[("AC1_LEVEL", "damascus"), ("AC1_NO_CROWD", "1"), ("AC1_START", "40,0,180,6")],
        secs: 0.5,
        want: &["the run ends at [40.00, 4.15, 0.00]"],
        never: &["EMBED"],
    },
    // The south-east gate's `CollisionHorse` is a HorseBarrier: the player walked into (and started on top of) an
    // invisible wall there, 3 m high; walking west through it he now goes on.
    Scenario {
        name: "Damascus: walking west through AC1's horse barrier by the south-east gate (it stops horses, not the player)",
        tags: &["ground"],
        env: &[("AC1_LEVEL", "damascus"), ("AC1_NO_CROWD", "1"), ("AC1_START", "184,-192,90,8"), ("AC1_WALK", "3")],
        secs: 4.0,
        // (Past the barrier's line, x 178.4: where along depends on the walk's start, 173 to 177 m.)
        want: &["walking toward [-1.00, 0.00] from [184.00, 0.00, -192.00]", "the run ends at [17"],
        never: &["EMBED", "the run ends at [178.", "the run ends at [179.", "the run ends at [18"],
    },
    Scenario {
        name: "eject from a wall run's hang with the stick back and left: off 45 degrees to the left, the stick's way (user report #10)",
        tags: &["hang", "wallrun", "eject"],
        env: &[
            ("AC1_START", "19.5,15.3,-90"),
            ("AC1_WALK", "6.2"),
            ("AC1_STOP", "1.0"),
            ("AC1_HIGH", "0-5"),
            ("AC1_LEGS", "0-1.0"),
            ("AC1_JUMP", "2.6"),
            ("AC1_CLIMB", "nograb,wait=2.4,downleft=1.2"),
        ],
        secs: 4.5,
        want: &["via xx_h_hangwall_tr_rebound_footr", "back eject, 45 degrees off straight away", "the run ends at [16.3", "0.00, 10.14] (game"],
        never: &["EMBED"],
    },
    // The eject goes by the stick as the camera sees it, not the climbing stick in the character's frame (AC1's jump off a
    // wall reads the interpreter's camera-relative direction): the climbing stick straight down, the camera's stick back
    // and to the left, 45 degrees off the wall (user report #10, 2026-10-12).
    Scenario {
        name: "eject from a wall run's hang goes the camera's stick's way: back and left with the climbing stick straight down",
        tags: &["hang", "wallrun", "eject"],
        env: &[
            ("AC1_START", "19.5,15.3,-90"),
            ("AC1_WALK", "6.2"),
            ("AC1_STOP", "1.0-2.5"),
            ("AC1_VEER", "2.4,135"),
            ("AC1_HIGH", "0-5"),
            ("AC1_LEGS", "0-1.0"),
            ("AC1_JUMP", "2.6"),
            ("AC1_CLIMB", "nograb,wait=2.4,down=1.2"),
        ],
        secs: 4.5,
        want: &["back eject, 45 degrees off straight away", "-> land via xx_h_landing_damage_footl_roll at [16.3", "0.00, 10.14] yaw 45"],
        never: &["EMBED"],
    },
    // User report #1 (2026-10-12): stuck in a Damascus rooftop hiding place. The hop out's room was tested along a ray
    // from over its middle, 1.25 m over its 1.07 m rim, which ran into the tarp's frame every way; and the hop was stopped
    // at the box's own wall. Now AC1's `Guard_HopOut` box past the footprint, the hiding place's own shapes left out.
    Scenario {
        name: "Damascus: free running into a rooftop hiding place, then out of it again (user report #1)",
        tags: &["hay"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "185.6,-137.72,90,9"),
            ("AC1_WALK", "6.2"),
            ("AC1_HIGH", "0-1.2"),
            ("AC1_LEGS", "0-1.2"),
            ("AC1_STOP", "1.2-30"),
            ("AC1_CLIMB", "nograb,wait=3.0,up=0.3"),
        ],
        secs: 4.5,
        want: &[
            "dive into the haystack at [181.73, 7.72, -137.72]",
            "hay -> hayout via xx_l_haystack_hop_out",
            "hayout -> ground via xx_l_haystack_hop_out_tr_l_wait at [181.73, 7.72, -139.9",
        ],
        never: &["no room to hop out"],
    },
    // User report #12 (2026-10-12): a Damascus roof's parapet (0.3 m wide, the roof 0.24 m under it, the street 8 m) was
    // a narrow top to balance on, from its hold facing the roof; standing by it with the stick along it he got onto it
    // and crouch-walked it. AC1 runs along it as ground.
    Scenario {
        name: "Damascus: standing by a roof's parapet, the stick along it walks the roof, not onto it as a beam (user report #12)",
        tags: &["beam", "ground"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "192.36,-11.2,20,9"),
            ("AC1_WALK", "1.9"),
            ("AC1_STOP", "0-1.0"),
            ("AC1_VEER", "0.9,70"),
        ],
        secs: 3.5,
        want: &["the run ends at [188."],
        never: &["onto perch", "EMBED"],
    },
    Scenario {
        name: "Damascus: walking up to a roof's parapet and along it stays on the roof (user report #12)",
        tags: &["beam", "ground"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "193.4,-10.0,45,9"),
            ("AC1_WALK", "1.9"),
            ("AC1_PATH", "192.36,-11.4;188.0,-11.4;stop"),
        ],
        secs: 5.0,
        want: &["waypoint 2 reached"],
        never: &["onto perch", "EMBED"],
    },
    // User report #9 (2026-10-12): off a beam's end at a Damascus balcony's handrail (its top 2.34 m over the beam, the
    // floor behind it under the rail), the wall run pulls up onto the rail, AC1's walling probe D taking the rail's edge
    // as a lip with room over it (class 1), and steps off it onto the balcony. Ours topped out with nothing to grab and
    // rebounded. (The wall run starts 0.03 m from the balcony's front, the beam's end: EMBED there is not this.)
    Scenario {
        name: "Damascus: a wall run off a beam at a balcony's handrail pulls up onto the rail and over it (report #9)",
        tags: &["wallrun", "beam"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "112,-33.58,-90,5.6"),
            ("AC1_WALK", "6.2"),
            ("AC1_HIGH", "0-4"),
            ("AC1_LEGS", "0-4"),
        ],
        secs: 5.0,
        want: &["via xx_h_wallingfront_step1_footr_tr_hangknee_250cm_footr", "hangknee -> perch via", "at [115.8", "landed from"],
        never: &["tops out with nothing to grab", "rebound"],
    },
    // User reports #7 and #8: up the wall under a Damascus balcony held out on stepped corbels, each step 0.45 m further
    // out and 0.12-0.3 m up. At the wall's top the stick up jumps up onto the corbels (AC1's TryBackEject, from a raised
    // hand's pose too), the free hang climbs them hand over hand out to the balcony's front (AC1's vertical rows, an edge
    // up to 0.5 m in or out) and pulls up onto it. Ours stepped 1m -> 1ru -> 1m over and over.
    Scenario {
        name: "Damascus: up the wall, onto the balcony's corbels and out along them, up onto the balcony (report #7)",
        tags: &["climb", "hang"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "112,-24.4,-90,5"),
            ("AC1_WALK", "6.2"),
            ("AC1_HIGH", "0-2"),
            ("AC1_LEGS", "0-2"),
            ("AC1_STOP", "2.5"),
            ("AC1_CLIMB", "nograb,wait=3,up=14"),
        ],
        secs: 17.0,
        want: &[
            "jump up from the climb to the ledge at [114.10, 11.48",
            // (How far up is measured from the free hang's hands, which ease down to AC1's 2.4 m over the root: 0.12-0.13.)
            "hand steps onto the edge 0.1",
            "up and 0.46 out",
            "hand steps onto the edge 0.3",
            // (0.38: the free hang's wait after the jump up from the climb is AC1's mix by how far out it jumped, its hands
            // 0.01 m further out than the closed wait's; audit LEDGE-15.)
            "up and 0.38 out",
            "-> top via xx_h_hangknee_footl_tr_freestep_entry_footl at [113.4",
        ],
        never: &["xx_l_climb_1ru_d_1m", "EMBED"],
    },
    Scenario {
        name: "Damascus: the same balcony from further along its wall, after a move to the left (report #8)",
        tags: &["climb", "hang"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "112,-24.4,-90,5"),
            ("AC1_WALK", "6.2"),
            ("AC1_HIGH", "0-2"),
            ("AC1_LEGS", "0-2"),
            ("AC1_STOP", "2.5"),
            ("AC1_CLIMB", "nograb,wait=3,up=3.4,left=1.2,up=10"),
        ],
        secs: 18.0,
        want: &["jump up from the climb to the ledge at [114.10, 11.48, -26.", "-> top via xx_h_hangknee_footl_tr_freestep_entry_footl at [113.4"],
        never: &["EMBED"],
    },
    // Audit ANIM-14: the robe and the secondary motion run on AC1's own data (the cloth settings, its soft mesh and
    // capsules, every bone modifier, the wind), through a sprint and a jump (the pop check reads the swung bones).
    Scenario {
        name: "the robe, the skirt, hood and sword tag run on AC1's cloth settings, bone modifiers and wind",
        tags: &["misc"],
        env: &[("AC1_WALK", "7"), ("AC1_HIGH", "0-5"), ("AC1_JUMP", "1.5")],
        secs: 3.0,
        want: &["robe cloth: 146 vertices (44 pinned), 253 edges, 6 capsules, AC1's settings", "56 of 56 bone modifiers", "wind: Wind_Masyaf_001"],
        never: &["cloth not read", "drawn skinned"],
    },
    // User reports #3-#5 (towers): the octagonal tower east of the city (289, 46), its faces 22-34 degrees apart. Climbing
    // right off its south face, the climber turns to each angled face its holds are on (AC1 faces its climber along its
    // holds' wall normals); ours kept facing the south face, the hands on the next face and the body in the air. The
    // step off sideways onto the 0.25 m cornice band under the W face (fallen off at once) is turned down.
    Scenario {
        name: "Damascus octagonal tower: climbing right round its angled faces turns the body to each (user reports #4, #5)",
        tags: &["climb"],
        env: &[("AC1_LEVEL", "damascus"), ("AC1_NO_CROWD", "1"), ("AC1_START", "289.3,42.6,180,16"), ("AC1_CLIMB", "up=8,right=6")],
        secs: 16.0,
        // (Round the next angled corner: past it on the climbing holds, as AC1's free-hang body boxes (0xB2DD50) turn down the
        // swing off the wall into a free hang there, the next face in the way.)
        want: &["onto holds on a face 34 degrees round, turning to it", "via xx_l_climb_2m_r_1m at [286.56, 21.14"],
        never: &["EMBED", "stepped off the wall sideways"],
    },
    // User report #3: the minaret by the viewpoint at (128, -12) (`Minaret_M_02a`, the wooden balcony on its corbel). The jump
    // up from its shaft hangs him free under the balcony's floor edge; AC1's free hang rests 2.4 m under its hold (ours
    // 2.1: the hands in the air over it), and the balcony's knobs (0.10-0.19 m guidance pieces, AC1's probes hold them at
    // their middle, audit GUIDE-2; ours dropped them) take a hand on the way up onto the railing.
    Scenario {
        name: "Damascus minaret: from the free hang under the balcony, a hand onto the railing's knob, up onto the railing (user report #3)",
        tags: &["climb", "hang"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "129.3,-18.1,180,17"),
            ("AC1_WALK", "6.2"),
            ("AC1_STOP", "1.0"),
            ("AC1_HIGH", "0-3"),
            ("AC1_LEGS", "0-1.0"),
            ("AC1_CLIMB", "nograb,wait=2.5,up=20"),
        ],
        secs: 24.0,
        want: &[
            "jump up from the climb to the ledge at [129.2",
            "24.24, -17.75]",
            "free_open -> free_1lu via xx_h_hangfree_climb_1m_u_1lu",
            "-> perch via xx_h_hangknee_onehand_footl_to_freestep_entry_footl at [129.37, 25.47",
        ],
        never: &["EMBED"],
    },
    // The climbing sweep (`AC1_CLIMB_SWEEP`, test level, run e2-hang): pulled down off the roof of the block at (60, 12),
    // left along its edge to the wall's end, where AC1's TrySwitchHangType (0xDE1060) goes into the free hang (one foot
    // ray off the wall) after its body boxes (0xB2DD50), the root under the holds (0xB15AD0). Ours ended the root 0.06 m
    // past the hold line, the legs inside the wall's corner (z 12.00, the face; now 12.06, the hold line).
    Scenario {
        name: "pulled down off a roof, left to the wall's end: into the free hang with the body under the holds, not in the wall",
        tags: &["hang", "climb"],
        env: &[
            ("AC1_START", "60.00,11.06,-180,12.89"),
            ("AC1_WALK", "1.5"),
            ("AC1_STOP", "1.0-60"),
            ("AC1_HAND", "1.8"),
            ("AC1_CLIMB", "nograb,wait=3.5,left=3,right=1"),
        ],
        secs: 9.0,
        want: &["hangwall -> free_1m via xx_h_hangwall_tr_hangfree_left", "-> hangwall_open via xx_h_hangfree_tr_hangwall_right at [58.08, 10.19, 12.06]"],
        never: &["EMBED"],
    },
    // The climbing sweep (test level, run w10-up): under the slab at (-92.9, 164), a cornice 0.8 m deep in front of a wall
    // going on up, the free hang's pull-up was refused (ours' inside-a-solid probe 0.95 m in, inside that wall) and its hand
    // step took the edge it held for a row 0.3 m up: up and down onto the same edge four times before it got over.
    Scenario {
        name: "free hang under a deep cornice, up: straight over the top, no hand stepping onto the edge it holds",
        tags: &["hang", "climb"],
        env: &[("AC1_START", "-92.61,164,-90,0.3"), ("AC1_CLIMB", "up=8")],
        secs: 9.0,
        want: &["free_open -> hangknee via xx_h_hangfree_tr_hangwaist_a", "-> top via xx_h_hangknee_footl_tr_freestep_entry_footl at [-92.65, 3.99"],
        never: &["via xx_h_hangfree_climb_1ru_u_1m", "via xx_h_hangfree_climb_1lu_u_1m", "EMBED"],
    },
    // Audit LEDGE-15 / LEDGE-7: the standing jump at a hold 2.75 m up with the wall under it, AC1's 2.5-3 m band (0xB21DA0):
    // the wall-free hang (flight 0x0121A8B1, reception 0x0121B072, `xx_h_hangwallfree_wait`); up: CanPullup's switch onto the
    // wall under it in place (`xx_h_hangfree_tr_hangwall`, 0xDE2270 -> 0xDD4160). Ours hung from it in the wall hang.
    Scenario {
        name: "gauntlet G24: standing jump at a hold 2.75 m up: AC1's wall-free hang, then up onto the wall in place",
        tags: &["jump", "hang"],
        env: &[("AC1_START", "-92.6,261.75,-90"), ("AC1_HIGH", "0-2"), ("AC1_JUMP", "0.5"), ("AC1_CLIMB", "nograb,wait=3.5,up=1.5")],
        secs: 6.0,
        want: &["-> free_1m via xx_h_jumpstraight_footl_to_hangwallfree_", "in the wall-free hang", "-> hangwall_open via xx_h_hangfree_tr_hangwall at"],
        never: &["EMBED", "xx_h_jumpstraight_footl_to_hangwall_2"],
    },
    // Audit LEDGE-15 (LEDGE-6's swing back, its way on): from the wall hang 2.4 m up, up with nothing on the wall: AC1's swing
    // back onto the slab's edge 0.8 m out (`hangwall_to_swingback_up_*` mixed), then the move graph's connector into the free
    // wait, `xx_h_hangfree_swingback_min_a/b` (0x0564BB0E, weighed [1, 0] by 0xDE1FE0), the wait half wall-free by how far out.
    Scenario {
        name: "gauntlet G24: from the wall hang, up: AC1's swing back onto the slab out from the wall, settling under it",
        tags: &["jump", "hang"],
        env: &[("AC1_START", "-92.6,266.25,-90,1"), ("AC1_HIGH", "0-2"), ("AC1_JUMP", "0.5"), ("AC1_CLIMB", "nograb,wait=3.0,up=1.0")],
        secs: 7.0,
        want: &["-> hangwall_open via xx_h_jumpstraight_footl_to_hangwall_", "swing back up to the ledge", "-> free_1m via xx_h_hangfree_swingback_min_b"],
        never: &["EMBED"],
    },
    // Audit LEDGE-15: down from the wall hang to a hold where not both feet find the wall: AC1's TrySwitchHangType at the hand
    // step's target, `xx_h_hangwall_tr_hangfree_down_left` (0x01C3217C), in place of the step. Ours had no move there.
    Scenario {
        name: "gauntlet G24: pulled down off a slab into the wall hang, down to a hold with no wall under it: AC1's switch into the free hang",
        tags: &["hang"],
        env: &[("AC1_START", "-90,272,90,5"), ("AC1_WALK", "1.5"), ("AC1_STOP", "1.2-30"), ("AC1_HAND", "2.0"), ("AC1_CLIMB", "nograb,wait=5.0,down=1.0")],
        secs: 9.0,
        want: &["-> hangwall_open via xx_l_ledge_pulldown_soft_to_hangwall_straight_b", "-> free_1m via xx_h_hangwall_tr_hangfree_down_left"],
        never: &["EMBED"],
    },
    // User report 2026-10-13 (recording 214883, "the inverted triangle climbs are bugged, Altair can't go down and when he
    // goes up he grabs the air"): pulled down off the front of a Damascus awning (`Awning_6x2_Wood_01A`, its frame's beams
    // under it each 0.13-0.31 m lower and 0.38-0.49 m further in) into a free hang. Down hand-steps along the beams (AC1's
    // vertical rows down; ours took one with a stance's grid step and stopped), up back along them and over the front
    // (ours' grid step put the hands 0.46 m over the beam, in the air).
    Scenario {
        name: "Damascus awning frame: from the free hang at its front, down its beams hand by hand and back up over it (user report 2026-10-13)",
        tags: &["hang", "climb"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "82.1,-43.97,-68.3,4.3"),
            ("AC1_HAND", "0.5"),
            ("AC1_CLIMB", "nograb,wait=3.5,down=4,up=5"),
        ],
        secs: 13.0,
        want: &[
            "hand steps onto the edge -0.32 up and -0.38 out at [82.14, 3.72",
            "hand steps onto the edge -0.12 up and -0.46 out at [82.14, 3.59",
            "hand steps onto the edge 0.33 up and 0.38 out at [82.14, 4.03",
            "-> top via xx_h_hangknee_footl_tr_freestep_entry_footl at [82.1",
        ],
        never: &["EMBED"],
    },
    // User report 2026-10-13 (recording 156949, "he grabs the air"): from the scaffold's planks at (41, 7.7, -25) a running jump
    // at the house wall catches a knob 9.99 m up (an authored guidance piece 0.17 m long, `House_P_6x6x8_01a`). The hang
    // after AC1's reception is the hands-together one, both hands on the knob; it was the hands-apart hang, 0.3 m either
    // side of it in the air. The reception before it too: both hands onto the knob from the hands' catch (AC1's hand goals
    // are their contacts, both on it); its clips hold them 0.6 m apart (user report 156949, the 1.8 s reception).
    Scenario {
        name: "Damascus: a running jump at a knob on a house wall hangs from it with both hands on it (user report 2026-10-13)",
        tags: &["jump", "hang"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "41.15,-24.3,0,8"),
            ("AC1_WALK", "6.2"),
            ("AC1_HIGH", "0-3"),
            ("AC1_LEGS", "0-3"),
            ("AC1_STOP", "0.6"),
        ],
        secs: 5.0,
        want: &[
            "running jump at the wall's ledge [41.14, 9.99, -30.01]",
            "both hands onto the knob over the reception",
            "drop -> hangwall via xx_hangwall_reception_front_straight_max_c at [41.15, 8.87",
        ],
        never: &["-> hangwall_open via xx_hangwall_reception", "EMBED"],
    },
    // Audit LEDGE-15: from a free hang up to a row where a foot finds the wall: AC1's TrySwitchHangType at the hand step's
    // target, `xx_h_hangfree_tr_hangwall_right` (0x01C32563) with the root interpolated a row up (0x711130, the clip's own
    // root motion unused); then down again, `xx_h_hangwall_tr_hangfree_down_left`. Ours stepped up in the free hang.
    Scenario {
        name: "gauntlet G24: from a free hang up a row onto the wall under it: AC1's switch into the wall hang a row up, then back down",
        tags: &["jump", "hang"],
        env: &[("AC1_START", "-92.6,284,-90"), ("AC1_HIGH", "0-2"), ("AC1_JUMP", "0.5"), ("AC1_CLIMB", "nograb,wait=3.5,up=1.0,wait=2.0,down=1.0")],
        secs: 9.0,
        want: &[
            "-> free_1m via xx_h_jumpstraight_footl_to_hangfree_",
            "the free hang onto the wall a row up",
            "-> hangwall_open via xx_h_hangfree_tr_hangwall_right",
            "-> free_1m via xx_h_hangwall_tr_hangfree_down_left",
        ],
        never: &["EMBED"],
    },
    // Audit LEDGE-15: a wall run under the slab sticking out 0.8 m: AC1's free catch off the run (0x0564BB1D / E, its
    // `_swingback_min` and `_max` clips mixed by how far out the edge is), on through the connector into the free wait
    // (`xx_h_hangfree_swingback_min_a/b`), that wait mixed by the same share (0xE37590 sets the wait kind 3 and the weight).
    Scenario {
        name: "gauntlet G24: free running at the wall under the slab: AC1's free catch off the wall run, settling under the edge",
        tags: &["wallrun", "hang"],
        env: &[("AC1_START", "-98,266.25,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-3"), ("AC1_LEGS", "0-3")],
        secs: 5.0,
        want: &["tr_hangfree_", "_swingback_max", "-> free_1m via xx_h_hangfree_swingback_min_b"],
        never: &["EMBED"],
    },
    // Combat (`combat`, docs/COMBAT.md): a militia guard at the fight yard (`AC1_GUARDS`), the player 2 m off facing it.
    Scenario {
        name: "fight: the sword drawn (3 pressed) and sheathed (3 again), HumanWeapon's upper-body pair over the legs",
        tags: &["fight"],
        env: &[("AC1_GUARDS", "26,-45,90"), ("AC1_START", "20,-45,-90"), ("AC1_WEAPON", "0.5:3,2.5:3")],
        secs: 4.0,
        want: &["drawing the sword", "drew the sword", "sheathing the sword", "sheathed the sword"],
        never: &["combat: no clip", "EMBED"],
    },
    Scenario {
        name: "fight: an attack chain on the dummy: AC1's attack by side and distance, then two combo hits pressed as the strikes land",
        tags: &["fight"],
        env: &[
            ("AC1_GUARDS", "22,-45,90"),
            ("AC1_GUARD_PASSIVE", "1"),
            ("AC1_GUARD_BLOCK", "0"),
            ("AC1_GUARD_HP", "5"),
            ("AC1_START", "20,-45,-90"),
            ("AC1_WEAPON", "0.5:3"),
            ("AC1_LOCK", "2.0"),
            ("AC1_ATTACK", "3.0,combo,combo"),
        ],
        secs: 6.0,
        want: &["fight mode on", "attack xx_h_light_front_", "the strike hits", "combo hit 1 xx_h_light_combo_", "combo hit 2 xx_h_light_combo_"],
        never: &["the strike misses", "combat: no clip", "EMBED"],
    },
    Scenario {
        name: "fight: the timed hit that would kill is AC1's paired combo kill; the guard goes limp",
        tags: &["fight", "ragdoll"],
        env: &[
            ("AC1_GUARDS", "22,-45,90"),
            ("AC1_GUARD_PASSIVE", "1"),
            ("AC1_GUARD_BLOCK", "0"),
            ("AC1_START", "20,-45,-90"),
            ("AC1_WEAPON", "0.5:3"),
            ("AC1_LOCK", "2.0"),
            ("AC1_ATTACK", "3.0,combo,combo"),
        ],
        secs: 8.0,
        want: &["combo kill xx_h_light_combo_light_0", "Paired xx_h_light_combo_light_0", "limp (the ragdoll)"],
        never: &["combat: no clip", "EMBED"],
    },
    Scenario {
        name: "fight: a first attack the guard blocks (block reception), the player's blow stopped",
        tags: &["fight"],
        env: &[
            ("AC1_GUARDS", "22,-45,90"),
            ("AC1_GUARD_PASSIVE", "1"),
            ("AC1_GUARD_BLOCK", "1"),
            ("AC1_START", "20,-45,-90"),
            ("AC1_WEAPON", "0.5:3"),
            ("AC1_LOCK", "2.0"),
            ("AC1_ATTACK", "3.0"),
        ],
        secs: 5.0,
        want: &["the strike is blocked", "Block xx_h_light_block_"],
        never: &["combat: no clip", "EMBED"],
    },
    Scenario {
        name: "fight: defending (high profile), the weapon hand as the guard's blow comes: the paired counter kill",
        tags: &["fight", "ragdoll"],
        env: &[
            ("AC1_GUARDS", "22,-45,90"),
            ("AC1_START", "20,-45,-90"),
            ("AC1_WEAPON", "0.5:3"),
            ("AC1_LOCK", "2.0"),
            ("AC1_HIGH", "2.5-9"),
            ("AC1_ATTACK", "counter"),
        ],
        secs: 8.0,
        want: &["counter kill xx_h_light_counter_light_01_kill_", "against xx_h_light_countered_light_01_death_", "limp (the ragdoll)"],
        never: &["mistimed", "combat: no clip", "EMBED"],
    },
    Scenario {
        name: "fight: defending without the counter: the guard's blow is blocked",
        tags: &["fight"],
        env: &[("AC1_GUARDS", "22,-45,90"), ("AC1_START", "20,-45,-90"), ("AC1_WEAPON", "0.5:3"), ("AC1_LOCK", "2.0"), ("AC1_HIGH", "2.5-9")],
        secs: 6.0,
        want: &["attack xx_h_light_front_", "the strike is blocked"],
        never: &["the strike hits", "combat: no clip", "EMBED"],
    },
    Scenario {
        name: "fight: the hidden blade behind an unaware guard in low profile: AC1's stealth back assassination, played paired",
        tags: &["fight", "ragdoll"],
        env: &[("AC1_GUARDS", "22,-45,90"), ("AC1_START", "23.2,-45,90"), ("AC1_WEAPON", "0.5:2"), ("AC1_ATTACK", "1.5")],
        secs: 8.0,
        want: &[
            "selected the hidden blade",
            "assassination type 11",
            "Paired xx_l_assassina_stealth_back_kill_ass_footr_a",
            "Paired xx_l_assassina_stealth_back_kill_npc_footm",
            "limp (the ragdoll)",
        ],
        never: &["combat: no clip", "EMBED"],
    },
    Scenario {
        name: "fight: the hidden blade 2.5 m behind a guard in high profile: AC1's running back assassination (type 5)",
        tags: &["fight", "ragdoll"],
        env: &[("AC1_GUARDS", "22,-45,90"), ("AC1_START", "24.5,-45,90"), ("AC1_HIGH", "0-9"), ("AC1_WEAPON", "0.5:2"), ("AC1_ATTACK", "1.5")],
        secs: 6.0,
        want: &["assassination type 5", "Paired xx_h_assassina_runback_kill_ass_footr_a", "limp (the ragdoll)"],
        never: &["combat: no clip", "EMBED"],
    },
    // User report 2026-10-13 (recording 194384): pulled down off a Damascus roof's edge at (73.6, 11.9, -56.6), down and left
    // along the house front into a free hang under a window's pointed cap (pieces 0.2-0.6 m long, its top sloping 30
    // degrees), then up: the user's build pulled up one-handed onto the cap's point and fell 7.4 m off it. Up switches onto
    // the wall under it (AC1's CanPullup), then pulls up as AC1 does (its room test leaves 0.35 m over the edge free, and
    // the balcony floor behind the cap is 0.31 m over the grip; ours turned down any top over 0.2 m), one-handed (the cap
    // drops away to either side, AC1's PullupNeedsOneHand) and standing where the two-hand pull-up does, 0.3 m in from the
    // wrists, on the floor behind the cap's point (ours stood where the one-hand clips end, on the point, and fell).
    Scenario {
        name: "Damascus: up from the free hang under a window's pointed cap pulls up onto the floor behind it, not onto its point (user report 2026-10-13)",
        tags: &["climb", "hang"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "73.61,-56.64,75.2,12.2"),
            ("AC1_HAND", "0.5"),
            ("AC1_CLIMB", "nograb,wait=1.85,downleft=0.5,left=1.2,up=6"),
        ],
        secs: 12.0,
        want: &[
            "2m -> free_1m via xx_l_climb_1m_l_hangfree",
            "free_1m -> hangwall_open via xx_h_hangfree_tr_hangwall",
            "hangknee -> top via xx_h_hangknee_onehand_footl_to_freestep_entry_footl at [73.33, 11.5",
        ],
        never: &["-> fall via", "landing_damage", "EMBED"],
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
        .env("AC1_POP_CHECK", "1")
        // (Windowed runs silent too.)
        .env("AC1_MUTE", "1");
    // (Windowed too at the fixed step: a scenario's screenshots show the very run the headless suite checks.)
    c.env("AC1_STEP", "60");
    if !windowed() {
        c.env("AC1_HEADLESS", "1");
    }
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
const TAGS: &[&str] = &["ground", "jump", "climb", "hang", "beam", "ladder", "bars", "hay", "vault", "wallrun", "ragdoll", "misc", "combo", "eject", "fight"];

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
    // (each its own process; default `HEADLESS_JOBS`, or `JOBS` windowed).
    let filter = std::env::var("AC1_SCENARIO_FILTER").unwrap_or_default();
    // `AC1_SCENARIO_TAGS=climb,hang`: only the scenarios through any of those systems (run what a change can affect).
    let tags: Vec<String> =
        std::env::var("AC1_SCENARIO_TAGS").map(|t| t.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default();
    let picked: Vec<&Scenario> = SCENARIOS
        .iter()
        .filter(|s| s.name.contains(filter.as_str()))
        .filter(|s| tags.is_empty() || s.tags.iter().any(|t| tags.iter().any(|w| w == t)))
        .collect();
    let jobs = std::env::var("AC1_SCENARIO_JOBS").ok().and_then(|j| j.parse().ok()).unwrap_or(if windowed() { JOBS } else { HEADLESS_JOBS }).max(1);
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
    let mode = if windowed() { "windowed" } else { "headless" };
    eprintln!("{} scenarios, {jobs} at a time {mode}, in {:.0} s", picked.len(), started.elapsed().as_secs_f32());
    assert!(failed.is_empty(), "{} of {} scenarios failed: {failed:?}", failed.len(), picked.len());
}
