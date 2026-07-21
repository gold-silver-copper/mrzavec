//! Interslavic lexicon and declension helpers.
//!
//! Single source of truth for every Interslavic word the game uses. All
//! surface forms come from the `interslavic` crate — never hand-written
//! inflections (see TRANSLATION_PROMPT.md). Word choices and their trust
//! status are documented in GLOSSARY.md; `game-lexicon.tsv` (the slovowiki
//! check-text project lexicon) is regenerated from these tables by the
//! `regenerate_project_lexicon` test.

use interslavic::{
    adj, comparative, l_participle, superlative as isv_superlative, noun_with, passive_participle, personal_pronoun, pronoun,
    verb, verb_forms, Animacy, Case, Gender, Number, Person, PronounStyle, Tense,
};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;

/// A declinable noun: dictionary lemma plus the metadata `noun_with` needs.
/// `indecl` marks loanwords that never change form (emu, zombi).
#[derive(Clone, Copy)]
pub struct Lex {
    pub lemma: &'static str,
    pub gender: Gender,
    pub animacy: Animacy,
    pub indecl: bool,
}

/// A noun phrase: optional agreeing adjective + head noun ("dȯlgy meč",
/// "gremųća zmija"). The adjective is stored as its Nom-sg-masc lemma.
#[derive(Clone, Copy)]
pub struct Phrase {
    pub adj: Option<&'static str>,
    pub head: Lex,
}

pub const fn lex(lemma: &'static str, gender: Gender, animacy: Animacy) -> Lex {
    Lex { lemma, gender, animacy, indecl: false }
}
pub const fn lex_indecl(lemma: &'static str, gender: Gender, animacy: Animacy) -> Lex {
    Lex { lemma, gender, animacy, indecl: true }
}
pub const fn ph(adj: Option<&'static str>, head: Lex) -> Phrase {
    Phrase { adj, head }
}

use Animacy::{Animate, Inanimate};
use Gender::{Feminine, Masculine, Neuter};

/// Some crate results pack byform alternatives into one slash-separated
/// string ("oči / očesa"); the game always uses the first variant.
fn first_variant(s: String) -> String {
    match s.split_once(" / ") {
        Some((first, _)) => first.trim().to_string(),
        None => s,
    }
}

/// Decline a lexicon noun. The only place `noun_with` is called.
pub fn decl(l: &Lex, case: Case, number: Number) -> String {
    if l.indecl {
        return l.lemma.to_string();
    }
    first_variant(noun_with(l.lemma, case, number, l.gender, l.animacy))
}

/// Decline an adjective to agree with a lexicon noun.
pub fn adj_for(a: &str, l: &Lex, case: Case, number: Number) -> String {
    first_variant(adj(a, case, number, l.gender, l.animacy))
}

/// Decline a full phrase (agreeing adjective + head noun).
pub fn phrase(p: &Phrase, case: Case, number: Number) -> String {
    match p.adj {
        Some(a) => format!("{} {}", adj_for(a, &p.head, case, number), decl(&p.head, case, number)),
        None => decl(&p.head, case, number),
    }
}

/// Slavic numeral government: 1 → Nom sg, 2–4 → Nom pl, 5+ → Gen pl.
/// Returns just the correctly-numbered noun phrase (caller prepends the digit).
pub fn counted(n: u32, p: &Phrase) -> String {
    match n {
        1 => phrase(p, Case::Nom, Number::Singular),
        2..=4 => phrase(p, Case::Nom, Number::Plural),
        _ => phrase(p, Case::Gen, Number::Plural),
    }
}

/// Genitive singular of a verb's gerund ("lěčiti" → "lěčenja") — the
/// standard "X of <doing>" effect-name pattern. Gerunds are neuter.
/// Gerund nominative/accusative (used after "za" — for neuter nouns the
/// accusative equals this form, which is an official paradigm cell).
pub fn gerund_nom(infinitive: &str) -> String {
    first_variant(verb_forms(infinitive).gerund)
}

pub fn gerund_gen(infinitive: &str) -> String {
    let g = first_variant(verb_forms(infinitive).gerund);
    first_variant(noun_with(&g, Case::Gen, Number::Singular, Neuter, Inanimate))
}

fn gen_sg(l: &Lex) -> String {
    decl(l, Case::Gen, Number::Singular)
}
fn gen_pl(l: &Lex) -> String {
    decl(l, Case::Gen, Number::Plural)
}


// ---------------------------------------------------------------------------
// Speech helpers: the only source of inflected word forms in game text.
// Every form is produced by the `interslavic` crate at call time and
// memoized process-wide (message rendering never recomputes hot cells).
// ---------------------------------------------------------------------------

fn cache() -> &'static Mutex<HashMap<(u8, String, u8), String>> {
    static CACHE: OnceLock<Mutex<HashMap<(u8, String, u8), String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn memo(kind: u8, lemma: &str, cell: u8, produce: impl FnOnce() -> String) -> String {
    let key = (kind, lemma.to_string(), cell);
    let lock = |m: &'static Mutex<HashMap<(u8, String, u8), String>>| {
        m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    };
    if let Some(hit) = lock(cache()).get(&key) {
        return hit.clone();
    }
    let value = produce();
    lock(cache()).insert(key, value.clone());
    value
}

/// 2nd-person-singular present ("udarjaješ") — the narration voice.
pub fn v2(inf: &str) -> String {
    memo(1, inf, 0, || {
        first_variant(verb(inf, Person::Second, Number::Singular, Gender::Masculine, Tense::Present))
    })
}

/// 3rd-person-singular present ("udarjaje").
pub fn v3(inf: &str) -> String {
    memo(1, inf, 1, || {
        first_variant(verb(inf, Person::Third, Number::Singular, Gender::Masculine, Tense::Present))
    })
}

/// 3rd-person-plural present ("udarjajųt").
pub fn v3pl(inf: &str) -> String {
    memo(1, inf, 2, || {
        first_variant(verb(inf, Person::Third, Number::Plural, Gender::Masculine, Tense::Present))
    })
}

/// 1st-person-singular present (wizard-mode voice).
pub fn v1(inf: &str) -> String {
    memo(1, inf, 3, || {
        first_variant(verb(inf, Person::First, Number::Singular, Gender::Masculine, Tense::Present))
    })
}

/// Imperative 2sg ("počivaj") — surface-ready since interslavic 0.11.0.
pub fn vimp(inf: &str) -> String {
    memo(1, inf, 4, || {
        first_variant(
            verb_forms(inf)
                .imperative
                .first()
                .cloned()
                .unwrap_or_else(|| inf.to_string()),
        )
    })
}

/// Bare l-participle for fixed-gender past subjects ("strěla tę ubila").
pub fn lpart(inf: &str, gender: Gender, number: Number) -> String {
    let cell = 10 + gender as u8 * 2 + number as u8;
    memo(2, inf, cell, || first_variant(l_participle(inf, gender, number)))
}

