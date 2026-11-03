# 0013. Build the second slice: sub-issues, attachments and projects, in three features

- **Status:** Accepted
- **Date:** 2026-11-03
- **Invariants affected:** 1, 3, 5, 6
- **Supersedes:** none

## Context

The first slice is done. ADR 0007 said it is done when the README's ten-minute path
runs in CI against the fake on Linux and macOS, `just smoke` passes against the
`appshapes` workspace, and the five Trello operations have a `muster` command each: CI
run 41561308933 on 2026-11-03 ran `just quickstart` green on both runners, `just smoke`
created and canceled APP-5 on 2026-11-03, and `docs/reference/commands.md` lists
`issue get`, `issue list`, `issue create`, `issue comment`, `issue close` and
`issue update`. ADR 0007's Revisit clause says the next slice's ADR is due when the
slice is done, and APP-6 names the candidates: cycles, sub-issues, attachments and
`muster issue list --project`.

What an agent needs next is read from this repository's own process, not from Linear's
feature list. `docs/how-to/implement-a-feature.md` (step 3.7) splits a feature into
several pull requests, each small enough for one review, and each pull request is an
issue with its own identifier; today those issues have no link to the feature's, and
APP-7, this ADR's own issue, became a sub-issue of APP-6 through a raw `issueUpdate`
mutation on 2026-11-03 because no `muster` command could do it. The `pr` skill's landing
comment cites the pull request number as text; Linear's place for a link is the
attachment, which `issue get` already prints under `attachments.nodes` and nothing in
`muster` can write. APP-6's first decision, the layout of the house's own work in
Linear, is between a team per product and one team with a project per product; the
second layout needs `issue list --project` and a project made without a browser, and
`muster` today can list projects and assign one, not filter by one or create one.

Linear's schema, introspected with the key on 2026-11-03, has every piece:
`IssueCreateInput` and `IssueUpdateInput` take `parentId`, `projectId` and `cycleId`;
`IssueFilter` has `parent`, `children`, `project` and `cycle`; `Issue` has `parent` and
`children: IssueConnection`; `AttachmentCreateInput` needs `title`, `url` and `issueId`;
`ProjectCreateInput` needs `name` and `teamIds`; `IssueRelationCreateInput` takes `type`
(`blocks`, `duplicate`, `related`, `similar`), `issueId` and `relatedIssueId`. The
workspace's one team has `cyclesEnabled: false`, so `cycles` answers no node: there is no
cycle to record a shape from, and ADR 0010 holds the fake to recorded shapes. Enabling
cycles changes the team for everyone who uses it; that is the owner's setting, not a
tool's.

## Decision

The second slice is three features, in this order, each a Linear issue of the `APP` team
and each landed as `docs/how-to/implement-a-feature.md` says, with its fixture, its fake
answer, its tests against the fake, its `just smoke` output in its commit and its lines
in the tutorial and the reference:

1. **Sub-issues.** `issue get` prints `children.nodes` (each child's `id`, `identifier`,
   `title` and `state`) beside the `parent` it already prints, bounded by a new constant
   `CHILDREN_PER_ISSUE` in `bounds.rs` with a `truncated` notice past it.
   `issue create --parent <ISSUE>` makes a sub-issue; the parent is resolved as
   `issue get` resolves an issue, and the team is `--team`, else `MUSTER_TEAM`, else
   the parent's team. `issue update --parent <ISSUE>` moves an issue under a parent and
   `--parent none` takes it out. `issue list --parent <ISSUE>` lists an issue's
   children, with the team and state filters `issue list` always applies. Operations:
   `IssueById` gains `children`; `IssueCreate` and `IssueUpdate` send `parentId`;
   `Issues` filters on `parent`. The fixture recorder prefers an issue whose
   `children`, `comments` and `attachments` are not empty, so that the node shapes are
   recorded, and says in a notice when the workspace has none.
2. **Attachments.** `issue attach <ISSUE> --url <URL> --title <TEXT>
   [--subtitle <TEXT>]` adds a link to an issue through `attachmentCreate` and prints
   the attachment as Linear returned it, with its `issue.identifier`. The title is
   required because Linear requires it and muster invents no value (invariant 1).
   Linear's own behaviour on a URL the issue already carries is what the output shows;
   muster adds no guard. Operation: `AttachmentCreate`, a mutation with the exit 8 test
   of invariant 1. The `IssueById` fixture is re-recorded on an issue that has an
   attachment.
