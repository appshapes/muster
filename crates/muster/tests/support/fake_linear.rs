//! A fake Linear for the tests and the quickstart (ADR 0010): an HTTP/1.1 server on a
//! loopback port that reads each POST's JSON, dispatches on the `operationName`
//! muster sends, and answers from an in-memory workspace. Mutations change the
//! workspace, so a test creates, reads, comments on and closes an issue through the
//! binary and sees each step in the next. It refuses a wrong key as Linear does, answers
//! an unknown issue as Linear does, and through [`Fake::fail_next`] answers one named
//! operation with a rate limit, a server error, or by closing the connection after
//! reading the request. The fixture test holds its answers to recorded real shapes.
//!
//! Shared by `tests/cli.rs` (in process) and `examples/fake-linear.rs` (a process).
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use muster::bounds::{ATTACHMENTS_PER_ISSUE, CHILDREN_PER_ISSUE, COMMENTS_PER_ISSUE};
use serde_json::{Value, json};

/// The key the fake accepts.
pub const KEY: &str = "lin_api_fake";

/// How the fake can fail one named operation, once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// Read the request, then close the connection without answering.
    Drop,
    /// Answer HTTP 429 with Linear's `RATELIMITED` error and the reset header.
    RateLimit,
    /// Answer HTTP 502 with a text body, as a proxy would.
    ServerError,
}

struct Workspace {
    counter: u64,
    organization: Value,
    teams: Vec<Value>,
    states: Vec<Value>,
    labels: Vec<Value>,
    projects: Vec<Value>,
    users: Vec<Value>,
    issues: Vec<Value>,
    comments: Vec<Value>,
    attachments: Vec<Value>,
    memberships: Vec<Value>,
    /// The workspace's project statuses, in position order.
    project_statuses: Vec<Value>,
    /// The milestones of every project, each with its `projectId`.
    milestones: Vec<Value>,
    /// The status posts of every project, each with its `projectId`, oldest first.
    project_updates: Vec<Value>,
    /// The initiatives, with `content` and the fake's archive flag.
    initiatives: Vec<Value>,
    /// The initiative-to-project links, each with `initiativeId` and `projectId`.
    initiative_links: Vec<Value>,
    /// The cycles of every team, each with its `team` by id and key, newest first.
    cycles: Vec<Value>,
    /// The documents, with `content`, newest first.
    documents: Vec<Value>,
    /// The templates, the team's and the workspace's, newest first.
    templates: Vec<Value>,
    /// The webhooks, newest first, each with its `secret`, which no answer carries.
    webhooks: Vec<Value>,
    /// The audit log, newest first, as Linear orders it.
    audit_entries: Vec<Value>,
}

struct State {
    ws: Workspace,
    fail_next: BTreeMap<String, Failure>,
    operations: Vec<String>,
    /// Every request body as the binary sent it: `query`, `operationName` and
    /// `variables`, so that a test can assert on what went over the wire.
    requests: Vec<Value>,
}

