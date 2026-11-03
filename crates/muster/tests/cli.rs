//! Every command through the binary against the fake Linear (ADR 0010), with stdin
//! closed and stdout a pipe (CLAUDE.md, gate 2): each stdout line is JSON, each failure
//! has its documented exit code and error object (gates 1 and 4), each bound is held
//! (gate 3). A test fails by panicking, so the test target allows what the crate denies.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use muster::bounds::{MEMBERS_PER_PROJECT, MILESTONES_PER_PROJECT, PAGE_CAP};
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
    assert_eq!(
        v["comments"]["nodes"][0]["body"], "The second comment, with `code`.",
        "newest first, as Linear orders a connection"
    );
    assert_eq!(v["comments"]["nodes"][1]["body"], "The first comment.");
    assert_eq!(v["attachments"]["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(v["attachments"]["nodes"][0]["title"], "The first link");
    assert_eq!(v["children"]["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(v["children"]["nodes"][0]["identifier"], "APP-4");
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
    assert!(
        without.one().get("children").is_some(),
        "--no-comments leaves the comments and attachments out; the sub-issues stay"
    );
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
    assert_eq!(
        e["linear"]["userPresentableMessage"], "Could not find referenced Issue.",
        "Linear's presentable message, verbatim (ADR 0015)"
    );

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
    assert_eq!(all.lines().len(), 4);
    let done = muster(&fake, &["issue", "list", "--state", "Done"]);
    assert_eq!(done.lines().len(), 1);
    assert_eq!(done.lines()[0]["identifier"], "APP-3");
    let two = muster(
        &fake,
        &["issue", "list", "--state", "Done", "--state", "Todo"],
    );
    assert_eq!(
        two.lines().len(),
        3,
        "APP-1 and APP-4 in Todo, APP-3 in Done"
    );
    let feature = muster(&fake, &["issue", "list", "--label", "Feature"]);
    assert_eq!(feature.lines().len(), 1);
    assert_eq!(feature.lines()[0]["identifier"], "APP-2");
    let other = muster(&fake, &["issue", "list", "--assignee", "other@example.com"]);
    assert_eq!(other.lines().len(), 1);
    let none = muster(&fake, &["issue", "list", "--assignee", "none"]);
    assert_eq!(none.lines().len(), 2, "APP-1 and its sub-issue APP-4");
    assert!(none.lines().iter().all(|i| i["assignee"].is_null()));
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
    assert_eq!(v["identifier"], "APP-5");
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
    assert_eq!(fake.issue("APP-5").unwrap()["title"], "Try muster");

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

// ---- sub-issues (ADR 0013, feature 1) ---------------------------------------------------

#[test]
fn issue_get_prints_the_children_and_the_parent() {
    let fake = Fake::start().unwrap();
    let parent = muster(&fake, &["issue", "get", "APP-1"]);
    assert_eq!(parent.code, 0, "{}", parent.stderr);
    let v = parent.one();
    let children = v["children"]["nodes"].as_array().unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0]["identifier"], "APP-4");
    assert_eq!(children[0]["title"], "Read the first comment");
    assert_eq!(children[0]["state"]["name"], "Todo");
    assert_eq!(v["children"]["pageInfo"]["hasNextPage"], false);
    assert!(v["parent"].is_null());
    assert!(parent.notices().is_empty(), "{}", parent.stderr);

    let child = muster(&fake, &["issue", "get", "APP-4"]);
    assert_eq!(child.code, 0, "{}", child.stderr);
    assert_eq!(child.one()["parent"]["identifier"], "APP-1");
    assert!(
        child.one()["children"]["nodes"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn issue_create_with_parent_makes_a_sub_issue_and_list_finds_it() {
    let fake = Fake::start().unwrap();
    let run = muster(
        &fake,
        &["issue", "create", "--title", "A part", "--parent", "APP-2"],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let v = run.one();
    assert_eq!(v["parent"]["identifier"], "APP-2");
    assert_eq!(v["team"]["key"], "APP");
    let by_number = muster(
        &fake,
        &[
            "issue",
            "create",
            "--title",
            "Another part",
            "--parent",
            "2",
        ],
    );
    assert_eq!(by_number.code, 0, "{}", by_number.stderr);
    assert_eq!(by_number.one()["parent"]["identifier"], "APP-2");
    let id = fake.issue("APP-2").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let by_id = muster(
        &fake,
        &[
            "issue",
            "create",
            "--title",
            "A third part",
            "--parent",
            &id,
        ],
    );
    assert_eq!(by_id.code, 0, "{}", by_id.stderr);
    assert_eq!(by_id.one()["parent"]["identifier"], "APP-2");

    let unknown = muster(
        &fake,
        &["issue", "create", "--title", "x", "--parent", "APP-999"],
    );
    assert_eq!(unknown.code, 4, "{}", unknown.stderr);
    assert_eq!(
        fake.operations()
            .iter()
            .filter(|o| *o == "IssueCreate")
            .count(),
        3,
        "an unknown parent is refused before any mutation"
    );

    let children = muster(&fake, &["issue", "list", "--parent", "APP-2"]);
    assert_eq!(children.code, 0, "{}", children.stderr);
    assert_eq!(children.lines().len(), 3);
    assert!(
        children
            .lines()
            .iter()
            .all(|i| i["parent"]["identifier"] == "APP-2")
    );
    let top = muster(&fake, &["issue", "list", "--parent", "none", "--all"]);
    assert_eq!(top.code, 0, "{}", top.stderr);
    assert_eq!(
        top.lines().len(),
        3,
        "APP-1, APP-2 and APP-3; APP-4 and the three parts have parents"
    );
    let get = muster(&fake, &["issue", "get", "APP-2"]);
    assert_eq!(get.one()["children"]["nodes"].as_array().unwrap().len(), 3);
}

#[test]
fn issue_update_moves_an_issue_under_a_parent_and_out_again() {
    let fake = Fake::start().unwrap();
    let run = muster(&fake, &["issue", "update", "APP-2", "--parent", "APP-1"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.one()["parent"]["identifier"], "APP-1");
    assert_eq!(
        fake.issue("APP-1").unwrap()["children"]["nodes"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let out = muster(&fake, &["issue", "update", "APP-4", "--parent", "none"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.one()["parent"].is_null());
    let unknown = muster(&fake, &["issue", "update", "APP-2", "--parent", "APP-999"]);
    assert_eq!(unknown.code, 4, "{}", unknown.stderr);

    let updates = fake
        .operations()
        .iter()
        .filter(|o| *o == "IssueUpdate")
        .count();
    let itself = muster(&fake, &["issue", "update", "APP-2", "--parent", "2"]);
    assert_eq!(itself.code, 2, "{}", itself.stderr);
    let e = itself.error();
    assert_eq!(e["code"], "usage");
    assert!(
        e["message"]
            .as_str()
            .unwrap()
            .contains("APP-2 cannot be its own parent"),
        "{e}"
    );
    assert_eq!(
        fake.operations()
            .iter()
            .filter(|o| *o == "IssueUpdate")
            .count(),
        updates,
        "refused before any mutation"
    );
}

#[test]
fn fixtures_records_the_newest_issue_that_has_comments_attachments_and_sub_issues() {
    let fake = Fake::start().unwrap();
    let dir = temp_dir();
    let run = muster(&fake, &["fixtures", dir.to_str().unwrap()]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        run.lines().len(),
        muster::linear::operations::READS.len(),
        "one line per read operation"
    );
    assert!(
        run.notices().is_empty(),
        "APP-1 has comments, a link and a sub-issue, so the recording lacks nothing: {}",
        run.stderr
    );
    let fixture: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("IssueById.json")).unwrap())
            .unwrap();
    assert_eq!(fixture["issue"]["identifier"], "<string>");
    assert_eq!(
        fixture["issue"]["children"]["nodes"][0]["identifier"],
        "<string>"
    );
    assert_eq!(fixture["issue"]["comments"]["nodes"][0]["body"], "<string>");
    assert_eq!(
        fixture["issue"]["attachments"]["nodes"][0]["url"],
        "<string>"
    );
    let issues: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("Issues.json")).unwrap()).unwrap();
    assert_eq!(
        issues["issues"]["nodes"][0]["labels"]["nodes"][0]["name"], "<string>",
        "the Issues fixture is recorded on labelled issues"
    );
    let project: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("ProjectByName.json")).unwrap())
            .unwrap();
    let node = &project["projects"]["nodes"][0];
    assert_eq!(
        node["members"]["nodes"][0]["email"], "<string>",
        "the member node shape is recorded"
    );
    assert_eq!(
        node["projectMilestones"]["nodes"][0]["name"], "<string>",
        "the milestone node shape is recorded"
    );

    // The recorder prefers a project with a member and a milestone over a newer one
    // with neither, and says in a `partial` notice what the project it had to settle
    // for lacks (ADR 0014).
    let empty = muster(&fake, &["project", "create", "--name", "Empty"]);
    assert_eq!(empty.code, 0, "{}", empty.stderr);
    let again = muster(&fake, &["fixtures", temp_dir().to_str().unwrap()]);
    assert_eq!(again.code, 0, "{}", again.stderr);
    assert!(
        again.notices().is_empty(),
        "Muster has both, so Empty is passed over: {}",
        again.stderr
    );
    let fewer = muster(
        &fake,
        &["project", "update", "Muster", "--remove-member", "me"],
    );
    assert_eq!(fewer.code, 0, "{}", fewer.stderr);
    let lacking = muster(&fake, &["fixtures", temp_dir().to_str().unwrap()]);
    assert_eq!(lacking.code, 0, "{}", lacking.stderr);
    let notices = lacking.notices();
    assert_eq!(notices.len(), 1, "{}", lacking.stderr);
    assert_eq!(notices[0]["code"], "partial");
    let message = notices[0]["message"].as_str().unwrap();
    assert!(
        message.starts_with("ProjectByName: recorded on \"Muster\"")
            && message.contains("no member")
            && !message.contains("milestone"),
        "{message}"
    );
    assert!(
        notices[0]["hint"]
            .as_str()
            .unwrap()
            .contains("--add-member me"),
        "{}",
        lacking.stderr
    );
}

#[test]
fn a_sub_issue_takes_the_parents_team_unless_the_flag_or_the_variable_names_one() {
    let fake = Fake::start().unwrap();
    fake.add_team("OPS", "Operations");
    let ops = muster(
        &fake,
        &["--team", "OPS", "issue", "create", "--title", "In ops"],
    );
    assert_eq!(ops.code, 0, "{}", ops.stderr);
    assert_eq!(ops.one()["identifier"], "OPS-1");

    // Two teams and none named: a plain create exits 2, a sub-issue goes to its
    // parent's team (ADR 0013).
    let plain = muster(&fake, &["issue", "create", "--title", "Nowhere"]);
    assert_eq!(plain.code, 2, "{}", plain.stderr);
    let child = muster(
        &fake,
        &[
            "issue",
            "create",
            "--title",
            "Under ops",
            "--parent",
            "OPS-1",
        ],
    );
    assert_eq!(child.code, 0, "{}", child.stderr);
    assert_eq!(child.one()["identifier"], "OPS-2");
    assert_eq!(child.one()["parent"]["identifier"], "OPS-1");

    // MUSTER_TEAM wins over the parent's team, as --team does (ADR 0008, ADR 0013).
    let elsewhere = muster_with(
        &fake,
        &[
            "issue",
            "create",
            "--title",
            "Under ops, in app",
            "--parent",
            "OPS-1",
        ],
        &[("LINEAR_API_KEY", KEY), ("MUSTER_TEAM", "APP")],
        &temp_dir(),
        Stdio::null(),
    );
    assert_eq!(elsewhere.code, 0, "{}", elsewhere.stderr);
    assert_eq!(elsewhere.one()["team"]["key"], "APP");
    assert_eq!(elsewhere.one()["parent"]["identifier"], "OPS-1");
}

#[test]
fn a_parent_with_more_sub_issues_than_the_bound_writes_a_notice_and_exits_0() {
    let fake = Fake::start().unwrap();
    fake.add_sub_issues("APP-3", 51);
    let run = muster(&fake, &["issue", "get", "APP-3"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let v = run.one();
    assert_eq!(v["children"]["nodes"].as_array().unwrap().len(), 50);
    assert_eq!(
        v["children"]["nodes"][0]["title"], "Part 51",
        "newest first, as Linear orders a connection; the oldest are left out"
    );
    assert_eq!(v["children"]["pageInfo"]["hasNextPage"], true);
    let notices = run.notices();
    assert_eq!(notices.len(), 1, "{}", run.stderr);
    assert_eq!(notices[0]["code"], "truncated");
    assert!(
        notices[0]["hint"]
            .as_str()
            .unwrap()
            .contains("CHILDREN_PER_ISSUE"),
        "{}",
        run.stderr
    );

    let exact = Fake::start().unwrap();
    exact.add_sub_issues("APP-3", 50);
    let run = muster(&exact, &["issue", "get", "APP-3"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.one()["children"]["nodes"].as_array().unwrap().len(), 50);
    assert!(run.notices().is_empty(), "at the bound is not over it");
}

// ---- attachments (ADR 0013, feature 2) ----------------------------------------------------

#[test]
fn issue_attach_adds_a_link_that_issue_get_prints_newest_first() {
    let fake = Fake::start().unwrap();
    let run = muster(
        &fake,
        &[
            "issue",
            "attach",
            "APP-1",
            "--url",
            "https://github.com/appshapes/muster/pull/3",
            "--title",
            "Pull request 3",
            "--subtitle",
            "sub-issues",
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let v = run.one();
    assert_eq!(v["url"], "https://github.com/appshapes/muster/pull/3");
    assert_eq!(v["title"], "Pull request 3");
    assert_eq!(v["subtitle"], "sub-issues");
    assert_eq!(v["issue"]["identifier"], "APP-1");
    assert!(v["id"].is_string() && v["createdAt"].is_string());
    assert_eq!(
        fake.operations(),
        vec!["Issues", "AttachmentCreate"],
        "one request resolves the identifier, one creates the link"
    );

    let by_number = muster(
        &fake,
        &[
            "issue",
            "attach",
            "1",
            "--url",
            "https://example.com/second",
            "--title",
            "Second",
        ],
    );
    assert_eq!(by_number.code, 0, "{}", by_number.stderr);
    assert!(by_number.one()["subtitle"].is_null());

    let get = muster(&fake, &["issue", "get", "APP-1"]);
    let links = get.one()["attachments"]["nodes"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(links.len(), 3, "the seeded link and the two added");
    assert_eq!(
        links[0]["title"], "Second",
        "newest first, as Linear orders them"
    );
    assert_eq!(
        links[1]["url"],
        "https://github.com/appshapes/muster/pull/3"
    );
    assert_eq!(links[2]["title"], "The first link");

    let unknown = muster(
        &fake,
        &[
            "issue",
            "attach",
            "APP-999",
            "--url",
            "https://example.com",
            "--title",
            "x",
        ],
    );
    assert_eq!(unknown.code, 4, "{}", unknown.stderr);
    assert_eq!(
        fake.operations()
            .iter()
            .filter(|o| *o == "AttachmentCreate")
            .count(),
        2,
        "an unknown issue is refused before any mutation"
    );
    let untitled = muster(
        &fake,
        &["issue", "attach", "APP-1", "--url", "https://example.com"],
    );
    assert_eq!(untitled.code, 2, "clap refuses a link without a title");
    for args in [
        &["issue", "attach", "APP-1", "--url", "", "--title", "x"][..],
        &[
            "issue",
            "attach",
            "APP-1",
            "--url",
            "https://example.com",
            "--title",
            " ",
        ],
    ] {
        let empty = muster(&fake, args);
        assert_eq!(empty.code, 2, "{args:?}: {}", empty.stderr);
        assert!(
            empty.error()["message"]
                .as_str()
                .unwrap()
                .contains("must not be empty")
        );
    }
    assert_eq!(
        fake.operations()
            .iter()
            .filter(|o| *o == "AttachmentCreate")
            .count(),
        2,
        "an empty value is refused before any request"
    );
    // An oversized --url, --title or --subtitle is not sent through the binary: Linux
    // refuses one argv string over 128 KiB (MAX_ARG_STRLEN) before muster runs, and
    // BODY_MAX_BYTES is 256 KiB, so the run would fail here and pass on macOS. The
    // bound on a flag's value is held in process by the unit tests in src/cli/run.rs,
    // on every OS; a body from a file reaches it through the binary in
    // a_body_over_the_bound_exits_6_before_any_request.

    // A URL the issue already has is updated in place, not added again (Linear's own
    // behaviour, read on 2026-11-03): the same id comes back with the new title, and
    // a subtitle left out stays as it was.
    let again = muster(
        &fake,
        &[
            "issue",
            "attach",
            "APP-1",
            "--url",
            "https://github.com/appshapes/muster/pull/3",
            "--title",
            "Renamed",
        ],
    );
    assert_eq!(again.code, 0, "{}", again.stderr);
    assert_eq!(again.one()["id"], links[1]["id"]);
    assert_eq!(again.one()["title"], "Renamed");
    assert_eq!(
        again.one()["subtitle"],
        "sub-issues",
        "kept, as Linear keeps it"
    );
    let get = muster(&fake, &["issue", "get", "APP-1"]);
    let links = get.one()["attachments"]["nodes"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(links.len(), 3, "still three");
    assert_eq!(links[1]["title"], "Renamed");
}

#[test]
fn an_issue_with_more_attachments_than_the_bound_writes_a_notice_and_exits_0() {
    let fake = Fake::start().unwrap();
    fake.add_attachments("APP-2", 51);
    let run = muster(&fake, &["issue", "get", "APP-2"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let v = run.one();
    assert_eq!(v["attachments"]["nodes"].as_array().unwrap().len(), 50);
    assert_eq!(v["attachments"]["nodes"][0]["title"], "Link 51");
    assert_eq!(v["attachments"]["pageInfo"]["hasNextPage"], true);
    let notices = run.notices();
    assert_eq!(notices.len(), 1, "{}", run.stderr);
    assert_eq!(notices[0]["code"], "truncated");
    assert!(
        notices[0]["hint"]
            .as_str()
            .unwrap()
            .contains("ATTACHMENTS_PER_ISSUE"),
        "{}",
        run.stderr
    );
}

// ---- projects (ADR 0013, feature 3) -------------------------------------------------------

#[test]
fn project_create_makes_a_project_for_the_team_and_refuses_a_duplicate_name() {
    let fake = Fake::start().unwrap();
    let run = muster(
        &fake,
        &[
            "project",
            "create",
            "--name",
            "Smoke tests",
            "--description",
            "Where the smoke issues go.",
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let v = run.one();
    assert_eq!(v["name"], "Smoke tests");
    assert_eq!(v["description"], "Where the smoke issues go.");
    assert_eq!(v["teams"]["nodes"][0]["key"], "APP");
    assert!(v["id"].is_string() && v["url"].is_string() && v["status"]["name"].is_string());
    let projects = muster(&fake, &["project", "list"]);
    assert_eq!(projects.lines().len(), 2, "the seeded one and the new one");

    let again = muster(&fake, &["project", "create", "--name", "smoke TESTS"]);
    assert_eq!(again.code, 6, "{}", again.stderr);
    let e = again.error();
    assert_eq!(e["code"], "refused");
    assert!(
        e["message"].as_str().unwrap().contains("Smoke tests"),
        "{e}"
    );
    assert_eq!(
        fake.operations()
            .iter()
            .filter(|o| *o == "ProjectCreate")
            .count(),
        1,
        "refused before any mutation"
    );
    let allowed = muster(
        &fake,
        &[
            "project",
            "create",
            "--name",
            "Smoke tests",
            "--allow-duplicate",
        ],
    );
    assert_eq!(allowed.code, 0, "{}", allowed.stderr);

    fake.add_team("OPS", "Operations");
    let nowhere = muster(&fake, &["project", "create", "--name", "Lost"]);
    assert_eq!(
        nowhere.code, 2,
        "two teams and none named: {}",
        nowhere.stderr
    );
    let ops = muster(
        &fake,
        &["--team", "OPS", "project", "create", "--name", "Ops work"],
    );
    assert_eq!(ops.code, 0, "{}", ops.stderr);
    assert_eq!(ops.one()["teams"]["nodes"][0]["key"], "OPS");
}

#[test]
fn issue_list_filters_by_project_and_by_no_project() {
    let fake = Fake::start().unwrap();
    let run = muster(
        &fake,
        &[
            "issue",
            "create",
            "--title",
            "In the project",
            "--project",
            "Muster",
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let in_project = muster(&fake, &["issue", "list", "--project", "muster"]);
    assert_eq!(in_project.code, 0, "{}", in_project.stderr);
    assert_eq!(in_project.lines().len(), 1);
    assert_eq!(in_project.lines()[0]["title"], "In the project");
    let none = muster(&fake, &["issue", "list", "--project", "none", "--all"]);
    assert_eq!(none.code, 0, "{}", none.stderr);
    assert!(none.lines().iter().all(|i| i["project"].is_null()));
    assert_eq!(none.lines().len(), 4, "the four seeded issues");
    let nope = muster(&fake, &["issue", "list", "--project", "Nope"]);
    assert_eq!(
        nope.code, 0,
        "an unknown project is an empty list, as Linear filters it"
    );
    assert_eq!(nope.lines().len(), 0);
    assert_eq!(
        fake.operations()
            .iter()
            .filter(|o| *o == "Projects")
            .count(),
        1,
        "only the create resolved a project name; the lists filter by name in the one request"
    );
}

#[test]
fn a_project_in_more_teams_than_the_bound_prints_the_bound_and_a_notice() {
    let fake = Fake::start().unwrap();
    fake.add_project_across_teams("Everywhere", 11);
    let run = muster(&fake, &["project", "list"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let everywhere = run
        .lines()
        .into_iter()
        .find(|p| p["name"] == "Everywhere")
        .unwrap();
    assert_eq!(everywhere["teams"]["nodes"].as_array().unwrap().len(), 10);
    assert_eq!(everywhere["teams"]["pageInfo"]["hasNextPage"], true);
    let notices = run.notices();
    assert_eq!(notices.len(), 1, "{}", run.stderr);
    assert_eq!(notices[0]["code"], "truncated");
    assert!(
        notices[0]["message"]
            .as_str()
            .unwrap()
            .contains("Everywhere")
            && notices[0]["hint"]
                .as_str()
                .unwrap()
                .contains("TEAMS_PER_PROJECT"),
        "{}",
        run.stderr
    );

    let exact = Fake::start().unwrap();
    exact.add_project_across_teams("Ten", 10);
    let run = muster(&exact, &["project", "list"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(run.notices().is_empty(), "at the bound is not over it");
}

// ---- the workspace (ADR 0014, group 1) ---------------------------------------------------

#[test]
fn workspace_get_prints_the_organization_as_linear_returns_it() {
    let fake = Fake::start().unwrap();
    let run = muster(&fake, &["workspace", "get"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let v = run.one();
    assert_eq!(v["urlKey"], "fake");
    assert_eq!(v["name"], "Fake Workspace");
    assert!(v["userCount"].is_number() && v["createdIssueCount"].is_number());
    assert!(v["securitySettings"].is_object() && v["authSettings"].is_object());
    assert!(
        v["subscription"].is_null(),
        "a Free workspace has no subscription"
    );
    assert!(v["previousUrlKeys"].is_array());
    assert_eq!(v.get("muster"), None, "nothing muster made up");
    assert_eq!(
        fake.operations(),
        vec!["Workspace"],
        "one request, and no team is resolved"
    );

    fake.fail_next("Workspace", Failure::Drop);
    let run = muster(&fake, &["workspace", "get"]);
    assert_eq!(
        run.code, 7,
        "a query whose answer never came: {}",
        run.stderr
    );
    assert_eq!(run.error()["code"], "network");
}

// ---- teams (ADR 0014, group 2) -----------------------------------------------------------

#[test]
fn team_get_prints_every_setting_by_key_or_by_the_team_rule() {
    let fake = Fake::start().unwrap();
    let run = muster(&fake, &["team", "get", "app"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let v = run.one();
    assert_eq!(v["key"], "APP");
    assert_eq!(v["cyclesEnabled"], false);
    assert_eq!(v["visibility"], "public");
    assert!(v["timezone"].is_string());
    assert_eq!(v["defaultIssueState"]["name"], "Backlog");
    assert_eq!(v["issueCount"], 4, "the four seeded issues");
    assert!(v["securitySettings"].is_object() && v["retiredAt"].is_null());
    assert_eq!(
        v.get("private"),
        None,
        "deprecated on Team, so not asked for"
    );
    assert_eq!(fake.operations(), vec!["TeamByKey"], "one request by key");

    let inferred = muster(&fake, &["team", "get"]);
    assert_eq!(inferred.code, 0, "{}", inferred.stderr);
    assert_eq!(inferred.one()["key"], "APP");
    let listed = muster(&fake, &["team", "list"]).one();
    assert_eq!(listed["cyclesEnabled"], false);
    assert!(listed["retiredAt"].is_null() && listed["timezone"].is_string());

    let nope = muster(&fake, &["team", "get", "NOPE"]);
    assert_eq!(nope.code, 4, "{}", nope.stderr);
    assert_eq!(nope.error()["code"], "not_found");
    fake.fail_next("TeamByKey", Failure::Drop);
    let run = muster(&fake, &["team", "get", "APP"]);
    assert_eq!(run.code, 7, "{}", run.stderr);
}

#[test]
fn team_create_makes_a_team_with_its_states_and_refuses_a_duplicate_name() {
    let fake = Fake::start().unwrap();
    let run = muster(
        &fake,
        &[
            "team",
            "create",
            "--name",
            "Smoke tests",
            "--key",
            "SMK",
            "--description",
            "Where the smoke test's cycles live.",
            "--timezone",
            "Europe/London",
            "--cycles-enabled",
            "true",
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let v = run.one();
    assert_eq!(v["key"], "SMK");
    assert_eq!(v["name"], "Smoke tests");
    assert_eq!(v["timezone"], "Europe/London");
    assert_eq!(v["cyclesEnabled"], true);
    assert_eq!(v["issueCount"], 0);
    assert_eq!(
        v["defaultIssueState"]["name"], "Backlog",
        "Linear's default states"
    );
    let states = muster(&fake, &["--team", "SMK", "state", "list"]);
    assert_eq!(states.code, 0, "{}", states.stderr);
    assert_eq!(states.lines().len(), 7, "the default workflow");
    let issue = muster(
        &fake,
        &["--team", "SMK", "issue", "create", "--title", "First"],
    );
    assert_eq!(issue.code, 0, "{}", issue.stderr);
    assert_eq!(issue.one()["identifier"], "SMK-1");

    let again = muster(&fake, &["team", "create", "--name", "smoke TESTS"]);
    assert_eq!(again.code, 6, "{}", again.stderr);
    let e = again.error();
    assert_eq!(e["code"], "refused");
    assert!(e["message"].as_str().unwrap().contains("SMK"), "{e}");
    assert_eq!(
        fake.operations()
            .iter()
            .filter(|o| *o == "TeamCreate")
            .count(),
        1,
        "refused before any mutation"
    );
    let same_key = muster(
        &fake,
        &[
            "team",
            "create",
            "--name",
            "Another",
            "--key",
            "smk",
            "--allow-duplicate",
        ],
    );
    assert_eq!(
        same_key.code, 6,
        "Linear keeps the key unique: {}",
        same_key.stderr
    );
    // Both of Linear's messages verbatim, each under Linear's name, and muster's own
    // message naming the reason (ADR 0015).
    let e = same_key.error();
    assert_eq!(
        e["linear"]["message"], "Argument Validation Error",
        "Linear's message as Linear sent it: {}",
        same_key.stderr
    );
    assert!(
        e["linear"]["userPresentableMessage"]
            .as_str()
            .unwrap()
            .contains("already in use"),
        "Linear's presentable reason under its own name: {}",
        same_key.stderr
    );
    assert!(
        e["message"].as_str().unwrap().contains("already in use"),
        "muster's message names the reason: {}",
        same_key.stderr
    );

    let child = muster(
        &fake,
        &["team", "create", "--name", "Smoke child", "--parent", "SMK"],
    );
    assert_eq!(child.code, 0, "{}", child.stderr);
    assert_eq!(child.one()["parent"]["key"], "SMK");
    assert_eq!(child.one()["key"], "SMO", "a key made from the name");
    let copied = muster(
        &fake,
        &[
            "team",
            "create",
            "--name",
            "Copied",
            "--copy-settings-from",
            "NOPE",
        ],
    );
    assert_eq!(copied.code, 4, "{}", copied.stderr);
    let blank = muster(&fake, &["team", "create", "--name", "Blank", "--key", " "]);
    assert_eq!(blank.code, 2, "{}", blank.stderr);

    fake.fail_next("TeamCreate", Failure::Drop);
    let run = muster(&fake, &["team", "create", "--name", "Dropped"]);
    assert_eq!(run.code, 8, "{}", run.stderr);
    assert_eq!(run.error()["code"], "unknown");
}

#[test]
fn team_update_changes_only_what_is_passed_and_resolves_names() {
    let fake = Fake::start().unwrap();
    fake.add_team("SMK", "Smoke tests");
    let run = muster(
        &fake,
        &[
            "team",
            "update",
            "SMK",
            "--description",
            "The smoke team.",
            "--private",
            "true",
            "--cycles-enabled",
            "true",
            "--cycle-enabled-start-date",
            "2026-11-10T00:00:00Z",
            "--cycle-start-day",
            "1",
            "--cycle-duration",
            "1",
            "--default-issue-state",
            "Todo",
            "--auto-close-state",
            "Canceled",
            "--auto-close-period",
            "none",
            "--issue-estimation-type",
            "fibonacci",
            "--parent",
            "APP",
            "--team-management-role",
            "owner",
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let v = run.one();
    assert_eq!(v["key"], "SMK");
    assert_eq!(v["description"], "The smoke team.");
    assert_eq!(
        v["visibility"], "private",
        "Linear's current field, not the deprecated one"
    );
    assert_eq!(v["cyclesEnabled"], true);
    assert_eq!(v["cycleStartDay"], 1);
    assert_eq!(v["cycleDuration"], 1);
    assert_eq!(
        v["cycleCooldownTime"], 0,
        "a flag left out leaves its setting alone"
    );
    assert_eq!(
        v["defaultIssueState"]["name"], "Todo",
        "resolved in the team"
    );
    assert!(v["autoCloseStateId"].is_string());
    assert!(v["autoClosePeriod"].is_null());
    assert_eq!(v["issueEstimationType"], "fibonacci");
    assert_eq!(v["parent"]["key"], "APP");
    assert_eq!(v["securitySettings"]["teamManagement"], "owner");
    assert_eq!(v["timezone"], "Etc/UTC", "untouched");
    let back = muster(&fake, &["team", "get", "SMK"]).one();
    assert_eq!(back["cyclesEnabled"], true);
    assert_eq!(back["parent"]["key"], "APP");

    let orphan = muster(&fake, &["team", "update", "SMK", "--parent", "none"]);
    assert_eq!(orphan.code, 0, "{}", orphan.stderr);
    assert!(orphan.one()["parent"].is_null());
    let lowercase = muster(
        &fake,
        &["team", "update", "smk", "--default-issue-state", "Todo"],
    );
    assert_eq!(
        lowercase.code, 0,
        "the states are filtered by the key Linear returned, not the key as typed: {}",
        lowercase.stderr
    );
    assert_eq!(lowercase.one()["key"], "SMK");
    assert_eq!(lowercase.one()["defaultIssueState"]["name"], "Todo");
    let inferred = muster_with(
        &fake,
        &["team", "update", "--timezone", "Europe/London"],
        &[("LINEAR_API_KEY", KEY), ("MUSTER_TEAM", "SMK")],
        &temp_dir(),
        Stdio::null(),
    );
    assert_eq!(inferred.code, 0, "{}", inferred.stderr);
    assert_eq!(inferred.one()["key"], "SMK");
    assert_eq!(inferred.one()["timezone"], "Europe/London");

    let before = fake
        .operations()
        .iter()
        .filter(|o| *o == "TeamUpdate")
        .count();
    let nothing = muster(&fake, &["team", "update", "SMK"]);
    assert_eq!(nothing.code, 2, "{}", nothing.stderr);
    assert_eq!(nothing.error()["message"], "nothing to change");
    let no_state = muster(
        &fake,
        &["team", "update", "SMK", "--default-issue-state", "Nope"],
    );
    assert_eq!(no_state.code, 4, "{}", no_state.stderr);
    let itself = muster(&fake, &["team", "update", "SMK", "--parent", "smk"]);
    assert_eq!(itself.code, 2, "{}", itself.stderr);
    let sunday_plus = muster(&fake, &["team", "update", "SMK", "--cycle-start-day", "7"]);
    assert_eq!(sunday_plus.code, 2, "{}", sunday_plus.stderr);
    let maybe = muster(&fake, &["team", "update", "SMK", "--private", "maybe"]);
    assert_eq!(maybe.code, 2, "{}", maybe.stderr);
    assert_eq!(
        fake.operations()
            .iter()
            .filter(|o| *o == "TeamUpdate")
            .count(),
        before,
        "the usage errors and the unknown state sent no mutation"
    );
    fake.fail_next("TeamUpdate", Failure::Drop);
    let run = muster(&fake, &["team", "update", "SMK", "--name", "Dropped"]);
    assert_eq!(run.code, 8, "{}", run.stderr);
}

#[test]
fn team_retire_and_unretire_set_and_clear_retired_at() {
    let fake = Fake::start().unwrap();
    fake.add_team("SMK", "Smoke tests");
    let retired = muster(&fake, &["team", "retire", "SMK"]);
    assert_eq!(retired.code, 0, "{}", retired.stderr);
    assert!(retired.one()["retiredAt"].is_string(), "{}", retired.stdout);
    let listed = muster(&fake, &["team", "list"]);
    let smk = listed
        .lines()
        .into_iter()
        .find(|t| t["key"] == "SMK")
        .unwrap();
    assert!(
        smk["retiredAt"].is_string(),
        "a retired team is still listed"
    );
    let back = muster(&fake, &["team", "unretire", "SMK"]);
    assert_eq!(back.code, 0, "{}", back.stderr);
    assert!(back.one()["retiredAt"].is_null());
    assert_eq!(
        fake.operations(),
        vec![
            "TeamByKey",
            "TeamUpdate",
            "Teams",
            "TeamByKey",
            "TeamUpdate"
        ]
    );
    fake.fail_next("TeamUpdate", Failure::Drop);
    let run = muster(&fake, &["team", "retire", "SMK"]);
    assert_eq!(run.code, 8, "{}", run.stderr);
}

#[test]
fn team_delete_archives_the_team_and_unarchive_brings_it_back() {
    let fake = Fake::start().unwrap();
    fake.add_team("SMK", "Smoke tests");
    let gone = muster(&fake, &["team", "delete", "SMK"]);
    assert_eq!(gone.code, 0, "{}", gone.stderr);
    let v = gone.one();
    assert_eq!(v["success"], true);
    assert!(
        v["entityId"].is_string(),
        "Linear's delete payload as it is"
    );
    let listed = muster(&fake, &["team", "list"]);
    assert!(
        listed.lines().iter().all(|t| t["key"] != "SMK"),
        "an archived team is not listed"
    );
    let get = muster(&fake, &["team", "get", "SMK"]);
    assert_eq!(get.code, 4, "{}", get.stderr);
    let back = muster(&fake, &["team", "unarchive", "SMK"]);
    assert_eq!(back.code, 0, "{}", back.stderr);
    assert_eq!(back.one()["key"], "SMK");
    assert!(back.one()["archivedAt"].is_null());
    let listed = muster(&fake, &["team", "list"]);
    assert!(listed.lines().iter().any(|t| t["key"] == "SMK"));
    fake.fail_next("TeamDelete", Failure::Drop);
    let run = muster(&fake, &["team", "delete", "SMK"]);
    assert_eq!(run.code, 8, "{}", run.stderr);
    fake.fail_next("TeamUnarchive", Failure::Drop);
    let run = muster(&fake, &["team", "unarchive", "SMK"]);
    assert_eq!(run.code, 8, "{}", run.stderr);
}

// ---- users (ADR 0014, group 4) -----------------------------------------------------------

#[test]
fn user_get_update_role_suspension_and_sessions() {
    let fake = Fake::start().unwrap();
    let me = muster(&fake, &["user", "get", "me"]);
    assert_eq!(me.code, 0, "{}", me.stderr);
    let v = me.one();
    assert_eq!(v["email"], "fake@example.com");
    assert_eq!(v["admin"], true);
    assert_eq!(v["owner"], false);
    assert_eq!(v["guest"], false);
    assert_eq!(v["app"], false);
    assert_eq!(v["isMe"], true);
    assert!(v["timezone"].is_string() && v["lastSeen"].is_string() && v["url"].is_string());
    assert!(v["statusLabel"].is_null() && v["disableReason"].is_null());
    assert_eq!(
        fake.operations(),
        vec!["Viewer", "UserById"],
        "`me` is the viewer"
    );
    let by_email = muster(&fake, &["user", "get", "other@example.com"]).one();
    assert_eq!(by_email["displayName"], "other");
    let by_name = muster(&fake, &["user", "get", "Other Person"]).one();
    assert_eq!(by_name["email"], "other@example.com");
    let nobody = muster(&fake, &["user", "get", "nobody"]);
    assert_eq!(nobody.code, 4, "{}", nobody.stderr);

    let updated = muster(
        &fake,
        &[
            "user",
            "update",
            "me",
            "--title",
            "Owner",
            "--status-emoji",
            "🚀",
            "--status-label",
            "Shipping",
            "--status-until",
            "2026-11-10T09:00:00Z",
        ],
    );
    assert_eq!(updated.code, 0, "{}", updated.stderr);
    let u = updated.one();
    assert_eq!(u["title"], "Owner");
    assert_eq!(u["statusLabel"], "Shipping");
    assert_eq!(u["statusUntilAt"], "2026-11-10T09:00:00Z");
    assert_eq!(
        u["name"], "Fake Person",
        "a flag left out leaves its field alone"
    );
    let kept = muster(&fake, &["user", "update", "me", "--status-until", "none"]).one();
    assert!(kept["statusUntilAt"].is_null(), "`none` keeps the status");
    let nothing = muster(&fake, &["user", "update", "me"]);
    assert_eq!(nothing.code, 2, "{}", nothing.stderr);

    let promoted = muster(&fake, &["user", "change-role", "other", "--role", "admin"]);
    assert_eq!(promoted.code, 0, "{}", promoted.stderr);
    let lines = promoted.lines();
    assert_eq!(lines.len(), 2, "Linear's success, then the user read back");
    assert_eq!(lines[0]["success"], true);
    assert_eq!(lines[1]["admin"], true);
    assert_eq!(lines[1]["displayName"], "other");
    let guest = muster(&fake, &["user", "change-role", "other", "--role", "guest"]).lines();
    assert_eq!(guest[1]["guest"], true);
    assert_eq!(guest[1]["admin"], false);
    let bad = muster(&fake, &["user", "change-role", "other", "--role", "app"]);
    assert_eq!(
        bad.code, 2,
        "`app` is not a role a person takes: {}",
        bad.stderr
    );

    // A read-back that fails after the mutation landed: Linear's `success` is on
    // stdout, the exit code is the read-back's, and the error says the mutation
    // landed and names `user get`, never "nothing changed" (invariant 4).
    fake.fail_next("UserById", Failure::Drop);
    let landed = muster(&fake, &["user", "suspend", "other"]);
    assert_eq!(landed.code, 7, "{}", landed.stderr);
    let landed_lines = landed.lines();
    assert_eq!(
        landed_lines.len(),
        1,
        "Linear's success and no user: {}",
        landed.stdout
    );
    assert_eq!(landed_lines[0]["success"], true);
    let error = landed.error();
    assert_eq!(error["code"], "network");
    let message = error["message"].as_str().unwrap_or("");
    assert!(
        message.contains("userSuspend succeeded in Linear") && !message.contains("nothing changed"),
        "{message}"
    );
    let hint = error["hint"].as_str().unwrap_or("");
    assert!(hint.contains("muster user get \"other\""), "{hint}");
    let everyone = muster(&fake, &["user", "list", "--all"]);
    assert!(
        everyone
            .lines()
            .iter()
            .any(|u| u["displayName"] == "other" && u["active"] == false),
        "the suspension landed: {}",
        everyone.stdout
    );
    let restored = muster(&fake, &["user", "unsuspend", "other"]);
    assert_eq!(restored.code, 0, "{}", restored.stderr);

    let suspended = muster(&fake, &["user", "suspend", "other"]);
    assert_eq!(suspended.code, 0, "{}", suspended.stderr);
    assert_eq!(suspended.lines()[0]["success"], true);
    assert_eq!(suspended.lines()[1]["active"], false);
    assert!(suspended.lines()[1]["disableReason"].is_string());
    let active = muster(&fake, &["user", "list"]);
    assert_eq!(active.lines().len(), 1, "a suspended user is not listed");
    let everyone = muster(&fake, &["user", "list", "--all"]);
    assert_eq!(everyone.lines().len(), 2, "unless --all");
    assert!(everyone.lines().iter().any(|u| u["active"] == false));
    let back = muster(&fake, &["user", "unsuspend", "other@example.com"]);
    assert_eq!(
        back.code, 0,
        "a suspended user is still found: {}",
        back.stderr
    );
    assert_eq!(back.lines()[1]["active"], true);
    let revoked = muster(&fake, &["user", "revoke-sessions", "other"]);
    assert_eq!(revoked.code, 0, "{}", revoked.stderr);
    assert_eq!(revoked.lines()[0]["success"], true);
    assert_eq!(revoked.lines()[1]["email"], "other@example.com");

    for (op, args) in [
        ("UserUpdate", vec!["user", "update", "me", "--title", "x"]),
        (
            "UserChangeRole",
            vec!["user", "change-role", "other", "--role", "user"],
        ),
        ("UserSuspend", vec!["user", "suspend", "other"]),
        ("UserUnsuspend", vec!["user", "unsuspend", "other"]),
        (
            "UserRevokeAllSessions",
            vec!["user", "revoke-sessions", "other"],
        ),
    ] {
        fake.fail_next(op, Failure::Drop);
        let run = muster(&fake, &args);
        assert_eq!(run.code, 8, "{op}: {}", run.stderr);
    }
    fake.fail_next("UserById", Failure::Drop);
    let run = muster(&fake, &["user", "get", "me"]);
    assert_eq!(run.code, 7, "{}", run.stderr);
}

// ---- initiatives (ADR 0014, group 12) ----------------------------------------------------

#[test]
fn initiative_list_get_create_update_archive_and_projects() {
    let fake = Fake::start().unwrap();
    // The fake seeds one initiative, "Launch", with the project "Muster" in it.
    let seeded = muster(&fake, &["initiative", "list"]);
    assert_eq!(seeded.lines().len(), 1);
    assert_eq!(seeded.lines()[0]["name"], "Launch");
    assert_eq!(
        seeded.lines()[0].get("content"),
        None,
        "the list shape has no content"
    );
    let created = muster(
        &fake,
        &[
            "initiative",
            "create",
            "--name",
            "Growth",
            "--description",
            "Reach the second thousand.",
            "--owner",
            "me",
            "--lead-team",
            "APP",
            "--status",
            "Active",
            "--target-date",
            "2026-12-01",
            "--priority",
            "2",
            "--content",
            "# Growth\n\nThe plan.",
        ],
    );
    assert_eq!(created.code, 0, "{}", created.stderr);
    let v = created.one();
    assert_eq!(v["name"], "Growth");
    assert_eq!(v["status"], "Active");
    assert_eq!(v["owner"]["displayName"], "fake");
    assert_eq!(v["leadTeam"]["key"], "APP");
    assert_eq!(v["targetDate"], "2026-12-01");
    assert_eq!(v["priority"], 2);
    assert!(v["url"].is_string() && v["health"].is_null() && v["archivedAt"].is_null());
    assert_eq!(v.get("content"), None, "the create's answer has no content");
    assert_eq!(
        muster(&fake, &["initiative", "get", "Growth"]).one()["content"],
        "# Growth\n\nThe plan.",
        "--content inline lands in the content `initiative get` prints"
    );
    let both = muster(
        &fake,
        &[
            "initiative",
            "create",
            "--name",
            "Both",
            "--content",
            "x",
            "--content-file",
            "-",
        ],
    );
    assert_eq!(
        both.code, 2,
        "--content and --content-file conflict: {}",
        both.stderr
    );
    let again = muster(&fake, &["initiative", "create", "--name", "growth"]);
    assert_eq!(again.code, 6, "{}", again.stderr);
    assert_eq!(
        fake.operations()
            .iter()
            .filter(|o| *o == "InitiativeCreate")
            .count(),
        1,
        "refused before any mutation"
    );
    let planned = muster(&fake, &["initiative", "create", "--name", "Later"]);
    assert_eq!(planned.one()["status"], "Planned", "Linear's default");
    assert_eq!(muster(&fake, &["initiative", "list"]).lines().len(), 3);
    let active = muster(&fake, &["initiative", "list", "--status", "Active"]);
    assert_eq!(active.lines().len(), 2);
    let planned = muster(&fake, &["initiative", "list", "--status", "Planned"]);
    assert_eq!(planned.lines().len(), 1);
    assert_eq!(planned.lines()[0]["name"], "Later");
    let bad = muster(&fake, &["initiative", "list", "--status", "active"]);
    assert_eq!(bad.code, 2, "Linear's spelling: {}", bad.stderr);
    let cut = muster(&fake, &["initiative", "list", "--limit", "1"]);
    assert_eq!(cut.lines().len(), 1);
    assert_eq!(cut.notices()[0]["code"], "truncated");
    // The guard reports a failed read as itself rather than as "no duplicate".
    fake.fail_next("InitiativeByName", Failure::Drop);
    let unread = muster(&fake, &["initiative", "create", "--name", "Unread"]);
    assert_eq!(unread.code, 7, "{}", unread.stderr);
    assert_eq!(
        fake.operations()
            .iter()
            .filter(|o| *o == "InitiativeCreate")
            .count(),
        2,
        "no create after the guard's read failed"
    );

    let got = muster(&fake, &["initiative", "get", "launch"]);
    assert_eq!(got.code, 0, "{}", got.stderr);
    assert_eq!(got.one()["name"], "Launch");
    assert_eq!(got.one()["projects"]["nodes"][0]["name"], "Muster");
    assert!(got.one()["content"].is_null());
    assert_eq!(
        fake.operations().last().map(String::as_str),
        Some("InitiativeByName"),
        "one request"
    );
    let added = muster(
        &fake,
        &["initiative", "add-project", "Growth", "--project", "Muster"],
    );
    assert_eq!(added.code, 0, "{}", added.stderr);
    assert_eq!(added.one()["project"]["name"], "Muster");
    assert_eq!(added.one()["initiative"]["name"], "Growth");
    assert!(added.one()["id"].is_string() && added.one()["createdAt"].is_string());
    let twice = muster(
        &fake,
        &["initiative", "add-project", "Growth", "--project", "Muster"],
    );
    assert_eq!(
        twice.code, 6,
        "a project once per initiative: {}",
        twice.stderr
    );
    assert_eq!(
        muster(&fake, &["initiative", "get", "Growth"]).one()["projects"]["nodes"][0]["name"],
        "Muster"
    );
    let removed = muster(
        &fake,
        &[
            "initiative",
            "remove-project",
            "Growth",
            "--project",
            "Muster",
        ],
    );
    assert_eq!(removed.code, 0, "{}", removed.stderr);
    assert_eq!(removed.one()["success"], true);
    assert!(removed.one()["entityId"].is_string());
    assert_eq!(
        muster(&fake, &["initiative", "get", "Growth"]).one()["projects"]["nodes"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        muster(&fake, &["initiative", "get", "Launch"]).one()["projects"]["nodes"][0]["name"],
        "Muster",
        "the other initiative keeps its project"
    );
    let not_in = muster(
        &fake,
        &[
            "initiative",
            "remove-project",
            "Growth",
            "--project",
            "Muster",
        ],
    );
    assert_eq!(not_in.code, 4, "{}", not_in.stderr);
    let no_project = muster(
        &fake,
        &["initiative", "add-project", "Growth", "--project", "Nope"],
    );
    assert_eq!(no_project.code, 4, "{}", no_project.stderr);

    // The projects of `initiative get` are bounded by PROJECTS_PER_INITIATIVE.
    fake.add_initiative_projects("Growth", "Project", 51);
    let full = muster(&fake, &["initiative", "get", "Growth"]);
    assert_eq!(full.code, 0, "{}", full.stderr);
    assert_eq!(
        full.one()["projects"]["nodes"].as_array().unwrap().len(),
        50,
        "PROJECTS_PER_INITIATIVE"
    );
    assert_eq!(full.notices()[0]["code"], "truncated");
    assert_eq!(
        muster(&fake, &["initiative", "get", "Launch"])
            .notices()
            .len(),
        0,
        "one project, no notice"
    );

    let dir = temp_dir();
    let content = write(&dir, "content.md", "# Growth\n\nWhy and how.\n");
    let updated = muster(
        &fake,
        &[
            "initiative",
            "update",
            "Growth",
            "--owner",
            "none",
            "--lead-team",
            "none",
            "--status",
            "Completed",
            "--target-date",
            "none",
            "--content-file",
            &content,
        ],
    );
    assert_eq!(updated.code, 0, "{}", updated.stderr);
    let u = updated.one();
    assert!(u["owner"].is_null() && u["leadTeam"].is_null() && u["targetDate"].is_null());
    assert_eq!(u["status"], "Completed");
    assert_eq!(u["priority"], 2, "a flag left out leaves its field alone");
    assert_eq!(
        muster(&fake, &["initiative", "get", "Growth"]).one()["content"],
        "# Growth\n\nWhy and how.\n"
    );
    let renamed = muster(
        &fake,
        &[
            "initiative",
            "update",
            "Growth",
            "--name",
            "Scale",
            "--lead-team",
            "APP",
            "--content",
            "Scaled.",
        ],
    );
    assert_eq!(renamed.code, 0, "{}", renamed.stderr);
    assert_eq!(renamed.one()["name"], "Scale");
    assert_eq!(renamed.one()["leadTeam"]["key"], "APP");
    assert_eq!(
        muster(&fake, &["initiative", "get", "Scale"]).one()["content"],
        "Scaled.",
        "--content inline on update"
    );
    let both = muster(
        &fake,
        &[
            "initiative",
            "update",
            "Scale",
            "--content",
            "x",
            "--content-file",
            &content,
        ],
    );
    assert_eq!(
        both.code, 2,
        "--content and --content-file conflict: {}",
        both.stderr
    );
    let nothing = muster(&fake, &["initiative", "update", "Scale"]);
    assert_eq!(nothing.code, 2, "{}", nothing.stderr);
    let nope = muster(&fake, &["initiative", "update", "Nope", "--priority", "1"]);
    assert_eq!(nope.code, 4, "{}", nope.stderr);

    let archived = muster(&fake, &["initiative", "archive", "Scale"]);
    assert_eq!(archived.code, 0, "{}", archived.stderr);
    assert!(archived.one()["archivedAt"].is_string());
    assert_eq!(muster(&fake, &["initiative", "list"]).lines().len(), 2);
    assert_eq!(muster(&fake, &["initiative", "get", "Scale"]).code, 4);
    assert_eq!(
        muster(&fake, &["initiative", "archive", "Scale"]).code,
        4,
        "an archived initiative is not archived again"
    );
    let back = muster(&fake, &["initiative", "unarchive", "Scale"]);
    assert_eq!(back.code, 0, "{}", back.stderr);
    assert!(back.one()["archivedAt"].is_null());
    assert_eq!(muster(&fake, &["initiative", "list"]).lines().len(), 3);

    for (op, args) in [
        (
            "InitiativeCreate",
            vec!["initiative", "create", "--name", "Dropped"],
        ),
        (
            "InitiativeUpdate",
            vec!["initiative", "update", "Scale", "--priority", "1"],
        ),
        ("InitiativeArchive", vec!["initiative", "archive", "Scale"]),
        (
            "InitiativeToProjectCreate",
            vec!["initiative", "add-project", "Scale", "--project", "Muster"],
        ),
        (
            "InitiativeToProjectDelete",
            vec![
                "initiative",
                "remove-project",
                "Launch",
                "--project",
                "Muster",
            ],
        ),
    ] {
        fake.fail_next(op, Failure::Drop);
        let run = muster(&fake, &args);
        assert_eq!(run.code, 8, "{op}: {}", run.stderr);
    }
    let archived = muster(&fake, &["initiative", "archive", "Scale"]);
    assert_eq!(archived.code, 0, "{}", archived.stderr);
    fake.fail_next("InitiativeUnarchive", Failure::Drop);
    let run = muster(&fake, &["initiative", "unarchive", "Scale"]);
    assert_eq!(run.code, 8, "{}", run.stderr);
    fake.fail_next("Initiatives", Failure::Drop);
    let run = muster(&fake, &["initiative", "list"]);
    assert_eq!(run.code, 7, "{}", run.stderr);
    fake.fail_next("InitiativeByName", Failure::Drop);
    let run = muster(&fake, &["initiative", "get", "Launch"]);
    assert_eq!(run.code, 7, "{}", run.stderr);
    // The third read: the link list `remove-project` searches, dropped before the
    // delete, is exit 7 with no delete sent.
    let deletes = |fake: &Fake| {
        fake.operations()
            .iter()
            .filter(|o| *o == "InitiativeToProjectDelete")
            .count()
    };
    let before = deletes(&fake);
    fake.fail_next("InitiativeLinks", Failure::Drop);
    let run = muster(
        &fake,
        &[
            "initiative",
            "remove-project",
            "Launch",
            "--project",
            "Muster",
        ],
    );
    assert_eq!(run.code, 7, "{}", run.stderr);
    assert_eq!(
        deletes(&fake),
        before,
        "no delete after the link list's read failed"
    );
}

/// `initiative remove-project` reads one page of up to `PAGE_CAP` links, since
/// Linear's list takes no filter: a link past that page is refused, exit 6, naming
/// the bound, before any mutation, rather than reported missing.
#[test]
fn initiative_remove_project_refuses_a_link_past_page_cap() {
    let fake = Fake::start().unwrap();
    // The fake seeds one link, Launch to Muster; PAGE_CAP more put the last one past
    // the page `remove-project` reads.
    fake.add_initiative_projects("Launch", "Linked", PAGE_CAP);
    let last = format!("Linked {PAGE_CAP}");
    let cut = muster(
        &fake,
        &["initiative", "remove-project", "Launch", "--project", &last],
    );
    assert_eq!(cut.code, 6, "{}", cut.stderr);
    assert!(
        cut.error()["message"]
            .as_str()
            .unwrap()
            .contains("PAGE_CAP"),
        "{}",
        cut.stderr
    );
    assert!(
        !fake
            .operations()
            .iter()
            .any(|o| o == "InitiativeToProjectDelete"),
        "refused before any mutation"
    );
    // A project in no initiative is refused the same way while the list is cut: the
    // one page cannot tell the two apart.
    let made = muster(&fake, &["project", "create", "--name", "Unlinked"]);
    assert_eq!(made.code, 0, "{}", made.stderr);
    let unknown = muster(
        &fake,
        &[
            "initiative",
            "remove-project",
            "Launch",
            "--project",
            "Unlinked",
        ],
    );
    assert_eq!(unknown.code, 6, "{}", unknown.stderr);
    // A link within the page is found and deleted, and the list is then whole again,
    // so the last link is found too.
    let first = muster(
        &fake,
        &[
            "initiative",
            "remove-project",
            "Launch",
            "--project",
            "Muster",
        ],
    );
    assert_eq!(first.code, 0, "{}", first.stderr);
    assert_eq!(first.one()["success"], true);
    let now_within = muster(
        &fake,
        &["initiative", "remove-project", "Launch", "--project", &last],
    );
    assert_eq!(now_within.code, 0, "{}", now_within.stderr);
    let gone = muster(
        &fake,
        &[
            "initiative",
            "remove-project",
            "Launch",
            "--project",
            "Unlinked",
        ],
    );
    assert_eq!(
        gone.code, 4,
        "a whole list tells a missing link: {}",
        gone.stderr
    );
}

// ---- webhooks (ADR 0014, group 16) -------------------------------------------------------

#[test]
fn webhook_list_create_update_and_delete() {
    let fake = Fake::start().unwrap();
    // The fake seeds one webhook of the team, "Fake hook".
    let seeded = muster(&fake, &["webhook", "list"]);
    assert_eq!(seeded.code, 0, "{}", seeded.stderr);
    assert_eq!(seeded.lines().len(), 1);
    assert_eq!(seeded.lines()[0]["label"], "Fake hook");
    assert_eq!(seeded.lines()[0]["enabled"], true);
    assert_eq!(seeded.lines()[0]["team"]["key"], "APP");
    assert_eq!(
        seeded.lines()[0].get("secret"),
        None,
        "the secret is never asked for"
    );
    let dir = temp_dir();
    let secret = write(&dir, "secret", "s3cr3t-signing-key\n");
    let created = muster(
        &fake,
        &[
            "webhook",
            "create",
            "--url",
            "https://example.com/hooks/issues",
            "--resource-type",
            "Issue",
            "--resource-type",
            "Comment",
            "--label",
            "Issues and comments",
            "--secret-file",
            &secret,
        ],
    );
    assert_eq!(created.code, 0, "{}", created.stderr);
    let v = created.one();
    assert_eq!(v["url"], "https://example.com/hooks/issues");
    assert_eq!(v["label"], "Issues and comments");
    assert_eq!(v["resourceTypes"], serde_json::json!(["Issue", "Comment"]));
    assert_eq!(v["enabled"], true);
    assert_eq!(v["team"]["key"], "APP", "the team by the usual rule");
    assert_eq!(v["allPublicTeams"], false);
    assert_eq!(v.get("secret"), None);
    assert!(
        !created.stdout.contains("s3cr3t") && !created.stderr.contains("s3cr3t"),
        "the secret is never printed"
    );
    let again = muster(
        &fake,
        &[
            "webhook",
            "create",
            "--url",
            "https://example.com/hooks/issues",
            "--resource-type",
            "Issue",
        ],
    );
    assert_eq!(
        again.code, 6,
        "a webhook posts to that URL: {}",
        again.stderr
    );
    fake.fail_next("Webhooks", Failure::Drop);
    let unread = muster(
        &fake,
        &[
            "webhook",
            "create",
            "--url",
            "https://example.com/hooks/unread",
            "--resource-type",
            "Issue",
        ],
    );
    assert_eq!(unread.code, 7, "the guard's read failed: {}", unread.stderr);
    let no_type = muster(
        &fake,
        &[
            "webhook",
            "create",
            "--url",
            "https://example.com/hooks/none",
        ],
    );
    assert_eq!(
        no_type.code, 2,
        "--resource-type is required: {}",
        no_type.stderr
    );
    let empty_secret = write(&dir, "empty", "\n");
    let blank = muster(
        &fake,
        &[
            "webhook",
            "create",
            "--url",
            "https://example.com/hooks/blank",
            "--resource-type",
            "Issue",
            "--secret-file",
            &empty_secret,
        ],
    );
    assert_eq!(blank.code, 2, "{}", blank.stderr);
    let http = muster(
        &fake,
        &[
            "webhook",
            "create",
            "--url",
            "http://example.com/hooks/plain",
            "--resource-type",
            "Issue",
        ],
    );
    assert_eq!(
        http.code, 6,
        "Linear refuses a URL that is not https: {}",
        http.stderr
    );
    assert_eq!(
        fake.operations()
            .iter()
            .filter(|o| *o == "WebhookCreate")
            .count(),
        2,
        "the refused, failed and bad inputs sent no mutation but Linear's own refusal"
    );
    let public = muster(
        &fake,
        &[
            "webhook",
            "create",
            "--url",
            "https://example.com/hooks/all",
            "--resource-type",
            "Project",
            "--all-public-teams",
        ],
    );
    assert_eq!(public.code, 0, "{}", public.stderr);
    assert!(public.one()["team"].is_null());
    assert_eq!(public.one()["allPublicTeams"], true);
    assert!(public.one()["label"].is_null());

    assert_eq!(muster(&fake, &["webhook", "list"]).lines().len(), 3);
    let cut = muster(&fake, &["webhook", "list", "--limit", "1"]);
    assert_eq!(cut.lines().len(), 1);
    assert_eq!(cut.notices()[0]["code"], "truncated");

    // By label, by URL, by id.
    let off = muster(
        &fake,
        &[
            "webhook",
            "update",
            "issues and comments",
            "--enabled",
            "false",
        ],
    );
    assert_eq!(off.code, 0, "{}", off.stderr);
    assert_eq!(off.one()["enabled"], false);
    assert_eq!(
        off.one()["label"],
        "Issues and comments",
        "the rest left alone"
    );
    let by_url = muster(
        &fake,
        &[
            "webhook",
            "update",
            "https://example.com/hooks/all",
            "--label",
            "Every project",
            "--resource-type",
            "Project",
            "--resource-type",
            "ProjectUpdate",
        ],
    );
    assert_eq!(by_url.code, 0, "{}", by_url.stderr);
    assert_eq!(by_url.one()["label"], "Every project");
    assert_eq!(
        by_url.one()["resourceTypes"],
        serde_json::json!(["Project", "ProjectUpdate"])
    );
    let id = by_url.one()["id"].as_str().unwrap().to_string();
    let new_secret = write(&dir, "secret2", "another-secret");
    let by_id = muster(
        &fake,
        &[
            "webhook",
            "update",
            &id,
            "--url",
            "https://example.com/hooks/all-2",
            "--secret-file",
            &new_secret,
        ],
    );
    assert_eq!(by_id.code, 0, "{}", by_id.stderr);
    assert_eq!(by_id.one()["url"], "https://example.com/hooks/all-2");
    assert!(!by_id.stdout.contains("another-secret"));
    let bad_bool = muster(
        &fake,
        &["webhook", "update", "Every project", "--enabled", "off"],
    );
    assert_eq!(bad_bool.code, 2, "{}", bad_bool.stderr);
    let nothing = muster(&fake, &["webhook", "update", "Every project"]);
    assert_eq!(nothing.code, 2, "{}", nothing.stderr);
    let nope = muster(&fake, &["webhook", "update", "Nope", "--enabled", "true"]);
    assert_eq!(nope.code, 4, "{}", nope.stderr);
    assert_eq!(
        fake.operations()
            .iter()
            .filter(|o| *o == "WebhookUpdate")
            .count(),
        3,
        "the usage and not-found errors sent no mutation"
    );

    let deleted = muster(&fake, &["webhook", "delete", "Every project"]);
    assert_eq!(deleted.code, 0, "{}", deleted.stderr);
    assert_eq!(deleted.one()["success"], true);
    assert_eq!(deleted.one()["entityId"], id);
    assert_eq!(muster(&fake, &["webhook", "list"]).lines().len(), 2);
    assert_eq!(
        muster(&fake, &["webhook", "delete", "Every project"]).code,
        4
    );

    for (op, args) in [
        (
            "WebhookCreate",
            vec![
                "webhook",
                "create",
                "--url",
                "https://example.com/hooks/dropped",
                "--resource-type",
                "Issue",
            ],
        ),
        (
            "WebhookUpdate",
            vec!["webhook", "update", "Fake hook", "--enabled", "false"],
        ),
        ("WebhookDelete", vec!["webhook", "delete", "Fake hook"]),
    ] {
        fake.fail_next(op, Failure::Drop);
        let run = muster(&fake, &args);
        assert_eq!(run.code, 8, "{op}: {}", run.stderr);
    }
    fake.fail_next("Webhooks", Failure::Drop);
    let run = muster(&fake, &["webhook", "list"]);
    assert_eq!(run.code, 7, "{}", run.stderr);
    fake.fail_next("Webhooks", Failure::Drop);
    let run = muster(&fake, &["webhook", "delete", "Fake hook"]);
    assert_eq!(
        run.code, 7,
        "the reference's read failed before any mutation: {}",
        run.stderr
    );
}

// ---- project updates (ADR 0014, group 11) ------------------------------------------------

#[test]
fn project_update_list_and_create() {
    let fake = Fake::start().unwrap();
    // The fake seeds one post on Muster, "Kicked off.", so that the project carries
    // `lastUpdate` and `health` as the fixture records them.
    let seeded = muster(&fake, &["project-update", "list", "--project", "Muster"]);
    assert_eq!(seeded.code, 0, "{}", seeded.stderr);
    assert_eq!(seeded.lines().len(), 1);
    assert_eq!(seeded.lines()[0]["body"], "Kicked off.");
    assert_eq!(seeded.lines()[0]["health"], "onTrack");
    let before = muster(&fake, &["project", "get", "Muster"]).one();
    assert_eq!(before["health"], "onTrack");
    assert_eq!(before["lastUpdate"]["health"], "onTrack");
    // A project with no posts lists nothing and exits 0.
    let quiet = muster(&fake, &["project", "create", "--name", "Quiet"]);
    assert_eq!(quiet.code, 0, "{}", quiet.stderr);
    let none = muster(&fake, &["project-update", "list", "--project", "Quiet"]);
    assert_eq!(none.code, 0, "{}", none.stderr);
    assert_eq!(none.lines().len(), 0);
    let posted = muster(
        &fake,
        &[
            "project-update",
            "create",
            "--project",
            "muster",
            "--body",
            "Shipping on Friday.",
            "--health",
            "atRisk",
        ],
    );
    assert_eq!(posted.code, 0, "{}", posted.stderr);
    let v = posted.one();
    assert_eq!(v["body"], "Shipping on Friday.");
    assert_eq!(v["health"], "atRisk");
    assert_eq!(v["project"]["name"], "Muster");
    assert_eq!(v["user"]["displayName"], "fake");
    assert!(v["url"].is_string() && v["createdAt"].is_string() && v["editedAt"].is_null());
    let moved = muster(&fake, &["project", "get", "Muster"]).one();
    assert_eq!(
        moved["health"], "atRisk",
        "the project's health moves with its latest update"
    );
    assert_eq!(moved["lastUpdate"]["id"], v["id"]);
    let dir = temp_dir();
    let file = write(
        &dir,
        "update.md",
        "# Week 2

All green.
",
    );
    let from_file = muster(
        &fake,
        &[
            "project-update",
            "create",
            "--project",
            "Muster",
            "--body-file",
            &file,
            "--health",
            "onTrack",
        ],
    );
    assert_eq!(from_file.code, 0, "{}", from_file.stderr);
    assert_eq!(
        from_file.one()["body"],
        "# Week 2

All green.
"
    );
    let listed = muster(&fake, &["project-update", "list", "--project", "Muster"]);
    assert_eq!(listed.lines().len(), 3);
    assert_eq!(
        listed.lines()[0]["body"].as_str().unwrap().lines().next(),
        Some("# Week 2"),
        "newest first"
    );
    assert_eq!(listed.lines()[1]["health"], "atRisk");
    assert_eq!(
        listed.lines()[2]["body"],
        "Kicked off.",
        "the seeded post last"
    );
    let detail = muster(&fake, &["project", "get", "Muster"]).one();
    assert_eq!(
        detail["health"], "onTrack",
        "the project's health follows its last update"
    );
    assert_eq!(detail["lastUpdate"]["health"], "onTrack");
    assert_eq!(detail["lastUpdate"]["id"], from_file.one()["id"]);
    let cut = muster(
        &fake,
        &[
            "project-update",
            "list",
            "--project",
            "Muster",
            "--limit",
            "1",
        ],
    );
    assert_eq!(cut.lines().len(), 1);
    assert_eq!(cut.notices()[0]["code"], "truncated");
    let health_only = muster(
        &fake,
        &[
            "project-update",
            "create",
            "--project",
            "Muster",
            "--health",
            "offTrack",
        ],
    );
    assert_eq!(health_only.code, 0, "{}", health_only.stderr);
    let nothing = muster(&fake, &["project-update", "create", "--project", "Muster"]);
    assert_eq!(nothing.code, 2, "{}", nothing.stderr);
    let bad = muster(
        &fake,
        &[
            "project-update",
            "create",
            "--project",
            "Muster",
            "--health",
            "fine",
        ],
    );
    assert_eq!(bad.code, 2, "Linear's three spellings: {}", bad.stderr);
    let nowhere = muster(
        &fake,
        &[
            "project-update",
            "create",
            "--project",
            "Nope",
            "--body",
            "x",
        ],
    );
    assert_eq!(nowhere.code, 4, "{}", nowhere.stderr);
    assert_eq!(
        fake.operations()
            .iter()
            .filter(|o| *o == "ProjectUpdateCreate")
            .count(),
        3,
        "the refusals sent nothing"
    );
    fake.fail_next("ProjectUpdateCreate", Failure::Drop);
    let run = muster(
        &fake,
        &[
            "project-update",
            "create",
            "--project",
            "Muster",
            "--body",
            "x",
        ],
    );
    assert_eq!(run.code, 8, "{}", run.stderr);
    fake.fail_next("ProjectUpdates", Failure::Drop);
    let run = muster(&fake, &["project-update", "list", "--project", "Muster"]);
    assert_eq!(run.code, 7, "{}", run.stderr);
}

// ---- milestones (ADR 0014, group 10) -----------------------------------------------------

#[test]
fn milestone_list_create_update_delete_and_the_issue_flag() {
    let fake = Fake::start().unwrap();
    // The fake seeds one milestone, "First cut", so that `project get` prints a node.
    let seeded = muster(&fake, &["milestone", "list", "--project", "Muster"]);
    assert_eq!(seeded.code, 0, "{}", seeded.stderr);
    assert_eq!(seeded.lines().len(), 1);
    assert_eq!(seeded.lines()[0]["name"], "First cut");
    let created = muster(
        &fake,
        &[
            "milestone",
            "create",
            "--project",
            "muster",
            "--name",
            "Alpha",
            "--description",
            "The first cut.",
            "--target-date",
            "2026-12-01",
        ],
    );
    assert_eq!(created.code, 0, "{}", created.stderr);
    let v = created.one();
    assert_eq!(v["name"], "Alpha");
    assert_eq!(v["targetDate"], "2026-12-01");
    assert_eq!(v["project"]["name"], "Muster");
    assert!(v["status"].is_string() && v["progress"].is_number());
    let again = muster(
        &fake,
        &[
            "milestone",
            "create",
            "--project",
            "Muster",
            "--name",
            "alpha",
        ],
    );
    assert_eq!(again.code, 6, "{}", again.stderr);
    assert_eq!(
        fake.operations()
            .iter()
            .filter(|o| *o == "MilestoneCreate")
            .count(),
        1,
        "refused before any mutation"
    );
    // The guard's read failing is the command's exit, never "no duplicate" and a
    // create: an agent that retries after a timeout makes one, not two (ADR 0008).
    fake.fail_next("Milestones", Failure::Drop);
    let unread = muster(
        &fake,
        &[
            "milestone",
            "create",
            "--project",
            "Muster",
            "--name",
            "Alpha",
        ],
    );
    assert_eq!(unread.code, 7, "{}", unread.stderr);
    assert_eq!(
        fake.operations()
            .iter()
            .filter(|o| *o == "MilestoneCreate")
            .count(),
        1,
        "no mutation after a guard whose read failed"
    );
    let nowhere = muster(
        &fake,
        &["milestone", "create", "--project", "Nope", "--name", "x"],
    );
    assert_eq!(nowhere.code, 4, "{}", nowhere.stderr);
    let listed = muster(&fake, &["milestone", "list", "--project", "Muster"]);
    assert_eq!(listed.lines().len(), 2);
    // The list at its bound: what fits, and a `truncated` notice, never silence.
    let cut = muster(
        &fake,
        &["milestone", "list", "--project", "Muster", "--limit", "1"],
    );
    assert_eq!(cut.code, 0, "{}", cut.stderr);
    assert_eq!(cut.lines().len(), 1);
    assert_eq!(cut.lines()[0]["name"], "First cut");
    assert_eq!(cut.notices()[0]["code"], "truncated", "{}", cut.stderr);
    let detail = muster(&fake, &["project", "get", "Muster"]).one();
    assert!(
        detail["projectMilestones"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["name"] == "Alpha"),
        "`project get` lists it"
    );

    let issue = muster(
        &fake,
        &[
            "issue",
            "create",
            "--title",
            "In alpha",
            "--project",
            "Muster",
            "--milestone",
            "Alpha",
        ],
    );
    assert_eq!(issue.code, 0, "{}", issue.stderr);
    assert_eq!(issue.one()["projectMilestone"]["name"], "Alpha");
    let id = issue.one()["identifier"].as_str().unwrap().to_string();
    let orphan = muster(
        &fake,
        &[
            "issue",
            "create",
            "--title",
            "Nowhere",
            "--milestone",
            "Alpha",
        ],
    );
    assert_eq!(
        orphan.code, 2,
        "--milestone needs --project: {}",
        orphan.stderr
    );
    let listed = muster(&fake, &["issue", "list", "--milestone", "alpha"]);
    assert_eq!(listed.lines().len(), 1);
    assert_eq!(listed.lines()[0]["identifier"], id);
    let without = muster(&fake, &["issue", "list", "--milestone", "none", "--all"]);
    assert!(
        without
            .lines()
            .iter()
            .all(|i| i["projectMilestone"].is_null())
    );
    let cleared = muster(&fake, &["issue", "update", &id, "--milestone", "none"]);
    assert_eq!(cleared.code, 0, "{}", cleared.stderr);
    assert!(cleared.one()["projectMilestone"].is_null());
    let back = muster(&fake, &["issue", "update", &id, "--milestone", "Alpha"]);
    assert_eq!(
        back.code, 0,
        "resolved in the issue's own project: {}",
        back.stderr
    );
    assert_eq!(back.one()["projectMilestone"]["name"], "Alpha");
    let no_project = muster(&fake, &["issue", "update", "APP-1", "--milestone", "Alpha"]);
    assert_eq!(
        no_project.code, 2,
        "an issue in no project: {}",
        no_project.stderr
    );

    let renamed = muster(
        &fake,
        &[
            "milestone",
            "update",
            "Alpha",
            "--project",
            "Muster",
            "--name",
            "Beta",
            "--target-date",
            "none",
        ],
    );
    assert_eq!(renamed.code, 0, "{}", renamed.stderr);
    assert_eq!(renamed.one()["name"], "Beta");
    assert!(renamed.one()["targetDate"].is_null());
    assert_eq!(
        muster(&fake, &["issue", "get", &id]).one()["projectMilestone"]["name"],
        "Beta",
        "the issue shows the new name"
    );
    let nothing = muster(
        &fake,
        &["milestone", "update", "Beta", "--project", "Muster"],
    );
    assert_eq!(nothing.code, 2, "{}", nothing.stderr);

    let deleted = muster(
        &fake,
        &["milestone", "delete", "Beta", "--project", "Muster"],
    );
    assert_eq!(deleted.code, 0, "{}", deleted.stderr);
    assert_eq!(deleted.one()["success"], true);
    assert!(
        muster(&fake, &["issue", "get", &id]).one()["projectMilestone"].is_null(),
        "a deleted milestone leaves its issues"
    );
    let gone = muster(
        &fake,
        &["milestone", "delete", "Beta", "--project", "Muster"],
    );
    assert_eq!(gone.code, 4, "{}", gone.stderr);

    fake.fail_next("MilestoneCreate", Failure::Drop);
    let run = muster(
        &fake,
        &[
            "milestone",
            "create",
            "--project",
            "Muster",
            "--name",
            "Dropped",
        ],
    );
    assert_eq!(run.code, 8, "{}", run.stderr);
    let created = muster(
        &fake,
        &[
            "milestone",
            "create",
            "--project",
            "Muster",
            "--name",
            "Gamma",
        ],
    );
    assert_eq!(created.code, 0, "{}", created.stderr);
    fake.fail_next("MilestoneUpdate", Failure::Drop);
    let run = muster(
        &fake,
        &[
            "milestone",
            "update",
            "Gamma",
            "--project",
            "Muster",
            "--name",
            "x",
        ],
    );
    assert_eq!(run.code, 8, "{}", run.stderr);
    fake.fail_next("MilestoneDelete", Failure::Drop);
    let run = muster(
        &fake,
        &["milestone", "delete", "Gamma", "--project", "Muster"],
    );
    assert_eq!(run.code, 8, "{}", run.stderr);
    fake.fail_next("Milestones", Failure::Drop);
    let run = muster(&fake, &["milestone", "list", "--project", "Muster"]);
    assert_eq!(run.code, 7, "{}", run.stderr);
}

// ---- project statuses (ADR 0014, group 9) ------------------------------------------------

#[test]
fn project_status_list_create_update_archive_and_unarchive() {
    let fake = Fake::start().unwrap();
    let listed = muster(&fake, &["project-status", "list"]);
    assert_eq!(listed.code, 0, "{}", listed.stderr);
    let statuses = listed.lines();
    assert_eq!(statuses.len(), 5, "Linear's five defaults");
    assert_eq!(statuses[0]["name"], "Backlog", "by position");
    assert_eq!(statuses[2]["type"], "started");
    assert!(statuses[0]["indefinite"].is_boolean() && statuses[0]["color"].is_string());
    // The list at its bound: what fits, and a `truncated` notice, never silence.
    let cut = muster(&fake, &["project-status", "list", "--limit", "1"]);
    assert_eq!(cut.code, 0, "{}", cut.stderr);
    assert_eq!(cut.lines().len(), 1);
    assert_eq!(cut.lines()[0]["name"], "Backlog");
    assert_eq!(cut.notices()[0]["code"], "truncated", "{}", cut.stderr);
    let created = muster(
        &fake,
        &[
            "project-status",
            "create",
            "--name",
            "Paused",
            "--type",
            "planned",
            "--color",
            "#f2c94c",
            "--position",
            "2.5",
            "--indefinite",
            "true",
        ],
    );
    assert_eq!(created.code, 0, "{}", created.stderr);
    let v = created.one();
    assert_eq!(v["name"], "Paused");
    assert_eq!(
        v["type"], "planned",
        "a second planned status, so that archiving it leaves one of the type"
    );
    assert_eq!(v["position"], 2.5);
    assert_eq!(v["indefinite"], true);
    // One order across the workspace, not one per type: Linear's `position` is "the
    // position of the status in the workspace's project flow", and the defaults sit
    // at 0 to 4, so a planned status at 2.5 lists between In Progress (2, started)
    // and Completed (3, completed), the two types either side of it.
    let listed = muster(&fake, &["project-status", "list"]).lines();
    assert_eq!(listed.len(), 6);
    assert_eq!(listed[2]["name"], "In Progress");
    assert_eq!(
        listed[3]["name"], "Paused",
        "between In Progress and Completed, across types"
    );
    assert_eq!(listed[4]["name"], "Completed");
    let again = muster(
        &fake,
        &[
            "project-status",
            "create",
            "--name",
            "paused",
            "--type",
            "paused",
            "--color",
            "#000000",
            "--position",
            "9",
        ],
    );
    assert_eq!(again.code, 6, "{}", again.stderr);
    assert_eq!(
        fake.operations()
            .iter()
            .filter(|o| *o == "ProjectStatusCreate")
            .count(),
        1,
        "refused before any mutation"
    );
    let moved = muster(
        &fake,
        &["project", "update", "Muster", "--status", "Paused"],
    );
    assert_eq!(
        moved.code, 0,
        "the new status takes projects: {}",
        moved.stderr
    );

    let renamed = muster(
        &fake,
        &[
            "project-status",
            "update",
            "Paused",
            "--name",
            "On hold",
            "--indefinite",
            "false",
        ],
    );
    assert_eq!(renamed.code, 0, "{}", renamed.stderr);
    assert_eq!(renamed.one()["name"], "On hold");
    assert_eq!(renamed.one()["indefinite"], false);
    assert_eq!(
        muster(&fake, &["project", "get", "Muster"]).one()["status"]["name"],
        "On hold",
        "the project shows the new name"
    );
    let nothing = muster(&fake, &["project-status", "update", "On hold"]);
    assert_eq!(nothing.code, 2, "{}", nothing.stderr);
    let nope = muster(
        &fake,
        &["project-status", "update", "Nope", "--color", "#000000"],
    );
    assert_eq!(nope.code, 4, "{}", nope.stderr);
    // `f64::from_str`, which `clap` parses `--position` with, takes `nan`, `inf` and
    // `1e400` (which overflows to infinity), and serde_json would write each as `null`:
    // a position the caller never typed. Each is muster's usage error, before any
    // request, as on `state create` and `state update`; the hint names this group's
    // list. The `=` form, since `clap` reads a bare `-inf` as flags.
    let sent = fake.operations().len();
    for position in ["nan", "inf", "-inf", "1e400"] {
        let flag = format!("--position={position}");
        let create = muster(
            &fake,
            &[
                "project-status",
                "create",
                "--name",
                "Nowhere",
                "--type",
                "backlog",
                "--color",
                "#000000",
                &flag,
            ],
        );
        assert_eq!(create.code, 2, "{flag}: {}", create.stderr);
        assert_eq!(create.error()["code"], "usage", "{flag}: {}", create.stderr);
        assert!(
            create.error()["hint"]
                .as_str()
                .unwrap()
                .contains("muster project-status list"),
            "{flag}: {}",
            create.stderr
        );
        let update = muster(&fake, &["project-status", "update", "On hold", &flag]);
        assert_eq!(update.code, 2, "{flag}: {}", update.stderr);
        assert_eq!(update.error()["code"], "usage", "{flag}: {}", update.stderr);
    }
    assert_eq!(
        fake.operations().len(),
        sent,
        "refused before any request: no ProjectStatusCreate, no ProjectStatusUpdate"
    );

    let held = muster(&fake, &["project-status", "archive", "On hold"]);
    assert_eq!(
        held.code, 6,
        "a status with a project in it: {}",
        held.stderr
    );
    let away = muster(
        &fake,
        &["project", "update", "Muster", "--status", "Planned"],
    );
    assert_eq!(away.code, 0, "{}", away.stderr);
    let archived = muster(&fake, &["project-status", "archive", "On hold"]);
    assert_eq!(archived.code, 0, "{}", archived.stderr);
    assert_eq!(archived.one()["name"], "On hold", "Linear's entity");
    assert_eq!(muster(&fake, &["project-status", "list"]).lines().len(), 5);
    let last = muster(&fake, &["project-status", "archive", "Backlog"]);
    assert_eq!(last.code, 6, "the last of its type: {}", last.stderr);
    let back = muster(&fake, &["project-status", "unarchive", "On hold"]);
    assert_eq!(
        back.code, 0,
        "an archived status is still found: {}",
        back.stderr
    );
    assert_eq!(back.one()["name"], "On hold");
    assert_eq!(muster(&fake, &["project-status", "list"]).lines().len(), 6);

    for (op, args) in [
        (
            "ProjectStatusCreate",
            vec![
                "project-status",
                "create",
                "--name",
                "Dropped",
                "--type",
                "backlog",
                "--color",
                "#000000",
                "--position",
                "9",
            ],
        ),
        (
            "ProjectStatusUpdate",
            vec!["project-status", "update", "On hold", "--color", "#000000"],
        ),
        (
            "ProjectStatusArchive",
            vec!["project-status", "archive", "On hold"],
        ),
    ] {
        fake.fail_next(op, Failure::Drop);
        let run = muster(&fake, &args);
        assert_eq!(run.code, 8, "{op}: {}", run.stderr);
    }
    let archived = muster(&fake, &["project-status", "archive", "On hold"]);
    assert_eq!(archived.code, 0, "{}", archived.stderr);
    fake.fail_next("ProjectStatusUnarchive", Failure::Drop);
    let run = muster(&fake, &["project-status", "unarchive", "On hold"]);
    assert_eq!(run.code, 8, "{}", run.stderr);
    fake.fail_next("ProjectStatuses", Failure::Drop);
    let run = muster(&fake, &["project-status", "list"]);
    assert_eq!(run.code, 7, "{}", run.stderr);
}

// ---- project get, update, delete, unarchive (ADR 0014, group 8) --------------------------

#[test]
fn project_get_prints_the_detail_and_bounds_the_members_and_milestones() {
    let fake = Fake::start().unwrap();
    let run = muster(&fake, &["project", "get", "muster"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let v = run.one();
    assert_eq!(v["name"], "Muster");
    assert_eq!(v["status"]["name"], "Planned");
    assert_eq!(v["lead"]["displayName"], "fake");
    assert_eq!(v["teams"]["nodes"][0]["key"], "APP");
    assert_eq!(v["members"]["nodes"][0]["displayName"], "fake");
    let milestone = &v["projectMilestones"]["nodes"][0];
    assert_eq!(milestone["name"], "First cut");
    assert_eq!(milestone["targetDate"], "2026-12-01");
    assert!(milestone["status"].is_string() && milestone["progress"].is_number());
    assert_eq!(v["projectMilestones"]["nodes"].as_array().unwrap().len(), 1);
    assert!(v["slugId"].is_string() && v["progress"].is_number() && v["createdAt"].is_string());
    assert!(v["content"].is_null() && v["trashed"].is_null() && v["archivedAt"].is_null());
    assert!(v["creator"]["id"].is_string());
    assert_eq!(fake.operations(), vec!["ProjectByName"], "one request");
    assert!(run.notices().is_empty());
    let nope = muster(&fake, &["project", "get", "Nope"]);
    assert_eq!(nope.code, 4, "{}", nope.stderr);

    // MILESTONES_PER_PROJECT at the bound: the seeded milestone and fifty more make
    // fifty-one, the page holds fifty, and the notice says so.
    fake.add_project_milestones("Muster", MILESTONES_PER_PROJECT);
    let stones = muster(&fake, &["project", "get", "Muster"]);
    assert_eq!(stones.code, 0, "{}", stones.stderr);
    assert_eq!(
        stones.one()["projectMilestones"]["nodes"]
            .as_array()
            .unwrap()
            .len(),
        MILESTONES_PER_PROJECT
    );
    assert_eq!(
        stones.one()["projectMilestones"]["pageInfo"]["hasNextPage"],
        true
    );
    let notices = stones.notices();
    assert_eq!(notices.len(), 1, "{}", stones.stderr);
    assert!(
        notices[0]["message"]
            .as_str()
            .unwrap()
            .starts_with("projectMilestones:")
            && notices[0]["hint"]
                .as_str()
                .unwrap()
                .contains("MILESTONES_PER_PROJECT"),
        "{}",
        stones.stderr
    );

    // MEMBERS_PER_PROJECT at the bound, the same way; both connections are now cut, so
    // there is one notice per connection, in the order the fields are printed.
    fake.add_project_members("Muster", MEMBERS_PER_PROJECT);
    let cut = muster(&fake, &["project", "get", "Muster"]);
    assert_eq!(cut.code, 0, "{}", cut.stderr);
    assert_eq!(
        cut.one()["members"]["nodes"].as_array().unwrap().len(),
        MEMBERS_PER_PROJECT
    );
    assert_eq!(cut.one()["members"]["pageInfo"]["hasNextPage"], true);
    let notices = cut.notices();
    assert_eq!(notices.len(), 2, "{}", cut.stderr);
    assert!(
        notices[0]["message"]
            .as_str()
            .unwrap()
            .starts_with("members:")
            && notices[0]["hint"]
                .as_str()
                .unwrap()
                .contains("MEMBERS_PER_PROJECT"),
        "{}",
        cut.stderr
    );
    assert!(
        notices[1]["message"]
            .as_str()
            .unwrap()
            .starts_with("projectMilestones:"),
        "{}",
        cut.stderr
    );
    let refused = muster(
        &fake,
        &["project", "update", "Muster", "--add-member", "other"],
    );
    assert_eq!(
        refused.code, 6,
        "a member list that cannot be read whole is not replaced: {}",
        refused.stderr
    );
    assert!(
        refused.error()["message"]
            .as_str()
            .unwrap()
            .contains("MEMBERS_PER_PROJECT"),
        "{}",
        refused.stderr
    );
    assert!(
        !fake.operations().iter().any(|o| o == "ProjectUpdate"),
        "refused before any mutation"
    );
    let renamed = muster(
        &fake,
        &["project", "update", "Muster", "--name", "Muster 2"],
    );
    assert_eq!(
        renamed.code, 0,
        "a change that leaves the members alone goes through: {}",
        renamed.stderr
    );

    fake.fail_next("ProjectByName", Failure::Drop);
    let run = muster(&fake, &["project", "get", "Muster 2"]);
    assert_eq!(run.code, 7, "{}", run.stderr);
}

#[test]
fn project_update_delete_and_unarchive() {
    let fake = Fake::start().unwrap();
    fake.add_team("OPS", "Operations");
    let dir = temp_dir();
    let content = write(
        &dir,
        "content.md",
        "# The plan

Three steps.
",
    );
    let run = muster(
        &fake,
        &[
            "project",
            "update",
            "Muster",
            "--description",
            "Renewed.",
            "--content-file",
            &content,
            "--status",
            "In Progress",
            "--lead",
            "other@example.com",
            "--start-date",
            "2026-11-10",
            "--target-date",
            "2026-12-01",
            "--priority",
            "2",
            "--teams",
            "APP,OPS",
            "--add-member",
            "other",
            "--color",
            "#4ea7fc",
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let v = run.one();
    assert_eq!(v["description"], "Renewed.");
    assert_eq!(
        v["status"]["name"], "In Progress",
        "resolved among the workspace's statuses"
    );
    assert_eq!(v["lead"]["displayName"], "other");
    assert_eq!(v["startDate"], "2026-11-10");
    assert_eq!(v["targetDate"], "2026-12-01");
    assert_eq!(v["priority"], 2);
    assert_eq!(v["teams"]["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(
        v["name"], "Muster",
        "a flag left out leaves its field alone"
    );
    let detail = muster(&fake, &["project", "get", "Muster"]).one();
    assert_eq!(
        detail["content"],
        "# The plan

Three steps.
"
    );
    assert_eq!(
        detail["members"]["nodes"].as_array().unwrap().len(),
        2,
        "me and other"
    );
    let inline = muster(
        &fake,
        &["project", "update", "Muster", "--content", "One line."],
    );
    assert_eq!(inline.code, 0, "{}", inline.stderr);
    assert_eq!(
        muster(&fake, &["project", "get", "Muster"]).one()["content"],
        "One line.",
        "--content inline, as --content-file from a file"
    );
    let both = muster(
        &fake,
        &[
            "project",
            "update",
            "Muster",
            "--content",
            "x",
            "--content-file",
            &content,
        ],
    );
    assert_eq!(
        both.code, 2,
        "--content and --content-file conflict: {}",
        both.stderr
    );
    let fewer = muster(
        &fake,
        &[
            "project",
            "update",
            "Muster",
            "--remove-member",
            "me",
            "--lead",
            "none",
            "--target-date",
            "none",
        ],
    );
    assert_eq!(fewer.code, 0, "{}", fewer.stderr);
    assert!(fewer.one()["lead"].is_null() && fewer.one()["targetDate"].is_null());
    let detail = muster(&fake, &["project", "get", "Muster"]).one();
    assert_eq!(detail["members"]["nodes"][0]["displayName"], "other");
    let no_status = muster(&fake, &["project", "update", "Muster", "--status", "Nope"]);
    assert_eq!(no_status.code, 4, "{}", no_status.stderr);
    let no_teams = muster(&fake, &["project", "update", "Muster", "--teams", ""]);
    assert_eq!(no_teams.code, 2, "{}", no_teams.stderr);
    let nothing = muster(&fake, &["project", "update", "Muster"]);
    assert_eq!(nothing.code, 2, "{}", nothing.stderr);
    assert_eq!(
        fake.operations()
            .iter()
            .filter(|o| *o == "ProjectUpdate")
            .count(),
        3,
        "the refusals sent nothing"
    );

    let gone = muster(&fake, &["project", "delete", "Muster"]);
    assert_eq!(gone.code, 0, "{}", gone.stderr);
    assert_eq!(gone.one()["name"], "Muster", "Linear's entity, as it is");
    let listed = muster(&fake, &["project", "list"]);
    assert!(
        listed.lines().iter().all(|p| p["name"] != "Muster"),
        "a trashed project is not listed"
    );
    let get = muster(&fake, &["project", "get", "Muster"]);
    assert_eq!(get.code, 4, "{}", get.stderr);
    let back = muster(&fake, &["project", "unarchive", "Muster"]);
    assert_eq!(back.code, 0, "{}", back.stderr);
    assert_eq!(back.one()["name"], "Muster");
    let detail = muster(&fake, &["project", "get", "Muster"]).one();
    assert!(detail["trashed"].is_null() && detail["archivedAt"].is_null());
    let twice = muster(&fake, &["project", "unarchive", "Muster"]);
    assert_eq!(twice.code, 4, "not trashed: {}", twice.stderr);

    for (op, args) in [
        (
            "ProjectUpdate",
            vec!["project", "update", "Muster", "--priority", "1"],
        ),
        ("ProjectDelete", vec!["project", "delete", "Muster"]),
    ] {
        fake.fail_next(op, Failure::Drop);
        let run = muster(&fake, &args);
        assert_eq!(run.code, 8, "{op}: {}", run.stderr);
    }
    let gone = muster(&fake, &["project", "delete", "Muster"]);
    assert_eq!(gone.code, 0, "{}", gone.stderr);
    fake.fail_next("ProjectUnarchive", Failure::Drop);
    let run = muster(&fake, &["project", "unarchive", "Muster"]);
    assert_eq!(run.code, 8, "{}", run.stderr);
}

// ---- states (ADR 0014, group 7) ----------------------------------------------------------

#[test]
fn state_create_update_and_archive() {
    let fake = Fake::start().unwrap();
    let created = muster(
        &fake,
        &[
            "state",
            "create",
            "--name",
            "Blocked",
            "--type",
            "started",
            "--color",
            "#eb5757",
            "--description",
            "Waiting on someone else.",
        ],
    );
    assert_eq!(created.code, 0, "{}", created.stderr);
    let v = created.one();
    assert_eq!(v["name"], "Blocked");
    assert_eq!(v["type"], "started");
    assert_eq!(v["color"], "#eb5757");
    assert_eq!(v["team"]["key"], "APP");
    assert!(v["position"].is_number(), "Linear puts it at the end");
    let listed = muster(&fake, &["state", "list"]);
    assert_eq!(
        listed.lines().len(),
        8,
        "the seven default states and the new one"
    );
    assert_eq!(listed.lines()[7]["name"], "Blocked", "by position");
    let again = muster(
        &fake,
        &[
            "state", "create", "--name", "blocked", "--type", "backlog", "--color", "#000000",
        ],
    );
    assert_eq!(again.code, 6, "{}", again.stderr);
    let triage = muster(
        &fake,
        &[
            "state", "create", "--name", "Inbox", "--type", "triage", "--color", "#000000",
        ],
    );
    assert_eq!(
        triage.code, 2,
        "triage is Linear's own type: {}",
        triage.stderr
    );
    // `f64::from_str`, which `clap` parses `--position` with, takes `nan`, `inf` and
    // `1e400` (which overflows to infinity), and serde_json would write each as `null`:
    // a position the caller never typed. Each is muster's usage error, before any
    // request. The `=` form, since `clap` reads a bare `-inf` as flags.
    for position in ["nan", "inf", "-inf", "1e400"] {
        let flag = format!("--position={position}");
        let create = muster(
            &fake,
            &[
                "state", "create", "--name", "Nowhere", "--type", "backlog", "--color", "#000000",
                &flag,
            ],
        );
        assert_eq!(create.code, 2, "{flag}: {}", create.stderr);
        assert_eq!(create.error()["code"], "usage", "{flag}: {}", create.stderr);
        let update = muster(&fake, &["state", "update", "Blocked", &flag]);
        assert_eq!(update.code, 2, "{flag}: {}", update.stderr);
        assert_eq!(update.error()["code"], "usage", "{flag}: {}", update.stderr);
    }
    assert_eq!(
        fake.operations()
            .iter()
            .filter(|o| *o == "WorkflowStateCreate")
            .count(),
        1,
        "refused before any mutation"
    );
    assert!(
        !fake.operations().iter().any(|o| o == "WorkflowStateUpdate"),
        "refused before any mutation"
    );
    let moved = muster(&fake, &["issue", "update", "APP-1", "--state", "Blocked"]);
    assert_eq!(moved.code, 0, "a new state takes issues: {}", moved.stderr);

    let renamed = muster(
        &fake,
        &[
            "state",
            "update",
            "Blocked",
            "--name",
            "Waiting",
            "--position",
            "2.5",
        ],
    );
    assert_eq!(renamed.code, 0, "{}", renamed.stderr);
    assert_eq!(renamed.one()["name"], "Waiting");
    assert_eq!(renamed.one()["position"], 2.5);
    assert_eq!(
        muster(&fake, &["issue", "get", "APP-1"]).one()["state"]["name"],
        "Waiting",
        "the issue shows the new name"
    );
    let nothing = muster(&fake, &["state", "update", "Waiting"]);
    assert_eq!(nothing.code, 2, "{}", nothing.stderr);
    let nope = muster(&fake, &["state", "update", "Nope", "--name", "x"]);
    assert_eq!(nope.code, 4, "{}", nope.stderr);

    let held = muster(&fake, &["state", "archive", "Waiting"]);
    assert_eq!(
        held.code, 6,
        "Linear refuses a state with an issue in it: {}",
        held.stderr
    );
    assert!(held.error()["linear"]["message"].is_string());
    let away = muster(&fake, &["issue", "update", "APP-1", "--state", "Todo"]);
    assert_eq!(away.code, 0, "{}", away.stderr);
    let archived = muster(&fake, &["state", "archive", "Waiting"]);
    assert_eq!(archived.code, 0, "{}", archived.stderr);
    assert_eq!(
        archived.one()["name"],
        "Waiting",
        "Linear's entity, as it is"
    );
    assert_eq!(muster(&fake, &["state", "list"]).lines().len(), 7);
    let gone = muster(&fake, &["state", "archive", "Waiting"]);
    assert_eq!(gone.code, 4, "{}", gone.stderr);

    for (op, args) in [
        (
            "WorkflowStateCreate",
            vec![
                "state", "create", "--name", "Dropped", "--type", "backlog", "--color", "#000000",
            ],
        ),
        (
            "WorkflowStateUpdate",
            vec!["state", "update", "Todo", "--color", "#000000"],
        ),
        (
            "WorkflowStateArchive",
            vec!["state", "archive", "Duplicate"],
        ),
    ] {
        fake.fail_next(op, Failure::Drop);
        let run = muster(&fake, &args);
        assert_eq!(run.code, 8, "{op}: {}", run.stderr);
    }
}

// ---- labels (ADR 0014, group 6) ----------------------------------------------------------

#[test]
fn label_create_update_retire_restore_and_delete() {
    let fake = Fake::start().unwrap();
    let created = muster(
        &fake,
        &[
            "label",
            "create",
            "--name",
            "Smoke",
            "--color",
            "#26b5ce",
            "--description",
            "Made by the smoke test.",
        ],
    );
    assert_eq!(created.code, 0, "{}", created.stderr);
    let v = created.one();
    assert_eq!(v["name"], "Smoke");
    assert_eq!(v["color"], "#26b5ce");
    assert_eq!(v["team"]["key"], "APP", "a team label by the team rule");
    assert_eq!(v["isGroup"], false);
    assert!(v["retiredAt"].is_null() && v["parent"].is_null());
    let listed = muster(&fake, &["label", "list"]);
    assert_eq!(
        listed.lines().len(),
        5,
        "three workspace labels, two of the team"
    );
    let again = muster(&fake, &["label", "create", "--name", "smoke"]);
    assert_eq!(again.code, 6, "{}", again.stderr);
    let for_workspace = muster(
        &fake,
        &["label", "create", "--name", "Smoke", "--workspace"],
    );
    assert_eq!(
        for_workspace.code, 0,
        "the guard is per scope: {}",
        for_workspace.stderr
    );
    assert!(for_workspace.one()["team"].is_null());
    let bug_again = muster(&fake, &["label", "create", "--name", "bug", "--workspace"]);
    assert_eq!(bug_again.code, 6, "{}", bug_again.stderr);
    assert_eq!(
        fake.operations()
            .iter()
            .filter(|o| *o == "IssueLabelCreate")
            .count(),
        2,
        "refused before any mutation"
    );
    let group = muster(&fake, &["label", "create", "--name", "Area", "--group"]);
    assert_eq!(group.code, 0, "{}", group.stderr);
    assert_eq!(group.one()["isGroup"], true);
    let child = muster(
        &fake,
        &["label", "create", "--name", "Backend", "--parent", "Area"],
    );
    assert_eq!(child.code, 0, "{}", child.stderr);
    assert_eq!(child.one()["parent"]["name"], "Area");
    let orphan = muster(
        &fake,
        &["label", "create", "--name", "Lost", "--parent", "Nope"],
    );
    assert_eq!(orphan.code, 4, "{}", orphan.stderr);

    let renamed = muster(
        &fake,
        &[
            "label", "update", "Smoke", "--name", "Smoky", "--parent", "Area",
        ],
    );
    assert_eq!(renamed.code, 0, "{}", renamed.stderr);
    assert_eq!(renamed.one()["name"], "Smoky");
    assert_eq!(renamed.one()["parent"]["name"], "Area");
    assert_eq!(
        renamed.one()["team"]["key"],
        "APP",
        "the team's own label is the one renamed"
    );
    let out = muster(&fake, &["label", "update", "Smoky", "--parent", "none"]);
    assert!(out.one()["parent"].is_null());
    let itself = muster(&fake, &["label", "update", "Area", "--parent", "area"]);
    assert_eq!(itself.code, 2, "{}", itself.stderr);
    let nothing = muster(&fake, &["label", "update", "Smoky"]);
    assert_eq!(nothing.code, 2, "{}", nothing.stderr);

    let retired = muster(&fake, &["label", "retire", "Smoky"]);
    assert_eq!(retired.code, 0, "{}", retired.stderr);
    assert!(retired.one()["retiredAt"].is_string());
    let refused = muster(
        &fake,
        &[
            "issue",
            "create",
            "--title",
            "With a retired label",
            "--label",
            "Smoky",
        ],
    );
    assert_eq!(
        refused.code, 6,
        "a retired label goes on no new issue: {}",
        refused.stderr
    );
    let restored = muster(&fake, &["label", "restore", "Smoky"]);
    assert_eq!(restored.code, 0, "{}", restored.stderr);
    assert!(restored.one()["retiredAt"].is_null());

    let labelled = muster(
        &fake,
        &["issue", "create", "--title", "Labelled", "--label", "Smoky"],
    );
    assert_eq!(labelled.code, 0, "{}", labelled.stderr);
    let id = labelled.one()["identifier"].as_str().unwrap().to_string();
    let deleted = muster(&fake, &["label", "delete", "Smoky"]);
    assert_eq!(deleted.code, 0, "{}", deleted.stderr);
    assert_eq!(deleted.one()["success"], true);
    assert!(deleted.one()["entityId"].is_string());
    let after = muster(&fake, &["issue", "get", &id]).one();
    assert_eq!(
        after["labels"]["nodes"].as_array().unwrap().len(),
        0,
        "a deleted label leaves its issues"
    );
    let gone = muster(&fake, &["label", "delete", "Smoky"]);
    assert_eq!(gone.code, 4, "{}", gone.stderr);

    for (op, args) in [
        (
            "IssueLabelCreate",
            vec!["label", "create", "--name", "Dropped"],
        ),
        (
            "IssueLabelUpdate",
            vec!["label", "update", "Bug", "--color", "#000000"],
        ),
        ("IssueLabelRetire", vec!["label", "retire", "Bug"]),
        ("IssueLabelRestore", vec!["label", "restore", "Bug"]),
        ("IssueLabelDelete", vec!["label", "delete", "Bug"]),
    ] {
        fake.fail_next(op, Failure::Drop);
        let run = muster(&fake, &args);
        assert_eq!(run.code, 8, "{op}: {}", run.stderr);
    }
}

// ---- members (ADR 0014, group 3) ---------------------------------------------------------

#[test]
fn member_remove_past_the_bound_is_refused_rather_than_called_a_non_member() {
    let fake = Fake::start().unwrap();
    // APP has its two seeded members; PAGE_CAP - 1 more make PAGE_CAP + 1, and the
    // last one added is past the one page of PAGE_CAP that `member remove` reads.
    fake.add_members("APP", PAGE_CAP - 1);
    let last = format!("member-{}@example.com", PAGE_CAP - 1);
    let run = muster(&fake, &["member", "remove", "--user", &last]);
    assert_eq!(run.code, 6, "{}", run.stderr);
    let e = run.error();
    assert_eq!(e["code"], "refused");
    assert!(
        e["message"]
            .as_str()
            .unwrap()
            .contains("more than PAGE_CAP (250) members"),
        "{e}"
    );
    assert!(e["hint"].as_str().unwrap().contains("browser"), "{e}");
    assert!(e["docs"].as_str().unwrap().ends_with("#bounds"), "{e}");
    assert!(
        !fake
            .operations()
            .iter()
            .any(|o| o == "TeamMembershipDelete"),
        "refused before any mutation: {:?}",
        fake.operations()
    );
    assert_eq!(
        muster(&fake, &["member", "list", "--limit", "250"])
            .lines()
            .len(),
        250,
        "the member is still there"
    );

    // A member inside the first page still goes.
    let run = muster(&fake, &["member", "remove", "--user", "other@example.com"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.one()["success"], true);

    // At the bound is not over it: the team now has exactly PAGE_CAP members, the
    // page is not cut, and a user who is not one of them is exit 4, as before.
    let run = muster(&fake, &["member", "remove", "--user", "other@example.com"]);
    assert_eq!(run.code, 4, "{}", run.stderr);
    assert_eq!(run.error()["code"], "not_found");
    let deletes = fake
        .operations()
        .iter()
        .filter(|o| *o == "TeamMembershipDelete")
        .count();
    assert_eq!(
        deletes, 1,
        "the one removal, and nothing for the two refusals"
    );
}

#[test]
fn member_list_add_and_remove_work_on_the_teams_memberships() {
    let fake = Fake::start().unwrap();
    let run = muster(&fake, &["member", "list"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let members = run.lines();
    assert_eq!(members.len(), 2, "the two seeded people");
    assert_eq!(members[0]["owner"], true, "the first is the owner");
    assert_eq!(members[0]["user"]["email"], "fake@example.com");
    assert_eq!(members[0]["team"]["key"], "APP");
    assert!(members[0]["id"].is_string() && members[0]["sortOrder"].is_number());
    assert_eq!(fake.operations(), vec!["Teams", "TeamMembers"]);
    let cut = muster(&fake, &["member", "list", "--limit", "1"]);
    assert_eq!(cut.code, 0, "{}", cut.stderr);
    assert_eq!(cut.lines().len(), 1);
    assert_eq!(cut.notices()[0]["code"], "truncated", "{}", cut.stderr);

    let created = muster(
        &fake,
        &["team", "create", "--name", "Smoke tests", "--key", "SMK"],
    );
    assert_eq!(created.code, 0, "{}", created.stderr);
    let own = muster(&fake, &["--team", "SMK", "member", "list"]);
    assert_eq!(own.lines().len(), 1, "the creator is the one member");
    assert_eq!(
        own.lines()[0]["owner"],
        false,
        "as Linear answered for a team made through the API (appshapes, 2026-11-03)"
    );
    let added = muster(
        &fake,
        &[
            "--team",
            "SMK",
            "member",
            "add",
            "--user",
            "other@example.com",
        ],
    );
    assert_eq!(added.code, 0, "{}", added.stderr);
    let v = added.one();
    assert_eq!(v["user"]["displayName"], "other");
    assert_eq!(v["team"]["key"], "SMK");
    assert_eq!(v["owner"], false);
    assert_eq!(
        muster(&fake, &["--team", "SMK", "member", "list"])
            .lines()
            .len(),
        2
    );
    let again = muster(
        &fake,
        &["--team", "SMK", "member", "add", "--user", "other"],
    );
    assert_eq!(
        again.code, 6,
        "Linear refuses a second membership: {}",
        again.stderr
    );
    assert!(again.error()["linear"]["message"].is_string());
    let nobody = muster(
        &fake,
        &["--team", "SMK", "member", "add", "--user", "nobody"],
    );
    assert_eq!(nobody.code, 4, "{}", nobody.stderr);

    let removed = muster(
        &fake,
        &["--team", "SMK", "member", "remove", "--user", "other"],
    );
    assert_eq!(removed.code, 0, "{}", removed.stderr);
    assert_eq!(removed.one()["success"], true);
    assert!(removed.one()["entityId"].is_string());
    assert_eq!(
        muster(&fake, &["--team", "SMK", "member", "list"])
            .lines()
            .len(),
        1
    );
    let not_member = muster(
        &fake,
        &["--team", "SMK", "member", "remove", "--user", "other"],
    );
    assert_eq!(not_member.code, 4, "{}", not_member.stderr);
    assert!(
        not_member.error()["message"]
            .as_str()
            .unwrap()
            .contains("not a member"),
        "{}",
        not_member.stderr
    );
    let deletes = fake
        .operations()
        .iter()
        .filter(|o| *o == "TeamMembershipDelete")
        .count();
    assert_eq!(deletes, 1, "a non-member sends no mutation");

    fake.fail_next("TeamMembershipCreate", Failure::Drop);
    let run = muster(
        &fake,
        &["--team", "SMK", "member", "add", "--user", "other"],
    );
    assert_eq!(run.code, 8, "{}", run.stderr);
    fake.fail_next("TeamMembershipDelete", Failure::Drop);
    let run = muster(
        &fake,
        &["member", "remove", "--user", "other", "--team", "APP"],
    );
    assert_eq!(run.code, 8, "{}", run.stderr);
    fake.fail_next("TeamMembers", Failure::Drop);
    let run = muster(&fake, &["--team", "APP", "member", "list"]);
    assert_eq!(run.code, 7, "{}", run.stderr);
    let nowhere = muster(&fake, &["member", "list"]);
    assert_eq!(
        nowhere.code, 2,
        "two teams and none named: {}",
        nowhere.stderr
    );
}

#[test]
fn workspace_update_changes_only_what_is_passed_and_reads_back() {
    let fake = Fake::start().unwrap();
    let run = muster(
        &fake,
        &[
            "workspace",
            "update",
            "--name",
            "Renamed",
            "--roadmap",
            "true",
            "--project-update-reminders-day",
            "Monday",
            "--project-update-reminders-hour",
            "9",
            "--pull-request-issue-mode",
            "autoOnMerge",
            "--invitations-role",
            "admin",
            "--allowed-auth-services",
            "all",
            "--git-branch-format",
            "none",
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let v = run.one();
    assert_eq!(v["name"], "Renamed");
    assert_eq!(v["roadmapEnabled"], true);
    assert_eq!(
        v["feedEnabled"], false,
        "a flag left out leaves its setting alone"
    );
    assert_eq!(v["projectUpdateRemindersDay"], "Monday");
    assert_eq!(v["projectUpdateRemindersHour"], 9);
    assert_eq!(v["pullRequestIssueMode"], "autoOnMerge");
    assert_eq!(v["securitySettings"]["invitationsRole"], "admin");
    assert_eq!(v["authSettings"]["allowedAuthServices"], json!([]));
    assert!(v["gitBranchFormat"].is_null(), "`none` clears the format");
    assert_eq!(v["urlKey"], "fake", "untouched");
    let back = muster(&fake, &["workspace", "get"]).one();
    assert_eq!(back["name"], "Renamed");
    assert_eq!(back["projectUpdateRemindersDay"], "Monday");
    assert_eq!(
        fake.operations(),
        vec!["WorkspaceUpdate", "Workspace"],
        "one mutation, no team resolved"
    );

    let moved = muster(&fake, &["workspace", "update", "--url-key", "moved"]);
    assert_eq!(moved.code, 0, "{}", moved.stderr);
    assert_eq!(moved.one()["urlKey"], "moved");
    assert_eq!(
        moved.one()["previousUrlKeys"],
        json!(["fake"]),
        "the old key keeps redirecting, as Linear keeps the last three"
    );

    let nothing = muster(&fake, &["workspace", "update"]);
    assert_eq!(nothing.code, 2, "{}", nothing.stderr);
    assert_eq!(nothing.error()["message"], "nothing to change");
    let maybe = muster(&fake, &["workspace", "update", "--roadmap", "maybe"]);
    assert_eq!(
        maybe.code, 2,
        "a boolean is true or false: {}",
        maybe.stderr
    );
    let late = muster(
        &fake,
        &[
            "workspace",
            "update",
            "--project-update-reminders-hour",
            "24",
        ],
    );
    assert_eq!(late.code, 2, "{}", late.stderr);
    let someday = muster(
        &fake,
        &[
            "workspace",
            "update",
            "--project-update-reminders-day",
            "monday",
        ],
    );
    assert_eq!(
        someday.code, 2,
        "the day is spelt as Linear's enum spells it: {}",
        someday.stderr
    );
    for empty in ["", ","] {
        let none = muster(
            &fake,
            &["workspace", "update", "--allowed-auth-services", empty],
        );
        assert_eq!(
            none.code, 2,
            "{empty:?} is the empty list too, which only `all` may send: {}",
            none.stderr
        );
        assert_eq!(
            none.error()["message"],
            "--allowed-auth-services names no service"
        );
    }
    assert_eq!(
        fake.operations()
            .iter()
            .filter(|o| *o == "WorkspaceUpdate")
            .count(),
        2,
        "the usage errors sent nothing"
    );

    fake.fail_next("WorkspaceUpdate", Failure::Drop);
    let run = muster(&fake, &["workspace", "update", "--feed", "true"]);
    assert_eq!(run.code, 8, "{}", run.stderr);
    assert_eq!(run.error()["code"], "unknown");
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

    fake.fail_next("ProjectCreate", Failure::Drop);
    let run = muster(&fake, &["project", "create", "--name", "Dropped"]);
    assert_eq!(run.code, 8, "{}", run.stderr);
    assert_eq!(run.error()["code"], "unknown");

    fake.fail_next("AttachmentCreate", Failure::Drop);
    let run = muster(
        &fake,
        &[
            "issue",
            "attach",
            "APP-1",
            "--url",
            "https://example.com",
            "--title",
            "Dropped",
        ],
    );
    assert_eq!(run.code, 8, "{}", run.stderr);
    assert_eq!(run.error()["code"], "unknown");

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
        let variables = muster::cli::schema::fixture_variables(
            &name,
            Some("APP-1"),
            Some("APP"),
            Some("Muster"),
            Some("Launch"),
        )
        .unwrap();
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
        checked >= muster::linear::operations::READS.len(),
        "only {checked} fixtures; run just record-fixtures"
    );
}
