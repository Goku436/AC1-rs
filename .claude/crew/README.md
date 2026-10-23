# The ac1-rs crew

Named agents for ac1-rs that run **inline in one session**: saying `Desmond: the swing bars pop` loads Desmond's soul,
memory and skills into the current context instead of spawning a subagent. The `crew` skill
(`.claude/skills/crew/SKILL.md`) does the switching and the saving.

Each member's workspace is `.claude/crew/<id>/`:
- `SOUL.md`: who it is, what it owns, what it never does, its skills, how it starts and what "done" means. Committed.
- `MEMORY.md`: where its line of work stands (Now, Inbox, Known, Log). Local and git-ignored: it holds local paths and
  half-finished state. Start one from `MEMORY.template.md`.
- `skills/*.md`: its own procedures. Shared skills are in `.claude/skills/`.

| Call | Member | Owns |
|---|---|---|
| `Desmond:` | desmond | movement and parkour |
| `Malik:` | malik | formats, reversing, Ghidra |
| `Lucy:` | lucy | scenarios, pad files, live comparisons |
| `Vidic:` | vidic | runtime, rendering, cities, sound, menus |
| `Robert:` | robert | combat |
| `Al Mualim:` | almualim | branches, merges, releases, co-work |

Hand a task over with `Desmond >> Lucy` (Desmond saves its memory and leaves Lucy a line in her inbox). `/crew` lists
everyone and where each stands; `save` writes the active member's memory.
