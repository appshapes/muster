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
        let (id, status_id) = (st.ws.id(), st.ws.id());
        st.ws.projects.push(json!({
            "id": id, "name": name, "url": format!("https://linear.app/fake/project/{}", name.to_ascii_lowercase()),
            "description": "", "priority": 0, "startDate": null, "targetDate": null,
            "status": { "id": status_id, "name": "Backlog", "type": "backlog" },
            "lead": null, "teams": { "nodes": teams },
        }));
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
        "gt" | "gte" | "lt" | "lte" => match (value.as_f64(), want.as_f64()) {
            (Some(v), Some(w)) => match op.as_str() {
                "gt" => v > w,
                "gte" => v >= w,
                "lt" => v < w,
                _ => v <= w,
            },
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
        let team = ws.new_team("APP", "Fake Team");
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
        ws.new_attachment(
            &a_id,
            "The first link",
            Some("seeded"),
            "https://example.com/first",
        );
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

    /// A project as `Projects` and `ProjectCreate` answer it: its `teams` paged at
    /// `first: $teams`, as Linear pages a nested connection.
    fn project_view(project: &Value, teams: usize) -> Value {
        let mut view = project.clone();
        let all: Vec<Value> = view["teams"]["nodes"]
            .as_array()
            .cloned()
            .unwrap_or_default();
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
                // key, as Linear's nested inputs do; a new URL key keeps the old one
                // among the last three previous keys, as Linear does.
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
            "Projects" => {
                let Some(teams) = vars["teams"].as_u64().map(|n| n as usize) else {
                    return missing_variable("teams", "Int!");
                };
                let projects: Vec<Value> = self
                    .projects
                    .iter()
                    .filter(|p| passes(p, filter))
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
                // them; answered as Linear answers `issues`, without them.
                let mut issues: Vec<Value> = self
                    .issues
                    .iter()
                    .filter(|i| passes(&self.filter_view(i), filter))
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
                let (id, status_id) = (self.id(), self.id());
                let project = json!({
                    "id": id, "name": name, "url": format!("https://linear.app/fake/project/{slug}"),
                    "description": input["description"].as_str().unwrap_or(""), "priority": 0,
                    "startDate": null, "targetDate": null,
                    "status": { "id": status_id, "name": "Backlog", "type": "backlog" },
                    "lead": null, "teams": { "nodes": teams },
                });
                let Some(teams) = vars["teams"].as_u64().map(|n| n as usize) else {
                    return missing_variable("teams", "Int!");
                };
                self.projects.push(project.clone());
                (
                    200,
                    json!({ "data": { "projectCreate": { "success": true, "project": Workspace::project_view(&project, teams) } } }),
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
