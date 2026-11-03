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
| attachment | A link on an issue, with a title and a URL. | `attachment` | attachment | `docs/reference/commands.md#muster-issue-get` |
| sub-issue | An issue whose `parent` is another issue; the parent prints its `children`, and `--parent` on `issue create`, `issue update` and `issue list` names one. | `parent`, `children`, `parentId` | checklist item (the nearest) | `docs/reference/commands.md#muster-issue-get` |
| assignee | The user responsible for an issue; `me` is the key's owner. | `assignee` | member | `docs/reference/commands.md#muster-issue-create` |
| priority | An integer from 0 (none) to 4 (low); 1 is urgent. | `priority`, `priorityLabel` | none | `docs/reference/commands.md#muster-issue-create` |
| session stamp | The last line of a comment an agent writes: `Session stamp: <team>/<session>, <date>`. | none | the same line | `.claude/skills/linear-done/SKILL.md` |
| exit code | What muster returns to the shell; each failure class has its own. | none | none | `docs/reference/commands.md#exit-codes` |
| error object | The one JSON object muster writes to stderr on failure. | none | none | `docs/reference/commands.md#exit-codes` |
| bound | A named limit: page size, page cap, timeout, body size, retries. | none | none | `docs/reference/commands.md#bounds` |
| fake Linear | The in-process server the tests and the quickstart run against. | none | none | `docs/decisions/0010-test-against-a-fake-linear-held-to-recorded-shapes.md` |
| fixture | A recorded shape of a real Linear answer, values stripped, that the fake is held to. | none | none | `docs/decisions/0010-test-against-a-fake-linear-held-to-recorded-shapes.md` |
| operation | One named GraphQL query or mutation muster sends; the fake answers by the name. | `operationName` | none | `crates/muster/src/linear/operations.rs` |
| request id | Linear's `x-request-id` for one answer, carried in every error object. | `x-request-id` | none | `docs/reference/commands.md#exit-codes` |
