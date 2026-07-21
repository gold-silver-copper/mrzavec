//! Government-lint helper: for each argv token, print `token<TAB>cases`
//! (comma-separated lowercase case codes) when the token is a recognized
//! preposition, so the lint queries the crate instead of hand-copying data.
fn main() {
    for arg in std::env::args().skip(1) {
        if let Some(cases) = interslavic::preposition_cases(&arg) {
            let codes: Vec<&str> = cases
                .iter()
                .map(|c| match c {
                    interslavic::Case::Nom => "nom",
                    interslavic::Case::Acc => "acc",
                    interslavic::Case::Gen => "gen",
                    interslavic::Case::Loc => "loc",
                    interslavic::Case::Dat => "dat",
                    interslavic::Case::Ins => "ins",
                })
                .collect();
            println!("{arg}\t{}", codes.join(","));
        }
    }
}
