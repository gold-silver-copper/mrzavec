# Porting notes

The C source in `../rogue5.4` is the behavioral authority.

| C source | Rust module |
| --- | --- |
| `main.c`, `init.c`, `extern.c`, `options.c` | `game`, `item`, `monster`, and startup/options state in `main.rs` |
| `rooms.c`, `passages.c`, `new_level.c` | `generation`, `map`, `game` |
| `command.c`, `move.c`, `misc.c` | `command`, `game`, and prompt state machines in `main.rs` |
| `things.c`, `pack.c`, `armor.c`, `weapons.c`, `rings.c` | `item`, `player`, `game`, and inventory views in `main.rs` |
| `potions.c`, `scrolls.c`, `sticks.c` | `item`, `effects`, `game` |
| `monsters.c`, `chase.c`, `fight.c` | `monster`, `combat`, `game` |
| `daemon.c`, `daemons.c` | `effects`, `game` |
| `rip.c`, `common.c`, `scmisc.c`, `scedit.c` | `score`, `game`, and end screens in `main.rs` |
| `save.c`, `state.c` | `save` and serializable core types |
| `io.c`, `mach_dep.c`, `mdport.c` | Bevy application, launcher, signals, messages, and glyph rendering in `main.rs` |

The map uses explicit terrain enums instead of character-plus-bit-mask cells;
horizontal and vertical secret-door variants retain the wall glyph that the C
cell stored while its `F_REAL` bit was clear.
Stable IDs replace C list-node pointer identity for equipment. The RNG is a
fixed, serializable xorshift64* stream; this intentionally makes seeded Rust
games portable rather than reproducing a platform libc random stream.

The C message accumulator and `endmsg` behavior become a serial-numbered core
message history plus a nonblocking Bevy-side sentence stream. Consecutive
events receive normalized punctuation and spacing, wrap at word boundaries
across the three newest visible rows, and never stop at `--More--`; an
associated one-line prompt is rendered after the events while remaining
immediately actionable. Display-only
capitalization leaves the raw recall buffer unchanged. Terminal tabs and
modal/help limits are expanded into a fixed 80×26 virtual terminal before
rendering: three event rows, 22 dungeon rows, and one status row. A separate
responsive Bevy action dock supplies ordinary context actions and modal
controls without consuming or overwriting terminal rows. Its complete command
palette is categorized from structured `HELP_ENTRIES` metadata and excludes
only non-command explanation rows and keyboard-only run variants. The dock and
terminal share one screen-layout calculation: the interactive rail follows the
rendered map width, normal gameplay remains one stable row, and prompt rows are
added only when their controls cannot fit. Context and command palettes are
map-aligned overlays, so opening them does not change terminal scale or pointer
mapping. Explicitly paginated menus and modal views retain their Space-driven
`--More--` behavior.

Pointer input is a deliberate extension beyond Rogue 5.4.5. Bevy mouse and
touch events share the terminal's calculated origin and scale on native and
WASM; there is no DOM input layer. Real Bevy buttons in the map-aligned dock
remain at least 44 logical pixels high while the 80-column terminal scales
independently. The ordinary dock ranks location and inventory actions using
shared `HELP_ENTRIES` importance and priority metadata. Urgent inverse-video
actions lead at the far left, followed by progressively less relevant actions;
Search and Wait are ordinary low-priority utilities. Capacity is calculated
from the rendered terminal width, with the final two columns permanently
reserved for `Možnosti` and
`Komandy…`. Commands that do not fit move, without duplication, into the
paginated `Možnosti` chooser; the complete category palette remains reachable
through the far-right `Komandy…` control. Both palettes use breadcrumbs and
fixed navigation positions and intercept pointer input without moving the map
underneath them. Dock buttons arm on pointer-down but inject an original
command character only after release inside the same control; leaving the
control cancels the action. They never call simulation handlers directly.
Item and option choices use paginated, map-aligned Bevy overlays with 44-pixel
targets while preserving their existing prompt-state and command-injection
paths. Direction, answer, ring-hand, pagination, and cancel controls replace
the ordinary dock during prompts. Direction prompts also
retain their remembered map overlay and map-relative selection. Core-owned tap
travel recomputes an eight-direction BFS over already-seen passable cells and
executes one ordinary move per step. It cancels on a second tap, damage, traps,
blocking UI, displaced/blocked movement, or a newly visible monster. Original
keyboard commands, including both automatic-run families, are unchanged.

