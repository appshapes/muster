//! What each command does: resolve names to ids, run the operations, emit Linear's
//! answers (ADR 0007, ADR 0008). Every handler writes through [`Output`] as it goes and
//! returns the first failure.

use serde_json::{Map, Value, json};

use crate::bounds::{
    ATTACHMENTS_PER_ISSUE, CHILDREN_PER_ISSUE, COMMENTS_PER_ISSUE, ISSUES_PER_BATCH,
    ISSUES_PER_CYCLE, MEMBERS_PER_PROJECT, MILESTONES_PER_PROJECT, PAGE_CAP,
    PROJECTS_PER_INITIATIVE, TEAMS_PER_PROJECT,
};
use crate::cli::{
    AttachArgs, AuditCommand, AuditListArgs, BatchUpdateArgs, Cli, CloseArgs, Command, CommentArgs,
    CommentCommand, CommentRefArgs, CommentUpdateArgs, CreateArgs, CycleCommand, CycleListArgs,
    CycleRefArgs, CycleUpdateArgs, DocumentCommand, DocumentCreateArgs, DocumentListArgs,
    DocumentRefArgs, DocumentUpdateArgs, GetArgs, InitiativeCommand, InitiativeCreateArgs,
    InitiativeListArgs, InitiativeProjectArgs, InitiativeRefArgs, InitiativeUpdateArgs,
    IssueCommand, IssueListArgs, IssueRefArgs, LabelCommand, LabelCreateArgs, LabelRefArgs,
    LabelUpdateArgs, ListArgs, MemberArgs, MemberCommand, MilestoneCommand, MilestoneCreateArgs,
    MilestoneListArgs, MilestoneRefArgs, MilestoneUpdateArgs, ProjectCommand, ProjectCreateArgs,
    ProjectRefArgs, ProjectStatusCommand, ProjectStatusCreateArgs, ProjectStatusRefArgs,
    ProjectStatusUpdateArgs, ProjectUpdateArgs, ProjectUpdateCommand, ProjectUpdateCreateArgs,
    ProjectUpdateListArgs, StateCommand, StateCreateArgs, StateRefArgs, StateUpdateArgs,
    TeamCommand, TeamCreateArgs, TeamRefArgs, TeamUpdateArgs, TemplateCommand, TemplateCreateArgs,
    TemplateListArgs, TemplateRefArgs, TemplateUpdateArgs, UpdateArgs, UserCommand, UserListArgs,
    UserRefArgs, UserRoleArgs, UserUpdateArgs, WebhookCommand, WebhookCreateArgs, WebhookListArgs,
    WebhookRefArgs, WebhookUpdateArgs, WorkspaceCommand, WorkspaceUpdateArgs,
};
use crate::config::{Config, KeySource, TeamSource};
use crate::error::{Error, ErrorCode};
use crate::linear::operations as ops;
use crate::linear::select::{
    Filter, at, find_by, has_next_page, is_identifier, is_uuid, names, need_str, nodes, str_at,
};
use crate::linear::{Linear, now_iso};
use crate::output::{Output, Text, check_body, read_body};
use crate::{DOCS_URL, VERSION};

/// Run the parsed command line.
pub fn run(cli: &Cli, out: &Output) -> Result<(), Error> {
    let config = Config::load(cli.team.clone(), cli.trace);
    match &cli.command {
        Command::Schema(args) => super::schema::run(args, out),
        Command::Fixtures(args) => {
            super::schema::record_fixtures(&Session::open(&config)?, &args.dir, out)
        }
        Command::Whoami => whoami(&Session::open(&config)?, out),
        Command::Workspace { command } => {
            let session = Session::open(&config)?;
            match command {
                WorkspaceCommand::Get => workspace_get(&session, out),
                WorkspaceCommand::Update(args) => workspace_update(&session, args, out),
            }
        }
        Command::Team { command } => {
            let session = Session::open(&config)?;
            match command {
                TeamCommand::List(args) => list_teams(&session, args, out),
                TeamCommand::Get(args) => team_get(&session, args, out),
                TeamCommand::Create(args) => team_create(&session, args, out),
                TeamCommand::Update(args) => team_update(&session, args, out),
                TeamCommand::Retire(args) => team_retire(&session, args, true, out),
                TeamCommand::Unretire(args) => team_retire(&session, args, false, out),
                TeamCommand::Delete(args) => team_delete(&session, args, out),
                TeamCommand::Unarchive(args) => team_unarchive(&session, args, out),
            }
        }
        Command::Member { command } => {
            let session = Session::open(&config)?;
            match command {
                MemberCommand::List(args) => member_list(&session, args, out),
                MemberCommand::Add(args) => member_add(&session, args, out),
                MemberCommand::Remove(args) => member_remove(&session, args, out),
            }
        }
        Command::State { command } => {
            let session = Session::open(&config)?;
            match command {
                StateCommand::List(args) => list_states(&session, args, out),
                StateCommand::Create(args) => state_create(&session, args, out),
                StateCommand::Update(args) => state_update(&session, args, out),
                StateCommand::Archive(args) => state_archive(&session, args, out),
            }
        }
        Command::Label { command } => {
            let session = Session::open(&config)?;
            match command {
                LabelCommand::List(args) => list_labels(&session, args, out),
                LabelCommand::Create(args) => label_create(&session, args, out),
                LabelCommand::Update(args) => label_update(&session, args, out),
                LabelCommand::Retire(args) => {
                    label_lifecycle(&session, args, &ops::LABEL_RETIRE, "issueLabelRetire", out)
                }
                LabelCommand::Restore(args) => label_lifecycle(
                    &session,
                    args,
                    &ops::LABEL_RESTORE,
                    "issueLabelRestore",
                    out,
                ),
                LabelCommand::Delete(args) => label_delete(&session, args, out),
            }
        }
        Command::Project { command } => {
            let session = Session::open(&config)?;
            match command {
                ProjectCommand::List(args) => list_projects(&session, args, out),
                ProjectCommand::Get(args) => project_get(&session, args, out),
                ProjectCommand::Create(args) => project_create(&session, args, out),
                ProjectCommand::Update(args) => project_update(&session, args, out),
                ProjectCommand::Delete(args) => project_delete(&session, args, out),
                ProjectCommand::Unarchive(args) => project_unarchive(&session, args, out),
            }
        }
        Command::Initiative { command } => {
            let session = Session::open(&config)?;
            match command {
                InitiativeCommand::List(args) => initiative_list(&session, args, out),
                InitiativeCommand::Get(args) => initiative_get(&session, args, out),
                InitiativeCommand::Create(args) => initiative_create(&session, args, out),
                InitiativeCommand::Update(args) => initiative_update(&session, args, out),
                InitiativeCommand::Archive(args) => initiative_lifecycle(
                    &session,
                    args,
                    &ops::INITIATIVE_ARCHIVE,
                    "initiativeArchive",
                    false,
                    out,
                ),
                InitiativeCommand::Unarchive(args) => initiative_lifecycle(
                    &session,
                    args,
                    &ops::INITIATIVE_UNARCHIVE,
                    "initiativeUnarchive",
                    true,
                    out,
                ),
                InitiativeCommand::AddProject(args) => initiative_add_project(&session, args, out),
                InitiativeCommand::RemoveProject(args) => {
                    initiative_remove_project(&session, args, out)
                }
            }
        }
        Command::Cycle { command } => {
            let session = Session::open(&config)?;
            match command {
                CycleCommand::List(args) => cycle_list(&session, args, out),
                CycleCommand::Get(args) => cycle_get(&session, args, out),
                CycleCommand::Update(args) => cycle_update(&session, args, out),
            }
        }
        Command::Document { command } => {
            let session = Session::open(&config)?;
            match command {
                DocumentCommand::List(args) => document_list(&session, args, out),
                DocumentCommand::Get(args) => document_get(&session, args, out),
                DocumentCommand::Create(args) => document_create(&session, args, out),
                DocumentCommand::Update(args) => document_update(&session, args, out),
                DocumentCommand::Delete(args) => document_lifecycle(
                    &session,
                    args,
                    &ops::DOCUMENT_DELETE,
                    "documentDelete",
                    false,
                    out,
                ),
                DocumentCommand::Unarchive(args) => document_lifecycle(
                    &session,
                    args,
                    &ops::DOCUMENT_UNARCHIVE,
                    "documentUnarchive",
                    true,
                    out,
                ),
            }
        }
        Command::Template { command } => {
            let session = Session::open(&config)?;
            match command {
                TemplateCommand::List(args) => template_list(&session, args, out),
                TemplateCommand::Get(args) => template_get(&session, args, out),
                TemplateCommand::Create(args) => template_create(&session, args, out),
                TemplateCommand::Update(args) => template_update(&session, args, out),
                TemplateCommand::Delete(args) => template_delete(&session, args, out),
            }
        }
        Command::Webhook { command } => {
            let session = Session::open(&config)?;
            match command {
                WebhookCommand::List(args) => webhook_list(&session, args, out),
                WebhookCommand::Create(args) => webhook_create(&session, args, out),
                WebhookCommand::Update(args) => webhook_update(&session, args, out),
                WebhookCommand::Delete(args) => webhook_delete(&session, args, out),
            }
        }
        Command::Audit { command } => {
            let session = Session::open(&config)?;
            match command {
                AuditCommand::List(args) => audit_list(&session, args, out),
            }
        }
        Command::Comment { command } => {
            let session = Session::open(&config)?;
            match command {
                CommentCommand::Update(args) => comment_update(&session, args, out),
                CommentCommand::Delete(args) => comment_delete(&session, args, out),
            }
        }
        Command::Milestone { command } => {
            let session = Session::open(&config)?;
            match command {
                MilestoneCommand::List(args) => milestone_list(&session, args, out),
                MilestoneCommand::Create(args) => milestone_create(&session, args, out),
                MilestoneCommand::Update(args) => milestone_update(&session, args, out),
                MilestoneCommand::Delete(args) => milestone_delete(&session, args, out),
            }
        }
        Command::ProjectUpdate { command } => {
            let session = Session::open(&config)?;
            match command {
                ProjectUpdateCommand::List(args) => project_update_list(&session, args, out),
                ProjectUpdateCommand::Create(args) => project_update_create(&session, args, out),
            }
        }
        Command::ProjectStatus { command } => {
            let session = Session::open(&config)?;
            match command {
                ProjectStatusCommand::List(args) => project_status_list(&session, args, out),
                ProjectStatusCommand::Create(args) => project_status_create(&session, args, out),
                ProjectStatusCommand::Update(args) => project_status_update(&session, args, out),
                ProjectStatusCommand::Archive(args) => project_status_lifecycle(
                    &session,
                    args,
                    &ops::PROJECT_STATUS_ARCHIVE,
                    "projectStatusArchive",
                    false,
                    out,
                ),
                ProjectStatusCommand::Unarchive(args) => project_status_lifecycle(
                    &session,
                    args,
                    &ops::PROJECT_STATUS_UNARCHIVE,
                    "projectStatusUnarchive",
                    true,
                    out,
                ),
            }
        }
        Command::User { command } => {
            let session = Session::open(&config)?;
            match command {
                UserCommand::List(args) => list_users(&session, args, out),
                UserCommand::Get(args) => user_get(&session, args, out),
                UserCommand::Update(args) => user_update(&session, args, out),
                UserCommand::ChangeRole(args) => user_change_role(&session, args, out),
                UserCommand::Suspend(args) => {
                    user_admin(&session, args, &ops::USER_SUSPEND, "userSuspend", out)
                }
                UserCommand::Unsuspend(args) => {
                    user_admin(&session, args, &ops::USER_UNSUSPEND, "userUnsuspend", out)
                }
                UserCommand::RevokeSessions(args) => user_admin(
                    &session,
                    args,
                    &ops::USER_REVOKE_SESSIONS,
                    "userRevokeAllSessions",
                    out,
                ),
            }
        }
        Command::Issue { command } => {
            let session = Session::open(&config)?;
            match command {
                IssueCommand::Get(args) => issue_get(&session, args, out),
                IssueCommand::List(args) => issue_list(&session, args, out),
                IssueCommand::Create(args) => issue_create(&session, args, out),
                IssueCommand::Comment(args) => issue_comment(&session, args, out),
                IssueCommand::Attach(args) => issue_attach(&session, args, out),
                IssueCommand::Close(args) => issue_close(&session, args, out),
                IssueCommand::Update(args) => issue_update(&session, args, out),
                IssueCommand::Archive(args) => {
                    issue_lifecycle(&session, args, &ops::ISSUE_ARCHIVE, "issueArchive", out)
                }
                IssueCommand::Unarchive(args) => {
                    issue_lifecycle(&session, args, &ops::ISSUE_UNARCHIVE, "issueUnarchive", out)
                }
                IssueCommand::Delete(args) => {
                    issue_lifecycle(&session, args, &ops::ISSUE_DELETE, "issueDelete", out)
                }
                IssueCommand::BatchUpdate(args) => issue_batch_update(&session, args, out),
            }
        }
    }
}

/// A connection and the configuration it came from.
pub struct Session {
    /// The client.
    pub linear: Linear,
    /// What was resolved before the key.
    pub config: Config,
}

impl Session {
    fn open(config: &Config) -> Result<Session, Error> {
        Ok(Session {
            linear: Linear::connect(config)?,
            config: config.clone(),
        })
    }

    fn teams(&self) -> Result<Vec<Value>, Error> {
        let data = self.linear.run(&ops::TEAMS, json!({ "first": PAGE_CAP }))?;
        Ok(nodes(&data, "teams"))
    }

    /// The team to work in: `--team`, else `MUSTER_TEAM`, else the only one.
    fn team(&self) -> Result<(Value, TeamSource), Error> {
        let teams = self.teams()?;
        match &self.config.team {
            Some((key, source)) => match find_by(&teams, "key", key) {
                Some(team) => Ok((team.clone(), *source)),
                None => Err(Error::with_hint(
                    ErrorCode::NotFound,
                    format!(
                        "{} names the team {key}, and the workspace has no such team (teams: {})",
                        source.name(),
                        names(&teams, "key")
                    ),
                    "name a team key from `muster team list`",
                )),
            },
            None => match teams.as_slice() {
                [only] => Ok((only.clone(), TeamSource::Inferred)),
                [] => Err(Error::new(ErrorCode::NotFound, "the workspace has no team")),
                _ => Err(Error::with_hint(
                    ErrorCode::Usage,
                    format!(
                        "the workspace has several teams ({}) and none was named",
                        names(&teams, "key")
                    ),
                    "pass --team <KEY>, or set MUSTER_TEAM (docs/reference/commands.md#environment)",
                )),
            },
        }
    }

    fn team_key(&self) -> Result<String, Error> {
        let (team, _) = self.team()?;
        need_str(&team, "key", "Teams")
    }

    /// The key a team command works on: the key given, else the usual rule.
    fn team_key_of(&self, key: &Option<String>) -> Result<String, Error> {
        match key.as_deref().map(str::trim) {
            Some(key) if !key.is_empty() => Ok(key.to_string()),
            _ => self.team_key(),
        }
    }

    /// The team with its memberships, `first` of them, in one `TeamMembers` request:
    /// the team's `id` and `key`, and `memberships` as Linear pages it (ADR 0014,
    /// group 3).
    fn team_members(&self, key: &str, first: usize) -> Result<Value, Error> {
        let data = self
            .linear
            .run(&ops::TEAM_MEMBERS, json!({ "key": key, "first": first }))?;
        match nodes(&data, "teams").into_iter().next() {
            Some(team) => Ok(team),
            None => Err(Error::with_hint(
                ErrorCode::NotFound,
                format!("no team with key {key:?}"),
                "`muster team list` shows the keys",
            )),
        }
    }

    /// One team with every setting, by key, in one `TeamByKey` request; an archived
    /// team only when asked, as Linear lists them (ADR 0014, group 2).
    fn team_full(&self, key: &str, include_archived: bool) -> Result<Value, Error> {
        let data = self.linear.run(
            &ops::TEAM_BY_KEY,
            json!({ "key": key, "includeArchived": include_archived }),
        )?;
        match nodes(&data, "teams").into_iter().next() {
            Some(team) => Ok(team),
            None => Err(Error::with_hint(
                ErrorCode::NotFound,
                format!("no team with key {key:?}"),
                "`muster team list` shows the keys; a deleted team is reached by `team unarchive <KEY>` within Linear's grace period",
            )),
        }
    }

    fn states(&self, team_key: &str) -> Result<Vec<Value>, Error> {
        let data = self.linear.run(
            &ops::WORKFLOW_STATES,
            json!({ "first": PAGE_CAP, "filter": Filter::new().team(team_key).build() }),
        )?;
        let mut states = nodes(&data, "workflowStates");
        states.sort_by(|a, b| {
            at(a, "position")
                .as_f64()
                .unwrap_or(0.0)
                .total_cmp(&at(b, "position").as_f64().unwrap_or(0.0))
        });
        Ok(states)
    }

    fn state_id(&self, team_key: &str, name: &str) -> Result<String, Error> {
        let states = self.states(team_key)?;
        match find_by(&states, "name", name) {
            Some(state) => need_str(state, "id", "WorkflowStates"),
            None => Err(Error::with_hint(
                ErrorCode::NotFound,
                format!(
                    "team {team_key} has no state named {name:?} (states: {})",
                    names(&states, "name")
                ),
                "name one of the team's states; `muster state list` shows them with their types",
            )),
        }
    }

    /// The first state of a type, by position: `completed` for close, `canceled` for cancel.
    fn state_of_type(&self, team_key: &str, kind: &str) -> Result<String, Error> {
        let states = self.states(team_key)?;
        match find_by(&states, "type", kind) {
            Some(state) => need_str(state, "id", "WorkflowStates"),
            None => Err(Error::with_hint(
                ErrorCode::NotFound,
                format!(
                    "team {team_key} has no state of type {kind} (states: {})",
                    names(&states, "name")
                ),
                "name the state to move to with --state",
            )),
        }
    }

    fn labels(&self, team_key: Option<&str>) -> Result<Vec<Value>, Error> {
        let filter = match team_key {
            Some(key) => {
                json!({ "or": [ { "team": { "key": { "eq": key } } }, { "team": { "null": true } } ] })
            }
            None => Value::Null,
        };
        let data = self.linear.run(
            &ops::ISSUE_LABELS,
            json!({ "first": PAGE_CAP, "filter": filter }),
        )?;
        Ok(nodes(&data, "issueLabels"))
    }

    /// One label by name among the team's and the workspace's, as `--label` resolves
    /// one: the team's own first, then the workspace's (ADR 0014, group 6).
    fn label_named(&self, team_key: &str, name: &str) -> Result<Value, Error> {
        let labels = self.labels(Some(team_key))?;
        let own = labels.iter().find(|l| {
            !at(l, "team").is_null()
                && str_at(l, "name").is_some_and(|n| n.eq_ignore_ascii_case(name))
        });
        match own.or_else(|| find_by(&labels, "name", name)) {
            Some(label) => Ok(label.clone()),
            None => Err(Error::with_hint(
                ErrorCode::NotFound,
                format!(
                    "no label named {name:?} for team {team_key} or the workspace (labels: {})",
                    names(&labels, "name")
                ),
                "name one from `muster label list`",
            )),
        }
    }

    fn label_ids(&self, team_key: &str, wanted: &[String]) -> Result<Vec<String>, Error> {
        if wanted.is_empty() {
            return Ok(Vec::new());
        }
        let labels = self.labels(Some(team_key))?;
        wanted
            .iter()
            .map(|name| match find_by(&labels, "name", name) {
                Some(label) => need_str(label, "id", "IssueLabels"),
                None => Err(Error::with_hint(
                    ErrorCode::NotFound,
                    format!(
                        "no label named {name:?} for team {team_key} or the workspace (labels: {})",
                        names(&labels, "name")
                    ),
                    "name one from `muster label list`",
                )),
            })
            .collect()
    }

    /// The first project of the workspace with this name, case-insensitively, in one
    /// `Projects` request; `None` when there is none. What `--project <NAME>` resolves
    /// and what `project create`'s duplicate guard checks, from one place.
    fn find_project(&self, name: &str) -> Result<Option<Value>, Error> {
        let data = self.linear.run(
            &ops::PROJECTS,
            json!({ "first": 1, "filter": { "name": { "eqIgnoreCase": name } }, "teams": TEAMS_PER_PROJECT }),
        )?;
        Ok(nodes(&data, "projects").into_iter().next())
    }

    /// One project with every detail field, by name, in one `ProjectByName` request;
    /// a trashed or archived project only when asked (ADR 0014, group 8).
    fn project_full(&self, name: &str, include_archived: bool) -> Result<Value, Error> {
        let data = self.linear.run(
            &ops::PROJECT_BY_NAME,
            json!({ "name": name, "includeArchived": include_archived, "teams": TEAMS_PER_PROJECT, "members": MEMBERS_PER_PROJECT, "milestones": MILESTONES_PER_PROJECT }),
        )?;
        match nodes(&data, "projects").into_iter().next() {
            Some(project) => Ok(project),
            None => Err(Error::with_hint(
                ErrorCode::NotFound,
                format!("no project named {name:?}"),
                "name one from `muster project list`; a trashed project is reached by `project unarchive <NAME>` within Linear's thirty days",
            )),
        }
    }

