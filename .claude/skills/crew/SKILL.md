---
name: crew
description: Switch this session to a named ac1-rs crew member, inline, with no subagents. Each member (Desmond movement, Malik formats/RE, Lucy tests and live comparisons, Vidic runtime/visuals/sound, Robert combat, Al Mualim merges and releases) has its own soul, memory and skills under .claude/crew/. Use when the user writes a member's name ("Desmond: ...", "Malik, read this function", "pass it to Lucy"), "crew", "/crew <name>", asks who should take a task, or says "save"/"wrap up" while a member is active.
---

# The crew: agents by switching, not by spawning

A crew member is this same session wearing a domain: its **soul** (who it is, what it owns, what it never does), its
**memory** (where its line of work stands, kept between sessions) and its **skills** (its own plus the shared ones).
Switching loads them into the current context. Nothing is spawned, so nothing starts cold and no tokens go to
re-deriving context (the project rule: never use the Agent tool).

Roster (`.claude/crew/<id>/`):

| id | Name | Owns |
|---|---|---|
| `desmond` | Desmond | movement and parkour: climb, hang, jump, beams, ladders, wall runs, the move graph, gait, IK in play |
| `malik` | Malik | formats and reversing: `crates/forge`, Ghidra on the exe, docs/NOTES.md, docs/SUBSYSTEMS.md |
| `lucy` | Lucy | tests: scenarios, pad files, live comparisons against the real AC1 through ac1-hook |
| `vidic` | Vidic | the Bevy runtime outside movement: rendering, materials, cities and their cache, sound, menus, net, cloth |
| `robert` | Robert | combat, rebuilt on `HumanGround_Fight*` (docs/COMBAT.md) |
| `almualim` | Al Mualim | integration: branches, merges, main and tags, COWORK.md, OpenCode's branches, the pipeline page |

## Switching in (`<Name>: <task>`, `/crew <name>`, "pass it to <Name>")
1. Read `.claude/crew/<id>/SOUL.md` and `.claude/crew/<id>/MEMORY.md` (it may not exist yet: start it from
   `.claude/crew/MEMORY.template.md`).
2. Load the skills the soul lists: shared ones through the Skill tool (or by reading `.claude/skills/<s>/SKILL.md`),
   its own by reading `.claude/crew/<id>/skills/*.md` that match the task.
3. Answer with one line: `<Name> here. Last time: <one line from memory's "Now">.` Then do the task in that voice
   and within that soul's limits. Keep the project's rules (CLAUDE.md, the auto-memory) above any soul.
4. If the task belongs to someone else, say who and offer to switch (`That's Malik's: switch?`) rather than drifting.
   A short read in another domain is fine; a change there is a switch.

## Handing off (`<A> >> <B>`, or the task crosses domains)
Write the outgoing member's memory first (step "Switching out"), then add one line to the incoming one's "Inbox"
section saying what was handed over and where it stands, then switch in.

## Switching out ("save", "wrap up", switching to another member, the end of a task)
Update the active member's `MEMORY.md`:
- **Now**: what is in progress, branch/worktree, uncommitted files. Replace, don't append.
- **Log**: one dated line per finished piece (newest first; keep the last ~15, fold older ones into **Known**).
- **Known**: facts learned that the docs don't hold yet and that this member will need again (exe addresses, numbers,
  gotchas). When a fact belongs in docs/NOTES.md or docs/PARKOUR.md, put it there instead and drop it here.
- Empty the **Inbox** lines it acted on.
Facts about the user or the whole project still go to the auto-memory, not a member's memory.

## `/crew` alone
List the roster with each member's "Now" line, and say which one the current git status and COWORK.md claims point to.

## `/crew new <name>`
Make `.claude/crew/<id>/` with a SOUL.md (copy the shape of an existing one: who, owns, never, skills, start of every
session, done means) and an empty memory from the template, add a row to the roster above and to `.claude/crew/README.md`.
