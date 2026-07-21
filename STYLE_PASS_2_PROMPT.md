# Task: PR #6 follow-up — government lint, valence audit, and final polish

You are working in `/Users/kisaczka/Desktop/code/rogue-rs/mrzavec`, branch `agent/style-pass` (PR #6, currently `f87b3de`). The style pass converted colon-listings to real sentences and restored flavor; a follow-up review then caught two **case-government bugs that every automated check missed**: `s + genitive` on the death screens (`s N zlåtnikov` — reads "off of N gold pieces"; `s` "with" takes instrumental) and a double accusative in `zadavaješ odličny udar {defender-acc}` (a blow's recipient takes the dative). Both were invisible to check-text because government across a `{placeholder}` or digit crosses token-analysis boundaries.

This brief closes that blind spot with tooling, audits the remaining instances of the same bug class by hand, and finishes the known polish items. Read `GLOSSARY.md` (conventions + review log), `src/lang.rs` (`speak()` marker grammar above `render_marker`), and `scripts/lint_inflection.py` before starting. Do not commit; leave changes in the tree.

## 1. Preposition-government lint (flagship — turn the blind spot into a fence)

Markers carry their case codes, and `interslavic::preposition_cases(prep)` knows what each preposition governs. So the bug class that slipped through is mechanically checkable:

Add a third stage to `scripts/check_lang.sh` (extend `lint_inflection.py` or add `scripts/lint_government.py`):

1. Extract production literals (reuse the existing extractor — it already handles test-stripping, char literals, SYLLABLES, diagnostics).
2. Scan for the pattern **preposition token immediately followed by a `⟨…⟩` marker** (possibly with an intervening agreeing-adjective marker `⟨a:…⟩`/`⟨cmp|sup:…⟩` — take the case from the first case-bearing marker). Preposition list: query `preposition_cases()` for each candidate token (it returns `None` for non-prepositions — that IS the membership test; get the case data by calling the crate from a tiny `cargo run --example` helper or a `#[test]`-generated JSON dump, whichever is cleaner — do not hand-copy a preposition table into Python).
3. **Fail** when the marker's case is not in the preposition's governed set (e.g. `s ⟨n:zlåtnik:gen:pl⟩` → `s` governs {Ins, Gen}: careful — if the crate lists both cases for `s`, the lint can only WARN on the s+Gen combination, not fail; check what `preposition_cases("s")` actually returns and document the chosen severity per preposition. For unambiguous ones — `mimo`+Gen-only, `protiv`+Dat-only, `k`+Dat, `po`+Loc/Dat, `bez`+Gen — a mismatch is a hard fail).
4. **Report** (not fail) every preposition immediately followed by a `{format-placeholder}` or a digit — those are the human-review residue this lint can't see through; the report list must be small and each entry either fixed (decline via a marker/`inventory_name_case`) or annotated in a committed `scripts/government-notes.txt` with one justification line (e.g. `s {} ⟨n:zlåtnik:ins:pl⟩ — digit is a bare numeral, noun carries the case`).
5. Selftest: fixture strings for one pass, one hard fail (`mimo ⟨n:uho:acc⟩`), one warn.

Run it; fix everything it finds (expect: the two known-fixed bugs stay fixed; there may be more — `na`, `v`, `o`, `po` sites are numerous).

## 2. Verb-valence audit (the other half of the bug class)

The `udar` bug wasn't prepositional — it was verb valence (two-argument verb, wrong case on the second argument). No tool can check this; do a one-time systematic sweep instead:

- Enumerate every production template that contains **both** a verb marker (`⟨v2|v3|v3p|vpf3|lp:…⟩`) and a second case-bearing slot (another marker or a `{placeholder}` documented as case-declined).
- For each, state the verb's valence frame and check the slot's case: watch specifically for dative-taking verbs (dati/davati/zadati "give/deal to", pomagati "help", škoditi "harm" — `ukųs ne škodi ti` is already correct with the dative clitic), instrumental-taking (vladati, mahati + Ins?), and genitive-under-negation choices (the project deliberately keeps accusative there — see GLOSSARY; don't "fix" those).
- Record the sweep as a table in `GLOSSARY.md` (template → frame → verdict) so the next reviewer doesn't redo it. Fix anything wrong via markers.

## 3. Remaining polish items (all dictionary-verified already)

- Terse option label `⟨a:kråtky:věsť:nom:pl:U⟩ ⟨n:věsť:nom:pl⟩` ("brief news") → **`⟨a:kråtky:sȯobčeńje:nom:pl:U⟩ ⟨n:sȯobčeńje:nom:pl⟩`** ("brief messages") — official lemma is `sȯobčeńje` (ȯ!); add it to the lang.rs registry (n, inanimate), drop `věsť` if nothing else uses it.
- `"čuješ sę silněje.  Kaka sila!"` → restore the muscle-flex flavor with **`Kaka moć!`** (official `moć`) — or keep `sila` if you judge the near-duplicate of the sentence's `silněje` acceptable; decide and note it.
- Effect-name style split (potions bare-genitive vs some scrolls `za`+Acc): unify toward the bare genitive **only** where the gerund is an official lemma with a full declension (re-check with slovowiki `check-text`: the za-pattern exists precisely because non-lemma gerund genitives are unverifiable) — likely candidates: none or few; if a scroll's gerund turns out to be an official lemma after all, convert it; otherwise leave and extend the GLOSSARY note explaining WHY the split is permanent.

## 4. PR #6 code-quality minors (from the review)

- `take_off`: reuse the already-fetched item instead of a second `inventory.iter().find()` for the accusative name.
- Trap identification: render only the phrase the active branch needs (move the two `phrase()` calls into the `if`).
- `wield`/`wear`: compute the terse name only under `self.options.terse` (and the accusative only under verbose).

## 5. Surface audit of the unswept corners

Quick pass over: wizard-mode strings, the CLI `usage()` text (exempted from the lint for the easter egg — verify the rest of it is translated and grammatical), and every path where a `save.rs` error string can reach the player (save.rs is lint-exempt as internal diagnostics — confirm that's true by tracing where its `Err` values surface in main.rs; if any error text reaches a modal, that string moves out of save.rs or gets translated and lint coverage). Fix or document each finding.

## Validation

1. `cargo test` green (bless re-rendered expectations via the established left/right loop; list every change).
2. `./scripts/check_lang.sh` — all THREE stages pass: template gate 0/0, inflection lint 0, new government lint 0 hard-fails with a fully-annotated warn list.
3. `cargo check --target wasm32-unknown-unknown` green; native boot clean.
4. `GLOSSARY.md` updated: valence-audit table, polish decisions, government-notes rationale.

## House rules (unchanged)

- Forms come only from the crate/slovowiki; sentence restructuring beats case-forcing; crate output wins over literals.
- Terse mode stays telegraphic (colons fine there); verbose mode is real sentences.
- Conventions must be written in GLOSSARY.md the moment they're decided — the legša/legše bug happened because one wasn't.
- One commit per numbered item, uncommitted at the end is fine (the session driver commits); keep the game compiling throughout.