    /// The workspace's project statuses, up to `PAGE_CAP`, by position; the archived
    /// ones only when asked.
    fn project_statuses(&self, include_archived: bool) -> Result<Vec<Value>, Error> {
        let data = self.linear.run(
            &ops::PROJECT_STATUSES,
            json!({ "first": PAGE_CAP, "includeArchived": include_archived }),
        )?;
        let mut statuses = nodes(&data, "projectStatuses");
        statuses.sort_by(|a, b| {
            at(a, "position")
                .as_f64()
                .unwrap_or(0.0)
                .total_cmp(&at(b, "position").as_f64().unwrap_or(0.0))
        });
        Ok(statuses)
    }

    /// One project status by name, the archived ones included when asked.
    fn project_status(&self, name: &str, include_archived: bool) -> Result<Value, Error> {
        let statuses = self.project_statuses(include_archived)?;
        match find_by(&statuses, "name", name) {
            Some(status) => Ok(status.clone()),
            None => Err(Error::with_hint(
                ErrorCode::NotFound,
                format!(
                    "the workspace has no project status named {name:?} (statuses: {})",
                    names(&statuses, "name")
                ),
                "name one from `muster project-status list`",
            )),
        }
    }

    /// The id of one live project status by name, what `project update --status`
    /// sends: [`Session::project_status`] and its `id`.
    fn project_status_id(&self, name: &str) -> Result<String, Error> {
        need_str(&self.project_status(name, false)?, "id", "ProjectStatuses")
    }

    /// A project's milestones, `first` of them, in one `Milestones` request.
    fn milestones(&self, project: &str, first: usize) -> Result<Value, Error> {
        self.linear.run(
            &ops::MILESTONES,
            json!({ "project": project, "first": first }),
        )
    }

    /// One milestone by name in a project, among up to `PAGE_CAP` (ADR 0014, group
    /// 10); the first match, as `--project` resolves a name.
    fn milestone_named(&self, project: &str, name: &str) -> Result<Value, Error> {
        let data = self.milestones(project, PAGE_CAP)?;
        let stones = nodes(&data, "projectMilestones");
        match find_by(&stones, "name", name) {
            Some(stone) => Ok(stone.clone()),
            None => Err(Error::with_hint(
                ErrorCode::NotFound,
                format!(
                    "project {project:?} has no milestone named {name:?} (milestones: {})",
                    names(&stones, "name")
                ),
                "name one from `muster milestone list --project <NAME>`",
            )),
        }
    }

    /// One initiative with its content and its projects, by name, in one request;
    /// an archived one only when asked (ADR 0014, group 12).
    fn initiative_full(&self, name: &str, include_archived: bool) -> Result<Value, Error> {
        let data = self.linear.run(
            &ops::INITIATIVE_BY_NAME,
            json!({ "name": name, "includeArchived": include_archived, "projects": PROJECTS_PER_INITIATIVE }),
        )?;
        match nodes(&data, "initiatives").into_iter().next() {
            Some(initiative) => Ok(initiative),
            None => Err(Error::with_hint(
                ErrorCode::NotFound,
                format!("no initiative named {name:?}"),
                "name one from `muster initiative list`; an archived one is reached by `initiative unarchive <NAME>`",
            )),
        }
    }

    /// The team's cycles that pass `filter`, newest first as Linear orders them, at
    /// most `first` (ADR 0014, group 13).
    fn cycles(
        &self,
        team_key: &str,
        filter: Map<String, Value>,
        first: usize,
    ) -> Result<Value, Error> {
        let mut filter = filter;
        filter.insert("team".into(), json!({ "key": { "eq": team_key } }));
        self.linear.run(
            &ops::CYCLES,
            json!({ "first": first, "filter": Value::Object(filter) }),
        )
    }

    /// One cycle of the team from `--cycle`'s or a command's word: a number, `active`
    /// or `next`; exit 2 for another word, exit 4 when the team has no such cycle.
    fn cycle_ref(&self, team_key: &str, word: &str) -> Result<Value, Error> {
        let (field, value, what) = match word.trim() {
            "active" => (
                "isActive",
                json!({ "eq": true }),
                "active cycle".to_string(),
            ),
            "next" => ("isNext", json!({ "eq": true }), "next cycle".to_string()),
            text => match text.parse::<u64>() {
                Ok(number) => (
                    "number",
                    json!({ "eq": number as f64 }),
                    format!("cycle {number}"),
                ),
                Err(_) => {
                    return Err(Error::with_hint(
                        ErrorCode::Usage,
                        format!("a cycle is a number, `active` or `next`, not {word:?}"),
                        "`muster cycle list` prints the team's cycles with their numbers",
                    ));
                }
            },
        };
        let mut filter = Map::new();
        filter.insert(field.into(), value);
        let data = self.cycles(team_key, filter, 1)?;
        match nodes(&data, "cycles").into_iter().next() {
            Some(cycle) => Ok(cycle),
            None => Err(Error::with_hint(
                ErrorCode::NotFound,
                format!("team {team_key} has no {what}"),
                "`muster cycle list` prints the team's cycles; `team update --cycles-enabled true` turns them on",
            )),
        }
    }

    /// One cycle with its issues, by number, in one request, the issues bounded by
    /// `ISSUES_PER_CYCLE`.
    fn cycle_full(&self, team_key: &str, number: u64) -> Result<Value, Error> {
        let data = self.linear.run(
            &ops::CYCLE_BY_NUMBER,
            json!({ "key": team_key, "number": number as f64, "issues": ISSUES_PER_CYCLE }),
        )?;
        match nodes(&data, "cycles").into_iter().next() {
            Some(cycle) => Ok(cycle),
            None => Err(Error::with_hint(
                ErrorCode::NotFound,
                format!("team {team_key} has no cycle {number}"),
                "`muster cycle list` prints the team's cycles with their numbers",
            )),
        }
    }

    /// One document with its content, by title, in one request; a trashed one only
    /// when asked (ADR 0014, group 14).
    fn document_full(&self, title: &str, include_archived: bool) -> Result<Value, Error> {
        let data = self.linear.run(
            &ops::DOCUMENT_BY_TITLE,
            json!({ "title": title, "includeArchived": include_archived }),
        )?;
        match nodes(&data, "documents").into_iter().next() {
            Some(document) => Ok(document),
            None => Err(Error::with_hint(
                ErrorCode::NotFound,
                format!("no document titled {title:?}"),
                "name one from `muster document list`; a trashed one is reached by `document unarchive <TITLE>`",
            )),
        }
    }

    /// An issue's id from its identifier, for a document's `issueId`.
    fn issue_id(&self, identifier: &str) -> Result<String, Error> {
        let issue = self.issue(identifier, 1, 1, 1)?;
        need_str(&issue, "id", "IssueById")
    }

    /// The templates named so in the team's scope, its own and the workspace's, in one
    /// request; an archived one only when asked (ADR 0014, group 15). Two are asked
    /// for, one for each scope the filter admits; a scope that `--allow-duplicate` gave
    /// several of one name is read as Linear lists them, the first two.
    fn templates_named(
        &self,
        team_key: &str,
        name: &str,
        include_archived: bool,
    ) -> Result<Vec<Value>, Error> {
        let data = self.linear.run(
            &ops::TEMPLATE_BY_NAME,
            json!({ "key": team_key, "name": name, "includeArchived": include_archived }),
        )?;
        Ok(data["templateSearch"]
            .as_array()
            .cloned()
            .unwrap_or_default())
    }

    /// One template by name among the team's and the workspace's, as `--label`
    /// resolves a label: the team's own first, then the workspace's (ADR 0014,
    /// group 15). Another team's template is not in the scope and is not found.
    fn template_named(
        &self,
        team_key: &str,
        name: &str,
        include_archived: bool,
    ) -> Result<Value, Error> {
        let found = self.templates_named(team_key, name, include_archived)?;
        let own = found.iter().find(|t| !at(t, "team").is_null());
        match own.or_else(|| found.first()) {
            Some(template) => Ok(template.clone()),
            None => Err(Error::with_hint(
                ErrorCode::NotFound,
                format!("no template named {name:?} for team {team_key} or the workspace"),
                "name one from `muster template list`",
            )),
        }
    }

    /// The workspace's webhooks, up to `PAGE_CAP`, and whether Linear has more; the
    /// list takes no filter (ADR 0014, group 16).
    fn webhooks(&self) -> Result<(Vec<Value>, bool), Error> {
        let data = self
            .linear
            .run(&ops::WEBHOOKS, json!({ "first": PAGE_CAP }))?;
        Ok((nodes(&data, "webhooks"), has_next_page(&data, "webhooks")))
    }

    /// One webhook from a word: its label (case aside), else its URL, else its id;
    /// exit 4 when none matches, exit 6 when the list was cut and none matched.
    fn webhook_ref(&self, word: &str) -> Result<Value, Error> {
        let (hooks, cut) = self.webhooks()?;
        let found = hooks
            .iter()
            .find(|h| str_at(h, "label").is_some_and(|l| l.eq_ignore_ascii_case(word)))
            .or_else(|| hooks.iter().find(|h| str_at(h, "url") == Some(word)))
            .or_else(|| hooks.iter().find(|h| str_at(h, "id") == Some(word)));
        match found {
            Some(hook) => Ok(hook.clone()),
            None if cut => Err(Error::with_hint(
                ErrorCode::Refused,
                format!(
                    "the workspace has more than PAGE_CAP ({PAGE_CAP}) webhooks and none of the first {PAGE_CAP} is labelled, at or with id {word:?}"
                ),
                "Linear's list of webhooks takes no filter; the rest are reached in the browser; PAGE_CAP is the bound",
            )
            .at("bounds")),
            None => Err(Error::with_hint(
                ErrorCode::NotFound,
                format!("no webhook labelled, at or with id {word:?}"),
                "`muster webhook list` prints the workspace's webhooks with their labels, URLs and ids",
            )),
        }
    }

    fn project_id(&self, name: &str) -> Result<String, Error> {
        match self.find_project(name)? {
            Some(project) => need_str(&project, "id", "Projects"),
            None => Err(Error::with_hint(
                ErrorCode::NotFound,
                format!("no project named {name:?}"),
                "name one from `muster project list`",
            )),
        }
    }

    /// `me`, an email address, or a name (`name` or `displayName`, case-insensitively).
    fn user_id(&self, who: &str) -> Result<String, Error> {
        if who.eq_ignore_ascii_case("me") {
            let data = self.linear.run(&ops::VIEWER, json!({}))?;
            return need_str(&data, "viewer.id", "Viewer");
        }
        let filter = if who.contains('@') {
            json!({ "email": { "eqIgnoreCase": who } })
        } else {
            json!({ "or": [ { "name": { "eqIgnoreCase": who } }, { "displayName": { "eqIgnoreCase": who } } ] })
        };
        // Suspended users included, so that `user unsuspend` and `user get` find
        // them; an assignment to one is Linear's refusal.
        let data = self.linear.run(
            &ops::USERS,
            json!({ "first": 2, "filter": filter, "includeDisabled": true }),
        )?;
        let users = nodes(&data, "users");
        match users.as_slice() {
            [user] => need_str(user, "id", "Users"),
            [] => Err(Error::with_hint(
                ErrorCode::NotFound,
                format!("no user named {who:?}"),
                "`me`, an email address, or a name from `muster user list`",
            )),
            _ => Err(Error::with_hint(
                ErrorCode::Usage,
                format!("{who:?} names more than one user"),
                "name the user by email address",
            )),
        }
    }

    /// One issue's fields, without its comments, attachments or sub-issues: what a
    /// `--parent` needs, in one `Issues` request (two when the reference is a bare
    /// number, which needs the team).
    fn issue_brief(&self, reference: &str) -> Result<Value, Error> {
        let reference = reference.trim();
        let clause = match issue_clause(reference, None)? {
            Some(clause) => clause,
            None => {
                let team_key = self.team_key()?;
                issue_clause(reference, Some(&team_key))?.unwrap_or(Value::Null)
            }
        };
        let data = self.linear.run(
            &ops::ISSUES,
            json!({ "first": 1, "filter": clause, "orderBy": "createdAt" }),
        )?;
        match nodes(&data, "issues").into_iter().next() {
            Some(issue) => Ok(issue),
            None => Err(Error::new(
                ErrorCode::NotFound,
                format!("no issue {reference}"),
            )),
        }
    }

    /// One issue by identifier, number or id, with `comments` comments, `attachments`
    /// attachments and `children` sub-issues fetched (at least one of each; the shape is
    /// the same). An archived or trashed issue is reached by every form: Linear's `issue`
    /// query answers for one by id or identifier, and the number lookup asks `issues`
    /// with `includeArchived`, which otherwise leaves archived issues out.
    fn issue(
        &self,
        reference: &str,
        comments: usize,
        attachments: usize,
        children: usize,
    ) -> Result<Value, Error> {
        let reference = reference.trim();
        let id = if is_identifier(reference) || is_uuid(reference) {
            reference.to_string()
        } else if !reference.is_empty() && reference.chars().all(|c| c.is_ascii_digit()) {
            let team_key = self.team_key()?;
            let number: u64 = reference.parse().map_err(|_| {
                Error::new(ErrorCode::Usage, format!("{reference:?} is not a number"))
            })?;
            let filter = Filter::new()
                .team(&team_key)
                .with("number", json!({ "eq": number }))
                .build();
            // `includeArchived`, so that the number reaches an archived or trashed issue
            // as the identifier and the id do; `issue unarchive 12` depends on it.
            let data = self.linear.run(
                &ops::ISSUES,
                json!({ "first": 1, "filter": filter, "orderBy": "createdAt", "includeArchived": true }),
            )?;
            match nodes(&data, "issues").first() {
                Some(issue) => need_str(issue, "identifier", "Issues")?,
                None => {
                    return Err(Error::new(
                        ErrorCode::NotFound,
                        format!("team {team_key} has no issue number {number}"),
                    ));
                }
            }
        } else {
            return Err(bad_reference(reference));
        };
        let data = self.linear.run(
            &ops::ISSUE_BY_ID,
            json!({ "id": id, "comments": comments.max(1), "attachments": attachments.max(1), "children": children.max(1) }),
        )?;
        match data.get("issue") {
            Some(issue) if !issue.is_null() => Ok(issue.clone()),
            _ => Err(Error::new(ErrorCode::NotFound, format!("no issue {id}"))),
        }
    }
}

fn limit(n: usize) -> Result<usize, Error> {
    if n == 0 || n > PAGE_CAP {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            format!("--limit {n} is outside 1 to PAGE_CAP ({PAGE_CAP})"),
            "narrow the list with --state, --label, --assignee or --search instead of a larger page",
        )
        .at("bounds"));
    }
    Ok(n)
}

fn emit_list(out: &Output, items: Vec<Value>, cut: bool, what: &str, n: usize) {
    for item in &items {
        out.emit(item);
    }
    if cut {
        out.notice(
            "truncated",
            &format!("{what}: Linear has more than the {n} printed"),
            &format!("raise --limit up to PAGE_CAP ({PAGE_CAP}), or narrow the list (docs/reference/commands.md#bounds)"),
        );
    }
}

fn whoami(session: &Session, out: &Output) -> Result<(), Error> {
    let data = session.linear.run(&ops::VIEWER, json!({}))?;
    let mut checks = Vec::new();
    let mut failure: Option<Error> = None;
    let key = match &session.linear.key_source {
        KeySource::Env => json!({ "source": "LINEAR_API_KEY" }),
        KeySource::File(path) => json!({ "source": "file", "path": path.display().to_string() }),
    };
    checks.push(
        json!({ "check": "key", "level": "ok", "message": match &session.linear.key_source {
        KeySource::Env => "the key was read from LINEAR_API_KEY".to_string(),
        KeySource::File(path) => format!("the key was read from {}", path.display()),
    } }),
    );
    if session.config.key_path_from_env && matches!(session.linear.key_source, KeySource::Env) {
        checks.push(json!({ "check": "key-file", "level": "warn", "message": "MUSTER_KEY_FILE is set and LINEAR_API_KEY is set; the variable won and the file was not read" }));
    }
    if session.config.endpoint_is_linear() {
        checks.push(
            json!({ "check": "endpoint", "level": "ok", "message": "the endpoint is Linear's" }),
        );
    } else {
        checks.push(json!({ "check": "endpoint", "level": "warn", "message": format!("the endpoint is {}, not Linear's: MUSTER_ENDPOINT is set, which only the tests and the quickstart do", session.config.endpoint) }));
    }
    let team = match session.team() {
        Ok((team, source)) => {
            checks.push(json!({ "check": "team", "level": "ok", "message": format!("the team is {} ({}), {}", str_at(&team, "key").unwrap_or("?"), str_at(&team, "name").unwrap_or("?"), match source {
                TeamSource::Inferred => "the workspace's only team".to_string(),
                other => format!("from {}", other.name()),
            }) }));
            json!({ "id": at(&team, "id"), "key": at(&team, "key"), "name": at(&team, "name"), "source": source.name() })
        }
        Err(e) if e.code == ErrorCode::Usage => {
            checks.push(json!({ "check": "team", "level": "warn", "message": format!("{}; commands that need a team will exit 2 until one is named", e.message) }));
            Value::Null
        }
        Err(e) => {
            checks.push(json!({ "check": "team", "level": "fail", "message": e.message.clone() }));
            failure = Some(e);
            Value::Null
        }
    };
    out.emit(&json!({
        "viewer": at(&data, "viewer"),
        "organization": at(&data, "organization"),
        "muster": {
            "version": VERSION,
            "endpoint": session.config.endpoint,
            "key": key,
            "team": team,
            "checks": checks,
            "docs": format!("{DOCS_URL}#muster-whoami"),
        },
    }));
    match failure {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// `workspace get`: the `organization` as Linear returns it, one line (ADR 0014, group 1).
fn workspace_get(session: &Session, out: &Output) -> Result<(), Error> {
    let data = session.linear.run(&ops::WORKSPACE, json!({}))?;
    match data.get("organization") {
        Some(organization) if !organization.is_null() => {
            out.emit(organization);
            Ok(())
        }
        _ => Err(Error::new(
            ErrorCode::Failed,
            "Linear's answer to Workspace has no organization",
        )),
    }
}

/// The input of `organizationUpdate` from `workspace update`'s flags: only what was
/// passed, so that a flag left out leaves its setting alone; `none` clears the branch
/// format; the roles go under `securitySettings` and the sign-in services under
/// `authSettings`, as Linear nests them; `all` is Linear's empty list, and the one way
/// to send it, since a value that names no service is a usage error. No flag at all is
/// a usage error too, before any request.
fn workspace_input(args: &WorkspaceUpdateArgs) -> Result<Map<String, Value>, Error> {
    let mut input = Map::new();
    if let Some(name) = &args.name {
        check_body(Text::Name, name)?;
        input.insert("name".into(), json!(name));
    }
    if let Some(key) = &args.url_key {
        input.insert("urlKey".into(), json!(key));
    }
    if let Some(url) = &args.logo_url {
        check_body(Text::Link, url)?;
        input.insert("logoUrl".into(), json!(url));
    }
    if let Some(format) = &args.git_branch_format {
        let value = if is_none(format) {
            Value::Null
        } else {
            json!(format)
        };
        input.insert("gitBranchFormat".into(), value);
    }
    for (field, value) in [
        ("gitLinkbackMessagesEnabled", args.git_linkback_messages),
        (
            "gitPublicLinkbackMessagesEnabled",
            args.git_public_linkback_messages,
        ),
        (
            "gitLinkbackDescriptionsEnabled",
            args.git_linkback_descriptions,
        ),
        ("suggestedBranchNameEnabled", args.suggested_branch_name),
        ("roadmapEnabled", args.roadmap),
        ("feedEnabled", args.feed),
        ("pullRequestTourEnabled", args.pull_request_tour),
        ("aiThreadSummariesEnabled", args.ai_thread_summaries),
        ("aiDiscussionSummariesEnabled", args.ai_discussion_summaries),
    ] {
        if let Some(value) = value {
            input.insert(field.into(), json!(value));
        }
    }
    if let Some(weeks) = args.project_update_reminder_weeks {
        input.insert("projectUpdateReminderFrequencyInWeeks".into(), json!(weeks));
    }
    if let Some(day) = args.project_update_reminders_day {
        input.insert("projectUpdateRemindersDay".into(), json!(day.linear()));
    }
    if let Some(hour) = args.project_update_reminders_hour {
        input.insert("projectUpdateRemindersHour".into(), json!(hour));
    }
    if let Some(month) = args.fiscal_year_start_month {
        input.insert("fiscalYearStartMonth".into(), json!(month));
    }
    if let Some(mode) = args.pull_request_issue_mode {
        input.insert("pullRequestIssueMode".into(), json!(mode.linear()));
    }
    if let Some(types) = &args.allowed_file_upload_content_types {
        input.insert("allowedFileUploadContentTypes".into(), json!(types));
    }
    let mut security = Map::new();
    for (field, role) in [
        ("invitationsRole", args.invitations_role),
        ("teamCreationRole", args.team_creation_role),
        ("labelManagementRole", args.label_management_role),
        ("personalApiKeysRole", args.personal_api_keys_role),
        ("adminManagementRole", args.admin_management_role),
        ("templateManagementRole", args.template_management_role),
        ("integrationCreationRole", args.integration_creation_role),
        ("importRole", args.import_role),
        ("apiSettingsRole", args.api_settings_role),
        ("workspaceInitiativesRole", args.workspace_initiatives_role),
    ] {
        if let Some(role) = role {
            security.insert(field.into(), json!(role.linear()));
        }
    }
    if !security.is_empty() {
        input.insert("securitySettings".into(), Value::Object(security));
    }
    let mut auth = Map::new();
    if let Some(services) = &args.allowed_auth_services {
        let list: Vec<&str> = if services.trim().eq_ignore_ascii_case("all") {
            Vec::new()
        } else {
            let named: Vec<&str> = services
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            // An empty value, or commas alone, would also be the empty list, which
            // Linear reads as every service allowed: a security setting opened by an
            // empty variable. Exit 2 rather than guess (CLAUDE.md, When in doubt).
            if named.is_empty() {
                return Err(Error::with_hint(
                    ErrorCode::Usage,
                    "--allowed-auth-services names no service",
                    "name the services to allow, comma-separated (google,saml); `all`, which lets every service sign in, is the one spelling of Linear's empty list",
                ));
            }
            named
        };
        auth.insert("allowedAuthServices".into(), json!(list));
    }
    if let Some(hide) = args.hide_non_primary_organizations {
        auth.insert("hideNonPrimaryOrganizations".into(), json!(hide));
    }
    if !auth.is_empty() {
        input.insert("authSettings".into(), Value::Object(auth));
    }
    if input.is_empty() {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            "nothing to change",
            "pass at least one setting; `muster workspace update --help` lists them, and `muster workspace get` shows their values",
        ));
    }
    Ok(input)
}

