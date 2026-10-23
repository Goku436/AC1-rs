---
name: live-ac1
description: Drive the real Assassin's Creed (2007) through ac1-hook to measure what it does - shots, where, tp, pad recording and playback, the live log, and tools/live-compare.sh against ac1-rs. Use whenever a question is "what does the real game do", for a live comparison, for recording a pad file, or before trusting a number nobody has measured.
---

# The real AC1, driven

## 0. Allowed right now?
Read `F:/Claude/ac1-wt/COWORK.md`:
- User status "Gaming: YES": **stop**. No keys, cursor or game windows. Say what you'd measure and wait.
- "Real AC1 (hook)" row: claim it (your name, date) before driving; free it when done. One driver at a time.
Then back up the save: copy `%APPDATA%/Ubisoft/Assassin's Creed/Saved Games` to a dated folder in the scratchpad.
Never a menu that overwrites the save: new game, "Repetir / recargar memoria", "¿jugar de nuevo?".

## 1. Launch
The game is at `P:/SteamLibrary/steamapps/common/Assassins Creed`; the hook is its `dinput8.dll` (needs
`AssassinsCreed_Dx9.exe`, not Dx10). Use `F:/Claude/ac1-wt/AC1 windowed.bat` (main monitor, 1280x720 at 320,150 via
`ac1-hook.window`; `__COMPAT_LAYER=RunAsInvoker`). Menus are Spanish: Enter at the title; profile `goku436` is one Down
then Enter (it shows "Esperando", Enter again); CONTINUAR; in the Animus "Continuar". **Take a `shot` before every
Enter in a menu.** The game only takes keys while its window is active: the user may have to click it into focus.
Don't restore/refocus a minimized AC1 from a script (that crashed it).

## 2. Commands
Write one per line to `ac1-hook.cmd` beside the exe; results append to `ac1-hook.log` (read the new tail).
`shot`, `where` (position, heading, context), `tp x y z yaw` (also sets the ground's heading), `key`, `mouse`, `look`,
`trace`, `log`, `live on|off` (the live logger), `padrec <file>` / `padplay <file>` (pad files, `ac1lab pad v1`).
`ac1-hook.status` changes every frame while the game draws: use it to tell it's alive.
A teleport keeps the context: if he's hanging, let go (empty hand) and put him down before the next run.

## 3. Live comparison
`tools/live-compare.sh <pad file> [out dir]` plays the same pad file into the real game and into ac1-rs (headless,
`target/release/ac1.exe` unless `AC1_EXE`) at once and writes `compare.txt` (`livelog compare`). The real game must
be loaded into the pad file's city. Results go in docs/PARKOUR.md's live comparison sections and, when a number
becomes a check, a pad-file scenario (Lucy's `pad-scenario.md`).

## 4. Done
`live off`, free the COWORK.md row, delete `ac1-hook.window` if you changed it from the user's layout, leave the game
as you found it (or closed, if you launched it and the user isn't playing).
