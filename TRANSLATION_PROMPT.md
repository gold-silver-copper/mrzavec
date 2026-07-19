# Task: Translate mrzavec completely from English to Interslavic

You are working in `/Users/kisaczka/Desktop/code/rogue-rs/mrzavec`, a Rust/Bevy rewrite of Rogue 5.4.5. Your job is to translate **every user-facing string** into Interslavic (medžuslovjansky), with grammatically correct case, number, gender, animacy, and verb aspect — not a word-for-word literal swap. The English original lives on as the separate `rogue-rs` repo, so you may change mrzavec's strings and string-producing code freely and permanently.

Use **standard Interslavic flavored Latin orthography** throughout (`ě ę ų å ȯ č š ž ć đ ń ľ ŕ ť ď ś ź`), matching the orthography of the two support libraries below.

## Your two support libraries

### 1. `interslavic` crate — runtime declension/conjugation engine

Local checkout: `/Users/kisaczka/Desktop/code/interslavic-rs` (Cargo workspace). Add it to mrzavec as a **path dependency** (do not use crates.io — the local 0.8.0 API is authoritative):

```toml
# mrzavec/Cargo.toml
interslavic = { path = "../../interslavic-rs/crates/interslavic" }
```

It is pure Rust (rlib), so it works in the wasm build too. Everything is under `use interslavic::*;`. Key API (all single-form calls return `String`):

```rust
// Enums: Case{Nom,Acc,Gen,Loc,Dat,Ins}, Number{Singular,Plural},
//        Gender{Masculine,Feminine,Neuter}, Animacy{Animate,Inanimate},
//        Person{First,Second,Third}, Tense{Present,Imperfect,Future,Perfect,Pluperfect,Conditional}

ISV::noun("meč", Case::Acc, Number::Singular)                     // dictionary-backed lookup
ISV::noun_with("mųž", Case::Acc, Number::Singular,
               Gender::Masculine, Animacy::Animate)               // == "mųža"; explicit metadata — PREFER THIS
ISV::adj("dobry", Case::Gen, Number::Singular,
         Gender::Masculine, Animacy::Animate)                     // == "dobrogo"; adjectives are pure rules
ISV::verb("pisati", Person::Third, Number::Singular,
          Gender::Masculine, Tense::Present)                      // == "piše"
ISV::verb_with_present_hint("bolěti", "(boli)", ...)              // when the dictionary gives a present-stem hint
ISV::pronoun("toj", ...), ISV::numeral("pęť", ...)                // Option<String>, closed classes
ISV::noun_forms(lemma) -> NounParadigm                            // full paradigm; .get(case, number)
ISV::verb_forms(inf) -> VerbParadigm                              // .present/.perfect/.imperative/... vectors
```

Hard rules learned from its source and review notes:

- **Always pass explicit `Gender` + `Animacy`** (`noun_with`, `adj`) using metadata from the dictionary CSV (below). For out-of-vocabulary words the engine *guesses* from the ending and can be wrong.
- Nouns go in as **Nom-sg lemma**, adjectives as **Nom-sg-masc** (`dobry`), verbs as the **infinitive** (`pisati`).
- Some results pack alternatives into one string: `"oči / očesa"`. **Split on `" / "` and take the first variant**; do this in one shared helper so it's consistent everywhere.
- The library declines *single words only*. It does **no agreement and no phrase assembly** — you decline the adjective and the noun separately into the same case/number/gender/animacy and `format!` them together yourself.
- Compound past tenses need the subject's gender (l-participle agreement) — see "player gender" below.
- Cache `NounParadigm`/`AdjParadigm` per lemma (a `HashMap` in the lexicon module) instead of re-declining hot words every message.

### 2. slovowiki — authoritative English → Interslavic dictionary

Local checkout: `/Users/kisaczka/Desktop/code/slovowiki`. The one file that matters for word choice:

**`data/official-isv.csv`** — 18,457 rows of the *official* Interslavic dictionary. Columns:
`id,isv,addition,partOfSpeech,type,en,sameInLanguages,genesis,ru,be,uk,pl,cs,sk,sl,hr,sr,mk,bg,cu,de,nl,eo,frequency,intelligibility,using_example`

Lookup workflow for every word you translate:

