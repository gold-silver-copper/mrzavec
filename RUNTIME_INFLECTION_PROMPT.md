# Task: Zero pre-inflected forms — every inflection produced at runtime

You are working in `/Users/kisaczka/Desktop/code/rogue-rs/mrzavec`, branch `agent/interslavic` (PR #5). The game is fully translated to Interslavic and verified (tests green, check-text template gate PASS 0/0), but sentence templates still contain **hand-typed inflected word forms** as string literals — conjugated verbs ("udarjaješ"), oblique nouns ("brȯnjų", "Temnice Pohibeli"), agreeing adjectives ("divna běla mgla"), pronoun forms ("tebe", "tę", "ničego"), participles ("opoznano", "ukradla"), comparative adverbs ("bystrěje"). An audit (2026-07-20) found **84 distinct pre-inflected tokens in game.rs literals alone**, plus more in main.rs and score.rs.

The policy, absolute: **no pre-inflected form may appear in any source literal.** Every word that surfaces in a non-citation form must be inflected at runtime by the `interslavic` crate. A literal may contain only:

1. **Uninflectable closed-class words** — prepositions, conjunctions, particles (ne, li, že, and the reflexive `sę`, which is part of verb citation forms), adverbs, interjections;
2. **Citation forms** of open-class words (Nom-sg lemma, Nom-sg-masc adjective, infinitive) — referencing `lang.rs` lexicon constants rather than being retyped;
3. Non-language content: digits, punctuation, key letters, machine option keys, scroll-syllable gibberish, "chongo was here".

Everything else is a violation, enforced by the Phase 2 lint.

## The crate is ready: pin `interslavic = "0.10.0"`

The prerequisite work is **merged upstream** (interslavic-rs `66d03b8..927a2a6`). Use crates.io `0.10.0`; if not yet published, path-pin `/Users/kisaczka/Desktop/code/interslavic-rs/crates/interslavic`. Record the pinned version in `GLOSSARY.md`. What you now have, all doc-tested and parity-checked:

```rust
use interslavic::{personal_pronoun, reflexive_pronoun, l_participle,
                  passive_participle, active_participle, PronounStyle};

// Personal pronouns: 198/198 parity vs @interslavic/utils, all three series.
personal_pronoun(Person::Second, Number::Singular, Gender::Masculine,
                 Case::Gen, PronounStyle::Full)        // == Some("tebe")
personal_pronoun(Person::Second, Number::Singular, Gender::Masculine,
                 Case::Acc, PronounStyle::Clitic)      // == Some("tę")
personal_pronoun(Person::Third, Number::Singular, Gender::Masculine,
                 Case::Gen, PronounStyle::AfterPreposition) // == Some("njego")
reflexive_pronoun(Case::Acc, PronounStyle::Clitic)     // == Some("sę")
// None = the cell does not exist (unattested clitic, Nom clitic) — handle it,
// never unwrap blindly; a None cell means restructure or use Full.

// Bare l-participles for fixed-gender past subjects ("strěla tę ubila"):
l_participle("ubiti", Gender::Feminine, Number::Singular)  // == "ubila"
// NOTE: the 0.10.0 rebuild FIXED three l-participle bugs (idti sg/pl swap,
// dropped prefixes, žegti/idti stem alternations). Trust the crate's output
// over any l-participle currently in a literal.

// Declined participles ("komnata jest osvětljena", "to jest opoznano"):
passive_participle("osvětliti", Case::Nom, Number::Singular,
                   Gender::Feminine, Animacy::Inanimate)   // Option<String>
active_participle(...)                                     // available if needed

// Also newly guaranteed by upstream doc-tests: pronoun("ty"/"svoj"/"oni", …)
// full forms via the standard entry point; comparative() pairs for
// bystry/blizky/dobry/silny/slaby; verb_forms() imperative + gerund for
// -ati/-iti/-ovati; dva/tri/pęť oblique numerals.
```

One upstream behavior change to be aware of: `pronoun("oni", …)` previously fell through to an adjectival declension; it now returns personal-pronoun forms (jih, jim…). mrzavec has no call sites affected, but do not "rediscover" the old behavior in blessed test strings.

## Phase 1 — template conversion

Extend `src/lang.rs` with a speech-helper layer wrapping the crate (plus the existing `first_variant` splitting), with a process-wide memoization cache — `HashMap<(kind, lemma, cell), String>` behind `Mutex`/`OnceLock`, wasm-safe:

```rust
pub fn v2(inf: &str) -> String      // 2sg present: v2("udarjati") == "udarjaješ"
pub fn v3(inf: &str) -> String      // 3sg present
pub fn v3pl(inf: &str) -> String    // 3pl present
pub fn vimp(inf: &str) -> String    // imperative 2sg (tombstone POČIVAJ = vimp("počivati").to_uppercase())
pub fn lpart(inf: &str, g: Gender, n: Number) -> String
pub fn ppart(inf: &str, l: &Lex, case: Case, n: Number) -> String  // passive_participle agreeing with l
pub fn pers(p: Person, n: Number, g: Gender, case: Case, style: PronounStyle) -> String // expect() the attested cells
pub fn poss(lemma: &str, l: &Lex, case: Case, n: Number) -> String // tvoj/moj/svoj via pronoun()
pub fn comp_adv(adj: &str) -> String // comparative("bystry").1 == "bystrěje"
```

Convert every template, file by file (game.rs → main.rs → score.rs). Target shape:

```rust
// before:
format!("udarjaješ {defender}")
// after:
format!("{} {defender}", lang::v2("udarjati"))

// before: "strěla tę ubila"
format!("{} {} {}",
    lang::decl(&STRELA, Case::Nom, Singular),
    lang::pers(Person::Second, Singular, Gender::Masculine, Case::Acc, PronounStyle::Clitic),
    lang::lpart("ubiti", Gender::Feminine, Singular))

// before: "v tvojej torbě ne jest města"
format!("v {} {} ne jest {}",
    lang::poss("tvoj", &TORBA, Case::Loc, Singular),
    lang::decl(&TORBA, Case::Loc, Singular),
    lang::decl(&MESTO, Case::Gen, Singular))
```

Conversion rules:

- **Output must be byte-identical** to the current review-verified strings wherever those are correct — most tests then stay green untouched. Where the crate produces a *different* form, **the crate wins** (that is a bug find — the l-participle fixes make this likely for any idti/prefixed-verb participles): re-bless the test via the established loop and log the change in `GLOSSARY.md`'s review section.
- Every fixed noun that any template declines (torba, město, zemja, tělo, ramę, glåva, oko, uho, rųka, noga, šija, nos, koža, světlo, iskra, tma, mgla, prah, sok, vkus, zapah, teplo, strah, směh, krik, bolj, pųť, dveri, prohod, koridor, komnata, voda, plamenj, dym, zvųk, karta, parola, čarovnik…) gets a `Lex` constant with dictionary metadata — it thereby lands in `game-lexicon.tsv` automatically via the regeneration test. Never inline gender/animacy at a call site.
- Multi-token fixed phrases become compositions: "Temnice Pohibeli" = `decl(&TEMNICA, Nom, Plural)` + `gen_sg(&POHIBEL)`; "Dobro došli" = adverb + `lpart("dojdti", Masculine, Plural)` (verify the idti-family suppletion output — this is one of the fixed bugs, the current literal may be stale).
- The clitic/full/n- distinction is now expressible — use it correctly: clitics in unstressed sentence-internal positions ("… tę zamråžaje"), full forms under emphasis or sentence-initially, AfterPreposition for every 3rd-person pronoun following a preposition ("mimo *njego*", never "mimo jego"). Sweep existing text for spots where the pre-0.10.0 workarounds used a full form where a clitic (or n- form) is standard, and fix via the API.
- Grammatical style dodges that avoid a case entirely (colon-listing confirmations, "za + gerund-Acc" effect names) may stay — the words inside them still follow the policy.
- Wizard/debug strings and CLI usage text are user-visible: same policy.

## Phase 2 — enforcement lint

Add `scripts/lint_inflection.py`, invoked as a second stage of `scripts/check_lang.sh` (gate fails on any violation):

1. Extract every string literal from `src/*.rs` **production code** (strip `#[cfg(test)]` modules), excluding: the `SYLLABLES` const, machine-key tables (`OPTION_LABELS` machine keys, key-letter strings), storage keys, and `format!` placeholder syntax.
2. Tokenize; drop digits/punctuation/single letters.
3. Classify each token via slovowiki `check-text --json --lexicon game-lexicon.tsv`: a token **passes** iff (a) its folded surface equals the folded surface of one of its own lemmas — citation form; this automatically admits adverbs, prepositions, conjunctions, and particles, which are their own lemmas — or (b) it appears in the committed allowlist `scripts/inflection-allow.txt` (interjections: mmm, fuj, hej, hura, buh; the easter egg; documented one-offs — keep it under ten entries, each justified in a comment; `sę` passes via the reflexive-verb citation forms in the index, add it to the allowlist only if it does not).
4. Any other token **fails** with file, literal, token, and analyses printed.

**Run the lint first** to get the authoritative violation inventory; drive Phase 1 from that list; you are done when it reports zero.

## Validation (all must pass)

1. `cargo test` — full suite green; every re-blessed string listed in the PR description with its reason (crate correction vs. mechanical identity).
2. `./scripts/check_lang.sh` — template gate PASS 0 unknown / 0 agreement **and** inflection lint 0 violations.
3. `cargo check --target wasm32-unknown-unknown` green; native binary boots.
4. Spot render: welcome line, one full inventory, one combat exchange, death screen, tombstone — unchanged from the current branch tip except documented corrections.
5. `GLOSSARY.md` records interslavic `0.10.0` and every output change.

## House rules

- **Never modify forms**: crate output is final. If a form looks wrong, the fix belongs in interslavic-rs (steen tables + the JS parity harness are the arbiters there) — file it, don't patch the string.
- One commit per phase; keep the game compiling throughout.
- `steen-legacy/` stays untracked reference; cite grammar sections in code comments where a linguistic decision needs justification.
