# Task: Translate mrzavec completely from English to Interslavic

You are working in `/Users/kisaczka/Desktop/code/rogue-rs/mrzavec`, a Rust/Bevy rewrite of Rogue 5.4.5. Your job is to translate **every user-facing string** into Interslavic (medžuslovjansky), with grammatically correct case, number, gender, animacy, and verb aspect — not a word-for-word literal swap. The English original lives on as the separate `rogue-rs` repo, so you may change mrzavec's strings and string-producing code freely and permanently.

Use **standard Interslavic flavored Latin orthography** throughout (`ě ę ų å ȯ č š ž ć đ ń ľ ŕ ť ď ś ź`), matching the orthography of the two support tools below. Record every word decision in a `GLOSSARY.md` and every sanctioned word in a machine-readable project lexicon (`game-lexicon.tsv`) — these two files, plus a check-text CI gate, are the quality backbone of the whole job.

## Your two support tools

### 1. `interslavic` crate (0.9.0) — runtime declension/conjugation engine

Published on crates.io; depend on it normally:

```toml
# mrzavec/Cargo.toml
interslavic = "0.9.0"
```

Pure Rust (rlib), wasm-safe. **0.9.0 exposes free functions at the crate root** (the older `ISV::` struct facade is gone). The local reference checkout is `/Users/kisaczka/Desktop/code/interslavic-rs` if you need to read the source.

```rust
use interslavic::{noun, noun_with, adj, verb, verb_with_present_hint,
                  noun_forms, verb_forms, pronoun, numeral, preposition_cases,
                  Case, Number, Gender, Animacy, Person, Tense};

// Enums: Case{Nom,Acc,Gen,Loc,Dat,Ins}, Number{Singular,Plural},
//        Gender{Masculine,Feminine,Neuter}, Animacy{Animate,Inanimate},
//        Person{First,Second,Third}, Tense{Present,Imperfect,Future,Perfect,Pluperfect,Conditional}

noun_with("mųž", Case::Acc, Number::Singular,
          Gender::Masculine, Animacy::Animate)          // == "mųža" — PREFER THIS over noun()
adj("dobry", Case::Gen, Number::Singular,
    Gender::Masculine, Animacy::Animate)                // == "dobrogo"; adjectives are pure rules
verb("pisati", Person::Third, Number::Singular,
     Gender::Masculine, Tense::Present)                 // == "piše"
verb_with_present_hint("bolěti", "(boli)", ...)         // when slovowiki gives a present-stem hint
noun_forms("žena").get(Case::Gen, Number::Singular)     // full cached paradigm
verb_forms("lěčiti").gerund                             // "lěčeńje" — verbal nouns for "X of healing"
```

Hard rules:

- **Always pass explicit `Gender` + `Animacy`** (`noun_with`, `adj`) using metadata from slovowiki. For out-of-vocabulary words (including your coinages) the engine *guesses* from the ending and can be wrong — `coin-check` shows you the guess and the divergence (below).
- Nouns go in as **Nom-sg lemma**, adjectives as **Nom-sg-masc** (`dobry`), verbs as the **infinitive** (`pisati`).
- Some results pack alternatives into one string: `"oči / očesa"`. **Split on `" / "` and take the first variant** in one shared helper.
- The library declines *single words only* — no agreement, no phrase assembly. You decline adjective and noun separately into the same case/number/gender/animacy and `format!` them together.
- Compound past tenses (`Tense::Perfect`) need the subject's gender — see "player gender" below.
- Cache `NounParadigm`/`AdjParadigm` per lemma in your lexicon module.

### 2. slovowiki — lookup, coinage validation, and the verification gate

Local checkout: `/Users/kisaczka/Desktop/code/slovowiki` (pin: record `git rev-parse --short HEAD` — currently `bf041ca` — in `GLOSSARY.md` so you know which data release validated your work). Build once: `cargo build --release`; the binary is `./target/release/interslavic-wiktionary-lab`. The static API under `site/api/` is exported and current; **read `site/api/agent-guide.md` first — it is the manual** (artifact-per-task table, lookup protocols, trust rules, workflows).

You will use four subcommands, in this order of life-cycle:

**a) `en --batch` — build the lexicon.** One English query per line (`#` comments and blanks skipped):

```bash
./target/release/interslavic-wiktionary-lab en --batch words.txt --json
```

