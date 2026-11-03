# Changelog

All notable changes to Muster are recorded here, in the reader's words, one line per
user-visible change, added in the same commit as the change. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Unreleased

Nothing yet.

## 0.3.0 - 2026-11-03

The first block of the third slice ([ADR 0014](docs/decisions/0014-build-the-third-slice-administrative-commands.md)),
for an agent that administers a workspace: `workspace get` and `workspace update`;
`team get`, `create`, `update`, `retire`, `unretire`, `delete` and `unarchive`;
`member list`, `add` and `remove`; `user get`, `update`, `change-role`, `suspend`,
`unsuspend` and `revoke-sessions`, with `user list --all`. Every setting is one typed
flag, a boolean takes `true` or `false`, `none` clears, and a flag left out leaves its
setting alone. Linear's plan and role refusals are exit 6, and the reference spells
out every enumerated flag's values. Invites wait on a paid plan. The crate is
`muster-linear`: `cargo install muster-linear`.

### Added

- ADR 0014 decides the third slice: Linear's administrative operations as eighty-one
  `muster` commands in twenty groups, landed one group per pull request in three
  release blocks (0.3.0 the workspace and its people, 0.4.0 the things a project is
  made of, 0.5.0 the rest), with the fixtures of the empty kinds recorded from a smoke
  team and an invite to the house's own address
  ([ADR 0014](docs/decisions/0014-build-the-third-slice-administrative-commands.md)).
- `workspace get` prints the workspace as Linear returns it, one JSON line and no team
  needed: its `name` and `urlKey`, its settings, `securitySettings` and
  `authSettings`, `userCount`, `createdIssueCount`, `previousUrlKeys` and
  `subscription` (ADR 0014, group 1;
  [the tutorial](docs/tutorials/work-an-issue-in-ten-minutes.md)).
- `workspace update` changes the workspace's settings through Linear's
  `organizationUpdate` and prints the workspace as Linear returns it afterwards: the
  name, the URL key (the last three previous keys keep redirecting), the logo, the git
  branch format and linkback switches, the roadmap, feed, pull request and AI
  switches, the project update reminders, the fiscal year, the upload content types,
  the ten security roles and the sign-in services. A flag left out leaves its setting
  alone, a boolean takes `true` or `false`, `none` clears the branch format, `all`
  allows every sign-in service and is the one spelling of that empty list (a value
  that names no service is exit 2), and no flag at all is a usage error, exit 2,
  before any request; most settings need a workspace admin, and Linear's refusal is
  exit 6 (ADR 0014, group 1;
  [the tutorial](docs/tutorials/work-an-issue-in-ten-minutes.md)).
- `team get [<KEY>]` prints a team with every setting, read in one request by key, or
  the team muster would use; `team list` now prints `visibility`, `cyclesEnabled`,
  `retiredAt` and `timezone` with each team. `team create --name <TEXT> [--key <KEY>]
  [--description] [--timezone <TZ>] [--cycles-enabled <true|false>] [--parent <KEY>]
  [--copy-settings-from <KEY>]` makes a team through Linear's `teamCreate`, whose
  creator becomes a member, with Linear's default states and labels; a name a
  team already has is refused, exit 6, unless `--allow-duplicate`, and Linear keeps the
  key unique itself. `team retire` and `team unretire` set and clear `retiredAt`, so
  that a team keeps its issues and takes no new ones; `team delete` archives a team and
  schedules its data for deletion after Linear's grace period, printing Linear's delete
  payload, and `team unarchive <KEY>` cancels that within the period (ADR 0014,
  group 2; [the tutorial](docs/tutorials/work-an-issue-in-ten-minutes.md)).
- `team update [<KEY>]` changes a team's settings through Linear's `teamUpdate` and
  prints the team as Linear returns it afterwards: the name, key, description, icon,
  colour and timezone, `--private`, `--parent <KEY|none>`, the cycle settings
  (`--cycles-enabled`, `--cycle-enabled-start-date`, the start day, duration, cooldown,
  the auto-assign and lock switches, the upcoming count), the triage settings, the
  default issue state by name, the estimation settings, the auto-close and
  auto-archive settings (`--auto-close-period none` never closes an idle issue), the
  membership settings, initiatives, grouped history, and the five team roles. A flag
  left out leaves its setting alone, a boolean takes `true` or `false`, a state is
  resolved in the team before any request, and no flag at all is exit 2; most settings
  need a team owner or a workspace admin, and Linear's refusal is exit 6 (ADR 0014,
  group 2; [the tutorial](docs/tutorials/work-an-issue-in-ten-minutes.md)).
