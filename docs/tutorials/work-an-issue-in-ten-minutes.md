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
cargo install muster-linear
```

The crate is `muster-linear` and the binary is `muster`
([ADR 0012](../decisions/0012-publish-as-muster-linear.md)); from a checkout,
`cargo install --path crates/muster` builds the same binary. You will see
`Installed package muster-linear v0.4.0` and `muster --version` prints `muster 0.4.0`.

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

`muster workspace get` prints the whole workspace on one line: its `name` and `urlKey`,
its settings (the git branch format, the reminder days and hours, the feature
switches), `securitySettings` and `authSettings` as Linear returns them, `userCount`,
`createdIssueCount`, `previousUrlKeys` and `subscription` (`null` on the Free plan). It
needs no team ([ADR 0014](../decisions/0014-build-the-third-slice-administrative-commands.md)).
`muster workspace update --project-update-reminders-day Monday` changes one setting and
prints the workspace again; a flag left out leaves its setting alone, a boolean setting
takes `true` or `false`, and `muster workspace update --help` lists every setting. Most
of them need a workspace admin: Linear's refusal is exit 6, `"code":"refused"`, with
Linear's message.

The team itself is `muster team get`: one JSON line with every setting of the team
muster would use (`muster team get SMK` for another), from `cyclesEnabled` and the
cycle settings to the `defaultIssueState`, the `timezone`, the `visibility` and the
`issueCount`. `muster team create --name "Trying muster" --key TRY` makes a team:
you become its member and Linear makes its default states and labels; a name
a team already has is exit 6 unless `--allow-duplicate`, and Linear keeps the key
unique itself. The team's lifecycle is four commands: `muster team retire TRY` sets
`retiredAt`, so the team keeps its issues and takes no new ones, `muster team unretire
TRY` clears it, `muster team delete TRY` archives the team and schedules its data for
deletion after Linear's grace period (one JSON line, Linear's `success` and
`entityId`), and `muster team unarchive TRY` cancels that within the period
([ADR 0014](../decisions/0014-build-the-third-slice-administrative-commands.md)).
`muster team update TRY` changes the team's settings. Every setting that
`muster team update --help` lists is one flag, and `team get` shows its value:
`--cycles-enabled true --cycle-enabled-start-date 2026-11-10T00:00:00Z` turns
cycles on from a date, `--default-issue-state Todo` names a state of the team, and
a flag left out leaves its setting alone.
Who is in the team is `muster member list`: one JSON line per membership, with its
`user` and whether they are an `owner`; `muster member add --user <email>` and
`muster member remove --user <email>` change it, for the team of `--team`,
`MUSTER_TEAM` or the only one, and a user who is not a member is exit 4.
A person is `muster user get me` (or an email address, or a name): one JSON line with
their role flags (`admin`, `owner`, `guest`), whether they are `active`, their profile
and status, and `lastSeen`. `muster user update me --status-label "Heads down"` changes
your own profile, and a workspace admin changes anyone's; `muster user change-role`,
`muster user suspend`, `muster user unsuspend` and `muster user revoke-sessions` are
the admin's, and each prints Linear's `success` and then the user read back.

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

A project is `muster project create --name "Trying muster"`: one JSON line, the project
with its `name`, `url`, `status` and `teams.nodes`; a name a project of the workspace
already has is exit 6 unless `--allow-duplicate`, so that `--project "Trying muster"` on
`issue create` and `issue update` keeps naming one project, and
`muster issue list --project "Trying muster"` lists the team's open issues in it
(`--project none` those in no project). `just quickstart` runs both against the fake.
`muster project get "Trying muster"` prints the project whole: its `content`,
`status`, `lead`, dates, `health` and `progress`, and its `members` and
`projectMilestones`, each bounded (`MEMBERS_PER_PROJECT`, `MILESTONES_PER_PROJECT`)
with a `truncated` notice past the bound. `muster project update "Trying muster"
--status "In Progress" --lead me --target-date 2026-12-01` changes it, one flag per
field, `none` clearing a lead or a date; `muster project delete "Trying muster"`
trashes it for thirty days and `muster project unarchive "Trying muster"` brings it
back ([ADR 0014](../decisions/0014-build-the-third-slice-administrative-commands.md)).
The statuses a project moves through are the workspace's: `muster project-status
list` shows them by position, one order across the workspace (the five defaults sit
at 0 to 4, `Backlog` to `Canceled`), `muster project-status create --name "On hold"
--type paused --color "#f2c94c" --position 2.5` adds one, which lists between `In
Progress` at 2 and `Completed` at 3, `muster project-status update "On hold" --name
"Paused"` changes it, and `muster project-status archive Paused` takes it out once
no project is in it (`unarchive` brings it back).
Inside a project, `muster milestone create --project "Trying muster" --name "Alpha"
--target-date 2026-12-01` makes a milestone (one JSON line with its `name`,
`targetDate`, `status` and `progress`), `muster milestone list --project "Trying
muster"` shows them, and an issue joins one with `--milestone Alpha` on `issue
create` (with `--project`) or `issue update`, and leaves it with `--milestone none`;
`muster issue list --milestone Alpha` lists the issues in it. How the project is
doing is a status post: `muster project-update create --project "Trying muster"
--body "Shipping on Friday." --health onTrack` (one JSON line with the post's `body`,
`health`, `user` and `url`), and `muster project-update list --project "Trying
muster"` shows them newest first; the project's own `health` and `lastUpdate` follow
the latest post. Above projects sit initiatives, which a workspace switches on once
with `muster workspace update --roadmap true` (Linear refuses `initiative create`,
exit 6, while they are off): `muster initiative create --name "Growth" --owner me
--status Active` makes one (one JSON line with its `status`, `owner`, `leadTeam` and
`targetDate`; `--lead-team <KEY>` names the team that leads it, which Linear sells on
its Business plan and refuses below it), `muster initiative add-project Growth
--project "Trying muster"` puts the project in it, `muster initiative get Growth`
prints it with its `projects` (bounded by `PROJECTS_PER_INITIATIVE`), and `muster
initiative archive Growth` puts it away (`unarchive` brings it back).
Several issues change at once with `muster issue batch-update --issue APP-5 --issue
APP-6 --state Todo --priority 3` (one JSON line per issue, as Linear returns them;
up to `ISSUES_PER_BATCH` issues of one team per run). `muster issue archive APP-5`
takes an issue out of the lists and `muster issue unarchive APP-5` brings it back;
`muster issue delete APP-5` puts it in Linear's trash, with its sub-issues, for
thirty days, and `unarchive` brings that back too; muster never deletes an issue for
good. An archived or trashed issue is reached by its identifier, its number alone
(`muster issue unarchive 5`) or its id, though `issue list` leaves it out.
A team that uses cycles (`team update --cycles-enabled true`) has
them made by Linear: `muster cycle list` prints them newest first with their `number`
and the `isActive` flag, `muster cycle list --active` the one running now, `muster
cycle get 2` one with its `issues` (bounded by `ISSUES_PER_CYCLE`), and `muster cycle
update 2 --name "Sprint 2"` names it; `--cycle active` on `issue create` or `issue
update` puts an issue in the running cycle (`next` the one after it, a number any
of them, and `none` on `issue update` takes it out; `issue create` does not take
`none`, since a new issue starts in no cycle), and `muster issue list --cycle
active` lists the cycle's issues (`--cycle none` the issues in no cycle).
A document is a page of markdown kept with a project or an issue:
`muster document create --title "Kickoff" --project "Trying muster" --content "#
Kickoff"` makes one (one JSON line with its `title`, `url`, `project` and `content`;
a document with neither a project nor an issue is Linear's refusal, exit 6), `muster
document list --project "Trying muster"` lists the project's without their content,
`muster document get Kickoff` prints one with it, `muster document update Kickoff
--content-file kickoff.md` replaces the text from a file, and `muster document delete
Kickoff` puts it in the trash (`unarchive` brings it back).
A template is a saved shape for a new issue, project or document:
`muster template list` prints the team's and the workspace's, `muster template get
"Bug report"` one with its `templateData` (a string of JSON, as Linear returns it;
`jq '.templateData | fromjson'` reads it), and `muster template create
--name "Feature" --type issue --data-file feature.json` makes one from a file holding
that JSON (`--workspace` for one every team shares); `muster template update Feature
--data-file feature.json` replaces the data, and `muster template delete Feature`
deletes it, with no undo.
A webhook is a URL Linear posts changes to: `muster webhook create --url
https://example.com/hooks/issues --resource-type Issue --resource-type Comment --label
"Issues" --secret-file secret.txt` makes one for the team (one JSON line with its
`url`, `enabled`, `resourceTypes` and `team`, never its secret), `muster webhook
list` prints them, `muster webhook update Issues --enabled false` stops it without
deleting it, and `muster webhook delete Issues` deletes it, with no undo; a webhook
is named by its label, else its URL, else its id.
The audit log says what was done, by whom and from where: `muster audit list --limit
5` prints the five newest entries (one JSON line each, with `type`, `createdAt`,
`actor`, `ip`, `countryCode` and Linear's `metadata`), `muster audit list --type
projectDeleted` the deletions, `muster audit list --actor me --since
2026-11-01T00:00:00Z` your own since a time. Linear reserves the log for owners on
its Enterprise plan; where it refuses, the exit code is 6 and the message is Linear's.
A comment is changed or removed by the id `issue get` prints under `comments` (and
`issue comment` prints for a new one): `muster comment update <ID> --body "Second
thoughts."` replaces its body (one JSON line, with `editedAt` set), `--body-file
notes.md` takes the body from a file, and `muster comment delete <ID>` deletes it,
with no undo.

A label of the team is `muster label create --name "Smoke" --color "#26b5ce"`
(`--workspace` makes one every team shares, `--group` one that holds other labels):
one JSON line, the label with its `name`, `color`, `team` and `retiredAt`.
`muster label update Smoke --name "Smoky"` changes it, `muster label retire Smoky`
takes it out of use (it stays on its issues and goes on no new one, and
`muster label restore Smoky` brings it back), and `muster label delete Smoky` removes
it from every issue, which Linear cannot undo
([ADR 0014](../decisions/0014-build-the-third-slice-administrative-commands.md)).
The team's workflow is `muster state list`, by position; `muster state create --name
"Blocked" --type started --color "#eb5757"` adds a state (one JSON line with its
`name`, `type`, `position` and `team`; the type never changes afterwards),
`muster state update Blocked --name "Waiting"` renames it, and `muster state archive
Waiting` takes it out once no issue is in it, with nothing to bring it back.

A sub-issue is the same command with `--parent APP-12`: the new issue prints
`parent.identifier` as `APP-12`, and it is created in the parent's team unless `--team`
or `MUSTER_TEAM` names one. `muster issue update APP-13 --parent none` takes it out
from under its parent again, and `--parent APP-12` on `issue update` moves an issue
under one ([ADR 0013](../decisions/0013-build-the-second-slice-sub-issues-attachments-and-projects.md)).

## 5. Comment on it (one minute)

```sh
muster issue comment APP-12 --body "Read it back with issue get."
```

One JSON line: the comment, with its `body`, `createdAt`, `user` and `issue.identifier`.
Exit 0. A long comment goes through a file: `--body-file notes.md`, or `--body-file -`
to read it from stdin.

A link is `muster issue attach APP-12 --url https://github.com/appshapes/muster/pull/3
--title "Pull request 3"`: one JSON line, the attachment with its `url`, `title`,
`subtitle` and `issue.identifier`, which step 6 prints under `attachments.nodes`. The
title is required; Linear requires one and muster invents none. Run it again with the
same URL and a new title and you get the same `id` back with the new title: Linear
updates a link the issue already has rather than adding a second
([ADR 0013](../decisions/0013-build-the-second-slice-sub-issues-attachments-and-projects.md)).

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

Without `jq` you get the whole issue: every field of step 4, plus `comments.nodes`,
`attachments.nodes`, and `children.nodes`, its sub-issues (each with its `identifier`,
`title` and `state`). Linear orders each of them newest first; past `COMMENTS_PER_ISSUE`
comments or `CHILDREN_PER_ISSUE` sub-issues a `truncated` notice goes to stderr, the
oldest are the ones left out, and `muster issue list --parent APP-12 --all` lists the
team's sub-issues of it, open and closed, up to `PAGE_CAP`. The number alone works inside the team: `muster issue get 12`. An issue that
does not exist is exit 4, `"code":"not_found"`.

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
