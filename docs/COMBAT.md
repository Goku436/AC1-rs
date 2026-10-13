# How AC1's fights work, and what ac1-rs does with them

Combat came back on 2026-10-13, rebuilt on AC1's own data: the fight action blocks of `Human_ActionKit` and the
executable's fight tables (AssassinsCreed_Dx9.exe v1.02, read in Ghidra). The code is `crates/game/src/combat.rs`
(Bevy), `crates/game/src/fight_data.rs` (the choices, pure, unit-tested) and `crates/game/src/fight_tables.rs` (the
exe's tables as numbers). No decompiled code is kept:
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
Statics filled at start (the initializers 0xF319A0, 0xF324E0, 0xF32E00 ... and the record setters 0xF314C0 for the blades,
0xF315F0 for the fists; the master reset 0xF39B10), kept as numbers in `crates/game/src/fight_tables.rs`, per weapon
(0 fists, 1 sword, 2 short blade; the stance tables have a 4th, the hidden blade `blade`):
- the stances: waits, defence waits, defence walks, walks [weapon][foot] (0x1A31760, 0x1A31784, 0x1A317A8, 0x1A317B0).
  The light and unarmed waits list no items, only idle variants as associated actions (0xFD645B's first, 0x8C6D0D28,
  is `xx_h_light_wait_footr`);
- wind-ups by [strong, max][side front/left/right/back][distance short/medium/long][foot] (0x1A31920, 0x1A31A48) and
  the strikes after them, records {action, the direction the blade ends in, 8} (0x1A33C18 / 0x1A33F78; the fists' long
  distance repeats the medium one, the short blade's three distances are one attack);
- the sword's straight-ahead attacks at medium and long distance: [strong, max][foot][distance][blade direction]
  (0x1A31D90, strikes 0x1A31FD8): 0xF3E2F0 rolls the direction at random (0..3, moved on by one when it repeats the last,
  +0x9DC); the sword only;
- the high and low attacks (a target over 0.5 m higher or lower, kind 3): [weapon][distance][low, high][strong, max]
  [foot] (0x1A31C38, strikes 0x1A342D8 / 0x1A34488); no combo follows them (+0xA0A);
- combo wind-ups [weapon][direction][strong, max][foot] (0x1A31B70) and strikes (0x1A34638): the sword's and the short
  blade's (the short blade has no upper-left combo hit; its direction tables never turn to it);
- what the attacker plays as its strike lands: `hit` on a body (0x1A32398, 0xF4A0A0: `xx_h_light_hit_<quick|strong>_
  <dir>_<foot>_a`, 0.27 s) and `blocked` on a block (0x1A322D0, 0xF3B820: `xx_h_blocked_...`), [weapon][the blade's
  direction][strong, max][foot]. (Round 1 played the `hit` clips on the victim: they are the attacker's.)
- the combo's next direction (0x170A640..0x170A670): after a strike that landed with one of those clips, the sword's
  [3, 2, 0, 1] (0xF4CDD0 after a hit, 0xF4CE50 after a block), the others' [1, 2, 3, 1]; after one that met nothing
  that answered (0xF4CD50), the sword's [1, 0, 3, 0], the others' [1, 3, 1, 2]. (Round 1 had the two sword tables the
  wrong way round.)
- the turns on the spot [weapon][45 left, 45 right, 90 left, 90 right, 180][defence, stance][foot] (0x1A33AD0);
- the paired combo kill [weapon][the combo hit's direction]: the killer's wind-up, step in, kill (0x1A33538, 0x1A3356C,
  0x1A335A0), the victim's death pose and death indexed by 3 - the direction (0x1A335D4, 0x1A33608);
- the counter kill: step in and kill [the defender's weapon][kill, slap][the attacker's weapon][direction][the
  defender's foot] (0x1A32E10 played by 0xF4A2B0, 0x1A33118 by 0xF4A9A0): the `_01` set off the left foot, `_02` off
  the right; `slap` (`xx_h_light_counter_slap_...`) the non-lethal one;
- the counter on a max attack (0x1A3341C ...: `xx_h_light_counter_max_light_...`, the attacker `countered_max_light_...
  _stunned`, `_kicked`), the counter dodge [weapon][side left/right/back][foot] (0x1A32460), a deflected blow's recoil
  [weapon][direction][foot] (0x1A32738), the mobile attacks [weapon][kind 0 drawn, 3 drawing as it strikes, ...][foot]
  (0x1A32220, strikes 0x1A34908), the throwing knife's throw and release [the knife's place: belt, leg, shoulder]
  [side][foot] (0x1A34EF0, 0x1A34F58).
The attack start (0xF3E2F0): kind 0 a first attack picked by side, distance and foot; kind 2 a combo hit; kind 3 the
high or low one. The side (0xF3E090): within 45 degrees of the facing front, to 135 left or right, else back. The
distance (0xF47760): the squared distance against 2.25 (short) and, for the sword, 6.25 (medium, 9 for the others), else
long. The fists' max attack: a second attack within 2 s of a strong one (the timer at +0xA88). The turn (0xF4BB60, played
by 0xF38D10): none within 30 degrees, the 45 one to 60, else the 90 one to 135, else the 180, its clip's turn scaled to
the angle. The tag bones (0xB11020): `RightHand_Weapon_Ref` (0x3FB256E5) is where a drawn weapon goes,
`P_LeftWaist_Tag` (0x3A835926) the sword's sheath, `P_Dagger_Tag` the short blade's.

The attack's state machine (the fight state's sub-states at +0xB17, entered by 0xF4E6E0, left by 0xF523A0): the wind-up
(0xF3E2F0) -> the strike (0xF40460, which clears the weapon hand's press latch +0x805 and the max flag +0x804 as it
starts) -> what it meets: a body (0xF4A0A0, the `hit` clip), a block (0xF3B820, the `blocked` clip), nothing that
answers (0xF4A050). Each sets the combo flag +0xA0C unless the attack was a max one (+0xA0A), two combo hits have played
(+0x9E0) or the weapon is not a blade. When the clip playing may be left (the anim graph's flag, 0x501760), a press
latched since the strike started (+0x805, set by the attack command 0xF3A2F0, reached from `HumanGround`'s handler at
its vtable +0x158) plays the next combo hit (0xF4DD00 / 0xF4DD40 / 0xF4DD80); if the target is no longer in front, a new
first attack instead. The max attack is its own command (0xF3A3F0 via +0x154: +0x804), sent by the input interpreter
(`GoAssassinActionInterpreter`, its hold timing not found). The mobile attack is the command 0xD88BD0 (+0x808 with its
kind at +0x814).

The fight manager (`FightManager`, update 0xB00DA0) reads per memory block a `FightManagerData` (DataPC.forge,
`AssassinsCreed`'s missions: two `FightManagerDifficultyData` each, 11 fields, read by 0xCD7430..0xCD74F0): +0/+4 ramp
the fighters it lets take part (memory block 1: 1-2, block 7: 14-16; 0xAFF460 -> +0x2A8, 0xB00C30 sets the rest aside),
+8/+0xC a share of them sent at once (0..0.3) every +0x10/+0x14 seconds (60..25), +0x1C (8 s) a cool-down, +0x20/+0x24/
+0x28 counts (target switching, the fighters drawn in). Read only in part: `fight_data::FIGHT_MANAGER`.

The `DamageData` (DataPC `Game Bootstrap Settings`) is a tree of per-attack damages (u8 fields, among them `ComboKill`,
`CounterStrong`, `CounterQuick`, `Throw`, `Arrow` by their CRC32s; values 5, 10, 13, 16, 20, 30, 255), rows not decoded;
`AssassinsMoraleBank` holds the player's health per memory block (arrays of 20..53), not decoded.

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
  assassin 0.55 m behind the shared root). A paired lead-in is time-scaled to its steer (0xF4A2B0: the clip's speed set
  so it lasts the steer's time, sub_711130).
- From the air: the jump code (0xB1EC40) takes a target of kind 0x8000 with `HumanInAir`'s 0x21B4DC3D / 0x21B4DC3E, a
  9-way blend of `xx_h_air_<front|down|up>_<cm>_<foot>_to_assassinate` by the jump's length and drop. The kill after it
  is not found (the block's `xx_h_assassina_failure_air_kill_*` leaves the victim standing: the failed one).

## 4. Input (the manual, `DefaultBindings.map`) **[V]**
1 short blade and throwing knives, 2 hidden blade, 3 sword, 4 fists: a press selects, a second draws. F target lock:
"Press F to enter Fight mode ... targeting will switch automatically to the nearest enemy; F again to exit". The left
button is the weapon hand: low profile attacks ("tap a second time as soon as your weapon makes contact to unlock a
combo attack; hold for a slower but stronger attack"); high profile held defends, and with the right timing the weapon
hand counter-attacks, the legs counter-dodge, the empty hand counter-grabs. Low profile Space steps, Shift grabs.

## 5. What ac1-rs does
- **Weapons**: the keys as the manual; the sword drawn with 0x0C1BB807 and sheathed with 0x0C1BB7D6 over the legs; the
  short blade with its `knife` pair; fists `xx_h_unarmed_select_<foot>`; the hidden blade has nothing to draw **[ours]**.
  The moment the weapon changes bone **[ours]**.
- **Fight mode**: F, on the nearest guard within 10 m (hostile first), with the weapon in hand (the sword drawn if none
  is), into that weapon's stance (**[V]** tables). Standing, a target over 30 degrees off is faced with AC1's turn clips,
  their turn scaled to the angle **[V]**; walking in the stance, or within 30 degrees, the facing follows at 6 rad/s
  **[ours]**. High profile the defence's wait and walk.
- **Attacks** **[V]**: AC1's table pick by weapon, side, distance and foot; the sword's straight-ahead medium and long
  ones by a random blade direction; a target over 0.5 m higher or lower the high or low attack; the fists' second attack
  within 2 s a max one. Strong on a tap, max when held 0.3 s **[ours: the hold]**. The wind-up's root is steered so its
  strike ends 0.95 m from the target **[ours]**. The blow lands as the strike item starts **[ours: AC1's contact
  event, 0x502210, not decoded]**: the target blocks, deflects, is hurt or dies; the attacker then plays AC1's `hit` or
  `blocked` clip and its way back **[V]** (none in the table for its foot: the strike action's own way back, `_tr_wait`).
- **The victim**: hurt `xx_h_light_hurt_quick_front_light_vertical` for a strong blow, `_strong_front_light_<sl|sr>` for
  a max or combo one by the blade's side, `_strong_back_common` from behind (the fists' `unarmed_hurt_..._punch` /
  `_kick`) **[ours: AC1's choice not found]**; dies `xx_h_light_death_wound_front_<sl|sr>` **[ours]**.
- **Combos** **[V]**: AC1's latch: a press from the strike's start through the `hit` / `blocked` clip plays the next combo
  hit when that clip ends (a press in the wind-up is dropped as the strike starts); AC1's direction tables; up to 2;
  not after a max attack; the blades only. The combo hit that would kill is AC1's paired combo kill for its direction.
  **[ours]**: no combo after a strike that met nothing (AC1 allows one, `[1, 0, 3, 0]`), and a guard never blocks a combo
  hit.
- **Counter** (high profile held, the weapon hand from 0.4 s before a guard's blow lands to 0.12 s after **[ours: the
  window]**): AC1's counter kill by the defender's foot (`_01` / `_02`) **[V]**, against `countered_light_<set>_death`
  of the guard's blade direction **[ours: AC1 indexes the defender's by 3 - an event's direction, not settled]**.
  **Counter dodge**: Space in the same window, AC1's `counter_dodge_<left|right|back>` by the stick (back with none)
  **[V clips, ours the window]**; the guard's blow then misses. Counter grabs and the counter on max attacks: not yet.
- **Paired clips** **[V]**: both sides play from the victim's root; the killer's root slides onto it over 0.3 s as the
  paired clip starts, the body held on the pair's root by an offset all along, so neither jumps **[ours: AC1 snaps
  the root and blends the pose]**.
- **Mobile attacks**: running over 4 m/s, a guard within 2.5 m and 45 degrees ahead, the weapon hand: AC1's mobile attack
  (kind 0 drawn, 3 drawing the sword as it strikes) **[V tables, ours the reach]**.
- **Throwing knives**: the short blade at a target past its medium band (3 m): AC1's belt throw and release by the
  target's side **[V clips]**, the knife flying at 25 m/s, killing a guard; 5 knives **[ours: speed, count, damage;
  no knife mesh]**.
- **Short blade and fists**: the same machinery on their tables and stances **[V]**; the fists no combos (AC1's).
- **Assassinations**: the hidden blade selected, the weapon hand: AC1's type choice over the guards not fighting, the
  lead-in blend by AC1's weights, steered so its last frame's body meets the kill **[ours: AC1 steers to the victim minus
  the table distance]**, then the pair. **From above** (a guard 1.5..7 m below within 3.5 m ahead: a ledge, a beam, a
  roof's edge, a jump): AC1's `to_assassinate` jump by the drop, stretched to the fall's time and steered through the
  air so its body lands where the kill starts, then the finish on the ground (types 8 / 9) **[ours: AC1's air kill not
  found; the victim knocked down under it]**.
- **Guards** **[ours: AC1's fight AI is the decision tree `DecisionTreeDefault` in code, not decoded]**: any entity with
  `Guard` and `Fighter` (`Guard::new(seed)`; the street militia's patrol moves one only while it is not hostile and
  `Fighter::idle`). Hostile when locked on, attacked, a drawn weapon within 4 m in front, or a guard within 15 m
  fighting (the alarm). They take turns: at most one attacks at once (`AC1_FIGHT_ATTACKERS`; the turn to the one that
  waited longest; AC1's fight manager numbers are in section 2), every 2.2-3.8 s within 2.6 m, AC1's max attacks;
  block a first attack by chance (0.5), or deflect it (`AC1_GUARD_DEFLECT`, 0 by default): `xx_h_light_deflect_<dir>`
  against the attacker's AC1 `deflected` recoil, then strike back at once; 3 hits.
- **The player hurt**: a strong blow takes 0.1 of the health, a max or combo one 0.15 **[ours: AC1's `DamageData` not
  decoded]**; at none, the wound death, limp, and desynchronised: back at the spawn with full health after 3 s
  **[ours: AC1 restarts at the memory's checkpoint]**. The HUD's health bar shows it.
- **Not yet**: counter grabs and grabs, guard breaks, the counter on a max attack, AC1's hold time for the max attack,
  its contact frame, the victim's hurt choice, the fight manager's group attacks, detection beyond the alarm, the
  throwing knife's mesh, the hidden blade's own fight stance (its tables are in), the air kill AC1 plays.

## 6. Scenarios (tag `fight`, 20)
Draw and sheathe; an attack chain with two combo hits (the attacker's `hit` clip, the victim's hurt clip); the paired
combo kill; a blocked attack; the counter kill; the player blocking; the stealth back and running assassinations; the
90 degree turn; a press in the wind-up dropped; a deflect and the riposte; the counter dodge; two guards in turns; the
player's death and desynchronisation; the mobile attack drawing the sword; the short blade's fight; the fists' fight;
the throwing knife; the high attack on the stairs; the air assassination off the gauntlet's 5 m block. Scripts:
`AC1_GUARDS`, `AC1_GUARD_BLOCK`, `AC1_GUARD_DEFLECT`, `AC1_GUARD_PASSIVE`, `AC1_GUARD_HP`, `AC1_GUARD_AWARE`,
`AC1_FIGHT_ATTACKERS`, `AC1_PLAYER_HEALTH`, `AC1_WEAPON`, `AC1_LOCK`, `AC1_ATTACK` (`combo`, `counter`, `dodge` cues),
`AC1_FIGHT_TRACE`, see `combat.rs`'s header. Screenshots: F:/Claude/ac1-wt/reports/combat2/. The pop check skips a limp
body (the ragdoll's root jumps under it); the short blade's scenario ends as its strike's way back starts (AC1's knife
sweeps the hand 0.45 m in a frame there, the clip's own motion).
