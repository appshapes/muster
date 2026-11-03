# Implement a feature, from issue to commit

A how-to page: one task, a feature from a Linear issue to its commits on `master`, for a
person with write access or for the agent. The rules are in `CLAUDE.md` and the gates
enforce them; this page is the order of work. Why the first features are the ones they
are is [ADR 0007](../decisions/0007-build-the-trello-command-set-first.md), and the
next ones [ADR 0013](../decisions/0013-build-the-second-slice-sub-issues-attachments-and-projects.md).

## Prerequisites

- A Linear issue on the team, one per feature. `/next-task` says which feature is next
  and whether it has an issue, and `/linear-create` makes one. Its identifier
  (`APP-12`) prefixes every commit of the feature.
- The four gates pass on a clean `master`: `just lint`, `just test`, `just docs` and
  `just lint-docs`. `just docs` needs `mdbook` 0.5.4 (`brew install mdbook`), and the
  scripts need Python 3. The last line of `just lint-docs` ends with `ok`.
- `CLAUDE.md`, read once. Then `crates/muster/README.md`, then the code.

## Steps

1. **Write the issue as the brief.** The title is the commit subject: imperative, under
   72 characters. The body says what changes, what done looks like in a "Done when"
   section, and which invariant it serves, by number. When the agent will do the work,
   the same text is the body of the GitHub issue in
   [Ask the agent to make a change](ask-the-agent.md); the agent reads nothing else.

2. **Decide and specify, before any code.** Only when the feature changes an output
   field, an exit code, a key source, a flag's meaning, a dependency or an environment
   variable. Otherwise go to step 3.
   1. Copy `docs/decisions/0000-template.md` to the next number and fill every section,
      the alternatives that lost included. Set Status to Accepted: an ADR is accepted
      on the evidence that informed it, and what the implementation proves goes in the
      commit message. If the decision still needs a prototype, leave it Proposed and
      stop here.
   2. Add a row to `docs/glossary.md` for every new term and a line under Unreleased in
      `CHANGELOG.md`.
   3. Commit (step 4), with the decision as the subject: `APP-12: Read the key from a
      file, never argv`.

3. **Write the fixture and the fake first, then the code.**
   1. For a read operation, record its shape: `just record-fixtures` with the key runs
      the operation against a real workspace and writes the values-stripped answer
      under `crates/muster/tests/fixtures/`
      ([ADR 0010](../decisions/0010-test-against-a-fake-linear-held-to-recorded-shapes.md)).
      Then write the fake's answer in `crates/muster/tests/support/fake_linear.rs`,
      with the fixture's key tree; the conformance test fails until the two agree.
   2. Write the line of the README's ten-minute path or of the tutorial that shows the
      command, with the JSON line it prints and its exit code. The command is designed
      from this side.
   3. Write the code: the operation string in `crates/muster/src/linear/operations.rs`
      with its name, the types with Linear's own field names, the command in the tree,
      and the output through the output module. No new field that Linear did not
      return; no prompt; every failure an `Error` with a code and a hint.
   4. Tests, through the binary, with stdin closed and stdout a pipe: the full case
      before the happy path; a test at every bound the feature adds; an exit-code test
      for every failure the feature can meet, the fake made to answer it through its
      control handle.
   5. Docs in the same commit: `just gen-commands`, so that
      `docs/reference/commands.md` carries the command; the README or tutorial line
      from 3.2; glossary rows; a `CHANGELOG.md` line. The reference page is generated,
      never written by hand.
   6. If a gate prints `not built yet` and would check this change, building it is
      part of this commit. The slice's ADR (ADR 0007, then ADR 0013) names which feature
      builds which gate.
   7. One command per pull request where the work goes through the review loop, and
      none over about a thousand changed lines, generated files aside. The reviewer
      reads a pull request whole and the fixer applies every blocker of a review in one
      run, so the blocker count of a first review must fit one run. Split by command,
      then by file within a command, so that each pull request passes the four gates on
      its own; the rest waits on its own branch until the first merges.

