# Task: Literal-translation pass — say what the player DOES

You are working in `/Users/kisaczka/Desktop/code/rogue-rs/mrzavec` (main, post-PR #6). Owner directive, verbatim intent: *"I don't like how it says 'sejčas imaješ žȯlty napitȯk' when I pick up a potion — I want it to more literally say something like 'you pick up a yellow potion', not 'now you have a yellow potion'."*

The translation was built under constraints that no longer exist: early passes paraphrased whenever a literal word was unverified or a construction was grammatically awkward, and later passes inherited those paraphrases. With runtime inflection, the three-stage gate, and cheap dictionary verification all in place, the default flips:

**Principle: translate the action.** Every message should say what the player (or monster) *does*, mirroring the original English message's verb and intent as literally as good Interslavic allows. Paraphrase survives only when (a) the dictionary genuinely lacks the words after real research (en CLI + synonym ladder + slovowiki check), or (b) the literal rendering would violate a documented convention (gender-neutral player narration, terse telegraphy). Every surviving paraphrase gets a GLOSSARY line saying what was tried.

## Method — audit every message pair

The English originals are preserved at `/Users/kisaczka/Desktop/code/rogue-rs/rogue-rs/src/{game,main,score}.rs` (the legacy repo — same file structure, pre-translation). For each production message/template in the current sources, locate its English counterpart (same function/flow position; the code structure matches closely) and classify:

- **LITERAL** — already mirrors the English action. Leave it.
- **UPGRADE** — paraphrase where a literal rendering is achievable. Fix it.
- **KEEP** — paraphrase forced by dictionary gap or convention. Document it.

Record the full audit as a three-column table appended to `GLOSSARY.md` (English → current → verdict/new). This is the deliverable that prevents pass number four.

## Pre-verified vocabulary (checked against slovowiki 2026-07-21 — use these)

| concept | official lemma | notes |
|---|---|---|
| pick up | **podbirati** | `⟨v2:podbirati⟩` → "podbiraješ" — THE pickup fix |
| take | brati / bereš | already in use |
| thirst | **žęđa** (f) | "you suddenly feel very thirsty" → literal |
| muscle | **myšca** (f) / muskul (m) | "What bulging muscles!" → "Kake myšce!" |
| stomach / gut | **želųdȯk** (m) / **črěvo** (n) | teleport wrench message |
| skillfully | **umělo** (adv ← uměti) | "more skillful" without gendered adjectives |

Not in the dictionary (already probed — don't re-guess, research properly or KEEP): `vzimati`, `strumenj` (gush), `zagraditi` (block), the entire faint family (`omdlěti`, `zamdlěti`, `nesvěst`, `mravjeńje` tingling). For each, run the en CLI synonym ladder (e.g. faint: "faint", "swoon", "collapse", "lose consciousness"; tingle: "tingle", "prickle", "itch") before conceding.

## Flagged upgrades (found in the audit that produced this prompt — start here, then sweep)

1. **Pickup** (both sites: merge + new item): `sejčas imaješ {acc} (a)` → **`podbiraješ {acc} (a)`**. Terse stays `{name} (a)`.
2. **Walk-over**: English "you moved onto X" became `tu leži X`. Literal option: `staješ na {acc}` ("you step onto X") — decide between it and the current existential (the count-agreement machinery supports either); the terse `tu: X` stays.
3. **Thirst** (mysterious trap): `naglo hoćeš piti` ← "you suddenly feel very thirsty" → `naglo čuješ velikų žęđų`.
4. **Muscles** (gain-strength potion): `Kaka moć!` ← "What bulging muscles!" → `Kake myšce!` (nom pl via the crate).
5. **Teleport wrench**: `čuješ bolj po vsem tělu` ← "you feel a wrenching sensation in your gut" → something with `želųdȯk`/`črěvo` (e.g. `čuješ silnų bolj v črěvě`) — verify the declined forms render as expected.
6. **Skillful** (dexterity potion): `naglo vse dělaješ mnogo lěpje` ← "you suddenly feel much more skillful" → use `umělo`: e.g. `naglo vse dělaješ mnogo umělěje` **if** the crate's comparative accepts the base — probe `comparative` on the right lemma first; if only the positive exists, `naglo dělaješ vse tako umělo` or similar. Gender-neutral, always.
7. **Faint** (×3 sites): `Padaješ bez sil` / `…Padaješ` ← "You faint" — research the verb (ladder above); if nothing verifies, the current restructure is the documented KEEP.
8. **Tingling**: `koža tę svŕbi` ← "you have a tingling feeling" — research; likely KEEP (itching is the nearest attested sensation), but document.
9. **Water gush** (rust trap): `voda lije sę ti na glåvų` ← "a gush of water hits you on the head" — try a literal "stream/jet of water hits you" if a gush-word verifies; else KEEP.
10. **Magic block**: `čarovna sila ne pušćaje tę dalje` ← "your way is magically blocked" — passive-literal needs a block/bar verb (`zagraditi` unknown; try "obstruct", "bar", "block" ladder). KEEP if dry.
11. **Munchies overpower** (hallucination hunger): `glad prěmagaje tę.  Panika!` ← "the munchies overpower your motor capabilities. You freak out" — the drug-humor register is flattened; see if a closer rendering lands without inventing words.
12. **Vanish on ground**: `{name} padaje i izčezaje` ← "the {name} vanishes as it hits the ground" — literal order: `{name} izčezaje pri udaru o zemjų` (verify `udar` Loc + `o` government passes the government lint) or KEEP.

Then sweep the remaining ~180 message pairs with the same eyes — the audit that produced this list stopped at the obvious cases; expect a dozen more UPGRADEs, especially among the potion/scroll/ring effect messages and the less-traveled wizard flows.

## Rules (all standing conventions apply)

- Gender-neutral player narration (2nd-person present; adverbial predicative comparatives — see GLOSSARY conventions).
- Terse mode stays telegraphic; upgrades are for verbose mode.
- Every new word dictionary-verified BEFORE use; every form from the crate (markers / lang helpers); the three gate stages must pass (`./scripts/check_lang.sh`: template 0/0, inflection lint 0, government lint 0 hard-fails — new prep+marker pairs will be auto-checked; annotate any new placeholder-crossing warns in `scripts/government-notes.txt`).
- Test expectations re-blessed to rendered output. **Known hazard**: the ad-hoc bless loop used in previous passes corrupts literals containing escaped quotes — either fix that (proper `\"`-aware replacement) as step zero or bless those few by hand; check `git diff` after every bless run.
- Branch off `main` (suggest `agent/literal-pass`), one commit per logical group, PR to `main` at the end with the audit table summarized in the description.

## Validation

1. `cargo test` — all green.
2. `./scripts/check_lang.sh` — all three stages PASS.
3. `cargo check --target wasm32-unknown-unknown` green; native boot clean.
4. `GLOSSARY.md` carries the full audit table and a KEEP-list with research notes.
5. PR description lists every message changed (English → old → new) — the owner wants to review the actual sentences, not diffs of markers.