/// `workspace update`: `organizationUpdate` with the input above, and the workspace as
/// Linear returns it afterwards (ADR 0014, group 1). Most settings need a workspace
/// admin; Linear's refusal is exit 6 with its message.
fn workspace_update(
    session: &Session,
    args: &WorkspaceUpdateArgs,
    out: &Output,
) -> Result<(), Error> {
    let input = workspace_input(args)?;
    let data = session.linear.run(
        &ops::WORKSPACE_UPDATE,
        json!({ "input": Value::Object(input) }),
    )?;
    out.emit(&payload(&data, "organizationUpdate", "organization")?);
    Ok(())
}

/// Refuse a `--position` that is not finite, before any request. `clap` parses the
/// flag with `f64::from_str`, which accepts `nan`, `inf`, `-inf` and `1e400` (which
/// overflows to infinity), and `serde_json` writes a number that is not finite as
/// `null`: a value the caller never typed, which Linear would read as its own default.
/// Exit 2 rather than guess (CLAUDE.md, When in doubt). `list` is the command that
/// shows the positions the number sits among: `muster state list` for a state,
/// `muster project-status list` for a project status.
fn check_position(position: f64, list: &str) -> Result<(), Error> {
    if position.is_finite() {
        return Ok(());
    }
    Err(Error::with_hint(
        ErrorCode::Usage,
        format!("--position {position} is not a finite number"),
        format!("pass a finite number, such as 2.5; `{list}` shows each one's position"),
    ))
}

/// The input of `workflowStateCreate` but its `teamId`: the flags, checked before any
/// request.
fn state_input(args: &StateCreateArgs) -> Result<Map<String, Value>, Error> {
    check_body(Text::Name, &args.name)?;
    let mut input = Map::new();
    input.insert("name".into(), json!(args.name));
    input.insert("type".into(), json!(args.r#type.linear()));
    input.insert("color".into(), json!(args.color));
    if let Some(description) = &args.description {
        check_body(Text::Body, description)?;
        input.insert("description".into(), json!(description));
    }
    if let Some(position) = args.position {
        check_position(position, "muster state list")?;
        input.insert("position".into(), json!(position));
    }
    Ok(input)
}

fn state_create(session: &Session, args: &StateCreateArgs, out: &Output) -> Result<(), Error> {
    let mut input = state_input(args)?;
    let (team, _) = session.team()?;
    let team_key = need_str(&team, "key", "Teams")?;
    // The guard: `--state <NAME>` resolves a name to one state of the team.
    if !args.allow_duplicate
        && let Some(existing) = find_by(&session.states(&team_key)?, "name", &args.name)
    {
        return Err(Error::with_hint(
            ErrorCode::Refused,
            format!(
                "team {team_key} has a state named {:?} already, of type {}",
                str_at(existing, "name").unwrap_or(&args.name),
                str_at(existing, "type").unwrap_or("?")
            ),
            "use that state, or pass --allow-duplicate if a second one is wanted",
        )
        .at("muster-state-create"));
    }
    input.insert("teamId".into(), json!(need_str(&team, "id", "Teams")?));
    let data = session
        .linear
        .run(&ops::STATE_CREATE, json!({ "input": Value::Object(input) }))?;
    out.emit(&payload(&data, "workflowStateCreate", "workflowState")?);
    Ok(())
}

/// The input of `workflowStateUpdate`: the flags, checked before any request; no flag
/// is a usage error, since Linear's input has no field for the type and nothing else
/// would change. The state is resolved by the handler, after these checks, so that a
/// refusal here sends nothing.
fn state_update_input(args: &StateUpdateArgs) -> Result<Map<String, Value>, Error> {
    let mut input = Map::new();
    if let Some(name) = &args.name {
        check_body(Text::Name, name)?;
        input.insert("name".into(), json!(name));
    }
    if let Some(color) = &args.color {
        input.insert("color".into(), json!(color));
    }
    if let Some(description) = &args.description {
        check_body(Text::Body, description)?;
        input.insert("description".into(), json!(description));
    }
    if let Some(position) = args.position {
        check_position(position, "muster state list")?;
        input.insert("position".into(), json!(position));
    }
    if input.is_empty() {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            "nothing to change",
            "pass at least one of --name, --color, --description, --position; a state's type cannot change",
        ));
    }
    Ok(input)
}

fn state_update(session: &Session, args: &StateUpdateArgs, out: &Output) -> Result<(), Error> {
    let input = state_update_input(args)?;
    let team_key = session.team_key()?;
    let id = session.state_id(&team_key, &args.state)?;
    let data = session.linear.run(
        &ops::STATE_UPDATE,
        json!({ "id": id, "input": Value::Object(input) }),
    )?;
    out.emit(&payload(&data, "workflowStateUpdate", "workflowState")?);
    Ok(())
}

fn state_archive(session: &Session, args: &StateRefArgs, out: &Output) -> Result<(), Error> {
    let team_key = session.team_key()?;
    let id = session.state_id(&team_key, &args.name)?;
    let data = session
        .linear
        .run(&ops::STATE_ARCHIVE, json!({ "id": id }))?;
    out.emit(&payload(&data, "workflowStateArchive", "entity")?);
    Ok(())
}

/// The input of `issueLabelCreate` but its `teamId` and `parentId`: the flags, checked
/// before any request.
fn label_input(args: &LabelCreateArgs) -> Result<Map<String, Value>, Error> {
    check_body(Text::Name, &args.name)?;
    let mut input = Map::new();
    input.insert("name".into(), json!(args.name));
    if let Some(color) = &args.color {
        input.insert("color".into(), json!(color));
    }
    if let Some(description) = &args.description {
        check_body(Text::Body, description)?;
        input.insert("description".into(), json!(description));
    }
    if args.group {
        input.insert("isGroup".into(), json!(true));
    }
    Ok(input)
}

fn label_create(session: &Session, args: &LabelCreateArgs, out: &Output) -> Result<(), Error> {
    let mut input = label_input(args)?;
    let (team, _) = session.team()?;
    let team_key = need_str(&team, "key", "Teams")?;
    // The guard is on the name in the label's own scope, the team's labels or the
    // workspace's, since `--label <NAME>` resolves a name to one label.
    if !args.allow_duplicate {
        let labels = session.labels(Some(&team_key))?;
        let same_scope = labels.iter().find(|l| {
            str_at(l, "name").is_some_and(|n| n.eq_ignore_ascii_case(&args.name))
                && at(l, "team").is_null() == args.workspace
        });
        if let Some(existing) = same_scope {
            let scope = if args.workspace {
                "the workspace".to_string()
            } else {
                format!("team {team_key}")
            };
            return Err(Error::with_hint(
                ErrorCode::Refused,
                format!(
                    "a label named {:?} exists for {scope}",
                    str_at(existing, "name").unwrap_or(&args.name)
                ),
                "use that label, or pass --allow-duplicate if a second one is wanted",
            )
            .at("muster-label-create"));
        }
    }
    if !args.workspace {
        input.insert("teamId".into(), json!(need_str(&team, "id", "Teams")?));
    }
    if let Some(parent) = &args.parent {
        let group = session.label_named(&team_key, parent)?;
        input.insert(
            "parentId".into(),
            json!(need_str(&group, "id", "IssueLabels")?),
        );
    }
    let data = session
        .linear
        .run(&ops::LABEL_CREATE, json!({ "input": Value::Object(input) }))?;
    out.emit(&payload(&data, "issueLabelCreate", "issueLabel")?);
    Ok(())
}

fn label_update(session: &Session, args: &LabelUpdateArgs, out: &Output) -> Result<(), Error> {
    let mut input = Map::new();
    if let Some(name) = &args.name {
        check_body(Text::Name, name)?;
        input.insert("name".into(), json!(name));
    }
    if let Some(color) = &args.color {
        input.insert("color".into(), json!(color));
    }
    if let Some(description) = &args.description {
        check_body(Text::Body, description)?;
        input.insert("description".into(), json!(description));
    }
    let team_key = session.team_key()?;
    let label = session.label_named(&team_key, &args.label)?;
    let id = need_str(&label, "id", "IssueLabels")?;
    if let Some(parent) = &args.parent {
        let value = if is_none(parent) {
            Value::Null
        } else {
            let group = session.label_named(&team_key, parent)?;
            let group_id = need_str(&group, "id", "IssueLabels")?;
            if group_id == id {
                return Err(Error::with_hint(
                    ErrorCode::Usage,
                    format!("{:?} cannot be its own group", args.label),
                    "name another label, or `none` to take it out of its group",
                ));
            }
            json!(group_id)
        };
        input.insert("parentId".into(), value);
    }
    if input.is_empty() {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            "nothing to change",
            "pass at least one of --name, --color, --description, --parent",
        ));
    }
    let data = session.linear.run(
        &ops::LABEL_UPDATE,
        json!({ "id": id, "input": Value::Object(input) }),
    )?;
    out.emit(&payload(&data, "issueLabelUpdate", "issueLabel")?);
    Ok(())
}

/// `label retire` and `label restore`: one mutation with the label's id, and the
/// label as Linear returns it.
fn label_lifecycle(
    session: &Session,
    args: &LabelRefArgs,
    op: &ops::Operation,
    field: &str,
    out: &Output,
) -> Result<(), Error> {
    let team_key = session.team_key()?;
    let label = session.label_named(&team_key, &args.name)?;
    let id = need_str(&label, "id", "IssueLabels")?;
    let data = session.linear.run(op, json!({ "id": id }))?;
    out.emit(&payload(&data, field, "issueLabel")?);
    Ok(())
}

fn label_delete(session: &Session, args: &LabelRefArgs, out: &Output) -> Result<(), Error> {
    let team_key = session.team_key()?;
    let label = session.label_named(&team_key, &args.name)?;
    let id = need_str(&label, "id", "IssueLabels")?;
    let data = session
        .linear
        .run(&ops::LABEL_DELETE, json!({ "id": id }))?;
    out.emit(&delete_payload(&data, "issueLabelDelete")?);
    Ok(())
}

/// One user with every `UserFields` field, by id, in one `UserById` request; `who` is
/// the name the caller gave, for the message when Linear has no such user.
fn user_by_id(session: &Session, id: &str, who: &str) -> Result<Value, Error> {
    let data = session.linear.run(&ops::USER_BY_ID, json!({ "id": id }))?;
    match data.get("user") {
        Some(user) if !user.is_null() => Ok(user.clone()),
        _ => Err(Error::new(ErrorCode::NotFound, format!("no user {who:?}"))),
    }
}

/// One user by `me`, email or name: the resolution, then [`user_by_id`] (ADR 0014,
/// group 4).
fn user_full(session: &Session, who: &str) -> Result<Value, Error> {
    let id = session.user_id(who)?;
    user_by_id(session, &id, who)
}

/// The user read back after a mutation that landed, by the id already resolved, with
/// Linear's `success` already on stdout. A failure here keeps the read-back's own code
/// (7 for a dropped query, 5 for the rate limit), but its message and hint say that
/// the mutation landed and that `user get` prints the result: the code's general
/// words, "nothing changed in Linear", would be a lie here (invariant 4).
fn user_read_back(session: &Session, id: &str, who: &str, field: &str) -> Result<Value, Error> {
    user_by_id(session, id, who).map_err(|error| Error {
        message: format!(
            "{field} succeeded in Linear and its success line is on stdout, but the user could not be read back: {}",
            error.message
        ),
        hint: format!(
            "`muster user get {who:?}` prints the user as Linear has them now; the mutation needs no repeating"
        ),
        ..error
    })
}

fn user_get(session: &Session, args: &UserRefArgs, out: &Output) -> Result<(), Error> {
    out.emit(&user_full(session, &args.who)?);
    Ok(())
}

/// The input of `userUpdate` from `user update`'s flags: only what was passed, the
/// texts bounded, `none` keeping a status for good.
fn user_input(args: &UserUpdateArgs) -> Result<Map<String, Value>, Error> {
    let mut input = Map::new();
    for (field, value, what) in [
        ("name", &args.name, Text::Name),
        ("displayName", &args.display_name, Text::Name),
        ("title", &args.title, Text::Title),
        ("description", &args.description, Text::Body),
        ("statusEmoji", &args.status_emoji, Text::Name),
        ("statusLabel", &args.status_label, Text::Title),
        ("avatarUrl", &args.avatar_url, Text::Link),
    ] {
        if let Some(value) = value {
            check_body(what, value)?;
            input.insert(field.into(), json!(value));
        }
    }
    if let Some(timezone) = &args.timezone {
        input.insert("timezone".into(), json!(timezone));
    }
    if let Some(until) = &args.status_until {
        let value = if is_none(until) {
            Value::Null
        } else {
            json!(until)
        };
        input.insert("statusUntilAt".into(), value);
    }
    if input.is_empty() {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            "nothing to change",
            "pass at least one of --name, --display-name, --title, --description, --timezone, --status-emoji, --status-label, --status-until, --avatar-url",
        ));
    }
    Ok(input)
}

fn user_update(session: &Session, args: &UserUpdateArgs, out: &Output) -> Result<(), Error> {
    let input = user_input(args)?;
    let id = session.user_id(&args.who)?;
    let data = session.linear.run(
        &ops::USER_UPDATE,
        json!({ "id": id, "input": Value::Object(input) }),
    )?;
    out.emit(&payload(&data, "userUpdate", "user")?);
    Ok(())
}

/// The `success` of a `UserAdminPayload`, which carries nothing else, as one line.
fn success_payload(data: &Value, field: &str) -> Result<Value, Error> {
    match at(data, &format!("{field}.success")).as_bool() {
        Some(true) => Ok(at(data, field).clone()),
        Some(false) => Err(Error::new(
            ErrorCode::Refused,
            format!("Linear answered {field} with success false"),
        )),
        None => Err(Error::new(
            ErrorCode::Failed,
            format!("Linear's answer to {field} has no success"),
        )),
    }
}

/// `user change-role`: Linear's `success`, then the user read back by the id already
/// resolved, so that the new role is on the output (two lines, as `issue close` prints
/// two).
fn user_change_role(session: &Session, args: &UserRoleArgs, out: &Output) -> Result<(), Error> {
    let id = session.user_id(&args.who)?;
    let data = session.linear.run(
        &ops::USER_CHANGE_ROLE,
        json!({ "id": id, "role": args.role.linear() }),
    )?;
    out.emit(&success_payload(&data, "userChangeRole")?);
    out.emit(&user_read_back(session, &id, &args.who, "userChangeRole")?);
    Ok(())
}

/// `user suspend`, `user unsuspend` and `user revoke-sessions`: one mutation with the
/// user's id, Linear's `success`, then the user read back by that id.
fn user_admin(
    session: &Session,
    args: &UserRefArgs,
    op: &ops::Operation,
    field: &str,
    out: &Output,
) -> Result<(), Error> {
    let id = session.user_id(&args.who)?;
    let data = session.linear.run(op, json!({ "id": id }))?;
    out.emit(&success_payload(&data, field)?);
    out.emit(&user_read_back(session, &id, &args.who, field)?);
    Ok(())
}

fn member_list(session: &Session, args: &ListArgs, out: &Output) -> Result<(), Error> {
    let n = limit(args.limit)?;
    let key = session.team_key()?;
    let team = session.team_members(&key, n)?;
    emit_list(
        out,
        nodes(&team, "memberships"),
        has_next_page(&team, "memberships"),
        "members",
        n,
    );
    Ok(())
}

fn member_add(session: &Session, args: &MemberArgs, out: &Output) -> Result<(), Error> {
    let key = session.team_key()?;
    let user_id = session.user_id(&args.user)?;
    let team_id = need_str(&session.team_members(&key, 1)?, "id", "TeamMembers")?;
    let data = session.linear.run(
        &ops::MEMBERSHIP_CREATE,
        json!({ "input": { "userId": user_id, "teamId": team_id } }),
    )?;
    out.emit(&payload(&data, "teamMembershipCreate", "teamMembership")?);
    Ok(())
}

/// `member remove`: the membership is resolved from the team's list, one page of
/// `PAGE_CAP`, before the one mutation. A user who is not in a page that holds the
/// whole team is exit 4. When the page is cut (`pageInfo.hasNextPage`), a user who
/// is not in it may be a member past the bound, so the command refuses before any
/// mutation, exit 6, a muster guard, rather than call them a non-member (invariant 1;
/// ADR 0014 settles the same case for `project update --remove-member`).
fn member_remove(session: &Session, args: &MemberArgs, out: &Output) -> Result<(), Error> {
    let key = session.team_key()?;
    let user_id = session.user_id(&args.user)?;
    let team = session.team_members(&key, PAGE_CAP)?;
    let found = nodes(&team, "memberships")
        .into_iter()
        .find(|m| str_at(m, "user.id") == Some(user_id.as_str()));
    let Some(membership) = found else {
        if has_next_page(&team, "memberships") {
            return Err(Error::with_hint(
                ErrorCode::Refused,
                format!(
                    "team {key} has more than PAGE_CAP ({PAGE_CAP}) members and {:?} is not among the first {PAGE_CAP}",
                    args.user
                ),
                "remove the member in the browser, on the team's members page in Linear; PAGE_CAP is the bound",
            )
            .at("bounds"));
        }
        return Err(Error::with_hint(
            ErrorCode::NotFound,
            format!("{:?} is not a member of team {key}", args.user),
            "`muster member list` shows the team's members",
        ));
    };
    let id = need_str(&membership, "id", "TeamMembers")?;
    let data = session
        .linear
        .run(&ops::MEMBERSHIP_DELETE, json!({ "id": id }))?;
    out.emit(&delete_payload(&data, "teamMembershipDelete")?);
    Ok(())
}

fn team_get(session: &Session, args: &TeamRefArgs, out: &Output) -> Result<(), Error> {
    let key = session.team_key_of(&args.key)?;
    out.emit(&session.team_full(&key, false)?);
    Ok(())
}

/// The input of `teamCreate` but its `parentId`: the flags, each checked before any
/// request; an empty key is a usage error, since Linear would make one from the name
/// and muster sends what it was given.
fn team_input(args: &TeamCreateArgs) -> Result<Map<String, Value>, Error> {
    check_body(Text::Name, &args.name)?;
    let mut input = Map::new();
    input.insert("name".into(), json!(args.name));
    if let Some(key) = &args.key {
        let key = key.trim();
        if key.is_empty() {
            return Err(Error::with_hint(
                ErrorCode::Usage,
                "--key must not be empty",
                "leave --key out and Linear makes a key from the name",
            ));
        }
        input.insert("key".into(), json!(key));
    }
    if let Some(description) = &args.description {
        check_body(Text::Body, description)?;
        input.insert("description".into(), json!(description));
    }
    if let Some(timezone) = &args.timezone {
        input.insert("timezone".into(), json!(timezone));
    }
    if let Some(cycles) = args.cycles_enabled {
        input.insert("cyclesEnabled".into(), json!(cycles));
    }
    Ok(input)
}

fn team_create(session: &Session, args: &TeamCreateArgs, out: &Output) -> Result<(), Error> {
    let mut input = team_input(args)?;
    // The guard is on the name: `--team` and `MUSTER_TEAM` resolve a key, which Linear
    // keeps unique itself, and a reader resolves a name.
    if !args.allow_duplicate
        && let Some(existing) = find_by(&session.teams()?, "name", &args.name)
    {
        return Err(Error::with_hint(
            ErrorCode::Refused,
            format!(
                "a team named {:?} exists: its key is {}",
                str_at(existing, "name").unwrap_or(&args.name),
                str_at(existing, "key").unwrap_or("?")
            ),
            "use that team, or pass --allow-duplicate if a second one is wanted",
        )
        .at("muster-team-create"));
    }
    if let Some(parent) = &args.parent {
        let parent = session.team_full(parent, false)?;
        input.insert(
            "parentId".into(),
            json!(need_str(&parent, "id", "TeamByKey")?),
        );
    }
    let copy = match &args.copy_settings_from {
        Some(key) => json!(need_str(
            &session.team_full(key, false)?,
            "id",
            "TeamByKey"
        )?),
        None => Value::Null,
    };
    let data = session.linear.run(
        &ops::TEAM_CREATE,
        json!({ "input": Value::Object(input), "copySettingsFromTeamId": copy }),
    )?;
    out.emit(&payload(&data, "teamCreate", "team")?);
    Ok(())
}

