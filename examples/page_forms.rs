//! Webpage form dump (WEBPAGE_PROMPT.md): every inflected surface form that
//! appears in the static Interslavic text of `web/index.html` is produced
//! here by the `interslavic` crate or rendered by `lang::speak` from the
//! same templates the in-game help uses — never typed from memory.
use interslavic::{
    Animacy::*, Case::*, Gender::*, Number::*, Person, PronounStyle, Tense, adj, l_participle,
    noun_with, personal_pronoun, verb,
};
use mrzavec::lang::speak;

fn main() {
    println!("== help renderings (verbatim HELP_ENTRIES templates) ==");
    for (key, template) in [
        ("s", "iskati pasť/⟨a:tajny:dvėri:nom:pl⟩ ⟨n:dvėri:nom:pl⟩"),
        ("i", "pokazati ⟨n:torba:acc⟩"),
        ("e", "jesti ⟨n:jeda:acc⟩"),
        ("W", "nositi ⟨n:brȯnja:acc⟩"),
        ("T", "sjęti ⟨n:brȯnja:acc⟩"),
        ("f", "boriti sę do ⟨n:smŕť:gen⟩ ili skoro do ⟨n:smŕť:gen⟩"),
        ("S", "shraniti ⟨n:igra:acc⟩"),
        ("amulet", "amulet ⟨n:Jendor:gen⟩"),
        ("win", "⟨v2:viděti⟩ ⟨a:dnevny:světlo:acc⟩ ⟨n:světlo:acc⟩"),
        ("welcome", "v ⟨n:temnica:acc:pl:U⟩ ⟨n:pohibel:gen:U⟩"),
        ("score", "⟨a:pȯlny:poběda:nom:U⟩ ⟨n:poběda:nom⟩"),
    ] {
        println!("{key}\t{}", speak(template));
    }

    println!("\n== noun forms ==");
    type NounRow = (
        &'static str,
        interslavic::Gender,
        interslavic::Animacy,
        &'static [(interslavic::Case, interslavic::Number)],
    );
    let nouns: &[NounRow] = &[
        (
            "igra",
            Feminine,
            Inanimate,
            &[
                (Acc, Singular),
                (Gen, Singular),
                (Ins, Singular),
                (Loc, Singular),
                (Gen, Plural),
            ],
        ),
        ("god", Masculine, Inanimate, &[(Gen, Singular)]),
        (
            "universitet",
            Masculine,
            Inanimate,
            &[(Loc, Singular), (Acc, Plural)],
        ),
        ("Kalifornija", Feminine, Inanimate, &[(Gen, Singular)]),
        ("dělo", Neuter, Inanimate, &[(Dat, Singular)]),
        (
            "temnica",
            Feminine,
            Inanimate,
            &[(Acc, Singular), (Loc, Plural), (Gen, Plural)],
        ),
        ("ekran", Masculine, Inanimate, &[(Loc, Singular)]),
        (
            "znak",
            Masculine,
            Inanimate,
            &[(Nom, Singular), (Ins, Plural)],
        ),
        ("sistema", Feminine, Inanimate, &[(Ins, Singular)]),
        ("svět", Masculine, Inanimate, &[(Gen, Singular)]),
        (
            "smŕť",
            Feminine,
            Inanimate,
            &[(Nom, Singular), (Loc, Singular)],
        ),
        (
            "stųpenj",
            Masculine,
            Inanimate,
            &[(Gen, Singular), (Loc, Singular)],
        ),
        ("myslj", Feminine, Inanimate, &[(Nom, Plural)]),
        ("kategorija", Feminine, Inanimate, &[(Acc, Singular)]),
        ("imę", Neuter, Inanimate, &[(Acc, Singular)]),
        ("prěvod", Masculine, Inanimate, &[(Nom, Singular)]),
        (
            "język",
            Masculine,
            Inanimate,
            &[(Acc, Singular), (Loc, Singular)],
        ),
        ("cělj", Feminine, Inanimate, &[(Nom, Singular)]),
        ("povŕhnja", Feminine, Inanimate, &[(Acc, Singular)]),
        ("hod", Masculine, Inanimate, &[(Loc, Plural)]),
        (
            "čudovišče",
            Neuter,
            Inanimate,
            &[(Nom, Plural), (Ins, Plural)],
        ),
        ("bukva", Feminine, Inanimate, &[(Nom, Plural)]),
        ("komnata", Feminine, Inanimate, &[(Acc, Plural)]),
        ("koridor", Masculine, Inanimate, &[(Acc, Plural)]),
        ("zlåto", Neuter, Inanimate, &[(Acc, Singular)]),
        ("prědmet", Masculine, Inanimate, &[(Acc, Plural)]),
        ("poběda", Feminine, Inanimate, &[(Nom, Singular)]),
        ("jeda", Feminine, Inanimate, &[(Gen, Singular)]),
        ("napitȯk", Masculine, Inanimate, &[(Nom, Plural)]),
        ("svitȯk", Masculine, Inanimate, &[(Nom, Plural)]),
        ("pŕstenj", Masculine, Inanimate, &[(Nom, Plural)]),
        ("žezlo", Neuter, Inanimate, &[(Nom, Plural)]),
        ("brȯnja", Feminine, Inanimate, &[(Nom, Singular)]),
        ("pasť", Feminine, Inanimate, &[(Acc, Plural)]),
        ("pokušeńje", Neuter, Inanimate, &[(Gen, Singular)]),
        (
            "komanda",
            Feminine,
            Inanimate,
            &[(Nom, Plural), (Gen, Plural)],
        ),
        ("knopka", Feminine, Inanimate, &[(Nom, Singular)]),
        (
            "spis",
            Masculine,
            Inanimate,
            &[(Nom, Singular), (Acc, Singular)],
        ),
        ("směr", Masculine, Inanimate, &[(Acc, Singular)]),
        ("tipkovnica", Feminine, Inanimate, &[(Ins, Singular)]),
        (
            "amulet",
            Masculine,
            Inanimate,
            &[(Acc, Singular), (Ins, Singular)],
        ),
        (
            "sila",
            Feminine,
            Inanimate,
            &[(Nom, Singular), (Acc, Singular)],
        ),
        (
            "dvėri",
            Feminine,
            Inanimate,
            &[(Nom, Plural), (Acc, Plural)],
        ),
    ];
    for (lemma, g, a, cells) in nouns {
        for (case, number) in *cells {
            println!(
                "{lemma}\t{case:?}:{number:?}\t{}",
                noun_with(lemma, *case, *number, *g, *a)
            );
        }
    }

    println!("\n== adjective forms ==");
    let adjs: &[(
        &str,
        interslavic::Case,
        interslavic::Number,
        interslavic::Gender,
        interslavic::Animacy,
    )] = &[
        ("cěly", Gen, Singular, Masculine, Inanimate),
        ("cěly", Acc, Singular, Feminine, Inanimate),
        ("každy", Nom, Singular, Feminine, Inanimate),
        ("novy", Acc, Singular, Feminine, Inanimate),
        ("slučajny", Acc, Singular, Feminine, Inanimate),
        ("konečny", Nom, Singular, Feminine, Inanimate),
        ("pŕvy", Gen, Singular, Masculine, Inanimate),
        ("věrny", Nom, Singular, Masculine, Inanimate),
        ("međuslovjansky", Acc, Singular, Masculine, Inanimate),
        ("neznajemy", Nom, Plural, Masculine, Inanimate),
        ("ukryty", Acc, Plural, Feminine, Inanimate),
        ("pȯlny", Nom, Singular, Feminine, Inanimate),
        ("pȯlny", Acc, Singular, Masculine, Inanimate),
        ("gotovy", Nom, Singular, Feminine, Inanimate),
        ("glåvny", Nom, Plural, Feminine, Inanimate),
        ("tvoj", Nom, Singular, Feminine, Inanimate),
        ("glųboky", Loc, Singular, Masculine, Inanimate),
    ];
    for (lemma, case, number, g, a) in adjs {
        println!(
            "{lemma}\t{case:?}:{number:?}:{g:?}\t{}",
            adj(lemma, *case, *number, *g, *a)
        );
    }

    println!("\n== verb forms ==");
    use Person::*;
    let verbs: &[(&str, Person, interslavic::Number)] = &[
        ("tvoriti", Third, Singular),
        ("boriti", Second, Singular),
        ("umrěti", Second, Singular),
        ("načinati", Second, Singular),
        ("nositi", Third, Singular),
        ("čekati", Third, Singular),
        ("dvigati", Third, Plural),
        ("dvigati", Second, Singular),
        ("izslědovati", Second, Singular),
        ("bojevati", Second, Singular),
        ("sbirati", Second, Singular),
        ("krěpiti", Third, Singular),
        ("stati", Second, Singular),
        ("znati", Third, Singular),
        ("vyjdti", Third, Singular),
        ("viděti", Third, Singular),
        ("značiti", Third, Singular),
        ("pokazati", Third, Singular),
        ("vråtiti", Third, Singular),
        ("čuti", Third, Singular),
        ("umirati", Second, Singular),
        ("strěgti", Third, Singular),
        ("načinati", Third, Singular),
    ];
    for (lemma, p, n) in verbs {
        println!(
            "{lemma}\t{p:?}:{n:?}\t{}",
            verb(lemma, *p, *n, Masculine, Tense::Present)
        );
    }

    println!("\n== l-participles ==");
    for (lemma, g, n) in [
        ("sȯzdati", Masculine, Plural),
        ("prijdti", Masculine, Singular),
        ("črtati", Feminine, Singular),
        ("širiti", Feminine, Singular),
        ("založiti", Feminine, Plural),
        ("načęti", Feminine, Singular),
    ] {
        println!("{lemma}\t{g:?}:{n:?}\t{}", l_participle(lemma, g, n));
    }

    println!("\n== pronouns ==");
    for (label, form) in [
        (
            "k+Dat3pl",
            personal_pronoun(
                Person::Third,
                Plural,
                Masculine,
                Dat,
                PronounStyle::AfterPreposition,
            ),
        ),
        (
            "s+Ins3sgM",
            personal_pronoun(
                Person::Third,
                Singular,
                Masculine,
                Ins,
                PronounStyle::AfterPreposition,
            ),
        ),
        (
            "Acc2sg-clitic",
            personal_pronoun(
                Person::Second,
                Singular,
                Masculine,
                Acc,
                PronounStyle::Clitic,
            ),
        ),
        (
            "Dat2sg-full",
            personal_pronoun(Person::Second, Singular, Masculine, Dat, PronounStyle::Full),
        ),
        (
            "Nom2sg",
            personal_pronoun(Person::Second, Singular, Masculine, Nom, PronounStyle::Full),
        ),
    ] {
        println!("{label}\t{form:?}");
    }
    println!(
        "toj:Nom:Pl:F\t{:?}",
        interslavic::pronoun("toj", Nom, Plural, Feminine, Inanimate)
    );
    println!(
        "tvoj:Nom:Sg:F\t{:?}",
        interslavic::pronoun("tvoj", Nom, Singular, Feminine, Inanimate)
    );
    println!(
        "tvoj:Acc:Sg:M\t{:?}",
        interslavic::pronoun("tvoj", Acc, Singular, Masculine, Inanimate)
    );
    println!(
        "vtory:Gen:Sg:N\t{}",
        adj("vtory", Gen, Singular, Neuter, Inanimate)
    );
    println!("dva:F\t(dvě — numeral, verify via check-text)");
}
// (l-participle probe appended during page work; folded into main() below.)