The three historical inventory display strategies are explicit presentation
states. Clear-screen is the default on the Bevy display (which always supports
clearing to end of line); overwrite and slow modes retain their different
pagination, prompts, RNG-shuffled discovery output, and message-recall side
effects. Action-specific item selection intentionally replaces C `get_item`'s
letter-first, `*`-to-list prompt with an immediate filtered menu. Its displayed
pack letters remain directly actionable; throw and zap select the item before
requesting a direction.

Signed integers preserve master-mode negative gold. A separate `in_pack` flag
safely represents the C master power command's out-of-pack equipment pointer
state when the pack is full. Arbitrary master-created object glyphs are retained, as
is `total_winner`'s reuse of the previous item's value for object types that
have no switch arm in the C source.

The safe Rust-native JSON save is versioned and atomically renamed. Restore
deserializes from the same validated open regular file, rejects symbolic links
and multiple hard links for normal games, rechecks file identity before
unlinking, and consumes a normal save before play resumes. As in the master
build, wizard saves remain reusable and may be hard-linked. Historical
encrypted C-save compatibility is not reproduced.

The WASM build retains the same JSON codec behind a key/value storage boundary.
Browser saves use versioned `localStorage` keys derived from logical option
slots. A normal entry is removed only after successful decoding, while wizard
entries remain reusable. Corrupt or incompatible entries are retained, and
storage denial or quota errors are displayed rather than treated as success.
The browser score list uses the same pure one-best-nonwinner ranking code, but
is local to the browser profile; no shared leaderboard is implied.

Hallucination glyphs are simulation state rather than frame-time randomness.
The `visuals` daemon consumes the seeded gameplay RNG once for each eligible
object, unknown stair, and visible or detected monster after a turn, stores the
chosen glyphs, and Bevy renders that stored snapshot without advancing RNG.
Those glyph snapshots are included in the versioned save schema.

The native launcher preserves the reference `-V`, `-h`, `-s`, `-d`, ignored
`-r`, accepted `-S`, empty-argument wizard gate, positional restore, and
`$ROGOSEED`/wizard `$SEED` interfaces. A safe handler covers the operating
system's interrupt/terminate/hangup signals: it only sets an atomic flag, then
the Bevy main thread performs the reference automatic save. With `-S`, the
same signal instead records the original untaxed death by `signal`. Deliberate
handling of process-fault signals such as segmentation violations is omitted;
attempting Rust serialization after memory corruption would not be safe.

The browser launcher has no CLI, native environment, signal, or path model. It
uses a browser-safe `player` name and the logical save/score slots `default` and
`local`, consumes a saved normal game at startup, and otherwise begins a new
seeded game. The host page supplies a responsive `#mrzavec` canvas whose
explicit parent size is independent of the canvas child. Bevy fits its window
to that parent after CSS safe-area insets have removed unusable browser space.
The fixed 80×26 terminal scales uniformly into the remaining area, and its
rendered width and origin directly determine the dock rail. Native uses the
same layout in a resizable window.
Explicit `S` remains the authoritative save-and-stop path. No unload or
visibility checkpoint is created because browser lifecycle delivery is not
reliable and a reusable background checkpoint would enable save scumming.

The local JSON score table uses the configured player name as its identity for
Rogue's one-best-nonwinner rule; the historical table used the Unix UID.

The original Ctrl-Z terminal job-control command has no direct equivalent in a
Bevy window. It is accepted as a free command and reports that suspension is
unavailable. The `!` shell escape remains available and uses `$SHELL`, falling
back to `/bin/sh`.

In WASM, both job control and the `!` shell escape report that the operation is
unavailable. Native shell behavior is unchanged.

The `flush` option is retained in saves and the option editor, but its
terminal-typeahead behavior is naturally inert because Bevy receives discrete
key presses. Plain `hjklyubn` movement and `.` waiting supply application-level
key repeat after 300 ms and then every 100 ms, consistently across native and
browser builds; prompts, modifiers, and blocked input reset it. A complete run
command is likewise simulated between rendered frames, so `jump` cannot change
intermediate window refreshes; it still has the
reference gameplay-RNG effect of suppressing hallucination `visuals` redraws
while a run continues.

The final parity audit found no player-visible gameplay behavior that could not
be compared conclusively with the local Rogue 5.4.5 source. The intentional
platform adaptations above and the four narrowly scoped defects retained in
`BUG_FIXES.md` account for every known difference.
