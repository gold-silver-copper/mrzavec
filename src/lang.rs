//! Interslavic lexicon and declension helpers.
//!
//! Single source of truth for every Interslavic word the game uses. All
//! surface forms come from the `interslavic` crate — never hand-written
//! inflections (see TRANSLATION_PROMPT.md). Word choices and their trust
//! status are documented in GLOSSARY.md; `game-lexicon.tsv` (the slovowiki
//! check-text project lexicon) is regenerated from these tables by the
//! `regenerate_project_lexicon` test.

use interslavic::{adj, comparative, noun_with, verb_forms, Animacy, Case, Gender, Number};

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
        12 => {
            let blizhe = comparative("blizky").map(|(_, adv)| adv).unwrap_or_default();
            format!("{} {}", gen_sg(&lex("teleportacija", Feminine, Inanimate)), blizhe)
        }                                                                 // teleport to
        _ => gen_sg(&lex("anulacija", Feminine, Inanimate)),                 // cancellation
    }
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
        std::fs::write(
            concat!(env!("CARGO_MANIFEST_DIR"), "/game-lexicon.tsv"),
            body,
        )
        .expect("write game-lexicon.tsv");
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