/// The scalar part of a `teamUpdate` input from `team update`'s flags: only what was
/// passed; `none` clears the auto-close period; the roles nest under
/// `securitySettings`. The parent and the two states are resolved by the handler,
/// after these checks, so that a refusal here sends nothing.
fn team_update_input(args: &TeamUpdateArgs) -> Result<Map<String, Value>, Error> {
    let mut input = Map::new();
    if let Some(name) = &args.name {
        check_body(Text::Name, name)?;
        input.insert("name".into(), json!(name));
    }
    if let Some(key) = &args.key {
        let key = key.trim();
        if key.is_empty() {
            return Err(Error::new(ErrorCode::Usage, "--key must not be empty"));
        }
        input.insert("key".into(), json!(key));
    }
    if let Some(description) = &args.description {
        check_body(Text::Body, description)?;
        input.insert("description".into(), json!(description));
    }
    for (field, value) in [
        ("icon", &args.icon),
        ("color", &args.color),
        ("timezone", &args.timezone),
        ("cycleEnabledStartDate", &args.cycle_enabled_start_date),
    ] {
        if let Some(value) = value {
            input.insert(field.into(), json!(value));
        }
    }
    for (field, value) in [
        ("private", args.private),
        ("cyclesEnabled", args.cycles_enabled),
        (
            "cycleIssueAutoAssignStarted",
            args.cycle_auto_assign_started,
        ),
        (
            "cycleIssueAutoAssignCompleted",
            args.cycle_auto_assign_completed,
        ),
        ("cycleLockToActive", args.cycle_lock_to_active),
        ("triageEnabled", args.triage_enabled),
        (
            "requirePriorityToLeaveTriage",
            args.require_priority_to_leave_triage,
        ),
        ("issueEstimationAllowZero", args.issue_estimation_allow_zero),
        ("issueEstimationExtended", args.issue_estimation_extended),
        ("autoCloseParentIssues", args.auto_close_parent_issues),
        ("autoCloseChildIssues", args.auto_close_child_issues),
        ("joinByDefault", args.join_by_default),
        ("allMembersCanJoin", args.all_members_can_join),
        ("initiativesEnabled", args.initiatives_enabled),
        ("groupIssueHistory", args.group_issue_history),
    ] {
        if let Some(value) = value {
            input.insert(field.into(), json!(value));
        }
    }
    if let Some(day) = args.cycle_start_day {
        input.insert("cycleStartDay".into(), json!(day));
    }
    for (field, value) in [
        ("cycleDuration", args.cycle_duration),
        ("cycleCooldownTime", args.cycle_cooldown),
        ("upcomingCycleCount", args.upcoming_cycle_count),
        ("defaultIssueEstimate", args.default_issue_estimate),
        ("autoArchivePeriod", args.auto_archive_period),
    ] {
        if let Some(value) = value {
            input.insert(field.into(), json!(value));
        }
    }
    if let Some(kind) = args.issue_estimation_type {
        input.insert("issueEstimationType".into(), json!(kind.linear()));
    }
    if let Some(period) = &args.auto_close_period {
        let value = if is_none(period) {
            Value::Null
        } else {
            // `f64::from_str` accepts `nan`, `inf` and a sign, and serde_json turns a
            // number that is not finite into `null`, the `none` sentinel: only a
            // finite, non-negative number is sent, and anything else is refused before
            // any request (CLAUDE.md, When in doubt: exit non-zero rather than guess).
            let months = period
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|months| months.is_finite() && *months >= 0.0)
                .ok_or_else(|| {
                    Error::with_hint(
                        ErrorCode::Usage,
                        format!("--auto-close-period {period:?} is not a number of months"),
                        "pass a non-negative number of months, or `none` to never close idle issues",
                    )
                })?;
            json!(months)
        };
        input.insert("autoClosePeriod".into(), value);
    }
    let mut security = Map::new();
    for (field, role) in [
        ("teamManagement", args.team_management_role),
        ("memberManagement", args.member_management_role),
        ("labelManagement", args.label_management_role),
        ("templateManagement", args.template_management_role),
        ("issueSharing", args.issue_sharing_role),
    ] {
        if let Some(role) = role {
            security.insert(field.into(), json!(role.linear()));
        }
    }
    if !security.is_empty() {
        input.insert("securitySettings".into(), Value::Object(security));
    }
    Ok(input)
}

/// `team update`: `teamUpdate` with the input above and the parent and states
/// resolved by key and name, and the team as Linear returns it afterwards (ADR 0014,
/// group 2). Most settings need a team owner or a workspace admin; Linear's refusal
/// is exit 6 with its message.
fn team_update(session: &Session, args: &TeamUpdateArgs, out: &Output) -> Result<(), Error> {
    let mut input = team_update_input(args)?;
    let team = session.team_full(&session.team_key_of(&args.key_of)?, false)?;
    let id = need_str(&team, "id", "TeamByKey")?;
    // The key as Linear spells it, not as typed: `TeamByKey` matches a key in any
    // case, but the states are filtered with `eq`, which is case-sensitive, so a
    // lowercase key would report a state that exists as matching nothing.
    let key = need_str(&team, "key", "TeamByKey")?;
    if let Some(parent) = &args.parent {
        let value = if is_none(parent) {
            Value::Null
        } else {
            let parent_id = need_str(&session.team_full(parent, false)?, "id", "TeamByKey")?;
            if parent_id == id {
                return Err(Error::with_hint(
                    ErrorCode::Usage,
                    format!("{key} cannot be its own parent"),
                    "name another team, or `none` to take it out from under its parent",
                ));
            }
            json!(parent_id)
        };
        input.insert("parentId".into(), value);
    }
    if let Some(state) = &args.default_issue_state {
        input.insert(
            "defaultIssueStateId".into(),
            json!(session.state_id(&key, state)?),
        );
    }
    if let Some(state) = &args.auto_close_state {
        input.insert(
            "autoCloseStateId".into(),
            json!(session.state_id(&key, state)?),
        );
    }
    if input.is_empty() {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            "nothing to change",
            "pass at least one setting; `muster team update --help` lists them, and `muster team get` shows their values",
        ));
    }
    let data = session.linear.run(
        &ops::TEAM_UPDATE,
        json!({ "id": id, "input": Value::Object(input) }),
    )?;
    out.emit(&payload(&data, "teamUpdate", "team")?);
    Ok(())
}

/// `team retire` and `team unretire`: `teamUpdate` with `retiredAt` set to now, or
/// cleared, and the team as Linear returns it.
fn team_retire(
    session: &Session,
    args: &TeamRefArgs,
    retire: bool,
    out: &Output,
) -> Result<(), Error> {
    let key = session.team_key_of(&args.key)?;
    let team = session.team_full(&key, false)?;
    let id = need_str(&team, "id", "TeamByKey")?;
    let value = if retire {
        json!(now_iso())
    } else {
        Value::Null
    };
    let data = session.linear.run(
        &ops::TEAM_UPDATE,
        json!({ "id": id, "input": { "retiredAt": value } }),
    )?;
    out.emit(&payload(&data, "teamUpdate", "team")?);
    Ok(())
}

/// A delete payload, after checking `success`: Linear's own object, `success` and the
/// `entityId`, printed as it is.
fn delete_payload(data: &Value, field: &str) -> Result<Value, Error> {
    match at(data, &format!("{field}.success")).as_bool() {
        Some(true) => Ok(at(data, field).clone()),
        Some(false) => Err(Error::new(
            ErrorCode::Refused,
            format!("Linear answered {field} with success false"),
        )),
        None => Err(Error::new(
            ErrorCode::Failed,
            format!("Linear's answer to {field} has no success"),
        )),
    }
}

fn team_delete(session: &Session, args: &TeamRefArgs, out: &Output) -> Result<(), Error> {
    let key = session.team_key_of(&args.key)?;
    let team = session.team_full(&key, false)?;
    let id = need_str(&team, "id", "TeamByKey")?;
    let data = session.linear.run(&ops::TEAM_DELETE, json!({ "id": id }))?;
    out.emit(&delete_payload(&data, "teamDelete")?);
    Ok(())
}

fn team_unarchive(session: &Session, args: &TeamRefArgs, out: &Output) -> Result<(), Error> {
    let key = session.team_key_of(&args.key)?;
    let team = session.team_full(&key, true)?;
    let id = need_str(&team, "id", "TeamByKey")?;
    let data = session
        .linear
        .run(&ops::TEAM_UNARCHIVE, json!({ "id": id }))?;
    out.emit(&payload(&data, "teamUnarchive", "entity")?);
    Ok(())
}

fn list_teams(session: &Session, args: &ListArgs, out: &Output) -> Result<(), Error> {
    let n = limit(args.limit)?;
    let data = session.linear.run(&ops::TEAMS, json!({ "first": n }))?;
    emit_list(
        out,
        nodes(&data, "teams"),
        has_next_page(&data, "teams"),
        "teams",
        n,
    );
    Ok(())
}

fn list_states(session: &Session, args: &ListArgs, out: &Output) -> Result<(), Error> {
    let n = limit(args.limit)?;
    let team_key = session.team_key()?;
    let mut states = session.states(&team_key)?;
    let cut = states.len() > n;
    states.truncate(n);
    emit_list(out, states, cut, "states", n);
    Ok(())
}

fn list_labels(session: &Session, args: &ListArgs, out: &Output) -> Result<(), Error> {
    let n = limit(args.limit)?;
    let team_key = match &session.config.team {
        Some(_) => Some(session.team_key()?),
        None => None,
    };
    let data = session.linear.run(&ops::ISSUE_LABELS, json!({ "first": n, "filter": match team_key.as_deref() {
        Some(key) => json!({ "or": [ { "team": { "key": { "eq": key } } }, { "team": { "null": true } } ] }),
        None => Value::Null,
    } }))?;
    emit_list(
        out,
        nodes(&data, "issueLabels"),
        has_next_page(&data, "issueLabels"),
        "labels",
        n,
    );
    Ok(())
}

fn list_projects(session: &Session, args: &ListArgs, out: &Output) -> Result<(), Error> {
    let n = limit(args.limit)?;
    let data = session.linear.run(
        &ops::PROJECTS,
        json!({ "first": n, "filter": Value::Null, "teams": TEAMS_PER_PROJECT }),
    )?;
    let projects = nodes(&data, "projects");
    let cut_teams: Vec<String> = projects
        .iter()
        .filter(|p| has_next_page(p, "teams"))
        .map(|p| str_at(p, "name").unwrap_or("?").to_string())
        .collect();
    emit_list(
        out,
        projects,
        has_next_page(&data, "projects"),
        "projects",
        n,
    );
    teams_notice(out, &cut_teams);
    Ok(())
}

/// The notice for projects whose teams were cut at `TEAMS_PER_PROJECT`.
fn teams_notice(out: &Output, cut: &[String]) {
    if !cut.is_empty() {
        out.notice(
            "truncated",
            &format!(
                "teams: {} in more than the {TEAMS_PER_PROJECT} teams printed",
                cut.join(", ")
            ),
            "TEAMS_PER_PROJECT is the bound (docs/reference/commands.md#bounds); Linear's own page shows the rest",
        );
    }
}

fn list_users(session: &Session, args: &UserListArgs, out: &Output) -> Result<(), Error> {
    let n = limit(args.limit)?;
    let data = session.linear.run(
        &ops::USERS,
        json!({ "first": n, "filter": Value::Null, "includeDisabled": args.all }),
    )?;
    emit_list(
        out,
        nodes(&data, "users"),
        has_next_page(&data, "users"),
        "users",
        n,
    );
    Ok(())
}

/// The input of `projectCreate` but its `teamIds`: `--name` and `--description`, each
/// checked against `BODY_MAX_BYTES` before any request. The unit test reaches the bound
/// here, in process, where Linux argv cannot (one argument is capped at 128 KiB).
fn project_input(args: &ProjectCreateArgs) -> Result<Map<String, Value>, Error> {
    check_body(Text::Name, &args.name)?;
    let mut input = Map::new();
    input.insert("name".into(), json!(args.name));
    if let Some(description) = &args.description {
        check_body(Text::Body, description)?;
        input.insert("description".into(), json!(description));
    }
    Ok(input)
}

fn project_create(session: &Session, args: &ProjectCreateArgs, out: &Output) -> Result<(), Error> {
    let mut input = project_input(args)?;
    let (team, _) = session.team()?;
    input.insert("teamIds".into(), json!([need_str(&team, "id", "Teams")?]));
    // The guard is workspace-wide, as `--project <NAME>` resolves workspace-wide: a
    // second project with the name would make the first unreachable by name.
    if !args.allow_duplicate
        && let Some(existing) = session.find_project(&args.name)?
    {
        return Err(Error::with_hint(
            ErrorCode::Refused,
            format!(
                "a project named {:?} exists in the workspace: {}",
                str_at(&existing, "name").unwrap_or(&args.name),
                str_at(&existing, "url").unwrap_or("?")
            ),
            "use that project, or pass --allow-duplicate if a second one is wanted; `--project <NAME>` resolves a name to the first match",
        )
        .at("muster-project-create"));
    }
    let data = session.linear.run(
        &ops::PROJECT_CREATE,
        json!({ "input": Value::Object(input), "teams": TEAMS_PER_PROJECT }),
    )?;
    // A project muster makes has the one team, so its `teams` page is never cut; the
    // `truncated` notice is `project list`'s, where a project may span many.
    out.emit(&payload(&data, "projectCreate", "project")?);
    Ok(())
}

fn initiative_list(
    session: &Session,
    args: &InitiativeListArgs,
    out: &Output,
) -> Result<(), Error> {
    let n = limit(args.limit)?;
    let filter = match args.status {
        Some(status) => json!({ "status": { "eq": status.linear() } }),
        None => Value::Null,
    };
    let data = session
        .linear
        .run(&ops::INITIATIVES, json!({ "first": n, "filter": filter }))?;
    emit_list(
        out,
        nodes(&data, "initiatives"),
        has_next_page(&data, "initiatives"),
        "initiatives",
        n,
    );
    Ok(())
}

fn initiative_get(session: &Session, args: &InitiativeRefArgs, out: &Output) -> Result<(), Error> {
    let initiative = session.initiative_full(&args.name, false)?;
    out.emit(&initiative);
    if has_next_page(&initiative, "projects") {
        out.notice(
            "truncated",
            &format!(
                "projects: {} has more than the {PROJECTS_PER_INITIATIVE} printed",
                str_at(&initiative, "name").unwrap_or(&args.name)
            ),
            "PROJECTS_PER_INITIATIVE is the bound (docs/reference/commands.md#bounds)",
        );
    }
    Ok(())
}

/// The scalar part of an initiative input from the create or update flags: only what
/// was passed, the texts bounded, the content inline or from its file, `none`
/// clearing the target date.
fn initiative_input(
    name: Option<&str>,
    description: &Option<String>,
    status: Option<crate::cli::InitiativeStatus>,
    target_date: &Option<String>,
    priority: Option<u8>,
    content: &Option<String>,
    content_file: &Option<String>,
) -> Result<Map<String, Value>, Error> {
    let mut input = Map::new();
    if let Some(name) = name {
        check_body(Text::Name, name)?;
        input.insert("name".into(), json!(name));
    }
    if let Some(description) = description {
        check_body(Text::Body, description)?;
        input.insert("description".into(), json!(description));
    }
    if let Some(status) = status {
        input.insert("status".into(), json!(status.linear()));
    }
    if let Some(date) = target_date {
        input.insert("targetDate".into(), date_or_none(date));
    }
    if let Some(priority) = priority {
        input.insert("priority".into(), json!(priority));
    }
    if let Some(content) = body_from(content, content_file)? {
        input.insert("content".into(), json!(content));
    }
    Ok(input)
}

fn initiative_create(
    session: &Session,
    args: &InitiativeCreateArgs,
    out: &Output,
) -> Result<(), Error> {
    let mut input = initiative_input(
        Some(&args.name),
        &args.description,
        args.status,
        &args.target_date,
        args.priority,
        &args.content,
        &args.content_file,
    )?;
    // The guard: `initiative get <NAME>` and `--project`'s kin resolve a name to one.
    // Only a missing initiative lets the create go on; any other failure of the read
    // (a timeout, the rate limit, a dropped connection) is reported as itself.
    if !args.allow_duplicate {
        match session.initiative_full(&args.name, false) {
            Ok(existing) => {
                return Err(Error::with_hint(
                    ErrorCode::Refused,
                    format!(
                        "an initiative named {:?} exists: {}",
                        str_at(&existing, "name").unwrap_or(&args.name),
                        str_at(&existing, "url").unwrap_or("?")
                    ),
                    "use that initiative, or pass --allow-duplicate if a second one is wanted",
                )
                .at("muster-initiative-create"));
            }
            Err(error) if error.code == ErrorCode::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    if let Some(owner) = &args.owner {
        input.insert("ownerId".into(), json!(session.user_id(owner)?));
    }
    if let Some(key) = &args.lead_team {
        input.insert(
            "leadTeamId".into(),
            json!(need_str(
                &session.team_full(key, false)?,
                "id",
                "TeamByKey"
            )?),
        );
    }
    let data = session.linear.run(
        &ops::INITIATIVE_CREATE,
        json!({ "input": Value::Object(input) }),
    )?;
    out.emit(&payload(&data, "initiativeCreate", "initiative")?);
    Ok(())
}

fn initiative_update(
    session: &Session,
    args: &InitiativeUpdateArgs,
    out: &Output,
) -> Result<(), Error> {
    let mut input = initiative_input(
        args.name.as_deref(),
        &args.description,
        args.status,
        &args.target_date,
        args.priority,
        &args.content,
        &args.content_file,
    )?;
    let initiative = session.initiative_full(&args.initiative, false)?;
    let id = need_str(&initiative, "id", "InitiativeByName")?;
    if let Some(owner) = &args.owner {
        let value = if is_none(owner) {
            Value::Null
        } else {
            json!(session.user_id(owner)?)
        };
        input.insert("ownerId".into(), value);
    }
    if let Some(key) = &args.lead_team {
        let value = if is_none(key) {
            Value::Null
        } else {
            json!(need_str(
                &session.team_full(key, false)?,
                "id",
                "TeamByKey"
            )?)
        };
        input.insert("leadTeamId".into(), value);
    }
    if input.is_empty() {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            "nothing to change",
            "pass at least one of --name, --description, --owner, --lead-team, --status, --target-date, --priority, --content, --content-file",
        ));
    }
    let data = session.linear.run(
        &ops::INITIATIVE_UPDATE,
        json!({ "id": id, "input": Value::Object(input) }),
    )?;
    out.emit(&payload(&data, "initiativeUpdate", "initiative")?);
    Ok(())
}

/// `initiative archive` and `unarchive`: one mutation with the initiative's id, and
/// Linear's entity.
fn initiative_lifecycle(
    session: &Session,
    args: &InitiativeRefArgs,
    op: &ops::Operation,
    field: &str,
    include_archived: bool,
    out: &Output,
) -> Result<(), Error> {
    let initiative = session.initiative_full(&args.name, include_archived)?;
    let id = need_str(&initiative, "id", "InitiativeByName")?;
    let data = session.linear.run(op, json!({ "id": id }))?;
    out.emit(&payload(&data, field, "entity")?);
    Ok(())
}

fn initiative_add_project(
    session: &Session,
    args: &InitiativeProjectArgs,
    out: &Output,
) -> Result<(), Error> {
    let initiative_id = need_str(
        &session.initiative_full(&args.name, false)?,
        "id",
        "InitiativeByName",
    )?;
    let project_id = session.project_id(&args.project)?;
    let data = session.linear.run(
        &ops::INITIATIVE_LINK_CREATE,
        json!({ "input": { "initiativeId": initiative_id, "projectId": project_id } }),
    )?;
    out.emit(&payload(
        &data,
        "initiativeToProjectCreate",
        "initiativeToProject",
    )?);
    Ok(())
}

/// `initiative remove-project`: the link is found among the workspace's up to
/// `PAGE_CAP` initiative-to-project links, since Linear's list takes no filter; exit
/// 4 when the project is not in the initiative, exit 6 when the list was cut.
fn initiative_remove_project(
    session: &Session,
    args: &InitiativeProjectArgs,
    out: &Output,
) -> Result<(), Error> {
    let initiative_id = need_str(
        &session.initiative_full(&args.name, false)?,
        "id",
        "InitiativeByName",
    )?;
    let project_id = session.project_id(&args.project)?;
    let data = session
        .linear
        .run(&ops::INITIATIVE_LINKS, json!({ "first": PAGE_CAP }))?;
    let links = nodes(&data, "initiativeToProjects");
    let found = links.iter().find(|l| {
        str_at(l, "initiative.id") == Some(initiative_id.as_str())
            && str_at(l, "project.id") == Some(project_id.as_str())
    });
    let link = match found {
        Some(link) => link.clone(),
        None if has_next_page(&data, "initiativeToProjects") => {
            return Err(Error::with_hint(
                ErrorCode::Refused,
                format!(
                    "the link between {:?} and {:?} is not among the PAGE_CAP ({PAGE_CAP}) newest, and the workspace has more",
                    args.name, args.project
                ),
                "Linear's list of initiative-to-project links takes no filter; the rest are reached in the browser; PAGE_CAP is the bound",
            )
            .at("bounds"));
        }
        None => {
            return Err(Error::with_hint(
                ErrorCode::NotFound,
                format!(
                    "project {:?} is not in initiative {:?}",
                    args.project, args.name
                ),
                "`muster initiative get <NAME>` lists an initiative's projects",
            ));
        }
    };
    let id = need_str(&link, "id", "InitiativeLinks")?;
    let data = session
        .linear
        .run(&ops::INITIATIVE_LINK_DELETE, json!({ "id": id }))?;
    out.emit(&delete_payload(&data, "initiativeToProjectDelete")?);
    Ok(())
}

