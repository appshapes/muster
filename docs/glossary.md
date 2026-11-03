# Glossary

One row per term that appears in a command, a flag, an output field or an error
message. The Linear column is the name in Linear's API, which is also the output
field's name; the Trello column exists so that a reader coming from the Trello skills
can map vocabulary in one place, and pages elsewhere mention Trello at most once.

| Muster term | Definition | Linear API name | Trello word | Reference |
| --- | --- | --- | --- | --- |
| workspace | One Linear organisation, reached with one API key; `muster whoami` names it and `muster workspace get` prints its settings. | `organization` | workspace | `docs/reference/commands.md#muster-workspace-get` |
| url key | The part of the workspace's web address that names it, `appshapes` in linear.app/appshapes; `--url-key` on `workspace update`, and Linear keeps the last three previous keys redirecting. | `urlKey`, `previousUrlKeys` | none | `docs/reference/commands.md#muster-workspace-update` |
| logo url | The address of the workspace's logo image; `--logo-url` on `workspace update`. | `logoUrl` | none | `docs/reference/commands.md#muster-workspace-update` |
| git branch format | The template for the branch name Linear suggests for an issue; `--git-branch-format` on `workspace update`, `none` for Linear's default. | `gitBranchFormat` | none | `docs/reference/commands.md#muster-workspace-update` |
| git linkback messages | Whether the git integration comments a link back to the issue on a private repository's pull request; `--git-linkback-messages`. | `gitLinkbackMessagesEnabled` | none | `docs/reference/commands.md#muster-workspace-update` |
| git public linkback messages | The same for a public repository's pull request; `--git-public-linkback-messages`. | `gitPublicLinkbackMessagesEnabled` | none | `docs/reference/commands.md#muster-workspace-update` |
| git linkback descriptions | Whether a linkback comment carries the issue's description; `--git-linkback-descriptions`. | `gitLinkbackDescriptionsEnabled` | none | `docs/reference/commands.md#muster-workspace-update` |
| suggested branch name | Whether copying an issue, or opening it in a coding tool, suggests a branch name; `--suggested-branch-name`. | `suggestedBranchNameEnabled` | none | `docs/reference/commands.md#muster-workspace-update` |
| roadmap | Linear's roadmap view of projects, on or off for the workspace; `--roadmap` on `workspace update`. | `roadmapEnabled` | none | `docs/reference/commands.md#muster-workspace-update` |
| project update reminder weeks | How many weeks apart Linear asks a project's lead for a project update; `--project-update-reminder-weeks`. | `projectUpdateReminderFrequencyInWeeks` | none | `docs/reference/commands.md#muster-workspace-update` |
| project update reminders day | The weekday a project update reminder goes out, `Sunday` to `Saturday`; `--project-update-reminders-day`. | `projectUpdateRemindersDay`, `Day` | none | `docs/reference/commands.md#muster-workspace-update` |
| project update reminders hour | The hour, 0 to 23, a project update reminder goes out; `--project-update-reminders-hour`. | `projectUpdateRemindersHour` | none | `docs/reference/commands.md#muster-workspace-update` |
| fiscal year start month | The month the workspace's fiscal year starts, 0 for January to 11 for December; `--fiscal-year-start-month`. | `fiscalYearStartMonth` | none | `docs/reference/commands.md#muster-workspace-update` |
| feed | Linear's activity feed, on or off for the workspace; `--feed` on `workspace update`. | `feedEnabled` | none | `docs/reference/commands.md#muster-workspace-update` |
| pull request tour | Whether Linear writes an AI guide for a new pull request; `--pull-request-tour`. | `pullRequestTourEnabled` | none | `docs/reference/commands.md#muster-workspace-update` |
| pull request issue mode | How Linear links an issue to a pull request that names none: `off`, `suggest` or `autoOnMerge`; `--pull-request-issue-mode`. | `pullRequestIssueMode` | none | `docs/reference/commands.md#muster-workspace-update` |
| ai thread summaries | Whether a resolved comment thread gets an AI summary; `--ai-thread-summaries`. | `aiThreadSummariesEnabled` | none | `docs/reference/commands.md#muster-workspace-update` |
| ai discussion summaries | Whether an issue's discussion gets an AI summary; `--ai-discussion-summaries`. | `aiDiscussionSummariesEnabled` | none | `docs/reference/commands.md#muster-workspace-update` |
| allowed file upload content types | The content types an upload may have, such as `image/png`, comma-separated; `--allowed-file-upload-content-types`. | `allowedFileUploadContentTypes` | none | `docs/reference/commands.md#muster-workspace-update` |
| role | A person's permission level in the workspace: `owner`, `admin`, `user` (a member) or `guest`; every member is an admin on the Free plan. | `UserRoleType`, `admin`, `owner`, `guest` | admin, normal, observer (the nearest) | `docs/reference/commands.md#muster-workspace-update` |
| invitations role | The least role that may invite a user; `--invitations-role` on `workspace update`. | `securitySettings.invitationsRole` | none | `docs/reference/commands.md#muster-workspace-update` |
| team creation role | The least role that may create a team; `--team-creation-role`. | `securitySettings.teamCreationRole` | none | `docs/reference/commands.md#muster-workspace-update` |
| label management role | The least role that may manage workspace labels; `--label-management-role`. | `securitySettings.labelManagementRole` | none | `docs/reference/commands.md#muster-workspace-update` |
| personal api keys role | The least role that may create a personal API key; `--personal-api-keys-role`. | `securitySettings.personalApiKeysRole` | none | `docs/reference/commands.md#muster-workspace-update` |
| admin management role | The least role that may grant or revoke the admin role; `--admin-management-role`. | `securitySettings.adminManagementRole` | none | `docs/reference/commands.md#muster-workspace-update` |
| template management role | The least role that may manage workspace templates; `--template-management-role`. | `securitySettings.templateManagementRole` | none | `docs/reference/commands.md#muster-workspace-update` |
| integration creation role | The least role that may install an integration; `--integration-creation-role`. | `securitySettings.integrationCreationRole` | none | `docs/reference/commands.md#muster-workspace-update` |
| import role | The least role that may import data; `--import-role`. | `securitySettings.importRole` | none | `docs/reference/commands.md#muster-workspace-update` |
| api settings role | The least role that may manage the workspace's API settings; `--api-settings-role`. | `securitySettings.apiSettingsRole` | none | `docs/reference/commands.md#muster-workspace-update` |
| workspace initiatives role | The least role that may create an initiative with no lead team; `--workspace-initiatives-role`. | `securitySettings.workspaceInitiativesRole` | none | `docs/reference/commands.md#muster-workspace-update` |
| allowed auth services | The sign-in services the workspace allows, comma-separated, or `all`, which is Linear's empty list and the one spelling of it muster sends (a value that names no service is exit 2); `--allowed-auth-services`. | `authSettings.allowedAuthServices` | none | `docs/reference/commands.md#muster-workspace-update` |
| hide non primary organizations | Whether a new user whose email domain this workspace claims is shown no other workspace at sign-up; `--hide-non-primary-organizations`. | `authSettings.hideNonPrimaryOrganizations` | none | `docs/reference/commands.md#muster-workspace-update` |
| key | The Linear API key muster authenticates with, read from the environment or a file, never from argv. | API key | API key and token | `docs/reference/commands.md#the-key` |
| team | The unit that owns issues and gives them their identifier prefix (`APP`). | `team`, `team.key` | board | `docs/reference/commands.md#muster-team-list` |
| issue | One unit of work, with a title, a description in markdown, a state, labels and comments. | `issue` | card | `docs/reference/commands.md#muster-issue-get` |
| identifier | An issue's human name: the team key, a dash and the number, `APP-12`. | `identifier` | card number in the URL | `docs/reference/commands.md#muster-issue-get` |
| number | The integer part of an identifier, unique inside a team. | `number` | `idShort` | `docs/reference/commands.md#muster-issue-get` |
| state | Where an issue is in its team's workflow; each state has a type: backlog, unstarted, started, completed, canceled or duplicate. | `workflowState`, `state` | list | `docs/reference/commands.md#muster-state-list` |
| completed state | The team's first state of type `completed`, where `issue close` puts an issue. | `WorkflowState.type == "completed"` | mark complete and archive | `docs/reference/commands.md#muster-issue-close` |
| label | A named tag on an issue; an issue can carry several. | `issueLabel`, `labels` | label | `docs/reference/commands.md#muster-label-list` |
| project | A named group of issues across teams, with a status of its own; `project create` makes one, `--project` on `issue create`, `issue update` and `issue list` names one. | `project` | none | `docs/reference/commands.md#muster-project-create` |
| comment | A markdown note on an issue, by a user; `issue get` prints them newest first, as Linear orders them. | `comment` | comment | `docs/reference/commands.md#muster-issue-comment` |
| attachment | A link on an issue, with a title and a URL; `issue attach` adds one and `issue get` prints them, newest first. | `attachment` | attachment | `docs/reference/commands.md#muster-issue-attach` |
| sub-issue | An issue whose `parent` is another issue; the parent prints its `children`, and `--parent` on `issue create`, `issue update` and `issue list` names one. | `parent`, `children`, `parentId` | checklist item (the nearest) | `docs/reference/commands.md#muster-issue-get` |
| user | A member of the workspace, named by `me`, an email address or a name; `muster user list` shows them. | `user` | member | `docs/reference/commands.md#muster-user-list` |
| assignee | The user responsible for an issue; `me` is the key's owner. | `assignee` | member | `docs/reference/commands.md#muster-issue-create` |
| title | An issue's one-line name, or an attachment's; `--title` on `issue create`, `issue update` and `issue attach`. | `title` | name | `docs/reference/commands.md#muster-issue-create` |
| description | An issue's text, markdown; `--description` or `--description-file`. | `description` | desc | `docs/reference/commands.md#muster-issue-create` |
| body | A comment's text, markdown; `--body` or `--body-file`. | `body` | text | `docs/reference/commands.md#muster-issue-comment` |
| parent | The issue a sub-issue belongs to; `--parent` on `issue create`, `issue update` and `issue list`. | `parent`, `parentId` | the card that holds the checklist (the nearest) | `docs/reference/commands.md#muster-issue-get` |
| name | What a team, state, label or project is called; the value of `--state`, `--label` and `--project`. | `name` | name | `docs/reference/commands.md#muster-state-list` |
| url | A link's address, on an attachment; `url` is also every issue's own web address. | `url` | attachment URL | `docs/reference/commands.md#muster-issue-get` |
| subtitle | A link's second line, under its title, on an attachment. | `subtitle` | none | `docs/reference/commands.md#muster-issue-get` |
| search | Text an issue's title must contain, case-insensitively; `--search` on `issue list`. | `title.containsIgnoreCase` | search | `docs/reference/commands.md#muster-issue-list` |
| limit | The most items a list prints, `--limit`, at most `PAGE_CAP`; past it a `truncated` notice. | `first` | none | `docs/reference/commands.md#bounds` |
| priority | An integer from 0 (none) to 4 (low); 1 is urgent. | `priority`, `priorityLabel` | none | `docs/reference/commands.md#muster-issue-create` |
| session stamp | The last line of a comment an agent writes: `Session stamp: <team>/<session>, <date>`. | none | the same line | `.claude/skills/linear-done/SKILL.md` |
| exit code | What muster returns to the shell; each failure class has its own. | none | none | `docs/reference/commands.md#exit-codes` |
| error object | The one JSON object muster writes to stderr on failure. | none | none | `docs/reference/commands.md#exit-codes` |
| bound | A named limit: page size, page cap, timeout, body size, retries. | none | none | `docs/reference/commands.md#bounds` |
| fake Linear | The in-process server the tests and the quickstart run against. | none | none | `docs/decisions/0010-test-against-a-fake-linear-held-to-recorded-shapes.md` |
| fixture | A recorded shape of a real Linear answer, values stripped, that the fake is held to. | none | none | `docs/decisions/0010-test-against-a-fake-linear-held-to-recorded-shapes.md` |
| operation | One named GraphQL query or mutation muster sends; the fake answers by the name. | `operationName` | none | `crates/muster/src/linear/operations.rs` |
| request id | Linear's `x-request-id` for one answer, carried in every error object. | `x-request-id` | none | `docs/reference/commands.md#exit-codes` |
