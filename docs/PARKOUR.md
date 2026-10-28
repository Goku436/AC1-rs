# How AC1's parkour works

What AssassinsCreed_Dx9.exe (v1.02) does to choose, chain and smooth Altaïr's movement, as far as it is known, and
what ac1-rs does with it. Most of what is here was traced by Banned445 in AC1-Movement-Rewritten (MIT, Copyright (c)
2026 Banned445; addresses and names are his, from his RE notes and code comments); the rest is ours (the move graph,
our measurements in the cities). Written here in our own words; no decompiled code. Addresses are the game's
functions and tables (image base 0x400000).

Marks: **[B]** from Banned445's work, **[V]** checked by us (in the exe, the data or play), **[ours]** our own
finding, **[?]** not known yet.

## 0. The context data (`Human*Data`) **[V]**
Each context keeps its state in a `Human*Data` block inside one container at `[Human+0x30]` (getters
`mov eax,[ecx+0x30]; add eax,X; ret` at 0xB2FC50..0xB2FD80: Ground +0x40, InAir +0x340, Ledge +0xC50, Climb +0xDA0,
...); a context stores its block's address at its own +0x10. Property names are CRC32 hashes in the reflection
data (`[name ptr]-0x20` = property list, 32 bytes each: hash, enum, type<<16, dword offset<<20); recovered by
hashing the exe's own words:
- `HumanClimbData` (0x70 bytes, 17 properties): +0x10 `CurDirection` (vec), +0x20 `SubState`, +0x24 `EntryType`,
  +0x28 `EntryPoseType`, +0x2C `CurSpeedRatio`, +0x30 `DestSpeedRatio`, +0x34 `GridTileWidth` 0.75, +0x38
  `GridTileHeight` 0.6, +0x3C `UseIK` (byte, false), +0x40 `LeftToePull` / `RightToePull` / `LeftHandPull` /
  `RightHandPull` 0.5 each, +0x50..+0x5C four more per-limb floats 0.1 (names not recovered); unreflected +0x60 = 4
  and +0x64..+0x6C = -1 (the reach's action ids, read by 0xDE8300, 0xDE9B50, 0xDEEE60, 0xDEF0C0). Of these the
  climbing code (0xDE4000..0xE00000, all 453 functions decompiled) and every caller of the getter 0xB2FCB0 read only
  the state, the two speed ratios and the action ids: the grid sizes, `UseIK` and the eight pull values are never read
  in v1.02 (the grid hard-codes 0.75 × 0.6, `BuildHoldGrid` 0xDF6A40).
- `HumanGroundData` (0x300 bytes, 38 properties): +0x10 `CollideNormal`, +0x20 `CurSight`, +0x30 `DestSight`,
  +0x40 `FactorLinkReset`, +0x50 `DestHeading`, +0x70 `CollidePosition`, +0xB0 `SubState`, +0xB4 `CurSpeedRatio`,
  +0xB8 `DestSpeedRatio`, +0xC4 `ParamFlags`, +0xC8 `InternalFlags`, +0xCC `LinkSettingsColor`, +0xD4
  `CurrentBodyAngle` and +0xD8 (?) (both fed through the lean filter 0xD94C80 by 0xDA0810), +0xDC `ObstacleLeanType`,
  +0xE0 `CollideHeight`, +0xE4 `CollideEntity`, +0x100 `PinDownData`, bits `Crouch`, `Sprint`, `IsSet`. Defaults:
  +0xE8 0.4, +0xEC 1.5, +0xF0 (0, −0.4, 0.9) and unreflected +0x238 1.0, +0x23C 5.0: none read by the ground
  locomotion (0xD80000..0xDD0000, 0xEE0000..0xF00000) or the getter's 72 callers: fight / pin-down values (the
  class's groups are "pindown" and "fight system").
So no hidden locomotion tuning lives in these blocks: the values are constants in the code (the rest of this file).

## 1. The frame: one locomotion context at a time
- The player (`Human`) runs exactly one locomotion context: Ground 4, Ladder 5, Pole 6, Rope 7, InAir 8, Ledge 9,
  Climb 10, Walling 11, NarrowObject 12, HayStack 21 (`ActorContextID`). **[B]**
- A switch is immediate (`AIActor::SwitchLocomotionContext` 0x55F7E0): the old context exits, a `TransitionSetup`
  object fills the new context's data (entry type, target, the action to start with), the new one enters and skips
  its first update. **[B]**
- Contexts ask for switches through numbered **events** (Ground: 42 obstacle, 49 wall run, 68/69/70 ledge and
  pull-down, 72 onto a beam, 119 look down, ...), each with a guard function that probes the world. **[B]**
- Input is read by `GoAssassinActionInterpreter` (0xEE65A0 on the ground, per-context states such as 0xEE9AF0 on a
  beam, 0xEE05E0 walling). Stick dead zone 0.35, speed = (|stick| − 0.35)/0.65; the legs button is buffered 0.3 s.
  **[B]**
- **The keys** (the install's `DefaultBindings.map` and manual): WASD move, right mouse held = high profile, Space =
  legs (low: blend; high: held, sprint and free run, "automatically adapt to any object in the path"; pressed, the
  jump), Left Shift = empty hand (push; high: grab and throw, tackle), left mouse = weapon hand, E = head (first
  person, Eagle Vision), F target lock, C centre camera, Tab map. A Legs press jumps only at a guidance target (the
  jump interpreter: high profile + the legs buffered + the stick); with nothing in reach it does nothing more than the
  sprint. **[V: manual, bindings; ours]**

## 2. Animation: actions, items, gates
- Every clip plays as an item of an action in an `ActionBlock` (452 blocks, 15463 actions in the install). An item's
  transitions name the actions it may hand over to; AC1's own graph of them is in `crates/forge/src/graph.rs`.
  **[ours, V]**
- Code-driven actions (`FROMAI`) are picked by the context code, not by the graph: jump takeoffs and flights, climb
  moves, landings. The graph still holds their follow-ups. **[B, V]**
- Each item carries a gate word (+60, `ActionItem::Serialize` 0x5BADF0) the contexts read for the playing item
  **[B]**:
  - 0x20 locked: no new move or mode while it plays (`HumanGround__AnimAllowsModeExit` 0xD80010,
    `HumanClimb__CanStartGridMove` 0xDE7FA0, ...);
  - 0x40 / 0x80: may be left for standing (low / high profile); 0x200 / 0x800: may be left for moving;
  - 0x10: the clip turns the body, the code's heading is not applied;
  - 0x1000 on jump takeoffs, 0x2000 on the `*_tr_fall` items;
  - bits 0-1 / 2-3: the feet before / after (1 left ahead, 2 right ahead, 3 parallel).
  These are how AC1 decides *when* a move may be interrupted: the smoothness comes from letting a clip run to a gate
  before the next starts, and from the item's own blend time (0.0-0.3 s, in the action data).
- An item's duration is the weighted sum of its clips' lengths (0x507650 → 0x5B9480). **[B]** **[V]** 2026-10-24: 0x5B9480 sums
  weight × length (+0xC) over the clips, skipping one flagged +0x20 bit 0; a chain's (0x507650) adds its items', each blend
  into the next taken off for kinds 1-2 (+0x28) and, for kinds 3-5, the step's +0x24 (capped at +0x28 when +0x34 bit 3 is
  set) less +0x28 added.
- Root motion: a clip's root track moves the character. Discrete moves (climb, ledge, hay entry) instead interpolate
  the root linearly from where it is to a computed target over the clip's length (`sub_711130`). Jumps add a linear
  correction to the clip's own displacement (section 5). **[B]**

## 3. Ground (`HumanGround`)
- **Speed**: one parameter 0..1, bands Walk ≤ 0.25 < Jog ≤ 0.5 < Run ≤ 0.75 < Sprint (`GetSpeedBand` 0xD807B0).
  Target = base + 0.25·stick, base 0 low profile / 0.5 high / 0.75 sprint (free run). It rises at 1.0/s and falls
  along a response curve. A start from standing sets it to 0.25 (walk, low) or 0.5 (jog, high) at once
  (`HumanGround__PlayStartMove` 0xD98990). **[B, ours: `gait.rs`]** **[V]** 2026-10-24: the bands (0xD807B0 on +0x5D4: 0 none,
  ≤ 0.25, ≤ 0.5, ≤ 0.75, over) and the start's 0.25 / 0.5 by the profile (+0x5DC) into +0x5E8.
- **Turn attenuation** in high profile: beyond 45° off the facing the speed scales down to 0 at 90°, floor 0.1,
  rising back at 5/s and falling at 10/s. **[B]** **[V]** 2026-10-24 (0xEE65A0: the stick's angle off the facing, +0x1264,
  capped at 90°; the factor +0x10D0 rises while at or under 1 − its share past 45° and falls while over it, so at 30 Hz it
  hunts round that value rather than stopping on it; held at 1 out of high profile, +0x112B). Ours (`gait.rs`) eases
  onto the same value: the same mean. The stick before it: (|s| − 0.35) / 0.65, capped at 1.
- **Heading** turns toward the stick at a capped rate and the character moves along the heading, not the stick: 4.0
  rad/s (2.6 free running), at that rate till the way wanted (no easing: live comparison run2, 3.8-4.3 rad/s to 2° off; the interpreter's RotateTowards,
  0xEE6D3F); a turn on the spot only past 90°. The running game turned 3.98 and 2.58 rad/s, its motion within 1-5° of
  its facing. **[B, measured; ours since 2026-10-10]**
- **Live comparisons** (2026-10-16, `tools/live-compare.sh`: one pad file played into the running game and ours at
  once, the real camera's yaw handed over; pad files in `crates/game/tests/pads/`). Found and fixed: **[V, ours]**
  - The turn rates by profile: from standing and in high profile the full 4.0 rad/s to within 2 degrees (run2: 3.8-4.3);
      the low-profile walk eases 4.0 to 1.5 over the 60-10 degree band (roof_walk: 3.36 at 42, 1.84 at 16, 1.58 at 9);
      free running eases 2.6 to 0.975 (roof_sprint: 1.15 from 26 to 6 degrees off).
  - AC1's camera turns after him: at 4/s times the angle to his heading, at most 6 rad/s, while he is within 118 degrees
      of its way (116 off followed, 119.8 did not), on the ground moving, in the air and on beams; a held stick off its
      way runs him round in a circle (street_run_stop). Fitted over 21 real runs: 510 degrees rms in all against 1331
      with no follow. Not modelled: its cuts off walls (run2: 126 to 71 degrees in a frame). Paused 0.6 s after the
      player turns it.
  - The wall run wants the wall 0.5 m to both sides of where the run meets it (0xE149E0, a face within 0.3 m of the
      contact's plane): at the bureau's west wall 0.15 m from its corner the running game stopped (its run stop, legacy
      action 87) where ours ran up into the recess past the corner (run3).
  - Free running into a wall within 45 degrees of square that is not run up: the wall slide's head-on case (0xEDD7C0
      result 2, its 135 degree bound read in the exe), the wanted speed 0: the run or sprint stop once, then standing at
      it; ours ground along the wall at 0.5 m/s. The contact is measured square to the wall (met at an angle the wall is
      further along the stick).
  - The edge jump wants the facing within 45 degrees of the edge's way and the stick held (not pulled back), not the
      stick within 45 too: the running game jumped off a roof with the stick 70 degrees off the edge's way (roof_turn2).
      Open: there it then fell past an awning 4.2 m down that ours' scorer takes as a far-edge target (AC1's own
      LedgeGrab holds; its reason not found).
  - The knee catch wants 0.5 m of air under the feet: at the foot of a 7.7 m fall ours caught a 0.4 m kerb with the
      knee, the feet on the street (the running game landed and rolled).
  - Leaning on a wall in high profile, the legs press runs up it (event 49 in the lean): ours' input sent the press to
      the ground only (bureau_wallrun).
  - Measured, not yet ours: the slide along a wall after the run stop is the camera coming round (it follows his
      sideways slip), then a turn along the wall at walk speed (1.8-1.9 m/s); the real interpreter's high-profile flag
      dropped mid-run in one run and not in another with the same input (not understood).
- **Locomotion** is one action (0x05923BDB), an item per leading foot, each blending 17 clips by speed, hip lean
  (crowd avoidance, walk band) and bank (heading change, jog and up), with a jog slow-down timer and a sprint settle
  weight (`HumanGround__UpdateMoveBlend` 0xDA0810). The character moves by the blend's root motion, not by a speed.
  **[B]** Ours since 2026-10-08 (`move_blend.rs`): the 17 weights by AC1's rules (four bands; the walk band's hip lean,
  none here as there is no crowd avoidance, so walks never lean; the bank from the jog up; the jog's slow-down timer
  shared with the jog's weight; the sprint's take-off weight), the clips on one clock whose step lasts Σw·T, and the
  ground speed is the blend's own, Σw·d / Σw·T (1.90 walk, 3.54 jog, 5.12 run, 6.28 sprint, slower between bands than
  a straight mix). No pose pops over curving walks, runs and free runs (`pops` example).
- **Run stop**: `xx_h_{jog,run,sprint}stop_foot?`, weights [jog 0, run 1 − f, sprint f], f = clamp((s − 0.75)·4),
  then the stop's own `_tr_h_wait`; locked (steering does not break it off); past the walk band (s > 0.25). In high
  profile a reversal past 135° plays it too, and at its end, the stick still over 120° round, the skid turn
  `xx_h_runturn180_<foot>` into `_tr_jog` (the facing flips as it starts). Free running swings round instead. **[B,
  ours]**
- **Ground loss** (`HumanGround__CheckGroundLoss` 0xD87720): the drop report (the nearest LedgeGrab edge in a 0.75 m
  zone, |dz| < 0.3, with at least 0.5 m of drop; else the drop under the feet) says the feet are on or past the edge
  line and the velocity does not point back from it → InAir fall. The fall starts at the edge line. **[B]**
  **[V]** the 0.5 m minimum drop and the 0.01 m edge distance are in the function; but it returns at once unless
  0xC7F150 says so (a byte at HumanGroundData+0x2F8), which Banned445 found set only while grabbed in a fight. **[V]**
  The ordinary walk-off is 0xDA3F40 (tried right after it by the ground update 0xDA6750 / 0xDB4F00): the character's
  cached edge report (Human+0xB00) says its distance to the edge is under 0.01 m and its type is 4 → 0xD9D1F0, the
  fall type from 0xD8C380 (drop < 1 m, or < 8 m on probe type 1 → 0/1 split at 2 m into 2/3; else 4; the faster one
  of each at a horizontal speed ≥ 2.5 m/s), then the switch to InAir (`SwitchLocomotionContext(8, ...)`). So the walk-off
  is at the edge line. Ours: the ground under the root gone.
- **Fall type** by height (1 / 2 / 8 m) and a horizontal speed of 2.5 m/s (0xD8C380). **[V]** (2026-10-23): the
  drop report's height under 1 m, or under 8 m on a probe of type 1 (its slot 0x148): types 0 / 1 below 2 m, 2 / 3 from
  2 m; else 4 / 6; the second of each at 2.5 m/s or more. (Banned445's port tests the 2 m inside the 1 m branch, so
  its types 2 / 3 never come.)
- **Obstacles** (event 42, guard 0xB25230: vertical speed ≤ 0.2 m/s, a contact within 45° of the facing at least
  0.5 m over the feet): at least 0.7·h high → the hand collide `collide_full_hand_{070,150}cm` then the two-hand lean;
  lower → the foot collide `collide_full_footl_{050,070}cm`, blended by height; the root moves 0.15 s to the contact
  + 0.4·h. Leaving: along the stick, [back, side] blended by angle, × height × [walk, jog]. **[B, ours: `collide`]**
- **Edges**: the ledge stop (event 69, only for a drop over 5 m), the low-profile halt within 0.16 m (normal within
  70°), look down (event 119: latched by the low-profile halt held 0.25 s, the edge within 0.16 m, more than 2 m of drop;
mixed front / left / right by the edge's side), the pull-down
  (event 70: the edge report within 0.75 m of the feet whose normal is nearest the facing, that normal within 90° of
  it, more than 2 m of drop). **[B]**
  Seen in the running game (Damascus, the route library): running in high profile to a roof's end that is a beam over a
  0.1 m crack, the next roof 0.74 m below, AC1 ran over the beam and stepped off it 1.07 m on in 0.27 s (`roof2_run_south`);
  at the same kind of beam over 8 m it made the ledge stop. Ours since 2026-10-10: a beam ending the floor is run over
  as ground; past it, a drop of 0.53 m or more is AC1's **step off** (event 68, 0xD84190 -> 0xDA7A10, Banned445's port):
  a free jump aimed 1 + 2·a·s m on (a 1 in high profile, 0 in low; s the speed value) and down to the ground there,
  in the air from its first frame at the speed carried, launched 0.5 m/s up, falling at the free jump's gravity; else,
  with a target in the **edge zones** (10-12, the primary only: 0.5-4.2 m on, 2.7 m up, 5 m down; the scorer's far side
  on), AC1's edge jump at it, either profile. Neither where the ledge stop takes the drop. **[B, ours, measured]**
- **The edge reports in a city** (`edge_report.rs`, 2026-10-14): ours' free-run edge jump, edge jump at a target, step
  off and ledge stop fire at AC1's foot-level edge reports (IHuman vt132 0xB1BC50, picked by vt136 0xB17EF0; Banned445's
  port), not at collision drops: the guidance edges (all kinds but ladders and beams, up to 40 degrees) within 0.75 m whose
  plane through the feet meets them within 0.3 m of their height, with 0.45 m of drop or more (0xB19620: first the line
  from 1 m over the feet to 0.02 m past the edge, then the 45 and 10 degree rays, steep floors slid down, a step under
  the edge adding the drop past it); +0x34 is the squared distance, negative past the edge. **[V]** in the interpreter
  (0xEE7D64..0xEE7DCE): the free run's edge jump wants the facing report's drop over 0.5 m (0x1683384), +0x34 under 0.25
  (0x1682378: 0.5 m) and its normal within 80 degrees of the stick (0x16FFD9C, cos 80), with the legs held. The edge jump
  at a target: within 0.1 m (0.01) or past it, the stick and the facing within 45 degrees of the normal; the step off:
  a drop of 0.53 m or more and room past the edge (0xB2E7A0's box); the ledge stop: a front report (within 60 degrees)
  over 5 m within 0.15 m. Off a collision drop with no edge he falls. The free-run jump now leaves 0.3-0.5 m before the
  edge, as the running game (audit JUMP-5: 0.13-0.31 m before ours' former 0.25 m test). A beam over a crack past a roof's
  end, its top over the edge, gives no drop (the line from over the feet meets it): run over (`roof2_run_south`). The
  test level has no guidance (its holds are ours), so there the collision tests stay (`Level::guided`;
  `AC1_NO_EDGE_REPORTS` keeps them in a city too). **[B, V, ours]**
- **AC1's candidate query and scorer** (`crate::jump_query`, after Banned445's port of 0xE18970 / 0xE96BF0): the hold
  edges in a box ahead (1 m either side of the way, 0.5-9 m on, 5 m down to 3 m up), chained, a grab point per chain
  where the forward line meets it, classed by three rays (room on top, what is below, a clear line from the chest) and
  AC1's reach zones (from the ground zones 1-3, from a beam or post zone 7 only); beams along the way within 40 degrees;
  far ends of tops. The scorer: a bar, the nearest roof edge in front (a top 1 m deep or more, or a beam), the highest
  other target in a 45 degree cone, a far edge below, then behind. Ours' own: side edges count only up to 90 degrees
  (Banned's 135 made a roof's own corner a target), a beam along the way only where the forward line passes within 0.6 m
  of it, beams across the way and posts read as roof edges (ours has beams without hold edges). `AC1_OLD_TARGETS` keeps
  the older search below, `AC1_JUMP_CANDS` logs the candidates.
- **Jump reach and the pick** (the older search, `jump_target_within`, still used by the ejects): a running jump's targets lie in AC1's reach zone 1 (`JumpZones`
  0x1A2BF40, read live by Banned445: up to 1.3 m up to 3.5 m on, 0.8 at 4.7, -0.5 at 6, -3 at 8, down to 5 m below),
  only on the forward line (tops are looked for straight on, not 20° to the sides), the far end of a top too (landed
  0.2 m short of it). The scorer (0xE96BF0): a target over a plane through the hips tilted down ahead (normal 0.5 on,
  0.7 up) is in front; the nearest in front, else the furthest behind; a swing bar as near as and no lower than the top
  first. Off roof 2 a beam 4.4 m down 1.5 m on is behind, and the running game jumped over it to the roof 2.8 m on; off
  the seams roof one 2.3 m down 6.5 m on is in front and it jumped there; off roof 1 onto a roof 3.7 m below all is
  behind, and it landed 0.2 m short of that roof's far end 4.6 m on (ours now 0.05 m from each). A hurt landing rolls
  only with the stick held (AC1 by the stick's speed): let go, it lands and stands. **[B, ours, measured]** **[V]**
  2026-10-24 (0xB25230): |vertical speed| ≤ 0.2, a contact within 45° (0x3F490FDB) whose height is at least 0.5 m over the
  feet, on a component with bit 0x10000000 at +8; not a `CharacterComponent` when the caller asks so, and a
  `RigidBodyComponent` only with +0xD4 ≥ 30 (its mass, likely: light props are pushed, not bumped). Ours bumps static
  walls only, so the last two do not come up.
- **Jump targets at roof edges** are where the way forward crosses the edge, as for beams (AC1's candidates): the edge's
  point nearest a spot 4 m ahead was its end when the way crossed the next piece, and the jump went 0.7 m off the way; a
  beam's own side edges are no roof edges. **[ours, measured]**
- **Grabbing a wall** (the empty hand at it, event 50; the legs only run up it, from the run), in order: a climb start (hand holds 1.8-2.4 m up and
  foot holds 1.2 m below them) → `xx_h_wait_hipm_foot?_tr_climbing_1m` with the root interpolated onto the holds;
  else a ledge 0.53-3.0 m up → the standing straight jump at it (0xB21DA0: knee and waist heights pull up onto the
  top, higher ones end hanging). The grab probe radius is 0.75 m (IHuman vt136). **[B]** The jump's bands **[V]**
  2026-10-24, by the top's height h over the feet: under 0.7 m action 0x012B291B (weight (h − 0.5) / 0.2), to 1.5 m
  0x01290ECF ((h − 0.7) / 0.8), to 2 m 0x01272A69 ((h − 1.5) × 2), to 2.5 m 0x01271639 onto the top or 0x01271631
  hanging (a target of type 8), from 2.5 m 0x0121A598 / 0x0121A8B1 hanging ((h − 2.5) × 2, full at 3 m); from 1.5 m up
  the 0x516D52Dx set instead unless the playing action is 0x59 or 0x01099C96 (the static jump's); with its third argument
  set (only from 0xF70E50, the beam autoclimb, event 17), 0x6DA7646E (or the other foot's) by (h − 0.2) / 1.1 over
  0.2-1.3 m. Ours plays the low bands as the obstacle's (`collide_full_footl_<050|070>cm_to_freestep_*`, the lean's
  `_to_hangknee_footl_<070|150>cm`).
- **Character proxy**: capsule radius 0.4 (0.35 + 0.05 keep distance), height 1.8, floating 0.37 m over the feet
  (step), stick-to-ground cast 0.58 m (0x57D240). **[V]** (radius and height 2026-10-25, below)
  **[V]** 2026-10-25: the capsule's owner (vtable 0x168A58C, ctor 0x52E700: radius +0x470 0.5, height +0x474 2.1, base
  +0x478 0 by default) rebuilds its shape in 0x52E9C0 whenever the radius (0x52ED40) or height (0x52ED20) is set: radius
  r = +0x470 × scale (at least the keep distance + 0.001), the shape's own radius r − keep distance (0.35); the segment
  from base + r up by height × scale − 2r, the height less the float (+0x64) when both bytes +0x7F and +0x80 are on (on
  the ground). Entering the ground (0xDA7D20) sets 0.4 and 1.8: on the ground the segment runs 0.77 to 1.4 m over the
  feet with the 0.37 float, the capsule 0.37 to 1.8, as ours (`level::BODY_*`). The height alone changes elsewhere:
  1.0 crouched (0xD859D0, back to 1.8 in 0xD85BE0 / 0xD85A90 / 0xD85DD0), 1.0 in a hang (0xDD2CA0) and 0xDA2E20, 0.9 on a
  kiosk with no float (0xE3B480, the height kept at +0xB4 and put back by 0xE3AC00), 1.8 again leaving the air
  (0xE03DE0). 0xDA7D20 also sets three fields of the controller to the radius less 0.4, 0.4 and 0.15 (+0x54, +0x58, +0x5C:
  0, 0, 0.25; ?).
  **[V]** 2026-10-24: `CharacterController`'s constructor (0x57B4F0, vtable 0x168C484) gives defaults (keep distance 0.05
  at +0xCC, gravity −9.8 at +0x38, +0x50 70, +0x60 1.0, +0x64 0.51); its fields are not serialized (2 of its 23
  properties are, neither of these). Entering the ground (0xDAE6E0) sets them: +0x60 = 0.58 (0x5782C0) and +0x64 = 0.37
  (0x578290), with the bytes +0x7F and +0x80 on. The stick-to-ground cast (0x57D240) runs +0x60 down, plus +0x64 with
  +0x80 set, times the entity's scale (+0x7C), and keeps the hit within +0xCC: Banned445's 0.58 m cast and 0.37 m float.
  The radius and height are still his.

### The edge events (69, 70, 119) **[V]**
- Posted by three methods of `HumanGround` (its vtable 0x016FFEFC, slots 186 / 191 / 385: 0xDB6340, 0xDB63D0,
  0xDB6AD0, each through the dispatcher 0xDB48B0) and handled in `HumanGround`'s event handler 0xDB1470: an event id,
  then guards in order, the first that passes switching the ground's state machine. The payload is an edge report:
  +0x10 the point, +0x20 the edge's normal, +0x30 the drop, +0x38 a flag (the guards want it clear), +0x40 a range
  (squared), +0x44 a byte.
- The report's direction (0xD9D7F0): nothing unless the drop is over 1.3 m and the edge within the range; then by
  the angle from the facing to the normal: within 60° → 1 (ahead), 60-120° → 3 or 4 (to one side or the other),
  past 120° → 2 (behind); with +0x44 set, only ahead or behind count.
- **69**: an edge to the side (3 / 4) → the ground's state 12, filled into the context data at +0xE40 (the one the
  mid-air catch also fills for posts and beams: the narrow-object context's, likely: walking along the edge) [?];
  else an edge ahead, the drop over 2 m, not grabbed (0xC7F150) and room for a 0.5 × 2 m body (0xB2E4F0) → 0xDA99F0
  (the stop at the edge, turning to its normal when +0x44 is set).
- **70** (pull-down): the flag clear, a profile bit (+0x3C bit 5) clear, the edge's normal along the facing, the drop
  over 2 m and room for the body → the ground's state 9, the report copied into the ledge context's data (+0xC50):
  the pull-down into a hang.
- **The senders are the ground's input interpreter** (0xEE65A0), through `HumanGround`'s vtable in pairs: an even slot
  asks whether the event would pass (it runs the guards, 0xDB48B0, and returns true on 2), the next one sends it
  (0xDB4A40): 184/185 event 68 (0x44), 186/187 69, 191/192 70, 385/386 119. In the interpreter, in order:
  - **119, look down**: an edge flagged for it (interpreter +0x1050) with more than 2 m under it, stood still for more
    than 0.25 s (a timer, ticks / 30000), and the player's abilities allowing it (`LookDown`, bit 29: see below).
  - **69, ledge stop**: an edge report, queried within 0.15 m (0.0225 squared), sent only when it is ahead (direction
    1) and the drop over 5 m.
  - **68**, when no jump was just started (+0x112F): an edge report → sent at once (handled by a sub-state; with the
    +0x112B flag set it is sent with 1.0).
  - **70, pull-down**: an edge report and the abilities allowing `Climb` (bit 4), then either the legs
    pressed within the last 0.2 s (button 3, the empty hand, 0xED4A20) or the stick within 70° (1.2217 rad) of the edge;
    otherwise the interpreter only
    marks the frame (+4 = 0x1D).
  - The **jump** asks the abilities too (`Jump`, bit 0) before `JumpToGuidanceTarget`. These "masks" are not the
    playing item's: 0xEF0320 asks the player's ability stack (`BhvAssassinData` +0x290: the rank's `AssassinAbilitySet`
    and any set a zone or a mission pushed), each set needing `AllActions` (+0x14 bit 1) and the flag (+8, 64 bits).
    The interpreter also asks `PassOver` (2), `Walling` (3), `Ladder` (5), `LeapOfFaith` (15) and bit 31 (unnamed), and
    the in-air catch `Grasp` (6, 0xEDBC26). Every rank allows the movement ones at sprint; ranks 0 to 3 lack `Grasp`
    (`forge::ability`, ours: `AC1_RANK`).
  Where they are handled **[V]** 2026-10-24: the ground's state machine (0xDB48B0) asks its current sub-states first,
  then its own top: event **68** at the top, its guard 0xD84190 (the step off): a forcing flag (report +0x41) passes at
  once; it fails held by a grab (0xC7F150), with the upper body in its modes 2 or 3 (0xB31260 / 0xB319D0) or a ground
  flag (+0x261 bit 0); then a drop of 0.53 m or more and, for a guidance edge (+0x38 clear), the feet past the edge's
  line (+0x34 under -0.01) and room for the body past it (0xB2E7A0), else a clear box swept from 0.7 m up toward the
  edge (0.25 × 0.25 × 1 m). Event **119** (the look-down) only in a standing sub-state (0xD96A20; 0xDA7250 enters it):
  its one guard there is the upper-body state machine (HumanGround +0x580) in its state 1; the state's enter
  (0xDA5260) copies the edge report and plays the look-down (0xD9FC80). The rest of 119's guards are the interpreter's
  (above). Ours: the step off's drop, room and edge as AC1's; no grabs or upper-body modes to fail it; the look-down
  by the interpreter's rules.
  Ours: the look-down after standing still 0.25 s, the pull-down over a drop of more than 2 m (were: at once, 1.8 m);
  the ledge stop starts with the edge within 0.15 m, or a frame's run (0.05 s) at speed (it started within 0.6 m and
  the pull-down after it slid the body 0.4 m to the edge). The ledge stop is high profile only, never free running;
  let go during it, it plays out (start, end, its exit into the wait) and he stands at the edge; still pushing as its
  start ends, the pull-down (`xx_l_ledge_stop_start_footl_pulldown_front_orientation`, code-driven in the graph).
  The empty hand (Shift) at an edge with over 2 m of drop, either profile, walking or standing, pressed within 0.2 s:
  straight down into the hang with the look-down's turn (`xx_l_ledge_lookdown_front_pulldown_front_orientation`, AC1's
  pull-down type Wait, event 70), no ledge stop; a pending press keeps the ledge stop from starting.

