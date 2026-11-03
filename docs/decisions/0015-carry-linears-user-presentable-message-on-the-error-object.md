# 0015. Carry Linear's `userPresentableMessage` on the error object, beside its message

- **Status:** Accepted
- **Date:** 2026-11-03
- **Invariants affected:** 1, 2, 4
- **Supersedes:** none

## Context

ADR 0008 fixed the error object muster writes to stderr: `{"error": {"code", "exit",
"message", "hint", "docs", "linear": {"code", "message", "requestId"}}}`, `linear`
present when Linear answered and its `message` Linear's own `message`, released in
0.1.0. Invariant 4 says every error carries Linear's own message and what to do next;
invariant 1 says a field muster prints is a field Linear returned, nothing
reconstructed; invariant 2 says a field never changes meaning once released.

Linear's validation failures do not say what failed in `message`. The `just smoke` run
of the project-status group on 2026-11-03, against the `appshapes` workspace, created
a status whose name was over 25 characters: Linear answered with `extensions.code`
`INVALID_INPUT`, the `message` "Argument Validation Error", and the reason, "name must
be shorter than or equal to 25 characters", in `extensions.userPresentableMessage`.
The error object said `Linear refused the input: Argument Validation Error` and
nothing more, and an agent reading it had to guess what to fix. Linear's error shape
has carried `userPresentableMessage` since the first probes: "Entity not found:
Issue" comes with "Could not find referenced Issue." (ADR 0008, Evidence), and the
fake Linear under `crates/muster/tests/support/` answers every validation failure
with one.

The first fix, in pull request 18, put the reason on `linear.message` after Linear's
message, joined by a colon, for the one message "Argument Validation Error". The
review blocked it: the result is a string Linear never sent, built from two of its
fields, on a released field, with no ADR.

## Decision

The error object's `linear` carries a fourth field, `userPresentableMessage`:
Linear's `extensions.userPresentableMessage` verbatim when the answer has one, `null`
when it does not, as `code` and `requestId` already are. `linear.message` stays
Linear's `message` verbatim, for every error class. Muster's own `message`, which is
muster's words, names the reason after Linear's message, joined by a colon, whenever
Linear gave a `userPresentableMessage` that differs from its `message`, so that
`--human`, which prints muster's message and hint and not the `linear` object, says
what to fix too: `Linear refused the input: Argument Validation Error: name must be
shorter than or equal to 25 characters`. The object is `{"error": {"code", "exit",
"message", "hint", "docs", "linear": {"code", "message", "userPresentableMessage",
"requestId"}}}`; ADR 0008's exit codes and every other field are unchanged.

## Alternatives considered

- **The reason folded into `linear.message`, for "Argument Validation Error" alone**
  (pull request 18's first form). Lost: a field Linear did not return, assembled from
  two it did, which invariant 1 forbids; a released field changing content, which
  invariant 2 forbids; and a rule that hangs on one message's exact text, which Linear
  may change without notice.
- **Nothing: `linear.message` as it was, the reason nowhere.** Lost: invariant 4, the
  output says what to fix, is not met for any validation failure; the smoke run's
  refusal is the evidence.
- **The reason in muster's `message` alone, no new field.** Lost: an agent that parses
  the error object gets Linear's reason only inside muster's sentence, and the field
  exists in Linear's answer under a name of its own; CLAUDE.md's rule is to copy
  Linear's name and shape.
- **`linear.detail` or `linear.reason`, a name of muster's.** Lost: a synonym for a
  Linear name (CLAUDE.md, "Linear's names"); `userPresentableMessage` is what Linear
  calls it, and the reader of Linear's documentation finds it under that name.

## Evidence

- The `just smoke` run of 2026-11-03 against `appshapes`, recorded in pull request 18:
  `project-status create` with a name over 25 characters was refused with
  `INVALID_INPUT`, the message "Argument Validation Error" and the presentable message
  "name must be shorter than or equal to 25 characters"; `scripts/smoke.sh` records
  the limit in a comment and names its status after the run's time of day.
- Linear's "Entity not found" answers, probed on 2026-11-02 (ADR 0008, Evidence),
  carry `userPresentableMessage` too ("Could not find referenced Issue."), which the
  fake answers with on every entity it cannot find and every validation failure.
- The tests through the binary in `crates/muster/tests/cli.rs`: a team key in use
  answers `linear.message` "Argument Validation Error",
  `linear.userPresentableMessage` "The team key is already in use.", and muster's
  message both; an unknown issue answers `linear.message` "Entity not found: Issue"
  and `linear.userPresentableMessage` "Could not find referenced Issue.". The unit
  tests in `crates/muster/src/error.rs` and `crates/muster/src/linear/mod.rs` hold
  the sentence and the object's shape.

## Consequences

Easier: an agent reads the reason from a field with Linear's name, or from muster's
message, and a person with `--human` sees it; a rule that depends on no message's
text.

Harder: the `linear` object has one more field to carry, `null` for the transport
failures and the HTTP 429 that come with no Linear error object; `muster::LinearError`
gains a public field, a breaking change to the library before 1.0.

Revisit: when Linear moves the reason into `message`, which the fixtures will show, or
adds a second presentable field.

## References

- ADR 0008, the error object; CLAUDE.md invariants 1, 2 and 4, "Output", "Linear's
  names"
- Pull request 18 and its review, blocker 2
- `crates/muster/src/error.rs`, `crates/muster/src/linear/mod.rs`
- `docs/reference/commands.md`, generated by `just gen-commands`
