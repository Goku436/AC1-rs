# AC1-rs

A from-scratch Rust/Bevy runtime for Assassin's Creed (2007) that reads the game's data from your own install:
Altaïr's original animations, climbing, free running and the cities. No game files are included.

It is not a port of the original code. Assets come straight from the game's `.forge` archives; movement is rebuilt
in Rust from how the game behaves and from AC1's own move graph, the table that says which animation leads to which.

## Requirements
- Assassin's Creed (PC, Director's Cut), installed. The default path is the Steam one; set `AC1_GAME_DIR` if yours
  is elsewhere.
- Rust (stable).

## Running
    cargo run --release -p ac1                    # greybox test level
    AC1_LEVEL=damascus cargo run --release -p ac1 # a city from the game (also masyaf; or run_damascus.bat)

## Controls
| Keyboard / mouse | Controller (Xbox / PS) | Action |
|---|---|---|
| WASD, mouse | left stick, right stick | move, camera |
| Right mouse (held) | R1 / R2 | high profile |
| Space | A / Cross | legs: jump, climb, free run (held in high profile) |
| Shift | B / Circle | empty hand: push, pickpocket, let go |
| Q | Y / Triangle | Eagle Vision |
| P | Start / Options | free camera |
| F9 | Back / Share | flight recorder: saves the last 10 s for bug reports |

## Progress
Movement first: combat is removed for now. The default scene is a greybox test level; Damascus and Masyaf load
whole from the install.

| Area | State |
|---|---|
| Game data | Working. `.forge` archives, meshes, textures, skeletons, animations (22212/22228 clips), the move graph, collision shapes, ragdolls and cloth all read from the install |
| Cities | Mostly working. Damascus and Masyaf load textured with collision; holds come from the buildings' geometry; haystacks, ladders, benches, swing poles and viewpoints are found by name. Acre, Jerusalem and Kingdom are unchecked; hay carts draw at the wrong scale |
| Ground: walk, jog, run, sprint, starts, stops, turns | Working. Turns on the spot follow the stick and hand straight into the gait |
| Jumps, falls and landings | Mostly working. Fall damage heights are estimates, not the game's values |
| Climbing: walls, ledges, free hang, corners, climb jumps, pull-ups, pull-downs | Working. Some city walls lack holds the original has |
| Wall runs | Working, including at an angle. The sideways wall run is procedural (AC1 ships no clips for it) |
| Beams, posts and two-step combos | Working |
| Ladders | Working, in both profiles, with the rebound and moving across to and from the wall |
| Swing bars and kiosk monkey bars | Working. Free running swings through a row hands-free |
| Haystacks, benches, leap of faith | Working, including from Damascus viewpoints |
| Low obstacles | Working. Free running steps or jumps onto them; otherwise Altaïr stops against them or glances off |
| Animation: blending, foot and hand IK, ragdoll, robe | Working. The robe is our own cloth simulation, not AC1's |
| Crowds | Partial. Scholar groups to blend into |
| Not started | Combat, swimming, missions, NPC behaviour |

Technical notes (formats, research, the move graph, how each system works) are in [`docs/NOTES.md`](docs/NOTES.md).
`AC1_SCENARIOS=1 cargo test -p ac1 --test scenarios -- --nocapture` runs the scripted movement tests (needs the game).

## Credits
- [AC1-Movement-Rewritten](https://github.com/Banned445/AC1-Movement-Rewritten) by Banned445: a sister project; we
  borrow from each other.

## License
MIT, see `LICENSE`. This repository contains no game files: you need your own copy of Assassin's Creed.