1. **Find candidates**: grep column 6 (`en`) for the English gloss, e.g. `grep -inE '(^|,|")sword' data/official-isv.csv` → `164,meč,,m.,1,sword,...` → the word is `meč`. Glosses are comma-separated sense lists, so anchor your pattern and check the whole row, not just a substring hit.
2. **Extract grammar metadata from column 4 (`partOfSpeech`)** and record it in your lexicon:
   - Nouns: `m.` / `f.` / `n.` = gender; `m.anim.` = masculine **animate** (critical for accusative); `pl.`/`sg.` = number-locked; `indecl.` = indeclinable.
   - Verbs: `v.tr. ipf.`, `v.intr. pf.`, etc. — **ipf/pf is the aspect**, tr/intr/refl the valency.
   - `adj.`, `adv.`, `prep.`, `num.card.`, etc.
3. **Column 3 (`addition`)** carries the verb **present-stem hint**, e.g. `pisati (piše)` — pass it via `ISV::verb_with_present_hint`.
4. **Choosing between candidates**: prefer rows with blank `genesis` (native Slavic) over `I` (internationalism) for flavor, and higher `frequency` when synonyms compete. A `!` prefix on any cell means unverified.
5. **Check false friends** in `data/semantic-notes.json` (e.g. `čas` = time, not hour; `slovo` = word, not letter). More generally: check the `en` gloss of every word you pick — never assume a cognate means what it does in Russian/Polish/etc.

**Verification — read `site/api/agent-guide.md` first; it is the manual for this.** Summary of its rules:

- **Verify every token** of your drafted Interslavic text (lexicon words *and* rendered message samples) with `cd /Users/kisaczka/Desktop/code/slovowiki && cargo run --release -- check-text <file> --json`. It classifies each token as known-lemma / known-inflected-form / generated / unknown, suggests nearest known forms for unknowns, applies the false-friend notes, and emits conservative `agreement` warnings (adjacent adjective–noun case/number/gender, preposition government, pronoun–verb) — exactly the mistakes a translation like this produces. `explain <word>` spot-checks a single word with a rule trace.
- Multi-word lemmas exist: reflexive verbs (`myti se`) and 2–3-token official entries (`adamovo jablȯko`, `dvojostry meč`). When verifying, try the trigram → bigram → unigram of adjacent tokens.
- **Trust rules**: only `status: official` / `official-only` (and `grammar` closed-class words) are verification-grade. `generated` entries are machine reconstructions or auto-derivatives — treat `probability < 0.6` as a suggestion, never verification; they deliberately have **no** inflection records. A token missing from the index means "unknown to Slovowiki", not "wrong" — expect this for your coined monster names.
- If you query the static form index directly (`site/api/forms/<n>.json`): fold the token per the guide's table, route with FNV-1a-32 `% 2048`, and validate your fold/router against `api/router-selftest.json` first. Analyses use Interslavic tags: `jd.`/`mn.` = sg/pl, `nom. akuz. gen. dat. lok. instr.`, `m.živ.`/`m.než.`/`ž.`/`sr.`, `prez.`, `komp.`/`superl.`. The index also contains declined participles, comparatives/superlatives, and pronoun/numeral paradigms.
- The committed `site/api/` is **schema 2** and has no `api/en/` English-lookup directory yet; the README describes schema 3. If you want the English API, run `cargo run --release -- export --out site` first — otherwise just grep the CSV as described above (that path is always reliable).

## What has to be translated (map of the codebase)

There is **no i18n infrastructure** — every string is an inline literal. The full surface, roughly 550–700 distinct strings:

| Where | What |
|---|---|
| `src/item.rs` | Canonical item names: `POTION_NAMES`(14), `SCROLL_NAMES`(18), `RING_NAMES`(14), `STICK_NAMES`(14), `WEAPON_NAMES`(9), `ARMOR_NAMES`(8) |
| `src/monster.rs` `MONSTERS` | 26 monster names |
| `src/game.rs` L18–27 | `TRAP_NAMES` (8, with English articles baked in — remove articles) |
| `src/game.rs` `Appearances` L119–280 | Unidentified-item vocab: 27 `COLORS`, 26 `STONES`, 33 `WOOD`, 22 `METAL`, ~145 `SYLLABLES` |
| `src/game.rs` (~130 `self.message(...)` sites) | Nearly all gameplay messages, death causes, `item_name`/`inventory_name`/`monster_message_name` builders |
| `src/main.rs` | Keybinding footer (L32–34), status line (`status_text` L3077, hunger words L3078), `HELP_ENTRIES` (60+, L3308), `OPTION_LABELS` (L3443), inventory/discovery screens, prompts, death/win/quit screens, tombstone ASCII art (L695), version string |
| `src/score.rs` `format`/`reason_text` | "Top 10 Rogueists" table, `killed`/`quit`/`A total winner` |

