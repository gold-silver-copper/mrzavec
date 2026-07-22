#!/usr/bin/env python3
"""Extract human-visible Interslavic text from web/index.html (WEBPAGE_PROMPT.md).

Emits one line per text block to stdout for slovowiki check-text. Skipped on
purpose: <style> and <kbd> contents (CSS and key letters), tokens containing
digits (versions, level numbers), all-caps acronyms (BSD), and JS string
literals without non-ASCII letters (event names, paths — Interslavic prose
always carries at least one diacritic).
"""

import re
from html.parser import HTMLParser
from pathlib import Path

SKIP_TAGS = {"style", "kbd"}


class Extract(HTMLParser):
    def __init__(self):
        super().__init__()
        self.skip = 0
        self.script = 0
        self.blocks = []

    def handle_starttag(self, tag, attrs):
        if tag in SKIP_TAGS:
            self.skip += 1
        if tag == "script":
            self.script += 1
        for key, value in attrs:
            if key == "aria-label" and value:
                self.blocks.append(value)

    def handle_endtag(self, tag):
        if tag in SKIP_TAGS:
            self.skip = max(0, self.skip - 1)
        if tag == "script":
            self.script = max(0, self.script - 1)

    def handle_data(self, data):
        if self.skip:
            return
        if self.script:
            for match in re.findall(r'"((?:[^"\\]|\\.)*)"|`((?:[^`\\]|\\.)*)`', data):
                literal = match[0] or match[1]
                literal = re.sub(r"\$\{[^}]*\}", " ", literal)
                if re.search(r"[^\x00-\x7f]", literal):
                    self.blocks.append(literal)
        elif data.strip():
            self.blocks.append(data)


def clean(block):
    """Drop digit-bearing tokens and acronyms but keep punctuation, so
    check-text still sees sentence boundaries for its agreement pass."""

    def keep(match):
        tok = match.group(0)
        if any(ch.isdigit() for ch in tok):
            return ""
        if tok.isupper() and len(tok) > 1:
            return ""
        return tok

    out = re.sub(r"[^\W_]+", keep, block)
    return re.sub(r"\s+", " ", out).strip()


def main():
    path = Path(__file__).resolve().parent.parent / "web" / "index.html"
    parser = Extract()
    parser.feed(path.read_text(encoding="utf-8"))
    for block in parser.blocks:
        text = clean(block)
        if re.search(r"[^\W\d_]", text):
            print(text)


main()
