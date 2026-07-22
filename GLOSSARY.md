# mrzavec Interslavic glossary

Word-choice contract for the Interslavic translation. Machine-readable twin:
`game-lexicon.tsv` (regenerated from `src/lang.rs` by the
`regenerate_project_lexicon` test — edit `lang.rs`, never the TSV).

Validated against slovowiki at commit `bf041ca` (schema: forms 4 / en 2 /
notes 1), interslavic crate `0.9.0`. All inflected forms are produced by the
`interslavic` crate at runtime; no hand-written forms anywhere.

Trust legend: **O** = official dictionary word · **G** = slovowiki generated
(unverified reconstruction, probability noted) · **C** = coined for this game
(passed `coin-check`: phonotactics, collision, false friends) · **S** =
deliberate substitution (different concept than the English original, chosen
for Slavic attestation).

## Item classes

| English | ISV | Trust | Notes |
|---|---|---|---|
| potion | napitȯk (m) | O | |
| scroll | svitȯk (m) | O | |
| ring | pŕstenj (m) | O | English key "ring" is a verb trap; found ISV-side |
| wand | žezlo (n) | O | official gloss "scepter" — closest single word |
| staff | posoh (m) | G | pan-Slavic pilgrim's staff; en "staff" is a gloss-token trap |
| amulet | amulet (m) | O | The Amulet of Yendor → "Amulet Jendora" (Jendor: C) |
| food ration | porcija jedy | O | "portion of food" |
| gold piece | zlåtnik (m) | C | transparent zlåto + -nik formation |
| monster | čudovišče (n) | O | |
| trap | pasť (f) | O | |
| fruit (default) | sliva (f) | O | replaces "slime-mold"; user-editable |
| Dungeons of Doom | Temnice Pohibeli | O+G | temnica O; pohibel G(0.20) "doom, perdition" |

## Monsters (glyph unchanged; name need not share the initial)

| Glyph | English | ISV | Trust |
|---|---|---|---|
| A | aquator | akvator (m an) | C |
| B | bat | netopyŕ (m an) | O |
| C | centaur | kitovras (m an) | G(0.13) — Old-Slavic centaur; chosen for flavor |
| D | dragon | drakon (m an) | O |
| E | emu | emu (m an, indecl) | G(0.05) |
| F | venus flytrap | muholovka (f) | O |
| G | griffin | inog (m an) | G(0.13) — Old-Slavic griffin-bird |
| H | hobgoblin | goblin (m an) | G(0.47) |
| I | ice monster | ledeno čudovišče (n) | O (adj deriv) |
| J | jabberwock | žabervok (m an) | C |
| K | kestrel | sokol (m an) | O, **S** (falcon) |
| L | leprechaun | leprekon (m an) | G(0.05) |
| M | medusa | meduza (f an) | G(0.19) |
| N | nymph | nimfa (f an) | O |
| O | orc | ork (m an) | C (reads as "orc" in 3 Slavic langs — intended) |
| P | phantom | fantom (m an) | O |
| Q | quagga | kvaga (f an) | C |
| R | rattlesnake | gremųća zmija (f an) | O (official two-word lemma) |
| S | snake | zmija (f an) | O |
| T | troll | trolj (m an) | O |
| U | black unicorn | črny jednorog (m an) | O |
| V | vampire | vampir (m an) | O |
| W | wraith | prizrak (m an) | O |
| X | xeroc | kserok (m an) | C |
| Y | yeti | jetij (m an) | C — first try "jeti" collided with official jęti (caught by the lexicon loader) |
| Z | zombie | zombi (m an, indecl) | O |

## Weapons

mace→bulava (C; reads as "mace" in 4 Slavic langs), long sword→dȯlgy meč (O),
short bow→kråtky lųk (O), arrow→strěla (O), dagger→kinžal (G 0.09, pan-Slavic),
two handed sword→dvorųčny meč (C adj), dart→drotik (G 0.22),
shuriken→šuriken (C), spear→kopje (O).

## Armor (head nouns brȯnja f / koljčuga f / pancyŕ m, all O)

