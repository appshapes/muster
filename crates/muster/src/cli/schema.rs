//! `muster schema`: the command tree, exit codes, notices, bounds and environment
//! variables, as one JSON object or as the reference page (ADR 0008); and `muster
//! fixtures`, which records the shape of every read operation's answer (ADR 0010).

use std::fmt::Write as _;
use std::path::Path;

use clap::{ArgAction, Command, CommandFactory};
use serde_json::{Map, Value, json};

use crate::bounds;
use crate::bounds::TEAMS_PER_PROJECT;
use crate::cli::{Cli, SchemaArgs, SchemaFormat};
use crate::config::{ENVIRONMENT, LINEAR_ENDPOINT};
use crate::error::{Error, ErrorCode};
use crate::linear::operations as ops;
use crate::linear::select::{nodes, str_at};
use crate::output::Output;
use crate::{DOCS_URL, VERSION};

use super::run::Session;

/// The notices a command can write to stderr while exiting 0.
pub const NOTICES: &[(&str, &str)] = &[
    (
        "truncated",
        "a list was cut at its --limit, or an issue's comments, attachments or sub-issues at their bound; the printed items are complete, there are more",
    ),
    (
        "skipped",
        "`fixtures` found no issue to record `IssueById` on, so that file was not written",
    ),
    (
        "partial",
        "`fixtures` recorded `IssueById` on an issue that lacks comments, attachments or sub-issues, so that node shape is not in the fixture",
    ),
];

const GLOBAL: &[&str] = &["human", "trace", "team", "help", "version"];

/// Run `schema`.
pub fn run(args: &SchemaArgs, out: &Output) -> Result<(), Error> {
    let schema = schema();
    match args.format {
        SchemaFormat::Json => out.emit(&schema),
        SchemaFormat::Markdown => out.emit_text(&markdown(&schema)),
    }
    Ok(())
}

/// The schema as a value.
pub fn schema() -> Value {
    let root = Cli::command();
    let mut commands = Vec::new();
    walk(&root, &mut Vec::new(), &mut commands);
    let global: Vec<Value> = root
        .get_arguments()
        .filter(|a| a.is_global_set())
        .map(describe_arg)
        .collect();
    json!({
        "name": "muster",
        "version": VERSION,
        "about": root.get_about().map(ToString::to_string),
        "docs": DOCS_URL,
        "globalFlags": global,
        "commands": commands,
        "exitCodes": ErrorCode::ALL.iter().map(|c| json!({ "exit": c.exit(), "code": c.name(), "meaning": c.meaning(), "hint": c.hint() })).collect::<Vec<_>>(),
        "notices": NOTICES.iter().map(|(code, meaning)| json!({ "code": code, "meaning": meaning })).collect::<Vec<_>>(),
        "bounds": bounds::ALL.iter().map(|b| json!({ "name": b.name, "value": b.value, "unit": b.unit, "meaning": b.meaning })).collect::<Vec<_>>(),
        "environment": ENVIRONMENT.iter().map(|e| json!({ "variable": e.name, "readBy": e.read_by, "whyNotDefault": e.why_not_default, "whoamiCheck": e.whoami_check })).collect::<Vec<_>>(),
        "operations": ops::READS.iter().chain(ops::WRITES.iter()).map(|op| json!({ "name": op.name, "kind": match op.kind { ops::Kind::Query => "query", ops::Kind::Mutation => "mutation" } })).collect::<Vec<_>>(),
    })
}

fn walk(cmd: &Command, path: &mut Vec<String>, out: &mut Vec<Value>) {
    let subcommands: Vec<&Command> = cmd
        .get_subcommands()
        .filter(|c| c.get_name() != "help")
        .collect();
    if subcommands.is_empty() && !path.is_empty() {
        let mut arguments = Vec::new();
        let mut flags = Vec::new();
        for arg in cmd.get_arguments() {
            if GLOBAL.contains(&arg.get_id().as_str()) || arg.is_global_set() {
                continue;
            }
            if arg.is_positional() {
                arguments.push(describe_arg(arg));
            } else {
                flags.push(describe_arg(arg));
            }
        }
        out.push(json!({
            "path": path.join(" "),
            "about": cmd.get_about().map(ToString::to_string),
            "arguments": arguments,
            "flags": flags,
        }));
        return;
    }
    for sub in subcommands {
        path.push(sub.get_name().to_string());
        walk(sub, path, out);
        path.pop();
    }
}

