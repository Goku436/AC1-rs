# The executable's subsystems, and a Rust layout that mirrors them

Made 2026-10-15 from the Ghidra project over `AssassinsCreed_Dx9.exe` (v1.02; 106,707 functions, 4,392 RTTI class
namespaces, 9,194 vtables), decompiling only the clock tick and the frame graph's builders and node callbacks (section 4). The raw outputs (every function's address,
subsystem and class; every class by subsystem) are derived from the user's own executable and stay out of the repo, in
`F:/Claude/re/work/subsys/` (`function_map.tsv`, `classes_by_subsystem.md`, the dumps and the scripts that made them:
`dump.java`, `group3.py`, `final.py`).

## 1. How the map was made

1. **RTTI.** Ghidra's RTTI analyzer gives the class namespaces, their base-class arrays (the hierarchy) and the vtables.
   Walking each vtable's slots assigns 28,170 functions to the least-derived class whose vtable holds them (a function in a
   base's and a derived class's vtable is the base's, unless overridden). Template adapters (`EventCallback<Owner, Event>`,
   `EventHandler<..>`, `EventAccumulator<..>`, `FunctionMember<Owner,..>`, `DataContext<Human, HumanData, X>`,
   `Singleton<T>`, `T::ObjectTypeInfo`) count for their owner class.
2. **Rules on class names and ancestry** group the 4,392 classes into subsystems (`group3.py`, `RULES`; ancestry first:
   anything under `scimitar::ActorContext` is parkour, under `scimitar::Goal` / `AIComponent` / `Bhv*` is AI, under
   `scimitar::Clip` / `AnimTrack*` / `ActionItem` is animation, under `scimitar::MissionObject` is mission, under
   `scimitar::Resource` / `magma::PageHandler` is streaming, `hk*` is Havok; then name patterns).
3. **The call graph** (184,552 edges) places 35,285 non-virtual functions by the majority subsystem of their callers (weight
   2) and callees (weight 1), to a fixed point, with hubs (40+ callers) not voting. 81 utilities with 200+ callers
   (allocators, string and math helpers) are engine core and are left out of the dependency counts.
4. **Address locality.** MSVC links each object file contiguously, and the link order grouped the game by subsystem: 76% of
   consecutive placed functions share a subsystem, and most 64 KB blocks are dominated by one. 4,316 functions take their
   block's subsystem where one subsystem holds 60%+ of the block's vtable evidence; library ranges with no RTTI and no
   calls into the game are labelled by range (SQLite by its error strings at 0x9A0000-0x9DFFFF; 27,000 tiny stubs at
   0x1520000-0x15DFFFF; a 414 KB C library at 0x15E0000-0x1670000 that nothing in the game calls).
5. 28,041 functions are libraries or CRT by Ghidra's own names (`_free`, `std::`, `D3DX`, `FID_` matches). 6,299 (483 KB)
   stay unassigned: 3,478 of them under 64 bytes, 3,976 with no call edge at all (dead code, data read as code).

Confidence: a function's subsystem is certain when it came from a vtable (28,170) or a library name (28,041), good when it
came from the call graph in a subsystem-dominated block, and loose for the engine-core region (0x400000-0x480000), where
the blocks mix and the labels there (mission, world, ...) are often the block's majority rather than the function's own.

## 2. The map

Three layers share the executable: `magma::` (Ubisoft's engine core: object model, reflection and serialization visitors,
memory, pages, UI widgets), `scimitar::` (the Scimitar/Anvil game layer: entities, components, actor contexts, AI, missions,
clips, cameras, sound) and `hk*` (Havok 4.x physics; no RTTI names for its functions, 7,223 of them by their vtables and
address range). `Gear::` and `Os::` are Ubisoft's platform layer (memory allocators, threads, files, network).