leather→kožana brȯnja, ring mail→koljčna brȯnja (G adj ← koljce),
studded leather→okovana brȯnja (okovany = iron-bound; participle of O kovati),
scale→luskova brȯnja, chain mail→koljčuga, splint→šinova brȯnja,
banded→pasova brȯnja, plate mail→pancyŕ (O: "coat of armour").

## Traps

trapdoor→laz (G 0.21), arrow→strělna pasť, sleeping gas→sȯnna pasť,
bear→medvěďja pasť, teleport→teleportna pasť (C adj), poison dart→jadna pasť,
rust→rđava pasť, mysterious→tajemna pasť. English articles dropped (no
articles in Interslavic).

## Magic-effect names

Rendered as genitives after the class noun ("napitȯk lěčeńja"), built at
runtime in `lang.rs` from official verbs' gerunds (`verb_forms(v).gerund`)
and lexicon nouns. Unverified effect nouns carried as project rows:
povyšeńje (raise level), slěpota (blindness), levitacija (G 0.17),
teleportacija (G 0.14), lovkosť (G 0.14, dexterity), regeneracija (G 0.13),
nevidimosť (G 0.90 derivative of O nevidimy). "stealth" → tišina (O,
"silence") — deliberate rephrasing. "teleport to" → "teleportacije bliže"
(comparative adverb from the crate).

## Appearance vocabulary

- **Colors** = 27 agreeing adjectives (O where verified; bagrovy,
  akvamarinovy, smaragdovy, rubinovy, lazurny, mlěčny, pepelny, temny,
  světly are project rows). English exotics cyan/ecru/plaid **substituted**.
- **Stones / woods / metals** = nouns rendered as "iz <Gen>" material
  phrases ("žezlo iz kedra"). Exotics without Slavic words substituted with
  attested materials (S): e.g. teak/zebrawood/manzanita → tis, rębina,
  listvenica, cesmina…; pewter→kositer, aluminum→aluminij; kryptonite kept
  as joke coinage kriptonit (C). Full lists in `lang.rs`.
- **Scroll syllables**: replaced with Slavic-flavored gibberish (see
  `game.rs::SYLLABLES`) — untranslatable magic language, not lexicon words.

## Grammar decisions

- No articles; a/an machinery deleted.
- Counts: 1 → Nom sg; 2–4 → Nom pl; 5+ → Gen pl (`lang::counted`).
- Player-directed narration stays 2nd-person singular present (`ty`),
  avoiding l-participle gender in the common path.
- Monster subjects capitalize at render time (existing `uppercase_first`).
- "by X" (passive agent) → instrumental; "X's" (possessor) → genitive;
  direct objects → accusative (animacy-sensitive via the crate).

## Review pass (steen-legacy + check-text, 2026-07-20)

Every hand-written literal in game.rs/main.rs/score.rs was run through
slovowiki check-text and cross-checked against steen.free.fr grammar
(cloned untracked as `steen-legacy/`). ~40 corrections: verb forms
(leti, padaje, udarjaje, prěstavaješ; unofficial promašati→hybiti,
zamahati→mahati, odskakuje→odskoči, blyskati→světi, oslabjaje),
etymological spellings (vȯzduhu, vysoky, Pŕvo, råzpadaje sę),
vocabulary (iměje→imaje, snęti→sjęti, kȯždoj→každoj, rameno→ramę,
dosta→dosť, ura→hura, mag→čarovnik, boli→bolja, mihajųći→migajųći,
stųpnja→stųpenja, odrazu→naglo, koristanju→koristaješ), impersonal
"jest ti nedobro" for sickness, gender-neutral comparative adverbs
(silněje/slaběje/lěpje instead of masculine silnějši/slabši/lovkějši),
hunger status Nemoć. Steen verbs.html confirms -ati 3sg in -aje
(contracted -a is a variant; the dictionary standard uses -aje).

## Runtime-inflection pass (interslavic 0.10.0, 2026-07-20)

