# Work an issue in ten minutes

A tutorial: from a checkout to an issue created, commented on, read back and closed in
your own Linear workspace, in seven commands and about ten minutes. Each step says what
you will see. `just quickstart` runs steps 3 to 7 against the fake Linear in CI, so the
path is tested on every commit ([ADR 0010](../decisions/0010-test-against-a-fake-linear-held-to-recorded-shapes.md)).

You need a Rust toolchain, `jq`, and a Linear personal API key (in Linear: Settings,
Security & access, Personal API keys). The commands below write to your workspace: one
issue, one comment, one close.

## 1. Install the binary (two minutes)

```sh
cargo install muster-cli
```

The crate is `muster-cli` and the binary is `muster`
([ADR 0011](../decisions/0011-release-as-muster-cli.md)); from a checkout,
`cargo install --path crates/muster` builds the same binary. You will see
`Installed package muster-cli v0.1.0` and `muster --version` prints `muster 0.1.0`.

## 2. Put the key where muster reads it (one minute)

```sh
mkdir -p ~/.config/muster && (umask 077; cat > ~/.config/muster/key)
```

Paste the key, press Enter, then Ctrl-D. Nothing is printed. The file is readable by
you alone; muster reads it when `LINEAR_API_KEY` is unset, and never takes the key as
a flag ([ADR 0008](../decisions/0008-speak-to-agents-first.md)).

## 3. See who you are and which team muster will use (one minute)

```sh
muster whoami
```

One JSON line: your `viewer`, the `organization`, and under `muster` the key's
`source` (`file`), the `endpoint`, the `team` with its `source` (`inferred` when the
workspace has one team) and a list of `checks`, every one at level `ok`. Exit 0.

If you see `"code":"authentication"` on stderr and exit 3, the key was not found or
was refused; the `hint` says where to put it. If the `team` is `null` with a `warn`
check, the workspace has several teams: add `--team <KEY>` to the commands below, or
`export MUSTER_TEAM=<KEY>` once.

## 4. Create an issue (one minute)

```sh
muster issue create --title "Try muster" --description "Created from the README."
```

One JSON line: the issue as Linear returned it, with its `identifier` (`APP-12` in a
team whose key is `APP`), `title`, `state.name` (the team's default state, `Backlog` on a
team with Linear's default workflow), `url`, and empty `labels.nodes`. Exit 0. Note the identifier: the next
steps use it.

Run the same command again and you get exit 6 with `"code":"refused"`: an open issue
of the team already has that title, and the message names it. That guard is for an
agent that retries after a timeout; `--allow-duplicate` turns it off.

## 5. Comment on it (one minute)

```sh
muster issue comment APP-12 --body "Read it back with issue get."
```

One JSON line: the comment, with its `body`, `createdAt`, `user` and `issue.identifier`.
Exit 0. A long comment goes through a file: `--body-file notes.md`, or `--body-file -`
to read it from stdin.

## 6. Read it back (one minute)

```sh
muster issue get APP-12 | jq '{title, state: .state.name, comments: [.comments.nodes[].body]}'
```

```text
{
  "title": "Try muster",
  "state": "Backlog",
  "comments": [
    "Read it back with issue get."
  ]
}
```

Without `jq` you get the whole issue: every field of step 4, plus `comments.nodes`
oldest first and `attachments.nodes`. The number alone works inside the team:
`muster issue get 12`. An issue that does not exist is exit 4, `"code":"not_found"`.

## 7. Close it (one minute)

```sh
muster issue close APP-12 --comment "Done from the README."
```

Two JSON lines: the closing comment, then the issue with `state.type` `completed` and
`completedAt` set. Exit 0. `--cancel` moves it to the team's canceled state instead,
and `--state "In Review"` to any state by name.

## What you have

An issue created, commented on, read and closed from the terminal, with no browser,
each step one JSON line an agent can parse and one exit code it can branch on. In
Trello, this is what the house's `trello-create`, `trello-read` and `trello-done`
skills did ([glossary](../glossary.md)).

## What to read next

- [Commands, exit codes, bounds and environment](../reference/commands.md): every
  command and flag, generated from the binary.
- [Why Muster, and why a CLI for agents](../explanation/why-muster.md).
- [Implement a feature, from issue to commit](../how-to/implement-a-feature.md), to
  change muster itself.
