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
  **[B]** (ours: bands and banks, not the full 17-weight blend.)
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
- **Grabbing a wall** (high profile + legs pushing into it), in order: a climb start (hand holds 1.8-2.4 m up and
  foot holds 1.2 m below them) → `xx_h_wait_hipm_foot?_tr_climbing_1m` with the root interpolated onto the holds;
  else a ledge 0.53-3.0 m up → the standing straight jump at it (0xB21DA0: knee and waist heights pull up onto the
  top, higher ones end hanging). The grab probe radius is 0.75 m (IHuman vt136). **[B]**
- **Character proxy**: capsule radius 0.4 (0.35 + 0.05 keep distance), height 1.8, floating 0.37 m over the feet
  (step), stick-to-ground cast 0.58 m (0x57D240). **[B, ours]**

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
5. **Start from standing** (0xD98990) when the stick moves.
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

Ours: moves are AC1's clips, accepted when the clip's end hands land within a tolerance of a hold (0.2 m, 0.35 m
up/down), the root corrected onto it. The grid and the 1.0 m depth box are the next step (Damascus walls vary in
depth: see section 10).

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
  0x010DF150), into the run, walk, wait or the beam; wall hang → `xx_h_air_surface_tr_hangwall_reception_*`; free
  hang → the swing reception; post / beam → their mounts (0xE50190, 0xE52AD0). **[B, ours]**
- **Ground landing** (`HumanInAir__SetupToGround_Landing` 0xE05940): the drop from the apex; over 3 m the damage
  landing (`xx_h_landing_damage_footl`, or `_roll` when moving) with a camera shake of (drop − 3)/7; heavy over 6.3 m,
  fatal over 7.0 m (0xE00FE0). Up to 3 m: moving with the stick within 75° of the motion → the forward landings
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
  (Ours stopped playing the edge stop: AC1 just stops 0.3 m short.)
- Where beams come from: `GuidanceBeamDetectorAccurate` (vtable 0x0169A1CC, constructor 0x678F80) is never loaded
  from data (no objects of the class in any forge); it is built on the stack with fixed values (1.0, 0.15, 5° =
  0.0873 rad, 0.5, 0.2) by 0xC7E810, which runs lazily when a query box overlaps a guidance object's bounds
  (0xC7EAA0, AABB test 0x61E880) and caches the result. It does not search city geometry: 0x680260 cuts the guidance
  object's own shape (object +0x170) into beam segments (start, end, half widths, the two side normals), and
  0x67DA30 builds each beam's volumes: the top and both sides, tilted ±5° about the beam's line, 0.15 m past its
  half width, 0.5 m along the up normal. So city beams are the authored guidance beams, which we already load (937 in
  Damascus → 1141 perches). **[V]**

## 9. Wall run, ladders, hay, swing bars
- **Walling** (event 49, wall test 0xE18390): entry A → B → vertical → end (0xE37590); probes A-D hand over to the
  ledge with the `wallingfront_*_tr_*` exits; the rebound (0xE365C0) pushes off straight back when the stick is
  within 50° of the facing, else along the stick. Only the front wall run exists in the data. **[B, ours]**
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

## Open questions
- The game's own jump candidate list (IHuman vt56/64/68) and whether jump links are used by the player.
- The senders of several ground events (69 ledge stop, 70 pull-down, 119 look down).
- The response curve of the speed parameter's fall, and the 17 locomotion weights (Banned has them in `move_blend`).
- Each item's gate word for every movement action (Banned's `item_flags.rs` is generated from the install: we can read
  the same with `forge`).
