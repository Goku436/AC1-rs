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
- An item's duration is the weighted sum of its clips' lengths (0x507650 → 0x5B9480). **[B]**
- Root motion: a clip's root track moves the character. Discrete moves (climb, ledge, hay entry) instead interpolate
  the root linearly from where it is to a computed target over the clip's length (`sub_711130`). Jumps add a linear
  correction to the clip's own displacement (section 5). **[B]**

## 3. Ground (`HumanGround`)
- **Speed**: one parameter 0..1, bands Walk ≤ 0.25 < Jog ≤ 0.5 < Run ≤ 0.75 < Sprint (`GetSpeedBand` 0xD807B0).
  Target = base + 0.25·stick, base 0 low profile / 0.5 high / 0.75 sprint (free run). It rises at 1.0/s and falls
  along a response curve. A start from standing sets it to 0.25 (walk, low) or 0.5 (jog, high) at once
  (`HumanGround__PlayStartMove` 0xD98990). **[B, ours: `gait.rs`]**
- **Turn attenuation** in high profile: beyond 45° off the facing the speed scales down to 0 at 90°, floor 0.1,
  rising back at 5/s and falling at 10/s. **[B]**
- **Heading** turns toward the stick at a capped rate and the character moves along the heading, not the stick: 4.0
  rad/s (2.6 free running), eased down to 1.5 / 0.975 within the last 60 to 10° (the interpreter's RotateTowards,
  0xEE6D3F); a turn on the spot only past 90°. The running game turned 3.98 and 2.58 rad/s, its motion within 1-5° of
  its facing. **[B, measured; ours since 2026-10-10]**
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
- **Fall type** by height (1 / 2 / 8 m) and a horizontal speed of 2.5 m/s (0xD8C380). **[B]**
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
  only with the stick held (AC1 by the stick's speed): let go, it lands and stands. **[B, ours, measured]**
- **Jump targets at roof edges** are where the way forward crosses the edge, as for beams (AC1's candidates): the edge's
  point nearest a spot 4 m ahead was its end when the way crossed the next piece, and the jump went 0.7 m off the way; a
  beam's own side edges are no roof edges. **[ours, measured]**
- **Grabbing a wall** (the empty hand at it, event 50; the legs only run up it, from the run), in order: a climb start (hand holds 1.8-2.4 m up and
  foot holds 1.2 m below them) → `xx_h_wait_hipm_foot?_tr_climbing_1m` with the root interpolated onto the holds;
  else a ledge 0.53-3.0 m up → the standing straight jump at it (0xB21DA0: knee and waist heights pull up onto the
  top, higher ones end hanging). The grab probe radius is 0.75 m (IHuman vt136). **[B]**
- **Character proxy**: capsule radius 0.4 (0.35 + 0.05 keep distance), height 1.8, floating 0.37 m over the feet
  (step), stick-to-ground cast 0.58 m (0x57D240). **[B, ours]**

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
    than 0.25 s (a timer, ticks / 30000), and the playing item allowing it (bit 29 of the item's 64-bit move mask).
  - **69, ledge stop**: an edge report, queried within 0.15 m (0.0225 squared), sent only when it is ahead (direction
    1) and the drop over 5 m.
  - **68**, when no jump was just started (+0x112F): an edge report → sent at once (handled by a sub-state; with the
    +0x112B flag set it is sent with 1.0).
  - **70, pull-down**: an edge report and the playing item allowing it (bit 4 of the move mask), then either the legs
    pressed within the last 0.2 s (button 3, the empty hand, 0xED4A20) or the stick within 70° (1.2217 rad) of the edge;
    otherwise the interpreter only
    marks the frame (+4 = 0x1D).
  - The **jump** asks the playing item too (bit 0 of the mask) before `JumpToGuidanceTarget`. The mask is checked only
    when the item has its flag 0x14 & 2 (0xEF0320), against the current item and the one blending out.
  Events 68 and 119 are not in `HumanGround`'s handler: its sub-states take them.
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
  apart with the left / right higher. Each pose has its wait action (0x012DA39F + pose). **[B]**
- The stick is quantised into 10 directions (`QuantizeStickDirection` 0xDEB8D0: up and down split by which side
  of straight they fall, the diagonals at 22.5-67.5° and 112.5-157.5°, sideways 67.5-112.5°). **[B]**
- A move is looked up by (pose, direction) in the **short** table (0x1A2D070), or first in the **long** one
  (0x1A2D7F0: hand over hand 1.2 m, both sides shuffling a column) when the stick is past 0.5 (`ChooseMove`
  0xDFDE90). An entry is (next pose, which side moves, columns, rows) or a redirect to another direction; each has its
  action (`xx_l_climb_*`). **[B]**
- A move is valid when the destination foot cell and the hand cell two rows above both hold (`IsGridMoveValid`
  0xDECD70). It plays its action while the root is interpolated over the action's length (0.5 s without one) to
  the **root computed from the four holds** (`sub_B1CC20`: 0.5 m out from the feet holds along the climb facing, the
  frame tilting with a leaning wall) (`StartMove` 0xDFA0C0). **[B]**
- No move found with the stick held → the look-around toward it (`OnNoMoveFound` 0xDF4410:
  `xx_l_climb_1m_lookaround_<up|down|left|right>`). **[B, ours]**
- No move up → into a ledge hang (`TryTransitionToLedgeHang` 0xDF4BA0); legs → release; reaches onto other climb
  holds or a ladder across a gap (0xDF0050, 0xDEF3A0); corner climbs (0xDF29E0); the jump up from the climb
  (`TryBackEject` 0xDF2F50: search 2.25 m over the root, 0.8 m out). **[B]**
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
- Lost grip (a hold's object gone, 0xE55A10) → InAir with action 0x1120E17B, blend 0.2. **[B]**

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
- **Bands** (0xB1EC40): ground-type targets up to 1.3 m up, near 2.5 / mid 5 / far 7 m, down to −3 m (deep variants
  past 3 m down); ledge-type targets up to 3.0 m up, bands 2.5 / 6 / 8 m. **[B, ours]**
- **Over-drop**: a target more than 5 m below (3 m for the leap of faith) is aimed at 5 m down, then a free fall
  (g = 9.8, steering ≤ 15 m/s). Plain falls drift (decays 4 m/s², ≤ 5 m/s). **[B]**
- **Arrival** (0xE07D00): free step → the reception `<flight>_tr_freestep_entry_foot?[_fast]` (0x010DE1FE /
  0x010DF150), into the run, walk, wait or the beam; wall hang → `xx_h_air_surface_tr_hangwall_reception_*` (action 0x011FF16A: **[V]** 0xE02790 weights its six clips by the wall's lean under the edge, measured about the body's side: leaning out → `straight` to `30_out` (full at 30°), sloping in → `straight` to `45_in` (full at 45°); the `max` set for a jump over 6 m or a second measure over 2.5, else `min`; the clips carry no yaw); free
  hang → the swing reception; post / beam → their mounts (0xE50190, 0xE52AD0). **[B, ours]**
- **Ground landing** (`HumanInAir__SetupToGround_Landing` 0xE05940): over 3 m of drop from the jump or fall's start
  the damage landing (`xx_h_landing_damage_footl`, or `_roll` with the stick held) with a camera shake of (drop − 3)/7.
  Damage: the player is the flagged mode (+0xE2 & 8): a fixed table, over 8 m 10, 10 m 20, 12 m 40, 14 m 80, 16 m 120,
  18 m 160, fatal over 20 m (of 200); the 6.3 / 7.0 m limits (0xE00FE0) are the non-player default (audit AIR-1, AIR-18).
  Up to 3 m: forward when the impact velocity comes in within 75° of level, else straight (not by the stick, AIR-15)
  (`landing_forward_{soft,hard}`, by the speed bucket [< 0.2, < 0.5, < 0.9, else] into walk, jog or sprint impulsion,
  or into the wait when slow). **[B, ours]**
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
  `WorldArea::JumpLinkRange_Normal / _Extended`). **[V: the names are in the exe]**
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
  **[B]**
- Moves are discrete (new hand targets, the root interpolated over the clip): shimmy with alternating hands (free
  space swept with a 0.15 m sphere over 1.55 m; step = min(d − 0.4, 1.0 − hand spacing), blocked under 0.7 m, at
  least 0.15 m); up/down: into the climb when foot holds 1.2 m below exist, else a hand step to a ledge 0.6-1.2 m
  away, else the swing back up to a ledge out from the wall (0xDD62A0, `hangwall_to_swingback_up_*`, ending in a free
  hang), else the pull-up if there is room (0xDE2270); corners
  (0xDD3BB0), side jumps between ledges (0xDDD490); lost ledge (0xDD20D0) → InAir. **[B]**
- Hang → climb: `xx_h_hang{wall,free}_{u,d}_climb_1m` and the sideways `{ul,l,dl,ur,r,dr}` variants (0xDD46A0,
  0xDD48B0). **[B]**

## 8. Beams and posts (`HumanNarrowObject`)
- From the ground on event 72; from the air a free-step jump mounts a beam (`TryMountBeam` 0xE52AD0) or a post
  (`TryPilotisFreeStep` 0xE50190); a fall onto one is caught. **[B]**
- On a beam the beam actions' root motion is projected onto the beam line (`ConstrainRootMotionToBeam` 0xF7C3A0):
  the walk **stops 0.3 m before an end**, and steps off an end with free space beyond (within 0.16 m of the end, a
  free capsule at end + 0.5·dir, `CanStepOffBeamEnd` 0xF77C00). Stick vs facing: within 75° forward, past 135° back
  (turn). **[B]**
- Sub-states: idle, walk, stop, the 90° wait (facing across), turn 180, the turns to and from the 90° wait; exits:
  the pull-down to a hang (event 9, the empty hand), the wall run (event 15, walking), the corner hop onto a beam
  beside (event 7). The edge stop (`xx_l_beam_edge_stop`) and NarrowObject → ladder are not used by v1.02. **[B]**
  (Ours stopped playing the edge stop: AC1 just stops 0.3 m short.) Starting along a beam from the crouch plays AC1's
  start (`crouchwait_<foot>_tr_crouch<walk|jog>_<other>`, 0x34662CB4/B5) and letting go while jogging its stop
  (`crouchjog_stop_<foot>` + `_tr_crouchwait_<foot>`, 0x3466339D/9E). **[ours]**
- **Side entry** (0xF7AAA0, actions 0xE6E0E9B8..BB): met across a beam (30-100° off the way the stick points along it)
  → `xx_h_freestep_entry_<foot>_tr_crouch<walk|jog>_<foot after>_<left|right>_<30|90>`, the 30° and 90° clips blended
  by the angle; a left turn goes on on the right foot, a right turn on the left. **[B, ours: not yet seen in play]**
- Where beams come from: `GuidanceBeamDetectorAccurate` (vtable 0x0169A1CC, constructor 0x678F80) is never loaded
  from data (no objects of the class in any forge); it is built on the stack with fixed values (1.0, 0.15, 5° =
  0.0873 rad, 0.5, 0.2) by 0xC7E810, which runs lazily when a query box overlaps a guidance object's bounds
  (0xC7EAA0, AABB test 0x61E880) and caches the result. It does not search city geometry: 0x680260 cuts the guidance
  object's own shape (object +0x170) into beam segments (start, end, half widths, the two side normals), and
  0x67DA30 builds each beam's volumes: the top and both sides, tilted ±5° about the beam's line, 0.15 m past its
  half width, 0.5 m along the up normal. So city beams are the authored guidance beams, which we already load (937 in
  Damascus → 1141 perches). **[V]**

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
  Swing bars.) Ours: a thin beam over open space (two holds facing apart at most 0.35 m across, nothing
  within 2.5 m under, no wall under) is also a swing bar (Damascus 1341); a free-step jump onto a beam ends on it,
  its reception cut once its step is taken; from a post or beam with no top in reach, a bar ahead down to 0.5 m under
  the feet is jumped at with the free-step takeoff and the `_to_swing` flight.
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
- **Swing bars** **[V, ours]**: AC1 swings on while high profile and the legs are held, and stops when they are let
  go (the interpreter's ledge events 5 / 6, Banned445's port and reanalysis), not by the stick. Caught with them let go
  (route ladder68_free), it hung still from the bar 0.25 s into the first upswing, no stop chain; ours settles into
  `xx_h_hangfree_waitclose` after that upswing. Let go mid-swing: at the end of the down item (event 6's guard 0xDCCF40), AC1's stop (`xx_h_swing_stop_<front|back>_a..d`, the
  last items of action 0x023E0C61), ending 1° from `xx_h_hangfree_wait`. Which stop is front and which back is to be
  checked live (the reanalysis says the port's are swapped). The stick forward swings him up again
  (`xx_h_swing_momentum_front_up`). The hay's hop out
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
- **Hay** (context 21): the faith landing (0x23A9666C, blend 0.3 s) or `xx_h_air_to_haystack` (0.1 s), the root
  moved to the hay over clamp(d/speed, 0.1, 0.4) s; hop out on the legs if the exit point (+0.5 m out, 1.25 m up)
  has room. **[B]**
- **Swing bars**: a free hang reached by a jump (target type 0x80) enters the swing reception (Ledge sub-state 8).
  **[B]**

## 10. Limb IK
- Limb weights rise at 4/s with a contact (0.25 s fade in) and fall at 5/s when released (0.2 s) (`LimbIK` 0xE57570); ours reads the contacts from the clips' hash 2 track (`forge::anim::contact`, `WallClimb::contacts`).
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
  before hopping out; the wall hang's shimmy steps 0.23 m then 0.43 m; letting go cuts a shimmy at once.
  `AC1_PAIRED_BEAMS=1` builds beams and posts as AC1's runtime does, pairing facing hold edges (`beam_pairs.rs`, after
  Banned445's decode) in place of ours' mesh-name beams and single-hold tops; not the default yet (31 of 41 routes
  against 34: the jump query and beam walk were tuned on ours' beams, and the workspace's side planes cut a roof-edge
  beam the running game lands on).
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
  down_<other>`, actions 0x04A943DE / 0x040C793D, played over 0.1 s) and the free-step takeoff, not the running one. The
  gate is bracketed by the routes (0.30-0.32 s after a reception did, 0.59 s into a run did not), not read from the code;
  only the front, `_down` set is used (every recorded one, a jump 1.17 m up included).
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
  0x0A64400C/D, when the attach box is blocked) and the leaning ladder's forced revolve. (Fixed after: onto a ladder
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
    the counter and `_tr_fall` off its end; along it, the side free step (ended where it leaves the counter) and a
    free-step jump on out. [ours] The stall's own collision is left out of the kiosk's room and clear-way tests and the
    way over: in Damascus it stands in the way (ours places it about 0.9 m out in front of the counter, a placement still
    to check) where AC1 comes onto the counter. The ground-entered overhead monkey bars are gone.
  - NARROW-11: on a beam, the empty hand with nothing to pull down onto climbs onto a wall ahead, else to its right or
    left (event 16 after the pull-down, holds probed 0.95 m ahead or 0.8 m to a side; ours: the ground's climb start
    turned that way). Gauntlet G17.
  - NARROW-14: the swing's stop (the legs let go) starts at the end of a down item (event 6's guard 0xDCCF40), the front
    stop after front_down and the back after back_down (Banned445's mapping; the reanalysis says swapped: a live check).
  - EXTRA-16: the ledge stop breaks into a standing turn toward the edge in high profile (as the running game's 0.084 s
    into `waitturn_left_090`).
  - HAY-7: out of a haystack over its rim, the side and the drop (1.5 m under the haystack's floor) come from its own box,
    the hang from a level hold along the rim if there is one, else from the rim itself.
  - HAY-9: the leap of faith's search is AC1's 0xE15AA0 with vt76's limits: the top (floor + 1.6 m) under 1.2 m over the
    feet; across 2 to 8 m, along 9 to 20 m and 40 to 70 degrees off the way from 1.5 m behind the feet, opened as the
    top lies lower (full at 30 m down); in front of the facing; the two-segment clear way (bent at 0.66). [ours] The 3 m
    split between the faith dive and the hay dive stays, and a viewpoint's hay is still taken whichever way he faces.
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
    - LADDER-7 (the rest): Damascus' 333 authored ladder edges lean less than 10 degrees (278 under 5), so the forced
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

## How far ours follows AC1's tables (2026-10-08)
Every movement context picks its moves by AC1's own data, where the game has a table or an action id for it: the
climbing stance (short / long tables, 10-way stick, redirects), the hangs (ledge-jump table, the sideways order), the
ladder (`HumanLadderData`), the ground (the 17-clip move blend, the starts from standing, run stops, pivots and turns by
their action items, landings by AC1's stick / speed rule), beams and posts (starts, jog stop, turns, side entry,
impulse jumps), jumps (takeoff / flight / reception tables, the edge classification of 0xE18970) and the edge events (as
the input interpreter sends them). Checked by mapping every action id Banned445's port lists for each context to its
clips (`forge` example `action_clips`) against the clips our code plays: left unplayed are combat's (hurt
falls, stumbles), other characters' (`cmaa_*`), the leap of faith's code-driven fall (`xx_h_faith_jump_fall`; ours
stretches the dive over the whole fall), and in `HumanLedge` (audit LEDGE-15, 2026-10-12, see the audit section's "the
hangs AC1 has"; the free to wall switch up or down and the wall run's free catches' `max` share and wait were added
2026-10-13): the clips AC1's code never asks for (`hangwall_tr_hangfree` in place, `_up_right`, `_down_right`,
`hangwallfree_tr_fall`, `step1_footr_tr_hangwallfree_*`; `hangwallfree_tr_hangwall` only for a hang type set with walling
actions no data file has; `hangfree_strafe_*_000cm_*` are weighed 0).

## Open questions
- Whether the player's jumps ever use the navigation meshes' jump links (the candidates come from the guidance).
- The move mask the interpreter checks (64 bits at the playing item's +8: jump bit 0, pull-down bit 4, look-down bit 29)
  is not the 12-byte flag word we read; where it comes from in the action data.
- Event 68's handler (a ground sub-state's) and event 119's guards in the game's own code (Banned445's values used).