fn describe_arg(arg: &clap::Arg) -> Value {
    let value = match arg.get_action() {
        ArgAction::SetTrue
        | ArgAction::SetFalse
        | ArgAction::Count
        | ArgAction::Help
        | ArgAction::HelpShort
        | ArgAction::HelpLong
        | ArgAction::Version => None,
        _ => {
            // The values clap knows, `owner|admin|user|guest` for a `ValueEnum` and
            // `true|false` for a bool, so that no enumerated flag can leave them out of
            // the reference; a flag clap cannot enumerate keeps its value's name.
            let known: Vec<String> = arg
                .get_possible_values()
                .iter()
                .filter(|v| !v.is_hide_set())
                .map(|v| v.get_name().to_string())
                .collect();
            Some(if known.is_empty() {
                arg.get_value_names()
                    .and_then(|v| v.first())
                    .map(ToString::to_string)
                    .unwrap_or_else(|| arg.get_id().to_string().to_uppercase())
            } else {
                known.join("|")
            })
        }
    };
    let defaults: Vec<String> = arg
        .get_default_values()
        .iter()
        .map(|d| d.to_string_lossy().to_string())
        .collect();
    json!({
        "name": arg.get_id().as_str(),
        "long": arg.get_long().map(|l| format!("--{l}")),
        "positional": arg.is_positional(),
        "value": value,
        "required": arg.is_required_set(),
        "repeatable": matches!(arg.get_action(), ArgAction::Append),
        "default": if defaults.is_empty() { Value::Null } else { json!(defaults.join(" ")) },
        "help": arg.get_help().map(ToString::to_string),
    })
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

/// The reference page, `docs/reference/commands.md`.
pub fn markdown(schema: &Value) -> String {
    let mut md = String::new();
    let _ = writeln!(md, "# Commands, exit codes, bounds and environment\n");
    let _ = writeln!(
        md,
        "Generated by `just gen-commands` from `muster schema --format markdown`. Do not edit by hand; edit `crates/muster/src/` and run the recipe (CLAUDE.md, invariant 6; ADR 0008). `just lint-commands` fails when this page is stale.\n"
    );
    let _ = writeln!(
        md,
        "Every command prints one JSON object per line on stdout with Linear's own field names, nested as Linear nests them, and exits 0. On failure it prints one error object on stderr, `{{\"error\": {{\"code\", \"exit\", \"message\", \"hint\", \"docs\", \"linear\": {{\"code\", \"message\", \"requestId\"}}}}}}`, and exits with the code below. `--human` renders the same data as text. Nothing prompts, pages or colours.\n"
    );
    let _ = writeln!(md, "## Global flags\n");
    let _ = writeln!(md, "| Flag | Value | Meaning |\n| --- | --- | --- |");
    for flag in schema["globalFlags"].as_array().into_iter().flatten() {
        let _ = writeln!(
            md,
            "| `{}` | {} | {} |",
            str_at(flag, "long").unwrap_or("?"),
            value_cell(flag),
            cell(str_at(flag, "help").unwrap_or(""))
        );
    }
    let _ = writeln!(md, "\n## Commands\n");
    for command in schema["commands"].as_array().into_iter().flatten() {
        let _ = writeln!(
            md,
            "### muster {}\n",
            str_at(command, "path").unwrap_or("?")
        );
        let _ = writeln!(
            md,
            "{}.\n",
            str_at(command, "about").unwrap_or("").trim_end_matches('.')
        );
        let arguments = command["arguments"].as_array().cloned().unwrap_or_default();
        let flags = command["flags"].as_array().cloned().unwrap_or_default();
        if arguments.is_empty() && flags.is_empty() {
            let _ = writeln!(md, "No arguments beyond the global flags.\n");
            continue;
        }
        let _ = writeln!(
            md,
            "| Argument or flag | Value | Required | Default | Meaning |\n| --- | --- | --- | --- | --- |"
        );
        for arg in arguments.iter().chain(flags.iter()) {
            let name = match str_at(arg, "long") {
                Some(long) => format!("`{long}`"),
                None => format!("`<{}>`", str_at(arg, "value").unwrap_or("?")),
            };
            let required = if arg["required"].as_bool().unwrap_or(false) {
                "yes"
            } else {
                "no"
            };
            let default = str_at(arg, "default")
                .map(|d| format!("`{d}`"))
                .unwrap_or_default();
            let mut help = cell(str_at(arg, "help").unwrap_or(""));
            if arg["repeatable"].as_bool().unwrap_or(false) && !help.contains("repeatable") {
                help.push_str("; repeatable");
            }
            let _ = writeln!(
                md,
                "| {name} | {} | {required} | {default} | {help} |",
                value_cell(arg)
            );
        }
        let _ = writeln!(md);
    }
    let _ = writeln!(md, "## Exit codes\n");
    let _ = writeln!(
        md,
        "The table is append-only (ADR 0008). `0` is success; `2` is also what `clap` returns for a command line it cannot parse.\n"
    );
    let _ = writeln!(
        md,
        "| Exit | Code | Meaning | What to do |\n| --- | --- | --- | --- |"
    );
    let _ = writeln!(
        md,
        "| 0 | `ok` | the operation happened in Linear and the output is Linear's answer | nothing |"
    );
    for code in schema["exitCodes"].as_array().into_iter().flatten() {
        let _ = writeln!(
            md,
            "| {} | `{}` | {} | {} |",
            code["exit"],
            str_at(code, "code").unwrap_or("?"),
            cell(str_at(code, "meaning").unwrap_or("")),
            cell(str_at(code, "hint").unwrap_or(""))
        );
    }
    let _ = writeln!(md, "\n## Notices\n");
    let _ = writeln!(
        md,
        "A notice is one JSON object on stderr, `{{\"notice\": {{\"code\", \"message\", \"hint\"}}}}`, written while the command exits 0.\n"
    );
    let _ = writeln!(md, "| Code | Meaning |\n| --- | --- |");
    for notice in schema["notices"].as_array().into_iter().flatten() {
        let _ = writeln!(
            md,
            "| `{}` | {} |",
            str_at(notice, "code").unwrap_or("?"),
            cell(str_at(notice, "meaning").unwrap_or(""))
        );
    }
    let _ = writeln!(md, "\n## Bounds\n");
    let _ = writeln!(
        md,
        "Every limit muster has, from `crates/muster/src/bounds.rs` (CLAUDE.md, invariant 3). Nothing is retried, nothing sleeps.\n"
    );
    let _ = writeln!(
        md,
        "| Name | Value | Unit | Meaning |\n| --- | --- | --- | --- |"
    );
    for bound in schema["bounds"].as_array().into_iter().flatten() {
        let _ = writeln!(
            md,
            "| `{}` | {} | {} | {} |",
            str_at(bound, "name").unwrap_or("?"),
            bound["value"],
            str_at(bound, "unit").unwrap_or(""),
            cell(str_at(bound, "meaning").unwrap_or(""))
        );
    }
    let _ = writeln!(md, "\n## Environment\n");
    let _ = writeln!(
        md,
        "Every environment variable muster reads, each with the reason its default cannot be right for everyone (CLAUDE.md, invariant 5). Nothing else is configuration; there is no configuration file.\n"
    );
    let _ = writeln!(
        md,
        "| Variable | Read by | Why not default | `whoami` check |\n| --- | --- | --- | --- |"
    );
    for var in schema["environment"].as_array().into_iter().flatten() {
        let _ = writeln!(
            md,
            "| `{}` | {} | {} | {} |",
            str_at(var, "variable").unwrap_or("?"),
            cell(str_at(var, "readBy").unwrap_or("")),
            cell(str_at(var, "whyNotDefault").unwrap_or("")),
            cell(str_at(var, "whoamiCheck").unwrap_or(""))
        );
    }
    let _ = writeln!(md, "\n## The key\n");
    let _ = writeln!(
        md,
        "The key is a Linear personal API key (Linear, Settings, Security & access, Personal API keys). It is read, in this order, from `LINEAR_API_KEY`; else from the file `MUSTER_KEY_FILE` names; else from `$XDG_CONFIG_HOME/muster/key` or `~/.config/muster/key`, trimmed of whitespace. It is never a flag, never printed, never in `--trace`, and never in a fixture (ADR 0008). To write the file on a laptop:\n"
    );
    let _ = writeln!(
        md,
        "```sh\nmkdir -p ~/.config/muster && (umask 077; cat > ~/.config/muster/key)   # paste the key, then Ctrl-D\n```\n"
    );
    let _ = writeln!(
        md,
        "`muster whoami` says which source was read and exits 3 when none held a key. The endpoint is `{LINEAR_ENDPOINT}` unless `MUSTER_ENDPOINT` says otherwise, which only the tests and the quickstart do (ADR 0010).\n"
    );
    let _ = writeln!(md, "## Operations\n");
    let _ = writeln!(
        md,
        "The named GraphQL operations muster sends, from `crates/muster/src/linear/operations.rs`; the fake Linear answers by the name and the fixtures are recorded by it (ADR 0010).\n"
    );
    let _ = writeln!(md, "| Operation | Kind |\n| --- | --- |");
    for op in schema["operations"].as_array().into_iter().flatten() {
        let _ = writeln!(
            md,
            "| `{}` | {} |",
            str_at(op, "name").unwrap_or("?"),
            str_at(op, "kind").unwrap_or("")
        );
    }
    md
}

/// The Value column: `<TEXT>`, or the values the flag takes, `<owner|admin|user|guest>`,
/// with the pipes escaped as a table cell needs even inside a code span.
fn value_cell(arg: &Value) -> String {
    str_at(arg, "value")
        .map(|v| format!("`<{}>`", cell(v)))
        .unwrap_or_default()
}

/// `muster fixtures <dir>`: one `<Operation>.json` per read operation, holding the
/// answer's shape with every value stripped to its type (ADR 0010).
pub fn record_fixtures(session: &Session, dir: &Path, out: &Output) -> Result<(), Error> {
    std::fs::create_dir_all(dir).map_err(|e| {
        Error::new(
            ErrorCode::Usage,
            format!("cannot create {}: {e}", dir.display()),
        )
    })?;
    let (sample, lack) = sample_issue(session)?;
    let team = sample_team(session)?;
    match (&sample, lack) {
        (Some(id), Some(lack)) => out.notice(
            "partial",
            &format!("IssueById: recorded on {id}, which {lack}"),
            "add what is missing to an issue and run `just record-fixtures` again; the fixture test cannot hold the fake to a shape that was not recorded",
        ),
        (None, _) => out.notice(
            "skipped",
            "IssueById: the workspace has no issue to record",
            "create one and run `just record-fixtures` again",
        ),
        _ => {}
    }
    for op in ops::READS {
        // The same variables the fixture test replays against the fake, from one place.
        let Some(variables) = fixture_variables(op.name, sample.as_deref(), team.as_deref()) else {
            if ["IssueById", "TeamByKey", "TeamMembers"].contains(&op.name) {
                continue;
            }
            return Err(Error::new(
                ErrorCode::Failed,
                format!("no variables known for {}", op.name),
            ));
        };
        let data = session.linear.run(op, variables)?;
        let shape = shape_of(&data);
        let path = dir.join(format!("{}.json", op.name));
        let text = serde_json::to_string_pretty(&shape)?;
        std::fs::write(&path, format!("{text}\n")).map_err(|e| {
            Error::new(
                ErrorCode::Failed,
                format!("cannot write {}: {e}", path.display()),
            )
        })?;
        out.emit(&json!({ "fixture": op.name, "path": path.display().to_string() }));
    }
    Ok(())
}

/// The issue `IssueById` is recorded on, and what it lacks: the newest issue whose
/// comments, attachments and sub-issues are all non-empty, so that every node shape is
/// recorded; else the newest with a sub-issue; else the newest issue. (Linear orders
/// `createdAt` newest first.)
fn sample_issue(session: &Session) -> Result<(Option<String>, Option<&'static str>), Error> {
    let candidates: [(Value, Option<&'static str>); 3] = [
        (
            json!({ "children": { "length": { "gt": 0 } }, "comments": { "length": { "gt": 0 } }, "attachments": { "length": { "gt": 0 } } }),
            None,
        ),
        (
            json!({ "children": { "length": { "gt": 0 } } }),
            Some("has a sub-issue but lacks a comment or an attachment"),
        ),
        (Value::Null, Some("has no sub-issue")),
    ];
    for (filter, lack) in candidates {
        let data = session.linear.run(
            &ops::ISSUES,
            json!({ "first": 1, "filter": filter, "orderBy": "createdAt" }),
        )?;
        if let Some(id) = nodes(&data, "issues")
            .first()
            .and_then(|i| str_at(i, "identifier").map(str::to_string))
        {
            return Ok((Some(id), lack));
        }
    }
    Ok((None, None))
}

/// The key of the team `TeamByKey` is recorded on: the first team `Teams` lists.
fn sample_team(session: &Session) -> Result<Option<String>, Error> {
    let data = session.linear.run(&ops::TEAMS, json!({ "first": 1 }))?;
    Ok(nodes(&data, "teams")
        .first()
        .and_then(|t| str_at(t, "key").map(str::to_string)))
}

/// The variables each read operation is recorded with, and the fixture test replays
/// against the fake; `None` when `IssueById` has no issue, or `TeamByKey` no team, to
/// ask for.
pub fn fixture_variables(
    op: &str,
    first_issue: Option<&str>,
    first_team: Option<&str>,
) -> Option<Value> {
    Some(match op {
        "Viewer" | "Workspace" => json!({}),
        "TeamByKey" => json!({ "key": first_team?, "includeArchived": false }),
        "TeamMembers" => json!({ "key": first_team?, "first": 5 }),
        "Teams" => json!({ "first": 5 }),
        "WorkflowStates" | "IssueLabels" => json!({ "first": 5, "filter": Value::Null }),
        "Users" => json!({ "first": 5, "filter": Value::Null, "includeDisabled": true }),
        "UserById" => json!({ "id": "me" }),
        "Projects" => json!({ "first": 5, "filter": Value::Null, "teams": TEAMS_PER_PROJECT }),
        // Issues that carry a label, so that the label node shape is recorded too.
        "Issues" => {
            json!({ "first": 2, "filter": { "labels": { "length": { "gt": 0 } } }, "orderBy": "createdAt" })
        }
        "IssueById" => {
            json!({ "id": first_issue?, "comments": 5, "attachments": 5, "children": 5 })
        }
        _ => return None,
    })
}

/// A value with every leaf replaced by its type: `"<string>"`, `0`, `true`, `null`;
/// an array by its first element's shape, or `[]`.
pub fn shape_of(value: &Value) -> Value {
    match value {
        Value::Null => Value::Null,
        Value::Bool(_) => Value::Bool(true),
        Value::Number(_) => json!(0),
        Value::String(_) => json!("<string>"),
        Value::Array(items) => match items.first() {
            Some(first) => json!([shape_of(first)]),
            None => json!([]),
        },
        Value::Object(map) => {
            let mut out = Map::new();
            for (k, v) in map {
                out.insert(k.clone(), shape_of(v));
            }
            Value::Object(out)
        }
    }
}

/// Every key path the fixture has that the answer lacks, or whose type differs; a
/// `null` in the fixture matches anything, since a recorded null carries no shape.
pub fn shape_mismatches(fixture: &Value, answer: &Value, path: &str, out: &mut Vec<String>) {
    match (fixture, answer) {
        (Value::Null, _) => {}
        (Value::Object(want), Value::Object(have)) => {
            for (k, v) in want {
                let here = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                match have.get(k) {
                    Some(h) => shape_mismatches(v, h, &here, out),
                    None => out.push(format!("{here}: missing")),
                }
            }
        }
        (Value::Array(want), Value::Array(have)) => {
            if let (Some(w), Some(h)) = (want.first(), have.first()) {
                shape_mismatches(w, h, &format!("{path}[]"), out);
            }
        }
        (Value::Array(_), other) | (Value::Object(_), other) if other.is_null() => {}
        (w, h) if std::mem::discriminant(w) != std::mem::discriminant(h) && !h.is_null() => {
            out.push(format!("{path}: fixture has {w}, answer has {h}"));
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_lists_every_command_and_code() {
        let s = schema();
        let paths: Vec<&str> = s["commands"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| str_at(c, "path"))
            .collect();
        for want in [
            "whoami",
            "workspace get",
            "workspace update",
            "team list",
            "team get",
            "team create",
            "team update",
            "team retire",
            "team unretire",
            "team delete",
            "team unarchive",
            "member list",
            "member add",
            "member remove",
            "user get",
            "user update",
            "user change-role",
            "user suspend",
            "user unsuspend",
            "user revoke-sessions",
            "state list",
            "label list",
            "project list",
            "project create",
            "user list",
            "issue get",
            "issue list",
            "issue create",
            "issue comment",
            "issue attach",
            "issue close",
            "issue update",
            "schema",
            "fixtures",
        ] {
            assert!(paths.contains(&want), "{want} missing from {paths:?}");
        }
        assert_eq!(
            s["exitCodes"].as_array().map(Vec::len),
            Some(ErrorCode::ALL.len())
        );
        assert_eq!(
            s["bounds"].as_array().map(Vec::len),
            Some(bounds::ALL.len())
        );
        assert_eq!(
            s["environment"].as_array().map(Vec::len),
            Some(ENVIRONMENT.len())
        );
    }

    /// A flag whose values clap knows lists them in the reference, so that a stranger
    /// learns there that a day is spelt `Monday` and what the four roles are called
    /// without leaving for `--help`; a flag it cannot enumerate keeps its value's name.
    #[test]
    fn an_enumerated_flag_lists_its_values() {
        let s = schema();
        let flags: Vec<&Value> = s["commands"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|c| str_at(c, "path") == Some("workspace update"))
            .flat_map(|c| c["flags"].as_array().into_iter().flatten())
            .collect();
        let value = |long: &str| {
            flags
                .iter()
                .find(|f| str_at(f, "long") == Some(long))
                .and_then(|f| str_at(f, "value"))
                .unwrap_or("?")
                .to_string()
        };
        assert_eq!(value("--invitations-role"), "owner|admin|user|guest");
        assert_eq!(
            value("--project-update-reminders-day"),
            "Sunday|Monday|Tuesday|Wednesday|Thursday|Friday|Saturday"
        );
        assert_eq!(
            value("--pull-request-issue-mode"),
            "off|suggest|autoOnMerge"
        );
        assert_eq!(value("--roadmap"), "true|false", "clap's own bool parser");
        assert_eq!(value("--git-branch-format"), "TEXT|none");
        assert_eq!(
            value("--project-update-reminders-hour"),
            "0-23",
            "a range is not enumerated; its value name says the bounds"
        );
        assert_eq!(value("--name"), "TEXT");
        let md = markdown(&s);
        assert!(
            md.contains("| `--invitations-role` | `<owner\\|admin\\|user\\|guest>` |"),
            "the pipes are escaped, so the row keeps its five cells"
        );
        assert!(md.contains("| `--format` | `<json\\|markdown>` |"));
    }

    #[test]
    fn markdown_has_the_contract_headings() {
        let md = markdown(&schema());
        for heading in [
            "## Commands",
            "### muster issue get",
            "## Exit codes",
            "## Bounds",
            "## Environment",
            "## The key",
            "| `LINEAR_API_KEY` |",
        ] {
            assert!(md.contains(heading), "{heading} missing");
        }
    }

    #[test]
    fn shapes() {
        let real = json!({ "a": "x", "b": 3, "c": [ { "d": true } ], "e": null, "f": [] });
        assert_eq!(
            shape_of(&real),
            json!({ "a": "<string>", "b": 0, "c": [ { "d": true } ], "e": null, "f": [] })
        );
        let mut found = Vec::new();
        shape_mismatches(
            &shape_of(&real),
            &json!({ "a": "y", "b": "no", "c": [ {} ], "e": 1 }),
            "",
            &mut found,
        );
        assert_eq!(
            found,
            vec![
                "b: fixture has 0, answer has \"no\"",
                "c[].d: missing",
                "f: missing"
            ]
        );
    }
}
