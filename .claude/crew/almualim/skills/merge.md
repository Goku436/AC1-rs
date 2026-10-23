# Merging and releasing

## A branch into dev (`fix/*`, `oc/*`, `feature/*`)
1. COWORK.md "Ready to merge" row (or the member's word): which tags it ran.
2. In `F:/Claude/ac1-rs` on `dev`: `git status` must be clean of unrelated work, or the merge waits (no stash).
3. `git merge --no-ff <branch>`; rerun fmt, clippy and the tags it touches here (target/agent).
4. Remove its COWORK.md rows, add a handoff line; delete the branch/worktree only if ours (copy its `out/` first).

## A commit on dev
`git status` + `git diff --stat` (line endings: a whole-file diff means CRLF/LF flipped); nothing from `out/`, no game
files. Message: one long plain-words sentence of what changed and why (see `git log`), then the attribution lines.
Push `dev` when the user wants it pushed.

## dev into main
Only after: the full suite green on dev, a release build, and the user happy with a play-test. Then
`git checkout main && git merge --no-ff dev`, and a tag (`git tag -a vX.Y.Z -m "..."`) only when asked. Back to `dev`.

## After
COWORK.md, the pipeline page (move its board items), AGENTS.md's "where the project stands" if something big landed.