/// Past passive participle agreeing with a lexicon noun ("osvětljena").
pub fn ppart(inf: &str, l: &Lex, case: Case, number: Number) -> String {
    let cell = 40 + case as u8 * 12 + number as u8 * 6 + l.gender as u8 * 2 + l.animacy as u8;
    memo(3, inf, cell, || {
        first_variant(
            passive_participle(inf, case, number, l.gender, l.animacy)
                .unwrap_or_else(|| inf.to_string()),
        )
    })
}

/// Personal pronoun; panics on unattested cells so a template bug is loud.
pub fn pers(person: Person, number: Number, gender: Gender, case: Case, style: PronounStyle) -> String {
    personal_pronoun(person, number, gender, case, style)
        .expect("template requests an unattested personal-pronoun cell")
}

/// Possessive / pronominal determiner agreeing with a lexicon noun
/// ("tvojej torbě": poss("tvoj", &TORBA, Loc, Sg)).
pub fn poss(lemma: &str, l: &Lex, case: Case, number: Number) -> String {
    let cell = 40 + case as u8 * 12 + number as u8 * 6 + l.gender as u8 * 2 + l.animacy as u8;
    memo(4, lemma, cell, || {
        pronoun(lemma, case, number, l.gender, l.animacy)
            .map(first_variant)
            .expect("template requests an undeclinable pronominal determiner")
    })
}

/// Comparative adverb ("bystrěje").
pub fn comp_adv(a: &str) -> String {
    memo(5, a, 0, || {
        comparative(a)
            .map(|(_, adv)| adv)
            .expect("template requests a comparative of a non-gradable adjective")
    })
}

// ---------------------------------------------------------------------------
// Item-class head nouns
// ---------------------------------------------------------------------------

pub const POTION: Lex = lex("napitȯk", Masculine, Inanimate);
pub const SCROLL: Lex = lex("svitȯk", Masculine, Inanimate);
pub const RING: Lex = lex("pŕstenj", Masculine, Inanimate);
pub const WAND: Lex = lex("žezlo", Neuter, Inanimate);
pub const STAFF: Lex = lex("posoh", Masculine, Inanimate);
pub const AMULET: Lex = lex("amulet", Masculine, Inanimate);
pub const FOOD_PORTION: Lex = lex("porcija", Feminine, Inanimate);
pub const FOOD_OF: Lex = lex("jeda", Feminine, Inanimate);
pub const GOLD_COIN: Lex = lex("zlåtnik", Masculine, Inanimate);
pub const MONSTER: Lex = lex("čudovišče", Neuter, Inanimate);
pub const TRAP: Lex = lex("pasť", Feminine, Inanimate);
pub const DEFAULT_FRUIT: &str = "sliva";

// ---------------------------------------------------------------------------
// Monsters — indexed like monster::MONSTERS (A..Z). Glyphs stay A–Z.
// ---------------------------------------------------------------------------

pub const MONSTER_LEX: [Phrase; 26] = [
    ph(None, lex("akvator", Masculine, Animate)),            // A aquator (coined)
    ph(None, lex("netopyŕ", Masculine, Animate)),            // B bat
    ph(None, lex("kitovras", Masculine, Animate)),           // C centaur (generated)
    ph(None, lex("drakon", Masculine, Animate)),             // D dragon
    ph(None, lex_indecl("emu", Masculine, Animate)),         // E emu (indeclinable)
    ph(None, lex("muholovka", Feminine, Inanimate)),         // F venus flytrap
    ph(None, lex("inog", Masculine, Animate)),               // G griffin (generated)
    ph(None, lex("goblin", Masculine, Animate)),             // H hobgoblin (generated)
    ph(Some("ledeny"), lex("čudovišče", Neuter, Inanimate)), // I ice monster
    ph(None, lex("žabervok", Masculine, Animate)),           // J jabberwock (coined)
    ph(None, lex("sokol", Masculine, Animate)),              // K kestrel (subst.: falcon)
    ph(None, lex("leprekon", Masculine, Animate)),           // L leprechaun (generated)
    ph(None, lex("meduza", Feminine, Animate)),              // M medusa (generated)
    ph(None, lex("nimfa", Feminine, Animate)),               // N nymph
    ph(None, lex("ork", Masculine, Animate)),                // O orc (coined)
    ph(None, lex("fantom", Masculine, Animate)),             // P phantom
    ph(None, lex("kvaga", Feminine, Animate)),               // Q quagga (coined)
    ph(Some("gremųći"), lex("zmija", Feminine, Animate)),    // R rattlesnake
    ph(None, lex("zmija", Feminine, Animate)),               // S snake
    ph(None, lex("trolj", Masculine, Animate)),              // T troll
    ph(Some("črny"), lex("jednorog", Masculine, Animate)),   // U black unicorn
    ph(None, lex("vampir", Masculine, Animate)),             // V vampire
    ph(None, lex("prizrak", Masculine, Animate)),            // W wraith
    ph(None, lex("kserok", Masculine, Animate)),             // X xeroc (coined)
    ph(None, lex("jetij", Masculine, Animate)),               // Y yeti (coined)
    ph(None, lex_indecl("zombi", Masculine, Animate)),       // Z zombie (indeclinable)
];

// ---------------------------------------------------------------------------
// Weapons / armor — indexed like item::WEAPON_NAMES / ARMOR_NAMES.
// ---------------------------------------------------------------------------

pub const WEAPON_LEX: [Phrase; 9] = [
    ph(None, lex("bulava", Feminine, Inanimate)),             // mace (project)
    ph(Some("dȯlgy"), lex("meč", Masculine, Inanimate)),      // long sword
    ph(Some("kråtky"), lex("lųk", Masculine, Inanimate)),     // short bow
    ph(None, lex("strěla", Feminine, Inanimate)),             // arrow
    ph(None, lex("kinžal", Masculine, Inanimate)),            // dagger (generated)
    ph(Some("dvorųčny"), lex("meč", Masculine, Inanimate)),   // two handed sword
    ph(None, lex("drotik", Masculine, Inanimate)),            // dart (generated)
    ph(None, lex("šuriken", Masculine, Inanimate)),           // shuriken (coined)
    ph(None, lex("kopje", Neuter, Inanimate)),                // spear
];

pub const ARMOR_LEX: [Phrase; 8] = [
    ph(Some("kožany"), lex("brȯnja", Feminine, Inanimate)),   // leather armor
    ph(Some("koljčny"), lex("brȯnja", Feminine, Inanimate)),  // ring mail
    ph(Some("okovany"), lex("brȯnja", Feminine, Inanimate)),  // studded leather armor
    ph(Some("luskovy"), lex("brȯnja", Feminine, Inanimate)),  // scale mail
    ph(None, lex("koljčuga", Feminine, Inanimate)),           // chain mail
    ph(Some("šinovy"), lex("brȯnja", Feminine, Inanimate)),   // splint mail
    ph(Some("pasovy"), lex("brȯnja", Feminine, Inanimate)),   // banded mail
    ph(None, lex("pancyŕ", Masculine, Inanimate)),            // plate mail
];

// ---------------------------------------------------------------------------
// Traps — indexed like game::TRAP_NAMES (no articles; nominative base).
// ---------------------------------------------------------------------------

