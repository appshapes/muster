# 0008. Speak to agents first: JSON Lines, exit codes, no prompt, the key from a file

- **Status:** Accepted
- **Date:** 2026-11-02
- **Invariants affected:** 1, 2, 3, 4, 5
- **Supersedes:** none

## Context

Muster's first user is a coding agent running it through a `Bash` tool: no terminal, a
pipe for stdout, a timeout, and a parser on the other end that is a language model or
`jq`. The agent cannot answer a prompt, cannot scroll a pager, cannot read colour
codes, and branches on what it can see: the exit code and the output. A person uses the
same binary at a terminal, less often. The Trello skills that muster replaces pass
`--format json` on every call and parse the result, and every one of them documents
the resolution of a name to an id by hand.

Three risks shape the contract. A command that prints a table by default and JSON on a
flag is used wrong by the agent that forgets the flag. A tool that detects a terminal
and changes its output format behaves differently in a test, in a skill and at a
keyboard, and the difference is found in production. And the API key, if it travels on
argv, is in `ps`, in the shell history, in the `Bash` tool's log and in the Actions
log of a public repository.

## Decision

**Output.** stdout carries the answer as JSON Lines: one JSON object per line, a list
as one object per line, and nothing else. Field names are Linear's own, nested as
Linear nests them (`state.name`, `team.key`), with no field muster made up. `--human`
renders the same data as text for a person; the format never depends on whether stdout
is a terminal. stderr carries one JSON error object on failure, `--trace`'s requests
and responses with the key redacted, and `--human`'s notes.

**Exit codes.** `0` ok. `1` muster's own failure, or an answer it does not understand.
`2` usage, which is `clap`'s. `3` authentication: no key, or Linear answered 401. `4`
not found: Linear's `INPUT_ERROR` whose message begins "Entity not found", or no issue
for the number or title. `5` rate limited: HTTP 429 or the `RATELIMITED` code, with the
reset time from `x-ratelimit-requests-reset`. `6` refused: Linear's validation, or a
muster guard such as a duplicate title. `7` network: the connection, the name
resolution, TLS or a timeout before the request was sent, or on a query. `8` unknown: a
mutation was sent and no answer came, so the request may have landed; the hint says
how to check. The error object is `{"error": {"code", "exit", "message", "hint",
"docs", "linear": {"code", "message", "requestId"}}}`, `linear` present when Linear
answered.

**No prompt, ever.** Every input is a flag, a file or stdin named `-`. `clippy.toml`
disallows `std::io::stdin` so the rule cannot erode. No pager. No colour.

**The key.** `LINEAR_API_KEY` if set; else the file `MUSTER_KEY_FILE` names; else
`~/.config/muster/key` (`$XDG_CONFIG_HOME/muster/key` when that variable is set),
trimmed of whitespace. Never a flag. Never printed, never in `--trace`, never in a
fixture. The team: `--team <key>`, else `MUSTER_TEAM`, else the workspace's only team,
else exit 2 naming the teams. The endpoint: `MUSTER_ENDPOINT`, for the fake Linear in
tests and the quickstart, else Linear's.

**Discovery.** `muster schema` prints the command tree, the exit codes, the bounds and
the environment variables, as JSON or as the markdown that is
`docs/reference/commands.md`, so an agent reads the surface from the binary it has.

**Guards.** `issue create` refuses a title that an open issue of the team already has,
exit 6 naming it, unless `--allow-duplicate`: an agent that retries after a timeout
makes one issue, not two.

## Alternatives considered

- **A table by default, `--json` on request, as `gh` does.** Lost: `gh` is for people
  first; muster's first user forgets flags and is the one who pays.
- **JSON when stdout is not a terminal, text when it is.** Lost: the same command
  behaves differently in a skill, in a test and at a keyboard, and the agent's tests
  pass while the person's session surprises them.
- **One JSON document per command with an `items` array.** Lost: JSON Lines lets
  `head`, `grep`, `wc -l` and a streaming reader work, and `jq -s` makes the array when
  one is wanted.
- **Flattened or renamed fields (`stateName`, `teamKey`).** Lost: every rename is a
  field to document and a mapping for the reader of Linear's documentation to undo;
  invariant 1 is cheaper to keep when the field is Linear's.
- **One exit code for every failure.** Lost: an agent that cannot tell not-found from
  rate-limited retries the wrong one.
- **`--key <value>`.** Lost: argv is public on a runner and in every log.
- **An interactive `muster login`.** Lost: it prompts, and the agent cannot answer.
- **The key in `~/.linear` or `~/.musterrc`.** Lost: `~/.config/<tool>/` is where
  `gh`, `git` and the XDG specification put it, and one convention is enough.
- **Retry on rate limit with a sleep.** Lost: a sleep inside muster is a bound the
  agent cannot see (invariant 3); the exit code and reset time let it decide.

## Evidence

- Linear's error shapes, probed on 2026-11-02 and kept under
  `crates/muster/tests/fixtures/`: a bad or missing key is HTTP 401 with
  `AUTHENTICATION_ERROR`; a missing issue or team is HTTP 200 with `INPUT_ERROR` and the
  message "Entity not found: Issue" or "… Team"; a malformed query is HTTP 400 with
  `GRAPHQL_PARSE_FAILED` or `GRAPHQL_VALIDATION_FAILED`; every answer carries
  `x-request-id` and the successful ones `x-ratelimit-requests-remaining` and `-reset`
  (milliseconds since the epoch).
- Cavalry's and Brigade's Trello skills pass `--format json` on every call and resolve
  names to ids by hand in prose, which the `list` commands and `--team` make
  unnecessary.
- The `brigade` CLI's lesson of 2026-10 on long command lines: a secret or a long body
  on argv is refused or logged; a file or stdin is neither.

## Consequences

Easier: a skill is a few `muster` lines and a `jq` filter; a test asserts an exit code;
a person reads the same fields the agent does; a leaked key cannot come from muster.

Harder: a person at a terminal types `--human`; a field Linear lacks cannot be added
without an ADR; the duplicate guard costs one query per create.

Revisit: when a field is needed that Linear does not return; when an exit code is
needed that the table lacks (new ADR, the table is append-only); when Linear changes an
error shape, which the fixtures will show.

## References

- `CLAUDE.md`, invariants 1 to 5, "Output", "The key"
- ADR 0007 (the commands), ADR 0010 (the fixtures)
- `docs/reference/commands.md`, generated by `just gen-commands`
- XDG Base Directory Specification: <https://specifications.freedesktop.org/basedir-spec/latest/>
