#!/usr/bin/env python3
"""lint-docs: the documentation gate CLAUDE.md names for invariant 6 (ADR 0006).

Two modes:

  lint-docs.py          every check below, including the change check over a diff range
  lint-docs.py links    the pages, links, ADR and glossary checks only; what `just docs` runs

Checks:

  pages   every page under docs/ is listed in docs/SUMMARY.md, and every page SUMMARY.md
          lists exists, so the book has no orphan and no hole
  links   every relative link in docs/, README.md and crates/*/README.md resolves to a
          file or directory, and every anchor names a heading in its target
  adrs    every file in docs/decisions/ is NNNN-title.md with a matching "# NNNN." title
          and a Status of Proposed, Accepted (optionally "; superseded in part by NNNN"),
          Superseded by NNNN or Rejected; numbers are unique; a Corrected line is in
          the header, as "- **Corrected:** YYYY-MM-DD: <what>" with a real date
          (ADR 0018)
  glossary every term the command tree introduces has a row in docs/glossary.md: the
          noun of every command group (`issue` of `muster issue get`) and of every flag
          (`label` of `--add-label`, `description` of `--description-file`, `git branch
          format` of `--git-branch-format`), read from
          docs/reference/commands.md, which `just lint-commands` holds equal to `muster
          schema`; a one-word command names an action, and the flags in NOT_TERMS name a
          switch or a mode, so neither needs a row. Output fields are not checked:
          `muster schema` lists commands and flags, not the fields an answer carries, so
          their rows stay by hand, and that clause of ADR 0006's Revisit stays open
  change  over the range: a change to a public item (a line with `pub ` in a crate's
          src/), or any change to the command tree (crates/muster/src/main.rs or
          crates/muster/src/cli/), needs a change under docs/ or to a README; an ADR
          that was already Accepted may change only its Status line, or take a
          correction of fact: one or more dated Corrected entries appended to its
          header, every earlier entry kept whole, and its title, its other header
          fields and its Decision section as they were (ADR 0018)

The range: $LINT_DOCS_RANGE if set; in CI, origin/<base>...HEAD for a pull request and
<before>..<after> for a push (HEAD~1..HEAD when <before> is the null commit); on a
laptop, HEAD against the working tree when anything is modified or untracked, else
HEAD~1..HEAD, which checks the commit just made.

Standard library only, like scripts/lint-workspace.py (ADR 0003). Ported from Cavalry's
script of the same name, without its counters, configuration and spec checks, which
muster has no sources for.
"""

from __future__ import annotations

import datetime
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOCS = ROOT / "docs"
SUMMARY = DOCS / "SUMMARY.md"
DECISIONS = DOCS / "decisions"
# The command tree: a change here is a change to what the binary does, which
# docs/reference/commands.md (generated) and the tutorial describe.
COMMAND_TREE_FILES = ("crates/muster/src/main.rs",)
COMMAND_TREE_DIRS = ("crates/muster/src/cli/",)

STATUSES = re.compile(r"^- \*\*Status:\*\* (?:(Proposed|Superseded by \d{4}|Rejected)"
                      r"|(Accepted)(?:; superseded in part by \d{4}(?:, \d{4})*)?)\s*$")
# A header field, `- **Name:** value`, whose continuation lines are indented two spaces.
HEADER_FIELD = re.compile(r"^- \*\*([A-Z][A-Za-z ]*):\*\*")
# Anything that looks like a Corrected line, so that a misspelt one is named, not missed.
LOOSE_CORRECTED = re.compile(r"^\s*[-*]\s*\**\s*corrected\b", re.IGNORECASE)
CORRECTED_LINE = re.compile(r"^- \*\*Corrected:\*\* (\d{4}-\d{2}-\d{2}): \S")
ADR_NAME = re.compile(r"^(\d{4})-[a-z0-9-]+\.md$")
LINK = re.compile(r"\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
REFERENCE = DOCS / "reference" / "commands.md"
GLOSSARY = DOCS / "glossary.md"
# Flags that name a switch or a mode rather than a thing of Linear's, each with the
# reason it needs no glossary row. A new flag is a term unless it is listed here.
NOT_TERMS = {
    "human": "an output mode (ADR 0008)",
    "trace": "an output mode (ADR 0008)",
    "all": "a switch: every state, not only the open ones",
    "cancel": "a switch: the canceled state, whose row is `state`",
    "format": "a switch of `schema`: json or markdown",
    "allow-duplicate": "a switch that turns a guard off",
    "no-comments": "a switch that leaves fields out",
}
# What a flag may carry around its noun: `--description-file`, `--add-label`.
FLAG_SUFFIXES = ("-file",)
FLAG_PREFIXES = ("add-", "remove-")
REF_DEF = re.compile(r"^\s*\[[^\]]+\]:\s*(\S+)", re.M)
PUB_LINE = re.compile(r"^[+-]\s*pub\s")
SCHEME = re.compile(r"^[a-z][a-z0-9+.-]*:")

findings: list[str] = []


def find(msg: str) -> None:
    findings.append(msg)


def git(*args: str, check: bool = True) -> str:
    p = subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True)
    if check and p.returncode != 0:
        sys.exit(f"lint-docs: git {' '.join(args)} failed: {p.stderr.strip()}")
    return p.stdout


