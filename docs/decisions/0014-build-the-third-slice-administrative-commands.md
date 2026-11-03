# 0014. Build the third slice: administrative commands, in twenty groups

- **Status:** Accepted
- **Date:** 2026-11-03
- **Invariants affected:** 1, 2, 3, 5, 6
- **Supersedes:** none

## Context

The second slice is done. ADR 0013 said it is done when its three features' commands
are in `docs/reference/commands.md`, each with its tests against the fake and its
`just smoke` output in a commit on `master`, and release 0.2.0 carries them:
`docs/reference/commands.md` lists `issue attach`, `project create`, `--parent` and
`--project` on `master` at b335e1d, and 0.2.0 was tagged and published on 2026-11-03
(release run 41574432801, `muster-linear` 0.2.0 on crates.io). ADR 0013's Revisit clause
says the next slice's ADR is due then, cycles first if a team has `cyclesEnabled` by
then; none has. On 2026-11-03 the owner asked for as many of Linear's administrative
operations as possible: the account, the workspace, projects and the rest.

What muster cannot do today is everything that lives under Linear's Settings. A team
cannot be made, so APP-6's layout decision (a team per product, or one team with a
project per product) has one side it cannot try without the browser; cycles cannot be
enabled, so ADR 0013's cycle trigger can only fire by hand; a label, a workflow state, a
milestone or a template cannot be made or changed; a member cannot be added, invited,
promoted or suspended; a project cannot be led, dated, trashed or restored; a webhook
cannot be registered; the audit log cannot be read. An agent that sets up a workspace
for a new product does all of that in a browser, and the house has no browser in its
agents' path.

Linear's schema, introspected on 2026-11-03 with the house's key by a read-only
research session (every `Query` and `Mutation` field and 660 types; Evidence names the
ones read), has every piece and marks what it does not support. The facts that
shape the set: roles change through one mutation,
`userChangeRole(id, role: owner | admin | user | guest | app)`, and there is no promote
or demote mutation; the schema exposes no API-key query or mutation at all; labels have
`issueLabelRetire` and `issueLabelRestore` beside an irreversible `issueLabelDelete`;
`cycleCreate`, `projectArchive`, `projectUpdateDelete`, `organizationStartTrial` and
the direct `allowedAuthServices` are deprecated in favour of automatic cycles,
`projectDelete` with `projectUnarchive`, `projectUpdateArchive`,
`organizationStartTrialForPlan` and `authSettings`; the membership `owner` flag, a
document's initiative and team links, a team's inheritance settings and its sub-team
handling are marked Internal; `issueBatchUpdate` takes at most fifty ids;
`issueDelete`'s `permanentlyDelete` skips the thirty-day trash and is admin-only;
`organizationUpdate` says most of its fields need the workspace-settings admin
permission; `teamUpdate` needs a team owner or a workspace admin for most changes;
`workflowStateArchive` takes only a state whose issues are all archived and has no
inverse; `organizationInviteCreate` sends an email. Linear's pages add the roles
(owner, admin, member, guest; every member is an admin on the Free plan), the limits
(2,500 requests an hour for a personal key, a complexity of 10,000 a query), and that a
deleted label cannot be recovered even through Linear's support.

The `appshapes` workspace on 2026-11-03, read with the key: one team, cycles off; two
users, one of them an app; three workspace labels, no team label and no group; seven
workflow states; one project and five project statuses; twenty-three audit entries,
readable by this admin key although Linear's pages reserve the audit log for owners on
the Enterprise plan; and nothing else: no invite, milestone, project update, initiative,
cycle, template, webhook or document. ADR 0010 holds the fake to recorded shapes, and a
connection with no node records no node shape, so eight of the kinds this slice reads
have nothing to record until something makes one. The workspace is on the Free plan
(the owner, 2026-11-03): two teams, 250 issues, unlimited members.

The loop's cost per pull request is fixed: a review about ten minutes after a push, a
fix run up to thirty minutes, CI on two runners, the landing comment and the close
(`.claude/skills/pr/SKILL.md`). `docs/how-to/implement-a-feature.md`, step 3.7, said
one command per pull request; at eighty-one commands that is eighty-one of those.

## Decision