### What the ground does with the input, in order (the interpreter 0xEE65A0 and the ground's update) **[B]**
1. **Stick let go**: at jog or faster (high profile) → the run stop (`RunStop` 0xD98E30, guard 0xD7EC90), its root
   motion, then its settle into the wait; in the walk band → the wait with a 0.2 s blend, stopped at once (Idle
   0xD8B220). After a landing the landing's own exit leads to the wait (no second slide).
2. **Stick pulled back** (more than 135° off the facing) at a run in high profile → the run stop (a skid), then from
   standing the pivot: the skid turn.
3. **Pivot** (state 25, 0xDA6150): the wanted heading more than 90° off, from standing or from a low-profile walk → a
   turn clip by side, [from, to] profile and leading foot, blending its 90° and 180° clips by (|a| − 90°)/90°; the
   clip's root yaw turns the body.
4. **Low-profile edge halt** (0xEE7BDA): an edge in the stick's direction within 0.16 m, a drop of more than 2 m, its
   normal within 70° of the facing → the wanted speed is zeroed: the walk stops at the edge, no clip (2-5 m drops;
   deeper ones get the ledge stop).
5. **Start from standing** (0xD98990) when the stick moves: `xx_<l|h>_wait_<foot>_tr_<l_walk_slow|l_walk|h_jog>_<other>` (actions 0x09A08AC6 / 0x09A0A426 low, 0x09A0AA2D / 2E high), mixed by the speed band, then the gait on the other foot. **[B, ours]** Ladder (event 38, guard 0xD83970 → 0xB239D0): within 3 m, facing within 90°, the top's variant when the feet are more than 1.5 m off the ladder's base. **[V]**
6. **Wall run** (event 49), tested before the jumps and the grab: high profile, the legs pressed, the stick within 45°
   of the facing, and the wall test 0xE18390.
7. **Beam** (event 72): a beam in the box ahead (±0.75 m across, 0-1 m up, ±0.53 m) while moving.
8. **Ladder** (event 38): a ladder within reach, the character in front of it within 90°.
9. **Grab / climb start**: high profile + legs into a wall (section 3).
10. **Ledge stop** (event 69): a front edge within 0.15 m with more than 5 m of drop, not free running (free running
    jumps first).
11. **Pull-down** (event 70) at an edge.
12. **Jump** (`JumpToGuidanceTarget`, IHuman vt24): high profile + the legs buffered + the stick past the dead zone →
    the best target (section 6); free running with the legs held also jumps at an edge (Banned445's model of it).
- Busy: while the playing item is locked (gate 0x20) none of these start; a one-shot that allows leaving for moving
  in this profile is left at once for the locomotion.

## 4. Climb (`HumanClimb`)
The climbing is a grid, not free placement:
- Every wait frame a **hold grid** is built around the feet (`BuildHoldGrid` 0xDF6A40): columns 0.75 m, rows 0.6 m,
  7-8 each way. Cells are foot cells; the hand of that side holds the cell two rows higher. A cell holds when a
  guidance edge passes through its probe box: 0.375 m along the wall, 0.3 m up or down, and **1.0 m in or out** of
  the grid plane (edges up to 45° off level; each edge piece shortened 0.1 m at both ends). **[B]**
  **[V]** in the function: cells 0.75 × 0.6 m, rows counted from 1.5 m under the feet, the probe centred on a cell
  (column·0.75 + 0.375, row·0.6 − 1.5 + 0.3) with a depth of 1.0, the grid 5.25 × 4.8 m (7 × 8 cells); a hold found
  is snapped to its cell (rounded) and kept only within 0.3 m along and 0.15 m up or down of the cell's centre.
- **Poses** (0x1A2CD70): 0 hands level ("1m"), 1 left higher, 2 right higher, 3 a column apart level ("2m"), 4 / 5
  apart with the left / right higher. Each pose has its wait action (0x012DA39F + pose). **[B]** **[V]** 2026-10-25
  (0xDE4F80): a pose's 0x60 bytes start with its wait action, three flags (a hand raised, a column apart, which hand is
  up: 1 left, 2 right) and the hands' cells (column, row): the left's (3, 2), or (3, 3) raised; the right's (3, 2),
  (3, 3) raised, a column on at 4 in poses 3-5.
- The stick is quantised into 10 directions (`QuantizeStickDirection` 0xDEB8D0: up and down split by which side
  of straight they fall, the diagonals at 22.5-67.5° and 112.5-157.5°, sideways 67.5-112.5°). **[B, V]** (2026-10-24:
  the signed angle off the climb's up, within ±22.5° directions 0 / 1 by its sign, then 6 / 7, 4 / 5, 8 / 9 by side, and
  past 157.5° 2 / 3)
- A move is looked up by (pose, direction) in the **short** table (0x1A2D070), or first in the **long** one
  (0x1A2D7F0: hand over hand 1.2 m, both sides shuffling a column) when the stick is past 0.5 (`ChooseMove`
  0xDFDE90). An entry is (next pose, which side moves, columns, rows) or a redirect to another direction; each has its
  action (`xx_l_climb_*`). **[B]** **[V]** 2026-10-25: the tables are written by a static initialiser (0xDE4F80, over
  defaults from 0x163AC90: kind 0xF "none", action −1); an entry is 24 bytes (kind: the next pose, or 9-14 a redirect
  to direction 4, 5, 0, 1, 2, 3; the left hand's columns and rows; the right's; the action), indexed pose·10 + direction.
  All 120 entries and actions match ours (`climb_table`). ChooseMove follows one redirect.
- A move is valid when the destination foot cell and the hand cell two rows above both hold (`IsGridMoveValid`
  0xDECD70). It plays its action while the root is interpolated over the action's length (0.5 s without one) to
  the **root computed from the four holds** (`sub_B1CC20`: 0.5 m out from the feet holds along the climb facing, the
  frame tilting with a leaning wall) (`StartMove` 0xDFA0C0). **[B]** **[V]** 2026-10-24: IsGridMoveValid tests each
  moving side's cell (its column and row offsets from the pose table, 0x1A2CD80 + pose·0x60) in two cell arrays 16 apart
  (+0xC4 and +0xD4: two rows of 8, the foot cell and the hand cell over it); StartMove takes 0.5 s when the move has no
  action (id 0 or −1), else the action's item length (0x507650), and the pose's wait from the pose table (0x1A2CD70 + pose·0x60).
  `sub_B1CC20` read the same day: the root 0.5 m back from the feet holds' middle along the pitched facing, onto the lower
  foot's plane, and across half way from the feet's middle to the mean of the hands' and feet's middles, held within 0.1 m
  outside the feet: ours' `climb_frame`, term for term.
- No move found with the stick held → the look-around toward it (`OnNoMoveFound` 0xDF4410:
  `xx_l_climb_1m_lookaround_<up|down|left|right>`). **[B, ours]**
- No move up → into a ledge hang (`TryTransitionToLedgeHang` 0xDF4BA0); legs → release; reaches onto other climb
  holds or a ladder across a gap (0xDF0050, 0xDEF3A0); corner climbs (0xDF29E0); the jump up from the climb
  (`TryBackEject` 0xDF2F50: search 2.25 m over the root, 0.8 m out). **[B]**, the back eject's 2.25 m / 0.8 m
  **[V]** 2026-10-23.
- Climb → ground below: `xx_l_climb_{1m,2m}_tr_h_wait_hipm_footl_a/b` (0xDF1D20). **[B, ours]**
- `ChooseMove`'s checks on a table move: the body's boxes at the pose it ends in (`Human__ClimbBodyBoxesFree`
  0xB2DF90, axes right / in / up from the root: half 0.375 × 0.25 × 0.5 at 0.5 m up, 0.15 × 0.15 × 0.4 at 1.4 m; a long
  move sideways 0.575 wide, 0.05 m back from its side). The lean guard (0xDF47E0) on the long moves and the short ones
  down, into a pose with one side a row higher: the lean of the hands' line (going up) or the feet's (going down) from
  up, seen from the side, positive leaning out (0xDF2CD0); refused when new − old < −30° going up (the new one taken as
  0-45°), old − new < −30° going down (the old one so taken); the long move then falls to the short one. **[V]** Ours
  tests both on its table moves.
