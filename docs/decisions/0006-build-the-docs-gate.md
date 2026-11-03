# 0006. Build the documentation gate from mdBook, rustdoc and a link script

- **Status:** Accepted
- **Date:** 2026-11-02
- **Invariants affected:** 6
- **Supersedes:** none

## Context

`CLAUDE.md` names the gate for invariant 6: `just docs` builds rustdoc and the mdBook with
warnings denied, runs every code block and checks links; `just lint-docs` fails a commit
that changes a public item or a command without touching `docs/` or a README.
`.claude/rules/docs.md` requires both to pass before a commit. Cavalry built this gate on
2026-10-11 (Cavalry ADR 0006) and has run it on every commit since, and muster takes
Cavalry's practices by the owner's brief of 2026-11-02; the repository starts with nine
ADRs, a glossary, a changelog and a book before it has a command, so the gate is needed in
the first commit, not after it.

Three facts set the shape, as they did for Cavalry. First, the gate must run in seconds on
a laptop and on both CI runners, and must not go red for a reason outside the repository: a
link checker that fetches the web turns a site's outage into a failed commit on `master`.
Second, `mdbook` ships prebuilt binaries for both runner platforms and the
`taiki-e/install-action` catalogue verifies them against its manifest, while `lychee`, the
Rust link checker, is not in that catalogue and compiles for minutes on each runner. Third,
muster has no `spec/`, no counters file and no configuration registry, which Cavalry's
script checks; what muster has instead is a command tree, whose change the generated
reference and the tutorial must follow.

## Decision

`just docs` runs rustdoc with `RUSTDOCFLAGS="-D warnings"`, builds the book from
`book.toml` at the repository root with `src = "docs"`, output under `target/book` and
`create-missing = false`, fails on any `[WARN]` or `[ERROR]` line mdBook prints, runs
`mdbook test` for the book's Rust code blocks, and runs `scripts/lint-docs.py links`,
which checks that every page under `docs/` is listed in `docs/SUMMARY.md` and exists,
that every relative link in `docs/`, `README.md` and the crate READMEs resolves and every
anchor names a heading in its target, and that every ADR is named `NNNN-title.md` with a
matching title and a valid Status. `just lint-docs` runs the same script in full, which
adds the change check over a diff range: in CI the pull request's merge base or the push's
range, on a laptop the working tree against `HEAD` when anything is modified, else the
last commit. The change check fails a change to a public item (a line with `pub ` under a
crate's `src/`), or any change to the command tree (`crates/muster/src/main.rs` or
`crates/muster/src/cli/`), with no change under `docs/` or to a README; and an edit to an
ADR that was already Accepted beyond its Status line. CI runs both after `just lint` and
`just test`, on both runners, with `mdbook` 0.5.4 installed by `taiki-e/install-action`
pinned by SHA and the checkout at full depth. External links are `just docs-links`, which
runs `lychee` online and is not a gate. The commit skill and the writer's brief run all
four gates before a commit or a push. `docs/SUMMARY.md` is grouped by the four kinds of
page; the root `README.md` stays GitHub's front page and is not in the book. The script
is Cavalry's with its counters, configuration and `spec/` checks removed and the
command-tree trigger added; the generated reference, `docs/reference/commands.md`, is
held equal to the binary by `just lint-commands` (ADR 0008), not by this script.

## Alternatives considered

- **`mdbook-linkcheck` as a backend.** Lost: a third-party backend that fetches the web
  by default, with its own binary to install, for a check the script does offline.
- **`lychee` as the gate.** Lost: online by nature, so a site going down fails a commit
  on `master`; not in the install action's catalogue, so each runner would compile it;
  and the only offline check it adds over the script is one the script has. It stays as
  `just docs-links` for a release.
- **`cargo install mdbook` on the runners.** Lost: minutes per runner on a cache miss. A
  prebuilt binary verified against a pinned manifest installs in seconds.
- **Allow mdBook warnings.** Lost: a warning today is a broken page tomorrow, and the
  rule for rustdoc is the same.
