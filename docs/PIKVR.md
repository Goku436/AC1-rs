# PIKVR: parkour, IK, visuals, research

A test campaign on top of the scenarios (`crates/game/tests/scenarios.rs`). Started 2026-10-07 (P, I and R; V
later). Each pass writes its findings to the run log at the end; fixes go in separately, and every bug found gets a
scenario so it stays fixed.

Rules learned on the way here:
- A scripted run must be checked to actually do the move (the log names the clip) before its numbers mean anything:
  a "climb" from 1.2 m off the wall jumped and fell instead, and measured nothing.
- Compare with IK off (`AC1_NO_IK=1`) to tell the clip's own motion from the IK's.
- Recordings (`AC1_RECORD_AT`, F9) give every bone per frame; the checks below read them.
- Check fixes at more than one frame rate: a fixed per-frame margin pinned the beam walk at high frame rates.
- Keep files' line endings (character.rs, climb.rs and NOTES.md are CRLF in git; scripts that rewrite them must too).

## Backlog, in order

### 1. Small bugs and tasks
- [x] High profile idle (fixed: the low and high stands now fade over 0.3 s; the whole body jumped up to 13 cm): holding the right button standing, after having moved a while, the pose snaps into the high
      profile stand (recording `ac1-recording-339883`, probably). Find the missing fade between the stand loops.