Per query: `status` (`verified`/`generated`/`miss`), `best_verified` and `best_generated` candidates (lemma, pos, gloss, aspect + `aspect_partners`, `frequency`, `langs`, `branch_pattern`, `borrowed`, warnings with severity, the ladder step and key that matched), and a `sense_note` when the top verified hit matched only inside a longer gloss. Summary counts include `sense_notes` — **read the gloss of every sense-noted result**; "staff" famously top-ranks `načeľnik štaba` (chief-of-staff) while the right words sit lower. The repo already contains this game's full vocabulary as a tracked probe: `tools/translation-probe.txt` (219 words; current coverage 147 verified / 44 generated-only / 28 miss) — start your lexicon from a batch run over it. Single-word form: `en <query>` walks the same retry ladder (de-suffixing, per-content-word) and prints candidates with the sense note.

**b) `coin-check` — validate coinages and hand them to the lexicon.** For the ~8 unavoidable fantasy names (jabberwock, xeroc, aquator…) and any other word you must invent:

```bash
./target/release/interslavic-wiktionary-lab coin-check žabervok \
    --pos noun --gender m --animacy anim --gloss jabberwock --lexicon-row --json
```

Four axes: phonotactics (bigrams attested in official lemmas — `jabberwok` fails on `w`, `bb`, `rw`, `wo`; respell until PASS), collision with existing lemmas/forms, false-friend readings across ten Slavic languages, and the **declension preview for your declared metadata** (it also prints the crate's ending-based guess and flags divergence — e.g. "ending suggests inanimate; you declared animate" — so you know exactly what `noun_with` will produce in-game). `--json` carries `lexicon_row`: append it verbatim to `game-lexicon.tsv`.

**c) The project lexicon — `game-lexicon.tsv`.** TSV, one row per sanctioned word: `lemma  pos  gender  animacy  gloss` (gender/animacy blank for non-nouns). Two kinds of rows: every coinage (via `--lexicon-row`), and **official words you pin** to lock one choice per concept (sword→`meč`, not any synonym). The consistency check reads the gloss column, so fill it with the English source concept.

**d) `check-text --lexicon` — the CI gate.** Run over rendered game text:

```bash
./target/release/interslavic-wiktionary-lab check-text rendered.txt \
    --lexicon game-lexicon.tsv --summary --max-unknown 0 --json
```

Tokens classify as `known-lemma` / `known-form` / `project` (your coinages **and their inflections** — paradigms are built from your declared metadata) / `generated` / `unknown`. You also get morphological `analyses` per token (verify a form really is the accusative you meant), false-friend warnings with severity, conservative `agreement` errors (adjacent adjective–noun case/number/gender, preposition government), and `consistency` warnings when text uses a verification-grade official word whose gloss overlaps a lexicon row but whose lemma differs — the register-drift catcher (`--max-consistency N` to gate it). Exit code is nonzero on gate failure: **wire this into mrzavec's test suite** (render every message template with sample nouns → run the gate). ASCII input is accepted (folded matching), so terminal-captured text works.

Trust rules (from the agent guide): `verified-official` / `verified-official-only` candidates are dictionary words; `generated-review` are algorithmic reconstructions with calibrated `probability` (≤0.396) — usable, but flag them in `GLOSSARY.md` as unverified. A missing key means "unknown to slovowiki", not "wrong". Warnings quote real divergent senses (exact-surface collisions first); when a warning's `prefer` list is non-empty it names dictionary-bridged alternatives — take them seriously.

## Word choice: reasoning rules

slovowiki proposes; you decide. For every word that matters:

1. **Read the gloss** — reject sense mismatches; on a `sense_note` or a gloss-token match, re-query synonyms until the sense is right (wand: query `scepter` → `žezlo`; staff: the right words are `posoh`/`ščap`, not the top hit).
2. **Weigh the evidence fields**: prefer high `frequency`, more `langs`, all-branch `branch_pattern` (`V+Z+J`); be suspicious of `borrowed` when a native word exists.
3. **Pick verb aspect consciously**: completed events ("you hit it", "the scroll turned to dust") take the perfective partner from `aspect_partners`; ongoing states take imperfective.
4. **Use your own Slavic knowledge** for register and connotation — but never override the data silently: dissenting choices go in `GLOSSARY.md` with reasoning, and the chosen word must exist in slovowiki or be a coin-checked coinage.
5. **Never modify forms.** Surface forms come from exactly two machines: the `interslavic` crate at runtime and slovowiki's records at build time. No hand-edited spellings, endings, or diacritics; no invented inflections. If the machinery can't produce what a sentence needs, choose a different word or restructure the sentence.
6. For exotic flavor vocabulary (gemstones, woods — the biggest miss category), **substitute rather than translate**: pick materials with well-attested Slavic words (amber→`jantaŕ` style). The tables are arbitrary; distinctness is all that matters.

