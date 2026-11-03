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
}

struct State {
    ws: Workspace,
    fail_next: BTreeMap<String, Failure>,
    operations: Vec<String>,
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
            .map(|i| st.ws.full_issue(i, 100, 50, 50))
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
        "null" => want.as_bool().is_some_and(|n| n == value.is_null()),
        _ => true,
    })
}

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
        };
        ws.organization = json!({ "id": ws.id(), "name": "Fake Workspace", "urlKey": "fake" });
        let me = json!({ "id": ws.id(), "name": "Fake Person", "displayName": "fake", "email": "fake@example.com", "active": true, "admin": true, "isMe": true });
        let other = json!({ "id": ws.id(), "name": "Other Person", "displayName": "other", "email": "other@example.com", "active": true, "admin": false, "isMe": false });
        ws.users = vec![me.clone(), other.clone()];
        for (name, color) in [
            ("Bug", "#eb5757"),
            ("Improvement", "#4ea7fc"),
            ("Feature", "#bb87fc"),
        ] {
            let label = json!({ "id": ws.id(), "name": name, "color": color, "description": null, "isGroup": false, "team": null, "parent": null });
            ws.labels.push(label);
        }
        let team = ws.new_team("APP", "Fake Team");
        ws.teams.push(team.clone());
        let team_label = json!({ "id": ws.id(), "name": "Release", "color": "#26b5ce", "description": "a team label", "isGroup": false, "team": { "id": team["id"], "key": "APP" }, "parent": null });
        ws.labels.push(team_label);
        let project = json!({ "id": ws.id(), "name": "Muster", "url": "https://linear.app/fake/project/muster", "description": "the fake project", "priority": 0, "startDate": null, "targetDate": null, "status": { "id": ws.id(), "name": "Planned", "type": "planned" }, "lead": { "id": me["id"], "name": me["name"], "displayName": me["displayName"] }, "teams": { "nodes": [ { "id": team["id"], "key": "APP" } ] } });
        ws.projects.push(project);
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
        let label_id = ws.labels[2]["id"].clone();
        let b = ws.new_issue(
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
            self.states.push(json!({ "id": sid, "name": state, "type": kind, "position": pos as f64, "color": "#bec2c8", "description": null, "team": { "id": id, "key": key } }));
        }
        json!({ "id": id, "key": key, "name": name, "description": null, "private": false })
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
            "labels": { "nodes": labels },
        })
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
        full["attachments"] =
            json!({ "nodes": [], "pageInfo": { "hasNextPage": attachments == 0 } });
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
                (
                    200,
                    json!({ "data": { "viewer": { "id": me["id"], "name": me["name"], "displayName": me["displayName"], "email": me["email"], "admin": me["admin"], "active": me["active"] }, "organization": self.organization } }),
                )
            }
            "Teams" => (
                200,
                json!({ "data": { "teams": Workspace::page(self.teams.clone(), first) } }),
            ),
            "WorkflowStates" => {
                let mut states: Vec<Value> = self
                    .states
                    .iter()
                    .filter(|s| passes(s, filter))
                    .cloned()
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
            "Projects" => (
                200,
                json!({ "data": { "projects": Workspace::page(self.projects.iter().filter(|p| passes(p, filter)).cloned().collect(), first) } }),
            ),
            "Users" => {
                let users: Vec<Value> = self.users.iter().filter(|u| passes(u, filter)).map(|u| json!({ "id": u["id"], "name": u["name"], "displayName": u["displayName"], "email": u["email"], "active": u["active"], "admin": u["admin"], "isMe": u["isMe"] })).collect();
                (
                    200,
                    json!({ "data": { "users": Workspace::page(users, first) } }),
                )
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
                let mut issues: Vec<Value> = self
                    .issues
                    .iter()
                    .filter(|i| passes(i, filter))
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
                let issue = self.new_issue(
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
                self.issues.push(issue.clone());
                (
                    200,
                    json!({ "data": { "issueCreate": { "success": true, "issue": issue } } }),
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

fn priority_label(priority: u64) -> &'static str {
    match priority {
        1 => "Urgent",
        2 => "High",
        3 => "Medium",
        4 => "Low",
        _ => "No priority",
    }
}
