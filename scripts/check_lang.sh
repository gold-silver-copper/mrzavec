#!/bin/sh
# Interslavic verification gate: renders the template corpus, then verifies
# every token with slovowiki's check-text against the project lexicon.
# Requires a slovowiki checkout (default: ../../slovowiki relative to repo).
set -e
cd "$(dirname "$0")/.."
SLOVOWIKI="${SLOVOWIKI:-$(pwd)/../../slovowiki}"
cargo test --lib lang::corpus::write_gate_corpus --quiet
CORPUS="$(pwd)/target/lang-corpus.txt"
LEXICON="$(pwd)/game-lexicon.tsv"
cd "$SLOVOWIKI"
./target/release/interslavic-wiktionary-lab check-text \
    "$CORPUS" --lexicon "$LEXICON" --summary --max-unknown 0 "$@"
# Stage 2: the zero-pre-inflection lint (RUNTIME_INFLECTION_PROMPT.md).
cd - >/dev/null
python3 scripts/lint_inflection.py
# Stage 3: preposition-government lint (STYLE_PASS_2_PROMPT.md).
exec python3 scripts/lint_government.py
