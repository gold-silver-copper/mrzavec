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
| hybiti | frame unattested in dictionary; Acc by analogy with udarjati | Acc | accepted by decision |
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