pub const TRAP_LEX: [Phrase; 8] = [
    ph(None, lex("laz", Masculine, Inanimate)),               // trapdoor (generated)
    ph(Some("strělny"), lex("pasť", Feminine, Inanimate)),    // arrow trap
    ph(Some("sȯnny"), lex("pasť", Feminine, Inanimate)),      // sleeping gas trap
    ph(Some("medvěďji"), lex("pasť", Feminine, Inanimate)),   // bear trap
    ph(Some("teleportny"), lex("pasť", Feminine, Inanimate)), // teleport trap
    ph(Some("jadny"), lex("pasť", Feminine, Inanimate)),      // poison dart trap
    ph(Some("rđavy"), lex("pasť", Feminine, Inanimate)),      // rust trap
    ph(Some("tajemny"), lex("pasť", Feminine, Inanimate)),    // mysterious trap
];

// ---------------------------------------------------------------------------
// Appearance vocabularies. Colors are adjective lemmas (agree with the item
// noun); stones/woods/metals are nouns used as "iz <Gen>" material phrases.
// ---------------------------------------------------------------------------

pub const COLOR_ADJ: [&str; 27] = [
    "běly", "črny", "črveny", "modry", "sinji", "zeleny", "žȯlty", "oranževy",
    "koričnevy", "sěry", "rozovy", "fioletovy", "zlåty", "srěbrny", "medovy",
    "jasny", "turkysovy", "višnjevy", "bagrovy", "akvamarinovy", "smaragdovy",
    "rubinovy", "lazurny", "mlěčny", "pepelny", "temny", "světly",
];

pub const STONE_LEX: [Lex; 26] = [
    lex("agat", Masculine, Inanimate),
    lex("ametist", Masculine, Inanimate),
    lex("almaz", Masculine, Inanimate),
    lex("smaragd", Masculine, Inanimate),
    lex("granit", Masculine, Inanimate),
    lex("kremenj", Masculine, Inanimate),
    lex("koral", Masculine, Inanimate),
    lex("kvarc", Masculine, Inanimate),
    lex("malahit", Masculine, Inanimate),
    lex("oniks", Masculine, Inanimate),
    lex("opal", Masculine, Inanimate),
    lex("perla", Feminine, Inanimate),
    lex("rubin", Masculine, Inanimate),
    lex("safir", Masculine, Inanimate),
    lex("topaz", Masculine, Inanimate),
    lex("cirkon", Masculine, Inanimate),
    lex("obsidian", Masculine, Inanimate),
    lex("beril", Masculine, Inanimate),
    lex("jaspis", Masculine, Inanimate),
    lex("jantaŕ", Masculine, Inanimate),
    lex("kriptonit", Masculine, Inanimate),
    lex("turmalin", Masculine, Inanimate),
    lex("granat", Masculine, Inanimate),
    lex("akvamarin", Masculine, Inanimate),
    lex("lazurit", Masculine, Inanimate),
    lex("nefrit", Masculine, Inanimate),
];

pub const WOOD_LEX: [Lex; 33] = [
    lex("brěza", Feminine, Inanimate),
    lex("dųb", Masculine, Inanimate),
    lex("lipa", Feminine, Inanimate),
    lex("buk", Masculine, Inanimate),
    lex("vŕba", Feminine, Inanimate),
    lex("jela", Feminine, Inanimate),
    lex("oljha", Feminine, Inanimate),
    lex("jalovec", Masculine, Inanimate),
    lex("osika", Feminine, Inanimate),
    lex("grab", Masculine, Inanimate),
    lex("lěska", Feminine, Inanimate),
    lex("klen", Masculine, Inanimate),
    lex("sosna", Feminine, Inanimate),
    lex("topolja", Feminine, Inanimate),
    lex("smrěk", Masculine, Inanimate),
    lex("drěn", Masculine, Inanimate),
    lex("višnja", Feminine, Inanimate),
    lex("vęz", Masculine, Inanimate),
    lex("boliglåv", Masculine, Inanimate),
    lex("bambus", Masculine, Inanimate),
    lex("orěh", Masculine, Inanimate),
    lex("kedr", Masculine, Inanimate),
    lex("kiparis", Masculine, Inanimate),
    lex("mahagon", Masculine, Inanimate),
    lex("sekvoja", Feminine, Inanimate),
    lex("eukaliptus", Masculine, Inanimate),
    lex("tis", Masculine, Inanimate),
    lex("rębina", Feminine, Inanimate),
    lex("listvenica", Feminine, Inanimate),
    lex("banian", Masculine, Inanimate),
    lex("cesmina", Feminine, Inanimate),
    lex("pekan", Masculine, Inanimate),
    lex("abonos", Masculine, Inanimate),
];

pub const METAL_LEX: [Lex; 22] = [
    lex("aluminij", Masculine, Inanimate),
    lex("berylij", Masculine, Inanimate),
    lex("bronza", Feminine, Inanimate),
    lex("měď", Masculine, Inanimate),
    lex("mosędź", Feminine, Inanimate),
    lex("elektrum", Masculine, Inanimate),
    lex("zlåto", Neuter, Inanimate),
    lex("želězo", Neuter, Inanimate),
    lex("olovo", Neuter, Inanimate),
    lex("magnezij", Masculine, Inanimate),
    lex("rtųť", Feminine, Inanimate),
    lex("nikelj", Masculine, Inanimate),
    lex("platina", Feminine, Inanimate),
    lex("čelik", Masculine, Inanimate),
    lex("srěbro", Neuter, Inanimate),
    lex("silicij", Masculine, Inanimate),
    lex("titan", Masculine, Inanimate),
    lex("volfram", Masculine, Inanimate),
    lex("cink", Masculine, Inanimate),
    lex("kosť", Feminine, Inanimate),
    lex("kositer", Masculine, Inanimate),
    lex("iridij", Masculine, Inanimate),
];

/// "iz <Gen>" material phrase for stones/woods/metals ("žezlo iz kedra").
pub fn material_of(l: &Lex) -> String {
    format!("iz {}", gen_sg(l))
}

/// Stick material: wood for staves, metal for wands.
pub fn stick_material_lex(is_staff: bool, index: usize) -> Lex {
    if is_staff { WOOD_LEX[index] } else { METAL_LEX[index] }
}

/// Decline a word we hold no metadata for (the user-editable fruit name):
/// dictionary lookup with ending-based fallback.
pub fn decl_guess(word: &str, case: Case, number: Number) -> String {
    first_variant(interslavic::noun(word, case, number))
}

/// Genitive of "food" for "porcija jedy".
pub fn food_gen() -> String {
    gen_sg(&FOOD_OF)
}

/// Adverbial/predicative color ("glows red"): neuter Nom sg form.
pub fn color_adv(color: &str) -> String {
    first_variant(adj(color, Case::Nom, Number::Singular, Gender::Neuter, Animacy::Inanimate))
}

/// Masculine Nom color agreeing with "ščit".
pub fn color_masc_nom(color: &str) -> String {
    first_variant(adj(color, Case::Nom, Number::Singular, Gender::Masculine, Animacy::Inanimate))
}

