# muster

The one crate (ADR 0003): `cargo install --path crates/muster` gives the `muster`
binary, and the library behind it is `muster::Linear`, the client the binary is built
on. The crate is not published yet, and its published name is decided at the first
release (ADR 0003): the name `muster` on crates.io belongs to another project.

## What this does

- The binary: `muster <command>` speaks to Linear's GraphQL API and prints one JSON
  object per line with Linear's own field names, exit 0, or one JSON error object on
  stderr with an exit code per failure class (ADR 0008). The commands of the first slice
  are `whoami`, `team list`, `state list`, `label list`, `project list`, `user list`,
  `issue get`, `issue list`, `issue create`, `issue comment`, `issue close`,
  `issue update` and `schema` (ADR 0007); `docs/reference/commands.md` is generated
  from `muster schema` and is the reference.
- The library: `muster::Linear` holds the key and the endpoint and runs one named
  operation at a time; `muster::Config` resolves the key, the team and the endpoint from
  the environment and the key file; `muster::Error` is the error object with its
  `ErrorCode` and exit code.
- Needs a key and nothing else: `LINEAR_API_KEY`, or the file `MUSTER_KEY_FILE` names, or
  `~/.config/muster/key`. `MUSTER_TEAM` names the team in a workspace with several;
  `MUSTER_ENDPOINT` points the binary at the fake Linear in tests (ADR 0010).

In Trello, this is the `trello` CLI the house's skills wrapped
([glossary](../../docs/glossary.md)).

## What it must never do

- Prompt, page or colour. An agent is at the keyboard (invariant 2); `std::io::stdin` is
  disallowed by `clippy.toml`.
- Print a field Linear did not return, or exit 0 for an operation that did not happen
  (invariant 1). A mutation whose answer never came exits 8.
- Take the key from argv, print it, or write it anywhere (ADR 0008).
- Sleep through a rate limit, fetch past the page cap, or retry without a bound
  (invariant 3).
- Panic. `unwrap`, `expect` and `panic` are denied by the workspace lints; every failure
  is an `Error` with a code and a hint.

## Example

Not yet. The example lands with feature 1 of ADR 0007, in the same commit as the crate's
first public item; until then this section says so instead of showing an API that does
not exist (ADR 0003).

## What to read next

- [`docs/reference/commands.md`](../../docs/reference/commands.md), every command, exit
  code, bound and variable, generated from the binary.
- [ADR 0007](../../docs/decisions/0007-build-the-trello-command-set-first.md), the first
  slice; [ADR 0008](../../docs/decisions/0008-speak-to-agents-first.md), the agent
  contract; [ADR 0010](../../docs/decisions/0010-test-against-a-fake-linear-held-to-recorded-shapes.md),
  how it is tested.
- [Work an issue in ten minutes](../../docs/tutorials/work-an-issue-in-ten-minutes.md),
  the tutorial.
