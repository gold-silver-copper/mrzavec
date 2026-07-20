# Task: Zero pre-inflected forms — every inflection produced at runtime

You are working in `/Users/kisaczka/Desktop/code/rogue-rs/mrzavec`, branch `agent/interslavic` (PR #5, currently at the post-review state `ac47fd6`). The game is fully translated to Interslavic and verified (tests green, check-text template gate PASS 0/0), but sentence templates still contain **hand-typed inflected word forms** as string literals — conjugated verbs ("udarjaješ"), oblique nouns ("brȯnjų", "Temnice Pohibeli"), agreeing adjectives ("divna běla mgla"), pronoun forms ("tebe", "tę", "ničego"), participles ("opoznano", "ukradla"), comparative adverbs ("bystrěje").

The new policy, absolute: **no pre-inflected form may appear in any source literal.** Every word that surfaces in a non-citation form must be inflected at runtime by the `interslavic` crate. A literal may contain only:

1. **Uninflectable closed-class words** — prepositions, conjunctions, particles (ne, li, sę, že), adverbs, interjections;
2. **Citation forms** of open-class words (Nom-sg lemma, Nom-sg-masc adjective, infinitive) — and these should reference `lang.rs` lexicon constants rather than being retyped;
3. Non-language content: digits, punctuation, key letters, machine option keys, scroll-syllable gibberish, "chongo was here".

Everything else is a policy violation, enforced by a new lint (Phase 2). An audit (2026-07-20, extraction + check-text classification) found **84 distinct pre-inflected tokens in game.rs literals alone**, plus more in main.rs (end screens, prompts, tombstone) and score.rs — the complete work inventory is reproducible with the audit method in Phase 2.

## Phase 0 — close the crate gap first (interslavic-rs)

Work in `/Users/kisaczka/Desktop/code/interslavic-rs` (the user's crate; separate PR there, then bump mrzavec's pin).

- **Personal pronouns are missing.** `interslavic_core::adjective::decline_pronoun` covers toj-class, moj-class, kto/čto, veś, ktory, -koli — but not ja/ty/on/ona/ono/my/vy/oni or the reflexive sebe. The game's templates need tebe/tę/ti/tobojų/jego/jemu/mně… Add a personal-pronoun module with the full paradigms **from the reference tables in steen.free.fr's grammar page** (a checkout is at `mrzavec/steen-legacy/steen.free.fr/interslavic/grammar.html`; cross-check against the slovowiki form index, which already carries pronoun paradigms from the STEEN-G tables — the two must agree). Include both full and clitic variants where the standard has them (mene/mę, tebe/tę, jego/go…), with an explicit API so callers choose: e.g. `personal_pronoun(person, number, gender, case, Clitic|Full) -> Option<String>`, plus routing bare lemmas ("ty", "on") through the existing `pronoun()` entry point for the full forms. Doc-test every cell against the steen table.
- **Expose the l-participle at the facade.** `interslavic_core::verb::l_participle(word, gender, number)` exists; make sure `interslavic::` re-exports it (add a facade fn + doc-test if not). The game needs bare l-participles for fixed-gender subjects ("strěla tę ubila" = l_participle("ubiti", Feminine, Singular)).
- Confirm the facade also covers everything else Phase 1 consumes: `verb` (2sg/3sg/3pl present), `verb_forms().imperative/.gerund/.pfpp/.prap`, `comparative` (adverbs), `adj`, `noun_with`, `numeral`. For declined participles ("osvětljena", "opoznano" predicatives) verify that feeding `verb_forms().pfpp` output through `adj()` produces the right agreement forms (slovowiki's declined-participle records were generated exactly this way — mirror that path).
- Version-bump, changelog, publish or path-pin; record the new version in `GLOSSARY.md`.

## Phase 1 — mrzavec: template conversion

Extend `src/lang.rs` with a small speech-helper layer (names indicative):

```rust
pub fn v2(inf: &str) -> String          // 2sg present: v2("udarjati") == "udarjaješ"
pub fn v3(inf: &str) -> String          // 3sg present  (subject gender irrelevant in present)
pub fn v3pl(inf: &str) -> String        // 3pl present
pub fn vimp(inf: &str) -> String        // imperative 2sg (tombstone "POČIVAJ" = vimp("počivati").to_uppercase())
pub fn lpart(inf: &str, g: Gender, n: Number) -> String   // bare l-participle
pub fn ppart(inf: &str, l: &Lex, case: Case, n: Number) -> String // past passive participle agreeing with l
pub fn pers(person: Person, n: Number, g: Gender, case: Case, clitic: bool) -> String
pub fn poss(lemma: &str, l: &Lex, case: Case, n: Number) -> String // tvoj/moj agreeing via pronoun()
pub fn comp_adv(adj: &str) -> String    // comparative adverb: comp_adv("bystry") == "bystrěje"
```

All of these are thin wrappers over the crate (plus the existing `first_variant` splitting) with a process-wide memoization cache — `HashMap<(kind, lemma, cell), String>` behind a `Mutex`/`OnceLock`, wasm-safe — so message rendering never recomputes hot forms.

Then convert every template, file by file (game.rs → main.rs → score.rs). Examples of the target shape:

```rust
// before:
format!("udarjaješ {defender}")
// after:
format!("{} {defender}", lang::v2("udarjati"))

// before: "strěla tę ubila"
format!("{} {} {}", nom(&STRELA), lang::pers(Second, Sg, _, Acc, true), lang::lpart("ubiti", Feminine, Singular))

// before: "v tvojej torbě ne jest města"
format!("v {} {} ne jest {}",
    lang::poss("tvoj", &TORBA, Case::Loc, Singular),
    lang::decl(&TORBA, Case::Loc, Singular),
    lang::decl(&MESTO, Case::Gen, Singular))
```

Rules for the conversion:

- **Output must be byte-identical** to the current (review-verified) strings wherever those are correct — most tests then stay green as-is. If the crate produces a *different* form than the literal, the crate wins (that is a bug find, not a regression): update the literal expectation via the established bless loop and note it in `GLOSSARY.md`'s review log.
- Fixed nouns that templates decline (torba, město, zemja, tělo, ramę, glåva, oko, uho, ruka, noga, šija, nos, koža, světlo, iskra, tma, mgla, prah, sok, vkus, zapah, teplo, strah, směh, krik, bolj, pųť, dveri, prohod, koridor, komnata, stěna, pod, voda, plamenj, dym, oblak, zvųk, karta, parola, čarovnik…) get `Lex` constants in `lang.rs` with dictionary metadata, exactly like the game nouns — and thereby also land in `game-lexicon.tsv` automatically. Grow the lexicon; never inline metadata at call sites.
- Multi-token fixed phrases ("Temnice Pohibeli", "Amulet Jendora", "Dobro došli v …") become compositions: `decl(&TEMNICA, Nom, Plural)` + `gen_sg(&POHIBEL)`, l-participle for "došli" (`lpart("dojdti", …, Plural)` — check the crate's suppletive idti handling), etc.
- Where a sentence exists only to dodge an inflection the crate couldn't produce before Phase 0 (the "za + gerund-Acc" effect names, colon-listing confirmations), you may keep the dodge — those are grammatical style choices, not violations — but the words inside them still follow the policy.
- Wizard/debug strings and CLI usage text are user-visible: same policy.

## Phase 2 — enforcement lint

Add `scripts/lint_inflection.py` (invoked as a second stage of `scripts/check_lang.sh`, failing the gate on any violation):

1. Extract every string literal from `src/*.rs` **production code** (strip `#[cfg(test)]` modules), excluding: the `SYLLABLES` const, machine-key tables (`OPTION_LABELS` second elements, key-letter strings), storage keys, and `format!` placeholder syntax.
2. Tokenize; drop digits/punctuation/single letters.
3. Classify each token via slovowiki `check-text --json --lexicon game-lexicon.tsv`: a token **passes** iff (a) its folded surface equals the folded surface of one of its own lemmas (citation form — this automatically admits adverbs, prepositions, conjunctions, and particles, which are their own lemmas), or (b) it appears in the committed allowlist `scripts/inflection-allow.txt` (interjections: mmm, fuj, hej, hura, buh; the easter egg; documented one-offs — keep this file under ten entries and justify each in a comment).
4. Any other token — i.e. anything check-text analyzes as a non-citation inflected form, and anything unknown that is not allowlisted — **fails the build** with file, literal, token, and analyses printed.

Run the lint before converting anything to get the authoritative violation inventory (expect ~84 distinct in game.rs + main.rs/score.rs additions); drive Phase 1 from that list; finish when the lint reports zero.

## Validation (all must pass)

1. `cargo test` — full suite green (re-bless only where the crate corrected a literal; list every such change in the PR description).
2. `./scripts/check_lang.sh` — template gate still PASS 0 unknown / 0 agreement, **and** the new inflection lint reports 0 violations.
3. `cargo check --target wasm32-unknown-unknown` green; native binary boots.
4. Spot render: the welcome line, one full inventory, one combat exchange, the death screen and tombstone — eyeball that output is unchanged from `ac47fd6` except documented corrections.
5. interslavic-rs: its own test suite green, personal-pronoun doc-tests match the steen tables; slovowiki's pronoun form index agrees (`check-text` on a sample of generated pronoun forms: all known).

## House rules

- **Never modify forms**: the crate's output is final; if it looks wrong, the fix happens in interslavic-rs (with steen/slovowiki as arbiters), never by editing the string.
- Player-visible text changes only when the crate corrects a genuine error; every such change is logged in `GLOSSARY.md`.
- One commit per phase in each repo; mrzavec pins the exact interslavic version it validated against.
- `steen-legacy/` stays untracked reference material; cite the grammar section (page + table) in code comments where paradigms were sourced.