def strip_code(text: str) -> str:
    """Drop fenced blocks and inline code: links and headings inside them are not links."""
    text = re.sub(r"^```.*?^```[ \t]*$", "", text, flags=re.S | re.M)
    text = re.sub(r"^~~~.*?^~~~[ \t]*$", "", text, flags=re.S | re.M)
    return re.sub(r"`[^`\n]*`", "", text)


def canon(s: str) -> str:
    """Anchor comparison that survives GitHub's and mdBook's slug differences."""
    return re.sub(r"[^a-z0-9]", "", s.lower())


def headings(path: Path, depth: int = 0) -> set[str]:
    """The headings of a page, and of every page it includes with mdBook's
    `{{#include <path>}}`, since a link to a heading of an included page lands on it
    in the book."""
    out: set[str] = set()
    for line in strip_code(path.read_text(encoding="utf-8")).splitlines():
        m = re.match(r"^#{1,6}\s+(.*?)\s*#*\s*$", line)
        if m:
            title = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", m.group(1))
            out.add(canon(title))
            continue
        include = re.match(r"^\s*\{\{#include\s+([^}:\s]+)[^}]*\}\}\s*$", line)
        if include and depth < 4:
            target = (path.parent / include.group(1)).resolve()
            if target.is_file():
                out |= headings(target, depth + 1)
    return out


def rel(path: Path) -> str:
    return path.relative_to(ROOT).as_posix()


# ---- pages ------------------------------------------------------------------------

def check_pages() -> int:
    if not SUMMARY.exists():
        find("pages: docs/SUMMARY.md is missing")
        return 0
    listed: set[Path] = set()
    for target in LINK.findall(strip_code(SUMMARY.read_text(encoding="utf-8"))):
        target = target.split("#", 1)[0]
        if not target or SCHEME.match(target):
            continue
        p = (SUMMARY.parent / target).resolve()
        listed.add(p)
        if not p.is_file():
            find(f"pages: docs/SUMMARY.md lists {target}, which does not exist")
    pages = sorted(p for p in DOCS.rglob("*.md") if p != SUMMARY)
    for p in pages:
        if p.resolve() not in listed:
            find(f"pages: {rel(p)} is not listed in docs/SUMMARY.md")
    return len(pages)


# ---- links ------------------------------------------------------------------------

def link_sources() -> list[Path]:
    srcs = sorted(DOCS.rglob("*.md")) + [ROOT / "README.md"]
    srcs += sorted((ROOT / "crates").glob("*/README.md"))
    return [p for p in srcs if p.is_file()]


def check_links() -> int:
    n = 0
    for src in link_sources():
        text = strip_code(src.read_text(encoding="utf-8"))
        targets = LINK.findall(text) + REF_DEF.findall(text)
        for target in targets:
            if SCHEME.match(target) or target.startswith("<"):
                continue
            n += 1
            path_part, _, anchor = target.partition("#")
            dest = src if not path_part else (src.parent / path_part).resolve()
            if not dest.exists():
                find(f"links: {rel(src)} links to {target}, which does not exist")
                continue
            if anchor:
                if dest.is_dir() or dest.suffix != ".md":
                    find(f"links: {rel(src)} links to {target}: an anchor on a non-page")
                elif canon(anchor) not in headings(dest):
                    find(f"links: {rel(src)} links to {target}: no heading matches #{anchor}")
    return n


# ---- adrs -------------------------------------------------------------------------

