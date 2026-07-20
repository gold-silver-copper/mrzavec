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