| subsystem | classes | functions | code | what it is | key classes (vtable functions) |
|---|---:|---:|---:|---|---|
| player_control | 33 | 726 | 221 KB | the pad, the mouse/keyboard pad emulator, the player's action interpreter (a Goal that turns the pad into action requests), the quadruped's | `ActionControllerEvaluator`(28), `Pad`(15), `GoQuadrupedActionInterpreter`(12), `PadProxyPC`(9), `GoAssassinActionInterpreter`(3; its `Update` 0xEEE700 and the state functions 0xEE05E0-0xEEF300) |
| parkour | 343 | 9,181 | 1,551 KB | the actor contexts (the FSM states of a Human / Quadruped): ground, ledge, climb, walling, in air, narrow object, ladder, haystack, pole, rope, kiosk, riding, dead, upper body, look-at; their data, events, guidance queries, contact and tolerance managers, ragdoll | `HumanGround`(482), `HumanNarrowObject`(120), `Human`(105), `QuadrupedGround`(75), `HumanLedge`(64), `HumanLadder`(54), `HumanInAir`(51), `HumanHayStack`(47), `HumanWalling`(45), `HumanClimb`(44) |
| combat | 217 | 2,720 | 363 KB | fights, weapons, reactions, danger, assassinations, synch kills, the fight tutorial, the informer-assassination mission items | `ReactionHandler`(21), `MissionItemInformerAssassination`(21), `GuardCircleVictimCustomAction`(15), `BhvWeapon`(13), `AttackClip`(13), `GoGuardCircleVictim`(13), `FightTutorialClip`(12) |
| ai | 526 | 6,130 | 702 KB | goals (99 `Go*`), behaviours (`Bhv*`, 48 of the 100 component types), decisions, target monitors, perception, focus, enticers, social and crowd character behaviour | `Focusable`(36), `CharacterBehavior`(33), `AIComponent`(26), `BhvAssassin`(23), `GoSimplePatrol`(23), `BhvGamePlayEnticer`(21), `Goal`(20), `BhvHorse`(20), `AIActor`(17) |
| navigation | 185 | 3,369 | 546 KB | nav meshes and zones, path progress, guidance systems and zones (the climbing markup's owner), crowd flows and cliques, formations, duties | `NavigationContext`(31), `Clique`(30), `CliqueEntity`(29), `CliqueBehaviorHandler`(28), `PathProgressEngine`(17), `GuidanceSystemComponent`(14), `GuidanceSystem`(13), `CrowdFlow`(12) |
| animation | 449 | 7,107 | 838 KB | the clip and action system: 141 `Clip` types (timeline items with conditions), `ActionItem` / `ActionSet` evaluation (the move graph), the `AnimTrack` template family (key count x time x value x compression x interpolator: 64 descriptor types), blends, bone modifiers, IK, facial | `ActionItemEvaluator`(29), the `AnimTrackDescriptorTyped<..>` family (23 each), `AnimComponent`, `ActionComponent`, `FootIK`, `BoneModifier` |
| camera | 162 | 2,610 | 429 KB | the camera stack: free roaming, low/high profile, sweet, target, fight, horse, first person, point of interest, spline, eavesdropping, pickpocket, their activators and settings; the head look-at | `FreeRoamingCamera`(65), `Camera`(42), `LowHighProfileCamera`(33), `SweetCamera`(29), `HumanLookAt`(28), `HorseCamera`(25), `TargetCamera`(24) |
| mission | 342 | 4,893 | 647 KB | mission items and steps, investigations, informers, accomplishments and their monitors, the Animus (memory blocks, save games, replay), scenes and sequencers, meta data | `MissionItem`(48), `MissionItemMM`(34), `MissionItemInformer`(30), `MissionItemList`(23), `MissionItemInvestigation`(22), `InvestigationGroupHandler`(20) |
| world | 89 | 1,433 | 168 KB | world and area handling, triggers and zones (start, condition, fuzzy, detection), terrain layers, grid partition | `FuzzyZoneComponent`(19), `StartTrigger`(16), `TriggerComponent`(15), `TerrainHabitatLayer`(15), `TerrainLayer`(14), `ConditionTrigger`(13), `GridPartition`(9) |
| fx | 60 | 897 | 98 KB | particle systems and emitters, FX graphs (elements, operators, phases, spawned entities), wind, glow, trails | `FXElement`(13), `FXSpawnedEntity`(13), `FXClip`(12), `FXComponent`(10), `ParticleSystemInstanceData`(10), `FXWorld`(9), `ParticleSystem`(9), `Wind`(8) |
| audio | 97 | 2,172 | 300 KB | sound sets and packs, 3D ambient sound components, music (bands, regions, storyline), speech, the DirectSound renderer | `RDSound3D`(42), `Ambient3dSoundComponent`(13), `MusicBandHandler`(12), the sound clips (11 each), `SoundAmbienceManager`(10) |
| render | 181 | 4,123 | 794 KB | the DX9 device and surfaces, materials and shaders, visuals and LOD selectors, mesh instances, cloth, lighting models, post effects, sky | `DX9DirectDisplay`(39), `Visual`(16), `LODSelectorInstance`(14), `MeshShape`(13), `DynamicMeshInstance`(13), `DX9ViewSurface`(13), `Cloth`(12), `RenderScimitar`(11) |
| streaming | 93 | 2,626 | 346 KB | serializers (binary, XML, multi), the bin streams (file, big file, compressed, memory), asynchronous file devices, resources and pages | `BinSerializer`(40), `MultiSerializer`(39), `XMLSerializer`(37), `CompressBinStream`(27), `BigFileBinStream`(26), `AsynchDevice`(13) |
| ui | 340 | 4,693 | 551 KB | magma's widget tree (pages, areas, text commands, fonts, sliders, edit boxes) and the game's menu handlers (`*Handler_m`), HUD, map, subtitles | `InstanceCountVisitor`(59), `Widget`(29), `PageFocusable`(25), `Slider`(21), `TextBase`(20), `MenuComponent`(19), `MagmaMenuHandler_m`(19) |
| game_flow | 59 | 1,585 | 235 KB | game play groups and params, player spawner and actor, gameplay reset and enticers, profiles, options, network tasks | `GamePlayGroupHandler`(31), `GamePlayGroupBehavior`(21), `PlayerSpawner`(12), `PlayerActor`(10), `GameplayResetManager`(10) |
| entity_core | 406 | 3,078 | 226 KB | `Entity` / `Component` / `Spawner`, the physics components (rigid body, character controller, soft body, rope, constraint), the event classes (228 `Event` types, seeds, listeners, accumulators), property and state descriptors | `RigidBodyComponent`(38), `PhysicComponent`(35), `CharacterController`(31), `Component`(29), `Spawner`(25), `RopeComponentNew`(22), `EntityGroup`(21), `SoftBody`(19) |
| engine_core | 247 | 2,409 | 170 KB | magma's object model: `Object` / `ManagedObject` / `BaseObject::ObjectTypeInfo`, the reflection visitors (load, const, cloning, link solving), data direct access, timing strategies | `BinaryLoadVisitor`(187), `LoadVisitor`(143), `ConstVisitor`(82), `Visitor`(81), `ElementCloningVisitor`(37), `DataDirectAccess`(33), `LinkSolverVisitor`(28) |
| memory | 72 | 484 | 27 KB | Gear allocators (dlmalloc 2.7/2.8 ports, fixed and page allocators), memory hooks, threads, buffers | `MemoryHook`(17), `MemAllocDl283`(17), `BlockAllocator`(14), `Thread`(10) |
| havok | 401 | 7,223 | 1,094 KB | Havok: broad phase, agents, shapes (MOPP, convex, height field), constraints, motions, vehicle, collision filters | `hk3AxisSweep`(23), `hkConvexListShape`(17), `hkMoppEmbeddedShape`(17), `hkKeyframedRigidMotion`(17), `hkSimpleConstraintContactMgr`(17) |
| sqlite | 0 | 811 | 210 KB | an embedded SQLite (0x9A0000-0x9DFFFF; the Animus database / tracking) | |
| crt_libs | 90 | 28,817 | 488 KB | the MSVC CRT and STL, D3DX, the name undecorator, MFC leftovers (`CTool`, `COleResizeBar`), Bink (`*.bik`), the small-stub region | |
| lib_unknown | 0 | 3,248 | 414 KB | a C library at 0x15E0000-0x1670000 with no RTTI that nothing in the game calls (vendor code linked whole) | |
| unassigned | 0 | 6,299 | 483 KB | tiny functions and dead code with no edges | |

Where the game's code sits (64 KB blocks a subsystem dominates): parkour 0xB10000-0xB2FFFF and 0xD50000-0xE5FFFF (the
actor contexts, one class after another: `HumanGround` 0xD7-0xDB, `HumanLedge` 0xDC-0xDF, `HumanClimb` 0xE0-0xE3,
`HumanInAir` 0xE4-0xE5) and 0xF10000, 0xF60000; the player's interpreter 0xEE0000; AI 0xCA, 0xD1, 0xEA-0xED; missions
0x50, 0x56, 0x63, 0xC2-0xC4, 0xF0; animation 0x49-0x4A (the track templates), 0x58-0x59, 0x5C, 0x62; navigation
0x66-0x67; render 0x9E-0xAB; audio 0x44, 0x7F, 0xBC; FX 0x6A; world 0x4B; UI and magma 0x85-0x8E; Havok 0xFD0000-0x10BFFFF
and 0x1180000-0x11BFFFF; the CRT 0x81-0x83, 0xFA-0xFC, 0x110-0x111.

