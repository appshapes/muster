# 0017. Keep writing the GraphQL operations by hand, held to Linear's recorded answers

- **Status:** Accepted
- **Date:** 2026-11-13
- **Invariants affected:** 1, 6
- **Supersedes:** none. It answers the Revisit clause of ADR 0002, which stands.

## Context

An operation is one GraphQL query (a read) or mutation (a write) that muster sends to
Linear. Its selection set is the list of fields it asks Linear to return, and a fragment
is a named selection set that several operations share. ADR 0002 chose operations
written by hand over schema code generation (`graphql-client`, `cynic`) "for now". Code
generation reads Linear's schema, the typed list of everything Linear's API offers, and
writes Rust code from it, checked when muster compiles. ADR 0002 said to revisit the
choice "when the operations pass about thirty or a field name drifts from the schema
unnoticed".

On 2026-11-13, `crates/muster/src/linear/operations.rs` holds 88 operations: 27 queries
and 61 mutations, which share 20 fragments, in 1,304 lines. That is after the first two
slices (ADRs 0007 and 0013) and nineteen of the third slice's twenty groups (ADR 0014).
The twentieth, the invite group, waits on its branch with five more. The first trigger
fired on 2026-11-03, when commit `f3b7cc9` (the label commands) took the count from 29
to 34, and no ADR has answered it.

Muster does not need Rust types to print an answer. The client sends an operation's text
and gets the answer back as a `serde_json::Value`. A command prints Linear's answer as
Linear returned it (invariant 1, ADR 0008), with no Rust type between the two. What
holds an operation to Linear today is running it against Linear. `just record-fixtures`
runs each of the 27 reads against a real workspace and records the answer's shape, and
the fake Linear's answer to that read is held to the recording (ADR 0010).
`just smoke` runs commands against a real workspace before each release, and most writes
with them. Both need a key, so both run by hand, never in CI. The question is whether
88 operations now justify generating them, or a check against Linear's schema.

## Decision

Muster keeps writing its GraphQL operations by hand, in `operations.rs`, as ADR 0002
did. The truth an operation is held to is Linear's answer, through the recordings of
ADR 0010 and the `just smoke` run, not Linear's schema. Linear serves at least one field
that its schema does not list: `IssueLabel.retiredAt`, which muster prints and which
`label retire` sets. Code generated from that schema refuses 5 of muster's 88
operations, and a check against it would fail the same 5. Muster adds no schema check.

## Alternatives considered

- **`graphql_client`, one derive per operation, with typed answers.** A derive is a Rust
  macro that writes code when the crate compiles. Its build refuses `IssueLabels`,
  `IssueLabelCreate`, `IssueLabelUpdate`, `IssueLabelRetire` and `IssueLabelRestore`:
  "No field named retiredAt on IssueLabel". Muster would have to drop a field Linear
  returns, or patch the schema by hand, which is the work code generation exists to
  remove. Printing a typed answer also means serializing Linear's answer again through
  Rust structs, so the output is muster's reconstruction, not Linear's answer
  (invariant 1). The cost is not why it lost: 8 new crates and a 9.7-second clean build
  of 83 derives.
- **`cynic`, operations written as Rust structs checked against the registered schema.**
  It needs the same schema, so it refuses the same field. Each of the 88 operations
  would be rewritten as Rust structs, and its GraphQL would be generated from them,
  written nowhere a reader can read it as GraphQL. It adds 42 crates to muster's 54,
  against ADR 0002's small tree.
- **Keep writing by hand, and check every operation against a copy of Linear's schema
  kept in the repository, in `just test`.** It is the one alternative that would run in
  CI, and it would catch a misspelt field in any operation when the tests run, the
  writes `just smoke` does not send to Linear included (see the Evidence). It lost on three counts. First, it had
  nothing to catch: on 2026-11-13 every field muster selects is either in Linear's
  schema or served by Linear without being in it. Second, it needs a list of fields
  Linear serves but does not publish, kept by hand, and a copy of the schema that lags
  Linear: Linear's published schema and the one its API reports differ in 82 types. A
  field Linear drops is seen only when that copy is fetched again, which is the same
  manual step as recording the fixtures again. Third, it needs a GraphQL parser: a new
  dependency, or a Python package outside the standard library the scripts keep to
  (ADR 0003).

## Evidence

All measured on 2026-11-13, on an Apple M5 Pro with 18 cores and 64 GB, with rustc 1.99.0.
The key used was an admin's key of the `appshapes` workspace, on Linear's Free plan.

