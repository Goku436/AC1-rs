# ac1-rs: technical notes

A from-scratch Rust runtime for Assassin's Creed (2007, PC) that reads the game data from **your own
install**. No game files, textures, meshes or code from the original are stored in this repo; `out/`
(extracted data) is git-ignored and must never be published.

## Status
| Layer | State |
|---|---|
| `.forge` archives (index, names, FILEDATA records) | done, all 18 archives |
| Chunked LZO1X decompression | done: 15906/15906 data files decompress (`examples/decodecheck`); 9357 split into objects, the rest are streamed sound (6531 `*_BAO_*`: Ubisoft BAO audio, not object containers; see "Sound") and each forge's `GlobalMetaFile` (18: one 170-byte record, byte-identical in every forge: small ids and values and a 32-bit time of 2014-07-22, likely the build's; nothing about the world, `examples/entryhex`) |
| Object container (class hash, name, id, body) | done |
| TextureMap -> PNG (DXT1/3/5, BGRA8) | done, verified visually: 32286/32286 textures decode (`examples/texcheck`); colour space (`GammaSettings`) and sampler (address, filter modes) read and used; cube maps 57/58, ramps (`TextureGradient`) 3/3. Stored levels as they are (`texture_levels`): 32251 block-compressed (27620 DXT1, 1878 DXT3, 2753 DXT5), every one in whole 4x4 blocks at its top level, 30640 with the full mip chain; city textures go to the GPU that way |
| Material (85c817c3): template and typed parameter table (scalars, colours, texture slots) | 15875/15929 parse (`examples/texcheck`; 15666 before the `UVTransform` parameters were read, 2026-10-12): every character and city material; the rest carry a 1 at +0x29 (28 trees), 2/4 at +0x31 (25) or a texture slot mode 3 (1) |
| Mesh (class 415d9568): skinned verts, submeshes, bone palettes, materials | done, 120 skinned (incl. cloth, type 4) + 31 static of 168 in DataPC |
| Static meshes (type 0, stride 24) | done: positions `xyz/32768 * |w|/8` m (sword, terrain tiles meeting exactly, houses on their ground); every mesh in every archive parses (43984/43984): vegetation and soft bodies (`Veg_Olivier_*`, `Herbe_Scatter_*`, `*_Softbody`: the cloth-simulation object 0x5755de7f embedded as in the robe, after `u32 0 or 1, u32 1 or 5, u8 0`; static LODs with an empty bone table) and effects meshes (type 2, laid out as the static ones); over 65535 vertices the indices are u32 (6 meshes, all the distant city's merged stand-ins, `Damascus_FakeMesh_26` ...) |
| Entity (0984415e): meshes, skeletons, material overrides | done for characters |
| City placement: entities' world transforms and meshes across a forge's data files, EntityGroup (3f742d26) members | done: Masyaf 4344 meshes placed (926 from groups; 2221 entities collide by their shapes, 5 data files of them looked up in `DataPC_Kingdom.forge`), Damascus 17581 (8851 entities from groups, 15.9 M collision triangles), shared props from `DataPC_Common.forge` |
| Characters assembled from shared parts across data files (soldiers, crowds) | done: AC1's NPC builders (`EntityBuilder`/`BuildTable`), links into `DataPC_Common.forge` through an object index; the NPC line-up and scholars use them |
| Move graph (`ActionKit`/`ActionBlock`) | done: 452/452 blocks parse (15463 actions, 15958 items); blend spaces used for locomotion, stops, leans, grabs, leaps |
| Collision shapes (`MeshShape`, `BoxShape`, `BarrelShape`, `CapsuleShape`, `ListShape`) and rigid bodies (`RigidBody`, its collision layer) | done: 14889/14889 meshes (6.5 M triangles) and 2098/2098 primitives parse; cities collide as AC1: by the entities' rigid bodies where their layer stops the player (Damascus: 16000 bodies, 289 left out: camera, horse and projectile barriers, out-of-bounds walls; 3.76 M triangles; only an entity's first body, a second is a broken state), entities with none collide with nothing but terrain; F11 draws it, `AC1_COLLISION_SWEEP` measures it ("Collision shapes") |
| Levels of detail (`LODSelector`, `forge::lod`) | done: 18807/18807 selectors parse (`examples/lodcheck`); cities draw each mesh at AC1's level for its distance |
| World grid (`World`'s `GridLayout`, `GridPartition`) and distant stand-ins (`FakeEntities`, `forge::world`) | done: 12/13 worlds' grids read (`Map_Menu`'s layout is empty), 9/10 stand-in sets check (every span inside its submesh and on a cell with a data file; `Flashback_Paradise`: one span past its submesh); cities stream their drawn meshes by these cells |
| Havok packfiles (`HavokChunk`), ragdolls (`RagdollNew`) | done: 72/72 ragdolls read; the dead go limp (Verlet ragdoll with AC1's bodies and limits) |
| Cloth (`ClothComponent`, Altaïr's robe tails) | done on AC1's data (`forge::cloth`, `ik::cloth`): the soft mesh's 146 vertices (44 pinned) and 253 edges, per-vertex pull and collision radius, the entity's `Cloth` settings, the 6 `humanragdoll_forcloth` capsules (thighs, shins, feet) mapped onto bones, the 20 `ClothActionSettings`, AC1's 0.033333 s step, the city's `Wind` |
| Bone modifiers (secondary motion: hinges, look-ats, compressions, rotation copies, roll bones, spring box) | done (`forge::modifier`, `ik::secondary`): all 56 of Altaïr's (main 31, skirt 20, hood 3, sword tag 2) parse and run every frame after the IK |
| Wind (`Wind`, a city's `Wind_<City>_001`) | done for Damascus and Masyaf (the test level borrows Masyaf's); AC1's noise tables, checked against a value read from the running game |
| Viewpoints (`ReachHighPoint_*` groups, entities named `*ReachHighPoint*`) | done: Damascus 12, each with a leap of faith into its hay; Masyaf 1 (the tutorial tower, its hay not found: 19.8 m to the nearest) |
| City parkour objects by mesh name (hay carts, `Ladder_*`, `Banc_*`, `Pole_*`) | done: Damascus 46 haystacks, 301 ladders, 320 benches, 159 swing bars; Masyaf 14, 40, 28, 2; climbing probe 8/20 spots over the top |
| Navigation meshes (`NavMeshManager`) | done: 1432/1432 parse (67836 pieces, 1.13 M triangles); city routes (A*) |
| Clip sets (`AnimSet`) | done: 81 read; soldiers and townsfolk play their own clips |
| Flight recorder (F9), controller, scenario suite | done |
| In-game menus: graphics settings (F8, presets for low-end PCs, saved beside the exe), online (F10: two players over UDP, host / join, replays) | first version ("Graphics settings and online play") |
| Ghost mode, AC1's development flight (the Human Debug context, unreachable in retail; F7) | done: no collision, action 0x0006F88B (request 7) plays `Ghost_mode01` then `Ghost_mode02` looping, out with request 33's `xx_h_jump_falling01` and a fall (from Banned445's MIT port, `debug_fly.rs`) |
| Skeleton (24aecb7c) + bone names (CRC32) | done, 50/50 in DataPC |
| Animation (0fa3067f): tracks, 16/24/32/48/96-bit quats, packed translations, camera field of view | done, 22228/22228 clips parse (the 16 `vo_*` voice lines have no tracks); the 64-bit quat (104 tracks, all present-day Lucy's) not decoded |
| Animation event tracks (sounds, surface contacts) | 21670/22228 clips' events parse (`examples/animcheck`, 20523 keys); the rest hold an `FXEvent` (552, assassinations) or class a9264210 (6), whose fields are not known |
| Sound (`SoundBao` d8295dcb, streamed BAO files, `forge::sound`, the `bao` tool) | 221707/221707 `SoundBao` objects and 6531/6531 streamed files parse; every distinct resource and stream with a header decodes (`bao scan`): IMA-ADPCM 38211/38211 resources and 2176/2176 streams, interleaved tracks (IMA mono or stereo, or Ogg Vorbis pages) 29/29 and 76/76, Ogg Vorbis 13743/13743 and 110/110 (6636 of those Ogg resources are a stream's start, cut); the 4169 headerless streams (speech) decode joined to their start for 3716, the other 453 have no start in any forge. `ContactTable` read (Altaïr's 24 surface groups). Played in game (`sound.rs`): the player's clip events (steps, starts and stops, landings, rolls, grunts, catches, wall runs, ejects, haystacks, the leap of faith; the landing grunt AC1's code plays, `OnoJumpLandHeavy`) on the surface's sound material (cities: from `CollisionMaterial`) and AC1's switch values (character kind, weapon sheathed, city), placed and panned with AC1's roll-off; the city's placed sounds (Damascus: 1292 trigger zones: fountains, braziers, flags, kiosks, gardens, palms), its ambience loops (116 region links, 36 sets, 11 streamed loops) and their one-off sounds at AC1's intervals; M mutes, F8 sets the volume |
| Effects (`FX` 824a23ba, `ParticleSystem` caaa7ee8, `HumanFXPack`, `forge::fx`) | 12/12 particle systems of DataPC's player files parse to their end; the contact table's FX per surface read. Played (`particles`): the surface's dust or straw on contact keys (steps, landings, rolls), the haystack's straw going in and out, as camera-facing sprites of our own; the landing's camera shake (`shake`). Not walked: the FX graphs' operators |
| City levels (`AC1_LEVEL=masyaf`, `damascus`, any `DataPC_<City>.forge`): textured, with water, collision and holds found in the buildings' edges; free camera (P); lit as AC1 lights them ("How AC1 lights its world": the level's sun, ambient cube, baked occlusion, fog and colour grade from its `WorldAmbiance`/`LayeredSky`), three shadow cascades in 120 m (the first 12 m); drawn streamed: AC1's levels of detail, the grid's cells round the camera on loader threads, the stand-ins beyond, BC textures (`AC1_DETAIL`, "City rendering cost and streaming") | first version |
| Level light and grade (`WorldAmbiance` 21c2d472, `LayeredSky` 24d32cf8: fog, post effects, suns, ambient cubes, sky layers) | done: 14/14 levels read, and the occlusion mapping (`AOSettings` in each `World`) of 13/13 (`examples/ambcheck`) |
| Baked occlusion (`MeshInstanceData` / `CompiledMeshInstance`: a byte per vertex per placed mesh) | done: 17436/17436 Damascus entities and groups, every stream a byte per vertex of its mesh (`examples/aocheck`) |
| Bevy runtime: skinned Altaïr with mip-mapped textures, locomotion, IK, climbing/jumping, free running (stepping and jumping onto low obstacles, posts, beams, swing bars, ladders, kiosks); combat removed for now; scholar crowds to blend into | first version |
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
  Wall runs need the sprint (over 5.5 m/s, so about 0.4 s of free running from a run); free running
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
- F7 Ghost mode, AC1's developers' flight (the Human Debug context, `fly.rs`; behaviour from Banned445's MIT port, which
  reads it from the executable): nothing collides and no ground move, climb or IK runs; the stick on the ground plane by
  the camera and Space / Left Ctrl up / down, all × 5 m/s, × 3 in high profile (the right button), × 10 with Left Alt; the
  body turns toward the horizontal motion (square over 0.1) unless Q (strafe) is held. In, HumanGround's action
  0x0006F88B (request 7): `Ghost_mode01.man` (0.93 s), then `Ghost_mode02.man` (2.27 s) looping, 0.4 s and 0.3 s blends.
  F7 again leaves with request 33's `xx_h_jump_falling01.man` into the ordinary fall, from rest, or with Shift (the
  empty hand) held at the flight's velocity (our fall still caps its drift at 5 m/s). The four `.man` clips carry their
  root track turned 180 degrees from every other clip's (their bones are as the rest's): their root rotation is taken
  from their first frame. Scripts: `AC1_FLY=from-to`, `AC1_FLY_UP` / `AC1_FLY_DOWN=from-to`.

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
- letting go of the direction while running stops (AC1's run stop, the run and sprint stops mixed by the speed value,
  locked: steering does not break it off); in high profile reversing past 135 degrees plays the run stop, then the
  skid turn (actions 0x5e/0x5f, `xx_h_runturn180_<foot>`: the facing flips as it starts and the body comes round in
  the clip), then on into the jog or walk (0x60/0x61, `_tr_<jog|walk>_hipm_<foot>`), or, the stick let go, into its
  stand (0x68/0x69, `_tr_h_wait_hipm_<other foot>`); standing, steering more than 90 degrees off the
  facing turns on the spot, a quarter or a half turn to that side (`xx_<l|h>_wait_hipm_<foot>_to_<l|h>_waitturn_
  <left|right>_<090|180>_<foot>`: AC1 ships all four, in both profiles, from either foot), then into the walk (low
  profile) or jog (high), the quarter and half turns mixed by the angle, through the turn's own `_tr_<l_walk|h_jog>` exit
  (AC1's pivot item and its transition, audits GROUND-4, GROUND-5; what follows describes the older code). Formerly:
  only the turn itself played: AC1's move graph goes from it straight into the walk, jog, run or
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
  than 2 m below, halted at it in low profile for 0.25 s, looks down over it
  (`xx_l_ledge_lookdown_<front|left|right>_<foot>` mixed by the edge's side, as the idle);
- the empty hand at an edge with a hold along it (pressed within 0.2 s), or pushing through the ledge stop's start,
  pulls down onto it,
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
to Pole Q from either of its upper rows). Ejects (AC1's rebound, measured in the running game at the Damascus bureau's
wall through ac1-hook): hanging or climbing, high profile and the legs (in low profile nothing happens); the stick back
or none jumps straight away from the wall, to a side along it (holding the side first shimmies that way; the press
during the shimmy ejects). Each is a jump at the best place to land that way (a beam or top, 0.5 to 3.3 m off, from 0.7 m
below to 1.1 m over the start in the tests): `xx_h_hangwall_tr_rebound_<foot>` (0.2 s), a takeoff turned away from the
wall from AC1's rebound table (`xx_h_rebound_<frontleft|frontright|left|right>_<front|down|up>_<050|300|550>cm_<foot>
_to_air`, blended like the running takeoff's slots, the side groups turning the body a quarter round: the stick's left
as he faced the wall is the turned body's right, the `right` group), then the running jump's flight and reception
(logged by AC1 under its blend's first clips, `rebound_frontleft_front_050cm`, `air_front_050cm`). Ours does this from
the wall hang and the climb (`WallClimb::rebound_jump`, `crate::jump::rebound`; back and left land within 0.12 m of
AC1's; scenarios tagged `eject`), from a wall hang or a climb after AC1's 0.2 s rebound pose against the wall
(`xx_h_hangwall_tr_rebound_<foot>`, the takeoff on the same foot 0.22 s after it in all four recorded ejects; the eject
routes now stay within 0.45 m of the running game's all the way), and with nothing to land on, any way, flies at AC1's free-jump target, 8 m on and 3 m down (since
2026-10-10; before, it pushed off backwards only with the stick back). Targets are searched to 8 m, the tap jump's
reach (was 4.6 m). An eject's targets start 0.5 m off (running jumps' at 1 m): AC1's right eject took a beam stuck out
of the wall 0.95 m off, and so does ours now (route `wall1_eject_right` 3.34 -> 0.45 m). As AC1's interpreter checks
the jump off before any move (Banned445's port, verified live), high profile + legs ejects in the middle of a move from
hold to hold too (a hand step, a reach, a shimmy), cutting it; pressed during a leap between holds it is kept 0.3 s
(the legs buffer) and ejects as the hand lands. As AC1 (audit WALL-8), the root turns away from the wall in one frame as
the takeoff starts (it was eased round over 0.25 s), placed so the hips stay put, the pose faded from turned with it (the
rebound pose already faces away: the hips are 4 degrees off the takeoff's first frame). Not yet: the ladder's rebound (`xx_h_ladder_wait_<l|r>_tr_rebound`), the mid-air one (`xx_h_air_all_<foot>_tr_rebound`) and the wall
run's one-piece ejects (`xx_h_wallingfront_<entry|step1>rebound_<back|left|right>`). Wall runs and free
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
teeth showed through the chin and the mouth hung open). Characters
and city meshes use their materials' normal maps (the texture set's "Normal" map spec: tangent-space RGB with
DirectX's green, flipped for Bevy; characters with their stored tangents, cities' generated), so cloth folds and wood
and plaster relief show. Textures are sampled in their own colour space and with their own sampler (GammaSettings,
address and filter modes). The eyes are opaque: their diffuse alpha is the iris tint's mask (alpha-tested, it cut the
irises out).
Altaïr is drawn with AC1's character templates (`character_material`, an `ExtendedMaterial` over Bevy's
`StandardMaterial`; shader and parameter use from Banned445's AC1-Movement-Rewritten, MIT): each material's template
picks the equations (cloth, cloth inside, skin, weapon/mouth, eyes), its typed parameters and slots give the maps
(Material -> TextureSelector -> TextureSet channel / TextureMapSpec -> TextureMap): the diffuse lit through the
template's ramp (`TextureGradient` in DataPC's `Game Fix`) at (N.L+1)/2, Phong highlights from the specular map, cloth's
rim, skin's tiled specular and multiply map, Fresnel on weapons and the mouth, the eyes' iris tint (the diffuse alpha
masks `EyeColor`) and reflection cube. Cloth with an inside material (the robe, hood, flaps) is drawn twice: the
outside back-culled, the inside front-culled with the inside material. Ours reads the colour parameters as sRGB
(Banned445's as linear: the irises read near white). The light is the level's ("How AC1 lights its world"): the
character ambient cube, the characters' sun through the ramp, Bevy's shadow maps, AC1's fog and grade; NPCs (other
templates) keep `StandardMaterial`. `AC1_PLAIN_LOOK=1` draws Altaïr with `StandardMaterial` too.

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
against it. Running-jump takeoffs leave at 5 m/s at most. Ladders (block L, south of the course): walking into the foot (or out over the top) climbs on, W/S climb 0.5 m a step
(`xx_l_ladder_climb_<up|down>_<l|r>` between `xx_l_ladder_wait_l/r`), it climbs off at the top or steps off at the
bottom, Space at the top edge climbs down onto it (`xx_h_wait_hipm_footl_tr_ladder_pulldown`), Space on it lets go.
Hanging on a wall with a ladder beside, sideways goes across onto it (`xx_l_climbing_1m_tr_l_ladder_wait_*`,
`xx_h_hangwall_tr_l_ladder_*`, `xx_h_hangfree_tr_l_ladder_*`, the side leaps `xx_h_climb_{l|r}_{hand}_{2|3}_tr_ladder_*`):
the move whose hands end nearest its rails, the rest made up over it. Sideways on a ladder goes back onto the wall's
holds (`xx_h_ladder_wait_tr_{hangwall|hangfree}_{left|right}`), steered like a leap since its rungs aren't level with
the holds. E's ladder has holds to its right to try it.
Kiosks (`HumanKiosk`, audit NARROW-16): a market stall's counter (its `Kiosk` guidance pieces, 0.74 m up, along the
front and the two sides, in every `Kiosk_*` entity group: they had been missed, the guidance check reading only
top-level entities) is a jump target (type 0x400, its piece's middle, in the jump's own zone only). Across its front:
`xx_h_kiosk_front_freestep_footr_tr_monkeybar_<left|right>_footr`, one `xx_h_kiosk_front_monkeybar_<side>_footr` along
the counter and `_tr_fall` off its end; along the stall entity's +Y (within 10 degrees) onto a side,
`xx_h_kiosk_side_freestep_foot<l|r>` and a free-step jump on out (Banned445's port). Kiosk K south of block L is one.
(Ours had kiosks as overhead monkey bars jumped up to from the ground: removed.) On the ground the root's height follows the ground averaged 0.35 m
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
  `UCMA_Altair_Cloth_SoftBody` and `ClothActionSettings` `Altair_ClothActionSettings`; the robe here runs on them
  since 2026-10-12, see "Cloth"). Its AC1 file lists (`File Lists/AC1.gfl`, `Full File Lists/AC1.fgfl`: a 24-byte header, then Oodle Kraken, which the
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
- Defaults set by constructors: `HumanClimbData` `GridTileWidth` 0.75, `GridTileHeight` 0.6, four limb pulls 0.5, four
  0.1, an int 4; `HumanGroundData` 0.4, 1.5, (0, -0.4, 0.9). No `Human*Data` object ships in the data. Property names
  are recovered by CRC32 of the exe's words, and which fields the code reads is in docs/PARKOUR.md section 0 (the
  tuning ones are never read: the code uses constants).
- `LandingEvent` is made through an event factory (vtable 0x16f0424), so its sender is not found by references to it;
  the landing heights are still to find (from the in-air decision code, not the `HumanInAir` wrappers).

## Behaviour notes from the executable
Strings and MSVC RTTI names in `AssassinsCreed_Game.exe` show how the original splits these mechanics (names only,
no code):
- Wall run: `HumanWallingData` ("rebound distances"), sub-states `WallingEntryA`, `WallingEntryB`,
  `WallingVertical`, `WallingVerticalEnd`, `ReboundTransition`, `WallingHorizontal`; `WallingType`
  Vertical/Horizontal, `WallingSide` Left/Right. No horizontal walling clips ship in any archive (all 43
  walling clips are `wallingfront`): no side wall run (a procedural one was taken out, 2026-10-08).
- Ladder: in high profile AC1 climbs 1 m a step (`xx_h_ladder_climb_<up|down>_<l|r>`) and runs off the top into a free
  step; the legs with the stick pulled back rebound off it backwards (push-off, then `xx_h_rebound_foot<l|r>_tr_fall`
  as it comes down, still facing the wall, landing standing). All from `HumanLadderData`'s table.
- Haystack: `EntityDescriptorObject_HayStack`, `BhvHayStack(Data)`, `HumanHayStackData`, actor state
  `InHayStack`, entry types `Top`, `Ground`, `FreeStep`, `SideJump` (only Top is recreated).
- Leap of faith: actor state `LeapOfFaith`, `LeapOfFaithClip` resolved on jump start/end.
- Landing: `LandingType` Safe / SmallDamage / HeavyDamage / Fatal: the damage landing from 3 m; the player's damage is
  the flagged mode's fixed table (over 8 m 10, 10 m 20, 12 m 40, 14 m 80, 16 m 120, 18 m 160, fatal over 20 m, of 200);
  the 6.3 / 7 m limits of `HumanInAir`'s constructor 0xE0FE80 are the non-player default (audit AIR-1, AIR-18).
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
- TextureMap body: u32 w, h, depth, format (0 BGRA8, 2/3 DXT1, 4 DXT3, 5/7 DXT5); +0x10 u32 type (1 2D, 2 cube);
  +0x14 u32 `GammaSettings` (1 sRGB, 0 linear); +0x18 u32 mip count; +0x1C/+0x20 u32 `AddressMode` U/V (0 wrap,
  1 mirror, 2 clamp, 3 border, 4 mirror once); +0x24 `?`; +0x28/+0x2C/+0x30 u32 `FilterType` mag/min/mip (0 point,
  1 linear, 2 none); +0x43 tag 0x13237fe9, +0x47 u32 size, +0x4B full mip chain (a cube: each face's chain, face
  after face). Field names from Banned445's AC1-Movement-Rewritten; over every texture (`examples/texcheck`) diffuse
  maps are sRGB (14750 of 14784; the 32 linear ones are colour masks) and normal maps linear (11105), the ramps clamp
  U. Our runtime uploads each texture in its colour space with its sampler (`assets::to_image`).
- TextureGradient (245f5ca5, the characters' lighting ramps in DataPC's `Game Fix`): the TextureMap layout, BGRA8,
  one mip.
- Material body: +0 u32 template id (the shader template: Altaïr's cloth 433482562, alpha cloth 2055988768, cloth
  inside 439116860, skin 433482821, weapon / throwing dagger / mouth 3292696482 / 661746657 / 439311561, eyes
  326205072), +4 u32 texture set id, +0x08 the inline `Mask` field (u32 id 0, class df5d6c0e, body `?`), +0x17..+0x29
  the render flags, a byte each in the order of Material's reflected property list (the exe's descriptor at 0x01939CB8,
  names by CRC32): +0x17 `BlendMode` (0 everywhere), +0x19 `InvertAlpha`, +0x1A `AlphaTestValue` (0..255), +0x20
  `InvertCulling`, +0x25 `AlphaTestEnabled` (426 of 2181 in Common: leaves, hay cards, ivy, trellises, tarps; windows
  carry a value with the test off), +0x26 `TwoSided` (871), the rest `?`; cities draw with them (`examples/matflags`), +0x29 u32 `?` (0), +0x2D u32 inside material id (0 none),
  +0x31 u32 `?` (0/1), +0x35 u32 parameter count, then `{u32 name CRC32, u32 class, u32 type (kind in bits 16..22),
  value}` to the body's end: kind 10 a f32, kind 13 four f32 (a colour), kind 19 class `TextureSelector` a slot
  `{u32 ?, u32 class, u32 mode, u32 channel, u32 set, u32 map}` (mode 0 the material's set's map spec at `channel`,
  1 set `set`'s, 2 map `map`), kind 19 class `UVTransform` (c52e2125, 42 bytes) `{u32 ?, u32 class, f32 ScaleU,
  ScaleV, TranslationU, TranslationV, TranslationSpeedU, TranslationSpeedV, Rotation, RotationSpeed, u8
  AnimateTranslation, u8 AnimateRotation}` (names from the exe's property list of `UVTransform`; its shader setup makes
  rows `(cos r su, -sin r sv, tu)`, `(sin r su, cos r sv, tv)`, the translation advancing by speed x frame time and
  wrapped to -1..1: the water's panners). A TextureSet body is its map spec ids by channel; a TextureMapSpec ends with its
  TextureMap's id (`forge::material`; layout from Banned445's repo).

## Mesh / skeleton notes
- Mesh body: u8 1 (0 = static), 7 B, u32 bone count, bones x 0x4C {u32 id, u32 class, u32 name CRC32,
  f32[16] inverse bind, row-major}. Then the compiled mesh (class fc9e1595): stride, vertex/index byte
  counts, 2 submesh tables, vertices, u16 indices; bone palettes, one per submesh: u8 3, u8 flag (1 on the first,
  0 on the rest; requiring 1 everywhere lost 525 meshes, e.g. hay carts, seagulls, townsfolk), u8 n, u8 submesh
  index, u8 `?`, u16 the submesh's vertex count, u8[n] bones; trailer with a material id per submesh.
- Meshes decoded per forge (`examples/meshcheck`): DataPC 151, Damascus 9180, Masyaf 3905, Acre 10759,
  Jerusalem 9466, Kingdom 4588, Common 1840. The rest: cloth meshes not of Altaïr's kind, and foliage/flags whose
  header reads as a huge bone count (another mesh type).
- The skinned hay meshes (`Hay_Bale_01`, `Hay_Bale_Charette_01A`: type 0 with 5 bones, stride 32) measure 6.1 x 4.7 m at
  the skinned scale (1/2048) and their collision 3.5 x 3.0 m: the outer cards are mostly cut away by the alpha test, and
  drawn with it the heap matches its collision and the cart (2026-10-12 shots); the skeleton `Hay_Bale_skul` has unit
  bones at the heap's corners (+-1.2, +-0.8, 0.79 m). No other factor found.
- What a city places, by mesh kind (`forge::mesh::mesh_kind`; `AC1_PROP_STATS=1` lists them), Damascus: 17826 static
  placements drawn (958 meshes), 11 skinned drawn in bind pose, 490 skinned not drawn (380 pigeons, the rest NPC body
  parts placed for cutscenes: no prop skeleton), 43 vegetation with soft bodies drawn (`Veg_Olivier_*`, `Herbe_Scatter_*`),
  299 cloth drawn in bind pose (rooftop tarps, flags), 144 left out by name (map markers, helpers). Puddles are
  submeshes of bushes and roofs with a water material (normal and cube maps only): drawn as AC1's water ("City materials").
- Cloth mesh (first byte 4, e.g. Altaïr's long robe): u32 4, u32 1, u8 0, an embedded cloth-sim object (class
  0x5755de7f, `SubMesh`: the soft body's own vertices and constraints, read by `forge::cloth`), then the usual bone count, bones and compiled mesh (found by walking back from the compiled tag).
- Static mesh (first byte 0): no bones, no palettes, stride-24 vertices: i16x4 position, u8x4 normal, u8x4 tangent,
  the vertex colour (a D3DCOLOR, bytes B G R A: not a binormal), i16x2 uv. The position's w is a scale shared by the
  mesh (sign varies): positions are `xyz / 32768 * |w| / 8` m, so every mesh uses the full i16 range (sword |w| = 8, a
  Masyaf terrain tile 3976, the tiles' edges coinciding); its sign is the tangent frame's handedness (the world's vertex
  shaders take the binormal as normal x tangent times it). The normal's and the tangent's fourth bytes / 255 are the
  layered materials' blend weights (the shaders' `COLOR1`: the first layer's, the second's). Damascus (`examples/vtxw`):
  7.39 M of 8.26 M static vertices white, 0.44 M grey, 0.43 M coloured (the stand-ins' baked tints, map markers, the
  cypresses' (255, 185, 0) that their template ignores), 0.20 M with alpha under 250 (puddle and river rims, moss
  patches), 3.05 M with a layer weight. Proof of the weights' order: the materials with no second layer
  (`DAM_Roof_01A`, `DAM_Stairs_Multi_01A`, `DAM_Marbre_Multi_01A`) have every tangent byte 0, the one with no first layer
  (`DAM_Floor_Multi_Rich_01A`) every normal byte 0.
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
- Skinned vertex (32 B): i16x4 pos (1/2048 m), 3x u8x4 normal/tangent/binormal (the tangent frame the normal maps were
  baked in: the tangent follows +u, the binormal mostly -v, as Bevy's generated ones; characters use them, with the
  handedness from the binormal's side; `examples/tangentcheck`. City meshes mix both sides of v and keep generated ones), i16x2 uv (1/2048, v as images are stored, repeating),
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
183 classes in the install get a name. Not parsed yet: `GridCellDataBlock` (a grid cell's own record; the grid itself is read, see "City rendering cost and streaming"),
`BodyPartMapping`. (`ContactTable` is read, see "Sound".)

## Sound
Read by `forge::sound` (layouts in its header), measured with `bao scan <game dir>`; `bao wav` writes a sound to
`out/sound/`, `bao clip <game dir> <clip>` lists a clip's event keys and what they play. Found with Banned445's
AC1-Movement-Rewritten (MIT), which first played AC1's sounds.
- Sounds are BAOs (Ubisoft's DARE engine): events (what the game triggers), sounds (gains, random containers) and
  resources (the samples) by BAO id, the top nibble the kind. `SoundBao` objects hold one each (speech one a language);
  the streamed files hold long sounds (`Common_BAO_*`: ambience and music; `<language>_BAO_*`: speech, whose start is
  a resource with the same low bits).
- Events: play (a sound, with a random pitch range), switch (by a variable: 0 is the surface's sound material), multi
  (several at once). Sounds: one resource (rate, channels, frames), or a random container (weights, the last pick not
  repeated). Resources: IMA-ADPCM (Ubisoft's header: PCM frames first, a mono last sample apart; some headers carry a
  time after the version byte), Ogg Vorbis (decoded with lewton), or interleaved tracks: IMA (the ambience: four mono
  tracks; 5 files of two stereo tracks) or Ogg Vorbis (music: two mono 48 kHz Vorbis streams, each track's first
  0x1400 bytes in its header, the rest of its pages split over the blocks). A headerless stream is the rest of the
  resource with the same low bits: the bytes carry on from the resource's last (`sound::join_stream`).
- What plays when is in the clips: their event tracks hold `AudioEvent`s (a `SoundBao` by object id: grunts, cloth,
  hand slaps) and `ContactEvent`s (`ContactEventTypeHuman`: `ft_walk_cycle`, `ft_run_cycle`, `ft_land_heavy`, ...)
  that the character's `ContactTable` (`ContactTable_AlTair` in "Rank 9": 24 surface groups of 15) turns into events.
  Keys are in 1/120 s.
- In game (`crates/game/src/sound.rs`): each frame the player's clips (`animation::Playing`: the heaviest of a blend,
  a legs-only or one-shot clip over it, the climbing move) fire the keys they passed; a gait's next step carries the
  phase over, a loop wraps, a move played again starts over. Each resource is decoded once and played with the
  event's gain and pitch, placed (below). M mutes, `AC1_MUTE=1` starts muted, `AC1_VOLUME=dB` and the graphics menu's
  (F8) volume row set the level; headless runs have no audio plugin (no device needed).
- Switches: the engine asks the sound object's type for a variable's value (the type table at `[0x1A16DD8]`, 0xBC a
  type, +0x10 the getter; the game's types are registered at 0xB475A0, their getter jumps to the sound manager's
  +0x568, set at 0xAE9D70 to the code at 0xB48840, a jump table over variables 0-18). What it returns: 0 the surface
  (the `CharacterController`'s material; 21 in the Present_Room world); 1 the character kind from the entity's kind
  bits (+0xA8: kind 1 sub-kind 0 is 0, sub-kind 1 is 3; kind 2's sub-kinds give 1, 2, 4, 5, ...); 2 `BhvHorse` +0x200;
  3 a `CharacterBehavior` module's bool (module 8, whose other calls are can-draw / draw, can-sheathe / sheathe: read
  as "weapon drawn"); 4 `BhvWeapon` +0x68 (the weapon kind); 5 a count over 1; 7 the world's index (Acre 0, Arsuf 1,
  LoadingRoom2 2, Assassination_Talal 3, Damascus 4, Jerusalem 5, Kingdom 6, Map_Menu 7, Masyaf 8, Present_Room 9); 18
  the entity's bits 11-13 (voices). Ours: 0 the surface, 1 = 0 (the player; in the footsteps cases 0 and 1 carry
  the gear layer, the default 2 is a townsman's, 3-5 plain), 3 = 0 (sheathed: a scabbard and gear rattle under each
  step, case 1 drops it), 7 the city. `bao tree` shows an event's switches.
- Placed sounds: events of type 1 and 6 hold a roll-off at +0x20 (type 6: +0x1C, after a gain at +0x18): DARE's
  `_RollOffParam` (saturation, clamp, fade start, fade end), read as `SND_fn_fDistanceToVolumeEx` at 0x7EA920 reads it
  (full volume to the saturation distance, then saturation / distance, a straight line to nothing from the fade start
  to its end; -2 takes the default's, the project BAO's 0x40000000 +0x13C: 2.5, -1, 10, 20). Ours: every sound but the
  ambience loops placed and panned from the camera (AC1's `SoundMicro::Mode` has FollowCamera, PositionToOrigin,
  FollowEntity; which one plays is not checked), Altaïr's at his hips. Sound type 2 is layers (+0x30 a count, sound
  ids from +0x84, all played: the palms' and awnings' rustles); a single sound's +0x38 = 1 loops it.
- `GlobalSoundSet` ("Game Bootstrap Settings"): the sounds AC1's code plays, 64 `SoundInstance`s in the order of the
  exe's property list (three `ProjectileSoundSet`s at 31-33); names found by CRC32 of guesses: 14 `SoundBodyFall`, 15
  `SoundJumpStart`, 23 `SoundClimbGrab` (empty), 25 `SoundLockOn`, 26 `SoundUnlock`, 28 `SoundPrayStart`; 33-54 are
  menu-like events (104d00xx, not named). `HumanSoundSet_Altair`'s are vocal `Ono*` (`OnoEffortLight`, `OnoGrab`,
  `OnoDodge`, ...), combat ones.
- Surfaces: a `MeshShape`'s triangles name `CollisionMaterial`s, whose body +0 is the sound material (0-24: Stone_Clean
  4, Marble 6, Wood_Plank 9, Grass_Short 14, Water_Shallow 19, Metal_Grill 24; `Default` 4001, none). Cities keep it per
  triangle (`Level::tri_sound`); the footsteps' switch 0 is the material under the player (`Level::sound_material_at`).
- `HumanSoundSet_Altair` ("Rank 9", class 2b315bea): Altaïr's vocal sounds, played by AC1's code, the same
  `SoundInstance` list (`forge::sound::global_sound_set`); slots by the exe's property order: 12 `OnoGrab`, 16 `OnoDodge`,
  23 `OnoJumpLandLight` (empty), 25 `OnoJumpLandHeavy` (10b102ff: one of ten grunts, -12 dB), 30 `OnoClimbPullUp`, 31
  `OnoBodyFallHurt` (empty), 32-34 `OnoEffortLight` / `Medium` / `Heavy`. At run time the set is 20-byte instances from +8
  (slot k at +8 + 20 k; the calls of the play helper 0x5269B0 pass these offsets): `HumanInAir__SetupToGround_Landing`
  (0xE05940) plays +0x1FC (25) on a landing from over 3 m, +0x1D4 (23) on the rest. Ours plays them on each ground
  landing (`Character::landings`); the landing and roll clips' own keys (`ft_land_heavy`, `ft_body_roll`, `ft_land_hands`,
  the roll's grunt) were already played (checked 2026-10-13: `RUST_LOG=ac1::sound=debug` lists every key fired).
- Not in AC1's clip data: the free jump's take-off (`xx_h_impultionstraight_*_to_jumpstraight_clear_*`,
  `xx_h_jumpstraight_clear_*` have no keys); it plays `GlobalSoundSet`'s `SoundJumpStart` (103205b7, the event the
  beam take-off clip and Altaïr's `ft_jump_start` name too). The haystack's straw and the leap of faith's rush of air
  are clip keys (`xx_h_freestep_footr_to_faith_jump_*`, `xx_h_air_to_haystack`, `xx_l_haystack_hop_out`).
- Ambience (`bao ambience <game dir> <City>`): a few grid cells' data files hold an `AssassinAmbienceRegionCellData`
  (linked from the cell's own record, class ac2bbf68), naming a `SoundAmbienceSet` (`A_Wind_Damas_City`,
  `A_City_Poor_Area`, ...): its `SoundAmbiencePad` is the loop (a sound of type 6, a streamed file of four mono IMA
  tracks, `_LP_4.0`: quad, folded to stereo front over back, not checked; or a looped single sound, the water sets'),
  the `SoundAmbienceRandomFX`s the one-offs: u32 id, u32 class, f32 `MinRadius`, `MaxRadius`, `Height`, 6 bytes ?,
  f32 `MinRepeatTime`, `MaxRepeatTime`, u32 ?, u8 ?, then the sound reference (names from the exe's reflection list by
  CRC32; Damascus' poor quarter: 14, every 1-30 to 90-180 s, 1-9 m away). Ours plays each one-off at a random time
  in its interval, again and again, placed that far round the listener. The region's area is not read (only the
  cells linking it, a big region's a coarse one; the areas are probably the `RegionLayout`s `Snd_A_*`, not read), so
  the game plays the wind set and the other sets linked from the finest cells over the player, fading over 2 s; the
  crowd murmur (`walla`) sets are left out. Not checked against the running game.
- Placed sounds (`bao emitters <game dir> <City>`; Damascus: 1292): entities (an `Entity`, or an `EntityGroup`'s
  member) holding an `Ambient3dSoundComponent` (99310f0c: `TriggerComponentSettings`, then `Ambient3dSoundAudioEventSeed`s,
  each an `AudioEvent` and its sound reference: a play and its stop, then a `TriggerZone`: u32 its entity, u32 ?, the
  `SphereLocalCenter`, the `SphereRadius`; box and plane zones exist, not told apart). Fountains (a loop, 8-35 m),
  braziers, torches and lamps, flags (type-6 loops), kiosks, rooftop gardens and other hiding places, awnings, palms and
  olive trees (one-off rustles, palms within 1 m). Their sound objects are in the cells, the samples in
  `DataPC_Common.forge` (`BAO_0x3...`, a data file each: 80 for Damascus). Ours starts an emitter's plays as the
  player enters its zone (loops until he leaves it, faded over 0.5 s). Eagles at viewpoints, pigeons (an animation's
  keys: `pigeon_take_off_all`) and the hay cart's deformation clip are not placed here.

## Effects (FX, particles, camera shakes)
Read by `forge::fx`; drawn by `crates/game/src/particles.rs`, the shake in `shake.rs`.
- `FX` objects (class 824a23ba, `FX_*`) are graphs of operators: `FXSpawnedEntityOperator` spawns an `Entity`
  (`GFX_FX_*`) whose `Visual` links a `ParticleSystem`; `FXCameraOperator` drives the camera; `FXTargetEntityOperator` a
  material or property (`FXProperty`, `PropertyPath`, `MinValue` / `MaxValue`, `FXLinkedValue`). The graph is not walked:
  `fx::links` lists a body's u32s and the game keeps those that are known objects (FX -> entities -> particle systems).
- `ParticleSystem` (caaa7ee8): u32 ? (2), the emitters (inline `ParticleEmitter`, a1361abc, a fixed 579 bytes), 28 bytes of
  factors. Emitter fields found (names by CRC32 of guesses against the exe's reflection list, `?` where none): +0x04
  `EmitterShape`, +0x9d three f32 ? (the shape's size), +0xb1 `MaxParticleCount`, +0xb5 `ParticleRate`, +0xb9
  `Acceleration` (Z up), +0xce/+0xd2 `Min`/`MaxVelocity`, +0xd6 `DirectionVariation`, +0xde/+0xe2 `Min`/`MaxRotation`,
  +0xe6/+0xf2 `Min`/`MaxPhaseDuration` (three phases), +0xfe/+0x11e `Min`/`MaxSize` (4 keys of width, height), +0x13e /
  +0x16e `Min`/`MaxScale` (4 vec3, all 1 here), +0x19e `Color` (4 inline `IColor`s, bytes B G R A), +0x1ce/+0x1de
  `Min`/`MaxRotationSpeed`, +0x1ee `Mesh`, +0x1f2 `TextureMap`, +0x1f6/+0x1f7 `TextureFrameCountX`/`Y`, +0x214 an inline
  `WindForce`. 12/12 systems of DataPC's "Rank 9" and "Game Bootstrap Settings" parse to their end. The dust
  (`GFX_FX_CollisionMaterial_Default`): 30 particles at 300/s, 0.1-1 m/s up within 2 rad, -2 m/s² down, life 0.34 + 0.88
  + 0.62 s, 0.1 m growing to 1.45 m, white at alpha 0.14 -> 0 over `GFX_Dust_Sand_01`; straw: 20 chips of
  `GFX_Dust_Chips_grass` at 1-3.5 m/s under gravity.
- What plays them: each `ContactTable` entry lists FX (`ContactTable::fx`): every contact type but `ft_pivot`, `ft_step_toe`,
  `ft_walk_end`, `ft_walk_start` plays its surface's `FX_CollisionMaterial_<surface>` (dirt: the dust and a smoke; sand,
  mud, stone, wood, ceramic: the dust; straw, grass, puddles their own; marble, pebbles, leaves, snow, glass, polished
  concrete none). `HumanFXPack_AlTair` (ba42b58e) lists the FX AC1's code plays: +88 `FX_Camera_Shake_Assassins_Fall`,
  the punch and sword shakes, critical state, sync, and the haystack's `FX_Clip_Character_Hay_Bal(TopEntry)`,
  `(SideEntry)`, `(Side Exit)` (straw systems at six bones: `RightArm`, `LeftArm`, `Spine1`, `Head`, `LeftForeArm`,
  `RightForeArm`). The contact events' two bytes after the type are 1, 1 in every clip looked at: no FX switch there.
- Ours (`particles`): the contact keys play their surface's FX at the lower foot (steps) or the root (landings, rolls,
  hands); the dives into hay play `TopEntry` at their straw sound key, the hop out `Side Exit`. An emitter sends
  `MaxParticleCount` at `ParticleRate`, each particle going key to key at its phase ends; sprites face the camera,
  alpha blended, unlit (blend and lighting modes not read), one mesh per texture. `AC1_NO_PARTICLES=1` leaves them out;
  never headless. Damascus at a run, `AC1_FPS=1`: about 40-44 fps either way, GPU 3.0-3.3 ms (2026-10-13, a shared
  machine).
- The landing's camera shake (`shake`): `FX_Camera_Shake_Assassins_Fall`, started by 0xE05940 on a landing from over 3 m
  with intensity `min((drop - 3) / 7, 1)` (its parameter 0xBE93B29C). The FX's tracks (120 ticks of 1/120 s): of the six
  camera channels only the pitch moves, `intensity * envelope * 30 * fBm2D(t, 20 * (200 + 200 t))` degrees, the envelope
  1.5 -> 0.1 at 0.4 s -> 0 at 1 s, a one-octave fBm (H 0.1, lacunarity 0.1) of Perlin noise (as Banned445's port reads
  it). Ours: the same curve with our own gradient noise, about the camera's right axis. The pack's other shakes are
  combat ones. Not checked against the running game.

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
- Scenarios: `AC1_SCENARIOS=1 cargo test --release -p ac1 --test scenarios -- --nocapture`. They run 4 games at once (6 flaked with long frames: a swing's foot moving 0.45 m in one)
  (`AC1_SCENARIO_JOBS=n`), each ending at its time without a screenshot (`AC1_SCENARIO_SHOTS=1` keeps them);
  `AC1_SCENARIO_FILTER=text` runs only those whose name holds it, `AC1_SCENARIO_LOGS=dir` keeps each game's log there.
  80 scenarios, about 150 s (56 took 560 s one at a time with screenshots, 108 s side by side; 4 at a time 161 s, 8
  104 s but with timing flakes). A game's start and exit cost about 4 s of each run
  (the scenarios themselves 339 s): one game teleporting between zones would save that, below what running them side
  by side gives, at the risk of one run's state leaking into the next.
- `cargo run -p ac1 --example pops -- <recording.txt>`: pose pops in a flight recording (a bone moving much faster than
  the frame before, relative to the root; long frames are not counted), by what was playing. `forge` examples
  `gaitseams` (how a gait's half-cycles join), `clipjumps` (jumps baked into a clip) and `clipseam` (which clips start
  where another ends: what a code-driven move goes on into when the move graph does not say). `AC1_VEER=secs,deg`
  turns a scripted walk's direction then (the stick swung mid-move); `AC1_STICK=1` has a scripted walk hold the stick
  over (the speed from the gait model, as the player's). `AC1_PATH="x,z;x,z;..[;stop]"` steers it through waypoints
  (each passed within 1 m, at any height; `stop` lets go of the stick at the last). `AC1_HEADINGS="t,deg;.."` gives the
  walk's game heading over time, blended between (the route library replays the running game's own: W follows its
  camera, which turns slowly as it runs).
- The route library (2026-10-10; scripts in the session scratchpad, not the repo): routes around the Damascus bureau
  played in the running game through ac1-hook (the camera first turned, by test steps and the mouse, until W walks
  within 2 degrees of the route's way), traced frame by frame, and played in ours (`AC1_CONTROL`, the same inputs:
  the stick held with the speed from ours' gait model, AC1's headings, held legs on a wall as leaps), compared by the
  distance at the inputs' end, where each comes to rest, and when each takeoff, landing, wall run, hang and climb step
  starts. Every change to movement is scored on all of them. 41 routes (with ladder 68 by the bureau and the roof
  haystack north-east of it): 31 within 1 m at the inputs' end (30 at rest), mean path gap 0.48 m (roof2_free_ne, where AC1 misses its own
  target, 5.6 m of it).
- ac1-hook's live log (2026-10-12, `crates/hook/src/live.rs`, `livefmt.rs`): `live on [MB]` / `live off` in
  `ac1-hook.cmd` (or the file `ac1-hook.live` beside the exe: there, on; gone, off) writes the real game's player every
  frame to `ac1-live.txt` beside the exe in the flight recorder's line format (Bevy space; `time` is game seconds, the
  frames' lengths from 0x192DDA8 added up), so `pops` and the comparisons read both games' files:
  `time root x y z yaw <deg> vel <m/s> | <context> clip <last action's clip> | bones Hips Head LeftHand RightHand LeftFoot
  RightFoot x,y,z | targets <limb IK goals> | ac1 frame= wall= mode= dt= sub= speed= act= started=id:clip,... stick=
  latch= btn= high= legs= hand= cam= camdir=`; and `ac1-live-events.txt`: takeoffs, landings (time in the air, drop),
  grabs, other context switches, every action started, teleports. Each file starts over past its limit (50 MB, the
  events 8 MB), two old ones kept (`.1`, `.2`); writes buffered, flushed twice a second. Off, it costs nothing but a
  file check a second. Where each field comes from: the root, facing and context as `trace` (Entity matrix, `Human`
  +0x48's class); the speed from the root's move over the frame; actions from the `ActionComponent::Play` hook, named
  from `ac1-hook.actions.tsv` beside the exe (`livelog names` writes it); the bones from the `Human`'s bone handles
  (+0x280 Head, +0x290/+0x298 hands, +0x2A0/+0x2A8 feet; Hips looked up by its hash in the skeleton) read out of the
  skeleton component's global pose as last rebuilt; the IK goals from the limb IK at `Human` +0x530 (four limbs of 0x90:
  on, weight, settled, the hold's point in its object's space); the input from the interpreter (+0xD0 its pad: buttons,
  stick; +0x112B high profile, +0x1126 / +0x1129 the legs / hand buffers, +0x1300 the latched stick); the camera from the
  `CameraManager` (0x1A11998) -> switcher -> the camera's matrix; the sub-state from the context data (ground +0xB0 and
  its speed, climb +0x20). Layouts in `livefmt.rs`' header; the bones, goals, input and camera were read off the exe and
  are to be checked live (the hook logs the classes it found them in; `livelog summary` checks the bones sit about 1 m
  from the root and the camera looks at the player). The reader is the `ac1` example `livelog`: `tail` follows the file
  (`--for secs`), `summary [--last secs]` sums one up, `runs [dir]` lists the scripted runs in a folder of live logs (split at teleports), `cut <out> --from s --to s [--dir dir]` cuts one out of the rotated files, `compare <real> <ours recording>` lines a real run up with an
  ac1-rs F9 recording of the same route from when each starts moving (gaps, modes, clips, when each switch happens), then the poses (with `--game`: ours' clips named by their action, as the live log names a blend by its first clip; for each clip both play, every bone relative to the root in the body's frame and the limbs' distance to their IK goals). A control run (`AC1_CONTROL`) with `AC1_RECORD=<file>` writes the recorder's lines for every frame of the run, the ours side of such a comparison.
- The parkour gauntlet (2026-10-08, north-west: x -100 to -55, z 30 to 130): ten lanes along +X, one multi-move route
  each, with a `combo` scenario each (`AC1_SCENARIO_TAGS=combo`, about 40 s): G1 box, two beams across a gap, a swing
  bar, down onto a low beam (AC1's beam landing); G2 free running onto a ladder, up, a beam bridge, the leap of faith
  into hay and the hop out; G3 a wall run up hold to hold, over, a roof gap, off the far end into the damage roll; G4
  step up, jump onto 1.2 m, a ramp, a 2.5 m gap, down across a gap, the wall run over a passover wall; G5 four posts
  zigzagging (the jump aimed by the stick), a beam, a box, the dive into hay beside it; G6 two swing bars, down onto a
  post, post to post, onto a beam; G7 up a wall, along, the jump across a gap to the next wall, up and over; G8 the
  ledge stop on a 5.4 m roof, the pull down, down the holds, the turn round into a run, the jump onto a wall; G9 the
  passover, a wall run up and over, a jump down onto a post; G10 a swing bar flung at a wall's holds (the catch), up,
  a jump down onto a lower roof, off it into the roll.
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
  block L stopping a run, pulling down and climbing down, turning round at an edge, blended grabs and
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
Items also give blend-in times (a NONE kind, a cut, for 86% of items; audit ANIM-15), where the root motion comes from (the clip, physics, or steered:
entries are steered), the feet at start and end, and transitions to other actions. Which action plays when is
partly code, partly data.

The graph at runtime (`forge::graph::MoveGraph`, built with the clip library, 4454 actions in 51 blocks): an item's
transitions are (connector action, destination action) keys, one per way out; after a clip may come the next item of
its action or the first item of a connector or destination. Actions no transition leads to are started by the game's
code (keys such as `0x44` are its requests; a post jump's push off, `beam_impultionstraight_to_jumpstraight`, comes from
the crouch that way), and an item that ends its action with no ways out hands back to the code (the wall run's
`entry_a` and `entry_b` are separate actions the code steps through, its `WallingEntryA/B`); the graph judges neither.
It is used for: each move's blend (`MoveGraph::blend_into`: the next item's blend, a way's blend A into its connector and B
out of it, else, for an action the code starts, the playing action's out-transition, the new one's in-transition or the
default ASTOPBROLL 0.2 s; a NONE kind is a cut, AROLLB* kinds keep A playing, *BSTOP* hold B, the weight linear unless
eased; audit ANIM-4 to 6), and a check on every step from one
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
Entities with an `InertComponent` (static) or a `RigidBodyComponent` (props the physics moves) hold a `RigidBody` inline
that links a shape, placed by the entity's world transform. `MeshShape`
(format in `crates/forge/src/shape.rs`): vertices, a Havok MOPP tree, triangles, per-triangle materials, collision
materials, filter info, bounds. The `shapes` tool parses all 14889 (6.5 M triangles).

### What collides (as AC1, 2026-10-12)
The `RigidBody` (0x228f402a, `forge::shape::rigid_bodies`): +8 `CollisionFilterInfo`, +12 its filter word (bits 0-5 the
collision layer), +20 the shape, +40 a 4x4 that is identity on every static body and on moving props the inertia tensor
and centre of mass (not a placement: Damascus 14330 identity, 1670 props). The player's body is on MainCharacter (21:
`Human__CharacterLayer` 0xB0EAA0 turns the player's Character 5 into 21), and the physics world's filter set-up
(0x4DC8A0) turns 22 layer pairs with it off: Ragdoll, NoCharacterCollision, Nothing, OnlyStatic, RaycastNoCharacters,
HorseBarrier, Camera, CameraBarrier, OnlyStaticAndCameraRaycast, HollywoodMode, OnlyLOS, NPCBarrier,
OcclusionHeightTest, CameraBarrierPlusLOS, ProjectileBarrier, ProjectileCameraBarrier, OnlyLOSAndFuzzy,
HorseNoMainCharacterCollision, RaycastNoCharactersFuzzyZone, OnlyDynamic, OnlyFuzzy, RagdollPenetrationSolver (layer
names as Banned445's AC1-Movement-Rewritten, MIT, reads them; the pairs checked in Ghidra). Damascus' rigid bodies
(`bodies` tool) by layer: Static 13302, Dynamic 1924 (boxes, hulls: props), Interactive 333 (ladders, gardens), fuzzy
zones 189 (out-of-bounds walls, the outskirts' `Damas_OS_Colmap`, tents, big olive trees: they stop the body),
StaticNoHorse 47, StaticNoCamera 21; passed through: ProjectileCameraBarrier 123 (every rooftop hiding place's box,
`Hiding_Spot_CameraBarrier_01a`), CameraBarrier 55 (the haystacks' capsules, the river's `*_CAM` surfaces, the
bureau's `BureauCamCol01`, a 0.5 m block on its floor), HorseBarrier 3, ProjectileBarrier 3.

So a city collides with each entity's rigid bodies on a layer that stops the player, less the out-of-bounds walls
(`OutOfBound*`, `OOB_*`: the locked districts and mission fences, left out by choice). Other shapes an entity refers to
are not collision (a breakable structure's own volume, `BhvBreakableStructure`/`BhvKiosk`: a box round a scaffold's top,
one 0.9 m in front of a stall's counter; 174 in Damascus, all once solid here). Only an entity's first rigid body counts:
the 192 Damascus entities with two (32 in Masyaf) are breakable structures whose second is the broken state
(`Kiosk_Collision_Broken`, `Scaffold_*_01b`, `Storage_01b`) and city gates whose second is the closed `CityGate_FAKE`.
Checked in the running game: tp'd 0.6 m over the bureau's camera block (game 40, 0) he lands on the floor at 4.15; tp'd
over the south-east gate's horse barrier (game 184, 192, 3.2) he drops to the ground at 0. An entity with no
rigid body collides with nothing (Damascus 3368 entities with meshes: pigeons, tarps, ivy, bushes, grass, rugs,
awnings, flags, spice bags, torches, NPC parts), except terrain (`city::terrain`: ground, rock, cliff, mountain
meshes), as AC1 keeps that collision in data not loaded here (Masyaf's ground and rocks, 2.7 M triangles, in the
Kingdom's world). `AC1_COLLISION=render` collides with the solid render meshes instead, `fallback` adds them for every
entity with no rigid body (the behaviour before). F11 draws the collision round the player (`collision_view`: walls
orange, floors green, faces looking down blue, the near ones outlined); `AC1_SHOW_COLLISION` draws every shape
at load.

`AC1_COLLISION_SWEEP=1` measures it (`sweep`): points spread over every face (one per 0.25 m²) inside the city (the
houses' bounds and 10 m round), a drawn point with no collision face within 0.3 m is drawn surface the body goes
through, a collision point with no drawn face within 0.3 m an invisible surface. Drawn: the solid-named meshes (not
foliage, cloth, water or backdrops). Before (every shape an entity refers to, render meshes for entities without) and
after (rigid bodies by layer):

| City | Collision triangles | Invisible collision: walls | floors | Drawn, no collision: walls | floors |
|---|---|---|---|---|---|
| Damascus before (dev's meshes before the 2026-10-12 merges) | 4.29 M | 100887 m² (10.1 %) | 69162 m² (12.1 %) | 22556 m² (2.3 %) | 39303 m² (7.9 %) |
| Damascus after | 3.76 M | 88777 m² (9.2 %) | 53772 m² (9.9 %) | 25136 m² (2.6 %) | 41317 m² (8.3 %) |
| Masyaf before | 3.36 M | 28986 m² (25.9 %) | 20729 m² (39.9 %) | 8537 m² (9.8 %) | 1882 m² (5.1 %) |
| Masyaf after | 2.99 M | 21463 m² (20.8 %) | 10247 m² (25.0 %) | 8626 m² (9.9 %) | 1948 m² (5.3 %) |

What is left is AC1's own: invisible, the city walls' collision standing higher than the drawn walls
(`Rempart_Wall_12m_01a` 17598 m²), market stalls' simplified boxes (`Kiosk_Collision_*` 6962 m²), palms' and cypresses'
trunks round the drawn fronds, houses' and stairs' shapes simpler than the drawn ones; drawn without collision, the
ground outside the gates (`Ground_Rich_*Gate*` 33146 m²), big rocks, skinned market stalls, Masyaf's tower and cliffs
(their collision in the Kingdom's world), and props with no rigid body (+1349 m² of walls and +1590 m² of floors in
Damascus against before: barrels, spice stacks, awnings).

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
Altaïr's entity has a `ClothComponent`: `UCMA_Altair_Cloth` (his robe tails, the `DynamicMesh`
`UCMA_Altair_Cloth_SoftBody`) as a soft body colliding with `humanragdoll_forcloth`'s capsules. Everything it runs on
is AC1's data (2026-10-12; layouts first found by Banned445's AC1-Movement-Rewritten, MIT, `src/cloth.rs`; class names
by CRC32 of the exe's strings: `SubMesh`, `ClothRagdoll`, `ClothRagdollCapsule`, `WindForce`, `ClothActionEntry`,
`Hinge`); formats in the header of `crates/forge/src/cloth.rs`, tested on the install by `crates/forge/tests/cloth_data.rs`:
- The soft mesh: its own 146 float vertices with skin weights (the compiled mesh's 146 in another order, matched by
  place), 208 triangles of which 253 edges are constraints (the quads' sides), per vertex a pull toward the animation
  (44 pinned at 255, others 0, 15/255 or 127/255) and a collision radius (0.01 to 0.07 m).
- `Cloth`: damping 0.1, upward damping 0.75 (+ -2 per m/s the character rises, up to 5), 3 iterations, pull from
  motion 0.03 per m/s and 0.015 per unit of turn, falling at most 0.99/s; `ClothComponent` gravity -9.8; a
  `WindForce` (scale 1, 0.3 of the force along the surface kept, random amplitude 1).
- 19 capsules of which 6 on (thighs r 0.092, shins 0.11, feet 0.08-0.09, all mode 0: a vertex goes out on the side
  its animated place is), each in a ragdoll body's frame, moved onto the skeleton bone through the packfile's
  `hkSkeletonMapper` (21 bones to 19 bodies).
- `Altair_ClothActionSettings`: 20 (action, strength): the wall hang 0.9, sitting 0.01 (the robe follows the
  animation), the free-hang climb 0.5, leans 0.5, collisions 0.9, the ladder 0.96, the wall run's catch 0.5, a fight
  grab 0.6, five unknown actions 0.
`ik::cloth` steps it as AC1 does (Banned445's recreation, written fresh): Verlet with the damping (an upward-moving
vertex by the upward damping), gravity and the wind (its part along the cloth's normal whole, 0.3 of the rest, times
1 ± a random amount), drawn to its animated place by its own pull or the motion pull, whichever is more (the motion
pull: speed and turn, eased off by the playing action's strength, 1 for actions not listed); then 3 times: out of the
capsules, edges to a length between the rest pose's and the animated one's. `robe.rs` skins the soft vertices on the
CPU, steps the cloth 0.033333 s at a time on game time (AC1 steps once a frame at its 30 fps) and draws the robe on
the body with its offsets from the animated pose eased between the last two steps, so it moves the same at 60 or 144
fps and in the headless fixed 1/60 s runs. Ours, not AC1's: that timing (AC1 runs the step every frame whatever its
length), the strength read only from the climb's clip (the ground's locomotion is in no listed action), a frame
counted at most 0.1 s. `AC1_NO_CLOTH` keeps the skinned mesh.

## Bone modifiers (secondary motion)
Each skeleton carries modifiers its `SkeletonComponent` runs over the pose every frame (`forge::modifier`, layouts in
its header; `ik::secondary` runs them): Altaïr's main skeleton 14 hinges, 10 compressions, 6 roll bones, a spring
box; the skirt 16 hinges, a look-at, a compression, 2 rotation copies; the hood a look-at (its root's X aimed at a
helper under the head) and 2 hinges; the sword tag 2 hinges. Hinges swing a bone about one axis: its swing speed
gains from the bone's own motion (`inertia`), the authored force (gravity, in the model's frame) and the wind (the
entity's `SkeletonComponent` `WindForce`, scale 1), loses `damping` while it swings against the force, and is held
by hard limits with soft zones inside them (later limits measured from another bone's frame); after a jump of over
2 m or a long frame it settles in three longer steps. The order is AC1's: by bone depth, then bone order, then as
written. They run after our IK (the look-at of the head included), so the hood follows the head as AC1's does; the
add-on skeletons' modified bones start each frame from their rest pose. The behaviour follows Banned445's
recreation (`src/visual_pose.rs`, `src/skirt_hinge.rs`, his commits caab683, 826e97e), written fresh.
`AC1_NO_SECONDARY` turns them off.

## Wind
A city has one `Wind` entity (`Wind_Damascus_001` in Damascus's `Cell01364_DataBlock`, `Wind_Masyaf_001` in Masyaf's
`Cell05460_DataBlock`): strength 10, noise on (61.4 % spread, 100 % speed, 33.1 % floor, 11.7 % magnitude speed),
blowing along its placement's X axis. Its force wanders by AC1's 1D gradient noise (tables from Knuth's subtractive
generator, seeds 84 and 42), on game time since the map loaded; `ik::wind` reproduces a value Banned445 read from the
running game. The data block takes seconds to read, so `wind.rs` keeps the wind in the temp directory after the first
time. The test level uses Masyaf's; Acre, Jerusalem and Kingdom have none yet. `AC1_WIND=0` turns it off.

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
`ReachHighPointComponent`); the sync spot is the highest beam or post end on it. Damascus has 12. Eagle Vision (E)
marks them with gold columns. A viewpoint's entity sits partway up its tower, so the spot is lifted to the top above it.
Standing on one, the legs do the leap of faith to its nearest hay whichever way Altaïr faces (up to 12.8 m out; AC1
builds each viewpoint over its hay). The high dive (`..._3000cm_down`) is used only where nothing is under its 8 m
takeoff drop, else the low dive, its flight carrying it clear of the tower.

## Authored guidance (AC1's climbing markup)
A climbable entity carries a `GuidanceSystem` (55af1c3e) inside its body: the edges the player may grab (format in
`crates/forge/src/guidance.rs`, found through Banned445's AC1-Movement-Rewritten). Every one parses
(`examples/guidancecheck`): Damascus 8023/8023 (151,456 enabled edges: 150,915 ledge grabs, 317 ladders, 356 poles),
Masyaf 963/963 (13,217), Acre 7018/7018 (138,980), Jerusalem 6493/6493 (105,908), Kingdom 5374/5374 (26,836). No beam
edges are authored in the cities. Kiosk edges are, inside entity groups' members (every `Kiosk_*` stall: three, its
counter's front and sides), which the check, reading top-level entities only, does not count; the city loader reads
them (Damascus: 336 pieces, 112 stalls).
- A ladder is a vertical edge from its foot to its top (4.5 m for `Ladder_4m`), its horizontal normal out from the wall.
- A pole is two horizontal edges along it, one per side: one swing bar.
- A ledge grab's two normals are its top's (up) and its wall's (out).
Cities use them (`Level::use_authored`): the ledge grabs (sloping at most 40 degrees) are the holds, instead of the edges
and lips found in the geometry; the ladders and horizontal poles replace the ones guessed from mesh names. Damascus:
174,677 ledge pieces, of which 151,176 holds: a piece is shortened 0.1 m at a free end (a vertex no other piece uses), as AC1's hold grid shortens edges, so the lone 8-12 cm window-frame bits leave nothing (leaping to one hung him from the air) while a sill stored as three short pieces stays one hold; then 6,745 more left out as buried in a wall (no way to them from 0.45 m in front: a box's top 0.3 m behind a bare wall's face was grabbed through it), 333 ladders, 183 swing poles. `AC1_GEOMETRY_HOLDS` brings the geometry's holds back. A top-out may come down onto a floor up to 0.45 m under the hold (a parapet's lip in front of a
roof: 0.21 m refused it before); it also
needs room to stand (1.7 m of headroom 0.5 m in, no wall just past the edge, probed from in front of the wall, and the
spot not inside a solid): window sills are holds whose top is a window, not a floor to climb onto, and a beam stuck into
a building shows its top inside the wall. Narrow wall tops (a fence's or a parapet's, 0.15-0.35 m deep behind a hold
with a drop past it and in front of the hold, open above; not a roof's parapet with the roof just under it) become perches along their middle (`Level::add_narrow_tops`), and so does AC1's own
rule, its runtime beam contacts (as Banned445's port reads them): two holds along each other within 30 degrees, facing
away from each other, overlapping, at most 1 m apart, with a drop past both (no floor within 0.5 m under the top 0.3 m
out from each: not a strip or a seam in a roof, where running across one he was caught balancing), give a beam along
their middle, its pieces in line up to 1 m apart joined; both kinds kept only where the drop is there, tested every 0.25 m along (a roof edge's strip over a drop for half its length was a beam its whole length; the running game stepped off it where the roof ran flush) (Damascus 9,064 in all; none along one of AC1's authored beams, in line with it and overlapping it: north of the open roof one ran on 2.5 m over the roof past the authored beam's end, where the running game stepped off). Got onto from the ground only walking
along it (within 30 degrees) or standing still on its line; from a halt never one with floor beside it. A plank bridge 0.9 m wide by the Damascus bureau (`Wood_Bridge_01a`) is one:
the running game walked it as a narrow object (`HumanNarrowObject`, `xx_l_beam_crouchwalk`). The cities' authored
guidance has no `Beam` edges at all (Damascus: 0): AC1's narrow objects come from these pairs. Walking off a beam's
end onto another going on from it (within 0.45 m, 35 degrees) walks on along it. Climbing onto one ends crouched,
balancing on it, and free running carries on along or off it.

## The grab data against AC1's (2026-10-12)
`AC1_GRAB_AUDIT=<file.tsv>` (`grab_audit`, cities) writes every authored guidance piece with its source (the group or
entity placing it and its first mesh), its fate in ours, and ours' holds, perches, bars and ladders that no authored
piece backs. Every `GuidanceSystem` in the data sits inline in an entity or a group member (none elsewhere, no
`GuidanceSystemComponent` or run-time generated guidance in any city); the inactive ones (Damascus 180) are the other
state of breakable props (`Kiosk_*`, `Scaffold_*`, `Storage_01a`: intact and broken) and the animated city gates.
Ledge-grab pieces (Damascus 203,012; Masyaf 14,323), before -> after this pass:

| fate | Damascus | Masyaf | why |
|---|---|---|---|
| kept (half or more on a hold of ours) | 186,832 -> 188,188 | 13,074 -> 13,158 | |
| steep (past 40 degrees) | 5,949 | 60 | AC1's own report rule (0x66C350): pointed window arches, roof slopes |
| buried (`Level::hold_open`: no way in from 0.45 m in front) | 8,815 | 1,090 | ours' rule; scaffolds 1,243 of their 1,931 (a breakable's volume taken for collision, below), crates and baskets stacked side by side, an awning's back beam behind its front one, short end pieces; Masyaf's terrain edges under its render-mesh collision |
| hay (on a haystack's heap) | 1,356 -> 0 | 83 -> 0 | the hiding places' tarp frames (2.3-2.5 m over the box's rim, authored), dropped only on the frames square to the city's axes; now only geometry holds are dropped there |
| ladder (carved round a ladder) | 57 | 15 | AC1's report rule |
| lost | 3 | 0 | tiny pieces |

Ladders (333 / 42), poles (366 / 4) and kiosk pieces (336 / 27) are all kept. Ours without an authored piece, before ->
after: Damascus 29 perches (24 on `WoodBeam` meshes lying in the street 0.1-0.2 m up) and 16 beam-end holds from mesh
names -> 0 (`Level::drop_unbacked`), 11 swing bars from `Pole_*` meshes -> 0 (no poles by name where the holds are
authored); left: 1,347 thin-beam swing bars from pairs of authored holds (AC1 swings from any free hang a jump reaches).
By prop type (Damascus, pieces / kept): houses 74,289 / 69,848, windows 19,649 / 17,469 (2,036 steep arches), souks
8,064 / 7,904, towers and minarets 7,517 / 7,135, beams 5,549 / 5,228, walls 5,084 / 4,924, hiding places 4,836 / 4,805,
doors 4,608 / 4,155, awnings 3,685 / 3,221, balconies 3,143 / 3,024, gardens 3,005 / 2,868, baskets 2,603 / 2,015,
scaffolds 1,931 / 688, handrails 1,570 / 1,370, kiosks 1,344 / 1,132, barrels 915 / 689, lamps 643 / 630, stairs 548 /
478, carts 494 / 446, crates 120 / 75 (only `Crate_1m_01A` carries guidance: 30 of 193 crates; a stall's small crates,
most lanterns and suspended lamps, tents and most garden props have none in AC1 either). Open: scaffolds and market
stalls refer to a `BhvBreakableStructure` volume (a `ListShape` of boxes round a scaffold from 1.05 to 4.8 m up, a
`BoxShape` by each stall) and to their broken state's `MeshShape` (`*_01b`, `Kiosk_Collision_Broken`), which the city
loads as solid collision: it buries the scaffolds' holds (for the collision builders).

## City parkour objects
Benches, and the ladders and poles of cities without authored ones, are found by mesh name. Haystacks and hiding spots
are found by their components (audit GUIDE-7): `BhvHayStack` (0x19F58E6A) on the hay's entity, `BhvHidingPlace`
(0xDC3F581A) on a meshless member of the hiding spot's group. The meshes named as below give the bounds (on an entity
with the component and no such mesh, its first mesh). Damascus has 169 (46 hay, 123 hiding places), Masyaf 21; every one
also matches the names, and the names alone are kept for entities with neither component. The components' own fields
(the volume) are not read; they take the entity's position and facing. All of them are found when a city is placed and
set up against the built collision (`Level::add_city_objects`), logged as "N haystacks, N ladders, ...":
- hay: `Hay_Bale_Charette*`, `Hay_Bale_Chariot*`, `Hay_Bale_01` (not the straw strewn on the ground, `Hay_Bale_Tile*`).
  Their meshes use a vertex format not decoded yet, so a cart's stack stands where its entity is (1.8 x 1.8 m, 1.8 m
  high); a cart's parts merge into one stack.
- rooftop hiding spots `Hiding_Spot_*` (a plank box under a tarp, not the `_Tarp` mesh): hay too, its top the box's
  rim, 1.07 m over the roof (AC1's state in one is `HumanHayStack`). Free running at one, the running game hopped onto
  the rim and dived in (`xx_h_freestep_footr_to_haystack_02`); ours does too, diving from within 0.6 m of its edge. Out of one by AC1's
  hop-out room test past its footprint, its own shapes (the plank box, the tarp's frame) left out (`Level::hay_hop_room`).
- ladders `Ladder_<h>m`: thin and over 2 m tall. The building is the side with a roof just under its top (the ladder
  leans on that wall, its rails standing up past the roof edge, so its top is that roof's floor); else the nearer wall.
  The foot stands toward the outer side of its bounds. Climbed from the street onto a roof and back down in Damascus.
  A ladder's foot or top is taken before the edge it stands at.
- benches `Banc_*`: sat on facing away from the wall behind them.
- poles `Pole_*` lying down (stuck out of a wall, or across a gap): swing bars along their length.
Skinned props (hay carts, market stalls, scaffolds, storage racks) stand in their own skeleton's pose (2026-10-13;
`AC1_BIND_PROPS=1` draws the bind pose as before). Their bind pose is not how they stand: Damascus's breakable props
(the entities with two rigid bodies) were drawn turned, the market stalls (`Outdoor_Market_Stall_Skinned`, bind z -3.0..0.4 m,
posed -0.6..3.2) hanging under their base, the small scaffolds (`Scaffold_2x3x4_01A`) on their side, the storage racks
(`Storage_01a`) 90 degrees about the vertical; posed, each fits its intact collision shape (`Kiosk_Collision_Intact`,
`Scaffold_2x3x4_01A`, `Storage_01a`); the tall scaffolds and the carts are the same either way. A prop's skeleton has 24
bones at most, or any number on an entity with collision shapes (Damascus's `Scaffold_2x3x8`, 69 bones; characters have
70 and more and no collision): a collided entity left undrawn was solid thin air (user report #13).

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
  surfaces (`*Water*`, `*Lake*`, `*Puddle*`) get no collision; they are drawn with their water template ("City materials").
- Holds come from AC1's own climbing markup (see "Authored guidance"); without it (or with `AC1_GEOMETRY_HOLDS`) the
  runtime makes them from building edges (a walkable top meeting a wall below).

## How AC1 lights its world
Learned from the level data, the executable's reflection tables (class descriptors and their property lists, the
property names matched by CRC32 against candidate words) and the DX9 shaders inside the `MaterialTemplate`s and the
executable (their constant tables name every input); written up here, no shader code is copied.

- `WorldAmbiance` (one per level, in its own data file: `Damascus` in `DataPC_Damascus.forge`): the `LayeredSky`'s id, an
  inline `WorldFog` (`Enabled`, `FogColor`, `FogIntensity`, `FogDensity`, `FogNear`, `FogFar`) and an inline
  `PostEffects` with a settings object per effect: `ColourControlSettings` (`Hue`, `Saturation`, `Brightness`,
  `Contrast`), `ColorBalanceSettings` (nine shifts, three per channel, then `BlackLevel`, `WhiteLevel`, `Gamma`,
  `Opacity`), `GlowEffectSettings`, and blur, glitch, depth of field and other effects (not read). Byte layouts in
  `forge::ambiance`. Damascus: fog (0.86, 0.78, 0.66) 0.8, near 15 m, far 70 m, density 0.5; saturation 0.85; balance:
  red up, blue down in the midtones, white level 0.93, gamma 0.8, opacity 0.6.
- `LayeredSky`: `SunLightAzimuth`/`Elevation` (degrees; the azimuth is a compass bearing, clockwise from +Y, matched
  against the running game's shadows), `SunLightColor` (an `IColor`, BGRA bytes), `SunLightIntensity`, the
  characters' own sun (`UseCharacterSunLight`, `CharacterSunLightColor`/`Intensity`/`Azimuth`/`Elevation`), the sun
  flare, cloud shadows, and two `AmbientCube`s, `EnvironmentAmbient` and `CharacterAmbient` (`UseCharacterAmbient`):
  `Right`, `Left`, `Front`, `Back`, `Top`, `Down` (+X, -X, +Y, -Y, +Z, -Z), `Tint`, `Saturation`, `Scale`, `Bias`,
  `NormalTopBendFactor` (the executable builds the shaders' faces each frame: face x `Tint`, its HSV saturation x
  `Saturation`, then x `Scale` + `Bias`; the world takes the environment cube and its bend, characters the character
  cube and its bend when `UseCharacterAmbient`), then `Layers` (each a `SkyLayer`: a mesh, `LAYERED_SKY_SPHERE` in DataPC's `Game Bootstrap
  Settings` for most levels, and a `GFX_Sky` material: its diffuse maps times `DiffColor`, unlit and unfogged). Damascus:
  sun 115/45 degrees, (229, 200, 160) x 1.2; the characters' sun (255, 235, 219) x 1; one layer, a 1024x256 hazy sky.
- Baked occlusion: after each `MeshInstanceData` of a placed entity, a `CompiledMeshInstance` holds a grey D3DCOLOR per
  vertex of that mesh (`forge::instance`; the `SubMeshAOData` before it are the source bytes per submesh). The vertex
  shaders read it as `TEXCOORD4` and map it by `g_AOParams` onto the ambient only (`saturate(max(v x + y, z))`). Higher
  is more open: on average a mesh's top quarter of vertices is 110/255, its bottom quarter 78. `g_AOParams` (found
  2026-10-13: the global constant table's index 0, register c81, set per frame by the renderer) is the level `World`'s
  inline `AOSettings` (eac8d5c4: u8 `Enable`, f32 `MinValue`, `Scale`, `Bias`, names by CRC32 from its reflection):
  (`Scale`, `Bias` when `Enable` else 1, `MinValue`). Damascus 1.5 v + 0.1, at least 0.55 (an open wall's 110 gives
  0.75); Masyaf 1.7 v + 0.25, at least 0.8; Acre 1.6 / 0.25 / 0.65; Jerusalem 1.4 / 0.3 / 0.6; Kingdom the bytes as
  they are; Arsuf off (13/13 levels with a `World`, `examples/ambcheck`).
- The world's shaders (`GEN_Standard` and kin): diffuse map x vertex colour x the template's colour, lit by the ambient
  cube (each face weighted by the squared normal component on its side, the normal's Z first scaled by
  `NormalTopBendFactor`) x the occlusion, plus the sun's N.L with the shadow maps and cloud shadows; no specular. Then
  `lerp(colour, g_FogColor, saturate((distance - x) * y) * z)`. The characters' (`Character-Cloth` and kin): the same
  cube (the character one), the sun through the template's ramp at (N.L+1)/2 with the specular map's Phong highlight,
  cloth's rim (`0.7 d + 0.1` toward the silhouette), the same fog.
- The grade: the executable builds a 16x16x16 colour table each frame: per channel `BlackLevel`/`WhiteLevel`/`Gamma`
  levels, then the colour balance (GIMP's transfer curves: shadows, midtones, highlights), mixed with the input by
  `Opacity`; then a matrix: brightness, contrast about 0.5, saturation with the weights (0.3086, 0.6094, 0.082), hue. The
  frame is looked up in it. The player's brightness option moves the black level and gamma (the default leaves them).

What ours does (`look.rs`, `look.wgsl`, `city_material.rs`/`.wgsl`, `character_material.wgsl`): the same equations in
the city's and the characters' shaders, lit in display space as AC1's D3D9 shaders work (`AC1_LINEAR_LOOK=1`: linear),
the grade at the end of each shader instead of a pass over the frame (the same per pixel, no extra pass; the sky colour
graded on the CPU; `AC1_NO_GRADE=1` leaves it out). The sky dome is the level's first sky layer, its texture graded once
on the CPU and drawn unlit round the camera (`sky.rs`; `AC1_NO_SKY=1`). `GlowEffectSettings` has no fields: AC1's glow is
its glow materials' pass (`GFX_Glow*`), none of which Altaïr or the city's walls use; not drawn. The occlusion bytes of every placement sit in one storage buffer
(Damascus 27 MB) and each drawn part's `MeshTag` points at its first vertex's byte; the vertex shader reads its own.
`AC1_PLAIN_CITY=1` keeps `StandardMaterial` for the city. On the roof camera the new city shader costs 2.0 ms of GPU a
frame against 2.3 ms for `StandardMaterial`. Chosen against shots of the running game (not found in the
executable): the fog's ramp (`g_FogParams`: from `FogNear` to `FogFar`, at most `FogIntensity`; `FogDensity` not used,
see below), the clear colour behind the sky dome (the fog colour halfway to white) and how the character cube's
`Saturation` applies before 2026-10-13 (now the executable's own, above).
The fog (2026-10-13, `AC1_FOGP="start,scale,max"` to try others): where the executable fills `g_FogParams` is not
found (the renderer copies it from the per-zone light state, which nothing found writes from `WorldFog`). Against the
running game's roof views (reports/look3, the cameras matched to reports/visuals' shots) the earlier ramp slowed by
`FogDensity` (full at 125 m in Damascus) left the city 20-60 m off crisp: the green dome 35 m off measured 62 (median
grey) against the game's 89, the far haze band 109 against 146; with the ramp ending at `FogFar` (70 m) they are 88
and 124. The haze is yellower than the game's (blue 93 against 112 in the far band); not settled.
The occlusion's mapping is the level's own since 2026-10-13 (above); before it was chosen by eye (`2 v`, at least 0.25):
it gave open walls more ambient than the game (0.86 against 0.75) and closed-in corners far less (0.25 against 0.55).
Altaïr's character cube is now tinted as the game does it (before, its `Tint` was left out: multiplied in after a
luminance desaturation it turned the robe yellow; the game multiplies the tint first and halves the HSV saturation
after, so it stays near white).
Materials whose template cuts out alpha (`*Alpha*`, `Plants`, `Env_Foliage*`) are alpha-tested whatever their mesh's name,
and an assassins' bureau's outside shell (`BA_Bureau_Ext`) and its courtyard trellis (`BA_Treilli_01A`: the running
game shows no lattice shadows there, reports/visuals' bureau shots) cast no shadow: its roof shaded the open courtyard (AC1
swaps it for the inside one there). Side-by-sides of the running game, before and after: `reports/visuals/` (not in the
repo).

### City materials (what each template does with its maps, 2026-10-12)
From the templates' own DX9 shaders (the `MaterialTemplate` objects hold every variant's bytecode with its constant
table; disassembled with the Windows SDK's `fxc /dumpbin`, nothing copied) and the material parameters' names (the
template's graph names each parameter's node: `DiffuseLayer1` -> `Layer1`, `TileFactorLayer1` -> `Operator 31`, ...).
`examples/templates <game> <city forge> [template] [all]` lists the templates a city's materials use, most used first,
with their parameters named from the template's constants. Damascus: `GEN_Standard` 13175 submeshes, the layered
`Env_MultiBlender2*` 2612, `Env_Atlas` 1823, `GEN_DiffuseAlpha` 1576, `GEN_Standard_Specular` 613,
`Env_Standard_Saturation` 448, `Env_Foliage*` 626, `GFX_Water_Puddle` 159 (Masyaf: 2728, 757, 618, ...).
- Every world template but `Env_Foliage`, `Plants` and `Env_DistrictColorTheme` multiplies its diffuse by the vertex
  colour and its alpha by the vertex alpha; `DiffuseColor` (`DiffColor`) tints it; `Env_Standard_Saturation` (its
  parameters known by CRC32 only: 0x7da142d7 the factor, 0x845f2507 the colour) first takes the vertex-coloured diffuse
  toward its grey (the mean of r, g, b) by its factor. `GEN_Standard`'s `DiffuseUp` (max(1, x) on the tint) and alpha
  scale are left at 1.
- Layered (`Env_MultiBlender2`, `_UVScale[0,1,2]`, `_WorldUV[0,2]`, `_Specular`, `Env_MultiBlender1`): base, first and
  second layer, diffuse x its colour (`BaseColor`, `Layer1Color`, `Layer2Color`) and normal, each mixed over the last by
  the vertex's weights: `lerp(lerp(base, l1, w1), l2, w2)`; `_UVScale` scales each layer's UVs by its `TileFactor*`,
  `_WorldUV[0,2]` maps the base and second layer on the world's x, y times `TileFactor` (the first on its UVs times
  `TileFactorLayer1`). No alpha (the vertex alpha). `_Specular`'s highlight is not drawn.
- `GFX_Water_Puddle` (every river, fountain and puddle material of Damascus and Masyaf; Acre's sea is `GFX_Water`, not
  read): two copies of the `Normal` map (DataPC's `Game Fix` holds it, `GEN_Water_Fountain_01A_NormalMap`) on the world's
  x, y through the `Wave01`/`Wave02` panners (river: 0.1 and 0.2 per m, scrolling -0.02/-0.03 and 0.02/0.02 a second),
  averaged, scaled by `BumpFactor`; the view's reflection looks up the `cubemap` (the level's, e.g.
  `DAM_Cubemap_Water_01A`, along the game's (x, z, y)), times `CubeMapIntensity` and `Color`; lit as `ambient cube x
  occlusion x R + sun N.L x R + sun N.L x pow(R.V, SpecularPower)`, fogged; alpha `Opacity` x vertex alpha, blended. No
  Fresnel, no depth tint, no refraction in this template. Its puddles lie at the ground's own height (drawn with a depth
  bias).
- `Env_Foliage_FadeOut` (moss, lichen, the `Ground_*` patches over streets): alpha = diffuse alpha x vertex alpha,
  blended when the material has no alpha test of its own (its vertex colour is (177, 83, 0) on the street patches:
  drawn opaque they were red-orange stripes).
- `Env_Standard_WaterBlend` (9 materials: the river's walls, piers and pillars, the vizier's palace basins and fountain
  walls; no mesh names them, entities swap them in by their material overrides): the diffuse x vertex colour goes to
  `WaterColorVertical` by `saturate((WorldOceanLevel - z) * WaterFalloff)` (z the world's height) before it is lit by
  the ambient cube x occlusion and the sun, then `WaterColorHorizontal` x (1 + the view direction's z) is added unlit by
  the same weight: walls darken to murky water below the waterline. Drawn since 2026-10-13.
- The fountains and courtyard pools (2026-10-13, `examples/meshmats`): every fountain, wall fountain, well, riad pool
  and the vizier's and the Great Mosque's basins carry a `DAM_Water_Fountain_01A` submesh (`GFX_Water_Puddle`, no
  diffuse map in the template: the normal map `GEN_Water_Fountain_01A_NormalMap` panned at 0.4 and 1 per metre, the
  level's water cube `DAM_Cubemap_Water_01A` (a street scene, +Y the sky; `examples/cubefaces`), `Color` (0.32, 0.44,
  0.43), opacity 0.6); their maps all load. Their "texture" in AC1 is that reflection, the ripples and the basin floor
  seen through; the jets, rings and foam are effects (`GFX_Water_FontaineMarket_03`, `GFX_Water_03` diffuse maps), not
  drawn. In shade the reflection is the ambient x the occlusion x the cube x `Color`: dark, as the shader has it.
- Not drawn yet: `GEN_Standard_Specular`'s and `GEN_Cubemap`'s highlights and reflections, `Env_ColoringMask_TexStrip`'s
  colour masks (vases), `Env_DistrictColorTheme` (tarps), `GFX_Light*`, the glow passes.
Ours (`city_material.rs`/`.wgsl`): one shader, the kind and values per material in a uniform, the layers' and water's
maps in its own slots, the vertex colour as `ATTRIBUTE_COLOR` and the layer weights as `ATTRIBUTE_UV_1` only on parts
where they are not white / zero; textures are made once per texture (they were per material slot: Damascus 27 MB at
medium, 34 before). `AC1_SHOW_LAYERS=1` draws the layer weights (red the first, green the second, blue a layered
material, white water). GPU time at the medium detail's roof and street cameras: 1.21 / 1.25 ms against 1.51 / 1.46 ms
before (same run conditions; the shared textures). Against the running game (`reports/textures/real_*.png`): the
outskirts' grass and dirt and the bureau's floor match; the market inside the north gate shows sand in AC1 where ours
draws `DAM_Outskirt_Nograss_01A`'s first layer (grass), with the data's weights (open: a live check of which mesh and
weights AC1 draws there).

## City rendering cost and streaming
What a city costs to draw, at five fixed cameras (`AC1_FPS=1` logs the time from launch to the first frame and the
frame rate; the city logs `render: <parts> parts of <n> vertices drawn, <MB> of textures`). Cameras: `dam_player`
(Damascus, the start, the follow camera), `dam_roof` (`AC1_LOOK=46,60,-23,-200,0,150`: over the roofs toward the
citadel), `dam_far` (`AC1_LOOK=350,180,350,0,0,0`: the whole city from outside the walls), `mas_player` (Masyaf, the
start), `mas_over` (`AC1_LOOK=150,120,200,20,50,40`: the village and cliffs from above). Measured on a Ryzen 5 5600X
with an RTX 4060 (Vulkan) at 1280x720, vsync on (165 Hz display: 165 is the cap), with another game running beside it;
RAM is the process's private bytes once loaded (peak in brackets), VRAM the process's dedicated GPU memory (Windows'
GPU Process Memory counter).

| Build | City | First frame (headless) | fps player / roof / far | RAM | VRAM | Drawn (parts, vertices) | Textures |
|---|---|---|---|---|---|---|---|
| baseline (everything at full detail, decoded to RGBA8, at most 512 px) | Damascus | 28.0-28.7 s (28.7 s) | 165 / 165 / 137 | 2.2 GB (3.3 GB) | 756-886 MB | 64970, 21.3 M | 219 MB |
| baseline | Masyaf | 7.0-7.6 s (6.9 s) | 165 / 165 (over) | 1.8 GB (2.0 GB) | 757-762 MB | 8735, 4.36 M | 182 MB |
| textures uploaded block-compressed (BC1/BC2/BC3 as stored, mips kept) | Damascus | 28.4-30.7 s (27.3 s) | 134 / 60 / 85 (unsteady: the window behind another game) | 1.95 GB (2.8 GB) | 501 MB | 64970, 21.3 M | 31 MB |
| textures block-compressed | Masyaf | 6.6 s (6.5 s) | 164 / 64 | 1.5 GB (1.8 GB) | 564 MB | 8735, 4.36 M | 26 MB |
| levels of detail, cells round the camera (128 m box), stand-ins beyond; meshes built when first drawn | Damascus | 28.7-29.4 s (27.2 s) | 60 / 60 / 60 (the window behind, on a 60 Hz screen: capped) | 1.7 GB (2.8 GB) | 565 MB | instances player 8163 (3.47 M vertices) / roof 8055 (2.91 M) / far 205 (0.03 M) | 34 MB |
| same | Masyaf | 6.4 s (5.3 s) | 60 / 60 | 1.45 GB (1.8 GB) | 548-566 MB | player 1828 (1.08 M) / over 1068 (0.32 M) | |

Back to back with nothing else running (2026-10-12), the window on a 60 Hz second screen (so every build shows 60 fps:
the comparable measure is the GPU's time a frame, its top-level passes from Bevy's render diagnostics, `AC1_FPS=1`;
shadow maps are not in it). Drawn: instances / vertices at the player, roof and far cameras (Masyaf: player, over).

| Build (`AC1_DETAIL`) | Damascus drawn | Its meshes on the GPU (vertices) | GPU ms | Masyaf drawn | GPU ms | Textures (Damascus, Masyaf) | VRAM | RAM peak (Damascus) |
|---|---|---|---|---|---|---|---|---|
| before streaming (commit 1) | 17969 / 21.3 M everywhere | 1.38 M | (no hook) | 4456 / 4.36 M | | 219, 182 MB | 756-758 MB | 3.2-3.3 GB |
| full | 17969 / 21.3 M | 1.38 M | 1.44 / 1.89 / 4.93 | 4456 / 4.36 M | 1.39 / 1.95 | 34, 28 MB | 504-762 MB | 2.8-2.9 GB |
| high | 15021 / 6.05 M, 14927 / 5.37 M, 968 / 0.15 M | 0.79 / 0.63 / 0.05 M | 0.97 / 0.97 / 1.52 | 3361 / 1.93 M, 2796 / 1.13 M | 1.18 / 1.69 | 38, 39 MB | 502-504 MB | 2.8-2.9 GB |
| medium (default) | 8163 / 3.47 M, 8055 / 2.91 M, 205 / 0.03 M | 0.62 / 0.38 / 0.02 M | 1.08 / 0.98 / 1.19 | 1828 / 1.08 M, 1068 / 0.32 M | 1.32 / 1.23 | 34, 28 MB | 502-506 MB | 2.9-3.0 GB |
| low | 2595 / 1.24 M, 2516 / 0.96 M, 205 / 0.03 M | 0.40 / 0.22 / 0.02 M | 0.92 / 1.02 / 1.19 | 1485 / 0.68 M, 367 / 0.09 M | 1.14 / 1.19 | 14, 11 MB | 502-506 MB | 3.0 GB |

About 450 MB of the VRAM is the same whatever the city draws (shadow maps, Bevy's buffers, the stand-ins, Altaïr).
Time to the first frame: headless Damascus 27.4-31.7 s against 31.9-35.3 s before, Masyaf 5.1-6.0 s against 6.9 s
(paired runs); windowed within the runs' noise (30-33 s, 8-9.4 s): the rest is the gameplay set-up, which is the same.

### Levels of detail, the grid and the stand-ins (formats: `forge::lod`, `forge::world`, `examples/lodcheck`)
- `LODSelector` (51dc6b80): u32 `?` (0), five inline `LODDescriptor`s (a360319a; 21 bytes: u32 id, u32 class, u32 mesh (0:
  empty), f32 distance, f32 fade width, u8 `?`: 1 on the fourth descriptor of 7850 of the 18807), u8 flags (bit 0 keeps
  empty slots), u8 flags (bit 1 no cross-fade). Only the first four count; a kept slot draws up to its distance, the last
  from there on; an empty slot before the last mesh pushes the one before it out to its own distance. Commonest tables:
  10/20/beyond (4938, helmets and props), 20/80/beyond (3467, houses), 20/beyond (1598), 40/160/beyond (1465); bushes
  keep an empty slot and draw nothing past 50 m (594). Rules as Banned445's AC1-Movement-Rewritten (MIT) read them.
- `World` holds an inline `GridLayout` (7db083ed): u8 95 (the loading radius outside the grid), i32 cell size (32 m; 64 in
  the Kingdom), i32 origin x, y, i32 `?` (0; 0x029100ad in Masyaf), i32 cells across the finest level. Damascus: from
  (-480, -512), 32 wide, 6 levels (1365 cells); Masyaf: from (-1024, -1024), 64 wide, 7 levels (5461). `GridPartition`
  starts u32 levels, u32 cells; cell k (finest level first, each level half as wide) keeps its objects in
  `Cell<k>_DataBlock` (big objects in coarse cells). The cells' own records are not read.
- `FakeEntities` (c69a7f31, `<City>_Fakes`): u32 `?`, u32 64, then a `FakeEntity` per block (8 x 8 blocks of the finest
  grid, Z-order). A block with a stand-in has an inline entity (its transform) whose `Visual` draws `<City>_FakeMesh_<k>`
  (the block merged at low detail: Damascus 24 blocks), then per submesh a `SubMeshFakeIndexSpans`: per finest cell of
  the block (Z-order), the index span that is that cell's buildings (Damascus 1047 spans, all on cells with a data file).

### How a city is drawn (`stream.rs`)
Collision, holds, beams, hay, kiosks, guidance and jump targets are built from every mesh at full detail as before;
only what is drawn streams. Each placed mesh draws at its selector's level for the camera's distance to its box. A cell
is drawn while it overlaps a square box round the camera (128 m half-side), every level of the grid (so walls and
terrain in coarse cells stay); the other cells' buildings are drawn by their stand-in spans, each hidden once its cell's
own meshes are all drawn. Past a vertex budget (nearest first) the rest draw at their coarsest level. Meshes are kept as
their compact bodies (Damascus 51 MB) and built for the GPU on loader threads, nearest first; an instance keeps its old
level until the new mesh is there (a cell's stand-in shows only while one of its wanted meshes has nothing drawn yet:
until 2026-10-13 it also showed while an instance waited for a new level, so the cell's blurred merged mesh was drawn
over its own for a frame or a few at every level change: the "blinking textures" as the camera moved), and a mesh
nothing has drawn for 5 s leaves the GPU.
Headless runs draw nothing: no city meshes, materials or textures are made.

`AC1_DETAIL`, else the graphics menu's city detail and texture size (F8, `settings.rs`; default medium):

| Detail | Box round the camera (half side) | LOD distances | Vertex budget | Largest texture |
|---|---|---|---|---|
| low (a weak laptop's integrated GPU) | 64 m (AC1's own loading radius in Damascus) | x1.5 (coarser sooner) | 1.5 M | 256 px |
| medium | 128 m | AC1's | 4 M | 512 px |
| high | 256 m | x0.75 | 12 M | 1024 px |
| ultra | 384 m | x0.5 | 20 M | 2048 px |
| full | everything, no stand-ins | full detail always | none | 512 px (the city as drawn before streaming, to compare) |

Most of Damascus' 28 s is gameplay set-up after the city's 10 s load (holds, beams, ladders, hay against the built
collision); streaming only what is drawn leaves that as it is.

## Graphics settings and online play (the F8 and F10 menus)
Both menus are drawn as the F2 map menu (`menu.rs`): Up / Down pick a row, Left / Right (or a click, the right button
back, or the wheel) change it, Enter presses a button or opens a text field (typing, Backspace, Enter keeps, Esc drops),
Esc or the key closes. The cursor is freed while one is open and the game gets no keys; closing gives the camera the
cursor back. Every change is saved at once to `ac1-settings.txt` beside the executable (`key = value` lines; unknown
lines or bad values keep the defaults) and read at the next start. Scripted and headless runs (`AC1_HEADLESS`,
`AC1_SHOT`, `AC1_FRAMES`, `AC1_CONTROL`, `AC1_NO_SETTINGS`) ignore the file and run on the defaults, so the scenarios
are untouched; `AC1_SETTINGS=<file>` reads (and writes) that file in any run; `AC1_DETAIL` and `AC1_WINDOW_SIZE` still win.

| Setting (F8) | Choices | Applies |
|---|---|---|
| Preset | Low, Medium (the game as before the menu), High, Ultra; Custom once a row differs | sets detail, textures, shadows, anti-aliasing, view distance |
| Window | windowed, borderless fullscreen, fullscreen (the monitor's mode) | live |
| Resolution | 1280x720 ... 3840x2160 (windowed) | live |
| VSync | on (`Fifo`) / off (`AutoNoVsync`) | live |
| Frame cap | off, 30, 60, 120, 144 (the frame's rest slept out in `Last`) | live |
| City detail | low, medium, high, ultra, full (`stream::Detail`: draw box, LOD scale, vertex budget) | live; to or from full on a restart |
| Texture size | 256, 512, 1024, 2048 px (city textures are cut down as they load) | restart (the menu's Restart row starts the game again where he stands) |
| Shadows | off; low (one 1024 cascade over 40 m, hardware 2x2 filtering); medium (as set up: the city's 3 cascades in 120 m, 2048); high (4096 maps) | live |
| Anti-aliasing | off, FXAA, MSAA 2x, MSAA 4x (the default) | live |
| Fog | on / off (the city's linear haze) | live |
| View distance | 600, 1200, 2500, 5000 m (the far plane; the haze closes in by 90 % of it) | live |

Low (low detail, 256 px, no shadows, no AA, 1.2 km) is for weak PCs: Damascus draws about 1.2 M vertices there against
about 8 M at Ultra.

Online (F10, `net.rs`; the old `feature/multiplayer` branch, folded in on 2026-10-12): two players over UDP, each
sending its root, every bone's rotation (16-bit) and the first four bones' positions and its camera 30 times a second;
the other player is a second Altaïr posed from them (made, hidden, when a session starts). Rows: Hosting on / off and,
when on, the hosting port (UDP, default 7777; open it in the firewall / router to be joined from outside); when off, Join
IP, Join port and Join game (and Leave); Record replay (`replay.rs`: `replays/ac1-replay-<secs>-<pid>.bin`, about
20 MB a minute, played back with `AC1_REPLAY=<file>` or `replay.bat`). The status line shows listening, connected (to
whom), the peer count (a host takes one friend; a new one once the last has been quiet 6 s) and errors (a port in use,
a bad address, "nobody is listening there" when Windows reports the joined port closed). `AC1_HOST=port` /
`AC1_JOIN=ip:port` start the same from the environment. Checked between two copies on 127.0.0.1 (windowed and
headless): each hears and shows the other. Not done: NAT punching, more than two players, anything but poses (no
shared world state).

## Animation notes
See the header of `crates/forge/src/anim.rs`. In short: per-track blocks
`u8 type, u32 size, u32 n, n-1 frame keys (u8, u16 if bit 0), n values` at 60 fps. Rotations are
smallest-three quaternions in 16/24/32/48 bits (or 3 floats whose low mantissa bits hold the index), translations are millimetres (packed 11/11/10
bits or i16x3). Hash 0 tracks are root motion; clips face +Y with the Reference bone turned 90 degrees.

## License
MIT, see `LICENSE`. This repository contains no game files: you need your own copy of Assassin's Creed.
