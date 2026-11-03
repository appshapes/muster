# Muster

[![ci](https://github.com/appshapes/muster/actions/workflows/ci.yml/badge.svg)](https://github.com/appshapes/muster/actions/workflows/ci.yml)

A command-line client for Linear, for coding agents first and people second.

Muster is one static binary that speaks to Linear's GraphQL API. Every command prints
JSON an agent can parse, one object per line, and exits with a code an agent can branch
on: `0` ok, `3` authentication, `4` not found, `5` rate limited, `6` refused, `7`
network, `8` unknown. It never prompts, never pages, never colours, and never takes the
API key on the command line. A person adds `--human`.

**Status:** 0.2.0 is released, as the crate `muster-linear`
([ADR 0011](docs/decisions/0011-release-as-muster-cli.md),
[ADR 0012](docs/decisions/0012-publish-as-muster-linear.md)): the first slice
([ADR 0007](docs/decisions/0007-build-the-trello-command-set-first.md)), the five
operations the house's Trello skills use, as `muster` commands, each tested against a
fake Linear held to recorded shapes and smoke-tested against a real workspace, and the
second slice ([ADR 0013](docs/decisions/0013-build-the-second-slice-sub-issues-attachments-and-projects.md)):
sub-issues, attachments and projects, as `--parent`, `issue attach`, `project create`
and `issue list --project`. Not yet: cycles, attachments as uploads.

## Start here

| If you want to | Read |
| --- | --- |
| Understand why Muster exists and why it is shaped for agents | [`docs/explanation/why-muster.md`](docs/explanation/why-muster.md) |
| Work on the code, human or agent | [`CLAUDE.md`](CLAUDE.md), the invariants and gates |
| Know what a word means, in Linear's terms and Trello's | [`docs/glossary.md`](docs/glossary.md) |
| Know every command, flag, exit code, bound and variable | [`docs/reference/commands.md`](docs/reference/commands.md) |
| Know what the crate does and must never do | [`crates/muster/README.md`](crates/muster/README.md) |
| Know why something was decided | [`docs/decisions/`](docs/decisions/) |
| Write documentation | [`.claude/rules/docs.md`](.claude/rules/docs.md) |
| Ask the agent to make a change | [`docs/how-to/ask-the-agent.md`](docs/how-to/ask-the-agent.md) |
| Implement a feature, from issue to commit | [`docs/how-to/implement-a-feature.md`](docs/how-to/implement-a-feature.md) |
| Work an issue, in ten minutes | [`docs/tutorials/work-an-issue-in-ten-minutes.md`](docs/tutorials/work-an-issue-in-ten-minutes.md) |
| Get the week's changes by email, or send them | [`docs/how-to/send-the-release-notes.md`](docs/how-to/send-the-release-notes.md) |
| Cut a release | [`docs/how-to/cut-a-release.md`](docs/how-to/cut-a-release.md) |
| Read the documentation as a book | `just docs`, then open `target/book/index.html` |

## The ten-minute path

Seven commands, from a checkout, against your own Linear workspace; `just quickstart`
runs the same seven in CI against the fake Linear, and
[the tutorial](docs/tutorials/work-an-issue-in-ten-minutes.md) says what each one
shows. You need a Linear API key (Linear, Settings, Security & access, Personal API
keys) and `jq`.

```sh
cargo install muster-linear                              # the binary is `muster`; from a checkout: cargo install --path crates/muster
mkdir -p ~/.config/muster && (umask 077; cat > ~/.config/muster/key)   # paste the key, then Ctrl-D
muster whoami                                         # the workspace, you, and the team muster will use
muster issue create --title "Try muster" --description "Created from the README."   # prints the issue; note its identifier
muster issue comment APP-12 --body "Read it back with issue get."                   # APP-12 is the identifier it printed
muster issue get APP-12 | jq '{title, state: .state.name, comments: [.comments[].body]}'
muster issue close APP-12 --comment "Done from the README."                         # the team's completed state
```

Each command prints one JSON line with Linear's own field names, and exits 0. The key
is read from `LINEAR_API_KEY` or from `~/.config/muster/key`, never from a flag
([ADR 0008](docs/decisions/0008-speak-to-agents-first.md)). The team is inferred when
the workspace has one; `--team <key>` or `MUSTER_TEAM` names it otherwise. No other
configuration exists.

## License

Apache-2.0, with no relicensing clause. See [`LICENSE`](LICENSE).
