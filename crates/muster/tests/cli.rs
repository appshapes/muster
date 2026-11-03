//! Every command through the binary against the fake Linear (ADR 0010), with stdin
//! closed and stdout a pipe (CLAUDE.md, gate 2): each stdout line is JSON, each failure
//! has its documented exit code and error object (gates 1 and 4), each bound is held
//! (gate 3). A test fails by panicking, so the test target allows what the crate denies.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};
use support::fake_linear::{Failure, Fake, KEY};

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Run {
    /// Every stdout line parsed as JSON; a line that is not JSON fails the test.
    fn lines(&self) -> Vec<Value> {
        self.stdout
            .lines()
            .map(|l| {
                serde_json::from_str(l)
                    .unwrap_or_else(|e| panic!("stdout line is not JSON ({e}): {l}"))
            })
            .collect()
    }
    fn one(&self) -> Value {
        let lines = self.lines();
        assert_eq!(
            lines.len(),
            1,
            "expected one line, got {} in {:?}",
            lines.len(),
            self.stdout
        );
        lines.into_iter().next().unwrap()
    }
    fn error(&self) -> Value {
        let last = self
            .stderr
            .lines()
            .last()
            .unwrap_or_else(|| panic!("no stderr; stdout was {:?}", self.stdout));
        let v: Value = serde_json::from_str(last)
            .unwrap_or_else(|e| panic!("stderr line is not JSON ({e}): {last}"));
        assert!(v.get("error").is_some(), "no error object in {last}");
        v["error"].clone()
    }
    fn notices(&self) -> Vec<Value> {
        self.stderr
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter(|v| v.get("notice").is_some())
            .map(|v| v["notice"].clone())
            .collect()
    }
}

static DIRS: AtomicUsize = AtomicUsize::new(0);

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "muster-test-{}-{}",
        std::process::id(),
        DIRS.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Run the binary with a clean environment: a temporary HOME with no key file, the
/// fake's key in LINEAR_API_KEY, the fake as the endpoint, stdin closed.
fn muster(fake: &Fake, args: &[&str]) -> Run {
    muster_with(
        fake,
        args,
        &[("LINEAR_API_KEY", KEY)],
        &temp_dir(),
        Stdio::null(),
    )
}

fn muster_with(fake: &Fake, args: &[&str], env: &[(&str, &str)], home: &Path, stdin: Stdio) -> Run {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_muster"));
    cmd.args(args)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", home)
        .env("MUSTER_ENDPOINT", fake.endpoint())
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let output = cmd.output().expect("the binary runs");
    Run {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    }
}

fn write(dir: &Path, name: &str, text: &str) -> String {
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path.display().to_string()
}

// ---- whoami and the key ------------------------------------------------------------------