- [x] Slow walk speed (set to the clip's 0.1 m/s): `gait.rs` used 1.0 m/s for a value of 0 (half stick); Altaïr's `xx_l_walk_slow_hipm_footl` moves
      0.17 m in 1.67 s (0.1 m/s). Measure the slow walk's real use (AC1 may stand it still and blend), then set it.
- [ ] Speed restart (partly: AC1's running jump hands the run's speed back on landing) (`GAIT_RESYNC`, ours, not AC1's): after a landing the speed value restarts from the jog. AC1 keeps
      the take-off speed through a landing (Banned445's notes); keep it unless the landing is a heavy one.
- [ ] (Needs a play test, feel) Run turn rate: ours 9 rad/s, AC1's 360 deg/s (2 pi). Try AC1's and the high profile turn hold back together.
- [ ] (Needs a play test, feel) Walk stop: AC1 stops a walk-band move at once (to the wait with a 0.2 s blend); ours eases the velocity down.
- [ ] Hands catch up fast as they leave a hold (fade window 6-20 cm): widen if it reads as a snap in play.
- [x] (AC1's up jump now: run_up/air_up 300 cm, quick) B4 to B5 jump: 0.88 s in the air (a 1.2 m rise needs it); offered: catch the edge and pull up instead.
- [x] (Was the start facing away; 20 degrees off works, scenario) Two-step lane from an angle: a scripted start at (38.5, -28.6) yaw -115 did nothing (check the start spot).
- [ ] Capsule: no ceiling handling, no stick-to-ground cast (0.58 m), no fall-off-support rule (rim past 45 degrees);
      the air, climb and fall states don't use it. Bring in what play shows is missing.
- [ ] Scabbard angle and the hood (lead: in Banned445's repo the hood follows the head through Altaïr's authored
      skeleton modifiers, a secondary hood look-at (aiming its X/Z axes) and hinges, and the sword tag has angular soft
      limits: `src/skirt_hinge.rs`, `src/visual_pose.rs`, commit `caab683`; his robe also runs on the authored skirt
      hinge chain, look-at and compression, worth the same port for our robe): read Banned445's port of the authored hood and sword-tag modifiers (his commits
      `caab683`, `826e97e`), then read AC1's modifiers here (the face pokes through the hood when the head turns while
      climbing: `out/shots/hood-clip-climb.png`).
- [ ] Haystack in Damascus with black squares (V, later).

### 2. AC1's own data and code (decomp, move graph)
The move graph is the big one: with it the game itself says which move follows which.
- [~] **AC1's jump system** (first step done 2026-10-07: running jumps onto tops play AC1's takeoff, flight and
      reception blends, `jump.rs`; then the vault, jumps onto posts and beams, at holds and with no target: no scenario plays a step off AC1's move
      graph any more; still to do: the five side directions, free-step takeoffs from beams and posts,
      flights to passover/surface/swing/assassinate, holds (`hangwall_reception_*`), posts and beams as targets, the
      ground landings by stick and speed) (found 2026-10-07, see the run log): our running jump takes off with
      `xx_h_sprint_impultion_footl`, which is a stride of the locomotion blend (`HumanGround` 0x5923bdb), not a jump.
      AC1 plays a takeoff blend (40 clips: `xx_h_run_<front|down|up>_<050|300|550>cm_foot?_to_air` from the ground,
      actions 0x0a4c8c0e/0f; `xx_h_freestep_<front|left|right|backleft|backright>_<front|down|up>_<dist>_foot?_to_air`
      from a beam, post or roof edge, 0x0112b589/0x0112b5ac), then a flight blend (`xx_h_air_<front|down|up>_<dist>_
      foot?_to_<freestep|freestep_down|freestep_deep|passover|surface|swing|assassinate>`, 0x010ddafa/0x010df0d8 and
      others by target type), then the reception (`..._tr_freestep_entry_foot?[_fast]`, 0x010de1fe/0x010df150, which
      the graph sends on into the walk, jog, sprint, wait or the beam walk/jog/wait), or a ground landing
      (`xx_h_landing_<forward|straight>_<soft|hard>_...`, by the stick and the speed; over 3 m damage or roll).
      Banned445's `src/player/jump_blend.rs` (blend weights by direction, height and distance; the root follows the
      blended displacement plus a linear correction to the target) and `jump_clips.rs` (the tables, generated from the
      install) are MIT: port them with credit, through `AnimLib`'s clip mixing. Also `xx_h_beam_autoclimb_foot?_<020|
      130>cm` (stepping up onto a beam) and `xx_h_freerun_entry_*` are in the same block, unused here.
- [ ] Drive moves from the move graph (`forge::graph::MoveGraph`): it is loaded and checks every chain step; only the
      ladder plays its table (`HumanLadderData`). Next: the other `Human*Data` constructors' action tables (climb,
      ledge, in-air, ground, narrow object, walling, haystack, kiosk: 2035 action ids used in code, docs/NOTES.md
      "What the executable's code asks for"), then pick moves from them instead of by name.
- [x] `xx_h_air_front_050cm_footl_to_freestep`: found (run log); part of the jump system above. The crate hop (an L of
      open crates in a market, the user's report) is a short free-step jump between narrow tops with it.
- [ ] Landing heights: who sends `LandingEvent` (made through an event factory, vtable 0x16f0424); the thresholds
      here are guesses (4.5 m soft). Banned445's repo reads the landing function (drop + 3 m camera shake, etc.).
- [ ] `HumanClimbData` / `HumanGroundData` constructor defaults (0.75, 0.6, 4 x 0.5, 4 x 0.1, int 4; 1.5, 0.4, -0.4,
      0.9): names are CRC32 hashes; match them against Banned445's decoded fields to learn what they are.
- [ ] Reflection descriptors: property entry type 0x0d and the first field are unknown.
- [ ] The senders of the ground events nobody has traced (ledge stop 69, pull-down 70, look-down 119, ladder 38, beam
      72): we use our own triggers.
- [ ] Beams in cities: no beam (or kiosk) edges are authored in any city, yet the exe has `GuidanceBeamDetectorAccurate`,
      `GuidanceSystemCapsule`, `GuidanceSystemBarrel`, `GuidanceSystemOptimizerGroundWall` and
      `GuidanceSystemGenerationType{None, Behaviour, InertComponent, RigidBody}`: guidance generated at run time from
      collision primitives (a capsule or barrel shape is a beam or pole) and from rigid bodies. Cities already read those
      primitives (Damascus: 2121 entities by primitives). Checked 2026-10-07: Damascus has only 47 capsule shapes, radius
      1.0-1.3 m (and one 0.15 m stub), none lying level as beams: its beams are not capsule shapes. Next: what
      `GuidanceBeamDetectorAccurate` and the `RigidBody` generation type look at (Ghidra: their constructors and the
      code that fills `GuidanceObjectSubTypeBeam`), and whether the crate rims are rigid bodies.
      Found: `GuidanceBeamDetectorAccurate` (class id `d0a682d2`, vtable 0x169a1cc) is a serialized settings object:
      its reader (0x679d50) takes five 4-byte values (+4..+0x14), its clone 0x679e00 copies them and two more members
      (+0x30, +0x38) and a sub-object at +0x18. Find the data objects of that class (or the class that embeds one) and
      read the five values: they are what AC1 calls a beam. `GuidanceDepthDetector` (`6f98e723`, with
      `GuidanceDepthDetectorSettings` `479a4b0d`) and `GuidanceZone*` likely the same kind.
- [~] (Mostly in: hang, knee, free hang; missing over the edge and onto a ladder) Wall run endings (`HumanWalling`, Banned445's `walling.rs` has the action ids): from the entry, a top 1.31-2.0 m
      up pulls up onto it (`_entry_..._tr_hangknee`), else a free hang or over the edge (`_tr_passover`); from the step,
      2.01-2.5 m up pulls up, 2.51-4.3 m ends hanging on the wall (`_step1_tr_hangwall`), else a free hang. Ours runs up
      and grabs the highest ledge with our own moves.
- [ ] Hay entries Ground, FreeStep, SideJump (`HumanHayStackData`): only Top is recreated.
- [ ] Cloth: AC1's `DynamicMesh` `UCMA_Altair_Cloth_SoftBody` and `ClothActionSettings`; the robe runs on our constants.
- [ ] Side wall run: AC1 has the code (`WallingType_Horizontal`) and no clips; ours is procedural.
- [ ] Format gaps: 16 of 9373 data files fail to decode; 16 of 22228 clips fail to parse; 64-bit quaternion tracks
      (hips and spine only) and the camera channel; 11 mesh types (foliage, flags, other cloth, the hay's vertex
      format); the cloth-sim object in cloth meshes (class 0x5755de7f); guidance's `Partitioner`; Havok body shapes
      other than the common primitives; the hay meshes' scale (6.2 m across, twice a cart).
- [ ] Real hold data: cities also take holds from probed lips in the geometry; AC1's authored guidance should cover them.
- [ ] Acre, Jerusalem and Kingdom: load and check.

### 3. Everything else
- [ ] The rest of Banned445's speed model: the 17-weight locomotion blend, lean and bank, pivots chosen by speed, the
      start-move clips' weights (our animator mixes gaits by m/s).
- [ ] Ladder clips spread a difference over the move (the closest clip's root motion, "made up over it").
- [ ] Damascus cornice hang/leap-through-wall fix: reasoned, never replayed. Masyaf's tutorial tower hay not found
      (19.8 m to the nearest).
- [ ] Combat (removed; rebuild on `HumanGround_Fight*` once movement is right), with the pad's lock-on and weapons.
- [ ] Rebound to hang: listed as a combo; check whether it was built.

## The campaign

### P: parkour
A course in the test world plus scripted runs through the existing areas (blocks A to E, flow lanes, rooftops B1 to
B6, tower T2, wall W, swing bars, beams, posts, kiosk bars, ladders, hay). Each move from standing, walking, running
and sprinting, in both profiles where it applies:
- Ground: start, stop, turn on the spot, run turn-around, skid, ledge stop, look down, edge halt, pull-down, steps
  (0.3 / 0.45 m walked, 0.6 m a wall), collide / glance / lean, vault, free step, ramps and stairs, stopping on a slope.
- Speed: bands reached in time, half stick, the high profile turn hold back, speed through a landing.
- Capsule: doorways, narrow lanes, corners, sliding along walls, thin walls at a sprint.
- Climbing: grab, up/down/sideways, corners, leaps, hangs, shimmy, top outs, drop, catch, wall run, side run, rebound,
  back eject.
- Air: running jumps to tops and holds, short hop, long fall, landing grades, leap of faith, beams and posts, bars,
  ladders.
- Combos (each gets a zone if it works): wall run to a hold, rebound to a hang, beam hop, roof gap into a catch,
  ladder top to jump, perch to perch, the crate hop.
- Smoothness per move and per seam: bones jumping in a frame relative to the root, root teleports, steps not in the
  move graph, feet sliding while planted, feet through the floor, hands off their holds.

### I: IK
Hands on holds, feet on the wall, foot placement on stairs and slopes, the pelvis drop, look-at, the climbing reach:
per-frame jumps, targets flipping between surfaces, hands off the hold line or inside the wall, knees and elbows
bending the wrong way. Each measured with IK on and off on the same run.

### V: visuals (later)
Altaïr (textures, cloth, weapons, hood, scabbard, robe through the legs); every city prop type from a few sides (black
or missing textures, wrong UVs, missing meshes); NPCs. Tool: a shot pass over a list of spots (`AC1_LOOK` +
`AC1_SHOT`), flagged by colour stats.

### R: research
- The move graph for every block the player uses: moves AC1 has that we don't.
- How Damascus parkour should play: the authored guidance kinds we don't use yet.
- Banned445's AC1-Movement-Rewritten: check for updates each pass, borrow with credit.

## Run log

### 2026-10-07: first pass (P and R; I through the fixes below)
Fixed before the pass (from the user's recordings), each with a scenario:
- Running up the stairs in high profile played the low-obstacle collide and stood leaning: the collide and the
  free-run vault measured an obstacle from the feet; on stairs a riser two steps up read as 0.42 m. They now measure
  from the floor just in front of it (`rise`).
- Slow walk on beams and no sprint: the speed model left the player's base speed at 0, which the beam (and the other
  moves off the ground) read to choose the jog. The profile's full speed is the base again; the speed model replaces it
  on the ground only.
- Stuck at a beam's end: the walk only counted as off the end when a frame's step passed it by 2 cm, which a slow walk
  at a high frame rate never does; and standing at the end had no step-off at all (it jumped). Both fixed.
- Leap of faith through T2's beam: run at a little off the beam's line, the floor probe missed it. A beam going on
  ahead now takes the run onto it (a hop, with `xx_h_air_front_050cm_footl_to_freestep`, as AC1 does), and free
  running off its end over hay is the leap of faith (it jumped to nothing before, to its death).
- Free running reached the edge below sprint speed (after a landing) and walked off: free running now jumps at edges
  at any speed, as AC1 does.
- Two-step lane overshot the first post onto the second: when the jump's push is raised to clear an edge, the way
  across is now spread over the real airtime (it kept the run's speed and flew too far).
- Arms and legs twitching while climbing: the IK's corrections switched on at full strength, and the wall ray met the
  front of each hold as a foot passed; faded in and smoothed.
- Stopping while sprinting up the course ramp sank into it: ground moves now follow the slope.

Research:
- The move graph places `xx_h_air_front_050cm_footl_to_freestep` in `HumanInAir` action 0x10ddafa (code-driven), after
  the `passover_*_to_air` clips; its `_tr_freestep_entry_footr` goes on into the run, walk, wait and the beam's
  walk/jog/wait. `xx_h_sprint_impultion_footl` is in `HumanGround`'s locomotion blend. So AC1's jumps are a takeoff
  blend, a flight blend and a reception (backlog section 2, first item); `HumanInAir` has 133 actions, among them
  `air_front/up/down` flights to 550 and 800 cm, `freestep_*` takeoffs in five directions, `beam_autoclimb`,
  `freerun_entry`, `hangwall_reception_front_<straight|45_in>_<min|max>` (catching a wall from a jump).
- Moves in AC1's blocks that nothing here plays yet (each a zone and a scenario once built):
  - `HumanWalling`: the wall run's hang, knee and free-hang endings are in (`wall_run_options`: names built at run
    time, which a first literal search missed); not yet: `_tr_passover_hand?` (over the top's edge),
    `_step1_tr_h_ladder_up` (onto a ladder), `_tr_hangwallfree_swingstraight`, the entry's `_tr_hangfree_swingback`.
  - `HumanClimb`: (done 2026-10-07: `xx_l_climb_1m_to_groundentry_<side>`, sideways off the wall at its bottom; the `_90`
    turned ones not yet) `xx_l_climb_1m_
    lookaround_<side>`, `xx_h_hangwall_wait_lookaround_<side>` (looking round on a wall), `xx_h_ladder_wait_tr_<hangwall|
    hangfree>_<side>` (used: sideways off a ladder).
  - `HumanNarrowObject`: `xx_l_beam_pilotis_to_pulldown_soft_<side>[_orientation]` (from a post down to a hang),
    `xx_l_beam_edge_stop_tr_crouchwait`, the crouch turns `xx_l_beam_crouchwait_turn_<side>_to_crouchwait_90` /
    `turn180`, `xx_h_freestep_entry_tr_beam_pilotis_wait` and `_tr_crouch<walk|jog>_<side>_<30|90>` (landing on a beam
    turned), `xx_h_wait_hipm_tr_freestep_front_turn90_<side>`.
  - `HumanLedge`: (done 2026-10-07: `xx_h_hangknee_tr_<l_walk|h_jog>`, topping out straight into walking or jogging) `xx_h_swing_stop_
    <front|back>` (a swing bar coming to rest), `xx_h_climbing_hangfree_tr_<hangwall|climb2m>_down_<min|max>_<200|300>`
    (dropping from a free hang onto a wall below).
  - `HumanClimb_Jumps`: leaps between hang kinds by side and distance (`xx_h_climbing_<hangwall|hangfree|climb1m>_tr_
    <hangwall|hangfree|climb1m>_<side>_<2|3>`): 2 and 3 m sideways leaps, ours stop at the near one.
- IK and pose pops (I), from the user's recordings of 2026-10-06/07, by a scratch detector that flags a bone whose speed
  relative to the root jumps to more than 3x (+1.5 m/s) its speed the frame before (by speed, not distance: long frames
  are hitches, not pops):
  - Fixed: the climbing reach and feet (82 in one climb before), the low/high stand switch (127 bone pops in one
    recording: the whole body jumped up to 13 cm each press of the right button).
  - Seams between states (each a few cm to 22 cm, in one frame): ground into a stop (`xx_h_jogstop_*`), a stop into
    a turn on the spot, a turn's end into the run (feet 22 cm), into and out of the vault and the falls
    (`jumpstraight_clear_footall_tr_fall`), a landing into the run. Next: fade each seam by its gap (the climbing
    chain already does: `seam_gap`).
  - While sprinting steadily and weaving (6.2 m/s, yaw changing), 5-7 cm pops of a hand or foot every 0.1-0.25 s:
    look at the banked gait mix (`FULL_BANK`, the bank's smoothing). Not the half-cycle seams: every gait's
    `_footl` end and `_footr` start match exactly in every bone's rotation and translation (`examples/gaitseams`).
    A pop at the start of a scripted sprint was a long first frame (loading): the speed rose 1.3 m/s in it.
  - Fade bugs found and fixed on the way: the exit fade always ran against 0.25 s whatever its length, and faded from
    the final pose (IK included) into one the IK then corrected again; ground foot placement switched off at once for
    every move, even standing ones (stops, turns), popping the feet at each seam.
  - The leap of faith's dive (`xx_h_faith_jump_100cm_long_3000cm_down`, 10 in one leap) and the climbing leaps up
    (`xx_h_climbing_climb1m_tr_climb1m_up_l_hand_2_a/b`): check the dive's stretch over the fall and the leap's IK.
- Jumps onto a post hold about 0.17 s on it while the quick reception plays out before the next jump: AC1's graph
  goes from `_tr_freestep_entry` straight on (`freestep_entry_*_tr_h_sprint_impultion`): cut it short when running on.
- Swing bars: AC1 stops a swing (`xx_h_swing_stop_<front|back>_a..d`, after the back or front swing) into a still hang
  on the bar, which nothing here has yet (the swing goes on until the legs fling or drop). Needs the bar hang state.