Zero pre-inflected forms policy implemented: message literals now carry
⟨…⟩ markers (citation lemmas + cell codes) rendered by `lang::speak()`
through the crate at the message sinks; `scripts/lint_inflection.py`
(stage 2 of check_lang.sh) enforces it permanently.

Crate corrections adopted over previous literals (the crate's
parity-verified output wins): hočeš→**hoćeš**, izgledaje→**izględaje**,
nepravilna→**nepraviľna**, slabša→**slabějša**, stųpenja→**stųpene**
(both valid byforms; crate's first variant), "po vsěm tělě"→**"po vsem
tělu"** (tělě was accidentally the word *telę* 'calf'!), "dva
pŕstenja"→**"dva pŕstenji"** (proper 2–4 numeral government),
ukradla→(unchanged, via paradigm path — see bug below).

Upstream bug found and reported: `interslavic::l_participle("ukrasti",
F, Sg)` returns "ukrasla", diverging from the crate's own compound-tense
paradigm ("ukradla", 100% parity-verified, matches the slovowiki index).
Worked around with the ⟨vpf3:…⟩ marker (paradigm-path 3sg perfect,
auxiliary-less variant per the (je)-optional convention). Fix belongs in
interslavic-rs's l_participle stem handling for -sti verbs.

Also: `verb("stajati", …)` gives "stajaje" — resolved upstream in 0.11.0
as CORRECT for the parity standard (the JS reference never contracts OOV
-jati presents; stajati is not a dictionary lemma). mrzavec deliberately
keeps ⟨v3h:stajati:staje⟩: slovowiki's checker (our rendered-text
arbiter) recognizes only "staje", and the contraction is the natural
Slavic form. The slovowiki-vs-interslavic divergence on this row is
documented on the interslavic side.

**interslavic 0.11.0 adopted (2026-07-21)**: the l_participle -sti bug
is fixed upstream (shared stem context); ⟨vpf3⟩ now uses the structured
`perfect_parts` accessor instead of the shortest-variant heuristic;
`vimp` simplified (imperative cells are surface-ready). Full battery
re-verified, zero output changes, zero expectations re-blessed.

Main/score conversion (same pass): further crate corrections blessed —
kake čislo→**kako čislo** (neuter agreement), shranjeńja→**shrånjeńja**
(official shrånjeńje), na stųpenju→**na stųpeni**, Pŕstenje→**Pŕsteni**,
dva pŕstenja→**dva pŕsteni**. Registry additions: věsť, běg, povråt,
čislo, stańje, skala, shrånjeńje, vzęťje, opoznańje. Allowlist: dir/map/
plate (UI placeholder and English diagnostic tokens colliding with
dictionary words). Enforcement: `scripts/check_lang.sh` = template gate
+ inflection lint, both PASS.

## Style pass (2026-07-21)

Colon-listing confirmations upgraded to real sentences now that
runtime inflection covers every case (verbose mode only; terse keeps
telegraphic colons): pickup → "sejčas imaješ X-acc (a)", wield →
"sejčas dŕžiš X-acc", wear/put-on-ring → "naděvaješ X-acc", take-off →
"snimaješ X-acc", drop → "ostavjaješ X-acc", walk-over → "tu leži/ležet
X" (number-agreeing verb), trap found → "nahodiš strělnų pasť"
(accusative trap phrase). `inventory_name` gained a case parameter
(5+ still forces Gen pl per numeral government); the nymph theft now
uses the accusative.

