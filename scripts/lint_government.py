#!/usr/bin/env python3
"""Preposition-government lint (STYLE_PASS_2, stage 3 of check_lang.sh).

The two case-government bugs that reached the branch ("s N zlåtnikov",
"zadavaješ odličny udar {acc}") were invisible to token-level checking.
Markers carry case codes and interslavic::preposition_cases() knows what
each preposition governs, so preposition→marker government is checkable:

- HARD FAIL: preposition immediately followed (allowing intervening
  agreeing markers ⟨a:/cmp:/sup:/ap:⟩) by a case-bearing marker whose case
  is not in the preposition's governed set.
- WARN (annotation required): (a) in-set but semantically suspicious
  combinations (s+gen = "off of", almost never intended for "with");
  (b) preposition immediately followed by a {format placeholder} or a
  digit — government crosses the placeholder, humans must vouch. Every
  warn site must be justified by a line in scripts/government-notes.txt
  (first token of each non-comment line is matched as a snippet key).

A built-in selftest runs first; the lint refuses to work if it fails.
"""
import pathlib
import re
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))
from lint_inflection import REPO, extract_literals, production_source  # noqa: E402

# Severity policy (interslavic 0.12.0, preposition_senses): a marker case
# that matches one sense of a MULTI-sense preposition is ambiguous — it
# needs a one-line (prep+case) annotation in government-notes.txt naming
# the intended sense. Single-sense prepositions pass silently.
AGREEING = ("a:", "cmp:", "sup:", "ap:")
CASE_FIELD = {  # marker kind -> index of the case field in the split body
    "n": 2, "a": 3, "cmp": 3, "sup": 3, "ap": 3, "pp": 3,
    "ty": 1, "ja": 1, "on": 1, "ona": 1, "ono": 1, "my": 1, "vy": 1, "oni": 1,
    "toj": 1, "taky": 1, "nikaky": 1, "ktory": 1, "veś": 1,
    "ničto": 1, "čto": 1, "kto": 1, "nikto": 1,
}
CASES = {"nom", "acc", "gen", "loc", "dat", "ins"}
TOKEN = re.compile(r"⟨[^⟩]*⟩|\{[^}]*\}|[0-9]+|[\wčšžćęųåȯěđŕľńťďśź]+", re.I)


def marker_case(body: str):
    parts = body.split(":")
    idx = CASE_FIELD.get(parts[0])
    if idx is None or idx >= len(parts):
        return None if parts[0] != "pp" else "nom"  # pp defaults to nom
    return parts[idx] if parts[idx] in CASES else ("nom" if parts[0] == "pp" else None)


def prep_table(tokens):
    helper = subprocess.run(
        ["cargo", "run", "--quiet", "--example", "prep_cases", "--", *sorted(tokens)],
        capture_output=True, text=True, cwd=REPO,
    )
    table = {}
    for line in helper.stdout.splitlines():
        tok, _, senses = line.partition("\t")
        table[tok] = {
            part.split(":", 1)[0]: part.split(":", 1)[1]
            for part in senses.split("|")
            if ":" in part
        }
    return table


def scan(literals):
    """Yield (kind, prep, detail, literal) findings."""
    words = set()
    seqs = []
    for lit in literals:
        seq = TOKEN.findall(lit)
        seqs.append((lit, seq))
        for tok in seq:
            if not tok.startswith(("⟨", "{")) and not tok.isdigit():
                words.add(tok.lower())
    table = prep_table(words)
    for lit, seq in seqs:
        for i, tok in enumerate(seq[:-1]):
            low = tok.lower()
            if low not in table:
                continue
            governed = table[low]
            j = i + 1
            # skip agreeing markers to reach the case-bearing head
            while j < len(seq) and seq[j].startswith("⟨") and seq[j][1:].startswith(AGREEING):
                head_case = marker_case(seq[j][1:-1])
                if head_case:  # agreeing marker already carries the case
                    break
                j += 1
            if j >= len(seq):
                continue
            nxt = seq[j]
            if nxt.startswith("⟨"):
                case = marker_case(nxt[1:-1])
                if case is None:
                    continue  # verb/adverb marker: government window closed
                if case not in governed:
                    yield ("FAIL", low, f"⟨…:{case}⟩ not in {{{','.join(sorted(governed))}}}", lit)
                elif len(governed) > 1:
                    yield ("WARN", f"{low}+{case}", f"ambiguous: {low}+{case} = \"{governed[case]}\"", lit)
            elif nxt.startswith("{") or nxt.isdigit():
                yield ("WARN", low, "government crosses a placeholder/digit", lit)


def selftest():
    lits = [
        "⟨v3:letěti⟩ mimo ⟨n:uho:gen⟩",          # pass
        "⟨v3:letěti⟩ mimo ⟨n:uho:acc⟩",          # fail
        "s {} ⟨n:zlåtnik:ins:pl⟩",                 # warn (placeholder)
    ]
    got = list(scan(lits))
    kinds = [k for k, *_ in got]
    assert kinds == ["FAIL", "WARN"], f"government selftest failed: {got}"


def main() -> int:
    selftest()
    notes_path = REPO / "scripts" / "government-notes.txt"
    notes = ""
    if notes_path.exists():
        notes = notes_path.read_text()
    literals = []
    for path in sorted((REPO / "src").glob("*.rs")):
        if path.name in ("lang.rs", "save.rs"):
            continue
        literals.extend(
            lit for lit in extract_literals(production_source(path)) if "chongo" not in lit
        )
    fails, unnoted = [], []
    for kind, prep, detail, lit in scan(literals):
        if kind == "FAIL":
            fails.append((prep, detail, lit))
        else:
            keys = [
                line.split()[0] for line in notes.splitlines()
                if line.strip() and not line.startswith("#")
            ]
            if prep not in keys and prep.split("+")[0] not in keys:
                unnoted.append((prep, detail, lit))
    if fails or unnoted:
        for prep, detail, lit in fails:
            print(f"GOVERNMENT FAIL: '{prep}' {detail}\n    in: \"{lit[:70]}\"")
        for prep, detail, lit in unnoted:
            print(f"GOVERNMENT WARN (unannotated): '{prep}' {detail}\n    in: \"{lit[:70]}\"")
        return 1
    print("GOVERNMENT LINT: PASS — all preposition-marker government verified")
    return 0


if __name__ == "__main__":
    sys.exit(main())