4. **Run the gates and commit.**
   - A person: `/commit APP-12` runs the four gates, then `git add`, `git commit`,
     `git pull --no-edit` and `git push` to `master`. The message is the review record.
     Subject: `<identifier>: <Imperative subject>`. Body: what changed, which invariant
     by number, and the evidence: the gate output, the ADR number, the test that fails
     without the change, and the `just smoke` output when the change claims to work
     against Linear. Merges only, never a rebase.
   - The agent: open a GitHub issue with the Linear issue's title and body and label it
     `claude`; [Ask the agent to make a change](ask-the-agent.md) says what happens
     next.
   - A Claude Code session on a branch: `/pr APP-12` runs the four gates one command at
     a time, commits, merges `master` in, pushes and opens the pull request with the
     review record as its body and the `auto-merge` label, then follows the review loop
     from the author's side; `/pr watch <number>` follows one that is open. The pull
     request's title and body become the commit on `master`.
   - When the feature's last commit is on `master`, comment the commit hashes on the
     issue (`muster issue comment APP-12 --body-file <file>`), close it
     (`muster issue close APP-12`), and run `just tidy`: it retires every branch whose
     pull request merged, locally and on origin, with its worktree; `just tidy <branch>`
     retires a working branch that never needed a pull request.

## How to verify it worked

- `git log --oneline --grep '^APP-12:' origin/master` lists the feature's commits,
  each with the prefix.
- `gh run list --commit <full sha>` shows the `ci` run green, with `gates (linux)` and
  `gates (macos)` both passed.
- `just docs`, then open `target/book/index.html`: the new ADR and pages are in the
  book, and `docs/reference/commands.md` shows the command.
- `muster issue get APP-12 | jq '.state.name'` prints the team's completed state, and
  the last comment carries the commit hashes.

## When it does not work

- `just lint-docs` says a public item or a command `changed with no change under docs/
  or to a README`: a public item needs its documentation in the same commit, and a
  command needs `just gen-commands` run (step 3.5).
- `just lint-docs` says an ADR `was Accepted at ... and changed beyond its Status line`:
  accepted ADRs are immutable. Write a new one that supersedes it.
- `just lint-commands` says `docs/reference/commands.md is stale`: run
  `just gen-commands` and commit the result with the code.
- `just docs` fails on the README example: the example is the doctest. Fix the code or
  the example, never mark the block `ignore`.
- `just lint` reports a dependency that is not on the allowlist: the crate's
  dependencies are listed in `crates/muster/Cargo.toml` metadata, and a new one needs
  the entry, a justification in the commit message and an ADR (step 2).
- `just test` fails the conformance test for an operation: the fake's answer and the
  fixture disagree. Re-record the fixture if Linear changed; fix the fake otherwise.
- A gate prints `not built yet`: step 3.6.

## The worked example

Feature 1 of ADR 0013, sub-issues, is the first feature that followed this page step by
step, as the Linear issue APP-8; `git log --oneline --grep '^APP-8:' origin/master`
lists its commit on `master`. In the order of the steps above:

1. **The issue** (step 1). `/linear-create` wrote APP-8 from a brief with "What
   changes", a "Done when" list and the invariants by number, and
   `muster issue update APP-8 --state "In Progress" --assignee me` started it.
2. **No ADR of its own** (step 2). ADR 0013 had already decided the output field
   (`children`), the three `--parent` flags and the bound, so the feature went to step
   3. A feature that decides one of those itself writes the ADR first.
3. **The fixture and the fake first** (step 3.1). The fake's seed gained a sub-issue of
   its first issue and its `full_issue` gained `children`, so that the fixture test
   has a node shape to hold it to; then `just record-fixtures` re-recorded `IssueById`
   on an issue of the workspace that has a sub-issue and a comment. The recorder now
   prefers such an issue and says in a `partial` notice what a recording lacks.
4. **The tutorial's lines** (step 3.2). Step 4 of the tutorial says what `--parent`
   prints and where the sub-issue is created; step 6 says what `children.nodes` holds
   and where its bound is.
5. **The code** (step 3.3). `CHILDREN_PER_ISSUE` in `bounds.rs`; `children` on the
   `IssueById` operation; `--parent` on `issue create`, `issue update` and
   `issue list` in `cli/mod.rs`; the resolution of the parent, the team rule and the
   `truncated` notice in `cli/run.rs`.
6. **The tests** (step 3.4). Five tests through the binary: the children and the
   parent in `issue get`, a sub-issue created by identifier, number and id and found by
   `issue list --parent`, an issue moved under a parent and out again, the team rule
   with two teams and with `MUSTER_TEAM` set, and the bound at 50 and at 51
   sub-issues. An unknown parent is exit 4 before any mutation.
7. **The docs** (step 3.5). `just gen-commands`, the glossary's `sub-issue` row, the
   changelog lines, the smoke script's sub-issue steps, and this section.
8. **The gates and the landing** (step 4). `/pr APP-8` ran the four gates one command
   at a time, opened the pull request with the review record as its body and the
   `auto-merge` label, the loop reviewed and squash-merged it, and
   `muster issue close APP-8 --comment-file comment.md` closed the issue with the
   commit's hash in the comment.
