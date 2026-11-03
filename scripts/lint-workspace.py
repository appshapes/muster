#!/usr/bin/env python3
"""lint-workspace: the CLAUDE.md rules rustc and clippy cannot see (ADR 0003).

Checks, for every crate under crates/:

  lints      Cargo.toml has `[lints] workspace = true`, so the crate inherits
             `unsafe_code = "forbid"` and the clippy denials of the root Cargo.toml
  deps       every direct dependency (normal, dev and build) is a workspace member or on
             the crate's `allowed-dependencies` list in `[package.metadata.muster]`; a new
             dependency needs the entry, a justification in the commit message and an ADR
  docs       src/lib.rs and src/main.rs, where present, carry
             `#![deny(missing_docs, rustdoc::broken_intra_doc_links)]`
  prints     `#[expect(clippy::print_stdout` and `print_stderr` appear only in src/output.rs,
             the one module that writes the answer and the error object (ADR 0008)

Python 3.9 standard library only.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOC_LINTS = "#![deny(missing_docs, rustdoc::broken_intra_doc_links)]"
PRINT_EXPECT = re.compile(r"#\[expect\(clippy::print_(stdout|stderr)")

findings: list[str] = []


def find(msg: str) -> None:
    findings.append(msg)


def metadata() -> dict:
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=ROOT, capture_output=True, text=True, check=True,
    ).stdout
    return json.loads(out)


def main() -> int:
    meta = metadata()
    members = {p["name"] for p in meta["packages"]}
    for pkg in meta["packages"]:
        name = pkg["name"]
        manifest = Path(pkg["manifest_path"])
        crate = manifest.parent
        text = manifest.read_text(encoding="utf-8")
        if not re.search(r"^\[lints\]\s*\nworkspace\s*=\s*true", text, re.M):
            find(f"lints: {name}: Cargo.toml must carry `[lints]` with `workspace = true` (ADR 0003)")
        allowed = set(pkg.get("metadata", {}).get("muster", {}).get("allowed-dependencies", []) if pkg.get("metadata") else [])
        for dep in pkg["dependencies"]:
            dname = dep["name"]
            if dname in members or dname in allowed:
                continue
            find(f"deps: {name}: dependency `{dname}` is not on allowed-dependencies in "
                 f"[package.metadata.muster]; add it with a justification in the commit message and an ADR (ADR 0003)")
        for rs in ("src/lib.rs", "src/main.rs"):
            path = crate / rs
            if path.exists() and DOC_LINTS not in path.read_text(encoding="utf-8"):
                find(f"docs: {name}: {rs} must carry `{DOC_LINTS}` (CLAUDE.md, invariant 6)")
        for path in sorted((crate / "src").rglob("*.rs")):
            if PRINT_EXPECT.search(path.read_text(encoding="utf-8")) and path.name != "output.rs":
                find(f"prints: {name}: {path.relative_to(ROOT).as_posix()} expects a print lint; only src/output.rs writes to stdout or stderr (ADR 0008)")
    if findings:
        for f in findings:
            print(f"lint-workspace: {f}")
        print(f"lint-workspace: {len(findings)} finding(s)")
        return 1
    print(f"lint-workspace: ok ({len(meta['packages'])} crate(s))")
    return 0


if __name__ == "__main__":
    sys.exit(main())
