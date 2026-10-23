# ac1-rs: instructions for OpenCode (and any other non-Claude agent)

You share this project with Claude Code, which has worked on it since 2026-10-04 and still does. You read this file
instead of `CLAUDE.md`, so the first thing you do in every session is:

1. Read `CLAUDE.md` in full. Its hard rules, layout, commands, branches and conventions apply to you word for word.
2. Read `F:/Claude/ac1-wt/COWORK.md`, the shared claims board and handoff log. Claude reads it too.
3. Before touching an area, read its doc: `docs/NOTES.md` (formats, status table), `docs/PARKOUR.md` (movement, what
   AC1 does and what ours does, with exe addresses), `docs/SUBSYSTEMS.md` (the exe's subsystems), and
   `docs/COMBAT.md` / `docs/PIKVR.md` when they're relevant.
4. Project skills are in `.claude/skills/*/SKILL.md` (`rust-recomp-practices`, `bevy-port-architecture`,
   `ida-reverse-engineering`; the RE tool is Ghidra, never IDA). Read the one that matches your task.

Claude's longer-term notes (the user's preferences, setups, history) are in
`C:/Users/Rias Gremory/.claude/projects/F--Claude-ac1-rs/memory/*.md`, indexed by `MEMORY.md`. Read them for context,
but don't edit them. Write your own notes in COWORK.md instead.

## Working next to Claude without colliding

**Where you work**
- `F:/Claude/ac1-rs` is Claude's checkout, on `dev`. It often holds uncommitted work. Never edit, build in, commit
  from, check out or reset that folder. Read-only `git -C F:/Claude/ac1-rs log/diff/status` is fine.
- Work in your own worktree, off `dev`:
  `git -C F:/Claude/ac1-rs worktree add F:/Claude/ac1-wt/oc-<name> -b oc/<name> dev`.
  Use branches named `oc/<name>` so everyone can see the work is yours.
- Set your own target dir on every cargo command: `CARGO_TARGET_DIR=F:/Claude/ac1-wt/target-oc`. `target/agent` is
  Claude's and `target/release` is the user's: never use either of them.
- Don't touch other worktrees (`git worktree list`): `fix/cities` is on hold, `feature/combat3` is paused, and
  `fix/grabdata` is old work.

**Git**
- Commit only on your `oc/<name>` branch. Don't push, don't merge into `dev` or `main`, and don't tag. `dev` is
  checked out in Claude's folder, so it gets merged there: when a branch is ready, mark it "ready to merge" in
  COWORK.md with the tags you ran, and either the user or Claude merges it (`--no-ff`).
- To pick up newer `dev` work, run `git merge dev` inside your own worktree.
- NEVER use `git stash`. Worktrees share one stash list, so you would pop someone else's work.
- Never `worktree remove`, `reset --hard` or `branch -D` anything you didn't create. If you remove your own worktree,
  copy its `out/` folder first (it's git-ignored and would be lost).
- Commit messages follow dev's style: one long plain-words sentence saying what changed and why (see `git log`), then
  your own attribution line.

**Claims**: before starting a task, add a row to the claims table in COWORK.md (who, what, files/areas, branch), and
check that Claude hasn't already claimed the same area. Two agents in the same file means merge conflicts, and
`crates/game/src/climb.rs` and `crates/game/tests/scenarios.rs` are the hot spots. Remove your row when you're done.
Claude does the same.

**Shared resources (one user at a time, claim them in COWORK.md)**
- The real AC1, driven through `crates/hook` (`ac1-hook.cmd` beside the game exe in
  `P:/SteamLibrary/steamapps/common/Assassins Creed`). Only one agent may drive it. Back up
  `%AppData%/Ubisoft/Assassin's Creed/Saved Games` before a session. Never use New Game, "replay memory" or any menu
  that overwrites the save. Its keys only land while its window is focused.
- The Ghidra MCP server at `http://127.0.0.1:8099` (headless, project `F:/Claude/re/mcp/ac1.gpr`) is read-only for
  you: no renames, retypes or `save_program`. Don't restart it. Port 8089 belongs to the user's AMP panel. Git Bash
  mangles `/AssassinsCreed_Dx9.exe` arguments.
- CPU: 6 cores, shared. Use `AC1_SCENARIO_JOBS=3` at most, and run one scenario run or cargo build of your own at a
  time.
- The user's `target/release` (what `run_damascus.bat` and `run_masyaf.bat` start) gets rebuilt only after a merge into
  `dev`, by whoever merged.

**The user's standing rules**
- When the user is gaming (COWORK.md says so, or they tell you): run no game windows at all, scenarios headless only,
  builds at BelowNormal priority, and send no `key`/`cursor` to the real AC1. A windowed ac1-rs stole focus from their
  fullscreen game.
- Otherwise, any windowed ac1-rs run sets `AC1_BACKGROUND=1`, which puts it on the second monitor.
- Keep each file's line endings. Most files are LF; `docs/NOTES.md` is CRLF. In Python, use `newline=''`. Check
  `git diff --stat` before committing: a few-line change must not show thousands of lines.
- Never use the IDA install at `F:/Claude/tools/ida` or the `.i64` made with it, because it's a cracked copy. All RE
  goes through Ghidra.
- No decompiled code goes in the repo, only facts (addresses, constants, behaviour) written up in our own words and
  Rust. Banned445's port (MIT, cloned under `F:/Claude/research/`) may be borrowed from with credit in the commit, but
  leave out anything in it that is decompiled code.
- Test with only the scenario tags your change affects (CLAUDE.md "Commands"). Run the full suite only for shared core
  code. Headless runs are deterministic: a failure is real, so don't rerun it hoping it passes.
- Every behaviour fix gets a scenario. User bug reports are flight recordings (F9) in `F:/Claude/ac1-wt/reports/`.
- The user plays to judge fixes. Say what to try and where (city, spot, keys).

## Where the project stands (2026-10-16)

- `main` is stable. `dev` (7813d93) is ahead: live comparisons against the running AC1 (tools/live-compare.sh plays a
  pad file into the real AC1 and ours, then `livelog compare`), Damascus play-test fixes, the exe's frame mapped
  (docs/SUBSYSTEMS.md).
- Claude's uncommitted work in its checkout (claimed, don't duplicate it): swing bars as AC1 drives them (the catch, the
  cycle, the swing jump, the stops' sides; climb.rs), bureau climb pad scenarios, roof_follow, live-compare.sh changes.
- Open work, roughly in the user's order:
  1. Parkour faithful to AC1, driven by the move graph (`forge::graph::MoveGraph`) and AC1's tables. See docs/PARKOUR.md
     "Open questions" and "How far ours follows AC1's tables". Live comparisons against the real AC1 decide what's right.
  2. Climbing with real hold data. Open leads: the climb stance's hips 0.08 m high, head 0.19 m out, AC1's hand goals
     0.1 m above our hold lines.
  3. Level editor recordings to look at (`target/release/ac1-recording-494925/514651/547826`), but only when the user
     asks. NPCs with routes in the level editor come later.
  4. Combat rebuilt on `HumanGround_Fight*` once movement is right (`feature/combat3` is paused).
  5. Robe from AC1's `DynamicMesh`/`ClothActionSettings`, sound for Ogg-page tracks and speech, Lucy's 64-bit quats.
  6. On hold until the user says otherwise: more cities (Acre, Jerusalem, Kingdom), motion matching for townsfolk.
- Don't start work the user hasn't asked for. If unsure, ask the user. Don't ask Claude: it can only read COWORK.md
  when its own session runs.