## What has to be translated (map of the codebase)

No i18n infrastructure exists — every string is an inline literal. ~550–700 distinct strings:

| Where | What |
|---|---|
| `src/item.rs` | Canonical item names: `POTION_NAMES`(14), `SCROLL_NAMES`(18), `RING_NAMES`(14), `STICK_NAMES`(14), `WEAPON_NAMES`(9), `ARMOR_NAMES`(8) |
| `src/monster.rs` `MONSTERS` | 26 monster names |
| `src/game.rs` L18–27 | `TRAP_NAMES` (8, with English articles baked in — remove articles) |
| `src/game.rs` `Appearances` L119–280 | Unidentified-item vocab: 27 `COLORS`, 26 `STONES`, 33 `WOOD`, 22 `METAL`, ~145 `SYLLABLES` |
| `src/game.rs` (~130 `self.message(...)` sites) | Nearly all gameplay messages, death causes, `item_name`/`inventory_name`/`monster_message_name` builders |
| `src/main.rs` | Keybinding footer (L32–34), status line (`status_text` L3077, hunger words L3078), `HELP_ENTRIES` (60+, L3308), `OPTION_LABELS` (L3443), inventory/discovery screens, prompts, death/win/quit screens, tombstone ASCII art (L695), version string |
| `src/score.rs` `format`/`reason_text` | "Top 10 Rogueists" table, `killed`/`quit`/`A total winner` |

Do **not** translate: dice notation (`"2x4"`), storage/save keys, JSON field names, internal option keys (machine names in `OPTION_LABELS` tuples), log/debug strings, keystroke characters.

## The core architectural problem — and the required refactor

Today, name helpers return one fixed English string spliced into any sentence slot:

```rust
monster_message_name(idx) -> "the bat"   // used as subject, object, AND "by ..." object
format!("you hit {defender}")            // needs ACCUSATIVE in Interslavic
format!("you are frozen by the {name}")  // needs INSTRUMENTAL
format!("{name}'s gaze has confused you")// needs GENITIVE
```

That cannot work in a case language. Do this instead:

1. **Create a lexicon module** (e.g. `src/lang.rs`) mirroring `game-lexicon.tsv`:

   ```rust
   pub struct Lex { pub lemma: &'static str, pub gender: Gender, pub animacy: Animacy }
   // bat: Lex { lemma: "netopyŕ", gender: Masculine, animacy: Animate }
   ```

   Gender/animacy come **from slovowiki candidates** (and for coinages, from your coin-check declarations), never from guessing. Helpers: `fn decl(lex, case, number) -> String` (wrapping `noun_with` + first-variant splitting + paradigm caching) and `fn adj_for(lex, adj_lemma, case, number) -> String` for agreeing adjectives.

2. **Make the name-producing choke points take a grammatical role.** `monster_message_name`, `item_name`, `inventory_name`, `monster_killer` must accept `(Case, Number)`. Then fix every call site to request the case its sentence governs. Splice-site inventory (from the English code):
   - Accusative objects: `"you hit {defender}"` (game.rs ~L2384), `"you missed {}"`, `"you have defeated {}"` (L3901)
   - Nominative subjects: `attack_hit_message`/`attack_miss_message` (L3837–3888) — the monster is subject and *you* are object; both slots change roles
   - Instrumental after passive "by": `"you are frozen by the {}"` (L4461), `"you are hit by the {}"` (L2718) — instrumental, or `od` + Gen where more natural (`preposition_cases` gives government; check-text verifies it)
   - Genitive possessor: `"{name}'s gaze has confused you"` → `"pogled {Gen} ..."`
   - Two-noun sentences: `"the {weapon} hits {defender}"` (L2382) — weapon Nom, monster Acc, verb agrees with the weapon's gender/number
   - Death causes (`monster_killer` L1012, `"Killed by {}"` in main.rs) → instrumental or `od` + genitive; the article-stripping in `death_cause_with_article`/tombstone (main.rs L707–753) becomes dead code
3. **Item-name composition** (`item_name`/`inventory_name`, game.rs L1948/L2035) is itself templated (`"{color} potion"`, `"potion of {effect}"`, `"{count} scrolls"`):
   - `"X of Y"` → head noun + **genitive** (use `verb_forms(...).gerund` for effect nouns: *potion of healing* → head + `lěčeńja`) or an official derived adjective. Decide per item; stay consistent.
   - Colors/materials become **agreeing adjectives** (via `adj`) or `<material> + Gen` phrases; store them as adjective lemmas in the lexicon. Affects `pick_color` hallucination swaps and predicative uses (`"your {name} glows {color}"`).
