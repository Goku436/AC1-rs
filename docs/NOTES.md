# ac1-rs: technical notes

A from-scratch Rust runtime for Assassin's Creed (2007, PC) that reads the game data from **your own
install**. No game files, textures, meshes or code from the original are stored in this repo; `out/`
(extracted data) is git-ignored and must never be published.

## Status
| Layer | State |
|---|---|
| `.forge` archives (index, names, FILEDATA records) | done, all 18 archives |
| Chunked LZO1X decompression | done: 15906/15906 data files decompress (`examples/decodecheck`); 9357 split into objects, the rest are streamed sound (6531 `*_BAO_*`: Ubisoft BAO audio, signatures 01 1b 01 00 / 02 1f 00 10 as the QuickBMS scimitar script names them, not object containers) and each forge's `GlobalMetaFile` (18, its own format, not read) |
| Object container (class hash, name, id, body) | done |
| TextureMap -> PNG (DXT1/3/5, BGRA8) | done, verified visually |
| Mesh (class 415d9568): skinned verts, submeshes, bone palettes, materials | done, 120 skinned (incl. cloth, type 4) + 31 static of 168 in DataPC |
| Static meshes (type 0, stride 24) | done: positions `xyz/32768 * |w|/8` m (sword, terrain tiles meeting exactly, houses on their ground); 11 other mesh types fail |
| Entity (0984415e): meshes, skeletons, material overrides | done for characters |
| City placement: entities' world transforms and meshes across a forge's data files, EntityGroup (3f742d26) members | done: Masyaf 4344 meshes placed (926 from groups; 2221 entities collide by their shapes, 5 data files of them looked up in `DataPC_Kingdom.forge`), Damascus 17581 (8851 entities from groups, 15.9 M collision triangles), shared props from `DataPC_Common.forge` |
| Characters assembled from shared parts across data files (soldiers, crowds) | done: AC1's NPC builders (`EntityBuilder`/`BuildTable`), links into `DataPC_Common.forge` through an object index; the NPC line-up and scholars use them |
| Move graph (`ActionKit`/`ActionBlock`) | done: 452/452 blocks parse (15463 actions, 15958 items); blend spaces used for locomotion, stops, leans, grabs, leaps |
| Collision shapes (`MeshShape`, `BoxShape`, `BarrelShape`, `CapsuleShape`, `ListShape`) | done: 14889/14889 meshes (6.5 M triangles) and 2098/2098 primitives parse; cities collide with them where an entity has one (Damascus: 15744 entities, 2121 by primitives), except AC1's out-of-bounds walls (`OutOfBound_*`: locked districts, mission areas) and camera-only barriers (Damascus: 231 left out) |
| Havok packfiles (`HavokChunk`), ragdolls (`RagdollNew`) | done: 72/72 ragdolls read; the dead go limp (Verlet ragdoll with AC1's bodies and limits) |
| Cloth (`ClothComponent`, Altaïr's robe tails) | done: simulated over the skinned mesh, colliding with the legs |
| Viewpoints (`ReachHighPoint_*` groups, entities named `*ReachHighPoint*`) | done: Damascus 12, each with a leap of faith into its hay; Masyaf 1 (the tutorial tower, its hay not found: 19.8 m to the nearest) |
| City parkour objects by mesh name (hay carts, `Ladder_*`, `Banc_*`, `Pole_*`) | done: Damascus 46 haystacks, 301 ladders, 320 benches, 159 swing bars; Masyaf 14, 40, 28, 2; climbing probe 8/20 spots over the top |
| Navigation meshes (`NavMeshManager`) | done: 1432/1432 parse (67836 pieces, 1.13 M triangles); city routes (A*) |
| Clip sets (`AnimSet`) | done: 81 read; soldiers and townsfolk play their own clips |
| Flight recorder (F9), controller, scenario suite | done |
| Skeleton (24aecb7c) + bone names (CRC32) | done, 50/50 in DataPC |
| Animation (0fa3067f): tracks, 16/24/32/48/96-bit quats, packed translations | done, 22212/22228 clips parse; 64-bit quats + camera channels todo |
| City levels (`AC1_LEVEL=masyaf`, `damascus`, any `DataPC_<City>.forge`): textured, with water, collision and holds found in the buildings' edges; free camera (P) | first version |
| Bevy runtime: skinned Altaïr with mip-mapped textures, locomotion, IK, climbing/jumping, free running (stepping and jumping onto low obstacles, posts, beams, swing bars, ladders, kiosk monkey bars); combat removed for now; scholar crowds to blend into | first version |
| Player controller, free-running, combat, AI, crowds, missions | long-term |

## Running
    cargo run -p ac1          # AC1_GAME_DIR overrides the default install path

A city from your install instead of the test level: `AC1_LEVEL=masyaf` or `AC1_LEVEL=damascus` (or `run_masyaf.bat`,
`run_damascus.bat`). P toggles a free camera (WASD along the view, Space/E up, Ctrl/Q down, Shift faster).

Or build once and run the executable:

    cargo build --release -p ac1
    target/release/ac1.exe    # set AC1_GAME_DIR if the game isn't at the default Steam path

See the header of `crates/game/src/main.rs` for controls and test hooks.

## Controls (AC1's puppet system)
The mouse looks around (Esc frees the cursor, a click takes it back); WASD moves.
Combat (fights, weapons, assassinations, guards) is left out for now, to get movement right first. The removed code
is kept outside the repo; see "Combat" below.
- Right button held: high profile (faster, louder).
- Space (legs): low profile, blend: the praying walk (`_cpm_monk_pray_walking`, `xx_l_pray_wait` standing); among
  scholars Altaïr hides, and with no direction held walks along with the group. High profile: free-run (sprint;
  steps or jumps onto low obstacles, jumps off edges, runs up or grabs any wall it is steered into, at any speed). A press jumps only in high
  profile with a direction held; standing, or in low profile, it only grabs or climbs onto what is in reach (a wall,
  ladder, bar). On a wall: held with a direction, leap (held going up, it leaps hold to hold and climbs over the
  top); a press jumps off a perch, rebounds off a wall run, flings off a bar or hops out of hay.
- Shift (empty hand): a gentle push (`xx_l_push_attemp_<hand>`, upper body only, layered over the walk: the spine and
  up from the clip, hips and legs from the gait); next to a scholar, a pickpocket; on a wall, let go. Sprinting into
  scholars shoves through (`xx_h_charge_run_shove_front_b`).
- Speed (`gait.rs`, AC1's model as Banned445's AC1-Movement-Rewritten reads it, MIT): one speed value from 0 to 1 in bands,
  walk up to 0.25, jog to 0.5, run to 0.75, sprint above. The wanted value is a base by profile (low 0, high 0.5, free
  running 0.75) plus a quarter of the stick, so a full stick walks (low), runs (high) or sprints (free running) and a
  half stick (a controller) goes a band slower, down to the slow walk. It rises at 1/s and falls along AC1's response
  curve (quick out of the walk and the sprint, slow through the jog and run); starting from standing it jumps straight
  to the walk (low) or jog (high); letting go zeroes it (the stop clip carries the slide). In high profile, steering
  more than 45 degrees off the facing holds it back while turning. The value maps onto the gait clips' speeds (root
  motion over the half-cycle): slow walk 0.1 m/s (`xx_l_walk_slow_hipm`, a shuffle), walk 1.9 (`xx_l_walk_hipm`), jog 3.5, run 5.2 (`xx_h_run_hipm`),
  sprint 6.2 (`xx_h_sprint_hipm`). The HUD shows the band and value. Scripted walks (`AC1_WALK`) keep a fixed speed.
  Wall runs and side runs need the sprint (over 5.5 m/s, so about 0.4 s of free running from a run); free running
  jumps off edges on its own at any speed (as AC1 does: after a landing the speed starts over below the sprint). Pressing Space to start free running is not a jump: a press while running jumps only with a
  drop within 1.2 m ahead.
- Body collision (`Level::capsule_push`): AC1's character proxy as a capsule, radius 0.4 m (0.35 and the 0.05 keep
  distance), 1.8 m tall, lifted 0.37 m off the feet on the ground, so lower things pass under it. A contact low on the
  bottom sphere (within 45 degrees of straight up at first touch: a top under 0.49 m) is a step and walked up; a 0.6 m
  block is a wall. The body moves in steps of at most 15 cm, is pushed out of walls and slides along them (the speed
  into a wall stops). Running into a low obstacle (collide, glance, vault) is judged along the stick's direction, as
  the body's own velocity runs along the wall once sliding. (Dimensions from Banned445's repo.)
- Running into scholars charges through them: Altaïr shoulders them aside (`xx_h_charge_run_shove_<left_handl|right_handr>_b`,
  upper body) and they stumble out of the way (`xx_stumble_soft_50cm_<back|front>_footl`), then walk back to their places.
- P free camera.

Low obstacles (collide, glance, vault) are measured from the floor just in front of them, not from the feet: up
stairs, a riser two steps up is one step, not a 0.42 m wall. A beam's end with ground going on at its height (a roof)
is walked or stepped off onto it, standing at the end too.

At the bottom of a wall, sideways past the last hold steps off onto the ground (`xx_l_climb_1m_to_groundentry_<left|
right>`, then `_tr_h_wait_foot?`), where there is ground near the feet's height and nothing in the way.
Topping out with the stick pushed on (toward the top), Altaïr goes up off the knee straight into the walk, or free
running the jog (`xx_h_hangknee_foot?_tr_<l_walk|h_jog>_foot?_a/b`), instead of standing up first.