The third slice is eighty-one commands in twenty groups, listed below with the
operation each one sends, each group a sub-issue of APP-31 landed as
`docs/how-to/implement-a-feature.md` says: its fixture, its fake answer, its tests
against the fake, its `just smoke` output in its commit, and its lines in the tutorial,
the glossary and the reference. The groups land in three blocks, in this order, and
each block is a release: 0.3.0 for the workspace and its people, 0.4.0 for the things a
project is made of, 0.5.0 for the rest.

**Block 1, release 0.3.0.**

1. `workspace get` (`organization`): the settings `workspace update` can change, the
   security and auth settings as Linear returns them (`securitySettings`,
   `authSettings`), `userCount`, `createdIssueCount`, `previousUrlKeys`, `trialEndsAt`,
   `deletionRequestedAt` and `subscription`. `workspace update` (`organizationUpdate`):
   `--name`, `--url-key`, `--logo-url`, `--git-branch-format <TEXT|none>`,
   `--git-linkback-messages`, `--git-public-linkback-messages`,
   `--git-linkback-descriptions`, `--suggested-branch-name`, `--roadmap`,
   `--project-update-reminder-weeks <N>`, `--project-update-reminders-day <Day>`,
   `--project-update-reminders-hour <0-23>`, `--fiscal-year-start-month <0-11>`,
   `--feed`, `--pull-request-tour`, `--pull-request-issue-mode <off|suggest|autoOnMerge>`,
   `--ai-thread-summaries`, `--ai-discussion-summaries`, `--oauth-app-review`,
   `--reduced-personal-information`, `--allowed-file-upload-content-types <TYPE,...>`,
   the ten `securitySettings` roles as `--invitations-role`, `--team-creation-role`,
   `--label-management-role`, `--personal-api-keys-role`, `--admin-management-role`,
   `--template-management-role`, `--integration-creation-role`, `--import-role`,
   `--api-settings-role` and `--workspace-initiatives-role`, each
   `<owner|admin|user|guest>`, and the `authSettings` as
   `--allowed-auth-services <SERVICE,...|all>` and `--hide-non-primary-organizations`.
   Not exposed: IP restrictions and HIPAA (a wrong value locks the owner out or changes
   a compliance posture), the home view, theme settings (Alpha), Slack, customers, code
   intelligence, agents and loops (Internal), and every deprecated field.
2. `team get [<KEY>]` (`team`): every setting `team update` changes, with `issueCount`,
   `visibility`, `retiredAt`, `parent`, `defaultIssueState` and `triageIssueState` by id
   and name. `team create` (`teamCreate`): `--name`, `--key`, `--description`,
   `--private`, `--timezone`, `--cycles-enabled`, `--parent <KEY>` and
   `--copy-settings-from <KEY>`; a name a team already has is refused, exit 6, unless
   `--allow-duplicate`, since `--team` and `MUSTER_TEAM` resolve a key and a reader
   resolves a name. `team update [<KEY>]` (`teamUpdate`): `--name`, `--key`,
   `--description`, `--icon`, `--color`, `--private`, `--timezone`,
   `--parent <KEY|none>`; the cycle settings `--cycles-enabled`,
   `--cycle-start-day <0-6>`, `--cycle-duration <WEEKS>`, `--cycle-cooldown <WEEKS>`,
   `--cycle-auto-assign-started`, `--cycle-auto-assign-completed`,
   `--cycle-lock-to-active`, `--cycle-enabled-start-date <TIME>` and
   `--upcoming-cycle-count <N>`; the triage settings `--triage-enabled` and
   `--require-priority-to-leave-triage`; `--default-issue-state <NAME>`; the
   estimation settings
   `--issue-estimation-type <notUsed|exponential|fibonacci|linear|tShirt>`,
   `--issue-estimation-allow-zero`, `--issue-estimation-extended` and
   `--default-issue-estimate <N>`; the closing settings
   `--auto-close-period <MONTHS|none>`, `--auto-close-state <NAME>`,
   `--auto-close-parent-issues`, `--auto-close-child-issues` and
   `--auto-archive-period <MONTHS>`; the membership settings `--join-by-default` and
   `--all-members-can-join`; `--initiatives-enabled`, `--issue-sharing-enabled`,
   `--group-issue-history`; and the five `securitySettings` roles
   `--team-management-role`, `--member-management-role`, `--label-management-role`,
   `--template-management-role` and `--issue-sharing-role`, each `<owner|member>`. Not
   exposed: Slack and AI settings, default templates, SCIM, hierarchy inheritance and
   every Internal or deprecated field. `team retire [<KEY>]` and
   `team unretire [<KEY>]` (`teamUpdate` with `retiredAt` set to now, or cleared): a
   retired team accepts no new issues. `team delete [<KEY>]` (`teamDelete`: archived
   and scheduled for deletion) and `team unarchive [<KEY>]` (`teamUnarchive`: cancels
   the deletion within the grace period). The `Teams` operation gains `cyclesEnabled`,
   `retiredAt` and `timezone`, so that `team list` shows what a reader needs to pick a
   team.
