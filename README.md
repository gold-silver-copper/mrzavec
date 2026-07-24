# Mrzavec

Mrzavec is a Rust/Bevy rewrite of Rogue 5.4.5. The game simulation is kept in
ordinary serializable Rust data structures and the Bevy application presents it
as an 80-column display with a three-row event stream, Rogue's 22-row dungeon
view, the status row, and a responsive context-sensitive action dock.

## Build and run

Stable Rust is required.

```sh
cargo run
```

### Build and embed the web version

Install the WASM target and the `wasm-bindgen` CLI version used by the lockfile,
then build the browser package:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.126 --locked
./scripts/build-wasm.sh
python3 -m http.server 8000
```

Open <http://localhost:8000/web/>. The files required for deployment are
`web/index.html` and the generated `web/pkg/` directory. They must be served
over HTTP; the server must send `.wasm` as `application/wasm`. Production
servers should also compress the WASM response with Brotli or gzip.

To embed Mrzavec in another page, create the canvas before initializing the
module:

```html
<canvas id="mrzavec" tabindex="0"></canvas>
<script type="module">
  import init from "/games/mrzavec/pkg/mrzavec.js";
  const canvas = document.querySelector("#mrzavec");
  canvas.addEventListener("pointerdown", () => canvas.focus());
  await init();
</script>
```

The Bevy app targets `#mrzavec` and prevents browser default key handling while
the game has focus. Give the canvas parent an explicit responsive width and
height. Bevy fits the canvas to that parent, scales the fixed 80×26 terminal
into the remaining area, and derives the action rail from the rendered map
width rather than the browser width. The compact dock is one row; larger prompt
sets add rows only when their controls cannot fit. Command and contextual
palettes overlay the map without rescaling it. Browser safe-area insets reduce
the usable canvas before Bevy performs this calculation. In a single-page app,
mount the canvas before importing/initializing the package and initialize it
only once.

During normal play, direct actions are ranked from most to least relevant
across the remaining width. Urgent inverse-video actions stay at the far left;
`Možnosti` is always immediately left of the far-right `Komandy…` control.
Lower-ranked actions move into `Možnosti` when the rendered terminal is too
narrow, while `Komandy…` continues to expose the complete categorized command
palette.

Ordinary gameplay events are combined into a sentence stream and wrapped over
the three rows at the top. When the stream overflows, the newest three rows stay
visible without pausing for Space. Explicitly paginated menus and modal views
still use Space for `--More--`.

The browser uses `localStorage` rather than server files. `file` and `score`
options are logical local slots (`default` and `local` initially), and the
score table is private to the current browser profile. A successful normal
restore consumes its saved slot; the most recently saved logical slot is
selected automatically at the next startup, and wizard saves remain reusable.
The explicit `S` command is the authoritative save operation and, as on native,
stops the game after success. Reload the page to restore and continue. Browser
close and reload events do not create automatic checkpoints because those
events are not reliable and doing so would change Rogue's single-use-save
rules.

Private browsing, disabled storage, corrupt data, or quota exhaustion is
reported inside the game and does not overwrite or consume an existing valid
entry. The `!` shell command reports that it is unavailable; native CLI,
signals, environment variables, filesystem locking, and path semantics do not
apply to the web build. WebGL2 is the supported compatibility path; WebGPU is
not required.

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

Movement uses Rogue's `hjklyubn` keys. Holding an unmodified movement key walks
continuously after a 300 ms delay, repeating every 100 ms; holding `.` rests at
the same cadence. Uppercase directions run and the corresponding control keys
run until something interesting is encountered.
`m` moves without picking up, `f` fights until danger, `F` fights to the death,
`.` rests, `,` picks up, `s` searches, and `^` identifies an adjacent trap.

Item commands are `q` quaff, `r` read, `e` eat, `w` wield, `W` wear armor,
`T` remove armor, `P`/`R` put on/remove a ring, `d` drop, `t` throw, `z` zap,
and `c` call an unidentified item. `i` shows the full inventory and `I` its
picky form. Item-selecting commands immediately show every eligible pack entry;
press its displayed letter or Escape to cancel. Throwing and zapping select the
item before asking for a direction. `)`, `]`, `=`, and `@` report current
equipment and statistics.

`>` descends and `<` ascends. `?` opens the complete command help immediately;
Space advances only when another help page is available, and Escape closes it.
`/` identifies a glyph, `D` lists discoveries, `o` edits options, Ctrl-P
recalls the last message, Ctrl-R redraws, and `v` reports the version. `a`
repeats the prior command, `S` saves, `Q` quits, and Escape cancels a pending
command. `!` runs the configured shell; Ctrl-Z is accepted but reports that
terminal suspension is unavailable in a window.

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
cargo check --target wasm32-unknown-unknown
cargo build --profile wasm-release --target wasm32-unknown-unknown
```

Project structure:

- `game`, `generation`, `map`, `player`, `monster`, `combat`, `item`, and
  `effects` contain the deterministic turn simulation.
- `command` defines the complete normal and master key set.
- `save` and `score` provide pure codecs/ranking plus native-file and browser
  storage integration; `platform` defines the testable browser storage seam.
- `main.rs` contains launcher/platform integration plus Bevy keyboard, prompt,
  modal, message, and 80×28 glyph-grid presentation state.
- `FEATURE_PARITY.md`, `PORTING_NOTES.md`, and `BUG_FIXES.md` record the source
  comparison and every intentional difference.

## Language tooling (Interslavic)

The game text is Interslavic; every inflected form is produced at runtime by
the [`interslavic`](https://crates.io/crates/interslavic) crate via the
`lang::speak()` template markers (see `RUNTIME_INFLECTION_PROMPT.md` and
`GLOSSARY.md`). Verification needs a sibling checkout of
[slovowiki](https://github.com/gold-silver-copper/slovowiki) (release binary
built; override the location with `SLOVOWIKI=/path`):

```sh
./scripts/check_lang.sh   # template gate (check-text) + zero-pre-inflection lint
```

`game-lexicon.tsv` is generated from `src/lang.rs` by the
`regenerate_project_lexicon` test (golden file — the test fails on drift so
the regenerated TSV must be reviewed and committed).