/// A running fake.
pub struct Fake {
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Fake {
    /// Start on a free loopback port, with one team, `APP`.
    pub fn start() -> std::io::Result<Fake> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        let state = Arc::new(Mutex::new(State {
            ws: Workspace::seed(),
            fail_next: BTreeMap::new(),
            operations: Vec::new(),
            requests: Vec::new(),
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let (state2, stop2) = (Arc::clone(&state), Arc::clone(&stop));
        let thread = std::thread::spawn(move || {
            for stream in listener.incoming() {
                if stop2.load(Ordering::SeqCst) {
                    break;
                }
                if let Ok(mut stream) = stream {
                    let _ = handle(&mut stream, &state2);
                }
            }
        });
        Ok(Fake {
            addr,
            state,
            stop,
            thread: Some(thread),
        })
    }

    /// The endpoint for `MUSTER_ENDPOINT`.
    pub fn endpoint(&self) -> String {
        format!("http://{}/graphql", self.addr)
    }

    /// Fail the next request for this operation in this way.
    pub fn fail_next(&self, operation: &str, failure: Failure) {
        self.state
            .lock()
            .unwrap()
            .fail_next
            .insert(operation.to_string(), failure);
    }

    /// Every operation name seen, in order.
    pub fn operations(&self) -> Vec<String> {
        self.state.lock().unwrap().operations.clone()
    }

    /// Every request body seen, in order, as the binary sent it: `query`,
    /// `operationName` and `variables`. A test that says muster never sends something
    /// asserts on these, the bytes on the wire, not on the operation names.
    pub fn requests(&self) -> Vec<Value> {
        self.state.lock().unwrap().requests.clone()
    }

    /// Add a team, so that the workspace has several.
    pub fn add_team(&self, key: &str, name: &str) {
        let mut st = self.state.lock().unwrap();
        let team = st.ws.new_team(key, name);
        st.ws.teams.push(team);
    }

    /// An issue by identifier, as the fake holds it.
    pub fn issue(&self, identifier: &str) -> Option<Value> {
        let st = self.state.lock().unwrap();
        st.ws
            .issues
            .iter()
            .find(|i| i["identifier"] == identifier)
            .map(|i| {
                st.ws.full_issue(
                    i,
                    COMMENTS_PER_ISSUE,
                    ATTACHMENTS_PER_ISSUE,
                    CHILDREN_PER_ISSUE,
                )
            })
    }

    /// Add a project that spans `n` new teams, for the bound test.
    pub fn add_project_across_teams(&self, name: &str, n: usize) {
        let mut st = self.state.lock().unwrap();
        let mut teams = Vec::new();
        for k in 1..=n {
            let team = st.ws.new_team(&format!("T{k}"), &format!("Team {k}"));
            teams.push(json!({ "id": team["id"], "key": team["key"] }));
            st.ws.teams.push(team);
        }
        let backlog = st.ws.project_statuses[0].clone();
        let team_values: Vec<Value> = teams
            .iter()
            .filter_map(|t| st.ws.teams.iter().find(|x| x["id"] == t["id"]).cloned())
            .collect();
        let project = st.ws.new_project(name, "", &team_values, &backlog);
        st.ws.projects.push(project);
    }

    /// Put `n` new projects, named `prefix 1` to `prefix n`, in an initiative, for the
    /// bound tests of `initiative get` and `initiative remove-project`; the prefix
    /// keeps the names unique across calls.
    pub fn add_initiative_projects(&self, initiative: &str, prefix: &str, n: usize) {
        let mut st = self.state.lock().unwrap();
        let initiative_id = st
            .ws
            .initiatives
            .iter()
            .find(|i| i["name"] == initiative)
            .map(|i| i["id"].clone())
            .expect("the initiative exists");
        let team = st.ws.teams[0].clone();
        let backlog = st.ws.project_statuses[0].clone();
        for k in 1..=n {
            let project = st.ws.new_project(
                &format!("{prefix} {k}"),
                "for the bound",
                std::slice::from_ref(&team),
                &backlog,
            );
            let project_id = project["id"].clone();
            st.ws.projects.push(project);
            let (id, now) = (st.ws.id(), st.ws.now());
            st.ws.initiative_links.push(json!({ "id": id, "createdAt": now, "initiativeId": initiative_id, "projectId": project_id }));
        }
    }

    /// Put `n` new issues of the team in its cycle `number`, for the bound test of
    /// `cycle get`.
    pub fn add_cycle_issues(&self, team_key: &str, number: f64, n: usize) {
        let mut st = self.state.lock().unwrap();
        let cycle = st
            .ws
            .cycles
            .iter()
            .find(|c| c["team"]["key"] == team_key && c["number"].as_f64() == Some(number))
            .map(|c| json!({ "id": c["id"], "number": c["number"], "name": c["name"] }))
            .expect("the cycle exists");
        for k in 1..=n {
            let mut issue = st.ws.new_issue(
                team_key,
                &format!("In the cycle {k}"),
                None,
                "Todo",
                &[],
                0,
                None,
                None,
                None,
            );
            issue["cycle"] = cycle.clone();
            st.ws.issues.push(issue);
        }
    }

    /// Add `n` new users as members of a project, for the bound tests.
    pub fn add_project_members(&self, project: &str, n: usize) {
        let mut st = self.state.lock().unwrap();
        let index = st
            .ws
            .projects
            .iter()
            .position(|p| p["name"] == project)
            .expect("the project exists");
        let mut ids: Vec<Value> = st.ws.projects[index]["memberIds"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for k in 1..=n {
            let user = st.ws.new_user(
                &format!("Member {k}"),
                &format!("member{k}"),
                &format!("member{k}@example.com"),
                false,
                false,
            );
            ids.push(user["id"].clone());
        }
        st.ws.projects[index]["memberIds"] = json!(ids);
    }

    /// Add `n` milestones to a project, for the bound tests.
    pub fn add_project_milestones(&self, project: &str, n: usize) {
        let mut st = self.state.lock().unwrap();
        let project = st
            .ws
            .projects
            .iter()
            .find(|p| p["name"] == project)
            .cloned()
            .expect("the project exists");
        for k in 1..=n {
            st.ws.new_milestone(
                &project,
                &format!("Milestone {k}"),
                None,
                Some("2026-12-01"),
            );
        }
    }

    /// Add `n` links to an issue, for the bound tests.
    pub fn add_attachments(&self, issue: &str, n: usize) {
        let mut st = self.state.lock().unwrap();
        let id = st
            .ws
            .issues
            .iter()
            .find(|i| i["identifier"] == issue)
            .map(|i| i["id"].clone())
            .expect("the issue exists");
        for k in 1..=n {
            st.ws.new_attachment(
                &id,
                &format!("Link {k}"),
                None,
                &format!("https://example.com/{k}"),
            );
        }
    }

    /// Add `n` sub-issues under an issue, in its team, for the bound tests.
    pub fn add_sub_issues(&self, parent: &str, n: usize) {
        let mut st = self.state.lock().unwrap();
        let parent = st
            .ws
            .issues
            .iter()
            .find(|i| i["identifier"] == parent)
            .cloned()
            .expect("the parent exists");
        let team_key = parent["team"]["key"].as_str().unwrap().to_string();
        for k in 1..=n {
            let child = st.ws.new_issue(
                &team_key,
                &format!("Part {k}"),
                None,
                "Todo",
                &[],
                0,
                None,
                None,
                Some(&parent),
            );
            st.ws.issues.push(child);
        }
    }

    /// Add `n` members to a team, each a new user `Member k` with the email
    /// `member-k@example.com`, for the bound test (ADR 0014, group 3).
    pub fn add_members(&self, team_key: &str, n: usize) {
        let mut st = self.state.lock().unwrap();
        let team_id = st
            .ws
            .team_by_key(team_key)
            .map(|t| t["id"].clone())
            .expect("the team exists");
        for k in 1..=n {
            let id = st.ws.id();
            let user = json!({ "id": id, "name": format!("Member {k}"), "displayName": format!("member-{k}"), "email": format!("member-{k}@example.com"), "active": true, "admin": false, "isMe": false });
            st.ws.users.push(user.clone());
            st.ws.new_membership(&team_id, &user["id"], false);
        }
    }

    /// Add `n` webhooks of a team, each through `new_webhook` so the store stays newest
    /// first, labelled `Hook k` at `https://example.com/hooks/k`, for the bound test
    /// (ADR 0014, group 16): with `PAGE_CAP` added, the seeded "Fake hook" is past the
    /// one page of `PAGE_CAP` that `<WEBHOOK>` and the duplicate guard read.
    pub fn add_webhooks(&self, team_key: &str, n: usize) {
        let mut st = self.state.lock().unwrap();
        let team_id = st
            .ws
            .team_by_key(team_key)
            .map(|t| t["id"].clone())
            .expect("the team exists");
        for k in 1..=n {
            st.ws
                .new_webhook(&json!({
                    "label": format!("Hook {k}"), "url": format!("https://example.com/hooks/{k}"),
                    "resourceTypes": ["Issue"], "teamId": team_id,
                }))
                .expect("the team exists");
        }
    }

    /// The comments of an issue, oldest first.
    pub fn comments(&self, identifier: &str) -> Vec<Value> {
        let st = self.state.lock().unwrap();
        let id = st
            .ws
            .issues
            .iter()
            .find(|i| i["identifier"] == identifier)
            .map(|i| i["id"].clone());
        st.ws
            .comments
            .iter()
            .filter(|c| Some(&c["issueId"]) == id.as_ref())
            .cloned()
            .collect()
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn handle(stream: &mut TcpStream, state: &Arc<Mutex<State>>) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut headers = BTreeMap::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    let len: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0; len];
    reader.read_exact(&mut body)?;
    let request: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let op = request["operationName"].as_str().unwrap_or("").to_string();
    let mut st = state.lock().unwrap();
    st.operations.push(op.clone());
    st.requests.push(request.clone());
    let n = st.operations.len();
    if let Some(failure) = st.fail_next.remove(&op) {
        return match failure {
            Failure::Drop => Ok(()),
            Failure::RateLimit => respond(
                stream,
                429,
                n,
                &json!({ "errors": [ { "message": "Rate limit exceeded", "extensions": { "type": "ratelimited", "code": "RATELIMITED", "statusCode": 429, "userError": true } } ] }).to_string(),
                "application/json; charset=utf-8",
            ),
            Failure::ServerError => respond(stream, 502, n, "<html>Bad Gateway</html>", "text/html"),
        };
    }
    if headers.get("authorization").map(String::as_str) != Some(KEY) {
        return respond(
            stream,
            401,
            n,
            &json!({ "errors": [ { "message": "Authentication required, not authenticated", "extensions": { "type": "authentication error", "code": "AUTHENTICATION_ERROR", "statusCode": 401, "userError": true } } ] }).to_string(),
            "application/json; charset=utf-8",
        );
    }
    let (status, answer) = st.ws.dispatch(&op, &request["variables"]);
    respond(
        stream,
        status,
        n,
        &answer.to_string(),
        "application/json; charset=utf-8",
    )
}

fn respond(
    stream: &mut TcpStream,
    status: u16,
    n: usize,
    body: &str,
    content_type: &str,
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        429 => "Too Many Requests",
        502 => "Bad Gateway",
        _ => "Unknown",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nx-request-id: fake-{n}\r\nx-ratelimit-requests-limit: 2500\r\nx-ratelimit-requests-remaining: {}\r\nx-ratelimit-requests-reset: 1793670142609\r\nConnection: close\r\n\r\n{body}",
        body.len(),
        2500 - n as u64
    )?;
    stream.flush()
}

/// Linear's answer to a request that left a required variable out, as the `$teams: Int!`
/// of `Projects` and `ProjectCreate`: a GraphQL validation error, which muster reports
/// as exit 1 (it is muster's own mistake, not Linear's).
fn missing_variable(name: &str, kind: &str) -> (u16, Value) {
    (
        400,
        json!({ "errors": [ { "message": format!("Variable \"${name}\" of required type \"{kind}\" was not provided."), "extensions": { "code": "GRAPHQL_VALIDATION_FAILED", "type": "graphql error", "userError": true } } ] }),
    )
}

fn not_found(what: &str) -> (u16, Value) {
    (
        200,
        json!({ "errors": [ { "message": format!("Entity not found: {what}"), "path": [], "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": format!("Could not find referenced {what}.") } } ], "data": null }),
    )
}

fn eq_ic(a: &Value, b: &Value) -> bool {
    match (a.as_str(), b.as_str()) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
        _ => a == b,
    }
}

/// Whether `value` passes a comparator object such as `{"eq": "x"}`.
fn compare(value: &Value, comparator: &Value) -> bool {
    let Some(map) = comparator.as_object() else {
        return true;
    };
    map.iter().all(|(op, want)| match op.as_str() {
        "eq" => value == want,
        "neq" => value != want,
        "eqIgnoreCase" => eq_ic(value, want),
        "in" => want
            .as_array()
            .is_some_and(|items| items.iter().any(|w| w == value)),
        "nin" => want
            .as_array()
            .is_some_and(|items| !items.iter().any(|w| w == value)),
        "containsIgnoreCase" => match (value.as_str(), want.as_str()) {
            (Some(v), Some(w)) => v.to_ascii_lowercase().contains(&w.to_ascii_lowercase()),
            _ => false,
        },
        "contains" => match (value.as_str(), want.as_str()) {
            (Some(v), Some(w)) => v.contains(w),
            _ => false,
        },
        "gt" | "gte" | "lt" | "lte" => {
            // Numbers as numbers; else strings, which orders ISO times as Linear does.
            let ordering = match (value.as_f64(), want.as_f64()) {
                (Some(v), Some(w)) => v.partial_cmp(&w),
                _ => match (value.as_str(), want.as_str()) {
                    (Some(v), Some(w)) => Some(v.cmp(w)),
                    _ => None,
                },
            };
            match (op.as_str(), ordering) {
                (_, None) => false,
                ("gt", Some(o)) => o == std::cmp::Ordering::Greater,
                ("gte", Some(o)) => o != std::cmp::Ordering::Less,
                ("lt", Some(o)) => o == std::cmp::Ordering::Less,
                (_, Some(o)) => o != std::cmp::Ordering::Greater,
            }
        }
        "null" => want.as_bool().is_some_and(|n| n == value.is_null()),
        _ => true,
    })
}

/// How many nodes a collection clause of a filter sees, Linear's largest page.
const PAGE_CAP_FOR_FILTERS: usize = 250;

/// Whether an object passes a filter: each key is a field (with nested filters for
/// objects), `and`/`or` combine, and a `nodes` list under a field matches if any node does.
fn passes(object: &Value, filter: &Value) -> bool {
    let Some(map) = filter.as_object() else {
        return true;
    };
    map.iter().all(|(key, clause)| match key.as_str() {
        "and" => clause
            .as_array()
            .is_some_and(|fs| fs.iter().all(|f| passes(object, f))),
        "or" => clause
            .as_array()
            .is_some_and(|fs| fs.iter().any(|f| passes(object, f))),
        "isMe" => compare(&object["isMe"], clause),
        "null" => clause.as_bool().is_some_and(|n| n == object.is_null()),
        field => {
            let value = &object[field];
            if let Some(map) = clause.as_object()
                && ["length", "some", "every"]
                    .iter()
                    .any(|k| map.contains_key(*k))
            {
                // A collection filter, as Linear's: `length` against the node count,
                // `some` and `every` against the nodes.
                let items = value["nodes"].as_array().cloned().unwrap_or_default();
                return map.iter().all(|(k, c)| match k.as_str() {
                    "length" => compare(&json!(items.len()), c),
                    "some" => items.iter().any(|n| passes(n, c)),
                    "every" => items.iter().all(|n| passes(n, c)),
                    _ => true,
                });
            }
            let is_comparator = clause.as_object().is_some_and(|m| {
                m.keys().all(|k| {
                    matches!(
                        k.as_str(),
                        "eq" | "neq"
                            | "eqIgnoreCase"
                            | "in"
                            | "nin"
                            | "containsIgnoreCase"
                            | "contains"
                            | "gt"
                            | "gte"
                            | "lt"
                            | "lte"
                            | "null"
                    )
                })
            });
            if is_comparator {
                compare(value, clause)
            } else if let Some(nodes) = value["nodes"].as_array() {
                nodes.iter().any(|n| passes(n, clause))
            } else if value.is_null() {
                clause.get("null").and_then(Value::as_bool) == Some(true)
            } else {
                passes(value, clause)
            }
        }
    })
}

impl Workspace {
    fn seed() -> Workspace {
        let mut ws = Workspace {
            counter: 0,
            organization: Value::Null,
            teams: Vec::new(),
            states: Vec::new(),
            labels: Vec::new(),
            projects: Vec::new(),
            users: Vec::new(),
            issues: Vec::new(),
            comments: Vec::new(),
            attachments: Vec::new(),
            memberships: Vec::new(),
            project_statuses: Vec::new(),
            milestones: Vec::new(),
            project_updates: Vec::new(),
            initiatives: Vec::new(),
            initiative_links: Vec::new(),
            cycles: Vec::new(),
            documents: Vec::new(),
            templates: Vec::new(),
            webhooks: Vec::new(),
            audit_entries: Vec::new(),
        };
        // Every field `Workspace` asks for, with the values a Free workspace shows; the
        // fixture holds this to the recorded shape (ADR 0010; ADR 0014, group 1).
        ws.organization = json!({
            "id": ws.id(), "name": "Fake Workspace", "urlKey": "fake", "logoUrl": "https://example.com/logo.png",
            "createdAt": "2026-11-02T10:00:00.000Z", "updatedAt": "2026-11-02T10:00:00.000Z",
            "gitBranchFormat": null, "gitLinkbackMessagesEnabled": true, "gitPublicLinkbackMessagesEnabled": true,
            "gitLinkbackDescriptionsEnabled": false, "suggestedBranchNameEnabled": true,
            "roadmapEnabled": false, "feedEnabled": false, "pullRequestTourEnabled": false, "pullRequestIssueMode": "suggest",
            "aiThreadSummariesEnabled": false, "aiDiscussionSummariesEnabled": false,
            "projectUpdateReminderFrequencyInWeeks": null, "projectUpdateRemindersDay": "Friday", "projectUpdateRemindersHour": 14,
            "initiativeUpdateReminderFrequencyInWeeks": null, "initiativeUpdateRemindersDay": "Friday", "initiativeUpdateRemindersHour": 14,
            "fiscalYearStartMonth": 0, "defaultHomeView": null,
            "samlEnabled": false, "scimEnabled": false, "hipaaComplianceEnabled": false, "releaseChannel": "public",
            "securitySettings": {}, "authSettings": {}, "allowedFileUploadContentTypes": null,
            "previousUrlKeys": [], "deletionRequestedAt": null, "trialStartsAt": null, "trialEndsAt": null,
            "userCount": 2, "createdIssueCount": 4, "subscription": null,
        });
        // Every field `UserFields` asks for (ADR 0014, group 4), with the values a
        // workspace shows for its admin and for a member.
        let me = ws.new_user("Fake Person", "fake", "fake@example.com", true, true);
        let other = ws.new_user("Other Person", "other", "other@example.com", false, false);
        for (name, color) in [
            ("Bug", "#eb5757"),
            ("Improvement", "#4ea7fc"),
            ("Feature", "#bb87fc"),
        ] {
            let label = ws.new_label(name, color, None, None, false);
            ws.labels.push(label);
        }
        let mut team = ws.new_team("APP", "Fake Team");
        // The seeded team uses cycles: one past, one active, one to come, so that every
        // cycle answer has a node and `--cycle active` and `next` resolve (ADR 0014,
        // group 13).
        team["cyclesEnabled"] = json!(true);
        let past = ws.new_cycle(
            &team,
            1.0,
            "2026-10-19T00:00:00.000Z",
            "2026-10-26T00:00:00.000Z",
            Some("2026-10-26T00:00:00.000Z"),
            (false, false, true, false, true),
        );
        let active = ws.new_cycle(
            &team,
            2.0,
            "2026-10-26T00:00:00.000Z",
            "2026-11-02T00:00:00.000Z",
            None,
            (true, false, false, false, false),
        );
        let next = ws.new_cycle(
            &team,
            3.0,
            "2026-11-02T00:00:00.000Z",
            "2026-11-09T00:00:00.000Z",
            None,
            (false, true, false, true, false),
        );
        let _ = (&past, &next);
        team["activeCycle"] =
            json!({ "id": active["id"], "number": active["number"], "name": active["name"] });
        ws.teams.push(team.clone());
        // The seeded people are the team's members, the first its owner (ADR 0014,
        // group 3).
        ws.new_membership(&team["id"], &me["id"], true);
        ws.new_membership(&team["id"], &other["id"], false);
        let team_label = ws.new_label(
            "Release",
            "#26b5ce",
            Some("a team label"),
            Some(&team),
            false,
        );
        ws.labels.push(team_label);
        // Linear's default project statuses, as the appshapes workspace lists them.
        for (pos, (name, kind, color, indefinite)) in [
            ("Backlog", "backlog", "#bec2c8", true),
            ("Planned", "planned", "#4ea7fc", true),
            ("In Progress", "started", "#f2c94c", false),
            ("Completed", "completed", "#5e6ad2", true),
            ("Canceled", "canceled", "#95a2b3", true),
        ]
        .iter()
        .enumerate()
        {
            let sid = ws.id();
            ws.project_statuses.push(json!({ "id": sid, "name": name, "type": kind, "color": color, "description": null, "position": pos as f64, "indefinite": indefinite, "archivedAt": null }));
        }
        let planned = ws.project_statuses[1].clone();
        let mut project = ws.new_project(
            "Muster",
            "the fake project",
            std::slice::from_ref(&team),
            &planned,
        );
        project["lead"] =
            json!({ "id": me["id"], "name": me["name"], "displayName": me["displayName"] });
        project["memberIds"] = json!([me["id"]]);
        ws.projects.push(project.clone());
        // One document in the project, so that every document answer has a node and
        // the recorder finds one (ADR 0014, group 14).
        let _ = ws
            .new_document(
                "Plan",
                &json!({ "content": "# Plan\n\nThe fake project's plan.", "projectId": project["id"] }),
            )
            .expect("the seeded project exists");
        // One issue template the workspace shares, so that every template answer has a
        // node and the recorder finds one (ADR 0014, group 15).
        let _ = ws
            .new_template(&json!({
                "type": "issue", "name": "Bug report", "description": "For a bug.",
                "templateData": { "title": "Bug: ", "description": "## Steps\n\n1. " },
            }))
            .expect("a workspace template needs no team");
        // One webhook of the team, so that every webhook answer has a node and the
        // recorder finds one (ADR 0014, group 16).
        let _ = ws
            .new_webhook(&json!({
                "label": "Fake hook", "url": "https://example.com/hooks/fake", "enabled": true,
                "resourceTypes": ["Issue"], "teamId": team["id"], "secret": "seeded-secret",
            }))
            .expect("the seeded team exists");
        // Three audit entries, so that every audit answer has a node and the filters
        // have something to tell apart (ADR 0014, group 17).
        ws.new_audit_entry("login", &me, "2026-11-02T09:00:00.000Z", json!({}));
        ws.new_audit_entry(
            "projectDeleted",
            &other,
            "2026-11-02T09:30:00.000Z",
            json!({ "entityType": "project", "entityIdentifier": "old-plan" }),
        );
        // The newest entry carries Linear's fullest `metadata` seen (a deletion), so the
        // fixture test can hold the fake to that shape.
        let app = ws
            .teams
            .iter()
            .find(|t| t["key"] == "APP")
            .map(|t| t["id"].clone())
            .unwrap_or(Value::Null);
        ws.new_audit_entry(
            "issueDeleted",
            &me,
            "2026-11-02T10:00:00.000Z",
            json!({ "id": "00000000-0000-4000-8000-00000000a0d1", "name": "An old thing", "slugId": "an-old-thing",
                    "teamIds": [app], "entityId": "00000000-0000-4000-8000-00000000a0d1", "entityType": "issue", "entityIdentifier": "APP-0" }),
        );
        // One milestone, so that `project get` prints a node and the recorder finds
        // one (ADR 0014, group 8).
        ws.new_milestone(&project, "First cut", None, Some("2026-12-01"));
        // One project update, so that the project carries `lastUpdate` and `health` as
        // the fixture records them, `project-update list` prints a node and the
        // recorder finds one (ADR 0014, group 11).
        let muster = ws.projects.len() - 1;
        ws.new_project_update(muster, Some("Kicked off."), Some("onTrack"));
        // One initiative with the project in it, so that `InitiativeByName` records a
        // node and the fixture test holds the fake to its shape (ADR 0014, group 12).
        let launch = ws.new_initiative(
            "Launch",
            &json!({ "description": "the fake initiative", "status": "Active", "ownerId": me["id"], "leadTeamId": team["id"] }),
        );
        let (link_id, now) = (ws.id(), ws.now());
        ws.initiative_links.push(json!({ "id": link_id, "createdAt": now, "initiativeId": launch["id"], "projectId": project["id"] }));
        let a = ws.new_issue(
            "APP",
            "Get familiar with Linear",
            Some("Linear's first onboarding issue.\n\nWith two paragraphs."),
            "Todo",
            &[],
            0,
            None,
            None,
            None,
        );
        let a_id = a["id"].clone();
        ws.issues.push(a.clone());
        ws.new_comment(&a_id, "The first comment.", &me);
        ws.new_comment(&a_id, "The second comment, with `code`.", &other);
        ws.new_attachment(
            &a_id,
            "The first link",
            Some("seeded"),
            "https://example.com/first",
        );
        let label_id = ws.labels[2]["id"].clone();
        let mut b = ws.new_issue(
            "APP",
            "Connect your tools",
            Some("The second onboarding issue."),
            "Backlog",
            &[label_id],
            2,
            None,
            Some(&other),
            None,
        );
        // In the active cycle, so that `cycle get` and the recorder find an issue node.
        b["cycle"] =
            json!({ "id": active["id"], "number": active["number"], "name": active["name"] });
        ws.issues.push(b);
        let c = ws.new_issue(
            "APP",
            "An old thing",
            None,
            "Done",
            &[],
            4,
            None,
            None,
            None,
        );
        ws.issues.push(c);
        // A sub-issue of the first, so that `children` has a node shape to hold the
        // fixture to (ADR 0010; ADR 0013).
        let d = ws.new_issue(
            "APP",
            "Read the first comment",
            Some("A sub-issue of the first onboarding issue."),
            "Todo",
            &[],
            0,
            None,
            None,
            Some(&a),
        );
        ws.issues.push(d);
        ws
    }

    fn id(&mut self) -> String {
        self.counter += 1;
        format!("00000000-0000-4000-8000-{:012}", self.counter)
    }

    fn now(&mut self) -> String {
        self.counter += 1;
        let secs = self.counter % 60;
        let mins = (self.counter / 60) % 60;
        format!("2026-11-02T10:{mins:02}:{secs:02}.000Z")
    }

    fn new_team(&mut self, key: &str, name: &str) -> Value {
        let id = self.id();
        for (pos, (state, kind)) in [
            ("Backlog", "backlog"),
            ("Todo", "unstarted"),
            ("In Progress", "started"),
            ("In Review", "started"),
            ("Done", "completed"),
            ("Canceled", "canceled"),
            ("Duplicate", "duplicate"),
        ]
        .iter()
        .enumerate()
        {
            let sid = self.id();
            self.states.push(json!({ "id": sid, "name": state, "type": kind, "position": pos as f64, "color": "#bec2c8", "description": null, "team": { "id": id, "key": key }, "archivedAt": null }));
        }
        let backlog = self
            .states
            .iter()
            .find(|s| s["team"]["id"] == id && s["name"] == "Backlog")
            .map(|s| json!({ "id": s["id"], "name": s["name"], "type": s["type"] }))
            .unwrap_or(Value::Null);
        let now = self.now();
        // Every field `TeamFields` asks for, with Linear's defaults for a new team
        // (ADR 0014, group 2), in three parts since one `json!` of them all passes the
        // macro recursion limit; `issueCount` is computed when the team is answered.
        let mut team = json!({
            "id": id, "key": key, "name": name, "displayName": name, "description": null, "icon": null, "color": null,
            "createdAt": now, "updatedAt": now, "archivedAt": null, "retiredAt": null,
            "visibility": "public", "private": false, "timezone": "Etc/UTC",
            "defaultIssueState": backlog, "triageIssueState": null, "parent": null, "activeCycle": null,
        });
        merge(
            &mut team,
            json!({
                "cyclesEnabled": false, "cycleStartDay": 1, "cycleDuration": 2, "cycleCooldownTime": 0,
                "cycleIssueAutoAssignStarted": false, "cycleIssueAutoAssignCompleted": false, "cycleLockToActive": false, "upcomingCycleCount": 2,
                "triageEnabled": false, "requirePriorityToLeaveTriage": false,
                "inheritIssueEstimation": false, "issueEstimationType": "notUsed", "issueEstimationAllowZero": true, "issueEstimationExtended": false, "defaultIssueEstimate": 0,
            }),
        );
        merge(
            &mut team,
            json!({
                "autoClosePeriod": null, "autoCloseStateId": null, "autoCloseParentIssues": null, "autoCloseChildIssues": null, "autoArchivePeriod": 6,
                "allMembersCanJoin": true, "initiativesEnabled": true, "groupIssueHistory": true,
                "securitySettings": {}, "scimManaged": false, "scimGroupName": null,
            }),
        );
        team
    }

