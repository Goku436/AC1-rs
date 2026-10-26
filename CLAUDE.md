# ac1-rs

From-scratch Rust runtime for Assassin's Creed (2007, PC). It reads data from the user's own install. It is **not** a port of the original code: IK, locomotion and climbing are recreations of behavior seen in-game.

Full format notes and status table are in `docs/NOTES.md` (the README is the short public page). Read it before touching a format. Keep its status table and format notes up to date when something changes.
`docs/SUBSYSTEMS.md` maps the executable's subsystems (its classes, functions and who calls whom) and the Rust layout that mirrors them; `docs/PARKOUR.md` is the movement reference.

## Hard rules
- Never commit or publish game files, extracted data, IDBs, or decompiler output. `out/` is git-ignored. Check `git status` before committing.
- Tests must not depend on game files being present in the repo. A test that needs the install must skip cleanly when it isn't found.
- Don't paste decompiled code into the repo. Use IDA or Ghidra to learn formats and constants, then write the Rust yourself.
- Before using a clip for a move, check which action block holds it (`actions <game> <block>`): the graph says what
  a clip is for (the `passover` clips looked like a vault and are a ledge move).

## Layout
- `crates/forge`: `.forge` archives, LZO1X, object container, textures, meshes, skeletons, animation (and clip mixing), action blocks (`action.rs`) and the move graph over them (`graph.rs`; example `graph_next`), collision shapes (`shape.rs`), Havok packfiles and ragdolls (`hkx.rs`), the object index (`index.rs`), sound (`sound.rs`: BAO sounds, the event graph, IMA-ADPCM, contact tables). Bins: `forge`, `catalog`, `findobj`, `animinfo` (clip duration, root motion and yaw by name), `objects` (one data file's objects, hex, or `dump` to files), `world` (a city's entities and meshes), `actions`, `shapes`, `ragdolls`, `idx`, `bodies` (a city's rigid bodies by collision layer, and the entities with none), `bao` (sound: `scan`, `wav` to out/, `clip` events). Animation notes live in the header of `src/anim.rs`.
- `crates/ik`: pure math, unit-tested: IK, the Verlet ragdoll (`ragdoll.rs`) and cloth (`cloth.rs`). No Bevy, no game data.
- `crates/game`: Bevy runtime, run with `-p ac1`. Controls and test hooks are in the header of `src/main.rs`.

## Branches
- `main`: stable. Always builds and passes the full scenario suite; what the user plays and shares. Only merged into
  from `dev` (or a finished feature), never committed to directly.
- `dev`: day-to-day work. Commit and push here; merge into `main` (`git checkout main && git merge --no-ff dev`) when a
  batch is tested (full suite green, the user happy with a play-test), then tag it.
- `fix/<name>`: one line of fixes (an audit area, a feature), each in its own worktree (`git worktree add ../ac1-wt/<name> -b fix/<name> dev`, its own `CARGO_TARGET_DIR`) so several can be worked on at once; merged into `dev` (`--no-ff`) once its tagged scenarios pass, then deleted.
- `feature/<name>`: bigger separate lines of work off `dev`, merged back when ready. (`feature/multiplayer`, the p2p UDP
  test, was folded in by `fix/menus` on 2026-10-12: the F10 online menu, `net.rs` / `replay.rs`, `AC1_HOST=port` /
  `AC1_JOIN=ip:port`.)
- Tags `vMAJOR.MINOR.PATCH` (annotated) mark snapshots on `main` to go back to: `git checkout v0.1.0` to look,
  `git switch -c fix/<name> v0.1.0` to work from one. v0.1.0 = movement on AC1's tables, every clip and mesh parsing.

## Commands
- Run: `cargo run -p ac1` (`AC1_GAME_DIR` overrides the install path; `AC1_LEVEL=masyaf` / `damascus` loads a city)
- City cache: a city's gameplay data (collision, holds, guidance, beams, objects, navigation) is saved after its first load
  to `ac1-cache/` beside the executable (`crates/game/src/level_cache.rs`; `AC1_NO_CACHE=1` bypasses it, `AC1_CACHE_CHECK=1`
  compares a fresh build with it, `AC1_SCENARIOS=1 cargo test --release -p ac1 --test level_cache` checks both cities). Its key
  covers the game files and the source of the files that build the data; bump `level_cache::CACHE_VERSION` whenever code
  outside its `SOURCES` list changes what a city's gameplay data holds, or `Level` gains a field (`write_level` names them all)
- List an archive: `cargo run --release -p forge --bin forge -- list "<game>/DataPC_Masyaf.forge"`
- Class histogram: `cargo run --release -p forge --bin catalog -- "<game>"` (class ids are CRC32 of class names; it names them from the game's exe, see docs/NOTES.md "Class names")
- Check before finishing: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`
- Scenarios (the game played by scripts, needs the install): `AC1_SCENARIOS=1 cargo test --release -p ac1 --test scenarios -- --nocapture` (headless since 2026-10-11: no window or GPU, every frame 1/60 s of game time, 6 games at once, the whole suite in about 1.5 minutes; the same run every time, a loaded machine only slows it; `AC1_SCENARIO_WINDOWED=1` to watch, `AC1_SCENARIO_SHOTS=1` for screenshots, both at the same fixed step; `AC1_SCENARIO_FILTER=text` for some, `a|b` for either). Run only what a change can affect: `AC1_SCENARIO_TAGS=climb,hang` (tags: ground, jump, climb, hang, beam, ladder, bars, hay, vault, wallrun, ragdoll, misc, combo: the gauntlet's multi-move routes, eject: Damascus ejects measured against the running AC1, fight, masyaf: Masyaf spots); the whole suite only for shared core code (capsule, animation mixing, level loading). `AC1_SCENARIO_LIST=1 cargo test -p ac1 --test scenarios scenario_tags -- --nocapture` lists them. Every scenario runs with `AC1_POP_CHECK=1` (the recorder's live pose-pop check): pops are printed as `pop:` lines, one over 0.4 m fails

## Conventions
- Game data is Z-up. Characters face +X. Mesh space is skeleton model space turned 90° about Z. Animation clips face +Y with the Reference bone turned 90°. Convert to Bevy's Y-up only in one place, in the `game` crate.
- Format work is measured by pass rate over every file (for example 15906/15906 data files decompress). A change must not lower a pass rate. Update the docs/NOTES.md table when a number changes.
- Parsers never panic on bad data. Return errors with the file offset.
- Unknown fields stay documented as `?` with their offset until proven.

## Skills
Project skills are in `.claude/skills/`: `ida-reverse-engineering`, `rust-recomp-practices`, `bevy-port-architecture`.
Use them for format reversing, parser/runtime Rust work, and Bevy work. Also `scenarios` (which tags to run, how)
and `live-ac1` (driving the real game through ac1-hook).

## Crew
Named agents that run inline in this session, never as subagents: `Desmond:` movement, `Malik:` formats/RE,
`Lucy:` tests and live comparisons, `Vidic:` runtime/visuals/sound, `Robert:` combat, `Al Mualim:` merges and releases.
A member's name (or `/crew <name>`) loads its soul, memory and skills from `.claude/crew/<id>/` through the `crew`
skill; `A >> B` hands a task over; `save` writes the active member's memory. See `.claude/crew/README.md`.

## Open work
Driving which move plays from the move graph itself (`forge::graph::MoveGraph` is loaded and checks every chain step;
the code's action requests are found: 2035 action ids used in code, see docs/NOTES.md "What the executable's code asks for";
every movement context now picks by AC1's tables or action ids, docs/PARKOUR.md "How far ours follows AC1's tables"), the robe from AC1's own `DynamicMesh`/`ClothActionSettings`, combat (removed 2026-10-05; rebuild it
on `HumanGround_Fight*` once movement is right), the narrow "Left" list in docs/PARKOUR.md,
Acre/Jerusalem/Kingdom checks, sound for Ogg-page tracks and the speech streams. Details are in the docs/NOTES.md status table and its Research section.