fn cycle_list(session: &Session, args: &CycleListArgs, out: &Output) -> Result<(), Error> {
    let n = limit(args.limit)?;
    let team_key = session.team_key()?;
    let mut filter = Map::new();
    if args.active {
        filter.insert("isActive".into(), json!({ "eq": true }));
    } else if args.upcoming {
        filter.insert("isFuture".into(), json!({ "eq": true }));
    } else if args.past {
        filter.insert("isPast".into(), json!({ "eq": true }));
    }
    let data = session.cycles(&team_key, filter, n)?;
    emit_list(
        out,
        nodes(&data, "cycles"),
        has_next_page(&data, "cycles"),
        "cycles",
        n,
    );
    Ok(())
}

fn cycle_get(session: &Session, args: &CycleRefArgs, out: &Output) -> Result<(), Error> {
    let team_key = session.team_key()?;
    let cycle = session.cycle_full(&team_key, args.number)?;
    out.emit(&cycle);
    if has_next_page(&cycle, "issues") {
        out.notice(
            "truncated",
            &format!(
                "issues: cycle {} has more than the {ISSUES_PER_CYCLE} printed",
                args.number
            ),
            "ISSUES_PER_CYCLE is the bound (docs/reference/commands.md#bounds); `muster issue list --cycle <NUMBER>` pages them",
        );
    }
    Ok(())
}

/// The input of `cycleUpdate` from the flags: only what was passed, the texts bounded;
/// the times go to Linear as given, since Linear validates a `DateTime`.
fn cycle_input(args: &CycleUpdateArgs) -> Result<Map<String, Value>, Error> {
    let mut input = Map::new();
    if let Some(name) = &args.name {
        check_body(Text::Name, name)?;
        input.insert("name".into(), json!(name));
    }
    if let Some(description) = &args.description {
        check_body(Text::Body, description)?;
        input.insert("description".into(), json!(description));
    }
    if let Some(time) = &args.starts_at {
        input.insert("startsAt".into(), json!(time));
    }
    if let Some(time) = &args.ends_at {
        input.insert("endsAt".into(), json!(time));
    }
    if input.is_empty() {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            "nothing to change",
            "pass at least one of --name, --description, --starts-at, --ends-at",
        ));
    }
    Ok(input)
}

fn cycle_update(session: &Session, args: &CycleUpdateArgs, out: &Output) -> Result<(), Error> {
    let input = cycle_input(args)?;
    let team_key = session.team_key()?;
    let cycle = session.cycle_ref(&team_key, &args.number.to_string())?;
    let id = need_str(&cycle, "id", "Cycles")?;
    let data = session.linear.run(
        &ops::CYCLE_UPDATE,
        json!({ "id": id, "input": Value::Object(input) }),
    )?;
    out.emit(&payload(&data, "cycleUpdate", "cycle")?);
    Ok(())
}

fn document_list(session: &Session, args: &DocumentListArgs, out: &Output) -> Result<(), Error> {
    let n = limit(args.limit)?;
    let mut filter = Map::new();
    if let Some(project) = &args.project {
        filter.insert(
            "project".into(),
            json!({ "name": { "eqIgnoreCase": project } }),
        );
    }
    if let Some(issue) = &args.issue {
        filter.insert(
            "issue".into(),
            json!({ "id": { "eq": session.issue_id(issue)? } }),
        );
    }
    let filter = if filter.is_empty() {
        Value::Null
    } else {
        Value::Object(filter)
    };
    let data = session
        .linear
        .run(&ops::DOCUMENTS, json!({ "first": n, "filter": filter }))?;
    emit_list(
        out,
        nodes(&data, "documents"),
        has_next_page(&data, "documents"),
        "documents",
        n,
    );
    Ok(())
}

fn document_get(session: &Session, args: &DocumentRefArgs, out: &Output) -> Result<(), Error> {
    out.emit(&session.document_full(&args.title, false)?);
    Ok(())
}

fn document_create(
    session: &Session,
    args: &DocumentCreateArgs,
    out: &Output,
) -> Result<(), Error> {
    check_body(Text::Title, &args.title)?;
    let content = body_from(&args.content, &args.content_file)?;
    // The guard: `document get <TITLE>` resolves a title to one document. Only a
    // missing document lets the create go on; any other failure of the read is itself.
    if !args.allow_duplicate {
        match session.document_full(&args.title, false) {
            Ok(existing) => {
                return Err(Error::with_hint(
                    ErrorCode::Refused,
                    format!(
                        "a document titled {:?} exists: {}",
                        str_at(&existing, "title").unwrap_or(&args.title),
                        str_at(&existing, "url").unwrap_or("?")
                    ),
                    "use that document, or pass --allow-duplicate if a second one is wanted",
                )
                .at("muster-document-create"));
            }
            Err(error) if error.code == ErrorCode::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    let mut input = Map::new();
    input.insert("title".into(), json!(args.title));
    if let Some(content) = content {
        input.insert("content".into(), json!(content));
    }
    if let Some(project) = &args.project {
        input.insert("projectId".into(), json!(session.project_id(project)?));
    }
    if let Some(issue) = &args.issue {
        input.insert("issueId".into(), json!(session.issue_id(issue)?));
    }
    let data = session.linear.run(
        &ops::DOCUMENT_CREATE,
        json!({ "input": Value::Object(input) }),
    )?;
    out.emit(&payload(&data, "documentCreate", "document")?);
    Ok(())
}

fn document_update(
    session: &Session,
    args: &DocumentUpdateArgs,
    out: &Output,
) -> Result<(), Error> {
    let mut input = Map::new();
    if let Some(title) = &args.title {
        check_body(Text::Title, title)?;
        input.insert("title".into(), json!(title));
    }
    if let Some(content) = body_from(&args.content, &args.content_file)? {
        input.insert("content".into(), json!(content));
    }
    let document = session.document_full(&args.document, false)?;
    let id = need_str(&document, "id", "DocumentByTitle")?;
    if let Some(project) = &args.project {
        let value = if is_none(project) {
            Value::Null
        } else {
            json!(session.project_id(project)?)
        };
        input.insert("projectId".into(), value);
    }
    if let Some(issue) = &args.issue {
        let value = if is_none(issue) {
            Value::Null
        } else {
            json!(session.issue_id(issue)?)
        };
        input.insert("issueId".into(), value);
    }
    if input.is_empty() {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            "nothing to change",
            "pass at least one of --title, --content, --content-file, --project, --issue",
        ));
    }
    let data = session.linear.run(
        &ops::DOCUMENT_UPDATE,
        json!({ "id": id, "input": Value::Object(input) }),
    )?;
    out.emit(&payload(&data, "documentUpdate", "document")?);
    Ok(())
}

/// `document delete` (the trash) and `document unarchive`: one mutation with the
/// document's id, and Linear's entity.
fn document_lifecycle(
    session: &Session,
    args: &DocumentRefArgs,
    op: &ops::Operation,
    field: &str,
    include_archived: bool,
    out: &Output,
) -> Result<(), Error> {
    let document = session.document_full(&args.title, include_archived)?;
    let id = need_str(&document, "id", "DocumentByTitle")?;
    let data = session.linear.run(op, json!({ "id": id }))?;
    out.emit(&payload(&data, field, "entity")?);
    Ok(())
}

/// `template list`: the team's and the workspace's templates. Linear's
/// `templateSearch` is a list with no page info, so one more than the limit is asked
/// for, and a cut list is known from the extra one (ADR 0014, group 15).
fn template_list(session: &Session, args: &TemplateListArgs, out: &Output) -> Result<(), Error> {
    let n = limit(args.limit)?;
    let team_key = session.team_key()?;
    let mut filter = Map::new();
    filter.insert(
        "or".into(),
        json!([ { "team": { "key": { "eq": team_key } } }, { "team": { "null": true } } ]),
    );
    if let Some(kind) = args.r#type {
        filter.insert("type".into(), json!({ "eq": kind.linear() }));
    }
    let data = session.linear.run(
        &ops::TEMPLATES,
        json!({ "first": n + 1, "filter": Value::Object(filter) }),
    )?;
    let mut templates = data["templateSearch"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let cut = templates.len() > n;
    templates.truncate(n);
    emit_list(out, templates, cut, "templates", n);
    Ok(())
}

fn template_get(session: &Session, args: &TemplateRefArgs, out: &Output) -> Result<(), Error> {
    let team_key = session.team_key()?;
    out.emit(&session.template_named(&team_key, &args.name, false)?);
    Ok(())
}

/// A template's data from its file: JSON, checked against `BODY_MAX_BYTES` by
/// `read_body` and parsed here, since Linear takes `templateData` as JSON.
fn template_data(path: &str) -> Result<Value, Error> {
    let text = read_body(path)?;
    serde_json::from_str(&text).map_err(|e| {
        Error::with_hint(
            ErrorCode::Usage,
            format!("the data file is not JSON: {e}"),
            "`--data-file` takes Linear's `templateData` as JSON; `muster template get <NAME>` shows the shape of an existing one",
        )
    })
}

fn template_create(
    session: &Session,
    args: &TemplateCreateArgs,
    out: &Output,
) -> Result<(), Error> {
    check_body(Text::Name, &args.name)?;
    if let Some(description) = &args.description {
        check_body(Text::Body, description)?;
    }
    let data = template_data(&args.data_file)?;
    let (team, _) = session.team()?;
    let team_key = need_str(&team, "key", "Teams")?;
    // The guard is on the name in the template's own scope, the team's templates or
    // the workspace's, as `label create` guards a label: `template get <NAME>`
    // resolves a name to the team's own first, then the workspace's. A failed read is
    // itself; only a scope without the name lets the create go on.
    if !args.allow_duplicate {
        let found = session.templates_named(&team_key, &args.name, false)?;
        let same_scope = found
            .iter()
            .find(|t| at(t, "team").is_null() == args.workspace);
        if let Some(existing) = same_scope {
            let scope = if args.workspace {
                "the workspace".to_string()
            } else {
                format!("team {team_key}")
            };
            return Err(Error::with_hint(
                ErrorCode::Refused,
                format!(
                    "a template named {:?} exists for {scope}",
                    str_at(existing, "name").unwrap_or(&args.name)
                ),
                "use that template, or pass --allow-duplicate if a second one is wanted",
            )
            .at("muster-template-create"));
        }
    }
    let mut input = Map::new();
    input.insert("name".into(), json!(args.name));
    input.insert("type".into(), json!(args.r#type.linear()));
    input.insert("templateData".into(), data);
    if let Some(description) = &args.description {
        input.insert("description".into(), json!(description));
    }
    if !args.workspace {
        input.insert("teamId".into(), json!(need_str(&team, "id", "Teams")?));
    }
    let answer = session.linear.run(
        &ops::TEMPLATE_CREATE,
        json!({ "input": Value::Object(input) }),
    )?;
    out.emit(&payload(&answer, "templateCreate", "template")?);
    Ok(())
}

fn template_update(
    session: &Session,
    args: &TemplateUpdateArgs,
    out: &Output,
) -> Result<(), Error> {
    let mut input = Map::new();
    if let Some(name) = &args.name {
        check_body(Text::Name, name)?;
        input.insert("name".into(), json!(name));
    }
    if let Some(description) = &args.description {
        check_body(Text::Body, description)?;
        input.insert("description".into(), json!(description));
    }
    if let Some(path) = &args.data_file {
        input.insert("templateData".into(), template_data(path)?);
    }
    if input.is_empty() {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            "nothing to change",
            "pass at least one of --name, --description, --data-file",
        ));
    }
    let team_key = session.team_key()?;
    let template = session.template_named(&team_key, &args.template, false)?;
    let id = need_str(&template, "id", "TemplateByName")?;
    let answer = session.linear.run(
        &ops::TEMPLATE_UPDATE,
        json!({ "id": id, "input": Value::Object(input) }),
    )?;
    out.emit(&payload(&answer, "templateUpdate", "template")?);
    Ok(())
}

fn template_delete(session: &Session, args: &TemplateRefArgs, out: &Output) -> Result<(), Error> {
    let team_key = session.team_key()?;
    let template = session.template_named(&team_key, &args.name, false)?;
    let id = need_str(&template, "id", "TemplateByName")?;
    let answer = session
        .linear
        .run(&ops::TEMPLATE_DELETE, json!({ "id": id }))?;
    out.emit(&delete_payload(&answer, "templateDelete")?);
    Ok(())
}

fn webhook_list(session: &Session, args: &WebhookListArgs, out: &Output) -> Result<(), Error> {
    let n = limit(args.limit)?;
    let data = session.linear.run(&ops::WEBHOOKS, json!({ "first": n }))?;
    emit_list(
        out,
        nodes(&data, "webhooks"),
        has_next_page(&data, "webhooks"),
        "webhooks",
        n,
    );
    Ok(())
}

/// A webhook's secret from its file: read through `read_body` and its bound, with
/// the line end a file carries taken off; never from argv, never printed.
fn secret_from(path: &str) -> Result<String, Error> {
    let text = read_body(path)?;
    let secret = text.trim_end_matches(['\n', '\r']).to_string();
    if secret.is_empty() {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            "the secret file is empty",
            "put the signing secret in the file, on one line",
        ));
    }
    Ok(secret)
}

/// The resource types of a create or update: each checked as a name, none empty.
fn resource_types(types: &[String]) -> Result<Vec<String>, Error> {
    let mut out = Vec::new();
    for kind in types {
        let kind = kind.trim();
        if kind.is_empty() {
            return Err(Error::with_hint(
                ErrorCode::Usage,
                "a resource type is empty",
                "name a kind of change as Linear does: Issue, Comment, Project, Cycle, IssueLabel, Reaction, ...",
            ));
        }
        check_body(Text::Name, kind)?;
        out.push(kind.to_string());
    }
    Ok(out)
}

fn webhook_create(session: &Session, args: &WebhookCreateArgs, out: &Output) -> Result<(), Error> {
    check_body(Text::Link, &args.url)?;
    let types = resource_types(&args.resource_type)?;
    if let Some(label) = &args.label {
        check_body(Text::Name, label)?;
    }
    let secret = match &args.secret_file {
        Some(path) => Some(secret_from(path)?),
        None => None,
    };
    // The guard: a webhook posts to one URL, and `<WEBHOOK>` resolves a URL to one.
    if !args.allow_duplicate {
        let (hooks, cut) = session.webhooks()?;
        if let Some(existing) = hooks
            .iter()
            .find(|h| str_at(h, "url") == Some(args.url.as_str()))
        {
            return Err(Error::with_hint(
                ErrorCode::Refused,
                format!(
                    "a webhook posts to {} already{}",
                    args.url,
                    match str_at(existing, "label") {
                        Some(label) => format!(" ({label:?})"),
                        None => String::new(),
                    }
                ),
                "use that webhook, or pass --allow-duplicate if a second one is wanted",
            )
            .at("muster-webhook-create"));
        }
        if cut {
            return Err(Error::with_hint(
                ErrorCode::Refused,
                format!("the workspace has more than PAGE_CAP ({PAGE_CAP}) webhooks, so a duplicate of {} cannot be ruled out", args.url),
                "pass --allow-duplicate to create it anyway; PAGE_CAP is the bound",
            )
            .at("bounds"));
        }
    }
    let mut input = Map::new();
    input.insert("url".into(), json!(args.url));
    input.insert("resourceTypes".into(), json!(types));
    if let Some(label) = &args.label {
        input.insert("label".into(), json!(label));
    }
    if let Some(secret) = secret {
        input.insert("secret".into(), json!(secret));
    }
    if args.all_public_teams {
        input.insert("allPublicTeams".into(), json!(true));
    } else {
        let (team, _) = session.team()?;
        input.insert("teamId".into(), json!(need_str(&team, "id", "Teams")?));
    }
    let data = session.linear.run(
        &ops::WEBHOOK_CREATE,
        json!({ "input": Value::Object(input) }),
    )?;
    out.emit(&payload(&data, "webhookCreate", "webhook")?);
    Ok(())
}

fn webhook_update(session: &Session, args: &WebhookUpdateArgs, out: &Output) -> Result<(), Error> {
    let mut input = Map::new();
    if let Some(url) = &args.url {
        check_body(Text::Link, url)?;
        input.insert("url".into(), json!(url));
    }
    if let Some(label) = &args.label {
        check_body(Text::Name, label)?;
        input.insert("label".into(), json!(label));
    }
    if let Some(enabled) = args.enabled {
        input.insert("enabled".into(), json!(enabled));
    }
    if !args.resource_type.is_empty() {
        input.insert(
            "resourceTypes".into(),
            json!(resource_types(&args.resource_type)?),
        );
    }
    if let Some(path) = &args.secret_file {
        input.insert("secret".into(), json!(secret_from(path)?));
    }
    if input.is_empty() {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            "nothing to change",
            "pass at least one of --url, --label, --enabled, --resource-type, --secret-file",
        ));
    }
    let hook = session.webhook_ref(&args.webhook)?;
    let id = need_str(&hook, "id", "Webhooks")?;
    let data = session.linear.run(
        &ops::WEBHOOK_UPDATE,
        json!({ "id": id, "input": Value::Object(input) }),
    )?;
    out.emit(&payload(&data, "webhookUpdate", "webhook")?);
    Ok(())
}

fn webhook_delete(session: &Session, args: &WebhookRefArgs, out: &Output) -> Result<(), Error> {
    let hook = session.webhook_ref(&args.webhook)?;
    let id = need_str(&hook, "id", "Webhooks")?;
    let data = session
        .linear
        .run(&ops::WEBHOOK_DELETE, json!({ "id": id }))?;
    out.emit(&delete_payload(&data, "webhookDelete")?);
    Ok(())
}

/// `audit list`: the workspace's audit entries through Linear's `auditEntries` and
/// its filter; the actor resolved as `--assignee` resolves a user, before any
/// request (ADR 0014, group 17).
fn audit_list(session: &Session, args: &AuditListArgs, out: &Output) -> Result<(), Error> {
    let n = limit(args.limit)?;
    let mut filter = Map::new();
    if let Some(kind) = &args.r#type {
        check_body(Text::Name, kind)?;
        filter.insert("type".into(), json!({ "eq": kind }));
    }
    if let Some(who) = &args.actor {
        filter.insert(
            "actor".into(),
            json!({ "id": { "eq": session.user_id(who)? } }),
        );
    }
    if let Some(since) = &args.since {
        filter.insert("createdAt".into(), json!({ "gte": since }));
    }
    let filter = if filter.is_empty() {
        Value::Null
    } else {
        Value::Object(filter)
    };
    let data = session
        .linear
        .run(&ops::AUDIT_ENTRIES, json!({ "first": n, "filter": filter }))?;
    emit_list(
        out,
        nodes(&data, "auditEntries"),
        has_next_page(&data, "auditEntries"),
        "audit entries",
        n,
    );
    Ok(())
}

/// `issue archive`, `issue unarchive` and `issue delete`: the issue resolved as
/// `issue get` resolves one (an archived issue is reached by its identifier, its number
/// or its id; see [`Session::issue`]), one mutation, Linear's entity printed. `issue
/// delete` is Linear's trash, never `permanentlyDelete` (ADR 0014, group 18).
fn issue_lifecycle(
    session: &Session,
    args: &IssueRefArgs,
    op: &ops::Operation,
    field: &str,
    out: &Output,
) -> Result<(), Error> {
    let issue = session.issue(&args.issue, 1, 1, 1)?;
    let id = need_str(&issue, "id", "IssueById")?;
    let data = session.linear.run(op, json!({ "id": id }))?;
    out.emit(&payload(&data, field, "entity")?);
    Ok(())
}

