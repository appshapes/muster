# Changelog

All notable changes to Muster are recorded here, in the reader's words, one line per
user-visible change, added in the same commit as the change. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Unreleased

Nothing yet.

## 0.1.0 - 2026-11-03

The first release: the five Trello operations as `muster` commands, for coding agents
first. Install with `cargo install muster-cli`; the binary is `muster`.

### Added

- Released as the crate `muster-cli` 0.1.0 on crates.io, because `muster` belongs to
  another project, with the binary and the library both named `muster`; a GitHub
  Release carries `muster-0.1.0-x86_64-unknown-linux-gnu.tar.gz` and
  `muster-0.1.0-aarch64-apple-darwin.tar.gz` with their SHA-256 sums, built by
  `.github/workflows/release.yml` from the tag `v0.1.0`
  ([ADR 0011](docs/decisions/0011-release-as-muster-cli.md),
  [how to cut a release](docs/how-to/cut-a-release.md)).

- Repository scaffold ported from Cavalry: `CLAUDE.md` invariants and gates, the
  documentation rules, the ADR template, the glossary and the book.
- ADR 0001: decisions are recorded as ADRs. ADR 0002: muster is Rust on a blocking
  HTTP client, and why not Go or the TypeScript SDK. ADR 0003: one crate, one version,
  no publishing yet, the lints and the dependency allowlist.
- ADR 0007: the first slice is the five Trello operations, as five features. ADR 0008:
  the agent contract, JSON Lines, the exit codes, no prompt, the key from a file.
  ADR 0010: the tests run against a fake Linear held to recorded shapes.
- Cargo workspace with the `muster` crate, `just lint` and `just test` passing for
  real: `unsafe` forbidden, `unwrap`, `expect` and `panic` denied, printing confined to
  the output module, `stdin`, `exit` and `sleep` disallowed, and
  `scripts/lint-workspace.py` checking the dependency allowlist.
- The agent loop ported from Cavalry (ADR 0004, ADR 0005): `ci.yml` runs the four
  gates and `just quickstart` on Linux and macOS Blacksmith runners; `claude.yml`,
  `dispatch-review.yml`, `review-pull-request.yml` and `auto-merge.yml` run the writer,
  the reviewer and the squash merge; Dependabot moves the pinned actions and the crates
  weekly. The briefs are `.claude/agents/developer.md` and `reviewer.md`;
  `docs/how-to/ask-the-agent.md` is the page for a person.
- The documentation gate (ADR 0006): `just docs` builds rustdoc and the book with
  warnings denied and checks every link; `just lint-docs` fails a public item or a
  command changed without its docs and an edit to an accepted ADR.
- The weekly release-notes email (ADR 0009): `send-release-notes.yml`, the scripts
  under `scripts/release-notes*`, the writer and reviewer brief, and
  `docs/how-to/send-the-release-notes.md`.
- `muster whoami`, `team list`, `state list`, `label list`, `project list` and
  `user list`; the key from `LINEAR_API_KEY`, `MUSTER_KEY_FILE` or
  `~/.config/muster/key`; `MUSTER_TEAM` and `MUSTER_ENDPOINT`; `muster schema` as JSON
  or as the generated `docs/reference/commands.md`, which `just lint-commands` holds to
  the binary (feature 1 of ADR 0007).
- `muster issue get` by identifier, number or id, with comments and attachments, and
  `muster issue list` with `--state`, `--all`, `--label`, `--assignee`, `--search` and
  `--limit`; every bound a constant in `bounds.rs`, a cut list a `truncated` notice
  (feature 2 of ADR 0007).
- `muster issue create` with labels, state, priority, project, assignee and a
  description from a file or stdin, refusing a duplicate open title unless
  `--allow-duplicate`; `muster issue comment` from a flag, a file or stdin
  (feature 3 of ADR 0007).
- `muster issue close` to the team's completed state, `--cancel` or `--state`, after an
  optional comment; `muster issue update` for title, state, labels, priority, assignee,
  project and description (feature 4 of ADR 0007).
- The fake Linear under `crates/muster/tests/support/`, held to the shapes recorded under
  `crates/muster/tests/fixtures/` by `just record-fixtures` (ADR 0010); 25 tests through
  the binary with stdin closed; `just quickstart` against the fake and `just smoke`
  against a real workspace.
- Claude Code skills under `.claude/skills/`: `/commit`, `/pr` and `/next-task` ported
  from Cavalry, and `/linear-read`, `/linear-create`, `/linear-done` and `/linear-start`
  calling `muster` in place of the Trello skills they replace (feature 5 of ADR 0007).
- The tutorial `docs/tutorials/work-an-issue-in-ten-minutes.md`, the explanation
  `docs/explanation/why-muster.md`, and `docs/how-to/implement-a-feature.md`.
