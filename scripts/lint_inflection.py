#!/usr/bin/env python3
"""Zero-pre-inflection lint (RUNTIME_INFLECTION_PROMPT.md, Phase 2).

Extracts every string literal from production code, tokenizes, and asks
slovowiki's check-text to analyze each distinct token. A token passes iff:
  (a) its folded surface equals the folded surface of one of its own lemmas
      (citation form; admits adverbs/prepositions/conjunctions/particles,
      which are their own lemmas), or
  (b) it is listed in scripts/inflection-allow.txt.
Everything else — anything analyzed as a non-citation inflected form, and
any diacritic-bearing unknown — fails the lint. ASCII-only unknowns are
treated as code artifacts (keys, identifiers) and ignored: production text
contains no English, and ASCII Interslavic inflections are dictionary-known
and therefore still caught by rule (a).

Exclusions: #[cfg(test)] modules, the SYLLABLES const (gibberish magic
language), format!-placeholder contents.
"""
import json
import pathlib
import re
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
SLOVOWIKI = pathlib.Path(
    __import__("os").environ.get("SLOVOWIKI", REPO.parent.parent / "slovowiki")
)
FOLD = {
    "ě": "e", "ę": "e", "ų": "u", "å": "a", "ȯ": "o", "ė": "e", "ĺ": "l",
    "ľ": "l", "ń": "n", "ŕ": "r", "ť": "t", "ď": "d", "ś": "s", "ź": "z",
    "ć": "č", "đ": "dž",
}
DIACRITIC = re.compile(r"[čšžćęųåȯěđŕľńťďśź]", re.I)
WORD = re.compile(r"[A-Za-zčšžćęųåȯěđŕľńťďśźČŠŽĆĘŲÅȮĚĐŔĽŃŤĎŚŹ]{2,}")


def fold(s: str) -> str:
    return "".join(FOLD.get(c, c) for c in s.lower())


def production_source(path: pathlib.Path) -> str:
    src = path.read_text()
    cut = src.find("#[cfg(test)]\nmod")
    if cut != -1:
        src = src[:cut]
    # drop expect()/panic!/assert message literals — developer diagnostics,
    # never rendered on the game grid
    src = re.sub(r'\.expect\(\s*"(?:[^"\\]|\\.)*"\s*\)', ".expect(X)", src)
    src = re.sub(r'(panic!|unreachable!|assert!|debug_assert!)\(\s*"(?:[^"\\]|\\.)*"', r"\1(X", src)
    # drop the SYLLABLES const block (gibberish, not lexicon words)
    m = re.search(r"const SYLLABLES: &\[&str\] = &\[.*?\];", src, re.S)
    if m:
        src = src[: m.start()] + src[m.end():]
    return src


def extract_literals(src: str):
    """A real single-pass scanner: honors //, /* */, char literals,
    lifetimes, and r#"…"# raw strings, so quote parity never flips."""
    out = []
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        two = src[i : i + 2]
        if two == "//":
            j = src.find("\n", i)
            i = n if j < 0 else j + 1
        elif two == "/*":
            j = src.find("*/", i + 2)
            i = n if j < 0 else j + 2
        elif c == "r" and re.match(r'r#*"', src[i:]):
            m = re.match(r'r(#*)"', src[i:])
            close = '"' + m.group(1)
            j = src.find(close, i + m.end())
            if j < 0:
                break
            out.append(src[i + m.end() : j])
            i = j + len(close)
        elif c == '"':
            j = i + 1
            buf = []
            while j < n:
                if src[j] == "\\":
                    buf.append(src[j : j + 2])
                    j += 2
                elif src[j] == '"':
                    break
                else:
                    buf.append(src[j])
                    j += 1
            out.append("".join(buf))
            i = j + 1
        elif c == "'":
            m = re.match(r"'(\\.|[^'\\])'", src[i:])
            i += m.end() if m else 1  # else: lifetime tick
        else:
            i += 1
    return out


def literal_tokens():
    per_token_sites = {}
    for path in sorted((REPO / "src").glob("*.rs")):
        if path.name in ("lang.rs", "save.rs"):
            # lang.rs IS the inflection engine: its literals are lemmas and
            # case codes, contract-tested in its own unit tests. save.rs
            # strings are internal serialization diagnostics (developer
            # English, never rendered on the game grid).
            continue
        for lit in extract_literals(production_source(path)):
            if "chongo" in lit:
                continue  # version easter egg, verbatim by tradition
            if " " not in lit and ("/" in lit or "." in lit):
                continue  # path / storage-key literal, not language
            # marker bodies are citation lemmas + codes by design; their
            # rendered output is verified by the test suite and the gate
            text = re.sub(r"⟨[^⟩]*⟩", " ", lit)
            text = re.sub(r"\{[^}]*\}", " ", text)
            text = text.replace("\\n", " ").replace("\\t", " ").replace("\\'", "'")
            for tok in WORD.findall(text):
                if tok.isupper():
                    continue  # acronyms / key mnemonics
                per_token_sites.setdefault(tok, set()).add(f"{path.name}: \"{lit[:60]}\"")
    return per_token_sites


def main() -> int:
    allow = set()
    allow_file = REPO / "scripts" / "inflection-allow.txt"
    if allow_file.exists():
        for line in allow_file.read_text().splitlines():
            word = line.split("#", 1)[0].strip()
            if word:
                allow.add(word)

    sites = literal_tokens()
    corpus = REPO / "target" / "lint-tokens.txt"
    corpus.parent.mkdir(exist_ok=True)
    corpus.write_text(".\n".join(sorted(sites)) + ".\n")

    result = subprocess.run(
        [
            str(SLOVOWIKI / "target/release/interslavic-wiktionary-lab"),
            "check-text", str(corpus),
            "--lexicon", str(REPO / "game-lexicon.tsv"),
            "--json",
        ],
        capture_output=True, text=True, cwd=SLOVOWIKI,
    )
    if result.returncode not in (0, 1) or not result.stdout.strip():
        print(result.stderr, file=sys.stderr)
        return 2
    analyses = {}
    for entry in json.loads(result.stdout):
        if isinstance(entry, dict) and entry.get("token"):
            analyses.setdefault(entry["token"], entry)

    violations = []
    for tok in sorted(sites):
        if tok in allow or tok.lower() in allow:
            continue
        info = analyses.get(tok) or analyses.get(tok.lower())
        if info is None:
            continue
        status = info.get("status")
        lemmas = info.get("lemmas") or []
        citation = any(fold(tok) == fold(l) for l in lemmas)
        if status == "unknown":
            if DIACRITIC.search(tok):
                violations.append((tok, "unknown (diacritic-bearing)", sites[tok]))
        elif not citation:
            cells = ", ".join((info.get("analyses") or [])[:3])
            violations.append((tok, f"inflected ({'/'.join(lemmas[:2])}: {cells})", sites[tok]))

    if violations:
        print(f"INFLECTION LINT: {len(violations)} violating token(s)\n")
        for tok, why, where in violations:
            print(f"  {tok}  —  {why}")
            for w in sorted(where)[:3]:
                print(f"      {w}")
        return 1
    print("INFLECTION LINT: PASS — no pre-inflected forms in production literals")
    return 0


if __name__ == "__main__":
    sys.exit(main())
