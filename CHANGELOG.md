# Changelog

All notable changes to Muster are recorded here, in the reader's words, one line per
user-visible change, added in the same commit as the change. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Unreleased

### Added

- ADR 0013 decides the second slice: sub-issues, attachments and projects, in three
  features, in that order; cycles wait until a team of the workspace enables them
  ([ADR 0013](docs/decisions/0013-build-the-second-slice-sub-issues-attachments-and-projects.md)).
- `issue get` prints `children`, the issue's sub-issues newest first as Linear orders
  them, bounded by `CHILDREN_PER_ISSUE` with a `truncated` notice past it; `issue create --parent`,
  `issue update --parent` (or `--parent none`) and `issue list --parent` make, move and
  list sub-issues, a sub-issue goes to its parent's team unless `--team` or
  `MUSTER_TEAM` names one, and an issue named as its own parent is a usage error,
  exit 2, before any request (ADR 0013, feature 1;
  [the tutorial](docs/tutorials/work-an-issue-in-ten-minutes.md)).
- `issue attach <ISSUE> --url <URL> --title <TEXT> [--subtitle <TEXT>]` adds a link to an
  issue through Linear's `attachmentCreate` and prints the attachment as Linear returned
  it, with its `issue.identifier`; `issue get` lists it under `attachments.nodes`, newest
  first. A URL the issue already has is updated in place, same id, and a subtitle left
  out stays, as Linear does; an empty URL or title is a usage error, exit 2, before any
  request (ADR 0013, feature 2; [the tutorial](docs/tutorials/work-an-issue-in-ten-minutes.md)).
- `BODY_MAX_BYTES` now bounds a title, a link and a subtitle as it bounds a description
  and a comment: a larger one is refused, exit 6, before any request.
- `project create --name <TEXT> [--description <TEXT>]` makes a project for the team
  through Linear's `projectCreate` and prints it as `project list` prints one; a name an
  existing project already has is refused, exit 6, unless `--allow-duplicate`, so that
  `--project <NAME>` keeps resolving to one project. `issue list --project <NAME>` lists
  the team's issues in a project and `--project none` those in no project (ADR 0013,
  feature 3; [the tutorial](docs/tutorials/work-an-issue-in-ten-minutes.md)).
- `muster fixtures` records `IssueById` on an issue that has comments, attachments and
  sub-issues when the workspace has one, and writes a `partial` notice saying what the
  recording lacks; `skipped` and `partial` join the notices table of the reference.

### Fixed

- The reference, the tutorial and the `linear-read` skill said `issue get` prints
  comments oldest first. Linear orders a connection newest first, `orderBy: createdAt`
  included, and muster prints what Linear returns; the pages now say so, and a cut thread
  is the newest `COMMENTS_PER_ISSUE` comments with the oldest left out. The output did
  not change.

## 0.1.1 - 2026-11-03

The first release on crates.io, as the crate `muster-linear`: `cargo install
muster-linear` installs the `muster` binary. The same five commands as 0.1.0.

### Changed

- The crate is `muster-linear`, not `muster-cli`: that name, like `muster`, belongs to
  another project, which the publish of 0.1.0 found out
  ([ADR 0012](docs/decisions/0012-publish-as-muster-linear.md)). The binary and the
  library are `muster` as before. The GitHub Release carries
  `muster-0.1.1-x86_64-unknown-linux-gnu.tar.gz` and
  `muster-0.1.1-aarch64-apple-darwin.tar.gz` with their SHA-256 sums.

## 0.1.0 - 2026-11-03

The first release: the five Trello operations as `muster` commands, for coding agents
first. On GitHub only: its notes said `cargo install muster-cli`, and that crate is
another project's, so the crates.io publish was refused; 0.1.1 is the one to install.

### Added

- A GitHub Release with `muster-0.1.0-x86_64-unknown-linux-gnu.tar.gz` and
  `muster-0.1.0-aarch64-apple-darwin.tar.gz` and their SHA-256 sums, built by
  `.github/workflows/release.yml` from the tag `v0.1.0`; the binary and the library
  are both named `muster`
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
