# Picking combat back up

1. Ask the user whether movement is ready for combat; if not, stop at reading and planning.
2. Read docs/COMBAT.md, then the `HumanGround_Fight*` blocks: `cargo run --release -p forge --bin actions -- "<game>"
   HumanGround_Fight...` (list the block names from the bin with no block argument).
3. `feature/combat3` is paused in its own worktree: `git worktree list`, read its log and diff against `dev` before
   deciding to continue it or restart on a fresh `feature/combat` off `dev`.
4. Each fight move: the action and its blend from the block, timings from the exe (Malik), a `fight`-tagged scenario.
5. COMBAT.md says which table drives what, as PARKOUR.md does for movement.
