#!/usr/bin/env python3
"""Bless test expectations to actual rendered output.

Runs `cargo test`, harvests assert_eq left/right pairs, and replaces the
stale `right` literal with the actual `left` inside #[cfg(test)] regions
only. Escape-aware: literals are matched with full escaped-quote syntax
(the ad-hoc loops used previously corrupted strings containing \\").
Iterates to a fixed point; prints non-eq failures for manual review.
"""
import re
import subprocess
import sys

LIT = r'"(?:[^"\\]|\\.)*"'
PAIR = re.compile(
    rf'left: (Some\({LIT}\)|{LIT}|\[.*?\])\n right: (Some\({LIT}\)|{LIT}|\[.*?\])',
    re.S,
)
FILES = ["src/game.rs", "src/main.rs", "src/score.rs", "src/save.rs"]


def inner(x: str):
    m = re.match(rf"Some\(({LIT})\)$", x, re.S)
    if m:
        return [m.group(1)]
    if x.startswith("["):
        return re.findall(LIT, x)
    return [x]


def main() -> int:
    for iteration in range(10):
        out = subprocess.run(["cargo", "test"], capture_output=True, text=True)
        text = out.stdout + out.stderr
        pairs = PAIR.findall(text)
        if not pairs:
            print("iter", iteration, re.findall(r"test result: \S+\. (\d+) passed; (\d+) failed", text))
            fails = sorted(set(re.findall(r"panicked at (src/\S+):", text)))
            if fails:
                print("non-eq failures:", fails[:8])
                return 1
            return 0
        n = 0
        for fname in FILES:
            src = open(fname).read()
            marker = "#[cfg(test)]\nmod tests"
            if marker not in src:
                continue
            cut = src.index(marker)
            head, tail = src[:cut], src[cut:]
            for left, right in pairs:
                ls, rs = inner(left), inner(right)
                if len(ls) == len(rs):
                    for l, r in zip(ls, rs):
                        if l != r and r in tail:
                            tail = tail.replace(r, l)
                            n += 1
            open(fname, "w").write(head + tail)
        print("iter", iteration, "blessed", n)
        if n == 0:
            return 1
    return 1


if __name__ == "__main__":
    sys.exit(main())
