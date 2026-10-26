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
    /// hay, vault, wallrun, ragdoll, misc, fight, crowd, masyaf (a Masyaf spot).
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
    // AC1's climb takes the release (the empty hand) mid-move in a grid move, into its lost-grip fall (0xDF90F0, Banned445's
    // port 91f7874); ours waited for the move's end.
    Scenario {
        name: "wall A: the empty hand in the middle of a climbing grid move lets go at once (AC1's lost grip)",
        tags: &["climb"],
        env: &[("AC1_START", "0,4.0,180"), ("AC1_CLIMB", "up=1.9,none=3"), ("AC1_HAND", "3.6")],
        secs: 5.5,
        want: &["let go in the middle of xx_l_climb_1ru_u_1lu", "via xx_h_hangwall_tr_fall_a", "-> land"],
        never: &["EMBED"],
    },
    // User report 1064093 (test world, the alley at x 35.3 between walls 3 m apart): the wall run's rebound with nothing to
    // land on flew its 550 cm takeoff and the flight 2.4 m and came down 0.5 m into the opposite wall, over and over. A
    // jump's takeoff and flight now move the body's capsule as AC1's proxy does (0xE0DEF0 through the controller): it
    // stops at the opposite wall.
    Scenario {
        name: "the alley at x 35.3: the wall run's rebound comes down against the opposite wall, not in it (report 1064093)",
        tags: &["wallrun", "eject"],
        env: &[("AC1_START", "35.3,-23.4,0"), ("AC1_WALK", "6.2"), ("AC1_STOP", "0.6-4"), ("AC1_HIGH", "0-4"), ("AC1_LEGS", "0-0.5"), ("AC1_JUMP", "0.8")],
        secs: 3.5,
        want: &["wall run rebound with nothing to land on (step1rebound)", "the run ends at [35.30, 0.00, -23.4"],
        never: &["EMBED", "-23.0"],
    },
    // User report 158904 (test world, x -85.5, z 51.7): the rebound flew through the wall 1.6 m behind; held back by it
    // now, it falls from there (AC1's jump held off its path by the proxy).
    Scenario {
        name: "the wall run's rebound at x -85.5 is held back by the wall behind and falls there (report 158904)",
        tags: &["wallrun", "eject"],
        env: &[("AC1_START", "-84.3,51.73,90"), ("AC1_WALK", "6.2"), ("AC1_STOP", "0.6-4"), ("AC1_HIGH", "0-4"), ("AC1_LEGS", "0-0.5"), ("AC1_JUMP", "0.9")],
        secs: 3.5,
        want: &["the jump is held back by a wall at [-84.4", "the run ends at [-84.4"],
        never: &["EMBED", "rebound off the wall in the air"],
    },
    // AC1's wall run from the lean (event 49 in the obstacle lean, 0xDA6FF0) plays the Ground's lead-in
    // `xx_h_impultionstraight_footl_to_wallingfront_step1_footr` first (Banned445's port a14bd2c, live RE/25 #49).
    Scenario {
        name: "wall W: leaning on it in high profile, the legs run up it after AC1's lead-in",
        tags: &["wallrun", "ground"],
        env: &[("AC1_START", "33,18,180,1"), ("AC1_WALK", "1.5"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "3.0-4.5")],
        secs: 4.0,
        want: &["lean on the wall", "the wall run's lead-in from standing", "-> wallrun via xx_h_wallingfront_entry_footl_b"],
        never: &["EMBED"],
    },
    // The jump off a wall is taken only where AC1 takes it (Banned445's port 91f7874, 6eec27f, e6a6eed; the exe's
    // HumanClimb event 2 guard 0xDF9230, HumanLedge 0xDCCE10 / 0xDCBF00), the press kept 0.3 s in the Legs buffer. Ours took
    // it at any point of any move on the wall, cutting it ("insta walljump at any point of an animation", the user,
    // 2026-10-13). Pressed late in the climb's entry from the ground (no grid move): kept, then taken in the stance, with
    // AC1's climbing rebound pose (`xx_h_hangwall_tr_rebound_footl`, always the left foot from a climb) and its takeoff.
    Scenario {
        name: "wall A: the legs late in the climb's entry wait for the stance, then the rebound off the left foot",
        tags: &["climb", "eject"],
        env: &[("AC1_START", "0,4.0,180"), ("AC1_CLIMB", "wait=3"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "1.15-1.2")],
        secs: 4.0,
        want: &[
            "the jump off waits: xx_h_wait_hipm_footl_tr_climbing_1m (0.6",
            "rebound pose off the left foot (0.00 m lower)",
            "back eject, 0 degrees off straight away (xx_h_rebound_frontright_front_550cm_footl_to_air)",
        ],
        never: &["EMBED", "the jump off cuts"],
    },
    // (Pressed more than 0.3 s before the entry ends: the buffer runs out, no jump off.)
    Scenario {
        name: "wall A: the legs early in the climb's entry are let go by the buffer: no jump off",
        tags: &["climb", "eject"],
        env: &[("AC1_START", "0,4.0,180"), ("AC1_CLIMB", "wait=3"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "1.0-1.05")],
        secs: 3.0,
        want: &["the jump off waits: xx_h_wait_hipm_footl_tr_climbing_1m (0.5"],
        never: &["EMBED", "jump off the wall", "back eject"],
    },
    // (In a grid move, AC1's climb takes it mid-move: the move cut, the rebound pose from where he is.)
    Scenario {
        name: "wall A: the legs in the middle of a climbing grid move jump off at once (AC1's climb, mid-move)",
        tags: &["climb", "eject"],
        env: &[("AC1_START", "0,4.0,180"), ("AC1_CLIMB", "up=1.9,none=3"), ("AC1_HIGH", "0-6"), ("AC1_LEGS", "3.6-3.65")],
        secs: 5.5,
        want: &["the jump off cuts xx_l_climb_1ru_u_1lu at 0.2", "rebound pose off the left foot", "back eject, 0 degrees off straight away"],
        never: &["EMBED", "the jump off waits"],
    },
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
    // AC1's beam speed by profile: band 2 (the crouch jog) in high profile, band 1 (the crouch walk) in low (`GetSpeedBand`
    // 0xE53090, Banned445's port); ours jogged only free running.
    Scenario {
        name: "running in high profile (no legs) along the beam to B4: the crouch jog, as AC1's high profile band",
        tags: &["beam"],
        env: &[("AC1_START", "62.0,33,-90,5"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-3")],
        secs: 2.0,
        want: &["onto perch", "along the beam at the crouch jog"],
        never: &["EMBED", "along the beam at the crouch walk"],
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
        // (A press in the wall run's first frames is held, AC1's 0.3 s Legs buffer, and ejects as the entry ends: AC1's
        // walling takes it at the end of EntryB, any time in Vertical and VerticalEnd, Banned445's `walling.rs`.)
        name: "wall W: the eject pressed in the wall run's first frames ejects as soon as the entry allows",
        tags: &["wallrun", "eject"],
        env: &[("AC1_START", "33,13,180,1"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "0-1.47"), ("AC1_JUMP", "1.5")],
        secs: 3.0,
        want: &["wallrun via xx_h_wallingfront_entry_footl_a", "wall run eject"],
        never: &["EMBED"],
    },
    Scenario {
        // (The takeoff turns the root away from the wall in one frame, the pose faded from turned with it; the pose shown
        // the frame before, which the blend's Reference keeps nearest, was left in the old root's space, and the frame
        // after the snap the body swung round, a hand and a foot 0.65 m across: the pop check fails it.)
        name: "test level: the wall run's early eject onto the wall hang beside turns away without the body swinging round",
        tags: &["wallrun", "eject"],
        env: &[
            ("AC1_START", "30.4,20.0,180"),
            ("AC1_WALK", "6.2"),
            ("AC1_HEADINGS", "0,-90;0.3,-90;0.32,90"),
            ("AC1_HIGH", "0-14"),
            ("AC1_LEGS", "0-14"),
            ("AC1_JUMP", "0.45"),
            ("AC1_CLIMB", "nograb,none=1.6,up=4"),
        ],
        secs: 2.5,
        want: &["wall run eject (entryrebound)", "the takeoff turned away at once", "-> hangwall_open via xx_hangwall_reception_front_straight_min_c"],
        never: &["EMBED"],
    },
    Scenario {
        name: "wall W: the stick swung left along the wall (by the camera, not the body) rebounds left",
        tags: &["wallrun", "climb"],
        env: &[("AC1_START", "33,13,180,1"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "0-1.6"), ("AC1_JUMP", "1.75"), ("AC1_VEER", "1.6,90")],
        secs: 4.0,
        want: &["wall run rebound with nothing to land on (step1rebound)", "back eject, 89 degrees off straight away"],
        never: &["EMBED"],
    },
    Scenario {
        name: "side grab: free running off the ledge's end, the stick turned to the wall and the empty hand held grabs it in the air",
        tags: &["jump", "hang"],
        // (AC1 reaches the stick's way only on the manual grab, the empty hand held: without it the catch looks ahead.)
        env: &[
            ("AC1_START", "-5,-50.5,-90,3"),
            ("AC1_WALK", "5.2"),
            ("AC1_HIGH", "0-4"),
            ("AC1_LEGS", "0-4"),
            ("AC1_VEER", "1.6,90"),
            ("AC1_GRASP", "1.6-3.5"),
        ],
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
    // AC1's pilotis wait (0xE51960): on a post the body keeps its facing and the wait leans toward the stick (90 degrees
    // off: three quarters of `_wait_left`); ours turned the body on the spot.
    Scenario {
        name: "standing on a post, the stick to the left: the wait leans left, the body keeps its facing",
        tags: &["beam"],
        env: &[("AC1_START", "38.6,0,-90,3"), ("AC1_WALK", "1.5"), ("AC1_VEER", "0.8,90")],
        secs: 2.5,
        want: &["xx_h_beam_pilotis_wait=0.25,xx_h_beam_pilotis_wait_left=0.75"],
        never: &["EMBED", "-> ground", "landed from", "-> jump"],
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
        // (AC1's jump query lists the ladder (type 0x1000): the surface flight onto it and its arrival on the ladder,
        // 0xE07D00's action 0x029258AA.)
        name: "a tap jump at block L's ladder from 4 m: flown at it and onto it (AC1's ladder jump target)",
        tags: &["ladder", "jump"],
        env: &[("AC1_START", "23.5,-1.4,0"), ("AC1_WALK", "3.5"), ("AC1_STOP", "0.5"), ("AC1_HIGH", "0-3"), ("AC1_JUMP", "0.4")],
        secs: 3.0,
        want: &["jump onto ladder 1", "_to_surface", "-> ladder_r via xx_h_air_surface_tr_ladder_verti_"],
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
        // (Off the foot the rung's wait ends on: which one comes down to the step reached at 4.2 s, the right since the
        // ladder's steps take their rate from the stick each frame.)
        env: &[("AC1_START", "23.5,-5.4,0"), ("AC1_CLIMB", "up=2.5,down=3"), ("AC1_HIGH", "4.0-4.6"), ("AC1_LEGS", "4.2-4.3")],
        secs: 7.0,
        want: &["rebound off the ladder", "back eject, 0 degrees off straight away (xx_h_rebound_frontright_front_550cm_foot", "land -> ground"],
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
        // (The hold over the doorway has no wall under it for the feet: AC1's arrival at a hang target with no foot
        // support is the swing reception (0xE093CC, Banned445's port), not the air catch's free hang; since 2026-10-15.)
        want: &["wall run eject (step1rebound)", "xx_h_rebound_front", "(a free hang: the swing)", "caught the free hang at [33.00, 2.99"],
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
        // (Audit NARROW-16: along the stall's axis, onto a side: the side free step, then past 0.73 of it on the counter's
        // end AC1's roll (0xE3D9A0); no jump on out.)
        name: "the tap jump at kiosk K along its axis: onto its side, its free step and the roll",
        tags: &["jump", "misc"],
        env: &[("AC1_START", "24.5,-9.6,-90"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "0.2-0.3")],
        secs: 4.0,
        want: &["side Lateral", "off the kiosk's side onto the ground, rolling on"],
        never: &["EMBED", "landing_death"],
    },
    Scenario {
        // (AC1's free-run target jump, no press: its query takes the counter's piece in high profile once 1.3 m on, and
        // comes onto it by the kiosk's move; ours bumped into it, or hopped onto it as onto any low top.)
        name: "running at kiosk K in high profile, no press: onto its counter by the kiosk's move",
        tags: &["jump", "misc"],
        env: &[("AC1_START", "29.85,-14,180"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-5")],
        secs: 4.0,
        want: &["jump onto kiosk at", "along the kiosk's counter", "off the kiosk's counter"],
        never: &["EMBED", "landing_death", "jump onto 0.74 m"],
    },
    Scenario {
        // (Free running, the legs held: AC1's box reaches 2 m on; ours' own hop went for the counter from further off.)
        name: "free running at kiosk K's side: onto it by the kiosk's move, not the hop onto a top",
        tags: &["jump", "misc"],
        env: &[("AC1_START", "24.5,-9.6,-90"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "0-5")],
        secs: 4.0,
        want: &["side Lateral", "off the kiosk's side"],
        never: &["EMBED", "landing_death", "jump onto 0.74 m"],
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
            // (Off the bar onto the beam as AC1 jumps at a target: the free-step flight and its reception, report 60396.)
            "the swing jump aimed at [-79.20, 1.50, 35.00]",
            "-> perch via xx_h_air_front_300cm_footl_to_freestep_down_tr_freestep_entry_footr at [-79.",
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
            // (Off the bar onto the post with the free-step reception, then onto it as from a post, report 60396.)
            "the swing jump aimed at [-80.50, 2.00, 84.00]",
            "-> perch via xx_h_freestep_entry_footr_tr_beam_pilotis_wait_a at [-80.50",
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
        // (Since 2026-10-13 the reach up is turned down first, by AC1's reach capsule; the table's moves by its body boxes.)
        want: &["(AC1's reach capsule)"],
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
    // The live comparison's first pad file (2026-10-15, both games played it at once, `tools/live-compare.sh`): run north
    // at the Damascus bureau's wall in high profile, the legs pressed and held once leaning on it. The running game ran
    // up it from the lean (its Ground lead-in), rebounded off the top, landed and ran up again; ours stayed leaning (the
    // lean's legs never reached it). Played through the player's own input, as a controller's.
    Scenario {
        name: "Damascus: pad file: leaning on the bureau's wall, the legs pressed in high profile run up it",
        tags: &["wallrun", "ground"],
        env: &[("AC1_LEVEL", "damascus"), ("AC1_NO_CROWD", "1"), ("AC1_PADPLAY", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/pads/bureau_wallrun.pad"))],
        secs: 5.5,
        want: &["-> lean via xx_h_collide_full_hand_150cm", "via xx_h_impultionstraight_footl_to_wallingfront_step1_footr", "rebound off it", "landed from"],
        never: &["EMBED"],
    },
    // User recording 17480 (2026-10-20, "parkour freerun jump still doesn't work"): played, the test level has its prop
    // zone, whose Damascus props brought AC1's guidance edges in, so the level counted as a city's (`Level::guided`) and
    // free running off every platform waited for an edge report and fell; scripted runs leave the props out and jumped.
    Scenario {
        name: "test level with the prop zone: pad file: free running off the 6 m platform leaps the gap and jumps down on",
        tags: &["jump", "ground"],
        env: &[("AC1_PROPS", "1"), ("AC1_PADPLAY", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/pads/props_freerun.pad"))],
        secs: 5.0,
        want: &["running jump aimed at [4.58", "running jump aimed at [14.33"],
        never: &["EMBED", "jump false, step false"],
    },
    // The eject at the inside corner's other wall: the side takeoff turns the root a quarter, so it starts turned away from
    // the wall it leaves and the reception ends facing B. (Started facing B, it caught B's hold facing A, the hands
    // stretched sideways, and the hang then squared up onto A; off the climb, turned into A, A's holds caught it 0.5 m up.)
    Scenario {
        name: "pad file: the eject off wall A's hang at the inside corner catches B's hold facing B",
        tags: &["wallrun", "hang", "eject"],
        env: &[("AC1_PADPLAY", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/pads/corner_eject.pad"))],
        secs: 5.0,
        want: &["eject at the wall hang on the hold at [-2.94, 4.19", "-> hangwall_open via xx_hangwall_reception_front_straight_min_c"],
        never: &["EMBED", "squaring up to the wall"],
    },
    Scenario {
        name: "pad file: the climb's jump by the inside corner goes up onto B's hold, not back onto A",
        tags: &["climb", "hang", "eject"],
        env: &[("AC1_PADPLAY", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/pads/corner_climb_eject.pad"))],
        secs: 6.5,
        want: &["eject at the wall hang on the hold at [-2.94, 5.39", "-> hangwall_open via xx_hangwall_reception_front_straight_min_c"],
        never: &["EMBED", "squaring up to the wall", "xx_fall_tr_climb_min_a"],
    },
    // (Off C's hang the side eject caught A's side face 0.43 m behind C's face, curving round C's corner into the gap.)
    Scenario {
        name: "pad file: the side eject off block C's hang goes along the wall, not round into the gap at A's side",
        tags: &["wallrun", "hang", "eject"],
        env: &[("AC1_PADPLAY", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/pads/gap_eject.pad"))],
        secs: 5.0,
        want: &["back eject, -89 degrees off straight away", "-> land via xx_h_landing_damage_footl"],
        never: &["EMBED", "eject at the wall hang on the hold at [3.06", "xx_hangwall_reception"],
    },
    // User recording 116252 (2026-10-22): up the test level's alley by the wall run's rebound and the climb, then back off
    // the top from the stand-up (the free-step stay's free jump, AC1's back-right free-step takeoff). Planned already
    // facing the way, the takeoff turned him a second time (-333 degrees), its 2.5 m back carried him onto the top toward
    // the wall, and he flew off facing the wall. Now the takeoff turns him itself, from the facing he had.
    Scenario {
        name: "test level alley: the free jump back off the top from the stand-up turns once and goes away from the wall",
        tags: &["jump", "wallrun", "eject"],
        env: &[
            ("AC1_PROPS", "1"),
            ("AC1_START", "30.4,20.0,180"),
            ("AC1_WALK", "6.2"),
            // (Turned back 0.5 s sooner than at first: the climb's stick cuts the reception's way out to the wait, as AC1's
            // does, and the top comes that much earlier.)
            ("AC1_HEADINGS", "0,-90;0.3,-90;0.32,90;3.37,90;3.39,-63"),
            ("AC1_HIGH", "0-6"),
            ("AC1_LEGS", "0-6"),
            ("AC1_JUMP", "0.5"),
            ("AC1_CLIMB", "nograb,none=1.6,up=2.3"),
        ],
        secs: 5.0,
        want: &[
            "free-step stay at [30.93, 3.59, 19.64]",
            "via xx_h_freestep_backright_front_550cm_footl_to_air at [30.93, 3.59, 19.64] yaw -0",
            "xx_h_air_down_800cm_footl_to_freestep_down at [31.5",
        ],
        never: &["EMBED", "_to_freestep_down at [29."],
    },
    // User recording 56678 (2026-10-23): Damascus, an inside corner by the street east of the roof, the run's jump onto
    // wall A's hang, the eject right onto wall B's climb, up a metre and the eject front-left at A's hold 1.2 m along it.
    // The reception refused a start facing along B and the fall flew him on facing away, caught A's hold side on and
    // turned 70 degrees in the catch (the old build here missed it and fell 3.5 m). Now he turns onto A over the flight.
    Scenario {
        name: "Damascus inside corner: the eject off B's climb at A's hold turns onto A in the flight, into its reception",
        tags: &["climb", "hang", "eject"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_START", "41.5,-101.5,-76,9.6"),
            ("AC1_WALK", "6.2"),
            ("AC1_HIGH", "0-8"),
            ("AC1_LEGS", "0-1.0"),
            ("AC1_STOP", "1.2-2.4"),
            ("AC1_HEADINGS", "0,16;2.39,16;2.4,90;4.79,90;4.8,-45"),
            ("AC1_CLIMB", "nograb,none=4.0,up=0.3"),
            ("AC1_JUMP", "2.6,5.6"),
        ],
        secs: 7.5,
        want: &[
            // (The first eject's flight onto B's hold on the free hang's band since 2026-10-24, AC1's 0xB1EC40, measured from
            // the hold as AC1 does, not the hang root under it: caught 0.1 m further along.)
            "-> 2m via xx_fall_tr_climb_min_b at [46.31, 9.52",
            "eject at the wall hang on the hold at [47.01, 12.0",
            // (By AC1's surface flight and its reception, a wall under the hold, since live 2026-10-23; on the surface's band
            // since 2026-10-24, its height from the hold, 1.3 m over the root: the up flight leads the mix.)
            "via xx_h_air_up_300cm_footl_to_surface",
            "-> drop via xx_h_air_surface_tr_hangwall_reception_front_straight_min",
            "-> hangwall_open via xx_hangwall_reception_front_straight_min_c at [46.57, 10.92",
        ],
        never: &["EMBED", "squaring up to the wall", "xx_fall_tr_hangwall", "xx_h_jumpfalling01"],
    },
    // User recording 142770 (2026-10-23): Damascus, on the beam over the street east of the roof, at its T with the beam
    // running north off it. Turned to face along that beam, he stayed crouched across his own; the stick held, the
    // camera following and the turns went round. Now he walks on along the crossing beam.
    Scenario {
        name: "Damascus beam T: turned at the crossing to face along the other beam, he walks on along it",
        tags: &["beam"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_START", "78.5,-94.0,-90,8.5"),
            ("AC1_WALK", "3"),
            ("AC1_HIGH", "0-2.5"),
            ("AC1_STOP", "2.5-2.8"),
            // (The turn left 0.45 s later since 2026-10-26: let go at the jog, the jog stop carries him 0.3 m on first, as AC1's.)
            ("AC1_HEADINGS", "0,0;2.79,0;2.8,180;5.05,180;5.06,-90"),
        ],
        secs: 8.0,
        want: &[
            "onto perch 261",
            "via xx_l_beam_crouchwait_footl_turn_left_to_crouchwait_90 at [84.5",
            "on along the crossing beam 262",
            "along the beam at the crouch walk",
        ],
        never: &["EMBED"],
    },
    // (The empty hand pressed in the flight unaimed it: the swing reception was never taken and he fell 2.8 m past the
    // beam, or the fall's catch took it into the knee hang mid-flight (user recording 95192).)
    Scenario {
        name: "Damascus beam 1.4 m up: the hand pressed in the jump's flight, he still catches it into the swing",
        tags: &["jump", "beam"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_START", "45.55,-55.82,1,6.5"),
            ("AC1_WALK", "4.5"),
            ("AC1_HIGH", "0-3"),
            ("AC1_JUMP", "0.3"),
            ("AC1_HAND", "0.6"),
        ],
        secs: 3.0,
        want: &[
            "jumping at the free hang at [45.51, 7.21, -59.86]",
            "reaching to catch a hold on the way down",
            "caught the free hang at [45.51, 7.21, -59.86] into the swing",
        ],
        never: &["EMBED", "landed from", "xx_fall_tr_hangknee"],
    },
    // (The plain step off's root goes 0.5 m in toward the wall and its stand 0.3 m more: he stood 0.35 m past the wall's
    // face, round the corner. Now he stands 0.35 m out from it.)
    Scenario {
        name: "pad file: the step off sideways at block B's outside corner stands clear of the wall's face",
        tags: &["climb", "ground"],
        env: &[("AC1_PADPLAY", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/pads/corner_step_off.pad"))],
        secs: 3.5,
        want: &["stepped off the wall sideways (left)", "via xx_l_climb_1m_to_groundentry_left_tr_h_wait_footl at [-2.26, 0.00, 2.37]"],
        never: &["EMBED"],
    },
    // Live comparison run3 (2026-10-16): free running at the bureau's west wall 0.15 m from its corner, 36 degrees off
    // square, the camera as the running game's was. AC1's wall run wants the wall 0.5 m to both sides (0xE149E0) and its
    // wall slide's head-on case stops the run (legacy action 87, the run stop): one stop, no wall run, no grinding on.
    Scenario {
        name: "Damascus: pad file: free running into the bureau's wall by its corner stops at it once, no wall run",
        tags: &["wallrun", "ground"],
        // (Its own pad, started on the heading the running game had (`bureau_corner.pad`'s note): from the shown yaw, 51
        // degrees off, ours turned on the spot and the camera's follow came round after the turn, bending the run 14
        // degrees off the wall's corner.)
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_PADPLAY", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/pads/bureau_corner.pad")),
            ("AC1_PADPLAY_CAM", "2.20162"),
        ],
        secs: 5.5,
        want: &["free running head-on into a wall at", "via xx_h_sprintstop_foot"],
        never: &["EMBED", "via xx_h_wallingfront_entry_footl_a", "left for running"],
    },
    // Live comparison back_eject (2026-10-23, the bureau courtyard, `courtyard_eject.pad` with the running game's camera,
    // `courtyard_eject.cam`): the wall run up the west wall catches the 8.02 m hold at AC1's grab point, 0.4 m along from
    // where the clip's hands are (the hand IK goals at z -20.41, a bracket 0.6 m along); the hang's move right onto the
    // climbing holds goes 1.0 m, AC1's lateral reach; then the eject, the stick 4.6 degrees off straight out as the
    // running game's camera had turned, flies the surface flight across the courtyard past a palm onto the facing wall's
    // hold at z -22.11 and strafes left along it. Ours caught by the nearest point, went 0.75 m right, then flew the free
    // eject straight out into the palm and fell 3.6 m.
    Scenario {
        name: "Damascus: pad file: the courtyard wall run, its catch at AC1's grab point, the climb right and the eject across",
        tags: &["wallrun", "hang", "climb", "eject"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_PADPLAY", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/pads/courtyard_eject.pad")),
            ("AC1_PADPLAY_CAM", "3.09659"),
            ("AC1_PADPLAY_MANUAL", "1"),
            ("AC1_PADPLAY_CAMLOG", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/pads/courtyard_eject.cam")),
        ],
        secs: 7.0,
        want: &[
            "wall run catch onto AC1's grab point [44.314, 8.025, -20.4",
            "1m -> 2m via xx_l_climb_1m_r_2m at [44.87, 6.78, -21.3",
            "eject at the wall hang on the hold at [49.70, 8.02, -22.0",
            // (Caught where AC1 did, z -22.11: past the palm, which pushes the flight 0.2 m aside, the body steers back
            // onto its path as the running game's did; ours caught 0.24 m along, strafed a step further than AC1 and
            // turned the inside corner at the hold's end.)
            "-> drop via xx_h_air_surface_tr_hangwall_reception_front_straight_min at [49.26, 6.92, -22.09]",
            "hangwall_open -> hangwall via xx_h_hangwall_strafe_left_050cm_close at [49.2",
        ],
        never: &["EMBED", "-> fall via", "landing_", "round the inside corner"],
    },
    // Live comparison beam_eject (2026-10-23): from the wall run's hang on the bureau's west wall, the stick up: the
    // running game pulled up onto the shallow ledge at 8.02 m (a step 0.7 m high just behind it), kneeling on its edge as
    // on a narrow object (`hangwall_tr_hangknee_footl_a`, `hangknee_footl_tr_freestep_entry_footl`), and stood there,
    // then walked into the step (`collide_full_hand_070cm`). Ours found the usual stand point inside the step and hung
    // there looking up.
    Scenario {
        name: "Damascus: pad file: up from the wall hang onto the bureau's shallow ledge with a step behind it",
        tags: &["climb", "hang"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_PADPLAY", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/pads/bureau_ledge_climb.pad")),
            ("AC1_PADPLAY_CAM", "1.57286"),
            ("AC1_PADPLAY_MANUAL", "1"),
        ],
        secs: 5.0,
        want: &[
            "top-out onto a shallow ledge, standing at its edge [44.26, 8.03",
            "-> top via xx_h_hangknee_footl_tr_freestep_entry_footl",
            "via xx_h_collide_full_hand_",
        ],
        // (Not "EMBED": the check flags four frames of the wall run's catch under the ledge's overhang, the chest 0.12 m
        // from the wall with the ledge 0.6 m over it, where the running game's root passed nearer the wall still (x 44.76
        // to 44.79, ours 44.83).)
        never: &["looking up"],
    },
    // Live comparison roof_turn2 (2026-10-16): running off a roof's edge in high profile with the stick turned 54 degrees
    // off the facing, the running game made its edge jump (`run_front_050cm`); ours wanted the stick within 45 degrees of
    // the edge's way too and fell.
    Scenario {
        name: "Damascus: pad file: running off the roof by the bureau with the stick turning right makes the edge jump",
        tags: &["jump"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_PADPLAY", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/pads/roof_turn_edge.pad")),
            ("AC1_PADPLAY_CAM", "-2.09159"),
        ],
        secs: 4.5,
        want: &["climb: the edge jump"],
        never: &["EMBED", "via xx_h_jumpstraight_clear_footall_tr_fall at [41.2"],
    },
    // User report 274414 (2026-10-15): free running off a roof onto a post and on, the free step at a roof's lip over
    // open air (no wall under it for the feet) swung on it like a bar. A ledge with a top behind it is caught into the
    // free hang; only a thin beam's side or a pole is swung from.
    Scenario {
        name: "Damascus: free stepping at a roof's lip over open air hangs free from it, not a bar swing",
        tags: &["jump", "hang", "bars"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "73.53,-85.89,-76.5,12.5"),
            ("AC1_WALK", "6.2"),
            ("AC1_HIGH", "0-5"),
            ("AC1_LEGS", "0-5"),
        ],
        secs: 5.0,
        want: &["onto perch 262", "-> free_1m via xx_fall_tr_hangfree_min_b"],
        never: &["EMBED", "into the swing", "swing -> swing"],
    },
    // User report 152348 (2026-10-15): up onto a stepped box by a wall 2.4 m over its top, free running at the wall:
    // the free-step stay jumped on into the wall, fell back onto the box and only then ran up. A wall to run up just past
    // the edge leaves the stay to the run, which runs up it.
    Scenario {
        name: "Damascus: free running off the stepped box's top at the wall beside it runs up the wall, no jump into it",
        tags: &["wallrun", "jump"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "-10.5,29.56,-23.4,16.5"),
            ("AC1_WALK", "6.2"),
            ("AC1_HIGH", "0-3"),
            ("AC1_LEGS", "0-3"),
        ],
        secs: 3.0,
        want: &["_tr_freestep_entry_footr_fast at [-7.10, 17.63", "-> hangknee via xx_h_wallingfront_entry_footl_a at [-6.", "-> top via"],
        never: &["EMBED", "the jump on from the free-step stay", "free jump off the narrow object"],
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
        want: &["xx_h_hangwall_strafe_left_050cm", "rebound off the wall at [49.", "via xx_h_rebound_right_", "_tr_freestep_entry_footl at [49.2"],
        never: &["EMBED"],
    },
    // The running game's three ejects at the 0.17 m beam by the Damascus bureau (live log 2026-10-12, 2801 s): from the
    // climb two rows up the bureau's wall (root 9.39 m), back off the wall at the beam's near hold 2.1 m away and 2.5 m
    // over the feet: AC1's jump query zone 2, a hang, no wall under it, so a free hang (target type 0x80) and its swing
    // reception, `xx_h_air_front_050cm_footl_to_swing`, `xx_h_swing_cycle_front_up`, then the still free hang at
    // (47.03, 9.46) with the legs let go. AC1 has no bar there, only the beam's two ledge grabs (grab data, item 1).
    Scenario {
        name: "Damascus: the eject off the bureau wall's climb at the beam behind is AC1's free hang target, swung from",
        tags: &["eject", "hang", "bars"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "47.6,-20,-90,5.55"),
            ("AC1_WALK", "6.2"),
            ("AC1_STOP", "1.0"),
            ("AC1_HIGH", "0-10"),
            ("AC1_LEGS", "0-1.0"),
            ("AC1_JUMP", "2.6,5.9"),
            ("AC1_CLIMB", "nograb,wait=2.4,up=1.6,wait=2.0,down=0.5"),
        ],
        secs: 9.0,
        want: &[
            "(a free hang: the swing)",
            "_to_swing",
            "caught the free hang at [47.03, 11.86",
            "via xx_h_swing_cycle_front_up",
            "hanging still",
            "the run ends at [47.0",
        ],
        never: &["EMBED", "AC1's catch search"],
    },
    // The same eject with the legs and high profile held on: a beam's side is a free hang, not a bar (user play-test
    // 2026-10-16: beams swung on like bars). The one upswing of the reception, then the still free hang; no swing on.
    Scenario {
        name: "Damascus: the eject at the bureau beam with the legs held on hangs free from it, no bar swing",
        tags: &["eject", "hang", "bars"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "47.6,-20,-90,5.55"),
            ("AC1_WALK", "6.2"),
            ("AC1_STOP", "1.0"),
            ("AC1_HIGH", "0-10"),
            ("AC1_LEGS", "0-10"),
            ("AC1_JUMP", "2.6,5.9"),
            ("AC1_CLIMB", "nograb,wait=2.4,up=1.6,wait=2.6,up=2.4"),
        ],
        secs: 9.0,
        want: &["(a free hang: the swing)", "caught the free hang at [47.03, 11.86", "hanging still"],
        never: &["EMBED", "xx_h_swing_cycle_front_down", "swinging up again", "the swing jump", "the swing comes to rest"],
    },
    // The same beam frame's lower beam (0.2 m, its holds along the wall, 1.3 m under the climb a row up): ejected back
    // along it, AC1's pick is the beam to land on (its beam contact), not a free hang: no swing (it was one of ours'
    // invented thin-beam bars before).
    Scenario {
        name: "Damascus: the eject off the bureau wall's climb onto the lower thin beam lands on it, no swing",
        tags: &["eject", "bars"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "47.6,-20,-90,5.55"),
            ("AC1_WALK", "6.2"),
            ("AC1_STOP", "1.0"),
            ("AC1_HIGH", "0-9"),
            ("AC1_LEGS", "0-1.0"),
            ("AC1_JUMP", "2.6,5.45"),
            ("AC1_CLIMB", "nograb,wait=2.4,up=1.0,wait=2.0,down=0.5"),
        ],
        secs: 8.0,
        want: &["rebound off the wall at [46.26, 7.44", "onto perch"],
        never: &["EMBED", "into the swing", "caught bar", "swing_cycle"],
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
    Scenario {
        // (The hop's landing takes event 7 again past half of it, its guard 0xF6FA50.)
        name: "gauntlet G26: jogging three beams in a zigzag, the stick right then left: a corner hop, and from its landing the next",
        tags: &["combo", "beam"],
        env: &[
            ("AC1_START", "-100,304,-90"),
            ("AC1_WALK", "6.2"),
            ("AC1_HIGH", "0-10"),
            ("AC1_LEGS", "0-10"),
            ("AC1_HEADINGS", "0,0;2.6,0;2.61,-90;3.1,-90;3.11,0"),
        ],
        secs: 4.5,
        want: &["along [0.00, 0.00, 1.00]", "the next corner hop from the hop's end", "along [1.00, 0.00, 0.00]"],
        never: &["EMBED"],
    },
    Scenario {
        // (The jump kind 2 is picked each frame before BeamState's turns; its side takeoff turns the body.)
        name: "gauntlet G27: stopped at a beam's end 1.2 m up, high profile, the stick to the side: the hop off that way, no quarter turn first",
        tags: &["beam", "jump"],
        env: &[("AC1_START", "-90.5,314,-90,2"), ("AC1_WALK", "2"), ("AC1_HIGH", "0-10"), ("AC1_STOP", "3.5-4.5"), ("AC1_HEADINGS", "0,0;4.4,0;4.41,-90")],
        secs: 6.0,
        want: &["stopped short of the beam's end", "free jump off the narrow object", "via xx_h_freestep_right_front_300cm_"],
        never: &["EMBED", "to_crouchwait_90"],
    },
    Scenario {
        // (Ledge event 10, its guard 0xDD30E0: the beam's far edge facing away within 0.45 m of the hands.)
        name: "gauntlet G28: pulled down off a beam 4 m up into a free hang, the stick back: the turn round under it onto its far side",
        tags: &["hang", "beam"],
        env: &[("AC1_START", "-87,324,-90,4.5"), ("AC1_CLIMB", "nograb,wait=1.5,drop=3,down=1")],
        secs: 7.0,
        want: &[
            "-> free_1m via xx_l_ledge_pulldown_soft_front_to_hangfree_b",
            "turned round under the beam onto its far side",
            "via xx_h_hangfree_turn_beam_180",
        ],
        never: &["EMBED", "squaring up to the wall (180"],
    },
    Scenario {
        // (NarrowObject's side entry, 0xF7AAA0: its clips' root faces along the beam, the body turned off it by the angle.)
        name: "gauntlet G29: a running jump onto a beam across the way, the stick turned along it in the flight: AC1's side entry into the jog",
        tags: &["beam", "jump"],
        env: &[("AC1_START", "-100,334,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-10"), ("AC1_LEGS", "0-10"), ("AC1_HEADINGS", "0,0;1.9,0;1.91,-90")],
        secs: 5.0,
        want: &[
            "side entry onto the beam via xx_h_freestep_entry_footl_tr_crouchjog_footl_right_90 after the arrival",
            "along the beam at the crouch jog",
            "stopped short of the beam's end at [-86.50, 2.00, 338.7",
        ],
        never: &["EMBED", "turning on the beam"],
    },
    Scenario {
        // (NarrowObject event 17, its guard 0xF7DB30: a top 0.3-1.4 m up in the way, its edge facing back; the standing
        // jump's autoclimb, then the free step's entry onto it.)
        name: "gauntlet G25: walking along a beam into a step 1 m up: AC1's beam autoclimb onto it",
        tags: &["beam"],
        env: &[("AC1_START", "-90.5,294,-90"), ("AC1_WALK", "1.9"), ("AC1_STICK", "1")],
        secs: 6.0,
        want: &["beam autoclimb onto the top 1.00 m up", "_tr_freestep_entry_foot"],
        never: &["EMBED", "stopped short of the beam's end"],
    },
    Scenario {
        // (From 3.2 m: the ladder's lowest target is 2.5 m over its foot, out of reach zone 3 from 4.6 m away.)
        name: "Damascus: a tap jump at ladder 68 from 3 m back: AC1's scorer picks the ladder (0x1000), onto it",
        tags: &["ladder", "jump"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "33.3,-16.82,-90,5.5"),
            ("AC1_WALK", "3.5"),
            ("AC1_STICK", "1"),
            ("AC1_STOP", "0.5"),
            ("AC1_HIGH", "0-3"),
            ("AC1_JUMP", "0.4"),
        ],
        secs: 3.0,
        want: &["jump onto ladder 68"],
        never: &["EMBED"],
    },
    // Audit ANIM-4 to 6: blends as AC1's move graph gives them. The running jump's takeoff is started by the code: it
    // blends in over its action's in-transition (AROLLBROLL 0.1 s); the swing cycle's items follow one another by cuts
    // where they line up.
    Scenario {
        name: "blends from AC1's move graph: the takeoff's in-transition, the swing's cuts (audit ANIM-4 to 6)",
        tags: &["bars", "jump"],
        env: &[("AC1_START", "50.3,0,-90"), ("AC1_WALK", "6"), ("AC1_HIGH", "0-10"), ("AC1_LEGS", "0-10")],
        secs: 3.0,
        // (Held through the catch, the swing jump goes off at its end, one swing (AC1's guard 0xDCD000, user play-test
        // 2026-10-16): no cycle, so no `back_down` cut; the catch hands over to the takeoff.)
        want: &[
            "blend into xx_h_run_front_300cm_footr_to_air: kind 1 0.10 s",
            "caught bar 0 at [55.00, 4.40, 0.00]",
            "blend into xx_h_swing_cycle_up_300cm_to_air: kind 2 0.20 s",
        ],
        never: &["EMBED"],
    },
    // AC1's swing jump (event 7, guard 0xDCD000) is taken at the end of the catch or of `back_down`, when high profile,
    // the legs and the stick within 75 degrees of the facing are held then: the stick let go over the catch, the legs
    // held, it swings the whole cycle and jumps at the end of `back_down`, two swings; the next bar, everything held as
    // its catch ends, in one (user play-test 2026-10-16).
    Scenario {
        name: "a swing bar with the stick let go as the catch ends swings the whole cycle, the next one held through in one",
        tags: &["bars", "jump"],
        env: &[
            ("AC1_START", "66.14,23.4,0,6.5"),
            ("AC1_WALK", "1.5"),
            ("AC1_STOP", "0.9-1.6"),
            ("AC1_JUMP", "0.5"),
            ("AC1_HIGH", "0-5"),
            ("AC1_LEGS", "0.6-5"),
        ],
        secs: 5.0,
        want: &[
            "caught bar 2 at [66.14, 8.09, 20.00]",
            "via xx_h_swing_cycle_back_down",
            "the swing jump at [66.14, 7.69, 16.60] (a bar)",
            "caught bar 3 at [66.14, 8.09, 16.60]",
            "via xx_h_air_up_300cm_footl_to_swing_tr_swing_front_a",
        ],
        never: &["hanging still", "EMBED"],
    },
    // AC1's stop (event 6, guard 0xDCCF40): the legs let go, it stops at the end of the next down item, the move graph's
    // stop for it: `back_down` into the front stop (0x023E0C61), `front_down` into the back one (Banned445's port).
    Scenario {
        name: "swing bar: the legs let go mid-swing stop it at the end of the down item, the graph's stop for it",
        tags: &["bars"],
        env: &[("AC1_START", "50.3,0,-90"), ("AC1_WALK", "6"), ("AC1_STOP", "0.7-5"), ("AC1_HIGH", "0-5"), ("AC1_LEGS", "0-2.0")],
        secs: 5.0,
        want: &["caught bar 0 at", "via xx_h_swing_cycle_back_down", "the swing comes to rest (front)", "via xx_h_swing_stop_front_d"],
        never: &["the swing jump", "EMBED"],
    },
    // User recording 124019 (2026-10-23): at rest under the bar, the stick up pulled him up onto it one-handed and stood
    // him over it. AC1's bars (its guidance sub-type 4, a pole) are nothing to pull up onto: the swing, the drop and the
    // jump off only (`under_swing_bar`).
    Scenario {
        name: "swing bar: at rest under it, the stick up does not pull up onto it (a bar is no beam)",
        tags: &["bars", "hang"],
        env: &[
            ("AC1_START", "50.3,0,-90"),
            ("AC1_WALK", "6"),
            ("AC1_STOP", "0.7-7"),
            ("AC1_HIGH", "0-7"),
            ("AC1_LEGS", "0-2.0"),
            ("AC1_CLIMB", "none=3.5,up=3"),
        ],
        secs: 7.0,
        want: &["jumping at the swing bar at [55.00, 4.40", "no top-out: hanging from a swing bar"],
        never: &["pull-up", "-> top via", "hangwaist", "EMBED"],
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
        want: &["rebound pose off the right foot (0.00 m lower)", "back eject, 89 degrees off straight away", "the run ends at [21.3", "0.00, 7.8"],
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
        want: &["rebound pose off the left foot (0.30 m lower)", "back eject, -89 degrees off straight away", "the run ends at [21.3", "0.00, 22.61] (game"],
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
    // Now he catches the ledge. (Since 2026-10-13 flown as AC1's scorer types it, a wall-hang target (0x40, the wall within
    // 0.7 m in from 0.5 m out at 1 m under it): the surface flight into the wall hang on it; ours caught it in a fall into
    // a free hang, its own wall check, from 1.2 m under, missing the wall.)
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
        secs: 4.0,
        // (Since 2026-10-14 the jump leaves at AC1's edge report, 0.45 m before the roof's edge: its grab point 1 cm on.)
        want: &["running jump at the wall's ledge [106.25, 7.77, -42.81]", "-> hangwall_open via xx_hangwall_reception_front_straight_max_c"],
        never: &["EMBED", "xx_h_jumpfalling01"],
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
        want: &["via xx_h_hangwall_tr_rebound_footr", "back eject, 45 degrees off straight away", "the run ends at [16.5", "0.00, 10.36] (game"],
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
        want: &["back eject, 45 degrees off straight away", "-> land via xx_h_landing_damage_footl_roll at [16.5", "0.00, 10.36] yaw 45"],
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
        // (The rail's line is AC1's paired one since 2026-10-14, 0.15 m further out than ours' was.)
        want: &["via xx_h_wallingfront_step1_footr_tr_hangknee_250cm_footr", "hangknee -> perch via", "at [115.9", "landed from"],
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
            // (The first corbel since 2026-10-15: its edge, 0.49 m in from the second, is AC1's hold by its climbing grid's
            // depth filter (`Awning_6x2_Wood_01A`), where ours' "buried" rule threw it away; the jump up takes it, the
            // hand steps then go on out as before.)
            "jump up from the climb to the ledge at [114.59, 11.33",
            // (How far up is measured from the free hang's hands, AC1's rows' origin, 2.4 m over the root since 2026-10-20 (it
            // was 2.1, 0.3 m under the hands at rest): the steps are 0.15, 0.13 and 0.32 m up. The steps now end on the free
            // hang's rest, so no step is followed by the rest's 0.3 m slide down.)
            "hand steps onto the edge 0.32 up",
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
        // (The first corbel, AC1's hold since 2026-10-15: see report #7's.)
        want: &["jump up from the climb to the ledge at [114.59, 11.33, -2", "-> top via xx_h_hangknee_footl_tr_freestep_entry_footl at [113.4"],
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
            // (AC1's paired railing line since 2026-10-14; from the climbing stance AC1's climb out is two-handed,
            // TryReachLedgeAbove 0xDF1730, then onto the railing as a perch.)
            "1m -> hangknee via xx_l_climb_1m_tr_hangknee_footl_a at [129.4",
            "onto perch",
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
    // wait, `xx_h_hangfree_swingback_<min|max>_a/b` (0x0564BB0E, weighed by how far out by 0xDE1FE0), then the free wait
    // `xx_h_hangfree_wait` whole (user report 608183: the wait half wall-free put the hands 0.35 m into the slab).
    Scenario {
        name: "gauntlet G24: from the wall hang, up: AC1's swing back onto the slab out from the wall, settling under it",
        tags: &["jump", "hang"],
        env: &[("AC1_START", "-92.6,266.25,-90,1"), ("AC1_HIGH", "0-2"), ("AC1_JUMP", "0.5"), ("AC1_CLIMB", "nograb,wait=3.0,up=1.0")],
        secs: 7.0,
        want: &["-> hangwall_open via xx_h_jumpstraight_footl_to_hangwall_", "swing back up to the ledge", "-> free_1m via xx_h_hangfree_swingback_max_b"],
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
            "hand steps onto the edge 0.32 up and 0.38 out at [82.14, 4.03",
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
            // (The jump query's own view of the knob since 2026-10-15, its depth filter (0.27 m out, 0.23 m under, 75
            // degrees down and 135 back up), cuts the 0.18 m piece shorter: the grab point 0.04 m along from the grid's.)
            "running jump at the wall's ledge [41.18, 9.99, -30.01]",
            "both hands onto the knob over the reception",
            // (Since 2026-10-14 the jump leaves at AC1's edge report, 0.45 m before the planks' edge: the gentler
            // reception, `_min`.)
            "drop -> hangwall via xx_hangwall_reception_front_straight_min_c at [41.15, 8.87",
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
    // (`xx_h_hangfree_swingback_<min|max>_a/b`) mixed by the same share (0xE37590 sets the wait kind 3 and the weight).
    Scenario {
        name: "gauntlet G24: free running at the wall under the slab: AC1's free catch off the wall run, settling under the edge",
        tags: &["wallrun", "hang"],
        env: &[("AC1_START", "-98,266.25,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-3"), ("AC1_LEGS", "0-3")],
        secs: 5.0,
        want: &["tr_hangfree_", "_swingback_max", "-> free_1m via xx_h_hangfree_swingback_max_b"],
        never: &["EMBED"],
    },
    // User report 608183 (2026-10-14, "Altair does an on and off thing, going from crouch hang to hang"): after the swing back
    // under G24's slab, sideways. AC1's wait kind 3 weighs the connector into the free wait (0xDE1FE0) and plays the free wait
    // `xx_h_hangfree_wait` whole; ours blended the wall-free wait into it after every shimmy step, its hands 0.35 m in the slab.
    Scenario {
        name: "gauntlet G24: the swing back under the slab, then sideways: the free hang's shimmy, no wall-free wait (user report 608183)",
        tags: &["hang"],
        env: &[("AC1_START", "-92.6,266.25,-90,1"), ("AC1_HIGH", "0-2"), ("AC1_JUMP", "0.5"), ("AC1_CLIMB", "nograb,wait=3.0,up=1.0,wait=2.5,right=3.5")],
        secs: 10.0,
        want: &[
            "swing back up to the ledge",
            "-> free_1m via xx_h_hangfree_strafe_right_050cm_close",
            "-> free_open via xx_h_hangfree_strafe_right_050cm_open",
        ],
        never: &["EMBED", "xx_h_hangwallfree_wait"],
    },
    // User report 694451 (2026-10-14, "going from crouch hang to hang and repeat"): along G24's upper row in the wall hang, the
    // wall stepped back 0.56 m under it. AC1's shimmy (StartHandStep 0xDDE0C0) keeps the hang's type; only TrySwitchHangType
    // (0xDE1060: both foot rays meet the wall here) changes it, and not on a closing step (the alternation flag +0x85). Ours
    // turned the shimmy down for no foot on the wall within 0.32 m and leapt down into a free hang and back, step by step.
    Scenario {
        name: "gauntlet G24: along the upper row in the wall hang, the wall stepped back under it: the wall hang's shimmy (user report 694451)",
        tags: &["hang"],
        env: &[("AC1_START", "-92.6,281.5,-90"), ("AC1_HIGH", "0-2"), ("AC1_JUMP", "0.5"), ("AC1_CLIMB", "nograb,wait=3.5,up=1.0,wait=2.0,right=4")],
        secs: 11.0,
        want: &["-> hangwall_open via xx_h_hangfree_tr_hangwall_right", "-> hangwall via xx_h_hangwall_strafe_right_050cm_close at [-92.60, 2.28, 283"],
        never: &["EMBED", "via xx_h_hangwall_tr_hangfree", "via xx_h_climbing_hangwall_tr_hangfree"],
    },
    // User reports 289508 / 440360 (2026-10-14, "wall run THROUGH G14"): a wall run up G14's wall under the block. AC1's
    // FindLedge (0xE36BD0) keeps a hold only with a clear line to it from 0.5 m back out of the walling root: the holds over
    // the block are hidden by it, and the run catches the row under it. Ours caught the row over it, the body in the block.
    Scenario {
        name: "gauntlet G14: a wall run under the block: caught under it, not through it (user reports 289508, 440360)",
        tags: &["wallrun", "climb"],
        env: &[("AC1_START", "-98,183.47,-90"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-3"), ("AC1_LEGS", "0-3"), ("AC1_CLIMB_WHY", "1")],
        secs: 4.0,
        want: &[
            "wall run catch at [-92.06, 4.19, 183.47]: no clear line to it",
            "-> hangwall_open via xx_h_wallingfront_step1_footr_tr_hangwall_251cm_000cm_b at [-92.50, 1.8",
        ],
        never: &["EMBED", "_430cm_000cm_b at"],
    },
    // User report 449200 (2026-10-14, "climb THROUGH G14"): hung under G14's block, up. AC1 tests its body boxes at every hang
    // and stance a move goes into: the climb from a hang (0xDD46A0 -> 0xB2DF90), the switch onto the wall (0xDE1060 -> 0xB2DC80),
    // the hand step up (0xDD8F70 -> 0xB2DD50), the ledge grab (0xDF0980). All meet the block: he stays under it.
    Scenario {
        name: "gauntlet G14: hung under the block, up: every way up meets AC1's body boxes in the block (user report 449200)",
        tags: &["hang", "climb"],
        env: &[("AC1_START", "-92.6,183.7,-90,1"), ("AC1_HIGH", "0-2"), ("AC1_JUMP", "0.5"), ("AC1_CLIMB", "nograb,wait=3,up=5"), ("AC1_CLIMB_WHY", "1")],
        secs: 9.0,
        want: &["hangfree_u_climb_1m : no room for the body there (AC1's body boxes)", "no room for the wall hang's body at"],
        never: &["EMBED", "-> 1m via", "-> hangwall_open via xx_h_hangfree_tr_hangwall", "-> top via"],
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
        want: &[
            "fight mode on",
            "attack xx_h_light_front_",
            "the strike hits",
            "Hit xx_h_light_hurt_",
            "landed xx_h_light_hit_",
            "combo hit 1 xx_h_light_combo_",
            "combo hit 2 xx_h_light_combo_",
        ],
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
        want: &["counter kill xx_h_light_counter_light_0", "against xx_h_light_countered_light_0", "limp (the ragdoll)"],
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
    // Combat round 2 (docs/COMBAT.md): AC1's turns, combo latch, deflects, counter dodge, turn-taking guards, the player's
    // death, mobile attacks, the short blade's and the fists' fights, throwing knives, the air assassination.
    Scenario {
        name: "fight: locked on a guard 90 degrees to the side, AC1's 90 degree turn on the spot (0xF4BB60)",
        tags: &["fight"],
        env: &[("AC1_GUARDS", "20,-47,0"), ("AC1_GUARD_PASSIVE", "1"), ("AC1_START", "20,-45,-90"), ("AC1_WEAPON", "0.5:3"), ("AC1_LOCK", "2.0")],
        secs: 4.5,
        want: &["fight mode on", "Turn xx_h_light_turn_90_"],
        never: &["combat: no clip", "EMBED"],
    },
    Scenario {
        name: "fight: a press in the wind-up is dropped as the strike starts (AC1's latch): no combo",
        tags: &["fight"],
        env: &[
            ("AC1_GUARDS", "22,-45,90"),
            ("AC1_GUARD_PASSIVE", "1"),
            ("AC1_GUARD_BLOCK", "0"),
            ("AC1_GUARD_HP", "5"),
            ("AC1_START", "20,-45,-90"),
            ("AC1_WEAPON", "0.5:3"),
            ("AC1_LOCK", "2.0"),
            ("AC1_ATTACK", "3.0,3.07"),
        ],
        secs: 5.0,
        want: &["attack xx_h_light_front_", "the strike hits"],
        never: &["combo hit", "combat: no clip", "EMBED"],
    },
    Scenario {
        name: "fight: a guard deflects the first attack: AC1's deflected recoil, then it strikes back at once",
        tags: &["fight"],
        env: &[
            ("AC1_GUARDS", "22,-45,90"),
            ("AC1_GUARD_DEFLECT", "1"),
            ("AC1_START", "20,-45,-90"),
            ("AC1_WEAPON", "0.5:3"),
            ("AC1_LOCK", "2.0"),
            ("AC1_ATTACK", "2.6"),
        ],
        secs: 5.0,
        want: &["the strike is deflected", "Block xx_h_light_deflect_", "Recover xx_h_light_deflected_", "the guard strikes back"],
        never: &["combat: no clip", "EMBED"],
    },
    Scenario {
        name: "fight: defending, the legs as the guard's blow comes: AC1's counter dodge, the blow misses",
        tags: &["fight"],
        env: &[
            ("AC1_GUARDS", "22,-45,90"),
            ("AC1_START", "20,-45,-90"),
            ("AC1_WEAPON", "0.5:3"),
            ("AC1_LOCK", "2.0"),
            ("AC1_HIGH", "2.5-9"),
            ("AC1_ATTACK", "dodge"),
        ],
        secs: 7.0,
        want: &["counter dodge xx_h_light_counter_dodge_back_a", "the strike misses"],
        never: &["the strike hits", "combat: no clip", "EMBED"],
    },
    Scenario {
        name: "fight: two hostile guards take turns: one attacks at a time (AC1's first memory blocks' fight manager)",
        tags: &["fight"],
        env: &[
            ("AC1_GUARDS", "22,-45,90;20,-43,180"),
            ("AC1_GUARD_AWARE", "1"),
            ("AC1_START", "20,-45,-90"),
            ("AC1_WEAPON", "0.5:3"),
            ("AC1_LOCK", "2.0"),
            ("AC1_HIGH", "2.5-14"),
        ],
        secs: 14.0,
        want: &["the guard takes the attack turn (1 attacking)", "the strike is blocked"],
        never: &["(2 attacking)", "combat: no clip", "EMBED"],
    },
    Scenario {
        name: "fight: the player hurt with little health left dies, goes limp and is desynchronised back to the start",
        tags: &["fight", "ragdoll"],
        env: &[
            ("AC1_GUARDS", "22,-45,90"),
            ("AC1_GUARD_AWARE", "1"),
            ("AC1_PLAYER_HEALTH", "0.05"),
            ("AC1_START", "20,-45,-90"),
            ("AC1_WEAPON", "0.5:3"),
            ("AC1_LOCK", "2.0"),
        ],
        secs: 9.5,
        want: &["the strike kills", "the player dies: desynchronised", "limp (the ragdoll)", "the player starts again"],
        never: &["combat: no clip", "EMBED"],
    },
    Scenario {
        name: "fight: running at a guard with the sword sheathed, the weapon hand: AC1's mobile attack drawing it as it strikes",
        tags: &["fight"],
        env: &[
            ("AC1_GUARDS", "24,-45,90"),
            ("AC1_GUARD_PASSIVE", "1"),
            ("AC1_GUARD_BLOCK", "0"),
            ("AC1_START", "14,-45,-90"),
            ("AC1_WALK", "5.2"),
            ("AC1_HIGH", "0-3"),
            ("AC1_ATTACK", "1.75"),
        ],
        secs: 4.0,
        want: &["mobile attack xx_h_mobile_unsheath_atk_dl_start_", "the strike hits"],
        never: &["combat: no clip", "EMBED"],
    },
    Scenario {
        name: "fight: a guard up the stairs, 0.7 m higher: AC1's high attack (the attack kind 3)",
        tags: &["fight"],
        env: &[
            ("AC1_GUARDS", "0,-4.1,180"),
            ("AC1_GUARD_PASSIVE", "1"),
            ("AC1_GUARD_BLOCK", "0"),
            ("AC1_START", "0,-2.2,0"),
            ("AC1_WEAPON", "0.5:3"),
            ("AC1_LOCK", "2.0"),
            ("AC1_ATTACK", "3.0"),
        ],
        secs: 5.0,
        want: &["attack xx_h_light_front_", "_atk_high_"],
        never: &["combat: no clip", "EMBED"],
    },
    Scenario {
        name: "fight: the short blade drawn (1, 1), locked on a guard: AC1's knife stance and its attack",
        tags: &["fight"],
        env: &[
            ("AC1_GUARDS", "21.5,-45,90"),
            ("AC1_GUARD_PASSIVE", "1"),
            ("AC1_GUARD_BLOCK", "0"),
            ("AC1_START", "20,-45,-90"),
            ("AC1_WEAPON", "0.3:1,0.6:1"),
            ("AC1_LOCK", "2.0"),
            ("AC1_ATTACK", "3.0"),
        ],
        // (It ends as the strike's way back starts: AC1's knife sweeps the hand 0.45 m in a frame there, its own clip's
        // motion, which the pop check would count.)
        secs: 3.3,
        want: &["drew the short blade", "fight mode on (short blade)", "attack xx_h_knife_", "the strike hits"],
        never: &["combat: no clip", "EMBED"],
    },
    Scenario {
        name: "fight: the fists (4, 4), locked on a guard: AC1's unarmed stance and punch",
        tags: &["fight"],
        env: &[
            ("AC1_GUARDS", "21.5,-45,90"),
            ("AC1_GUARD_PASSIVE", "1"),
            ("AC1_GUARD_BLOCK", "0"),
            ("AC1_START", "20,-45,-90"),
            ("AC1_WEAPON", "0.3:4,0.6:4"),
            ("AC1_LOCK", "2.0"),
            ("AC1_ATTACK", "3.0"),
        ],
        secs: 5.0,
        want: &["selected the fists", "fight mode on (fists)", "attack xx_h_unarmed_", "the strike hits"],
        never: &["combat: no clip", "EMBED"],
    },
    Scenario {
        name: "fight: the short blade at a guard 6 m off: a throwing knife (AC1's belt throw), the guard killed",
        tags: &["fight", "ragdoll"],
        env: &[
            ("AC1_GUARDS", "26,-45,90"),
            ("AC1_GUARD_PASSIVE", "1"),
            ("AC1_START", "20,-45,-90"),
            ("AC1_WEAPON", "0.3:1,0.6:1"),
            ("AC1_LOCK", "2.0"),
            ("AC1_ATTACK", "3.0"),
        ],
        secs: 7.0,
        want: &["throwing knife xx_h_knife_belt_front_long_strong_throw_", "the throwing knife kills", "limp (the ragdoll)"],
        never: &["combat: no clip", "EMBED"],
    },
    Scenario {
        name: "fight: the hidden blade on the gauntlet's 5 m block (G2) over a guard below: the air assassination, the kill on the ground",
        tags: &["fight", "ragdoll"],
        env: &[("AC1_GUARDS", "-85.5,44,90"), ("AC1_START", "-88.3,44,-90,6"), ("AC1_WEAPON", "0.3:2"), ("AC1_ATTACK", "1.0")],
        secs: 7.0,
        want: &["air assassination from", "Air xx_h_air_down_", "Paired xx_h_finish_on_ground_", "limp (the ragdoll)"],
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
            // (From the climb stance itself since 2026-10-23: the hang's move down-left onto the holds goes as far as AC1's
            // lateral probe reaches, a grid step further than its clip, `side_reach`. Into the wall hang since 2026-10-24:
            // the wall under the cap is 0.34-0.54 m behind the hold, inside AC1's ledge grab foot probe, 0xB16130; it was
            // the free hang, then the switch onto the wall.)
            "1m -> hangwall_open via xx_l_climb_1m_l_hangwall",
            "hangknee -> top via xx_h_hangknee_onehand_footl_to_freestep_entry_footl at [73.4",
        ],
        never: &["-> fall via", "landing_damage", "EMBED"],
    },
    // AC1's climb start (its guard 0xB2E860: a foot hold up to 0.9 m over the feet, the hand hold 1.2 m over it, 0.3 m either
    // way) takes a first hold 2.28 m up with a hold for the feet under it; ours' clip reaches 1.8 m and took holds 0.3 m
    // round that, and jumped at this one. (A crate in the prop zone.)
    Scenario {
        name: "the climb start takes a hold 2.3 m up with a foot hold under it (AC1's band), then over the top",
        tags: &["climb"],
        env: &[("AC1_PROPS", "1"), ("AC1_START", "28.19,-32.70,101.9,0.30"), ("AC1_CLIMB", "up=6")],
        secs: 7.0,
        want: &["the climb start onto a hold 2.28 m over the feet", "1m -> hangknee via"],
        never: &["out of reach: jump to the next hold", "EMBED"],
    },
    // User report 2026-10-13 (recording 1300517, "can't get up most borders"): pulled down off a Damascus roof's edge at
    // (44.78, 11.9, -29.4) into the free hang, onto the wall under it (`hangfree_tr_hangwall`), then up: the roof top is
    // 0.24 m over the grip and ours turned down any top over 0.2 m (AC1's room test leaves 0.35 m free, tests no floor).
    Scenario {
        name: "Damascus: from the hang under a roof's edge whose top is 0.24 m over the grip, up pulls up onto the roof (user report 1300517)",
        tags: &["hang", "climb"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "44.78,-29.6,180,12.5"),
            ("AC1_WALK", "1.5"),
            ("AC1_STOP", "0.8-60"),
            ("AC1_HAND", "1.0"),
            ("AC1_CLIMB", "nograb,wait=3,up=6"),
        ],
        secs: 9.0,
        want: &["free_open -> hangwall_open via xx_h_hangfree_tr_hangwall", "-> top via xx_h_hangknee_"],
        never: &["landing", "EMBED"],
    },
    // User reports 2026-10-13 (recordings 1319499, 13501): up the wall under a plank walkway sticking out 2.2 m over the
    // street from its top (y 11.8-12.0): AC1's vertical reach sweeps the body's capsule (0.35 m, 1.8 m tall) to the new
    // root (StartReachMove 0xDF6810, Banned445's port), which meets the planks; ours reached up through them and stood
    // with the body in them. (Whether AC1 gets up there some other way is not known: no live check yet.) Since 2026-10-14 the
    // reach from the hang is turned down first by AC1's body boxes at the stance it ends in (0xDD46A0 -> 0xB2DF90).
    Scenario {
        name: "Damascus: up the wall under a plank walkway, the reach up does not go through the planks (user reports 1319499, 13501)",
        tags: &["climb", "hang", "jump"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_CLIMB_WHY", "1"),
            ("AC1_START", "40.67,-25.3,0,8.5"),
            ("AC1_WALK", "4"),
            ("AC1_STICK", "1"),
            ("AC1_HIGH", "0-3"),
            ("AC1_LEGS", "0-3"),
            ("AC1_CLIMB", "nograb,up=10"),
        ],
        secs: 9.0,
        // (Since 2026-10-15 the awning beam under the walkway's planks, 11.37 m, is a hold (AC1's by its climbing grid's
        // depth filter; ours' "buried" rule threw it away): up from the knob's hang the free hang goes onto it, and the
        // reach up from there is the one AC1's body boxes turn down.)
        want: &[
            "-> hangwall via xx_hangwall_reception_front_straight_",
            "-> free_1m via xx_h_hangwall_tr_hangfree_up_left",
            "xx_h_hangfree_u_climb_1m : no room for the body there (AC1's body boxes)",
        ],
        never: &["leap -> 1m via xx_h_climbing_hangwall_tr_climb1m_up_l_hand_2_c", "EMBED"],
    },
    // User report 2026-10-13 (recording 1310993, "grabbing on stuff that doesn't look grabbable"): a free run off a Damascus
    // roof at the roof edge across the street (11.77 m, sticking 0.45 m out over the facade): AC1's scorer's wall-hang
    // target (0x40), flown into the wall hang; ours' own check for a wall under the feet fell short of the facade, the jump
    // flew the plain arc, met the wall 0.4 m under the edge and fell 2 m onto a window's frame (AC1's own holds).
    Scenario {
        name: "Damascus: a running jump at the roof edge across the street receives into the wall hang on it (user report 1310993)",
        tags: &["jump", "hang"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "40.45,-32.5,180,12.6"),
            ("AC1_WALK", "6.2"),
            ("AC1_HIGH", "0-5"),
            ("AC1_LEGS", "0-5"),
        ],
        secs: 4.0,
        want: &["running jump at the wall's ledge [40.45, 11.77, -24.75]", "drop -> hangwall_open via xx_hangwall_reception_front_straight_max_c"],
        never: &["xx_h_jumpfalling01", "fall_tr_climb", "EMBED"],
    },
    // User report 2026-10-13 (recording 62377, "hands grab air and then adjust"): up a Damascus wall to below a roof's
    // edge, the jump up from the climb onto the edge (`xx_h_climbing_1m_impultion_to_swingback_*`) and its swing back:
    // the chain ends in the free hang's wait, its wrists 2.3 m over the root, so AC1's free hang root 2.4 m under the
    // hold; ours put it 2.1 m under (the catch's), the hands 0.2 m over the edge for the swing back's 2 s.
    Scenario {
        name: "Damascus: the jump up from the climb onto a roof's edge hangs 2.4 m under it, the hands on the edge (user report 62377)",
        tags: &["climb", "hang"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "46.3,-25.0,0,4.5"),
            ("AC1_WALK", "6.2"),
            ("AC1_HIGH", "0-1.5"),
            ("AC1_LEGS", "0-1.5"),
            ("AC1_STOP", "1.5"),
            ("AC1_CLIMB", "nograb,wait=2.5,up=12"),
        ],
        secs: 11.0,
        want: &[
            "jump up from the climb to the ledge at [46.33, 11.77",
            "free_1m -> free_1m via xx_h_hangfree_swingback_max_a at [46.33, 9.37",
            "-> top via xx_h_hangknee_footl_tr_freestep_entry_footl",
        ],
        never: &["EMBED"],
    },
    // User report 2026-10-15 (recording 152477, "the inverted triangles"): a Damascus pavilion at (-45.3, 45.4), its
    // arches 12.15 m up with a knob over each (12.45) and the roof slab's edge 12.93 m up, 0.58 m out over them. Let go
    // off the slab into the free hang on the arch, up: AC1's vertical rows step a hand onto the knob, then the hand step
    // onto the slab's edge 0.45 m out, and over it. Ours' step onto the edge kept the body's depth, the hands 0.45 m short of
    // the edge, in the slab, and the rest eased the root back down under the knob: up and down for ever. Now the hands go
    // onto the edge (within AC1's rows' 0.5 m) and the body under it. And the free hang's rest under a knob, probed from the
    // sinking root, slid on down onto the arch under it (the same loop on the next arch).
    Scenario {
        name: "Damascus pavilion: from the free hang on an arch, up onto its knob, the roof slab's edge and over, no hand stepping up and down (user report 152477)",
        tags: &["hang", "climb"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "-45.3,-45.4,90,13.5"),
            ("AC1_WALK", "1.0"),
            ("AC1_STOP", "0.4-30"),
            ("AC1_HAND", "0.6,3.0"),
            ("AC1_GRASP", "3.1-5"),
            ("AC1_CLIMB", "nograb,wait=5.5,up=8"),
        ],
        secs: 13.0,
        want: &[
            "leap -> free_1m via xx_fall_tr_hangfree_min_b at [-45.29, 9.75",
            "hand steps onto the edge 0.30 up and 0.04 out at [-45.28, 12.45",
            "free_1ru -> free_1m via xx_h_hangfree_climb_1ru_u_1m at [-45.55",
            "-> top via xx_h_hangknee_footl_tr_freestep_entry_footl at [-45.56, 12.93",
        ],
        never: &["EMBED"],
    },
    // Played by hand (a pad file, 2026-10-20): the same let go off the slab's free hang, the empty hand pressed again in the
    // fall (as a player catches: pressed and held, not held from before). The catch takes holds 0.45 m under the one let go
    // of, which was taken as a wall hang's (1.13 m over the root, not the free hang's 2.4): the knob and the arch 0.4-0.8 m
    // under the slab's edge were turned down and he fell 4 m to the floor. Now the knob under the slab is caught.
    Scenario {
        name: "Damascus pavilion: let go of the roof slab's free hang, the empty hand pressed again in the fall catches the knob under it (pad play 2026-10-20)",
        tags: &["hang", "climb"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "-45.3,-45.4,90,13.5"),
            ("AC1_WALK", "1.0"),
            ("AC1_STOP", "0.4-30"),
            ("AC1_HAND", "0.6,3.0,3.1"),
            ("AC1_GRASP", "3.1-5"),
            ("AC1_CLIMB", "nograb,wait=5.5,up=8"),
        ],
        secs: 13.0,
        want: &[
            "reaching to catch a hold on the way down",
            "leap -> free_1m via xx_fall_tr_hangfree_min_b",
            "-> top via xx_h_hangknee_footl_tr_freestep_entry_footl",
        ],
        never: &["landing_straight_hard", "EMBED"],
    },
    // User report 2026-10-15 (recording 161446): the same pavilion, from the knob left: AC1's switch onto the wall under the
    // next knob (`hangfree_tr_hangwall_left`), then up he hung looking up. AC1's hand step keeps the hang's type
    // (StartHandStep 0xDDE0C0; only TrySwitchHangType changes it, and both its foot rays find the arch there): up onto the
    // slab's edge 0.6 m out over the arch and over it onto the roof. Ours wanted each foot on a wall within 0.32 m of the
    // new hold.
    Scenario {
        name: "Damascus pavilion: from the knob left into the wall hang, up onto the roof slab's edge and over (user report 161446)",
        tags: &["hang", "climb"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "-45.3,-45.4,90,13.5"),
            ("AC1_WALK", "1.0"),
            ("AC1_STOP", "0.4-30"),
            ("AC1_HAND", "0.6,3.0"),
            ("AC1_GRASP", "3.1-5"),
            ("AC1_CLIMB", "nograb,wait=5.5,up=0.5,wait=1.5,left=0.5,wait=1.5,up=5"),
        ],
        secs: 13.0,
        want: &[
            "free_1m -> hangwall_open via xx_h_hangfree_tr_hangwall_left at [-45.33, 10.05",
            // (The slab's wall is set back under its edge: the hang leans, its root 1.1 m down the body's axis and 0.5 m
            // out square to it, 0.08 m lower than upright (AC1's sub_B157B0, Banned445's 9390e4e).)
            "hangwall -> hangwall_1ru via xx_h_hangwall_climb_1m_u_1ru at [-45.69, 11.1",
            "-> top via xx_h_hangknee_footl_tr_freestep_entry_footl at [-45.6",
        ],
        never: &["EMBED"],
    },
    // User report 2026-10-15 (recording 116822): under a Damascus awning's frame at (24.5, 10.9) (beams 0.9-1.4 m out from
    // the wall under the planks), the free hang's drop onto the wall below, then up: a hand stepped onto a knob 11.25 m up
    // and on to the planks' edge, and the second hand never followed (no room for the wall hang's body under the planks'
    // beam), the body held in the half step. AC1's FindVerticalRows tests the wall hang's box at the step's row before the
    // step starts (0xDD8F70 -> 0xB2DC80): no step there, the look up.
    Scenario {
        name: "Damascus awning frame: the drop onto the wall under it, up: no hand step half done under the beams (user report 116822)",
        tags: &["hang", "climb"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "26.9,-12.5,-135,12"),
            ("AC1_WALK", "1.0"),
            ("AC1_STOP", "1.2-30"),
            ("AC1_HAND", "1.6"),
            ("AC1_CLIMB", "nograb,wait=4,left=3.3,wait=0.6,right=0.5,wait=1,down=0.5,wait=1.5,up=4"),
        ],
        secs: 16.0,
        want: &["leap -> hangwall_open via xx_h_climbing_hangfree_tr_hangwall_down_max_200_c at [24.42, 9.55", "looking up"],
        never: &["-> hangwall_1lu", "-> hangwall_1ru", "EMBED"],
    },
    // User report 2026-10-15 (recording 265066): off a Damascus roof, the fall's catch on a chimney's top (54.2, 13.1,
    // -84.8) into the wall hang, its roof 0.35 m under the root; the empty hand let go (a press: ours has no let-go of its
    // own) and the release carried the root 1 m down with no floor test, through the roof, and he fell 6 m. A floor within
    // the release's drop: the fall from where he hangs, onto the roof. (Up beside the chimney in AC1's Ghost mode, then the
    // fall with the hand held.)
    Scenario {
        name: "Damascus chimney: the fall's catch on its top over the roof, the empty hand lets go onto the roof, not through it (user report 265066)",
        tags: &["hang", "climb"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "53.95,-84.81,-90,12.5"),
            ("AC1_FLY", "0.2-1.0"),
            ("AC1_FLY_UP", "0.2-1.0"),
            ("AC1_GRASP", "0.2-3"),
            ("AC1_HAND", "4.0"),
        ],
        secs: 6.0,
        want: &[
            "leap -> hangwall_open via xx_fall_tr_hangwall_straight_max_b at [54.15, 12.10",
            "let go with a floor within the release's",
            "fall -> land via xx_h_landing_straight_soft_footr_tr_h_wait_footr_a at [54.19, 11.76",
        ],
        never: &["landing_damage", "EMBED"],
    },
    // User report 2026-10-13 (recording 85785, "the triangle": a Damascus window box's frame, holds at 8.7, 9.5, a knob at
    // 10.1 and 10.7 m): up onto it and down off it again. (The stance's hands now go onto AC1's goals, the grid cell's
    // holds, up to 0.5 m off where the clip holds them: ours took a hold only within 0.2 m and gripped the air.) (Up for 2 s:
    // since the wall run's catch ends on the wall hang's own root, 0.09 m higher, it climbs on without a hand step first.)
    Scenario {
        name: "Damascus: up onto a window box's frame and back down off it (user report 85785)",
        tags: &["climb", "hang"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_NO_CROWD", "1"),
            // (From 0.2 m further west since 2026-10-23: the wall run's catch takes AC1's grab point (`wall_run_grab`), its
            // hands 0.5 m in from the 8.28 m hold's west end, where it caught 0.11 m from it.)
            ("AC1_START", "42.4,-25.0,0,4.5"),
            ("AC1_WALK", "6.2"),
            ("AC1_HIGH", "0-1.5"),
            ("AC1_LEGS", "0-1.5"),
            ("AC1_STOP", "1.5"),
            ("AC1_CLIMB", "nograb,wait=2.5,up=2.0,wait=2,down=3"),
        ],
        secs: 11.0,
        // (Down: the frame's 0.13 m sloped piece at 8.28 m is cut to 0.1 m by AC1's depth filter since 2026-10-15 (kept in
        // part), its foot hold 0.05 m further in; the way down goes by the hold at 7.20 m into the same stance at the
        // bottom, where it went by 8.28 m.)
        want: &["1lu -> 1m via xx_l_climb_1lu_u_1m at [42.54, 8.86", "1ru -> 1m via xx_l_climb_1ru_d_1m at [42.45, 6.97"],
        never: &["EMBED"],
    },
    // User reports 2026-10-15 (recordings 102743, 116822, "the inverted triangles"): the awning frame over the street by
    // (24, 11.5, -11) (`Awning_6x2_Wood_01A`), three beams under its planks, each cut where a diagonal brace crosses it.
    // Its holds are AC1's ledge grabs by AC1's own rules (`Level::ac1_holds`: its climbing grid's depth filter, 0xB27E80):
    // the third beam, 0.32 m behind the second, is a hold (ours' own "buried" rule threw it away); none on the braces.
    Scenario {
        name: "Damascus awning frame: its holds are AC1's, the three beams under it cut at the braces (user report 2026-10-15)",
        tags: &["hang"],
        env: &[("AC1_LEVEL", "damascus"), ("AC1_NO_CROWD", "1"), ("AC1_START", "24.5,-9.0,0,6"), ("AC1_HOLDS_AT", "23.75,11.45,-11.0,0.75")],
        secs: 0.5,
        want: &[
            "[24.82, 11.59, -10.47] to [23.94, 11.59, -10.47], out [0.00, 0.00, 1.00]",
            "[24.99, 11.47, -10.93] to [23.94, 11.47, -10.93], out [0.00, 0.00, 1.00]",
            "[25.21, 11.32, -11.41] to [23.94, 11.32, -11.41], out [0.00, 0.00, 1.00]",
            "[23.56, 11.32, -11.41] to [21.92, 11.32, -11.41], out [0.00, 0.00, 1.00]",
        ],
        never: &["ac1::level: perch ", "ac1::level: bar "],
    },
    // City life (`city_life`): Damascus peopled from AC1's crowd data, 12 at most (scripted runs load and build in step,
    // so each run is the same).
    Scenario {
        // (Walking into someone is no bump in AC1: only its mover classes from a run in high profile up send the collision
        // request, `city_life::bump`.)
        name: "city life: a Damascus street peopled from AC1's crowd regions and flows; walking into passers-by bumps no one",
        tags: &["crowd"],
        env: &[("AC1_LEVEL", "damascus"), ("AC1_CITY_LIFE", "1"), ("AC1_CROWD_MAX", "12"), ("AC1_START", "44,-50,0,6"), ("AC1_WALK", "1.5")],
        secs: 10.0,
        want: &["city life: 29 regions, 16303 region triangles, 451 flows", "city life: 12 people near (12 wanted"],
        never: &["city life: no crowd data", "city life: bump ("],
    },
    Scenario {
        name: "city life: running into the crowd, Altaïr shoves through and they stumble (AC1's hurt type 0 from Fast)",
        tags: &["crowd"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_CITY_LIFE", "1"),
            ("AC1_CROWD_MAX", "12"),
            ("AC1_START", "44,-50,0,6"),
            ("AC1_WALK", "3.5"),
            ("AC1_HIGH", "0-10"),
        ],
        secs: 10.0,
        want: &["bump (run, 3.5 m/s) MCxx_Poor plays xx_stumble_strong_"],
        never: &["city life: no crowd data"],
    },
    Scenario {
        name: "city life: free running into the crowd, a passer-by knocked down (AC1's hurt type 7 from SuperFast)",
        tags: &["crowd"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_CITY_LIFE", "1"),
            ("AC1_CROWD_MAX", "12"),
            ("AC1_START", "44,-50,0,6"),
            ("AC1_WALK", "6"),
            ("AC1_HIGH", "0-10"),
            ("AC1_LEGS", "0-10"),
        ],
        secs: 10.0,
        want: &["bump (sprint, ", "m/s) MCxx_Poor plays xx_fall_straight_", "then [\"xx_getup_slow_face"],
        never: &["city life: no crowd data"],
    },
    Scenario {
        name: "city life: the street's militia (AC1's militia fraction, combat's guards) fights back when attacked",
        tags: &["crowd", "fight"],
        env: &[
            ("AC1_LEVEL", "damascus"),
            ("AC1_CITY_LIFE", "1"),
            ("AC1_CROWD_MAX", "12"),
            ("AC1_START", "44,-50,0,6"),
            ("AC1_WALK", "1.5"),
            ("AC1_STOP", "7"),
            ("AC1_LOCK", "8"),
            ("AC1_ATTACK", "9:0.6,10.5:0.6,12:0.6"),
        ],
        secs: 16.0,
        want: &["city life: militia (MMMA_Damascus_Militia) in Poor Street", "combat: guard turns hostile", "combat: the strike hits"],
        never: &["city life: no crowd data"],
    },
    // User report 60396 (2026-10-13): off the second swing bar the jump at B6 came down 0.5 m short, on its near edge, and
    // stepped back off it 4.7 m. AC1 flies the swing jump at its target and lands it in the free-step reception.
    Scenario {
        name: "rooftops: off the second swing bar onto B6 with the reception, not stepping back off its near edge (report 60396)",
        tags: &["bars", "jump"],
        // (Over by 6 s since the bars are swung through in one swing each: held on to 8 s, it ran on off B6's far end.)
        env: &[("AC1_START", "66.14,35,0,6"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-6"), ("AC1_LEGS", "0-6")],
        secs: 6.0,
        want: &["the swing jump aimed at [66.14, 4.69, 13.01]", "-> ground via xx_h_air_front_300cm_footl_to_freestep_down_tr_freestep_entry_footr"],
        never: &["came down on an edge", "landing_damage", "EMBED"],
    },
    // User report 46392: running off B4 toward B5's north-west corner, 1.2 m up and 4 m on (outside AC1's reach zone 1,
    // so no target): AC1's free jump flies the stick's way (Banned445's port 1753486, live), its feet meet the corner,
    // and the edge landing steps off over B5's west edge the way he was going, facing the drop, as AC1's does; never
    // back over the edge he came from.
    Scenario {
        name: "rooftops: B4 at B5's corner out of reach: the free jump, the edge landing over the far edge, not back (report 46392)",
        tags: &["jump"],
        env: &[("AC1_START", "68.9,31.5,50,5"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-3"), ("AC1_LEGS", "0-3")],
        secs: 2.5,
        want: &["came down on an edge at [63.9", "step off it (back)"],
        never: &["step off it (front)", "EMBED"],
    },
    // User report 423408: a wall run's eject back at a hold on the wall behind flew at the hold's hang, and the fall's catch
    // took the hold 0.6 m higher, into the climb's crouched catch. AC1 lands its eject at a hold in the wall-hang reception
    // on that hold (audit WALL-10: `air_front_050cm_footr_to_swing`, `hangwall_reception_front_straight_min_a`).
    Scenario {
        name: "wall run eject back at a hold: AC1's wall-hang reception on the hold aimed at, not the climb's catch on another (report 423408)",
        tags: &["wallrun", "hang", "jump"],
        env: &[("AC1_START", "9.08,11,0"), ("AC1_WALK", "6.2"), ("AC1_HIGH", "0-3"), ("AC1_LEGS", "0-0.8"), ("AC1_JUMP", "0.85")],
        secs: 3.0,
        want: &["eject at the wall hang on the hold at [9.08, 3.00, 11.94]", "-> hangwall_open via xx_hangwall_reception_front_straight_min_c"],
        never: &["xx_fall_tr_climb_min", "EMBED"],
    },
    // Masyaf (2026-10-13): its two leaps of faith. The siege's (M05_S17_Trigger_LOFJump): off the scaffold on the tower over
    // the fortress gate (`Scaffold_Leap1`), 44 m down into `Hay_Bale_01 Group_001` 5.9 m out; the tutorial tower's
    // (`M05_T40_Scene_TUT_ReachHighPoint`, `Maysaf_TowerTrap`): off its top's north side into `Group_009` 13 m out, 13.4 m
    // down. AC1's search casts past the haystack's own collision (the heaps stand 2.1 m, over the 1.85 m its way ends at),
    // blends its four takeoffs and dives by the way, and flies the dive to the hay; ours found neither (a plain running
    // jump: an 85 m fall past the siege hay), then dived 9 m short of the tower's, then died on the heap's top.
    Scenario {
        name: "Masyaf: the siege's leap of faith off the fortress tower's scaffold, 44 m down into its hay",
        tags: &["hay", "jump", "masyaf"],
        env: &[
            ("AC1_LEVEL", "masyaf"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "196.2,67,170,124"),
            ("AC1_WALK", "6.2"),
            ("AC1_HIGH", "0-2.5"),
            ("AC1_LEGS", "0-2.5"),
            ("AC1_STOP", "2.5"),
        ],
        secs: 7.0,
        want: &["-> faith via xx_h_freestep_footr_to_faith_jump_800cm_long_3000cm_down at [194.", "-> hay via xx_h_faith_jump_landing at [194."],
        never: &["landed from", "dead via", "EMBED"],
    },
    Scenario {
        name: "Masyaf: the leap of faith off the tutorial tower's top into its hay 13 m out",
        tags: &["hay", "jump", "masyaf"],
        env: &[
            ("AC1_LEVEL", "masyaf"),
            ("AC1_NO_CROWD", "1"),
            ("AC1_START", "160.3,87.5,-1.4,105"),
            ("AC1_WALK", "6.2"),
            ("AC1_HIGH", "0-1.5"),
            ("AC1_LEGS", "0-1.5"),
            ("AC1_STOP", "1.5"),
        ],
        secs: 5.0,
        want: &["-> faith via xx_h_freestep_footr_to_faith_jump_", "-> hay via xx_h_faith_jump_landing at [160."],
        never: &["landed from", "dead via", "EMBED"],
    },
    // Masyaf's climbing sweep (2026-10-13), wall w18 in the village below the fortress: up the wall, into the free hang
    // under its top, over it onto the roof 4.1 m up.
    Scenario {
        name: "Masyaf village: up a house wall, the free hang under its top and over onto the roof",
        tags: &["climb", "masyaf"],
        env: &[("AC1_LEVEL", "masyaf"), ("AC1_NO_CROWD", "1"), ("AC1_START", "78.20,85.12,5.1,71.50"), ("AC1_CLIMB", "up=11")],
        secs: 12.0,
        want: &["1m -> free_1m via xx_l_climb_1m_u_hangfree", "hangknee -> top via xx_h_hangknee_footl_tr_freestep_entry_footl at [78.1"],
        never: &["EMBED", "letgo"],
    },
    // The online friend in the walls (user report 2026-10-13, replay 1791930058, `AC1_REPLAY_SCAN`): ours' own beams, from
    // mesh names and single-hold tops, made a Damascus drum's plinth (0.3 m deep against the drum's wall) and a window box's
    // sill into beams, and balancing on them the body stood half in the wall (the scan: the capsule 0.35 and 0.45 m into
    // it). AC1's beams are its paired hold edges, cut where anything stands in their workspace, and its posts are open
    // over them (`sub_B2B600`): none there.
    Scenario {
        name: "Damascus: no beam on a drum's plinth against its wall (the online friend inside the wall, user report 2026-10-13)",
        tags: &["beam"],
        env: &[("AC1_LEVEL", "damascus"), ("AC1_NO_CROWD", "1"), ("AC1_HOLDS_AT", "10.4,12.44,-112.2,0.6")],
        secs: 0.5,
        want: &[],
        never: &["perch "],
    },
    Scenario {
        name: "Damascus: no beam in a window box's sill (us inside the box in the same replay)",
        tags: &["beam"],
        env: &[("AC1_LEVEL", "damascus"), ("AC1_NO_CROWD", "1"), ("AC1_HOLDS_AT", "69.12,8.91,-43.61,0.5")],
        secs: 0.5,
        want: &[],
        never: &["perch "],
    },
    // The same plinth met from the roof below it: leaning on it and pushing on, ours climbed onto it and balanced on it
    // in the wall. Its top has no room for the body (the capsule against the drum): no climb, as AC1's jump at the edge
    // from the lean needs a top to stand on.
    Scenario {
        name: "Damascus: pushing on into a drum's plinth from the lean does not climb onto it (no room for the body there)",
        tags: &["beam", "ground"],
        env: &[("AC1_LEVEL", "damascus"), ("AC1_NO_CROWD", "1"), ("AC1_START", "10.0,-112.4,120,12.8"), ("AC1_WALK", "1.5")],
        secs: 4.0,
        want: &["lean on the wall at"],
        never: &["EMBED", "from leaning on it", "onto perch"],
    },
    // Audit route roof2_run_south (the running game, Damascus): in high profile to a roof's end that ours took for a beam
    // over the crack to the next roof 0.74 m down; AC1 runs on over it as ground and steps off it (`run_front_050cm`).
    // Ours balanced on its own beam there and free-stepped off it. AC1's beams (paired hold edges) have none there.
    Scenario {
        name: "Damascus: running in high profile off a roof's end over a crack runs on as ground and steps off (route roof2_run_south)",
        tags: &["beam", "ground"],
        env: &[("AC1_LEVEL", "damascus"), ("AC1_NO_CROWD", "1"), ("AC1_START", "41,-20,180,12.5"), ("AC1_WALK", "5.2"), ("AC1_HIGH", "0-5")],
        secs: 4.0,
        want: &["step off the edge", "landed from 0.7 m"],
        never: &["onto perch", "EMBED"],
    },
    // The level editor (`editor`, `level_file`): a saved level on empty ground, wall A as a box with hold rows up its
    // faces (`tests/levels/climb_block.level`, read from the crate's folder, the tests' working directory), climbed to
    // its top as wall A is.
    Scenario {
        name: "a saved editor level: its box with holds is climbed to the top as wall A",
        tags: &["climb", "misc"],
        env: &[("AC1_LEVEL", "tests/levels/climb_block.level"), ("AC1_START", "0,4.0,180"), ("AC1_CLIMB", "up=14"), ("AC1_WALK", "1.9")],
        secs: 8.0,
        want: &["editor level tests/levels/climb_block.level: 3 pieces on the empty base", "-> top via xx_h_hangknee_footl_tr_freestep_entry_footl"],
        never: &["EMBED", "editor level: "],
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
const TAGS: &[&str] =
    &["ground", "jump", "climb", "hang", "beam", "ladder", "bars", "hay", "vault", "wallrun", "ragdoll", "misc", "combo", "eject", "fight", "crowd", "masyaf"];

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
    // `AC1_SCENARIO_FILTER`: only the scenarios whose name contains it (or any of its `|`-separated parts: a run's failures
    // again). `AC1_SCENARIO_JOBS`: how many games run at once
    // (each its own process; default `HEADLESS_JOBS`, or `JOBS` windowed).
    let filter = std::env::var("AC1_SCENARIO_FILTER").unwrap_or_default();
    // `AC1_SCENARIO_TAGS=climb,hang`: only the scenarios through any of those systems (run what a change can affect).
    let tags: Vec<String> =
        std::env::var("AC1_SCENARIO_TAGS").map(|t| t.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default();
    let picked: Vec<&Scenario> = SCENARIOS
        .iter()
        .filter(|s| filter.split('|').any(|f| s.name.contains(f)))
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