    /// A project with every field `ProjectFields` and `ProjectDetailFields` ask for
    /// (ADR 0013; ADR 0014, group 8), not yet pushed; `memberIds` and `archivedAt`
    /// are the fake's own and the views leave them out.
    fn new_project(
        &mut self,
        name: &str,
        description: &str,
        teams: &[Value],
        status: &Value,
    ) -> Value {
        let slug: String = name
            .to_ascii_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let (id, now) = (self.id(), self.now());
        let me = &self.users[0];
        json!({
            "id": id, "name": name, "url": format!("https://linear.app/fake/project/{slug}"), "slugId": slug,
            "description": description, "content": null, "priority": 0, "startDate": null, "targetDate": null,
            "status": { "id": status["id"], "name": status["name"], "type": status["type"] },
            "lead": null, "teams": { "nodes": teams.iter().map(|t| json!({ "id": t["id"], "key": t["key"] })).collect::<Vec<_>>() },
            "color": "#bec2c8", "icon": null, "health": null, "progress": 0.0, "scope": 0.0,
            "startedAt": null, "completedAt": null, "canceledAt": null, "trashed": null,
            "createdAt": now, "updatedAt": now, "archivedAt": null,
            "creator": { "id": me["id"], "name": me["name"], "displayName": me["displayName"] },
            "lastUpdate": null, "memberIds": [],
        })
    }

    /// A project as `ProjectByName` answers it: the detail fields with `members` and
    /// `projectMilestones` paged at their bounds, the fake's own fields left out.
    fn project_detail(
        &self,
        project: &Value,
        teams: usize,
        members: usize,
        milestones: usize,
    ) -> Value {
        let mut view = Workspace::project_view(project, teams);
        for field in [
            "content",
            "slugId",
            "color",
            "icon",
            "health",
            "progress",
            "scope",
            "startedAt",
            "completedAt",
            "canceledAt",
            "trashed",
            "createdAt",
            "updatedAt",
            "archivedAt",
            "creator",
            "lastUpdate",
        ] {
            view[field] = project[field].clone();
        }
        let people: Vec<Value> = project["memberIds"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|id| self.users.iter().find(|u| &u["id"] == id))
            .map(|u| json!({ "id": u["id"], "name": u["name"], "displayName": u["displayName"], "email": u["email"] }))
            .collect();
        view["members"] = Workspace::page(people, members);
        let stones: Vec<Value> = self
            .milestones
            .iter()
            .filter(|m| m["projectId"] == project["id"])
            .map(|m| json!({ "id": m["id"], "name": m["name"], "targetDate": m["targetDate"], "status": m["status"], "progress": m["progress"] }))
            .collect();
        view["projectMilestones"] = Workspace::page(stones, milestones);
        view
    }

    /// A state as `StateFields` answers it: without the archive flag the fake keeps.
    fn state_view(state: &Value) -> Value {
        let mut view = state.clone();
        if let Some(map) = view.as_object_mut() {
            map.remove("archivedAt");
        }
        view
    }

    /// A label as `LabelFields` answers it (ADR 0014, group 6), not yet pushed.
    fn new_label(
        &mut self,
        name: &str,
        color: &str,
        description: Option<&str>,
        team: Option<&Value>,
        group: bool,
    ) -> Value {
        let (id, now) = (self.id(), self.now());
        json!({
            "id": id, "name": name, "color": color, "description": description, "isGroup": group, "retiredAt": null, "createdAt": now,
            "team": team.map(|t| json!({ "id": t["id"], "key": t["key"] })).unwrap_or(Value::Null),
            "parent": null,
        })
    }

    fn new_user(&mut self, name: &str, display: &str, email: &str, admin: bool, me: bool) -> Value {
        let (id, now) = (self.id(), self.now());
        let user = json!({
            "id": id, "name": name, "displayName": display, "email": email, "avatarUrl": null,
            "url": format!("https://linear.app/fake/profiles/{display}"),
            "active": true, "admin": admin, "owner": false, "guest": false, "app": false, "isMe": me, "disableReason": null,
            "title": null, "description": null, "timezone": "Etc/UTC", "statusEmoji": null, "statusLabel": null, "statusUntilAt": null,
            "lastSeen": now, "createdAt": now, "updatedAt": now,
        });
        self.users.push(user.clone());
        user
    }

    fn new_membership(&mut self, team_id: &Value, user_id: &Value, owner: bool) -> Value {
        let (id, now) = (self.id(), self.now());
        let membership = json!({ "id": id, "teamId": team_id, "userId": user_id, "owner": owner, "sortOrder": self.memberships.len() as f64, "createdAt": now });
        self.memberships.push(membership.clone());
        membership
    }

    /// A membership as `MembershipFields` answers it: with its user and its team.
    fn membership_view(&self, membership: &Value) -> Value {
        let user = self
            .users
            .iter()
            .find(|u| u["id"] == membership["userId"])
            .map(|u| json!({ "id": u["id"], "name": u["name"], "displayName": u["displayName"], "email": u["email"], "active": u["active"], "admin": u["admin"] }))
            .unwrap_or(Value::Null);
        let team = self
            .teams
            .iter()
            .find(|t| t["id"] == membership["teamId"])
            .map(|t| json!({ "id": t["id"], "key": t["key"] }))
            .unwrap_or(Value::Null);
        json!({ "id": membership["id"], "owner": membership["owner"], "sortOrder": membership["sortOrder"], "createdAt": membership["createdAt"], "user": user, "team": team })
    }

    /// A team as `TeamFields` answers it: with its `issueCount`, and without the
    /// deprecated `private`, which only the `Teams` projection asks for.
    fn team_view(&self, team: &Value) -> Value {
        let mut view = team.clone();
        if let Some(map) = view.as_object_mut() {
            map.remove("private");
        }
        view["issueCount"] = json!(
            self.issues
                .iter()
                .filter(|i| i["team"]["id"] == team["id"] && i["archivedAt"].is_null())
                .count()
        );
        view
    }

    /// The teams `Teams` lists: the ones not archived, unless asked.
    fn live_teams(&self, include_archived: bool) -> Vec<&Value> {
        self.teams
            .iter()
            .filter(|t| include_archived || t["archivedAt"].is_null())
            .collect()
    }

    fn team_by_key(&self, key: &str) -> Option<&Value> {
        self.teams.iter().find(|t| {
            t["key"]
                .as_str()
                .is_some_and(|k| k.eq_ignore_ascii_case(key))
        })
    }

    fn state_of(&self, team_key: &str, name: &str) -> Option<Value> {
        self.states
            .iter()
            .find(|s| s["team"]["key"] == team_key && s["name"] == name)
            .cloned()
    }

    #[allow(clippy::too_many_arguments)]
    fn new_issue(
        &mut self,
        team_key: &str,
        title: &str,
        description: Option<&str>,
        state: &str,
        label_ids: &[Value],
        priority: u64,
        project: Option<Value>,
        assignee: Option<&Value>,
        parent: Option<&Value>,
    ) -> Value {
        let team = self.team_by_key(team_key).cloned().expect("team");
        let number = self
            .issues
            .iter()
            .filter(|i| i["team"]["key"] == team_key)
            .count() as u64
            + 1;
        let identifier = format!("{team_key}-{number}");
        let slug: String = title
            .to_ascii_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let state = self.state_of(team_key, state).expect("state");
        let labels: Vec<Value> = self
            .labels
            .iter()
            .filter(|l| label_ids.contains(&l["id"]))
            .map(|l| json!({ "id": l["id"], "name": l["name"] }))
            .collect();
        let me = self.users[0].clone();
        let (id, now) = (self.id(), self.now());
        let done = state["type"] == "completed";
        let canceled = state["type"] == "canceled";
        json!({
            "id": id, "identifier": identifier, "number": number, "title": title, "description": description,
            "url": format!("https://linear.app/fake/issue/{identifier}/{slug}"),
            "priority": priority, "priorityLabel": priority_label(priority), "estimate": null, "dueDate": null,
            "branchName": format!("fake/{}-{slug}", identifier.to_ascii_lowercase()),
            "createdAt": now, "updatedAt": now, "startedAt": null,
            "completedAt": if done { json!(now) } else { Value::Null }, "canceledAt": if canceled { json!(now) } else { Value::Null }, "archivedAt": null,
            "state": { "id": state["id"], "name": state["name"], "type": state["type"] },
            "team": { "id": team["id"], "key": team["key"], "name": team["name"] },
            "project": project.map(|p| json!({ "id": p["id"], "name": p["name"] })).unwrap_or(Value::Null),
            "cycle": null,
            "assignee": assignee.map(|u| json!({ "id": u["id"], "name": u["name"], "displayName": u["displayName"], "email": u["email"] })).unwrap_or(Value::Null),
            "creator": { "id": me["id"], "name": me["name"], "displayName": me["displayName"] },
            "parent": parent.map(|p| json!({ "id": p["id"], "identifier": p["identifier"] })).unwrap_or(Value::Null),
            "projectMilestone": null,
            "labels": { "nodes": labels },
        })
    }

    /// A milestone as `MilestoneFields` answers it; the fake keeps `projectId` beside.
    fn new_milestone(
        &mut self,
        project: &Value,
        name: &str,
        description: Option<&str>,
        target: Option<&str>,
    ) -> Value {
        let (id, now) = (self.id(), self.now());
        let order = self
            .milestones
            .iter()
            .filter(|m| m["projectId"] == project["id"])
            .count() as f64;
        let stone = json!({
            "id": id, "name": name, "description": description, "targetDate": target,
            "status": "unstarted", "progress": 0.0, "sortOrder": order, "createdAt": now,
            "project": { "id": project["id"], "name": project["name"] }, "projectId": project["id"],
        });
        self.milestones.push(stone.clone());
        stone
    }

    /// A project update as `ProjectUpdateFields` answers it; the fake keeps
    /// `projectId` beside and sets the project's `lastUpdate` and `health` from it,
    /// as Linear derives them.
    fn new_project_update(
        &mut self,
        project_index: usize,
        body: Option<&str>,
        health: Option<&str>,
    ) -> Value {
        let (id, now) = (self.id(), self.now());
        let me = self.users[0].clone();
        let project = &self.projects[project_index];
        let post = json!({
            "id": id, "body": body.unwrap_or(""), "health": health.unwrap_or("onTrack"),
            "url": format!("https://linear.app/fake/project/{}/updates/{id}", project["slugId"].as_str().unwrap_or("p")),
            "createdAt": now, "editedAt": null,
            "user": { "id": me["id"], "name": me["name"], "displayName": me["displayName"] },
            "project": { "id": project["id"], "name": project["name"] }, "projectId": project["id"],
        });
        self.project_updates.push(post.clone());
        let project = &mut self.projects[project_index];
        project["lastUpdate"] =
            json!({ "id": post["id"], "health": post["health"], "createdAt": post["createdAt"] });
        project["health"] = post["health"].clone();
        post
    }

    /// An initiative with every `InitiativeFields` field (ADR 0014, group 12), plus
    /// `content`, which `InitiativeByName` asks for.
    fn new_initiative(&mut self, name: &str, input: &Value) -> Value {
        let slug: String = name
            .to_ascii_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let (id, now) = (self.id(), self.now());
        let owner = input["ownerId"]
            .as_str()
            .and_then(|uid| self.users.iter().find(|u| u["id"] == uid))
            .map(|u| json!({ "id": u["id"], "name": u["name"], "displayName": u["displayName"] }))
            .unwrap_or(Value::Null);
        let lead = input["leadTeamId"]
            .as_str()
            .and_then(|tid| self.teams.iter().find(|t| t["id"] == tid))
            .map(|t| json!({ "id": t["id"], "key": t["key"] }))
            .unwrap_or(Value::Null);
        let initiative = json!({
            "id": id, "name": name, "description": input["description"], "status": input["status"].as_str().unwrap_or("Planned"),
            "health": null, "targetDate": input["targetDate"], "priority": input["priority"].as_u64().unwrap_or(0),
            "url": format!("https://linear.app/fake/initiative/{slug}"), "slugId": slug,
            "createdAt": now, "updatedAt": now, "archivedAt": null, "trashed": null,
            "owner": owner, "leadTeam": lead, "content": input["content"],
        });
        self.initiatives.push(initiative.clone());
        initiative
    }

    /// An initiative as `InitiativeFields` answers it: without `content`.
    fn initiative_view(initiative: &Value) -> Value {
        let mut view = initiative.clone();
        if let Some(map) = view.as_object_mut() {
            map.remove("content");
        }
        view
    }

    /// An initiative as `InitiativeByName` answers it: with `content` and its
    /// projects paged at the bound.
    fn initiative_detail(&self, initiative: &Value, projects: usize) -> Value {
        let mut view = initiative.clone();
        let linked: Vec<Value> = self
            .initiative_links
            .iter()
            .filter(|l| l["initiativeId"] == initiative["id"])
            .filter_map(|l| self.projects.iter().find(|p| p["id"] == l["projectId"]))
            .map(|p| json!({ "id": p["id"], "name": p["name"] }))
            .collect();
        view["projects"] = Workspace::page(linked, projects);
        view
    }

    fn link_view(&self, link: &Value) -> Value {
        let project = self
            .projects
            .iter()
            .find(|p| p["id"] == link["projectId"])
            .map(|p| json!({ "id": p["id"], "name": p["name"] }))
            .unwrap_or(Value::Null);
        let initiative = self
            .initiatives
            .iter()
            .find(|i| i["id"] == link["initiativeId"])
            .map(|i| json!({ "id": i["id"], "name": i["name"] }))
            .unwrap_or(Value::Null);
        json!({ "id": link["id"], "createdAt": link["createdAt"], "project": project, "initiative": initiative })
    }

    /// A cycle of a team with every `CycleFields` field (ADR 0014, group 13), newest
    /// first in the store as Linear orders a connection; the flags are
    /// `(isActive, isNext, isPrevious, isFuture, isPast)`.
    fn new_cycle(
        &mut self,
        team: &Value,
        number: f64,
        starts_at: &str,
        ends_at: &str,
        completed_at: Option<&str>,
        flags: (bool, bool, bool, bool, bool),
    ) -> Value {
        let (id, now) = (self.id(), self.now());
        let cycle = json!({
            "id": id, "number": number, "name": null, "description": null,
            "startsAt": starts_at, "endsAt": ends_at, "completedAt": completed_at,
            "isActive": flags.0, "isNext": flags.1, "isPrevious": flags.2, "isFuture": flags.3, "isPast": flags.4,
            "progress": 0.0, "createdAt": now, "updatedAt": now, "archivedAt": null,
            "team": { "id": team["id"], "key": team["key"] },
        });
        self.cycles.insert(0, cycle.clone());
        cycle
    }

    /// A cycle as `CycleByNumber` answers it: with its issues paged at the bound.
    fn cycle_detail(&self, cycle: &Value, issues: usize) -> Value {
        let mut view = cycle.clone();
        let in_cycle: Vec<Value> = self
            .issues
            .iter()
            .filter(|i| i["cycle"]["id"] == cycle["id"] && i["archivedAt"].is_null())
            .map(|i| json!({ "id": i["id"], "identifier": i["identifier"], "title": i["title"], "state": i["state"] }))
            .collect();
        view["issues"] = Workspace::page(in_cycle, issues);
        view
    }