def adr_status(text: str) -> str | None:
    for line in text.splitlines():
        m = STATUSES.match(line)
        if m:
            return m.group(1) or m.group(2)
    return None


def header_lines(text: str) -> list[str]:
    """An ADR's lines after its title, up to its first `## ` heading."""
    out: list[str] = []
    for line in text.splitlines()[1:]:
        if line.startswith("## "):
            break
        out.append(line)
    return out


def header_entries(text: str) -> list[tuple[str, str]]:
    """The header's fields in order, as (name, the entry's whole text with its
    continuation lines)."""
    entries: list[list[str]] = []
    for line in header_lines(text):
        m = HEADER_FIELD.match(line)
        if m:
            entries.append([m.group(1), line])
        elif entries and line.startswith("  "):
            entries[-1][1] += "\n" + line
    return [(name, body) for name, body in entries]


def check_adrs() -> int:
    seen: dict[str, Path] = {}
    files = sorted(p for p in DECISIONS.glob("*.md") if p.name != "0000-template.md")
    for p in files:
        m = ADR_NAME.match(p.name)
        if not m:
            find(f"adrs: {rel(p)} is not named NNNN-short-title.md")
            continue
        num = m.group(1)
        if num in seen:
            find(f"adrs: {rel(p)} reuses number {num} of {rel(seen[num])}")
        seen[num] = p
        text = p.read_text(encoding="utf-8")
        first = text.splitlines()[0] if text else ""
        if not first.startswith(f"# {num}. "):
            find(f"adrs: {rel(p)} first line must start with '# {num}. '")
        if adr_status(text) is None:
            find(f"adrs: {rel(p)} has no '- **Status:**' line with a valid status")
        for problem in corrected_problems(text):
            find(f"adrs: {rel(p)} {problem} (ADR 0018)")
    return len(files)


def corrected_problems(text: str) -> list[str]:
    """What is wrong with an ADR's Corrected lines: one outside the header, one not in
    the form `- **Corrected:** YYYY-MM-DD: <what>`, or one whose date is not a date."""
    problems: list[str] = []
    header = set(header_lines(text))
    for line in text.splitlines():
        if not LOOSE_CORRECTED.match(line):
            continue
        m = CORRECTED_LINE.match(line)
        if not m:
            problems.append(f"has a Corrected line not in the form "
                            f"'- **Corrected:** YYYY-MM-DD: <what>': {line.strip()[:60]}")
            continue
        if line not in header:
            problems.append("has a Corrected line outside its header, which ends at the "
                            "first '## ' heading")
        try:
            datetime.date.fromisoformat(m.group(1))
        except ValueError:
            problems.append(f"has a Corrected line whose date {m.group(1)} is not a date")
    return problems


def decision_section(text: str) -> list[str]:
    """The lines of an ADR's `## Decision` section, up to the next `## ` heading outside
    a fenced code block."""
    out: list[str] = []
    inside = fenced = False
    for line in text.splitlines():
        if line.lstrip().startswith("```"):
            fenced = not fenced
        elif line.startswith("## ") and not fenced:
            inside = line.strip() == "## Decision"
            continue
        if inside:
            out.append(line)
    return out


def without_status(text: str) -> list[str]:
    """An ADR's lines without its Status line, which may always change (ADR 0001)."""
    return [line for line in text.splitlines() if not STATUSES.match(line)]


def fixed_header(text: str) -> list[tuple[str, str]]:
    """The header fields a correction never changes: all but Status and Corrected."""
    return [entry for entry in header_entries(text) if entry[0] not in ("Status", "Corrected")]


def adr_change_problem(before: str, after: str) -> str | None:
    """Why an Accepted ADR may not change from `before` to `after`, or None when it may
    (ADR 0001, ADR 0018). Its Status line may change. Any other change is a correction of
    fact: it appends one or more Corrected entries to the header, keeps every earlier
    entry whole and first, and leaves the title, the other header fields and the
    Decision section as they were."""
    if without_status(before) == without_status(after):
        return None
    if before.splitlines()[:1] != after.splitlines()[:1]:
        return "its title changed"
    if fixed_header(before) != fixed_header(after):
        return "a header field other than Status and Corrected changed"
    old = [body for name, body in header_entries(before) if name == "Corrected"]
    new = [body for name, body in header_entries(after) if name == "Corrected"]
    if new[:len(old)] != old:
        return "an earlier Corrected entry changed or moved"
    if len(new) == len(old):
        return "it changed beyond its Status line without a new Corrected entry"
    if decision_section(before) != decision_section(after):
        return "its Decision section changed, which a correction never does"
    return None