- `member list` prints the team's memberships, one per line, each with its `user`,
  `owner` and `sortOrder`, bounded by `--limit` with a `truncated` notice past it;
  `member add --user <WHO>` adds a user to the team through Linear's
  `teamMembershipCreate` (a second membership is Linear's refusal, exit 6), and
  `member remove --user <WHO>` removes one, the membership found in the team's first
  `PAGE_CAP` members first, so that a user who is not a member is exit 4 before any
  mutation, and a user who is not among them in a team with more is refused, exit 6,
  a muster guard, rather than called a non-member; the team is the usual rule,
  `--team`, `MUSTER_TEAM` or the only one (ADR 0014, group 3;
  [the tutorial](docs/tutorials/work-an-issue-in-ten-minutes.md)).
- `user get <WHO>` prints one user with their role flags (`admin`, `owner`, `guest`,
  `app`), `active` and `disableReason`, profile, status and `lastSeen`; `user list`
  now prints `owner`, `guest` and `app` too and leaves suspended users out unless
  `--all`. `user update <WHO>` changes a profile through Linear's `userUpdate` (your
  own, or anyone's as a workspace admin): `--name`, `--display-name`, `--title`,
  `--description`, `--timezone`, `--status-emoji`, `--status-label`, `--status-until
  <TIME|none>` and `--avatar-url`; a flag left out leaves its field alone. The admin's
  commands `user change-role <WHO> --role <owner|admin|user|guest>`, `user suspend`,
  `user unsuspend` and `user revoke-sessions` print Linear's `success`, then the user
  read back, two JSON lines; when the read-back fails after the mutation landed, the
  exit code is the read-back's (7 for a dropped query) and the error says the mutation
  landed and names `user get`, rather than "nothing changed in Linear". A suspended
  user is still found by email or name, by every command that names a user, so that
  `unsuspend` reaches them (ADR 0014, group 4;
  [the tutorial](docs/tutorials/work-an-issue-in-ten-minutes.md)).

### Changed

- `muster schema` and the reference it generates spell out the values an enumerated
  flag takes, `<owner|admin|user|guest>` and `<Sunday|Monday|...|Saturday>` rather than
  `<ROLE>` and `<DAY>`, for every flag whose values `clap` knows, so a flag added later
  cannot leave them out ([the reference](docs/reference/commands.md)).
- Linear's `FORBIDDEN` and `FEATURE_NOT_ACCESSIBLE` answers, to what the workspace's
  plan or the key's role does not allow ("Not allowed on free plan", "Not allowed to
  access feature 'guestAccounts'"), are a refusal, exit 6, with Linear's code and
  message and a hint naming the plan and the role; they were exit 1, muster's own
  failure (ADR 0014; [the reference](docs/reference/commands.md#exit-codes)).

## 0.2.0 - 2026-11-03

The second slice ([ADR 0013](docs/decisions/0013-build-the-second-slice-sub-issues-attachments-and-projects.md)),
for an agent that splits an issue, links its pull request and works in a project:
`--parent` on `issue create`, `issue update` and `issue list`, with `children` in
`issue get`; `issue attach`; `project create` and `issue list --project`. Two bounds
join the table, `CHILDREN_PER_ISSUE` and `TEAMS_PER_PROJECT`, and `BODY_MAX_BYTES` now
covers every text sent. The glossary is a gate, the CI and release timeouts are
measured, and the pages say in which order Linear returns comments. The crate is
`muster-linear`, as for 0.1.1: `cargo install muster-linear`.

### Added

- ADR 0013 decides the second slice: sub-issues, attachments and projects, in three
  features, in that order; cycles wait until a team of the workspace enables them
  ([ADR 0013](docs/decisions/0013-build-the-second-slice-sub-issues-attachments-and-projects.md)).
- `issue get` prints `children`, the issue's sub-issues newest first as Linear orders
  them, bounded by `CHILDREN_PER_ISSUE` with a `truncated` notice past it; `issue create --parent`,
  `issue update --parent` (or `--parent none`) and `issue list --parent` make, move and
  list sub-issues, a sub-issue goes to its parent's team unless `--team` or
  `MUSTER_TEAM` names one, and an issue named as its own parent is a usage error,
  exit 2, before any request (ADR 0013, feature 1;
  [the tutorial](docs/tutorials/work-an-issue-in-ten-minutes.md)).
- `issue attach <ISSUE> --url <URL> --title <TEXT> [--subtitle <TEXT>]` adds a link to an
  issue through Linear's `attachmentCreate` and prints the attachment as Linear returned
  it, with its `issue.identifier`; `issue get` lists it under `attachments.nodes`, newest
  first. A URL the issue already has is updated in place, same id, and a subtitle left
  out stays, as Linear does; an empty URL or title is a usage error, exit 2, before any
  request (ADR 0013, feature 2; [the tutorial](docs/tutorials/work-an-issue-in-ten-minutes.md)).
- `BODY_MAX_BYTES` now bounds a title, a link and a subtitle as it bounds a description
  and a comment: a larger one is refused, exit 6, before any request, and the message
  names which one was too long with a hint that fits it; only a body is told it can be
  split over several comments. Linux refuses one argv string over 128 KiB before
  muster runs, so from a flag's value the bound is reached only where the OS allows
  the argument, as macOS does; from a file or stdin it is reached everywhere.
- `just lint-docs` and `just docs` check the glossary: every command group and every
  flag noun the reference introduces has a row in `docs/glossary.md`, and a flag that
  is a switch or a mode is listed in the script with its reason (ADR 0006's Revisit;
  output fields are not checked, since `muster schema` does not list them, so their
  rows stay by hand).
  Ten rows join the glossary: `user`, `title`, `description`, `body`, `parent`, `name`,
  `url`, `subtitle`, `search` and `limit`.
- `project create --name <TEXT> [--description <TEXT>]` makes a project for the team
  through Linear's `projectCreate` and prints it as `project list` prints one; a name an
  existing project already has is refused, exit 6, unless `--allow-duplicate`, so that
  `--project <NAME>` keeps resolving to one project. `issue list --project <NAME>` lists
  the team's issues in a project and `--project none` those in no project; the guard is
  workspace-wide, as `--project <NAME>` resolves, and `--name` and `--description` are
  bounded by `BODY_MAX_BYTES` like every other text (ADR 0013, feature 3;
  [the tutorial](docs/tutorials/work-an-issue-in-ten-minutes.md)).
- `muster fixtures` records `IssueById` on an issue that has comments, attachments and
  sub-issues when the workspace has one, and writes a `partial` notice saying what the
  recording lacks; `skipped` and `partial` join the notices table of the reference.

### Fixed

- `project list --limit 250` was refused by Linear as "Query too complex": each
  project's `teams` connection multiplied the query's cost. `project list` and
  `project create` now print at most `TEAMS_PER_PROJECT` teams per project, and
  `project list` writes a `truncated` notice for a project in more (a project
  `project create` makes has the one team, so it is never cut), so that a page of
  `PAGE_CAP` projects stays inside the limit; the project lookup behind
  `--project <NAME>` asks for one project, not a page.
- The reference, the tutorial and the `linear-read` skill said `issue get` prints
  comments oldest first. Linear orders a connection newest first, `orderBy: createdAt`
  included, and muster prints what Linear returns; the pages now say so, and a cut thread
  is the newest `COMMENTS_PER_ISSUE` comments with the oldest left out. The output did
  not change.

## 0.1.1 - 2026-11-03

The first release on crates.io, as the crate `muster-linear`: `cargo install
muster-linear` installs the `muster` binary. The same five commands as 0.1.0.

### Changed

- The crate is `muster-linear`, not `muster-cli`: that name, like `muster`, belongs to
  another project, which the publish of 0.1.0 found out
  ([ADR 0012](docs/decisions/0012-publish-as-muster-linear.md)). The binary and the
  library are `muster` as before. The GitHub Release carries
  `muster-0.1.1-x86_64-unknown-linux-gnu.tar.gz` and
  `muster-0.1.1-aarch64-apple-darwin.tar.gz` with their SHA-256 sums.

## 0.1.0 - 2026-11-03

The first release: the five Trello operations as `muster` commands, for coding agents
first. On GitHub only: its notes said `cargo install muster-cli`, and that crate is
another project's, so the crates.io publish was refused; 0.1.1 is the one to install.

### Added

- A GitHub Release with `muster-0.1.0-x86_64-unknown-linux-gnu.tar.gz` and
  `muster-0.1.0-aarch64-apple-darwin.tar.gz` and their SHA-256 sums, built by
  `.github/workflows/release.yml` from the tag `v0.1.0`; the binary and the library
  are both named `muster`
  ([ADR 0011](docs/decisions/0011-release-as-muster-cli.md),
  [how to cut a release](docs/how-to/cut-a-release.md)).

- Repository scaffold ported from Cavalry: `CLAUDE.md` invariants and gates, the
  documentation rules, the ADR template, the glossary and the book.
- ADR 0001: decisions are recorded as ADRs. ADR 0002: muster is Rust on a blocking
  HTTP client, and why not Go or the TypeScript SDK. ADR 0003: one crate, one version,
  no publishing yet, the lints and the dependency allowlist.
- ADR 0007: the first slice is the five Trello operations, as five features. ADR 0008:
  the agent contract, JSON Lines, the exit codes, no prompt, the key from a file.
  ADR 0010: the tests run against a fake Linear held to recorded shapes.
- Cargo workspace with the `muster` crate, `just lint` and `just test` passing for
  real: `unsafe` forbidden, `unwrap`, `expect` and `panic` denied, printing confined to
  the output module, `stdin`, `exit` and `sleep` disallowed, and
  `scripts/lint-workspace.py` checking the dependency allowlist.
- The agent loop ported from Cavalry (ADR 0004, ADR 0005): `ci.yml` runs the four
  gates and `just quickstart` on Linux and macOS Blacksmith runners; `claude.yml`,
  `dispatch-review.yml`, `review-pull-request.yml` and `auto-merge.yml` run the writer,
  the reviewer and the squash merge; Dependabot moves the pinned actions and the crates
  weekly. The briefs are `.claude/agents/developer.md` and `reviewer.md`;
  `docs/how-to/ask-the-agent.md` is the page for a person.
- The documentation gate (ADR 0006): `just docs` builds rustdoc and the book with
  warnings denied and checks every link; `just lint-docs` fails a public item or a
  command changed without its docs and an edit to an accepted ADR.
- The weekly release-notes email (ADR 0009): `send-release-notes.yml`, the scripts
  under `scripts/release-notes*`, the writer and reviewer brief, and
  `docs/how-to/send-the-release-notes.md`.
- `muster whoami`, `team list`, `state list`, `label list`, `project list` and
  `user list`; the key from `LINEAR_API_KEY`, `MUSTER_KEY_FILE` or
  `~/.config/muster/key`; `MUSTER_TEAM` and `MUSTER_ENDPOINT`; `muster schema` as JSON
  or as the generated `docs/reference/commands.md`, which `just lint-commands` holds to
  the binary (feature 1 of ADR 0007).
- `muster issue get` by identifier, number or id, with comments and attachments, and
  `muster issue list` with `--state`, `--all`, `--label`, `--assignee`, `--search` and
  `--limit`; every bound a constant in `bounds.rs`, a cut list a `truncated` notice
  (feature 2 of ADR 0007).
- `muster issue create` with labels, state, priority, project, assignee and a
  description from a file or stdin, refusing a duplicate open title unless
  `--allow-duplicate`; `muster issue comment` from a flag, a file or stdin
  (feature 3 of ADR 0007).
- `muster issue close` to the team's completed state, `--cancel` or `--state`, after an
  optional comment; `muster issue update` for title, state, labels, priority, assignee,
  project and description (feature 4 of ADR 0007).
- The fake Linear under `crates/muster/tests/support/`, held to the shapes recorded under
  `crates/muster/tests/fixtures/` by `just record-fixtures` (ADR 0010); 25 tests through
  the binary with stdin closed; `just quickstart` against the fake and `just smoke`
  against a real workspace.
- Claude Code skills under `.claude/skills/`: `/commit`, `/pr` and `/next-task` ported
  from Cavalry, and `/linear-read`, `/linear-create`, `/linear-done` and `/linear-start`
  calling `muster` in place of the Trello skills they replace (feature 5 of ADR 0007).
- The tutorial `docs/tutorials/work-an-issue-in-ten-minutes.md`, the explanation
  `docs/explanation/why-muster.md`, and `docs/how-to/implement-a-feature.md`.