/// `issue batch-update`: the issues resolved one by one (an unknown one is exit 4
/// before any request), at most `ISSUES_PER_BATCH` and all in one team, since a
/// state, a label and a cycle are the team's; the fields resolved as `issue update`
/// resolves them; one `issueBatchUpdate`; the issues printed one per line as Linear
/// returns them.
fn issue_batch_update(
    session: &Session,
    args: &BatchUpdateArgs,
    out: &Output,
) -> Result<(), Error> {
    let mut names: Vec<&str> = Vec::new();
    for name in &args.issue {
        let name = name.trim();
        if !name.is_empty() && !names.iter().any(|n| n.eq_ignore_ascii_case(name)) {
            names.push(name);
        }
    }
    if names.is_empty() {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            "no issue named",
            "pass --issue <ISSUE> once per issue, up to ISSUES_PER_BATCH",
        ));
    }
    if names.len() > ISSUES_PER_BATCH {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            format!(
                "{} issues named, and Linear changes at most {ISSUES_PER_BATCH} in one request",
                names.len()
            ),
            "split the issues over several runs (docs/reference/commands.md#bounds)",
        )
        .at("bounds"));
    }
    let mut ids = Vec::new();
    let mut team_key: Option<String> = None;
    for name in &names {
        let issue = session.issue(name, 1, 1, 1)?;
        let key = need_str(&issue, "team.key", "IssueById")?;
        match &team_key {
            Some(first) if *first != key => {
                return Err(Error::with_hint(
                    ErrorCode::Usage,
                    format!(
                        "the issues are in two teams, {first} and {key}; a batch changes one team's issues"
                    ),
                    "run `issue batch-update` once per team",
                ));
            }
            Some(_) => {}
            None => team_key = Some(key),
        }
        ids.push(need_str(&issue, "id", "IssueById")?);
    }
    let Some(team_key) = team_key else {
        return Err(Error::new(ErrorCode::Usage, "no issue named"));
    };
    let mut input = Map::new();
    if let Some(state) = &args.state {
        input.insert("stateId".into(), json!(session.state_id(&team_key, state)?));
    }
    if let Some(priority) = args.priority {
        input.insert("priority".into(), json!(priority));
    }
    if let Some(who) = &args.assignee {
        let value = if is_none(who) {
            Value::Null
        } else {
            json!(session.user_id(who)?)
        };
        input.insert("assigneeId".into(), value);
    }
    let added = session.label_ids(&team_key, &args.add_label)?;
    if !added.is_empty() {
        input.insert("addedLabelIds".into(), json!(added));
    }
    if let Some(project) = &args.project {
        let value = if is_none(project) {
            Value::Null
        } else {
            json!(session.project_id(project)?)
        };
        input.insert("projectId".into(), value);
        if let Some(milestone) = &args.milestone {
            let value = if is_none(milestone) || is_none(project) {
                Value::Null
            } else {
                let stone = session.milestone_named(project, milestone)?;
                json!(need_str(&stone, "id", "Milestones")?)
            };
            input.insert("projectMilestoneId".into(), value);
        }
    }
    if let Some(cycle) = &args.cycle {
        let value = if is_none(cycle) {
            Value::Null
        } else {
            let cycle = session.cycle_ref(&team_key, cycle)?;
            json!(need_str(&cycle, "id", "Cycles")?)
        };
        input.insert("cycleId".into(), value);
    }
    if input.is_empty() {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            "nothing to change",
            "pass at least one of --state, --priority, --assignee, --add-label, --project, --cycle, --milestone",
        ));
    }
    let data = session.linear.run(
        &ops::ISSUE_BATCH_UPDATE,
        json!({ "ids": ids, "input": Value::Object(input) }),
    )?;
    let answer = &data["issueBatchUpdate"];
    if answer["success"] != Value::Bool(true) {
        return Err(Error::with_hint(
            ErrorCode::Refused,
            "Linear answered issueBatchUpdate with success false",
            "the issues were not changed; `muster issue get <ISSUE>` shows each as it is",
        ));
    }
    let issues = answer["issues"].as_array().cloned().ok_or_else(|| {
        Error::new(
            ErrorCode::Failed,
            "issueBatchUpdate answered without its issues",
        )
    })?;
    for issue in &issues {
        out.emit(issue);
    }
    Ok(())
}

/// `comment update`: the body replaced, by the comment's id; Linear's "not found"
/// is exit 4 (ADR 0014, group 19).
fn comment_update(session: &Session, args: &CommentUpdateArgs, out: &Output) -> Result<(), Error> {
    let body = body_from(&args.body, &args.body_file)?
        .ok_or_else(|| Error::new(ErrorCode::Usage, "a comment needs --body or --body-file"))?;
    let id = args.id.trim();
    if id.is_empty() {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            "the comment id is empty",
            "`muster issue get <ISSUE>` prints each comment's id",
        ));
    }
    let data = session.linear.run(
        &ops::COMMENT_UPDATE,
        json!({ "id": id, "input": { "body": body } }),
    )?;
    out.emit(&payload(&data, "commentUpdate", "comment")?);
    Ok(())
}

fn comment_delete(session: &Session, args: &CommentRefArgs, out: &Output) -> Result<(), Error> {
    let id = args.id.trim();
    if id.is_empty() {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            "the comment id is empty",
            "`muster issue get <ISSUE>` prints each comment's id",
        ));
    }
    let data = session
        .linear
        .run(&ops::COMMENT_DELETE, json!({ "id": id }))?;
    out.emit(&delete_payload(&data, "commentDelete")?);
    Ok(())
}

fn milestone_list(session: &Session, args: &MilestoneListArgs, out: &Output) -> Result<(), Error> {
    let n = limit(args.limit)?;
    let data = session.milestones(&args.project, n)?;
    emit_list(
        out,
        nodes(&data, "projectMilestones"),
        has_next_page(&data, "projectMilestones"),
        "milestones",
        n,
    );
    Ok(())
}

/// The input of `projectMilestoneCreate` but its `projectId`: the flags, checked
/// before any request.
fn milestone_input(args: &MilestoneCreateArgs) -> Result<Map<String, Value>, Error> {
    check_body(Text::Name, &args.name)?;
    let mut input = Map::new();
    input.insert("name".into(), json!(args.name));
    if let Some(description) = &args.description {
        check_body(Text::Body, description)?;
        input.insert("description".into(), json!(description));
    }
    if let Some(date) = &args.target_date {
        input.insert("targetDate".into(), json!(date.trim()));
    }
    Ok(input)
}

fn milestone_create(
    session: &Session,
    args: &MilestoneCreateArgs,
    out: &Output,
) -> Result<(), Error> {
    let mut input = milestone_input(args)?;
    let project_id = session.project_id(&args.project)?;
    // The guard: `--milestone <NAME>` resolves a name to one milestone of the project.
    // Its read is the command's own: a timeout, a rate limit or a dropped connection
    // is that error on the output, never "no duplicate" and a `MilestoneCreate`. The
    // guard exists for an agent that retries after a timeout (ADR 0008), so a read
    // it could not make is no answer.
    if !args.allow_duplicate {
        let stones = nodes(
            &session.milestones(&args.project, PAGE_CAP)?,
            "projectMilestones",
        );
        if let Some(existing) = find_by(&stones, "name", &args.name) {
            return Err(Error::with_hint(
                ErrorCode::Refused,
                format!(
                    "project {:?} has a milestone named {:?} already",
                    args.project,
                    str_at(existing, "name").unwrap_or(&args.name)
                ),
                "use that milestone, or pass --allow-duplicate if a second one is wanted",
            )
            .at("muster-milestone-create"));
        }
    }
    input.insert("projectId".into(), json!(project_id));
    let data = session.linear.run(
        &ops::MILESTONE_CREATE,
        json!({ "input": Value::Object(input) }),
    )?;
    out.emit(&payload(
        &data,
        "projectMilestoneCreate",
        "projectMilestone",
    )?);
    Ok(())
}

fn milestone_update(
    session: &Session,
    args: &MilestoneUpdateArgs,
    out: &Output,
) -> Result<(), Error> {
    let mut input = Map::new();
    if let Some(name) = &args.name {
        check_body(Text::Name, name)?;
        input.insert("name".into(), json!(name));
    }
    if let Some(description) = &args.description {
        check_body(Text::Body, description)?;
        input.insert("description".into(), json!(description));
    }
    if let Some(date) = &args.target_date {
        input.insert("targetDate".into(), date_or_none(date));
    }
    if input.is_empty() {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            "nothing to change",
            "pass at least one of --name, --description, --target-date",
        ));
    }
    let stone = session.milestone_named(&args.project, &args.milestone)?;
    let id = need_str(&stone, "id", "Milestones")?;
    let data = session.linear.run(
        &ops::MILESTONE_UPDATE,
        json!({ "id": id, "input": Value::Object(input) }),
    )?;
    out.emit(&payload(
        &data,
        "projectMilestoneUpdate",
        "projectMilestone",
    )?);
    Ok(())
}

fn milestone_delete(session: &Session, args: &MilestoneRefArgs, out: &Output) -> Result<(), Error> {
    let stone = session.milestone_named(&args.project, &args.name)?;
    let id = need_str(&stone, "id", "Milestones")?;
    let data = session
        .linear
        .run(&ops::MILESTONE_DELETE, json!({ "id": id }))?;
    out.emit(&delete_payload(&data, "projectMilestoneDelete")?);
    Ok(())
}

fn project_update_list(
    session: &Session,
    args: &ProjectUpdateListArgs,
    out: &Output,
) -> Result<(), Error> {
    let n = limit(args.limit)?;
    let data = session.linear.run(
        &ops::PROJECT_UPDATES,
        json!({ "project": args.project, "first": n }),
    )?;
    emit_list(
        out,
        nodes(&data, "projectUpdates"),
        has_next_page(&data, "projectUpdates"),
        "project updates",
        n,
    );
    Ok(())
}

/// The input of `projectUpdateCreate` but its `projectId`: a body, a health, or
/// both; neither is a usage error before any request.
fn status_post_input(args: &ProjectUpdateCreateArgs) -> Result<Map<String, Value>, Error> {
    let mut input = Map::new();
    if let Some(body) = body_from(&args.body, &args.body_file)? {
        input.insert("body".into(), json!(body));
    }
    if let Some(health) = args.health {
        input.insert("health".into(), json!(health.linear()));
    }
    if input.is_empty() {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            "a project update needs --body, --body-file or --health",
            "say what changed, how the project is doing, or both",
        ));
    }
    Ok(input)
}

fn project_update_create(
    session: &Session,
    args: &ProjectUpdateCreateArgs,
    out: &Output,
) -> Result<(), Error> {
    let mut input = status_post_input(args)?;
    input.insert(
        "projectId".into(),
        json!(session.project_id(&args.project)?),
    );
    let data = session.linear.run(
        &ops::PROJECT_UPDATE_CREATE,
        json!({ "input": Value::Object(input) }),
    )?;
    out.emit(&payload(&data, "projectUpdateCreate", "projectUpdate")?);
    Ok(())
}

fn project_status_list(session: &Session, args: &ListArgs, out: &Output) -> Result<(), Error> {
    let n = limit(args.limit)?;
    let mut statuses = session.project_statuses(false)?;
    let cut = statuses.len() > n;
    statuses.truncate(n);
    emit_list(out, statuses, cut, "project statuses", n);
    Ok(())
}

/// The input of `projectStatusCreate`: the flags, checked before any request, the
/// position by [`check_position`] since Linear requires one here.
fn project_status_input(args: &ProjectStatusCreateArgs) -> Result<Map<String, Value>, Error> {
    check_body(Text::Name, &args.name)?;
    check_position(args.position, "muster project-status list")?;
    let mut input = Map::new();
    input.insert("name".into(), json!(args.name));
    input.insert("type".into(), json!(args.r#type.linear()));
    input.insert("color".into(), json!(args.color));
    input.insert("position".into(), json!(args.position));
    if let Some(description) = &args.description {
        check_body(Text::Body, description)?;
        input.insert("description".into(), json!(description));
    }
    if let Some(indefinite) = args.indefinite {
        input.insert("indefinite".into(), json!(indefinite));
    }
    Ok(input)
}

fn project_status_create(
    session: &Session,
    args: &ProjectStatusCreateArgs,
    out: &Output,
) -> Result<(), Error> {
    let input = project_status_input(args)?;
    // The guard: `--status <NAME>` on `project update` resolves a name to one status.
    if !args.allow_duplicate
        && let Some(existing) = find_by(&session.project_statuses(false)?, "name", &args.name)
    {
        return Err(Error::with_hint(
            ErrorCode::Refused,
            format!(
                "the workspace has a project status named {:?} already, of type {}",
                str_at(existing, "name").unwrap_or(&args.name),
                str_at(existing, "type").unwrap_or("?")
            ),
            "use that status, or pass --allow-duplicate if a second one is wanted",
        )
        .at("muster-project-status-create"));
    }
    let data = session.linear.run(
        &ops::PROJECT_STATUS_CREATE,
        json!({ "input": Value::Object(input) }),
    )?;
    out.emit(&payload(&data, "projectStatusCreate", "status")?);
    Ok(())
}

fn project_status_update(
    session: &Session,
    args: &ProjectStatusUpdateArgs,
    out: &Output,
) -> Result<(), Error> {
    let mut input = Map::new();
    if let Some(name) = &args.name {
        check_body(Text::Name, name)?;
        input.insert("name".into(), json!(name));
    }
    if let Some(color) = &args.color {
        input.insert("color".into(), json!(color));
    }
    if let Some(description) = &args.description {
        check_body(Text::Body, description)?;
        input.insert("description".into(), json!(description));
    }
    if let Some(position) = args.position {
        check_position(position, "muster project-status list")?;
        input.insert("position".into(), json!(position));
    }
    if let Some(kind) = args.r#type {
        input.insert("type".into(), json!(kind.linear()));
    }
    if let Some(indefinite) = args.indefinite {
        input.insert("indefinite".into(), json!(indefinite));
    }
    if input.is_empty() {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            "nothing to change",
            "pass at least one of --name, --color, --description, --position, --type, --indefinite",
        ));
    }
    let status = session.project_status(&args.status, false)?;
    let id = need_str(&status, "id", "ProjectStatuses")?;
    let data = session.linear.run(
        &ops::PROJECT_STATUS_UPDATE,
        json!({ "id": id, "input": Value::Object(input) }),
    )?;
    out.emit(&payload(&data, "projectStatusUpdate", "status")?);
    Ok(())
}

/// `project-status archive` and `unarchive`: one mutation with the status's id, and
/// Linear's entity.
fn project_status_lifecycle(
    session: &Session,
    args: &ProjectStatusRefArgs,
    op: &ops::Operation,
    field: &str,
    include_archived: bool,
    out: &Output,
) -> Result<(), Error> {
    let status = session.project_status(&args.name, include_archived)?;
    let id = need_str(&status, "id", "ProjectStatuses")?;
    let data = session.linear.run(op, json!({ "id": id }))?;
    out.emit(&payload(&data, field, "entity")?);
    Ok(())
}

/// The team ids of `--teams`, each key resolved with `TeamByKey`.
fn team_ids(session: &Session, keys: &[String]) -> Result<Vec<String>, Error> {
    keys.iter()
        .map(|key| key.trim())
        .filter(|key| !key.is_empty())
        .map(|key| need_str(&session.team_full(key, false)?, "id", "TeamByKey"))
        .collect()
}

fn project_get(session: &Session, args: &ProjectRefArgs, out: &Output) -> Result<(), Error> {
    let project = session.project_full(&args.name, false)?;
    out.emit(&project);
    let name = str_at(&project, "name").unwrap_or(&args.name).to_string();
    if has_next_page(&project, "teams") {
        teams_notice(out, std::slice::from_ref(&name));
    }
    if has_next_page(&project, "members") {
        out.notice(
            "truncated",
            &format!("members: {name} has more than the {MEMBERS_PER_PROJECT} printed"),
            "MEMBERS_PER_PROJECT is the bound (docs/reference/commands.md#bounds); `project update --add-member` and `--remove-member` refuse a list they cannot read whole",
        );
    }
    if has_next_page(&project, "projectMilestones") {
        out.notice(
            "truncated",
            &format!(
                "projectMilestones: {name} has more than the {MILESTONES_PER_PROJECT} printed"
            ),
            "MILESTONES_PER_PROJECT is the bound (docs/reference/commands.md#bounds)",
        );
    }
    Ok(())
}

/// A date flag's value: `none` clears, anything else is sent as it is for Linear to
/// validate.
fn date_or_none(value: &str) -> Value {
    if is_none(value) {
        Value::Null
    } else {
        json!(value.trim())
    }
}

/// The scalar part of a `projectUpdate` input from `project update`'s flags: only
/// what was passed, the texts bounded, `none` clearing a date.
fn project_update_input(args: &ProjectUpdateArgs) -> Result<Map<String, Value>, Error> {
    let mut input = Map::new();
    if let Some(name) = &args.name {
        check_body(Text::Name, name)?;
        input.insert("name".into(), json!(name));
    }
    if let Some(description) = &args.description {
        check_body(Text::Body, description)?;
        input.insert("description".into(), json!(description));
    }
    if let Some(content) = body_from(&args.content, &args.content_file)? {
        input.insert("content".into(), json!(content));
    }
    if let Some(date) = &args.start_date {
        input.insert("startDate".into(), date_or_none(date));
    }
    if let Some(date) = &args.target_date {
        input.insert("targetDate".into(), date_or_none(date));
    }
    if let Some(priority) = args.priority {
        input.insert("priority".into(), json!(priority));
    }
    if let Some(color) = &args.color {
        input.insert("color".into(), json!(color));
    }
    if let Some(icon) = &args.icon {
        input.insert("icon".into(), json!(icon));
    }
    Ok(input)
}

fn project_update(session: &Session, args: &ProjectUpdateArgs, out: &Output) -> Result<(), Error> {
    let mut input = project_update_input(args)?;
    let project = session.project_full(&args.project, false)?;
    let id = need_str(&project, "id", "ProjectByName")?;
    let name = str_at(&project, "name")
        .unwrap_or(&args.project)
        .to_string();
    if let Some(status) = &args.status {
        input.insert("statusId".into(), json!(session.project_status_id(status)?));
    }
    if let Some(lead) = &args.lead {
        let value = if is_none(lead) {
            Value::Null
        } else {
            json!(session.user_id(lead)?)
        };
        input.insert("leadId".into(), value);
    }
    if let Some(keys) = &args.teams {
        let ids = team_ids(session, keys)?;
        if ids.is_empty() {
            return Err(Error::with_hint(
                ErrorCode::Usage,
                "--teams must name at least one team",
                "a project belongs to one team or more",
            ));
        }
        input.insert("teamIds".into(), json!(ids));
    }
    if !args.add_member.is_empty() || !args.remove_member.is_empty() {
        // Linear replaces `memberIds` whole, so the list is read first; a list cut
        // at MEMBERS_PER_PROJECT is refused rather than shortened (ADR 0014).
        if has_next_page(&project, "members") {
            return Err(Error::with_hint(
                ErrorCode::Refused,
                format!(
                    "{name} has more than MEMBERS_PER_PROJECT ({MEMBERS_PER_PROJECT}) members, so its member list cannot be read whole and would be cut"
                ),
                "change the members in Linear's own page; the bound is docs/reference/commands.md#bounds",
            )
            .at("bounds"));
        }
        let mut members: Vec<String> = nodes(&project, "members")
            .iter()
            .filter_map(|m| str_at(m, "id").map(str::to_string))
            .collect();
        for who in &args.add_member {
            let uid = session.user_id(who)?;
            if !members.contains(&uid) {
                members.push(uid);
            }
        }
        for who in &args.remove_member {
            let uid = session.user_id(who)?;
            members.retain(|m| m != &uid);
        }
        input.insert("memberIds".into(), json!(members));
    }
    if input.is_empty() {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            "nothing to change",
            "pass at least one of --name, --description, --content, --content-file, --status, --lead, --start-date, --target-date, --priority, --teams, --add-member, --remove-member, --color, --icon",
        ));
    }
    let data = session.linear.run(
        &ops::PROJECT_UPDATE,
        json!({ "id": id, "input": Value::Object(input), "teams": TEAMS_PER_PROJECT }),
    )?;
    out.emit(&payload(&data, "projectUpdate", "project")?);
    Ok(())
}

fn project_delete(session: &Session, args: &ProjectRefArgs, out: &Output) -> Result<(), Error> {
    let id = session.project_id(&args.name)?;
    let data = session.linear.run(
        &ops::PROJECT_DELETE,
        json!({ "id": id, "teams": TEAMS_PER_PROJECT }),
    )?;
    out.emit(&payload(&data, "projectDelete", "entity")?);
    Ok(())
}

fn project_unarchive(session: &Session, args: &ProjectRefArgs, out: &Output) -> Result<(), Error> {
    let project = session.project_full(&args.name, true)?;
    let id = need_str(&project, "id", "ProjectByName")?;
    let data = session.linear.run(
        &ops::PROJECT_UNARCHIVE,
        json!({ "id": id, "teams": TEAMS_PER_PROJECT }),
    )?;
    out.emit(&payload(&data, "projectUnarchive", "entity")?);
    Ok(())
}

fn issue_get(session: &Session, args: &GetArgs, out: &Output) -> Result<(), Error> {
    let (comments, attachments) = if args.no_comments {
        (1, 1)
    } else {
        (COMMENTS_PER_ISSUE, ATTACHMENTS_PER_ISSUE)
    };
    let mut issue = session.issue(&args.issue, comments, attachments, CHILDREN_PER_ISSUE)?;
    if args.no_comments
        && let Some(map) = issue.as_object_mut()
    {
        map.remove("comments");
        map.remove("attachments");
    }
    out.emit(&issue);
    if !args.no_comments && has_next_page(&issue, "comments") {
        out.notice(
            "truncated",
            &format!(
                "comments: the issue has more than the {COMMENTS_PER_ISSUE} printed, the newest"
            ),
            "COMMENTS_PER_ISSUE is the bound (docs/reference/commands.md#bounds)",
        );
    }
    if !args.no_comments && has_next_page(&issue, "attachments") {
        out.notice(
            "truncated",
            &format!("attachments: the issue has more than the {ATTACHMENTS_PER_ISSUE} printed"),
            "ATTACHMENTS_PER_ISSUE is the bound (docs/reference/commands.md#bounds)",
        );
    }
    if has_next_page(&issue, "children") {
        out.notice(
            "truncated",
            &format!(
                "children: the issue has more than the {CHILDREN_PER_ISSUE} sub-issues printed, the newest"
            ),
            "CHILDREN_PER_ISSUE is the bound (docs/reference/commands.md#bounds); `muster issue list --parent <ISSUE> --all` lists the team's sub-issues of it, open and closed, up to PAGE_CAP",
        );
    }
    Ok(())
}