/// Instrumental neuter color agreeing with "světlom".
pub fn color_ins_n(color: &str) -> String {
    first_variant(adj(color, Case::Ins, Number::Singular, Gender::Neuter, Animacy::Inanimate))
}

// ---------------------------------------------------------------------------
// Magic-effect names, always genitive after the class head noun
// ("napitȯk lěčenja"). Every form is produced by the crate at call time.
// ---------------------------------------------------------------------------

const SILA: Lex = lex("sila", Feminine, Inanimate);
const BRONJA: Lex = lex("brȯnja", Feminine, Inanimate);
const ORUZJE: Lex = lex("orųžje", Neuter, Inanimate);

pub fn potion_effect_gen(which: usize) -> String {
    match which {
        0 => gen_sg(&lex("smęteńje", Neuter, Inanimate)),             // confusion
        1 => gen_sg(&lex("halucinacija", Feminine, Inanimate)),          // hallucination
        2 => gen_sg(&lex("jad", Masculine, Inanimate)),                  // poison
        3 => gen_sg(&SILA),                                              // gain strength
        4 => format!("{} nevidimogo", gerund_gen("viděti")),          // see invisible
        5 => gerund_gen("lěčiti"),                                    // healing
        6 => format!("{} {}", gen_sg(&lex("čuťje", Neuter, Inanimate)), gen_pl(&MONSTER)), // monster detection
        7 => format!("{} {}", gen_sg(&lex("čuťje", Neuter, Inanimate)), gen_pl(&lex("čar", Masculine, Inanimate))), // magic detection
        8 => gen_sg(&lex("povyšeńje", Neuter, Inanimate)),               // raise level (project)
        9 => format!("velikogo {}", gerund_gen("lěčiti")),            // extra healing
        10 => gen_sg(&lex("pospěh", Masculine, Inanimate)),              // haste self
        11 => format!("{} {}", gen_sg(&lex("obnovjeńje", Neuter, Inanimate)), gen_sg(&SILA)), // restore strength
        12 => gen_sg(&lex("slěpota", Feminine, Inanimate)),              // blindness (project)
        _ => gen_sg(&lex("levitacija", Feminine, Inanimate)),            // levitation (generated)
    }
}

pub fn scroll_effect_gen(which: usize) -> String {
    match which {
        0 => format!("{} {}", gen_sg(&lex("smęteńje", Neuter, Inanimate)), gen_pl(&MONSTER)), // monster confusion
        1 => format!("čarovnoj {}", gen_sg(&lex("karta", Feminine, Inanimate))), // magic mapping
        2 => format!("za {} {}", gerund_nom("dŕžati"), gen_sg(&MONSTER)),  // hold monster
        3 => gen_sg(&lex("sȯn", Masculine, Inanimate)),                      // sleep
        4 => format!("za {} {}", gerund_nom("očarovati"), gen_sg(&BRONJA)), // enchant armor
        5 => format!("za {} {}", gerund_nom("opoznati"), gen_pl(&POTION)),  // identify potion
        6 => format!("za {} {}", gerund_nom("opoznati"), gen_pl(&SCROLL)),  // identify scroll
        7 => format!("za {} {}", gerund_nom("opoznati"), gen_sg(&ORUZJE)),  // identify weapon
        8 => format!("za {} {}", gerund_nom("opoznati"), gen_sg(&BRONJA)),  // identify armor
        9 => format!(
            "za {} {}, {} ili {}",
            gerund_nom("opoznati"), gen_sg(&RING), gen_sg(&WAND), gen_sg(&STAFF)
        ),                                                                // identify ring, wand or staff
        10 => format!("za {} {}", gerund_nom("strašiti"), gen_pl(&MONSTER)), // scare monster
        11 => format!("{} {}", gen_sg(&lex("čuťje", Neuter, Inanimate)), gen_sg(&FOOD_OF)), // food detection
        12 => gen_sg(&lex("teleportacija", Feminine, Inanimate)),            // teleportation
        13 => format!("za {} {}", gerund_nom("očarovati"), gen_sg(&ORUZJE)), // enchant weapon
        14 => format!("za {} {}", gerund_nom("sȯzdati"), gen_sg(&MONSTER)),     // create monster
        15 => "protiv proklęťju".to_string(),                               // remove curse
        16 => format!("za {} {}", gerund_nom("gněvati"), gen_pl(&MONSTER)),  // aggravate monsters
        _ => format!("{} {}", gen_sg(&lex("ohråna", Feminine, Inanimate)), gen_sg(&BRONJA)), // protect armor
    }
}

pub fn ring_effect_gen(which: usize) -> String {
    match which {
        0 => gen_sg(&lex("ohråna", Feminine, Inanimate)),                    // protection
        1 => gen_sg(&SILA),                                                  // add strength
        2 => format!("za {} {}", gerund_nom("poddŕžati"), gen_sg(&SILA)),       // sustain strength
        3 => gen_sg(&lex("iskańje", Neuter, Inanimate)),                     // searching
        4 => format!("{} nevidimogo", gerund_gen("viděti")),              // see invisible
        5 => gen_sg(&lex("ukrašeńje", Neuter, Inanimate)),                   // adornment
        6 => format!("za {} {}", gerund_nom("gněvati"), gen_pl(&MONSTER)),   // aggravate monster
        7 => gen_sg(&lex("lovkosť", Feminine, Inanimate)),                   // dexterity (generated)
        8 => format!("{} {}", gen_sg(&SILA), gen_sg(&lex("udar", Masculine, Inanimate))), // increase damage
        9 => gen_sg(&lex("regeneracija", Feminine, Inanimate)),              // regeneration (generated)
        10 => format!("pomalogo {}", gen_sg(&lex("travjeńje", Neuter, Inanimate))), // slow digestion
        11 => gen_sg(&lex("teleportacija", Feminine, Inanimate)),            // teleportation
        12 => gen_sg(&lex("tišina", Feminine, Inanimate)),                   // stealth
        _ => format!("{} {}", gen_sg(&lex("ohråna", Feminine, Inanimate)), gen_sg(&BRONJA)), // maintain armor
    }
}

pub fn stick_effect_gen(which: usize) -> String {
    match which {
        0 => gen_sg(&lex("světlo", Neuter, Inanimate)),                      // light
        1 => gen_sg(&lex("nevidimosť", Feminine, Inanimate)),                // invisibility (derived)
        2 => gen_sg(&lex("mȯlnja", Feminine, Inanimate)),                    // lightning
        3 => gen_sg(&lex("ogȯnj", Masculine, Inanimate)),                    // fire
        4 => gen_sg(&lex("hlåd", Masculine, Inanimate)),                     // cold
        5 => gen_sg(&lex("prěobražeńje", Neuter, Inanimate)),                // polymorph
        6 => format!("čarovnoj {}", gen_sg(&lex("strěla", Feminine, Inanimate))), // magic missile
        7 => format!("{} {}", gen_sg(&lex("uskorjeńje", Neuter, Inanimate)), gen_sg(&MONSTER)), // haste monster
        8 => format!("{} {}", gen_sg(&lex("zamedljeńje", Neuter, Inanimate)), gen_sg(&MONSTER)), // slow monster
        9 => format!("za {} {}", gerund_nom("odbirati"), gen_sg(&lex("žiťje", Neuter, Inanimate))), // drain life
        10 => "ničego".to_string(),                                       // nothing (pronoun gen)
        11 => format!("{} prȯč", gen_sg(&lex("teleportacija", Feminine, Inanimate))), // teleport away
        12 => format!(
            "{} k {}",
            gen_sg(&lex("teleportacija", Feminine, Inanimate)),
            pers(Person::Second, Number::Singular, Gender::Masculine, Case::Dat, PronounStyle::Full)
        ),                                                                // teleport to (toward you)
        _ => gen_sg(&lex("anulacija", Feminine, Inanimate)),                 // cancellation
    }
}