Do **not** translate: dice notation (`"2x4"`), storage/save keys, JSON field names, internal option keys (the machine names in `OPTION_LABELS` tuples), log/debug strings, keystroke characters.

## The core architectural problem — and the required refactor

Today, name helpers return one fixed English string that gets spliced into any sentence slot:

```rust
monster_message_name(idx) -> "the bat"   // then used as subject, object, AND "by ..." object
format!("you hit {defender}")            // needs ACCUSATIVE in Interslavic
format!("you are frozen by the {name}")  // needs INSTRUMENTAL
format!("{name}'s gaze has confused you")// needs GENITIVE
```

That cannot work in a case language. Do this instead:

1. **Create a lexicon module** (e.g. `src/lang.rs`). For every game noun define an entry:

   ```rust
   pub struct Lex { pub lemma: &'static str, pub gender: Gender, pub animacy: Animacy }
   // e.g. bat: Lex { lemma: "netopyŕ", gender: Masculine, animacy: Animate }
   ```

   Populate gender/animacy/aspect **from the CSV's `partOfSpeech` column**, never by guessing. Provide helpers `fn decl(lex, case, number) -> String` (wrapping `ISV::noun_with` + first-variant splitting) and `fn adj_for(lex, adj_lemma, case, number) -> String` for agreeing adjectives.