### Who calls whom

Calls between subsystems (from the row to the column; engine core, memory, libraries and the 81 utilities left out):

```
               playe parko comba    ai navig anima camer missi world    fx audio rende strea    ui game_
player_control     .   268    33    72    53    43    31     8    41     3    10     3     1     2     1
parkour          217     .   407   450   352   465   105    54   151    34   191    42    13    14    16
combat            29   435     .   315    42   171    24    85    67    26    88     9     1    55     0
ai                88   340   211     .   239   163    29    69   155     9   190    34    10    55    31
navigation        62   249    40   284     .    75    13    68    60     5    41    17     8    16    13
animation         33   386    61   106    27     .    74    61    38    63   118    67    31   100     3
camera            47   101    22    74    10   125     .    38    39     5    87    41    45    13     2
mission            4    23    29    89    47    80    19     .    90    38   157    75    53   317    99
world              0    33     6    16    20    19     7    16     .     6    28    37    23    15     9
fx                 0    13     1     4     1   116     6    29    17     .     5    30    18     2     5
audio              2    28     3    21     5    36     7    30    34     2     .    63   305    10    10
render             1    25     8    36    16   149    61    74    36    45    82     .   236    85    27
streaming          1    11     1     7     0    17    34    35     9     5   215   116     .    22    12
ui                 9    22     9    60     8    70    21   217    34     9    78    37    46     .    12
game_flow          1     2     7    37    39    15     1    91    18     7    21    23    16    21     .
```

What it says:
- **parkour, animation, AI, combat and navigation are one knot.** Each calls the others hundreds of times both ways
  (parkour -> animation 465 and back 386; parkour -> AI 450 and back 340; combat -> parkour 435 and back 407). In the
  executable they are not layered: the actor contexts start clips and read the action evaluator, the clips and action
  items call back into the contexts (events, root motion, item locks), the goals drive the contexts and read their
  state, the fight is a context family of its own. Any module layout that puts these in separate crates needs traits
  for the back edges; modules inside one crate match the executable.
- **player_control sits between the pad and the contexts** (268 calls into parkour, 217 back: the interpreter reads
  context state to decide the event to send). It is small (221 KB) and self-contained apart from that.
- **Camera reads animation and parkour** (125 and 101 calls) and is read by few: a consumer that runs after them.
- **Mission is the UI's main client** (317 calls into ui, 217 back) and talks to game_flow; it rarely touches movement.
- **Audio depends on streaming** (305 calls; the sound banks stream) and is called from everywhere (every subsystem plays
  sounds through events).
- **Render, FX and streaming** form the presentation side; render reads animation (149, the skinned meshes) and world.
- **world, game_flow and entity_core** are leaves everyone reads.

## 3. The object model the game is built on

Everything game-side is a `scimitar::Object` (2,058 descendants); the reference-counted ones are `ManagedObject` (1,014).
The patterns Bevy has to stand in for:

- **Entity + Component.** `scimitar::Entity` holds `Component`s (100 types: `AnimComponent`, `ActionComponent`,
  `SkeletonComponent`, `AIComponent`, `CharacterController`, `RigidBodyComponent`, `PhysicComponent`,
  `GuidanceSystemComponent`, `TriggerComponent`, `Ambient3dSoundComponent`, `FXComponent`, `VisualComponent`,
  `CameraZone`, `MenuComponent`, ..., and the 48 `Bhv*` behaviour components: `BhvAssassin`, `BhvHorse`, `BhvLadder`,
  `BhvHayStack`, `BhvNarrowObject`, `BhvBreakable`, `BhvKiosk`, `BhvTradingPost`...). Components are serialized with
  their entity (the object containers `forge::object` parses) and found by class id at run time.
