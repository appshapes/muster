# 0001. Record decisions as ADRs in the repository

- **Status:** Accepted; superseded in part by 0018
- **Date:** 2026-11-02
- **Invariants affected:** 6
- **Supersedes:** none

## Context

Muster is built by a small distributed team, with most of the code written by coding
agents, for coding agents to use. Neither the people nor the agents can walk over to a
desk and ask why something is the way it is. Cavalry, the sibling repository whose
practices this one ports, records every such answer as an Architecture Decision Record
(Cavalry ADR 0001, 2026-10-10) and has found that the records are what an agent reads
before it changes anything. Muster's position is the same: a stranger can understand the
tool from the repository alone (`CLAUDE.md`, invariant 6).

## Decision

Every decision that a new team member would ask "why" about is recorded as an
Architecture Decision Record in `docs/decisions/`, numbered, copied from
`0000-template.md`. A decision discussed in chat, on a call or in a pull request thread
does not exist until an ADR records it. Accepted ADRs are immutable except for their
Status line; a change of mind is a new ADR that supersedes the old one.

An ADR is required for any change to an output field, an exit code, a key source, a
flag's meaning once released, a dependency, or an invariant in `CLAUDE.md`.

## Alternatives considered

- **Design docs in a wiki or Notion.** Lost: not versioned with the code, not reviewable
  in the same commit, invisible to an agent working from the checkout.
- **Rationale in code comments only.** Lost: a comment explains a line, not a choice
  between alternatives, and it does not record what was rejected.
- **Decisions in pull request descriptions.** Lost: hard to find later, and written for
  the reviewer of that change, not for the reader two years on.

## Evidence

Cavalry: 43 ADRs between 2026-10-10 and 2026-11-02, read by its writer and reviewer
agents on every run (`.claude/agents/`), with `just lint-docs` refusing an edit to an
accepted one. No measurement of its own; this is a process decision made before code
exists.

## Consequences

Easier: onboarding, agent context, auditing why a rule exists. Harder: a change to the
command surface carries a writing step before the code step. Revisit if ADRs stop being
read, which the writer agent's pull request bodies would show.

## References

- `CLAUDE.md`, invariant 6 and the Documentation rules
- `.claude/rules/docs.md`
- Cavalry ADR 0001, `docs/decisions/0001-record-decisions-as-adrs.md` in
  <https://github.com/appshapes/cavalry>
