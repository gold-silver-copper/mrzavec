# Task: Add an Interslavic "about Rogue" and "how to play" section to the game page

You are working in `/Users/kisaczka/Desktop/code/rogue-rs/mrzavec`. The game is
deployed to https://grift.rs/mrzavec/ from `web/index.html` on every merge to
`main`. Your job is to extend that page — below the canvas — with two prose
sections, **written entirely in Interslavic** (standard flavored Latin
orthography: `ě ę ų å ȯ č š ž ć đ ń ľ ŕ ť ď ś ź`):

1. **«Čto jest Rogue?»** — what the game Rogue is and a short history.
2. **«Kako igrati»** — gameplay, mechanics, the command keys, and how to win.

Read `TRANSLATION_PROMPT.md` first: its support-tool protocols (slovowiki
lookup ladder, trust rules, coin-check) and word-choice reasoning rules apply
verbatim here. This document only adds what is specific to the web page.

## Sources of truth — never invent facts

- **Game facts come from the code.** Win condition: `AMULET_LEVEL` in
  `src/lib.rs` + the `has_amulet` logic in `src/game.rs` (fetch the amulet at
  depth ≥ 26, then climb back up and out). Mechanics: hunger (`status_text`,
  hunger states in `src/main.rs`), armor/weapons/potions/scrolls/rings/wands
  (`src/item.rs`), traps (`TRAP_NAMES`), searching, resting, identification.
  Describe only mechanics the port actually has.
- **The command table comes from `HELP_ENTRIES` and `MOVEMENT_KEYS` in
  `src/main.rs`.** Every key you list must exist there, and the Interslavic
  wording for each command must match the in-game help text exactly — the page
  and the `?` screen must never disagree. Do not list every key; pick the
  ~20 a new player needs (movement, inventory, quaff/read/eat/wield/wear,
  throw/zap, search, rest, stairs, save, help), and say that `?` shows the rest.
- **History section facts** (keep it to one short paragraph, ~5 sentences):
  Rogue was created around 1980 by Michael Toy and Glenn Wichman at UC Santa
  Cruz, later with Ken Arnold at Berkeley; it used the curses terminal library
  and spread with BSD Unix; its procedurally generated dungeons and permanent
  death founded the genre that carries its name ("roguelike"). Close with one
  sentence: mrzavec is a faithful Rust rewrite of Rogue 5.4.5 in Interslavic.
  Verify these claims against at least one independent source before
  translating; do not add further claims.

## Vocabulary rules

- **Game concepts must reuse the game's own words.** `src/lang.rs` /
  `GLOSSARY.md` are the authority: amulet, dungeon, monster, potion, scroll,
  ring, wand, armor, weapon, gold, trap, hunger, level — every one of these
  already has a sanctioned lemma. Introducing a synonym for an existing
  concept is a defect (check-text's consistency warnings will catch it; the
  gate below runs with `--lexicon`).
- **Page-only vocabulary** (history words: university, terminal, genre, …)
  goes through the slovowiki `en` lookup protocol with the usual trust rules.
  Record decisions in a new `## Webpage` section of `GLOSSARY.md`, and pin
  every chosen lemma in a new hand-maintained `web/page-lexicon.tsv` (same
  columns as `game-lexicon.tsv`; that file itself stays generated-only —
  never edit it). Coinages (unlikely — expect zero) go through `coin-check`.
- **No hand-inflected guesses.** The page is static text, so unlike game
  source it necessarily contains surface forms in oblique cases — but every
  such form must be *produced* by machinery, not typed from memory: dump the
  needed paradigms with the `interslavic` crate (add a small
  `examples/page_forms.rs` in the spirit of `examples/prep_cases.rs`, run it,
  copy forms out verbatim) or take forms from slovowiki records. If neither
  machine can produce what a sentence needs, restructure the sentence.

## The gate — wire it in, don't run it once

Add `scripts/extract_page_text.py`: parse `web/index.html`, emit all
human-visible Interslavic text (element text, `aria-label`, the status
strings inside the `<script>` block) to stdout; skip key letters, digits,
`__BUILD__`, URLs, and code identifiers. Then add a stage to
`scripts/check_lang.sh` that pipes that text through slovowiki `check-text`
with `game-lexicon.tsv` + `web/page-lexicon.tsv` merged, `--summary
--max-unknown 0`. The page must pass with zero unknowns and no agreement or
government errors, and future page edits stay gated forever.

## Page mechanics (don't break the deploy)

- Keep the existing script logic intact: the `__BUILD__` cache-busting
  stamp, `autofocus`/`canvas.focus()` calls, and the loading/error status
  flow. While you are there, move the remaining English strings into
  Interslavic too: `<html lang>` becomes `isv`, the `aria-label`, the
  "Loading the game…" / post-init hint / "Unable to start" strings.
- Style the new sections to match the page (dark background, same fonts,
  `max-width` aligned with the canvas column, command keys in `<kbd>` or
  monospace). No external resources; the page must stay self-contained.
- The English draft you translate from is a working artifact — it does not
  ship. Only Interslavic text lands on the page.

## Verification workflow (all four, in order)

1. **Machine gate**: `scripts/check_lang.sh` passes end-to-end, including the
   new page stage.
2. **Blind back-translation**: give the final Interslavic page text — alone,
   with no English draft — to a fresh reviewer (agent) and have it translate
   back to English. Diff the meaning against your draft; investigate every
   divergence (wrong case read, wrong aspect, ambiguous sentence) and fix the
   Interslavic, not the back-translation.
3. **Consistency sweep**: every command description on the page matches the
   in-game `?` help wording; every game term matches `lang.rs`; every listed
   key exists in `HELP_ENTRIES`/`MOVEMENT_KEYS`.
4. **Render check**: `./scripts/build-wasm.sh`, serve `web/` locally, and
   confirm the page renders correctly (sections readable on desktop and
   narrow viewports, no layout break, game still loads and focuses).

Ship via branch + PR to `main`; the merge deploys. After deploy, fetch the
live page and confirm the new sections and the build stamp are present.
