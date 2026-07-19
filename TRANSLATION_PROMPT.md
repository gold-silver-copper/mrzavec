# Task: Translate mrzavec completely from English to Interslavic

You are working in `/Users/kisaczka/Desktop/code/rogue-rs/mrzavec`, a Rust/Bevy rewrite of Rogue 5.4.5. Your job is to translate **every user-facing string** into Interslavic (medžuslovjansky), with grammatically correct case, number, gender, animacy, and verb aspect — not a word-for-word literal swap. The English original lives on as the separate `rogue-rs` repo, so you may change mrzavec's strings and string-producing code freely and permanently.

Use **standard Interslavic flavored Latin orthography** throughout (`ě ę ų å ȯ č š ž ć đ ń ľ ŕ ť ď ś ź`), matching the orthography of the two support tools below.

## Your two support tools

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
ISV::verb_with_present_hint("bolěti", "(boli)", ...)              // when a present-stem hint is known
ISV::pronoun("toj", ...), ISV::numeral("pęť", ...)                // Option<String>, closed classes
ISV::noun_forms(lemma) -> NounParadigm                            // full paradigm; .get(case, number)
ISV::verb_forms(inf) -> VerbParadigm                              // .present/.perfect/.imperative/.gerund/...
```

Hard rules learned from its source and review notes:

- **Always pass explicit `Gender` + `Animacy`** (`noun_with`, `adj`) using metadata from slovowiki (below). For out-of-vocabulary words the engine *guesses* from the ending and can be wrong.
- Nouns go in as **Nom-sg lemma**, adjectives as **Nom-sg-masc** (`dobry`), verbs as the **infinitive** (`pisati`).
- Some results pack alternatives into one string: `"oči / očesa"`. **Split on `" / "` and take the first variant**; do this in one shared helper so it's consistent everywhere.
- The library declines *single words only*. It does **no agreement and no phrase assembly** — you decline the adjective and the noun separately into the same case/number/gender/animacy and `format!` them together yourself.
- Compound past tenses need the subject's gender (l-participle agreement) — see "player gender" below.
- Cache `NounParadigm`/`AdjParadigm` per lemma (a `HashMap` in the lexicon module) instead of re-declining hot words every message.

Note: slovowiki's inflection tables are generated *by this same crate*, so the two agree by construction — slovowiki tells you *which word* and its metadata; the crate produces its *forms* at runtime.

### 2. slovowiki — the lexical API (word choice, metadata, verification)

Local checkout: `/Users/kisaczka/Desktop/code/slovowiki`. **Read `site/api/agent-guide.md` first — it is the manual**, with an artifact-per-task table, both lookup protocols, self-tests, trust rules, and translation/verification workflows. The static API under `site/api/` has already been exported at **schema 3** (form-index `meta.json` says `schema_version: 3`; if you ever find it at 2 or stale, regenerate with `cargo run --release -- export --out site`). No server needed — every artifact is a plain JSON file you read from disk.

**Why the API and not just the official dictionary CSV:** the API covers ~63k lemmas — the ~17k official dictionary words (`status: official` / `official-only`), closed-class `grammar` words, and ~46k `generated` entries (cognate-based reconstructions and regular derivatives off official bases). The generated layer is your source of *options for words the official dictionary lacks* — monster names, flavor vocabulary, derived adjectives like `mečny` ← `meč`. The API also carries what the raw CSV cannot give you: ranked English→Interslavic candidates, verb aspect partners, false-friend warnings with preferred alternatives, per-entry attestation evidence, and a verifier for every surface form.

#### English → Interslavic lookup (`site/api/en/`)

Protocol (validated against `api/en/selftest.json` — run that check once per session; all samples must reproduce):

1. Normalize the English query: lowercase → replace punctuation with spaces → collapse whitespace → trim → strip leading `to `.
2. Route: `shard = fnv1a32(utf8(key)) % 256` (FNV-1a 32-bit, offset `0x811c9dc5`, prime `16777619`). Read `records[key]` in `api/en/<shard>.json`.
3. On a miss, retry without a leading article, then per content word, then with synonyms.

Working reference implementation (tested):

```python
import json, re
def fnv1a32(b):
    h = 0x811c9dc5
    for x in b: h = ((h ^ x) * 16777619) & 0xffffffff
    return h
