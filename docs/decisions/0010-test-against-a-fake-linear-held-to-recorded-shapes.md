# 0010. Test against a fake Linear in process, held to recorded response shapes

- **Status:** Accepted
- **Date:** 2026-11-02
- **Invariants affected:** 1, 5, 6
- **Supersedes:** none

## Context

Invariant 1 says a field muster prints is a field Linear returned, and the gate for it
runs every command on every commit, on two operating systems, on a public repository
whose pull requests may come from forks. A test that needs the house's API key cannot
run there: a fork's pull request has no secrets, and a test that creates issues in the
house's workspace on every push is not a test. A test that mocks the HTTP client inside
the process does not exercise the binary, the exit codes, the key sources or the
output, which is what the invariants are about.

Linear's answers have a shape muster must not invent. The shape is read once from the
real API and can change; the fake has to be held to it, or the tests pass while Linear
has moved.

## Decision

The integration tests under `crates/muster/tests/` run the built binary, through
`CARGO_BIN_EXE_muster`, against a fake Linear that lives in
`crates/muster/tests/support/fake_linear.rs`: an HTTP/1.1 server on a loopback port,
started on a thread inside the test process, that reads each POST's JSON, dispatches on
the `operationName` muster sends, and answers from an in-memory workspace with a team,
its states and labels, a project, two users and a few issues with comments. Mutations
change that state, so a test creates, reads, comments on and closes an issue through the
binary and sees each step in the next. The fake refuses a missing or wrong key as Linear
does (401, `AUTHENTICATION_ERROR`), answers an unknown issue as Linear does (200,
`INPUT_ERROR`, "Entity not found: Issue"), and has a control handle through which a test
asks it to answer the next named operation with a rate limit, a server error, or by
closing the connection after reading the request, which is how the exit 8 test of
invariant 1 is written. The binary reaches it through `MUSTER_ENDPOINT`.
`crates/muster/examples/fake-linear.rs` serves the same module as a process, for
`just quickstart`.

The fake is held to Linear by fixtures: `just record-fixtures` runs every read
operation against a real workspace with the key, strips every value to its JSON type
(`"<string>"`, `0`, `true`, `null`), and writes one file per operation under
`crates/muster/tests/fixtures/`. A test asserts that the fake's answer to each
operation has the fixture's key tree, so a field the fake forgets, or a field Linear
dropped since the recording, fails the suite. The fixtures hold shapes only: no
identifier, name, address or key of the house's workspace enters the repository.

`just smoke` runs the ten-minute path against a real workspace with the key, by hand,
and its output goes in the commit message of a feature that claims to work against
Linear.

## Alternatives considered

- **Record and replay real HTTP exchanges (VCR style).** Lost: a recording of a create
  holds the house's identifiers and names, which do not belong in a public repository,
  and a replay cannot answer a sequence the recording did not see.
- **A mock inside the process.** Lost: it tests the client, not the binary; the exit
  codes, the key sources and the output are outside it.
- **The real API in CI with a secret.** Lost: forks have no secrets, every push would
  write to the house's workspace, and a Linear outage would be a red `master`.
- **A fake in Python.** Lost: a second language for the fake and the tests to share
  state in, and `cargo test` would need Python to pass. The scripts under `scripts/`
  are Python because they run outside the build; the fake runs inside it.
- **A fake that parses GraphQL.** Lost: muster names every operation, so the fake can
  dispatch on the name, and a parser is more code than the tool.
- **No fixtures; trust the fake.** Lost: the fake would drift from Linear unnoticed,
  which is a lie about Linear by another route.

## Evidence

- Linear's error shapes and headers, probed on 2026-11-02 (ADR 0008, Evidence).
- Cavalry's conformance suites hold three implementations to one set of vectors
  written by a script in another language (Cavalry ADR 0008, `spec/vectors/`); the
  fixture-and-fake split here is the same shape, with the real API as the vector
  writer.
- The first suite's run time on both runners is cited in the commit that lands feature
  1 of ADR 0007.

## Consequences

Easier: every command has a test through the binary that runs anywhere; a new field is
a fixture line, a fake line and an output line, in that order; a Linear change shows
as a failed fixture test after the next `just record-fixtures`.

Harder: the fake is a small server to maintain, and the fixtures are re-recorded by
hand with the key when Linear changes; a behaviour the fake does not model is a
behaviour muster cannot test without `just smoke`.

Revisit: when the fixtures are re-recorded and a shape has moved (a new ADR if the
output changes); when the fake passes about a thousand lines (split by operation).

## References

- `CLAUDE.md`, invariant 1 and its gate; ADR 0007, ADR 0008
- `crates/muster/tests/support/fake_linear.rs`, `crates/muster/tests/fixtures/`,
  `crates/muster/examples/fake-linear.rs`, `justfile` (`test`, `quickstart`, `smoke`,
  `record-fixtures`)
- Cavalry ADR 0008 and `spec/README.md` in <https://github.com/appshapes/cavalry>
