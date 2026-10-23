---
name: scenarios
description: Run the ac1-rs scenario suite (the game played by scripts, headless) the fast way - pick tags from the files touched, run once, read pop lines, batch full runs. Use after any change to crates/game, before calling a fix done, before a commit or merge, or when the user says "run the tests", "check it", "is it green".
---

# Scenarios, the fast way

The suite is deterministic (headless, every frame 1/60 s, 6 games at once, ~1.5 min for all of it). A failure is real:
never rerun hoping it passes.

## 1. Pick tags from what changed
`git diff --name-only` (plus untracked), then:

| Touched | Tags |
|---|---|
| climb.rs, climb_sweep.rs, climb_table.rs, ledge_grid.rs | climb,hang |
| jump.rs, jump_query.rs | jump,hang,beam |
| beam_pairs.rs, beam code | beam |
| gait.rs, ground code in character.rs | ground,vault,wallrun |
| wall run code | wallrun,eject |
| ladders / bars / hay | ladder / bars / hay |
| ragdoll.rs, crates/ik ragdoll | ragdoll |
| combat.rs, fight_* | fight |
| a Masyaf-only path | masyaf |
| move_blend.rs, animation.rs (mixing), the capsule, level loading, level_cache.rs | **everything** |
| docs, tools, forge-only code that the game doesn't load differently | none (forge tests instead) |

Add `combo` when the change crosses moves (the gauntlet's routes). List them all:
`AC1_SCENARIO_LIST=1 cargo test -p ac1 --test scenarios scenario_tags -- --nocapture`.

## 2. Run once (agent build folder, never target/release)
```
CARGO_TARGET_DIR=target/agent AC1_SCENARIOS=1 AC1_SCENARIO_TAGS=climb,hang \
  cargo test --release -p ac1 --test scenarios -- --nocapture 2>&1 | tail -60
```
One scenario by name: `AC1_SCENARIO_FILTER=<text>`. Run it in the background when other work can go on meanwhile.
While the user is gaming (COWORK.md), nothing windowed: no `AC1_SCENARIO_WINDOWED` / `AC1_SCENARIO_SHOTS`.

## 3. Read it
- Every scenario prints its result; failures say which check and the numbers.
- `pop:` lines are pose pops (AC1_POP_CHECK=1 is always on): one over 0.4 m fails the scenario; smaller ones are worth
  a look if the change was about blends.
- A failure in a tag you didn't expect to touch means the change reaches further than thought: read why before
  widening anything.

## 4. Batch
Quick tags after each fix; the full suite once every 3-4 fixes, then one commit for the batch. If the full run fails,
bisect the fixes one at a time. Before a merge into `main`: the full suite, always.

## 5. Before calling it done
`cargo fmt --check`, `cargo clippy -- -D warnings` (target/agent), and the tags green. Update the pipeline page's
scenario count when scenarios were added or removed. When the user should try it: rebuild `target/release`.