    /// The cycle `cycleId` names, as an issue carries it, for the issue's team; `None`
    /// to clear; a cycle of another team is Linear's refusal.
    fn cycle_for(&self, input: &Value, team_id: &Value) -> Result<Value, (u16, Value)> {
        match input["cycleId"].as_str() {
            Some(cid) => match self.cycles.iter().find(|c| c["id"] == cid) {
                Some(c) if c["team"]["id"] == *team_id => {
                    Ok(json!({ "id": c["id"], "number": c["number"], "name": c["name"] }))
                }
                Some(_) => Err((
                    200,
                    json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "The cycle belongs to another team." } } ], "data": null }),
                )),
                None => Err(not_found("Cycle")),
            },
            None => Ok(Value::Null),
        }
    }

    /// A document with every `DocumentFields` field (ADR 0014, group 14) and its
    /// `content`, newest first in the store as Linear orders a connection.
    fn new_document(&mut self, title: &str, input: &Value) -> Result<Value, (u16, Value)> {
        let slug: String = title
            .to_ascii_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let (id, now) = (self.id(), self.now());
        let me = self.users[0].clone();
        let person =
            json!({ "id": me["id"], "name": me["name"], "displayName": me["displayName"] });
        let project = match input["projectId"].as_str() {
            Some(pid) => match self.projects.iter().find(|p| p["id"] == pid) {
                Some(p) => json!({ "id": p["id"], "name": p["name"] }),
                None => return Err(not_found("Project")),
            },
            None => Value::Null,
        };
        let issue = match input["issueId"].as_str() {
            Some(iid) => match self.issues.iter().find(|i| i["id"] == iid) {
                Some(i) => json!({ "id": i["id"], "identifier": i["identifier"] }),
                None => return Err(not_found("Issue")),
            },
            None => Value::Null,
        };
        // Linear, 2026-11-03: "A document needs a project, initiative, team, issue,
        // release, or cycle."; muster offers the project and the issue.
        if project.is_null() && issue.is_null() {
            return Err(Workspace::document_needs_a_home());
        }
        let document = json!({
            "id": id, "title": title, "icon": null, "color": null,
            "url": format!("https://linear.app/fake/document/{slug}"), "slugId": slug,
            "createdAt": now, "updatedAt": now, "archivedAt": null, "trashed": null,
            "creator": person, "owner": person, "updatedBy": person,
            "project": project, "issue": issue, "content": input["content"],
        });
        self.documents.insert(0, document.clone());
        Ok(document)
    }

    /// Linear's refusal of a document with nothing to live under, as answered on
    /// 2026-11-03.
    fn document_needs_a_home() -> (u16, Value) {
        (
            200,
            json!({ "errors": [ { "message": "invalid document input", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "A document needs a project, initiative, team, issue, release, or cycle." } } ], "data": null }),
        )
    }

    /// A document as `DocumentFields` answers it: without `content`.
    fn document_view(document: &Value) -> Value {
        let mut view = document.clone();
        if let Some(map) = view.as_object_mut() {
            map.remove("content");
        }
        view
    }

    /// A template with every `TemplateFields` field (ADR 0014, group 15), newest first
    /// in the store; `team` is `None` for one the workspace shares.
    fn new_template(&mut self, input: &Value) -> Result<Value, (u16, Value)> {
        let (id, now) = (self.id(), self.now());
        let me = self.users[0].clone();
        let team = match input["teamId"].as_str() {
            Some(tid) => match self.teams.iter().find(|t| t["id"] == tid) {
                Some(t) => json!({ "id": t["id"], "key": t["key"] }),
                None => return Err(not_found("Team")),
            },
            None => Value::Null,
        };
        // Linear takes `templateData` as JSON and returns it as a string of JSON
        // (read on 2026-11-03), so the store keeps the string.
        let data = Workspace::template_data_string(&input["templateData"]);
        let template = json!({
            "id": id, "type": input["type"], "name": input["name"], "description": input["description"],
            "icon": null, "color": null, "templateData": data, "sortOrder": 0.0,
            "lastAppliedAt": null, "createdAt": now, "updatedAt": now, "archivedAt": null,
            "team": team, "creator": { "id": me["id"], "name": me["name"], "displayName": me["displayName"] },
        });
        self.templates.insert(0, template.clone());
        Ok(template)
    }

    /// `templateData` as Linear returns it: a string of JSON, whatever JSON came in.
    fn template_data_string(data: &Value) -> Value {
        match data {
            Value::String(s) => Value::String(s.clone()),
            other => Value::String(other.to_string()),
        }
    }

    /// A webhook with every `WebhookFields` field (ADR 0014, group 16) and its
    /// `secret`, which the view leaves out; `team` is `None` for every public team.
    fn new_webhook(&mut self, input: &Value) -> Result<Value, (u16, Value)> {
        let (id, now) = (self.id(), self.now());
        let me = self.users[0].clone();
        let team = if input["allPublicTeams"].as_bool() == Some(true) {
            Value::Null
        } else {
            match input["teamId"].as_str() {
                Some(tid) => match self.teams.iter().find(|t| t["id"] == tid) {
                    Some(t) => json!({ "id": t["id"], "key": t["key"] }),
                    None => return Err(not_found("Team")),
                },
                None => {
                    return Err((
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "A webhook needs a team or allPublicTeams." } } ], "data": null }),
                    ));
                }
            }
        };
        let hook = json!({
            "id": id, "label": input["label"], "url": input["url"], "enabled": input["enabled"].as_bool().unwrap_or(true),
            "resourceTypes": input["resourceTypes"], "allPublicTeams": team.is_null(),
            "createdAt": now, "updatedAt": now, "archivedAt": null,
            "team": team, "creator": { "id": me["id"], "name": me["name"], "displayName": me["displayName"] },
            "secret": input["secret"],
        });
        self.webhooks.insert(0, hook.clone());
        Ok(hook)
    }

    /// A webhook as `WebhookFields` answers it: without its `secret`.
    fn webhook_view(hook: &Value) -> Value {
        let mut view = hook.clone();
        if let Some(map) = view.as_object_mut() {
            map.remove("secret");
        }
        view
    }

    /// An audit entry with every `AuditEntryFields` field (ADR 0014, group 17), newest
    /// first in the store.
    fn new_audit_entry(&mut self, kind: &str, actor: &Value, at: &str, metadata: Value) -> Value {
        let id = self.id();
        let entry = json!({
            "id": id, "type": kind, "createdAt": at, "ip": "203.0.113.7", "countryCode": "GB",
            "metadata": metadata,
            "actor": { "id": actor["id"], "name": actor["name"], "displayName": actor["displayName"] },
        });
        self.audit_entries.insert(0, entry.clone());
        entry
    }

    fn milestone_view(stone: &Value) -> Value {
        let mut view = stone.clone();
        if let Some(map) = view.as_object_mut() {
            map.remove("projectId");
        }
        view
    }

    /// The milestone an issue input names, checked to be in the issue's project, as
    /// Linear requires; `None` to clear.
    fn milestone_for(&self, input: &Value, project: &Value) -> Result<Value, (u16, Value)> {
        match input["projectMilestoneId"].as_str() {
            Some(mid) => match self.milestones.iter().find(|m| m["id"] == mid) {
                Some(m) if m["projectId"] == project["id"] => {
                    Ok(json!({ "id": m["id"], "name": m["name"] }))
                }
                Some(_) => Err((
                    200,
                    json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "The milestone must belong to the issue's project." } } ], "data": null }),
                )),
                None => Err(not_found("ProjectMilestone")),
            },
            None => Ok(Value::Null),
        }
    }

    fn new_comment(&mut self, issue_id: &Value, body: &str, user: &Value) -> Value {
        let (id, now) = (self.id(), self.now());
        let identifier = self
            .issues
            .iter()
            .find(|i| &i["id"] == issue_id)
            .map(|i| i["identifier"].clone())
            .unwrap_or(Value::Null);
        let comment = json!({
            "id": id, "body": body, "url": format!("https://linear.app/fake/issue/{}#comment-{id}", identifier.as_str().unwrap_or("?")),
            "createdAt": now, "updatedAt": now, "editedAt": null,
            "user": { "id": user["id"], "name": user["name"], "displayName": user["displayName"] },
            "parent": null, "issueId": issue_id, "issue": { "id": issue_id, "identifier": identifier },
        });
        self.comments.push(comment.clone());
        comment
    }

    fn new_attachment(
        &mut self,
        issue_id: &Value,
        title: &str,
        subtitle: Option<&str>,
        url: &str,
    ) -> Value {
        let (id, now) = (self.id(), self.now());
        let identifier = self
            .issues
            .iter()
            .find(|i| &i["id"] == issue_id)
            .map(|i| i["identifier"].clone())
            .unwrap_or(Value::Null);
        let attachment = json!({
            // `sourceType` is `"unknown"` for a link the API made (appshapes, 2026-11-03).
            "id": id, "title": title, "subtitle": subtitle, "url": url, "sourceType": "unknown", "createdAt": now,
            "issueId": issue_id, "issue": { "id": issue_id, "identifier": identifier },
        });
        self.attachments.push(attachment.clone());
        attachment
    }

    fn full_issue(
        &self,
        issue: &Value,
        comments: usize,
        attachments: usize,
        children: usize,
    ) -> Value {
        let mut full = issue.clone();
        // Linear orders a connection newest first, `orderBy: createdAt` included
        // (read on the appshapes workspace, 2026-11-03); so does the fake.
        let mut all: Vec<Value> = self
            .comments
            .iter()
            .filter(|c| c["issueId"] == issue["id"])
            .map(|c| {
                let mut c = c.clone();
                c.as_object_mut().unwrap().remove("issueId");
                c.as_object_mut().unwrap().remove("issue");
                c
            })
            .collect();
        all.reverse();
        let cut = all.len() > comments;
        full["comments"] = json!({ "nodes": all.into_iter().take(comments).collect::<Vec<_>>(), "pageInfo": { "hasNextPage": cut } });
        let mut links: Vec<Value> = self
            .attachments
            .iter()
            .filter(|a| a["issueId"] == issue["id"])
            .map(|a| json!({ "id": a["id"], "title": a["title"], "subtitle": a["subtitle"], "url": a["url"], "sourceType": a["sourceType"], "createdAt": a["createdAt"] }))
            .collect();
        links.reverse();
        full["attachments"] = Workspace::page(links, attachments);
        let mut kids: Vec<Value> = self
            .issues
            .iter()
            .filter(|i| i["parent"]["id"] == issue["id"])
            .map(|i| json!({ "id": i["id"], "identifier": i["identifier"], "title": i["title"], "state": i["state"] }))
            .collect();
        kids.reverse();
        full["children"] = Workspace::page(kids, children);
        full
    }

    /// An issue as a filter sees it: with every comment, attachment and sub-issue
    /// attached, and its `parent` as the whole parent issue, so that a clause such as
    /// `parent: { number: { eq: 12 }, team: { key: { eq: "APP" } } }` resolves as it
    /// does in Linear, where `parent` is an `Issue`.
    fn filter_view(&self, issue: &Value) -> Value {
        let mut view = self.full_issue(issue, usize::MAX, usize::MAX, usize::MAX);
        if let Some(pid) = view["parent"]["id"].as_str()
            && let Some(parent) = self.issues.iter().find(|i| i["id"] == pid)
        {
            view["parent"] = parent.clone();
        }
        view
    }

    /// A project as `Projects` and `ProjectCreate` answer it: `ProjectFields` with
    /// its `teams` paged at `first: $teams`, as Linear pages a nested connection.
    fn project_view(project: &Value, teams: usize) -> Value {
        let all: Vec<Value> = project["teams"]["nodes"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let mut view = json!({});
        for field in [
            "id",
            "name",
            "url",
            "description",
            "priority",
            "startDate",
            "targetDate",
            "status",
            "lead",
        ] {
            view[field] = project[field].clone();
        }
        view["teams"] = Workspace::page(all, teams);
        view
    }

    fn page(items: Vec<Value>, first: usize) -> Value {
        let cut = items.len() > first;
        json!({ "nodes": items.into_iter().take(first).collect::<Vec<_>>(), "pageInfo": { "hasNextPage": cut } })
    }

    fn dispatch(&mut self, op: &str, vars: &Value) -> (u16, Value) {
        let first = vars["first"].as_u64().unwrap_or(50) as usize;
        let filter = &vars["filter"];
        match op {
            "Viewer" => {
                let me = &self.users[0];
                let org = &self.organization;
                (
                    200,
                    json!({ "data": { "viewer": { "id": me["id"], "name": me["name"], "displayName": me["displayName"], "email": me["email"], "admin": me["admin"], "active": me["active"] }, "organization": { "id": org["id"], "name": org["name"], "urlKey": org["urlKey"] } } }),
                )
            }
            "Workspace" => (
                200,
                json!({ "data": { "organization": self.organization } }),
            ),
            "WorkspaceUpdate" => {
                // Only what is sent changes; the security and auth settings merge key by
                // key, which is the fake's model of Linear's nested inputs, not a
                // recorded fact (the smoke run proved the hour path only); a new URL key
                // keeps the old one among the last three previous keys, as Linear does.
                let Some(input) = vars["input"].as_object().cloned() else {
                    return missing_variable("input", "OrganizationUpdateInput!");
                };
                if input.get("urlKey").and_then(Value::as_str) == Some("") {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "urlKey should not be empty" } } ], "data": null }),
                    );
                }
                let now = self.now();
                for (field, value) in input {
                    match field.as_str() {
                        "securitySettings" | "authSettings" => {
                            if let (Some(have), Some(given)) =
                                (self.organization[&field].as_object_mut(), value.as_object())
                            {
                                for (k, v) in given {
                                    have.insert(k.clone(), v.clone());
                                }
                            }
                        }
                        "urlKey" => {
                            let old = self.organization["urlKey"].clone();
                            if old != value {
                                let mut previous: Vec<Value> = self.organization["previousUrlKeys"]
                                    .as_array()
                                    .cloned()
                                    .unwrap_or_default();
                                previous.insert(0, old);
                                previous.truncate(3);
                                self.organization["previousUrlKeys"] = json!(previous);
                                self.organization["urlKey"] = value;
                            }
                        }
                        other => {
                            self.organization[other] = value;
                        }
                    }
                }
                self.organization["updatedAt"] = json!(now);
                (
                    200,
                    json!({ "data": { "organizationUpdate": { "success": true, "organization": self.organization } } }),
                )
            }
            "Teams" => {
                let teams: Vec<Value> = self
                    .live_teams(false)
                    .into_iter()
                    .map(|t| json!({ "id": t["id"], "key": t["key"], "name": t["name"], "description": t["description"], "private": t["private"], "visibility": t["visibility"], "cyclesEnabled": t["cyclesEnabled"], "retiredAt": t["retiredAt"], "timezone": t["timezone"] }))
                    .collect();
                (
                    200,
                    json!({ "data": { "teams": Workspace::page(teams, first) } }),
                )
            }
            "TeamByKey" => {
                let key = vars["key"].as_str().unwrap_or("");
                let include_archived = vars["includeArchived"].as_bool().unwrap_or(false);
                let found: Vec<Value> = self
                    .live_teams(include_archived)
                    .into_iter()
                    .filter(|t| {
                        t["key"]
                            .as_str()
                            .is_some_and(|k| k.eq_ignore_ascii_case(key))
                    })
                    .map(|t| self.team_view(t))
                    .collect();
                (
                    200,
                    json!({ "data": { "teams": Workspace::page(found, 1) } }),
                )
            }
            "TeamMembers" => {
                let key = vars["key"].as_str().unwrap_or("");
                let found: Vec<Value> = self
                    .live_teams(false)
                    .into_iter()
                    .filter(|t| t["key"].as_str().is_some_and(|k| k.eq_ignore_ascii_case(key)))
                    .map(|t| {
                        let members: Vec<Value> = self
                            .memberships
                            .iter()
                            .filter(|m| m["teamId"] == t["id"])
                            .map(|m| self.membership_view(m))
                            .collect();
                        json!({ "id": t["id"], "key": t["key"], "memberships": Workspace::page(members, first) })
                    })
                    .collect();
                (
                    200,
                    json!({ "data": { "teams": Workspace::page(found, 1) } }),
                )
            }
            "TeamMembershipCreate" => {
                let input = &vars["input"];
                let (team_id, user_id) = (input["teamId"].clone(), input["userId"].clone());
                let Some(team) = self.teams.iter().find(|t| t["id"] == team_id) else {
                    return not_found("Team");
                };
                if !self.users.iter().any(|u| u["id"] == user_id) {
                    return not_found("User");
                }
                // Linear refuses a second membership, and a team that is archived or
                // retired (the mutation's own description).
                let refusal = if self
                    .memberships
                    .iter()
                    .any(|m| m["teamId"] == team_id && m["userId"] == user_id)
                {
                    Some("User is already a member of the team.")
                } else if !team["archivedAt"].is_null() || !team["retiredAt"].is_null() {
                    Some("Cannot add a member to an archived or retired team.")
                } else {
                    None
                };
                if let Some(message) = refusal {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": message } } ], "data": null }),
                    );
                }
                let membership = self.new_membership(&team_id, &user_id, false);
                (
                    200,
                    json!({ "data": { "teamMembershipCreate": { "success": true, "teamMembership": self.membership_view(&membership) } } }),
                )
            }
            "TeamMembershipDelete" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(index) = self.memberships.iter().position(|m| m["id"] == id) else {
                    return not_found("TeamMembership");
                };
                self.memberships.remove(index);
                (
                    200,
                    json!({ "data": { "teamMembershipDelete": { "success": true, "entityId": id } } }),
                )
            }
            "TeamCreate" => {
                let input = vars["input"].clone();
                let Some(name) = input["name"].as_str().filter(|n| !n.trim().is_empty()) else {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "name should not be empty" } } ], "data": null }),
                    );
                };
                let key = match input["key"].as_str() {
                    Some(key) => key.to_string(),
                    None => name
                        .chars()
                        .filter(|c| c.is_ascii_alphanumeric())
                        .take(3)
                        .collect::<String>()
                        .to_ascii_uppercase(),
                };
                if self.teams.iter().any(|t| {
                    t["key"]
                        .as_str()
                        .is_some_and(|k| k.eq_ignore_ascii_case(&key))
                }) {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "The team key is already in use." } } ], "data": null }),
                    );
                }
                let parent = match input["parentId"].as_str() {
                    Some(pid) => match self.teams.iter().find(|t| t["id"] == pid) {
                        Some(p) => json!({ "id": p["id"], "key": p["key"], "name": p["name"] }),
                        None => return not_found("Team"),
                    },
                    None => Value::Null,
                };
                if let Some(copy) = vars["copySettingsFromTeamId"].as_str()
                    && !self.teams.iter().any(|t| t["id"] == copy)
                {
                    return not_found("Team");
                }
                let mut team = self.new_team(&key, name);
                // The creator is a member of the new team; `owner` came back false on
                // the appshapes workspace (2026-11-03), whatever the mutation's
                // description says, so the fake answers what Linear answered.
                let me = self.users[0]["id"].clone();
                self.new_membership(&team["id"], &me, false);
                team["parent"] = parent;
                for field in ["description", "timezone", "cyclesEnabled"] {
                    if let Some(value) = input.get(field) {
                        team[field] = value.clone();
                    }
                }
                self.teams.push(team.clone());
                (
                    200,
                    json!({ "data": { "teamCreate": { "success": true, "team": self.team_view(&team) } } }),
                )
            }
            "TeamUpdate" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(input) = vars["input"].as_object().cloned() else {
                    return missing_variable("input", "TeamUpdateInput!");
                };
                let Some(index) = self.teams.iter().position(|t| t["id"] == id) else {
                    return not_found("Team");
                };
                let now = self.now();
                let mut team = self.teams[index].clone();
                for (field, value) in input {
                    match field.as_str() {
                        // Only what is sent changes; the nested and the referenced
                        // settings are answered as `TeamFields` answers them.
                        "securitySettings" => {
                            if let (Some(have), Some(given)) =
                                (team["securitySettings"].as_object_mut(), value.as_object())
                            {
                                for (k, v) in given {
                                    have.insert(k.clone(), v.clone());
                                }
                            }
                        }
                        "parentId" => {
                            team["parent"] = match value.as_str() {
                                Some(pid) => match self.teams.iter().find(|t| t["id"] == pid) {
                                    Some(p) => {
                                        json!({ "id": p["id"], "key": p["key"], "name": p["name"] })
                                    }
                                    None => return not_found("Team"),
                                },
                                None => Value::Null,
                            };
                        }
                        "defaultIssueStateId" => {
                            team["defaultIssueState"] = match value.as_str() {
                                Some(sid) => match self.states.iter().find(|s| s["id"] == sid) {
                                    Some(s) => {
                                        json!({ "id": s["id"], "name": s["name"], "type": s["type"] })
                                    }
                                    None => return not_found("WorkflowState"),
                                },
                                None => Value::Null,
                            };
                        }
                        "private" => {
                            let private = value.as_bool().unwrap_or(false);
                            team["private"] = json!(private);
                            team["visibility"] = json!(if private { "private" } else { "public" });
                        }
                        "key" => {
                            if let Some(key) = value.as_str() {
                                team["key"] = json!(key);
                            }
                        }
                        "name" => {
                            team["name"] = value.clone();
                            team["displayName"] = value;
                        }
                        other => {
                            team[other] = value;
                        }
                    }
                }
                team["updatedAt"] = json!(now);
                self.teams[index] = team.clone();
                (
                    200,
                    json!({ "data": { "teamUpdate": { "success": true, "team": self.team_view(&team) } } }),
                )
            }
            "TeamDelete" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(index) = self
                    .teams
                    .iter()
                    .position(|t| t["id"] == id && t["archivedAt"].is_null())
                else {
                    return not_found("Team");
                };
                let now = self.now();
                self.teams[index]["archivedAt"] = json!(now);
                (
                    200,
                    json!({ "data": { "teamDelete": { "success": true, "entityId": id } } }),
                )
            }
            "TeamUnarchive" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(index) = self.teams.iter().position(|t| t["id"] == id) else {
                    return not_found("Team");
                };
                self.teams[index]["archivedAt"] = Value::Null;
                let team = self.teams[index].clone();
                (
                    200,
                    json!({ "data": { "teamUnarchive": { "success": true, "entity": self.team_view(&team) } } }),
                )
            }
            "WorkflowStates" => {
                let mut states: Vec<Value> = self
                    .states
                    .iter()
                    .filter(|s| s["archivedAt"].is_null() && passes(s, filter))
                    .map(Workspace::state_view)
                    .collect();
                states.sort_by(|a, b| {
                    a["position"]
                        .as_f64()
                        .unwrap_or(0.0)
                        .total_cmp(&b["position"].as_f64().unwrap_or(0.0))
                });
                (
                    200,
                    json!({ "data": { "workflowStates": Workspace::page(states, first) } }),
                )
            }
            "IssueLabels" => (
                200,
                json!({ "data": { "issueLabels": Workspace::page(self.labels.iter().filter(|l| passes(l, filter)).cloned().collect(), first) } }),
            ),
            "Projects" => {
                let Some(teams) = vars["teams"].as_u64().map(|n| n as usize) else {
                    return missing_variable("teams", "Int!");
                };
                // The filter is answered from the detail view, which carries the
                // `members` and `projectMilestones` connections the recorder's sampler
                // filters on, as Linear filters a project by its relations; the page
                // prints `ProjectFields`.
                let projects: Vec<Value> = self
                    .projects
                    .iter()
                    .filter(|p| {
                        p["archivedAt"].is_null()
                            && passes(
                                &self.project_detail(p, teams, usize::MAX, usize::MAX),
                                filter,
                            )
                    })
                    .map(|p| Workspace::project_view(p, teams))
                    .collect();
                (
                    200,
                    json!({ "data": { "projects": Workspace::page(projects, first) } }),
                )
            }
            "Users" => {
                // Suspended users are left out unless `includeDisabled`, as Linear
                // does (the query's own description).
                let include_disabled = vars["includeDisabled"].as_bool().unwrap_or(false);
                let users: Vec<Value> = self
                    .users
                    .iter()
                    .filter(|u| passes(u, filter) && (include_disabled || u["active"] == true))
                    .map(|u| json!({ "id": u["id"], "name": u["name"], "displayName": u["displayName"], "email": u["email"], "active": u["active"], "admin": u["admin"], "owner": u["owner"], "guest": u["guest"], "app": u["app"], "isMe": u["isMe"] }))
                    .collect();
                (
                    200,
                    json!({ "data": { "users": Workspace::page(users, first) } }),
                )
            }
            "ProjectByName" => {
                let (Some(teams), Some(members), Some(milestones)) = (
                    vars["teams"].as_u64().map(|n| n as usize),
                    vars["members"].as_u64().map(|n| n as usize),
                    vars["milestones"].as_u64().map(|n| n as usize),
                ) else {
                    return missing_variable("members", "Int!");
                };
                let name = vars["name"].as_str().unwrap_or("");
                let include_archived = vars["includeArchived"].as_bool().unwrap_or(false);
                let found: Vec<Value> = self
                    .projects
                    .iter()
                    .filter(|p| {
                        (include_archived || p["archivedAt"].is_null())
                            && p["name"]
                                .as_str()
                                .is_some_and(|n| n.eq_ignore_ascii_case(name))
                    })
                    .map(|p| self.project_detail(p, teams, members, milestones))
                    .collect();
                (
                    200,
                    json!({ "data": { "projects": Workspace::page(found, 1) } }),
                )
            }
            "ProjectStatuses" => {
                let include_archived = vars["includeArchived"].as_bool().unwrap_or(false);
                let statuses: Vec<Value> = self
                    .project_statuses
                    .iter()
                    .filter(|s| include_archived || s["archivedAt"].is_null())
                    .map(Workspace::state_view)
                    .collect();
                (
                    200,
                    json!({ "data": { "projectStatuses": Workspace::page(statuses, first) } }),
                )
            }
            "Milestones" => {
                let name = vars["project"].as_str().unwrap_or("");
                let stones: Vec<Value> = self
                    .milestones
                    .iter()
                    .filter(|m| {
                        m["project"]["name"]
                            .as_str()
                            .is_some_and(|n| n.eq_ignore_ascii_case(name))
                    })
                    .map(Workspace::milestone_view)
                    .collect();
                (
                    200,
                    json!({ "data": { "projectMilestones": Workspace::page(stones, first) } }),
                )
            }
            "MilestoneCreate" => {
                let input = &vars["input"];
                let Some(name) = input["name"].as_str().filter(|n| !n.trim().is_empty()) else {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "name should not be empty" } } ], "data": null }),
                    );
                };
                let Some(project) = self
                    .projects
                    .iter()
                    .find(|p| p["id"] == input["projectId"] && p["archivedAt"].is_null())
                    .cloned()
                else {
                    return not_found("Project");
                };
                let stone = self.new_milestone(
                    &project,
                    name,
                    input["description"].as_str(),
                    input["targetDate"].as_str(),
                );
                (
                    200,
                    json!({ "data": { "projectMilestoneCreate": { "success": true, "projectMilestone": Workspace::milestone_view(&stone) } } }),
                )
            }
            "MilestoneUpdate" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(input) = vars["input"].as_object().cloned() else {
                    return missing_variable("input", "ProjectMilestoneUpdateInput!");
                };
                let Some(index) = self.milestones.iter().position(|m| m["id"] == id) else {
                    return not_found("ProjectMilestone");
                };
                for (field, value) in input {
                    self.milestones[index][&field] = value;
                }
                let (mid, mname) = (
                    self.milestones[index]["id"].clone(),
                    self.milestones[index]["name"].clone(),
                );
                for issue in &mut self.issues {
                    if issue["projectMilestone"]["id"] == mid {
                        issue["projectMilestone"]["name"] = mname.clone();
                    }
                }
                (
                    200,
                    json!({ "data": { "projectMilestoneUpdate": { "success": true, "projectMilestone": Workspace::milestone_view(&self.milestones[index]) } } }),
                )
            }
            "MilestoneDelete" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(index) = self.milestones.iter().position(|m| m["id"] == id) else {
                    return not_found("ProjectMilestone");
                };
                self.milestones.remove(index);
                for issue in &mut self.issues {
                    if issue["projectMilestone"]["id"] == id {
                        issue["projectMilestone"] = Value::Null;
                    }
                }
                (
                    200,
                    json!({ "data": { "projectMilestoneDelete": { "success": true, "entityId": id } } }),
                )
            }
            "ProjectUpdates" => {
                let name = vars["project"].as_str().unwrap_or("");
                let mut posts: Vec<Value> = self
                    .project_updates
                    .iter()
                    .filter(|p| {
                        p["project"]["name"]
                            .as_str()
                            .is_some_and(|n| n.eq_ignore_ascii_case(name))
                    })
                    .map(Workspace::milestone_view)
                    .collect();
                posts.reverse();
                (
                    200,
                    json!({ "data": { "projectUpdates": Workspace::page(posts, first) } }),
                )
            }
            "ProjectUpdateCreate" => {
                let input = &vars["input"];
                let Some(index) = self
                    .projects
                    .iter()
                    .position(|p| p["id"] == input["projectId"] && p["archivedAt"].is_null())
                else {
                    return not_found("Project");
                };
                if let Some(h) = input["health"].as_str()
                    && !["onTrack", "atRisk", "offTrack"].contains(&h)
                {
                    return (
                        400,
                        json!({ "errors": [ { "message": format!("Variable \"$input\" got invalid value {h:?} at \"input.health\""), "extensions": { "code": "GRAPHQL_VALIDATION_FAILED", "type": "graphql error", "userError": true } } ] }),
                    );
                }
                let post = self.new_project_update(
                    index,
                    input["body"].as_str(),
                    input["health"].as_str(),
                );
                (
                    200,
                    json!({ "data": { "projectUpdateCreate": { "success": true, "projectUpdate": Workspace::milestone_view(&post) } } }),
                )
            }
            "Initiatives" => {
                let initiatives: Vec<Value> = self
                    .initiatives
                    .iter()
                    .filter(|i| i["archivedAt"].is_null() && passes(i, filter))
                    .map(Workspace::initiative_view)
                    .collect();
                (
                    200,
                    json!({ "data": { "initiatives": Workspace::page(initiatives, first) } }),
                )
            }
            "InitiativeByName" => {
                let Some(projects) = vars["projects"].as_u64().map(|n| n as usize) else {
                    return missing_variable("projects", "Int!");
                };
                let name = vars["name"].as_str().unwrap_or("");
                let include_archived = vars["includeArchived"].as_bool().unwrap_or(false);
                let found: Vec<Value> = self
                    .initiatives
                    .iter()
                    .filter(|i| {
                        (include_archived || i["archivedAt"].is_null())
                            && i["name"]
                                .as_str()
                                .is_some_and(|n| n.eq_ignore_ascii_case(name))
                    })
                    .map(|i| self.initiative_detail(i, projects))
                    .collect();
                (
                    200,
                    json!({ "data": { "initiatives": Workspace::page(found, 1) } }),
                )
            }
            "InitiativeLinks" => {
                let links: Vec<Value> = self
                    .initiative_links
                    .iter()
                    .map(|l| self.link_view(l))
                    .collect();
                (
                    200,
                    json!({ "data": { "initiativeToProjects": Workspace::page(links, first) } }),
                )
            }
            "InitiativeCreate" => {
                let input = vars["input"].clone();
                let Some(name) = input["name"].as_str().filter(|n| !n.trim().is_empty()) else {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "name should not be empty" } } ], "data": null }),
                    );
                };
                if let Some(uid) = input["ownerId"].as_str()
                    && !self.users.iter().any(|u| u["id"] == uid)
                {
                    return not_found("User");
                }
                if let Some(tid) = input["leadTeamId"].as_str()
                    && !self.teams.iter().any(|t| t["id"] == tid)
                {
                    return not_found("Team");
                }
                let initiative = self.new_initiative(name, &input);
                (
                    200,
                    json!({ "data": { "initiativeCreate": { "success": true, "initiative": Workspace::initiative_view(&initiative) } } }),
                )
            }
            "InitiativeUpdate" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(input) = vars["input"].as_object().cloned() else {
                    return missing_variable("input", "InitiativeUpdateInput!");
                };
                let Some(index) = self
                    .initiatives
                    .iter()
                    .position(|i| i["id"] == id && i["archivedAt"].is_null())
                else {
                    return not_found("Initiative");
                };
                let now = self.now();
                let mut initiative = self.initiatives[index].clone();
                for (field, value) in input {
                    match field.as_str() {
                        "ownerId" => {
                            initiative["owner"] = match value.as_str() {
                                Some(uid) => match self.users.iter().find(|u| u["id"] == uid) {
                                    Some(u) => {
                                        json!({ "id": u["id"], "name": u["name"], "displayName": u["displayName"] })
                                    }
                                    None => return not_found("User"),
                                },
                                None => Value::Null,
                            };
                        }
                        "leadTeamId" => {
                            initiative["leadTeam"] = match value.as_str() {
                                Some(tid) => match self.teams.iter().find(|t| t["id"] == tid) {
                                    Some(t) => json!({ "id": t["id"], "key": t["key"] }),
                                    None => return not_found("Team"),
                                },
                                None => Value::Null,
                            };
                        }
                        other => {
                            initiative[other] = value;
                        }
                    }
                }
                initiative["updatedAt"] = json!(now);
                self.initiatives[index] = initiative.clone();
                (
                    200,
                    json!({ "data": { "initiativeUpdate": { "success": true, "initiative": Workspace::initiative_view(&initiative) } } }),
                )
            }
            "InitiativeArchive" | "InitiativeUnarchive" => {
                let id = vars["id"].as_str().unwrap_or("");
                let archiving = op == "InitiativeArchive";
                let Some(index) = self
                    .initiatives
                    .iter()
                    .position(|i| i["id"] == id && (i["archivedAt"].is_null() == archiving))
                else {
                    return not_found("Initiative");
                };
                let now = self.now();
                self.initiatives[index]["archivedAt"] =
                    if archiving { json!(now) } else { Value::Null };
                let field = if archiving {
                    "initiativeArchive"
                } else {
                    "initiativeUnarchive"
                };
                (
                    200,
                    json!({ "data": { field: { "success": true, "entity": Workspace::initiative_view(&self.initiatives[index]) } } }),
                )
            }
            "InitiativeToProjectCreate" => {
                let input = &vars["input"];
                let (iid, pid) = (input["initiativeId"].clone(), input["projectId"].clone());
                if !self.initiatives.iter().any(|i| i["id"] == iid) {
                    return not_found("Initiative");
                }
                if !self.projects.iter().any(|p| p["id"] == pid) {
                    return not_found("Project");
                }
                // A project appears once in an initiative (the mutation's description).
                if self
                    .initiative_links
                    .iter()
                    .any(|l| l["initiativeId"] == iid && l["projectId"] == pid)
                {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "The project is already in the initiative." } } ], "data": null }),
                    );
                }
                let (id, now) = (self.id(), self.now());
                let link =
                    json!({ "id": id, "createdAt": now, "initiativeId": iid, "projectId": pid });
                self.initiative_links.push(link.clone());
                (
                    200,
                    json!({ "data": { "initiativeToProjectCreate": { "success": true, "initiativeToProject": self.link_view(&link) } } }),
                )
            }
            "InitiativeToProjectDelete" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(index) = self.initiative_links.iter().position(|l| l["id"] == id) else {
                    return not_found("InitiativeToProject");
                };
                self.initiative_links.remove(index);
                (
                    200,
                    json!({ "data": { "initiativeToProjectDelete": { "success": true, "entityId": id } } }),
                )
            }
            "Cycles" => {
                // The filter sees the cycle with its issues, so that a collection
                // clause such as the recorder's `issues: { length: { gt: 0 } }` works.
                let cycles: Vec<Value> = self
                    .cycles
                    .iter()
                    .filter(|c| {
                        c["archivedAt"].is_null()
                            && passes(&self.cycle_detail(c, PAGE_CAP_FOR_FILTERS), filter)
                    })
                    .cloned()
                    .collect();
                (
                    200,
                    json!({ "data": { "cycles": Workspace::page(cycles, first) } }),
                )
            }
            "CycleByNumber" => {
                let Some(issues) = vars["issues"].as_u64().map(|n| n as usize) else {
                    return missing_variable("issues", "Int!");
                };
                let key = vars["key"].as_str().unwrap_or("");
                let number = vars["number"].as_f64();
                let found: Vec<Value> = self
                    .cycles
                    .iter()
                    .filter(|c| {
                        c["archivedAt"].is_null()
                            && c["team"]["key"] == key
                            && c["number"].as_f64() == number
                    })
                    .map(|c| self.cycle_detail(c, issues))
                    .collect();
                (
                    200,
                    json!({ "data": { "cycles": Workspace::page(found, 1) } }),
                )
            }
            "CycleUpdate" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(input) = vars["input"].as_object().cloned() else {
                    return missing_variable("input", "CycleUpdateInput!");
                };
                let Some(index) = self.cycles.iter().position(|c| c["id"] == id) else {
                    return not_found("Cycle");
                };
                let now = self.now();
                for (field, value) in input {
                    if ["name", "description", "startsAt", "endsAt", "completedAt"]
                        .contains(&field.as_str())
                    {
                        self.cycles[index][field] = value;
                    }
                }
                self.cycles[index]["updatedAt"] = json!(now);
                (
                    200,
                    json!({ "data": { "cycleUpdate": { "success": true, "cycle": self.cycles[index].clone() } } }),
                )
            }
            "Documents" => {
                let documents: Vec<Value> = self
                    .documents
                    .iter()
                    .filter(|d| d["archivedAt"].is_null() && passes(d, filter))
                    .map(Workspace::document_view)
                    .collect();
                (
                    200,
                    json!({ "data": { "documents": Workspace::page(documents, first) } }),
                )
            }
            "DocumentByTitle" => {
                let title = vars["title"].as_str().unwrap_or("");
                let include_archived = vars["includeArchived"].as_bool().unwrap_or(false);
                let found: Vec<Value> = self
                    .documents
                    .iter()
                    .filter(|d| {
                        (include_archived || d["archivedAt"].is_null())
                            && d["title"]
                                .as_str()
                                .is_some_and(|t| t.eq_ignore_ascii_case(title))
                    })
                    .cloned()
                    .collect();
                (
                    200,
                    json!({ "data": { "documents": Workspace::page(found, 1) } }),
                )
            }
            "DocumentCreate" => {
                let input = vars["input"].clone();
                let Some(title) = input["title"].as_str().filter(|t| !t.trim().is_empty()) else {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "title should not be empty" } } ], "data": null }),
                    );
                };
                match self.new_document(title, &input) {
                    Ok(document) => (
                        200,
                        json!({ "data": { "documentCreate": { "success": true, "document": document } } }),
                    ),
                    Err(answer) => answer,
                }
            }
            "DocumentUpdate" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(input) = vars["input"].as_object().cloned() else {
                    return missing_variable("input", "DocumentUpdateInput!");
                };
                let Some(index) = self
                    .documents
                    .iter()
                    .position(|d| d["id"] == id && d["archivedAt"].is_null())
                else {
                    return not_found("Document");
                };
                let now = self.now();
                let mut document = self.documents[index].clone();
                for (field, value) in input {
                    match field.as_str() {
                        "projectId" => {
                            document["project"] = match value.as_str() {
                                Some(pid) => match self.projects.iter().find(|p| p["id"] == pid) {
                                    Some(p) => json!({ "id": p["id"], "name": p["name"] }),
                                    None => return not_found("Project"),
                                },
                                None => Value::Null,
                            };
                        }
                        "issueId" => {
                            document["issue"] = match value.as_str() {
                                Some(iid) => match self.issues.iter().find(|i| i["id"] == iid) {
                                    Some(i) => {
                                        json!({ "id": i["id"], "identifier": i["identifier"] })
                                    }
                                    None => return not_found("Issue"),
                                },
                                None => Value::Null,
                            };
                        }
                        other => {
                            document[other] = value;
                        }
                    }
                }
                if document["project"].is_null() && document["issue"].is_null() {
                    return Workspace::document_needs_a_home();
                }
                document["updatedAt"] = json!(now);
                let me = &self.users[0];
                document["updatedBy"] =
                    json!({ "id": me["id"], "name": me["name"], "displayName": me["displayName"] });
                self.documents[index] = document.clone();
                (
                    200,
                    json!({ "data": { "documentUpdate": { "success": true, "document": document } } }),
                )
            }
            "DocumentDelete" | "DocumentUnarchive" => {
                let id = vars["id"].as_str().unwrap_or("");
                let deleting = op == "DocumentDelete";
                let Some(index) = self
                    .documents
                    .iter()
                    .position(|d| d["id"] == id && (d["archivedAt"].is_null() == deleting))
                else {
                    return not_found("Document");
                };
                let now = self.now();
                if deleting {
                    self.documents[index]["archivedAt"] = json!(now);
                    self.documents[index]["trashed"] = json!(true);
                } else {
                    self.documents[index]["archivedAt"] = Value::Null;
                    self.documents[index]["trashed"] = Value::Null;
                }
                let field = if deleting {
                    "documentDelete"
                } else {
                    "documentUnarchive"
                };
                (
                    200,
                    json!({ "data": { field: { "success": true, "entity": Workspace::document_view(&self.documents[index]) } } }),
                )
            }
            "Templates" => {
                let include_archived = vars["includeArchived"].as_bool().unwrap_or(false);
                let templates: Vec<Value> = self
                    .templates
                    .iter()
                    .filter(|t| {
                        (include_archived || t["archivedAt"].is_null()) && passes(t, filter)
                    })
                    .take(first)
                    .cloned()
                    .collect();
                (200, json!({ "data": { "templateSearch": templates } }))
            }
            "TemplateByName" => {
                // The name in the team's scope, its own and the workspace's, two at
                // most, as the operation's filter and `first` ask (ADR 0014, group 15).
                let key = vars["key"].as_str().unwrap_or("");
                let name = vars["name"].as_str().unwrap_or("");
                let include_archived = vars["includeArchived"].as_bool().unwrap_or(false);
                let found: Vec<Value> = self
                    .templates
                    .iter()
                    .filter(|t| {
                        (include_archived || t["archivedAt"].is_null())
                            && t["name"]
                                .as_str()
                                .is_some_and(|n| n.eq_ignore_ascii_case(name))
                            && (t["team"].is_null() || t["team"]["key"] == key)
                    })
                    .take(2)
                    .cloned()
                    .collect();
                (200, json!({ "data": { "templateSearch": found } }))
            }
            "TemplateCreate" => {
                let input = vars["input"].clone();
                if input["name"].as_str().is_none_or(|n| n.trim().is_empty()) {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "name should not be empty" } } ], "data": null }),
                    );
                }
                if !["issue", "project", "document"].contains(&input["type"].as_str().unwrap_or(""))
                {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "type must be one of issue, project, document" } } ], "data": null }),
                    );
                }
                match self.new_template(&input) {
                    Ok(template) => (
                        200,
                        json!({ "data": { "templateCreate": { "success": true, "template": template } } }),
                    ),
                    Err(answer) => answer,
                }
            }
            "TemplateUpdate" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(input) = vars["input"].as_object().cloned() else {
                    return missing_variable("input", "TemplateUpdateInput!");
                };
                let Some(index) = self
                    .templates
                    .iter()
                    .position(|t| t["id"] == id && t["archivedAt"].is_null())
                else {
                    return not_found("Template");
                };
                let now = self.now();
                for (field, value) in input {
                    if field == "templateData" {
                        self.templates[index][field] = Workspace::template_data_string(&value);
                    } else if ["name", "description", "icon", "color", "sortOrder"]
                        .contains(&field.as_str())
                    {
                        self.templates[index][field] = value;
                    }
                }
                self.templates[index]["updatedAt"] = json!(now);
                (
                    200,
                    json!({ "data": { "templateUpdate": { "success": true, "template": self.templates[index].clone() } } }),
                )
            }
            "TemplateDelete" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(index) = self.templates.iter().position(|t| t["id"] == id) else {
                    return not_found("Template");
                };
                self.templates.remove(index);
                (
                    200,
                    json!({ "data": { "templateDelete": { "success": true, "entityId": id } } }),
                )
            }
            "Webhooks" => {
                let hooks: Vec<Value> = self
                    .webhooks
                    .iter()
                    .filter(|h| h["archivedAt"].is_null())
                    .map(Workspace::webhook_view)
                    .collect();
                (
                    200,
                    json!({ "data": { "webhooks": Workspace::page(hooks, first) } }),
                )
            }
            "WebhookCreate" => {
                let input = vars["input"].clone();
                let url = input["url"].as_str().unwrap_or("");
                if !url.starts_with("https://") {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "url must be an https URL" } } ], "data": null }),
                    );
                }
                if input["resourceTypes"]
                    .as_array()
                    .is_none_or(|t| t.is_empty())
                {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "resourceTypes should not be empty" } } ], "data": null }),
                    );
                }
                match self.new_webhook(&input) {
                    Ok(hook) => (
                        200,
                        json!({ "data": { "webhookCreate": { "success": true, "webhook": Workspace::webhook_view(&hook) } } }),
                    ),
                    Err(answer) => answer,
                }
            }
            "WebhookUpdate" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(input) = vars["input"].as_object().cloned() else {
                    return missing_variable("input", "WebhookUpdateInput!");
                };
                let Some(index) = self
                    .webhooks
                    .iter()
                    .position(|h| h["id"] == id && h["archivedAt"].is_null())
                else {
                    return not_found("Webhook");
                };
                let now = self.now();
                for (field, value) in input {
                    if ["label", "url", "enabled", "resourceTypes", "secret"]
                        .contains(&field.as_str())
                    {
                        self.webhooks[index][field] = value;
                    }
                }
                self.webhooks[index]["updatedAt"] = json!(now);
                (
                    200,
                    json!({ "data": { "webhookUpdate": { "success": true, "webhook": Workspace::webhook_view(&self.webhooks[index]) } } }),
                )
            }
            "WebhookDelete" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(index) = self.webhooks.iter().position(|h| h["id"] == id) else {
                    return not_found("Webhook");
                };
                self.webhooks.remove(index);
                (
                    200,
                    json!({ "data": { "webhookDelete": { "success": true, "entityId": id } } }),
                )
            }
            "AuditEntries" => {
                let entries: Vec<Value> = self
                    .audit_entries
                    .iter()
                    .filter(|e| passes(e, filter))
                    .cloned()
                    .collect();
                (
                    200,
                    json!({ "data": { "auditEntries": Workspace::page(entries, first) } }),
                )
            }
            "ProjectStatusCreate" => {
                let input = &vars["input"];
                let (Some(name), Some(kind), Some(color), Some(position)) = (
                    input["name"].as_str().filter(|n| !n.trim().is_empty()),
                    input["type"].as_str(),
                    input["color"].as_str(),
                    input["position"].as_f64(),
                ) else {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "name, type, color and position are required" } } ], "data": null }),
                    );
                };
                let sid = self.id();
                let status = json!({ "id": sid, "name": name, "type": kind, "color": color, "description": input["description"], "position": position, "indefinite": input["indefinite"].as_bool().unwrap_or(true), "archivedAt": null });
                self.project_statuses.push(status.clone());
                (
                    200,
                    json!({ "data": { "projectStatusCreate": { "success": true, "status": Workspace::state_view(&status) } } }),
                )
            }
            "ProjectStatusUpdate" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(input) = vars["input"].as_object().cloned() else {
                    return missing_variable("input", "ProjectStatusUpdateInput!");
                };
                let Some(index) = self
                    .project_statuses
                    .iter()
                    .position(|s| s["id"] == id && s["archivedAt"].is_null())
                else {
                    return not_found("ProjectStatus");
                };
                for (field, value) in input {
                    self.project_statuses[index][&field] = value;
                }
                let (sid, sname) = (
                    self.project_statuses[index]["id"].clone(),
                    self.project_statuses[index]["name"].clone(),
                );
                for project in &mut self.projects {
                    if project["status"]["id"] == sid {
                        project["status"]["name"] = sname.clone();
                    }
                }
                (
                    200,
                    json!({ "data": { "projectStatusUpdate": { "success": true, "status": Workspace::state_view(&self.project_statuses[index]) } } }),
                )
            }
            "ProjectStatusArchive" | "ProjectStatusUnarchive" => {
                let id = vars["id"].as_str().unwrap_or("");
                let archiving = op == "ProjectStatusArchive";
                let Some(index) = self
                    .project_statuses
                    .iter()
                    .position(|s| s["id"] == id && (s["archivedAt"].is_null() == archiving))
                else {
                    return not_found("ProjectStatus");
                };
                if archiving {
                    // A status with active projects, or the last of its type, is
                    // refused (the mutation's own description).
                    let kind = self.project_statuses[index]["type"].clone();
                    let in_use = self
                        .projects
                        .iter()
                        .any(|p| p["status"]["id"] == id && p["archivedAt"].is_null());
                    let last_of_type = self
                        .project_statuses
                        .iter()
                        .filter(|s| s["type"] == kind && s["archivedAt"].is_null())
                        .count()
                        == 1;
                    if in_use || last_of_type {
                        return (
                            200,
                            json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": if in_use { "Cannot archive a project status with active projects." } else { "Cannot archive the last project status of its type." } } } ], "data": null }),
                        );
                    }
                }
                let now = self.now();
                self.project_statuses[index]["archivedAt"] =
                    if archiving { json!(now) } else { Value::Null };
                let field = if archiving {
                    "projectStatusArchive"
                } else {
                    "projectStatusUnarchive"
                };
                (
                    200,
                    json!({ "data": { field: { "success": true, "entity": Workspace::state_view(&self.project_statuses[index]) } } }),
                )
            }
            "ProjectUpdate" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(teams) = vars["teams"].as_u64().map(|n| n as usize) else {
                    return missing_variable("teams", "Int!");
                };
                let Some(input) = vars["input"].as_object().cloned() else {
                    return missing_variable("input", "ProjectUpdateInput!");
                };
                let Some(index) = self
                    .projects
                    .iter()
                    .position(|p| p["id"] == id && p["archivedAt"].is_null())
                else {
                    return not_found("Project");
                };
                let now = self.now();
                let mut project = self.projects[index].clone();
                for (field, value) in input {
                    match field.as_str() {
                        "statusId" => {
                            project["status"] = match self
                                .project_statuses
                                .iter()
                                .find(|s| s["id"] == value)
                            {
                                Some(s) => {
                                    json!({ "id": s["id"], "name": s["name"], "type": s["type"] })
                                }
                                None => return not_found("ProjectStatus"),
                            };
                        }
                        "leadId" => {
                            project["lead"] = match value.as_str() {
                                Some(uid) => match self.users.iter().find(|u| u["id"] == uid) {
                                    Some(u) => {
                                        json!({ "id": u["id"], "name": u["name"], "displayName": u["displayName"] })
                                    }
                                    None => return not_found("User"),
                                },
                                None => Value::Null,
                            };
                        }
                        "teamIds" => {
                            let ids: Vec<Value> = value.as_array().cloned().unwrap_or_default();
                            let teams: Option<Vec<Value>> = ids
                                .iter()
                                .map(|tid| {
                                    self.teams
                                        .iter()
                                        .find(|t| &t["id"] == tid)
                                        .map(|t| json!({ "id": t["id"], "key": t["key"] }))
                                })
                                .collect();
                            match teams {
                                Some(list) if !list.is_empty() => {
                                    project["teams"] = json!({ "nodes": list })
                                }
                                _ => return not_found("Team"),
                            }
                        }
                        "memberIds" => {
                            let ids: Vec<Value> = value.as_array().cloned().unwrap_or_default();
                            if ids
                                .iter()
                                .any(|uid| !self.users.iter().any(|u| &u["id"] == uid))
                            {
                                return not_found("User");
                            }
                            project["memberIds"] = json!(ids);
                        }
                        other => {
                            project[other] = value;
                        }
                    }
                }
                project["updatedAt"] = json!(now);
                self.projects[index] = project.clone();
                (
                    200,
                    json!({ "data": { "projectUpdate": { "success": true, "project": Workspace::project_view(&project, teams) } } }),
                )
            }
            "ProjectDelete" | "ProjectUnarchive" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(teams) = vars["teams"].as_u64().map(|n| n as usize) else {
                    return missing_variable("teams", "Int!");
                };
                let deleting = op == "ProjectDelete";
                let Some(index) = self
                    .projects
                    .iter()
                    .position(|p| p["id"] == id && (p["archivedAt"].is_null() == deleting))
                else {
                    return not_found("Project");
                };
                let now = self.now();
                let project = &mut self.projects[index];
                if deleting {
                    project["archivedAt"] = json!(now);
                    project["trashed"] = json!(true);
                } else {
                    project["archivedAt"] = Value::Null;
                    project["trashed"] = Value::Null;
                }
                let view = Workspace::project_view(project, teams);
                let field = if deleting {
                    "projectDelete"
                } else {
                    "projectUnarchive"
                };
                (
                    200,
                    json!({ "data": { field: { "success": true, "entity": view } } }),
                )
            }
            "WorkflowStateCreate" => {
                let input = &vars["input"];
                let Some(team) = self
                    .teams
                    .iter()
                    .find(|t| t["id"] == input["teamId"])
                    .cloned()
                else {
                    return not_found("Team");
                };
                let (Some(name), Some(kind), Some(color)) = (
                    input["name"].as_str().filter(|n| !n.trim().is_empty()),
                    input["type"].as_str(),
                    input["color"].as_str(),
                ) else {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "name, type and color are required" } } ], "data": null }),
                    );
                };
                if !["backlog", "unstarted", "started", "completed", "canceled"].contains(&kind) {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "type must be one of backlog, unstarted, started, completed, canceled" } } ], "data": null }),
                    );
                }
                let position = match input["position"].as_f64() {
                    Some(p) => p,
                    None => {
                        self.states
                            .iter()
                            .filter(|s| s["team"]["id"] == team["id"])
                            .filter_map(|s| s["position"].as_f64())
                            .fold(0.0, f64::max)
                            + 1.0
                    }
                };
                let sid = self.id();
                let state = json!({ "id": sid, "name": name, "type": kind, "position": position, "color": color, "description": input["description"], "team": { "id": team["id"], "key": team["key"] }, "archivedAt": null });
                self.states.push(state.clone());
                (
                    200,
                    json!({ "data": { "workflowStateCreate": { "success": true, "workflowState": Workspace::state_view(&state) } } }),
                )
            }
            "WorkflowStateUpdate" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(input) = vars["input"].as_object().cloned() else {
                    return missing_variable("input", "WorkflowStateUpdateInput!");
                };
                let Some(index) = self
                    .states
                    .iter()
                    .position(|s| s["id"] == id && s["archivedAt"].is_null())
                else {
                    return not_found("WorkflowState");
                };
                for (field, value) in input {
                    self.states[index][&field] = value;
                }
                // Issues carry the state's name, so a rename shows on them too.
                let (sid, sname) = (
                    self.states[index]["id"].clone(),
                    self.states[index]["name"].clone(),
                );
                for issue in &mut self.issues {
                    if issue["state"]["id"] == sid {
                        issue["state"]["name"] = sname.clone();
                    }
                }
                (
                    200,
                    json!({ "data": { "workflowStateUpdate": { "success": true, "workflowState": Workspace::state_view(&self.states[index]) } } }),
                )
            }
            "WorkflowStateArchive" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(index) = self
                    .states
                    .iter()
                    .position(|s| s["id"] == id && s["archivedAt"].is_null())
                else {
                    return not_found("WorkflowState");
                };
                // Only a state whose issues are all archived can be archived (the
                // mutation's own description).
                if self
                    .issues
                    .iter()
                    .any(|i| i["state"]["id"] == id && i["archivedAt"].is_null())
                {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "Cannot archive a workflow state with active issues." } } ], "data": null }),
                    );
                }
                let now = self.now();
                self.states[index]["archivedAt"] = json!(now);
                (
                    200,
                    json!({ "data": { "workflowStateArchive": { "success": true, "entity": Workspace::state_view(&self.states[index]) } } }),
                )
            }
            "IssueLabelCreate" => {
                let input = &vars["input"];
                let Some(name) = input["name"].as_str().filter(|n| !n.trim().is_empty()) else {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "name should not be empty" } } ], "data": null }),
                    );
                };
                let team = match input["teamId"].as_str() {
                    Some(tid) => match self.teams.iter().find(|t| t["id"] == tid).cloned() {
                        Some(t) => Some(t),
                        None => return not_found("Team"),
                    },
                    None => None,
                };
                let parent = match input["parentId"].as_str() {
                    Some(pid) => match self.labels.iter().find(|l| l["id"] == pid) {
                        Some(p) => json!({ "id": p["id"], "name": p["name"] }),
                        None => return not_found("IssueLabel"),
                    },
                    None => Value::Null,
                };
                let mut label = self.new_label(
                    name,
                    input["color"].as_str().unwrap_or("#95a2b3"),
                    input["description"].as_str(),
                    team.as_ref(),
                    input["isGroup"].as_bool().unwrap_or(false),
                );
                label["parent"] = parent;
                self.labels.push(label.clone());
                (
                    200,
                    json!({ "data": { "issueLabelCreate": { "success": true, "issueLabel": label } } }),
                )
            }
            "IssueLabelUpdate" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(input) = vars["input"].as_object().cloned() else {
                    return missing_variable("input", "IssueLabelUpdateInput!");
                };
                let Some(index) = self.labels.iter().position(|l| l["id"] == id) else {
                    return not_found("IssueLabel");
                };
                for (field, value) in input {
                    if field == "parentId" {
                        self.labels[index]["parent"] = match value.as_str() {
                            Some(pid) => match self.labels.iter().find(|l| l["id"] == pid) {
                                Some(p) => json!({ "id": p["id"], "name": p["name"] }),
                                None => return not_found("IssueLabel"),
                            },
                            None => Value::Null,
                        };
                    } else {
                        self.labels[index][&field] = value;
                    }
                }
                // Issues carry the label's name, so a rename shows on them too.
                let (lid, lname) = (
                    self.labels[index]["id"].clone(),
                    self.labels[index]["name"].clone(),
                );
                for issue in &mut self.issues {
                    if let Some(nodes) = issue["labels"]["nodes"].as_array_mut() {
                        for node in nodes.iter_mut() {
                            if node["id"] == lid {
                                node["name"] = lname.clone();
                            }
                        }
                    }
                }
                (
                    200,
                    json!({ "data": { "issueLabelUpdate": { "success": true, "issueLabel": self.labels[index] } } }),
                )
            }
            "IssueLabelRetire" | "IssueLabelRestore" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(index) = self.labels.iter().position(|l| l["id"] == id) else {
                    return not_found("IssueLabel");
                };
                let now = self.now();
                let field = if op == "IssueLabelRetire" {
                    self.labels[index]["retiredAt"] = json!(now);
                    "issueLabelRetire"
                } else {
                    self.labels[index]["retiredAt"] = Value::Null;
                    "issueLabelRestore"
                };
                (
                    200,
                    json!({ "data": { field: { "success": true, "issueLabel": self.labels[index] } } }),
                )
            }
            "IssueLabelDelete" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(index) = self.labels.iter().position(|l| l["id"] == id) else {
                    return not_found("IssueLabel");
                };
                self.labels.remove(index);
                // Linear removes a deleted label from every issue (its pages, 2026-11-03).
                for issue in &mut self.issues {
                    if let Some(nodes) = issue["labels"]["nodes"].as_array_mut() {
                        nodes.retain(|n| n["id"] != id);
                    }
                }
                (
                    200,
                    json!({ "data": { "issueLabelDelete": { "success": true, "entityId": id } } }),
                )
            }
            "UserById" => {
                let id = vars["id"].as_str().unwrap_or("");
                match self
                    .users
                    .iter()
                    .find(|u| u["id"] == id || (id == "me" && u["isMe"] == true))
                {
                    Some(user) => (200, json!({ "data": { "user": user } })),
                    None => not_found("User"),
                }
            }
            "UserUpdate" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(input) = vars["input"].as_object().cloned() else {
                    return missing_variable("input", "UserUpdateInput!");
                };
                let Some(index) = self
                    .users
                    .iter()
                    .position(|u| u["id"] == id || (id == "me" && u["isMe"] == true))
                else {
                    return not_found("User");
                };
                let now = self.now();
                for (field, value) in input {
                    self.users[index][&field] = value;
                }
                self.users[index]["updatedAt"] = json!(now);
                (
                    200,
                    json!({ "data": { "userUpdate": { "success": true, "user": self.users[index] } } }),
                )
            }
            "UserChangeRole" => {
                let id = vars["id"].as_str().unwrap_or("");
                let role = vars["role"].as_str().unwrap_or("");
                let Some(index) = self.users.iter().position(|u| u["id"] == id) else {
                    return not_found("User");
                };
                let (owner, admin, guest) = match role {
                    "owner" => (true, true, false),
                    "admin" => (false, true, false),
                    "user" => (false, false, false),
                    "guest" => (false, false, true),
                    _ => {
                        return (
                            400,
                            json!({ "errors": [ { "message": format!("Variable \"$role\" got invalid value {role:?}"), "extensions": { "code": "GRAPHQL_VALIDATION_FAILED", "type": "graphql error", "userError": true } } ] }),
                        );
                    }
                };
                let user = &mut self.users[index];
                user["owner"] = json!(owner);
                user["admin"] = json!(admin);
                user["guest"] = json!(guest);
                (
                    200,
                    json!({ "data": { "userChangeRole": { "success": true } } }),
                )
            }
            "UserSuspend" | "UserUnsuspend" | "UserRevokeAllSessions" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(index) = self.users.iter().position(|u| u["id"] == id) else {
                    return not_found("User");
                };
                let field = match op {
                    "UserSuspend" => {
                        self.users[index]["active"] = json!(false);
                        self.users[index]["disableReason"] = json!("suspended");
                        "userSuspend"
                    }
                    "UserUnsuspend" => {
                        self.users[index]["active"] = json!(true);
                        self.users[index]["disableReason"] = Value::Null;
                        "userUnsuspend"
                    }
                    _ => "userRevokeAllSessions",
                };
                (200, json!({ "data": { field: { "success": true } } }))
            }
            "IssueById" => {
                let id = vars["id"].as_str().unwrap_or("");
                let comments = vars["comments"].as_u64().unwrap_or(10) as usize;
                let attachments = vars["attachments"].as_u64().unwrap_or(10) as usize;
                let children = vars["children"].as_u64().unwrap_or(10) as usize;
                match self.issues.iter().find(|i| {
                    i["id"] == id
                        || i["identifier"]
                            .as_str()
                            .is_some_and(|x| x.eq_ignore_ascii_case(id))
                }) {
                    Some(issue) => (
                        200,
                        json!({ "data": { "issue": self.full_issue(issue, comments, attachments, children) } }),
                    ),
                    None => not_found("Issue"),
                }
            }
            "Issues" => {
                // Filtered with their comments, attachments and children attached, so
                // that a collection filter (`children: { length: { gt: 0 } }`) sees
                // them; answered as Linear answers `issues`, without them. Archived
                // issues are left out unless `includeArchived` is true, as Linear
                // leaves them out (the number lookup of `issue unarchive 12` asks).
                let include_archived = vars["includeArchived"].as_bool().unwrap_or(false);
                let mut issues: Vec<Value> = self
                    .issues
                    .iter()
                    .filter(|i| {
                        (include_archived || i["archivedAt"].is_null())
                            && passes(&self.filter_view(i), filter)
                    })
                    .cloned()
                    .collect();
                match vars["orderBy"].as_str() {
                    // Newest first, as Linear orders `createdAt` (2026-11-03).
                    Some("createdAt") => {
                        issues.sort_by(|a, b| b["createdAt"].as_str().cmp(&a["createdAt"].as_str()))
                    }
                    _ => {
                        issues.sort_by(|a, b| b["updatedAt"].as_str().cmp(&a["updatedAt"].as_str()))
                    }
                }
                (
                    200,
                    json!({ "data": { "issues": Workspace::page(issues, first) } }),
                )
            }
            "IssueCreate" => {
                let input = &vars["input"];
                let Some(team) = self
                    .teams
                    .iter()
                    .find(|t| t["id"] == input["teamId"])
                    .cloned()
                else {
                    return not_found("Team");
                };
                let Some(title) = input["title"].as_str().filter(|t| !t.is_empty()) else {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "title should not be empty" } } ], "data": null }),
                    );
                };
                let team_key = team["key"].as_str().unwrap().to_string();
                let state_name = match input["stateId"].as_str() {
                    Some(sid) => match self
                        .states
                        .iter()
                        .find(|s| s["id"] == sid && s["team"]["key"] == team_key)
                    {
                        Some(s) => s["name"].as_str().unwrap().to_string(),
                        None => return not_found("WorkflowState"),
                    },
                    None => "Backlog".to_string(),
                };
                let label_ids: Vec<Value> =
                    input["labelIds"].as_array().cloned().unwrap_or_default();
                if label_ids
                    .iter()
                    .any(|l| !self.labels.iter().any(|x| &x["id"] == l))
                {
                    return not_found("IssueLabel");
                }
                // A retired label cannot be applied to a new issue (Linear's
                // description of `issueLabelRetire`).
                if label_ids.iter().any(|l| {
                    self.labels
                        .iter()
                        .any(|x| &x["id"] == l && !x["retiredAt"].is_null())
                }) {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "A retired label cannot be applied." } } ], "data": null }),
                    );
                }
                let project = match input["projectId"].as_str() {
                    Some(pid) => match self.projects.iter().find(|p| p["id"] == pid) {
                        Some(p) => Some(p.clone()),
                        None => return not_found("Project"),
                    },
                    None => None,
                };
                let assignee = match input["assigneeId"].as_str() {
                    Some(uid) => match self.users.iter().find(|u| u["id"] == uid) {
                        Some(u) => Some(u.clone()),
                        None => return not_found("User"),
                    },
                    None => None,
                };
                let parent = match input["parentId"].as_str() {
                    Some(pid) => match self.issues.iter().find(|i| i["id"] == pid) {
                        Some(p) => Some(p.clone()),
                        None => return not_found("Issue"),
                    },
                    None => None,
                };
                let milestone = match input.get("projectMilestoneId") {
                    Some(_) => match &project {
                        Some(p) => match self.milestone_for(input, p) {
                            Ok(m) => m,
                            Err(answer) => return answer,
                        },
                        None => return not_found("ProjectMilestone"),
                    },
                    None => Value::Null,
                };
                let cycle = match input.get("cycleId") {
                    Some(_) => match self.cycle_for(input, &team["id"]) {
                        Ok(c) => c,
                        Err(answer) => return answer,
                    },
                    None => Value::Null,
                };
                let mut issue = self.new_issue(
                    &team_key,
                    title,
                    input["description"].as_str(),
                    &state_name,
                    &label_ids,
                    input["priority"].as_u64().unwrap_or(0),
                    project,
                    assignee.as_ref(),
                    parent.as_ref(),
                );
                issue["projectMilestone"] = milestone;
                issue["cycle"] = cycle;
                self.issues.push(issue.clone());
                (
                    200,
                    json!({ "data": { "issueCreate": { "success": true, "issue": issue } } }),
                )
            }
            "IssueArchive" | "IssueUnarchive" | "IssueDelete" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(index) = self.issues.iter().position(|i| i["id"] == id) else {
                    return not_found("Issue");
                };
                let now = self.now();
                let field = match op {
                    "IssueArchive" => {
                        self.issues[index]["archivedAt"] = json!(now);
                        "issueArchive"
                    }
                    "IssueDelete" => {
                        // The trash: archived, with the sub-issues, as Linear does.
                        // `archivedAt` is the whole signal `IssueFields` carries; an
                        // issue has no `trashed` field in Linear's answer (the
                        // IssueById and Issues fixtures), so the fake writes none.
                        self.issues[index]["archivedAt"] = json!(now);
                        let parent_id = self.issues[index]["id"].clone();
                        for child in self
                            .issues
                            .iter_mut()
                            .filter(|i| i["parent"]["id"] == parent_id)
                        {
                            child["archivedAt"] = json!(now);
                        }
                        "issueDelete"
                    }
                    _ => {
                        self.issues[index]["archivedAt"] = Value::Null;
                        "issueUnarchive"
                    }
                };
                (
                    200,
                    json!({ "data": { field: { "success": true, "entity": self.issues[index].clone() } } }),
                )
            }
            "IssueBatchUpdate" => {
                // Linear's cap, read on 2026-11-03: fifty ids in one request.
                let ids: Vec<String> = vars["ids"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                if ids.is_empty() || ids.len() > 50 {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "ids must have between 1 and 50 items" } } ], "data": null }),
                    );
                }
                let mut issues = Vec::new();
                for id in ids {
                    let (status, answer) =
                        self.dispatch("IssueUpdate", &json!({ "id": id, "input": vars["input"] }));
                    if answer.get("errors").is_some() || status != 200 {
                        return (status, answer);
                    }
                    issues.push(answer["data"]["issueUpdate"]["issue"].clone());
                }
                (
                    200,
                    json!({ "data": { "issueBatchUpdate": { "success": true, "issues": issues } } }),
                )
            }
            "IssueUpdate" => {
                let id = vars["id"].as_str().unwrap_or("");
                let input = vars["input"].clone();
                let Some(index) = self.issues.iter().position(|i| i["id"] == id) else {
                    return not_found("Issue");
                };
                let team_key = self.issues[index]["team"]["key"]
                    .as_str()
                    .unwrap()
                    .to_string();
                let now = self.now();
                let mut issue = self.issues[index].clone();
                if let Some(title) = input["title"].as_str() {
                    issue["title"] = json!(title);
                }
                if input.get("description").is_some() {
                    issue["description"] = input["description"].clone();
                }
                if let Some(sid) = input["stateId"].as_str() {
                    let Some(state) = self
                        .states
                        .iter()
                        .find(|s| s["id"] == sid && s["team"]["key"] == team_key)
                        .cloned()
                    else {
                        return not_found("WorkflowState");
                    };
                    issue["state"] =
                        json!({ "id": state["id"], "name": state["name"], "type": state["type"] });
                    issue["completedAt"] = if state["type"] == "completed" {
                        json!(now)
                    } else {
                        Value::Null
                    };
                    issue["canceledAt"] = if state["type"] == "canceled" {
                        json!(now)
                    } else {
                        Value::Null
                    };
                    if state["type"] == "started" {
                        issue["startedAt"] = json!(now);
                    }
                }
                let mut labels: Vec<Value> = issue["labels"]["nodes"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                if let Some(set) = input["labelIds"].as_array() {
                    labels = self
                        .labels
                        .iter()
                        .filter(|l| set.contains(&l["id"]))
                        .map(|l| json!({ "id": l["id"], "name": l["name"] }))
                        .collect();
                }
                for added in input["addedLabelIds"].as_array().into_iter().flatten() {
                    match self.labels.iter().find(|l| &l["id"] == added) {
                        Some(l) if !labels.iter().any(|x| x["id"] == l["id"]) => {
                            labels.push(json!({ "id": l["id"], "name": l["name"] }))
                        }
                        Some(_) => {}
                        None => return not_found("IssueLabel"),
                    }
                }
                for removed in input["removedLabelIds"].as_array().into_iter().flatten() {
                    labels.retain(|l| &l["id"] != removed);
                }
                issue["labels"] = json!({ "nodes": labels });
                if let Some(priority) = input["priority"].as_u64() {
                    issue["priority"] = json!(priority);
                    issue["priorityLabel"] = json!(priority_label(priority));
                }
                if input.get("assigneeId").is_some() {
                    issue["assignee"] = match input["assigneeId"].as_str() {
                        Some(uid) => match self.users.iter().find(|u| u["id"] == uid) {
                            Some(u) => {
                                json!({ "id": u["id"], "name": u["name"], "displayName": u["displayName"], "email": u["email"] })
                            }
                            None => return not_found("User"),
                        },
                        None => Value::Null,
                    };
                }
                if input.get("projectId").is_some() {
                    issue["project"] = match input["projectId"].as_str() {
                        Some(pid) => match self.projects.iter().find(|p| p["id"] == pid) {
                            Some(p) => json!({ "id": p["id"], "name": p["name"] }),
                            None => return not_found("Project"),
                        },
                        None => Value::Null,
                    };
                }
                if input.get("projectMilestoneId").is_some() {
                    let project = match issue["project"]["id"].as_str() {
                        Some(pid) => self.projects.iter().find(|p| p["id"] == pid).cloned(),
                        None => None,
                    };
                    issue["projectMilestone"] =
                        match (&project, input["projectMilestoneId"].as_str()) {
                            (_, None) => Value::Null,
                            (Some(p), Some(_)) => match self.milestone_for(&input, p) {
                                Ok(m) => m,
                                Err(answer) => return answer,
                            },
                            (None, Some(_)) => return not_found("ProjectMilestone"),
                        };
                }
                if input.get("cycleId").is_some() {
                    issue["cycle"] = match self.cycle_for(&input, &issue["team"]["id"]) {
                        Ok(c) => c,
                        Err(answer) => return answer,
                    };
                }
                if input.get("parentId").is_some() {
                    issue["parent"] = match input["parentId"].as_str() {
                        Some(pid) => match self.issues.iter().find(|i| i["id"] == pid) {
                            Some(p) => json!({ "id": p["id"], "identifier": p["identifier"] }),
                            None => return not_found("Issue"),
                        },
                        None => Value::Null,
                    };
                }
                issue["updatedAt"] = json!(now);
                self.issues[index] = issue.clone();
                (
                    200,
                    json!({ "data": { "issueUpdate": { "success": true, "issue": issue } } }),
                )
            }
            "ProjectCreate" => {
                let input = &vars["input"];
                let Some(name) = input["name"].as_str().filter(|n| !n.is_empty()) else {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "name should not be empty" } } ], "data": null }),
                    );
                };
                let team_ids: Vec<Value> = input["teamIds"].as_array().cloned().unwrap_or_default();
                let teams: Vec<Value> = team_ids
                    .iter()
                    .map(|id| {
                        self.teams
                            .iter()
                            .find(|t| &t["id"] == id)
                            .map(|t| json!({ "id": t["id"], "key": t["key"] }))
                    })
                    .collect::<Option<Vec<Value>>>()
                    .unwrap_or_default();
                if teams.is_empty() || teams.len() != team_ids.len() {
                    return not_found("Team");
                }
                let slug: String = name
                    .to_ascii_lowercase()
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                    .collect();
                let _ = slug;
                let backlog = self.project_statuses[0].clone();
                let team_values: Vec<Value> = teams
                    .iter()
                    .filter_map(|t| self.teams.iter().find(|x| x["id"] == t["id"]).cloned())
                    .collect();
                let project = self.new_project(
                    name,
                    input["description"].as_str().unwrap_or(""),
                    &team_values,
                    &backlog,
                );
                let Some(teams) = vars["teams"].as_u64().map(|n| n as usize) else {
                    return missing_variable("teams", "Int!");
                };
                self.projects.push(project.clone());
                (
                    200,
                    json!({ "data": { "projectCreate": { "success": true, "project": Workspace::project_view(&project, teams) } } }),
                )
            }
            "AttachmentUpdate" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(index) = self.attachments.iter().position(|a| a["id"] == id) else {
                    return not_found("Attachment");
                };
                let Some(title) = vars["input"]["title"].as_str().filter(|t| !t.is_empty()) else {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "title should not be empty" } } ], "data": null }),
                    );
                };
                self.attachments[index]["title"] = json!(title);
                if let Some(subtitle) = vars["input"].get("subtitle") {
                    self.attachments[index]["subtitle"] = subtitle.clone();
                }
                let mut attachment = self.attachments[index].clone();
                attachment.as_object_mut().unwrap().remove("issueId");
                (
                    200,
                    json!({ "data": { "attachmentUpdate": { "success": true, "attachment": attachment } } }),
                )
            }
            "AttachmentDelete" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(index) = self.attachments.iter().position(|a| a["id"] == id) else {
                    return not_found("Attachment");
                };
                self.attachments.remove(index);
                (
                    200,
                    json!({ "data": { "attachmentDelete": { "success": true, "entityId": id } } }),
                )
            }
            "AttachmentCreate" => {
                let input = &vars["input"];
                let issue_id = input["issueId"].clone();
                if !self.issues.iter().any(|i| i["id"] == issue_id) {
                    return not_found("Issue");
                }
                let (Some(title), Some(url)) = (
                    input["title"].as_str().filter(|t| !t.is_empty()),
                    input["url"].as_str().filter(|u| !u.is_empty()),
                ) else {
                    return (
                        200,
                        json!({ "errors": [ { "message": "Argument Validation Error", "extensions": { "type": "invalid input", "code": "INPUT_ERROR", "statusCode": 400, "userError": true, "userPresentableMessage": "title and url should not be empty" } } ], "data": null }),
                    );
                };
                // A URL the issue already carries is updated in place, same id, as
                // Linear does (appshapes, 2026-11-03: a second `attachmentCreate` with
                // the URL returned the first attachment's id with the new title).
                let existing = self
                    .attachments
                    .iter()
                    .position(|a| a["issueId"] == issue_id && a["url"] == url);
                // Only what is sent changes: a subtitle left out stays (read on
                // APP-16, 2026-11-03).
                let mut attachment = match existing {
                    Some(index) => {
                        let a = &mut self.attachments[index];
                        a["title"] = json!(title);
                        if let Some(subtitle) = input["subtitle"].as_str() {
                            a["subtitle"] = json!(subtitle);
                        }
                        a.clone()
                    }
                    None => self.new_attachment(&issue_id, title, input["subtitle"].as_str(), url),
                };
                attachment.as_object_mut().unwrap().remove("issueId");
                (
                    200,
                    json!({ "data": { "attachmentCreate": { "success": true, "attachment": attachment } } }),
                )
            }
            "CommentUpdate" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(index) = self.comments.iter().position(|c| c["id"] == id) else {
                    return not_found("Comment");
                };
                let Some(body) = vars["input"]["body"].as_str() else {
                    return missing_variable("input", "CommentUpdateInput!");
                };
                let now = self.now();
                self.comments[index]["body"] = json!(body);
                self.comments[index]["updatedAt"] = json!(now);
                self.comments[index]["editedAt"] = json!(now);
                let mut comment = self.comments[index].clone();
                comment.as_object_mut().unwrap().remove("issueId");
                (
                    200,
                    json!({ "data": { "commentUpdate": { "success": true, "comment": comment } } }),
                )
            }
            "CommentDelete" => {
                let id = vars["id"].as_str().unwrap_or("");
                let Some(index) = self.comments.iter().position(|c| c["id"] == id) else {
                    return not_found("Comment");
                };
                self.comments.remove(index);
                (
                    200,
                    json!({ "data": { "commentDelete": { "success": true, "entityId": id } } }),
                )
            }
            "CommentCreate" => {
                let input = &vars["input"];
                let issue_id = input["issueId"].clone();
                if !self.issues.iter().any(|i| i["id"] == issue_id) {
                    return not_found("Issue");
                }
                let Some(body) = input["body"].as_str() else {
                    return not_found("Comment");
                };
                let me = self.users[0].clone();
                let mut comment = self.new_comment(&issue_id, body, &me);
                comment.as_object_mut().unwrap().remove("issueId");
                (
                    200,
                    json!({ "data": { "commentCreate": { "success": true, "comment": comment } } }),
                )
            }
            other => (
                400,
                json!({ "errors": [ { "message": format!("Unknown operation {other:?}"), "extensions": { "code": "GRAPHQL_VALIDATION_FAILED", "type": "graphql error", "userError": true } } ] }),
            ),
        }
    }
}

/// Put every field of `more` on `object`.
fn merge(object: &mut Value, more: Value) {
    if let (Some(have), Some(given)) = (object.as_object_mut(), more.as_object()) {
        for (k, v) in given {
            have.insert(k.clone(), v.clone());
        }
    }
}

fn priority_label(priority: u64) -> &'static str {
    match priority {
        1 => "Urgent",
        2 => "High",
        3 => "Medium",
        4 => "Low",
        _ => "No priority",
    }
}