- **Actor contexts.** A `Human` (an `AIActor` with an `IHuman` interface) runs one `ActorContext` at a time, each an
  `FSMStaticState` with its own `DataContext<Human, HumanData, XData>`: 21 of them (ground, ledge, climb, walling, in air,
  narrow object, ladder, pole, rope, hay stack, kiosk, riding, dead, upper body, look-at, vocalization, ragdoll ground,
  debug) and 4 for quadrupeds. A context switch is a `TransitionSetup` handed over inside the frame: the new context's
  entry runs in the same update the old one left (Banned445's port keeps this; ours keeps it inside `WallClimb`).
- **Events.** 228 `Event` types; handlers are members bound through `EventCallback<Owner, Event>` / `EventHandler<..>`
  (612 `IEventHandler` implementations); `EventSeed`s are serialized event templates fired by data (a trigger zone's
  `Ambient3dSoundAudioEventSeed`); `EventAccumulator<Owner, Event>` batches events for an owner to take at its next
  update. The two delivery styles (immediate dispatch through the owner's dispatcher, as `HumanGround`'s 0xDB1470 takes
  the interpreter's events the moment they are sent, and accumulated-until-update) are the timing distinction that
  matters for Bevy (section 6).
- **Goals.** 99 `Goal`s run by `AIComponent`s: patrols, searches, fights, navigation, needs; the player is one too,
  `GoAssassinActionInterpreter`, the goal that reads the pad (through `PadProxyPC`, the interpreter's +0xD0) and sends the
  contexts their events. Nothing distinguishes the player's human from an NPC's but its goal.
- **Clips and actions.** The animation side is data-driven twice: `ActionItem`s / `ActionSet`s (the move graph, blends and
  gates `forge::action` / `forge::graph` read) picked by the contexts, and 141 `Clip` types on timelines (scenes, missions,
  cut scenes, music, FX, conditions) with `ConditionClip`s gating them.
- **Resources and pages.** `scimitar::Resource` (207 descendants) are loaded through `magma::PageHandler` /
  `AreaHandler` by asynchronous file devices; the serializers are visitor-based (`BinaryLoadVisitor`, 187 virtuals: the
  reflection the class descriptors in the exe describe, docs/NOTES.md "Class names").

## 4. The frame

**AC1's frame is a task graph.** `FUN_00417930` (run once, at start-up) builds it: each node is a named job (the names
are the game's own profiler scopes) with a callback, and an edge `A -> B` (`0x942110`: B keeps a list of the nodes it
waits for) makes B wait for A. A job pool runs whatever is ready, on any of four threads, so the frame has no fixed
thread layout; what is fixed is the graph. `FUN_00417470`, found first as "the frame function", is only the callback of
its first node, `Engine::BeginFrame`. The game's update is three more graphs per world (built by `0x45AA80`, `0x45ACC0`,
`0x45C900`), joined into the frame each frame by `Engine::EngineLoop::Step` (`0x415FD0`).

The top level (node, its callback):

```
Engine::BeginFrame (0x417470: the clock tick 0x416420)
 |-> Graphic::BeginGraphicFrame (0x4162D0) -> Graphic::GraphicFrame (0x4168F0) -> Graphic::EndGraphicFrame (0x415730) -> Engine::EndFrame
 '-> Engine::BeginEngineFrame (0x4167B0: the pad read, PadProxyPC::Update 0x909C90)
      -> CheckEngineStop (a branch on engine +0xD8: stop -> Graphic::EndEngineFrame)
      -> Engine::Step (0x415600)
          -> Magma::MagmaStep (0x784FE0, the menus) | Ai::UpdateCamera (0x41E3E0) | Sound::Update (0x40ED40)   (in parallel)
      -> a loop check (0x415F90, a branch: stop -> Graphic::EndEngineFrame)
      -> Engine::EngineLoop::Step (0x415FD0), which adds this frame's chain:
          Engine::StepPreWorlds (0x415620)
          -> for each world: its PreAI graph (whose Gather node waits only on EngineLoop::Step: beside StepPreWorlds)
          -> the global AI graph (DAT_01a11cb0: its entry node and +0x2A8)
          -> for each world: its AI graph, then its PostAI graph
          -> Ai::AvoidanceManagerUpdate (0x480E20) | Anim::ActionManagerUpdate (0x468FB0) | Ai::AccomplishmentManagerUpdate
      -> Graphic::EndEngineFrame (0x415710) -> Engine::EndFrame (0x4157C0)
```

A world's three graphs (offsets in its scheduler block, `world+0xC0`):

```
PreAI   +0x4..+0x354:   Ai::UpdatePreAIGather (+0xAC) -> Ai::UpdatePreAISetup -> Ai::UpdatePreAI (jobs)
                        | Ai::UpdatePreAISingleThreaded | Graphic::UpdateEntitiesCullingFlags -> Ai::UpdatePreAIWrapup
AI      +0x4B4..+0x6B4: Ai::UpdateAISetup -> Ai::UpdateAISingleThreaded | Ai::UpdateAIMultiThreaded -> Ai::UpdateAIWrapup
PostAI  +0x814..+0x1418:
   Ai::UpdatePostAISetup -> Ai::UpdateActionStage2 (ActionComponent, jobs) and Ai::UpdatePostAIGatherComponents
   then two branches that run AT THE SAME TIME:
     physics:   Physic::BeforeSimulation -> Physic::PhysicUpdate (0x45C2B0, the Havok world step) -> Physic::AfterSimulation
                -> Physic::PhysicsWrapup -> (the PostAI wrap-up)
     animation: Anim::UpdateActionStage3 (jobs) -> Anim::UpdateAnimation (jobs)
   joined at:   Anim::SkeletonUpdate (waits for AfterSimulation and UpdateAnimation) -> Physic::AfterAnimation
                -> Ai::UpdateAIAfterPhysics -> Anim::SkeletonPostUpdate x4 (0x4E7C60, 0x4E07C0, 0x4E75C0, 0x4E6790)
                -> Graphic::FXUpdate (0x4236C0) | Engine::UpdateAfter -> Ai::UpdatePostAIWrapup
```

Measured live (2026-10-15, our hook's probes on every node's callback, each hit stamped with the frame, the thread and
the time; Damascus, the bureau, 8 characters in view; the probes stretch the times about ten-fold, but the order held in
every frame; the hit lists are in `F:/Claude/re/work/subsys/hits1.txt` .. `hits5.txt`, and `hits3.txt` with `live3.txt`
is a wall-run jump):

| order | node | what runs in it (probe, vtable slot) |
|---|---|---|
| 1 | BeginFrame, BeginGraphicFrame, GraphicFrame | the clock tick; then the **render of the previous frame's state** on one thread, alongside everything below until EndGraphicFrame |
| 2 | BeginEngineFrame | `PadProxyPC::Update` 0x909C90 (slot 9): the pad read |
| 3 | Engine::Step -> MagmaStep, UpdateCamera, Sound (in parallel) | `LowHighProfileCamera` slots 83 and 102 (0x692D90, 0x690160): **the camera updates before any character, from last frame's positions**; sound reads last frame's positions too |
| 4 | EngineLoop::Step -> StepPreWorlds, PreAI | `Human` slot 34, 0xB17EF0 (the edge reports) for every character, as PreAI jobs |
| 5 | AI: Setup, `GoAssassinActionInterpreter::Update` 0xEEE700 (slot 18), then AIMultiThreaded | the player's interpreter first, then every character's actor-context update as AI jobs (`HumanGround__UpdateHeading` 0xD95290 three at a time; `HumanGround` slot 237, 0xDAC620, the player's) |
| 6 | PostAI: ActionStage2 (jobs) | the `ActionComponent`s' stage 2, per character |
| 7 | PostAI: PhysicUpdate, on one thread | the Havok world step; in it `CharacterController` slot 33, 0x57D900, **serially** for each character (the pre-integration step, which takes the frame's motion), then the proxies' integration (0x57D240 / 0x57B0A0, from slot 61) and AfterSimulation (`CharacterController` slot 62, 0x57A8A0, the write-back) as jobs |
| 7' | PostAI: ActionStage3, UpdateAnimation, **at the same time as 7** | `AnimComponent` slot 32, 0x575220, per character, on the other threads |
| 8 | PostAI: SkeletonUpdate, AfterAnimation, AIAfterPhysics, SkeletonPostUpdate x4 | the join: the pose from this frame's animation and this frame's physics; `SkeletonComponent` slot 33, 0x4E8920, finalises it |
| 9 | FXUpdate, UpdateAfter, then Avoidance, ActionManager, Accomplishment, EndEngineFrame, EndFrame | effects from the final poses; the managers last |

The context handoff, measured on the wall-run jump: on the switch frame the interpreter's update (0xEEE700) reaches its
wall-run side (0xEE05E0), and `HumanWalling`'s update (0xE37590) runs 0.5 ms later on the same thread, while `HumanGround`
slot 237 is not hit that frame: the new context runs the same frame, inside the AI phase, before physics.

Spawns (once, not per frame): `SkeletonComponent` slot 3 -> `ActionComponent` slot 3 -> `AIComponent` slot 3 ->
`SkeletonComponent` slot 14 -> `HumanGround` slot 2 -> slot 14, then `Human` slot 12 a frame later. Not hit in the
measured frames: the PostAI `SkeletonUpdate` callback, the first two `SkeletonPostUpdate` callbacks and the AI
single-threaded node (they may run only when they have work). The three calls `0x417170`, `0x417250`, `0x415F10` first
taken for the engine's steps are not on the per-frame path: the graph is the frame.

What this settles for the port:
1. **The camera comes before the characters.** AC1 computes this frame's camera from last frame's body, before the
   interpreter reads the stick.
2. **The interpreter, then the contexts, all in the AI phase.** The player's interpreter runs before every character's
   context update, and a context switch takes effect inside that phase.
3. **Physics and animation run in parallel and meet at the skeleton.** The Havok step (the character controllers
   serially, then their integration and write-back) runs beside this frame's animation update. The controllers use the
   motion that the action stages set up, not the sampled pose. Both meet in `SkeletonUpdate`, and the pose is finalised
   after physics.
4. **Effects and managers come last, and the picture lags.** FX, avoidance and accomplishments run after the final poses.
   The render draws the previous frame while this one updates, so AC1's picture is one update behind its simulation.

**The clock** (`FUN_00416420`, decompiled): reads the performance counter; spin-waits until a target time if one is set
(+0x20/+0x24: a frame-time floor, the frame cap); then the frame's game time by mode (+4): **0** a fixed step 1/(+8) s;
**1** a table of steps (+0x2C[]) with a hysteresis counter (+0x28, 10 frames) that moves to the next longer step only
after ten long frames and drops back at once; **2** the measured time capped at +0x2C[0]; **3** the mean of the last 16
frames; then times the speed (+0xC); then a floor (the 0.1 ms a frame our hook's `pause` leaves). The game ran in **mode 2
with the cap at 1/30 s**: a variable timestep, never longer than 33.3 ms, no fixed physics step at the engine level. Every
timer in the game (the 0.3 s legs buffer, the 0.3 s quick-reversal flag, blend times, the Ledge's "item 0 + 0.2 s", the
pad's per-button press times at +0x30/+0x38) is in this game time, so a slow frame stretches them with the simulation,
and a very slow frame (over 33 ms) runs the simulation slower than real time.

## 5. A Rust layout that mirrors it

The dependency matrix allows a crate boundary where the calls are one-way, and asks for modules in one crate where they
are two-way. Proposed workspace (what each maps from in the current tree in brackets):

```
crates/
  magma/         engine core + streaming + memory: the object container, reflection descriptors, serializers,
                 archives, pages, LZO, the index                       [forge: lib, read, datafile, index, object]
  forge/         the game's resource formats on top of magma: meshes, textures, skeletons, clips, action blocks,
                 the move graph, shapes, guidance, nav meshes, sound, FX, build tables, world/ambiance
                                                                       [forge: the rest; stays the format crate]
  ik/            pure math: IK, ragdoll, cloth, chains                 [ik, unchanged]
  anim/          animation: clip sampling and binding, the action evaluator, blends and seams, bone modifiers,
                 the clip timeline (Clip types as needed), IK goals    [game: animation.rs, move_blend.rs, gait.rs,
                                                                        the blend/seam code now inside climb.rs]
  scimitar/      the knot, one crate, modules as the exe's classes:
    entity/      Entity, Component registry, spawners, the 228 events as typed enums, seeds, accumulators
    actor/       the ActorContext FSM and its contexts                 [game: climb.rs 11,230 lines, character.rs]
      ground.rs      HumanGround (482 virtuals: the biggest class in the game)   [character::locomotion, gait, edge_report]
      ledge.rs       HumanLedge (hangs, hand steps, pull-ups, swing)             [climb.rs HANGWALL*/free_*/swing_*]
      climb.rs       HumanClimb (the climbing stance grid)                       [climb.rs 1m/2m/* stance code]
      walling.rs     HumanWalling (the wall run and its catches)                 [climb.rs WALL_RUN*, try_rebound]
      in_air.rs      HumanInAir (jumps, flights, catches, landings)              [climb.rs jump*, fall, try_catch]
      narrow.rs      HumanNarrowObject (beams, posts, the free-step stay)        [climb.rs PERCH*, beam_*]
      ladder.rs, pole.rs, rope.rs, haystack.rs, kiosk.rs, riding.rs, dead.rs, upper_body.rs, look_at.rs
      quadruped/     QuadrupedGround, QuadrupedInAir                             [new]
      data.rs        the HumanData / per-context data blocks AC1 reads from its tables  [climb_table.rs, jump.rs tables]
    interpreter/ player_control: the pad model (bands, snap, curve, latch, buffers), GoAssassinActionInterpreter's
                 states and events, the mouse/keyboard emulator        [main.rs player_input/pad_input, pad.rs]
    guidance/    navigation: guidance systems, hold grids and depth filters, edge reports, jump candidates, nav
                 meshes, crowd flows and cliques                       [level.rs holds, guide_depth.rs, jump_query.rs,
                                                                        edge_report.rs, beam_pairs.rs, ledge_grid.rs, nav.rs]
    ai/          goals and behaviours: the 99 Go*, the Bhv* components, decisions, perception, enticers, crowd
                 character behaviour                                   [city_life.rs, crowd.rs, npc.rs, combat's guard AI]
    combat/      fights, weapons, reactions, assassinations, synch kills [combat.rs, fight_data.rs, fight_tables.rs]
    camera/      the camera stack and look-at                          [main.rs camera_follow, shake.rs]
    mission/     mission items, accomplishments, Animus, save games, scenes [new; settings/save only today]
    world/       areas, zones, triggers, terrain, the city's placed entities [level.rs, city.rs placement, level_file.rs]
    fx/          particle systems, FX graphs, wind                     [particles.rs, particle_material.rs, forge::fx]
    audio/       sound sets, 3D sounds, music, speech                   [sound.rs]
  render/        the DX9-side equivalents: materials and shaders, visuals and LOD, streaming of draws, lighting, sky,
                 post, cloth draw                                      [city_material.*, character_material.*, look.*, sky.rs,
                                                                        stream.rs, render_cache.rs, assets.rs]
  game/          game_flow and ui: the app, settings and menus, HUD, the editor, net and replays, the hook-side
                 tools (recorder, scenarios, sweeps)                   [main.rs, menu.rs, settings.rs, editor.rs, net.rs,
                                                                        replay.rs, recorder.rs, climb_sweep.rs, profile.rs]
```

Why these boundaries:
- `magma` -> `forge` -> `anim` -> `scimitar` -> `render` -> `game` is one-way in the executable (streaming is called by
  everyone and calls audio only to stream banks; animation is called by parkour 465 times and calls back 386, so `anim`
  holds only the clip machinery and the evaluator, and the parkour-side blend decisions stay in `scimitar::actor`).
- The knot (actor, interpreter, guidance, ai, combat, camera, world, events) is one crate because every pair has back
  edges; inside it the modules mirror the classes one for one, which is what makes the executable's functions findable
  from ours (a `HumanLedge` finding goes to `actor/ledge.rs`).
- `render` is separate because nothing in the knot calls it (render -> animation 149, animation -> render 67: the few
  back edges are the skinned mesh's bone uploads, which `anim` can expose as data).
- Splitting `climb.rs` along the contexts is the one move the map argues for most: the executable keeps 482 + 120 + 64 +
  54 + 51 + 47 + 45 + 44 virtual functions in eight classes where we keep one 11,230-line file; the context FSM
  (`WallClimb` today) becomes `actor::Context` with one module per state and a shared `Human` data block.

## 6. Bevy systems and components for the original's logic, and where Bevy's scheduling changes its timing

### The translation

| executable | Bevy |
|---|---|
| `Entity` with `Component`s found by class id | `Entity` with typed components; one component type per AC1 component class that has run-time state (`AnimComponent`, `ActionComponent`, `CharacterController`, `RigidBodyComponent`, `AIComponent`, `Bhv*`, `TriggerComponent`, `Ambient3dSoundComponent`, `FXComponent`), data-only ones kept as plain fields of their owner |
| `Human` + its active `ActorContext` (FSM, `DataContext<Human, HumanData, X>`) | `Human` component (the shared `HumanData`) + `ActorState` enum component holding the active context's data; **one** system `actor::update` steps the FSM (not one system per context, see hazard 2) |
| `GoAssassinActionInterpreter` reading `PadProxyPC` | `Interpreter` component on the player (its latch, bands, buffers, press times) + `PadState` resource written in `PreUpdate`; `interpreter::update` runs first in the knot's set |
| `Goal`s run by `AIComponent` | `Goal` enum component per NPC + `ai::run_goals` system (the player's goal is the interpreter, so NPCs and the player diverge only in which system writes their `ActorRequest`s) |
| `Event` sent to a handler at once (`HumanGround`'s dispatcher 0xDB1470) | a direct call inside the same system, or a `Commands`-free message queue drained in the same system; **not** a Bevy `Message`/`Event` (hazard 3) |
| `EventAccumulator<Owner, E>` taken at the owner's next update | Bevy `Messages<E>` read by the owner's system (the double buffer matches "next update") |
| `EventSeed` (data-driven event templates) | data in `forge`, fired by the owning system when its trigger holds |
| `Clip` timelines and `ConditionClip` | `Timeline` resource per scene with `Clip` enum items; `anim::clips` system; conditions as closures over the ECS state |
| `ActionItem` / `ActionSet` evaluation (the move graph) | `anim::Evaluator` as a component (the playing item, its gates and blend), stepped by the actor system in the same frame it changes state |
| `AnimTrack*` sampling at the clock | `anim::sample` in `PostUpdate` before transform propagation; the game time from `Time<Virtual>` with the same cap as AC1 (hazard 1) |
| `CharacterController` / `RigidBodyComponent` (Havok) | our capsule and ragdoll in `scimitar::actor`/`ik`, stepped inside the actor system (not a separate physics schedule, hazard 6) |
| the camera stack (`LowHighProfileCamera`, `SweetCamera`...) | `CameraMode` enum resource + `camera::update`; AC1 runs it before the interpreter, on last frame's body (section 4, hazard 9); ours runs after the actor system, before propagation |
| `TriggerComponent` / zones | `Zone` components + `world::triggers` system before the AI and actor systems (so a zone entered this frame acts this frame, as in AC1 where the trigger's event is immediate) |
| `Resource` / pages | Bevy `Assets` + our loader threads and caches (`stream.rs`): the one place the frame may wait |

### The schedule

AC1's frame is a task graph run by a job pool (section 4): pad -> camera -> pre-AI -> interpreter -> contexts -> physics
beside animation -> skeleton -> FX and managers, with the render of the previous frame alongside. Bevy's default is
parallel within a schedule and ordered only where told. The faithful shape is one chained set per stage with explicit `after`s between the stages:

```
First:      tick (Time<Virtual> with max_delta = 1/30 s: AC1's clock mode 2), script clocks, ledge grid
PreUpdate:  pad read -> interpreter  (player_control)
Update:     world triggers -> ai goals -> actor FSM (parkour + combat, one system, same-frame handoffs)
            -> crowd / NPC locomotion -> anim evaluator -> camera          (all .chain())
PostUpdate: anim sample -> IK -> cloth -> transform propagate -> render upload / streaming / sound events
Last:       recorder, frame cap
```

which is what `main.rs` already does for the player (`player_input, run_script, fly, locomotion, crowds, animate,
camera_follow, ..., recorder` chained; `pad_input.before(player_input)`; cloth and the sky after `TransformSystems::Propagate`).
The hazards below are where the default Bevy shapes differ from the original.

### Where Bevy's scheduling changes the timing

1. **The timestep.** AC1: variable, capped at 1/30 s, times a speed factor, floored at 0.1 ms. Bevy `Time<Virtual>`:
   variable, capped at `max_delta` = 250 ms by default, with `relative_speed`. A hitch of 100 ms moves AC1's simulation
   33 ms and ours 100 ms: landings, catches and the 0.3 s buffers all land differently after a stall. Set
   `Time::<Virtual>::set_max_delta(1/30 s)`; never put movement in `FixedUpdate` (AC1 has no fixed step; its clip ends,
   item locks and buffers happen on frame boundaries of the variable clock, and a fixed step would move every clip end
   to the next fixed tick). Scripted runs are fixed at 1/60 s by `AC1_STEP`, which is AC1's mode 0, so they are
   repeatable; the user's play is mode 2.
2. **Same-frame handoffs.** A context switch in AC1 (ledge -> climb, walling -> ledge, ground -> in air) enters the new
   context in the same update, and the new context's first tick (its entry clip, its first room test, its first event
   check) happens that frame. Measured (a scripted high-profile jump at the bureau wall, `hits3.txt` / `live3.txt`):
   on the switch frame the interpreter's update (0xEEE700, 1.7 ms after the pad) reaches its wall-run side
   (0xEE05E0) at 2.1 ms and `HumanWalling`'s update (0xE37590) at 2.6 ms on the same game thread, while `HumanGround`
   slot 237 (the ground post-update, hit three times a frame until then) is not hit at all that frame: the old
   context's per-frame work is skipped and the new one's runs, all before the serial controller pass. The same for
   the walling -> in-air switch (the air-catch query 0xE0BB70 first fires on the switch frame). If each context were
   its own Bevy system, a switch made by system A is seen by system B
   next frame (or this frame only if B happens to be ordered after A): one frame of hang pose at the start of every
   climb, one frame of the old clip after every catch. Keep the FSM in one system, as `WallClimb::update` does, and loop
   until the state settles within the frame.
3. **Immediate events versus `Messages`.** The interpreter's events to the contexts, the contexts' events to each
   other (eject, let go, the Ledge's `TransitionSetup`), the trigger zones' entry events and the fight's hit events are
   dispatched immediately in AC1: the handler runs before the sender's update returns. Bevy's `Messages<E>` are
   double-buffered: a reader that runs before the writer in the schedule sees the message next frame, and a reader after
   it sees it this frame. A reorder of two systems (or Bevy picking a different parallel order when no `after` is given)
   silently adds a frame to a chain that AC1 runs in one. Use direct calls or a queue drained in the same system for
   these; use `Messages` only for what AC1 accumulates (`EventAccumulator`: sound notifications, scene outputs, UI
   notices).
4. **Input sampling.** AC1 reads the pad inside the frame (`PadProxyPC::Update`, slot 9) and stamps each button's press
   time with the pad's clock (+0x30/+0x38); the interpreter's buffers (the legs press, 0.3 s; the quick reversal, 0.3 s;
   the latch, 0.3 of stick travel) compare against game time. Bevy reads input at the start of the frame (`PreUpdate`,
   `winit` events) against `Time<Real>`: with a speed factor or a pause, a buffer measured in real time outlives or dies
   early against the simulation. Stamp presses with `Time<Virtual>` (the hook's `speed 0.25` runs showed AC1's buffers
   stretch with the slow motion).
5. **`GlobalTransform` is a frame old in `Update`.** Bevy propagates transforms in `PostUpdate`; a system in `Update` that
   reads a bone's or the root's `GlobalTransform` reads last frame's. AC1 computes the pose and reads it in the same
   update (the limb IK goals, the camera's target bone, the hand-to-hold distances). We hit this (audit WALL-8: the eject's
   takeoff turned the root but the pose shown that frame was the hang's; fixed by `snap_now` applying the turn before the
   first frame is shown). Read our own `Pose` data, not `GlobalTransform`, in anything that runs before propagation.
6. **Physics ordering.** Measured (section 4): physics is one node of the frame graph, `Physic::PhysicUpdate`, after
   every character's context update. In it the character controllers take one serial pass (the pre-integration step,
   which takes the motion the action stages set up), then their proxies integrate as jobs, then `AfterSimulation`
   writes the positions back. Meanwhile, on the other threads, this frame's animation is updated. The skeleton waits for
   both. All of it runs at the frame's dt (the capped clock), once a frame. With a separate physics system in the same
   schedule and no `after`, Bevy may run it before or after movement on different frames (the executor's order is not
   fixed when systems do not conflict). Contacts, the ragdoll and the character capsule then see this frame's or last
   frame's pose at random, and scenarios stop being repeatable. Chain it in AC1's order (contexts -> controller step ->
   integrate -> write back, beside the animation sample -> pose), and step it with the same capped dt, not
   `FixedUpdate`. Running physics and animation in parallel is safe only because neither reads what the other writes
   that frame. If ours samples the pose and reads it back in the capsule step, keep the two chained instead.
7. **Clip ends and locks on frame boundaries.** AC1 ends an item, applies its gate (the 0x20 lock, the 0x1000 takeoff
   bit) and starts the next item in the frame the clip's time passes its end; the leftover time carries into the next
   clip (our chain seams do this since 2026-10-13). Any scheme that samples animation in one system and decides the next
   item in another (a frame later) lengthens every move by a frame and breaks the measured route timings.
8. **Streaming stalls.** AC1's loader threads fill pages; the simulation does not wait, it reads what is loaded (an
   unloaded cell has no collision yet: AC1's "stand in the air until the cell comes" at a hard teleport). Bevy async
   tasks finish on arbitrary frames; a scenario that depends on a cell's holds must either load synchronously (as
   `AC1_HEADLESS` runs do) or be written to wait, or it is non-deterministic. The warm caches (level and render) make the
   first frames the same every run.
9. **Camera and stick frame.** The stick is camera-relative. Measured (section 4): AC1 updates the camera **before** the
   interpreter, from last frame's character position, so the stick is read against a camera that is one frame behind
   the body. Ours runs `camera_follow` after `locomotion`, so our camera is up to date with the body and the stick is
   read against the previous frame's camera: the same stick frame as AC1 (both use the camera computed from the
   previous body position), but our camera is drawn a frame ahead of AC1's. Moving `camera_follow` before
   `locomotion` would match AC1's drawn camera exactly; keep the camera-cut latch (`stick_yaw`) either way.
10. **Contact keys and sounds.** AC1 fires a clip's event keys when the clip time crosses them during the frame's
    sampling; a key crossed is fired once even when a long frame crosses two. With a fixed-rate audio or FX system
    polling the pose, keys would be missed or doubled; fire them from the sampler (as `sound.rs` and `particles.rs` do).

What the executable settles and what it does not: the clock, the object model, the class-by-class layout and the
cross-subsystem dependencies above are read from the binary; the within-frame order of the game's phases, which fixes
hazards 2, 6 and 9 for certain, is a probe run away (section 4).
