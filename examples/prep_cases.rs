//! Government-lint helper: for each argv token, print
//! `token<TAB>case:gloss|case:gloss` when the token is a recognized
//! preposition — the lint's severity policy is driven by the crate's
//! per-case senses (interslavic 0.12.0), never by hand-copied tables.
fn main() {
    for arg in std::env::args().skip(1) {
        if let Some(senses) = interslavic::preposition_senses(&arg) {
            let parts: Vec<String> = senses
                .iter()
                .map(|(case, gloss)| {
                    let code = match case {
                        interslavic::Case::Nom => "nom",
                        interslavic::Case::Acc => "acc",
                        interslavic::Case::Gen => "gen",
                        interslavic::Case::Loc => "loc",
                        interslavic::Case::Dat => "dat",
                        interslavic::Case::Ins => "ins",
                    };
                    format!("{code}:{gloss}")
                })
                .collect();
            println!("{arg}\t{}", parts.join("|"));
        }
    }
}