# ---- glossary ---------------------------------------------------------------------

def glossary_terms() -> set[str]:
    """The `Muster term` column of the glossary's one table."""
    terms: set[str] = set()
    for line in GLOSSARY.read_text(encoding="utf-8").splitlines():
        m = re.match(r"^\| ([^|]+?) \|", line)
        if m and m.group(1).strip() not in ("Muster term", "---"):
            terms.add(m.group(1).strip())
    return terms


def flag_noun(flag: str) -> str:
    """The glossary term a flag wants: its name without the prefixes and suffixes
    above, hyphens read as spaces (`--git-branch-format` wants `git branch format`)."""
    for prefix in FLAG_PREFIXES:
        flag = flag.removeprefix(prefix)
    for suffix in FLAG_SUFFIXES:
        flag = flag.removesuffix(suffix)
    return flag.replace("-", " ")


def reference_terms() -> tuple[list[tuple[str, str]], int, int]:
    """Every (term, where) the generated reference introduces: the noun of a command
    group and the noun of a flag, each with the command it was read under (`muster
    --team` for a global flag); and how many commands and flags were parsed, so that a
    reference whose shape moved cannot pass as one with nothing to check."""
    out: list[tuple[str, str]] = []
    command = ""
    commands = flags = 0
    for line in REFERENCE.read_text(encoding="utf-8").splitlines():
        if line.startswith("## Global flags"):
            command = ""
            continue
        m = re.match(r"^### muster (.+?)\s*$", line)
        if m:
            command = m.group(1)
            commands += 1
            words = command.split()
            if len(words) > 1:
                out.append((words[0], f"muster {command}"))
            continue
        m = re.match(r"^\| `--([^` ]+)` \|", line)
        if m:
            flags += 1
            if m.group(1) not in NOT_TERMS:
                where = " ".join(part for part in ("muster", command, f"--{m.group(1)}") if part)
                out.append((flag_noun(m.group(1)), where))
    return out, commands, flags


def check_glossary() -> int:
    for path, fix in ((REFERENCE, "run just gen-commands"), (GLOSSARY, "restore it")):
        if not path.exists():
            find(f"glossary: {rel(path)} is missing; {fix}")
            return 0
    terms, commands, flags = reference_terms()
    if commands == 0 or flags == 0:
        find(f"glossary: {commands} commands and {flags} flags parsed from {rel(REFERENCE)}; "
             "its headings or flag cells changed shape, so reference_terms() in this script "
             "must follow before the check means anything")
        return 0
    have = glossary_terms()
    seen: set[str] = set()
    for term, where in terms:
        if term in have or term in seen:
            continue
        seen.add(term)
        find(f"glossary: `{term}` (from `{where}`) has no row in {rel(GLOSSARY)}; add the row "
             "(.claude/rules/docs.md, Glossary), or, for a switch or a mode, list the flag in "
             "scripts/lint-docs.py NOT_TERMS with its reason (docs/how-to/implement-a-feature.md, "
             "When it does not work)")
    return len({term for term, _ in terms})


# ---- change -----------------------------------------------------------------------

def pick_range() -> tuple[str | None, str | None]:
    """Return (base, head); head None means the working tree."""
    env = os.environ.get("LINT_DOCS_RANGE")
    if env:
        return split_range(env)
    event = os.environ.get("GITHUB_EVENT_NAME")
    if event == "pull_request" or event == "pull_request_target":
        return f"origin/{os.environ['GITHUB_BASE_REF']}", "HEAD"
    if event == "push":
        before = None
        path = os.environ.get("GITHUB_EVENT_PATH")
        if path and Path(path).exists():
            before = json.loads(Path(path).read_text()).get("before")
        if not before or set(before) == {"0"}:
            return parent_of_head(), "HEAD"
        return before, "HEAD"
    if event:
        return parent_of_head(), "HEAD"
    dirty = git("status", "--porcelain", "--untracked-files=all").strip()
    if dirty:
        return "HEAD", None
    return parent_of_head(), "HEAD"


