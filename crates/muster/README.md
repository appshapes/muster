# muster

The one crate (ADR 0003), published as `muster-linear` because `muster` and `muster-cli`
on crates.io belong to another project (ADR 0011, ADR 0012): `cargo install muster-linear`
gives the `muster` binary, and `cargo add muster-linear` gives the library, whose crate
name is `muster`, with `muster::Linear`, the client the binary is built on.

## What this does

- The binary: `muster <command>` speaks to Linear's GraphQL API and prints one JSON
  object per line with Linear's own field names, exit 0, or one JSON error object on
  stderr with an exit code per failure class (ADR 0008). The commands of the first slice
  are `whoami`, `team list`, `state list`, `label list`, `project list`, `user list`,
  `issue get`, `issue list`, `issue create`, `issue comment`, `issue close`,
  `issue update` and `schema` (ADR 0007), and the second slice adds `--parent` and
  `--project` on `issue list` and its kin, `issue attach` and `project create`
  (ADR 0013); `docs/reference/commands.md` is generated from `muster schema` and is the
  reference.
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

The library behind the binary: resolve the key, the team and the endpoint as the binary
does, run one named operation, read Linear's answer. This is the crate's doctest; it
compiles on every `just docs` and is not run there, because it reaches Linear with the
key of whoever runs it (`no_run`).

```rust,no_run
use muster::linear::operations::{ISSUES, VIEWER};
use muster::{Config, Error, Linear};
use serde_json::json;

fn main() -> Result<(), Error> {
    // LINEAR_API_KEY, else MUSTER_KEY_FILE, else ~/.config/muster/key (ADR 0008).
    let config = Config::load(None, false);
    let linear = Linear::connect(&config)?;

    let me = linear.run(&VIEWER, json!({}))?;
    println!("{} in {}", me["viewer"]["email"], me["organization"]["name"]);

    // Linear's own filter objects and field names, as `muster issue list` sends them.
    let filter = json!({ "state": { "type": { "in": ["unstarted", "started"] } } });
    let open = linear.run(&ISSUES, json!({ "first": 5, "filter": filter, "orderBy": "updatedAt" }))?;
    for issue in open["issues"]["nodes"].as_array().into_iter().flatten() {
        println!("{} {} ({})", issue["identifier"], issue["title"], issue["state"]["name"]);
    }
    Ok(())
}
```

Running it against a workspace prints the key's owner and up to five open issues, one a
line, such as `"APP-5" "Try muster" ("Backlog")`; a missing key is an `Error` whose
`code` is `Authentication` and whose `to_json()` is the object the binary prints.

## What to read next

- [`docs/reference/commands.md`](../../docs/reference/commands.md), every command, exit
  code, bound and variable, generated from the binary.
- [ADR 0007](../../docs/decisions/0007-build-the-trello-command-set-first.md), the first
  slice, and [ADR 0013](../../docs/decisions/0013-build-the-second-slice-sub-issues-attachments-and-projects.md),
  the second; [ADR 0008](../../docs/decisions/0008-speak-to-agents-first.md), the agent
  contract; [ADR 0010](../../docs/decisions/0010-test-against-a-fake-linear-held-to-recorded-shapes.md),
  how it is tested; [ADR 0011](../../docs/decisions/0011-release-as-muster-cli.md) and
  [ADR 0012](../../docs/decisions/0012-publish-as-muster-linear.md), how it is released
  and why under this name.
- [Work an issue in ten minutes](../../docs/tutorials/work-an-issue-in-ten-minutes.md),
  the tutorial.