- **The count.** A scratch program that depends on the crate printed every operation in
  `READS` and `WRITES`: 88 operations with 88 distinct names, 27 queries and 61
  mutations.
- **The check against Linear's two schemas,** with `graphql-core` 3.2.6. The schema
  Linear's API reports came from introspection, GraphQL's query for its own schema: 810
  types with fields, 159 query fields and 336 mutation fields. The published schema is
  the schema file of Linear's SDK, `packages/sdk/src/schema.graphql` in
  github.com/linear/linear at commit `87d638c0b1` (2026-11-11), 46,122 lines. Against
  each, 83 operations are valid and the same 5 are not, each with "Cannot query field
  'retiredAt' on type 'IssueLabel'". The two schemas also differ from each other: 75
  types are only in the published file and 7 only in the one the API reports.
- **Linear serves the field.** `muster label list --limit 1 --trace` against the
  `appshapes` workspace printed Linear's answer, request id `a59f6179ee091eec-IAD`, with
  `"retiredAt":null` on the label. The fixture `IssueLabels.json`, recorded from a real
  answer on 2026-11-03 (commit `f3b7cc9`), carries `retiredAt` too.
- **The second trigger has not fired.** ADR 0002's "a field name drifts from the schema
  unnoticed" did not happen: the 83 other operations are valid against both schemas.
  Whether Linear still answers each field as recorded is what the next
  `just record-fixtures` and `just smoke` show; the fixtures were last recorded on
  2026-11-03.
- **What runs against Linear by hand.** All 27 reads, through `just record-fixtures`.
  Of the 61 writes, `scripts/smoke.sh` does not run `user change-role`, `user suspend`,
  `user unsuspend` or `user revoke-sessions`: the workspace's two users, its owner and an
  app, leave no safe target for them (commit `7e7af76`). It runs
  `team create --name "Smoke tests" --key SMK` on every run, but only a workspace's
  first run sends `TeamCreate` to Linear. From the second run on, the smoke team exists,
  and `team create` refuses a name a team already has (exit 6) before it sends the
  mutation. `TeamCreate` last reached Linear on 2026-11-03, in the first smoke run
  against `appshapes` (commit `89f28fd`), and its text has not changed since.
  `TeamUpdate` and `TeamUnarchive` send its `TeamFields` fragment on every run, but not
  its `teamCreate(input:, copySettingsFromTeamId:)` call. The four `user` writes and
  `TeamCreate` were valid in the check above.
- **The costs.** Muster today: a 12.8-second clean release build, 54 crates in its
  normal and build tree, and a 4.9 MB binary. `graphql_client` 0.16.0 adds 8 crates
  (`graphql_client`, `graphql_client_codegen`, `graphql_query_derive`, `graphql-parser`,
  `graphql-introspection-query`, `combine`, `thiserror`, `thiserror-impl`). A clean
  release build of 83 derives against the schema the API reports, 275 KB of schema text,
  took 9.7 seconds. `cynic` 3.14.0 with its `rkyv` feature adds 42 crates. A clean
  release build that registers the same schema and derives one query took 8.9 seconds.

## Consequences

- Operations stay as GraphQL text in one file, each named as `--trace`, the fake and the
  fixtures name it. Muster keeps printing every field Linear returns, including the ones
  its schema does not list, and keeps four direct dependencies.
- A misspelt or dropped field is caught only when the operation runs against Linear: a
  read when `just record-fixtures` records it, a write when `just smoke` sends it. `just
  test` does not catch it, because the fake answers by the operation's name, and its
  answers to writes are not held to any recording. The four `user` writes above reach
  Linear through neither `just record-fixtures` nor `just smoke`, and `TeamCreate` only
  through the first smoke run against a workspace.
- Revisit when a released command fails against Linear on a field it selects, which is
  drift or a misspelling that went unnoticed; when Linear's schema lists every field
  muster selects and the operations pass about two hundred; or when a command must
  compute over Linear's answer rather than print it, which would make typed answers
  worth having.

## References

- [ADR 0002](0002-write-muster-in-rust-on-a-blocking-client.md): the choice and the
  Revisit clause this answers.
- [ADR 0003](0003-lay-out-the-workspace.md): the small tree and the standard-library
  scripts.
- [ADR 0008](0008-speak-to-agents-first.md) and
  [ADR 0010](0010-test-against-a-fake-linear-held-to-recorded-shapes.md): the answer
  printed as Linear returned it, and the fake held to recorded shapes.
- `crates/muster/src/linear/operations.rs`, `crates/muster/tests/fixtures/IssueLabels.json`,
  `scripts/smoke.sh`
- Brigade work item 296.