def norm(q):
    q = re.sub(r'[^\w\s]', ' ', q.lower()); q = ' '.join(q.split())
    return q[3:] if q.startswith('to ') else q
def lookup(q, api='/Users/kisaczka/Desktop/code/slovowiki/site/api'):
    k = norm(q)
    d = json.load(open(f'{api}/en/{fnv1a32(k.encode()) % 256}.json'))
    return d.get('records', d).get(k) or []
```

Each candidate object: `lemma`, `entry_id`, `official_id`, `pos`, `gloss`, `status`, `trust` (`verified-official` / `verified-official-only` / `generated-review`), `rank`, `match` (`phrase` / `exact-gloss-head` / `gloss-token`), `aspect` (`ipf`/`pf`/`ipf/pf`/null) + `aspect_partners`, `warnings`, `prefer`, `probability`, and `form_lookup` (key/shard/path into the form index). Real examples from this checkout:

- `sword` → `meč` (noun, `verified-official`) — the easy case.
- `heal` → `lěčiti` (`ipf`, partner `izlěčiti` pf); `cure` also surfaces `izcěliti` pf. Verbs come with their aspect — pick ipf for ongoing/habitual, pf for a completed single event, and get the partner from `aspect_partners`.
- `healing` → only `generated-review` candidates. Don't settle: re-query `heal`/`cure` and take the official verb's gerund (`lěčeńje` via `ISV::verb_forms("lěčiti").gerund`) or another official derivative.
- `wand` → complete miss; `staff` → only `načeľnik štaba` ("chief-of-staff" — a `gloss-token` trap). Resolution: synonym queries — `scepter` → `žezlo` (official), `stick` → `kyj`, `palka`, `rod` → `prųt`. This kind of miss-then-reason loop is normal; budget for it.

Ranking caveats: `rank` is comparable only *within* one English key; verified records always sort before generated. A `gloss-token` match means your word merely appeared inside a longer gloss — **read the gloss before trusting it**.

#### Other artifacts you will use

| Need | Artifact |
|---|---|
| Verify/analyse an Interslavic token (is it real? which case/number?) | `api/forms/<n>.json` — fold the token (`ě ę ė→e, ų→u, å→a, ȯ→o, ĺ/ľ→l, ń→n, ŕ→r, ť→t, ď→d, ś→s, ź→z, ć→č, đ→dž`, keep `č š ž`), route `fnv1a32 % 2048`; validate against `api/router-selftest.json` first. Records: `[form, lemma, entry_id, pos, [analyses], source, status, probability, gloss]`; analyses like `akuz.jd. m.živ.` (acc sg masc animate), `prez.3mn.`, `perf.3jd.m.`. Covers full paradigms, declined participles, comparatives/superlatives, pronoun & numeral paradigms; multi-word official lemmas have space-joined keys — try trigram → bigram → unigram. |
| Enumerate/filter all lemmas by status, POS, aspect | `api/lemmas.json` — 8-field rows `[lemma, pos, status, probability, entry_id, gloss, aspect, aspect_partners]` |
| Verb pair model | `api/aspect-pairs.json` |
| False-friend warnings + preferred replacements | `api/notes.json` (e.g. `pytati` = to *ask*, not torture; `rok` = year, not deadline) — check-text applies these automatically |
| Attestation evidence for reasoning | `site/entries.json` — per entry: `langs_list` (which of 12 Slavic languages attest cognates), `branches`/`branch_pattern` (`V+Z+J` = East+West+South), `borrowed`, Proto-Slavic `ancestor`, `official_id` |
| Human-checkable citation | `site/entry/<entry_id>.html` |

#### The verifier — run it constantly

```bash
cd /Users/kisaczka/Desktop/code/slovowiki
cargo run --release -- check-text <file> --json    # or ./target/release/interslavic-wiktionary-lab
```

Per token it returns `status` (`known-lemma` / `known-form` / `generated` / `unknown`), the matched `lemmas`, morphological `analyses` (so you can confirm a form really is the accusative you meant), `ambiguous`, typo `suggestions`, false-friend `warning`/`prefer`, and conservative `agreement` errors (adjacent adjective–noun case/number/gender, preposition government, pronoun–verb person/number). It accepts ASCII-folded input (`Netopyr` matches `netopyŕ`). Run your lexicon and rendered message samples through it at every phase; `explain <word>` spot-checks a single word with a rule trace.

## Word choice: trust levels, your own reasoning, and the forms rule

**Trust hierarchy** (from the agent guide, confirmed in the data): `official` and `official-only` are both verification-grade dictionary words (`official` additionally has cognate-corpus evidence attached); `grammar` covers closed-class function words; `generated` is *never* verification-grade — `probability` (when non-null) is a model score, and a missing key means "unknown to Slovowiki", **not** "wrong".

**Generated words are allowed — deliberately, not accidentally.** Where the official dictionary has no word (fantasy monsters, flavor vocabulary, derived adjectives), a `generated` candidate is often better than a clumsy official paraphrase. Using one is a *decision*: record it in `GLOSSARY.md` as unverified, note its probability and entry page, and prefer generated **derivatives of official bases** (`deriv:<pattern>` analyses, e.g. `mečny` ← `meč`, p≈0.80) over cognate reconstructions, since their base is attested.

**Do your own reasoning — slovowiki proposes, you decide.** For every word that matters, before accepting a candidate:

1. **Read the gloss** and reject sense mismatches (the `staff` → chief-of-staff trap). Re-query synonyms until you find the sense you mean.
2. **Weigh the evidence** in `entries.json`: prefer words attested across all three branches (`V+Z+J`) and many `langs_list` languages; be suspicious of `borrowed` items when a native word exists; for pan-Slavic recognizability this beats any single intuition.
3. **Heed `warnings`/`prefer`** — when `prefer` is non-empty, use those lemmas instead.
4. **Pick aspect consciously** for every verb: game messages describing a completed event ("you hit it", "the scroll turned to dust") want the perfective partner; ongoing states want imperfective.
5. **Use your own Slavic knowledge** to judge between candidates — register, connotation, what a Polish/Russian/Serbian speaker would each understand — but never let it *override* the data silently: if you believe slovowiki's best candidate is wrong, say so in `GLOSSARY.md` with your reasoning, and pick a word that still exists in slovowiki (or is an explicit flagged coinage — see monster names below).
6. Optional secondary signals: `data/official-isv.csv` (same official dictionary as the API) additionally carries a `frequency` column and per-language cognate columns; use it to break ties between official synonyms, not as the primary lookup.

**Never modify forms.** This is a hard rule. Surface forms come from exactly two sources: the `interslavic` crate's output at runtime, and slovowiki's form index / official lemmas at lexicon time. Do not hand-edit spellings, endings, or diacritics; do not "fix" a declension you find odd; do not invent an inflected form for a generated lemma (they have no inflection records *on purpose* — an inflected form of a wrong lemma is confidently wrong; if you adopt a generated *noun*, its runtime forms must come from the crate with explicit gender/animacy, and be flagged unverified in `GLOSSARY.md`). If the machinery cannot produce a form you need, choose a different word or restructure the sentence — never patch the string.

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

   Populate gender/animacy/aspect **from slovowiki** (the candidate's `pos` + the form index's analyses tags `m.živ.`/`m.než.`/`ž.`/`sr.`), never by guessing. Provide helpers `fn decl(lex, case, number) -> String` (wrapping `ISV::noun_with` + first-variant splitting) and `fn adj_for(lex, adj_lemma, case, number) -> String` for agreeing adjectives.

2. **Change the name-producing choke points to take a grammatical role.** `monster_message_name`, `item_name`, `inventory_name`, `monster_killer` must accept `(Case, Number)` (and internally know each noun's gender/animacy). Then fix every call site to request the case its sentence governs. The main splice-site inventory (from the English code):
   - Accusative objects: `"you hit {defender}"` (game.rs ~L2384), `"you missed {}"`, `"you have defeated {}"` (L3901)
   - Nominative subjects: `attack_hit_message`/`attack_miss_message` (L3837–3888) — here the *monster* is subject and *you* are object, so both slots change roles
   - Instrumental after passive "by": `"you are frozen by the {}"` (L4461), `"you are hit by the {}"` (L2718) — use the instrumental (or `od` + Gen where more natural; check government with `api/preposition` data via `ISV::preposition_cases` and check-text)
   - Genitive possessor: `"{name}'s gaze has confused you"` → `"pogled {Gen} ..."`
   - Two-noun sentences: `"the {weapon} hits {defender}"` (L2382) — weapon Nom, monster Acc, and the *verb agrees with the weapon's gender/number*
   - Death causes (`monster_killer` L1012, `"Killed by {}"` in main.rs) → instrumental or `od` + genitive; also fix `death_cause_with_article` and the tombstone's article-stripping (main.rs L707–753), which become unnecessary
3. **Item-name composition** (`item_name`/`inventory_name`, game.rs L1948/L2035) is itself templated (`"{color} potion"`, `"potion of {effect}"`, `"{count} scrolls"`). Rules:
   - `"X of Y"` → head noun + **genitive** (or an official/derived adjective when one exists): *potion of healing* → e.g. head noun + `lěčeńja`. Decide per item; keep a consistent pattern.
   - Color/material descriptors become **adjectives that agree** with the head noun in gender/case/number, inflected via `ISV::adj`. Store `COLORS`/`STONES`/`WOOD`/`METAL` as adjective lemmas (or `<material> + Gen` phrases where no adjective exists). This also affects `pick_color` hallucination swaps and `"your {name} glows {color}"`-type messages, where the color is predicative — use the appropriate short/neuter form.
4. **Counts** (`inventory_name` plural logic): replace English `+"s"` with Slavic numeral government —
   - 1 → Nom **singular** (with `jedin` agreeing, if you show the numeral)
   - 2–4 → Nom **plural** (dual-style agreement)
   - 5+ → **genitive plural**, and any agreeing verb goes neuter singular
   Centralize this in one `fn counted(n, lex) -> String` helper.
5. **Delete article machinery.** Interslavic has no articles: remove the three a/an vowel checks (`inventory_name`'s closure L2037, `monster_killer` L1012, `death_cause_with_article` main.rs L753), the hardcoded `"this scroll is an {} scroll"` (L1431), and strip articles from `TRAP_NAMES`.
6. **Player gender for past tense.** Compound past (`Tense::Perfect`) l-participles agree in gender. Keep player-directed messages in 2nd person **present** where the original does; where past is unavoidable add a `player_gender` field to `Options` (default `Masculine`, editable in the options screen like `fruit` is).
7. **Keep, adapted:** the render-time capitalization rule (`message_display_text` main.rs L80) and `uppercase_first` still apply. The hallucination name-swap and xeroc-reveal logic (game.rs L3800, L3813) work unchanged once names flow through the case-aware helpers.

## Translation content guidelines

- **Scroll `SYLLABLES`** are gibberish "magic language" — don't translate; **replace** with Slavic-flavored syllables (e.g. `zdra`, `mir`, `vlk`, `grom`, `pŕst`, `sněg`...) so generated titles look right next to Interslavic text. Keep the table ~same size so title-length distribution is preserved.
- **Monster names**: real animals/beings via the English API (bat→`netopyŕ`, dragon→`drakon`, snake, wolf...); for mythological ones (jabberwock, xeroc, griffin) first check the API — including `generated` candidates — and only if nothing fits, coin a phonologically Slavic name. Every coinage and every generated pick gets a hand-written gender/animacy in the lexicon, a `GLOSSARY.md` entry marked *unverified/coined*, and a printed-paradigm sanity check. Preserve the classic A–Z initial-letter mapping to display glyphs **only if feasible**; the glyph char is separate from the name in `MonsterTemplate`, so when Interslavic initials can't cover A–Z, keep the original glyphs and let names diverge — gameplay identity lives in the glyph.
- **Fruit**: change the default `"slime-mold"` (game.rs L331) to something Slavic and declinable (e.g. `sliva`). It gets counted and spliced (`"{count} {fruit}s"`, `"...tastes like {} juice"` → genitive), so run it through the same lexicon machinery; since it's *user-editable*, fall back to `ISV::noun` (dictionary lookup + ending-guess) for arbitrary user input.
- **UI chrome** (help, options, status): translate labels; keep the actual key characters (`h/j/k/l`, `i`, `q`...) untouched. The 80-column layout is fixed — check every translated footer/status/help line still fits 80 chars (Interslavic runs longer than English; abbreviate like the original does if needed).
- **Tombstone / score / end screens**: translate the prose; re-center the ASCII tombstone text for the new word lengths (main.rs L695–744).
- **Register**: address the player with informal 2nd-person singular (`ty`), matching classic Rogue's tone.
- Flavored orthography everywhere; both tools emit it natively. Non-ASCII is fine — the renderer draws Unicode chars, but **verify early** that `ě/ų/č` render in the terminal grid and the wasm build (put one in the welcome message and run the game before translating everything).

## Suggested execution order

1. **Wire up**: add the path dependency; smoke-test `ISV::noun_with("meč", Acc, Sg, Masculine, Inanimate)` from inside mrzavec; verify a diacritic renders in-game. Run both slovowiki selftests (`api/en/selftest.json`, `api/router-selftest.json`) against your lookup helpers once.
2. **Build the lexicon** (`src/lang.rs`): all 111 canonical nouns (items/monsters/traps) + appearance adjectives + the verbs your message templates need, each chosen through the English API with the reasoning checklist above, gender/animacy/aspect recorded from slovowiki, decisions logged in `GLOSSARY.md` (English → ISV word + metadata + trust status + rationale). Write declension tests (`assert_eq!(decl(BAT, Acc, Sg), "netopyŕa")`-style) and verify the whole lexicon with `check-text --json`.
3. **Refactor the choke points** to case-aware signatures (`monster_message_name`, `item_name`, `inventory_name`, `monster_killer`, count/pluralization, article removal) while messages are still English-ish — get it compiling and tests passing first, then translating templates becomes mechanical.
4. **Translate messages** file by file: `game.rs` messages (largest), then `main.rs` UI, then `score.rs`. At each splice slot, decide the case the Interslavic sentence governs — it is often *not* the case the English construction suggests. Choose verb aspect per event semantics.
5. **Fix the test suite**: game.rs/main.rs contain many tests asserting English message text; update expectations as you go, per file, so `cargo test` stays green throughout.
6. **Verify**: `cargo test`; render a sample of every message template with a masculine-animate, feminine, and neuter noun and eyeball the output; run the collected rendered text through `check-text --json` and fix every `unknown` token and `agreement` warning (or consciously accept and document it); play a full game natively and in the web build (`web/`).

Work incrementally and keep the game compiling and playable after every phase. `GLOSSARY.md` is the contract that keeps the whole game terminologically consistent — every noun, verb, and adjective choice goes in it, with its trust status and your reasoning for anything non-obvious.
