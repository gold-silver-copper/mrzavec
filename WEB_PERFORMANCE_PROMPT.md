# Bevy Web Performance Prompt

Improve mrzavec’s web performance using the easiest, lowest-risk wins while
keeping the renderer entirely in Bevy.

Do not replace Bevy UI/text rendering with Canvas2D, DOM, `<pre>`, or another
frontend. Do not change game rules, controls, visual layout, responsive
behavior, or native behavior.

## Primary problems to address

### 1. Stop false `State` change detection

`prepare_messages` currently calls `collect_messages(&mut state)` every frame.
Dereferencing `ResMut<State>` mutably marks the resource changed even when
there are no new messages. As a result, `render` redraws the whole terminal
continuously.

Restructure this path so a no-op frame does not mark `State` changed. Prefer an
immutable early check followed by mutable access only when work is required.
Avoid using `bypass_change_detection` to hide real mutations.

### 2. Diff terminal updates

The renderer currently rewrites all 2,080 glyphs whenever `State` changes. Add
a cached terminal buffer resource and compare the newly generated buffer with
the previous one. Only update Bevy `Text`, color, or background components for
cells whose rendered value actually changed.

Ensure the initial frame still renders fully. Avoid allocating a new
one-character `String` for unchanged cells.

### 3. Remove redundant cell background work

The terminal grid root is already black. If individual cell backgrounds are
not used for meaningful styling, remove their `BackgroundColor` components and
stop assigning black to every cell during rendering. Keep the visible result
identical.

### 4. Avoid unnecessary layout recomputation

`update_screen_layout` currently recalculates dock specifications and labels
every update. Gate this work on actual changes to the window size, `State`, or
dock state. Be careful not to create another self-sustaining change-detection
loop by mutably dereferencing `DockUi` when its value does not change.

### 5. Use an appropriate reactive Bevy update mode

This is a turn-based game, so it should not run continuously while idle.
Configure Bevy/winit to wake immediately for input and window events, with a
maximum wait of approximately 100 ms so held movement and automatic pointer
travel retain their existing cadence.

Validate held-key delay/repeat, pointer travel, touch controls, hover/press
feedback, resize behavior, and modal interactions before keeping this change.

## Constraints

- Keep Bevy and WebGL2.
- Do not undertake a renderer rewrite.
- Do not change the simulation or language code.
- Preserve the current 80×26 display and responsive action dock.
- Preserve native and WASM behavior.
- Keep changes narrowly scoped and easy to review.
- Do not modify unrelated or pre-existing untracked files.

## Verification

Add regression coverage proving:

- Two no-input updates do not mark `State` changed merely because message
  collection ran.
- Rendering an identical buffer a second time changes zero glyph components.
- A small game-state change does not update all 2,080 glyphs.
- Initial rendering, resizing, held movement, and pointer travel still work.

Run:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo check --target wasm32-unknown-unknown
cargo build --profile wasm-release --target wasm32-unknown-unknown
```

Build the web package and profile the deployed-equivalent page in Chrome.
Compare at least a five-second idle window before and after. Report:

- Main-thread `TaskDuration` and `ScriptDuration`
- Number of glyph components updated
- WASM size before and after
- Any behavior or measurement limitations

Target at least an 80% reduction in idle main-thread task time, with no visible
or gameplay regression. If one requested optimization proves unsafe, retain
the confirmed improvements and clearly document why that optimization was
omitted.