- **Public items by rustdoc's JSON output or a Rust parser.** Lost for now: the JSON is
  nightly-only, and a parser means an `xtask` crate ADR 0003 defers. A `pub ` line in a
  diff over-approximates in a safe direction.
- **Trigger the change check on every `.rs` file.** Lost: a private refactor changes no
  behaviour a page describes, and a gate that asks for a doc line on every commit teaches
  people to write empty ones. The command tree is the surface, so the command tree is the
  trigger, with `pub ` lines for the library.
- **Check the glossary from the command tree, as Cavalry checks it from its counters.**
  Lost for now: the terms a command, a flag or a field introduces are not in one file the
  script can read until `muster schema` exists; revisited then.
- **The root `README.md` as the book's first page.** Lost: its links are repository
  paths, which do not resolve from inside the book.

## Evidence

- Cavalry: `just docs` and `just lint-docs` have run on both Blacksmith runners on every
  commit since 2026-10-11 (Cavalry ADR 0006, CI run 38117225595, 17 s and 22 s).
- This laptop, 2026-11-02, Python 3.14, on a scratch clone of this tree with one commit:
  - four violations planted together gave three findings in one run: a page not listed in
    `docs/SUMMARY.md` (`pages: docs/how-to/orphan.md is not listed`), a link to a heading
    the glossary does not have (`links: … no heading matches #no-such-heading`), and a
    paragraph added to accepted ADR 0001 (`change: … was Accepted at HEAD and changed
    beyond its Status line`); the fourth, a `pub fn` added to `crates/muster/src/lib.rs`,
    gave none in that run because the other three plants were under `docs/`, which is the
    check working as written;
  - the `pub fn` alone gave `change: crates/muster/src/lib.rs (public item) changed with
    no change under docs/ or to a README`;
  - a comment appended to `crates/muster/src/main.rs` alone gave `change:
    crates/muster/src/main.rs (command tree) changed with no change under docs/ or to a
    README`, and the same change with a line added to the crate README gave none;
  - every plant reverted, the clone was clean.
- The gate on the real tree, the same day: `scripts/lint-docs.py links` reports 15 pages,
  49 links and 9 ADRs, and its remaining findings name the pages `docs/SUMMARY.md` lists
  that the slice's other commits write (the tutorial, the explanation, two how-tos, the
  generated reference, ADRs 0004 and 0005). The first CI run on both runners is cited in
  the commit that lands `.github/workflows/ci.yml`.

## Consequences

Easier: a broken link, a page left out of the book, a malformed ADR, an edit to an
accepted ADR, a public item or a command that ships without a doc change is caught on the
laptop before the commit and on both runners after it; the book is one `just docs` away;
the writer and the reviewer have a gate to cite instead of a rule to argue.

Harder: a laptop needs `mdbook` installed, and the recipe says how; the command-tree
trigger asks for a doc change on some edits that change no behaviour, which a README line
answers; `mdbook` is one more pinned version to move, in two places, and Dependabot moves
only the action. On a laptop whose checkout has one commit, the clean-tree range
`HEAD~1..HEAD` has no base, and the script says so; `LINT_DOCS_RANGE` names a range until
the second commit.

Revisit: when `muster schema` exists, the glossary check joins the script (every term of a
command, flag or field has a row); when the `xtask` crate exists, the public-item check
moves to a parser; when a second crate exists, the command-tree paths become a list the
script reads from the crates' manifests.

## References

- `CLAUDE.md`, "Gates" and "Documentation"; `.claude/rules/docs.md`
- `book.toml`, `docs/SUMMARY.md`, `scripts/lint-docs.py`, `justfile`,
  `.github/workflows/ci.yml`, `.claude/skills/commit/SKILL.md`,
  `.claude/agents/developer.md`
- ADR 0003 (the Python lint precedent and the `xtask` item); ADR 0008 (the generated
  reference and `just lint-commands`)
- Cavalry ADR 0006, `docs/decisions/0006-build-the-docs-gate.md` in
  <https://github.com/appshapes/cavalry>
- mdBook 0.5.4: <https://github.com/rust-lang/mdBook>; `taiki-e/install-action`:
  <https://github.com/taiki-e/install-action>; lychee: <https://github.com/lycheeverse/lychee>
