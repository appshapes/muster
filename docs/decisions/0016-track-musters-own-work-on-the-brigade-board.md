# 0016. Track muster's own work on the Brigade Board, not in Linear

- **Status:** Accepted
- **Date:** 2026-11-13
- **Invariants affected:** 6
- **Supersedes:** feature 5 of ADR 0007, in part: muster's own work is no longer tracked
  in Linear through the `linear-*` skills. The rest of ADR 0007 stands.

## Context

Until 2026-11-13, muster's own work was tracked as Linear issues. They lived in the
`appshapes` workspace, team `APP`, project `Muster` (APP-6), and were worked through
four repository skills that call muster itself: `linear-create`, `linear-read`,
`linear-start` and `linear-done` (feature 5 of ADR 0007). A commit's subject started
with the issue's identifier (`APP-12: Add the comment command`), and the comment that
closed an issue ended with a `Session stamp:` line naming the Claude Code session.

On 2026-11-13 the house's five repositories (brigade, cavalry, muster, telder and
appshapes-www) became one Brigade team, `appshapes`, on the Brigade server. Brigade is
the plugin that connects the house's Claude Code sessions. The team has one Brigade
Board: a board of work items with lists (Wanting, Doing, Done and a hidden
Hibernating), tags and one number sequence for all five repositories, read and changed
with `brigade work-item`. The same day the owner ruled, recorded as decision 7 on
Brigade work item 272, that the old tracker skills all go in every repository, the
reading ones too, because "Linear was simply an experiment, supplanted by Brigade
Board". brigade had already ported its own Trello skills (Brigade work item 265, brigade
commit `e17a9eb`). There the owner ruled that a finished work item is moved to Done and
not closed: closing is the owner's word. That port also took two defaults, left for the owner to
overrule, which the session leading work item 272 passed to muster as binding: a work
item's comment carries no `Session stamp:` line, because the board records the member
and the session of every comment; and a commit's number is its work item's. The owner
confirmed this port of muster's process to the session that wrote this ADR, on
2026-11-13.

None of this changes the product. Muster is a command-line client for Linear, and its
commands, its fixtures, the fake Linear, `just smoke`, the smoke team `SMK` and the
key's sources stay as they are.

## Decision

Muster's own work is tracked as work items on the team's Brigade Board, each tagged
`muster`. A commit's subject starts with its work item's number and a colon
(`272: Track muster's own work on the Brigade Board`); a commit with no work item has
no prefix. A Claude Code session takes a work item from reading it to Done with the
repository skill `/work-item-start <number>`: it takes the work item (assigned to the
user, moved to Doing), works it on a branch as `docs/how-to/implement-a-feature.md`
says, lands it through `/pr`, then comments the landing on the work item and moves it
to Done. It never closes a work item. The skills `linear-create`, `linear-read`,
`linear-start` and `linear-done` are removed. `/pr` lands on the work item and
`/next-task` reads the board. A GitHub issue for the agent loop names its work item's
number in its title when it has one, because a GitHub runner cannot reach the board. The
session or person who filed the issue moves the work item to Done after the merge. An
issue carried from Linear is a work item whose description starts with
`Linear issue APP-<n>`. Its earlier commits keep that identifier, and
`git log --grep '^APP-<n>:'` still finds them.

## Alternatives considered

- **Keep Linear for muster's own work.** Muster would keep using itself every day, on
  real issues of a real workspace. It lost to the owner's ruling and to having one tracker: every
  other repository's sessions work from the board, and a muster issue in Linear is
  invisible to them. `just smoke` before each release still runs muster against a real
  workspace (invariant 1's gate).
- **Keep `linear-read` for the history.** The owner ruled it out: no muster issue
  stays open in Linear. The history is still readable without a skill, with
  `muster issue get APP-31`.
- **Use the plugin's `/brigade:work-item-start`.** It knows the board but none of
  muster's gates: the branch, the four gates, `just smoke` and the review loop of
  `/pr`. A repository skill follows `docs/how-to/implement-a-feature.md` and lands
  through `/pr`.
- **Track the work in both, with the board mirrored to Linear.** Two sources of truth
  for one piece of work. Nothing reads the second.

## Evidence

- Brigade work item 272 (`brigade work-item show 272`): the owner's decisions of
  2026-11-13, decision 7 among them, and the plan's muster section (an ADR first,
  `work-item-start` ported from `linear-start`, `pr` and `next-task` on the board, the
  commit-prefix rule changed in `CLAUDE.md` and the agents' briefs).
- Brigade work item 265 and brigade commit `e17a9eb`: the same port of `trello-start`,
  which settled where a finished work item goes and dropped the session stamp.
- Linear on 2026-11-13, read with `muster issue list` on team `APP`: two open issues of
  the project `Muster`, APP-31 (In Progress) and APP-39 (Todo); APP-1 to APP-4 are
  Linear's own onboarding issues, and team `SMK` had none open. Both were carried to
  the board that day, each with `Linear issue APP-<n>` as the first line of its
  description and its Linear comments copied over as its first comments: APP-31 is work
  item 293 and APP-39 is work item 294. Each was then canceled in Linear with a comment
  naming its work item, and `muster issue list --project Muster` printed nothing.

## Consequences

- One board holds the work of all five repositories. A session in any of them sees
  muster's work, and a work item's history (its comments, moves and the sessions that
  made them) stays with it.
- Working on muster's process needs the Brigade plugin and a checkout joined to the
  team (`.brigade.json`). Muster's own tests and gates still need no key. `just smoke`
  and `just record-fixtures` need the Linear key, as before.
- A contributor outside the team cannot read the board. The GitHub issue is their
  brief, and their commits carry no prefix.
- A GitHub runner cannot read the board. The agent loop's brief stays the GitHub
  issue's body, and its number comes from the issue's title.
- Muster no longer uses itself for its own work every day. A fault that only daily
  use would find can now first show up in `just smoke` or at a user.
- Revisit when the house leaves the Brigade Board, or when a release carries a fault in
  a command muster's old process ran every day (`issue get`, `issue list`,
  `issue create`, `issue comment`, `issue close`, `issue attach`) that `just smoke`
  did not catch.

## References

- [Implement a feature, from work item to commit](../how-to/implement-a-feature.md)
- [Ask the agent to make a change](../how-to/ask-the-agent.md)
- `.claude/skills/work-item-start/SKILL.md`, `.claude/skills/pr/SKILL.md`,
  `.claude/skills/next-task/SKILL.md`
- [ADR 0004](0004-run-agents-on-branches.md): sessions and agents work on branches.
- [ADR 0007](0007-build-the-trello-command-set-first.md), feature 5: the Linear skills
  this decision removes.
