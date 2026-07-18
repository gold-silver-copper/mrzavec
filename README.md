# Mrzavec

Mrzavec is a Rust/Bevy rewrite of Rogue 5.4.5. The game simulation is kept in
ordinary serializable Rust data structures and the Bevy application presents it
as Rogue's 80-column by 24-line glyph display.

## Build and run

Stable Rust is required.

```sh
cargo run
```

Historical launcher modes are also available:

```sh
cargo run -- -V                 # print version and release
cargo run -- -h                 # print command-line help
cargo run -- -s                 # print the configured score table
cargo run -- -s other.scores    # print another score table
cargo run -- -d                 # immediate bat death and scoring
cargo run -- -r                 # accepted compatibility option
```

By default, interrupt/terminate/hangup signals automatically save and exit;
`-S` changes them to the reference untaxed death-by-signal behavior. An empty
first argument opens the master-build wizard password prompt; `$SEED` then
selects its dungeon number. A player name beginning with `rogo-` may use
`$ROGOSEED` in the same way as the reference debugging interface.

Movement uses Rogue's `hjklyubn` keys; uppercase directions run and the
corresponding control keys run until something interesting is encountered.
`m` moves without picking up, `f` fights until danger, `F` fights to the death,
`.` rests, `,` picks up, `s` searches, and `^` identifies an adjacent trap.

Item commands are `q` quaff, `r` read, `e` eat, `w` wield, `W` wear armor,
`T` remove armor, `P`/`R` put on/remove a ring, `d` drop, `t` throw, `z` zap,
and `c` call an unidentified item. `i` shows the full inventory and `I` its
picky form. `)`, `]`, `=`, and `@` report current equipment and statistics.

`>` descends and `<` ascends. `?` opens command help, `/` identifies a glyph,
`D` lists discoveries, `o` edits options, Ctrl-P recalls the last message,
Ctrl-R redraws, and `v` reports the version. `a` repeats the prior command,
`S` saves, `Q` quits, and Escape cancels a pending command. `!` runs the
configured shell; Ctrl-Z is accepted but reports that terminal suspension is
unavailable in a window.

Decimal prefixes repeat commands, capped at the historical maximum of 255.

`o` opens the editable options view, `c` calls an unidentified item, and `D`
lists discoveries. As in the historical master build, `+` prompts for the
wizard password; entering wizard mode disables score submission.
The master-build control-key commands, object creation/charging, unrestricted
identification, and power-up kit are available while wizard mode is active.

Saves default to `~/.rogue.save.json`, are written atomically, and are made
read-only. `S` confirms
the configured filename and asks again before overwriting an existing file; as
in Rogue, a successful manual save exits the game. The `file`, `score`, and
`lock` paths can be changed in the option editor or through `ROGUEOPTS`. Leading
`~` in an option string expands to the current home directory. The format is versioned
and includes the simulation RNG, map memory, actors, inventory, options, and
turn state. It is intentionally not binary-compatible with historical C saves.
Restore a saved game by passing its path:

```sh
cargo run -- ~/.rogue.save.json
```

As in the reference game, a successfully restored normal save is consumed
before play resumes, while wizard-mode saves remain reusable. Restore rejects
non-regular and symbolic-linked files, plus multiply hard-linked normal saves.
Invalid, obsolete, and unreadable saves are left in place. The restored path
becomes the default for the next manual save.

The local top-ten score table defaults to `~/.rogue.scores.json`.

Run tests and project checks with:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release
```

Project structure:

- `game`, `generation`, `map`, `player`, `monster`, `combat`, `item`, and
  `effects` contain the deterministic turn simulation.
- `command` defines the complete normal and master key set.
- `save` and `score` provide the versioned save and local score-table formats.
- `main.rs` contains launcher/platform integration plus Bevy keyboard, prompt,
  modal, message, and 80×24 glyph-grid presentation state.
- `FEATURE_PARITY.md`, `PORTING_NOTES.md`, and `BUG_FIXES.md` record the source
  comparison and every intentional difference.
