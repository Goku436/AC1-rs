# How AC1's fights work, and what ac1-rs does with them

Combat came back on 2026-10-13, rebuilt on AC1's own data: the fight action blocks of `Human_ActionKit` and the
executable's fight tables (AssassinsCreed_Dx9.exe v1.02, read in Ghidra). The code is `crates/game/src/combat.rs`
(Bevy) and `crates/game/src/fight_data.rs` (the tables and choices, pure, unit-tested). No decompiled code is kept:
the tables are the game's numbers, the logic is written fresh.

Marks as in docs/PARKOUR.md: **[V]** checked by us in the exe or the data, **[ours]** our own (not found in AC1), **[?]**
not known yet.

## 1. The fight's data
- **Blocks** (`Human_ActionKit`, DataPC `Game Fix`; `actions <game> <block>` prints one) **[V]**:
  `HumanWeapon` (22 actions: draw and sheathe by weapon, the hidden blade out / release / retract, fists),
  `HumanGround_Fight` (578: the sword's stance, walks, attacks, combos, blocks, deflects, hits, grabs, turns, steps),
  `HumanGround_Fight_Light` (the mobile attacks, the knife's steps), `HumanGround_ComboKill` (48: the paired combo
  kills), `HumanGround_CounterKills` (206: paired counter kills, sword and short blade), `HumanGround_CounterDodge`,
  `HumanGround_CounterStrong`, `HumanGround_Fight_Hurt` (137: deaths, falls), `HumanGround_Fight_Unarmed` (180),
  `Human_Ground_Fight_Dagger` (328), `HumanGround_Assassination` (57), `HumanGround_FightEmotions`.
- **Clip families** (sword, `light`): the stance `xx_h_light_wait_<foot>` (actions 0x00FD645B / 0x0108295C: no items,
  only idle variants as associated actions), the defence `xx_h_defence_wait_<foot>` (0x1122A264 / 65), the walks in
  stance, one 8-way blend space per foot (0x0429C55B / 0x0429DBAE; the defence's 0x1122A263 / 62: front, frontright,
  right, backright, back, backleft, left, frontleft). An attack is two actions: the wind-up (`..._start`) and the
  strike action, whose items are the strike (0.2 s, the blow) and the follow-through `_miss`, with its way back
  `_miss_tr_wait` to the wait. Directions are named by where the blade goes: `ul`, `dl`, `dr`, `ur`.
- **Gate words**: every fight item may be left for standing or moving in both profiles (bits 0x40-0x800); bit 0x4000
  is set only on death items (`xx_h_fight_death_strong_*`, `countered_*_death_*_a`): read as "the body falls" **[?]**.
- **Events**: the strike clips carry a sound (the swish) and contact keys, no "hit" event: when the blow lands is code.
  `animinfo` prints a clip's events (`ANIMINFO_EVENTS=1`) and where it puts the body against its root (`ANIMINFO_BODY=1`).

## 2. The executable's tables **[V]**
Statics filled at start (initializers 0xF319A0, 0xF324E0, 0xF32E00 ...), per weapon (0 fists, 1 sword, 2 short blade):
- wind-ups by [strong, max][side front/left/right/back][distance short/medium/long][foot] (0x1A31920, 0x1A31A48);
- the strikes after them, records {action, the direction the blade ends in, 8} (setter 0xF314C0);
- combo wind-ups by [direction][strong, max][foot] (0x1A31B70) and their strikes (records at 0x1A34638: each ends in
  its own direction);
- hit reactions by [the blow's direction][quick, strong][foot] (0x1A32398), blocked recoils (0x1A322D0), block
  anticipations and receptions, deflects, turns (0x1A33B20).
The attack start (0xF3E2F0): kind 0 a first attack picked by side, distance and foot; kind 2 a combo hit; kind 3 one
at a target over 0.5 m higher or lower (the `_low_` / `_high_` attacks, not done here). The side (0xF3E090): within
45 degrees of the facing front, to 135 left or right, else back. The distance (the attack setup 0xF47760): the squared
distance against 2.25 (short) and, for the sword, 6.25 (medium, 9 for the others), else long. The combo's next
direction: after a hit 0xF4CD50's table [1, 0, 3, 0] (ul -> dl, dl -> ul, dr -> ur, ur -> ul), after a blocked strike
0xF4CDD0's [3, 2, 0, 1]; the combo stays open while fewer than 2 combo hits have played and the attack was not a max
one (0xF3B820, the flag at +0xA0C). The tag bones (0xB11020): `RightHand_Weapon_Ref` (0x3FB256E5) is where a drawn
weapon goes, `P_LeftWaist_Tag` (0x3A835926) the sword's sheath, `P_Dagger_Tag` the short blade's (names found by
CRC32 against the exe's `BIPEDBONE_*` list).

## 3. Assassinations **[V]**
- The type (0xF6C480): more than 1 m of height apart, none. High profile, not running: within 1.5 m the walking one
  (type 0 front, 1 back), to 3 m (an object's +0x224, 3 without one) the running leap (4, 5). High profile running:
  within 1 m the tackle to the ground (2, 3), else the running one. Low profile: the stealth one (10, 11) within 1.5 m
  and no further than its lead-in's distance + 0.5 (12, 13 on a state check not decoded). Back when the assassin is
  more than 90 degrees off the victim's facing. 8, 9 finish a victim lying on the ground.
- Per type (0x170A8D8..0x170A9B4): the assassin's lead-in (`_positionf_ass`, a blend space f / r / l weighted by the
  angle, 0xF6D140) and kill (`_kill_ass_footr_a`, `_b`), the victim's lead-in (`_positionf_npc_footm`) and kill
  (`_kill_npc_footm`); and how far from the victim the lead-in steers the assassin's root (0x170A8A0: 1.12, 0.407, 1,
  1, 1.656, 1.515, ... m).
- The kill (0xF6CA30) puts the assassin's root on the victim's (sub_52AAF0): the pair is authored in the victim's
  frame, both roots at its origin, the bodies apart by the Reference bones (`ANIMINFO_BODY=1`: the back kill's
  assassin 0.55 m behind the shared root).

## 4. Input (the manual, `DefaultBindings.map`) **[V]**
1 short blade and throwing knives, 2 hidden blade, 3 sword, 4 fists: a press selects, a second draws. F target lock:
"Press F to enter Fight mode ... targeting will switch automatically to the nearest enemy; F again to exit". The left
button is the weapon hand: low profile attacks ("tap a second time as soon as your weapon makes contact to unlock a
combo attack; hold for a slower but stronger attack"); high profile held defends, and with the right timing the weapon
hand counter-attacks, the legs counter-dodge, the empty hand counter-grabs. Low profile Space steps, Shift grabs.

## 5. What ac1-rs does
- **Weapons**: the keys as the manual; the sword drawn with 0x0C1BB807 (`xx_h_unsheath_grab_upper_body`, then
  `xx_h_unsheath_upper_body`) and sheathed with 0x0C1BB7D6 (`xx_h_sheath_upper_body`, `_release_upper_body`), over
  the legs; the weapon changes bone between the two items (ours: AC1's moment not found); in the hand the sheath bone
  is posed at `RightHand_Weapon_Ref` (`combat::hold_weapon`). The short blade with its `knife` pair; fists
  `xx_h_unarmed_select_<foot>`; the hidden blade has nothing to draw **[ours]**.
- **Fight mode**: F, on the nearest guard within 10 m (hostile first), the sword drawn if it is not; in through
  `xx_h_wait_hipm_<foot>_tr_xx_h_light_wait_<foot>`, out through `xx_h_light_wait_<foot>_tr_xx_h_wait_hipm_<foot>`. In
  it the stick walks the 8-way blend space's nearest walk (no mix of two) facing the target, turning at 6 rad/s **[ours:
  AC1 turns with its 45 / 90 / 180 degree clips]**; high profile the defence's wait and walk.
- **Attacks**: AC1's table pick; strong on a tap, max when held 0.3 s **[ours: the hold]**. The wind-up's root is
  steered along its own progress so its strike ends 0.95 m from the target **[ours: AC1 steers its discrete moves,
  sub_711130, the place for an attack not found]**. The blow lands as the strike item starts **[ours]**: the target
  blocks, is hit (the reaction by the blow's direction, quick for strong attacks, strong for max and combo hits), or
  dies (`xx_h_light_death_wound_front_<sl|sr>_<foot>`, the side from the blade's direction **[ours]**). A hit or
  blocked strike returns to the stance at its end; a miss plays the follow-through and its way back.
- **Combos**: the weapon hand pressed while the strike item plays (the landed blow, 0.2 s) **[ours: the manual's
  "as soon as your weapon makes contact"]** queues the next hit, AC1's direction table, up to 2 combo hits (AC1's
  rule); a press during the wind-up spoils it **[ours]**. The combo hit that would kill is the paired combo kill
  (`HumanGround_ComboKill` light_01..04 by the combo's direction **[ours: which one AC1 picks]**): the killer's wind-up
  and step in (`_start`, `_position`), then `_a` / `_b` against the victim's `death_pose`, `death_a`, `death_b`.
- **Counter**: high profile held, the weapon hand from 0.4 s before a guard's blow lands to 0.12 s after **[ours]**:
  `xx_h_light_counter_light_01_kill_<dir>_step*` then `_position_*_a` / `_b` against
  `xx_h_light_countered_light_01_death_<dir>_footr_a` / `_b`, `<dir>` the guard's strike's. Out of the window nothing
  happens **[ours: AC1 leaves the player open]**.
- **Blocking**: the player defending in the stance blocks a guard's blow (`xx_h_light_block_<max|strong>_rec_<dir>_
  <foot>`, its `_tr_wait`).
- **Assassinations**: the hidden blade selected, the weapon hand: AC1's type choice over the guards not fighting;
  the assassin's lead-in blend by AC1's weights, its root steered so that its last frame's body meets the kill's first
  (ours: AC1 steers to the victim minus its table distance; the table is used for the choice); the victim's lead-in,
  then both kills from the victim's root, the victim limp after.
- **Paired clips**: both sides play from the victim's root. AC1 moves the killer's root there; ours keeps the root
  where it is and moves the body by the offset (`Character::body_offset`), eased out over the step after the last
  paired one, so neither the body nor the root jumps **[ours]**.
- **Deaths**: limp when the death clip ends (`ik::ragdoll` on AC1's `human_ragdoll`); AC1's 0x4000 item bit is not
  used yet.
- **Guards** **[ours: AC1's fight AI, `FightManager` / `NPCFighterType` / the `GoalID_Fight*` goals, is not decoded]**:
  AC1's militia (`MMMA_FreeMission_Militia`, its clip set `mmaa_anim_set`) with the player's sword on its hip (the
  guards' own weapons are separate entities, not read). It stands until the player locks on it, attacks it, or comes
  within 4 m in front of it with a weapon drawn; then it fights: faces the player, attacks every 2.2-3.8 s within 2.6 m
  with AC1's max attacks, blocks a first attack by chance (0.5; never a timed combo hit), takes 3 hits.
- **Not yet**: counter dodges and grabs, deflects and guard breaks, the low / high attacks (kind 3), mobile attacks,
  the short blade's and fists' fights, throwing knives, air assassinations, detection and awareness, the player's
  death, more than one guard attacking in turns.

## 6. Scenarios (tag `fight`)
Draw and sheathe; an attack and two combo hits on a passive dummy; the paired combo kill; a blocked attack; the
counter kill; the player blocking; the stealth back assassination and the running one. Scripts: `AC1_GUARDS`,
`AC1_GUARD_BLOCK`, `AC1_GUARD_PASSIVE`, `AC1_GUARD_HP`, `AC1_WEAPON`, `AC1_LOCK`, `AC1_ATTACK` (`combo`, `counter`
cues press inside the windows), see `combat.rs`'s header.