Flavor/idiom restorations: Ken Arnold easter egg back ("naglo znaješ
vse, tako kako Ken Arnold, …"), purse → "tvoja torba staje sę legša",
magic block → "čarovna sila ne pušćaje tę dalje" (dictionary spelling
pušćati), hunger → "čuješ glad" / "načinaješ čuti glad", quit prompt →
"istinno li izhodiš?", direction prompt → "v ktorų strånų?" (standard
interrogative + dictionary spelling stråna), sense-of-loss tautology →
"imaješ divno čuťje utraty", wand of teleport-to → "teleportacije k
tebě".

## Predicative-comparative convention (2026-07-21)

After change-of-state and perception verbs (stavati sę, izgledati,
čuti sę), comparatives are ADVERBIAL (⟨cav:…⟩ → silněje, slaběje,
legše), never agreeing adjectives — matching the established "čuješ sę
silněje" pattern. Root cause of the legša/legše report: the style pass
reached for ⟨cmp⟩ (agreeing adjective — also grammatical, but the
West/South-style pattern) against this convention; both affected sites
fixed ("tvoja torba staje sę legše", "tvoja brȯnja sejčas izgledaje
slaběje").

Follow-up review (same day): two pre-existing case-government bugs
found and fixed — death screens said "s N zlåtnikov" (s + genitive =
'off of'; now instrumental "s N zlåtnikami", matching the quit
screen), and the attack variant "zadavaješ odličny udar {acc}" was a
double accusative (recipient of a blow takes the dative); replaced with
the case-compatible adverbial "odlično udarjaješ {acc}".

## Style pass 2 (2026-07-21)

**Government lint added** (stage 3 of check_lang.sh): preposition→marker
case government is now machine-checked against
`interslavic::preposition_cases()` (queried live via
examples/prep_cases.rs — no hand-copied tables). Result on the current
tree: zero hard failures; 11 placeholder-crossing warns, each annotated
with its code contract in scripts/government-notes.txt.

**Verb-valence audit** (one-time sweep of every template with a verb
marker plus a second case slot — no tool can check valence):

| template verb | frame | slot | verdict |
|---|---|---|---|
| udarjati / raniti / poběđati / nahoditi / imati / viděti / slyšati / čuti | + Acc | Acc | OK |
| dŕžati, okrųžati, zamråžati, oslabjati, prěmagati, loviti, strěgti, smųtiti, ubiti, ukrasti, pušćati, naděvati, snimati, ostavjati | + Acc (tę / name-acc) | Acc | OK |
| škoditi | + Dat | ⟨ty:dat⟩ "ti" | OK |
| liti sę | Dat benefactive + na Acc | ti / na glåvų | OK |
| dostigati | + Gen | stųpenja | OK |
| svŕběti | Acc experiencer ("svrbi tę") | Acc | OK |
| hybiti | **v.intr. per verb_info (interslavic 0.12.0)** — the by-analogy Acc was wrong | bare (terse) / restructured | fixed: verbose misses now use "udarjati mimo + Gen" and "ne udarjati + Gen" |
| zadavati (+Acc +Dat) | REMOVED — replaced by "odlično udarjati" (double-Acc bug) | — | fixed |

**Polish decisions**: terse label → "Kråtka sȯobčeńja" (official lemma
sȯobčeńje; věsť dropped repo-wide, help "repeat last message" unified);
"Kaka moć!" restores the muscle-flex flavor (moć official; avoids the
silněje/sila near-duplicate). Effect-name split is PERMANENT: re-checked
— none of the za-pattern scroll gerunds (dŕžańje, očarovańje, opoznańje,
strašeńje, sȯzdańje, gněvańje, poddŕžańje, odbirańje) are official
lemmas, so their genitives are unverifiable; za+Acc (an official
paradigm cell) stays the correct dodge.

**Corners audit**: usage() fully translated (easter egg aside);
wizard-mode strings clean; save.rs's lint-exemption premise was FALSE —
its two semantic errors reach players (browser-restore message, CLI
stderr), so both are now marker templates ("nepodpirana verzija
sȯhranjeńja", "ne možno obnoviti dokončenų ili mrtvų igrų") and the CLI
restore eprintln speaks.

## Literal-translation pass (STYLE_PASS_3, 2026-07-21)

Owner directive: translate the ACTION ("podbiraješ žȯlty napitȯk", not
"sejčas imaješ…"). Audit of message pairs against the English originals
in the rogue-rs legacy repo. Upgrades applied (English → old → new):

| English | was | now |
|---|---|---|
| you now have X (pickup ×2) | sejčas imaješ X | **podbiraješ X** |
| you moved onto X (×2) | tu leži X | **stųpaješ na X-acc** |
| you suddenly feel very thirsty | naglo hoćeš piti | **naglo čuješ velikų žęđų** |
| What bulging muscles! | Kaka moć! | **Kake myšce!** |
| wrenching sensation in your gut | čuješ bolj po vsem tělu | **čuješ silny bolj v želųdku** |
| much more skillful | vse dělaješ mnogo lěpje | **vse dělaješ mnogo bolje umělo** (analytic comparative, steen-sourced; umělo allowlisted — adverb of official uměti) |
| You faint (×3) | padaješ bez sil | **padaješ v obmråk** (obmråk: slovowiki-generated "swoon", project row, flagged) |
| tingling feeling | koža tę svŕbi | **koža tę mråvi** (mråviti: generated, pan-Slavic ant-crawl idiom, project verb row, flagged) |
| a gush of water hits you on the head | voda lije sę ti na glåvų | **struja vody udarjaje tę v glåvų** |
| your way is magically blocked | čarovna sila ne pušćaje tę dalje | **tvoj pųť jest čarovno zablokovany** |
| vanishes as it hits the ground | padaje i izčezaje | **izčezaje pri udaru o zemjų** |
| a cloak of darkness falls around you | tma okrųžaje tę | **plašč tmy padaje okolo tebe** |
| the veil of darkness lifts | tma izčezaje | **zavěsa tmy izčezaje** |

KEEP (documented, research done): munchies-overpower drug humor
(prěmagaje + Panika! stays — no register-faithful rendering without
inventing words); "faint" verb family absent from dictionary (obmråk
noun construction chosen instead); sick/feel-verbs stay impersonal per
the gender-neutral convention; "welcome to level N" → "dostigaješ
stųpenja N" (the literal greeting takes a gendered participle);
original first-person narrator lines ("I see no way down") →
second-person ("ne vidiš pųť dolu") — deliberate perspective
normalization, the game addresses the player as "ty" throughout.
Sweep status, honestly stated: the remaining pairs were verified
LITERAL across the cumulative review passes (grammar pass, style
passes 1–2, this pass's targeted audit), not in one fresh itemized
sitting — anything found later belongs in this table, not a new pass.

Valence addition: mråviti — Acc experiencer ("koža tę mråvi"), by
analogy with svŕběti; frame unattestable (generated word), accepted
by decision.
New registry nouns: žęđa, myšca, želųdȯk, struja, obmråk*, plašč,
zavěsa (*project-flagged). scripts/bless.py added (escape-safe blessing).

Kill message (2026-07-22, owner request): "you have defeated X" now
renders with the kill verb — ⟨v2:ubiti⟩ "ubiješ {acc}" (narrative
perfective present, same pattern as usneš) — replacing poběđaješ
(defeat), which read as sports-victory register.

## interslavic 0.12.0 adopted (2026-07-22)

- **quantified() owns count government**: lang::counted_in and the gold
  sites delegate the noun form to the crate (adjective agreement mirrors
  the documented policy, guarded by the
  adjective_agreement_matches_quantified consistency test). Real fix:
  end screens said "s 1 zlåtnikami" — quantified gives "s 1 zlåtnikom".
- **hybiti is intransitive** (verb_info exposes the dictionary's
  v.intr.) — the valence audit's by-analogy accusative is corrected:
  verbose miss messages are now "udarjaješ mimo {Gen}" / "mahaješ i
  udarjaješ mimo {Gen}" / "jedva udarjaješ mimo {Gen}" / "ne udarjaješ
  {Gen}" (genitive of negation is used HERE because all four variants
  share one pre-declined genitive target; the elsewhere-kept accusative
  negation convention is unchanged). Terse keeps bare hybiš/hybi.
- **Government lint is senses-driven**: severity comes from
  preposition_senses (multi-sense pairs need a pair-keyed annotation
  naming the intended sense — all 11 pairs annotated with crate
  glosses); the hand-curated SUSPICIOUS set is gone.
- noun_info now makes fruit gender agreement possible (not yet used).

Kill message aspect (2026-07-22, owner request): ubivati (imperfective)
over ubiti — "ubivaješ {acc}", plain present narration consistent with
udarjaješ/raniš, replacing the perfective-present "ubiješ".

interslavic 0.13.0 adopted (2026-07-22): quantified_parts supplies both
the governed noun form and the agreement (case, number) — the local
quantified_case/quantified_number inference and its consistency test
are deleted. Adjectives can no longer desynchronize from nouns by
construction; the last duplicated grammar logic in the game is gone.

## Webpage (WEBPAGE_PROMPT.md, 2026-07-22)

Static Interslavic prose on web/index.html (about/how-to-play sections).
Verified against slovowiki `bf041ca` via the check-text page stage in
scripts/check_lang.sh (`--max-unknown 0`, extraction by
scripts/extract_page_text.py, lexicon = game-lexicon.tsv +
web/page-lexicon.tsv). All inflected surface forms were produced by the
interslavic crate or lang::speak — dump: `cargo run --example page_forms`.

New official words admitted for the page (all **O**, slovowiki
best-verified; game concepts keep their game lemmas):
universitet, Kalifornija, dělo, ekran, sistema, svět, kategorija, imę,
prěvod, język, cělj, povŕhnja, bukva, komnata, koridor, pokušeńje,
komanda, spis, směr, sȯzdati (create), prijdti (come), črtati (draw),
širiti (spread), tvoriti, založiti (found), izslědovati (explore),
bojevati (fight), sbirati (collect), krěpiti (strengthen), strěgti
(protect), značiti, gotovy, konečny, slučajny, cěly, věrny, neznajemy,
ukryty, každy, vtory, međuslovjansky, vkupě, dneś, opęť, toliko, kȯgda,
okolo, potom, poniž, glųboko.

Dissents / notes:
- goal → **cělj** (top hit "gol" is the sport sense); genre → **kategorija**
  ("žanr" unattested, "fantastika" wrong sense); win → phrase built on
  **poběda** to match score.rs's "pȯlna poběda".
- **dvėri, not dvere**: the in-game help rendered ⟨n:dveri:nom:pl⟩ as
  "dvere", which slovowiki does not know (official lemma is dvėri).
  Fixed game-side (2026-07-22): the lang.rs lemma and every template now
  use dvėri, so game and page agree on the dictionary form.
- **Help templates are now gated** (2026-07-22): HELP_ENTRIES moved to
  src/help.rs (shared lib module) and rendered into the gate corpus.
  Fallout fixed: technical loans escape/shell pinned as project rows;
  the 'm' entry reworded to "iti i ⟨ničto:gen⟩ ne vzęti" because vzęťje's
  oblique cases are absent from the official forms index and the loader
  (slovowiki f8dc218) rejects a project row shadowing an official form.
  Re-validated against slovowiki `f8dc218` (which now prints a lexicon
  header before --json output — lint_inflection.py parses past it).
- **Grammar review pass** (steen-verified, 2026-07-22): three fixes on
  the page — "vyjde … vidi" → "… uvidi" (aspect concord in the
  conditional-generic; uviděti attested), "prěvod v język" → "prěvod na
  język" (English "into" calque; pan-Slavic frame is na + acc), and
  "… najde komanda s" → "komanda s nahodi …" (habitual takes the
  imperfective nahoditi; SVO restored where nom=acc would ambiguate).
  Everything else verified fine, including dvě + nom-pl "myslji",
  universal -li past plural, staneš (no sę) vs načęti sę, and predicate
  nominative after byti/stati.
- **prišėl** (prijdti) replaced pristųpiti: the dictionary lemma is the
  phrase "pristųpiti do", so its participle is absent from the forms index.
- Proper names (Mrzavec, Rogue, roguelike, Toy, Wichman, Arnold, Santa,
  Cruz, Berkeley, Unix, Rust, …) are pinned in web/page-lexicon.tsv with
  bare-name glosses so the consistency checker cannot map concepts onto
  them. "Santa Cruz" is parenthesized in prose to keep it out of
  preposition government. BSD is skipped by the extractor as an acronym.

## Full sweep (2026-07-22, slovowiki f8dc218)

Three-reviewer sweep (game messages / screens / vocabulary) plus machine
triage of all ungated warnings. Changes:

**Spelling vs official headwords** (the gates fold diacritics, so these
passed silently; now enforced forever by `scripts/lint_spelling.py`,
stage 4 of check_lang.sh): glad→glåd, tma→ťma, opęt→opęť, vkus(ny)→
vkųs(ny), silny→siľny, silněje→siľněje, normalno→normaľno,
plamenj→plåmenj, prah→pråh, paralelny→paraleľny, pěstry→pestry
(over-etymologized), tancovati→tancevati, spuščati→spušćati (šć!),
prokleto→⟨pp:proklęti:n⟩, pravilny→praviľny, ime→imę, ledeny→leděny,
poględ (was pogled), pråzdny (was prazdny), zamražati (was zamråžati —
the official headword has no å), iti→idti (iti was generated-grade).
oblačȯk: dictionary headword is oblaček, but the crate cannot decline
its mobile e (gives "oblačeku"); replaced with plain oblåk ("v oblåku
dyma") per rule 5 (restructure over hand-fixing forms).

**English leftovers translated**: terse "gold pieces" → quantified
zlåtnik; terse "in use" ×4 → "to uže koristaješ"; browser → prěględka
(official); "suspend" pinned as a project row (command name, like shell).

**Gender-neutrality repairs**: "Začto by ty hotěl piti to?" → "Začto
hoćeš piti to?"; "čuti sę slaby" → ⟨adv:slaby⟩ ("slabo").

**Grammar**: tam (static location) replaces directional tamo in nine
messages (steen correlatives); negated direct objects normalized to the
accusative convention (nositi brȯnjų / taky pŕstenj, viděti čudovišče,
nahoditi pasť) — existential "ne jest + gen" stays; "na tom ničego ne
piše" → "ničto ne jest napisano"; monster hit variant "udarja" →
⟨v3:udarjati⟩ (-aje standard); "je" → "jest"; zlo stvorjeńje → agreeing
markers (stvorjeńje registered); die("pasti") and "vid pasti" →
⟨n:pasť:gen⟩ markers; nom markers in acc slots fixed (tajne dvėri,
kako čislo); verbose prompt "kaky prědmet hoćeš viděti? " gets its
question mark; score header "Pȯlna"→"pȯlna" (no :U mid-line).

**Vocabulary**: omlěvati/omlěti (faint) is now official — replaces
"padaješ v obmråk" ×3; obmråk project row retired. "Top 10 rogueistov"
(undocumented ad-hoc coinage) → "10 mŕzavcev" (official mŕzavec
'villain, scoundrel, rogue' — also the game's namesake). 19 G-grade
entries re-audited: all others confirmed best available.

**Naming**: display name is now Mŕzavec (official spelling) — window
title, page title/aria/prose. Crate/binary/repo/URL stay ASCII
"mrzavec".

**Status line**: Hp→Zdr (zdravje), Exp→Izk (izkušeńje) — status
abbreviations carry no period (footer abbreviations do, with a trailing
period, for the 80-col limit); both classes live in
scripts/inflection-allow.txt. This sentence is the abbreviation
convention of record.

**Valence table addition**: slědovati + Dat (follow) — pan-Slavic
frame (ru следовать кому/чему), used in the passgo option label.

**Known crate/dictionary divergence** (reported, not changed): the
crate declines official pancyŕ as "panciŕ" in every cell including
Nom; rendered text is self-consistent, upstream issue filed against
interslavic-rs alongside the earlier ukrasla report.

**Deliberate keeps**: "skoro do smŕti" (official 'almost' sense,
in-game help wording); pokušeńje; poniž; sjęti; pohibel; the page's
"ne znaje vtorogo pokušeńja" genitive (steen-sanctioned, prose
register).