3. **Projects.** `issue list --project <NAME>` lists the team's issues in a project, by
   name, and `--project none` those in no project. `project create --name <TEXT>
   [--description <TEXT>]` makes a project for the team (`--team`, else `MUSTER_TEAM`,
   else the workspace's only team) through `projectCreate` and prints it as
   `project list` prints one; a name an existing project already has is refused, exit
   6, unless `--allow-duplicate`, for the reason `issue create` refuses a duplicate
   title: `--project <NAME>` resolves a name to one project, and an agent that retries
   after a timeout makes one project, not two. Operations: `Issues` filters on
   `project`; `ProjectCreate`. The `Projects` fixture is re-recorded once the workspace
   has a project.

The slice is done when the three features' commands are in
`docs/reference/commands.md`, each has its tests against the fake and its `just smoke`
output in a commit on `master`, and release 0.2.0 carries them (a new command is a
minor bump, `docs/how-to/cut-a-release.md`).

Out of the slice, each with its trigger: cycles, until a team of the workspace has
`cyclesEnabled` true, when `cycle list` and `--cycle` on `issue create`, `issue update`
and `issue list` open the next slice; relations (`blocks`, `related`, `duplicate`),
until a skill in this repository needs one; file uploads as attachments, which are a
signed-URL flow of their own; project milestones, updates and initiatives; sub-issue
ordering; and anything Linear's web application does that an agent working an issue
does not need.

## Alternatives considered

- **Cycles in this slice, with `muster` enabling them on the team.** Lost:
  `cyclesEnabled` is a team setting that changes what every member sees; a tool that
  flips it to test itself is not a tool the owner can trust with a key. Without a cycle
  in the workspace the fake would answer a shape nobody recorded, which is a lie about
  Linear by the route ADR 0010 closes.
- **`attachmentLinkURL`, which lets Linear fetch the title of the page.** Lost: the
  title then depends on a page muster never sees and on Linear's fetcher, so two runs
  can print two titles; an agent that links a pull request knows what to call it.
  `attachmentLinkGitHubPR` lost for the same reason and one more: it ties the command
  to one host, and Linear's GitHub integration makes that attachment by itself where it
  is set up.
- **A raw `muster graphql` command.** Lost: an unbounded surface the fake cannot
  answer, the reference cannot describe and the glossary cannot cover; every field it
  prints would be Linear's and nothing would say what they mean. An agent that needs an
  operation muster lacks writes it into the next slice.
- **Relations in this slice.** Lost: no skill in this repository orders issues by a
  relation; `/next-task` reads order from the slice ADR. The schema is recorded above
  so that the day a skill needs `blocks` the feature has its evidence.
- **`issue children <ISSUE>` as a command of its own.** Lost: `issue list` already
  carries the team filter, the state filters and the bound; one flag is less surface
  than one command.
- **`project create` without the duplicate guard.** Lost: Linear allows two projects
  with one name, and `--project <NAME>` on `issue create` and `issue update` resolves a
  name to the first match, so a second project with the name makes the first
  unreachable by name.
- **Make `--parent` imply the parent's team over `MUSTER_TEAM`.** Lost: an environment
  variable that sometimes loses to a flag's side effect is one a reader cannot predict;
  the resolution order of ADR 0008 stands, and the parent's team is the last resort,
  where today there is none.

## Evidence

- Linear's GraphQL schema, introspected with the house's key on 2026-11-03 (`__type` on
  `IssueFilter`, `IssueCreateInput`, `IssueUpdateInput`, `AttachmentCreateInput`,
  `ProjectCreateInput`, `IssueRelationCreateInput`, `Issue`, `Cycle`, `Attachment`,
  `Project` and `Mutation`): the fields named in Context, and among the mutations
  `attachmentCreate(input)`, `attachmentLinkURL(url, issueId, title)`,
  `attachmentLinkGitHubPR`, `projectCreate(input)` and `issueRelationCreate(input)`.
- The `appshapes` workspace on 2026-11-03, read with the key: one team, `APP`,
  `cyclesEnabled: false`, seven issues, no project, no cycle.
- APP-7 is a sub-issue of APP-6 since 2026-11-03, set with
  `issueUpdate(id, input: { parentId })` outside muster; the `IssueById` fixture of
  feature 1 is recorded on that pair.
- The first slice's done-when: CI run 41561308933 (`gates (linux)` and `gates (macos)`,
  2026-11-03, 1 min 44 s); APP-5 created and canceled by `just smoke` on 2026-11-03; the
  five commands in `docs/reference/commands.md` on `master` at 65f257d.
- `docs/how-to/implement-a-feature.md`, step 3.7, and `.claude/skills/pr/SKILL.md`,
  section 2, "Merged": the two places in the process that need a sub-issue and an
  attachment.

## Consequences

Easier: an agent splits an issue into sub-issues and reads them back with the parent; a
pull request is a link on its issue, where Linear shows it; the house's layout decision
has both options open, with `issue list --project` and `project create` on the one side
and `--team` on the other.

Harder: `issue get` carries one more connection and `bounds.rs` one more constant; the
fake models parents, children, attachments and projects; the `Projects` and `IssueById`
fixtures need a workspace with a project, a sub-issue and an attachment at recording
time, which the `just smoke` run of each feature provides.

Revisit: when the slice is done (the next slice's ADR, cycles first if a team has
`cyclesEnabled` true by then); when a skill needs a relation; when a Trello skill needs
an operation the slice lacks.

## References

- ADR 0007 (the first slice and its Revisit), ADR 0008 (the output contract and the
  resolution order), ADR 0010 (the fixtures and the fake)
- APP-6 and APP-7 in the `appshapes` workspace
- `docs/how-to/implement-a-feature.md`, `docs/how-to/cut-a-release.md`,
  `.claude/skills/pr/SKILL.md`
- Linear's API: <https://linear.app/developers/graphql>,
  <https://linear.app/developers/attachments>