4. **Counts** (`inventory_name`): replace `+"s"` with numeral government — 1 → Nom sg; 2–4 → Nom pl; 5+ → **Gen pl** (agreeing verb neuter singular). One `fn counted(n, lex) -> String` helper.
5. **Delete article machinery**: the three a/an vowel checks (game.rs L2037, L1012; main.rs L753), the hardcoded `"this scroll is an {} scroll"` (L1431), and articles inside `TRAP_NAMES`.
6. **Player gender**: keep player-directed messages in 2nd-person **present** where the original does; where past is unavoidable add `player_gender` to `Options` (default `Masculine`, user-editable like `fruit`).
7. **Keep, adapted**: render-time capitalization (`message_display_text` main.rs L80), `uppercase_first`; hallucination swap and xeroc-reveal (game.rs L3800, L3813) work unchanged once names flow through the case-aware helpers.

## Translation content guidelines

- **Scroll `SYLLABLES`**: gibberish "magic language" — replace with Slavic-flavored syllables (`zdra`, `mir`, `vlk`, `grom`, `pŕst`, `sněg`…), keep the table ~same size.
- **Monster names**: real creatures via the batch lookup (bat→`netopyŕ`, dragon→`drakon`); for the fantasy ones run the coin-check loop until all four axes pass (worked example: `jabberwok` fails phonotactics → `žabervok` passes; `kserok`, `akvator` pass as-is), declare gender/animacy explicitly, `--lexicon-row` into the lexicon, mark *coined* in `GLOSSARY.md`. Keep the original display glyphs — gameplay identity lives in the glyph, so names need not preserve A–Z initials.
- **Fruit**: replace default `"slime-mold"` (game.rs L331) with something Slavic and declinable (e.g. `sliva`); it is counted and spliced (`"{count} {fruit}s"`, `"...tastes like {} juice"` → genitive). It's user-editable, so fall back to plain `noun()` for arbitrary user input.
- **UI chrome**: translate labels, keep key characters (`h/j/k/l`, `i`, `q`…). The 80-column grid is fixed — verify every translated footer/status/help line fits; abbreviate like the original does.
- **Tombstone / score / end screens**: translate prose; re-center the ASCII tombstone for new word lengths (main.rs L695–744).
- **Register**: informal 2nd-person singular (`ty`), matching classic Rogue.
- **Verify orthography rendering early**: put `ě/ų/č` in the welcome message and run the game (native + `web/`) before translating everything.

## Suggested execution order

1. **Wire up**: add `interslavic = "0.9.0"`; smoke-test `noun_with("meč", Case::Acc, Number::Singular, Gender::Masculine, Animacy::Inanimate)`; verify a diacritic renders in-game; build slovowiki release binary; record the slovowiki commit in `GLOSSARY.md`.
2. **Build the lexicon**: `en --batch` over `slovowiki/tools/translation-probe.txt` (it *is* this game's vocabulary) plus the verbs your templates need; apply the reasoning rules; coin-check the fantasy names; produce `game-lexicon.tsv` (pinned official words + coinages) and `GLOSSARY.md` (every decision + rationale + trust status). Write declension tests (`assert_eq!(decl(BAT, Acc, Sg), "netopyŕa")`).
3. **Refactor the choke points** to case-aware signatures while messages are still English — compiling and green first; then template translation is mechanical.
4. **Translate messages** file by file: `game.rs`, then `main.rs` UI, then `score.rs`. At each splice slot decide the case the *Interslavic* sentence governs (often not what the English suggests); choose aspect per event semantics.
5. **Fix tests as you go** (game.rs/main.rs assert English text) so `cargo test` stays green per file.
6. **Gate**: add a test that renders every message template with a masculine-animate, feminine, and neuter noun plus samples of every screen, writes the text out, and runs `check-text --lexicon game-lexicon.tsv --summary --max-unknown 0` (start `--max-consistency` ungated, tighten later). Eyeball the rendered samples; play a full game natively and in the web build.

Work incrementally; keep the game compiling and playable after every phase. `GLOSSARY.md` + `game-lexicon.tsv` + the check-text gate are the contract that keeps 600 strings coherent — every noun, verb, and adjective choice goes through them.