/// Whether a flag's value is the `none` sentinel of `--assignee`, `--project`,
/// `--parent` and `team update`'s `--auto-close-period`.
fn is_none(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case("none")
}

/// A filter that names one issue, which Linear resolves inside the same request: by
/// id, by identifier (`APP-12`: the team key and the number), or by number in
/// `team_key`. `None` for a bare number when no team key was given.
fn issue_clause(reference: &str, team_key: Option<&str>) -> Result<Option<Value>, Error> {
    let reference = reference.trim();
    if is_uuid(reference) {
        return Ok(Some(json!({ "id": { "eq": reference } })));
    }
    if let Some((key, number)) = identifier_parts(reference) {
        return Ok(Some(
            json!({ "number": { "eq": number }, "team": { "key": { "eqIgnoreCase": key } } }),
        ));
    }
    if !reference.is_empty() && reference.chars().all(|c| c.is_ascii_digit()) {
        let number: u64 = reference
            .parse()
            .map_err(|_| Error::new(ErrorCode::Usage, format!("{reference:?} is not a number")))?;
        return Ok(team_key
            .map(|key| json!({ "number": { "eq": number }, "team": { "key": { "eq": key } } })));
    }
    Err(bad_reference(reference))
}

/// The team key and the number of an identifier, `APP-12`.
fn identifier_parts(reference: &str) -> Option<(&str, u64)> {
    if !is_identifier(reference) {
        return None;
    }
    let (key, number) = reference.rsplit_once('-')?;
    Some((key, number.parse().ok()?))
}

fn bad_reference(reference: &str) -> Error {
    Error::with_hint(
        ErrorCode::Usage,
        format!("{reference:?} is not an issue identifier (APP-12), a number (12) or an id"),
        "to find an issue by its title, run `muster issue list --search <text>`",
    )
}

/// The `parent` clause of an issue filter: `none` for issues with no parent, else the
/// issue the reference names, resolved by Linear inside the same request.
fn parent_clause(reference: &str, team_key: &str) -> Result<Value, Error> {
    if is_none(reference) {
        return Ok(json!({ "null": true }));
    }
    Ok(issue_clause(reference, Some(team_key))?.unwrap_or(Value::Null))
}

fn assignee_filter(who: &str) -> Value {
    if who.eq_ignore_ascii_case("me") {
        json!({ "isMe": { "eq": true } })
    } else if is_none(who) {
        json!({ "null": true })
    } else if who.contains('@') {
        json!({ "email": { "eqIgnoreCase": who } })
    } else {
        json!({ "or": [ { "name": { "eqIgnoreCase": who } }, { "displayName": { "eqIgnoreCase": who } } ] })
    }
}

fn issue_list(session: &Session, args: &IssueListArgs, out: &Output) -> Result<(), Error> {
    let n = limit(args.limit)?;
    let team_key = session.team_key()?;
    let mut filter = Filter::new().team(&team_key);
    if !args.state.is_empty() {
        filter = filter.with("state", json!({ "name": { "in": args.state } }));
    } else if !args.all {
        filter = filter.with(
            "state",
            json!({ "type": { "in": ["triage", "backlog", "unstarted", "started"] } }),
        );
    }
    if let Some(label) = &args.label {
        filter = filter.with("labels", json!({ "name": { "eqIgnoreCase": label } }));
    }
    if let Some(who) = &args.assignee {
        filter = filter.with("assignee", assignee_filter(who));
    }
    if let Some(text) = &args.search {
        filter = filter.with("title", json!({ "containsIgnoreCase": text }));
    }
    if let Some(parent) = &args.parent {
        filter = filter.with("parent", parent_clause(parent, &team_key)?);
    }
    if let Some(project) = &args.project {
        let clause = if is_none(project) {
            json!({ "null": true })
        } else {
            json!({ "name": { "eqIgnoreCase": project } })
        };
        filter = filter.with("project", clause);
    }
    if let Some(milestone) = &args.milestone {
        let clause = if is_none(milestone) {
            json!({ "null": true })
        } else {
            json!({ "name": { "eqIgnoreCase": milestone } })
        };
        filter = filter.with("projectMilestone", clause);
    }
    if let Some(cycle) = &args.cycle {
        let clause = if is_none(cycle) {
            json!({ "null": true })
        } else {
            let cycle = session.cycle_ref(&team_key, cycle)?;
            json!({ "id": { "eq": need_str(&cycle, "id", "Cycles")? } })
        };
        filter = filter.with("cycle", clause);
    }
    let data = session.linear.run(
        &ops::ISSUES,
        json!({ "first": n, "filter": filter.build(), "orderBy": "updatedAt" }),
    )?;
    emit_list(
        out,
        nodes(&data, "issues"),
        has_next_page(&data, "issues"),
        "issues",
        n,
    );
    Ok(())
}

fn body_from(text: &Option<String>, file: &Option<String>) -> Result<Option<String>, Error> {
    match (text, file) {
        (Some(text), _) => {
            check_body(Text::Body, text)?;
            Ok(Some(text.clone()))
        }
        (None, Some(path)) => Ok(Some(read_body(path)?)),
        (None, None) => Ok(None),
    }
}

/// The `title` and `description` of an `issueCreate` or `issueUpdate` input, from
/// `--title`, `--description` and `--description-file`, each checked against
/// `BODY_MAX_BYTES` before any request; a flag not passed leaves its field out. The
/// unit test reaches the bound here, in process: on Linux a value on argv cannot, since
/// the kernel refuses one argument over 128 KiB (`MAX_ARG_STRLEN`) before muster runs.
fn title_and_description(
    title: Option<&str>,
    description: &Option<String>,
    file: &Option<String>,
) -> Result<Map<String, Value>, Error> {
    let mut input = Map::new();
    if let Some(title) = title {
        check_body(Text::Title, title)?;
        input.insert("title".into(), json!(title));
    }
    if let Some(description) = body_from(description, file)? {
        input.insert("description".into(), json!(description));
    }
    Ok(input)
}

fn issue_create(session: &Session, args: &CreateArgs, out: &Output) -> Result<(), Error> {
    let mut input =
        title_and_description(Some(&args.title), &args.description, &args.description_file)?;
    let parent = match &args.parent {
        Some(reference) => Some(session.issue_brief(reference)?),
        None => None,
    };
    // The team: `--team`, else `MUSTER_TEAM`, else the parent's team, else the
    // workspace's only team (ADR 0008; ADR 0013).
    let (team_key, team_id) = match (&session.config.team, &parent) {
        (None, Some(parent)) => (
            need_str(parent, "team.key", "Issues")?,
            need_str(parent, "team.id", "Issues")?,
        ),
        _ => {
            let (team, _) = session.team()?;
            (
                need_str(&team, "key", "Teams")?,
                need_str(&team, "id", "Teams")?,
            )
        }
    };
    if !args.allow_duplicate {
        let filter = Filter::new()
            .team(&team_key)
            .with("title", json!({ "eqIgnoreCase": args.title }))
            .with(
                "state",
                json!({ "type": { "nin": ["completed", "canceled"] } }),
            )
            .build();
        let data = session.linear.run(
            &ops::ISSUES,
            json!({ "first": 1, "filter": filter, "orderBy": "createdAt" }),
        )?;
        if let Some(existing) = nodes(&data, "issues").first() {
            return Err(Error::with_hint(
                ErrorCode::Refused,
                format!(
                    "team {team_key} already has an open issue titled {:?}: {}",
                    args.title,
                    str_at(existing, "identifier").unwrap_or("?")
                ),
                "use that issue, or pass --allow-duplicate if a second one is wanted",
            )
            .at("muster-issue-create"));
        }
    }
    input.insert("teamId".into(), json!(team_id));
    if let Some(state) = &args.state {
        input.insert("stateId".into(), json!(session.state_id(&team_key, state)?));
    }
    let labels = session.label_ids(&team_key, &args.label)?;
    if !labels.is_empty() {
        input.insert("labelIds".into(), json!(labels));
    }
    if let Some(priority) = args.priority {
        input.insert("priority".into(), json!(priority));
    }
    if let Some(project) = &args.project {
        input.insert("projectId".into(), json!(session.project_id(project)?));
        if let Some(milestone) = &args.milestone {
            let stone = session.milestone_named(project, milestone)?;
            input.insert(
                "projectMilestoneId".into(),
                json!(need_str(&stone, "id", "Milestones")?),
            );
        }
    }
    if let Some(cycle) = &args.cycle {
        let cycle = session.cycle_ref(&team_key, cycle)?;
        input.insert("cycleId".into(), json!(need_str(&cycle, "id", "Cycles")?));
    }
    if let Some(who) = &args.assignee {
        input.insert("assigneeId".into(), json!(session.user_id(who)?));
    }
    if let Some(parent) = &parent {
        input.insert("parentId".into(), json!(need_str(parent, "id", "Issues")?));
    }
    let data = session
        .linear
        .run(&ops::ISSUE_CREATE, json!({ "input": Value::Object(input) }))?;
    out.emit(&payload(&data, "issueCreate", "issue")?);
    Ok(())
}

/// The object of a mutation payload, after checking `success`.
fn payload(data: &Value, field: &str, object: &str) -> Result<Value, Error> {
    let success = at(data, &format!("{field}.success")).as_bool();
    let value = at(data, &format!("{field}.{object}"));
    match (success, value) {
        (Some(true), v) if !v.is_null() => Ok(v.clone()),
        (Some(false), _) => Err(Error::new(
            ErrorCode::Refused,
            format!("Linear answered {field} with success false"),
        )),
        _ => Err(Error::new(
            ErrorCode::Failed,
            format!("Linear's answer to {field} has no {object}"),
        )),
    }
}

fn issue_comment(session: &Session, args: &CommentArgs, out: &Output) -> Result<(), Error> {
    let body = body_from(&args.body, &args.body_file)?
        .ok_or_else(|| Error::new(ErrorCode::Usage, "a comment needs --body or --body-file"))?;
    let issue = session.issue(&args.issue, 1, 1, 1)?;
    let issue_id = need_str(&issue, "id", "IssueById")?;
    let data = session.linear.run(
        &ops::COMMENT_CREATE,
        json!({ "input": { "issueId": issue_id, "body": body } }),
    )?;
    out.emit(&payload(&data, "commentCreate", "comment")?);
    Ok(())
}

/// The input of `attachmentCreate` but its `issueId`: `--url`, `--title` and
/// `--subtitle`, checked before any request. An empty URL or title is a usage error,
/// exit 2, since Linear requires both and muster invents neither; a value over
/// `BODY_MAX_BYTES` is refused, exit 6. The unit test reaches the bound here, in
/// process: on Linux a value on argv cannot, since the kernel refuses one argument over
/// 128 KiB (`MAX_ARG_STRLEN`) before muster runs, and macOS allows it.
fn attachment_input(args: &AttachArgs) -> Result<Map<String, Value>, Error> {
    for (flag, value) in [("--url", &args.url), ("--title", &args.title)] {
        if value.trim().is_empty() {
            return Err(Error::with_hint(
                ErrorCode::Usage,
                format!("{flag} must not be empty"),
                "Linear requires a URL and a title for a link; muster invents neither",
            ));
        }
    }
    check_body(Text::Link, &args.url)?;
    check_body(Text::Title, &args.title)?;
    let mut input = Map::new();
    input.insert("url".into(), json!(args.url));
    input.insert("title".into(), json!(args.title));
    if let Some(subtitle) = &args.subtitle {
        check_body(Text::Subtitle, subtitle)?;
        input.insert("subtitle".into(), json!(subtitle));
    }
    Ok(input)
}

fn issue_attach(session: &Session, args: &AttachArgs, out: &Output) -> Result<(), Error> {
    let mut input = attachment_input(args)?;
    let issue = session.issue_brief(&args.issue)?;
    input.insert("issueId".into(), json!(need_str(&issue, "id", "Issues")?));
    let data = session.linear.run(
        &ops::ATTACHMENT_CREATE,
        json!({ "input": Value::Object(input) }),
    )?;
    out.emit(&payload(&data, "attachmentCreate", "attachment")?);
    Ok(())
}

fn issue_close(session: &Session, args: &CloseArgs, out: &Output) -> Result<(), Error> {
    let comment = body_from(&args.comment, &args.comment_file)?;
    let issue = session.issue(&args.issue, 1, 1, 1)?;
    let issue_id = need_str(&issue, "id", "IssueById")?;
    let team_key = need_str(&issue, "team.key", "IssueById")?;
    let state_id = match (&args.state, args.cancel) {
        (Some(name), _) => session.state_id(&team_key, name)?,
        (None, true) => session.state_of_type(&team_key, "canceled")?,
        (None, false) => session.state_of_type(&team_key, "completed")?,
    };
    if let Some(body) = comment {
        let data = session.linear.run(
            &ops::COMMENT_CREATE,
            json!({ "input": { "issueId": issue_id, "body": body } }),
        )?;
        out.emit(&payload(&data, "commentCreate", "comment")?);
    }
    let data = session.linear.run(
        &ops::ISSUE_UPDATE,
        json!({ "id": issue_id, "input": { "stateId": state_id } }),
    )?;
    out.emit(&payload(&data, "issueUpdate", "issue")?);
    Ok(())
}

