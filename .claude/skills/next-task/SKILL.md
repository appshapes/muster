---
name: next-task
description: Say what the next task is, read from the repository and its process alone (master's CI, open pull requests and issues, the team's Linear issues, the slice ADR's feature order, the gates that print not built yet, Proposed ADRs, fired Revisit clauses, active sessions); an optional count lists that many, in order
argument-hint: "[count]"
---

Answer "What is the next task?" from the repository and its process, not from memory or
preference. Print the top task, or the top N in order, each as a title and a short
description. Every task names the file, issue, run or command it was read from, so a
stranger can check it (CLAUDE.md, invariant 6). A task with no source is not listed.

The count is: $ARGUMENTS. Empty means 1. Not a positive integer means 1, and say so.
More than 10 means 10, and say so.

Read everything yourself, with no subagent sweeps: the session that cites a line is the
session that read it. Create nothing, change nothing, start nothing.

## Why this needs no backlog

The process fixes the order of work, so the next task is a reading, not a choice.
`master` is green after every commit (CLAUDE.md, Working rules for agents). A feature is
a Linear issue and follows `docs/how-to/implement-a-feature.md`. The slice ADR fixes the
order of the slice's features and which gates each one builds: ADR 0007 for the first
slice, then the ADR its Revisit clause leads to, once that one is Accepted. A gate that
prints `not built yet` is built by the first feature that needs it (CLAUDE.md, Gates). A
Proposed ADR is accepted when the evidence it names exists. An accepted ADR's Revisit
clause says when a new ADR is due.

## 1. Read the state

Run these in parallel. Read `origin/master`, not the local branch: other sessions commit
to `master` while this one runs.

- `git fetch --quiet origin`, then `git log --oneline -30 origin/master`.
- `gh run list --branch master --workflow ci.yml --limit 1`: CI at `master`'s head.
- `gh pr list --state open` and `gh issue list --state open`: work in flight through
  `docs/how-to/ask-the-agent.md`.
- The team's open issues in Linear, one command per state:
  `muster issue list --state Backlog`, `muster issue list --state Todo` and
  `muster issue list --state "In Progress"`. Each prints one JSON object per line with
  `identifier`, `title`, `state.name`, `url` and `description`; `description` is the
  brief, with its "Done when" section. On exit 3, say the key is missing (CLAUDE.md,
  "The key") and continue; on exit 7, say Linear was not reached and continue: the
  slice ADR still gives the order.
- `brigade sessions`, when `brigade` is installed: the line under an active session is
  what it says it is doing, in its owner's words, unverified and possibly stale.
- `grep -n 'not built yet' justfile`: the gates that do not exist.
- `grep -H -m1 'Status' docs/decisions/*.md`: the Proposed ADRs.
- `grep -n -A4 '^Revisit' docs/decisions/*.md`: the Revisit clauses.
- The Decision section of the slice ADR: ADR 0014,
  `docs/decisions/0014-build-the-third-slice-administrative-commands.md`,
  twenty groups in three blocks, in order, since the three features of ADR 0013 are
  done (its Revisit clause fired on 2026-11-03 with release 0.2.0; ADR 0007's fired
  the same day); the issue identifier where it names one, and the gates each group
  builds.
  When its Revisit clause has fired (the slice is done by its own "done when"), the
  slice is done, and until the next slice's ADR is Accepted, writing it is the task.
- For every feature of the slice and every open issue:
  `git log --oneline origin/master --grep '^APP-<n>:'`, with the issue's team key and
  number.

## 2. Decide what is done, in flight or free

- A feature of the slice is **done** when its issue's "Done when" holds or, with no
  issue, when the gates the slice ADR says it builds no longer print `not built yet`
  and its commands are in `docs/reference/commands.md`. `master` is the only evidence:
  a branch, a pull request or a session's line is not done.
- A task is **in flight** when it has an open pull request or GitHub issue, has commits
  on `origin/master` with its identifier prefix but is not done, is in the `In Progress`
  state, or an active session's line names it.
- Everything else is **free**.

## 3. Order the candidates

Highest first. Within one rule, the slice ADR's order, then the issue's order in the
list.

1. **A red `master`.** CI failed at `origin/master`'s head: the task is to make it
   green, titled with the failing run's subject. A run that is queued or in progress is
   not red; name it in the footer and go on.
2. **Work stuck on a person.** A pull request that is approved without `auto-merge`,
   conflicting, or at the fix cap; a GitHub issue labelled `claude` whose run failed.
   The "When it does not work" section of `docs/how-to/ask-the-agent.md` says what each
   needs.
3. **The first feature of the slice that is not done.** With an issue: the issue,
   titled `<identifier>: <issue title>`, and when commits with its prefix exist, the
   description says which of the issue's commits is next. Without an issue: the
   feature, titled in the imperative from the slice ADR's words, and the description
   says that step 1 of `docs/how-to/implement-a-feature.md`, writing the issue, is the
   first move. The features after it follow in the slice's order, each waiting on the
   one before.
4. **A Proposed ADR whose evidence now exists.** Its Evidence or Revisit section says
   what acceptance waits on; when that is on `master`, accepting it is a task. When it
   is not, the ADR is not a task.
5. **A fired Revisit clause.** The trigger has happened on `master`, the action it names
   has not, and no open issue lists it. Cite the trigger and the evidence that it fired.
6. **Every other open issue**, Todo before Backlog, then by position in the list.

A task in flight in another session is reported in one line as taken, with the
session's name, and does not count toward N. A task in flight on a pull request or a
GitHub issue counts: finishing it is the task.

## 4. Report

For each task, in order:

```
1. <Title>
   <Description>
```

- **Title**: imperative, capitalized, under 72 characters, usable verbatim as the issue
  title and the commit subject. An issue's own title when one exists, prefixed
  `<identifier>: `.
- **Description**: two to four sentences, under 80 words. What it is. Why it is next,
  with the source: a file and section, an issue identifier, a run id or a command's
  output. Which invariant, by number. What the first move is. What it waits on, when
  N > 1. No code block, and at most one path per sentence.

After the list, one footer line beginning `Read:` with what was consulted: `origin/master`'s
head and its CI state, the counts of open pull requests, GitHub issues and Linear
issues, the slice's done count out of its total, the Proposed ADRs, and the active
sessions. A source that could not be read is named here.

End with the one command that starts the top task: `/linear-read <identifier>` when it
has an issue, `/linear-create "<title>"` when it does not, or `gh pr view <n>` for a
pull request. Do not run it.
