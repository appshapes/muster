//! What each command does: resolve names to ids, run the operations, emit Linear's
//! answers (ADR 0007, ADR 0008). Every handler writes through [`Output`] as it goes and
//! returns the first failure.

use serde_json::{Map, Value, json};

use crate::bounds::{
    ATTACHMENTS_PER_ISSUE, CHILDREN_PER_ISSUE, COMMENTS_PER_ISSUE, PAGE_CAP, TEAMS_PER_PROJECT,
};
use crate::cli::{
    AttachArgs, Cli, CloseArgs, Command, CommentArgs, CreateArgs, GetArgs, IssueCommand,
    IssueListArgs, ListArgs, ListCommand, MemberArgs, MemberCommand, ProjectCommand,
    ProjectCreateArgs, TeamCommand, TeamCreateArgs, TeamRefArgs, TeamUpdateArgs, UpdateArgs,
    WorkspaceCommand, WorkspaceUpdateArgs,
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
        Command::State {
            command: ListCommand::List(args),
        } => list_states(&Session::open(&config)?, args, out),
        Command::Label {
            command: ListCommand::List(args),
        } => list_labels(&Session::open(&config)?, args, out),
        Command::Project { command } => {
            let session = Session::open(&config)?;
            match command {
                ProjectCommand::List(args) => list_projects(&session, args, out),
                ProjectCommand::Create(args) => project_create(&session, args, out),
            }
        }
        Command::User {
            command: ListCommand::List(args),
        } => list_users(&Session::open(&config)?, args, out),
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
        let data = self
            .linear
            .run(&ops::USERS, json!({ "first": 2, "filter": filter }))?;
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
    /// the same).
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
            let data = self.linear.run(
                &ops::ISSUES,
                json!({ "first": 1, "filter": filter, "orderBy": "createdAt" }),
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
/// `authSettings`, as Linear nests them; `all` is Linear's empty list. No flag at all
/// is a usage error before any request.
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
            services
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect()
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

/// `member remove`: the membership is resolved from the team's list, up to
/// `PAGE_CAP`, before the one mutation; a user who is not a member is exit 4.
fn member_remove(session: &Session, args: &MemberArgs, out: &Output) -> Result<(), Error> {
    let key = session.team_key()?;
    let user_id = session.user_id(&args.user)?;
    let team = session.team_members(&key, PAGE_CAP)?;
    let found = nodes(&team, "memberships")
        .into_iter()
        .find(|m| str_at(m, "user.id") == Some(user_id.as_str()));
    let Some(membership) = found else {
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
            let months: f64 = period.trim().parse().map_err(|_| {
                Error::with_hint(
                    ErrorCode::Usage,
                    format!("--auto-close-period {period:?} is not a number of months"),
                    "pass a number of months, or `none` to never close an idle issue",
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
    let key = session.team_key_of(&args.key_of)?;
    let team = session.team_full(&key, false)?;
    let id = need_str(&team, "id", "TeamByKey")?;
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

fn list_users(session: &Session, args: &ListArgs, out: &Output) -> Result<(), Error> {
    let n = limit(args.limit)?;
    let data = session
        .linear
        .run(&ops::USERS, json!({ "first": n, "filter": Value::Null }))?;
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

/// Whether a flag's value is the `none` sentinel of `--assignee`, `--project` and
/// `--parent`.
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
            "pass at least one of --title, --state, --add-label, --remove-label, --priority, --assignee, --project, --parent, --description, --description-file",
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
        let bad = TeamUpdateArgs {
            auto_close_period: Some("soon".into()),
            ..TeamUpdateArgs::default()
        };
        assert_eq!(
            team_update_input(&bad).err().map(|e| e.code),
            Some(ErrorCode::Usage)
        );
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