In the air, the stick pointing off to the side (more than 20 degrees off the facing, not behind) looks for a wall that
way within 1.2 m: met within 60 degrees of square, its holds are caught as ahead, turning to it (the directional
grab). A fall from walking off an edge catches only while the legs are held (AC1's grab request), ahead or that way.

The run turn-around flips the root under the body as it starts: its fade starts from the shown pose turned with it (a
little short of half round, so it comes round about the vertical one way), the root's new facing is set in the frame
the move starts (posed that frame, it showed the body swung round), and fades blend the root bone (which turns the
body in some clips) whichever way round stays nearest the last frame (the shortest way switched sides mid-fade).

Ground moves (AC1's `HumanGround`, played as clip chains with root motion, then back to walking; the clips' paths are
flat, so the stops and landings follow the ground's height, kept for the rest of the move, up a ramp too):
- letting go of the direction while running stops (`xx_h_runstop_<foot>` + `_tr_h_wait_hipm`, sprinting
  `xx_h_sprintstop_<foot>`; steering again breaks it off); reversing while running turns round with AC1's run
  turn-around (actions 0x5e/0x5f, `xx_h_runturn180_<foot>`: the facing flips as it starts and the body comes round in
  the clip), then on into the jog or walk (0x60/0x61, `_tr_<jog|walk>_hipm_<foot>`), or, the stick let go, into its
  stand (0x68/0x69, `_tr_h_wait_hipm_<other foot>`); standing, steering more than 45 degrees off the
  facing turns on the spot, a quarter or a half turn to that side (`xx_<l|h>_wait_hipm_<foot>_to_<l|h>_waitturn_
  <left|right>_<090|180>_<foot>`: AC1 ships all four, in both profiles, from either foot), then into the walk (low
  profile) or jog (high). Only the turn itself plays: AC1's move graph goes from it straight into the walk, jog, run or
  sprint, so the gait the stick asks for takes over as it ends (on the turn's foot, as the graph has it), and in high
  profile the turn breaks off into the run once it and the stick are within 30 degrees (the `_tr_<gait>` exits that
  walk off forward are not used). Turns follow the stick: steering during one bends it toward the stick (up to 4 rad/s on top
  of the clip's own turn), so the camera and the stick stay free through it; letting go before the exit plays ends
  the turn in its stand (`_waitturn_..._tr_<l|h>_wait_hipm_<foot>`). Out of a move into a gait, the walk picks up on
  the step and at the point in it whose legs match the pose (`Animator::resume_matching`), at the gait's speed: a turn's
  exit names the gait, not the point in its stride, and a frame standing still restarted the walk from the stand.
- changing profile standing fades between the two stands over 0.3 s (the feet stand elsewhere in each: switched at
  once, the body jumped up to 13 cm); fades start from the animated pose before the IK, so the IK is not counted twice
  through them. Ground foot placement goes on through the moves made standing (a stop, a turn, leaning) and fades in
  and out over 0.2 s where it starts or stops.
- standing and starting: Altaïr stands in the profile's stand loop on the foot ahead (`xx_<l|h>_wait_hipm_<footl|
  footr>`, as AC1 names stands, gaits, stops and turns by that foot); starting off begins the gait at the top of the
  half-cycle of that foot, stopping stands on the other one, and after a stop, turn, landing or climb the gait or
  stand picks up on the foot the last clip names (`runstop_footl_tr_h_wait_hipm_footr` stands on the right); Ground moves
  (stops, turns, landings, rolls) stop at an edge with more than 0.6 m below, as at a wall, rather than walk off it;
- walking at an edge with more than 5 m below stops at it (`xx_h_ledge_stop_start_footl`, `_end_footl`,
  `_end_tr_h_wait_footr`) and holds back from it until steering away or free running; standing at an edge with more
  than 2 m below looks down over it (`xx_l_ledge_lookdown_front_<foot>` as the idle);
- low profile + legs at an edge with a hold along it (or the legs standing there in high profile) pulls down onto it,
  and down on the stick then climbs down the wall's holds and steps off at the bottom: the root faces the wall throughout while the
  body starts turned round (`xx_l_ledge_<stop_start_footl|lookdown_front>_pulldown_front_orientation`, which steps
  back over the edge), `xx_l_ledge_pulldown_soft_front`, then `_to_hangwall_straight_a/b` (feet on the wall) or
  `_front_to_hangfree_a/b`, steered so the hands end on the hold;
- running into a wall at least 1.5 m tall without free running puts the hands on it (`xx_h_collide_full_hand_150cm`,
  `_tr_h_lean_025cm_twohand_150cm_wait`) and leans there (`xx_h_lean_040cm_twohand_150cm_wait`) while pushing into
  it; steering away pushes off to that side or back (`..._to_<left|right|backleft|backright>_jog` and its `_tr_h_jog`),
  letting go stands up, the legs climb;
- a running landing that would hurt, or from more than 3 m, rolls (`xx_h_landing_damage_footl_roll`,
  `xx_roll_hipm_tr_h_jog_hipm_footr`);
- the legs next to a haystack jump into it and hide (`xx_h_air_to_haystack`), in low profile only: in high profile they free run past it (a dive in from a top beside it stays).

Partial-body clips: measured over every clip for Altaïr's skeleton (legs that never move while the spine and arms
do, or the other way round), the ones the player uses are layered over the rest of the pose: upper body (spine and
up) for `xx_l_push_attemp_<hand>`, `xx_h_charge_run_shove_*`, `_cpm_monk_pray_walking`/`xx_l_pray_wait` (the blend).
Other partial clips
are NPC idles and talks (`*_puppet_sad_pray`, `guard_offduty_talk`, `*_rest_sit_idle`) and cutscene poses.

Holds over a set-back wall (a cap on a pole, a sill): only a wall within 0.32 m of the ankles gives the feet footing
(flush walls put them 0.15-0.23 m away), so those hangs are free hangs; jumping onto such a top measures to its edge,
not to the wall under it; wall runs need a wall flush under the edge or holds (none up a pole). Side leaps
(`xx_h_climbing_<climb1m|hangwall|hangfree>_tr_<climb1m|hangwall|hangfree>_<left|right>_<2|3>_<a..e>`) go from and
onto the wide wall hang too; the long one into a free hang (`_tr_hangfree_<side>_3_a..e`, 2.55 m across, 1.3 m down)
is AC1's risky catch: one hand on the ledge, the feet finding nothing, a swing back, then steady.

Where to jump: running jumps and jumps off a post or beam aim at one target, chosen as AC1 does (per the
movement notes): posts and beams, or a walkable top across a gap, within 45 degrees of the wanted direction, 1-4.6 m
across and at most 3 m down, the highest first, then the nearest and nearest the line steered along; the jump flies
the arc that comes down on it (from where the takeoff leaves the ground). With nothing there, the plain running jump.
Clip moves on the ground (landings, rolls, stops, turns, hopping out of hay) stop at walls.

Ledge hang: hanging from a ledge with the feet on the wall is AC1's `hangwall`, not the climbing grid: jump-ups,
wall-run catches, pull-downs and leaps onto a ledge end in it (`xx_h_hangwall_wait`, hands apart, or `_waitclose`); it
shims along the ledge (`xx_h_hangwall_strafe_<side>_050cm_<open|close>`), climbs onto the wall's holds
(`xx_h_hangwall_<dir>_climb_1m`) and tops out (`xx_h_hangwall_tr_hangknee_footl_a/b`). (Before, these ended in the
grid's `xx_climb_wait_2m`, chest to the wall.)

Momentum (the flow lane, south of the test area, x -20 to 40 at z -26..-23, walled: a 0.6 m block to step onto, 1.2
and 1.0 m walls to jump onto, a ramp
to a 2 m walkway, a 2.5 m gap, a jump down across a gap to a 1 m walkway, a drop): free running holds the 6.2 m/s
sprint all the way. Running landings from under 2 m carry on at speed without the landing clip (the crossfade from
the air pose covers it); landings and jumps leave at up to 6.5 m/s; aimed running jumps land on their target (see the
running jump below), a short hop landing further on where the ground goes on.

Collision: triangles too big for the raycast grid (the test world's ground plane around the blocks) are tested by
every raycast; they used to be skipped, so the ground outside the 40 m block area had no collision (cities have none).

Pose gallery (with `AC1_GALLERY`; test world, west of the start, x -40 and beyond, rows from z -14): Altaïr's held loops (the `xx_`
clips that wait, idle or cycle; not transitions, look-arounds, horses), one foot of each pair, up to 10 per row, in
rows by category (climbing, ledge hang, free hang, ladder on walls; beam and post, swing bar, lean, hay, standing,
sitting, other; combat clips are left out), each figure named up close. Scripted runs leave it out unless `AC1_GALLERY` is set.

Ledge hang square-up and waits: a hang settling after a move turns square to the wall (a catch at an angle never
stays at it) and picks the closed or open wait loop by the hands' spacing; every move into a loop crossfades by how
far its first frame is from where the move ended. Hands hold a ledge 0.09 m in front of its grab line and 0.08 m
below the top, so the fingers hook over the edge.

Grab outlines (G): the ledges near the player are outlined in yellow, posts and beams orange, bars brown. The HUD's
detail lines: context and the previous one, the move playing, the ground gait, speed, stick and legs, the speed in the
air and the height above the ground, the last landing (drop and damage), FPS.

Leaps: aimed at a hold within 0.75 m of where the clip lands, or diagonally down onto one up to 1.2 m lower (Pole P
to Pole Q from either of its upper rows). The back eject: on a wall, the legs with the stick pulled back turn away
and push off (`xx_h_rebound_<frontleft|frontright>_front_300cm_footl_to_air`), catching what comes. Wall runs and free
running's own grabs need a wall at least 0.7 m wide (not a pole's side). Footing under a hold needs a wall within
0.16 m behind the hold line, probed at each foot and between them (a thin pole flush with a cap is footing, one set
back is not). Both hands must land as far apart as the move puts them (within 0.35 m): a leap can't catch with one
hand on each side of a gap (it did from E's lower rows toward the risky ledge, holding the air). An up leap must land
up: free running up a wall leaps hold to hold, and with nothing above, the plain moves climb over the top (before, it
leapt onto the ledge it held, over and over). Ground moves keep the body 0.32 m out of walls.

Benches: low profile + Space by one turns and sits (`xx_rest_sit_<left|right>_<000|090|180>_to_sitting`, the seated
loop `xx_rest_sit_idle_040cm_01`), hidden as in hay; the legs or steering get up. Pickpocketing: the empty hand next to
a scholar (`xx_pickpocket_attempt_walk_footl`, `_success_finish_footl`). Viewpoint tower V (east of platform E, holds
up its west face to 15.6 m): on its beam's end Q synchronizes (the camera goes round once, 5 s), and the leap of faith
lands in the hay below: the legs on a perch (its beam) try the leap of faith first, aimed at hay below that way
(the stick's direction, else the way he faces), before a jump to the next post. Held anywhere else, Q is Eagle Vision (the screen dims, scholars outlined blue).
Idle: standing still 7 s, an idle variation (`xx_l_idle_onspot_footl_v01..06`).

Characters: scholars wear Rafic's head on its face rig. The shared eyes and mouth
(`Universal_Head_Obj*`) are skinned with their own bind matrices, so they sit on each head's face (rigidly placed, the
teeth showed through the chin and the mouth hung open). Sky fill light 1500 (faces under hoods read). Characters
and city meshes use their materials' normal maps (the texture set's "Normal" map spec: tangent-space RGB with
DirectX's green, flipped for Bevy; tangents generated), so cloth folds and wood and plaster relief show.

Test world labels: every feature has its name over it on screen (Block A-D, Tower T, Poles P and Q, Platform E,
Hiding spots A-C, the course's P1-P4, Crowd A...).

Test world: platform E (6 m, x 8-12, z 12-16) has a ladder up its north face, holds up its south face to the top edge
(pull down onto them; walking at its edges stops there and looks down), a bare west face to lean on and a haystack off
its east face; another haystack stands on the ground at (5, 15.5) to jump into. A ledge hangs in the air west of
E (x 4.6-6.4 at 2.4 m, on a pole set back under it): from E's face at its west end, low on the wall, the long leap
right catches it one-handed.

## IK layers (`crates/ik`, pure math, unit-tested)
- analytic two-bone IK with pole control (legs, arms)
- weighted look-at chain with per-bone limits (Spine..Head)
- FABRIK and CCD for arbitrary chains
- foot placement (pelvis drop, per-foot ground offsets, slope tilt, smoothing); the drop moves the spine too when it is not under the hips (AC1 hangs `Hips` and `Spine` side by side off `Reference`: lowering the hips alone stretched the torso on stairs)
- hand placement (palm onto a surface point, fingers along a direction, elbow hint)

On the ground the game plays AC1's own clips (idle, walk/jog/run half-cycles picked and rate-matched by
root-motion speed) with foot placement on top. Wall climbing plays AC1's climb graph (hang states
1m/1lu/1ru/2m/2lu/2ru, `xx_l_climb_<from>_<dir>_<to>` moves with root motion and rotation, ground
entry jump, inside/outside corner turns, and the pull-up/kneel/stand top-out chain; a corner steers its hands onto the next
face's holds up to 0.6 m from where its clip puts them, out from the new wall too, and sidesteps and climbing jumps need
room for the body beside them, stopping short of a wall meeting this one; AC1 has corner clips for the climb and the free
hang only, not the wall hang), free hang where the
wall drops away under the feet (`xx_l_climb_1m_<dir>_hangfree` in, `xx_h_hangfree_*` shimmy and climb,
`xx_h_hangfree_<dir>_climb_1m` back onto the wall, hangwaist/hangknee top-out), leaps between holds
(`xx_h_climbing_<climb1m|hangfree>_tr_<climb1m|hangfree>_<dir>_<2|3>_*`, 1.5-2.25 m sideways,
1.2-1.8 m up/down), letting go (step down onto near ground, or push off / release, fall under gravity
and land soft or hard), a standing jump straight up (`xx_h_jumpstraight_clear_*`, 1 m) or up to a ledge in front
(`xx_h_jumpstraight_footl_to_<hangknee|hangwaist|hangwall|hangfree>_<150..300>cm`: onto tops at 1.5-2.5 m,
or hanging from holds at 2-3 m), a wall run when sprinting at a wall met up to 60 degrees off square (turning to face it; `xx_h_wallingfront_*`: up the wall to a hold at 2.5 or 4.3 m, onto a top
at 1.3-2.5 m, into a free hang, or back down), and a running jump on the ground (sprint takeoff, ballistic flight, forward
landing into a jog, catching wall holds on the way down with `xx_fall_tr_climb_*` / `xx_fall_tr_hangfree_*`); a move
is only taken if its end pose puts both hands on real holds and the feet on the wall (or, for free hang,
clear of it), and IK puts hands on holds and feet on the wall. Every correction fades in by distance (a hand from 20 cm
of its hold, the shoulder's reach toward it too; a foot from 30 cm of the wall), and each foot's distance onto the wall
is smoothed (12/s), since a foot passing a hold meets its front 12 cm out: switched on at once, these twitched the limbs.
The whole climbing IK also fades in and out over 0.2 s as the hands take to the wall or leave it (corners, top outs,
1 m to 2 m steps): switched at once, the feet jumped 10-13 cm. Block C in the test level has an overhang
for free hang, block D a roof slab to top out from a free hang, and A, C, D stand 1.5 m apart for leaps
and roof jumps (C's side facing D has holds to catch from a jump off D's roof: with no top in reach, a running jump
aims at the nearest hold within AC1's 45 degree cone, 1 m below to 2.6 m above the feet, 0.35 m in from its ends; a
top needs 1.7 m of room over it and must not be inside a block, which a probe starting inside one finds as the floor
under it). A mid-air catch pulls the body in to the wall too (caught further out, he hung that far off it after), and
looks for the wall at the chest or lower (at a roof's edge the chest is level with the roof); a hold is aimed at with
the root arriving 0.9 m under it. A running jump onto a top (not a hold, post or beam) plays AC1's own jump clips
(`jump.rs`, the `HumanInAir` tables as Banned445's repo reads them): a takeoff blended from `xx_h_run_<front|down|up>_<050|
300|550>cm_footl_to_air` and a flight from `xx_h_air_<front|down|up>_<dist>_footl_to_freestep[_down|_deep]`, weighted
by how far and how high the top is (bands: up to 1.3 m up, 3 m down, near 2.5 m, middle 5, far 7), then the
reception `..._tr_freestep_entry_footr`, blended into its `_fast` version by the run's speed; the root follows the
clips' own motion with the difference to the target spread over the flight (as AC1 moves it), and runs on at the run's
speed: running on, the reception ends where its step, past its fastest, slows to the run's speed (it front-loads
its step; played faster to keep pace it lurched the root to 16 m/s, played out it stood still 0.1 s). It falls back to the planned arc below when the clips' own way is more than half off the target or the way
over is not clear at the chest. The free-run vault onto a low top, jumps onto posts and beams, jumps at holds and
jumps with no target (weighted for a level top 2.5 m ahead) take their clips from the same tables, so every step the
scenarios play is one AC1's move graph has (the sprint stride into the hop, which it lacks, is only a fallback now).
Otherwise a running jump's takeoff (about 1.7 m of root motion) starts partway in when pressed
near the edge, so it leaves the ground at the edge rather than running on over the drop. It is planned at running
speed (lengthening the flight instead made jumps float in slow motion); the push up is at least 1.5 m/s, and up onto a
higher top enough to cross the edge with the feet 0.25 m clear (lower, the knees met the edge and the body snapped up
onto it). The way across is spread over the real airtime, down to the target's height (kept at the run's speed, a
push raised to clear an edge overshot the first fence post onto the second), and the air clip plays over it. Moves between hangs of one kind keep
the body as far out from the wall as it was (a free-hang leap drifts in; under a cornice it went into the stone). Behind the stairs, blocks
with tops at 1.5, 2 and 2.5 m and block G (holds from 2.39 m) are for jumping up to ledges. Without animation data the old procedural climb is used.
Space again during a wall run rebounds off the wall, to the side the stick points on screen (by the camera: of the
back, left and right kick-offs, the one heading nearest the stick; pushing into the wall counts only by its part along
the wall) (`xx_h_wallingfront_<entry|step1>rebound_<back|left|right>`; from the
step until 60% into its fall back off the wall, as AC1 also rebounds from there), catching holds behind, and at
the edge of a high roof over a haystack it does the leap of faith (`xx_h_freestep_footr_to_faith_jump_*`, the
`xx_h_faith_jump_*` dive stretched over the real fall, `xx_h_faith_jump_landing` into the hay, `xx_h_haystack_wait`,
`xx_l_haystack_hop_out`); any fall into a haystack lands in it (`xx_h_air_to_haystack`), and walking off a ledge is an
animated fall. Tower T (15 m) has a haystack off its back edge.
Sprinting along a wall, Space runs along it: AC1 ships no clips for that (only the unused `WallingType_Horizontal`),
so it is built from the sprint cycle on a rolled, arcing root. Landings hurt by height (guessed thresholds: 4.5 m
`xx_h_landing_damage_footl`, 8 m `xx_h_hurt_fall_balanced_front_short_500cm_landing`, 13 m `xx_h_landing_death_back`
and a respawn); health regenerates and shows on the HUD; haystacks are safe. Clip seams inside a move crossfade by
how far the hands and feet jump.
Top-outs work from the narrow and the wide hang (`xx_l_climb_<1m|2m>_tr_hangknee_footl_a/b`). Where the top drops
away beside the hands (measured 0.15 m in from the edge, 0.35 m to either side: a post, the end of a wall) the pull-up
uses one hand, as AC1 does: from the wall `xx_h_hangwall_onehand_to_hangknee_onehand_footl_a/b`, from a free hang
`xx_h_hangfree_onehand_to_hangwaist_onehand_a/b` then `xx_h_hangwaist_onehand_to_hangknee_onehand_footl`, both ending
`xx_h_hangknee_onehand_footl_to_freestep_entry_footl`, balancing on the post (perch) when the top is one. A standing
jump onto a post's top (hangknee/hangwaist entry) finishes the same way. Test world: pillars behind block A
(z 11.7-12.3), P at x -1.2 (0.6 m, holds up its front, top 3.59 m: wall hang) and Q at x 1.2 (a 0.6 m cap at
2.39 m on a thin pole flush with its front: a standing jump hangs from it, feet on the pole and hands together,
`xx_h_hangwall_waitclose`; up pulls onto it one-handed, `xx_h_hangwall_onehand_to_hangknee_onehand_footl_a/b`). The 30/45-degree angled one-hand variants are not used. Jumps play their
air clip through the flight (time-stretched to the airtime) and longer falls loop `xx_h_jumpfalling01`.
Free running (east of the blocks, a course along z = 0 from x = 21): low obstacles (tops 0.35-1.3 m up) as AC1's
move graph has them (AC1 has no vault; the `passover` clips are `HumanLedge`'s, going over a ledge's edge downward).
Free running jumps onto one: the sprint takeoff, `xx_h_air_front_050cm_footl_to_freestep` and
`_tr_freestep_entry_footr`, running on along the top at the run's speed (off a thin wall, a running drop). Running
without free running, a top up to 0.85 m stops the body against it (`HumanGround`'s collide item:
`xx_h_collide_full_footl_<050|070>cm_a`, `_b`, `_tr_..._wait`, the 50 and 70 cm clips mixed by the top's height, a foot
up on it) and it waits there (`_wait`); pushing into it with the legs (or free running) steps up (`_to_freestep_<050|
070>cm`, up 0.51 / 0.71 m, `_tr_freestep_entry_footl`), steering away pushes off and runs back (`_backleft_run` /
`_backright_run`). Met at more than about 30 degrees it glances off and runs on along it (`_<left|right>_run`, `_tr_h_run_
hipm_<foot>`). Sprinting (or free running) off an edge jumps (a leap of faith if a
haystack is below; not where a beam goes on from the edge, even run at a little off its line: the run hops onto the
beam, and free running off the beam's end over hay is the leap of faith); running jumps are steered onto the first post or beam along the way whose arc there is clear of
walls (`arc_clear`), land on it with
`xx_h_beam_landing_soft_tr_pilotis_wait_a/b` and balance in `xx_h_beam_pilotis_wait`; from a post, Space or sprint +
a direction jumps a ballistic arc to the next post or top that way (`xx_h_beam_pilotis_tr_impultionstraight_a`); on a
beam a direction along it walks (`xx_l_beam_crouchwalk_*`, sprinting `xx_h_beam_crouchjog_*`). Bars caught in the air
swing (`xx_h_swing_cycle_<front|back>_<up|down>`) and Space flings off at the next forward swing
(`xx_h_swing_cycle_front_300cm_to_air`, S held: `_down_050cm_to_air`) toward the next bar; free running (the legs held,
pushing on) flings on its own, so a run goes through a row of bars hands-free as in AC1. Only bars lying across the
way (within 60 degrees of square) are caught: a pole along the path is not swung on. A flight whose knees meet an
edge with its top at most 0.7 m over the feet (the next roof, a parapet) lands on that top instead of being stopped
against it. Running-jump takeoffs leave at 5 m/s at most. Ladders (block L, south of the course): Space at the foot climbs on, W/S climb 0.5 m a step
(`xx_l_ladder_climb_<up|down>_<l|r>` between `xx_l_ladder_wait_l/r`), it climbs off at the top or steps off at the
bottom, Space at the top edge climbs down onto it (`xx_h_wait_hipm_footl_tr_ladder_pulldown`), Space on it lets go.
Hanging on a wall with a ladder beside, sideways goes across onto it (`xx_l_climbing_1m_tr_l_ladder_wait_*`,
`xx_h_hangwall_tr_l_ladder_*`, `xx_h_hangfree_tr_l_ladder_*`, the side leaps `xx_h_climb_{l|r}_{hand}_{2|3}_tr_ladder_*`):
the move whose hands end nearest its rails, the rest made up over it. Sideways on a ladder goes back onto the wall's
holds (`xx_h_ladder_wait_tr_{hangwall|hangfree}_{left|right}`), steered like a leap since its rungs aren't level with
the holds. E's ladder has holds to its right to try it.
Kiosk frame (overhead bars at 2.1 m, east of block L): Space under it, or at most 1 m short of its end, with nothing
in the way, jumps up to it
(`xx_h_kiosk_freestep_footr_tr_monkeybar_footr`), `xx_h_kiosk_monkeybar_footr` crosses hand over hand and
`_tr_fall` drops off at the end (or on Space); AC1 only ships the right-to-left-hand half of the step, so every
other step plays it mirrored (`ik::mirror`: bones paired by name, unnamed ones by mirrored rest position, each
bone given its pair's reflected skinning delta). On the ground the root's height follows the ground averaged 0.35 m
behind and ahead along the way it moves (steps under 0.4 m count; an edge or a wall does not), so stairs become a
steady climb with each foot set on its own step by the foot IK (which also drops the pelvis for the lower foot and
tilts the feet to slopes), instead of the body jumping 17 cm a step.
Moves started from the ground ease in (pose crossfade, root slides and turns into place over the first clip).
These IK layers are recreations of the behaviour seen in-game, not ports of the original code.

## Fix later
- The face pokes through the side of the hood when the head turns while climbing (head look over a climb clip; the hood is
  skinned to the head without AC1's hood modifiers; Banned445's repo now ports "authored hood and sword-tag modifiers",
  worth reading for this and the scabbard angle). Shot: `out/shots/hood-clip-climb.png` (local).

## Usage
    cargo run --release -p forge --bin forge -- list     "<game>/DataPC_Masyaf.forge"
    cargo run --release -p forge --bin forge -- textures "<game>/DataPC_Masyaf.forge" out/tex [filter]
    cargo run --release -p forge --bin catalog -- "<game>"       # class histogram
    cargo run --release -p forge --bin animinfo -- "<game>/DataPC.forge" "Game Fix" hangfree climb_1m   # clip root motion
    cargo run --release -p forge --bin objects -- "<game>/DataPC_Masyaf.forge" Cell05460_DataBlock [name [hex 256]]  # one data file's objects
    cargo run --release -p forge --bin world -- "<game>/DataPC_Masyaf.forge"   # a city's entities and the meshes they place

## Combat
Removed on 2026-10-05, to get movement right first: sword fights, the hidden blade, knives and fists, ground and air
assassinations, guards and their detection. The code (`combat.rs` and its hooks) is kept outside the repo and comes
back once movement is right, rebuilt on the move graph's fight blocks (`HumanGround_Fight*`, `HumanGround_Assassination`).

## Crowds
Crowds: scholars (AC1's `CNMT_Scholar`: robe, hood and shoes from Acre's data on the shared human skeleton,
limbs and head from the common data, assembled across forges) walk a loop in a 2x2
formation with their hands together (`_cpm_monk_pray_walking`). Walking among them (two within 1.4 m, not
running) the player blends: it walks at their pace in the same praying walk (`xx_l_pray_wait` standing still).

## Research (what is known about AC1 from outside this repo)
- The QuickBMS scimitar scripts (RetingencyPlan/le_quickbms_script_compendium, `scimitar_new.bms` and
  `scimitar_compressed_container.bms`, 2026-10-08) read the same archive and compressed container as `forge` does: a
  chunk's method byte is 1 LZO1X, 2 LZO2A, 3 Xbox LZX (`forge` decodes 1, 2 and 5 with LZO1X; every PC data file
  decompresses, so method 2 did not come up wrongly), and streamed sound is `.bao` / `.sbao`. They stop at the
  container: no textures, meshes or other objects (which `forge` already reads).
Collected 2026-10-05, to check what this project has understood wrong.
- Nobody has published a decompilation of AC1 or a full spec of its data. The AC1 projects online are patches:
  EaglePatch (Sergeanur) and AC Definitive (HenryPDT): controller, windowing, MSAA and telemetry fixes, no movement or
  data formats. ACExplorer reads Unity's forges; the AnvilNext `.forge` notes (broadside wiki) are for AC4.
- AnvilToolkit (Kamzik123) supports AC1 for unpacking; its wiki and resources (`AnvilToolkit-Resources`) document
  later games' data that AC1 shares in spirit. Agreed with what is here: clips are 60 fps, keyed to bones by name
  hash, skeleton-agnostic, referenced by id from AnimSets; add-on skeletons hang from the main one at a bone of the
  same name; a skeleton has at most 255 bones before Unity (byte indices, 255 = none). New to this project:
  `BuildTable` rows of weight 0 are never drawn at random, only by tags or an explicit `RowSelection` (`npc.rs`
  already never draws them, but takes the last row when a table has only weight-0 rows); pre-Unity cloth is
  `ClothSettings`-driven Verlet with distance constraints, per-vertex collision spheres against capsules on the
  bones, and a blend back to the skinned pose by the character's speed (AC1 ships `DynamicMesh`
  `UCMA_Altair_Cloth_SoftBody` and `ClothActionSettings` `Altair_ClothActionSettings`; the robe here still uses its
  own constants). Its AC1 file lists (`File Lists/AC1.gfl`, `Full File Lists/AC1.fgfl`: a 24-byte header, then Oodle Kraken, which the
pure-Rust `oozextract` crate unpacks) list each forge's data files and their objects by name and id: nothing that
`forge::index` does not already give.
- AC1 used Autodesk HumanIK for the hands and feet when climbing and pushing (Autodesk and Ubisoft, 2009). The IK here
  is a recreation; HumanIK is a full-body solver with effectors, so arms and spine follow a hand target.
- The move graph is the authority on what a clip is for. `HumanLedge` holds the `passover` clips (over a ledge's
  edge downward, to a hang or a fall), so the vault built from them was wrong; running at a 50-70 cm obstacle is
  `HumanGround`'s `xx_h_collide_full_footl_<050|070>cm` chain (now used). Every other clip this project plays sits in
  the block of the state it is played in. A transition on an item is (connector action, destination action), one per way out:
  a run stop goes on to walking or jogging through `runstop_tr_walk/jog`, or to standing (`wait_hipm`) through
  `runstop_tr_h_wait`; after `collide_full_*_b` the way out is its `_wait` loop through `_tr_collide_full_*_wait`,
  and stepping up (`_to_freestep`) is an action of its own, started from there. Driving moves from the graph itself
  is open work.
- Tools: Ghidra 12.1.4 with `ghidra-cli` (akiselev) keeps the exe analysed in a local project outside the repo
  (`F:/Claude/re`), for decompiling a function by address or following references; `gamedb` (smileybaal) can index
  such output for search. Neither the project nor its output goes in the repo.
- Ghidra MCP (bethington/ghidra-mcp, Apache-2.0) runs headless on a copy of that project (`F:/Claude/re/mcp`, port
  8099 on this machine) and answers decompile, xref, memory, byte-search and inline-script requests over HTTP; the
  analysis (106,959 functions) is saved in it.

## What the executable's code asks for (2026-10-06)
- Action requests: 2035 of Game Fix's 4454 action ids appear as 32-bit immediates in `.text` (a byte scan for every
  id, each hit mapped to its function by a Ghidra script): these are the moves the code asks for by id, the code-driven
  half that the move graph's transitions do not cover. Movement blocks: `HumanClimb_Jumps` 150, `HumanClimb` 141,
  `HumanLedge` 122, `HumanInAir` 94, `HumanGround` 77, `HumanNarrowObject` 63, `HumanLadder` 49, `HumanWalling` 12,
  `HumanHayStack` 10, `HumanKiosk` 9, in 246 functions.
- The `Human*Data` constructors fill these as tables. `HumanLadderData` (vtable 0x16f20a4, constructor 0xc7cac0)
  holds one action per ladder slot: low and high wait (`xx_<l|h>_ladder_wait_<l|r>`), climb up/down in both
  profiles, entries from the ground (`xx_<l|h>_wait_hipm_foot*_tr_<l|h>_ladder_climb_up_*`), the pull-down from the
  top, the exits (low: `_tr_l_wait_hipm`, high: `xx_h_ladder_climb_up_*_tr_freestep_*`,
  `xx_h_ladder_climb_down_*_tr_h_wait_hipm_*`), falling off (`_tr_falling`) and the rebound
  (`xx_h_ladder_wait_<l|r>_tr_rebound_foot<l|r>`). The ladder here now plays all of these.
- Reflection descriptors (in `.rdata` near the enum tables): a class entry points to its property list, enum list and
  group labels; a property entry is 32 bytes {?, u32 CRC32 of its name (names are not in the exe), u32 enum id or 0,
  u32 type (0x0a float, 0x19 enum, 0x0d ?), u32 offset << 20 (in dwords), ...}. Group labels are strings:
  `HumanClimbData` "IK values", "Grid settings"; `HumanNarrowObjectData` "Lean Variables", "Edge variables",
  "Beam Variables", "Jump distances"; `HumanInAirData` "HumanGuidance"; `HumanLedgeData` "GuidanceObject reports";
  `HumanGroundData` "pindown", "fight system".
- Defaults set by constructors (meanings unproven, names are hashes): `HumanClimbData` 0.75, 0.6, 4 x 0.5, 4 x 0.1 and
  an int 4 (13 floats in "IK values"/"Grid settings"); `HumanGroundData` 1.5, 0.4, -0.4, 0.9. No `Human*Data` object
  ships in the data: these are the values the game runs with.
- `LandingEvent` is made through an event factory (vtable 0x16f0424), so its sender is not found by references to it;
  the landing heights are still to find (from the in-air decision code, not the `HumanInAir` wrappers).

## Behaviour notes from the executable
Strings and MSVC RTTI names in `AssassinsCreed_Game.exe` show how the original splits these mechanics (names only,
no code):
- Wall run: `HumanWallingData` ("rebound distances"), sub-states `WallingEntryA`, `WallingEntryB`,
  `WallingVertical`, `WallingVerticalEnd`, `ReboundTransition`, `WallingHorizontal`; `WallingType`
  Vertical/Horizontal, `WallingSide` Left/Right. No horizontal walling clips ship in any archive (all 43
  walling clips are `wallingfront`), so the side wall run here is procedural.
- Ladder: in high profile AC1 climbs 1 m a step (`xx_h_ladder_climb_<up|down>_<l|r>`) and runs off the top into a free
  step; the legs with the stick pulled back rebound off it backwards (push-off, then `xx_h_rebound_foot<l|r>_tr_fall`
  as it comes down, still facing the wall, landing standing). All from `HumanLadderData`'s table.
- Haystack: `EntityDescriptorObject_HayStack`, `BhvHayStack(Data)`, `HumanHayStackData`, actor state
  `InHayStack`, entry types `Top`, `Ground`, `FreeStep`, `SideJump` (only Top is recreated).
- Leap of faith: actor state `LeapOfFaith`, `LeapOfFaithClip` resolved on jump start/end.
- Landing: `LandingType` Safe / SmallDamage / HeavyDamage / Fatal: damaging from 3 m, heavy over 6.3 m, fatal over 7 m
  of drop from the top of the fall (the game's landing function, as Banned445's AC1-Movement-Rewritten reads it).
- In-air jump types: `JumpType_Straight`, `JumpType_1m`, `JumpType_3m5m`.

## Format notes (forge v25)
- Header: `scimitar\0`, u32 version=25, u64 file-data-header offset.
- Index entries 16 B {u64 offset, u32 ?, u32 size}; name entries 0xBC B, name at +0x2C.
- Record: `FILEDATA` + name, metadata at +0x187 (u32 ?, u32 size, u64 id, ...), payload at +0x1B8.
- Payload: 0+ streams `33 AA FB 57 99 FA 04 10`, u16 ver, u8 method (1 = LZO1X), u16 x2 max chunk,
  u16 chunk count, {u16 usize, u16 csize}[n], then per chunk u32 checksum + data. Stream 1 = object table,
  stream 2 = objects.
- Object table: u16 n, {u32 id, u32 size}[n]. Object: u32 class, u32 size, u32 name_len, name\0, u32 id,
  u32 class, body.
- TextureMap body: u32 w, h, depth, format (0 BGRA8, 2/3 DXT1, 4 DXT3, 5/7 DXT5); +0x43 tag 0x13237fe9,
  +0x47 u32 size, +0x4B full mip chain.

## Mesh / skeleton notes
- Mesh body: u8 1 (0 = static), 7 B, u32 bone count, bones x 0x4C {u32 id, u32 class, u32 name CRC32,
  f32[16] inverse bind, row-major}. Then the compiled mesh (class fc9e1595): stride, vertex/index byte
  counts, 2 submesh tables, vertices, u16 indices; bone palettes, one per submesh: u8 3, u8 flag (1 on the first,
  0 on the rest; requiring 1 everywhere lost 525 meshes, e.g. hay carts, seagulls, townsfolk), u8 n, u8 submesh
  index, u8 `?`, u16 the submesh's vertex count, u8[n] bones; trailer with a material id per submesh.
- Meshes decoded per forge (`examples/meshcheck`): DataPC 151, Damascus 9180, Masyaf 3905, Acre 10759,
  Jerusalem 9466, Kingdom 4588, Common 1840. The rest: cloth meshes not of Altaïr's kind, and foliage/flags whose
  header reads as a huge bone count (another mesh type).
- Open: the skinned hay meshes (`Hay_Bale_01`, `Hay_Bale_Charette_01A`, stride 32, w always 32767) measure 6.2 m
  across at the skinned scale (1/2048), twice a cart's size; their scale or a per-mesh factor is not known yet.
- Cloth mesh (first byte 4, e.g. Altaïr's long robe): u32 4, u32 1, u8 0, an embedded cloth-sim object (class
  0x5755de7f, `?`), then the usual bone count, bones and compiled mesh (found by walking back from the compiled tag).
- Static mesh (first byte 0): no bones, no palettes, stride-24 vertices = the skinned layout without palette
  indices and weights. The position's w is a scale shared by the mesh (sign varies): positions are
  `xyz / 32768 * |w| / 8` m, so every mesh uses the full i16 range (sword |w| = 8, a Masyaf terrain tile 3976,
  the tiles' edges coinciding).
- Heads and faces don't receive shadows (the hood's shadow-map shadow made the face black); arms are pushed out of
  walls (where the upper arm or forearm would cross level geometry the hand moves out along the surface, two-bone
  IK bending the elbow away).
- Every mesh's texture coordinates are raw / 2048, unflipped, and repeat past 0..1 (from Banned445's repo; until
  2026-10-06 we read 1 - raw / 4096 and patched the heads, which showed every texture at half scale).
- Altaïr carries weapons with no skin of their own on tag bones, their vertices in the bone's frame: the sword
  `ARCM_Altair_Sword_D` on bone 3a835926 (from the `UCMA_Sword_Tag` skeleton) and the short blade `UCMA_Altair_Dagger`
  on 685e46b6 (pairs from Banned445's repo); the entity does not list them. The scabbard hangs straight down: AC1
  angles it with the sword tag's modifiers, not read yet.
- Some materials name a texture set kept in another data file, or are kept there themselves: the throwing daggers'
  are in DataPC's `Game Bootstrap Settings` (`DataFile::material_map_in`).
- Cloth (materials named `*Cloth*`) is drawn from both sides: AC1 gives it an inside material, and the robe's tails
  fold their backs into view (culled, they showed the trousers through holes).
- Textures carry the full mip chain after the top level; it is decoded and uploaded (without it they shimmer).
- Skin weights: the four bytes sum to about 255 (254-256); they are normalized to 1 on load, otherwise skinned
  positions scale with the distance from the world origin and the mesh streaks.
- Skinned vertex (32 B): i16x4 pos (1/2048 m), 3x u8x4 normal/tangent/binormal, i16x2 uv (1/2048, v as images are stored, repeating),
  u8x4 palette indices, u8x4 weights.
- Skeleton: u32 0, u32 count, bones with parent refs (`02 id` / `03`), object-space and local
  {vec4 pos, quat xyzw}, then constraint extras. Z up, character faces +X. Mesh space is the skeleton
  model space turned 90 degrees about Z.
- An entity lists `(placeholder, real)` material pairs, its meshes, then the main skeleton followed by
  add-on skeletons (head, skirt, hood, sword tag) rooted at main-skeleton bones with the same hash.

## Class names
A class id is the CRC32 of the engine's class name (Mesh `415d9568`, Entity, Skeleton, Animation, Material,
TextureSet, TextureMapSpec, CompiledMesh all check). The `catalog` tool reads the names at run time from the game's
own executable (its RTTI descriptors, `.?AV<name>@scimitar@@`, and identifier strings) and labels every class: all
183 classes in the install get a name. Not parsed yet: `GridPartition`/`GridCellDataBlock` (the world's cell grid),
`ContactTable` (per-material contact sounds and effects; there is no audio yet), `BodyPartMapping`.

## Navigation meshes
`NavMeshManager_<cell>` (format in `crates/forge/src/navmesh.rs`): pieces, each a walkable polygon with world-space
vertices, triangles (index, three neighbours, negative for borders) and their corner indices; the `navmesh` tool
parses all 1432. A city's pieces form one graph (`nav.rs`): triangles sharing corners, and border edges of
different pieces on one line and overlapping (pieces meet with T-junctions). Damascus: 300535 triangles; street
routes of 50 to 80 m come out (rooftops are separate, reached by climbing). `AC1_ROUTE="x1,z1,x2,z2"` draws one.

## Clip sets (`AnimSet`)
A clip set lists (shared clip, replacement) pairs for a kind of character (`cmma_anim_set`: townsmen walk with
`cmma_walk_hipm_*`; `mmaa_anim_set`, 365 pairs: soldiers; `Elite_military_animset`, `Leader_Military_animset`:
their idles). `AnimLib::use_set` gives a rig its replacements (under the shared clips' names); the line-up townsfolk use
`cmma`/`cfaa`.

## Tools for bugs and tests
- Scenarios: `AC1_SCENARIOS=1 cargo test --release -p ac1 --test scenarios -- --nocapture`. They run 6 games at once
  (`AC1_SCENARIO_JOBS=n`), each ending at its time without a screenshot (`AC1_SCENARIO_SHOTS=1` keeps them);
  `AC1_SCENARIO_FILTER=text` runs only those whose name holds it. 56 scenarios: one at a time with screenshots 560 s,
  now 108 s (4 at a time 161 s, 8 104 s but with timing flakes). A game's start and exit cost about 4 s of each run
  (the scenarios themselves 339 s): one game teleporting between zones would save that, below what running them side
  by side gives, at the risk of one run's state leaking into the next.
- `cargo run -p ac1 --example pops -- <recording.txt>`: pose pops in a flight recording (a bone moving much faster than
  the frame before, relative to the root; long frames are not counted), by what was playing. `forge` examples
  `gaitseams` (how a gait's half-cycles join) and `clipjumps` (jumps baked into a clip). `AC1_VEER=secs,deg` turns a
  scripted walk's direction then (the stick swung mid-move).
The PIKVR test campaign (parkour, IK, visuals, research) is planned in `docs/PIKVR.md`.
- `AC1_SURFACES="x,z,..."` logs every collision surface down a vertical line at each point and `AC1_RAYS` what rays
  hit (and whether from behind); `AC1_START="x,z,yaw,y"` starts on the ground under height y (not the highest roof).
  Used to replay a recording's spot.
- F9: the flight recorder writes the player's last 10 s, frame by frame (root, facing, speed, the move and clip,
  hips/head/hands/feet in the world, IK targets) to `ac1-recording-<ms>.txt` with a screenshot.
- Controller (Xbox or DualShock 4, by button position): left stick moves, right stick turns the camera; A/Cross legs,
  B/Circle empty hand, Y/Triangle head; R1/R2 high profile; Back/Share records; Start/Options free camera; R3 shows
  the holds (`pad.rs`). The game logs each controller it finds ("controller connected: Sony DualShock4 Gamepad").
- Test world additions (2026-10-06): rooftops north-east of the start (x 40-70, z 8-36: climb B1, gaps across, down and
  up, a beam, two swing bars, a running jump onto tower T2's holds and a leap of faith off its beam) and wall W (run
  through a doorway, up a bare wall, rebound back onto the holds over the door). G cycles the outlines: where the hands
  grab (yellow) and the edge of the ledge grabbed (white, found by probing the face in front of each hold), one, the
  other, none. The pose gallery is off unless `AC1_GALLERY` is set (it made the test world slow to load).
- `AC1_SCENARIOS=1 cargo test -p ac1 --test scenarios -- --nocapture`: 39 scripted runs (climbs, top-outs, free
  running up a wall, the risky leap, Pole Q's hang, the course and the flow lane, stopping against a low block and
  stepping up, glancing off it, a quarter turn let go, ladder, poles, bench, hay and the leap of faith off tower V,
  block L stopping a run toward the kiosk, pulling down and climbing down, turning round at an edge, blended grabs and
  stops, the ragdoll) checked against their
  logs; skipped without the flag or without the game. Each run also lists the steps between clips that AC1's move
  graph does not have ("off the move graph"), reported, not failed. Scripts run on their own clock, stepped at most 0.05 s a
  frame as movement is, so slow first frames can't put a script ahead of the body; the game's clock is left alone.

The executable also carries the engine's reflection data: a class descriptor per class (its name pointer, a parent
hash or 1, the CRC32 of its name, its size; at name-0x20 its property list and count), each property 32 bytes
(flags, the CRC32 of its name, its type's hash, a type code, a layout key), and per class an optional generated
deserializer (the function at name+0x58; 0 when the generic one is used). The types and the deserializers' reads
give the byte layout of an object; field names are only hashes when the exe does not hold the name. (Learned with a
disassembler and the class tables, written up here; no code from the executable is in the repo.)

## Move graph (`ActionKit`, `ActionBlock`)
Each item's 12 flag bytes are, in order, bits 0x10.. of the game's per-item gate word (`forge::action::Item::word`,
`gate`; meanings from Banned445's AC1-Movement-Rewritten, checked against its table): 0x10 the animation turns the
body, 0x20 locked (no new move or mode starts during it), 0x40 / 0x80 may be left for standing in low / high profile,
0x200 / 0x800 for moving, 0x1000 a jump takeoff, 0x2000 a `*_tr_fall`. The turns on the spot are 0x30 (turn, locked);
their exits into a gait 0x0fc0 (may be left any time): AC1 plays the turn through and lets the gait take over.

`Human_ActionKit` (DataPC, `Game Fix`) lists 51 action blocks (`HumanGround`, `HumanInAir`, `HumanWalling`,
`HumanLedge`, `HumanLadder`, `HumanClimb`, `HumanClimb_Jumps`, `HumanNarrowObject`, `HumanHayStack`, fights,
reactions...). Format: see `crates/forge/src/action.rs`; the `actions` tool prints a block (`actions <game> HumanLedge`)
or parses every one (452/452). An action is a chain of items; an item lists one clip or several: several are a blend
space, the variants of one move mixed by weight. AC1 fits its moves to the world this way rather than by steering:
- locomotion: walk-slow, walk, jog, run and sprint mixed by speed, each with clips banking into left and right turns
  (`hipl`/`hipr`, `bank_left`/`bank_right`) mixed by turn rate;
- run stops: jog, run and sprint stops by speed; leans: hands at 70 and 150 cm by the wall's height;
- climbing entries and grabs by height (`hangknee_131cm`/`200cm`, `150cm`/`200cm`), passing over by depth
  (`passover_030cm`/`100cm`), `hangwall_tr_hangknee` by angle (straight, 45 in, 30 out), take-offs by direction.
Items also give blend-in times (0.2 s mostly), where the root motion comes from (the clip, physics, or steered:
entries are steered), the feet at start and end, and transitions to other actions. Which action plays when is
partly code, partly data.

The graph at runtime (`forge::graph::MoveGraph`, built with the clip library, 4454 actions in 51 blocks): an item's
transitions are (connector action, destination action) keys, one per way out; after a clip may come the next item of
its action or the first item of a connector or destination. Actions no transition leads to are started by the game's
code (keys such as `0x44` are its requests; a post jump's push off, `beam_impultionstraight_to_jumpstraight`, comes from
the crouch that way), and an item that ends its action with no ways out hands back to the code (the wall run's
`entry_a` and `entry_b` are separate actions the code steps through, its `WallingEntryA/B`); the graph judges neither.
It is used for: each move's fade-in (its item's blend time, where it gives one), and a check on every step from one
clip to the next of a chain (debug log "graph: a -> b is not in AC1's move graph", listed by the scenario suite).
The check found: the post jump skipped its push off (now crouch, push off, flight); free running into a low obstacle
went collide straight into the step-up, where the graph steps up only after stopping against it (now free running
jumps onto it, and the step-up starts from the stop); the standing jump skips `xx_h_impultionstraight_footl`, a 1 s held
crouch the code leaves when the jump fires (kept, the jump fires at once); a running jump's landing on a top is picked
by the code (`LandingType`; in the data only `passover ... to_air` clips lead to it), so it stays listed. The
`graph_next` example prints a clip's place, successors and predecessors
(`cargo run -p forge --example graph_next -- <game> <clip>`); `action_keys` lists the actions the code requests.

The code's requests: 34 actions of `Game Fix`'s blocks have a key other than their id, the numbers the code asks for
(`action_keys` example): stands 0x30-0x35 (`xx_<l|h>_wait_hipm_<footl|footm|footr>`), stops 0x56/0x57 and their exits
0x5a-0x5d, the run turn-around 0x5e/0x5f and its exits 0x60/0x61/0x68/0x69, jumps and falls 0x1e/0x1f/0x21 and
landings into a walk or run 0x2e/0x2f, the wall run's steps 0x44-0x47, and a few others. The exe does not hold these
as plain constants; they are computed from the state machine's enums (the stands are 0x30 + division x 3 + foot:
`WaitLow`/`WaitHigh` by left, middle, right). Those enums are in the exe's reflection tables (a 12-byte entry: name
hash, name pointer, value), so AC1's player state machine can be read by name (`.rdata` around 0x1991000-0x1998000
in the Steam DX9 build):
- ground: `Movement`, `Fight`, `FreeRun`, `OrientedMove`, `Hurt`, `ObstacleCollision`; movement divisions
  `WaitLow`, `WaitHigh`, `WalkVerySlow`, `Walk`, `Jog`, `Run`, `Crouch`; `ObstacleLeanType` `Hands` / `Feet` (the lean
  on a tall wall and the stop against a low obstacle);
- narrow objects: `Movement`, `Beam`, `BeamEntry`, `BeamReception`, `PilotisReception`, `Edge` (`Front`, `Right`,
  `Left`), `FreeRun` (`EntryB`, `EntrySide`), `Lean` (`FaceLeft`, `FaceRight`), `CrowdRun`, `ObstacleCollision`; a
  post is reached from a free step, from the air or from a jump start (`PilotisEntryType`);
- ledge: `Entry`, `Movement`, `TurnCorner`, `HandPlacement`, `Pullup`, `ReboundTransition`, receptions into a wall
  hang, a free hang and a swing, `TransitionInFromClimb`/`FromLadder`, `PullDown` (`Orientation`, `Descent`,
  `Reception`, `ReleaseToInAir`; types `Ground_Soft_EdgeStop`, `Ground_Soft_Wait`, `Ground_Hard`, `Beam_Soft`,
  `HandPassOver`), `HandPassOver` (`Entry`, `PassOver`), `Grasp`, `ParallelJump`; `NextHand` left / right;
- climb: entries `FromGround`, `FromAirStraight`, `FromAirInclined`, `FromWalling`, `FromClimb`; divisions
  `WaitLow`, `WaitHigh`, `ClimbLow`, `ClimbHigh`; `Inclination` vertical / horizontal; ladder: `MvtAnimState`
  (waits, climbs up and down, `Revolve`, entries from the ground and from the top, exits);
- in the air: `FallOrigin` (ground, climb, wall hang, free hang), `JumpType` (`Straight`, `1m`, `3m5m`), `LandingType`
  (`Safe`, `SmallDamage`, `HeavyDamage`, `Fatal`);
- walling: `EntryA`, `EntryB`, `Vertical`, `Horizontal`, `ReboundTransition`, `VerticalEnd`, `WallStep`; riding:
  `ClimbUp`, `Riding`, `ClimbDown`.
Next: map each requested key to the state and division that asks for it, and drive the ground state from that.

The runtime mixes clips the same way (`forge::anim::mix`: matched by normalized time, the mix lasting the weighted
duration; `AnimLib` bakes `mix:<clip>=<w>,...` names on first use): locomotion blends neighbouring gaits and banks
(the slow walk, authored in place, gets its speed from its stride); stops blend by speed; leans by wall height; wall
runs and standing jumps onto a top between two variants' heights mix the two (a 1.75 m top: half the 150 cm jump and
half the 200 cm one; a wall run there 0.36 of the 131 cm entry, 0.64 of the 200 cm one); leaps mix their 2 m and
3 m variants toward the hold when that lands nearer. Test blocks `Jump 1.75 m (blend)` and `Jump 2.25 m (blend)`.

## Collision shapes (`MeshShape`)
Entities with an `InertComponent` and a `RigidBody` link a shape, placed by the entity's world transform. `MeshShape`
(format in `crates/forge/src/shape.rs`): vertices, a Havok MOPP tree, triangles, per-triangle materials, collision
materials, filter info, bounds. The `shapes` tool parses all 14889 (6.5 M triangles). A city's collision now comes
from these where an entity has one (Damascus: 3.6 M of its 4.6 M triangles, from 13797 entities; they line up with
the buildings, `AC1_SHOW_COLLISION` draws them); other solid meshes still collide by their render triangles
(`AC1_COLLISION=render` for the old behaviour). Masyaf's village collision is in the Kingdom forge (not loaded).

Primitive shapes (`shape.rs`): a box (half extents and convex radius, a local transform, a material), a convex
hull ("barrel": a local transform, face planes, vertices), a capsule (two ends, a radius), a list of inline shapes.
Props (market goods, vases, crates, swords) use them (the test world's prop zone, east of the course, places
some of Damascus's with `city::spawn_props`; `AC1_NO_PROPS` leaves it out, scripted runs too unless `AC1_PROPS` is set); they become collision triangles (a hull's faces from the
vertices on each plane, a capsule as a prism) and replace those props' render triangles.

## Ragdolls and Havok packfiles
`RagdollNew` holds a Havok 4.6.1 binary packfile (`crates/forge/src/hkx.rs`: sections, pointer fixups, its own
`hkClass` type definitions, read generically by member name). `human_ragdoll`: 19 capsule bodies (masses 3 to 15 kg)
on the bones of the same names, 18 joints (ragdoll joints with twist, cone and plane limits; knees and elbows hinges,
0.17 to 1.4 rad). The `ragdolls` tool reads all 72. `ik::ragdoll` is a Verlet ragdoll: particles at the bones (AC1's
radii and masses), tips for hands, feet and head, bone lengths kept, pelvis and chest braced, AC1's hinge ranges and
the spine's cone held as distance ranges (law of cosines, momentum-free), collision through the level; bones are
rebuilt from the particles (a bone with several children turns by a frame on two of them). A body whose death clip
is over goes limp and settles; `AC1_LIMP=secs` drops the player for testing.

## Cloth
Altaïr's entity has a `ClothComponent`: `UCMA_Altair_Cloth` (his robe tails) as a soft body colliding with
`humanragdoll_forcloth`'s capsules. `ik::cloth` runs it over the skinned mesh: the vertices as Verlet particles,
held to their edge lengths and across neighbouring triangles, pinned at the waist and kept within reach of where the
animation puts them (more toward the hem), pushed out of capsules round the hips and legs; drawn from a world-space
copy (`robe.rs`). `AC1_NO_CLOTH` keeps the skinned mesh.

## NPCs (`EntityBuilder`, `BuildTable`)
A builder names a base entity and tables; a table is weighted rows whose cells set a column: a mesh (through its
`LODSelector`), a texture, a nested table, an add-on skeleton (hood, ponytail, a head's face rig) or a value (the
`Size_*` tables' height scales, 0.92 to 1.03). Mission data blocks carry builders with what they use (Damascus
`MB02`: peasants, women, harassers, vigilantes, scholars, merchants, militia, elite guards); base entities link
universal parts in `DataPC_Common.forge`, found through `forge::index` (every object of a forge by id, built once and
cached in the temp directory; `idx` tool). `npc.rs` picks a row per table by weight (seeded), so each NPC differs.
The blend crowd is drawn from the hooded `CNMA_Scholar` builder; a
line-up of every type stands south of the flow lane (`AC1_NPCS` in scripted runs).

## Viewpoints
Cities place viewpoint towers and spires as entity groups named `ReachHighPoint_*` (and one entity with a
`ReachHighPointComponent`); the sync spot is the highest beam or post end on it. Damascus has 12. Eagle Vision (Q)
marks them with gold columns. A viewpoint's entity sits partway up its tower, so the spot is lifted to the top above it.
Standing on one, the legs do the leap of faith to its nearest hay whichever way Altaïr faces (up to 12.8 m out; AC1
builds each viewpoint over its hay). The high dive (`..._3000cm_down`) is used only where nothing is under its 8 m
takeoff drop, else the low dive, its flight carrying it clear of the tower.

## Authored guidance (AC1's climbing markup)
A climbable entity carries a `GuidanceSystem` (55af1c3e) inside its body: the edges the player may grab (format in
`crates/forge/src/guidance.rs`, found through Banned445's AC1-Movement-Rewritten). Every one parses
(`examples/guidancecheck`): Damascus 8023/8023 (151,456 enabled edges: 150,915 ledge grabs, 317 ladders, 356 poles),
Masyaf 963/963 (13,217), Acre 7018/7018 (138,980), Jerusalem 6493/6493 (105,908), Kingdom 5374/5374 (26,836). No beam
or kiosk edges are authored in the cities.
- A ladder is a vertical edge from its foot to its top (4.5 m for `Ladder_4m`), its horizontal normal out from the wall.
- A pole is two horizontal edges along it, one per side: one swing bar.
- A ledge grab's two normals are its top's (up) and its wall's (out).
Cities use them (`Level::use_authored`): the ledge grabs (sloping at most 0.35) are the holds, instead of the edges
and lips found in the geometry; the ladders and horizontal poles replace the ones guessed from mesh names. Damascus:
174,677 holds, 333 ladders, 183 swing poles. `AC1_GEOMETRY_HOLDS` brings the geometry's holds back. A top-out also
needs room to stand (1.7 m of headroom 0.5 m in, no wall just past the edge, probed from in front of the wall, and the
spot not inside a solid): window sills are holds whose top is a window, not a floor to climb onto, and a beam stuck into
a building shows its top inside the wall. Narrow wall tops (a fence's or a parapet's, 0.15-0.6 m deep behind a hold
with a drop past it, open above) become perches along their middle (`Level::add_narrow_tops`, Damascus 10,557):
climbing onto one ends crouched, balancing on it, and free running carries on along or off it.

## City parkour objects
Hay, benches and the ladders and poles of cities without authored ones have no data of their own: they are found by mesh name when a city is placed, and
set up against the built collision (`Level::add_city_objects`), logged as "N haystacks, N ladders, ...":
- hay: `Hay_Bale_Charette*`, `Hay_Bale_Chariot*`, `Hay_Bale_01` (not the straw strewn on the ground, `Hay_Bale_Tile*`).
  Their meshes use a vertex format not decoded yet, so a cart's stack stands where its entity is (1.8 x 1.8 m, 1.8 m
  high); a cart's parts merge into one stack.
- ladders `Ladder_<h>m`: thin and over 2 m tall. The building is the side with a roof just under its top (the ladder
  leans on that wall, its rails standing up past the roof edge, so its top is that roof's floor); else the nearer wall.
  The foot stands toward the outer side of its bounds. Climbed from the street onto a roof and back down in Damascus.
  A ladder's foot or top is taken before the edge it stands at.
- benches `Banc_*`: sat on facing away from the wall behind them.
- poles `Pole_*` lying down (stuck out of a wall, or across a gap): swing bars along their length.
Skinned props (hay carts, market stalls) are drawn in their bind pose; `AC1_POSE_PROPS` stands them in their own
skeleton's pose instead (24 bones at most), which so far changes nothing visible on the carts.

The climbing probe (`AC1_PROBE_CLIMB=n`) takes the n holds nearest the start that hang 1.6-2.6 m over the ground, grabs
each and holds up for 10 s, and logs how far he got, where the wall's top is and which holds go up it. Damascus: 8 of 20
over the top, 1.9 m up on average. The rest stop where the next hold is more than a climbing jump above (1.7 m, then 8 m).

## City notes
- A city forge (e.g. `DataPC_Masyaf.forge`: 785 data files) has a world file (`Masyaf`, 14000 `BAO_0x...` objects of
  class d8295dcb = `SoundBao`: sound banks, not geometry) and `Cell*_DataBlock` cells holding entities, meshes, materials and
  textures.
- Entity body: starts with its world transform, a column-major 4x4 (Z up, metres; Masyaf's village is around
  (40, -80, 64)), then references by object id: the meshes it shows (full detail and `_LOD_0n`), often stored in
  another data file of the forge, and `(placeholder, real)` material pairs (adjacent ids). 2429 of Masyaf's 5765
  entities place meshes (houses like `House_3x6x4_01a` x25, walls, rocks, trees, props, terrain tiles at identity).
- Materials reach their diffuse map through material -> texture set -> map spec named `*Diffuse*` -> texture; layered
  ground materials (`*_Multi_*`) have no such spec (the first map spec with a texture is used). City UVs tile past 0..1.
- Entity groups (class 3f742d26; Damascus has 1805: souks, balconies, scaffolds, bridges, market stalls, rooftop
  gardens, chimneys, lanterns, puddles) start with the group's world transform and embed their member entities inline:
  `[u32 id][u32 0984415e]` then the member's body (its world transform, then its references) up to the next member.
  The members are not in the data file's object table.
- Beams: every `WoodBeam_*` (and `WoodBeam_Fixture`) instance's top line is kept where it is out in the open (headroom
  above, the beam's top the first thing below, so not the part inside the wall it is stuck into): those stretches are
  perches to balance on, walk along and jump between (Damascus: 1336 from 937 beams); a stub sticking out of a wall less
  than 0.8 m also gets a hand hold across its end (67). A stretch inside a solid (`Level::inside_solid`: three of four
  level rays meet faces from behind; AC1's shapes are closed and wound outward) is not a perch, even where the beam's
  own top shows inside a building's wall (Damascus: 1141 perches). Pegs modelled into the house meshes themselves are not found yet.
- Shared props: city entities also refer by id to meshes and materials stored in `DataPC_Common.forge` (bushes,
  palms, souk windows, wooden beams, lanterns, ladders, hiding-spot tarps, columns; Damascus uses 240 of them, 24836
  references). Only the data files holding those, and their materials, texture sets, maps and textures, are read
  (Damascus 251, Masyaf 90).
- Left out: `_LOD_*` meshes, ground clutter, dead bodies of a mission's scene, `OutOfBound*`/`OOB_*` mission walls,
  `GP_MARK_*` gameplay map markers (big floating letters), `PositionHelper`s and `PillarDust` light shafts. Water
  surfaces (`*Water*`, `*Lake*`, `*Puddle*`: AC1 shades them itself) get a translucent material and no collision.
- Holds come from AC1's own climbing markup (see "Authored guidance"); without it (or with `AC1_GEOMETRY_HOLDS`) the
  runtime makes them from building edges (a walkable top meeting a wall below).

## Animation notes
See the header of `crates/forge/src/anim.rs`. In short: per-track blocks
`u8 type, u32 size, u32 n, n-1 frame keys (u8, u16 if bit 0), n values` at 60 fps. Rotations are
smallest-three quaternions in 16/24/32/48 bits (or 3 floats whose low mantissa bits hold the index), translations are millimetres (packed 11/11/10
bits or i16x3). Hash 0 tracks are root motion; clips face +Y with the Reference bone turned 90 degrees.

## License
MIT, see `LICENSE`. This repository contains no game files: you need your own copy of Assassin's Creed.
