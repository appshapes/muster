# 0007. Build the Trello command set first, in five features

- **Status:** Accepted
- **Date:** 2026-11-02
- **Invariants affected:** 1, 2, 5, 6
- **Supersedes:** none

## Context

The owner's brief of 2026-11-02: muster starts with the operations the house has
already wrapped for Trello, because those are the ones its agents use every day. The
wrappers are the `trello-read`, `trello-create`, `trello-done` and `trello-start`
skills under `.claude/skills/` in Cavalry and Brigade. Read together they call five
Trello operations:

1. read a card by number, title or id, with its description, comments, checklists,
   attachments, list and labels, and resolve the bare number a commit subject cites;
2. create a card with a title, a board, a list, exactly one project label and a
   description from a markdown file, the number assigned by Trello;
3. comment on a card, every comment ending with a session stamp line;
4. close a card: a closing comment, mark complete, archive;
5. list boards, lists and labels, to resolve names to ids.

In Linear's terms a card is an issue, a board is a team, a list is a workflow state, a
label is a label, and the number is the issue's `number` inside its team, written as
the identifier `APP-12`. A project is a Linear project, which Trello has no word for.
The house's Linear workspace on 2026-11-02 has one team, `APP`, with the default
states (Backlog, Todo, In Progress, In Review, Done, Canceled, Duplicate) and three
labels (Bug, Improvement, Feature); its first four issues are Linear's onboarding
issues.

The slice has to be usable by an agent at every step, so each feature ships with its
tests against the fake Linear (ADR 0010), its line in the tutorial, and the gate it is
the first to need (`CLAUDE.md`, Gates).

## Decision

The first slice is five features, in this order, each a Linear issue once the
workspace tracks muster's own work and each landed through the loop of ADR 0004 or by
the developer's direct commit:

1. **Reach the workspace.** `muster whoami`; `team list`, `state list`, `label list`,
   `project list`, `user list`; the key sources of ADR 0008; `MUSTER_TEAM` and
   `MUSTER_ENDPOINT`; `muster schema` with `--format json|markdown`. Builds: the fake
   Linear and the test harness, `just lint-commands` and `docs/reference/commands.md`,
   the first `just quickstart`.
2. **Read an issue.** `issue get <identifier | number | uuid>` with its description,
   state, labels, project, assignee, priority, comments and attachments; `issue list`
   with `--team`, `--state`, `--label`, `--assignee`, `--search` and `--limit`, which
   is also how a title is resolved. Builds: the bounds of `bounds.rs` and the cap
   behaviour of invariant 3.
3. **Create and comment.** `issue create` with `--title`, `--team`, `--state`,
   `--label`, `--description-file` or `--description`, `--priority`, `--project`,
   `--assignee`, and the duplicate-title guard with `--allow-duplicate`;
   `issue comment <id>` with `--body` or `--body-file`. Builds: the exit 8 test of
   invariant 1.
4. **Close and update.** `issue close <id>`, to the team's completed state or
   `--state`, with `--comment` or `--comment-file` first and `--cancel` for the
   canceled state; `issue update <id>` for title, state, labels, priority, assignee,
   project and description.
5. **Dogfood.** The `linear-read`, `linear-create`, `linear-done` and `linear-start`
   skills, ports of the four Trello skills calling `muster`, replace the Trello ones in
   this repository's `.claude/skills/`, and muster's own work is tracked in Linear
   from then on.

The slice is done when the README's ten-minute path runs in CI against the fake on
Linux and macOS, `just smoke` passes against the `appshapes` workspace, and the five
Trello operations above have a `muster` command each.

Out of the slice: cycles, sub-issues, relations, attachments as uploads, projects
beyond listing and assignment, reactions, and anything Linear's web application does
that an agent working an issue does not need. Each is a new ADR or a line in the next
slice's.

## Alternatives considered

- **Mirror Linear's whole API.** Lost: the brief names the five operations, and every
  command past them is surface a stranger has to understand and a test the fake has to
  answer.
- **Start with `issue create`.** Lost: an agent that cannot read what it created cannot
  check its work, and the key, the team inference and the output contract are needed
  by every command, so they come first.
- **One `muster issue` command with many flags.** Lost: an agent discovers a command
  tree through `--help` and `muster schema`; a verb per operation reads as the Trello
  skills do.
- **Keep Trello's words (`card`, `board`, `list`).** Lost: muster's names are Linear's
  (`CLAUDE.md`, "Linear's names"); the glossary carries the mapping once.

## Evidence

- The four Trello skills in Cavalry (`.claude/skills/trello-{read,create,start}/`)
  and Brigade (`trello-done`), read on 2026-11-02, call the five operations above and
  nothing else.
- The `appshapes` workspace on 2026-11-02, read with the key: one team, seven states,
  three labels, no projects, four onboarding issues.

## Consequences

Easier: each feature has a stated scope, a gate and a place in the tutorial; the Trello
skills can be ported line for line once feature 4 lands.

Harder: an agent that wants a cycle or a sub-issue waits for the next slice.

Revisit: when the slice is done (the next slice's ADR); when a Trello skill needs an
operation the slice lacks.

## References

- `CLAUDE.md`, "Working rules for agents"; ADR 0008, ADR 0010
- `docs/tutorials/work-an-issue-in-ten-minutes.md`, `docs/glossary.md`
- Cavalry: `.claude/skills/trello-read/SKILL.md`, `trello-create/SKILL.md`,
  `trello-start/SKILL.md`; Brigade: `.claude/skills/trello-done/SKILL.md`
