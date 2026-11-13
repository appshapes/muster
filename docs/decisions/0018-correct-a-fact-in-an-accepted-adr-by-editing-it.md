# 0018. Correct a fact in an accepted ADR by editing it, never its decision

- **Status:** Accepted
- **Date:** 2026-11-13
- **Invariants affected:** 6
- **Supersedes:** ADR 0001, in part: its rule that an accepted ADR is immutable except
  for its Status line. The rest of ADR 0001 stands, and a change of mind is still a new
  ADR that supersedes the old one. ADR 0001's Status now links here.

## Context

ADR 0001 made accepted ADRs immutable except for their Status line, and
`scripts/lint-docs.py` enforces it: `just lint-docs`, locally and in CI, fails a change
to an accepted ADR beyond that line. The rule protects a decision from being rewritten
after the fact. It also keeps a wrong statement of fact in place for good, because a
superseding ADR is for a change of mind, and the old ADR cannot link forward to a
correction.

ADR 0016 was accepted on 2026-11-13 with one fact wrong. It said a commit's number being
its work item's was a default of brigade's port, left for the owner to overrule. It is
the owner's own ruling of 2026-11-11, as the leading session's comment of 2026-11-13 on
Brigade work item 272 records. The decision itself was right. A reader of ADR 0016 would
still read the wrong attribution, with nothing to tell them it is wrong. The owner chose,
on 2026-11-13, to correct ADR 0016 by editing it.

## Decision

An accepted ADR may be edited to correct a statement of fact that was wrong when it was
written: a date, a count, a citation, who decided something. Each correction adds one
entry to the ADR's header, after the Supersedes line and any earlier entry:
`- **Corrected:** YYYY-MM-DD: <what was wrong, and what the ADR says now>`, wrapped onto
lines indented two spaces when it is long. It names every fact the edit changes. The
earlier entries stay as they are. The Decision section never changes. A choice that turns out wrong,
or a fact wrong in the Decision section itself, is a new ADR that supersedes the old one.
An ADR that a later one supersedes in part, not whole, keeps the Status Accepted and
links forward on its Status line: `Accepted; superseded in part by NNNN`.
`scripts/lint-docs.py` accepts a change to an accepted ADR beyond its Status line only
as a correction: one or more dated Corrected entries appended to the header, every
earlier entry kept whole, and the title, the other header fields and the Decision
section as they were. It also refuses a Corrected line outside the header, one not in
the form above, and one whose date is not a date. `scripts/lint-docs-test.py` holds
these rules.

## Alternatives considered

- **Keep ADR 0001's rule and supersede ADR 0016 with an ADR that corrects the
  attribution.** A new ADR would record no new decision, and ADR 0016 would still state
  the wrong fact with no link forward. The owner rejected it.
- **Errata in a separate file.** A reader of the ADR does not see them, and a second file
  must be kept in step with seventeen ADRs.
- **Let accepted ADRs be edited freely.** That loses what ADR 0001 protects: a decision
  rewritten after the fact, with the history only in git.

## Evidence

- ADR 0016's Context as accepted in commit `ff65cce`, and the comment of 2026-11-13 on
  Brigade work item 272 (`brigade work-item show 272`), which names the source of each of
  the three rules: two are the owner's own, and the session stamp is a default of
  brigade's port that the owner has not overruled.
- `scripts/lint-docs.py`, the `change` check, which this ADR changes.

## Consequences

- A wrong fact in an accepted ADR is fixed where readers read it, and its Corrected line
  says when and what. `git log -p` on the file shows the text before.
- The title, the header fields and the Decision section of an accepted ADR are now
  held by the gate, not by a reviewer alone. ADR 0001 and ADR 0007, which later ADRs
  supersede in part, link forward from their Status lines.
- A reviewer must still judge whether an edit outside the Decision section is a
  correction of fact or a change of mind, and whether the Corrected entry names every
  fact the edit changes. A change of mind dressed as a correction is a blocker.
- Revisit if Corrected lines start to carry changes of mind, or if a correction is
  needed in a Decision section often enough that a superseding ADR for each becomes a
  burden.

## References

- [ADR 0001](0001-record-decisions-as-adrs.md): the rule this changes in part.
- [ADR 0006](0006-build-the-docs-gate.md): the documentation gate.
- [ADR 0016](0016-track-musters-own-work-on-the-brigade-board.md): the first ADR
  corrected under this rule.
- `.claude/rules/docs.md`, Decisions (ADRs).
