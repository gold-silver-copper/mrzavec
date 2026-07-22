#!/bin/sh
# Interslavic verification gate: renders the template corpus, then verifies
# every token with slovowiki's check-text against the project lexicon.
# Requires a slovowiki checkout (default: ../../slovowiki relative to repo).
set -e
cd "$(dirname "$0")/.."
REPO="$(pwd)"
SLOVOWIKI="${SLOVOWIKI:-$REPO/../../slovowiki}"
cargo test --lib lang::corpus::write_gate_corpus --quiet
CORPUS="$REPO/target/lang-corpus.txt"
LEXICON="$REPO/game-lexicon.tsv"
cd "$SLOVOWIKI"
./target/release/interslavic-wiktionary-lab check-text \
    "$CORPUS" --lexicon "$LEXICON" --summary --max-unknown 0 "$@"
# Stage 1b: the static webpage text (WEBPAGE_PROMPT.md). Proper names and
# page-only pins live in web/page-lexicon.tsv, merged with the game lexicon.
cd "$REPO"
python3 scripts/extract_page_text.py > target/page-text.txt
cat "$LEXICON" web/page-lexicon.tsv > target/page-lexicon-merged.tsv
cd "$SLOVOWIKI"
./target/release/interslavic-wiktionary-lab check-text \
    "$REPO/target/page-text.txt" --lexicon "$REPO/target/page-lexicon-merged.tsv" \
    --summary --max-unknown 0 "$@"
# Stage 2: the zero-pre-inflection lint (RUNTIME_INFLECTION_PROMPT.md).
cd "$REPO"
python3 scripts/lint_inflection.py
# Stage 3: preposition-government lint (STYLE_PASS_2_PROMPT.md).
python3 scripts/lint_government.py
# Stage 4: strict lemma-spelling lint — the fold-blind gate closer.
exec python3 scripts/lint_spelling.py
