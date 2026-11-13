# Why Muster, and why a CLI for agents

Muster exists because coding agents work Linear issues from a checkout, as the house's
own did when muster was built, and nothing Linear ships lets a program do that with an
exit code it can branch on. (Muster's own work has since moved to the house's Brigade
Board: [ADR 0016](../decisions/0016-track-musters-own-work-on-the-brigade-board.md).) Muster
is one binary that prints one JSON object per line, exits with a documented code per
failure class, never prompts, and never takes the key on the command line. A person can
use it too, with `--human`, but the agent is the first user and every default is set
for it.

## The problem

A coding agent that works an issue needs five things from the tracker: read the issue,
create one, comment, close, and resolve a name to an id. Before muster, the house did
this through four Claude Code skills wrapping a third-party command-line client, with
`--format json` on every call and the resolution of names to ids written out in prose
for the agent to follow. In Trello, this is the `trello` CLI and the `trello-read`,
`trello-create`, `trello-done` and `trello-start` skills
([glossary](../glossary.md)).

Linear has four surfaces: the web application, the GraphQL API, the TypeScript SDK
`@linear/sdk`, and an MCP server. None is a command-line client. The web application is
for people. The GraphQL API needs a query language, a UUID for everything and a cursor
for every list. The SDK needs Node and an install step on every runner. The MCP server
needs an MCP host and gives the agent tools, not a process with an exit code it can
compose in a shell. The third-party clients named `linear-cli` on GitHub, npm, JSR,
crates.io and PyPI collide in name and are built for a person at a terminal: a table by
default, a prompt for a missing value, colour.

## What an agent needs

An agent runs a command through a shell tool, with a pipe for stdout and a timeout. It
cannot answer a prompt, cannot scroll a pager, and cannot read colour codes. It branches
on what it can see. So muster gives it, in this order of importance:

- **One JSON object per line** on stdout, with Linear's own field names, nested as
  Linear nests them. `head`, `grep`, `wc -l` and `jq` work on it without a flag.
- **An exit code per failure class**: `3` authentication, `4` not found, `5` rate
  limited, `6` refused, `7` network, `8` unknown. An agent that cannot tell not-found
  from rate-limited retries the wrong one.
- **One error object** on stderr with muster's code, Linear's message and the reason
  it puts beside it, Linear's request id and what to do next.
- **No prompt, ever.** Every input is a flag, a file, or stdin named `-`. The lint
  configuration disallows `std::io::stdin` so the rule cannot erode.
- **The key from a file or the environment**, never from argv, which is in `ps`, the
  shell history, the shell tool's log and the Actions log of a public repository.
- **Discovery from the binary.** `muster schema` prints the command tree, the exit
  codes, the bounds and the environment variables, as JSON or as the markdown that is
  the reference page. The agent reads the surface it has, not the one it remembers.
- **A duplicate guard.** `issue create` refuses a title an open issue of the team
  already has, unless told otherwise, so an agent that retries after a timeout makes
  one issue, not two.

[ADR 0008](../decisions/0008-speak-to-agents-first.md) records these choices and the
alternatives that lost.

## Why Rust, and why a blocking client

A command makes one to three requests and exits. It never holds a connection open and
never waits on two requests at once. So muster is one Rust crate on a blocking HTTP
client with its own TLS, no async runtime and no schema code generation, and the
direct dependencies are four crates. The binary installs with `cargo install` or as one
release artifact, and a CI runner needs nothing else. Go would have served the binary
as well; Rust won because the house's gates, lints and agent workflows are already
Rust-shaped in Cavalry, the sibling repository, and port by copying.
[ADR 0002](../decisions/0002-write-muster-in-rust-on-a-blocking-client.md) is the
decision, with Go, the TypeScript SDK and Python as the alternatives.

## Why a fake Linear in the tests

The first invariant is that muster never lies about Linear: a field it prints is a
field Linear returned. The test for that has to run on every commit, on a public
repository, where a pull request from a fork has no secrets and a test that writes to
the house's workspace is not a test. So the tests run the built binary against a fake
Linear that starts inside the test process, answers by operation name from an
in-memory workspace, and can be told to rate-limit, fail or drop the next request. The
fake is held to Linear by fixtures: recorded real answers with every value stripped to
its type, so a field the fake forgets or Linear drops fails the suite.
[ADR 0010](../decisions/0010-test-against-a-fake-linear-held-to-recorded-shapes.md)
is the decision.

## What Muster is not

- Not a sync tool. It holds no state, caches nothing, and every answer is Linear's at
  the moment of the call.
- Not an MCP server. An agent that has a shell has muster; an MCP host can wrap it.
- Not a replacement for Linear's own clients for people. The web application is where a
  person plans; muster is where a program acts.
- Not the whole API. The first slice is the five operations the agents use
  ([ADR 0007](../decisions/0007-build-the-trello-command-set-first.md)); each command
  past them is a new decision.
