# Interslavic Translation Guidance

Everything learned building and maintaining mrzavec's Interslavic
(medžuslovjansky) text, distilled for the next contributor — human or
agent. Read this before touching any player-visible string. The
companion documents are `GLOSSARY.md` (the word-choice contract and
decision log — this file explains *how*, that file records *what*) and
the upstream crate's `INTEGRATION.md`.

## 1. The one rule everything else serves

**Zero pre-inflected forms.** No string literal in production code may
contain an inflected word. Every surface form is produced at runtime by
the [`interslavic`](https://crates.io/crates/interslavic) crate, through
the `⟨…⟩` marker templates rendered by `lang::speak()` or through
`lang.rs` helper calls. Literals may contain only citation forms
(Nom-sg lemmas, Nom-sg-masc adjectives, infinitives), uninflectable
closed-class words (prepositions, conjunctions, particles, adverbs,
interjections), and non-language content (digits, key letters,
punctuation).

Why: hand-typed inflections rot silently. Every correction cycle in this
project's history found errors in hand-written forms (`hočeš`→`hoćeš`,
`izgledaje`→`izględaje`, `tělě` turning out to be the word for "calf");
crate-produced forms are parity-verified against the reference
implementation and can be regenerated when the standard evolves.

**Corollary — never modify a form.** If the crate's output looks wrong,
the fix belongs upstream in interslavic-rs (where the parity harness and
steen's grammar arbitrate), never in a patched string here. The crate's
output wins over any literal, always: when adopting a new crate version
changes a rendered string, re-bless the expectation and log it —
that is a fix arriving, not a regression.

## 2. The arbiters, in order

Different questions have different authorities. Confusing them causes
bugs; the `staje`/`stajaje` and `legša`/`legše` episodes both came from
arbiter confusion.

| Question | Arbiter |
|---|---|
| What is the surface form of an inflection? | the `interslavic` crate (parity-verified vs `@interslavic/utils`) |
| Is this a real word / what does it mean / what metadata? | slovowiki (`en` CLI, `check-text`, official dictionary; `verb_info`/`noun_info` for what the crate's embedded dictionary says) |
| What does the grammar prescribe (government, agreement, word order)? | steen.free.fr (checkout at `steen-legacy/`, gitignored) and the crate's documented policies |
| What has this project decided (register, conventions, word choices)? | `GLOSSARY.md` — and a decision is not made until it is written there |

Where arbiters disagree (slovowiki's index has `staje`, the parity
standard says `stajaje`), pick deliberately, document the divergence in
GLOSSARY, and flag it to the losing project.

## 3. How text is produced

- **Message templates** carry `⟨kind:lemma:…⟩` markers; the grammar is
  documented above `render_marker` in `src/lang.rs` (verbs `v1/v2/v3/
  v3p/vim/v3h/vpf3`, participles `lp/pp/ap`, nouns `n`, agreeing
  adjectives/determiners `a`, comparatives/superlatives `cav/cmp/sup`,
  adverbs `adv`, pronouns `ty/on/…` with clitic/full/n-form styles,
  `toj`-class, `ničto`-class; any marker takes a trailing `:U` to
  uppercase its first letter). Rendering happens at the sinks
  (`Game::message`/`die`/`remember_message`, plus main.rs display choke
  points) — `speak()` is idempotent and never panics on malformed input
  in release builds (player-typed labels flow through it).
- **Dynamic names** (items, monsters) are built by `lang.rs` from `Lex`
  entries (lemma + gender + animacy) — `decl`/`phrase`/
  `inventory_name_case(item, drop, Case)` — declined into the case the
  sentence governs at each call site.
- **Counts** go through `interslavic::quantified` (the crate owns
  numeral government; local code only mirrors its case/number choice for
  adjective agreement, guarded by a consistency test until
  `quantified_parts` ships upstream).
- Every noun a template declines lives in the `reg()` registry in
  `lang.rs`; the project lexicon `game-lexicon.tsv` is **generated** from
  lang.rs by the `regenerate_project_lexicon` golden test — edit lang.rs,
  never the TSV.

## 4. Choosing words

1. **Look it up**: `slovowiki en <english> --json` (or `en --batch` for
   lists). Prefer `verified-official`; read the gloss of every candidate
   — "staff → načeľnik štaba" is the canonical trap; heed `sense_note`.
2. **Weigh evidence** for synonyms: frequency, attesting-language count,
   branch pattern (`V+Z+J`), native over borrowed.
3. **Verify the exact spelling** the dictionary uses before writing it —
   `pušćati` not puščati, `stråna` not strana, `sȯobčeńje` with the `ȯ`.
   Run candidates through `check-text`; do not trust memory, including
   your own from earlier in the same session.
4. **Aspect is a choice**: completed events take the perfective partner
   (`aspect_partners` in the en output), ongoing states the
   imperfective. The narration voice is imperfective present
   (`ubivaješ`, `udarjaješ`); perfective present is used sparingly for
   punctual narration (`usneš`).
5. **Valence is not in any lint.** Before pairing a verb with an object,
   check `interslavic::verb_info` for transitivity (it corrected this
   project once: `hybiti` is intransitive — the "by analogy" accusative
   was wrong). Full case frames (dative-takers etc.) aren't in the
   dictionary; reason from pan-Slavic patterns, then record the frame in
   GLOSSARY's valence table. New verb + second argument = new table row,
   no exceptions.
6. **No official word?** In order: try the synonym ladder (en CLI);
   consider a slovowiki `generated` word if it's genuinely pan-Slavic
   (`obmråk`, `mråviti` — flag it in GLOSSARY, add a project-lexicon
   row); for fantasy names, coin through `slovowiki coin-check`
   (phonotactics/collision/false-friends/declension — it caught
   `jeti` colliding with official `jęti`); for arbitrary flavor items
   (gemstones, woods), **substitute** an attested material instead of
   forcing a translation. Paraphrase is the last resort and gets a
   GLOSSARY line saying what was tried.

## 5. Project conventions (violating these is a bug even if grammatical)

- **Translate the action.** Messages say what the player does —
  `podbiraješ žȯlty napitȯk`, not "now you have…". Literal to the
  English original's verb and intent, as far as good Interslavic allows.
- **Gender-neutral player.** 2nd-person singular (`ty`), present-tense
  narration; no l-participles or gendered adjectives predicated of the
  player. Sickness and similar states use impersonal datives ("jest ti
  nedobro"). Fixed-gender subjects (strěla, nymph) may use gendered past
  forms freely.
- **Predicative comparatives are adverbial** after change-of-state and
  perception verbs: `čuješ sę silněje`, `staje sę legše` — never the
  agreeing adjective (`legša`). This convention existed implicitly and
  caused a bug when unwritten; it is now law.
- **Terse mode stays telegraphic** (colon-listings fine: `tu: X`,
  `orųžje: X (a)`); verbose mode is real sentences.
- **Effect names**: bare genitive after the class noun where the gerund
  is an official lemma (`napitȯk lěčeńja`); `za` + accusative where it
  isn't (`svitȯk za opoznańje napitkov`) — the neuter accusative equals
  the nominative, an official paradigm cell. This split is permanent
  (re-verified); don't "unify" it.
- **Negation case**: accusative kept in most places (checker-verifiable);
  genitive of negation used where a construction shares a pre-declined
  genitive slot (the miss messages). Match the surrounding pattern.
- Capitalization: raw messages lowercase; the display layer uppercases
  message-initial letters (Unicode-aware); mid-string capitals via the
  marker `:U` flag. Proper names (`Temnice Pohibeli`, `Amulet Jendora`)
  carry `:U` in their markers.

## 6. The gates — and their blind spots

`./scripts/check_lang.sh` (needs a sibling slovowiki checkout; override
with `SLOVOWIKI=`) runs three stages, all of which must PASS:

1. **Template gate**: renders every template family and runs slovowiki
   `check-text --lexicon game-lexicon.tsv --max-unknown 0` — catches
   unknown words and adjacent-token agreement errors.
2. **Inflection lint** (`lint_inflection.py`): no pre-inflected forms in
   literals. Allowlist (`inflection-allow.txt`) is small and justified.
3. **Government lint** (`lint_government.py`): preposition→marker case
   government checked against `interslavic::preposition_senses` (queried
   live — no hand-copied tables). Multi-sense pairs need a one-line
   annotation in `government-notes.txt`; government across a
   `{placeholder}` needs one too, citing the code contract that declines
   the placeholder.

**Known blind spots — human judgment still required:**
- **Verb valence** (the `udar` double-accusative and `hybiti` lessons)
  — check `verb_info`, consult the GLOSSARY table, extend it.
- **Semantics and register** — every tool verifies forms, none verifies
  that the sentence says the right thing. Compare against the English
  original in the `rogue-rs` legacy repo.
- **Government inside placeholder contracts** — the annotations in
  `government-notes.txt` are promises; when you change what a call site
  passes, re-check the promise.

## 7. Workflow for changes

- **Edit a message**: change the marker template → `cargo test` →
  `python3 scripts/bless.py` (escape-safe; re-blesses `assert_eq`
  expectations to rendered output — `contains`/`starts_with` asserts
  must be updated by hand, grep for the old text) → all three gates →
  log anything notable in GLOSSARY.
- **Add a word**: registry entry in `lang.rs` (metadata from the
  dictionary, not guessed) → `cargo test` regenerates the lexicon (the
  golden test FAILS on drift — commit the regenerated TSV) → gates.
- **Adopt a new crate version**: bump the pin, run everything, treat
  changed output as arriving fixes (bless + GLOSSARY), delete any local
  workaround the release supersedes — the release notes say which.
- Always: branch, PR, and put every changed sentence (English → old →
  new) in the PR description — prose is reviewed as prose.

## 8. Lessons that cost the most to learn

1. **Write conventions down the moment they're decided.** The only
   style bug that survived three reviews (`legša`) existed because a
   convention lived in precedent instead of GLOSSARY.
2. **Verify, don't remember.** Nearly every hand-written form that
   later needed correction had been "known" confidently.
3. **Tools catch forms; tables catch frames; only reading catches
   meaning.** The three-layer defense (gates → GLOSSARY tables → human
   sweeps against the English) exists because each layer has caught
   bugs the others structurally cannot.
4. **When a fence catches its own builder, the fence is working.** The
   lints have flagged their author's fresh text in every pass since
   they were built. That is the desired steady state.
5. **Push fixes to the source of truth.** Every workaround here
   (`⟨vpf3⟩` min-length, `⟨v3h⟩` hints, the SUSPICIOUS set, local
   numeral government) was deleted when the underlying capability moved
   upstream. If you find yourself post-processing crate output, stop
   and file it upstream instead.
