#!/usr/bin/env python3
"""Strict-spelling lint: citation lemmas must match official headwords exactly.

check-text and lint_inflection fold diacritics when matching, so a lemma
written "glad" passes every gate even though the official headword is
"glåd". This lint closes that hole: every lemma referenced in a ⟨…⟩ speech
marker or a lex("…") registry row is compared, at the exact-character
level, against the official headword list (slovowiki data/official-isv.csv)
and the project lexicons. A lemma whose folded form equals a folded
official headword but whose surface differs is a spelling error.

Findings from the 2026-07-22 sweep that motivated this: glad→glåd,
tma→ťma, vkus→vkųs, silny→siľny, plamenj→plåmenj, normalno→normaľno,
paralelny→paraleľny, spuščati→spušćati, ime→imę, and friends.
"""

import csv
import re
import sys
import unicodedata
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SLOVOWIKI = Path(__import__("os").environ.get("SLOVOWIKI", REPO / "../../slovowiki")).resolve()

# Marker slots that carry lemmas. ⟨a:adj:noun:…⟩ carries two.
ONE_LEMMA = {"n", "v1", "v2", "v3", "v3p", "v3pl", "vim", "vpf3", "lp", "pp", "cav", "adv", "ap"}
TWO_LEMMA = {"a", "sup"}

FOLD = {
    "å": "a", "ȯ": "o", "ě": "e", "ę": "e", "ų": "u", "ė": "e",
    "ľ": "l", "ń": "n", "ŕ": "r", "ť": "t", "ď": "d", "ś": "s", "ź": "z",
    "đ": "d", "ć": "c",
}


def fold(word):
    out = []
    for ch in unicodedata.normalize("NFC", word.lower()):
        out.append(FOLD.get(ch, ch))
    return "".join(out)


def official_headwords():
    exact = set()
    with open(SLOVOWIKI / "data" / "official-isv.csv", newline="", encoding="utf-8") as fh:
        for row in csv.DictReader(fh):
            for lemma in row["isv"].split(","):
                lemma = lemma.strip()
                # Strip parenthetical hints like "bolěti (boli)".
                lemma = re.sub(r"\s*\(.*\)$", "", lemma)
                if lemma:
                    exact.add(lemma)
    return exact


def project_lemmas():
    lemmas = set()
    for tsv in [REPO / "game-lexicon.tsv", REPO / "web" / "page-lexicon.tsv"]:
        if tsv.exists():
            for line in tsv.read_text(encoding="utf-8").splitlines():
                if line.strip():
                    lemmas.add(line.split("\t")[0])
    return lemmas


def used_lemmas():
    """(lemma, where) pairs from markers and registry rows in src/."""
    used = []
    for path in sorted((REPO / "src").glob("*.rs")):
        text = path.read_text(encoding="utf-8")
        for lineno, line in enumerate(text.splitlines(), 1):
            for marker in re.findall(r"⟨([^⟩]+)⟩", line):
                parts = marker.split(":")
                kind = parts[0]
                slots = []
                if kind in ONE_LEMMA and len(parts) >= 2:
                    slots = [parts[1]]
                elif kind in TWO_LEMMA and len(parts) >= 3:
                    slots = [parts[1], parts[2]]
                elif kind == "v3h" and len(parts) >= 2:
                    slots = [parts[1]]
                for lemma in slots:
                    if re.fullmatch(r"[^\W\d_]+", lemma):
                        used.append((lemma, f"{path.name}:{lineno}"))
            for lemma in re.findall(r'lex(?:_indecl)?\("([^"]+)"', line):
                used.append((lemma, f"{path.name}:{lineno}"))
    return used


def main():
    official = official_headwords()
    folded_official = {}
    for lemma in official:
        folded_official.setdefault(fold(lemma), set()).add(lemma)
    project = project_lemmas()

    case_slots = {"nom", "acc", "gen", "dat", "loc", "ins", "sg", "pl", "m", "f", "n", "U"}
    failures = []
    seen = set()
    for lemma, where in used_lemmas():
        if not lemma or lemma in case_slots or lemma in seen:
            continue
        seen.add(lemma)
        if lemma in official or lemma in project:
            continue
        candidates = folded_official.get(fold(lemma), set()) - {lemma}
        if candidates:
            failures.append((lemma, where, sorted(candidates)))

    if failures:
        print(f"SPELLING LINT: {len(failures)} lemma(s) diverge from official headwords")
        for lemma, where, candidates in sorted(failures):
            print(f"  {lemma}  ({where})  → official: {', '.join(candidates)}")
        return 1
    print("SPELLING LINT: PASS — all lemmas match official headwords exactly")
    return 0


sys.exit(main())
