# Muster

Muster is a command-line client for Linear, written in Rust, built for coding agents first and for people second. One static binary, one GraphQL endpoint, and every command prints JSON an agent can parse and exits with a code an agent can branch on. The promise: an agent takes a Linear issue from reading it to closing it with no browser, no SDK and no guessing.

Linear's API (developers.linear.app) is the reference. Where its names are good, use them. Where its surface is hard for an agent (nested GraphQL, a UUID for everything, cursors), that is the product. Invariants come first because they decide conflicts. Everything here is a rule.

## Invariants, in priority order

When two conflict, the higher one wins. Never trade down.

1. **Never lie about Linear.** Exit 0 means the operation happened in Linear and the output is Linear's answer. A field muster prints is a field Linear returned; nothing is cached, reconstructed or guessed. A mutation whose answer never came exits 8, `unknown`, and says the request may have landed. A bug here blocks release; nothing else does automatically.
2. **An agent is at the keyboard.** Every command runs with no terminal: no prompt, no pager, no colour unless stdout is a terminal and `NO_COLOR` is unset, one JSON object per line on stdout, one JSON error object on stderr, and a documented exit code per failure class. A flag or a field never changes meaning once released. A person gets `--human`.
3. **Bounded everything.** Every list has a page size and a cap, every request a timeout, every retry a count of zero or a documented bound, every body a size limit. Linear's rate limit is reported with its reset time, never absorbed by sleeping.
4. **Explainable from the output.** Every error carries muster's code, Linear's own message, Linear's request id and what to do next. `--trace` prints every request and response to stderr with the key redacted. No telemetry, no log file.
5. **Correct by default.** `muster` with a key and nothing else works against any workspace: the team is inferred when the workspace has one, the done state is the team's completed state, and the key comes from a file or the environment, never from argv. A new flag or environment variable needs a written reason why the default cannot be right, and `muster whoami` reports it when it is set wrong.
6. **Understandable by a stranger.** A competent engineer who has never used Linear, in another time zone, with nobody to ask, can run muster in ten minutes, find why any behaviour happens, and find why any decision was made, from the repository alone. Documentation never overrides the invariants above, but code without its documentation does not merge.

Convenience yields to boundedness. Boundedness yields to correctness. Documentation wins no conflicts and blocks every merge.

## Gates: how each invariant is enforced

This file is context, not enforcement. The gates below are the enforcement. If a gate named here does not exist yet, building it is part of the first feature that needs it. Do not ship around a missing gate.

| Invariant | Gate |
| --- | --- |
| 1 Truth | `just test` runs every command through the binary against the fake Linear in `crates/muster/tests/support/`, whose answers carry the field sets of real responses recorded under `crates/muster/tests/fixtures/` (ADR 0010). A test per mutation asserts exit 8 on a connection dropped after the request was sent. `just smoke` runs the ten-minute path against a real workspace, by hand, before a release. |
| 2 Agent | `just test` runs every command with stdin closed and stdout a pipe, and asserts valid JSON on every stdout line and the documented exit code. `clippy.toml` disallows `std::io::stdin`. `just lint-commands` fails when `docs/reference/commands.md` differs from `muster schema --format markdown`. |
| 3 Bounded | Every bound is a named constant in `crates/muster/src/bounds.rs`, listed by `muster schema`, with a test at the bound. |
| 4 Explainable | Every error is an `ErrorCode` with a hint; a test enumerates the codes and asserts each has its row in `docs/reference/commands.md`. |
| 5 Defaults | `just quickstart` runs the README's ten-minute path end to end against the fake Linear, in CI on Linux and macOS. Every environment variable muster reads is listed by `muster schema` with its `why_not_default`. |
| 6 Understandable | `#![deny(missing_docs, rustdoc::broken_intra_doc_links)]` on the crate. `just docs` builds rustdoc and the mdBook with warnings denied, runs every code block and checks links. Reference pages are generated from `muster schema`, never hand-written. `just lint-docs` fails a commit that changes a public item or a command without touching `docs/` or a README. A change to the output shape, an exit code or a key source needs an accepted ADR in `docs/decisions/`. |

## Commands

The `justfile` is the single entry point. `cargo` works, but `just` sets the flags.

```
just build          # release build of the binary
just test           # unit and integration tests, against the fake Linear
just smoke          # the ten-minute path against a real workspace; needs a key; not in CI
just quickstart     # the README path against the fake Linear, timed; what CI runs
just lint           # clippy with workspace lints, fmt, the workspace lint, lint-commands
just docs           # rustdoc + mdBook, warnings denied, code blocks run, links checked
just lint-docs      # fail if code changed without its docs (what CI runs)
just gen-commands   # regenerate docs/reference/commands.md from `muster schema`
just record-fixtures  # re-record the real response shapes the fake is held to; needs a key
just tidy           # retire branches whose work is on master; `just tidy <branch>` for a named one
```

## Repository map

