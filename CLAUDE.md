# ac1-rs

From-scratch Rust runtime for Assassin's Creed (2007, PC). It reads data from the user's own install. It is **not** a port of the original code: IK, locomotion and climbing are recreations of behavior seen in-game.

Full format notes and status table are in `README.md`. Read it before touching a format. Keep its status table and format notes up to date when something changes.

## Hard rules
- Never commit or publish game files, extracted data, IDBs, or decompiler output. `out/` is git-ignored. Check `git status` before committing.
- Tests must not depend on game files being present in the repo. A test that needs the install must skip cleanly when it isn't found.
- Don't paste decompiled code into the repo. Use IDA or Ghidra to learn formats and constants, then write the Rust yourself.
- Before using a clip for a move, check which action block holds it (`actions <game> <block>`): the graph says what
  a clip is for (the `passover` clips looked like a vault and are a ledge move).

## Layout
- `crates/forge`: `.forge` archives, LZO1X, object container, textures, meshes, skeletons, animation (and clip mixing), action blocks (`action.rs`) and the move graph over them (`graph.rs`; example `graph_next`), collision shapes (`shape.rs`), Havok packfiles and ragdolls (`hkx.rs`), the object index (`index.rs`). Bins: `forge`, `catalog`, `findobj`, `animinfo` (clip duration, root motion and yaw by name), `objects` (one data file's objects, hex, or `dump` to files), `world` (a city's entities and meshes), `actions`, `shapes`, `ragdolls`, `idx`. Animation notes live in the header of `src/anim.rs`.
- `crates/ik`: pure math, unit-tested: IK, the Verlet ragdoll (`ragdoll.rs`) and cloth (`cloth.rs`). No Bevy, no game data.
- `crates/game`: Bevy runtime, run with `-p ac1`. Controls and test hooks are in the header of `src/main.rs`.

## Commands
- Run: `cargo run -p ac1` (`AC1_GAME_DIR` overrides the install path; `AC1_LEVEL=masyaf` / `damascus` loads a city)
- List an archive: `cargo run --release -p forge --bin forge -- list "<game>/DataPC_Masyaf.forge"`
- Class histogram: `cargo run --release -p forge --bin catalog -- "<game>"` (class ids are CRC32 of class names; it names them from the game's exe, see README "Class names")
- Check before finishing: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`

## Conventions
- Game data is Z-up. Characters face +X. Mesh space is skeleton model space turned 90° about Z. Animation clips face +Y with the Reference bone turned 90°. Convert to Bevy's Y-up only in one place, in the `game` crate.
- Format work is measured by pass rate over every file (for example 9357/9373 data files decode). A change must not lower a pass rate. Update the README table when a number changes.
- Parsers never panic on bad data. Return errors with the file offset.
- Unknown fields stay documented as `?` with their offset until proven.

## Skills
Project skills are in `.claude/skills/`: `ida-reverse-engineering`, `rust-recomp-practices`, `bevy-port-architecture`.
Use them for format reversing, parser/runtime Rust work, and Bevy work.

## Open work
Driving which move plays from the move graph itself (`forge::graph::MoveGraph` is loaded and checks every chain step;
the code's action requests are found: 2035 action ids used in code, see README "What the executable's code asks for";
the ladder plays its table, the other systems' tables are next), the robe from AC1's own `DynamicMesh`/`ClothActionSettings`, combat (removed 2026-10-05; rebuild it
on `HumanGround_Fight*` once movement is right), the 11 other mesh types, real hold data for climbing,
Acre/Jerusalem/Kingdom checks, 64-bit quats and camera channels in animation, the 16 data files that fail to decode,
the 16 clips that fail to parse. Details are in the README status table and its Research section.