2. **Change the name-producing choke points to take a grammatical role.** `monster_message_name`, `item_name`, `inventory_name`, `monster_killer` must accept `(Case, Number)` (and internally know each noun's gender/animacy). Then fix every call site to request the case its sentence governs. The main splice-site inventory (from the English code):
   - Accusative objects: `"you hit {defender}"` (game.rs ~L2384), `"you missed {}"`, `"you have defeated {}"` (L3901)
   - Nominative subjects: `attack_hit_message`/`attack_miss_message` (L3837–3888) — here the *monster* is subject and *you* are object, so both slots change roles
   - Instrumental after passive "by": `"you are frozen by the {}"` (L4461), `"you are hit by the {}"` (L2718) — in Interslavic use the instrumental (or `od` + Gen where more natural)
   - Genitive possessor: `"{name}'s gaze has confused you"` → `"pogled {Gen} ..."`
   - Two-noun sentences: `"the {weapon} hits {defender}"` (L2382) — weapon Nom, monster Acc, and the *verb agrees with the weapon's gender/number*
   - Death causes (`monster_killer` L1012, `"Killed by {}"` in main.rs) → instrumental or `od` + genitive; also fix `death_cause_with_article` and the tombstone's article-stripping (main.rs L707–753), which become unnecessary
3. **Item-name composition** (`item_name`/`inventory_name`, game.rs L1948/L2035) is itself templated (`"{color} potion"`, `"potion of {effect}"`, `"{count} scrolls"`). Rules:
   - `"X of Y"` → head noun + **genitive** (or a derived adjective when the dictionary has one): *potion of healing* → e.g. `napitok cěljenja`. Decide per item; keep a consistent pattern.
   - Color/material descriptors become **adjectives that agree** with the head noun: a "red potion" needs the feminine/masculine/neuter adjective form matching your word for potion, in whatever case the sentence needs. Store `COLORS`/`STONES`/`WOOD`/`METAL` as adjective lemmas (or `<material> + Gen` phrases where no adjective exists) and inflect via `ISV::adj`. This also affects `pick_color` hallucination swaps and `"your {name} glows {color}"`-type messages, where the color is a *predicate adverb/short form* — use the neuter/adverbial form there.
4. **Counts** (`inventory_name` plural logic): replace English `+"s"` with Slavic numeral government —
   - 1 → Nom **singular** (with `jedin` agreeing, if you show the numeral)
   - 2–4 → Nom **plural** (dual-style agreement)
   - 5+ → **genitive plural**, and any agreeing verb goes neuter singular
   Centralize this in one `fn counted(n, lex) -> String` helper.
5. **Delete article machinery.** Interslavic has no articles: remove the three a/an vowel checks (`inventory_name`'s closure L2037, `monster_killer` L1012, `death_cause_with_article` main.rs L753), the hardcoded `"this scroll is an {} scroll"` (L1431), and strip articles from `TRAP_NAMES`.
6. **Player gender for past tense.** Compound past (`Tense::Perfect`) l-participles agree in gender. Simplest correct options: (a) keep player-directed messages in 2nd person **present** where the original does, and where past is unavoidable add a `player_gender` field to `Options` (default `Masculine`, editable in the options screen like `fruit` is); or (b) use the library's `(a)` convention. Pick (a) — it matches how the game already exposes `fruit` as an option.
7. **Keep, adapted:** the render-time capitalization rule (`message_display_text` main.rs L80) and `uppercase_first` still apply. The hallucination name-swap and xeroc-reveal logic (game.rs L3800, L3813) work unchanged once names flow through the case-aware helpers.

## Translation content guidelines

- **Scroll `SYLLABLES`** are gibberish "magic language" — don't translate; **replace** with Slavic-flavored syllables (e.g. `zdra`, `mir`, `vlk`, `grom`, `pŕst`, `sněg`...) so generated titles look right next to Interslavic text. Keep the table ~same size so title-length distribution is preserved.
- **Monster names**: real animals/beings via the CSV (bat→`netopyŕ`, dragon→`drak`/`zmij`, wolf etc.); mythological ones (jabberwock, xeroc, griffin) pick the closest official word or a phonologically Slavic coinage — but then **hand-write its gender/animacy** in the lexicon and verify declension output by printing the paradigm. Preserve the classic A–Z initial-letter mapping to display glyphs **only if feasible**; the glyph char is separate from the name in `MonsterTemplate`, so when Interslavic initials can't cover A–Z, keep the original glyphs and let names diverge — gameplay identity lives in the glyph.
- **Fruit**: change the default `"slime-mold"` (game.rs L331) to something Slavic and declinable (e.g. `sliva`). It gets counted and spliced (`"{count} {fruit}s"`, `"...tastes like {} juice"` → genitive), so run it through the same lexicon machinery; since it's *user-editable*, fall back to `ISV::noun` (dictionary lookup + ending-guess) for arbitrary user input.
- **UI chrome** (help, options, status): translate labels; keep the actual key characters (`h/j/k/l`, `i`, `q`...) untouched. The 80-column layout is fixed — check every translated footer/status/help line still fits 80 chars (Interslavic runs longer than English; abbreviate like the original does: `Ur:`/`Zl:` style for `Level:`/`Gold:` if needed).
- **Tombstone / score / end screens**: translate the prose; re-center the ASCII tombstone text for the new word lengths (main.rs L695–744).
- **Register**: address the player with informal 2nd-person singular (`ty`), matching classic Rogue's tone.
- Flavored orthography everywhere; both libraries emit it natively. Non-ASCII is fine — the renderer draws Unicode chars, but **verify early** that `ě/ų/č` render in the terminal grid and the wasm build (put one in the welcome message and run the game before translating everything).

## Suggested execution order

1. **Wire up**: add the path dependency; smoke-test `ISV::noun_with("meč", Acc, Sg, Masculine, Inanimate)` from inside mrzavec; verify a diacritic renders in-game.
2. **Build the lexicon** (`src/lang.rs`): all 111 canonical nouns (items/monsters/traps) + appearance adjectives + the verbs your message templates need, each looked up in `official-isv.csv` with gender/animacy/aspect recorded. Write it as data + a few tests asserting expected declensions (`assert_eq!(decl(BAT, Acc, Sg), "netopyŕa")`-style) so regressions are caught.
3. **Refactor the choke points** to case-aware signatures (`monster_message_name`, `item_name`, `inventory_name`, `monster_killer`, count/pluralization, article removal) while messages are still English-ish — get it compiling and tests passing first, then translating templates becomes mechanical.
4. **Translate messages** file by file: `game.rs` messages (largest), then `main.rs` UI, then `score.rs`. At each splice slot, decide the case the Interslavic sentence governs — it is often *not* the case the English construction suggests.
5. **Fix the test suite**: game.rs/main.rs contain many tests asserting English message text; update expectations as you go, per file, so `cargo test` stays green throughout.
6. **Verify**: `cargo test`; render a sample of every message template with a masculine-animate, feminine, and neuter noun and eyeball the output; run collected message text through slovowiki's `check-text --json`; play a full game natively and in the web build (`web/`).

Work incrementally and keep the game compiling and playable after every phase. When a word choice is genuinely ambiguous (several official candidates), prefer native `genesis`, higher `frequency`, and note the decision in a `GLOSSARY.md` (English → chosen ISV word + gender/animacy + rationale) so the whole game stays terminologically consistent.
