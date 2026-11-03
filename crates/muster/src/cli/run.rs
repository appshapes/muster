//! What each command does: resolve names to ids, run the operations, emit Linear's
//! answers (ADR 0007, ADR 0008). Every handler writes through [`Output`] as it goes and
//! returns the first failure.

use serde_json::{Map, Value, json};

use crate::bounds::{ATTACHMENTS_PER_ISSUE, CHILDREN_PER_ISSUE, COMMENTS_PER_ISSUE, PAGE_CAP};
use crate::cli::{
    AttachArgs, Cli, CloseArgs, Command, CommentArgs, CreateArgs, GetArgs, IssueCommand,
    IssueListArgs, ListArgs, ListCommand, UpdateArgs,
};
use crate::config::{Config, KeySource, TeamSource};
use crate::error::{Error, ErrorCode};
use crate::linear::Linear;
use crate::linear::operations as ops;
use crate::linear::select::{
    Filter, at, find_by, has_next_page, is_identifier, is_uuid, names, need_str, nodes, str_at,
};
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
        Command::Team {
            command: ListCommand::List(args),
        } => list_teams(&Session::open(&config)?, args, out),
        Command::State {
            command: ListCommand::List(args),
        } => list_states(&Session::open(&config)?, args, out),
        Command::Label {
            command: ListCommand::List(args),
        } => list_labels(&Session::open(&config)?, args, out),
        Command::Project {
            command: ListCommand::List(args),
        } => list_projects(&Session::open(&config)?, args, out),
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

    fn project_id(&self, name: &str) -> Result<String, Error> {
        let data = self.linear.run(
            &ops::PROJECTS,
            json!({ "first": PAGE_CAP, "filter": { "name": { "eqIgnoreCase": name } } }),
        )?;
        let projects = nodes(&data, "projects");
        match projects.first() {
            Some(project) => need_str(project, "id", "Projects"),
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
    let data = session
        .linear
        .run(&ops::PROJECTS, json!({ "first": n, "filter": Value::Null }))?;
    emit_list(
        out,
        nodes(&data, "projects"),
        has_next_page(&data, "projects"),
        "projects",
        n,
    );
    Ok(())
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
            ));
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