3. `member list` (`team.memberships`, bounded by `--limit`): the team's memberships,
   each with `user`, `owner` and `sortOrder`. `member add --user <WHO>`
   (`teamMembershipCreate`) and `member remove --user <WHO>` (`teamMembershipDelete`,
   the membership resolved from the team's list first). The `owner` flag is Internal in
   both inputs and stays out.
4. `user get <WHO>` (`user`): `admin`, `owner`, `guest`, `app`, `active`,
   `disableReason`, `title`, `description`, `timezone`, `statusEmoji`, `statusLabel`,
   `statusUntilAt`, `lastSeen`, `createdAt`, `url` and `isMe` beside the fields
   `user list` prints. `user list --all` passes `includeDisabled: true`.
   `user update <WHO>` (`userUpdate`): `--name`, `--display-name`, `--title`,
   `--description`, `--timezone`, `--status-emoji`, `--status-label`,
   `--status-until <TIME|none>` and `--avatar-url`.
   `user change-role <WHO> --role <owner|admin|user|guest>` (`userChangeRole`; `app`
   is not a role a person takes). `user suspend <WHO>` and `user unsuspend <WHO>`
   (`userSuspend`, `userUnsuspend`). `user revoke-sessions <WHO>`
   (`userRevokeAllSessions`).
5. `invite list` (`organizationInvites`): `email`, `role`, `external`, `acceptedAt`,
   `expiresAt`, `createdAt`, `inviter` and `invitee`.
   `invite create --email <ADDRESS> [--role <admin|user|guest>] [--team <KEY>...]`
   (`organizationInviteCreate`, which sends the email; a pending invite for the address
   is refused, exit 6, unless `--allow-duplicate`). `invite update <EMAIL> --team
   <KEY>...` (`organizationInviteUpdate`, which replaces the teams).
   `invite resend <EMAIL>` (`resendOrganizationInviteByEmail`). `invite delete <EMAIL>`
   (`organizationInviteDelete`; Linear's pages call it revoking).

**Block 2, release 0.4.0.**

6. `label create --name <TEXT> [--workspace] [--color <HEX>] [--description <TEXT>]
   [--parent <NAME>] [--group]` (`issueLabelCreate`: a team label by the team rule, a
   workspace label with `--workspace`; a name a label of the same scope has is refused,
   exit 6, unless `--allow-duplicate`). `label update <NAME> [--name] [--color]
   [--description] [--parent <NAME|none>]` (`issueLabelUpdate`). `label retire <NAME>`
   and `label restore <NAME>` (`issueLabelRetire`, `issueLabelRestore`: a retired label
   stays on its issues and cannot be applied to new ones). `label delete <NAME>`
   (`issueLabelDelete`: removes the label from every issue, no undo).
7. `state create --name <TEXT> --type <backlog|unstarted|started|completed|canceled>
   --color <HEX> [--description <TEXT>] [--position <N>]` (`workflowStateCreate`, in
   the team; the type cannot change afterwards, and `triage` is a type Linear sets
   itself). `state update <NAME> [--name] [--color] [--description] [--position]`
   (`workflowStateUpdate`). `state archive <NAME>` (`workflowStateArchive`: Linear
   refuses it while the state holds an unarchived issue, exit 6; no inverse).
8. `project get <NAME>` (`project`): `ProjectFields` with `content`, `slugId`, `color`,
   `icon`, `health`, `progress`, `scope`, `startedAt`, `completedAt`, `canceledAt`,
   `trashed`, `createdAt`, `updatedAt`, `creator`, `lastUpdate` (`id`, `health`,
   `createdAt`), `members` and `projectMilestones`, the last two bounded by
   `MEMBERS_PER_PROJECT` and `MILESTONES_PER_PROJECT` with a `truncated` notice.
   `project update <NAME>` (`projectUpdate`): `--name`, `--description`,
   `--content <TEXT>|--content-file <PATH>`, `--status <NAME>`, `--lead <WHO|none>`,
   `--start-date <DATE|none>`, `--target-date <DATE|none>`, `--priority <0-4>`,
   `--team <KEY>...` (which replaces the teams), `--add-member <WHO>`,
   `--remove-member <WHO>`, `--color` and `--icon`. `ProjectUpdateInput` has
   `memberIds`, the whole list, and no added or removed form; `IssueUpdateInput` has
   `addedLabelIds` and `removedLabelIds`, which is why `issue update` sends the
   change and never reads the labels back. So `--add-member` and `--remove-member`
   read the project's `members` first, through the `project` query `project get`
   sends, one page of `MEMBERS_PER_PROJECT` with `pageInfo.hasNextPage`, and send
   that list with the one member added or taken out. When `hasNextPage` is true the
   command refuses before any mutation, exit 6, a muster guard like the duplicate
   title: the message says the project has more members than `MEMBERS_PER_PROJECT`,
   and the hint names the browser as the way to change them. A `memberIds` shorter
   than the project's members would drop every member past the bound, which
   invariant 3 forbids.
   `project delete <NAME>` (`projectDelete`: the trash, thirty days) and
   `project unarchive <NAME>` (`projectUnarchive`); the deprecated `projectArchive` is
   not used.
9. `project-status list` (`projectStatuses`): `name`, `type`, `color`, `description`,
   `position` and `indefinite`. `project-status create --name <TEXT>
   --type <backlog|planned|started|paused|completed|canceled> --color <HEX>
   --position <N> [--description <TEXT>] [--indefinite <true|false>]`
   (`projectStatusCreate`), `project-status update <NAME> [...]`
   (`projectStatusUpdate`), `project-status archive <NAME>` and
   `project-status unarchive <NAME>` (`projectStatusArchive`, which Linear refuses for
   a status with active projects or the last of its type, and
   `projectStatusUnarchive`).
10. `milestone list --project <NAME>` (`projectMilestones` filtered on the project):
    `name`, `description`, `targetDate`, `status`, `progress`, `sortOrder` and
    `project`. `milestone create --project <NAME> --name <TEXT> [--description <TEXT>]
    [--target-date <DATE>]` (`projectMilestoneCreate`; a name a milestone of the
    project has is refused, exit 6, unless `--allow-duplicate`).
    `milestone update <NAME> --project <NAME> [--name] [--description]
    [--target-date <DATE|none>]` (`projectMilestoneUpdate`).
    `milestone delete <NAME> --project <NAME>` (`projectMilestoneDelete`, no undo).
    `--milestone <NAME>` on `issue create`, `issue update` (`none` clears) and
    `issue list`, through `projectMilestoneId` and the `projectMilestone` filter; the
    milestone is resolved in the issue's project.
11. `project-update list --project <NAME>` (`projectUpdates` filtered on the project):
    `body`, `health`, `createdAt`, `editedAt`, `url`, `user` and `project`.
    `project-update create --project <NAME> [--body <TEXT>|--body-file <PATH>]
    [--health <onTrack|atRisk|offTrack>]` (`projectUpdateCreate`). A project update is
    Linear's name for a status post on a project; `project update` changes the project
    itself, and the glossary says so.
12. `initiative list [--status <S>]` and `initiative get <NAME>` (`initiatives`,
    `initiative`): `name`, `description`, `status`, `health`, `targetDate`, `priority`,
    `url`, `owner`, `leadTeam`, `slugId` and `createdAt`, and on `get` the `content`
    and the `projects`, bounded by `PROJECTS_PER_INITIATIVE`. `initiative create
    --name <TEXT> [--description] [--owner <WHO>] [--lead-team <KEY>]
    [--status <Proposed|Planned|Active|Completed|Canceled>] [--target-date <DATE>]
    [--priority <0-4>] [--content <TEXT>|--content-file <PATH>]` (`initiativeCreate`,
    the duplicate guard on the name). `initiative update <NAME> [...]`
    (`initiativeUpdate`; `none` clears the owner, the lead team and the target date).
    `initiative archive <NAME>`
    and `initiative unarchive <NAME>` (`initiativeArchive`, `initiativeUnarchive`).
    `initiative add-project <NAME> --project <NAME>` and
    `initiative remove-project <NAME> --project <NAME>` (`initiativeToProjectCreate`,
    `initiativeToProjectDelete`).

**Block 3, release 0.5.0.**

13. `cycle list [--active|--upcoming|--past]` and `cycle get <NUMBER>` (`cycles` with
    the team filter and `isActive`, `isFuture` or `isPast`; `cycle`): `number`, `name`,
    `description`, `startsAt`, `endsAt`, `completedAt`, `isActive`, `isNext`,
    `isPrevious`, `isFuture`, `isPast`, `progress` and `team`, and on `get` the
    `issues`, bounded by `ISSUES_PER_CYCLE`. `cycle update <NUMBER> [--name]
    [--description] [--starts-at <TIME>] [--ends-at <TIME>]` (`cycleUpdate`).
    `--cycle <NUMBER|active|next|none>` on `issue create`, `issue update` and
    `issue list`, through `cycleId` and the `cycle` filter. The deprecated
    `cycleCreate` is not used: Linear makes cycles from the team's settings.
    `cycleArchive` stays out: it unlinks every issue and has no inverse.
14. `document list [--project <NAME>] [--issue <ISSUE>]` and `document get <TITLE>`
    (`documents`, `document`): `title`, `icon`, `color`, `content`, `url`, `slugId`,
    `createdAt`, `updatedAt`, `creator`, `owner`, `updatedBy`, `project`, `issue` and
    `trashed`. `document create --title <TEXT> [--content <TEXT>|--content-file <PATH>]
    [--project <NAME>] [--issue <ISSUE>]` (`documentCreate`, the duplicate guard on
    the title; the initiative and team links are Internal and stay out).
    `document update <TITLE> [--title] [--content|--content-file]
    [--project <NAME|none>] [--issue <ISSUE|none>]` (`documentUpdate`). `document delete <TITLE>`
    (`documentDelete`: the trash) and `document unarchive <TITLE>`
    (`documentUnarchive`).
15. `template list [--type <issue|project|document>]` and `template get <NAME>`
    (`templateSearch` with `first` and the team-or-workspace filter, asked for one
    more than the limit so that a cut list is known; `template`): `name`, `type`,
    `description`, `icon`, `color`, `templateData` as Linear returns it, `team`,
    `createdAt` and `lastAppliedAt`. `template create --name <TEXT>
    --type <issue|project|document> --data-file <PATH> [--workspace]
    [--description <TEXT>]` (`templateCreate`; the data is JSON read from the file,
    bounded by `BODY_MAX_BYTES`). `template update <NAME> [--name] [--description]
    [--data-file <PATH>]` (`templateUpdate`). `template delete <NAME>`
    (`templateDelete`, no undo).
16. `webhook list` (`webhooks`): `label`, `url`, `enabled`, `resourceTypes`,
    `allPublicTeams`, `team`, `createdAt` and `creator`; the `secret` is never asked
    for. `webhook create --url <URL> --resource-type <TYPE>... [--label <TEXT>]
    [--all-public-teams] [--secret-file <PATH>]` (`webhookCreate`; the team by the
    team rule unless `--all-public-teams`; a URL a webhook already has is refused,
    exit 6, unless `--allow-duplicate`). `webhook update <WEBHOOK> [--url] [--label]
    [--enabled <true|false>] [--resource-type <TYPE>...] [--secret-file <PATH>]`
    (`webhookUpdate`; `<WEBHOOK>` is a label, else a URL, else an id).
    `webhook delete <WEBHOOK>` (`webhookDelete`, no undo; `--enabled false` is the
    reversible stop).
17. `audit list [--type <TYPE>] [--actor <WHO>] [--since <TIME>]` (`auditEntries` with
    the filter): `type`, `createdAt`, `actor`, `ip`, `countryCode` and `metadata`.
    Linear's pages reserve the audit log for owners on the Enterprise plan; the command
    prints Linear's answer, or its refusal as exit 6.
18. `issue archive <ISSUE>` and `issue unarchive <ISSUE>` (`issueArchive`,
    `issueUnarchive`). `issue delete <ISSUE>` (`issueDelete`: the trash, thirty days,
    sub-issues included; `permanentlyDelete` is never sent). `issue batch-update
    --issue <ISSUE>... [--state <NAME>] [--priority <0-4>] [--assignee <WHO|none>]
    [--add-label <NAME>] [--project <NAME|none>] [--cycle ...] [--milestone ...]`
    (`issueBatchUpdate`; at most `ISSUES_PER_BATCH`, fifty, Linear's own cap, exit 2
    past it before any request).
19. `comment update <ID> --body <TEXT>|--body-file <PATH>` and `comment delete <ID>`
    (`commentUpdate`, `commentDelete`; the id is the one `issue get` prints).
20. `attachment update <ID> --title <TEXT> [--subtitle <TEXT>]` and
    `attachment delete <ID>` (`attachmentUpdate`, whose input requires the title;
    `attachmentDelete`).

**Rules every group follows.** A setting that is a boolean in Linear takes `true` or
`false` as its value, so one flag sets it either way and a flag left out leaves the
setting alone; the listing above writes such a flag bare, `--private` for
`--private <true|false>`, except where the value matters to the sentence. `none`
clears a nullable field, as `--assignee none` does. A name
resolves to one thing as `issue create --project` resolves a project: a team by key, a
user by `me`, an email address or a name, a label, state, project, milestone,
initiative, template or document by name in its scope, a cycle by number, an invite by
email; a name that matches nothing is exit 4 before any mutation, and a create whose
name an existing thing of the same scope already has is exit 6 unless
`--allow-duplicate`, for the reason ADR 0008 gives. The reversible command of a pair
comes before the irreversible one in the reference and the tutorial: retire before
delete, archive before delete, `--enabled false` before `webhook delete`; a command
that removes something prints what Linear returned and nothing muster reconstructed;
muster never sends `permanentlyDelete`. Every list is a bounded connection with
`--limit` up to `PAGE_CAP` and a `truncated` notice; the nested bounds are
`MEMBERS_PER_PROJECT`, `MILESTONES_PER_PROJECT`, `PROJECTS_PER_INITIATIVE` and
`ISSUES_PER_CYCLE`, and `ISSUES_PER_BATCH` is fifty. A body or a content comes
inline, `--body <TEXT>`, or from a file or stdin through its `-file` form,
`--body-file <PATH>` with `-` for stdin, as `issue comment` takes a body today; a
description comes inline, as every `--description` does today, with a `-file` form
where the listing names one. Each is checked against `BODY_MAX_BYTES`, a file through
`read_body`. A secret and a template's data come from a file or stdin only,
`--secret-file` and `--data-file`, never argv. A value Linear marks Internal, Alpha or
deprecated is not a flag and not a field. An operation that needs a role the key lacks is Linear's refusal, printed as
exit 6 with Linear's message; muster checks no role itself, since the schema documents
none and `whoami` already prints `admin`.

**Where the fixtures come from.** `just smoke` creates, once, a team `Smoke tests`
with key `SMK`, enables its cycles with `team update --cycles-enabled true
--cycle-enabled-start-date <now>`, leaves it, and records the cycle, team and
membership shapes there; the smoke team is also the only team `team retire`,
`team delete` and `team unarchive` are tested on. In the `Smoke tests` project it
creates and leaves one milestone, one project update, one document and one initiative,
and in the workspace one issue template and one webhook, disabled, pointing at a URL of
the house's own, so that `just record-fixtures` finds a node of each kind. For the
invite shape it invites the house's team address as a guest and deletes the invite
after recording. The owner chose each of these on 2026-11-03 over enabling cycles on
`APP`, over shipping the invite commands without a fixture, and over leaving the kinds
out. Cycles may not exist the moment they are enabled, so the cycle fixtures may wait
for the following smoke run.

**One group per pull request.** `docs/how-to/implement-a-feature.md`, step 3.7, now
reads: one group of related verbs per pull request where the work goes through the
review loop, and none over about a thousand changed lines, generated files aside;
`team update` and `workspace update` are pull requests of their own. The cap, not the
command count, is what fits one review and one fix run.

The slice is done when the twenty groups' commands are in
`docs/reference/commands.md`, each with its tests against the fake and its
`just smoke` output in a commit on `master`, and releases 0.3.0, 0.4.0 and 0.5.0 carry
the three blocks.

Out of the slice, each with its trigger: API keys, until Linear's schema exposes them;
workspace deletion, trials and domains, until an owner asks for one, since deletion is
irreversible, a trial changes the plan and the domain operations are Internal; the
listing of a user's sessions, until Linear bounds it; membership ownership, document
sharing and team hierarchy inheritance, until Linear removes the Internal marker;
integrations, which need a browser for their OAuth flows; customers, releases,
roadmaps, saved views, emoji, agent skills, time schedules, triage responsibility, git
automation and email intake, until a skill in this repository needs one;
`cycleArchive` and `cycleShiftAll`, until a team asks for a schedule change the browser
cannot make; project relations and labels, initiative labels, updates and relations,
until a skill needs one; and every operation Linear marks deprecated.

## Alternatives considered

- **A raw `muster graphql` command, which would cover all of this in one.** Lost, as
  in ADR 0013: an unbounded surface the fake cannot answer, the reference cannot
  describe and the glossary cannot cover.
- **One command per pull request, as step 3.7 said.** Lost: eighty-one reviews, fix
  runs and landing comments for a slice whose groups share one operation section, one
  fake model and one tutorial step each; the thousand-line cap was the reason for the
  rule, and the cap stands.
- **Cycles on `APP` instead of a smoke team.** Lost: every `APP` issue gains a cycle
  and Linear starts scheduling the team the house works in; ADR 0013 refused that from
  a tool, and the owner refused it on 2026-11-03 knowing the smoke team takes the Free
  plan's second team slot.
- **No invite fixture, or the invite commands held back.** Lost: ADR 0010 allows no
  shape the workspace did not answer, and the owner chose an invite to the house's own
  address over leaving five commands out.
- **A confirmation flag on the irreversible deletes.** Lost: a flag that must be passed
  every time is a prompt by another name (ADR 0008), and the browser asks no more of an
  owner than muster does; the order of the pairs and the reference page carry the
  warning instead.
- **`user promote` and `user demote`.** Lost: the schema has one mutation,
  `userChangeRole`, and one command per operation is the rule.
- **`organization get` and `organization update`, after Linear's API type.** Lost:
  Linear's product, its settings page and its documentation say workspace, the
  glossary row maps the word to the type, and `whoami` keeps printing `organization`,
  since a field is Linear's and a command name is the reader's; a reader who has never
  used the API looks for the product's word.
- **Every field of `TeamUpdateInput` and `OrganizationUpdateInput` as a flag.** Lost:
  the Internal, Alpha, deprecated and integration-coupled fields change under a released
  flag, which invariant 2 forbids; each flag costs a glossary row and a reference row
  for a setting no agent of the house sets.
- **The deprecated `projectArchive`, `cycleCreate` and `projectUpdateDelete`.** Lost:
  Linear's deprecation page says obsolete functionality may stay as a non-functional
  stub; the replacements are the commands.
- **`member list` through the workspace-wide `teamMemberships` query.** Lost: it takes
  no team filter, so the team's memberships would be a client-side cut of a bounded
  page, which can miss some; `team.memberships` is bounded and exact.
- **`issue delete --permanently` for admins.** Lost: the trash is the undo, and a tool
  that skips it on a flag is one an owner cannot trust with a key.
- **Three pull requests, one per block.** Lost: each would pass the thousand-line cap
  many times over, and a first review's blockers would not fit one fix run.

## Evidence

- Linear's GraphQL schema, introspected with the house's key on 2026-11-03 by the
  `muster-researcher` session, read-only: the `Query` root (182 fields) and the
  `Mutation` root (391 fields) with their descriptions and deprecations, and 660 types,
  among them `Organization`, `OrganizationUpdateInput`,
  `OrganizationSecuritySettingsInput`, `OrganizationAuthSettingsInput`,
  `OrganizationInvite`, `OrganizationInviteCreateInput`, `User`, `UserUpdateInput`,
  `UserRoleType`, `Team`, `TeamCreateInput`, `TeamUpdateInput`,
  `TeamSecuritySettingsInput`, `TeamRoleType`, `TeamMembership`,
  `TeamMembershipCreateInput`, `Project`, `ProjectUpdateInput`, `ProjectStatus`,
  `ProjectStatusCreateInput`, `ProjectMilestone`, `ProjectMilestoneCreateInput`,
  `ProjectUpdate`, `ProjectUpdateCreateInput`, `Initiative`, `InitiativeCreateInput`,
  `InitiativeUpdateInput`, `IssueLabel`, `IssueLabelCreateInput`, `WorkflowState`,
  `WorkflowStateCreateInput`, `Cycle`, `CycleUpdateInput`, `Template`,
  `TemplateCreateInput`, `Webhook`, `WebhookCreateInput`, `Document`,
  `DocumentCreateInput`, `AuditEntry`, `AuditEntryFilter`, `AttachmentUpdateInput` and
  `CommentUpdateInput`. The facts in Context are read from them.
- Linear's documentation, read the same day: the GraphQL API, OAuth scopes, rate
  limiting and deprecations pages under `linear.app/developers`, and the members and
  roles, API keys and webhooks, workspaces, teams, invites, audit log, labels and
  workflows, trash, cycles, documents, initiatives, projects, and billing and plans
  pages under `linear.app/docs`, with `linear.app/pricing` for the plan limits.
- The `appshapes` workspace on 2026-11-03, read with the key: the counts in Context,
  every connection asked for five at a time.
- The owner's answers on 2026-11-03: the smoke team, the invite to the house address,
  `--url-key`, the six deletes, one group per pull request; the plan is Free, the smoke
  team may take the second team slot, and an upgrade is acceptable if the house needs a
  third team.
- ADR 0013's done-when on `master`: the three features in `docs/reference/commands.md`
  at b335e1d, the tag `v0.2.0`, release run 41574432801 and `muster-linear` 0.2.0 on
  crates.io, all 2026-11-03.
- The loop's timings as `.claude/skills/pr/SKILL.md` records them: a review about ten
  minutes after a push, a fix run up to thirty minutes.

## Consequences

Easier: an agent sets a workspace up, from the team and its states and labels to the
members, invites, milestones and webhooks, with no browser; APP-6's layout decision has
both sides open; cycles are enabled on purpose by the owner's own command; a mistaken
comment, link or issue is cleaned up by the agent that made it.

Harder: `operations.rs` gains about forty operations and the fake fourteen models;
`bounds.rs` five constants; the glossary a row per new noun and `commands.md` a section
per command; `just smoke` creates lasting objects in the workspace (one team, a
milestone, a project update, a document, an initiative, a template, a disabled webhook)
and sends one email per run; on the Free plan the smoke team is the second of two
teams, and every smoke issue counts toward 250, which `issue delete` can trash once it
exists; the recorder's `partial` notice is the signal when a kind is empty at recording
time.

Revisit: when each block releases, for the next block's issues; when a kind's fixture
cannot be recorded after two smoke runs; when `issue unarchive` proves not to restore
an issue `issue delete` trashed, in which case `issue delete` documents the browser as
the recovery and the pair is split; when Linear's audit log refuses the key, in which
case `audit list` documents the plan it needs; when the house needs a third team; when
Linear removes a deprecated operation or an Internal marker named above; when the
slice is done, for the fourth slice's ADR.

## References

- ADR 0007 and ADR 0013 (the first two slices), ADR 0008 (the output contract, the
  resolution order, the duplicate guard), ADR 0010 (the fixtures and the fake)
- APP-31 and its sub-issues in the `appshapes` workspace; APP-6 (the layout decision)
- `docs/how-to/implement-a-feature.md`, `docs/how-to/cut-a-release.md`,
  `.claude/skills/pr/SKILL.md`
- Linear's API: <https://linear.app/developers/graphql>,
  <https://linear.app/developers/deprecations>,
  <https://linear.app/developers/rate-limiting>; Linear's pages:
  <https://linear.app/docs/members-roles>, <https://linear.app/docs/invite-members>,
  <https://linear.app/docs/delete-archive-issues>, <https://linear.app/docs/use-cycles>,
  <https://linear.app/docs/audit-log>, <https://linear.app/pricing>