// ---------------------------------------------------------------------------
// speak(): the template-marker interpreter. Message literals carry only
// citation-form lemmas inside ⟨…⟩ markers; every surface form is produced
// here, at runtime, by the interslavic crate. Marker grammar:
//   ⟨v1|v2|v3|v3p|vim:INF⟩            finite verb / imperative
//   ⟨lp:INF:m|f|n[:pl]⟩               bare l-participle
//   ⟨pp:INF:m|f|n[:CASE][:pl]⟩        past passive participle (inanimate agr)
//   ⟨n:LEMMA:CASE[:pl]⟩               registry noun, declined
//   ⟨a:LEMMA:NOUN:CASE[:pl]⟩          adjective/determiner agreeing with noun
//   ⟨ty:CASE[:f]⟩ ⟨on:CASE[:n]⟩ …     personal pronouns (clitic default,
//                                     :f = full form, :n = after-preposition)
//   ⟨ničto:CASE⟩ ⟨čto:CASE⟩ ⟨kto:CASE⟩ pronoun() closed classes
//   ⟨cav:ADJ⟩                          comparative adverb
// Unknown markers panic loudly in debug (a template bug must not ship).
// ---------------------------------------------------------------------------

fn reg(lemma: &str) -> Option<Lex> {
    static REGISTRY: OnceLock<HashMap<&'static str, Lex>> = OnceLock::new();
    let map = REGISTRY.get_or_init(|| {
        let mut m: HashMap<&'static str, Lex> = HashMap::new();
        let mut put = |l: Lex| {
            m.insert(l.lemma, l);
        };
        for p in MONSTER_LEX.iter().chain(WEAPON_LEX.iter()).chain(ARMOR_LEX.iter()).chain(TRAP_LEX.iter()) {
            put(p.head);
        }
        for l in STONE_LEX.iter().chain(WOOD_LEX.iter()).chain(METAL_LEX.iter()) {
            put(*l);
        }
        for l in [
            POTION, SCROLL, RING, WAND, STAFF, AMULET, FOOD_PORTION, FOOD_OF, GOLD_COIN,
            MONSTER, TRAP, SILA, BRONJA, ORUZJE,
        ] {
            put(l);
        }
        for l in [
            lex("torba", Feminine, Inanimate),
            lex("město", Neuter, Inanimate),
            lex("zemja", Feminine, Inanimate),
            lex("tělo", Neuter, Inanimate),
            lex("ramę", Neuter, Inanimate),
            lex("glåva", Feminine, Inanimate),
            lex("oko", Neuter, Inanimate),
            lex("uho", Neuter, Inanimate),
            lex("rųka", Feminine, Inanimate),
            lex("noga", Feminine, Inanimate),
            lex("šija", Feminine, Inanimate),
            lex("koža", Feminine, Inanimate),
            lex("světlo", Neuter, Inanimate),
            lex("iskra", Feminine, Inanimate),
            lex("linija", Feminine, Inanimate),
            lex("tma", Feminine, Inanimate),
            lex("mgla", Feminine, Inanimate),
            lex("prah", Masculine, Inanimate),
            lex("sok", Masculine, Inanimate),
            lex("vkus", Masculine, Inanimate),
            lex("zapah", Masculine, Inanimate),
            lex("teplo", Neuter, Inanimate),
            lex("směh", Masculine, Inanimate),
            lex("krik", Masculine, Inanimate),
            lex("bolj", Masculine, Inanimate),
            lex("pųť", Masculine, Inanimate),
            lex("prohod", Masculine, Inanimate),
            lex("komnata", Feminine, Inanimate),
            lex("voda", Feminine, Inanimate),
            lex("plamenj", Masculine, Inanimate),
            lex("dym", Masculine, Inanimate),
            lex("oblačȯk", Masculine, Inanimate),
            lex("zvųk", Masculine, Inanimate),
            lex("karta", Feminine, Inanimate),
            lex("parola", Feminine, Inanimate),
            lex("čarovnik", Masculine, Animate),
            lex("čar", Masculine, Inanimate),
            lex("glad", Masculine, Inanimate),
            lex("slabosť", Feminine, Inanimate),
            lex("nedostatȯk", Masculine, Inanimate),
            lex("jedeńje", Neuter, Inanimate),
            lex("ukųs", Masculine, Inanimate),
            lex("utrata", Feminine, Inanimate),
            lex("naboj", Masculine, Inanimate),
            lex("znak", Masculine, Inanimate),
            lex("zamȯk", Masculine, Inanimate),
            lex("fajl", Masculine, Inanimate),
            lex("rezultat", Masculine, Inanimate),
            lex("pozicija", Feminine, Inanimate),
            lex("opcija", Feminine, Inanimate),
            lex("komanda", Feminine, Inanimate),
            lex("verzija", Feminine, Inanimate),
            lex("režim", Masculine, Inanimate),
            lex("igra", Feminine, Inanimate),
            lex("konec", Masculine, Inanimate),
            lex("smŕť", Feminine, Inanimate),
            lex("bog", Masculine, Animate),
            lex("vsemir", Masculine, Inanimate),
            lex("temnica", Feminine, Inanimate),
            lex("pohibel", Feminine, Inanimate),
            lex("prědmet", Masculine, Inanimate),
            lex("stråna", Feminine, Inanimate),
            lex("grob", Masculine, Inanimate),
            lex("denj", Masculine, Inanimate),
            lex("svět", Masculine, Inanimate),
            lex("Jendor", Masculine, Inanimate),
            lex("signal", Masculine, Inanimate),
            lex("obsluga", Feminine, Inanimate),
            lex("prěškoda", Feminine, Inanimate),
            lex("sȯhranjeńje", Neuter, Inanimate),
            lex("čestitańje", Neuter, Inanimate),
            lex("dveri", Feminine, Inanimate),
            lex("stěna", Feminine, Inanimate),
            lex("poběda", Feminine, Inanimate),
            lex("ime", Neuter, Inanimate),
            lex("vȯzduh", Masculine, Inanimate),
            lex("jedinstvo", Neuter, Inanimate),
            lex("ščit", Masculine, Inanimate),
            lex("pogled", Masculine, Inanimate),
            lex("stųpenj", Masculine, Inanimate),
            lex("hlåd", Masculine, Inanimate),
            lex("nos", Masculine, Inanimate),
            lex("udar", Masculine, Inanimate),
            lex("ubod", Masculine, Inanimate),
            lex("blizkosť", Feminine, Inanimate),
            lex("čuťje", Neuter, Inanimate),
            lex("sȯn", Masculine, Inanimate),
            lex("apetit", Masculine, Inanimate),
            lex("jad", Masculine, Inanimate),
            lex("rđa", Feminine, Inanimate),
            // main.rs / score.rs screen text (metadata: slovowiki official-isv.csv)
            lex("sȯobčeńje", Neuter, Inanimate),
            lex("moć", Feminine, Inanimate),
            lex("běg", Masculine, Inanimate),
            lex("povråt", Masculine, Inanimate),
            lex("čislo", Neuter, Inanimate),
            lex("stańje", Neuter, Inanimate),
            lex("skala", Feminine, Inanimate),
            lex("shrånjeńje", Neuter, Inanimate),
            // gerund nominalizations (systematic neuter -ńje, not separate
            // dictionary headwords; declined like any -je neuter)
            lex("vzęťje", Neuter, Inanimate),
            lex("opoznańje", Neuter, Inanimate),
        ] {
            put(l);
        }
        m
    });
    map.get(lemma).copied()
}