fn issue_update(session: &Session, args: &UpdateArgs, out: &Output) -> Result<(), Error> {
    let mut input = title_and_description(
        args.title.as_deref(),
        &args.description,
        &args.description_file,
    )?;
    let issue = session.issue(&args.issue, 1, 1, 1)?;
    let issue_id = need_str(&issue, "id", "IssueById")?;
    let team_key = need_str(&issue, "team.key", "IssueById")?;
    if let Some(state) = &args.state {
        input.insert("stateId".into(), json!(session.state_id(&team_key, state)?));
    }
    let added = session.label_ids(&team_key, &args.add_label)?;
    if !added.is_empty() {
        input.insert("addedLabelIds".into(), json!(added));
    }
    let removed = session.label_ids(&team_key, &args.remove_label)?;
    if !removed.is_empty() {
        input.insert("removedLabelIds".into(), json!(removed));
    }
    if let Some(priority) = args.priority {
        input.insert("priority".into(), json!(priority));
    }
    if let Some(who) = &args.assignee {
        let value = if is_none(who) {
            Value::Null
        } else {
            json!(session.user_id(who)?)
        };
        input.insert("assigneeId".into(), value);
    }
    if let Some(project) = &args.project {
        let value = if is_none(project) {
            Value::Null
        } else {
            json!(session.project_id(project)?)
        };
        input.insert("projectId".into(), value);
    }
    if let Some(milestone) = &args.milestone {
        let value = if is_none(milestone) {
            Value::Null
        } else {
            // In the project `--project` names, else the issue's own.
            let project = match &args.project {
                Some(name) if !is_none(name) => name.clone(),
                _ => match str_at(&issue, "project.name") {
                    Some(name) => name.to_string(),
                    None => {
                        return Err(Error::with_hint(
                            ErrorCode::Usage,
                            format!(
                                "{} is in no project, so it can take no milestone",
                                str_at(&issue, "identifier").unwrap_or(&issue_id)
                            ),
                            "pass --project <NAME> with --milestone",
                        ));
                    }
                },
            };
            let stone = session.milestone_named(&project, milestone)?;
            json!(need_str(&stone, "id", "Milestones")?)
        };
        input.insert("projectMilestoneId".into(), value);
    }
    if let Some(cycle) = &args.cycle {
        let value = if is_none(cycle) {
            Value::Null
        } else {
            // In the issue's own team: a cycle belongs to one team.
            let team_key = need_str(&issue, "team.key", "IssueById")?;
            let cycle = session.cycle_ref(&team_key, cycle)?;
            json!(need_str(&cycle, "id", "Cycles")?)
        };
        input.insert("cycleId".into(), value);
    }
    if let Some(parent) = &args.parent {
        let value = if is_none(parent) {
            Value::Null
        } else {
            let parent_id = need_str(&session.issue_brief(parent)?, "id", "Issues")?;
            if parent_id == issue_id {
                return Err(Error::with_hint(
                    ErrorCode::Usage,
                    format!(
                        "{} cannot be its own parent",
                        str_at(&issue, "identifier").unwrap_or(&issue_id)
                    ),
                    "name another issue, or `none` to take it out from under its parent",
                ));
            }
            json!(parent_id)
        };
        input.insert("parentId".into(), value);
    }
    if input.is_empty() {
        return Err(Error::with_hint(
            ErrorCode::Usage,
            "nothing to change",
            "pass at least one of --title, --state, --add-label, --remove-label, --priority, --assignee, --project, --milestone, --cycle, --parent, --description, --description-file",
        ));
    }
    let data = session.linear.run(
        &ops::ISSUE_UPDATE,
        json!({ "id": issue_id, "input": Value::Object(input) }),
    )?;
    out.emit(&payload(&data, "issueUpdate", "issue")?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounds::BODY_MAX_BYTES;

    /// The bound on a flag's value is held here, in process, on every OS. The tests
    /// through the binary cannot reach it from argv on Linux: the kernel refuses one
    /// argument over 128 KiB (`MAX_ARG_STRLEN`) before muster runs, and
    /// `BODY_MAX_BYTES` is 256 KiB; macOS allows the argument, so a test that sent it
    /// passed there and failed on Linux.
    #[test]
    fn an_attachment_input_is_checked_before_any_request() {
        let attach = |url: &str, title: &str, subtitle: Option<&str>| AttachArgs {
            issue: "APP-1".to_string(),
            url: url.to_string(),
            title: title.to_string(),
            subtitle: subtitle.map(str::to_string),
        };
        let at_bound = "x".repeat(BODY_MAX_BYTES);
        let over = "x".repeat(BODY_MAX_BYTES + 1);
        let len = |input: &Map<String, Value>, field: &str| {
            input.get(field).and_then(Value::as_str).map(str::len)
        };
        let full =
            attachment_input(&attach(&at_bound, &at_bound, Some(&at_bound))).unwrap_or_default();
        assert_eq!(
            len(&full, "url"),
            Some(BODY_MAX_BYTES),
            "at the bound is not over it"
        );
        assert_eq!(len(&full, "title"), Some(BODY_MAX_BYTES));
        assert_eq!(len(&full, "subtitle"), Some(BODY_MAX_BYTES));
        let plain = attachment_input(&attach("https://example.com", "x", None)).unwrap_or_default();
        assert_eq!(
            plain.get("subtitle"),
            None,
            "a subtitle not passed is left out, never invented"
        );
        assert_eq!(
            plain.get("issueId"),
            None,
            "the issue is resolved after the checks, so no request precedes a refusal"
        );
        for (args, what) in [
            (attach(&over, "x", None), "the link"),
            (attach("https://example.com", &over, None), "the title"),
            (
                attach("https://example.com", "x", Some(&over)),
                "the subtitle",
            ),
        ] {
            let e = attachment_input(&args).err();
            assert_eq!(
                e.as_ref().map(|e| e.code),
                Some(ErrorCode::Refused),
                "{what}"
            );
            assert!(
                e.as_ref()
                    .is_some_and(|e| e.message.starts_with(what) && e.hint.contains(what)),
                "{what}: {e:?}"
            );
        }
        for args in [
            attach("", "x", None),
            attach("https://example.com", " ", None),
        ] {
            let e = attachment_input(&args).err();
            assert_eq!(e.as_ref().map(|e| e.code), Some(ErrorCode::Usage), "{e:?}");
            assert!(
                e.as_ref()
                    .is_some_and(|e| e.message.contains("must not be empty")),
                "{e:?}"
            );
        }
    }

    /// The same bound on `issue create --title`, `issue update --title` and their
    /// `--description`, for the same reason.
    #[test]
    fn a_title_and_a_description_are_checked_before_any_request() {
        let at_bound = "x".repeat(BODY_MAX_BYTES);
        let over = "x".repeat(BODY_MAX_BYTES + 1);
        let len = |input: &Map<String, Value>, field: &str| {
            input.get(field).and_then(Value::as_str).map(str::len)
        };
        // `issue create`: the title is required.
        let full = title_and_description(Some(&at_bound), &Some(at_bound.clone()), &None)
            .unwrap_or_default();
        assert_eq!(
            len(&full, "title"),
            Some(BODY_MAX_BYTES),
            "at the bound is not over it"
        );
        assert_eq!(len(&full, "description"), Some(BODY_MAX_BYTES));
        let e = title_and_description(Some(&over), &None, &None).err();
        assert_eq!(
            e.as_ref().map(|e| e.code),
            Some(ErrorCode::Refused),
            "{e:?}"
        );
        assert!(
            e.as_ref()
                .is_some_and(|e| e.message.starts_with("the title")),
            "{e:?}"
        );
        // `issue update`: the title is optional, and a flag not passed leaves its
        // field out.
        assert_eq!(
            title_and_description(None, &None, &None).ok(),
            Some(Map::new())
        );
        let only_title = title_and_description(Some("x"), &None, &None).unwrap_or_default();
        assert_eq!(only_title.get("description"), None);
        let e = title_and_description(None, &Some(over), &None).err();
        assert!(
            e.as_ref()
                .is_some_and(|e| e.code == ErrorCode::Refused && e.message.starts_with("the body")),
            "{e:?}"
        );
        let e =
            title_and_description(Some("x"), &None, &Some("/nonexistent/notes.md".into())).err();
        assert_eq!(e.as_ref().map(|e| e.code), Some(ErrorCode::Usage), "{e:?}");
    }

    #[test]
    fn a_project_input_is_bounded_in_process() {
        let at = ProjectCreateArgs {
            name: "n".repeat(BODY_MAX_BYTES),
            description: Some("d".repeat(BODY_MAX_BYTES)),
            allow_duplicate: false,
        };
        assert!(project_input(&at).is_ok(), "at the bound is not over it");
        let over = ProjectCreateArgs {
            name: "n".repeat(BODY_MAX_BYTES + 1),
            description: None,
            allow_duplicate: false,
        };
        let e = project_input(&over).err();
        assert_eq!(e.as_ref().map(|e| e.code), Some(ErrorCode::Refused));
        assert!(
            e.as_ref()
                .is_some_and(|e| e.message.starts_with("the name is")),
            "{e:?}"
        );
        assert_eq!(
            e.as_ref().map(|e| e.hint.as_str()),
            Some("shorten the name; the detail belongs in the description"),
            "the hint names what `project create` has, not another command's flag"
        );
        let long = ProjectCreateArgs {
            name: "ok".into(),
            description: Some("d".repeat(BODY_MAX_BYTES + 1)),
            allow_duplicate: false,
        };
        assert!(
            project_input(&long)
                .err()
                .is_some_and(|e| e.message.starts_with("the body is"))
        );
    }

    #[test]
    fn a_workspace_input_holds_only_what_was_passed() {
        use crate::cli::{Day, PullRequestIssueMode, Role};
        let e = workspace_input(&WorkspaceUpdateArgs::default()).err();
        assert_eq!(e.as_ref().map(|e| e.code), Some(ErrorCode::Usage), "{e:?}");
        assert!(
            e.is_some_and(|e| e.message == "nothing to change"),
            "no flag is a usage error before any request"
        );
        let args = WorkspaceUpdateArgs {
            name: Some("Renamed".into()),
            git_branch_format: Some(" None ".into()),
            roadmap: Some(false),
            project_update_reminders_day: Some(Day::Monday),
            project_update_reminders_hour: Some(9),
            pull_request_issue_mode: Some(PullRequestIssueMode::AutoOnMerge),
            invitations_role: Some(Role::Admin),
            allowed_auth_services: Some("ALL".into()),
            hide_non_primary_organizations: Some(true),
            ..WorkspaceUpdateArgs::default()
        };
        let input = workspace_input(&args).unwrap_or_default();
        assert_eq!(input.get("name"), Some(&json!("Renamed")));
        assert_eq!(
            input.get("gitBranchFormat"),
            Some(&Value::Null),
            "`none` clears the format"
        );
        assert_eq!(input.get("roadmapEnabled"), Some(&json!(false)));
        assert_eq!(
            input.get("feedEnabled"),
            None,
            "a flag left out leaves its setting alone"
        );
        assert_eq!(
            input.get("projectUpdateRemindersDay"),
            Some(&json!("Monday"))
        );
        assert_eq!(input.get("projectUpdateRemindersHour"), Some(&json!(9)));
        assert_eq!(
            input.get("pullRequestIssueMode"),
            Some(&json!("autoOnMerge"))
        );
        assert_eq!(
            at(
                &Value::Object(input.clone()),
                "securitySettings.invitationsRole"
            ),
            "admin"
        );
        assert_eq!(
            at(
                &Value::Object(input.clone()),
                "securitySettings.teamCreationRole"
            ),
            &Value::Null,
            "a role left out is not sent"
        );
        assert_eq!(
            at(
                &Value::Object(input.clone()),
                "authSettings.allowedAuthServices"
            ),
            &json!([]),
            "`all` is Linear's empty list"
        );
        assert_eq!(
            at(
                &Value::Object(input),
                "authSettings.hideNonPrimaryOrganizations"
            ),
            true
        );
        let some = WorkspaceUpdateArgs {
            allowed_auth_services: Some("google, saml,".into()),
            ..WorkspaceUpdateArgs::default()
        };
        let input = workspace_input(&some).unwrap_or_default();
        assert_eq!(
            at(&Value::Object(input), "authSettings.allowedAuthServices"),
            &json!(["google", "saml"])
        );
        for empty in ["", " ", ",", " , ,"] {
            let none = WorkspaceUpdateArgs {
                allowed_auth_services: Some(empty.into()),
                ..WorkspaceUpdateArgs::default()
            };
            let e = workspace_input(&none).err();
            assert_eq!(
                e.as_ref().map(|e| e.code),
                Some(ErrorCode::Usage),
                "{empty:?} is not `all`: {e:?}"
            );
            assert!(
                e.is_some_and(|e| e.message == "--allowed-auth-services names no service"),
                "{empty:?}"
            );
        }
        let long = WorkspaceUpdateArgs {
            name: Some("n".repeat(BODY_MAX_BYTES + 1)),
            ..WorkspaceUpdateArgs::default()
        };
        assert!(
            workspace_input(&long)
                .err()
                .is_some_and(|e| e.code == ErrorCode::Refused && e.message.starts_with("the name")),
        );
    }

    #[test]
    fn a_team_input_is_checked_before_any_request() {
        let args = TeamCreateArgs {
            name: "Smoke tests".into(),
            key: Some(" SMK ".into()),
            cycles_enabled: Some(true),
            ..TeamCreateArgs::default()
        };
        let input = team_input(&args).unwrap_or_default();
        assert_eq!(input.get("key"), Some(&json!("SMK")), "trimmed");
        assert_eq!(input.get("cyclesEnabled"), Some(&json!(true)));
        assert_eq!(input.get("timezone"), None, "a flag left out is not sent");
        assert_eq!(
            input.get("parentId"),
            None,
            "the parent is resolved after the checks"
        );
        let empty = TeamCreateArgs {
            name: "x".into(),
            key: Some(" ".into()),
            ..TeamCreateArgs::default()
        };
        assert_eq!(
            team_input(&empty).err().map(|e| e.code),
            Some(ErrorCode::Usage)
        );
        let long = TeamCreateArgs {
            name: "n".repeat(BODY_MAX_BYTES + 1),
            ..TeamCreateArgs::default()
        };
        assert!(
            team_input(&long)
                .err()
                .is_some_and(|e| e.code == ErrorCode::Refused)
        );
        assert!(
            delete_payload(
                &json!({ "teamDelete": { "success": true, "entityId": "x" } }),
                "teamDelete"
            )
            .is_ok_and(|v| v["entityId"] == "x")
        );
        assert_eq!(
            delete_payload(&json!({ "teamDelete": { "success": false } }), "teamDelete")
                .err()
                .map(|e| e.code),
            Some(ErrorCode::Refused)
        );
    }

    #[test]
    fn a_team_update_input_holds_only_what_was_passed() {
        use crate::cli::{EstimationType, TeamRole};
        assert_eq!(
            team_update_input(&TeamUpdateArgs::default()).ok(),
            Some(Map::new()),
            "no flag is an empty input; the handler refuses it after resolving the team"
        );
        let args = TeamUpdateArgs {
            name: Some("Smoke".into()),
            private: Some(true),
            cycle_start_day: Some(1),
            cycle_duration: Some(2),
            issue_estimation_type: Some(EstimationType::TShirt),
            auto_close_period: Some(" None ".into()),
            team_management_role: Some(TeamRole::Owner),
            ..TeamUpdateArgs::default()
        };
        let input = team_update_input(&args).unwrap_or_default();
        assert_eq!(input.get("name"), Some(&json!("Smoke")));
        assert_eq!(input.get("private"), Some(&json!(true)));
        assert_eq!(input.get("cycleStartDay"), Some(&json!(1)));
        assert_eq!(input.get("cycleDuration"), Some(&json!(2)));
        assert_eq!(
            input.get("cyclesEnabled"),
            None,
            "a flag left out is not sent"
        );
        assert_eq!(input.get("issueEstimationType"), Some(&json!("tShirt")));
        assert_eq!(
            input.get("autoClosePeriod"),
            Some(&Value::Null),
            "`none` clears the period"
        );
        assert_eq!(
            at(
                &Value::Object(input.clone()),
                "securitySettings.teamManagement"
            ),
            "owner"
        );
        assert_eq!(
            at(&Value::Object(input), "securitySettings.memberManagement"),
            &Value::Null
        );
        let months = TeamUpdateArgs {
            auto_close_period: Some("3".into()),
            ..TeamUpdateArgs::default()
        };
        assert_eq!(
            team_update_input(&months)
                .unwrap_or_default()
                .get("autoClosePeriod"),
            Some(&json!(3.0))
        );
        for period in ["soon", "nan", "inf", "-inf", "infinity", "-1"] {
            let bad = TeamUpdateArgs {
                auto_close_period: Some(period.into()),
                ..TeamUpdateArgs::default()
            };
            assert_eq!(
                team_update_input(&bad).err().map(|e| e.code),
                Some(ErrorCode::Usage),
                "--auto-close-period {period:?} is refused before any request, \
                 never sent as `null` or as a negative number"
            );
        }
        let blank = TeamUpdateArgs {
            key: Some(" ".into()),
            ..TeamUpdateArgs::default()
        };
        assert_eq!(
            team_update_input(&blank).err().map(|e| e.code),
            Some(ErrorCode::Usage)
        );
    }

    #[test]
    fn a_state_input_is_checked_before_any_request() {
        use crate::cli::StateType;
        let blocked = StateCreateArgs {
            name: "Blocked".into(),
            r#type: StateType::Started,
            color: "#eb5757".into(),
            ..StateCreateArgs::default()
        };
        let input = state_input(&blocked).unwrap_or_default();
        assert_eq!(input.get("name"), Some(&json!("Blocked")));
        assert_eq!(
            input.get("type"),
            Some(&json!("started")),
            "as Linear spells it"
        );
        assert_eq!(input.get("color"), Some(&json!("#eb5757")));
        assert_eq!(
            input.get("description"),
            None,
            "a flag left out is not sent"
        );
        assert_eq!(
            input.get("position"),
            None,
            "the default, the end, is Linear's to pick"
        );
        assert_eq!(
            input.get("teamId"),
            None,
            "the team is resolved after the checks"
        );
        let len = |input: &Map<String, Value>, field: &str| {
            input.get(field).and_then(Value::as_str).map(str::len)
        };
        let at_bound = StateCreateArgs {
            name: "n".repeat(BODY_MAX_BYTES),
            description: Some("d".repeat(BODY_MAX_BYTES)),
            position: Some(2.5),
            ..StateCreateArgs::default()
        };
        let full = state_input(&at_bound).unwrap_or_default();
        assert_eq!(
            len(&full, "name"),
            Some(BODY_MAX_BYTES),
            "at the bound is not over it"
        );
        assert_eq!(len(&full, "description"), Some(BODY_MAX_BYTES));
        assert_eq!(full.get("position"), Some(&json!(2.5)));
        for (name, description) in [
            ("n".repeat(BODY_MAX_BYTES + 1), None),
            ("n".into(), Some("d".repeat(BODY_MAX_BYTES + 1))),
        ] {
            let over = StateCreateArgs {
                name,
                description,
                ..StateCreateArgs::default()
            };
            assert!(
                state_input(&over)
                    .err()
                    .is_some_and(|e| e.code == ErrorCode::Refused)
            );
        }
        // `f64::from_str`, which `clap` parses `--position` with, takes these spellings,
        // and serde_json would write each as `null`.
        let huge = "1e400".parse::<f64>().ok();
        assert_eq!(
            huge,
            Some(f64::INFINITY),
            "a number too large for an f64 parses as infinity, not as an error"
        );
        for position in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            huge.unwrap_or(f64::NAN),
        ] {
            let create = StateCreateArgs {
                name: "Nowhere".into(),
                position: Some(position),
                ..StateCreateArgs::default()
            };
            assert_eq!(
                state_input(&create).err().map(|e| e.code),
                Some(ErrorCode::Usage),
                "--position {position} is refused before any request, never sent as `null`"
            );
            let update = StateUpdateArgs {
                state: "Blocked".into(),
                position: Some(position),
                ..StateUpdateArgs::default()
            };
            assert_eq!(
                state_update_input(&update).err().map(|e| e.code),
                Some(ErrorCode::Usage),
                "--position {position} is refused before any request, never sent as `null`"
            );
        }
        assert_eq!(
            state_update_input(&StateUpdateArgs {
                state: "Blocked".into(),
                ..StateUpdateArgs::default()
            })
            .err()
            .map(|e| e.code),
            Some(ErrorCode::Usage),
            "no flag is nothing to change"
        );
        let moved = StateUpdateArgs {
            state: "Blocked".into(),
            position: Some(2.5),
            ..StateUpdateArgs::default()
        };
        let input = state_update_input(&moved).unwrap_or_default();
        assert_eq!(input.get("position"), Some(&json!(2.5)));
        assert_eq!(input.get("name"), None, "a flag left out is not sent");
    }

    #[test]
    fn a_user_input_holds_only_what_was_passed() {
        let e = user_input(&UserUpdateArgs {
            who: "me".into(),
            ..UserUpdateArgs::default()
        })
        .err();
        assert_eq!(e.as_ref().map(|e| e.code), Some(ErrorCode::Usage), "{e:?}");
        let args = UserUpdateArgs {
            who: "me".into(),
            display_name: Some("rjae".into()),
            status_label: Some("Heads down".into()),
            status_until: Some("none".into()),
            ..UserUpdateArgs::default()
        };
        let input = user_input(&args).unwrap_or_default();
        assert_eq!(input.get("displayName"), Some(&json!("rjae")));
        assert_eq!(input.get("statusLabel"), Some(&json!("Heads down")));
        assert_eq!(
            input.get("statusUntilAt"),
            Some(&Value::Null),
            "`none` keeps the status"
        );
        assert_eq!(input.get("name"), None, "a flag left out is not sent");
        let long = UserUpdateArgs {
            who: "me".into(),
            description: Some("d".repeat(BODY_MAX_BYTES + 1)),
            ..UserUpdateArgs::default()
        };
        assert!(
            user_input(&long)
                .err()
                .is_some_and(|e| e.code == ErrorCode::Refused)
        );
        assert!(
            success_payload(
                &json!({ "userSuspend": { "success": true } }),
                "userSuspend"
            )
            .is_ok_and(|v| v["success"] == true)
        );
        assert_eq!(
            success_payload(
                &json!({ "userSuspend": { "success": false } }),
                "userSuspend"
            )
            .err()
            .map(|e| e.code),
            Some(ErrorCode::Refused)
        );
    }

    #[test]
    fn a_project_update_input_holds_only_what_was_passed() {
        let args = ProjectUpdateArgs {
            project: "Smoke tests".into(),
            name: Some("Smoke".into()),
            content: Some("# The plan".into()),
            start_date: Some("none".into()),
            target_date: Some(" 2026-12-01 ".into()),
            priority: Some(2),
            ..ProjectUpdateArgs::default()
        };
        let input = project_update_input(&args).unwrap_or_default();
        assert_eq!(input.get("name"), Some(&json!("Smoke")));
        assert_eq!(
            input.get("content"),
            Some(&json!("# The plan")),
            "--content inline, as it is"
        );
        assert_eq!(
            input.get("startDate"),
            Some(&Value::Null),
            "`none` clears a date"
        );
        assert_eq!(
            input.get("targetDate"),
            Some(&json!("2026-12-01")),
            "trimmed"
        );
        assert_eq!(input.get("priority"), Some(&json!(2)));
        assert_eq!(
            input.get("description"),
            None,
            "a flag left out is not sent"
        );
        assert_eq!(
            input.get("statusId"),
            None,
            "the status is resolved after the checks"
        );
        for long in [
            ProjectUpdateArgs {
                project: "x".into(),
                name: Some("n".repeat(BODY_MAX_BYTES + 1)),
                ..ProjectUpdateArgs::default()
            },
            ProjectUpdateArgs {
                project: "x".into(),
                content: Some("c".repeat(BODY_MAX_BYTES + 1)),
                ..ProjectUpdateArgs::default()
            },
        ] {
            assert!(
                project_update_input(&long)
                    .err()
                    .is_some_and(|e| e.code == ErrorCode::Refused),
                "a name or a content over BODY_MAX_BYTES is refused before any request"
            );
        }
    }

    #[test]
    fn an_initiative_input_holds_only_what_was_passed() {
        use crate::cli::InitiativeStatus;
        let input = initiative_input(
            Some("Growth"),
            &None,
            Some(InitiativeStatus::Active),
            &Some("none".into()),
            None,
            &Some("# The plan".into()),
            &None,
        )
        .unwrap_or_default();
        assert_eq!(input.get("name"), Some(&json!("Growth")));
        assert_eq!(input.get("status"), Some(&json!("Active")));
        assert_eq!(
            input.get("targetDate"),
            Some(&Value::Null),
            "`none` clears the date"
        );
        assert_eq!(
            input.get("content"),
            Some(&json!("# The plan")),
            "--content inline, as it is"
        );
        assert_eq!(
            input.get("description"),
            None,
            "a flag left out is not sent"
        );
        assert_eq!(input.get("priority"), None);
        assert!(
            initiative_input(
                None,
                &None,
                None,
                &None,
                None,
                &Some("c".repeat(BODY_MAX_BYTES + 1)),
                &None,
            )
            .err()
            .is_some_and(|e| e.code == ErrorCode::Refused),
            "a content over BODY_MAX_BYTES is refused before any request"
        );
    }

    #[test]
    fn a_project_update_needs_a_body_or_a_health() {
        use crate::cli::Health;
        let e = status_post_input(&ProjectUpdateCreateArgs {
            project: "Muster".into(),
            ..ProjectUpdateCreateArgs::default()
        })
        .err();
        assert_eq!(e.as_ref().map(|e| e.code), Some(ErrorCode::Usage), "{e:?}");
        let health_only = status_post_input(&ProjectUpdateCreateArgs {
            project: "Muster".into(),
            health: Some(Health::AtRisk),
            ..ProjectUpdateCreateArgs::default()
        })
        .unwrap_or_default();
        assert_eq!(health_only.get("health"), Some(&json!("atRisk")));
        assert_eq!(health_only.get("body"), None);
        let long = ProjectUpdateCreateArgs {
            project: "Muster".into(),
            body: Some("b".repeat(BODY_MAX_BYTES + 1)),
            ..ProjectUpdateCreateArgs::default()
        };
        assert!(
            status_post_input(&long)
                .err()
                .is_some_and(|e| e.code == ErrorCode::Refused)
        );
    }

    #[test]
    fn limits_are_bounded() {
        assert!(limit(0).is_err());
        assert!(limit(PAGE_CAP + 1).is_err());
        assert_eq!(limit(PAGE_CAP).ok(), Some(PAGE_CAP));
    }

    #[test]
    fn assignee_filters() {
        assert_eq!(assignee_filter("me"), json!({ "isMe": { "eq": true } }));
        assert_eq!(assignee_filter("none"), json!({ "null": true }));
        assert_eq!(at(&assignee_filter("a@b.c"), "email.eqIgnoreCase"), "a@b.c");
        assert!(assignee_filter("Rjae").get("or").is_some());
    }

    #[test]
    fn issue_clauses_and_the_none_sentinel() {
        let id = "1f979a91-84ec-4817-9397-3cdc6ca67612";
        let by_id = issue_clause(id, None).ok().flatten().unwrap_or_default();
        assert_eq!(at(&by_id, "id.eq"), id);
        let by_identifier = issue_clause(" app-12 ", None)
            .ok()
            .flatten()
            .unwrap_or_default();
        assert_eq!(at(&by_identifier, "number.eq"), 12);
        assert_eq!(at(&by_identifier, "team.key.eqIgnoreCase"), "app");
        assert!(
            issue_clause("12", None).ok().flatten().is_none(),
            "a bare number needs the team"
        );
        let by_number = issue_clause("12", Some("APP"))
            .ok()
            .flatten()
            .unwrap_or_default();
        assert_eq!(at(&by_number, "number.eq"), 12);
        assert_eq!(at(&by_number, "team.key.eq"), "APP");
        assert!(issue_clause("Try muster", None).is_err());
        assert_eq!(
            parent_clause("none", "APP").ok(),
            Some(json!({ "null": true }))
        );
        assert!(is_none(" None "));
        assert!(!is_none("nonesuch"));
    }

    #[test]
    fn payloads() {
        assert!(
            payload(
                &json!({ "issueCreate": { "success": true, "issue": { "id": "x" } } }),
                "issueCreate",
                "issue"
            )
            .is_ok()
        );
        assert_eq!(
            payload(
                &json!({ "issueCreate": { "success": false, "issue": null } }),
                "issueCreate",
                "issue"
            )
            .map_err(|e| e.code),
            Err(ErrorCode::Refused)
        );
        assert_eq!(
            payload(
                &json!({ "issueCreate": { "success": true } }),
                "issueCreate",
                "issue"
            )
            .map_err(|e| e.code),
            Err(ErrorCode::Failed)
        );
    }
}
