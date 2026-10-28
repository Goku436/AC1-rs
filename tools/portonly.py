"""List the movement rules taken from Banned445's port with no evidence from the running game beside them.

Banned445's AC1-Movement-Rewritten is a lead, not AC1 itself: where a rule rests on it alone, it is checked against the
running game (the traced routes in tools/routes/lib, recordings, live comparisons) when one can show it. This lists
the comment blocks in crates/game/src that cite Banned445 and none of the running game, a route, a live comparison, a
recording or a user report, with the item each one documents.

    py tools/portonly.py            # the list
    py tools/portonly.py --count    # the counts by file
"""

import collections
import os
import re
import sys

SRC = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "crates", "game", "src")
EVIDENCE = re.compile(r"running game|route `|live|recording|seen in|user report|trace|measured|in-game|real game", re.I)
ITEM = re.compile(r"(?:fn|const|struct|enum|static|let(?: mut)?)\s+([A-Za-z_][A-Za-z0-9_]*)")
ADDRESS = re.compile(r"0x[0-9A-Fa-f]{5,8}")


def blocks(path):
    lines = open(path, encoding="utf-8").read().split("\n")
    i = 0
    while i < len(lines):
        if not lines[i].strip().startswith("//"):
            i += 1
            continue
        j = i
        while j < len(lines) and lines[j].strip().startswith("//"):
            j += 1
        text = " ".join(l.strip().lstrip("/!").strip() for l in lines[i:j])
        k = j
        while k < len(lines) and not lines[k].strip():
            k += 1
        yield i + 1, text, lines[k].strip() if k < len(lines) else ""
        i = j


def main():
    cited, rows = 0, []
    for name in sorted(os.listdir(SRC)):
        if not name.endswith(".rs"):
            continue
        for line, text, code in blocks(os.path.join(SRC, name)):
            if "Banned445" not in text:
                continue
            cited += 1
            if EVIDENCE.search(text):
                continue
            item = ITEM.search(code)
            address = ADDRESS.search(text)
            rows.append((name, line, item.group(1) if item else "", address.group(0) if address else "", text))
    if "--count" in sys.argv:
        print(f"{cited} comment blocks cite Banned445; {len(rows)} cite nothing from the running game")
        for name, n in collections.Counter(r[0] for r in rows).most_common():
            print(f"  {name:24s} {n}")
        return
    for name, line, item, address, text in rows:
        print(f"{name}:{line}\t{item}\t{address}\t{text[:140]}")


if __name__ == "__main__":
    main()
