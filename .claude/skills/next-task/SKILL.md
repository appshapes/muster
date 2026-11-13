---
name: next-task
description: Say what the next task is, read from the repository and its process alone (master's CI, open pull requests and issues, muster's work items on the Brigade Board, the slice ADR's feature order, the gates that print not built yet, Proposed ADRs, fired Revisit clauses, active sessions); an optional count lists that many, in order
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
a work item on the team's Brigade Board, tagged `muster` (ADR 0016), and follows
`docs/how-to/implement-a-feature.md`. The slice ADR fixes the
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
- Muster's work items on the Brigade Board: `brigade work-item list --tag muster`
  prints a table of the open ones by list (Wanting, Doing, Done; the hidden Hibernating
  is parked work and not a candidate), with each one's number, title, assignee and tags.
  Then `brigade work-item show <number>` for each one in Wanting or Doing: its
  description is the brief, with its "Done when" section, and its comments and history
  say who has it. One `brigade` command per Bash call. A work item whose description
  starts with `Linear issue APP-<n>` was carried from Linear on 2026-11-13 (ADR 0016),
  and its earlier commits carry `APP-<n>:`. When `brigade` is not installed or this
  checkout is not joined to the team, say the board was not read and continue: the
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
  the same day); the Linear issue identifier where it names one, which is now the work
  item whose description starts with `Linear issue <identifier>`, and the gates each
  group builds.
  When its Revisit clause has fired (the slice is done by its own "done when"), the
  slice is done, and until the next slice's ADR is Accepted, writing it is the task.
- For every feature of the slice and every open work item:
  `git log --oneline origin/master --grep '^<number>:'`, and for a work item carried
  from Linear also `--grep '^APP-<n>:'`.

## 2. Decide what is done, in flight or free

- A feature of the slice is **done** when its work item's "Done when" holds or, with
  no work item, when the gates the slice ADR says it builds no longer print `not built yet`
  and its commands are in `docs/reference/commands.md`. `master` is the only evidence:
  a branch, a pull request or a session's line is not done.
- A task is **in flight** when it has an open pull request or GitHub issue, has commits
  on `origin/master` with its number's prefix (or `APP-<n>:` for one carried from
  Linear) but is not done, is in the board's Doing list, or an active session's line
  names it.
- Everything else is **free**.

## 3. Order the candidates

Highest first. Within one rule, the slice ADR's order, then the work item's order in
its list.

1. **A red `master`.** CI failed at `origin/master`'s head: the task is to make it
   green, titled with the failing run's subject. A run that is queued or in progress is
   not red; name it in the footer and go on.
2. **Work stuck on a person.** A pull request that is approved without `auto-merge`,
   conflicting, or at the fix cap; a GitHub issue labelled `claude` whose run failed.
   The "When it does not work" section of `docs/how-to/ask-the-agent.md` says what each
   needs.
3. **The first feature of the slice that is not done.** With a work item: the work
   item, titled `<number>: <work item title>`, and when commits with its prefix exist,
   the description says which of the work item's commits is next. Without a work item:
   the feature, titled in the imperative from the slice ADR's words, and the description
   says that step 1 of `docs/how-to/implement-a-feature.md`, writing the work item, is
   the first move. The features after it follow in the slice's order, each waiting on the
   one before.
4. **A Proposed ADR whose evidence now exists.** Its Evidence or Revisit section says
   what acceptance waits on; when that is on `master`, accepting it is a task. When it
   is not, the ADR is not a task.
5. **A fired Revisit clause.** The trigger has happened on `master`, the action it names
   has not, and no open work item lists it. Cite the trigger and the evidence that it fired.
6. **Every other open work item** tagged `muster`, in Wanting, by position in the
   list.

A task in flight in another session is reported in one line as taken, with the
session's name, and does not count toward N. A task in flight on a pull request or a
GitHub issue counts: finishing it is the task.

## 4. Report

For each task, in order:

```
1. <Title>
   <Description>
```

- **Title**: imperative, capitalized, under 72 characters, usable verbatim as the work
  item's title and the commit subject. A work item's own title when one exists,
  prefixed `<number>: `.
- **Description**: two to four sentences, under 80 words. What it is. Why it is next,
  with the source: a file and section, a work item's number, a run id or a command's
  output. Which invariant, by number. What the first move is. What it waits on, when
  N > 1. No code block, and at most one path per sentence.

After the list, one footer line beginning `Read:` with what was consulted: `origin/master`'s
head and its CI state, the counts of open pull requests, GitHub issues and muster's
work items by list, the slice's done count out of its total, the Proposed ADRs, and the active
sessions. A source that could not be read is named here.

End with the one command that starts the top task: `/work-item-start <number>` when it
has a work item, `brigade work-item add --tag muster` (the title, a blank line and the
brief on stdin) when it does not, or `gh pr view <n>` for a pull request. Do not run
it.