```
docs/                 mdBook: tutorials/, how-to/, reference/ (generated), explanation/, decisions/ (ADRs), glossary.md
crates/
  muster/             the one crate, published as `muster-linear` (ADR 0011, ADR 0012): the `muster` binary and the `muster::Linear` client behind it
    src/main.rs       the command tree (clap), the output and the exit codes
    src/linear/       the GraphQL client: transport, operations, types
    src/bounds.rs     every limit, named
    tests/            integration tests through the binary; support/ is the fake Linear; fixtures/ the recorded shapes
scripts/              the Python gates: lint-docs, lint-workspace, tidy, release notes
.claude/              agents, rules and skills for Claude Code sessions and the agent workflows
.github/workflows/    ci, the agent loop (claude, dispatch-review, review-pull-request, auto-merge), send-release-notes
```

## Rules

### Output

- stdout carries the answer and nothing else: one JSON object per line, lists as one object per line. stderr carries the error object, the notice object, and `--trace`.
- Field names are Linear's (`identifier`, `createdAt`, `state.name`). Do not rename, flatten or invent a field; add one only when it comes from Linear.
- `--human` renders the same data as text. Nothing is in one format and not the other.
- Exit codes: 0 ok; 1 muster's own failure or an answer it does not understand; 2 usage; 3 authentication; 4 not found; 5 rate limited; 6 refused, by Linear's validation or by a muster guard such as a duplicate title; 7 network, before the request was sent; 8 unknown, a mutation sent with no answer. `docs/reference/commands.md` is the table.

### The key

- Read from `LINEAR_API_KEY`, else from the file `MUSTER_KEY_FILE` names, else from `~/.config/muster/key`. Never a flag, never printed, never in `--trace`, never in a test fixture.
- An error that names the key says where to put it, with the docs anchor.

### Linear's names

- Use Linear's vocabulary: issue, identifier (`APP-12`), team, workflow state (`state`), label, project, cycle, comment, attachment, assignee, priority. Do not invent synonyms. `docs/glossary.md` maps each to Trello's word, since the first users come from Trello.
- One GraphQL operation per command where Linear allows it. An operation string lives in `src/linear/operations.rs` with its name, and the fake answers by that name.

### Configuration and defaults

- Zero configuration is the primary supported configuration. `MUSTER_TEAM` exists for a workspace with several teams; nothing else is configured.
- Every environment variable has a `why_not_default`, a `whoami` check, and a row in the generated reference, or it does not merge. Never add a flag to make a test pass.

### Bounds

- Page size, page cap, request timeout, body size and retry count are constants in `bounds.rs`. A command that hits a cap exits 0 with what it has and writes a `truncated` notice object to stderr, never silently truncates.

### Documentation

- Docs ship in the same commit as the code they describe. A commit whose message says "docs to follow" gets reverted.
- `docs/` has four kinds of page and they do not mix: tutorial, how-to, reference (generated), explanation. Detail in `.claude/rules/docs.md`.
- Write for the reader in invariant 6. Define a term on first use or link it to `docs/glossary.md`.
- The crate README answers four questions: what this does, what it must never do, a twenty-line example that is also a doctest, and what to read next.
- Decisions live in `docs/decisions/` as numbered ADRs, immutable once accepted and superseded by a new one. A decision made in chat, a call or a commit message does not exist until it is an ADR.
- Every error message names the docs anchor that explains it.

## Working rules for agents

- Read `CLAUDE.md`, then the crate's `README.md`, then `docs/decisions/0007-build-the-trello-command-set-first.md`, before the code.
- Trunk-based by default: developers commit directly to `master`. Bots and agent workflows use branches and pull requests. `master` is green after every commit, so run `just lint`, `just test`, `just docs` and `just lint-docs` before each one; `/commit` does. The commit message is the review record: what changed, which invariant, and the evidence.
- A Claude Code session working a Linear issue uses `/linear-start`; the issue's identifier prefixes its commits (`APP-12: Add the comment command`). After a pull request merges, run `just tidy`.
- Concurrent branches collide in the files everyone appends to: `CHANGELOG.md`, `docs/SUMMARY.md`, `docs/glossary.md`, `src/linear/operations.rs` and `tests/support/fake_linear.rs`. Append at the end of the section so both sides can be kept, and resolve a conflict by keeping both sides with `master`'s lines first, in the branch that merges second.
- Before claiming a command works, run `just test`; before claiming it works against Linear, run `just smoke` and put its output in the commit message.
- Do not add a dependency without a justification in the commit message and an entry on the allowlist in `crates/muster/Cargo.toml` metadata; `just lint` checks it. The tree is small on purpose (ADR 0002).
- Decide, do, and show the evidence; no change waits on a person's approval. A decision that cannot be undone (an output field, an exit code, a key source, a flag's meaning) is an ADR before it is acted on.
- A new command starts with its line in the first-slice ADR or a new ADR, its README or tutorial line, its fixture and its fake answer, in that order, before the code.
- When the docs failed to tell you something you needed, fix the docs in the same session.
- Keep this file under 200 lines. Instructions that matter only for part of the tree go in `.claude/rules/` with a `paths:` scope.

## When in doubt

- Copy Linear's name and shape.
- Make it bounded first, then convenient. Exit non-zero rather than guess.
- An exit code beats a message; a message with a hint beats a bare one.
- If a decision cannot be undone (an output field, an exit code, a key source), write it up as an ADR, then act on it.

# Overrides
- @./.brigade/CLAUDE.brigade.md
