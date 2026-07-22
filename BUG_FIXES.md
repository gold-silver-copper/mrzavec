# Intentional bug fixes

## Adjacent-room corridor connectivity

`passages.c::conn` performs its lateral corridor turn only inside the primary
distance loop. When two room walls are adjacent (`distance == 0`) but their
random endpoints differ on the other axis, the loop never runs and the source
prints `warning, connectivity problem on this level`, leaving the rooms
disconnected.

The Rust port follows the same endpoint and turn selection, then bridges only a
remaining non-adjacent final gap. `generation::tests::generated_levels_are_connected`
checks the resulting invariant across seeded levels.

## Random movement onto a disguised Xeroc

`move.c::rndmove` decides whether a random monster step is passable from the
rendered screen character. A disguised Xeroc therefore looks like an object and
is accepted, unlike every visible monster. `chase.c::relocate` then clears the
moving monster's old `moat` entry and overwrites the Xeroc's entry at the new
coordinate, leaving two monster nodes at one position while only one remains
addressable through the map.

The Rust port rejects every monster-occupied destination for random movement,
including disguised Xerocs. This preserves the surrounding one-monster-per-cell
invariant and the deterministic chase path's explicit Xeroc exclusion without
adding a player-visible mechanic. The regression test is
`game::tests::random_monster_move_does_not_overwrite_a_disguised_xeroc`.

## Missing terse direction prompt

`misc.c::get_dir` assigns `"direction: "` to its local `prompt` pointer in
terse mode but, unlike the verbose branch, never passes that prompt to `msg`
before waiting for input. The same function does print the terse prompt after
an invalid direction, showing that the missing initial call is accidental and
otherwise leaves a valid command apparently unresponsive.

The Rust input state machine displays `direction: ` before the first terse
direction key, just as it displays `which direction? ` in verbose mode. The
regression test is
`main::tests::direction_prompts_preserve_the_reference_forms_with_the_terse_typo_fixed`.

## Wizard ascent below dungeon level zero

The master `CTRL-A` branch in `command.c` decrements the signed global `level`
without a lower-bound check and immediately calls `new_level`. Repeating it at
level zero creates negative dungeon levels, even though ordinary ascent treats
zero as the terminal surface and generation code uses `level` as a probability
and random-range input. This violates the surrounding nonnegative depth
invariant and produces nonsensical status and generation values.

The Rust master command permits level zero for source-compatible wizard
inspection but clamps further ascent at zero. The regression test is
`game::tests::wizard_up_can_generate_level_zero_without_winning`.

Platform adaptations are described in `PORTING_NOTES.md` and are not gameplay
changes.

## AI fixes vs the initial port (2026-07-22)

- **Monsters sometimes never woke.** The port rolled the mean-monster wake
  chance once at room entry (plus per-turn within lamp radius); the
  reference re-rolls every turn for every *visible* monster (`look(true)`
  runs before each command; `wake_monster` gives sleeping mean monsters a
  fresh `rnd(3)!=0` chance per sighting). ~1/3 of room monsters therefore
  slept permanently until approached, and teleporting into a room woke
  nobody. `wake_nearby_monsters` now includes every monster in the shared
  LIT room each turn (dark rooms/corridors keep the lamp-radius adjacency),
  restoring reference probabilities and covering teleports.
- **Monsters froze in doorways.** Chase routing resolved door tiles
  passage-first, so a monster standing in a doorway with the player inside
  the adjacent room targeted "the nearest passage exit" — the door under
  its own feet — and never moved. Routing now uses room-first areas
  (`chase_area`, mirroring `do_chase`'s `t_room`/`roomin`) with the door
  clause's exit union (`goto over` kept `mindist` across room + passage
  exits). Regression tests cover both (`door_stander_targets_player_inside
  _the_room`, `lit_room_mean_monsters_wake_within_a_few_turns`).
