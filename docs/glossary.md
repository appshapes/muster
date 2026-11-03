# Glossary

One row per term that appears in a command, a flag, an output field or an error
message. The Linear column is the name in Linear's API, which is also the output
field's name; the Trello column exists so that a reader coming from the Trello skills
can map vocabulary in one place, and pages elsewhere mention Trello at most once.

| Muster term | Definition | Linear API name | Trello word | Reference |
| --- | --- | --- | --- | --- |
| workspace | One Linear organisation, reached with one API key; `muster whoami` names it. | `organization` | workspace | `docs/reference/commands.md#muster-whoami` |
| key | The Linear API key muster authenticates with, read from the environment or a file, never from argv. | API key | API key and token | `docs/reference/commands.md#the-key` |
| team | The unit that owns issues and gives them their identifier prefix (`APP`). | `team`, `team.key` | board | `docs/reference/commands.md#muster-team-list` |
| issue | One unit of work, with a title, a description in markdown, a state, labels and comments. | `issue` | card | `docs/reference/commands.md#muster-issue-get` |
| identifier | An issue's human name: the team key, a dash and the number, `APP-12`. | `identifier` | card number in the URL | `docs/reference/commands.md#muster-issue-get` |
| number | The integer part of an identifier, unique inside a team. | `number` | `idShort` | `docs/reference/commands.md#muster-issue-get` |
| state | Where an issue is in its team's workflow; each state has a type: backlog, unstarted, started, completed, canceled or duplicate. | `workflowState`, `state` | list | `docs/reference/commands.md#muster-state-list` |
| completed state | The team's first state of type `completed`, where `issue close` puts an issue. | `WorkflowState.type == "completed"` | mark complete and archive | `docs/reference/commands.md#muster-issue-close` |
| label | A named tag on an issue; an issue can carry several. | `issueLabel`, `labels` | label | `docs/reference/commands.md#muster-label-list` |
| project | A named group of issues across teams, with a state of its own. | `project` | none | `docs/reference/commands.md#muster-project-list` |
| comment | A markdown note on an issue, by a user; `issue get` prints them newest first, as Linear orders them. | `comment` | comment | `docs/reference/commands.md#muster-issue-comment` |
| attachment | A link on an issue, with a title and a URL; `issue attach` adds one and `issue get` prints them, newest first. | `attachment` | attachment | `docs/reference/commands.md#muster-issue-attach` |
| sub-issue | An issue whose `parent` is another issue; the parent prints its `children`, and `--parent` on `issue create`, `issue update` and `issue list` names one. | `parent`, `children`, `parentId` | checklist item (the nearest) | `docs/reference/commands.md#muster-issue-get` |
| user | A member of the workspace, named by `me`, an email address or a name; `muster user list` shows them. | `user` | member | `docs/reference/commands.md#muster-user-list` |
| assignee | The user responsible for an issue; `me` is the key's owner. | `assignee` | member | `docs/reference/commands.md#muster-issue-create` |
| title | An issue's one-line name; `--title` on `issue create` and `issue update`. | `title` | name | `docs/reference/commands.md#muster-issue-create` |
| description | An issue's text, markdown; `--description` or `--description-file`. | `description` | desc | `docs/reference/commands.md#muster-issue-create` |
| body | A comment's text, markdown; `--body` or `--body-file`. | `body` | text | `docs/reference/commands.md#muster-issue-comment` |
| parent | The issue a sub-issue belongs to; `--parent` on `issue create`, `issue update` and `issue list`. | `parent`, `parentId` | checklist item (the nearest) | `docs/reference/commands.md#muster-issue-get` |
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