def parent_of_head() -> str:
    """HEAD~1, or the empty tree when HEAD is a root commit, so that the repository's
    first commit is checked whole instead of failing for want of a parent (CI run
    41559093461 on muster's first push)."""
    if git("rev-parse", "--verify", "--quiet", "HEAD~1", check=False).strip():
        return "HEAD~1"
    return git("hash-object", "-t", "tree", "/dev/null").strip()


def split_range(spec: str) -> tuple[str, str]:
    if "..." in spec:
        a, b = spec.split("...", 1)
        return git("merge-base", a, b or "HEAD").strip(), b or "HEAD"
    if ".." in spec:
        a, b = spec.split("..", 1)
        return a, b or "HEAD"
    return spec, "HEAD"


def changed_files(base: str, head: str | None) -> list[str]:
    if head is None:
        files = set(git("diff", "--name-only", base).split())
        files |= set(git("ls-files", "--others", "--exclude-standard").split())
        return sorted(files)
    return sorted(set(git("diff", "--name-only", base, head).split()))


def file_diff(base: str, head: str | None, path: str) -> str:
    if head is None:
        if git("ls-files", "--error-unmatch", path, check=False) == "" and not (ROOT / path).exists():
            return ""
        tracked = subprocess.run(["git", "ls-files", "--error-unmatch", path], cwd=ROOT,
                                 capture_output=True).returncode == 0
        if not tracked:
            p = ROOT / path
            return "".join("+" + line for line in p.read_text(encoding="utf-8", errors="replace").splitlines(True))
        return git("diff", base, "--", path)
    return git("diff", base, head, "--", path)


def content_at(rev: str | None, path: str) -> str | None:
    if rev is None:
        p = ROOT / path
        return p.read_text(encoding="utf-8") if p.exists() else None
    p = subprocess.run(["git", "show", f"{rev}:{path}"], cwd=ROOT, capture_output=True, text=True)
    return p.stdout if p.returncode == 0 else None


def is_command_tree(path: str) -> bool:
    return path in COMMAND_TREE_FILES or any(path.startswith(d) for d in COMMAND_TREE_DIRS)


def check_change() -> tuple[str, int]:
    base, head = pick_range()
    label = f"{base}..{head or 'working tree'}"
    if git("rev-parse", "--verify", "--quiet", base, check=False).strip() == "":
        find(f"change: base {base} is not a commit; set LINT_DOCS_RANGE")
        return label, 0
    files = changed_files(base, head)
    code_triggers: list[str] = []
    for f in files:
        if is_command_tree(f):
            code_triggers.append(f"{f} (command tree)")
        elif f.endswith(".rs") and f.startswith("crates/") and "/src/" in f:
            if any(PUB_LINE.match(line) for line in file_diff(base, head, f).splitlines()):
                code_triggers.append(f"{f} (public item)")
    docs_changed = [f for f in files if f.startswith("docs/") or Path(f).name == "README.md"]
    if code_triggers and not docs_changed:
        find("change: " + ", ".join(code_triggers) + " changed with no change under docs/ or "
             "to a README (CLAUDE.md, invariant 6: docs ship in the same commit)")
    for f in files:
        if not (f.startswith("docs/decisions/") and ADR_NAME.match(Path(f).name)):
            continue
        before = content_at(base, f)
        after = content_at(head, f)
        if before is None or after is None or adr_status(before) != "Accepted":
            continue
        problem = adr_change_problem(before, after)
        if problem:
            find(f"change: {f} was Accepted at {base} and changed beyond its Status line, "
                 f"but not as a correction of fact: {problem}. A correction appends a dated "
                 "Corrected entry to the header and changes no title, header field or "
                 "Decision section; a change of mind is a new ADR (ADR 0018, "
                 ".claude/rules/docs.md)")
    return label, len(files)


# ---- main -------------------------------------------------------------------------

def main(argv: list[str]) -> int:
    mode = argv[1] if len(argv) > 1 else "all"
    if mode not in ("all", "links"):
        sys.exit("usage: lint-docs.py [all|links]")
    pages = check_pages()
    links = check_links()
    adrs = check_adrs()
    terms = check_glossary()
    summary = f"{pages} pages, {links} links, {adrs} ADRs, {terms} terms"
    if mode == "all":
        label, n = check_change()
        summary += f", {n} files changed in {label}"
    if findings:
        for f in findings:
            print(f"lint-docs: {f}")
        print(f"lint-docs: {summary}, {len(findings)} finding(s)")
        return 1
    print(f"lint-docs: {summary}, ok")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
