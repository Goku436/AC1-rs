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
- **Heading** turns toward the stick at 360°/s (`HumanGround__UpdateHeading` 0xD95290). **[B]** (ours: 9 rad/s run
  turn; to play-test.)
- **Locomotion** is one action (0x05923BDB), an item per leading foot, each blending 17 clips by speed, hip lean
  (crowd avoidance, walk band) and bank (heading change, jog and up), with a jog slow-down timer and a sprint settle
  weight (`HumanGround__UpdateMoveBlend` 0xDA0810). The character moves by the blend's root motion, not by a speed.
  **[B]** Ours since 2026-10-08 (`move_blend.rs`): the 17 weights by AC1's rules (four bands; the walk band's hip lean,
  none here as there is no crowd avoidance, so walks never lean; the bank from the jog up; the jog's slow-down timer
  shared with the jog's weight; the sprint's take-off weight), the clips on one clock whose step lasts Σw·T, and the
  ground speed is the blend's own, Σw·d / Σw·T (1.90 walk, 3.54 jog, 5.12 run, 6.28 sprint, slower between bands than
  a straight mix). No pose pops over curving walks, runs and free runs (`pops` example).
- **Run stop**: `xx_h_{jog,run,sprint}stop_foot?`, weights [jog 0, run 1 − f, sprint f], f = clamp((s − 0.75)·4),
  then the stop's own `_tr_h_wait`. **[B]**
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
  70°), look down (event 119: a LedgeGrab edge within 0.6 m, not behind, more than 2 m of drop), the pull-down
  (event 70: an edge within 0.8 m ahead, normal along the facing, more than 2 m of drop). **[B]**
  Seen in the running game (Damascus, the route library): running in high profile to a roof's end that is a beam over a
  0.1 m crack, the next roof 0.74 m below, AC1 ran onto the beam (`beam_crouchwalk`) and free-stepped off it 2.5 m on
  (`freestep_front_front`, low: 6 cm up, 5 m/s); at the same kind of beam over 8 m it made the ledge stop. Ours: the
  ledge stop needs the drop to go on 0.3 m past the edge (no stop at a crack), a beam crossed running in high profile
  is got onto when past it the floor steps down 0.5-5 m, and off a beam in high profile the stick across it free-steps
  onto the floor a step down (at most 1.2 m): AC1's two such hops, off that beam and off a 0.57 m block on the open roof
  (1.07 m down, 3.2 m on in 0.6 s), are both a launch of 5.4 m/s on and 1 m/s up under gravity. A jump onto a post or a
  beam that short stops on it to balance (the block counts as a post), then free-steps on. **[ours, measured]**
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
- **Grabbing a wall** (high profile + legs pushing into it), in order: a climb start (hand holds 1.8-2.4 m up and
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
    held 0.2 s (button 3, 0xED4A20) or the stick within 70° (1.2217 rad) of the edge; otherwise the interpreter only
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
- Lost grip (a hold's object gone, 0xE55A10) → InAir with action 0x1120E17B, blend 0.2. **[B]**

Ours: in the six climbing poses the move comes from AC1's tables (`climb_table.rs`, the entries and action ids
from Banned445's port; each id names its clip in the action blocks, `forge` example `action_clips`): the stick's ten
directions, the long move first past half a stick, redirects followed, the pose it ends in taken from the table (the
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
- **Ground landing** (`HumanInAir__SetupToGround_Landing` 0xE05940): the drop from the apex; over 3 m the damage
  landing (`xx_h_landing_damage_footl`, or `_roll` when moving) with a camera shake of (drop − 3)/7; heavy over 6.3 m,
  fatal over 7.0 m (0xE00FE0; **[V]** its limits are `HumanInAir` fields set by its constructor 0xE0FE80: small damage at the largest float, so from 3 m to 6.3 m only the animation, no health; heavy costs 10 health, fatal 200; a flagged mode (+0xE2 & 8) uses a fixed table instead, 10-20 m in 2 m steps costing 20-160). Up to 3 m: moving with the stick within 75° of the motion → the forward landings
  (`landing_forward_{soft,hard}`, by the speed bucket [< 0.2, < 0.5, < 0.9, else] into walk, jog or sprint impulsion,
  or into the wait when slow); no stick → the straight landings. **[B, ours]**
- **Catching** while falling (0xE0A990 / 0xE0AC70): a hand box 0.4 × 0.3 m, edges within 70°, reach +1.4 m (ledge) /
  +1.95 m (wall); posts and beams are caught too (0xE0BB70). **[B]**

## 6. Jump targets
- AC1 always jumps at a target. The candidates are scored by `0xE96BF0`: within 45° of the wanted direction, no lower
  than 3 m down, the **highest** first, then the nearest. **[B]**
  **[V]** More exactly, the scorer keeps a best candidate per kind (by the candidate's type bits): free-step targets
  (type 1, or 0x10000 with sub-type 2) the **nearest**; the hang-type ones (type bits 0x80 with sub-types 0x10/0x20)
  the nearest unless another is more than 2.5 m higher; the rest (within 45°, more than 3 m down excluded) the
  **highest**. At the end a hang-type best wins over the nearest free-step one when it is at least as high and either
  nearer or 2.5 m higher (target type 0x80 returned); else the free-step one. So onto tops, posts and beams AC1 jumps
  at the nearest in the cone, not the highest.
- The candidates are guidance: LedgeGrab edges facing the player (a roof edge: land 0.45 m in from it if at most
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
  away, else the hop up into a free hang (0xDD62A0), else the pull-up if there is room (0xDE2270); corners
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
- **Wall run over a thin wall** **[ours, AC1's clips]**: a top 1.3-2.5 m up, at most 1.2 m deep with a drop of 0.5 m
  or more beyond: the run up, `xx_h_wallingfront_entry_footl_tr_passover_<131|200>cm_handl` (to 2 m, from the entry) or
  `xx_h_wallingfront_step1_footr_tr_passover_<201|250>cm_handr` (from the first step), blended by height, into
  `xx_h_passover_<hand>_<030|100>cm` (7° off) and its fall, the root carried past the far edge.
- **Wall run onto a ladder** **[ours]**: sprinting at a wall with a ladder up it, the wall run comes first (as the
  interpreter tests it before the ladder) and ends `xx_h_wallingfront_step1_footr_tr_h_ladder_up_l` (7° from
  `xx_h_ladder_climb_up_l`'s start) on the ladder.
- **Ladders and haystack boxes** **[V, ours]** (the running game at the Damascus bureau): moving into a ladder's foot
  from up to 1 m in front gets on with no button. Walking into a haystack's 1.06 m box, AC1 leans on it
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
  `xx_h_hangfree_waitclose` after that upswing. Let go mid-swing: AC1's stop (`xx_h_swing_stop_<front|back>_a..d`, the
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
  The catch after the vertical step (probe D) takes the edge nearest out from the wall first, then the lowest (FindLedge's
  order: distance ahead + 0.01 × height); it asks nothing of the wall under the edge (ours checks the feet's footing
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
- Limb weights rise at 4/s with a contact (0.25 s fade in) and fall at 5/s when released (0.2 s) (`LimbIK` 0xE57570).
  HumanIK's chest pull, shoulder correction and twist are off for humans; reaches stay in the limb. **[B]**

## 11. Cities: what makes Damascus different (ours)
- Damascus's walls have holds (authored guidance) that are not on the clips' 0.6 m steps, storeys set back by
  0.2-0.3 m, sills with walls 0.17-0.3 m behind them, and holds 5-6 m apart where the wall is bare. The climbing
  probe (`AC1_PROBE_CLIMB`) measures it; `AC1_CLIMB_WHY` logs why each climbing move is turned down. **[ours]**
- Our street jumps picked tops past awnings and walls (the flight stopped in the air and dropped back, over and
  over): targets must have a clear way across. **[ours]**

## How far ours follows AC1's tables (2026-10-08)
Every movement context picks its moves by AC1's own data, where the game has a table or an action id for it: the
climbing stance (short / long tables, 10-way stick, redirects), the hangs (ledge-jump table, the sideways order), the
ladder (`HumanLadderData`), the ground (the 17-clip move blend, the starts from standing, run stops, pivots and turns by
their action items, landings by AC1's stick / speed rule), beams and posts (starts, jog stop, turns, side entry,
impulse jumps), jumps (takeoff / flight / reception tables, the edge classification of 0xE18970) and the edge events (as
the input interpreter sends them). Checked by mapping every action id Banned445's port lists for each context to its
clips (`forge` example `action_clips`) against the clips our code plays: what is left unplayed is combat's (hurt
falls, stumbles), other characters' (`cmaa_*`), and the leap of faith's code-driven fall (`xx_h_faith_jump_fall`; ours
stretches the dive over the whole fall).

## Open questions
- Whether the player's jumps ever use the navigation meshes' jump links (the candidates come from the guidance).
- The move mask the interpreter checks (64 bits at the playing item's +8: jump bit 0, pull-down bit 4, look-down bit 29)
  is not the 12-byte flag word we read; where it comes from in the action data.
- Event 68's handler (a ground sub-state's) and event 119's guards in the game's own code (Banned445's values used).