- Climb out over the top (`TryReachLedgeAbove` 0xDF1730: dirs 0, 1, 6, 7 from a level pose): the edge is the middle of
  the two hand holds, the way in the negated sum of their normals (else a guidance edge round the root + 0.8 m up,
  0.5 m in). Room is three empty boxes (0xB2E240, the knee catch's test too; axes along the edge, in, up): 0.6 m wide
  from 1 m out to 0.3 m in, 0.35-1.8 m over the edge; a 0.2 m column over the edge, 0.1-1.8 m; a 0.2 × 0.2 × 1 m slab
  tilted 30° in, centred 0.35 m out and 0.2 m up. No floor is tested. **[V]** Ours tests the same boxes
  (`climb_out_room`), the edge from the hold the wrists are at.
- Lost grip (a hold's object gone, 0xE55A10) → InAir with action 0x1120E17B, blend 0.2. **[V]** 2026-10-23: 0xE55A10
  checks the four limbs' hold handles; the climb's switch (0xDF3300) plays 0x1120E17B with a 0.2 s blend and sets the
  fall's top 1.2 m over the root, its origin 1 (the climb).

Ours: in the six climbing poses the move comes from AC1's tables (`climb_table.rs`, the entries and action ids
from Banned445's port; each id names its clip in the action blocks, `forge` example `action_clips`): the stick's ten
directions, the long move first whatever the stick's length (AC1's interpreter writes 1.0 there every frame), redirects followed, the pose it ends in taken from the table (the
clip `xx_l_climb_2ru_u_2ru` ends in the 2lu pose, 3° from `xx_climb_wait_2lu` and 40° from `_2ru`: its name is
wrong). A move is accepted when each of the clip's end hands has a hold within AC1's grid tolerance of it (0.3 m along, 0.15
m up or down, 1.0 m deep: the strict cell fit, the same results in Damascus and every climbing scenario as the looser
box) and each foot has a hold under its hand (`IsGridMoveValid`), the root corrected onto them. Not yet: the root
from the four holds (`sub_B1CC20`); the hangs, corners, leaps and reaches keep their own candidate lists.

## 5. In the air (`HumanInAir`)
- **Jumps are not ballistic.** A jump plays a takeoff item then a flight item (both `FROMAI`), chosen and weighted by
  the target's height and distance (`Human__ComputeJumpAnimBlend` 0xB1EC40); the root follows their blended
  displacement plus a linear correction (target − animation end)·t/(T₁+T₂), so it ends exactly on the target
  (`Human__SetupJumpToTarget` 0xB20200, `HumanInAir__UpdateJumpMotion` 0xE0DEF0); arrival within 0.01 m (0xE07D00).
  **[B, ours: `jump.rs`]**
- **Takeoff**: 40 clips, 5 direction groups (front, left, right, back-left, back-right) × {front 050/300/550, down
  050/300/550, up 050/300 cm}; ground running takeoff (kind 0) 0x0A4C8C0E / 0F by foot, free-step takeoff (kind 1:
  beam, post, roof edge) 0x0112B589 / 0x0112B5AC. **[B, V]**
- **Flight by target type**: free step (type 1, also hay) 0x010DDAFA / 0x010DF0D8; passover (2); assassinate
  (0x8000) 0x21B4DC3D; surface (0x40 wall hang, 0x1000 ladder, ...) 0x011E555B; swing (0x80 free hang, others).
  **[B, ours]**
- **Bands** (0xB1EC40) by the target's type, up / near / mid / far, down to −3 m (deep variants past 3 m down): the
  free step's and the ground's (1, 2, 0x100, 0x200, 0x400, 0x8000, 0x10000) 1.3 / 2.5 / 5 / 7 m; hay (0x800) far 6 m
  (and the leap of faith's own past 3 m down); a surface's (0x40 wall hang, 0x1000, 0x2000, 0x4000) 2.5 / 2.5 / 5.5 /
  7.5 m; the rest (0x80 free hang, swing bars) 3.0 / 2.5 / 6 / 8 m; the jump off a swing bar (takeoff kind 3) lifts its
  band 0.7 m. Takeoff and flight weigh on the same band. **[V]** 2026-10-24 (ours had the free step's for every jump:
  `jump::Band`, `running_in`).
- **The leap of faith's actions** (0xB1EC40, type 0x800 from the ground or a narrow object with the target 3 m or more
  below): the takeoff 0x23A949B7 (0x23A949B1 off the other foot), the dive 0x23A949B2 (`xx_h_faith_jump_<100|800>cm_long_
  <300|3000>cm_down`), and for its fall 0x23A949B5 `xx_h_faith_jump_fall` where every other jump has request 33's
  `xx_h_jumpfalling01`; bands −3 to −30 m down, 0 to 7.5 m on. The hang's let-go over a haystack (0xDD7810: the release,
  0x1120E17B / 0x1120E179, the hay its target) goes, once the release is played out, straight into 0x23A949B5 with a blend
  0.6 of the time left to the hay (0xE0DEF0), no dive. **[V]** 2026-10-25; ours since (it stretched the dive over the fall,
  the let-go's too).
- **What the bands measure** (0xB1EC40, 0xB1F0A0 / 0xB1F480): the target's own point (a hold's grab point, not the
  hang's root under it; the free step's landing point) pulled back 0.5 m toward the start (times entity +0x7C, 1 for
  the player), its flat distance from the start and its height over the start's root. The hang types' aim is moved
  onto the hang afterwards (0x40: 0.5 m out along the target's vector, 1.1 m down; 0x80: 0.5 m down, 0xB1F60E), with
  the weights already set. **[V]** 2026-10-24: the bureau courtyard's eject onto the facing wall's hold, 1.28 m over
  the root, ran AC1's takeoff 0.35 s and surface flight 0.28 s as this gives (h 0.59); ours measured from the hang's
  root (0.16 m up), leant on the long flat clips (0.38 / 0.33 s) and arrived 0.8 m behind (`ac1_jump_measure`).
- **Over-drop**: a target more than 5 m below (3 m for the leap of faith) is aimed at 5 m down, then a free fall
  (g = 9.8, its steering's change ≤ 15 m/s²). Plain falls drift (decays 4 m/s², ≤ 5 m/s). **[B]** The thresholds (0xB1B8C0: −5 m, a
  haystack's −3 m, its pull-back min(d, 1.5)·k with k at most 2) and the drift's 4 / 5 (0x19BA434 / 438) **[V]**
  2026-10-24; and the 15 **[V]** 2026-10-24: not a speed but the steering's change per step, the tail's flat
  velocity turned toward the real target by at most 15 x the 1/30 s step (DAT_0192DDA8, 0.5 m/s) a step, its fall 0.326 m/s a step
  (9.8 x 1/30; in 0xE0DEF0's over-drop tail, near 0xE0FB10). Ours flies the faith dive along its solved path, so nothing
  of ours steers that tail.
- **Arrival** (0xE07D00): free step → the reception `<flight>_tr_freestep_entry_foot?[_fast]` (0x010DE1FE /
  0x010DF150), into the run, walk, wait or the beam; wall hang → `xx_h_air_surface_tr_hangwall_reception_*` (action 0x011FF16A: **[V]** 0xE02790 weights its six clips by the wall's lean under the edge, measured about the body's side: leaning out → `straight` to `30_out` (full at 30°), sloping in → `straight` to `45_in` (full at 45°); the `max` set for a jump over 6 m or a second measure over 2.5, else `min`; the clips carry no yaw); free
  hang → the swing reception; post / beam → their mounts (0xE50190, 0xE52AD0). **[B, ours]**
- **Ground landing** (`HumanInAir__SetupToGround_Landing` 0xE05940): over 3 m of drop from the jump or fall's start
  the damage landing (`xx_h_landing_damage_footl`, or `_roll` with the stick held) with a camera shake of (drop − 3)/7.
  Damage: the player is the flagged mode (+0xE2 & 8): a fixed table, over 8 m 10, 10 m 20, 12 m 40, 14 m 80, 16 m 120,
  18 m 160, fatal over 20 m (of 200); the 6.3 / 7.0 m limits (0xE00FE0) are the non-player default (audit AIR-1, AIR-18).
  Up to 3 m: forward when the impact velocity comes in within 75° of level, else straight (not by the stick, AIR-15)
  (`landing_forward_{soft,hard}`; the port reads them going on by the pad's speed bucket [< 0.2, < 0.5, < 0.9, else]
  into the wait, walk, jog or sprint impulsion, and so does ours; let go, the wait). The traces name only the landing
  action's first clip (the soft walk one) whichever of its six play, but the `_a` lengths tell the sprint's apart (hard
  0.33 s, the others 0.27): free running at 1.1 m/s the running game left it 0.317 s in, the sprint's mix (route
  `wall0_free`, three times). **[B, traces]**
- **Catching** while falling (0xE0A990 / 0xE0AC70): a hand box 0.4 × 0.3 m, edges within 70°, reach +1.4 m (the ledge
  search, giving the wall hang when the feet find the wall) / +1.95 m (the free-hang search); posts and beams are caught
  too (0xE0BB70); ladders (0xE04100); the knee catch (`xx_fall_tr_hangknee_*`); the edge landing (a lip with a drop past
  it: `xx_fall_step_off_*`, no landing). **[B, ours]**

## 6. Jump targets
- AC1 always jumps at a target; with no candidate it builds the free-jump target (0xB1E7F0: 8 m on, 3 m down) and
  jumps at it, flying the beam set's `xx_h_beam_air_*` (audit JUMP-1, JUMP-11). The candidates are scored by
  `0xE96BF0`: within 45° of the wanted direction, no lower
  than 3 m down, the **highest** first, then the nearest. **[B]**
  **[V]** More exactly, the scorer keeps a best candidate per kind (by the candidate's type bits): free-step targets
  (type 1, or 0x10000 with sub-type 2) the **nearest**; the hang-type ones (type bits 0x80 with sub-types 0x10/0x20)
  the nearest unless another is more than 2.5 m higher; the rest (within 45°, more than 3 m down excluded) the
  **highest**. At the end a hang-type best wins over the nearest free-step one when it is at least as high and either
  nearer or 2.5 m higher (target type 0x80 returned); else the free-step one. So onto tops, posts and beams AC1 jumps
  at the nearest in the cone, not the highest.
- The candidates are guidance: LedgeGrab edges facing the player (a roof edge: land just in from it (ours 0.05 m,
  measured; Banned445's 0.45 m is his placeholder; the blend aims 0.5 m short, the reception carrying the rest) if at most
  1.3 m up; a ledge to hang from if higher, wall hang when a wall is under it, else free), thin wall tops (passover),
  beams, posts, ladders, bars. **[B: a reduced port; the game's own candidate list (IHuman vt56/64/68) is not
  traced]**
- The world also carries precomputed **jump links** in the navigation meshes (`NavMeshMetaLink`, types
  `MetaLinkTypeID_JumpLink`, `_JumpLinkUniDir_0_1 / _1_0`, `_ObjectJumpLink`, `_ObjectToObjectJumpLink`, ranges
  `WorldArea::JumpLinkRange_Normal / _Extended`). **[V: the names are in the exe]** They are the path follower's
  **[V]** 2026-10-24: `scimitar::JumpLinkSurrogate` (vtable 0x016FF394, built by 0xD70A70 for the AI's paths, 0xB38BC0 /
  0xE5C760 / 0xE5C790) takes the jump at a link through the same candidate finder, its slot 6 (0xD73D30) calling
  0xE18970 with the general volume (the "other callers" below). The player's jumps reach the finder only through
  0xB13260 and never read the links.
- **[V]** The player's candidates come from the guidance, not from those links: the ground interpreter (0xEE65A0)
  asks the character (`IHuman`, Human's vtable at 0x016D91B4 for that interface, slot 0x38 → 0xB13260) for them;
  0xB13260 queries from a point 0.15 m along the wanted direction, 8 m range, with a bound 9·(character scale), query
  flags 5 (0x15 in one character mode), in `0xE18970` (107 KB: the full candidate finder; it calls the guidance
  queries at 0x1173590 seven times; its constants include 0.45, 1.1, 1.3, 2.5, 2.7, 3.0, 3.1, 3.7, 4.2, 4.3 and
  cos 40°). The scorer 0xE96BF0 then picks among them. Reading 0xE18970 in full is the next step for exact targets.
- **[V]** 0xE18970's search volumes by query kind (its 7th/8th arguments): the general one up to 3.0 m above and 5.0 m
  below within 9 m; kinds 1 and 2 (and kind-7 = 4) 2.7 m up, 5 m down, 4.2 m; kind-7 = 3: 3.7 m up, 4.3 m down, 9 m,
  the probe 0.7 m higher; kind 4: 0.45 to 1.3 m up within 2.0 m; kind 3: 0.45 to 1.3 m up within 1.3 m (waist-high
  tops just ahead: the step-up and vault). All boxes start 0.5 m ahead and are ±1 m across (±0.5 m for kinds 3 and
  4), laid along the wanted direction from the body's mid-height (pos + 0.5·up). The other callers, 0xD72F60 and
  0xD73D30 (three calls), pass kind 0 and kind-7 0 with query flags 5: the general volume. Which kind the wall's
  rebound 0xE365C0 and the beam's 0xEE8EC0 pass is still to map. **[V]** The ground's jump (the interpreter's call at 0xEE73ED) passes kind 0 (and a
  character field, Human+0x994, as kind-7): the general volume, 3.0 m up, 5.0 m down, 9 m; the scorer then keeps
  what is no lower than 3 m down.
- **[V] How 0xE18970 classifies each guidance edge** (read in full, 2026-10-08). The guidance in the box is gathered
  by three 45° sector queries (0x116CD50) around the wanted direction; edges of a kind masked out by the caller
  (argument 13, a bit per edge kind) are skipped. For each edge left: P = the point on it nearest the query, n = its
  outward normal (horizontal), h = P.z − start.z.
  - Facing: an edge not crossing the query is dropped unless n · dir ≤ −0.707 (it faces the jumper within 45°).
  - Volumes: P must be in the primary volume (then "on line") or one of two wider ones; a point in the excluded inner
    volume is dropped.
  - **Top depth** (+0x50 of the record): from Q = P + 0.5·n + 0.05 up, a sweep back along −n 1.75 m; depth = hit − 0.5:
    < 0.03 → 0x40 (no top: a wall goes on up), < 0.3 → 0x10 (narrow), < 1.0 → 0x08, else 0x02 (wide). Edge kind 4
    → 0x20; kind 7 → 0x02.
  - **Under the edge** (+0x54): from Q a sweep down 3.05 m, drop = hit − 0.05: < 0.5 → 0x02 (a step), < 2.0 → 0x04,
    else from P + 0.5·n − 1.0 up a sweep along −n 1.4 m: wall within 0.7 m → 0x08 (a wall under it), else 0x10 (over
    nothing: free hang). Kind 7 → 0x01.
  - **Moves allowed** (+0x58), starting from 0xC3 (kind 3: 0x40 only; kind 4: 0xC2; kind 7: 0x200 on line; kind 8:
    0x400 on line): 0x01 free step onto the top, 0x02 run onto the top, 0x40 wall hang, 0x80 hang. Dropped: 0x02 if
    h < −0.5; 0x01 and 0x02 if not on line; 0x40 if in no volume. Unless the top class is 0x40: 0x01 if the top is
    narrow (< 0.3 m); 0x02 unless it is wide (≥ 1 m); 0x80 if the drop in front is under 2.5 m; 0x40 and 0x80 if the
    edge is under 2 m above what is in front (a step or low wall); 0x40 over nothing.
  - **Clear way**, from the chest (start + 0.75 up): to P + 0.2·n for the top moves (0x10703; a swing bar uses its own
    sweep 0xE1C720), and to P + 0.2·n − 1.1 m (the hang's chest) for 0x40 and for 0x80: a hit short of it drops them.
  - An edge with any move left is a candidate (point, edge end, normal, the three words above). Objects ≥ 2 m long
    (the second list) give 0x1000; with argument 12 the beam detector (0x680260) adds beams (0.3 m checks, 1.0 m and
    1.3 m clearances).
  Ours: roof-edge targets from ledges (0.45 m in), the clear way from above the higher top, hangs aimed by the wall
  below (`jump_hold_target`). The depth classes, the 2.5 m drop for a hang and the chest-height clear way are AC1's
  rules still to adopt.
- **The passover's endings** (`HumanLedge`, code-driven): over and down (`xx_h_passover_<hand>_<cm>cm` + `_tr_fall`);
  over a big drop, round into the wall hang on the far side (`xx_h_passover_<hand>_tr_hangwall_a/b`, 0x0109BB59: 0.6 s,
  1 m down, turned 180°; ours when the far side drops more than 2 m and its top edge has a hold); the pull-down over it
  (`_pulldown_soft[_orientation]`, ending 14° from `ledge_pulldown_soft_to_hangwall_straight_b`) and the jumps on from
  it (`xx_h_passover_<cm>_<hand>_<down|front_down>_<050|300>cm_<foot>_to_air`, HumanInAir, into the down landings) are
  not used yet. The game lacks `xx_h_air_up_300cm_footr_to_passover` (only its reception): ours goes off the other
  foot then.

## 7. Ledge (`HumanLedge`)
- Hangs on two hand contacts on guidance edges. Wall hang: root 1.1 m below the hands, 0.5 m out; free hang: 2.4 m
  below (`ComputeRootFromHandTargets` 0xDD6730). Hang type: wall if a foot finds support (two 1.2 m rays toward the
  wall starting 1 m below the hands, 0.5 m back, 0.1 m to either side), re-evaluated after each step (0xDE1060).
  **[B, V]** (2026-10-24: 0xDD6730's 1.1 / 0.5 / 2.4 m; the foot rays 1.2 m long, 0xDE0A60)
- Moves are discrete (new hand targets, the root interpolated over the clip): shimmy with alternating hands (free
  space swept with a 0.15 m sphere over 1.55 m; step = min(d − 0.4, 1.0 − hand spacing), blocked under 0.7 m, at
  least 0.15 m; **[V]** 0xDD9640, see "The shimmy's widths"); up/down: into the climb when foot holds 1.2 m below exist, else a hand step to a ledge 0.6-1.2 m
  away, else the swing back up to a ledge out from the wall (0xDD62A0, `hangwall_to_swingback_up_*`, ending in a free
  hang), else the pull-up if there is room (0xDE2270); corners
  (0xDD3BB0; **[V]** 2026-10-24: one of eight actions, 0x1E67E881-884 from the free hang and 885-888 from the wall hang, by
  side and inside / outside; both hands' targets go onto the corner's ledge, the root from them (the wall hang's leaning
  0xB157B0, the free hang's 0xB15AD0), then the hang's body boxes there (0xB2DC80 / 0xB2DD50) and the way round
  (0xB2A410, 1.1 m for the wall hang, 2.4 m for the free; ours tests points along the turned root's path), then the root
  interpolator over the action, 0x711130), side jumps between ledges (0xDDD490; its sweep 0.2625 m back and 0.15 m out, 1.8 / 1.0 / 2.0 m tall to
  the climb, wall and free hang, **[V]** 2026-10-24); lost ledge (0xDD20D0) → InAir. **[B]**
- Hang → climb: `xx_h_hang{wall,free}_{u,d}_climb_1m` and the sideways `{ul,l,dl,ur,r,dr}` variants (0xDD46A0,
  0xDD48B0). **[B]**

## 8. Beams and posts (`HumanNarrowObject`)
- From the ground on event 72; from the air a free-step jump mounts a beam (`TryMountBeam` 0xE52AD0) or a post
  (`TryPilotisFreeStep` 0xE50190); a fall onto one is caught. **[B]**
- On a beam the beam actions' root motion is projected onto the beam line (`ConstrainRootMotionToBeam` 0xF7C3A0):
  the walk **stops 0.3 m before an end**, and steps off an end with free space beyond (within 0.16 m of the end, a
  free capsule at end + 0.5·dir, `CanStepOffBeamEnd` 0xF77C00). Stick vs facing: within 75° forward, past 135° back
  (turn). **[B]** The 0.3 m stop (0xF7C3A0, from the end along the beam) and the step-off's 0.16 m and end + 0.5·dir
  (0xF77C00, the capsule from 1.2 m up) **[V]** 2026-10-24.
- Sub-states: idle, walk, stop, the 90° wait (facing across), turn 180, the turns to and from the 90° wait; exits:
  the pull-down to a hang (event 9, the empty hand), the wall run (event 15, walking), the corner hop onto a beam
  beside (event 7). The edge stop (`xx_l_beam_edge_stop`) and NarrowObject → ladder are not used by v1.02. **[B]**
  (Ours stopped playing the edge stop: AC1 just stops 0.3 m short.) Starting along a beam from the crouch plays AC1's
  start (`crouchwait_<foot>_tr_crouch<walk|jog>_<other>`, 0x34662CB4/B5) and letting go while jogging its stop
  (`crouchjog_stop_<foot>` + `_tr_crouchwait_<foot>`, 0x3466339D/9E). **[ours]** The corner hop (event 7) **[V, ours]** 2026-10-26: taken by
  NarrowObject's walk (0xF7F150) only while the speed (+0x6B4) is over band 1's (the crouch walk's), as the jog stop; the
  hop's landing (`_tr_jog_<foot>`) takes it again past half of it (guard 0xF6FA50: the playing action's phase over
  0.5). Ours from 2026-10-26 (both: it took the hop by the profile, and only from the jog after the landing). Gauntlet
  G26 (three beams in a zigzag).
- **At a crossing, stopped** **[ours]**: crouched across his beam (the 90° wait) with another beam within 0.5 m running
  his way, the stick forward walks on along that one (`crossing_beam`), the walk's start faded from the 90° wait. AC1's
  corner hop is from the jog only and the 90° wait has no clip onto a crossing beam; how AC1 gets on one from a stop is
  unchecked. Before, he turned to face it and stayed (user recording 142770, a Damascus T; scenario `Damascus beam T`).
- **Side entry** (0xF7AAA0, actions 0xE6E0E9B8..BB): met across a beam (30-100° off the way the stick points along it)
  → `xx_h_freestep_entry_<foot>_tr_crouch<walk|jog>_<foot after>_<left|right>_<30|90>`, the 30° and 90° clips blended
  by the angle; a left turn goes on on the right foot, a right turn on the left. The clips' root faces along the beam
  (no root turn in them) while the body starts turned off it by the angle (their Reference bone at the first frame 30° /
  90° off the straight entry's, all ending alike), so they play with the root along the beam: from the facing he met it
  with, the 1.7-2.7 m they move went across the beam and off it. Ours plays it on getting onto a beam with the stick
  across (`WallClimb::perch`) and after a jump's arrival onto one (`..._tr_freestep_entry_<foot>_fast`) with the stick
  turned along it in the flight, then the crouch jog/walk along it with no quarter turn (G29). The fade into it starts
  from the pose before turned back under the root's turn, the Reference bone's place too. **[B, ours: G29]**
- Where beams come from: `GuidanceBeamDetectorAccurate` (vtable 0x0169A1CC, constructor 0x678F80) is never loaded
  from data (no objects of the class in any forge); it is built on the stack with fixed values (1.0, 0.15, 5° =
  0.0873 rad, 0.5, 0.2) by 0xC7E810, which runs lazily when a query box overlaps a guidance object's bounds
  (0xC7EAA0, AABB test 0x61E880) and caches the result. It does not search city geometry: 0x680260 cuts the guidance
  object's own shape (object +0x170) into beam segments (start, end, half widths, the two side normals), and
  0x67DA30 builds each beam's volumes: the top and both sides, tilted ±5° about the beam's line, 0.15 m past its
  half width, 0.5 m along the up normal. So city beams are the authored guidance beams, which we already load (937 in
  Damascus → 1141 perches). **[V]**
  Since 2026-10-14 ours builds a city's beams that way (`beam_pairs`) and its posts by `sub_B2B600`: see "Beams as AC1
  does them".

## 9. Wall run, ladders, hay, swing bars
- **Wall run over a thin wall**: removed 2026-10-10. AC1's walling classifier (0xE341D0) never returns the pass-over
  class, so the `xx_h_wallingfront_*_tr_passover_*` exits are unreachable; a thin wall's top is knelt onto
  (`tr_hangknee_131cm`) or hung from (audit WALL-19).
- **Wall run onto a ladder** **[ours]**: sprinting at a wall with a ladder up it, the wall run comes first (as the
  interpreter tests it before the ladder) and ends `xx_h_wallingfront_step1_footr_tr_h_ladder_up_l` (7° from
  `xx_h_ladder_climb_up_l`'s start) on the ladder.
- **Ladders and haystack boxes** **[V, ours]** (the running game at the Damascus bureau): moving into a ladder's foot
  from 0.6 m in front gets on with no button (the running game: 0.61 m), and walking out over its top too. Walking into a haystack's 1.06 m box, AC1 leans on it
  (`xx_h_collide_full_hand_070cm`), and pushing on 0.2 s climbs onto the rim
  (`xx_h_lean_wait_025cm_twohand_070cm_to_hangknee_footl_070cm`, `_tr_hangknee_footl`, the free-step stand) and
  dives in (`xx_h_freestep_footr_to_haystack_02`). From the hay, the stick toward a side with a drop past it goes out
  over the rim (`xx_l_haystack_wait_to_passover_handl`, `xx_h_passover_handl_030cm`) and down into a hang from it
  (`_pulldown_soft_orientation`, `_pulldown_soft`, the ledge pull-down's hang); the pull-up from there lands on the
  rim and dives back in (`xx_h_freestep_footr_to_haystack_01`). The hay heap is solid in the collision built from its
  mesh: ours leaves out the holds found on it (up to 3 m over the rim).
- **Free hangs swung from** **[V, ours]**: AC1 swings from any free hang a jump reaches (target type 0x80, the swing
  reception), not only from poles: off the beam frame east of the Damascus bureau's ladder, it free-stepped onto the
  frame's near beam (NarrowObject), jumped on 0.08 s later (`xx_h_freestep_front_front_050cm_footr_to_air`,
  `xx_h_air_front_050cm_footr_to_swing`) at the 0.2 m beam 5.6 m across, level with its feet (inside the narrow-object
  reach zone 7), and caught it. (It did not swing on: legs let go, it hung still 0.25 s into the first upswing; see
  Swing bars.) Ours (2026-10-15): no bars along thin beams; AC1's scorer's free-hang pick (0x80) on a hold edge with
  no wall for the feet under it is jumped at and swung from like a bar (`swing_hold_ahead`: the running jump, the free
  step from a post or beam, and the eject, whose three at the bureau's beam ours now matches, `_to_swing`,
  `swing_cycle_front_up`, the still hang at (47.03, 9.46)); feet on the wall take the wall reception (0xE093CC). Ours keeps the
  swing to holds with no top behind them (`top_behind`: no ground at the hold's height 0.4 m in, a beam's side or a pole):
  a roof's lip over open air is caught into the free hang, `xx_fall_tr_hangfree_min_a/b` (user report 274414). A
  free-step jump onto a beam ends on it, its reception cut once its step is taken. The empty hand pressed in such a
  flight arms the catch but keeps the aim at the hold (ours: AC1 jumps at its target; unaimed, the fall's catch took a
  beam 1.4 m up into the knee hang mid-flight, user recording 95192). AC1's flights end only in `_to_swing`,
  `_to_surface`, `_to_freestep*`, `_to_passover` and `_to_assassinate`: a beam out of reach zone 1 (over 1.3 m up, or
  past about 5.6 m on when level) is a free hang, flown `_to_swing`. **[B, V, ours]**
- **Free running across beams** **[V, ours]** (route roof1_free_sw, off the Damascus bureau's roof south-west): AC1
  jumped onto a beam crossing its way 1.1 m on (its reach zone starts at 0.5 m; ours' targets started at 1 m, and its
  shortest clips' 1.6 m was too far to correct within 50% of 1 m: now of at least 1.5 m), then, with 1.45 m of that
  beam left along the stick's way and the stick 45 degrees off it, free-stepped on to the next beam 0.08 s later; on
  that one, 4 m of it left, it walked along. Ours jumps on first free running with under 2 m left. Walking the beam with
  the stick over 30 degrees off it, AC1 stepped off onto a roof that came up beside it, level with it; ours does too.
- **Onto a beam beside a top's end** **[V, ours]**: free running north off a raised Damascus top that ends 0.6 m short
  of a beam whose line runs 0.55 m beside the way, AC1 stepped across onto the beam's end and walked it, steering onto
  its line over 1.5 m (route seams_free_north); off the open roof north, a beam carrying on in line from the edge, it
  jumped along it (roof2_free_north). Ours steps on when a beam's near end is within 1.2 m ahead, 0.3-0.6 m to the side
  and within 0.35 m in height; walking a beam, the root comes onto its line at most 1.5 m/s across (it snapped before).
- **Swing bars** **[V, B, exe, ours]**: the jump at one (ours, 2026-10-26): AC1 flies only at a target in its reach zones,
  and at it; ours flies a running jump at a bar plainly (the run's speed, 3 m/s up, a plain fall's drift) and takes the bar
  as its target only when that flight brings it into the catch (`plain_flight_catches`), else the free jump: a bar 6.1 m
  on and 3.4 m up off the test level's block played the reach (`_to_swing`) and fell short (user recording 210302). (The
  test level's bars stand 3.4 m over that block, above AC1's zones 1-3 (3.0 m at most): AC1 would not take them from it.)
  The catch (action 0x02926510, nine `xx_h_air_<way>_to_swing_tr_swing_front_a`,
  blended as the flight it ends, Banned445's `swing::enter`), then the cycle (0x023E0C60: `front_up`, `front_down`,
  `back_up`, `back_down`). The interpreter drives it with what is held each frame: high profile and the legs (+4397), and
  the swing jump (event 7) with the stick within 75 degrees of the facing too; no press. The jump's guard (0xDCD000) takes
  it at the end of the catch (the catch ends as `back_down` does, the feet going forward) or at the end of `back_down`,
  both hands on a pole (contact kind 3, +0x700 / +0x704); it asks nothing of speed. So held through the catch it jumps off
  in one swing; not held then, a whole cycle later (2026-10-16, after ours waited out a cycle always, as Banned445's port
  does). With no target the free jump when a box ahead is free (IHumanLedge vt104, Banned445's `free_jump_room`), else it
  swings on. Caught with the legs let go (route ladder68_free), it hung still from the bar 0.25 s into the first upswing,
  no stop chain; ours settles into `xx_h_hangfree_waitclose` after that upswing. Let go mid-swing: at the end of a down
  item (event 6's guard 0xDCCF40, never in the catch), the move graph's stop for it: `front_down` into the back stop
  0x023E0C62, `back_down` into the front one 0x023E0C61, items a to d (ours had them swapped). From the still hang, high
  profile and the legs held start it again (event 5, guard 0xDD3750, a box ahead free; 0xDCF080 plays the cycle from
  `front_up` over 0.1 s); ours used the stick and `xx_h_swing_momentum_front_up`, which no code asks for. The one-item
  action 0x02F4BF04 (`back_up` alone) is asked for by no code either. A beam's side (not a pole) is no bar: one upswing,
  then the free hang. The hay's hop out
  ends in the stand of the profile held (`xx_l_haystack_hop_out_tr_<l|h>_wait`).
- **Walling** (event 49, wall test 0xE18390): entry A → B → vertical → end (0xE37590); probes A-D hand over to the
  ledge with the `wallingfront_*_tr_*` exits; the rebound (0xE365C0) pushes off straight back when the stick is
  within 50° of the facing, else along the stick. Only the front wall run exists in the data. **[B, ours]**
  The entry is warped to end 0.5·h out from where a probe 1.3·h over the feet meets the wall, h (1 m) up; a catch's
  further way to its hold is taken in the catch clip. Measured on the running game's three wall runs by the bureau:
  0.50 m out and 1.00 m up at the entry's end, ours within 0.03 m now (it was 0.08-0.2 m off when the whole way to the
  hold went into the entry). The rebound's fall, `xx_h_rebound_footr_tr_fall`, carries 0.8 m back and 3 m down in 0.47 s:
  AC1 falls the 2 m at a steady 7 m/s; ours starts the fall at that clip's 6.4 m/s down. **[B, ours, measured]**
  The catch after the vertical step (probe D) takes the highest edge 1 to 2.8 m over the root first (FindLedge 0xE36BD0
  sorts by height, highest first for probe D: its comparator 0xE33AF0), and mixes its four clips as action 0x106CFC5's
  weights (0xE38FE8): by height, (edge - root - h) / 1.5·h (the 251 cm clip to the 430 cm one), by the lip's overhang
  out from the face run up, up to 0.75 m (the `_000cm` clips to the `_050cm` ones); it asks nothing of the wall under the edge (ours checks the feet's footing
  where they go). A cap stops the run only straight over its line (within 0.2 m along the wall). On a Damascus street
  wall the running game ran up past a pilaster's top 0.28 m to the side and caught an edge 0.1 m proud of the wall over a
  window, 3.8 m up; ours now does the same (route `street_free_east` 2.54 -> 0.38 m). **[B, ours, measured]**
- **Ground landings, soft or hard**: one blend, its hard share drop / 2.5 m (0xE05940, as Banned445's port reads it; the
  trace's action names only the soft clip); ours blends the two by it (it was a switch at 7 m/s down). **[B, ours]**
- **Ladders** (context 5): actions from `HumanLadderData`'s table (+392 + 4·state + 2·foot + inclination). **[B]**
  **[V]** 2026-10-25 (its constructor 0xC7CAC0, slots 0x62-0xAD): states 0 / 1 the low / high wait, 2-5 the low and
  high climbs up and down, 7 / 8 the low / high entries from the ground (`xx_<l|h>_wait_hipm_foot*_tr_<l|h>_ladder_climb_
  up_*`), 9 / 10 the pull-downs from the top (the second slot of each foot `xx_l_ladder_wait_l`), 11 / 12 the low / high
  exits down onto the ground, 13 / 14 off the top (low into the wait, high into a free step), 15 / 16 the low / high
  let-go (`_tr_falling`), 17 and 18 both the rebound; state 6 left unset. Every one is a clip ours plays; the two slots
  of each foot hold the same action except for states 9 and 10.
- **Jumping onto a ladder** (target type 0x1000) **[B, V, ours]** 2026-10-26: the jump query lists ladders (type 1,
  sub-type 1, flags 0x1000; not for the free run's target jump, which leaves them out): at least 2 m of it, the point
  nearest 1.5 m over the feet on the part from 1 m over its foot to 0.5 m under its top, kept 1.5 m in from those ends
  (so never under 2.5 m over its foot), in reach zone 3 and in view from the chest (Banned445's `jump_candidates.rs`).
  The scorer takes it among the "other" targets, the highest in the 45 degree cone, and its type first (0x1000 heads
  the order). The flight is the surface one (as a wall hang's, band up 2.5 / down -3 / far 7.5); arrived (0xE07D00),
  AC1 plays 0x029258AA, `xx_h_air_surface_tr_ladder_verti_<up|down|long>_a` then `_b` (no root motion), weighed by the
  target in the band (0xB1EAA0: v = (2.5 - dz) / 5.5, l = dist / (7.5 v), up 1 - v, down (1 - l) v, long l v), brings
  the root onto the ladder over 0.2 s (0x711130) and enters `HumanLadder` (0xD8DE80). Ours (2026-10-26) does this, the
  root flown straight onto the rung under the target, the feet 1.5 m under it; it caught ladders only by the fall's
  catch (`xx_fall_tr_ladder_min_a/b`), the jump aimed at them only when the scorer had nothing else. From ladder 68's
  foot in Damascus, 4.6 m back, its lowest target (2.5 m up) is outside zone 3: no ladder jump from there.
- **Hay** (context 21): the faith landing (0x23A9666C, blend 0.3 s) or `xx_h_air_to_haystack` (0.1 s), the root
  moved to the hay over clamp(d/speed, 0.1, 0.4) s; hop out on the legs if the exit point (+0.5 m out, 1.25 m up)
  has room. **[B]** **[V]** 2026-10-25: the top entry (0xE41D50) plays 0x23A9666C with a 0.3 s blend, sets collision
  layer 0x1C, and moves the root onto the haystack entity's origin through the root interpolator (0x711130, its
  progress dt / time, capped at 1, in 0x7113F0) over the 3-D distance over the speed (+0x4F0), clamped to 0.1-0.4 s;
  its timer runs the action's length + 0.2 s. The side jump's (0xE42700: 0x7750D212, 0.1 s) is the same; the
  interpolator's mode 0 (the 11th argument, +0xC8) leaves the clip's root motion out, the root on a straight line.
  Ours since 2026-10-25 (`try_hay`; it spread the move to the seat over the whole clip). The ground and free-step
  entries (0xE42420 / 0xE42190) pick `_01` or `_02` at random and move the root over the action's length. The hop
  out's guard (0xE434E0) tests the spot 0.5 m on along the way out past where it leaves the footprint and 1.25 m up,
  with a box 0.35 / 0.5 / 0.75 (0xB2D2A0), as ours (`level::HAY_HOP_*`).
- **Swing bars**: a free hang reached by a jump (target type 0x80) enters the swing reception (Ledge sub-state 8).
  **[B]** **[V]** 2026-10-25 (the arrival 0xE07D00): with the feet finding the wall under the hold (0xB15C30) a 0x40
  target plays 0x011FF16A (`xx_h_air_surface_tr_hangwall_reception_*`) and a 0x80 one 0x8E5444EE
  (`xx_hangwall_reception_front_*_min_a`), both weighed by the wall's lean (0xE02790), into Ledge sub-state 6; else a
  0x80 target plays 0x23E0C60, the swing cycle itself (`xx_h_swing_cycle_front_up`, ...), the root interpolated onto the
  free hang's (0xB15AD0) over the action's length, into sub-state 8 with the free flag (+0x68) on.

## 10. Limb IK
- Limb weights rise at 4/s with a contact (0.25 s fade in) and fall at 5/s when released (0.2 s) (`LimbIK` 0xE57570, **[V]** 2026-10-24); ours reads the contacts from the clips' hash 2 track (`forge::anim::contact`, `WallClimb::contacts`).
  HumanIK's chest pull, shoulder correction and twist are off for humans; reaches stay in the limb. **[B]**

## 11. Cities: what makes Damascus different (ours)
- Damascus's walls have holds (authored guidance) that are not on the clips' 0.6 m steps, storeys set back by
  0.2-0.3 m, sills with walls 0.17-0.3 m behind them, and holds 5-6 m apart where the wall is bare. The climbing
  probe (`AC1_PROBE_CLIMB`) measures it; `AC1_CLIMB_WHY` logs why each climbing move is turned down. **[ours]**
- Our street jumps picked tops past awnings and walls (the flight stopped in the air and dropped back, over and
  over): targets must have a clear way across. **[ours]**

## The audit fixes (2026-10-10)
Twelve audits of ours against AC1 (Banned445's port, the reanalysis, the route recordings) found 205 differences. Fixed
in this batch, by audit id:
- **Buttons** (INPUT-1 to 4, 11, 12): E is the head (eagle view), never a climb. The empty hand (Shift) starts a climb
  at a wall (event 50), else the pull-down within 0.2 s; held in a fall it catches after 0.3 m (the manual catch). The
  legs: pressed in high profile at run speed with a wall within 45°, the wall run at once; let go within 0.3 s, the
  tap jump (high profile moving: the jump targets, then the leap of faith and haystacks only if none; standing: the
  static jump); low profile, only a bench's seat. A free run's wall run waits out the tap window.
- **Ground** (GROUND-1 to 3, 6 to 8; EXTRA-1, 2, 12): the heading and run stop above; the look-down latched only by the
  low-profile edge halt held 0.25 s, kept until the walk restarts; a roof's parapet beam (floor beside it) is never
  perched on from a halt; the lean starts at the lean spot (0.5 m out, on contact) and squares out from where he is.
- **Falls** (AIR-1 to 4, 6, 10 to 12): the player's damage table (over 8 m 10, 10 m 20, 12 m 40, 14 m 80, 16 m 120,
  18 m 160, fatal over 20 m, of 200); every ground landing plays a clip, over 3 m the damage landing (its roll with
  the stick held); a fall with no target drifts at most 5 m/s, easing off at 4 m/s², and a wall takes only the part
  into it; catches: the climb only with a hold under each foot, else the wall hang, else the free hang, `_max` after
  a fall of 3 m or more.
- **Jumps** (JUMP-1, 2; WALL-9, 12): the free jump aims at AC1's free target, 8 m on and 3 m down, with the beam set's
  flight (`xx_h_beam_air_*`); an eject with no target flies at that target too, and a hold beats a top by the hold's
  own height. On a level with no authored holds (the test level) the jump target falls back on the geometry's tops.
- **Beams and posts** (NARROW-1, 2, 7, 8): arriving along a beam, crouched on the other foot; across it,
  `freestep_entry_<foot>_tr_beam_air_90` into the crouch across; on a post, `_tr_beam_pilotis_wait_a`; with the stick
  along the beam, straight into the walk. A free-step reception ending on a lone beam or post goes onto it at once.
  Jumping off needs free running; the empty hand is only the pull-down (more than 2.5 m under the hold).
- **Hay** (HAY-1, 6, 10): walking into a haystack dives in (either profile, no button); hopping out goes the stick's
  way, with room 0.5 m past the rim.
- **Climbing** (CLIMB-E1, E3; LEDGE-4, 9, 11; WALL-3, 5, 6): the grid window 0.375 m along by 0.3 m up; AC1's move
  order with or without the legs (grid moves, the inside corner, the reaches, the outside corner, the top-out); the
  wall hang turns corners with its strafe pair onto the new face (AC1 has no wall-hang corner clips: actions
  0x1E67E885-88 are `hangwall_strafe_<side>_050cm_open` + `_close`), the root turned by AC1's interpolator over both
  clips (0xDD3BB0 → 0x711130: slerped at a steady rate, the clips' sideways root motion unused) about the hands, which
  go along the holds through the corner [ours: AC1 moves the root on a straight line and holds the hands on the corner
  point by IK]; it had turned in 0.25 s in place, then slid 0.85 m; the pull-up ends on the top's height, 0.45 m in from
  the wrists; the climbing stick's left and right mirror while the camera faces him; a wall run with nothing to grab
  always rebounds off.
- **Holds and ladders** (GUIDE-1, 3; LADDER-1, 4): holds kept up to 40° of slope (0.35 threw away 21,312 in Damascus);
  holds carved out round ladders; a ladder got on from 0.6 m, the stick within 90°; the legs in high profile jump off.
- **Second batch** (WALL-1, 2, 4, 14, 19; NARROW-5, 9, 17; LADDER-2, 3, 5, 9, 10; EXTRA-5, 11, 13, 14; GROUND-9, 12;
  ANIM-2, 3, 12; INPUT-7 to 9; CLIMB-E6 in part): no wall-width test and the 45-degree wall-run limit; no pass-over off
  the wall run (the thin wall's top is knelt onto); rebounds by AC1's sub-states; ejects search the tap jump's reach;
  the beam walk steers onto its line (toward 0.6 m on) and leaves an end onto the ground ahead; the legs with no stick
  on a beam or post crouch into the impulsion and jump straight up on release; ladders got on from the top by walking
  out over them, the ground entry the high stand's (rising 0.5 m), the top exit at the walkable top, low profile half
  rate with a part stick, the empty-hand drop with no push; from a stand or walk, onto a low top by the impulsion and
  free step; the ledge stop 0.25 m out; the lean's and halts' exits and stops eased over 0.2 s, by profile; standing
  stance changes play `xx_hl_` / `xx_lh_`; falls keep a takeoff's turn easing in; squaring up to a wall turns the fade
  pose back; the ground's foot IK only standing still; the pad's 0.35 dead zone, the trigger at 30/255, R3 centring;
  the top-out with the stick within 67.5 degrees of up.
- **Third batch** (AIR-7, 8; HAY-2, 3, 5; LEDGE-1, 12; GUIDE-5 behind a flag): the knee catch
  (`xx_fall_tr_hangknee_*`); the edge landing (a fall coming down on a lip steps off it, `xx_fall_step_off_*`); the hay
  wait `xx_l_haystack_wait`, turned to the haystack's facing, its seat 0.21 m along that from the entity, 0.2 s settled
  before hopping out; the wall hang's shimmy steps 0.23 m then 0.43 m (since 2026-10-24 AC1's widths from its hand
  goals, see "The shimmy's widths"); letting go cuts a shimmy at once.
  `AC1_PAIRED_BEAMS=1` builds beams and posts as AC1's runtime does, pairing facing hold edges (`beam_pairs.rs`, after
  Banned445's decode) in place of ours' mesh-name beams and single-hold tops; not the default yet (31 of 41 routes
  against 34: the jump query and beam walk were tuned on ours' beams, and the workspace's side planes cut a roof-edge
  beam the running game lands on); the default since 2026-10-14, see "Beams as AC1 does them").
- **Fourth batch** (WALL-8): an eject's takeoff turns the root away from the wall in one frame, as AC1's does (it was
  eased round over the takeoff), the hips kept where the rebound pose left them. (WALL-16 to 18) The wall run's catch
  into the wall hang (probe D) as the exe has it: the highest edge first, its height blend over 1.5 m (ours had the
  clips' own 1.8 m rise, Banned445's reading), its `_050cm` clips mixed in by the lip's overhang. (LEDGE-13) The free
  hang's drop onto the wall below mixes its four clips `{min,max}_{200,300}` by the exe's weights (0xDDF390: by the
  drop, hands - 2 m (1.6 into the wall hang) - feet, and by how far the new hold line is, / 0.7 m), the holds a row
  1.2 m under the hands and 0.65 m on (the four clips whole after it, for holds out of that reach). (LEDGE-3) Up or
  down from the wall hang, into the climbing stance only with a hold for each foot 1.2 m under its new hand (AC1's
  TryTransitionToClimb), else the hand step to the ledge there, one hand then the other
  (`xx_h_hangwall_climb_1m_<u|d>_1<l|r>u`, then `_1<l|r>u_<u|d>_1m`); ours climbed into the stance with the feet on the
  ledge just let go of, 0.6 m under the hands. (LEDGE-2) Sideways, the hang switches its type in place of the step
  where the stretch ahead needs the other one (AC1's TrySwitchHangType 0xDE1060 before StartHandStep, its foot rays as
  Banned445's port reads them: 1.2 m in toward the wall from 1 m under the hands, 0.5 m out, 0.1 m to either side): the
  wall hang into the free one (`xx_h_hangwall_tr_hangfree_<side>`) unless both rays meet the wall, the free hang into
  the wall one (`xx_h_hangfree_tr_hangwall_<side>`) if either does; after the climbing holds beside, before the
  shimmy. (No scenario: the test level has no ledge running on past its wall without holds beside it.) (LEDGE-8) The
  wall hang at rest eases its root to 1.1 m under its hold (AC1's `ComputeRootFromHandTargets` 0xDD6730: the lower
  hand's target - 1.1 m, 0.5 m out; ours was wherever the move into it ended, 1.02 to 1.14 m under), still 0.50 m from
  the wall's face.
- **Jump targets** (JUMP-10): in a city the scorer's pick decides the jump: a top, a beam or post, a far edge, a hold to
  hang from (0x40 the wall hang, flown with the surface set into `xx_h_air_surface_tr_hangwall_reception_*` even with
  holds under it; 0x80 the free hang), a pass-over or a pole, aimed at the grab point on its chain. Ours' own hold search
  (`jump_hold`, 1.3-2.6 m up, the nearest) is left for the test level, which has no authored holds; the edge jump goes
  for any pick, a free hang below too.
- **Impulsion** (JUMP-3): a jump onto a top from a standstill, or within 0.45 s of the walk or run starting or a move
  handing back to the ground, starts with AC1's ground impulsion (`xx_h_impultionstraight_<foot>_to_ground_to_air_front_
  down_<other>`, actions 0x04A943DE / 0x040C793D, played over 0.1 s) and the free-step takeoff, not the running one.
  **[V]** 2026-10-24: event 45's guard (0xD83830) takes the impulsion standing, or with a start or transition playing
  under 0.75 of its normalised time (the jog's start, 0.67 s: until 0.50 s, which the routes bracket, 0.32 s did and
  0.59 s did not), never in the locomotion blend 0x05923BDB, and never with the item locked (flag 0x20). It enters
  0xD9BE90: the front `_down` and `_up` mixed by the target's height over the feet, (1 - h / 1.3, h / 1.3) (the events
  name only the first clip, `_down`), a 0.2 s blend. The free run's own (0xDA27C0, from its guard 0xD85690) also turns
  by the target's side, front to left / right by |a| / 90 degrees, beyond that to back-left / back-right. Ours: the
  start's phase and the front mix by height; after a reception (no item left in ours) still 0.45 s.
- **Air catches** (AIR-17, GUIDE-8): AC1's searches decide the catch: a hold round the feet + 1.4 m (0.3 m on along the
  reach, 0.4 m across, 0.3 m up or down, facing within 70 degrees) for the climbing or wall-hang catch, then one round the
  feet + 1.95 m for the free hang (0.25 / 0.5 m on after a fall of 3 m), with no wall ray first. The grab point is the
  probe's line along its direction meeting the hold (`Ledge::project_along`), not the nearest point across. Ours' wall
  ray is kept only for its directional grab (a wall up to 1.2 m the stick's way) and a swing bar's fling at a wall's
  holds (AC1's arrives by the jump target, which ours' ballistic fling lacks).
- **2026-10-11, climbing and ladders** (CLIMB-E6): the climb-out's room is AC1's three boxes over the edge (no headroom
  or wall rays; a top too thin to probe is climbed onto at the edge's height); gauntlet G13 (an awning over a wall's top).
  (CLIMB-E13, E14) The table moves get AC1's body boxes at the pose they end in (up and down too; the two side rays
  dropped for them) and its lean guard; gauntlet G14 (a block out of a wall between two rows).
  (CLIMB-E16, the release) The empty hand on the wall releases into the fall near the ground too (the step down onto it
  is the stick's).
  (CLIMB-E12, the overhang) A short table move up from a level pose whose holds stick out over the feet (the higher foot
  to the lower hand leaning out 30° past the hands' line) turns into the free hang,
  `xx_h_climb_1m_u_overhangfree_<1lu|1ru>` (TryTransitionToLedgeHang 0xDF4BA0). Not shown by a scenario: ours' feet rule
  (a wall at most `RECESS_MAX` 0.32 m behind the hold) refuses the stance on such holds before it, going into a hang.
  (LADDER-7) Ladders are climbed from either side where they stand free (the frame turned toward the climber; no exit
  off the top from the back), and in the wait the stick within 30° of a side goes round to the other side when AC1's
  box beside it is clear (the revolve, `xx_h_ladder_turn_<l|r>_a` / `_b_tr_h_wait_<l|r>`, guard 0xE262B0); against a
  wall the box is in the wall. Gauntlet G16 (a free-standing ladder). Not done: the back-approach turn (TURN
  0x0A64400C/D, when the attach box is blocked). The leaning ladder's forced revolve (leaning 10 degrees or more over the
  climber) is taken at load: ours climbs such a ladder from its other side (Masyaf's one, 11 degrees). (2026-10-24, LADDER-17)
  Ladders lean as AC1 climbs them, along the authored edge's axis (`Ladder::lean`: entries, catches, jump targets, each
  step's root and the exit off the top follow it; Damascus' lean up to 8 degrees, ladder 68 3.8, so its top is 0.5 m
  in from its foot). ladder68_walk against AC1: mean 0.27 -> 0.14 m, end 0.58 -> 0.06; no other route moved. The body
  lies along the ladder as AC1's frame does (the climb's tilt, eased at 1 rad/s, upright again off it): on ladder 68 the
  hands reach 0.07 m further in toward its rungs (the clip holds them 0.18 m in from the feet on an upright ladder; upright on
  68's lean they were 0.04, tilted 0.11). (Fixed after: onto a ladder
  from its top, the pull-down turns him round and the wall normal turned with it, ending into the wall: the box was
  tested behind him in the open and he went round into E's wall, then off its far side onto E's holds from inside it,
  and the exit off the top took him for being on its back. The normal now starts turned back by the chain's turn.)
- **Blends** (ANIM-4 to 6): every step takes AC1's whole `ActionBlend` from the move graph, looked up as the game does
  (`MoveGraph::blend_into`): item to item, the next item's blend; out of an item, its way's blend A into the connector
  (or the destination when it has none), B from the connector into the destination; an action the code starts, the
  playing item's way to it, the playing action's out-transition, the new one's in-transition, the default ASTOPBROLL
  0.2 s (a cut for an action flagged to have none). The item's own blend no longer overrides a transition's on the
  move's first frame. Kind NONE is a cut (86% of item blends, 221 of 243 ways in the movement blocks), AROLLB* keep A
  playing under the blend, *BSTOP* hold B on its first pose, the weight is linear (eased only where the flags say, two
  blends), a clamped blend lasts at most what is left of A. Ours: a cut is kept only where the two poses line up within
  5 cm (hands and feet); where our chain does not line up as AC1's (a move begun mid-clip, a step turned onto a hold),
  the cut would pop, and our crossfade by the gap is used. No movement block uses a B start position other than
  FROMSTART. Our own fades (steps the graph does not have) stay smoothstep.
- **Turns** (ANIM-8; ANIM-9 was GROUND-4): while the playing item has TURNS_BODY (the turn on the spot, the run
  turn-around) the stick no longer bends its facing and a high-profile turn is no longer broken off into the run near
  its end: the clip's own turn (090 and 180 mixed by the angle) ends it, and it leaves through its own ways.
- **Limb contacts** (ANIM-10): the climbing IK reads the clip's limb-contact track (hash 2: bits 2/3 the left/right
  toe, 4/5 the left/right hand; a mix takes its heaviest clip's, a mirrored clip swaps them): a hand or foot is pulled
  onto its hold or the wall only while its bit is set, its weight rising 4/s and falling 5/s; the hold search only picks
  where it lands. Clips without the track keep the pull by distance (6/s). Not yet: the travel from the old hold to the
  new along the clip's own delta, and the grab predicted 0.2 s before a contact.
- **Root path** (ANIM-16): the pull-ups from the waist (`xx_h_hangwaist_*`) spread their correction linearly over the
  clip, as every other move and AC1's root interpolator do; the cubic, late correction was ours. (No scenario reaches
  a waist pull-up: the free hang's top-out and the 2.5 m wall run onto a top want a look.)
- **Narrow objects, ground, hay, input** (NARROW-11, 14, 16; EXTRA-16; HAY-7, 9; INPUT-10):
  - NARROW-16: kiosks are AC1's market-stall counters, read from the cities' `Kiosk_*` entity groups (Damascus: 112
    stalls), not overhead bars: a jump target (type 0x400, its piece's middle, in the jump's own zone only), the entry
    side taken against the stall entity's +Y (0x4C2F90's second matrix row). Across the front: the entry, one run along
    the counter and `_tr_fall` off its end; along it, the side free step and, past 0.73 of it with something underfoot,
    the roll `xx_h_kiosk_side_freestep_to_roll` (0x2288984C) into the ground, or at its end with nothing the fall
    (0xE3E3F0, 0xE3D9A0, 0xE3DA00, 0xE3DE20). **[V]** 2026-10-25: there is no jump on out (0xE3BF90 is HumanRiding's,
    as Banned445's port has it); ours jumped on out from where the step left the counter, and now holds the step on what
    is under it (AC1's controller, the clip's root 2.9 m down) and rolls at the first frame past 0.73 on it. [ours] The stall's own collision is left out of the kiosk's room and clear-way tests and the
    way over: in Damascus it stands in the way (ours places it about 0.9 m out in front of the counter, a placement still
    to check) where AC1 comes onto the counter. The ground-entered overhead monkey bars are gone. **[V]** 2026-10-25:
    arrived on the counter (0xE07D00's type 0x400), AC1 plays the free step's reception (0x010DF150 with the left foot
    leading, else 0x010DE1FE), the root interpolated onto the target over 0.067 s with the clip's motion left out, and
    `HumanKiosk`'s wait (0xE3EAA0) starts the entry once the reception is 0.6 through. Ours now does the same (the
    reception held in place on the target, cut at 0.6); it went from the flight straight into the entry.
  - NARROW-11: on a beam, the empty hand with nothing to pull down onto climbs onto a wall ahead, else to its right or
    left (event 16 after the pull-down, holds probed 0.95 m ahead or 0.8 m to a side; ours: the ground's climb start
    turned that way). Gauntlet G17.
  - NARROW-14: the swing's stop (the legs let go) starts at the end of a down item (event 6's guard 0xDCCF40), the front
    stop's side from the move graph's transitions: the back stop after front_down, the front after back_down (as the reanalysis and Banned445's current port have it; ours had them swapped until 2026-10-16).
  - EXTRA-16: the ledge stop breaks into a standing turn toward the edge in high profile (as the running game's 0.084 s
    into `waitturn_left_090`).
  - HAY-7: out of a haystack over its rim, the side and the drop (1.5 m under the haystack's floor) come from its own box,
    the hang from a level hold along the rim if there is one, else from the rim itself.
  - HAY-9: the leap of faith's search is AC1's 0xE15AA0 with vt76's limits: the top (floor + 1.6 m) under 1.2 m over the
    feet; across 2 to 8 m, along 9 to 20 m and 40 to 70 degrees off the way from 1.5 m behind the feet, opened as the
    top lies lower (full at 30 m down); in front of the facing; the two-segment clear way (bent at 0.66). [ours] The 3 m
    split between the faith dive and the hay dive stays, and a viewpoint's hay is still taken whichever way he faces.
    Masyaf (2026-10-13): AC1 casts the clear way with the player and the haystack itself on an ignore list
    (`FUN_00b1aa90` and two pushes before the casts); ours stopped at the haystack's own collision, so Masyaf's
    `Hay_Bale_01` heaps (solid to 2.1 m, over the 1.85 m the way ends at) were never found. The takeoff and dive are AC1's
    blend of its four (1 m / 8 m long by 3 m / 30 m down: l = way / 7.5 m, d = drop to the search top / 27 m, 0xB1EC40's
    haystack case, Banned445's `jump_blend::faith`), not one pair by the drop (and the short one where a wall stood under
    the edge); the dive flies to the hay (no plain fall's drift decay, which left the dive at the tutorial tower's hay
    13 m out 9 m short) and enters at the heap's own top when it stands over the entry height (it landed on the heap,
    killed by a 35 m fall). Masyaf's two leaps: the siege's off `Scaffold_Leap1` (44 m down, `M05_S17_Trigger_LOFJump`)
    and the tutorial tower's (13 m out); scenarios tagged `masyaf`. [ours] The hay wait's turn to the heap's own facing
    shows a 0.13 m pop there (the dive's way and the heap's facing far apart).
  - INPUT-10: the arrow keys turn the camera (AC1's keyboard right stick); with `HighProfileToggle=1` under `[Input]` in
    the player's `Assassin.ini` (or `AC1_HIGH_TOGGLE=1`) a tap of under 0.5 s flips high profile (0x90B080), held as ever.
- **Towers** (user reports #3-#5, 2026-10-12; GUIDE-2):
  - GUIDE-2: a guidance piece too short to shorten (0.2 m or less, its ends free) holds at its middle, as AC1's probes
    take one (`sub_116D3C0`, read in Ghidra: shortened 0.1 m at both ends, the midpoint at 2·0.1 + 0.0005 or less);
    ours dropped them. Damascus: 197,063 holds from 166,951 (8,975 left out as buried). The `Minaret_M_02a` balconies'
    knobs (0.10-0.19 m pieces at the railing's middle) are among them.
  - The free hang at rest eases its root 2.4 m under its hold (AC1's `ComputeRootFromHandTargets` 0xDD6730, type 1: the
    higher hand's target − 2.4); ours ended the moves into it 2.1 m under, the wait clip's wrists 0.2-0.3 m over the
    hold, the hands in the air, and the hand steps up missed their holds by that much.
  - A move onto holds on a face 3-60 degrees round from the climber's (an octagonal tower's next face, a round tower's
    ring) turns the body about the hands to the holds' mean wall normal over the move, and the wall normal with it, as
    AC1 faces its climber along its holds' normals (`climb_pose`, Banned445's port); ours kept facing the old face.
  - The step off sideways onto the ground needs room to stand (the body pushed under 0.1 m out of walls): not a tower's
    0.25 m cornice band, fallen off at once. [ours]
  - Not changed: from #14's spot (the round tower's pilaster, hands at 26.88) the next hold up is 2.43 m over the hands
    (29.31) and the ring beside it at 28.71 lies 0.5-0.6 m to either side of the pilaster. AC1's reach up from the climb
    (0xDF0050: a foot hold 0.8-2.2 m up with a hand hold 1.2 m over it), its grid (rows to 3.3 m over the feet) and its
    jump up (TryBackEject: an edge 1.5-3.0 m over the feet, 0.3-1.3 m out) find none there either: a live check.
- **User reports (2026-10-12)**:
  - #1, hiding places: the hop out's room is AC1's `Guard_HopOut` box (half extents 0.35 across, 0.5 along, 0.75 up),
    0.5 m past where the way out leaves the footprint and 1.25 m over what he sits on, the haystack itself left out (its
    meshes' bounds with the collision shapes of hay and hiding-place entities touching them: a cart's bed, a hiding
    place's plank box, 3.1 m across in some, and the tarp's frame over it) (`Level::hay_hop_room`). Ours cast a ray from
    over the middle, 1.25 m over the 1.07 m rim, which in Damascus' rooftop hiding places met the tarp's frame (2.3 m up)
    every way, and the hop itself was stopped at the box's own wall: he stayed in. The older over-the-top ray stays as a
    second way to pass. The hop turns to the way out eased (turned in one frame from the wait's facing, the hands jumped
    0.6 m). The load logs the haystacks with no way out: in Damascus one pile in a walled yard (178.4, -221.9), none of
    the 123 hiding places.
  - #12, parapets: a Damascus roof's parapet (0.3 m wide, the roof 0.24 m under it, the street 8 m down) is a narrow top
    (`Level::add_narrow_tops`), and standing by it with the stick along it he got onto it and crouch-walked it; AC1 runs
    along it as ground. No beam with floor beside it (0.35 m to each side) is got onto from the ground any more, standing
    or walking, the stick along it or not. The top itself stays: free running off the bureau's roof the running game
    jumps onto one (audit JUMP-3's route; dropping parapets from the tops, first tried, lost that jump)
    (EXTRA-1's rule held only for standing on its line). A side entry onto a beam
    (`xx_h_freestep_entry_<foot>_tr_crouch<walk|jog>_*`) ends on the beam, its clip's own way corrected onto the line:
    played from where it met the parapet it walked 1.4 m off into the air, then the perch pulled him back.
  - #10, ejects: the eject with nothing to land on (`back_eject`) goes the stick's way by AC1's `jump_off_wall_dir`
    (straight away with no stick, at most 89 degrees off it, none past 135: into the wall) at the free-jump target, its
    takeoff the rebound groups' front 300 cm clips blended by the angle (`frontleft`/`frontright` straight away,
    `left`/`right` along the wall), after the rebound pose (`xx_h_hangwall_tr_rebound_<foot>`) as an eject at a target
    has; it was always 45 degrees off to the stick's side. Every eject, at a target too (`rebound_jump`, which searches
    along that way), reads the stick as the camera sees it (`move_dir`: AC1's jump off a wall reads the interpreter's
    camera-relative +0x1250, Banned445's port), not the climbing stick in the character's frame; scripted runs that hold
    only the climbing stick fall back on it.
- **2026-10-12, user reports #7-#9 (Damascus)**:
  - #9, a balcony's handrail 2.34 m over a beam's end: the wall run's tops came from the collision only (the floor behind
    the rail, under it, is not open), so it rebounded. Now a guidance edge in front stands in for the top when the
    collision gives none, as AC1's walling probes A and D take edges (0xE36BD0): one 1.25-2 m (A) or 2.1-2.6 m (D) over
    the feet whose `ClassifyLedgeCandidate` (0xE341D0) cast, 45 degrees up over its edge and in, runs 0.4 m past it
    (class 1, a pull-up), with a beam or a floor there to kneel on [ours]. On the rail (a beam) it is the pull-up onto a
    post, then on over it. (`xx_h_hangwall_tr_passover_handr`, 0x0109BBBD, is only compared against in the passover's
    update 0xDE2EE0, never started by the code found: not used.)
  - #7, #8, a balcony held out on stepped corbels (edges 0.45 m further out and 0.12-0.3 m up each, out to its front):
    the climb's jump up to an overhang (TryBackEject) is tried from any climbing pose, as AC1's ChooseMove does, not only
    the level ones; ours' fallback "bring the hands level first" no longer steps down (from the raised hand it went
    1ru -> 1m -> 1ru for ever). In the free hang, up onto an edge the clips miss: AC1's vertical rows (`FindVerticalRows`
    0xDD8F70, Banned445's `vertical_rows`), 0.1-0.8 m up, up to 0.5 m in or out and 0.2 m along, the highest; both hands
    step onto it in turn (the second following on, `ContinuePendingVerticalStep`), the body carried under it. Then the
    free hang's top-out onto the balcony.
  - #11, #6, #13, "thin air": [ours] the running jump into the wall hang
    (`jump_hangwall`) put each hand on whichever hold was nearest where the clips ended, within 3 m: by a market stall one
    took a house's roof edge 8 m up and the other the stall's awning 5 m up, and he hung from the air between them. The
    hands now land on the hold aimed at (their middle on its grab point, each then on a hold within 0.35 m), as AC1 flies
    the jump at its target's grab point. A climbing leap's way, chest to chest up or down with it, must be clear (a leap
    to the side and 2.4 m down from a wall hang stood on a porch roof went through the roof to a hold under it). The
    wall run on nothing was a scaffold's collision: skinned props with more than 24 bones were left undrawn as
    characters, and the scaffolds (69 bones) and storage racks (61) with them; an entity with collision shapes is now
    drawn whatever its skeleton, as the running game draws that scaffold (checked live) (the city's load logs how many collided entities draw nothing: 0 in Damascus).
- **2026-10-12, the last findings**:
  - EXTRA-17: the pull-down's hold is AC1's edge report (the interpreter's vt136 pick, 0xB17EF0, with 0.75 m and any
    angle; the guard 0xD9D6C0): a hold at the feet within 0.75 m, facing out within 90 degrees of his facing, the one
    facing most his way, more than 2 m over what lies 0.3 m past it. Ours walked 0.9 m along the facing to where the
    floor ended and took a hold within 0.45 m of there facing within 45 degrees.
  - JUMP-6: the running jump (AC1's kind 0) plays its takeoff and flight in the takeoff facing, the correction carrying
    it onto the target (across the ground spread over both; the height's on the flight, [ours]: the takeoff's feet are
    on the roof), and turns the facing toward the target linearly over the takeoff, at most 0.4 s (0xB20200 keeps the
    body's matrix as the jump frame, 0xE0EF30 slerps the facing, Banned445's port). Ours ran the clips along the aim from
    the first frame, the facing eased round over 0.25 s. Measured on B1 to B3 run 12 degrees off the roofs' line: the 11
    degree turn now done by the flight's start (the running game: 14 degrees at 1.5 per frame over a 0.15 s takeoff),
    the path within 0.04 m of before, the landing the same; every other jump of the jump / combo scenarios starts
    facing its target (no turn) and is unchanged.
  - ANIM-13: a mix of clips (`forge::anim::mix`) folds them in one after another, as AC1's item evaluator does
    (`sub_777AE0`: clip i blended into the ones before it by w_i over the running sum of the weights, rotations by
    AC1's interpolation 0x4916F0, a normalised lerp within about 8 degrees, else a slerp); ours summed the weighted
    quaternions and normalised. The same for two clips; with three or more far-apart ones (the 090 / 180 turns mixed with
    a straight one) the fold follows the arc. (0x507650, which the audit pointed at, is the action's length: the items'
    durations less their blends.)
  - ANIM-11: a climbing hand's IK puts the middle finger's knuckle (`<side>HandMiddle1`) on the hold point, as AC1's
    HumanIK hand effector does (0xE57450 via 0x4FA910, Banned445's port): the wrist goes where the knuckle, as the clip
    holds it that frame, lands on the hold. Ours aimed the wrist 9 cm out and 8 cm under the hold whatever the hand's
    pose. Not done: the toe as
    the foot's effector: AC1 puts it on a foot hold's point, ours has no foot holds (the feet go onto the wall's face,
    the ankle 0.11 m off it); it waits for foot holds. (In the climbing stance at rest, done 2026-10-13: the toe on the
    stance's foot goal, "The climbing stance's pose".)
  - WALL-21: the wall run takes walls leaning up to 45 degrees either way, as AC1's (`Human__WallingSlopeDir` 0xE14590,
    `Human__WallingSlopeLimit` 0xE14FB0, read in the exe: 45 degrees refuses, the lean held to -10..+20 degrees): a ray
    from 0.75 m out and 1 m up back toward the wall gives its lean, and the entry's contact is 0.5 m out along the
    leaning normal, 0.3 m under the probe's hit (on a wall leaning back 20 degrees, 0.17 m higher and 0.03 m nearer than
    on an upright one). Ours refused any face more than 17 degrees off upright. [ours] Faces over 44 degrees off upright
    are floors to our collision; the run's body stays upright (AC1 pitches it with the slope, Banned445's port does not
    either). The test level's leaning wall is south of the passover walls. (`wall0`'s entry, 0.10 m higher in the running
    game than in ours, is still to explain: a lean of 11 degrees would give that height but not its 0.57 m out.)
  - Checked, no change:
    - CLIMB-E16 (timing): at the fixed step the reach items take their clips' time, `_a` 0.200 s, `_b` 0.400, `_c`
      0.666 (block A, up), against the running game's 0.200 / 0.385 / 0.682 (`wall1_hang_up`): `_b` and `_c` together
      1.066 s against 1.067. The audit's 0.43 / 0.75 s came from the windowed runs' uneven frames.
    - INPUT-15 (AC1's ability stack): not needed yet. Nothing in ours gives or takes an ability (no missions, memory
      blocks or tutorial); in free roam AC1 allows every one at MaxSpeed 4 (Banned445's default set), so the stack would
      only ever say yes. It comes with the first mission or Masyaf's opening, which lock moves.
    - LADDER-7 (the rest; 2026-10-24: Masyaf's 42 have one at 11 degrees, now climbed from its other side at load, and
      ladders lean, see the ladder section): Damascus' 333 authored ladder edges lean less than 10 degrees (278 under 5), so the forced
      revolve off a ladder leaning 10 degrees or more over the climber never fires there; ours keeps ladders upright
      (the authored edge's top is kept as a height). The back-approach turn needs a ladder whose near side is blocked at
      chest height (AC1's attach box, 0.95 m up); none is known. To settle: the other cities' ladders' leans, and a
      free-standing ladder with something in front of it in a city.
    - LEDGE-7 (the wall-free hang): no free hang in the hang scenarios (20 rests, Damascus' included) has a wall under its
      hands (AC1's test: 0.15 m in, 0.6 / 1.2 / 1.8 m down): ours takes the free hang only where the feet find no wall.
      AC1 reaches `hangwallfree_wait` through entries that force a free hang whatever is under it (walling's free-hang
      catches B and C with their swing strength, the jump's `hangwallfree` 250 / 300 cm action). Live logs (2026-10-12,
      24 routes round the Damascus bureau and the user's own run): no action ended in `xx_h_hangwallfree_wait`. The wall
      hangs came from the wall run's catch (0x0106CFC5), the pull-down off a roof's edge and the standing jump at a hold
      2.4 m up (0x01271631), all into `hangwall_wait`; the only free hangs were swing-bar catches after a back eject
      (`air_front_050cm_to_swing`, `swing_cycle_front_up`, `hangfree_wait`). The standing jump matched Banned445's bands of
      0xB21DA0 to the centimetre (2.0-2.5 m with a wall: 0x01271631, the root 1.1 m under the hands and 0.5 m out), whose
      next band, 2.5-3.0 m with a wall under the hold, is 0x0121A8B1 (`jumpstraight_footl_to_hangwallfree_250/300cm`, the
      reception into the wall-free hang): that, and the wall run's `step1_footr_tr_hangwallfree_350/400cm`, are the
      entries; ours hangs free there. To see one: a standing jump at a hold 2.5-3.0 m over the feet with wall under it.
- **2026-10-12, the hangs AC1 has that ours did not play** (LEDGE-15, LEDGE-7; read in the exe: every action id below
  searched for in its code, and the move graph's ways):
  - The wall-free hang (LEDGE-7): the standing jump keeps AC1's bands (0xB21DA0) by the hold's height over the feet: the
    wall hang under 2.5 m; from 2.5 m, with a wall under the hold (the jump query's class 0x08, `wall_under_hold`), the
    wall-free hang, `jumpstraight_footl_to_hangwallfree_<250|300>cm` + its reception `_tr_hangwallfree_a/b` (0x0121A8B1,
    0x0121B072), into `xx_h_hangwallfree_wait` (0xE07D00 plays it, the hang's type set to free), the root 2.4 m under the
    hold and 0.5 m out; else the free hang. Ours had the wall hang there. Gauntlet G24.
  - The free hang onto the wall under it, in place (CanPullup 0xDE2270: up in a free hang where both foot rays find the wall
    at the hands, `sub_DE0E30`, and the body has room, ahead of the pull-up; Pullup_Start 0xDDBE80 -> 0xDD4160 plays
    `xx_h_hangfree_tr_hangwall`, 0x01B702FD, into the wall hang's wait under the same hands). The way out of the wall-free
    hang and of a swing back onto a ledge near the wall. Gauntlet G24.
  - Up or down from the wall hang onto a ledge where not both feet find the wall: AC1's TrySwitchHangType (0xDE1060) at the
    hand step's target plays `xx_h_hangwall_tr_hangfree_up_left` / `_down_left` (0x01C3217A / 0x01C3217C) in place of the
    step, into the free hang. Gauntlet G24 (down, off a slab over open air); no scenario up: its foot rays, 1 m under the
    new hold, meet the wall the hang is on, unless the new ledge stands out from it.
  - (2026-10-13) Up or down from the free hang onto a ledge a row on where a foot finds the wall: the same TrySwitchHangType
    at the hand step's target (both hands' targets the vertical probe's hold), after the moves onto the climbing holds and
    before the step; free to wall it wants either foot ray only. It plays `xx_h_hangfree_tr_hangwall_right` (0x01C32563,
    up and down; `_left`, 0x01C32562, only going left) while its root interpolator (0x711130, over the action's length)
    carries the root from the free hang to the wall hang under the new holds: the clip's own 0.75 m to the right and 1.3 m
    up are left out (`Move::in_place`, `free_to_wall_row`; the row: a hold for each hand about the free step's 0.7 m on).
    Ours stepped up or down in the free hang. Not onto a post's cap (ours' one-hand holds: the Damascus minaret's railing
    knob, user report #3, keeps its hand step and one-hand pull-up); AC1's own room test there (0xB2A410, a 0.5 by 1.1 m
    body) is not read. Gauntlet G24's stepped block (z 284): up from the free hang at 2.7 m onto the wall hang at 3.4 m,
    and back down through `hangwall_tr_hangfree_down_left`.
  - The swing back (LEDGE-6, `swing_back`) and the jump up from the climb (`climb_jump_up`) go on as the move graph has
    them: out of their last item (0x4CD68F50, 0x1EB21811) the connector 0x0564BB0E into the free wait,
    `xx_h_hangfree_swingback_min_a/b` (weighed [1, 0] by 0xDE1FE0 under the wait kind 3 the jumps set, 0xDDAB00,
    0xDF9650), and the free wait after it mixes `xx_h_hangfree_wait` and `xx_h_hangwallfree_wait` by the jump's `max`
    share (how far out the ledge is), while he hangs from ledges (the kind is reset leaving them, 0xDCD780). Gauntlet G24.
    The wall run's free catches (2026-10-13; 0xE37590: the entry's 0x0564BB1D, probe B; the step's 0x0564BB1E, probe C)
    mix four clips, [min low, min high, max low, max high] by [(1-w)(1-b), w(1-b), (1-w)b, w·b]: w by height (ours by the
    clips' own hand heights, as before), b the `max` share by how far out the edge is, its distance d across from the point
    0.6 m out from the walling root (ours: the face run up + `WALL_RUN_OUT` + 0.6), b = 1 - (d - 0.1) / 0.7 for the
    entry's, 1 - d / 0.7 for the step's, clamped; they set the wait kind 3 and its weight b (+0x6C / +0x70), so they go on
    through the same connector (`swing_settle`) into the free wait mixed by b (`swing_max_share`). Ours played the `min`
    clips only, into the plain free wait. Gauntlet G24: a wall run under the slab 0.8 m out, b 0.65.
  - `hangfree_strafe_<side>_000cm_<open|close>`: the free shimmy's actions (0x01A2490A-D) mix them with the `_050cm` ones,
    and StartHandStep (0xDDE0C0) sets the weights [0, 1] on every start: the `_000cm` clips are never shown (they have no
    root motion; the root goes by the interpolator). Ours plays the `_050cm` ones whole: as AC1.
  - Never asked for: `xx_h_hangwall_tr_hangfree` in place (0x01B70FA9), `_up_right` / `_down_right` (0x01C3217B / D),
    `xx_h_hangwallfree_tr_fall` (0x98F78DA5): no reference in the exe's code, no way into them in the graph. The wall
    run's `step1_footr_tr_hangwallfree_<350|400>cm_swingstraight` (0x0106F36F) is only in a list of actions 0xDCD910
    leaves alone, never started (the walling's free catches are 0x0564BB1D / E). `xx_h_hangwallfree_tr_hangwall`
    (0x0106F2EA) plays for the ledge's hang type 2, which only 0xE39550 sets: the walling's fourth sub-state (+0x91, run
    from its update 0xE39C20), which reads the action playing (0x035F27A0 / 0x02926C67 the wall hang, else the wall-free
    hang) and starts 0x035F27A1 / 0x02926C68 or 0x035F27AD / 0x029240CA. None of those ids is an action (or key, or a
    transition's target) in any action block of any data file, nor an object in DataPC's index, nor named anywhere else in
    the exe (searched 2026-10-13): a walling path the shipped game cannot play. Left out; to settle, whether the running
    game ever enters that sub-state (the walling's +0x3E logged on a wall run).
- **2026-10-13, the pops in the hang jumps' flights** (the scenarios' live pop check, `AC1_POP_CHECK`): the standing jump's
  flights into the hangs (`jumpstraight_footl_to_hangwall_250cm`, `_hangwallfree_250/300cm`, `_hangfree_250cm`) showed
  0.17-0.18 m pops at their start. Two causes. (1) Ours went from the crouch's still end straight into the flight's arm
  swing; AC1's ground plays its static jump's push off first, `xx_h_impultionstraight_<foot>_to_jumpstraight_clear_<foot>`
  (0x00F0A538, request 89, 0.13 s in place), for these jumps too: the running game logged it (0x59) for 0.135 s before
  `jumpstraight_footl_to_hangwall_200cm`, the body rising into the flight's pose (live log 2026-10-12, 3114.245), as
  Banned445's port has it (StaticPrepare before the jump). Now in every standing jump to a ledge (`jump_options`).
  (2) Every seam of a chain into a blend showed the frame before again: the fade started from the pose last shown at
  weight 0, A's own last frame never shown, then B's motion all at once. The seam's fade now starts from A's last pose
  (under a fade still running into A, as that fade shows it), and B and the fade go on by the time left over in the frame
  (AC1's slot ends with time to spare and the next item takes it, 0x730E50 / 0x72FC30 as the port reads them). The
  flights' pops are gone (0.18 -> under 0.1 m); over the climb, hang, jump, wallrun and bars tags the pops logged went
  911 -> 715, those over 0.12 m 93 -> 77. Left (looked at, not ours to change or no clear cause): the swing back's
  `hangwall_to_swingback_up_max_*_a` (0.15 m, the clip's own left hand leaving the hold at 9 m/s in its last frames, with
  or without IK); the eject off a wall run's hang (`rebound_*_to_air`, 0.2-0.3 m): the new chain's first frame shows the
  frozen pose under the root already turned 170 degrees, `snap_takeoff` turning that pose only on the next update (one
  frame of the body turned; to settle by snapping in the frame the eject starts).
Open (still ours' own): the hang root against the running game's: 1.13 m under our hold line on `wall1`, 1.01 m under
the top on `hay70` (ours now 1.1 under both): AC1's hand targets are not our hold lines (they sit 0.03 m above it on
`wall1`, 0.09 m over the haystack's top on `hay70`); to settle with the hands' targets read live on those two holds.
(Read in the live log, 2026-10-13: on `wall1` AC1's two hand goals in `hangwall_wait` are on the hold line, 8.025 m on
the line at 8.02, and its root 1.131 m under them: the gap is AC1's 1.13 against ours' 1.1, not the hold. `hay70` not yet.)

## The climbing sweep (2026-10-12)
`AC1_CLIMB_SWEEP=n` (`climb_sweep.rs`, any level, headless:
`AC1_HEADLESS=1 AC1_LEVEL=damascus AC1_CLIMB_SWEEP=30 cargo run --release -p ac1`) climbs a level the way a player does
and scores it. It picks n walls (a first hold 1.5-2.05 m over open ground, the climb start's reach from standing) and n/2
roof edges from the level's holds, 8 m apart, and runs scripts at each through the control runs (the level loads once):
grab and up 16 s, up then left and right, down to the ground, let go, a free run into the wall then up, and at a roof
edge the pull-down then left, right and up. Every frame is checked; each run reports these classes once: `stuck` (stick
held 2.5 s, no progress, while one of AC1's grid cells on that way holds), `topout` (up held at a hold with a walkable
top behind it), `letgo` / `letgo-down`, `hands-off` / `no-hold` (resting, a knuckle over 0.15 / 0.4 m from every hold),
`embed` (hips or chest over 0.08 m into a solid), `pop` (the pose check's, over 0.15 m), `flipflop` (four state changes
between two states inside 3 s, the root within 0.25 m), `feet-air` / `free-on-wall` (AC1's hang foot rays, `sub_DE0A60`,
finding no wall in a wall stance / wall for both feet in a free hang), `no-grab`, `death`. One TSV line a run
(`AC1_SWEEP_OUT`; its start replays it as `AC1_START` in a control run), the counts by class in the log.
`AC1_SWEEP_NEAR="x,z,r"`, `AC1_SWEEP_PROGS=up,side,...`, `AC1_SWEEP_SEED`, `AC1_SWEEP_RECORD=dir` (each run's frames).
About 5 minutes for the test level at 20, 30 for Damascus at 30, two at once. Runs with each class, dev (0dbe38a)
against these fixes: Damascus (147 runs) embed 7 / 5, topout 6 / 6, pop 5 / 5, stuck 2 / 2, hands-off 2 / 2, flipflop
1 / 1, free-on-wall 1 / 1; test level (101 runs) topout 4 / 3, free-on-wall 4 / 4, pop 4 / 3, embed 1 / 1, flipflop 1 / 0.
Left: top-outs refused under four Damascus walls (not traced), the climb start at one Damascus wall easing the body into
it, two stuck sideways moves, pops of 0.15-0.2 m in the post pull-down and landings.

Fixed from it and from the user's recordings of 2026-10-13:
- The switch from the wall hang into the free hang (`TrySwitchHangType` 0xDE1060) and the climbing stance's ledge grab
  into a free hang (`TryLedgeGrab` 0xDF0980) test AC1's free-hang body boxes (0xB2DD50: 0.75 x 0.5 x 2.4 m 0.5 m out and
  1.2 m up, 0.75 x 0.44 x 1.2 m 0.25 m out and 0.8 m up) at its pose (0xB15AD0: the root 2.4 m under the hands' middle),
  and put the root there, under the holds: at a wall's end the clip had carried the legs into its corner. On the
  octagonal tower the swing off the wall at the next angled corner is now turned down, the climb going on round it.
- A free hang under a cornice 0.8 m deep (test level, the slab at (-92.9, 164)) pulls up onto it: ours' inside-a-solid
  guard on the top-out probed 0.95 m in from the wrists, inside the wall behind; it now probes where he stands up. Its
  hand step took the edge held for a row 0.3 m up (it measures from 2.1 m over the root, the hang rests 2.4 m under its
  hold) and stepped onto it over and over; the edge held is left out.
- The free hang's hand steps go by AC1's vertical rows (`FindVerticalRows` 0xDD8F70: up 0.1-0.8 m, down within 0.8 m,
  0.5 m in or out, 0.2 m along) before ours' grid hand steps, and down too: under a Damascus awning's frame
  (`Awning_6x2_Wood_01A`, beams each 0.13-0.31 m lower and 0.38-0.49 m further in, the user's "inverted triangles") he
  climbs down the beams and back up over the front; the grid step had put the hands 0.46 m over a beam (in the air).
- A running jump at a knob (an authored piece shorter than the hands are apart, 0.17 m on a Damascus house) ends in
  the hands-together hang, both hands on it; they hung 0.3 m either side of it. The reception before it too (2026-10-13,
  see below).
- The window cap in recording 194384 (a pointed top sloping 30 degrees): the user's build pulled up one-handed onto its
  point and fell 7.4 m; with dev's free-to-wall switch he hung on the wall under it; now he pulls up onto the floor behind
  the cap (below).

Damascus sweep again (2026-10-14, fix/parkour3, 147 runs): pop 5, hands-off 3, stuck 3, free-on-wall 1, no-grab 1,
topout 1; flipflop 0. Fixed from it: the post pull-down (`w11-drop`, `w12-drop`): its half turn onto the wall eased the
root the same way as the clip's own turn (180 degrees: the shortest way is either), the body spinning a whole extra turn
in 0.2 s; it now eases the way the clip's turn cancels, as off a ledge (pop 0.20 -> 0.15 m, the rest the turn's speed).
Traced, left: `w14-up` hands-off (hung from a cornice 0.3 m out from the wall's holds after the free-to-wall switch,
the closed wait's hands 0.3 m inside the hold, no hand IK there), `w2-run` no-grab (the same with ours' collision edge
tests, `AC1_NO_EDGE_REPORTS`: not from the edge reports). Pops over 0.15 m in the scenarios, by cause: baked into the
clip (`xx_h_climbing_hangwall_to_swingback_up_max_*_a` frame 13, bones 8-11.5 degrees in a frame), AC1's own blends from a
frozen pose into a clip that starts fast (`xx_h_collide_full_hand_150cm`, the impulsions' `_to_ground_to_air_*`, the
rebounds' 550 cm takeoffs), a script driving a low-profile walk at 3 m/s (the horse barrier); fixed: the jump on out of a
kiosk's side (its clip's root runs 1.1-1.9 m ahead of the body: started under the hips now), and the pop check counted a
root moved under a pose kept in place (it wants the world's jerk at least half the jump now).

## The live log comparisons (2026-10-12)
The route library played again in the running game with ac1-hook's live log on (every frame's root, context, action,
bones, limb IK goals, input and camera), and in ours by control runs with `AC1_RECORD` (`livelog compare --game`: the
paths, the mode switches, and the poses by clip, ours' clips named by their action as the live log names a blend by its
first clip). Found and fixed:
- The standing jump's target: AC1 goes for the highest hold in reach. On the bureau's wall (`wall1`, holds 1.7 and 2.4 m
  over the feet) the running game jumped to the 2.4 m one (action 0x01271631, `jumpstraight_footl_to_hangwall_200/250cm`
  blended toward 250, the root 1.1 m under the hand goals and 0.5 m out, as Banned445's bands of 0xB21DA0 give), ours
  to the 1.7 m one, the root 0.6 m lower. AC1's chooser (0xE97720) takes the higher of two reports facing within 30
  degrees of each other; ours now takes the highest of the jump's options that fit. Ours hangs 0.005 m from the running
  game's height (0.1 m nearer the wall).
- Up from a ledge hang pulls up first: on the haystack route (`hay70_free`) the running game, hung from the roof's edge
  after the pull-down with W and the legs held, pulled up onto it (`hangwall_tr_hangknee`, then the free-step entry);
  ours leapt to a hold 2.4 m higher, as the leaps came before the top-out. AC1's ledge tries the pull-up first
  (CanPullup 0xDE2270, event 0, ahead of the hand steps and jumps up; Banned445's port); ours now does from the wall and
  free hangs (the climbing stances keep AC1's ChooseMove order, the top-out last). The route's mean gap 0.71 to 0.54 m,
  the path within 0.3 m to 4 s where it was 1.6 m off.

The live log names a blend by its action's first clip, so its names say nothing about a blend's weights: the wall run's
`..._hangwall_251cm_000cm_a` is action 0x0106CFC5 (251 and 430 cm, `_000cm` and `_050cm`), the ground impulsion's
`..._front_down_...` action 0x04A943DE (all ten directions, up and down), `air_front_050cm_..._to_freestep` the whole
flight blend. (The comment in `jump.rs` that every impulsion seen was `_down` read that name.) `livelog compare --game`
names ours' clips the same way.

Measured (mean root gap over the route, from when each starts moving; 21 routes): `wall1_*` hangs, climbs and ejects
0.08-0.29 m, `wall1_standjump` 1.32 to 0.09 m, `crate_climb` 0.21, `ladder68_walk` 0.52 (0.02 at the end), roof runs
0.28-0.61, `hay70_free` 0.71 to 0.54; `roof1_free_south` 1.21 (ours takes a parapet 0.3 m past the first edge as a jump
target, a perch from ours' own parapet tops, where the running game jumps 2.3 m to the next roof's rim), `seams_free_west`
0.56. Poses (bones from the root in the body's frame, same action): the wall hang's (`hangwall_wait`) hips and head
within 0.01 m, the hands 0.02-0.03 m (their spread is the hold's); the running game's hands stop 0.11 m short of their
IK goals and the feet 0.17 m (ours reaches them), and its hand goals sit 0.1 m over ours' hold lines on the bureau's
walls (the root as much higher). The climbing stance (`climb_wait_1m`, 469 frames) is the largest steady one: ours'
hips 0.08 m higher and 0.09 m nearer the wall, the head 0.09 m higher and 0.19 m further back, with the hands and feet
within 0.1 m: AC1 leans the chest into the wall more. Traced and fixed below.

### The climbing stance's pose (2026-10-13)
Two of the live log's climbs, set side by side frame by frame (the bureau's wall `wall1`, the wall run's hang and two
reaches up, then 4.9 s in `climb_wait_1m`; and the wall 15 m north, `wall1_sill`, one reach up onto a sill, 7.4 s), and
ours on the same routes (control runs with `AC1_RECORD`; root, bones and goals by clip):
- **AC1's limb goals are its grid's hold points.** Both hands' goals are one point, the hold straight over the root, and
  both feet's one point on the hold two rows under it, in the 1m poses (the 1ru pose: each side's own row, the same
  column): AC1's hold grid gives one hold a cell, the cell's column centred on the root (`BuildHoldGrid` 0xDF6A40).
  The goals lie on the authored hold lines (8.025 m on the line at 8.02, 10.642 on 10.64): the "0.1 m over ours' hold
  lines" was ours' log giving the wrist's target, 0.08 m under the line, not the hold point (now it logs the point the
  knuckle goes to, as ac1-hook logs AC1's). The "hands 0.11 m short, feet 0.17 m" are AC1's effectors: the middle
  knuckle on the hand's goal (ANIM-11) and the toe on the foot's (the wrist and ankle, which the logs give, sit that far
  off): AC1 reaches its goals. Ours now puts both hands' knuckles and both toes on the stance's goals at rest (eased onto
  them at 0.6 m/s, each limb kept on its goal through the next move until its clip lets it go): the hands 0.13 m apart
  (AC1 0.13), the ankles 0.17 m off their goal (AC1 0.165), where ours' hands were 0.28 m apart and its ankles 0.11 m
  off the wall's face.
- **AC1's stance root comes from the four holds** (`sub_B1CC20` via `ComputeRootFromMove` 0xDEC6E0, Banned445's port
  `climb_pose`): 0.5 m back from the feet's hold along a facing pitched as the hold rows lean (the hand → foot line), on
  the lower foot's plane. Checked against both live climbs to the millimetre: on `wall1`'s second row (holds in one
  plane) the root 0.500 m out from the feet's hold at its height; on its first row (the hands' row 0.2 m deeper than a
  ledge under the feet) and on `wall1_sill` (a sill 0.16 m out under the hands' row) 0.5 m out from the ledge or sill,
  0.07 m over it, the body pitched 8.5 and 7.5 degrees into the wall. That pitch was the "chest leant in": the clip is
  the same, the whole body turns about the root. Ours stood upright, and slid at rest to 0.5 m from the nearest hold
  round the body: on `wall1` the ledge 1.4 m under the feet, 0.2 m out, so the body hung 0.2 m off the wall with the
  hands 0.33 m off their holds and no grip. Ours now: the stance settles onto AC1's root from its holds and the body
  pitches with them over the move (`climb_frame`, `StanceFrame`, `WallClimb::tilt`); a move into the stance ends no
  nearer the wall than that root (on `wall1`'s first row ours' hands went 0.15 m into the wall and its feet into the
  ledge through the reach). [ours: the move otherwise still ends where its clip puts the hands on the holds, 0.59 m out
  from them, as the moves on from there are tuned: taking AC1's root outright sent the routes round the octagonal tower's
  cornice, the balcony's corbels and the gauntlet's block (G14) other ways, some through the geometry; AC1's own checks on
  those ways (its reach's capsule sweep, the hang boxes `sub_B2DC80` / `sub_B2DD50`) are not in ours yet.] The jump up
  from the climb (`TryBackEject`) searches round the hands' goals, as AC1 does, not round where the root puts them.
- At rest in `climb_wait_1m`, bones from the root (forward, up, right), before / after against the running game:
  `wall1` hips 0.03 / 0.035 m (the root 0.20 / 0.00 m off AC1's), head 0.05 / 0.03, hands 0.33 off the wall / 0.015-0.025,
  feet 0.08 / 0.01-0.015; `wall1_sill` hips 0.12 / 0.03, head 0.21 / 0.03, hands 0.07 / 0.05, feet 0.05 / 0.03. The goals
  within 0.01 m of AC1's in both.
- Still ours' own: the move itself between the stances (AC1 holds the root still through the reach's first item, 0.27 s,
  then carries it up linearly; ours follows the clip's root motion with the correction spread over it); the post-adjust
  hips drop and head clearance (AC1's `ADJUST_SLOPE` 0xE58333 / `ADJUST_HEAD` 0xE58799) are not in ours.

### The routes re-cut, climbing along them (2026-10-13)
The route files cut from the live logs were lost; the 21 routes are cut again from the raw logs
(`reports/live-logs-20261012/`, 2476-3228 s) by `livelog runs` (the logs split at teleports, each part's start, facing,
input span, modes) and `livelog cut` (one route's frames and events out of the rotated files). The driver,
`F:/Claude/ac1-wt/reports/climb-routes/routes.py` (data, not in the repo), holds each route's live-log window and redoes
it all:

    cargo build --release -p ac1 --bin ac1 --example livelog            (CARGO_TARGET_DIR as the worktree's)
    python -I routes.py cut [route...]       livelog cut <route>.real.txt --from s --to s --dir <live logs>
    python -I routes.py ours [route...]      one headless control-mode Damascus game, a `run` line per route, AC1_RECORD
    python -I routes.py compare [route...]   livelog compare <route>.real.txt <route>.ours.txt --game <install>
    python -I routes.py show [route...]      the run settings ours gets

Ours' inputs are the running game's pad as logged (the stick, high profile bit 0x1000, legs bit 0), timed from its first
press held 0.3 s (a camera-alignment blip before it is left out); on the ground ours is steered the way the real one ran
(`AC1_HEADINGS`), elsewhere the stick's way from the logged camera (off the ground the run's way shows nothing). Mean root
gap (m) from when each starts moving; before = this code at dev 978b735 with the same cuts, the 2026-10-12 figure in
brackets:

| route | before | after | what it was |
|---|---|---|---|
| `seams_free_west` | 9.01 (0.56) | 0.42 | a 2 cm crack between two roofs' collision at x 37.99: the ray down fell through, ours stepped off and fell 8 m (`ground_over_crack`: AC1 walks off at a guidance edge's line, 0xDA3F40, never at a crack) |
| `roof2_free_south` | 1.32 | 0.22 | the same: a crack on the way |
| `wall1_standjump` | 1.36 (0.09) | 0.09 | the script did not pass the legs on while the high profile's stance change played (the player's input does): no jump |
| `roof1_free_south` | 1.05 (1.21) | 0.90 | the rim beam's edge jump fired 0.28 m short of a parapet, onto it as a perch and on; now only once up on the rim: one 2.3 m jump onto the next roof's rim (0.05 m at the landing). Left: AC1 comes down on the rim into NarrowObject and stands 0.1 s before running on; ours runs on at once and leads by about 1 m from there |
| `wall1_hang_right` | 0.32 | 0.03 | the hang's lead-hand reach went on where AC1's sweep is blocked (a beam 0.6 m under the hold, 0.9 m on): AC1 sweeps a capsule (r 0.15) from 0.2 m over the lead hand, 0.75 m out, down to the hang's root (1.1 m under the hands, free 2.4), 1.55 m along, and refuses under 0.7 m past its radius (0xDD9640, read in Ghidra; Banned445's port has it as a sphere) |
| `wall1_hang_down` | 0.28 | 0.19 | the back eject onto the beam behind came down as on the ground and ran 0.76 m on before balancing; now onto the beam as NarrowObject (as `jump_freestep`). Left: AC1's reception steps 0.3 m on into the crouched wait |
| `ladder68_walk` | 0.47 (0.52) | 0.28 | onto the ladder from its foot always with the high profile's entry; then by profile and foot as Banned445's port picks; the trace has the high profile's entry walking too (`xx_h_wait_hipm_footl_tr_h_ladder_climb_up_r`, rising 0.5 m at once), so now that one by foot in either profile (0.53 to 0.23 m, mean 0.25 to 0.16). Left: the running game's root goes 0.5 m in toward the wall over the 8 m climb (a leaning ladder?), ours' stays out |
| `hay70_free` | 1.08 (0.54) | 1.08 | pulled up onto the haystack's rim with the stick held away from the hay, the running game stood on it (NarrowObject) and free ran on at a target 4.4 m off; ours dove back in (the dive is right with the stick let go, the 2026-10-10 recording). Now on the rim as a perch; left: from it ours' query picks the box's own far rim 0.9 m off where AC1 jumps 4.4 m |
| `wall1_hang_up2` | (mis-cut) | 0.17 | the run's start had a 0.2 s stick blip; cut after it |
| `roof2_run_east` | 0.73 -> 0.25 | 0.73 -> 0.09 | (fixed 2026-10-27: the narrow rim's stand, the edge lock let go on hanging) the second ledge stop: after pulling up, the running game's turn broke into the ledge stop 0.09 s in and pulled down again before W was let go; ours turned round into the run first and stopped 0.35 s later, too late for the pull-down. Not changed (ground turns) |
| `ladder68_free` | 0.98 | 0.98 | the running game stumbled over something at 35.5 m (`xx_stumble_sprint_right`, 1.7 s lost; not in ours' city), then the same wall run and rebound loop |
| `wall1_*` others, `wall0_*`, `crate_climb`, roof runs | 0.05-0.42 | same | |

Climbing in general, where the live logs have none of it (no corner, side leap or drop in any of the 21 routes; the
climbing seen is the wall run's catch, the hang's steps, up the wall, the ejects): checked against Banned445's port and
the exe. The climbing stance turns inside corners before the reaches and outside ones after (`xx_l_climb_1m_corner_<side>_090_<in|out>`,
TrySideJump 0xDF29E0 near / far), steps off sideways onto the ground first (0xDF4670), and the hangs turn corners about
the hands (TryTurnCorner 0xDD3BB0); ours does all of these. Not done as AC1 does: TrySideLedgeGrab (0xDF22C0), from the
climbing stance round a corner straight into a hang on the side wall's ledge (`xx_l_climb_1m_<l|r|ul|...>_hang<wall|free>`
turned onto that wall); ours goes into the hang along its own wall and then round the corner. AC1's free-hang shimmy uses
the same sweep, 2.4 m down.

The turn round under a beam (Ledge event 10) **[V, ours]** 2026-10-26: the interpreter sends it for the stick more than
135 degrees off the facing (0xEECCCA); its guard (0xDD30E0) probes a hold pair 0.45 m on from the hands (a box facing
back, 70 degrees), the free-hang root under that pair, a box 1 m up from root to root and the free hang's body boxes
(0xB2DD50) there; 0xDD3490 plays `xx_h_hangfree_turn_beam_180` (0x2F3C2551), the root carried over the action onto the
far side's free hang (Banned445's `try_turn_under_beam`, taken before the down moves). Ours (`turn_under_beam`) since
2026-10-26; it had none. Gauntlet G28.

### Jumps to targets and their landings (2026-10-13)
Three user reports in the test world, each a jump at a target ending somewhere AC1 would not put it:
- **The swing jump onto a top, post or beam** (report 60396): AC1 jumps off a bar at its target as at any target
  (`Human__SetupJumpToTarget` kind 3: Banned445's port `SwingJump`), the root on the clips' way plus the linear
  correction, so it ends on the target and goes into the free-step reception. Ours flung the root at it under gravity
  with a plain fall's drift slowing it (4 m/s²), came down 0.5 m short on B6's near lip and played the edge landing,
  stepping back off it 4.7 m; AC1's edge landing is only for falls with no target. Now (`swing_onto`): the swing's
  takeoff, the running jump's flight onto a free step and its `_tr_freestep_entry_footr` reception, the top landed
  `SWING_TOP_IN` (0.5 m) in from its edge; onto a post or beam the same, then the post's own entry. Bars and holds keep
  the old flight.
- **Report 46392 is AC1's own rules at work**: running off B4 toward B5's north-west corner, B5's edge is 1.2 m up at
  4 m, just outside reach zone 1 (1.15 m there), so no target; AC1's free jump flies the stick's way (Banned445's port
  1753486, live: IHuman vt68 gets the query's direction), the feet meet the corner 0.2 m under its top, the proxy steps
  up onto it and the edge landing steps off over the west edge, facing the drop, the way he was going (`step_off_back`
  is the one along the facing). Not a backward step off; scenario kept. Its later eject at B3's wall goes through the
  wall-hang reception below.
- **An eject at a wall hang lands in the wall-hang reception on that hold** (report 423408, audit WALL-10): the running
  game's eject at a hold flew `air_front_050cm_footr_to_swing` into `hangwall_reception_front_straight_min_a` and the
  wall hang. Ours flew the plain arc at the hang's root and let the fall's catch take the first hold it met: off the
  test level's wall run, one 0.6 m higher, into the climb's crouched catch (`fall_tr_climb_min_b`, the hips down at the
  feet), which read as reaching down for the lower hold. Now (`wall_hang_eject`): the rebound takeoff, the hang flight
  and `xx_hangwall_reception_front_<straight|30_out|45_in>_min_a..c`, ending 0.44 m out and 1.1 m under the aimed hold
  (where the wall hang's rest puts the root). A hold with no wall under it is still caught by the fall (free hang).
  **[ours]** At an inside corner, a hold on the other wall all but straight away (its wall across the way, the takeoff a
  front one turning little) has no start that both faces away and ends facing it: ours starts turned away as every
  takeoff and turns the rest onto the hold's wall over the flight (up to 100 degrees, `EJECT_CORNER_TURN`). It refused
  that start before and the fall flew him on facing away, caught the hold side on and turned 70 degrees in the catch
  (user recording 56678, Damascus; scenario `Damascus inside corner`). How AC1 faces that flight is unchecked.

### The climbing round two (2026-10-13)
The sweep's classes now follow AC1's own checks (`climb_sweep.rs`): `topout` only where AC1's room over the edge is
free (`climb_out_room`, 0xB2E240), `stuck` in a climbing stance only where AC1's grid move has its foot cell two rows
under the hand cell and its body boxes free (0xDECD70, 0xB2DF90), `embed` only inside a closed solid (five of six axis
rays meeting faces from behind), `free-on-wall` measured at the hold over a free hang's root (AC1's hang point, 0xB15AD0).
Traced with `AC1_CLIMB_WHY` (now also the faces in a refused room box or body box): the four Damascus top-outs and the
test level's three were all AC1's own room refusals (a lintel 0.5 m over the hold, a ledge 0.25 m deep before a wall
going on up, a roof face over the hands, the gauntlet's slab); the two sideways stuck spots AC1's (a buttress's side in
the body box; no foot cell, the feet on the ground); the climb start that "eased the body into a wall" a Damascus wall
wound inward over 9.2 m (the hips 0.38 m out of it). `AC1_SWEEP_ENTRY="lo,hi"` picks the walls by another first-hold band.

Fixed, by AC1's rule:
- **Top-outs a step up**: AC1 tests no floor, and its room test leaves 0.35 m over the edge free to 0.3 m in; ours
  turned down any top over 0.2 m over the grip. Now 0.35 (`TOP_OUT_STEP_UP`). A Damascus roof edge (user report 1300517,
  the top 0.24 m over the grip of the hang after the free-to-wall switch) and the window cap of 194384 (the balcony
  floor 0.31 m over the grip).
- **The one-hand pull-up stands where the two-hand one does** (AC1's Pullup_Start 0xDDBE80 steers both to one point in
  from the hands): ours ended it where its clips put the root, 0.1 m in, on the window cap's point, and fell.
- **A knob's reception**: both hands on the knob from the catch on (AC1's hand goals are their contacts); ours had no hand
  IK in the reception and its clips hold the hands 0.6 m apart for 1.8 s (156949).
- **The hang-type switch's foot rays** start from the hold point (TrySwitchHangType 0xDE1060), not the wrists' target.
- **An eject started inside the update** (the wall run hang's, after its rebound pose) snaps its takeoff's turn away at
  once, the root turned with it (WALL-8's `snap_takeoff` waited a frame: the hands 0.2-0.3 m off for that frame).
- **The climb start's band**: AC1's guard (0xB2E860, read in Ghidra) probes a foot hold from the feet up to 0.9 m and the
  hand hold 1.2 m over it, 0.3 m either way: holds up to 2.4 m over the feet with a foot hold under them. Ours took 0.3 m
  round its clip's 1.8 m and jumped at the rest.
- **AC1's reach capsule** (StartReachMove 0xDF6810, Banned445's port): the vertical reach sweeps the body's capsule (0.35
  m, 1.8 m tall, 0.15 m out from the root) to the new root. Under a Damascus plank walkway sticking 2.2 m out over the
  street from a wall's top (the real AC1 has the same planks, live check 2026-10-13) ours reached up through the planks
  and stood in them (1319499, 13501). Along the wall either way he climbs on out from under them. [ours: what AC1 does
  there with the stick up is not seen.]
- **AC1's wall-hang jump target** (the scorer's 0x40, its class having the wall under the edge) is flown into the wall
  hang whatever ours' own wall ray finds: off a Damascus roof at the roof edge across the street (1310993; the edge 0.45 m
  out over the facade) the ray fell short, the jump flew the plain arc, met the wall under the edge and fell 2 m onto a
  window's frame (AC1's own holds there).

- **The jump up from the climb and the swing back hang at AC1's free hang root** (2.4 m under the hold, 0xDD6730): both end
  in the free wait, its wrists 2.3 m over the root; at the fall catch's 2.1 the hands hung 0.2 m over the edge through
  the swing back's 2 s (62377).
- **The stance's hands reach AC1's goals** (the grid cell's holds, up to 0.3 m along and 0.15 up or down off the clip's
  hands): ours pulled a hand onto a hold only within 0.2 m; on a Damascus window box's frame both gripped the air (85785).
  A goal within 0.5 m is reached, eased from the nearest hold. The hand takes the hold nearest it once its pull is under 0.1 (it was
  only at 0, which the pull never reached between grid moves: along a Damascus wall a hand kept a hold 1.2 m back for
  four moves and was pulled 0.2 m off every hold for 2 s, the sweep's `w27-side`), the offset taken at once on a switch.

Not done: the climbing stance's hang round a corner (TrySideLedgeGrab 0xDF22C0, its side columns' hand row into a hang
on the side wall, the hang's boxes and the path round the corner, 0xB2A410): no test spot with a side wall's ledge and
no climbing holds was found, and ours' corner hangs would need the lateral path test. The routes `roof2_run_east` (a
ground turn's break into the ledge stop) and `hay70_free` (from the haystack rim the query takes the box's own far rim,
0.9 m, where AC1 jumps 4.4 m; AC1's query does not leave out the object stood on, **[V]** 2026-10-24: the only entity
its ray tests skip is the character's own, 0xB13260 handing 0xE18970 his entity as their one-element ignore list, so the
difference there is in the edges taken or the scorer; 2026-10-24: the running game took off from the rim as it stood up
on it, the stick 4-22 degrees off the rim, and landed 5.8 m on at (59.67, 12.00, -34.81), in its beam mode, on the middle
of a wall top 0.58 m wide (authored ledge grabs 47126 and 47123) running 40 degrees off the rim, 0.9 m short of a wall
crossing it. Ours stood up and walked the rim. Not explained offline: AC1's per-query beam pairing (0xE19BE0 on the
box-clipped ledge grabs, Banned445's port, tried) pairs the far face 47123 first, the longest, with a seam 0.04 m behind
it (46809), which its workspace cuts, so no beam lies there; the jump query's depth filter drops 47126 in ours; no edge
or far-edge candidate gives that point. Needs a live run with the query's candidates logged. `AC1_JUMP_QUERY_AT` logs
ours' query and pick from a point, `AC1_HOLDS_AT` now the authored pieces there too). Pops: the post pull-down's
0.2 m (the root turning 180 degrees over the 0.47 s `front_orientation` clip, a turn the pop check, relative to the root's
position only, reads as a jump), the swing back's hand (the clip's own 8 m/s swing), the landings' 0.15-0.18 m.

Sweep runs with each class (dev c4f4154 / this round): Damascus (147 runs) embed 5 / 0, topout 6 / 0, stuck 2 / 0,
pop 5 / 5, hands-off 2 / 2, flipflop 1 / 1, free-on-wall 1 / 1; test level (102) topout 3 / 0,
free-on-wall 4 / 1, pop 3 / 3, embed 1 / 1.
Damascus again on dev 2026-10-24 (152 runs, after the day's seam, ladder and corner fixes): pop 3, hands-off 1, stuck 6
(4 of them hangs with the stick held along a wall's end: the free hang's `e0` / `e5`, the wall hang's look-round at `e2`),
no-grab 8 (two walls never reached from their starts, all four programs each), embed 1 (a sprint stop into a wall,
`w25-run`, not seen again on a replay), flipflop 1, free-on-wall 1, no-hold 1.
Damascus on dev 2026-10-28 (152 runs, after the impulsion's own-rate play, 6598c4c): pop 0, stuck 4 (the hangs `e0`, `e2`,
`e4`, `e5`), no-grab 8 (the same two walls), hands-off 2 (`w15-run`, `w27-side`), embed 1 and flipflop 1 (`w25-run`), no-hold 1,
free-on-wall 0.
- **`e0`, the free hang in a recess** [V the box]: under a Damascus ledge the wall below is set back; the free hang hangs in
  the recess and its side wall stops the shimmy back toward the wall it came from. AC1's switch onto the wall (0xDE1060)
  tests the wall hang's box (0xB2DC80) under the lead hand's hold as it is now (0xB157B0 is handed +0x5F0 / +0x600, the
  hands' holds, +0x630 / +0x640 their normals; the new holds only tilt the box's up from the feet's wall to the hand).
  Ours tested it at the new holds, which straddled the recess's side wall; it now tests it at the lead hand. Still stuck:
  ours' own side rays from the low free hang root meet that wall, where AC1 sweeps the body between the two hangs
  (0xB2A410, 27 KB, with the free hang's 2.4 m and the wall hang's 1.1 m; not ported).
- **`e2`**: a wall rises from the ledge 0.8 m to the wall hang's right with no holds on it: the hang stops and looks round,
  as the running AC1 did at the bureau's wall (route `wall1_hang_right`). Not a fault.
- **`e5`**: round an inside corner in the wall hang, then into a free hang; back toward the corner the next face has a wall
  under the feet, and ours has no corner from the free hang into the wall hang (`xx_h_hangfree_corner_*` wants free feet
  on both faces) while the switch's slide runs into the corner's other wall. What AC1 does there wants a live look. `e4` (2026-10-28) is
  the same: round the inside corner, down a row into the free hang (`xx_l_climb_1m_ul_hangfree`), and every move back
  toward the corner meets its other face 0.17 m along.
- **`e13`, a free hang against a wall** [V]: the climbing stance's ledge grab (TryLedgeGrab 0xDF0980) takes the wall hang
  when its foot probe (0xB16130 -> 0x11718B0) meets a wall: a box 0.5 m wide and 0.3 m high, 1 m under the hold, swept
  from 0.5 m out to 0.75 m behind it, at the side and the middle; else (or with no room for the wall hang's box and
  path) the free hang. Ours wanted the clip's feet flush within 0.32 m; a wall 0.39 m back gave a free hang against it.
  The grab into the wall hang now takes the switch's foot rays (0xDE0A60, to 0.7 m behind). The window cap's scenario
  (user report 2026-10-13) goes into the wall hang there now, not the free hang and the switch onto the wall.
- **`w9-down`** [ours]: down the climbing stance the table move's hand target snapped to the nearest hold a grid row (0.6 m)
  down, here the row the feet stood on (11.70 m), whose foot cell is empty; the 12.02 m row the climb came up by, 0.25 m
  further, was not tried, and he looked down 2 m over the ground. A grid move's hand now takes the nearest hold in the
  box with a foot cell under it, else the nearest; he climbs back down the rows he came up. How AC1's cells (0xDECD70)
  pick among holds in one cell is not read yet.
- **`w16-side`** [ours]: at the bottom of a wall beside a block, sideways: the plain step-off (`xx_l_climb_1m_to_groundentry_left`)
  ended inside the block over the step, and the turned one (`_90`) is AC1's only where the ground's edge faces out; he
  hung looking left. The turned one now stands in when the plain one has no room.
Acre and Jerusalem, first sweeps on dev 2026-10-24 (30 spots each): Acre 150 runs: no-grab 10, embed 4 (not seen again
replayed alone), topout 4 (two low walls whose tops are enclosed by a solid: AC1's room boxes test faces only, ours' cramped
test refuses them), stuck 3, pop 2, death 1 (a run off a roof 17 m up), no-hold 1. Jerusalem 144 runs: pop 6 (all under
0.2 m, inside take-off and leap clips), stuck 2, embed 1, free-on-wall 1, hands-off 1, no-hold 1. The Kingdom 147 runs: no-grab 12
(three starts never reach their wall), pop 7 (two the respawn after a run off a 40 m drop, the deaths 2; three at one spot,
not seen alone), stuck 5, no-hold 4, topout 2, free-on-wall 1, hands-off 1. Most of its hang faults are on a battered fort
wall (`e6`, `e10`, `w10`): the wall under the ledge slopes back 0.4-1.1 m behind the hold and changes along it, and the hang
flips between finding and missing it under the feet; what AC1 does on those wants a live look.

### The routes again, and four new ones from the running game (2026-10-14)
The 21 routes replayed on dev 98deb3d (before) and with this round's fixes (after); four new routes recorded in the
running game through ac1-hook (keys, the live log; `F:/Claude/ac1-wt/reports/live-logs-20261014-parkour2/`, cut by
`routes.py` in `reports/routes-parkour2/`, the bureau's wall `wall1`). Mean root gap (m), the 2026-10-13 figure first:

| route | 10-13 | before | after | what changed |
|---|---|---|---|---|
| `wall1_climb` | 0.16 | 0.61 | 0.35 | ours pulled up one-handed from the climbing stance (and stood 0.25 s); AC1's climb out from the stance is always `xx_l_climb_<1m\|2m>_tr_hangknee` (TryReachLedgeAbove 0xDF1730, read in Ghidra: only action 0x35F11AF + the pose's "apart" flag). The 10-13 figure hid a 0.25 s stand under a slower walk start |
| `w1_climb_down` (new) | - | 0.28 (end 2.16) | 0.08 (end 0.05) | two rows up, the stick down: the running game reached down once and then looked down; ours reached down twice. AC1's vertical reach (0xDF0050 / 0xDE8400): a foot hold 1.2 m (then 1.8 m) from the feet's holds with a hand hold 1.2 m over it, now ours' gate on its climb-to-climb reaches (`reach_foot_hold`, levels with authored holds) |
| `w1_stance_side` (new) | - | 0.42 | 0.27 | in the 1ru pose on a column of holds 0.45 m wide, the stick left then right: the running game looked round each way; ours shuffled 0.3 m left (a hold 0.4 m along taken as the next column's) and settled a row up. Now a short sideways grid move needs its hand's hold within 0.3 m of the cell a column (0.75 m) along (`BuildHoldGrid` 0xDF6A40), and stuck sideways ours brings the hands level only with somewhere to go that way (`reach_holds_toward`, `side_exit_toward`), else AC1's look-round (0xDF4410). Left: the back eject at the end flies to the free hang behind 0.2 m off the running game's |
| `w1_hang_left` (new) | - | 0.07 | 0.07 | seven wall-hang shimmy steps left: as the running game |
| `w1_climb_side` (new) | - | 8.01 | 8.08 | not comparable: the running game started from standing with the stick 49 degrees off the wall and its first frame turned the body straight onto the stick; ours' start turns it over 0.5 s, so ours met the wall 32 degrees off and ran up it where the running game slid along into the corner and ran up the cross wall (a ground start's turn, not changed). Its later moves (the free-step stay's jump on, a leap right, the drop into the hang) are what the route is kept for |
| `roof1_free_south` | 0.90 | 0.77 | 0.64, then 0.30 | (the free-step stay below; dev's own changes); then the receptions below |
| `roof2_free_east` | 0.42 | 0.41 | 0.50, then 0.38 | the free-step stay's jump on now at the running game's moment. Then (2026-10-14): a free-step jump down on a top hands over to the ground at the walk (`PERCH_OFF_WALK`; the running game went into HumanGround on `walk_slow`, ours at the run and led by 1.1 m), and a running jump's reception onto a top plays 0.17 s past its step (`RECEPTION_STAY`, AC1's stay) |
| `hay70_free` | 1.08 | 1.05 | 1.05 | unchanged: from the haystack rim's stay the running game jumped 4.4 m on; ours finds no drop edge ahead there (the rim's hold pieces stop 0.15 m short of the box's corner) |
| others | | | same within 0.02 | |

Also AC1's rule, from the same routes and Banned445's port: the free-step stay (every free-step arrival and climb-up
onto a top is a HumanNarrowObject reception) takes the jump on from half of the reception, free running with the stick
toward a drop edge (0xB1C100) and no beam piece ahead (0xB15DE0), a haystack that way still its dive; the wall run's wall
test is along the facing with the wall facing back within 45 degrees (0xE18390; ours cast along the stick and took any
upright face); the wall hang's corner turn is refused where the body's way round, the root turned about the hands,
goes through something (Masyaf sweep `e0-hang`'s embed; AC1 tests the hang boxes and the lateral path, 0xB2A410).
Masyaf climbing sweep (141 runs), before / after: embed 3 / 2, stuck 3 / 4 (e0's embed became a look-round at that
corner), free-on-wall 3 / 3, pop 5 / 5 (four at one spawn), death 1 / 1 (the drop program letting go 35 m up). Left:
e12's one-hand pull-up from a free hang onto a post grazes the post with the chest for 4 frames (the clip itself, the
correction is not it); e3's corner out of the climbing stance into the hang.

### The 41 traced routes, headless (2026-10-26)
The 41 Damascus routes recorded in the running game as frame traces (ac1-hook's `trace into`: seconds, position, yaw,
context, actions; the keys from the hook log's frames, matched to each trace by its first frame, as the hook's frame
count restarts with the game) replayed in ours in headless control mode (`AC1_CONTROL` + `AC1_HEADLESS`, the city from
its cache, three games at once: the 41 in about three minutes; `AC1_TRACE=<file>` on a `run` line, the path with no
spaces, as a control line splits on whitespace). The scripts are `tools/routes/` (`routes.py ours`, `report`, `side`,
`diff`; `real` records the running game's runs through ac1-hook); the traces go to its git-ignored `lib/` (game data). Ending within 1 m of
the running game: 29 of 41 on dev 9ce9dc2 (mean path gap 0.47 m), 31 with AC1's mount guard above (0.50 m: `roof2_free_nw`
above), 32 (0.47 m) with a running jump's reception handing over to the ground at the walk, as the free-step jump's does:
in all 15 running-jump receptions onto a top in the traces the running game went into HumanGround on `walk_slow` at
about 3 m/s, 4.5-5 over the next 0.2 s and 6-7 after; ours went on at the run's speed (`roof2_free_north` led by 0.6 m,
`seams_free_south` 1.37 m off at the end, now 0.39). Ours' reception is still faster than AC1's (about 5 m/s over its
last 0.2 s against 3.1; the `_fast` share of the reception's blend comes from the human's weights at +0x1c4, copied in
by 0xE07D00 through 0x502E30, whose writer is not found; the reception played without `_fast` changed no route).
Tried and dropped: cutting the paired beams only by triangles reaching their top (AC1 cuts by the guidance object's own
shape, which ours has not loaded; the roof-edge beam on roof2 north, its west side on the roof 4-12 cm under its top, is
cut away whole and the running game jumped onto it): Damascus went from 5218 beams to 8022 and the routes from 31 to 28
within 1 m (`roof1_free_east`, `seams_free_west` jumped onto the new ones).
33 (0.42 m) with the free run's edge jump first at a roof's rim: AC1's mount guard is skipped where the edge jump goes
from where he stands, and ours' own `beam_step_on` (a beam's end just ahead at an edge, walked onto) is off in the cities;
at roof2's north-west and west rims the running game jumped where ours walked onto a beam sticking out under the rim
(`roof2_free_nw` 5.3 m off at the end, 2.4 now; `roof2_free_west` 2.8, 0.4 now). The traces hold no poses (path, yaw,
context and actions only); the hook's live log has the bones and IK goals, so a pose comparison is the next step.
The free jump (2026-10-27): the running game's seven in the traces (`xx_h_beam_air_front_*`, no target) are one shape:
the takeoff makes the rise, 0.55 m over the takeoff each, the flight leaving at about 0.3 m/s up; then -0.3, -1.5, -2.9,
-5.6, -8.0, -8.5 m/s over its 0.1 s steps, every one within 0.4 (about 13 m/s² down to 3.3 m/s, then 30 to 8.5, held:
`FREE_JUMP_UP`, `FREE_JUMP_GRAVITY_EARLY`, `FREE_JUMP_HEAVY_FROM`, `FREE_JUMP_GRAVITY`, `FREE_JUMP_FAST`). Ours had
launched it at 3 m/s up on top of the takeoff (0.4 m higher, at its top 0.2 s later) and come down at 20 m/s² from the
top: on `roof1_free_east` its height off by up to 1.4 m, now 0.3. Mean path gap 0.42 to 0.41 m. Its flat speed is the
takeoff's own (`run_front_550cm` leaves at 7.2 m/s, the running game's flew at 7.0-7.8), not held to the run's 6.5 as
other jumps' are: `roof1_free_east`'s flight within 0.15 m along. The free-running landing roll in a city
(`xx_h_landing_damage_<foot>_roll`, over 3 m) is not held at a rim: on `roof1_free_east` the running game rolled 0.2 m
past the roof's rim and jumped once the ground was gone, 0.17 s into the roll (the roll's item is locked; the jump is
`run_front_050cm` off the roll's own foot, into `air_front_050cm_*_to_freestep`); ours stopped the roll at the rim
and played it out (0.65 s), now the same clips from the same place within 0.1 m. Each scripted run now starts from a
fresh character (`Character::fresh_run`): the gait's phase and the feet had carried over between routes in one game,
`roof2_run_south` 0.23 m alone and 0.64 after another. 32 of 41 within 1 m, mean 0.40 m.
`street_free_east` (1.34 m off at the end, 0.41 now; mean 1.17 to 0.38; 33 of 41, mean 0.38 m), up the Damascus bureau's
courtyard wall: the hop onto the box from a stand cuts its impulsion short and takes off from the right foot, as the jump's
does (all six impulsions in the traces: `footr_..._front_down_footl`, then `freestep_front_front_050cm_footr_to_air`
0.03-0.13 s in; ours played the whole 0.27 s, 0.3 s behind from the start); the wall run's catch measures a wall set back
under the hold from the hold line, not the wrists 0.09 m out (`RECESS_MAX`; the lip 0.26 m proud of the wall under it read
0.35 and was passed for a sill 0.75 m lower), and tries probe C's free hang before probe D's wall hangs, as
`UpdateWallingSubState` does (else the Damascus balcony's wall run caught a sill under its corbels in the wall hang);
AC1's reach capsule is a capsule, swept from past the start (the box round it reached 0.14 m past its sides and met a
bracket beside the climb); a triangle with no area in the collision no longer makes a NaN that blocks every capsule
near it (the octagonal tower's reach); and the bureau's roof hatch `BA_Trap` (the closed bureau's) is left out, the
running game climbing to 0.7 m under where it lies (docs/NOTES.md). `roof2_run_east` (1.07 to 0.25 m) and
`roof1_run_west` (0.91 to 0.35 m), a pull-up onto a roof's rim with the roof lower behind it: the narrow top is told by
the hold's own height (the hang's wrists sink 0.12 m under the rim, and the roof 0.15 m lower read as level with them),
stood on 0.1 m in from the hold's edge (AC1's knee point; the running game 0.094 m in, ours had stood 0.265 m in from
the wrists, 0.12 m short), through the left foot's stand out of a hang (`hangknee_footl_tr_freestep_entry_footl`, the
right foot's only out of the climbing stance, as in every trace); the stand hands over to the ground from half its
reception with the stick back over the edge (`CheckSupportAndFall`), and the edge lock of the ledge stop before is let go
on hanging, so the turn on the spot back to the edge breaks into its ledge stop at once, as the running game's did
(34 of 41, mean 0.37 m). `roof2_free_south` (0.84 to 0.58 m; at rest 1.75 to 0.09): the free-step stay with no drop
edge ahead hops at a target, AC1's jump kind 4 with the legs held, 3 without (the narrow interpreter's request 0xEE9564,
Banned445's `jump_kind`), from half its reception: stood up on a roof's rim with a wall's top 0.8 m higher 1.5 m on, the
running game hopped onto it 0.13 s into the stand-up; ours stood it out and walked off the rim. A wall run comes first.
(Kind 3, without the legs, is the port's reading; the traces show only the legs held.) Left there: the jump onto that roof was a target
jump (`run_front_050cm`, `air_front_050cm`, a soft landing), ours the free jump 8 m on (`air_down_800cm`, a hard one).
`wall0_free` (0.92 to 0.63 m; mean 0.56 to 0.33): the wall run's kick off the top holds still 4 frames once in the air
before it falls (`REBOUND_HOLD`; the running game's root stood at the top 0.067 s into HumanInAir, both times, then
fell at a steady 7 m/s). (It landed into the walk here for a while, read off the trace's clip name, which is the landing
action's first clip whichever of its six play; the time it was left, 0.317 s in, is the sprint takeoff's mix, the pad's
speed ratio 1.0 (0xEDBAF0, Banned445's `speed_ratio`), and ours lands by that ratio again: 0.63 to 0.35 m, mean 0.25.) `roof2_free_south` (0.58 to 0.45 m): landing free running head-on
against a wall to run up (the legs held, the wall going on 0.5 m both sides), the landing is left for running at its
gate, and the wall run starts off it, 0.35 s after landing as the running game's; ours played the landing out first.
`roof1_free_east` (2.79 m) after the roll onto the lower roof: the running game jumped 0.4 m (`run_front_050cm`) onto a
ledge 1.3 m down under the roof's overhang and hung on its edge in the air 0.8 s before falling to the street; ours'
candidate query drops that ledge (no room on top behind it, the port's room ray), aims at a diagonal beam 1.2 m on and
stands on it. Left.
The low-profile jog stop (`roof2_free_east`, `seams_free_north`, `seams_free_west`, `wall1_climb`; at rest 1.08 to 0.45,
1.52 to 1.07, 0.74 to 0.12 and 0.86 to 0.23 m): let go at 5-6.5 m/s, the running game eased down to about 2 m/s over
0.15 s before its 0.2 s `xx_h_jogstop_<foot>` began, 1.15 m in all, mostly on the right foot; ours played it at once and
went 0.33 m. The run now eases down to 2.5 m/s first (`JOG_STOP_FROM`), the foot the cycle has then (33 of 41 at rest).
Banned445's port is a lead, not AC1: a rule resting on it alone is checked against the running game when a trace or a
recording can show it, and the traces have overruled it before (the landings' going-on buckets, the free-step stay's
wall test along the facing). `py tools/portonly.py` lists the comment blocks that cite the port and nothing from the
running game (174 of 239 on 2026-10-28, 111 of them in `climb.rs`), the item each documents and the address it cites:
the checklist for live comparisons.
`roof2_free_ne` (10.70 to 1.96 m; at rest 13.64 to 0.80): free running at exactly 45 degrees across a beam's side edge
that the forward line misses, the candidate query's "faces the jumper within 45 degrees" tie went to taking it, and ours
aimed 37 degrees off the way at the edge and walked the beam; the running game made its free jump on into the street.
An exact 45 now stays out (`FACING_45_COS`). (36 of 41, 34 at rest, mean path gap 0.23 m.)
`street_run_east` (0.37 to 0.08 m): walking into a 0.66 m step 0.2 m deep with more risers over it, the running game put
a hand on it (`xx_h_collide_full_hand_070cm`); ours' foot-up collide stepped onto it and went 0.36 m further. The foot-up
collide now wants its top 0.3 m deep (`COLLIDE_FOOT_ROOM`), else the lean's hands. Then (0.08 to 0.03 m, mean 0.21 to
0.09): off the street the running game got onto the 1.16 m box with the impulsion and the free step at 2.3 m/s too, as
in all four impulsions up in the traces (tops 1.05-1.32 m up); ours took it only under 1.9 m/s and flew the run's
`air_up_300cm`. It now takes it at any speed onto a top over 1 m too near for the takeoff (`VAULT_IMPULSION_OVER`); at
lower tops, or at any speed and distance, five roof routes broke.
`seams_free_north` (0.71 to 0.26 m; 35 of 41 at rest, mean path gap 0.22 m): mounted onto a beam from the roof at 6 m/s,
the running game went along it at 3.7 m/s at once; ours stood a moment and started again from 2.1 m/s. The speed
brought on carries into the walk along it (`beam_carry`), up to the crouch jog's own. Not after a side entry: the
running game went on from it at 2 m/s, rising to 3.6 by 0.6 s, but 0.1-0.2 s later than ours' entry starts (ours'
reception before it is the `_fast` one), so carried there the routes `roof2_free_nw` and `roof1_free_sw` went 0.4 m
ahead.
`ladder68_walk` (0.53 to 0.23 m): walking onto ladder 68 in low profile, the running game stepped on with the high
profile's entry (`xx_h_wait_hipm_footl_tr_h_ladder_climb_up_r`, rising 0.5 m at once), not the low one the port picks by
profile; ours climbed 0.3 m lower all the way up. `ladder68_free` (climb 0.37 to 0.27 m mean): free running at it, the
running game ran up the wall onto the ladder (`xx_h_wallingfront_entry_footl_b`, then
`xx_h_wallingfront_step1_footr_tr_h_ladder_up_l`) where ours' wall-run width test (the wall going on 0.5 m to both sides)
missed the rungs, so it stepped on at the foot; a ladder up the wall there now waives it. Left there: off the ladder's
top the running game ran at 3.0-3.6 m/s for a second (the speed of the run without the legs, `wall1_hang_up`) before
it free ran again, where off the wall hang's top-out it free ran at 5.5 m/s at once (`wall1_climb`); ours free runs
at once, so the end went 1.16 to 2.26 m. One trace; parked.
`seams_free_east` (1.53 to 0.22 m), `seams_free_south` (0.39 to 0.11 m), `roof2_free_west` (0.32 to 0.15 m); 37 of 41
end within 1 m, mean path gap 0.22 m: in every traced free run going on to the ground off a free-step reception, the
running game stayed in `HumanNarrowObject` 0.15-0.2 s, the reception played out (still from 0.08 s), and only left it
early to jump on (0.08 s) or run up a wall (0.09 s); ours' ground gate let it go 0.07 s in and ran 0.3-0.5 m ahead.
Played out, the free run then comes back at once (2.0, 4.7, 5.4, 6.0, 6.5 m/s at 0.025-0.225 s, the reception's 0.2 s
blend into the gait: `QUICK_RAMP_TAU`), not at `RUN_ACCEL`'s start from rest. Left: after that the running game holds
6.5-7 m/s for a while, ours' run 6.2, so `roof2_free_east` and `roof1_free_south` trail by 0.3-0.4 m.
`street_free_east` (mean 0.54 to 0.44 m): the impulsion hop up a box free running took the slow reception (speed ratio
0), so its stay's wall run, from half of it, came 0.25 s after the running game's (its stay 0.1 s); now the free run's
quick one. High profile alone keeps the slow one (the port's ratios there are unmeasured).
`ladder68_walk` (mean 0.16 to 0.04 m), `ladder68_free` (mean 0.94 to 0.49 m; mean path gap 0.20 m): off the entry the
running game went through the ladder's wait for a frame (`xx_l_ladder_wait_r`) and stepped from it with the wait's
blend, `ASTOP_BSTOP` 0.15 s, which holds the step on its first pose: it stood 0.14 s at the entry's end before rising.
Ours stepped from the entry with the default blend and climbed 0.15 s ahead all the way up; a step off a move that is
not the ladder's own now takes the wait's blend. (The climb action holds both hands' steps as items, picked by the
feet, so the trace's `climb_up_l` names only its first item.)
The free run's start (mean path gap 0.20 to 0.19 m; `seams_free_west` 0.21 to 0.12, `seams_free_north` 0.18 to 0.12,
`roof2_free_south` 0.24 to 0.17): from a standstill the running game was at 0.5, 1.6, 2.9, 4.7, 5.5, 5.8, 6.2 and 6.4
m/s at 0.05-0.75 s (eight free runs in the traced routes, within 0.3 m/s of each other), easing into its top speed; ours
reached 6.3 by 0.55 s at the steady 13 m/s² and ran 0.2 m ahead. Past 5.5 m/s it now eases in (0.17 s), 0.02-0.12 m
off the real distance at 1 and 1.5 s. (The first 0.15 s is still 0.4 m/s quicker than AC1's: one stride, left.)
The high profile's jog (`roof2_run_south` mean 0.23 to 0.15 m, its largest gap 0.94 to 0.54; the other high-profile
runs 0.01 nearer): measured at 0.05 s on the three traced starts, the running game held the jog at 3.5-3.9 m/s to 0.7
s, was at 4.5 at 0.8 and 5.0 at 0.9; ours held it to 0.8 s and reached 5.0 at 1.05 (`gait::HIGH_START_HOLD` 0.8 to
0.7). After a landing in high profile (a 0.72 m step off) it came down at 3.4-3.9 m/s, held that to 0.65 s after the
touchdown and was back at 5.1 0.85 s after it, as after a start; ours went back to the run as the reception ended.
Ours now holds the jog after such a landing for what is left of the start's hold, and after any move handing back
to the ground in high profile (`Gait::regained_high`): over a wall's top on route `wall1_hang_up` the running game ran
on at 3.6-3.7 m/s for 0.75 s before rising, ours at the run's 5.1 (the route's window ends before; measured past it).
Free running is left as it was: off ladder 68's top the running game held the jog 0.8 s, over wall 1's top it went
straight to 6.4-6.9 (one trace each). Not AC1's
mechanism (unknown: 0xD7D6B0's test of +0x5B0 is only the ground's entry time, 0xD98D90), its numbers.
The step off's flight (`roof2_run_south` mean 0.15 to 0.09 m; mean path gap 0.18 m): **[V]** 2026-10-28 the step
off's enter (0xDA5D70 -> 0xD9D1F0) aims `1 + 2 · [+0x5E8] · a` m on along the facing clamped to within 60 degrees of it
(past 120 degrees it turns instead) and **3 m under** the takeoff, not at the ground there; +0x5E8 is the speed value
(0xD83510 reads the speed table by it), `a` the event's float (+0x44: 1.0 with +0x112B, high profile). At the run, 2.5 m
on: falling as the free jump does, 3 m takes 0.65 s, so 3.8 m/s across, and the roof 0.72 m down is met 1.2 m on; the
running game flew 1.15 m in 0.30 s at 3.4-4.7 m/s. Ours had carried the run's 5.1; it now flies across at the aim's
speed (`free_jump_fall_time`). (The jog held after that landing still rises 0.1 s after the running game's.)
Off a free-step reception onto the ground (`roof2_free_east` mean 0.25 to 0.16 m, `roof1_free_south` 0.23 to 0.17):
stepping off a roof's rim beam onto the roof, the running game was at 3.4, 5.0, 5.6 and 6.3 m/s 0.05-0.2 s after, then
6.5-7.0 to 0.5 s and 6.3 again by 0.6 (seven times in the traced routes, within 0.4 m/s of each other): it kept its
speed value. Ours held the value down to the run's (the speed it had, not under the run free running) and came back at
1/s, 5.5 m/s at 0.2 s, 0.3 m behind each time; off a free-step reception it now keeps it. (The 6.5-7.0 surge over the
sprint's 6.3 is not ours: the clips' root motion, likely.) The eject sideways onto the bar off the bureau wall (`wall1_eject_right`, `wall1_runeject_right`, 0.56 and
0.39 m): the running game came to the bar 0.4 m out from the wall's line both times, near the point ours' jump query picks
(0.5 m in from the hold's end, 0.2 m short of it); ours' eject aims along the wall at the crossing. Left: the query has no
zones for a jump off the wall.
`roof1_free_south` (0.76 to 0.43 m; at rest 10.8 to 5.6) and `roof1_free_sw` (2.82 to 1.74 m): the free-step stay with
its jump on to come (free running at a drop edge ahead) is not left at its item's gate, which let it go 0.07 s in; it
jumps on from half, as AC1 stays in HumanNarrowObject (off a roof onto a beam at its edge the running game jumped on
0.1 s after landing; ours ran onto the beam crosswise and crouched). And the free-step jump's reception takes its quick
version's share from AC1's speed ratio from the pad (`pad_speed_ratio`, HumanInAir +0x16C, 0xEDBAF0; Banned445's
`speed_ratio` / `reception_weights`), 1 free running: every real stay after a free-step jump jumped on 0.08 s in, half
of `_fast`'s 0.13 s; ours played the 0.27-0.4 s one.
`roof1_free_sw` (1.74 to 0.78 m): walking a beam onto a roof with the stick 45 degrees off, the running game went on
along it 0.45 m past its end (AC1's step off the end, `CanStepOffBeamEnd` 0xF77C00: from 0.4 m short, 0.8 m on) and
into HumanGround on `xx_l_walk_slow_hipm_footl` before turning; ours stepped off sideways 0.4 m short and hopped. Now
the walk carries `BEAM_STEP_OFF_PAST` past the end, no side step off within `BEAM_STEP_OFF_NEAR` of it, and the ground
goes on at the beam's speed and gets up to the run at `BEAM_OFF_ACCEL` (4.5 m/s²: the running game at 3.7, 4.0, 4.4,
4.8, 5.3, 5.8, 6.0 m/s over the next 0.6 s off all three beams' ends, where after a free-step arrival it was at 6 within
0.1 s; ours at `RUN_ACCEL`'s 13), turning meanwhile at free running's full 2.6 rad/s to within 8 degrees as the running
game did, not eased in (35 of 41; `seams_free_north` 0.86 to 0.71). (Gauntlet G5's run off the beam onto the box now
reaches its edge at 5.4 m/s and runs off into the hay below, where it stopped at the edge and dived.)
`roof2_free_nw` (2.66 to 0.23 m): free running along a beam with the stick 45 degrees off it, the jump at a target off
its side wants no beam piece ahead along the stick (the beam walked counts, within 45 degrees), as AC1's jump on is kind
0 only then (IHuman vt148, Banned445's `jump_kind`); the running game went on to the beam's end, ours jumped off 2 m
short (36 of 41, mean 0.35 m).
`hay70_walk` and `hay70_free` (at rest 2.33 to 0.40 and 0.32 m): pulled up onto the haystack's rim with the stick away
from the hay, the running game dived in both times, walking and free running; ours stood on the rim. It stands there
only free running with a target for the jump on (the stay's jump on, from half, comes before its hay search from 0.6),
as the live log of 2026-10-12 showed. (32 of 41 come to rest within 1 m.)
`roof1_free_sw` after its beam: the free jump's fall along a wall the run meets 44-45 degrees off caught a wall hang in
ours, where the running game slid down it into the street; Banned445's automatic catch wants a contact faced within 45
degrees (0xE0BB70), ours tests none; a tie on the bound, left.
The largest left: `roof2_free_ne` (8.3 m): running north-east at exactly 45 degrees past a beam's side edge, ours'
jump query takes the edge (it faces the run at exactly 45 degrees, the bound of "faces the jumper within 45"), the running
game jumped on into the street; a run scripted at 45.0 degrees cannot tell which side of the bound AC1 is on.

## Beams as AC1 does them (2026-10-14)
The online friend "went into walls, specially from beams" (replay 1791930058, user report 2026-10-13). `AC1_REPLAY_SCAN=1`
(with `AC1_REPLAY`) checks every frame's two roots against the city's collision (the capsule pushed out, the chest through
a wall face, the chest in a solid, on a perch or not) and logs `scan:` lines. It found ours' own beams: a drum's plinth
0.3 m deep against its wall (the friend balancing along it, the capsule 0.35 m into the drum) and a window box's sill (us,
0.45 m into the box), both beams invented from mesh names or single-hold tops. AC1 has neither. Now, in the cities:
- **Where beams and posts come from [V, B]**: AC1's beams only, its paired authored hold edges (`beam_pairs`, the
  `AC1_PAIRED_BEAMS` build, now the default; `AC1_OLD_BEAMS=1` keeps ours' old set), cut wherever anything stands in their
  workspace (0.5 m over the top, side planes 0.15 m past the half-width), so never against a wall; and AC1's posts
  (`sub_B2B600`, read in Ghidra: from a hold edge's middle, the edge facing back within 0.3 m, one facing on within 1.3 m,
  one to each side within 0.6 m of between them, all within 0.5 m in height and 30 degrees of the search, front-back and
  side-side at most 0.7 m apart; the top the four points' mean, kept only with a 0.4 m capsule free from 0.6 to 1.4 m over
  it): 5218 beams and 1374 posts in Damascus. No mesh-name beams, no single-hold narrow tops, no stub-end holds.
- **Catching a beam from the air or arriving on it [V]** (`sub_116D960`, the air catch 0xE0B890 and the free-step
  arrival): no floor within 0.5 m under the top at half-width + 0.2 m to either side, and no wall across 0.2 m over it.
- **From the ground [B]**: AC1's mount guard (event 72, 0xD9F4C0): the stick past 0.35, the beam piece running from its
  nearer end within 90 degrees of the stick at any angle, head room from the feet to the entry 0.5 m on along it. Ours took a
  beam only walking within 30 degrees of it. (The test level keeps ours' rule.) Read **[V]** 2026-10-24: event 72 goes to
  `HumanGround`'s handler (0xDB1470, entering 0xD898A0), its guard 0xD9F4C0: the beam query (0x116E1A0) over a box
  ±0.75 m across, 0-1 m up, ±0.53 m, within 90 degrees; the entry the piece's nearer end, moved half a step on; a capsule
  (radius 0.25 m) swept from the feet to 1.1 m over the entry must be clear; the feet within the piece's half-width plus
  up to 0.5 m (all of it level with the top, none 0.53 m under it) of its line; then a box (0.5 m past either side,
  1 m up) along the piece. The stick's 0.35 is not in the guard (the event's sender).
  Read again **[V]** 2026-10-26, and ours now does it in the cities (`climb::ac1_beam_mount`, before ours' own rule): the
  query box is in the stick's frame, 0.75 m to either side, **0 to 1 m ahead**, 0.53 m up and down (min (-0.75, 0, -0.53),
  max (0.75, 1, 0.53)), the angle compared with `<` against the π/2 argument (a 90 degree half-angle); the feet within
  `(1 - clamp(rise / 0.53)) x 0.5 + half-width` of the piece's end (3D, `rise` the end over the feet), inside a box about it
  (half-width + 0.5 m to either side, 0.5 m back to 1 m on, 0.5 m up and down); then the capsule. Ours had taken a beam
  only once the feet were over it, 0.3 m in from its ends: off the seams roof north the running game, the stick straight
  on, got on a beam starting 0.34 m ahead and 0.57 m to the side and curved onto its line (route `seams_free_north`, 3.46
  m off at the end before, 0.15 m now). Not settled: which end of a piece counts (ours: the nearer; AC1's clip of the
  piece to the box may keep the data's a-to-b order, which would make the cone one-way), and the edge jump's race with it
  (route `roof2_free_nw`: at a roof's west rim the running game made its free-run jump where ours now mounts the beam
  sticking out under the rim, 0.17 m below the roof, 0.49 m before its end; 2.67 m off at the end before, 5.32 now).
- **Speed [B]**: the crouch jog in high profile, the crouch walk in low (AC1's speed band by profile); ours jogged only
  free running. **[V]** 2026-10-26: the band's speed (0xE53090) is a fixed table off narrow objects (bands 0-3:
  1.039, 1.779, 2.873, 4.707 m/s) and NarrowObject's own on one (+0xA4, 0xF721E0), filled by 0xF7D3E0 from action
  0x28A3A8E4 (`xx_l_beam_crouchwalk_<foot>` at weight 0, `xx_h_beam_crouchjog_<foot>` at 1): each clip's root motion over
  its length, bands 0-1 the crouch walk's, 2-4 the crouch jog's. The action is one blend of the two, so the gait is mixed by
  the speed walked. Ours (2026-10-26): the speed goes to the profile's band (the jog's from band 2: high profile, free
  running) at +2 m/s² up and -3 down (Banned445's NARROW-1), the two steps mixed by it at one phase, the phase run at
  the speed over the mixed step's length. It played the jog alone slowed to 0.57 at its 2.1 m/s start, and kept the
  gait it started in through a change of profile. Let go, the jog stop plays over the walk's speed (0xF749D0), else the
  crouch (ours took it by the profile).
- **Ends [B]**: free running to a beam's end that runs into a wall holds there (no drop edge past it: AC1's
  `BeamRunsAlong` 0xF78F00 leaps only within 0.4 m of the drop edge at the end) and the legs held run up the wall (event 15
  reads the legs held). Ours leapt straight up into a balcony's floor (report #9). **[V]** 2026-10-26: NarrowObject keeps two pieces (+0x260
  and +0x2B0, each a start and an end; the second while +0x410 is 1, the piece being crossed onto) and a 0x80-byte end
  record for each (+0x5B0 / +0x630) whose +0x10 is the point the walk ends at (its +0x38 set: the piece's own end).
  `BeamRunsAlong` (its angle the argument): the velocity and the facing both within that angle of the piece's direction,
  and the root, carried along the piece to the plane through the record's point, within 0.4 m of it (flat). The end stop
  (0xF7C3A0) takes the same point: the facing more than 10 degrees (0.1736) across the plane, past it or within 0.31 m,
  holds the root 0.3 m short (flag 1 at +0x8B0). The record's point is written through a pointer (no store at +0x5C0 in
  NarrowObject's code): by `sub_F72FD0`, read 2026-10-26 (its +0x10 the drop edge's point past the end and +0x38 cleared,
  else the piece's own end and +0x38 set: the beam's run, below), as Banned445 read it. Ours as both.
- **The beam's run [V]** 2026-10-26: `sub_F72FD0` (0xF72FD0) moves each end of the piece on along it to the drop edge
  past it: the drop edge search (IHuman vt +0x90, `Human__FindDropEdgeAhead` 0xB1C100) from the end, −0.1 to 1.25 m on
  (plus `sub_1174260`'s distance past a connected piece), ±0.5 m up and down, a drop of 0.3 m or more, steps of 0.02 m.
  Found: the end record's point is the edge's (its height the end's) and its +0x38 (+56) byte cleared; none: the
  piece's own end, +0x38 set (`CanStepOffBeamEnd` reads it: only an end with no drop edge is stepped off). So a beam's
  walk runs on over what it rests on to the drop past it (Banned445's live note: the Bureau courtyard beam over its cross
  plank). Ours (`beam_run`, once per perch from the hold edges): the walk, the 0.3 m stop, the wait's hold on the line,
  the step off and `BeamRunsAlong` all take the run's ends; past the piece the root stands on the ground under it
  (gauntlet G30: a beam onto a 0.8 m block level with it, walked to 0.3 m short of the block's far edge, free running the
  jump on from there). The connected piece's extra reach is ours' beam chaining.
- **The jump kinds 3 / 4 on a beam [V]** 2026-10-26 (0xEEAAA0-0xEEAC0A, Banned445's `beam_jump_kind` branch for branch:
  the stick past 0.35 (0x17085D4), the drop under 2.0 (0x1683380) for kinds 1 / 2 with high profile (+0x112B) or the
  forward sector; high profile with the legs held (0x6D0F50(0)) at that edge kind 0, free mode 2 at a drop of 1.0 or
  less (0x1680F90), else 0 in the forward sector, else 2 when vt +0xA4 says so, else none; no kind yet: 4 with the legs,
  else 3; the stored stick past 0.35 with the press (+0x1126) and the legs let go: kind 0, free mode 0). BeamState
  (0xEEAAA0) picks a jump kind every frame from the stick, the
  profile, the legs held and the drop edge ahead, and any kind runs the candidate search, no press needed (Banned445's
  `beam_jump_kind`): in high profile with the stick and no drop edge under 2 m, kind 3 (4 with the legs held), whose
  query box is 0.45-1.3 m up and 0.5-1.3 m on (2 m with the legs), ±0.5 m across (0xE18970's volumes above). Ours
  (`beam_box_hop`, while walking or jogging the beam after the autoclimb, and from the waits with the stick across it):
  a top found there is jumped onto by the free step, a kiosk's counter by its own jump. So in high profile the step 1 m
  up at a beam's end is hopped onto from 1.26 m before it (gauntlet G25 in high profile), where low profile walks into it
  and autoclimbs (event 17). Every kind searches its candidates before its free jump falls back (Banned445's
  `narrow_jump`): ours' hop off toward a drop edge (kinds 1 / 2, `low_hop`) queries the edge zones 10-12 first (the
  primary only, which reaches down from the feet, none up) and jumps onto a top there (gauntlet G31: a block 0.6 m under
  a beam's end beside it, kind 2's side free step onto it; 1 m over it, out of zone 10, it is the free jump and the catch
  on its edge). On a post (NarrowObjectState 0xEE9564-0xEE969C, read 2026-10-26 **[V]**, as Banned445's
  `free_step::jump_kind`: the beam ahead from vt +0x94, the same 0.35 / 2.0 limits) high profile with the stick
  is kind 3 whatever is below (4 with the legs held unless a drop edge is ahead, then kind 0), in place of kind 2, its
  free jump kept for no target: ours searches the post's box every frame then (gauntlet G32: a block 1 m over a post,
  0.8 m ahead, jumped onto).
- **The empty hand's hop off [V]** 2026-10-26 (BeamState 0xEEA99E-0xEEAA9F, not in Banned445's notes): before the jump
  kinds, the empty hand held (`Pad__IsPressed`(3) 0x6D0F50: 0x484830 then 0x93F170, inlined), the drop edge report along
  the stick (IHuman vt +0x90 from 0xEEA60A with −0.3 to 1.1 m on, ±0.3 m, a drop of 0.5 m, steps of 0.02: ours'
  `DROP_EDGE_*` **[V]**) past NARROW-11's gate, a drop under 2.5 m (0x1708624), no next piece: `BuildFreeJumpTarget` mode 1
  with no report (1 m on, 3 m down), posted, in either profile. The pull-down (event 9) wants over 2.5 m, so the empty
  hand toward a low drop hops off where a deep one hangs. Ours (`hand_hop`, on the beam's walk and waits; scenario
  G27 in low profile with the hand held: the side free step down 1 m that way). Ours had taken the empty hand on a beam
  for the pull-down alone (audit NARROW-8).
- **Jumps with no target [B]**, the target **[V]** 2026-10-26 (`BuildFreeJumpTarget` 0xB1E7F0 read: mode 1 → 1 m on,
  mode 2 → 3 m, mode 0 → the character's tuning distance (8 m by Banned445's reading), each 3 m under the feet; with the
  drop edge's report, the edge's height less min(drop + 0.25, 3), then 0xB17D50 (0.7, π/2). The character field +0x994
  overrides it: 4 gives mode 2's 3 m, 3 gives 4.5 m on and 2.3 m down; Altaïr's is the general 0, as the ground's query
  kind-7 shows): AC1's free jump off a narrow object (the narrow interpreter's jump kinds, BeamState
  0xEEAAA0 / NarrowObjectState 0xEE9564, `BuildFreeJumpTarget` 0xB1E7F0): free running, only over a drop edge ahead along
  the stick (`FindDropEdgeAhead` 0xB1C100: the hold edges crossing the stick's plane 0.3 m back to 1.1 m on, within 0.3 m
  in height, a drop of 0.5 m or more past them), 3 m on over a drop of 1 m or less from a beam, else 8 m on (the stick within
  45 degrees of the facing; from a post always), down to the drop's floor + 0.25 m, at most 3 m; the beam set of takeoffs and
  flights (`xx_h_beam_freestep_*`, `xx_h_beam_air_*`) where the target is over 0.4 m down and the way to it blocked
  (0xB145B0). No drop edge: no free jump. Ours crouched and leapt 3.5 m on and 3 m up anywhere: on route `roof1_free_south`,
  on a beam across the way, it went straight up where the running game free-stepped on (end gap 2.14 -> 0.49 m). Not free
  running, the stick toward a drop edge under 2 m (within 45 degrees of the facing, or any way in high profile; along a beam
  only within 0.4 m of its end): the hop off, 1 m on in low profile, 3 m in high (kinds 1 / 2). The kind is picked every frame
  before BeamState's turns, so in high profile the stick across a beam toward a low drop hops off that way at once, the
  side takeoff (`freestep_<left|right>_*`) turning the body; ours made the quarter turn first and jumped from it, ahead
  (2026-10-26, gauntlet G27: a beam whose sides are hold edges, as AC1's).
  The free-step takeoff groups turn the body themselves (side 90 degrees, back 180, the back ones moving the root 2.5 m back
  off the start facing), so every free-step takeoff (`narrow_free_jump`, `free_step`, `jump_at_bar`, as `jump_freestep`)
  plays from the facing he has. Ours had planned these three already facing the jump: off a top's stand-up with the
  stick back (user recording 116252) he turned twice (-333 degrees), was carried 2 m back onto the top toward the wall,
  and flew off facing it, then snapped round 190 degrees on the catch (scenario `test level alley: the free jump back`).
  Left: the drop edge right behind that stand-up is not one of ours' hold edges (only a ledge off to the side is, so the
  jump fires or not by centimetres), and the 8 m target there lies through the alley's other wall, which he scrapes down
  (ours has a room check only off a swing bar, `free_jump_room`). AC1's narrow free jump has none **[V]** 2026-10-24:
  `BuildFreeJumpTarget` 0xB1E7F0 sets the point (1 m on for kind 1, 3 m for kind 2 or the beam, 4.5 m and 2.3 m down
  from a post, else the action's own distance, 3 m down; over a drop edge its floor + 0.25 m, at most 3 m) and 0xB17D50
  only turns the way to within 90 degrees of the edge's normal, pulls the point 0.5 m back and keeps it 0.7 m past the
  edge's line: no cast. A blocked way only picks the beam set (0xB145B0).
- **Posts [B]**: the body keeps its facing and the pilotis wait leans toward the stick (`_wait`, `_left`, `_right` by the
  stick's angle, 120 degrees full, at 3/s; 0xE51960); ours turned the body on the spot. **[V]** 2026-10-24: 0xE51960
  clamps the stick's signed angle off the facing to ±2.0944, eases +0x174 toward angle / 2.0944 by 3 × the step, and weighs
  the wait 1 − |lean| and one side |lean|, as ours.
- **The beam autoclimb** (event 17) **[V, ours]** 2026-10-26: NarrowObject's event 17 (its dispatch 0xF7F3B0, guard
  0xF7EF90 -> 0xF7DB30) takes walking into an obstacle on a beam: the obstacle guard 0xB25230 (a contact at least 0.5 m
  over the feet within 45 degrees, not a character), then the hold edges in a box ahead (0.3 m either side, up to 0.7 m
  on, 0.3-1.4 m up; not sub-type 7, 5 or 8) facing back within 45 degrees, each classified as the jump query does (room
  on top from 0.5 m out and 0.3 m over it, back 1.75 m: under 0.03 m 0x40, under 0.3 m 0x10, both refused), one picked
  (0xE97720). It schedules 0xF70E50, the standing jump 0xB21DA0 with its autoclimb flag: the height kept to 0.2-1.3 m,
  `xx_h_beam_autoclimb_<foot>_020cm` to `_130cm` by (h - 0.2) / 1.1 by the leading foot (actions 0x6DA7646E / 0x6EFC142E,
  their own blend 0.06 s), straight up; the arrival (0xE07D00) then plays `..._tr_freestep_entry_<foot>` (0x6DA7646F /
  0x6EFC142F), 0.5 m on, in NarrowObject. Ours (2026-10-26) does this from the beam walk; it stopped at the obstacle.
  Gauntlet G25. (The vertical poles' arrival, target type 0x2000, `xx_h_air_surface_tr_pole_verti_<up|down|long>_a..c`,
  has no pole climbing after it in the game's clips: not used.)
- **Climbing onto a narrow top**: the lean's climb needs room for the body on the top (the capsule clear of walls), as the
  plinth had none.
- Measured on the live-log routes with a beam in them (`wall1_*`, `roof*`, `seams_free_west`, `hay70_free`): unchanged or
  better (`roof1_free_south` 0.82 -> 0.77 mean). Scenarios: the plinth and window box have no beam; the lean does not climb
  the plinth; `roof2_run_south` runs on over the roof's end as ground (AC1's, ours had a beam there); the high profile jog;
  the post's lean.

Left (Banned445's port has them; ours does not yet): the support search in the walk too (ours: the waits; the walk's
current piece always passes) and its loss of support falling (0xF808D0: no piece found, NarrowObject's mode not 8 and
+0x2A8 / +0x2F8 clear, to state 8 through 0xF6ECF0; ours keeps the piece), one runtime report with the chains walked
through; the side entry's 30/90 blend
checked live (ours plays it, G29); BeamState's event 20, read but not placed (vt +0x13C / +0x140: pad button 1 pressed
within 0.2 s, 0x93F370, with the flag at the frame's +0x11; maybe the attack from above, combat's); whether a viewpoint's
beam end dives into its hay on the stick alone (event 22 below says it does, by the code; not seen live).
Done 2026-10-26: the beam ends extended to the drop edge (G30), the jump kinds 3 / 4
on beams and posts (G25 high, G32), event 22's dive into a haystack (G33), the support search in the waits (below). The
kind 0 mid-beam, read: its free mode
off the forward sector over a drop of more than 1 m comes from INarrow vt +0xA4 (0xE52FC0), true on a post (+0x164 is 2)
or with +0x881 set and NarrowObject's mode (+0x140) 10; so on a beam, free running with the stick turned to the side over
G27's 1.2 m drop, AC1 takes no free jump (kind 4's box only), as ours (it had been listed as ours' gap).
- **The support search [B, V, ours]** 2026-10-26: `DetectBeamSegments` (0xF753A0, called from the waits' and the walk's
  updates, 0xF7D650 / 0xF7EED0 and others) finds the piece under the feet every frame: three passes, each a box about the
  feet with half sizes 0.5 m across, 2 m along, 0.5 m up and down (the constants at its start), along the facing with the
  piece within 60 degrees of it (0.5), the same turned 90 degrees, then 1 m by 1 m with no cone; in a pass the current
  piece wins when it passes, else the nearest (Banned445's `detect_beam_segments`). Ours (`beam_support`, in the beam's
  waits): another piece found takes over, and from across the old one facing along the new, the 90 degree wait becomes
  the wait along it. So at the Damascus T (user recording 142770) the quarter turn to face along the crossing beam puts
  him on it, and the stick walks on along it; ours' own rule (the stick forward, the crossing going on past the walk's
  stop) is gone. [ours: a piece is taken only within 0.5 m of the feet, `BEAM_CROSS_NEAR`, as ours eases the root onto
  the support's line.]
- **The stick toward a haystack off a beam [V, ours]** 2026-10-26: event 22 (INarrow vt +0x144 / +0x148, 0xE542D0 /
  0xE555E0) is sent by the beam interpreter every frame the stick is past 0.35 (0xEE9F13, the pad's +0x5C0; no button,
  no free-run gate, before the jump events). BeamState's Idle, Walk and 90 degree wait take it (0xF7A4F0 / 0xF7F150 /
  0xF7A650; not the post's wait nor the free-step arrival, whose dispatches 0xE50BF0 / 0xE4F220 have no 22), each through
  the guard 0xF767C0: IHuman's +0xFC face vt +0x50 (0xB10AB0) runs the ledge let-go's haystack search (0xE17DA0: 1 / 5 m
  across, 2 / 10 m along, the top (floor + 1.6) 0-100 m under the feet, opened fully at 30 m down, 90 degrees, 0.1; ours
  `LET_GO_SEARCH`) along the stick from the actor's facing, and takes the haystack it finds when the flat way to it is
  within 30 degrees of the stick (dot < -0.866 against the actor minus the point). The action (0xF74650 / 0xF74590 /
  0xF74710) writes the haystack into the human's +0x1120 record (mode 1, 0xF72210) and changes to state 0x15,
  `HumanHayStack` (ground event 122's too), whose entry 0xE42420 plays `xx_h_freestep_footr_to_haystack_01` or `_02` at
  random (actions 0x244CD280 / 0x244CFACB), the root carried onto the haystack over the dive's length + 0.2 s (0x711130).
  Ours (`narrow_hay_dive`, `dive_into_hay`): first in the beam's wait and walk. G2's haystack moved from 1.4 to 3.6 m past
  its beam's end so its free run still leaps of faith (at 1.4 m the walk dived first). Banned445's port has none. The window-box side leap through a building's corner in the same replay is the in-air wall test's
(single rays at 0.3 / 1.0 m), passed on to the wall-kick work.

## Banned445's port, 2026-10-13 round
Banned445's AC1-Movement-Rewritten (MIT) moved on 233 commits (c81c555..5f081fa), each read from the exe. Ported here
area by area, written anew, each checked in Ghidra where it could be. **[B, V]**

**The jump off a wall (the user's "insta walljump at any point of an animation")** **[V]**:
- *When*: AC1 takes the jump off (event 2) only in two of `HumanLedge`'s states (its handler 0xDE3A30): Movement (the
  hang at rest and its own steps: shimmies, hand steps, the climbs out of the hang, look-arounds), refused while the
  playing item is locked unless it is a strafe or a swing back (0xDCCE10, 0xDCCD50) and in a free hang; and Entry (the
  wall run's catches), at any time in the catch into the wall hang (action 0x0106CFC5), else in the catch's first
  quarter (0xDCBF00). `HumanClimb` takes it in Wait, and mid-move only in a grid move or a corner climb (0xDF9230,
  the flag `ChooseMove` sets). Everything else on the wall refuses it: the corners, the receptions after a jump or a
  fall, the pull-downs, the climb's entry from the ground, its reaches and leaps, the transitions into a hang. The press
  waits in the Legs buffer, 0.3 s: taken as soon as a state that takes it comes, else dropped. Ours took it at any point
  of any move ending on the wall, cutting it. (The wall run itself: never in its entry's first half, at the end of the
  second, any time in the step up and the kick off the top, as before.)
- *The hop*: from a wall hang (and the wall run's catch) the rebound pose by the side of the jump, from the direction
  to the target found at the press (0xDD2CA0): to the right of the body facing the wall `xx_h_hangwall_tr_rebound_footl`,
  the root lowered 0.3 m × (angle off straight away, up to 45°)/45° over its 0.2 s; straight away or to the left `_footr`
  (ours: `_footl` unless the stick was to the left, no drop). From a climb always `_footl` in place (0xDEA560). Then,
  facing away, the rebound jump (kind 2) off the foot the pose ends on (its item's feet, 0xDD0A40 / 0xDEAE30): the
  32-clip `xx_h_rebound_<frontleft|frontright|left|right>_*_to_air` takeoff blended by the target and the side angle
  (0xB1EC40, slots as Banned445's 9369c0b: the angle's sign picks frontright / right or frontleft / left, `t = |a|/90°`).
  With no target, the free target (8 m on, 3 m down; the wall run's own 7 m, 0xE365C0): its far band's 550 cm push off
  (ours had the 300 cm one for every free eject).
- *The wall run's rebound*: the push straight back with the stick within 50° of the facing or let go, else along the
  stick turned in to 89°; off the left foot (every walling item ends on it, 0xE36660); at a target, else AC1's fallback
  above. Ours, with no target, played `xx_h_wallingfront_<entry|step1>rebound_<back|left|right>_*`: no code of the exe
  asks for those actions (0x00FF7C41, 0x00FF7C97..A0, 0x47: the exe's search finds no reference).
- *Off a ladder*: `xx_h_ladder_wait_<l|r>_tr_rebound_foot<l|r>`, then the same rebound jump facing away, the way by the
  stick (straight back with none, refused into the ladder) (6400374, LADDER-12). Ours flew a fixed arc, 3.5 m/s out.
- Scenarios: `wall A: the legs late in the climb's entry ...`, `... early in the climb's entry ...`, `... in the middle of a
  climbing grid move ...`; the wall-run hang ejects now check the pose's foot and drop.

**In the air, the body's capsule** **[B]**: AC1 drives a jump's path as a velocity through the character proxy
(0xE0DEF0 → the controller), so the body slides along what it meets and a wall holds it back. Ours flew the clips' path
(takeoff, flight) and the falls with two centre rays at the chest and knees: the 550 cm rebound takeoff crossed a 3 m
alley into the opposite wall (reports 1064093, 158904), and a leap off a Damascus window box ended inside a house (the
replay scan, ~326 s). Now the capsule (0.4 m, as on the ground) is swept along every takeoff and flight frame and every
falling step, pushed out of walls, the speed into them gone; a jump held back more than 0.6 m in all falls from there, 30 %
of its speed along kept (Banned445's port's anti-stuck stand-in; 0xE0B1E0's own nudge is ours already). A push leaves
the path where it was: the body steers back onto it once free, in about two frames (`JUMP_LAG_CLOSE`), as the running
game did past the courtyard's palm (pushed 0.21 m aside over four frames, back on its line two later); ours flew on
from where the push left it and caught the far hold 0.24 m along. **[ours, measured]** The hangs' side
leaps first sweep AC1's clearance capsule to the new hang (0xDDDF85: r 0.35, from 0.2625 m back and 0.15 m out, 1.8 / 1.0 /
2.0 m tall to the climb / wall hang / free hang; Banned445's 3580692), as the vertical reach already did.
**[V]** 2026-10-25: in the air AC1's capsule reaches the feet. The jumps' starts (0xB20200, the standing jump 0xB21DA0,
0xE01C00, the climb's release 0xDE97B0) turn the float's byte +0x7F off (0x57AB70(0)); the change (0x579500) moves the
body down by the float along its up and rebuilds the shape (vtable +0x11C, 0x52EB60 → 0x52E9C0) to the full 1.8 m from
the feet. Ours keeps the ground's 0.37 m lift in the air: tried with it at the feet (2026-10-25), ours' push, flat only,
held back a jump off a Damascus roof's lip at its takeoff and slid a fall off rooftop B5's corner where AC1 comes down
on the edge; AC1's proxy resolves a contact along its normal (up a lip, onto an edge), which ours does not yet.
Tried again 2026-10-26 with the capsule from the feet and each wall contact resolved along its own normal (vertical too,
lips under the feet left to the landing): the full suite kept everything but rooftop B5's corner (report 46392). There
the feet are 0.28 m under B5's top as the fall passes its corner, the contact's normal only 0.3 up, so the body is pushed
0.06 m out and the edge leaves ours' "came down on an edge" window; with the 0.37 m lift nothing touches. Any resolution
pushes that contact out, so AC1 reaches that edge some other way (its path higher there, or its shape): needs the fall's
root logged live at B5 before the float can go.

| Commit(s) | What | Here |
|---|---|---|
| 6eec27f | eject during the wall run's grab (Entry state, 0xDCBF00) | ported |
| e6a6eed | the ledge jump off: wall hang only, the rebound turn first | ported (side, drop, foot) |
| 91f7874 | the climb's jump: the rebound first, also mid grid move | ported (the release part: see climbing) |
| 9369c0b, eda4aa7 | side takeoffs, the rebound takeoff (kind 2), its weights per clip | already ours (`jump::rebound`); the free eject's band ported |
| 6400374 | ladder: the jump off is the rebound (kind 2) facing away | ported (QuickDrop's low release was already ours) |
| 38c5bcf, 011aeac | walling: the stale stick angle decides the push; free run up along the last strong stick | push ported; the stale angle and the 0.2 s free-run gate skipped (ours has no stale stick) |
| 3580692 | the side jump's clearance capsule | ported |
| a14bd2c | the wall run's Ground lead-in from the lean / standing free run | ported (`xx_h_impultionstraight_footl_to_wallingfront_step1_footr` before the entry, from the lean on the legs in high profile and from the standing free run; from moving, at once as before) |
| c8e6b69, 61a6d37 | the let-go over a haystack dives into it (vt84's search) | ported (`let_go_dive`, `LET_GO_SEARCH`; unit-tested: no test-world hang over hay) |
| 91f7874 (release) | the climb's release mid grid move is the lost-grip fall | ported (scenario on wall A) |
| 7feeb39 | no empty-hand catch, no edge landing in the leap of faith's dive | ported |
| 5e5f594 | a free hang's row step needs room for the body | tried, reverted: ours' free-hang boxes are not 0xB2DD50's (they refused the Damascus corbels' and awning's steps); to read in the exe |
| 22e8105, 6a84956, 46b292a, 0d08760, 5e83d5a, ebda260, bfba118, 779dfc0, 421521d, 19a7a35, c257aee, ad1ec38, e648e3b, cefccc5, 347c6eb, 9ae3ef1 | the hang's order (pull-up last), blocked look-arounds, shimmy sweep, corners, hang-type switches, let go mid-step, vertical steps, swing-strength idle, ledge to ladder | already ours (the audit batches of 2026-10-10..13: `hang_corner`, `free_to_wall_*`, `free_wait`, the shimmy sweep, `look_around`, `let_go`) |
| 20532a7, 3dc0efd, 01630a1, e822dfd, 39a971c, 1cae28f, f836a51, 7e0a5f5, aae1df1, 3f40643, 51ed60e | the pull-down reception's `_b`/`_tr` played in the hang's Movement state, the free pull-up's waist step on unlock, the hands re-posed from the bones, root paths of table jumps, IK gates | skipped: timing and IK details of Banned445's own sub-state model; ours plays these chains out (the jump off and the shimmy wait for them) |
| eda4aa7 (catches), 9f4f935, fb56f67, fb73f52 | catches reach the hang over item 0 + 0.2 s | skipped: ours spreads the catch's correction over its first item (0.07 s); needs a correction span on the move |
| 3f0c9b9, 47d8143, efe5a2c, e29cbea, 1478b18, 4d4ea57, 44203b4, 0b7b95b, 130bd19, ab1fe10, 731d302, d448693, 1c6cd45, da20f1d, baac142, 7cbae5a, 0c2fd6e | climbing: long table first and the lean guard, climb-out room, ledge grabs, jump up to an overhang, reach timings, entry types, grid root from four holds, the climb-down's landing | already ours where the climbing sweeps put them (sections 4 and "The climbing round two"); the timing and entry-type details skipped; G14 / G24 and the climb-down are fix/climb3's |
| 6668ee0, 58b67ce, 9ac87df, 91985c2, 4c0de51 | haystacks solid, entered on any contact, the hop-out's room, the pass-over from the hay, the low wait | already ours (section 9, user report #1) |
| 6400374, 0a5e5a0, 9548c89, dbd95d9, ff7a8e0, 98e0048, 1f25eb1, a02d462, 87c7141, ecf8bf3, 8c08fbb, b5de470, 29315e6, 3aa5275 | ladders | 6400374 ported; ff7a8e0, a02d462, 87c7141 already ours (the wall run onto a ladder, the walk-in entry, the back turn); the rest's rung alignment and entry frames skipped |
| 1753486, 627324b, 47beb7e, 9571acc, d1ba9d9, aa64eb2, b92172b, 11f9641, 3d217ff, 1ff921d, 6571980, 3ad3bc0, c08e610, 5bc8af0, 2b230ae, a12d41c, 7954a54, cadae0e, b020fdd, bf7789c | air and jumps | 1753486 already ours (fix/landings); the jump aim pull-back, down variants, ladder / wall reception sets, takeoff foot and catches mostly ours from the audit (section 5, 6); the over-drop tail's per-frame integration, the arrival's requested-root test, the apex rules skipped |
| ddb3102, 8926002, f0e6a8c, 9ff950b, bb0fba5, 0c43b23, 7718d53, 7d154ac, e457f11 | lab / city replays, the demo course | skipped: Banned445's own test harness; ours has the scenarios and `replay.rs` |
| beams, posts, swing (98b75f7 .. 5f55770, d5ad375, 142a8f3, fd795de, 00dc174, 5b1b9ff, 90560aa, f73c6a0) | narrow objects and swing bars | the beam agent's (fix/beams) |
| guidance, animation, input, ground (c3b3ac1 .. 34c8049, 8816ad2, cebd9ec, 10253c9, 429bb82, 95217b4, d050de0, 0cf1fbc, 9f1b04b, db55991, 0f94032, bb34a58, e726a01, ...) | the guidance workspace, blend evaluation, input tracker, ground waits | not this round (shared core; each its own batch) |

## Banned445's port, 2026-10-20 round
Banned445's AC1-Movement-Rewritten moved on 80 more commits (5f081fa..c78db3a, his `feature/m3-group9`). Ported the same
way: each rule rewritten here, credited in the comments. His README still calls it early and unfinished; his RE notes
(RE/xx) are not public, so his exe addresses are taken as he gives them. **[B]**

| His commit | What | Here |
|---|---|---|
| afec3cd | the pass-over only from the jump query's mode 2 | ported: ours' own passover fallback only off the test level (`jump_aimed`) |
| 2764d9f | the let-go's hold record: 1.1 m over a wall hang's root, 2.4 a free hang's, 1.2 the climb's | ported (`LET_GO_HOLD_WALL`, `LET_GO_HOLD_CLIMB`) |
| b31c01f, 3c2e51b | the let-go's fall action carries the fall by its root motion, then falls from rest | ported (`fall_by_clip`: `xx_h_hang<wall\|free>_tr_fall_b`) |
| 895fb64 | the other FROMANIM fall actions (ladder releases, rebound, lost grip, vault fall) | the ladder's `_tr_falling` ported (3.5 m down in 0.73 s, ours fell 2.6 under gravity); the rebound keeps ours' live fit; FreeRun's start skipped |
| 48b34bb | the let-go's dive enters the hay as a side jump | ported (`AIR_TO_HAY`, not the leap of faith's landing) |
| c78db3a | the input tracker: the stick snapped ahead within 5 degrees, small angles curved | ported (`pad::tracked`, unit-tested), on all of `move_dir` |
| a9007ff | an exact reversal pivots right; the ledge stop cuts a wait turn; the ladder step seams | ported / already ours (EXTRA-16) / ours by the move graph's blend kinds |
| a061762, 5d1e35f | a wall move ending in a stand takes its wait's profile, then the stance change | ported (the wall-move finish in `character.rs`) |
| b50fe65 | the ledge stop needs the pull-down's room (0xB2E4F0's two boxes) | ported (`pull_down_clear`, city edge reports only) |
| f246687 | the ladder step's rate per frame, half under a 0.5 stick in low profile | ported (`ladder_rate`); the ladder rebound scenario's foot loosened (scripts climbed at half rate before) |
| ca0e1ad | C centres the camera eased, 3.5 pi / the larger angle per second | ported (`OrbitCam::centring`) |
| 4c9b126 | a swing started again from a still hang plays the momentum build-up first | ported (`SWING_MOMENTUM`) |
| 9390e4e, afa4e0f | the wall hang's body axis from the foot records: the root 1.1 m down it and 0.5 m out square to it; the body box on that pose | ported for the hang at rest (`wall_hang_lean`, the body tilted by `tilt`, kept along its shimmies; eased out and up to it, never along the wall: on a sloping hold the root followed the nearest point 0.77 m down a Damascus gable at rest, sweep `w15-run`, and routes `wall1_hang_right` / `street_free_west` ended 0.10 / 0.14 m off for it, now 0.00 / 0.02); where the leaned box meets the wall ours stays upright (gauntlet G24's upper row); his entries' support boxes and step roots not yet |
| 73c8329, 693de57, a49244e, 3c2e51b (A01), 4257ced, 922c701 | shimmy across a bend, the wall-free hang, its switch clip, wall run from the lean, beam step-off, the free-step stay | already ours |
| 6f0bfae, 9199a0e | the free jump falls from rest at its target | ours' fit to the running game's seven free jumps in the traced routes (2026-10-27, below): the flight leaves the takeoff at its top (0.3 m/s up), 13 m/s² down to 3.3 m/s, then 30 to 8.5 m/s, held; no fall from rest seen |
| 2764d9f, ad6f172, e0cf444 | the long free-hang catch to the side, one hand, then the second hand's reach | open: his notes and constants disagree on which clip is which side, the side comes from a stale record, and ours has no second-hand reach |
| b284b1e | the wall run's width check off a jut | skipped: ours' `wall_spans` is fitted to live runs (7813d93) |
| 5d5b570 | the free-step stay's wall run from half of its reception (0xE503D0's first test) | ported (ours waited for the stay to end); the take-off foot from the pose skipped |
| 400b4f4 | the drop past an edge measured from the actor's feet first: the lip ray, the line from 1 m over the feet | ported (`drop_at_lip` in `drop_edge_ahead`) |
| 9f9e809 | the grasp: the falling blend on the upper body whenever the manual catch is armed, its reach eased at 7/s | ported (`Grasp`, `grasp_pose`, layered over the spine; unit-tested weights); ours had no grasp at all |
| camera.rs (0x691890) | the follow turns after the average displacement's direction, the facing only standing still | ported for the ground (the velocity's way); the run3 pad scenario now starts on the heading the running game had (`bureau_corner.pad`: the old hook's teleport set only the shown yaw) |
| d7a57fa, e78cc96, 84c0fd9, 15e7189, 4124a98 | the knee stand-up's root, the running jump's wall reception, the ladder back-approach turn, the reach's root on a leaning wall, the switches' lateral path | already ours (the stand 0.3 m in from the hands, live-fitted; the reception ends in the wall hang) or not in ours' structure (no back-approach turn, his own climb reach and path boxes) |
| 2c5fdd5 | haystacks from the entity's origin and collision box | not yet: city data (a cache version and a hay sweep of its own) |
| b87d783 | the over-drop per jump kind | skipped: ours has no per-kind over-drop |
| 9a7cbf0, 94fa975, ba2f3f8, 380bc88, e7e811e, fd7987d, aee9e57, f149e21 | ledge-to-climb body boxes, the side move back onto a ladder, swing landing weights, the animation runtime's slot rules, the ledge's hand IK, first person, hand records, switch-frame clocks | not this round |

## The gauntlet's G14 and G24, user reports 2026-10-14
- **Through G14's block** (289508, 440360: a wall run; 449200: hung under it, up): AC1's walling FindLedge (0xE36BD0) keeps
  an edge only with a clear line to it from 0.5 m back out of the walling root (Banned445's port); the block hides the
  holds over it and the run catches the row under it. Ours took the highest row in reach through the block. And AC1 tests
  its body boxes at each hang or stance a move goes into, read in Ghidra: the climb from a hang (0xDD46A0 / 0xDD48B0 ->
  `Human__ClimbBodyBoxesFree` 0xB2DF90), the switch onto the wall and the climbing stance's ledge grab (0xDE1060 / 0xDF0980
  -> 0xB2DC80, the wall hang's box: 0.75 x 0.5 x 1.3 m centred 0.65 m over its root, 1.1 m under the hands and 0.5 m out),
  the free hang's hand step on its vertical rows (0xDD8F70 -> 0xB2DD50). Ours now does; ours' own grid hand steps in a free
  hang want the hands' way from the edge held to the new one clear [ours].
- **The hang type flip-flop** (694451): AC1's shimmy (StartHandStep 0xDDE0C0) keeps the hang's type; only TrySwitchHangType
  (0xDE1060, tried first) changes it, and it returns at once on a closing step (the ledge's alternation flag +0x85, the
  hands apart). Ours turned the wall hang's shimmy down where no foot found the wall within 0.32 m of the hold (the
  gauntlet's wall stepped back 0.56 m) and leapt down into a free hang and back up, step by step.
- **The wall-free crouch after the swing back** (608183): AC1's entry end 0xDE1FE0, under the wait kind
  3 weighs the playing item, the connector `xx_h_hangfree_swingback_<min|max>`, [1 - h, h] (0x502E30) and the free wait
  behind it [1, 0] (0x503040, the requested action's item): `xx_h_hangfree_wait` whole. Ours had it the other way round,
  the wait half `xx_h_hangwallfree_wait` (its hands 0.46 m in front of the root, 0.35 m into G24's slab) after every
  shimmy step. Now the connector is mixed by h [ours: its second item too], the wait is the free one, until the next move.

## The inverted triangles played by hand, 2026-10-20
The same spots played as a player plays them (hand-written pad files through `AC1_PADPLAY`, the player's own input),
both hands' world positions checked every frame against the holds (a flight-recorder run; a "glide" is the whole body
sliding as a block, every bone the same way, which no clip does).
- **The climb up and the glide back**: the free hang's hand steps (AC1's vertical rows) ended the root 2.1 m under the
  new edge, the catch's height, where the free hang rests 2.4 m under it (`FREE_ROOT_DOWN`, AC1's 0xDD6730). Each step
  ended with the hands 0.17-0.2 m over the edge, then the rest slid the whole body 0.3 m down onto it (the awning frame's
  beams: 0.29 m in 0.5 s after each step). The steps now end on the rest, the hands within 2-5 cm of each beam, and the
  rows are measured from the hands (AC1's origin; ours had measured from 0.3 m under them at rest).
- **The empty hand pressed again in the fall** (as a player catches, where the scripts only held it) took only holds
  0.45 m under the one let go of, taking it as a wall hang's (1.13 m over the root). Off the pavilion's roof slab's free
  hang (2.4 m) that turned down the knob and the arch under the slab, and he fell 4 m to the floor. The let-go now keeps
  its hold's height (`let_go_hold`): he catches the knob, then hand-steps up onto the slab and over.

## The inverted triangles, user reports 2026-10-15
A Damascus pavilion (-45.3, 45.4: arches 12.15 m up, a knob over each, the roof slab's edge 0.58 m out over them) and an
awning's frame (24.5 to 27.3, -11 to -12.6). Each report is a Damascus scenario (`AC1_SCENARIO_FILTER=<report number>`).
- **152477, up and down for ever**: the free hang's grid hand step onto an edge 0.45 m further out kept the body's depth,
  the hands in the slab under the edge, and the rest then eased the root back under the knob. Now the hands go onto the
  edge (within AC1's vertical rows' 0.5 m, `FindVerticalRows` 0xDD8F70) and the body out under it (as AC1's root follows
  its hand targets, 0xB15AD0; never in toward a wall under a recessed edge: that is the switch onto the wall's); then over
  the slab. And the free hang's rest finds its hold once, as it begins (`rest_hold`): probed from the sinking root, an
  arch 0.3 m under a knob came nearer halfway down and the root sank onto it, and the knob was stepped onto again.
- **161446, stuck in the wall hang under a knob**: AC1's hand step keeps the hang's type (StartHandStep 0xDDE0C0; only
  TrySwitchHangType changes it, its foot rays finding the arch there). Ours wanted each foot on a wall within 0.32 m of
  the new hold and refused the slab's edge 0.6 m out; now the wall hang's hand steps take any footing.
- **116822, held in a half hand step**: the first half of a wall-hang hand step was tested at its own holds (one hand up,
  one down) and passed under the awning's beam; the second, both hands up, never did. Now the first half tests the row
  both hands end on, as AC1's FindVerticalRows tests the step row before starting (0xDD8F70 -> 0xB2DC80): no step there.
- **265066, through the roof**: the let-go was the empty hand (ours lets go of a hang only on it). The release clip
  carries the root 1 m down with no floor test; under the chimney's top the roof was 0.35 m under the root. A floor within
  the release's drop: the fall from where he hangs (`drop_now`), which lands on it.
- **102743, the wall hang at the awning's end looking up**: AC1's own refusal (not live-checked). Ours' top-out wanted a top under 37 degrees and within
  0.35 m over the grip at 0.45 m in; AC1's CanPullup (0xDE2270) tests the room only, so the slope test is gone and the
  nearer probes are taken on a top rising from the edge. What is left is AC1's room box over the edge (0xB2E240), which
  meets the planks' slope. AC1's room test there is three boxes through the character's collision query, each
  passing only when empty, with no layer mask (the last argument, 0x1934100, is a debug colour); its middle box, 0.2 x
  0.2 m from 0.1 to 1.8 m over the edge, takes in the planks rising about 47 degrees from the edge (0.106 m higher 0.1 m
  in) by about 6 mm, and AC1's shapes are a little fatter than ours: AC1 refuses the pull-up here too. Along the edge,
  from the free hang, he pulls up. A live check would settle it for certain (the running game was not up).

## Live comparisons, 2026-10-23 (Damascus bureau courtyard, `tools/live-compare.sh`)

Played into both games at once (the hook's background mode: AC1 needs no focus). The real camera is turned behind him
with the hook's `look` (+1 mouse unit = -0.00212 rad of yaw); the pad's right stick does not turn the PC game's camera.
- **Shallow ledge with a wall just behind** (the bureau's west wall, 8.02 m): AC1 pulls up (`hangwall_tr_hangknee_footl_a`,
  `hangknee_footl_tr_freestep_entry_footl`, a moment in HumanNarrowObject, then Ground); ours hung on, looking up. Fixed:
  `try_top_out` stands him at the first floor inward of the edge with room for the body (pad `bureau_ledge_climb.pad`).
- **Limb IK at the eject:** AC1 drops every limb goal from `hangwall_tr_rebound` on; ours held the hands on the wall
  through the pose and into the takeoff. Fixed in `hands_on_wall`.
- **Eject flights by target type:** AC1's eject at a wall hang hold (target 0x40: a wall within 0.7 m under the hold)
  flies the surface flight, action 0x011E555B (`air_*_<foot>_to_surface`), into `air_surface_tr_hangwall_reception_*`;
  only a free hang (0x80) flies `_to_swing`. Ours flies `_to_swing` to every one (the "hands down / about to grab a bar"
  look). AC1 also took a hold out of the landing zone 1 but in the hanging zone (4.83 m on, 1.28 m up). Ours now does
  both (`wall_hang_eject`, `EJECT_SURFACE_WALL`; `jump_hold`'s far holds, taken only with no hold in its own reach).
  AC1 has no limb IK goals over the surface flight (they start at the reception), so `catching()` keeps to `_to_swing`.
- **The catch onto AC1's grab point (fixed the same evening):** off the wall run AC1 caught at z -20.01 and slid 0.4 m
  along (hand goals at -20.406): not a grid hold but `FindLedge`'s grab point (0xE36BD0). Its probe boxes from 0.6 m
  back out of the root (B: the free hang off the entry, C: off the step, D: the wall hang; up / width / height / depth /
  radius as `WALL_PROBE_*`), the edges cut to the box and by the depth detectors (`guide_depth::walling`), chained
  (0.01 m), the point by `FindGrabPoint` (0x66E150: hand offsets 0.5 / 0.1, margin 0.3, the room past a piece's ends
  by 0x66FFD0: its free samples from the one at its start down, from the one after its end up, so a piece cut by
  something in front keeps 0.1 m at its start, none at its end), then for D the hands' own pair search (0xE38C4A: a
  box 1 m either side at the point, each hand 0.3 m to its side and 0.15 m back, pieces shortened 0.4 m). Here a
  lantern's bracket 0.6 m along cuts the edge, and the point lands on -20.400. Ours took the nearest point to where
  the clip's hands are (`wall_run_grab`, `wall_probe_grabs`, `chain_grab`, `wall_hang_hands` in `climb.rs`).
- **The hang's move sideways onto climbing holds** (`hangwall_r_climb_1m`, 0.75 m in the clip) went 1.0 m in AC1: its
  hands onto the hold row's point nearest the far end of `ProbeLateral`'s reach (0xDD9640: the room swept from the
  lead hand 0.15 m back, 0.75 m out, 0.2 m over to the hang's depth under it, at most 1.55 m less 0.15, less 0.4; at
  most 1 m less the hands' spacing; the box from 0.1 m past the lead hand, the band's row 0.6 m up, level or down).
  Ours took the holds where the clip's hands land (`side_reach`).
- **The eject's way is the camera's stick:** AC1's camera turned 4.3 degrees by itself over the hang and the climb
  (ours does not: an open camera item), so the same stick went 4.6 degrees off straight out. `AC1_PADPLAY_CAMLOG`
  plays the real camera's yaw into ours for comparisons (`tests/pads/courtyard_eject.cam`). At that angle the target
  is AC1's grab point where the forward line crosses the far wall's hold (z -22.09, AC1 -22.11), not the point
  nearest 2.5 m on (`jump_hold_in`, far holds only), and the line from the chest misses a palm (`Veg_PalmTree_Broken`,
  layer 33, a fuzzy zone: it stops the body and the rays) that blocked the straight one.
- **What turned AC1's camera on the wall** (2026-10-24, from the same run's log): not its follow. The script had turned
  the camera to look along the wall from Altaïr's side (90 degrees off behind him, past the lazy follow's 1 rad band), and
  he moved along the view line. The 5 degrees came in the walling's step to the hang (1.43-1.94 s, 3 of them while the
  root stood still) and eased back 0.9 over the climb's wait. On the climb AC1's camera has its own mode (Banned445's
  camera.rs, 0xC5E370 / 0xC5EE20: no lazy follow, farther out (4.25 / 3.75 m), the stick held sideways past 0.75 turning it
  0.5 rad off behind, the top of the wall bringing it behind). Ours has that mode now (`ClimbCam`), apart from its pitch. Before it, ours took: the
  eye's collision (`CAMERA_SPHERE`, 0x692930). At the eject AC1's eye stopped on the courtyard wall's face (x 44.0)
  where ours' does now, and through the palm (a fuzzy zone) it stayed out, so ours' cast passes fuzzy zones.
- **The flight pushed off its way:** the palm's collision still brushes the flight (0.29 m out from the hold's wall);
  AC1's catch takes the hold from where the body arrives, so the reception makes the push up out from the wall
  (`catch_reaim`) and the hang sits 0.44 m out as AC1's.
- **The reception's last clip is a connector:** action 0x011FF16A is `air_surface_tr_hangwall_reception_*`, `_a`, `_b`;
  `_c` (0x8CFB3CBC) is the way out to the wait. With the stick held AC1 strafed 0.68 s into the reception; ours played
  the connector through first. A move now cuts it (`reception_connector`); with no move it plays (the hard
  reception's `_b` ends swung out).
- **The hang's own root** (2026-10-24): the wall run's catch into the wall hang and each wall-hang shimmy step end on
  the hang's root from the hand goals and the feet's supports (`ComputeRootFromHandTargets` 0xDD6730, ours'
  `wall_hang_rest`), not where the clip leaves the hands on the holds: AC1's catch ended 0.42 m out from the hold, ours
  0.58 m; along the far wall AC1's root kept 0.44 m out, ours swung 0.08 m further out and back.
- **The shimmy's widths** (`ProbeLateral` 0xDD9640, Banned445's port, **[V]** 2026-10-24: the capsule r 0.15 from the hand
  0.15 m back, 0.75 m out and 0.2 m up, down 1.1 / 2.4 m, swept 1.55 m; blocked under 0.7 m): the lead hand reaches the sweep's free distance
  less 0.4 m, the hands at most 1.0 m apart, and the trailing hand closes to 0.15 m of it; the root follows their middle
  (`shimmy_free`, `WallClimb::shimmy_hands`). On an open hold that is 0.425 m a step (ours' old fixed 0.43); toward the
  far wall's inside corner AC1's lead reached 0.73 m and its last two steps were 0.36 m, which ours now matches (the
  replay's end 0.12 m off, now 0.01 m; mean 0.07 m). Left: ours' takeoff drifts 0.06 m sideways before the flight.
  Scenario "the courtyard wall run, its catch at AC1's grab point, the climb right and the eject across".
- **The drop's lip rays** (`drop_at_lip`) count a hit only short of their ends, as Havok's casts keep one under the
  fraction 1: ending on the plane of a top that goes on flush past the edge, rounding decided the drop (the test
  level's doorway lintel: the free jump off its stand-up came and went with the flight before it).
- The wall run catch's `251cm` name in AC1's events is the first clip of the blended action 0x0106CFC5, not a different
  move.

## How far ours follows AC1's tables (2026-10-08)
Every movement context picks its moves by AC1's own data, where the game has a table or an action id for it: the
climbing stance (short / long tables, 10-way stick, redirects), the hangs (ledge-jump table, the sideways order), the
ladder (`HumanLadderData`), the ground (the 17-clip move blend, the starts from standing, run stops, pivots and turns by
their action items, landings by AC1's stick / speed rule), beams and posts (starts, jog stop, turns, side entry,
impulse jumps), jumps (takeoff / flight / reception tables, the edge classification of 0xE18970) and the edge events (as
the input interpreter sends them). Checked by mapping every action id Banned445's port lists for each context to its
clips (`forge` example `action_clips`) against the clips our code plays: left unplayed are combat's (hurt
falls, stumbles), other characters' (`cmaa_*`), and in `HumanLedge` (audit LEDGE-15, 2026-10-12, see the audit section's "the
hangs AC1 has"; the free to wall switch up or down and the wall run's free catches' `max` share and wait were added
2026-10-13): the clips AC1's code never asks for (`hangwall_tr_hangfree` in place, `_up_right`, `_down_right`,
`hangwallfree_tr_fall`, `step1_footr_tr_hangwallfree_*`; `hangwallfree_tr_hangwall` only for a hang type set with walling
actions no data file has; `hangfree_strafe_*_000cm_*` are weighed 0).

## Open questions