fn parse_case(code: &str) -> Option<Case> {
    Some(match code {
        "nom" => Case::Nom,
        "acc" => Case::Acc,
        "gen" => Case::Gen,
        "loc" => Case::Loc,
        "dat" => Case::Dat,
        "ins" => Case::Ins,
        _ => return None,
    })
}

fn parse_gender(code: &str) -> Option<Gender> {
    Some(match code {
        "m" => Masculine,
        "f" => Feminine,
        "n" => Neuter,
        _ => return None,
    })
}

fn render_marker(body: &str) -> Option<String> {
    let parts: Vec<&str> = body.split(':').collect();
    let num = |p: &[&str]| if p.contains(&"pl") { Number::Plural } else { Number::Singular };
    let arg = |i: usize| parts.get(i).copied();
    Some(match *parts.first()? {
        "v1" => v1(arg(1)?),
        "v2" => v2(arg(1)?),
        "v3" => v3(arg(1)?),
        "v3p" => v3pl(arg(1)?),
        "vim" => vimp(arg(1)?),
        "cav" => comparative(arg(1)?).map(|(_, adv)| adv)?,
        // adverb of manner / predicative neuter ("tako kosmično")
        "adv" => first_variant(adj(arg(1)?, Case::Nom, Number::Singular, Neuter, Inanimate)),
        // active present participle agreeing with a registry noun
        "ap" => {
            let noun = reg(arg(2)?)?;
            first_variant(interslavic::active_participle(
                arg(1)?, parse_case(arg(3)?)?, num(&parts[4..]), noun.gender, noun.animacy,
            )?)
        }
        // 3sg perfect, auxiliary-less (the standard drops the 3rd-person
        // auxiliary): structured accessor from interslavic 0.11.0.
        "vpf3" => {
            interslavic::perfect_parts(arg(1)?, Person::Third, Number::Singular, parse_gender(arg(2)?)?)
                .participle
        }
        // 3sg present with an explicit dictionary present-stem hint
        // ("stajati (staje)") for lemmas where blind conjugation misfires
        "v3h" => first_variant(interslavic::verb_with_present_hint(
            arg(1)?, &format!("({})", arg(2)?),
            Person::Third, Number::Singular, Gender::Masculine, Tense::Present,
        )),
        // declined comparative agreeing with a registry noun
        "cmp" => {
            let base = comparative(arg(1)?).map(|(a, _)| a)?;
            let noun = reg(arg(2)?)?;
            first_variant(adj(&base, parse_case(arg(3)?)?, num(&parts[4..]), noun.gender, noun.animacy))
        }
        // declined superlative agreeing with a registry noun
        "sup" => {
            let base = isv_superlative(arg(1)?).map(|(a, _)| a)?;
            let noun = reg(arg(2)?)?;
            first_variant(adj(&base, parse_case(arg(3)?)?, num(&parts[4..]), noun.gender, noun.animacy))
        }
        "lp" => lpart(arg(1)?, parse_gender(arg(2)?)?, num(&parts[3..])),
        "pp" => {
            let l = Lex { lemma: "", gender: parse_gender(arg(2)?)?, animacy: Inanimate, indecl: false };
            let case = match parts.get(3).filter(|c| **c != "pl") {
                Some(c) => parse_case(c)?,
                None => Case::Nom,
            };
            ppart(arg(1)?, &l, case, num(&parts[3..]))
        }
        "n" => decl(&reg(arg(1)?)?, parse_case(arg(2)?)?, num(&parts[3..])),
        "a" => {
            let noun = reg(arg(2)?)?;
            let case = parse_case(arg(3)?)?;
            let n = num(&parts[4..]);
            interslavic::pronoun(arg(1)?, case, n, noun.gender, noun.animacy)
                .map(first_variant)
                .unwrap_or_else(|| adj_for(parts[1], &noun, case, n))
        }
        "ty" | "ja" | "on" | "ona" | "ono" | "my" | "vy" | "oni" => {
            let (person, number, gender) = match parts[0] {
                "ja" => (Person::First, Number::Singular, Masculine),
                "ty" => (Person::Second, Number::Singular, Masculine),
                "on" => (Person::Third, Number::Singular, Masculine),
                "ona" => (Person::Third, Number::Singular, Feminine),
                "ono" => (Person::Third, Number::Singular, Neuter),
                "my" => (Person::First, Number::Plural, Masculine),
                "vy" => (Person::Second, Number::Plural, Masculine),
                _ => (Person::Third, Number::Plural, Masculine),
            };
            let case = parse_case(arg(1)?)?;
            let style = match parts.get(2) {
                Some(&"f") => PronounStyle::Full,
                Some(&"n") => PronounStyle::AfterPreposition,
                Some(_) => return None,
                None => PronounStyle::Clitic,
            };
            personal_pronoun(person, number, gender, case, style)
                .or_else(|| personal_pronoun(person, number, gender, case, PronounStyle::Full))?
        }
        "toj" | "taky" | "nikaky" | "ktory" | "veś" => {
            let case = parse_case(arg(1)?)?;
            let (gender, animacy) = match parts.get(2).copied() {
                Some("f") => (Feminine, Inanimate),
                Some("n") => (Neuter, Inanimate),
                Some("ma") => (Masculine, Animate),
                _ => (Masculine, Inanimate),
            };
            let n = num(&parts[2..]);
            interslavic::pronoun(parts[0], case, n, gender, animacy).map(first_variant)?
        }
        "ničto" | "čto" | "kto" | "nikto" => interslavic::pronoun(
            parts[0], parse_case(arg(1)?)?, Number::Singular, Masculine, Animacy::Animate,
        )
        .map(first_variant)?,
        _ => return None,
    })
}

