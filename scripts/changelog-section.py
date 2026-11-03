#!/usr/bin/env python3
"""changelog-section: print one version's section of CHANGELOG.md, which is the release's
notes (ADR 0011).

    changelog-section.py X.Y.Z    the section headed "## X.Y.Z", heading left out

Exits 1 with a message when the section does not exist, which is how release.yml refuses
to create a release with no notes. Python 3.9 standard library only.
"""

from __future__ import annotations

import sys
from pathlib import Path


def section(text: str, version: str) -> str | None:
    lines = text.splitlines()
    start = None
    for i, line in enumerate(lines):
        if line.startswith("## ") and line[3:].split(" ")[0] == version:
            start = i + 1
            break
    if start is None:
        return None
    end = len(lines)
    for j in range(start, len(lines)):
        if lines[j].startswith("## "):
            end = j
            break
    return "\n".join(lines[start:end]).strip() + "\n"


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        sys.exit(__doc__)
    path = Path(__file__).resolve().parent.parent / "CHANGELOG.md"
    body = section(path.read_text(encoding="utf-8"), argv[1])
    if body is None or not body.strip():
        print(f"changelog-section: CHANGELOG.md has no section `## {argv[1]}`", file=sys.stderr)
        return 1
    sys.stdout.write(body)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