#[test]
fn whoami_reports_the_key_source_the_endpoint_and_the_inferred_team() {
    let fake = Fake::start().unwrap();
    let run = muster(&fake, &["whoami"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let v = run.one();
    assert_eq!(v["viewer"]["email"], "fake@example.com");
    assert_eq!(v["organization"]["urlKey"], "fake");
    assert_eq!(v["muster"]["key"]["source"], "LINEAR_API_KEY");
    assert_eq!(v["muster"]["team"]["key"], "APP");
    assert_eq!(v["muster"]["team"]["source"], "inferred");
    let checks = v["muster"]["checks"].as_array().unwrap();
    let endpoint = checks.iter().find(|c| c["check"] == "endpoint").unwrap();
    assert_eq!(
        endpoint["level"], "warn",
        "the fake is not Linear's endpoint"
    );
    assert_eq!(fake.operations(), vec!["Viewer", "Teams"]);
}

#[test]
fn without_a_key_whoami_exits_3_and_names_the_sources() {
    let fake = Fake::start().unwrap();
    let run = muster_with(&fake, &["whoami"], &[], &temp_dir(), Stdio::null());
    assert_eq!(run.code, 3);
    assert_eq!(run.stdout, "");
    let e = run.error();
    assert_eq!(e["code"], "authentication");
    assert_eq!(e["exit"], 3);
    assert!(
        e["message"]
            .as_str()
            .unwrap()
            .contains("LINEAR_API_KEY is unset"),
        "{e}"
    );
    assert!(e["docs"].as_str().unwrap().ends_with("#the-key"));
    assert!(fake.operations().is_empty(), "no request without a key");
}

#[test]
fn the_key_file_is_read_from_home_and_from_muster_key_file() {
    let fake = Fake::start().unwrap();
    let home = temp_dir();
    std::fs::create_dir_all(home.join(".config/muster")).unwrap();
    std::fs::write(home.join(".config/muster/key"), format!("{KEY}\n")).unwrap();
    let run = muster_with(&fake, &["whoami"], &[], &home, Stdio::null());
    assert_eq!(run.code, 0, "{}", run.stderr);
    let v = run.one();
    assert_eq!(v["muster"]["key"]["source"], "file");
    assert!(
        v["muster"]["key"]["path"]
            .as_str()
            .unwrap()
            .ends_with(".config/muster/key")
    );

    let other = temp_dir();
    let path = write(&other, "linear.key", KEY);
    let run = muster_with(
        &fake,
        &["whoami"],
        &[("MUSTER_KEY_FILE", &path)],
        &temp_dir(),
        Stdio::null(),
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.one()["muster"]["key"]["path"], path);

    let run = muster_with(
        &fake,
        &["whoami"],
        &[("MUSTER_KEY_FILE", "/nonexistent/key")],
        &temp_dir(),
        Stdio::null(),
    );
    assert_eq!(run.code, 3);
    assert!(
        run.error()["message"]
            .as_str()
            .unwrap()
            .contains("MUSTER_KEY_FILE names /nonexistent/key")
    );

    let xdg = temp_dir();
    std::fs::create_dir_all(xdg.join("muster")).unwrap();
    std::fs::write(xdg.join("muster/key"), KEY).unwrap();
    let run = muster_with(
        &fake,
        &["whoami"],
        &[("XDG_CONFIG_HOME", xdg.to_str().unwrap())],
        &temp_dir(),
        Stdio::null(),
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
}

#[test]
fn a_wrong_key_exits_3_with_linears_error() {
    let fake = Fake::start().unwrap();
    let run = muster_with(
        &fake,
        &["whoami"],
        &[("LINEAR_API_KEY", "lin_api_wrong")],
        &temp_dir(),
        Stdio::null(),
    );
    assert_eq!(run.code, 3);
    let e = run.error();
    assert_eq!(e["linear"]["code"], "AUTHENTICATION_ERROR");
    assert_eq!(e["linear"]["requestId"], "fake-1");
}

// ---- lists and bounds --------------------------------------------------------------------

#[test]
fn the_plain_lists_print_one_object_per_line() {
    let fake = Fake::start().unwrap();
    let teams = muster(&fake, &["team", "list"]);
    assert_eq!(teams.code, 0, "{}", teams.stderr);
    assert_eq!(teams.lines().len(), 1);
    assert_eq!(teams.lines()[0]["key"], "APP");

    let states = muster(&fake, &["state", "list"]);
    assert_eq!(states.code, 0, "{}", states.stderr);
    let names: Vec<&str> = states
        .lines()
        .iter()
        .map(|s| s["name"].as_str().unwrap().to_string())
        .collect::<Vec<_>>()
        .leak()
        .iter()
        .map(String::as_str)
        .collect();
    assert_eq!(
        names,
        [
            "Backlog",
            "Todo",
            "In Progress",
            "In Review",
            "Done",
            "Canceled",
            "Duplicate"
        ]
    );

    let labels = muster(&fake, &["label", "list"]);
    assert_eq!(labels.code, 0, "{}", labels.stderr);
    assert_eq!(
        labels.lines().len(),
        4,
        "three workspace labels and one team label"
    );
    let projects = muster(&fake, &["project", "list"]);
    assert_eq!(projects.lines()[0]["name"], "Muster");
    let users = muster(&fake, &["user", "list"]);
    assert_eq!(users.lines().len(), 2);
}

#[test]
fn a_limit_over_the_cap_exits_2_and_a_cut_list_writes_a_notice() {
    let fake = Fake::start().unwrap();
    let run = muster(&fake, &["team", "list", "--limit", "251"]);
    assert_eq!(run.code, 2);
    let e = run.error();
    assert_eq!(e["code"], "usage");
    assert!(
        e["message"].as_str().unwrap().contains("PAGE_CAP (250)"),
        "{e}"
    );
    assert!(fake.operations().is_empty(), "refused before any request");

    fake.add_team("OPS", "Operations");
    let run = muster(&fake, &["team", "list", "--limit", "1"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.lines().len(), 1);
    let notices = run.notices();
    assert_eq!(notices.len(), 1, "{}", run.stderr);
    assert_eq!(notices[0]["code"], "truncated");
}

// ---- issue get and list ------------------------------------------------------------------

#[test]
fn issue_get_by_identifier_number_and_id_with_comments() {
    let fake = Fake::start().unwrap();
    let by_identifier = muster(&fake, &["issue", "get", "APP-1"]);
    assert_eq!(by_identifier.code, 0, "{}", by_identifier.stderr);
    let v = by_identifier.one();
    assert_eq!(v["identifier"], "APP-1");
    assert_eq!(v["state"]["name"], "Todo");
    assert_eq!(v["comments"]["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(v["comments"]["nodes"][0]["body"], "The first comment.");
    assert_eq!(v["attachments"]["nodes"].as_array().unwrap().len(), 0);
    let id = v["id"].as_str().unwrap().to_string();

    let by_number = muster(&fake, &["issue", "get", "1"]);
    assert_eq!(by_number.code, 0, "{}", by_number.stderr);
    assert_eq!(by_number.one()["id"], id);

    let by_id = muster(&fake, &["issue", "get", &id]);
    assert_eq!(by_id.code, 0, "{}", by_id.stderr);
    assert_eq!(by_id.one()["identifier"], "APP-1");

    let lower = muster(&fake, &["issue", "get", "app-1"]);
    assert_eq!(lower.code, 0, "{}", lower.stderr);

    let without = muster(&fake, &["issue", "get", "APP-1", "--no-comments"]);
    assert_eq!(without.code, 0, "{}", without.stderr);
    assert!(without.one().get("comments").is_none());
}

#[test]
fn an_unknown_issue_exits_4_and_a_bad_reference_exits_2() {
    let fake = Fake::start().unwrap();
    let run = muster(&fake, &["issue", "get", "APP-999"]);
    assert_eq!(run.code, 4);
    let e = run.error();
    assert_eq!(e["code"], "not_found");
    assert_eq!(e["linear"]["code"], "INPUT_ERROR");
    assert_eq!(e["linear"]["message"], "Entity not found: Issue");

    let run = muster(&fake, &["issue", "get", "999"]);
    assert_eq!(run.code, 4);
    assert!(
        run.error()["message"]
            .as_str()
            .unwrap()
            .contains("team APP has no issue number 999")
    );

    let run = muster(&fake, &["issue", "get", "Try muster"]);
    assert_eq!(run.code, 2);
    assert!(run.error()["hint"].as_str().unwrap().contains("--search"));
}

#[test]
fn issue_list_filters_by_state_label_assignee_and_title() {
    let fake = Fake::start().unwrap();
    let open = muster(&fake, &["issue", "list"]);
    assert_eq!(open.code, 0, "{}", open.stderr);
    let ids: Vec<String> = open
        .lines()
        .iter()
        .map(|i| i["identifier"].as_str().unwrap().to_string())
        .collect();
    assert!(
        ids.contains(&"APP-1".to_string())
            && ids.contains(&"APP-2".to_string())
            && !ids.contains(&"APP-3".to_string()),
        "{ids:?}"
    );

    let all = muster(&fake, &["issue", "list", "--all"]);
    assert_eq!(all.lines().len(), 3);
    let done = muster(&fake, &["issue", "list", "--state", "Done"]);
    assert_eq!(done.lines().len(), 1);
    assert_eq!(done.lines()[0]["identifier"], "APP-3");
    let two = muster(
        &fake,
        &["issue", "list", "--state", "Done", "--state", "Todo"],
    );
    assert_eq!(two.lines().len(), 2);
    let feature = muster(&fake, &["issue", "list", "--label", "Feature"]);
    assert_eq!(feature.lines().len(), 1);
    assert_eq!(feature.lines()[0]["identifier"], "APP-2");
    let other = muster(&fake, &["issue", "list", "--assignee", "other@example.com"]);
    assert_eq!(other.lines().len(), 1);
    let none = muster(&fake, &["issue", "list", "--assignee", "none"]);
    assert_eq!(none.lines().len(), 1);
    assert_eq!(none.lines()[0]["identifier"], "APP-1");
    let me = muster(&fake, &["issue", "list", "--assignee", "me"]);
    assert_eq!(me.lines().len(), 0);
    let search = muster(&fake, &["issue", "list", "--search", "connect"]);
    assert_eq!(search.lines().len(), 1);
    assert_eq!(search.lines()[0]["identifier"], "APP-2");
    let limited = muster(&fake, &["issue", "list", "--all", "--limit", "2"]);
    assert_eq!(limited.lines().len(), 2);
    assert_eq!(limited.notices()[0]["code"], "truncated");
}

// ---- create, comment, close, update ------------------------------------------------------

#[test]
fn issue_create_prints_linears_issue_and_the_fake_holds_it() {
    let fake = Fake::start().unwrap();
    let dir = temp_dir();
    let description = write(&dir, "body.md", "# Brief\n\nFrom a file.\n");
    let run = muster(
        &fake,
        &[
            "issue",
            "create",
            "--title",
            "Try muster",
            "--description-file",
            &description,
            "--label",
            "Bug",
            "--label",
            "Release",
            "--priority",
            "2",
            "--project",
            "Muster",
            "--assignee",
            "me",
            "--state",
            "In Progress",
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let v = run.one();
    assert_eq!(v["identifier"], "APP-4");
    assert_eq!(v["title"], "Try muster");
    assert_eq!(v["description"], "# Brief\n\nFrom a file.\n");
    assert_eq!(v["state"]["name"], "In Progress");
    assert_eq!(v["priority"], 2);
    assert_eq!(v["priorityLabel"], "High");
    assert_eq!(v["project"]["name"], "Muster");
    assert_eq!(v["assignee"]["email"], "fake@example.com");
    let labels: Vec<&str> = v["labels"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["name"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["Bug", "Release"]);
    assert_eq!(fake.issue("APP-4").unwrap()["title"], "Try muster");

    let plain = muster(&fake, &["issue", "create", "--title", "Plain"]);
    assert_eq!(plain.code, 0, "{}", plain.stderr);
    assert_eq!(
        plain.one()["state"]["name"],
        "Backlog",
        "the team's default state, as Linear's is"
    );
    assert!(plain.one()["description"].is_null());
}

#[test]
fn a_duplicate_title_exits_6_unless_allowed() {
    let fake = Fake::start().unwrap();
    let run = muster(
        &fake,
        &["issue", "create", "--title", "get FAMILIAR with linear"],
    );
    assert_eq!(run.code, 6);
    let e = run.error();
    assert_eq!(e["code"], "refused");
    assert!(e["message"].as_str().unwrap().contains("APP-1"), "{e}");
    assert_eq!(
        fake.operations()
            .iter()
            .filter(|o| *o == "IssueCreate")
            .count(),
        0
    );

    let done_title = muster(&fake, &["issue", "create", "--title", "An old thing"]);
    assert_eq!(
        done_title.code, 0,
        "a completed issue's title is free: {}",
        done_title.stderr
    );

    let allowed = muster(
        &fake,
        &[
            "issue",
            "create",
            "--title",
            "Get familiar with Linear",
            "--allow-duplicate",
        ],
    );
    assert_eq!(allowed.code, 0, "{}", allowed.stderr);
}

#[test]
fn unknown_names_exit_4_before_any_mutation() {
    let fake = Fake::start().unwrap();
    for args in [
        &["issue", "create", "--title", "x", "--label", "Nope"][..],
        &["issue", "create", "--title", "x", "--state", "Nope"],
        &["issue", "create", "--title", "x", "--project", "Nope"],
        &[
            "issue",
            "create",
            "--title",
            "x",
            "--assignee",
            "nobody@example.com",
        ],
    ] {
        let run = muster(&fake, args);
        assert_eq!(run.code, 4, "{args:?}: {}", run.stderr);
        assert_eq!(run.error()["code"], "not_found");
    }
    assert!(!fake.operations().iter().any(|o| o == "IssueCreate"));
}

#[test]
fn issue_comment_takes_a_body_a_file_or_stdin() {
    let fake = Fake::start().unwrap();
    let run = muster(
        &fake,
        &["issue", "comment", "APP-1", "--body", "From a flag."],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let v = run.one();
    assert_eq!(v["body"], "From a flag.");
    assert_eq!(v["issue"]["identifier"], "APP-1");
    assert_eq!(v["user"]["displayName"], "fake");

    let dir = temp_dir();
    let path = write(
        &dir,
        "comment.md",
        "From a file.\n\nSession stamp: appshapes/test, 2026-11-02\n",
    );
    let run = muster(&fake, &["issue", "comment", "1", "--body-file", &path]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(
        run.one()["body"]
            .as_str()
            .unwrap()
            .ends_with("2026-11-02\n")
    );

    let stdin = std::fs::File::open(&path).unwrap();
    let run = muster_with(
        &fake,
        &["issue", "comment", "APP-1", "--body-file", "-"],
        &[("LINEAR_API_KEY", KEY)],
        &temp_dir(),
        Stdio::from(stdin),
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        fake.comments("APP-1").len(),
        5,
        "two seeded and three added"
    );

    let none = muster(&fake, &["issue", "comment", "APP-1"]);
    assert_eq!(none.code, 2, "clap refuses a comment without a body");
}

#[test]
fn a_body_over_the_bound_exits_6_before_any_request() {
    let fake = Fake::start().unwrap();
    let dir = temp_dir();
    let big = "x".repeat(256 * 1024 + 1);
    let path = write(&dir, "big.md", &big);
    let run = muster(&fake, &["issue", "comment", "APP-1", "--body-file", &path]);
    assert_eq!(run.code, 6);
    let e = run.error();
    assert!(
        e["message"].as_str().unwrap().contains("BODY_MAX_BYTES"),
        "{e}"
    );
    assert!(e["docs"].as_str().unwrap().ends_with("#bounds"));
    assert!(fake.operations().is_empty());
}

#[test]
fn issue_close_comments_then_moves_to_done_canceled_or_a_named_state() {
    let fake = Fake::start().unwrap();
    let run = muster(
        &fake,
        &[
            "issue",
            "close",
            "APP-1",
            "--comment",
            "Done from the README.",
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let lines = run.lines();
    assert_eq!(lines.len(), 2, "the comment, then the issue");
    assert_eq!(lines[0]["body"], "Done from the README.");
    assert_eq!(lines[1]["identifier"], "APP-1");
    assert_eq!(lines[1]["state"]["type"], "completed");
    assert!(lines[1]["completedAt"].is_string());

    let run = muster(&fake, &["issue", "close", "APP-2", "--cancel"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.one()["state"]["name"], "Canceled");

    let run = muster(&fake, &["issue", "close", "APP-2", "--state", "Duplicate"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.one()["state"]["name"], "Duplicate");

    let run = muster(&fake, &["issue", "close", "APP-2", "--state", "Nope"]);
    assert_eq!(run.code, 4);
}

#[test]
fn issue_update_changes_what_it_is_told_and_nothing_else() {
    let fake = Fake::start().unwrap();
    let run = muster(
        &fake,
        &[
            "issue",
            "update",
            "APP-2",
            "--title",
            "Connect the tools",
            "--state",
            "In Progress",
            "--add-label",
            "Bug",
            "--remove-label",
            "Feature",
            "--priority",
            "1",
            "--assignee",
            "none",
            "--project",
            "Muster",
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let v = run.one();
    assert_eq!(v["title"], "Connect the tools");
    assert_eq!(v["state"]["name"], "In Progress");
    assert!(v["startedAt"].is_string());
    let labels: Vec<&str> = v["labels"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["name"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["Bug"]);
    assert_eq!(v["priority"], 1);
    assert!(v["assignee"].is_null());
    assert_eq!(v["project"]["name"], "Muster");
    assert_eq!(
        v["description"], "The second onboarding issue.",
        "untouched"
    );

    let nothing = muster(&fake, &["issue", "update", "APP-2"]);
    assert_eq!(nothing.code, 2);
    assert_eq!(nothing.error()["message"], "nothing to change");
}

// ---- failures: network, unknown, rate limit, server -------------------------------------

#[test]
fn a_mutation_whose_answer_never_came_exits_8_and_a_query_exits_7() {
    let fake = Fake::start().unwrap();
    fake.fail_next("IssueCreate", Failure::Drop);
    let run = muster(&fake, &["issue", "create", "--title", "Dropped"]);
    assert_eq!(run.code, 8, "{}", run.stderr);
    let e = run.error();
    assert_eq!(e["code"], "unknown");
    assert!(
        e["message"].as_str().unwrap().contains("may have landed"),
        "{e}"
    );
    assert!(e["hint"].as_str().unwrap().contains("issue get"), "{e}");

    fake.fail_next("Teams", Failure::Drop);
    let run = muster(&fake, &["team", "list"]);
    assert_eq!(run.code, 7, "{}", run.stderr);
    assert_eq!(run.error()["code"], "network");
}

#[test]
fn an_unreachable_endpoint_exits_7_without_a_request() {
    let fake = Fake::start().unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_muster"));
    let output = cmd
        .args(["issue", "create", "--title", "x"])
        .env_clear()
        .env("HOME", temp_dir())
        .env("LINEAR_API_KEY", KEY)
        .env("MUSTER_ENDPOINT", "http://127.0.0.1:1/graphql")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(7),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let e: Value = serde_json::from_str(
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .last()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(e["error"]["code"], "network");
    assert!(
        e["error"]["hint"]
            .as_str()
            .unwrap()
            .contains("nothing changed"),
        "{e}"
    );
    assert!(fake.operations().is_empty());
}

#[test]
fn a_rate_limit_exits_5_with_the_reset_time() {
    let fake = Fake::start().unwrap();
    fake.fail_next("Viewer", Failure::RateLimit);
    let run = muster(&fake, &["whoami"]);
    assert_eq!(run.code, 5, "{}", run.stderr);
    let e = run.error();
    assert_eq!(e["code"], "rate_limited");
    assert_eq!(e["linear"]["code"], "RATELIMITED");
    assert!(
        e["message"]
            .as_str()
            .unwrap()
            .contains("2026-11-03T01:42:22Z"),
        "{e}"
    );
}

#[test]
fn a_server_error_is_exit_1_on_a_query_and_exit_8_on_a_mutation() {
    let fake = Fake::start().unwrap();
    fake.fail_next("Teams", Failure::ServerError);
    let run = muster(&fake, &["team", "list"]);
    assert_eq!(run.code, 1, "{}", run.stderr);
    assert_eq!(run.error()["code"], "failed");
    assert_eq!(run.error()["linear"]["message"], "HTTP 502");

    fake.fail_next("CommentCreate", Failure::ServerError);
    let run = muster(&fake, &["issue", "comment", "APP-1", "--body", "x"]);
    assert_eq!(run.code, 8, "{}", run.stderr);
}

// ---- the team ----------------------------------------------------------------------------

#[test]
fn several_teams_need_a_name_from_the_flag_or_the_variable() {
    let fake = Fake::start().unwrap();
    fake.add_team("OPS", "Operations");
    let run = muster(&fake, &["issue", "list"]);
    assert_eq!(run.code, 2, "{}", run.stderr);
    let e = run.error();
    assert!(e["message"].as_str().unwrap().contains("APP, OPS"), "{e}");
    assert!(e["hint"].as_str().unwrap().contains("MUSTER_TEAM"), "{e}");

    let run = muster(&fake, &["--team", "ops", "issue", "list"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.lines().len(), 0);

    let run = muster_with(
        &fake,
        &["issue", "create", "--title", "In ops"],
        &[("LINEAR_API_KEY", KEY), ("MUSTER_TEAM", "OPS")],
        &temp_dir(),
        Stdio::null(),
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.one()["identifier"], "OPS-1");

    let run = muster_with(
        &fake,
        &["whoami"],
        &[("LINEAR_API_KEY", KEY), ("MUSTER_TEAM", "NOPE")],
        &temp_dir(),
        Stdio::null(),
    );
    assert_eq!(
        run.code, 4,
        "whoami reports the variable set wrong and exits 4: {}",
        run.stderr
    );
    let v = run.one();
    let team = v["muster"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["check"] == "team")
        .unwrap();
    assert_eq!(team["level"], "fail");

    let run = muster(&fake, &["whoami"]);
    assert_eq!(
        run.code, 0,
        "several teams and none named is a warning, not a failure: {}",
        run.stderr
    );
    let v = run.one();
    assert!(v["muster"]["team"].is_null());
}

// ---- the human format, the schema and the reference -------------------------------------

#[test]
fn human_renders_the_same_data_as_text() {
    let fake = Fake::start().unwrap();
    let run = muster(&fake, &["--human", "issue", "get", "APP-1"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(
        serde_json::from_str::<Value>(&run.stdout).is_err(),
        "text, not JSON"
    );
    assert!(run.stdout.contains("identifier: APP-1\n"), "{}", run.stdout);
    assert!(run.stdout.contains("  name: Todo\n"), "{}", run.stdout);
    assert!(
        run.stdout.contains("body: The first comment.\n"),
        "{}",
        run.stdout
    );

    let run = muster(&fake, &["--human", "issue", "get", "APP-999"]);
    assert_eq!(run.code, 4);
    assert!(run.stderr.contains("code: not_found\n"), "{}", run.stderr);
}

#[test]
fn schema_is_one_object_and_its_markdown_is_the_reference_page() {
    let fake = Fake::start().unwrap();
    let run = muster_with(&fake, &["schema"], &[], &temp_dir(), Stdio::null());
    assert_eq!(run.code, 0, "no key is needed: {}", run.stderr);
    let v = run.one();
    assert_eq!(v["name"], "muster");
    assert!(v["commands"].as_array().unwrap().len() >= 13);
    assert_eq!(v["exitCodes"].as_array().unwrap().len(), 8);
    let run = muster_with(
        &fake,
        &["schema", "--format", "markdown"],
        &[],
        &temp_dir(),
        Stdio::null(),
    );
    assert_eq!(run.code, 0);
    let reference = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/reference/commands.md"),
    )
    .expect("docs/reference/commands.md exists; run just gen-commands");
    assert_eq!(
        run.stdout, reference,
        "docs/reference/commands.md is stale: run just gen-commands"
    );
}

#[test]
fn every_exit_code_bound_and_variable_has_its_row_in_the_reference() {
    let reference = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/reference/commands.md"),
    )
    .unwrap();
    for code in muster::ErrorCode::ALL {
        let row = format!("| {} | `{}` |", code.exit(), code.name());
        assert!(reference.contains(&row), "no row {row}");
    }
    for bound in muster::bounds::ALL {
        assert!(
            reference.contains(&format!("| `{}` | {} |", bound.name, bound.value)),
            "no row for {}",
            bound.name
        );
    }
    for var in muster::config::ENVIRONMENT {
        assert!(
            reference.contains(&format!("| `{}` |", var.name)),
            "no row for {}",
            var.name
        );
    }
    assert!(reference.contains("## The key"));
}

// ---- the fixtures hold the fake to Linear (ADR 0010) ------------------------------------

fn post(endpoint: &str, body: &Value) -> Value {
    use std::io::{Read, Write};
    let addr = endpoint
        .trim_start_matches("http://")
        .trim_end_matches("/graphql");
    let mut stream = std::net::TcpStream::connect(addr).unwrap();
    let text = body.to_string();
    write!(stream, "POST /graphql HTTP/1.1\r\nHost: {addr}\r\nAuthorization: {KEY}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let (_, body) = response.split_once("\r\n\r\n").unwrap();
    serde_json::from_str(body).unwrap()
}

#[test]
fn the_fakes_answers_have_the_recorded_shapes() {
    let fake = Fake::start().unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let name = path.file_stem().unwrap().to_str().unwrap().to_string();
        let op = muster::linear::operations::by_name(&name)
            .unwrap_or_else(|| panic!("fixture {name} names no operation"));
        let fixture: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let variables = muster::cli::schema::fixture_variables(&name, Some("APP-1")).unwrap();
        let answer = post(
            &fake.endpoint(),
            &json!({ "query": op.document(), "operationName": op.name, "variables": variables }),
        );
        assert!(
            answer.get("errors").is_none(),
            "{name}: the fake answered an error: {answer}"
        );
        let mut mismatches = Vec::new();
        muster::cli::schema::shape_mismatches(&fixture, &answer["data"], "", &mut mismatches);
        assert!(
            mismatches.is_empty(),
            "{name}: the fake's answer differs from the recorded shape: {mismatches:?}"
        );
        checked += 1;
    }
    assert!(
        checked >= 8,
        "only {checked} fixtures; run just record-fixtures"
    );
}