/// Render a message template: replaces every ⟨…⟩ marker with the
/// crate-produced surface form. Text outside markers passes through.
///
/// Malformed or unknown markers are passed through VERBATIM in release
/// builds (message text can embed user-typed labels and the fruit name,
/// so the renderer must never panic on player-reachable input); debug
/// builds assert loudly so template bugs are caught by the test suite.
pub fn speak(template: &str) -> String {
    if !template.contains('⟨') {
        return template.to_string();
    }
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('⟨') {
        out.push_str(&rest[..start]);
        let after = &rest[start + '⟨'.len_utf8()..];
        let Some(end) = after.find('⟩') else {
            debug_assert!(false, "speak: unterminated marker in {template:?}");
            out.push('⟨');
            rest = after;
            break;
        };
        let body = &after[..end];
        let (inner, upper) = match body.strip_suffix(":U") {
            Some(stripped) => (stripped, true),
            None => (body, false),
        };
        match render_marker(inner) {
            Some(rendered) if upper => {
                let mut chars = rendered.chars();
                if let Some(first) = chars.next() {
                    out.extend(first.to_uppercase());
                    out.push_str(chars.as_str());
                }
            }
            Some(rendered) => out.push_str(&rendered),
            None => {
                debug_assert!(false, "speak: bad marker ⟨{body}⟩ in {template:?}");
                out.push('⟨');
                out.push_str(body);
                out.push('⟩');
            }
        }
        rest = &after[end + '⟩'.len_utf8()..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use interslavic::{Case, Number};

    #[test]
    fn declines_core_nouns() {
        // bat: masc animate → Acc sg = Gen sg
        let bat = &MONSTER_LEX[1];
        assert_eq!(phrase(bat, Case::Nom, Number::Singular), "netopyŕ");
        assert_eq!(phrase(bat, Case::Acc, Number::Singular), "netopyŕa");
        // sword phrase agrees
        let ls = &WEAPON_LEX[1];
        assert_eq!(phrase(ls, Case::Nom, Number::Singular), "dȯlgy meč");
        // feminine animate rattlesnake
        let rs = &MONSTER_LEX[17];
        assert_eq!(phrase(rs, Case::Acc, Number::Singular), "gremųćų zmijų");
        // indeclinables never change
        assert_eq!(phrase(&MONSTER_LEX[4], Case::Ins, Number::Plural), "emu");
    }

    #[test]
    fn counts_follow_numeral_government() {
        let potion = ph(None, POTION);
        assert_eq!(counted(1, &potion), "napitȯk");
        assert_eq!(counted(3, &potion), "napitky");
        assert_eq!(counted(7, &potion), "napitkov");
    }

    #[test]
    fn speech_helpers_produce_expected_forms() {
        use interslavic::{Person, PronounStyle};
        assert_eq!(v2("udarjati"), "udarjaješ");
        assert_eq!(v3("udarjati"), "udarjaje");
        assert_eq!(v3pl("tancovati"), "tancujųt");
        assert_eq!(vimp("počivati"), "počivaj");
        assert_eq!(lpart("ubiti", Gender::Feminine, Number::Singular), "ubila");
        assert_eq!(
            pers(Person::Second, Number::Singular, Gender::Masculine, Case::Acc, PronounStyle::Clitic),
            "tę"
        );
        assert_eq!(
            pers(Person::Third, Number::Singular, Gender::Masculine, Case::Gen, PronounStyle::AfterPreposition),
            "njego"
        );
        let torba = lex("torba", Feminine, Inanimate);
        assert_eq!(poss("tvoj", &torba, Case::Loc, Number::Singular), "tvojej");
        assert_eq!(comp_adv("bystry"), "bystrěje");
        assert_eq!(ppart("opoznati", &lex("žezlo", Neuter, Inanimate), Case::Nom, Number::Singular), "opoznano");
    }

    /// Release-only: player-reachable text must never panic the renderer
    /// (debug builds assert instead — run with `cargo test --release`).
    #[cfg(not(debug_assertions))]
    #[test]
    fn speak_passes_malformed_markers_through() {
        assert_eq!(speak("nazvano «⟨» konec"), "nazvano «⟨» konec");
        assert_eq!(speak("⟨totally:bogus⟩ text"), "⟨totally:bogus⟩ text");
        assert_eq!(speak("⟨n:neregistrovany:gen⟩"), "⟨n:neregistrovany:gen⟩");
    }

    #[test]
    fn speak_renders_markers() {
        assert_eq!(speak("⟨v2:čuti⟩ sę ⟨cav:silny⟩"), "čuješ sę silněje");
        assert_eq!(speak("⟨n:strěla:nom⟩ ⟨ty:acc⟩ ⟨lp:ubiti:f⟩"), "strěla tę ubila");
        assert_eq!(
            speak("v ⟨a:tvoj:torba:loc⟩ ⟨n:torba:loc⟩ ne jest ⟨n:město:gen⟩"),
            "v tvojej torbě ne jest města"
        );
        assert_eq!(speak("mimo ⟨on:gen:n⟩"), "mimo njego");
        assert_eq!(speak("⟨ničto:gen⟩"), "ničego");
        assert_eq!(speak("to jest ⟨pp:opoznati:n⟩"), "to jest opoznano");
        assert_eq!(speak("bez markera"), "bez markera");
    }

    #[test]
    fn effect_names_are_genitive() {
        assert_eq!(potion_effect_gen(5), "lěčeńja");
        assert_eq!(scroll_effect_gen(3), "sna");
        assert_eq!(ring_effect_gen(12), "tišiny");
        assert_eq!(stick_effect_gen(2), "mȯlnje");
    }

    #[test]
    fn all_effect_names_render_nonempty() {
        for i in 0..14 {
            assert!(!potion_effect_gen(i).trim().is_empty(), "potion {i}");
            assert!(!ring_effect_gen(i).trim().is_empty(), "ring {i}");
            assert!(!stick_effect_gen(i).trim().is_empty(), "stick {i}");
        }
        for i in 0..18 {
            assert!(!scroll_effect_gen(i).trim().is_empty(), "scroll {i}");
        }
    }

    /// Regenerates game-lexicon.tsv (the slovowiki check-text project
    /// lexicon) from these tables so the two can never drift. Run
    /// `cargo test regenerate_project_lexicon` after editing the lexicon.
    #[test]
    fn regenerate_project_lexicon() {
        fn g(l: &Lex) -> (&'static str, &'static str) {
            let g = match l.gender {
                Gender::Masculine => "m",
                Gender::Feminine => "f",
                Gender::Neuter => "n",
            };
            let a = match l.animacy {
                Animacy::Animate => "anim",
                Animacy::Inanimate => "inanim",
            };
            (g, a)
        }
        fn noun_into(rows: &mut Vec<String>, l: &Lex, gloss: &str) {
            let (ge, an) = g(l);
            rows.push(format!("{}\tnoun\t{}\t{}\t{}", l.lemma, ge, an, gloss));
        }
        fn adj_into(rows: &mut Vec<String>, a: &str, _gloss: &str) {
            // Attributive adjectives get a neutral gloss so the consistency
            // checker never maps a noun concept onto them.
            rows.push(format!("{a}\tadj\t\t\tattributive"));
        }
        let mut rows: Vec<String> = Vec::new();
        for (p, gl) in MONSTER_LEX.iter().zip([
            "aquator", "bat", "centaur", "dragon", "emu", "venus flytrap", "griffin",
            "hobgoblin", "ice monster", "jabberwock", "kestrel", "leprechaun", "medusa",
            "nymph", "orc", "phantom", "quagga", "rattlesnake", "snake", "troll",
            "black unicorn", "vampire", "wraith", "xeroc", "yeti", "zombie",
        ]) {
            noun_into(&mut rows, &p.head, gl);
            if let Some(a) = p.adj {
                adj_into(&mut rows, a, gl);
            }
        }
        for (p, gl) in WEAPON_LEX.iter().zip([
            "mace", "long sword", "short bow", "arrow", "dagger", "two handed sword",
            "dart", "shuriken", "spear",
        ]) {
            noun_into(&mut rows, &p.head, gl);
            if let Some(a) = p.adj {
                adj_into(&mut rows, a, gl);
            }
        }
        for (p, gl) in ARMOR_LEX.iter().zip([
            "leather armor", "ringmail", "studded armor", "scalemail",
            "chainmail", "splintmail", "bandedmail", "platemail",
        ]) {
            noun_into(&mut rows, &p.head, gl);
            if let Some(a) = p.adj {
                adj_into(&mut rows, a, gl);
            }
        }
        for (p, gl) in TRAP_LEX.iter().zip([
            "trapdoor", "arrow trap", "sleeping gas trap", "bear trap", "teleport trap",
            "poison dart trap", "rust trap", "mysterious trap",
        ]) {
            noun_into(&mut rows, &p.head, gl);
            if let Some(a) = p.adj {
                adj_into(&mut rows, a, gl);
            }
        }
        for a in COLOR_ADJ {
            adj_into(&mut rows, a, "color");
        }
        for l in STONE_LEX.iter() {
            noun_into(&mut rows, l, "stone");
        }
        for l in WOOD_LEX.iter() {
            noun_into(&mut rows, l, "wood");
        }
        for l in METAL_LEX.iter() {
            noun_into(&mut rows, l, "metal");
        }
        for (l, gl) in [
            (&POTION, "potion"), (&SCROLL, "scroll"), (&RING, "ring"), (&WAND, "wand"),
            (&STAFF, "staff"), (&AMULET, "amulet"), (&FOOD_PORTION, "portion"),
            (&FOOD_OF, "food"), (&GOLD_COIN, "gold piece"), (&MONSTER, "monster"),
            (&TRAP, "trap"),
        ] {
            noun_into(&mut rows, l, gl);
        }
        noun_into(&mut rows, &lex(DEFAULT_FRUIT, Feminine, Inanimate), "fruit");
        // effect nouns that are project-grade (not verified official)
        noun_into(&mut rows, &lex("povyšeńje", Neuter, Inanimate), "raise level");
        noun_into(&mut rows, &lex("slěpota", Feminine, Inanimate), "blindness");
        noun_into(&mut rows, &lex("levitacija", Feminine, Inanimate), "levitation");
        noun_into(&mut rows, &lex("teleportacija", Feminine, Inanimate), "teleportation");
        noun_into(&mut rows, &lex("lovkosť", Feminine, Inanimate), "dexterity");
        noun_into(&mut rows, &lex("regeneracija", Feminine, Inanimate), "regeneration");
        noun_into(&mut rows, &lex("nevidimosť", Feminine, Inanimate), "invisibility");
        noun_into(&mut rows, &lex("pohibel", Feminine, Inanimate), "doom");
        noun_into(&mut rows, &lex("Jendor", Masculine, Inanimate), "Yendor");
        // Merge duplicate lemmas (shared head nouns like brȯnja), joining
        // glosses — the check-text loader rejects duplicate rows.
        let mut merged: std::collections::BTreeMap<String, (String, Vec<String>)> =
            std::collections::BTreeMap::new();
        for row in rows {
            let mut cols = row.splitn(5, '\t');
            let lemma = cols.next().unwrap().to_string();
            let mid: String = {
                let pos = cols.next().unwrap();
                let ge = cols.next().unwrap();
                let an = cols.next().unwrap();
                format!("{pos}\t{ge}\t{an}")
            };
            let gloss = cols.next().unwrap().to_string();
            let e = merged.entry(lemma).or_insert_with(|| (mid, Vec::new()));
            if !e.1.contains(&gloss) {
                e.1.push(gloss);
            }
        }
        let body = merged
            .iter()
            .map(|(lemma, (mid, glosses))| format!("{lemma}\t{mid}\t{}", glosses.join(", ")))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        // Golden-file semantics: regenerate on drift, but FAIL so the
        // change must be reviewed and committed (a silently-rewritten
        // lexicon would bypass CI).
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/game-lexicon.tsv");
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        if existing != body {
            std::fs::write(path, &body).expect("write game-lexicon.tsv");
            panic!(
                "game-lexicon.tsv was out of date and has been regenerated — \
                 review and commit the updated file, then re-run the tests"
            );
        }
    }
}

#[cfg(test)]
mod corpus {
    //! Renders a representative corpus of every dynamic template family to
    //! `target/lang-corpus.txt` for the slovowiki check-text gate
    //! (scripts/check_lang.sh). Pure generation — always passes; the gate
    //! itself runs outside cargo.
    use super::*;
    use interslavic::Case::*;
    use interslavic::Number::{Plural, Singular};

    #[test]
    fn write_gate_corpus() {
        let mut out = String::new();
        for p in MONSTER_LEX.iter() {
            for case in [Nom, Acc, Gen, Ins] {
                out.push_str(&phrase(p, case, Singular));
                out.push(' ');
            }
            out.push('\n');
        }
        for p in WEAPON_LEX.iter().chain(ARMOR_LEX.iter()).chain(TRAP_LEX.iter()) {
            for case in [Nom, Acc, Gen] {
                out.push_str(&phrase(p, case, Singular));
                out.push(' ');
            }
            for n in [1u32, 3, 7] {
                out.push_str(&format!("{n} {} ", counted(n, p)));
            }
            out.push_str(".\n");
        }
        for i in 0..14 {
            out.push_str(&format!(
                "napitȯk {}.\npŕstenj {}.\nžezlo {}.\n",
                potion_effect_gen(i),
                ring_effect_gen(i),
                stick_effect_gen(i)
            ));
        }
        for i in 0..18 {
            out.push_str(&format!("svitȯk {}.\n", scroll_effect_gen(i)));
        }
        for c in COLOR_ADJ {
            out.push_str(&format!(
                "{} napitȯk, {} světlo, {} iskry.\n",
                adj_for(c, &POTION, Nom, Singular),
                color_adv(c),
                adj_for(c, &lex("iskra", Gender::Feminine, Animacy::Inanimate), Nom, Plural)
            ));
        }
        for l in STONE_LEX.iter().chain(WOOD_LEX.iter()).chain(METAL_LEX.iter()) {
            out.push_str(&format!("pŕstenj {}. ", material_of(l)));
        }
        out.push('\n');
        std::fs::create_dir_all(concat!(env!("CARGO_MANIFEST_DIR"), "/target")).ok();
        std::fs::write(
            concat!(env!("CARGO_MANIFEST_DIR"), "/target/lang-corpus.txt"),
            out,
        )
        .expect("write corpus");
    }
}
