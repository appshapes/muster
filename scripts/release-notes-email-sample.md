# What's new in Muster, November 2 to 8, 2026

Muster is a command-line client for Linear, built for coding agents first and for people second. **This email covers the week's commits on master**: the first slice reaches a workspace from a terminal, and the contract between muster and an agent is decided.

## What's new

### Reach your workspace from a terminal, with no configuration

`muster whoami` names the workspace, you, and the team muster will use; `muster team list`, `muster state list` and `muster label list` print what an issue can be made of, one JSON object per line. The key is read from `LINEAR_API_KEY` or from a file, never from a flag.

```in a terminal
muster whoami
muster state list
```

[Step by step →](https://github.com/appshapes/muster/blob/master/docs/tutorials/work-an-issue-in-ten-minutes.md)

### Read an issue by its identifier or its number

`muster issue get APP-12` prints the issue with its comments and attachments, under Linear's own field names; `muster issue list` filters by state, label, assignee or a search term. A number alone resolves in the team `MUSTER_TEAM` names, or in the workspace's only team.

### Branch on an exit code instead of parsing a message

Every failure has its own exit code, and the error object on stderr carries Linear's message, Linear's request id and a hint saying what to do next. `just quickstart` runs the README's path against the fake Linear, so a change that breaks the path fails before it merges.

## Decided

- ADR 0002: muster is Rust on a blocking HTTP client, with hand-written operations and four dependencies. [Read the decision →](https://github.com/appshapes/muster/blob/master/docs/decisions/0002-write-muster-in-rust-on-a-blocking-client.md)
- ADR 0007: the first slice is the five operations the Trello skills used, built as five features.
- ADR 0008: JSON Lines on stdout, an exit code per failure class, no prompt ever, the key from a file. [Read the decision →](https://github.com/appshapes/muster/blob/master/docs/decisions/0008-speak-to-agents-first.md)

## Before you update

- The key is no longer read from a flag. Put it in `LINEAR_API_KEY` or in the file muster reads, as the tutorial says.

## Try it

Not running it yet? [The ten-minute path](https://github.com/appshapes/muster#the-ten-minute-path) is seven commands from a checkout.
